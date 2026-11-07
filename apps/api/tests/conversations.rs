//! Direct two-user chat: pair canonicalization, privacy, idempotent sending,
//! keysets, high-water Read/Seen, blocking, and Inbox synchronization.
//!
//! Everything runs against the real router and a real MariaDB scratch schema.
//! Message/Inbox seeding inserts fixture rows directly (fast, deterministic
//! timestamps); every read/write path under test goes through the HTTP API.

mod common;

use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::Router;
use chrono::{DateTime, Utc};
use http_body_util::BodyExt;
use shabdar_api::config::Config;
use sqlx::MySqlPool;
use tower::ServiceExt;

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const CLIENT_ID: &str = "123e4567-e89b-42d3-a456-426614174000";

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

async fn setup() -> Option<Setup> {
    let lock = TEST_LOCK.lock().await;
    let pool = common::test_pool().await?;
    common::reset_database(&pool).await;
    let cfg = common::test_config();
    let app = common::spawn_app(cfg.clone()).await;
    Some(Setup {
        _lock: lock,
        app,
        pool,
        cfg,
    })
}

async fn setup_with(tweak: impl FnOnce(&mut Config)) -> Option<Setup> {
    let lock = TEST_LOCK.lock().await;
    let pool = common::test_pool().await?;
    common::reset_database(&pool).await;
    let cfg = common::test_config_with(tweak);
    let app = common::spawn_app(cfg.clone()).await;
    Some(Setup {
        _lock: lock,
        app,
        pool,
        cfg,
    })
}

async fn create_user(pool: &MySqlPool, cfg: &Config, phone: &str, name: &str) -> TestUser {
    let public_id = ulid::Ulid::new().to_string();
    let result = sqlx::query(
        "INSERT INTO users (public_id, phone_e164, display_name, created_at, updated_at) \
         VALUES (?, ?, ?, NOW(3), NOW(3))",
    )
    .bind(&public_id)
    .bind(phone)
    .bind(name)
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

/// A counterpart without a session: Inbox/pagination fixtures never act.
async fn create_bare_user(pool: &MySqlPool, phone: &str, name: &str) -> TestUser {
    let public_id = ulid::Ulid::new().to_string();
    let result = sqlx::query(
        "INSERT INTO users (public_id, phone_e164, display_name, created_at, updated_at) \
         VALUES (?, ?, ?, NOW(3), NOW(3))",
    )
    .bind(&public_id)
    .bind(phone)
    .bind(name)
    .execute(pool)
    .await
    .expect("counterpart fixture");
    TestUser {
        id: result.last_insert_id(),
        public_id,
        cookie: String::new(),
        csrf: String::new(),
    }
}

async fn create_listing(pool: &MySqlPool, seller_id: u64, status: &str) -> ListingFixture {
    let public_id = ulid::Ulid::new().to_string();
    let slug = format!("chat-{}", ulid::Ulid::new());
    let result = sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, published_at, created_at, updated_at, \
         image_reviewed_by, image_reviewed_at) \
         VALUES (?, ?, 1, 1, 'آگهی گفتگو', ?, 'توضیحات', 1000, 'fixed', ?, NOW(3), NOW(3), \
         NOW(3), ?, NOW(3))",
    )
    .bind(&public_id)
    .bind(seller_id)
    .bind(&slug)
    .bind(status)
    .bind(seller_id)
    .execute(pool)
    .await
    .expect("listing fixture");
    ListingFixture {
        id: result.last_insert_id(),
        slug,
    }
}

async fn call(
    app: &Router,
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
    let request = match body {
        Some(body) => builder
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request"),
        None => builder.body(Body::empty()).expect("request"),
    };
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = BodyExt::collect(response.into_body())
        .await
        .expect("response body")
        .to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, headers, body)
}

async fn get_as(
    app: &Router,
    uri: &str,
    user: &TestUser,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    call(app, Method::GET, uri, Some(&user.cookie), None, None).await
}

async fn post_as(
    app: &Router,
    uri: &str,
    user: &TestUser,
    body: serde_json::Value,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    call(
        app,
        Method::POST,
        uri,
        Some(&user.cookie),
        Some(&user.csrf),
        Some(body),
    )
    .await
}

async fn start_conversation(
    app: &Router,
    user: &TestUser,
    slug: &str,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    call(
        app,
        Method::POST,
        &format!("/api/v1/listings/{slug}/conversations"),
        Some(&user.cookie),
        Some(&user.csrf),
        None,
    )
    .await
}

async fn send_message(
    app: &Router,
    user: &TestUser,
    conversation_public_id: &str,
    client_message_id: &str,
    body: &str,
    listing_slug: Option<&str>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    let mut payload = serde_json::json!({
        "client_message_id": client_message_id,
        "body": body,
    });
    if let Some(slug) = listing_slug {
        payload["listing_slug"] = serde_json::json!(slug);
    }
    post_as(
        app,
        &format!("/api/v1/conversations/{conversation_public_id}/messages"),
        user,
        payload,
    )
    .await
}

async fn mark_read(
    app: &Router,
    user: &TestUser,
    conversation_public_id: &str,
    message_public_id: &str,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    post_as(
        app,
        &format!("/api/v1/conversations/{conversation_public_id}/read"),
        user,
        serde_json::json!({ "through_message_id": message_public_id }),
    )
    .await
}

fn assert_private(headers: &HeaderMap) {
    assert_eq!(
        headers
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("private, no-store"),
        "private chat responses must never be cacheable"
    );
    assert_eq!(
        headers.get("vary").and_then(|value| value.to_str().ok()),
        Some("Cookie")
    );
}

async fn conversation_count(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM conversations")
        .fetch_one(pool)
        .await
        .expect("conversation count")
}

async fn message_count(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM messages")
        .fetch_one(pool)
        .await
        .expect("message count")
}

/// Seeds a fully-formed conversation with one message, returning
/// `(conversation_id, message_public_id)`.
async fn seed_conversation(
    pool: &MySqlPool,
    first: u64,
    second: u64,
    sender_id: u64,
    body: &str,
    created_at: DateTime<Utc>,
) -> (u64, String) {
    let (low, high) = if first < second {
        (first, second)
    } else {
        (second, first)
    };
    let conversation_public_id = ulid::Ulid::new().to_string();
    let conversation_id = sqlx::query(
        "INSERT INTO conversations (public_id, user_low_id, user_high_id, initiator_id, \
         created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&conversation_public_id)
    .bind(low)
    .bind(high)
    .bind(sender_id)
    .bind(created_at)
    .bind(created_at)
    .execute(pool)
    .await
    .expect("seed conversation")
    .last_insert_id();
    for user_id in [low, high] {
        sqlx::query("INSERT INTO conversation_reads (conversation_id, user_id) VALUES (?, ?)")
            .bind(conversation_id)
            .bind(user_id)
            .execute(pool)
            .await
            .expect("seed read row");
    }
    let message_public_id = ulid::Ulid::new().to_string();
    let message_id = sqlx::query(
        "INSERT INTO messages (public_id, conversation_id, sender_id, client_message_id, \
         body, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&message_public_id)
    .bind(conversation_id)
    .bind(sender_id)
    .bind(ulid::Ulid::new().to_string())
    .bind(body)
    .bind(created_at)
    .execute(pool)
    .await
    .expect("seed message")
    .last_insert_id();
    sqlx::query(
        "UPDATE conversations SET last_message_id = ?, last_message_at = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(message_id)
    .bind(created_at)
    .bind(created_at)
    .bind(conversation_id)
    .execute(pool)
    .await
    .expect("seed last message");
    (conversation_id, message_public_id)
}

async fn seed_messages(
    pool: &MySqlPool,
    conversation_id: u64,
    low: u64,
    high: u64,
    sender_id: u64,
    count: usize,
) -> Vec<String> {
    // All fixture timestamps sit in the past, so every real mutation is newer.
    let base = Utc::now() - chrono::Duration::days(2);
    let mut public_ids = Vec::with_capacity(count);
    let mut last = 0;
    for index in 0..count {
        let public_id = ulid::Ulid::new().to_string();
        let created = base + chrono::Duration::milliseconds(i64::try_from(index).unwrap_or(0));
        last = sqlx::query(
            "INSERT INTO messages (public_id, conversation_id, sender_id, client_message_id, \
             body, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&public_id)
        .bind(conversation_id)
        .bind(if index % 2 == 0 {
            sender_id
        } else {
            other_of(sender_id, low, high)
        })
        .bind(ulid::Ulid::new().to_string())
        .bind(format!("پیام {index}"))
        .bind(created)
        .execute(pool)
        .await
        .expect("seed message")
        .last_insert_id();
        public_ids.push(public_id);
    }
    sqlx::query(
        "UPDATE conversations SET last_message_id = ?, last_message_at = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(last)
    .bind(base + chrono::Duration::milliseconds(i64::try_from(count).unwrap_or(0)))
    .bind(base)
    .bind(conversation_id)
    .execute(pool)
    .await
    .expect("seed last message");
    public_ids
}

fn other_of(sender: u64, low: u64, high: u64) -> u64 {
    if sender == low {
        high
    } else {
        low
    }
}

async fn conversation_updated_at(pool: &MySqlPool, conversation_id: u64) -> DateTime<Utc> {
    sqlx::query_scalar("SELECT updated_at FROM conversations WHERE id = ?")
        .bind(conversation_id)
        .fetch_one(pool)
        .await
        .expect("conversation updated_at")
}

// --- Task 2: canonical pair, privacy, and block locking -------------------

#[tokio::test]
async fn start_is_canonical_in_both_directions() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000001", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000002", "خریدار").await;
    let seller_listing = create_listing(&setup.pool, seller.id, "active").await;
    let buyer_listing = create_listing(&setup.pool, buyer.id, "active").await;

    let (status, headers, body) =
        start_conversation(&setup.app, &buyer, &seller_listing.slug).await;
    assert_eq!(status, StatusCode::OK, "start failed: {body}");
    assert_private(&headers);
    let public_id = body["public_id"].as_str().expect("public_id").to_string();
    assert_eq!(body["other_party"]["public_id"], seller.public_id);
    assert_eq!(body["initiator_id"], serde_json::Value::Null);
    assert_eq!(body["can_message"], true);
    assert_eq!(body["viewer_blocked_other"], false);
    assert!(body["last_message_at"].is_null());

    // The seller reaches the same pair from a listing owned by the buyer.
    let (status, _, body) = start_conversation(&setup.app, &seller, &buyer_listing.slug).await;
    assert_eq!(status, StatusCode::OK, "reverse start failed: {body}");
    assert_eq!(body["public_id"], public_id, "one pair, one conversation");
    assert_eq!(body["other_party"]["public_id"], buyer.public_id);
    assert_eq!(conversation_count(&setup.pool).await, 1);

    // Repeated starts stay idempotent: no message, no timestamp movement.
    let (status, _, body) = start_conversation(&setup.app, &buyer, &seller_listing.slug).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["public_id"], public_id);
    assert_eq!(message_count(&setup.pool).await, 0);
    let initiator: u64 =
        sqlx::query_scalar("SELECT initiator_id FROM conversations WHERE public_id = ?")
            .bind(&public_id)
            .fetch_one(&setup.pool)
            .await
            .expect("initiator");
    assert_eq!(initiator, buyer.id, "initiator is the first caller only");

    let read_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM conversation_reads WHERE last_read_message_id IS NULL",
    )
    .fetch_one(&setup.pool)
    .await
    .expect("read rows");
    assert_eq!(read_rows, 2, "both participant read rows start NULL");
}

#[tokio::test]
async fn start_rejects_self_and_unavailable_listings() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000011", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000012", "خریدار").await;
    let active = create_listing(&setup.pool, seller.id, "active").await;
    let pending = create_listing(&setup.pool, seller.id, "pending").await;
    let removed = create_listing(&setup.pool, seller.id, "removed").await;
    let sold = create_listing(&setup.pool, seller.id, "sold").await;

    let (status, headers, body) = start_conversation(&setup.app, &seller, &active.slug).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "self contact: {body}");
    assert_private(&headers);

    let (status, _, _) = start_conversation(&setup.app, &buyer, "unknown-slug").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = start_conversation(&setup.app, &buyer, &pending.slug).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = start_conversation(&setup.app, &buyer, &sold.slug).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = start_conversation(&setup.app, &buyer, &removed.slug).await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(conversation_count(&setup.pool).await, 0);

    // Soft deletion is the same 410 as removal.
    sqlx::query("UPDATE listings SET deleted_at = NOW(3) WHERE id = ?")
        .bind(active.id)
        .execute(&setup.pool)
        .await
        .expect("soft delete");
    let (status, _, _) = start_conversation(&setup.app, &buyer, &active.slug).await;
    assert_eq!(status, StatusCode::GONE);

    // Unreviewed images hide the listing from contact exactly like Detail.
    let unreviewed = create_listing(&setup.pool, seller.id, "active").await;
    sqlx::query(
        "UPDATE listings SET image_reviewed_at = NULL, image_reviewed_by = NULL WHERE id = ?",
    )
    .bind(unreviewed.id)
    .execute(&setup.pool)
    .await
    .expect("clear image review");
    sqlx::query(
        "INSERT INTO listing_images (listing_id, storage_key, alt_text, sort_order, width, \
         height, created_at) VALUES (?, 'unreviewed.png', 'x', 0, 800, 600, NOW(3))",
    )
    .bind(unreviewed.id)
    .execute(&setup.pool)
    .await
    .expect("listing image");
    let (status, _, _) = start_conversation(&setup.app, &buyer, &unreviewed.slug).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn start_requires_csrf_and_returns_private_errors() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000021", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000022", "خریدار").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let uri = format!("/api/v1/listings/{}/conversations", listing.slug);

    let (status, headers, body) = call(
        &setup.app,
        Method::POST,
        &uri,
        Some(&buyer.cookie),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_private(&headers);
    let (status, headers, _) = call(
        &setup.app,
        Method::POST,
        &uri,
        Some(&buyer.cookie),
        Some("wrong"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_private(&headers);

    let (status, headers, body) = call(&setup.app, Method::POST, &uri, None, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_private(&headers);
    assert_eq!(
        conversation_count(&setup.pool).await,
        0,
        "CSRF precedes any write"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM conversation_reads")
            .fetch_one(&setup.pool)
            .await
            .expect("read rows"),
        0
    );
}

#[tokio::test]
async fn metadata_hides_nonparticipants_and_account_state() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000031", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000032", "خریدار").await;
    let outsider = create_user(&setup.pool, &setup.cfg, "+989130000033", "ناظر").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();

    let (status, headers, unknown) = get_as(
        &setup.app,
        "/api/v1/conversations/UNKNOWN00000000000000000",
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_private(&headers);
    let (status, headers, foreign) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}"),
        &outsider,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_private(&headers);
    assert_eq!(
        unknown["error"]["code"], foreign["error"]["code"],
        "non-participant and unknown must be indistinguishable"
    );

    let (status, _, body) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}"),
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["can_message"], true);

    // A suspended counterpart keeps the conversation readable but not writable.
    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = ?")
        .bind(seller.id)
        .execute(&setup.pool)
        .await
        .expect("suspend seller");
    let (status, _, body) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}"),
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["can_message"], false);
}

#[tokio::test]
async fn block_sync_only_on_real_change_and_start_is_blocked() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000041", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000042", "خریدار").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();
    let conversation_id: u64 =
        sqlx::query_scalar("SELECT id FROM conversations WHERE public_id = ?")
            .bind(&public_id)
            .fetch_one(&setup.pool)
            .await
            .expect("conversation id");
    let before = conversation_updated_at(&setup.pool, conversation_id).await;

    let uri = format!("/api/v1/me/blocks/{}", seller.public_id);
    let (status, _, _) = call(
        &setup.app,
        Method::PUT,
        &uri,
        Some(&buyer.cookie),
        Some(&buyer.csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let after_first = conversation_updated_at(&setup.pool, conversation_id).await;
    assert!(
        after_first > before,
        "a real block advances synchronization"
    );

    let (status, _, _) = call(
        &setup.app,
        Method::PUT,
        &uri,
        Some(&buyer.cookie),
        Some(&buyer.csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        conversation_updated_at(&setup.pool, conversation_id).await,
        after_first,
        "an idempotent block never advances synchronization"
    );

    // A blocked pair cannot start and cannot send in either direction.
    let (status, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "conversation_blocked");
    let (status, _, body) = start_conversation(&setup.app, &seller, &listing.slug).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "reverse start also blocked: {body}"
    );

    let (status, _, _) = call(
        &setup.app,
        Method::DELETE,
        &uri,
        Some(&buyer.cookie),
        Some(&buyer.csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let after_unblock = conversation_updated_at(&setup.pool, conversation_id).await;
    assert!(after_unblock > after_first);
    let (status, _, _) = call(
        &setup.app,
        Method::DELETE,
        &uri,
        Some(&buyer.cookie),
        Some(&buyer.csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        conversation_updated_at(&setup.pool, conversation_id).await,
        after_unblock,
        "an idempotent unblock never advances synchronization"
    );
}

// --- Task 3: validated idempotent sending --------------------------------

#[tokio::test]
async fn send_validation_boundaries() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000051", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000052", "خریدار").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();

    let longest = "س".repeat(1000);
    let (status, headers, body) =
        send_message(&setup.app, &buyer, &public_id, CLIENT_ID, &longest, None).await;
    assert_eq!(status, StatusCode::OK, "1000 scalars must pass: {body}");
    assert_private(&headers);
    assert_eq!(body["message"]["body"], longest);
    assert_eq!(body["message"]["seen"], false);
    assert_eq!(body["conversation"]["can_message"], true);

    for (client_id, text, field) in [
        (CLIENT_ID, "س".repeat(1001), "body"),
        (
            "123e4567-e89b-42d3-a456-426614174001",
            "   ".to_string(),
            "body",
        ),
        (
            "123e4567-e89b-42d3-a456-426614174002",
            "a\u{0}b".to_string(),
            "body",
        ),
        (
            "123e4567-e89b-42d3-a456-426614174003",
            "a\rb".to_string(),
            "body",
        ),
        (
            "123e4567-e89b-42d3-a456-426614174004",
            "a\tb".to_string(),
            "body",
        ),
        ("not-a-uuid", "سلام".to_string(), "client_message_id"),
        ("", "سلام".to_string(), "client_message_id"),
    ] {
        let (status, headers, body) =
            send_message(&setup.app, &buyer, &public_id, client_id, &text, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text:?} must be rejected");
        assert_private(&headers);
        assert!(
            body["error"]["fields"].get(field).is_some(),
            "expected a {field} field error, got {body}"
        );
    }

    // Multiline content is preserved, and unknown fields are rejected.
    let (status, _, body) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        "123e4567-e89b-42d3-a456-426614174005",
        "خط اول\nخط دوم",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"]["body"], "خط اول\nخط دوم");

    let (status, _, body) = post_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages"),
        &buyer,
        serde_json::json!({
            "client_message_id": "123e4567-e89b-42d3-a456-426614174006",
            "body": "سلام",
            "unexpected": true,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_request");
    assert_eq!(
        message_count(&setup.pool).await,
        2,
        "only the valid sends wrote rows"
    );
}

#[tokio::test]
async fn send_is_idempotent_and_conflicts_safely() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000061", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000062", "خریدار").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();

    let (status, _, first) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        CLIENT_ID,
        "  سلام، موجود است؟  ",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let first_id = first["message"]["public_id"]
        .as_str()
        .expect("id")
        .to_string();
    assert_eq!(
        first["message"]["body"], "سلام، موجود است؟",
        "body is trimmed"
    );

    let (status, _, replay) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        CLIENT_ID,
        "سلام، موجود است؟",
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "identical replay succeeds: {replay}"
    );
    assert_eq!(replay["message"]["public_id"], first_id);
    assert_eq!(message_count(&setup.pool).await, 1);
    let last_at: DateTime<Utc> =
        sqlx::query_scalar("SELECT last_message_at FROM conversations WHERE public_id = ?")
            .bind(&public_id)
            .fetch_one(&setup.pool)
            .await
            .expect("last_message_at");
    let (status, _, _) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        CLIENT_ID,
        "متن تغییر کرد",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let last_after: DateTime<Utc> =
        sqlx::query_scalar("SELECT last_message_at FROM conversations WHERE public_id = ?")
            .bind(&public_id)
            .fetch_one(&setup.pool)
            .await
            .expect("last_message_at");
    assert_eq!(
        last_at, last_after,
        "replays and conflicts never move the Inbox"
    );

    let (status, _, conflict) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        CLIENT_ID,
        "متن تغییر کرد",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(conflict["error"]["fields"]["client_message_id"], "conflict");
    assert_eq!(message_count(&setup.pool).await, 1);

    // The UUID is scoped per sender and conversation.
    let (status, _, other) = send_message(
        &setup.app,
        &seller,
        &public_id,
        CLIENT_ID,
        "پاسخ فروشنده",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{other}");
    assert_ne!(other["message"]["public_id"], first_id);
    assert_eq!(message_count(&setup.pool).await, 2);
}

#[tokio::test]
async fn listing_reference_rules_and_lifecycle() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000071", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000072", "خریدار").await;
    let third = create_user(&setup.pool, &setup.cfg, "+989130000073", "شخص سوم").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let second_listing = create_listing(&setup.pool, seller.id, "active").await;
    let foreign_listing = create_listing(&setup.pool, third.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();

    let (status, _, body) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        "123e4567-e89b-42d3-a456-426614174011",
        "دربارهٔ این آگهی",
        Some(&second_listing.slug),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["message"]["listing"]["slug"], second_listing.slug);
    assert_eq!(body["message"]["listing"]["available"], true);

    for (index, slug) in [foreign_listing.slug.as_str(), "unknown-listing-slug"]
        .into_iter()
        .enumerate()
    {
        let client_id = format!("123e4567-e89b-42d3-a456-42661417402{index}");
        let (status, _, body) = send_message(
            &setup.app,
            &buyer,
            &public_id,
            &client_id,
            "مرجع نامعتبر",
            Some(slug),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["fields"]["listing_slug"], "invalid");
    }
    // A seller can never reference their own listing.
    let (status, _, body) = send_message(
        &setup.app,
        &seller,
        &public_id,
        "123e4567-e89b-42d3-a456-426614174031",
        "آگهی خودم",
        Some(&listing.slug),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["listing_slug"], "invalid");

    // Every later listing state keeps the reference as context; only the link
    // availability changes and the thread stays writable.
    for (state_index, status) in ["sold", "archived", "removed"].into_iter().enumerate() {
        sqlx::query("UPDATE listings SET status = ? WHERE id = ?")
            .bind(status)
            .bind(second_listing.id)
            .execute(&setup.pool)
            .await
            .expect("listing lifecycle");
        let (status_code, _, page) = get_as(
            &setup.app,
            &format!("/api/v1/conversations/{public_id}/messages"),
            &buyer,
        )
        .await;
        assert_eq!(status_code, StatusCode::OK);
        let reference = &page["data"][0]["listing"];
        assert_eq!(reference["slug"], second_listing.slug);
        assert_eq!(
            reference["available"], false,
            "state {status} removes the link"
        );
        let client_id = format!("123e4567-e89b-42d3-a456-4266141741{state_index}0");
        let (send_status, _, sent) = send_message(
            &setup.app,
            &buyer,
            &public_id,
            &client_id,
            "ادامهٔ گفتگو",
            None,
        )
        .await;
        assert_eq!(send_status, StatusCode::OK, "thread stays writable: {sent}");
    }
    sqlx::query("UPDATE listings SET deleted_at = NOW(3) WHERE id = ?")
        .bind(second_listing.id)
        .execute(&setup.pool)
        .await
        .expect("soft delete");
    let (_, _, page) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages"),
        &buyer,
    )
    .await;
    assert_eq!(page["data"][0]["listing"]["available"], false);
    assert_eq!(page["data"][0]["listing"]["slug"], second_listing.slug);
}

#[tokio::test]
async fn send_requires_auth_csrf_and_participation() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000081", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000082", "خریدار").await;
    let outsider = create_user(&setup.pool, &setup.cfg, "+989130000083", "ناظر").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();
    let uri = format!("/api/v1/conversations/{public_id}/messages");
    let payload = serde_json::json!({ "client_message_id": CLIENT_ID, "body": "سلام" });

    let (status, headers, _) = call(
        &setup.app,
        Method::POST,
        &uri,
        None,
        None,
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_private(&headers);
    let (status, _, _) = call(
        &setup.app,
        Method::POST,
        &uri,
        Some(&buyer.cookie),
        None,
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "CSRF is required");
    let (status, _, body) = call(
        &setup.app,
        Method::POST,
        &uri,
        Some(&outsider.cookie),
        Some(&outsider.csrf),
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "non-participant: {body}");

    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = ?")
        .bind(seller.id)
        .execute(&setup.pool)
        .await
        .expect("suspend");
    let (status, _, body) = call(
        &setup.app,
        Method::POST,
        &uri,
        Some(&buyer.cookie),
        Some(&buyer.csrf),
        Some(payload),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "inactive counterpart: {body}"
    );
    assert_eq!(message_count(&setup.pool).await, 0);
}

#[tokio::test]
async fn send_rate_limit_counts_only_validated_requests() {
    let Some(setup) = setup_with(|cfg| {
        cfg.message_user_limit = 2;
        cfg.message_user_window_secs = 600;
    })
    .await
    else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000091", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000092", "خریدار").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();

    // Malformed and unauthorized attempts must not spend the allowance.
    let (status, _, _) =
        send_message(&setup.app, &buyer, &public_id, "bad-uuid", "سلام", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = post_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages"),
        &buyer,
        serde_json::json!({ "client_message_id": CLIENT_ID, "body": "سلام", "x": 1 }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _, _) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        "123e4567-e89b-42d3-a456-426614174041",
        "اول",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        "123e4567-e89b-42d3-a456-426614174042",
        "دوم",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // The third validated request — even an idempotent replay — is limited.
    let (status, headers, body) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        "123e4567-e89b-42d3-a456-426614174041",
        "اول",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    let retry_after = headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .expect("retry-after")
        .parse::<u64>()
        .expect("numeric retry-after");
    assert!(retry_after >= 1);
    assert_eq!(message_count(&setup.pool).await, 2, "429 wrote nothing");
}

#[tokio::test]
async fn concurrent_identical_sends_collapse_to_one_row() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000101", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000102", "خریدار").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();
    let updated_before: DateTime<Utc> =
        sqlx::query_scalar("SELECT updated_at FROM conversations WHERE public_id = ?")
            .bind(&public_id)
            .fetch_one(&setup.pool)
            .await
            .expect("updated_at");

    let app_one = setup.app.clone();
    let app_two = setup.app.clone();
    let cookie_one = buyer.cookie.clone();
    let cookie_two = buyer.cookie.clone();
    let csrf_one = buyer.csrf.clone();
    let csrf_two = buyer.csrf.clone();
    let uri = format!("/api/v1/conversations/{public_id}/messages");
    let payload = serde_json::json!({ "client_message_id": CLIENT_ID, "body": "هم‌زمان" });
    let spawn = |app: Router, cookie: String, csrf: String, payload: serde_json::Value| {
        let uri = uri.clone();
        tokio::spawn(async move {
            call(
                &app,
                Method::POST,
                &uri,
                Some(&cookie),
                Some(&csrf),
                Some(payload),
            )
            .await
            .2
        })
    };
    let (first, second) = tokio::join!(
        spawn(app_one, cookie_one, csrf_one, payload.clone()),
        spawn(app_two, cookie_two, csrf_two, payload)
    );
    let first = first.expect("first send task");
    let second = second.expect("second send task");
    assert_eq!(
        first["message"]["public_id"], second["message"]["public_id"],
        "both responses must reference the same row: {first} / {second}"
    );
    assert_eq!(message_count(&setup.pool).await, 1);
    let updated_after: DateTime<Utc> =
        sqlx::query_scalar("SELECT updated_at FROM conversations WHERE public_id = ?")
            .bind(&public_id)
            .fetch_one(&setup.pool)
            .await
            .expect("updated_at");
    assert!(updated_after >= updated_before);
}

#[tokio::test]
async fn blocked_pair_keeps_history_and_replays_confirmed_receipt() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000111", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000112", "خریدار").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();
    let (status, _, _) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        CLIENT_ID,
        "قبل از مسدودسازی",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let uri = format!("/api/v1/me/blocks/{}", seller.public_id);
    let (status, _, _) = call(
        &setup.app,
        Method::PUT,
        &uri,
        Some(&buyer.cookie),
        Some(&buyer.csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // A confirmed receipt still replays after the block; nothing new is written.
    let (status, _, replay) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        CLIENT_ID,
        "قبل از مسدودسازی",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(message_count(&setup.pool).await, 1);

    // Sending in either direction is rejected.
    let (status, _, body) = send_message(
        &setup.app,
        &buyer,
        &public_id,
        "123e4567-e89b-42d3-a456-426614174051",
        "بعد از مسدودسازی",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "conversation_blocked");
    let (status, _, _) = send_message(
        &setup.app,
        &seller,
        &public_id,
        "123e4567-e89b-42d3-a456-426614174052",
        "پاسخ",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // History and Read remain available while blocked.
    let (status, _, page) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages"),
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["data"].as_array().expect("data").len(), 1);
    let message_public_id = page["data"][0]["public_id"].as_str().expect("id");
    let (status, _, receipt) = mark_read(&setup.app, &buyer, &public_id, message_public_id).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
}

#[tokio::test]
async fn send_waits_for_committed_block() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000121", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000122", "خریدار").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();

    // Hold both participant rows, exactly like a block transaction would.
    let mut holding = setup.pool.begin().await.expect("holding transaction");
    for user_id in [buyer.id.min(seller.id), buyer.id.max(seller.id)] {
        sqlx::query("SELECT id FROM users WHERE id = ? FOR UPDATE")
            .bind(user_id)
            .fetch_one(&mut *holding)
            .await
            .expect("lock user row");
    }

    let app = setup.app.clone();
    let cookie = buyer.cookie.clone();
    let csrf = buyer.csrf.clone();
    let uri = format!("/api/v1/conversations/{public_id}/messages");
    let pending = tokio::spawn(async move {
        call(
            &app,
            Method::POST,
            &uri,
            Some(&cookie),
            Some(&csrf),
            Some(serde_json::json!({ "client_message_id": CLIENT_ID, "body": "هم‌زمان با مسدودسازی" })),
        )
        .await
        .0
    });

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!pending.is_finished(), "send must wait for the pair locks");

    let (low, high) = if buyer.id < seller.id {
        (buyer.id, seller.id)
    } else {
        (seller.id, buyer.id)
    };
    sqlx::query("INSERT INTO blocks (blocker_id, blocked_id, created_at) VALUES (?, ?, NOW(3))")
        .bind(low)
        .bind(high)
        .execute(&mut *holding)
        .await
        .expect("block inside holding transaction");
    holding.commit().await.expect("commit block");

    let status = tokio::time::timeout(Duration::from_secs(10), pending)
        .await
        .expect("send returns after the block commits")
        .expect("send task");
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(message_count(&setup.pool).await, 0);
}

// --- Task 4: keysets, Read, Seen, listing lifecycle ----------------------

#[tokio::test]
async fn message_pages_are_disjoint_and_cursors_are_scoped() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000131", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000132", "خریدار").await;
    let outsider = create_user(&setup.pool, &setup.cfg, "+989130000133", "ناظر").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();
    let (low, high) = if buyer.id < seller.id {
        (buyer.id, seller.id)
    } else {
        (seller.id, buyer.id)
    };
    let conversation_id: u64 =
        sqlx::query_scalar("SELECT id FROM conversations WHERE public_id = ?")
            .bind(&public_id)
            .fetch_one(&setup.pool)
            .await
            .expect("conversation id");
    let seeded = seed_messages(&setup.pool, conversation_id, low, high, seller.id, 65).await;
    assert_eq!(seeded.len(), 65);

    let (status, headers, page) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages"),
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_private(&headers);
    let data = page["data"].as_array().expect("data").to_vec();
    assert_eq!(data.len(), 30);
    assert_eq!(page["page"]["has_more_older"], true);
    assert_eq!(page["page"]["has_more_newer"], false);
    assert_eq!(data.last().expect("newest")["public_id"], seeded[64]);
    assert_eq!(data.first().expect("oldest")["public_id"], seeded[35]);
    let older_cursor = page["page"]["older_cursor"].as_str().expect("older_cursor");
    let newer_cursor = page["page"]["newer_cursor"].as_str().expect("newer_cursor");

    let mut rendered = data
        .iter()
        .map(|item| item["public_id"].as_str().expect("id").to_string())
        .collect::<Vec<_>>();
    let mut cursor = older_cursor.to_string();
    for _ in 0..3 {
        let (status, _, page) = get_as(
            &setup.app,
            &format!("/api/v1/conversations/{public_id}/messages?before={cursor}&limit=30"),
            &buyer,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let items = page["data"].as_array().expect("data").to_vec();
        if items.is_empty() {
            assert_eq!(page["page"]["has_more_older"], false);
            break;
        }
        for item in &items {
            let id = item["public_id"].as_str().expect("id").to_string();
            assert!(!rendered.contains(&id), "pages must stay disjoint: {id}");
            rendered.push(id);
        }
        match page["page"]["older_cursor"].as_str() {
            Some(next) => cursor = next.to_string(),
            None => break,
        }
    }
    assert_eq!(rendered.len(), 65, "every seeded message is reachable");

    // Forward catch-up continues beyond one page.
    let (status, _, page) = get_as(
        &setup.app,
        &format!(
            "/api/v1/conversations/{public_id}/messages?after={}&limit=30",
            seeded[5]
        ),
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        page["data"][0]["public_id"], seeded[6],
        "after must be strict and ascending"
    );
    assert_eq!(page["page"]["has_more_newer"], true);
    let next = page["page"]["newer_cursor"].as_str().expect("newer_cursor");
    let (_, _, second) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages?after={next}&limit=30"),
        &buyer,
    )
    .await;
    assert_eq!(second["data"][0]["public_id"], seeded[36]);
    assert!(newer_cursor.starts_with("m:"));

    // Cursor scope, mutual exclusion, and clamping.
    let foreign = format!("m:{}", seeded[0]);
    let (status, _, body) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages?before={foreign}&after={foreign}"),
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, _, _) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages?before=garbage"),
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages"),
        &outsider,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _, clamp) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages?limit=999"),
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(clamp["data"].as_array().expect("data").len(), 50);
    let (status, _, clamp) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages?limit=0"),
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(clamp["data"].as_array().expect("data").len(), 1);
}

#[tokio::test]
async fn read_is_monotonic_and_drives_seen_and_unread() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000141", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000142", "خریدار").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();

    let (_, _, first) = send_message(&setup.app, &seller, &public_id, CLIENT_ID, "سلام", None).await;
    let first_id = first["message"]["public_id"]
        .as_str()
        .expect("id")
        .to_string();
    let (_, _, second) = send_message(
        &setup.app,
        &seller,
        &public_id,
        "123e4567-e89b-42d3-a456-426614174061",
        "هنوز موجود است",
        None,
    )
    .await;
    let second_id = second["message"]["public_id"]
        .as_str()
        .expect("id")
        .to_string();

    let (status, _, page) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages"),
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        page["read_state"]["viewer_read_through"],
        serde_json::Value::Null
    );
    assert_eq!(page["data"].as_array().expect("data").len(), 2);

    // Sender does not see "seen" before the other party reads.
    let (_, _, sender_view) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages"),
        &seller,
    )
    .await;
    assert_eq!(sender_view["data"][0]["seen"], false);

    let (status, _, receipt) = mark_read(&setup.app, &buyer, &public_id, &first_id).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["read_through"], first_id);
    assert_eq!(
        receipt["unread_count"], 1,
        "one newer incoming message remains"
    );

    // Older and duplicate reads are no-ops that return the current state.
    let (status, _, repeat) = mark_read(&setup.app, &buyer, &public_id, &first_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeat["read_through"], first_id);

    let (status, _, receipt) = mark_read(&setup.app, &buyer, &public_id, &second_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["unread_count"], 0);
    assert!(receipt["read_at"].is_string());

    // The sender now sees Seen on both of their messages.
    let (_, _, sender_view) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages"),
        &seller,
    )
    .await;
    assert_eq!(sender_view["data"][0]["seen"], true);
    assert_eq!(sender_view["data"][1]["seen"], true);
    assert_eq!(sender_view["read_state"]["other_read_through"], second_id);

    // A message arriving after the Read stays unread.
    let (_, _, third) = send_message(
        &setup.app,
        &seller,
        &public_id,
        "123e4567-e89b-42d3-a456-426614174062",
        "پیام تازه",
        None,
    )
    .await;
    let third_id = third["message"]["public_id"]
        .as_str()
        .expect("id")
        .to_string();
    let (status, _, receipt) = mark_read(&setup.app, &buyer, &public_id, &second_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        receipt["read_through"], second_id,
        "the older Read must not swallow the arriving message"
    );
    assert_eq!(receipt["unread_count"], 1);
    let (_, _, receipt) = mark_read(&setup.app, &buyer, &public_id, &third_id).await;
    assert_eq!(receipt["unread_count"], 0);

    // Read validation and CSRF never advance state.
    let (status, _, body) = post_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/read"),
        &buyer,
        serde_json::json!({ "through_message_id": "unknown-message" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, _, body) = post_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/read"),
        &buyer,
        serde_json::json!({ "through_message_id": third_id, "extra": true }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_request");
    let (status, _, _) = call(
        &setup.app,
        Method::POST,
        &format!("/api/v1/conversations/{public_id}/read"),
        Some(&buyer.cookie),
        None,
        Some(serde_json::json!({ "through_message_id": third_id })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT m.public_id FROM conversation_reads cr JOIN messages m \
             ON m.id = cr.last_read_message_id WHERE cr.user_id = ?"
        )
        .bind(buyer.id)
        .fetch_one(&setup.pool)
        .await
        .expect("read state"),
        Some(third_id)
    );
}

#[tokio::test]
async fn empty_thread_starts_with_null_read_state() {
    let Some(setup) = setup().await else {
        return;
    };
    let seller = create_user(&setup.pool, &setup.cfg, "+989130000151", "فروشنده").await;
    let buyer = create_user(&setup.pool, &setup.cfg, "+989130000152", "خریدار").await;
    let listing = create_listing(&setup.pool, seller.id, "active").await;
    let (_, _, body) = start_conversation(&setup.app, &buyer, &listing.slug).await;
    let public_id = body["public_id"].as_str().expect("public_id").to_string();
    let (status, _, page) = get_as(
        &setup.app,
        &format!("/api/v1/conversations/{public_id}/messages"),
        &buyer,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["data"].as_array().expect("data").len(), 0);
    assert!(page["page"]["older_cursor"].is_null());
    assert!(page["page"]["newer_cursor"].is_null());
    assert!(page["read_state"]["viewer_read_through"].is_null());
    assert!(page["read_state"]["other_read_through"].is_null());
}

// --- Task 5: Inbox keysets and synchronization ---------------------------

#[tokio::test]
async fn inbox_pages_unread_previews_and_sync_cursor() {
    let Some(setup) = setup().await else {
        return;
    };
    let viewer = create_user(&setup.pool, &setup.cfg, "+989130000161", "خریدار").await;
    let base = Utc::now() - chrono::Duration::days(2);
    let mut seeded = Vec::new();
    for index in 0..25u32 {
        let counterpart =
            create_bare_user(&setup.pool, &format!("+9891300101{index:02}"), "طرف مقابل").await;
        // Two equal timestamps prove the ID tie-breaker.
        let created = base + chrono::Duration::minutes(i64::from(index / 2));
        let (conversation_id, message_public_id) = seed_conversation(
            &setup.pool,
            viewer.id,
            counterpart.id,
            counterpart.id,
            &"ب".repeat(200),
            created,
        )
        .await;
        seeded.push((conversation_id, message_public_id, created));
    }
    // An empty conversation (no message) must never appear in the Inbox.
    let empty_pair = create_bare_user(&setup.pool, "+989130019999", "بدون پیام").await;
    let (low, high) = if viewer.id < empty_pair.id {
        (viewer.id, empty_pair.id)
    } else {
        (empty_pair.id, viewer.id)
    };
    sqlx::query(
        "INSERT INTO conversations (public_id, user_low_id, user_high_id, initiator_id, \
         created_at, updated_at) VALUES (?, ?, ?, ?, NOW(3), NOW(3))",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(low)
    .bind(high)
    .bind(viewer.id)
    .execute(&setup.pool)
    .await
    .expect("empty conversation");

    let (status, headers, page) = get_as(&setup.app, "/api/v1/me/conversations", &viewer).await;
    assert_eq!(status, StatusCode::OK);
    assert_private(&headers);
    let data = page["data"].as_array().expect("data").to_vec();
    assert_eq!(data.len(), 20);
    assert_eq!(page["page"]["has_more_older"], true);
    assert!(page["page"]["older_cursor"].is_string());
    assert!(page["page"]["sync_cursor"].is_string());
    assert_eq!(page["page"]["has_more_after"], false);

    let preview = data[0]["last_message"]["body_preview"]
        .as_str()
        .expect("preview");
    assert_eq!(
        preview.chars().count(),
        160,
        "preview is bounded by scalars"
    );
    assert_eq!(data[0]["unread_count"], 1, "incoming only");
    assert!(data[0]["last_message"]["sender_public_id"]
        .as_str()
        .is_some());
    let serialized = page.to_string();
    assert!(!serialized.contains("+9891"), "no phone numbers leak");
    assert!(!serialized.contains("last_read_message_id"));
    assert!(!serialized.contains("user_low_id"));

    // Recency order: newest first, equal timestamps tie-broken by id.
    let mut previous: Option<(DateTime<Utc>, i64)> = None;
    for item in &data {
        let at = DateTime::parse_from_rfc3339(item["last_message_at"].as_str().expect("at"))
            .expect("rfc3339")
            .with_timezone(&Utc);
        let conversation_public = item["public_id"].as_str().expect("id");
        let id: u64 = sqlx::query_scalar("SELECT id FROM conversations WHERE public_id = ?")
            .bind(conversation_public)
            .fetch_one(&setup.pool)
            .await
            .expect("conversation id");
        if let Some((previous_at, previous_id)) = previous {
            assert!(
                at < previous_at || (at == previous_at && (id as i64) < previous_id),
                "recency page must be strictly descending"
            );
        }
        previous = Some((at, id as i64));
    }

    let older = page["page"]["older_cursor"].as_str().expect("older_cursor");
    let (status, _, next) = get_as(
        &setup.app,
        &format!("/api/v1/me/conversations?before={older}"),
        &viewer,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let rest = next["data"].as_array().expect("data").to_vec();
    assert_eq!(rest.len(), 5);
    assert_eq!(next["page"]["has_more_older"], false);
    let first_page: Vec<&str> = data
        .iter()
        .filter_map(|item| item["public_id"].as_str())
        .collect();
    for item in &rest {
        let id = item["public_id"].as_str().expect("id");
        assert!(!first_page.contains(&id), "Inbox pages stay disjoint");
    }

    // Self-sent messages never inflate the caller's unread count.
    let own_conversation = seeded[0].0;
    let own_public_id: String =
        sqlx::query_scalar("SELECT public_id FROM conversations WHERE id = ?")
            .bind(own_conversation)
            .fetch_one(&setup.pool)
            .await
            .expect("own conversation");
    let (status, _, sent) = send_message(
        &setup.app,
        &viewer,
        &own_public_id,
        CLIENT_ID,
        "پیام خودم",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sent}");
    let (_, _, page) = get_as(&setup.app, "/api/v1/me/conversations?limit=50", &viewer).await;
    let own = page["data"]
        .as_array()
        .expect("data")
        .iter()
        .find(|item| item["public_id"] == own_public_id.as_str())
        .expect("own conversation present")
        .clone();
    assert_eq!(own["unread_count"], 1, "own message never counts as unread");
}

#[tokio::test]
async fn inbox_forward_pages_and_read_sync_without_reorder() {
    let Some(setup) = setup().await else {
        return;
    };
    let viewer = create_user(&setup.pool, &setup.cfg, "+989130000171", "خریدار").await;

    // Empty Inbox first: the snapshot is the epoch sentinel, so everything
    // created afterwards is reachable through `after`.
    let (status, _, initial) = get_as(&setup.app, "/api/v1/me/conversations", &viewer).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(initial["data"].as_array().expect("data").len(), 0);
    let sync_cursor = initial["page"]["sync_cursor"]
        .as_str()
        .expect("sync_cursor")
        .to_string();

    let base = Utc::now() - chrono::Duration::days(1);
    let mut seeded = Vec::new();
    for index in 0..55u32 {
        let counterpart =
            create_bare_user(&setup.pool, &format!("+9891300201{index:02}"), "طرف مقابل").await;
        let created = base + chrono::Duration::seconds(i64::from(index));
        let (conversation_id, message_public_id) = seed_conversation(
            &setup.pool,
            viewer.id,
            counterpart.id,
            counterpart.id,
            "پیام",
            created,
        )
        .await;
        seeded.push((
            conversation_id,
            message_public_id,
            counterpart.public_id.clone(),
        ));
    }

    let mut seen = Vec::new();
    let mut cursor = sync_cursor;
    for page_index in 0..3 {
        let (status, _, page) = get_as(
            &setup.app,
            &format!("/api/v1/me/conversations?after={cursor}"),
            &viewer,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "forward page {page_index}");
        for item in page["data"].as_array().expect("data") {
            let id = item["public_id"].as_str().expect("id").to_string();
            assert!(!seen.contains(&id), "forward pages dedupe by conversation");
            seen.push(id);
        }
        match page["page"]["next_sync_cursor"].as_str() {
            Some(next) => {
                assert_eq!(page["page"]["has_more_after"], true);
                cursor = next.to_string();
            }
            None => {
                assert_eq!(page["page"]["has_more_after"], false);
                break;
            }
        }
    }
    assert_eq!(seen.len(), 55, "every changed conversation is reachable");

    // Same-timestamp change: pin one row to the exact snapshot boundary. The
    // strict `>` predicate compares the tie id against 0, so id > 0 keeps the
    // row visible even though its timestamp did not move.
    let (_, _, page) = get_as(&setup.app, "/api/v1/me/conversations?limit=50", &viewer).await;
    let snapshot = page["page"]["sync_cursor"]
        .as_str()
        .expect("sync_cursor")
        .to_string();
    let snapshot_at = micros_of(&snapshot);
    let (conversation_id, first_message_public_id, counterpart_public_id) = seeded[0].clone();
    sqlx::query("UPDATE conversations SET updated_at = ? WHERE id = ?")
        .bind(snapshot_at)
        .bind(conversation_id)
        .execute(&setup.pool)
        .await
        .expect("pin updated_at");
    let target = conversation_public_id(&setup.pool, conversation_id).await;
    let (status, _, after) = get_as(
        &setup.app,
        &format!("/api/v1/me/conversations?after={snapshot}"),
        &viewer,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        after["data"]
            .as_array()
            .expect("data")
            .iter()
            .any(|item| item["public_id"].as_str() == Some(target.as_str())),
        "a same-timestamp change must still be returned by the next poll: {after}"
    );

    // A Read advances synchronization but never Inbox recency order.
    let (status, _, receipt) =
        mark_read(&setup.app, &viewer, &target, &first_message_public_id).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["unread_count"], 0);
    let (_, _, page) = get_as(&setup.app, "/api/v1/me/conversations?limit=50", &viewer).await;
    let items = page["data"].as_array().expect("data").clone();
    let position = items
        .iter()
        .position(|item| item["public_id"] == target.as_str())
        .expect("conversation present");
    assert_ne!(
        items[0]["public_id"],
        target.as_str(),
        "a Read must not promote the conversation to the top"
    );
    assert!(position > 0);

    // Blocking the counterpart also appears through forward synchronization.
    let (_, _, page) = get_as(&setup.app, "/api/v1/me/conversations?limit=1", &viewer).await;
    let before_block = page["page"]["sync_cursor"]
        .as_str()
        .expect("sync_cursor")
        .to_string();
    let block_uri = format!("/api/v1/me/blocks/{counterpart_public_id}");
    let (status, _, _) = call(
        &setup.app,
        Method::PUT,
        &block_uri,
        Some(&viewer.cookie),
        Some(&viewer.csrf),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, _, after) = get_as(
        &setup.app,
        &format!("/api/v1/me/conversations?after={before_block}"),
        &viewer,
    )
    .await;
    let blocked_item = after["data"]
        .as_array()
        .expect("data")
        .iter()
        .find(|item| item["other_party"]["public_id"] == counterpart_public_id.as_str())
        .cloned()
        .expect("blocked conversation appears through sync");
    assert_eq!(blocked_item["can_message"], false);
    assert_eq!(blocked_item["viewer_blocked_other"], true);
}

/// Decodes the timestamp half of a `micros:id` cursor.
fn micros_of(cursor: &str) -> DateTime<Utc> {
    let micros = cursor
        .split(':')
        .next()
        .expect("cursor timestamp")
        .parse::<i64>()
        .expect("numeric micros");
    DateTime::from_timestamp_micros(micros).expect("valid timestamp")
}

async fn conversation_public_id(pool: &MySqlPool, conversation_id: u64) -> String {
    sqlx::query_scalar("SELECT public_id FROM conversations WHERE id = ?")
        .bind(conversation_id)
        .fetch_one(pool)
        .await
        .expect("conversation public id")
}

#[tokio::test]
async fn inbox_requires_auth_and_rejects_conflicting_cursors() {
    let Some(setup) = setup().await else {
        return;
    };
    let (status, headers, body) = call(
        &setup.app,
        Method::GET,
        "/api/v1/me/conversations",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_private(&headers);

    let viewer = create_user(&setup.pool, &setup.cfg, "+989130000181", "خریدار").await;
    let (status, headers, body) = get_as(
        &setup.app,
        "/api/v1/me/conversations?before=1:1&after=2:2",
        &viewer,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_private(&headers);
    let (status, _, _) = get_as(
        &setup.app,
        "/api/v1/me/conversations?before=garbage",
        &viewer,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
