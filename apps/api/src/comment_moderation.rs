//! Automatic-comment moderation queue and transactional decisions.
//!
//! Every comment visibility transition shares listing → comment → case lock
//! ordering with report moderation and rescans. Counter changes happen only
//! for actual visible/non-deleted transitions. Queue output contains persisted
//! rule snapshots, never clearance internals or live rule/case identifiers.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use sqlx::{MySqlPool, Row, Transaction};

use crate::error::ApiError;
use crate::moderation::RuleClearance;

fn validation(field: &str, reason: &str) -> ApiError {
    let mut fields = BTreeMap::new();
    fields.insert(field.to_string(), reason.to_string());
    ApiError::Validation { fields }
}

pub fn invalid_transition() -> ApiError {
    validation("status", "invalid_transition")
}

fn optional_reason(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    let trimmed = raw.unwrap_or_default().trim();
    if trimmed.chars().count() > 500 {
        return Err(validation("reason", "too_long"));
    }
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

#[derive(Clone, Copy)]
pub enum CommentTransition {
    Hide,
    RestoreReport,
    RestoreFilter,
}

struct CommentLock {
    status: String,
    deleted: bool,
}

async fn lock_listing(
    tx: &mut Transaction<'_, sqlx::MySql>,
    listing_id: u64,
) -> Result<u32, ApiError> {
    sqlx::query_scalar("SELECT comment_count FROM listings WHERE id = ? FOR UPDATE")
        .bind(listing_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or_else(invalid_transition)
}

async fn lock_comment(
    tx: &mut Transaction<'_, sqlx::MySql>,
    listing_id: u64,
    comment_id: u64,
) -> Result<CommentLock, ApiError> {
    let row = sqlx::query(
        "SELECT status, deleted_at FROM listing_comments \
         WHERE id = ? AND listing_id = ? FOR UPDATE",
    )
    .bind(comment_id)
    .bind(listing_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or_else(invalid_transition)?;
    let deleted_at: Option<DateTime<Utc>> =
        row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
    Ok(CommentLock {
        status: row.try_get("status").map_err(|_| ApiError::Internal)?,
        deleted: deleted_at.is_some(),
    })
}

fn clearances_from_json(bytes: &[u8]) -> Result<Vec<RuleClearance>, ApiError> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| ApiError::Internal)?;
    let entries = value.as_array().ok_or(ApiError::Internal)?;
    entries
        .iter()
        .map(|entry| {
            Ok(RuleClearance {
                public_id: entry
                    .get("public_id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or(ApiError::Internal)?
                    .to_string(),
                revision: entry
                    .get("revision")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or(ApiError::Internal)?,
            })
        })
        .collect()
}

async fn lock_case_for_restore(
    tx: &mut Transaction<'_, sqlx::MySql>,
    comment_id: u64,
    filter: bool,
) -> Result<Vec<RuleClearance>, ApiError> {
    let row = sqlx::query(
        "SELECT status, matched_rules FROM comment_moderation_cases \
         WHERE comment_id = ? FOR UPDATE",
    )
    .bind(comment_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    let Some(row) = row else {
        if filter {
            return Err(invalid_transition());
        }
        return Ok(Vec::new());
    };
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    if filter {
        if !matches!(status.as_str(), "pending" | "confirmed_hidden") {
            return Err(invalid_transition());
        }
        let matched_rules: Vec<u8> = row
            .try_get("matched_rules")
            .map_err(|_| ApiError::Internal)?;
        clearances_from_json(&matched_rules)
    } else {
        if matches!(status.as_str(), "pending" | "confirmed_hidden") {
            return Err(invalid_transition());
        }
        Ok(Vec::new())
    }
}

/// Apply one strict comment transition inside the caller's transaction.
/// Automatic restores return their current matched rule/revision clearances;
/// report restores reject active automatic cases and return no clearances.
pub async fn transition_comment_in_tx(
    tx: &mut Transaction<'_, sqlx::MySql>,
    listing_id: u64,
    comment_id: u64,
    transition: CommentTransition,
) -> Result<Vec<RuleClearance>, ApiError> {
    let count = lock_listing(tx, listing_id).await?;
    let comment = lock_comment(tx, listing_id, comment_id).await?;
    let clearances = match transition {
        CommentTransition::Hide => Vec::new(),
        CommentTransition::RestoreReport => lock_case_for_restore(tx, comment_id, false).await?,
        CommentTransition::RestoreFilter => lock_case_for_restore(tx, comment_id, true).await?,
    };
    let (expected, desired) = match transition {
        CommentTransition::Hide => ("visible", "hidden"),
        CommentTransition::RestoreReport | CommentTransition::RestoreFilter => {
            ("hidden", "visible")
        }
    };
    if comment.deleted || comment.status != expected {
        return Err(invalid_transition());
    }

    let updated =
        sqlx::query("UPDATE listing_comments SET status = ?, updated_at = ? WHERE id = ?")
            .bind(desired)
            .bind(Utc::now())
            .bind(comment_id)
            .execute(&mut **tx)
            .await
            .map_err(|_| ApiError::Internal)?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::Internal);
    }
    let count = match transition {
        CommentTransition::Hide => count.checked_sub(1).ok_or(ApiError::Internal)?,
        CommentTransition::RestoreReport | CommentTransition::RestoreFilter => {
            count.checked_add(1).ok_or(ApiError::Internal)?
        }
    };
    let updated = sqlx::query("UPDATE listings SET comment_count = ? WHERE id = ?")
        .bind(count)
        .bind(listing_id)
        .execute(&mut **tx)
        .await
        .map_err(|_| ApiError::Internal)?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::Internal);
    }
    Ok(clearances)
}

fn case_select() -> &'static str {
    "SELECT mc.id, mc.status AS moderation_status, mc.matched_rules, \
     mc.flagged_at, mc.handled_at, c.public_id AS comment_public_id, c.body, \
     c.status AS comment_status, c.deleted_at, c.created_at, l.slug AS listing_slug, \
     author.public_id AS author_public_id, author.display_name AS author_name, \
     handler.public_id AS handler_public_id, handler.display_name AS handler_name \
     FROM comment_moderation_cases mc \
     JOIN listing_comments c ON c.id = mc.comment_id \
     JOIN listings l ON l.id = c.listing_id \
     JOIN users author ON author.id = c.author_id \
     LEFT JOIN users handler ON handler.id = mc.handled_by"
}

fn snapshots_from_row(row: &sqlx::mysql::MySqlRow) -> Result<Vec<serde_json::Value>, ApiError> {
    let bytes: Vec<u8> = row
        .try_get("matched_rules")
        .map_err(|_| ApiError::Internal)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| ApiError::Internal)?;
    value
        .as_array()
        .ok_or(ApiError::Internal)?
        .iter()
        .map(|snapshot| {
            let public_id = snapshot
                .get("public_id")
                .and_then(serde_json::Value::as_str)
                .ok_or(ApiError::Internal)?;
            let revision = snapshot
                .get("revision")
                .and_then(serde_json::Value::as_u64)
                .ok_or(ApiError::Internal)?;
            let phrase = snapshot
                .get("phrase")
                .and_then(serde_json::Value::as_str)
                .ok_or(ApiError::Internal)?;
            let match_mode = snapshot
                .get("match_mode")
                .and_then(serde_json::Value::as_str)
                .ok_or(ApiError::Internal)?;
            if !matches!(match_mode, "phrase" | "compact") {
                return Err(ApiError::Internal);
            }
            Ok(serde_json::json!({
                "public_id": public_id,
                "revision": revision,
                "phrase": phrase,
                "match_mode": match_mode,
            }))
        })
        .collect()
}

fn item_from_row(row: &sqlx::mysql::MySqlRow) -> Result<serde_json::Value, ApiError> {
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(|_| ApiError::Internal)?;
    let flagged_at: DateTime<Utc> = row.try_get("flagged_at").map_err(|_| ApiError::Internal)?;
    let handled_at: Option<DateTime<Utc>> =
        row.try_get("handled_at").map_err(|_| ApiError::Internal)?;
    let deleted_at: Option<DateTime<Utc>> =
        row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
    let handler_public_id: Option<String> = row
        .try_get("handler_public_id")
        .map_err(|_| ApiError::Internal)?;
    let handler_name: Option<String> = row
        .try_get("handler_name")
        .map_err(|_| ApiError::Internal)?;
    let handled_by = handler_public_id.map_or(
        serde_json::Value::Null,
        |public_id| serde_json::json!({"public_id": public_id, "display_name": handler_name}),
    );
    Ok(serde_json::json!({
        "comment": {
            "public_id": row.try_get::<String, _>("comment_public_id").map_err(|_| ApiError::Internal)?,
            "body": row.try_get::<String, _>("body").map_err(|_| ApiError::Internal)?,
            "status": row.try_get::<String, _>("comment_status").map_err(|_| ApiError::Internal)?,
            "deleted": deleted_at.is_some(),
            "created_at": created_at.to_rfc3339(),
            "author": {
                "public_id": row.try_get::<String, _>("author_public_id").map_err(|_| ApiError::Internal)?,
                "display_name": row.try_get::<Option<String>, _>("author_name").map_err(|_| ApiError::Internal)?,
            },
            "listing_slug": row.try_get::<String, _>("listing_slug").map_err(|_| ApiError::Internal)?,
        },
        "status": row.try_get::<String, _>("moderation_status").map_err(|_| ApiError::Internal)?,
        "flagged_at": flagged_at.to_rfc3339(),
        "matched_rules": snapshots_from_row(row)?,
        "handled_at": handled_at.map(|value| value.to_rfc3339()),
        "handled_by": handled_by,
    }))
}

pub async fn queue(
    pool: &MySqlPool,
    status_raw: Option<&str>,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let status = status_raw.unwrap_or("pending");
    if !matches!(status, "pending" | "confirmed_hidden" | "restored" | "all") {
        return Err(validation("status", "invalid"));
    }
    let anchor = cursor.map(crate::listings::decode_cursor).transpose()?;
    let limit = crate::listings::page_limit(limit_raw);
    let mut sql = case_select().to_string();
    sql.push_str(" WHERE 1 = 1");
    if status != "all" {
        sql.push_str(" AND mc.status = ?");
    }
    if anchor.is_some() {
        sql.push_str(" AND (mc.flagged_at, mc.id) > (?, ?)");
    }
    sql.push_str(" ORDER BY mc.flagged_at ASC, mc.id ASC LIMIT ?");
    let mut query = sqlx::query(&sql);
    if status != "all" {
        query = query.bind(status);
    }
    if let Some((flagged_at, id)) = anchor {
        query = query.bind(flagged_at).bind(id);
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
                let flagged_at: DateTime<Utc> =
                    row.try_get("flagged_at").map_err(|_| ApiError::Internal)?;
                let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
                Ok(crate::listings::encode_cursor(&flagged_at, id))
            })
            .transpose()?
    } else {
        None
    };
    let data = rows
        .iter()
        .map(item_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({
        "data": data,
        "page": {"next_cursor": next_cursor, "has_more": has_more},
    }))
}

async fn get_item(pool: &MySqlPool, public_id: &str) -> Result<serde_json::Value, ApiError> {
    let row = sqlx::query(&format!("{} WHERE c.public_id = ?", case_select()))
        .bind(public_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    item_from_row(&row)
}

struct CaseLock {
    id: u64,
    status: String,
    cleared_rules: Option<Vec<u8>>,
}

async fn lock_case(
    tx: &mut Transaction<'_, sqlx::MySql>,
    comment_id: u64,
) -> Result<CaseLock, ApiError> {
    let row = sqlx::query(
        "SELECT id, status, cleared_rules FROM comment_moderation_cases \
         WHERE comment_id = ? FOR UPDATE",
    )
    .bind(comment_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or_else(invalid_transition)?;
    Ok(CaseLock {
        id: row.try_get("id").map_err(|_| ApiError::Internal)?,
        status: row.try_get("status").map_err(|_| ApiError::Internal)?,
        cleared_rules: row
            .try_get("cleared_rules")
            .map_err(|_| ApiError::Internal)?,
    })
}

fn merged_clearances(
    previous: Option<&[u8]>,
    current: Vec<RuleClearance>,
) -> Result<String, ApiError> {
    let mut merged = previous
        .map(clearances_from_json)
        .transpose()?
        .unwrap_or_default();
    for clearance in current {
        if !merged.contains(&clearance) {
            merged.push(clearance);
        }
    }
    let value = merged
        .iter()
        .map(|clearance| {
            serde_json::json!({
                "public_id": &clearance.public_id,
                "revision": clearance.revision,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&value).map_err(|_| ApiError::Internal)
}

pub async fn act(
    pool: &MySqlPool,
    admin_id: u64,
    public_id: &str,
    action_raw: Option<&str>,
    reason_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let action = action_raw.unwrap_or_default().trim();
    if action.is_empty() {
        return Err(validation("action", "required"));
    }
    if !matches!(action, "confirm_hidden" | "restore") {
        return Err(validation("action", "invalid"));
    }
    let reason = if action == "confirm_hidden" {
        Some(crate::admin_users::required_reason(reason_raw)?)
    } else {
        optional_reason(reason_raw)?
    };
    let preliminary =
        sqlx::query("SELECT id, listing_id FROM listing_comments WHERE public_id = ?")
            .bind(public_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| ApiError::Internal)?
            .ok_or(ApiError::NotFound)?;
    let comment_id: u64 = preliminary.try_get("id").map_err(|_| ApiError::Internal)?;
    let listing_id: u64 = preliminary
        .try_get("listing_id")
        .map_err(|_| ApiError::Internal)?;

    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    lock_listing(&mut tx, listing_id).await?;
    let comment = lock_comment(&mut tx, listing_id, comment_id).await?;
    let case = lock_case(&mut tx, comment_id).await?;
    let repeated = match action {
        "confirm_hidden" => case.status == "confirmed_hidden" && comment.status == "hidden",
        "restore" => case.status == "restored" && comment.status == "visible" && !comment.deleted,
        _ => false,
    };
    if repeated {
        tx.commit().await.map_err(|_| ApiError::Internal)?;
        return get_item(pool, public_id).await;
    }

    let now = Utc::now();
    let (new_status, audit_action, clearances) = match action {
        "confirm_hidden" if case.status == "pending" && comment.status == "hidden" => {
            ("confirmed_hidden", "comment.filter_confirm_hidden", None)
        }
        "restore"
            if matches!(case.status.as_str(), "pending" | "confirmed_hidden")
                && comment.status == "hidden"
                && !comment.deleted =>
        {
            let current = transition_comment_in_tx(
                &mut tx,
                listing_id,
                comment_id,
                CommentTransition::RestoreFilter,
            )
            .await?;
            let clearances = merged_clearances(case.cleared_rules.as_deref(), current)?;
            ("restored", "comment.filter_restore", Some(clearances))
        }
        _ => return Err(invalid_transition()),
    };
    let updated = if let Some(clearances) = clearances {
        sqlx::query(
            "UPDATE comment_moderation_cases SET status = ?, cleared_rules = ?, \
             handled_by = ?, handled_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(new_status)
        .bind(clearances)
        .bind(admin_id)
        .bind(now)
        .bind(now)
        .bind(case.id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?
    } else {
        sqlx::query(
            "UPDATE comment_moderation_cases SET status = ?, handled_by = ?, \
             handled_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(new_status)
        .bind(admin_id)
        .bind(now)
        .bind(now)
        .bind(case.id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?
    };
    if updated.rows_affected() != 1 {
        return Err(ApiError::Internal);
    }
    crate::admin_users::audit_in_tx(
        &mut tx,
        admin_id,
        audit_action,
        "comment",
        comment_id,
        reason.as_deref(),
        Some(&serde_json::json!({"source": "automatic_profanity"})),
    )
    .await?;
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    get_item(pool, public_id).await
}
