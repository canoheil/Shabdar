//! Work Package 1 (docs/PHASE2_PLAN.md): the `CurrentUser` session extractor,
//! GET /api/v1/me, PATCH /api/v1/me, and POST /api/v1/auth/logout. DB tests
//! need TEST_DATABASE_URL=mysql://…/shabdar_test; without it they print a
//! note and skip. Tests never read DATABASE_URL, and the pool helper refuses
//! anything that is not a *_test database.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// All auth tests share one database; run them one at a time.
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Drop everything the migrations own (FK-safe order) and re-apply, so every
/// test starts from a clean schema (same helper as tests/auth_otp.rs).
async fn reset_db(pool: &sqlx::MySqlPool) {
    for table in ["sessions", "otp_challenges", "users", "_sqlx_migrations"] {
        let sql = format!("DROP TABLE IF EXISTS {table}"); // fixed literals, not user input
        sqlx::query(&sql).execute(pool).await.expect("drop failed");
    }
    shabdar_api::db::run_migrations(pool)
        .await
        .expect("migrations apply");
}

/// POST a JSON body; returns the raw response (headers + body).
async fn post_json(
    app: axum::Router,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    app.oneshot(req).await.unwrap()
}

/// GET a path, optionally with a raw `Cookie:` header value.
async fn get_with_cookie(
    app: axum::Router,
    path: &str,
    cookie: Option<&str>,
) -> axum::response::Response {
    let mut builder = Request::builder().method("GET").uri(path);
    if let Some(value) = cookie {
        builder = builder.header("cookie", value);
    }
    let req = builder.body(Body::empty()).unwrap();
    app.oneshot(req).await.unwrap()
}

/// Collect a response body into parsed JSON.
async fn json_body(resp: axum::response::Response) -> serde_json::Value {
    let bytes = BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    serde_json::from_slice(&bytes).expect("JSON body")
}

fn error_code(body: &serde_json::Value) -> &str {
    body["error"]["code"].as_str().expect("error.code")
}

/// Build a challenge with a KNOWN code via the service: the request endpoint
/// generates its own code internally, so tests create challenges directly
/// (same construction as tests/auth_otp.rs).
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

fn verify_body(challenge_id: &str, code: &str) -> serde_json::Value {
    serde_json::json!({ "challenge_id": challenge_id, "code": code })
}

/// Register a user through the real OTP flow — a service-created challenge,
/// then the HTTP verify endpoint — and return the session cookie header value
/// (`shabdar_session=<token>`) plus the verify response's `user` object.
async fn register_user(
    app: &axum::Router,
    pool: &sqlx::MySqlPool,
    cfg: &shabdar_api::config::Config,
    phone: &str,
) -> (String, serde_json::Value) {
    let (challenge_id, code) = challenge_with_code(pool, cfg, phone, "register").await;
    let resp = post_json(
        app.clone(),
        "/api/v1/auth/otp/verify",
        verify_body(&challenge_id, &code),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    // Owned copies first: json_body(resp) below moves the response, so the
    // header borrow must not outlive it.
    let set_cookie = resp
        .headers()
        .get(axum::http::header::SET_COOKIE)
        .expect("Set-Cookie header")
        .to_str()
        .expect("ascii cookie")
        .to_string();
    let token = set_cookie
        .strip_prefix("shabdar_session=")
        .and_then(|rest| rest.split(';').next())
        .expect("token in cookie")
        .to_string();
    let body = json_body(resp).await;
    (format!("shabdar_session={token}"), body["user"].clone())
}

// --- Task 1: CurrentUser extractor + GET /api/v1/me ------------------------

#[tokio::test]
async fn me_with_valid_cookie_returns_the_exact_user_shape() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return; // skipped: TEST_DATABASE_URL unset or not a *_test database
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, user) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let public_id = user["public_id"].as_str().expect("public_id").to_string();

    let resp = get_with_cookie(app, "/api/v1/me", Some(&cookie)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    // Body exactly {"user":{"public_id","display_name","is_admin"}} — no
    // extra keys — with display_name still null on a fresh account.
    assert_eq!(
        body,
        serde_json::json!({
            "user": {
                "public_id": public_id,
                "display_name": null,
                "is_admin": false,
            }
        })
    );
}

#[tokio::test]
async fn me_without_cookie_is_401_unauthorized() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let resp = get_with_cookie(app, "/api/v1/me", None).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "unauthorized");
}

#[tokio::test]
async fn me_with_garbage_cookie_is_401_unauthorized() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let resp = get_with_cookie(app, "/api/v1/me", Some("shabdar_session=not-a-real-token")).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "unauthorized");
}

#[tokio::test]
async fn me_with_expired_session_is_401_unauthorized() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    // Force expiry (boundary: expired at or before expires_at).
    sqlx::query("UPDATE sessions SET expires_at = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1))
        .execute(&pool)
        .await
        .expect("force expiry");

    let resp = get_with_cookie(app, "/api/v1/me", Some(&cookie)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "unauthorized");
}

#[tokio::test]
async fn me_for_suspended_user_is_401_unauthorized() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    sqlx::query("UPDATE users SET status = 'suspended'")
        .execute(&pool)
        .await
        .expect("suspend user");

    let resp = get_with_cookie(app, "/api/v1/me", Some(&cookie)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "unauthorized");
}
