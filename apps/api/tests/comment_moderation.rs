//! Phase 10 automatic-comment review queue, decisions, and report interop.

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

struct ListingFixture {
    id: u64,
    slug: String,
}

struct CommentFixture {
    id: u64,
    public_id: String,
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
    sqlx::query("DELETE FROM moderation_rules")
        .execute(pool)
        .await
        .expect("clear seed rules");
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
    .bind(&public_id)
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
        public_id,
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

async fn create_listing(pool: &MySqlPool, seller_id: u64) -> ListingFixture {
    let public_id = ulid::Ulid::new().to_string();
    let slug = format!("listing-{}", ulid::Ulid::new());
    let result = sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, published_at, created_at, updated_at) \
         VALUES (?, ?, 1, 1, 'fixture', ?, 'description', 1000, 'fixed', 'active', \
         NOW(3), NOW(3), NOW(3))",
    )
    .bind(public_id)
    .bind(seller_id)
    .bind(&slug)
    .execute(pool)
    .await
    .expect("listing fixture");
    ListingFixture {
        id: result.last_insert_id(),
        slug,
    }
}

async fn insert_comment(
    pool: &MySqlPool,
    listing_id: u64,
    author_id: u64,
    body: &str,
    status: &str,
    deleted: bool,
    created_at: chrono::DateTime<chrono::Utc>,
) -> CommentFixture {
    let public_id = ulid::Ulid::new().to_string();
    let result = sqlx::query(
        "INSERT INTO listing_comments \
         (public_id, listing_id, author_id, body, status, created_at, updated_at, deleted_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, IF(?, ?, NULL))",
    )
    .bind(&public_id)
    .bind(listing_id)
    .bind(author_id)
    .bind(body)
    .bind(status)
    .bind(created_at)
    .bind(created_at)
    .bind(deleted)
    .bind(created_at)
    .execute(pool)
    .await
    .expect("comment fixture");
    if status == "visible" && !deleted {
        sqlx::query("UPDATE listings SET comment_count = comment_count + 1 WHERE id = ?")
            .bind(listing_id)
            .execute(pool)
            .await
            .expect("counter fixture");
    }
    CommentFixture {
        id: result.last_insert_id(),
        public_id,
    }
}

fn snapshots(count: usize) -> serde_json::Value {
    serde_json::Value::Array(
        (0..count)
            .map(|index| {
                serde_json::json!({
                    "public_id": ulid::Ulid::new().to_string(),
                    "revision": index + 1,
                    "phrase": format!("<script>rule {index}</script>"),
                    "match_mode": if index % 2 == 0 { "compact" } else { "phrase" },
                })
            })
            .collect(),
    )
}

async fn insert_case(
    pool: &MySqlPool,
    comment_id: u64,
    status: &str,
    matched_rules: &serde_json::Value,
    flagged_at: chrono::DateTime<chrono::Utc>,
) -> u64 {
    let result = sqlx::query(
        "INSERT INTO comment_moderation_cases \
         (comment_id, status, matched_rules, flagged_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(comment_id)
    .bind(status)
    .bind(serde_json::to_string(matched_rules).expect("snapshot JSON"))
    .bind(flagged_at)
    .bind(flagged_at)
    .execute(pool)
    .await
    .expect("case fixture");
    result.last_insert_id()
}

async fn insert_report(pool: &MySqlPool, reporter_id: u64, comment_id: u64) -> String {
    let public_id = ulid::Ulid::new().to_string();
    sqlx::query(
        "INSERT INTO reports (public_id, reporter_id, target_type, target_id, reason, created_at) \
         VALUES (?, ?, 'comment', ?, 'spam', NOW(3))",
    )
    .bind(&public_id)
    .bind(reporter_id)
    .bind(comment_id)
    .execute(pool)
    .await
    .expect("report fixture");
    public_id
}

async fn send(
    app: Router,
    method: Method,
    uri: &str,
    user: Option<&TestUser>,
    csrf: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(user) = user {
        builder = builder.header("cookie", &user.cookie);
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

async fn queue(
    app: Router,
    user: Option<&TestUser>,
    query: &str,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::GET,
        &format!("/api/v1/admin/moderation/comments{query}"),
        user,
        None,
        None,
    )
    .await
}

async fn decide(
    app: Router,
    user: Option<&TestUser>,
    csrf: Option<&str>,
    comment_id: &str,
    body: serde_json::Value,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::POST,
        &format!("/api/v1/admin/moderation/comments/{comment_id}/actions"),
        user,
        csrf,
        Some(body),
    )
    .await
}

async fn action(
    app: Router,
    admin: &TestUser,
    comment_id: &str,
    action: &str,
    reason: Option<&str>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    decide(
        app,
        Some(admin),
        Some(&admin.csrf),
        comment_id,
        serde_json::json!({"action": action, "reason": reason}),
    )
    .await
}

async fn report_action(
    app: Router,
    admin: &TestUser,
    report_id: &str,
    action: &str,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::POST,
        &format!("/api/v1/admin/reports/{report_id}/actions"),
        Some(admin),
        Some(&admin.csrf),
        Some(serde_json::json!({"action": action, "reason": "report review"})),
    )
    .await
}

async fn delete_comment(app: Router, user: &TestUser, comment_id: &str) -> StatusCode {
    send(
        app,
        Method::DELETE,
        &format!("/api/v1/comments/{comment_id}"),
        Some(user),
        Some(&user.csrf),
        None,
    )
    .await
    .0
}

async fn create_comment(app: Router, user: &TestUser, slug: &str, body: &str) -> StatusCode {
    send(
        app,
        Method::POST,
        &format!("/api/v1/listings/{slug}/comments"),
        Some(user),
        Some(&user.csrf),
        Some(serde_json::json!({"body": body})),
    )
    .await
    .0
}

fn assert_keys(value: &serde_json::Value, expected: &[&str]) {
    let object = value.as_object().expect("object");
    let mut actual = object.keys().map(String::as_str).collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "object keys: {value}");
}

fn assert_item_shape(item: &serde_json::Value) {
    assert_keys(
        item,
        &["comment", "status", "flagged_at", "matched_rules", "handled_at", "handled_by"],
    );
    assert_keys(
        &item["comment"],
        &["public_id", "body", "status", "deleted", "created_at", "author", "listing_slug"],
    );
    assert_keys(&item["comment"]["author"], &["public_id", "display_name"]);
    if !item["handled_by"].is_null() {
        assert_keys(&item["handled_by"], &["public_id", "display_name"]);
    }
    for rule in item["matched_rules"].as_array().expect("matched rules") {
        assert_keys(rule, &["public_id", "revision", "phrase", "match_mode"]);
    }
    assert!(item.get("cleared_rules").is_none());
    assert!(item.get("id").is_none());
    assert!(item.get("comment_id").is_none());
    assert!(!item.to_string().contains("phone_e164"));
    assert!(!item.to_string().contains("reporter"));
}

async fn assert_counter(pool: &MySqlPool, listing_id: u64) -> u32 {
    let counter: u32 = sqlx::query_scalar("SELECT comment_count FROM listings WHERE id = ?")
        .bind(listing_id)
        .fetch_one(pool)
        .await
        .expect("unsigned counter");
    let visible: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM listing_comments \
         WHERE listing_id = ? AND status = 'visible' AND deleted_at IS NULL",
    )
    .bind(listing_id)
    .fetch_one(pool)
    .await
    .expect("visible rows");
    assert_eq!(i64::from(counter), visible);
    counter
}

async fn audit_count(pool: &MySqlPool, action: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs WHERE action = ?")
        .bind(action)
        .fetch_one(pool)
        .await
        .expect("audit count")
}

async fn case_status(pool: &MySqlPool, comment_id: u64) -> String {
    sqlx::query_scalar("SELECT status FROM comment_moderation_cases WHERE comment_id = ?")
        .bind(comment_id)
        .fetch_one(pool)
        .await
        .expect("case status")
}

async fn clearances(pool: &MySqlPool, comment_id: u64) -> serde_json::Value {
    let bytes: Vec<u8> = sqlx::query_scalar(
        "SELECT cleared_rules FROM comment_moderation_cases WHERE comment_id = ?",
    )
    .bind(comment_id)
    .fetch_one(pool)
    .await
    .expect("clearances");
    serde_json::from_slice(&bytes).expect("clearance JSON")
}

#[tokio::test]
async fn queue_requires_admin_private_cache_and_exact_safe_shapes() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let non_admin = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, author.id).await;
    let now = chrono::Utc::now();
    let comment = insert_comment(
        &t.pool,
        listing.id,
        author.id,
        "<img src=x onerror=alert(1)>",
        "hidden",
        false,
        now,
    )
    .await;
    insert_case(&t.pool, comment.id, "pending", &snapshots(1), now).await;

    assert_eq!(queue(t.app.clone(), None, "").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        queue(t.app.clone(), Some(&non_admin), "").await.0,
        StatusCode::FORBIDDEN
    );
    let (status, headers, body) = queue(t.app.clone(), Some(&admin), "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        headers.get("cache-control").and_then(|value| value.to_str().ok()),
        Some("private, no-store")
    );
    assert_eq!(headers.get("vary").and_then(|value| value.to_str().ok()), Some("Cookie"));
    assert_eq!(body["data"].as_array().map(Vec::len), Some(1));
    let item = &body["data"][0];
    assert_item_shape(item);
    assert_eq!(item["status"], "pending");
    assert_eq!(item["comment"]["public_id"], comment.public_id);
    assert_eq!(item["comment"]["body"], "<img src=x onerror=alert(1)>");
    assert_eq!(item["comment"]["status"], "hidden");
    assert_eq!(item["comment"]["deleted"], false);
    assert_eq!(item["comment"]["listing_slug"], listing.slug);
    assert_eq!(item["comment"]["author"]["public_id"], author.public_id);
    assert_eq!(item["matched_rules"][0]["phrase"], "<script>rule 0</script>");
    assert!(item["handled_by"].is_null());
    assert!(item["handled_at"].is_null());
    for (query, field) in [("?status=bad", "status"), ("?cursor=bad", "cursor")] {
        let (status, _, response) = queue(t.app.clone(), Some(&admin), query).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(response["error"]["fields"][field].is_string());
    }
}

#[tokio::test]
async fn queue_filters_limits_and_cursor_survive_anchor_decisions() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, author.id).await;
    let now = chrono::Utc::now();
    let snapshot = snapshots(1);
    let mut comments = Vec::new();
    for _ in 0..55 {
        let comment = insert_comment(
            &t.pool,
            listing.id,
            author.id,
            "queued",
            "hidden",
            false,
            now,
        )
        .await;
        insert_case(&t.pool, comment.id, "pending", &snapshot, now).await;
        comments.push(comment);
    }
    let (_, _, default) = queue(t.app.clone(), Some(&admin), "").await;
    assert_eq!(default["data"].as_array().map(Vec::len), Some(20));
    assert_eq!(default["page"]["has_more"], true);
    assert_eq!(default["data"][0]["comment"]["public_id"], comments[0].public_id);
    let expected_second = default["data"][1]["comment"]["public_id"].clone();
    let (_, _, capped) = queue(t.app.clone(), Some(&admin), "?status=all&limit=100").await;
    assert_eq!(capped["data"].as_array().map(Vec::len), Some(50));
    assert_eq!(capped["page"]["has_more"], true);

    let (_, _, first) = queue(t.app.clone(), Some(&admin), "?limit=1").await;
    let cursor = first["page"]["next_cursor"].as_str().expect("cursor").to_string();
    let anchor = first["data"][0]["comment"]["public_id"].as_str().expect("anchor").to_string();
    assert_eq!(
        action(t.app.clone(), &admin, &anchor, "confirm_hidden", Some("review"))
            .await
            .0,
        StatusCode::OK
    );
    let encoded = cursor.replace(':', "%3A");
    let (_, _, continued) = queue(
        t.app.clone(),
        Some(&admin),
        &format!("?limit=1&cursor={encoded}"),
    )
    .await;
    assert_eq!(continued["data"][0]["comment"]["public_id"], expected_second);
    let (_, _, confirmed) = queue(t.app.clone(), Some(&admin), "?status=confirmed_hidden").await;
    assert_eq!(confirmed["data"].as_array().map(Vec::len), Some(1));
    assert_eq!(confirmed["data"][0]["comment"]["public_id"], anchor);

    assert_eq!(
        action(t.app.clone(), &admin, &anchor, "restore", None).await.0,
        StatusCode::OK
    );
    let (_, _, restored) = queue(t.app.clone(), Some(&admin), "?status=restored").await;
    assert_eq!(restored["data"].as_array().map(Vec::len), Some(1));
    assert_eq!(restored["data"][0]["comment"]["public_id"], anchor);
    let (_, _, confirmed) = queue(t.app.clone(), Some(&admin), "?status=confirmed_hidden").await;
    assert_eq!(confirmed["data"].as_array().map(Vec::len), Some(0));
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
}

#[tokio::test]
async fn confirm_restore_reversal_deleted_and_idempotency_are_exact() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, author.id).await;
    let now = chrono::Utc::now();
    insert_comment(&t.pool, listing.id, author.id, "visible", "visible", false, now).await;
    let comment = insert_comment(&t.pool, listing.id, author.id, "pending", "hidden", false, now).await;
    let matched = snapshots(2);
    insert_case(&t.pool, comment.id, "pending", &matched, now).await;
    let ordinary = insert_comment(&t.pool, listing.id, author.id, "ordinary", "visible", false, now).await;

    assert_eq!(
        decide(t.app.clone(), Some(&admin), None, &comment.public_id, serde_json::json!({"action":"confirm_hidden","reason":"x"}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    for (body, field, expected) in [
        (serde_json::json!({}), "action", "required"),
        (serde_json::json!({"action":"explode"}), "action", "invalid"),
        (serde_json::json!({"action":"confirm_hidden"}), "reason", "required"),
        (serde_json::json!({"action":"confirm_hidden","reason":" "}), "reason", "required"),
        (serde_json::json!({"action":"confirm_hidden","reason":"x".repeat(501)}), "reason", "too_long"),
        (serde_json::json!({"action":"restore","reason":"x".repeat(501)}), "reason", "too_long"),
    ] {
        let (status, _, response) = decide(
            t.app.clone(),
            Some(&admin),
            Some(&admin.csrf),
            &comment.public_id,
            body,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert_eq!(response["error"]["fields"][field], expected);
    }
    assert_eq!(
        action(t.app.clone(), &admin, &ordinary.public_id, "confirm_hidden", Some("x"))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        action(t.app.clone(), &admin, &"0".repeat(26), "confirm_hidden", Some("x"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(audit_count(&t.pool, "comment.filter_confirm_hidden").await, 0);

    let reason = "ر".repeat(500);
    let (status, _, confirmed) = action(
        t.app.clone(),
        &admin,
        &comment.public_id,
        "confirm_hidden",
        Some(&reason),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{confirmed}");
    assert_item_shape(&confirmed);
    assert_eq!(confirmed["status"], "confirmed_hidden");
    assert_eq!(confirmed["comment"]["status"], "hidden");
    assert_eq!(confirmed["handled_by"]["public_id"], admin.public_id);
    assert!(confirmed["handled_at"].is_string());
    assert_eq!(assert_counter(&t.pool, listing.id).await, 2);
    assert_eq!(audit_count(&t.pool, "comment.filter_confirm_hidden").await, 1);
    let (status, _, repeated) = action(
        t.app.clone(),
        &admin,
        &comment.public_id,
        "confirm_hidden",
        Some("ignored"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated, confirmed);
    assert_eq!(audit_count(&t.pool, "comment.filter_confirm_hidden").await, 1);

    let (status, _, restored) = action(t.app.clone(), &admin, &comment.public_id, "restore", None).await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["status"], "restored");
    assert_eq!(restored["comment"]["status"], "visible");
    assert_eq!(assert_counter(&t.pool, listing.id).await, 3);
    assert_eq!(audit_count(&t.pool, "comment.filter_restore").await, 1);
    let expected_clearances = serde_json::json!([
        {"public_id": matched[0]["public_id"], "revision": matched[0]["revision"]},
        {"public_id": matched[1]["public_id"], "revision": matched[1]["revision"]}
    ]);
    assert_eq!(clearances(&t.pool, comment.id).await, expected_clearances);
    let (status, _, repeated_restore) = action(t.app.clone(), &admin, &comment.public_id, "restore", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated_restore, restored);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 3);
    assert_eq!(audit_count(&t.pool, "comment.filter_restore").await, 1);
    let (status, _, response) = action(
        t.app.clone(),
        &admin,
        &comment.public_id,
        "confirm_hidden",
        Some("invalid reversal"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response["error"]["fields"]["status"], "invalid_transition");

    let audits = sqlx::query(
        "SELECT action, target_type, reason, CAST(metadata AS CHAR) AS metadata \
         FROM admin_audit_logs ORDER BY id ASC",
    )
    .fetch_all(&t.pool)
    .await
    .expect("decision audits");
    assert_eq!(audits.len(), 2);
    for row in audits {
        assert_eq!(row.try_get::<String, _>("target_type").expect("target"), "comment");
        let metadata: String = row.try_get("metadata").expect("metadata");
        let metadata: serde_json::Value = serde_json::from_str(&metadata).expect("metadata JSON");
        assert_eq!(metadata, serde_json::json!({"source":"automatic_profanity"}));
    }

    let deleted = insert_comment(&t.pool, listing.id, author.id, "deleted", "hidden", true, now).await;
    insert_case(&t.pool, deleted.id, "pending", &snapshots(1), now).await;
    assert_eq!(
        action(t.app.clone(), &admin, &deleted.public_id, "confirm_hidden", Some("close"))
            .await
            .0,
        StatusCode::OK,
        "deleted pending cases can be closed"
    );
    assert_eq!(case_status(&t.pool, deleted.id).await, "confirmed_hidden");
    let (status, _, response) = action(t.app.clone(), &admin, &deleted.public_id, "restore", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response["error"]["fields"]["status"], "invalid_transition");
    assert_eq!(assert_counter(&t.pool, listing.id).await, 3);
    assert_eq!(audit_count(&t.pool, "comment.filter_confirm_hidden").await, 2);
    assert_eq!(audit_count(&t.pool, "comment.filter_restore").await, 1);
    assert_eq!(
        action(t.app.clone(), &admin, &deleted.public_id, "confirm_hidden", Some("retry"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(audit_count(&t.pool, "comment.filter_confirm_hidden").await, 2);
}

#[tokio::test]
async fn report_status_and_restore_guard_respect_automatic_cases() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let first_reporter = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let second_reporter = create_user(&t.pool, &t.cfg, "+989100000004").await;
    let listing = create_listing(&t.pool, author.id).await;
    let now = chrono::Utc::now();
    let comment = insert_comment(&t.pool, listing.id, author.id, "reported", "visible", false, now).await;
    let first_report = insert_report(&t.pool, first_reporter.id, comment.id).await;

    let (status, _, hidden) = report_action(t.app.clone(), &admin, &first_report, "hide_comment").await;
    assert_eq!(status, StatusCode::OK, "{hidden}");
    assert_eq!(hidden["target"]["moderation_status"], serde_json::Value::Null);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 0);
    insert_case(&t.pool, comment.id, "pending", &snapshots(1), now).await;
    let (status, _, current) = report_action(t.app.clone(), &admin, &first_report, "hide_comment").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(current["target"]["moderation_status"], "pending");
    for _ in 0..2 {
        let (status, _, response) = report_action(t.app.clone(), &admin, &first_report, "restore_comment").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert_eq!(response["error"]["fields"]["status"], "invalid_transition");
        assert_eq!(assert_counter(&t.pool, listing.id).await, 0);
    }
    assert_eq!(audit_count(&t.pool, "report.restore_comment").await, 0);

    assert_eq!(
        action(t.app.clone(), &admin, &comment.public_id, "confirm_hidden", Some("automatic"))
            .await
            .0,
        StatusCode::OK
    );
    let (status, _, current) = report_action(t.app.clone(), &admin, &first_report, "hide_comment").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(current["target"]["moderation_status"], "confirmed_hidden");
    let (status, _, restored) = action(t.app.clone(), &admin, &comment.public_id, "restore", None).await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
    let automatic_clearances = clearances(&t.pool, comment.id).await;

    let second_report = insert_report(&t.pool, second_reporter.id, comment.id).await;
    let (status, _, hidden) = report_action(t.app.clone(), &admin, &second_report, "hide_comment").await;
    assert_eq!(status, StatusCode::OK, "{hidden}");
    assert_eq!(hidden["target"]["moderation_status"], "restored");
    assert_eq!(case_status(&t.pool, comment.id).await, "restored");
    assert_eq!(assert_counter(&t.pool, listing.id).await, 0);
    let (status, _, restored) = report_action(t.app.clone(), &admin, &second_report, "restore_comment").await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["target"]["moderation_status"], "restored");
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
    assert_eq!(clearances(&t.pool, comment.id).await, automatic_clearances);
    assert_eq!(audit_count(&t.pool, "report.hide_comment").await, 2);
    assert_eq!(audit_count(&t.pool, "report.restore_comment").await, 1);
    assert_eq!(audit_count(&t.pool, "comment.filter_confirm_hidden").await, 1);
    assert_eq!(audit_count(&t.pool, "comment.filter_restore").await, 1);
}

#[tokio::test]
async fn concurrent_filter_decisions_are_idempotent_and_counter_exact() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, author.id).await;
    let now = chrono::Utc::now();
    insert_comment(&t.pool, listing.id, author.id, "visible", "visible", false, now).await;
    let comment = insert_comment(&t.pool, listing.id, author.id, "pending", "hidden", false, now).await;
    insert_case(&t.pool, comment.id, "pending", &snapshots(1), now).await;

    let (first, second) = tokio::join!(
        action(t.app.clone(), &admin, &comment.public_id, "confirm_hidden", Some("one")),
        action(t.app.clone(), &admin, &comment.public_id, "confirm_hidden", Some("two")),
    );
    assert_eq!(first.0, StatusCode::OK, "{:?}", first.2);
    assert_eq!(second.0, StatusCode::OK, "{:?}", second.2);
    assert_eq!(audit_count(&t.pool, "comment.filter_confirm_hidden").await, 1);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);

    let (first, second) = tokio::join!(
        action(t.app.clone(), &admin, &comment.public_id, "restore", None),
        action(t.app.clone(), &admin, &comment.public_id, "restore", None),
    );
    assert_eq!(first.0, StatusCode::OK, "{:?}", first.2);
    assert_eq!(second.0, StatusCode::OK, "{:?}", second.2);
    assert_eq!(first.2, second.2);
    assert_eq!(audit_count(&t.pool, "comment.filter_restore").await, 1);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 2);
    assert_eq!(case_status(&t.pool, comment.id).await, "restored");

    let (first, second) = tokio::join!(
        action(t.app.clone(), &admin, &comment.public_id, "confirm_hidden", Some("bad")),
        action(t.app.clone(), &admin, &comment.public_id, "confirm_hidden", Some("bad")),
    );
    assert_eq!(first.0, StatusCode::BAD_REQUEST);
    assert_eq!(second.0, StatusCode::BAD_REQUEST);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 2);
}

#[tokio::test]
async fn concurrent_create_delete_report_and_filter_actions_never_corrupt_counters() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let other = create_user(&t.pool, &t.cfg, "+989100000004").await;
    let listing = create_listing(&t.pool, author.id).await;
    let now = chrono::Utc::now();
    let comment = insert_comment(&t.pool, listing.id, author.id, "pending", "hidden", false, now).await;
    insert_case(&t.pool, comment.id, "pending", &snapshots(1), now).await;
    let report = insert_report(&t.pool, reporter.id, comment.id).await;

    let (
        created,
        deleted,
        report_hidden,
        report_restored,
        confirmed,
        filter_restored,
    ) = tokio::join!(
        create_comment(t.app.clone(), &other, &listing.slug, "new clean"),
        delete_comment(t.app.clone(), &author, &comment.public_id),
        report_action(t.app.clone(), &admin, &report, "hide_comment"),
        report_action(t.app.clone(), &admin, &report, "restore_comment"),
        action(t.app.clone(), &admin, &comment.public_id, "confirm_hidden", Some("race")),
        action(t.app.clone(), &admin, &comment.public_id, "restore", None),
    );
    assert_eq!(created, StatusCode::OK);
    assert_eq!(deleted, StatusCode::OK);
    for status in [report_hidden.0, report_restored.0, confirmed.0, filter_restored.0] {
        assert!(status == StatusCode::OK || status == StatusCode::BAD_REQUEST, "unexpected {status}");
    }
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT deleted_at FROM listing_comments WHERE id = ?",
    )
    .bind(comment.id)
    .fetch_one(&t.pool)
    .await
    .expect("deleted target");
    assert!(deleted_at.is_some());
    for action in [
        "comment.filter_confirm_hidden",
        "comment.filter_restore",
        "report.hide_comment",
        "report.restore_comment",
    ] {
        assert!(audit_count(&t.pool, action).await <= 1, "duplicate {action}");
    }
    let (_, _, page) = send(
        t.app.clone(),
        Method::GET,
        &format!("/api/v1/listings/{}/comments", listing.slug),
        None,
        None,
        None,
    )
    .await;
    let items = page["data"].as_array().expect("public comments");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["body"], "new clean");
}
