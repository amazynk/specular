//! HTTP/1.1 for XML-RPC between processes on this computer: a POST client and a server that
//! answer only loopback addresses. Every request is one connection (`Connection: close`).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The largest request or response body (SAMP messages are small; a VOTable travels by URL).
pub const MAX_BODY: usize = 8 << 20;
/// The longest header block.
const MAX_HEAD: usize = 64 << 10;
/// How long a peer may take.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// Why an exchange failed.
#[derive(Debug)]
pub enum Error {
    /// The address is not `http://` on this computer.
    NotLocal(String),
    Io(std::io::Error),
    /// The peer did not speak HTTP as expected.
    Protocol(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotLocal(u) => write!(f, "{u} is not on this computer"),
            Error::Io(e) => write!(f, "{e}"),
            Error::Protocol(p) => write!(f, "{p}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e)
    }
}

/// An `http://host:port/path` address on this computer: its socket address and path.
pub fn local(url: &str) -> Result<(SocketAddr, String), Error> {
    let not = || Error::NotLocal(url.to_owned());
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("HTTP://"))
        .ok_or_else(not)?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].to_owned()),
        None => (rest, "/".to_owned()),
    };
    if hostport.contains('@') {
        return Err(not());
    }
    let (host, port) = if let Some(h) = hostport.strip_prefix('[') {
        let (h, p) = h.split_once(']').ok_or_else(not)?;
        (h, p.strip_prefix(':').unwrap_or("80"))
    } else {
        match hostport.rsplit_once(':') {
            Some((h, p)) => (h, p),
            None => (hostport, "80"),
        }
    };
    let port: u16 = port.parse().map_err(|_| not())?;
    let ip = local_ip(host).ok_or_else(not)?;
    Ok((SocketAddr::new(ip, port), path))
}

/// This computer's name, where the system says it without a library call.
fn own_hostname() -> Option<String> {
    let from_file = |p: &str| std::fs::read_to_string(p).ok();
    let name = if cfg!(windows) {
        std::env::var("COMPUTERNAME").ok()
    } else {
        from_file("/proc/sys/kernel/hostname")
            .or_else(|| from_file("/etc/hostname"))
            .or_else(|| std::env::var("HOSTNAME").ok())
            .or_else(|| std::env::var("HOST").ok())
    }?;
    let n = name.trim().to_ascii_lowercase();
    (!n.is_empty()).then_some(n)
}

/// Whether `ip` is one of this computer's own addresses (one a socket can be bound to).
fn is_own(ip: IpAddr) -> bool {
    ip.is_loopback() || TcpListener::bind((ip, 0)).is_ok()
}

/// The address of `host` when it is this computer: a loopback address, one of its own
/// addresses, `localhost`, or this computer's own name (which hubs such as astropy's write
/// in their lock files). Any other name is never looked up.
fn local_ip(host: &str) -> Option<IpAddr> {
    let h = host.to_ascii_lowercase();
    if h == "localhost" {
        return Some(IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
    if let Ok(ip) = h.parse::<IpAddr>() {
        return is_own(ip).then_some(ip);
    }
    let own = own_hostname()?;
    let short = |s: &str| s.split('.').next().unwrap_or("").to_owned();
    if h != own && !(short(&h) == short(&own) && (h == short(&h) || own == short(&own))) {
        return None;
    }
    use std::net::ToSocketAddrs;
    let ips: Vec<IpAddr> = (h.as_str(), 0)
        .to_socket_addrs()
        .ok()?
        .map(|a| a.ip())
        .collect();
    if ips.is_empty() || !ips.iter().all(|ip| is_own(*ip)) {
        return None;
    }
    ips.iter()
        .find(|ip| ip.is_loopback())
        .or(ips.first())
        .copied()
}

/// Whether a host name (from any URL) is this computer.
pub fn is_local_host(host: &str) -> bool {
    local_ip(host.trim_start_matches('[').trim_end_matches(']')).is_some()
}

/// Reads one HTTP message head and body from `r`: the first line, the headers and the body.
/// An HTTP message: its first line, headers (names lower-cased) and body.
type Message = (String, Vec<(String, String)>, Vec<u8>);

fn read_message(r: &mut impl BufRead) -> Result<Message, Error> {
    let mut head = Vec::new();
    let mut first = String::new();
    let mut headers = Vec::new();
    loop {
        let mut line = Vec::new();
        let n = r
            .by_ref()
            .take(MAX_HEAD as u64)
            .read_until(b'\n', &mut line)?;
        if n == 0 {
            return Err(Error::Protocol("the connection closed early".into()));
        }
        head.extend_from_slice(&line);
        if head.len() > MAX_HEAD {
            return Err(Error::Protocol("headers too long".into()));
        }
        let l = String::from_utf8_lossy(&line).trim_end().to_owned();
        if l.is_empty() {
            if first.is_empty() {
                continue;
            }
            break;
        }
        if first.is_empty() {
            first = l;
        } else if let Some((k, v)) = l.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_owned()));
        }
    }
    let get = |k: &str| {
        headers
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_str())
    };
    if get("transfer-encoding").is_some_and(|t| !t.eq_ignore_ascii_case("identity")) {
        return Err(Error::Protocol("chunked bodies are not read".into()));
    }
    let body = match get("content-length") {
        Some(n) => {
            let n: usize = n
                .parse()
                .map_err(|_| Error::Protocol("bad Content-Length".into()))?;
            if n > MAX_BODY {
                return Err(Error::Protocol("body too large".into()));
            }
            let mut b = vec![0; n];
            r.read_exact(&mut b)?;
            b
        }
        None => {
            let mut b = Vec::new();
            r.by_ref().take(MAX_BODY as u64 + 1).read_to_end(&mut b)?;
            if b.len() > MAX_BODY {
                return Err(Error::Protocol("body too large".into()));
            }
            b
        }
    };
    Ok((first, headers, body))
}

/// POSTs `body` (XML) to a local `url` and returns the response body.
pub fn post(url: &str, body: &[u8]) -> Result<Vec<u8>, Error> {
    let (addr, path) = local(url)?;
    let mut s = TcpStream::connect_timeout(&addr, TIMEOUT)?;
    s.set_read_timeout(Some(TIMEOUT))?;
    s.set_write_timeout(Some(TIMEOUT))?;
    let host = match addr.ip() {
        IpAddr::V6(ip) => format!("[{ip}]:{}", addr.port()),
        IpAddr::V4(ip) => format!("{ip}:{}", addr.port()),
    };
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: Specular\r\nContent-Type: text/xml\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes())?;
    s.write_all(body)?;
    s.flush()?;
    let mut r = BufReader::new(s);
    let (status, _, body) = read_message(&mut r)?;
    let code = status.split_whitespace().nth(1).unwrap_or("");
    if code != "200" {
        return Err(Error::Protocol(format!("the peer answered {status}")));
    }
    Ok(body)
}

/// Fetches a local `http://` address (a file another SAMP application serves).
pub fn get(url: &str, limit: usize) -> Result<Vec<u8>, Error> {
    let (addr, path) = local(url)?;
    let mut s = TcpStream::connect_timeout(&addr, TIMEOUT)?;
    s.set_read_timeout(Some(TIMEOUT))?;
    let head = format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\nUser-Agent: Specular\r\nConnection: close\r\n\r\n",
        addr
    );
    s.write_all(head.as_bytes())?;
    let mut r = BufReader::new(s);
    let mut head = Vec::new();
    // Status and headers, then the body as it comes (no length limit beyond `limit`).
    loop {
        let mut line = Vec::new();
        if r.by_ref()
            .take(MAX_HEAD as u64)
            .read_until(b'\n', &mut line)?
            == 0
        {
            return Err(Error::Protocol("the connection closed early".into()));
        }
        let done = line == b"\r\n" || line == b"\n";
        head.push(String::from_utf8_lossy(&line).trim_end().to_owned());
        if done || head.len() > 200 {
            break;
        }
    }
    let status = head.first().cloned().unwrap_or_default();
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(Error::Protocol(format!("the peer answered {status}")));
    }
    if head.iter().any(|h| {
        h.to_ascii_lowercase()
            .starts_with("transfer-encoding: chunked")
    }) {
        return Err(Error::Protocol("chunked bodies are not read".into()));
    }
    let mut body = Vec::new();
    r.take(limit as u64 + 1).read_to_end(&mut body)?;
    if body.len() > limit {
        return Err(Error::Protocol("the file is too large".into()));
    }
    Ok(body)
}

/// Answers one POST body with a response body (XML).
pub type Handler = Arc<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>;

/// A server on `127.0.0.1` (a free port) calling `handler` for each POST; stopped on drop.
pub struct Server {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
}

impl Server {
    pub fn start(thread: &str, handler: Handler) -> std::io::Result<Server> {
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let addr = l.local_addr()?;
        l.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let st = stop.clone();
        specular_thread::builder(thread).spawn(move || {
            while !st.load(Ordering::Relaxed) {
                match l.accept() {
                    Ok((s, peer)) => {
                        if !peer.ip().is_loopback() {
                            continue;
                        }
                        let _ = serve(s, &handler);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(15));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(50)),
                }
            }
        })?;
        Ok(Server { addr, stop })
    }

    /// The address to give the hub.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/xmlrpc", self.addr.port())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn serve(s: TcpStream, handler: &Handler) -> Result<(), Error> {
    s.set_nonblocking(false)?;
    s.set_read_timeout(Some(TIMEOUT))?;
    s.set_write_timeout(Some(TIMEOUT))?;
    let mut r = BufReader::new(s.try_clone()?);
    let (first, _, body) = match read_message(&mut r) {
        Ok(m) => m,
        Err(e) => {
            let _ = reply(&s, "400 Bad Request", b"");
            return Err(e);
        }
    };
    if !first.starts_with("POST ") {
        return reply(&s, "405 Method Not Allowed", b"");
    }
    let out = handler(&body);
    reply(&s, "200 OK", &out)
}

fn reply(mut s: &TcpStream, status: &str, body: &[u8]) -> Result<(), Error> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes())?;
    s.write_all(body)?;
    s.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_addresses() {
        assert_eq!(
            local("http://127.0.0.1:2112/xmlrpc").unwrap(),
            ("127.0.0.1:2112".parse().unwrap(), "/xmlrpc".into())
        );
        assert_eq!(
            local("http://localhost:80").unwrap().0,
            "127.0.0.1:80".parse().unwrap()
        );
        assert_eq!(
            local("http://[::1]:5/").unwrap().0,
            "[::1]:5".parse().unwrap()
        );
        for bad in [
            "http://192.168.1.2:80/",
            "http://example.org/",
            "https://127.0.0.1/",
            "http://user@127.0.0.1/",
            "http://127.0.0.1:99999/",
            "file:///x",
        ] {
            assert!(local(bad).is_err(), "{bad}");
        }
        assert!(is_local_host("LOCALHOST") && is_local_host("[::1]") && !is_local_host("a.b"));
    }

    #[test]
    fn post_to_own_server() {
        let srv = Server::start(
            "test-samp-srv",
            Arc::new(|b: &[u8]| {
                let mut v = b.to_vec();
                v.reverse();
                v
            }),
        )
        .unwrap();
        assert_eq!(post(&srv.url(), b"abc").unwrap(), b"cba");
        // A large body.
        let big = vec![b'x'; 1 << 20];
        assert_eq!(post(&srv.url(), &big).unwrap().len(), 1 << 20);
        let too_big = vec![b'x'; MAX_BODY + 1];
        assert!(post(&srv.url(), &too_big).is_err());
    }
}
