//! MySQL connection pool and runtime migrations.
//! sqlx runtime queries only — no compile-time macros (AGENTS.md rule).

use std::path::Path;
use std::time::Duration;

use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;

use crate::config::Config;

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
