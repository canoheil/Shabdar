//! The single router builder every task extends (docs/PHASE1_PLAN.md).
//! Handlers stay thin: they parse HTTP and call one service function.
//! Routes so far: GET /api/v1/health.

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{middleware, routing::get, Json, Router};

use crate::request_id::request_id_middleware;
use crate::state::AppState;

/// Builds the full router. New endpoints are added here, not in main.rs.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        // Task 1 placeholder until real routes exist; a later task turns the
        // fallback into a proper 404.
        .fallback(placeholder_ok)
        // Request IDs wrap every route, fallback included.
        .layer(middleware::from_fn(request_id_middleware))
        .with_state(state)
}

/// Placeholder handler: the literal string `ok`.
async fn placeholder_ok() -> &'static str {
    "ok"
}

/// GET /api/v1/health — approved shape (plan review §5):
/// 200 {"status":"ok","database":"ok"} when the database answers,
/// 503 {"status":"error","database":"error"} otherwise.
/// Bounded: this 2 s cap plus the pool's 3 s acquire timeout.
async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query("SELECT 1").execute(&state.pool),
    )
    .await
    .is_ok_and(|result| result.is_ok());

    let (http_status, field) = if db_ok {
        (StatusCode::OK, "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "error")
    };
    (
        http_status,
        Json(serde_json::json!({ "status": field, "database": field })),
    )
}
