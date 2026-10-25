//! Phase 3 Tasks 2–4 (docs/PHASE3_PLAN.md): reference reads, seller listing
//! flow, and public feed/detail. Task 2 cases first (RED: routes missing).

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::Row;
use tower::ServiceExt;

/// Send one request with an optional JSON body; return status + parsed JSON.
async fn send(
    app: axum::Router,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(path);
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

/// Reference reads need the seeded rows; never drop here (read-only — the
/// binaries that reset share this database, same exposure as health.rs).
async fn reference_app() -> Option<axum::Router> {
    let pool = common::test_pool().await?;
    shabdar_api::db::run_migrations(&pool)
        .await
        .expect("migrations apply");
    Some(common::spawn_app(common::test_config()).await)
}

#[tokio::test]
async fn categories_lists_the_seven_launch_categories() {
    let Some(app) = reference_app().await else {
        return; // skipped
    };
    let (status, body) = send(app, Method::GET, "/api/v1/categories", None).await;
    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().expect("data is an array");
    assert_eq!(data.len(), 7, "the 7 launch categories: {body}");
    assert_eq!(data[0]["name_fa"], "شیرینی خانگی");
    assert_eq!(data[0]["slug"], "home-sweets");
}

#[tokio::test]
async fn provinces_lists_gilan() {
    let Some(app) = reference_app().await else {
        return; // skipped
    };
    let (status, body) = send(app, Method::GET, "/api/v1/locations/provinces", None).await;
    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().expect("data is an array");
    assert_eq!(data.len(), 1, "exactly Gilan: {body}");
    assert_eq!(data[0]["slug"], "gilan");
}

#[tokio::test]
async fn cities_of_gilan_contains_launch_city_rasht() {
    let Some(app) = reference_app().await else {
        return; // skipped
    };
    let (status, body) = send(
        app,
        Method::GET,
        "/api/v1/locations/provinces/gilan/cities",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().expect("data is an array");
    assert_eq!(data.len(), 1, "exactly Rasht: {body}");
    assert_eq!(data[0]["slug"], "rasht");
    assert_eq!(data[0]["is_launch_city"], true);
    // The post form binds real ids (no array-index fiction).
    assert_eq!(data[0]["id"], 1);
}

#[tokio::test]
async fn cities_of_unknown_province_is_not_found() {
    let Some(app) = reference_app().await else {
        return; // skipped
    };
    let (status, body) = send(
        app,
        Method::GET,
        "/api/v1/locations/provinces/tehran/cities",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

// --- Task 3: seller listing write + state machine ---------------------------

/// All listing-write tests share one database; run them one at a time.
static WRITE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Drop everything the migrations own (same child-first order as
/// tests/db_migrations.rs) and re-apply, so each test starts clean.
async fn reset_db(pool: &sqlx::MySqlPool) {
    for table in [
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

/// Build a challenge with a KNOWN code via the service (same construction as
/// tests/session_auth.rs).
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
    assert!(!session.is_empty(), "session Set-Cookie");
    assert!(!csrf.is_empty(), "csrf Set-Cookie");
    (format!("{session}; shabdar_csrf={csrf}"), csrf)
}

/// Send a JSON request with optional auth (cookie + csrf token for mutations).
async fn send_authed(
    app: axum::Router,
    method: Method,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(c) = cookie {
        builder = builder.header("cookie", c);
    }
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

fn valid_draft() -> serde_json::Value {
    serde_json::json!({
        "category_id": 1,
        "city_id": 1,
        "title": "فرش دستباف",
        "description": "فرش دستباف در حد نو",
        "price_toman": 5_000_000,
        "price_type": "fixed",
    })
}

#[tokio::test]
async fn create_draft_returns_draft_with_ids() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let (status, body) = send_authed(
        app,
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(valid_draft()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "draft");
    let public_id = body["public_id"].as_str().expect("public_id");
    assert_eq!(public_id.len(), 26, "public_id is a 26-char ULID");
    assert!(
        !body["slug"].as_str().expect("slug").is_empty(),
        "slug generated"
    );
}

#[tokio::test]
async fn create_draft_validation_pins_field_reasons() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;

    // Empty title → required.
    let mut bad = valid_draft();
    bad["title"] = serde_json::json!("");
    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(bad),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
    assert_eq!(body["error"]["fields"]["title"], "required");

    // 161-char title → too_long.
    let mut bad = valid_draft();
    bad["title"] = serde_json::json!("x".repeat(161));
    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(bad),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["title"], "too_long");

    // Unknown category → unknown_category.
    let mut bad = valid_draft();
    bad["category_id"] = serde_json::json!(999);
    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(bad),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["category_id"], "unknown_category");

    // fixed without price → invalid_price.
    let mut bad = valid_draft();
    bad["price_toman"] = serde_json::Value::Null;
    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(bad),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["price_toman"], "invalid_price");

    // free with a price → invalid_price.
    let mut bad = valid_draft();
    bad["price_type"] = serde_json::json!("free");
    let (status, body) = send_authed(
        app,
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(bad),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["price_toman"], "invalid_price");
}

#[tokio::test]
async fn edit_by_non_owner_is_forbidden() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie_a, csrf_a) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (cookie_b, csrf_b) = register_user(&app, &pool, &cfg, "+989009876543").await;

    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie_a),
        Some(&csrf_a),
        Some(valid_draft()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let public_id = body["public_id"].as_str().unwrap().to_string();

    let (status, body) = send_authed(
        app,
        Method::PATCH,
        &format!("/api/v1/listings/{public_id}"),
        Some(&cookie_b),
        Some(&csrf_b),
        Some(serde_json::json!({ "title": "hijacked" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");
}

#[tokio::test]
async fn mutation_without_csrf_changes_nothing() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(valid_draft()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let public_id = body["public_id"].as_str().unwrap().to_string();

    // No CSRF header → 403 and the title is unchanged afterwards.
    let (status, body) = send_authed(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/listings/{public_id}"),
        Some(&cookie),
        None,
        Some(serde_json::json!({ "title": "changed" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "csrf_failed");

    let row = sqlx::query("SELECT title FROM listings WHERE public_id = ?")
        .bind(&public_id)
        .fetch_one(&pool)
        .await
        .expect("listing row");
    let title: String = row.try_get("title").unwrap();
    assert_eq!(title, "فرش دستباف");
}

#[tokio::test]
async fn submit_requires_an_image_then_moves_to_pending() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(valid_draft()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let public_id = body["public_id"].as_str().unwrap().to_string();

    // No images → 400.
    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        &format!("/api/v1/listings/{public_id}/submit"),
        Some(&cookie),
        Some(&csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");

    // One image row (direct setup — Task 5 owns the upload endpoint).
    sqlx::query(
        "INSERT INTO listing_images (listing_id, storage_key, sort_order, created_at) \
         SELECT id, 'setup/cover.png', 0, NOW(3) FROM listings WHERE public_id = ?",
    )
    .bind(&public_id)
    .execute(&pool)
    .await
    .expect("image setup row");

    let (status, body) = send_authed(
        app,
        Method::POST,
        &format!("/api/v1/listings/{public_id}/submit"),
        Some(&cookie),
        Some(&csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "pending");
}

#[tokio::test]
async fn dashboard_filters_drafts() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let (status, _) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(valid_draft()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Drafts are the seller's working set: the explicit filter must list them.
    let (status, body) = send_authed(
        app,
        Method::GET,
        "/api/v1/me/listings?status=draft",
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn same_title_by_two_sellers_yields_distinct_slugs() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie_a, csrf_a) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (cookie_b, csrf_b) = register_user(&app, &pool, &cfg, "+989009876543").await;

    let mut draft = valid_draft();
    draft["title"] = serde_json::json!("فرش دستباف");
    let (status, first) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie_a),
        Some(&csrf_a),
        Some(draft.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, second) = send_authed(
        app,
        Method::POST,
        "/api/v1/listings",
        Some(&cookie_b),
        Some(&csrf_b),
        Some(draft),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let slug_a = first["slug"].as_str().unwrap();
    let slug_b = second["slug"].as_str().unwrap();
    assert!(!slug_a.is_empty() && !slug_b.is_empty());
    assert_ne!(slug_a, slug_b, "suffix dedup, never a 500");
}

#[tokio::test]
async fn edit_slug_exhaustion_is_400_not_500() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;

    // Six identical titles take base, -2, -3, -4, -5, -6.
    let mut draft = valid_draft();
    draft["title"] = serde_json::json!("مبل راحتی");
    for _ in 0..6 {
        let (status, _) = send_authed(
            app.clone(),
            Method::POST,
            "/api/v1/listings",
            Some(&cookie),
            Some(&csrf),
            Some(draft.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    // A seventh listing with another title…
    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(valid_draft()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let public_id = body["public_id"].as_str().unwrap().to_string();

    // …renamed onto the exhausted slug must 400, never 500 on UNIQUE.
    let (status, body) = send_authed(
        app,
        Method::PATCH,
        &format!("/api/v1/listings/{public_id}"),
        Some(&cookie),
        Some(&csrf),
        Some(serde_json::json!({ "title": "مبل راحتی" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
}

#[tokio::test]
async fn mark_sold_from_pending_is_rejected() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(valid_draft()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let public_id = body["public_id"].as_str().unwrap().to_string();

    let (status, body) = send_authed(
        app,
        Method::POST,
        &format!("/api/v1/listings/{public_id}/mark-sold"),
        Some(&cookie),
        Some(&csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
}

// --- Task 4: public feed, detail, sitemap -----------------------------------

/// Create a draft through the API, attach one image row + submit through the
/// API, then flip it to `active` directly (the admin approve endpoint arrives
/// in Task 6; the flip is test setup, the reads under test are HTTP).
async fn create_active(
    app: &axum::Router,
    pool: &sqlx::MySqlPool,
    cookie: &str,
    csrf: &str,
    title: &str,
) -> (String, String) {
    create_active_with(
        app,
        pool,
        cookie,
        csrf,
        title,
        "fixed",
        Some(5_000_000),
        None,
    )
    .await
}

/// Sort fixtures need a chosen price and rating counters; the reads under
/// test are HTTP, so the counters are written directly (no ratings domain).
#[allow(clippy::too_many_arguments)]
async fn create_active_with(
    app: &axum::Router,
    pool: &sqlx::MySqlPool,
    cookie: &str,
    csrf: &str,
    title: &str,
    price_type: &str,
    price_toman: Option<i64>,
    rating: Option<(i64, i64)>,
) -> (String, String) {
    let mut draft = valid_draft();
    draft["title"] = serde_json::json!(title);
    draft["price_type"] = serde_json::json!(price_type);
    draft["price_toman"] = serde_json::json!(price_toman);
    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(cookie),
        Some(csrf),
        Some(draft),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let public_id = body["public_id"].as_str().unwrap().to_string();
    let slug = body["slug"].as_str().unwrap().to_string();
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
        Some(cookie),
        Some(csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    sqlx::query("UPDATE listings SET status = 'active', published_at = NOW(3) WHERE public_id = ?")
        .bind(&public_id)
        .execute(pool)
        .await
        .expect("activate");
    if let Some((sum, count)) = rating {
        sqlx::query("UPDATE listings SET rating_sum = ?, rating_count = ? WHERE public_id = ?")
            .bind(sum)
            .bind(count)
            .bind(&public_id)
            .execute(pool)
            .await
            .expect("rating fixture");
    }
    (public_id, slug)
}

#[tokio::test]
async fn feed_shows_only_active_listings() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;

    // Empty feed shape first.
    let (status, body) = send(app.clone(), Method::GET, "/api/v1/listings", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"], serde_json::json!([]));
    assert_eq!(body["page"]["has_more"], false);

    // One active, one draft, one pending: only the active one is public.
    create_active(&app, &pool, &cookie, &csrf, "فرش دستباف").await;
    let (status, _) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(valid_draft()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(app, Method::GET, "/api/v1/listings", None).await;
    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().expect("data array");
    assert_eq!(data.len(), 1, "only the active listing is public: {body}");
    assert_eq!(data[0]["title"], "فرش دستباف");
    assert_eq!(data[0]["rating_average"], 0);
    assert_eq!(data[0]["rating_count"], 0);
}

#[tokio::test]
async fn feed_filters_by_category_and_city() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    create_active(&app, &pool, &cookie, &csrf, "فرش دستباف").await;

    let (status, body) = send(
        app.clone(),
        Method::GET,
        "/api/v1/listings?category=home-sweets",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 1);

    let (status, body) = send(
        app.clone(),
        Method::GET,
        "/api/v1/listings?category=education",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"], serde_json::json!([]));

    let (status, body) = send(
        app.clone(),
        Method::GET,
        "/api/v1/listings?city=rasht",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 1);

    // Unknown filter slugs are a client error, not an empty list.
    let (status, body) = send(app, Method::GET, "/api/v1/listings?category=nope", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
}

#[tokio::test]
async fn feed_keyset_cursor_pages_without_gaps() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    for title in ["یک", "دو", "سه"] {
        create_active(&app, &pool, &cookie, &csrf, title).await;
    }

    let (status, page1) = send(app.clone(), Method::GET, "/api/v1/listings?limit=2", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page1["data"].as_array().unwrap().len(), 2);
    assert_eq!(page1["page"]["has_more"], true);
    let cursor = page1["page"]["next_cursor"]
        .as_str()
        .expect("cursor")
        .to_string();

    let (status, page2) = send(
        app,
        Method::GET,
        &format!("/api/v1/listings?limit=2&cursor={cursor}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page2["data"].as_array().unwrap().len(), 1);
    assert_eq!(page2["page"]["has_more"], false);

    // Three distinct slugs across both pages — no duplicate, no drop.
    let mut slugs: Vec<String> = page1["data"]
        .as_array()
        .unwrap()
        .iter()
        .chain(page2["data"].as_array().unwrap().iter())
        .map(|c| c["slug"].as_str().unwrap().to_string())
        .collect();
    slugs.sort();
    slugs.dedup();
    assert_eq!(slugs.len(), 3, "every active listing appears exactly once");
}

#[tokio::test]
async fn feed_unknown_sort_is_400() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let (status, body) = send(app, Method::GET, "/api/v1/listings?sort=cheapest", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
    assert_eq!(body["error"]["fields"]["sort"], "unknown_sort");
}

/// A cursor is only meaningful for the sort that produced it: replaying it
/// with another sort must be a client error, never a quietly wrong page.
#[tokio::test]
async fn feed_cursor_from_another_sort_is_400() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    for title in ["یک", "دو", "سه"] {
        create_active(&app, &pool, &cookie, &csrf, title).await;
    }

    let (status, page1) = send(app.clone(), Method::GET, "/api/v1/listings?limit=1", None).await;
    assert_eq!(status, StatusCode::OK);
    let cursor = page1["page"]["next_cursor"]
        .as_str()
        .expect("cursor")
        .to_string();

    let (status, body) = send(
        app,
        Method::GET,
        &format!("/api/v1/listings?sort=price_asc&cursor={cursor}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
    assert_eq!(body["error"]["fields"]["cursor"], "invalid_cursor");
}

// --- Phase 4 Task 2: price and popular sorts -------------------------------

/// Four actives: two priced, two NULL-priced (free + negotiable). NULLs must
/// sort last in BOTH price directions.
async fn seed_price_fixtures(app: &axum::Router, pool: &sqlx::MySqlPool, cookie: &str, csrf: &str) {
    create_active_with(
        app,
        pool,
        cookie,
        csrf,
        "ارزان",
        "fixed",
        Some(100_000),
        None,
    )
    .await;
    create_active_with(
        app,
        pool,
        cookie,
        csrf,
        "گران",
        "fixed",
        Some(200_000),
        None,
    )
    .await;
    create_active_with(app, pool, cookie, csrf, "رایگان", "free", None, None).await;
    create_active_with(app, pool, cookie, csrf, "توافقی", "negotiable", None, None).await;
}

fn titles(body: &serde_json::Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("data array")
        .iter()
        .map(|c| c["title"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn feed_price_sorts_put_null_prices_last() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    seed_price_fixtures(&app, &pool, &cookie, &csrf).await;

    let (status, asc) = send(
        app.clone(),
        Method::GET,
        "/api/v1/listings?sort=price_asc",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let asc_titles = titles(&asc);
    assert_eq!(
        &asc_titles[..2],
        &["ارزان", "گران"],
        "cheapest first: {asc}"
    );
    let mut nulls = asc_titles[2..].to_vec();
    nulls.sort();
    assert_eq!(nulls, vec!["توافقی", "رایگان"], "NULL prices last: {asc}");

    let (status, desc) = send(app, Method::GET, "/api/v1/listings?sort=price_desc", None).await;
    assert_eq!(status, StatusCode::OK);
    let desc_titles = titles(&desc);
    assert_eq!(
        &desc_titles[..2],
        &["گران", "ارزان"],
        "dearest first: {desc}"
    );
    let mut nulls = desc_titles[2..].to_vec();
    nulls.sort();
    assert_eq!(nulls, vec!["توافقی", "رایگان"], "NULL prices last: {desc}");
}

#[tokio::test]
async fn feed_price_cursor_walks_all_rows_once() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    seed_price_fixtures(&app, &pool, &cookie, &csrf).await;

    for sort in ["price_asc", "price_desc"] {
        let path = format!("/api/v1/listings?sort={sort}&limit=2");
        let (status, page1) = send(app.clone(), Method::GET, &path, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page1["page"]["has_more"], true, "{sort} page 1");
        let cursor = page1["page"]["next_cursor"]
            .as_str()
            .expect("cursor")
            .to_string();

        let (status, page2) = send(
            app.clone(),
            Method::GET,
            &format!("/api/v1/listings?sort={sort}&limit=2&cursor={cursor}"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page2["page"]["has_more"], false, "{sort} page 2");

        let mut slugs: Vec<String> = page1["data"]
            .as_array()
            .unwrap()
            .iter()
            .chain(page2["data"].as_array().unwrap().iter())
            .map(|c| c["slug"].as_str().unwrap().to_string())
            .collect();
        slugs.sort();
        slugs.dedup();
        assert_eq!(slugs.len(), 4, "{sort}: every row exactly once");
    }
}

#[tokio::test]
async fn feed_popular_prefers_many_good_over_one_perfect() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    // A: 5/1 → 25/6 = 4.166… · B: 48/10 → 68/15 = 4.533… · C: 0/0 → 20/5 = 4.0
    create_active_with(
        &app,
        &pool,
        &cookie,
        &csrf,
        "یک‌ستاره‌کامل",
        "fixed",
        Some(100_000),
        Some((5, 1)),
    )
    .await;
    create_active_with(
        &app,
        &pool,
        &cookie,
        &csrf,
        "خیلی‌خوب",
        "fixed",
        Some(100_000),
        Some((48, 10)),
    )
    .await;
    create_active_with(
        &app,
        &pool,
        &cookie,
        &csrf,
        "بدون‌امتیاز",
        "fixed",
        Some(100_000),
        None,
    )
    .await;

    let (status, body) = send(app, Method::GET, "/api/v1/listings?sort=popular", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        titles(&body),
        vec!["خیلی‌خوب", "یک‌ستاره‌کامل", "بدون‌امتیاز"],
        "Bayesian prior, not raw average: {body}"
    );
}

#[tokio::test]
async fn feed_popular_cursor_walks_all_rows_once() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    create_active_with(
        &app,
        &pool,
        &cookie,
        &csrf,
        "الف",
        "fixed",
        Some(100_000),
        Some((5, 1)),
    )
    .await;
    create_active_with(
        &app,
        &pool,
        &cookie,
        &csrf,
        "ب",
        "fixed",
        Some(100_000),
        Some((48, 10)),
    )
    .await;
    create_active_with(
        &app,
        &pool,
        &cookie,
        &csrf,
        "ج",
        "fixed",
        Some(100_000),
        None,
    )
    .await;

    let (status, page1) = send(
        app.clone(),
        Method::GET,
        "/api/v1/listings?sort=popular&limit=2",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page1["page"]["has_more"], true);
    let cursor = page1["page"]["next_cursor"]
        .as_str()
        .expect("cursor")
        .to_string();

    let (status, page2) = send(
        app,
        Method::GET,
        &format!("/api/v1/listings?sort=popular&limit=2&cursor={cursor}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page2["data"].as_array().unwrap().len(), 1);
    assert_eq!(page2["page"]["has_more"], false);

    let mut slugs: Vec<String> = page1["data"]
        .as_array()
        .unwrap()
        .iter()
        .chain(page2["data"].as_array().unwrap().iter())
        .map(|c| c["slug"].as_str().unwrap().to_string())
        .collect();
    slugs.sort();
    slugs.dedup();
    assert_eq!(slugs.len(), 3, "every row exactly once across pages");
}

// --- Phase 4 Task 3: keyword search ---------------------------------------

/// Search fixtures: one title hit, one description-only hit, one unrelated.
async fn seed_search_fixtures(
    app: &axum::Router,
    pool: &sqlx::MySqlPool,
    cookie: &str,
    csrf: &str,
) {
    // Descriptions are pinned per fixture: the shared draft description would
    // otherwise make every listing match the same words.
    for (title, description) in [
        ("فرش دستباف", "فرش دستباف"),
        ("مبل راحتی", "قالیچه نفیس ایرانی"),
        ("لپ تاپ", "لپ تاپ"),
    ] {
        let (public_id, _) = create_active(app, pool, cookie, csrf, title).await;
        sqlx::query("UPDATE listings SET description = ? WHERE public_id = ?")
            .bind(description)
            .bind(&public_id)
            .execute(pool)
            .await
            .expect("description fixture");
    }
}

#[tokio::test]
async fn feed_search_matches_title_and_description() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    seed_search_fixtures(&app, &pool, &cookie, &csrf).await;

    let (status, body) = send(app.clone(), Method::GET, "/api/v1/listings?q=دستباف", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), vec!["فرش دستباف"], "title hit: {body}");

    let (status, body) = send(app.clone(), Method::GET, "/api/v1/listings?q=قالیچه", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), vec!["مبل راحتی"], "description hit: {body}");

    let (status, body) = send(app, Method::GET, "/api/v1/listings?q=یخچال", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"],
        serde_json::json!([]),
        "no match is an empty page"
    );
}

#[tokio::test]
async fn feed_search_requires_every_term() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    seed_search_fixtures(&app, &pool, &cookie, &csrf).await;

    let (status, body) = send(
        app.clone(),
        Method::GET,
        "/api/v1/listings?q=فرش%20دستباف",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        titles(&body),
        vec!["فرش دستباف"],
        "both terms in the title: {body}"
    );

    let (status, body) = send(app, Method::GET, "/api/v1/listings?q=فرش%20مبل", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"],
        serde_json::json!([]),
        "terms are AND, not OR: {body}"
    );
}

#[tokio::test]
async fn feed_search_folds_arabic_letters_and_zwnj() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    // ZWNJ in the title, Arabic kaf in the other title.
    create_active(&app, &pool, &cookie, &csrf, "می‌خواهم کتاب").await;
    create_active(&app, &pool, &cookie, &csrf, "كتاب قدیمی").await;

    // Query without ZWNJ, Persian yeh: matches the ZWNJ title.
    let (status, body) = send(app.clone(), Method::GET, "/api/v1/listings?q=میخواهم", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), vec!["می‌خواهم کتاب"], "ZWNJ folded: {body}");

    // Persian kaf query: matches both the Persian and the Arabic spelling.
    let (status, body) = send(app, Method::GET, "/api/v1/listings?q=کتاب", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body).len(), 2, "yeh/kaf folded: {body}");
}

#[tokio::test]
async fn feed_search_escapes_like_wildcards() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    create_active(&app, &pool, &cookie, &csrf, "کد 20% تخفیف").await;
    create_active(&app, &pool, &cookie, &csrf, "کد 20x تخفیف").await;

    let (status, body) = send(app.clone(), Method::GET, "/api/v1/listings?q=20%25", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), vec!["کد 20% تخفیف"], "% is literal: {body}");

    // `_` alone would match any single character; escaped it matches nothing.
    let (status, body) = send(app.clone(), Method::GET, "/api/v1/listings?q=20_", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"],
        serde_json::json!([]),
        "no wildcard widening: {body}"
    );
}

#[tokio::test]
async fn feed_search_rejects_overlong_q_and_ignores_blank() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    create_active(&app, &pool, &cookie, &csrf, "فرش دستباف").await;

    let long = "a".repeat(101);
    let (status, body) = send(
        app.clone(),
        Method::GET,
        &format!("/api/v1/listings?q={long}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["q"], "too_long");

    let (status, body) = send(app, Method::GET, "/api/v1/listings?q=%20%20", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        titles(&body),
        vec!["فرش دستباف"],
        "blank q is no search: {body}"
    );
}

// --- Phase 4 Task 4: dashboard summary ------------------------------------

#[tokio::test]
async fn summary_counts_owner_listings_by_status() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;

    // 2 active (one soft-deleted afterwards), 1 sold, 1 pending, 1 draft.
    for title in ["فعال یک", "فعال دو", "فروخته", "در انتظار"] {
        create_active(&app, &pool, &cookie, &csrf, title).await;
    }
    let (status, _) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(valid_draft()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    for (title, status_value) in [("فروخته", "sold"), ("در انتظار", "pending")] {
        sqlx::query("UPDATE listings SET status = ? WHERE title = ?")
            .bind(status_value)
            .bind(title)
            .execute(&pool)
            .await
            .expect("status fixture");
    }
    sqlx::query("UPDATE listings SET deleted_at = NOW(3) WHERE title = 'فعال دو'")
        .execute(&pool)
        .await
        .expect("soft delete fixture");

    let (status, body) = send_authed(
        app,
        Method::GET,
        "/api/v1/me/dashboard/summary",
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["active_count"], 1);
    assert_eq!(body["sold_count"], 1);
    assert_eq!(body["pending_count"], 1);
    assert_eq!(body["favorite_count"], 0);
}

#[tokio::test]
async fn summary_is_zeroes_for_a_new_seller() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, _csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let (status, body) = send_authed(
        app,
        Method::GET,
        "/api/v1/me/dashboard/summary",
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        serde_json::json!({
            "active_count": 0, "sold_count": 0, "pending_count": 0, "favorite_count": 0
        })
    );
}

#[tokio::test]
async fn summary_requires_a_session() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let (status, body) = send(app, Method::GET, "/api/v1/me/dashboard/summary", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");
}

#[tokio::test]
async fn detail_visibility_rules() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (active_id, active_slug) = create_active(&app, &pool, &cookie, &csrf, "فرش دستباف").await;

    // Active → 200 with the Persian title.
    let (status, body) = send(
        app.clone(),
        Method::GET,
        &format!("/api/v1/listings/{active_slug}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["title"], "فرش دستباف");

    // Removed → 410 gone (no existence detail beyond the code).
    sqlx::query("UPDATE listings SET status = 'removed' WHERE public_id = ?")
        .bind(&active_id)
        .execute(&pool)
        .await
        .expect("remove");
    let (status, body) = send(
        app.clone(),
        Method::GET,
        &format!("/api/v1/listings/{active_slug}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(body["error"]["code"], "gone");

    // Draft → 404 for strangers (no existence oracle).
    let (status, body) = send_authed(
        app.clone(),
        Method::POST,
        "/api/v1/listings",
        Some(&cookie),
        Some(&csrf),
        Some(valid_draft()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let draft_slug = body["slug"].as_str().unwrap().to_string();
    let (status, body) = send(
        app,
        Method::GET,
        &format!("/api/v1/listings/{draft_slug}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn sitemap_lists_only_active_slugs() {
    let _lock = WRITE_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (_, slug) = create_active(&app, &pool, &cookie, &csrf, "فرش دستباف").await;

    let (status, body) = send(app, Method::GET, "/api/v1/sitemap/listings", None).await;
    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().expect("data array");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["url"], format!("/listings/{slug}"));
    assert!(data[0]["updated_at"].is_string(), "updated_at present");
}
