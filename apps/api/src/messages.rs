//! Chat message writes, keyset pages, and high-water Read receipts.
//!
//! Every mutation runs in one transaction with the shared lock order from
//! `conversations`: canonical user rows (ascending id), then the conversation
//! row. Unique `(conversation_id, sender_id, client_message_id)` is the real
//! idempotency boundary; the lookup below only turns it into a friendly replay.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::mysql::MySqlRow;
use sqlx::{MySqlPool, Row};

use crate::config::Config;
use crate::conversations::{self, validation, MESSAGE_SELECT};
use crate::error::ApiError;
use crate::ratelimit::RateLimiter;

pub const MESSAGE_DEFAULT_LIMIT: usize = 30;
pub const MESSAGE_MAX_LIMIT: usize = 50;
const MAX_BODY_CHARS: usize = 1000;
/// The established operational rule (comment rescan): three retries after the
/// first attempt, then the retryable conflict becomes a plain 500.
const MAX_RETRIES: u8 = 3;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendBody {
    pub client_message_id: Option<String>,
    pub body: Option<String>,
    pub listing_slug: Option<String>,
}

/// A send request that has passed every field-level rule. Nothing is spent or
/// written before this exists.
pub struct ValidatedSend {
    pub client_message_id: String,
    pub body: String,
    pub listing_slug: Option<String>,
}

/// Body of `POST /conversations/{public_id}/read`. Unknown fields are rejected
/// so a client cannot smuggle a second claim into the receipt.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadBody {
    pub through_message_id: Option<String>,
}

impl ReadBody {
    pub fn validate(self) -> Result<String, ApiError> {
        match self.through_message_id {
            Some(raw) if !raw.trim().is_empty() => Ok(raw.trim().to_string()),
            _ => Err(validation("through_message_id", "required")),
        }
    }
}

/// Canonical UUID shape (8-4-4-4-12 hex). No UUID crate: the client owns the
/// value, the server only needs it to be canonical and comparable.
fn canonical_uuid(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    if bytes.len() != 36 {
        return None;
    }
    for (index, byte) in bytes.iter().enumerate() {
        let hyphen_slot = matches!(index, 8 | 13 | 18 | 23);
        if hyphen_slot {
            if *byte != b'-' {
                return None;
            }
        } else if !byte.is_ascii_hexdigit() {
            return None;
        }
    }
    Some(raw.to_ascii_lowercase())
}

impl SendBody {
    /// Trim, then enforce 1–1000 Unicode scalar characters, plain text, and a
    /// canonical client ID. Unknown fields never reach this point: serde
    /// rejects them as `invalid_json`.
    pub fn validate(self) -> Result<ValidatedSend, ApiError> {
        let client_message_id = match self.client_message_id {
            Some(raw) => {
                canonical_uuid(&raw).ok_or_else(|| validation("client_message_id", "invalid"))?
            }
            None => return Err(validation("client_message_id", "required")),
        };
        let body = self
            .body
            .ok_or_else(|| validation("body", "required"))?
            .trim()
            .to_string();
        if body.is_empty() {
            return Err(validation("body", "required"));
        }
        if body.chars().count() > MAX_BODY_CHARS {
            return Err(validation("body", "too_long"));
        }
        if body.chars().any(|c| c.is_control() && c != '\n') {
            return Err(validation("body", "invalid_characters"));
        }
        let listing_slug = match self.listing_slug {
            Some(raw) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    return Err(validation("listing_slug", "invalid"));
                }
                Some(trimmed.to_string())
            }
            None => None,
        };
        Ok(ValidatedSend {
            client_message_id,
            body,
            listing_slug,
        })
    }
}

/// Spends the validated-send allowance once per HTTP request, even when the
/// transaction is retried for a MariaDB conflict. Replays count; malformed,
/// unauthorized, inactive-participant, and blocked-new-send paths returned
/// before reaching this function.
fn spend_allowance(
    limiter: &RateLimiter,
    config: &Config,
    viewer_id: u64,
    spent: &mut bool,
) -> Result<(), ApiError> {
    if *spent {
        return Ok(());
    }
    limiter
        .hit(
            &format!("message:user:{viewer_id}"),
            config.message_user_limit,
            Duration::from_secs(u64::from(config.message_user_window_secs)),
        )
        .into_result()?;
    *spent = true;
    Ok(())
}

fn message_limit(raw: Option<&str>) -> Result<usize, ApiError> {
    match raw {
        None => Ok(MESSAGE_DEFAULT_LIMIT),
        Some(value) => value
            .parse::<usize>()
            .map(|limit| limit.clamp(1, MESSAGE_MAX_LIMIT))
            .map_err(|_| validation("limit", "invalid_limit")),
    }
}

fn message_cursor(row: &MySqlRow) -> Result<String, ApiError> {
    let public_id: String = row
        .try_get("message_public_id")
        .map_err(|_| ApiError::Internal)?;
    Ok(format!("m:{public_id}"))
}

/// Renders one message for its participant. `seen` is present only on the
/// caller's own messages and comes from the other high-water mark, never from
/// an optimistic client guess.
fn message_json(
    row: &MySqlRow,
    viewer_id: u64,
    other_read_id: Option<u64>,
) -> Result<serde_json::Value, ApiError> {
    let message_id: u64 = row.try_get("message_id").map_err(|_| ApiError::Internal)?;
    let sender_id: u64 = row.try_get("sender_id").map_err(|_| ApiError::Internal)?;
    let created_at: DateTime<Utc> = row
        .try_get("message_created_at")
        .map_err(|_| ApiError::Internal)?;
    let mut value = serde_json::json!({
        "public_id": row.try_get::<String, _>("message_public_id").map_err(|_| ApiError::Internal)?,
        "sender_public_id": row.try_get::<String, _>("sender_public_id").map_err(|_| ApiError::Internal)?,
        "body": row.try_get::<String, _>("message_body").map_err(|_| ApiError::Internal)?,
        "created_at": created_at.to_rfc3339(),
        "listing": conversations::listing_reference(row)?,
    });
    if sender_id == viewer_id {
        value["seen"] = serde_json::json!(other_read_id.is_some_and(|read| read >= message_id));
    }
    Ok(value)
}

fn message_query(extra: &str) -> String {
    format!(
        "SELECT {MESSAGE_SELECT}, su.public_id AS sender_public_id, m.client_message_id \
         FROM messages m \
         JOIN users su ON su.id = m.sender_id \
         LEFT JOIN listings l ON l.id = m.listing_id {extra}"
    )
}

/// `POST /conversations/{public_id}/messages` — validated, idempotent send.
pub async fn send(
    pool: &MySqlPool,
    limiter: &RateLimiter,
    config: &Config,
    viewer_id: u64,
    conversation_public_id: &str,
    input: &ValidatedSend,
) -> Result<serde_json::Value, ApiError> {
    let mut remaining_retries = MAX_RETRIES;
    let mut spent = false;
    loop {
        match send_once(
            pool,
            limiter,
            config,
            viewer_id,
            conversation_public_id,
            input,
            &mut spent,
        )
        .await
        {
            Err(ApiError::RetryableDatabase) if remaining_retries > 0 => {
                remaining_retries -= 1;
            }
            Err(ApiError::RetryableDatabase) => return Err(ApiError::Internal),
            result => return result,
        }
    }
}

async fn send_once(
    pool: &MySqlPool,
    limiter: &RateLimiter,
    config: &Config,
    viewer_id: u64,
    conversation_public_id: &str,
    input: &ValidatedSend,
    spent: &mut bool,
) -> Result<serde_json::Value, ApiError> {
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    // 1. Non-locking resolve only to learn the pair; authorization repeats
    //    under the locks below.
    let pre = sqlx::query(
        "SELECT id, user_low_id, user_high_id FROM conversations \
         WHERE public_id = ? AND (user_low_id = ? OR user_high_id = ?)",
    )
    .bind(conversation_public_id)
    .bind(viewer_id)
    .bind(viewer_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(crate::error::database_error)?
    .ok_or(ApiError::NotFound)?;
    let conversation_id: u64 = pre.try_get("id").map_err(|_| ApiError::Internal)?;
    let low: u64 = pre.try_get("user_low_id").map_err(|_| ApiError::Internal)?;
    let high: u64 = pre
        .try_get("user_high_id")
        .map_err(|_| ApiError::Internal)?;
    let other_id = if low == viewer_id { high } else { low };

    // 2. User locks in ascending id order, then the conversation row.
    crate::db::lock_user_pair(&mut tx, viewer_id, other_id).await?;
    let conversation =
        conversations::lock_conversation(&mut tx, viewer_id, conversation_id).await?;
    let other_active = conversations::participant_is_active(&mut tx, other_id).await?;

    // 3. Listing reference: only the other participant's listing is valid, and
    //    a seller can never reference their own listing.
    let listing_id = match &input.listing_slug {
        None => None,
        Some(slug) => {
            let row = sqlx::query("SELECT id, seller_id FROM listings WHERE slug = ?")
                .bind(slug)
                .fetch_optional(&mut *tx)
                .await
                .map_err(crate::error::database_error)?;
            let row = row.ok_or_else(|| validation("listing_slug", "invalid"))?;
            let seller_id: u64 = row.try_get("seller_id").map_err(|_| ApiError::Internal)?;
            if seller_id != other_id {
                return Err(validation("listing_slug", "invalid"));
            }
            Some(
                row.try_get::<u64, _>("id")
                    .map_err(|_| ApiError::Internal)?,
            )
        }
    };

    // 4. Idempotency before any new-send rejection: a confirmed receipt stays
    //    replayable even after a later block.
    let stored_sql = message_query(
        "WHERE m.conversation_id = ? AND m.sender_id = ? AND m.client_message_id = ?",
    );
    let stored = sqlx::query(&stored_sql)
        .bind(conversation_id)
        .bind(viewer_id)
        .bind(&input.client_message_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::error::database_error)?;
    let (_, other_read_id) =
        conversations::read_state_tx(&mut tx, conversation.id, viewer_id, other_id).await?;
    if let Some(stored) = stored {
        spend_allowance(limiter, config, viewer_id, spent)?;
        let stored_body: String = stored
            .try_get("message_body")
            .map_err(|_| ApiError::Internal)?;
        let stored_listing: Option<u64> = stored
            .try_get("listing_id")
            .map_err(|_| ApiError::Internal)?;
        if stored_body != input.body || stored_listing != listing_id {
            return Err(validation("client_message_id", "conflict"));
        }
        let message = message_json(&stored, viewer_id, other_read_id)?;
        tx.commit().await.map_err(|_| ApiError::Internal)?;
        return Ok(serde_json::json!({
            "message": message,
            "conversation": conversations::metadata_by_id(pool, viewer_id, conversation_id).await?,
        }));
    }

    // 5. Only a genuinely new message is subject to account and block state.
    if !other_active {
        return Err(ApiError::Forbidden);
    }
    if conversations::has_either_block(&mut tx, low, high).await? {
        return Err(ApiError::ConversationBlocked);
    }

    // 6. Spend the validated-send allowance once, before any write.
    spend_allowance(limiter, config, viewer_id, spent)?;

    let public_id = ulid::Ulid::new().to_string();
    let now = Utc::now();
    let inserted = sqlx::query(
        "INSERT INTO messages (public_id, conversation_id, sender_id, listing_id, \
         client_message_id, body, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&public_id)
    .bind(conversation_id)
    .bind(viewer_id)
    .bind(listing_id)
    .bind(&input.client_message_id)
    .bind(&input.body)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(crate::error::database_error)?;
    let message_id = inserted.last_insert_id();
    sqlx::query(
        "UPDATE conversations SET last_message_id = ?, last_message_at = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(message_id)
    .bind(now)
    .bind(now)
    .bind(conversation_id)
    .execute(&mut *tx)
    .await
    .map_err(crate::error::database_error)?;

    let inserted_sql = message_query("WHERE m.id = ?");
    let row = sqlx::query(&inserted_sql)
        .bind(message_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::error::database_error)?;
    let message = message_json(&row, viewer_id, other_read_id)?;
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    Ok(serde_json::json!({
        "message": message,
        "conversation": conversations::metadata_by_id(pool, viewer_id, conversation_id).await?,
    }))
}

/// `GET /conversations/{public_id}/messages` — keyset pages + read state.
pub async fn list(
    pool: &MySqlPool,
    viewer_id: u64,
    public_id: &str,
    before: Option<&str>,
    after: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    if before.is_some() && after.is_some() {
        return Err(validation("cursor", "mutually_exclusive"));
    }
    let limit = message_limit(limit_raw)?;
    let conversation = conversations::participant_conversation(pool, viewer_id, public_id).await?;
    let other_id = conversation.other_id(viewer_id);
    let (viewer_read_id, other_read_id) =
        conversations::read_state(pool, conversation.id, viewer_id, other_id).await?;

    // Cursors carry only the message public id; the server resolves it inside
    // the authorized conversation, so a foreign anchor is a 400, never a peek.
    let anchor = match (before, after) {
        (Some(raw), _) => Some(resolve_cursor(pool, conversation.id, raw).await?),
        (None, Some(raw)) => Some(resolve_cursor(pool, conversation.id, raw).await?),
        (None, None) => None,
    };
    let forward = after.is_some();

    let mut sql = message_query("WHERE m.conversation_id = ?");
    if forward {
        sql.push_str(" AND m.id > ? ORDER BY m.id ASC LIMIT ?");
    } else if anchor.is_some() {
        sql.push_str(" AND m.id < ? ORDER BY m.id DESC LIMIT ?");
    } else {
        sql.push_str(" ORDER BY m.id DESC LIMIT ?");
    }
    let mut query = sqlx::query(&sql).bind(conversation.id);
    if let Some(anchor) = anchor {
        query = query.bind(anchor);
    }
    query = query.bind((limit + 1) as i64);
    let mut rows = query
        .fetch_all(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    // Newest-first scans are reversed so `data` is always oldest → newest.
    if !forward {
        rows.reverse();
    }

    let data = rows
        .iter()
        .map(|row| message_json(row, viewer_id, other_read_id))
        .collect::<Result<Vec<_>, ApiError>>()?;

    let older_cursor = if let Some(first) = rows.first() {
        if forward || has_more {
            Some(message_cursor(first)?)
        } else {
            None
        }
    } else {
        None
    };
    let newer_cursor = match rows.last() {
        Some(last) => Some(message_cursor(last)?),
        None => None,
    };
    let has_more_older = if forward { !rows.is_empty() } else { has_more };
    let has_more_newer = if forward {
        has_more
    } else {
        before.is_some() && !rows.is_empty()
    };

    let viewer_read_through =
        conversations::message_public_id(pool, conversation.id, viewer_read_id).await?;
    let other_read_through =
        conversations::message_public_id(pool, conversation.id, other_read_id).await?;
    Ok(serde_json::json!({
        "data": data,
        "page": {
            "older_cursor": older_cursor,
            "newer_cursor": newer_cursor,
            "has_more_older": has_more_older,
            "has_more_newer": has_more_newer,
        },
        "conversation": conversations::metadata_by_id(pool, viewer_id, conversation.id).await?,
        "read_state": {
            "viewer_read_through": viewer_read_through,
            "other_read_through": other_read_through,
        },
    }))
}

async fn resolve_cursor(
    pool: &MySqlPool,
    conversation_id: u64,
    raw: &str,
) -> Result<u64, ApiError> {
    let public = raw
        .strip_prefix("m:")
        .ok_or_else(|| validation("cursor", "invalid_cursor"))?;
    let id: Option<u64> =
        sqlx::query_scalar("SELECT id FROM messages WHERE public_id = ? AND conversation_id = ?")
            .bind(public)
            .bind(conversation_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| ApiError::Internal)?;
    id.ok_or_else(|| validation("cursor", "invalid_cursor"))
}

/// `POST /conversations/{public_id}/read` — forward-only high-water mark.
///
/// The caller supplies only the latest message it actually rendered. A newer
/// message inserted afterwards stays above the mark, so a racing arrival can
/// never be falsely Seen.
pub async fn mark_read(
    pool: &MySqlPool,
    viewer_id: u64,
    public_id: &str,
    through_message_id: &str,
) -> Result<serde_json::Value, ApiError> {
    let conversation = conversations::participant_conversation(pool, viewer_id, public_id).await?;
    let other_id = conversation.other_id(viewer_id);
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    let target: Option<u64> =
        sqlx::query_scalar("SELECT id FROM messages WHERE public_id = ? AND conversation_id = ?")
            .bind(through_message_id)
            .bind(conversation.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| ApiError::Internal)?;
    let target = target.ok_or_else(|| validation("through_message_id", "invalid"))?;
    conversations::lock_conversation(&mut tx, viewer_id, conversation.id).await?;
    let current: Option<u64> = sqlx::query_scalar::<_, Option<u64>>(
        "SELECT last_read_message_id FROM conversation_reads \
         WHERE conversation_id = ? AND user_id = ? FOR UPDATE",
    )
    .bind(conversation.id)
    .bind(viewer_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?
    .flatten();
    if current.is_none_or(|read| target > read) {
        sqlx::query(
            "UPDATE conversation_reads SET last_read_message_id = ?, last_read_at = NOW(3) \
             WHERE conversation_id = ? AND user_id = ?",
        )
        .bind(target)
        .bind(conversation.id)
        .bind(viewer_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
        // Read advances synchronization but never recency ordering.
        sqlx::query("UPDATE conversations SET updated_at = NOW(3) WHERE id = ?")
            .bind(conversation.id)
            .execute(&mut *tx)
            .await
            .map_err(|_| ApiError::Internal)?;
    }
    tx.commit().await.map_err(|_| ApiError::Internal)?;

    let (viewer_read_id, _) =
        conversations::read_state(pool, conversation.id, viewer_id, other_id).await?;
    let read_through =
        conversations::message_public_id(pool, conversation.id, viewer_read_id).await?;
    let read_at: Option<DateTime<Utc>> = sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
        "SELECT last_read_at FROM conversation_reads WHERE conversation_id = ? AND user_id = ?",
    )
    .bind(conversation.id)
    .bind(viewer_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?
    .flatten();
    let unread =
        conversations::unread_count(pool, conversation.id, viewer_id, viewer_read_id).await?;
    Ok(serde_json::json!({
        "read_through": read_through,
        "read_at": read_at.map(|value| value.to_rfc3339()),
        "unread_count": unread,
    }))
}

#[cfg(test)]
mod tests {
    use super::{canonical_uuid, message_limit, SendBody, MESSAGE_DEFAULT_LIMIT};

    fn body(body: &str) -> SendBody {
        SendBody {
            client_message_id: Some("123e4567-e89b-42d3-a456-426614174000".to_string()),
            body: Some(body.to_string()),
            listing_slug: None,
        }
    }

    #[test]
    fn uuid_shape_is_canonical_and_normalized() {
        assert_eq!(
            canonical_uuid("123E4567-E89B-42D3-A456-426614174000").as_deref(),
            Some("123e4567-e89b-42d3-a456-426614174000")
        );
        assert!(canonical_uuid("123e4567e89b42d3a456426614174000").is_none());
        assert!(canonical_uuid("123e4567-e89b-42d3-a456-42661417400z").is_none());
        assert!(canonical_uuid("123e4567e89b-42d3-a456-426614174000").is_none());
    }

    #[test]
    fn body_validation_counts_scalars_and_keeps_newlines() {
        let persian = "س".repeat(1000);
        assert!(body(&persian).validate().is_ok());
        assert!(body(&"س".repeat(1001)).validate().is_err());
        assert!(body("خط اول\nخط دوم").validate().is_ok());
        assert!(body("   ").validate().is_err());
        assert!(body("a\rb").validate().is_err());
        assert!(body("a\tb").validate().is_err());
        assert!(body("a\0b").validate().is_err());
    }

    #[test]
    fn missing_client_id_is_required() {
        let mut request = body("سلام");
        request.client_message_id = None;
        assert!(request.validate().is_err());
    }

    #[test]
    fn message_limit_defaults_and_clamps() {
        assert_eq!(message_limit(None).expect("default"), MESSAGE_DEFAULT_LIMIT);
        assert_eq!(message_limit(Some("0")).expect("clamped"), 1);
        assert_eq!(message_limit(Some("999")).expect("clamped"), 50);
        assert!(message_limit(Some("x")).is_err());
    }
}
