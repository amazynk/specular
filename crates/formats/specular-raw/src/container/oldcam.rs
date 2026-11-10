//! Old and unusual camera containers without TIFF structure:
//!
//! - Apple QuickTake (`.qtk`): `qktk` (QuickTake 100) or `qktn` (QuickTake 150/200) at the
//!   start, the picture size as big-endian 16-bit width and height at 18 and 20, then a
//!   QuickDraw picture whose QuickTime image description is followed by the codec's 12-byte
//!   header (the magic again, a version, the length) and the compressed sensor data. The 150/200 data is
//!   Kodak's radial compression (the DC40's, [`crate::radc`]). The QuickTake 100 codec has no
//!   public description: refused, with the 80 x 60 4-bit grey thumbnail the camera's
//!   standard-format files end with shown instead.
//! - Ricoh J6I: `\x80\x3eDSCIM` and a 512-byte camera header, then a plain JPEG.
//! - Lytro (`.lfp`, `.lfr`): `\x89LFP\r\n\x1a\n`, then sections (`\x89LFM` metadata,
//!   `\x89LFC` content), each a 16-byte header (magic, 4 bytes, big-endian length), an
//!   80-byte `sha1-...` name, the data and padding to 16 bytes. The JPEG pictures the camera
//!   or Lytro's software rendered are shown; the light-field sensor data is not developed.
//! - Minolta RD-175 (`.mdc`): by name and exact size, see [`crate::rd175`].
//! - Headerless `.raw` sensor dumps ([`HEADERLESS`]): by name, exact size and a check of the
//!   content (unused high bits, empty margins), measured on sample files.
//!
//! All layouts here come from public descriptions of the containers and our own analysis of
//! sample files.

use super::{Black, Codec, Packing, Previews, Sensor, Storage};
use crate::cfa::Cfa;
use crate::error::{RawError, Result};
use crate::vendor::VendorCodec;
use crate::{MatrixSource, Preview, PreviewKind, RawFormat, RawInfo, SensorCompression};

const LFP_MAGIC: &[u8; 8] = b"\x89LFP\r\n\x1a\n";

/// Where the QuickTake 150/200 sensor data usually starts (after the picture's image
/// description and the codec's 12-byte header).
const QT_DATA: usize = 736;

fn be16(d: &[u8], p: usize) -> usize {
    d.get(p..p + 2)
        .map_or(0, |s| u16::from_be_bytes([s[0], s[1]]) as usize)
}

fn be32(d: &[u8], p: usize) -> usize {
    d.get(p..p + 4)
        .map_or(0, |s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as usize)
}

/// The formats here that carry a signature.
pub(super) fn detect(d: &[u8]) -> Option<RawFormat> {
    if matches!(d.get(..4), Some(b"qktk" | b"qktn")) && d.len() > 0x300 {
        return Some(RawFormat::QuickTake);
    }
    if d.get(2..7) == Some(b"DSCIM") && d.get(512..514) == Some(&[0xff, 0xd8]) {
        return Some(RawFormat::J6i);
    }
    if d.starts_with(LFP_MAGIC) {
        return Some(RawFormat::Lytro);
    }
    None
}

/// A headerless sensor dump: file size, maker and model, size, packing, row stride, visible
/// width, CFA, black and white.
struct Headerless {
    size: usize,
    make: &'static str,
    model: &'static str,
    w: u32,
    h: u32,
    packing: Packing,
    row_bytes: usize,
    crop_w: u32,
    cfa: &'static str,
    black: f32,
    white: Option<f32>,
}

/// The headerless `.raw` files we know (layouts measured on sample files from
/// raw.pixls.us).
const HEADERLESS: &[Headerless] = &[
    // 12-bit big-endian bit order, rows of 3520 bytes; the right 22 columns are empty.
    Headerless {
        size: 6_114_240,
        make: "Pentax",
        model: "Optio S4",
        w: 2346,
        h: 1737,
        packing: Packing::Msb(12),
        row_bytes: 3520,
        crop_w: 2324,
        cfa: "RGGB",
        black: 0.0,
        white: None,
    },
    // 14-bit samples in little-endian words (Ambarella action cameras: Xiaomi Yi, GitUp
    // GIT2 at 4:3).
    Headerless {
        size: 31_850_496,
        make: "Ambarella",
        model: "action camera 4608 x 3456",
        w: 4608,
        h: 3456,
        packing: Packing::U16Le,
        row_bytes: 0,
        crop_w: 4608,
        cfa: "RGGB",
        black: 800.0,
        white: Some(16383.0),
    },
];

/// The content check of a headerless layout: what its samples cannot be (bits above 14 in
/// 16-bit words, anything in an empty margin), on a spread of rows.
fn plausible(d: &[u8], l: &Headerless) -> bool {
    let mut rows = (0..l.h as usize).step_by(97);
    match l.packing {
        Packing::U16Le => rows.all(|y| {
            let w = l.w as usize * 2;
            d.get(y * w..(y + 1) * w)
                .is_some_and(|row| row.chunks_exact(2).step_by(13).all(|c| c[1] < 0x40))
        }),
        Packing::Msb(12) => {
            let first = (l.crop_w as usize * 12).div_ceil(8) + 1;
            rows.all(|y| {
                d.get(y * l.row_bytes + first..(y + 1) * l.row_bytes)
                    .is_some_and(|m| m.iter().all(|&b| b == 0))
            })
        }
        _ => false,
    }
}

fn headerless_layout(d: &[u8], ext: &str) -> Option<&'static Headerless> {
    if !ext.eq_ignore_ascii_case("raw") {
        return None;
    }
    HEADERLESS
        .iter()
        .find(|l| l.size == d.len() && plausible(d, l))
}

/// Whether a `.raw` file is a headerless camera dump we read: name and size first, then a
/// check of the samples. Self-contained, for callers that rank the decoders claiming `.raw`.
pub(crate) fn is_headerless_camera_raw(d: &[u8], ext: &str) -> bool {
    headerless_layout(d, ext).is_some()
}

/// The formats recognised by name and size together.
pub(super) fn detect_named(d: &[u8], ext: &str) -> Option<RawFormat> {
    if ext.eq_ignore_ascii_case("mdc") && d.len() == crate::rd175::FILE_SIZE {
        return Some(RawFormat::Rd175);
    }
    headerless_layout(d, ext).map(|_| RawFormat::Headerless)
}

fn headerless(d: &[u8], ext: &str) -> Result<(RawInfo, Option<Sensor>)> {
    let l = headerless_layout(d, ext).ok_or(RawError::NotRaw)?;
    let info = RawInfo {
        make: l.make.into(),
        model: l.model.into(),
        ..Default::default()
    };
    let bits = if l.packing == Packing::Msb(12) {
        12
    } else {
        14
    };
    let mut s = Sensor::new(l.w, l.h, bits, SensorCompression::Uncompressed);
    s.cfa = Cfa::bayer(l.cfa);
    s.crop.w = l.crop_w;
    s.black = Black::Levels([l.black; 4]);
    s.white = l.white;
    s.storage = Storage::Blocks {
        blocks: std::iter::once(0..d.len()).collect(),
        tile_w: l.w,
        tile_h: l.h,
        codec: Codec::Raw {
            packing: l.packing,
            row_bytes: l.row_bytes,
        },
    };
    Ok((info, Some(s)))
}

pub(super) fn parse(d: &[u8], format: RawFormat) -> Result<(RawInfo, Option<Sensor>)> {
    parse_ext(d, format, "")
}

pub(super) fn parse_ext(
    d: &[u8],
    format: RawFormat,
    ext: &str,
) -> Result<(RawInfo, Option<Sensor>)> {
    let (mut info, sensor) = match format {
        RawFormat::Headerless => headerless(d, ext)?,
        RawFormat::QuickTake => quicktake(d)?,
        RawFormat::J6i => j6i(d)?,
        RawFormat::Lytro => lytro(d)?,
        RawFormat::Rd175 => rd175(d)?,
        _ => return Err(RawError::NotRaw),
    };
    info.format = Some(format);
    if info.orientation == 0 {
        info.orientation = 1;
    }
    info.sensor = sensor.as_ref().map(Sensor::info);
    Ok((info, sensor))
}

/// A sensor that is described but not decoded, for the reason given.
fn refused(why: &'static str) -> Sensor {
    Sensor::new(0, 0, 8, SensorCompression::Other(0)).unsupported(why)
}

fn quicktake(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    let magic = d.get(..4).ok_or(RawError::NotRaw)?;
    let (w, h) = (be16(d, 0x12) as u32, be16(d, 0x14) as u32);
    let qt100 = magic == b"qktk";
    let mut info = RawInfo {
        make: "Apple".into(),
        model: if qt100 {
            "QuickTake 100".into()
        } else {
            "QuickTake 150".into()
        },
        ..Default::default()
    };
    if qt100 {
        // The standard-format files (header version 4) end with the 80 x 60 thumbnail.
        if be32(d, 4) == 4 && d.len() > 0x400 {
            info.previews.push(Preview {
                kind: PreviewKind::Grey4,
                range: d.len() - 2400..d.len(),
                width: 80,
                height: 60,
                orientation: 1,
                source: "QuickTake thumbnail",
            });
        }
        return Ok((
            info,
            Some(refused(
                "QuickTake 100 compression has no public description",
            )),
        ));
    }
    if !(16..=1024).contains(&w) || !(16..=1024).contains(&h) || w % 4 != 0 || h % 4 != 0 {
        return Err(RawError::Malformed("QuickTake picture size"));
    }
    // The codec's own 12-byte header (the magic, a version, the data length) is the second
    // copy of the magic after the image description's.
    let data = d
        .get(0x200..d.len().min(0x400))
        .and_then(|s| {
            s.windows(4)
                .enumerate()
                .filter(|(_, x)| *x == magic)
                .nth(1)
                .map(|(p, _)| 0x200 + p + 12)
        })
        .unwrap_or(QT_DATA);
    let mut s = Sensor::new(w, h, 14, SensorCompression::Other(32867));
    s.cfa = Cfa::bayer("GRBG");
    s.white = Some(crate::radc::WHITE as f32);
    if let Some(m) = crate::color::extra_entry("Apple", "QuickTake")
        .and_then(|e| e.matrix.as_ref())
        .and_then(crate::color::adobe_rgb_cam)
    {
        s.color = super::ColorSource::Matrix(m, MatrixSource::Published);
    }
    s.storage = Storage::Vendor {
        range: data.min(d.len())..d.len(),
        codec: VendorCodec::KodakRadc { cbpp: 0 },
    };
    Ok((info, Some(s)))
}

fn j6i(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    let mut info = RawInfo {
        make: "Ricoh".into(),
        ..Default::default()
    };
    let mut pv = Previews::default();
    pv.jpeg_and_thumb(d, 512..d.len(), "J6I JPEG");
    info.previews = pv.finish(1);
    if info.previews.is_empty() {
        return Err(RawError::Malformed("J6I without its JPEG"));
    }
    Ok((info, Some(refused("the camera stores the picture as JPEG"))))
}

/// The value of `"key": "..."` in Lytro's JSON metadata (first occurrence).
fn json_str(meta: &str, key: &str) -> Option<String> {
    let at = meta.find(&format!("\"{key}\""))?;
    let rest = &meta[at + key.len() + 2..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let v = &rest[..rest.find('"')?];
    (!v.is_empty() && v.len() < 64).then(|| v.to_string())
}

fn lytro(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    let mut info = RawInfo {
        make: "Lytro".into(),
        ..Default::default()
    };
    let mut pv = Previews::default();
    let mut pos = 16usize;
    let mut sections = 0;
    while pos + 96 <= d.len() && sections < 4096 {
        let head = &d[pos..pos + 8];
        if head[0] != 0x89 || !matches!(&head[1..4], b"LFM" | b"LFC" | b"LFP") {
            break;
        }
        let len = be32(d, pos + 12);
        let start = pos + 96;
        let end = start.saturating_add(len).min(d.len());
        let body = &d[start..end];
        if body.starts_with(&[0xff, 0xd8, 0xff]) {
            pv.jpeg_and_thumb(d, start..end, "Lytro picture");
        } else if body.first() == Some(&b'{') && body.len() < 1 << 20 {
            let meta = String::from_utf8_lossy(body);
            if info.model.is_empty() {
                // Lytro writes its camera model under "camera" ("model": "ILLUM").
                if let Some(m) = json_str(&meta, "model") {
                    info.model = m;
                }
            }
            if info.iso.is_none()
                && let Some(i) = meta.find("\"iso\"")
            {
                let v: String = meta[i + 5..]
                    .chars()
                    .skip_while(|c| !c.is_ascii_digit())
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                info.iso = v.parse().ok().filter(|&v: &u32| v > 0 && v < 1_000_000);
            }
        }
        pos = start.saturating_add(len).div_ceil(16) * 16;
        sections += 1;
    }
    info.previews = pv.finish(1);
    Ok((
        info,
        Some(refused(
            "light-field sensor data (a picture behind every microlens) is not developed",
        )),
    ))
}

fn rd175(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    if d.len() != crate::rd175::FILE_SIZE {
        return Err(RawError::NotRaw);
    }
    let info = RawInfo {
        make: "Minolta".into(),
        model: "RD-175".into(),
        ..Default::default()
    };
    let (w, h) = (crate::rd175::WIDTH as u32, crate::rd175::HEIGHT as u32);
    let mut s = Sensor::new(w, h, 9, SensorCompression::Uncompressed);
    s.cfa = Cfa::bayer("GRBG");
    s.white = Some(510.0);
    s.storage = Storage::Vendor {
        range: crate::rd175::HEADER..d.len(),
        codec: VendorCodec::Rd175,
    };
    Ok((info, Some(s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_values() {
        let m = "{ \"camera\": { \"make\": \"Lytro, Inc.\", \"model\" : \"ILLUM\" } }";
        assert_eq!(json_str(m, "model").as_deref(), Some("ILLUM"));
        assert_eq!(json_str(m, "serial"), None);
    }

    #[test]
    fn signatures() {
        let mut q = b"qktn".to_vec();
        q.resize(0x400, 0);
        assert_eq!(detect(&q), Some(RawFormat::QuickTake));
        assert_eq!(detect(&q[..0x100]), None);
        let mut j = b"\x80\x3eDSCIM".to_vec();
        j.resize(512, 0);
        j.extend_from_slice(&[0xff, 0xd8, 0xff, 0xe0]);
        assert_eq!(detect(&j), Some(RawFormat::J6i));
        assert_eq!(detect(LFP_MAGIC), Some(RawFormat::Lytro));
        let mdc = vec![0u8; crate::rd175::FILE_SIZE];
        assert_eq!(detect_named(&mdc, "MDC"), Some(RawFormat::Rd175));
        assert_eq!(detect_named(&mdc[1..], "mdc"), None);
        assert_eq!(detect_named(&mdc, "raw"), None);
    }

    #[test]
    fn headerless_needs_name_size_and_fitting_samples() {
        let mut yi = vec![0u8; 31_850_496];
        for (i, b) in yi.iter_mut().enumerate() {
            *b = if i % 2 == 0 { (i % 251) as u8 } else { 0x0c };
        }
        assert_eq!(detect_named(&yi, "RAW"), Some(RawFormat::Headerless));
        assert_eq!(detect_named(&yi, "vol"), None);
        let (info, s) = parse_ext(&yi, RawFormat::Headerless, "raw").unwrap();
        assert_eq!(info.make, "Ambarella");
        let s = s.unwrap();
        assert_eq!((s.width, s.height, s.bits), (4608, 3456, 14));
        assert!(info.sensor.unwrap().decodable);
        // A 16-bit volume of the same size uses the high bits: not ours.
        yi[2 * 4608 * 97 + 27] = 0xff;
        assert_eq!(detect_named(&yi, "raw"), None);

        let mut optio = vec![0x55u8; 6_114_240];
        for row in optio.chunks_exact_mut(3520) {
            row[3486..].fill(0);
        }
        assert_eq!(detect_named(&optio, "raw"), Some(RawFormat::Headerless));
        optio[3519] = 1;
        assert_eq!(detect_named(&optio, "raw"), None);
    }

    #[test]
    fn quicktake_100_thumbnail() {
        let mut q = b"qktk\0\0\0\x04".to_vec();
        q.resize(0x2000, 0);
        q[0x12..0x16].copy_from_slice(&[0x01, 0x40, 0x00, 0xf0]);
        let n = q.len();
        q[n - 2400] = 0xf0;
        let (info, s) = parse(&q, RawFormat::QuickTake).unwrap();
        assert_eq!(info.model, "QuickTake 100");
        assert!(!info.sensor.unwrap().decodable);
        assert!(s.unwrap().unsupported.unwrap().contains("QuickTake 100"));
        let p = &info.previews[0];
        assert_eq!((p.kind, p.width, p.height), (PreviewKind::Grey4, 80, 60));
        let img = crate::decode_preview(&q, p, 0).unwrap();
        assert_eq!(&img.rgba[..8], &[255, 255, 255, 255, 0, 0, 0, 255]);
    }

    #[test]
    fn hostile_quicktake_never_panics() {
        for n in [0x301usize, 0x400, 0x2000] {
            let mut q = b"qktn".to_vec();
            q.resize(n, 0xff);
            let _ = parse(&q, RawFormat::QuickTake);
            q[..4].copy_from_slice(b"qktk");
            let _ = parse(&q, RawFormat::QuickTake);
        }
        let mut l = LFP_MAGIC.to_vec();
        l.extend_from_slice(&[0; 8]);
        l.extend_from_slice(b"\x89LFC\r\n\x1a\n\0\0\0\0\xff\xff\xff\xff");
        l.resize(200, 0);
        assert!(parse(&l, RawFormat::Lytro).is_ok());
    }
}
