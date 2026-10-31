//! Admin-managed literal moderation rules (Phase 10).
//!
//! The settings row serializes every rule mutation and rescan. Duplicate
//! retries and no-op patches return current state without changing revisions
//! or manufacturing audits. Matcher snapshots never expose internal IDs.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use sqlx::{mysql::MySqlRow, MySqlPool, Row, Transaction};

use crate::error::ApiError;
use crate::moderation::{normalize_rule, MatchMode, ModerationRule};

const RULE_SELECT: &str =
    "SELECT id, public_id, phrase, normalized_phrase, dedupe_key, match_mode, \
     is_active, revision, created_at, updated_at FROM moderation_rules";

#[derive(Debug)]
pub struct RulePatch {
    pub phrase: Option<String>,
    pub match_mode: Option<String>,
    pub is_active: Option<bool>,
    pub reason: String,
}

#[derive(Clone)]
struct RuleRecord {
    id: u64,
    public_id: String,
    phrase: String,
    normalized_phrase: String,
    dedupe_key: String,
    match_mode: MatchMode,
    is_active: bool,
    revision: u64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl RuleRecord {
    fn from_row(row: &MySqlRow) -> Result<Self, ApiError> {
        let match_mode: String = row.try_get("match_mode").map_err(|_| ApiError::Internal)?;
        Ok(Self {
            id: row.try_get("id").map_err(|_| ApiError::Internal)?,
            public_id: row.try_get("public_id").map_err(|_| ApiError::Internal)?,
            phrase: row.try_get("phrase").map_err(|_| ApiError::Internal)?,
            normalized_phrase: row
                .try_get("normalized_phrase")
                .map_err(|_| ApiError::Internal)?,
            dedupe_key: row.try_get("dedupe_key").map_err(|_| ApiError::Internal)?,
            match_mode: MatchMode::parse(&match_mode).ok_or(ApiError::Internal)?,
            is_active: row.try_get("is_active").map_err(|_| ApiError::Internal)?,
            revision: row.try_get("revision").map_err(|_| ApiError::Internal)?,
            created_at: row.try_get("created_at").map_err(|_| ApiError::Internal)?,
            updated_at: row.try_get("updated_at").map_err(|_| ApiError::Internal)?,
        })
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "public_id": &self.public_id,
            "phrase": &self.phrase,
            "match_mode": self.match_mode.as_str(),
            "is_active": self.is_active,
            "revision": self.revision,
            "created_at": self.created_at.to_rfc3339(),
            "updated_at": self.updated_at.to_rfc3339(),
        })
    }

    fn matcher_rule(&self) -> ModerationRule {
        ModerationRule {
            public_id: self.public_id.clone(),
            phrase: self.phrase.clone(),
            normalized_phrase: self.normalized_phrase.clone(),
            revision: self.revision,
            match_mode: self.match_mode,
        }
    }

    fn audit_metadata(&self) -> serde_json::Value {
        serde_json::json!({
            "public_id": &self.public_id,
            "revision": self.revision,
            "match_mode": self.match_mode.as_str(),
            "is_active": self.is_active,
        })
    }
}

fn validation(field: &str, reason: &str) -> ApiError {
    let mut fields = BTreeMap::new();
    fields.insert(field.to_string(), reason.to_string());
    ApiError::Validation { fields }
}

async fn lock_settings(tx: &mut Transaction<'_, sqlx::MySql>) -> Result<(), ApiError> {
    let _revision: u64 =
        sqlx::query_scalar("SELECT revision FROM moderation_settings WHERE id = 1 FOR UPDATE")
            .fetch_one(&mut **tx)
            .await
            .map_err(|_| ApiError::Internal)?;
    Ok(())
}

async fn increment_settings_revision(
    tx: &mut Transaction<'_, sqlx::MySql>,
) -> Result<(), ApiError> {
    let result = sqlx::query("UPDATE moderation_settings SET revision = revision + 1 WHERE id = 1")
        .execute(&mut **tx)
        .await
        .map_err(|_| ApiError::Internal)?;
    if result.rows_affected() != 1 {
        return Err(ApiError::Internal);
    }
    Ok(())
}

/// Newest-first keyset administration pages. Cursor keys are used directly,
/// so editing or deactivating the anchor cannot invalidate the next page.
pub async fn list(
    pool: &MySqlPool,
    active: Option<&str>,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let active = active.unwrap_or("all");
    let active_clause = match active {
        "all" => "",
        "active" => " AND is_active = TRUE",
        "inactive" => " AND is_active = FALSE",
        _ => return Err(validation("active", "invalid")),
    };
    let anchor = cursor.map(crate::listings::decode_cursor).transpose()?;
    let limit = crate::listings::page_limit(limit_raw);

    let mut sql = format!("{RULE_SELECT} WHERE 1 = 1{active_clause}");
    if anchor.is_some() {
        sql.push_str(" AND (created_at, id) < (?, ?)");
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");
    let mut query = sqlx::query(&sql);
    if let Some((created_at, id)) = anchor {
        query = query.bind(created_at).bind(id);
    }
    let mut rows = query
        .bind((limit + 1) as i64)
        .fetch_all(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                let created_at: DateTime<Utc> =
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
        .map(|row| RuleRecord::from_row(row).map(|rule| rule.json()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({
        "data": data,
        "page": { "next_cursor": next_cursor, "has_more": has_more },
    }))
}

/// Create an active rule, or return the unchanged exact duplicate. The
/// settings lock makes the duplicate check and insert one serialized decision.
pub async fn create(
    pool: &MySqlPool,
    admin_id: u64,
    phrase: Option<&str>,
    match_mode: Option<&str>,
    reason: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let reason = crate::admin_users::required_reason(reason)?;
    let match_mode = match match_mode {
        None => return Err(validation("match_mode", "required")),
        Some(raw) => MatchMode::parse(raw).ok_or_else(|| validation("match_mode", "invalid"))?,
    };
    let normalized = normalize_rule(phrase.unwrap_or_default(), match_mode)
        .map_err(|reason| validation("phrase", reason))?;

    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    lock_settings(&mut tx).await?;
    let duplicate = sqlx::query(&format!("{RULE_SELECT} WHERE dedupe_key = ? FOR UPDATE"))
        .bind(&normalized.dedupe_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
    if let Some(row) = duplicate {
        let existing = RuleRecord::from_row(&row)?;
        if existing.match_mode != match_mode {
            return Err(validation("phrase", "duplicate"));
        }
        let body = existing.json();
        tx.commit().await.map_err(|_| ApiError::Internal)?;
        return Ok(body);
    }

    let public_id = ulid::Ulid::new().to_string();
    let now = Utc::now();
    let inserted = sqlx::query(
        "INSERT INTO moderation_rules \
         (public_id, phrase, normalized_phrase, dedupe_key, match_mode, is_active, revision, \
          created_at, updated_at) VALUES (?, ?, ?, ?, ?, TRUE, 1, ?, ?)",
    )
    .bind(&public_id)
    .bind(&normalized.phrase)
    .bind(&normalized.normalized_phrase)
    .bind(&normalized.dedupe_key)
    .bind(match_mode.as_str())
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    let rule_id = inserted.last_insert_id();
    let row = sqlx::query(&format!("{RULE_SELECT} WHERE id = ? FOR UPDATE"))
        .bind(rule_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
    let rule = RuleRecord::from_row(&row)?;
    increment_settings_revision(&mut tx).await?;
    crate::admin_users::audit_in_tx(
        &mut tx,
        admin_id,
        "moderation_rule.create",
        "moderation_rule",
        rule.id,
        Some(&reason),
        Some(&rule.audit_metadata()),
    )
    .await?;
    let body = rule.json();
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    Ok(body)
}

/// Apply one atomic patch. Phrase/mode changes advance the content revision;
/// activation changes advance only the global rule-set revision.
pub async fn update(
    pool: &MySqlPool,
    admin_id: u64,
    public_id: &str,
    patch: RulePatch,
) -> Result<serde_json::Value, ApiError> {
    let reason = crate::admin_users::required_reason(Some(&patch.reason))?;
    if patch.phrase.is_none() && patch.match_mode.is_none() && patch.is_active.is_none() {
        return Err(validation("patch", "no_changes"));
    }
    let requested_mode = patch
        .match_mode
        .as_deref()
        .map(|raw| MatchMode::parse(raw).ok_or_else(|| validation("match_mode", "invalid")))
        .transpose()?;

    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    lock_settings(&mut tx).await?;
    let row = sqlx::query(&format!("{RULE_SELECT} WHERE public_id = ? FOR UPDATE"))
        .bind(public_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    let current = RuleRecord::from_row(&row)?;
    let match_mode = requested_mode.unwrap_or(current.match_mode);
    let normalized = normalize_rule(
        patch.phrase.as_deref().unwrap_or(&current.phrase),
        match_mode,
    )
    .map_err(|reason| validation("phrase", reason))?;
    let is_active = patch.is_active.unwrap_or(current.is_active);
    let content_changed = normalized.phrase != current.phrase
        || normalized.normalized_phrase != current.normalized_phrase
        || normalized.dedupe_key != current.dedupe_key
        || match_mode != current.match_mode;
    let active_changed = is_active != current.is_active;
    if !content_changed && !active_changed {
        let body = current.json();
        tx.commit().await.map_err(|_| ApiError::Internal)?;
        return Ok(body);
    }

    if content_changed {
        let duplicate = sqlx::query(&format!(
            "{RULE_SELECT} WHERE dedupe_key = ? AND id <> ? FOR UPDATE"
        ))
        .bind(&normalized.dedupe_key)
        .bind(current.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
        if duplicate.is_some() {
            return Err(validation("phrase", "duplicate"));
        }
    }

    let revision = if content_changed {
        current.revision.checked_add(1).ok_or(ApiError::Internal)?
    } else {
        current.revision
    };
    sqlx::query(
        "UPDATE moderation_rules SET phrase = ?, normalized_phrase = ?, dedupe_key = ?, \
         match_mode = ?, is_active = ?, revision = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&normalized.phrase)
    .bind(&normalized.normalized_phrase)
    .bind(&normalized.dedupe_key)
    .bind(match_mode.as_str())
    .bind(is_active)
    .bind(revision)
    .bind(Utc::now())
    .bind(current.id)
    .execute(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    let row = sqlx::query(&format!("{RULE_SELECT} WHERE id = ? FOR UPDATE"))
        .bind(current.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
    let rule = RuleRecord::from_row(&row)?;
    increment_settings_revision(&mut tx).await?;
    crate::admin_users::audit_in_tx(
        &mut tx,
        admin_id,
        "moderation_rule.update",
        "moderation_rule",
        rule.id,
        Some(&reason),
        Some(&rule.audit_metadata()),
    )
    .await?;
    let body = rule.json();
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    Ok(body)
}

/// Load one consistent, deterministic active-rule snapshot for comment writes.
pub async fn active_rules(pool: &MySqlPool) -> Result<Vec<ModerationRule>, ApiError> {
    let rows = sqlx::query(&format!(
        "{RULE_SELECT} WHERE is_active = TRUE ORDER BY id ASC"
    ))
    .fetch_all(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    rows.iter()
        .map(|row| RuleRecord::from_row(row).map(|rule| rule.matcher_rule()))
        .collect()
}
