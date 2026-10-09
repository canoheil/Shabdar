//! The single router builder every task extends (docs/PHASE1_PLAN.md).
//! Handlers stay thin: they parse HTTP and call one service function.
//! Routes so far: GET /api/v1/health, POST /api/v1/auth/otp/request,
//! POST /api/v1/auth/otp/verify.

use std::collections::BTreeMap;
use std::time::Duration;

use axum::extract::State;
use axum::http::header::{HeaderName, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{
    middleware,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::error::ApiError;
use crate::request_id::request_id_middleware;
use crate::state::AppState;

/// Builds the full router. New endpoints are added here, not in main.rs.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/auth/otp/request", post(otp_request))
        .route("/api/v1/auth/otp/verify", post(otp_verify))
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

/// Request body for POST /api/v1/auth/otp/request (API.md §2).
#[derive(Deserialize)]
struct OtpRequestBody {
    phone: Option<String>,
    purpose: Option<String>,
}

/// POST /api/v1/auth/otp/request — exact response body per API.md §2:
/// {"challenge_id","expires_in_seconds","resend_after_seconds"}.
async fn otp_request(
    State(state): State<AppState>,
    Json(body): Json<OtpRequestBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let phone = body.phone.unwrap_or_default();
    let purpose = body.purpose.unwrap_or_default();
    let mut fields = BTreeMap::new();
    if phone.is_empty() {
        fields.insert("phone".to_string(), "required".to_string());
    }
    if purpose.is_empty() {
        fields.insert("purpose".to_string(), "required".to_string());
    }
    if !fields.is_empty() {
        return Err(ApiError::Validation { fields });
    }

    let code = crate::otp::generate_code();
    let created =
        crate::otp_service::create_challenge(&state.pool, &state.config, &phone, &purpose, &code)
            .await?;

    // Dev print (approved form), only in debug builds with OTP_DEV_MODE on:
    // the phone is masked to its last 4 digits, never logged in full.
    // ponytail: cfg!(debug_assertions) checks a compile-time fact at runtime;
    // the compile-time enforcement upgrade is a build-feature gate (e.g.
    // `#[cfg(feature = "dev-otp")]`) once a release profile exists.
    if state.config.otp_dev_mode && cfg!(debug_assertions) {
        if let Ok(e164) = crate::phone::normalize_iranian_mobile(&phone) {
            let masked = format!("***{}", &e164[e164.len() - 4..]);
            tracing::info!("DEV OTP for {masked}: {code}");
        }
    }

    Ok(Json(serde_json::json!({
        "challenge_id": created.public_id,
        "expires_in_seconds": state.config.otp_ttl_secs,
        "resend_after_seconds": state.config.otp_resend_secs,
    })))
}

/// Request body for POST /api/v1/auth/otp/verify (API.md §2).
#[derive(Deserialize)]
struct OtpVerifyBody {
    challenge_id: Option<String>,
    code: Option<String>,
}

/// POST /api/v1/auth/otp/verify — sets the session cookie and returns
/// {"user":{"public_id","display_name","is_admin"},"is_new_user"}.
async fn otp_verify(
    State(state): State<AppState>,
    Json(body): Json<OtpVerifyBody>,
) -> Result<
    (
        StatusCode,
        [(HeaderName, String); 1],
        Json<serde_json::Value>,
    ),
    ApiError,
> {
    let challenge_id = body.challenge_id.unwrap_or_default();
    let code = body.code.unwrap_or_default();
    let mut fields = BTreeMap::new();
    if challenge_id.is_empty() {
        fields.insert("challenge_id".to_string(), "required".to_string());
    }
    if code.is_empty() {
        fields.insert("code".to_string(), "required".to_string());
    }
    if !fields.is_empty() {
        return Err(ApiError::Validation { fields });
    }

    let outcome =
        crate::otp_service::verify_challenge(&state.pool, &state.config, &challenge_id, &code)
            .await?;
    let cookie = crate::session::build_set_cookie(&state.config, &outcome.session_token);
    Ok((
        StatusCode::OK,
        [(SET_COOKIE, cookie)],
        Json(serde_json::json!({
            "user": {
                "public_id": outcome.user.public_id,
                "display_name": outcome.user.display_name,
                "is_admin": outcome.user.is_admin,
            },
            "is_new_user": outcome.is_new_user,
        })),
    ))
}
