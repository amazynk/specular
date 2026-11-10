//! False-colour maps (DESIGN.md §6.2): perceptually uniform, colour-vision-deficiency-safe
//! maps for greyscale data. Viridis is the default; cividis and Crameri's batlow, roma, vik,
//! oslo and lajolla follow, then Thyng's cmocean maps for the ocean and atmosphere (thermal to
//! phase); jet stays available for users who need it but is never chosen
//! for them.
//!
//! The tables are 256 sRGB-encoded entries each, generated from the published data by
//! `tools/gen-colormaps.py` (sources and licences there and in packaging/THIRD-PARTY.md).
//! The GPU gets them once as a storage buffer ([`ColourMap::gpu_table`]); the shaders
//! interpolate neighbouring entries in sRGB and decode to linear light, as [`ColourMap::sample`]
//! does here.

mod tables;

/// A false-colour map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColourMap {
    /// Sequential, dark blue → yellow (matplotlib's default since 2.0).
    #[default]
    Viridis,
    /// Sequential, optimised for colour-vision deficiency (Nuñez et al. 2018).
    Cividis,
    /// Crameri: sequential, dark blue → pink, the Scientific colour maps' all-rounder.
    Batlow,
    /// Crameri: diverging, red-brown → blue.
    Roma,
    /// Crameri: diverging, dark blue → white → dark red.
    Vik,
    /// Crameri: sequential, black → blue → white.
    Oslo,
    /// Crameri: sequential, black-brown → red → pale yellow.
    Lajolla,
    /// cmocean: sequential, black-blue → purple → yellow (temperature).
    Thermal,
    /// cmocean: sequential, dark blue → green → yellow-white (salinity).
    Haline,
    /// cmocean: sequential, dark brown → yellow (irradiance).
    Solar,
    /// cmocean: sequential, black-blue → white (sea ice).
    Ice,
    /// cmocean: sequential, pale yellow → dark blue (depth).
    Deep,
    /// cmocean: sequential, white → dark purple (density).
    Dense,
    /// cmocean: sequential, pale → dark green (chlorophyll).
    Algae,
    /// cmocean: sequential, pale brown → dark blue (precipitation).
    Rain,
    /// cmocean: sequential, pale yellow → dark green (speed).
    Speed,
    /// cmocean: diverging, dark blue → white → dark red.
    Balance,
    /// cmocean: diverging, dark green → white → dark red (vorticity).
    Curl,
    /// cmocean: cyclic (angles, phase, direction).
    Phase,
    /// The rainbow map of old; not perceptually uniform, never the default.
    Jet,
}

/// Entries per table.
pub const MAP_LEN: usize = 256;

impl ColourMap {
    /// Every map, in the order the picker lists them and `next` cycles.
    pub const ALL: [ColourMap; 20] = [
        Self::Viridis,
        Self::Cividis,
        Self::Batlow,
        Self::Roma,
        Self::Vik,
        Self::Oslo,
        Self::Lajolla,
        Self::Thermal,
        Self::Haline,
        Self::Solar,
        Self::Ice,
        Self::Deep,
        Self::Dense,
        Self::Algae,
        Self::Rain,
        Self::Speed,
        Self::Balance,
        Self::Curl,
        Self::Phase,
        Self::Jet,
    ];

    /// Stable identifier (settings, `--show`).
    pub fn id(self) -> &'static str {
        match self {
            Self::Viridis => "viridis",
            Self::Cividis => "cividis",
            Self::Batlow => "batlow",
            Self::Roma => "roma",
            Self::Vik => "vik",
            Self::Oslo => "oslo",
            Self::Lajolla => "lajolla",
            Self::Thermal => "thermal",
            Self::Haline => "haline",
            Self::Solar => "solar",
            Self::Ice => "ice",
            Self::Deep => "deep",
            Self::Dense => "dense",
            Self::Algae => "algae",
            Self::Rain => "rain",
            Self::Speed => "speed",
            Self::Balance => "balance",
            Self::Curl => "curl",
            Self::Phase => "phase",
            Self::Jet => "jet",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }

    /// The map's display name. Map names are proper names and are not translated.
    pub fn name(self) -> &'static str {
        match self {
            Self::Viridis => "viridis",
            Self::Cividis => "cividis",
            Self::Batlow => "batlow",
            Self::Roma => "roma",
            Self::Vik => "vik",
            Self::Oslo => "oslo",
            Self::Lajolla => "lajolla",
            Self::Thermal => "thermal",
            Self::Haline => "haline",
            Self::Solar => "solar",
            Self::Ice => "ice",
            Self::Deep => "deep",
            Self::Dense => "dense",
            Self::Algae => "algae",
            Self::Rain => "rain",
            Self::Speed => "speed",
            Self::Balance => "balance",
            Self::Curl => "curl",
            Self::Phase => "phase",
            Self::Jet => "jet",
        }
    }

    /// Position in [`ColourMap::ALL`] and in [`ColourMap::gpu_table`] (the shader's map index).
    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|&m| m == self).unwrap_or(0) as u32
    }

    /// The next map (wrapping).
    pub fn next(self) -> Self {
        Self::ALL[(self.index() as usize + 1) % Self::ALL.len()]
    }

    /// Diverging maps have a meaningful centre (the legend marks it).
    pub fn is_diverging(self) -> bool {
        matches!(self, Self::Roma | Self::Vik | Self::Balance | Self::Curl)
    }

    /// The 256 sRGB-encoded entries.
    pub fn table(self) -> &'static [[f32; 3]; MAP_LEN] {
        match self {
            Self::Viridis => &tables::VIRIDIS,
            Self::Cividis => &tables::CIVIDIS,
            Self::Batlow => &tables::BATLOW,
            Self::Roma => &tables::ROMA,
            Self::Vik => &tables::VIK,
            Self::Oslo => &tables::OSLO,
            Self::Lajolla => &tables::LAJOLLA,
            Self::Thermal => &tables::THERMAL,
            Self::Haline => &tables::HALINE,
            Self::Solar => &tables::SOLAR,
            Self::Ice => &tables::ICE,
            Self::Deep => &tables::DEEP,
            Self::Dense => &tables::DENSE,
            Self::Algae => &tables::ALGAE,
            Self::Rain => &tables::RAIN,
            Self::Speed => &tables::SPEED,
            Self::Balance => &tables::BALANCE,
            Self::Curl => &tables::CURL,
            Self::Phase => &tables::PHASE,
            Self::Jet => &tables::JET,
        }
    }

    /// The map at `t` (0‥1, clamped; NaN reads as 0), sRGB-encoded: the two nearest entries
    /// interpolated linearly. The shaders' `cmap` follows the same rule.
    pub fn sample(self, t: f32) -> [f32; 3] {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        let x = t * (MAP_LEN - 1) as f32;
        let i = (x.floor() as usize).min(MAP_LEN - 2);
        let f = x - i as f32;
        let (a, b) = (self.table()[i], self.table()[i + 1]);
        [
            a[0] + (b[0] - a[0]) * f,
            a[1] + (b[1] - a[1]) * f,
            a[2] + (b[2] - a[2]) * f,
        ]
    }

    /// The map at `t` in linear light (what the shaders write).
    pub fn sample_linear(self, t: f32) -> [f32; 3] {
        self.sample(t).map(crate::srgb_decode)
    }

    /// Every table, in [`ColourMap::ALL`] order, as `vec4<f32>` entries (alpha 1): the
    /// shaders' storage buffer. Map `m` entry `i` is at `m * MAP_LEN + i`.
    pub fn gpu_table() -> Vec<[f32; 4]> {
        Self::ALL
            .iter()
            .flat_map(|m| m.table().iter().map(|c| [c[0], c[1], c[2], 1.0]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{luma, srgb_decode};

    #[test]
    fn published_anchors_are_exact() {
        // matplotlib _cm_listed.py, first and last entries.
        assert_eq!(
            ColourMap::Viridis.table()[0],
            [0.267004, 0.004874, 0.329415]
        );
        assert_eq!(
            ColourMap::Viridis.table()[128],
            [0.127568, 0.566949, 0.550556]
        );
        assert_eq!(
            ColourMap::Viridis.table()[255],
            [0.993248, 0.906157, 0.143936]
        );
        assert_eq!(ColourMap::Cividis.table()[0], [0.0, 0.135112, 0.304751]);
        assert_eq!(
            ColourMap::Cividis.table()[255],
            [0.995737, 0.909344, 0.217772]
        );
        // Scientific colour maps 8.0 (cmcrameri 1.10), first and last lines.
        assert_eq!(ColourMap::Batlow.table()[0], [0.005193, 0.098238, 0.349842]);
        assert_eq!(
            ColourMap::Batlow.table()[255],
            [0.981354, 0.800406, 0.981267]
        );
        assert_eq!(ColourMap::Roma.table()[0], [0.492325, 0.090787, 0.000076]);
        assert_eq!(ColourMap::Roma.table()[255], [0.009824, 0.192129, 0.596704]);
        assert_eq!(ColourMap::Vik.table()[0], [0.001328, 0.069836, 0.379529]);
        assert_eq!(ColourMap::Vik.table()[255], [0.350423, 0.000061, 0.030499]);
        assert_eq!(ColourMap::Oslo.table()[0], [0.003670, 0.005082, 0.002454]);
        assert_eq!(ColourMap::Oslo.table()[255], [0.999801, 1.0, 0.999961]);
        assert_eq!(
            ColourMap::Lajolla.table()[0],
            [0.098791, 0.099669, 0.000088]
        );
        assert_eq!(ColourMap::Lajolla.table()[255], [1.0, 0.997796, 0.794247]);
        // cmocean 4.0.3, rounded to six decimals; curl resampled from 512 rows.
        assert_eq!(
            ColourMap::Thermal.table()[0],
            [0.015556, 0.138244, 0.201811]
        );
        assert_eq!(ColourMap::Curl.table()[0], [0.082256, 0.114924, 0.264790]);
        // Jet from its piecewise definition: dark blue, cyan-ish centre, dark red.
        assert_eq!(ColourMap::Jet.table()[0], [0.0, 0.0, 0.5]);
        assert_eq!(ColourMap::Jet.table()[255], [0.5, 0.0, 0.0]);
    }

    #[test]
    fn sampling_hits_entries_and_interpolates_between() {
        let m = ColourMap::Viridis;
        assert_eq!(m.sample(0.0), m.table()[0]);
        assert_eq!(m.sample(1.0), m.table()[255]);
        assert_eq!(m.sample(-3.0), m.table()[0]);
        assert_eq!(m.sample(f32::NAN), m.table()[0]);
        assert_eq!(m.sample(9.0), m.table()[255]);
        let t = 100.0 / 255.0;
        for c in 0..3 {
            assert!((m.sample(t)[c] - m.table()[100][c]).abs() < 1e-5);
        }
        let mid = m.sample(100.5 / 255.0);
        for (c, got) in mid.iter().enumerate() {
            let want = (m.table()[100][c] + m.table()[101][c]) / 2.0;
            assert!((got - want).abs() < 1e-5);
        }
    }

    #[test]
    fn ids_round_trip_and_viridis_is_default() {
        assert_eq!(ColourMap::default(), ColourMap::Viridis);
        for m in ColourMap::ALL {
            assert_eq!(ColourMap::from_id(m.id()), Some(m));
            assert_eq!(ColourMap::ALL[m.index() as usize], m);
        }
        assert_eq!(ColourMap::Jet.next(), ColourMap::Viridis);
        assert_eq!(ColourMap::from_id("rainbow"), None);
        let g = ColourMap::gpu_table();
        assert_eq!(g.len(), ColourMap::ALL.len() * MAP_LEN);
        let b = ColourMap::Batlow.index() as usize * MAP_LEN;
        assert_eq!(&g[b][..3], &ColourMap::Batlow.table()[0][..]);
    }

    #[test]
    fn sequential_maps_rise_in_lightness() {
        // The CVD-safe sequential maps read in greyscale: luminance rises monotonically
        // (within the tables' 6-digit rounding).
        for m in [
            ColourMap::Viridis,
            ColourMap::Cividis,
            ColourMap::Batlow,
            ColourMap::Oslo,
            ColourMap::Lajolla,
            ColourMap::Thermal,
            ColourMap::Haline,
            ColourMap::Solar,
            ColourMap::Ice,
        ] {
            let mut last = -1.0;
            for (i, c) in m.table().iter().enumerate() {
                let l = luma(c.map(srgb_decode));
                assert!(l >= last - 2e-4, "{} entry {i}: {l} < {last}", m.id());
                last = l;
            }
        }
        // cmocean's light-to-dark maps fall instead.
        for m in [
            ColourMap::Deep,
            ColourMap::Dense,
            ColourMap::Algae,
            ColourMap::Rain,
            ColourMap::Speed,
        ] {
            let mut last = 2.0;
            for (i, c) in m.table().iter().enumerate() {
                let l = luma(c.map(srgb_decode));
                assert!(l <= last + 2e-4, "{} entry {i}: {l} > {last}", m.id());
                last = l;
            }
        }
        // phase is cyclic: its ends meet.
        let p = ColourMap::Phase.table();
        assert!((0..3).all(|c| (p[0][c] - p[255][c]).abs() < 0.02), "{p:?}");
        assert!(ColourMap::Vik.is_diverging() && !ColourMap::Viridis.is_diverging());
    }
}
