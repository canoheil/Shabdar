//! Phase 3 Task 6 (docs/PHASE3_PLAN.md): admin listing queue — approve,
//! reject, remove — each writing exactly one audit row in the same
//! transaction.

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::Row;
use tower::ServiceExt;

/// All admin tests share one database; run them one at a time.
static ADMIN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Same child-first reset as the other suites (see tests/db_migrations.rs).
async fn reset_db(pool: &sqlx::MySqlPool) {
    for table in [
        "favorites",
        "listing_images",
        "listings",
        "admin_audit_logs",
        "sessions",
        "otp_challenges",
        "cities",
        "categories",
        "provinces",
        "users",
        "_sqlx_migrations",
    ] {
        let sql = format!("DROP TABLE IF EXISTS {table}"); // fixed literals
        sqlx::query(&sql).execute(pool).await.expect("drop failed");
    }
    shabdar_api::db::run_migrations(pool)
        .await
        .expect("migrations apply");
}

async fn challenge_with_code(
    pool: &sqlx::MySqlPool,
    cfg: &shabdar_api::config::Config,
    phone: &str,
    purpose: &str,
) -> (String, String) {
    let code = shabdar_api::otp::generate_code();
    let created = shabdar_api::otp_service::create_challenge(pool, cfg, phone, purpose, &code)
        .await
        .expect("create challenge");
    (created.public_id, code)
}

/// Register through the real OTP flow; return (cookie header, csrf token).
async fn register_user(
    app: &axum::Router,
    pool: &sqlx::MySqlPool,
    cfg: &shabdar_api::config::Config,
    phone: &str,
) -> (String, String) {
    let (challenge_id, code) = challenge_with_code(pool, cfg, phone, "register").await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/otp/verify")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "challenge_id": challenge_id, "code": code }).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let mut session = String::new();
    let mut csrf = String::new();
    for value in resp.headers().get_all(axum::http::header::SET_COOKIE) {
        let text = value.to_str().expect("ascii cookie");
        if let Some(rest) = text.strip_prefix("shabdar_session=") {
            session = format!("shabdar_session={}", rest.split(';').next().unwrap());
        } else if let Some(rest) = text.strip_prefix("shabdar_csrf=") {
            csrf = rest.split(';').next().unwrap().to_string();
        }
    }
    (format!("{session}; shabdar_csrf={csrf}"), csrf)
}

/// Promote a registered phone to admin; returns that user's cookie pair.
async fn register_admin(
    app: &axum::Router,
    pool: &sqlx::MySqlPool,
    cfg: &shabdar_api::config::Config,
    phone: &str,
) -> (String, String) {
    let pair = register_user(app, pool, cfg, phone).await;
    sqlx::query("UPDATE users SET is_admin = TRUE WHERE phone_e164 = ?")
        .bind(phone)
        .execute(pool)
        .await
        .expect("promote to admin");
    pair
}

/// Send a JSON request with auth; return status + parsed JSON.
async fn send_authed(
    app: axum::Router,
    method: Method,
    path: &str,
    cookie: &str,
    csrf: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(path).header("cookie", cookie);
    if let Some(t) = csrf {
        builder = builder.header("x-csrf-token", t);
    }
    let req = if let Some(json) = body {
        builder = builder.header("content-type", "application/json");
        builder.body(Body::from(json.to_string())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    };
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).expect("JSON body"))
}

/// Create a draft via API, attach one image row via SQL, submit via API.
async fn submit_listing(
    app: &axum::Router,
    pool: &sqlx::MySqlPool,
    cookie: &str,
    csrf: &str,
    title: &str,
) -> String {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/listings")
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .body(Body::from(
            serde_json::json!({
                "category_id": 1, "city_id": 1, "title": title,
                "description": "شرح", "price_toman": 1_000_000, "price_type": "fixed",
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let public_id = created["public_id"].as_str().unwrap().to_string();
    sqlx::query(
        "INSERT INTO listing_images (listing_id, storage_key, sort_order, created_at) \
         SELECT id, 'setup/cover.png', 0, NOW(3) FROM listings WHERE public_id = ?",
    )
    .bind(&public_id)
    .execute(pool)
    .await
    .expect("image setup row");
    let (status, _) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/listings/{public_id}/submit"),
        cookie,
        Some(csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    public_id
}

async fn audit_count(pool: &sqlx::MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs")
        .fetch_one(pool)
        .await
        .expect("audit count")
}

#[tokio::test]
async fn queue_lists_pending_oldest_first() {
    let _lock = ADMIN_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller_a, csrf_a) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (seller_b, csrf_b) = register_user(&app, &pool, &cfg, "+989009876543").await;
    let (admin_cookie, _admin_csrf) = register_admin(&app, &pool, &cfg, "+989000000000").await;

    let first = submit_listing(&app, &pool, &seller_a, &csrf_a, "اول").await;
    let second = submit_listing(&app, &pool, &seller_b, &csrf_b, "دوم").await;
    // Distinct submission times pin the oldest-first order.
    sqlx::query("UPDATE listings SET submitted_at = NOW(3) - INTERVAL 1 HOUR WHERE public_id = ?")
        .bind(&first)
        .execute(&pool)
        .await
        .expect("backdate first");

    let (status, body) = send_authed(
        app,
        Method::GET,
        "/api/v1/admin/listings/queue",
        &admin_cookie,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().expect("data array");
    assert_eq!(data.len(), 2, "both pending listings: {body}");
    assert_eq!(data[0]["public_id"], serde_json::json!(first), "oldest first");
    assert_eq!(data[1]["public_id"], serde_json::json!(second));
}

#[tokio::test]
async fn non_admin_approve_is_forbidden_and_changes_nothing() {
    let _lock = ADMIN_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (other, other_csrf) = register_user(&app, &pool, &cfg, "+989009876543").await;
    let public_id = submit_listing(&app, &pool, &seller, &csrf, "فرش").await;

    let (status, body) = send_authed(
        app,
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/approve"),
        &other,
        Some(&other_csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");
    assert_eq!(audit_count(&pool).await, 0, "no audit row on rejection");

    let row = sqlx::query("SELECT status FROM listings WHERE public_id = ?")
        .bind(&public_id)
        .fetch_one(&pool)
        .await
        .expect("listing row");
    let status: String = row.try_get("status").unwrap();
    assert_eq!(status, "pending", "status unchanged");
}

#[tokio::test]
async fn approve_publishes_and_audits() {
    let _lock = ADMIN_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (admin_cookie, admin_csrf) = register_admin(&app, &pool, &cfg, "+989000000000").await;
    let public_id = submit_listing(&app, &pool, &seller, &csrf, "فرش").await;

    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/approve"),
        &admin_cookie,
        Some(&admin_csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "active");

    // Published: the public feed shows it now.
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/listings")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let feed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(feed["data"].as_array().unwrap().len(), 1);

    // Exactly one audit row, naming action + target.
    let row = sqlx::query(
        "SELECT action, target_type, reason FROM admin_audit_logs",
    )
    .fetch_one(&pool)
    .await
    .expect("audit row");
    let action: String = row.try_get("action").unwrap();
    let target_type: String = row.try_get("target_type").unwrap();
    assert_eq!(action, "listing.approve");
    assert_eq!(target_type, "listing");

    // Double approve → 400, no second audit row.
    let (status, _) = send_authed(
        app,
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/approve"),
        &admin_cookie,
        Some(&admin_csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(audit_count(&pool).await, 1);
}

#[tokio::test]
async fn reject_needs_reason_and_hides_from_public() {
    let _lock = ADMIN_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (admin_cookie, admin_csrf) = register_admin(&app, &pool, &cfg, "+989000000000").await;
    let public_id = submit_listing(&app, &pool, &seller, &csrf, "فرش").await;

    // No reason → 400, still pending, no audit row.
    let (status, _) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/reject"),
        &admin_cookie,
        Some(&admin_csrf),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(audit_count(&pool).await, 0);

    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/reject"),
        &admin_cookie,
        Some(&admin_csrf),
        Some(serde_json::json!({ "reason": "عکس نامناسب است" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "rejected");

    // Owner sees the reason on their own dashboard…
    let (status, mine) = send_authed(
        app.clone(),
        Method::GET,
        "/api/v1/me/listings?status=rejected",
        &seller,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(mine["data"].as_array().unwrap().len(), 1);
    assert_eq!(mine["data"][0]["rejection_reason"], serde_json::json!("عکس نامناسب است"));

    // …but the public detail stays 404 (slug known from the dashboard).
    let slug = mine["data"][0]["slug"].as_str().unwrap().to_string();
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/listings/{slug}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn remove_makes_detail_gone_and_failed_csrf_writes_no_audit() {
    let _lock = ADMIN_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (admin_cookie, admin_csrf) = register_admin(&app, &pool, &cfg, "+989000000000").await;
    let public_id = submit_listing(&app, &pool, &seller, &csrf, "فرش").await;
    // Publish first (direct setup — approve is covered by its own test).
    sqlx::query("UPDATE listings SET status = 'active', published_at = NOW(3) WHERE public_id = ?")
        .bind(&public_id)
        .execute(&pool)
        .await
        .expect("publish");

    // No CSRF header → 403 and no audit row (nothing mutated).
    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/remove"),
        &admin_cookie,
        None,
        Some(serde_json::json!({ "reason": "تخلف" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "csrf_failed");
    assert_eq!(audit_count(&pool).await, 0);

    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/remove"),
        &admin_cookie,
        Some(&admin_csrf),
        Some(serde_json::json!({ "reason": "تخلف" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "removed");

    let slug = body["slug"].as_str().unwrap().to_string();
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/listings/{slug}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::GONE);
    assert_eq!(audit_count(&pool).await, 1);
}
