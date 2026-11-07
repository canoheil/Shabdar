//! Direct two-user conversations (approved spec 2026-11-04).
//!
//! One canonical unordered user pair owns exactly one conversation. A listing
//! is only an entry point and an optional per-message reference: it never owns
//! the thread, and its later lifecycle never freezes an existing conversation.
//!
//! Lock order for every chat mutation is: canonical user rows in ascending id
//! order (`db::lock_user_pair`), then the conversation row. Directed blocks use
//! the same user locks, which is what makes "message first" and "block first"
//! the only two observable orders.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use sqlx::mysql::MySqlRow;
use sqlx::{MySql, MySqlConnection, MySqlPool, Row, Transaction};

use crate::error::ApiError;

/// Inbox page defaults (spec §5.2).
pub const INBOX_DEFAULT_LIMIT: usize = 20;
pub const INBOX_MAX_LIMIT: usize = 50;
/// Compact preview length in Unicode scalar characters.
const PREVIEW_CHARS: usize = 160;

pub(crate) fn validation(field: &str, reason: &str) -> ApiError {
    let mut fields = BTreeMap::new();
    fields.insert(field.to_string(), reason.to_string());
    ApiError::Validation { fields }
}

/// Public metadata of one conversation as seen by its viewer.
pub(crate) struct ConversationRow {
    pub id: u64,
    pub public_id: String,
    pub user_low_id: u64,
    pub user_high_id: u64,
    pub created_at: DateTime<Utc>,
    pub last_message_at: Option<DateTime<Utc>>,
}

impl ConversationRow {
    pub(crate) fn other_id(&self, viewer_id: u64) -> u64 {
        if self.user_low_id == viewer_id {
            self.user_high_id
        } else {
            self.user_low_id
        }
    }
}

fn conversation_row(row: &MySqlRow) -> Result<ConversationRow, ApiError> {
    Ok(ConversationRow {
        id: row.try_get("id").map_err(|_| ApiError::Internal)?,
        public_id: row.try_get("public_id").map_err(|_| ApiError::Internal)?,
        user_low_id: row.try_get("user_low_id").map_err(|_| ApiError::Internal)?,
        user_high_id: row
            .try_get("user_high_id")
            .map_err(|_| ApiError::Internal)?,
        created_at: row.try_get("created_at").map_err(|_| ApiError::Internal)?,
        last_message_at: row
            .try_get("last_message_at")
            .map_err(|_| ApiError::Internal)?,
    })
}

/// Resolves a public conversation id for a participant. A non-participant and
/// an unknown conversation are deliberately indistinguishable (404).
pub(crate) async fn participant_conversation(
    pool: &MySqlPool,
    viewer_id: u64,
    public_id: &str,
) -> Result<ConversationRow, ApiError> {
    let row = sqlx::query(
        "SELECT id, public_id, user_low_id, user_high_id, created_at, last_message_at \
         FROM conversations \
         WHERE public_id = ? AND (user_low_id = ? OR user_high_id = ?)",
    )
    .bind(public_id)
    .bind(viewer_id)
    .bind(viewer_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    conversation_row(&row)
}

/// True when either directed block exists for the pair.
///
/// This is a **locking** read on purpose. MariaDB's default REPEATABLE READ
/// fixes a consistent snapshot at the transaction's first read; a send that
/// waited on the pair locks would otherwise still see the pre-block snapshot
/// and slip a message in after a committed block. A locking read always reads
/// the latest committed rows, and every caller already holds the pair locks, so
/// the lock order stays pair-users → blocks → conversation.
pub async fn has_either_block(
    tx: &mut Transaction<'_, MySql>,
    low: u64,
    high: u64,
) -> Result<bool, ApiError> {
    let blocked: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM blocks \
         WHERE (blocker_id = ? AND blocked_id = ?) OR (blocker_id = ? AND blocked_id = ?) \
         FOR UPDATE LIMIT 1",
    )
    .bind(low)
    .bind(high)
    .bind(high)
    .bind(low)
    .fetch_optional(&mut **tx)
    .await
    .map_err(crate::error::database_error)?;
    Ok(blocked.is_some())
}

/// Advances the pair conversation's synchronization clock for a real change.
/// `changed == false` (an idempotent block/unblock) must never move it, so
/// Inbox polling does not report phantom updates. `last_message_at` is never
/// touched here: blocking must not reorder the Inbox.
pub async fn sync_pair_block_state(
    tx: &mut Transaction<'_, MySql>,
    low: u64,
    high: u64,
    changed: bool,
) -> Result<(), ApiError> {
    if !changed {
        return Ok(());
    }
    sqlx::query(
        "UPDATE conversations SET updated_at = NOW(3) \
         WHERE user_low_id = ? AND user_high_id = ?",
    )
    .bind(low)
    .bind(high)
    .execute(&mut **tx)
    .await
    .map_err(crate::error::database_error)?;
    Ok(())
}

/// Whether a participant may send: active status and no deletion stamp. Read
/// under the pair locks so a concurrent suspension cannot slip in between.
pub(crate) async fn participant_is_active(
    conn: &mut MySqlConnection,
    user_id: u64,
) -> Result<bool, ApiError> {
    let row = sqlx::query("SELECT status, deleted_at FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(crate::error::database_error)?
        .ok_or(ApiError::NotFound)?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    let deleted_at: Option<DateTime<Utc>> =
        row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
    Ok(status == "active" && deleted_at.is_none())
}

/// A listing resolved for a contact entry, with the public-visibility facts the
/// start endpoint must revalidate under the pair locks.
pub(crate) struct ContactListing {
    pub id: u64,
    pub seller_id: u64,
}

/// Resolves a listing for `POST /listings/{slug}/conversations`.
///
/// Unknown, non-active, and image-review-pending listings are the same 404 as
/// Detail; a removed or soft-deleted listing is 410. Callers run this twice:
/// once to learn the seller before locking, then again under the pair locks.
pub(crate) async fn contact_listing(
    tx: &mut Transaction<'_, MySql>,
    slug: &str,
) -> Result<ContactListing, ApiError> {
    let row = sqlx::query(
        "SELECT id, seller_id, status, deleted_at, \
         image_reviewed_at IS NOT NULL AS image_reviewed, \
         EXISTS(SELECT 1 FROM listing_images i WHERE i.listing_id = listings.id) AS has_images \
         FROM listings WHERE slug = ?",
    )
    .bind(slug)
    .fetch_optional(&mut **tx)
    .await
    .map_err(crate::error::database_error)?
    .ok_or(ApiError::NotFound)?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    let deleted_at: Option<DateTime<Utc>> =
        row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
    if status == "removed" || deleted_at.is_some() {
        return Err(ApiError::Gone);
    }
    if status != "active" {
        return Err(ApiError::NotFound);
    }
    crate::listings::check_public_image_review(&row)?;
    Ok(ContactListing {
        id: row.try_get("id").map_err(|_| ApiError::Internal)?,
        seller_id: row.try_get("seller_id").map_err(|_| ApiError::Internal)?,
    })
}

/// Resolves a conversation row for a participant inside a transaction, taking
/// the row lock that serializes send/read against each other.
pub(crate) async fn lock_conversation(
    tx: &mut Transaction<'_, MySql>,
    viewer_id: u64,
    conversation_id: u64,
) -> Result<ConversationRow, ApiError> {
    let row = sqlx::query(
        "SELECT id, public_id, user_low_id, user_high_id, created_at, last_message_at \
         FROM conversations \
         WHERE id = ? AND (user_low_id = ? OR user_high_id = ?) FOR UPDATE",
    )
    .bind(conversation_id)
    .bind(viewer_id)
    .bind(viewer_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(crate::error::database_error)?
    .ok_or(ApiError::NotFound)?;
    conversation_row(&row)
}

/// `POST /listings/{slug}/conversations` — find or create the canonical pair.
///
/// Repeated starts return the same conversation and never create a message, a
/// system event, or a timestamp change.
pub async fn start(
    pool: &MySqlPool,
    viewer_id: u64,
    slug: &str,
) -> Result<serde_json::Value, ApiError> {
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    // First read (no lock): we need the seller to know which pair to lock.
    let initial = contact_listing(&mut tx, slug).await?;
    if initial.seller_id == viewer_id {
        return Err(ApiError::Forbidden);
    }
    crate::db::lock_user_pair(&mut tx, viewer_id, initial.seller_id).await?;
    // Revalidate under the locks: the listing may have been removed or its
    // image review withdrawn while we were waiting.
    let listing = contact_listing(&mut tx, slug).await?;
    if listing.id != initial.id || listing.seller_id != initial.seller_id {
        return Err(ApiError::NotFound);
    }
    if !participant_is_active(&mut tx, listing.seller_id).await? {
        return Err(ApiError::Forbidden);
    }

    let (low, high) = crate::db::canonical_pair(viewer_id, listing.seller_id);
    if has_either_block(&mut tx, low, high).await? {
        return Err(ApiError::ConversationBlocked);
    }

    let existing: Option<u64> = sqlx::query_scalar(
        "SELECT id FROM conversations WHERE user_low_id = ? AND user_high_id = ?",
    )
    .bind(low)
    .bind(high)
    .fetch_optional(&mut *tx)
    .await
    .map_err(crate::error::database_error)?;

    let conversation_id = match existing {
        Some(id) => id,
        None => {
            let public_id = ulid::Ulid::new().to_string();
            let inserted = sqlx::query(
                "INSERT INTO conversations \
                 (public_id, user_low_id, user_high_id, initiator_id, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, NOW(3), NOW(3))",
            )
            .bind(&public_id)
            .bind(low)
            .bind(high)
            .bind(viewer_id)
            .execute(&mut *tx)
            .await
            .map_err(crate::error::database_error)?;
            let id = inserted.last_insert_id();
            for user_id in [low, high] {
                sqlx::query(
                    "INSERT INTO conversation_reads (conversation_id, user_id) VALUES (?, ?)",
                )
                .bind(id)
                .bind(user_id)
                .execute(&mut *tx)
                .await
                .map_err(crate::error::database_error)?;
            }
            id
        }
    };
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    metadata_by_id(pool, viewer_id, conversation_id).await
}

/// `GET /conversations/{public_id}` — participant-specific metadata.
pub async fn metadata(
    pool: &MySqlPool,
    viewer_id: u64,
    public_id: &str,
) -> Result<serde_json::Value, ApiError> {
    let conversation = participant_conversation(pool, viewer_id, public_id).await?;
    metadata_from(pool, viewer_id, &conversation).await
}

pub async fn metadata_by_id(
    pool: &MySqlPool,
    viewer_id: u64,
    conversation_id: u64,
) -> Result<serde_json::Value, ApiError> {
    let row = sqlx::query(
        "SELECT id, public_id, user_low_id, user_high_id, created_at, last_message_at \
         FROM conversations WHERE id = ? AND (user_low_id = ? OR user_high_id = ?)",
    )
    .bind(conversation_id)
    .bind(viewer_id)
    .bind(viewer_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    metadata_from(pool, viewer_id, &conversation_row(&row)?).await
}

async fn metadata_from(
    pool: &MySqlPool,
    viewer_id: u64,
    conversation: &ConversationRow,
) -> Result<serde_json::Value, ApiError> {
    let other_id = conversation.other_id(viewer_id);
    let other = sqlx::query(
        "SELECT u.public_id, u.display_name, u.status, u.deleted_at, \
         EXISTS(SELECT 1 FROM blocks b WHERE b.blocker_id = ? AND b.blocked_id = ?) \
           AS viewer_blocked_other, \
         EXISTS(SELECT 1 FROM blocks b WHERE b.blocker_id = ? AND b.blocked_id = ?) \
           AS other_blocked_viewer \
         FROM users u WHERE u.id = ?",
    )
    .bind(viewer_id)
    .bind(other_id)
    .bind(other_id)
    .bind(viewer_id)
    .bind(other_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    let status: String = other.try_get("status").map_err(|_| ApiError::Internal)?;
    let deleted_at: Option<DateTime<Utc>> = other
        .try_get("deleted_at")
        .map_err(|_| ApiError::Internal)?;
    let viewer_blocked: bool = other
        .try_get("viewer_blocked_other")
        .map_err(|_| ApiError::Internal)?;
    let other_blocked: bool = other
        .try_get("other_blocked_viewer")
        .map_err(|_| ApiError::Internal)?;
    let active_account = status == "active" && deleted_at.is_none();
    Ok(serde_json::json!({
        "public_id": conversation.public_id,
        "other_party": {
            "public_id": other.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
            "display_name": other.try_get::<Option<String>, _>("display_name").map_err(|_| ApiError::Internal)?,
        },
        "can_message": active_account && !viewer_blocked && !other_blocked,
        "viewer_blocked_other": viewer_blocked,
        "last_message_at": conversation.last_message_at.map(|value| value.to_rfc3339()),
        "created_at": conversation.created_at.to_rfc3339(),
    }))
}

/// Reads both high-water marks of a conversation as internal message ids.
///
/// The connection-bound form exists so a transaction never has to borrow a
/// second pool connection while it holds one (the pool is deliberately tiny).
pub(crate) async fn read_state_conn(
    conn: &mut MySqlConnection,
    conversation_id: u64,
    viewer_id: u64,
    other_id: u64,
) -> Result<(Option<u64>, Option<u64>), ApiError> {
    let rows = sqlx::query(
        "SELECT user_id, last_read_message_id FROM conversation_reads WHERE conversation_id = ?",
    )
    .bind(conversation_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|_| ApiError::Internal)?;
    let mut viewer = None;
    let mut other = None;
    for row in rows {
        let user_id: u64 = row.try_get("user_id").map_err(|_| ApiError::Internal)?;
        let read_id: Option<u64> = row
            .try_get("last_read_message_id")
            .map_err(|_| ApiError::Internal)?;
        if user_id == viewer_id {
            viewer = read_id;
        } else if user_id == other_id {
            other = read_id;
        }
    }
    Ok((viewer, other))
}

pub(crate) async fn read_state(
    pool: &MySqlPool,
    conversation_id: u64,
    viewer_id: u64,
    other_id: u64,
) -> Result<(Option<u64>, Option<u64>), ApiError> {
    let mut conn = pool.acquire().await.map_err(|_| ApiError::Internal)?;
    read_state_conn(&mut conn, conversation_id, viewer_id, other_id).await
}

pub(crate) async fn read_state_tx(
    tx: &mut Transaction<'_, MySql>,
    conversation_id: u64,
    viewer_id: u64,
    other_id: u64,
) -> Result<(Option<u64>, Option<u64>), ApiError> {
    read_state_conn(&mut *tx, conversation_id, viewer_id, other_id).await
}

/// Resolves an internal message id to its public id inside a conversation.
pub(crate) async fn message_public_id(
    pool: &MySqlPool,
    conversation_id: u64,
    message_id: Option<u64>,
) -> Result<Option<String>, ApiError> {
    match message_id {
        None => Ok(None),
        Some(id) => {
            let public_id: Option<String> = sqlx::query_scalar(
                "SELECT public_id FROM messages WHERE id = ? AND conversation_id = ?",
            )
            .bind(id)
            .bind(conversation_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| ApiError::Internal)?;
            Ok(public_id)
        }
    }
}

/// Counts incoming messages above the viewer's high-water mark. Own messages
/// never contribute to a caller's unread count.
pub(crate) async fn unread_count(
    pool: &MySqlPool,
    conversation_id: u64,
    viewer_id: u64,
    viewer_read_id: Option<u64>,
) -> Result<u64, ApiError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages \
         WHERE conversation_id = ? AND sender_id <> ? AND id > ?",
    )
    .bind(conversation_id)
    .bind(viewer_id)
    .bind(viewer_read_id.unwrap_or(0))
    .fetch_one(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    Ok(u64::try_from(count).unwrap_or(0))
}

fn inbox_limit(raw: Option<&str>) -> Result<usize, ApiError> {
    match raw {
        None => Ok(INBOX_DEFAULT_LIMIT),
        Some(value) => value
            .parse::<usize>()
            .map(|limit| limit.clamp(1, INBOX_MAX_LIMIT))
            .map_err(|_| validation("limit", "invalid_limit")),
    }
}

fn preview(body: &str) -> String {
    body.chars().take(PREVIEW_CHARS).collect()
}

/// Message columns plus the listing-reference projection shared by the thread
/// pages and the Inbox preview. `available` is the same public-visibility
/// predicate Detail uses, so a card never links to an unreadable listing.
pub(crate) const MESSAGE_SELECT: &str = "\
    m.id AS message_id, m.public_id AS message_public_id, m.sender_id, \
    m.body AS message_body, m.created_at AS message_created_at, m.listing_id, \
    l.slug AS listing_slug, l.title AS listing_title, \
    (l.id IS NOT NULL AND l.status = 'active' AND l.deleted_at IS NULL \
      AND (l.image_reviewed_at IS NOT NULL OR NOT EXISTS( \
           SELECT 1 FROM listing_images i WHERE i.listing_id = l.id))) AS listing_available";

/// Builds the public listing reference (or `None`) without leaking any
/// seller-only or moderation field.
pub(crate) fn listing_reference(row: &MySqlRow) -> Result<Option<serde_json::Value>, ApiError> {
    let listing_id: Option<u64> = row.try_get("listing_id").map_err(|_| ApiError::Internal)?;
    if listing_id.is_none() {
        return Ok(None);
    }
    let slug: Option<String> = row
        .try_get("listing_slug")
        .map_err(|_| ApiError::Internal)?;
    let title: Option<String> = row
        .try_get("listing_title")
        .map_err(|_| ApiError::Internal)?;
    let available: bool = row
        .try_get("listing_available")
        .map_err(|_| ApiError::Internal)?;
    Ok(Some(serde_json::json!({
        "slug": slug,
        "title": title,
        "available": available && slug.is_some(),
    })))
}

fn epoch() -> DateTime<Utc> {
    DateTime::from_timestamp_micros(0).unwrap_or_else(Utc::now)
}

/// `GET /me/conversations` — recency pages plus forward synchronization.
///
/// Two cursors with different jobs:
/// - `older_cursor` (initial/before) is the `(last_message_at,id)` recency key,
///   so a Read never reorders the Inbox.
/// - `sync_cursor` is a server snapshot `(max updated_at, 0)`. Because the
///   forward predicate is strict `>`, the next poll includes every row carrying
///   that same timestamp, so a change made in the same millisecond is never
///   skipped. `next_sync_cursor` is the real last-row `(updated_at,id)` used
///   only to walk the remaining pages of the current scan.
pub async fn inbox(
    pool: &MySqlPool,
    viewer_id: u64,
    before: Option<&str>,
    after: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    if before.is_some() && after.is_some() {
        return Err(validation("cursor", "mutually_exclusive"));
    }
    let limit = inbox_limit(limit_raw)?;
    let forward = after.is_some();

    // Snapshot before scanning: any change after this point is greater than or
    // equal to the boundary and therefore returned by the next poll.
    let max_updated: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT MAX(updated_at) FROM conversations WHERE user_low_id = ? OR user_high_id = ?",
    )
    .bind(viewer_id)
    .bind(viewer_id)
    .fetch_one(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    let sync_cursor = crate::listings::encode_cursor(&max_updated.unwrap_or_else(epoch), 0);

    let mut sql = format!(
        "SELECT c.id, c.public_id, c.user_low_id, c.user_high_id, c.created_at, \
         c.last_message_at, c.updated_at AS conversation_updated_at, \
         {MESSAGE_SELECT}, \
         u.public_id AS other_public_id, u.display_name AS other_display_name, \
         u.status AS other_status, u.deleted_at AS other_deleted_at, \
         su.public_id AS sender_public_id, \
         EXISTS(SELECT 1 FROM blocks b WHERE b.blocker_id = ? AND b.blocked_id = \
           IF(c.user_low_id = ?, c.user_high_id, c.user_low_id)) AS viewer_blocked_other, \
         EXISTS(SELECT 1 FROM blocks b WHERE b.blocker_id = \
           IF(c.user_low_id = ?, c.user_high_id, c.user_low_id) AND b.blocked_id = ?) \
           AS other_blocked_viewer, \
         (SELECT COUNT(*) FROM messages um WHERE um.conversation_id = c.id \
            AND um.sender_id <> ? AND um.id > COALESCE((SELECT last_read_message_id \
              FROM conversation_reads cr WHERE cr.conversation_id = c.id \
                AND cr.user_id = ?), 0)) AS unread_count \
         FROM conversations c \
         JOIN messages m ON m.id = c.last_message_id \
         JOIN users su ON su.id = m.sender_id \
         JOIN users u ON u.id = IF(c.user_low_id = ?, c.user_high_id, c.user_low_id) \
         LEFT JOIN listings l ON l.id = m.listing_id \
         WHERE (c.user_low_id = ? OR c.user_high_id = ?) AND c.last_message_id IS NOT NULL"
    );
    let anchor = match (before, after) {
        (Some(raw), _) => Some(crate::listings::decode_cursor(raw)?),
        (None, Some(raw)) => Some(crate::listings::decode_cursor(raw)?),
        (None, None) => None,
    };
    if forward {
        sql.push_str(" AND (c.updated_at, c.id) > (?, ?)");
        sql.push_str(" ORDER BY c.updated_at ASC, c.id ASC LIMIT ?");
    } else {
        if anchor.is_some() {
            sql.push_str(" AND (c.last_message_at, c.id) < (?, ?)");
        }
        sql.push_str(" ORDER BY c.last_message_at DESC, c.id DESC LIMIT ?");
    }

    let mut query = sqlx::query(&sql)
        .bind(viewer_id) // viewer_blocked_other.blocker
        .bind(viewer_id) // IF(c.user_low_id = ?, …)
        .bind(viewer_id) // IF(… , c.user_high_id, c.user_low_id)
        .bind(viewer_id) // other_blocked_viewer.blocked
        .bind(viewer_id) // unread: sender_id <> ?
        .bind(viewer_id) // unread: read-state user
        .bind(viewer_id) // other-party IF
        .bind(viewer_id) // participant low
        .bind(viewer_id); // participant high
    if let Some((at, id)) = anchor {
        query = query.bind(at).bind(id);
    }
    query = query.bind((limit + 1) as i64);
    let mut rows = query
        .fetch_all(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    let has_more = rows.len() > limit;
    rows.truncate(limit);

    let mut data = Vec::with_capacity(rows.len());
    for row in &rows {
        let other_status: String = row
            .try_get("other_status")
            .map_err(|_| ApiError::Internal)?;
        let other_deleted: Option<DateTime<Utc>> = row
            .try_get("other_deleted_at")
            .map_err(|_| ApiError::Internal)?;
        let viewer_blocked: bool = row
            .try_get("viewer_blocked_other")
            .map_err(|_| ApiError::Internal)?;
        let other_blocked: bool = row
            .try_get("other_blocked_viewer")
            .map_err(|_| ApiError::Internal)?;
        let unread: i64 = row
            .try_get("unread_count")
            .map_err(|_| ApiError::Internal)?;
        let last_message_at: DateTime<Utc> = row
            .try_get("last_message_at")
            .map_err(|_| ApiError::Internal)?;
        let conversation_created_at: DateTime<Utc> =
            row.try_get("created_at").map_err(|_| ApiError::Internal)?;
        let body: String = row
            .try_get("message_body")
            .map_err(|_| ApiError::Internal)?;
        data.push(serde_json::json!({
            "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
            "other_party": {
                "public_id": row.try_get::<String, _>("other_public_id").map_err(|_| ApiError::Internal)?,
                "display_name": row.try_get::<Option<String>, _>("other_display_name").map_err(|_| ApiError::Internal)?,
            },
            "can_message": other_status == "active" && other_deleted.is_none() && !viewer_blocked && !other_blocked,
            "viewer_blocked_other": viewer_blocked,
            "last_message_at": last_message_at.to_rfc3339(),
            "created_at": conversation_created_at.to_rfc3339(),
            "unread_count": u64::try_from(unread).unwrap_or(0),
            "last_message": {
                "public_id": row.try_get::<String, _>("message_public_id").map_err(|_| ApiError::Internal)?,
                "sender_public_id": row.try_get::<String, _>("sender_public_id").map_err(|_| ApiError::Internal)?,
                "body_preview": preview(&body),
                "created_at": row.try_get::<DateTime<Utc>, _>("message_created_at").map_err(|_| ApiError::Internal)?.to_rfc3339(),
                "listing": listing_reference(row)?,
            },
        }));
    }

    let (older_cursor, next_sync_cursor) = match (forward, rows.last()) {
        (false, Some(last)) if has_more => {
            let at: DateTime<Utc> = last
                .try_get("last_message_at")
                .map_err(|_| ApiError::Internal)?;
            let id: u64 = last.try_get("id").map_err(|_| ApiError::Internal)?;
            (Some(crate::listings::encode_cursor(&at, id)), None)
        }
        (true, Some(last)) if has_more => {
            let at: DateTime<Utc> = last
                .try_get("conversation_updated_at")
                .map_err(|_| ApiError::Internal)?;
            let id: u64 = last.try_get("id").map_err(|_| ApiError::Internal)?;
            (None, Some(crate::listings::encode_cursor(&at, id)))
        }
        _ => (None, None),
    };

    Ok(serde_json::json!({
        "data": data,
        "page": {
            "older_cursor": older_cursor,
            "sync_cursor": sync_cursor,
            "next_sync_cursor": next_sync_cursor,
            "has_more_older": !forward && has_more,
            "has_more_after": forward && has_more,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::{inbox_limit, preview, INBOX_DEFAULT_LIMIT, INBOX_MAX_LIMIT, PREVIEW_CHARS};

    #[test]
    fn inbox_limit_defaults_and_clamps() {
        assert_eq!(inbox_limit(None).expect("default"), INBOX_DEFAULT_LIMIT);
        assert_eq!(inbox_limit(Some("0")).expect("clamped"), 1);
        assert_eq!(inbox_limit(Some("500")).expect("clamped"), INBOX_MAX_LIMIT);
        assert_eq!(inbox_limit(Some("17")).expect("explicit"), 17);
        assert!(inbox_limit(Some("many")).is_err());
    }

    #[test]
    fn preview_counts_unicode_scalars_not_bytes() {
        let body = "س".repeat(PREVIEW_CHARS + 40);
        let cut = preview(&body);
        assert_eq!(cut.chars().count(), PREVIEW_CHARS);
        assert_eq!(preview("کوتاه"), "کوتاه");
    }
}
