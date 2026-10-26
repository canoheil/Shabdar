//! Listing images: local-disk storage + image rows (Phase 3 Task 5).
//!
//! Validation is magic bytes + size + count only (JPEG/PNG/WebP). No image
//! decode — no `image` crate on Termux (approved decision 5). Width/height
//! stay NULL until the image-moderation phase. Client filenames are ignored
//! entirely; the storage key is server-generated.

use serde::Serialize;
use sqlx::{MySqlPool, Row};

use crate::error::ApiError;

/// What the API returns per image (`url` is served from `MEDIA_DIR`).
#[derive(Serialize)]
pub struct ImageOut {
    pub id: u64,
    pub storage_key: String,
    pub url: String,
    pub sort_order: u8,
}

/// File signature → extension. Lengths are the shortest prefixes that pin
/// the format (never decodes the image).
fn detect_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        Some("jpg")
    } else if bytes.len() >= 4 && bytes[0..4] == [0x89, b'P', b'N', b'G'] {
        Some("png")
    } else if bytes.len() >= 12 && bytes[0..4] == *b"RIFF" && bytes[8..12] == *b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// Server-generated key: `<ulid>/<ulid>.<ext>`. No client path segment can
/// ever reach the filesystem (traversal impossible by construction).
fn storage_key(ext: &str) -> String {
    format!("{}/{}.{ext}", ulid::Ulid::new(), ulid::Ulid::new())
}

/// The listing id behind a `public_id`, enforcing owner + `draft`/`rejected`
/// (uploads never touch live listings — v1 immutability).
async fn draft_listing_id(
    pool: &MySqlPool,
    seller_id: u64,
    public_id: &str,
) -> Result<u64, ApiError> {
    let row = crate::listings::own_listing(pool, seller_id, public_id).await?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    if status != "draft" && status != "rejected" {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("status".to_string(), "not_editable".to_string());
        return Err(ApiError::Validation { fields });
    }
    row.try_get("id").map_err(|_| ApiError::Internal)
}

/// Stores one upload: owner/status/count/size/signature checks, file write,
/// then the row in the same logical unit (a DB failure removes the file).
pub async fn add_image(
    pool: &MySqlPool,
    cfg: &crate::config::Config,
    seller_id: u64,
    public_id: &str,
    bytes: &[u8],
) -> Result<ImageOut, ApiError> {
    let listing_id = draft_listing_id(pool, seller_id, public_id).await?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM listing_images WHERE listing_id = ?")
        .bind(listing_id)
        .fetch_one(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    if count >= cfg.max_images_per_listing as i64 {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("images".to_string(), "too_many".to_string());
        return Err(ApiError::Validation { fields });
    }
    if bytes.len() as u64 > cfg.max_image_bytes {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("images".to_string(), "too_large".to_string());
        return Err(ApiError::Validation { fields });
    }
    let ext = detect_ext(bytes).ok_or_else(|| {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("images".to_string(), "invalid_type".to_string());
        ApiError::Validation { fields }
    })?;

    let key = storage_key(ext);
    let path = std::path::Path::new(&cfg.media_dir).join(&key);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|_| ApiError::Internal)?;
    }
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|_| ApiError::Internal)?;

    // MAX over TINYINT UNSIGNED decodes into Option<u64> (same-sign widening;
    // Option<i64> is rejected by sqlx's type check on non-NULL values).
    let next: Option<u64> =
        sqlx::query_scalar("SELECT MAX(sort_order) FROM listing_images WHERE listing_id = ?")
            .bind(listing_id)
            .fetch_one(pool)
            .await
            .map_err(|_| ApiError::Internal)?;
    let order = next.map(|n| n + 1).unwrap_or(0) as u8;
    let now = chrono::Utc::now();
    // The insert id comes from the statement result itself: LAST_INSERT_ID()
    // is connection-scoped, and a separate SELECT may land on another pool
    // connection (regression: every upload after the first returned id 1).
    let outcome = sqlx::query(
        "INSERT INTO listing_images (listing_id, storage_key, sort_order, created_at) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(listing_id)
    .bind(&key)
    .bind(order)
    .bind(now)
    .execute(pool)
    .await;
    let result = match outcome {
        Ok(result) => result,
        Err(_) => {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(ApiError::Internal);
        }
    };
    Ok(ImageOut {
        id: result.last_insert_id(),
        url: format!("/media/{key}"),
        storage_key: key,
        sort_order: order,
    })
}

/// Deletes one image: row first, then the file (a missing file is not an
/// error — the row is the source of truth).
pub async fn remove_image(
    pool: &MySqlPool,
    cfg: &crate::config::Config,
    seller_id: u64,
    public_id: &str,
    image_id: u64,
) -> Result<(), ApiError> {
    let listing_id = draft_listing_id(pool, seller_id, public_id).await?;
    let key: Option<String> = sqlx::query_scalar(
        "SELECT storage_key FROM listing_images WHERE id = ? AND listing_id = ?",
    )
    .bind(image_id)
    .bind(listing_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    let Some(key) = key else {
        return Err(ApiError::NotFound);
    };
    sqlx::query("DELETE FROM listing_images WHERE id = ?")
        .bind(image_id)
        .execute(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    let _ = tokio::fs::remove_file(std::path::Path::new(&cfg.media_dir).join(&key)).await;
    Ok(())
}

/// Reorders images: `ids` must equal the listing's current set exactly
/// (a foreign or missing id → 400). One transaction for all rows.
pub async fn set_order(
    pool: &MySqlPool,
    seller_id: u64,
    public_id: &str,
    ids: &[u64],
) -> Result<(), ApiError> {
    let listing_id = draft_listing_id(pool, seller_id, public_id).await?;
    let current: Vec<u64> =
        sqlx::query_scalar("SELECT id FROM listing_images WHERE listing_id = ? ORDER BY id")
            .bind(listing_id)
            .fetch_all(pool)
            .await
            .map_err(|_| ApiError::Internal)?;
    let mut want: Vec<u64> = ids.to_vec();
    want.sort_unstable();
    if want != current {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("image_ids".to_string(), "mismatch".to_string());
        return Err(ApiError::Validation { fields });
    }
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    // Two passes: a straight swap would trip UNIQUE(listing_id, sort_order)
    // midway (row A takes row B's old slot while B still holds it). The temp
    // band 128..135 can never collide — positions stay below max_images (8).
    for (position, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE listing_images SET sort_order = ? WHERE id = ? AND listing_id = ?")
            .bind(128 + position as u8)
            .bind(id)
            .bind(listing_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| ApiError::Internal)?;
    }
    for (position, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE listing_images SET sort_order = ? WHERE id = ? AND listing_id = ?")
            .bind(position as u8)
            .bind(id)
            .bind(listing_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| ApiError::Internal)?;
    }
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    Ok(())
}

/// Validates a `/media/<key>` path: exactly `<26-char ULID>/<26-char ULID>`
/// with an image extension. Anything else → None (404 — never a filesystem
/// read). The charset makes traversal impossible: no dots except the one
/// before the extension, no slashes except the separator.
fn valid_media_key(key: &str) -> Option<&'static str> {
    let (dir, file) = key.split_once('/')?;
    if dir.len() != 26 || !is_crockford(dir) {
        return None;
    }
    let (name, ext) = file.rsplit_once('.')?;
    if name.len() != 26 || !is_crockford(name) {
        return None;
    }
    match ext {
        "jpg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn is_crockford(s: &str) -> bool {
    s.bytes().all(|b| {
        matches!(b,
            b'0'..=b'9' | b'A'..=b'H' | b'J'..=b'K' | b'M'..=b'N' | b'P'..=b'T' | b'V'..=b'Z')
    })
}

/// Reads one stored file for `GET /media/…`. No auth: a listing's photos are
/// public exactly like the listing, and the ULID keys are unguessable.
/// Missing files and malformed keys are both 404 (no existence signal).
pub async fn read_media(
    cfg: &crate::config::Config,
    key: &str,
) -> Result<(Vec<u8>, &'static str), ApiError> {
    let content_type = valid_media_key(key).ok_or(ApiError::NotFound)?;
    let bytes = tokio::fs::read(std::path::Path::new(&cfg.media_dir).join(key))
        .await
        .map_err(|_| ApiError::NotFound)?;
    Ok((bytes, content_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_map_to_extensions() {
        assert_eq!(detect_ext(&[0xFF, 0xD8, 0xFF, 0x00]), Some("jpg"));
        assert_eq!(detect_ext(&[0x89, b'P', b'N', b'G']), Some("png"));
        assert_eq!(detect_ext(b"RIFF1234WEBP"), Some("webp"));
        assert_eq!(detect_ext(b"MZ\x90\x00"), None);
        assert_eq!(detect_ext(&[]), None);
        // Truncated WebP (no trailing tag) is not WebP.
        assert_eq!(detect_ext(b"RIFF1234"), None);
    }

    #[test]
    fn storage_key_is_ulids_with_an_image_extension() {
        let key = storage_key("png");
        let (dirs, file) = key.split_once('/').expect("dir/file shape");
        assert_eq!(dirs.len(), 26, "random ULID directory");
        assert!(file.ends_with(".png"), "server-chosen extension: {key}");
        assert!(
            !key.contains("..") && !key.starts_with('/'),
            "no traversal possible: {key}"
        );
    }

    // ponytail: magic bytes only — a payload with a valid signature but
    // garbage after it still passes detect_ext. Full decode arrives with the
    // image-moderation decision; this test pins the current ceiling.
    #[test]
    fn magic_bytes_only_garbage_after_signature_passes() {
        assert_eq!(
            detect_ext(&[0xFF, 0xD8, 0xFF, b'!', b'!', b'!']),
            Some("jpg")
        );
    }
}
