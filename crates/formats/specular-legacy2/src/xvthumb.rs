//! XV thumbnails (`P7 332`), the previews the XV viewer keeps in `.xvpics` folders.
//!
//! The line `P7 332`, comment lines starting `#` (XV writes `#IMGINFO:` with the original's
//! size and format, and `#END_OF_COMMENTS`), a line with width, height and 255, then one
//! byte per pixel: three bits of red, three of green, two of blue (`RRRGGGBB`).

use crate::{Error, Image, Info, Model, Pixels, Result, check_dims, check_payload};

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"P7 332")
}

struct Header {
    width: u32,
    height: u32,
    data: usize,
    original: Option<String>,
}

fn parse(d: &[u8]) -> Result<Header> {
    if !probe(d) {
        return Err(Error::NotRecognised);
    }
    let mut at = 0;
    let mut original = None;
    let mut lines = 0;
    loop {
        let rest = d.get(at..).unwrap_or(&[]);
        let n = rest
            .iter()
            .take(4096)
            .position(|&c| c == b'\n')
            .ok_or_else(|| Error::Truncated("header".into()))?;
        let line = String::from_utf8_lossy(&rest[..n]).trim().to_owned();
        at += n + 1;
        lines += 1;
        if lines > 256 {
            return Err(Error::Malformed("header too long".into()));
        }
        if lines == 1 || line.is_empty() {
            continue;
        }
        if let Some(c) = line.strip_prefix('#') {
            if let Some(i) = c.strip_prefix("IMGINFO:") {
                original = Some(crate::swatch::short(i));
            }
            continue;
        }
        let v: Vec<u32> = line
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if v.len() < 2 {
            return Err(Error::Malformed(format!("size line {line:?}")));
        }
        check_dims(v[0].into(), v[1].into())?;
        return Ok(Header {
            width: v[0],
            height: v[1],
            data: at,
            original,
        });
    }
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    let h = parse(data)?;
    let mut fields = Vec::new();
    if let Some(o) = h.original {
        fields.push(("Original".to_string(), o));
    }
    Ok(Info {
        width: h.width,
        height: h.height,
        pages: 1,
        bit_depth: 8,
        model: Model::Rgb,
        has_alpha: false,
        orientation: 1,
        coding: "3:3:2 colour".into(),
        summary: Vec::new(),
        fields,
    })
}

/// The colour of a `RRRGGGBB` byte.
pub fn colour(b: u8) -> [u8; 3] {
    let (r, g, bl) = (u32::from(b >> 5), u32::from((b >> 2) & 7), u32::from(b & 3));
    [
        (r * 255 / 7) as u8,
        (g * 255 / 7) as u8,
        (bl * 255 / 3) as u8,
    ]
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    let h = parse(data)?;
    let n = h.width as usize * h.height as usize;
    let src = &data[h.data.min(data.len())..];
    check_payload(src.len(), n as u64, 1)?;
    let mut out = Vec::with_capacity(n * 4);
    for &b in src.iter().take(n) {
        let [r, g, bl] = colour(b);
        out.extend_from_slice(&[r, g, bl, 255]);
    }
    let truncated = out.len() < n * 4;
    out.resize(n * 4, crate::GREY);
    Ok(Image {
        width: h.width,
        height: h.height,
        pixels: Pixels::Rgba8(out),
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_colours() {
        let mut d = b"P7 332\n#IMGINFO:640x480 JPEG\n#END_OF_COMMENTS\n2 1 255\n".to_vec();
        d.extend_from_slice(&[0xFF, 0x1C]);
        let i = info(&d, 0).unwrap();
        assert_eq!(i.fields[0].1, "640x480 JPEG");
        let img = decode(&d, 0).unwrap();
        assert_eq!(
            img.pixels,
            Pixels::Rgba8(vec![255, 255, 255, 255, 0, 255, 0, 255])
        );
    }
}
