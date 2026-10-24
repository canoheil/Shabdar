//! Phase 5 (docs/PHASE5_PLAN.md, WP B): abuse limits on the OTP endpoints.
//! DB tests need TEST_DATABASE_URL=mysql://…/shabdar_test; without it they
//! print a note and skip. Only `otp_challenges` is cleared between tests.

mod common;

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use shabdar_api::config::Config;

/// One database, many test threads: serialize the DB-touching tests.
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Schema is applied once for this binary; every test then clears the one
/// table it depends on (no FKs point at otp_challenges).
async fn prepare(pool: &sqlx::MySqlPool) {
    static INIT: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
    INIT.get_or_init(|| async {
        shabdar_api::db::run_migrations(pool).await.expect("migrations");
    })
    .await;
    sqlx::query("DELETE FROM otp_challenges")
        .execute(pool)
        .await
        .expect("clear challenges");
}

/// A request from a specific client IP (the proxy header trust is on in these
/// tests, exactly as the Nuxt proxy runs in production).
fn request(path: &str, body: &serde_json::Value, ip: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn send(app: axum::Router, req: Request<Body>) -> (StatusCode, HeaderMap, serde_json::Value) {
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let body = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("JSON body")
    };
    (status, headers, body)
}

fn code(body: &serde_json::Value) -> &str {
    body["error"]["code"].as_str().unwrap_or("")
}

/// The `Retry-After` value as seconds, if the header exists.
fn retry_after(headers: &HeaderMap) -> Option<u64> {
    headers
        .get("retry-after")?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

fn otp_req(phone: &str) -> serde_json::Value {
    serde_json::json!({ "phone": phone, "purpose": "register" })
}

/// Trusted-proxy config with every abuse limit effectively off, so a test can
/// lower the single number it exercises.
fn base_config() -> Config {
    common::test_config_with(|cfg| {
        cfg.trust_proxy = true;
        cfg.otp_request_ip_limit = 1000;
        cfg.otp_verify_ip_limit = 1000;
        cfg.otp_phone_daily_limit = 1000;
        cfg.otp_sms_global_limit = 1000;
        cfg.otp_resend_secs = 0; // no per-phone cooldown unless a test asks
    })
}

#[tokio::test]
async fn otp_request_per_ip_limit_is_429_with_retry_after() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let _guard = DB_LOCK.lock().await;
    prepare(&pool).await;
    let config = common::test_config_with(|cfg| {
        cfg.trust_proxy = true;
        cfg.otp_request_ip_limit = 3;
        cfg.otp_sms_global_limit = 1000;
        cfg.otp_phone_daily_limit = 1000;
        cfg.otp_resend_secs = 0;
    });
    let app = common::spawn_app(config).await;

    for i in 0..3 {
        let phone = format!("0912000{i:04}");
        let (status, _, body) = send(
            app.clone(),
            request("/api/v1/auth/otp/request", &otp_req(&phone), "203.0.113.9"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "request {i} must pass: {body}");
    }

    let (status, headers, body) = send(
        app,
        request(
            "/api/v1/auth/otp/request",
            &otp_req("09129999999"),
            "203.0.113.9",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(code(&body), "rate_limited");
    let secs = retry_after(&headers).expect("retry-after header on 429");
    assert!(
        (1..=600).contains(&secs),
        "retry-after must be inside the window, got {secs}"
    );
}

#[tokio::test]
async fn otp_request_ip_limit_is_per_ip() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let _guard = DB_LOCK.lock().await;
    prepare(&pool).await;
    let config = common::test_config_with(|cfg| {
        cfg.trust_proxy = true;
        cfg.otp_request_ip_limit = 2;
        cfg.otp_sms_global_limit = 1000;
        cfg.otp_phone_daily_limit = 1000;
        cfg.otp_resend_secs = 0;
    });
    let app = common::spawn_app(config).await;

    for i in 0..2 {
        let phone = format!("0912100{i:04}");
        let (status, _, body) = send(
            app.clone(),
            request("/api/v1/auth/otp/request", &otp_req(&phone), "203.0.113.9"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let (status, _, _) = send(
        app.clone(),
        request(
            "/api/v1/auth/otp/request",
            &otp_req("09121009999"),
            "203.0.113.9",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "same ip is capped");

    let (status, _, body) = send(
        app,
        request(
            "/api/v1/auth/otp/request",
            &otp_req("09121008888"),
            "203.0.113.10",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "another ip has its own window: {body}");
}

#[tokio::test]
async fn otp_request_phone_daily_cap_is_429() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let _guard = DB_LOCK.lock().await;
    prepare(&pool).await;
    let config = common::test_config_with(|cfg| {
        cfg.trust_proxy = true;
        cfg.otp_request_ip_limit = 1000;
        cfg.otp_phone_daily_limit = 2;
        cfg.otp_sms_global_limit = 1000;
        cfg.otp_resend_secs = 0;
    });
    let app = common::spawn_app(config).await;
    let phone = "09121112233";

    for i in 0..2 {
        let ip = format!("203.0.113.{}", 20 + i);
        let (status, _, body) =
            send(app.clone(), request("/api/v1/auth/otp/request", &otp_req(phone), &ip)).await;
        assert_eq!(status, StatusCode::OK, "request {i}: {body}");
    }

    let (status, headers, body) = send(
        app,
        request(
            "/api/v1/auth/otp/request",
            &otp_req(phone),
            "203.0.113.29",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(code(&body), "rate_limited");
    assert!(retry_after(&headers).is_some(), "phone cap explains when to retry");
}

#[tokio::test]
async fn otp_request_global_sms_cap_is_429() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let _guard = DB_LOCK.lock().await;
    prepare(&pool).await;
    let config = common::test_config_with(|cfg| {
        cfg.trust_proxy = true;
        cfg.otp_request_ip_limit = 1000;
        cfg.otp_phone_daily_limit = 1000;
        cfg.otp_sms_global_limit = 2;
        cfg.otp_resend_secs = 0;
    });
    let app = common::spawn_app(config).await;

    for i in 0..2 {
        let phone = format!("0912200{i:04}");
        let ip = format!("203.0.113.{}", 40 + i);
        let (status, _, body) =
            send(app.clone(), request("/api/v1/auth/otp/request", &otp_req(&phone), &ip)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    let (status, headers, body) = send(
        app,
        request(
            "/api/v1/auth/otp/request",
            &otp_req("09122009999"),
            "203.0.113.49",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(code(&body), "rate_limited");
    assert!(retry_after(&headers).is_some());
}

#[tokio::test]
async fn otp_request_malformed_phone_does_not_consume_sms_budget() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let _guard = DB_LOCK.lock().await;
    prepare(&pool).await;
    let mut config = base_config();
    config.otp_sms_global_limit = 1;
    let app = common::spawn_app(config).await;

    let (status, _, body) = send(
        app.clone(),
        request(
            "/api/v1/auth/otp/request",
            &otp_req("12345"),
            "203.0.113.60",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&body), "invalid_phone");

    let (status, _, body) = send(
        app,
        request(
            "/api/v1/auth/otp/request",
            &otp_req("09123004455"),
            "203.0.113.60",
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a rejected request must not spend the single sms slot: {body}"
    );
}

#[tokio::test]
async fn otp_verify_per_ip_limit_is_429() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let _guard = DB_LOCK.lock().await;
    prepare(&pool).await;
    let config = common::test_config_with(|cfg| {
        cfg.trust_proxy = true;
        cfg.otp_verify_ip_limit = 2;
        cfg.otp_verify_ip_window_secs = 600;
    });
    let app = common::spawn_app(config).await;
    let bogus = serde_json::json!({ "challenge_id": "01HN00000000000000000000AA", "code": "123456" });

    for _ in 0..2 {
        let (status, _, _) = send(
            app.clone(),
            request("/api/v1/auth/otp/verify", &bogus, "203.0.113.70"),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "bogus challenge is 401");
    }

    let (status, headers, body) = send(
        app.clone(),
        request("/api/v1/auth/otp/verify", &bogus, "203.0.113.70"),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(code(&body), "rate_limited");
    assert!(retry_after(&headers).is_some());

    let (status, _, _) = send(
        app,
        request("/api/v1/auth/otp/verify", &bogus, "203.0.113.71"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "another ip is not capped");
}

/// The peer-address path (TRUST_PROXY off) is what a direct API deployment
/// uses; oneshot requests carry no peer, so the shared "unknown" key applies.
#[tokio::test]
async fn otp_request_without_trust_proxy_ignores_forwarded_header() {
    let Some(pool) = common::test_pool().await else {
        return;
    };
    let _guard = DB_LOCK.lock().await;
    prepare(&pool).await;
    let config = common::test_config_with(|cfg| {
        cfg.trust_proxy = false;
        cfg.otp_request_ip_limit = 2;
        cfg.otp_sms_global_limit = 1000;
        cfg.otp_phone_daily_limit = 1000;
        cfg.otp_resend_secs = 0;
    });
    let app = common::spawn_app(config).await;

    // Two different spoofed headers must land on the same "unknown" key.
    for i in 0..2 {
        let phone = format!("0912400{i:04}");
        let ip = format!("198.51.100.{i}");
        let (status, _, body) =
            send(app.clone(), request("/api/v1/auth/otp/request", &otp_req(&phone), &ip)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let (status, _, body) = send(
        app,
        request(
            "/api/v1/auth/otp/request",
            &otp_req("09124009999"),
            "198.51.100.99",
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "a spoofed header must not buy a fresh window: {body}"
    );
}
