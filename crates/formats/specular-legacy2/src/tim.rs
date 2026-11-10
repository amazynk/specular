//! PlayStation TIM textures.
//!
//! Layout (little-endian): the id `10 00 00 00`, a flags word (bits 0–2 the pixel mode:
//! 0 = 4-bit CLUT, 1 = 8-bit CLUT, 2 = 16-bit direct, 3 = 24-bit direct, 4 = mixed; bit 3 a
//! CLUT block follows), then the optional CLUT block and the image block. Each block is a
//! length (including its 12-byte header), the frame-buffer position (x, y), the size in 16-bit
//! frame-buffer units (width, height) and the data.
//!
//! 16-bit colours are `0BBBBBGGGGGRRRRR` with bit 15 the semi-transparency (STP) flag; 5-bit
//! values widen to 8 bits as `(v << 3) | (v >> 2)`. Transparency follows the hardware's rule
//! for textures: the colour 0x0000 (black with STP clear) is not drawn, so it is decoded as
//! transparent; every other colour is opaque (the STP flag only selects blending, which has no
//! meaning outside the console). Files with several CLUTs show the first; the count is in the
//! info. Mixed-mode images (a frame-buffer dump without a single pixel format) are shown as
//! the frame buffer's 16-bit colours, as a VRAM viewer shows them.

use crate::{Error, Image, Info, Model, Pixels, Result, check_dims, check_payload, u16le, u32le};

/// The 8-byte signature and a flags word with a known pixel mode and no unknown bits.
pub fn probe(head: &[u8]) -> bool {
    head.len() >= 8
        && head[..4] == [0x10, 0, 0, 0]
        && u32le(head, 4).is_some_and(|f| f & !0xF == 0 && f & 7 <= 4)
}

struct Block<'a> {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    data: &'a [u8],
    /// The block's data is shorter than its header says.
    short: bool,
}

fn block<'a>(data: &'a [u8], at: usize, what: &str) -> Result<(Block<'a>, usize)> {
    let len = u32le(data, at).ok_or_else(|| Error::Truncated(format!("no {what} block")))? as usize;
    let (x, y, w, h) = (
        u16le(data, at + 4),
        u16le(data, at + 6),
        u16le(data, at + 8),
        u16le(data, at + 10),
    );
    let (Some(x), Some(y), Some(w), Some(h)) = (x, y, w, h) else {
        return Err(Error::Truncated(format!("{what} block header")));
    };
    if len < 12 {
        return Err(Error::Malformed(format!("{what} block length {len}")));
    }
    let start = at + 12;
    let want = (w as usize * h as usize * 2).max(len - 12);
    let end = start.saturating_add(want).min(data.len());
    let d = data.get(start..end).unwrap_or(&[]);
    Ok((
        Block {
            x,
            y,
            w,
            h,
            data: d,
            short: d.len() < w as usize * h as usize * 2,
        },
        at.saturating_add(len),
    ))
}

struct Parsed<'a> {
    mode: u32,
    clut: Option<Block<'a>>,
    image: Block<'a>,
    width: u32,
}

fn parse(data: &[u8]) -> Result<Parsed<'_>> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let flags = u32le(data, 4).unwrap_or(0);
    let mode = flags & 7;
    let mut at = 8;
    let clut = if flags & 8 != 0 {
        let (b, next) = block(data, at, "colour table")?;
        at = next;
        Some(b)
    } else {
        None
    };
    let (image, _) = block(data, at, "image")?;
    let width = match mode {
        0 => image.w as u32 * 4,
        1 => image.w as u32 * 2,
        // Mixed: the frame buffer's own 16-bit words.
        2 | 4 => image.w as u32,
        _ => image.w as u32 * 2 / 3,
    };
    check_dims(width as u64, image.h as u64)?;
    if mode < 2 && clut.as_ref().is_none_or(|c| c.w == 0 || c.h == 0) {
        return Err(Error::Malformed(
            "a 4- or 8-bit TIM image without a colour table".into(),
        ));
    }
    Ok(Parsed {
        mode,
        clut,
        image,
        width,
    })
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    let p = parse(data)?;
    let depth = [4u8, 8, 16, 24, 16][p.mode as usize];
    let mut fields = vec![
        (
            "Pixel mode".to_string(),
            if p.mode == 4 {
                "mixed".to_string()
            } else {
                format!("{depth}-bit")
            },
        ),
        (
            "Image position".into(),
            format!("{}, {}", p.image.x, p.image.y),
        ),
    ];
    let mut coding = format!("{depth}-bit");
    if let Some(c) = &p.clut {
        fields.push(("CLUT position".into(), format!("{}, {}", c.x, c.y)));
        fields.push(("CLUT colours".into(), c.w.to_string()));
        fields.push(("CLUTs".into(), c.h.to_string()));
        if p.mode < 2 {
            coding += &if c.h > 1 {
                format!(" palette, first of {} colour tables", c.h)
            } else {
                " palette".to_string()
            };
        }
    } else if p.mode == 2 {
        coding += " 5:5:5 RGB";
    } else {
        coding += " RGB";
    }
    if p.mode == 4 {
        coding = "mixed, shown as 16-bit frame-buffer colours".to_string();
    }
    let has_alpha = match p.mode {
        0 | 1 => p.clut.as_ref().is_some_and(|c| {
            c.data
                .chunks_exact(2)
                .take(c.w as usize)
                .any(|v| v == [0, 0])
        }),
        2 | 4 => true,
        _ => false,
    };
    Ok(Info {
        width: p.width,
        height: p.image.h as u32,
        pages: 1,
        bit_depth: if matches!(p.mode, 2 | 4) {
            5
        } else {
            depth.min(8)
        },
        model: if p.mode < 2 {
            Model::Palette
        } else {
            Model::Rgb
        },
        has_alpha,
        orientation: 1,
        coding,
        summary: Vec::new(),
        fields,
    })
}

#[inline]
fn c5(v: u16) -> u8 {
    let v = (v & 31) as u8;
    (v << 3) | (v >> 2)
}

#[inline]
fn colour(v: u16) -> [u8; 4] {
    [c5(v), c5(v >> 5), c5(v >> 10), if v == 0 { 0 } else { 255 }]
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    let p = parse(data)?;
    let (w, h) = (p.width as usize, p.image.h as usize);
    let stride = p.image.w as usize * 2;
    check_payload(p.image.data.len(), (stride * h) as u64, 1)?;
    let mut out = vec![0u8; w * h * 4];
    // Missing pixels: opaque grey.
    for px in out.chunks_exact_mut(4) {
        px.copy_from_slice(&[crate::GREY, crate::GREY, crate::GREY, 255]);
    }
    let pal: Vec<[u8; 4]> = p
        .clut
        .as_ref()
        .map(|c| {
            c.data
                .chunks_exact(2)
                .take(c.w as usize)
                .map(|v| colour(u16::from_le_bytes([v[0], v[1]])))
                .collect()
        })
        .unwrap_or_default();
    let look = |i: usize| pal.get(i).copied().unwrap_or([0, 0, 0, 255]);
    let rows = p.image.data.len() / stride.max(1);
    for (y, row) in p.image.data.chunks_exact(stride).take(h).enumerate() {
        let o = &mut out[y * w * 4..(y + 1) * w * 4];
        match p.mode {
            0 => {
                for (x, px) in o.chunks_exact_mut(4).enumerate() {
                    let b = row[x / 2];
                    let i = if x % 2 == 0 { b & 15 } else { b >> 4 };
                    px.copy_from_slice(&look(i as usize));
                }
            }
            1 => {
                for (px, &i) in o.chunks_exact_mut(4).zip(row) {
                    px.copy_from_slice(&look(i as usize));
                }
            }
            2 | 4 => {
                for (px, v) in o.chunks_exact_mut(4).zip(row.chunks_exact(2)) {
                    px.copy_from_slice(&colour(u16::from_le_bytes([v[0], v[1]])));
                }
            }
            _ => {
                for (px, v) in o.chunks_exact_mut(4).zip(row.chunks_exact(3)) {
                    px.copy_from_slice(&[v[0], v[1], v[2], 255]);
                }
            }
        }
    }
    Ok(Image {
        width: p.width,
        height: h as u32,
        pixels: Pixels::Rgba8(out),
        truncated: p.image.short || rows < h,
    })
}
