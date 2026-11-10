//! Colour swatches shown as a sheet: one square per colour, in file order, row by row.
//!
//! Shared by the palette formats ([`crate::gpl`], [`crate::aco`], [`crate::act`],
//! [`crate::ase`]). Each colour is converted to sRGB for display; the stored values (CMYK
//! percentages, Lab, HSB) are kept for the info panel.
//!
//! Conversions: CMYK is shown uncalibrated (`(1 − c)(1 − k)` per channel, as an unmanaged
//! preview); Lab is CIE L\*a\*b\* with a D50 white, adapted to sRGB's D65 with the Bradford
//! matrix; HSB is hue in degrees, saturation and brightness 0–1.

use crate::{Error, Image, Info, Model, Pixels, Result};

/// Side of one colour square.
pub const CELL: u32 = 32;
/// Transparent space between squares.
pub const GAP: u32 = 4;
/// Widest sheet in squares when the file does not say.
pub const MAX_COLUMNS: u32 = 16;
/// Most colours a palette may hold (more is a corrupt count).
pub const MAX_COLOURS: usize = 1 << 16;
/// Colours listed one by one in the info panel.
pub const LISTED: usize = 256;

/// One colour of a palette.
#[derive(Clone, Debug, PartialEq)]
pub struct Swatch {
    pub name: String,
    /// sRGB for display, with alpha (0 for a transparent entry).
    pub rgba: [u8; 4],
    /// The stored values in words ("CMYK 0 50 100 0 %"), empty when sRGB says it all.
    pub stored: String,
    /// The group it belongs to (Adobe Swatch Exchange), else empty.
    pub group: String,
}

impl Swatch {
    pub fn rgb(name: impl Into<String>, r: u8, g: u8, b: u8) -> Swatch {
        Swatch {
            name: name.into(),
            rgba: [r, g, b, 255],
            stored: String::new(),
            group: String::new(),
        }
    }
}

/// A palette read from a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Palette {
    pub name: String,
    pub swatches: Vec<Swatch>,
    /// Squares per row the file asks for (GIMP's `Columns:`), 0 when unset.
    pub columns: u32,
    /// Format-specific facts for the info panel, before the colours.
    pub fields: Vec<(String, String)>,
    /// Short technical description.
    pub coding: String,
}

impl Palette {
    fn columns_used(&self) -> u32 {
        let n = self.swatches.len().max(1) as u32;
        if self.columns > 0 {
            self.columns.min(n).min(256)
        } else {
            // Close to square, no wider than MAX_COLUMNS.
            let mut c = 1;
            while c * c < n {
                c += 1;
            }
            c.min(MAX_COLUMNS)
        }
    }

    /// The sheet's size in pixels.
    pub fn size(&self) -> (u32, u32) {
        let cols = self.columns_used();
        let rows = (self.swatches.len().max(1) as u32).div_ceil(cols);
        (cols * (CELL + GAP) - GAP, rows * (CELL + GAP) - GAP)
    }

    pub fn info(&self) -> Result<Info> {
        if self.swatches.is_empty() {
            return Err(Error::Unsupported("the palette has no colours".into()));
        }
        let (width, height) = self.size();
        let mut summary = Vec::new();
        if !self.name.is_empty() {
            summary.push(("title", self.name.clone()));
        }
        let mut fields = Vec::new();
        if !self.name.is_empty() {
            fields.push(("Name".into(), self.name.clone()));
        }
        fields.push(("Colours".into(), self.swatches.len().to_string()));
        fields.extend(self.fields.iter().cloned());
        for (i, s) in self.swatches.iter().take(LISTED).enumerate() {
            let [r, g, b, a] = s.rgba;
            let mut v = format!("#{r:02X}{g:02X}{b:02X}");
            if a == 0 {
                v += " transparent";
            }
            if !s.stored.is_empty() {
                v += &format!(" ({})", s.stored);
            }
            if !s.group.is_empty() {
                v += &format!(", group {}", s.group);
            }
            let key = if s.name.trim().is_empty() {
                format!("Colour {}", i + 1)
            } else {
                format!("{} {}", i + 1, s.name.trim())
            };
            fields.push((key, v));
        }
        if self.swatches.len() > LISTED {
            fields.push((
                "More colours".into(),
                (self.swatches.len() - LISTED).to_string(),
            ));
        }
        Ok(Info {
            width,
            height,
            pages: 1,
            bit_depth: 8,
            model: Model::Palette,
            has_alpha: true,
            orientation: 1,
            coding: self.coding.clone(),
            summary,
            fields,
        })
    }

    /// The sheet: squares on a transparent ground.
    pub fn render(&self) -> Result<Image> {
        if self.swatches.is_empty() {
            return Err(Error::Unsupported("the palette has no colours".into()));
        }
        let (w, h) = self.size();
        let cols = self.columns_used() as usize;
        let mut px = vec![0u8; w as usize * h as usize * 4];
        let stride = w as usize * 4;
        for (i, s) in self.swatches.iter().enumerate() {
            let (cx, cy) = (i % cols, i / cols);
            let (x0, y0) = (cx * (CELL + GAP) as usize, cy * (CELL + GAP) as usize);
            for y in y0..y0 + CELL as usize {
                let row = &mut px[y * stride + x0 * 4..y * stride + (x0 + CELL as usize) * 4];
                for (x, p) in row.chunks_exact_mut(4).enumerate() {
                    if s.rgba[3] == 0 {
                        // A transparent entry: a diagonal line across an empty square.
                        let on = (x as i64 - (y - y0) as i64).abs() <= 1
                            || x == 0
                            || y == y0
                            || x == CELL as usize - 1
                            || y == y0 + CELL as usize - 1;
                        if on {
                            p.copy_from_slice(&[128, 128, 128, 255]);
                        }
                    } else {
                        p.copy_from_slice(&s.rgba);
                    }
                }
            }
        }
        Ok(Image {
            width: w,
            height: h,
            pixels: Pixels::Rgba8(px),
            truncated: false,
        })
    }
}

/// 0–1 to 8 bits, rounded and clamped.
pub fn unit8(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Uncalibrated CMYK (each 0–1, 1 = full ink) to sRGB.
pub fn cmyk_to_rgb(c: f64, m: f64, y: f64, k: f64) -> [u8; 3] {
    let k = 1.0 - k.clamp(0.0, 1.0);
    [
        unit8((1.0 - c) * k),
        unit8((1.0 - m) * k),
        unit8((1.0 - y) * k),
    ]
}

/// Hue in degrees, saturation and value 0–1, to RGB 0–1.
pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> [f64; 3] {
    let h = h.rem_euclid(360.0) / 60.0;
    let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i as u32 % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// RGB 0–1 to hue in degrees, saturation and value 0–1.
pub fn rgb_to_hsv(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    [h, if max <= 0.0 { 0.0 } else { d / max }, max]
}

fn srgb_encode(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// CIE L\*a\*b\* (D50 white) to sRGB.
pub fn lab_to_rgb(l: f64, a: f64, b: f64) -> [u8; 3] {
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let inv = |t: f64| {
        if t > 6.0 / 29.0 {
            t * t * t
        } else {
            3.0 * (6.0f64 / 29.0).powi(2) * (t - 4.0 / 29.0)
        }
    };
    // D50 white.
    let (x, y, z) = (0.964_22 * inv(fx), inv(fy), 0.825_21 * inv(fz));
    // XYZ (D50) to linear sRGB, Bradford-adapted to D65.
    let r = 3.133_856 * x - 1.616_867 * y - 0.490_615 * z;
    let g = -0.978_768 * x + 1.916_141 * y + 0.033_454 * z;
    let bl = 0.071_945 * x - 0.228_991 * y + 1.405_243 * z;
    [
        unit8(srgb_encode(r)),
        unit8(srgb_encode(g)),
        unit8(srgb_encode(bl)),
    ]
}

/// UTF-16 big-endian code units to text, stopping at a NUL.
pub fn utf16be(d: &[u8]) -> String {
    let units: Vec<u16> = d
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Trimmed text with a cap on its length.
pub fn short(s: &str) -> String {
    let s = s.trim();
    match s.char_indices().nth(200) {
        Some((i, _)) => s[..i].to_owned(),
        None => s.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert_eq!(cmyk_to_rgb(0.0, 0.0, 0.0, 0.0), [255, 255, 255]);
        assert_eq!(cmyk_to_rgb(1.0, 0.0, 0.0, 0.0), [0, 255, 255]);
        assert_eq!(lab_to_rgb(100.0, 0.0, 0.0), [255, 255, 255]);
        assert_eq!(lab_to_rgb(0.0, 0.0, 0.0), [0, 0, 0]);
        let mid = lab_to_rgb(50.0, 0.0, 0.0);
        assert!(mid.iter().all(|&v| (118..=120).contains(&v)), "{mid:?}");
        let [r, g, b] = hsv_to_rgb(120.0, 1.0, 1.0);
        assert_eq!([r, g, b], [0.0, 1.0, 0.0]);
        let h = rgb_to_hsv([0.0, 0.0, 1.0]);
        assert_eq!(h, [240.0, 1.0, 1.0]);
    }

    #[test]
    fn sheet_size() {
        let mut p = Palette {
            swatches: (0..20).map(|i| Swatch::rgb("", i, i, i)).collect(),
            ..Palette::default()
        };
        assert_eq!(p.size(), (5 * 36 - 4, 4 * 36 - 4));
        p.columns = 10;
        assert_eq!(p.size(), (10 * 36 - 4, 2 * 36 - 4));
        let img = p.render().unwrap();
        assert_eq!((img.width, img.height), p.size());
    }
}
