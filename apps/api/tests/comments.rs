//! Phase 8 comments: public pages, safe creation, ownership, and exact counters.

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
    query: &str,
    user: Option<&TestUser>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::GET,
        &format!("{}{query}", comments_uri(&listing.slug)),
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
    assert_eq!(
        i64::from(counter),
        visible,
        "visible comment_count invariant"
    );
    counter
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

fn assert_public_comment_shape(comment: &serde_json::Value, author: &TestUser) {
    let object = comment.as_object().expect("comment object");
    assert_eq!(object.len(), 4, "no viewer or moderation fields: {comment}");
    assert_eq!(comment["public_id"].as_str().map(str::len), Some(26));
    assert!(comment["body"].is_string());
    assert!(comment["created_at"].is_string());
    assert_eq!(comment["author"]["public_id"], author.public_id);
    let author_object = comment["author"].as_object().expect("author object");
    assert_eq!(author_object.len(), 2);
    assert!(author_object.contains_key("display_name"));
    assert!(!object.contains_key("can_delete"));
}

fn fixture_created_at() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp_micros(1_700_000_000_123_000).expect("valid fixture timestamp")
}

async fn insert_visible_comment(pool: &MySqlPool, listing_id: u64, author_id: u64) -> String {
    let public_id = ulid::Ulid::new().to_string();
    sqlx::query(
        "INSERT INTO listing_comments \
         (public_id, listing_id, author_id, body, created_at, updated_at) \
         VALUES (?, ?, ?, 'page fixture', ?, ?)",
    )
    .bind(&public_id)
    .bind(listing_id)
    .bind(author_id)
    .bind(fixture_created_at())
    .bind(fixture_created_at())
    .execute(pool)
    .await
    .expect("comment page fixture");
    public_id
}

#[tokio::test]
async fn comment_mutations_require_session_and_csrf() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let post_uri = comments_uri(&listing.slug);
    let delete_uri = format!("/api/v1/comments/{}", "0".repeat(26));

    for uri in [&post_uri, &delete_uri] {
        let method = if uri == &post_uri {
            Method::POST
        } else {
            Method::DELETE
        };
        let body = (method == Method::POST).then(|| serde_json::json!({ "body": "comment" }));
        let (status, _, response) = send(t.app.clone(), method, uri, None, None, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(response["error"]["code"], "unauthorized");
    }
    for uri in [&post_uri, &delete_uri] {
        let method = if uri == &post_uri {
            Method::POST
        } else {
            Method::DELETE
        };
        let body = (method == Method::POST).then(|| serde_json::json!({ "body": "comment" }));
        let (status, _, response) =
            send(t.app.clone(), method, uri, Some(&author), None, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(response["error"]["code"], "csrf_failed");
    }
    let (status, _, _) = send(
        t.app.clone(),
        Method::POST,
        &post_uri,
        Some(&author),
        Some("wrong-token"),
        Some(serde_json::json!({ "body": "comment" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        0
    );
}

#[tokio::test]
async fn comment_validation_trim_unicode_and_public_shape() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let uri = comments_uri(&listing.slug);

    for body in [
        serde_json::json!({}),
        serde_json::json!({ "body": null }),
        serde_json::json!({ "body": "" }),
        serde_json::json!({ "body": " \n\t " }),
    ] {
        let (status, _, response) = send(
            t.app.clone(),
            Method::POST,
            &uri,
            Some(&author),
            Some(&author.csrf),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert_eq!(response["error"]["code"], "validation_error");
        assert_eq!(response["error"]["fields"]["body"], "required");
    }
    let (status, _, response) =
        post_comment(t.app.clone(), &listing, &author, "🙂".repeat(1001)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response["error"]["fields"]["body"], "too_long");
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        0
    );

    let (status, _, created) = post_comment(
        t.app.clone(),
        &listing,
        &author,
        " \n\t متن اول\nمتن دوم 🙂  \n ",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["body"], "متن اول\nمتن دوم 🙂");
    assert_public_comment_shape(&created, &author);

    let (status, _, anonymous_page) = list_comments(t.app.clone(), &listing, "", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comment_ids(&anonymous_page).len(), 1);
    assert_eq!(anonymous_page["data"][0]["body"], "متن اول\nمتن دوم 🙂");
    assert_public_comment_shape(&anonymous_page["data"][0], &author);
    let (status, _, viewer_page) = list_comments(t.app.clone(), &listing, "", Some(&author)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        viewer_page, anonymous_page,
        "public pages have no viewer state"
    );

    // Self-commenting is allowed, and utf8mb4/CHAR counting accepts exactly 1000.
    let (status, _, long_created) =
        post_comment(t.app.clone(), &listing, &seller, "🙂".repeat(1000)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        long_created["body"]
            .as_str()
            .map(|body| body.chars().count()),
        Some(1000)
    );
    assert_public_comment_shape(&long_created, &seller);
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        2
    );
}

#[tokio::test]
async fn comment_listing_visibility_and_cursor_validation() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "draft").await;

    for listing_status in ["draft", "pending", "rejected", "sold", "archived"] {
        sqlx::query("UPDATE listings SET status = ? WHERE id = ?")
            .bind(listing_status)
            .bind(listing.id)
            .execute(&t.pool)
            .await
            .expect("listing status fixture");
        assert_eq!(
            list_comments(t.app.clone(), &listing, "", None).await.0,
            StatusCode::NOT_FOUND,
            "GET {listing_status}"
        );
        assert_eq!(
            post_comment(t.app.clone(), &listing, &author, "comment")
                .await
                .0,
            StatusCode::NOT_FOUND,
            "POST {listing_status}"
        );
    }
    sqlx::query("UPDATE listings SET status = 'removed' WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("removed fixture");
    assert_eq!(
        list_comments(t.app.clone(), &listing, "", None).await.0,
        StatusCode::GONE
    );
    assert_eq!(
        post_comment(t.app.clone(), &listing, &author, "comment")
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
        list_comments(t.app.clone(), &listing, "", None).await.0,
        StatusCode::GONE
    );
    assert_eq!(
        post_comment(t.app.clone(), &listing, &author, "comment")
            .await
            .0,
        StatusCode::GONE
    );
    let unknown = ListingFixture {
        id: listing.id,
        slug: "unknown-slug".to_string(),
    };
    assert_eq!(
        list_comments(t.app.clone(), &unknown, "?cursor=bad", None)
            .await
            .0,
        StatusCode::NOT_FOUND,
        "unknown target does not become a cursor-validation oracle"
    );
    assert_eq!(
        post_comment(t.app.clone(), &unknown, &author, "comment")
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    sqlx::query("UPDATE listings SET status = 'active', deleted_at = NULL WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("restore active fixture");
    let (status, _, body) = list_comments(t.app.clone(), &listing, "?cursor=bad", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["fields"]["cursor"], "invalid_cursor");
    assert_eq!(
        list_comments(t.app.clone(), &listing, "", None).await.0,
        StatusCode::OK,
        "positive control after the visibility matrix"
    );
}

#[tokio::test]
async fn public_list_excludes_hidden_removed_deleted_and_other_listings() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let other_listing = create_listing(&t.pool, seller.id, "active").await;
    let mut ids = Vec::new();
    for body in ["hidden", "removed", "deleted", "visible"] {
        let (status, _, response) = post_comment(t.app.clone(), &listing, &author, body).await;
        assert_eq!(status, StatusCode::OK);
        ids.push(response["public_id"].as_str().expect("id").to_string());
    }
    sqlx::query("UPDATE listing_comments SET status = 'hidden' WHERE public_id = ?")
        .bind(&ids[0])
        .execute(&t.pool)
        .await
        .expect("hidden fixture");
    sqlx::query("UPDATE listing_comments SET status = 'removed' WHERE public_id = ?")
        .bind(&ids[1])
        .execute(&t.pool)
        .await
        .expect("removed fixture");
    sqlx::query("UPDATE listing_comments SET deleted_at = NOW(3) WHERE public_id = ?")
        .bind(&ids[2])
        .execute(&t.pool)
        .await
        .expect("deleted fixture");
    sqlx::query(
        "UPDATE listings l SET comment_count = (SELECT COUNT(*) FROM listing_comments c \
         WHERE c.listing_id = l.id AND c.status = 'visible' AND c.deleted_at IS NULL) \
         WHERE l.id = ?",
    )
    .bind(listing.id)
    .execute(&t.pool)
    .await
    .expect("align counter with direct moderation fixtures");
    assert_eq!(
        post_comment(t.app.clone(), &other_listing, &author, "other")
            .await
            .0,
        StatusCode::OK
    );

    let (status, _, body) = list_comments(t.app.clone(), &listing, "", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comment_ids(&body), vec![ids[3].clone()]);
    assert_eq!(body["data"][0]["body"], "visible");
    assert_eq!(body["page"]["has_more"], false);
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        1
    );
}

#[tokio::test]
async fn author_only_delete_is_idempotent_and_changes_only_deleted_at() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let other = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let (_, _, first) = post_comment(t.app.clone(), &listing, &author, "first").await;
    let (_, _, second) = post_comment(t.app.clone(), &listing, &other, "second").await;
    let first_id = first["public_id"].as_str().expect("first id");
    let second_id = second["public_id"].as_str().expect("second id");

    let (status, _, body) = delete_comment(t.app.clone(), first_id, &other).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        2
    );
    let before = sqlx::query("SELECT status, updated_at FROM listing_comments WHERE public_id = ?")
        .bind(first_id)
        .fetch_one(&t.pool)
        .await
        .expect("before delete");
    let before_updated: chrono::DateTime<chrono::Utc> =
        before.try_get("updated_at").expect("updated_at");

    for _ in 0..2 {
        let (status, _, body) = delete_comment(t.app.clone(), first_id, &author).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::json!({ "ok": true }));
        assert_eq!(
            assert_counter_matches_visible_rows(&t.pool, listing.id).await,
            1
        );
    }
    let after = sqlx::query(
        "SELECT status, updated_at, deleted_at FROM listing_comments WHERE public_id = ?",
    )
    .bind(first_id)
    .fetch_one(&t.pool)
    .await
    .expect("after delete");
    assert_eq!(
        after.try_get::<String, _>("status").expect("status"),
        "visible"
    );
    assert_eq!(
        after
            .try_get::<chrono::DateTime<chrono::Utc>, _>("updated_at")
            .expect("updated_at"),
        before_updated,
        "author deletion sets only deleted_at"
    );
    assert!(after
        .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("deleted_at")
        .expect("deleted_at")
        .is_some());
    let (_, _, page) = list_comments(t.app.clone(), &listing, "", None).await;
    assert_eq!(comment_ids(&page), vec![second_id.to_string()]);
    assert_eq!(
        delete_comment(t.app.clone(), &"0".repeat(26), &author)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn comment_limit_is_per_user_and_delete_does_not_spend_it() {
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
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let mut first_id = String::new();
    for index in 0..2 {
        let (status, _, body) =
            post_comment(t.app.clone(), &listing, &first, format!("comment {index}")).await;
        assert_eq!(status, StatusCode::OK);
        if index == 0 {
            first_id = body["public_id"].as_str().expect("id").to_string();
        }
    }
    assert_eq!(
        post_comment(t.app.clone(), &listing, &first, " ").await.0,
        StatusCode::BAD_REQUEST,
        "validation precedes the creation limiter"
    );
    let (status, _, body) = send(
        t.app.clone(),
        Method::POST,
        &comments_uri(&listing.slug),
        Some(&first),
        None,
        Some(serde_json::json!({ "body": "third" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "CSRF precedes the limiter");
    assert_eq!(body["error"]["code"], "csrf_failed");
    let (status, headers, body) = post_comment(t.app.clone(), &listing, &first, "third").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], "rate_limited");
    let retry_after: u64 = headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .expect("numeric Retry-After");
    assert!((1..=60).contains(&retry_after));

    assert_eq!(
        post_comment(t.app.clone(), &listing, &second, "independent")
            .await
            .0,
        StatusCode::OK,
        "another user has an independent window"
    );
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        3
    );
    assert_eq!(
        delete_comment(t.app.clone(), &first_id, &first).await.0,
        StatusCode::OK,
        "delete does not consume the exhausted creation allowance"
    );
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        2
    );
}

#[tokio::test]
async fn cursor_walks_equal_timestamps_after_anchor_delete() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    let mut ids = Vec::new();
    for index in 0..5 {
        let (status, _, body) =
            post_comment(t.app.clone(), &listing, &author, format!("comment {index}")).await;
        assert_eq!(status, StatusCode::OK);
        ids.push(body["public_id"].as_str().expect("id").to_string());
    }
    sqlx::query("UPDATE listing_comments SET created_at = ? WHERE listing_id = ?")
        .bind(fixture_created_at())
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("equal timestamp fixture");

    let (status, _, first_page) = list_comments(t.app.clone(), &listing, "?limit=2", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        comment_ids(&first_page),
        vec![ids[4].clone(), ids[3].clone()]
    );
    assert_eq!(first_page["page"]["has_more"], true);
    let cursor = first_page["page"]["next_cursor"]
        .as_str()
        .expect("first cursor")
        .to_string();
    assert_eq!(
        delete_comment(t.app.clone(), &ids[3], &author).await.0,
        StatusCode::OK
    );

    let (status, _, second_page) = list_comments(
        t.app.clone(),
        &listing,
        &format!("?limit=2&cursor={cursor}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        comment_ids(&second_page),
        vec![ids[2].clone(), ids[1].clone()]
    );
    assert_eq!(second_page["page"]["has_more"], true);
    let cursor = second_page["page"]["next_cursor"]
        .as_str()
        .expect("second cursor")
        .to_string();
    let (status, _, third_page) = list_comments(
        t.app.clone(),
        &listing,
        &format!("?limit=2&cursor={cursor}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comment_ids(&third_page), vec![ids[0].clone()]);
    assert_eq!(third_page["page"]["has_more"], false);
    assert!(third_page["page"]["next_cursor"].is_null());

    let mut walked = comment_ids(&first_page);
    walked.extend(comment_ids(&second_page));
    walked.extend(comment_ids(&third_page));
    assert_eq!(
        walked,
        vec![
            ids[4].clone(),
            ids[3].clone(),
            ids[2].clone(),
            ids[1].clone(),
            ids[0].clone(),
        ],
        "equal timestamps and a deleted anchor cause no gaps or duplicates"
    );
    let (_, _, current) = list_comments(t.app.clone(), &listing, "?limit=50", None).await;
    assert!(!comment_ids(&current).contains(&ids[3]));
}

#[tokio::test]
async fn comment_pages_default_to_twenty_and_cap_at_fifty() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let author = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;
    for _ in 0..55 {
        insert_visible_comment(&t.pool, listing.id, author.id).await;
    }
    sqlx::query("UPDATE listings SET comment_count = 55 WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("page counter fixture");

    let (status, _, default_page) = list_comments(t.app.clone(), &listing, "", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comment_ids(&default_page).len(), 20);
    assert_eq!(default_page["page"]["has_more"], true);
    for query in ["?limit=50", "?limit=100"] {
        let (status, _, page) = list_comments(t.app.clone(), &listing, query, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(comment_ids(&page).len(), 50, "cap for {query}");
        assert_eq!(page["page"]["has_more"], true);
    }
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        55
    );
}

#[tokio::test]
async fn concurrent_comment_writers_and_duplicate_deletes_keep_counter_exact() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let first = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let second = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "active").await;

    let (one, two) = tokio::join!(
        post_comment(t.app.clone(), &listing, &first, "one"),
        post_comment(t.app.clone(), &listing, &second, "two"),
    );
    assert_eq!(one.0, StatusCode::OK);
    assert_eq!(two.0, StatusCode::OK);
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        2
    );
    let one_id = one.2["public_id"].as_str().expect("one id").to_string();
    let two_id = two.2["public_id"].as_str().expect("two id").to_string();

    let (delete, duplicate, three) = tokio::join!(
        delete_comment(t.app.clone(), &one_id, &first),
        delete_comment(t.app.clone(), &one_id, &first),
        post_comment(t.app.clone(), &listing, &second, "three"),
    );
    assert_eq!(delete.0, StatusCode::OK);
    assert_eq!(duplicate.0, StatusCode::OK);
    assert_eq!(three.0, StatusCode::OK);
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        2
    );
    let three_id = three.2["public_id"].as_str().expect("three id").to_string();

    let (two_delete, three_delete) = tokio::join!(
        delete_comment(t.app.clone(), &two_id, &second),
        delete_comment(t.app.clone(), &three_id, &second),
    );
    assert_eq!(two_delete.0, StatusCode::OK);
    assert_eq!(three_delete.0, StatusCode::OK);
    assert_eq!(
        assert_counter_matches_visible_rows(&t.pool, listing.id).await,
        0
    );
}
