//! Shared helpers for integration tests. Included via `mod common;` from each
//! tests/*.rs file; this file also compiles as an empty standalone target.
#![allow(dead_code)]

use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;

use shabdar_api::config::Config;

/// Extracts the database name from a mysql:// URL (no parsing crate needed).
///
/// "mysql://u:p@127.0.0.1:3306/shabdar_test?charset=utf8mb4" → "shabdar_test"
pub fn db_name_from_url(url: &str) -> Option<&str> {
    let after_scheme = url.strip_prefix("mysql://")?;
    let path = after_scheme.split('/').nth(1)?; // skip "user:pass@host:port"
    let name = path.split(['?', '#']).next()?;
    if name.is_empty() { None } else { Some(name) }
}

/// Destructive test setup is allowed only on a *_test database.
/// Anything else — shabdar_dev included — is refused.
pub fn is_test_database(name: &str) -> bool {
    name.ends_with("_test")
}

/// A pool to the test database, or None (with a printed note) when the test
/// should skip: TEST_DATABASE_URL unset, or pointing at a non-*_test database.
/// Tests NEVER read DATABASE_URL.
pub async fn test_pool() -> Option<MySqlPool> {
    let url = match std::env::var("TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("note: DB tests skipped — TEST_DATABASE_URL is not set");
            return None;
        }
    };
    let name = db_name_from_url(&url).unwrap_or("");
    if !is_test_database(name) {
        eprintln!("REFUSING to run DB tests against {name:?}: database name must end in _test");
        return None;
    }
    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("TEST_DATABASE_URL must reach the running test database");
    Some(pool)
}

/// Config with throwaway values for tests (defaults per the config contract).
pub fn test_config() -> Config {
    Config {
        bind_addr: "127.0.0.1:0".to_string(),
        database_url: std::env::var("TEST_DATABASE_URL").unwrap_or_default(),
        otp_pepper: "test-pepper".to_string(),
        otp_dev_mode: false,
        otp_ttl_secs: 120,
        otp_resend_secs: 60,
        otp_max_attempts: 5,
        session_ttl_secs: 2_592_000,
        cookie_name: "shabdar_session".to_string(),
        cookie_secure: false,
        media_dir: "./data/media".to_string(),
        max_image_bytes: 5_242_880,
        max_images_per_listing: 8,
    }
}

/// Builds the app router for oneshot tests. No database contact here:
/// create_pool is lazy, so a router against an unreachable URL is legal too.
/// Tests that need schema run migrations on their own pool first.
pub async fn spawn_app(config: Config) -> axum::Router {
    let pool = shabdar_api::db::create_pool(&config);
    shabdar_api::routes::router(shabdar_api::state::AppState { pool, config })
}
