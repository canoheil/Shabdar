//! Listing ratings (Phase 8).
//!
//! The listing row lock serializes every rating writer for one listing. The
//! rating row and its exact denormalized counters commit together; repeated
//! same-star writes and absent deletes do no writes at all.

use sqlx::{MySqlConnection, MySqlPool, Row, Transaction};

use crate::error::ApiError;

struct LockedListing {
    id: u64,
    seller_id: u64,
    rating_sum: u32,
    rating_count: u32,
}

fn validation(reason: &str) -> ApiError {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("stars".to_string(), reason.to_string());
    ApiError::Validation { fields }
}

/// Validate a JSON integer rather than letting serde coerce floats/strings.
pub fn validate_stars(value: Option<&serde_json::Value>) -> Result<u8, ApiError> {
    let stars = match value {
        None | Some(serde_json::Value::Null) => return Err(validation("required")),
        Some(serde_json::Value::Number(number)) if number.is_u64() => {
            number.as_u64().and_then(|n| u8::try_from(n).ok())
        }
        Some(_) => None,
    };
    match stars {
        Some(stars @ 1..=5) => Ok(stars),
        _ => Err(validation("out_of_range")),
    }
}

/// Lock and classify a slug under the public listing visibility contract.
async fn lock_listing(
    connection: &mut MySqlConnection,
    slug: &str,
) -> Result<LockedListing, ApiError> {
    let row = sqlx::query(
        "SELECT l.id, l.seller_id, l.status, l.deleted_at, l.rating_sum, l.rating_count, \
         l.image_reviewed_at IS NOT NULL AS image_reviewed, \
         EXISTS(SELECT 1 FROM listing_images i WHERE i.listing_id = l.id) AS has_images \
         FROM listings l WHERE l.slug = ? FOR UPDATE",
    )
    .bind(slug)
    .fetch_optional(&mut *connection)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
    if status == "removed" || deleted_at.is_some() {
        return Err(ApiError::Gone);
    }
    if status != "active" {
        return Err(ApiError::NotFound);
    }
    crate::listings::check_public_image_review(&row)?;
    Ok(LockedListing {
        id: row.try_get("id").map_err(|_| ApiError::Internal)?,
        seller_id: row.try_get("seller_id").map_err(|_| ApiError::Internal)?,
        rating_sum: row.try_get("rating_sum").map_err(|_| ApiError::Internal)?,
        rating_count: row
            .try_get("rating_count")
            .map_err(|_| ApiError::Internal)?,
    })
}

async fn current_stars(
    tx: &mut Transaction<'_, sqlx::MySql>,
    listing_id: u64,
    user_id: u64,
) -> Result<Option<u8>, ApiError> {
    sqlx::query_scalar("SELECT stars FROM listing_ratings WHERE listing_id = ? AND user_id = ?")
        .bind(listing_id)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| ApiError::Internal)
}

async fn write_counters(
    tx: &mut Transaction<'_, sqlx::MySql>,
    listing_id: u64,
    sum: u32,
    count: u32,
) -> Result<(), ApiError> {
    let changed = sqlx::query("UPDATE listings SET rating_sum = ?, rating_count = ? WHERE id = ?")
        .bind(sum)
        .bind(count)
        .bind(listing_id)
        .execute(&mut **tx)
        .await
        .map_err(|_| ApiError::Internal)?;
    if changed.rows_affected() != 1 {
        return Err(ApiError::Internal);
    }
    Ok(())
}

fn response(sum: u32, count: u32, my_stars: Option<u8>) -> serde_json::Value {
    let average = if count == 0 {
        serde_json::json!(0)
    } else {
        serde_json::json!(sum as f64 / count as f64)
    };
    serde_json::json!({
        "rating_average": average,
        "rating_count": count,
        "my_stars": my_stars,
    })
}

/// Create or update the current user's rating under one listing transaction.
pub async fn put(
    pool: &MySqlPool,
    user_id: u64,
    slug: &str,
    stars: u8,
) -> Result<serde_json::Value, ApiError> {
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let listing = lock_listing(&mut tx, slug).await?;
    if listing.seller_id == user_id {
        return Err(ApiError::Forbidden);
    }
    let previous = current_stars(&mut tx, listing.id, user_id).await?;
    let mut sum = listing.rating_sum;
    let mut count = listing.rating_count;

    match previous {
        None => {
            let inserted = sqlx::query(
                "INSERT INTO listing_ratings (listing_id, user_id, stars, created_at, updated_at) \
                 VALUES (?, ?, ?, NOW(3), NOW(3))",
            )
            .bind(listing.id)
            .bind(user_id)
            .bind(stars)
            .execute(&mut *tx)
            .await
            .map_err(|_| ApiError::Internal)?;
            if inserted.rows_affected() != 1 {
                return Err(ApiError::Internal);
            }
            sum = sum
                .checked_add(u32::from(stars))
                .ok_or(ApiError::Internal)?;
            count = count.checked_add(1).ok_or(ApiError::Internal)?;
            write_counters(&mut tx, listing.id, sum, count).await?;
        }
        Some(old) if old != stars => {
            let updated = sqlx::query(
                "UPDATE listing_ratings SET stars = ?, updated_at = NOW(3) \
                 WHERE listing_id = ? AND user_id = ?",
            )
            .bind(stars)
            .bind(listing.id)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| ApiError::Internal)?;
            if updated.rows_affected() != 1 {
                return Err(ApiError::Internal);
            }
            sum = sum
                .checked_sub(u32::from(old))
                .and_then(|value| value.checked_add(u32::from(stars)))
                .ok_or(ApiError::Internal)?;
            write_counters(&mut tx, listing.id, sum, count).await?;
        }
        Some(_) => {
            // Same stars: preserve both timestamps and the listing counters.
        }
    }
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    Ok(response(sum, count, Some(stars)))
}

/// Delete the current user's rating exactly once. An absent row is a no-op.
pub async fn delete(
    pool: &MySqlPool,
    user_id: u64,
    slug: &str,
) -> Result<serde_json::Value, ApiError> {
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let listing = lock_listing(&mut tx, slug).await?;
    let previous = current_stars(&mut tx, listing.id, user_id).await?;
    let mut sum = listing.rating_sum;
    let mut count = listing.rating_count;
    if let Some(old) = previous {
        let deleted =
            sqlx::query("DELETE FROM listing_ratings WHERE listing_id = ? AND user_id = ?")
                .bind(listing.id)
                .bind(user_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| ApiError::Internal)?;
        if deleted.rows_affected() != 1 {
            return Err(ApiError::Internal);
        }
        sum = sum.checked_sub(u32::from(old)).ok_or(ApiError::Internal)?;
        count = count.checked_sub(1).ok_or(ApiError::Internal)?;
        write_counters(&mut tx, listing.id, sum, count).await?;
    }
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    Ok(response(sum, count, None))
}
