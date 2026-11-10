//! The connection: one request at a time, each answered in order.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use std::os::windows::net::UnixStream;

use serde_json::Value;

use crate::protocol::{self, ArraySource, MAX_CHUNK, MAX_LINE, Request, ViewSet};

/// A failed request: the protocol's error code (`docs/ipc.md`) and its message. Local
/// failures use the codes `io` (the connection) and `parse` (a malformed response).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: String,
    pub message: String,
}

impl Error {
    pub fn local(code: &str, e: impl std::fmt::Display) -> Error {
        Error {
            code: code.into(),
            message: e.to_string(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for Error {}

/// The first viewer's socket: `$SPECULAR_SOCKET`, else the usual places (`docs/ipc.md`,
/// "Transport"). `None` when no socket file exists there.
pub fn default_socket() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SPECULAR_SOCKET").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let name = "specular.sock";
    let mut dirs: Vec<PathBuf> = Vec::new();
    let env = |k: &str| {
        std::env::var_os(k)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    #[cfg(unix)]
    {
        if let Some(d) = env("XDG_RUNTIME_DIR") {
            dirs.push(d.join("specular"));
        }
        let uid = uid();
        #[cfg(target_os = "macos")]
        if let Some(d) = env("TMPDIR") {
            dirs.push(d.join(format!("specular-{uid}")));
        }
        #[cfg(target_os = "macos")]
        if let Some(h) = env("HOME") {
            dirs.push(h.join("Library/Caches/Specular/run"));
        }
        #[cfg(not(target_os = "macos"))]
        if let Some(c) = env("XDG_CACHE_HOME").or_else(|| env("HOME").map(|h| h.join(".cache"))) {
            dirs.push(c.join("specular/run"));
        }
        dirs.push(std::env::temp_dir().join(format!("specular-{uid}")));
        dirs.push(PathBuf::from(format!("/tmp/specular-{uid}")));
    }
    #[cfg(windows)]
    if let Some(d) = env("LOCALAPPDATA") {
        dirs.push(d.join("Specular").join("cache").join("run"));
        dirs.push(d.join("Specular").join("run"));
    }
    #[cfg(not(any(unix, windows)))]
    let _ = env;
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.exists())
}

/// The real user id, read from `/proc` or the socket folder's owner (no `unsafe`).
#[cfg(unix)]
fn uid() -> u32 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").map_or_else(
        |_| {
            std::env::var_os("HOME")
                .and_then(|h| std::fs::metadata(h).ok())
                .map_or(0, |m| m.uid())
        },
        |m| m.uid(),
    )
}

/// A connection to a viewer.
pub struct Client {
    r: BufReader<UnixStream>,
    w: UnixStream,
    next_id: u64,
    version: u64,
    timeout: Duration,
    /// Events that arrived while waiting for an answer, oldest first.
    events: VecDeque<Value>,
}

/// How long a subscriber waits for the next line: the viewer sends `ping` every minute, so
/// silence this long means it is gone.
const EVENT_SILENCE: Duration = Duration::from_secs(150);

impl Client {
    /// Connects to the socket at `path`; every read and write gives up after `timeout`.
    ///
    /// Where the viewer keeps a key next to its socket (Windows), or when this program was
    /// started by a hook (`$SPECULAR_HOOK`), a `hello` carrying them goes first.
    pub fn connect_timeout(path: &Path, timeout: Duration) -> Result<Client, Error> {
        let s = UnixStream::connect(path).map_err(|e| Error::local("io", e))?;
        s.set_read_timeout(Some(timeout))
            .and_then(|()| s.set_write_timeout(Some(timeout)))
            .map_err(|e| Error::local("io", e))?;
        let mut c = Client {
            w: s.try_clone().map_err(|e| Error::local("io", e))?,
            r: BufReader::new(s),
            next_id: 1,
            version: protocol::VERSION,
            timeout,
            events: VecDeque::new(),
        };
        let (key, hook) = protocol::introduction(path);
        if key.is_some() || hook.is_some() {
            let id = c.next_id;
            c.next_id += 1;
            let line = protocol::hello_line(1, id, key.as_deref(), hook.as_deref());
            c.answer(line.as_bytes(), id)?;
        }
        Ok(c)
    }

    /// Connects to the socket at `path` (30 s timeouts).
    pub fn connect(path: &Path) -> Result<Client, Error> {
        Client::connect_timeout(path, Duration::from_secs(30))
    }

    /// Connects to the first viewer ([`default_socket`]).
    pub fn connect_default() -> Result<Client, Error> {
        let p = default_socket().ok_or_else(|| Error::local("io", "no running viewer"))?;
        Client::connect(&p)
    }

    /// Speaks protocol `v` from now on (1 for an old viewer).
    pub fn set_version(&mut self, v: u64) {
        self.version = v;
    }

    /// Sends one line and returns the response line (event lines met on the way are kept
    /// for [`Client::next_event`]).
    pub fn exchange(&mut self, line: &[u8]) -> Result<Vec<u8>, Error> {
        let mut v = Vec::with_capacity(line.len() + 1);
        v.extend_from_slice(line);
        v.push(b'\n');
        self.w
            .write_all(&v)
            .and_then(|()| self.w.flush())
            .map_err(|e| Error::local("io", e))?;
        loop {
            let l = self.read_line()?;
            match serde_json::from_slice::<Value>(&l) {
                Ok(v) if protocol::is_event(&v) => self.keep(v),
                _ => return Ok(l),
            }
        }
    }

    /// Keeps an event for later, the oldest going first when too many wait.
    fn keep(&mut self, v: Value) {
        if self.events.len() >= 4096 {
            self.events.pop_front();
        }
        self.events.push_back(v);
    }

    /// Sends `line` (a request with `id`) and returns its `result`.
    fn answer(&mut self, line: &[u8], id: u64) -> Result<Value, Error> {
        let line = self.exchange(line)?;
        let v: Value = serde_json::from_slice(&line).map_err(|e| Error::local("parse", e))?;
        // A `busy` notice arrives unasked, with no id.
        if v["ok"] == Value::Bool(true) && v["id"] == id {
            return Ok(v["result"].clone());
        }
        Err(Error {
            code: v["error"]["code"].as_str().unwrap_or("parse").to_owned(),
            message: v["error"]["message"]
                .as_str()
                .unwrap_or("malformed response")
                .to_owned(),
        })
    }

    /// One line from the viewer.
    fn read_line(&mut self) -> Result<Vec<u8>, Error> {
        let mut buf = Vec::new();
        let n = (&mut self.r)
            .take(MAX_LINE as u64 + 2)
            .read_until(b'\n', &mut buf)
            .map_err(|e| Error::local("io", e))?;
        if n == 0 {
            return Err(Error::local("io", "the viewer closed the connection"));
        }
        if buf.last() != Some(&b'\n') {
            return Err(Error::local("too-long", "response too long"));
        }
        buf.pop();
        if buf.last() == Some(&b'\r') {
            buf.pop();
        }
        Ok(buf)
    }

    /// Sends `req` and returns its `result`.
    pub fn call(&mut self, req: &Request) -> Result<Value, Error> {
        let id = self.next_id;
        self.next_id += 1;
        let line = protocol::request_line_v(self.version, id, req);
        self.answer(line.as_bytes(), id)
    }

    /// Subscribes to `events` (names from [`protocol::EVENTS`], or `"*"` for all) and returns
    /// them as they come, `ping`s left out. The connection still takes requests in between
    /// ([`Events::client`]).
    ///
    /// ```no_run
    /// let mut c = specular_client::Client::connect_default()?;
    /// for e in c.subscribe(&["marked"])? {
    ///     let e = e?;
    ///     println!("{} {}", e["event"], e["files"]);
    /// }
    /// # Ok::<(), specular_client::Error>(())
    /// ```
    pub fn subscribe(&mut self, events: &[&str]) -> Result<Events<'_>, Error> {
        let r = self.call(&Request::Subscribe {
            events: events.iter().map(|e| (*e).to_owned()).collect(),
        })?;
        self.r
            .get_ref()
            .set_read_timeout(Some(EVENT_SILENCE))
            .map_err(|e| Error::local("io", e))?;
        Ok(Events {
            seq: r["seq"].as_u64().unwrap_or(0),
            client: self,
        })
    }

    /// The next event: one that came in while waiting for an answer, else the next line
    /// (`None`: the viewer closed the connection).
    pub fn next_event(&mut self) -> Option<Result<Value, Error>> {
        if let Some(v) = self.events.pop_front() {
            return Some(Ok(v));
        }
        let l = match self.read_line() {
            Ok(l) => l,
            Err(e) if e.message.contains("closed the connection") => return None,
            Err(e) => return Some(Err(e)),
        };
        Some(serde_json::from_slice::<Value>(&l).map_err(|e| Error::local("parse", e)))
    }

    /// `hello`: the viewer, its protocol versions and capabilities.
    pub fn hello(&mut self) -> Result<Value, Error> {
        self.call(&Request::Hello)
    }

    /// `view-set`, returning the view as `view-get` would.
    pub fn view_set(&mut self, set: ViewSet) -> Result<Value, Error> {
        self.call(&Request::ViewSet(set))
    }

    /// Uploads `data` into a new blob, in chunks; returns its id.
    pub fn upload(&mut self, data: &[u8]) -> Result<u64, Error> {
        let r = self.call(&Request::BlobNew {
            bytes: data.len() as u64,
        })?;
        let blob = r["blob"]
            .as_u64()
            .ok_or_else(|| Error::local("parse", "blob-new without a blob id"))?;
        for (i, chunk) in data.chunks(MAX_CHUNK).enumerate() {
            let w = self.call(&Request::BlobWrite {
                blob,
                offset: (i * MAX_CHUNK) as u64,
                data: chunk.to_vec(),
            });
            if let Err(e) = w {
                let _ = self.call(&Request::BlobFree { blob });
                return Err(e);
            }
        }
        Ok(blob)
    }

    /// Reads all `bytes` of a blob, in chunks, and frees it.
    pub fn download(&mut self, blob: u64, bytes: u64) -> Result<Vec<u8>, Error> {
        let mut out = Vec::with_capacity(usize::try_from(bytes).unwrap_or(0));
        while (out.len() as u64) < bytes {
            let r = self.call(&Request::BlobRead {
                blob,
                offset: out.len() as u64,
                length: (bytes - out.len() as u64).min(MAX_CHUNK as u64),
            })?;
            let d = r["data"]
                .as_str()
                .and_then(crate::base64::decode)
                .ok_or_else(|| Error::local("parse", "blob-read without base64 data"))?;
            if d.is_empty() {
                return Err(Error::local("parse", "blob shorter than announced"));
            }
            out.extend_from_slice(&d);
        }
        self.call(&Request::BlobFree { blob })?;
        Ok(out)
    }

    /// Shows an array in the viewer: `data` holds its elements as `dtype` (a NumPy type
    /// string), C order unless `strides` (bytes) say otherwise.
    pub fn show_array(
        &mut self,
        data: &[u8],
        shape: &[u64],
        dtype: &str,
        strides: Option<&[i64]>,
        name: Option<&str>,
    ) -> Result<Value, Error> {
        let blob = self.upload(data)?;
        let r = self.call(&Request::ArrayShow {
            source: ArraySource::Blob(blob),
            shape: shape.to_vec(),
            dtype: dtype.to_owned(),
            strides: strides.map(<[i64]>::to_vec),
            name: name.map(str::to_owned),
        });
        // The viewer frees the blob once shown; on failure it is freed here.
        if r.is_err() {
            let _ = self.call(&Request::BlobFree { blob });
        }
        r
    }

    /// A PNG of what the window shows.
    pub fn screenshot(&mut self) -> Result<Vec<u8>, Error> {
        let r = self.call(&Request::Screenshot)?;
        let (Some(blob), Some(bytes)) = (r["blob"].as_u64(), r["bytes"].as_u64()) else {
            return Err(Error::local("parse", "screenshot without a blob"));
        };
        self.download(blob, bytes)
    }
}

/// The events of a subscription ([`Client::subscribe`]), oldest first.
pub struct Events<'a> {
    client: &'a mut Client,
    /// The sequence number the subscription started at: the events have higher ones.
    pub seq: u64,
}

impl Events<'_> {
    /// The connection, for requests in between events.
    pub fn client(&mut self) -> &mut Client {
        self.client
    }

    /// Stops the subscription: later events are not sent.
    pub fn unsubscribe(self) -> Result<(), Error> {
        let t = self.client.timeout;
        self.client.call(&Request::Unsubscribe { events: None })?;
        self.client
            .r
            .get_ref()
            .set_read_timeout(Some(t))
            .map_err(|e| Error::local("io", e))
    }
}

impl Iterator for Events<'_> {
    type Item = Result<Value, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.client.next_event()? {
                Ok(v) if v["event"] == "ping" => continue,
                // A stray response (to nothing asked): not an event.
                Ok(v) if !protocol::is_event(&v) => continue,
                r => return Some(r),
            }
        }
    }
}
