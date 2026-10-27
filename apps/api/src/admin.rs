//! Admin listing moderation: pending queue, approve/reject/remove, each
//! writing exactly one audit row in the same transaction (Phase 3 Task 6).
//!
//! Every entry point takes the authenticated `CurrentUser` and rejects
//! non-admins with `Forbidden` before touching anything.

use std::collections::BTreeMap;

use chrono::Utc;
use sqlx::{MySqlPool, Row};

use crate::error::ApiError;
use crate::listings::{decode_cursor, encode_cursor, listing_out, page_limit, ListingOut};
use crate::session::CurrentUser;

/// Gate every admin handler through this first: non-admins learn nothing
/// beyond the generic 403 (no existence signal either way).
pub fn require_admin(user: &CurrentUser) -> Result<(), ApiError> {
    if user.is_admin {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

fn required_reason(raw: &Option<String>) -> Result<String, ApiError> {
    match raw {
        Some(reason) if !reason.trim().is_empty() => {
            let trimmed = reason.trim().to_string();
            if trimmed.chars().count() > 500 {
                let mut fields = BTreeMap::new();
                fields.insert("reason".to_string(), "too_long".to_string());
                return Err(ApiError::Validation { fields });
            }
            Ok(trimmed)
        }
        _ => {
            let mut fields = BTreeMap::new();
            fields.insert("reason".to_string(), "required".to_string());
            Err(ApiError::Validation { fields })
        }
    }
}

async fn audit(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    admin_id: u64,
    action: &str,
    target_id: u64,
    reason: Option<&str>,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO admin_audit_logs (admin_id, action, target_type, target_id, \
         reason, created_at) VALUES (?, ?, 'listing', ?, ?, ?)",
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_id)
    .bind(reason)
    .bind(Utc::now())
    .execute(&mut **tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    Ok(())
}

/// GET /admin/listings/queue — `pending` oldest first, keyset on
/// (submitted_at, id). Page envelope matches the public feed.
pub async fn queue(
    pool: &MySqlPool,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let limit = page_limit(limit_raw);
    let anchor: Option<(chrono::DateTime<Utc>, u64)> = cursor.map(decode_cursor).transpose()?;
    let mut sql = "SELECT l.*, c.name_fa AS city, cat.name_fa AS category, \
         u.public_id AS seller_public_id, u.display_name AS seller_name \
         FROM listings l JOIN cities c ON c.id = l.city_id \
         JOIN categories cat ON cat.id = l.category_id \
         JOIN users u ON u.id = l.seller_id \
         WHERE l.status = 'pending' AND l.deleted_at IS NULL"
        .to_string();
    if anchor.is_some() {
        sql.push_str(" AND (l.submitted_at, l.id) > (?, ?)");
    }
    sql.push_str(" ORDER BY l.submitted_at ASC, l.id ASC LIMIT ?");
    let mut q = sqlx::query(&sql);
    if let Some((ts, id)) = anchor {
        q = q.bind(ts).bind(id);
    }
    q = q.bind((limit + 1) as i64);
    let rows = q.fetch_all(pool).await.map_err(|_| ApiError::Internal)?;
    let mut data = Vec::with_capacity(rows.len().min(limit));
    let mut last: Option<(chrono::DateTime<Utc>, u64)> = None;
    for row in rows.iter().take(limit) {
        let submitted_at: Option<chrono::DateTime<Utc>> = row
            .try_get("submitted_at")
            .map_err(|_| ApiError::Internal)?;
        let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
        last = submitted_at.map(|ts| (ts, id));
        data.push(serde_json::json!({
            "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
            "slug": row.try_get::<String, _>("slug").map_err(|_| ApiError::Internal)?,
            "title": row.try_get::<String, _>("title").map_err(|_| ApiError::Internal)?,
            "price_toman": row.try_get::<Option<u64>, _>("price_toman").map_err(|_| ApiError::Internal)?,
            "price_type": row.try_get::<String, _>("price_type").map_err(|_| ApiError::Internal)?,
            "city": row.try_get::<String, _>("city").map_err(|_| ApiError::Internal)?,
            "category": row.try_get::<String, _>("category").map_err(|_| ApiError::Internal)?,
            "submitted_at": submitted_at.map(|ts| ts.to_rfc3339()),
            "seller": {
                "public_id": row.try_get::<String, _>("seller_public_id").map_err(|_| ApiError::Internal)?,
                "display_name": row.try_get::<Option<String>, _>("seller_name").map_err(|_| ApiError::Internal)?,
            },
        }));
    }
    let has_more = rows.len() > limit;
    let next_cursor = if has_more {
        last.map(|(ts, id)| encode_cursor(&ts, id))
    } else {
        None
    };
    Ok(serde_json::json!({
        "data": data,
        "page": { "next_cursor": next_cursor, "has_more": has_more },
    }))
}

/// GET /admin/listings/{public_id} — full listing with seller info (phone
/// included: admin eyes only) and report history (empty until the reports
/// phase owns that table).
pub async fn admin_detail(
    pool: &MySqlPool,
    public_id: &str,
) -> Result<serde_json::Value, ApiError> {
    let row = sqlx::query(
        "SELECT l.*, c.name_fa AS city, p.name_fa AS province, cat.name_fa AS category, \
         u.public_id AS seller_public_id, u.display_name AS seller_name, \
         u.phone_e164 AS seller_phone, u.status AS seller_status \
         FROM listings l JOIN cities c ON c.id = l.city_id \
         JOIN provinces p ON p.id = c.province_id \
         JOIN categories cat ON cat.id = l.category_id \
         JOIN users u ON u.id = l.seller_id WHERE l.public_id = ?",
    )
    .bind(public_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    let mut listing = serde_json::to_value(listing_out(&row)?).map_err(|_| ApiError::Internal)?;
    listing["city"] = serde_json::json!(row
        .try_get::<String, _>("city")
        .map_err(|_| ApiError::Internal)?);
    listing["province"] = serde_json::json!(row
        .try_get::<String, _>("province")
        .map_err(|_| ApiError::Internal)?);
    listing["category"] = serde_json::json!(row
        .try_get::<String, _>("category")
        .map_err(|_| ApiError::Internal)?);
    listing["seller"] = serde_json::json!({
        "public_id": row.try_get::<String, _>("seller_public_id").map_err(|_| ApiError::Internal)?,
        "display_name": row.try_get::<Option<String>, _>("seller_name").map_err(|_| ApiError::Internal)?,
        "phone_e164": row.try_get::<String, _>("seller_phone").map_err(|_| ApiError::Internal)?,
        "status": row.try_get::<String, _>("seller_status").map_err(|_| ApiError::Internal)?,
    });
    let listing_id = row.try_get("id").map_err(|_| ApiError::Internal)?;
    listing["report_history"] =
        serde_json::json!(crate::admin_reports::history_for_listing(pool, listing_id).await?);
    Ok(listing)
}

async fn load_for_review(
    pool: &MySqlPool,
    public_id: &str,
) -> Result<sqlx::mysql::MySqlRow, ApiError> {
    sqlx::query("SELECT * FROM listings WHERE public_id = ? AND deleted_at IS NULL")
        .bind(public_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)
}

fn bad_transition() -> ApiError {
    let mut fields = BTreeMap::new();
    fields.insert("status".to_string(), "invalid_transition".to_string());
    ApiError::Validation { fields }
}

/// POST /admin/listings/{public_id}/approve — pending → active + audit.
pub async fn approve(
    pool: &MySqlPool,
    admin: &CurrentUser,
    public_id: &str,
) -> Result<ListingOut, ApiError> {
    let row = load_for_review(pool, public_id).await?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    if status != "pending" {
        return Err(bad_transition());
    }
    let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
    let now = Utc::now();
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    sqlx::query(
        "UPDATE listings SET status = 'active', published_at = ?, reviewed_by = ?, \
         reviewed_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(now)
    .bind(admin.user_id)
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    audit(&mut tx, admin.user_id, "listing.approve", id, None).await?;
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    let row = sqlx::query("SELECT * FROM listings WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    listing_out(&row)
}

/// POST /admin/listings/{public_id}/reject — pending → rejected + audit.
/// Reason required (1–500 chars); the seller reads it on their dashboard.
pub async fn reject(
    pool: &MySqlPool,
    admin: &CurrentUser,
    public_id: &str,
    reason_raw: &Option<String>,
) -> Result<ListingOut, ApiError> {
    let reason = required_reason(reason_raw)?;
    let row = load_for_review(pool, public_id).await?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    if status != "pending" {
        return Err(bad_transition());
    }
    let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
    let now = Utc::now();
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    sqlx::query(
        "UPDATE listings SET status = 'rejected', rejection_reason = ?, reviewed_by = ?, \
         reviewed_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&reason)
    .bind(admin.user_id)
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    audit(&mut tx, admin.user_id, "listing.reject", id, Some(&reason)).await?;
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    let row = sqlx::query("SELECT * FROM listings WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    listing_out(&row)
}

/// Lock and apply active → removed without writing an audit. The direct admin
/// route and report decisions each add their own single audit outside this
/// helper. An already-removed listing returns false for the caller's own
/// transition/idempotency rules.
pub async fn remove_by_id_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    admin_id: u64,
    id: u64,
    reason: &str,
) -> Result<bool, ApiError> {
    let row = sqlx::query("SELECT status, deleted_at FROM listings WHERE id = ? FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    let deleted_at: Option<chrono::DateTime<Utc>> =
        row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
    if deleted_at.is_some() {
        return Err(ApiError::NotFound);
    }
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    if status == "removed" {
        return Ok(false);
    }
    if status != "active" {
        return Err(bad_transition());
    }
    let now = Utc::now();
    let updated = sqlx::query(
        "UPDATE listings SET status = 'removed', removed_reason = ?, reviewed_by = ?, \
         reviewed_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(reason)
    .bind(admin_id)
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(&mut **tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::Internal);
    }
    Ok(true)
}

/// POST /admin/listings/{public_id}/remove — active → removed + audit.
/// Public reads go 410 afterwards.
pub async fn remove(
    pool: &MySqlPool,
    admin: &CurrentUser,
    public_id: &str,
    reason_raw: &Option<String>,
) -> Result<ListingOut, ApiError> {
    let reason = required_reason(reason_raw)?;
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let id: u64 = sqlx::query_scalar(
        "SELECT id FROM listings WHERE public_id = ? AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(public_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    let changed = remove_by_id_in_tx(&mut tx, admin.user_id, id, &reason).await?;
    if !changed {
        return Err(bad_transition());
    }
    audit(&mut tx, admin.user_id, "listing.remove", id, Some(&reason)).await?;
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    let row = sqlx::query("SELECT * FROM listings WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    listing_out(&row)
}
