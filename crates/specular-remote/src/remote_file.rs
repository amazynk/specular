//! A file on an HTTP server, read by byte ranges or downloaded whole into the disk cache.
//!
//! Decoders take a [`Source`]: [`RemoteFile::source`] downloads (or finds) the file in the
//! disk cache and maps it like any local file. [`RemoteFile::read_at`] reads ranges in
//! fixed-size blocks kept in an in-memory LRU, for formats that only need parts of a file
//! (tiled TIFF/COG, Zarr chunks, a DICOM header) — the seam Stage 5 builds on.

use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::{CancelToken, Source};

use crate::auth::Auth;
use crate::cache::DiskCache;
use crate::http::{Client, Error, Progress, Request};
use crate::lru::MemLru;

/// Range block size: one request per 256 KiB (latency dominates small reads).
pub const BLOCK: u64 = 256 * 1024;

/// A remote file.
pub struct RemoteFile {
    client: Client,
    url: String,
    auth: Auth,
    cache: Option<Arc<DiskCache>>,
    state: Mutex<State>,
}

struct State {
    len: Option<u64>,
    validator: Option<String>,
    ranges: bool,
    blocks: MemLru<u64, Arc<[u8]>>,
}

impl std::fmt::Debug for RemoteFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteFile")
            .field("url", &crate::http::redact_url(&self.url))
            .finish()
    }
}

impl RemoteFile {
    /// `memory`: bytes of blocks kept in memory.
    pub fn new(
        client: Client,
        url: impl Into<String>,
        auth: Auth,
        cache: Option<Arc<DiskCache>>,
        memory: u64,
    ) -> Self {
        RemoteFile {
            client,
            url: url.into(),
            auth,
            cache,
            state: Mutex::new(State {
                len: None,
                validator: None,
                ranges: true,
                blocks: MemLru::new(memory),
            }),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    fn request(&self) -> Request {
        Request::get(self.url.clone()).auth(&self.auth)
    }

    /// Size of the file (one small range request the first time).
    pub fn len(&self, cancel: &CancelToken) -> Result<u64, Error> {
        if let Some(n) = self.state.lock().len {
            return Ok(n);
        }
        let mut first = Vec::new();
        let head = self
            .client
            .get_stream(&self.request().range(0..BLOCK), cancel, |c, _| {
                first.extend_from_slice(c);
                if first.len() as u64 >= BLOCK {
                    std::ops::ControlFlow::Break(())
                } else {
                    std::ops::ControlFlow::Continue(())
                }
            })?;
        let mut st = self.state.lock();
        st.ranges = head.status == 206;
        st.validator = head.headers.validator().map(str::to_string);
        let len = head
            .total_len()
            .or_else(|| (head.status == 200).then_some(first.len() as u64))
            .ok_or_else(|| Error::Protocol("server did not say the file size".into()))?;
        st.len = Some(len);
        first.truncate(BLOCK as usize);
        let w = first.len() as u64;
        st.blocks.insert(0, Arc::from(first), w);
        Ok(len)
    }

    /// Reads `len` bytes at `offset` (fewer at the end of the file).
    pub fn read_at(&self, offset: u64, len: usize, cancel: &CancelToken) -> Result<Vec<u8>, Error> {
        let total = self.len(cancel)?;
        let end = (offset + len as u64).min(total);
        if offset >= end {
            return Ok(Vec::new());
        }
        let (b0, b1) = (offset / BLOCK, (end - 1) / BLOCK);
        let mut out = Vec::with_capacity((end - offset) as usize);
        let mut b = b0;
        while b <= b1 {
            let cached = self.state.lock().blocks.get(&b);
            let block = match cached {
                Some(c) => c,
                None => {
                    // Coalesce the run of missing blocks into one request.
                    let mut last = b;
                    while last < b1 && !self.state.lock().blocks.contains(&(last + 1)) {
                        last += 1;
                    }
                    self.fetch_blocks(b, last, total, cancel)?;
                    match self.state.lock().blocks.get(&b) {
                        Some(c) => c,
                        None => return Err(Error::Protocol("block not kept".into())),
                    }
                }
            };
            let bstart = b * BLOCK;
            let lo = offset.max(bstart) - bstart;
            let hi = end.min(bstart + block.len() as u64) - bstart;
            out.extend_from_slice(&block[lo as usize..hi as usize]);
            b += 1;
        }
        Ok(out)
    }

    fn fetch_blocks(
        &self,
        b0: u64,
        b1: u64,
        total: u64,
        cancel: &CancelToken,
    ) -> Result<(), Error> {
        let start = b0 * BLOCK;
        let end = ((b1 + 1) * BLOCK).min(total);
        let (ranges, validator) = {
            let st = self.state.lock();
            (st.ranges, st.validator.clone())
        };
        let mut req = self.request();
        if ranges {
            req = req.range(start..end);
            if let Some(v) = validator {
                req = req.header("If-Range", v);
            }
        }
        let res = self.client.get(&req, cancel)?;
        let body = if res.status == 206 {
            if res.headers.content_range().map(|r| r.0) != Some(start) {
                return Err(Error::Protocol("server sent another range".into()));
            }
            res.body
        } else {
            // Ranges unsupported (or the file changed): take the slice from the whole body.
            let mut st = self.state.lock();
            st.ranges = false;
            if res.body.len() as u64 != total {
                st.len = Some(res.body.len() as u64);
                st.blocks.clear();
            }
            res.body
                .get(start as usize..(end as usize).min(res.body.len()))
                .unwrap_or_default()
                .to_vec()
        };
        let mut st = self.state.lock();
        for (i, chunk) in body.chunks(BLOCK as usize).enumerate() {
            st.blocks
                .insert(b0 + i as u64, Arc::from(chunk), chunk.len() as u64);
        }
        Ok(())
    }

    /// The whole file as a [`Source`] (from the disk cache, downloading it first if needed).
    /// Without a disk cache the bytes are held in memory.
    pub fn source(
        &self,
        cancel: &CancelToken,
        on_progress: impl FnMut(&Progress),
    ) -> Result<Source, Error> {
        match &self.cache {
            Some(cache) => {
                let rel = DiskCache::url_key(&self.url);
                let p = cache.fetch(&self.client, &self.request(), &rel, cancel, on_progress)?;
                Ok(Source::open(&p)?)
            }
            None => {
                let mut on_progress = on_progress;
                let mut v = Vec::new();
                self.client.get_stream(&self.request(), cancel, |c, p| {
                    v.extend_from_slice(c);
                    on_progress(p);
                    std::ops::ControlFlow::Continue(())
                })?;
                Ok(Source::from_vec(crate::http::redact_url(&self.url), v))
            }
        }
    }
}
