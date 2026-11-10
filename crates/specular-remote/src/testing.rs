//! A small in-process HTTP/1.1 server for tests and screenshots: std `TcpListener` on
//! 127.0.0.1, keep-alive, chunked bodies, simulated latency and a shared bandwidth limit,
//! stalls and dropped connections. Nothing here touches the network beyond loopback.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// A request as the handler sees it.
#[derive(Clone, Debug)]
pub struct TestRequest {
    pub method: String,
    /// Path and query, as sent (`/studies?limit=10`).
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl TestRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    /// The path without the query.
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or("")
    }
    /// Query parameters (percent-decoded, `+` as space).
    pub fn query(&self) -> Vec<(String, String)> {
        let Some((_, q)) = self.target.split_once('?') else {
            return Vec::new();
        };
        q.split('&')
            .filter(|s| !s.is_empty())
            .map(|kv| {
                let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
                (decode(k), decode(v))
            })
            .collect()
    }
    pub fn param(&self, name: &str) -> Option<String> {
        self.query()
            .into_iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v)
    }
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("x"), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What to send back.
#[derive(Clone, Debug)]
pub struct TestResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Send the body with `Transfer-Encoding: chunked`, in pieces of this size.
    pub chunked: Option<usize>,
    /// After this many body bytes, stop sending: hold the connection (`hold`) then close.
    pub cut_after: Option<usize>,
    pub hold: Duration,
    /// Extra wait before the headers (on top of the server latency).
    pub delay: Duration,
}

impl TestResponse {
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        TestResponse {
            status,
            headers: Vec::new(),
            body: body.into(),
            chunked: None,
            cut_after: None,
            hold: Duration::ZERO,
            delay: Duration::ZERO,
        }
    }
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self::new(200, body)
    }
    pub fn status(status: u16) -> Self {
        Self::new(status, Vec::new())
    }
    pub fn header(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
    pub fn chunked(mut self, piece: usize) -> Self {
        self.chunked = Some(piece.max(1));
        self
    }
    /// Sends `n` body bytes, waits `hold`, then drops the connection.
    pub fn cut(mut self, n: usize, hold: Duration) -> Self {
        self.cut_after = Some(n);
        self.hold = hold;
        self
    }
    pub fn delay(mut self, d: Duration) -> Self {
        self.delay = d;
        self
    }

    /// Serves `data` honouring `Range` and `If-Range` against `etag` (single ranges only).
    pub fn ranged(req: &TestRequest, data: &[u8], etag: &str) -> Self {
        let full = || {
            TestResponse::ok(data.to_vec())
                .header("ETag", etag)
                .header("Accept-Ranges", "bytes")
        };
        let Some(range) = req.header("range") else {
            return full();
        };
        if let Some(v) = req.header("if-range")
            && v != etag
        {
            return full();
        }
        let Some(spec) = range.trim().strip_prefix("bytes=") else {
            return full();
        };
        let (a, b) = spec.split_once('-').unwrap_or((spec, ""));
        let n = data.len() as u64;
        let (start, end) = match (a.trim().parse::<u64>().ok(), b.trim().parse::<u64>().ok()) {
            (Some(s), Some(e)) => (s, e.min(n.saturating_sub(1))),
            (Some(s), None) => (s, n.saturating_sub(1)),
            (None, Some(suffix)) => (n.saturating_sub(suffix), n.saturating_sub(1)),
            (None, None) => return full(),
        };
        if start >= n || start > end {
            return TestResponse::status(416).header("Content-Range", format!("bytes */{n}"));
        }
        TestResponse::new(206, data[start as usize..=end as usize].to_vec())
            .header("ETag", etag)
            .header("Content-Range", format!("bytes {start}-{end}/{n}"))
    }
}

/// Server behaviour.
#[derive(Clone, Copy, Debug, Default)]
pub struct ServerOptions {
    /// Added before every response's headers (round-trip latency).
    pub latency: Duration,
    /// Bytes per second shared by all connections (a link), or unlimited.
    pub bandwidth: Option<u64>,
}

/// Counters.
#[derive(Debug, Default)]
pub struct ServerStats {
    pub requests: AtomicU64,
    pub connections: AtomicU64,
    pub bytes: AtomicU64,
}

type Handler = dyn Fn(&TestRequest) -> TestResponse + Send + Sync;

struct Shared {
    handler: Box<Handler>,
    options: ServerOptions,
    stop: AtomicBool,
    stats: ServerStats,
    /// The link: when the next byte may leave.
    link: Mutex<Instant>,
    log: Mutex<Vec<TestRequest>>,
}

/// A running server; stops when dropped.
pub struct TestServer {
    addr: SocketAddr,
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl TestServer {
    pub fn start(
        options: ServerOptions,
        handler: impl Fn(&TestRequest) -> TestResponse + Send + Sync + 'static,
    ) -> io::Result<TestServer> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let shared = Arc::new(Shared {
            handler: Box::new(handler),
            options,
            stop: AtomicBool::new(false),
            stats: ServerStats::default(),
            link: Mutex::new(Instant::now()),
            log: Mutex::new(Vec::new()),
        });
        let s = shared.clone();
        let thread = specular_thread::builder("net-test-srv").spawn(move || {
            for conn in listener.incoming() {
                if s.stop.load(Ordering::Acquire) {
                    break;
                }
                let Ok(conn) = conn else { continue };
                s.stats.connections.fetch_add(1, Ordering::Relaxed);
                let s = s.clone();
                let _ = specular_thread::builder("net-test-conn").spawn(move || {
                    let _ = serve(conn, &s);
                });
            }
        })?;
        Ok(TestServer {
            addr,
            shared,
            thread: Some(thread),
        })
    }

    /// `http://127.0.0.1:port` + `path`.
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }
    pub fn stats(&self) -> &ServerStats {
        &self.shared.stats
    }
    pub fn requests(&self) -> u64 {
        self.shared.stats.requests.load(Ordering::Relaxed)
    }
    /// Every request received so far.
    pub fn log(&self) -> Vec<TestRequest> {
        self.shared.log.lock().clone()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        // Wake the accept loop.
        let _ = TcpStream::connect(self.addr);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn serve(conn: TcpStream, s: &Shared) -> io::Result<()> {
    conn.set_read_timeout(Some(Duration::from_millis(100)))?;
    conn.set_nodelay(true)?;
    let mut w = conn.try_clone()?;
    let mut r = BufReader::new(conn);
    loop {
        // Request line, waiting for keep-alive requests until the server stops.
        let mut line = String::new();
        loop {
            if s.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            match r.read_line(&mut line) {
                Ok(0) => return Ok(()),
                Ok(_) => break,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let target = parts.next().unwrap_or("/").to_string();
        let mut headers = Vec::new();
        loop {
            let mut h = String::new();
            if r.read_line(&mut h)? == 0 {
                return Ok(());
            }
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                headers.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
        let len: usize = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0);
        let mut body = vec![0; len];
        r.read_exact(&mut body)?;
        let req = TestRequest {
            method,
            target,
            headers,
            body,
        };
        s.stats.requests.fetch_add(1, Ordering::Relaxed);
        s.log.lock().push(req.clone());
        let res = (s.handler)(&req);
        std::thread::sleep(s.options.latency + res.delay);
        let close = req
            .header("connection")
            .is_some_and(|v| v.eq_ignore_ascii_case("close"));
        let mut head = format!("HTTP/1.1 {} {}\r\n", res.status, reason(res.status));
        let has = |n: &str| res.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(n));
        for (k, v) in &res.headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        let head_only = req.method == "HEAD";
        if res.chunked.is_some() && !head_only {
            head.push_str("Transfer-Encoding: chunked\r\n");
        } else if !has("content-length") {
            head.push_str(&format!("Content-Length: {}\r\n", res.body.len()));
        }
        if close {
            head.push_str("Connection: close\r\n");
        }
        head.push_str("\r\n");
        send(&mut w, s, head.as_bytes())?;
        if !head_only {
            let body = match res.cut_after {
                Some(n) => &res.body[..n.min(res.body.len())],
                None => &res.body[..],
            };
            match res.chunked {
                Some(piece) => {
                    for c in body.chunks(piece) {
                        send(&mut w, s, format!("{:x}\r\n", c.len()).as_bytes())?;
                        send(&mut w, s, c)?;
                        send(&mut w, s, b"\r\n")?;
                    }
                    if res.cut_after.is_none() {
                        send(&mut w, s, b"0\r\n\r\n")?;
                    }
                }
                None => send(&mut w, s, body)?,
            }
        }
        w.flush()?;
        if res.cut_after.is_some() {
            let end = Instant::now() + res.hold;
            while Instant::now() < end && !s.stop.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(10));
            }
            let _ = w.shutdown(std::net::Shutdown::Both);
            return Ok(());
        }
        if close {
            return Ok(());
        }
    }
}

/// Writes through the shared link, paced to the bandwidth.
fn send(w: &mut TcpStream, s: &Shared, data: &[u8]) -> io::Result<()> {
    const PIECE: usize = 16 * 1024;
    for c in data.chunks(PIECE) {
        if let Some(bps) = s.options.bandwidth.filter(|b| *b > 0) {
            let slot = {
                let mut next = s.link.lock();
                let now = Instant::now();
                let start = (*next).max(now);
                *next = start + Duration::from_secs_f64(c.len() as f64 / bps as f64);
                *next
            };
            let now = Instant::now();
            if slot > now {
                std::thread::sleep(slot - now);
            }
        }
        if s.stop.load(Ordering::Acquire) {
            return Err(io::Error::other("server stopped"));
        }
        w.write_all(c)?;
        s.stats.bytes.fetch_add(c.len() as u64, Ordering::Relaxed);
    }
    Ok(())
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        206 => "Partial Content",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        406 => "Not Acceptable",
        416 => "Range Not Satisfiable",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Status",
    }
}
