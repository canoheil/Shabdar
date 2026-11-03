//! Advertising slot and internal request workflow against the real MariaDB schema.

mod common;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use shabdar_api::config::Config;
use sqlx::MySqlPool;
use tower::ServiceExt;

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Setup {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    app: Router,
    pool: MySqlPool,
    cfg: Config,
}

#[derive(Clone)]
struct TestUser {
    public_id: String,
    cookie: String,
    csrf: String,
}

async fn reset_db(pool: &MySqlPool) {
    for table in [
        "advertising_requests",
        "advertising_slots",
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
        sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
            .execute(pool)
            .await
            .expect("drop failed");
    }
    shabdar_api::db::run_migrations(pool)
        .await
        .expect("migrations apply");
}

async fn setup() -> Option<Setup> {
    let lock = TEST_LOCK.lock().await;
    let pool = common::test_pool().await?;
    reset_db(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    Some(Setup {
        _lock: lock,
        app,
        pool,
        cfg,
    })
}

async fn create_user(pool: &MySqlPool, cfg: &Config, phone: &str, admin: bool) -> TestUser {
    let public_id = ulid::Ulid::new().to_string();
    let result = sqlx::query(
        "INSERT INTO users (public_id, phone_e164, display_name, is_admin, created_at, updated_at) \
         VALUES (?, ?, 'آزمون تبلیغ', ?, NOW(3), NOW(3))",
    )
    .bind(&public_id)
    .bind(phone)
    .bind(admin)
    .execute(pool)
    .await
    .expect("user fixture");
    let id = result.last_insert_id();
    let token = shabdar_api::session::create_session(pool, cfg, id)
        .await
        .expect("session fixture");
    let csrf = ulid::Ulid::new().to_string();
    TestUser {
        public_id,
        cookie: format!("shabdar_session={token}; shabdar_csrf={csrf}"),
        csrf,
    }
}

struct TestResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    user: Option<&TestUser>,
    csrf: bool,
    body: Option<Value>,
) -> TestResponse {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(user) = user {
        builder = builder.header("cookie", &user.cookie);
        if csrf {
            builder = builder.header("x-csrf-token", &user.csrf);
        }
    }
    let request = if let Some(body) = body {
        builder
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request")
    } else {
        builder.body(Body::empty()).expect("request")
    };
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("JSON response")
    };
    TestResponse {
        status,
        headers,
        body,
    }
}

fn valid_slot(enabled: bool) -> Value {
    json!({
        "title": "معرفی کسب‌وکار محلی",
        "body": "متن واقعی جایگاه تبلیغاتی برای معرفی خدمات به اهالی رشت.",
        "cta_label": "ثبت تبلیغ در شبدر",
        "cta_url": "/advertising",
        "enabled": enabled,
    })
}

fn request_body(index: usize) -> Value {
    json!({
        "submission_id": format!("00000000-0000-4000-8000-{index:012}"),
        "contact_name": format!("درخواست‌کنندهٔ {index}"),
        "phone": "09123456789",
        "details": "برای معرفی فروشگاه محلی و هماهنگی جایگاه تماس بگیرید."
    })
}

#[tokio::test]
async fn slot_is_public_admin_controlled_and_safely_deleted() {
    let Some(setup) = setup().await else { return };
    let admin = create_user(&setup.pool, &setup.cfg, "+989100000101", true).await;
    let member = create_user(&setup.pool, &setup.cfg, "+989100000102", false).await;

    let public = call(
        &setup.app,
        Method::GET,
        "/api/v1/advertising/slot",
        None,
        false,
        None,
    )
    .await;
    assert_eq!(public.status, StatusCode::OK);
    assert_eq!(public.body["slot"]["cta_url"], "/advertising");
    assert!(public.body["slot"].get("enabled").is_none());
    assert!(public.body["slot"].get("phone_e164").is_none());

    let anonymous_admin = call(
        &setup.app,
        Method::GET,
        "/api/v1/admin/advertising/slot",
        None,
        false,
        None,
    )
    .await;
    assert_eq!(anonymous_admin.status, StatusCode::UNAUTHORIZED);
    let member_save = call(
        &setup.app,
        Method::PUT,
        "/api/v1/admin/advertising/slot",
        Some(&member),
        true,
        Some(valid_slot(true)),
    )
    .await;
    assert_eq!(member_save.status, StatusCode::FORBIDDEN);

    let mut unsafe_slot = valid_slot(true);
    unsafe_slot["cta_url"] = json!("javascript:alert(1)");
    let invalid = call(
        &setup.app,
        Method::PUT,
        "/api/v1/admin/advertising/slot",
        Some(&admin),
        true,
        Some(unsafe_slot),
    )
    .await;
    assert_eq!(invalid.status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid.body["error"]["fields"]["cta_url"], "invalid_url");

    let saved = call(
        &setup.app,
        Method::PUT,
        "/api/v1/admin/advertising/slot",
        Some(&admin),
        true,
        Some(valid_slot(true)),
    )
    .await;
    assert_eq!(saved.status, StatusCode::OK);
    assert_eq!(saved.body["slot"]["title"], "معرفی کسب‌وکار محلی");
    let original_id = saved.body["slot"]["public_id"].clone();

    let disabled = call(
        &setup.app,
        Method::PUT,
        "/api/v1/admin/advertising/slot",
        Some(&admin),
        true,
        Some(valid_slot(false)),
    )
    .await;
    assert_eq!(disabled.status, StatusCode::OK);
    assert_eq!(disabled.body["slot"]["public_id"], original_id);
    assert_eq!(disabled.body["slot"]["enabled"], false);
    let hidden = call(
        &setup.app,
        Method::GET,
        "/api/v1/advertising/slot",
        None,
        false,
        None,
    )
    .await;
    assert_eq!(hidden.body["slot"], Value::Null);

    let without_csrf = call(
        &setup.app,
        Method::DELETE,
        "/api/v1/admin/advertising/slot",
        Some(&admin),
        false,
        None,
    )
    .await;
    assert_eq!(without_csrf.status, StatusCode::FORBIDDEN);
    let still_there = call(
        &setup.app,
        Method::GET,
        "/api/v1/admin/advertising/slot",
        Some(&admin),
        false,
        None,
    )
    .await;
    assert!(still_there.body["slot"].is_object());

    let deleted = call(
        &setup.app,
        Method::DELETE,
        "/api/v1/admin/advertising/slot",
        Some(&admin),
        true,
        None,
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert_eq!(deleted.body["deleted"], true);
    let deleted_again = call(
        &setup.app,
        Method::DELETE,
        "/api/v1/admin/advertising/slot",
        Some(&admin),
        true,
        None,
    )
    .await;
    assert_eq!(deleted_again.body["deleted"], false);
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_logs WHERE target_type = 'advertising_slot'",
    )
    .fetch_one(&setup.pool)
    .await
    .expect("audit count");
    assert_eq!(count, 3, "two edits and one actual delete only");
}

#[tokio::test]
async fn requests_are_idempotent_private_and_follow_the_admin_workflow() {
    let Some(setup) = setup().await else { return };
    let admin = create_user(&setup.pool, &setup.cfg, "+989100000111", true).await;
    let member = create_user(&setup.pool, &setup.cfg, "+989100000112", false).await;

    let mut invalid = request_body(1);
    invalid["phone"] = json!("02112345678");
    let rejected = call(
        &setup.app,
        Method::POST,
        "/api/v1/advertising/requests",
        None,
        false,
        Some(invalid),
    )
    .await;
    assert_eq!(rejected.status, StatusCode::BAD_REQUEST);
    assert_eq!(rejected.body["error"]["fields"]["phone"], "invalid_phone");

    let first = call(
        &setup.app,
        Method::POST,
        "/api/v1/advertising/requests",
        None,
        false,
        Some(request_body(1)),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(first.body["request"]["status"], "new");
    let request_id = first.body["request"]["public_id"]
        .as_str()
        .expect("id")
        .to_string();
    assert!(
        !first.headers.contains_key("set-cookie"),
        "no anonymous identity cookie"
    );

    let mut retry = request_body(1);
    retry["contact_name"] = json!("نام تغییرکرده در retry");
    let replay = call(
        &setup.app,
        Method::POST,
        "/api/v1/advertising/requests",
        None,
        false,
        Some(retry),
    )
    .await;
    assert_eq!(replay.status, StatusCode::OK);
    assert_eq!(replay.body["request"]["public_id"], request_id);
    let stored_name: String = sqlx::query_scalar("SELECT contact_name FROM advertising_requests")
        .fetch_one(&setup.pool)
        .await
        .expect("stored request");
    assert_eq!(stored_name, "درخواست‌کنندهٔ 1");

    let anonymous_queue = call(
        &setup.app,
        Method::GET,
        "/api/v1/admin/advertising/requests",
        None,
        false,
        None,
    )
    .await;
    assert_eq!(anonymous_queue.status, StatusCode::UNAUTHORIZED);
    let member_queue = call(
        &setup.app,
        Method::GET,
        "/api/v1/admin/advertising/requests",
        Some(&member),
        false,
        None,
    )
    .await;
    assert_eq!(member_queue.status, StatusCode::FORBIDDEN);

    let queue = call(
        &setup.app,
        Method::GET,
        "/api/v1/admin/advertising/requests?status=new",
        Some(&admin),
        false,
        None,
    )
    .await;
    assert_eq!(queue.status, StatusCode::OK);
    assert_eq!(queue.headers["cache-control"], "private, no-store");
    assert_eq!(queue.body["data"].as_array().expect("page").len(), 1);
    assert_eq!(queue.body["data"][0]["contact_name"], "درخواست‌کنندهٔ 1");
    assert_eq!(queue.body["data"][0]["phone_e164"], "+989123456789");
    assert_eq!(queue.body["page"]["has_more"], false);

    let patch_uri = format!("/api/v1/admin/advertising/requests/{request_id}");
    let no_csrf = call(
        &setup.app,
        Method::PATCH,
        &patch_uri,
        Some(&admin),
        false,
        Some(json!({"status": "in_progress"})),
    )
    .await;
    assert_eq!(no_csrf.status, StatusCode::FORBIDDEN);
    let member_patch = call(
        &setup.app,
        Method::PATCH,
        &patch_uri,
        Some(&member),
        true,
        Some(json!({"status": "in_progress"})),
    )
    .await;
    assert_eq!(member_patch.status, StatusCode::FORBIDDEN);

    for status in ["in_progress", "closed", "new"] {
        let updated = call(
            &setup.app,
            Method::PATCH,
            &patch_uri,
            Some(&admin),
            true,
            Some(json!({"status": status})),
        )
        .await;
        assert_eq!(updated.status, StatusCode::OK);
        assert_eq!(updated.body["request"]["status"], status);
        assert_eq!(updated.body["request"]["phone_e164"], "+989123456789");
        assert_eq!(updated.headers["cache-control"], "private, no-store");
        if status == "in_progress" {
            assert_eq!(
                updated.body["request"]["handled_by"]["public_id"],
                admin.public_id
            );
            let repeat = call(
                &setup.app,
                Method::PATCH,
                &patch_uri,
                Some(&admin),
                true,
                Some(json!({"status": status})),
            )
            .await;
            assert_eq!(repeat.status, StatusCode::OK);
        }
        if status == "new" {
            assert_eq!(updated.body["request"]["handled_by"], Value::Null);
            assert_eq!(updated.body["request"]["handled_at"], Value::Null);
        }
    }
    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admin_audit_logs WHERE target_type = 'advertising_request'",
    )
    .fetch_one(&setup.pool)
    .await
    .expect("request audits");
    assert_eq!(audits, 3, "same-status retry must not add an audit");
    let admin_id: Option<u64> = sqlx::query_scalar("SELECT handled_by FROM advertising_requests")
        .fetch_one(&setup.pool)
        .await
        .expect("handler reset");
    assert_eq!(admin_id, None);
}

#[tokio::test]
async fn request_pages_are_keyset_and_phone_abuse_limit_keeps_replays_idempotent() {
    let Some(setup) = setup().await else { return };
    let admin = create_user(&setup.pool, &setup.cfg, "+989100000121", true).await;
    let mut first_id = String::new();
    for index in 1..=3 {
        // Separate timestamps make the fixture independent of database clock precision.
        let created = call(
            &setup.app,
            Method::POST,
            "/api/v1/advertising/requests",
            None,
            false,
            Some(request_body(index)),
        )
        .await;
        assert_eq!(created.status, StatusCode::OK);
        if index == 1 {
            first_id = created.body["request"]["public_id"]
                .as_str()
                .expect("id")
                .to_string();
        }
        sqlx::query("UPDATE advertising_requests SET created_at = created_at + INTERVAL 1 SECOND, updated_at = updated_at + INTERVAL 1 SECOND WHERE submission_id = ?")
            .bind(format!("00000000-0000-4000-8000-{index:012}"))
            .execute(&setup.pool)
            .await
            .expect("fixture ordering");
    }

    let first_page = call(
        &setup.app,
        Method::GET,
        "/api/v1/admin/advertising/requests?limit=2",
        Some(&admin),
        false,
        None,
    )
    .await;
    assert_eq!(first_page.status, StatusCode::OK);
    assert_eq!(first_page.body["data"].as_array().expect("data").len(), 2);
    assert_eq!(first_page.body["page"]["has_more"], true);
    let cursor = first_page.body["page"]["next_cursor"]
        .as_str()
        .expect("cursor");
    let second_page = call(
        &setup.app,
        Method::GET,
        &format!("/api/v1/admin/advertising/requests?limit=2&cursor={cursor}"),
        Some(&admin),
        false,
        None,
    )
    .await;
    assert_eq!(second_page.status, StatusCode::OK);
    assert_eq!(second_page.body["data"].as_array().expect("data").len(), 1);
    assert_eq!(second_page.body["page"]["has_more"], false);
    let first_ids = first_page.body["data"].as_array().expect("data");
    let second_ids = second_page.body["data"].as_array().expect("data");
    assert!(!first_ids.iter().any(|left| second_ids
        .iter()
        .any(|right| left["public_id"] == right["public_id"])));

    let limited = call(
        &setup.app,
        Method::POST,
        "/api/v1/advertising/requests",
        None,
        false,
        Some(request_body(4)),
    )
    .await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(limited.headers.contains_key("retry-after"));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM advertising_requests")
        .fetch_one(&setup.pool)
        .await
        .expect("request count");
    assert_eq!(count, 3);

    let replay = call(
        &setup.app,
        Method::POST,
        "/api/v1/advertising/requests",
        None,
        false,
        Some(request_body(1)),
    )
    .await;
    assert_eq!(
        replay.status,
        StatusCode::OK,
        "existing receipt bypasses abuse counters"
    );
    assert_eq!(replay.body["request"]["public_id"], first_id);
}
