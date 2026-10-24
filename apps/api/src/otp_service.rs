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

/// A request that passed the per-phone rules: normalization, purpose, the
/// Phase 5 daily cap and the resend cooldown. Splitting this out lets the
/// handler reserve the global SMS budget *after* a request is known to be
/// legitimate, so a rejected request never spends an SMS slot.
pub struct ValidatedRequest {
    pub phone_e164: String,
    pub purpose: String,
}

/// The rules that only need the request itself (no insert): normalize the
/// phone, validate the purpose, enforce the per-phone daily cap and the
/// resend cooldown (docs/PHASE5_PLAN.md "Rate limits").
pub async fn validate_request(
    pool: &MySqlPool,
    cfg: &Config,
    phone_raw: &str,
    purpose: &str,
) -> Result<ValidatedRequest, ApiError> {
    let phone_e164 =
        phone::normalize_iranian_mobile(phone_raw).map_err(|_| ApiError::InvalidPhone)?;
    if purpose != "login" && purpose != "register" {
        return Err(ApiError::InvalidPurpose);
    }

    // Per-phone cap: every challenge for this number inside the window counts,
    // whatever the purpose, because each one costs an SMS. The index
    // idx_otp_phone_purpose_created covers the scan.
    if cfg.otp_phone_daily_limit > 0 {
        let cutoff = Utc::now() - Duration::seconds(cfg.otp_phone_daily_window_secs as i64);
        let (count, oldest) = sqlx::query_as::<_, (i64, Option<chrono::DateTime<Utc>>)>(
            "SELECT COUNT(*) AS n, MIN(created_at) AS oldest
             FROM otp_challenges
             WHERE phone_e164 = ? AND created_at > ?",
        )
        .bind(&phone_e164)
        .bind(cutoff)
        .fetch_one(pool)
        .await
        .map_err(|_| ApiError::Internal)?;

        if count >= i64::from(cfg.otp_phone_daily_limit) {
            // The oldest row in the window decides when the number frees up.
            let retry_after_secs = match oldest {
                Some(oldest) => (cfg.otp_phone_daily_window_secs as i64
                    - (Utc::now() - oldest).num_seconds())
                .max(1) as u64,
                None => u64::from(cfg.otp_phone_daily_window_secs).max(1),
            };
            return Err(ApiError::RateLimited {
                retry_after_secs: Some(retry_after_secs),
            });
        }
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
        let resend_window = Duration::seconds(cfg.otp_resend_secs as i64);
        if created_at > Utc::now() - resend_window {
            // The client is told exactly how long is left, never less than 1 s.
            let elapsed = (Utc::now() - created_at).num_seconds().max(0);
            let remaining = (cfg.otp_resend_secs as i64 - elapsed).max(1) as u64;
            return Err(ApiError::RateLimited {
                retry_after_secs: Some(remaining),
            });
        }
    }

    Ok(ValidatedRequest {
        phone_e164,
        purpose: purpose.to_string(),
    })
}

/// Inserts the challenge with the HMAC of the code (never the plaintext).
pub async fn insert_challenge(
    pool: &MySqlPool,
    cfg: &Config,
    request: &ValidatedRequest,
    code: &str,
) -> Result<ChallengeCreated, ApiError> {
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
    .bind(&request.phone_e164)
    .bind(&request.purpose)
    .bind(&code_hash[..])
    .bind(expires_at)
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|_| ApiError::Internal)?;

    Ok(ChallengeCreated { public_id })
}

/// Validate + insert in one call. Kept for callers (including test fixtures)
/// that do not reserve a global budget themselves.
pub async fn create_challenge(
    pool: &MySqlPool,
    cfg: &Config,
    phone_raw: &str,
    purpose: &str,
    code: &str,
) -> Result<ChallengeCreated, ApiError> {
    let request = validate_request(pool, cfg, phone_raw, purpose).await?;
    insert_challenge(pool, cfg, &request, code).await
}

/// The public user shape in verify responses (approved, plan review §3).
pub struct UserSummary {
    pub public_id: String,
    pub display_name: Option<String>,
    pub is_admin: bool,
}

/// What a successful verification returns: the user, whether it was just
/// created, and the raw session token (returned once, never stored).
pub struct VerifyOutcome {
    pub user: UserSummary,
    pub is_new_user: bool,
    pub session_token: String,
}

/// Internal user row: the numeric id (for the sessions FK) plus the summary.
struct UserRecord {
    id: u64,
    summary: UserSummary,
}

/// The challenge row verify needs, in SELECT order.
type ChallengeRow = (
    String,                        // phone_e164
    String,                        // purpose
    Vec<u8>,                       // code_hash
    u16,                           // attempt_count (SMALLINT UNSIGNED)
    chrono::DateTime<Utc>,         // expires_at
    Option<chrono::DateTime<Utc>>, // consumed_at
);

/// Verify an OTP challenge (Task 9 rules): single use, expiry, attempt
/// limit, constant-time code check, then purpose rules + session issue.
pub async fn verify_challenge(
    pool: &MySqlPool,
    cfg: &Config,
    challenge_id: &str,
    code: &str,
) -> Result<VerifyOutcome, ApiError> {
    // 1. Load by public_id. Unknown or already consumed → generic invalid_code
    //    (the response never says which — no probing).
    let row = sqlx::query_as::<_, ChallengeRow>(
        "SELECT phone_e164, purpose, code_hash, attempt_count, expires_at, consumed_at
         FROM otp_challenges
         WHERE public_id = ?",
    )
    .bind(challenge_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?;

    let Some((phone_e164, purpose, code_hash, attempt_count, expires_at, consumed_at)) = row
    else {
        return Err(ApiError::InvalidCode);
    };
    if consumed_at.is_some() {
        return Err(ApiError::InvalidCode);
    }
    // 2. Expired at or past expiry (boundary pinned by the plan's review focus).
    if expires_at <= Utc::now() {
        return Err(ApiError::ChallengeExpired);
    }
    // 3. Too many wrong attempts.
    if attempt_count >= cfg.otp_max_attempts {
        return Err(ApiError::ChallengeLocked);
    }
    // 4. Constant-time HMAC check; a wrong code burns one attempt.
    if !otp::verify_code(code, &cfg.otp_pepper, &code_hash) {
        sqlx::query("UPDATE otp_challenges SET attempt_count = attempt_count + 1 WHERE public_id = ?")
            .bind(challenge_id)
            .execute(pool)
            .await
            .map_err(|_| ApiError::Internal)?;
        return Err(ApiError::InvalidCode);
    }

    // 5. Success: the challenge is single-use.
    sqlx::query("UPDATE otp_challenges SET consumed_at = ? WHERE public_id = ?")
        .bind(Utc::now())
        .bind(challenge_id)
        .execute(pool)
        .await
        .map_err(|_| ApiError::Internal)?;

    // 6. Purpose rules: register creates the account, login requires one
    //    (approved: login never creates an account).
    let (user, is_new_user) = match purpose.as_str() {
        "register" => get_or_create_user(pool, &phone_e164).await?,
        _ => {
            // purpose is ENUM('login','register'); anything else is a data bug.
            let user = find_user(pool, &phone_e164).await?.ok_or(ApiError::NoAccount)?;
            (user, false)
        }
    };

    // 7. Issue the session; only the token's hash is stored.
    let session_token = crate::session::create_session(pool, cfg, user.id).await?;

    Ok(VerifyOutcome {
        user: user.summary,
        is_new_user,
        session_token,
    })
}

/// Load the active user for a phone number.
async fn find_user(pool: &MySqlPool, phone_e164: &str) -> Result<Option<UserRecord>, ApiError> {
    let row = sqlx::query_as::<_, (u64, String, Option<String>, bool)>(
        "SELECT id, public_id, display_name, is_admin
         FROM users
         WHERE phone_e164 = ? AND status = 'active'",
    )
    .bind(phone_e164)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    Ok(row.map(|(id, public_id, display_name, is_admin)| UserRecord {
        id,
        summary: UserSummary {
            public_id,
            display_name,
            is_admin,
        },
    }))
}

/// register purpose: the user if one exists, otherwise create it (ULID
/// public_id, no display name, active).
async fn get_or_create_user(
    pool: &MySqlPool,
    phone_e164: &str,
) -> Result<(UserRecord, bool), ApiError> {
    if let Some(user) = find_user(pool, phone_e164).await? {
        return Ok((user, false));
    }
    let public_id = ulid::Ulid::new().to_string();
    let now = Utc::now();
    // ponytail: SELECT-then-INSERT is not atomic — a concurrent duplicate
    // register surfaces as a UNIQUE(phone_e164) violation → Internal here.
    // Accepted: Phase 1 is single-user development (same spirit as the Task 8
    // cooldown race).
    let result = sqlx::query(
        "INSERT INTO users (public_id, phone_e164, status, created_at, updated_at)
         VALUES (?, ?, 'active', ?, ?)",
    )
    .bind(&public_id)
    .bind(phone_e164)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    Ok((
        UserRecord {
            id: result.last_insert_id(),
            summary: UserSummary {
                public_id,
                display_name: None,
                is_admin: false,
            },
        },
        true,
    ))
}
