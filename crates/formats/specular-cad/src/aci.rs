//! The AutoCAD Color Index (ACI): 255 colours by number.
//!
//! 1-9 are the named colours, 10-249 are 24 hues (15° apart) in groups of ten: five
//! brightness levels, each fully saturated (even) and half saturated (odd), and 250-255 are
//! greys. 7 is the foreground colour (white on dark, black on light) and is resolved by the
//! caller; 0 (BYBLOCK) and 256 (BYLAYER) are not colours.

use specular_vector::Color;

const NAMED: [[u8; 3]; 10] = [
    [0, 0, 0],
    [255, 0, 0],
    [255, 255, 0],
    [0, 255, 0],
    [0, 255, 255],
    [0, 0, 255],
    [255, 0, 255],
    [255, 255, 255],
    [128, 128, 128],
    [192, 192, 192],
];

const GREYS: [u8; 6] = [51, 91, 132, 173, 214, 255];
const LEVELS: [u32; 5] = [255, 165, 127, 76, 38];

/// RGB of ACI `index` (1..=255); out-of-range numbers give the foreground's white.
pub(crate) fn rgb(index: i64) -> [u8; 3] {
    match index {
        1..=9 => NAMED[index as usize],
        10..=249 => {
            let i = (index - 10) as u32;
            let hue = (i / 10) * 15;
            let v = LEVELS[((i % 10) / 2) as usize];
            let lo = if i.is_multiple_of(2) { 0 } else { v / 2 };
            // HSV with saturation 1 or 1/2: the hue decides the ramp between lo and v.
            let sector = hue / 60;
            let frac = hue % 60;
            let up = lo + (v - lo) * frac / 60;
            let down = lo + (v - lo) * (60 - frac) / 60;
            let (r, g, b) = match sector {
                0 => (v, up, lo),
                1 => (down, v, lo),
                2 => (lo, v, up),
                3 => (lo, down, v),
                4 => (up, lo, v),
                _ => (v, lo, down),
            };
            [r as u8, g as u8, b as u8]
        }
        250..=255 => {
            let g = GREYS[(index - 250) as usize];
            [g, g, g]
        }
        _ => [255, 255, 255],
    }
}

/// The colour of ACI `index`, with 7 (and anything out of range) as the foreground.
pub(crate) fn color(index: i64) -> Color {
    match index {
        1..=6 | 8..=255 => {
            let [r, g, b] = rgb(index);
            Color::Rgb(r, g, b)
        }
        _ => Color::Foreground,
    }
}

/// A DXF true colour (group 420): 0x00RRGGBB.
pub(crate) fn true_color(v: i64) -> Color {
    Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_entries() {
        assert_eq!(rgb(1), [255, 0, 0]);
        assert_eq!(rgb(10), [255, 0, 0]);
        assert_eq!(rgb(11), [255, 127, 127]);
        assert_eq!(rgb(12), [165, 0, 0]);
        assert_eq!(rgb(20), [255, 63, 0]);
        assert_eq!(rgb(30), [255, 127, 0]);
        assert_eq!(rgb(50), [255, 255, 0]);
        assert_eq!(rgb(90), [0, 255, 0]);
        assert_eq!(rgb(130), [0, 255, 255]);
        assert_eq!(rgb(170), [0, 0, 255]);
        assert_eq!(rgb(210), [255, 0, 255]);
        assert_eq!(rgb(240), [255, 0, 63]);
        assert_eq!(rgb(255), [255, 255, 255]);
        assert_eq!(color(7), Color::Foreground);
    }
}
