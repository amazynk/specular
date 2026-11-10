//! BBC Micro screen memory dumps. The extension names the mode: `.bb0`, `.bb1`, `.bb2`
//! (20 KiB: 640×256 in 2 colours, 320×256 in 4, 160×256 in 8) and `.bb4`, `.bb5` (10 KiB:
//! 320×256 in 2, 160×256 in 4).
//!
//! Memory is laid out in character cells: 8 bytes are the 8 lines of one cell, cells run
//! left to right, then the next character row. Pixels of a byte interleave their bits
//! (mode 1: bits 7 and 3 are the left pixel). Colours are the default logical palette of
//! each mode (flashing colours shown in their first phase). Every mode is drawn 640×512.

use crate::{Canvas, Error, Hints, Image, Info, Result, at, field};

/// The eight physical colours.
const PHYSICAL: [[u8; 3]; 8] = [
    [0, 0, 0],
    [255, 0, 0],
    [0, 255, 0],
    [255, 255, 0],
    [0, 0, 255],
    [255, 0, 255],
    [0, 255, 255],
    [255, 255, 255],
];

fn mode(hints: &Hints) -> Result<u8> {
    match hints.ext {
        "bb0" => Ok(0),
        "bb1" => Ok(1),
        "bb2" => Ok(2),
        "bb4" => Ok(4),
        "bb5" => Ok(5),
        _ => Err(Error::Unsupported(
            "the BBC Micro screen mode is unknown: name the file .bb0, .bb1, .bb2, .bb4 or .bb5"
                .into(),
        )),
    }
}

/// (pixels across, bits per pixel, bytes).
fn layout(mode: u8) -> (u32, u32, usize) {
    match mode {
        0 => (640, 1, 20480),
        1 => (320, 2, 20480),
        2 => (160, 4, 20480),
        4 => (320, 1, 10240),
        _ => (160, 2, 10240),
    }
}

fn check(data: &[u8], mode: u8) -> Result<()> {
    let (_, _, bytes) = layout(mode);
    if data.len() == bytes {
        return Ok(());
    }
    Err(Error::Unsupported(format!(
        "a BBC Micro mode {mode} screen is {} KB; this file is {} bytes",
        bytes / 1024,
        data.len()
    )))
}

pub fn info(data: &[u8], hints: &Hints) -> Result<Info> {
    let m = mode(hints)?;
    check(data, m)?;
    let (w, bpp, _) = layout(m);
    Ok(Info {
        width: 640,
        height: 512,
        pages: 1,
        bit_depth: bpp as u8,
        palette: true,
        orientation: 1,
        coding: format!("mode {m}, {}×256 in {} colours", w, 1 << bpp),
        fields: vec![
            field("Machine", "BBC Micro"),
            field(
                "Screen mode",
                format!("{m} ({w}×256, {} colours)", 1 << bpp),
            ),
            field("Palette", "default"),
        ],
    })
}

/// The logical colour of pixel `i` of byte `b` at `bpp` bits per pixel.
pub fn logical(b: u8, bpp: u32, i: u32) -> u8 {
    let bit = |n: u32| (b >> n) & 1;
    match bpp {
        1 => bit(7 - i),
        2 => bit(7 - i) << 1 | bit(3 - i),
        _ => bit(7 - i) << 3 | bit(5 - i) << 2 | bit(3 - i) << 1 | bit(1 - i),
    }
}

pub fn decode(data: &[u8], hints: &Hints) -> Result<Image> {
    let m = mode(hints)?;
    check(data, m)?;
    let (w, bpp, bytes) = layout(m);
    let pal: Vec<[u8; 3]> = match bpp {
        1 => vec![PHYSICAL[0], PHYSICAL[7]],
        2 => vec![PHYSICAL[0], PHYSICAL[1], PHYSICAL[3], PHYSICAL[7]],
        _ => (0..16).map(|i| PHYSICAL[i % 8]).collect(),
    };
    let per = 8 / bpp;
    let row_bytes = bytes / 32;
    let mut c = Canvas::new(w, 256, 640 / w, 2);
    for y in 0..256u32 {
        let base = (y / 8) as usize * row_bytes + (y % 8) as usize;
        for cell in 0..(row_bytes / 8) as u32 {
            let b = at(data, base + cell as usize * 8);
            for i in 0..per {
                c.put(cell * per + i, y, pal[logical(b, bpp, i) as usize]);
            }
        }
    }
    Ok(c.into_image(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits() {
        assert_eq!(logical(0b1000_1000, 2, 0), 3);
        assert_eq!(logical(0b0000_0001, 2, 3), 1);
        assert_eq!(logical(0b1010_1010, 4, 0), 15);
        assert_eq!(logical(0b0101_0101, 4, 1), 15);
        assert_eq!(logical(0b0000_0010, 4, 0), 1);
    }
}
