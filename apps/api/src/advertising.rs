//! Public house-ad inventory and the private advertising-request workflow.
//!
//! The slot is a singleton, never a listing. Public responses contain creative
//! copy only; contact details exist solely in admin request responses.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::{mysql::MySqlRow, MySqlPool, Row};

use crate::error::ApiError;
use crate::ratelimit::RateLimiter;

const REQUEST_IP_LIMIT: u32 = 8;
const REQUEST_PHONE_DAILY_LIMIT: u32 = 3;
const REQUEST_GLOBAL_LIMIT: u32 = 200;

#[derive(Debug)]
pub struct SlotInput {
    pub title: Option<String>,
    pub body: Option<String>,
    pub cta_label: Option<String>,
    pub cta_url: Option<String>,
    pub enabled: Option<bool>,
}

struct ValidatedSlot {
    title: String,
    body: String,
    cta_label: String,
    cta_url: String,
    enabled: bool,
}

struct SlotRecord {
    public_id: String,
    title: String,
    body: String,
    cta_label: String,
    cta_url: String,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl SlotRecord {
    fn from_row(row: &MySqlRow) -> Result<Self, ApiError> {
        Ok(Self {
            public_id: row.try_get("public_id").map_err(|_| ApiError::Internal)?,
            title: row.try_get("title").map_err(|_| ApiError::Internal)?,
            body: row.try_get("body").map_err(|_| ApiError::Internal)?,
            cta_label: row.try_get("cta_label").map_err(|_| ApiError::Internal)?,
            cta_url: row.try_get("cta_url").map_err(|_| ApiError::Internal)?,
            enabled: row.try_get("enabled").map_err(|_| ApiError::Internal)?,
            created_at: row.try_get("created_at").map_err(|_| ApiError::Internal)?,
            updated_at: row.try_get("updated_at").map_err(|_| ApiError::Internal)?,
        })
    }

    fn public_json(&self) -> serde_json::Value {
        serde_json::json!({
            "public_id": &self.public_id,
            "title": &self.title,
            "body": &self.body,
            "cta_label": &self.cta_label,
            "cta_url": &self.cta_url,
        })
    }

    fn admin_json(&self) -> serde_json::Value {
        serde_json::json!({
            "public_id": &self.public_id,
            "title": &self.title,
            "body": &self.body,
            "cta_label": &self.cta_label,
            "cta_url": &self.cta_url,
            "enabled": self.enabled,
            "created_at": self.created_at.to_rfc3339(),
            "updated_at": self.updated_at.to_rfc3339(),
        })
    }
}

const SLOT_SELECT: &str = "SELECT public_id, title, body, cta_label, cta_url, enabled, \
     created_at, updated_at FROM advertising_slots WHERE id = 1";

fn validation(field: &str, reason: &str) -> ApiError {
    let mut fields = BTreeMap::new();
    fields.insert(field.to_string(), reason.to_string());
    ApiError::Validation { fields }
}

fn required_text(
    raw: Option<String>,
    field: &'static str,
    maximum: usize,
) -> Result<String, ApiError> {
    let value = raw.unwrap_or_default().trim().to_string();
    if value.is_empty() {
        return Err(validation(field, "required"));
    }
    if value.chars().count() > maximum {
        return Err(validation(field, "too_long"));
    }
    if value
        .chars()
        .any(|character| character.is_control() && character != '\n')
    {
        return Err(validation(field, "invalid"));
    }
    Ok(value)
}

/// Internal absolute paths and HTTPS destinations only. Protocol-relative,
/// HTTP, credential-bearing and whitespace/control-containing URLs are refused.
fn valid_destination(value: &str) -> bool {
    if value.is_empty()
        || value.chars().any(|character| {
            character.is_control() || character.is_whitespace() || character == '\\'
        })
    {
        return false;
    }
    if let Some(path) = value.strip_prefix('/') {
        return !path.starts_with('/');
    }
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') || authority.starts_with('[') {
        return false;
    }
    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    if host.contains(':')
        || port.is_some_and(|value| value.parse::<u16>().ok().is_none_or(|port| port == 0))
    {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn validate_slot(input: SlotInput) -> Result<ValidatedSlot, ApiError> {
    let title = required_text(input.title, "title", 100)?;
    let body = required_text(input.body, "body", 500)?;
    let cta_label = required_text(input.cta_label, "cta_label", 80)?;
    let cta_url = required_text(input.cta_url, "cta_url", 500)?;
    if !valid_destination(&cta_url) {
        return Err(validation("cta_url", "invalid_url"));
    }
    Ok(ValidatedSlot {
        title,
        body,
        cta_label,
        cta_url,
        enabled: input.enabled.unwrap_or(true),
    })
}

/// Public creative only. Disabled or deleted inventory is an explicit null.
pub async fn public_slot(pool: &MySqlPool) -> Result<serde_json::Value, ApiError> {
    let row = sqlx::query(&format!("{SLOT_SELECT} AND enabled = TRUE"))
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    let slot = row
        .as_ref()
        .map(SlotRecord::from_row)
        .transpose()?
        .map(|record| record.public_json());
    Ok(serde_json::json!({ "slot": slot }))
}

pub async fn admin_slot(pool: &MySqlPool) -> Result<serde_json::Value, ApiError> {
    let row = sqlx::query(SLOT_SELECT)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    let slot = row
        .as_ref()
        .map(SlotRecord::from_row)
        .transpose()?
        .map(|record| record.admin_json());
    Ok(serde_json::json!({ "slot": slot }))
}

/// Full replacement upsert. A confirmed delete followed by save creates a new
/// public id; ordinary edits retain it so visitor dismissal stays meaningful.
pub async fn save_slot(
    pool: &MySqlPool,
    admin_id: u64,
    input: SlotInput,
) -> Result<serde_json::Value, ApiError> {
    let input = validate_slot(input)?;
    let now = Utc::now();
    let mut tx = pool.begin().await.map_err(crate::error::database_error)?;
    let existing = sqlx::query("SELECT public_id FROM advertising_slots WHERE id = 1 FOR UPDATE")
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::error::database_error)?;
    let (public_id, action) = if let Some(row) = existing {
        let public_id: String = row.try_get("public_id").map_err(|_| ApiError::Internal)?;
        sqlx::query(
            "UPDATE advertising_slots SET title = ?, body = ?, cta_label = ?, cta_url = ?, \
             enabled = ?, updated_at = ? WHERE id = 1",
        )
        .bind(&input.title)
        .bind(&input.body)
        .bind(&input.cta_label)
        .bind(&input.cta_url)
        .bind(input.enabled)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(crate::error::database_error)?;
        (public_id, "advertising.slot.update")
    } else {
        let public_id = ulid::Ulid::new().to_string();
        sqlx::query(
            "INSERT INTO advertising_slots \
             (id, public_id, title, body, cta_label, cta_url, enabled, created_at, updated_at) \
             VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&public_id)
        .bind(&input.title)
        .bind(&input.body)
        .bind(&input.cta_label)
        .bind(&input.cta_url)
        .bind(input.enabled)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(crate::error::database_error)?;
        (public_id, "advertising.slot.create")
    };
    crate::admin_users::audit_in_tx(
        &mut tx,
        admin_id,
        action,
        "advertising_slot",
        1,
        None,
        Some(&serde_json::json!({ "public_id": public_id, "enabled": input.enabled })),
    )
    .await?;
    tx.commit().await.map_err(crate::error::database_error)?;
    admin_slot(pool).await
}

pub async fn delete_slot(pool: &MySqlPool, admin_id: u64) -> Result<serde_json::Value, ApiError> {
    let mut tx = pool.begin().await.map_err(crate::error::database_error)?;
    let existing = sqlx::query("SELECT public_id FROM advertising_slots WHERE id = 1 FOR UPDATE")
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::error::database_error)?;
    let Some(row) = existing else {
        tx.commit().await.map_err(crate::error::database_error)?;
        return Ok(serde_json::json!({ "deleted": false }));
    };
    let public_id: String = row.try_get("public_id").map_err(|_| ApiError::Internal)?;
    sqlx::query("DELETE FROM advertising_slots WHERE id = 1")
        .execute(&mut *tx)
        .await
        .map_err(crate::error::database_error)?;
    crate::admin_users::audit_in_tx(
        &mut tx,
        admin_id,
        "advertising.slot.delete",
        "advertising_slot",
        1,
        None,
        Some(&serde_json::json!({ "public_id": public_id })),
    )
    .await?;
    tx.commit().await.map_err(crate::error::database_error)?;
    Ok(serde_json::json!({ "deleted": true }))
}

#[derive(Debug)]
pub struct RequestInput {
    pub submission_id: Option<String>,
    pub contact_name: Option<String>,
    pub phone: Option<String>,
    pub details: Option<String>,
}

struct ValidatedRequest {
    submission_id: String,
    contact_name: String,
    phone_e164: String,
    details: Option<String>,
}

fn valid_submission_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23]
            .into_iter()
            .all(|index| bytes[index] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_hexdigit())
}

fn validate_request(input: RequestInput) -> Result<ValidatedRequest, ApiError> {
    let submission_id = input.submission_id.unwrap_or_default();
    if !valid_submission_id(&submission_id) {
        return Err(validation("submission_id", "invalid"));
    }
    let contact_name = required_text(input.contact_name, "contact_name", 80)?;
    let phone_e164 = crate::phone::normalize_iranian_mobile(input.phone.as_deref().unwrap_or(""))
        .map_err(|_| validation("phone", "invalid_phone"))?;
    let details = input
        .details
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if details
        .as_ref()
        .is_some_and(|value| value.chars().count() > 1000)
    {
        return Err(validation("details", "too_long"));
    }
    if details.as_ref().is_some_and(|value| {
        value
            .chars()
            .any(|character| character.is_control() && character != '\n')
    }) {
        return Err(validation("details", "invalid"));
    }
    Ok(ValidatedRequest {
        submission_id,
        contact_name,
        phone_e164,
        details,
    })
}

async fn request_receipt(
    pool: &MySqlPool,
    submission_id: &str,
) -> Result<serde_json::Value, ApiError> {
    let row =
        sqlx::query("SELECT public_id, status FROM advertising_requests WHERE submission_id = ?")
            .bind(submission_id)
            .fetch_one(pool)
            .await
            .map_err(|_| ApiError::Internal)?;
    Ok(serde_json::json!({
        "request": {
            "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
            "status": row.try_get::<String, _>("status").map_err(|_| ApiError::Internal)?,
        }
    }))
}

/// Idempotent by a per-form UUID. A retry returns the original receipt without
/// changing its status or spending another abuse-limit allowance.
pub async fn create_request(
    pool: &MySqlPool,
    limiter: &RateLimiter,
    client_ip: &str,
    input: RequestInput,
) -> Result<serde_json::Value, ApiError> {
    let input = validate_request(input)?;
    let existing =
        sqlx::query("SELECT public_id FROM advertising_requests WHERE submission_id = ?")
            .bind(&input.submission_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| ApiError::Internal)?;
    if existing.is_some() {
        return request_receipt(pool, &input.submission_id).await;
    }

    limiter
        .hit(
            &format!("advertising:request:phone:{}", input.phone_e164),
            REQUEST_PHONE_DAILY_LIMIT,
            Duration::from_secs(86_400),
        )
        .into_result()?;
    limiter
        .hit(
            &format!("advertising:request:ip:{client_ip}"),
            REQUEST_IP_LIMIT,
            Duration::from_secs(3_600),
        )
        .into_result()?;
    limiter
        .hit(
            "advertising:request:global",
            REQUEST_GLOBAL_LIMIT,
            Duration::from_secs(3_600),
        )
        .into_result()?;

    let now = Utc::now();
    sqlx::query(
        "INSERT IGNORE INTO advertising_requests \
         (public_id, submission_id, contact_name, phone_e164, details, status, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, 'new', ?, ?)",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(&input.submission_id)
    .bind(&input.contact_name)
    .bind(&input.phone_e164)
    .bind(input.details.as_deref())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    request_receipt(pool, &input.submission_id).await
}

struct AdminRequestRecord {
    public_id: String,
    contact_name: String,
    phone_e164: String,
    details: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    handled_at: Option<DateTime<Utc>>,
    handler_public_id: Option<String>,
    handler_display_name: Option<String>,
}

impl AdminRequestRecord {
    fn from_row(row: &MySqlRow) -> Result<Self, ApiError> {
        Ok(Self {
            public_id: row.try_get("public_id").map_err(|_| ApiError::Internal)?,
            contact_name: row
                .try_get("contact_name")
                .map_err(|_| ApiError::Internal)?,
            phone_e164: row.try_get("phone_e164").map_err(|_| ApiError::Internal)?,
            details: row.try_get("details").map_err(|_| ApiError::Internal)?,
            status: row.try_get("status").map_err(|_| ApiError::Internal)?,
            created_at: row.try_get("created_at").map_err(|_| ApiError::Internal)?,
            updated_at: row.try_get("updated_at").map_err(|_| ApiError::Internal)?,
            handled_at: row.try_get("handled_at").map_err(|_| ApiError::Internal)?,
            handler_public_id: row
                .try_get("handler_public_id")
                .map_err(|_| ApiError::Internal)?,
            handler_display_name: row
                .try_get("handler_display_name")
                .map_err(|_| ApiError::Internal)?,
        })
    }

    fn json(&self) -> serde_json::Value {
        let handled_by = self.handler_public_id.as_ref().map(|public_id| {
            serde_json::json!({
                "public_id": public_id,
                "display_name": &self.handler_display_name,
            })
        });
        serde_json::json!({
            "public_id": &self.public_id,
            "contact_name": &self.contact_name,
            "phone_e164": &self.phone_e164,
            "details": &self.details,
            "status": &self.status,
            "created_at": self.created_at.to_rfc3339(),
            "updated_at": self.updated_at.to_rfc3339(),
            "handled_at": self.handled_at.as_ref().map(|value| value.to_rfc3339()),
            "handled_by": handled_by,
        })
    }
}

const ADMIN_REQUEST_SELECT: &str = "SELECT r.id, r.public_id, r.contact_name, r.phone_e164, \
     r.details, r.status, r.created_at, r.updated_at, r.handled_at, \
     u.public_id AS handler_public_id, u.display_name AS handler_display_name \
     FROM advertising_requests r LEFT JOIN users u ON u.id = r.handled_by";

fn valid_status(value: &str) -> bool {
    matches!(value, "new" | "in_progress" | "closed")
}

/// Newest-first private pages, with a real keyset independent of public feeds.
pub async fn list_requests(
    pool: &MySqlPool,
    status: Option<&str>,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let status = status.unwrap_or("all");
    if status != "all" && !valid_status(status) {
        return Err(validation("status", "invalid"));
    }
    let anchor = cursor.map(crate::listings::decode_cursor).transpose()?;
    let limit = crate::listings::page_limit(limit_raw);
    let mut sql = format!("{ADMIN_REQUEST_SELECT} WHERE 1 = 1");
    if status != "all" {
        sql.push_str(" AND r.status = ?");
    }
    if anchor.is_some() {
        sql.push_str(" AND (r.created_at, r.id) < (?, ?)");
    }
    sql.push_str(" ORDER BY r.created_at DESC, r.id DESC LIMIT ?");
    let mut query = sqlx::query(&sql);
    if status != "all" {
        query = query.bind(status);
    }
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
        .map(|row| AdminRequestRecord::from_row(row).map(|record| record.json()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({
        "data": data,
        "page": { "next_cursor": next_cursor, "has_more": has_more },
    }))
}

async fn fetch_admin_request(
    pool: &MySqlPool,
    public_id: &str,
) -> Result<serde_json::Value, ApiError> {
    let row = sqlx::query(&format!("{ADMIN_REQUEST_SELECT} WHERE r.public_id = ?"))
        .bind(public_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    Ok(serde_json::json!({ "request": AdminRequestRecord::from_row(&row)?.json() }))
}

/// Any explicit workflow transition is allowed; a same-status retry is a no-op
/// and does not manufacture another audit row.
pub async fn update_request(
    pool: &MySqlPool,
    admin_id: u64,
    public_id: &str,
    status: Option<String>,
) -> Result<serde_json::Value, ApiError> {
    let status = status.unwrap_or_default();
    if !valid_status(&status) {
        return Err(validation("status", "invalid"));
    }
    let now = Utc::now();
    let mut tx = pool.begin().await.map_err(crate::error::database_error)?;
    let row =
        sqlx::query("SELECT id, status FROM advertising_requests WHERE public_id = ? FOR UPDATE")
            .bind(public_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(crate::error::database_error)?
            .ok_or(ApiError::NotFound)?;
    let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
    let previous: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    if previous != status {
        let (handled_by, handled_at) = if status == "new" {
            (None, None)
        } else {
            (Some(admin_id), Some(now))
        };
        sqlx::query(
            "UPDATE advertising_requests SET status = ?, handled_by = ?, handled_at = ?, \
             updated_at = ? WHERE id = ?",
        )
        .bind(&status)
        .bind(handled_by)
        .bind(handled_at)
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(crate::error::database_error)?;
        crate::admin_users::audit_in_tx(
            &mut tx,
            admin_id,
            "advertising.request.status",
            "advertising_request",
            id,
            None,
            Some(&serde_json::json!({ "public_id": public_id, "from": previous, "to": status })),
        )
        .await?;
    }
    tx.commit().await.map_err(crate::error::database_error)?;
    fetch_admin_request(pool, public_id).await
}

#[cfg(test)]
mod tests {
    use super::{valid_destination, valid_submission_id};

    #[test]
    fn destinations_are_plain_safe_links() {
        assert!(valid_destination("/advertising"));
        assert!(valid_destination("/advertising#request-form"));
        assert!(valid_destination("https://example.com/path?q=1"));
        for invalid in [
            "http://example.com",
            "//example.com",
            "javascript:alert(1)",
            "https://user@example.com",
            "https://",
            "https://example.com:not-a-port",
            "https://example.com:70000",
            "https://bad_host.example",
            "https://example.com/a b",
        ] {
            assert!(!valid_destination(invalid), "{invalid}");
        }
    }

    #[test]
    fn submission_ids_are_uuid_shaped() {
        assert!(valid_submission_id("123e4567-e89b-42d3-a456-426614174000"));
        assert!(!valid_submission_id("anonymous-viewer-id"));
        assert!(!valid_submission_id("123e4567-e89b-42d3-a456-42661417400g"));
    }
}
