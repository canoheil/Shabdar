//! Double-submit CSRF (docs/PHASE2_PLAN.md "CSRF proposal", approved
//! 2026-10-10). Every state-changing request that uses a session must carry
//! header `X-CSRF-Token` equal to the `shabdar_csrf` cookie value. Today that
//! is PATCH /api/v1/me and POST /api/v1/auth/logout; every Phase 3+ mutation
//! calls `check_csrf` the same way (see routes.rs). OTP endpoints and all
//! GETs are exempt.

use axum::http::HeaderMap;

use crate::config::Config;
use crate::error::ApiError;

/// The csrf cookie name — part of the frozen design (not configurable).
pub const CSRF_COOKIE_NAME: &str = "shabdar_csrf";

/// The exact Set-Cookie value for the csrf cookie: NOT HttpOnly (the browser
/// must read it for double-submit), `Secure` only when configured.
pub fn build_csrf_cookie(cfg: &Config, token: &str) -> String {
    let mut cookie = format!("{CSRF_COOKIE_NAME}={token}; Path=/; SameSite=Lax");
    if cfg.cookie_secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// The clearing Set-Cookie (Max-Age=0), same flags as build_csrf_cookie.
pub fn build_csrf_clear_cookie(cfg: &Config) -> String {
    let mut cookie = format!("{CSRF_COOKIE_NAME}=; Path=/; SameSite=Lax; Max-Age=0");
    if cfg.cookie_secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// Every `shabdar_csrf` value across all `Cookie` headers. The check needs
/// the complete list: exactly one value is required, because a duplicated
/// cookie (cookie tossing) must not be silently ignored.
fn csrf_cookie_values(headers: &HeaderMap) -> Vec<&str> {
    let mut values = Vec::new();
    for header in headers.get_all(axum::http::header::COOKIE) {
        let Ok(text) = header.to_str() else {
            continue;
        };
        for pair in text.split(';') {
            if let Some((name, value)) = pair.trim().split_once('=') {
                if name == CSRF_COOKIE_NAME {
                    values.push(value);
                }
            }
        }
    }
    values
}

/// Constant-time byte equality (dependency set is frozen — no `subtle`
/// crate). Lengths are compared first; token lengths are not secret (ULIDs
/// are fixed-width) and an early reject only reveals "different length".
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Double-submit check: exactly one non-blank `shabdar_csrf` cookie value
/// and a non-blank `X-CSRF-Token` header, equal in constant time. Empty or
/// blank tokens never match (`"" == ""` is not proof of anything), duplicated
/// cookies are rejected (cookie tossing), and any other mismatch or absence
/// → 403 `csrf_failed`; the calling handler must not mutate anything.
pub fn check_csrf(headers: &HeaderMap) -> Result<(), ApiError> {
    let values = csrf_cookie_values(headers);
    if values.len() != 1 {
        return Err(ApiError::CsrfFailed);
    }
    let cookie = values[0];
    let Some(token) = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
    else {
        return Err(ApiError::CsrfFailed);
    };
    if cookie.trim().is_empty() || token.trim().is_empty() {
        return Err(ApiError::CsrfFailed);
    }
    if ct_eq(cookie.as_bytes(), token.as_bytes()) {
        Ok(())
    } else {
        Err(ApiError::CsrfFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    use crate::config::Config;

    /// Config with the contract defaults; only `cookie_secure` varies.
    fn test_cfg(cookie_secure: bool) -> Config {
        Config {
            bind_addr: "127.0.0.1:0".to_string(),
            trust_proxy: false,
            database_url: "mysql://u:p@127.0.0.1:3306/shabdar_test".to_string(),
            otp_pepper: "test-pepper".to_string(),
            otp_dev_mode: false,
            otp_ttl_secs: 120,
            otp_resend_secs: 60,
            otp_max_attempts: 5,
            otp_request_ip_limit: 5,
            otp_request_ip_window_secs: 600,
            otp_verify_ip_limit: 20,
            otp_verify_ip_window_secs: 600,
            otp_phone_daily_limit: 10,
            otp_phone_daily_window_secs: 86_400,
            otp_sms_global_limit: 200,
            otp_sms_global_window_secs: 3_600,
            session_ttl_secs: 2_592_000,
            cookie_name: "shabdar_session".to_string(),
            cookie_secure,
            media_dir: "./data/media".to_string(),
            max_image_bytes: 5_242_880,
            max_images_per_listing: 8,
        }
    }

    /// Header map with the two inputs of the double-submit check.
    fn headers(cookie: Option<&str>, token: Option<&str>) -> HeaderMap {
        let mut map = HeaderMap::new();
        if let Some(value) = cookie {
            map.insert(axum::http::header::COOKIE, value.parse().unwrap());
        }
        if let Some(value) = token {
            map.insert("x-csrf-token", value.parse().unwrap());
        }
        map
    }

    #[test]
    fn csrf_cookie_is_not_httponly_and_has_the_approved_flags() {
        // NOT HttpOnly — the browser must read it for double-submit.
        assert_eq!(
            build_csrf_cookie(&test_cfg(false), "tok123"),
            "shabdar_csrf=tok123; Path=/; SameSite=Lax"
        );
        assert_eq!(
            build_csrf_cookie(&test_cfg(true), "tok123"),
            "shabdar_csrf=tok123; Path=/; SameSite=Lax; Secure"
        );
    }

    #[test]
    fn csrf_clear_cookie_has_the_approved_flags() {
        assert_eq!(
            build_csrf_clear_cookie(&test_cfg(false)),
            "shabdar_csrf=; Path=/; SameSite=Lax; Max-Age=0"
        );
        assert_eq!(
            build_csrf_clear_cookie(&test_cfg(true)),
            "shabdar_csrf=; Path=/; SameSite=Lax; Max-Age=0; Secure"
        );
    }

    #[test]
    fn check_csrf_accepts_a_matching_pair_and_real_cookie_shapes() {
        let good = headers(Some("shabdar_csrf=tok123"), Some("tok123"));
        assert!(check_csrf(&good).is_ok(), "matching pair must pass");
        assert!(
            check_csrf(&good).is_ok(),
            "check must be repeatable (GETs skip it; mutations call once)"
        );
        // The real browser/proxy shape: session cookie alongside the csrf one.
        let both = headers(
            Some("shabdar_session=abc; shabdar_csrf=tok123"),
            Some("tok123"),
        );
        assert!(
            check_csrf(&both).is_ok(),
            "the session cookie must not confuse the scan"
        );
    }

    #[test]
    fn check_csrf_rejects_missing_header() {
        let map = headers(Some("shabdar_csrf=tok123"), None);
        assert!(matches!(check_csrf(&map), Err(ApiError::CsrfFailed)));
    }

    #[test]
    fn check_csrf_rejects_missing_cookie() {
        let map = headers(None, Some("tok123"));
        assert!(matches!(check_csrf(&map), Err(ApiError::CsrfFailed)));
    }

    #[test]
    fn check_csrf_rejects_empty_tokens() {
        // The regression: "" == "" used to pass.
        let map = headers(Some("shabdar_csrf="), Some(""));
        assert!(matches!(check_csrf(&map), Err(ApiError::CsrfFailed)));
    }

    #[test]
    fn check_csrf_rejects_one_sided_empty_tokens() {
        assert!(matches!(
            check_csrf(&headers(Some("shabdar_csrf="), Some("tok123"))),
            Err(ApiError::CsrfFailed)
        ));
        assert!(matches!(
            check_csrf(&headers(Some("shabdar_csrf=tok123"), Some(""))),
            Err(ApiError::CsrfFailed)
        ));
    }

    #[test]
    fn check_csrf_rejects_blank_whitespace_tokens() {
        let map = headers(Some("shabdar_csrf=   "), Some("  "));
        assert!(matches!(check_csrf(&map), Err(ApiError::CsrfFailed)));
        let map = headers(Some("shabdar_csrf=tok123"), Some("   "));
        assert!(matches!(check_csrf(&map), Err(ApiError::CsrfFailed)));
    }

    #[test]
    fn check_csrf_rejects_mismatched_tokens() {
        let map = headers(Some("shabdar_csrf=tok123"), Some("tok124"));
        assert!(matches!(check_csrf(&map), Err(ApiError::CsrfFailed)));
    }

    #[test]
    fn check_csrf_rejects_different_length_tokens() {
        let map = headers(Some("shabdar_csrf=tok123"), Some("tok123-extra"));
        assert!(matches!(check_csrf(&map), Err(ApiError::CsrfFailed)));
    }

    #[test]
    fn check_csrf_rejects_duplicated_cookies() {
        // Two shabdar_csrf values in one Cookie header (cookie tossing).
        let mut map = HeaderMap::new();
        map.insert(
            axum::http::header::COOKIE,
            "shabdar_csrf=tok123; shabdar_csrf=tok123".parse().unwrap(),
        );
        map.insert("x-csrf-token", "tok123".parse().unwrap());
        assert!(matches!(check_csrf(&map), Err(ApiError::CsrfFailed)));

        // The same value split across two Cookie headers.
        let mut map = HeaderMap::new();
        map.append(
            axum::http::header::COOKIE,
            "shabdar_csrf=tok123".parse().unwrap(),
        );
        map.append(
            axum::http::header::COOKIE,
            "shabdar_session=abc; shabdar_csrf=tok123".parse().unwrap(),
        );
        map.insert("x-csrf-token", "tok123".parse().unwrap());
        assert!(matches!(check_csrf(&map), Err(ApiError::CsrfFailed)));
    }
}
