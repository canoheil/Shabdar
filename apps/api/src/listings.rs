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

fn listing_out(row: &sqlx::mysql::MySqlRow) -> Result<ListingOut, ApiError> {
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
async fn own_listing(
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
