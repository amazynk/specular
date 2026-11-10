//! A keyed store (a Zarr hierarchy) on an HTTP(S) server or in an S3 bucket, read lazily:
//! every value is fetched when a reader asks for it, by range where it asks for part of one
//! (a shard's index, a chunk inside a shard), and kept in a bounded in-memory LRU, and
//! chunks also in the disk cache when one is given.
//!
//! [`KeyStore::prefetch`] fetches what a reader is about to need with a small pool of
//! workers (at most [`StoreOptions::concurrency`] requests at once), stopping when the
//! reader's cancel flag is set (the view moved on).
//!
//! Absent keys: HTTP 404 (and 410); S3 answers 403 for a missing key to anonymous readers
//! without list rights, so a 403 counts as absent there (and for metadata documents that
//! are probed: `.zattrs` or `zarr.json`), and the store remembers it was refused
//! ([`RemoteStore::was_denied`]) so "nothing here" can be told apart from "not allowed".
//! An S3 bucket is read unsigned first; after a refusal, with configured credentials, the
//! store signs (SigV4) from then on.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use specular_core::CancelToken;
use specular_core::keys::{KeyError, KeyStore, Part, Want};
use specular_core::net::What;

use crate::cache::DiskCache;
use crate::http::{Client, Error, Request, Response};
use crate::lru::MemLru;
use crate::s3::{self, Credentials, Env, S3Location};

/// The largest value (or answer) read.
const MAX_VALUE: u64 = 1 << 30;

/// Settings of a store.
#[derive(Clone)]
pub struct StoreOptions {
    /// Requests in flight at most during a prefetch.
    pub concurrency: usize,
    /// Bytes of fetched values kept in memory.
    pub memory: u64,
    /// Chunks are kept here too (never metadata, which can change).
    pub disk: Option<Arc<DiskCache>>,
}

impl Default for StoreOptions {
    fn default() -> Self {
        StoreOptions {
            concurrency: 8,
            memory: 256 << 20,
            disk: None,
        }
    }
}

enum Root {
    /// `base` ends in '/'; `query` (a pre-signed URL's) is added to every request.
    Http { base: String, query: String },
    S3 {
        loc: S3Location,
        creds: Option<Credentials>,
    },
}

/// Fetched values (and parts of values) by key, bounded by bytes.
type Values = MemLru<(String, Part), Arc<Vec<u8>>>;

/// A Zarr hierarchy on a server.
pub struct RemoteStore(Arc<Inner>);

struct Inner {
    client: Client,
    root: Root,
    /// S3: requests are signed (after an anonymous one was refused).
    signing: AtomicBool,
    denied: AtomicBool,
    opts: StoreOptions,
    mem: Mutex<Values>,
    missing: Mutex<HashSet<String>>,
    sizes: Mutex<HashMap<String, u64>>,
    requests: AtomicU64,
}

/// Metadata documents: probed (absent is normal), labelled as metadata, never disk-cached.
fn is_metadata(key: &str) -> bool {
    let name = key.rsplit('/').next().unwrap_or(key);
    matches!(
        name,
        ".zattrs" | ".zgroup" | ".zarray" | ".zmetadata" | "zarr.json"
    ) || name.ends_with(".xml")
        || name.ends_with(".json")
}

/// A URL without its query, fragment and user name or password.
pub fn display_url(url: &str) -> String {
    let u = url.split(['?', '#']).next().unwrap_or("");
    match u.split_once("://") {
        Some((scheme, rest)) => {
            let (auth, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
            let host = auth.rsplit('@').next().unwrap_or(auth);
            if path.is_empty() {
                format!("{scheme}://{host}")
            } else {
                format!("{scheme}://{host}/{path}")
            }
        }
        None => u.to_owned(),
    }
}

fn map_err(e: Error) -> KeyError {
    match e {
        Error::Cancelled => KeyError::Cancelled,
        Error::Status(401 | 403) => KeyError::Denied,
        Error::Status(s) => KeyError::Status(s),
        Error::Timeout => KeyError::Timeout,
        Error::Connect(m) => KeyError::Offline(m),
        Error::Tls(m) => KeyError::Failed(format!("secure connection failed: {m}")),
        Error::Io(e) => KeyError::Failed(e.to_string()),
        Error::Protocol(m) => KeyError::Failed(m),
        Error::NetworkOff => KeyError::NetworkOff,
    }
}

/// What a request brought back.
struct Fetched {
    body: Vec<u8>,
    /// The whole value's size, when the answer said.
    total: Option<u64>,
}

impl RemoteStore {
    /// A store at `url`: an S3 address (`s3://`, or an AWS S3 host) through `env` and
    /// `creds`, else a plain HTTP(S) folder. `creds` are used only after a refusal.
    pub fn open(
        client: Client,
        url: &str,
        env: &dyn Env,
        creds: Option<Credentials>,
        opts: StoreOptions,
    ) -> Result<RemoteStore, KeyError> {
        if let Some(loc) = S3Location::parse(url, env) {
            return Ok(RemoteStore::s3(client, loc, creds, opts));
        }
        let lower = url.trim().to_ascii_lowercase();
        if !(lower.starts_with("https://") || lower.starts_with("http://")) {
            return Err(KeyError::Failed("not an http, https or s3 address".into()));
        }
        let url = url.trim().split('#').next().unwrap_or("");
        let (base, query) = url.split_once('?').unwrap_or((url, ""));
        let mut base = base.to_owned();
        if !base.ends_with('/') {
            base.push('/');
        }
        let root = Root::Http {
            base,
            query: query.to_owned(),
        };
        Ok(RemoteStore::with_root(client, root, opts))
    }

    /// A store in a bucket, at `loc`.
    pub fn s3(
        client: Client,
        loc: S3Location,
        creds: Option<Credentials>,
        opts: StoreOptions,
    ) -> RemoteStore {
        RemoteStore::with_root(client, Root::S3 { loc, creds }, opts)
    }

    fn with_root(client: Client, root: Root, opts: StoreOptions) -> RemoteStore {
        RemoteStore(Arc::new(Inner {
            client,
            root,
            signing: AtomicBool::new(false),
            denied: AtomicBool::new(false),
            mem: Mutex::new(MemLru::new(opts.memory)),
            opts,
            missing: Mutex::new(HashSet::new()),
            sizes: Mutex::new(HashMap::new()),
            requests: AtomicU64::new(0),
        }))
    }

    /// Requests sent so far.
    pub fn requests(&self) -> u64 {
        self.0.requests.load(Ordering::Relaxed)
    }

    /// Whether a request was refused (401/403) and counted as absent.
    pub fn was_denied(&self) -> bool {
        self.0.denied.load(Ordering::Relaxed)
    }

    /// Whether requests are signed now.
    pub fn is_signing(&self) -> bool {
        self.0.is_signing()
    }
}

impl Inner {
    fn is_signing(&self) -> bool {
        self.signing.load(Ordering::Relaxed)
    }

    /// The URL of `key`, without the query a pre-signed URL carries (for cache keys).
    fn key_url(&self, key: &str) -> String {
        match &self.root {
            Root::Http { base, .. } => format!("{base}{}", s3::uri_encode(key, false)),
            Root::S3 { loc, .. } => loc.object_url(key),
        }
    }

    fn what(key: &str) -> What {
        if is_metadata(key) {
            What::ZarrMetadata
        } else {
            What::ZarrChunks
        }
    }

    /// The request for `key` (and `part`), signed when the store signs.
    fn build(&self, head: bool, key: &str, part: Part) -> Request {
        let range = match part {
            Part::Whole => None,
            Part::At(s, l) => Some(format!("bytes={s}-{}", s + l.max(1) - 1)),
            Part::Suffix(l) => Some(format!("bytes=-{}", l.max(1))),
        };
        let url = match &self.root {
            Root::Http { query, .. } if !query.is_empty() => {
                format!("{}?{query}", self.key_url(key))
            }
            _ => self.key_url(key),
        };
        let mut req = if head {
            Request::head(url)
        } else {
            Request::get(url)
        }
        .activity(Self::what(key));
        if let Some(r) = &range {
            req = req.header("range", r.clone());
        }
        // A server that ignores Range sends the whole value (then sliced): still bounded.
        req = req.limit(MAX_VALUE);
        if let Root::S3 {
            loc,
            creds: Some(c),
        } = &self.root
            && self.is_signing()
        {
            let path = loc.object_path(key);
            req = self.sign(
                req,
                loc,
                c,
                if head { "HEAD" } else { "GET" },
                &path,
                &[],
                range,
            );
        }
        req
    }

    #[allow(clippy::too_many_arguments)]
    fn sign(
        &self,
        mut req: Request,
        loc: &S3Location,
        c: &Credentials,
        method: &str,
        path: &str,
        query: &[(String, String)],
        range: Option<String>,
    ) -> Request {
        let host = loc
            .endpoint
            .split_once("://")
            .map_or(loc.endpoint.as_str(), |(_, h)| h)
            .to_owned();
        let mut headers = vec![
            ("host".to_owned(), host),
            (
                "x-amz-content-sha256".to_owned(),
                s3::EMPTY_SHA256.to_owned(),
            ),
        ];
        if let Some(r) = range {
            headers.push(("range".into(), r));
        }
        let date = s3::amz_date(std::time::SystemTime::now());
        let signed = s3::sign(
            &s3::ToSign {
                method,
                path,
                query,
                headers: &headers,
                payload_hash: s3::EMPTY_SHA256,
                region: &loc.region,
                service: "s3",
                amz_date: &date,
            },
            c,
        );
        req = req.header("x-amz-content-sha256", s3::EMPTY_SHA256);
        for (k, v) in signed {
            req = req.header(k, v);
        }
        req
    }

    /// Sends a request; `Ok(None)` when the key is absent. Switches an S3 store to signed
    /// requests after a refusal when it has credentials.
    fn send(
        &self,
        head: bool,
        key: &str,
        part: Part,
        cancel: &CancelToken,
    ) -> Result<Option<Response>, KeyError> {
        loop {
            let was_signing = self.is_signing();
            let req = self.build(head, key, part);
            self.requests.fetch_add(1, Ordering::Relaxed);
            let r = if head {
                self.client.head(&req, cancel).map(|h| Response {
                    status: h.status,
                    headers: h.headers,
                    body: Vec::new(),
                })
            } else {
                self.client.get(&req, cancel)
            };
            return match r {
                Ok(r) => Ok(Some(r)),
                Err(Error::Status(404 | 410)) => Ok(None),
                Err(Error::Status(401 | 403)) => {
                    let s3 = matches!(&self.root, Root::S3 { .. });
                    if let Root::S3 { creds: Some(_), .. } = &self.root
                        && !was_signing
                    {
                        self.signing.store(true, Ordering::Relaxed);
                        continue;
                    }
                    if s3 || is_metadata(key) {
                        self.denied.store(true, Ordering::Relaxed);
                        Ok(None)
                    } else {
                        Err(KeyError::Denied)
                    }
                }
                Err(e) => Err(map_err(e)),
            };
        }
    }

    /// Fetches `part` of `key` from the server (no caches).
    fn fetch(
        &self,
        key: &str,
        part: Part,
        cancel: &CancelToken,
    ) -> Result<Option<Fetched>, KeyError> {
        let Some(r) = self.send(false, key, part, cancel)? else {
            return Ok(None);
        };
        let body = r.body;
        let n = body.len() as u64;
        let ranged = r.status == 206;
        let total = if ranged {
            r.headers.content_range().and_then(|c| c.2)
        } else {
            Some(n)
        };
        let body = match (part, ranged) {
            (Part::Whole, _) | (_, true) => body,
            // The server sent the whole value: cut out what was asked.
            (Part::At(s, l), false) => {
                let s = s.min(n) as usize;
                let e = s.saturating_add(l as usize).min(n as usize);
                body[s..e].to_vec()
            }
            (Part::Suffix(l), false) => body[(n.saturating_sub(l)) as usize..].to_vec(),
        };
        Ok(Some(Fetched { body, total }))
    }

    fn disk_key(&self, key: &str, part: Part) -> Option<String> {
        (self.opts.disk.is_some() && !is_metadata(key))
            .then(|| DiskCache::url_key(&format!("{}#{part:?}", self.key_url(key))))
    }

    /// From memory or the disk cache, without a request.
    fn cached(&self, key: &str, part: Part) -> Option<Arc<Vec<u8>>> {
        let k = (key.to_owned(), part);
        if let Some(v) = self.mem.lock().get(&k) {
            return Some(v);
        }
        if let Part::At(s, l) = part
            && let Some(w) = self.mem.lock().get(&(key.to_owned(), Part::Whole))
        {
            let s = (s as usize).min(w.len());
            let e = s.saturating_add(l as usize).min(w.len());
            return Some(Arc::new(w[s..e].to_vec()));
        }
        let rel = self.disk_key(key, part)?;
        let disk = self.opts.disk.as_ref()?;
        let p = disk.get(&rel)?;
        let bytes = std::fs::read(p).ok()?;
        // A suffix's value size is stored with it (8 bytes in front).
        let v = if let Part::Suffix(_) = part {
            let (size, rest) = bytes.split_at_checked(8)?;
            let size = u64::from_le_bytes(size.try_into().ok()?);
            self.sizes.lock().insert(key.to_owned(), size);
            rest.to_vec()
        } else {
            bytes
        };
        let v = Arc::new(v);
        self.remember(key, part, v.clone());
        Some(v)
    }

    fn remember(&self, key: &str, part: Part, v: Arc<Vec<u8>>) {
        let w = v.len() as u64 + 64;
        self.mem.lock().insert((key.to_owned(), part), v, w);
    }

    fn is_missing(&self, key: &str) -> bool {
        self.missing.lock().contains(key)
    }

    /// `part` of `key`: from the caches, else fetched (and kept).
    fn load(
        &self,
        key: &str,
        part: Part,
        cancel: &CancelToken,
    ) -> Result<Option<Arc<Vec<u8>>>, KeyError> {
        if self.is_missing(key) {
            return Ok(None);
        }
        if let Some(v) = self.cached(key, part) {
            return Ok(Some(v));
        }
        let Some(f) = self.fetch(key, part, cancel)? else {
            let mut m = self.missing.lock();
            if m.len() > 1 << 16 {
                m.clear();
            }
            m.insert(key.to_owned());
            return Ok(None);
        };
        if let Some(t) = f.total {
            self.sizes.lock().insert(key.to_owned(), t);
        }
        if let (Some(rel), Some(disk)) = (self.disk_key(key, part), &self.opts.disk) {
            let stored = match (part, f.total) {
                (Part::Suffix(_), Some(t)) => {
                    let mut b = t.to_le_bytes().to_vec();
                    b.extend_from_slice(&f.body);
                    Some(b)
                }
                (Part::Suffix(_), None) => None,
                _ => Some(f.body.clone()),
            };
            if let Some(b) = stored {
                let _ = disk.insert_bytes(&rel, &b);
            }
        }
        let v = Arc::new(f.body);
        self.remember(key, part, v.clone());
        Ok(Some(v))
    }

    fn s3_list(
        &self,
        loc: &S3Location,
        creds: Option<&Credentials>,
        dir: &str,
        cancel: &CancelToken,
    ) -> Result<Vec<String>, KeyError> {
        let prefix = format!("{}{dir}", loc.prefix);
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        for _page in 0..64 {
            let mut query = vec![
                ("list-type".to_owned(), "2".to_owned()),
                ("prefix".to_owned(), prefix.clone()),
                ("delimiter".to_owned(), "/".to_owned()),
            ];
            if let Some(t) = &token {
                query.push(("continuation-token".into(), t.clone()));
            }
            let path = loc.bucket_path();
            let url = format!("{}{path}?{}", loc.endpoint, s3::canonical_query(&query));
            let mut req = Request::get(url).activity(What::ZarrMetadata);
            if let (Some(c), true) = (creds, self.is_signing()) {
                req = self.sign(req, loc, c, "GET", &path, &query, None);
            }
            self.requests.fetch_add(1, Ordering::Relaxed);
            let r = match self.client.get(&req, cancel) {
                Ok(r) => r,
                Err(Error::Status(401 | 403)) if creds.is_some() && !self.is_signing() => {
                    self.signing.store(true, Ordering::Relaxed);
                    continue;
                }
                Err(e) => return Err(map_err(e)),
            };
            let text = String::from_utf8_lossy(&r.body);
            let (names, next) = s3::parse_list(&text);
            out.extend(names.into_iter().filter_map(|p| {
                p.strip_prefix(&prefix)
                    .map(|n| n.trim_end_matches('/').to_owned())
                    .filter(|n| !n.is_empty())
            }));
            match next {
                Some(t) => token = Some(t),
                None => break,
            }
        }
        Ok(out)
    }
}

/// Runs `f` on a worker with a token that is cancelled when `flag` is set; returns as
/// soon as the flag is set (a request waiting for its answer ends in the background, within
/// its timeouts, and keeps what it fetched).
fn with_cancel<T: Send + 'static>(
    inner: &Arc<Inner>,
    flag: Option<&AtomicBool>,
    f: impl FnOnce(&Inner, &CancelToken) -> Result<T, KeyError> + Send + 'static,
) -> Result<T, KeyError> {
    let tok = CancelToken::new();
    let Some(flag) = flag else {
        return f(inner, &tok);
    };
    if flag.load(Ordering::Relaxed) {
        return Err(KeyError::Cancelled);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let (me, t) = (inner.clone(), tok.clone());
    specular_thread::spawn("zarr-fetch", move || {
        let _ = tx.send(f(&me, &t));
    });
    loop {
        match rx.recv_timeout(Duration::from_millis(5)) {
            Ok(r) => return r,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(KeyError::Failed("fetch worker failed".into()));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if flag.load(Ordering::Relaxed) {
                    tok.cancel();
                    return Err(KeyError::Cancelled);
                }
            }
        }
    }
}

/// Fetches `todo` with `workers` threads, stopping at the first failure or when `tok` is
/// cancelled.
fn fetch_all(inner: &Arc<Inner>, todo: Vec<Want>, tok: &CancelToken) -> Result<(), KeyError> {
    let workers = inner.opts.concurrency.clamp(1, 32).min(todo.len());
    let todo = Arc::new(todo);
    let next = Arc::new(AtomicUsize::new(0));
    let failed: Arc<Mutex<Option<KeyError>>> = Arc::new(Mutex::new(None));
    let handles: Vec<_> = (0..workers)
        .map(|i| {
            let (me, todo, next, failed, tok) = (
                inner.clone(),
                todo.clone(),
                next.clone(),
                failed.clone(),
                tok.clone(),
            );
            specular_thread::spawn(format!("zarr-fetch-{i}"), move || {
                while !tok.is_cancelled() {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(w) = todo.get(k) else { break };
                    match me.load(&w.key, w.part, &tok) {
                        Ok(_) => {}
                        Err(KeyError::Cancelled) => break,
                        Err(e) => {
                            failed.lock().get_or_insert(e);
                            tok.cancel();
                            break;
                        }
                    }
                }
            })
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
    match failed.lock().take() {
        Some(e) => Err(e),
        None if tok.is_cancelled() => Err(KeyError::Cancelled),
        None => Ok(()),
    }
}

impl KeyStore for RemoteStore {
    fn get(&self, key: &str, cancel: Option<&AtomicBool>) -> Result<Option<Vec<u8>>, KeyError> {
        if let Some(v) = self.0.cached(key, Part::Whole) {
            return Ok(Some((*v).clone()));
        }
        let key = key.to_owned();
        with_cancel(&self.0, cancel, move |s, t| s.load(&key, Part::Whole, t))
            .map(|v| v.map(|v| (*v).clone()))
    }

    fn get_range(
        &self,
        key: &str,
        start: u64,
        len: u64,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Vec<u8>>, KeyError> {
        let part = Part::At(start, len);
        if let Some(v) = self.0.cached(key, part) {
            return Ok(Some((*v).clone()));
        }
        let key = key.to_owned();
        with_cancel(&self.0, cancel, move |s, t| s.load(&key, part, t))
            .map(|v| v.map(|v| (*v).clone()))
    }

    fn get_suffix(
        &self,
        key: &str,
        len: u64,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<(Vec<u8>, u64)>, KeyError> {
        let part = Part::Suffix(len);
        let v = match self.0.cached(key, part) {
            Some(v) => Some(v),
            None => {
                let k = key.to_owned();
                with_cancel(&self.0, cancel, move |s, t| s.load(&k, part, t))?
            }
        };
        let Some(v) = v else { return Ok(None) };
        let size = self.0.sizes.lock().get(key).copied();
        Ok(Some(((*v).clone(), size.unwrap_or(v.len() as u64))))
    }

    fn size(&self, key: &str, cancel: Option<&AtomicBool>) -> Result<Option<u64>, KeyError> {
        if let Some(s) = self.0.sizes.lock().get(key) {
            return Ok(Some(*s));
        }
        if self.0.is_missing(key) {
            return Ok(None);
        }
        let key = key.to_owned();
        with_cancel(&self.0, cancel, move |s, t| {
            let Some(h) = s.send(true, &key, Part::Whole, t)? else {
                return Ok(None);
            };
            let n = h.headers.content_length();
            if let Some(n) = n {
                s.sizes.lock().insert(key, n);
            }
            Ok(n)
        })
    }

    fn prefetch(&self, wants: &[Want], cancel: Option<&AtomicBool>) -> Result<(), KeyError> {
        let mut seen = HashSet::new();
        let todo: Vec<Want> = wants
            .iter()
            .filter(|w| !self.0.is_missing(&w.key) && self.0.cached(&w.key, w.part).is_none())
            .filter(|w| seen.insert((*w).clone()))
            .cloned()
            .collect();
        if todo.is_empty() {
            return Ok(());
        }
        let me = self.0.clone();
        with_cancel(&self.0, cancel, move |_, tok| fetch_all(&me, todo, tok))
    }

    fn subdirs(&self, dir: &str, cancel: Option<&AtomicBool>) -> Result<Vec<String>, KeyError> {
        if let Root::Http { .. } = &self.0.root {
            return Ok(Vec::new());
        }
        let dir = dir.to_owned();
        with_cancel(&self.0, cancel, move |s, t| match &s.root {
            Root::S3 { loc, creds } => s.s3_list(loc, creds.as_ref(), &dir, t),
            Root::Http { .. } => Ok(Vec::new()),
        })
    }

    fn refused(&self) -> bool {
        self.was_denied()
    }

    fn location(&self) -> String {
        match &self.0.root {
            Root::Http { base, .. } => display_url(base),
            Root::S3 { loc, .. } => loc.display(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_urls_carry_no_secrets() {
        assert_eq!(
            display_url("https://u:pw@h.example:8443/a/b.zarr/?X-Amz-Signature=1#f"),
            "https://h.example:8443/a/b.zarr/"
        );
        assert_eq!(display_url("http://h"), "http://h");
    }

    #[test]
    fn metadata_keys() {
        assert!(is_metadata(".zattrs"));
        assert!(is_metadata("0/zarr.json"));
        assert!(is_metadata("OME/METADATA.ome.xml"));
        assert!(!is_metadata("0/0.0.0"));
        assert!(!is_metadata("0/c/0/0"));
    }
}
