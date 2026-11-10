//! Colour-vision-deficiency simulation (DESIGN.md §6.2): how the whole view looks to someone
//! with a dichromacy or with no colour vision, so a figure or a false-colour choice can be
//! checked before it is shared. It is a final display-pass option (the compositor applies
//! [`Cvd::matrix`] to the frame in linear light); it never touches the file or an export.
//!
//! The dichromat matrices are Machado, Oliveira and Fernandes, "A Physiologically-based Model
//! for Simulation of Color Vision Deficiency" (IEEE TVCG 15(6), 2009), severity 1.0, which act
//! on linear RGB with sRGB / BT.709 primaries. Achromatopsia (rod monochromacy) is modelled as
//! luminance only: every channel becomes the BT.709 relative luminance.
//!
//! The tests cross-check the protan and deutan matrices against the independent cone-space
//! construction of Viénot, Brettel and Mollon, "Digital video colourmaps for checking the
//! legibility of displays by dichromats" (Color Res. Appl. 24(4), 1999): a colour difference
//! along the missing cone's axis must all but vanish.

/// A simulated colour-vision deficiency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Cvd {
    #[default]
    Off,
    /// No L (long-wavelength, "red") cones.
    Protanopia,
    /// No M (medium-wavelength, "green") cones.
    Deuteranopia,
    /// No S (short-wavelength, "blue") cones.
    Tritanopia,
    /// No cone vision: luminance only.
    Achromatopsia,
}

/// Machado et al. 2009, protanopia, severity 1.0 (linear RGB, rows).
pub const PROTANOPIA: [[f32; 3]; 3] = [
    [0.152286, 1.052583, -0.204868],
    [0.114503, 0.786281, 0.099216],
    [-0.003882, -0.048116, 1.051998],
];

/// Machado et al. 2009, deuteranopia, severity 1.0.
pub const DEUTERANOPIA: [[f32; 3]; 3] = [
    [0.367322, 0.860646, -0.227968],
    [0.280085, 0.672501, 0.047413],
    [-0.011820, 0.042940, 0.968881],
];

/// Machado et al. 2009, tritanopia, severity 1.0.
pub const TRITANOPIA: [[f32; 3]; 3] = [
    [1.255528, -0.076749, -0.178779],
    [-0.078411, 0.930809, 0.147602],
    [0.004733, 0.691367, 0.303900],
];

/// BT.709 relative luminance in every channel.
pub const ACHROMATOPSIA: [[f32; 3]; 3] = [
    [0.2126, 0.7152, 0.0722],
    [0.2126, 0.7152, 0.0722],
    [0.2126, 0.7152, 0.0722],
];

impl Cvd {
    pub const ALL: [Cvd; 5] = [
        Self::Off,
        Self::Protanopia,
        Self::Deuteranopia,
        Self::Tritanopia,
        Self::Achromatopsia,
    ];

    /// Stable identifier (settings, `--show`).
    pub fn id(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Protanopia => "protanopia",
            Self::Deuteranopia => "deuteranopia",
            Self::Tritanopia => "tritanopia",
            Self::Achromatopsia => "achromatopsia",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.id() == id)
    }

    /// The linear-RGB matrix (rows) the display pass applies; `None` when off.
    pub fn matrix(self) -> Option<[[f32; 3]; 3]> {
        match self {
            Self::Off => None,
            Self::Protanopia => Some(PROTANOPIA),
            Self::Deuteranopia => Some(DEUTERANOPIA),
            Self::Tritanopia => Some(TRITANOPIA),
            Self::Achromatopsia => Some(ACHROMATOPSIA),
        }
    }

    /// The simulation turned on with `self` already on goes off; any other turns it on.
    pub fn toggle(self, to: Cvd) -> Cvd {
        if self == to { Self::Off } else { to }
    }

    /// The next simulation (wrapping through off).
    pub fn next(self) -> Cvd {
        let i = Self::ALL.iter().position(|&c| c == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// Reference for the shader: `rgb` (linear) as simulated.
    pub fn apply(self, rgb: [f32; 3]) -> [f32; 3] {
        match self.matrix() {
            None => rgb,
            Some(m) => m.map(|r| r[0] * rgb[0] + r[1] * rgb[1] + r[2] * rgb[2]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrices_are_the_published_values() {
        // Machado et al. 2009, Table "severity 1.0" for each dichromacy.
        assert_eq!(
            Cvd::Protanopia.matrix().unwrap()[0],
            [0.152286, 1.052583, -0.204868]
        );
        assert_eq!(
            Cvd::Protanopia.matrix().unwrap()[2],
            [-0.003882, -0.048116, 1.051998]
        );
        assert_eq!(
            Cvd::Deuteranopia.matrix().unwrap()[1],
            [0.280085, 0.672501, 0.047413]
        );
        assert_eq!(
            Cvd::Tritanopia.matrix().unwrap()[2],
            [0.004733, 0.691367, 0.303900]
        );
        assert_eq!(
            Cvd::Achromatopsia.matrix().unwrap()[1],
            [0.2126, 0.7152, 0.0722]
        );
        assert_eq!(Cvd::Off.matrix(), None);
    }

    #[test]
    fn neutrals_stay_neutral() {
        // Every row sums to 1 (to the tables' six digits): white and greys are unchanged.
        for c in Cvd::ALL {
            for g in [0.0, 0.18, 1.0] {
                let out = c.apply([g, g, g]);
                for v in out {
                    assert!((v - g).abs() < 4e-6, "{c:?} {g}: {out:?}");
                }
            }
        }
    }

    #[test]
    fn achromatopsia_is_luminance() {
        let out = Cvd::Achromatopsia.apply([1.0, 0.0, 0.0]);
        assert_eq!(out, [0.2126; 3]);
        let out = Cvd::Achromatopsia.apply([0.2, 0.5, 0.9]);
        let y = crate::luma([0.2, 0.5, 0.9]);
        assert!(out.iter().all(|v| (v - y).abs() < 1e-6));
    }

    /// Viénot, Brettel and Mollon 1999: linear RGB → LMS (Smith–Pokorny cone fundamentals).
    const RGB_TO_LMS: [[f64; 3]; 3] = [
        [17.8824, 43.5161, 4.11935],
        [3.45565, 27.1554, 3.86714],
        [0.0299566, 0.184309, 1.46709],
    ];

    fn inverse(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
        let [[a, b, c], [d, e, f], [g, h, i]] = m;
        let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
        [
            [
                (e * i - f * h) / det,
                (c * h - b * i) / det,
                (b * f - c * e) / det,
            ],
            [
                (f * g - d * i) / det,
                (a * i - c * g) / det,
                (c * d - a * f) / det,
            ],
            [
                (d * h - e * g) / det,
                (b * g - a * h) / det,
                (a * e - b * d) / det,
            ],
        ]
    }

    /// |M · d| for the unit RGB direction `d` that changes only cone `k`.
    fn response(m: [[f32; 3]; 3], k: usize) -> f64 {
        let inv = inverse(RGB_TO_LMS);
        let d = [inv[0][k], inv[1][k], inv[2][k]];
        let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let d = d.map(|v| v / n);
        let o = m.map(|r| r[0] as f64 * d[0] + r[1] as f64 * d[1] + r[2] as f64 * d[2]);
        (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt()
    }

    #[test]
    fn the_missing_cone_is_invisible() {
        // A difference only the missing cone sees all but vanishes; differences the remaining
        // cones see survive. Thresholds hold for the 2009 matrices against the 1999 cone
        // space (protan 0.025, deutan 0.005, tritan 0.19 against ≥ 0.32 / ≥ 1.27 for the
        // others).
        let p = Cvd::Protanopia.matrix().unwrap();
        assert!(response(p, 0) < 0.03);
        assert!(response(p, 1) > 0.3 && response(p, 2) > 0.9);
        let d = Cvd::Deuteranopia.matrix().unwrap();
        assert!(response(d, 1) < 0.01);
        assert!(response(d, 0) > 0.3 && response(d, 2) > 0.9);
        let t = Cvd::Tritanopia.matrix().unwrap();
        assert!(response(t, 2) < 0.2);
        assert!(response(t, 0) > 1.2 && response(t, 1) > 1.2);
    }

    #[test]
    fn red_and_green_merge_for_red_green_dichromats() {
        // A saturated red and a green of similar luminance: distinct normally, close under
        // protanopia and deuteranopia, still distinct under tritanopia.
        let red = [0.6, 0.05, 0.05];
        let green = [0.12, 0.2, 0.05];
        let dist = |c: Cvd| {
            let (a, b) = (c.apply(red), c.apply(green));
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        };
        assert!(dist(Cvd::Off) > 0.45);
        assert!(dist(Cvd::Protanopia) < 0.25 * dist(Cvd::Off));
        assert!(dist(Cvd::Deuteranopia) < 0.15 * dist(Cvd::Off));
        assert!(dist(Cvd::Tritanopia) > 0.8 * dist(Cvd::Off));
    }

    #[test]
    fn toggling_and_cycling() {
        assert_eq!(Cvd::Off.toggle(Cvd::Tritanopia), Cvd::Tritanopia);
        assert_eq!(Cvd::Tritanopia.toggle(Cvd::Tritanopia), Cvd::Off);
        assert_eq!(Cvd::Protanopia.toggle(Cvd::Deuteranopia), Cvd::Deuteranopia);
        assert_eq!(Cvd::Achromatopsia.next(), Cvd::Off);
        for c in Cvd::ALL {
            assert_eq!(Cvd::from_id(c.id()), Some(c));
        }
    }
}
