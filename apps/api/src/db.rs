//! MySQL connection pool, runtime migrations, and the shared pair-lock helper.
//! sqlx runtime queries only — no compile-time macros (AGENTS.md rule).

use std::path::Path;
use std::time::Duration;

use sqlx::mysql::MySqlPoolOptions;
use sqlx::{MySql, MySqlPool, Transaction};

use crate::config::Config;
use crate::error::ApiError;

/// Small pool for a phone-hosted server (5 connections, 3 s acquire timeout).
/// Connects lazily: the first query is the first real database contact.
pub fn create_pool(config: &Config) -> MySqlPool {
    MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect_lazy(&config.database_url)
        .expect("DATABASE_URL must be a valid mysql:// connection string")
}

/// Applies apps/api/migrations/*.sql in filename order. The directory is read
/// at RUNTIME (Migrator, never the migrate! macro), so the process must be
/// started from the crate directory (cargo run / cargo test from apps/api).
pub async fn run_migrations(pool: &MySqlPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate::Migrator::new(Path::new("migrations"))
        .await?
        .run(pool)
        .await
}

/// The canonical unordered pair key: `(lower user id, higher user id)`.
/// One pair therefore has exactly one conversation row regardless of who
/// initiates or which party sells on a later listing.
pub fn canonical_pair(a: u64, b: u64) -> (u64, u64) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Locks both participant rows in ascending id order and returns immediately
/// if either account is gone.
///
/// Chat start/send and directed blocks must all take this lock before they
/// inspect block state or write, which is what linearizes "message first" and
/// "block first". Ascending order prevents a lock cycle between two users who
/// act at the same time. Database conflicts surface as `RetryableDatabase` so
/// the caller's whole-transaction retry can run.
pub async fn lock_user_pair(
    tx: &mut Transaction<'_, MySql>,
    a: u64,
    b: u64,
) -> Result<(), ApiError> {
    let (low, high) = canonical_pair(a, b);
    for user_id in [low, high] {
        sqlx::query("SELECT id FROM users WHERE id = ? FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(crate::error::database_error)?
            .ok_or(ApiError::NotFound)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::canonical_pair;

    #[test]
    fn canonical_pair_is_order_independent() {
        assert_eq!(canonical_pair(9, 4), (4, 9));
        assert_eq!(canonical_pair(4, 9), (4, 9));
        assert_eq!(canonical_pair(7, 7), (7, 7));
    }
}
