//! Atari 8-bit screens: Micro Painter (`.mic`: 160×192 in 4 colours, ANTIC mode E, with
//! the colour registers appended when saved), Graphics 8 (`.gr8`: 320×192 in 2 colours) and
//! Graphics 9 (`.gr9`: 80×192 in 16 shades of one hue). Each is 7680 bytes of screen, 40
//! bytes a line.
//!
//! Colour register values (hue in the high nibble, luminance in the low) are turned into
//! RGB with an approximate NTSC palette computed from the hue angle; registers not in the
//! file take the operating system's start-up values. Pixels are drawn 2 (Micro Painter) or
//! 4 (Graphics 9) wide so every picture is 320×192.

use crate::{Canvas, Error, Image, Info, Kind, Result, at, field};

const SCREEN: usize = 7680;

/// RGB of an Atari colour register value (approximate NTSC).
pub fn atari_rgb(v: u8) -> [u8; 3] {
    let y = (v & 15) as f32 / 15.0;
    let hue = v >> 4;
    if hue == 0 {
        let g = (y * 255.0).round() as u8;
        return [g, g, g];
    }
    let a = (164.0 - 24.0 * hue as f32).to_radians();
    let s = 0.2;
    let (u, w) = (s * a.cos(), s * a.sin());
    let c = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    [
        c(y + 1.14 * w),
        c(y - 0.395 * u - 0.581 * w),
        c(y + 2.032 * u),
    ]
}

/// The operating system's start-up colour registers 708–712 (COLOR0–COLOR4).
const OS_COLOURS: [u8; 5] = [0x28, 0xCA, 0x94, 0x46, 0x00];

fn check(data: &[u8], kind: Kind) -> Result<()> {
    if (SCREEN..=SCREEN + 5).contains(&data.len()) {
        return Ok(());
    }
    let what = match kind {
        Kind::AtariMic => "Micro Painter",
        Kind::AtariGr8 => "Graphics 8",
        _ => "Graphics 9",
    };
    Err(Error::Unsupported(format!(
        "an Atari {what} picture is 7680 bytes of screen; this file is {} bytes (compressed pictures are not read)",
        data.len()
    )))
}

/// Micro Painter's four colours (pixel values 0–3) and whether the file gave them: the
/// appended bytes are COLOR0, COLOR1, COLOR2 and the background.
fn mic_colours(data: &[u8]) -> ([u8; 4], bool) {
    match data.get(SCREEN..SCREEN + 4) {
        Some(c) => ([c[3], c[0], c[1], c[2]], true),
        None => (
            [OS_COLOURS[4], OS_COLOURS[0], OS_COLOURS[1], OS_COLOURS[2]],
            false,
        ),
    }
}

pub fn info(kind: Kind, data: &[u8]) -> Result<Info> {
    check(data, kind)?;
    let (coding, bits, mode, colours) = match kind {
        Kind::AtariMic => {
            let (_, saved) = mic_colours(data);
            (
                "Micro Painter, 160×192 in 4 colours",
                2,
                "ANTIC E (Graphics 15)",
                if saved {
                    "saved with the picture"
                } else {
                    "system default"
                },
            )
        }
        Kind::AtariGr8 => (
            "Graphics 8, 320×192 in 2 colours",
            1,
            "Graphics 8",
            "system default",
        ),
        _ => (
            "Graphics 9, 80×192 in 16 shades",
            4,
            "Graphics 9 (GTIA)",
            "grey shades",
        ),
    };
    Ok(Info {
        width: 320,
        height: 192,
        pages: 1,
        bit_depth: bits,
        palette: true,
        orientation: 1,
        coding: coding.into(),
        fields: vec![
            field("Machine", "Atari 8-bit"),
            field("Screen mode", mode),
            field("Colours", colours),
        ],
    })
}

pub fn decode(kind: Kind, data: &[u8]) -> Result<Image> {
    check(data, kind)?;
    let img = match kind {
        Kind::AtariMic => {
            let (regs, _) = mic_colours(data);
            let pal = regs.map(atari_rgb);
            let mut c = Canvas::new(160, 192, 2, 1);
            for y in 0..192u32 {
                for xb in 0..40u32 {
                    let b = at(data, (y * 40 + xb) as usize);
                    for i in 0..4 {
                        c.put(xb * 4 + i, y, pal[((b >> (6 - 2 * i)) & 3) as usize]);
                    }
                }
            }
            c
        }
        Kind::AtariGr8 => {
            let bg = OS_COLOURS[2];
            let fg = (bg & 0xF0) | (OS_COLOURS[1] & 0x0F);
            let (bg, fg) = (atari_rgb(bg), atari_rgb(fg));
            let mut c = Canvas::new(320, 192, 1, 1);
            for y in 0..192u32 {
                for xb in 0..40u32 {
                    let b = at(data, (y * 40 + xb) as usize);
                    for i in 0..8 {
                        c.put(xb * 8 + i, y, if b & (0x80 >> i) != 0 { fg } else { bg });
                    }
                }
            }
            c
        }
        _ => {
            let mut c = Canvas::new(80, 192, 4, 1);
            for y in 0..192u32 {
                for xb in 0..40u32 {
                    let b = at(data, (y * 40 + xb) as usize);
                    c.put(xb * 2, y, atari_rgb(b >> 4));
                    c.put(xb * 2 + 1, y, atari_rgb(b & 15));
                }
            }
            c
        }
    };
    Ok(img.into_image(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette() {
        assert_eq!(atari_rgb(0x00), [0, 0, 0]);
        assert_eq!(atari_rgb(0x0F), [255, 255, 255]);
        // Hue 7 is blue, hue 3 red-orange, hue 12 green.
        let b = atari_rgb(0x78);
        assert!(b[2] > b[0] && b[2] > b[1]);
        let r = atari_rgb(0x38);
        assert!(r[0] > r[1] && r[0] > r[2]);
        let g = atari_rgb(0xC8);
        assert!(g[1] > g[0] && g[1] > g[2]);
    }
}
