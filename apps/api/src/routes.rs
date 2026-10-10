//! The single router builder every task extends (docs/PHASE1_PLAN.md).
//! Handlers stay thin: they parse HTTP and call one service function.
//! Routes so far: GET /api/v1/health, POST /api/v1/auth/otp/request,
//! POST /api/v1/auth/otp/verify, GET/PATCH /api/v1/me,
//! POST /api/v1/auth/logout.

use std::collections::BTreeMap;
use std::time::Duration;

use axum::extract::State;
use axum::http::header::{HeaderName, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{
    middleware,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::error::ApiError;
use crate::request_id::request_id_middleware;
use crate::session::CurrentUser;
use crate::state::AppState;

/// Builds the full router. New endpoints are added here, not in main.rs.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/auth/otp/request", post(otp_request))
        .route("/api/v1/auth/otp/verify", post(otp_verify))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/me", get(me).patch(update_me))
        // Unknown paths: 404 in the documented envelope (plan Task 4).
        .fallback(not_found)
        // Request IDs wrap every route, fallback included.
        .layer(middleware::from_fn(request_id_middleware))
        .with_state(state)
}

/// Fallback: unknown paths get 404 `not_found` in the documented envelope
/// (plan Task 4).
async fn not_found() -> axum::response::Response {
    ApiError::NotFound.into_response()
}

/// GET /api/v1/me — the pinned Phase 1 user shape (PHASE2 plan review 5):
/// {"user":{"public_id","display_name","is_admin"}}.
async fn me(user: CurrentUser) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "user": {
            "public_id": user.public_id,
            "display_name": user.display_name,
            "is_admin": user.is_admin,
        }
    }))
}

/// Request body for PATCH /api/v1/me (plan Task 2).
#[derive(Deserialize)]
struct UpdateMeBody {
    display_name: Option<String>,
}

/// PATCH /api/v1/me — set the display name (plan Task 2). Validation:
/// required, trimmed, 1–80 characters (chars, not bytes — VARCHAR(80) under
/// utf8mb4). Missing/empty after trim → `required`; over 80 chars → `too_long`.
async fn update_me(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Json(body): Json<UpdateMeBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // CSRF first (plan Task 8): a failed check must not mutate anything.
    crate::csrf::check_csrf(&headers)?;
    let name = body.display_name.unwrap_or_default();
    let trimmed = name.trim();
    if trimmed.is_empty() {
        let mut fields = BTreeMap::new();
        fields.insert("display_name".to_string(), "required".to_string());
        return Err(ApiError::Validation { fields });
    }
    if trimmed.chars().count() > 80 {
        let mut fields = BTreeMap::new();
        fields.insert("display_name".to_string(), "too_long".to_string());
        return Err(ApiError::Validation { fields });
    }

    crate::user_service::update_display_name(&state.pool, user.user_id, trimmed).await?;
    Ok(Json(serde_json::json!({
        "user": {
            "public_id": user.public_id,
            "display_name": trimmed,
            "is_admin": user.is_admin,
        }
    })))
}

/// POST /api/v1/auth/logout — revokes exactly the presented session and
/// clears both cookies (plan Task 3 + Task 8). Returns exactly {"ok":true}.
async fn logout(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
) -> Result<([(HeaderName, String); 2], Json<serde_json::Value>), ApiError> {
    // CSRF first (plan Task 8): a failed check must not revoke anything.
    crate::csrf::check_csrf(&headers)?;
    crate::session::delete_session(&state.pool, user.session_id).await?;
    let clear_cookie = crate::session::build_clear_cookie(&state.config);
    let clear_csrf = crate::csrf::build_csrf_clear_cookie(&state.config);
    Ok((
        [(SET_COOKIE, clear_cookie), (SET_COOKIE, clear_csrf)],
        Json(serde_json::json!({ "ok": true })),
    ))
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
        [(HeaderName, String); 2],
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
    // Two Set-Cookie headers (approved CSRF proposal): the session cookie and
    // the double-submit shabdar_csrf cookie (a fresh ULID per login).
    let cookie = crate::session::build_set_cookie(&state.config, &outcome.session_token);
    let csrf_token = ulid::Ulid::new().to_string();
    let csrf_cookie = crate::csrf::build_csrf_cookie(&state.config, &csrf_token);
    Ok((
        StatusCode::OK,
        [(SET_COOKIE, cookie), (SET_COOKIE, csrf_cookie)],
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
