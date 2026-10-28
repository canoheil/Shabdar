//! Private user blocks (Phase 9).
//!
//! A block is one directed `(blocker, blocked)` row. It has no global target
//! effect: only the blocker's optional-viewer comment query reads these rows.

use chrono::Utc;
use sqlx::{MySqlPool, Row};

use crate::error::ApiError;

fn self_block() -> ApiError {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("public_id".to_string(), "self_block".to_string());
    ApiError::Validation { fields }
}

/// Idempotently block another existing, non-deleted user. Suspended users
/// remain valid targets; duplicate writes retain the original timestamp.
pub async fn put(pool: &MySqlPool, user_id: u64, public_id: &str) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let target =
        sqlx::query("SELECT id, status, deleted_at FROM users WHERE public_id = ? FOR UPDATE")
            .bind(public_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| ApiError::Internal)?
            .ok_or(ApiError::NotFound)?;
    let target_id: u64 = target.try_get("id").map_err(|_| ApiError::Internal)?;
    if target_id == user_id {
        return Err(self_block());
    }
    let status: String = target.try_get("status").map_err(|_| ApiError::Internal)?;
    let deleted_at: Option<chrono::DateTime<Utc>> = target
        .try_get("deleted_at")
        .map_err(|_| ApiError::Internal)?;
    if status == "deleted" || deleted_at.is_some() {
        return Err(ApiError::NotFound);
    }

    sqlx::query("INSERT IGNORE INTO blocks (blocker_id, blocked_id, created_at) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(target_id)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
    tx.commit().await.map_err(|_| ApiError::Internal)
}

/// Idempotently remove the viewer's directed row for any known user, including
/// a suspended or deleted target. The reverse relationship is untouched.
pub async fn delete(pool: &MySqlPool, user_id: u64, public_id: &str) -> Result<(), ApiError> {
    let target_id: u64 = sqlx::query_scalar("SELECT id FROM users WHERE public_id = ?")
        .bind(public_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    sqlx::query("DELETE FROM blocks WHERE blocker_id = ? AND blocked_id = ?")
        .bind(user_id)
        .bind(target_id)
        .execute(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    Ok(())
}

/// Newest-first private keyset page. Cursor values are bound directly so an
/// unblock of the anchor cannot invalidate the following page.
pub async fn list(
    pool: &MySqlPool,
    user_id: u64,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let anchor = cursor.map(crate::listings::decode_cursor).transpose()?;
    let limit = crate::listings::page_limit(limit_raw);
    let mut sql = String::from(
        "SELECT b.blocked_id, b.created_at AS blocked_at, \
         u.public_id, u.display_name \
         FROM blocks b JOIN users u ON u.id = b.blocked_id \
         WHERE b.blocker_id = ?",
    );
    if anchor.is_some() {
        sql.push_str(" AND (b.created_at, b.blocked_id) < (?, ?)");
    }
    sql.push_str(" ORDER BY b.created_at DESC, b.blocked_id DESC LIMIT ?");

    let mut query = sqlx::query(&sql).bind(user_id);
    if let Some((created_at, blocked_id)) = anchor {
        query = query.bind(created_at).bind(blocked_id);
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
                let blocked_at: chrono::DateTime<Utc> =
                    row.try_get("blocked_at").map_err(|_| ApiError::Internal)?;
                let blocked_id: u64 = row.try_get("blocked_id").map_err(|_| ApiError::Internal)?;
                Ok(crate::listings::encode_cursor(&blocked_at, blocked_id))
            })
            .transpose()?
    } else {
        None
    };
    let data = rows
        .iter()
        .map(|row| {
            let blocked_at: chrono::DateTime<Utc> = row
                .try_get("blocked_at")
                .map_err(|_| ApiError::Internal)?;
            Ok(serde_json::json!({
                "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
                "display_name": row.try_get::<Option<String>, _>("display_name").map_err(|_| ApiError::Internal)?,
                "blocked_at": blocked_at.to_rfc3339(),
            }))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(serde_json::json!({
        "data": data,
        "page": { "next_cursor": next_cursor, "has_more": has_more },
    }))
}
