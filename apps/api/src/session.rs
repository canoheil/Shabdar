//! Sessions: opaque cookie tokens, stored only as SHA-256 hashes
//! (docs/PHASE1_PLAN.md, Task 9; migration 0002). The raw token is returned
//! to the client once and never persisted.

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
}
