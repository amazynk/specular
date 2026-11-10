//! McIDAS AREA files (`.area`, `.ara`): weather satellite images (GOES, Meteosat, Himawari)
//! from the McIDAS system.
//!
//! A directory of 64 32-bit words (big endian, or little endian on files written on
//! little-endian machines): word 1 is 4 (the type), 2 the sensor source, 3 the image date
//! (YYYDDD), 4 the time (HHMMSS), 5–6 the upper-left image line and element, 8 the lines,
//! 9 the elements per line, 10 the bytes per element (1, 2 or 4), 11–12 the line and element
//! resolution, 13 the bands, 14 the line-prefix bytes, 24–31 a memo, 33 where the data
//! starts. Each line is its prefix, then the elements with their bands interleaved.
//!
//! 1-byte data shows as stored; 2- and 4-byte data (counts or calibrated values) are
//! stretched from their own range. Every band is a page. Navigation and calibration blocks
//! are not read.

use crate::{Error, Image, Info, Model, Pixels, Result, check_dims, check_payload, stretch16};

const DIRECTORY: usize = 256;

fn word(d: &[u8], i: usize, be: bool) -> Option<i32> {
    let b: [u8; 4] = d.get(i * 4..i * 4 + 4)?.try_into().ok()?;
    Some(if be {
        i32::from_be_bytes(b)
    } else {
        i32::from_le_bytes(b)
    })
}

/// Word 0 is 0 and word 1 is 4, in either byte order.
pub fn probe(head: &[u8]) -> bool {
    head.len() >= 8
        && head[..4] == [0; 4]
        && (head[4..8] == [0, 0, 0, 4] || head[4..8] == [4, 0, 0, 0])
}

struct Directory {
    be: bool,
    sensor: i32,
    date: i32,
    time: i32,
    lines: u32,
    elements: u32,
    bytes: u32,
    bands: u32,
    prefix: u32,
    resolution: (i32, i32),
    memo: String,
    data: usize,
}

fn parse(d: &[u8]) -> Result<Directory> {
    if !probe(d) {
        return Err(Error::NotRecognised);
    }
    if d.len() < DIRECTORY {
        return Err(Error::Truncated("directory".into()));
    }
    let be = d[4..8] == [0, 0, 0, 4];
    let w = |i| word(d, i, be).unwrap_or(0);
    let pos = |i| u32::try_from(w(i)).map_err(|_| Error::Malformed(format!("word {i}")));
    let (lines, elements, bytes, bands, prefix) = (pos(8)?, pos(9)?, pos(10)?, pos(13)?, pos(14)?);
    check_dims(elements.into(), lines.into())?;
    if !matches!(bytes, 1 | 2 | 4) {
        return Err(Error::Unsupported(format!(
            "{bytes}-byte AREA elements are not supported (1, 2 and 4 are)"
        )));
    }
    if bands == 0 || bands > 64 {
        return Err(Error::Malformed(format!("{bands} bands")));
    }
    let memo: String = d[24 * 4..32 * 4]
        .iter()
        .map(|&c| {
            if (0x20..0x7F).contains(&c) {
                c as char
            } else {
                ' '
            }
        })
        .collect();
    Ok(Directory {
        be,
        sensor: w(2),
        date: w(3),
        time: w(4),
        lines,
        elements,
        bytes,
        bands,
        prefix,
        resolution: (w(11), w(12)),
        memo: memo.trim().to_owned(),
        data: pos(33)? as usize,
    })
}

fn line_bytes(a: &Directory) -> usize {
    a.prefix as usize + a.elements as usize * a.bytes as usize * a.bands as usize
}

pub fn info(data: &[u8], page: u32) -> Result<Info> {
    let a = parse(data)?;
    if page >= a.bands {
        return Err(Error::Malformed("no such band".into()));
    }
    let mut fields = vec![
        ("Sensor source".to_string(), a.sensor.to_string()),
        (
            "Date".into(),
            format!("{} day {}", 1900 + a.date / 1000, a.date % 1000),
        ),
        (
            "Time".into(),
            format!(
                "{:02}:{:02}:{:02} UTC",
                a.time / 10000,
                a.time / 100 % 100,
                a.time % 100
            ),
        ),
        ("Bytes per element".into(), a.bytes.to_string()),
        ("Bands".into(), a.bands.to_string()),
        (
            "Resolution".into(),
            format!("line {}, element {}", a.resolution.0, a.resolution.1),
        ),
    ];
    if !a.memo.is_empty() {
        fields.push(("Memo".into(), a.memo.clone()));
    }
    fields.push((
        "Byte order".into(),
        if a.be { "big endian" } else { "little endian" }.into(),
    ));
    Ok(Info {
        width: a.elements,
        height: a.lines,
        pages: a.bands,
        bit_depth: (a.bytes * 8).min(16) as u8,
        model: Model::Grey,
        has_alpha: false,
        orientation: 1,
        coding: if a.bytes == 1 {
            "8-bit counts".into()
        } else {
            format!("{}-bit values, stretched for display", a.bytes * 8)
        },
        summary: Vec::new(),
        fields,
    })
}

/// The stored values of band `band`, row by row, and whether lines were missing.
pub fn samples(data: &[u8], band: u32) -> Result<(u32, u32, Vec<u32>, bool)> {
    let a = parse(data)?;
    if band >= a.bands {
        return Err(Error::Malformed("no such band".into()));
    }
    let lb = line_bytes(&a);
    let (w, h, b) = (a.elements as usize, a.lines as usize, a.bytes as usize);
    let have = data.len().saturating_sub(a.data);
    check_payload(have, (lb * h) as u64, 1)?;
    let mut v = Vec::with_capacity(w * h);
    let mut short = false;
    for y in 0..h {
        let at = a.data + y * lb + a.prefix as usize;
        for x in 0..w {
            let o = at + (x * a.bands as usize + band as usize) * b;
            let Some(s) = data.get(o..o + b) else {
                short = true;
                v.push(0);
                continue;
            };
            v.push(match (b, a.be) {
                (1, _) => u32::from(s[0]),
                (2, true) => u32::from(u16::from_be_bytes([s[0], s[1]])),
                (2, false) => u32::from(u16::from_le_bytes([s[0], s[1]])),
                (_, true) => u32::from_be_bytes([s[0], s[1], s[2], s[3]]),
                (_, false) => u32::from_le_bytes([s[0], s[1], s[2], s[3]]),
            });
        }
    }
    Ok((a.elements, a.lines, v, short))
}

pub fn decode(data: &[u8], page: u32) -> Result<Image> {
    let a = parse(data)?;
    let (w, h, v, truncated) = samples(data, page)?;
    let pixels = if a.bytes == 1 {
        Pixels::Gray8(v.into_iter().map(|x| x as u8).collect())
    } else {
        let f: Vec<f64> = v
            .into_iter()
            .map(|x| {
                if a.bytes == 4 {
                    f64::from(x as i32)
                } else {
                    f64::from(x)
                }
            })
            .collect();
        Pixels::Gray16(stretch16(&f))
    };
    Ok(Image {
        width: w,
        height: h,
        pixels,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_and_prefix() {
        let mut d = vec![0u8; DIRECTORY];
        for (i, v) in [
            (1, 4i32),
            (8, 2),
            (9, 2),
            (10, 1),
            (13, 2),
            (14, 4),
            (33, 256),
        ] {
            d[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
        }
        // Two lines: a 4-byte prefix, then two elements of two bands.
        d.extend_from_slice(&[0, 0, 0, 0, 1, 2, 3, 4]);
        d.extend_from_slice(&[0, 0, 0, 0, 5, 6, 7, 8]);
        assert!(probe(&d));
        assert_eq!(info(&d, 0).unwrap().pages, 2);
        assert_eq!(
            decode(&d, 1).unwrap().pixels,
            Pixels::Gray8(vec![2, 4, 6, 8])
        );
    }
}
