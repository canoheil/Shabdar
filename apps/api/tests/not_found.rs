//! Phase 2 Task 4 (docs/PHASE2_PLAN.md): the 404 `not_found` fallback
//! envelope. Unknown paths must never return 200 again. The fallback tests
//! never touch the database; the health sanity case needs TEST_DATABASE_URL
//! and skips without it.

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// The 404 tests never touch the database, so they run with NO database at
/// all: a deliberately unreachable URL (valid format, nothing listens on
/// port 1 — same pattern as health.rs's 503 test).
async fn not_found_app() -> axum::Router {
    let mut config = common::test_config();
    config.database_url =
        "mysql://shabdar:fake-placeholder-not-a-secret@127.0.0.1:1/shabdar_test".to_string();
    common::spawn_app(config).await
}

/// Send one request with an optional JSON body; return the raw response.
async fn send(
    app: axum::Router,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> axum::response::Response {
    let mut builder = Request::builder().method(method).uri(path);
    let req = if let Some(json) = body {
        builder = builder.header("content-type", "application/json");
        builder.body(Body::from(json.to_string())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    };
    app.oneshot(req).await.unwrap()
}

/// Collect a response body into parsed JSON.
async fn json_body(resp: axum::response::Response) -> serde_json::Value {
    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("JSON body")
}

/// The Crockford base32 alphabet of a ULID (^[0-9A-HJKMNP-TV-Z]{26}$).
fn is_crockford_ulid(s: &str) -> bool {
    s.len() == 26
        && s.bytes().all(|b| {
            matches!(b,
                b'0'..=b'9' | b'A'..=b'H' | b'J'..=b'K' | b'M'..=b'N' | b'P'..=b'T' | b'V'..=b'Z')
        })
}

/// The documented error envelope for `not_found` (API.md §1, error contract).
fn assert_not_found_envelope(body: &serde_json::Value) {
    assert_eq!(body["error"]["code"], "not_found");
    assert_eq!(body["error"]["message"], "Not found");
    assert!(
        body["error"].get("fields").is_none(),
        "`fields` must be absent on not_found: {body}"
    );
    let id = body["request_id"].as_str().expect("request_id is a string");
    assert!(
        is_crockford_ulid(id),
        "request_id must be a 26-char ULID: {id}"
    );
}

#[tokio::test]
async fn unknown_get_root_is_the_404_not_found_envelope() {
    let app = not_found_app().await;
    let resp = send(app, Method::GET, "/", None).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_not_found_envelope(&json_body(resp).await);
}

#[tokio::test]
async fn unknown_api_paths_are_the_404_not_found_envelope() {
    let app = not_found_app().await;
    for (method, path) in [
        (Method::GET, "/api/v1/unknown"),
        (Method::POST, "/api/v1/unknown"),
    ] {
        let resp = send(app.clone(), method, path, None).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{path}");
        assert_not_found_envelope(&json_body(resp).await);
    }
}

#[tokio::test]
async fn known_routes_still_answer_the_404_fallback_replaced_nothing() {
    let Some(pool) = common::test_pool().await else {
        return; // skipped: TEST_DATABASE_URL unset or not a *_test database
    };
    shabdar_api::db::run_migrations(&pool)
        .await
        .expect("migrations apply");

    // One sanity case (plan Task 4): health and auth routes still 200.
    let app = common::spawn_app(common::test_config()).await;
    let resp = send(app, Method::GET, "/api/v1/health", None).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(
        body,
        serde_json::json!({ "status": "ok", "database": "ok" })
    );
}
