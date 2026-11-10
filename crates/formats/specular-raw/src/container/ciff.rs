//! Canon CRW: a CIFF heap file. A heap ends with the offset of its record table; each 10-byte
//! record is a type, a size and an offset (relative to the heap), or holds its value inline.
//! Types `0x28xx` / `0x30xx` are sub-heaps. The sensor data (`0x2005`) is coded with
//! [`crate::crw`]; the full-size JPEG (`0x2007`) and the thumbnail (`0x2008`) are records too.

use super::{Black, Previews, Sensor, Storage};
use crate::cfa::Cfa;
use crate::error::{RawError, Result};
use crate::vendor::VendorCodec;
use crate::{RawFormat, RawInfo, Rect, SensorCompression};
use std::ops::Range;

pub(super) fn is_crw(d: &[u8]) -> bool {
    d.len() >= 14 && &d[..2] == b"II" && &d[6..14] == b"HEAPCCDR"
}

fn u16_at(d: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        d.get(p..p.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(d: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        d.get(p..p.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// A record: its type and either its data range or its inline value (`size`, `offset`).
#[derive(Clone, Debug)]
enum Rec {
    Data(u16, Range<usize>),
    Inline(u16, u32),
}

/// Collects the records of the heap at `heap` (all depths up to 4, at most 4096 records).
fn walk(d: &[u8], heap: Range<usize>, depth: u32, out: &mut Vec<Rec>) {
    if heap.len() < 6 || heap.end > d.len() || out.len() >= 4096 {
        return;
    }
    let Some(table) = u32_at(d, heap.end - 4).and_then(|t| heap.start.checked_add(t as usize))
    else {
        return;
    };
    let Some(n) = u16_at(d, table) else { return };
    for k in 0..(n as usize).min(127) {
        let p = table + 2 + 10 * k;
        let (Some(t), Some(size), Some(off)) = (u16_at(d, p), u32_at(d, p + 2), u32_at(d, p + 6))
        else {
            return;
        };
        if t & 0xc000 == 0x4000 {
            out.push(Rec::Inline(t & 0x3fff, size));
            continue;
        }
        let start = heap.start.saturating_add(off as usize);
        let end = start.saturating_add(size as usize);
        if end > heap.end {
            continue;
        }
        let kind = (t & 0x3fff) >> 8;
        if (kind == 0x28 || kind == 0x30) && depth < 4 {
            walk(d, start..end, depth + 1, out);
        } else {
            out.push(Rec::Data(t & 0x3fff, start..end));
        }
        if out.len() >= 4096 {
            return;
        }
    }
}

/// `YYYY:MM:DD HH:MM:SS` of a Unix time.
fn date(t: u32) -> String {
    let days = (t / 86400) as i64;
    let secs = t % 86400;
    // Civil date from days since 1970-01-01 (proleptic Gregorian).
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}:{month:02}:{day:02} {:02}:{:02}:{:02}",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

pub(super) fn parse(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    let hl = u32_at(d, 2).ok_or(RawError::Malformed("CRW header"))? as usize;
    if hl < 14 || hl >= d.len() {
        return Err(RawError::Malformed("CRW header"));
    }
    let mut recs = Vec::new();
    walk(d, hl..d.len(), 0, &mut recs);
    let mut info = RawInfo {
        format: Some(RawFormat::Crw),
        orientation: 1,
        ..Default::default()
    };
    let mut pv = Previews::default();
    let mut raw = None;
    let mut table = None;
    let mut sensor_info = None;
    let mut balance = None;
    let mut color_info = None;
    let mut d30 = None;
    let mut color1 = None;
    let mut wbi = None;
    for r in &recs {
        match r {
            Rec::Data(0x2005, r) => raw = Some(r.clone()),
            Rec::Data(0x2007, r) => {
                pv.jpeg(d, r.clone(), "CRW JpgFromRaw");
            }
            Rec::Data(0x2008, r) => {
                pv.jpeg(d, r.clone(), "CRW ThumbnailImage");
            }
            Rec::Data(0x080a, r) => {
                let mut parts = d[r.clone()].split(|&b| b == 0);
                let mut text = || {
                    parts
                        .next()
                        .map(|s| String::from_utf8_lossy(s).trim().to_string())
                        .unwrap_or_default()
                };
                info.make = text();
                info.model = text();
            }
            Rec::Data(0x080b, r) => {
                let s = d[r.clone()].split(|&b| b == 0).next().unwrap_or(&[]);
                if !s.is_empty() {
                    info.software = Some(String::from_utf8_lossy(s).trim().to_string());
                }
            }
            Rec::Data(0x1810, r) if r.len() >= 16 => {
                info.orientation = match u32_at(d, r.start + 12).map(|v| v as i32) {
                    Some(90) => 6,
                    Some(180) => 3,
                    Some(270) | Some(-90) => 8,
                    _ => 1,
                };
            }
            Rec::Data(0x180e, r) if r.len() >= 4 => {
                if let Some(t) = u32_at(d, r.start).filter(|&t| t != 0) {
                    info.date_time = Some(date(t));
                }
            }
            Rec::Data(0x1835, r) if r.len() >= 4 => table = u32_at(d, r.start),
            Rec::Data(0x1031, r) if r.len() >= 18 => sensor_info = Some(r.start),
            Rec::Data(0x10a9, r) if r.len() >= 2 + 8 * 2 => balance = Some(r.clone()),
            Rec::Data(0x102c, r) if r.len() >= 128 => color_info = Some(r.start),
            Rec::Data(0x0032, r) if r.len() == 768 => d30 = Some(r.start),
            Rec::Data(0x0032, r) if r.len() >= 80 + 18 * 8 => color1 = Some(r.start),
            Rec::Data(0x102a, r) if r.len() >= 16 => {
                let g = |i: usize| u16_at(d, r.start + 2 * i).unwrap_or(0);
                let iso = 50.0 * 2f64.powf(g(2) as f64 / 32.0 - 4.0);
                if g(2) != 0 && (1.0..1e6).contains(&iso) {
                    info.iso = Some(iso.round() as u32);
                }
                let av = g(4) as i16;
                if av != 0 {
                    info.f_number = Some((2f64.powf(av as f64 / 64.0) * 10.0).round() / 10.0);
                }
                let tv = g(5) as i16;
                if tv != 0 {
                    info.exposure_time = Some(2f64.powf(-(tv as f64) / 32.0));
                }
                wbi = Some(g(7));
            }
            Rec::Inline(0x1029 | 0x5029, v) => {
                let mut f = (v >> 16) as f64;
                if v & 0xffff == 2 {
                    f /= 32.0;
                }
                if f > 0.0 {
                    info.focal_length = Some(f);
                }
            }
            _ => {}
        }
    }
    info.previews = pv.finish(info.orientation);
    let sensor = raw.map(|raw| {
        let mut s = sensor(d, raw, table.unwrap_or(0), sensor_info, &info.model);
        s.wb = white_balance(d, wbi, balance, color_info, d30, color1, &info.model);
        if let Some(f) = s.four.as_mut() {
            // Four-colour bodies (Pro90 IS, G1): C, Y, G, M multipliers at byte 120 of the
            // colour record.
            s.wb = None;
            f.mul = color_info
                .filter(|&p| u16_at(d, p).is_some_and(|k| k > 512))
                .and_then(|p| {
                    let w: Vec<f32> = (0..4)
                        .map(|i| u16_at(d, p + 120 + 2 * i).map(f32::from))
                        .collect::<Option<_>>()?;
                    Some([w[2], w[3], w[0], w[1]])
                })
                .filter(|m| m.iter().all(|&v| v > 0.0));
        }
        s
    });
    info.sensor = sensor.as_ref().map(Sensor::info);
    Ok((info, sensor))
}

fn sensor(d: &[u8], raw: Range<usize>, table: u32, si: Option<usize>, model: &str) -> Sensor {
    let g = |i: usize| si.and_then(|p| u16_at(d, p + 2 * i)).unwrap_or(0) as u32;
    let (w, h) = (g(1), g(2));
    let rec = &d[raw.clone()];
    let low_bits = crate::crw::has_low_bits(rec);
    let mut s = Sensor::new(
        w,
        h,
        if low_bits { 12 } else { 10 },
        SensorCompression::Other(0),
    );
    // The Pro70 stores plain 10-bit samples (MSB first in little-endian words) without a
    // sensor record; its colour filter repeats over eight rows.
    if model.contains("PowerShot Pro70") {
        let (w, h) = (1552u32, 1024u32);
        let mut s = Sensor::new(w, h, 10, SensorCompression::Uncompressed);
        // Rows of the filter pattern: Y C, M G, C Y, G M, Y C, G M, C Y, M G.
        s.cfa = cmyg(&["YC", "MG", "CY", "GM", "YC", "GM", "CY", "MG"]);
        s.white = Some(1023.0);
        s.four = four_color(model, None);
        s.storage = Storage::Blocks {
            blocks: vec![raw],
            tile_w: w,
            tile_h: h,
            codec: crate::container::Codec::Raw {
                packing: crate::container::Packing::Msb16Le(10),
                row_bytes: 0,
            },
        };
        return s;
    }
    if si.is_none() || w < 64 || h < 8 || w > 16384 || h > 16384 {
        return s.unsupported("CRW sensor size");
    }
    s.cfa = Cfa::bayer("RGGB");
    // Crop: the camera's own active area (inclusive corners).
    let (l, t, r, b) = (g(5), g(6), g(7), g(8));
    if l < r && t < b && r < w && b < h {
        s.crop = Rect {
            x: l,
            y: t,
            w: r - l + 1,
            h: b - t + 1,
        };
    }
    // Pro90 IS and G1: green magenta / yellow cyan from the sensor origin.
    if model.contains("PowerShot Pro90") || model.ends_with("PowerShot G1") {
        s.cfa = cmyg(&["GM", "YC"]);
        s.four = four_color(model, None);
    }
    // Optical black: the camera's black mask (inclusive corners), else part of the left margin.
    let (ml, mt, mr, mb) = (g(9), g(10), g(11), g(12));
    let area = if ml <= mr && mt <= mb && mr < w && mb < h && (mr - ml + 1) * (mb - mt + 1) >= 64 {
        Some(Rect {
            x: ml,
            y: mt,
            w: mr - ml + 1,
            h: mb - mt + 1,
        })
    } else {
        (s.crop.x >= 16).then(|| Rect {
            x: 2,
            y: s.crop.y,
            w: s.crop.x * 3 / 8 - 2,
            h: s.crop.h,
        })
    };
    if let Some(area) = area {
        s.black = Black::Masked {
            area,
            also: None,
            fallback: [0.0; 4],
        };
    }
    s.storage = Storage::Vendor {
        range: raw,
        codec: VendorCodec::Crw { table, low_bits },
    };
    s
}

/// As-shot white balance as `[R/G, 1, B/G]`.
/// A cyan, magenta, yellow and green filter pattern from its rows (two columns each).
fn cmyg(rows: &[&str]) -> Option<Cfa> {
    let colors = rows
        .iter()
        .flat_map(|r| r.bytes())
        .map(|b| match b {
            b'G' => 1,
            b'C' => 3,
            b'M' => 4,
            _ => 5,
        })
        .collect();
    Cfa::new(2, rows.len() as u32, colors)
}

/// The published four-colour matrix of `model`, with the as-shot multipliers (G, M, C, Y).
fn four_color(model: &str, mul: Option<[f32; 4]>) -> Option<Box<crate::cfagen::FourColor>> {
    let e = crate::color::four_color_entry("Canon", model)?;
    crate::cfagen::FourColor::new(e.colors, &e.matrix, mul).map(Box::new)
}

fn white_balance(
    d: &[u8],
    wbi: Option<u16>,
    balance: Option<Range<usize>>,
    color_info: Option<usize>,
    d30: Option<usize>,
    color1: Option<usize>,
    model: &str,
) -> Option<[f32; 3]> {
    let norm =
        |r: f32, g: f32, b: f32| (r > 0.0 && g > 0.0 && b > 0.0).then(|| [r / g, 1.0, b / g]);
    let v = |p: usize| u16_at(d, p).unwrap_or(0) as f32;
    if let Some(r) = balance {
        // Records of R, G, G, B: Auto, Daylight, Shade, Cloudy, Tungsten, Fluorescent, Flash,
        // Custom, Kelvin.
        let n = (r.len() - 2) / 8;
        let rec = match wbi.unwrap_or(0) {
            0 | 7 => 0,
            1 => 1,
            2 => 3,
            3 => 4,
            4 => 5,
            5 => 6,
            6 => 7,
            8 => 2,
            9 => 8,
            _ => 0,
        };
        if rec < n {
            let p = r.start + 2 + 8 * rec;
            return norm(v(p), (v(p + 2) + v(p + 4)) / 2.0, v(p + 6));
        }
    }
    if let Some(p) = d30 {
        // D30: the as-shot multipliers' reciprocals (×1024) in R, G, G, B; auto has none.
        // Under Auto the record holds no measurement of the shot (its values tint a test
        // shot badly: B/G 0.27 of the camera's JPEG), so the develop estimates instead.
        if wbi.unwrap_or(0) == 0 {
            return None;
        }
        let q = |i: usize| v(p + 72 + 2 * i);
        let (r, g1, g2, b) = (q(0), q(1), q(2), q(3));
        if r > 0.0 && g1 > 0.0 && g2 > 0.0 && b > 0.0 {
            let g = 2.0 / (1.0 / g1 + 1.0 / g2);
            return norm(g / r, 1.0, g / b);
        }
        return None;
    }
    // Colour data 1 of the later PowerShots: a table of presets (G, R, B, G multipliers), XOR
    // obfuscated in the Pro1, G6, S60 and S70 (marked by the key as the first word); the
    // shot's preset picks the entry. Its Auto entry is not the shot's measurement (a G6 test
    // shot: R/G 0.81, B/G 1.36 of the camera's JPEG, against 0.88, 1.00 estimated), so Auto
    // is left to the develop's estimate.
    if let Some(p) = color1
        && let Some(w) = wbi.filter(|&w| w != 0)
    {
        let w = w as usize;
        let keyed = u16_at(d, p) == Some(0x410);
        let map: &[u8] = match (keyed, model.contains("Pro1")) {
            (true, true) => b"012346000000000000",
            (true, false) => b"01345:000000006008",
            (false, _) => b"023457000000006000",
        };
        let idx = (*map.get(w).unwrap_or(&b'0') - b'0') as usize + if keyed { 2 } else { 0 };
        let key = if keyed { [0x410u16, 0x45f3] } else { [0, 0] };
        let m: [f32; 4] = std::array::from_fn(|c| {
            (u16_at(d, p + 80 + idx * 8 + 2 * c).unwrap_or(0) ^ key[c & 1]) as f32
        });
        if let Some(wb) = norm(m[1], (m[0] + m[3]) / 2.0, m[2]) {
            return Some(wb);
        }
    }
    if let Some(p) = color_info {
        let kind = u16_at(d, p).unwrap_or(0);
        if kind > 512 {
            // Four-colour multipliers (Pro90 IS, G1): see the sensor.
            return None;
        }
        if kind != 276 {
            // G, R, B, G at byte 100.
            return norm(v(p + 102), (v(p + 100) + v(p + 106)) / 2.0, v(p + 104));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A heap: data blobs then the record table, the table offset last.
    fn heap(recs: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut body = Vec::new();
        let mut table = (recs.len() as u16).to_le_bytes().to_vec();
        for (t, b) in recs {
            table.extend(t.to_le_bytes());
            table.extend((b.len() as u32).to_le_bytes());
            table.extend((body.len() as u32).to_le_bytes());
            body.extend(b);
        }
        let at = body.len() as u32;
        body.extend(table);
        body.extend(at.to_le_bytes());
        body
    }

    fn u16s(v: &[u16]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn file(w: u16, h: u16) -> Vec<u8> {
        // Every block: first difference 0, then end of block: all samples 512.
        // No `FF` in the coded data: the low-order bits (all zero) come first.
        let mut raw = vec![0u8; crate::crw::CODED_AT + w as usize * h as usize / 4];
        let blocks = w as usize * h as usize / 64;
        let mut acc = 0u64;
        let mut n = 0;
        for _ in 0..blocks {
            for (c, l) in [
                crate::crw::test_code(true, 0, 0),
                crate::crw::test_code(false, 0, 0),
            ] {
                acc = acc << l | c as u64;
                n += l;
                while n >= 8 {
                    raw.push((acc >> (n - 8)) as u8);
                    n -= 8;
                }
            }
        }
        raw.extend([0, 0, 0, 0]);
        let mut sensor = vec![34, w, h, 1, 1, 8, 2, w - 9, h - 3];
        sensor.resize(17, 0);
        let mut balance = vec![82u16];
        balance.extend([100, 50, 50, 200]);
        balance.extend([200, 100, 100, 150]);
        balance.resize(41, 0);
        let mut image = vec![0u8; 28];
        image[12..16].copy_from_slice(&90u32.to_le_bytes());
        let mut shot = vec![0u16; 33];
        shot[2] = 160;
        shot[7] = 1;
        let sub = heap(&[
            (0x080a, b"Canon\0Canon EOS 300D DIGITAL\0".to_vec()),
            (0x1810, image),
            (0x1031, u16s(&sensor)),
            (0x1835, 0u32.to_le_bytes().to_vec()),
            (0x10a9, u16s(&balance)),
            (0x102a, u16s(&shot)),
        ]);
        let mut f = b"II".to_vec();
        f.extend(26u32.to_le_bytes());
        f.extend(b"HEAPCCDR");
        f.extend([0u8; 12]);
        f.extend(heap(&[(0x2005, raw), (0x300a, sub)]));
        f
    }

    #[test]
    fn parses_and_decodes() {
        let f = file(128, 16);
        assert!(is_crw(&f));
        let raw = crate::parse(&f).unwrap();
        let info = raw.info();
        assert_eq!(info.format, Some(RawFormat::Crw));
        assert_eq!(info.model, "Canon EOS 300D DIGITAL");
        assert_eq!(info.orientation, 6);
        assert_eq!(info.iso, Some(100));
        let img = raw.decode_raw(&f, &Default::default()).unwrap();
        assert_eq!((img.width, img.height), (128, 16));
        assert!(img.data.iter().all(|&v| v == 512 << 2));
        // Daylight (wbi 1): R 200, G 100, B 150.
        assert_eq!(img.wb, Some([2.0, 1.0, 1.5]));
        assert!(!img.truncated);
    }

    #[test]
    fn truncated_and_hostile() {
        let f = file(128, 16);
        for n in [6, 14, 26, 40, 600, f.len() / 2, f.len() - 1] {
            let _ = crate::parse(&f[..n]).map(|r| r.decode_raw(&f[..n], &Default::default()));
        }
        // A table offset pointing back at itself and huge sizes.
        let mut bad = f.clone();
        let n = bad.len();
        bad[n - 4..].copy_from_slice(&0xffff_fff0u32.to_le_bytes());
        let _ = crate::parse(&bad);
        assert_eq!(date(0), "1970:01:01 00:00:00");
        assert_eq!(date(951_782_400), "2000:02:29 00:00:00");
    }
}
