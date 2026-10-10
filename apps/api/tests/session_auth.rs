//! Work Package 1 + 3 (docs/PHASE2_PLAN.md): the `CurrentUser` session
//! extractor, GET /api/v1/me, PATCH /api/v1/me, POST /api/v1/auth/logout, and
//! the double-submit CSRF checks. DB tests need
//! TEST_DATABASE_URL=mysql://…/shabdar_test; without it they print a note
//! and skip. Tests never read DATABASE_URL, and the pool helper refuses
//! anything that is not a *_test database.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::Row;
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

/// PATCH a JSON body with optional `Cookie:` and `X-CSRF-Token:` header values.
async fn patch_json(
    app: axum::Router,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: serde_json::Value,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("PATCH")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(value) = cookie {
        builder = builder.header("cookie", value);
    }
    if let Some(token) = csrf {
        builder = builder.header("x-csrf-token", token);
    }
    let req = builder.body(Body::from(body.to_string())).unwrap();
    app.oneshot(req).await.unwrap()
}

/// POST with an empty body and optional `Cookie:` / `X-CSRF-Token:` values.
async fn post_with_cookie(
    app: axum::Router,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
) -> axum::response::Response {
    let mut builder = Request::builder().method("POST").uri(path);
    if let Some(value) = cookie {
        builder = builder.header("cookie", value);
    }
    if let Some(token) = csrf {
        builder = builder.header("x-csrf-token", token);
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

/// The Crockford base32 alphabet of a ULID (^[0-9A-HJKMNP-TV-Z]{26}$).
fn is_crockford_ulid(s: &str) -> bool {
    s.len() == 26
        && s.bytes().all(|b| {
            matches!(b,
                b'0'..=b'9' | b'A'..=b'H' | b'J'..=b'K' | b'M'..=b'N' | b'P'..=b'T' | b'V'..=b'Z')
        })
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
/// then the HTTP verify endpoint — and return:
/// (1) the full Cookie header value (`shabdar_session=…; shabdar_csrf=…`),
/// (2) the raw `shabdar_csrf` token (echoed in X-CSRF-Token on mutations),
/// (3) the verify response's `user` object.
async fn register_user(
    app: &axum::Router,
    pool: &sqlx::MySqlPool,
    cfg: &shabdar_api::config::Config,
    phone: &str,
) -> (String, String, serde_json::Value) {
    let (challenge_id, code) = challenge_with_code(pool, cfg, phone, "register").await;
    let resp = post_json(
        app.clone(),
        "/api/v1/auth/otp/verify",
        verify_body(&challenge_id, &code),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    // Owned copies first: json_body(resp) below moves the response, so the
    // header borrows must not outlive it.
    let mut session_pair = String::new();
    let mut csrf_pair = String::new();
    let mut csrf_token = String::new();
    for value in resp.headers().get_all(axum::http::header::SET_COOKIE) {
        let text = value.to_str().expect("ascii cookie");
        if let Some(rest) = text.strip_prefix("shabdar_session=") {
            let token = rest.split(';').next().expect("token in cookie");
            session_pair = format!("shabdar_session={token}");
        } else if let Some(rest) = text.strip_prefix("shabdar_csrf=") {
            let token = rest.split(';').next().expect("token in cookie");
            csrf_token = token.to_string();
            csrf_pair = format!("shabdar_csrf={token}");
        }
    }
    assert!(!session_pair.is_empty(), "session Set-Cookie");
    let body = json_body(resp).await;
    (
        format!("{session_pair}; {csrf_pair}"),
        csrf_token,
        body["user"].clone(),
    )
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
    let (cookie, _csrf, user) = register_user(&app, &pool, &cfg, "+989123456789").await;
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
    let (cookie, _csrf, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

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
    let (cookie, _csrf, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    sqlx::query("UPDATE users SET status = 'suspended'")
        .execute(&pool)
        .await
        .expect("suspend user");

    let resp = get_with_cookie(app, "/api/v1/me", Some(&cookie)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "unauthorized");
}

// --- Task 2: PATCH /api/v1/me (display_name) -------------------------------

#[tokio::test]
async fn patch_me_sets_a_persian_display_name() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf, user) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let public_id = user["public_id"].as_str().expect("public_id").to_string();
    // A valid Persian name (10 chars).
    let name = "آزمایش نام";

    let resp = patch_json(
        app.clone(),
        "/api/v1/me",
        Some(&cookie),
        Some(&csrf),
        serde_json::json!({ "display_name": name }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(
        body,
        serde_json::json!({
            "user": {
                "public_id": public_id,
                "display_name": name,
                "is_admin": false,
            }
        })
    );

    // GET /me reflects the new name.
    let resp = get_with_cookie(app, "/api/v1/me", Some(&cookie)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["user"]["display_name"], name);
}

#[tokio::test]
async fn patch_me_missing_display_name_is_400_required() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let resp = patch_json(app, "/api/v1/me", Some(&cookie), Some(&csrf), serde_json::json!({})).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = json_body(resp).await;
    assert_eq!(error_code(&body), "validation_error");
    assert_eq!(body["error"]["fields"]["display_name"], "required");
}

#[tokio::test]
async fn patch_me_blank_display_name_is_400_required() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    for blank in ["", "   ", "\t\n "] {
        let resp = patch_json(
            app.clone(),
            "/api/v1/me",
            Some(&cookie),
            Some(&csrf),
            serde_json::json!({ "display_name": blank }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "blank {blank:?}");
        let body = json_body(resp).await;
        assert_eq!(error_code(&body), "validation_error");
        assert_eq!(body["error"]["fields"]["display_name"], "required");
    }
}

#[tokio::test]
async fn patch_me_81_multibyte_chars_is_400_too_long() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    // 81 Persian digits (multi-byte): the limit is chars, not bytes.
    let name81 = "۱".repeat(81);
    let resp = patch_json(
        app,
        "/api/v1/me",
        Some(&cookie),
        Some(&csrf),
        serde_json::json!({ "display_name": &name81 }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = json_body(resp).await;
    assert_eq!(error_code(&body), "validation_error");
    assert_eq!(body["error"]["fields"]["display_name"], "too_long");
}

#[tokio::test]
async fn patch_me_exactly_80_multibyte_chars_is_200() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf, user) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let public_id = user["public_id"].as_str().expect("public_id").to_string();

    // Exactly 80 Persian digits (multi-byte) must pass the boundary.
    let name80 = "۱".repeat(80);
    let resp = patch_json(
        app,
        "/api/v1/me",
        Some(&cookie),
        Some(&csrf),
        serde_json::json!({ "display_name": &name80 }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(
        body,
        serde_json::json!({
            "user": {
                "public_id": public_id,
                "display_name": name80,
                "is_admin": false,
            }
        })
    );
}

#[tokio::test]
async fn patch_me_without_cookie_is_401_unauthorized() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let resp = patch_json(
        app,
        "/api/v1/me",
        None,
        None,
        serde_json::json!({ "display_name": "کاربر" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "unauthorized");
}

// --- Task 3: POST /api/v1/auth/logout --------------------------------------

#[tokio::test]
async fn logout_revokes_the_session_and_clears_both_cookies() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, csrf, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let resp = post_with_cookie(app.clone(), "/api/v1/auth/logout", Some(&cookie), Some(&csrf)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    // Owned copies first: json_body(resp) below moves the response.
    let mut clears: Vec<String> = resp
        .headers()
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().expect("ascii cookie").to_string())
        .collect();
    clears.sort();
    // Both cookies cleared with matching flags and Max-Age=0 (Review Focus 4).
    assert_eq!(
        clears,
        [
            "shabdar_csrf=; Path=/; SameSite=Lax; Max-Age=0",
            "shabdar_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        ]
    );
    let body = json_body(resp).await;
    assert_eq!(body, serde_json::json!({ "ok": true }));

    // The sessions row is gone.
    let row = sqlx::query("SELECT COUNT(*) AS c FROM sessions")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(row.try_get::<i64, _>("c").expect("count"), 0);

    // Replaying the same cookie afterwards → 401.
    let resp = get_with_cookie(app, "/api/v1/me", Some(&cookie)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "unauthorized");
}

#[tokio::test]
async fn logout_revokes_only_the_presented_session() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let mut cfg = common::test_config();
    // Two sessions for one user: disable the resend cooldown.
    cfg.otp_resend_secs = 0;
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie_a, csrf_a, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;
    let (cookie_b, _csrf_b, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let resp = post_with_cookie(
        app.clone(),
        "/api/v1/auth/logout",
        Some(&cookie_a),
        Some(&csrf_a),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Exactly one session revoked (approved decision 5).
    let row = sqlx::query("SELECT COUNT(*) AS c FROM sessions")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(row.try_get::<i64, _>("c").expect("count"), 1);

    // Session B still works; session A is dead.
    let resp = get_with_cookie(app.clone(), "/api/v1/me", Some(&cookie_b)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = get_with_cookie(app, "/api/v1/me", Some(&cookie_a)).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "unauthorized");
}

#[tokio::test]
async fn logout_without_cookie_is_401_unauthorized() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let resp = post_with_cookie(app, "/api/v1/auth/logout", None, None).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "unauthorized");
}

// --- Task 8: double-submit CSRF (shabdar_csrf + csrf_failed) ---------------

#[tokio::test]
async fn verify_sets_session_and_csrf_cookies() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (challenge_id, code) = challenge_with_code(&pool, &cfg, "+989123456789", "register").await;

    let resp = post_json(app, "/api/v1/auth/otp/verify", verify_body(&challenge_id, &code)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    // Owned copies first: json_body(resp) below moves the response.
    let mut cookies: Vec<String> = resp
        .headers()
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().expect("ascii cookie").to_string())
        .collect();
    cookies.sort();
    assert_eq!(cookies.len(), 2, "session + csrf Set-Cookie: {cookies:?}");

    // Session cookie: the approved Phase 1 flags.
    let session = cookies
        .iter()
        .find(|c| c.starts_with("shabdar_session="))
        .expect("session cookie");
    let token = session
        .strip_prefix("shabdar_session=")
        .and_then(|rest| rest.split(';').next())
        .expect("token");
    assert_eq!(token.len(), 64);
    assert_eq!(
        session,
        &format!("shabdar_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000")
    );

    // CSRF cookie: NOT HttpOnly, 26-char ULID token (approved CSRF proposal).
    let csrf = cookies
        .iter()
        .find(|c| c.starts_with("shabdar_csrf="))
        .expect("csrf cookie");
    let csrf_token = csrf
        .strip_prefix("shabdar_csrf=")
        .and_then(|rest| rest.split(';').next())
        .expect("csrf token");
    assert!(is_crockford_ulid(csrf_token), "csrf token must be a ULID: {csrf_token}");
    assert_eq!(csrf, &format!("shabdar_csrf={csrf_token}; Path=/; SameSite=Lax"));

    let body = json_body(resp).await;
    assert_eq!(body["is_new_user"], true);
}

#[tokio::test]
async fn patch_me_without_csrf_header_is_403_and_does_not_mutate() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, _csrf, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let resp = patch_json(
        app.clone(),
        "/api/v1/me",
        Some(&cookie),
        None,
        serde_json::json!({ "display_name": "دزد" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(&json_body(resp).await), "csrf_failed");

    // No state change: the display name is untouched.
    let resp = get_with_cookie(app, "/api/v1/me", Some(&cookie)).await;
    let body = json_body(resp).await;
    assert!(body["user"]["display_name"].is_null(), "mutation must not apply");
}

#[tokio::test]
async fn patch_me_with_wrong_csrf_header_is_403() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, _csrf, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let resp = patch_json(
        app,
        "/api/v1/me",
        Some(&cookie),
        Some("wrong-token"),
        serde_json::json!({ "display_name": "دزد" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(&json_body(resp).await), "csrf_failed");
}

#[tokio::test]
async fn logout_without_csrf_header_is_403_and_the_session_still_works() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, _csrf, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    let resp = post_with_cookie(app.clone(), "/api/v1/auth/logout", Some(&cookie), None).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(&json_body(resp).await), "csrf_failed");

    // A failed CSRF check must not revoke anything: the session still works.
    let resp = get_with_cookie(app, "/api/v1/me", Some(&cookie)).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn csrf_empty_or_blank_tokens_are_403_and_do_not_mutate() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (cookie, _csrf, _user) = register_user(&app, &pool, &cfg, "+989123456789").await;

    // Empty X-CSRF-Token with otherwise-valid cookies.
    let resp = patch_json(
        app.clone(),
        "/api/v1/me",
        Some(&cookie),
        Some(""),
        serde_json::json!({ "display_name": "دزد" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(&json_body(resp).await), "csrf_failed");

    // The exact bypass shape: empty shabdar_csrf cookie + empty header.
    let session_pair = cookie.split(';').next().expect("session pair first").trim();
    let empty_cookie = format!("{session_pair}; shabdar_csrf=");
    let resp = patch_json(
        app.clone(),
        "/api/v1/me",
        Some(&empty_cookie),
        Some(""),
        serde_json::json!({ "display_name": "دزد" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(&json_body(resp).await), "csrf_failed");

    // No state change: the display name is untouched.
    let resp = get_with_cookie(app, "/api/v1/me", Some(&cookie)).await;
    let body = json_body(resp).await;
    assert!(body["user"]["display_name"].is_null(), "mutation must not apply");
}
