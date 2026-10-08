//! Task 5 (docs/PHASE1_PLAN.md): GET /api/v1/health. Approved shape:
//! 200 {"status":"ok","database":"ok"} with a reachable database,
//! 503 {"status":"error","database":"error"} without one.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt; // collect a body to bytes
use tower::ServiceExt;       // Router::oneshot

async fn call_health(app: axum::Router) -> (StatusCode, serde_json::Value, bool) {
    let req = Request::builder()
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let has_request_id = resp.headers().contains_key("x-request-id");
    let bytes = BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let body = serde_json::from_slice(&bytes).expect("JSON body");
    (status, body, has_request_id)
}

#[tokio::test]
async fn health_200_ok_ok_when_database_is_reachable() {
    let Some(pool) = common::test_pool().await else {
        return; // skipped: TEST_DATABASE_URL unset or not a *_test database
    };
    shabdar_api::db::run_migrations(&pool)
        .await
        .expect("migrations apply");

    let app = common::spawn_app(common::test_config()).await;
    let (status, body, has_request_id) = call_health(app).await;

    assert_eq!(status, StatusCode::OK);
    // Exact body: these two keys and nothing else.
    assert_eq!(body, serde_json::json!({"status": "ok", "database": "ok"}));
    assert!(has_request_id, "request-id layer must wrap the health route too");
}

#[tokio::test]
async fn health_503_error_error_when_database_is_unreachable() {
    // Deliberately unreachable: nothing listens on port 1. Runs with NO
    // database at all (plan: "test (b) runs without MariaDB").
    let mut config = common::test_config();
    config.database_url =
        "mysql://shabdar:fake-placeholder-not-a-secret@127.0.0.1:1/shabdar_test".to_string();

    let app = common::spawn_app(config).await;
    let (status, body, _has_request_id) = call_health(app).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body, serde_json::json!({"status": "error", "database": "error"}));
}
