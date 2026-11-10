//! Downloads as a [`Read`] that start at an offset and resume themselves.
//!
//! For callers that keep their own partial file (the Local AI installer hashes as it writes):
//! [`Client::open_at`] asks for the bytes from an offset with `Range`, guarded by `If-Range`
//! when the caller knows the resource's validator (ETag or Last-Modified), so a changed file
//! comes back whole instead of being spliced. The returned [`BodyStream`] reconnects by itself
//! after a dropped connection or a body timeout, again with `Range` + `If-Range`.
//! [`Resumer`] remembers validators per URL (in memory, and optionally in a folder) so a
//! download interrupted in one session resumes safely in the next.

use std::collections::HashMap;
use std::io::{self, Read};
use std::path::PathBuf;

use parking_lot::Mutex;
use specular_core::CancelToken;

use specular_core::net::Failure;

use crate::http::{Client, Error, Head, Request, Track, sleep_cancellable};

/// An open download.
pub struct Opened {
    pub head: Head,
    /// Offset of the body's first byte: the offset asked for, or 0 when the server sent the
    /// whole resource (no range support, or the validator no longer matched).
    pub start: u64,
    /// Length of the whole resource, if the server said.
    pub total: Option<u64>,
    pub body: BodyStream,
}

impl Opened {
    /// The body starts at the offset asked for.
    pub fn resumed(&self, offset: u64) -> bool {
        offset > 0 && self.start == offset
    }
    /// The resource's validator (strong ETag, else Last-Modified).
    pub fn validator(&self) -> Option<&str> {
        self.head.headers.validator()
    }
}

impl Client {
    /// `GET req` from byte `offset` (`req` itself carries no range). `validator`: what the
    /// caller saw last time; it goes in `If-Range`. Without one, the range is asked for
    /// unguarded and the caller must check the content (a hash) itself.
    ///
    /// Opening is retried like any request; the body reconnects on its own (see
    /// [`BodyStream`]).
    pub fn open_at(
        &self,
        req: &Request,
        offset: u64,
        validator: Option<&str>,
        cancel: &CancelToken,
    ) -> Result<Opened, Error> {
        let track = self.track(req);
        let r = self.with_retries(cancel, &track, || {
            let r = (|| {
                if offset == 0 {
                    let (h, b) = self.send(req)?;
                    return Ok((h, b, 0));
                }
                let mut r = req.clone().range_from(offset);
                if let Some(v) = validator {
                    r = r.header("If-Range", v);
                }
                let (h, b) = match self.send(&r) {
                    // Shorter than what the caller has: it changed; start over.
                    Err(Error::Status(416)) => self.send(req)?,
                    other => other?,
                };
                let start = match (h.status, h.headers.content_range()) {
                    (206, Some((a, _, _))) if a == offset => offset,
                    (206, _) => {
                        return Err(Error::Protocol("unexpected Content-Range".into()));
                    }
                    _ => 0,
                };
                Ok((h, b, start))
            })();
            (r, false)
        });
        let (head, body, start) = match r {
            Ok(v) => v,
            Err(e) => return track.end(Err(e)),
        };
        let total = if head.status == 206 {
            head.headers.content_range().and_then(|c| c.2)
        } else {
            head.headers.content_length()
        };
        track.head(total, start);
        let body = BodyStream {
            client: self.clone(),
            req: req.clone(),
            body: Some(body),
            pos: start,
            total,
            validator: head.headers.validator().map(str::to_string),
            cancel: cancel.clone(),
            failures: 0,
            track,
        };
        Ok(Opened {
            head,
            start,
            total,
            body,
        })
    }
}

/// A response body that reconnects after a dropped connection, a stall past the body
/// timeout, or a body that ends early, by asking for the rest with `Range` + `If-Range`.
/// That needs a validator; without one the error is returned. Gives up after the client's
/// retry count of failures in a row (any progress resets the count).
pub struct BodyStream {
    client: Client,
    req: Request,
    body: Option<ureq::BodyReader<'static>>,
    pos: u64,
    total: Option<u64>,
    validator: Option<String>,
    cancel: CancelToken,
    failures: u32,
    /// Its report to the "Downloads" indicator, ended with the body.
    track: Track,
}

impl Drop for BodyStream {
    fn drop(&mut self) {
        // Put down before its end: the reader stopped it.
        let early = self.total.is_some_and(|t| self.pos < t);
        let r = if early || self.cancel.is_cancelled() {
            Err(Failure::Cancelled)
        } else {
            Ok(())
        };
        self.track.finish(r);
    }
}

impl BodyStream {
    /// Bytes of the resource read so far, counting from 0 (including the skipped offset).
    pub fn position(&self) -> u64 {
        self.pos
    }

    fn reconnect(&mut self, why: &io::Error) -> bool {
        let Some(v) = self.validator.clone() else {
            return false;
        };
        if self.failures >= self.client.config().retries || self.cancel.is_cancelled() {
            return false;
        }
        self.failures += 1;
        self.body = None;
        let wait = self
            .client
            .config()
            .backoff
            .saturating_mul(1 << (self.failures - 1).min(16))
            .min(self.client.config().max_backoff);
        specular_log::debug!(
            target: specular_log::IO,
            "http resume at {} in {wait:?}: {why}",
            self.pos
        );
        if sleep_cancellable(wait, &self.cancel).is_err() {
            return false;
        }
        let r = self.req.clone().range_from(self.pos).header("If-Range", v);
        match self.client.send(&r) {
            Ok((h, b))
                if h.status == 206
                    && h.headers
                        .content_range()
                        .is_some_and(|(a, _, _)| a == self.pos) =>
            {
                self.body = Some(b);
                true
            }
            // Changed under us (200), or no longer there.
            _ => false,
        }
    }
}

impl Read for BodyStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.cancel.is_cancelled() || self.track.cancelled() {
                self.body = None;
                self.track.finish(Err(Failure::Cancelled));
                return Err(io::Error::other(Error::Cancelled));
            }
            let Some(body) = self.body.as_mut() else {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "download could not resume",
                ));
            };
            let err = match body.read(buf) {
                Ok(0) => match self.total {
                    Some(t) if self.pos < t => io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("body ended early: {} of {t} bytes", self.pos),
                    ),
                    _ => {
                        self.track.finish(Ok(()));
                        return Ok(0);
                    }
                },
                Ok(n) => {
                    self.pos += n as u64;
                    self.track.add(n as u64);
                    self.failures = 0;
                    return Ok(n);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => e,
            };
            if !self.reconnect(&err) {
                self.body = None;
                let f = match err.kind() {
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => Failure::Timeout,
                    _ => Failure::Other,
                };
                self.track.finish(Err(f));
                return Err(err);
            }
        }
    }
}

/// Opens downloads and remembers each URL's validator, so a later [`open`](Self::open) at an
/// offset (a partial file the caller kept) sends `If-Range`. With a folder, validators
/// survive restarts (one small file per URL, named by a hash of it; no credentials).
pub struct Resumer {
    client: Client,
    dir: Option<PathBuf>,
    known: Mutex<HashMap<String, String>>,
}

impl Resumer {
    pub fn new(client: Client, dir: Option<PathBuf>) -> Resumer {
        Resumer {
            client,
            dir,
            known: Mutex::default(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    fn file(&self, url: &str) -> Option<PathBuf> {
        let h = blake3::hash(url.as_bytes()).to_hex();
        Some(self.dir.as_ref()?.join(format!("{}.validator", &h[..32])))
    }

    /// The validator last seen for `url`.
    pub fn validator(&self, url: &str) -> Option<String> {
        if let Some(v) = self.known.lock().get(url) {
            return Some(v.clone());
        }
        let text = std::fs::read_to_string(self.file(url)?).ok()?;
        let (u, v) = text.split_once('\n')?;
        (u == url && !v.trim().is_empty()).then(|| v.trim().to_string())
    }

    fn remember(&self, url: &str, v: Option<&str>) {
        let mut known = self.known.lock();
        match v {
            Some(v) => {
                if known.get(url).is_some_and(|k| k == v) {
                    return;
                }
                known.insert(url.to_string(), v.to_string());
                if let Some(f) = self.file(url) {
                    let _ = f.parent().map(std::fs::create_dir_all);
                    let _ = std::fs::write(f, format!("{url}\n{v}"));
                }
            }
            None => {
                known.remove(url);
                if let Some(f) = self.file(url) {
                    let _ = std::fs::remove_file(f);
                }
            }
        }
    }

    /// `GET url` from `offset` (see [`Client::open_at`]).
    pub fn open(&self, req: &Request, offset: u64, cancel: &CancelToken) -> Result<Opened, Error> {
        let url = req.url().to_string();
        let v = if offset > 0 {
            self.validator(&url)
        } else {
            None
        };
        let o = self.client.open_at(req, offset, v.as_deref(), cancel)?;
        self.remember(&url, o.validator());
        Ok(o)
    }

    /// Forgets `url` (its download finished or was abandoned).
    pub fn forget(&self, url: &str) {
        self.remember(url, None);
    }
}
