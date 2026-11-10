//! A blocking HTTP/HTTPS client: range requests, a keep-alive connection pool, timeouts,
//! cancellation through [`CancelToken`] and retries with backoff.
//!
//! Built on `ureq` (HTTP/1.1, rustls with the system trust store; no async runtime, no
//! OpenSSL). Requests run on the caller's thread, which is meant to be a worker: the viewer
//! never calls this from the UI thread.
//!
//! Retries: idempotent requests (`GET`, `HEAD`) are retried on connection failures, timeouts
//! and `429/502/503/504` (honouring `Retry-After`), only while no body bytes have reached the
//! caller. [`Client::download_to`] goes further: with a validator (ETag or Last-Modified) a
//! partial download resumes with `Range` + `If-Range`, also across calls.
//!
//! Every request reports into the download-activity registry ([`specular_core::net`]): the
//! window's "Downloads" indicator. A client says what it fetches with [`Client::tracked`]
//! (or a request with [`Request::activity`]); the rest show as "Data from <host>". The
//! registry's Stop cancels the request like its cancel token does.

use std::fmt;
use std::io::{self, Read, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use std::cell::RefCell;

use specular_core::CancelToken;
use specular_core::net::{self, Failure, Label, What};

use crate::auth::Auth;

/// Read size between cancellation checks.
const CHUNK: usize = 64 * 1024;

/// Client settings.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// Time to open a connection (TCP + TLS).
    pub connect_timeout: Duration,
    /// Time from sending a request to receiving the response headers.
    pub response_timeout: Duration,
    /// Deadline for a whole response body (per attempt). `None`: no deadline; cancel instead.
    pub body_timeout: Option<Duration>,
    /// Extra attempts after the first.
    pub retries: u32,
    /// First backoff; doubles per attempt, with jitter.
    pub backoff: Duration,
    /// Longest wait between attempts (also caps `Retry-After`).
    pub max_backoff: Duration,
    /// Idle keep-alive connections kept per host.
    pub max_idle_per_host: usize,
    pub user_agent: String,
    /// Use `HTTPS_PROXY`/`ALL_PROXY`/… from the environment.
    pub env_proxy: bool,
    /// An HTTP proxy for every request (`http://host:port`), before `env_proxy`.
    pub proxy: Option<String>,
    /// Verify servers with the operating system's trust store (hospital CAs); otherwise the
    /// bundled Mozilla roots.
    pub system_roots: bool,
    /// Report requests of an untagged client to the download-activity registry (a client
    /// made with [`Client::tracked`] always reports).
    pub track: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        ClientConfig {
            connect_timeout: Duration::from_secs(10),
            response_timeout: Duration::from_secs(30),
            body_timeout: Some(Duration::from_secs(600)),
            retries: 3,
            backoff: Duration::from_millis(250),
            max_backoff: Duration::from_secs(8),
            max_idle_per_host: 8,
            user_agent: concat!("Specular/", env!("CARGO_PKG_VERSION")).to_string(),
            env_proxy: true,
            proxy: None,
            system_roots: true,
            track: true,
        }
    }
}

impl ClientConfig {
    /// Settings for tests against a local server: short timeouts, short backoff, no proxy.
    pub fn for_tests() -> Self {
        ClientConfig {
            connect_timeout: Duration::from_secs(2),
            response_timeout: Duration::from_secs(5),
            body_timeout: Some(Duration::from_secs(30)),
            retries: 2,
            backoff: Duration::from_millis(10),
            max_backoff: Duration::from_millis(100),
            env_proxy: false,
            system_roots: false,
            track: false,
            ..ClientConfig::default()
        }
    }
}

/// What went wrong.
#[derive(Debug)]
pub enum Error {
    /// The cancel token was set.
    Cancelled,
    /// The server answered with an error status (after retries).
    Status(u16),
    /// No answer in time.
    Timeout,
    /// The host could not be resolved or reached: offline, wrong address, firewall.
    Connect(String),
    /// TLS handshake or certificate failure.
    Tls(String),
    /// Reading or writing failed (network or disk).
    Io(io::Error),
    /// The server spoke something that is not valid HTTP, or a bad URL.
    Protocol(String),
    /// The no-network switch is on ([`specular_core::net::off`]): nothing was sent.
    NetworkOff,
}

impl Error {
    /// The server could not be reached at all (offline mode makes sense).
    pub fn is_offline(&self) -> bool {
        matches!(self, Error::Connect(_) | Error::Timeout | Error::NetworkOff)
    }
    /// The no-network switch refused the request (the app says "Network is off").
    pub fn is_network_off(&self) -> bool {
        matches!(self, Error::NetworkOff)
    }
    /// Credentials missing or refused.
    pub fn is_auth(&self) -> bool {
        matches!(self, Error::Status(401 | 403))
    }
    fn retryable(&self) -> bool {
        match self {
            Error::Status(s) => matches!(s, 408 | 429 | 502 | 503 | 504),
            Error::Timeout | Error::Connect(_) => true,
            Error::Io(e) => !matches!(e.kind(), io::ErrorKind::StorageFull),
            Error::Protocol(_) => true,
            Error::Cancelled | Error::Tls(_) | Error::NetworkOff => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Cancelled => write!(f, "cancelled"),
            Error::Status(s) => write!(f, "HTTP {s}"),
            Error::Timeout => write!(f, "timed out"),
            Error::Connect(m) => write!(f, "cannot connect: {m}"),
            Error::Tls(m) => write!(f, "TLS: {m}"),
            Error::Io(e) => write!(f, "{e}"),
            Error::Protocol(m) => write!(f, "{m}"),
            Error::NetworkOff => f.write_str(net::off::MESSAGE),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

fn map_ureq(e: ureq::Error) -> Error {
    use ureq::Error as U;
    match e {
        U::StatusCode(s) => Error::Status(s),
        U::Timeout(_) => Error::Timeout,
        U::HostNotFound => Error::Connect("host not found".into()),
        U::ConnectionFailed => Error::Connect("connection failed".into()),
        U::ConnectProxyFailed(m) => Error::Connect(format!("proxy: {m}")),
        U::Io(e) => match e.kind() {
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => Error::Timeout,
            io::ErrorKind::ConnectionRefused
            | io::ErrorKind::NetworkUnreachable
            | io::ErrorKind::HostUnreachable
            | io::ErrorKind::AddrNotAvailable => Error::Connect(e.to_string()),
            _ => Error::Io(e),
        },
        U::Tls(m) => Error::Tls(m.to_string()),
        U::Rustls(e) => Error::Tls(e.to_string()),
        U::Pem(e) => Error::Tls(e.to_string()),
        U::BodyStalled => Error::Timeout,
        other => Error::Protocol(other.to_string()),
    }
}

/// Response headers (names lower-case, in arrival order).
#[derive(Clone, Debug, Default)]
pub struct Headers(pub Vec<(String, String)>);

impl Headers {
    /// First value of `name` (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    pub fn content_type(&self) -> Option<&str> {
        self.get("content-type")
    }
    pub fn content_length(&self) -> Option<u64> {
        self.get("content-length")?.trim().parse().ok()
    }
    pub fn etag(&self) -> Option<&str> {
        self.get("etag")
    }
    pub fn last_modified(&self) -> Option<&str> {
        self.get("last-modified")
    }
    /// `Content-Range: bytes a-b/total` as `(a, b, total)`; `total` is `None` for `*`.
    pub fn content_range(&self) -> Option<(u64, u64, Option<u64>)> {
        let v = self.get("content-range")?.trim();
        let v = v.strip_prefix("bytes")?.trim_start();
        let (span, total) = v.split_once('/')?;
        let (a, b) = span.split_once('-')?;
        Some((
            a.trim().parse().ok()?,
            b.trim().parse().ok()?,
            total.trim().parse().ok(),
        ))
    }
    /// A validator for `If-Range`: a strong ETag, else Last-Modified.
    pub fn validator(&self) -> Option<&str> {
        match self.etag() {
            Some(e) if !e.starts_with("W/") => Some(e),
            _ => self.last_modified(),
        }
    }
    /// `Retry-After` in seconds (HTTP dates are ignored).
    pub fn retry_after(&self) -> Option<Duration> {
        let s: u64 = self.get("retry-after")?.trim().parse().ok()?;
        Some(Duration::from_secs(s))
    }
}

/// Status and headers of a response.
#[derive(Clone, Debug)]
pub struct Head {
    pub status: u16,
    pub headers: Headers,
}

impl Head {
    /// Total size of the resource: from `Content-Range` on 206, else `Content-Length`.
    pub fn total_len(&self) -> Option<u64> {
        if self.status == 206 {
            self.headers.content_range().and_then(|r| r.2)
        } else {
            self.headers.content_length()
        }
    }
}

/// A buffered response.
#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Headers,
    pub body: Vec<u8>,
}

/// Download progress.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// Bytes so far (including any resumed prefix).
    pub received: u64,
    /// Expected total, when the server said.
    pub total: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    Get,
    Head,
    Post,
}

/// A request to send.
#[derive(Clone)]
pub struct Request {
    url: String,
    method: Method,
    headers: Vec<(String, String)>,
    range: Option<(u64, Option<u64>)>,
    auth: Auth,
    body_timeout: Option<Option<Duration>>,
    limit: Option<u64>,
    /// A `POST` body (it may hold a password: never printed).
    body: Option<Vec<u8>>,
    /// Follow redirects (the default); otherwise a 3xx is an [`Error::Status`].
    redirects: bool,
    /// What it fetches, for the download-activity registry (else the client's).
    what: Option<What>,
    /// Hand back a 4xx/5xx response with its body instead of an [`Error::Status`] (STOW-RS
    /// says per instance what failed in a 409's body).
    any_status: bool,
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Headers may carry tokens; only the names are shown.
        f.debug_struct("Request")
            .field("url", &self.url)
            .field("method", &self.method)
            .field(
                "headers",
                &self.headers.iter().map(|(k, _)| k).collect::<Vec<_>>(),
            )
            .field("range", &self.range)
            .field("auth", &self.auth)
            .field("body", &self.body.as_ref().map(Vec::len))
            .finish()
    }
}

impl Request {
    pub fn get(url: impl Into<String>) -> Self {
        Request {
            url: url.into(),
            method: Method::Get,
            headers: Vec::new(),
            range: None,
            auth: Auth::None,
            body_timeout: None,
            limit: None,
            body: None,
            redirects: true,
            what: None,
            any_status: false,
        }
    }
    /// A `POST` of `body` (sent as `content_type`). Not idempotent, so never retried: a
    /// failure is final, whatever it was.
    pub fn post(url: impl Into<String>, content_type: &str, body: Vec<u8>) -> Self {
        Request {
            method: Method::Post,
            body: Some(body),
            ..Request::get(url)
        }
        .header("Content-Type", content_type)
    }
    pub fn head(url: impl Into<String>) -> Self {
        Request {
            method: Method::Head,
            ..Request::get(url)
        }
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    /// Adds a header (replacing one of the same name).
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        let name = name.into();
        self.headers.retain(|(k, _)| !k.eq_ignore_ascii_case(&name));
        self.headers.push((name, value.into()));
        self
    }
    pub fn accept(self, value: impl Into<String>) -> Self {
        self.header("Accept", value)
    }
    /// Bytes `range.start..range.end` (end exclusive).
    pub fn range(mut self, range: std::ops::Range<u64>) -> Self {
        if range.end > range.start {
            self.range = Some((range.start, Some(range.end - 1)));
        }
        self
    }
    /// Bytes from `start` to the end.
    pub fn range_from(mut self, start: u64) -> Self {
        self.range = Some((start, None));
        self
    }
    pub fn auth(mut self, auth: &Auth) -> Self {
        self.auth = auth.clone();
        self
    }
    /// Overrides [`ClientConfig::body_timeout`] for this request.
    pub fn body_timeout(mut self, t: Option<Duration>) -> Self {
        self.body_timeout = Some(t);
        self
    }
    /// Refuses bodies larger than this (for buffered requests); the default is 1 GiB.
    pub fn limit(mut self, bytes: u64) -> Self {
        self.limit = Some(bytes);
        self
    }
    /// What this request fetches, as the "Downloads" indicator shows it (overrides
    /// [`Client::tracked`]).
    pub fn activity(mut self, what: What) -> Self {
        self.what = Some(what);
        self
    }
    /// Answers a redirect with [`Error::Status`] (3xx) instead of following it: a sign-in
    /// page a server redirects to is not the resource asked for.
    pub fn no_redirects(mut self) -> Self {
        self.redirects = false;
        self
    }
    /// Any status of 400 and above is a [`Response`] (with its body), not an
    /// [`Error::Status`].
    pub fn any_status(mut self) -> Self {
        self.any_status = true;
        self
    }
    fn has_header(&self, name: &str) -> bool {
        self.headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case(name))
    }
}

/// Shared HTTP client; cheap to clone, safe to use from many threads (connections are pooled).
#[derive(Clone)]
pub struct Client {
    agent: ureq::Agent,
    config: ClientConfig,
    /// What its requests fetch ([`Client::tracked`]).
    what: Option<What>,
    /// Where they report (tests); the global registry otherwise.
    registry: Option<net::Registry>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("config", &self.config)
            .field("what", &self.what)
            .finish()
    }
}

impl Client {
    pub fn new(config: ClientConfig) -> Result<Client, Error> {
        let roots = if config.system_roots {
            ureq::tls::RootCerts::PlatformVerifier
        } else {
            ureq::tls::RootCerts::WebPki
        };
        let tls = ureq::tls::TlsConfig::builder().root_certs(roots).build();
        let proxy = match &config.proxy {
            Some(p) => Some(ureq::Proxy::new(p).map_err(map_ureq)?),
            None if config.env_proxy => ureq::Proxy::try_from_env(),
            None => None,
        };
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(config.connect_timeout))
            .timeout_recv_response(Some(config.response_timeout))
            .timeout_recv_body(config.body_timeout)
            .max_idle_connections_per_host(config.max_idle_per_host)
            .max_idle_connections(config.max_idle_per_host * 8)
            .user_agent(config.user_agent.as_str())
            .tls_config(tls)
            .proxy(proxy)
            .build()
            .new_agent();
        Ok(Client {
            agent,
            config,
            what: None,
            registry: None,
        })
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// The same client (sharing its connections), its requests shown in the "Downloads"
    /// indicator as `what` from their host.
    pub fn tracked(&self, what: What) -> Client {
        Client {
            what: Some(what),
            ..self.clone()
        }
    }

    /// The same client reporting to `registry` instead of the global one (tests).
    pub fn reporting_to(&self, registry: net::Registry) -> Client {
        Client {
            registry: Some(registry),
            ..self.clone()
        }
    }

    /// Starts reporting `req` to the registry (nothing for an untracked client).
    pub(crate) fn track(&self, req: &Request) -> Track {
        // Refused requests are not downloads: the indicator never appears for them.
        if net::off::is_off() {
            return Track(None);
        }
        let what = req.what.clone().or_else(|| self.what.clone());
        if what.is_none() && !self.config.track && self.registry.is_none() {
            return Track(None);
        }
        let label = Label::new(what.unwrap_or(What::Data), &req.url);
        let reg = self.registry.as_ref().unwrap_or_else(|| net::global());
        Track(Some(RefCell::new(reg.begin(label))))
    }

    /// Sends one attempt; returns the head and the body reader on a 2xx.
    pub(crate) fn send(&self, req: &Request) -> Result<(Head, ureq::BodyReader<'static>), Error> {
        // The no-network switch's choke point: before any name lookup or socket.
        if net::off::is_off() {
            net::off::note_refused();
            return Err(Error::NetworkOff);
        }
        let uri: ureq::http::Uri = req
            .url
            .parse()
            .map_err(|e| Error::Protocol(format!("bad URL: {e}")))?;
        let mut b = ureq::http::Request::builder()
            .method(match req.method {
                Method::Get => "GET",
                Method::Head => "HEAD",
                Method::Post => "POST",
            })
            .uri(uri);
        for (k, v) in &req.headers {
            b = b.header(k.as_str(), v.as_str());
        }
        if let Some((a, e)) = req.range {
            let v = match e {
                Some(e) => format!("bytes={a}-{e}"),
                None => format!("bytes={a}-"),
            };
            b = b.header("Range", v);
            // Byte offsets refer to the stored representation, never a gzipped one.
            if !req.has_header("accept-encoding") {
                b = b.header("Accept-Encoding", "identity");
            }
        }
        if let Some(h) = req.auth.header_value() {
            b = b.header("Authorization", h.expose());
        }
        let bad = |e: ureq::http::Error| Error::Protocol(format!("bad request: {e}"));
        let res = match &req.body {
            Some(body) => self.run(b.body(body.as_slice()).map_err(bad)?, req),
            None => self.run(b.body(()).map_err(bad)?, req),
        }
        .map_err(map_ureq)?;
        let status = res.status().as_u16();
        let headers = Headers(
            res.headers()
                .iter()
                .map(|(k, v)| {
                    (
                        k.as_str().to_ascii_lowercase(),
                        String::from_utf8_lossy(v.as_bytes()).into_owned(),
                    )
                })
                .collect(),
        );
        let head = Head { status, headers };
        if !(200..300).contains(&status) && !(req.any_status && status >= 400) {
            specular_log::debug!(
                target: specular_log::IO,
                "http {} {}: {status}",
                match req.method {
                    Method::Get => "GET",
                    Method::Head => "HEAD",
                    Method::Post => "POST",
                },
                redact_url(&req.url)
            );
            // `with_retries` reads Retry-After from here.
            RETRY_AFTER.with(|c| c.set(head.headers.retry_after()));
            return Err(Error::Status(status));
        }
        let body = res.into_body().into_with_config().reader();
        Ok((head, body))
    }

    fn run(
        &self,
        r: ureq::http::Request<impl ureq::AsSendBody>,
        req: &Request,
    ) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
        if req.body_timeout.is_none() && req.redirects {
            return self.agent.run(r);
        }
        let mut c = self.agent.configure_request(r);
        if let Some(t) = req.body_timeout {
            c = c.timeout_recv_body(t);
        }
        if !req.redirects {
            c = c.max_redirects(0);
        }
        self.agent.run(c.build())
    }

    /// Runs `attempt` with retries. `attempt` reports whether bytes reached the caller
    /// (which makes a failure final).
    pub(crate) fn with_retries<T>(
        &self,
        cancel: &CancelToken,
        track: &Track,
        mut attempt: impl FnMut() -> (Result<T, Error>, bool),
    ) -> Result<T, Error> {
        let mut tries = 0u32;
        let stop = || cancel.is_cancelled() || track.cancelled();
        loop {
            if stop() {
                return Err(Error::Cancelled);
            }
            RETRY_AFTER.with(|c| c.set(None));
            let (r, delivered) = attempt();
            match r {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if stop() {
                        return Err(Error::Cancelled);
                    }
                    if delivered || !e.retryable() || tries >= self.config.retries {
                        return Err(e);
                    }
                    let base = self.config.backoff.saturating_mul(1 << tries.min(16));
                    let jitter = base.mul_f64(fastrand::f64() * 0.5);
                    let mut wait = (base + jitter).min(self.config.max_backoff);
                    if let Some(ra) = RETRY_AFTER.with(|c| c.get()) {
                        wait = ra.min(self.config.max_backoff);
                    }
                    specular_log::debug!(target: specular_log::IO, "http retry in {wait:?}: {e}");
                    tries += 1;
                    sleep_until(wait, &stop)?;
                }
            }
        }
    }

    /// `HEAD`: status and headers.
    pub fn head(&self, req: &Request, cancel: &CancelToken) -> Result<Head, Error> {
        let mut req = req.clone();
        req.method = Method::Head;
        let track = self.track(&req);
        let r = self.with_retries(cancel, &track, || {
            (
                self.send(&req).map(|(h, _)| {
                    track.head(Some(0), 0);
                    h
                }),
                false,
            )
        });
        track.end(r)
    }

    /// `GET` (or a [`Request::post`], sent once) into memory. A failure while reading the body is retried too (nothing has
    /// reached the caller yet).
    pub fn get(&self, req: &Request, cancel: &CancelToken) -> Result<Response, Error> {
        let limit = req.limit.unwrap_or(1 << 30);
        // A POST is sent once: the server may have acted on it whatever came back.
        let once = req.method == Method::Post;
        let track = self.track(req);
        let r = self.with_retries(cancel, &track, || {
            let r = (|| {
                let (head, mut body) = self.send(req)?;
                track.head(head.headers.content_length(), 0);
                let mut out = Vec::with_capacity(
                    head.headers.content_length().unwrap_or(0).min(limit) as usize,
                );
                read_all(&mut body, cancel, &track, limit, |c| {
                    out.extend_from_slice(c);
                    ControlFlow::Continue(())
                })?;
                Ok(Response {
                    status: head.status,
                    headers: head.headers,
                    body: out,
                })
            })();
            (r, once)
        });
        track.end(r)
    }

    /// Sends a [`Request::post`] and reads the response into memory (once, no retries).
    pub fn post(&self, req: &Request, cancel: &CancelToken) -> Result<Response, Error> {
        self.get(req, cancel)
    }

    /// `GET`, handing the body to `on_chunk` as it arrives. Returning `Break` stops reading
    /// (the result is still `Ok`). Retried only until the first chunk is delivered.
    pub fn get_stream(
        &self,
        req: &Request,
        cancel: &CancelToken,
        on_chunk: impl FnMut(&[u8], &Progress) -> ControlFlow<()>,
    ) -> Result<Head, Error> {
        self.get_stream_head(req, cancel, |_| ControlFlow::Continue(()), on_chunk)
    }

    /// [`get_stream`](Self::get_stream) that also shows the status and headers before the
    /// body (to pick a parser by Content-Type); `on_head` returning `Break` skips the body.
    /// `on_head` runs again if an attempt is retried.
    pub fn get_stream_head(
        &self,
        req: &Request,
        cancel: &CancelToken,
        mut on_head: impl FnMut(&Head) -> ControlFlow<()>,
        mut on_chunk: impl FnMut(&[u8], &Progress) -> ControlFlow<()>,
    ) -> Result<Head, Error> {
        let track = self.track(req);
        let r = self.with_retries(cancel, &track, || {
            let mut delivered = false;
            let r = (|| {
                let (head, mut body) = self.send(req)?;
                if on_head(&head).is_break() {
                    track.head(Some(0), 0);
                    return Ok(head);
                }
                track.head(head.headers.content_length(), 0);
                let total = head.total_len();
                let mut p = Progress { received: 0, total };
                read_all(&mut body, cancel, &track, u64::MAX, |c| {
                    delivered = true;
                    p.received += c.len() as u64;
                    on_chunk(c, &p)
                })?;
                Ok(head)
            })();
            (r, delivered)
        });
        track.end(r)
    }

    /// `GET` into the file `path`: written to `path.part` next to it and renamed when
    /// complete, so `path` is either absent or whole. A partial `.part` left by a failed or
    /// cancelled attempt is resumed with `Range` + `If-Range` when the server gave a
    /// validator (ETag or Last-Modified); a server that changed the file answers 200 and the
    /// download restarts. Mid-body failures are retried by resuming when possible.
    pub fn download_to(
        &self,
        req: &Request,
        cancel: &CancelToken,
        path: &Path,
        mut on_progress: impl FnMut(&Progress),
    ) -> Result<Head, Error> {
        let part = part_path(path);
        let meta = meta_path(&part);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let track = self.track(req);
        let r = self.with_retries(cancel, &track, || {
            let mut delivered = false;
            let r = (|| {
                // Resume only with a validator recorded from the same URL.
                let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
                let validator = std::fs::read_to_string(&meta).ok().and_then(|m| {
                    let (url, v) = m.split_once('\n')?;
                    (url == req.url && !v.trim().is_empty()).then(|| v.trim().to_string())
                });
                let mut r = req.clone();
                let resuming = have > 0 && validator.is_some() && req.range.is_none();
                if let (true, Some(v)) = (resuming, &validator) {
                    r = r.range_from(have).header("If-Range", v.clone());
                }
                let (head, mut body) = match self.send(&r) {
                    // The partial file is longer than the resource: start over.
                    Err(Error::Status(416)) if resuming => {
                        let _ = std::fs::remove_file(&part);
                        let _ = std::fs::remove_file(&meta);
                        self.send(req)?
                    }
                    other => other?,
                };
                let appending = resuming
                    && head.status == 206
                    && head
                        .headers
                        .content_range()
                        .is_some_and(|(a, _, _)| a == have);
                let mut file = if appending {
                    std::fs::OpenOptions::new().append(true).open(&part)?
                } else {
                    std::fs::File::create(&part)?
                };
                let resumable = head.headers.validator().is_some();
                match head.headers.validator() {
                    Some(v) => std::fs::write(&meta, format!("{}\n{v}", req.url))?,
                    None => {
                        let _ = std::fs::remove_file(&meta);
                    }
                }
                let start = if appending { have } else { 0 };
                let total = if head.status == 206 && req.range.is_none() {
                    head.headers.content_range().and_then(|c| c.2)
                } else {
                    head.headers.content_length()
                };
                let mut p = Progress {
                    received: start,
                    total,
                };
                track.head(total, start);
                on_progress(&p);
                let mut io_err = None;
                read_all(&mut body, cancel, &track, u64::MAX, |c| {
                    if let Err(e) = file.write_all(c) {
                        io_err = Some(e);
                        return ControlFlow::Break(());
                    }
                    p.received += c.len() as u64;
                    // Without a validator a failure from here on cannot resume.
                    delivered |= !resumable;
                    on_progress(&p);
                    ControlFlow::Continue(())
                })?;
                if let Some(e) = io_err {
                    return Err(Error::Io(e));
                }
                if let Some(t) = total
                    && p.received < t
                {
                    return Err(Error::Protocol(format!(
                        "body ended early: {} of {t} bytes",
                        p.received
                    )));
                }
                file.flush()?;
                drop(file);
                std::fs::rename(&part, path)?;
                let _ = std::fs::remove_file(&meta);
                Ok(head)
            })();
            (r, delivered)
        });
        track.end(r)
    }
}

/// A request's report to the download-activity registry (nothing for an untracked client).
pub(crate) struct Track(Option<RefCell<net::Request>>);

impl Track {
    /// A response head arrived (again, for a retried attempt).
    pub(crate) fn head(&self, len: Option<u64>, prefix: u64) {
        if let Some(t) = &self.0 {
            t.borrow_mut().head(len, prefix);
        }
    }

    #[inline]
    pub(crate) fn add(&self, n: u64) {
        if let Some(t) = &self.0 {
            t.borrow_mut().add(n);
        }
    }

    /// Stopped from the "Downloads" panel.
    #[inline]
    pub(crate) fn cancelled(&self) -> bool {
        self.0.as_ref().is_some_and(|t| t.borrow().is_cancelled())
    }

    /// Ends the report with the request's result; a request stopped from the panel ends
    /// as cancelled.
    pub(crate) fn end<T>(self, r: Result<T, Error>) -> Result<T, Error> {
        let r = match r {
            Err(_) if self.cancelled() => Err(Error::Cancelled),
            r => r,
        };
        if let Some(t) = self.0 {
            t.into_inner()
                .finish(r.as_ref().map(|_| ()).map_err(failure));
        }
        r
    }

    /// Ends it with this outcome (a body stream that stopped).
    pub(crate) fn finish(&mut self, r: Result<(), Failure>) {
        if let Some(t) = self.0.take() {
            t.into_inner().finish(r);
        }
    }
}

/// How the registry words an error (no message: those may carry a URL).
pub(crate) fn failure(e: &Error) -> Failure {
    match e {
        Error::Cancelled => Failure::Cancelled,
        Error::Status(401 | 403) => Failure::Auth,
        Error::Status(s) => Failure::Status(*s),
        Error::Timeout => Failure::Timeout,
        Error::Connect(_) | Error::NetworkOff => Failure::Offline,
        Error::Tls(_) | Error::Io(_) | Error::Protocol(_) => Failure::Other,
    }
}

thread_local! {
    static RETRY_AFTER: std::cell::Cell<Option<Duration>> = const { std::cell::Cell::new(None) };
}

/// `name.ext` → `name.ext.part`.
pub fn part_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

fn meta_path(part: &Path) -> PathBuf {
    let mut s = part.as_os_str().to_owned();
    s.push(".meta");
    PathBuf::from(s)
}

fn read_all(
    body: &mut impl Read,
    cancel: &CancelToken,
    track: &Track,
    limit: u64,
    mut f: impl FnMut(&[u8]) -> ControlFlow<()>,
) -> Result<(), Error> {
    let mut buf = vec![0u8; CHUNK];
    let mut seen = 0u64;
    loop {
        if cancel.is_cancelled() || track.cancelled() {
            return Err(Error::Cancelled);
        }
        let n = match body.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                // ureq wraps its own errors in io::Error; unwrap timeouts.
                return Err(match e.kind() {
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => Error::Timeout,
                    _ => match e.downcast::<ureq::Error>() {
                        Ok(u) => map_ureq(u),
                        Err(e) => Error::Io(e),
                    },
                });
            }
        };
        seen += n as u64;
        track.add(n as u64);
        if seen > limit {
            return Err(Error::Protocol(format!(
                "response larger than {limit} bytes"
            )));
        }
        if f(&buf[..n]).is_break() {
            return Ok(());
        }
    }
}

/// Sleeps up to `d`, waking early (with `Cancelled`) if `cancel` is set.
pub fn sleep_cancellable(d: Duration, cancel: &CancelToken) -> Result<(), Error> {
    sleep_until(d, &|| cancel.is_cancelled())
}

/// Sleeps up to `d`, waking early (with `Cancelled`) once `stop` says so.
pub(crate) fn sleep_until(d: Duration, stop: &dyn Fn() -> bool) -> Result<(), Error> {
    let end = Instant::now() + d;
    loop {
        if stop() {
            return Err(Error::Cancelled);
        }
        let now = Instant::now();
        if now >= end {
            return Ok(());
        }
        std::thread::sleep((end - now).min(Duration::from_millis(20)));
    }
}

/// The URL without query string or user info, for logs.
pub fn redact_url(url: &str) -> String {
    let no_query = url.split(['?', '#']).next().unwrap_or("");
    match no_query.split_once("://") {
        Some((scheme, rest)) => {
            let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
            let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
            format!("{scheme}://{host}{path}")
        }
        None => no_query.to_string(),
    }
}
