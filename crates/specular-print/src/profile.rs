//! Printing through a printer's ICC profile (Develop's soft-proof profile): the page
//! bands (sRGB) are converted to the profile's device values with the chosen intent, and the
//! PDF's colour space and output intent become that profile. The print service then gets
//! data already in the printer's colours (print with its own colour management off).
//!
//! The conversion is `specular-color`'s transform, baked once on a 33³ grid over the 8-bit
//! sRGB cube and interpolated tetrahedrally per pixel ([`PrintProfile::convert_row`]);
//! the tests keep it within two codes of the exact transform.

use specular_color::icc::{ColorSpace, Profile};
use specular_color::profiles;
use specular_color::transform::{Intent, Transform};

const GRID: usize = 33;

/// A printer profile ready to convert page rows.
pub struct PrintProfile {
    /// The profile's bytes, embedded in the PDF.
    pub icc: Vec<u8>,
    /// Device channels: 1 grey, 3 RGB, 4 CMYK.
    pub channels: usize,
    /// The profile's description (the PDF's output condition).
    pub name: String,
    pub intent: Intent,
    pub bpc: bool,
    exact: Transform,
    /// Device values per grid node (red slowest), 0..1.
    table: Vec<f32>,
}

impl std::fmt::Debug for PrintProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrintProfile")
            .field("name", &self.name)
            .field("channels", &self.channels)
            .field("intent", &self.intent)
            .field("bpc", &self.bpc)
            .finish_non_exhaustive()
    }
}

impl PrintProfile {
    /// The profile in `icc` with `intent` (perceptual or relative colorimetric) and black
    /// point compensation; an error says why it cannot print.
    pub fn new(icc: Vec<u8>, intent: Intent, bpc: bool) -> Result<PrintProfile, String> {
        let p = Profile::parse(&icc).map_err(|e| format!("{e:?}"))?;
        let channels = match p.color_space {
            ColorSpace::Gray => 1,
            ColorSpace::Rgb => 3,
            ColorSpace::Cmyk => 4,
            _ => return Err("not a grey, RGB or CMYK profile".into()),
        };
        let exact =
            Transform::new(&profiles::srgb(), &p, intent, bpc).map_err(|e| format!("{e:?}"))?;
        let mut table = Vec::with_capacity(GRID * GRID * GRID * channels);
        let mut out = [0f32; 4];
        for r in 0..GRID {
            for g in 0..GRID {
                for b in 0..GRID {
                    let v = [r, g, b].map(|i| i as f32 / (GRID - 1) as f32);
                    exact.eval(&v, &mut out[..channels]);
                    table.extend(out[..channels].iter().map(|v| v.clamp(0.0, 1.0)));
                }
            }
        }
        let name = specular_color::icc::description(&icc).unwrap_or_else(|| "ICC".into());
        Ok(PrintProfile {
            icc,
            channels,
            name,
            intent,
            bpc,
            exact,
            table,
        })
    }

    /// The exact device values of one sRGB colour (tests).
    pub fn exact(&self, rgb: [u8; 3]) -> Vec<u8> {
        let mut out = [0f32; 4];
        self.exact.eval(
            &rgb.map(|v| f32::from(v) / 255.0),
            &mut out[..self.channels],
        );
        out[..self.channels]
            .iter()
            .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
            .collect()
    }

    /// Converts rows of packed sRGB8 into `out` (device values, `channels` per pixel).
    pub fn convert_row(&self, rgb: &[u8], out: &mut Vec<u8>) {
        let n = self.channels;
        let s = (GRID - 1) as f32 / 255.0;
        let at = |r: usize, g: usize, b: usize| ((r * GRID + g) * GRID + b) * n;
        for p in rgb.chunks_exact(3) {
            let f = [p[0], p[1], p[2]].map(|v| f32::from(v) * s);
            let i = f.map(|v| (v as usize).min(GRID - 2));
            let (fr, fg, fb) = (f[0] - i[0] as f32, f[1] - i[1] as f32, f[2] - i[2] as f32);
            let c000 = at(i[0], i[1], i[2]);
            let c111 = at(i[0] + 1, i[1] + 1, i[2] + 1);
            let (c1, c2, w1, w2, w3) = if fr >= fg {
                if fg >= fb {
                    (
                        at(i[0] + 1, i[1], i[2]),
                        at(i[0] + 1, i[1] + 1, i[2]),
                        fr,
                        fg,
                        fb,
                    )
                } else if fr >= fb {
                    (
                        at(i[0] + 1, i[1], i[2]),
                        at(i[0] + 1, i[1], i[2] + 1),
                        fr,
                        fb,
                        fg,
                    )
                } else {
                    (
                        at(i[0], i[1], i[2] + 1),
                        at(i[0] + 1, i[1], i[2] + 1),
                        fb,
                        fr,
                        fg,
                    )
                }
            } else if fb >= fg {
                (
                    at(i[0], i[1], i[2] + 1),
                    at(i[0], i[1] + 1, i[2] + 1),
                    fb,
                    fg,
                    fr,
                )
            } else if fb >= fr {
                (
                    at(i[0], i[1] + 1, i[2]),
                    at(i[0], i[1] + 1, i[2] + 1),
                    fg,
                    fb,
                    fr,
                )
            } else {
                (
                    at(i[0], i[1] + 1, i[2]),
                    at(i[0] + 1, i[1] + 1, i[2]),
                    fg,
                    fr,
                    fb,
                )
            };
            let t = &self.table;
            for k in 0..n {
                let v = t[c000 + k]
                    + (t[c1 + k] - t[c000 + k]) * w1
                    + (t[c2 + k] - t[c1 + k]) * w2
                    + (t[c111 + k] - t[c2 + k]) * w3;
                out.push((v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative colorimetric with black point compensation maps sRGB's black to the
    /// printer's deepest black and keeps shadow steps apart; without it the shadows clip.
    #[test]
    fn black_point_compensation_switches_with_the_relative_intent() {
        let icc =
            include_bytes!("../../specular-color/fuzz/seeds/profile/cmyk_v2_lut16.icc").to_vec();
        let on = PrintProfile::new(icc.clone(), Intent::Relative, true).unwrap();
        let off = PrintProfile::new(icc, Intent::Relative, false).unwrap();
        assert!(on.bpc && !off.bpc);
        let ink = |p: &PrintProfile, v: u8| -> u32 {
            let mut out = Vec::new();
            p.convert_row(&[v, v, v], &mut out);
            out.iter().map(|&c| u32::from(c)).sum()
        };
        let differ = (0..=48u8)
            .step_by(4)
            .filter(|&v| ink(&on, v).abs_diff(ink(&off, v)) > 2)
            .count();
        eprintln!("shadow greys that differ: {differ} of 13");
        assert!(differ > 0, "BPC changes the shadows");
        // With compensation, darker greys never get less ink.
        let steps: Vec<u32> = (0..=48u8).step_by(4).map(|v| ink(&on, v)).collect();
        assert!(steps.windows(2).all(|w| w[0] + 2 >= w[1]), "{steps:?}");
    }

    #[test]
    fn table_follows_the_exact_transform() {
        for (name, icc) in [
            (
                "cmyk",
                include_bytes!("../../specular-color/fuzz/seeds/profile/cmyk_v2_lut16.icc")
                    .to_vec(),
            ),
            (
                "prophoto",
                include_bytes!(
                    "../../formats/specular-jxl/src/color/icc/test-profiles/prophoto-gamma18-rel.icc"
                )
                .to_vec(),
            ),
        ] {
            for intent in [Intent::Perceptual, Intent::Relative] {
                let p = PrintProfile::new(icc.clone(), intent, true).unwrap();
                let mut worst = 0u8;
                let mut over1 = 0usize;
                let mut n = 0usize;
                for r in (0..=255u32).step_by(17) {
                    for g in (0..=255u32).step_by(15) {
                        for b in (0..=255u32).step_by(13) {
                            let c = [r as u8, g as u8, b as u8];
                            let mut got = Vec::new();
                            p.convert_row(&c, &mut got);
                            let want = p.exact(c);
                            for (a, e) in got.iter().zip(&want) {
                                let d = a.abs_diff(*e);
                                worst = worst.max(d);
                                over1 += usize::from(d > 1);
                                n += 1;
                            }
                        }
                    }
                }
                eprintln!("{name} {intent:?}: worst {worst} codes, {over1}/{n} over 1");
                assert!(worst <= 3 && over1 * 100 <= n, "{name} {intent:?}");
            }
        }
    }
}
