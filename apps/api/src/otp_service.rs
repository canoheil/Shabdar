//! OTP challenge service: business rules for /auth/otp/request and
//! /auth/otp/verify (docs/PHASE1_PLAN.md, Tasks 8–9). Handlers stay thin;
//! every rule lives here. All SQL is sqlx runtime queries with bound
//! parameters, and every database error maps to Internal (never leaks SQL).

use chrono::{Duration, Utc};
use sqlx::MySqlPool;

use crate::config::Config;
use crate::error::ApiError;
use crate::otp;
use crate::phone;

/// What a successful challenge creation returns: the public id only.
pub struct ChallengeCreated {
    pub public_id: String,
}

/// Create an OTP challenge for a phone + purpose (Task 8 rules): normalize
/// the phone, validate the purpose, enforce the resend cooldown, then insert
/// the challenge with the HMAC of the code (never the plaintext).
pub async fn create_challenge(
    pool: &MySqlPool,
    cfg: &Config,
    phone_raw: &str,
    purpose: &str,
    code: &str,
) -> Result<ChallengeCreated, ApiError> {
    let phone_e164 =
        phone::normalize_iranian_mobile(phone_raw).map_err(|_| ApiError::InvalidPhone)?;
    if purpose != "login" && purpose != "register" {
        return Err(ApiError::InvalidPurpose);
    }

    // Cooldown: the newest challenge for (phone, purpose) must be older than
    // the resend window. ponytail: the check + INSERT below is not atomic —
    // two concurrent requests inside the window can both pass; the upgrade is
    // a unique-constraint trick or SELECT … FOR UPDATE once contention exists.
    let newest = sqlx::query_as::<_, (chrono::DateTime<Utc>,)>(
        "SELECT created_at FROM otp_challenges
         WHERE phone_e164 = ? AND purpose = ?
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .bind(&phone_e164)
    .bind(purpose)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?;

    if let Some((created_at,)) = newest {
        if created_at > Utc::now() - Duration::seconds(cfg.otp_resend_secs as i64) {
            return Err(ApiError::RateLimited);
        }
    }

    let public_id = ulid::Ulid::new().to_string();
    let code_hash = otp::hash_code(code, &cfg.otp_pepper);
    let expires_at = Utc::now() + Duration::seconds(cfg.otp_ttl_secs as i64);

    // send_count defaults to 1 and request_ip_hash stays NULL (Phase 1).
    sqlx::query(
        "INSERT INTO otp_challenges
         (public_id, phone_e164, purpose, code_hash, expires_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&public_id)
    .bind(&phone_e164)
    .bind(purpose)
    .bind(&code_hash[..])
    .bind(expires_at)
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|_| ApiError::Internal)?;

    Ok(ChallengeCreated { public_id })
}
