//! The one error envelope (API.md §1, docs/PHASE1_PLAN.md "Error contract").
//!
//! Every error response has the same shape:
//! `{"error":{"code":"…","message":"…","fields":{…}?},"request_id":"…"}`
//! `fields` appears only for field-level validation errors.

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

    /// The plan's contract table, pinned exactly: HTTP status + code for all 9
    /// variants, plus a non-empty message on every one.
    #[tokio::test]
    async fn status_and_code_match_contract_for_every_variant() {
        let cases: [(super::ApiError, StatusCode, &str); 9] = [
            (
                super::ApiError::Validation { fields: fields(&[]) },
                StatusCode::BAD_REQUEST,
                "validation_error",
            ),
            (super::ApiError::InvalidPhone, StatusCode::BAD_REQUEST, "invalid_phone"),
            (
                super::ApiError::InvalidPurpose,
                StatusCode::BAD_REQUEST,
                "invalid_purpose",
            ),
            (
                super::ApiError::RateLimited,
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
            ),
            (super::ApiError::InvalidCode, StatusCode::UNAUTHORIZED, "invalid_code"),
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
            (super::ApiError::NoAccount, StatusCode::UNAUTHORIZED, "no_account"),
            (
                super::ApiError::Internal,
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
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
