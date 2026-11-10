//! The file a renderer plays, served over HTTP while casting: bound to the address of the
//! interface that reaches the renderer (not every interface), answering only the renderer's
//! own address, only that file (and its subtitles, when sent) under a random 128-bit path,
//! GET and HEAD with single byte ranges. It stops when dropped.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::http::{header, read_head};

/// Connections served at once (renderers open two or three: probe, play, seek).
const MAX_CONNECTIONS: usize = 8;
/// How often the accept loop looks at the stop flag.
const POLL: Duration = Duration::from_millis(50);
/// A connection idle this long is closed.
const IDLE: Duration = Duration::from_secs(30);

/// The local address that reaches `remote`: the one the system routes from (no packet is
/// sent: a UDP socket's connect only picks the route).
pub fn local_ip_towards(remote: IpAddr) -> std::io::Result<IpAddr> {
    if remote.is_loopback() {
        return Ok(remote);
    }
    let bind: SocketAddr = if remote.is_ipv4() {
        (std::net::Ipv4Addr::UNSPECIFIED, 0).into()
    } else {
        (std::net::Ipv6Addr::UNSPECIFIED, 0).into()
    };
    let s = UdpSocket::bind(bind)?;
    s.connect((remote, 1900))?;
    Ok(s.local_addr()?.ip())
}

/// A random path token (32 hex digits).
fn token() -> std::io::Result<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// The file's name for the URL: ASCII letters, digits, `.`, `-` and `_` kept (renderers
/// guess types from the extension), the rest `_`.
fn url_name(path: &Path) -> String {
    let n = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut s: String = n
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() || s.starts_with('.') {
        s.insert_str(0, "media");
    }
    s
}

/// A file served.
struct Served {
    path: PathBuf,
    mime: String,
    /// Its path (`/<token>/<name>`).
    url_path: String,
}

struct Shared {
    /// The media first, then its subtitles.
    files: Vec<Served>,
    /// The one address answered (the renderer's); `None`: any.
    peer: Option<IpAddr>,
    /// The subtitles' URL, said to Samsung renderers asking with `getcaptionInfo.sec`.
    caption: Option<String>,
    stop: AtomicBool,
    open: AtomicUsize,
    requests: AtomicU64,
    bytes: AtomicU64,
}

/// The running server.
pub struct FileServer {
    shared: Arc<Shared>,
    addr: SocketAddr,
    thread: Option<JoinHandle<()>>,
}

impl FileServer {
    /// Serves `path` as `mime` on `ip` (a free port) to anyone. Refused while the network is
    /// off.
    pub fn start(ip: IpAddr, path: &Path, mime: &str) -> std::io::Result<FileServer> {
        FileServer::start_for(ip, None, &[(path, mime)])
    }

    /// Serves `files` (path and type; the media first) on `ip` (a free port), answering
    /// only `peer` when given: other addresses are hung up on before anything is read.
    /// Refused while the network is off.
    pub fn start_for(
        ip: IpAddr,
        peer: Option<IpAddr>,
        files: &[(&Path, &str)],
    ) -> std::io::Result<FileServer> {
        specular_core::net::off::check()?;
        let token = token()?;
        let mut served = Vec::new();
        for (path, mime) in files {
            // The file must be there and readable now.
            File::open(path)?;
            served.push(Served {
                path: path.to_path_buf(),
                mime: (*mime).to_string(),
                url_path: format!("/{token}/{}", url_name(path)),
            });
        }
        if served.is_empty() {
            return Err(std::io::Error::other("nothing to serve"));
        }
        let listener = TcpListener::bind((ip, 0))?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let caption = served
            .get(1)
            .map(|s| format!("http://{}{}", crate::http::authority(addr), s.url_path));
        let shared = Arc::new(Shared {
            files: served,
            peer: peer.map(canonical),
            caption,
            stop: AtomicBool::new(false),
            open: AtomicUsize::new(0),
            requests: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
        });
        let s = shared.clone();
        let thread = specular_thread::spawn("net-cast-serve", move || accept_loop(&listener, &s));
        Ok(FileServer {
            shared,
            addr,
            thread: Some(thread),
        })
    }

    /// The media's URL.
    pub fn url(&self) -> String {
        self.url_of(0).unwrap_or_default()
    }

    /// The URL of served file `i` (0: the media, then its subtitles).
    pub fn url_of(&self, i: usize) -> Option<String> {
        let f = self.shared.files.get(i)?;
        Some(format!(
            "http://{}{}",
            crate::http::authority(self.addr),
            f.url_path
        ))
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Requests answered (any status) and file bytes sent so far.
    pub fn requests(&self) -> u64 {
        self.shared.requests.load(Ordering::Relaxed)
    }

    pub fn bytes_sent(&self) -> u64 {
        self.shared.bytes.load(Ordering::Relaxed)
    }
}

impl Drop for FileServer {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn accept_loop(listener: &TcpListener, s: &Arc<Shared>) {
    while !s.stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((conn, peer)) => {
                // Only the renderer: anyone else on the network is hung up on.
                if s.peer.is_some_and(|p| p != canonical(peer.ip())) {
                    specular_log::debug!(target: specular_log::IO, "cast: refused {peer}");
                    drop(conn);
                    continue;
                }
                if s.open.load(Ordering::Acquire) >= MAX_CONNECTIONS {
                    drop(conn);
                    continue;
                }
                s.open.fetch_add(1, Ordering::AcqRel);
                let s2 = s.clone();
                let spawned = specular_thread::builder("net-cast-conn").spawn(move || {
                    if let Err(e) = serve_connection(conn, &s2) {
                        specular_log::debug!(target: specular_log::IO, "cast: {peer}: {e}");
                    }
                    s2.open.fetch_sub(1, Ordering::AcqRel);
                });
                if spawned.is_err() {
                    s.open.fetch_sub(1, Ordering::AcqRel);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(POLL),
            Err(_) => std::thread::sleep(POLL),
        }
    }
}

/// A `Range` header against a file of `len` bytes: `Some(Ok((start, end)))` inclusive,
/// `Some(Err(()))` unsatisfiable, `None` for the whole file (no header, or one we ignore).
pub fn parse_range(h: Option<&str>, len: u64) -> Option<Result<(u64, u64), ()>> {
    let spec = h?.trim().strip_prefix("bytes=")?;
    // Several ranges: answered with the whole file (allowed by RFC 9110).
    if spec.contains(',') {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (a, b) = (a.trim(), b.trim());
    let r = if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        if n == 0 || len == 0 {
            return Some(Err(()));
        }
        (len.saturating_sub(n), len - 1)
    } else {
        let start: u64 = a.parse().ok()?;
        let end = if b.is_empty() {
            len.saturating_sub(1)
        } else {
            b.parse::<u64>().ok()?.min(len.saturating_sub(1))
        };
        if start >= len || end < start {
            return Some(Err(()));
        }
        (start, end)
    };
    Some(Ok(r))
}

fn serve_connection(conn: TcpStream, s: &Shared) -> std::io::Result<()> {
    conn.set_nonblocking(false)?;
    conn.set_read_timeout(Some(IDLE))?;
    conn.set_write_timeout(Some(IDLE))?;
    let mut out = conn.try_clone()?;
    let mut r = BufReader::new(conn);
    loop {
        if s.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        let Some((line, headers)) = read_head(&mut r)? else {
            return Ok(());
        };
        s.requests.fetch_add(1, Ordering::Relaxed);
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let target = parts.next().unwrap_or("");
        let close = header(&headers, "connection").is_some_and(|c| c.eq_ignore_ascii_case("close"))
            || line.ends_with("HTTP/1.0");
        // Anything with a body is refused, so a body never has to be skipped.
        let head_only = method == "HEAD";
        if method != "GET" && !head_only {
            simple(&mut out, 405, "Method Not Allowed", "Allow: GET, HEAD\r\n")?;
            return Ok(());
        }
        let want = target.split('?').next().unwrap_or("");
        let Some((i, file)) = s.files.iter().enumerate().find(|(_, f)| f.url_path == want) else {
            simple(&mut out, 404, "Not Found", "")?;
            if close {
                return Ok(());
            }
            continue;
        };
        let mut f = File::open(&file.path)?;
        let len = f.metadata()?.len();
        // Samsung renderers ask for the subtitles' address with the media's.
        let caption = match (&s.caption, header(&headers, "getcaptioninfo.sec")) {
            (Some(c), Some(v)) if i == 0 && v.trim() == "1" => format!("CaptionInfo.sec: {c}\r\n"),
            _ => String::new(),
        };
        let features = if i == 0 {
            format!(
                "contentFeatures.dlna.org: {}\r\n",
                crate::accept::DLNA_FEATURES
            )
        } else {
            String::new()
        };
        let common = format!(
            "Content-Type: {}\r\nAccept-Ranges: bytes\r\ntransferMode.dlna.org: {}\r\n\
{features}{caption}Cache-Control: no-store\r\nServer: Specular/{} UPnP/1.0\r\n{}",
            file.mime,
            if i == 0 { "Streaming" } else { "Interactive" },
            env!("CARGO_PKG_VERSION"),
            if close { "Connection: close\r\n" } else { "" }
        );
        let (start, end, status) = match parse_range(header(&headers, "range"), len) {
            Some(Err(())) => {
                let extra = format!("Content-Range: bytes */{len}\r\n{common}");
                simple(&mut out, 416, "Range Not Satisfiable", &extra)?;
                if close {
                    return Ok(());
                }
                continue;
            }
            Some(Ok((a, b))) => (a, b, 206),
            None => (0, len.saturating_sub(1), 200),
        };
        let count = if len == 0 { 0 } else { end - start + 1 };
        let mut head = if status == 206 {
            format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{len}\r\n")
        } else {
            "HTTP/1.1 200 OK\r\n".to_string()
        };
        head.push_str(&format!("Content-Length: {count}\r\n{common}\r\n"));
        out.write_all(head.as_bytes())?;
        if !head_only && count > 0 {
            f.seek(SeekFrom::Start(start))?;
            let mut left = count;
            let mut buf = vec![0u8; 64 * 1024];
            while left > 0 {
                if s.stop.load(Ordering::Acquire) {
                    return Ok(());
                }
                let want = left.min(buf.len() as u64) as usize;
                let n = f.read(&mut buf[..want])?;
                if n == 0 {
                    // The file shrank: the connection ends short.
                    return Ok(());
                }
                out.write_all(&buf[..n])?;
                s.bytes.fetch_add(n as u64, Ordering::Relaxed);
                left -= n as u64;
            }
        }
        out.flush()?;
        if close {
            return Ok(());
        }
    }
}

/// An IPv4 address mapped into IPv6 as the IPv4 address.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 => v4,
    }
}

fn simple(out: &mut TcpStream, code: u16, reason: &str, extra: &str) -> std::io::Result<()> {
    write!(
        out,
        "HTTP/1.1 {code} {reason}\r\nContent-Length: 0\r\n{extra}\r\n"
    )?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range(Some("bytes=0-99"), 1000), Some(Ok((0, 99))));
        assert_eq!(parse_range(Some("bytes=900-"), 1000), Some(Ok((900, 999))));
        assert_eq!(parse_range(Some("bytes=-100"), 1000), Some(Ok((900, 999))));
        assert_eq!(parse_range(Some("bytes=-5000"), 1000), Some(Ok((0, 999))));
        assert_eq!(
            parse_range(Some("bytes=990-2000"), 1000),
            Some(Ok((990, 999)))
        );
        assert_eq!(parse_range(Some("bytes=1000-"), 1000), Some(Err(())));
        assert_eq!(parse_range(Some("bytes=5-2"), 1000), Some(Err(())));
        assert_eq!(parse_range(Some("bytes=0-1,5-6"), 1000), None);
        assert_eq!(parse_range(Some("items=0-1"), 1000), None);
        assert_eq!(parse_range(None, 1000), None);
    }

    #[test]
    fn names() {
        assert_eq!(url_name(Path::new("/a/My Film (2).mp4")), "My_Film__2_.mp4");
        assert_eq!(url_name(Path::new("/a/שיר.flac")), "___.flac");
        assert_eq!(url_name(Path::new("/a/.hidden")), "media.hidden");
    }

    #[test]
    fn tokens_differ() {
        let (a, b) = (token().unwrap(), token().unwrap());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }
}
