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

/// Double-submit check: the X-CSRF-Token header must equal the shabdar_csrf
/// cookie value (both present, equal). Any mismatch or absence → 403
/// `csrf_failed`; the calling handler must not mutate anything.
pub fn check_csrf(headers: &HeaderMap) -> Result<(), ApiError> {
    let cookie =
        crate::session::cookie_value(headers, CSRF_COOKIE_NAME).ok_or(ApiError::CsrfFailed)?;
    let token = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::CsrfFailed)?;
    if token == cookie {
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
    fn check_csrf_passes_only_on_matching_header_and_cookie() {
        let good = headers(Some("shabdar_csrf=tok123"), Some("tok123"));
        assert!(check_csrf(&good).is_ok(), "matching pair must pass");
        assert!(
            check_csrf(&good).is_ok(),
            "check must be repeatable (GETs skip it; mutations call once)"
        );

        // Missing header, wrong header, missing cookie, empty cookie.
        for map in [
            headers(Some("shabdar_csrf=tok123"), None),
            headers(Some("shabdar_csrf=tok123"), Some("tok124")),
            headers(None, Some("tok123")),
            headers(Some("shabdar_csrf="), Some("")),
        ] {
            assert!(matches!(
                check_csrf(&map),
                Err(crate::error::ApiError::CsrfFailed)
            ));
        }
    }
}
