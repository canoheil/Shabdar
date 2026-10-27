//! Phase 9 blocks: private idempotent management and viewer-only comment filtering.

mod common;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
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

async fn create_listing(pool: &MySqlPool, seller_id: u64) -> ListingFixture {
    let public_id = ulid::Ulid::new().to_string();
    let slug = format!("listing-{}", ulid::Ulid::new());
    let result = sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, published_at, created_at, updated_at) \
         VALUES (?, ?, 1, 1, 'block fixture', ?, 'description', 1000, 'fixed', 'active', \
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

async fn send(
    app: Router,
    method: Method,
    uri: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    let response = app
        .oneshot(builder.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = BodyExt::collect(response.into_body())
        .await
        .expect("response body")
        .to_bytes();
    let body = serde_json::from_slice(&bytes).expect("JSON response");
    (status, headers, body)
}

fn block_uri(public_id: &str) -> String {
    format!("/api/v1/me/blocks/{public_id}")
}

async fn mutate_block(
    app: Router,
    method: Method,
    user: &TestUser,
    target: &TestUser,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        method,
        &block_uri(&target.public_id),
        Some(&user.cookie),
        Some(&user.csrf),
    )
    .await
}

async fn list_blocks(
    app: Router,
    user: &TestUser,
    query: &str,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::GET,
        &format!("/api/v1/me/blocks{query}"),
        Some(&user.cookie),
        None,
    )
    .await
}

async fn block_rows(pool: &MySqlPool, blocker_id: u64, blocked_id: u64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM blocks WHERE blocker_id = ? AND blocked_id = ?")
        .bind(blocker_id)
        .bind(blocked_id)
        .fetch_one(pool)
        .await
        .expect("block rows")
}

fn blocked_ids(body: &serde_json::Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("block data")
        .iter()
        .map(|item| {
            item["public_id"]
                .as_str()
                .expect("blocked public ID")
                .to_string()
        })
        .collect()
}

fn assert_public_block_shape(item: &serde_json::Value, target: &TestUser) {
    let object = item.as_object().expect("block object");
    assert_eq!(object.len(), 3, "no private user fields: {item}");
    assert_eq!(item["public_id"], target.public_id);
    assert!(item["display_name"].is_string() || item["display_name"].is_null());
    assert!(item["blocked_at"].is_string());
    assert!(!object.contains_key("phone_e164"));
    assert!(!object.contains_key("status"));
    assert!(!object.contains_key("blocked"));
}

async fn insert_block(pool: &MySqlPool, blocker_id: u64, blocked_id: u64) {
    sqlx::query(
        "INSERT INTO blocks (blocker_id, blocked_id, created_at) VALUES (?, ?, ?)",
    )
    .bind(blocker_id)
    .bind(blocked_id)
    .bind(fixture_created_at())
    .execute(pool)
    .await
    .expect("block page fixture");
}

async fn insert_comment(
    pool: &MySqlPool,
    listing_id: u64,
    author_id: u64,
    body: &str,
) -> String {
    let public_id = ulid::Ulid::new().to_string();
    let mut tx = pool.begin().await.expect("comment fixture transaction");
    sqlx::query(
        "INSERT INTO listing_comments \
         (public_id, listing_id, author_id, body, created_at, updated_at) \
         VALUES (?, ?, ?, ?, NOW(3), NOW(3))",
    )
    .bind(&public_id)
    .bind(listing_id)
    .bind(author_id)
    .bind(body)
    .execute(&mut *tx)
    .await
    .expect("comment fixture");
    sqlx::query("UPDATE listings SET comment_count = comment_count + 1 WHERE id = ?")
        .bind(listing_id)
        .execute(&mut *tx)
        .await
        .expect("comment counter fixture");
    tx.commit().await.expect("comment fixture commit");
    public_id
}

async fn list_comments(
    app: Router,
    listing: &ListingFixture,
    user: Option<&TestUser>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::GET,
        &format!("/api/v1/listings/{}/comments", listing.slug),
        user.map(|user| user.cookie.as_str()),
        None,
    )
    .await
}

fn comment_bodies(body: &serde_json::Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("comment data")
        .iter()
        .map(|comment| {
            comment["body"]
                .as_str()
                .expect("comment body")
                .to_string()
        })
        .collect()
}

fn assert_public_comment_shape(comment: &serde_json::Value) {
    let object = comment.as_object().expect("comment object");
    assert_eq!(object.len(), 4, "unchanged public comment shape: {comment}");
    assert!(comment["public_id"].is_string());
    assert!(comment["body"].is_string());
    assert!(comment["created_at"].is_string());
    assert_eq!(
        comment["author"].as_object().expect("author object").len(),
        2
    );
    assert!(!object.contains_key("blocked"));
    assert!(!object.contains_key("can_block"));
}

async fn comment_count(pool: &MySqlPool, listing_id: u64) -> u32 {
    sqlx::query_scalar("SELECT comment_count FROM listings WHERE id = ?")
        .bind(listing_id)
        .fetch_one(pool)
        .await
        .expect("comment counter")
}

fn fixture_created_at() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp_micros(1_700_000_000_123_000).expect("valid fixture timestamp")
}

#[tokio::test]
async fn block_routes_require_session_and_mutations_require_csrf() {
    let Some(t) = setup().await else { return };
    let blocker = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let target = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let uri = block_uri(&target.public_id);

    for method in [Method::GET, Method::PUT, Method::DELETE] {
        let request_uri = if method == Method::GET {
            "/api/v1/me/blocks"
        } else {
            &uri
        };
        let (status, _, body) = send(t.app.clone(), method, request_uri, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "unauthorized");
    }
    for method in [Method::PUT, Method::DELETE] {
        let (status, _, body) = send(
            t.app.clone(),
            method,
            &uri,
            Some(&blocker.cookie),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"]["code"], "csrf_failed");
    }
    let (status, _, _) = send(
        t.app.clone(),
        Method::PUT,
        &uri,
        Some(&blocker.cookie),
        Some("wrong-token"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(block_rows(&t.pool, blocker.id, target.id).await, 0);

    // Safe reads do not require CSRF and are always private.
    let (status, headers, body) = list_blocks(t.app.clone(), &blocker, "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"], serde_json::json!([]));
    assert_eq!(
        headers
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("private, no-store")
    );
}

#[tokio::test]
async fn block_is_idempotent_and_list_is_private() {
    let Some(t) = setup().await else { return };
    let blocker = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let target = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let other = create_user(&t.pool, &t.cfg, "+989100000003").await;
    sqlx::query("UPDATE users SET display_name = 'کاربر مسدودشده' WHERE id = ?")
        .bind(target.id)
        .execute(&t.pool)
        .await
        .expect("display name fixture");

    for _ in 0..2 {
        let (status, _, body) =
            mutate_block(t.app.clone(), Method::PUT, &blocker, &target).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body, serde_json::json!({ "blocked": true }));
        assert_eq!(block_rows(&t.pool, blocker.id, target.id).await, 1);
    }
    let created_at: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT created_at FROM blocks WHERE blocker_id = ? AND blocked_id = ?")
            .bind(blocker.id)
            .bind(target.id)
            .fetch_one(&t.pool)
            .await
            .expect("original block timestamp");
    let (status, _, _) = mutate_block(t.app.clone(), Method::PUT, &blocker, &target).await;
    assert_eq!(status, StatusCode::OK);
    let after_duplicate: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT created_at FROM blocks WHERE blocker_id = ? AND blocked_id = ?")
            .bind(blocker.id)
            .bind(target.id)
            .fetch_one(&t.pool)
            .await
            .expect("duplicate block timestamp");
    assert_eq!(after_duplicate, created_at);

    let (status, _, blocker_page) = list_blocks(t.app.clone(), &blocker, "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(blocked_ids(&blocker_page), vec![target.public_id.clone()]);
    assert_public_block_shape(&blocker_page["data"][0], &target);
    assert_eq!(blocker_page["data"][0]["display_name"], "کاربر مسدودشده");
    for viewer in [&target, &other] {
        let (status, _, page) = list_blocks(t.app.clone(), viewer, "").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["data"], serde_json::json!([]));
    }
}

#[tokio::test]
async fn block_rejects_self_unknown_and_deleted_but_allows_suspended_target() {
    let Some(t) = setup().await else { return };
    let blocker = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let suspended = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let deleted = create_user(&t.pool, &t.cfg, "+989100000003").await;

    let (status, _, body) =
        mutate_block(t.app.clone(), Method::PUT, &blocker, &blocker).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
    assert_eq!(body["error"]["fields"]["public_id"], "self_block");

    for unknown in ["0".repeat(26), "not-a-public-id".to_string()] {
        let (status, _, body) = send(
            t.app.clone(),
            Method::PUT,
            &block_uri(&unknown),
            Some(&blocker.cookie),
            Some(&blocker.csrf),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
    }

    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = ?")
        .bind(suspended.id)
        .execute(&t.pool)
        .await
        .expect("suspended target fixture");
    let (status, _, body) =
        mutate_block(t.app.clone(), Method::PUT, &blocker, &suspended).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(block_rows(&t.pool, blocker.id, suspended.id).await, 1);

    sqlx::query("UPDATE users SET status = 'deleted', deleted_at = NOW(3) WHERE id = ?")
        .bind(deleted.id)
        .execute(&t.pool)
        .await
        .expect("deleted target fixture");
    let (status, _, _) = mutate_block(t.app.clone(), Method::PUT, &blocker, &deleted).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(block_rows(&t.pool, blocker.id, deleted.id).await, 0);
    assert_eq!(block_rows(&t.pool, blocker.id, blocker.id).await, 0);
}

#[tokio::test]
async fn unblock_is_idempotent_directional_and_allows_deleted_target() {
    let Some(t) = setup().await else { return };
    let first = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let second = create_user(&t.pool, &t.cfg, "+989100000002").await;
    for (blocker, target) in [(&first, &second), (&second, &first)] {
        let (status, _, _) = mutate_block(t.app.clone(), Method::PUT, blocker, target).await;
        assert_eq!(status, StatusCode::OK);
    }

    for _ in 0..2 {
        let (status, _, body) =
            mutate_block(t.app.clone(), Method::DELETE, &first, &second).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::json!({ "blocked": false }));
        assert_eq!(block_rows(&t.pool, first.id, second.id).await, 0);
        assert_eq!(block_rows(&t.pool, second.id, first.id).await, 1);
    }

    // A known deleted user remains listed and can still be unblocked.
    sqlx::query("UPDATE users SET status = 'deleted', deleted_at = NOW(3) WHERE id = ?")
        .bind(first.id)
        .execute(&t.pool)
        .await
        .expect("deleted blocked-user fixture");
    let (status, _, page) = list_blocks(t.app.clone(), &second, "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(blocked_ids(&page), vec![first.public_id.clone()]);
    let (status, _, body) =
        mutate_block(t.app.clone(), Method::DELETE, &second, &first).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({ "blocked": false }));
    assert_eq!(block_rows(&t.pool, second.id, first.id).await, 0);

    let (status, _, _) = send(
        t.app.clone(),
        Method::DELETE,
        &block_uri(&"0".repeat(26)),
        Some(&second.cookie),
        Some(&second.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn block_list_rejects_bad_cursor_and_survives_anchor_unblock() {
    let Some(t) = setup().await else { return };
    let blocker = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let mut targets = Vec::new();
    for phone in [
        "+989100000002",
        "+989100000003",
        "+989100000004",
        "+989100000005",
    ] {
        let target = create_user(&t.pool, &t.cfg, phone).await;
        insert_block(&t.pool, blocker.id, target.id).await;
        targets.push(target);
    }

    let (status, _, body) = list_blocks(t.app.clone(), &blocker, "?cursor=not-a-cursor").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
    assert_eq!(body["error"]["fields"]["cursor"], "invalid_cursor");

    let (status, _, first_page) = list_blocks(t.app.clone(), &blocker, "?limit=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_page["page"]["has_more"], true);
    assert_eq!(
        blocked_ids(&first_page),
        vec![targets[3].public_id.clone(), targets[2].public_id.clone()]
    );
    let cursor = first_page["page"]["next_cursor"]
        .as_str()
        .expect("next cursor")
        .to_string();

    // Removing the cursor anchor cannot skip or duplicate another equal-time row.
    let (status, _, _) =
        mutate_block(t.app.clone(), Method::DELETE, &blocker, &targets[2]).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, second_page) = list_blocks(
        t.app.clone(),
        &blocker,
        &format!("?limit=2&cursor={cursor}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_page["page"]["has_more"], false);
    assert!(second_page["page"]["next_cursor"].is_null());
    assert_eq!(
        blocked_ids(&second_page),
        vec![targets[1].public_id.clone(), targets[0].public_id.clone()]
    );
}

#[tokio::test]
async fn concurrent_duplicate_blocks_keep_one_row() {
    let Some(t) = setup().await else { return };
    let blocker = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let target = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let mut tasks = Vec::new();
    for _ in 0..6 {
        let app = t.app.clone();
        let cookie = blocker.cookie.clone();
        let csrf = blocker.csrf.clone();
        let uri = block_uri(&target.public_id);
        tasks.push(tokio::spawn(async move {
            send(
                app,
                Method::PUT,
                &uri,
                Some(&cookie),
                Some(&csrf),
            )
            .await
        }));
    }
    for task in tasks {
        let (status, _, body) = task.await.expect("block task");
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body, serde_json::json!({ "blocked": true }));
    }
    assert_eq!(block_rows(&t.pool, blocker.id, target.id).await, 1);
}

#[tokio::test]
async fn blocks_filter_only_blockers_comments_without_changing_public_state() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let blocked_author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let other_author = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let blocker = create_user(&t.pool, &t.cfg, "+989100000004").await;
    let other_viewer = create_user(&t.pool, &t.cfg, "+989100000005").await;
    let listing = create_listing(&t.pool, seller.id).await;
    insert_comment(
        &t.pool,
        listing.id,
        blocked_author.id,
        "blocked author comment",
    )
    .await;
    insert_comment(&t.pool, listing.id, other_author.id, "other author comment").await;
    assert_eq!(comment_count(&t.pool, listing.id).await, 2);

    let (status, _, body) =
        mutate_block(t.app.clone(), Method::PUT, &blocker, &blocked_author).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(comment_count(&t.pool, listing.id).await, 2);

    let (status, headers, anonymous) = list_comments(t.app.clone(), &listing, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comment_bodies(&anonymous).len(), 2);
    assert_ne!(
        headers
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("private, no-store")
    );
    for comment in anonymous["data"].as_array().expect("anonymous comments") {
        assert_public_comment_shape(comment);
    }

    let (status, headers, blocked_page) =
        list_comments(t.app.clone(), &listing, Some(&blocker)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comment_bodies(&blocked_page), vec!["other author comment"]);
    assert_eq!(
        headers
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("private, no-store")
    );
    assert!(headers
        .get(axum::http::header::VARY)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("cookie")));
    assert_public_comment_shape(&blocked_page["data"][0]);

    for viewer in [&other_viewer, &blocked_author] {
        let (status, _, page) = list_comments(t.app.clone(), &listing, Some(viewer)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(comment_bodies(&page).len(), 2);
    }
    let invalid_cookie = "shabdar_session=invalid; shabdar_csrf=invalid";
    let (status, _, invalid_session_page) = send(
        t.app.clone(),
        Method::GET,
        &format!("/api/v1/listings/{}/comments", listing.slug),
        Some(invalid_cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(invalid_session_page, anonymous);

    for viewer in [None, Some(&blocker)] {
        let (status, _, detail) = send(
            t.app.clone(),
            Method::GET,
            &format!("/api/v1/listings/{}", listing.slug),
            viewer.map(|user| user.cookie.as_str()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(detail["comment_count"], 2);
        assert!(detail.get("blocked").is_none());
        assert!(detail.get("can_block").is_none());
    }

    // Incoming blocks are irrelevant: only the viewer's outgoing rows filter.
    let (status, _, _) =
        mutate_block(t.app.clone(), Method::PUT, &blocked_author, &blocker).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) =
        mutate_block(t.app.clone(), Method::DELETE, &blocker, &blocked_author).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, unblocked_page) =
        list_comments(t.app.clone(), &listing, Some(&blocker)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comment_bodies(&unblocked_page).len(), 2);
    assert_eq!(comment_count(&t.pool, listing.id).await, 2);
}
