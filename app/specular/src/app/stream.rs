//! Network streams (`specular-stream`): File › Open network stream… (Ctrl+N), a pasted
//! address, the Open address dialog for media addresses, and a play queue's web items.
//!
//! - The worker fetches into a folder of the cache: a whole file, or pieces of a few seconds
//!   (radio, HLS, DASH). The first piece (or the file) opens like any file; each next piece
//!   opens when the one before ends, and the sound splices into it without a gap
//!   (`sound_fx.rs` primes it through [`App::stream_next_for`]).
//! - Going to anything else stops the stream; its files are deleted.
//! - The download indicator shows every request ("Stream from <host>"); with the network off
//!   nothing is fetched and the dialog says so.
//! - The recent streams (address without user, password or query; and the title) are kept in
//!   the data folder and listed in the dialog; forgetting one asks first.
//! - Quality: automatic by measured throughput, or a variant the user picks (Stream quality,
//!   in the palette and the cheat sheet, and in the player's More menu); a change applies
//!   from the next piece.
//! - A web file whose server takes byte ranges opens once its index is fetched and seeks
//!   anywhere (reads of parts not there yet wait for them: "Buffering…"). A Matroska or
//!   WebM one plays in pieces cut at its clusters, seekable as HLS is.
//! - HLS and DASH (not live): the seek bar is the whole stream's; a seek outside the piece
//!   playing opens the piece holding the time and seeks in it. The next piece is opened
//!   in the background before the one playing ends, so the picture runs on.
//! - Subtitle tracks a piece carries (a Matroska or WebM file's) span the whole stream: each
//!   piece's packets are kept once, on the stream's time line, and every piece shows them
//!   all ([`PieceSubs`]), so a cue that began in an earlier piece goes on showing.

use std::path::{Path, PathBuf};

use specular_core::net::What;
use specular_i18n::t;
use specular_stream::{Chunk, Failure, Options, Status, Stream, StreamKind, addr};
use specular_widgets::Playback;

use super::App;
use specular_core::Priority;

/// Recent streams kept.
pub(super) const MAX_RECENT: usize = 12;
/// The confirmation of forgetting a recent stream.
pub(super) const FORGET_ITEM: u64 = super::items::STREAM_FORGET;

/// A recent stream: its address as kept (no secrets) and its title when one was known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Recent {
    pub url: String,
    pub title: Option<String>,
}

/// The list from its file: `address<TAB>title` a line, cleaned again on reading.
pub(super) fn parse_recent(text: &str) -> Vec<Recent> {
    let mut out = Vec::new();
    for l in text.lines().rev() {
        let (u, t) = l.split_once('\t').unwrap_or((l, ""));
        let url = addr::kept(u);
        if !addr::is_stream_url(&url) {
            continue;
        }
        let t = t.trim();
        push_recent(
            &mut out,
            Recent {
                url,
                title: (!t.is_empty()).then(|| t.to_owned()),
            },
        );
    }
    out
}

/// The file's text.
pub(super) fn recent_text(list: &[Recent]) -> String {
    list.iter()
        .map(|r| {
            let t = r
                .title
                .as_deref()
                .unwrap_or("")
                .replace(['\t', '\n', '\r'], " ");
            format!("{}\t{t}\n", r.url)
        })
        .collect()
}

/// `r` first in `list`, once, at most [`MAX_RECENT`].
pub(super) fn push_recent(list: &mut Vec<Recent>, r: Recent) {
    if r.url.is_empty() {
        return;
    }
    list.retain(|e| e.url != r.url);
    list.insert(0, r);
    list.truncate(MAX_RECENT);
}

/// How the list shows an entry: its host and title, else its host and path.
pub(super) fn shown(r: &Recent) -> String {
    let host = specular_core::net::host_of(&r.url);
    match &r.title {
        Some(t) => format!("{host} — {t}"),
        None => r
            .url
            .split_once("://")
            .map_or(r.url.as_str(), |x| x.1)
            .to_owned(),
    }
}

pub(super) fn recent_file() -> PathBuf {
    specular_settings::Locations::detect()
        .data_dir
        .join("recent-streams.txt")
}

pub(super) fn read_recent() -> Vec<Recent> {
    std::fs::read_to_string(recent_file())
        .map(|t| parse_recent(&t))
        .unwrap_or_default()
}

/// Whether an address typed or pasted is one the stream layer opens without being told:
/// a media file, a playlist, HLS or DASH by its extension.
pub(super) fn looks_like_stream(url: &str) -> bool {
    crate::clipboard::is_stream_address(url)
}

/// What to say when a stream stopped.
pub(super) fn failure_text(f: &Failure) -> String {
    match f {
        Failure::NetworkOff => t!("stream-network-off"),
        Failure::Drm => t!("stream-drm"),
        Failure::Encrypted => t!("stream-encrypted"),
        Failure::Page { installed: true } => t!("stream-page-ytdlp-off"),
        Failure::Page { installed: false } => t!("stream-page"),
        Failure::YtDlp(m) => t!("stream-ytdlp-failed", reason = m.clone()),
        Failure::Status(s) => t!("stream-status", status = i64::from(*s)),
        Failure::Unreachable(m) => t!("stream-unreachable", reason = m.clone()),
        Failure::Empty => t!("stream-empty"),
        Failure::Unsupported(m) => t!("stream-unsupported", reason = m.clone()),
        Failure::Disk(m) => t!("stream-disk", reason = m.clone()),
    }
}

/// The yt-dlp to use: the path set, else the one on `PATH`; `None` while turned off.
pub(super) fn ytdlp_for(on: bool, path: Option<&Path>) -> Option<PathBuf> {
    if !on {
        return None;
    }
    match path {
        Some(p) if !p.as_os_str().is_empty() => Some(p.to_path_buf()),
        _ => specular_stream::ytdlp::find(),
    }
}

/// The setting the yt-dlp panel sits under.
pub(super) const YTDLP_PANEL: &str = "privacy.ytdlp";

/// What the panel under the yt-dlp setting says.
pub(super) fn ytdlp_panel(found: Option<PathBuf>) -> specular_widgets::SettingPanel {
    let label = match found {
        Some(p) => t!(
            "stream-ytdlp-found",
            path = specular_i18n::isolate_ltr(&p.display().to_string())
        ),
        None => t!("stream-ytdlp-missing"),
    };
    specular_widgets::SettingPanel {
        title: t!("stream-ytdlp-title"),
        rows: vec![specular_widgets::PanelRow {
            id: "found".into(),
            label,
            ..specular_widgets::PanelRow::default()
        }],
        keywords: "yt-dlp youtube video page stream".into(),
        ..specular_widgets::SettingPanel::default()
    }
}

/// Subtitle packets of a stream's pieces on the stream's time line (µs), each piece's
/// added once (by its start).
#[derive(Default)]
pub(super) struct PieceSubs {
    seen: Vec<i64>,
    tracks: Vec<Vec<specular_video::extras::Packet>>,
}

impl PieceSubs {
    /// Adds the subtitle packets of the piece starting `start_us` into the stream (`x` as
    /// read from the piece, its times from the piece's start) and gives `x` back with every
    /// piece's packets so far, on that piece's time line (those before it at negative
    /// times).
    pub(super) fn merge(
        &mut self,
        start_us: i64,
        mut x: specular_video::extras::Extras,
    ) -> specular_video::extras::Extras {
        if !self.seen.contains(&start_us) {
            self.seen.push(start_us);
            if self.tracks.len() < x.subtitles.len() {
                self.tracks.resize_with(x.subtitles.len(), Vec::new);
            }
            for (acc, sub) in self.tracks.iter_mut().zip(&x.subtitles) {
                for p in &sub.packets {
                    let q = specular_video::extras::Packet {
                        start_us: p.start_us + start_us,
                        dur_us: p.dur_us,
                        data: p.data.clone(),
                    };
                    if !acc.contains(&q) {
                        acc.push(q);
                    }
                }
                acc.sort_by_key(|p| p.start_us);
            }
        }
        for (sub, acc) in x.subtitles.iter_mut().zip(&self.tracks) {
            sub.packets = acc
                .iter()
                .map(|p| specular_video::extras::Packet {
                    start_us: p.start_us - start_us,
                    dur_us: p.dur_us,
                    data: p.data.clone(),
                })
                .collect();
        }
        x
    }
}

/// The stream playing.
struct Playing {
    stream: Stream,
    url: String,
    /// The piece on screen (or the file), once one opened.
    shown: Option<PathBuf>,
    /// A piece ended before the next one came.
    waiting: bool,
    /// The title last said (radio's now playing).
    said: Option<String>,
    /// Kept in the recent list with this title.
    kept: Option<Option<String>>,
    /// Opened from the play queue: it goes on when the stream ends.
    from_queue: bool,
    /// Said "Buffering" for this wait.
    said_wait: bool,
    /// A seek to this time (seconds from the stream's start) waits for its piece.
    pending: Option<f64>,
    /// The worker was asked to fetch from the pending time (not a piece already there).
    seeking: bool,
    /// The web file's reader waits ("Buffering…" said), and a part did not come (said).
    said_reading: bool,
    said_stalled: bool,
    /// The next piece was opened ahead.
    prepared: Option<PathBuf>,
    /// The folder was read again for this next piece.
    scanned: Option<PathBuf>,
    /// The next piece's player, made paused before the one playing ends (its first
    /// pictures decoded): taken when that piece shows.
    ready: Option<crate::anim::Anim>,
    /// The pieces' subtitles, stream-wide.
    subs: PieceSubs,
}

#[derive(Default)]
pub(super) struct StreamState {
    cur: Option<Playing>,
    /// The recent stream being forgotten (asked first).
    forgetting: Option<Recent>,
    opened: u64,
}

impl App {
    /// Opens `url` as a network stream.
    pub(super) fn stream_open(&mut self, url: String, from_queue: bool) {
        let now = self.now();
        if specular_core::net::off::is_off() {
            self.chrome.warning(t!("stream-network-off"), now);
            self.redraw = true;
            return;
        }
        let client = match specular_remote::Client::new(specular_remote::ClientConfig::default()) {
            Ok(c) => c.tracked(What::Stream),
            Err(e) => {
                self.chrome.warning(
                    t!("stream-unreachable", reason = crate::error_text::reason(&e)),
                    now,
                );
                return;
            }
        };
        let root = specular_settings::Locations::detect()
            .cache_dir
            .join("streams");
        // Folders left by an earlier run (a crash) go; the one playing is dropped first.
        self.stream.cur = None;
        if let Ok(rd) = std::fs::read_dir(&root) {
            for e in rd.flatten() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
        self.stream.opened += 1;
        let dir = root.join(format!("{}-{}", std::process::id(), self.stream.opened));
        let mut opts = Options::new(dir, client);
        opts.ytdlp = ytdlp_for(
            self.settings.privacy.ytdlp,
            self.settings.privacy.ytdlp_path.as_deref(),
        );
        opts.wake = Some(self.loader.waker());
        let stream = Stream::open(opts, &url);
        let host = specular_core::net::host_of(&url);
        self.chrome
            .toast(t!("stream-opening", host = host), None, now);
        self.stream.cur = Some(Playing {
            stream,
            url,
            shown: None,
            waiting: false,
            said: None,
            kept: None,
            from_queue,
            said_wait: false,
            pending: None,
            seeking: false,
            said_reading: false,
            said_stalled: false,
            prepared: None,
            scanned: None,
            ready: None,
            subs: PieceSubs::default(),
        });
        self.redraw = true;
    }

    /// Whether `path` is a piece of the stream playing.
    pub(super) fn stream_owns(&self, path: &Path) -> bool {
        self.stream
            .cur
            .as_ref()
            .is_some_and(|p| p.stream.owns(path))
    }

    /// A piece's extras with its subtitle tracks spanning the stream ([`PieceSubs`]); others
    /// as they are.
    pub(super) fn stream_piece_extras(
        &mut self,
        path: &Path,
        x: specular_video::extras::Extras,
    ) -> specular_video::extras::Extras {
        let Some(p) = self.stream.cur.as_mut().filter(|p| p.stream.owns(path)) else {
            return x;
        };
        match p.stream.chunk_of(path) {
            Some(c) if !x.subtitles.is_empty() => p.subs.merge((c.start * 1e6).round() as i64, x),
            _ => x,
        }
    }

    /// The piece after `path`, when it is ready (gapless priming).
    pub(super) fn stream_next_for(&self, path: &Path) -> Option<PathBuf> {
        let p = self.stream.cur.as_ref()?;
        p.stream.after(path).map(|c| c.path)
    }

    /// The piece `path` ended: the next one opens, or the stream waits for it. False when
    /// `path` is not a piece.
    pub(super) fn stream_on_end(&mut self, path: &Path) -> bool {
        let Some(p) = self.stream.cur.as_mut().filter(|p| p.stream.owns(path)) else {
            return false;
        };
        // A seek waits for its piece: that one opens, not the next.
        if p.pending.is_some() {
            return true;
        }
        match p.stream.after(path) {
            Some(next) => {
                p.waiting = false;
                self.queue_open(next.path, false);
            }
            None => {
                p.waiting = true;
                p.said_wait = false;
                self.stream_tick();
            }
        }
        true
    }

    /// Each turn: the first piece opened when ready, a waited-for piece opened, titles said,
    /// a failure said, and the stream stopped when something else is on screen.
    pub(super) fn stream_tick(&mut self) {
        if self.stream.cur.is_none() {
            return;
        }
        let now = self.now();
        let on_screen = self.current.and_then(|c| self.entries.get(c)).cloned();
        let Some(p) = self.stream.cur.as_mut() else {
            return;
        };
        let st: Status = p.stream.status();
        // Something else is on screen: the stream stops (its files go).
        if p.shown.is_some()
            && let Some(c) = &on_screen
            && !p.stream.owns(c)
        {
            self.stream.cur = None;
            return;
        }
        if let Some(c) = &on_screen
            && let Some(chunk) = p.stream.chunk_of(c)
        {
            p.stream.playing(chunk.index);
            p.shown = Some(c.clone());
        }
        self.stream_reading(now);
        self.stream_pending();
        self.stream_prepare_next();
        let Some(p) = self.stream.cur.as_mut() else {
            return;
        };
        // The title, for the recent list and to say.
        let title = st.title.clone();
        if p.shown.is_some() && p.kept.as_ref() != Some(&title) {
            p.kept = Some(title.clone());
            let r = Recent {
                url: addr::kept(&p.url),
                title,
            };
            self.stream_keep_recent(r);
        }
        let Some(p) = self.stream.cur.as_mut() else {
            return;
        };
        if let Some(np) = &st.now_playing
            && p.said.as_ref() != Some(np)
        {
            p.said = Some(np.clone());
            self.chrome
                .toast(t!("stream-now-playing", title = np.clone()), None, now);
            self.redraw = true;
        }
        let Some(p) = self.stream.cur.as_mut() else {
            return;
        };
        if p.shown.is_none() {
            if let Some(first) = st.chunks.first() {
                let path = first.path.clone();
                p.shown = Some(path.clone());
                self.gallery_dismiss();
                self.queue_open(path, true);
            } else if st.ended {
                let msg = st
                    .failure
                    .as_ref()
                    .map_or_else(|| t!("stream-empty"), failure_text);
                self.stream.cur = None;
                self.chrome.warning(msg, now);
            } else if let (Some(total), true) = (st.total, st.received > 0) {
                // A whole file: how far, while it comes.
                let pct = (st.received as f64 * 100.0 / total.max(1) as f64).round() as i64;
                self.chrome
                    .announce_status(t!("stream-buffering-percent", percent = pct));
            }
            self.redraw = true;
            return;
        }
        if p.waiting {
            let shown = p.shown.clone();
            if let Some(next) = shown.as_ref().and_then(|s| p.stream.after(s)) {
                p.waiting = false;
                self.queue_open(next.path, false);
            } else if st.ended {
                p.waiting = false;
                let from_queue = p.from_queue;
                if let Some(f) = &st.failure {
                    self.chrome.warning(failure_text(f), now);
                }
                self.stream.cur = None;
                if from_queue && self.queue.q.current().is_some() {
                    self.queue_after_stream();
                }
            } else if !p.said_wait {
                p.said_wait = true;
                self.chrome.toast(t!("stream-buffering"), None, now);
            }
            self.redraw = true;
        }
    }

    fn stream_keep_recent(&mut self, r: Recent) {
        let mut list = read_recent();
        push_recent(&mut list, r);
        let file = recent_file();
        self.loader.background("stream-recent", move |_| {
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&file, recent_text(&list));
        });
    }

    /// Asks before forgetting recent stream `r`.
    pub(super) fn stream_forget_ask(&mut self, r: Recent) {
        let name = shown(&r);
        self.stream.forgetting = Some(r);
        self.chrome.confirm_destructive(
            t!("stream-forget-ask", stream = name),
            t!("stream-forget-detail"),
            t!("stream-forget-ok"),
            FORGET_ITEM,
        );
        self.redraw = true;
    }

    /// Forgets the recent stream asked about.
    pub(super) fn stream_forget_confirmed(&mut self) {
        let Some(r) = self.stream.forgetting.take() else {
            return;
        };
        let mut list = read_recent();
        list.retain(|e| e.url != r.url);
        let _ = std::fs::write(recent_file(), recent_text(&list));
        self.address_recent_changed();
        let now = self.now();
        self.chrome
            .toast(t!("stream-forgotten", stream = shown(&r)), None, now);
        self.redraw = true;
    }

    /// A web file played while fetched: "Buffering…" while a read waits, and a part that
    /// never came said once.
    fn stream_reading(&mut self, now: f64) {
        let Some(p) = self.stream.cur.as_mut() else {
            return;
        };
        if p.stream.stalled() {
            if !p.said_stalled {
                p.said_stalled = true;
                self.chrome.warning(t!("stream-stalled"), now);
                self.redraw = true;
            }
        } else if p.stream.buffering() {
            if !p.said_reading {
                p.said_reading = true;
                self.chrome.toast(t!("stream-buffering"), None, now);
                self.redraw = true;
            }
        } else {
            p.said_reading = false;
        }
    }

    /// The piece on screen of a finite HLS or DASH stream, and the stream's length.
    fn stream_vod_piece(&self) -> Option<(Chunk, f64)> {
        let path = self.current.and_then(|c| self.entries.get(c))?;
        self.stream_piece_at(path)
    }

    /// The piece of a finite piece stream (HLS, DASH, Matroska or WebM in pieces) at `path`,
    /// and the stream's length.
    pub(super) fn stream_piece_at(&self, path: &Path) -> Option<(Chunk, f64)> {
        let p = self.stream.cur.as_ref()?;
        let chunk = p.stream.chunk_of(path)?;
        let st = p.stream.status();
        if st.live
            || !matches!(
                st.kind,
                Some(StreamKind::Hls | StreamKind::Dash | StreamKind::Pieces)
            )
        {
            return None;
        }
        Some((chunk, st.duration.filter(|d| *d > 0.0)?))
    }

    /// Info's Movie group for a piece of a finite stream: the stream's length (a piece's own
    /// is a few seconds, and a Matroska piece has none), and which part of it is on screen.
    pub(super) fn piece_rows(
        group: &mut specular_metadata::SummaryGroup,
        chunk: &Chunk,
        total: f64,
    ) {
        use specular_metadata::SummaryItem;
        let length = SummaryItem {
            key: "video-duration",
            label: t!("info-video-duration"),
            value: super::playback::clock(total),
            detail: None,
        };
        let at = match group.items.iter().position(|i| i.key == "video-duration") {
            Some(i) => {
                group.items[i] = length;
                i + 1
            }
            None => {
                group.items.insert(0, length);
                1
            }
        };
        let end = (chunk.start + chunk.seconds).min(total);
        group.items.retain(|i| i.key != "video-part");
        group.items.insert(
            at,
            SummaryItem {
                key: "video-part",
                label: t!("info-video-part"),
                value: t!(
                    "info-video-part-value",
                    start = super::playback::clock(chunk.start),
                    end = super::playback::clock(end)
                ),
                detail: None,
            },
        );
    }

    /// The transport of a piece of a finite stream: where in the whole stream, and its
    /// whole length.
    pub(super) fn stream_playback(&self, mut pb: Playback) -> Playback {
        if let (Some(v), Some((chunk, total))) = (pb.video.as_mut(), self.stream_vod_piece()) {
            let pending = self.stream.cur.as_ref().and_then(|p| p.pending);
            v.position = pending.unwrap_or(chunk.start + v.position).min(total);
            v.duration = total;
        }
        pb
    }

    /// The seek bar on a piece of a finite stream: `fraction` of the whole stream. False
    /// when not a stream's piece.
    pub(super) fn stream_seek_fraction(&mut self, fraction: f64) -> bool {
        let Some((_, total)) = self.stream_vod_piece() else {
            return false;
        };
        self.stream_go(fraction.clamp(0.0, 1.0) * total);
        true
    }

    /// Seeking by `secs` on a piece of a finite stream (it may land in another piece).
    pub(super) fn stream_seek_by(&mut self, secs: f64) -> bool {
        let Some((chunk, _)) = self.stream_vod_piece() else {
            return false;
        };
        let pending = self.stream.cur.as_ref().and_then(|p| p.pending);
        let at = pending.unwrap_or_else(|| {
            chunk.start
                + self
                    .anim
                    .as_ref()
                    .and_then(|a| a.time(std::time::Instant::now()))
                    .map_or(0.0, |t| t.as_secs_f64())
        });
        self.stream_go(at + secs);
        true
    }

    /// Plays from `t` seconds of the stream: in the piece on screen when it holds `t`, else
    /// in the piece holding it (fetched first when it is not there).
    fn stream_go(&mut self, t: f64) {
        let Some((chunk, total)) = self.stream_vod_piece() else {
            return;
        };
        let t = t.clamp(0.0, (total - 0.05).max(0.0));
        let len = self
            .anim
            .as_ref()
            .and_then(|a| a.playback().video)
            .map_or(chunk.seconds, |v| v.duration.max(chunk.seconds));
        let Some(p) = self.stream.cur.as_mut() else {
            return;
        };
        if t >= chunk.start && t < chunk.start + len {
            p.pending = None;
            p.seeking = false;
            if let Some(a) = self.anim.as_mut() {
                a.seek_time(std::time::Duration::from_secs_f64(t - chunk.start));
            }
            self.tick_animation();
            self.redraw = true;
            return;
        }
        p.pending = Some(t);
        p.prepared = None;
        p.ready = None;
        // A piece already fetched (ahead, or kept behind) holds it, unless a seek is under
        // way (its old pieces are going).
        let have = !p.seeking && p.stream.chunk_at(t).is_some();
        if !have {
            p.seeking = true;
            p.stream.seek(t);
            p.said_wait = false;
        }
        self.stream_pending();
        self.redraw = true;
    }

    /// A seek waiting for its piece: the piece opens when it is ready, and the movie seeks
    /// in it once it plays.
    fn stream_pending(&mut self) {
        let Some(p) = self.stream.cur.as_mut() else {
            return;
        };
        let Some(t) = p.pending else {
            return;
        };
        if p.seeking
            && p.stream
                .status()
                .seek
                .is_none_or(|s| (s.0 - t).abs() > 1e-6)
        {
            return;
        }
        let Some(c) = p.stream.chunk_at(t) else {
            if !p.said_wait {
                p.said_wait = true;
                let now = self.now();
                self.chrome.toast(t!("stream-buffering"), None, now);
            }
            return;
        };
        let on_screen = self.current.and_then(|i| self.entries.get(i)).cloned();
        if on_screen.as_ref() != Some(&c.path) {
            p.shown = Some(c.path.clone());
            p.waiting = false;
            self.queue_open(c.path, false);
            return;
        }
        let cur = self.current_id();
        let Some(a) = self
            .anim
            .as_mut()
            .filter(|a| Some(a.id) == cur && a.movie.is_some())
        else {
            return;
        };
        a.seek_time(std::time::Duration::from_secs_f64((t - c.start).max(0.0)));
        if let Some(p) = self.stream.cur.as_mut() {
            p.pending = None;
            p.seeking = false;
        }
        self.tick_animation();
        self.redraw = true;
    }

    /// The next piece, once fetched, is opened in the background (its file read and its
    /// first picture decoded), so it shows at once when the one playing ends.
    fn stream_prepare_next(&mut self) {
        let Some(p) = self.stream.cur.as_ref() else {
            return;
        };
        let Some(next) = p.shown.as_ref().and_then(|s| p.stream.after(s)) else {
            return;
        };
        if p.pending.is_some() {
            return;
        }
        if p.prepared.as_ref() == Some(&next.path) {
            self.stream_ready_player(&next.path);
            return;
        }
        let Some(i) = self.index_of(&next.path) else {
            // Not listed yet: the folder is read again (once for this piece).
            if p.scanned.as_ref() != Some(&next.path)
                && let Some(dir) = next.path.parent()
            {
                let dir = dir.to_path_buf();
                if let Some(p) = self.stream.cur.as_mut() {
                    p.scanned = Some(next.path);
                }
                self.scan_folder(dir);
            }
            return;
        };
        let id = self.id_for(i);
        self.ensure_open(id, Priority::Prefetch);
        self.request_levels(id, Priority::Prefetch);
        if let Some(p) = self.stream.cur.as_mut() {
            p.prepared = Some(next.path);
        }
    }

    /// The next piece's player, once its file is open: made paused, so its first pictures
    /// are decoded before it shows.
    fn stream_ready_player(&mut self, next: &Path) {
        let Some(i) = self.index_of(next) else {
            return;
        };
        let id = self.id_for(i);
        if self
            .stream
            .cur
            .as_ref()
            .is_none_or(|p| p.ready.as_ref().is_some_and(|a| a.id == id))
        {
            return;
        }
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let (Some(session), Some(info)) = (img.session.clone(), img.info.as_ref()) else {
            return;
        };
        if session.video().is_none_or(|f| f.unplayable.is_some()) {
            return;
        }
        let (dw, dh) = info.display_size();
        let need = self
            .viewport_px()
            .map_or(1, |(vw, vh)| crate::loader::needed_scale(dw, dh, vw, vh, 8));
        let scale = crate::anim::pick_scale(info.scales, need);
        let mut cfg = crate::anim::video_config(&self.settings, self.loop_forever);
        cfg.autoplay = false;
        if let Ok(Some(a)) =
            crate::anim::Anim::new(id, session.as_ref(), scale, cfg, self.loader.waker())
            && let Some(p) = self.stream.cur.as_mut()
        {
            p.ready = Some(a);
        }
    }

    /// The player made ahead for image `id` at `scale`, when there is one.
    pub(super) fn stream_take_ready(
        &mut self,
        id: crate::loader::ImageId,
        scale: u32,
    ) -> Option<crate::anim::Anim> {
        let p = self.stream.cur.as_mut()?;
        if p.ready
            .as_ref()
            .is_some_and(|a| a.id == id && a.scale == scale)
        {
            return p.ready.take();
        }
        None
    }

    /// The qualities on offer for the player's menu (more than one), and the one chosen.
    pub(super) fn stream_qualities(&self) -> (Vec<String>, Option<usize>) {
        let Some(p) = self.stream.cur.as_ref() else {
            return (Vec::new(), None);
        };
        let st = p.stream.status();
        if st.variants.len() < 2 {
            return (Vec::new(), None);
        }
        (st.variants, st.variant.filter(|_| st.manual))
    }

    /// A quality from the player's menu (`None`: automatic).
    pub(super) fn stream_choose_quality(&mut self, q: Option<usize>) {
        let now = self.now();
        let Some(p) = self.stream.cur.as_ref() else {
            self.chrome.toast(t!("stream-quality-none"), None, now);
            return;
        };
        let st = p.stream.status();
        let q = q.filter(|&v| v < st.variants.len());
        p.stream.choose(q);
        let msg = match q {
            Some(v) => t!("stream-quality-set", quality = st.variants[v].clone()),
            None => t!("stream-quality-auto"),
        };
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// Stream quality: automatic, then each variant from the lowest; says which.
    pub(super) fn stream_quality(&mut self) {
        let now = self.now();
        let Some(p) = self.stream.cur.as_ref() else {
            self.chrome.toast(t!("stream-quality-none"), None, now);
            return;
        };
        let st = p.stream.status();
        if st.variants.len() < 2 {
            self.chrome.toast(t!("stream-quality-one"), None, now);
            return;
        }
        let next = if !st.manual {
            Some(0)
        } else {
            st.variant.map(|v| v + 1).filter(|&v| v < st.variants.len())
        };
        p.stream.choose(next);
        let msg = match next {
            Some(v) => t!("stream-quality-set", quality = st.variants[v].clone()),
            None => t!("stream-quality-auto"),
        };
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// The panel under "Use yt-dlp for web pages": where it was found, or that it is not
    /// installed (looked for on `PATH` when no path is set). Going off-network stops the
    /// stream.
    pub(super) fn stream_settings_apply(&mut self) {
        if specular_core::net::off::is_off() {
            self.stream.cur = None;
        }
        let p = ytdlp_panel(ytdlp_for(true, self.settings.privacy.ytdlp_path.as_deref()));
        if self.chrome.set_setting_panel(YTDLP_PANEL, Some(p)) {
            self.redraw = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each piece shows every piece's subtitles, on its own time line; a piece read again
    /// (after a seek) adds nothing twice.
    #[test]
    fn piece_subtitles_span_the_stream() {
        use specular_video::extras::{Extras, Packet, SubStream};
        let piece = |cues: &[(i64, &str)]| Extras {
            subtitles: vec![SubStream {
                packets: cues
                    .iter()
                    .map(|&(t, d)| Packet {
                        start_us: t,
                        dur_us: Some(3_000_000),
                        data: d.as_bytes().to_vec(),
                    })
                    .collect(),
                ..SubStream::default()
            }],
            ..Extras::default()
        };
        let times = |x: &Extras| -> Vec<(i64, String)> {
            x.subtitles[0]
                .packets
                .iter()
                .map(|p| (p.start_us, String::from_utf8(p.data.clone()).unwrap()))
                .collect()
        };
        let mut s = PieceSubs::default();
        // Piece 0 (0–10 s): a cue at 8 s that lasts into piece 1.
        let a = s.merge(0, piece(&[(1_000_000, "one"), (8_000_000, "two")]));
        assert_eq!(
            times(&a),
            [(1_000_000, "one".into()), (8_000_000, "two".into())]
        );
        // Piece 1 (10–20 s): its own cue at 2 s (12 s), and the earlier ones before it.
        let b = s.merge(10_000_000, piece(&[(2_000_000, "three")]));
        assert_eq!(
            times(&b),
            [
                (-9_000_000, "one".into()),
                (-2_000_000, "two".into()),
                (2_000_000, "three".into())
            ]
        );
        // "two" (8–11 s) still shows 0.5 s into piece 1.
        assert!(
            b.subtitles[0]
                .packets
                .iter()
                .any(|p| p.start_us <= 500_000 && p.start_us + p.dur_us.unwrap() > 500_000)
        );
        // Piece 0 again after a seek back: the same three, nothing doubled.
        let c = s.merge(0, piece(&[(1_000_000, "one"), (8_000_000, "two")]));
        assert_eq!(c.subtitles[0].packets.len(), 3);
        assert_eq!(c.subtitles[0].packets[2].start_us, 12_000_000);
    }

    /// Info on a piece of a stream gives the stream's length and the part on screen, not
    /// the piece's own length (a Matroska piece has none at all).
    #[test]
    fn info_on_a_piece_shows_the_stream_and_the_part() {
        use specular_metadata::{SummaryGroup, SummaryItem};
        let item = |key: &'static str, value: &str| SummaryItem {
            key,
            label: String::new(),
            value: value.into(),
            detail: None,
        };
        let chunk = Chunk {
            path: PathBuf::from("piece-3.mkv"),
            index: 3,
            start: 75.0,
            seconds: 10.0,
            title: None,
        };
        // A piece with its own length (HLS): replaced, the part after it.
        let mut g = SummaryGroup {
            key: "video",
            title: String::new(),
            items: vec![item("video-duration", "0:10"), item("video-codec", "H.264")],
        };
        App::piece_rows(&mut g, &chunk, 5025.0);
        let keys: Vec<_> = g.items.iter().map(|i| i.key).collect();
        assert_eq!(keys, ["video-duration", "video-part", "video-codec"]);
        assert_eq!(g.items[0].value, "1:23:45");
        assert!(
            g.items[1].value.contains("1:15") && g.items[1].value.contains("1:25"),
            "{}",
            g.items[1].value
        );
        // A Matroska piece has no length: the stream's comes first; done twice, one part row.
        let mut g = SummaryGroup {
            key: "video",
            title: String::new(),
            items: vec![item("video-codec", "VP9")],
        };
        App::piece_rows(&mut g, &chunk, 80.0);
        App::piece_rows(&mut g, &chunk, 80.0);
        let keys: Vec<_> = g.items.iter().map(|i| i.key).collect();
        assert_eq!(keys, ["video-duration", "video-part", "video-codec"]);
        assert_eq!(g.items[0].value, "1:20");
        // The last piece ends at the stream's end.
        assert!(g.items[1].value.contains("1:20"), "{}", g.items[1].value);
    }

    #[test]
    fn recent_streams_keep_no_secrets_and_show_host_and_title() {
        let text = "https://u:pw@radio.example:8000/live?token=abc\tJazz FM\nnot an address\tx\nhttp://h/a.m3u8\t\n";
        let l = parse_recent(text);
        assert_eq!(
            l,
            vec![
                Recent {
                    url: "https://radio.example:8000/live".into(),
                    title: Some("Jazz FM".into())
                },
                Recent {
                    url: "http://h/a.m3u8".into(),
                    title: None
                },
            ],
            "most recent first, as written"
        );
        assert_eq!(shown(&l[0]), "radio.example:8000 — Jazz FM");
        assert_eq!(shown(&l[1]), "h/a.m3u8");
        assert_eq!(parse_recent(&recent_text(&l)), l, "round trip");
        let mut m = Vec::new();
        for i in 0..20 {
            push_recent(
                &mut m,
                Recent {
                    url: format!("http://h/{i}"),
                    title: None,
                },
            );
        }
        assert_eq!(m.len(), MAX_RECENT);
        assert_eq!(m[0].url, "http://h/19");
        // A title with a tab or line break stays one line.
        let t = recent_text(&[Recent {
            url: "http://h/x".into(),
            title: Some("a\tb\nc".into()),
        }]);
        assert_eq!(t, "http://h/x\ta b c\n");
    }

    #[test]
    fn media_addresses_are_streams_and_every_failure_has_words() {
        assert!(looks_like_stream("https://h/live/index.m3u8"));
        assert!(looks_like_stream("http://h/song.mp3?x=1"));
        assert!(looks_like_stream("http://h/radio.pls"));
        assert!(!looks_like_stream("https://h/x.zarr"));
        assert!(!looks_like_stream("https://h/watch?v=1"));
        for f in [
            Failure::NetworkOff,
            Failure::Drm,
            Failure::Encrypted,
            Failure::Page { installed: true },
            Failure::Page { installed: false },
            Failure::YtDlp("x".into()),
            Failure::Status(404),
            Failure::Unreachable("x".into()),
            Failure::Empty,
            Failure::Unsupported("x".into()),
            Failure::Disk("x".into()),
        ] {
            let s = failure_text(&f);
            assert!(!s.is_empty() && !s.contains("stream-"), "{f:?}: {s}");
        }
        assert_eq!(ytdlp_for(false, None), None);
        assert!(ytdlp_panel(None).rows[0].label.contains("not installed"));
        assert!(
            ytdlp_panel(Some(PathBuf::from("/usr/bin/yt-dlp"))).rows[0]
                .label
                .contains("/usr/bin/yt-dlp")
        );
        assert_eq!(
            ytdlp_for(true, Some(Path::new("/opt/yt-dlp"))),
            Some(PathBuf::from("/opt/yt-dlp"))
        );
    }

    /// Plays `a` (from paused) until it shows a frame; when.
    fn first_shown(a: &mut crate::anim::Anim) -> std::time::Instant {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        loop {
            let now = Instant::now();
            if a.tick(now).0.is_some() {
                return now;
            }
            assert!(t0.elapsed() < Duration::from_secs(20), "no frame");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// The boundary between two pieces of a stream: the time from the last frame of one to
    /// the first of the next, with the next piece's player made ahead (as the app does) and
    /// opened only when the first ends (as before). Frames are 40 ms apart.
    #[test]
    fn the_next_piece_made_ahead_shows_without_a_stall() {
        use std::time::{Duration, Instant};
        let ok = std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_ok_and(|o| o.status.success());
        if !ok {
            eprintln!("ffmpeg not installed: skipped");
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("specular-stream-boundary-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let out = std::process::Command::new("ffmpeg")
            .current_dir(&dir)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
            ])
            .args([
                "testsrc2=rate=25:size=1280x720:duration=4",
                "-c:v",
                "libx264",
            ])
            .args([
                "-g", "25", "-bf", "2", "-pix_fmt", "yuv420p", "-f", "segment",
            ])
            .args(["-segment_time", "2", "-reset_timestamps", "1", "p%d.ts"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        // Movies open through the video layer, as at the app's start.
        specular_video::install();
        let reg = specular_decode::Registry::default();
        let waker: specular_anim::Waker = std::sync::Arc::new(|| {});
        let cfg = || {
            let mut c = crate::anim::video_config(&specular_settings::Settings::default(), false);
            c.autoplay = false;
            c
        };
        let open = |name: &str| {
            let session = reg.open_path(&dir.join(name)).unwrap();
            crate::anim::Anim::new(1, session.as_ref(), 1, cfg(), waker.clone())
                .unwrap()
                .unwrap()
        };
        // Plays piece 0 to its end: when its last frame showed.
        let play_out = || {
            let mut a = open("p0.ts");
            a.toggle(Instant::now());
            let mut last = first_shown(&mut a);
            let t0 = Instant::now();
            while !a.finished() {
                let now = Instant::now();
                if a.tick(now).0.is_some() {
                    last = now;
                }
                assert!(t0.elapsed() < Duration::from_secs(20));
                std::thread::sleep(Duration::from_millis(1));
            }
            last
        };
        // Made ahead: opened while piece 0 plays. Best of two: one boundary can lose its
        // core to another process.
        let made_ahead = || {
            let mut ready = open("p1.ts");
            let last = play_out();
            ready.toggle(Instant::now());
            first_shown(&mut ready).duration_since(last)
        };
        let warm = made_ahead().min(made_ahead());
        // As before: opened when piece 0 ends.
        let last = play_out();
        let mut cold = open("p1.ts");
        cold.toggle(Instant::now());
        let cold = first_shown(&mut cold).duration_since(last);
        eprintln!(
            "piece boundary (720p H.264, 40 ms frames): {:.0} ms with the next piece made ahead, {:.0} ms opened at the end",
            warm.as_secs_f64() * 1e3,
            cold.as_secs_f64() * 1e3
        );
        // Wall-clock: not judged on an overloaded machine (`specular_log::timing`).
        specular_log::timing::assert_within("made ahead, against opened at the end", warm, cold);
        specular_log::timing::assert_within(
            "made ahead, about one frame",
            warm,
            Duration::from_millis(120),
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
