//! Phase 9 report creation: validation, visibility, inert side effects,
//! idempotent duplicates, and process-local per-user limits.

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
    setup_with(common::test_config()).await
}

async fn setup_with(cfg: Config) -> Option<Setup> {
    let lock = TEST_LOCK.lock().await;
    let pool = common::test_pool().await?;
    reset_db(&pool).await;
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
        "INSERT INTO users (public_id, phone_e164, created_at, updated_at) \
         VALUES (?, ?, NOW(3), NOW(3))",
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

async fn create_listing(pool: &MySqlPool, seller_id: u64, status: &str) -> ListingFixture {
    let public_id = ulid::Ulid::new().to_string();
    let slug = format!("listing-{}", ulid::Ulid::new());
    let result = sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, published_at, created_at, updated_at) \
         VALUES (?, ?, 1, 1, 'report fixture', ?, 'description', 1000, 'fixed', ?, \
         NOW(3), NOW(3), NOW(3))",
    )
    .bind(public_id)
    .bind(seller_id)
    .bind(&slug)
    .bind(status)
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
    status: &str,
    deleted: bool,
) -> String {
    let public_id = ulid::Ulid::new().to_string();
    let deleted_at = deleted.then(fixture_created_at);
    sqlx::query(
        "INSERT INTO listing_comments \
         (public_id, listing_id, author_id, body, status, deleted_at, created_at, updated_at) \
         VALUES (?, ?, ?, 'report comment fixture', ?, ?, NOW(3), NOW(3))",
    )
    .bind(&public_id)
    .bind(listing_id)
    .bind(author_id)
    .bind(status)
    .bind(deleted_at)
    .execute(pool)
    .await
    .expect("comment fixture");
    if status == "visible" && !deleted {
        sqlx::query("UPDATE listings SET comment_count = comment_count + 1 WHERE id = ?")
            .bind(listing_id)
            .execute(pool)
            .await
            .expect("comment counter fixture");
    }
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

async fn post_report(
    app: Router,
    user: Option<&TestUser>,
    csrf: Option<&str>,
    body: serde_json::Value,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(app, Method::POST, "/api/v1/reports", user, csrf, Some(body)).await
}

async fn report_target(
    app: Router,
    user: &TestUser,
    target_type: &str,
    target_id: &str,
    reason: &str,
    details: Option<&str>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    let mut body = serde_json::json!({
        "target_type": target_type,
        "target_id": target_id,
        "reason": reason,
    });
    if let Some(details) = details {
        body["details"] = serde_json::json!(details);
    }
    post_report(app, Some(user), Some(&user.csrf), body).await
}

async fn report_count(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM reports")
        .fetch_one(pool)
        .await
        .expect("report count")
}

fn assert_report_shape(body: &serde_json::Value, expected_status: &str) -> String {
    let object = body.as_object().expect("report object");
    assert_eq!(object.len(), 2, "public response only: {body}");
    assert_eq!(body["public_id"].as_str().map(str::len), Some(26));
    assert_eq!(body["status"], expected_status);
    assert!(!object.contains_key("target_id"));
    assert!(!object.contains_key("reporter_id"));
    body["public_id"].as_str().expect("report ID").to_string()
}

fn fixture_created_at() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp_micros(1_700_000_000_123_000).expect("valid fixture timestamp")
}

#[tokio::test]
async fn report_requires_session_and_csrf_without_creating_rows() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let body = serde_json::json!({
        "target_type": "listing",
        "target_id": listing.slug,
        "reason": "spam",
    });

    let (status, _, response) = post_report(t.app.clone(), None, None, body.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(response["error"]["code"], "unauthorized");
    for csrf in [None, Some("wrong-token")] {
        let (status, _, response) =
            post_report(t.app.clone(), Some(&reporter), csrf, body.clone()).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(response["error"]["code"], "csrf_failed");
    }
    assert_eq!(report_count(&t.pool).await, 0);
}

#[tokio::test]
async fn report_validation_unknown_fields_and_unicode_details() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let cases = vec![
        (
            serde_json::json!({"target_type": null, "target_id": listing.slug, "reason": "spam"}),
            "target_type",
            "required",
        ),
        (
            serde_json::json!({"target_type": "seller", "target_id": listing.slug, "reason": "spam"}),
            "target_type",
            "unknown_target_type",
        ),
        (
            serde_json::json!({"target_type": "listing", "target_id": null, "reason": "spam"}),
            "target_id",
            "required",
        ),
        (
            serde_json::json!({"target_type": "listing", "target_id": "  ", "reason": "spam"}),
            "target_id",
            "required",
        ),
        (
            serde_json::json!({"target_type": "listing", "target_id": listing.slug, "reason": null}),
            "reason",
            "required",
        ),
        (
            serde_json::json!({"target_type": "listing", "target_id": listing.slug, "reason": "violence"}),
            "reason",
            "unknown_reason",
        ),
        (
            serde_json::json!({"target_type": "listing", "target_id": listing.slug, "reason": "spam", "details": 5}),
            "details",
            "invalid",
        ),
        (
            serde_json::json!({"target_type": "listing", "target_id": listing.slug, "reason": "spam", "unexpected": true}),
            "unexpected",
            "unknown_field",
        ),
        (
            serde_json::json!({"target_type": "listing", "target_id": listing.slug, "reason": "spam", "details": "🙂".repeat(501)}),
            "details",
            "too_long",
        ),
    ];
    for (body, field, reason) in cases {
        let (status, _, response) =
            post_report(t.app.clone(), Some(&reporter), Some(&reporter.csrf), body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert_eq!(response["error"]["code"], "validation_error");
        assert_eq!(response["error"]["fields"][field], reason);
    }
    assert_eq!(report_count(&t.pool).await, 0);

    // Missing, null, and whitespace-only details all normalize to SQL NULL.
    for mode in ["missing", "null", "blank"] {
        let target = create_listing(&t.pool, seller.id, "active").await;
        let mut body = serde_json::json!({
            "target_type": "listing",
            "target_id": target.slug,
            "reason": "other",
        });
        if mode == "null" {
            body["details"] = serde_json::Value::Null;
        } else if mode == "blank" {
            body["details"] = serde_json::json!(" \n\t ");
        }
        let (status, _, response) =
            post_report(t.app.clone(), Some(&reporter), Some(&reporter.csrf), body).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        let public_id = assert_report_shape(&response, "open");
        let details: Option<String> =
            sqlx::query_scalar("SELECT details FROM reports WHERE public_id = ?")
                .bind(public_id)
                .fetch_one(&t.pool)
                .await
                .expect("normalized details");
        assert_eq!(details, None, "mode {mode}");
    }

    let target = create_listing(&t.pool, seller.id, "active").await;
    let exact = "🙂".repeat(500);
    let padded = format!(" \n{exact}\t ");
    let (status, _, response) = report_target(
        t.app.clone(),
        &reporter,
        "listing",
        &target.slug,
        "other",
        Some(&padded),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let public_id = assert_report_shape(&response, "open");
    let details: Option<String> =
        sqlx::query_scalar("SELECT details FROM reports WHERE public_id = ?")
            .bind(public_id)
            .fetch_one(&t.pool)
            .await
            .expect("500-character details");
    assert_eq!(details.as_deref(), Some(exact.as_str()));
    assert_eq!(report_count(&t.pool).await, 4);
}

#[tokio::test]
async fn reports_cover_every_target_and_reason_without_side_effects() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let comment_author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let target_user = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let mut reporters = Vec::new();
    for phone in [
        "+989100000010",
        "+989100000011",
        "+989100000012",
        "+989100000013",
        "+989100000014",
        "+989100000015",
    ] {
        reporters.push(create_user(&t.pool, &t.cfg, phone).await);
    }
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let comment_id = insert_comment(&t.pool, listing.id, comment_author.id, "visible", false).await;
    let cases = vec![
        ("listing", listing.slug.clone(), "spam", 0),
        ("comment", comment_id.clone(), "abuse", 1),
        ("user", target_user.public_id.clone(), "fraud", 2),
        ("listing", listing.slug.clone(), "prohibited_item", 3),
        ("listing", listing.slug.clone(), "offensive", 4),
        ("listing", listing.slug.clone(), "other", 5),
    ];
    let mut public_ids = std::collections::BTreeSet::new();
    for (target_type, target_id, reason, reporter_index) in cases {
        let (status, _, response) = report_target(
            t.app.clone(),
            &reporters[reporter_index],
            target_type,
            &target_id,
            reason,
            Some("  evidence  "),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{response}");
        let public_id = assert_report_shape(&response, "open");
        public_ids.insert(public_id.clone());
        let stored_reason: String =
            sqlx::query_scalar("SELECT reason FROM reports WHERE public_id = ?")
                .bind(&public_id)
                .fetch_one(&t.pool)
                .await
                .expect("stored reason");
        assert_eq!(stored_reason, reason);
        let details: Option<String> =
            sqlx::query_scalar("SELECT details FROM reports WHERE public_id = ?")
                .bind(public_id)
                .fetch_one(&t.pool)
                .await
                .expect("stored details");
        assert_eq!(details.as_deref(), Some("evidence"));
    }
    assert_eq!(public_ids.len(), 6);
    assert_eq!(report_count(&t.pool).await, 6);

    // Reports are inert until an admin decision: no automatic hiding/removal.
    let listing_state = sqlx::query(
        "SELECT status, comment_count, rating_sum, rating_count, favorite_count \
         FROM listings WHERE id = ?",
    )
    .bind(listing.id)
    .fetch_one(&t.pool)
    .await
    .expect("listing state");
    assert_eq!(
        listing_state
            .try_get::<String, _>("status")
            .expect("status"),
        "active"
    );
    assert_eq!(
        listing_state
            .try_get::<u32, _>("comment_count")
            .expect("comments"),
        1
    );
    assert_eq!(
        listing_state
            .try_get::<u32, _>("rating_sum")
            .expect("rating sum"),
        0
    );
    assert_eq!(
        listing_state
            .try_get::<u32, _>("rating_count")
            .expect("ratings"),
        0
    );
    assert_eq!(
        listing_state
            .try_get::<u32, _>("favorite_count")
            .expect("favorites"),
        0
    );
    let comment_state =
        sqlx::query("SELECT status, deleted_at FROM listing_comments WHERE public_id = ?")
            .bind(&comment_id)
            .fetch_one(&t.pool)
            .await
            .expect("comment state");
    assert_eq!(
        comment_state
            .try_get::<String, _>("status")
            .expect("comment status"),
        "visible"
    );
    assert!(comment_state
        .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("deleted_at")
        .expect("comment deletion")
        .is_none());
    let user_status: String = sqlx::query_scalar("SELECT status FROM users WHERE id = ?")
        .bind(target_user.id)
        .fetch_one(&t.pool)
        .await
        .expect("target user status");
    assert_eq!(user_status, "active");
    let audits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs")
        .fetch_one(&t.pool)
        .await
        .expect("audit count");
    assert_eq!(audits, 0);
}

#[tokio::test]
async fn listing_report_visibility_and_self_report() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;

    for status in ["draft", "pending", "rejected", "sold", "archived"] {
        sqlx::query("UPDATE listings SET status = ? WHERE id = ?")
            .bind(status)
            .bind(listing.id)
            .execute(&t.pool)
            .await
            .expect("listing status fixture");
        let (http_status, _, body) = report_target(
            t.app.clone(),
            &reporter,
            "listing",
            &listing.slug,
            "spam",
            None,
        )
        .await;
        assert_eq!(
            http_status,
            StatusCode::NOT_FOUND,
            "status {status}: {body}"
        );
        assert_eq!(body["error"]["code"], "not_found");
    }
    sqlx::query("UPDATE listings SET status = 'removed' WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("removed fixture");
    assert_eq!(
        report_target(
            t.app.clone(),
            &reporter,
            "listing",
            &listing.slug,
            "spam",
            None
        )
        .await
        .0,
        StatusCode::GONE
    );
    sqlx::query("UPDATE listings SET status = 'active', deleted_at = NOW(3) WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("deleted fixture");
    assert_eq!(
        report_target(
            t.app.clone(),
            &reporter,
            "listing",
            &listing.slug,
            "spam",
            None
        )
        .await
        .0,
        StatusCode::GONE
    );
    assert_eq!(
        report_target(
            t.app.clone(),
            &reporter,
            "listing",
            "unknown-slug",
            "spam",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(report_count(&t.pool).await, 0);

    let (status, _, body) = report_target(
        t.app.clone(),
        &seller,
        "listing",
        &listing.slug,
        "spam",
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::GONE,
        "visibility precedes ownership: {body}"
    );
    sqlx::query("UPDATE listings SET deleted_at = NULL WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("restore fixture");
    let (status, _, body) = report_target(
        t.app.clone(),
        &seller,
        "listing",
        &listing.slug,
        "spam",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["target_id"], "self_report");
    let (status, _, response) = report_target(
        t.app.clone(),
        &reporter,
        "listing",
        &listing.slug,
        "spam",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_report_shape(&response, "open");
    assert_eq!(report_count(&t.pool).await, 1);
}

#[tokio::test]
async fn comment_report_visibility_and_self_report() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;

    for (status, deleted) in [("hidden", false), ("removed", false), ("visible", true)] {
        let public_id = insert_comment(&t.pool, listing.id, author.id, status, deleted).await;
        let (http_status, _, body) = report_target(
            t.app.clone(),
            &reporter,
            "comment",
            &public_id,
            "abuse",
            None,
        )
        .await;
        assert_eq!(
            http_status,
            StatusCode::NOT_FOUND,
            "{status}/{deleted}: {body}"
        );
    }
    let pending = create_listing(&t.pool, seller.id, "pending").await;
    let pending_comment = insert_comment(&t.pool, pending.id, author.id, "visible", false).await;
    assert_eq!(
        report_target(
            t.app.clone(),
            &reporter,
            "comment",
            &pending_comment,
            "abuse",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let removed = create_listing(&t.pool, seller.id, "removed").await;
    let removed_comment = insert_comment(&t.pool, removed.id, author.id, "visible", false).await;
    assert_eq!(
        report_target(
            t.app.clone(),
            &reporter,
            "comment",
            &removed_comment,
            "abuse",
            None
        )
        .await
        .0,
        StatusCode::GONE
    );
    assert_eq!(
        report_target(
            t.app.clone(),
            &reporter,
            "comment",
            &"0".repeat(26),
            "abuse",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    let own_comment = insert_comment(&t.pool, listing.id, reporter.id, "visible", false).await;
    let (status, _, body) = report_target(
        t.app.clone(),
        &reporter,
        "comment",
        &own_comment,
        "abuse",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["target_id"], "self_report");
    assert_eq!(report_count(&t.pool).await, 0);

    let visible = insert_comment(&t.pool, listing.id, author.id, "visible", false).await;
    let (status, _, response) =
        report_target(t.app.clone(), &reporter, "comment", &visible, "abuse", None).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_report_shape(&response, "open");
    assert_eq!(report_count(&t.pool).await, 1);
}

#[tokio::test]
async fn user_report_requires_active_target_and_rejects_self() {
    let Some(t) = setup().await else { return };
    let reporter = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let active = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let suspended = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let deleted = create_user(&t.pool, &t.cfg, "+989100000004").await;
    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = ?")
        .bind(suspended.id)
        .execute(&t.pool)
        .await
        .expect("suspended fixture");
    sqlx::query("UPDATE users SET status = 'deleted', deleted_at = NOW(3) WHERE id = ?")
        .bind(deleted.id)
        .execute(&t.pool)
        .await
        .expect("deleted fixture");

    for target in [&suspended, &deleted] {
        let (status, _, _) = report_target(
            t.app.clone(),
            &reporter,
            "user",
            &target.public_id,
            "abuse",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    assert_eq!(
        report_target(
            t.app.clone(),
            &reporter,
            "user",
            &"0".repeat(26),
            "abuse",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, _, body) = report_target(
        t.app.clone(),
        &reporter,
        "user",
        &reporter.public_id,
        "abuse",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["target_id"], "self_report");
    assert_eq!(report_count(&t.pool).await, 0);

    let (status, _, response) = report_target(
        t.app.clone(),
        &reporter,
        "user",
        &active.public_id,
        "abuse",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_report_shape(&response, "open");
    assert_eq!(report_count(&t.pool).await, 1);
}

#[tokio::test]
async fn duplicate_reports_return_existing_without_overwriting() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let other = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;

    let (status, _, first) = report_target(
        t.app.clone(),
        &reporter,
        "listing",
        &listing.slug,
        "spam",
        Some("original"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let public_id = assert_report_shape(&first, "open");
    let created_at: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT created_at FROM reports WHERE public_id = ?")
            .bind(&public_id)
            .fetch_one(&t.pool)
            .await
            .expect("original timestamp");

    for (reason, details) in [("spam", Some("original")), ("fraud", Some("changed"))] {
        let (status, _, duplicate) = report_target(
            t.app.clone(),
            &reporter,
            "listing",
            &listing.slug,
            reason,
            details,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(assert_report_shape(&duplicate, "open"), public_id);
        assert_eq!(report_count(&t.pool).await, 1);
    }
    let row = sqlx::query("SELECT reason, details, created_at FROM reports WHERE public_id = ?")
        .bind(&public_id)
        .fetch_one(&t.pool)
        .await
        .expect("unchanged report");
    assert_eq!(row.try_get::<String, _>("reason").expect("reason"), "spam");
    assert_eq!(
        row.try_get::<Option<String>, _>("details")
            .expect("details")
            .as_deref(),
        Some("original")
    );
    assert_eq!(
        row.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .expect("timestamp"),
        created_at
    );

    sqlx::query("UPDATE reports SET status = 'dismissed', resolution_action = 'dismiss' WHERE public_id = ?")
        .bind(&public_id)
        .execute(&t.pool)
        .await
        .expect("closed report fixture");
    let (status, _, closed_duplicate) = report_target(
        t.app.clone(),
        &reporter,
        "listing",
        &listing.slug,
        "other",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        assert_report_shape(&closed_duplicate, "dismissed"),
        public_id
    );

    let (status, _, independent) = report_target(
        t.app.clone(),
        &other,
        "listing",
        &listing.slug,
        "other",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_report_shape(&independent, "open");
    assert_eq!(report_count(&t.pool).await, 2);
}

#[tokio::test]
async fn concurrent_duplicate_reports_create_one_row() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let reporter = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let reasons = [
        "spam",
        "abuse",
        "fraud",
        "prohibited_item",
        "offensive",
        "other",
    ];
    let mut tasks = Vec::new();
    for reason in reasons {
        let app = t.app.clone();
        let reporter = reporter.clone();
        let slug = listing.slug.clone();
        tasks.push(tokio::spawn(async move {
            report_target(app, &reporter, "listing", &slug, reason, Some("race")).await
        }));
    }
    let mut public_ids = std::collections::BTreeSet::new();
    for task in tasks {
        let (status, _, body) = task.await.expect("report task");
        assert_eq!(status, StatusCode::OK, "{body}");
        public_ids.insert(assert_report_shape(&body, "open"));
    }
    assert_eq!(public_ids.len(), 1);
    assert_eq!(report_count(&t.pool).await, 1);
}

#[tokio::test]
async fn report_limit_is_per_user_after_validation_csrf_and_independent() {
    let Some(t) = setup_with({
        let mut cfg = common::test_config();
        cfg.report_user_limit = 2;
        cfg.report_user_window_secs = 60;
        cfg
    })
    .await
    else {
        return;
    };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let first = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let second = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let valid = serde_json::json!({
        "target_type": "listing",
        "target_id": listing.slug,
        "reason": "spam",
    });
    let invalid = serde_json::json!({
        "target_type": "listing",
        "target_id": listing.slug,
        "reason": null,
    });

    assert_eq!(
        post_report(
            t.app.clone(),
            Some(&first),
            Some(&first.csrf),
            invalid.clone()
        )
        .await
        .0,
        StatusCode::BAD_REQUEST,
        "validation precedes the limiter"
    );
    assert_eq!(
        post_report(t.app.clone(), Some(&first), None, valid.clone())
            .await
            .0,
        StatusCode::FORBIDDEN,
        "CSRF precedes the limiter"
    );
    for reason in ["spam", "abuse"] {
        let (status, _, body) = report_target(
            t.app.clone(),
            &first,
            "listing",
            &listing.slug,
            reason,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    assert_eq!(
        report_count(&t.pool).await,
        1,
        "the second attempt is a duplicate"
    );

    let (status, headers, body) = report_target(
        t.app.clone(),
        &first,
        "listing",
        &listing.slug,
        "other",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], "rate_limited");
    let retry_after: u64 = headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .expect("numeric Retry-After");
    assert!((1..=60).contains(&retry_after));
    assert_eq!(
        post_report(t.app.clone(), Some(&first), Some(&first.csrf), invalid)
            .await
            .0,
        StatusCode::BAD_REQUEST,
        "validation still precedes an exhausted limiter"
    );
    assert_eq!(
        post_report(t.app.clone(), Some(&first), None, valid)
            .await
            .0,
        StatusCode::FORBIDDEN,
        "CSRF still precedes an exhausted limiter"
    );

    let (status, _, _) = report_target(
        t.app.clone(),
        &second,
        "listing",
        &listing.slug,
        "spam",
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "another user has an independent window"
    );

    // Report limits use their own keys, not the rating/comment allowances.
    let (status, _, comment) = send(
        t.app.clone(),
        Method::POST,
        &format!("/api/v1/listings/{}/comments", listing.slug),
        Some(&first),
        Some(&first.csrf),
        Some(serde_json::json!({"body": "independent comment"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{comment}");
    let (status, _, rating) = send(
        t.app.clone(),
        Method::PUT,
        &format!("/api/v1/listings/{}/rating", listing.slug),
        Some(&first),
        Some(&first.csrf),
        Some(serde_json::json!({"stars": 4})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rating}");
    assert_eq!(report_count(&t.pool).await, 2);
}
