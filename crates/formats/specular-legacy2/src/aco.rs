//! Photoshop colour swatches (`.aco`), shown as a swatch sheet.
//!
//! Big-endian. Version 1: a version word (1), a count, then per colour a colour-space word
//! and four 16-bit values. Version 2 (written after version 1's colours, or alone) has the
//! same records, each followed by the colour's name: a 32-bit count of UTF-16 units
//! (including the closing NUL) and the units.
//!
//! Spaces: 0 RGB (0–65535), 1 HSB (hue 0–65535 for 0–360°), 2 CMYK (65535 = no ink), 7 Lab
//! (L 0–10000, a and b signed hundredths), 8 grayscale (0–10000 of black), 9 wide CMYK
//! (0–10000 of ink). Colour-book spaces (Pantone, Focoltone, Trumatch, Toyo, HKS) store
//! only a book reference: they are listed by name and drawn grey.

use crate::swatch::{
    self, MAX_COLOURS, Palette, Swatch, cmyk_to_rgb, hsv_to_rgb, lab_to_rgb, unit8, utf16be,
};
use crate::{Error, Image, Info, Result};

fn u16be(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn u32be(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// A version 1 or 2 header whose records fit the file and use known colour spaces.
pub fn probe(data: &[u8]) -> bool {
    let (Some(v), Some(n)) = (u16be(data, 0), u16be(data, 2)) else {
        return false;
    };
    if !(v == 1 || v == 2) || n == 0 {
        return false;
    }
    let n = n as usize;
    if v == 1 && data.len() < 4 + n * 10 {
        return false;
    }
    let mut at = 4;
    for _ in 0..n.min(64) {
        match u16be(data, at) {
            Some(s) if s <= 10 => {}
            _ => return false,
        }
        at += 10;
        if v == 2 {
            match u32be(data, at) {
                Some(l) if l < 1024 => at += 4 + l as usize * 2,
                _ => return false,
            }
        }
    }
    at <= data.len()
}

fn colour(space: u16, w: u16, x: u16, y: u16, z: u16) -> ([u8; 3], String) {
    let f = |v: u16| f64::from(v) / 65535.0;
    match space {
        0 => ([unit8(f(w)), unit8(f(x)), unit8(f(y))], String::new()),
        1 => {
            let (h, s, b) = (f(w) * 360.0, f(x), f(y));
            let c = hsv_to_rgb(h, s, b);
            (
                c.map(unit8),
                format!("HSB {:.0}° {:.0}% {:.0}%", h, s * 100.0, b * 100.0),
            )
        }
        2 => {
            let ink = |v: u16| 1.0 - f(v);
            let (c, m, yy, k) = (ink(w), ink(x), ink(y), ink(z));
            (
                cmyk_to_rgb(c, m, yy, k),
                format!(
                    "CMYK {:.0} {:.0} {:.0} {:.0} %",
                    c * 100.0,
                    m * 100.0,
                    yy * 100.0,
                    k * 100.0
                ),
            )
        }
        7 => {
            let l = f64::from(w) / 100.0;
            let (a, b) = (f64::from(x as i16) / 100.0, f64::from(y as i16) / 100.0);
            (lab_to_rgb(l, a, b), format!("Lab {l:.1} {a:.1} {b:.1}"))
        }
        8 => {
            let k = (f64::from(w) / 10000.0).clamp(0.0, 1.0);
            let g = unit8(1.0 - k);
            ([g, g, g], format!("Gray {:.0} %", k * 100.0))
        }
        9 => {
            let p = |v: u16| (f64::from(v) / 10000.0).clamp(0.0, 1.0);
            let (c, m, yy, k) = (p(w), p(x), p(y), p(z));
            (
                cmyk_to_rgb(c, m, yy, k),
                format!(
                    "CMYK {:.0} {:.0} {:.0} {:.0} %",
                    c * 100.0,
                    m * 100.0,
                    yy * 100.0,
                    k * 100.0
                ),
            )
        }
        3 => ([128; 3], "Pantone book colour, shown grey".into()),
        4 => ([128; 3], "Focoltone book colour, shown grey".into()),
        5 => ([128; 3], "Trumatch book colour, shown grey".into()),
        6 => ([128; 3], "Toyo book colour, shown grey".into()),
        10 => ([128; 3], "HKS book colour, shown grey".into()),
        s => ([128; 3], format!("colour space {s}, shown grey")),
    }
}

/// The records of one section from `at`; returns them and where the section ends.
fn section(data: &[u8], at: usize) -> Result<(u16, Vec<Swatch>, usize)> {
    let v = u16be(data, at).ok_or_else(|| Error::Truncated("swatch header".into()))?;
    let n = u16be(data, at + 2).ok_or_else(|| Error::Truncated("swatch header".into()))? as usize;
    if !(v == 1 || v == 2) {
        return Err(Error::Malformed(format!("swatch version {v}")));
    }
    if n > MAX_COLOURS {
        return Err(Error::Malformed("too many colours".into()));
    }
    let mut p = at + 4;
    let mut out = Vec::with_capacity(n.min(4096));
    for i in 0..n {
        let rec = data
            .get(p..p + 10)
            .ok_or_else(|| Error::Truncated(format!("colour {} of {n}", i + 1)))?;
        let g = |k: usize| u16::from_be_bytes([rec[k], rec[k + 1]]);
        let (rgb, stored) = colour(g(0), g(2), g(4), g(6), g(8));
        p += 10;
        let mut name = String::new();
        if v == 2 {
            let len = u32be(data, p).ok_or_else(|| Error::Truncated("colour name".into()))?;
            let bytes = (len as usize).saturating_mul(2);
            let s = data
                .get(p + 4..(p + 4).saturating_add(bytes))
                .ok_or_else(|| Error::Truncated("colour name".into()))?;
            name = swatch::short(&utf16be(s));
            p = p + 4 + bytes;
        }
        out.push(Swatch {
            name,
            rgba: [rgb[0], rgb[1], rgb[2], 255],
            stored,
            group: String::new(),
        });
    }
    Ok((v, out, p))
}

pub fn parse(data: &[u8]) -> Result<Palette> {
    if u16be(data, 0).is_none_or(|v| v != 1 && v != 2) {
        return Err(Error::NotRecognised);
    }
    let (v, mut swatches, end) = section(data, 0)?;
    let mut version = v;
    // The names, in a version 2 section after the first.
    if v == 1
        && data.len() >= end + 4
        && u16be(data, end) == Some(2)
        && let Ok((_, named, _)) = section(data, end)
        && named.len() == swatches.len()
    {
        swatches = named;
        version = 2;
    }
    Ok(Palette {
        name: String::new(),
        swatches,
        columns: 0,
        fields: vec![("Version".into(), version.to_string())],
        coding: if version == 2 {
            "Photoshop swatches, named".into()
        } else {
            "Photoshop swatches".into()
        },
    })
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    parse(data)?.info()
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    parse(data)?.render()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A file with both sections; `colours`: (space, values, name).
    pub(crate) fn file(colours: &[(u16, [u16; 4], &str)]) -> Vec<u8> {
        let mut d = Vec::new();
        for v in [1u16, 2] {
            d.extend_from_slice(&v.to_be_bytes());
            d.extend_from_slice(&(colours.len() as u16).to_be_bytes());
            for (s, w, name) in colours {
                d.extend_from_slice(&s.to_be_bytes());
                for x in w {
                    d.extend_from_slice(&x.to_be_bytes());
                }
                if v == 2 {
                    let u: Vec<u16> = name.encode_utf16().chain([0]).collect();
                    d.extend_from_slice(&(u.len() as u32).to_be_bytes());
                    for c in u {
                        d.extend_from_slice(&c.to_be_bytes());
                    }
                }
            }
        }
        d
    }

    #[test]
    fn reads_both_sections_and_spaces() {
        let f = file(&[
            (0, [65535, 0, 0, 0], "Red"),
            (2, [65535, 0, 65535, 65535], "Magenta"),
            (7, [10000, 0, 0, 0], "White"),
            (8, [10000, 0, 0, 0], "Black"),
            (3, [0, 0, 0, 0], "Book"),
        ]);
        assert!(probe(&f));
        let p = parse(&f).unwrap();
        assert_eq!(p.swatches.len(), 5);
        assert_eq!(p.swatches[0].name, "Red");
        assert_eq!(p.swatches[0].rgba, [255, 0, 0, 255]);
        assert_eq!(p.swatches[1].rgba, [255, 0, 255, 255]);
        assert_eq!(p.swatches[2].rgba, [255, 255, 255, 255]);
        assert_eq!(p.swatches[3].rgba, [0, 0, 0, 255]);
        assert!(p.swatches[4].stored.contains("Pantone"));
        // Version 1 only, cut short.
        let mut v1 = f[..4 + 5 * 10].to_vec();
        assert_eq!(parse(&v1).unwrap().swatches[0].name, "");
        v1.truncate(30);
        assert!(matches!(parse(&v1), Err(Error::Truncated(_))));
        assert!(!probe(b"\x00\x03\x00\x01"));
    }
}
