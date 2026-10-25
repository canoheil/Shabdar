//! Phase 8 ratings: validation, atomic counters, visibility, and user limits.

mod common;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use shabdar_api::config::Config;
use sqlx::{MySqlPool, Row};
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

/// Direct fixtures keep these tests focused on ratings rather than OTP.
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

async fn create_listing(
    pool: &MySqlPool,
    seller_id: u64,
    title: &str,
    status: &str,
) -> ListingFixture {
    let public_id = ulid::Ulid::new().to_string();
    let slug = format!("listing-{}", ulid::Ulid::new());
    let result = sqlx::query(
        "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, slug, \
         description, price_toman, price_type, status, published_at, created_at, updated_at) \
         VALUES (?, ?, 1, 1, ?, ?, 'description', 1000, 'fixed', ?, \
         NOW(3), NOW(3), NOW(3))",
    )
    .bind(public_id)
    .bind(seller_id)
    .bind(title)
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

fn rating_uri(slug: &str) -> String {
    format!("/api/v1/listings/{slug}/rating")
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

async fn put_rating(
    app: Router,
    listing: &ListingFixture,
    user: &TestUser,
    stars: serde_json::Value,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::PUT,
        &rating_uri(&listing.slug),
        Some(user),
        Some(&user.csrf),
        Some(serde_json::json!({ "stars": stars })),
    )
    .await
}

async fn delete_rating(
    app: Router,
    listing: &ListingFixture,
    user: &TestUser,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    send(
        app,
        Method::DELETE,
        &rating_uri(&listing.slug),
        Some(user),
        Some(&user.csrf),
        None,
    )
    .await
}

async fn listing_counters(pool: &MySqlPool, listing_id: u64) -> (u32, u32) {
    let row = sqlx::query("SELECT rating_sum, rating_count FROM listings WHERE id = ?")
        .bind(listing_id)
        .fetch_one(pool)
        .await
        .expect("listing counters");
    (
        row.try_get("rating_sum").expect("rating sum"),
        row.try_get("rating_count").expect("rating count"),
    )
}

async fn assert_counter_matches_rows(pool: &MySqlPool, listing_id: u64) -> (u32, u32) {
    let counters = listing_counters(pool, listing_id).await;
    let row = sqlx::query(
        "SELECT COUNT(*) AS row_count, \
         CAST(COALESCE(SUM(stars), 0) AS SIGNED) AS row_sum \
         FROM listing_ratings WHERE listing_id = ?",
    )
    .bind(listing_id)
    .fetch_one(pool)
    .await
    .expect("actual rating rows");
    let row_count: i64 = row.try_get("row_count").expect("row count");
    let row_sum: i64 = row.try_get("row_sum").expect("row sum");
    assert_eq!(i64::from(counters.0), row_sum, "rating_sum invariant");
    assert_eq!(i64::from(counters.1), row_count, "rating_count invariant");
    counters
}

#[tokio::test]
async fn rating_mutations_require_session_and_csrf() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let rater = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "rating fixture", "active").await;
    let uri = rating_uri(&listing.slug);

    for method in [Method::PUT, Method::DELETE] {
        let body = (method == Method::PUT).then(|| serde_json::json!({ "stars": 5 }));
        let (status, _, body) = send(t.app.clone(), method, &uri, None, None, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "unauthorized");
    }
    for method in [Method::PUT, Method::DELETE] {
        let body = (method == Method::PUT).then(|| serde_json::json!({ "stars": 5 }));
        let (status, _, body) = send(t.app.clone(), method, &uri, Some(&rater), None, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"]["code"], "csrf_failed");
    }
    let (status, _, _) = send(
        t.app.clone(),
        Method::PUT,
        &uri,
        Some(&rater),
        Some("wrong-token"),
        Some(serde_json::json!({ "stars": 5 })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (0, 0)
    );
}

#[tokio::test]
async fn rating_validation_uses_field_errors_and_never_mutates() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let rater = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "rating fixture", "active").await;
    let uri = rating_uri(&listing.slug);

    for body in [serde_json::json!({}), serde_json::json!({ "stars": null })] {
        let (status, _, response) = send(
            t.app.clone(),
            Method::PUT,
            &uri,
            Some(&rater),
            Some(&rater.csrf),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert_eq!(response["error"]["code"], "validation_error");
        assert_eq!(response["error"]["fields"]["stars"], "required");
    }
    for stars in [
        serde_json::json!(0),
        serde_json::json!(6),
        serde_json::json!(-1),
        serde_json::json!(1.5),
        serde_json::json!(1.0),
        serde_json::json!("5"),
        serde_json::json!(true),
        serde_json::json!([]),
        serde_json::json!({}),
    ] {
        let (status, _, response) = put_rating(t.app.clone(), &listing, &rater, stars).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert_eq!(response["error"]["fields"]["stars"], "out_of_range");
    }
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (0, 0)
    );
}

#[tokio::test]
async fn seller_cannot_rate_own_listing() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let listing = create_listing(&t.pool, seller.id, "rating fixture", "active").await;
    let (status, _, body) =
        put_rating(t.app.clone(), &listing, &seller, serde_json::json!(5)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (0, 0)
    );
}

#[tokio::test]
async fn rating_targets_follow_public_visibility_rules() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let rater = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let listing = create_listing(&t.pool, seller.id, "rating fixture", "draft").await;

    for listing_status in ["draft", "pending", "rejected", "sold", "archived"] {
        sqlx::query("UPDATE listings SET status = ? WHERE id = ?")
            .bind(listing_status)
            .bind(listing.id)
            .execute(&t.pool)
            .await
            .expect("listing status fixture");
        let (put_status, _, put_body) =
            put_rating(t.app.clone(), &listing, &rater, serde_json::json!(5)).await;
        let (delete_status, _, delete_body) = delete_rating(t.app.clone(), &listing, &rater).await;
        assert_eq!(put_status, StatusCode::NOT_FOUND, "{listing_status}");
        assert_eq!(delete_status, StatusCode::NOT_FOUND, "{listing_status}");
        assert_eq!(put_body["error"]["code"], "not_found");
        assert_eq!(delete_body["error"]["code"], "not_found");
    }
    sqlx::query("UPDATE listings SET status = 'removed' WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("removed fixture");
    assert_eq!(
        put_rating(t.app.clone(), &listing, &rater, serde_json::json!(5))
            .await
            .0,
        StatusCode::GONE
    );
    assert_eq!(
        delete_rating(t.app.clone(), &listing, &rater).await.0,
        StatusCode::GONE
    );

    sqlx::query("UPDATE listings SET status = 'active', deleted_at = NOW(3) WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("soft-delete fixture");
    assert_eq!(
        put_rating(t.app.clone(), &listing, &rater, serde_json::json!(5))
            .await
            .0,
        StatusCode::GONE
    );
    assert_eq!(
        delete_rating(t.app.clone(), &listing, &rater).await.0,
        StatusCode::GONE
    );
    assert_eq!(
        put_rating(
            t.app.clone(),
            &ListingFixture {
                id: listing.id,
                slug: "unknown-slug".to_string(),
            },
            &rater,
            serde_json::json!(5),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (0, 0)
    );

    // Positive control: the assertions above cannot be satisfied by a generic 404.
    sqlx::query("UPDATE listings SET status = 'active', deleted_at = NULL WHERE id = ?")
        .bind(listing.id)
        .execute(&t.pool)
        .await
        .expect("restore active fixture");
    assert_eq!(
        put_rating(t.app.clone(), &listing, &rater, serde_json::json!(5))
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn create_update_repeat_and_delete_keep_exact_counters() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let first = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let second = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "rating fixture", "active").await;

    let (status, _, body) = put_rating(t.app.clone(), &listing, &first, serde_json::json!(5)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["rating_average"], 5);
    assert_eq!(body["rating_count"], 1);
    assert_eq!(body["my_stars"], 5);
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (5, 1)
    );

    let original = sqlx::query(
        "SELECT created_at, updated_at FROM listing_ratings WHERE listing_id = ? AND user_id = ?",
    )
    .bind(listing.id)
    .bind(first.id)
    .fetch_one(&t.pool)
    .await
    .expect("original rating timestamps");
    let created_at: chrono::DateTime<chrono::Utc> =
        original.try_get("created_at").expect("created_at");
    let updated_at: chrono::DateTime<chrono::Utc> =
        original.try_get("updated_at").expect("updated_at");
    let (status, _, _) = put_rating(t.app.clone(), &listing, &first, serde_json::json!(5)).await;
    assert_eq!(status, StatusCode::OK);
    let repeated = sqlx::query(
        "SELECT created_at, updated_at FROM listing_ratings WHERE listing_id = ? AND user_id = ?",
    )
    .bind(listing.id)
    .bind(first.id)
    .fetch_one(&t.pool)
    .await
    .expect("repeated rating timestamps");
    assert_eq!(
        repeated
            .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .expect("repeated created_at"),
        created_at
    );
    assert_eq!(
        repeated
            .try_get::<chrono::DateTime<chrono::Utc>, _>("updated_at")
            .expect("repeated updated_at"),
        updated_at
    );
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (5, 1)
    );

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let (status, _, body) = put_rating(t.app.clone(), &listing, &first, serde_json::json!(2)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rating_average"], 2);
    assert_eq!(body["rating_count"], 1);
    assert_eq!(body["my_stars"], 2);
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (2, 1)
    );
    let changed = sqlx::query(
        "SELECT created_at, updated_at FROM listing_ratings WHERE listing_id = ? AND user_id = ?",
    )
    .bind(listing.id)
    .bind(first.id)
    .fetch_one(&t.pool)
    .await
    .expect("updated rating timestamps");
    assert_eq!(
        changed
            .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .expect("changed created_at"),
        created_at
    );
    assert_ne!(
        changed
            .try_get::<chrono::DateTime<chrono::Utc>, _>("updated_at")
            .expect("updated_at"),
        updated_at
    );

    let (status, _, body) =
        put_rating(t.app.clone(), &listing, &second, serde_json::json!(4)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rating_average"], 3);
    assert_eq!(body["rating_count"], 2);
    assert_eq!(body["my_stars"], 4);
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (6, 2)
    );

    for _ in 0..2 {
        let (status, _, body) = delete_rating(t.app.clone(), &listing, &first).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["rating_average"], 4);
        assert_eq!(body["rating_count"], 1);
        assert!(body["my_stars"].is_null());
        assert_eq!(
            assert_counter_matches_rows(&t.pool, listing.id).await,
            (4, 1)
        );
    }
    let (status, _, body) = delete_rating(t.app.clone(), &listing, &second).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rating_average"], 0);
    assert_eq!(body["rating_count"], 0);
    assert!(body["my_stars"].is_null());
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (0, 0)
    );
}

#[tokio::test]
async fn concurrent_rating_writers_match_actual_rows() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let first = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let second = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "rating fixture", "active").await;

    let (one, five) = tokio::join!(
        put_rating(t.app.clone(), &listing, &first, serde_json::json!(1)),
        put_rating(t.app.clone(), &listing, &first, serde_json::json!(5)),
    );
    assert_eq!(one.0, StatusCode::OK);
    assert_eq!(five.0, StatusCode::OK);
    assert_eq!(assert_counter_matches_rows(&t.pool, listing.id).await.1, 1);

    let (added, removed) = tokio::join!(
        put_rating(t.app.clone(), &listing, &second, serde_json::json!(4)),
        delete_rating(t.app.clone(), &listing, &first),
    );
    assert_eq!(added.0, StatusCode::OK);
    assert_eq!(removed.0, StatusCode::OK);
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (4, 1)
    );

    let (three, two) = tokio::join!(
        put_rating(t.app.clone(), &listing, &first, serde_json::json!(3)),
        put_rating(t.app.clone(), &listing, &second, serde_json::json!(2)),
    );
    assert_eq!(three.0, StatusCode::OK);
    assert_eq!(two.0, StatusCode::OK);
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (5, 2)
    );

    let (first_delete, second_delete) = tokio::join!(
        delete_rating(t.app.clone(), &listing, &first),
        delete_rating(t.app.clone(), &listing, &second),
    );
    assert_eq!(first_delete.0, StatusCode::OK);
    assert_eq!(second_delete.0, StatusCode::OK);
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (0, 0)
    );
}

#[tokio::test]
async fn rating_limit_is_per_user_and_comes_after_validation_and_csrf() {
    let cfg = common::test_config_with(|cfg| {
        cfg.rating_user_limit = 2;
        cfg.rating_user_window_secs = 60;
    });
    let Some(t) = setup_with(cfg).await else {
        return;
    };
    let seller = create_user(&t.pool, &t.cfg, "+989100000001").await;
    let first = create_user(&t.pool, &t.cfg, "+989100000002").await;
    let second = create_user(&t.pool, &t.cfg, "+989100000003").await;
    let listing = create_listing(&t.pool, seller.id, "rating fixture", "active").await;

    for stars in [4, 5] {
        assert_eq!(
            put_rating(t.app.clone(), &listing, &first, serde_json::json!(stars))
                .await
                .0,
            StatusCode::OK
        );
    }
    let (status, _, body) = put_rating(t.app.clone(), &listing, &first, serde_json::json!(0)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "validation precedes the limiter"
    );
    assert_eq!(body["error"]["code"], "validation_error");
    let (status, _, body) = send(
        t.app.clone(),
        Method::PUT,
        &rating_uri(&listing.slug),
        Some(&first),
        None,
        Some(serde_json::json!({ "stars": 3 })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "CSRF precedes the limiter");
    assert_eq!(body["error"]["code"], "csrf_failed");

    let (status, headers, body) =
        put_rating(t.app.clone(), &listing, &first, serde_json::json!(3)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], "rate_limited");
    let retry_after: u64 = headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .expect("numeric Retry-After");
    assert!((1..=60).contains(&retry_after));
    assert_eq!(
        delete_rating(t.app.clone(), &listing, &first).await.0,
        StatusCode::TOO_MANY_REQUESTS,
        "DELETE spends the same rating-mutation allowance"
    );

    assert_eq!(
        put_rating(t.app.clone(), &listing, &second, serde_json::json!(1))
            .await
            .0,
        StatusCode::OK,
        "another user has an independent window"
    );
    assert_eq!(
        assert_counter_matches_rows(&t.pool, listing.id).await,
        (6, 2)
    );
}

fn titles(body: &serde_json::Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("feed data")
        .iter()
        .map(|card| card["title"].as_str().expect("title").to_string())
        .collect()
}

#[tokio::test]
async fn popular_sort_uses_counters_written_by_real_rating_mutations() {
    let Some(t) = setup().await else { return };
    let seller = create_user(&t.pool, &t.cfg, "+989140000000").await;
    let mut raters = Vec::new();
    for index in 0..10 {
        raters.push(create_user(&t.pool, &t.cfg, &format!("+989141{index:06}")).await);
    }
    let one = create_listing(&t.pool, seller.id, "یک‌ستاره‌کامل", "active").await;
    let many = create_listing(&t.pool, seller.id, "خیلی‌خوب", "active").await;
    let _none = create_listing(&t.pool, seller.id, "بدون‌امتیاز", "active").await;

    assert_eq!(
        put_rating(t.app.clone(), &one, &raters[0], serde_json::json!(5))
            .await
            .0,
        StatusCode::OK
    );
    for (index, rater) in raters.iter().enumerate() {
        let stars = if index == 9 { 3 } else { 5 };
        assert_eq!(
            put_rating(t.app.clone(), &many, rater, serde_json::json!(stars))
                .await
                .0,
            StatusCode::OK
        );
    }
    assert_eq!(assert_counter_matches_rows(&t.pool, one.id).await, (5, 1));
    assert_eq!(
        assert_counter_matches_rows(&t.pool, many.id).await,
        (48, 10)
    );

    let (status, _, body) = send(
        t.app.clone(),
        Method::GET,
        "/api/v1/listings?sort=popular",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        titles(&body),
        vec!["خیلی‌خوب", "یک‌ستاره‌کامل", "بدون‌امتیاز"],
        "existing Bayesian ordering reads mutation-written counters: {body}"
    );
}
