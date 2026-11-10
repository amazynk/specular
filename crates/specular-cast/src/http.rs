//! A small HTTP/1.1 client for devices on the local network (descriptions and SOAP calls):
//! plain `http://` to an IP address, one request per connection, bodies capped. It checks
//! the no-network switch before it opens a socket. Also the request-head reader the file
//! server and the fake renderer share.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::Duration;

use crate::{Error, Result};

/// The longest request or response head read.
pub(crate) const MAX_HEAD: usize = 16 * 1024;

/// An `http://` URL split up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Url {
    pub ip: IpAddr,
    pub port: u16,
    /// The path and query, from its `/`.
    pub path: String,
}

impl Url {
    /// Parses `http://ip[:port]/path`. Host names are refused: devices give addresses, and
    /// a name would need a lookup.
    pub fn parse(s: &str) -> Option<Url> {
        let rest = s.strip_prefix("http://").or_else(|| {
            s.get(..7)
                .filter(|p| p.eq_ignore_ascii_case("http://"))
                .map(|_| &s[7..])
        })?;
        let (auth, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let (ip, port) = if let Some(v6) = auth.strip_prefix('[') {
            let (h, p) = v6.split_once(']')?;
            let port = match p.strip_prefix(':') {
                Some(p) => p.parse().ok()?,
                None if p.is_empty() => 80,
                None => return None,
            };
            (IpAddr::V6(h.parse().ok()?), port)
        } else {
            let (h, p) = match auth.rsplit_once(':') {
                Some((h, p)) => (h, p.parse().ok()?),
                None => (auth, 80),
            };
            (IpAddr::V4(h.parse().ok()?), p)
        };
        Some(Url {
            ip,
            port,
            path: path.to_string(),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip, self.port)
    }

    /// `ip:port` as the Host header and URLs write it.
    pub fn authority(&self) -> String {
        authority(self.addr())
    }
}

/// `ip:port`, IPv6 in brackets.
pub fn authority(a: SocketAddr) -> String {
    match a.ip() {
        IpAddr::V4(v) => format!("{v}:{}", a.port()),
        IpAddr::V6(v) => format!("[{v}]:{}", a.port()),
    }
}

/// `rel` against `base` (a description's URL or its URLBase): absolute URLs as they are,
/// `/path` on the base's host, anything else beside the base's path.
pub fn resolve(base: &str, rel: &str) -> String {
    let rel = rel.trim();
    if rel.len() >= 7 && rel[..7].eq_ignore_ascii_case("http://") {
        return rel.to_string();
    }
    let Some(b) = Url::parse(base) else {
        return rel.to_string();
    };
    let root = format!("http://{}", b.authority());
    if rel.starts_with('/') {
        return root + rel;
    }
    let path = b.path.split(['?', '#']).next().unwrap_or("/");
    let dir = &path[..path.rfind('/').map_or(0, |i| i + 1)];
    let dir = if dir.is_empty() { "/" } else { dir };
    format!("{root}{dir}{rel}")
}

/// A response.
#[derive(Clone, Debug, Default)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }
}

pub(crate) fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// A head: its first line and headers.
pub(crate) type Head = (String, Vec<(String, String)>);

/// Reads a head (a request's or a response's) up to the blank line: its first line and
/// headers. `None` at a clean end of stream before anything came.
pub(crate) fn read_head(r: &mut impl BufRead) -> std::io::Result<Option<Head>> {
    let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, "bad HTTP head");
    let mut first = None;
    let mut headers = Vec::new();
    let mut total = 0;
    loop {
        let mut line = Vec::new();
        let n = r
            .by_ref()
            .take(MAX_HEAD as u64)
            .read_until(b'\n', &mut line)?;
        if n == 0 {
            return if first.is_none() && total == 0 {
                Ok(None)
            } else {
                Err(bad())
            };
        }
        total += n;
        if total > MAX_HEAD {
            return Err(bad());
        }
        let line = String::from_utf8_lossy(&line);
        let line = line.trim_end_matches(['\r', '\n']);
        if first.is_none() {
            if line.is_empty() {
                continue;
            }
            first = Some(line.to_string());
            continue;
        }
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    Ok(first.map(|f| (f, headers)))
}

/// Reads a body by its length, chunks or the connection's end, up to `max` bytes.
pub(crate) fn read_body(
    r: &mut impl BufRead,
    headers: &[(String, String)],
    max: usize,
) -> std::io::Result<Vec<u8>> {
    let big = || std::io::Error::new(std::io::ErrorKind::InvalidData, "body too large");
    let mut body = Vec::new();
    if header(headers, "transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked")) {
        loop {
            let mut line = String::new();
            r.by_ref().take(256).read_line(&mut line)?;
            let size = line.trim().split(';').next().unwrap_or("");
            let n = usize::from_str_radix(size, 16)
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "chunk"))?;
            if n == 0 {
                // Trailers, up to the blank line.
                for _ in 0..64 {
                    let mut t = String::new();
                    if r.by_ref().take(1024).read_line(&mut t)? == 0 || t.trim().is_empty() {
                        break;
                    }
                }
                break;
            }
            if body.len() + n > max {
                return Err(big());
            }
            let at = body.len();
            body.resize(at + n, 0);
            r.read_exact(&mut body[at..])?;
            let mut crlf = [0; 2];
            r.read_exact(&mut crlf)?;
        }
    } else if let Some(len) = header(headers, "content-length") {
        let n: usize = len
            .parse()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "length"))?;
        if n > max {
            return Err(big());
        }
        body.resize(n, 0);
        r.read_exact(&mut body)?;
    } else {
        r.take(max as u64 + 1).read_to_end(&mut body)?;
        if body.len() > max {
            return Err(big());
        }
    }
    Ok(body)
}

/// Sends one request to a device on the local network and reads its answer (bodies up to
/// [`crate::xml::MAX_BYTES`]). Refused while the network is off, and for an address that is
/// not local.
pub fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    timeout: Duration,
) -> Result<Response> {
    let u = Url::parse(url).ok_or(Error::BadReply("not an http:// address"))?;
    if !crate::is_local(u.ip) {
        return Err(Error::BadReply("not on the local network"));
    }
    specular_core::net::off::check()?;
    let mut s = TcpStream::connect_timeout(&u.addr(), timeout)?;
    s.set_read_timeout(Some(timeout))?;
    s.set_write_timeout(Some(timeout))?;
    let mut head = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: Specular/{} UPnP/1.0\r\n",
        u.path,
        u.authority(),
        env!("CARGO_PKG_VERSION"),
    );
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    if !body.is_empty() || method == "POST" {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("\r\n");
    s.write_all(head.as_bytes())?;
    s.write_all(body)?;
    s.flush()?;
    let mut r = BufReader::new(s);
    let (status_line, headers) = read_head(&mut r)?.ok_or(Error::BadReply("no answer"))?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or(Error::BadReply("no status"))?;
    let body = if method == "HEAD" {
        Vec::new()
    } else {
        read_body(&mut r, &headers, crate::xml::MAX_BYTES).map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData {
                Error::BadReply("broken body")
            } else {
                Error::from(e)
            }
        })?
    };
    Ok(Response {
        status,
        headers,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        let u = Url::parse("http://192.168.1.5:49152/desc.xml").unwrap();
        assert_eq!(u.ip.to_string(), "192.168.1.5");
        assert_eq!(u.port, 49152);
        assert_eq!(u.path, "/desc.xml");
        let u = Url::parse("HTTP://10.0.0.2").unwrap();
        assert_eq!((u.port, u.path.as_str()), (80, "/"));
        let u = Url::parse("http://[fe80::1]:8080/x?y").unwrap();
        assert_eq!(u.authority(), "[fe80::1]:8080");
        assert_eq!(u.path, "/x?y");
        assert!(Url::parse("http://tv.local/x").is_none());
        assert!(Url::parse("https://10.0.0.2/").is_none());
    }

    #[test]
    fn resolves() {
        let b = "http://10.0.0.2:1400/xml/device.xml";
        assert_eq!(resolve(b, "/ctl/AVT"), "http://10.0.0.2:1400/ctl/AVT");
        assert_eq!(resolve(b, "AVT/ctl"), "http://10.0.0.2:1400/xml/AVT/ctl");
        assert_eq!(resolve(b, "http://10.0.0.3/x"), "http://10.0.0.3/x");
        assert_eq!(
            resolve("http://10.0.0.2:1400", "ctl"),
            "http://10.0.0.2:1400/ctl"
        );
    }

    #[test]
    fn reads_chunked_bodies() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5;x=1\r\npedia\r\n0\r\n\r\n";
        let mut r = BufReader::new(&raw[..]);
        let (first, h) = read_head(&mut r).unwrap().unwrap();
        assert_eq!(first, "HTTP/1.1 200 OK");
        assert_eq!(read_body(&mut r, &h, 100).unwrap(), b"Wikipedia");
        let mut r = BufReader::new(&raw[..]);
        let (_, h) = read_head(&mut r).unwrap().unwrap();
        assert!(read_body(&mut r, &h, 5).is_err());
    }

    #[test]
    fn refuses_addresses_off_the_network() {
        let e = request(
            "GET",
            "http://8.8.8.8/",
            &[],
            &[],
            Duration::from_millis(10),
        );
        assert_eq!(e.err(), Some(Error::BadReply("not on the local network")));
    }
}
