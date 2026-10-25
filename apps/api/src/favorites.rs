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
pub async fn save(pool: &MySqlPool, user_id: u64, public_id: &str) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let row =
        sqlx::query("SELECT id, status, deleted_at FROM listings WHERE public_id = ? FOR UPDATE")
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
        let changed =
            sqlx::query("UPDATE listings SET favorite_count = favorite_count + 1 WHERE id = ?")
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
pub async fn unsave(pool: &MySqlPool, user_id: u64, public_id: &str) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let listing_id: u64 =
        sqlx::query_scalar("SELECT id FROM listings WHERE public_id = ? FOR UPDATE")
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
        let changed =
            sqlx::query("UPDATE listings SET favorite_count = favorite_count - 1 WHERE id = ?")
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

/// Return the viewer's active saved cards using the same shape and keyset
/// conventions as the public feed. Cursor values are bound directly, so an
/// anchor remains valid even after its favorite row is removed.
pub async fn list(
    pool: &MySqlPool,
    user_id: u64,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let limit = crate::listings::page_limit(limit_raw);
    let anchor = cursor.map(crate::listings::decode_cursor).transpose()?;
    let mut sql = String::from(
        "SELECT l.id, l.public_id, l.slug, l.title, l.price_toman, l.price_type, \
         l.rating_sum, l.rating_count, l.published_at, c.name_fa AS city, \
         p.name_fa AS province, cat.name_fa AS category, \
         (SELECT storage_key FROM listing_images WHERE listing_id = l.id \
          ORDER BY sort_order LIMIT 1) AS cover, \
         f.created_at AS saved_at, f.listing_id AS favorite_listing_id \
         FROM favorites f JOIN listings l ON l.id = f.listing_id \
         JOIN cities c ON c.id = l.city_id \
         JOIN provinces p ON p.id = c.province_id \
         JOIN categories cat ON cat.id = l.category_id \
         WHERE f.user_id = ? AND l.status = 'active' AND l.deleted_at IS NULL",
    );
    if anchor.is_some() {
        sql.push_str(" AND (f.created_at, f.listing_id) < (?, ?)");
    }
    sql.push_str(" ORDER BY f.created_at DESC, f.listing_id DESC LIMIT ?");

    let mut query = sqlx::query(&sql).bind(user_id);
    if let Some((saved_at, listing_id)) = anchor {
        query = query.bind(saved_at).bind(listing_id);
    }
    query = query.bind((limit + 1) as i64);
    let mut rows = query
        .fetch_all(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    let has_more = rows.len() > limit;
    rows.truncate(limit);

    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                let saved_at: chrono::DateTime<Utc> =
                    row.try_get("saved_at").map_err(|_| ApiError::Internal)?;
                let listing_id: u64 = row
                    .try_get("favorite_listing_id")
                    .map_err(|_| ApiError::Internal)?;
                Ok(crate::listings::encode_cursor(&saved_at, listing_id))
            })
            .transpose()?
    } else {
        None
    };
    let data = rows
        .iter()
        .map(crate::listings::card_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({
        "data": data,
        "page": { "next_cursor": next_cursor, "has_more": has_more },
    }))
}
