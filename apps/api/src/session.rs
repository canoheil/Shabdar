//! Sessions: opaque cookie tokens, stored only as SHA-256 hashes
//! (docs/PHASE1_PLAN.md, Task 9; migration 0002). The raw token is returned
//! to the client once and never persisted.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::HeaderMap;
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use sqlx::Row;

use crate::config::Config;
use crate::error::ApiError;
use crate::state::AppState;

/// Generate a session token: 32 random bytes, hex-encoded (64 chars).
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("OS randomness source unavailable");
    to_hex(&bytes)
}

/// SHA-256 of the token — the only form ever stored (migration 0002).
pub fn hash_token(token: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hasher.finalize().into()
}

/// The exact Set-Cookie value: configured name, approved flags, `Secure`
/// only when the config says so (true behind HTTPS, false on localhost).
pub fn build_set_cookie(cfg: &Config, token: &str) -> String {
    let mut cookie = format!(
        "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        cfg.cookie_name, cfg.session_ttl_secs
    );
    if cfg.cookie_secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// Insert a session row (hash only) and return the raw token.
pub async fn create_session(
    pool: &MySqlPool,
    cfg: &Config,
    user_id: u64,
) -> Result<String, ApiError> {
    let token = generate_token();
    let token_hash = hash_token(&token);
    let expires_at = Utc::now() + Duration::seconds(cfg.session_ttl_secs as i64);
    sqlx::query(
        "INSERT INTO sessions (user_id, token_hash, expires_at, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(&token_hash[..])
    .bind(expires_at)
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|_| ApiError::Internal)?;
    Ok(token)
}

/// The authenticated user behind a valid `shabdar_session` cookie (Phase 2,
/// docs/PHASE2_PLAN.md Task 1). Rejection is deliberately generic: a missing
/// cookie, an unknown or expired session, and a non-active user all get the
/// same 401 `unauthorized` (ARCHITECTURE.md §7 — no probing signal).
pub struct CurrentUser {
    pub session_id: u64,
    pub user_id: u64,
    pub public_id: String,
    pub display_name: Option<String>,
    pub is_admin: bool,
}

/// Pull `name=value` out of the Cookie header(s); None when absent.
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    for header in headers.get_all(axum::http::header::COOKIE) {
        let Ok(text) = header.to_str() else {
            continue;
        };
        for pair in text.split(';') {
            if let Some((key, value)) = pair.trim().split_once('=') {
                if key == name {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Missing Cookie header or missing COOKIE_NAME entry → generic 401.
        let token =
            cookie_value(&parts.headers, &state.config.cookie_name).ok_or(ApiError::Unauthorized)?;
        let token_hash = hash_token(&token);
        // One joined lookup (plan Task 1): session + user in a single query.
        let row = sqlx::query(
            "SELECT s.id AS session_id, s.expires_at, u.id AS user_id, \
             u.public_id, u.display_name, u.is_admin, u.status \
             FROM sessions s JOIN users u ON u.id = s.user_id \
             WHERE s.token_hash = ?",
        )
        .bind(&token_hash[..])
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::Unauthorized)?;
        let expires_at: chrono::DateTime<Utc> =
            row.try_get("expires_at").map_err(|_| ApiError::Internal)?;
        // Expired at or past the boundary → generic 401. No lazy delete;
        // cleanup jobs are a later phase.
        if expires_at <= Utc::now() {
            return Err(ApiError::Unauthorized);
        }
        let status: String = row.try_get("status").map_err(|_| ApiError::Internal)?;
        if status != "active" {
            return Err(ApiError::Unauthorized);
        }
        Ok(CurrentUser {
            session_id: row.try_get("session_id").map_err(|_| ApiError::Internal)?,
            user_id: row.try_get("user_id").map_err(|_| ApiError::Internal)?,
            public_id: row.try_get("public_id").map_err(|_| ApiError::Internal)?,
            display_name: row.try_get("display_name").map_err(|_| ApiError::Internal)?,
            is_admin: row.try_get("is_admin").map_err(|_| ApiError::Internal)?,
        })
    }
}

/// Lowercase hex of a byte slice (no hex crate — five lines, per the plan's ladder).
fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    /// Config with the contract defaults; only `cookie_secure` varies.
    fn test_cfg(cookie_secure: bool) -> Config {
        Config {
            bind_addr: "127.0.0.1:0".to_string(),
            database_url: "mysql://u:p@127.0.0.1:3306/shabdar_test".to_string(),
            otp_pepper: "test-pepper".to_string(),
            otp_dev_mode: false,
            otp_ttl_secs: 120,
            otp_resend_secs: 60,
            otp_max_attempts: 5,
            session_ttl_secs: 2_592_000,
            cookie_name: "shabdar_session".to_string(),
            cookie_secure,
        }
    }

    #[test]
    fn generate_token_is_64_lowercase_hex_chars() {
        for _ in 0..10 {
            let token = generate_token();
            assert_eq!(token.len(), 64, "token must be 64 hex chars: {token}");
            assert!(
                token
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f')),
                "token must be lowercase hex: {token}"
            );
        }
    }

    #[test]
    fn hash_token_is_deterministic_sha256_and_not_the_token() {
        let token = "0123456789abcdef".repeat(4);
        assert_eq!(hash_token(&token), hash_token(&token), "deterministic");
        assert_ne!(
            hash_token(&token).as_slice(),
            token.as_bytes(),
            "the stored hash must differ from the raw token"
        );
        // Known answer: SHA-256("abc") pins the exact construction.
        let expected: [u8; 32] = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        assert_eq!(hash_token("abc"), expected);
    }

    #[test]
    fn set_cookie_has_the_approved_flags() {
        let cfg = test_cfg(false);
        assert_eq!(
            build_set_cookie(&cfg, "tok123"),
            "shabdar_session=tok123; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000"
        );
        let cfg = test_cfg(true);
        assert_eq!(
            build_set_cookie(&cfg, "tok123"),
            "shabdar_session=tok123; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000; Secure"
        );
    }

    #[test]
    fn clear_cookie_has_the_approved_flags() {
        // Flag parity with build_set_cookie (Review Focus 4), Max-Age=0.
        let cfg = test_cfg(false);
        assert_eq!(
            build_clear_cookie(&cfg),
            "shabdar_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
        );
        let cfg = test_cfg(true);
        assert_eq!(
            build_clear_cookie(&cfg),
            "shabdar_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; Secure"
        );
    }
}
