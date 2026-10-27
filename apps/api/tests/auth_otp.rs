//! Tasks 8–9 (docs/PHASE1_PLAN.md): POST /api/v1/auth/otp/request and
//! POST /api/v1/auth/otp/verify. DB tests need
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
/// test starts from a clean schema. `DROP TABLE IF EXISTS` is a no-op for a
/// missing table, so this works whether or not migration 0002 exists yet.
/// Phase 3 tables drop first: listings/images/audit reference users, cities
/// reference provinces (dropping users first fails with error 1451/3730).
async fn reset_db(pool: &sqlx::MySqlPool) {
    for table in [
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

/// Collect a response body into parsed JSON.
async fn json_body(resp: axum::response::Response) -> serde_json::Value {
    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
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

// --- Task 8: POST /api/v1/auth/otp/request --------------------------------

#[tokio::test]
async fn request_valid_returns_the_exact_response_shape() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return; // skipped: TEST_DATABASE_URL unset or not a *_test database
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let resp = post_json(
        app,
        "/api/v1/auth/otp/request",
        serde_json::json!({ "phone": "+989123456789", "purpose": "register" }),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    let mut keys: Vec<String> = body
        .as_object()
        .expect("object body")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        ["challenge_id", "expires_in_seconds", "resend_after_seconds"]
    );
    let challenge_id = body["challenge_id"].as_str().expect("challenge_id");
    assert!(
        is_crockford_ulid(challenge_id),
        "not a ULID: {challenge_id}"
    );
    assert_eq!(body["expires_in_seconds"], 120);
    assert_eq!(body["resend_after_seconds"], 60);
}

#[tokio::test]
async fn request_invalid_phone_is_400_invalid_phone() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let resp = post_json(
        app,
        "/api/v1/auth/otp/request",
        serde_json::json!({ "phone": "02112345678", "purpose": "register" }),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&json_body(resp).await), "invalid_phone");
}

#[tokio::test]
async fn request_unknown_purpose_is_400_invalid_purpose() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let resp = post_json(
        app,
        "/api/v1/auth/otp/request",
        serde_json::json!({ "phone": "+989123456789", "purpose": "reset" }),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&json_body(resp).await), "invalid_purpose");
}

#[tokio::test]
async fn request_missing_fields_is_400_validation_error() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let resp = post_json(app, "/api/v1/auth/otp/request", serde_json::json!({})).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = json_body(resp).await;
    assert_eq!(error_code(&body), "validation_error");
    assert_eq!(body["error"]["fields"]["phone"], "required");
    assert_eq!(body["error"]["fields"]["purpose"], "required");
}

#[tokio::test]
async fn request_cooldown_blocks_repeat_but_not_other_purpose() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let first = post_json(
        app.clone(),
        "/api/v1/auth/otp/request",
        serde_json::json!({ "phone": "+989123456789", "purpose": "register" }),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let repeat = post_json(
        app.clone(),
        "/api/v1/auth/otp/request",
        serde_json::json!({ "phone": "+989123456789", "purpose": "register" }),
    )
    .await;
    assert_eq!(repeat.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(error_code(&json_body(repeat).await), "rate_limited");

    let other = post_json(
        app,
        "/api/v1/auth/otp/request",
        serde_json::json!({ "phone": "+989123456789", "purpose": "login" }),
    )
    .await;
    assert_eq!(other.status(), StatusCode::OK);
}

#[tokio::test]
async fn service_stores_the_hmac_of_the_code_never_the_plaintext() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();

    // The HTTP endpoint generates its own code internally, so the DB-level
    // assertion drives the service directly with a known plaintext code
    // (plan: "tests call generate_code() themselves and pass the plaintext
    // in — real code paths, no mocks").
    let code = shabdar_api::otp::generate_code();
    let created =
        shabdar_api::otp_service::create_challenge(&pool, &cfg, "+989123456789", "register", &code)
            .await
            .expect("create challenge");

    let row = sqlx::query("SELECT code_hash FROM otp_challenges WHERE public_id = ?")
        .bind(&created.public_id)
        .fetch_one(&pool)
        .await
        .expect("challenge row");
    let stored: Vec<u8> = row.try_get("code_hash").expect("code_hash bytes");
    assert_ne!(
        stored.as_slice(),
        code.as_bytes(),
        "the plaintext code must never be stored"
    );
    let expected = shabdar_api::otp::hash_code(&code, &cfg.otp_pepper);
    assert_eq!(stored.as_slice(), expected.as_slice());
}

// --- Task 9: POST /api/v1/auth/otp/verify ---------------------------------

/// Build a challenge with a KNOWN code via the service: the request endpoint
/// generates its own code internally, so verify tests create challenges
/// directly (plan: "tests call generate_code() themselves and pass the
/// plaintext in — real code paths, no mocks").
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

/// SHA-256 of bytes, computed independently of the session module so the
/// token-hash assertion does not depend on the code it checks.
fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// A wrong 6-digit code, guaranteed different from `code`.
fn wrong_code(code: &str) -> String {
    if let Some(rest) = code.strip_prefix('0') {
        format!("1{rest}")
    } else {
        format!("0{}", &code[1..])
    }
}

#[tokio::test]
async fn verify_register_happy_path_creates_user_and_session() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (challenge_id, code) = challenge_with_code(&pool, &cfg, "+989123456789", "register").await;

    let resp = post_json(
        app,
        "/api/v1/auth/otp/verify",
        verify_body(&challenge_id, &code),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Set-Cookie: the exact approved flags; the token is 64 hex chars.
    // Owned copies: json_body(resp) below moves the response, so the
    // header borrow must not outlive it.
    let cookie = resp
        .headers()
        .get(axum::http::header::SET_COOKIE)
        .expect("Set-Cookie header")
        .to_str()
        .expect("ascii cookie")
        .to_string();
    let token = cookie
        .strip_prefix("shabdar_session=")
        .and_then(|rest| rest.split(';').next())
        .expect("token in cookie")
        .to_string();
    assert_eq!(token.len(), 64, "session token is 64 hex chars");
    assert_eq!(
        cookie,
        format!("shabdar_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000")
    );

    let body = json_body(resp).await;
    assert_eq!(body["is_new_user"], true);
    let user = &body["user"];
    assert!(is_crockford_ulid(
        user["public_id"].as_str().expect("public_id")
    ));
    assert!(user["display_name"].is_null());
    assert_eq!(user["is_admin"], false);

    // users row: normalized phone, active, no display name.
    let row =
        sqlx::query("SELECT id, phone_e164, display_name, status FROM users WHERE phone_e164 = ?")
            .bind("+989123456789")
            .fetch_one(&pool)
            .await
            .expect("user row");
    let user_id: u64 = row.try_get("id").expect("id");
    assert_eq!(
        row.try_get::<String, _>("phone_e164").expect("phone"),
        "+989123456789"
    );
    assert!(row
        .try_get::<Option<String>, _>("display_name")
        .expect("display_name")
        .is_none());
    assert_eq!(
        row.try_get::<String, _>("status").expect("status"),
        "active"
    );

    // sessions row: only the token's hash, ~30-day expiry, linked to the user.
    let row = sqlx::query("SELECT token_hash, expires_at, user_id FROM sessions")
        .fetch_one(&pool)
        .await
        .expect("session row");
    let stored_hash: Vec<u8> = row.try_get("token_hash").expect("token_hash");
    assert_eq!(stored_hash.as_slice(), sha256(token.as_bytes()).as_slice());
    assert_ne!(stored_hash.as_slice(), token.as_bytes());
    assert_eq!(row.try_get::<u64, _>("user_id").expect("user_id"), user_id);
    let expires_at: chrono::DateTime<chrono::Utc> = row.try_get("expires_at").expect("expires_at");
    let expected = chrono::Utc::now() + chrono::Duration::seconds(cfg.session_ttl_secs as i64);
    assert!(
        (expires_at - expected).num_seconds().abs() < 60,
        "expires_at must be ≈ now + 30 days, got {expires_at:?}"
    );
}

#[tokio::test]
async fn verify_replayed_code_is_401_invalid_code() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (challenge_id, code) = challenge_with_code(&pool, &cfg, "+989123456789", "register").await;

    let first = post_json(
        app.clone(),
        "/api/v1/auth/otp/verify",
        verify_body(&challenge_id, &code),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let replay = post_json(
        app,
        "/api/v1/auth/otp/verify",
        verify_body(&challenge_id, &code),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(replay).await), "invalid_code");
}

#[tokio::test]
async fn verify_wrong_code_is_401_and_increments_attempt_count() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (challenge_id, code) = challenge_with_code(&pool, &cfg, "+989123456789", "register").await;
    let wrong = wrong_code(&code);

    let resp = post_json(
        app,
        "/api/v1/auth/otp/verify",
        verify_body(&challenge_id, &wrong),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "invalid_code");

    let row = sqlx::query("SELECT attempt_count FROM otp_challenges WHERE public_id = ?")
        .bind(&challenge_id)
        .fetch_one(&pool)
        .await
        .expect("challenge row");
    let attempts: u16 = row.try_get("attempt_count").expect("attempt_count");
    assert_eq!(attempts, 1);
}

#[tokio::test]
async fn verify_locks_after_max_wrong_attempts() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config(); // otp_max_attempts = 5
    let app = common::spawn_app(cfg.clone()).await;
    let (challenge_id, code) = challenge_with_code(&pool, &cfg, "+989123456789", "register").await;
    let wrong = wrong_code(&code);

    for expected_attempts in 1..=5u16 {
        let resp = post_json(
            app.clone(),
            "/api/v1/auth/otp/verify",
            verify_body(&challenge_id, &wrong),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(error_code(&json_body(resp).await), "invalid_code");
        let row = sqlx::query("SELECT attempt_count FROM otp_challenges WHERE public_id = ?")
            .bind(&challenge_id)
            .fetch_one(&pool)
            .await
            .expect("challenge row");
        let attempts: u16 = row.try_get("attempt_count").expect("attempt_count");
        assert_eq!(attempts, expected_attempts);
    }

    // Even the correct code is now locked out.
    let resp = post_json(
        app,
        "/api/v1/auth/otp/verify",
        verify_body(&challenge_id, &code),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "challenge_locked");
}

#[tokio::test]
async fn verify_expired_challenge_is_401_challenge_expired() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (challenge_id, code) = challenge_with_code(&pool, &cfg, "+989123456789", "register").await;

    // Force expiry (boundary: expired at or past expires_at).
    sqlx::query("UPDATE otp_challenges SET expires_at = ? WHERE public_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1))
        .bind(&challenge_id)
        .execute(&pool)
        .await
        .expect("force expiry");

    let resp = post_json(
        app,
        "/api/v1/auth/otp/verify",
        verify_body(&challenge_id, &code),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "challenge_expired");
}

#[tokio::test]
async fn verify_unknown_challenge_id_is_401_invalid_code() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let app = common::spawn_app(common::test_config()).await;

    let unknown = ulid::Ulid::new().to_string();
    let resp = post_json(
        app,
        "/api/v1/auth/otp/verify",
        verify_body(&unknown, "123456"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "invalid_code");
}

#[tokio::test]
async fn verify_register_on_existing_phone_is_not_a_new_user() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let mut cfg = common::test_config();
    // Two challenges for the same phone+purpose: disable the resend cooldown.
    cfg.otp_resend_secs = 0;
    let app = common::spawn_app(cfg.clone()).await;

    let (first_id, first_code) =
        challenge_with_code(&pool, &cfg, "+989123456789", "register").await;
    let resp = post_json(
        app.clone(),
        "/api/v1/auth/otp/verify",
        verify_body(&first_id, &first_code),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let first_body = json_body(resp).await;
    assert_eq!(first_body["is_new_user"], true);
    let public_id = first_body["user"]["public_id"]
        .as_str()
        .expect("public_id")
        .to_string();

    let (second_id, second_code) =
        challenge_with_code(&pool, &cfg, "+989123456789", "register").await;
    let resp = post_json(
        app,
        "/api/v1/auth/otp/verify",
        verify_body(&second_id, &second_code),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["is_new_user"], false);
    assert_eq!(
        body["user"]["public_id"].as_str().expect("public_id"),
        public_id
    );

    // Still exactly one user for the phone.
    let row = sqlx::query("SELECT COUNT(*) AS c FROM users WHERE phone_e164 = ?")
        .bind("+989123456789")
        .fetch_one(&pool)
        .await
        .expect("count");
    let count: i64 = row.try_get("c").expect("count");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn verify_login_without_account_is_401_no_account() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (challenge_id, code) = challenge_with_code(&pool, &cfg, "+989123456789", "login").await;

    let resp = post_json(
        app,
        "/api/v1/auth/otp/verify",
        verify_body(&challenge_id, &code),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&json_body(resp).await), "no_account");

    // Login never creates an account or a session.
    let row = sqlx::query("SELECT COUNT(*) AS c FROM users")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(row.try_get::<i64, _>("c").expect("count"), 0);
    let row = sqlx::query("SELECT COUNT(*) AS c FROM sessions")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(row.try_get::<i64, _>("c").expect("count"), 0);
}

#[tokio::test]
async fn verify_persian_digit_phone_creates_the_e164_user() {
    let _lock = DB_LOCK.lock().await;
    let Some(pool) = common::test_pool().await else {
        return;
    };
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    let (challenge_id, code) = challenge_with_code(&pool, &cfg, "۰۹۱۲۳۴۵۶۷۸۹", "register").await;

    let resp = post_json(
        app,
        "/api/v1/auth/otp/verify",
        verify_body(&challenge_id, &code),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let row = sqlx::query("SELECT phone_e164 FROM users")
        .fetch_one(&pool)
        .await
        .expect("user row");
    assert_eq!(
        row.try_get::<String, _>("phone_e164").expect("phone"),
        "+989123456789"
    );
}
