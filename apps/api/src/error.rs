//! The one error envelope (API.md §1, docs/PHASE1_PLAN.md "Error contract").
//!
//! Every error response has the same shape:
//! `{"error":{"code":"…","message":"…","fields":{…}?},"request_id":"…"}`
//! `fields` appears only for field-level validation errors.

use std::collections::BTreeMap;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

/// All API errors. The `code` strings are the exact, frozen contract with
/// clients (docs/PHASE1_PLAN.md "Error contract" — do not rename).
/// Debug is derived so tests can `.expect()` on `Result<_, ApiError>`.
#[derive(Debug)]
pub enum ApiError {
    /// 400 with per-field detail, e.g. `{"phone": "required"}`.
    Validation {
        fields: BTreeMap<String, String>,
    },
    InvalidPhone,
    InvalidPurpose,
    /// HTTP 429. `retry_after_secs` becomes the `Retry-After` header when
    /// known; both the resend cooldown and every Phase 5 abuse limit know it.
    RateLimited {
        retry_after_secs: Option<u64>,
    },
    InvalidCode,
    ChallengeExpired,
    ChallengeLocked,
    NoAccount,
    /// 404 for unknown paths (Phase 2 fallback, `not_found`).
    NotFound,
    /// 401 for a missing/invalid/expired session or non-active user
    /// (Phase 2 extractor, `unauthorized` — deliberately generic).
    Unauthorized,
    /// 403 when `X-CSRF-Token` is missing or ≠ the `shabdar_csrf` cookie
    /// (Phase 2 CSRF, `csrf_failed`).
    CsrfFailed,
    /// 403 when the caller is not the owner (seller routes) or not an admin
    /// (admin routes). Message names no target (Phase 3, `forbidden`).
    Forbidden,
    /// 410 for publicly removed or deleted listings (Phase 3, `gone`).
    Gone,
    /// 400 for a request body axum could not accept (Phase 5, `invalid_request`).
    /// `body` becomes `fields.body`: malformed_json, invalid_json,
    /// unsupported_content_type or unreadable_body.
    InvalidRequest {
        body: &'static str,
    },
    /// 413 for a body past the extractor's length limit (Phase 5,
    /// `payload_too_large`).
    PayloadTooLarge,
    Internal,
}

impl ApiError {
    fn status(&self) -> StatusCode {
        match self {
            ApiError::Validation { .. }
            | ApiError::InvalidPhone
            | ApiError::InvalidPurpose
            | ApiError::InvalidRequest { .. } => StatusCode::BAD_REQUEST,
            ApiError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ApiError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            ApiError::InvalidCode
            | ApiError::ChallengeExpired
            | ApiError::ChallengeLocked
            | ApiError::NoAccount
            | ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::CsrfFailed | ApiError::Forbidden => StatusCode::FORBIDDEN,
            ApiError::Gone => StatusCode::GONE,
            ApiError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            ApiError::Validation { .. } => "validation_error",
            ApiError::InvalidPhone => "invalid_phone",
            ApiError::InvalidPurpose => "invalid_purpose",
            ApiError::RateLimited { .. } => "rate_limited",
            ApiError::InvalidCode => "invalid_code",
            ApiError::ChallengeExpired => "challenge_expired",
            ApiError::ChallengeLocked => "challenge_locked",
            ApiError::NoAccount => "no_account",
            ApiError::NotFound => "not_found",
            ApiError::Unauthorized => "unauthorized",
            ApiError::CsrfFailed => "csrf_failed",
            ApiError::Forbidden => "forbidden",
            ApiError::Gone => "gone",
            ApiError::InvalidRequest { .. } => "invalid_request",
            ApiError::PayloadTooLarge => "payload_too_large",
            ApiError::Internal => "internal_error",
        }
    }

    /// Human-readable, free of secrets and SQL detail. Clients key off
    /// `code`, never off this text.
    fn message(&self) -> &'static str {
        match self {
            ApiError::Validation { .. } => "One or more fields are invalid",
            ApiError::InvalidPhone => "Phone number is not a valid Iranian mobile number",
            ApiError::InvalidPurpose => "Purpose must be \"login\" or \"register\"",
            ApiError::RateLimited { .. } => "Too many requests; please wait and try again",
            ApiError::InvalidCode => "Invalid code",
            ApiError::ChallengeExpired => "Code expired; request a new one",
            ApiError::ChallengeLocked => "Too many wrong attempts; request a new code",
            ApiError::NoAccount => "No account exists for this phone number",
            ApiError::NotFound => "Not found",
            ApiError::Unauthorized => "Not authenticated",
            ApiError::CsrfFailed => "CSRF token missing or invalid",
            ApiError::Forbidden => "Not allowed",
            ApiError::Gone => "Listing is no longer available",
            ApiError::InvalidRequest { .. } => "Request body is not valid JSON",
            ApiError::PayloadTooLarge => "Request body is too large",
            ApiError::Internal => "Internal server error",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut error = serde_json::json!({
            "code": self.code(),
            "message": self.message(),
        });
        match &self {
            ApiError::Validation { fields } => error["fields"] = serde_json::json!(fields),
            ApiError::InvalidRequest { body } => {
                error["fields"] = serde_json::json!({ "body": body })
            }
            _ => {}
        }
        // ponytail: a fresh ULID keeps the envelope self-contained; the
        // request-id middleware owns the x-request-id header + extensions.
        // If a later phase needs header/body correlation, thread the
        // extension id into the handlers that build these errors.
        let body = serde_json::json!({
            "error": error,
            "request_id": ulid::Ulid::new().to_string(),
        });
        let mut response = (self.status(), Json(body)).into_response();
        // 429 tells the client when to come back (docs/PHASE5_PLAN.md).
        if let ApiError::RateLimited {
            retry_after_secs: Some(secs),
        } = self
        {
            if let Ok(value) = axum::http::HeaderValue::from_str(&secs.to_string()) {
                response
                    .headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, value);
            }
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    /// Renders an error the way a client sees it: (status, parsed JSON body).
    async fn render(err: super::ApiError) -> (StatusCode, serde_json::Value) {
        let resp = err.into_response();
        let status = resp.status();
        let bytes = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .expect("readable body")
            .to_bytes();
        (status, serde_json::from_slice(&bytes).expect("valid JSON"))
    }

    fn fields(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// The plan's contract table, pinned exactly: HTTP status + code for every
    /// variant, plus a non-empty message on every one.
    #[tokio::test]
    async fn status_and_code_match_contract_for_every_variant() {
        let cases: [(super::ApiError, StatusCode, &str); 16] = [
            (
                super::ApiError::Validation {
                    fields: fields(&[]),
                },
                StatusCode::BAD_REQUEST,
                "validation_error",
            ),
            (
                super::ApiError::InvalidPhone,
                StatusCode::BAD_REQUEST,
                "invalid_phone",
            ),
            (
                super::ApiError::InvalidPurpose,
                StatusCode::BAD_REQUEST,
                "invalid_purpose",
            ),
            (
                super::ApiError::RateLimited {
                    retry_after_secs: None,
                },
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
            ),
            (
                super::ApiError::InvalidCode,
                StatusCode::UNAUTHORIZED,
                "invalid_code",
            ),
            (
                super::ApiError::ChallengeExpired,
                StatusCode::UNAUTHORIZED,
                "challenge_expired",
            ),
            (
                super::ApiError::ChallengeLocked,
                StatusCode::UNAUTHORIZED,
                "challenge_locked",
            ),
            (
                super::ApiError::NoAccount,
                StatusCode::UNAUTHORIZED,
                "no_account",
            ),
            (
                super::ApiError::Internal,
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
            ),
            // Phase 2 additions (docs/PHASE2_PLAN.md "Error contract additions").
            (
                super::ApiError::NotFound,
                StatusCode::NOT_FOUND,
                "not_found",
            ),
            (
                super::ApiError::Unauthorized,
                StatusCode::UNAUTHORIZED,
                "unauthorized",
            ),
            (
                super::ApiError::CsrfFailed,
                StatusCode::FORBIDDEN,
                "csrf_failed",
            ),
            (
                super::ApiError::Forbidden,
                StatusCode::FORBIDDEN,
                "forbidden",
            ),
            (super::ApiError::Gone, StatusCode::GONE, "gone"),
            // Phase 5 additions (docs/PHASE5_PLAN.md "Contract additions").
            (
                super::ApiError::InvalidRequest {
                    body: "malformed_json",
                },
                StatusCode::BAD_REQUEST,
                "invalid_request",
            ),
            (
                super::ApiError::PayloadTooLarge,
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
            ),
        ];
        for (err, want_status, want_code) in cases {
            let (status, body) = render(err).await;
            assert_eq!(status, want_status);
            assert_eq!(body["error"]["code"], want_code, "code for {want_code}");
            assert!(
                body["error"]["message"]
                    .as_str()
                    .is_some_and(|m| !m.is_empty()),
                "message must be a non-empty string"
            );
        }
    }

    #[tokio::test]
    async fn rate_limited_sets_retry_after_header() {
        let resp = super::ApiError::RateLimited {
            retry_after_secs: Some(42),
        }
        .into_response();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            resp.headers()
                .get(axum::http::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok()),
            Some("42")
        );
        let bytes = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .expect("readable body")
            .to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("valid JSON");
        assert_eq!(body["error"]["code"], "rate_limited");
    }

    #[tokio::test]
    async fn rate_limited_without_seconds_has_no_header() {
        let resp = super::ApiError::RateLimited {
            retry_after_secs: None,
        }
        .into_response();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(
            resp.headers()
                .get(axum::http::header::RETRY_AFTER)
                .is_none(),
            "no retry-after when the remaining time is unknown"
        );
    }

    #[tokio::test]
    async fn validation_error_includes_the_fields_object_verbatim() {
        let err = super::ApiError::Validation {
            fields: fields(&[("phone", "required"), ("purpose", "unknown value")]),
        };
        let (_status, body) = render(err).await;
        assert_eq!(body["error"]["fields"]["phone"], "required");
        assert_eq!(body["error"]["fields"]["purpose"], "unknown value");
    }

    #[tokio::test]
    async fn fields_key_is_absent_on_non_validation_errors() {
        for err in [super::ApiError::InvalidCode, super::ApiError::Internal] {
            let (_status, body) = render(err).await;
            assert!(
                body["error"].get("fields").is_none(),
                "`fields` leaked into a non-validation error: {body}"
            );
        }
    }

    #[tokio::test]
    async fn every_error_body_carries_a_ulid_request_id() {
        let (_status, body) = render(super::ApiError::Internal).await;
        let id = body["request_id"].as_str().expect("request_id is a string");
        assert_eq!(id.len(), 26, "request_id must be a 26-char ULID, got {id}");
        assert!(
            id.bytes().all(|b| matches!(b,
                b'0'..=b'9' | b'A'..=b'H' | b'J'..=b'K' | b'M'..=b'N' | b'P'..=b'T' | b'V'..=b'Z')),
            "request_id must be Crockford base32 (^[0-9A-HJKMNP-TV-Z]{{26}}$), got {id}"
        );
    }
}
