//! Commodore 64 Koala Painter (`.koa`, `.kla`): a two-byte load address (usually 0x6000),
//! 8000 bytes of bitmap, 1000 of screen memory, 1000 of colour memory and the background
//! colour: 10003 bytes (10001 without the load address).
//!
//! Multicolour mode: 160×200 pixels of two bits in 4×8 cells; 00 is the background, 01 the
//! screen byte's high nibble, 10 its low nibble, 11 the colour memory's low nibble. Pixels
//! are twice as wide as tall, so the image is shown 320×200 with each pixel doubled.
//!
//! Colours: the commonly used "Pepto" measurement of the VIC-II palette.

use crate::*;

/// The VIC-II's 16 colours (Philip "Pepto" Timmermann's measured values).
pub const PALETTE: [[u8; 3]; 16] = [
    [0x00, 0x00, 0x00],
    [0xff, 0xff, 0xff],
    [0x68, 0x37, 0x2b],
    [0x70, 0xa4, 0xb2],
    [0x6f, 0x3d, 0x86],
    [0x58, 0x8d, 0x43],
    [0x35, 0x28, 0x79],
    [0xb8, 0xc7, 0x6f],
    [0x6f, 0x4f, 0x25],
    [0x43, 0x39, 0x00],
    [0x9a, 0x67, 0x59],
    [0x44, 0x44, 0x44],
    [0x6c, 0x6c, 0x6c],
    [0x9a, 0xd2, 0x84],
    [0x6c, 0x5e, 0xb5],
    [0x95, 0x95, 0x95],
];

const BODY: usize = 10001;

/// Where the bitmap starts (after the load address when there is one).
fn start(data: &[u8]) -> Result<usize> {
    match data.len() {
        BODY => Ok(0),
        n if (BODY + 2..=BODY + 2 + 64).contains(&n) => Ok(2),
        n if n < BODY => Err(truncated("Koala images are 10003 bytes")),
        _ => Err(Error::NotRecognised),
    }
}

pub fn probe_file(data: &[u8]) -> bool {
    start(data).is_ok()
}

pub fn info(data: &[u8]) -> Result<Info> {
    start(data)?;
    Ok(Info::new(
        320,
        200,
        PixelFormat::Rgba8,
        Model::Palette,
        4,
        "C64 multicolour, 160×200 shown 320×200",
    ))
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let s = start(data)?;
    let bitmap = &data[s..s + 8000];
    let screen = &data[s + 8000..s + 9000];
    let colour = &data[s + 9000..s + 10000];
    let bg = data[s + 10000] & 15;
    let mut idx = vec![0u8; 320 * 200];
    for y in 0..200usize {
        for cx in 0..40usize {
            let cell = (y / 8) * 40 + cx;
            let byte = bitmap[cell * 8 + y % 8];
            for k in 0..4usize {
                let c = match byte >> (6 - 2 * k) & 3 {
                    0 => bg,
                    1 => screen[cell] >> 4,
                    2 => screen[cell] & 15,
                    _ => colour[cell] & 15,
                };
                let x = (cx * 4 + k) * 2;
                idx[y * 320 + x] = c;
                idx[y * 320 + x + 1] = c;
            }
        }
    }
    let pal: Vec<[u8; 4]> = PALETTE.iter().map(|&[r, g, b]| [r, g, b, 255]).collect();
    Ok(Image::from_indices(320, 200, &idx, &pal))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells() {
        let mut f = vec![0x00, 0x60];
        let mut bitmap = vec![0u8; 8000];
        bitmap[0] = 0b00_01_10_11;
        f.extend_from_slice(&bitmap);
        let mut screen = vec![0u8; 1000];
        screen[0] = 0x25; // hi 2 red, lo 5 green
        f.extend_from_slice(&screen);
        let mut colour = vec![0u8; 1000];
        colour[0] = 0xf7; // low nibble 7 yellow
        f.extend_from_slice(&colour);
        f.push(6); // blue background
        assert!(probe_file(&f));
        assert!(!probe_file(&f[..9000]));
        let img = decode(&f).unwrap();
        let px = |x: usize| &img.data[x * 4..x * 4 + 3];
        assert_eq!(px(0), PALETTE[6]);
        assert_eq!(px(1), PALETTE[6]);
        assert_eq!(px(2), PALETTE[2]);
        assert_eq!(px(4), PALETTE[5]);
        assert_eq!(px(6), PALETTE[7]);
    }
}
