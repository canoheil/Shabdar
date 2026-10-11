//! Phase 3 Tasks 2–4 (docs/PHASE3_PLAN.md): reference reads, seller listing
//! flow, and public feed/detail. Task 2 cases first (RED: routes missing).

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
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
    let bytes = BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
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
