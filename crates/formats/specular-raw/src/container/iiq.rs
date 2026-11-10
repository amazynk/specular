//! Phase One IIQ: a TIFF (previews, Exif) whose header is followed by a Phase One block
//! (`IIII` + `?Raw`, or `MMMM` + `Raw?`) of 16-byte entries: tag, type, length, value or offset
//! from the block start.

use super::{Black, Previews, Sensor, Storage, fill_from_tiff_block, mrw::tiff_previews};
use crate::cfa::Cfa;
use crate::error::{RawError, Result};
use crate::phase_one::Kind;
use crate::vendor::VendorCodec;
use crate::{RawFormat, RawInfo, Rect, SensorCompression};

/// Offset of the Phase One block (in the first 32 bytes) and its byte order.
pub(super) fn find(d: &[u8]) -> Option<(usize, bool)> {
    let head = &d[..d.len().min(32)];
    (0..head.len().saturating_sub(7)).find_map(|i| match &head[i..i + 8] {
        [b'I', b'I', b'I', b'I', _, b'w', b'a', b'R'] => Some((i, true)),
        [b'M', b'M', b'M', b'M', b'R', b'a', b'w', _] => Some((i, false)),
        _ => None,
    })
}

struct Rd<'a> {
    d: &'a [u8],
    le: bool,
}

impl Rd<'_> {
    fn u32(&self, p: usize) -> Option<u32> {
        let b: [u8; 4] = self.d.get(p..p.checked_add(4)?)?.try_into().ok()?;
        Some(if self.le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }
    fn f32(&self, p: usize) -> Option<f32> {
        self.u32(p).map(f32::from_bits).filter(|v| v.is_finite())
    }
}

#[derive(Default)]
struct Tags {
    flip: u32,
    raw_w: u32,
    raw_h: u32,
    left: u32,
    top: u32,
    w: u32,
    h: u32,
    format: Option<u32>,
    data: Option<(usize, usize)>,
    rows: Option<usize>,
    key: Option<(u16, u16)>,
    black: u32,
    wb: Option<[f32; 3]>,
    /// Calibration block (0x110), split column / row and their black offset tables
    /// (0x222..0x225).
    cal_block: Option<(usize, usize)>,
    split_col: u32,
    split_row: u32,
    row_black: Option<(usize, usize)>,
    col_black: Option<(usize, usize)>,
    /// ROMM primaries to camera matrix (0x106).
    romm_cam: Option<[f32; 9]>,
    model: Option<String>,
    lens: Option<String>,
    focal: Option<f64>,
    aperture: Option<f64>,
}

fn text(d: &[u8], p: usize, len: usize) -> Option<String> {
    let b = d.get(p..p.checked_add(len.min(256))?)?;
    let b = b.split(|&c| c == 0).next()?;
    let s = String::from_utf8_lossy(b).trim().to_string();
    (!s.is_empty() && b.first() != Some(&0xff)).then_some(s)
}

fn tags(r: &Rd, base: usize) -> Option<Tags> {
    let dir = base.checked_add(r.u32(base + 8)? as usize)?;
    let n = r.u32(dir)?;
    if n > 8192 {
        return None;
    }
    let mut t = Tags::default();
    for k in 0..n as usize {
        let p = dir + 8 + 16 * k;
        let (Some(tag), Some(typ), Some(len), Some(data)) =
            (r.u32(p), r.u32(p + 4), r.u32(p + 8), r.u32(p + 12))
        else {
            break;
        };
        let at = base.saturating_add(data as usize);
        // Numbers stored as IEEE floats in the value field (type 4) or at the offset.
        let real = || {
            if typ == 4 && len == 4 {
                Some(f32::from_bits(data)).filter(|v| v.is_finite())
            } else {
                r.f32(at)
            }
        };
        match tag {
            0x100 => t.flip = data & 3,
            0x107 if len >= 12 => {
                t.wb = (|| Some([r.f32(at)?, r.f32(at + 4)?, r.f32(at + 8)?]))();
            }
            0x108 => t.raw_w = data,
            0x109 => t.raw_h = data,
            0x10a => t.left = data,
            0x10b => t.top = data,
            0x10c => t.w = data,
            0x10d => t.h = data,
            0x10e => t.format = Some(data),
            0x10f => t.data = Some((at, len as usize)),
            0x110 => t.cal_block = Some((at, len as usize)),
            0x106 if len >= 36 => {
                t.romm_cam = (0..9)
                    .map(|i| r.f32(at + 4 * i))
                    .collect::<Option<Vec<f32>>>()
                    .and_then(|v| v.try_into().ok());
            }
            0x222 => t.split_col = data,
            0x223 => t.row_black = Some((at, len as usize)),
            0x224 => t.split_row = data,
            0x225 => t.col_black = Some((at, len as usize)),
            0x112 => {
                let b = data.to_le_bytes();
                let (a, bb) = if r.le {
                    (
                        u16::from_le_bytes([b[0], b[1]]),
                        u16::from_le_bytes([b[2], b[3]]),
                    )
                } else {
                    let b = data.to_be_bytes();
                    (
                        u16::from_be_bytes([b[0], b[1]]),
                        u16::from_be_bytes([b[2], b[3]]),
                    )
                };
                t.key = Some((a, bb));
            }
            0x21c => t.rows = Some(at),
            0x21d => t.black = data,
            0x301 => {
                if let Some(s) = text(r.d, at, len as usize) {
                    // "IQ3 100MP, User Firmware: …", "P25+-M, Firmware: …", "… camera".
                    let mut m = s.split(',').next().unwrap_or("").to_string();
                    if let Some(i) = m.find(" camera") {
                        m.truncate(i);
                    }
                    if let Some(i) = m.find('-') {
                        m.truncate(i);
                    }
                    t.model = Some(m.trim().to_string()).filter(|m| !m.is_empty());
                }
            }
            0x401 => {
                t.aperture = real()
                    .map(|v| 2f64.powf(v as f64 / 2.0))
                    .filter(|v| (0.5..1000.0).contains(v));
            }
            0x403 => t.focal = real().map(f64::from).filter(|v| *v > 0.0 && *v < 10000.0),
            0x412 => t.lens = text(r.d, at, len as usize),
            _ => {}
        }
    }
    Some(t)
}

/// "P25+" → "P 25+" (the naming of the colour tables); bare backs by sensor height.
fn model_name(t: &Tags) -> String {
    match &t.model {
        Some(m) => {
            let b = m.as_bytes();
            if b.len() > 1 && b[0] == b'P' && b[1].is_ascii_digit() {
                format!("P {}", &m[1..])
            } else {
                m.clone()
            }
        }
        None => match t.raw_h {
            2060 => "LightPhase",
            2682 => "H 10",
            4128 => "H 20",
            5488 => "H 25",
            _ => "",
        }
        .to_string(),
    }
}

pub(super) fn parse(d: &[u8], base: usize, le: bool) -> Result<(RawInfo, Option<Sensor>)> {
    let r = Rd { d, le };
    let t = tags(&r, base).ok_or(RawError::Malformed("Phase One header"))?;
    let mut info = RawInfo {
        format: Some(RawFormat::Iiq),
        make: "Phase One".into(),
        model: model_name(&t),
        ..Default::default()
    };
    info.orientation = [1, 6, 8, 3][t.flip as usize];
    let mut pv = Previews::default();
    if base > 0 {
        fill_from_tiff_block(&mut info, d, 0..d.len());
        tiff_previews(&mut pv, d, 0..d.len());
        info.tiff_blocks.push(0..d.len());
    }
    info.make = "Phase One".into();
    info.lens = info.lens.take().or(t.lens.clone());
    info.focal_length = info.focal_length.or(t.focal);
    info.f_number = info.f_number.or(t.aperture);
    info.previews = pv.finish(info.orientation);
    let s = sensor(&t);
    info.sensor = Some(s.info());
    Ok((info, Some(s)))
}

fn sensor(t: &Tags) -> Sensor {
    let format = t.format.unwrap_or(0);
    let (w, h) = (t.raw_w, t.raw_h);
    let mut s = Sensor::new(w, h, 16, SensorCompression::Other(format));
    if w < 8 || h < 2 || w > 65535 || h > 65535 {
        return s.unsupported("Phase One sensor size");
    }
    // The backs read RGGB from the top-left of the visible area; the sensor origin may sit an
    // odd number of rows or columns before it (H 25 and P 25+ start the image at row 21).
    s.cfa = Cfa::bayer(match (t.top & 1, t.left & 1) {
        (0, 0) => "RGGB",
        (0, _) => "GRBG",
        (_, 0) => "GBRG",
        _ => "BGGR",
    });
    if t.w > 0 && t.h > 0 && t.left.saturating_add(t.w) <= w && t.top.saturating_add(t.h) <= h {
        s.crop = Rect {
            x: t.left,
            y: t.top,
            w: t.w,
            h: t.h,
        };
    }
    let nominal = if format == 8 { 65535.0 } else { 65532.0 };
    s.black = Black::Levels([t.black as f32; 4]);
    s.white = Some(nominal);
    // The back's own camera -> ROMM matrix; a table entry for the model takes precedence.
    if let Some(m) = t.romm_cam.as_ref().and_then(crate::color::romm_rgb_cam) {
        s.color = super::ColorSource::Matrix(m, crate::MatrixSource::CameraCalibration);
    }
    if let Some([r, g, b]) = t.wb
        && r > 0.0
        && g > 0.0
        && b > 0.0
    {
        s.wb = Some([r / g, 1.0, b / g]);
    }
    let Some((at, len)) = t.data else {
        return s.unsupported("Phase One sensor data");
    };
    let range = at..at.saturating_add(len);
    // Every format carries the sensor calibration; the decode subtracts the black (with its
    // row and column tables) and applies it.
    let cal_range = |v: Option<(usize, usize)>| v.map(|(a, n)| a..a.saturating_add(n));
    let cal = crate::phase_one_cal::Calibration {
        black: t.black.min(65535) as i32,
        split_col: t.split_col,
        split_row: t.split_row,
        row_offsets: cal_range(t.row_black),
        col_offsets: cal_range(t.col_black),
        block: cal_range(t.cal_block),
        colors: s
            .cfa
            .as_ref()
            .and_then(Cfa::bayer2x2)
            .unwrap_or([0, 1, 1, 2]),
    };
    let calibrated = matches!(format, 0..=3 | 5 | 6 | 8);
    if calibrated {
        s.black = Black::Levels([0.0; 4]);
        s.white = Some(nominal - t.black.min(60000) as f32);
    }
    let codec = match format {
        0..=2 => VendorCodec::PhaseOnePlain {
            key: match (format, t.key) {
                (0, _) | (_, None) => None,
                (1, Some((a, b))) => Some((a, b, 0x5555)),
                (_, Some((a, b))) => Some((a, b, 0x1354)),
            },
            cal: Box::new(cal),
        },
        3 | 5 | 8 | 6 => {
            let Some(rows) = t.rows else {
                return s.unsupported("Phase One row offsets");
            };
            VendorCodec::PhaseOne {
                kind: if format == 6 {
                    Kind::S
                } else {
                    Kind::L { format }
                },
                rows: rows..rows.saturating_add(4 * h as usize),
                cal: Box::new(cal),
            }
        }
        _ => return s.unsupported("Phase One IIQ variant"),
    };
    s.storage = Storage::Vendor { range, codec };
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(format: u32) -> Vec<u8> {
        let (w, h) = (8u32, 2u32);
        // Two IIQ S rows of raw samples (step code 0 twice, block code "1", 8 × 14 bits).
        let mut bits: Vec<bool> = Vec::new();
        let mut put = |v: u32, n: u32| {
            for k in (0..n).rev() {
                bits.push(v >> k & 1 == 1);
            }
        };
        put(0, 16);
        put(0, 7);
        put(0, 7);
        put(1, 1);
        for i in 0..8 {
            put(100 * i + 5, 14);
        }
        let mut row = Vec::new();
        for word in bits.chunks(32) {
            let mut v = 0u32;
            for (i, &b) in word.iter().enumerate() {
                v |= (b as u32) << (31 - i);
            }
            row.extend(v.to_le_bytes());
        }
        let mut f = b"II*\0\0\0\0\0".to_vec();
        let base = f.len();
        f.extend(b"IIIICwaR");
        f.extend(0u32.to_le_bytes()); // directory offset, patched below
        let data_at = f.len() - base;
        f.extend(&row);
        f.extend(&row);
        let rows_at = f.len() - base;
        f.extend(0u32.to_le_bytes());
        f.extend((row.len() as u32).to_le_bytes());
        let dir = f.len() - base;
        f[base + 8..base + 12].copy_from_slice(&(dir as u32).to_le_bytes());
        let entries: Vec<(u32, u32, u32)> = vec![
            (0x100, 4, 1),
            (0x108, 4, w),
            (0x109, 4, h),
            (0x10e, 4, format),
            (0x10f, 2 * row.len() as u32, data_at as u32),
            (0x21c, 8, rows_at as u32),
            (0x21d, 4, 256),
        ];
        f.extend((entries.len() as u32).to_le_bytes());
        f.extend(0u32.to_le_bytes());
        for (tag, len, data) in entries {
            for v in [tag, 4, len, data] {
                f.extend(v.to_le_bytes());
            }
        }
        f
    }

    #[test]
    fn iiq_s() {
        let f = file(6);
        assert_eq!(find(&f), Some((8, true)));
        let raw = crate::parse(&f).unwrap();
        let info = raw.info();
        assert_eq!(info.format, Some(RawFormat::Iiq));
        assert_eq!(info.orientation, 6);
        let img = raw.decode_raw(&f, &Default::default()).unwrap();
        assert_eq!((img.width, img.height), (8, 2));
        // The black (256) is subtracted by the decode.
        let want: Vec<u16> = (0..8)
            .map(|i| (((100 * i + 5) << 2) as u16).saturating_sub(256))
            .collect();
        assert_eq!(&img.data[..8], &want[..]);
        assert_eq!(&img.data[8..], &want[..]);
        assert_eq!(img.black, [0.0; 4]);
        assert_eq!(img.white, 65532.0 - 256.0);
        assert!(!img.truncated);
        assert!(
            crate::parse(&file(4))
                .unwrap()
                .decode_raw(&f, &Default::default())
                .is_err()
        );
    }

    #[test]
    fn truncated_and_hostile() {
        let f = file(6);
        for n in [9, 16, 20, 40, f.len() / 2, f.len() - 1] {
            let _ = crate::parse(&f[..n]).map(|r| r.decode_raw(&f[..n], &Default::default()));
        }
        let mut bad = f.clone();
        // Huge row count.
        let at = bad.len() - 16 * 5 + 12;
        bad[at..at + 4].copy_from_slice(&0xffffu32.to_le_bytes());
        let _ = crate::parse(&bad).map(|r| r.decode_raw(&bad, &Default::default()));
    }
}
