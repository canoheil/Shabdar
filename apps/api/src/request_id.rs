//! Per-request IDs (API.md §1: a request ID on every request).
//!
//! The middleware mints a ULID per request, stores it in request extensions
//! for handlers/services, and echoes it as the `x-request-id` response header.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

const HEADER_NAME: HeaderName = HeaderName::from_static("x-request-id");

/// axum middleware; attach with `middleware::from_fn` (see main.rs).
pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    let id = ulid::Ulid::new().to_string();
    req.extensions_mut().insert(id.clone());
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        HEADER_NAME,
        HeaderValue::from_str(&id).expect("a ULID is always a valid header value"),
    );
    resp
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::extract::Request;
    use axum::{middleware, routing::get, Router};
    use tower::ServiceExt; // Router::oneshot: drive the stack without a socket

    /// 26 chars, Crockford base32 — the plan's ^[0-9A-HJKMNP-TV-Z]{26}$.
    fn is_crockford_ulid(s: &str) -> bool {
        s.len() == 26
            && s.bytes().all(|b| {
                matches!(b,
                    b'0'..=b'9' | b'A'..=b'H' | b'J'..=b'K' | b'M'..=b'N' | b'P'..=b'T' | b'V'..=b'Z')
            })
    }

    /// Echoes the id the middleware put into request extensions, as the body.
    async fn echo_id(axum::Extension(id): axum::Extension<String>) -> String {
        id
    }

    fn test_app() -> Router {
        Router::new()
            .route("/echo", get(echo_id))
            .layer(middleware::from_fn(super::request_id_middleware))
    }

    /// Runs one request through the middleware stack; returns (header id,
    /// extension id echoed by the handler).
    async fn header_and_extension_ids() -> (String, String) {
        let req = Request::builder().uri("/echo").body(Body::empty()).unwrap();
        let resp = test_app().oneshot(req).await.unwrap();
        let header = resp
            .headers()
            .get("x-request-id")
            .expect("x-request-id header present")
            .to_str()
            .expect("ascii header value")
            .to_string();
        let bytes = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .expect("readable body")
            .to_bytes();
        (header, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn header_and_extension_carry_the_same_wellformed_ulid() {
        let (header, extension_echo) = header_and_extension_ids().await;
        assert_eq!(
            header, extension_echo,
            "handler must see the same id the response header carries"
        );
        assert!(is_crockford_ulid(&header), "not a ULID: {header}");
    }

    #[tokio::test]
    async fn every_request_gets_a_fresh_id() {
        let (first, _) = header_and_extension_ids().await;
        let (second, _) = header_and_extension_ids().await;
        assert_ne!(first, second, "request ids must be unique");
    }
}
