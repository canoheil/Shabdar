//! `ApiJson`: the request-body extractor every handler uses (docs/PHASE5_PLAN.md
//! WP C). axum's own `Json` rejection answers with a plain-text body, which
//! breaks the one-envelope rule (API.md §1) for exactly the clients that are
//! already confused. This wrapper maps each rejection onto `ApiError`, so a
//! malformed body looks like every other error.
//!
//! Responses keep using `axum::Json`; only extraction is wrapped.

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use axum::Json;
use serde::de::DeserializeOwned;

use crate::error::ApiError;

pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(ApiJson(value)),
            Err(rejection) => Err(map_rejection(rejection)),
        }
    }
}

/// One row per way the body can be unacceptable (the table in API.md §1).
fn map_rejection(rejection: JsonRejection) -> ApiError {
    match rejection {
        JsonRejection::MissingJsonContentType(_) => ApiError::InvalidRequest {
            body: "unsupported_content_type",
        },
        JsonRejection::JsonSyntaxError(_) => ApiError::InvalidRequest {
            body: "malformed_json",
        },
        JsonRejection::JsonDataError(_) => ApiError::InvalidRequest {
            body: "invalid_json",
        },
        // A length-limit failure is really "too large"; any other body-read
        // failure is still a 400 with its own reason, never plain text.
        JsonRejection::BytesRejection(inner) => {
            if inner.status() == StatusCode::PAYLOAD_TOO_LARGE {
                ApiError::PayloadTooLarge
            } else {
                ApiError::InvalidRequest {
                    body: "unreadable_body",
                }
            }
        }
        // `JsonRejection` is non_exhaustive in axum: stay on the envelope.
        _ => ApiError::InvalidRequest {
            body: "invalid_json",
        },
    }
}
