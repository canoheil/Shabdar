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
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
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

/// Every table the migrations own, children first. The three chat tables lead
/// because `conversations.last_message_id` and `messages.conversation_id` form
/// an intentional cycle: a plain ordered DROP cannot resolve it.
pub const OWNED_TABLES: [&str; 23] = [
    "conversation_reads",
    "messages",
    "conversations",
    "advertising_requests",
    "advertising_slots",
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
];

/// Drops every owned table on ONE pinned connection with
/// `FOREIGN_KEY_CHECKS=0`, then restores the setting before the connection
/// returns to the pool. Pinning the connection matters: the setting is
/// per-session, so a pooled DROP could otherwise run on a connection where
/// checks are still enabled and fail on the circular chat keys.
///
/// Destructive: callers reach this only through `test_pool()`, which refuses
/// every database whose name does not end in `_test`.
pub async fn drop_owned_tables(pool: &MySqlPool) {
    let mut conn = pool.acquire().await.expect("scratch reset connection");
    sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
        .execute(&mut *conn)
        .await
        .expect("disable foreign key checks");
    for table in OWNED_TABLES {
        let sql = format!("DROP TABLE IF EXISTS {table}"); // fixed literals, not user input
        sqlx::query(&sql)
            .execute(&mut *conn)
            .await
            .expect("drop failed");
    }
    sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
        .execute(&mut *conn)
        .await
        .expect("restore foreign key checks");
}

/// Empty scratch schema restored by the real migrations. Every full-schema
/// fixture uses this single implementation so chat tables cannot be forgotten.
pub async fn reset_database(pool: &MySqlPool) {
    drop_owned_tables(pool).await;
    shabdar_api::db::run_migrations(pool)
        .await
        .expect("migrations apply");
}

/// Config with throwaway values for tests (defaults per the config contract).
pub fn test_config() -> Config {
    Config {
        bind_addr: "127.0.0.1:0".to_string(),
        database_url: std::env::var("TEST_DATABASE_URL").unwrap_or_default(),
        otp_pepper: "test-pepper".to_string(),
        otp_dev_mode: false,
        otp_dev_code: None,
        otp_ttl_secs: 120,
        otp_resend_secs: 60,
        otp_max_attempts: 5,
        trust_proxy: false,
        // Deliberately generous so existing tests never trip an abuse limit;
        // the rate-limit tests lower exactly the number they exercise.
        otp_request_ip_limit: 1000,
        otp_request_ip_window_secs: 60,
        otp_verify_ip_limit: 1000,
        otp_verify_ip_window_secs: 60,
        otp_phone_daily_limit: 1000,
        otp_phone_daily_window_secs: 86_400,
        otp_sms_global_limit: 1000,
        otp_sms_global_window_secs: 3_600,
        rating_user_limit: 1000,
        rating_user_window_secs: 600,
        comment_user_limit: 1000,
        comment_user_window_secs: 600,
        report_user_limit: 1000,
        report_user_window_secs: 600,
        // Generous by default; chat tests lower exactly the window they probe.
        message_user_limit: 1000,
        message_user_window_secs: 600,
        session_ttl_secs: 2_592_000,
        cookie_name: "shabdar_session".to_string(),
        cookie_secure: false,
        media_dir: "./data/media".to_string(),
        max_image_bytes: 5_242_880,
        max_images_per_listing: 8,
    }
}

/// `test_config()` with one or more fields changed (rate-limit tests need a
/// small limit without a second copy of the whole struct).
pub fn test_config_with(tweak: impl FnOnce(&mut Config)) -> Config {
    let mut config = test_config();
    tweak(&mut config);
    config
}

/// Builds the app router for oneshot tests. No database contact here:
/// create_pool is lazy, so a router against an unreachable URL is legal too.
/// Tests that need schema run migrations on their own pool first.
pub async fn spawn_app(config: Config) -> axum::Router {
    let pool = shabdar_api::db::create_pool(&config);
    shabdar_api::routes::router(shabdar_api::state::AppState {
        pool,
        config,
        limiter: shabdar_api::ratelimit::RateLimiter::new(),
    })
}
