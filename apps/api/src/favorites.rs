//! Current user's favorites (Phase 6).
//!
//! Every mutation locks its listing row and changes the favorite row and the
//! denormalized counter in one transaction. The lock serializes all writers
//! for a listing; no counter clamp can hide an invariant violation.

use chrono::Utc;
use sqlx::{MySqlPool, Row};

use crate::error::ApiError;

/// Save an active listing for the current user. Duplicate saves leave the
/// original row (including created_at) and counter untouched.
pub async fn save(
    pool: &MySqlPool,
    user_id: u64,
    public_id: &str,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let row = sqlx::query(
        "SELECT id, status, deleted_at FROM listings WHERE public_id = ? FOR UPDATE",
    )
    .bind(public_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    let listing_id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    let deleted_at: Option<chrono::DateTime<Utc>> =
        row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
    if status != "active" || deleted_at.is_some() {
        return Err(ApiError::NotFound);
    }

    let inserted = sqlx::query(
        "INSERT IGNORE INTO favorites (user_id, listing_id, created_at) VALUES (?, ?, ?)",
    )
    .bind(user_id)
    .bind(listing_id)
    .bind(Utc::now())
    .execute(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    if inserted.rows_affected() == 1 {
        let changed = sqlx::query(
            "UPDATE listings SET favorite_count = favorite_count + 1 WHERE id = ?",
        )
        .bind(listing_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
        if changed.rows_affected() != 1 {
            return Err(ApiError::Internal);
        }
    }
    tx.commit().await.map_err(|_| ApiError::Internal)
}

/// Remove the current user's favorite from any known listing, including one
/// that is no longer public. Repeating a delete never decrements twice.
pub async fn unsave(
    pool: &MySqlPool,
    user_id: u64,
    public_id: &str,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let listing_id: u64 = sqlx::query_scalar(
        "SELECT id FROM listings WHERE public_id = ? FOR UPDATE",
    )
    .bind(public_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;

    let deleted = sqlx::query("DELETE FROM favorites WHERE user_id = ? AND listing_id = ?")
        .bind(user_id)
        .bind(listing_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
    if deleted.rows_affected() == 1 {
        let changed = sqlx::query(
            "UPDATE listings SET favorite_count = favorite_count - 1 WHERE id = ?",
        )
        .bind(listing_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
        if changed.rows_affected() != 1 {
            return Err(ApiError::Internal);
        }
    }
    tx.commit().await.map_err(|_| ApiError::Internal)
}
