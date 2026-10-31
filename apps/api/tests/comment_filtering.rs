//! Phase 10 clean versus pending comment creation, privacy, and exact counters.

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

async fn create_listing(pool: &MySqlPool, seller_id: u64, status: &str) -> ListingFixture {
    let public_id = ulid::Ulid::new().to_string();
    let slug = format!("listing-{}", ulid::Ulid::new());
    let result = sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, published_at, created_at, updated_at) \
         VALUES (?, ?, 1, 1, 'comment fixture', ?, 'description', 1000, 'fixed', ?, \
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

async fn insert_rule(pool: &MySqlPool, phrase: &str, mode: MatchMode, active: bool) -> String {
    let normalized = normalize_rule(phrase, mode).expect("valid rule fixture");
    let public_id = ulid::Ulid::new().to_string();
    sqlx::query(
        "INSERT INTO moderation_rules \
         (public_id, phrase, normalized_phrase, dedupe_key, match_mode, is_active, revision, \
          created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 1, NOW(3), NOW(3))",
    )
    .bind(&public_id)
    .bind(&normalized.phrase)
    .bind(&normalized.normalized_phrase)
    .bind(&normalized.dedupe_key)
    .bind(mode.as_str())
    .bind(active)
    .execute(pool)
    .await
    .expect("rule fixture");
    public_id
}

fn comments_uri(slug: &str) -> String {
    format!("/api/v1/listings/{slug}/comments")
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

async fn post_comment(
    app: Router,
    listing: &ListingFixture,
    user: &TestUser,
    body: impl Into<String>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::POST,
        &comments_uri(&listing.slug),
        Some(user),
        Some(&user.csrf),
        Some(serde_json::json!({ "body": body.into() })),
    )
    .await
}

async fn delete_comment(
    app: Router,
    public_id: &str,
    user: &TestUser,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::DELETE,
        &format!("/api/v1/comments/{public_id}"),
        Some(user),
        Some(&user.csrf),
        None,
    )
    .await
}

async fn list_comments(
    app: Router,
    listing: &ListingFixture,
    user: Option<&TestUser>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::GET,
        &comments_uri(&listing.slug),
        user,
        None,
        None,
    )
    .await
}

async fn listing_detail(
    app: Router,
    listing: &ListingFixture,
    user: Option<&TestUser>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::GET,
        &format!("/api/v1/listings/{}", listing.slug),
        user,
        None,
        None,
    )
    .await
}

async fn assert_counter_matches_visible_rows(pool: &MySqlPool, listing_id: u64) -> u32 {
    let counter: u32 = sqlx::query_scalar("SELECT comment_count FROM listings WHERE id = ?")
        .bind(listing_id)
        .fetch_one(pool)
        .await
        .expect("comment counter");
    let visible: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM listing_comments \
         WHERE listing_id = ? AND status = 'visible' AND deleted_at IS NULL",
    )
    .bind(listing_id)
    .fetch_one(pool)
    .await
    .expect("visible rows");
    assert_eq!(i64::from(counter), visible, "comment_count invariant");
    counter
}

async fn case_count(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM comment_moderation_cases")
        .fetch_one(pool)
        .await
        .expect("case count")
}

async fn comment_row_count(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM listing_comments")
        .fetch_one(pool)
        .await
        .expect("comment row count")
}

async fn automatic_audit_count(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_logs")
        .fetch_one(pool)
        .await
        .expect("audit count")
}

fn comment_ids(body: &serde_json::Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("comment data")
        .iter()
        .map(|comment| {
            comment["public_id"]
                .as_str()
                .expect("comment public_id")
                .to_string()
        })
        .collect()
}

fn assert_clean_shape(comment: &serde_json::Value, author: &TestUser) {
    let object = comment.as_object().expect("clean comment object");
    assert_eq!(object.len(), 4, "unchanged clean shape: {comment}");
    for key in ["public_id", "body", "author", "created_at"] {
        assert!(object.contains_key(key));
    }
    assert_eq!(comment["author"]["public_id"], author.public_id);
    assert!(comment.get("status").is_none());
    assert!(comment.get("moderation_status").is_none());
}

fn assert_pending_shape(comment: &serde_json::Value) {
    let object = comment.as_object().expect("pending comment object");
    assert_eq!(object.len(), 3, "exact pending shape: {comment}");
    for key in ["public_id", "status", "created_at"] {
        assert!(object.contains_key(key));
    }
    assert_eq!(comment["status"], "pending_review");
    assert_eq!(comment["public_id"].as_str().map(str::len), Some(26));
    for key in [
        "id",
        "comment_id",
        "listing_id",
        "author_id",
        "rule_id",
        "body",
        "matched_rules",
    ] {
        assert!(!object.contains_key(key), "pending leaks {key}: {comment}");
    }
}

#[tokio::test]
async fn clean_and_matched_creation_have_exact_contracts_and_privacy() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let viewer = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let rule_id = insert_rule(&t.pool, "BadWord", MatchMode::Compact, true).await;

    let (status, _, clean) =
        post_comment(t.app.clone(), &listing, &author, "ordinary safe text").await;
    assert_eq!(status, StatusCode::OK, "{clean}");
    assert_clean_shape(&clean, &author);
    assert_eq!(clean["body"], "ordinary safe text");
    let clean_id = clean["public_id"].as_str().expect("clean ID").to_string();
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        1
    );

    let matched_body = "  <script>alert('x')</script> b.a.d.w.o.r.d  ";
    let (status, _, pending) = post_comment(t.app.clone(), &listing, &author, matched_body).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{pending}");
    assert_pending_shape(&pending);
    let pending_text = pending.to_string();
    assert!(!pending_text.contains(matched_body.trim()));
    assert!(!pending_text.contains("BadWord"));
    assert!(!pending_text.contains("<script>"));
    let pending_id = pending["public_id"]
        .as_str()
        .expect("pending ID")
        .to_string();

    let row = sqlx::query(
        "SELECT c.id, c.body, c.status, c.deleted_at, mc.status AS case_status, \
         mc.matched_rules, mc.cleared_rules, mc.handled_by, mc.handled_at \
         FROM listing_comments c JOIN comment_moderation_cases mc ON mc.comment_id = c.id \
         WHERE c.public_id = ?",
    )
    .bind(&pending_id)
    .fetch_one(&t.pool)
    .await
    .expect("pending database row");
    assert_eq!(
        row.try_get::<String, _>("body").expect("body"),
        matched_body.trim()
    );
    assert_eq!(
        row.try_get::<String, _>("status").expect("status"),
        "hidden"
    );
    assert!(row
        .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("deleted_at")
        .expect("deleted_at")
        .is_none());
    assert_eq!(
        row.try_get::<String, _>("case_status").expect("case"),
        "pending"
    );
    assert!(row
        .try_get::<Option<u64>, _>("handled_by")
        .expect("handler")
        .is_none());
    assert!(row
        .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("handled_at")
        .expect("handled_at")
        .is_none());
    assert!(row
        .try_get::<Option<Vec<u8>>, _>("cleared_rules")
        .expect("clearances")
        .is_none());
    let snapshots: Vec<u8> = row.try_get("matched_rules").expect("matched rules");
    let snapshots: serde_json::Value = serde_json::from_slice(&snapshots).expect("snapshot JSON");
    let snapshot_array = snapshots.as_array().expect("snapshot array");
    assert_eq!(snapshot_array.len(), 1);
    assert_eq!(
        snapshot_array[0].as_object().map(serde_json::Map::len),
        Some(4)
    );
    assert_eq!(snapshot_array[0]["public_id"], rule_id);
    assert_eq!(snapshot_array[0]["phrase"], "BadWord");
    assert_eq!(snapshot_array[0]["match_mode"], "compact");
    assert_eq!(snapshot_array[0]["revision"], 1);
    assert_eq!(case_count(&t.pool).await, 1);
    assert_eq!(automatic_audit_count(&t.pool).await, 0);
    let reports: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reports")
        .fetch_one(&t.pool)
        .await
        .expect("report count");
    assert_eq!(
        reports, 0,
        "automatic filtering manufactures no user report"
    );
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        1
    );

    for current_viewer in [None, Some(&author), Some(&viewer)] {
        let (status, _, page) = list_comments(t.app.clone(), &listing, current_viewer).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(comment_ids(&page), vec![clean_id.clone()]);
        assert_clean_shape(&page["data"][0], &author);
        assert!(!page.to_string().contains("moderation_status"));
    }
    let (status, _, detail) = listing_detail(t.app.clone(), &listing, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["comment_count"], 1);
    assert!(!detail.to_string().contains(matched_body.trim()));
    assert!(!detail.to_string().contains("BadWord"));
    assert!(!detail.to_string().contains("moderation_status"));
    assert!(detail.get("favorited").is_none());
    assert!(detail.get("my_stars").is_none());

    assert_eq!(
        delete_comment(t.app.clone(), &pending_id, &viewer).await.0,
        StatusCode::FORBIDDEN
    );
    for _ in 0..2 {
        assert_eq!(
            delete_comment(t.app.clone(), &pending_id, &author).await.0,
            StatusCode::OK
        );
        assert_eq!(
            assert_counter_matches_visible_rows(&t.pool, listing.id).await,
            1
        );
    }
    let deleted = sqlx::query(
        "SELECT c.status, c.deleted_at, mc.status AS case_status FROM listing_comments c \
         JOIN comment_moderation_cases mc ON mc.comment_id = c.id WHERE c.public_id = ?",
    )
    .bind(&pending_id)
    .fetch_one(&t.pool)
    .await
    .expect("deleted pending row");
    assert_eq!(
        deleted.try_get::<String, _>("status").expect("status"),
        "hidden"
    );
    assert!(deleted
        .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("deleted_at")
        .expect("deleted_at")
        .is_some());
    assert_eq!(
        deleted.try_get::<String, _>("case_status").expect("case"),
        "pending"
    );
}

#[tokio::test]
async fn active_rules_apply_normalized_evasions_without_retroactive_writes() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let compact_id = insert_rule(&t.pool, "BadWord", MatchMode::Compact, false).await;

    assert_eq!(
        post_comment(t.app.clone(), &listing, &author, "badword")
            .await
            .0,
        StatusCode::OK,
        "inactive rules do not filter"
    );
    sqlx::query("UPDATE moderation_rules SET is_active = TRUE WHERE public_id = ?")
        .bind(&compact_id)
        .execute(&t.pool)
        .await
        .expect("activate fixture");
    for body in [
        "b.a.d.w.o.r.d".to_string(),
        "B\u{200c}A\u{200c}D\u{200c}W\u{200c}O\u{200c}R\u{200c}D".to_string(),
        "bbaaddwwoorrdd".to_string(),
        format!("{}badword", "🙂".repeat(993)),
    ] {
        let (status, _, response) = post_comment(t.app.clone(), &listing, &author, body).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{response}");
        assert_pending_shape(&response);
    }
    assert_eq!(
        post_comment(t.app.clone(), &listing, &author, "badger")
            .await
            .0,
        StatusCode::OK,
        "safe near-match remains public"
    );

    insert_rule(&t.pool, "safe phrase", MatchMode::Phrase, true).await;
    assert_eq!(
        post_comment(t.app.clone(), &listing, &author, "SAFE   PHRASE")
            .await
            .0,
        StatusCode::ACCEPTED
    );
    for safe in ["safephrase", "safe phrasely"] {
        assert_eq!(
            post_comment(t.app.clone(), &listing, &author, safe).await.0,
            StatusCode::OK
        );
    }
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        4
    );
    assert_eq!(case_count(&t.pool).await, 5);
    assert_eq!(comment_row_count(&t.pool).await, 9);
}

#[tokio::test]
async fn matched_requests_preserve_auth_csrf_validation_and_target_guards() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    insert_rule(&t.pool, "BadWord", MatchMode::Compact, true).await;

    let (status, _, _) = send(
        t.app.clone(),
        Method::POST,
        &comments_uri(&listing.slug),
        None,
        None,
        Some(serde_json::json!({"body": "badword"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    for csrf in [None, Some("wrong-token")] {
        let (status, _, _) = send(
            t.app.clone(),
            Method::POST,
            &comments_uri(&listing.slug),
            Some(&author),
            csrf,
            Some(serde_json::json!({"body": "badword"})),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let (status, _, response) = post_comment(
        t.app.clone(),
        &listing,
        &author,
        format!("{}badword", "🙂".repeat(994)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response["error"]["fields"]["body"], "too_long");

    let unknown = ListingFixture {
        id: listing.id,
        slug: "unknown-listing".to_string(),
    };
    assert_eq!(
        post_comment(t.app.clone(), &unknown, &author, "badword")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = ?")
        .bind(author.id)
        .execute(&t.pool)
        .await
        .expect("suspend fixture");
    assert_eq!(
        post_comment(t.app.clone(), &listing, &author, "badword")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(comment_row_count(&t.pool).await, 0);
    assert_eq!(case_count(&t.pool).await, 0);
    assert_eq!(automatic_audit_count(&t.pool).await, 0);
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        0
    );
}

#[tokio::test]
async fn matched_creation_consumes_limiter_and_pending_delete_is_free() {
    let cfg = common::test_config_with(|cfg| {
        cfg.comment_user_limit = 2;
        cfg.comment_user_window_secs = 60;
    });
    let Some(t) = setup_with(cfg).await else {
        return;
    };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let first = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let second = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let first_listing = create_listing(&t.pool, seller.id, "active").await;
    let second_listing = create_listing(&t.pool, seller.id, "active").await;
    insert_rule(&t.pool, "BadWord", MatchMode::Compact, true).await;

    assert_eq!(
        post_comment(t.app.clone(), &first_listing, &first, "clean")
            .await
            .0,
        StatusCode::OK
    );
    let (status, _, pending) = post_comment(t.app.clone(), &first_listing, &first, "badword").await;
    assert_eq!(status, StatusCode::ACCEPTED, "{pending}");
    let pending_id = pending["public_id"]
        .as_str()
        .expect("pending ID")
        .to_string();
    let (status, headers, response) =
        post_comment(t.app.clone(), &first_listing, &first, "b.a.d.w.o.r.d").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{response}");
    assert_eq!(response["error"]["code"], "rate_limited");
    let retry_after: u64 = headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .expect("numeric Retry-After");
    assert!((1..=60).contains(&retry_after));

    assert_eq!(
        post_comment(t.app.clone(), &second_listing, &second, "badword")
            .await
            .0,
        StatusCode::ACCEPTED,
        "another user and listing have independent state"
    );
    assert_eq!(comment_row_count(&t.pool).await, 3);
    assert_eq!(case_count(&t.pool).await, 2);
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, first_listing.id).await,
        1
    );
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, second_listing.id).await,
        0
    );
    for _ in 0..2 {
        assert_eq!(
            delete_comment(t.app.clone(), &pending_id, &first).await.0,
            StatusCode::OK
        );
        assert_eq!(
            assert_counter_matches_visible_rows(&t.pool, first_listing.id).await,
            1
        );
    }
}

#[tokio::test]
async fn viewer_filtering_remains_blocker_only() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let clean_author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let pending_author = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let viewer = create_user(&t.pool, &t.cfg, "+989100000004").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    insert_rule(&t.pool, "BadWord", MatchMode::Compact, true).await;

    let (_, _, clean) = post_comment(t.app.clone(), &listing, &clean_author, "clean").await;
    assert_clean_shape(&clean, &clean_author);
    let clean_id = clean["public_id"].as_str().expect("clean ID").to_string();
    assert_eq!(
        post_comment(t.app.clone(), &listing, &pending_author, "badword")
            .await
            .0,
        StatusCode::ACCEPTED
    );
    let (status, _, blocked) = send(
        t.app.clone(),
        Method::PUT,
        &format!("/api/v1/me/blocks/{}", clean_author.public_id),
        Some(&viewer),
        Some(&viewer.csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{blocked}");

    let (_, _, anonymous) = list_comments(t.app.clone(), &listing, None).await;
    assert_eq!(comment_ids(&anonymous), vec![clean_id.clone()]);
    let (_, headers, blocked_page) = list_comments(t.app.clone(), &listing, Some(&viewer)).await;
    assert!(comment_ids(&blocked_page).is_empty());
    assert_eq!(
        headers
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("private, no-store")
    );
    let (_, _, pending_author_page) =
        list_comments(t.app.clone(), &listing, Some(&pending_author)).await;
    assert_eq!(comment_ids(&pending_author_page), vec![clean_id]);
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        1
    );
}

#[tokio::test]
async fn concurrent_clean_matched_creation_and_deletion_keep_counters_exact() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let first = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let second = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    insert_rule(&t.pool, "BadWord", MatchMode::Compact, true).await;

    let (clean_one, pending_one, clean_two, pending_two) = tokio::join!(
        post_comment(t.app.clone(), &listing, &first, "clean one"),
        post_comment(t.app.clone(), &listing, &first, "b.a.d.w.o.r.d"),
        post_comment(t.app.clone(), &listing, &second, "clean two"),
        post_comment(t.app.clone(), &listing, &second, "bbaaddwwoorrdd"),
    );
    assert_eq!(clean_one.0, StatusCode::OK, "{:?}", clean_one.2);
    assert_eq!(pending_one.0, StatusCode::ACCEPTED, "{:?}", pending_one.2);
    assert_eq!(clean_two.0, StatusCode::OK, "{:?}", clean_two.2);
    assert_eq!(pending_two.0, StatusCode::ACCEPTED, "{:?}", pending_two.2);
    assert_eq!(comment_row_count(&t.pool).await, 4);
    assert_eq!(case_count(&t.pool).await, 2);
    assert_eq!(automatic_audit_count(&t.pool).await, 0);
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        2
    );

    let clean_one_id = clean_one.2["public_id"]
        .as_str()
        .expect("clean ID")
        .to_string();
    let pending_one_id = pending_one.2["public_id"]
        .as_str()
        .expect("pending ID")
        .to_string();
    let clean_two_id = clean_two.2["public_id"]
        .as_str()
        .expect("clean ID")
        .to_string();
    let (deleted_clean, deleted_pending, duplicate_pending, clean_three) = tokio::join!(
        delete_comment(t.app.clone(), &clean_one_id, &first),
        delete_comment(t.app.clone(), &pending_one_id, &first),
        delete_comment(t.app.clone(), &pending_one_id, &first),
        post_comment(t.app.clone(), &listing, &second, "clean three"),
    );
    assert_eq!(deleted_clean.0, StatusCode::OK);
    assert_eq!(deleted_pending.0, StatusCode::OK);
    assert_eq!(duplicate_pending.0, StatusCode::OK);
    assert_eq!(clean_three.0, StatusCode::OK, "{:?}", clean_three.2);
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        2
    );
    assert_eq!(case_count(&t.pool).await, 2);
    let (_, _, page) = list_comments(t.app.clone(), &listing, None).await;
    let ids = comment_ids(&page);
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&clean_two_id));
    assert!(ids.contains(
        &clean_three.2["public_id"]
            .as_str()
            .expect("new ID")
            .to_string()
    ));
    assert!(!ids.contains(&pending_one_id));
}
