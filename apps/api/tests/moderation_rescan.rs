//! Phase 10 revision-safe existing-comment rescan batches.

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

struct TestUser {
    id: u64,
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

fn comment_public_id(number: u64) -> String {
    format!("{number:026}")
}

async fn insert_comment(
    pool: &MySqlPool,
    listing_id: u64,
    author_id: u64,
    number: u64,
    body: &str,
    status: &str,
    deleted: bool,
) -> CommentFixture {
    let public_id = comment_public_id(number);
    let result = sqlx::query(
        "INSERT INTO listing_comments \
         (public_id, listing_id, author_id, body, status, created_at, updated_at, deleted_at) \
         VALUES (?, ?, ?, ?, ?, NOW(3), NOW(3), IF(?, NOW(3), NULL))",
    )
    .bind(&public_id)
    .bind(listing_id)
    .bind(author_id)
    .bind(body)
    .bind(status)
    .bind(deleted)
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

fn snapshot(rule: &serde_json::Value) -> serde_json::Value {
    serde_json::json!([{
        "public_id": rule["public_id"],
        "revision": rule["revision"],
        "phrase": rule["phrase"],
        "match_mode": rule["match_mode"],
    }])
}

async fn insert_case(
    pool: &MySqlPool,
    comment_id: u64,
    status: &str,
    matched_rules: &serde_json::Value,
    cleared_rules: Option<&serde_json::Value>,
    handled_by: Option<u64>,
) {
    let flagged_at = chrono::Utc::now() - chrono::Duration::hours(1);
    sqlx::query(
        "INSERT INTO comment_moderation_cases \
         (comment_id, status, matched_rules, cleared_rules, flagged_at, updated_at, handled_by, handled_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, IF(?, ?, NULL))",
    )
    .bind(comment_id)
    .bind(status)
    .bind(serde_json::to_string(matched_rules).expect("matched JSON"))
    .bind(cleared_rules.map(|value| serde_json::to_string(value).expect("clearance JSON")))
    .bind(flagged_at)
    .bind(flagged_at)
    .bind(handled_by)
    .bind(handled_by.is_some())
    .bind(flagged_at)
    .execute(pool)
    .await
    .expect("case fixture");
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

async fn rescan(
    app: Router,
    user: Option<&TestUser>,
    csrf: Option<&str>,
    body: serde_json::Value,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::POST,
        "/api/v1/admin/moderation/rescan",
        user,
        csrf,
        Some(body),
    )
    .await
}

async fn new_rescan(
    app: Router,
    admin: &TestUser,
    limit: serde_json::Value,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    rescan(
        app,
        Some(admin),
        Some(&admin.csrf),
        serde_json::json!({"cursor": null, "rules_revision": null, "limit": limit}),
    )
    .await
}

async fn continuation(
    app: Router,
    admin: &TestUser,
    cursor: &str,
    revision: u64,
    limit: u64,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    rescan(
        app,
        Some(admin),
        Some(&admin.csrf),
        serde_json::json!({"cursor": cursor, "rules_revision": revision, "limit": limit}),
    )
    .await
}

async fn create_rule(
    app: Router,
    admin: &TestUser,
    phrase: &str,
    match_mode: &str,
) -> serde_json::Value {
    let (status, _, body) = send(
        app,
        Method::POST,
        "/api/v1/admin/moderation/rules",
        Some(admin),
        Some(&admin.csrf),
        Some(serde_json::json!({
            "phrase": phrase,
            "match_mode": match_mode,
            "reason": "rescan fixture",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn patch_rule(
    app: Router,
    admin: &TestUser,
    public_id: &str,
    patch: serde_json::Value,
) -> serde_json::Value {
    let mut body = patch.as_object().expect("patch object").clone();
    body.insert("reason".to_string(), serde_json::json!("revision fixture"));
    let (status, _, response) = send(
        app,
        Method::PATCH,
        &format!("/api/v1/admin/moderation/rules/{public_id}"),
        Some(admin),
        Some(&admin.csrf),
        Some(serde_json::Value::Object(body)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    response
}

async fn delete_comment(app: Router, user: &TestUser, public_id: &str) -> StatusCode {
    send(
        app,
        Method::DELETE,
        &format!("/api/v1/comments/{public_id}"),
        Some(user),
        Some(&user.csrf),
        None,
    )
    .await
    .0
}

async fn report_action(
    app: Router,
    admin: &TestUser,
    public_id: &str,
    action: &str,
) -> StatusCode {
    send(
        app,
        Method::POST,
        &format!("/api/v1/admin/reports/{public_id}/actions"),
        Some(admin),
        Some(&admin.csrf),
        Some(serde_json::json!({"action": action, "reason": "race"})),
    )
    .await
    .0
}

async fn filter_restore(app: Router, admin: &TestUser, public_id: &str) -> StatusCode {
    send(
        app,
        Method::POST,
        &format!("/api/v1/admin/moderation/comments/{public_id}/actions"),
        Some(admin),
        Some(&admin.csrf),
        Some(serde_json::json!({"action": "restore"})),
    )
    .await
    .0
}

fn assert_response_shape(body: &serde_json::Value) {
    let mut keys = body
        .as_object()
        .expect("response object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["flagged_count", "has_more", "next_cursor", "rules_revision", "scanned_count"]
    );
}

async fn current_revision(pool: &MySqlPool) -> u64 {
    sqlx::query_scalar("SELECT revision FROM moderation_settings WHERE id = 1")
        .fetch_one(pool)
        .await
        .expect("settings revision")
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

async fn case_row(
    pool: &MySqlPool,
    comment_id: u64,
) -> (String, serde_json::Value, Option<serde_json::Value>, Option<u64>) {
    let row = sqlx::query(
        "SELECT status, matched_rules, cleared_rules, handled_by \
         FROM comment_moderation_cases WHERE comment_id = ?",
    )
    .bind(comment_id)
    .fetch_one(pool)
    .await
    .expect("case row");
    let matched: Vec<u8> = row.try_get("matched_rules").expect("matched bytes");
    let cleared: Option<Vec<u8>> = row.try_get("cleared_rules").expect("cleared bytes");
    (
        row.try_get("status").expect("case status"),
        serde_json::from_slice(&matched).expect("matched JSON"),
        cleared.map(|bytes| serde_json::from_slice(&bytes).expect("cleared JSON")),
        row.try_get("handled_by").expect("handler"),
    )
}

async fn case_count(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM comment_moderation_cases")
        .fetch_one(pool)
        .await
        .expect("case count")
}

#[tokio::test]
async fn rescan_requires_admin_csrf_pairing_cursor_and_integer_limit() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let non_admin = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, non_admin.id).await;
    let comment = insert_comment(&t.pool, listing.id, non_admin.id, 1, "clean", "visible", false).await;
    let body = serde_json::json!({"cursor": null, "rules_revision": null, "limit": 50});
    assert_eq!(
        rescan(t.app.clone(), None, None, body.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        rescan(t.app.clone(), Some(&non_admin), Some(&non_admin.csrf), body.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        rescan(t.app.clone(), Some(&admin), None, body).await.0,
        StatusCode::FORBIDDEN
    );

    for (body, field, expected) in [
        (serde_json::json!({"cursor": comment.public_id}), "rules_revision", "required"),
        (serde_json::json!({"rules_revision": 1}), "cursor", "required"),
        (serde_json::json!({"cursor": "bad", "rules_revision": 1}), "cursor", "invalid_cursor"),
        (serde_json::json!({"limit": 0}), "limit", "out_of_range"),
        (serde_json::json!({"limit": 51}), "limit", "out_of_range"),
        (serde_json::json!({"limit": -1}), "limit", "out_of_range"),
        (serde_json::json!({"limit": 1.5}), "limit", "invalid"),
        (serde_json::json!({"limit": "50"}), "limit", "invalid"),
        (serde_json::json!({"limit": true}), "limit", "invalid"),
    ] {
        let (status, _, response) = rescan(
            t.app.clone(),
            Some(&admin),
            Some(&admin.csrf),
            body,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert_eq!(response["error"]["fields"][field], expected);
    }
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
    assert_eq!(case_count(&t.pool).await, 0);
    assert_eq!(audit_count(&t.pool, "moderation.rescan").await, 0);
}

#[tokio::test]
async fn fifty_one_rows_retry_and_deleted_anchor_keep_counts_cursors_and_audits_exact() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, author.id).await;
    let rule = create_rule(t.app.clone(), &admin, "rescanbad", "compact").await;
    let mut comments = Vec::new();
    for number in 1..=51 {
        comments.push(
            insert_comment(&t.pool, listing.id, author.id, number, "rescanbad", "visible", false)
                .await,
        );
    }
    sqlx::query("UPDATE listings SET status = 'removed' WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("rescan ignores listing status");
    let revision = current_revision(&t.pool).await;
    let cursor = comment_public_id(0);
    let first = continuation(t.app.clone(), &admin, &cursor, revision, 50).await;
    assert_eq!(first.0, StatusCode::OK, "{:?}", first.2);
    assert_response_shape(&first.2);
    assert_eq!(first.2["rules_revision"], revision);
    assert_eq!(first.2["scanned_count"], 50);
    assert_eq!(first.2["flagged_count"], 50);
    assert_eq!(first.2["has_more"], true);
    assert_eq!(first.2["next_cursor"], comments[49].public_id);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
    assert_eq!(case_count(&t.pool).await, 50);

    assert_eq!(
        delete_comment(t.app.clone(), &author, &comments[49].public_id).await,
        StatusCode::OK
    );
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
    let retry = continuation(t.app.clone(), &admin, &cursor, revision, 50).await;
    assert_eq!(retry.0, StatusCode::OK, "{:?}", retry.2);
    assert_eq!(retry.2["scanned_count"], 1);
    assert_eq!(retry.2["flagged_count"], 1);
    assert_eq!(retry.2["has_more"], false);
    assert_eq!(retry.2["next_cursor"], serde_json::Value::Null);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 0);
    assert_eq!(case_count(&t.pool).await, 51);

    let after_anchor = continuation(
        t.app.clone(),
        &admin,
        &comments[49].public_id,
        revision,
        50,
    )
    .await;
    assert_eq!(after_anchor.0, StatusCode::OK, "{:?}", after_anchor.2);
    assert_eq!(after_anchor.2["scanned_count"], 0);
    assert_eq!(after_anchor.2["flagged_count"], 0);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 0);
    assert_eq!(audit_count(&t.pool, "moderation.rescan").await, 3);
    assert_eq!(audit_count(&t.pool, "comment.filter_confirm_hidden").await, 0);
    assert_eq!(audit_count(&t.pool, "comment.filter_restore").await, 0);

    let audits = sqlx::query(
        "SELECT admin_id, target_type, target_id, reason, metadata \
         FROM admin_audit_logs WHERE action = 'moderation.rescan' ORDER BY id ASC",
    )
    .fetch_all(&t.pool)
    .await
    .expect("rescan audits");
    assert_eq!(audits.len(), 3);
    let expected = [(50, 50, true), (1, 1, false), (0, 0, false)];
    for (row, (scanned, flagged, has_more)) in audits.iter().zip(expected) {
        assert_eq!(row.try_get::<u64, _>("admin_id").expect("admin"), admin.id);
        assert_eq!(row.try_get::<String, _>("target_type").expect("target"), "moderation_settings");
        assert_eq!(row.try_get::<u64, _>("target_id").expect("target id"), 1);
        assert_eq!(row.try_get::<Option<String>, _>("reason").expect("reason"), None);
        let metadata: Vec<u8> = row.try_get("metadata").expect("metadata bytes");
        let metadata: serde_json::Value = serde_json::from_slice(&metadata).expect("metadata JSON");
        assert_eq!(metadata.as_object().map(serde_json::Map::len), Some(4));
        assert_eq!(metadata["rules_revision"], revision);
        assert_eq!(metadata["scanned_count"], scanned);
        assert_eq!(metadata["flagged_count"], flagged);
        assert_eq!(metadata["has_more"], has_more);
    }
    assert_eq!(rule["revision"], 1);
}

#[tokio::test]
async fn changed_rules_reject_continuation_without_writes_and_fresh_run_preserves_hides() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, author.id).await;
    create_rule(t.app.clone(), &admin, "revisionbad", "compact").await;
    let first_comment = insert_comment(&t.pool, listing.id, author.id, 1, "revisionbad", "visible", false).await;
    let second_comment = insert_comment(&t.pool, listing.id, author.id, 2, "revisionbad", "visible", false).await;
    let first = new_rescan(t.app.clone(), &admin, serde_json::json!(1)).await;
    assert_eq!(first.0, StatusCode::OK, "{:?}", first.2);
    let old_revision = first.2["rules_revision"].as_u64().expect("old revision");
    let cursor = first.2["next_cursor"].as_str().expect("cursor").to_string();
    assert_eq!(first.2["flagged_count"], 1);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);

    create_rule(t.app.clone(), &admin, "differentbad", "phrase").await;
    assert!(current_revision(&t.pool).await > old_revision);
    let (status, _, stale) = continuation(t.app.clone(), &admin, &cursor, old_revision, 1).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{stale}");
    assert_eq!(stale["error"]["fields"]["rules_revision"], "changed");
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
    assert_eq!(case_count(&t.pool).await, 1);
    assert_eq!(audit_count(&t.pool, "moderation.rescan").await, 1);
    assert_eq!(case_row(&t.pool, first_comment.id).await.0, "pending");
    let status: String = sqlx::query_scalar("SELECT status FROM listing_comments WHERE id = ?")
        .bind(second_comment.id)
        .fetch_one(&t.pool)
        .await
        .expect("second status");
    assert_eq!(status, "visible");

    let fresh = new_rescan(t.app.clone(), &admin, serde_json::json!(50)).await;
    assert_eq!(fresh.0, StatusCode::OK, "{:?}", fresh.2);
    assert_eq!(fresh.2["scanned_count"], 1);
    assert_eq!(fresh.2["flagged_count"], 1);
    assert_eq!(fresh.2["has_more"], false);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 0);
    assert_eq!(case_row(&t.pool, first_comment.id).await.0, "pending");
    assert_eq!(case_row(&t.pool, second_comment.id).await.0, "pending");
    assert_eq!(audit_count(&t.pool, "moderation.rescan").await, 2);
}

#[tokio::test]
async fn zero_active_rules_scan_without_hiding_or_restoring() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, author.id).await;
    insert_comment(&t.pool, listing.id, author.id, 1, "anything", "visible", false).await;
    let response = rescan(
        t.app.clone(),
        Some(&admin),
        Some(&admin.csrf),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(response.0, StatusCode::OK, "{:?}", response.2);
    assert_response_shape(&response.2);
    assert_eq!(response.2["scanned_count"], 1);
    assert_eq!(response.2["flagged_count"], 0);
    assert_eq!(response.2["has_more"], false);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
    assert_eq!(case_count(&t.pool).await, 0);
    assert_eq!(audit_count(&t.pool, "moderation.rescan").await, 1);
}

#[tokio::test]
async fn clearances_are_revision_safe_and_deactivation_never_restores() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, author.id).await;
    let rule = create_rule(t.app.clone(), &admin, "durablybad", "compact").await;
    let restored = insert_comment(&t.pool, listing.id, author.id, 1, "durablybad", "visible", false).await;
    let pending = insert_comment(&t.pool, listing.id, author.id, 2, "durablybad", "hidden", false).await;
    let clearance = serde_json::json!([{
        "public_id": rule["public_id"],
        "revision": rule["revision"],
    }]);
    insert_case(&t.pool, restored.id, "restored", &snapshot(&rule), Some(&clearance), Some(admin.id)).await;
    insert_case(&t.pool, pending.id, "pending", &snapshot(&rule), None, None).await;

    let skipped = new_rescan(t.app.clone(), &admin, serde_json::json!(50)).await;
    assert_eq!(skipped.0, StatusCode::OK, "{:?}", skipped.2);
    assert_eq!(skipped.2["scanned_count"], 1);
    assert_eq!(skipped.2["flagged_count"], 0);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
    assert_eq!(case_row(&t.pool, restored.id).await.0, "restored");

    let edited = patch_rule(
        t.app.clone(),
        &admin,
        rule["public_id"].as_str().expect("rule id"),
        serde_json::json!({"match_mode": "phrase"}),
    )
    .await;
    assert_eq!(edited["revision"], 2);
    let reopened = new_rescan(t.app.clone(), &admin, serde_json::json!(50)).await;
    assert_eq!(reopened.0, StatusCode::OK, "{:?}", reopened.2);
    assert_eq!(reopened.2["scanned_count"], 1);
    assert_eq!(reopened.2["flagged_count"], 1);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 0);
    let (status, matched, retained, handler) = case_row(&t.pool, restored.id).await;
    assert_eq!(status, "pending");
    assert_eq!(matched, snapshot(&edited));
    assert_eq!(retained, Some(clearance.clone()));
    assert_eq!(handler, None);
    assert_eq!(case_row(&t.pool, pending.id).await.0, "pending");

    let (status, _, restored_response) = send(
        t.app.clone(),
        Method::POST,
        &format!("/api/v1/admin/moderation/comments/{}/actions", restored.public_id),
        Some(&admin),
        Some(&admin.csrf),
        Some(serde_json::json!({"action": "restore", "reason": "false positive"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored_response}");
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
    let expected_clearances = serde_json::json!([
        {"public_id": rule["public_id"], "revision": 1},
        {"public_id": rule["public_id"], "revision": 2},
    ]);
    assert_eq!(case_row(&t.pool, restored.id).await.2, Some(expected_clearances));
    let unchanged = new_rescan(t.app.clone(), &admin, serde_json::json!(50)).await;
    assert_eq!(unchanged.0, StatusCode::OK);
    assert_eq!(unchanged.2["flagged_count"], 0);
    assert_eq!(assert_counter(&t.pool, listing.id).await, 1);

    for active in [false, true] {
        patch_rule(
            t.app.clone(),
            &admin,
            rule["public_id"].as_str().expect("rule id"),
            serde_json::json!({"is_active": active}),
        )
        .await;
        let response = new_rescan(t.app.clone(), &admin, serde_json::json!(50)).await;
        assert_eq!(response.0, StatusCode::OK, "{:?}", response.2);
        assert_eq!(response.2["flagged_count"], 0);
        assert_eq!(assert_counter(&t.pool, listing.id).await, 1);
        assert_eq!(case_row(&t.pool, restored.id).await.0, "restored");
        assert_eq!(case_row(&t.pool, pending.id).await.0, "pending");
        let pending_status: String = sqlx::query_scalar("SELECT status FROM listing_comments WHERE id = ?")
            .bind(pending.id)
            .fetch_one(&t.pool)
            .await
            .expect("pending comment");
        assert_eq!(pending_status, "hidden");
    }
    assert_eq!(audit_count(&t.pool, "moderation.rescan").await, 5);
    assert_eq!(audit_count(&t.pool, "comment.filter_restore").await, 1);
}

#[tokio::test]
async fn concurrent_rescan_delete_report_and_restore_preserve_counters() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, author.id).await;
    let rule = create_rule(t.app.clone(), &admin, "racebad", "compact").await;
    let comment = insert_comment(&t.pool, listing.id, author.id, 1, "racebad", "visible", false).await;
    insert_case(&t.pool, comment.id, "restored", &snapshot(&rule), Some(&serde_json::json!([])), Some(admin.id)).await;
    let report = insert_report(&t.pool, reporter.id, comment.id).await;

    let (scanned, deleted, report_hidden, report_restored, filter_restored) = tokio::join!(
        new_rescan(t.app.clone(), &admin, serde_json::json!(1)),
        delete_comment(t.app.clone(), &author, &comment.public_id),
        report_action(t.app.clone(), &admin, &report, "hide_comment"),
        report_action(t.app.clone(), &admin, &report, "restore_comment"),
        filter_restore(t.app.clone(), &admin, &comment.public_id),
    );
    assert_eq!(scanned.0, StatusCode::OK, "{:?}", scanned.2);
    assert!(scanned.2["scanned_count"].as_u64().expect("scanned") <= 1);
    assert!(scanned.2["flagged_count"].as_u64().expect("flagged") <= 1);
    assert_eq!(deleted, StatusCode::OK);
    for status in [report_hidden, report_restored, filter_restored] {
        assert!(status == StatusCode::OK || status == StatusCode::BAD_REQUEST, "unexpected {status}");
    }
    assert_eq!(assert_counter(&t.pool, listing.id).await, 0);
    assert_eq!(audit_count(&t.pool, "moderation.rescan").await, 1);
    for action in ["report.hide_comment", "report.restore_comment", "comment.filter_restore"] {
        assert!(audit_count(&t.pool, action).await <= 1, "duplicate {action}");
    }
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT deleted_at FROM listing_comments WHERE id = ?",
    )
    .bind(comment.id)
    .fetch_one(&t.pool)
    .await
    .expect("deleted target");
    assert!(deleted_at.is_some());
    let (_, _, page) = send(
        t.app.clone(),
        Method::GET,
        &format!("/api/v1/listings/{}/comments", listing.slug),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(page["data"].as_array().map(Vec::len), Some(0));
}
