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

/// Normalize an Iranian mobile number to E.164: `+989XXXXXXXXX`.
pub fn normalize_iranian_mobile(raw: &str) -> Result<String, PhoneError> {
    // 1. Reduce the input to ASCII digits, keeping a single leading '+'.
    //    Spaces and dashes are separators; any other character is invalid.
    let mut digits = String::with_capacity(raw.len());
    for (i, ch) in raw.chars().enumerate() {
        match ch {
            '0'..='9' => digits.push(ch),
            '۰'..='۹' => digits.push(to_ascii_digit(ch, '۰')),
            '٠'..='٩' => digits.push(to_ascii_digit(ch, '٠')),
            '+' if i == 0 => digits.push(ch),
            c if c.is_whitespace() || c == '-' => {} // separator: skip
            _ => return Err(PhoneError::Invalid),
        }
    }

    // 2. Drop the prefixes: +98, 0098, 98, or a leading 0 (longest first).
    let without_plus = digits.strip_prefix('+').unwrap_or(&digits);
    let local = without_plus
        .strip_prefix("0098")
        .or_else(|| without_plus.strip_prefix("98"))
        .or_else(|| without_plus.strip_prefix('0'))
        .unwrap_or(without_plus);

    // 3. What remains must be the 10-digit national number: 9 + 9 digits.
    //    (`digits` holds only ASCII digits here, so no charset check is needed.)
    if local.len() != 10 || !local.starts_with('9') {
        return Err(PhoneError::Invalid);
    }

    Ok(format!("+98{local}"))
}

/// Map a Persian/Arabic digit to its ASCII digit; `zero` is that script's 0.
fn to_ascii_digit(ch: char, zero: char) -> char {
    char::from_digit(ch as u32 - zero as u32, 10).expect("ch is within the script's 0-9 range")
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
