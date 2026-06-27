//! Unit tests for `routes::resolve_client_ip` — the client-IP precedence that
//! keys the signup rate limiter.
//!
//! The incident this guards against: when the real client IP can't be derived,
//! every visitor must NOT collapse into one shared bucket (a `"unknown"`
//! literal did exactly that, so 3 signups/hr blocked *everyone*). The socket
//! peer is the fallback, so each connection is keyed distinctly.

use slatehub::routes::resolve_client_ip;
use std::net::{IpAddr, Ipv4Addr};

fn peer() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))
}

#[test]
fn prefers_left_most_forwarded_for() {
    // X-Forwarded-For is "client, proxy1, proxy2" — the original client is first.
    assert_eq!(
        resolve_client_ip(Some("198.51.100.23, 70.0.0.1, 10.0.0.1"), None, peer()),
        "198.51.100.23"
    );
}

#[test]
fn trims_forwarded_for_whitespace() {
    assert_eq!(
        resolve_client_ip(Some("  198.51.100.23 "), None, peer()),
        "198.51.100.23"
    );
}

#[test]
fn falls_back_to_real_ip_when_no_forwarded_for() {
    assert_eq!(
        resolve_client_ip(None, Some("198.51.100.99"), peer()),
        "198.51.100.99"
    );
}

#[test]
fn empty_forwarded_for_does_not_win() {
    // A present-but-empty header must not shadow X-Real-IP / the peer.
    assert_eq!(
        resolve_client_ip(Some(""), Some("198.51.100.99"), peer()),
        "198.51.100.99"
    );
    assert_eq!(
        resolve_client_ip(Some("   "), None, peer()),
        peer().to_string()
    );
}

#[test]
fn falls_back_to_socket_peer_when_no_headers() {
    // The crux: no headers must NOT yield a shared literal — it must key by the
    // real connection address so clients stay in distinct rate-limit buckets.
    assert_eq!(resolve_client_ip(None, None, peer()), "203.0.113.7");
}

#[test]
fn peer_fallback_is_never_the_unknown_literal() {
    let resolved = resolve_client_ip(None, None, peer());
    assert_ne!(resolved, "unknown");
    assert!(!resolved.is_empty());
}
