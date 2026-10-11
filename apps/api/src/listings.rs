//! Seller listings: validation, slug generation, and the draft → pending →
//! active/rejected/sold/archived state machine (Phase 3 Task 3).
//!
//! Handlers in routes.rs stay thin: they check CSRF, call exactly one
//! function here, and serialize the returned `ListingOut`.

use std::collections::BTreeMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{MySqlPool, Row};

use crate::error::ApiError;

/// Description bound (approved 2026-10-11: 5000 chars).
const MAX_DESCRIPTION_CHARS: usize = 5000;
/// Title bound (`VARCHAR(160)`, chars not bytes).
const MAX_TITLE_CHARS: usize = 160;
/// Slug headroom: base is cut here so `-N` suffixes stay in `VARCHAR(200)`.
const SLUG_BASE_CHARS: usize = 180;

/// Create body: every field except the price is required; the price rule
/// depends on `price_type` (approved 2026-10-11: negotiable leaves it
/// optional, fixed requires it, free forbids it).
#[derive(Deserialize)]
pub struct CreateBody {
    pub category_id: Option<u16>,
    pub city_id: Option<u32>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub price_toman: Option<i64>,
    pub price_type: Option<String>,
}

/// Edit body: missing fields are left alone. `price_toman` is doubly
/// optional so `null` clears the price while a missing key keeps it.
#[derive(Deserialize)]
pub struct EditBody {
    pub category_id: Option<u16>,
    pub city_id: Option<u32>,
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "opt_opt_i64")]
    pub price_toman: Option<Option<i64>>,
    pub price_type: Option<String>,
}

/// `Option<Option<i64>>`: missing → None, `null` → Some(None), n → Some(Some(n)).
fn opt_opt_i64<'de, D>(deserializer: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

/// The pinned owner-facing shape (docs/PHASE3_PLAN.md Task 3).
#[derive(Serialize)]
pub struct ListingOut {
    pub public_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub price_toman: Option<u64>,
    pub price_type: String,
    pub status: String,
    pub category_id: u16,
    pub city_id: u32,
    pub rejection_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub fn listing_out(row: &sqlx::mysql::MySqlRow) -> Result<ListingOut, ApiError> {
    // Timestamps serialize as RFC3339 strings (ponytail: string fields dodge
    // the chrono `serde` feature — no new dependency surface for one format).
    let created_at: chrono::DateTime<Utc> = row.try_get("created_at").map_err(|_| ApiError::Internal)?;
    let updated_at: chrono::DateTime<Utc> = row.try_get("updated_at").map_err(|_| ApiError::Internal)?;
    Ok(ListingOut {
        public_id: row.try_get("public_id").map_err(|_| ApiError::Internal)?,
        slug: row.try_get("slug").map_err(|_| ApiError::Internal)?,
        title: row.try_get("title").map_err(|_| ApiError::Internal)?,
        description: row.try_get("description").map_err(|_| ApiError::Internal)?,
        price_toman: row.try_get("price_toman").map_err(|_| ApiError::Internal)?,
        price_type: row.try_get("price_type").map_err(|_| ApiError::Internal)?,
        status: row.try_get("status").map_err(|_| ApiError::Internal)?,
        category_id: row.try_get("category_id").map_err(|_| ApiError::Internal)?,
        city_id: row.try_get("city_id").map_err(|_| ApiError::Internal)?,
        rejection_reason: row.try_get("rejection_reason").map_err(|_| ApiError::Internal)?,
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    })
}

/// Server-side slug from a Persian title: trim, whitespace runs become one
/// `-`, keep Unicode alphanumerics and `-` only, cut to 180 chars. Falls back
/// to `listing` when nothing survives (e.g. a symbols-only title).
pub fn make_slug(title: &str) -> String {
    let dashed: String = title.split_whitespace().collect::<Vec<_>>().join("-");
    let mut slug: String = dashed
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .take(SLUG_BASE_CHARS)
        .collect();
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug.push_str("listing");
    }
    slug
}

fn required_text(value: &Option<String>, field: &str, max: usize, errors: &mut BTreeMap<String, String>) -> Option<String> {
    match value {
        None => {
            errors.insert(field.to_string(), "required".to_string());
            None
        }
        Some(raw) => {
            let trimmed = raw.trim().to_string();
            if trimmed.is_empty() {
                errors.insert(field.to_string(), "required".to_string());
                None
            } else if trimmed.chars().count() > max {
                errors.insert(field.to_string(), "too_long".to_string());
                None
            } else {
                Some(trimmed)
            }
        }
    }
}

/// The price rule, shared by create and edit. Returns the stored price.
fn check_price(
    price_type: &str,
    price_toman: Option<i64>,
    errors: &mut BTreeMap<String, String>,
) -> Option<Option<u64>> {
    match price_type {
        "fixed" => match price_toman {
            Some(n) if n >= 0 => Some(Some(n as u64)),
            _ => {
                errors.insert("price_toman".to_string(), "invalid_price".to_string());
                None
            }
        },
        "negotiable" => match price_toman {
            None => Some(None),
            Some(n) if n >= 0 => Some(Some(n as u64)),
            _ => {
                errors.insert("price_toman".to_string(), "invalid_price".to_string());
                None
            }
        },
        "free" => match price_toman {
            None => Some(None),
            _ => {
                errors.insert("price_toman".to_string(), "invalid_price".to_string());
                None
            }
        },
        _ => {
            errors.insert("price_type".to_string(), "invalid_price".to_string());
            None
        }
    }
}

async fn check_category(pool: &MySqlPool, id: u16, errors: &mut BTreeMap<String, String>) {
    let found = sqlx::query("SELECT id FROM categories WHERE id = ? AND is_active = TRUE")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal);
    match found {
        Err(_) | Ok(None) => {
            errors.insert("category_id".to_string(), "unknown_category".to_string());
        }
        Ok(Some(_)) => {}
    }
}

async fn check_city(pool: &MySqlPool, id: u32, errors: &mut BTreeMap<String, String>) {
    let found = sqlx::query("SELECT id FROM cities WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal);
    match found {
        Err(_) | Ok(None) => {
            errors.insert("city_id".to_string(), "unknown_city".to_string());
        }
        Ok(Some(_)) => {}
    }
}

/// Load one listing by `public_id`: unknown → `NotFound`, another seller's →
/// `Forbidden` (the plan pins 403 for non-owners).
pub async fn own_listing(
    pool: &MySqlPool,
    seller_id: u64,
    public_id: &str,
) -> Result<sqlx::mysql::MySqlRow, ApiError> {
    let row = sqlx::query("SELECT * FROM listings WHERE public_id = ? AND deleted_at IS NULL")
        .bind(public_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    let owner: u64 = row.try_get("seller_id").map_err(|_| ApiError::Internal)?;
    if owner != seller_id {
        return Err(ApiError::Forbidden);
    }
    Ok(row)
}

fn is_duplicate_slug(err: &sqlx::Error) -> bool {
    // MariaDB: "Duplicate entry '…' for key 'slug'".
    let text = err.to_string();
    text.contains("Duplicate") && text.contains("slug")
}

/// POST /listings — validate everything, then insert a `draft`.
pub async fn create_draft(
    pool: &MySqlPool,
    seller_id: u64,
    body: CreateBody,
) -> Result<ListingOut, ApiError> {
    let mut errors = BTreeMap::new();
    let title = required_text(&body.title, "title", MAX_TITLE_CHARS, &mut errors);
    let description = required_text(&body.description, "description", MAX_DESCRIPTION_CHARS, &mut errors);
    let price_type = match &body.price_type {
        Some(t) if !t.trim().is_empty() => Some(t.trim().to_string()),
        _ => {
            errors.insert("price_type".to_string(), "required".to_string());
            None
        }
    };
    let price = price_type
        .as_deref()
        .and_then(|t| check_price(t, body.price_toman, &mut errors));
    if let Some(id) = body.category_id {
        check_category(pool, id, &mut errors).await;
    } else {
        errors.insert("category_id".to_string(), "required".to_string());
    }
    if let Some(id) = body.city_id {
        check_city(pool, id, &mut errors).await;
    } else {
        errors.insert("city_id".to_string(), "required".to_string());
    }
    if !errors.is_empty() {
        return Err(ApiError::Validation { fields: errors });
    }
    let (title, description, price_type, price) =
        (title.unwrap(), description.unwrap(), price_type.unwrap(), price.unwrap());
    let category_id = body.category_id.unwrap();
    let city_id = body.city_id.unwrap();

    let base = make_slug(&title);
    let now = Utc::now();
    // Numeric suffix on slug collision (plan Task 3).
    for attempt in 0..=5 {
        let slug = if attempt == 0 {
            base.clone()
        } else {
            format!("{base}-{}", attempt + 1)
        };
        let public_id = ulid::Ulid::new().to_string();
        let result = sqlx::query(
            "INSERT INTO listings (public_id, seller_id, category_id, city_id, title, \
             slug, description, price_toman, price_type, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'draft', ?, ?)",
        )
        .bind(&public_id)
        .bind(seller_id)
        .bind(category_id)
        .bind(city_id)
        .bind(&title)
        .bind(&slug)
        .bind(&description)
        .bind(price)
        .bind(&price_type)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await;
        match result {
            Ok(_) => {
                let row = sqlx::query("SELECT * FROM listings WHERE public_id = ?")
                    .bind(&public_id)
                    .fetch_one(pool)
                    .await
                    .map_err(|_| ApiError::Internal)?;
                return listing_out(&row);
            }
            Err(e) if is_duplicate_slug(&e) => continue,
            Err(_) => return Err(ApiError::Internal),
        }
    }
    Err(ApiError::Internal)
}

/// PATCH /listings/{public_id} — owner only, `draft`/`rejected` only.
pub async fn edit_listing(
    pool: &MySqlPool,
    seller_id: u64,
    public_id: &str,
    body: EditBody,
) -> Result<ListingOut, ApiError> {
    let row = own_listing(pool, seller_id, public_id).await?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    if status != "draft" && status != "rejected" {
        let mut fields = BTreeMap::new();
        fields.insert("status".to_string(), "not_editable".to_string());
        return Err(ApiError::Validation { fields });
    }

    let mut errors = BTreeMap::new();
    // Patched text fields re-run the create-time validation; missing fields
    // are left alone (required_text errors exactly when it returns None).
    let title = match &body.title {
        None => None,
        Some(t) => required_text(&Some(t.clone()), "title", MAX_TITLE_CHARS, &mut errors),
    };
    let description = match &body.description {
        None => None,
        Some(d) => required_text(&Some(d.clone()), "description", MAX_DESCRIPTION_CHARS, &mut errors),
    };
    // Effective price inputs: patched values or the stored ones.
    let stored_type: String = row.try_get("price_type").map_err(|_| ApiError::Internal)?;
    let stored_price: Option<u64> = row.try_get("price_toman").map_err(|_| ApiError::Internal)?;
    let price_type = body.price_type.clone().unwrap_or(stored_type);
    let price_input = body.price_toman.unwrap_or_else(|| stored_price.map(|n| n as i64));
    let price = check_price(&price_type, price_input, &mut errors);
    if let Some(id) = body.category_id {
        check_category(pool, id, &mut errors).await;
    }
    if let Some(id) = body.city_id {
        check_city(pool, id, &mut errors).await;
    }
    if !errors.is_empty() {
        return Err(ApiError::Validation { fields: errors });
    }
    let price = price.unwrap();

    let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
    let new_title: String = title.unwrap_or_else(|| row.try_get("title").unwrap_or_default());
    let new_description: String =
        description.unwrap_or_else(|| row.try_get("description").unwrap_or_default());
    let new_category: u16 = body.category_id.unwrap_or_else(|| row.try_get("category_id").unwrap_or(0));
    let new_city: u32 = body.city_id.unwrap_or_else(|| row.try_get("city_id").unwrap_or(0));

    // A new title means a new slug (same collision retry, excluding own row).
    let old_title: String = row.try_get("title").map_err(|_| ApiError::Internal)?;
    let mut slug: String = row.try_get("slug").map_err(|_| ApiError::Internal)?;
    if new_title != old_title {
        let base = make_slug(&new_title);
        slug = base.clone();
        for attempt in 1..=5 {
            let candidate = if attempt == 1 { base.clone() } else { format!("{base}-{attempt}") };
            let clash: Option<u64> = sqlx::query_scalar("SELECT id FROM listings WHERE slug = ? AND id <> ?")
                .bind(&candidate)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|_| ApiError::Internal)?;
            if clash.is_none() {
                slug = candidate;
                break;
            }
        }
    }

    sqlx::query(
        "UPDATE listings SET category_id = ?, city_id = ?, title = ?, slug = ?, \
         description = ?, price_toman = ?, price_type = ?, updated_at = ? WHERE id = ?",
    )
    .bind(new_category)
    .bind(new_city)
    .bind(&new_title)
    .bind(&slug)
    .bind(&new_description)
    .bind(price)
    .bind(&price_type)
    .bind(Utc::now())
    .bind(id)
    .execute(pool)
    .await
    .map_err(|_| ApiError::Internal)?;

    let row = sqlx::query("SELECT * FROM listings WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    listing_out(&row)
}

/// Owner state transitions: `submit` (draft|rejected → pending, needs ≥ 1
/// image), `mark-sold` (active → sold), `archive` (active → archived),
/// `delete` (soft-deletes draft|rejected|archived). Anything else → 400.
pub async fn transition(
    pool: &MySqlPool,
    seller_id: u64,
    public_id: &str,
    to: &str,
) -> Result<ListingOut, ApiError> {
    let row = own_listing(pool, seller_id, public_id).await?;
    let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    let now = Utc::now();

    match to {
        "submit" => {
            if status != "draft" && status != "rejected" {
                return Err(bad_transition());
            }
            let images: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM listing_images WHERE listing_id = ?")
                .bind(id)
                .fetch_one(pool)
                .await
                .map_err(|_| ApiError::Internal)?;
            if images < 1 {
                let mut fields = BTreeMap::new();
                fields.insert("images".to_string(), "required".to_string());
                return Err(ApiError::Validation { fields });
            }
            // A resubmit starts a fresh review: the old reason is cleared.
            sqlx::query(
                "UPDATE listings SET status = 'pending', submitted_at = ?, \
                 rejection_reason = NULL, updated_at = ? WHERE id = ?",
            )
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|_| ApiError::Internal)?;
        }
        "mark-sold" => {
            if status != "active" {
                return Err(bad_transition());
            }
            sqlx::query("UPDATE listings SET status = 'sold', sold_at = ?, updated_at = ? WHERE id = ?")
                .bind(now)
                .bind(now)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|_| ApiError::Internal)?;
        }
        "archive" => {
            if status != "active" {
                return Err(bad_transition());
            }
            sqlx::query("UPDATE listings SET status = 'archived', updated_at = ? WHERE id = ?")
                .bind(now)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|_| ApiError::Internal)?;
        }
        "delete" => {
            if status != "draft" && status != "rejected" && status != "archived" {
                return Err(bad_transition());
            }
            sqlx::query("UPDATE listings SET deleted_at = ?, updated_at = ? WHERE id = ?")
                .bind(now)
                .bind(now)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|_| ApiError::Internal)?;
        }
        _ => return Err(ApiError::Internal),
    }

    let row = sqlx::query("SELECT * FROM listings WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    listing_out(&row)
}

fn bad_transition() -> ApiError {
    let mut fields = BTreeMap::new();
    fields.insert("status".to_string(), "invalid_transition".to_string());
    ApiError::Validation { fields }
}

/// GET /me/listings — the seller's own listings, newest first, optional
/// status filter (`all` default). No cursor: seller lists are small at Rasht
/// scale (ponytail: cursor pagination deferred to the dashboard follow-up).
pub async fn my_listings(
    pool: &MySqlPool,
    seller_id: u64,
    status: &str,
) -> Result<Vec<ListingOut>, ApiError> {
    const ALLOWED: [&str; 5] = ["pending", "active", "rejected", "sold", "archived"];
    let rows = if status == "all" || status.is_empty() {
        sqlx::query(
            "SELECT * FROM listings WHERE seller_id = ? AND deleted_at IS NULL \
             ORDER BY updated_at DESC, id DESC LIMIT 50",
        )
        .bind(seller_id)
        .fetch_all(pool)
        .await
        .map_err(|_| ApiError::Internal)?
    } else if ALLOWED.contains(&status) {
        sqlx::query(
            "SELECT * FROM listings WHERE seller_id = ? AND status = ? AND deleted_at IS NULL \
             ORDER BY updated_at DESC, id DESC LIMIT 50",
        )
        .bind(seller_id)
        .bind(status)
        .fetch_all(pool)
        .await
        .map_err(|_| ApiError::Internal)?
    } else {
        let mut fields = BTreeMap::new();
        fields.insert("status".to_string(), "unknown_status".to_string());
        return Err(ApiError::Validation { fields });
    };
    rows.iter().map(listing_out).collect()
}

// --- Task 4: public reads (feed, detail, sitemap) ----------------------------

/// Opaque keyset cursor: `<published-or-updated-micros>:<id>`. Plain composite
/// text, no encoding crate (ponytail: base64 would cost a dependency for zero
/// privacy — the values are ordering keys, documented as opaque).
pub fn encode_cursor(ts: &chrono::DateTime<Utc>, id: u64) -> String {
    format!("{}:{id}", ts.timestamp_micros())
}

pub fn decode_cursor(raw: &str) -> Result<(chrono::DateTime<Utc>, u64), ApiError> {
    let (micros, id) = raw.split_once(':').ok_or_else(|| {
        let mut fields = BTreeMap::new();
        fields.insert("cursor".to_string(), "invalid_cursor".to_string());
        ApiError::Validation { fields }
    })?;
    let ts = micros
        .parse::<i64>()
        .ok()
        .and_then(chrono::DateTime::from_timestamp_micros)
        .ok_or_else(|| {
            let mut fields = BTreeMap::new();
            fields.insert("cursor".to_string(), "invalid_cursor".to_string());
            ApiError::Validation { fields }
        })?;
    let id = id.parse::<u64>().map_err(|_| {
        let mut fields = BTreeMap::new();
        fields.insert("cursor".to_string(), "invalid_cursor".to_string());
        ApiError::Validation { fields }
    })?;
    Ok((ts, id))
}

pub fn page_limit(raw: Option<&str>) -> usize {
    raw.and_then(|s| s.parse::<usize>().ok()).unwrap_or(20).clamp(1, 50)
}

/// Resolves an optional slug filter to its id: unknown slugs are a client
/// error, never an empty list.
async fn filter_id(
    pool: &MySqlPool,
    table: &str,
    slug: &str,
    field: &str,
    reason: &str,
) -> Result<Option<u64>, ApiError> {
    // Fixed table names from the caller, never user input.
    let sql = format!("SELECT id FROM {table} WHERE slug = ?");
    let id: Option<u64> = sqlx::query_scalar(&sql)
        .bind(slug)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::Internal)?;
    match id {
        Some(id) => Ok(Some(id)),
        None => {
            let mut fields = BTreeMap::new();
            fields.insert(field.to_string(), reason.to_string());
            Err(ApiError::Validation { fields })
        }
    }
}

fn card_from_row(row: &sqlx::mysql::MySqlRow) -> Result<serde_json::Value, ApiError> {
    let sum: u32 = row.try_get("rating_sum").map_err(|_| ApiError::Internal)?;
    let count: u32 = row.try_get("rating_count").map_err(|_| ApiError::Internal)?;
    let average = if count == 0 {
        serde_json::json!(0)
    } else {
        serde_json::json!(sum as f64 / count as f64)
    };
    let cover: Option<String> = row.try_get("cover").map_err(|_| ApiError::Internal)?;
    let published_at: chrono::DateTime<Utc> =
        row.try_get("published_at").map_err(|_| ApiError::Internal)?;
    Ok(serde_json::json!({
        "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
        "slug": row.try_get::<String, _>("slug").map_err(|_| ApiError::Internal)?,
        "title": row.try_get::<String, _>("title").map_err(|_| ApiError::Internal)?,
        "price_toman": row.try_get::<Option<u64>, _>("price_toman").map_err(|_| ApiError::Internal)?,
        "price_type": row.try_get::<String, _>("price_type").map_err(|_| ApiError::Internal)?,
        "city": row.try_get::<String, _>("city").map_err(|_| ApiError::Internal)?,
        "province": row.try_get::<String, _>("province").map_err(|_| ApiError::Internal)?,
        "category": row.try_get::<String, _>("category").map_err(|_| ApiError::Internal)?,
        "cover_image": cover.map(|k| format!("/media/{k}")),
        "rating_average": average,
        "rating_count": count,
        "published_at": published_at.to_rfc3339(),
    }))
}

/// GET /listings — `active` only, newest first, keyset on (published_at, id).
/// Filters: `category`, `city`, `province` (slugs). Page size caps at 50.
pub async fn feed(
    pool: &MySqlPool,
    category: Option<&str>,
    city: Option<&str>,
    province: Option<&str>,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let limit = page_limit(limit_raw);
    let mut sql = "SELECT l.id, l.public_id, l.slug, l.title, l.price_toman, l.price_type, \
         l.rating_sum, l.rating_count, l.published_at, c.name_fa AS city, \
         p.name_fa AS province, cat.name_fa AS category, \
         (SELECT storage_key FROM listing_images WHERE listing_id = l.id \
          ORDER BY sort_order LIMIT 1) AS cover \
         FROM listings l JOIN cities c ON c.id = l.city_id \
         JOIN provinces p ON p.id = c.province_id \
         JOIN categories cat ON cat.id = l.category_id \
         WHERE l.status = 'active' AND l.deleted_at IS NULL"
        .to_string();
    // Fixed fragments only; every value is bound, never interpolated.
    // Unknown filter slugs are a client error (validated up front).
    if let Some(slug) = category {
        filter_id(pool, "categories", slug, "category", "unknown_category").await?;
        sql.push_str(" AND cat.slug = ?");
    }
    if let Some(slug) = city {
        filter_id(pool, "cities", slug, "city", "unknown_city").await?;
        sql.push_str(" AND c.slug = ?");
    }
    if let Some(slug) = province {
        filter_id(pool, "provinces", slug, "province", "unknown_province").await?;
        sql.push_str(" AND p.slug = ?");
    }
    feed_query(pool, &sql, category, city, province, cursor, limit).await
}

async fn feed_query(
    pool: &MySqlPool,
    base: &str,
    category: Option<&str>,
    city: Option<&str>,
    province: Option<&str>,
    cursor: Option<&str>,
    limit: usize,
) -> Result<serde_json::Value, ApiError> {
    let mut sql = base.to_string();
    // Keyset: rows strictly older than the cursor (ties break on id).
    let anchor: Option<(chrono::DateTime<Utc>, u64)> =
        cursor.map(decode_cursor).transpose()?;
    if anchor.is_some() {
        sql.push_str(" AND (l.published_at, l.id) < (?, ?)");
    }
    sql.push_str(" ORDER BY l.published_at DESC, l.id DESC LIMIT ?");
    let mut q = sqlx::query(&sql);
    if let Some(slug) = category {
        q = q.bind(slug);
    }
    if let Some(slug) = city {
        q = q.bind(slug);
    }
    if let Some(slug) = province {
        q = q.bind(slug);
    }
    if let Some((ts, id)) = anchor {
        q = q.bind(ts).bind(id);
    }
    q = q.bind((limit + 1) as i64);
    let rows = q.fetch_all(pool).await.map_err(|_| ApiError::Internal)?;
    // Track each card with its (published_at, id) for the next cursor.
    let mut with_keys: Vec<(serde_json::Value, chrono::DateTime<Utc>, u64)> =
        Vec::with_capacity(rows.len());
    for row in &rows {
        let ts: chrono::DateTime<Utc> = row.try_get("published_at").map_err(|_| ApiError::Internal)?;
        let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
        with_keys.push((card_from_row(row)?, ts, id));
    }
    let has_more = with_keys.len() > limit;
    with_keys.truncate(limit);
    let next_cursor = if has_more {
        with_keys.last().map(|(_, ts, id)| encode_cursor(ts, *id))
    } else {
        None
    };
    let data: Vec<serde_json::Value> = with_keys.into_iter().map(|(item, _, _)| item).collect();
    Ok(serde_json::json!({
        "data": data,
        "page": { "next_cursor": next_cursor, "has_more": has_more },
    }))
}

/// GET /listings/{slug} — public detail. `active` → 200; `removed` or
/// soft-deleted → 410 `gone`; anything else (draft/pending/rejected/unknown)
/// → 404 with no existence signal.
pub async fn detail(pool: &MySqlPool, slug: &str) -> Result<serde_json::Value, ApiError> {
    let row = sqlx::query(
        "SELECT l.*, c.name_fa AS city, p.name_fa AS province, cat.name_fa AS category, \
         u.public_id AS seller_public_id, u.display_name AS seller_name \
         FROM listings l JOIN cities c ON c.id = l.city_id \
         JOIN provinces p ON p.id = c.province_id \
         JOIN categories cat ON cat.id = l.category_id \
         JOIN users u ON u.id = l.seller_id WHERE l.slug = ?",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?
    .ok_or(ApiError::NotFound)?;
    let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
    let deleted: Option<chrono::DateTime<Utc>> =
        row.try_get("deleted_at").map_err(|_| ApiError::Internal)?;
    if status == "removed" || deleted.is_some() {
        return Err(ApiError::Gone);
    }
    if status != "active" {
        return Err(ApiError::NotFound);
    }
    let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
    let images = sqlx::query(
        "SELECT storage_key, alt_text FROM listing_images WHERE listing_id = ? ORDER BY sort_order",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    let images: Vec<serde_json::Value> = images
        .iter()
        .map(|img| {
            let key: String = img.try_get("storage_key").map_err(|_| ApiError::Internal)?;
            let alt: Option<String> = img.try_get("alt_text").map_err(|_| ApiError::Internal)?;
            Ok(serde_json::json!({ "url": format!("/media/{key}"), "alt_text": alt }))
        })
        .collect::<Result<_, ApiError>>()?;
    let sum: u32 = row.try_get("rating_sum").map_err(|_| ApiError::Internal)?;
    let count: u32 = row.try_get("rating_count").map_err(|_| ApiError::Internal)?;
    let published_at: chrono::DateTime<Utc> =
        row.try_get("published_at").map_err(|_| ApiError::Internal)?;
    Ok(serde_json::json!({
        "public_id": row.try_get::<String, _>("public_id").map_err(|_| ApiError::Internal)?,
        "slug": row.try_get::<String, _>("slug").map_err(|_| ApiError::Internal)?,
        "title": row.try_get::<String, _>("title").map_err(|_| ApiError::Internal)?,
        "description": row.try_get::<String, _>("description").map_err(|_| ApiError::Internal)?,
        "price_toman": row.try_get::<Option<u64>, _>("price_toman").map_err(|_| ApiError::Internal)?,
        "price_type": row.try_get::<String, _>("price_type").map_err(|_| ApiError::Internal)?,
        "city": row.try_get::<String, _>("city").map_err(|_| ApiError::Internal)?,
        "province": row.try_get::<String, _>("province").map_err(|_| ApiError::Internal)?,
        "category": row.try_get::<String, _>("category").map_err(|_| ApiError::Internal)?,
        "images": images,
        "seller": {
            "public_id": row.try_get::<String, _>("seller_public_id").map_err(|_| ApiError::Internal)?,
            "display_name": row.try_get::<Option<String>, _>("seller_name").map_err(|_| ApiError::Internal)?,
        },
        "rating_average": if count == 0 { serde_json::json!(0) } else { serde_json::json!(sum as f64 / count as f64) },
        "rating_count": count,
        "comment_count": row.try_get::<u32, _>("comment_count").map_err(|_| ApiError::Internal)?,
        "published_at": published_at.to_rfc3339(),
    }))
}

/// GET /sitemap/listings — active slugs + `updated_at`, same page envelope.
pub async fn sitemap(
    pool: &MySqlPool,
    cursor: Option<&str>,
    limit_raw: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let limit = page_limit(limit_raw);
    let anchor: Option<(chrono::DateTime<Utc>, u64)> =
        cursor.map(decode_cursor).transpose()?;
    let mut sql = "SELECT id, slug, updated_at FROM listings \
         WHERE status = 'active' AND deleted_at IS NULL"
        .to_string();
    if anchor.is_some() {
        sql.push_str(" AND (updated_at, id) < (?, ?)");
    }
    sql.push_str(" ORDER BY updated_at DESC, id DESC LIMIT ?");
    let mut q = sqlx::query(&sql);
    if let Some((ts, id)) = anchor {
        q = q.bind(ts).bind(id);
    }
    q = q.bind((limit + 1) as i64);
    let rows = q.fetch_all(pool).await.map_err(|_| ApiError::Internal)?;
    let mut entries: Vec<(serde_json::Value, chrono::DateTime<Utc>, u64)> =
        Vec::with_capacity(rows.len());
    for row in &rows {
        let slug: String = row.try_get("slug").map_err(|_| ApiError::Internal)?;
        let updated_at: chrono::DateTime<Utc> =
            row.try_get("updated_at").map_err(|_| ApiError::Internal)?;
        let id: u64 = row.try_get("id").map_err(|_| ApiError::Internal)?;
        entries.push((
            serde_json::json!({ "url": format!("/listings/{slug}"), "updated_at": updated_at.to_rfc3339() }),
            updated_at,
            id,
        ));
    }
    let has_more = entries.len() > limit;
    entries.truncate(limit);
    let next_cursor = if has_more {
        entries.last().map(|(_, ts, id)| encode_cursor(ts, *id))
    } else {
        None
    };
    let data: Vec<serde_json::Value> = entries.into_iter().map(|(item, _, _)| item).collect();
    Ok(serde_json::json!({
        "data": data,
        "page": { "next_cursor": next_cursor, "has_more": has_more },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_from_persian_title_keeps_words_joined_by_dashes() {
        assert_eq!(make_slug("فرش دستباف تبریز"), "فرش-دستباف-تبریز");
    }

    #[test]
    fn slug_collapses_whitespace_and_strips_symbols() {
        assert_eq!(make_slug("  مبل   راحتی!! "), "مبل-راحتی");
    }

    #[test]
    fn slug_falls_back_when_nothing_survives() {
        assert_eq!(make_slug("!!!"), "listing");
    }

    #[test]
    fn slug_truncates_to_180_chars() {
        let long = "x".repeat(300);
        assert_eq!(make_slug(&long).chars().count(), 180);
    }
}
