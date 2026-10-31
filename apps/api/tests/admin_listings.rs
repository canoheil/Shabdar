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
        "comment_moderation_cases",
        "moderation_rules",
        "moderation_settings",
        "reports",
        "blocks",
        "listing_comments",
        "listing_ratings",
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
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookie);
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

fn admin_test_config() -> shabdar_api::config::Config {
    let dir = std::env::temp_dir().join(format!("shabdar-admin-review-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(dir);
    let mut cfg = common::test_config();
    cfg.media_dir = dir.to_string_lossy().into_owned();
    cfg
}

async fn upload_png(
    app: &axum::Router,
    public_id: &str,
    cookie: &str,
    csrf: &str,
) -> serde_json::Value {
    let boundary = "----shabdar-admin-image-review";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"review.png\"\r\nContent-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(include_bytes!("fixtures/tiny.png"));
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/listings/{public_id}/images"))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("upload response")
}

/// Create a draft, upload real static PNGs, and submit it through the API.
async fn submit_listing_with_images(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    title: &str,
    image_count: usize,
) -> (String, Vec<u64>) {
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
    let mut image_ids = Vec::with_capacity(image_count);
    for _ in 0..image_count {
        let image = upload_png(app, &public_id, cookie, csrf).await;
        image_ids.push(image["id"].as_u64().expect("uploaded image id"));
    }
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
    (public_id, image_ids)
}

async fn submit_listing(app: &axum::Router, cookie: &str, csrf: &str, title: &str) -> String {
    submit_listing_with_images(app, cookie, csrf, title, 1)
        .await
        .0
}

async fn activate_legacy(pool: &sqlx::MySqlPool, public_id: &str, published_at: &str) {
    sqlx::query(
        "UPDATE listings SET status = 'active', published_at = ?, \
         reviewed_by = seller_id, reviewed_at = '2025-02-03 04:05:06.000', \
         updated_at = '2025-03-04 05:06:07.000', \
         image_reviewed_by = NULL, image_reviewed_at = NULL, image_reviewed_ids = NULL \
         WHERE public_id = ?",
    )
    .bind(published_at)
    .bind(public_id)
    .execute(pool)
    .await
    .expect("legacy active fixture");
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
    let cfg = admin_test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller_a, csrf_a) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (seller_b, csrf_b) = register_user(&app, &pool, &cfg, "+989009876543").await;
    let (admin_cookie, _admin_csrf) = register_admin(&app, &pool, &cfg, "+989000000000").await;

    let first = submit_listing(&app, &seller_a, &csrf_a, "اول").await;
    let second = submit_listing(&app, &seller_b, &csrf_b, "دوم").await;
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
    assert_eq!(
        data[0]["public_id"],
        serde_json::json!(first),
        "oldest first"
    );
    assert_eq!(data[1]["public_id"], serde_json::json!(second));
}

#[tokio::test]
async fn non_admin_approve_is_forbidden_and_changes_nothing() {
    let _lock = ADMIN_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = admin_test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (other, other_csrf) = register_user(&app, &pool, &cfg, "+989009876543").await;
    let (public_id, image_ids) = submit_listing_with_images(&app, &seller, &csrf, "فرش", 1).await;

    let (status, body) = send_authed(
        app,
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/approve"),
        &other,
        Some(&other_csrf),
        Some(serde_json::json!({"image_ids": image_ids})),
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
async fn approve_publishes_exact_reviewed_images_with_detail_and_audit() {
    let _lock = ADMIN_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = admin_test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (admin_cookie, admin_csrf) = register_admin(&app, &pool, &cfg, "+989000000000").await;
    let (public_id, image_ids) = submit_listing_with_images(&app, &seller, &csrf, "فرش", 2).await;

    let (detail_status, detail) = send_authed(
        app.clone(),
        Method::GET,
        &format!("/api/v1/admin/listings/{public_id}"),
        &admin_cookie,
        None,
        None,
    )
    .await;
    assert_eq!(detail_status, StatusCode::OK);
    let detail_ids: Vec<u64> = detail["images"]
        .as_array()
        .expect("all admin images")
        .iter()
        .map(|image| image["id"].as_u64().unwrap())
        .collect();
    assert_eq!(detail_ids, image_ids);
    assert!(detail["images"].as_array().unwrap().iter().all(|image| {
        image["width"] == 3
            && image["height"] == 2
            && image["url"].as_str().unwrap().starts_with("/media/")
    }));
    assert!(detail["image_review"].is_null());
    let (_, queue) = send_authed(
        app.clone(),
        Method::GET,
        "/api/v1/admin/listings/queue",
        &admin_cookie,
        None,
        None,
    )
    .await;
    assert_eq!(queue["data"][0]["image_count"], 2);
    assert!(queue["data"][0]["cover_image"]
        .as_str()
        .unwrap()
        .starts_with("/media/"));

    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/approve"),
        &admin_cookie,
        Some(&admin_csrf),
        Some(serde_json::json!({"image_ids": &image_ids})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "active");

    let (_, reviewed) = send_authed(
        app.clone(),
        Method::GET,
        &format!("/api/v1/admin/listings/{public_id}"),
        &admin_cookie,
        None,
        None,
    )
    .await;
    assert_eq!(
        reviewed["image_review"]["image_ids"],
        serde_json::json!(&image_ids)
    );
    assert!(reviewed["image_review"]["reviewed_at"].is_string());
    let admin_public_id: String =
        sqlx::query_scalar("SELECT public_id FROM users WHERE phone_e164 = '+989000000000'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        reviewed["image_review"]["reviewer"]["public_id"],
        serde_json::json!(admin_public_id)
    );
    assert!(reviewed["image_review"]["reviewer"]
        .get("phone_e164")
        .is_none());

    // Published and reviewed: the public feed shows it now.
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

    let row = sqlx::query(
        "SELECT action, target_type, reason, CAST(metadata AS CHAR) AS metadata FROM admin_audit_logs",
    )
    .fetch_one(&pool)
    .await
    .expect("audit row");
    let action: String = row.try_get("action").unwrap();
    let target_type: String = row.try_get("target_type").unwrap();
    let metadata: String = row.try_get("metadata").unwrap();
    let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();
    assert_eq!(action, "listing.approve");
    assert_eq!(target_type, "listing");
    assert_eq!(metadata["image_ids"], serde_json::json!(&image_ids));

    // Double approve → 400, no second audit row.
    let (status, _) = send_authed(
        app,
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/approve"),
        &admin_cookie,
        Some(&admin_csrf),
        Some(serde_json::json!({"image_ids": &image_ids})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(audit_count(&pool).await, 1);
}

#[tokio::test]
async fn approve_rejects_missing_empty_duplicate_and_foreign_image_sets_atomically() {
    let _lock = ADMIN_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = admin_test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (admin_cookie, admin_csrf) = register_admin(&app, &pool, &cfg, "+989000000000").await;
    let (public_id, image_ids) =
        submit_listing_with_images(&app, &seller, &csrf, "دو تصویر", 2).await;
    let (_foreign_listing, foreign_ids) =
        submit_listing_with_images(&app, &seller, &csrf, "تصویر خارجی", 1).await;
    let first = image_ids[0];
    let second = image_ids[1];
    let foreign = foreign_ids[0];
    let cases = vec![
        (serde_json::json!({}), "required"),
        (serde_json::json!({"image_ids": null}), "required"),
        (serde_json::json!({"image_ids": []}), "mismatch"),
        (serde_json::json!({"image_ids": [first, first]}), "mismatch"),
        (serde_json::json!({"image_ids": [first]}), "mismatch"),
        (
            serde_json::json!({"image_ids": [first, second, foreign]}),
            "mismatch",
        ),
    ];
    let mut failures = Vec::new();
    for (payload, expected) in cases {
        let (status, body) = send_authed(
            app.clone(),
            Method::POST,
            &format!("/api/v1/admin/listings/{public_id}/approve"),
            &admin_cookie,
            Some(&admin_csrf),
            Some(payload.clone()),
        )
        .await;
        let actual = body["error"]["fields"]["image_ids"].as_str();
        if status != StatusCode::BAD_REQUEST || actual != Some(expected) {
            failures.push(format!(
                "payload={payload}, status={status}, reason={actual:?}, body={body}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let row = sqlx::query("SELECT status, image_reviewed_at FROM listings WHERE public_id = ?")
        .bind(&public_id)
        .fetch_one(&pool)
        .await
        .expect("pending listing");
    let status: String = row.try_get("status").unwrap();
    let reviewed_at: Option<chrono::DateTime<chrono::Utc>> =
        row.try_get("image_reviewed_at").unwrap();
    assert_eq!(status, "pending");
    assert!(reviewed_at.is_none());
    assert_eq!(audit_count(&pool).await, 0);
}

#[tokio::test]
async fn approve_revalidates_real_storage_and_backfills_dimensions_atomically() {
    let _lock = ADMIN_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = admin_test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (admin_cookie, admin_csrf) = register_admin(&app, &pool, &cfg, "+989000000000").await;
    let (public_id, image_ids) =
        submit_listing_with_images(&app, &seller, &csrf, "بازبینی فایل", 1).await;
    let key: String = sqlx::query_scalar("SELECT storage_key FROM listing_images WHERE id = ?")
        .bind(image_ids[0])
        .fetch_one(&pool)
        .await
        .expect("stored key");
    let path = std::path::Path::new(&cfg.media_dir).join(key);
    tokio::fs::remove_file(&path)
        .await
        .expect("remove stored fixture");
    let (missing_status, missing_body) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/approve"),
        &admin_cookie,
        Some(&admin_csrf),
        Some(serde_json::json!({"image_ids": &image_ids})),
    )
    .await;
    assert_eq!(missing_status, StatusCode::BAD_REQUEST);
    assert_eq!(missing_body["error"]["fields"]["images"], "unreadable");

    let valid = include_bytes!("fixtures/tiny.png");
    let cases: Vec<(&[u8], &str)> = vec![
        (&valid[..valid.len() / 2], "invalid_image"),
        (include_bytes!("fixtures/animated.png"), "animated"),
        (include_bytes!("fixtures/dimensions.png"), "too_large"),
    ];
    let mut failures = Vec::new();
    for (bytes, expected) in cases {
        tokio::fs::write(&path, bytes)
            .await
            .expect("corrupt stored fixture");
        let (status, body) = send_authed(
            app.clone(),
            Method::POST,
            &format!("/api/v1/admin/listings/{public_id}/approve"),
            &admin_cookie,
            Some(&admin_csrf),
            Some(serde_json::json!({"image_ids": &image_ids})),
        )
        .await;
        let actual = body["error"]["fields"]["images"].as_str();
        if status != StatusCode::BAD_REQUEST || actual != Some(expected) {
            failures.push(format!(
                "expected={expected}, status={status}, reason={actual:?}, body={body}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let status: String = sqlx::query_scalar("SELECT status FROM listings WHERE public_id = ?")
        .bind(&public_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "pending");
    assert_eq!(audit_count(&pool).await, 0);

    // Legacy rows may have NULL dimensions. Valid stored bytes are decoded
    // again and the measured values are persisted by the same approval.
    tokio::fs::write(&path, valid)
        .await
        .expect("restore valid fixture");
    sqlx::query("UPDATE listing_images SET width = NULL, height = NULL WHERE id = ?")
        .bind(image_ids[0])
        .execute(&pool)
        .await
        .expect("NULL legacy dimensions");
    let (status, _) = send_authed(
        app,
        Method::POST,
        &format!("/api/v1/admin/listings/{public_id}/approve"),
        &admin_cookie,
        Some(&admin_csrf),
        Some(serde_json::json!({"image_ids": &image_ids})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let dimensions = sqlx::query("SELECT width, height FROM listing_images WHERE id = ?")
        .bind(image_ids[0])
        .fetch_one(&pool)
        .await
        .expect("backfilled dimensions");
    assert_eq!(dimensions.try_get::<u16, _>("width").unwrap(), 3);
    assert_eq!(dimensions.try_get::<u16, _>("height").unwrap(), 2);
    assert_eq!(audit_count(&pool).await, 1);
}

#[tokio::test]
async fn legacy_queue_pages_and_exact_review_restores_without_rewriting_publication() {
    let _lock = ADMIN_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = admin_test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (admin_cookie, admin_csrf) = register_admin(&app, &pool, &cfg, "+989000000000").await;
    let (first, first_images) =
        submit_listing_with_images(&app, &seller, &csrf, "قدیمی اول", 1).await;
    let (second, _) = submit_listing_with_images(&app, &seller, &csrf, "قدیمی دوم", 1).await;
    let (zero, _) = submit_listing_with_images(&app, &seller, &csrf, "بدون تصویر", 1).await;
    activate_legacy(&pool, &first, "2025-01-01 01:02:03.000").await;
    activate_legacy(&pool, &second, "2025-01-02 01:02:03.000").await;
    activate_legacy(&pool, &zero, "2025-01-03 01:02:03.000").await;
    sqlx::query("DELETE FROM listing_images WHERE listing_id = (SELECT id FROM listings WHERE public_id = ?)")
        .bind(&zero)
        .execute(&pool)
        .await
        .expect("zero-image active fixture");

    let (_, first_page) = send_authed(
        app.clone(),
        Method::GET,
        "/api/v1/admin/listings/image-review-queue?limit=1",
        &admin_cookie,
        None,
        None,
    )
    .await;
    assert_eq!(first_page["data"].as_array().unwrap().len(), 1);
    assert_eq!(
        first_page["data"][0]["public_id"],
        serde_json::json!(&first)
    );
    assert_eq!(first_page["data"][0]["image_count"], 1);
    assert!(first_page["data"][0]["cover_image"].is_string());
    assert_eq!(first_page["page"]["has_more"], true);
    let cursor = first_page["page"]["next_cursor"].as_str().unwrap();
    let (_, second_page) = send_authed(
        app.clone(),
        Method::GET,
        &format!("/api/v1/admin/listings/image-review-queue?limit=1&cursor={cursor}"),
        &admin_cookie,
        None,
        None,
    )
    .await;
    assert_eq!(second_page["data"].as_array().unwrap().len(), 1);
    assert_eq!(
        second_page["data"][0]["public_id"],
        serde_json::json!(&second)
    );
    assert_eq!(second_page["page"]["has_more"], false);

    let (queue_forbidden, _) = send_authed(
        app.clone(),
        Method::GET,
        "/api/v1/admin/listings/image-review-queue",
        &seller,
        None,
        None,
    )
    .await;
    assert_eq!(queue_forbidden, StatusCode::FORBIDDEN);
    let (forbidden, _) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/admin/listings/{first}/images/review"),
        &seller,
        Some(&csrf),
        Some(serde_json::json!({"image_ids": &first_images})),
    )
    .await;
    assert_eq!(forbidden, StatusCode::FORBIDDEN);
    let (missing_csrf, _) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/admin/listings/{first}/images/review"),
        &admin_cookie,
        None,
        Some(serde_json::json!({"image_ids": &first_images})),
    )
    .await;
    assert_eq!(missing_csrf, StatusCode::FORBIDDEN);
    assert_eq!(audit_count(&pool).await, 0);

    let before = sqlx::query(
        "SELECT status, reviewed_by, published_at, reviewed_at, updated_at FROM listings WHERE public_id = ?",
    )
    .bind(&first)
    .fetch_one(&pool)
    .await
    .expect("publication timestamps");
    let (status, reviewed) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/admin/listings/{first}/images/review"),
        &admin_cookie,
        Some(&admin_csrf),
        Some(serde_json::json!({"image_ids": &first_images})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reviewed["status"], "active");
    assert_eq!(
        reviewed["image_review"]["image_ids"],
        serde_json::json!(&first_images)
    );
    let after = sqlx::query(
        "SELECT status, reviewed_by, published_at, reviewed_at, updated_at FROM listings WHERE public_id = ?",
    )
    .bind(&first)
    .fetch_one(&pool)
    .await
    .expect("unchanged publication timestamps");
    assert_eq!(
        after.try_get::<String, _>("status").unwrap(),
        before.try_get::<String, _>("status").unwrap()
    );
    assert_eq!(
        after.try_get::<Option<u64>, _>("reviewed_by").unwrap(),
        before.try_get::<Option<u64>, _>("reviewed_by").unwrap()
    );
    for column in ["published_at", "reviewed_at", "updated_at"] {
        let before_value: Option<chrono::DateTime<chrono::Utc>> = before.try_get(column).unwrap();
        let after_value: Option<chrono::DateTime<chrono::Utc>> = after.try_get(column).unwrap();
        assert_eq!(after_value, before_value, "{column} must not change");
    }

    let row =
        sqlx::query("SELECT action, CAST(metadata AS CHAR) AS metadata FROM admin_audit_logs")
            .fetch_one(&pool)
            .await
            .expect("legacy review audit");
    assert_eq!(
        row.try_get::<String, _>("action").unwrap(),
        "listing.images.approve"
    );
    let metadata: serde_json::Value =
        serde_json::from_str(row.try_get::<String, _>("metadata").unwrap().as_str()).unwrap();
    assert_eq!(metadata["image_ids"], serde_json::json!(&first_images));

    let (_, remaining) = send_authed(
        app.clone(),
        Method::GET,
        "/api/v1/admin/listings/image-review-queue",
        &admin_cookie,
        None,
        None,
    )
    .await;
    assert_eq!(remaining["data"].as_array().unwrap().len(), 1);
    assert_eq!(
        remaining["data"][0]["public_id"],
        serde_json::json!(&second)
    );
    let (_, feed) = send_authed(
        app.clone(),
        Method::GET,
        "/api/v1/listings",
        &admin_cookie,
        None,
        None,
    )
    .await;
    let feed_ids: Vec<&str> = feed["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["public_id"].as_str().unwrap())
        .collect();
    assert!(feed_ids.contains(&first.as_str()));
    assert!(!feed_ids.contains(&second.as_str()));
    assert!(feed_ids.contains(&zero.as_str()));

    let (repeat, _) = send_authed(
        app,
        Method::POST,
        &format!("/api/v1/admin/listings/{first}/images/review"),
        &admin_cookie,
        Some(&admin_csrf),
        Some(serde_json::json!({"image_ids": &first_images})),
    )
    .await;
    assert_eq!(repeat, StatusCode::BAD_REQUEST);
    assert_eq!(audit_count(&pool).await, 1);
}

#[tokio::test]
async fn reject_needs_reason_and_hides_from_public() {
    let _lock = ADMIN_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = admin_test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (admin_cookie, admin_csrf) = register_admin(&app, &pool, &cfg, "+989000000000").await;
    let public_id = submit_listing(&app, &seller, &csrf, "فرش").await;

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
    assert_eq!(
        mine["data"][0]["rejection_reason"],
        serde_json::json!("عکس نامناسب است")
    );

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
    let cfg = admin_test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (seller, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (admin_cookie, admin_csrf) = register_admin(&app, &pool, &cfg, "+989000000000").await;
    let public_id = submit_listing(&app, &seller, &csrf, "فرش").await;
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
