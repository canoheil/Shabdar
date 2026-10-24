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

use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

/// The key used when neither a trusted header nor a peer address is available
/// (oneshot tests build requests without connection info).
pub const UNKNOWN_IP: &str = "unknown";

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
    raw.rsplit(',').find_map(|part| part.trim().parse::<IpAddr>().ok())
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
        assert_eq!(ip, "198.51.100.7", "peer address must win when not trusting the proxy");
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
    fn unknown_when_nothing_is_available() {
        assert_eq!(client_ip(true, None, &HeaderMap::new()), UNKNOWN_IP);
        assert_eq!(client_ip(false, None, &HeaderMap::new()), UNKNOWN_IP);
    }
}
