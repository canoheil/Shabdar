//! Immediate public listing comments (Phase 8).
//!
//! Creation and deletion hold the listing row lock while changing the visible
//! counter. Pagination binds cursor values directly, so deleting an anchor
//! never invalidates the next page.

use sqlx::{MySqlConnection, MySqlPool, Row, Transaction};

use crate::error::ApiError;

fn validation(reason: &str) -> ApiError {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("body".to_string(), reason.to_string());
    ApiError::Validation { fields }
}

/// Trim outer whitespace, preserve internal newlines, and count Unicode
/// scalar characters rather than UTF-8 bytes.
pub fn validate_body(value: Option<&str>) -> Result<String, ApiError> {
    let trimmed = value.unwrap_or_default().trim();
    if trimmed.is_empty() {
        return Err(validation("required"));
    }
    if trimmed.chars().count() > 1000 {
        return Err(validation("too_long"));
    }
    Ok(trimmed.to_string())
}

fn check_visibility(status: &str, deleted: bool) -> Result<(), ApiError> {
    if status == "removed" || deleted {
        return Err(ApiError::Gone);
    }
    if status != "active" {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

struct CreatedListing {
    id: u64,
    comment_count: u32,
}

async fn lock_listing_for_create(
    connection: &mut MySqlConnection,
    slug: &str,
) -> Result<CreatedListing, ApiError> {
    let row = sqlx::query(
        "SELECT id, status, deleted_at, comment_count FROM listings WHERE slug = ? FOR UPDATE",
    )
    .bind(slug)
    .fetch_optional(&mut *connection)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
    check_visibility(&status, deleted_at.is_some())?;
    Ok(CreatedListing {
        id: row.try_get("id").map_err(|_| ApiError::Internal)?,
        comment_count: row
            .try_get("comment_count")
            .map_err(|_| ApiError::Internal)?,
    })
}

async fn write_visible_count(
    tx: &mut Transaction<'_, sqlx::MySql>,
    listing_id: u64,
    count: u32,
) -> Result<(), ApiError> {
    let changed = sqlx::query("UPDATE listings SET comment_count = ? WHERE id = ?")
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

fn comment_from_row(row: &sqlx::mysql::MySqlRow) -> Result<serde_json::Value, ApiError> {
    let created_at: chrono::DateTime<chrono::Utc> =
        row.try_get("created_at").map_err(|_| ApiError::Internal)?;
    Ok(serde_json::json!({
        "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
        "body": row.try_get::<String, _>("body").map_err(|_| ApiError::Internal)?,
        "author": {
            "public_id": row.try_get::<String, _>("author_public_id").map_err(|_| ApiError::Internal)?,
            "display_name": row.try_get::<Option<String>, _>("author_name").map_err(|_| ApiError::Internal)?,
        },
        "created_at": created_at.to_rfc3339(),
    }))
}

#[derive(Debug)]
pub enum CreateResult {
    Visible(serde_json::Value),
    Pending(serde_json::Value),
}

/// Create one clean public comment or one hidden pending-review case. Active
/// rules are read once for this attempt; the listing lock still serializes
/// every counter mutation, and automatic filtering writes no audit or report.
pub async fn create(
    pool: &MySqlPool,
    user_id: u64,
    slug: &str,
    body: &str,
) -> Result<CreateResult, ApiError> {
    let rules = crate::moderation_rules::active_rules(pool).await?;
    let matches = crate::moderation::matching_rules(body, &rules);
    let pending = !matches.is_empty();
    let matched_rules = if pending {
        let snapshots = matches
            .iter()
            .map(|rule| {
                serde_json::json!({
                    "public_id": &rule.public_id,
                    "revision": rule.revision,
                    "phrase": &rule.phrase,
                    "match_mode": rule.match_mode.as_str(),
                })
            })
            .collect::<Vec<_>>();
        Some(serde_json::to_string(&snapshots).map_err(|_| ApiError::Internal)?)
    } else {
        None
    };

    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let listing = lock_listing_for_create(&mut tx, slug).await?;
    let public_id = ulid::Ulid::new().to_string();
    let inserted = sqlx::query(
        "INSERT INTO listing_comments \
         (public_id, listing_id, author_id, body, status, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, NOW(3), NOW(3))",
    )
    .bind(&public_id)
    .bind(listing.id)
    .bind(user_id)
    .bind(body)
    .bind(if pending { "hidden" } else { "visible" })
    .execute(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    if inserted.rows_affected() != 1 {
        return Err(ApiError::Internal);
    }
    let comment_id = inserted.last_insert_id();
    if let Some(matched_rules) = matched_rules {
        let case = sqlx::query(
            "INSERT INTO comment_moderation_cases \
             (comment_id, status, matched_rules, flagged_at, updated_at) \
             VALUES (?, 'pending', ?, NOW(3), NOW(3))",
        )
        .bind(comment_id)
        .bind(matched_rules)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
        if case.rows_affected() != 1 {
            return Err(ApiError::Internal);
        }
    } else {
        let count = listing
            .comment_count
            .checked_add(1)
            .ok_or(ApiError::Internal)?;
        write_visible_count(&mut tx, listing.id, count).await?;
    }

    let row = sqlx::query(
        "SELECT c.public_id, c.body, c.created_at, \
         u.public_id AS author_public_id, u.display_name AS author_name \
         FROM listing_comments c JOIN users u ON u.id = c.author_id WHERE c.id = ?",
    )
    .bind(comment_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    let result = if pending {
        let created_at: chrono::DateTime<chrono::Utc> = row
            .try_get("created_at")
            .map_err(|_| ApiError::Internal)?;
        CreateResult::Pending(serde_json::json!({
            "public_id": public_id,
            "status": "pending_review",
            "created_at": created_at.to_rfc3339(),
        }))
    } else {
        CreateResult::Visible(comment_from_row(&row)?)
    };
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    Ok(result)
}

/// Newest-first keyset page. Only visible, non-deleted rows appear. For a
/// valid viewer, outgoing block rows hide only that viewer's matching authors;
/// anonymous and stale sessions retain the original public page.
pub async fn list(
    pool: &MySqlPool,
    slug: &str,
    viewer_user_id: Option<u64>,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let listing = sqlx::query("SELECT id, status, deleted_at FROM listings WHERE slug = ?")
        .bind(slug)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    let status: String = listing.try_get("status").map_err(|_| ApiError::Internal)?;
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> = listing
        .try_get("deleted_at")
        .map_err(|_| ApiError::Internal)?;
    check_visibility(&status, deleted_at.is_some())?;
    let listing_id: u64 = listing.try_get("id").map_err(|_| ApiError::Internal)?;

    // Resolve the target before validating a cursor: unknown/non-public
    // listings have one consistent response and do not become existence oracles.
    let anchor = cursor.map(crate::listings::decode_cursor).transpose()?;
    let limit = crate::listings::page_limit(limit_raw);
    let mut sql = String::from(
        "SELECT c.id, c.public_id, c.body, c.created_at, \
         u.public_id AS author_public_id, u.display_name AS author_name \
         FROM listing_comments c JOIN users u ON u.id = c.author_id \
         WHERE c.listing_id = ? AND c.status = 'visible' AND c.deleted_at IS NULL",
    );
    if viewer_user_id.is_some() {
        sql.push_str(
            " AND NOT EXISTS (SELECT 1 FROM blocks b \
             WHERE b.blocker_id = ? AND b.blocked_id = c.author_id)",
        );
    }
    if anchor.is_some() {
        sql.push_str(" AND (c.created_at, c.id) < (?, ?)");
    }
    sql.push_str(" ORDER BY c.created_at DESC, c.id DESC LIMIT ?");
    let mut query = sqlx::query(&sql).bind(listing_id);
    if let Some(viewer_user_id) = viewer_user_id {
        query = query.bind(viewer_user_id);
    }
    if let Some((created_at, id)) = anchor {
        query = query.bind(created_at).bind(id);
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
                let created_at: chrono::DateTime<chrono::Utc> =
                    row.try_get("created_at").map_err(|_| ApiError::Internal)?;
                let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
                Ok(crate::listings::encode_cursor(&created_at, id))
            })
            .transpose()?
    } else {
        None
    };
    let data = rows
        .iter()
        .map(comment_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({
        "data": data,
        "page": { "next_cursor": next_cursor, "has_more": has_more },
    }))
}

/// Author-only soft delete. Repeating it never decrements twice; status and
/// updated_at retain their meanings for the future moderation phase.
pub async fn delete(
    pool: &MySqlPool,
    user_id: u64,
    public_id: &str,
) -> Result<serde_json::Value, ApiError> {
    let comment =
        sqlx::query("SELECT id, listing_id, author_id FROM listing_comments WHERE public_id = ?")
            .bind(public_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| ApiError::Internal)?
            .ok_or(ApiError::NotFound)?;
    let comment_id: u64 = comment.try_get("id").map_err(|_| ApiError::Internal)?;
    let listing_id: u64 = comment
        .try_get("listing_id")
        .map_err(|_| ApiError::Internal)?;
    let author_id: u64 = comment
        .try_get("author_id")
        .map_err(|_| ApiError::Internal)?;
    if author_id != user_id {
        return Err(ApiError::Forbidden);
    }

    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let count: u32 =
        sqlx::query_scalar("SELECT comment_count FROM listings WHERE id = ? FOR UPDATE")
            .bind(listing_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| ApiError::Internal)?
            .ok_or(ApiError::NotFound)?;
    let current = sqlx::query(
        "SELECT status, deleted_at FROM listing_comments WHERE id = ? AND listing_id = ? FOR UPDATE",
    )
    .bind(comment_id)
    .bind(listing_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> = current
        .try_get("deleted_at")
        .map_err(|_| ApiError::Internal)?;

    if deleted_at.is_none() {
        let status: String = current.try_get("status").map_err(|_| ApiError::Internal)?;
        let updated = sqlx::query(
            "UPDATE listing_comments SET deleted_at = NOW(3) WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(comment_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
        if updated.rows_affected() != 1 {
            return Err(ApiError::Internal);
        }
        if status == "visible" {
            let count = count.checked_sub(1).ok_or(ApiError::Internal)?;
            write_visible_count(&mut tx, listing_id, count).await?;
        }
    }
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    Ok(serde_json::json!({ "ok": true }))
}
