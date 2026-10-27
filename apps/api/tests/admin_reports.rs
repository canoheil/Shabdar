//! Phase 9 admin report queue and atomic moderation decisions.

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

struct ListingFixture {
    id: u64,
    public_id: String,
    slug: String,
}

struct CommentFixture {
    #[allow(dead_code)]
    id: u64,
    public_id: String,
}

struct ReportFixture {
    #[allow(dead_code)]
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
    let row = insert_user(pool, phone, display_name, chrono::Utc::now()).await;
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

async fn create_admin(pool: &MySqlPool, cfg: &Config) -> TestUser {
    let admin = create_user(pool, cfg, "+989100000001", Some("مدیر")).await;
    sqlx::query("UPDATE users SET is_admin = TRUE WHERE id = ?")
        .bind(admin.id)
        .execute(pool)
        .await
        .expect("admin fixture");
    admin
}

async fn insert_listing(pool: &MySqlPool, seller_id: u64, title: &str) -> ListingFixture {
    let public_id = ulid::Ulid::new().to_string();
    let slug = format!("listing-{}", ulid::Ulid::new());
    let result = sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, published_at, created_at, updated_at) \
         VALUES (?, ?, 1, 1, ?, ?, 'description', 1000, 'fixed', 'active', \
         NOW(3), NOW(3), NOW(3))",
    )
    .bind(&public_id)
    .bind(seller_id)
    .bind(title)
    .bind(&slug)
    .execute(pool)
    .await
    .expect("listing fixture");
    ListingFixture {
        id: result.last_insert_id(),
        public_id,
        slug,
    }
}

async fn insert_comment(
    pool: &MySqlPool,
    listing_id: u64,
    author_id: u64,
    body: &str,
) -> CommentFixture {
    let public_id = ulid::Ulid::new().to_string();
    let result = sqlx::query(
        "INSERT INTO listing_comments \
         (public_id, listing_id, author_id, body, created_at, updated_at) \
         VALUES (?, ?, ?, ?, NOW(3), NOW(3))",
    )
    .bind(&public_id)
    .bind(listing_id)
    .bind(author_id)
    .bind(body)
    .execute(pool)
    .await
    .expect("comment fixture");
    sqlx::query("UPDATE listings SET comment_count = comment_count + 1 WHERE id = ?")
        .bind(listing_id)
        .execute(pool)
        .await
        .expect("comment counter fixture");
    CommentFixture {
        id: result.last_insert_id(),
        public_id,
    }
}

async fn insert_report(
    pool: &MySqlPool,
    reporter_id: u64,
    target_type: &str,
    target_id: u64,
    created_at: chrono::DateTime<chrono::Utc>,
) -> ReportFixture {
    let public_id = ulid::Ulid::new().to_string();
    let result = sqlx::query(
        "INSERT INTO reports \
         (public_id, reporter_id, target_type, target_id, reason, details, created_at) \
         VALUES (?, ?, ?, ?, 'spam', 'safe <script>details</script>', ?)",
    )
    .bind(&public_id)
    .bind(reporter_id)
    .bind(target_type)
    .bind(target_id)
    .bind(created_at)
    .execute(pool)
    .await
    .expect("report fixture");
    ReportFixture {
        id: result.last_insert_id(),
        public_id,
    }
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
        &format!("/api/v1/admin/reports{query}"),
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
    report_public_id: &str,
    body: serde_json::Value,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::POST,
        &format!("/api/v1/admin/reports/{report_public_id}/actions"),
        user,
        csrf,
        Some(body),
    )
    .await
}

async fn action(
    app: Router,
    admin: &TestUser,
    report_public_id: &str,
    action: &str,
    reason: Option<&str>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    let mut body = serde_json::json!({ "action": action });
    if let Some(reason) = reason {
        body["reason"] = serde_json::json!(reason);
    }
    decide(
        app,
        Some(admin),
        Some(&admin.csrf),
        report_public_id,
        body,
    )
    .await
}

fn report_ids(body: &serde_json::Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("report data")
        .iter()
        .map(|item| item["public_id"].as_str().expect("public ID").to_string())
        .collect()
}

fn assert_keys(value: &serde_json::Value, expected: &[&str]) {
    let object = value.as_object().expect("object");
    let mut actual = object.keys().map(String::as_str).collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

fn assert_report_shape(item: &serde_json::Value) {
    assert_keys(
        item,
        &[
            "public_id",
            "target_type",
            "reason",
            "details",
            "status",
            "resolution_action",
            "created_at",
            "handled_at",
            "reporter",
            "handled_by",
            "target",
        ],
    );
    assert_keys(&item["reporter"], &["public_id", "display_name"]);
}

async fn audit_count(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs")
        .fetch_one(pool)
        .await
        .expect("audit count")
}

async fn listing_count(pool: &MySqlPool, listing_id: u64) -> i64 {
    sqlx::query_scalar("SELECT comment_count FROM listings WHERE id = ?")
        .bind(listing_id)
        .fetch_one(pool)
        .await
        .expect("listing counter")
}

async fn visible_rows(pool: &MySqlPool, listing_id: u64) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM listing_comments \
         WHERE listing_id = ? AND status = 'visible' AND deleted_at IS NULL",
    )
    .bind(listing_id)
    .fetch_one(pool)
    .await
    .expect("visible rows")
}

#[tokio::test]
async fn admin_report_queue_filters_keysets_and_shapes_current_targets() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000002", Some("گزارش‌دهنده")).await;
    let non_admin = create_user(&t.pool, &t.cfg, "+989100000003", Some("عادی")).await;
    let seller = create_user(&t.pool, &t.cfg, "+989100000004", Some("فروشنده")).await;
    let target_user = create_user(&t.pool, &t.cfg, "+989100000005", Some("کاربر هدف")).await;
    let listing = insert_listing(&t.pool, seller.id, "آگهی <script>safe</script>").await;
    let comment = insert_comment(&t.pool, listing.id, seller.id, "متن <b>کامنت</b>").await;
    let created_at = chrono::Utc::now();
    let listing_report = insert_report(&t.pool, reporter.id, "listing", listing.id, created_at).await;
    let comment_report = insert_report(&t.pool, reporter.id, "comment", comment.id, created_at).await;
    let user_report = insert_report(&t.pool, reporter.id, "user", target_user.id, created_at).await;
    let missing_report = insert_report(&t.pool, reporter.id, "user", 9_999_999, created_at).await;

    assert_eq!(queue(t.app.clone(), None, "").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        queue(t.app.clone(), Some(&non_admin), "").await.0,
        StatusCode::FORBIDDEN
    );
    for query in ["?status=bad", "?cursor=bad"] {
        let (status, _, body) = queue(t.app.clone(), Some(&admin), query).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "validation_error");
    }

    let (status, _, first) = queue(t.app.clone(), Some(&admin), "?limit=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["page"]["has_more"], true);
    assert_eq!(
        report_ids(&first),
        vec![listing_report.public_id.clone(), comment_report.public_id.clone()]
    );
    let listing_item = &first["data"][0];
    assert_report_shape(listing_item);
    assert_eq!(listing_item["reporter"]["public_id"], reporter.public_id);
    assert_eq!(listing_item["handled_by"], serde_json::Value::Null);
    assert_eq!(listing_item["details"], "safe <script>details</script>");
    assert_keys(
        &listing_item["target"],
        &["public_id", "slug", "title", "status", "deleted"],
    );
    assert_eq!(listing_item["target"]["public_id"], listing.public_id);
    assert_eq!(listing_item["target"]["slug"], listing.slug);
    assert_eq!(listing_item["target"]["title"], "آگهی <script>safe</script>");
    assert_eq!(listing_item["target"]["deleted"], false);
    let comment_item = &first["data"][1];
    assert_keys(
        &comment_item["target"],
        &["public_id", "body", "status", "deleted", "author", "listing_slug"],
    );
    assert_eq!(comment_item["target"]["body"], "متن <b>کامنت</b>");
    assert_eq!(comment_item["target"]["listing_slug"], listing.slug);
    assert_keys(
        &comment_item["target"]["author"],
        &["public_id", "display_name"],
    );
    let cursor = first["page"]["next_cursor"].as_str().expect("cursor").to_string();

    let (status, _, dismissed) = action(
        t.app.clone(),
        &admin,
        &comment_report.public_id,
        "dismiss",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dismissed["status"], "dismissed");
    assert_eq!(dismissed["handled_by"]["public_id"], admin.public_id);

    let (status, _, second) = queue(
        t.app.clone(),
        Some(&admin),
        &format!("?limit=2&cursor={cursor}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["page"]["has_more"], false);
    assert_eq!(
        report_ids(&second),
        vec![user_report.public_id.clone(), missing_report.public_id.clone()]
    );
    assert_keys(&second["data"][0]["target"], &["public_id", "display_name", "status"]);
    assert_eq!(second["data"][0]["target"]["public_id"], target_user.public_id);
    assert_eq!(second["data"][1]["target"], serde_json::Value::Null);

    let (_, _, dismissed_page) = queue(t.app.clone(), Some(&admin), "?status=dismissed").await;
    assert_eq!(report_ids(&dismissed_page), vec![comment_report.public_id.clone()]);
    assert_eq!(dismissed_page["data"][0]["resolution_action"], "dismiss");
    assert!(dismissed_page["data"][0]["handled_at"].is_string());
    let (_, _, all) = queue(t.app.clone(), Some(&admin), "?status=all&limit=50").await;
    assert_eq!(all["data"].as_array().expect("all").len(), 4);

    let (status, _, detail) = send(
        t.app.clone(),
        Method::GET,
        &format!("/api/v1/admin/listings/{}", listing.public_id),
        Some(&admin),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let history = detail["report_history"].as_array().expect("report history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["public_id"], listing_report.public_id);
    assert!(history[0].get("id").is_none());
}

#[tokio::test]
async fn admin_report_queue_defaults_to_twenty_and_caps_at_fifty() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let target = insert_user(&t.pool, "+989100000002", Some("هدف"), chrono::Utc::now()).await;
    let created_at = chrono::Utc::now();
    let mut expected = Vec::new();
    for index in 0..55 {
        let reporter = insert_user(
            &t.pool,
            &format!("+98930{index:07}"),
            None,
            created_at,
        )
        .await;
        let report = insert_report(&t.pool, reporter.id, "user", target.id, created_at).await;
        expected.push(report.public_id);
    }

    let (status, _, default) = queue(t.app.clone(), Some(&admin), "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(default["data"].as_array().expect("default").len(), 20);
    assert_eq!(default["page"]["has_more"], true);
    assert_eq!(report_ids(&default), expected[..20].to_vec());

    let (status, _, capped) = queue(t.app.clone(), Some(&admin), "?limit=999").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(capped["data"].as_array().expect("capped").len(), 50);
    assert_eq!(capped["page"]["has_more"], true);
    assert_eq!(report_ids(&capped), expected[..50].to_vec());
    let cursor = capped["page"]["next_cursor"].as_str().expect("cursor");
    let (_, _, last) = queue(
        t.app.clone(),
        Some(&admin),
        &format!("?limit=50&cursor={cursor}"),
    )
    .await;
    assert_eq!(last["data"].as_array().expect("last").len(), 5);
    assert_eq!(last["page"]["has_more"], false);
    assert_eq!(report_ids(&last), expected[50..].to_vec());
}

#[tokio::test]
async fn actions_validate_reason_target_and_closed_retries() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000002", None).await;
    let non_admin = create_user(&t.pool, &t.cfg, "+989100000003", None).await;
    let seller = create_user(&t.pool, &t.cfg, "+989100000004", None).await;
    let target = create_user(&t.pool, &t.cfg, "+989100000005", None).await;
    let listing = insert_listing(&t.pool, seller.id, "listing").await;
    let comment = insert_comment(&t.pool, listing.id, seller.id, "comment").await;
    let now = chrono::Utc::now();
    let comment_report = insert_report(&t.pool, reporter.id, "comment", comment.id, now).await;
    let user_report = insert_report(&t.pool, reporter.id, "user", target.id, now).await;

    let valid = serde_json::json!({"action":"dismiss"});
    assert_eq!(
        decide(t.app.clone(), Some(&non_admin), Some(&non_admin.csrf), &user_report.public_id, valid.clone())
            .await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        decide(t.app.clone(), Some(&admin), None, &user_report.public_id, valid).await.0,
        StatusCode::FORBIDDEN
    );
    for body in [
        serde_json::json!({}),
        serde_json::json!({"action":"explode"}),
        serde_json::json!({"action":"hide_comment","reason":"bad target"}),
    ] {
        let report = if body["action"] == "hide_comment" {
            &user_report
        } else {
            &comment_report
        };
        let (status, _, response) = decide(
            t.app.clone(),
            Some(&admin),
            Some(&admin.csrf),
            &report.public_id,
            body,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(response["error"]["code"], "validation_error");
        assert!(response["error"]["fields"]["action"].is_string());
    }
    for body in [
        serde_json::json!({"action":"hide_comment"}),
        serde_json::json!({"action":"hide_comment","reason":"   "}),
        serde_json::json!({"action":"hide_comment","reason":"x".repeat(501)}),
    ] {
        let (status, _, response) = decide(
            t.app.clone(),
            Some(&admin),
            Some(&admin.csrf),
            &comment_report.public_id,
            body,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(response["error"]["fields"]["reason"].is_string());
    }
    let (status, _, response) = action(
        t.app.clone(),
        &admin,
        &user_report.public_id,
        "dismiss",
        Some(&"x".repeat(501)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response["error"]["fields"]["reason"], "too_long");
    assert_eq!(audit_count(&t.pool).await, 0);

    let (status, _, dismissed) = action(
        t.app.clone(),
        &admin,
        &user_report.public_id,
        "dismiss",
        Some("   "),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dismissed["status"], "dismissed");
    assert_eq!(dismissed["resolution_action"], "dismiss");
    assert_report_shape(&dismissed);
    assert_eq!(audit_count(&t.pool).await, 1);
    let (status, _, repeated) = action(
        t.app.clone(),
        &admin,
        &user_report.public_id,
        "dismiss",
        Some("different ignored reason"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated, dismissed);
    assert_eq!(audit_count(&t.pool).await, 1);
    let (status, _, conflict) = action(
        t.app.clone(),
        &admin,
        &user_report.public_id,
        "suspend_user",
        Some("conflict"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(conflict["error"]["fields"]["status"], "invalid_transition");
    assert_eq!(audit_count(&t.pool).await, 1);

    assert_eq!(
        action(t.app.clone(), &admin, &"0".repeat(26), "dismiss", None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn comment_hide_restore_is_exact_and_audited() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000002", None).await;
    let other_reporter = create_user(&t.pool, &t.cfg, "+989100000003", None).await;
    let seller = create_user(&t.pool, &t.cfg, "+989100000004", None).await;
    let listing = insert_listing(&t.pool, seller.id, "listing").await;
    let first = insert_comment(&t.pool, listing.id, seller.id, "first").await;
    let second = insert_comment(&t.pool, listing.id, seller.id, "second").await;
    let now = chrono::Utc::now();
    let report = insert_report(&t.pool, reporter.id, "comment", first.id, now).await;
    let open_report = insert_report(&t.pool, other_reporter.id, "comment", second.id, now).await;
    assert_eq!(listing_count(&t.pool, listing.id).await, 2);

    let reason = "ر".repeat(500);
    let (status, _, hidden) = action(
        t.app.clone(),
        &admin,
        &report.public_id,
        "hide_comment",
        Some(&reason),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hidden["status"], "actioned");
    assert_eq!(hidden["resolution_action"], "hide_comment");
    assert_eq!(hidden["target"]["status"], "hidden");
    assert_eq!(listing_count(&t.pool, listing.id).await, 1);
    assert_eq!(visible_rows(&t.pool, listing.id).await, 1);
    assert_eq!(audit_count(&t.pool).await, 1);

    let (status, _, repeated) = action(
        t.app.clone(),
        &admin,
        &report.public_id,
        "hide_comment",
        Some("ignored"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated, hidden);
    assert_eq!(listing_count(&t.pool, listing.id).await, 1);
    assert_eq!(audit_count(&t.pool).await, 1);

    let (status, _, restored) = action(
        t.app.clone(),
        &admin,
        &report.public_id,
        "restore_comment",
        Some("بازگردانی"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(restored["status"], "actioned");
    assert_eq!(restored["resolution_action"], "restore_comment");
    assert_eq!(restored["target"]["status"], "visible");
    assert_eq!(listing_count(&t.pool, listing.id).await, 2);
    assert_eq!(visible_rows(&t.pool, listing.id).await, 2);
    assert_eq!(audit_count(&t.pool).await, 2);
    let (status, _, repeated_restore) = action(
        t.app.clone(),
        &admin,
        &report.public_id,
        "restore_comment",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated_restore, restored);
    assert_eq!(listing_count(&t.pool, listing.id).await, 2);
    assert_eq!(audit_count(&t.pool).await, 2);

    let (status, _, _) = action(
        t.app.clone(),
        &admin,
        &report.public_id,
        "dismiss",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, response) = action(
        t.app.clone(),
        &admin,
        &open_report.public_id,
        "restore_comment",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response["error"]["fields"]["status"], "invalid_transition");
    assert_eq!(audit_count(&t.pool).await, 2);
}

#[tokio::test]
async fn listing_and_user_actions_apply_shared_effects_once() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000002", None).await;
    let seller = create_user(&t.pool, &t.cfg, "+989100000003", None).await;
    let target = create_user(&t.pool, &t.cfg, "+989100000004", None).await;
    let other_cookie = {
        let token = shabdar_api::session::create_session(&t.pool, &t.cfg, target.id)
            .await
            .expect("second target session");
        format!("shabdar_session={token}; shabdar_csrf=other")
    };
    let listing = insert_listing(&t.pool, seller.id, "listing").await;
    let now = chrono::Utc::now();
    let listing_report = insert_report(&t.pool, reporter.id, "listing", listing.id, now).await;
    let user_report = insert_report(&t.pool, reporter.id, "user", target.id, now).await;

    let (status, _, removed) = action(
        t.app.clone(),
        &admin,
        &listing_report.public_id,
        "remove_listing",
        Some("policy"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(removed["target"]["status"], "removed");
    assert_eq!(audit_count(&t.pool).await, 1);
    assert_eq!(
        send(t.app.clone(), Method::GET, &format!("/api/v1/listings/{}", listing.slug), None, None, None)
            .await.0,
        StatusCode::GONE
    );
    let (status, _, repeated) = action(
        t.app.clone(),
        &admin,
        &listing_report.public_id,
        "remove_listing",
        Some("ignored"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated, removed);
    assert_eq!(audit_count(&t.pool).await, 1);

    let (status, _, suspended) = action(
        t.app.clone(),
        &admin,
        &user_report.public_id,
        "suspend_user",
        Some("abuse"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(suspended["target"]["status"], "suspended");
    assert_eq!(audit_count(&t.pool).await, 2);
    for cookie in [&target.cookie, &other_cookie] {
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/me")
            .header("cookie", cookie.as_str())
            .body(Body::empty())
            .expect("request");
        let response = t.app.clone().oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let (status, _, repeated_suspend) = action(
        t.app.clone(),
        &admin,
        &user_report.public_id,
        "suspend_user",
        Some("ignored"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated_suspend, suspended);
    assert_eq!(audit_count(&t.pool).await, 2);

    let audits = sqlx::query(
        "SELECT action, target_type, reason, CAST(metadata AS CHAR) AS metadata \
         FROM admin_audit_logs ORDER BY id ASC",
    )
    .fetch_all(&t.pool)
    .await
    .expect("audits");
    assert_eq!(audits[0].try_get::<String, _>("action").expect("action"), "report.remove_listing");
    assert_eq!(audits[1].try_get::<String, _>("action").expect("action"), "report.suspend_user");
    for row in audits {
        let metadata: String = row.try_get("metadata").expect("metadata");
        assert!(metadata.contains("report_public_id"));
        assert!(!metadata.contains("phone"));
    }
}

#[tokio::test]
async fn concurrent_decisions_and_comment_mutations_preserve_exact_counters() {
    let Some(t) = setup().await else { return };
    let admin = create_admin(&t.pool, &t.cfg).await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000002", None).await;
    let author = create_user(&t.pool, &t.cfg, "+989100000003", None).await;
    let other = create_user(&t.pool, &t.cfg, "+989100000004", None).await;
    let seller = create_user(&t.pool, &t.cfg, "+989100000005", None).await;
    let listing = insert_listing(&t.pool, seller.id, "listing").await;
    let comment = insert_comment(&t.pool, listing.id, author.id, "moderated").await;
    let report = insert_report(
        &t.pool,
        reporter.id,
        "comment",
        comment.id,
        chrono::Utc::now(),
    )
    .await;

    let mut hide_tasks = Vec::new();
    for _ in 0..2 {
        let app = t.app.clone();
        let admin = admin.clone();
        let report_id = report.public_id.clone();
        hide_tasks.push(tokio::spawn(async move {
            action(app, &admin, &report_id, "hide_comment", Some("race")).await
        }));
    }
    for task in hide_tasks {
        let (status, _, body) = task.await.expect("hide task");
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    assert_eq!(listing_count(&t.pool, listing.id).await, 0);
    assert_eq!(visible_rows(&t.pool, listing.id).await, 0);
    assert_eq!(audit_count(&t.pool).await, 1);

    let mut tasks = Vec::new();
    let delete_app = t.app.clone();
    let delete_author = author.clone();
    let delete_id = comment.public_id.clone();
    tasks.push(tokio::spawn(async move {
        send(
            delete_app,
            Method::DELETE,
            &format!("/api/v1/comments/{delete_id}"),
            Some(&delete_author),
            Some(&delete_author.csrf),
            None,
        )
        .await
        .0
    }));
    let create_app = t.app.clone();
    let create_user = other.clone();
    let create_slug = listing.slug.clone();
    tasks.push(tokio::spawn(async move {
        send(
            create_app,
            Method::POST,
            &format!("/api/v1/listings/{create_slug}/comments"),
            Some(&create_user),
            Some(&create_user.csrf),
            Some(serde_json::json!({"body":"new visible"})),
        )
        .await
        .0
    }));
    let restore_app = t.app.clone();
    let restore_admin = admin.clone();
    let restore_id = report.public_id.clone();
    tasks.push(tokio::spawn(async move {
        action(
            restore_app,
            &restore_admin,
            &restore_id,
            "restore_comment",
            None,
        )
        .await
        .0
    }));
    let mut statuses = Vec::new();
    for task in tasks {
        statuses.push(task.await.expect("mutation task"));
    }
    assert_eq!(statuses[0], StatusCode::OK);
    assert!(statuses[1].is_success());
    assert!(statuses[2] == StatusCode::OK || statuses[2] == StatusCode::BAD_REQUEST);
    assert_eq!(listing_count(&t.pool, listing.id).await, 1);
    assert_eq!(visible_rows(&t.pool, listing.id).await, 1);

    let (status, _, page) = send(
        t.app.clone(),
        Method::GET,
        &format!("/api/v1/listings/{}/comments", listing.slug),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = page["data"].as_array().expect("public comments");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["body"], "new visible");
    let expected_audits = if statuses[2] == StatusCode::OK { 2 } else { 1 };
    assert_eq!(audit_count(&t.pool).await, expected_audits);
}
