//! MSX screens saved with BASIC's `BSAVE "…",S` (video memory): a 7-byte header (`&FE`,
//! start, end and run addresses) and the VRAM from the start address. The extension names
//! the screen mode: `.sc2` (screen 2, 256×192 patterns), `.sc5` (256×212, 16 colours),
//! `.sc6` (512×212, 4 colours), `.sc7` (512×212, 16 colours), `.sc8` (256×212, 256 fixed
//! colours), `.sca` (screen 10: YJK and 16 palette colours) and `.scc` (screen 12: YJK).
//!
//! Screens 5–7 and 10 use the palette saved with them when the dump reaches it (&7680 or
//! &FA80), else the MSX2 default. Dumps without the header are read when they are exactly
//! the screen's size.

use crate::{Canvas, Error, Hints, Image, Info, Result, at, field};

pub const EXTENSIONS: &[&str] = &["sc2", "sc5", "sc6", "sc7", "sc8", "sca", "scc"];

/// A screen mode's layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mode {
    /// BASIC's screen number.
    pub screen: u8,
    pub width: u32,
    pub height: u32,
    /// Bitmap bytes (screen 2: up to the end of its colour table).
    pub bytes: usize,
    /// VRAM address of the palette, if the mode has one.
    pub palette_at: Option<usize>,
}

pub fn mode_of_ext(ext: &str) -> Option<Mode> {
    let m = |screen, width, height, bytes, palette_at| Mode {
        screen,
        width,
        height,
        bytes,
        palette_at,
    };
    Some(match ext {
        "sc2" => m(2, 256, 192, 0x3800, None),
        "sc5" => m(5, 256, 212, 0x6A00, Some(0x7680)),
        "sc6" => m(6, 512, 212, 0x6A00, Some(0x7680)),
        "sc7" => m(7, 512, 212, 0xD400, Some(0xFA80)),
        "sc8" => m(8, 256, 212, 0xD400, None),
        "sca" => m(10, 256, 212, 0xD400, Some(0xFA80)),
        "scc" => m(12, 256, 212, 0xD400, None),
        _ => return None,
    })
}

/// The BSAVE header of `data`: (start, end, run) addresses.
pub fn bsave(data: &[u8]) -> Option<(u16, u16, u16)> {
    let h = data.get(..7)?;
    let word = |i: usize| u16::from_le_bytes([h[i], h[i + 1]]);
    let (start, end, run) = (word(1), word(3), word(5));
    // Screen saves start at VRAM 0 and are at least a screen 2 pattern table long.
    (h[0] == 0xFE && start == 0 && end >= 0x17FF && run <= end).then_some((start, end, run))
}

/// VRAM as saved (index = address), and whether it came with a BSAVE header.
fn vram<'a>(data: &'a [u8], mode: &Mode) -> Result<(&'a [u8], Option<u16>)> {
    if let Some((_, end, _)) = bsave(data) {
        let want = end as usize + 1;
        let body = &data[7..];
        return Ok((&body[..body.len().min(want)], Some(end)));
    }
    if data.len() == mode.bytes
        || mode.palette_at.is_some_and(|p| data.len() == p + 32)
        || (mode.screen == 2 && data.len() == 0x4000)
    {
        return Ok((data, None));
    }
    if data.starts_with(b"Scenarist_SCC") {
        return Err(Error::Unsupported(
            "this is a Scenarist closed-caption file, not an MSX screen".into(),
        ));
    }
    if data.first() == Some(&0xFE) {
        return Err(Error::Unsupported(
            "this MSX BSAVE file does not hold a screen: it does not start at video memory address 0"
                .into(),
        ));
    }
    Err(Error::Unsupported(
        "only BSAVE screen dumps are read: compressed MSX picture formats are not".into(),
    ))
}

fn mode(hints: &Hints) -> Result<Mode> {
    mode_of_ext(hints.ext).ok_or_else(|| {
        Error::Unsupported(
            "the MSX screen mode is unknown: name the file .sc2, .sc5, .sc6, .sc7, .sc8, .sca or .scc"
                .into(),
        )
    })
}

/// Output pixel repeats: 512-wide modes are drawn on doubled lines.
fn scale(m: &Mode) -> (u32, u32) {
    if m.width == 512 { (1, 2) } else { (1, 1) }
}

pub fn info(data: &[u8], hints: &Hints) -> Result<Info> {
    let m = mode(hints)?;
    let (v, header) = vram(data, &m)?;
    let (sx, sy) = scale(&m);
    let pal_saved = m.palette_at.is_some_and(|p| v.len() >= p + 32);
    let (bits, palette, coding) = match m.screen {
        2 => (1, true, "pattern graphics, 16 colours".to_owned()),
        5 | 7 => (4, true, "16 of 512 colours".to_owned()),
        6 => (2, true, "4 of 512 colours".to_owned()),
        8 => (8, false, "256 fixed colours (GRB 3-3-2)".to_owned()),
        10 => (8, false, "YJK with 16 palette colours".to_owned()),
        _ => (8, false, "YJK, 19 268 colours".to_owned()),
    };
    let mut fields = vec![
        field("Machine", if m.screen == 2 { "MSX" } else { "MSX2" }),
        field(
            "Screen mode",
            format!("{} ({}×{})", m.screen, m.width, m.height),
        ),
    ];
    if m.palette_at.is_some() {
        fields.push(field(
            "Palette",
            if pal_saved {
                "saved with the screen"
            } else {
                "MSX2 default"
            },
        ));
    }
    if let Some(end) = header {
        fields.push(field("Header", format!("BSAVE, video memory 0–&{end:04X}")));
    }
    Ok(Info {
        width: m.width * sx,
        height: m.height * sy,
        pages: 1,
        bit_depth: bits,
        palette,
        orientation: 1,
        coding: format!("screen {}, {coding}", m.screen),
        fields,
    })
}

/// The TMS9918 colours of screen 2 (colour 0 is transparent: shown black).
const TMS9918: [[u8; 3]; 16] = [
    [0, 0, 0],
    [0, 0, 0],
    [33, 200, 66],
    [94, 220, 120],
    [84, 85, 237],
    [125, 118, 252],
    [212, 82, 77],
    [66, 235, 245],
    [252, 85, 84],
    [255, 121, 120],
    [212, 193, 84],
    [230, 206, 128],
    [33, 176, 59],
    [201, 91, 186],
    [204, 204, 204],
    [255, 255, 255],
];

/// The MSX2 start-up palette, 3 bits per channel (R, G, B).
const MSX2_DEFAULT: [[u8; 3]; 16] = [
    [0, 0, 0],
    [0, 0, 0],
    [1, 6, 1],
    [3, 7, 3],
    [1, 1, 7],
    [2, 3, 7],
    [5, 1, 1],
    [2, 6, 7],
    [7, 1, 1],
    [7, 3, 3],
    [6, 6, 1],
    [6, 6, 4],
    [1, 4, 1],
    [6, 2, 5],
    [5, 5, 5],
    [7, 7, 7],
];

fn s3(v: u8) -> u8 {
    ((v.min(7) as u32 * 255 + 3) / 7) as u8
}

/// 16 colours from a saved palette table (`0RRR0BBB`, `00000GGG` per entry) or the default.
fn palette(v: &[u8], m: &Mode) -> [[u8; 3]; 16] {
    let mut p = MSX2_DEFAULT.map(|c| c.map(s3));
    if let Some(at_) = m.palette_at
        && let Some(t) = v.get(at_..at_ + 32)
    {
        for (i, c) in p.iter_mut().enumerate() {
            let (a, b) = (t[i * 2], t[i * 2 + 1]);
            *c = [s3((a >> 4) & 7), s3(b & 7), s3(a & 7)];
        }
    }
    p
}

/// RGB of a screen 10/12 YJK group pixel (5-bit Y, signed 6-bit J and K).
pub fn yjk(y: i32, j: i32, k: i32) -> [u8; 3] {
    let c = |v: i32| ((v.clamp(0, 31) * 255 + 15) / 31) as u8;
    [c(y + j), c(y + k), c((5 * y - 2 * j - k + 2) / 4)]
}

fn signed6(v: u8) -> i32 {
    let v = (v & 63) as i32;
    if v >= 32 { v - 64 } else { v }
}

pub fn decode(data: &[u8], hints: &Hints) -> Result<Image> {
    let m = mode(hints)?;
    let (v, _) = vram(data, &m)?;
    let truncated = v.len() < m.bytes;
    let (sx, sy) = scale(&m);
    let mut c = Canvas::new(m.width, m.height, sx, sy);
    match m.screen {
        2 => {
            // Without its name table the dump is shown with the BASIC default (0–255 in
            // each third).
            let names = v.len() >= 0x1B00;
            for cy in 0..24u32 {
                let bank = (cy / 8) as usize * 2048;
                for cx in 0..32u32 {
                    let n = if names {
                        at(v, 0x1800 + (cy * 32 + cx) as usize) as usize
                    } else {
                        ((cy % 8) * 32 + cx) as usize
                    };
                    for row in 0..8u32 {
                        let pat = at(v, bank + n * 8 + row as usize);
                        let col = at(v, 0x2000 + bank + n * 8 + row as usize);
                        let (fg, bg) = (TMS9918[(col >> 4) as usize], TMS9918[(col & 15) as usize]);
                        for b in 0..8u32 {
                            let on = pat & (0x80 >> b) != 0;
                            c.put(cx * 8 + b, cy * 8 + row, if on { fg } else { bg });
                        }
                    }
                }
            }
        }
        5..=7 => {
            let pal = palette(v, &m);
            let bits = if m.screen == 6 { 2 } else { 4 };
            let per = 8 / bits;
            let line = (m.width / per) as usize;
            for y in 0..m.height {
                for xb in 0..line as u32 {
                    let b = at(v, y as usize * line + xb as usize);
                    for i in 0..per {
                        let shift = 8 - bits * (i + 1);
                        let p = (b >> shift) & ((1 << bits) - 1);
                        c.put(xb * per + i, y, pal[p as usize]);
                    }
                }
            }
        }
        8 => {
            for y in 0..m.height {
                for x in 0..m.width {
                    let b = at(v, (y * 256 + x) as usize);
                    let (g, r, bl) = (b >> 5, (b >> 2) & 7, b & 3);
                    c.put(x, y, [s3(r), s3(g), ((bl as u32 * 255) / 3) as u8]);
                }
            }
        }
        _ => {
            let pal = palette(v, &m);
            for y in 0..m.height {
                for gx in 0..64u32 {
                    let i = (y * 256 + gx * 4) as usize;
                    let b = [at(v, i), at(v, i + 1), at(v, i + 2), at(v, i + 3)];
                    let k = signed6((b[0] & 7) | (b[1] & 7) << 3);
                    let j = signed6((b[2] & 7) | (b[3] & 7) << 3);
                    for (n, &px) in b.iter().enumerate() {
                        let rgb = if m.screen == 10 && px & 8 != 0 {
                            // YAE: the pixel is palette colour Y/2.
                            pal[(px >> 4) as usize]
                        } else if m.screen == 10 {
                            yjk((px >> 4) as i32 * 2, j, k)
                        } else {
                            yjk((px >> 3) as i32, j, k)
                        };
                        c.put(gx * 4 + n as u32, y, rgb);
                    }
                }
            }
        }
    }
    Ok(c.into_image(truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yjk_grey_and_header() {
        assert_eq!(yjk(31, 0, 0), [255, 255, 255]);
        assert_eq!(yjk(0, 0, 0), [0, 0, 0]);
        let mut d = vec![0xFE, 0, 0, 0xFF, 0x69, 0, 0];
        d.resize(7 + 0x6A00, 0);
        assert_eq!(bsave(&d), Some((0, 0x69FF, 0)));
        assert_eq!(signed6(63), -1);
    }
}
