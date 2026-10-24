//! Phase 6 favorites: atomic mutations, exact counters, and viewer isolation.

mod common;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use shabdar_api::config::Config;
use sqlx::MySqlPool;
use tower::ServiceExt;

/// The suite shares one scratch database; each test owns it exclusively.
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
    public_id: String,
    slug: String,
}

async fn reset_db(pool: &MySqlPool) {
    for table in [
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

/// Direct fixtures keep these tests focused on favorites rather than OTP.
async fn create_user(pool: &MySqlPool, cfg: &Config, phone: &str) -> TestUser {
    let result = sqlx::query(
        "INSERT INTO users (public_id, phone_e164, created_at, updated_at) \
         VALUES (?, ?, NOW(3), NOW(3))",
    )
    .bind(ulid::Ulid::new().to_string())
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

async fn create_listing(pool: &MySqlPool, seller_id: u64, status: &str) -> ListingFixture {
    let public_id = ulid::Ulid::new().to_string();
    let slug = format!("listing-{}", ulid::Ulid::new());
    let result = sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, published_at, created_at, updated_at) \
         VALUES (?, ?, 1, 1, 'favorite fixture', ?, 'description', 1000, 'fixed', ?, \
         NOW(3), NOW(3), NOW(3))",
    )
    .bind(&public_id)
    .bind(seller_id)
    .bind(&slug)
    .bind(status)
    .execute(pool)
    .await
    .expect("listing fixture");
    ListingFixture {
        id: result.last_insert_id(),
        public_id,
        slug,
    }
}

/// One body-free API request; headers are retained for detail-cache tests.
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

async fn favorite_count(pool: &MySqlPool, listing_id: u64) -> u64 {
    sqlx::query_scalar("SELECT favorite_count FROM listings WHERE id = ?")
        .bind(listing_id)
        .fetch_one(pool)
        .await
        .expect("favorite counter")
}

async fn favorite_rows(pool: &MySqlPool, listing_id: u64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM favorites WHERE listing_id = ?")
        .bind(listing_id)
        .fetch_one(pool)
        .await
        .expect("favorite rows")
}

async fn assert_counter_matches_rows(pool: &MySqlPool, listing_id: u64) {
    let counter = favorite_count(pool, listing_id).await;
    let rows = favorite_rows(pool, listing_id).await;
    assert_eq!(counter, u64::try_from(rows).expect("non-negative row count"));
}

#[tokio::test]
async fn favorite_mutations_require_session_and_csrf() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let uri = format!("/api/v1/me/favorites/{}", listing.public_id);

    for method in [Method::PUT, Method::DELETE] {
        let (status, _, body) = send(t.app.clone(), method, &uri, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "unauthorized");
    }
    for method in [Method::PUT, Method::DELETE] {
        let (status, _, body) = send(
            t.app.clone(),
            method,
            &uri,
            Some(&seller.cookie),
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
        Some(&seller.cookie),
        Some("wrong-token"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(favorite_rows(&t.pool, listing.id).await, 0);
    assert_eq!(favorite_count(&t.pool, listing.id).await, 0);
}

#[tokio::test]
async fn save_is_idempotent_and_counts_distinct_users() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let first = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let second = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let uri = format!("/api/v1/me/favorites/{}", listing.public_id);

    for _ in 0..2 {
        let (status, _, body) = send(
            t.app.clone(),
            Method::PUT,
            &uri,
            Some(&first.cookie),
            Some(&first.csrf),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::json!({ "favorited": true }));
        assert_eq!(favorite_count(&t.pool, listing.id).await, 1);
        assert_eq!(favorite_rows(&t.pool, listing.id).await, 1);
    }

    let saved_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "SELECT created_at FROM favorites WHERE user_id = ? AND listing_id = ?",
    )
    .bind(first.id)
    .bind(listing.id)
    .fetch_one(&t.pool)
    .await
    .expect("original saved-at");
    let (status, _, _) = send(
        t.app.clone(),
        Method::PUT,
        &uri,
        Some(&first.cookie),
        Some(&first.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let after_duplicate: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "SELECT created_at FROM favorites WHERE user_id = ? AND listing_id = ?",
    )
    .bind(first.id)
    .bind(listing.id)
    .fetch_one(&t.pool)
    .await
    .expect("duplicate saved-at");
    assert_eq!(after_duplicate, saved_at, "duplicate save keeps created_at");

    let (status, _, _) = send(
        t.app.clone(),
        Method::PUT,
        &uri,
        Some(&second.cookie),
        Some(&second.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(favorite_count(&t.pool, listing.id).await, 2);
    assert_eq!(favorite_rows(&t.pool, listing.id).await, 2);
}

#[tokio::test]
async fn save_rejects_unknown_inactive_and_deleted_listings() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let saver = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "draft").await;
    let uri = format!("/api/v1/me/favorites/{}", listing.public_id);

    for status in ["draft", "pending", "rejected", "sold", "archived", "removed"] {
        sqlx::query("UPDATE listings SET status = ? WHERE id = ?")
            .bind(status)
            .bind(listing.id)
            .execute(&t.pool)
            .await
            .expect("listing status fixture");
        let (http_status, _, body) = send(
            t.app.clone(),
            Method::PUT,
            &uri,
            Some(&saver.cookie),
            Some(&saver.csrf),
        )
        .await;
        assert_eq!(http_status, StatusCode::NOT_FOUND, "status {status}");
        assert_eq!(body["error"]["code"], "not_found");
        assert_eq!(favorite_count(&t.pool, listing.id).await, 0);
    }

    sqlx::query("UPDATE listings SET status = 'active', deleted_at = NOW(3) WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("soft-delete fixture");
    let (status, _, _) = send(
        t.app.clone(),
        Method::PUT,
        &uri,
        Some(&saver.cookie),
        Some(&saver.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let unknown = format!("/api/v1/me/favorites/{}", "0".repeat(26));
    let (status, _, _) = send(
        t.app.clone(),
        Method::PUT,
        &unknown,
        Some(&saver.cookie),
        Some(&saver.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(favorite_rows(&t.pool, listing.id).await, 0);
    assert_eq!(favorite_count(&t.pool, listing.id).await, 0);

    // Positive control: a generic 404 route cannot satisfy this test.
    sqlx::query("UPDATE listings SET deleted_at = NULL WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("restore active fixture");
    let (status, _, _) = send(
        t.app.clone(),
        Method::PUT,
        &uri,
        Some(&saver.cookie),
        Some(&saver.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(favorite_count(&t.pool, listing.id).await, 1);
}

#[tokio::test]
async fn unsave_is_idempotent_and_allows_inactive_listing() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let saver = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let uri = format!("/api/v1/me/favorites/{}", listing.public_id);

    let (status, _, body) = send(
        t.app.clone(),
        Method::PUT,
        &uri,
        Some(&saver.cookie),
        Some(&saver.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({ "favorited": true }));
    sqlx::query("UPDATE listings SET status = 'sold' WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("sold fixture");

    for expected_count in [0, 0] {
        let (status, _, body) = send(
            t.app.clone(),
            Method::DELETE,
            &uri,
            Some(&saver.cookie),
            Some(&saver.csrf),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::json!({ "favorited": false }));
        assert_eq!(favorite_count(&t.pool, listing.id).await, expected_count);
    }

    sqlx::query("UPDATE listings SET status = 'active' WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("active fixture");
    let (status, _, _) = send(
        t.app.clone(),
        Method::PUT,
        &uri,
        Some(&saver.cookie),
        Some(&saver.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    sqlx::query("UPDATE listings SET deleted_at = NOW(3) WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("deleted fixture");
    for _ in 0..2 {
        let (status, _, body) = send(
            t.app.clone(),
            Method::DELETE,
            &uri,
            Some(&saver.cookie),
            Some(&saver.csrf),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::json!({ "favorited": false }));
        assert_eq!(favorite_count(&t.pool, listing.id).await, 0);
    }

    let unknown = format!("/api/v1/me/favorites/{}", "0".repeat(26));
    let (status, _, _) = send(
        t.app.clone(),
        Method::DELETE,
        &unknown,
        Some(&saver.cookie),
        Some(&saver.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn users_cannot_mutate_each_others_favorites() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let first = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let second = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let uri = format!("/api/v1/me/favorites/{}", listing.public_id);

    let (status, _, _) = send(
        t.app.clone(),
        Method::PUT,
        &uri,
        Some(&first.cookie),
        Some(&first.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, body) = send(
        t.app.clone(),
        Method::DELETE,
        &uri,
        Some(&second.cookie),
        Some(&second.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({ "favorited": false }));

    let first_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM favorites WHERE user_id = ? AND listing_id = ?",
    )
    .bind(first.id)
    .bind(listing.id)
    .fetch_one(&t.pool)
    .await
    .expect("first user's row");
    assert_eq!(first_rows, 1);
    assert_eq!(favorite_count(&t.pool, listing.id).await, 1);
}

#[tokio::test]
async fn self_favorite_is_allowed() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let uri = format!("/api/v1/me/favorites/{}", listing.public_id);
    let (status, _, body) = send(
        t.app.clone(),
        Method::PUT,
        &uri,
        Some(&seller.cookie),
        Some(&seller.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({ "favorited": true }));
    assert_eq!(favorite_count(&t.pool, listing.id).await, 1);
}

#[tokio::test]
async fn favorite_concurrent_mutations_keep_counter_exact() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let saver = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let uri = format!("/api/v1/me/favorites/{}", listing.public_id);

    let (first, second) = tokio::join!(
        send(
            t.app.clone(),
            Method::PUT,
            &uri,
            Some(&saver.cookie),
            Some(&saver.csrf),
        ),
        send(
            t.app.clone(),
            Method::PUT,
            &uri,
            Some(&saver.cookie),
            Some(&saver.csrf),
        )
    );
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(second.0, StatusCode::OK);
    assert_eq!(favorite_count(&t.pool, listing.id).await, 1);
    assert_counter_matches_rows(&t.pool, listing.id).await;

    // The final state may legitimately be either saved or absent; the row set
    // and denormalized counter must agree in either serialization.
    let (save, remove) = tokio::join!(
        send(
            t.app.clone(),
            Method::PUT,
            &uri,
            Some(&saver.cookie),
            Some(&saver.csrf),
        ),
        send(
            t.app.clone(),
            Method::DELETE,
            &uri,
            Some(&saver.cookie),
            Some(&saver.csrf),
        )
    );
    assert_eq!(save.0, StatusCode::OK);
    assert_eq!(remove.0, StatusCode::OK);
    assert_counter_matches_rows(&t.pool, listing.id).await;

    let (first, second) = tokio::join!(
        send(
            t.app.clone(),
            Method::PUT,
            &uri,
            Some(&saver.cookie),
            Some(&saver.csrf),
        ),
        send(
            t.app.clone(),
            Method::PUT,
            &uri,
            Some(&saver.cookie),
            Some(&saver.csrf),
        )
    );
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(second.0, StatusCode::OK);
    let (first, second) = tokio::join!(
        send(
            t.app.clone(),
            Method::DELETE,
            &uri,
            Some(&saver.cookie),
            Some(&saver.csrf),
        ),
        send(
            t.app.clone(),
            Method::DELETE,
            &uri,
            Some(&saver.cookie),
            Some(&saver.csrf),
        )
    );
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(second.0, StatusCode::OK);
    assert_eq!(favorite_count(&t.pool, listing.id).await, 0);
    assert_counter_matches_rows(&t.pool, listing.id).await;
}

/// A fixed millisecond-precision timestamp makes cursor ties deterministic.
fn fixture_saved_at() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp_micros(1_700_000_000_123_000)
        .expect("valid fixture timestamp")
}

/// Direct list fixtures preserve the same row/counter invariant as the API.
async fn insert_favorite(pool: &MySqlPool, user_id: u64, listing_id: u64) {
    sqlx::query("INSERT INTO favorites (user_id, listing_id, created_at) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(listing_id)
        .bind(fixture_saved_at())
        .execute(pool)
        .await
        .expect("favorite fixture");
    sqlx::query("UPDATE listings SET favorite_count = favorite_count + 1 WHERE id = ?")
        .bind(listing_id)
        .execute(pool)
        .await
        .expect("favorite counter fixture");
}

fn card_public_ids(body: &serde_json::Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("data array")
        .iter()
        .map(|card| {
            card["public_id"]
                .as_str()
                .expect("card public_id")
                .to_string()
        })
        .collect()
}

#[tokio::test]
async fn favorites_list_requires_session() {
    let Some(t) = setup().await else { return };
    let (status, _, body) = send(
        t.app.clone(),
        Method::GET,
        "/api/v1/me/favorites",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");
}

#[tokio::test]
async fn favorites_list_returns_only_viewers_active_cards() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let viewer = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let other = create_user(&t.pool, &t.cfg, "+989100000003").await;

    let visible = create_listing(&t.pool, seller.id, "active").await;
    insert_favorite(&t.pool, viewer.id, visible.id).await;
    let other_only = create_listing(&t.pool, seller.id, "active").await;
    insert_favorite(&t.pool, other.id, other_only.id).await;
    for status in ["sold", "archived", "removed", "pending", "draft", "rejected"] {
        let hidden = create_listing(&t.pool, seller.id, status).await;
        insert_favorite(&t.pool, viewer.id, hidden.id).await;
    }
    let deleted = create_listing(&t.pool, seller.id, "active").await;
    insert_favorite(&t.pool, viewer.id, deleted.id).await;
    sqlx::query("UPDATE listings SET deleted_at = NOW(3) WHERE id = ?")
        .bind(deleted.id)
        .execute(&t.pool)
        .await
        .expect("soft-delete fixture");
    sqlx::query(
        "INSERT INTO listing_images (listing_id, storage_key, sort_order, created_at) \
         VALUES (?, 'setup/cover.png', 0, NOW(3))",
    )
    .bind(visible.id)
    .execute(&t.pool)
    .await
    .expect("cover fixture");

    let (status, _, body) = send(
        t.app.clone(),
        Method::GET,
        "/api/v1/me/favorites",
        Some(&viewer.cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(card_public_ids(&body), vec![visible.public_id.clone()]);
    assert_eq!(body["data"][0]["slug"].as_str(), Some(visible.slug.as_str()));
    assert_eq!(body["data"][0]["cover_image"], "/media/setup/cover.png");
    assert_eq!(body["data"][0]["city"], "رشت");
    assert_eq!(body["data"][0]["province"], "گیلان");
    assert!(body["data"][0]["category"].as_str().is_some());
    assert_eq!(body["page"]["has_more"], false);
    assert!(body["page"]["next_cursor"].is_null());
}

#[tokio::test]
async fn favorites_list_rejects_bad_cursor() {
    let Some(t) = setup().await else { return };
    let viewer = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let (status, _, body) = send(
        t.app.clone(),
        Method::GET,
        "/api/v1/me/favorites?cursor=not-a-cursor",
        Some(&viewer.cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "validation_error");
    assert_eq!(body["error"]["fields"]["cursor"], "invalid_cursor");
}

#[tokio::test]
async fn favorites_cursor_walks_equal_timestamps() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let viewer = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let first = create_listing(&t.pool, seller.id, "active").await;
    let second = create_listing(&t.pool, seller.id, "active").await;
    let third = create_listing(&t.pool, seller.id, "active").await;
    for listing in [&first, &second, &third] {
        insert_favorite(&t.pool, viewer.id, listing.id).await;
    }

    let (status, _, first_page) = send(
        t.app.clone(),
        Method::GET,
        "/api/v1/me/favorites?limit=2",
        Some(&viewer.cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_page["page"]["has_more"], true);
    assert_eq!(
        card_public_ids(&first_page),
        vec![third.public_id.clone(), second.public_id.clone()]
    );
    let cursor = first_page["page"]["next_cursor"]
        .as_str()
        .expect("next cursor")
        .to_string();

    let (status, _, second_page) = send(
        t.app.clone(),
        Method::GET,
        &format!("/api/v1/me/favorites?limit=2&cursor={cursor}"),
        Some(&viewer.cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_page["page"]["has_more"], false);
    assert!(second_page["page"]["next_cursor"].is_null());
    assert_eq!(card_public_ids(&second_page), vec![first.public_id.clone()]);

    let mut walked = card_public_ids(&first_page);
    walked.extend(card_public_ids(&second_page));
    assert_eq!(
        walked,
        vec![third.public_id.clone(), second.public_id.clone(), first.public_id.clone()],
        "equal timestamps must not duplicate or skip a card"
    );
}

#[tokio::test]
async fn favorites_cursor_survives_anchor_unsave() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let viewer = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let first = create_listing(&t.pool, seller.id, "active").await;
    let second = create_listing(&t.pool, seller.id, "active").await;
    let third = create_listing(&t.pool, seller.id, "active").await;
    for listing in [&first, &second, &third] {
        insert_favorite(&t.pool, viewer.id, listing.id).await;
    }

    let (status, _, first_page) = send(
        t.app.clone(),
        Method::GET,
        "/api/v1/me/favorites?limit=1",
        Some(&viewer.cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(card_public_ids(&first_page), vec![third.public_id.clone()]);
    let cursor = first_page["page"]["next_cursor"]
        .as_str()
        .expect("next cursor")
        .to_string();
    let anchor = third.public_id.clone();
    let (status, _, _) = send(
        t.app.clone(),
        Method::DELETE,
        &format!("/api/v1/me/favorites/{anchor}"),
        Some(&viewer.cookie),
        Some(&viewer.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(favorite_count(&t.pool, third.id).await, 0);

    let (status, _, second_page) = send(
        t.app.clone(),
        Method::GET,
        &format!("/api/v1/me/favorites?limit=2&cursor={cursor}"),
        Some(&viewer.cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_page["page"]["has_more"], false);
    assert_eq!(
        card_public_ids(&second_page),
        vec![second.public_id.clone(), first.public_id.clone()]
    );
    assert!(!card_public_ids(&second_page).contains(&anchor));
}

async fn dashboard(app: Router, user: &TestUser) -> serde_json::Value {
    let (status, _, body) = send(
        app,
        Method::GET,
        "/api/v1/me/dashboard/summary",
        Some(&user.cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body
}

#[tokio::test]
async fn dashboard_counts_received_not_personal_favorites() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let first = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let second = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let other_seller = create_user(&t.pool, &t.cfg, "+989100000004").await;
    let a = create_listing(&t.pool, seller.id, "active").await;
    let b = create_listing(&t.pool, seller.id, "active").await;
    let other = create_listing(&t.pool, other_seller.id, "active").await;

    for (user, listing) in [(&first, &a), (&second, &a), (&first, &b)] {
        let (status, _, body) = send(
            t.app.clone(),
            Method::PUT,
            &format!("/api/v1/me/favorites/{}", listing.public_id),
            Some(&user.cookie),
            Some(&user.csrf),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::json!({ "favorited": true }));
    }
    let initial = dashboard(t.app.clone(), &seller).await;
    assert_eq!(initial["favorite_count"], 3);
    assert_eq!(initial.as_object().expect("summary object").len(), 4);

    // Sold listings still belong to the seller's non-deleted dashboard sum.
    sqlx::query("UPDATE listings SET status = 'sold' WHERE id = ?")
        .bind(a.id)
        .execute(&t.pool)
        .await
        .expect("sold fixture");
    assert_eq!(dashboard(t.app.clone(), &seller).await["favorite_count"], 3);

    // A personal save on somebody else's listing is received by that seller,
    // never added to the viewer's own received-favorites number.
    let (status, _, _) = send(
        t.app.clone(),
        Method::PUT,
        &format!("/api/v1/me/favorites/{}", other.public_id),
        Some(&seller.cookie),
        Some(&seller.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dashboard(t.app.clone(), &seller).await["favorite_count"], 3);
    assert_eq!(dashboard(t.app.clone(), &other_seller).await["favorite_count"], 1);

    let (status, _, _) = send(
        t.app.clone(),
        Method::DELETE,
        &format!("/api/v1/me/favorites/{}", a.public_id),
        Some(&first.cookie),
        Some(&first.csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dashboard(t.app.clone(), &seller).await["favorite_count"], 2);

    sqlx::query("UPDATE listings SET deleted_at = NOW(3) WHERE id = ?")
        .bind(b.id)
        .execute(&t.pool)
        .await
        .expect("soft-delete fixture");
    assert_eq!(dashboard(t.app.clone(), &seller).await["favorite_count"], 1);
    assert_eq!(dashboard(t.app.clone(), &other_seller).await["favorite_count"], 1);
}
