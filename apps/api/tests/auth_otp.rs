//! Task 8 (docs/PHASE1_PLAN.md): POST /api/v1/auth/otp/request.
//! DB tests need TEST_DATABASE_URL=mysql://…/shabdar_test; without it they
//! print a note and skip. Tests never read DATABASE_URL, and the pool helper
//! refuses anything that is not a *_test database.

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
    assert!(is_crockford_ulid(challenge_id), "not a ULID: {challenge_id}");
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
    let created = shabdar_api::otp_service::create_challenge(
        &pool,
        &cfg,
        "+989123456789",
        "register",
        &code,
    )
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
