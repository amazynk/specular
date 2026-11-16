//! A web file played while it is fetched, when its server takes byte ranges.
//!
//! The file is made at its full length; its index (an MP4's `moov`, wherever it is) is
//! fetched first, then the file is offered and the rest is fetched by ranges: what a reader
//! waits for first (a seek), else on from where the readers are (see
//! [`specular_core::fill`]). A file without an index up front (fragmented MP4) or of another
//! kind is fetched whole first, as without ranges.

use std::cell::Cell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use specular_core::fill::{self, Filling};
use specular_remote::{Client, Request};

use super::{Failure, Job, Shared, StreamKind, net_failure};

/// Bytes asked for in one request at most.
const RUN: u64 = 8 << 20;
/// A wanted range this close after the bytes coming in is waited for, not jumped to.
const SLACK: u64 = 2 << 20;
/// Small top-level boxes fetched whole while looking for the index.
const SMALL_BOX: u64 = 1 << 20;
/// How long a request for a part of a file on this computer itself waits for its answer
/// before it is asked again, or the part wanted by then is.
const LOCAL_WAIT: Duration = Duration::from_secs(5);
/// How long a lane with nothing to ask for waits before it looks again (it is woken by
/// every change).
const IDLE_WAIT: Duration = Duration::from_millis(500);
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
        if self.opts.client.waits_for(url) {
            return Lanes::new(self.sh, &self.opts.client, url, file, filling).fill();
        }
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
                    // A reader waits for a part not coming soon: go there. What it misses is
                    // told by its first missing byte, not by where its read starts: a read
                    // across the end of what came in so far (a server that sends a little
                    // at a time) is being answered by this very request.
                    match filling.wanted_hole() {
                        Some(w) if !(w >= p && w < (end + SLACK).min(b.max(end))) => {
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

/// No request under way.
const IDLE: u64 = u64::MAX;
/// Requests answering blocked reads, beside the one fetching the file in order.
const SIDES: usize = 2;

/// The fetch of a file from this computer itself (a downloader's file while it downloads),
/// whose server may hold a request until it has the bytes: for seconds, for minutes.
///
/// One request alone would starve the player whenever a read far ahead (a seek, a cover's
/// frame, a fingerprint) is being held, so there are several, which cost nothing here: lane
/// 0 fetches the file in order, from its first hole on, as the server gets it; each of the
/// other lanes answers a blocked read that no lane is about to, and keeps its request open
/// (asked again every [`LOCAL_WAIT`]) for as long as that read waits.
struct Lanes<'a> {
    sh: &'a Shared,
    client: &'a Client,
    url: &'a str,
    file: &'a File,
    filling: &'a Filling,
    /// One write at a time (a seek and a write on the one file).
    write: Mutex<()>,
    /// Where each lane's next byte goes, [`IDLE`] between its requests.
    at: [AtomicU64; 1 + SIDES],
    /// The file is whole, or the fetch is over.
    stop: AtomicBool,
    /// What ended a side lane, for the stream to end with.
    failure: Mutex<Option<Failure>>,
}

/// How one request ended: the client's answer, whether bytes came, and a failed write.
type Asked = (Result<(), specular_remote::Error>, bool, Option<Failure>);

impl<'a> Lanes<'a> {
    fn new(
        sh: &'a Shared,
        client: &'a Client,
        url: &'a str,
        file: &'a File,
        filling: &'a Filling,
    ) -> Self {
        Lanes {
            sh,
            client,
            url,
            file,
            filling,
            write: Mutex::new(()),
            at: std::array::from_fn(|_| AtomicU64::new(IDLE)),
            stop: AtomicBool::new(false),
            failure: Mutex::new(None),
        }
    }

    fn fill(&self) -> Result<(), Failure> {
        std::thread::scope(|sc| {
            for i in 1..=SIDES {
                specular_thread::spawn_scoped(sc, format!("stream-side-{i}"), move || self.side(i));
            }
            let r = self.front();
            self.stop.store(true, Ordering::Release);
            r
        })
    }

    fn over(&self) -> bool {
        self.stop.load(Ordering::Acquire) || self.sh.cancel.is_cancelled()
    }

    /// Whether a lane other than `me` is about to bring byte `w`.
    fn coming(&self, me: usize, w: u64) -> bool {
        self.at.iter().enumerate().any(|(i, at)| {
            let p = at.load(Ordering::Acquire);
            i != me && p != IDLE && w >= p && w < p.saturating_add(SLACK)
        })
    }

    /// The blocked read side lane `me` should answer: the first (see
    /// [`Filling::wanted_holes`]) that no other lane is about to, and that is not the
    /// file's first hole (lane 0's).
    fn target(&self, me: usize) -> Option<u64> {
        let first = self.filling.hole_from(0).map(|h| h.0);
        self.filling
            .wanted_holes()
            .into_iter()
            .find(|&w| Some(w) != first && !self.coming(me, w))
    }

    /// Asks for `a..b` on lane `me` and writes what comes; `go_on(p, end)`, after each piece
    /// `p..end`, says whether to take more.
    fn ask(&self, me: usize, a: u64, b: u64, go_on: impl Fn(u64, u64) -> bool) -> Asked {
        let req = Request::get(self.url).range(a..b).local_wait(LOCAL_WAIT);
        let (pos, whole, wrote) = (Cell::new(a), Cell::new(false), Cell::new(false));
        let mut fail = None;
        self.at[me].store(a, Ordering::Release);
        let r = self.client.get_stream_head(
            &req,
            &self.sh.cancel,
            |h| {
                // A server ignoring the range sends the whole file.
                whole.set(h.status != 206);
                pos.set(if whole.get() { 0 } else { a });
                ControlFlow::Continue(())
            },
            |chunk, _| {
                if self.over() {
                    return ControlFlow::Break(());
                }
                let p = pos.get();
                let end = p + chunk.len() as u64;
                // Into what another lane fetched already: plan again.
                if !whole.get() && self.filling.has(p, end) {
                    return ControlFlow::Break(());
                }
                let written = {
                    let _one = self.write.lock();
                    write_at(self.file, p, chunk)
                };
                if let Err(e) = written {
                    fail = Some(disk(e));
                    return ControlFlow::Break(());
                }
                self.filling.add(p, end);
                pos.set(end);
                self.at[me].store(end, Ordering::Release);
                wrote.set(true);
                self.sh.st.lock().received = self.filling.filled();
                if whole.get() || go_on(p, end) {
                    ControlFlow::Continue(())
                } else {
                    ControlFlow::Break(())
                }
            },
        );
        self.at[me].store(IDLE, Ordering::Release);
        specular_log::debug!(target: specular_log::IO, "stream web file: lane {me} asked for {a}..{b}, got to {}: {}", pos.get(), match &r { Ok(_) => "done".to_string(), Err(e) => e.to_string() });
        (r.map(|_| ()), wrote.get(), fail)
    }

    /// What a request's end means for its lane: on (after a pause when it failed), or `Err`
    /// when the stream ends with it.
    fn after(&self, asked: Asked, fails: &mut u32) -> Result<(), Failure> {
        let (r, wrote, fail) = asked;
        if let Some(f) = fail {
            return Err(f);
        }
        match r {
            Ok(()) => *fails = 0,
            Err(e) if e.is_network_off() => return Err(Failure::NetworkOff),
            Err(_) if self.over() => {}
            // No answer yet: the server holds the request until it has the bytes. Waited
            // for, however long, by asking again.
            Err(specular_remote::Error::Timeout) if !wrote => {}
            Err(e) => {
                *fails = if wrote { 1 } else { *fails + 1 };
                if *fails >= MAX_FAILS {
                    return Err(net_failure(e));
                }
                // Cancelled while asleep: the lane's loop sees it.
                let _ = specular_remote::http::sleep_cancellable(
                    Duration::from_millis(300 * u64::from(*fails)),
                    &self.sh.cancel,
                );
            }
        }
        Ok(())
    }

    /// Lane 0: the file in order, from its first hole on.
    fn front(&self) -> Result<(), Failure> {
        let mut fails = 0;
        let mut seen = self.filling.changes();
        loop {
            if self.sh.cancel.is_cancelled() {
                return Ok(());
            }
            if let Some(f) = self.failure.lock().take() {
                return Err(f);
            }
            let Some((a, e)) = self.filling.hole_from(0) else {
                return Ok(());
            };
            // A side lane is at it (a seek the fetch in order has caught up with): it goes
            // on from there once that lane's request is over.
            if self.coming(0, a) {
                seen = self.filling.wait_past(seen, IDLE_WAIT);
                continue;
            }
            let asked = self.ask(0, a, e.min(a.saturating_add(RUN)), |_, _| true);
            self.after(asked, &mut fails)?;
        }
    }

    /// A side lane: the blocked reads no other lane is about to answer.
    fn side(&self, me: usize) {
        let mut fails = 0;
        let mut seen = self.filling.changes();
        while !self.over() {
            let hole = self.target(me).and_then(|w| self.filling.hole_from(w));
            let Some((a, e)) = hole else {
                seen = self.filling.wait_past(seen, IDLE_WAIT);
                continue;
            };
            let asked = self.ask(me, a, e.min(a.saturating_add(RUN)), |p, end| {
                // On while a blocked read is answered by this request; away to one that
                // nothing answers; else on (a player reading along behind it).
                let near = |w: u64| w >= p && w < end.saturating_add(SLACK);
                self.filling.wanted_holes().into_iter().any(near) || self.target(me).is_none()
            });
            if let Err(f) = self.after(asked, &mut fails) {
                *self.failure.lock() = Some(f);
                return;
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
