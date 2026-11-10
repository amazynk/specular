//! ZX Spectrum screen dumps (`.scr`, "SCREEN$"): 6144 bytes of bitmap and 768 of attributes,
//! 6912 in all (a bitmap-only dump is 6144). 256×192 pixels.
//!
//! Bitmap rows are interleaved the way the ULA reads them: row y is at
//! `(y & 0xC0) << 5 | (y & 7) << 8 | (y & 0x38) << 2`. Each 8×8 cell's attribute byte gives
//! ink (bits 0–2), paper (bits 3–5), BRIGHT (bit 6) and FLASH (bit 7, shown unflashed).
//!
//! No signature: `.scr` is also a Windows screen saver, so only files of exactly these sizes
//! are taken.

use crate::*;

pub const BITMAP: usize = 6144;
pub const FULL: usize = 6912;

/// Channel levels of normal and BRIGHT colours.
const NORMAL: u8 = 0xc0;
const BRIGHT: u8 = 0xff;

pub fn probe_file(data: &[u8]) -> bool {
    matches!(data.len(), BITMAP | FULL)
}

pub fn info(data: &[u8]) -> Result<Info> {
    if !probe_file(data) {
        return Err(Error::NotRecognised);
    }
    let coding = if data.len() == FULL {
        "ZX Spectrum screen, attributes"
    } else {
        "ZX Spectrum screen, bitmap only"
    };
    Ok(Info::new(
        256,
        192,
        PixelFormat::Rgba8,
        Model::Palette,
        4,
        coding,
    ))
}

fn colour(c: u8, bright: bool) -> [u8; 4] {
    let v = if bright { BRIGHT } else { NORMAL };
    // Colour bits: blue 1, red 2, green 4.
    [
        if c & 2 != 0 { v } else { 0 },
        if c & 4 != 0 { v } else { 0 },
        if c & 1 != 0 { v } else { 0 },
        255,
    ]
}

pub fn decode(data: &[u8]) -> Result<Image> {
    info(data)?;
    let mut img = Image::filled(256, 192, PixelFormat::Rgba8, 0);
    for y in 0..192usize {
        let row = (y & 0xc0) << 5 | (y & 7) << 8 | (y & 0x38) << 2;
        for cx in 0..32usize {
            let bits = data[row + cx];
            let (ink, paper) = match data.get(BITMAP + (y / 8) * 32 + cx) {
                Some(&a) => {
                    let b = a & 0x40 != 0;
                    (colour(a & 7, b), colour(a >> 3 & 7, b))
                }
                None => ([0, 0, 0, 255], [NORMAL, NORMAL, NORMAL, 255]),
            };
            for k in 0..8 {
                let px = if bits >> (7 - k) & 1 != 0 { ink } else { paper };
                let o = (y * 256 + cx * 8 + k) * 4;
                img.data[o..o + 4].copy_from_slice(&px);
            }
        }
    }
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_and_attributes() {
        let mut f = vec![0u8; FULL];
        // Row 1 (address 0x100), first byte: left pixel set.
        f[0x100] = 0x80;
        // Row 8 is at 0x20.
        f[0x20] = 0x01;
        // Cell 0: bright, paper blue (1), ink red (2).
        f[BITMAP] = 0x40 | 1 << 3 | 2;
        let img = decode(&f).unwrap();
        let px = |x: usize, y: usize| &img.data[(y * 256 + x) * 4..][..4];
        assert_eq!(px(0, 1), [255, 0, 0, 255]);
        assert_eq!(px(1, 1), [0, 0, 255, 255]);
        assert_eq!(px(7, 8), [0, 0, 0, 255]);
        assert_eq!(px(6, 8), [0, 0, 0, 255]);
        assert!(info(&f[..100]).is_err());
    }
}
