//! SSDP search (UPnP Device Architecture §1.3): an `M-SEARCH` for media renderers to the
//! multicast group, and the unicast answers that point at each device's description.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

use specular_core::CancelToken;

use crate::Result;

/// The SSDP multicast group and port.
pub const MULTICAST: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), 1900);

/// The search target: media renderers (TVs, speakers, receivers).
pub const RENDERER: &str = "urn:schemas-upnp-org:device:MediaRenderer:1";

/// How long devices may wait before answering (seconds; they spread their answers).
pub const MX: u32 = 2;

/// An answer to the search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    /// The description's URL.
    pub location: String,
    /// The device's unique service name (its UDN and type).
    pub usn: String,
    /// Where the answer came from.
    pub from: SocketAddr,
}

/// The `M-SEARCH` request for `st`.
pub fn search_message(st: &str, mx: u32) -> String {
    format!(
        "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: {mx}\r\nST: {st}\r\nUSER-AGENT: Specular/{} UPnP/1.1\r\n\r\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// Reads an answer: `HTTP/1.1 200`, a renderer (or every device) as its search target, and an
/// `http://` location on the local network.
pub fn parse_answer(data: &[u8], from: SocketAddr) -> Option<Answer> {
    let text = std::str::from_utf8(data).ok()?;
    let mut lines = text.split("\r\n").flat_map(|l| l.split('\n'));
    let first = lines.next()?;
    let mut parts = first.split_whitespace();
    if !parts.next()?.starts_with("HTTP/1.") || parts.next()? != "200" {
        return None;
    }
    let (mut location, mut usn, mut st) = (None, None, None);
    for l in lines {
        let Some((k, v)) = l.split_once(':') else {
            continue;
        };
        let v = v.trim();
        match k.trim().to_ascii_lowercase().as_str() {
            "location" => location = Some(v),
            "usn" => usn = Some(v),
            "st" => st = Some(v),
            _ => {}
        }
    }
    let st = st?;
    if !st.starts_with("urn:schemas-upnp-org:device:MediaRenderer:") && st != "ssdp:all" {
        return None;
    }
    let location = location?;
    let u = crate::http::Url::parse(location)?;
    if !crate::is_local(u.ip) {
        return None;
    }
    Some(Answer {
        location: location.to_string(),
        usn: usn.unwrap_or(location).to_string(),
        from,
    })
}

/// The IPv4 addresses of every interface that is up, not loopback and takes multicast,
/// once each (Unix: `getifaddrs`; Windows: the adapters' addresses, link-local ones left
/// out). Empty when none are listed: the search then leaves by the system's default
/// interface.
pub fn interfaces() -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        // An adapter that is down has no address here.
        let nets = sysinfo::Networks::new_with_refreshed_list();
        let mut ips: Vec<Ipv4Addr> = nets
            .list()
            .values()
            .flat_map(|n| n.ip_networks())
            .filter_map(|n| match n.addr {
                std::net::IpAddr::V4(ip) => Some(ip),
                std::net::IpAddr::V6(_) => None,
            })
            .collect();
        ips.sort_unstable();
        for ip in ips {
            if usable(ip) && !ip.is_link_local() && !out.contains(&ip) {
                out.push(ip);
            }
        }
    }
    #[cfg(unix)]
    if let Ok(list) = nix::ifaddrs::getifaddrs() {
        use nix::net::if_::InterfaceFlags as F;
        for i in list {
            if !i.flags.contains(F::IFF_UP)
                || i.flags.contains(F::IFF_LOOPBACK)
                || !i.flags.contains(F::IFF_MULTICAST)
            {
                continue;
            }
            let Some(ip) = i
                .address
                .as_ref()
                .and_then(|a| a.as_sockaddr_in())
                .map(|a| a.ip())
            else {
                continue;
            };
            if usable(ip) && !out.contains(&ip) {
                out.push(ip);
            }
        }
    }
    out
}

/// An address a search can leave from: not loopback, unspecified or multicast.
fn usable(ip: Ipv4Addr) -> bool {
    !ip.is_loopback() && !ip.is_unspecified() && !ip.is_multicast()
}

/// A socket the search leaves `from` by (the interface with that address).
fn socket_from(from: Ipv4Addr, multicast: bool) -> std::io::Result<UdpSocket> {
    let sock = UdpSocket::bind((from, 0))?;
    if multicast {
        // Routers between subnets pass a few hops at most (UDA: TTL 2 by default; 4 is common).
        let _ = sock.set_multicast_ttl_v4(4);
        if !from.is_unspecified() {
            // Linux already sends from the bound address's interface; the BSDs, macOS and
            // Windows need it said.
            let _ = rustix::net::sockopt::set_ip_multicast_if(&sock, &from);
        }
    }
    sock.set_nonblocking(true)?;
    Ok(sock)
}

/// Searches for renderers: sends the search to `target` (the multicast group, or a test's
/// responder) from every interface ([`interfaces`]; the default one when none are listed or
/// the target is not multicast), twice, then hands each new answer to `each` until `wait`
/// has passed or `cancel` is set. Refused while the network is off.
pub fn search(
    target: SocketAddr,
    wait: Duration,
    cancel: &CancelToken,
    each: impl FnMut(Answer),
) -> Result<()> {
    let from = if target.ip().is_multicast() {
        interfaces()
    } else {
        Vec::new()
    };
    search_from(&from, target, wait, cancel, each)
}

/// [`search`] from the interfaces with addresses `from` (none: the default one).
pub fn search_from(
    from: &[Ipv4Addr],
    target: SocketAddr,
    wait: Duration,
    cancel: &CancelToken,
    mut each: impl FnMut(Answer),
) -> Result<()> {
    specular_core::net::off::check()?;
    let multicast = target.ip().is_multicast();
    let mut socks = Vec::new();
    if target.is_ipv4() {
        for &ip in from {
            match socket_from(ip, multicast) {
                Ok(s) => socks.push(s),
                Err(e) => specular_log::debug!(
                    target: specular_log::IO,
                    "cast: no search from {ip}: {e}"
                ),
            }
        }
    }
    if socks.is_empty() {
        let any: std::net::IpAddr = if target.is_ipv4() {
            Ipv4Addr::UNSPECIFIED.into()
        } else {
            std::net::Ipv6Addr::UNSPECIFIED.into()
        };
        let sock = UdpSocket::bind((any, 0))?;
        if multicast {
            let _ = sock.set_multicast_ttl_v4(4);
        }
        sock.set_nonblocking(true)?;
        socks.push(sock);
    }
    let msg = search_message(RENDERER, MX);
    let send_all = |socks: &[UdpSocket]| -> usize {
        socks
            .iter()
            .filter(|s| s.send_to(msg.as_bytes(), target).is_ok())
            .count()
    };
    if send_all(&socks) == 0 {
        // Not one interface could send: say why, from the first.
        socks[0].send_to(msg.as_bytes(), target)?;
    }
    let start = Instant::now();
    let mut resent = false;
    let mut seen: Vec<String> = Vec::new();
    let mut buf = [0u8; 2048];
    while !cancel.is_cancelled() {
        let elapsed = start.elapsed();
        if elapsed >= wait {
            break;
        }
        // A second search a moment later: UDP drops packets.
        if !resent && elapsed >= Duration::from_millis(300) {
            resent = true;
            send_all(&socks);
        }
        let mut got = false;
        for sock in &socks {
            match sock.recv_from(&mut buf) {
                Ok((n, from)) => {
                    got = true;
                    if let Some(a) = parse_answer(&buf[..n], from)
                        && !seen.contains(&a.location)
                    {
                        seen.push(a.location.clone());
                        each(a);
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                // Windows reports an ICMP port-unreachable on the next receive.
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
                Err(e) => return Err(e.into()),
            }
        }
        if !got {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from() -> SocketAddr {
        "192.168.1.9:1900".parse().unwrap()
    }

    #[test]
    fn message_has_the_required_headers() {
        let m = search_message(RENDERER, 2);
        assert!(m.starts_with("M-SEARCH * HTTP/1.1\r\n"));
        assert!(m.contains("MAN: \"ssdp:discover\"\r\n"));
        assert!(m.contains("ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n"));
        assert!(m.ends_with("\r\n\r\n"));
    }

    #[test]
    fn interfaces_are_up_and_not_loopback() {
        let l = interfaces();
        assert!(l.iter().all(|ip| !ip.is_loopback() && !ip.is_unspecified()));
        let mut d = l.clone();
        d.dedup();
        assert_eq!(d, l, "each once");
        assert!(usable(Ipv4Addr::new(192, 168, 1, 4)));
        for ip in [
            Ipv4Addr::LOCALHOST,
            Ipv4Addr::UNSPECIFIED,
            Ipv4Addr::new(239, 255, 255, 250),
        ] {
            assert!(!usable(ip), "{ip}");
        }
    }

    #[test]
    fn reads_answers() {
        let a = b"HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=1800\r\nLocation: http://192.168.1.9:49152/description.xml\r\nST: urn:schemas-upnp-org:device:MediaRenderer:1\r\nUSN: uuid:abc::urn:schemas-upnp-org:device:MediaRenderer:1\r\n\r\n";
        let r = parse_answer(a, from()).unwrap();
        assert_eq!(r.location, "http://192.168.1.9:49152/description.xml");
        assert!(r.usn.starts_with("uuid:abc"));
        // Not a renderer, an error status, a location on the internet: ignored.
        let other = String::from_utf8_lossy(a).replace("MediaRenderer", "MediaServer");
        assert!(parse_answer(other.as_bytes(), from()).is_none());
        let err = String::from_utf8_lossy(a).replace("200 OK", "404 Not Found");
        assert!(parse_answer(err.as_bytes(), from()).is_none());
        let far = String::from_utf8_lossy(a).replace("192.168.1.9", "93.184.216.34");
        assert!(parse_answer(far.as_bytes(), from()).is_none());
        assert!(parse_answer(b"\xff\xfe", from()).is_none());
    }
}
