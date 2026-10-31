//! Listing images: bounded local decode, local-disk storage, and media access.
//!
//! Client filenames and declared MIME are never trusted. Only static
//! JPEG/PNG/WebP bytes are accepted, and the pure-Rust decoder runs on the
//! blocking pool with explicit resource ceilings. The storage key remains
//! server-generated.

use std::io::Cursor;

use image::{GenericImageView, ImageReader};
use serde::Serialize;
use sqlx::{MySqlPool, Row};

use crate::error::ApiError;

const MAX_IMAGE_DIMENSION: u32 = 8_000;
const MAX_IMAGE_PIXELS: u64 = 16_000_000;
const MAX_DECODER_ALLOCATION: u64 = 80 * 1024 * 1024;

/// What the API returns per image (`url` is served from `MEDIA_DIR`).
#[derive(Serialize)]
pub struct ImageOut {
    pub id: u64,
    pub storage_key: String,
    pub url: String,
    pub sort_order: u8,
    pub width: u16,
    pub height: u16,
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

#[derive(Debug, Eq, PartialEq)]
enum ImageRejection {
    InvalidType,
    InvalidImage,
    Animated,
    TooLarge,
}

impl ImageRejection {
    fn reason(&self) -> &'static str {
        match self {
            Self::InvalidType => "invalid_type",
            Self::InvalidImage => "invalid_image",
            Self::Animated => "animated",
            Self::TooLarge => "too_large",
        }
    }
}

struct ValidatedImage {
    extension: &'static str,
    width: u16,
    height: u16,
}

fn image_validation(reason: &str) -> ApiError {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("images".to_string(), reason.to_string());
    ApiError::Validation { fields }
}

/// Detect APNG structurally. Malformed chunk bounds are left to the full
/// decoder, which rejects them; this pass only recognizes animation markers.
fn is_animated_png(bytes: &[u8]) -> bool {
    let mut offset = 8_usize;
    while offset.checked_add(8).is_some_and(|end| end <= bytes.len()) {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
            as usize;
        let kind = &bytes[offset + 4..offset + 8];
        if kind == b"acTL" {
            return true;
        }
        let Some(data_end) = (offset + 8).checked_add(length) else {
            return false;
        };
        let Some(next) = data_end.checked_add(4) else {
            return false;
        };
        if next > bytes.len() {
            return false;
        }
        if kind == b"IEND" {
            return false;
        }
        offset = next;
    }
    false
}

/// Reject animated WebP by its VP8X animation flag or animation chunks. RIFF
/// sizes and payload validity are still verified by the actual decoder.
fn is_animated_webp(bytes: &[u8]) -> bool {
    let mut offset = 12_usize;
    while offset.checked_add(8).is_some_and(|end| end <= bytes.len()) {
        let kind = &bytes[offset..offset + 4];
        let length = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().expect("four bytes"))
            as usize;
        if kind == b"ANIM" || kind == b"ANMF" {
            return true;
        }
        let data_start = offset + 8;
        if kind == b"VP8X"
            && length >= 1
            && data_start < bytes.len()
            && bytes[data_start] & 0x02 != 0
        {
            return true;
        }
        let Some(data_end) = data_start.checked_add(length) else {
            return false;
        };
        let Some(next) = data_end.checked_add(length % 2) else {
            return false;
        };
        if next > bytes.len() {
            return false;
        }
        offset = next;
    }
    false
}

fn check_dimensions(width: u32, height: u32) -> Result<(), ImageRejection> {
    if width == 0 || height == 0 {
        return Err(ImageRejection::InvalidImage);
    }
    if width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
    {
        return Err(ImageRejection::TooLarge);
    }
    Ok(())
}

/// Fully decode one supported static image with bounded dimensions/allocation.
/// Signature detection chooses the decoder; it is never treated as validity.
fn decode_image(bytes: &[u8]) -> Result<ValidatedImage, ImageRejection> {
    let extension = detect_ext(bytes).ok_or(ImageRejection::InvalidType)?;
    match extension {
        "png" if is_animated_png(bytes) => return Err(ImageRejection::Animated),
        "webp" if is_animated_webp(bytes) => return Err(ImageRejection::Animated),
        _ => {}
    }
    let format = match extension {
        "jpg" => image::ImageFormat::Jpeg,
        "png" => image::ImageFormat::Png,
        "webp" => image::ImageFormat::WebP,
        _ => return Err(ImageRejection::InvalidType),
    };
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODER_ALLOCATION);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|error| match error {
        image::ImageError::Limits(_) => ImageRejection::TooLarge,
        _ => ImageRejection::InvalidImage,
    })?;
    let (width, height) = decoded.dimensions();
    check_dimensions(width, height)?;
    Ok(ValidatedImage {
        extension,
        width: u16::try_from(width).map_err(|_| ImageRejection::TooLarge)?,
        height: u16::try_from(height).map_err(|_| ImageRejection::TooLarge)?,
    })
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
    bytes: Vec<u8>,
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
        return Err(image_validation("too_large"));
    }
    let (bytes, validated) = tokio::task::spawn_blocking(move || {
        let validated = decode_image(&bytes)?;
        Ok::<_, ImageRejection>((bytes, validated))
    })
    .await
    .map_err(|_| ApiError::Internal)?
    .map_err(|error| image_validation(error.reason()))?;

    let key = storage_key(validated.extension);
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
        "INSERT INTO listing_images \
         (listing_id, storage_key, sort_order, width, height, created_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(listing_id)
    .bind(&key)
    .bind(order)
    .bind(validated.width)
    .bind(validated.height)
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
        width: validated.width,
        height: validated.height,
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

    #[test]
    fn signature_garbage_is_not_a_decoded_image() {
        assert!(matches!(
            decode_image(&[0xFF, 0xD8, 0xFF, b'!', b'!', b'!']),
            Err(ImageRejection::InvalidImage)
        ));
    }

    #[test]
    fn real_static_fixtures_decode_their_dimensions() {
        for (bytes, extension, dimensions) in [
            (&include_bytes!("../tests/fixtures/tiny.png")[..], "png", (3, 2)),
            (&include_bytes!("../tests/fixtures/tiny.jpg")[..], "jpg", (4, 3)),
            (&include_bytes!("../tests/fixtures/tiny.webp")[..], "webp", (5, 4)),
        ] {
            let image = decode_image(bytes).expect("valid checked-in fixture");
            assert_eq!(image.extension, extension);
            assert_eq!((image.width, image.height), dimensions);
        }
    }

    #[test]
    fn animation_and_decoded_pixel_ceiling_are_explicit() {
        assert!(matches!(
            decode_image(include_bytes!("../tests/fixtures/animated.png")),
            Err(ImageRejection::Animated)
        ));
        assert!(matches!(
            decode_image(include_bytes!("../tests/fixtures/animated.webp")),
            Err(ImageRejection::Animated)
        ));
        assert_eq!(check_dimensions(4001, 4000), Err(ImageRejection::TooLarge));
        assert_eq!(check_dimensions(4000, 4000), Ok(()));
    }
}
