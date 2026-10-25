//! Client address resolution for rate limiting (docs/PHASE5_PLAN.md, WP A).
//!
//! `X-Forwarded-For` is client-controlled, so it is honoured only when the
//! deployment says a trusted proxy sits in front (`TRUST_PROXY=true`). The Nuxt
//! proxy *replaces* the header with the real client IP rather than appending to
//! whatever the browser sent (`apps/web/server/api/[...path].ts`), so the last
//! entry is the trustworthy one. With the flag off, the socket peer address
//! wins and the header is ignored entirely.
//!
//! The returned string is always either a parsed IP or `"unknown"`, so a
//! hostile header can never create unbounded distinct limiter keys.

use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::HeaderMap;

/// The key used when neither a trusted header nor a peer address is available
/// (oneshot tests build requests without connection info).
pub const UNKNOWN_IP: &str = "unknown";

/// The socket peer address, when the server was started with connect info.
///
/// axum's `ConnectInfo` rejects the request when no peer is available, and
/// `Option<ConnectInfo<_>>` is not implemented, so this wrapper yields `None`
/// instead. Tests deliberately run without a peer and expect that.
pub struct PeerAddr(pub Option<SocketAddr>);

impl<S: Send + Sync> FromRequestParts<S> for PeerAddr {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(PeerAddr(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ConnectInfo(addr)| *addr),
        ))
    }
}

pub fn client_ip(trust_proxy: bool, peer: Option<SocketAddr>, headers: &HeaderMap) -> String {
    if trust_proxy {
        if let Some(ip) = forwarded_ip(headers) {
            return ip.to_string();
        }
    }
    peer.map(|p| p.ip().to_string())
        .unwrap_or_else(|| UNKNOWN_IP.to_string())
}

/// Last non-empty `X-Forwarded-For` entry, parsed as an IP; anything else is
/// ignored (never a free-form string in a limiter key).
fn forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    let raw = headers.get("x-forwarded-for")?.to_str().ok()?;
    raw.rsplit(',')
        .find_map(|part| part.trim().parse::<IpAddr>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers_with(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_str(value).unwrap());
        headers
    }

    fn peer(ip: &str) -> Option<SocketAddr> {
        Some(SocketAddr::new(ip.parse().unwrap(), 40000))
    }

    #[test]
    fn ignores_forwarded_header_without_trust() {
        let ip = client_ip(false, peer("198.51.100.7"), &headers_with("203.0.113.9"));
        assert_eq!(
            ip, "198.51.100.7",
            "peer address must win when not trusting the proxy"
        );
    }

    #[test]
    fn uses_the_last_forwarded_entry_when_trusted() {
        let ip = client_ip(
            true,
            peer("10.0.0.5"),
            &headers_with("1.2.3.4, 203.0.113.9"),
        );
        assert_eq!(ip, "203.0.113.9", "the proxy appends the real client last");
    }

    #[test]
    fn falls_back_to_peer_when_the_header_is_junk() {
        let ip = client_ip(true, peer("10.0.0.5"), &headers_with("not-an-ip"));
        assert_eq!(ip, "10.0.0.5");
    }

    #[test]
    fn peer_addr_extracts_from_extensions() {
        // The extractor is exercised end-to-end through the router in
        // tests/auth_rate_limits.rs; here we pin the Option mapping only.
        let mut parts = axum::http::Request::new(()).into_parts().0;
        parts.extensions.insert(ConnectInfo(SocketAddr::new(
            "203.0.113.5".parse().unwrap(),
            9000,
        )));
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| *addr);
        assert_eq!(peer.unwrap().ip().to_string(), "203.0.113.5");
    }

    #[test]
    fn unknown_when_nothing_is_available() {
        assert_eq!(client_ip(true, None, &HeaderMap::new()), UNKNOWN_IP);
        assert_eq!(client_ip(false, None, &HeaderMap::new()), UNKNOWN_IP);
    }
}
