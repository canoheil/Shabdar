//! Phase 10 admin-managed literal moderation rules, revisions, and audits.

mod common;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use shabdar_api::config::Config;
use shabdar_api::moderation::{normalize_rule, MatchMode};
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
    cookie: String,
    csrf: String,
}

async fn reset_db(pool: &MySqlPool) {
    for table in [
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

async fn create_user(pool: &MySqlPool, cfg: &Config, phone: &str) -> TestUser {
    let public_id = ulid::Ulid::new().to_string();
    let result = sqlx::query(
        "INSERT INTO users (public_id, phone_e164, display_name, created_at, updated_at) \
         VALUES (?, ?, 'Fixture', NOW(3), NOW(3))",
    )
    .bind(public_id)
    .bind(phone)
    .execute(pool)
    .await
    .expect("user fixture");
    let id = result.last_insert_id();
    let token = shabdar_api::session::create_session(pool, cfg, id)
        .await
        .expect("session fixture");
    let csrf = ulid::Ulid::new().to_string();
    TestUser {
        id,
        cookie: format!("shabdar_session={token}; shabdar_csrf={csrf}"),
        csrf,
    }
}

async fn create_admin(pool: &MySqlPool, cfg: &Config) -> TestUser {
    let admin = create_user(pool, cfg, "+989100000001").await;
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

async fn list_rules(
    app: Router,
    admin: &TestUser,
    query: &str,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::GET,
        &format!("/api/v1/admin/moderation/rules{query}"),
        Some(&admin.cookie),
        None,
        None,
    )
    .await
}

async fn create_rule(
    app: Router,
    admin: &TestUser,
    body: serde_json::Value,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::POST,
        "/api/v1/admin/moderation/rules",
        Some(&admin.cookie),
        Some(&admin.csrf),
        Some(body),
    )
    .await
}

async fn patch_rule(
    app: Router,
    admin: &TestUser,
    public_id: &str,
    body: serde_json::Value,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::PATCH,
        &format!("/api/v1/admin/moderation/rules/{public_id}"),
        Some(&admin.cookie),
        Some(&admin.csrf),
        Some(body),
    )
    .await
}

async fn clear_rules(pool: &MySqlPool) {
    sqlx::query("DELETE FROM moderation_rules")
        .execute(pool)
        .await
        .expect("clear rules");
    sqlx::query("DELETE FROM admin_audit_logs")
        .execute(pool)
        .await
        .expect("clear audits");
    sqlx::query("UPDATE moderation_settings SET revision = 1 WHERE id = 1")
        .execute(pool)
        .await
        .expect("reset rule-set revision");
}

async fn settings_revision(pool: &MySqlPool) -> u64 {
    sqlx::query_scalar("SELECT revision FROM moderation_settings WHERE id = 1")
        .fetch_one(pool)
        .await
        .expect("settings revision")
}

async fn rule_count(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM moderation_rules")
        .fetch_one(pool)
        .await
        .expect("rule count")
}

async fn audit_count(pool: &MySqlPool, action: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE action = ?")
        .bind(action)
        .fetch_one(pool)
        .await
        .expect("audit count")
}

async fn rule_state(pool: &MySqlPool, public_id: &str) -> (String, bool, u64) {
    let row = sqlx::query(
        "SELECT phrase, is_active, revision FROM moderation_rules WHERE public_id = ?",
    )
    .bind(public_id)
    .fetch_one(pool)
    .await
    .expect("rule state");
    (
        row.try_get("phrase").expect("phrase"),
        row.try_get("is_active").expect("active"),
        row.try_get("revision").expect("revision"),
    )
}

async fn insert_rule_fixture(
    pool: &MySqlPool,
    phrase: &str,
    active: bool,
    created_at: chrono::DateTime<chrono::Utc>,
) -> String {
    let normalized = normalize_rule(phrase, MatchMode::Compact).expect("valid fixture");
    let public_id = ulid::Ulid::new().to_string();
    sqlx::query(
        "INSERT INTO moderation_rules \
         (public_id, phrase, normalized_phrase, dedupe_key, match_mode, is_active, revision, \
          created_at, updated_at) VALUES (?, ?, ?, ?, 'compact', ?, 1, ?, ?)",
    )
    .bind(&public_id)
    .bind(&normalized.phrase)
    .bind(&normalized.normalized_phrase)
    .bind(&normalized.dedupe_key)
    .bind(active)
    .bind(created_at)
    .bind(created_at)
    .execute(pool)
    .await
    .expect("rule fixture");
    public_id
}

fn assert_rule_shape(rule: &serde_json::Value) {
    let object = rule.as_object().expect("rule object");
    assert_eq!(object.len(), 7, "exact public rule shape: {rule}");
    for key in [
        "public_id",
        "phrase",
        "match_mode",
        "is_active",
        "revision",
        "created_at",
        "updated_at",
    ] {
        assert!(object.contains_key(key), "missing {key}: {rule}");
    }
    assert!(rule.get("dedupe_key").is_none());
    assert!(rule.get("normalized_phrase").is_none());
    assert!(rule.get("id").is_none());
}

fn assert_field(body: &serde_json::Value, field: &str, expected: &str) {
    assert_eq!(body["error"]["code"], "validation_error", "{body}");
    assert_eq!(body["error"]["fields"][field], expected, "{body}");
}

#[tokio::test]
async fn routes_require_admin_csrf_and_private_cache() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let non_admin = create_user(&t.pool, &t.cfg, "+989100000002").await;

    let (status, _, _) = send(
        t.app.clone(),
        Method::GET,
        "/api/v1/admin/moderation/rules",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = list_rules(t.app.clone(), &non_admin, "").await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, headers, body) = list_rules(t.app.clone(), &admin, "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        headers.get("cache-control").and_then(|value| value.to_str().ok()),
        Some("private, no-store")
    );
    assert_eq!(body["data"].as_array().map(Vec::len), Some(20));

    let create = serde_json::json!({
        "phrase": "Blocked create", "match_mode": "compact", "reason": "review"
    });
    let (status, _, _) = send(
        t.app.clone(),
        Method::POST,
        "/api/v1/admin/moderation/rules",
        Some(&admin.cookie),
        None,
        Some(create.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = create_rule(t.app.clone(), &non_admin, create).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(settings_revision(&t.pool).await, 1);
    assert_eq!(audit_count(&t.pool, "moderation_rule.create").await, 0);
}

#[tokio::test]
async fn rule_validation_boundaries_literals_and_patch_contract() {
    let Some(t) = setup().await else { return };
    clear_rules(&t.pool).await;
    let admin = create_admin(&t.pool, &t.cfg).await;
    let invalid = [
        (
            serde_json::json!({"phrase": "valid", "match_mode": "compact"}),
            "reason",
            "required",
        ),
        (
            serde_json::json!({"phrase": "valid", "match_mode": "compact", "reason": " "}),
            "reason",
            "required",
        ),
        (
            serde_json::json!({"phrase": "valid", "match_mode": "compact", "reason": "ر".repeat(501)}),
            "reason",
            "too_long",
        ),
        (
            serde_json::json!({"match_mode": "compact", "reason": "review"}),
            "phrase",
            "required",
        ),
        (
            serde_json::json!({"phrase": "آب".repeat(60) + "آ", "match_mode": "compact", "reason": "review"}),
            "phrase",
            "too_long",
        ),
        (
            serde_json::json!({"phrase": "آ", "match_mode": "compact", "reason": "review"}),
            "phrase",
            "too_short",
        ),
        (
            serde_json::json!({"phrase": "valid", "reason": "review"}),
            "match_mode",
            "required",
        ),
        (
            serde_json::json!({"phrase": "valid", "match_mode": "regex", "reason": "review"}),
            "match_mode",
            "invalid",
        ),
    ];
    for (body, field, expected) in invalid {
        let (status, _, response) = create_rule(t.app.clone(), &admin, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert_field(&response, field, expected);
    }
    assert_eq!(rule_count(&t.pool).await, 0);
    assert_eq!(settings_revision(&t.pool).await, 1);

    let (status, _, two) = create_rule(
        t.app.clone(),
        &admin,
        serde_json::json!({"phrase": "آب", "match_mode": "compact", "reason": "review"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{two}");
    assert_rule_shape(&two);
    let (status, _, long) = create_rule(
        t.app.clone(),
        &admin,
        serde_json::json!({"phrase": "آب".repeat(60), "match_mode": "phrase", "reason": "review"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{long}");
    let (status, _, literal) = create_rule(
        t.app.clone(),
        &admin,
        serde_json::json!({"phrase": "e2e*%[literal]", "match_mode": "compact", "reason": "review"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{literal}");
    assert_eq!(literal["phrase"], "e2e*%[literal]");
    assert_eq!(rule_count(&t.pool).await, 3);
    assert_eq!(settings_revision(&t.pool).await, 4);

    let public_id = two["public_id"].as_str().expect("public ID");
    let (status, _, response) = patch_rule(
        t.app.clone(),
        &admin,
        public_id,
        serde_json::json!({"is_active": false}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    assert_field(&response, "reason", "required");
    let (status, _, response) = patch_rule(
        t.app.clone(),
        &admin,
        public_id,
        serde_json::json!({"phrase": long["phrase"].clone(), "reason": "duplicate edit"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    assert_field(&response, "phrase", "duplicate");
    assert_eq!(
        rule_state(&t.pool, public_id).await,
        ("آب".to_string(), true, 1)
    );
    assert_eq!(settings_revision(&t.pool).await, 4);
    assert_eq!(audit_count(&t.pool, "moderation_rule.update").await, 0);

    for body in [
        serde_json::json!({"reason": "review"}),
        serde_json::json!({"phrase": null, "match_mode": null, "is_active": null, "reason": "review"}),
    ] {
        let (status, _, response) = patch_rule(t.app.clone(), &admin, public_id, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert_field(&response, "patch", "no_changes");
    }
    let unknown = ulid::Ulid::new().to_string();
    let (status, _, response) = patch_rule(
        t.app.clone(),
        &admin,
        &unknown,
        serde_json::json!({"phrase": "new", "reason": "review"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{response}");
    assert_eq!(settings_revision(&t.pool).await, 4);
}

#[tokio::test]
async fn create_duplicates_toggles_noops_and_revisions_are_exact() {
    let Some(t) = setup().await else { return };
    clear_rules(&t.pool).await;
    let admin = create_admin(&t.pool, &t.cfg).await;
    let create = serde_json::json!({"phrase": "Foo", "match_mode": "compact", "reason": "initial"});
    let (status, _, created) = create_rule(t.app.clone(), &admin, create.clone()).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_rule_shape(&created);
    assert_eq!(created["is_active"], true);
    assert_eq!(created["revision"], 1);
    let public_id = created["public_id"].as_str().expect("public ID").to_string();
    assert_eq!(settings_revision(&t.pool).await, 2);
    assert_eq!(audit_count(&t.pool, "moderation_rule.create").await, 1);

    let (status, _, duplicate) = create_rule(
        t.app.clone(),
        &admin,
        serde_json::json!({"phrase": " Foo ", "match_mode": "compact", "reason": "retry"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{duplicate}");
    assert_eq!(duplicate["public_id"], public_id);
    assert_eq!(settings_revision(&t.pool).await, 2);
    assert_eq!(audit_count(&t.pool, "moderation_rule.create").await, 1);

    let (status, _, cross_mode) = create_rule(
        t.app.clone(),
        &admin,
        serde_json::json!({"phrase": "Foo", "match_mode": "phrase", "reason": "review"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{cross_mode}");
    assert_field(&cross_mode, "phrase", "duplicate");

    let (status, _, inactive) = patch_rule(
        t.app.clone(),
        &admin,
        &public_id,
        serde_json::json!({"is_active": false, "reason": "pause"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{inactive}");
    assert_eq!(inactive["is_active"], false);
    assert_eq!(inactive["revision"], 1);
    assert_eq!(settings_revision(&t.pool).await, 3);

    let (status, _, inactive_duplicate) = create_rule(t.app.clone(), &admin, create).await;
    assert_eq!(status, StatusCode::OK, "{inactive_duplicate}");
    assert_eq!(inactive_duplicate["is_active"], false);
    assert_eq!(settings_revision(&t.pool).await, 3);

    let (status, _, noop) = patch_rule(
        t.app.clone(),
        &admin,
        &public_id,
        serde_json::json!({"phrase": "Foo", "match_mode": "compact", "is_active": false, "reason": "noop"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{noop}");
    assert_eq!(settings_revision(&t.pool).await, 3);
    assert_eq!(audit_count(&t.pool, "moderation_rule.update").await, 1);

    let (status, _, phrase) = patch_rule(
        t.app.clone(),
        &admin,
        &public_id,
        serde_json::json!({"phrase": "Bar", "reason": "edit"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{phrase}");
    assert_eq!(phrase["revision"], 2);
    let (status, _, mode) = patch_rule(
        t.app.clone(),
        &admin,
        &public_id,
        serde_json::json!({"match_mode": "phrase", "reason": "mode"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{mode}");
    assert_eq!(mode["revision"], 3);
    let (status, _, active) = patch_rule(
        t.app.clone(),
        &admin,
        &public_id,
        serde_json::json!({"is_active": true, "reason": "resume"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{active}");
    assert_eq!(active["revision"], 3);
    assert_eq!(settings_revision(&t.pool).await, 6);
    assert_eq!(audit_count(&t.pool, "moderation_rule.update").await, 4);

    let metadata: String = sqlx::query_scalar(
        "SELECT metadata FROM admin_audit_logs WHERE action = 'moderation_rule.update' \
         ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&t.pool)
    .await
    .expect("update metadata");
    let metadata: serde_json::Value = serde_json::from_str(&metadata).expect("metadata JSON");
    assert_eq!(metadata.as_object().map(serde_json::Map::len), Some(4));
    assert_eq!(metadata["public_id"], public_id);
    assert_eq!(metadata["revision"], 3);
    assert_eq!(metadata["match_mode"], "phrase");
    assert_eq!(metadata["is_active"], true);
    assert!(!metadata.to_string().contains("Bar"));
}

#[tokio::test]
async fn list_filters_limits_and_cursor_survive_anchor_activation_change() {
    let Some(t) = setup().await else { return };
    clear_rules(&t.pool).await;
    let admin = create_admin(&t.pool, &t.cfg).await;
    let created_at = chrono::Utc::now();
    for index in 0..55 {
        insert_rule_fixture(&t.pool, &format!("rule{index}"), true, created_at).await;
    }

    let (status, _, default) = list_rules(t.app.clone(), &admin, "").await;
    assert_eq!(status, StatusCode::OK, "{default}");
    assert_eq!(default["data"].as_array().map(Vec::len), Some(20));
    assert_eq!(default["page"]["has_more"], true);
    for rule in default["data"].as_array().expect("default data") {
        assert_rule_shape(rule);
    }
    let expected_second = default["data"][1]["public_id"].clone();
    let (status, _, capped) = list_rules(t.app.clone(), &admin, "?active=all&limit=100").await;
    assert_eq!(status, StatusCode::OK, "{capped}");
    assert_eq!(capped["data"].as_array().map(Vec::len), Some(50));
    assert_eq!(capped["page"]["has_more"], true);

    let (status, _, first) = list_rules(t.app.clone(), &admin, "?active=active&limit=1").await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let anchor = first["data"][0]["public_id"].as_str().expect("anchor").to_string();
    let cursor = first["page"]["next_cursor"].as_str().expect("cursor").to_string();
    let (status, _, _) = patch_rule(
        t.app.clone(),
        &admin,
        &anchor,
        serde_json::json!({"is_active": false, "reason": "filter fixture"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let encoded_cursor = cursor.replace(':', "%3A");
    let (status, _, continued) = list_rules(
        t.app.clone(),
        &admin,
        &format!("?active=active&limit=1&cursor={encoded_cursor}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{continued}");
    assert_eq!(continued["data"][0]["public_id"], expected_second);

    let (_, _, active) = list_rules(t.app.clone(), &admin, "?active=active&limit=100").await;
    assert!(active["data"]
        .as_array()
        .expect("active data")
        .iter()
        .all(|rule| rule["is_active"] == true));
    let (_, _, inactive) = list_rules(t.app.clone(), &admin, "?active=inactive&limit=100").await;
    assert_eq!(inactive["data"].as_array().map(Vec::len), Some(1));
    assert_eq!(inactive["data"][0]["public_id"], anchor);

    let (status, _, invalid) = list_rules(t.app.clone(), &admin, "?active=sometimes").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}");
    assert_field(&invalid, "active", "invalid");
    let (status, _, invalid) = list_rules(t.app.clone(), &admin, "?cursor=broken").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}");
    assert_field(&invalid, "cursor", "invalid_cursor");
}

#[tokio::test]
async fn concurrent_identical_creates_write_one_row_revision_and_audit() {
    let Some(t) = setup().await else { return };
    clear_rules(&t.pool).await;
    let admin = create_admin(&t.pool, &t.cfg).await;
    let body = serde_json::json!({"phrase": "Concurrent", "match_mode": "compact", "reason": "race"});
    let first = create_rule(t.app.clone(), &admin, body.clone());
    let second = create_rule(t.app.clone(), &admin, body);
    let (first, second) = tokio::join!(first, second);
    assert_eq!(first.0, StatusCode::OK, "{:?}", first.2);
    assert_eq!(second.0, StatusCode::OK, "{:?}", second.2);
    assert_eq!(first.2["public_id"], second.2["public_id"]);
    assert_eq!(rule_count(&t.pool).await, 1);
    assert_eq!(settings_revision(&t.pool).await, 2);
    assert_eq!(audit_count(&t.pool, "moderation_rule.create").await, 1);
}

#[tokio::test]
async fn concurrent_edits_and_noops_preserve_both_changes_without_extra_audits() {
    let Some(t) = setup().await else { return };
    clear_rules(&t.pool).await;
    let admin = create_admin(&t.pool, &t.cfg).await;
    let (_, _, created) = create_rule(
        t.app.clone(),
        &admin,
        serde_json::json!({"phrase": "Original", "match_mode": "compact", "reason": "initial"}),
    )
    .await;
    let public_id = created["public_id"].as_str().expect("public ID").to_string();

    let deactivate = patch_rule(
        t.app.clone(),
        &admin,
        &public_id,
        serde_json::json!({"is_active": false, "reason": "deactivate"}),
    );
    let rename = patch_rule(
        t.app.clone(),
        &admin,
        &public_id,
        serde_json::json!({"phrase": "Changed", "reason": "rename"}),
    );
    let (deactivated, renamed) = tokio::join!(deactivate, rename);
    assert_eq!(deactivated.0, StatusCode::OK, "{:?}", deactivated.2);
    assert_eq!(renamed.0, StatusCode::OK, "{:?}", renamed.2);
    assert_eq!(
        rule_state(&t.pool, &public_id).await,
        ("Changed".to_string(), false, 2)
    );
    assert_eq!(settings_revision(&t.pool).await, 4);
    assert_eq!(audit_count(&t.pool, "moderation_rule.update").await, 2);

    let noop_body = serde_json::json!({"is_active": false, "reason": "retry"});
    let first = patch_rule(t.app.clone(), &admin, &public_id, noop_body.clone());
    let second = patch_rule(t.app.clone(), &admin, &public_id, noop_body);
    let (first, second) = tokio::join!(first, second);
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(second.0, StatusCode::OK);
    assert_eq!(settings_revision(&t.pool).await, 4);
    assert_eq!(audit_count(&t.pool, "moderation_rule.update").await, 2);

    let rows = sqlx::query("SELECT metadata FROM admin_audit_logs WHERE action = 'moderation_rule.update'")
        .fetch_all(&t.pool)
        .await
        .expect("audit metadata");
    for row in rows {
        let metadata: String = row.try_get("metadata").expect("metadata");
        assert!(!metadata.contains("Changed"));
        assert!(!metadata.contains("Original"));
    }
}
