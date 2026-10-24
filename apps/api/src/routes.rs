//! The single router builder every task extends (docs/PHASE1_PLAN.md).
//! Handlers stay thin: they parse HTTP and call one service function.
//! Routes so far: GET /api/v1/health, POST /api/v1/auth/otp/request,
//! POST /api/v1/auth/otp/verify, GET/PATCH /api/v1/me,
//! POST /api/v1/auth/logout, GET /api/v1/categories,
//! GET /api/v1/locations/provinces,
//! GET /api/v1/locations/provinces/{slug}/cities.

use std::collections::BTreeMap;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{
    middleware,
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use serde::Deserialize;
use sqlx::Row;

use crate::api_json::ApiJson;
use crate::client_ip::{client_ip, PeerAddr};
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
        .route("/api/v1/me/favorites", get(my_favorites))
        .route(
            "/api/v1/me/favorites/{public_id}",
            put(save_favorite).delete(unsave_favorite),
        )
        .route("/api/v1/me/listings", get(my_listings))
        .route(
            "/api/v1/me/dashboard/summary",
            get(dashboard_summary),
        )
        .route("/api/v1/listings", post(create_listing).get(feed_listings))
        // One template for all single-listing methods (axum rejects two
        // registrations of the same shape under different capture names).
        .route(
            "/api/v1/listings/{public_id}",
            patch(edit_listing).delete(delete_listing).get(listing_detail),
        )
        .route("/api/v1/sitemap/listings", get(sitemap_listings))
        .route("/api/v1/listings/{public_id}/submit", post(submit_listing))
        .route("/api/v1/listings/{public_id}/mark-sold", post(mark_sold_listing))
        .route("/api/v1/listings/{public_id}/archive", post(archive_listing))
        .route("/api/v1/listings/{public_id}/images", post(upload_image))
        .route(
            "/api/v1/listings/{public_id}/images/{image_id}",
            delete(delete_image),
        )
        .route("/api/v1/listings/{public_id}/images/order", patch(images_order))
        .route("/api/v1/admin/listings/queue", get(admin_queue))
        .route("/api/v1/admin/listings/{public_id}", get(admin_detail))
        .route("/api/v1/admin/listings/{public_id}/approve", post(admin_approve))
        .route("/api/v1/admin/listings/{public_id}/reject", post(admin_reject))
        .route("/api/v1/admin/listings/{public_id}/remove", post(admin_remove))
        .route("/media/{*key}", get(serve_media))
        .route("/api/v1/categories", get(categories))
        .route("/api/v1/locations/provinces", get(provinces))
        .route(
            "/api/v1/locations/provinces/{slug}/cities",
            get(province_cities),
        )
        // Unknown paths: 404 in the documented envelope (plan Task 4).
        .fallback(not_found)
        // Request IDs wrap every route, fallback included.
        .layer(middleware::from_fn(request_id_middleware))
        .with_state(state)
}

/// Two `Set-Cookie` headers on one response. A `[(HeaderName, String); 2]`
/// return does NOT work: axum-core converts the array with
/// `HeaderMap::insert` per pair, and insert replaces — the second cookie
/// wipes the first (regression: the session cookie never reached the client,
/// caught by `verify_register_happy_path_creates_user_and_session`).
/// `HeaderMap::extend` appends, so both values survive.
fn two_set_cookies(first: String, second: String) -> HeaderMap {
    let mut map = HeaderMap::new();
    map.append(SET_COOKIE, first.parse().expect("server-built cookie is ascii"));
    map.append(SET_COOKIE, second.parse().expect("server-built cookie is ascii"));
    map
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
    ApiJson(body): ApiJson<UpdateMeBody>,
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
) -> Result<(HeaderMap, Json<serde_json::Value>), ApiError> {
    // CSRF first (plan Task 8): a failed check must not revoke anything.
    crate::csrf::check_csrf(&headers)?;
    crate::session::delete_session(&state.pool, user.session_id).await?;
    let clear_cookie = crate::session::build_clear_cookie(&state.config);
    let clear_csrf = crate::csrf::build_csrf_clear_cookie(&state.config);
    Ok((
        two_set_cookies(clear_cookie, clear_csrf),
        Json(serde_json::json!({ "ok": true })),
    ))
}

/// GET /api/v1/me/dashboard/summary — the seller's own counts (Phase 4
/// Task 4). Session required; read-only, so no CSRF header.
async fn dashboard_summary(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        crate::listings::summary(&state.pool, user.user_id).await?,
    ))
}

/// GET /api/v1/me/favorites — current viewer's active saved cards.
async fn my_favorites(
    State(state): State<AppState>,
    user: CurrentUser,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        crate::favorites::list(
            &state.pool,
            user.user_id,
            params.get("cursor").map(String::as_str),
            params.get("limit").map(String::as_str),
        )
        .await?,
    ))
}

/// PUT /api/v1/me/favorites/{public_id} — save an active listing.
async fn save_favorite(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    crate::favorites::save(&state.pool, user.user_id, &public_id).await?;
    Ok(Json(serde_json::json!({ "favorited": true })))
}

/// DELETE /api/v1/me/favorites/{public_id} — remove the current user's row.
async fn unsave_favorite(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    crate::favorites::unsave(&state.pool, user.user_id, &public_id).await?;
    Ok(Json(serde_json::json!({ "favorited": false })))
}

/// GET /api/v1/me/listings — the seller's own listings (Phase 3 Task 3).
/// Query: `status` (all|pending|active|rejected|sold|archived, default all).
async fn my_listings(
    State(state): State<AppState>,
    user: CurrentUser,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let status = params.get("status").map(String::as_str).unwrap_or("all");
    let items = crate::listings::my_listings(&state.pool, user.user_id, status).await?;
    Ok(Json(serde_json::json!({ "data": items })))
}

/// POST /api/v1/listings — create a draft (Phase 3 Task 3).
async fn create_listing(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    ApiJson(body): ApiJson<crate::listings::CreateBody>,
) -> Result<Json<crate::listings::ListingOut>, ApiError> {
    // CSRF first: a failed check must not create anything.
    crate::csrf::check_csrf(&headers)?;
    Ok(Json(crate::listings::create_draft(&state.pool, user.user_id, body).await?))
}

/// PATCH /api/v1/listings/{public_id} — edit a draft/rejected listing.
async fn edit_listing(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
    ApiJson(body): ApiJson<crate::listings::EditBody>,
) -> Result<Json<crate::listings::ListingOut>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    Ok(Json(crate::listings::edit_listing(&state.pool, user.user_id, &public_id, body).await?))
}

/// POST /api/v1/listings/{public_id}/submit — draft|rejected → pending.
async fn submit_listing(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
) -> Result<Json<crate::listings::ListingOut>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    Ok(Json(crate::listings::transition(&state.pool, user.user_id, &public_id, "submit").await?))
}

/// POST /api/v1/listings/{public_id}/mark-sold — active → sold.
async fn mark_sold_listing(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
) -> Result<Json<crate::listings::ListingOut>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    Ok(Json(crate::listings::transition(&state.pool, user.user_id, &public_id, "mark-sold").await?))
}

/// POST /api/v1/listings/{public_id}/archive — active → archived.
async fn archive_listing(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
) -> Result<Json<crate::listings::ListingOut>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    Ok(Json(crate::listings::transition(&state.pool, user.user_id, &public_id, "archive").await?))
}

/// DELETE /api/v1/listings/{public_id} — soft-delete draft|rejected|archived.
async fn delete_listing(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    crate::listings::transition(&state.pool, user.user_id, &public_id, "delete").await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /api/v1/listings/{public_id}/images — multipart upload (Phase 3
/// Task 5). CSRF is checked before any file or row is written; multipart
/// parsing itself writes nothing.
async fn upload_image(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<crate::listing_images::ImageOut>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    let mut bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart.next_field().await.map_err(|_| {
        let mut fields = BTreeMap::new();
        fields.insert("images".to_string(), "unreadable".to_string());
        ApiError::Validation { fields }
    })? {
        if field.name() == Some("file") {
            bytes = Some(field.bytes().await.map_err(|_| {
                let mut fields = BTreeMap::new();
                fields.insert("images".to_string(), "unreadable".to_string());
                ApiError::Validation { fields }
            })?.to_vec());
            break;
        }
    }
    let Some(bytes) = bytes else {
        let mut fields = BTreeMap::new();
        fields.insert("images".to_string(), "required".to_string());
        return Err(ApiError::Validation { fields });
    };
    Ok(Json(
        crate::listing_images::add_image(&state.pool, &state.config, user.user_id, &public_id, &bytes)
            .await?,
    ))
}

/// DELETE /api/v1/listings/{public_id}/images/{image_id} — removes row + file.
async fn delete_image(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path((public_id, image_id)): Path<(String, u64)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    crate::listing_images::remove_image(&state.pool, &state.config, user.user_id, &public_id, image_id)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Request body for admin reject/remove (Phase 3 Task 6).
#[derive(Deserialize)]
struct AdminReasonBody {
    reason: Option<String>,
}

/// GET /api/v1/admin/listings/queue — pending oldest first (admin only).
async fn admin_queue(
    State(state): State<AppState>,
    user: CurrentUser,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::admin::require_admin(&user)?;
    Ok(Json(
        crate::admin::queue(
            &state.pool,
            params.get("cursor").map(String::as_str),
            params.get("limit").map(String::as_str),
        )
        .await?,
    ))
}

/// GET /api/v1/admin/listings/{public_id} — full view with seller info.
async fn admin_detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(public_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::admin::require_admin(&user)?;
    Ok(Json(crate::admin::admin_detail(&state.pool, &public_id).await?))
}

/// POST /api/v1/admin/listings/{public_id}/approve — pending → active.
async fn admin_approve(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
) -> Result<Json<crate::listings::ListingOut>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    crate::admin::require_admin(&user)?;
    Ok(Json(crate::admin::approve(&state.pool, &user, &public_id).await?))
}

/// POST /api/v1/admin/listings/{public_id}/reject — pending → rejected.
async fn admin_reject(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
    ApiJson(body): ApiJson<AdminReasonBody>,
) -> Result<Json<crate::listings::ListingOut>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    crate::admin::require_admin(&user)?;
    Ok(Json(crate::admin::reject(&state.pool, &user, &public_id, &body.reason).await?))
}

/// POST /api/v1/admin/listings/{public_id}/remove — active → removed.
async fn admin_remove(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
    ApiJson(body): ApiJson<AdminReasonBody>,
) -> Result<Json<crate::listings::ListingOut>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    crate::admin::require_admin(&user)?;
    Ok(Json(crate::admin::remove(&state.pool, &user, &public_id, &body.reason).await?))
}

/// Request body for PATCH …/images/order (Phase 3 Task 5).
#[derive(Deserialize)]
struct ImagesOrderBody {
    image_ids: Option<Vec<u64>>,
}

/// PATCH /api/v1/listings/{public_id}/images/order — sets display order.
async fn images_order(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(public_id): Path<String>,
    ApiJson(body): ApiJson<ImagesOrderBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::csrf::check_csrf(&headers)?;
    let Some(ids) = body.image_ids else {
        let mut fields = BTreeMap::new();
        fields.insert("image_ids".to_string(), "required".to_string());
        return Err(ApiError::Validation { fields });
    };
    crate::listing_images::set_order(&state.pool, user.user_id, &public_id, &ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /api/v1/listings — public feed (Phase 3 Task 4, Phase 4 Tasks 1–3).
/// Query: `category`, `city`, `province` (slugs), `q`, `sort`, `cursor`,
/// `limit`. No auth: only `active` listings can ever appear (SQL filter).
async fn feed_listings(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let page = crate::listings::feed(
        &state.pool,
        params.get("category").map(String::as_str),
        params.get("city").map(String::as_str),
        params.get("province").map(String::as_str),
        params.get("q").map(String::as_str),
        params.get("sort").map(String::as_str),
        params.get("cursor").map(String::as_str),
        params.get("limit").map(String::as_str),
    )
    .await?;
    Ok(Json(page))
}

/// GET /api/v1/listings/{slug} — public detail with optional viewer state.
async fn listing_detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    current_user: Result<CurrentUser, ApiError>,
) -> Result<axum::response::Response, ApiError> {
    let viewer_user_id = match current_user {
        Ok(user) => Some(user.user_id),
        Err(ApiError::Unauthorized) => None,
        Err(error) => return Err(error),
    };
    let body = crate::listings::detail(&state.pool, &slug, viewer_user_id).await?;
    let mut response = Json(body).into_response();
    if viewer_user_id.is_some() {
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("private, no-store"),
        );
    }
    Ok(response)
}

/// GET /api/v1/sitemap/listings — active slugs for the Nuxt sitemap.
async fn sitemap_listings(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let page = crate::listings::sitemap(
        &state.pool,
        params.get("cursor").map(String::as_str),
        params.get("limit").map(String::as_str),
    )
    .await?;
    Ok(Json(page))
}

/// GET /media/{*key} — serves one stored listing photo (public, like the
/// listing itself). Strict key shape; traversal and unknown keys → 404.
async fn serve_media(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<(StatusCode, [(axum::http::header::HeaderName, &'static str); 1], Vec<u8>), ApiError> {
    let (bytes, content_type) = crate::listing_images::read_media(&state.config, &key).await?;
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, content_type)],
        bytes,
    ))
}

/// GET /api/v1/categories — active categories, sort-ordered (Phase 3 Task 2).
/// Plain SELECTs: single reads need no service file (YAGNI).
async fn categories(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, parent_id, name_fa, slug, sort_order FROM categories \
         WHERE is_active = TRUE ORDER BY sort_order, id",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    let data: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            let id: u16 = row.try_get("id").map_err(|_| ApiError::Internal)?;
            let parent_id: Option<u16> =
                row.try_get("parent_id").map_err(|_| ApiError::Internal)?;
            let name_fa: String = row.try_get("name_fa").map_err(|_| ApiError::Internal)?;
            let slug: String = row.try_get("slug").map_err(|_| ApiError::Internal)?;
            // sort_order is signed SMALLINT per DATABASE.md §2 — decode as i16.
            let sort_order: i16 = row.try_get("sort_order").map_err(|_| ApiError::Internal)?;
            Ok(serde_json::json!({
                "id": id, "parent_id": parent_id, "name_fa": name_fa,
                "slug": slug, "sort_order": sort_order,
            }))
        })
        .collect::<Result<_, ApiError>>()?;
    Ok(Json(serde_json::json!({ "data": data })))
}

/// GET /api/v1/locations/provinces — all provinces (Phase 3 Task 2).
async fn provinces(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query("SELECT name_fa, slug FROM provinces ORDER BY id")
        .fetch_all(&state.pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    let data: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            let name_fa: String = row.try_get("name_fa").map_err(|_| ApiError::Internal)?;
            let slug: String = row.try_get("slug").map_err(|_| ApiError::Internal)?;
            Ok(serde_json::json!({ "name_fa": name_fa, "slug": slug }))
        })
        .collect::<Result<_, ApiError>>()?;
    Ok(Json(serde_json::json!({ "data": data })))
}

/// GET /api/v1/locations/provinces/{slug}/cities — cities of one province
/// (Phase 3 Task 2). Unknown province slug → 404 `not_found`.
async fn province_cities(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let province = sqlx::query("SELECT id FROM provinces WHERE slug = ?")
        .bind(&slug)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    let province_id: u16 = province.try_get("id").map_err(|_| ApiError::Internal)?;
    let rows = sqlx::query(
        "SELECT id, name_fa, slug, is_launch_city FROM cities \
         WHERE province_id = ? ORDER BY name_fa",
    )
    .bind(province_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    let data: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            let id: u32 = row.try_get("id").map_err(|_| ApiError::Internal)?;
            let name_fa: String = row.try_get("name_fa").map_err(|_| ApiError::Internal)?;
            let slug: String = row.try_get("slug").map_err(|_| ApiError::Internal)?;
            let is_launch_city: bool =
                row.try_get("is_launch_city").map_err(|_| ApiError::Internal)?;
            Ok(serde_json::json!({
                "id": id, "name_fa": name_fa, "slug": slug,
                "is_launch_city": is_launch_city,
            }))
        })
        .collect::<Result<_, ApiError>>()?;
    Ok(Json(serde_json::json!({ "data": data })))
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
    peer: PeerAddr,
    headers: HeaderMap,
    ApiJson(body): ApiJson<OtpRequestBody>,
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

    // Abuse limits (docs/PHASE5_PLAN.md): the client's window is charged first,
    // so scanning phone numbers from one address still throttles, then the
    // request must survive the per-phone rules, and only then may it spend a
    // slot from the global SMS budget.
    let ip = client_ip(
        state.config.trust_proxy,
        peer.0,
        &headers,
    );
    state
        .limiter
        .hit(
            &format!("otp:req:ip:{ip}"),
            state.config.otp_request_ip_limit,
            Duration::from_secs(u64::from(state.config.otp_request_ip_window_secs)),
        )
        .into_result()?;

    let validated =
        crate::otp_service::validate_request(&state.pool, &state.config, &phone, &purpose).await?;

    state
        .limiter
        .hit(
            "otp:sms:global",
            state.config.otp_sms_global_limit,
            Duration::from_secs(u64::from(state.config.otp_sms_global_window_secs)),
        )
        .into_result()?;

    let code = crate::otp::generate_code();
    let created =
        crate::otp_service::insert_challenge(&state.pool, &state.config, &validated, &code).await?;

    // Dev print (approved form), only in debug builds with OTP_DEV_MODE on:
    // the phone is masked to its last 4 digits, never logged in full.
    // ponytail: cfg!(debug_assertions) checks a compile-time fact at runtime;
    // the compile-time enforcement upgrade is a build-feature gate (e.g.
    // `#[cfg(feature = "dev-otp")]`) once a release profile exists.
    if state.config.otp_dev_mode && cfg!(debug_assertions) {
        let e164 = &validated.phone_e164;
        let masked = format!("***{}", &e164[e164.len() - 4..]);
        tracing::info!("DEV OTP for {masked}: {code}");
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
    peer: PeerAddr,
    headers: HeaderMap,
    ApiJson(body): ApiJson<OtpVerifyBody>,
) -> Result<(StatusCode, HeaderMap, Json<serde_json::Value>), ApiError> {
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

    // Guessing codes is a per-client activity: bound it before touching the DB.
    let ip = client_ip(
        state.config.trust_proxy,
        peer.0,
        &headers,
    );
    state
        .limiter
        .hit(
            &format!("otp:verify:ip:{ip}"),
            state.config.otp_verify_ip_limit,
            Duration::from_secs(u64::from(state.config.otp_verify_ip_window_secs)),
        )
        .into_result()?;

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
        two_set_cookies(cookie, csrf_cookie),
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
