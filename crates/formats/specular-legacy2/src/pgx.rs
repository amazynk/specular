//! PGX (`.pgx`), the raw component format of the JPEG 2000 conformance suite (ISO/IEC
//! 15444-4): one image component per file.
//!
//! A text line `PG ML` (big endian) or `PG LM` (little endian), the sign (`+`, `-` or
//! none), the bit depth (1–32), width and height, then a newline and the samples, row by
//! row, in 1, 2 or 4 bytes each. Signed samples are shown offset by half their range;
//! samples deeper than 16 bits are scaled to 16.

use crate::{Error, Image, Info, Model, Pixels, Result, check_dims, check_payload};

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"PG ML") || head.starts_with(b"PG LM")
}

pub struct Header {
    pub big_endian: bool,
    pub signed: bool,
    pub depth: u32,
    pub width: u32,
    pub height: u32,
    pub data: usize,
}

pub fn header(d: &[u8]) -> Result<Header> {
    if !probe(d) {
        return Err(Error::NotRecognised);
    }
    let n = d
        .iter()
        .take(256)
        .position(|&c| c == b'\n')
        .ok_or_else(|| Error::Truncated("header".into()))?;
    let line = String::from_utf8_lossy(&d[5..n]).replace('\r', "");
    let mut s = line.trim_start();
    let signed = match s.chars().next() {
        Some('-') => {
            s = &s[1..];
            true
        }
        Some('+') => {
            s = &s[1..];
            false
        }
        _ => false,
    };
    let v: Vec<u32> = s
        .split_whitespace()
        .map(|t| t.parse::<u32>())
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| Error::Malformed(format!("header {line:?}")))?;
    let [depth, width, height] = v[..] else {
        return Err(Error::Malformed(format!("header {line:?}")));
    };
    if !(1..=32).contains(&depth) {
        return Err(Error::Unsupported(format!("{depth}-bit PGX samples")));
    }
    check_dims(width.into(), height.into())?;
    Ok(Header {
        big_endian: d.starts_with(b"PG ML"),
        signed,
        depth,
        width,
        height,
        data: n + 1,
    })
}

fn bytes(depth: u32) -> usize {
    match depth {
        1..=8 => 1,
        9..=16 => 2,
        _ => 4,
    }
}

/// The samples as stored (sign-extended when signed).
pub fn samples(data: &[u8]) -> Result<(Header, Vec<i64>, bool)> {
    let h = header(data)?;
    let b = bytes(h.depth);
    let n = h.width as usize * h.height as usize;
    let src = &data[h.data.min(data.len())..];
    check_payload(src.len(), (n * b) as u64, 1)?;
    let mask = if h.depth == 32 {
        u32::MAX
    } else {
        (1u32 << h.depth) - 1
    };
    let mut v = Vec::with_capacity(n);
    for s in src.chunks_exact(b).take(n) {
        let raw = match (b, h.big_endian) {
            (1, _) => u32::from(s[0]),
            (2, true) => u32::from(u16::from_be_bytes([s[0], s[1]])),
            (2, false) => u32::from(u16::from_le_bytes([s[0], s[1]])),
            (_, true) => u32::from_be_bytes([s[0], s[1], s[2], s[3]]),
            (_, false) => u32::from_le_bytes([s[0], s[1], s[2], s[3]]),
        } & mask;
        v.push(
            if h.signed && h.depth < 64 && raw >> (h.depth - 1) & 1 == 1 {
                i64::from(raw) - (1i64 << h.depth)
            } else {
                i64::from(raw)
            },
        );
    }
    let short = v.len() < n;
    v.resize(n, 0);
    Ok((h, v, short))
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    let h = header(data)?;
    Ok(Info {
        width: h.width,
        height: h.height,
        pages: 1,
        bit_depth: h.depth.min(16) as u8,
        model: Model::Grey,
        has_alpha: false,
        orientation: 1,
        coding: format!(
            "{}-bit {}, {} endian",
            h.depth,
            if h.signed { "signed" } else { "unsigned" },
            if h.big_endian { "big" } else { "little" }
        ),
        summary: Vec::new(),
        fields: vec![("Bit depth".into(), h.depth.to_string())],
    })
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    let (h, v, truncated) = samples(data)?;
    let off = if h.signed { 1i64 << (h.depth - 1) } else { 0 };
    let max = (1i64 << h.depth) - 1;
    let unsigned = v.iter().map(|&s| (s + off).clamp(0, max) as u64);
    let pixels = if h.depth <= 8 {
        Pixels::Gray8(
            unsigned
                .map(|u| ((u * 255 + (max as u64) / 2) / max as u64) as u8)
                .collect(),
        )
    } else {
        Pixels::Gray16(
            unsigned
                .map(|u| ((u as u128 * 65535 + (max as u128) / 2) / max as u128) as u16)
                .collect(),
        )
    };
    Ok(Image {
        width: h.width,
        height: h.height,
        pixels,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_and_depths() {
        let mut d = b"PG ML +8 2 1\n".to_vec();
        d.extend_from_slice(&[0, 255]);
        assert_eq!(decode(&d, 0).unwrap().pixels, Pixels::Gray8(vec![0, 255]));
        let mut s = b"PG LM -12 2 1\n".to_vec();
        s.extend_from_slice(&0x0800u16.to_le_bytes());
        s.extend_from_slice(&0x07FFu16.to_le_bytes());
        let (_, v, _) = samples(&s).unwrap();
        assert_eq!(v, [-2048, 2047]);
        assert_eq!(
            decode(&s, 0).unwrap().pixels,
            Pixels::Gray16(vec![0, 65535])
        );
        let mut nosign = b"PG ML 4 1 1\n".to_vec();
        nosign.push(15);
        assert_eq!(decode(&nosign, 0).unwrap().pixels, Pixels::Gray8(vec![255]));
    }
}
