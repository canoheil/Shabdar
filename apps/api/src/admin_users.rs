//! Admin user search and account moderation (Phase 9).
//!
//! The locked transition helper is shared with report-driven moderation. It
//! performs no audit by itself; the outer workflow writes exactly one when its
//! own contract says an actual decision occurred.

use std::collections::BTreeMap;

use chrono::Utc;
use sqlx::{MySqlPool, Row, Transaction};

use crate::error::ApiError;

#[derive(Clone, Copy)]
pub enum UserTransition {
    Suspend,
    Unsuspend,
}

impl UserTransition {
    fn desired_status(self) -> &'static str {
        match self {
            Self::Suspend => "suspended",
            Self::Unsuspend => "active",
        }
    }

    fn expected_status(self) -> &'static str {
        match self {
            Self::Suspend => "active",
            Self::Unsuspend => "suspended",
        }
    }

    fn audit_action(self) -> &'static str {
        match self {
            Self::Suspend => "user.suspend",
            Self::Unsuspend => "user.unsuspend",
        }
    }
}

pub fn required_reason(raw: Option<&str>) -> Result<String, ApiError> {
    let trimmed = raw.unwrap_or_default().trim();
    if trimmed.is_empty() {
        let mut fields = BTreeMap::new();
        fields.insert("reason".to_string(), "required".to_string());
        return Err(ApiError::Validation { fields });
    }
    if trimmed.chars().count() > 500 {
        let mut fields = BTreeMap::new();
        fields.insert("reason".to_string(), "too_long".to_string());
        return Err(ApiError::Validation { fields });
    }
    Ok(trimmed.to_string())
}

fn invalid_transition() -> ApiError {
    let mut fields = BTreeMap::new();
    fields.insert("status".to_string(), "invalid_transition".to_string());
    ApiError::Validation { fields }
}

/// Insert one audit row inside the caller's transaction. Metadata is JSON and
/// must contain public identifiers only, never session or personal payloads.
pub async fn audit_in_tx(
    tx: &mut Transaction<'_, sqlx::MySql>,
    admin_id: u64,
    action: &str,
    target_type: &str,
    target_id: u64,
    reason: Option<&str>,
    metadata: Option<&serde_json::Value>,
) -> Result<(), ApiError> {
    let metadata = metadata
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| ApiError::Internal)?;
    sqlx::query(
        "INSERT INTO admin_audit_logs \
         (admin_id, action, target_type, target_id, reason, metadata, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(reason)
    .bind(metadata)
    .bind(Utc::now())
    .execute(&mut **tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    Ok(())
}

/// Lock and re-evaluate one user transition. An already-desired status is an
/// idempotent no-op; any other source status is invalid. Suspension revokes all
/// sessions in the same transaction and never restores them on unsuspension.
pub async fn transition_by_id_in_tx(
    tx: &mut Transaction<'_, sqlx::MySql>,
    admin_id: u64,
    user_id: u64,
    transition: UserTransition,
) -> Result<bool, ApiError> {
    if matches!(transition, UserTransition::Suspend) && user_id == admin_id {
        return Err(ApiError::Forbidden);
    }
    let status: String = sqlx::query_scalar("SELECT status FROM users WHERE id = ? FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    if status == transition.desired_status() {
        return Ok(false);
    }
    if status != transition.expected_status() {
        return Err(invalid_transition());
    }

    let updated = sqlx::query("UPDATE users SET status = ?, updated_at = ? WHERE id = ?")
        .bind(transition.desired_status())
        .bind(Utc::now())
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(|_| ApiError::Internal)?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::Internal);
    }
    if matches!(transition, UserTransition::Suspend) {
        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut **tx)
            .await
            .map_err(|_| ApiError::Internal)?;
    }
    Ok(true)
}

fn user_from_row(row: &sqlx::mysql::MySqlRow) -> Result<serde_json::Value, ApiError> {
    let created_at: chrono::DateTime<Utc> =
        row.try_get("created_at").map_err(|_| ApiError::Internal)?;
    Ok(serde_json::json!({
        "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
        "display_name": row.try_get::<Option<String>, _>("display_name").map_err(|_| ApiError::Internal)?,
        "phone_e164": row.try_get::<String, _>("phone_e164").map_err(|_| ApiError::Internal)?,
        "status": row.try_get::<String, _>("status").map_err(|_| ApiError::Internal)?,
        "is_admin": row.try_get::<bool, _>("is_admin").map_err(|_| ApiError::Internal)?,
        "created_at": created_at.to_rfc3339(),
    }))
}

async fn get_user(pool: &MySqlPool, public_id: &str) -> Result<serde_json::Value, ApiError> {
    let row = sqlx::query(
        "SELECT public_id, display_name, phone_e164, status, is_admin, created_at \
         FROM users WHERE public_id = ?",
    )
    .bind(public_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    user_from_row(&row)
}

/// Direct admin suspend/unsuspend command. Only an actual transition writes an
/// audit; repeating the desired state returns the same public admin object.
pub async fn transition(
    pool: &MySqlPool,
    admin_id: u64,
    public_id: &str,
    transition: UserTransition,
    reason_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let reason = match transition {
        UserTransition::Suspend => Some(required_reason(reason_raw)?),
        UserTransition::Unsuspend => None,
    };
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let user_id: u64 = sqlx::query_scalar("SELECT id FROM users WHERE public_id = ?")
        .bind(public_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    let changed = transition_by_id_in_tx(&mut tx, admin_id, user_id, transition).await?;
    if changed {
        audit_in_tx(
            &mut tx,
            admin_id,
            transition.audit_action(),
            "user",
            user_id,
            reason.as_deref(),
            None,
        )
        .await?;
    }
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    get_user(pool, public_id).await
}

fn like_pattern(raw: &str) -> String {
    let mut escaped = String::new();
    for character in raw.chars() {
        if matches!(character, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    format!("%{escaped}%")
}

/// Newest-first admin-only user search. LIKE wildcards from the query are
/// literal, and cursor values are bound directly rather than re-queried.
pub async fn search(
    pool: &MySqlPool,
    query_raw: Option<&str>,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let query = query_raw.unwrap_or_default().trim();
    if query.chars().count() > 100 {
        let mut fields = BTreeMap::new();
        fields.insert("query".to_string(), "too_long".to_string());
        return Err(ApiError::Validation { fields });
    }
    let anchor = cursor.map(crate::listings::decode_cursor).transpose()?;
    let limit = crate::listings::page_limit(limit_raw);
    let pattern = like_pattern(query);
    let mut sql = String::from(
        "SELECT id, public_id, display_name, phone_e164, status, is_admin, created_at \
         FROM users WHERE 1 = 1",
    );
    if !query.is_empty() {
        sql.push_str(" AND (display_name LIKE ? ESCAPE '\\\\' OR phone_e164 LIKE ? ESCAPE '\\\\')");
    }
    if anchor.is_some() {
        sql.push_str(" AND (created_at, id) < (?, ?)");
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");

    let mut query_builder = sqlx::query(&sql);
    if !query.is_empty() {
        query_builder = query_builder.bind(&pattern).bind(&pattern);
    }
    if let Some((created_at, id)) = anchor {
        query_builder = query_builder.bind(created_at).bind(id);
    }
    query_builder = query_builder.bind((limit + 1) as i64);
    let mut rows = query_builder
        .fetch_all(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                let created_at: chrono::DateTime<Utc> =
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
        .map(user_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({
        "data": data,
        "page": { "next_cursor": next_cursor, "has_more": has_more },
    }))
}
