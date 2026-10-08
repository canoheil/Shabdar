//! Shared application state handed to handlers (axum state).

use sqlx::MySqlPool;

use crate::config::Config;

/// Everything handlers need. Cloning is cheap: the pool is a handle and the
/// config is small and immutable for the process lifetime.
#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub config: Config,
}
