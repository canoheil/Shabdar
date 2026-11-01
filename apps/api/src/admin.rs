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

#[derive(Clone, Copy)]
enum ListingQueue {
    Pending,
    LegacyImages,
}

async fn queue_page(
    pool: &MySqlPool,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
    kind: ListingQueue,
) -> Result<serde_json::Value, ApiError> {
    let limit = page_limit(limit_raw);
    let anchor: Option<(chrono::DateTime<Utc>, u64)> = cursor.map(decode_cursor).transpose()?;
    let (queue_time, visibility) = match kind {
        ListingQueue::Pending => (
            "l.submitted_at",
            "l.status = 'pending' AND l.deleted_at IS NULL",
        ),
        ListingQueue::LegacyImages => (
            "COALESCE(l.published_at, l.created_at)",
            "l.status = 'active' AND l.deleted_at IS NULL \
             AND l.image_reviewed_at IS NULL AND EXISTS(\
                 SELECT 1 FROM listing_images i WHERE i.listing_id = l.id)",
        ),
    };
    let mut sql = format!(
        "SELECT l.*, {queue_time} AS queue_at, c.name_fa AS city, cat.name_fa AS category, \
         u.public_id AS seller_public_id, u.display_name AS seller_name, \
         (SELECT storage_key FROM listing_images WHERE listing_id = l.id \
          ORDER BY sort_order, id LIMIT 1) AS cover, \
         (SELECT COUNT(*) FROM listing_images WHERE listing_id = l.id) AS image_count \
         FROM listings l JOIN cities c ON c.id = l.city_id \
         JOIN categories cat ON cat.id = l.category_id \
         JOIN users u ON u.id = l.seller_id WHERE {visibility}"
    );
    if anchor.is_some() {
        sql.push_str(&format!(" AND ({queue_time}, l.id) > (?, ?)"));
    }
    sql.push_str(&format!(" ORDER BY {queue_time} ASC, l.id ASC LIMIT ?"));
    let mut q = sqlx::query(&sql);
    if let Some((ts, id)) = anchor {
        q = q.bind(ts).bind(id);
    }
    q = q.bind((limit + 1) as i64);
    let rows = q.fetch_all(pool).await.map_err(|_| ApiError::Internal)?;
    let mut data = Vec::with_capacity(rows.len().min(limit));
    let mut last: Option<(chrono::DateTime<Utc>, u64)> = None;
    for row in rows.iter().take(limit) {
        let queue_at: Option<chrono::DateTime<Utc>> =
            row.try_get("queue_at").map_err(|_| ApiError::Internal)?;
        let submitted_at: Option<chrono::DateTime<Utc>> = row
            .try_get("submitted_at")
            .map_err(|_| ApiError::Internal)?;
        let published_at: Option<chrono::DateTime<Utc>> = row
            .try_get("published_at")
            .map_err(|_| ApiError::Internal)?;
        let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
        let cover: Option<String> = row.try_get("cover").map_err(|_| ApiError::Internal)?;
        last = queue_at.map(|ts| (ts, id));
        data.push(serde_json::json!({
            "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
            "slug": row.try_get::<String, _>("slug").map_err(|_| ApiError::Internal)?,
            "title": row.try_get::<String, _>("title").map_err(|_| ApiError::Internal)?,
            "price_toman": row.try_get::<Option<u64>, _>("price_toman").map_err(|_| ApiError::Internal)?,
            "price_type": row.try_get::<String, _>("price_type").map_err(|_| ApiError::Internal)?,
            "city": row.try_get::<String, _>("city").map_err(|_| ApiError::Internal)?,
            "category": row.try_get::<String, _>("category").map_err(|_| ApiError::Internal)?,
            "submitted_at": submitted_at.map(|ts| ts.to_rfc3339()),
            "published_at": published_at.map(|ts| ts.to_rfc3339()),
            "cover_image": cover.map(|key| format!("/media/{key}")),
            "image_count": row.try_get::<i64, _>("image_count").map_err(|_| ApiError::Internal)?,
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

/// GET /admin/listings/queue — `pending` oldest first, keyset on
/// (submitted_at, id). Page envelope matches the public feed.
pub async fn queue(
    pool: &MySqlPool,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    queue_page(pool, cursor, limit_raw, ListingQueue::Pending).await
}

/// GET /admin/listings/image-review-queue — active legacy image sets that
/// still need explicit human review, oldest publication first.
pub async fn image_review_queue(
    pool: &MySqlPool,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    queue_page(pool, cursor, limit_raw, ListingQueue::LegacyImages).await
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
         u.phone_e164 AS seller_phone, u.status AS seller_status, \
         reviewer.public_id AS image_reviewer_public_id, \
         reviewer.display_name AS image_reviewer_name, \
         CAST(l.image_reviewed_ids AS CHAR) AS image_reviewed_ids_text \
         FROM listings l JOIN cities c ON c.id = l.city_id \
         JOIN provinces p ON p.id = c.province_id \
         JOIN categories cat ON cat.id = l.category_id \
         JOIN users u ON u.id = l.seller_id \
         LEFT JOIN users reviewer ON reviewer.id = l.image_reviewed_by \
         WHERE l.public_id = ?",
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
    let images = sqlx::query(
        "SELECT id, storage_key, alt_text, sort_order, width, height \
         FROM listing_images WHERE listing_id = ? ORDER BY sort_order, id",
    )
    .bind(listing_id)
    .fetch_all(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    let images = images
        .iter()
        .map(|image| {
            let key: String = image
                .try_get("storage_key")
                .map_err(|_| ApiError::Internal)?;
            Ok(serde_json::json!({
                "id": image.try_get::<u64, _>("id").map_err(|_| ApiError::Internal)?,
                "url": format!("/media/{key}"),
                "alt_text": image.try_get::<Option<String>, _>("alt_text").map_err(|_| ApiError::Internal)?,
                "sort_order": image.try_get::<u8, _>("sort_order").map_err(|_| ApiError::Internal)?,
                "width": image.try_get::<Option<u16>, _>("width").map_err(|_| ApiError::Internal)?,
                "height": image.try_get::<Option<u16>, _>("height").map_err(|_| ApiError::Internal)?,
            }))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    listing["images"] = serde_json::json!(images);
    let reviewed_at: Option<chrono::DateTime<Utc>> = row
        .try_get("image_reviewed_at")
        .map_err(|_| ApiError::Internal)?;
    listing["image_review"] = if let Some(reviewed_at) = reviewed_at {
        let ids: String = row
            .try_get::<Option<String>, _>("image_reviewed_ids_text")
            .map_err(|_| ApiError::Internal)?
            .ok_or(ApiError::Internal)?;
        let ids: serde_json::Value = serde_json::from_str(&ids).map_err(|_| ApiError::Internal)?;
        serde_json::json!({
            "image_ids": ids,
            "reviewed_at": reviewed_at.to_rfc3339(),
            "reviewer": {
                "public_id": row.try_get::<Option<String>, _>("image_reviewer_public_id").map_err(|_| ApiError::Internal)?,
                "display_name": row.try_get::<Option<String>, _>("image_reviewer_name").map_err(|_| ApiError::Internal)?,
            },
        })
    } else {
        serde_json::Value::Null
    };
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

fn image_ids_validation(reason: &str) -> ApiError {
    let mut fields = BTreeMap::new();
    fields.insert("image_ids".to_string(), reason.to_string());
    ApiError::Validation { fields }
}

async fn current_images(pool: &MySqlPool, listing_id: u64) -> Result<Vec<(u64, String)>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, storage_key FROM listing_images \
         WHERE listing_id = ? ORDER BY sort_order, id",
    )
    .bind(listing_id)
    .fetch_all(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    rows.iter()
        .map(|row| {
            Ok((
                row.try_get("id").map_err(|_| ApiError::Internal)?,
                row.try_get("storage_key").map_err(|_| ApiError::Internal)?,
            ))
        })
        .collect()
}

fn exact_image_set(
    supplied: Option<&[u64]>,
    current: &[(u64, String)],
) -> Result<Vec<u64>, ApiError> {
    let supplied = supplied.ok_or_else(|| image_ids_validation("required"))?;
    let mut supplied = supplied.to_vec();
    supplied.sort_unstable();
    let canonical: Vec<u64> = current.iter().map(|(id, _)| *id).collect();
    let mut expected = canonical.clone();
    expected.sort_unstable();
    if supplied.is_empty() || supplied != expected {
        return Err(image_ids_validation("mismatch"));
    }
    Ok(canonical)
}

#[derive(Clone, Copy)]
enum ImageReviewMode {
    PublishPending,
    RestoreLegacy,
}

impl ImageReviewMode {
    fn audit_action(self) -> &'static str {
        match self {
            Self::PublishPending => "listing.approve",
            Self::RestoreLegacy => "listing.images.approve",
        }
    }

    fn check_row(&self, row: &sqlx::mysql::MySqlRow) -> Result<(), ApiError> {
        let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
        let reviewed_at: Option<chrono::DateTime<Utc>> = row
            .try_get("image_reviewed_at")
            .map_err(|_| ApiError::Internal)?;
        let valid = match self {
            Self::PublishPending => status == "pending",
            Self::RestoreLegacy => status == "active" && reviewed_at.is_none(),
        };
        if valid {
            Ok(())
        } else {
            Err(bad_transition())
        }
    }
}

async fn review_exact_images(
    pool: &MySqlPool,
    cfg: &crate::config::Config,
    admin: &CurrentUser,
    public_id: &str,
    supplied_ids: Option<&[u64]>,
    mode: ImageReviewMode,
) -> Result<u64, ApiError> {
    let row = load_for_review(pool, public_id).await?;
    mode.check_row(&row)?;
    let listing_id = row.try_get("id").map_err(|_| ApiError::Internal)?;
    let images = current_images(pool, listing_id).await?;
    let reviewed_ids = exact_image_set(supplied_ids, &images)?;
    let dimensions = crate::listing_images::validate_stored_images(cfg, &images).await?;

    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let locked = sqlx::query(
        "SELECT status, image_reviewed_at FROM listings \
         WHERE id = ? AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(listing_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    mode.check_row(&locked)?;
    let locked_rows = sqlx::query(
        "SELECT id, storage_key FROM listing_images \
         WHERE listing_id = ? ORDER BY sort_order, id FOR UPDATE",
    )
    .bind(listing_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    let locked_images = locked_rows
        .iter()
        .map(|row| {
            Ok((
                row.try_get("id").map_err(|_| ApiError::Internal)?,
                row.try_get("storage_key").map_err(|_| ApiError::Internal)?,
            ))
        })
        .collect::<Result<Vec<(u64, String)>, ApiError>>()?;
    if locked_images != images {
        return Err(image_ids_validation("mismatch"));
    }
    for (image_id, width, height) in dimensions {
        sqlx::query(
            "UPDATE listing_images SET width = ?, height = ? WHERE id = ? AND listing_id = ?",
        )
        .bind(width)
        .bind(height)
        .bind(image_id)
        .bind(listing_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
    }
    let reviewed_ids_json = serde_json::to_string(&reviewed_ids).map_err(|_| ApiError::Internal)?;
    let now = Utc::now();
    let changed = match mode {
        ImageReviewMode::PublishPending => sqlx::query(
            "UPDATE listings SET status = 'active', published_at = ?, reviewed_by = ?, \
             reviewed_at = ?, image_reviewed_by = ?, image_reviewed_at = ?, \
             image_reviewed_ids = ?, updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(admin.user_id)
        .bind(now)
        .bind(admin.user_id)
        .bind(now)
        .bind(&reviewed_ids_json)
        .bind(now)
        .bind(listing_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?,
        ImageReviewMode::RestoreLegacy => sqlx::query(
            "UPDATE listings SET image_reviewed_by = ?, image_reviewed_at = ?, \
             image_reviewed_ids = ? WHERE id = ?",
        )
        .bind(admin.user_id)
        .bind(now)
        .bind(&reviewed_ids_json)
        .bind(listing_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?,
    };
    if changed.rows_affected() != 1 {
        return Err(ApiError::Internal);
    }
    let metadata = serde_json::json!({ "image_ids": reviewed_ids });
    crate::admin_users::audit_in_tx(
        &mut tx,
        admin.user_id,
        mode.audit_action(),
        "listing",
        listing_id,
        None,
        Some(&metadata),
    )
    .await?;
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    Ok(listing_id)
}

/// POST /admin/listings/{public_id}/approve — pending → active + audit.
pub async fn approve(
    pool: &MySqlPool,
    cfg: &crate::config::Config,
    admin: &CurrentUser,
    public_id: &str,
    image_ids: Option<&[u64]>,
) -> Result<ListingOut, ApiError> {
    let id = review_exact_images(
        pool,
        cfg,
        admin,
        public_id,
        image_ids,
        ImageReviewMode::PublishPending,
    )
    .await?;
    let row = sqlx::query("SELECT * FROM listings WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    listing_out(&row)
}

/// POST /admin/listings/{public_id}/images/review — restore a legacy active
/// image set without changing its publication or original listing decision.
pub async fn review_legacy_images(
    pool: &MySqlPool,
    cfg: &crate::config::Config,
    admin: &CurrentUser,
    public_id: &str,
    image_ids: Option<&[u64]>,
) -> Result<serde_json::Value, ApiError> {
    review_exact_images(
        pool,
        cfg,
        admin,
        public_id,
        image_ids,
        ImageReviewMode::RestoreLegacy,
    )
    .await?;
    admin_detail(pool, public_id).await
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
