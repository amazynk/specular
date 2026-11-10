//! A web file played while it is fetched, when its server takes byte ranges.
//!
//! The file is made at its full length; its index (an MP4's `moov`, wherever it is) is
//! fetched first, then the file is offered and the rest is fetched by ranges: what a reader
//! waits for first (a seek), else on from where the readers are (see
//! [`specular_core::fill`]). A file without an index up front (fragmented MP4) or of another
//! kind is fetched whole first, as without ranges.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use specular_core::fill::{self, Filling};
use specular_remote::Request;

use super::{Failure, Job, StreamKind, net_failure};

/// Bytes asked for in one request at most.
const RUN: u64 = 8 << 20;
/// A wanted range this close after the bytes coming in is waited for, not jumped to.
const SLACK: u64 = 2 << 20;
/// Small top-level boxes fetched whole while looking for the index.
const SMALL_BOX: u64 = 1 << 20;
/// Failed requests in a row before the stream stops.
const MAX_FAILS: u32 = 5;
/// Files smaller than this are fetched whole.
pub(super) const MIN_RANGED: u64 = 1 << 20;

/// Whether the first bytes are an ISO base media file (MP4, MOV, M4A, 3GP).
pub(super) fn is_isobmff(first: &[u8]) -> bool {
    first.len() >= 8
        && matches!(
            &first[4..8],
            b"ftyp" | b"moov" | b"free" | b"skip" | b"wide" | b"mdat" | b"pdin"
        )
}

/// Closes the filling on every way out: blocked readers fail at once.
struct CloseOnDrop(Arc<Filling>);

impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        self.0.close();
    }
}

fn disk(e: std::io::Error) -> Failure {
    Failure::Disk(e.to_string())
}

fn write_at(mut f: &File, at: u64, data: &[u8]) -> std::io::Result<()> {
    f.seek(SeekFrom::Start(at))?;
    f.write_all(data)
}

fn read_at(mut f: &File, at: u64, n: usize) -> std::io::Result<Vec<u8>> {
    f.seek(SeekFrom::Start(at))?;
    let mut v = vec![0; n];
    f.read_exact(&mut v)?;
    Ok(v)
}

fn be32(b: &[u8]) -> u64 {
    u64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Whether box body `b` holds a child box of kind `kind` (at its top level).
fn has_child(b: &[u8], kind: &[u8; 4]) -> bool {
    let mut at = 0usize;
    while at + 8 <= b.len() {
        let size = be32(&b[at..]) as usize;
        if &b[at + 4..at + 8] == kind {
            return true;
        }
        if size < 8 {
            return false;
        }
        at = at.saturating_add(size);
    }
    false
}

impl Job<'_> {
    /// Plays `url` (`total` bytes, `first` of them read) while fetching it by ranges.
    pub(super) fn web_file(
        &mut self,
        url: &str,
        path: PathBuf,
        total: u64,
        first: &[u8],
    ) -> Result<(), Failure> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(disk)?;
        file.set_len(total).map_err(disk)?;
        let filling = Filling::new(total, self.opts.read_timeout);
        let _closed = CloseOnDrop(Arc::clone(&filling));
        *self.sh.filling.lock() = Some((Arc::clone(&filling), fill::register(&path, &filling)));
        let n = first.len().min(total as usize);
        write_at(&file, 0, &first[..n]).map_err(disk)?;
        filling.add(0, n as u64);
        self.set(|s| {
            s.kind = Some(StreamKind::File);
            s.total = Some(total);
            s.received = n as u64;
        });
        let seekable = self.mp4_index(url, &file, &filling)?;
        specular_log::debug!(target: specular_log::IO, "stream web file of {total} bytes by ranges; playable while fetched: {seekable}");
        if seekable {
            self.set(|s| s.ranged = true);
            self.file_done(path.clone())?;
        }
        self.fill(url, &file, &filling)?;
        if !seekable && !self.cancelled() {
            self.file_done(path)?;
        }
        Ok(())
    }

    /// Bytes `at..at + n` of the file, fetched when not there yet.
    fn bytes_at(
        &mut self,
        url: &str,
        file: &File,
        filling: &Filling,
        at: u64,
        n: u64,
    ) -> Result<Vec<u8>, Failure> {
        let n = n.min(filling.len().saturating_sub(at));
        if !filling.has(at, at + n) {
            let data = self.fetch(url, Some((at, n)))?;
            if (data.len() as u64) < n {
                return Err(Failure::Unreachable(
                    "the server sent less than asked".into(),
                ));
            }
            write_at(file, at, &data[..n as usize]).map_err(disk)?;
            filling.add(at, at + n);
            self.set(|s| s.received = filling.filled());
        }
        read_at(file, at, n as usize).map_err(disk)
    }

    /// Fetches the MP4's top-level box headers, its small boxes and its `moov`. Whether the
    /// file can play before the rest comes (a `moov` with every sample, not fragments).
    fn mp4_index(&mut self, url: &str, file: &File, filling: &Filling) -> Result<bool, Failure> {
        let total = filling.len();
        let (mut at, mut moov, mut frag) = (0u64, false, false);
        for _ in 0..256 {
            if at + 8 > total || self.cancelled() {
                break;
            }
            let h = self.bytes_at(url, file, filling, at, 16)?;
            let size = match be32(&h) {
                0 => total - at,
                1 if h.len() >= 16 => u64::from_be_bytes(h[8..16].try_into().unwrap_or([0; 8])),
                n => n,
            };
            if size < 8 {
                break;
            }
            let kind: [u8; 4] = h[4..8].try_into().unwrap_or([0; 4]);
            match &kind {
                b"moov" => {
                    let body = self.bytes_at(url, file, filling, at, size)?;
                    moov = true;
                    frag |= has_child(body.get(8..).unwrap_or(&[]), b"mvex");
                }
                b"moof" => frag = true,
                b"mdat" => {}
                _ if size <= SMALL_BOX => {
                    self.bytes_at(url, file, filling, at, size)?;
                }
                _ => {}
            }
            at = at.saturating_add(size);
        }
        Ok(moov && !frag)
    }

    /// Fetches the rest, by ranges, in the order the readers need it.
    fn fill(&mut self, url: &str, file: &File, filling: &Filling) -> Result<(), Failure> {
        let mut fails = 0;
        loop {
            if self.cancelled() {
                return Ok(());
            }
            let Some((a, b)) = filling.next_fetch(RUN) else {
                return Ok(());
            };
            let req = Request::get(url).range(a..b);
            let pos = std::cell::Cell::new(a);
            let mut fail = None;
            let (sh, wrote) = (self.sh, std::cell::Cell::new(false));
            let r = self.opts.client.get_stream_head(
                &req,
                &sh.cancel,
                |h| {
                    // A server ignoring the range sends the whole file.
                    pos.set(if h.status == 206 { a } else { 0 });
                    ControlFlow::Continue(())
                },
                |chunk, _| {
                    if sh.cancel.is_cancelled() {
                        return ControlFlow::Break(());
                    }
                    let p = pos.get();
                    if let Err(e) = write_at(file, p, chunk) {
                        fail = Some(disk(e));
                        return ControlFlow::Break(());
                    }
                    let end = p + chunk.len() as u64;
                    filling.add(p, end);
                    pos.set(end);
                    wrote.set(true);
                    sh.st.lock().received = filling.filled();
                    // A reader waits for a part not coming soon: go there.
                    match filling.wanted() {
                        Some((w, _)) if !(w >= p && w < (end + SLACK).min(b.max(end))) => {
                            ControlFlow::Break(())
                        }
                        _ => ControlFlow::Continue(()),
                    }
                },
            );
            if let Some(f) = fail {
                return Err(f);
            }
            match r {
                Ok(_) => fails = 0,
                Err(e) if e.is_network_off() => return Err(Failure::NetworkOff),
                Err(e) => {
                    if self.cancelled() {
                        return Ok(());
                    }
                    fails = if wrote.get() { 1 } else { fails + 1 };
                    if fails >= MAX_FAILS {
                        return Err(net_failure(e));
                    }
                    specular_remote::http::sleep_cancellable(
                        Duration::from_millis(300 * u64::from(fails)),
                        &self.sh.cancel,
                    )
                    .map_err(|_| Failure::Unreachable("cancelled".into()))?;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp4_boxes_are_told() {
        assert!(is_isobmff(b"\0\0\0\x20ftypisom"));
        assert!(is_isobmff(b"\0\0\0\x08mdat"));
        assert!(!is_isobmff(b"ID3\x04\0\0\0\0\0\0"));
        let mut moov = Vec::new();
        moov.extend_from_slice(&16u32.to_be_bytes());
        moov.extend_from_slice(b"mvhd");
        moov.extend_from_slice(&[0; 8]);
        moov.extend_from_slice(&8u32.to_be_bytes());
        moov.extend_from_slice(b"mvex");
        assert!(has_child(&moov, b"mvex"));
        assert!(!has_child(&moov[..16], b"mvex"));
    }
}
