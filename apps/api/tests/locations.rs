//! Phase 14: one country-wide city-by-slug read for clean `/city/{slug}` URLs.
//!
//! City slugs are unique per province, not globally, so a clean URL cannot
//! validate itself from the province list or the city list alone. This endpoint
//! answers "which city and province does this slug mean" in one query, and the
//! Nuxt route 404s when the answer is nothing.

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn send(app: axum::Router, method: Method, path: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).expect("JSON body"))
}

/// Reference reads need the seeded rows, so this file never drops tables (same
/// exposure as `listings.rs` and `health.rs`). It only adds and removes its own
/// uniquely-slugged fixtures.
async fn reference_app() -> Option<axum::Router> {
    let pool = common::test_pool().await?;
    shabdar_api::db::run_migrations(&pool)
        .await
        .expect("migrations apply");
    Some(common::spawn_app(common::test_config()).await)
}

#[tokio::test]
async fn city_by_slug_resolves_the_seeded_rasht_city() {
    let Some(app) = reference_app().await else {
        return; // skipped without TEST_DATABASE_URL
    };
    let (status, body) = send(app, Method::GET, "/api/v1/locations/cities/rasht").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let city = &body["data"];
    assert_eq!(city["slug"], "rasht");
    assert_eq!(city["name_fa"], "رشت");
    // The province is what the clean URL needs for its filter and its breadcrumb.
    assert_eq!(city["province_slug"], "gilan");
    assert_eq!(city["province_name_fa"], "گیلان");
}

#[tokio::test]
async fn city_by_slug_is_404_for_an_unknown_slug() {
    let Some(app) = reference_app().await else {
        return; // skipped
    };
    let (status, body) = send(app, Method::GET, "/api/v1/locations/cities/no-such-city").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn city_by_slug_picks_the_lowest_id_deterministically() {
    let Some(app) = reference_app().await else {
        return; // skipped
    };
    let pool = common::test_pool().await.expect("pool");
    // Reference reads never drop tables, so a previous interrupted run could
    // have left these fixtures behind and the unique slugs would reject them.
    sqlx::query("DELETE FROM cities WHERE slug = 'duplicate-city'")
        .execute(&pool)
        .await
        .expect("pre-clean cities");
    sqlx::query("DELETE FROM provinces WHERE slug IN ('test-alpha', 'test-beta')")
        .execute(&pool)
        .await
        .expect("pre-clean provinces");
    // Two provinces genuinely share some city names; the national snapshot only
    // guarantees uniqueness inside a province. Seed a deliberate collision in
    // insertion order, so the lowest city id belongs to `test-alpha`.
    let mut ids = Vec::new();
    for province in ["test-alpha", "test-beta"] {
        sqlx::query("INSERT INTO provinces (name_fa, slug) VALUES ('استان آزمون', ?)")
            .bind(province)
            .execute(&pool)
            .await
            .expect("province insert");
        sqlx::query(
            "INSERT INTO cities (province_id, name_fa, slug, is_launch_city) \
             SELECT id, 'شهر تکراری', 'duplicate-city', FALSE \
             FROM provinces WHERE slug = ?",
        )
        .bind(province)
        .execute(&pool)
        .await
        .expect("city insert");
        let id: u32 = sqlx::query_scalar(
            "SELECT c.id FROM cities c JOIN provinces p ON p.id = c.province_id \
             WHERE p.slug = ? AND c.slug = 'duplicate-city'",
        )
        .bind(province)
        .fetch_one(&pool)
        .await
        .expect("city id");
        ids.push((id, province.to_string()));
    }
    let (lowest_id, lowest_province) = ids
        .iter()
        .min_by_key(|(id, _)| *id)
        .expect("two seeded colliding cities")
        .clone();

    let (status, first) = send(
        app.clone(),
        Method::GET,
        "/api/v1/locations/cities/duplicate-city",
    )
    .await;
    let (_, again) = send(app, Method::GET, "/api/v1/locations/cities/duplicate-city").await;

    // Clean up before asserting, so a failed expectation cannot leave the
    // collision rows behind for the next run.
    sqlx::query("DELETE FROM cities WHERE slug = 'duplicate-city'")
        .execute(&pool)
        .await
        .expect("cleanup cities");
    sqlx::query("DELETE FROM provinces WHERE slug IN ('test-alpha', 'test-beta')")
        .execute(&pool)
        .await
        .expect("cleanup provinces");

    // A collision must resolve to the lowest city id, not a random one per
    // request, so the rendered page and its canonical always agree.
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(lowest_id, ids[0].0, "insertion order gives the lowest id");
    assert_ne!(
        ids[0].0, ids[1].0,
        "collision fixtures must have distinct ids"
    );
    assert_eq!(first["data"]["slug"], "duplicate-city");
    assert_eq!(first["data"]["province_slug"], lowest_province);
    assert_eq!(again["data"]["province_slug"], lowest_province);
}
