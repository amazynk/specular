//! Apple II hi-res (`.hgr`: the 8 KiB page at &2000, 280×192) and double hi-res (`.dhr`,
//! `.a2fc`: the auxiliary 8 KiB, then the main 8 KiB, 560 dots or 140 colour pixels a line).
//!
//! Line `y` starts at `(y % 8) * 1024 + ((y / 8) % 8) * 128 + (y / 64) * 40`. Colour is the
//! NTSC artefact colour the machine shows: in hi-res, a lit dot beside another is white, a
//! lone one violet or green (blue or orange when its byte's high bit is set) by column; in
//! double hi-res, each group of four dots is one of the 16 low-res colours. Both are drawn
//! 560×384 (dots repeated) to keep the screen's 4:3 shape.

use crate::{Canvas, Error, Image, Info, Result, at, field};

/// The 16 low-res colours (approximate sRGB of the NTSC signal).
pub const LORES: [[u8; 3]; 16] = [
    [0, 0, 0],
    [227, 30, 96],
    [96, 78, 189],
    [255, 68, 253],
    [0, 163, 96],
    [156, 156, 156],
    [20, 207, 253],
    [208, 195, 255],
    [96, 114, 3],
    [255, 106, 60],
    [156, 156, 156],
    [255, 160, 208],
    [20, 245, 60],
    [208, 221, 141],
    [114, 255, 208],
    [255, 255, 255],
];

const PAGE: usize = 8192;

/// Offset of line `y` in an 8 KiB page.
pub fn line_offset(y: u32) -> usize {
    ((y % 8) * 1024 + ((y / 8) % 8) * 128 + (y / 64) * 40) as usize
}

fn check(data: &[u8], double: bool) -> Result<()> {
    let ok = if double {
        (2 * PAGE - 8..=2 * PAGE).contains(&data.len())
    } else {
        (PAGE - 8..=PAGE).contains(&data.len())
    };
    if ok {
        return Ok(());
    }
    if !double && (2 * PAGE - 8..=2 * PAGE).contains(&data.len()) {
        return Err(Error::Unsupported(
            "this is 16 KB: a double hi-res picture. Name it .dhr to open it.".into(),
        ));
    }
    Err(Error::Unsupported(format!(
        "an Apple II {} screen is {} KB; this file is {} bytes (packed pictures are not read)",
        if double { "double hi-res" } else { "hi-res" },
        if double { 16 } else { 8 },
        data.len()
    )))
}

pub fn info(data: &[u8], double: bool) -> Result<Info> {
    check(data, double)?;
    Ok(Info {
        width: 560,
        height: 384,
        pages: 1,
        bit_depth: if double { 4 } else { 1 },
        palette: true,
        orientation: 1,
        coding: if double {
            "double hi-res, 140×192 in 16 colours".into()
        } else {
            "hi-res, 280×192 with artefact colour".into()
        },
        fields: vec![
            field("Machine", if double { "Apple IIe/IIc" } else { "Apple II" }),
            field(
                "Screen mode",
                if double {
                    "double hi-res (560×192 dots)"
                } else {
                    "hi-res (280×192)"
                },
            ),
        ],
    })
}

pub fn decode(data: &[u8], double: bool) -> Result<Image> {
    check(data, double)?;
    let truncated = data.len() < if double { 2 * PAGE } else { PAGE };
    if double {
        let mut c = Canvas::new(140, 192, 4, 2);
        let mut dots = [0u8; 560];
        for y in 0..192 {
            let base = line_offset(y);
            for col in 0..40 {
                let aux = at(data, base + col);
                let main = at(data, PAGE + base + col);
                for i in 0..7 {
                    dots[col * 14 + i] = (aux >> i) & 1;
                    dots[col * 14 + 7 + i] = (main >> i) & 1;
                }
            }
            for g in 0..140u32 {
                let d = &dots[g as usize * 4..g as usize * 4 + 4];
                let idx = d[0] | d[1] << 1 | d[2] << 2 | d[3] << 3;
                c.put(g, y, LORES[idx as usize]);
            }
        }
        return Ok(c.into_image(truncated));
    }
    let mut c = Canvas::new(280, 192, 2, 2);
    let mut dots = [0u8; 282];
    let mut high = [0u8; 282];
    for y in 0..192 {
        let base = line_offset(y);
        for col in 0..40 {
            let b = at(data, base + col);
            for i in 0..7 {
                dots[1 + col * 7 + i] = (b >> i) & 1;
                high[1 + col * 7 + i] = b >> 7;
            }
        }
        // A dot's own colour: by column parity and its byte's high bit.
        let hue = |x: usize| -> [u8; 3] {
            let even = (x - 1).is_multiple_of(2);
            LORES[match (high[x], even) {
                (0, true) => 3,
                (0, false) => 12,
                (_, true) => 6,
                (_, false) => 9,
            }]
        };
        for x in 1..=280usize {
            let (l, m, r) = (dots[x - 1], dots[x], dots[x + 1]);
            let rgb = if m == 1 {
                if l == 1 || r == 1 { [255; 3] } else { hue(x) }
            } else if l == 1 && r == 1 {
                // A gap in a run of lone dots takes their colour.
                hue(x - 1)
            } else {
                [0; 3]
            };
            c.put(x as u32 - 1, y, rgb);
        }
    }
    Ok(c.into_image(truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines() {
        assert_eq!(line_offset(0), 0);
        assert_eq!(line_offset(1), 0x400);
        assert_eq!(line_offset(8), 0x80);
        assert_eq!(line_offset(64), 0x28);
        assert_eq!(line_offset(191), 0x3D0 + 0x1C00);
    }
}
