//! Raw video clips: Canon Cinema RAW Light (`.crm`) and Apple ProRes RAW (QuickTime with a
//! `aprn` or `aprh` track).
//!
//! **Cinema RAW Light** (C200, C300 III, C500 II, C70, R5 C…) is the CR3 container
//! (`ftyp crx `, the Canon `uuid` box with CMT1–4 metadata and a thumbnail) with the
//! movie's tracks: a `CRAW` video track whose sample entry carries the `CMP1` CRX header,
//! every sample one frame coded with CRX (the CR3 sensor codec, [`crate::crx`]), sound
//! tracks of big-endian PCM, a timecode track and a `CTMD` timed-metadata track. The
//! `CNCV` box names it `CanonCRM…` (a CR3 says `CanonCR3…`). The sensor facts (crop, black,
//! white balance) come from the maker note as for CR3; every frame shares them.
//!
//! **ProRes RAW** frames carry their own header ([`specular_prores::raw`]): size, Bayer
//! layout, sensel range, white-balance factors and a camera → XYZ matrix. The camera and
//! recorder are in the movie's `meta` keys (Apple's `com.apple.proapps.*` and SMPTE RDD 18
//! names).
//!
//! Frames whose bytes reach past the end of the file (a clip cut short while copying) are
//! left out; [`crate::Clip::listed`] keeps how many the sample table lists.

use std::ops::Range;

use super::{Black, ColorSource, Sensor, Storage};
use crate::bmff::{self, BoxRef};
use crate::cfa::Cfa;
use crate::error::{RawError, Result};
use crate::tiff::{be_u16, be_u32, be_u64};
use crate::{Clip, RawFormat, RawInfo, Rect, SensorCompression};

/// How much of each end of the file the content check reads for a ProRes RAW sample entry.
const PROBE_SPAN: usize = 1 << 20;

/// Whether `d` is a Canon Cinema RAW Light clip: the CR3 brand and Canon's `CanonCRM`
/// compressor name.
pub(crate) fn is_crm(d: &[u8]) -> bool {
    d.get(4..12) == Some(b"ftypcrx ") && d[..d.len().min(512)].windows(8).any(|w| w == b"CanonCRM")
}

/// Whether `d` is a QuickTime movie with a ProRes RAW track (its sample description, in the
/// first or last megabyte: the movie header is at either end).
pub(crate) fn is_prores_raw(d: &[u8]) -> bool {
    if !matches!(
        d.get(4..8),
        Some(b"ftyp" | b"wide" | b"mdat" | b"moov" | b"free" | b"skip")
    ) {
        return false;
    }
    let head = &d[..d.len().min(PROBE_SPAN)];
    let tail = &d[d.len().saturating_sub(PROBE_SPAN)..];
    has_prores_raw_stsd(head) || has_prores_raw_stsd(tail)
}

fn has_prores_raw_stsd(b: &[u8]) -> bool {
    let mut at = 0;
    while let Some(i) = b[at..].windows(4).position(|w| w == b"stsd") {
        let p = at + i;
        // stsd: version/flags (4), entry count (4), then the entry: size (4), format (4).
        if matches!(b.get(p + 16..p + 20), Some(b"aprn" | b"aprh")) {
            return true;
        }
        at = p + 4;
    }
    false
}

/// One track of a movie.
struct Track {
    handler: [u8; 4],
    /// The first sample entry.
    entry: Option<BoxRef>,
    samples: Vec<Range<usize>>,
    timescale: u32,
    /// Media duration in `timescale` units.
    duration: u64,
    /// The first sample's duration (`stts`), in `timescale` units.
    delta: u32,
}

fn u32_at(d: &[u8], o: usize) -> u32 {
    be_u32(d, o).unwrap_or(0)
}

/// The tracks of `moov`, with their samples' byte ranges (stsc, stsz, stco or co64).
fn tracks(d: &[u8], moov: &BoxRef) -> Vec<Track> {
    let mut out = Vec::new();
    for t in bmff::boxes(d, moov.payload.clone())
        .into_iter()
        .filter(|b| &b.typ == b"trak")
    {
        let Some(mdia) = bmff::find(d, t.payload.clone(), b"mdia") else {
            continue;
        };
        let handler = bmff::find(d, mdia.payload.clone(), b"hdlr")
            .and_then(|h| d.get(h.payload.start + 8..h.payload.start + 12))
            .and_then(|s| s.try_into().ok())
            .unwrap_or_default();
        let (timescale, duration) = bmff::find(d, mdia.payload.clone(), b"mdhd")
            .map(|m| {
                let p = m.payload.start;
                if d.get(p) == Some(&1) {
                    (u32_at(d, p + 20), be_u64(d, p + 24).unwrap_or(0))
                } else {
                    (u32_at(d, p + 12), u64::from(u32_at(d, p + 16)))
                }
            })
            .unwrap_or((0, 0));
        let Some(stbl) = bmff::path(d, mdia.payload.clone(), &[b"minf", b"stbl"]) else {
            continue;
        };
        let entry = bmff::find(d, stbl.payload.clone(), b"stsd").and_then(|s| {
            bmff::boxes(d, s.payload.start.saturating_add(8)..s.payload.end)
                .into_iter()
                .next()
        });
        out.push(Track {
            handler,
            entry,
            samples: samples(d, stbl.payload.clone()).unwrap_or_default(),
            timescale,
            duration,
            delta: bmff::find(d, stbl.payload.clone(), b"stts")
                .filter(|t| u32_at(d, t.payload.start + 4) > 0)
                .map_or(0, |t| u32_at(d, t.payload.start + 12)),
        });
    }
    out
}

/// Every sample's byte range, in order.
fn samples(d: &[u8], stbl: Range<usize>) -> Option<Vec<Range<usize>>> {
    let stsz = bmff::find(d, stbl.clone(), b"stsz")?;
    let z = stsz.payload.start;
    let fixed = be_u32(d, z + 4)? as usize;
    let count = be_u32(d, z + 8)? as usize;
    if fixed == 0 && count > (stsz.payload.len().saturating_sub(12)) / 4 {
        return None;
    }
    let size = |i: usize| {
        if fixed != 0 {
            fixed
        } else {
            u32_at(d, z + 12 + 4 * i) as usize
        }
    };
    let (offsets, wide) = match bmff::find(d, stbl.clone(), b"co64") {
        Some(c) => (c, true),
        None => (bmff::find(d, stbl.clone(), b"stco")?, false),
    };
    let o = offsets.payload.start;
    let chunks = be_u32(d, o + 4)? as usize;
    let each = if wide { 8 } else { 4 };
    if chunks > offsets.payload.len().saturating_sub(8) / each {
        return None;
    }
    let chunk_at = |i: usize| -> usize {
        if wide {
            be_u64(d, o + 8 + 8 * i).unwrap_or(0) as usize
        } else {
            u32_at(d, o + 8 + 4 * i) as usize
        }
    };
    let stsc = bmff::find(d, stbl, b"stsc")?;
    let c = stsc.payload.start;
    let runs = (be_u32(d, c + 4)? as usize).min(stsc.payload.len().saturating_sub(8) / 12);
    let run = |i: usize| {
        (
            u32_at(d, c + 8 + 12 * i) as usize,
            u32_at(d, c + 12 + 12 * i) as usize,
        )
    };
    // A fixed sample size over many samples needs no table: bound the count by the chunks.
    let mut out = Vec::with_capacity(count.min(1 << 20));
    let mut k = 0;
    let mut r = 0;
    for chunk in 0..chunks {
        while r + 1 < runs && run(r + 1).0 <= chunk + 1 {
            r += 1;
        }
        let per = if runs == 0 { 0 } else { run(r).1 };
        let mut at = chunk_at(chunk);
        for _ in 0..per {
            if k >= count {
                return Some(out);
            }
            let s = size(k);
            let end = at.checked_add(s)?;
            out.push(at..end);
            at = end;
            k += 1;
        }
    }
    Some(out)
}

/// The leading frames that lie wholly in the file.
fn present(d: &[u8], frames: &[Range<usize>]) -> Vec<Range<usize>> {
    frames
        .iter()
        .take_while(|r| r.end <= d.len() && r.start < r.end)
        .cloned()
        .collect()
}

/// Frame rate and duration of a video track.
/// Frame rate from the sample durations (`stts`), else the media duration over the samples;
/// the length the sample table lists.
fn timing(t: &Track) -> (Option<f64>, Option<u64>) {
    if t.timescale == 0 || t.samples.is_empty() {
        return (None, None);
    }
    let rate = if t.delta > 0 {
        f64::from(t.timescale) / f64::from(t.delta)
    } else if t.duration > 0 {
        t.samples.len() as f64 * f64::from(t.timescale) / t.duration as f64
    } else {
        return (None, None);
    };
    let secs = t.samples.len() as f64 / rate;
    (Some(rate), Some((secs * 1e6).round() as u64))
}

/// The sound tracks, described: "PCM, 4 × 1 ch, 48 kHz".
fn sound(d: &[u8], tracks: &[Track]) -> Option<String> {
    let mut found: Vec<(String, u16, u32)> = Vec::new();
    for t in tracks.iter().filter(|t| &t.handler == b"soun") {
        let Some(e) = &t.entry else { continue };
        let p = e.payload.start;
        // Sound sample entry: 8 reserved, version (2), revision (2), vendor (4), channels (2),
        // sample size (2), compression (2), packet size (2), rate (16.16).
        let channels = be_u16(d, p + 16).unwrap_or(0);
        let rate = u32_at(d, p + 24) >> 16;
        let codec = match &e.typ {
            b"twos" | b"sowt" | b"in24" | b"in32" | b"lpcm" | b"raw " => "PCM".to_string(),
            b"mp4a" => "AAC".to_string(),
            other => String::from_utf8_lossy(other).trim().to_string(),
        };
        found.push((codec, channels, rate));
    }
    let (codec, ch, rate) = found.first()?.clone();
    let khz = if rate % 1000 == 0 {
        format!("{} kHz", rate / 1000)
    } else {
        format!("{:.1} kHz", f64::from(rate) / 1000.0)
    };
    let same = found
        .iter()
        .all(|f| f.0 == codec && f.1 == ch && f.2 == rate);
    Some(if found.len() > 1 && same {
        format!("{codec}, {} × {ch} ch, {khz}", found.len())
    } else {
        format!("{codec}, {ch} ch, {khz}")
    })
}

fn moov(d: &[u8]) -> Result<BoxRef> {
    bmff::boxes(d, 0..d.len())
        .into_iter()
        .find(|b| &b.typ == b"moov")
        .ok_or(RawError::Malformed("movie without its header (moov)"))
}

// ---------------------------------------------------------------------------------------------
// Canon Cinema RAW Light

pub(crate) fn parse_crm(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    let (mut info, mut sensor) = super::parse_cr3(d)?;
    info.format = Some(RawFormat::Crm);
    let moov = moov(d)?;
    let tracks = tracks(d, &moov);
    let video = tracks
        .iter()
        .filter(|t| t.entry.as_ref().is_some_and(|e| &e.typ == b"CRAW"))
        .max_by_key(|t| t.samples.len());
    let Some(video) = video else {
        return Err(RawError::Malformed(
            "Cinema RAW Light clip without a video track",
        ));
    };
    let frames = present(d, &video.samples);
    let (frame_rate, duration_us) = timing(video);
    let mut assumed_black = None;
    if let Some(s) = sensor.as_mut() {
        // The CR3 reader's reasons, for a clip.
        if let Some(why) = s.unsupported {
            s.unsupported = Some(match why {
                w if w.contains("signed CRX") => {
                    "Cinema RAW Light: signed CRX samples (no camera file or specification known)"
                }
                w if w.contains("header variant") => {
                    "Cinema RAW Light: this camera's CRX header variant is not supported"
                }
                _ => "Cinema RAW Light: this frame layout is not supported",
            });
        }
        if let Storage::Crx { range, .. } = &mut s.storage
            && let Some(f) = frames.first()
        {
            *range = f.clone();
        }
        // The clip's own facts: no maker-note colour data (black, white balance) and no
        // sensor area. The recorded picture is the Exif size, centred on the coded frame.
        if s.crop
            == (Rect {
                x: 0,
                y: 0,
                w: s.width,
                h: s.height,
            })
            && let Some((w, h)) = exif_size(d, &info)
            && w <= s.width
            && h <= s.height
        {
            let (x, y) = (((s.width - w) / 2) & !1, ((s.height - h) / 2) & !1);
            s.crop = Rect {
                x,
                y,
                w: w & !1,
                h: h & !1,
            };
        }
        if matches!(s.black, Black::Levels(b) if b == [0.0; 4]) && s.bits >= 8 {
            // Canon states none; the video convention (a sixteenth of the range: 64 at 10
            // bits, 256 at 12) fits the darkest samples of the clips known.
            s.black = Black::Levels([(1u32 << (s.bits - 4)) as f32; 4]);
            assumed_black = Some(1u32 << (s.bits - 4));
        }
    }
    let mut facts = Vec::new();
    if let Some(h) = sensor.as_ref().and_then(|s| match &s.storage {
        Storage::Crx { header, .. } => Some(*header),
        _ => None,
    }) {
        facts.push((
            "coding",
            if h.levels > 0 {
                format!("CRX wavelet, {} levels", h.levels)
            } else {
                "CRX lossless".into()
            },
        ));
    }
    if let Some(b) = assumed_black {
        facts.push(("black level", format!("{b} (assumed)")));
    }
    info.sensor = sensor.as_ref().map(Sensor::info);
    info.clip = Some(Clip {
        codec: "Canon Cinema RAW Light".into(),
        listed: video.samples.len().min(u32::MAX as usize) as u32,
        frames,
        frame_rate,
        duration_us,
        audio: sound(d, &tracks),
        facts,
    });
    Ok((info, sensor))
}

/// The picture size the Exif block (CMT2) gives (`PixelXDimension`, `PixelYDimension`).
fn exif_size(d: &[u8], info: &RawInfo) -> Option<(u32, u32)> {
    info.tiff_blocks.iter().find_map(|r| {
        let b = d.get(r.clone())?;
        let (le, first) = crate::tiff::header(b, 0)?;
        let ifd = crate::tiff::read_ifd(b, first, 0, le)?;
        let w = ifd.uint(b, 0xa002)?;
        let h = ifd.uint(b, 0xa003)?;
        (w > 0 && h > 0).then_some((w, h))
    })
}

// ---------------------------------------------------------------------------------------------
// Apple ProRes RAW

/// The movie's `meta` keys and values that name the camera (strings and numbers as text).
fn meta_items(d: &[u8], moov: &BoxRef) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(meta) = bmff::find(d, moov.payload.clone(), b"meta") else {
        return out;
    };
    // QuickTime's `meta` is a plain box; ISO's is a full box (4 bytes of version and flags).
    let mut inner = meta.payload.clone();
    if d.get(inner.start + 4..inner.start + 8) != Some(b"hdlr") {
        inner.start += 4;
    }
    let (Some(keys), Some(ilst)) = (
        bmff::find(d, inner.clone(), b"keys"),
        bmff::find(d, inner, b"ilst"),
    ) else {
        return out;
    };
    let mut names = Vec::new();
    let mut p = keys.payload.start + 8;
    while p + 8 <= keys.payload.end {
        let size = u32_at(d, p) as usize;
        if size < 8 || p + size > keys.payload.end {
            break;
        }
        names.push(String::from_utf8_lossy(&d[p + 8..p + size]).into_owned());
        p += size;
    }
    for item in bmff::boxes(d, ilst.payload.clone()) {
        let index = u32::from_be_bytes(item.typ) as usize;
        let Some(name) = index.checked_sub(1).and_then(|i| names.get(i)) else {
            continue;
        };
        let Some(data) = bmff::find(d, item.payload.clone(), b"data") else {
            continue;
        };
        let p = data.payload.start;
        let kind = u32_at(d, p) & 0xff_ffff;
        let Some(v) = d.get(p + 8..data.payload.end) else {
            continue;
        };
        let text = match kind {
            1 => String::from_utf8_lossy(v)
                .trim_end_matches('\0')
                .trim()
                .to_string(),
            23 if v.len() == 4 => format!("{}", f32::from_be_bytes([v[0], v[1], v[2], v[3]])),
            24 if v.len() == 8 => {
                format!("{}", f64::from_be_bytes(v.try_into().unwrap_or_default()))
            }
            21 | 22 if v.len() <= 8 => {
                let mut n = 0u64;
                for &b in v {
                    n = n << 8 | u64::from(b);
                }
                n.to_string()
            }
            _ => continue,
        };
        if !text.is_empty() {
            out.push((name.clone(), text));
        }
    }
    out
}

pub(crate) fn parse_prores_raw(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    let moov = moov(d)?;
    let tracks = tracks(d, &moov);
    let Some(video) = tracks.iter().find(|t| {
        t.entry
            .as_ref()
            .is_some_and(|e| matches!(&e.typ, b"aprn" | b"aprh"))
    }) else {
        return Err(RawError::NotRaw);
    };
    let hq = video.entry.as_ref().is_some_and(|e| &e.typ == b"aprh");
    let mut info = RawInfo {
        format: Some(RawFormat::ProResRaw),
        orientation: 1,
        ..Default::default()
    };
    let items = meta_items(d, &moov);
    let item = |k: &str| {
        items
            .iter()
            .find(|(n, _)| n.as_str() == k)
            .map(|(_, v)| v.clone())
    };
    // The camera: Apple's manufacturer key (the recorder's own Make and Model are the
    // `{TIFF}` keys when an external recorder wrote the file).
    let recorder = [
        item("com.apple.proapps.image.{TIFF}.Make"),
        item("com.apple.proapps.image.{TIFF}.Model"),
        item("com.apple.proapps.image.{TIFF}.Software"),
    ];
    info.make = item("com.apple.proapps.manufacturer")
        .or_else(|| recorder[0].clone())
        .unwrap_or_default();
    if let Some(m) = item("com.apple.proapps.modelname") {
        info.model = m;
    } else if item("com.apple.proapps.manufacturer").is_none() {
        info.model = recorder[1].clone().unwrap_or_default();
    }
    let recorded_by = recorder
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    if !recorded_by.is_empty() && item("com.apple.proapps.manufacturer").is_some() {
        info.software = Some(recorded_by);
    }
    info.iso = item("org.smpte.rdd18.camera.isosensitivity")
        .or_else(|| item("com.apple.proapps.exif.{Exif}.ExposureIndex"))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0 && *v < 1e7)
        .map(|v| v.round() as u32);
    info.f_number = item("com.apple.proapps.exif.{Exif}.FNumber")
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| *v > 0.0 && *v < 1000.0);
    let frames = present(d, &video.samples);
    let (frame_rate, duration_us) = timing(video);
    let mut facts = Vec::new();
    for (key, name) in [
        ("org.smpte.rdd18.camera.whitebalance", "white balance"),
        ("org.smpte.rdd18.camera.shutterspeed_angle", "shutter angle"),
        (
            "org.smpte.rdd18.camera.neutraldensityfilterwheelsetting",
            "ND filter",
        ),
    ] {
        if let Some(v) = item(key) {
            facts.push((name, v));
        }
    }
    let codec = if hq {
        "Apple ProRes RAW HQ"
    } else {
        "Apple ProRes RAW"
    };
    let first = frames.first().and_then(|r| d.get(r.clone()));
    let header = first.map(specular_prores::raw::raw_header);
    let sensor = match header {
        None => None,
        Some(Err(e)) => {
            let why = match e {
                specular_prores::Error::Unsupported(_) => {
                    "ProRes RAW: a frame version this decoder does not read"
                }
                _ => "ProRes RAW: the first frame's header is damaged",
            };
            Some(Sensor::new(0, 0, 16, SensorCompression::ProResRaw).unsupported(why))
        }
        Some(Ok(h)) => {
            facts.push((
                "encoder",
                String::from_utf8_lossy(&h.vendor).trim().to_string(),
            ));
            if h.cct > 0 {
                facts.push(("white balance", format!("{} K", h.cct)));
            }
            if h.gain.is_finite() && h.gain > 0.0 && h.gain != 1.0 {
                facts.push(("gain", format!("{:.2}×", h.gain)));
            }
            Some(prores_sensor(&h, frames[0].clone()))
        }
    };
    info.clip = Some(Clip {
        codec: codec.into(),
        listed: video.samples.len().min(u32::MAX as usize) as u32,
        frames,
        frame_rate,
        duration_us,
        audio: sound(d, &tracks),
        facts,
    });
    info.sensor = sensor.as_ref().map(Sensor::info);
    Ok((info, sensor))
}

fn prores_sensor(h: &specular_prores::raw::RawHeader, range: Range<usize>) -> Sensor {
    let mut s = Sensor::new(h.width, h.height, 16, SensorCompression::ProResRaw);
    s.cfa = Cfa::bayer(h.cfa_name());
    // Margins to even sizes, keeping the Bayer phase.
    let [l, r, t, b] = h.crop.map(u32::from);
    let (x, y) = ((l + 1) & !1, (t + 1) & !1);
    if x + r < h.width && y + b < h.height {
        let w = (h.width - x - r) & !1;
        let hh = (h.height - y - b) & !1;
        if w > 0 && hh > 0 {
            s.crop = Rect { x, y, w, h: hh };
        }
    }
    s.black = Black::Levels([f32::from(specular_prores::raw::BLACK); 4]);
    s.white = Some(h.white as f32);
    let [wr, wbl] = h.wb;
    if wr.is_finite() && wbl.is_finite() && wr > 0.0 && wbl > 0.0 {
        s.wb = Some([wr, 1.0, wbl]);
    }
    if let Some(m) = crate::color::xyz_rgb_cam(&h.matrix) {
        s.color = ColorSource::Matrix(m, crate::MatrixSource::CameraCalibration);
    }
    if h.bayer != 0 {
        // Only RGGB is known in files; the layout field's other values are untested.
        s.unsupported = Some("ProRes RAW: a Bayer layout other than RGGB (no sample known)");
    }
    s.storage = Storage::ProResRaw { range };
    s
}
