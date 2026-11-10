//! A Matroska or WebM web file (its server takes byte ranges) played while it is fetched,
//! in pieces cut at its clusters.
//!
//! The header, `Info`, `Tracks` and the `Cues` (where they are: before the clusters, or
//! wherever the `SeekHead` says) are fetched first. Each piece is a file that plays on its
//! own: the file's EBML header, a Segment of unknown size, `Info` (its duration left out),
//! `Tracks`, and the clusters of about [`super::Options::piece_s`] from a key frame of the
//! main track (the picture's, else the sound's), their times made to start at 0. The pieces
//! go to the player as an HLS stream's do: one after another, the sound without a gap, a
//! seek opens the piece holding the time.
//!
//! - With cues, every piece is known at once (time and bytes): a seek anywhere fetches the
//!   piece holding it.
//! - Without, the clusters are read in order as their bytes arrive (also clusters of unknown
//!   size, as live recordings write them); a seek goes back to a piece already read, or
//!   reads on to the time. The stream's length is the file's own (`Info`), else as far as
//!   it was read.

use super::{Failure, Job, StreamKind};

const EBML_HEADER: u32 = 0x1A45_DFA3;
const SEGMENT: u32 = 0x1853_8067;
const SEEK_HEAD: u32 = 0x114D_9B74;
const SEEK: u32 = 0x4DBB;
const SEEK_ID: u32 = 0x53AB;
const SEEK_POSITION: u32 = 0x53AC;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2AD7B1;
const DURATION: u32 = 0x4489;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_NUMBER: u32 = 0xD7;
const TRACK_TYPE: u32 = 0x83;
const CLUSTER: u32 = 0x1F43_B675;
const CLUSTER_TIMESTAMP: u32 = 0xE7;
const SIMPLE_BLOCK: u32 = 0xA3;
const BLOCK_GROUP: u32 = 0xA0;
const BLOCK: u32 = 0xA1;
const REFERENCE_BLOCK: u32 = 0xFB;
const BLOCK_DURATION: u32 = 0x9B;
const CUES: u32 = 0x1C53_BB6B;
const CUE_POINT: u32 = 0xBB;
const CUE_TIME: u32 = 0xB3;
const CUE_TRACK_POSITIONS: u32 = 0xB7;
const CUE_TRACK: u32 = 0xF7;
const CUE_CLUSTER_POSITION: u32 = 0xF1;
const CRC32: u32 = 0xBF;
/// Top-level elements: one of these ends a cluster of unknown size.
const TOP_LEVEL: [u32; 9] = [
    CLUSTER,
    CUES,
    0x1254_C367, // Tags
    INFO,
    TRACKS,
    SEEK_HEAD,
    0x1043_A770, // Chapters
    0x1941_A469, // Attachments
    EBML_HEADER,
];

/// Bytes asked for at least in one request while reading in order.
const WINDOW: u64 = 256 << 10;
/// Header elements read whole at most (`Info`, `Tracks`, `SeekHead`); `Cues` at most
/// [`MAX_CUES`].
const MAX_HEAD: u64 = 4 << 20;
const MAX_CUES: u64 = 32 << 20;
/// A cluster larger than this is refused (a damaged size).
const MAX_CLUSTER: u64 = 256 << 20;

/// Whether the first bytes are an EBML document (Matroska, WebM).
pub(super) fn is_matroska(first: &[u8]) -> bool {
    first.starts_with(&EBML_HEADER.to_be_bytes())
}

/// An element's header: its id, body size (`None`: unknown) and header length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Head {
    pub id: u32,
    pub size: Option<u64>,
    pub len: usize,
}

/// The element header at `p` of `b`.
pub(crate) fn head(b: &[u8], p: usize) -> Option<Head> {
    let first = *b.get(p)?;
    let n = first.leading_zeros() as usize + 1;
    if n > 4 {
        return None;
    }
    let id = b
        .get(p..p + n)?
        .iter()
        .fold(0u32, |a, &x| (a << 8) | u32::from(x));
    let q = p + n;
    let f = *b.get(q)?;
    let m = f.leading_zeros() as usize + 1;
    if m > 8 {
        return None;
    }
    let raw = b.get(q..q + m)?;
    let mut v = u64::from(f) & (0xFF >> m);
    let mut ones = v == 0xFF >> m;
    for &x in &raw[1..] {
        v = (v << 8) | u64::from(x);
        ones &= x == 0xFF;
    }
    Some(Head {
        id,
        size: (!ones).then_some(v),
        len: n + m,
    })
}

fn uint(b: &[u8]) -> u64 {
    b.iter().take(8).fold(0u64, |a, &x| (a << 8) | u64::from(x))
}

fn float(b: &[u8]) -> Option<f64> {
    match b.len() {
        4 => Some(f64::from(f32::from_be_bytes(b.try_into().ok()?))),
        8 => Some(f64::from_be_bytes(b.try_into().ok()?)),
        _ => None,
    }
}

/// The children of a body: (id, body).
fn children(b: &[u8]) -> Vec<(u32, &[u8])> {
    let mut out = Vec::new();
    let mut p = 0;
    while let Some(h) = head(b, p) {
        let s = p + h.len;
        let Some(e) = h.size.and_then(|n| s.checked_add(n as usize)) else {
            break;
        };
        let Some(body) = b.get(s..e) else { break };
        out.push((h.id, body));
        p = e;
    }
    out
}

fn el_id(id: u32, out: &mut Vec<u8>) {
    let b = id.to_be_bytes();
    let skip = b.iter().position(|&x| x != 0).unwrap_or(3);
    out.extend_from_slice(&b[skip..]);
}

/// An element with an 8-byte size.
fn el(id: u32, body: &[u8], out: &mut Vec<u8>) {
    el_id(id, out);
    out.push(0x01);
    out.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
    out.extend_from_slice(body);
}

/// What the header says.
#[derive(Default)]
pub(crate) struct Layout {
    /// Each piece's start: the file's EBML header, the Segment, `Info`, `Tracks`.
    pub prefix: Vec<u8>,
    /// Where the Segment's body ends.
    pub seg_end: u64,
    pub scale_ns: u64,
    pub duration_s: Option<f64>,
    /// The main track (picture, else sound) and whether it is sound.
    pub main: Option<u64>,
    pub main_is_sound: bool,
    /// The first cluster.
    pub first_cluster: Option<u64>,
    /// The cues of the main track: (time in ticks, cluster position in the file), in order.
    pub cues: Vec<(i64, u64)>,
}

/// The clusters of one piece: where they are in the file and the first one's time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Piece {
    pub t0: i64,
    pub start: u64,
    pub end: u64,
}

/// A cluster read: where it ends, its time, whether it starts with a key frame of `main`,
/// and when its last frame ends (ticks: its block's duration, else the step from the one
/// before it on its track).
pub(crate) struct Cluster {
    pub end: u64,
    pub ts: i64,
    pub key: bool,
    pub end_ts: i64,
}

/// Reads the cluster whose element starts at the start of `b` (its bytes from there on, at
/// least its header): `None` when more bytes are needed; `Some(Err)` when it is not one.
pub(crate) fn cluster_at(
    b: &[u8],
    at: u64,
    main: Option<u64>,
    main_is_sound: bool,
    eof: bool,
) -> Option<Result<Cluster, ()>> {
    let Some(h) = head(b, 0) else {
        return if eof || b.len() >= 12 {
            Some(Err(()))
        } else {
            None
        };
    };
    if h.id != CLUSTER {
        return Some(Err(()));
    }
    let end = match h.size {
        Some(n) if n > MAX_CLUSTER => return Some(Err(())),
        Some(n) => h.len + n as usize,
        None => {
            // Unknown size: up to the next top-level element (or the end of the file).
            let mut p = h.len;
            loop {
                match head(b, p) {
                    Some(c) if TOP_LEVEL.contains(&c.id) => break p,
                    Some(c) => match c.size {
                        Some(n) => p += c.len + n as usize,
                        None => return Some(Err(())),
                    },
                    None if eof => break p.min(b.len()),
                    None => return None,
                }
                if p > b.len() {
                    if eof {
                        break b.len();
                    }
                    return None;
                }
            }
        }
    };
    if end > b.len() {
        return if eof { Some(Err(())) } else { None };
    }
    let body = &b[h.len..end];
    let mut ts = 0i64;
    let mut key = None;
    // Per track: the last two block times (relative to the cluster), and a stated duration.
    let mut times: Vec<(u64, i64, Option<i64>, Option<i64>)> = Vec::new();
    let mut note = |block: &[u8], dur: Option<i64>| {
        if let Some((t, l)) = track_of(block)
            && let Some(r) = block.get(l..l + 2)
        {
            let rel = i64::from(i16::from_be_bytes([r[0], r[1]]));
            match times.iter_mut().find(|e| e.0 == t) {
                Some(e) => {
                    e.2 = Some(e.1);
                    e.1 = rel;
                    e.3 = dur;
                }
                None => times.push((t, rel, None, dur)),
            }
        }
    };
    for (id, c) in children(body) {
        match id {
            SIMPLE_BLOCK => note(c, None),
            BLOCK_GROUP => {
                let kids = children(c);
                if let Some(k) = kids.iter().find(|k| k.0 == BLOCK) {
                    let dur = kids
                        .iter()
                        .find(|k| k.0 == BLOCK_DURATION)
                        .map(|k| uint(k.1).min(i64::MAX as u64) as i64);
                    note(k.1, dur);
                }
            }
            _ => {}
        }
    }
    let last_rel = times
        .iter()
        .map(|&(_, last, prev, dur)| last + dur.or(prev.map(|p| (last - p).max(0))).unwrap_or(0))
        .max()
        .unwrap_or(0);
    for (id, c) in children(body) {
        match id {
            CLUSTER_TIMESTAMP => ts = uint(c).min(i64::MAX as u64) as i64,
            SIMPLE_BLOCK if key.is_none() => {
                if let Some((t, l)) = track_of(c)
                    && main.is_none_or(|m| m == t)
                {
                    key = Some(main_is_sound || c.get(l + 2).is_some_and(|f| f & 0x80 != 0));
                }
            }
            BLOCK_GROUP if key.is_none() => {
                let kids = children(c);
                let track = kids
                    .iter()
                    .find(|k| k.0 == BLOCK)
                    .and_then(|k| track_of(k.1));
                if let Some((t, _)) = track
                    && main.is_none_or(|m| m == t)
                {
                    key = Some(main_is_sound || !kids.iter().any(|k| k.0 == REFERENCE_BLOCK));
                }
            }
            _ => {}
        }
    }
    Some(Ok(Cluster {
        end: at + end as u64,
        ts,
        key: key.unwrap_or(false),
        end_ts: ts + last_rel,
    }))
}

/// A block's track number and the length of that number.
fn track_of(b: &[u8]) -> Option<(u64, usize)> {
    let f = *b.first()?;
    let n = f.leading_zeros() as usize + 1;
    if n > 8 {
        return None;
    }
    let mut v = u64::from(f) & (0xFF >> n);
    for &x in b.get(1..n)? {
        v = (v << 8) | u64::from(x);
    }
    Some((v, n))
}

/// The cluster in `b` (a whole element, of known or unknown size up to `b`'s end) with its
/// time made `ts - t0` (8-byte timestamp and size; a CRC-32 left out, as the bytes change).
pub(crate) fn rebased_cluster(b: &[u8], t0: i64, out: &mut Vec<u8>) {
    let Some(h) = head(b, 0) else { return };
    let end = h
        .size
        .map_or(b.len(), |n| (h.len + n as usize).min(b.len()));
    let mut body = Vec::with_capacity(end);
    let mut p = h.len;
    let mut ts = 0i64;
    while let Some(c) = head(b, p) {
        let s = p + c.len;
        let e = c.size.map_or(end, |n| (s + n as usize).min(end));
        if e > end || s > end {
            break;
        }
        match c.id {
            CLUSTER_TIMESTAMP => ts = uint(&b[s..e]).min(i64::MAX as u64) as i64,
            CRC32 => {}
            _ => body.extend_from_slice(&b[p..e]),
        }
        p = e;
        if p >= end {
            break;
        }
    }
    let mut new = Vec::with_capacity(body.len() + 11);
    el(
        CLUSTER_TIMESTAMP,
        &((ts - t0).max(0) as u64).to_be_bytes(),
        &mut new,
    );
    new.extend_from_slice(&body);
    el(CLUSTER, &new, out);
}

/// Pieces from the cues: from a cue at least `piece_ticks` after the piece's first.
pub(crate) fn plan(cues: &[(i64, u64)], piece_ticks: i64, end: u64) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for &(t, pos) in cues {
        match out.last_mut() {
            Some(last) if t - last.t0 < piece_ticks || pos <= last.start => {}
            Some(last) => {
                last.end = pos;
                out.push(Piece {
                    t0: t,
                    start: pos,
                    end,
                });
            }
            None => out.push(Piece {
                t0: t,
                start: pos,
                end,
            }),
        }
    }
    out
}

/// The header read: the EBML header, Segment, `Info`, `Tracks`, `SeekHead` and `Cues`.
pub(crate) fn read_header(
    mut read: impl FnMut(u64, u64) -> Result<Vec<u8>, Failure>,
    total: u64,
) -> Result<Layout, Failure> {
    let bad = |w: &str| Failure::Unsupported(format!("a Matroska file with {w}"));
    let first = read(0, 64.min(total))?;
    let h = head(&first, 0).ok_or_else(|| bad("a damaged header"))?;
    let hdr_len = h.len as u64 + h.size.ok_or_else(|| bad("a damaged header"))?;
    let hdr = read(0, hdr_len)?;
    // The Segment, after any Void elements.
    let mut p = hdr_len;
    let (seg_body, seg_end) = loop {
        let b = read(p, 12.min(total.saturating_sub(p)))?;
        let s = head(&b, 0).ok_or_else(|| bad("no Segment"))?;
        if s.id == SEGMENT {
            let body = p + s.len as u64;
            break (body, s.size.map_or(total, |n| (body + n).min(total)));
        }
        p += s.len as u64 + s.size.ok_or_else(|| bad("no Segment"))?;
    };
    let mut lay = Layout {
        seg_end,
        scale_ns: 1_000_000,
        ..Layout::default()
    };
    let (mut info, mut tracks, mut cues_at) = (None, None, None);
    let mut cues: Option<Vec<u8>> = None;
    let mut p = seg_body;
    while p + 2 <= seg_end {
        let b = read(p, 12.min(seg_end - p))?;
        let Some(e) = head(&b, 0) else { break };
        let body = p + e.len as u64;
        if e.id == CLUSTER {
            lay.first_cluster = Some(p);
            break;
        }
        let Some(size) = e.size else { break };
        match e.id {
            INFO | TRACKS | SEEK_HEAD if size <= MAX_HEAD => {
                let v = read(body, size)?;
                match e.id {
                    INFO => info = info.or(Some(v)),
                    TRACKS => tracks = tracks.or(Some(v)),
                    _ => {
                        for (_, s) in children(&v).into_iter().filter(|c| c.0 == SEEK) {
                            let kids = children(s);
                            let what = kids.iter().find(|k| k.0 == SEEK_ID).map(|k| uint(k.1));
                            let at = kids
                                .iter()
                                .find(|k| k.0 == SEEK_POSITION)
                                .map(|k| uint(k.1));
                            if what == Some(u64::from(CUES))
                                && let Some(a) = at
                            {
                                cues_at = Some(seg_body + a);
                            }
                        }
                    }
                }
            }
            CUES if size <= MAX_CUES => cues = Some(read(body, size)?),
            _ => {}
        }
        p = body + size;
    }
    if cues.is_none()
        && let Some(at) = cues_at.filter(|&a| a + 12 <= seg_end)
    {
        let b = read(at, 12)?;
        if let Some(e) = head(&b, 0)
            && e.id == CUES
            && let Some(size) = e.size.filter(|&s| s <= MAX_CUES)
        {
            cues = Some(read(at + e.len as u64, size)?);
        }
    }
    let info = info.ok_or_else(|| bad("no Info"))?;
    let tracks = tracks.ok_or_else(|| bad("no Tracks"))?;
    let mut info_kept = Vec::new();
    let mut duration = None;
    for (id, c) in children(&info) {
        match id {
            TIMESTAMP_SCALE => lay.scale_ns = uint(c).max(1),
            DURATION => {
                duration = float(c);
                continue;
            }
            CRC32 => continue,
            _ => {}
        }
        // Copied as it is (the duration is the whole file's, not the piece's).
        el(id, c, &mut info_kept);
    }
    lay.duration_s = duration
        .filter(|d| *d > 0.0)
        .map(|d| d * lay.scale_ns as f64 / 1e9);
    let mut video = None;
    let mut sound = None;
    for (id, t) in children(&tracks) {
        if id != TRACK_ENTRY {
            continue;
        }
        let kids = children(t);
        let num = kids.iter().find(|k| k.0 == TRACK_NUMBER).map(|k| uint(k.1));
        let kind = kids.iter().find(|k| k.0 == TRACK_TYPE).map(|k| uint(k.1));
        match kind {
            Some(1) => video = video.or(num),
            Some(2) => sound = sound.or(num),
            _ => {}
        }
    }
    lay.main = video.or(sound);
    lay.main_is_sound = video.is_none();
    if let (Some(c), Some(main)) = (&cues, lay.main) {
        for (_, cp) in children(c).into_iter().filter(|c| c.0 == CUE_POINT) {
            let kids = children(cp);
            let t = kids.iter().find(|k| k.0 == CUE_TIME).map(|k| uint(k.1));
            for (_, tp) in kids.iter().filter(|k| k.0 == CUE_TRACK_POSITIONS) {
                let pos = children(tp);
                let track = pos.iter().find(|k| k.0 == CUE_TRACK).map(|k| uint(k.1));
                let at = pos
                    .iter()
                    .find(|k| k.0 == CUE_CLUSTER_POSITION)
                    .map(|k| uint(k.1));
                if let (Some(t), Some(at), Some(tr)) = (t, at, track)
                    && tr == main
                {
                    lay.cues
                        .push((t.min(i64::MAX as u64) as i64, seg_body + at));
                }
            }
        }
        lay.cues.sort_unstable();
        lay.cues.dedup_by_key(|c| c.1);
    }
    let mut prefix = hdr;
    el_id(SEGMENT, &mut prefix);
    prefix.extend_from_slice(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    el(INFO, &info_kept, &mut prefix);
    el(TRACKS, &tracks, &mut prefix);
    lay.prefix = prefix;
    Ok(lay)
}

/// Bytes read in order, kept from where they start.
struct Window {
    at: u64,
    buf: Vec<u8>,
    /// Bytes from here on are kept when reading on (the piece being read).
    keep_from: u64,
}

impl Job<'_> {
    /// Bytes `at..at + n` (fewer at the end of the file), through the window.
    fn mkv_read(
        &mut self,
        url: &str,
        w: &mut Window,
        total: u64,
        at: u64,
        n: u64,
    ) -> Result<Vec<u8>, Failure> {
        let n = n.min(total.saturating_sub(at));
        if n == 0 {
            return Ok(Vec::new());
        }
        let have_end = w.at + w.buf.len() as u64;
        if at >= w.at && at + n <= have_end {
            let s = (at - w.at) as usize;
            return Ok(w.buf[s..s + n as usize].to_vec());
        }
        if self.cancelled() {
            return Err(Failure::Unreachable("cancelled".into()));
        }
        // Reading on from the window's end: keep what is still needed (from the piece
        // being read on).
        let (from, keep, kept_at) = if at >= w.at && at <= have_end {
            let k = at.min(w.keep_from).max(w.at);
            (have_end, w.buf.split_off((k - w.at) as usize), k)
        } else {
            (at, Vec::new(), at)
        };
        let want = (at + n).saturating_sub(from).max(WINDOW).min(total - from);
        let got = self.fetch(url, Some((from, want)))?;
        if (got.len() as u64) < want.min(at + n - from) {
            return Err(Failure::Unreachable(
                "the server sent less than asked".into(),
            ));
        }
        let fetched = got.len() as u64;
        self.set(|s| s.received = (s.received + fetched).min(total));
        w.at = kept_at;
        w.buf = keep;
        w.buf.extend_from_slice(&got);
        let s = (at - w.at) as usize;
        Ok(w.buf[s..(s + n as usize).min(w.buf.len())].to_vec())
    }

    /// Plays the Matroska or WebM file at `url` (`total` bytes) in pieces as it is fetched.
    pub(super) fn mkv_pieces(&mut self, url: &str, total: u64) -> Result<(), Failure> {
        self.set(|s| {
            s.kind = Some(StreamKind::Pieces);
            s.total = Some(total);
            s.live = false;
        });
        let mut w = Window {
            at: 0,
            buf: Vec::new(),
            keep_from: 0,
        };
        let lay = {
            let mut read = |a: u64, n: u64| self.mkv_read(url, &mut w, total, a, n);
            read_header(&mut read, total)?
        };
        let first = lay.first_cluster.ok_or(Failure::Empty)?;
        let ext = if lay.prefix.windows(4).any(|x| x == b"webm") {
            "webm"
        } else {
            "mkv"
        };
        let ticks_s = lay.scale_ns as f64 / 1e9;
        let piece_ticks = (self.opts.piece_s / ticks_s).max(1.0) as i64;
        let mut table = plan(&lay.cues, piece_ticks, lay.seg_end);
        // With cues every piece is known; without, they are found as the file is read.
        let cued = table.first().is_some_and(|p| p.start == first);
        if !cued {
            table.clear();
        }
        let origin = table.first().map(|p| p.t0);
        specular_log::debug!(target: specular_log::IO, "stream Matroska web file of {total} bytes in pieces; cues: {}", table.len());
        if let Some(d) = lay.duration_s {
            self.set(|s| s.duration = Some(d));
        }
        let mut origin = origin;
        // Without cues: how far the clusters were read, and the time there.
        let mut scanned = first;
        let mut scan_t: Option<i64> = None;
        // Without cues: where the last frame read ends.
        let mut end_t: Option<i64> = None;
        let mut k = 0usize;
        let mut target: Option<f64> = None;
        let secs = |t: i64, origin: Option<i64>| (t - origin.unwrap_or(t)) as f64 * ticks_s;
        loop {
            self.wait_room()?;
            if let Some(t) = self.take_seek() {
                let read_to = scan_t.map_or(f64::INFINITY, |s| secs(s, origin));
                let known = cued || scanned >= lay.seg_end || t < read_to;
                match table.iter().rposition(|p| secs(p.t0, origin) <= t + 1e-6) {
                    Some(i) if known => {
                        k = i;
                        target = None;
                    }
                    _ if known && !table.is_empty() => {
                        k = 0;
                        target = None;
                    }
                    _ => {
                        // Beyond what was read: read on to it.
                        k = table.len();
                        target = Some(t);
                    }
                }
            }
            if k < table.len() {
                // A piece known: its clusters fetched (or still at hand) and sent.
                let p = table[k];
                w.keep_from = p.start;
                let bytes = self.mkv_read(url, &mut w, total, p.start, p.end - p.start)?;
                // The last piece read without cues ends where the file says it does, else
                // where its last frame ends.
                let last = !cued && k + 1 == table.len() && scanned >= lay.seg_end;
                let origin_s = origin.map_or(0.0, |o| o as f64 * ticks_s);
                let end = table
                    .get(k + 1)
                    .map(|n| secs(n.t0, origin))
                    .or(lay.duration_s.filter(|_| last).map(|d| d - origin_s))
                    .or(end_t.filter(|_| last).map(|t| secs(t, origin)))
                    .or(scan_t.filter(|_| !cued).map(|s| secs(s, origin)))
                    .or(lay.duration_s)
                    .unwrap_or(secs(p.t0, origin));
                let (data, first_t) = piece_file(&lay, &bytes);
                let start = secs(first_t.unwrap_or(p.t0), origin);
                self.emit(ext, data, start, (end - start).max(0.0))?;
                k += 1;
                continue;
            }
            if cued || scanned >= lay.seg_end {
                // Every piece made: a later seek is still answered.
                self.idle_until_seek()?;
                continue;
            }
            // Without cues: the clusters read on from where the reading got to, up to the
            // next key frame of the main track at least a piece's length on.
            let start = scanned;
            w.keep_from = start;
            let mut t0 = None;
            let mut cut = None;
            let mut last_ts = None;
            let mut p = scanned;
            while p < lay.seg_end {
                let mut need = 64u64;
                let c = loop {
                    let b = self.mkv_read(url, &mut w, total, p, need)?;
                    let eof = p + b.len() as u64 >= lay.seg_end;
                    match cluster_at(&b, p, lay.main, lay.main_is_sound, eof) {
                        Some(r) => break r,
                        None if eof => break Err(()),
                        None => need = (need * 2).max(WINDOW),
                    }
                };
                let Ok(c) = c else {
                    // Not a cluster (Cues or Tags at the end): the clusters are over.
                    p = lay.seg_end;
                    break;
                };
                let first_t = *t0.get_or_insert(c.ts);
                if p > start && c.key && c.ts - first_t >= piece_ticks {
                    cut = Some((p, c.ts));
                    break;
                }
                last_ts = Some(c.ts);
                end_t = Some(end_t.map_or(c.end_ts, |e: i64| e.max(c.end_ts)));
                p = c.end;
            }
            let Some(t0) = t0 else {
                self.idle_until_seek()?;
                continue;
            };
            scanned = cut.map_or(lay.seg_end.max(p), |c| c.0);
            // Where the next piece starts (the last: its last cluster's time, a guess).
            let next_t = cut.map_or(last_ts.unwrap_or(t0), |c| c.1);
            scan_t = Some(next_t);
            origin.get_or_insert(t0);
            table.push(Piece {
                t0,
                start,
                end: cut.map_or(p, |c| c.0),
            });
            if lay.duration_s.is_none() {
                // The stream is as long as it was read (to its last frame's end at the end).
                let d = match (cut, end_t) {
                    (None, Some(e)) => secs(e, origin),
                    _ => secs(next_t, origin),
                };
                self.set(|s| s.duration = Some(d));
            }
            match target {
                Some(t) if cut.is_some() && secs(next_t, origin) <= t => {
                    // Not there yet: read on (this piece is not sent).
                    k = table.len();
                }
                _ => {
                    target = None;
                    k = table.len() - 1;
                }
            }
        }
    }
}

/// One piece: the prefix and the clusters in `bytes`, their times from the first one's (a
/// cue's time is its key frame's; the cluster may start before it). That time too.
pub(crate) fn piece_file(lay: &Layout, bytes: &[u8]) -> (Vec<u8>, Option<i64>) {
    let mut out = lay.prefix.clone();
    let mut t0 = None;
    let mut p = 0usize;
    while let Some(h) = head(bytes, p) {
        let end = h
            .size
            .map_or(bytes.len(), |n| (p + h.len + n as usize).min(bytes.len()));
        let end = if h.size.is_none() {
            // Unknown size: up to the next top-level element.
            let mut q = p + h.len;
            while let Some(c) = head(bytes, q) {
                if TOP_LEVEL.contains(&c.id) {
                    break;
                }
                match c.size {
                    Some(n) => q += c.len + n as usize,
                    None => break,
                }
            }
            q.min(bytes.len())
        } else {
            end
        };
        if h.id == CLUSTER {
            let c = &bytes[p..end];
            let base = *t0.get_or_insert_with(|| {
                cluster_at(c, 0, None, false, true)
                    .and_then(Result::ok)
                    .map_or(0, |c| c.ts)
            });
            rebased_cluster(c, base, &mut out);
        }
        if end <= p {
            break;
        }
        p = end;
    }
    (out, t0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heads_and_plans() {
        let unknown = [
            0x1F, 0x43, 0xB6, 0x75, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ];
        assert_eq!(
            head(&unknown, 0),
            Some(Head {
                id: CLUSTER,
                size: None,
                len: 12
            })
        );
        assert_eq!(
            head(&[0xE7, 0x81, 0x05], 0),
            Some(Head {
                id: CLUSTER_TIMESTAMP,
                size: Some(1),
                len: 2
            })
        );
        let cues = [(0, 100), (1000, 200), (2500, 300), (4500, 400), (4600, 500)];
        let p = plan(&cues, 2000, 900);
        let piece = |t0, start, end| Piece { t0, start, end };
        assert_eq!(
            p,
            [
                piece(0, 100, 300),
                piece(2500, 300, 400),
                piece(4500, 400, 900)
            ]
        );
    }

    #[test]
    fn clusters_are_rebased_and_found() {
        // A cluster of unknown size at time 5000: a timestamp and a key block of track 1.
        let mut c = vec![
            0x1F, 0x43, 0xB6, 0x75, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ];
        c.extend_from_slice(&[0xE7, 0x82, 0x13, 0x88]);
        let block = [0xA3, 0x86, 0x81, 0x00, 0x00, 0x80, 0xAA, 0xBB];
        c.extend_from_slice(&block);
        let mut next = c.clone();
        next.extend_from_slice(&[0x1F, 0x43, 0xB6, 0x75, 0x80]);
        let r = cluster_at(&next, 1000, Some(1), false, false)
            .unwrap()
            .unwrap();
        assert_eq!((r.end, r.ts, r.key), (1000 + c.len() as u64, 5000, true));
        assert!(
            cluster_at(&c[..14], 0, Some(1), false, false).is_none(),
            "needs more"
        );
        let mut out = Vec::new();
        rebased_cluster(&c, 4000, &mut out);
        let h = head(&out, 0).unwrap();
        assert_eq!((h.id, h.size), (CLUSTER, Some(out.len() as u64 - 12)));
        let r = cluster_at(&out, 0, Some(1), false, true).unwrap().unwrap();
        assert_eq!((r.ts, r.key), (1000, true));
        assert!(out.ends_with(&block));
    }
}
