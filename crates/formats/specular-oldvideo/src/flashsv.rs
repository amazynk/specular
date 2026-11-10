//! Flash Screen Video 1 and 2 (Adobe SWF File Format Specification v19, chapter 14 and
//! appendix C): a grid of zlib-compressed blocks, laid out from the bottom left, each block's
//! rows bottom to top. Version 1 blocks hold B, G, R pixels; version 2 blocks may also hold the
//! 15/7-bit hybrid colours (a byte with the top bit clear indexes a 128-entry palette, else it
//! and the next byte give a 15-bit RGB colour), cover only some rows (a diff block), or be
//! compressed against the last full block at their place (zlib priming: a raw DEFLATE stream
//! with that block's data as the preset dictionary). An empty block keeps its pixels.

use crate::{Pixels, Reader, Result, bad};

/// The 128-entry palette version 2 falls back on (appendix C), 0xRRGGBB.
#[rustfmt::skip]
static DEFAULT_PALETTE: [u32; 128] = [
    0x000000, 0x333333, 0x666666, 0x999999, 0xCCCCCC, 0xFFFFFF, 0x330000, 0x660000,
    0x990000, 0xCC0000, 0xFF0000, 0x003300, 0x006600, 0x009900, 0x00CC00, 0x00FF00,
    0x000033, 0x000066, 0x000099, 0x0000CC, 0x0000FF, 0x333300, 0x666600, 0x999900,
    0xCCCC00, 0xFFFF00, 0x003333, 0x006666, 0x009999, 0x00CCCC, 0x00FFFF, 0x330033,
    0x660066, 0x990099, 0xCC00CC, 0xFF00FF, 0xFFFF33, 0xFFFF66, 0xFFFF99, 0xFFFFCC,
    0xFF33FF, 0xFF66FF, 0xFF99FF, 0xFFCCFF, 0x33FFFF, 0x66FFFF, 0x99FFFF, 0xCCFFFF,
    0xCCCC33, 0xCCCC66, 0xCCCC99, 0xCCCCFF, 0xCC33CC, 0xCC66CC, 0xCC99CC, 0xCCFFCC,
    0x33CCCC, 0x66CCCC, 0x99CCCC, 0xFFCCCC, 0x999933, 0x999966, 0x9999CC, 0x9999FF,
    0x993399, 0x996699, 0x99CC99, 0x99FF99, 0x339999, 0x669999, 0xCC9999, 0xFF9999,
    0x666633, 0x666699, 0x6666CC, 0x6666FF, 0x663366, 0x669966, 0x66CC66, 0x66FF66,
    0x336666, 0x996666, 0xCC6666, 0xFF6666, 0x333366, 0x333399, 0x3333CC, 0x3333FF,
    0x336633, 0x339933, 0x33CC33, 0x33FF33, 0x663333, 0x993333, 0xCC3333, 0xFF3333,
    0x003366, 0x336600, 0x660033, 0x006633, 0x330066, 0x663300, 0x336699, 0x669933,
    0x993366, 0x339966, 0x663399, 0x996633, 0x6699CC, 0x99CC66, 0xCC6699, 0x66CC99,
    0x9966CC, 0xCC9966, 0x99CCFF, 0xCCFF99, 0xFF99CC, 0x99FFCC, 0xCC99FF, 0xFFCC99,
    0x111111, 0x222222, 0x444444, 0x555555, 0xAAAAAA, 0xBBBBBB, 0xDDDDDD, 0xEEEEEE,
];

/// What a stream keeps between frames: its palette and each block's last full data (the
/// priming dictionary).
#[derive(Default)]
pub(crate) struct State {
    palette: Option<Vec<[u8; 3]>>,
    keys: Vec<Vec<u8>>,
}

const MAX_BLOCK: usize = 256 * 256 * 3;

fn inflate(data: &[u8], dict: Option<&[u8]>) -> Result<Vec<u8>> {
    let r = match dict {
        Some(d) => specular_png::inflate_raw_with_dictionary(data, d, MAX_BLOCK),
        None => specular_png::inflate_zlib(data, MAX_BLOCK),
    };
    r.map_err(|e| bad(format!("block data: {e}")))
}

/// Decodes one frame (`v2`: version 2) onto the R, G, B canvas of `w`×`h`.
pub(crate) fn decode(
    data: &[u8],
    w: usize,
    h: usize,
    v2: bool,
    st: &mut State,
    canvas: &mut Pixels,
) -> Result<()> {
    let Pixels::Rgb24(px) = canvas else {
        return Err(bad("canvas"));
    };
    if data.is_empty() {
        return Ok(());
    }
    let mut r = Reader::new(data);
    let a = r.u16be()?;
    let b = r.u16be()?;
    let bw = (usize::from(a >> 12) + 1) * 16;
    let bh = (usize::from(b >> 12) + 1) * 16;
    let (iw, ih) = (usize::from(a & 0xFFF), usize::from(b & 0xFFF));
    if iw != w || ih != h {
        return Err(crate::Error::Unsupported(format!(
            "Flash Screen Video changing its size to {iw}×{ih}"
        )));
    }
    let cols = w.div_ceil(bw);
    let rows = h.div_ceil(bh);
    if st.keys.len() != cols * rows {
        st.keys = vec![Vec::new(); cols * rows];
    }
    if v2 {
        let flags = r.u8()?;
        if flags & 2 != 0 {
            return Err(crate::Error::Unsupported(
                "Flash Screen Video 2 with an I-frame image".into(),
            ));
        }
        if flags & 1 != 0 {
            let n = usize::from(r.u16be()?);
            let raw = inflate(r.bytes(n)?, None)?;
            st.palette = Some(
                raw.chunks_exact(3)
                    .take(128)
                    .map(|c| [c[2], c[1], c[0]])
                    .collect(),
            );
        }
    }
    for row in 0..rows {
        for col in 0..cols {
            let n = usize::from(r.u16be()?);
            if n == 0 {
                continue;
            }
            let blk = r.bytes(n)?;
            let x0 = col * bw;
            let cw = bw.min(w - x0);
            // The block's bottom edge, counted in rows from the top.
            let bottom = h - row * bh;
            let ch = bh.min(bottom);
            let (mut first, mut count) = (0usize, ch);
            let mut fmt = 0u8;
            let mut at = 0;
            if v2 {
                fmt = blk[0];
                at = 1;
                if fmt & 4 != 0 {
                    let p = blk.get(1..3).ok_or_else(|| bad("diff position"))?;
                    (first, count) = (usize::from(p[0]), usize::from(p[1]));
                    at = 3;
                }
                if fmt & 2 != 0 {
                    return Err(crate::Error::Unsupported(
                        "Flash Screen Video 2 blocks primed from another block".into(),
                    ));
                }
            }
            let key = row * cols + col;
            let primed = v2 && fmt & 1 != 0;
            let body = blk.get(at..).ok_or_else(|| bad("block"))?;
            let raw = inflate(body, primed.then_some(st.keys[key].as_slice()))?;
            let hybrid = (fmt >> 3) & 3 == 2;
            let pal = st.palette.as_deref();
            let mut i = 0;
            let mut next = || -> Option<[u8; 3]> {
                if hybrid {
                    let b0 = *raw.get(i)?;
                    i += 1;
                    if b0 & 0x80 != 0 {
                        let b1 = *raw.get(i)?;
                        i += 1;
                        let c = u16::from(b0 & 0x7F) << 8 | u16::from(b1);
                        let five = |v: u16| ((v << 3) | (v >> 2)) as u8;
                        Some([five(c >> 10 & 31), five(c >> 5 & 31), five(c & 31)])
                    } else {
                        let k = usize::from(b0);
                        Some(match pal {
                            Some(p) => p.get(k).copied().unwrap_or_default(),
                            None => {
                                let v = DEFAULT_PALETTE[k];
                                [(v >> 16) as u8, (v >> 8) as u8, v as u8]
                            }
                        })
                    }
                } else {
                    let p = raw.get(i..i + 3)?;
                    i += 3;
                    Some([p[2], p[1], p[0]])
                }
            };
            'rows: for yy in first..(first + count).min(ch) {
                let y = bottom - 1 - yy;
                for x in x0..x0 + cw {
                    let Some(c) = next() else { break 'rows };
                    px[3 * (y * w + x)..3 * (y * w + x) + 3].copy_from_slice(&c);
                }
            }
            if !primed {
                st.keys[key] = raw;
            }
        }
    }
    Ok(())
}
