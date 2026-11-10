//! FLIR sequences: `.seq` (FFF frames with raw counts, one after another) and `.csq` (the
//! same with each frame's counts JPEG-LS coded).
//!
//! Every frame is a complete FFF: a header, its record directory and the records, with
//! offsets from the frame's own start. A frame ends after its last record; the next one
//! starts at the next `FFF\0` (cameras put them back to back, some pad). Indexing walks the
//! headers and directories only and reads each frame's capture time from its CameraInfo
//! record, so a file of thousands of frames indexes without touching its pixels. A frame is
//! read in full (counts, its own Planck constants and object parameters) when asked for.

use crate::flir::{self, Rec, is_fff};
use crate::{Conditions, Error, Kind, Result, Thermal, malformed};

const F: &str = "FLIR sequence";

/// Frames a second assumed when the file records neither times nor a rate.
pub const DEFAULT_RATE: f64 = 30.0;

/// Where one frame lies and when it was taken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameRef {
    pub offset: usize,
    pub len: usize,
    /// Capture time, Unix seconds with the milliseconds, when the frame records one.
    pub time: Option<f64>,
    /// The object parameters this frame records (they may change while recording).
    pub conditions: Conditions,
}

/// The frames of a sequence file.
#[derive(Clone, Debug, PartialEq)]
pub struct Sequence {
    pub kind: Kind,
    pub frames: Vec<FrameRef>,
    pub width: usize,
    pub height: usize,
    /// Frames a second as the camera records it (CameraInfo), when it does.
    pub frame_rate: Option<f64>,
    /// Frames a second played: the recorded rate, else from the recorded times, else 30.
    pub rate: f64,
    /// Seconds from the first frame to each frame: from the recorded times when every frame
    /// has one and they run forward, else from `rate`.
    seconds: Vec<f64>,
}

/// A record's type, offset and length in its frame.
type Record = (u16, usize, usize);

/// The directory of one frame: (record type, offset, length) and where the frame ends (past
/// the end of `d` when it is cut short).
fn directory(d: &[u8]) -> Option<(Vec<Record>, usize)> {
    if !is_fff(d) {
        return None;
    }
    let head = flir::header(d);
    let dir = head.u32(0x18)? as usize;
    let count = head.u32(0x1c)? as usize;
    if count == 0 || count > d.len() / 32 || dir.checked_add(count * 32)? > d.len() {
        return None;
    }
    let mut recs = Vec::with_capacity(count);
    let mut end = dir + count * 32;
    for i in 0..count {
        let e = dir + 32 * i;
        let ty = head.u16(e)?;
        let off = head.u32(e + 0x0c)? as usize;
        let len = head.u32(e + 0x10)? as usize;
        if ty == 0 {
            continue;
        }
        end = end.max(off.saturating_add(len));
        recs.push((ty, off, len));
    }
    Some((recs, end))
}

/// The record of type `ty` in `frame` (the first one).
fn record<'a>(frame: &'a [u8], recs: &[Record], ty: u16) -> Option<&'a [u8]> {
    let &(_, off, len) = recs.iter().find(|r| r.0 == ty)?;
    frame.get(off..off.saturating_add(len).min(frame.len()))
}

/// Where the next frame starts at or after `from`.
fn next_frame(d: &[u8], from: usize) -> Option<usize> {
    let rest = d.get(from..)?;
    rest.windows(4)
        .position(|w| w == b"FFF\0")
        .map(|p| from + p)
}

/// Whether the RawData record holds a JPEG-LS stream (CSQ).
fn jpegls_raw(frame: &[u8], recs: &[Record]) -> bool {
    record(frame, recs, flir::RAW_DATA)
        .and_then(|r| r.get(32..36))
        .is_some_and(|b| b == [0xff, 0xd8, 0xff, 0xf7])
}

/// Whether `d` is a FLIR sequence: an FFF whose first frame is followed by another, or whose
/// counts are JPEG-LS coded (CSQ, even with one frame). Reads the first directory and looks
/// past the first frame only.
pub fn is_sequence(d: &[u8]) -> bool {
    let Some((recs, end)) = directory(d) else {
        return false;
    };
    if jpegls_raw(d, &recs) {
        return true;
    }
    end < d.len() && next_frame(d, end).is_some_and(|n| directory(&d[n..]).is_some())
}

/// SEQ or CSQ, by the first frame's counts.
pub(crate) fn kind(d: &[u8]) -> Kind {
    match directory(d) {
        Some((recs, _)) if jpegls_raw(d, &recs) => Kind::FlirCsq,
        _ => Kind::FlirSeq,
    }
}

/// The capture time a frame's CameraInfo record gives (Unix seconds and milliseconds).
fn frame_time(frame: &[u8], recs: &[Record]) -> Option<f64> {
    let r = Rec::checked(record(frame, recs, flir::CAMERA_INFO)?);
    let s = r.u32(0x384).filter(|&s| s != 0)?;
    let ms = r.u32(0x388).unwrap_or(0) % 1000;
    Some(f64::from(s) + f64::from(ms) / 1000.0)
}

/// Indexes the frames of a sequence.
pub fn index(d: &[u8]) -> Result<Sequence> {
    let (first, _) = directory(d).ok_or(Error::NotThermal)?;
    let kind = kind(d);
    let (width, height) = record(d, &first, flir::RAW_DATA)
        .map(Rec::checked)
        .and_then(|r| Some((usize::from(r.u16(2)?), usize::from(r.u16(4)?))))
        .ok_or(Error::NoRawData)?;
    let frame_rate = record(d, &first, flir::CAMERA_INFO)
        .map(Rec::checked)
        .and_then(|r| r.u16(0x464))
        .filter(|&r| r > 0 && r < 10_000)
        .map(f64::from);
    let mut frames = Vec::new();
    let mut pos = 0;
    while pos < d.len() {
        let frame = &d[pos..];
        let Some((recs, end)) = directory(frame) else {
            break;
        };
        if end > frame.len() {
            // The last frame was cut short (a recording stopped mid-write): left out.
            break;
        }
        let raw = record(frame, &recs, flir::RAW_DATA);
        if raw.is_none() {
            // A frame without counts (a stray header) is skipped.
            match next_frame(d, pos + end.max(4)) {
                Some(n) => {
                    pos = n;
                    continue;
                }
                None => break,
            }
        }
        frames.push(FrameRef {
            offset: pos,
            len: end,
            time: frame_time(frame, &recs),
            conditions: record(frame, &recs, flir::CAMERA_INFO)
                .map(|r| flir::camera_info_record(r).1)
                .unwrap_or_default(),
        });
        match next_frame(d, pos + end.max(4)) {
            Some(n) => pos = n,
            None => break,
        }
    }
    if frames.is_empty() {
        return Err(Error::NoRawData);
    }
    let (rate, seconds) = timing(&frames, frame_rate);
    Ok(Sequence {
        kind,
        frames,
        width,
        height,
        frame_rate,
        rate,
        seconds,
    })
}

/// The playing rate and each frame's offset in seconds (see [`Sequence::seconds`]).
fn timing(frames: &[FrameRef], recorded: Option<f64>) -> (f64, Vec<f64>) {
    // The recorded times, when every frame has one, they never run backwards and they do
    // not all coincide.
    let times: Option<Vec<f64>> = frames.iter().map(|f| f.time).collect();
    let times = times
        .filter(|t| t.windows(2).all(|w| w[1] >= w[0]) && t.len() > 1 && t[t.len() - 1] > t[0]);
    let rate = recorded
        .or_else(|| {
            let t = times.as_ref()?;
            Some((t.len() - 1) as f64 / (t[t.len() - 1] - t[0]))
        })
        .unwrap_or(DEFAULT_RATE);
    let seconds = match times {
        Some(t) => t.iter().map(|v| v - t[0]).collect(),
        None => (0..frames.len()).map(|i| i as f64 / rate).collect(),
    };
    (rate, seconds)
}

impl Sequence {
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Frame `i` read in full: its counts, Planck constants, object parameters and camera.
    pub fn frame(&self, d: &[u8], i: usize) -> Result<Thermal> {
        let f = self
            .frames
            .get(i)
            .ok_or_else(|| malformed(F, format!("no frame {i}")))?;
        let bytes = d
            .get(f.offset..f.offset + f.len)
            .ok_or_else(|| malformed(F, "frame past the end"))?;
        let t = flir::read_fff(bytes, self.kind)?;
        if (t.width, t.height) != (self.width, self.height) {
            return Err(malformed(
                F,
                format!(
                    "frame {i} is {} × {}, the first {} × {}",
                    t.width, t.height, self.width, self.height
                ),
            ));
        }
        Ok(t)
    }

    /// Seconds from the first frame to frame `i`.
    pub fn seconds(&self, i: usize) -> f64 {
        self.seconds[i]
    }

    /// How long frame `i` shows when played: the gap to the next frame (the last one as
    /// long as the one before), in seconds.
    pub fn duration(&self, i: usize) -> f64 {
        let n = self.frames.len();
        if n < 2 {
            return 1.0 / self.rate;
        }
        let j = i.min(n - 2);
        (self.seconds(j + 1) - self.seconds(j)).max(0.0)
    }
}

/// "YYYY-MM-DD HH:MM:SS.fff" (UTC) of Unix seconds.
pub fn utc_text(secs: f64) -> String {
    let ms = (secs * 1000.0).round() as i64;
    let (s, ms) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let (y, m, d) = flir::civil(s.div_euclid(86_400));
    let t = s.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}.{ms:03}",
        t / 3600,
        t / 60 % 60,
        t % 60
    )
}
