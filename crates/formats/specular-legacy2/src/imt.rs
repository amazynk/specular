//! IM Tools images (`.imt`).
//!
//! A text header of `key value` lines (`width`, `height`, `pixel n8`; lines starting `*` are
//! comments), then a form feed (0x0C) and the pixels, row by row. Only 8-bit grey (`n8`)
//! exists in the files found in the wild; other pixel kinds are refused by name.

use crate::{Error, Image, Info, Model, Pixels, Result, check_dims, check_payload};

struct Header {
    width: u32,
    height: u32,
    data: usize,
}

fn parse(d: &[u8]) -> Result<Header> {
    let end = d
        .iter()
        .take(4096)
        .position(|&c| c == 0x0C)
        .ok_or(Error::NotRecognised)?;
    let text = std::str::from_utf8(&d[..end]).map_err(|_| Error::NotRecognised)?;
    let (mut w, mut h, mut pixel) = (None, None, None);
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('*') {
            continue;
        }
        let mut it = line.splitn(2, ' ');
        let (k, v) = (it.next().unwrap_or(""), it.next().unwrap_or("").trim());
        match k {
            "width" => w = v.parse::<u32>().ok(),
            "height" => h = v.parse::<u32>().ok(),
            "pixel" => pixel = Some(v.to_owned()),
            _ if k.bytes().all(|c| c.is_ascii_lowercase()) => {}
            _ => return Err(Error::NotRecognised),
        }
    }
    let (Some(width), Some(height)) = (w, h) else {
        return Err(Error::NotRecognised);
    };
    match pixel.as_deref() {
        Some("n8") => {}
        Some(p) => {
            return Err(Error::Unsupported(format!(
                "IM Tools pixel kind {p:?} is not supported (n8 is)"
            )));
        }
        None => return Err(Error::Malformed("no pixel line".into())),
    }
    check_dims(width.into(), height.into())?;
    Ok(Header {
        width,
        height,
        data: end + 1,
    })
}

/// A header with `width` and `height` lines before a form feed.
pub fn probe(head: &[u8]) -> bool {
    let end = head.iter().position(|&c| c == 0x0C).unwrap_or(head.len());
    let t = &head[..end];
    let has = |k: &[u8]| t.windows(k.len()).any(|w| w == k);
    (t.starts_with(b"width ") || t.starts_with(b"*") || t.starts_with(b"height "))
        && has(b"width ")
        && has(b"height ")
        && has(b"pixel ")
        && t.iter()
            .all(|&c| c == b'\n' || c == b'\r' || (0x20..0x7F).contains(&c))
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    let h = parse(data)?;
    Ok(Info {
        width: h.width,
        height: h.height,
        pages: 1,
        bit_depth: 8,
        model: Model::Grey,
        has_alpha: false,
        orientation: 1,
        coding: "8-bit grey".into(),
        summary: Vec::new(),
        fields: Vec::new(),
    })
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    let h = parse(data)?;
    let n = h.width as usize * h.height as usize;
    let src = &data[h.data.min(data.len())..];
    check_payload(src.len(), n as u64, 1)?;
    let mut v = src[..src.len().min(n)].to_vec();
    let truncated = v.len() < n;
    v.resize(n, crate::GREY);
    Ok(Image {
        width: h.width,
        height: h.height,
        pixels: Pixels::Gray8(v),
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_pixels() {
        let mut d = b"*comment\nwidth 3\nheight 1\npixel n8\n\x0c".to_vec();
        d.extend_from_slice(&[1, 2, 3]);
        assert!(probe(&d));
        assert_eq!(decode(&d, 0).unwrap().pixels, Pixels::Gray8(vec![1, 2, 3]));
        let bad = b"width 3\nheight 1\npixel n16\n\x0c\x00";
        assert!(matches!(info(bad, 0), Err(Error::Unsupported(_))));
    }
}
