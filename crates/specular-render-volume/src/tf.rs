//! Transfer functions for volume rendering: modality value → linear-light colour and
//! extinction (opacity per mm), as a 1D lookup table the shader reads.
//!
//! Each [`Preset`] is a few control points over a nominal value range; the user's window
//! (centre, width) moves and stretches that range over the data, so window/level shifts the
//! whole function the way it shifts a greyscale window.

/// Entries in the lookup table.
pub const LUT_SIZE: usize = 1024;

/// A transfer function preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Preset {
    /// Bone surfaces (CT).
    #[default]
    CtBone,
    /// Skin, fat and muscle with bone (CT).
    CtSoft,
    /// Lung parenchyma and airways (CT).
    CtLung,
    /// Contrast-filled vessels with bone (CT angiography).
    CtAngio,
    /// A neutral ramp over the series' own window (MR and other modalities).
    Mr,
}

/// One control point: value, sRGB colour, extinction per mm.
type Point = (f64, [f32; 3], f32);

const BONE: [Point; 5] = [
    (150.0, [0.62, 0.32, 0.22], 0.0),
    (250.0, [0.82, 0.62, 0.47], 0.4),
    (400.0, [0.93, 0.85, 0.72], 3.0),
    (1000.0, [1.0, 0.97, 0.9], 3.0),
    (1500.0, [1.0, 1.0, 1.0], 3.0),
];

const SOFT: [Point; 7] = [
    (-600.0, [0.75, 0.55, 0.45], 0.0),
    (-300.0, [0.9, 0.7, 0.6], 0.06),
    (-80.0, [0.92, 0.8, 0.52], 0.04),
    (20.0, [0.8, 0.32, 0.26], 0.08),
    (100.0, [0.86, 0.42, 0.32], 0.15),
    (250.0, [0.95, 0.9, 0.8], 1.0),
    (700.0, [1.0, 1.0, 1.0], 2.0),
];

const LUNG: [Point; 5] = [
    (-1000.0, [0.5, 0.4, 0.4], 0.0),
    (-950.0, [0.65, 0.5, 0.5], 0.0),
    (-850.0, [0.9, 0.62, 0.56], 0.1),
    (-500.0, [0.97, 0.78, 0.72], 0.8),
    (-300.0, [1.0, 0.88, 0.82], 0.0),
];

const ANGIO: [Point; 5] = [
    (100.0, [0.55, 0.08, 0.06], 0.0),
    (180.0, [0.78, 0.14, 0.1], 0.6),
    (300.0, [0.95, 0.32, 0.22], 2.0),
    (600.0, [1.0, 0.9, 0.8], 2.0),
    (1200.0, [1.0, 1.0, 1.0], 3.0),
];

/// Positions as fractions of the series' window (0 its lower edge, 1 its upper).
const MR: [Point; 4] = [
    (0.1, [0.3, 0.2, 0.15], 0.0),
    (0.35, [0.75, 0.6, 0.5], 0.05),
    (0.6, [0.9, 0.8, 0.7], 0.3),
    (1.0, [1.0, 1.0, 1.0], 0.6),
];

impl Preset {
    pub const ALL: [Preset; 5] = [
        Preset::CtBone,
        Preset::CtSoft,
        Preset::CtLung,
        Preset::CtAngio,
        Preset::Mr,
    ];

    /// A stable name (also the Fluent selector).
    pub fn id(self) -> &'static str {
        match self {
            Preset::CtBone => "bone",
            Preset::CtSoft => "soft",
            Preset::CtLung => "lung",
            Preset::CtAngio => "angio",
            Preset::Mr => "mr",
        }
    }

    /// The next preset (cycling); CT presets are skipped for data not in Hounsfield units.
    pub fn next(self, hounsfield: bool) -> Preset {
        if !hounsfield {
            return Preset::Mr;
        }
        let i = Preset::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Preset::ALL[(i + 1) % Preset::ALL.len()]
    }

    /// The first preset for a series (bone for Hounsfield units, else the neutral ramp).
    pub fn initial(hounsfield: bool) -> Preset {
        if hounsfield {
            Preset::CtBone
        } else {
            Preset::Mr
        }
    }

    /// The transfer function; `series` is the series' default window (centre, width), which
    /// the [`Preset::Mr`] ramp spans.
    pub fn transfer(self, series: (f64, f64)) -> Transfer {
        let (points, iso): (&[Point], f64) = match self {
            Preset::CtBone => (&BONE, 300.0),
            Preset::CtSoft => (&SOFT, -300.0),
            Preset::CtLung => (&LUNG, -600.0),
            Preset::CtAngio => (&ANGIO, 180.0),
            Preset::Mr => (&MR, 0.4),
        };
        let (c, w) = (series.0, series.1.max(1e-6));
        let place = |x: f64| {
            if self == Preset::Mr {
                c - w / 2.0 + x * w
            } else {
                x
            }
        };
        Transfer {
            preset: self,
            points: points
                .iter()
                .map(|&(x, rgb, s)| (place(x), rgb.map(srgb_to_linear), s))
                .collect(),
            iso: place(iso),
        }
    }
}

/// A transfer function: control points (value, linear colour, extinction per mm) in
/// increasing value, and the value of its main surface (for isosurface export).
#[derive(Clone, Debug, PartialEq)]
pub struct Transfer {
    pub preset: Preset,
    pub points: Vec<(f64, [f32; 3], f32)>,
    pub iso: f64,
}

impl Transfer {
    /// The value range the table spans (the first and last points).
    pub fn range(&self) -> (f64, f64) {
        let lo = self.points.first().map_or(0.0, |p| p.0);
        let hi = self.points.last().map_or(1.0, |p| p.0);
        (lo, hi.max(lo + 1e-6))
    }

    /// The nominal window (centre, width): what the user's window starts at.
    pub fn window(&self) -> (f64, f64) {
        let (lo, hi) = self.range();
        ((lo + hi) / 2.0, hi - lo)
    }

    /// The table: [`LUT_SIZE`] entries evenly over [`Transfer::range`], each linear RGB and
    /// extinction, linear between the points (constant past the ends).
    pub fn lut(&self) -> Vec<[f32; 4]> {
        let (lo, hi) = self.range();
        (0..LUT_SIZE)
            .map(|n| {
                let x = lo + (hi - lo) * n as f64 / (LUT_SIZE - 1) as f64;
                let k = self.points.partition_point(|p| p.0 <= x);
                let (a, b) = match k {
                    0 => (0, 0),
                    k if k >= self.points.len() => (k - 1, k - 1),
                    k => (k - 1, k),
                };
                let (pa, pb) = (self.points[a], self.points[b]);
                let t = if b > a {
                    ((x - pa.0) / (pb.0 - pa.0)) as f32
                } else {
                    0.0
                };
                let c = [0, 1, 2].map(|i| pa.1[i] + (pb.1[i] - pa.1[i]) * t);
                [c[0], c[1], c[2], pa.2 + (pb.2 - pa.2) * t]
            })
            .collect()
    }

    /// The table index of value `v` under the user's `window` (centre, width) as
    /// `v · scale + offset`: the window maps onto the nominal one.
    pub fn mapping(&self, window: (f64, f64)) -> (f64, f64) {
        let (lo, hi) = self.range();
        let (c, w) = self.window();
        let k = w / window.1.max(1e-6);
        let n = (LUT_SIZE - 1) as f64 / (hi - lo);
        (k * n, (c - window.0 * k - lo) * n)
    }

    /// The data value of the main surface under the user's `window`.
    pub fn iso_at(&self, window: (f64, f64)) -> f64 {
        let (c, w) = self.window();
        window.0 + (self.iso - c) * window.1 / w
    }
}

pub(crate) fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The table entry at value `v` (`mapping` from [`Transfer::mapping`]), linear between
/// entries: what the shader computes, in f32 as it does.
pub(crate) fn lookup(lut: &[[f32; 4]], mapping: (f32, f32), v: f32) -> [f32; 4] {
    let n = lut.len() as f32;
    let x = (v * mapping.0 + mapping.1).clamp(0.0, n - 1.0);
    let i0 = x.floor() as usize;
    let i1 = (i0 + 1).min(lut.len() - 1);
    let t = x - x.floor();
    [0, 1, 2, 3].map(|c| lut[i0][c] + (lut[i1][c] - lut[i0][c]) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_shifts_the_function() {
        let t = Preset::CtBone.transfer((40.0, 400.0));
        let lut = t.lut();
        let nominal = t.mapping(t.window());
        let m = (nominal.0 as f32, nominal.1 as f32);
        // At the nominal window, the table's ends sit at the first and last points.
        assert!((lookup(&lut, m, 150.0)[3]).abs() < 1e-6);
        assert!((lookup(&lut, m, 400.0)[3] - 3.0).abs() < 0.01);
        // Raising the level by 100 moves every feature up by 100.
        let (c, w) = t.window();
        let up = t.mapping((c + 100.0, w));
        let u = (up.0 as f32, up.1 as f32);
        for v in [200.0, 250.0, 330.0, 900.0] {
            let a = lookup(&lut, m, v);
            let b = lookup(&lut, u, v + 100.0);
            assert!((a[3] - b[3]).abs() < 1e-3, "{v}: {a:?} {b:?}");
        }
        assert!((t.iso_at((c + 100.0, w)) - 400.0).abs() < 1e-9);
        // Halving the width doubles the slope about the centre.
        assert!((t.iso_at((c, w / 2.0)) - (c + (300.0 - c) / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn mr_spans_the_series_window() {
        let t = Preset::Mr.transfer((500.0, 1000.0));
        assert_eq!(t.range(), (100.0, 1000.0));
        assert!((t.iso - 400.0).abs() < 1e-9);
        assert_eq!(Preset::CtLung.next(false), Preset::Mr);
        assert_eq!(Preset::Mr.next(true), Preset::CtBone);
    }
}
