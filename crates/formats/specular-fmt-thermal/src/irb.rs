//! InfraTec `.irb` (IRBIS stills from VarioCAM and ImageIR cameras).
//!
//! Little-endian. A 64-byte header (`\xffIRB\0`, file type and subtype names, then the
//! block directory's offset and count), 32-byte block entries (type, frame index, offset,
//! size), and blocks. The first IMAGE block holds a 60-byte header (bytes per pixel,
//! compression, size, emissivity, distance, ambient and path temperatures, wavelength), a
//! 256-entry kelvin table, a 644-byte metadata block (calibration range, device, optics,
//! timestamp), and the pixels:
//!
//! * compression 0: per pixel a fraction byte and an index byte, the temperature
//!   interpolated between table entries `index` and `index + 1`;
//! * compression 1: all fraction bytes, then run-length coded (count, index) pairs;
//! * compression 2: the first pixel as a u16, then each next pixel's difference from the
//!   previous as a code read MSB-first from a stream refilled 16 bits (little-endian) at a
//!   time: an 8-bit prefix below 192 is the code, otherwise 11 more bits extend it (minus
//!   192 x 2047); the difference is the code halved, negative when odd. Pixels are
//!   centikelvin.
//!
//! Layout as documented by the Apache-2.0 `irb` reader (Jonathan Schilling) and the MIT
//! Irbis-File-Format notes (Thomas Zeutschler); only field positions and the coding are
//! taken from them. Video files (several frames) give their first image.

use crate::{Camera, Conditions, Error, Kind, Mapping, Pixels, Result, Thermal, malformed};

const F: &str = "InfraTec IRB";
const MAGIC: &[u8] = b"\xffIRB\0";
const KELVIN: f64 = 273.15;
const IMAGE: u32 = 1;
const HEADER: usize = 60;
const TABLE: usize = 1024;
const META: usize = 644;
const DATA: usize = HEADER + TABLE + META;

pub(crate) fn is_irb(d: &[u8]) -> bool {
    d.starts_with(MAGIC) && d.len() >= 64
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

fn f32_at(d: &[u8], at: usize) -> Option<f64> {
    let v = f64::from(f32::from_bits(u32_at(d, at)?));
    v.is_finite().then_some(v)
}

fn f64_at(d: &[u8], at: usize) -> Option<f64> {
    let v = f64::from_bits(u64::from_le_bytes(d.get(at..at + 8)?.try_into().ok()?));
    v.is_finite().then_some(v)
}

fn text(d: &[u8], at: usize, n: usize) -> String {
    let Some(s) = d.get(at..(at + n).min(d.len())) else {
        return String::new();
    };
    let s = &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())];
    s.iter()
        .map(|&b| char::from(b))
        .collect::<String>()
        .trim()
        .to_owned()
}

pub(crate) fn read(d: &[u8]) -> Result<Thermal> {
    if !is_irb(d) {
        return Err(Error::NotThermal);
    }
    let dir = u32_at(d, 25).unwrap_or(0) as usize;
    let count = u32_at(d, 29).unwrap_or(0) as usize;
    if count > d.len() / 32 || dir.saturating_add(32 * count) > d.len() {
        return Err(malformed(F, "block directory past the end"));
    }
    let block = (0..count)
        .map(|i| dir + 32 * i)
        .find_map(|e| {
            let ty = u32_at(d, e)?;
            let off = u32_at(d, e + 12)? as usize;
            let size = u32_at(d, e + 16)? as usize;
            (ty == IMAGE && size > 0).then_some((off, size))
        })
        .ok_or_else(|| Error::Unsupported {
            format: F,
            what: "no image block".into(),
        })?;
    let (off, size) = block;
    let b = d
        .get(off..off.saturating_add(size).min(d.len()))
        .filter(|b| b.len() >= DATA)
        .ok_or_else(|| malformed(F, "image block past the end"))?;
    image(b)
}

fn image(b: &[u8]) -> Result<Thermal> {
    let compression = u16_at(b, 2).unwrap_or(0);
    let width = usize::from(u16_at(b, 4).unwrap_or(0));
    let height = usize::from(u16_at(b, 6).unwrap_or(0));
    if width == 0 || height == 0 || width > 10_000 || height > 10_000 {
        return Err(malformed(F, format!("image size {width} x {height}")));
    }
    let n = width * height;
    let table: Vec<f32> = (0..256)
        .map(|i| f32::from_bits(u32_at(b, HEADER + 4 * i).unwrap_or(0)))
        .collect();
    let lerp = |frac: u8, idx: u8| -> f32 {
        let i = usize::from(idx);
        let f = f32::from(frac) / 256.0;
        let v = table[(i + 1).min(255)] * f + table[i] * (1.0 - f);
        (f64::from(v.max(0.0)) - KELVIN) as f32
    };
    let px = &b[DATA..];
    let pixels = match compression {
        0 => {
            let px = px
                .get(..2 * n)
                .ok_or_else(|| malformed(F, "pixels shorter than the image"))?;
            Pixels::Celsius(px.chunks_exact(2).map(|p| lerp(p[0], p[1])).collect())
        }
        1 => {
            let fracs = px
                .get(..n)
                .ok_or_else(|| malformed(F, "pixels shorter than the image"))?;
            let mut runs = px[n..].chunks_exact(2);
            let mut out = Vec::with_capacity(n);
            let (mut left, mut idx) = (0u32, 0u8);
            for &frac in fracs {
                if left == 0 {
                    let r = runs
                        .next()
                        .ok_or_else(|| malformed(F, "run-length data ends early"))?;
                    (left, idx) = (u32::from(r[0]), r[1]);
                }
                out.push(lerp(frac, idx));
                left = left.saturating_sub(1);
            }
            Pixels::Celsius(out)
        }
        2 => Pixels::Counts {
            values: deltas(px, n)?,
            mapping: Mapping::Linear {
                divisor: 100.0,
                offset: KELVIN,
            },
        },
        c => {
            return Err(Error::Unsupported {
                format: F,
                what: format!("compression {c}"),
            });
        }
    };
    let m = HEADER + TABLE;
    let c = |k: Option<f64>| k.filter(|&k| k > 0.0).map(|k| k - KELVIN);
    let range = match (f32_at(b, m + 92), f32_at(b, m + 96)) {
        (Some(lo), Some(hi)) if hi > lo && lo > 0.0 => Some((lo - KELVIN, hi - KELVIN)),
        _ => None,
    };
    let lens = [text(b, m + 202, 32), text(b, m + 554, 48)]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(Thermal {
        kind: Kind::InfratecIrb,
        width,
        height,
        pixels,
        conditions: Conditions {
            emissivity: f32_at(b, 24).filter(|&e| e > 0.0),
            distance: f32_at(b, 28),
            reflected: c(f32_at(b, 32)),
            atmospheric: c(f32_at(b, 40)),
            ..Default::default()
        },
        camera: Camera {
            model: text(b, m + 142, 12),
            serial: text(b, m + 164, 16),
            lens,
            lens_serial: text(b, m + 450, 16),
            range,
            wavelength: f32_at(b, 48).filter(|&w| w > 0.0),
            date: f64_at(b, m + 540)
                .filter(|&t| t > 0.0)
                .map(|t| ole_date(t, u32_at(b, m + 548).unwrap_or(0))),
            ..Default::default()
        },
        palette: None,
        visible: None,
        pip: None,
    })
}

/// "YYYY-MM-DD HH:MM:SS.mmm" from an OLE automation date (days since 1899-12-30), whole
/// seconds from it and milliseconds as recorded separately.
fn ole_date(days: f64, ms: u32) -> String {
    let secs = ((days - 25_569.0) * 86_400.0).round() as i64;
    let (d, t) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, mo, da) = crate::flir::civil(d);
    format!(
        "{y:04}-{mo:02}-{da:02} {:02}:{:02}:{:02}.{:03}",
        t / 3600,
        t / 60 % 60,
        t % 60,
        ms % 1000
    )
}

/// Compression 2: 16-bit values coded as differences in an MSB-first bit stream refilled
/// from little-endian 16-bit words.
pub(crate) fn deltas(d: &[u8], n: usize) -> Result<Vec<u16>> {
    let first = u16_at(d, 0).ok_or_else(|| malformed(F, "no pixel data"))?;
    let mut out = Vec::with_capacity(n);
    out.push(first);
    let mut pos = 2;
    let (mut buf, mut bits) = (0u64, 0u32);
    let mut take = |k: u32| -> Option<u32> {
        while bits < k {
            let w = u16_at(d, pos)?;
            pos += 2;
            buf |= u64::from(w) << (48 - bits);
            bits += 16;
        }
        let v = (buf >> (64 - k)) as u32;
        buf <<= k;
        bits -= k;
        Some(v)
    };
    let mut prev = first;
    for _ in 1..n {
        let p = take(8).ok_or_else(|| malformed(F, "coded pixels end early"))?;
        let u = if p < 192 {
            p
        } else {
            let ext = take(11).ok_or_else(|| malformed(F, "coded pixels end early"))?;
            ((p << 11) | ext) - 192 * 2047
        };
        let mag = (u >> 1) as u16;
        prev = if u & 1 == 1 {
            prev.wrapping_sub(mag)
        } else {
            prev.wrapping_add(mag)
        };
        out.push(prev);
    }
    Ok(out)
}
