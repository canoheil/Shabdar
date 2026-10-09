//! Iranian mobile numbers: normalize user input to E.164 (`+989XXXXXXXXX`).
//!
//! Accepted shapes (docs/PHASE1_PLAN.md, Task 6): ASCII or Persian/Arabic
//! digits, optional spaces and dashes, and a `+98`, `0098`, `98`, or leading
//! `0` prefix. Landlines, wrong lengths, letters, and empty input are rejected
//! with [`PhoneError::Invalid`].
//!
//! Accepting Persian/Arabic digits at the API boundary is a deliberate
//! robustness addition; the display/input digit policy stays a frontend
//! concern (ARCHITECTURE.md §8) — Phase 1 stores ASCII E.164 only.

/// Why a phone number was rejected. The API maps every rejection to the same
/// 400 `invalid_phone` response (API.md §2), so a single variant is enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhoneError {
    /// The input is not a valid Iranian mobile number.
    Invalid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_local_format_with_separators() {
        assert_eq!(
            normalize_iranian_mobile("0912 345-6789"),
            Ok("+989123456789".to_string())
        );
    }

    #[test]
    fn accepts_e164_format() {
        assert_eq!(
            normalize_iranian_mobile("+989123456789"),
            Ok("+989123456789".to_string())
        );
    }

    #[test]
    fn accepts_double_zero_and_bare_country_code() {
        assert_eq!(
            normalize_iranian_mobile("00989123456789"),
            Ok("+989123456789".to_string())
        );
        assert_eq!(
            normalize_iranian_mobile("989123456789"),
            Ok("+989123456789".to_string())
        );
    }

    #[test]
    fn accepts_persian_digits() {
        assert_eq!(
            normalize_iranian_mobile("۰۹۱۲۳۴۵۶۷۸۹"),
            Ok("+989123456789".to_string())
        );
    }

    #[test]
    fn accepts_arabic_digits() {
        assert_eq!(
            normalize_iranian_mobile("٠٩١٢٣٤٥٦٧٨٩"),
            Ok("+989123456789".to_string())
        );
    }

    #[test]
    fn rejects_landline_number() {
        assert_eq!(
            normalize_iranian_mobile("02112345678"),
            Err(PhoneError::Invalid)
        );
    }

    #[test]
    fn rejects_too_short_number() {
        assert_eq!(normalize_iranian_mobile("0912345"), Err(PhoneError::Invalid));
    }

    #[test]
    fn rejects_too_long_number() {
        assert_eq!(
            normalize_iranian_mobile("091234567890"),
            Err(PhoneError::Invalid)
        );
    }

    #[test]
    fn rejects_letters() {
        assert_eq!(
            normalize_iranian_mobile("0912abc6789"),
            Err(PhoneError::Invalid)
        );
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(normalize_iranian_mobile(""), Err(PhoneError::Invalid));
    }
}
