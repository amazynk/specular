//! Windows Media Center TV recordings (`.wtv`) into the same [`Movie`] the MP4 demuxer
//! gives. There is no published specification; the layout below is what recordings by
//! Windows 7 Media Center and the files ffmpeg writes show.
//!
//! - The file is a small file system of 4 KiB sectors. The header (its GUID at 0) points at
//!   the root directory's sector (32-bit word at 0x38). A directory entry: the entry GUID,
//!   its length (u64), the file's size (u64, its top byte flags: 0x40 the data follows in the
//!   entry, 0x80 the file is made of 4 KiB sectors, else of 256 KiB chunks), the name's
//!   length in UTF-16 units (u32) and a word of padding, the name, then the first sector and
//!   the depth: 0 the data itself, 1 a list of sector numbers, 2 a list of such lists.
//! - `timeline` holds the packets: a 32-byte header (GUID, length with the header, stream
//!   id, counter), the payload, padding to 8 bytes. Stream descriptors carry the stream's
//!   DirectShow media type ([`crate::dshow`]); timestamp packets the next sample's
//!   presentation time (100 ns units, −1 when the sample continues); data packets the
//!   elementary stream, at most 8 KiB each, so a picture spans several.
//! - `table.0.entries.legacy_attrib` holds the programme's tags (Windows Media attribute
//!   records: GUID, type, length, UTF-16 name, value): title, episode, description, channel,
//!   broadcast and recording dates.
//! - Encrypted recordings (copy-once broadcasts: `WM/WMRVContentProtectedPercent` above 0)
//!   are refused, never decrypted. `WM/WMRVContentProtected` alone only marks the
//!   broadcast's flag; such recordings are in the clear.
//!
//! Samples are found by one pass over the packets (the file is memory-mapped; nothing is
//! copied). A sample's offset is the position in `timeline` of the packet where it starts,
//! its size the elementary-stream bytes it holds; [`WtvLayout::read`] gathers them.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::asf::guid;
use crate::dshow::Media;
use crate::extras::TrackInfo;
use crate::mp4::{
    AudioCodec, AudioTrack, Codec, Edit, Error, Layout, MAX_SAMPLES, Movie, Result, Sample,
    VideoTrack,
};

const FILE: [u8; 16] = guid(b"2000D8B7-4937-11DA-A64E-0007E95EAD8D");
const ENTRY: [u8; 16] = guid(b"9174B792-7059-4470-88DF-063B82CC213D");
/// Stream descriptors: three forms, the media type at 20, 12 and 28 bytes in.
const STREAM: [u8; 16] = guid(b"C2D2C3A1-9A7E-11DA-8BF7-0007E95EAD8D");
const STREAM_B: [u8; 16] = guid(b"C2D2C3A2-9A7E-11DA-8BF7-0007E95EAD8D");
const STREAM2: [u8; 16] = guid(b"2313A4ED-BF2D-454F-AD8A-D95BA7F91FEE");
const TIMESTAMP: [u8; 16] = guid(b"1BE6055B-A997-4349-8817-1A655A298A97");
const DATA: [u8; 16] = guid(b"C2D2C395-9A7E-11DA-8BF7-0007E95EAD8D");
/// Windows Media attribute records in `legacy_attrib`.
const ATTRIBUTE: [u8; 16] = guid(b"6DD7FE5A-1DC8-4A8F-9922-FAB11C381453");

const SECTOR: u64 = 0x1000;
const CHUNK: u64 = 0x40000;
/// Directory entries read at most.
const MAX_ENTRIES: usize = 512;
/// Bytes of a sample read at parse time for its picture headers.
const PEEK: usize = 4096;

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

fn u64le(d: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(d.get(o..o + 8)?.try_into().ok()?))
}

/// Whether `head` starts a WTV file.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 0x40 && head[..16] == FILE
}

/// A file of the WTV file system: its pieces in the container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WtvFile {
    /// File offsets of the units, in order.
    units: Arc<Vec<u64>>,
    /// Bytes per unit (the last may be short).
    unit: u64,
    pub size: u64,
}

impl WtvFile {
    /// `len` bytes at position `at`: borrowed when in one unit.
    fn read<'a>(&self, data: &'a [u8], at: u64, len: usize) -> Option<Cow<'a, [u8]>> {
        let end = at.checked_add(len as u64)?;
        if end > self.size {
            return None;
        }
        let k = (at / self.unit) as usize;
        let within = at % self.unit;
        let start = self.units.get(k)?.checked_add(within)?;
        if within + len as u64 <= self.unit {
            let s = usize::try_from(start).ok()?;
            return data.get(s..s.checked_add(len)?).map(Cow::Borrowed);
        }
        let mut out = Vec::with_capacity(len);
        let mut p = at;
        while p < end {
            let k = (p / self.unit) as usize;
            let w = p % self.unit;
            let n = (self.unit - w).min(end - p) as usize;
            let s = usize::try_from(self.units.get(k)? + w).ok()?;
            out.extend_from_slice(data.get(s..s + n)?);
            p += n as u64;
        }
        Some(Cow::Owned(out))
    }

    fn all<'a>(&self, data: &'a [u8]) -> Option<Cow<'a, [u8]>> {
        self.read(data, 0, usize::try_from(self.size).ok()?)
    }
}

/// A directory entry: the file it names (resident data is kept as its offset in `data`).
fn open_file(data: &[u8], size: u64, flags: u8, rest_at: usize) -> Result<WtvFile> {
    let len = data.len() as u64;
    if flags & 0x40 != 0 {
        // The data follows the name.
        let at = rest_at as u64;
        if at + size > len {
            return Err(bad("WTV resident file runs past the end"));
        }
        return Ok(WtvFile {
            units: Arc::new(vec![at]),
            unit: size.max(1),
            size,
        });
    }
    let sector = u64::from(u32le(data, rest_at).ok_or_else(|| bad("WTV entry cut short"))?);
    let depth = u32le(data, rest_at + 4).ok_or_else(|| bad("WTV entry cut short"))?;
    let unit = if flags & 0x80 != 0 { SECTOR } else { CHUNK };
    if size > len.saturating_mul(2) {
        return Err(bad("WTV file larger than the container"));
    }
    let count = size.div_ceil(unit) as usize;
    let pointers = |at: u64, n: usize| -> Result<Vec<u64>> {
        let from = usize::try_from(at * SECTOR).map_err(|_| bad("WTV sector out of range"))?;
        let bytes = data
            .get(from..from.saturating_add(n * 4))
            .ok_or_else(|| bad("WTV sector list past the end"))?;
        Ok(bytes
            .chunks_exact(4)
            .map(|c| u64::from(u32::from_le_bytes([c[0], c[1], c[2], c[3]])))
            .collect())
    };
    let sectors = match depth {
        0 => {
            return Ok(WtvFile {
                units: Arc::new(vec![sector * SECTOR]),
                unit: size.max(1),
                size,
            });
        }
        1 => pointers(sector, count)?,
        2 => {
            let per = (SECTOR / 4) as usize;
            let mut v = Vec::with_capacity(count);
            for list in pointers(sector, count.div_ceil(per))? {
                let n = per.min(count - v.len());
                v.extend(pointers(list, n)?);
            }
            v
        }
        d => return Err(Error::Unsupported(format!("WTV files {d} levels deep"))),
    };
    let units: Vec<u64> = sectors.into_iter().map(|s| s * SECTOR).collect();
    if units.iter().any(|&u| u >= len) {
        return Err(bad("WTV sector out of range"));
    }
    Ok(WtvFile {
        units: Arc::new(units),
        unit,
        size,
    })
}

/// The root directory's files by name.
fn directory(data: &[u8]) -> Result<HashMap<String, WtvFile>> {
    let root = u64::from(u32le(data, 0x38).ok_or_else(|| bad("WTV header cut short"))?);
    let mut p = usize::try_from(root * SECTOR).map_err(|_| bad("WTV root out of range"))?;
    let mut files = HashMap::new();
    for _ in 0..MAX_ENTRIES {
        if data.get(p..p + 16) != Some(&ENTRY[..]) {
            break;
        }
        let (Some(len), Some(size), Some(chars)) = (
            u64le(data, p + 16),
            u64le(data, p + 24),
            u32le(data, p + 32),
        ) else {
            break;
        };
        let name_end = p + 40 + chars as usize * 2;
        if len < 40 || name_end as u64 > p as u64 + len {
            break;
        }
        // The entry's length may claim more than the file holds.
        let Some(name) = data.get(p + 40..name_end) else {
            break;
        };
        let name: Vec<u16> = name
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&c| c != 0)
            .collect();
        let name = String::from_utf16_lossy(&name);
        let flags = (size >> 56) as u8;
        if let Ok(f) = open_file(data, size & ((1 << 56) - 1), flags, name_end) {
            files.entry(name).or_insert(f);
        }
        p = match usize::try_from(len).ok().and_then(|l| p.checked_add(l)) {
            Some(n) => n,
            None => break,
        };
    }
    if files.is_empty() {
        return Err(bad("WTV file without a directory"));
    }
    Ok(files)
}

/// Where a WTV track's samples are: `Sample::offset` is the position in `timeline` of the
/// data packet where the sample starts, `Sample::size` its elementary-stream bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WtvLayout {
    pub timeline: WtvFile,
    pub stream: u32,
    /// H.264 Annex B: start codes become 4-byte lengths.
    pub annex_b: bool,
}

impl WtvLayout {
    /// The bytes of sample `s` (see [`crate::mp4::Layout::read`]).
    pub fn read<'a>(&self, data: &'a [u8], s: &Sample) -> Option<Cow<'a, [u8]>> {
        let es = gather(data, &self.timeline, self.stream, s.offset, s.size as usize)?;
        Some(if self.annex_b {
            Cow::Owned(crate::ts::length_prefixed(&es))
        } else {
            es
        })
    }
}

/// `limit` payload bytes of `stream`'s data packets from the packet at `at`.
fn gather<'a>(
    data: &'a [u8],
    tl: &WtvFile,
    stream: u32,
    mut at: u64,
    limit: usize,
) -> Option<Cow<'a, [u8]>> {
    let mut out: Option<Cow<'a, [u8]>> = None;
    let mut got = 0;
    while got < limit && at + 32 <= tl.size {
        let h = tl.read(data, at, 32)?;
        let len = u64::from(u32le(&h, 16)?);
        if len < 32 {
            return None;
        }
        if h[..16] == DATA && u32le(&h, 20)? == stream {
            let n = ((len - 32) as usize).min(limit - got);
            let piece = tl.read(data, at + 32, n)?;
            out = Some(match out {
                // The whole sample in one packet: borrowed.
                None if n == limit => piece,
                None => {
                    let mut v = Vec::with_capacity(limit);
                    v.extend_from_slice(&piece);
                    Cow::Owned(v)
                }
                Some(mut o) => {
                    o.to_mut().extend_from_slice(&piece);
                    o
                }
            });
            got += n;
        }
        at += len + (len.wrapping_neg() & 7);
    }
    out
}

/// One data packet of a stream.
#[derive(Clone, Copy)]
struct Piece {
    /// The packet's position in `timeline`.
    at: u64,
    len: u32,
    /// From the timestamp packet before it (100 ns); `None` continues the sample.
    pts: Option<i64>,
}

/// The timeline's streams: their media types and data packets.
struct Scan {
    media: HashMap<u32, Media>,
    pieces: HashMap<u32, Vec<Piece>>,
    /// Stream ids in order of their first data packet.
    order: Vec<u32>,
}

fn scan(data: &[u8], tl: &WtvFile) -> Result<Scan> {
    let mut media = HashMap::new();
    let mut pieces: HashMap<u32, Vec<Piece>> = HashMap::new();
    let mut pending: HashMap<u32, i64> = HashMap::new();
    let mut order = Vec::new();
    let mut total = 0usize;
    let mut at = 0u64;
    while at + 32 <= tl.size {
        let Some(h) = tl.read(data, at, 32) else {
            break;
        };
        let len = u64::from(u32le(&h, 16).unwrap_or(0));
        if len < 32 || at + len > tl.size {
            break; // the end of the recording (or a damaged packet)
        }
        let id: [u8; 16] = h[..16].try_into().unwrap_or_default();
        let sid = u32le(&h, 20).unwrap_or(0);
        let body_len = (len - 32) as usize;
        if id == DATA {
            if !pieces.contains_key(&sid) && media.contains_key(&sid) {
                order.push(sid);
                pieces.insert(sid, Vec::new());
            }
            if let Some(list) = pieces.get_mut(&sid) {
                list.push(Piece {
                    at,
                    len: body_len as u32,
                    pts: pending.remove(&sid),
                });
                total += 1;
                if total > 4 * MAX_SAMPLES {
                    return Err(Error::Unsupported("over four million WTV packets".into()));
                }
            }
        } else if id == TIMESTAMP {
            if let Some(b) = tl.read(data, at + 32, body_len.min(16))
                && let Some(t) = u64le(&b, 8).map(|t| t as i64)
                && t >= 0
            {
                pending.insert(sid & 0x3FFF_FFFF, t);
            }
        } else if id == STREAM || id == STREAM_B || id == STREAM2 {
            let skip = if id == STREAM {
                20
            } else if id == STREAM_B {
                12
            } else {
                28
            };
            let sid = sid & 0x7FFF_FFFF;
            if !media.contains_key(&sid)
                && let Some(b) = tl.read(data, at + 32, body_len)
                && let Some(m) = b.get(skip..).and_then(crate::dshow::parse)
            {
                media.insert(sid, m);
            }
        }
        at += len + (len.wrapping_neg() & 7);
    }
    Ok(Scan {
        media,
        pieces,
        order,
    })
}

/// Pieces grouped into samples: (first piece's packet, bytes, pts).
fn groups(pieces: &[Piece]) -> Vec<(u64, u64, i64)> {
    let mut out: Vec<(u64, u64, i64)> = Vec::new();
    for p in pieces {
        match (p.pts, out.last_mut()) {
            (Some(t), _) => out.push((p.at, u64::from(p.len), t)),
            (None, Some(last)) => last.1 += u64::from(p.len),
            (None, None) => {}
        }
    }
    out.retain(|g| g.1 > 0);
    out
}

/// Durations until the next sample in presentation order; decode times sorted.
fn time_samples(list: &mut [Sample], fallback: i64) {
    let mut order: Vec<i64> = list.iter().map(|s| s.cts).collect();
    order.sort_unstable();
    for (s, &t) in list.iter_mut().zip(&order) {
        s.dts = t.max(0) as u64;
    }
    order.dedup();
    for s in list.iter_mut() {
        let k = order.partition_point(|&c| c <= s.cts);
        s.duration = order
            .get(k)
            .map_or(fallback, |n| n - s.cts)
            .clamp(0, i64::from(u32::MAX)) as u32;
    }
}

const TIMESCALE: u32 = 10_000_000;

#[allow(clippy::too_many_arguments)]
fn video_track(
    data: &[u8],
    tl: &WtvFile,
    sid: u32,
    codec: Codec,
    mut config: Vec<u8>,
    (width, height): (u32, u32),
    pieces: &[Piece],
    origin: i64,
    samples: bool,
) -> VideoTrack {
    let annex_b = matches!(codec, Codec::H264 | Codec::Hevc);
    let layout = WtvLayout {
        timeline: tl.clone(),
        stream: sid,
        annex_b: false,
    };
    let mut list: Vec<Sample> = Vec::new();
    let mut started = false;
    let mut open_field: Option<specular_mpeg2::Structure> = None;
    let mut heads: Vec<Vec<u8>> = Vec::new();
    for (at, size, pts) in groups(pieces) {
        let size = size.min(u64::from(u32::MAX)) as u32;
        let head_sample = Sample {
            offset: at,
            size: size.min(PEEK as u32),
            dts: 0,
            cts: 0,
            duration: 0,
            sync: false,
        };
        let head = layout.read(data, &head_sample).unwrap_or_default();
        let sync = match codec {
            Codec::Mpeg2 | Codec::Mpeg1 => {
                let sc = specular_mpeg2::scan(&head);
                if config.is_empty()
                    && let Some(b) = specular_mpeg2::sequence_bytes(&head)
                {
                    config = b.to_vec();
                }
                let info = sc.pictures.first().copied();
                // The second field of a field pair joins the first.
                let structure = info.map_or(specular_mpeg2::Structure::Frame, |i| i.structure);
                if let (Some(o), Some(last)) = (open_field, list.last_mut())
                    && structure != specular_mpeg2::Structure::Frame
                    && structure != o
                    && sc.pictures.len() == 1
                {
                    last.size = last.size.saturating_add(size);
                    open_field = None;
                    continue;
                }
                open_field = (structure != specular_mpeg2::Structure::Frame
                    && sc.pictures.len() == 1)
                    .then_some(structure);
                let intra = info.is_some_and(|i| i.kind == specular_mpeg2::PictureType::I);
                // A recording starts mid-stream: wait for an I picture with its header.
                if !started {
                    if intra && !config.is_empty() {
                        started = true;
                    } else {
                        continue;
                    }
                }
                intra
            }
            _ => {
                if heads.len() < 8 {
                    heads.push(head.to_vec());
                }
                let key = crate::mxf::h264_key(&head);
                if !started {
                    if key {
                        started = true;
                    } else {
                        continue;
                    }
                }
                key
            }
        };
        list.push(Sample {
            offset: at,
            size,
            dts: 0,
            cts: pts - origin,
            duration: 0,
            sync,
        });
        if list.len() >= MAX_SAMPLES {
            break;
        }
    }
    // The sequence header's size is the picture's (the format block's may be the source's).
    let (width, height) = match specular_mpeg2::probe(&config) {
        Some(_) if matches!(codec, Codec::Mpeg2 | Codec::Mpeg1) => (0, 0),
        _ => (width, height),
    };
    if annex_b && config.is_empty() {
        let units: Vec<&[u8]> = heads.iter().map(Vec::as_slice).collect();
        config = crate::mxf::avcc(&units);
    }
    time_samples(&mut list, 400_000);
    let duration = list
        .iter()
        .map(|s| s.cts.max(0) as u64 + u64::from(s.duration))
        .max()
        .unwrap_or(0);
    VideoTrack {
        codec: Some(codec),
        config,
        width,
        height,
        display: (width, height),
        timescale: TIMESCALE,
        duration,
        sample_count: list.len() as u32,
        samples: if samples { list } else { Vec::new() },
        layout: Layout::Wtv(WtvLayout { annex_b, ..layout }),
        edit: Edit::default(),
        ..VideoTrack::default()
    }
}

fn audio_track(
    tl: &WtvFile,
    sid: u32,
    (codec, channels, rate): (AudioCodec, u16, u32),
    pieces: &[Piece],
    origin: i64,
    samples: bool,
) -> AudioTrack {
    let mut list: Vec<Sample> = groups(pieces)
        .into_iter()
        .take(MAX_SAMPLES)
        .map(|(at, size, pts)| Sample {
            offset: at,
            size: size.min(u64::from(u32::MAX)) as u32,
            dts: 0,
            cts: pts - origin,
            duration: 0,
            sync: true,
        })
        .collect();
    time_samples(&mut list, 240_000);
    AudioTrack {
        codec,
        channels,
        sample_rate: rate,
        timescale: TIMESCALE,
        duration: list
            .iter()
            .map(|s| s.cts.max(0) as u64 + u64::from(s.duration))
            .max()
            .unwrap_or(0),
        sample_count: list.len() as u32,
        samples: if samples { list } else { Vec::new() },
        edit: Edit::default(),
        layout: Layout::Wtv(WtvLayout {
            timeline: tl.clone(),
            stream: sid,
            annex_b: false,
        }),
        extra: Default::default(),
    }
}

/// The programme's tags from `legacy_attrib` (text, numbers and dates; pictures and GUIDs
/// left out).
fn attributes(b: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut p = 0;
    while p + 24 <= b.len() && b[p..p + 16] == ATTRIBUTE && out.len() < 512 {
        let (Some(kind), Some(len)) = (u32le(b, p + 16), u32le(b, p + 20)) else {
            break;
        };
        let mut q = p + 24;
        while q + 1 < b.len() && (b[q] != 0 || b[q + 1] != 0) {
            q += 2;
        }
        let name = utf16(&b[p + 24..q.min(b.len())]);
        let from = q + 2;
        let Some(v) = from
            .checked_add(len as usize)
            .and_then(|to| b.get(from..to))
        else {
            break;
        };
        let value = match kind {
            0 | 3 if v.len() >= 4 => Some(u32le(v, 0).unwrap_or(0).to_string()),
            1 => Some(utf16(v)),
            4 if v.len() >= 8 => Some(u64le(v, 0).unwrap_or(0).to_string()),
            5 if v.len() >= 2 => Some(u16::from_le_bytes([v[0], v[1]]).to_string()),
            _ => None,
        };
        if let Some(v) = value.filter(|v| !v.is_empty()) {
            out.push((name, v));
        }
        p = from + len as usize;
    }
    out
}

fn utf16(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&c| c != 0)
        .collect();
    String::from_utf16_lossy(&units).trim().to_owned()
}

/// A FILETIME (100 ns since 1601) as ISO 8601 UTC.
fn filetime(t: u64) -> Option<String> {
    let secs = (t / 10_000_000).checked_sub(11_644_473_600)? as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // civil from days (proleptic Gregorian)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (1990..2200).contains(&y).then(|| {
        format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
            rem / 3600,
            rem / 60 % 60,
            rem % 60
        )
    })
}

/// The tags shown for a recording, under the names Info and the window title read.
fn programme_tags(raw: &[(String, String)]) -> (Vec<(String, String)>, Option<String>) {
    let get = |k: &str| raw.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let mut tags = Vec::new();
    for (from, to) in [
        ("Title", "Title"),
        ("WM/SubTitle", "Episode"),
        ("WM/SubTitleDescription", "Description"),
        ("WM/Genre", "Genre"),
        ("WM/MediaStationName", "Channel"),
        ("WM/MediaStationCallSign", "Channel call sign"),
        ("WM/MediaOriginalBroadcastDateTime", "First broadcast"),
        ("WM/MediaCredits", "Credits"),
    ] {
        if let Some(v) = get(from).filter(|v| !v.is_empty() && !v.starts_with("0001-")) {
            tags.push((to.to_owned(), v));
        }
    }
    let date = get("WM/WMRVEncodeTime")
        .and_then(|v| v.parse::<u64>().ok())
        .and_then(filetime);
    (tags, date)
}

/// The sound streams with data, in order (ids for [`parse_with`]).
fn sound_streams(s: &Scan) -> Vec<(u32, (AudioCodec, u16, u32))> {
    s.order
        .iter()
        .filter_map(|id| match s.media.get(id) {
            Some(Media::Audio {
                codec,
                channels,
                rate,
            }) => Some((*id, (codec.clone(), *channels, *rate))),
            _ => None,
        })
        .collect()
}

/// A recording's sound tracks (to choose one).
pub fn audio_tracks(data: &[u8]) -> Vec<TrackInfo> {
    let Ok(files) = directory(data) else {
        return Vec::new();
    };
    let Some(tl) = files.get("timeline") else {
        return Vec::new();
    };
    let Ok(s) = scan(data, tl) else {
        return Vec::new();
    };
    sound_streams(&s)
        .into_iter()
        .enumerate()
        .map(|(i, (id, (codec, channels, _)))| TrackInfo {
            id: u64::from(id),
            codec: codec.name(),
            default: i == 0,
            channels,
            ..TrackInfo::default()
        })
        .collect()
}

/// Parses a WTV recording; `audio` picks a sound stream by id.
pub fn parse_with(data: &[u8], samples: bool, audio: Option<u64>) -> Result<Movie> {
    if !sniff(data) {
        return Err(bad("not a WTV file"));
    }
    let files = directory(data)?;
    let tl = files
        .get("timeline")
        .ok_or_else(|| bad("WTV file without its timeline"))?;
    let raw = files
        .get("table.0.entries.legacy_attrib")
        .and_then(|f| f.all(data))
        .map(|a| attributes(&a))
        .unwrap_or_default();
    // Copy-once broadcasts are recorded encrypted: the share of the programme that is.
    let protected = raw.iter().any(|(n, v)| {
        n == "WM/WMRVContentProtectedPercent" && v.parse::<u32>().is_ok_and(|p| p > 0)
    });
    if protected {
        return Err(Error::Unsupported("copy-protected TV recording".into()));
    }
    let s = scan(data, tl)?;
    let video = s.order.iter().find_map(|id| match s.media.get(id) {
        Some(Media::Video {
            codec,
            config,
            width,
            height,
        }) => Some((*id, *codec, config.clone(), (*width, *height))),
        _ => None,
    });
    let sounds = sound_streams(&s);
    let sound = audio
        .and_then(|want| sounds.iter().find(|(id, _)| u64::from(*id) == want))
        .or(sounds.first())
        .cloned();
    let first_pts = |id: u32| s.pieces.get(&id).and_then(|l| l.iter().find_map(|p| p.pts));
    let origin = [video.as_ref().map(|v| v.0), sound.as_ref().map(|a| a.0)]
        .into_iter()
        .flatten()
        .filter_map(first_pts)
        .min()
        .unwrap_or(0);
    let mut movie = Movie {
        brand: *b"WTV ",
        ..Movie::default()
    };
    if let Some((id, codec, config, size)) = video {
        let pieces = s.pieces.get(&id).map_or(&[][..], Vec::as_slice);
        movie.video = Some(video_track(
            data, tl, id, codec, config, size, pieces, origin, samples,
        ));
    }
    if let Some((id, a)) = sound {
        let pieces = s.pieces.get(&id).map_or(&[][..], Vec::as_slice);
        movie.audio = Some(audio_track(tl, id, a, pieces, origin, samples));
        movie.has_audio = true;
    }
    if movie.video.is_none() && movie.audio.is_none() {
        return Err(Error::Unsupported(
            "WTV recording without video or sound".into(),
        ));
    }
    (movie.tags, movie.date) = programme_tags(&raw);
    let vd = movie.video.as_ref().and_then(|v| v.duration_us());
    let ad = movie
        .audio
        .as_ref()
        .and_then(|a| (a.timescale > 0).then(|| a.us(a.duration as i64).max(0) as u64));
    movie.duration_us = vd.or(ad);
    Ok(movie)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        // 2008-08-12 23:36:55 UTC
        let t = (1_218_584_215u64 + 11_644_473_600) * 10_000_000;
        assert_eq!(filetime(t).as_deref(), Some("2008-08-12T23:36:55Z"));
        assert_eq!(filetime(0), None);
    }

    #[test]
    fn hostile_directory() {
        // a header pointing at itself, entries that loop or run past the end
        let mut d = vec![0u8; 0x3000];
        d[..16].copy_from_slice(&FILE);
        d[0x38..0x3C].copy_from_slice(&1u32.to_le_bytes());
        let e = 0x1000;
        d[e..e + 16].copy_from_slice(&ENTRY);
        d[e + 16..e + 24].copy_from_slice(&0x40u64.to_le_bytes());
        // `timeline`, 4 GiB, depth 1 at sector 0
        d[e + 24..e + 32].copy_from_slice(&0x1_0000_0000u64.to_le_bytes());
        d[e + 32..e + 36].copy_from_slice(&8u32.to_le_bytes());
        for (i, c) in "timeline".encode_utf16().enumerate() {
            d[e + 40 + 2 * i..e + 42 + 2 * i].copy_from_slice(&c.to_le_bytes());
        }
        d[e + 60..e + 64].copy_from_slice(&1u32.to_le_bytes());
        assert!(parse_with(&d, true, None).is_err());
        // depth 7
        d[e + 24..e + 32].copy_from_slice(&0x100u64.to_le_bytes());
        d[e + 60..e + 64].copy_from_slice(&7u32.to_le_bytes());
        assert!(parse_with(&d, true, None).is_err());
        // a sane empty timeline
        d[e + 60..e + 64].copy_from_slice(&0u32.to_le_bytes());
        assert!(parse_with(&d, true, None).is_err());
    }

    #[test]
    fn attributes_read() {
        let mut b = Vec::new();
        let mut rec = |kind: u32, name: &str, v: &[u8]| {
            b.extend_from_slice(&ATTRIBUTE);
            b.extend_from_slice(&kind.to_le_bytes());
            b.extend_from_slice(&(v.len() as u32).to_le_bytes());
            for c in name.encode_utf16().chain([0]) {
                b.extend_from_slice(&c.to_le_bytes());
            }
            b.extend_from_slice(v);
        };
        let text = |s: &str| -> Vec<u8> {
            s.encode_utf16()
                .chain([0])
                .flat_map(|c| c.to_le_bytes())
                .collect()
        };
        rec(1, "Title", &text("News"));
        rec(1, "WM/SubTitle", &text("Late"));
        rec(2, "WM/Picture", &[1, 2, 3]);
        rec(
            4,
            "WM/WMRVEncodeTime",
            &128_634_000_000_000_000u64.to_le_bytes(),
        );
        let raw = attributes(&b);
        let (tags, date) = programme_tags(&raw);
        assert_eq!(tags[0], ("Title".into(), "News".into()));
        assert_eq!(tags[1], ("Episode".into(), "Late".into()));
        assert!(date.is_some_and(|d| d.starts_with("2008-")));
        // cut short: what is whole is kept
        assert_eq!(attributes(&b[..b.len() - 3]).len(), 2);
    }
}
