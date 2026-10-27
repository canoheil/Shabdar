//! Admin abuse-report queue and transactional moderation decisions.
//!
//! Pages load reports first and then at most one current-target batch per
//! represented type. Decisions lock the report, re-evaluate its resolution,
//! mutate the target, and write exactly one audit in the same transaction.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use sqlx::{MySqlPool, Row, Transaction};

use crate::error::ApiError;

#[derive(Clone)]
struct ReportCore {
    id: u64,
    public_id: String,
    target_type: String,
    target_id: u64,
    reason: String,
    details: Option<String>,
    status: String,
    resolution_action: Option<String>,
    handled_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl ReportCore {
    fn from_row(row: &sqlx::mysql::MySqlRow) -> Result<Self, ApiError> {
        Ok(Self {
            id: row.try_get("id").map_err(|_| ApiError::Internal)?,
            public_id: row.try_get("public_id").map_err(|_| ApiError::Internal)?,
            target_type: row.try_get("target_type").map_err(|_| ApiError::Internal)?,
            target_id: row.try_get("target_id").map_err(|_| ApiError::Internal)?,
            reason: row.try_get("reason").map_err(|_| ApiError::Internal)?,
            details: row.try_get("details").map_err(|_| ApiError::Internal)?,
            status: row.try_get("status").map_err(|_| ApiError::Internal)?,
            resolution_action: row
                .try_get("resolution_action")
                .map_err(|_| ApiError::Internal)?,
            handled_at: row.try_get("handled_at").map_err(|_| ApiError::Internal)?,
            created_at: row.try_get("created_at").map_err(|_| ApiError::Internal)?,
        })
    }
}

struct ReportRecord {
    core: ReportCore,
    reporter_public_id: String,
    reporter_name: Option<String>,
    handler_public_id: Option<String>,
    handler_name: Option<String>,
}

impl ReportRecord {
    fn from_row(row: &sqlx::mysql::MySqlRow) -> Result<Self, ApiError> {
        Ok(Self {
            core: ReportCore::from_row(row)?,
            reporter_public_id: row
                .try_get("reporter_public_id")
                .map_err(|_| ApiError::Internal)?,
            reporter_name: row
                .try_get("reporter_name")
                .map_err(|_| ApiError::Internal)?,
            handler_public_id: row
                .try_get("handler_public_id")
                .map_err(|_| ApiError::Internal)?,
            handler_name: row
                .try_get("handler_name")
                .map_err(|_| ApiError::Internal)?,
        })
    }

    fn json(&self, target: serde_json::Value) -> serde_json::Value {
        let handled_by = match &self.handler_public_id {
            Some(public_id) => serde_json::json!({
                "public_id": public_id,
                "display_name": &self.handler_name,
            }),
            None => serde_json::Value::Null,
        };
        serde_json::json!({
            "public_id": &self.core.public_id,
            "target_type": &self.core.target_type,
            "reason": &self.core.reason,
            "details": &self.core.details,
            "status": &self.core.status,
            "resolution_action": &self.core.resolution_action,
            "created_at": self.core.created_at.to_rfc3339(),
            "handled_at": self.core.handled_at.as_ref().map(|value| value.to_rfc3339()),
            "reporter": {
                "public_id": &self.reporter_public_id,
                "display_name": &self.reporter_name,
            },
            "handled_by": handled_by,
            "target": target,
        })
    }
}

fn report_select() -> &'static str {
    "SELECT r.*, reporter.public_id AS reporter_public_id, \
     reporter.display_name AS reporter_name, handler.public_id AS handler_public_id, \
     handler.display_name AS handler_name FROM reports r \
     JOIN users reporter ON reporter.id = r.reporter_id \
     LEFT JOIN users handler ON handler.id = r.handled_by"
}

#[derive(Default)]
struct TargetMaps {
    listings: HashMap<u64, serde_json::Value>,
    comments: HashMap<u64, serde_json::Value>,
    users: HashMap<u64, serde_json::Value>,
}

impl TargetMaps {
    fn get(&self, target_type: &str, target_id: u64) -> serde_json::Value {
        let map = match target_type {
            "listing" => &self.listings,
            "comment" => &self.comments,
            "user" => &self.users,
            _ => return serde_json::Value::Null,
        };
        map.get(&target_id)
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }
}

fn in_sql(prefix: &str, ids: &BTreeSet<u64>) -> String {
    let placeholders = vec!["?"; ids.len()].join(",");
    format!("{prefix} IN ({placeholders})")
}

async fn load_targets(
    pool: &MySqlPool,
    refs: &[(String, u64)],
) -> Result<TargetMaps, ApiError> {
    let mut listing_ids = BTreeSet::new();
    let mut comment_ids = BTreeSet::new();
    let mut user_ids = BTreeSet::new();
    for (target_type, target_id) in refs {
        match target_type.as_str() {
            "listing" => {
                listing_ids.insert(*target_id);
            }
            "comment" => {
                comment_ids.insert(*target_id);
            }
            "user" => {
                user_ids.insert(*target_id);
            }
            _ => {}
        }
    }
    let mut maps = TargetMaps::default();
    if !listing_ids.is_empty() {
        let sql = in_sql(
            "SELECT id, public_id, slug, title, status, deleted_at FROM listings WHERE id",
            &listing_ids,
        );
        let mut query = sqlx::query(&sql);
        for id in &listing_ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(pool).await.map_err(|_| ApiError::Internal)?;
        for row in rows {
            let deleted_at: Option<DateTime<Utc>> = row
                .try_get("deleted_at")
                .map_err(|_| ApiError::Internal)?;
            let id = row.try_get("id").map_err(|_| ApiError::Internal)?;
            maps.listings.insert(
                id,
                serde_json::json!({
                    "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
                    "slug": row.try_get::<String, _>("slug").map_err(|_| ApiError::Internal)?,
                    "title": row.try_get::<String, _>("title").map_err(|_| ApiError::Internal)?,
                    "status": row.try_get::<String, _>("status").map_err(|_| ApiError::Internal)?,
                    "deleted": deleted_at.is_some(),
                }),
            );
        }
    }
    if !comment_ids.is_empty() {
        let sql = in_sql(
            "SELECT c.id, c.public_id, c.body, c.status, c.deleted_at, \
             l.slug AS listing_slug, u.public_id AS author_public_id, \
             u.display_name AS author_name FROM listing_comments c \
             JOIN listings l ON l.id = c.listing_id JOIN users u ON u.id = c.author_id \
             WHERE c.id",
            &comment_ids,
        );
        let mut query = sqlx::query(&sql);
        for id in &comment_ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(pool).await.map_err(|_| ApiError::Internal)?;
        for row in rows {
            let deleted_at: Option<DateTime<Utc>> = row
                .try_get("deleted_at")
                .map_err(|_| ApiError::Internal)?;
            let id = row.try_get("id").map_err(|_| ApiError::Internal)?;
            maps.comments.insert(
                id,
                serde_json::json!({
                    "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
                    "body": row.try_get::<String, _>("body").map_err(|_| ApiError::Internal)?,
                    "status": row.try_get::<String, _>("status").map_err(|_| ApiError::Internal)?,
                    "deleted": deleted_at.is_some(),
                    "author": {
                        "public_id": row.try_get::<String, _>("author_public_id").map_err(|_| ApiError::Internal)?,
                        "display_name": row.try_get::<Option<String>, _>("author_name").map_err(|_| ApiError::Internal)?,
                    },
                    "listing_slug": row.try_get::<String, _>("listing_slug").map_err(|_| ApiError::Internal)?,
                }),
            );
        }
    }
    if !user_ids.is_empty() {
        let sql = in_sql(
            "SELECT id, public_id, display_name, status FROM users WHERE id",
            &user_ids,
        );
        let mut query = sqlx::query(&sql);
        for id in &user_ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(pool).await.map_err(|_| ApiError::Internal)?;
        for row in rows {
            let id = row.try_get("id").map_err(|_| ApiError::Internal)?;
            maps.users.insert(
                id,
                serde_json::json!({
                    "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
                    "display_name": row.try_get::<Option<String>, _>("display_name").map_err(|_| ApiError::Internal)?,
                    "status": row.try_get::<String, _>("status").map_err(|_| ApiError::Internal)?,
                }),
            );
        }
    }
    Ok(maps)
}

async fn render_records(
    pool: &MySqlPool,
    records: Vec<ReportRecord>,
) -> Result<Vec<serde_json::Value>, ApiError> {
    let refs = records
        .iter()
        .map(|record| (record.core.target_type.clone(), record.core.target_id))
        .collect::<Vec<_>>();
    let targets = load_targets(pool, &refs).await?;
    records
        .iter()
        .map(|record| {
            let target = targets.get(&record.core.target_type, record.core.target_id);
            Ok(record.json(target))
        })
        .collect()
}

pub async fn queue(
    pool: &MySqlPool,
    status_raw: Option<&str>,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let status = status_raw.unwrap_or("open");
    if !matches!(status, "open" | "actioned" | "dismissed" | "all") {
        let mut fields = BTreeMap::new();
        fields.insert("status".to_string(), "invalid".to_string());
        return Err(ApiError::Validation { fields });
    }
    let anchor = cursor.map(crate::listings::decode_cursor).transpose()?;
    let limit = crate::listings::page_limit(limit_raw);
    let mut sql = report_select().to_string();
    sql.push_str(" WHERE 1 = 1");
    if status != "all" {
        sql.push_str(" AND r.status = ?");
    }
    if anchor.is_some() {
        sql.push_str(" AND (r.created_at, r.id) > (?, ?)");
    }
    sql.push_str(" ORDER BY r.created_at ASC, r.id ASC LIMIT ?");
    let mut query = sqlx::query(&sql);
    if status != "all" {
        query = query.bind(status);
    }
    if let Some((created_at, id)) = anchor {
        query = query.bind(created_at).bind(id);
    }
    query = query.bind((limit + 1) as i64);
    let mut rows = query.fetch_all(pool).await.map_err(|_| ApiError::Internal)?;
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                let core = ReportCore::from_row(row)?;
                Ok(crate::listings::encode_cursor(&core.created_at, core.id))
            })
            .transpose()?
    } else {
        None
    };
    let records = rows
        .iter()
        .map(ReportRecord::from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let data = render_records(pool, records).await?;
    Ok(serde_json::json!({
        "data": data,
        "page": { "next_cursor": next_cursor, "has_more": has_more },
    }))
}

async fn get_report(
    pool: &MySqlPool,
    public_id: &str,
) -> Result<serde_json::Value, ApiError> {
    let row = sqlx::query(&format!("{} WHERE r.public_id = ?", report_select()))
        .bind(public_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    let record = ReportRecord::from_row(&row)?;
    let mut rendered = render_records(pool, vec![record]).await?;
    rendered.pop().ok_or(ApiError::Internal)
}

/// Real report history for an admin listing detail; no internal IDs or
/// placeholder array remains in the public contract.
pub async fn history_for_listing(
    pool: &MySqlPool,
    listing_id: u64,
) -> Result<Vec<serde_json::Value>, ApiError> {
    let rows = sqlx::query(&format!(
        "{} WHERE r.target_type = 'listing' AND r.target_id = ? \
         ORDER BY r.created_at ASC, r.id ASC",
        report_select()
    ))
    .bind(listing_id)
    .fetch_all(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    let records = rows
        .iter()
        .map(ReportRecord::from_row)
        .collect::<Result<Vec<_>, _>>()?;
    render_records(pool, records).await
}

fn validation(field: &str, reason: &str) -> ApiError {
    let mut fields = BTreeMap::new();
    fields.insert(field.to_string(), reason.to_string());
    ApiError::Validation { fields }
}

fn invalid_transition() -> ApiError {
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

async fn moderate_comment_in_tx(
    tx: &mut Transaction<'_, sqlx::MySql>,
    listing_id: u64,
    comment_id: u64,
    restore: bool,
) -> Result<(), ApiError> {
    // The listing ID is immutable and was obtained before this transaction;
    // this is the first comment/listing lock and preserves listing → comment.
    let count: u32 = sqlx::query_scalar("SELECT comment_count FROM listings WHERE id = ? FOR UPDATE")
        .bind(listing_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or_else(invalid_transition)?;
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
    let deleted_at: Option<DateTime<Utc>> = row
        .try_get("deleted_at")
        .map_err(|_| ApiError::Internal)?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    let expected = if restore { "hidden" } else { "visible" };
    if deleted_at.is_some() || status != expected {
        return Err(invalid_transition());
    }
    let desired = if restore { "visible" } else { "hidden" };
    let updated = sqlx::query(
        "UPDATE listing_comments SET status = ?, updated_at = ? WHERE id = ?",
    )
    .bind(desired)
    .bind(Utc::now())
    .bind(comment_id)
    .execute(&mut **tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::Internal);
    }
    let count = if restore {
        count.checked_add(1).ok_or(ApiError::Internal)?
    } else {
        count.checked_sub(1).ok_or(ApiError::Internal)?
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
    Ok(())
}

fn target_error(error: ApiError) -> ApiError {
    match error {
        ApiError::NotFound | ApiError::Gone => invalid_transition(),
        other => other,
    }
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
    if !matches!(
        action,
        "hide_comment" | "restore_comment" | "remove_listing" | "suspend_user" | "dismiss"
    ) {
        return Err(validation("action", "invalid"));
    }
    let reason = if matches!(action, "hide_comment" | "remove_listing" | "suspend_user") {
        Some(crate::admin_users::required_reason(reason_raw)?)
    } else {
        optional_reason(reason_raw)?
    };

    // Prefetch only the immutable comment → listing association. Authoritative
    // report/target state is still checked under locks below. Keeping this read
    // outside the transaction avoids a stale-snapshot read before FOR UPDATE.
    let preliminary = sqlx::query("SELECT target_type, target_id FROM reports WHERE public_id = ?")
        .bind(public_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    let preliminary_type: String = preliminary
        .try_get("target_type")
        .map_err(|_| ApiError::Internal)?;
    let preliminary_id: u64 = preliminary
        .try_get("target_id")
        .map_err(|_| ApiError::Internal)?;
    let comment_listing_id = if matches!(action, "hide_comment" | "restore_comment")
        && preliminary_type == "comment"
    {
        sqlx::query_scalar::<_, u64>("SELECT listing_id FROM listing_comments WHERE id = ?")
            .bind(preliminary_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| ApiError::Internal)?
    } else {
        None
    };

    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let row = sqlx::query("SELECT * FROM reports WHERE public_id = ? FOR UPDATE")
        .bind(public_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    let report = ReportCore::from_row(&row)?;

    if report.status != "open" {
        if report.resolution_action.as_deref() == Some(action) {
            drop(tx);
            return get_report(pool, public_id).await;
        }
        let can_restore = report.status == "actioned"
            && report.resolution_action.as_deref() == Some("hide_comment")
            && action == "restore_comment";
        if !can_restore {
            return Err(invalid_transition());
        }
    } else if action == "restore_comment" {
        return Err(invalid_transition());
    }

    let compatible = match action {
        "hide_comment" | "restore_comment" => report.target_type == "comment",
        "remove_listing" => report.target_type == "listing",
        "suspend_user" => report.target_type == "user",
        "dismiss" => true,
        _ => false,
    };
    if !compatible {
        return Err(validation("action", "target_mismatch"));
    }

    match action {
        "hide_comment" | "restore_comment" => {
            let listing_id = comment_listing_id.ok_or_else(invalid_transition)?;
            moderate_comment_in_tx(
                &mut tx,
                listing_id,
                report.target_id,
                action == "restore_comment",
            )
            .await
            .map_err(target_error)?;
        }
        "remove_listing" => {
            let changed = crate::admin::remove_by_id_in_tx(
                &mut tx,
                admin_id,
                report.target_id,
                reason.as_deref().unwrap_or_default(),
            )
            .await
            .map_err(target_error)?;
            if !changed {
                return Err(invalid_transition());
            }
        }
        "suspend_user" => {
            let changed = crate::admin_users::transition_by_id_in_tx(
                &mut tx,
                admin_id,
                report.target_id,
                crate::admin_users::UserTransition::Suspend,
            )
            .await
            .map_err(target_error)?;
            if !changed {
                return Err(invalid_transition());
            }
        }
        "dismiss" => {}
        _ => return Err(validation("action", "invalid")),
    }

    let now = Utc::now();
    let report_status = if action == "dismiss" {
        "dismissed"
    } else {
        "actioned"
    };
    let updated = sqlx::query(
        "UPDATE reports SET status = ?, resolution_action = ?, handled_by = ?, handled_at = ? \
         WHERE id = ?",
    )
    .bind(report_status)
    .bind(action)
    .bind(admin_id)
    .bind(now)
    .bind(report.id)
    .execute(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::Internal);
    }
    let metadata = serde_json::json!({ "report_public_id": &report.public_id });
    crate::admin_users::audit_in_tx(
        &mut tx,
        admin_id,
        &format!("report.{action}"),
        &report.target_type,
        report.target_id,
        reason.as_deref(),
        Some(&metadata),
    )
    .await?;
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    get_report(pool, public_id).await
}
