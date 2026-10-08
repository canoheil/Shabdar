//! Environment-based configuration (docs/PHASE1_PLAN.md "Config contract").
//!
//! Required: DATABASE_URL, OTP_PEPPER. Everything else has a default.
//! Secrets (database URL, pepper) live only here and must never be logged.

// Most fields are only read from Task 4 (pool) and Tasks 8–9 (OTP/session)
// onward; until then this silences per-field dead-code warnings instead of
// writing fake reads. Remove this attribute once all fields are consumed.
#[allow(dead_code)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    pub otp_pepper: String,
    pub otp_dev_mode: bool,
    pub otp_ttl_secs: u32,
    pub otp_resend_secs: u32,
    pub otp_max_attempts: u16,
    pub session_ttl_secs: u32,
    pub cookie_name: String,
    pub cookie_secure: bool,
}

impl Config {
    /// Reads all settings from the process environment. The first problem
    /// returns Err naming the offending variable.
    pub fn from_env() -> Result<Config, String> {
        Ok(Config {
            bind_addr: optional_string("BIND_ADDR", "127.0.0.1:8080"),
            database_url: required("DATABASE_URL")?,
            otp_pepper: required("OTP_PEPPER")?,
            otp_dev_mode: parse_bool("OTP_DEV_MODE", false)?,
            otp_ttl_secs: parse_u32("OTP_TTL_SECS", 120)?,
            otp_resend_secs: parse_u32("OTP_RESEND_SECS", 60)?,
            // u16 per the contract; reject values above u16::MAX, never wrap.
            otp_max_attempts: u16::try_from(parse_u32("OTP_MAX_ATTEMPTS", 5)?)
                .map_err(|_| "OTP_MAX_ATTEMPTS must fit in 0..=65535".to_string())?,
            session_ttl_secs: parse_u32("SESSION_TTL_SECS", 2_592_000)?,
            cookie_name: optional_string("COOKIE_NAME", "shabdar_session"),
            cookie_secure: parse_bool("COOKIE_SECURE", false)?,
        })
    }
}

/// A missing required variable is an error naming the variable (contract).
fn required(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("missing required environment variable {name}"))
}

fn optional_string(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Strict on purpose: a typo must not silently disable dev-mode or Secure cookies.
fn parse_bool(name: &str, default: bool) -> Result<bool, String> {
    match std::env::var(name) {
        Err(_) => Ok(default),
        Ok(v) => match v.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(format!("{name} must be true/false/1/0, got {v:?}")),
        },
    }
}

fn parse_u32(name: &str, default: u32) -> Result<u32, String> {
    match std::env::var(name) {
        Err(_) => Ok(default),
        Ok(v) => v
            .parse()
            .map_err(|_| format!("{name} must be a non-negative integer, got {v:?}")),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    // Env vars are process-global and cargo runs tests on multiple threads,
    // so every test that mutates the environment takes this lock.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// Every variable Config reads (keep in sync with from_env).
    const VARS: [&str; 10] = [
        "DATABASE_URL",
        "OTP_PEPPER",
        "BIND_ADDR",
        "OTP_DEV_MODE",
        "OTP_TTL_SECS",
        "OTP_RESEND_SECS",
        "OTP_MAX_ATTEMPTS",
        "SESSION_TTL_SECS",
        "COOKIE_NAME",
        "COOKIE_SECURE",
    ];

    /// Saves all VARS, clears them, and restores the saved values on drop.
    struct EnvGuard {
        saved: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn cleared() -> Self {
            let saved = VARS.iter().map(|&k| (k, std::env::var(k).ok())).collect();
            for &k in &VARS {
                std::env::remove_var(k);
            }
            EnvGuard { saved }
        }

        fn set(&self, key: &str, value: &str) {
            std::env::set_var(key, value);
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.saved {
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    /// Locks the env, clears it, then sets the two required variables.
    fn locked_required_env() -> (MutexGuard<'static, ()>, EnvGuard) {
        let lock = ENV_MUTEX.lock().unwrap();
        let env = EnvGuard::cleared();
        env.set("DATABASE_URL", "mysql://user:pass@127.0.0.1:3306/shabdar_test");
        env.set("OTP_PEPPER", "test-pepper");
        (lock, env)
    }

    #[test]
    fn defaults_when_only_required_vars_set() {
        let (_lock, _env) = locked_required_env();
        let cfg = super::Config::from_env().expect("required vars are set");
        assert_eq!(cfg.bind_addr, "127.0.0.1:8080");
        assert_eq!(
            cfg.database_url,
            "mysql://user:pass@127.0.0.1:3306/shabdar_test"
        );
        assert_eq!(cfg.otp_pepper, "test-pepper");
        assert!(!cfg.otp_dev_mode);
        assert_eq!(cfg.otp_ttl_secs, 120);
        assert_eq!(cfg.otp_resend_secs, 60);
        assert_eq!(cfg.otp_max_attempts, 5);
        assert_eq!(cfg.session_ttl_secs, 2_592_000);
        assert_eq!(cfg.cookie_name, "shabdar_session");
        assert!(!cfg.cookie_secure);
    }

    #[test]
    fn parses_set_values() {
        let (_lock, env) = locked_required_env();
        env.set("BIND_ADDR", "0.0.0.0:9000");
        env.set("OTP_DEV_MODE", "true");
        env.set("OTP_TTL_SECS", "90"); // plan's example: 90 must land in otp_ttl_secs
        env.set("OTP_RESEND_SECS", "30");
        env.set("OTP_MAX_ATTEMPTS", "3");
        env.set("SESSION_TTL_SECS", "3600");
        env.set("COOKIE_NAME", "test_cookie");
        env.set("COOKIE_SECURE", "1");
        let cfg = super::Config::from_env().expect("valid env");
        assert_eq!(cfg.bind_addr, "0.0.0.0:9000");
        assert!(cfg.otp_dev_mode);
        assert_eq!(cfg.otp_ttl_secs, 90);
        assert_eq!(cfg.otp_resend_secs, 30);
        assert_eq!(cfg.otp_max_attempts, 3);
        assert_eq!(cfg.session_ttl_secs, 3600);
        assert_eq!(cfg.cookie_name, "test_cookie");
        assert!(cfg.cookie_secure);
    }

    #[test]
    fn missing_database_url_errors_and_names_the_variable() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let env = EnvGuard::cleared();
        env.set("OTP_PEPPER", "test-pepper");
        match super::Config::from_env() {
            Ok(_) => panic!("DATABASE_URL missing must be an error"),
            Err(e) => assert!(
                e.contains("DATABASE_URL"),
                "error must name the variable, got: {e}"
            ),
        }
    }

    #[test]
    fn missing_otp_pepper_errors_and_names_the_variable() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let env = EnvGuard::cleared();
        env.set("DATABASE_URL", "mysql://user:pass@127.0.0.1:3306/shabdar_test");
        match super::Config::from_env() {
            Ok(_) => panic!("OTP_PEPPER missing must be an error"),
            Err(e) => assert!(
                e.contains("OTP_PEPPER"),
                "error must name the variable, got: {e}"
            ),
        }
    }

    #[test]
    fn rejects_non_numeric_and_non_bool_values() {
        let (_lock, env) = locked_required_env();
        env.set("OTP_TTL_SECS", "two-minutes");
        match super::Config::from_env() {
            Ok(_) => panic!("non-numeric OTP_TTL_SECS must be an error"),
            Err(e) => assert!(
                e.contains("OTP_TTL_SECS"),
                "error must name the variable, got: {e}"
            ),
        }
        env.set("OTP_TTL_SECS", "120");
        env.set("OTP_DEV_MODE", "yes");
        match super::Config::from_env() {
            Ok(_) => panic!("non-bool OTP_DEV_MODE must be an error"),
            Err(e) => assert!(
                e.contains("OTP_DEV_MODE"),
                "error must name the variable, got: {e}"
            ),
        }
    }
}
