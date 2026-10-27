//! Phase 9 admin users: bounded private search, idempotent transitions,
//! exactly-once audits, and immediate session revocation.

mod common;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use shabdar_api::config::Config;
use sqlx::{MySqlPool, Row};
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
    id: u64,
    public_id: String,
    cookie: String,
    csrf: String,
}

struct UserRow {
    id: u64,
    public_id: String,
}

async fn reset_db(pool: &MySqlPool) {
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
        let sql = format!("DROP TABLE IF EXISTS {table}");
        sqlx::query(&sql).execute(pool).await.expect("drop failed");
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

async fn insert_user(
    pool: &MySqlPool,
    phone: &str,
    display_name: Option<&str>,
    created_at: chrono::DateTime<chrono::Utc>,
) -> UserRow {
    let public_id = ulid::Ulid::new().to_string();
    let result = sqlx::query(
        "INSERT INTO users (public_id, phone_e164, display_name, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&public_id)
    .bind(phone)
    .bind(display_name)
    .bind(created_at)
    .bind(created_at)
    .execute(pool)
    .await
    .expect("user fixture");
    UserRow {
        id: result.last_insert_id(),
        public_id,
    }
}

async fn create_user(
    pool: &MySqlPool,
    cfg: &Config,
    phone: &str,
    display_name: Option<&str>,
) -> TestUser {
    let now = chrono::Utc::now();
    let row = insert_user(pool, phone, display_name, now).await;
    let token = shabdar_api::session::create_session(pool, cfg, row.id)
        .await
        .expect("session fixture");
    let csrf = ulid::Ulid::new().to_string();
    TestUser {
        id: row.id,
        public_id: row.public_id,
        cookie: format!("shabdar_session={token}; shabdar_csrf={csrf}"),
        csrf,
    }
}

async fn create_admin(pool: &MySqlPool, cfg: &Config, phone: &str) -> TestUser {
    let admin = create_user(pool, cfg, phone, Some("مدیر")).await;
    sqlx::query("UPDATE users SET is_admin = TRUE WHERE id = ?")
        .bind(admin.id)
        .execute(pool)
        .await
        .expect("admin fixture");
    admin
}

async fn send(
    app: Router,
    method: Method,
    uri: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    let request = if let Some(json) = body {
        builder
            .header("content-type", "application/json")
            .body(Body::from(json.to_string()))
            .expect("request")
    } else {
        builder.body(Body::empty()).expect("request")
    };
    let response = app.oneshot(request).await.expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = BodyExt::collect(response.into_body())
        .await
        .expect("response body")
        .to_bytes();
    let body = serde_json::from_slice(&bytes).expect("JSON response");
    (status, headers, body)
}

async fn list_users(
    app: Router,
    admin: &TestUser,
    query: &str,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::GET,
        &format!("/api/v1/admin/users{query}"),
        Some(&admin.cookie),
        None,
        None,
    )
    .await
}

async fn transition(
    app: Router,
    admin: &TestUser,
    target_public_id: &str,
    action: &str,
    csrf: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::POST,
        &format!("/api/v1/admin/users/{target_public_id}/{action}"),
        Some(&admin.cookie),
        csrf,
        body,
    )
    .await
}

async fn audit_count(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs")
        .fetch_one(pool)
        .await
        .expect("audit count")
}

async fn session_count(pool: &MySqlPool, user_id: u64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("session count")
}

fn user_ids(body: &serde_json::Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("user data")
        .iter()
        .map(|user| user["public_id"].as_str().expect("public ID").to_string())
        .collect()
}

fn assert_admin_user_shape(user: &serde_json::Value) {
    let object = user.as_object().expect("admin user object");
    assert_eq!(object.len(), 6, "exact admin-only shape: {user}");
    for key in [
        "public_id",
        "display_name",
        "phone_e164",
        "status",
        "is_admin",
        "created_at",
    ] {
        assert!(object.contains_key(key), "missing {key}: {user}");
    }
    assert!(!object.contains_key("id"));
    assert!(!object.contains_key("deleted_at"));
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[tokio::test]
async fn admin_user_routes_require_admin_csrf_and_prevent_self_suspend() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg, "+989100000001").await;
    let user = create_user(&t.pool, &t.cfg, "+989100000002", Some("کاربر")).await;

    let (status, _, body) = send(
        t.app.clone(),
        Method::GET,
        "/api/v1/admin/users",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");
    let (status, _, body) = list_users(t.app.clone(), &user, "").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");
    assert_eq!(
        list_users(t.app.clone(), &admin, "").await.0,
        StatusCode::OK
    );

    let uri = format!("/api/v1/admin/users/{}/suspend", user.public_id);
    let reason = serde_json::json!({"reason": "abuse"});
    let (status, _, _) = send(
        t.app.clone(),
        Method::POST,
        &uri,
        None,
        None,
        Some(reason.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = transition(
        t.app.clone(),
        &user,
        &user.public_id,
        "suspend",
        Some(&user.csrf),
        Some(reason.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    for csrf in [None, Some("wrong-token")] {
        let (status, _, body) = transition(
            t.app.clone(),
            &admin,
            &user.public_id,
            "suspend",
            csrf,
            Some(reason.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(body["error"]["code"], "csrf_failed");
    }
    let (status, _, body) = transition(
        t.app.clone(),
        &admin,
        &admin.public_id,
        "suspend",
        Some(&admin.csrf),
        Some(reason),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");
    assert_eq!(audit_count(&t.pool).await, 0);
    assert_eq!(session_count(&t.pool, user.id).await, 1);
}

#[tokio::test]
async fn admin_user_search_escapes_like_and_validates_unicode_length() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg, "+989100000001").await;
    let now = chrono::Utc::now();
    let percent = insert_user(&t.pool, "+989100000010", Some("100% واقعی"), now).await;
    insert_user(&t.pool, "+989100000011", Some("1000 غیر"), now).await;
    let underscore = insert_user(&t.pool, "+989100000012", Some("زیر_خط"), now).await;
    insert_user(&t.pool, "+989100000013", Some("زیرXخط"), now).await;
    let backslash = insert_user(&t.pool, "+989100000014", Some("slash\\name"), now).await;
    insert_user(&t.pool, "+989100000015", Some("slashXname"), now).await;
    let phone = insert_user(&t.pool, "+989100000099", Some("تلفن"), now).await;

    for (raw, expected) in [
        ("%", vec![percent.public_id.clone()]),
        ("_", vec![underscore.public_id.clone()]),
        ("\\", vec![backslash.public_id.clone()]),
        ("90000099", vec![phone.public_id.clone()]),
    ] {
        let query = format!("?query={}", percent_encode(raw));
        let (status, _, body) = list_users(t.app.clone(), &admin, &query).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(user_ids(&body), expected, "query {raw}");
        for user in body["data"].as_array().expect("users") {
            assert_admin_user_shape(user);
        }
    }

    let exactly_100 = "🙂".repeat(100);
    let query = format!("?query={}", percent_encode(&exactly_100));
    assert_eq!(
        list_users(t.app.clone(), &admin, &query).await.0,
        StatusCode::OK
    );
    let too_long = "🙂".repeat(101);
    let query = format!("?query={}", percent_encode(&too_long));
    let (status, _, body) = list_users(t.app.clone(), &admin, &query).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
    assert_eq!(body["error"]["fields"]["query"], "too_long");

    let (status, _, body) = list_users(t.app.clone(), &admin, "?cursor=bad").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["cursor"], "invalid_cursor");
}

#[tokio::test]
async fn admin_user_list_is_newest_keyset_and_survives_anchor_status_change() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg, "+989100000001").await;
    let created_at = chrono::Utc::now();
    let mut users = Vec::new();
    for (phone, name) in [
        ("+989100000010", "اول"),
        ("+989100000011", "دوم"),
        ("+989100000012", "سوم"),
        ("+989100000013", "چهارم"),
    ] {
        users.push(insert_user(&t.pool, phone, Some(name), created_at).await);
    }

    let (status, _, first_page) = list_users(
        t.app.clone(),
        &admin,
        "?limit=2&query=98910000001",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_page["page"]["has_more"], true);
    assert_eq!(
        user_ids(&first_page),
        vec![users[3].public_id.clone(), users[2].public_id.clone()]
    );
    assert_admin_user_shape(&first_page["data"][0]);
    assert_eq!(first_page["data"][0]["phone_e164"], "+989100000013");
    assert_eq!(first_page["data"][0]["status"], "active");
    assert_eq!(first_page["data"][0]["is_admin"], false);
    let cursor = first_page["page"]["next_cursor"]
        .as_str()
        .expect("next cursor")
        .to_string();

    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = ?")
        .bind(users[2].id)
        .execute(&t.pool)
        .await
        .expect("anchor status fixture");
    let (status, _, second_page) =
        list_users(
            t.app.clone(),
            &admin,
            &format!("?limit=2&query=98910000001&cursor={cursor}"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_page["page"]["has_more"], false);
    assert_eq!(
        user_ids(&second_page),
        vec![users[1].public_id.clone(), users[0].public_id.clone()]
    );
    assert!(second_page["page"]["next_cursor"].is_null());
}

#[tokio::test]
async fn admin_user_list_defaults_to_twenty_and_caps_at_fifty() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg, "+989100000001").await;
    let created_at = chrono::Utc::now();
    for index in 0..55 {
        insert_user(
            &t.pool,
            &format!("+98920{index:07}"),
            Some("صفحه"),
            created_at,
        )
        .await;
    }
    let (status, _, default_page) = list_users(t.app.clone(), &admin, "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        default_page["data"].as_array().expect("default data").len(),
        20
    );
    assert_eq!(default_page["page"]["has_more"], true);

    let (status, _, capped_page) = list_users(t.app.clone(), &admin, "?limit=100").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        capped_page["data"].as_array().expect("capped data").len(),
        50
    );
    assert_eq!(capped_page["page"]["has_more"], true);
}

#[tokio::test]
async fn user_transitions_validate_reason_and_are_idempotent_with_exact_audits() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg, "+989100000001").await;
    let target = create_user(&t.pool, &t.cfg, "+989100000002", Some("هدف")).await;

    for body in [
        serde_json::json!({}),
        serde_json::json!({"reason": "  "}),
        serde_json::json!({"reason": "🙂".repeat(501)}),
    ] {
        let (status, _, response) = transition(
            t.app.clone(),
            &admin,
            &target.public_id,
            "suspend",
            Some(&admin.csrf),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert_eq!(response["error"]["code"], "validation_error");
    }
    assert_eq!(audit_count(&t.pool).await, 0);
    assert_eq!(session_count(&t.pool, target.id).await, 1);

    let reason = "🙂".repeat(500);
    let (status, _, suspended) = transition(
        t.app.clone(),
        &admin,
        &target.public_id,
        "suspend",
        Some(&admin.csrf),
        Some(serde_json::json!({"reason": format!("  {reason}  ")})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{suspended}");
    assert_admin_user_shape(&suspended);
    assert_eq!(suspended["public_id"], target.public_id);
    assert_eq!(suspended["status"], "suspended");
    assert_eq!(suspended["phone_e164"], "+989100000002");
    assert_eq!(audit_count(&t.pool).await, 1);

    let (status, _, repeated) = transition(
        t.app.clone(),
        &admin,
        &target.public_id,
        "suspend",
        Some(&admin.csrf),
        Some(serde_json::json!({"reason": "different"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated, suspended);
    assert_eq!(audit_count(&t.pool).await, 1);

    for _ in 0..2 {
        let (status, _, active) = transition(
            t.app.clone(),
            &admin,
            &target.public_id,
            "unsuspend",
            Some(&admin.csrf),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{active}");
        assert_eq!(active["status"], "active");
        assert_admin_user_shape(&active);
    }
    assert_eq!(audit_count(&t.pool).await, 2);

    let audits = sqlx::query(
        "SELECT action, target_type, target_id, reason FROM admin_audit_logs ORDER BY id",
    )
    .fetch_all(&t.pool)
    .await
    .expect("audit rows");
    assert_eq!(audits.len(), 2);
    assert_eq!(
        audits[0].try_get::<String, _>("action").expect("action"),
        "user.suspend"
    );
    assert_eq!(
        audits[0]
            .try_get::<String, _>("target_type")
            .expect("target"),
        "user"
    );
    assert_eq!(
        audits[0].try_get::<u64, _>("target_id").expect("target ID"),
        target.id
    );
    assert_eq!(
        audits[0]
            .try_get::<Option<String>, _>("reason")
            .expect("reason")
            .as_deref(),
        Some(reason.as_str())
    );
    assert_eq!(
        audits[1].try_get::<String, _>("action").expect("action"),
        "user.unsuspend"
    );
    assert!(audits[1]
        .try_get::<Option<String>, _>("reason")
        .expect("reason")
        .is_none());

    let (status, _, _) = transition(
        t.app.clone(),
        &admin,
        &"0".repeat(26),
        "suspend",
        Some(&admin.csrf),
        Some(serde_json::json!({"reason": "unknown"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(audit_count(&t.pool).await, 2);
}

#[tokio::test]
async fn suspension_revokes_every_session_and_unsuspend_does_not_restore_them() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg, "+989100000001").await;
    let target = create_user(&t.pool, &t.cfg, "+989100000002", Some("هدف")).await;
    let mut target_cookies = vec![target.cookie.clone()];
    for _ in 0..2 {
        let token = shabdar_api::session::create_session(&t.pool, &t.cfg, target.id)
            .await
            .expect("extra session");
        target_cookies.push(format!("shabdar_session={token}; shabdar_csrf=extra"));
    }
    assert_eq!(session_count(&t.pool, target.id).await, 3);

    // Concurrent duplicate commands serialize on the user row and audit once.
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let app = t.app.clone();
        let admin = admin.clone();
        let public_id = target.public_id.clone();
        tasks.push(tokio::spawn(async move {
            transition(
                app,
                &admin,
                &public_id,
                "suspend",
                Some(&admin.csrf),
                Some(serde_json::json!({"reason": "concurrent abuse"})),
            )
            .await
        }));
    }
    for task in tasks {
        let (status, _, body) = task.await.expect("suspend task");
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["status"], "suspended");
    }
    assert_eq!(session_count(&t.pool, target.id).await, 0);
    assert_eq!(audit_count(&t.pool).await, 1);

    for cookie in &target_cookies {
        let (status, _, body) = send(
            t.app.clone(),
            Method::GET,
            "/api/v1/me",
            Some(cookie),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "unauthorized");
    }

    let (status, _, active) = transition(
        t.app.clone(),
        &admin,
        &target.public_id,
        "unsuspend",
        Some(&admin.csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(active["status"], "active");
    assert_eq!(audit_count(&t.pool).await, 2);
    for cookie in &target_cookies {
        let (status, _, _) = send(
            t.app.clone(),
            Method::GET,
            "/api/v1/me",
            Some(cookie),
            None,
            None,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "old sessions stay revoked"
        );
    }

    let new_token = shabdar_api::session::create_session(&t.pool, &t.cfg, target.id)
        .await
        .expect("new post-unsuspend session");
    let (status, _, _) = send(
        t.app.clone(),
        Method::GET,
        "/api/v1/me",
        Some(&format!("shabdar_session={new_token}")),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
