//! Pixar Image Computer pictures (`.pxr`, `.picio`).
//!
//! A 1024-byte header starting `80 E8 00 00`; little-endian words at byte 416 give the
//! height and width, at 420 the tile height and width, and at 424 the storage (14: 8 bits
//! per channel) and the channel layout (2: RGB). The pixels follow at byte 1024, row by row.
//! This is the layout every converter documents; other storage and channel codes are
//! refused by name, as is a picture cut into tiles smaller than itself.

use crate::{Error, Image, Info, Model, Pixels, Result, check_dims, check_payload, u16le};

const DATA: usize = 1024;

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(&[0x80, 0xE8, 0, 0])
}

struct Header {
    width: u32,
    height: u32,
    tile: (u32, u32),
}

fn header(d: &[u8]) -> Result<Header> {
    if !probe(d) {
        return Err(Error::NotRecognised);
    }
    let w = |o| u32::from(u16le(d, o).unwrap_or(0));
    if d.len() < 428 {
        return Err(Error::Truncated("header".into()));
    }
    let (height, width) = (w(416), w(418));
    let tile = (w(420), w(422));
    let (storage, layout) = (w(424), w(426));
    if (storage, layout) != (14, 2) {
        return Err(Error::Unsupported(format!(
            "Pixar pictures with storage {storage} and channel layout {layout} are not \
             supported (8-bit RGB is)"
        )));
    }
    check_dims(width.into(), height.into())?;
    if tile.0 != 0 && tile.1 != 0 && (tile.0 < height || tile.1 < width) {
        return Err(Error::Unsupported(
            "tiled Pixar pictures are not supported".into(),
        ));
    }
    Ok(Header {
        width,
        height,
        tile,
    })
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    let h = header(data)?;
    let mut fields = Vec::new();
    if h.tile.0 > 0 {
        fields.push((
            "Tile size".to_string(),
            format!("{}×{}", h.tile.1, h.tile.0),
        ));
    }
    Ok(Info {
        width: h.width,
        height: h.height,
        pages: 1,
        bit_depth: 8,
        model: Model::Rgb,
        has_alpha: false,
        orientation: 1,
        coding: "8-bit RGB".into(),
        summary: Vec::new(),
        fields,
    })
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    let h = header(data)?;
    let n = h.width as usize * h.height as usize;
    let src = data.get(DATA..).unwrap_or(&[]);
    check_payload(src.len(), (n * 3) as u64, 1)?;
    let mut out = Vec::with_capacity(n * 4);
    for p in src.chunks_exact(3).take(n) {
        out.extend_from_slice(&[p[0], p[1], p[2], 255]);
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
    fn rgb_and_refusals() {
        let mut d = vec![0u8; DATA];
        d[..4].copy_from_slice(&[0x80, 0xE8, 0, 0]);
        for (o, v) in [
            (416, 1u16),
            (418, 2),
            (420, 1),
            (422, 2),
            (424, 14),
            (426, 2),
        ] {
            d[o..o + 2].copy_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let img = decode(&d, 0).unwrap();
        assert_eq!(img.pixels, Pixels::Rgba8(vec![1, 2, 3, 255, 4, 5, 6, 255]));
        d[426] = 1;
        assert!(matches!(info(&d, 0), Err(Error::Unsupported(_))));
    }
}
