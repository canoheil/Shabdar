//! Listing images: bounded local decode, local-disk storage, and media access.
//!
//! Client filenames and declared MIME are never trusted. Only static
//! JPEG/PNG/WebP bytes are accepted, and the pure-Rust decoder runs on the
//! blocking pool with explicit resource ceilings. The storage key remains
//! server-generated.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, GenericImageView, ImageEncoder, ImageReader};
use serde::Serialize;
use sqlx::{MySqlPool, Row};
use tokio::io::AsyncReadExt;

use crate::error::ApiError;

const MAX_IMAGE_DIMENSION: u32 = 8_000;
const MAX_IMAGE_PIXELS: u64 = 16_000_000;
const MAX_DECODER_ALLOCATION: u64 = 80 * 1024 * 1024;
const DERIVATIVE_WIDTHS: [u32; 3] = [240, 480, 960];

/// What the API returns per image (`url` is served from `MEDIA_DIR`).
#[derive(Serialize)]
pub struct ImageOut {
    pub id: u64,
    pub storage_key: String,
    pub url: String,
    pub sort_order: u8,
    pub width: u16,
    pub height: u16,
    pub srcset: String,
    pub webp_srcset: String,
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
    decoded: DynamicImage,
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
        let length =
            u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("four bytes")) as usize;
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
        let length = u32::from_le_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .expect("four bytes"),
        ) as usize;
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
        decoded,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DerivativeFormat {
    Jpeg,
    Webp,
}

impl DerivativeFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
        }
    }

    fn content_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
        }
    }
}

struct GeneratedDerivative {
    width: u32,
    format: DerivativeFormat,
    bytes: Vec<u8>,
}

/// The original extension is retained inside a derivative key so the parent
/// row remains unambiguous: `name.png--w480.webp` belongs to `name.png`.
fn derivative_key(original_key: &str, width: u32, format: DerivativeFormat) -> String {
    format!("{original_key}--w{width}.{}", format.extension())
}

/// Lossless WebP is useful for large PNG/illustration sources, but can make
/// photographic JPEGs larger. Keep the modern source where it has a credible
/// byte-saving path and always retain a high-quality JPEG fallback.
fn webp_is_justified(extension: &str, width: u16) -> bool {
    extension == "png" && width > DERIVATIVE_WIDTHS[DERIVATIVE_WIDTHS.len() - 1]
}

/// Additive API metadata. No derivative is ever wider than its source; the
/// original remains the largest fallback candidate.
pub fn responsive_srcsets(storage_key: &str, width: u16) -> (String, String) {
    let extension = storage_key
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or_default();
    let mut fallback = DERIVATIVE_WIDTHS
        .iter()
        .copied()
        .filter(|candidate| *candidate < u32::from(width))
        .map(|candidate| {
            format!(
                "/media/{} {candidate}w",
                derivative_key(storage_key, candidate, DerivativeFormat::Jpeg)
            )
        })
        .collect::<Vec<_>>();
    fallback.push(format!("/media/{storage_key} {width}w"));
    let webp = if webp_is_justified(extension, width) {
        DERIVATIVE_WIDTHS
            .iter()
            .copied()
            .filter(|candidate| *candidate < u32::from(width))
            .map(|candidate| {
                format!(
                    "/media/{} {candidate}w",
                    derivative_key(storage_key, candidate, DerivativeFormat::Webp)
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        String::new()
    };
    (fallback.join(", "), webp)
}

fn encode_jpeg(image: &DynamicImage) -> image::ImageResult<Vec<u8>> {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut rgb = image::RgbImage::new(width, height);
    // Composite transparency over white instead of letting a JPEG conversion
    // turn transparent PNG pixels black.
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = u16::from(pixel[3]);
        let mut channels = [255_u8; 3];
        for (channel, output) in channels.iter_mut().enumerate() {
            let foreground = u16::from(pixel[channel]);
            *output = ((foreground * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
        }
        rgb.put_pixel(x, y, image::Rgb(channels));
    }
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 82).write_image(
        rgb.as_raw(),
        width,
        height,
        ExtendedColorType::Rgb8,
    )?;
    Ok(bytes)
}

fn encode_webp(image: &DynamicImage) -> image::ImageResult<Vec<u8>> {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut bytes = Vec::new();
    WebPEncoder::new_lossless(&mut bytes).write_image(
        rgba.as_raw(),
        width,
        height,
        ExtendedColorType::Rgba8,
    )?;
    Ok(bytes)
}

fn generate_derivative(
    decoded: &DynamicImage,
    width: u32,
    format: DerivativeFormat,
) -> image::ImageResult<GeneratedDerivative> {
    let resized = decoded.resize(width, u32::MAX, FilterType::Lanczos3);
    let bytes = match format {
        DerivativeFormat::Jpeg => encode_jpeg(&resized)?,
        DerivativeFormat::Webp => encode_webp(&resized)?,
    };
    Ok(GeneratedDerivative {
        width,
        format,
        bytes,
    })
}

fn generate_derivatives(
    validated: &ValidatedImage,
) -> image::ImageResult<Vec<GeneratedDerivative>> {
    let mut generated = Vec::new();
    for width in DERIVATIVE_WIDTHS {
        if width >= u32::from(validated.width) {
            continue;
        }
        generated.push(generate_derivative(
            &validated.decoded,
            width,
            DerivativeFormat::Jpeg,
        )?);
        if webp_is_justified(validated.extension, validated.width) {
            generated.push(generate_derivative(
                &validated.decoded,
                width,
                DerivativeFormat::Webp,
            )?);
        }
    }
    Ok(generated)
}

fn derivative_is_allowed(
    source_extension: &str,
    source_width: u16,
    width: u32,
    format: DerivativeFormat,
) -> bool {
    width < u32::from(source_width)
        && DERIVATIVE_WIDTHS.contains(&width)
        && (format != DerivativeFormat::Webp || webp_is_justified(source_extension, source_width))
}

async fn cleanup_files(paths: &[std::path::PathBuf]) {
    for path in paths {
        let _ = tokio::fs::remove_file(path).await;
    }
}

/// Revalidate stored files at a human review boundary. Reads are capped even
/// for legacy/corrupted rows, keys are validated before joining the media
/// path, and each image is decoded sequentially to bound total memory.
pub async fn validate_stored_images(
    cfg: &crate::config::Config,
    images: &[(u64, String)],
) -> Result<Vec<(u64, u16, u16)>, ApiError> {
    let mut validated = Vec::with_capacity(images.len());
    for (id, key) in images {
        let expected_content_type =
            valid_media_key(key).ok_or_else(|| image_validation("invalid_image"))?;
        let file = tokio::fs::File::open(std::path::Path::new(&cfg.media_dir).join(key))
            .await
            .map_err(|_| image_validation("unreadable"))?;
        let mut bytes = Vec::new();
        file.take(cfg.max_image_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| image_validation("unreadable"))?;
        if bytes.len() as u64 > cfg.max_image_bytes {
            return Err(image_validation("too_large"));
        }
        let decoded = tokio::task::spawn_blocking(move || decode_image(&bytes))
            .await
            .map_err(|_| ApiError::Internal)?
            .map_err(|error| image_validation(error.reason()))?;
        let actual_content_type = match decoded.extension {
            "jpg" => "image/jpeg",
            "png" => "image/png",
            "webp" => "image/webp",
            _ => return Err(image_validation("invalid_image")),
        };
        if actual_content_type != expected_content_type {
            return Err(image_validation("invalid_image"));
        }
        validated.push((*id, decoded.width, decoded.height));
    }
    Ok(validated)
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

async fn lock_editable_listing(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    listing_id: u64,
) -> Result<(), ApiError> {
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM listings WHERE id = ? AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(listing_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    match status.as_deref() {
        Some("draft" | "rejected") => Ok(()),
        Some(_) => {
            let mut fields = std::collections::BTreeMap::new();
            fields.insert("status".to_string(), "not_editable".to_string());
            Err(ApiError::Validation { fields })
        }
        None => Err(ApiError::NotFound),
    }
}

async fn clear_review_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    listing_id: u64,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE listings SET image_reviewed_by = NULL, image_reviewed_at = NULL, \
         image_reviewed_ids = NULL WHERE id = ?",
    )
    .bind(listing_id)
    .execute(&mut **tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    Ok(())
}

/// Stores one upload: owner/status/count/size/signature checks, file write,
/// then the row and review reset in one DB transaction (failure removes file).
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
    let (bytes, extension, width, height, derivatives) = tokio::task::spawn_blocking(move || {
        let validated = decode_image(&bytes).map_err(|error| image_validation(error.reason()))?;
        let derivatives = generate_derivatives(&validated).map_err(|_| ApiError::Internal)?;
        Ok::<_, ApiError>((
            bytes,
            validated.extension,
            validated.width,
            validated.height,
            derivatives,
        ))
    })
    .await
    .map_err(|_| ApiError::Internal)??;

    let key = storage_key(extension);
    let path = std::path::Path::new(&cfg.media_dir).join(&key);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|_| ApiError::Internal)?;
    }
    let mut written_paths = Vec::with_capacity(derivatives.len() + 1);
    if tokio::fs::write(&path, bytes).await.is_err() {
        return Err(ApiError::Internal);
    }
    written_paths.push(path.clone());
    for derivative in &derivatives {
        let derivative_path = std::path::Path::new(&cfg.media_dir).join(derivative_key(
            &key,
            derivative.width,
            derivative.format,
        ));
        if tokio::fs::write(&derivative_path, &derivative.bytes)
            .await
            .is_err()
        {
            cleanup_files(&written_paths).await;
            return Err(ApiError::Internal);
        }
        written_paths.push(derivative_path);
    }

    let outcome = async {
        let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
        // Every mutation uses listing → image lock order, also rechecking
        // editability and count after any concurrent submit/upload.
        lock_editable_listing(&mut tx, listing_id).await?;
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM listing_images WHERE listing_id = ?")
                .bind(listing_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| ApiError::Internal)?;
        if count >= cfg.max_images_per_listing as i64 {
            return Err(image_validation("too_many"));
        }
        // MAX over TINYINT UNSIGNED decodes into Option<u64> (same-sign widening;
        // Option<i64> is rejected by sqlx's type check on non-NULL values).
        let next: Option<u64> =
            sqlx::query_scalar("SELECT MAX(sort_order) FROM listing_images WHERE listing_id = ?")
                .bind(listing_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| ApiError::Internal)?;
        let order = next.map(|n| n + 1).unwrap_or(0) as u8;
        let now = chrono::Utc::now();
        // The insert id comes from the statement result itself: LAST_INSERT_ID()
        // is connection-scoped, and a separate SELECT may use another connection.
        let result = sqlx::query(
            "INSERT INTO listing_images \
             (listing_id, storage_key, sort_order, width, height, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(listing_id)
        .bind(&key)
        .bind(order)
        .bind(width)
        .bind(height)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
        clear_review_in_tx(&mut tx, listing_id).await?;
        tx.commit().await.map_err(|_| ApiError::Internal)?;
        Ok::<_, ApiError>((result, order))
    }
    .await;
    let (result, order) = match outcome {
        Ok(result) => result,
        Err(error) => {
            cleanup_files(&written_paths).await;
            return Err(error);
        }
    };
    let (srcset, webp_srcset) = responsive_srcsets(&key, width);
    Ok(ImageOut {
        id: result.last_insert_id(),
        url: format!("/media/{key}"),
        storage_key: key,
        sort_order: order,
        width,
        height,
        srcset,
        webp_srcset,
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
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    lock_editable_listing(&mut tx, listing_id).await?;
    let key: Option<String> = sqlx::query_scalar(
        "SELECT storage_key FROM listing_images WHERE id = ? AND listing_id = ? FOR UPDATE",
    )
    .bind(image_id)
    .bind(listing_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    let Some(key) = key else {
        return Err(ApiError::NotFound);
    };
    let deleted = sqlx::query("DELETE FROM listing_images WHERE id = ? AND listing_id = ?")
        .bind(image_id)
        .bind(listing_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::Internal)?;
    if deleted.rows_affected() != 1 {
        return Err(ApiError::NotFound);
    }
    clear_review_in_tx(&mut tx, listing_id).await?;
    tx.commit().await.map_err(|_| ApiError::Internal)?;
    let media_dir = std::path::Path::new(&cfg.media_dir);
    let _ = tokio::fs::remove_file(media_dir.join(&key)).await;
    for width in DERIVATIVE_WIDTHS {
        for format in [DerivativeFormat::Jpeg, DerivativeFormat::Webp] {
            let _ = tokio::fs::remove_file(
                media_dir.join(derivative_key(&key, width, format)),
            )
            .await;
        }
    }
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
    let mut tx = pool.begin().await.map_err(|_| ApiError::Internal)?;
    lock_editable_listing(&mut tx, listing_id).await?;
    let current: Vec<u64> = sqlx::query_scalar(
        "SELECT id FROM listing_images WHERE listing_id = ? ORDER BY id FOR UPDATE",
    )
    .bind(listing_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| ApiError::Internal)?;
    let mut want: Vec<u64> = ids.to_vec();
    want.sort_unstable();
    if want != current {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("image_ids".to_string(), "mismatch".to_string());
        return Err(ApiError::Validation { fields });
    }
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

fn content_type_for_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "jpg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// Validate the original `<ULID>/<ULID>.<ext>` shape and return its extension.
fn valid_original_file(dir: &str, file: &str) -> Option<&'static str> {
    if dir.len() != 26 || !is_crockford(dir) {
        return None;
    }
    let (name, extension) = file.rsplit_once('.')?;
    if name.len() != 26 || !is_crockford(name) {
        return None;
    }
    match extension {
        "jpg" => Some("jpg"),
        "png" => Some("png"),
        "webp" => Some("webp"),
        _ => None,
    }
}

struct MediaRequest {
    original_key: String,
    source_extension: &'static str,
    content_type: &'static str,
    derivative: Option<(u32, DerivativeFormat)>,
}

/// Original and derivative keys share the same strict ULID grammar. Only the
/// three server-approved widths and JPEG/WebP response formats can reach disk.
fn parse_media_key(key: &str) -> Option<MediaRequest> {
    let (dir, file) = key.split_once('/')?;
    if let Some(source_extension) = valid_original_file(dir, file) {
        return Some(MediaRequest {
            original_key: key.to_string(),
            source_extension,
            content_type: content_type_for_extension(source_extension)?,
            derivative: None,
        });
    }

    let (derivative_stem, response_extension) = file.rsplit_once('.')?;
    let format = match response_extension {
        "jpg" => DerivativeFormat::Jpeg,
        "webp" => DerivativeFormat::Webp,
        _ => return None,
    };
    let (original_file, raw_width) = derivative_stem.rsplit_once("--w")?;
    let source_extension = valid_original_file(dir, original_file)?;
    let width = raw_width.parse::<u32>().ok()?;
    if !DERIVATIVE_WIDTHS.contains(&width) {
        return None;
    }
    Some(MediaRequest {
        original_key: format!("{dir}/{original_file}"),
        source_extension,
        content_type: format.content_type(),
        derivative: Some((width, format)),
    })
}

/// Stored rows always point to originals, never to derived cache files.
fn valid_media_key(key: &str) -> Option<&'static str> {
    let request = parse_media_key(key)?;
    if request.derivative.is_some() {
        return None;
    }
    Some(request.content_type)
}

fn is_crockford(s: &str) -> bool {
    s.bytes().all(|b| {
        matches!(b,
            b'0'..=b'9' | b'A'..=b'H' | b'J'..=b'K' | b'M'..=b'N' | b'P'..=b'T' | b'V'..=b'Z')
    })
}

/// Reads one stored original or derivative for `GET /media/…`. Authorization
/// always resolves the original parent row before disk access. A missing
/// derivative for legacy content is generated once from the bounded original.
pub async fn read_media(
    pool: &MySqlPool,
    cfg: &crate::config::Config,
    key: &str,
    viewer: Option<(u64, bool)>,
) -> Result<(Vec<u8>, &'static str, bool), ApiError> {
    let request = parse_media_key(key).ok_or(ApiError::NotFound)?;
    let row = sqlx::query(
        "SELECT l.seller_id, l.status, l.deleted_at, i.width, \
         l.image_reviewed_at IS NOT NULL AS image_reviewed \
         FROM listing_images i JOIN listings l ON l.id = i.listing_id \
         WHERE i.storage_key = ?",
    )
    .bind(&request.original_key)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
    let image_reviewed: bool = row
        .try_get("image_reviewed")
        .map_err(|_| ApiError::Internal)?;
    let public = status == "active" && deleted_at.is_none() && image_reviewed;
    if !public {
        let seller_id: u64 = row.try_get("seller_id").map_err(|_| ApiError::Internal)?;
        let allowed = viewer.is_some_and(|(user_id, is_admin)| is_admin || user_id == seller_id);
        if !allowed {
            return Err(ApiError::NotFound);
        }
    }

    let source_width: Option<u16> = row.try_get("width").map_err(|_| ApiError::Internal)?;
    if let (Some(width), Some((derivative_width, format))) = (source_width, request.derivative) {
        if !derivative_is_allowed(
            request.source_extension,
            width,
            derivative_width,
            format,
        ) {
            return Err(ApiError::NotFound);
        }
    }

    let path = std::path::Path::new(&cfg.media_dir).join(key);
    match tokio::fs::read(&path).await {
        Ok(bytes) => return Ok((bytes, request.content_type, public)),
        Err(_) if request.derivative.is_none() => return Err(ApiError::NotFound),
        Err(_) => {}
    }

    let (derivative_width, format) = request.derivative.ok_or(ApiError::NotFound)?;
    let original_bytes = tokio::fs::read(
        std::path::Path::new(&cfg.media_dir).join(&request.original_key),
    )
    .await
    .map_err(|_| ApiError::NotFound)?;
    let source_extension = request.source_extension;
    let bytes = tokio::task::spawn_blocking(move || {
        let validated = decode_image(&original_bytes).map_err(|_| ApiError::NotFound)?;
        if !derivative_is_allowed(
            source_extension,
            validated.width,
            derivative_width,
            format,
        ) {
            return Err(ApiError::NotFound);
        }
        generate_derivative(&validated.decoded, derivative_width, format)
            .map(|generated| generated.bytes)
            .map_err(|_| ApiError::Internal)
    })
    .await
    .map_err(|_| ApiError::Internal)??;

    // A unique temporary sibling plus atomic rename prevents concurrent
    // first-hit requests from exposing a partially written derivative.
    let temporary_path = std::path::PathBuf::from(format!(
        "{}.{}.tmp",
        path.to_string_lossy(),
        ulid::Ulid::new()
    ));
    if tokio::fs::write(&temporary_path, &bytes).await.is_err() {
        return Err(ApiError::Internal);
    }
    if tokio::fs::rename(&temporary_path, &path).await.is_err() {
        let _ = tokio::fs::remove_file(&temporary_path).await;
        return Err(ApiError::Internal);
    }
    Ok((bytes, request.content_type, public))
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
    fn derivative_keys_resolve_only_their_original_and_approved_widths() {
        let original = storage_key("png");
        let derivative = derivative_key(&original, 480, DerivativeFormat::Webp);
        let parsed = parse_media_key(&derivative).expect("valid derivative key");
        assert_eq!(parsed.original_key, original);
        assert_eq!(parsed.source_extension, "png");
        assert_eq!(parsed.content_type, "image/webp");
        assert_eq!(parsed.derivative, Some((480, DerivativeFormat::Webp)));
        assert!(valid_media_key(&derivative).is_none());

        assert!(parse_media_key(&derivative_key(&original, 481, DerivativeFormat::Jpeg)).is_none());
        assert!(parse_media_key(&derivative.replace("--w480", "/--w480")).is_none());
        assert!(parse_media_key("../--w480.webp").is_none());
    }

    #[test]
    fn responsive_sources_never_upscale_and_webp_is_selective() {
        let original = storage_key("png");
        let (fallback, webp) = responsive_srcsets(&original, 1200);
        for width in DERIVATIVE_WIDTHS {
            assert!(fallback.contains(&format!("--w{width}.jpg {width}w")));
            assert!(webp.contains(&format!("--w{width}.webp {width}w")));
        }
        assert!(fallback.contains(&format!("/media/{original} 1200w")));

        let (small_fallback, small_webp) = responsive_srcsets(&original, 480);
        assert!(small_fallback.contains("--w240.jpg 240w"));
        assert!(!small_fallback.contains("--w480.jpg"));
        assert!(!small_fallback.contains("--w960.jpg"));
        assert!(small_fallback.contains(&format!("/media/{original} 480w")));
        assert!(small_webp.is_empty());

        let jpeg = storage_key("jpg");
        assert!(responsive_srcsets(&jpeg, 1200).1.is_empty());
    }

    #[test]
    fn generated_derivatives_have_real_dimensions_and_decodable_formats() {
        let validated = ValidatedImage {
            extension: "png",
            width: 1200,
            height: 600,
            decoded: DynamicImage::new_rgba8(1200, 600),
        };
        let generated = generate_derivatives(&validated).expect("encode derivatives");
        assert_eq!(generated.len(), 6);
        for derivative in generated {
            let decoded = image::load_from_memory(&derivative.bytes).expect("decodable derivative");
            assert_eq!(decoded.width(), derivative.width);
            assert_eq!(decoded.height(), derivative.width / 2);
            assert!(DERIVATIVE_WIDTHS.contains(&decoded.width()));
        }
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
            (
                &include_bytes!("../tests/fixtures/tiny.png")[..],
                "png",
                (3, 2),
            ),
            (
                &include_bytes!("../tests/fixtures/tiny.jpg")[..],
                "jpg",
                (4, 3),
            ),
            (
                &include_bytes!("../tests/fixtures/tiny.webp")[..],
                "webp",
                (5, 4),
            ),
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
