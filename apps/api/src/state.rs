//! Shared application state handed to handlers (axum state).

use sqlx::MySqlPool;

use crate::config::Config;
use crate::ratelimit::RateLimiter;

/// Everything handlers need. Cloning is cheap: the pool is a handle and the
/// config is small and immutable for the process lifetime.
#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub config: Config,
    /// Phase 5 abuse counters. Each process (and each test app) owns its own
    /// map: clones of the state share it, a new state starts empty.
    pub limiter: RateLimiter,
}
