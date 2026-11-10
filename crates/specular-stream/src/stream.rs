//! The worker that fetches a stream into the cache.
//!
//! - A file with a known length (an MP4, an MP3 on a web server) is fetched whole into one
//!   file; the player opens it when it is complete (the download indicator shows how far).
//!   With byte ranges, an MP4 plays while it is fetched (`webfile.rs`) and a Matroska or
//!   WebM file plays in pieces cut at its clusters (`mkvpieces.rs`).
//! - An endless sound stream (internet radio, ICY titles taken out) is cut into pieces of
//!   [`Options::radio_piece_s`] at frame boundaries.
//! - HLS and DASH segments are put together into pieces of about [`Options::piece_s`]: MPEG-TS
//!   segments joined, fragmented MP4 rewritten as plain MP4 (with a DASH or HLS sound
//!   rendition as its second track), AES-128 segments decrypted.
//!
//! Each piece is a file that plays on its own; the player plays them one after another (the
//! sound without a gap). The worker keeps at most [`Options::ahead_s`] of a finite stream
//! ahead of the piece playing, and the files before the one playing are deleted. A variant
//! (quality) is picked by measured throughput, or as the user chose; a change applies from
//! the next piece.

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};
use specular_remote::{CancelToken, Client, Request};

use crate::addr::{self, Guess};
use crate::dash;
use crate::fmp4::{self, Defrag};
use crate::frames::{Cutter, Framing};
use crate::hls;
use crate::icy;
use crate::ytdlp;

mod mkvpieces;
mod webfile;

/// Text documents (playlists, manifests) read at most.
const MAX_TEXT: u64 = 4 << 20;
/// A segment read at most.
const MAX_SEGMENT: u64 = 256 << 20;
/// How long yt-dlp may take.
const YTDLP_TIMEOUT: Duration = Duration::from_secs(60);
/// Playlists of playlists followed at most.
const MAX_HOPS: u32 = 4;

/// Called when the status changes (the app wakes its event loop).
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// How the stream is fetched.
#[derive(Clone)]
pub struct Options {
    /// Where the files go (made; deleted when the stream stops).
    pub dir: PathBuf,
    /// The client (tracked as a stream in the download indicator).
    pub client: Client,
    /// yt-dlp, when the user turned it on and it is installed.
    pub ytdlp: Option<PathBuf>,
    pub radio_piece_s: f64,
    pub piece_s: f64,
    pub ahead_s: f64,
    /// The variant asked for (`None`: by throughput).
    pub variant: Option<usize>,
    pub wake: Option<Wake>,
    /// How long a reader of a web file played while fetched waits for a missing part.
    pub read_timeout: Duration,
}

impl Options {
    pub fn new(dir: PathBuf, client: Client) -> Options {
        Options {
            dir,
            client,
            ytdlp: None,
            radio_piece_s: 5.0,
            piece_s: 10.0,
            ahead_s: 60.0,
            variant: None,
            wake: None,
            read_timeout: Duration::from_secs(20),
        }
    }
}

/// What the stream turned out to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamKind {
    File,
    Radio,
    Hls,
    Dash,
    /// A Matroska or WebM web file played in pieces cut at its clusters while it is fetched
    /// by ranges: seekable as HLS is.
    Pieces,
}

/// A piece ready to play.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    pub path: PathBuf,
    pub index: u32,
    /// Seconds from the stream's start (or from where a live stream was joined).
    pub start: f64,
    pub seconds: f64,
    /// The now-playing title when it was received (radio).
    pub title: Option<String>,
}

/// Why the stream stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The "No network" switch is on.
    NetworkOff,
    /// Protected by a DRM system.
    Drm,
    /// Encrypted in a way not read here (HLS SAMPLE-AES).
    Encrypted,
    /// A web page: only yt-dlp could open it (`true`: it is installed but turned off).
    Page { installed: bool },
    /// yt-dlp failed, with its message.
    YtDlp(String),
    /// The server answered with this status.
    Status(u16),
    /// The server could not be reached, or the connection broke.
    Unreachable(String),
    /// Nothing playable (an empty playlist, not media at all).
    Empty,
    /// A kind of stream not read here, with the reason.
    Unsupported(String),
    /// Writing to the cache failed.
    Disk(String),
}

/// What the app shows of a stream.
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub kind: Option<StreamKind>,
    /// The stream's name: ICY `icy-name`, yt-dlp's title, a playlist entry's title.
    pub title: Option<String>,
    /// The radio's now-playing title.
    pub now_playing: Option<String>,
    pub chunks: Vec<Chunk>,
    /// No more pieces will come (finished, failed or stopped). A Matroska web file whose
    /// pieces were all made still answers a seek (it is then false again).
    pub ended: bool,
    pub failure: Option<Failure>,
    /// The variants (qualities) on offer, lowest first, as labels.
    pub variants: Vec<String>,
    /// The variant of the last piece (an index into `variants`).
    pub variant: Option<usize>,
    /// The user chose the variant (else by throughput).
    pub manual: bool,
    pub live: bool,
    /// Seconds, when known (a finite HLS or DASH stream).
    pub duration: Option<f64>,
    /// Bytes of a whole file fetched, and its length when known.
    pub received: u64,
    pub total: Option<u64>,
    /// The web file plays while it is fetched (its server takes byte ranges).
    pub ranged: bool,
    /// The last seek asked for ([`Stream::seek`]): its time, and the first piece made for
    /// it (the piece holding the time, or one after).
    pub seek: Option<(f64, u32)>,
}

#[derive(Default)]
struct Control {
    /// The piece playing.
    playing: Option<u32>,
    /// A variant asked for: `Some(None)` by throughput.
    variant: Option<Option<usize>>,
    /// A seek asked for, in seconds from the stream's start.
    seek: Option<f64>,
}

struct Shared {
    notify: Option<Wake>,
    st: Mutex<Status>,
    ctl: Mutex<Control>,
    wake: Condvar,
    cancel: CancelToken,
    /// The web file being filled, when it plays while fetched, registered for its path as
    /// long as the stream lives (a source opened on it after a failure still never reads
    /// the zeros of a hole).
    filling: Mutex<
        Option<(
            Arc<specular_core::fill::Filling>,
            specular_core::fill::Registered,
        )>,
    >,
}

impl Shared {
    fn changed(&self) {
        if let Some(w) = &self.notify {
            w();
        }
    }
}

/// A stream being fetched. Dropping it stops the worker, which deletes its files.
pub struct Stream {
    sh: Arc<Shared>,
    dir: PathBuf,
}

impl Stream {
    /// Starts fetching `url` on a worker.
    pub fn open(opts: Options, url: &str) -> Stream {
        let sh = Arc::new(Shared {
            notify: opts.wake.clone(),
            st: Mutex::new(Status::default()),
            ctl: Mutex::new(Control {
                playing: None,
                variant: opts.variant.map(Some),
                seek: None,
            }),
            wake: Condvar::new(),
            cancel: CancelToken::new(),
            filling: Mutex::new(None),
        });
        let dir = opts.dir.clone();
        let w = Arc::clone(&sh);
        let url = url.trim().to_owned();
        let spawned = specular_thread::builder("stream").spawn(move || {
            let mut job = Job {
                sh: &w,
                opts: &opts,
                index: 0,
                keys: HashMap::new(),
                inits: HashMap::new(),
                bps: None,
                title: None,
            };
            let r = job.run(url);
            let mut st = w.st.lock();
            st.ended = true;
            if let Err(f) = r
                && !w.cancel.is_cancelled()
            {
                specular_log::info!(target: specular_log::IO, "stream stopped: {f:?}");
                st.failure = Some(f);
            }
            // Stopped: the files go (here, or in `drop` when it ended before the stop).
            if w.cancel.is_cancelled() {
                let _ = std::fs::remove_dir_all(&opts.dir);
            }
            drop(st);
            w.changed();
        });
        if let Err(e) = spawned {
            let mut st = sh.st.lock();
            st.ended = true;
            st.failure = Some(Failure::Unsupported(e.to_string()));
        }
        Stream { sh, dir }
    }

    pub fn status(&self) -> Status {
        self.sh.st.lock().clone()
    }

    /// The folder its files are in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Whether `path` is one of its pieces.
    pub fn owns(&self, path: &Path) -> bool {
        path.parent() == Some(self.dir.as_path())
    }

    /// The piece at `path`, if it is one.
    pub fn chunk_of(&self, path: &Path) -> Option<Chunk> {
        self.sh
            .st
            .lock()
            .chunks
            .iter()
            .find(|c| c.path == path)
            .cloned()
    }

    /// The piece after `path`, when it is ready.
    pub fn after(&self, path: &Path) -> Option<Chunk> {
        let st = self.sh.st.lock();
        let i = st.chunks.iter().find(|c| c.path == path)?.index;
        st.chunks.iter().find(|c| c.index == i + 1).cloned()
    }

    /// The piece `index` is playing: those before the one before it are deleted, and the
    /// worker may fetch further.
    pub fn playing(&self, index: u32) {
        self.sh.ctl.lock().playing = Some(index);
        let gone: Vec<PathBuf> = {
            let mut st = self.sh.st.lock();
            let (old, keep): (Vec<Chunk>, Vec<Chunk>) =
                st.chunks.drain(..).partition(|c| c.index + 1 < index);
            st.chunks = keep;
            old.into_iter().map(|c| c.path).collect()
        };
        for p in gone {
            let _ = std::fs::remove_file(p);
        }
        self.sh.wake.notify_all();
    }

    /// Plays variant `v` from the next piece (`None`: by throughput).
    pub fn choose(&self, v: Option<usize>) {
        self.sh.ctl.lock().variant = Some(v);
        self.sh.st.lock().manual = v.is_some();
        self.sh.wake.notify_all();
    }

    /// Plays on from `t` seconds (a finite HLS or DASH stream): the pieces after the one
    /// playing go, and the next piece made starts with the segment holding `t`
    /// ([`Status::seek`] says which piece that is once it is known).
    pub fn seek(&self, t: f64) {
        self.sh.ctl.lock().seek = Some(t.max(0.0));
        self.sh.st.lock().seek = None;
        self.sh.wake.notify_all();
    }

    /// The piece holding `t` seconds, when it is ready.
    pub fn chunk_at(&self, t: f64) -> Option<Chunk> {
        let st = self.sh.st.lock();
        let from = st.seek.map_or(0, |s| s.1);
        st.chunks
            .iter()
            .filter(|c| c.index >= from || st.seek.is_none())
            .find(|c| c.start <= t + 1e-3 && t < c.start + c.seconds)
            .cloned()
    }

    /// A reader of the web file waits for a part still being fetched (buffering).
    pub fn buffering(&self) -> bool {
        self.sh
            .filling
            .lock()
            .as_ref()
            .is_some_and(|f| f.0.waiting())
    }

    /// A part of the web file did not come within [`Options::read_timeout`].
    pub fn stalled(&self) -> bool {
        self.sh
            .filling
            .lock()
            .as_ref()
            .is_some_and(|f| f.0.stalled())
    }

    /// Stops fetching (the worker then deletes the files).
    pub fn stop(&self) {
        self.sh.cancel.cancel();
        if let Some(f) = self.sh.filling.lock().as_ref() {
            f.0.close();
        }
        self.sh.wake.notify_all();
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.stop();
        // A worker that already ended does not see the stop: its files go here.
        let st = self.sh.st.lock();
        if st.ended {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

fn net_failure(e: specular_remote::Error) -> Failure {
    use specular_remote::Error as E;
    match e {
        E::NetworkOff => Failure::NetworkOff,
        E::Status(s) => Failure::Status(s),
        E::Cancelled => Failure::Unreachable("cancelled".into()),
        e => Failure::Unreachable(e.to_string()),
    }
}

/// What the first answer turned out to be.
enum Probe {
    Undecided(Vec<u8>),
    Text(Vec<u8>),
    Radio {
        demux: icy::Demux,
        cutter: Cutter,
    },
    File {
        file: std::fs::File,
        path: PathBuf,
    },
    /// A file played while fetched by ranges: its first bytes.
    Ranged {
        path: PathBuf,
        first: Vec<u8>,
    },
}

/// An initialisation segment's address and byte range.
type InitKey = (String, Option<(u64, u64)>);

struct Job<'a> {
    sh: &'a Shared,
    opts: &'a Options,
    index: u32,
    keys: HashMap<String, Vec<u8>>,
    inits: HashMap<InitKey, Arc<Vec<u8>>>,
    /// Measured throughput, bits per second.
    bps: Option<f64>,
    title: Option<String>,
}

impl Job<'_> {
    fn cancelled(&self) -> bool {
        self.sh.cancel.is_cancelled()
    }

    fn set(&self, f: impl FnOnce(&mut Status)) {
        f(&mut self.sh.st.lock());
    }

    fn run(&mut self, mut url: String) -> Result<(), Failure> {
        if specular_core::net::off::is_off() {
            return Err(Failure::NetworkOff);
        }
        std::fs::create_dir_all(&self.opts.dir).map_err(|e| Failure::Disk(e.to_string()))?;
        for _ in 0..MAX_HOPS {
            let next = match addr::guess(&url) {
                Some(Guess::Page) => self.page(&url)?,
                Some(Guess::Hls) => {
                    let (text, _) = self.text(&url)?;
                    return self.hls(&url, &text);
                }
                Some(Guess::Dash) => {
                    let (text, _) = self.text(&url)?;
                    return self.dash(&url, &text);
                }
                Some(Guess::Playlist) => {
                    let (text, _) = self.text(&url)?;
                    self.playlist(&url, &text)?
                }
                _ => match self.probe(&url)? {
                    Some((Guess::Hls, text)) => return self.hls(&url, &text),
                    Some((Guess::Dash, text)) => return self.dash(&url, &text),
                    Some((Guess::Playlist, text)) => self.playlist(&url, &text)?,
                    Some((Guess::Page, _)) => self.page(&url)?,
                    Some((Guess::Media, _)) => return Err(Failure::Empty),
                    None => return Ok(()),
                },
            };
            url = next;
        }
        Err(Failure::Empty)
    }

    /// A page: the media address yt-dlp finds on it.
    fn page(&mut self, url: &str) -> Result<String, Failure> {
        let Some(prog) = self.opts.ytdlp.clone() else {
            return Err(Failure::Page {
                installed: ytdlp::find().is_some(),
            });
        };
        let cancel = self.sh.cancel.clone();
        let found = ytdlp::run(&prog, url, YTDLP_TIMEOUT, &|| cancel.is_cancelled()).map_err(
            |e| match e {
                ytdlp::Error::Start(m) | ytdlp::Error::Failed(m) => Failure::YtDlp(m),
                ytdlp::Error::NothingFound => Failure::Empty,
                ytdlp::Error::TimedOut => Failure::YtDlp("timed out".into()),
                ytdlp::Error::Cancelled => Failure::Unreachable("cancelled".into()),
            },
        )?;
        if let Some(t) = &found.title {
            self.title = Some(t.clone());
            self.set(|s| s.title = Some(t.clone()));
        }
        let first = found.urls.into_iter().next().ok_or(Failure::Empty)?;
        if addr::guess(&first) == Some(Guess::Page) {
            return Err(Failure::Empty);
        }
        Ok(first)
    }

    /// A PLS or M3U radio playlist: its first network entry.
    fn playlist(&mut self, url: &str, text: &[u8]) -> Result<String, Failure> {
        let kind = specular_playlist::Kind::probe(text)
            .or_else(|| match addr::extension(url).as_deref() {
                Some("pls") => Some(specular_playlist::Kind::Pls),
                _ => Some(specular_playlist::Kind::M3u),
            })
            .ok_or(Failure::Empty)?;
        let pl = specular_playlist::read(text, kind, Path::new("/")).map_err(|_| Failure::Empty)?;
        let entry = pl
            .entries
            .iter()
            .find(|e| matches!(&e.location, specular_playlist::Location::Url(u) if addr::is_stream_url(u)))
            .ok_or(Failure::Empty)?;
        if let Some(t) = entry.title.as_ref().filter(|_| self.title.is_none()) {
            self.title = Some(t.clone());
            self.set(|s| s.title = Some(t.clone()));
        }
        match &entry.location {
            specular_playlist::Location::Url(u) => Ok(addr::resolve(url, u)),
            specular_playlist::Location::File(_) => Err(Failure::Empty),
        }
    }

    /// A text document (playlist, manifest).
    fn text(&self, url: &str) -> Result<(Vec<u8>, Option<String>), Failure> {
        let r = self
            .opts
            .client
            .get(&Request::get(url).limit(MAX_TEXT), &self.sh.cancel)
            .map_err(net_failure)?;
        let ct = r.headers.content_type().map(str::to_owned);
        Ok((r.body, ct))
    }

    /// Bytes of `url` (a byte range of it), counted towards the throughput.
    fn fetch(&mut self, url: &str, range: Option<(u64, u64)>) -> Result<Vec<u8>, Failure> {
        let mut req = Request::get(url).limit(MAX_SEGMENT);
        if let Some((o, n)) = range {
            req = req.range(o..o + n);
        }
        let t0 = Instant::now();
        let r = self
            .opts
            .client
            .get(&req, &self.sh.cancel)
            .map_err(net_failure)?;
        let mut body = r.body;
        if let Some((o, n)) = range
            && r.status == 200
        {
            // A server ignoring the range sent the whole resource.
            let o = o as usize;
            body = body
                .get(o..(o + n as usize).min(body.len()))
                .map(<[u8]>::to_vec)
                .unwrap_or_default();
        }
        let dt = t0.elapsed().as_secs_f64().max(1e-3);
        if body.len() > 32 * 1024 {
            let r = body.len() as f64 * 8.0 / dt;
            self.bps = Some(self.bps.map_or(r, |b| b + 0.3 * (r - b)));
        }
        Ok(body)
    }

    /// The first answer: a text document comes back; a file or a radio stream is fetched
    /// here (`None`).
    fn probe(&mut self, url: &str) -> Result<Option<(Guess, Vec<u8>)>, Failure> {
        let req = Request::get(url)
            .header("Icy-MetaData", "1")
            .body_timeout(None);
        let (opts, sh) = (self.opts, self.sh);
        let mut index = self.index;
        let head_seen: std::cell::RefCell<(usize, Option<String>, Option<u64>)> =
            std::cell::RefCell::new((0, None, None));
        let ranges = std::cell::Cell::new(false);
        let probe_cell = std::cell::RefCell::new(Probe::Undecided(Vec::new()));
        let mut text_kind = None;
        let mut fail: Option<Failure> = None;
        let head = opts.client.get_stream_head(
            &req,
            &sh.cancel,
            |h| {
                *probe_cell.borrow_mut() = Probe::Undecided(Vec::new());
                *head_seen.borrow_mut() = (
                    h.headers
                        .get("icy-metaint")
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0),
                    h.headers.content_type().map(str::to_owned),
                    h.headers.content_length(),
                );
                ranges.set(
                    h.status == 200
                        && h.headers
                            .get("accept-ranges")
                            .is_some_and(|v| v.trim().eq_ignore_ascii_case("bytes")),
                );
                if let Some(name) = h
                    .headers
                    .get("icy-name")
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                {
                    sh.st.lock().title = Some(name.to_owned());
                }
                ControlFlow::Continue(())
            },
            |chunk, p| {
                if sh.cancel.is_cancelled() {
                    return ControlFlow::Break(());
                }
                let (metaint, ct, total) = head_seen.borrow().clone();
                let mut probe = probe_cell.borrow_mut();
                let step = match &mut *probe {
                    Probe::Undecided(buf) => {
                        buf.extend_from_slice(chunk);
                        if buf.len() < 4096 && total.is_none_or(|t| (buf.len() as u64) < t) {
                            return ControlFlow::Continue(());
                        }
                        let first = std::mem::take(buf);
                        decide(&first, ct.as_deref(), metaint, total, url, opts).map(|d| (d, first))
                    }
                    Probe::Text(buf) => {
                        buf.extend_from_slice(chunk);
                        if buf.len() as u64 > MAX_TEXT {
                            Err(Failure::Empty)
                        } else {
                            return ControlFlow::Continue(());
                        }
                    }
                    Probe::Ranged { .. } => return ControlFlow::Break(()),
                    Probe::File { file, .. } => {
                        use std::io::Write;
                        sh.st.lock().received = p.received;
                        return match file.write_all(chunk) {
                            Ok(()) => ControlFlow::Continue(()),
                            Err(e) => {
                                fail = Some(Failure::Disk(e.to_string()));
                                ControlFlow::Break(())
                            }
                        };
                    }
                    Probe::Radio { demux, cutter } => {
                        return match radio_feed(sh, opts, &mut index, demux, cutter, chunk) {
                            Ok(()) => ControlFlow::Continue(()),
                            Err(f) => {
                                fail = Some(f);
                                ControlFlow::Break(())
                            }
                        };
                    }
                };
                match step {
                    Err(f) => {
                        fail = Some(f);
                        ControlFlow::Break(())
                    }
                    Ok((Decided::Text(g), first)) => {
                        text_kind = Some(g);
                        *probe = Probe::Text(first);
                        ControlFlow::Continue(())
                    }
                    Ok((Decided::Radio(framing), first)) => {
                        {
                            let mut st = sh.st.lock();
                            st.kind = Some(StreamKind::Radio);
                            st.live = true;
                        }
                        let mut demux = icy::Demux::new(metaint);
                        let mut cutter = Cutter::new(framing, opts.radio_piece_s);
                        let r = radio_feed(sh, opts, &mut index, &mut demux, &mut cutter, &first);
                        *probe = Probe::Radio { demux, cutter };
                        match r {
                            Ok(()) => ControlFlow::Continue(()),
                            Err(f) => {
                                fail = Some(f);
                                ControlFlow::Break(())
                            }
                        }
                    }
                    Ok((Decided::File(path), first))
                        if ranges.get()
                            && total.is_some_and(|t| t >= webfile::MIN_RANGED)
                            && (webfile::is_isobmff(&first) || mkvpieces::is_matroska(&first)) =>
                    {
                        *probe = Probe::Ranged { path, first };
                        ControlFlow::Break(())
                    }
                    Ok((Decided::File(path), first)) => {
                        use std::io::Write;
                        {
                            let mut st = sh.st.lock();
                            st.kind = Some(StreamKind::File);
                            st.total = total;
                            st.received = p.received;
                        }
                        let made = std::fs::File::create(&path).and_then(|mut f| {
                            f.write_all(&first)?;
                            Ok(f)
                        });
                        match made {
                            Ok(file) => {
                                *probe = Probe::File { file, path };
                                ControlFlow::Continue(())
                            }
                            Err(e) => {
                                fail = Some(Failure::Disk(e.to_string()));
                                ControlFlow::Break(())
                            }
                        }
                    }
                }
            },
        );
        self.index = index;
        if let Some(f) = fail {
            return Err(f);
        }
        if self.cancelled() {
            return Ok(None);
        }
        let head_result = head.map(|_| ());
        let (metaint, ct, total) = head_seen.into_inner();
        match probe_cell.into_inner() {
            Probe::Text(t) => {
                head_result.map_err(net_failure)?;
                Ok(Some((text_kind.unwrap_or(Guess::Media), t)))
            }
            Probe::Undecided(buf) if !buf.is_empty() => {
                head_result.map_err(net_failure)?;
                // A short answer, all of it read.
                match decide(
                    &buf,
                    ct.as_deref(),
                    metaint,
                    Some(buf.len() as u64),
                    url,
                    opts,
                )? {
                    Decided::Text(g) => Ok(Some((g, buf))),
                    Decided::Radio(_) | Decided::File(_) => {
                        let path = file_path(url, ct.as_deref(), opts);
                        std::fs::write(&path, &buf).map_err(|e| Failure::Disk(e.to_string()))?;
                        self.set(|s| {
                            s.kind = Some(StreamKind::File);
                            s.received = buf.len() as u64;
                            s.total = Some(buf.len() as u64);
                        });
                        self.file_done(path)?;
                        Ok(None)
                    }
                }
            }
            Probe::Undecided(_) => {
                head_result.map_err(net_failure)?;
                Err(Failure::Empty)
            }
            Probe::File { file, path } => {
                drop(file);
                head_result.map_err(net_failure)?;
                if total.is_some_and(|t| std::fs::metadata(&path).map_or(0, |m| m.len()) < t) {
                    return Err(Failure::Unreachable("the connection broke".into()));
                }
                self.file_done(path)?;
                Ok(None)
            }
            Probe::Radio { demux, cutter } => {
                self.radio(url, demux, cutter)?;
                Ok(None)
            }
            Probe::Ranged { path, first } => {
                let total = total.ok_or(Failure::Empty)?;
                if mkvpieces::is_matroska(&first) {
                    self.mkv_pieces(url, total)?;
                } else {
                    self.web_file(url, path, total, &first)?;
                }
                Ok(None)
            }
        }
    }

    fn file_done(&mut self, path: PathBuf) -> Result<(), Failure> {
        let index = self.index;
        self.index += 1;
        self.set(|s| {
            s.chunks.push(Chunk {
                path,
                index,
                start: 0.0,
                seconds: 0.0,
                title: None,
            });
        });
        self.sh.changed();
        Ok(())
    }

    /// Reads on from an endless sound stream after its connection ended: connects again
    /// (a few times in a row at most) until stopped.
    fn radio(&mut self, url: &str, demux: icy::Demux, mut cutter: Cutter) -> Result<(), Failure> {
        let demux = std::cell::RefCell::new(demux);
        let mut drops = 0;
        loop {
            if self.cancelled() {
                return Ok(());
            }
            drops += 1;
            if drops > 3 {
                if let Some(p) = cutter.finish() {
                    let mut index = self.index;
                    emit_to(
                        self.sh,
                        self.opts,
                        &mut index,
                        cutter.framing().extension(),
                        p.data,
                        None,
                        p.seconds,
                    )?;
                    self.index = index;
                }
                return Err(Failure::Unreachable("the stream ended".into()));
            }
            specular_remote::http::sleep_cancellable(Duration::from_millis(500), &self.sh.cancel)
                .map_err(|_| Failure::Unreachable("cancelled".into()))?;
            let req = Request::get(url)
                .header("Icy-MetaData", "1")
                .body_timeout(None);
            let (opts, sh) = (self.opts, self.sh);
            let mut index = self.index;
            let mut fail = None;
            let started = Instant::now();
            let r = opts.client.get_stream_head(
                &req,
                &sh.cancel,
                |h| {
                    let mi = h
                        .headers
                        .get("icy-metaint")
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0usize);
                    *demux.borrow_mut() = icy::Demux::new(mi);
                    ControlFlow::Continue(())
                },
                |chunk, _| {
                    if sh.cancel.is_cancelled() {
                        return ControlFlow::Break(());
                    }
                    match radio_feed(
                        sh,
                        opts,
                        &mut index,
                        &mut demux.borrow_mut(),
                        &mut cutter,
                        chunk,
                    ) {
                        Ok(()) => ControlFlow::Continue(()),
                        Err(f) => {
                            fail = Some(f);
                            ControlFlow::Break(())
                        }
                    }
                },
            );
            self.index = index;
            if let Some(f) = fail {
                return Err(f);
            }
            if let Err(e) = r
                && e.is_network_off()
            {
                return Err(Failure::NetworkOff);
            }
            if started.elapsed() > Duration::from_secs(30) {
                drops = 0;
            }
        }
    }

    /// Waits while enough is fetched ahead of the piece playing (finite streams).
    fn wait_room(&self) -> Result<(), Failure> {
        let mut ctl = self.sh.ctl.lock();
        loop {
            if self.cancelled() {
                return Err(Failure::Unreachable("cancelled".into()));
            }
            let playing = ctl.playing.unwrap_or(0);
            let ahead: f64 = self
                .sh
                .st
                .lock()
                .chunks
                .iter()
                .filter(|c| c.index >= playing)
                .map(|c| c.seconds)
                .sum();
            if ahead < self.opts.ahead_s || ctl.seek.is_some() {
                return Ok(());
            }
            self.sh.wake.wait_for(&mut ctl, Duration::from_millis(250));
        }
    }

    /// Writes a piece starting `start` seconds into the stream and offers it.
    fn emit(&mut self, ext: &str, data: Vec<u8>, start: f64, seconds: f64) -> Result<(), Failure> {
        let mut index = self.index;
        let r = emit_to(
            self.sh,
            self.opts,
            &mut index,
            ext,
            data,
            Some(start),
            seconds,
        );
        self.index = index;
        r
    }

    /// Every piece of a finite stream made: says so (no more pieces come, [`Status::ended`])
    /// and waits for a seek, which the caller then answers. `Err` when stopped.
    fn idle_until_seek(&mut self) -> Result<(), Failure> {
        self.set(|s| s.ended = true);
        {
            let mut ctl = self.sh.ctl.lock();
            while ctl.seek.is_none() {
                if self.cancelled() {
                    return Err(Failure::Unreachable("cancelled".into()));
                }
                self.sh.wake.wait_for(&mut ctl, Duration::from_millis(250));
            }
        }
        self.set(|s| s.ended = false);
        Ok(())
    }

    /// A seek asked for (finite streams): the pieces after the one playing are dropped and
    /// the next piece made answers it. Its time, then.
    fn take_seek(&mut self) -> Option<f64> {
        let t = self.sh.ctl.lock().seek.take()?;
        let playing = self.sh.ctl.lock().playing;
        let gone: Vec<PathBuf> = {
            let mut st = self.sh.st.lock();
            let (gone, keep): (Vec<Chunk>, Vec<Chunk>) = st
                .chunks
                .drain(..)
                .partition(|c| playing.is_none_or(|p| c.index > p));
            st.chunks = keep;
            st.seek = Some((t, self.index));
            gone.into_iter().map(|c| c.path).collect()
        };
        for p in gone {
            let _ = std::fs::remove_file(p);
        }
        self.sh.changed();
        Some(t)
    }

    /// The variant asked for now, if the user or the throughput changed it.
    fn want_variant(&self, pick: impl Fn(Option<f64>) -> usize) -> usize {
        match self.sh.ctl.lock().variant {
            Some(Some(v)) => v,
            _ => pick(self.bps),
        }
    }

    fn key(&mut self, uri: &str) -> Result<Vec<u8>, Failure> {
        if let Some(k) = self.keys.get(uri) {
            return Ok(k.clone());
        }
        let k = self.fetch(uri, None)?;
        if k.len() != 16 {
            return Err(Failure::Unsupported("the key is not an AES-128 key".into()));
        }
        self.keys.insert(uri.to_owned(), k.clone());
        Ok(k)
    }

    fn init(&mut self, url: &str, range: Option<(u64, u64)>) -> Result<Arc<Vec<u8>>, Failure> {
        let k = (url.to_owned(), range);
        if let Some(i) = self.inits.get(&k) {
            return Ok(Arc::clone(i));
        }
        let b = Arc::new(self.fetch(url, range)?);
        self.inits.insert(k, Arc::clone(&b));
        Ok(b)
    }

    /// A segment's bytes, decrypted.
    fn hls_segment(&mut self, s: &hls::Segment) -> Result<Vec<u8>, Failure> {
        let mut data = self.fetch(&s.uri, s.range)?;
        if s.key.method == hls::Method::Aes128 {
            let uri = s.key.uri.clone().ok_or(Failure::Encrypted)?;
            let key = self.key(&uri)?;
            let iv = s.key.iv.unwrap_or_else(|| hls::default_iv(s.sequence));
            let n = crate::aes128::decrypt(&mut data, &key, &iv)
                .map_err(|e| Failure::Unsupported(e.to_string()))?;
            data.truncate(n);
        }
        Ok(data)
    }

    fn media_playlist(&mut self, url: &str) -> Result<hls::Media, Failure> {
        let (text, _) = self.text(url)?;
        match hls::parse(&String::from_utf8_lossy(&text), url) {
            Ok(hls::Playlist::Media(m)) => {
                match m.refusal() {
                    Some(hls::Refusal::Drm) => return Err(Failure::Drm),
                    Some(_) => return Err(Failure::Encrypted),
                    None => {}
                }
                Ok(m)
            }
            Ok(hls::Playlist::Master(_)) => Err(Failure::Unsupported(
                "a master playlist inside a master playlist".into(),
            )),
            Err(_) => Err(Failure::Empty),
        }
    }

    fn hls(&mut self, url: &str, text: &[u8]) -> Result<(), Failure> {
        self.set(|s| s.kind = Some(StreamKind::Hls));
        let pl = hls::parse(&String::from_utf8_lossy(text), url).map_err(|_| Failure::Empty)?;
        let (master, order) = match pl {
            hls::Playlist::Master(m) => {
                if m.refusal().is_some() {
                    return Err(Failure::Drm);
                }
                let order = m.by_bandwidth();
                let labels = order
                    .iter()
                    .map(|&i| {
                        let v = &m.variants[i];
                        let rate = dash::rate_label(v.bandwidth);
                        match v.resolution {
                            Some((w, h)) => format!("{w}×{h}, {rate}"),
                            None => rate,
                        }
                    })
                    .collect();
                self.set(|s| s.variants = labels);
                (Some(m), order)
            }
            hls::Playlist::Media(_) => (None, vec![0]),
        };
        // The variant as an index into `order` (the menu's order).
        let pick = |m: &Option<hls::Master>, bps: Option<f64>| -> usize {
            m.as_ref().map_or(0, |m| {
                let i = m.pick(bps, 0.7);
                order.iter().position(|&o| o == i).unwrap_or(0)
            })
        };
        let mut cur = self.want_variant(|b| pick(&master, b)).min(order.len() - 1);
        let media_url = |v: usize| -> String {
            master
                .as_ref()
                .map_or_else(|| url.to_owned(), |m| m.variants[order[v]].uri.clone())
        };
        let audio_url =
            |v: usize| -> Option<String> { master.as_ref()?.audio_for(order[v])?.uri.clone() };
        let mut media = match &master {
            Some(_) => self.media_playlist(&media_url(cur))?,
            None => match hls::parse(&String::from_utf8_lossy(text), url) {
                Ok(hls::Playlist::Media(m)) => {
                    match m.refusal() {
                        Some(hls::Refusal::Drm) => return Err(Failure::Drm),
                        Some(_) => return Err(Failure::Encrypted),
                        None => {}
                    }
                    m
                }
                _ => return Err(Failure::Empty),
            },
        };
        let mut audio = match audio_url(cur) {
            Some(u) => Some((u.clone(), self.media_playlist(&u)?)),
            None => None,
        };
        let live = media.live();
        self.set(|s| {
            s.live = live;
            s.duration = (!live).then(|| media.duration());
            s.variant = Some(cur);
        });
        // Where to go on: a live stream is joined three segments from its end.
        let mut next_seq = if live {
            media
                .segments
                .len()
                .checked_sub(3)
                .map_or(media.media_sequence, |i| media.segments[i].sequence)
        } else {
            media.media_sequence
        };
        // Seconds from the start, for finding the same place in another variant.
        let mut at_time = 0.0f64;
        let mut first = true;
        let mut last_reload = Instant::now();
        loop {
            if !live {
                self.wait_room()?;
                if let Some(t) = self.take_seek() {
                    next_seq = seq_at(&media, t);
                    at_time = seq_start(&media, next_seq);
                    first = true;
                }
            }
            if self.cancelled() {
                return Ok(());
            }
            // A change of variant applies from this piece.
            let want = self.want_variant(|b| pick(&master, b)).min(order.len() - 1);
            if want != cur && master.is_some() {
                let m = self.media_playlist(&media_url(want))?;
                if !live {
                    next_seq = seq_at(&m, at_time);
                }
                media = m;
                audio = match audio_url(want) {
                    Some(u) => Some((u.clone(), self.media_playlist(&u)?)),
                    None => None,
                };
                cur = want;
                self.set(|s| s.variant = Some(cur));
            }
            // The segments of the next piece.
            let target = if first { 0.1 } else { self.opts.piece_s };
            let mut segs = Vec::new();
            let mut secs = 0.0;
            while secs < target {
                match media.segments.iter().find(|s| s.sequence == next_seq) {
                    Some(s) => {
                        if !segs.is_empty() && s.discontinuity {
                            break;
                        }
                        secs += s.duration;
                        segs.push(s.clone());
                        next_seq += 1;
                    }
                    None if !live => break,
                    None => {
                        if !segs.is_empty() {
                            break;
                        }
                        // Wait for the playlist to grow.
                        let wait =
                            Duration::from_secs_f64((media.target_duration / 2.0).clamp(0.2, 5.0))
                                .saturating_sub(last_reload.elapsed());
                        specular_remote::http::sleep_cancellable(wait, &self.sh.cancel)
                            .map_err(|_| Failure::Unreachable("cancelled".into()))?;
                        last_reload = Instant::now();
                        media = self.media_playlist(&media_url(cur))?;
                        if let Some((u, _)) = &audio {
                            let u = u.clone();
                            audio = Some((u.clone(), self.media_playlist(&u)?));
                        }
                        if media
                            .segments
                            .first()
                            .is_some_and(|s| s.sequence > next_seq)
                        {
                            // Fell behind the window: join it again.
                            next_seq =
                                media.segments[media.segments.len().saturating_sub(3)].sequence;
                        }
                        if !media.live() && media.segments.iter().all(|s| s.sequence < next_seq) {
                            return Ok(());
                        }
                    }
                }
            }
            if segs.is_empty() {
                return Ok(());
            }
            let piece_start = at_time;
            at_time += secs;
            first = false;
            // The sound rendition's segments that start within the piece.
            let mut asegs = Vec::new();
            // The last piece of a finite stream takes the sound's rest (it may run longer).
            let last_piece = !live && media.segments.last().is_none_or(|l| l.sequence < next_seq);
            if let Some((_, am)) = &audio {
                let mut t = 0.0;
                for s in &am.segments {
                    let in_piece = if live {
                        s.sequence >= segs[0].sequence
                            && s.sequence <= segs[segs.len() - 1].sequence
                    } else {
                        t + 1e-6 >= piece_start && (last_piece || t + 1e-6 < piece_start + secs)
                    };
                    if in_piece {
                        asegs.push(s.clone());
                    }
                    t += s.duration;
                }
            }
            let mut bodies = Vec::new();
            for s in &segs {
                bodies.push(self.hls_segment(s)?);
            }
            let fragmented =
                segs[0].map.is_some() || bodies.first().is_some_and(|b| fmp4::sniff(b));
            let (ext, data) = if fragmented {
                let mut d = Defrag::new();
                if let Some(m) = &segs[0].map {
                    let init = self.init(&m.uri, m.range)?;
                    d.add_init(0, &init).map_err(defrag_failure)?;
                } else {
                    d.add_init(0, &bodies[0]).map_err(defrag_failure)?;
                }
                for b in &bodies {
                    d.add_fragment(0, b).map_err(defrag_failure)?;
                }
                if let Some(first_a) = asegs.first().cloned() {
                    let fmp4_audio = first_a.map.is_some();
                    if fmp4_audio {
                        let m = first_a.map.clone().unwrap_or(hls::Map {
                            uri: String::new(),
                            range: None,
                        });
                        let init = self.init(&m.uri, m.range)?;
                        d.add_init(1, &init).map_err(defrag_failure)?;
                        for s in &asegs {
                            let b = self.hls_segment(s)?;
                            d.add_fragment(1, &b).map_err(defrag_failure)?;
                        }
                    }
                }
                ("mp4", d.build())
            } else {
                let mut out = Vec::new();
                let mut ext = "ts";
                for b in &bodies {
                    if b.first() == Some(&0x47) {
                        out.extend_from_slice(b);
                    } else {
                        // Packed sound (ADTS, MP3) with an ID3 timestamp in front.
                        let start = id3_end(b);
                        ext = match Framing::sniff(&b[start..]) {
                            Some(f) => f.extension(),
                            None => "bin",
                        };
                        out.extend_from_slice(&b[start..]);
                    }
                }
                // A sound rendition of its own (MPEG-TS or packed sound): added to the
                // piece when the variant's segments have no sound.
                if ext == "ts" && asegs.first().is_some_and(|a| a.map.is_none()) {
                    let mut sound = Vec::new();
                    for s in &asegs {
                        sound.push(self.hls_segment(s)?);
                    }
                    if let Some(merged) = crate::tsmux::add_sound(&out, &sound) {
                        out = merged;
                    }
                }
                (ext, out)
            };
            self.emit(ext, data, piece_start, secs)?;
        }
    }

    fn dash(&mut self, url: &str, text: &[u8]) -> Result<(), Failure> {
        self.set(|s| s.kind = Some(StreamKind::Dash));
        let mut mpd =
            dash::parse(&String::from_utf8_lossy(text), url).map_err(|_| Failure::Empty)?;
        if mpd.protected() {
            return Err(Failure::Drm);
        }
        let live = mpd.dynamic;
        self.set(|s| {
            s.live = live;
            s.duration = (!live).then_some(mpd.duration).flatten();
        });
        let now = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64())
        };
        let mut period_i = if live {
            mpd.periods.len().saturating_sub(1)
        } else {
            0
        };
        let mut at_time = 0.0f64;
        // Live: the next segment number of each set.
        let mut next_num: [Option<u64>; 2] = [None, None];
        let mut first = true;
        let mut last_refresh = Instant::now();
        // A seek into another period, carried into it.
        let mut seek_after: Option<f64> = None;
        loop {
            let Some(period) = mpd.periods.get(period_i).cloned() else {
                return Ok(());
            };
            let video = period.set(dash::Kind::Video).cloned();
            let sound = period.set(dash::Kind::Audio).cloned();
            if video.is_none() && sound.is_none() {
                return Err(Failure::Empty);
            }
            for set in [&video, &sound].into_iter().flatten() {
                if set
                    .reps
                    .iter()
                    .any(|r| r.mime.as_deref().is_some_and(|m| m.contains("webm")))
                {
                    return Err(Failure::Unsupported("WebM in DASH".into()));
                }
            }
            let main = video.as_ref().or(sound.as_ref()).ok_or(Failure::Empty)?;
            let order = main.by_bandwidth();
            let labels: Vec<String> = order.iter().map(|&i| main.reps[i].label()).collect();
            self.set(|s| s.variants = labels);
            let cur = self
                .want_variant(|b| {
                    let i = main.pick(b.map(|b| b * 0.7));
                    order.iter().position(|&o| o == i).unwrap_or(0)
                })
                .min(order.len() - 1);
            self.set(|s| s.variant = Some(cur));
            let main_rep = main.reps[order[cur]].clone();
            let sound_rep = if video.is_some() {
                sound
                    .as_ref()
                    .map(|s| s.reps[s.pick(Some(256_000.0))].clone())
            } else {
                None
            };
            // A seek: the period holding it, and the segment holding it in there.
            let mut seek_in: Option<f64> = None;
            if !live {
                self.wait_room()?;
                if let Some(t) = self.take_seek() {
                    let i = mpd
                        .periods
                        .iter()
                        .rposition(|p| p.start <= t + 1e-6)
                        .unwrap_or(0);
                    first = true;
                    if i != period_i {
                        period_i = i;
                        at_time = (t - mpd.periods[i].start).max(0.0);
                        seek_after = Some(at_time);
                        continue;
                    }
                    at_time = (t - period.start).max(0.0);
                    seek_in = Some(at_time);
                }
            }
            if let Some(t) = seek_after.take() {
                seek_in = Some(t);
            }
            if self.cancelled() {
                return Ok(());
            }
            let t = now();
            let mut reps: Vec<(usize, dash::Rep, Vec<dash::SegRef>)> = Vec::new();
            for (k, rep) in [Some(main_rep), sound_rep].into_iter().enumerate() {
                let Some(rep) = rep else { continue };
                let segs = match &rep.addressing {
                    dash::Addressing::Base {
                        index: Some((o, n)),
                        ..
                    } => {
                        let idx = self.init(&rep.base, Some((*o, *n)))?;
                        let subs = dash::parse_sidx(&idx, *o).ok_or(Failure::Empty)?;
                        let mut start = 0.0;
                        subs.into_iter()
                            .enumerate()
                            .map(|(i, (off, len, d))| {
                                let s = dash::SegRef {
                                    url: rep.base.clone(),
                                    range: Some((off, len)),
                                    start,
                                    duration: d,
                                    number: i as u64,
                                };
                                start += d;
                                s
                            })
                            .collect()
                    }
                    _ => rep.segments(&mpd, &period, t),
                };
                reps.push((k, rep, segs));
            }
            // A seek lands on the start of the segment holding its time.
            if let Some(ts) = seek_in
                && let Some((_, _, segs)) = reps.first()
                && let Some(sg) = segs.iter().rev().find(|s| s.start <= ts + 1e-6)
            {
                at_time = sg.start;
            }
            // The piece: each set's segments starting within it.
            let target = if first { 0.1 } else { self.opts.piece_s };
            let mut d = Defrag::new();
            let mut secs = 0.0f64;
            let mut any = false;
            let mut piece_end: Option<f64> = None;
            for (k, rep, segs) in &reps {
                let pick: Vec<dash::SegRef> = if live {
                    let from = next_num[*k]
                        .unwrap_or_else(|| segs.len().checked_sub(3).map_or(0, |i| segs[i].number));
                    let mut v = Vec::new();
                    let mut acc = 0.0;
                    for s in segs.iter().filter(|s| s.number >= from) {
                        if acc >= target && !v.is_empty() {
                            break;
                        }
                        acc += s.duration;
                        v.push(s.clone());
                    }
                    if let Some(l) = v.last() {
                        next_num[*k] = Some(l.number + 1);
                    }
                    v
                } else if *k == 0 {
                    let mut v = Vec::new();
                    let mut acc = 0.0;
                    for s in segs.iter().filter(|s| s.start + 1e-6 >= at_time) {
                        if acc >= target && !v.is_empty() {
                            break;
                        }
                        acc += s.duration;
                        v.push(s.clone());
                    }
                    piece_end = v.last().map(|s| s.start + s.duration);
                    v
                } else {
                    let end = piece_end.unwrap_or(at_time + target);
                    // After a seek, also the sound segment already running at the start.
                    let from = |s: &dash::SegRef| {
                        s.start + 1e-6 >= at_time
                            || seek_in.is_some() && s.start + s.duration > at_time + 1e-6
                    };
                    segs.iter()
                        .filter(|s| from(s) && s.start + 1e-6 < end)
                        .cloned()
                        .collect()
                };
                if pick.is_empty() {
                    continue;
                }
                if *k == 0 {
                    secs = pick.iter().map(|s| s.duration).sum();
                }
                let init = match rep.init() {
                    Some(i) => Some(self.init(&i.url, i.range)?),
                    None => None,
                };
                for (j, s) in pick.iter().enumerate() {
                    let b = self.fetch(&s.url, s.range)?;
                    if j == 0 {
                        match &init {
                            Some(i) => d.add_init(*k, i).map_err(defrag_failure)?,
                            None => d.add_init(*k, &b).map_err(defrag_failure)?,
                        }
                    }
                    d.add_fragment(*k, &b).map_err(defrag_failure)?;
                    any = true;
                }
            }
            first = false;
            if any && !d.is_empty() {
                if secs <= 0.0 {
                    secs = d.duration();
                }
                let start = if live {
                    None
                } else {
                    Some(period.start + at_time)
                };
                at_time += secs;
                let mut index = self.index;
                emit_to(
                    self.sh,
                    self.opts,
                    &mut index,
                    "mp4",
                    d.build(),
                    start,
                    secs,
                )?;
                self.index = index;
                continue;
            }
            if live {
                // Wait for new segments, reading the manifest again.
                let every = mpd.min_update.unwrap_or(2.0).clamp(0.5, 10.0);
                let wait = Duration::from_secs_f64(every).saturating_sub(last_refresh.elapsed());
                specular_remote::http::sleep_cancellable(
                    wait.max(Duration::from_millis(200)),
                    &self.sh.cancel,
                )
                .map_err(|_| Failure::Unreachable("cancelled".into()))?;
                last_refresh = Instant::now();
                let (text, _) = self.text(url)?;
                mpd = dash::parse(&String::from_utf8_lossy(&text), url)
                    .map_err(|_| Failure::Empty)?;
                if mpd.protected() {
                    return Err(Failure::Drm);
                }
                period_i = mpd.periods.len().saturating_sub(1);
                continue;
            }
            // The period is done: on to the next.
            period_i += 1;
            at_time = 0.0;
        }
    }
}

fn defrag_failure(e: fmp4::Error) -> Failure {
    match e {
        fmp4::Error::Encrypted => Failure::Drm,
        e => Failure::Unsupported(e.to_string()),
    }
}

fn note_titles(sh: &Shared, titles: &[String]) {
    for t in titles {
        if let Some(title) = icy::stream_title(t) {
            sh.st.lock().now_playing = Some(title);
            sh.changed();
        }
    }
}

/// The end of an ID3v2 tag at the start of `b` (0 without one).
fn id3_end(b: &[u8]) -> usize {
    if b.len() >= 10 && &b[..3] == b"ID3" {
        let n = b[6..10]
            .iter()
            .fold(0usize, |a, &x| (a << 7) | usize::from(x & 0x7F));
        (10 + n).min(b.len())
    } else {
        0
    }
}

/// The sequence number of the segment playing at `t` seconds.
fn seq_at(m: &hls::Media, t: f64) -> u64 {
    let mut acc = 0.0;
    for s in &m.segments {
        if acc + s.duration > t + 1e-6 {
            return s.sequence;
        }
        acc += s.duration;
    }
    m.segments
        .last()
        .map_or(m.media_sequence, |s| s.sequence + 1)
}

/// Seconds from the playlist's start to segment `seq`.
fn seq_start(m: &hls::Media, seq: u64) -> f64 {
    m.segments
        .iter()
        .take_while(|s| s.sequence < seq)
        .map(|s| s.duration)
        .sum()
}

/// What the first bytes of an answer are.
enum Decided {
    Text(Guess),
    Radio(Framing),
    File(PathBuf),
}

fn decide(
    first: &[u8],
    ct: Option<&str>,
    metaint: usize,
    total: Option<u64>,
    url: &str,
    opts: &Options,
) -> Result<Decided, Failure> {
    let by_ct = ct.and_then(addr::from_content_type);
    let text = match by_ct {
        Some(g @ (Guess::Hls | Guess::Dash | Guess::Playlist | Guess::Page)) => {
            // An `.m3u` served as a playlist is HLS by its content.
            Some(
                addr::sniff(first)
                    .filter(|s| *s != Guess::Media)
                    .unwrap_or(g),
            )
        }
        _ if metaint > 0 => None,
        _ => addr::sniff(first),
    };
    if let Some(g) = text {
        return Ok(Decided::Text(g));
    }
    let mut demux = icy::Demux::new(metaint);
    let mut sound = Vec::new();
    let _ = demux.feed(first, &mut sound);
    let framing = ct
        .and_then(Framing::of_content_type)
        .or_else(|| Framing::sniff(&sound));
    let endless = metaint > 0 || total.is_none();
    match framing {
        Some(f) if endless => Ok(Decided::Radio(f)),
        None if metaint > 0 => Err(Failure::Unsupported(
            "a sound stream in an unknown format".into(),
        )),
        _ => Ok(Decided::File(file_path(url, ct, opts))),
    }
}

/// Where a whole file goes: its own name when it has a media extension, else a name by its
/// `Content-Type`.
fn file_path(url: &str, ct: Option<&str>, opts: &Options) -> PathBuf {
    if let (Some(name), Some(ext)) = (addr::file_name(url), addr::extension(url))
        && addr::is_media_ext(&ext)
        && !name.starts_with('.')
    {
        return opts.dir.join(name);
    }
    let ct = ct
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let ext = match ct.as_str() {
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/aac" | "audio/aacp" | "audio/x-aac" => "aac",
        "audio/mp4" | "audio/x-m4a" => "m4a",
        "audio/ogg" | "application/ogg" => "ogg",
        "audio/opus" => "opus",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "video/mp4" => "mp4",
        "video/quicktime" => "mov",
        "video/webm" | "audio/webm" => "webm",
        "video/x-matroska" | "audio/x-matroska" => "mkv",
        "video/mp2t" => "ts",
        "video/x-msvideo" => "avi",
        "video/x-flv" => "flv",
        _ => "bin",
    };
    opts.dir.join(format!("stream.{ext}"))
}

/// Takes radio bytes: titles noted, pieces written.
fn radio_feed(
    sh: &Shared,
    opts: &Options,
    index: &mut u32,
    demux: &mut icy::Demux,
    cutter: &mut Cutter,
    chunk: &[u8],
) -> Result<(), Failure> {
    let mut sound = Vec::new();
    let titles = demux.feed(chunk, &mut sound);
    note_titles(sh, &titles);
    for p in cutter.push(&sound) {
        // A listener far behind (paused for long): pieces past ten minutes are dropped.
        let ahead: f64 = {
            let playing = sh.ctl.lock().playing.unwrap_or(0);
            sh.st
                .lock()
                .chunks
                .iter()
                .filter(|c| c.index >= playing)
                .map(|c| c.seconds)
                .sum()
        };
        if ahead > 600.0 {
            continue;
        }
        emit_to(
            sh,
            opts,
            index,
            cutter.framing().extension(),
            p.data,
            None,
            p.seconds,
        )?;
    }
    Ok(())
}

/// Writes piece `*index` and offers it.
fn emit_to(
    sh: &Shared,
    opts: &Options,
    index: &mut u32,
    ext: &str,
    data: Vec<u8>,
    start: Option<f64>,
    seconds: f64,
) -> Result<(), Failure> {
    let i = *index;
    let path = opts.dir.join(format!("p{i:06}.{ext}"));
    let tmp = opts.dir.join(format!(".p{i:06}.part"));
    std::fs::write(&tmp, &data)
        .and_then(|()| std::fs::rename(&tmp, &path))
        .map_err(|e| Failure::Disk(e.to_string()))?;
    *index += 1;
    let mut st = sh.st.lock();
    let start = start.unwrap_or_else(|| st.chunks.last().map_or(0.0, |c| c.start + c.seconds));
    let title = st.now_playing.clone();
    st.chunks.push(Chunk {
        path,
        index: i,
        start,
        seconds,
        title,
    });
    drop(st);
    sh.changed();
    Ok(())
}
