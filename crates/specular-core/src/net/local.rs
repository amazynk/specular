//! This computer itself as an address: the loopback addresses (`127.0.0.0/8`, `::1`) and the
//! name `localhost`.
//!
//! A request to one of them never leaves the computer, so the no-network switch
//! ([`super::off`]) lets a client that asked for it reach them (`specular_remote::Client`'s
//! `with_loopback`): a program on this computer that serves a stream to the player (a
//! downloader's file while it downloads). Everything else stays behind the switch.
//!
//! Only literal addresses and `localhost` count. Other spellings of an address (`127.1`,
//! `0x7f.0.0.1`, `2130706433`), other names that happen to resolve to loopback and the
//! unspecified address (`0.0.0.0`) do not: nothing is looked up to decide.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// The loopback addresses `host` names (a URL's host, an IPv6 address with or without its
/// brackets); `None` when it is not this computer.
pub fn loopback_addrs(host: &str) -> Option<Vec<IpAddr>> {
    let h = host.trim();
    if h.eq_ignore_ascii_case("localhost") || h.eq_ignore_ascii_case("localhost.") {
        return Some(vec![
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ]);
    }
    let bare = h
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .unwrap_or(h);
    // In a URL an IPv6 address is in brackets; without them only IPv4 is an address.
    let ip = if h.starts_with('[') {
        IpAddr::V6(bare.parse().ok()?)
    } else {
        IpAddr::V4(bare.parse().ok()?)
    };
    let loopback = match ip {
        IpAddr::V4(a) => a.is_loopback(),
        IpAddr::V6(a) => a.is_loopback() || a.to_ipv4_mapped().is_some_and(|m| m.is_loopback()),
    };
    loopback.then(|| vec![ip])
}

/// Whether `host` (a URL's host) is this computer.
pub fn is_loopback_host(host: &str) -> bool {
    loopback_addrs(host).is_some()
}

/// The host of an `http://` or `https://` address, without user name, password or port (an
/// IPv6 address keeps its brackets). `None` for anything else, and for an address with
/// characters a URL cannot hold (spaces, control characters, a backslash): what the HTTP
/// client would make of those is not what is written.
pub fn http_host(url: &str) -> Option<&str> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    if url
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next()?;
    let host = if host_port.starts_with('[') {
        let end = host_port.find(']')?;
        let tail = &host_port[end + 1..];
        if !tail.is_empty() && !is_port(tail) {
            return None;
        }
        &host_port[..=end]
    } else {
        match host_port.rsplit_once(':') {
            Some((h, _)) if is_port(&host_port[h.len()..]) => h,
            Some(_) => return None,
            None => host_port,
        }
    };
    (!host.is_empty()).then_some(host)
}

/// `:` and a port number.
fn is_port(s: &str) -> bool {
    s.strip_prefix(':')
        .is_some_and(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether `url` is an `http://` or `https://` address on this computer itself.
pub fn is_loopback_url(url: &str) -> bool {
    http_host(url.trim()).is_some_and(is_loopback_host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_computer_is_told_from_everything_else() {
        for yes in [
            "http://127.0.0.1:41234/a1b2.m3u8",
            "http://127.0.0.1/",
            "http://127.8.9.10:1/x",
            "https://127.0.0.1:8443/x.mp4?token=1#t",
            "http://[::1]:8080/x",
            "http://[::1]/x",
            "http://[::ffff:127.0.0.1]:80/x",
            "http://localhost:8080/live.m3u8",
            "HTTP://LOCALHOST/x",
            "http://localhost./x",
            "http://user:pw@127.0.0.1:9/x",
            " http://127.0.0.1:9/x ",
        ] {
            assert!(is_loopback_url(yes), "{yes}");
        }
        for no in [
            "http://example.com/x",
            "http://192.168.1.10:8080/x",
            "http://10.0.0.1/x",
            "http://0.0.0.0:8080/x",
            "http://[::]:8080/x",
            // The host is what follows the last `@`.
            "http://127.0.0.1@example.com/x",
            "http://127.0.0.1:80@example.com/x",
            "http://localhost@example.com/x",
            "http://example.com/127.0.0.1",
            "http://example.com?127.0.0.1",
            "http://example.com#@127.0.0.1/",
            // Names that only look local, and other spellings of the address.
            "http://127.0.0.1.example.com/x",
            "http://localhost.example.com/x",
            "http://notlocalhost/x",
            "http://127.1/x",
            "http://0x7f.0.0.1/x",
            "http://2130706433/x",
            "http://127.0.0.1:port/x",
            "http://127.0.0.1:/x",
            "http://127.0.0.1\\@example.com/x",
            "http://127.0.0.1 .example.com/x",
            "http://[::1/x",
            "http://[::1]x/y",
            // Other schemes, and no address at all.
            "ftp://127.0.0.1/x",
            "file:///127.0.0.1/x",
            "127.0.0.1:8080/x",
            "localhost",
            "http:///x",
            "",
        ] {
            assert!(!is_loopback_url(no), "{no}");
        }
        assert_eq!(http_host("http://[::1]:8080/x"), Some("[::1]"));
        assert_eq!(http_host("https://Example.com:443/x"), Some("Example.com"));
        assert_eq!(
            loopback_addrs("localhost"),
            Some(vec![
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                IpAddr::V6(Ipv6Addr::LOCALHOST)
            ])
        );
        assert_eq!(
            loopback_addrs("[::1]"),
            Some(vec![IpAddr::V6(Ipv6Addr::LOCALHOST)])
        );
        assert_eq!(
            loopback_addrs("::1"),
            None,
            "a bare IPv6 address is no host"
        );
        assert_eq!(loopback_addrs("example.com"), None);
    }
}
