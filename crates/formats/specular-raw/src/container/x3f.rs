//! Sigma X3F (Foveon): `FOVb`, a version, a header (size, rotation, white balance name) and a
//! directory (`SECd`, its offset in the last four bytes) of sections: `SECp` properties
//! (UTF-16 name/value pairs), `SECi` images (raw, JPEG / RGB / Huffman thumbnails) and `SECc`
//! CAMF, the camera's calibration (obfuscated or compressed; `CMbT` text, `CMbP` property
//! lists and `CMbM` matrices). Quattro files (version 4) have no property section.
//!
//! Colour comes from CAMF: per white-balance preset, layer gains and a colour-correction
//! matrix to sRGB (DP and Merrill and Quattro bodies), or a camera-to-XYZ matrix with a
//! correction (SD9 to SD14). The visible area, the dark shield rows (black level) and the
//! sample depth are in CAMF too.

use super::{Black, ColorSource, Previews, Sensor, Storage};
use crate::error::{RawError, Result};
use crate::vendor::VendorCodec;
use crate::x3f::{AfGrid, TrueTable};
use crate::{MatrixSource, RawFormat, RawInfo, Rect, SensorCompression};

pub(super) fn is_x3f(d: &[u8]) -> bool {
    d.starts_with(b"FOVb")
}

fn u16le(b: &[u8], p: usize) -> u16 {
    b.get(p..p.wrapping_add(2))
        .map_or(0, |s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32le(b: &[u8], p: usize) -> u32 {
    b.get(p..p.wrapping_add(4))
        .map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Image section type and format (`type << 16 | format`).
const THUMB_PLAIN: u32 = 0x20003;
const THUMB_CODED: u32 = 0x2000b;
const THUMB_JPEG: u32 = 0x20012;
const RAW_X530: u32 = 0x30005;
const RAW_HUFFMAN_10BIT: u32 = 0x30006;
const RAW_TRUE: u32 = 0x3001e;
const RAW_MERRILL: u32 = 0x1001e;
const RAW_QUATTRO: [u32; 4] = [0x10023, 0x10025, 0x10027, 0x10029];

/// Most sections a directory may list.
const MAX_SECTIONS: usize = 64;
/// Largest decoded CAMF accepted.
const MAX_CAMF: usize = 64 << 20;

struct Image {
    kind: u32,
    cols: u32,
    rows: u32,
    stride: u32,
    /// Data after the 28-byte image header, to the section end.
    data: std::ops::Range<usize>,
}

pub(super) fn parse(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    if !is_x3f(d) || d.len() < 64 {
        return Err(RawError::NotRaw);
    }
    let version = u32le(d, 4);
    let (rotation, header_wb) = if version < 0x40000 {
        let wb = if version >= 0x20001 {
            cstr(d.get(40..72).unwrap_or(&[]))
        } else {
            String::new()
        };
        (u32le(d, 36), wb)
    } else {
        (u32le(d, 48), String::new())
    };
    let dir = u32le(d, d.len() - 4) as usize;
    if d.get(dir..dir + 4) != Some(b"SECd") {
        return Err(RawError::Malformed("X3F directory"));
    }
    let count = (u32le(d, dir + 8) as usize).min(MAX_SECTIONS);
    let mut info = RawInfo {
        format: Some(RawFormat::X3f),
        make: "Sigma".into(),
        orientation: match rotation {
            90 => 6,
            180 => 3,
            270 => 8,
            _ => 1,
        },
        ..Default::default()
    };
    let mut pv = Previews::default();
    let mut raw: Option<Image> = None;
    let mut camf = None;
    let mut props = Vec::new();
    for k in 0..count {
        let e = dir + 12 + 12 * k;
        let (off, size) = (u32le(d, e) as usize, u32le(d, e + 4) as usize);
        let end = off.saturating_add(size).min(d.len());
        match d.get(off..off + 4) {
            Some(b"SECi") => {
                let img = Image {
                    kind: u32le(d, off + 8) << 16 | u32le(d, off + 12) & 0xffff,
                    cols: u32le(d, off + 16),
                    rows: u32le(d, off + 20),
                    stride: u32le(d, off + 24),
                    data: (off + 28).min(end)..end,
                };
                match img.kind {
                    THUMB_JPEG => {
                        pv.jpeg(d, img.data.clone(), "X3F JPEG preview");
                    }
                    THUMB_PLAIN => {
                        // SD9/SD10 rows are padded to whole 32-bit words.
                        let stride = (img.stride as usize).max(img.cols as usize * 3);
                        pv.rgb_rows(
                            d,
                            img.data.start,
                            img.cols,
                            img.rows,
                            stride,
                            "X3F thumbnail",
                        );
                    }
                    THUMB_CODED => {
                        pv.x3f_coded(img.data.clone(), img.cols, img.rows, "X3F thumbnail");
                    }
                    k if is_raw(k) && raw.as_ref().is_none_or(|r| pixels(&img) > pixels(r)) => {
                        raw = Some(img);
                    }
                    _ => {}
                }
            }
            Some(b"SECp") => props = properties(d, off, end),
            Some(b"SECc") => camf = Some(off..end),
            _ => {}
        }
    }
    let prop = |k: &str| {
        props
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    if let Some(m) = prop("CAMMODEL") {
        info.model = m
            .strip_prefix("SIGMA ")
            .or_else(|| m.strip_prefix("Sigma "))
            .unwrap_or(&m)
            .to_string();
    } else {
        info.model = quattro_model(d, raw.as_ref().map(|r| (r.cols, r.rows)));
    }
    info.iso = prop("ISO")
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| v as u32);
    info.exposure_time = prop("SHUTTER")
        .and_then(|v| v.parse::<f64>().ok())
        .or_else(|| {
            prop("EXPTIME")
                .and_then(|v| v.parse::<f64>().ok())
                .map(|v| v / 1e6)
        })
        .filter(|v| *v > 0.0);
    info.f_number = prop("APERTURE")
        .and_then(|v| v.parse().ok())
        .filter(|v: &f64| *v > 0.0);
    info.focal_length = prop("FLENGTH")
        .and_then(|v| v.parse().ok())
        .filter(|v: &f64| *v > 0.0);
    info.exposure_bias = prop("EXPCOMP").and_then(|v| v.parse().ok());
    info.date_time = prop("TIME")
        .and_then(|v| v.parse::<i64>().ok())
        .and_then(date_time);
    info.software = prop("FIRMVERS");
    info.previews = pv.finish(info.orientation);

    let camf = camf.and_then(|r| Camf::decode(d, r));
    // The shot's preset: the property list (the header may hold a converter's later choice),
    // the header, or the Quattro CAMF code.
    let wb_name = prop("WB_DESC")
        .or((!header_wb.is_empty()).then_some(header_wb))
        .or_else(|| {
            camf.as_ref()
                .and_then(|c| c.floats("WhiteBalance"))
                .and_then(|v| v.first().copied())
                .map(|v| quattro_wb_name(v as u32).to_string())
        })
        .unwrap_or_else(|| "Auto".into());
    let lens = (info.f_number, info.focal_length);
    let sensor = raw.map(|img| sensor(&img, camf.as_ref(), &wb_name, lens));
    info.sensor = sensor.as_ref().map(Sensor::info);
    Ok((info, sensor))
}

fn is_raw(kind: u32) -> bool {
    matches!(kind, RAW_X530 | RAW_HUFFMAN_10BIT | RAW_TRUE | RAW_MERRILL)
        || RAW_QUATTRO.contains(&kind)
}

fn pixels(i: &Image) -> u64 {
    i.cols as u64 * i.rows as u64
}

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim().to_string()
}

/// Quattro files carry no property list: the dp models name themselves in the first 2 KiB;
/// the sd bodies only in the preview's EXIF, so they are told apart by the raw image size.
fn quattro_model(d: &[u8], size: Option<(u32, u32)>) -> String {
    let head = &d[..d.len().min(2048)];
    let find = |pat: &[u8]| head.windows(pat.len()).position(|w| w == pat);
    if let Some(p) = find(b"SIGMA dp") {
        let n = head
            .get(p + 8)
            .copied()
            .filter(u8::is_ascii_digit)
            .unwrap_or(b'2');
        return format!("dp{} Quattro", n as char);
    }
    if let Some(p) = find(b"sd Quattro") {
        return if head.get(p + 10..p + 12) == Some(b" H") {
            "sd Quattro H".into()
        } else {
            "sd Quattro".into()
        };
    }
    match size {
        Some((6656, 4480)) => "sd Quattro H".into(),
        Some((5888, 3776)) => "sd Quattro".into(),
        _ => "Quattro".into(),
    }
}

/// CAMF `WhiteBalance` code (Quattro) to preset name.
fn quattro_wb_name(v: u32) -> &'static str {
    match v {
        2 => "Sunlight",
        3 => "Shade",
        4 => "Overcast",
        5 => "Incandescent",
        6 => "Fluorescent",
        7 => "Flash",
        8 => "Custom",
        12 => "AutoLSP",
        _ => "Auto",
    }
}

/// `SECp`: count, character format, reserved, total length; then (name, value) offsets in
/// UTF-16 units into the character data that follows.
fn properties(d: &[u8], off: usize, end: usize) -> Vec<(String, String)> {
    let n = (u32le(d, off + 8) as usize).min(4096);
    let table = off + 24;
    let chars = table + 8 * n;
    let Some(text) = d.get(chars.min(end)..end) else {
        return Vec::new();
    };
    let s16 = |o: usize| -> String {
        let units: Vec<u16> = (o..)
            .map(|i| u16le(text, 2 * i))
            .take_while(|&u| u != 0)
            .take(512)
            .collect();
        String::from_utf16_lossy(&units)
    };
    (0..n)
        .filter(|i| table + 8 * i + 8 <= end)
        .map(|i| {
            let p = table + 8 * i;
            (s16(u32le(d, p) as usize), s16(u32le(d, p + 4) as usize))
        })
        .collect()
}

/// `YYYY:MM:DD HH:MM:SS` of seconds since 1970 (the camera clock, no zone).
fn date_time(t: i64) -> Option<String> {
    if !(0..=253_402_300_799).contains(&t) {
        return None;
    }
    let (days, secs) = (t.div_euclid(86400), t.rem_euclid(86400));
    // Civil date from days since 1970-01-01 (proleptic Gregorian).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Some(format!(
        "{year:04}:{month:02}:{day:02} {:02}:{:02}:{:02}",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    ))
}

// ---------------------------------------------------------------------------------------------
// CAMF

struct Camf {
    data: Vec<u8>,
    /// Name, tag and start of each entry.
    entries: Vec<(String, [u8; 4], usize)>,
}

impl Camf {
    /// Decodes the section at `r` (type 2: XOR stream; 4: 2-D predicted 12-bit values; 5:
    /// 1-D predicted bytes) and indexes its entries.
    fn decode(d: &[u8], r: std::ops::Range<usize>) -> Option<Camf> {
        let sec = d.get(r)?;
        let kind = u32le(sec, 8);
        let h: [u32; 4] = std::array::from_fn(|i| u32le(sec, 12 + 4 * i));
        let src = sec.get(28..)?;
        let data = match kind {
            2 => {
                let mut key = h[3];
                src.iter()
                    .map(|&b| {
                        key = key.wrapping_mul(1597).wrapping_add(51749) % 244_944;
                        let t = ((key as u64 * 301_593_171) >> 24) as u32;
                        let k = ((((key << 8).wrapping_sub(t) >> 1).wrapping_add(t)) >> 17) as u8;
                        b ^ k
                    })
                    .collect()
            }
            4 | 5 => {
                let size = h[0] as usize;
                if size > MAX_CAMF || size > src.len().saturating_mul(64) {
                    return None;
                }
                let (table, _) = TrueTable::read(src);
                let bits = src.get(32..)?;
                if kind == 5 {
                    camf_type5(bits, &table, h[1] as i32, size)
                } else {
                    camf_type4(
                        bits,
                        &table,
                        h[1] as i32,
                        h[2] as usize,
                        h[3] as usize,
                        size,
                    )
                }
            }
            _ => return None,
        };
        let mut entries = Vec::new();
        let mut p = 0usize;
        while let Some(tag) = data.get(p..p + 4) {
            let tag: [u8; 4] = [tag[0], tag[1], tag[2], tag[3]];
            if !matches!(&tag, b"CMbP" | b"CMbT" | b"CMbM") {
                break;
            }
            let size = u32le(&data, p + 8) as usize;
            let no = u32le(&data, p + 12) as usize;
            if size < 20 || p + size > data.len() || entries.len() > 100_000 {
                break;
            }
            let name = data
                .get(p + no..p + size)
                .map(|b| cstr(&b[..b.len().min(256)]))
                .unwrap_or_default();
            entries.push((name, tag, p));
            p += size;
        }
        Some(Camf { data, entries })
    }

    fn entry(&self, name: &str, tag: &[u8; 4]) -> Option<usize> {
        self.entries
            .iter()
            .find(|(n, t, _)| n == name && t == tag)
            .map(|e| e.2)
    }

    /// Entry whose name ends with `suffix` (some bodies prefix the model, `DP1_`).
    fn entry_suffix(&self, suffix: &str, tag: &[u8; 4]) -> Option<usize> {
        self.entries
            .iter()
            .find(|(n, t, _)| n.ends_with(suffix) && t == tag)
            .map(|e| e.2)
    }

    /// Value of `key` in the property list entry at `p`.
    fn property(&self, p: usize, key: &str) -> Option<String> {
        let c = &self.data;
        let v = p + u32le(c, p + 16) as usize;
        let (n, off) = (u32le(c, v) as usize, u32le(c, v + 4) as usize);
        let base = p + off;
        (0..n.min(1024)).find_map(|i| {
            let k = c.get(base + u32le(c, v + 8 + 8 * i) as usize..)?;
            let k = cstr(&k[..k.len().min(256)]);
            (k == key).then(|| {
                let o = base + u32le(c, v + 12 + 8 * i) as usize;
                c.get(o..).map(|b| cstr(&b[..b.len().min(256)]))
            })?
        })
    }

    /// The elements of matrix entry `name` as f64 (all integer and float element types).
    fn floats(&self, name: &str) -> Option<Vec<f64>> {
        self.matrix_at(self.entry(name, b"CMbM")?)
    }

    fn matrix_at(&self, p: usize) -> Option<Vec<f64>> {
        let c = &self.data;
        let v = p + u32le(c, p + 16) as usize;
        let (kind, dim, off) = (
            u32le(c, v),
            u32le(c, v + 4) as usize,
            u32le(c, v + 8) as usize,
        );
        if dim == 0 || dim > 8 {
            return None;
        }
        let mut n = 1usize;
        for i in 0..dim {
            n = n.checked_mul(u32le(c, v + 12 + 12 * i) as usize)?;
        }
        let size = match kind {
            0 | 6 => 2,
            1..=3 => 4,
            5 => 1,
            _ => return None,
        };
        let start = p + off;
        let b = c.get(start..start.checked_add(n.checked_mul(size)?)?)?;
        Some(
            b.chunks_exact(size)
                .map(|e| match kind {
                    0 => i16::from_le_bytes([e[0], e[1]]) as f64,
                    6 => u16::from_le_bytes([e[0], e[1]]) as f64,
                    1 => u32::from_le_bytes([e[0], e[1], e[2], e[3]]) as i32 as f64,
                    2 => u32::from_le_bytes([e[0], e[1], e[2], e[3]]) as f64,
                    3 => f32::from_le_bytes([e[0], e[1], e[2], e[3]]) as f64,
                    _ => e[0] as f64,
                })
                .collect(),
        )
    }

    /// The matrix the property list `list` (name suffix) names for preset `wb`.
    fn preset(&self, list: &str, wb: &str) -> Option<Vec<f64>> {
        let l = self.entry_suffix(list, b"CMbP")?;
        let name = self.property(l, wb).or_else(|| self.property(l, "Auto"))?;
        self.floats(&name)
    }

    /// An area entry `[x0, y0, x1, y1]` (inclusive) as a rectangle.
    fn area(&self, name: &str) -> Option<Rect> {
        let v = self.floats(name)?;
        let [x0, y0, x1, y1] = <[f64; 4]>::try_from(v.get(..4)?).ok()?;
        (x1 >= x0 && y1 >= y0 && x0 >= 0.0 && y0 >= 0.0).then(|| Rect {
            x: x0 as u32,
            y: y0 as u32,
            w: (x1 - x0) as u32 + 1,
            h: (y1 - y0) as u32 + 1,
        })
    }
}

/// CAMF type 5: a byte stream of predicted differences.
fn camf_type5(bits: &[u8], table: &TrueTable, bias: i32, size: usize) -> Vec<u8> {
    crate::x3f::camf_bytes(bits, table, bias, size)
}

/// CAMF type 4: a `cols x rows` TRUE-predicted plane of 12-bit values packed into bytes
/// (only the low 12 bits of each value count, so a 16-bit seed and plane suffice).
fn camf_type4(
    bits: &[u8],
    table: &TrueTable,
    bias: i32,
    cols: usize,
    rows: usize,
    size: usize,
) -> Vec<u8> {
    let n = cols
        .saturating_mul(rows)
        .min(size.saturating_mul(2) / 3 + 2);
    let mut vals = vec![0u16; n];
    if let Some(rows) = n.checked_div(cols) {
        crate::x3f::true_plane(bits, table, bias as u16, cols, rows, cols, &mut vals);
    }
    let mut out = Vec::with_capacity(size);
    for pair in vals.chunks(2) {
        let a = pair[0] as u32;
        out.push((a >> 4) as u8);
        match pair.get(1) {
            Some(&b) => {
                let b = b as u32;
                out.push(((a << 4) & 0xf0 | (b >> 8) & 0x0f) as u8);
                out.push(b as u8);
            }
            None => out.push(((a << 4) & 0xf0) as u8),
        }
    }
    out.resize(size, 0);
    out
}

// ---------------------------------------------------------------------------------------------
// Sensor

const XYZ_TO_SRGB: [[f64; 3]; 3] = [
    [3.2404542, -1.5371385, -0.4985314],
    [-0.9692660, 1.8760108, 0.0415560],
    [0.0556434, -0.2040259, 1.0572252],
];
const D65: [f64; 3] = [0.95047, 1.0, 1.08883];

type M3 = [[f64; 3]; 3];

fn m3(v: &[f64]) -> Option<M3> {
    (v.len() >= 9).then(|| std::array::from_fn(|i| std::array::from_fn(|j| v[i * 3 + j])))
}

fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn mulv(a: &M3, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| (0..3).map(|k| a[i][k] * v[k]).sum())
}

fn inv(m: &M3) -> Option<M3> {
    let c = |i: usize, j: usize| {
        let (r0, r1) = ((i + 1) % 3, (i + 2) % 3);
        let (c0, c1) = ((j + 1) % 3, (j + 2) % 3);
        m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
    };
    let det: f64 = (0..3).map(|j| m[0][j] * c(0, j)).sum();
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    Some(std::array::from_fn(|i| {
        std::array::from_fn(|j| c(j, i) / det)
    }))
}

/// A matrix from white-balanced camera RGB to linear sRGB, and the layer multipliers.
type Colour = ([[f32; 3]; 3], Option<[f32; 3]>);

/// White-balanced camera RGB -> linear sRGB and the layer multipliers for preset `wb`. The
/// SD9 to SD14 calibration has no gains for "Auto" (the camera's estimate is not stored): no
/// multipliers then, so the develop estimates them.
fn colour(c: &Camf, wb: &str) -> Option<Colour> {
    let mut known = true;
    let (m, mut gain) = if let (Some(g), Some(cc)) = (
        c.preset("WhiteBalanceGains", wb),
        c.preset("WhiteBalanceColorCorrections", wb),
    ) {
        let mut g: [f64; 3] = g.get(..3)?.try_into().ok()?;
        // `DigitalISOGain` (sd Quattro H): the layers are stored before a per-layer gain
        // (4, 4, 2), the bottom two at half the top's scale; the calibration expects it.
        for f in ["TempGainFact", "FNumberGainFact", "DigitalISOGain"] {
            if let Some(v) = c.floats(f).filter(|v| v.len() >= 3) {
                for k in 0..3 {
                    g[k] *= v[k];
                }
            }
        }
        (m3(&cc)?, g)
    } else {
        let cam_xyz = m3(&c.preset("WhiteBalanceIlluminants", wb)?)?;
        let corr = c
            .preset("WhiteBalanceCorrections", wb)
            .and_then(|v| m3(&v))
            .unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        // The correction acts on XYZ (applied to the camera matrix's output).
        let raw_xyz = mul(&corr, &cam_xyz);
        let neutral = mulv(&inv(&raw_xyz)?, D65);
        if neutral.iter().any(|&v| v <= 0.0) {
            return None;
        }
        let m = mul(&XYZ_TO_SRGB, &raw_xyz);
        let m = std::array::from_fn(|i| std::array::from_fn(|j| m[i][j] * neutral[j]));
        known = wb != "Auto";
        (m, neutral.map(|v| 1.0 / v))
    };
    if gain.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return None;
    }
    let g1 = gain[1];
    for v in &mut gain {
        *v /= g1;
    }
    // Keep white: rows sum to one.
    let mut out = [[0f32; 3]; 3];
    for (o, row) in out.iter_mut().zip(m) {
        let s: f64 = row.iter().sum();
        if !s.is_finite() || s.abs() < 1e-6 {
            return None;
        }
        *o = row.map(|v| (v / s) as f32);
    }
    Some((out, known.then(|| gain.map(|v| v as f32))))
}

/// The vignetting (lens shading) gains of the calibration: one table for the preset (SD9,
/// SD10, SD14, DP1/DP2: `SpatialGain`, rows x columns x 3 floats), or for Merrill bodies the
/// tables of the shot's focal length and the nearest aperture (`SpatialGainsProps_<f>_<mm>`:
/// per layer a table of integers scaled by `Delta` above `MinGains`). Quattro bodies' shading
/// model (sensor base gains with lens terms) is not applied.
fn spatial_gain(
    c: &Camf,
    wb: &str,
    lens: (Option<f64>, Option<f64>),
    area: Rect,
    dark: Option<Rect>,
) -> Option<crate::x3f::SpatialGain> {
    let make = |grids: [(usize, usize, Vec<f32>); 3]| {
        grids
            .iter()
            .all(|(r, c, g)| {
                *r >= 2
                    && *c >= 2
                    && g.len() == r * c
                    && g.iter().all(|v| v.is_finite() && *v > 0.25 && *v < 8.0)
            })
            .then_some(crate::x3f::SpatialGain { grids, area, dark })
    };
    let dims = |p: usize| -> Vec<usize> {
        let v = p + u32le(&c.data, p + 16) as usize;
        let n = (u32le(&c.data, v + 4) as usize).min(8);
        (0..n)
            .map(|i| u32le(&c.data, v + 12 + 12 * i) as usize)
            .collect()
    };
    // One table of all three layers.
    let table = c
        .entry_suffix("SpatialGainTables", b"CMbP")
        .and_then(|l| c.property(l, wb).or_else(|| c.property(l, "Auto")))
        .or_else(|| {
            c.entry("SpatialGain", b"CMbM")
                .map(|_| "SpatialGain".to_string())
        });
    if let Some(name) = table
        && let Some(p) = c.entry(&name, b"CMbM")
        && let [rows, cols, 3] = dims(p)[..]
        && let Some(v) = c.matrix_at(p)
    {
        let plane = |k: usize| {
            (
                rows,
                cols,
                v.iter().skip(k).step_by(3).map(|&x| x as f32).collect(),
            )
        };
        return make([plane(0), plane(1), plane(2)]);
    }
    // Merrill: tables per aperture and focal length.
    let mut best: Option<(f64, usize)> = None;
    for (i, (n, t, _)) in c.entries.iter().enumerate() {
        let Some(rest) = n
            .strip_prefix("SpatialGainsProps_")
            .filter(|_| t == b"CMbP")
        else {
            continue;
        };
        let mut parts = rest.split('_').map(|v| v.parse::<f64>().ok());
        let (Some(Some(ap)), Some(Some(mm))) = (parts.next(), parts.next()) else {
            continue;
        };
        let log =
            |a: f64, b: Option<f64>| b.filter(|b| *b > 0.0).map_or(0.0, |b| (a / b).ln().abs());
        // Only tables of the shot's focal length (the calibration is for particular lenses:
        // an SD1 Merrill file shot at 105 mm carries 30 and 50 mm tables); then the nearest
        // aperture.
        let Some(fl) = lens.1.filter(|f| *f > 0.0) else {
            continue;
        };
        if (mm / fl).ln().abs() > 0.1 {
            continue;
        }
        let d = log(ap, lens.0);
        if best.is_none_or(|b| d < b.0) {
            best = Some((d, i));
        }
    }
    let props = c.entries[best?.1].2;
    let layer = |k: &str| -> Option<(usize, usize, Vec<f32>)> {
        let min: f64 = c.property(props, &format!("MinGains{k}"))?.parse().ok()?;
        let delta: f64 = c.property(props, &format!("Delta{k}"))?.parse().ok()?;
        let p = c.entry(&c.property(props, &format!("GainsTable{k}"))?, b"CMbM")?;
        let [rows, cols] = dims(p)[..] else {
            return None;
        };
        let v = c.matrix_at(p)?;
        Some((
            rows,
            cols,
            v.iter().map(|&x| (min + delta * x) as f32).collect(),
        ))
    };
    make([layer("R")?, layer("G")?, layer("B")?])
}

fn sensor(img: &Image, camf: Option<&Camf>, wb: &str, lens: (Option<f64>, Option<f64>)) -> Sensor {
    let quattro = RAW_QUATTRO.contains(&img.kind);
    let depth = camf
        .and_then(|c| c.floats("ImageDepth"))
        .and_then(|v| v.first().copied())
        .filter(|v| (8.0..=16.0).contains(v))
        .map(|v| v as u32);
    let bits = depth.unwrap_or(match img.kind {
        RAW_HUFFMAN_10BIT | RAW_X530 => 14,
        RAW_TRUE => 14,
        _ => 12,
    });
    let mut s = Sensor::new(img.cols, img.rows, bits, SensorCompression::Other(img.kind));
    s.cpp = 3;
    // CAMF areas count from the sensor origin; the stored image starts at `KeepImageArea`.
    let keep = camf.and_then(|c| c.area("KeepImageArea"));
    let (kx, ky) = keep.map_or((0, 0), |k| (k.x, k.y));
    let area = |name: &str| -> Option<Rect> {
        let a = camf?.area(name)?;
        let (x, y) = (a.x.max(kx) - kx, a.y.max(ky) - ky);
        let (x1, y1) = (
            (a.x + a.w).saturating_sub(kx),
            (a.y + a.h).saturating_sub(ky),
        );
        (x1 > x && y1 > y && x1 <= img.cols && y1 <= img.rows).then(|| Rect {
            x,
            y,
            w: x1 - x,
            h: y1 - y,
        })
    };
    if let Some(a) = area("ActiveImageArea") {
        s.crop = a;
    }
    // The dark rows above the image, or (sd Quattro, which has none) the left dark columns
    // of `DarkShieldColRange` (`[left x0, left x1, right x0, right x1]`) beside it.
    let dark_cols = || -> Option<Rect> {
        let v = camf?.floats("DarkShieldColRange")?;
        let [x0, x1] = <[f64; 2]>::try_from(v.get(..2)?).ok()?;
        let x0 = (x0 as u32).checked_sub(kx)?;
        let x1 = (x1 as u32).checked_sub(kx)?;
        (x1 >= x0 && x1 < s.crop.x && s.crop.h > 0).then(|| Rect {
            x: x0,
            y: s.crop.y,
            w: x1 - x0 + 1,
            h: s.crop.h,
        })
    };
    if let Some(a) = area("DarkShieldTop")
        .filter(|a| a.y + a.h <= s.crop.y)
        .or_else(dark_cols)
    {
        s.black = Black::Masked {
            area: a,
            also: None,
            fallback: [0.0; 4],
        };
    }
    // Saturation of the TRUE (DP1/DP2, SD14/SD15) bodies; Merrill and Quattro clip at their
    // `ImageDepth`.
    if depth.is_none()
        && let Some(v) = camf.and_then(|c| c.floats("RawSaturationLevel"))
        && let Some(m) = v.iter().copied().reduce(f64::max)
        && m > 0.0
        && m < 65535.0
    {
        s.white = Some(m as f32);
    }
    // Layers stored before a digital gain clip at `ImageDepth` over the gain: the least
    // amplified layer sets the white, the white balance carries the rest.
    if let Some(v) = camf.and_then(|c| c.floats("DigitalISOGain"))
        && let Some(g) = v.get(..3)
        && g.iter().all(|g| g.is_finite() && *g >= 1.0)
        && let Some(m) = g.iter().copied().reduce(f64::min)
        && m > 1.0
    {
        s.white = Some((((1u32 << bits) - 1) as f64 / m) as f32);
    }
    if let Some((m, g)) = camf.and_then(|c| colour(c, wb)) {
        s.color = ColorSource::Matrix(m, MatrixSource::CameraCalibration);
        s.wb = g;
    }
    // SD9/SD10 store linearised samples: saturation is the top of the linearisation table
    // (plus the offset the decoder adds to lift negative sums).
    let lin_max = camf
        .and_then(|c| c.floats("LinLUTS"))
        .and_then(|v| {
            v.chunks(4096)
                .take(3)
                .filter_map(|ch| ch.iter().copied().reduce(f64::max))
                .reduce(f64::min)
        })
        .filter(|m| *m > 0.0 && *m < 60000.0)
        .map(|m| m as u16);
    if img.cols == 0 || img.rows == 0 || img.cols > 65535 || img.rows > 65535 {
        return s.unsupported("X3F sensor size");
    }
    let grid = |y0: u32, y1: u32, sd: bool| AfGrid {
        x0: s.crop.x,
        y0,
        x1: s.crop.x + s.crop.w,
        y1,
        step_x: 32,
        step_y: 8,
        sd,
    };
    // AF pixel rows of the sd Quattro and sd Quattro H (sensor rows of the first and past
    // the last AF block), as LibRaw's raw data shows them.
    let af = match (quattro, img.cols, img.rows) {
        (true, 5888, 3672) => Some(grid(s.crop.y, s.crop.y + s.crop.h, false)),
        (true, 5888, 3776) => Some(grid(464, 3314, true)),
        (true, 6656, 4480) => Some(grid(592, 3922, true)),
        _ => None,
    };
    let dark = match s.black {
        Black::Masked { area, .. } => Some(area),
        _ => None,
    };
    let gain = camf
        .filter(|_| !quattro)
        .and_then(|c| spatial_gain(c, wb, lens, s.crop, dark))
        .map(Box::new);
    s.storage = Storage::Vendor {
        range: img.data.clone(),
        codec: match img.kind {
            RAW_HUFFMAN_10BIT | RAW_X530 => VendorCodec::X3fHuffman {
                stride: img.stride as usize,
                white: lin_max,
                gain,
            },
            _ => VendorCodec::X3fTrue { quattro, af, gain },
        },
    };
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(date_time(0).as_deref(), Some("1970:01:01 00:00:00"));
        assert_eq!(
            date_time(1_160_592_724).as_deref(),
            Some("2006:10:11 18:52:04")
        );
        assert_eq!(
            date_time(951_782_400).as_deref(),
            Some("2000:02:29 00:00:00")
        );
        assert_eq!(date_time(-1), None);
    }

    fn le32(v: u32) -> [u8; 4] {
        v.to_le_bytes()
    }

    /// A version 2.2 file: properties, a JPEG-less TRUE raw image and no CAMF.
    pub(crate) fn file(w: u32, h: u32, vals: &[[i32; 3]]) -> Vec<u8> {
        let mut f = b"FOVb".to_vec();
        f.extend(le32(0x20002));
        f.extend([0u8; 20]);
        f.extend(le32(w));
        f.extend(le32(h));
        f.extend(le32(0));
        let mut wb = b"Sunlight".to_vec();
        wb.resize(32, 0);
        f.extend(wb);
        f.resize(128, 0);
        // Properties.
        let prop_off = f.len();
        let pairs = [("CAMMODEL", "SIGMA SD14"), ("ISO", "200"), ("TIME", "0")];
        let mut text: Vec<u16> = Vec::new();
        let mut table = Vec::new();
        for (k, v) in pairs {
            table.push(text.len() as u32);
            text.extend(k.encode_utf16());
            text.push(0);
            table.push(text.len() as u32);
            text.extend(v.encode_utf16());
            text.push(0);
        }
        f.extend(b"SECp");
        f.extend(le32(0x20000));
        f.extend(le32(pairs.len() as u32));
        f.extend(le32(0));
        f.extend(le32(0));
        f.extend(le32(text.len() as u32));
        for t in table {
            f.extend(le32(t));
        }
        for u in text {
            f.extend(u.to_le_bytes());
        }
        let prop_len = f.len() - prop_off;
        // Raw image.
        let img_off = f.len();
        f.extend(b"SECi");
        f.extend(le32(0x20000));
        f.extend(le32(3));
        f.extend(le32(30));
        f.extend(le32(w));
        f.extend(le32(h));
        f.extend(le32(0));
        f.extend(crate::x3f::tests::true_image(w as usize, h as usize, vals));
        let img_len = f.len() - img_off;
        let dir = f.len();
        f.extend(b"SECd");
        f.extend(le32(0x20000));
        f.extend(le32(2));
        for (o, l, t) in [(prop_off, prop_len, b"PROP"), (img_off, img_len, b"IMA2")] {
            f.extend(le32(o as u32));
            f.extend(le32(l as u32));
            f.extend(t);
        }
        f.extend(le32(dir as u32));
        f
    }

    #[test]
    fn true_file() {
        let (w, h) = (6u32, 4u32);
        let vals: Vec<[i32; 3]> = (0..(w * h) as i32)
            .map(|i| [i % 5, 2 + i % 3, 7 - i % 6])
            .collect();
        let f = file(w, h, &vals);
        let raw = crate::parse(&f).unwrap();
        let info = raw.info();
        assert_eq!(info.format, Some(RawFormat::X3f));
        assert_eq!((info.make.as_str(), info.model.as_str()), ("Sigma", "SD14"));
        assert_eq!(info.iso, Some(200));
        assert_eq!(info.date_time.as_deref(), Some("1970:01:01 00:00:00"));
        let s = info.sensor.as_ref().unwrap();
        assert_eq!((s.width, s.height, s.samples_per_pixel), (6, 4, 3));
        assert!(s.decodable);
        let img = raw.decode_raw(&f, &Default::default()).unwrap();
        let want: Vec<u16> = vals.iter().flat_map(|p| p.map(|v| v as u16)).collect();
        assert_eq!(img.data, want);
        assert!(!img.truncated);
        let dev = crate::develop(&img, &Default::default());
        assert!(dev.is_ok());
    }

    #[test]
    fn truncated_and_hostile() {
        let vals: Vec<[i32; 3]> = (0..24).map(|i| [i % 4, i % 3, i % 5]).collect();
        let f = file(6, 4, &vals);
        for n in [4, 8, 63, 64, 100, f.len() / 2, f.len() - 1] {
            let b = &f[..n];
            let _ = crate::parse(b).map(|r| r.decode_raw(b, &Default::default()));
        }
        // Huge sizes, directory pointing anywhere.
        let mut big = f.clone();
        let dir = u32le(&f, f.len() - 4) as usize;
        let img = u32le(&f, dir + 24) as usize;
        big[img + 16..img + 20].copy_from_slice(&le32(60000));
        big[img + 20..img + 24].copy_from_slice(&le32(60000));
        let _ = crate::parse(&big).map(|r| r.decode_raw(&big, &Default::default()));
        let mut bad = f.clone();
        let n = bad.len();
        bad[n - 4..].copy_from_slice(&le32(0xffff_fff0));
        assert!(crate::parse(&bad).is_err());
        // Garbage CAMF of every type.
        for t in [2u32, 4, 5, 9] {
            let mut sec = b"SECc".to_vec();
            sec.extend(le32(0x20000));
            sec.extend(le32(t));
            for v in [0xffff_ffffu32, 7, 3, 0x1234_5678] {
                sec.extend(le32(v));
            }
            sec.extend([0x55u8; 64]);
            let _ = Camf::decode(&sec, 0..sec.len());
        }
    }

    /// A CAMF `CMbM` entry: a one-dimensional f32 matrix.
    fn matrix_entry(name: &str, v: &[f32]) -> Vec<u8> {
        let mut name = name.as_bytes().to_vec();
        name.push(0);
        while !name.len().is_multiple_of(4) {
            name.push(0);
        }
        let head = 20 + name.len();
        let data = head + 24;
        let mut e = b"CMbM".to_vec();
        e.extend(le32(2));
        e.extend(le32((data + 4 * v.len()) as u32));
        e.extend(le32(20));
        e.extend(le32(head as u32));
        e.extend(&name);
        for x in [3, 1, data as u32, v.len() as u32, 0, 0] {
            e.extend(le32(x));
        }
        for x in v {
            e.extend(x.to_le_bytes());
        }
        e
    }

    /// A CAMF `CMbP` property list of (key, value) strings.
    fn property_entry(name: &str, kv: &[(&str, &str)]) -> Vec<u8> {
        let mut name = name.as_bytes().to_vec();
        name.push(0);
        while !name.len().is_multiple_of(4) {
            name.push(0);
        }
        let head = 20 + name.len();
        let base = head + 8 + 8 * kv.len();
        let mut strings = Vec::new();
        let mut offs = Vec::new();
        for (k, v) in kv {
            offs.push(strings.len() as u32);
            strings.extend(k.as_bytes());
            strings.push(0);
            offs.push(strings.len() as u32);
            strings.extend(v.as_bytes());
            strings.push(0);
        }
        let mut e = b"CMbP".to_vec();
        e.extend(le32(2));
        e.extend(le32((base + strings.len()) as u32));
        e.extend(le32(20));
        e.extend(le32(head as u32));
        e.extend(&name);
        e.extend(le32(kv.len() as u32));
        e.extend(le32(base as u32));
        for o in offs {
            e.extend(le32(o));
        }
        e.extend(strings);
        e
    }

    fn camf(entries: &[Vec<u8>]) -> Camf {
        let mut sec = b"SECc".to_vec();
        sec.extend(le32(0x20000));
        sec.extend(le32(2));
        sec.extend([0u8; 16]);
        // Type 2 is XOR-obfuscated with a key stream seeded by the last header word (0 here).
        let plain: Vec<u8> = entries.concat();
        let mut key = 0u32;
        sec.extend(plain.iter().map(|&b| {
            key = key.wrapping_mul(1597).wrapping_add(51749) % 244_944;
            let t = ((key as u64 * 301_593_171) >> 24) as u32;
            let k = ((((key << 8).wrapping_sub(t) >> 1).wrapping_add(t)) >> 17) as u8;
            b ^ k
        }));
        Camf::decode(&sec, 0..sec.len()).expect("camf")
    }

    #[test]
    fn quattro_model_by_size() {
        let head = vec![0u8; 4096];
        assert_eq!(quattro_model(&head, Some((6656, 4480))), "sd Quattro H");
        assert_eq!(quattro_model(&head, Some((5888, 3776))), "sd Quattro");
        assert_eq!(quattro_model(&head, None), "Quattro");
        let mut dp = head.clone();
        dp[100..112].copy_from_slice(b"SIGMA dp2 Qu");
        assert_eq!(quattro_model(&dp, Some((5888, 3776))), "dp2 Quattro");
    }

    /// sd Quattro H: layers stored before `DigitalISOGain` (4, 4, 2). The gain joins the white
    /// balance and the white is the depth over the smallest gain (the magenta-colour bug);
    /// without dark rows (sd Quattro), the left dark columns give the black.
    #[test]
    fn digital_gain_and_dark_columns() {
        let id = [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let base = [
            property_entry("WhiteBalanceGains", &[("Auto", "AutoWBGain")]),
            property_entry("WhiteBalanceColorCorrections", &[("Auto", "AutoCCMatrix")]),
            matrix_entry("AutoWBGain", &[1.0, 1.05, 1.05]),
            matrix_entry("AutoCCMatrix", &id),
            matrix_entry("ImageDepth", &[14.0]),
            matrix_entry("KeepImageArea", &[0.0, 0.0, 99.0, 49.0]),
            matrix_entry("ActiveImageArea", &[20.0, 2.0, 89.0, 47.0]),
            matrix_entry("DarkShieldColRange", &[2.0, 9.0, 92.0, 97.0]),
        ];
        let img = Image {
            kind: RAW_QUATTRO[0],
            cols: 100,
            rows: 50,
            stride: 0,
            data: 0..0,
        };
        let plain = camf(&base);
        let s = sensor(&img, Some(&plain), "Auto", (None, None));
        let wb = s.wb.expect("wb");
        assert!(
            (wb[0] - 1.0 / 1.05).abs() < 1e-3 && (wb[2] - 1.0).abs() < 1e-3,
            "{wb:?}"
        );
        assert_eq!(s.white, None);
        match s.black {
            Black::Masked { area, .. } => assert_eq!(
                area,
                Rect {
                    x: 2,
                    y: 2,
                    w: 8,
                    h: 46
                }
            ),
            _ => panic!("no dark columns"),
        }
        let mut h = base.to_vec();
        h.push(matrix_entry("DigitalISOGain", &[4.0, 4.0, 2.0]));
        let gained = camf(&h);
        let s = sensor(&img, Some(&gained), "Auto", (None, None));
        let g = s.wb.expect("wb");
        assert!((g[0] / g[2] - 2.0 * wb[0] / wb[2]).abs() < 1e-3, "{g:?}");
        assert!((g[1] / g[2] - 2.0 * wb[1] / wb[2]).abs() < 1e-3, "{g:?}");
        assert_eq!(s.white, Some(8191.5));
    }
}
