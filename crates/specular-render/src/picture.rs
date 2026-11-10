//! Picture adjustments of a playing movie: brightness, contrast, saturation, gamma and hue,
//! drawn by the image shader (display only; never written to a file).
//!
//! They work on sRGB-encoded values, as a player's picture controls do: brightness adds,
//! contrast scales about middle grey, saturation and hue turn about the BT.709 grey axis
//! (one 3×4 matrix), and gamma bends the result. [`PictureAdjust::apply`] is the CPU
//! reference the shader is tested against.

/// Brightness −1‥1 (added), contrast 0‥2 (1 unchanged), saturation 0‥2 (0 grey), gamma
/// 0.25‥4 (1 unchanged; above 1 lighter), hue −180‥180 degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PictureAdjust {
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub gamma: f32,
    pub hue: f32,
}

impl Default for PictureAdjust {
    fn default() -> Self {
        PictureAdjust::NEUTRAL
    }
}

/// BT.709 luma weights (the grey axis saturation and hue turn about).
const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

impl PictureAdjust {
    pub const NEUTRAL: PictureAdjust = PictureAdjust {
        brightness: 0.0,
        contrast: 1.0,
        saturation: 1.0,
        gamma: 1.0,
        hue: 0.0,
    };

    /// Nothing changes: the shader skips the step.
    pub fn is_neutral(&self) -> bool {
        (self.brightness).abs() < 1e-4
            && (self.contrast - 1.0).abs() < 1e-4
            && (self.saturation - 1.0).abs() < 1e-4
            && (self.gamma - 1.0).abs() < 1e-4
            && self.hue.abs() < 1e-3
    }

    /// The values kept within their ranges.
    pub fn clamped(self) -> PictureAdjust {
        PictureAdjust {
            brightness: self.brightness.clamp(-1.0, 1.0),
            contrast: self.contrast.clamp(0.0, 2.0),
            saturation: self.saturation.clamp(0.0, 2.0),
            gamma: self.gamma.clamp(0.25, 4.0),
            hue: self.hue.clamp(-180.0, 180.0),
        }
    }

    /// The affine part as three rows (offset in the fourth column): contrast and brightness,
    /// then saturation and hue as a scale and turn of the BT.709 Cb/Cr plane (grey and luma
    /// stay).
    pub fn rows(&self) -> [[f32; 4]; 3] {
        let [lr, lg, lb] = LUMA;
        let (kb, kr) = (2.0 * (1.0 - lb), 2.0 * (1.0 - lr));
        // RGB → Y, Cb, Cr.
        let to = [
            [lr, lg, lb],
            [-lr / kb, -lg / kb, (1.0 - lb) / kb],
            [(1.0 - lr) / kr, -lg / kr, -lb / kr],
        ];
        // Y, Cb, Cr → RGB.
        let from = [
            [1.0, 0.0, kr],
            [1.0, -kb * lb / lg, -kr * lr / lg],
            [1.0, kb, 0.0],
        ];
        let (s, h) = (self.saturation, self.hue.to_radians());
        let (cos, sin) = (h.cos() * s, h.sin() * s);
        let turn = [[1.0, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]];
        let mul = |a: [[f32; 3]; 3], b: [[f32; 3]; 3]| {
            let mut m = [[0.0f32; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    m[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
                }
            }
            m
        };
        let m = mul(from, mul(turn, to));
        let c = self.contrast;
        let off = 0.5 - 0.5 * c + self.brightness;
        // Grey (the offset) passes the colour part unchanged: each of its rows sums to 1.
        let mut out = [[0.0f32; 4]; 3];
        for i in 0..3 {
            for j in 0..3 {
                out[i][j] = m[i][j] * c;
            }
            out[i][3] = off;
        }
        out
    }

    /// The uniform: three rows and (1 / gamma, on, 0, 0).
    pub fn uniform(&self) -> ([f32; 12], [f32; 4]) {
        let r = self.rows();
        let mut flat = [0.0; 12];
        for (i, row) in r.iter().enumerate() {
            flat[i * 4..i * 4 + 4].copy_from_slice(row);
        }
        (flat, [1.0 / self.gamma.max(1e-3), 1.0, 0.0, 0.0])
    }

    /// An sRGB-encoded colour as shown (the shader's reference). Nothing goes below black;
    /// values above white stay (for HDR displays).
    pub fn apply(&self, e: [f32; 3]) -> [f32; 3] {
        let r = self.rows();
        let g = 1.0 / self.gamma.max(1e-3);
        let mut out = [0.0; 3];
        for (i, row) in r.iter().enumerate() {
            let v = row[0] * e[0] + row[1] * e[1] + row[2] * e[2] + row[3];
            out[i] = v.max(0.0).powf(g);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
    }

    #[test]
    fn neutral_changes_nothing() {
        let p = PictureAdjust::NEUTRAL;
        assert!(p.is_neutral());
        for c in [[0.0, 0.0, 0.0], [1.0, 0.5, 0.25], [0.3, 0.9, 0.1]] {
            assert!(close(p.apply(c), c), "{c:?}");
        }
    }

    #[test]
    fn brightness_and_contrast() {
        let b = PictureAdjust {
            brightness: 0.1,
            ..PictureAdjust::NEUTRAL
        };
        assert!(close(b.apply([0.2, 0.4, 0.6]), [0.3, 0.5, 0.7]));
        let c = PictureAdjust {
            contrast: 2.0,
            ..PictureAdjust::NEUTRAL
        };
        // Middle grey stays; the rest moves away from it.
        assert!(close(c.apply([0.5; 3]), [0.5; 3]));
        assert!(close(c.apply([0.25, 0.75, 0.5]), [0.0, 1.0, 0.5]));
    }

    #[test]
    fn saturation_zero_is_grey_and_keeps_luma() {
        let p = PictureAdjust {
            saturation: 0.0,
            ..PictureAdjust::NEUTRAL
        };
        let c = [0.8, 0.3, 0.1];
        let y = LUMA[0] * c[0] + LUMA[1] * c[1] + LUMA[2] * c[2];
        assert!(close(p.apply(c), [y; 3]));
    }

    #[test]
    fn hue_keeps_grey_and_luma_and_turns_back() {
        for deg in [30.0, 90.0, -120.0, 180.0] {
            let p = PictureAdjust {
                hue: deg,
                ..PictureAdjust::NEUTRAL
            };
            assert!(close(p.apply([0.4; 3]), [0.4; 3]), "{deg}");
            let c = [0.55, 0.4, 0.45];
            let o = p.apply(c);
            let y = |c: [f32; 3]| LUMA[0] * c[0] + LUMA[1] * c[1] + LUMA[2] * c[2];
            assert!(
                (y(o) - y(c)).abs() < 1e-3,
                "{deg}: luma {} vs {}",
                y(o),
                y(c)
            );
            let back = PictureAdjust {
                hue: -deg,
                ..PictureAdjust::NEUTRAL
            };
            assert!(close(back.apply(o), c), "{deg}: {:?}", back.apply(o));
        }
    }

    #[test]
    fn hue_turns_red_towards_green_then_blue() {
        let p = PictureAdjust {
            hue: 120.0,
            ..PictureAdjust::NEUTRAL
        };
        // A third of a turn: a red's chroma lands on the green side.
        let o = p.apply([0.8, 0.4, 0.4]);
        assert!(o[1] > o[0] && o[1] > o[2], "{o:?}");
        let o = p.apply(o);
        assert!(o[2] > o[0] && o[2] > o[1], "{o:?}");
    }

    #[test]
    fn gamma_above_one_lightens_and_keeps_ends() {
        let p = PictureAdjust {
            gamma: 2.0,
            ..PictureAdjust::NEUTRAL
        };
        let o = p.apply([0.0, 0.25, 1.0]);
        assert!(close(o, [0.0, 0.5, 1.0]), "{o:?}");
    }
}
