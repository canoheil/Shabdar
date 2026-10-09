//! OTP codes: 6-digit crypto-random generation and HMAC-SHA256 hashing
//! (docs/PHASE1_PLAN.md, Task 7).
//!
//! The pepper is the secret HMAC key shared by hashing and verification; it
//! must never change once codes are in use (docs/STATUS.md). Codes are 6 ASCII
//! digits; only their HMAC is stored, never the plaintext code.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generate_code_returns_6_ascii_digits() {
        let mut seen = HashSet::new();
        for _ in 0..1000 {
            let code = generate_code();
            assert_eq!(code.len(), 6, "code must be 6 digits: {code}");
            assert!(
                code.bytes().all(|b| b.is_ascii_digit()),
                "code must be ASCII digits: {code}"
            );
            seen.insert(code);
        }
        assert!(seen.len() > 1, "codes must vary (crypto-random)");
    }

    #[test]
    fn hash_code_is_32_bytes_and_deterministic() {
        let a = hash_code("123456", "pepper");
        let b = hash_code("123456", "pepper");
        assert_eq!(a.len(), 32);
        assert_eq!(a, b);
    }

    #[test]
    fn hash_code_differs_when_pepper_differs() {
        assert_ne!(
            hash_code("123456", "pepper-a"),
            hash_code("123456", "pepper-b")
        );
    }

    #[test]
    fn hash_code_differs_when_code_differs() {
        assert_ne!(hash_code("123456", "pepper"), hash_code("123457", "pepper"));
    }

    #[test]
    fn hash_code_matches_rfc4231_test_case_1() {
        // RFC 4231 §4.1: key = 0x0b × 20, message = "Hi There". Pins both the
        // crypto and the argument order: HMAC key = pepper, message = code.
        let pepper = "\u{b}".repeat(20);
        let expected: [u8; 32] = [
            0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b,
            0xf1, 0x2b, 0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c,
            0x2e, 0x32, 0xcf, 0xf7,
        ];
        assert_eq!(hash_code("Hi There", &pepper), expected);
    }

    #[test]
    fn verify_code_accepts_the_correct_code() {
        let hash = hash_code("123456", "pepper");
        assert!(verify_code("123456", "pepper", &hash));
    }

    #[test]
    fn verify_code_rejects_wrong_code_or_pepper() {
        let hash = hash_code("123456", "pepper");
        assert!(!verify_code("123457", "pepper", &hash));
        assert!(!verify_code("123456", "other-pepper", &hash));
    }
}
