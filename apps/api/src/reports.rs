//! Abuse report creation (Phase 9).
//!
//! Reports are inert until an admin decision. Public identifiers are resolved
//! to internal polymorphic IDs here and never returned to the caller.

use std::collections::BTreeMap;

use chrono::Utc;
use serde::Deserialize;
use sqlx::{MySqlPool, Row};

use crate::error::ApiError;

const TARGET_TYPES: [&str; 3] = ["listing", "comment", "user"];
const REASONS: [&str; 6] = [
    "spam",
    "abuse",
    "fraud",
    "prohibited_item",
    "offensive",
    "other",
];

/// JSON is kept as values until validation so wrong types and unknown fields
/// use the same field-level 400 envelope instead of serde's generic rejection.
#[derive(Deserialize)]
pub struct ReportBody {
    target_type: Option<serde_json::Value>,
    target_id: Option<serde_json::Value>,
    reason: Option<serde_json::Value>,
    details: Option<serde_json::Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

pub struct ValidatedReport {
    target_type: String,
    target_id: String,
    reason: String,
    details: Option<String>,
}

fn required_string(
    value: Option<serde_json::Value>,
    field: &str,
    invalid_reason: &str,
    fields: &mut BTreeMap<String, String>,
) -> Option<String> {
    match value {
        None | Some(serde_json::Value::Null) => {
            fields.insert(field.to_string(), "required".to_string());
            None
        }
        Some(serde_json::Value::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                fields.insert(field.to_string(), "required".to_string());
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Some(_) => {
            fields.insert(field.to_string(), invalid_reason.to_string());
            None
        }
    }
}

impl ReportBody {
    pub fn validate(self) -> Result<ValidatedReport, ApiError> {
        let mut fields = BTreeMap::new();
        for key in self.extra.keys() {
            fields.insert(key.clone(), "unknown_field".to_string());
        }
        let target_type = required_string(
            self.target_type,
            "target_type",
            "unknown_target_type",
            &mut fields,
        );
        if let Some(target_type) = &target_type {
            if !TARGET_TYPES.contains(&target_type.as_str()) {
                fields.insert("target_type".to_string(), "unknown_target_type".to_string());
            }
        }
        let target_id = required_string(self.target_id, "target_id", "invalid", &mut fields);
        let reason = required_string(self.reason, "reason", "unknown_reason", &mut fields);
        if let Some(reason) = &reason {
            if !REASONS.contains(&reason.as_str()) {
                fields.insert("reason".to_string(), "unknown_reason".to_string());
            }
        }
        let details = match self.details {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(value)) => {
                let trimmed = value.trim();
                if trimmed.chars().count() > 500 {
                    fields.insert("details".to_string(), "too_long".to_string());
                    None
                } else if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Some(_) => {
                fields.insert("details".to_string(), "invalid".to_string());
                None
            }
        };

        if !fields.is_empty() {
            return Err(ApiError::Validation { fields });
        }
        Ok(ValidatedReport {
            target_type: target_type.ok_or(ApiError::Internal)?,
            target_id: target_id.ok_or(ApiError::Internal)?,
            reason: reason.ok_or(ApiError::Internal)?,
            details,
        })
    }
}

fn self_report() -> ApiError {
    let mut fields = BTreeMap::new();
    fields.insert("target_id".to_string(), "self_report".to_string());
    ApiError::Validation { fields }
}

fn check_listing_visibility(status: &str, deleted: bool) -> Result<(), ApiError> {
    if status == "removed" || deleted {
        return Err(ApiError::Gone);
    }
    if status != "active" {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

async fn resolve_target(
    pool: &MySqlPool,
    reporter_id: u64,
    input: &ValidatedReport,
) -> Result<u64, ApiError> {
    match input.target_type.as_str() {
        "listing" => {
            let row = sqlx::query(
                "SELECT l.id, l.seller_id, l.status, l.deleted_at, \
                 l.image_reviewed_at IS NOT NULL AS image_reviewed, \
                 EXISTS(SELECT 1 FROM listing_images i WHERE i.listing_id = l.id) AS has_images \
                 FROM listings l WHERE l.slug = ?",
            )
            .bind(&input.target_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| ApiError::Internal)?
            .ok_or(ApiError::NotFound)?;
            let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
            let deleted_at: Option<chrono::DateTime<Utc>> =
                row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
            check_listing_visibility(&status, deleted_at.is_some())?;
            crate::listings::check_public_image_review(&row)?;
            let seller_id: u64 = row.try_get("seller_id").map_err(|_| ApiError::Internal)?;
            if seller_id == reporter_id {
                return Err(self_report());
            }
            row.try_get("id").map_err(|_| ApiError::Internal)
        }
        "comment" => {
            let row = sqlx::query(
                "SELECT c.id, c.author_id, c.status AS comment_status, \
                 c.deleted_at AS comment_deleted_at, l.status AS listing_status, \
                 l.deleted_at AS listing_deleted_at, \
                 l.image_reviewed_at IS NOT NULL AS image_reviewed, \
                 EXISTS(SELECT 1 FROM listing_images i WHERE i.listing_id = l.id) AS has_images \
                 FROM listing_comments c JOIN listings l ON l.id = c.listing_id \
                 WHERE c.public_id = ?",
            )
            .bind(&input.target_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| ApiError::Internal)?
            .ok_or(ApiError::NotFound)?;
            let listing_status: String = row
                .try_get("listing_status")
                .map_err(|_| ApiError::Internal)?;
            let listing_deleted_at: Option<chrono::DateTime<Utc>> = row
                .try_get("listing_deleted_at")
                .map_err(|_| ApiError::Internal)?;
            check_listing_visibility(&listing_status, listing_deleted_at.is_some())?;
            crate::listings::check_public_image_review(&row)?;
            let comment_status: String = row
                .try_get("comment_status")
                .map_err(|_| ApiError::Internal)?;
            let comment_deleted_at: Option<chrono::DateTime<Utc>> = row
                .try_get("comment_deleted_at")
                .map_err(|_| ApiError::Internal)?;
            if comment_status != "visible" || comment_deleted_at.is_some() {
                return Err(ApiError::NotFound);
            }
            let author_id: u64 = row.try_get("author_id").map_err(|_| ApiError::Internal)?;
            if author_id == reporter_id {
                return Err(self_report());
            }
            row.try_get("id").map_err(|_| ApiError::Internal)
        }
        "user" => {
            let row = sqlx::query("SELECT id, status, deleted_at FROM users WHERE public_id = ?")
                .bind(&input.target_id)
                .fetch_optional(pool)
                .await
                .map_err(|_| ApiError::Internal)?
                .ok_or(ApiError::NotFound)?;
            let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
            let deleted_at: Option<chrono::DateTime<Utc>> =
                row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
            if status != "active" || deleted_at.is_some() {
                return Err(ApiError::NotFound);
            }
            let target_id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
            if target_id == reporter_id {
                return Err(self_report());
            }
            Ok(target_id)
        }
        _ => Err(ApiError::Internal),
    }
}

/// Create or return the caller's existing report for this target. INSERT
/// IGNORE uses the unique key as the concurrency boundary; reason/details and
/// the original timestamp are never overwritten by a retry.
pub async fn create(
    pool: &MySqlPool,
    reporter_id: u64,
    input: ValidatedReport,
) -> Result<serde_json::Value, ApiError> {
    let target_id = resolve_target(pool, reporter_id, &input).await?;
    let candidate_id = ulid::Ulid::new().to_string();
    sqlx::query(
        "INSERT IGNORE INTO reports \
         (public_id, reporter_id, target_type, target_id, reason, details, status, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, 'open', ?)",
    )
    .bind(candidate_id)
    .bind(reporter_id)
    .bind(&input.target_type)
    .bind(target_id)
    .bind(&input.reason)
    .bind(input.details.as_deref())
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|_| ApiError::Internal)?;

    let row = sqlx::query(
        "SELECT public_id, status FROM reports \
         WHERE reporter_id = ? AND target_type = ? AND target_id = ?",
    )
    .bind(reporter_id)
    .bind(&input.target_type)
    .bind(target_id)
    .fetch_one(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    Ok(serde_json::json!({
        "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
        "status": row.try_get::<String, _>("status").map_err(|_| ApiError::Internal)?,
    }))
}
