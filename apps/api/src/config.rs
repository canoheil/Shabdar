//! Environment-based configuration (docs/PHASE1_PLAN.md "Config contract").
//!
//! Required: DATABASE_URL, OTP_PEPPER. Everything else has a default.
//! Secrets (database URL, pepper) live only here and must never be logged.

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
