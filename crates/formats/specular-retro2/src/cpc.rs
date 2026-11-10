//! Amstrad CPC screen memory dumps (`.scr`): the 16 KiB at &C000, saved as is or with the
//! 128-byte AMSDOS header.
//!
//! Line `y` starts at `(y / 8) * 80 + (y % 8) * 2048`; each line is 80 bytes. The dump
//! does not say its screen mode (the picture's resolution and colour count), so the mode
//! comes from an OCP Art Studio `.pal` beside it, else mode 1, the machine's start-up
//! mode; the other two modes are the file's other pages. Colours are the 27 of the
//! hardware, from the `.pal` or the firmware's default pens.

use crate::{Canvas, Error, Hints, Image, Info, Result, at, field};

/// Screen memory size.
pub const SCREEN_BYTES: usize = 16384;
/// Bytes of an AMSDOS header.
pub const AMSDOS_HEADER: usize = 128;
/// The shortest dump: the last 48 bytes of screen memory are never shown and often left
/// out.
const MIN_SCREEN: usize = 16336;

/// Firmware colour `n` (0–26) as RGB: `n = 9·green + 3·red + blue`, each at 0, half or full.
pub fn firmware_rgb(n: u8) -> [u8; 3] {
    let n = n.min(26);
    let level = |v: u8| [0u8, 128, 255][v as usize];
    [level((n / 3) % 3), level(n / 9), level(n % 3)]
}

/// Hardware colour numbers (`&40`–`&5F`, low 5 bits) to firmware colour numbers.
const HW_TO_FW: [u8; 32] = [
    13, 13, 19, 25, 1, 7, 10, 16, 7, 25, 24, 26, 6, 8, 15, 17, 1, 19, 18, 20, 0, 2, 9, 11, 4, 22,
    21, 23, 3, 5, 12, 14,
];

/// The firmware's default pens per mode.
const DEFAULT_PENS: [u8; 16] = [1, 24, 20, 6, 26, 0, 2, 8, 10, 12, 14, 16, 18, 22, 24, 16];
const DEFAULT_PENS_MODE1: [u8; 4] = [1, 24, 20, 6];
const DEFAULT_PENS_MODE2: [u8; 2] = [1, 24];

/// The AMSDOS header of `data`, when it has a valid one: (file type, load address, length).
fn amsdos(data: &[u8]) -> Option<(u8, u16, usize)> {
    let h = data.get(..AMSDOS_HEADER)?;
    let sum: u32 = h[..67].iter().map(|&b| b as u32).sum();
    let stored = u16::from_le_bytes([h[67], h[68]]) as u32;
    if sum == 0 || sum & 0xFFFF != stored {
        return None;
    }
    let load = u16::from_le_bytes([h[0x15], h[0x16]]);
    let len = h[0x40] as usize | (h[0x41] as usize) << 8 | (h[0x42] as usize) << 16;
    Some((h[0x12], load, len))
}

/// Whether `data` is an AMSDOS file loading at &C000 that fills the screen.
pub fn amsdos_screen(data: &[u8]) -> bool {
    amsdos(data).is_some_and(|(_, load, len)| {
        load == 0xC000
            && (MIN_SCREEN..=SCREEN_BYTES).contains(&len)
            && data.len() >= AMSDOS_HEADER + MIN_SCREEN
    })
}

/// The screen memory of `data` and whether it had an AMSDOS header.
fn screen(data: &[u8]) -> Result<(&[u8], bool)> {
    if amsdos_screen(data) {
        let end = (AMSDOS_HEADER + SCREEN_BYTES).min(data.len());
        return Ok((&data[AMSDOS_HEADER..end], true));
    }
    if amsdos(data).is_some() {
        return Err(Error::Unsupported(
            "this Amstrad CPC file does not load into screen memory: it is a program or data, not a picture"
                .into(),
        ));
    }
    if data.starts_with(b"MZ") {
        return Err(Error::Unsupported(
            "this is a Windows screen saver (a program), not an Amstrad CPC screen".into(),
        ));
    }
    if (MIN_SCREEN..=SCREEN_BYTES).contains(&data.len()) {
        return Ok((data, false));
    }
    if data.len() < SCREEN_BYTES && data.len() > 2048 {
        return Err(Error::Unsupported(
            "packed or partial Amstrad CPC screens are not read: only 16 KB screen dumps".into(),
        ));
    }
    Err(Error::NotRecognised)
}

/// Screen mode and 16 pens (firmware colours) from an OCP Art Studio `.pal`.
fn palette_file(pal: &[u8]) -> Option<(u8, [u8; 16])> {
    if pal.len() < 3 + 16 * 12 {
        return None;
    }
    let mode = pal[0];
    if mode > 2 {
        return None;
    }
    let mut pens = [0u8; 16];
    for (i, p) in pens.iter_mut().enumerate() {
        let v = pal[3 + i * 12];
        *p = if v >= 0x40 {
            HW_TO_FW[(v & 0x1F) as usize]
        } else if v < 27 {
            v
        } else {
            return None;
        };
    }
    Some((mode, pens))
}

/// The modes in page order: the `.pal`'s (or 1) first.
fn modes(hints: &Hints) -> [u8; 3] {
    match hints.palette.and_then(palette_file).map(|p| p.0) {
        Some(0) => [0, 1, 2],
        Some(2) => [2, 0, 1],
        _ => [1, 0, 2],
    }
}

/// Output size of a mode (pixels repeated to the CPC's 4:3 screen).
fn size(mode: u8) -> (u32, u32, u32, u32) {
    match mode {
        0 => (160, 200, 2, 1),
        1 => (320, 200, 1, 1),
        _ => (640, 200, 1, 2),
    }
}

pub fn info(data: &[u8], hints: &Hints, page: u32) -> Result<Info> {
    let (_, header) = screen(data)?;
    let mode = *modes(hints)
        .get(page as usize)
        .ok_or_else(|| Error::Malformed("no such page".into()))?;
    let (w, h, sx, sy) = size(mode);
    let colours = [16, 4, 2][mode as usize];
    let pal = hints.palette.and_then(palette_file);
    let mut fields = vec![
        field("Machine", "Amstrad CPC"),
        field(
            "Screen mode",
            format!(
                "{mode} ({}×{}, {colours} colours){}",
                w,
                h,
                if pal.is_some_and(|p| p.0 == mode) {
                    ""
                } else if page == 0 {
                    ", assumed"
                } else {
                    ", alternative"
                }
            ),
        ),
        field(
            "Palette",
            if pal.is_some() {
                "from the .pal file beside it"
            } else {
                "firmware default"
            },
        ),
    ];
    if header {
        fields.push(field("Header", "AMSDOS"));
    }
    Ok(Info {
        width: w * sx,
        height: h * sy,
        pages: 3,
        bit_depth: [4, 2, 1][mode as usize],
        palette: true,
        orientation: 1,
        coding: format!("screen mode {mode}, {colours} colours"),
        fields,
    })
}

/// The pen of pixel `i` of a screen byte in `mode`.
pub fn pen(mode: u8, b: u8, i: u32) -> u8 {
    let bit = |n: u32| (b >> n) & 1;
    match mode {
        0 => {
            let s = 1 - i; // pixel 0: bits 7, 3, 5, 1
            bit(6 + s) | bit(2 + s) << 1 | bit(4 + s) << 2 | bit(s) << 3
        }
        1 => bit(7 - i) | bit(3 - i) << 1,
        _ => bit(7 - i),
    }
}

pub fn decode(data: &[u8], hints: &Hints, page: u32) -> Result<Image> {
    let (scr, _) = screen(data)?;
    let mode = *modes(hints)
        .get(page as usize)
        .ok_or_else(|| Error::Malformed("no such page".into()))?;
    let pens: [u8; 16] = match hints.palette.and_then(palette_file) {
        Some((m, p)) if m == mode => p,
        _ => {
            let mut p = DEFAULT_PENS;
            match mode {
                1 => p[..4].copy_from_slice(&DEFAULT_PENS_MODE1),
                2 => p[..2].copy_from_slice(&DEFAULT_PENS_MODE2),
                _ => {}
            }
            p
        }
    };
    let (w, h, sx, sy) = size(mode);
    let per_byte = [2u32, 4, 8][mode as usize];
    let mut c = Canvas::new(w, h, sx, sy);
    for y in 0..h {
        let line = (y as usize / 8) * 80 + (y as usize % 8) * 2048;
        for xb in 0..80u32 {
            let b = at(scr, line + xb as usize);
            for i in 0..per_byte {
                let p = pen(mode, b, i);
                c.put(xb * per_byte + i, y, firmware_rgb(pens[p as usize & 15]));
            }
        }
    }
    Ok(c.into_image(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firmware_colours() {
        assert_eq!(firmware_rgb(0), [0, 0, 0]);
        assert_eq!(firmware_rgb(1), [0, 0, 128]);
        assert_eq!(firmware_rgb(6), [255, 0, 0]);
        assert_eq!(firmware_rgb(24), [255, 255, 0]);
        assert_eq!(firmware_rgb(26), [255, 255, 255]);
        // Hardware &54 is black, &4B bright white.
        assert_eq!(HW_TO_FW[0x14], 0);
        assert_eq!(HW_TO_FW[0x0B], 26);
    }

    #[test]
    fn pixel_bits() {
        // Mode 1: pixel 0 is bits 7 (pen bit 0) and 3 (pen bit 1).
        assert_eq!(pen(1, 0b1000_0000, 0), 1);
        assert_eq!(pen(1, 0b0000_1000, 0), 2);
        assert_eq!(pen(1, 0b0001_0001, 3), 3);
        // Mode 0: pixel 0 is bits 7, 3, 5, 1 (pen bits 0–3).
        assert_eq!(pen(0, 0b1010_1010, 0), 15);
        assert_eq!(pen(0, 0b0000_0010, 0), 8);
        assert_eq!(pen(0, 0b0100_0000, 1), 1);
        assert_eq!(pen(2, 0b0000_0001, 7), 1);
    }
}
