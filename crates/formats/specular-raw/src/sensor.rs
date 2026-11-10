//! Sensor data for a scene-referred develop (Specular's Develop mode): the samples
//! as stored, unclipped, with everything needed to turn them into camera RGB kept apart as
//! metadata. Nothing here changes what [`crate::develop`] (the viewer's path) outputs.
//!
//! [`crate::RawFile::decode_sensor`] returns a [`SensorData`]:
//!
//! - **Samples** exactly as stored (the values LibRaw calls `raw_image`), whole sensor,
//!   before linearization, black subtraction, white scaling and any DNG opcode list. Vendor
//!   calibrations that are part of decoding (Phase One black tables and flat fields, Sigma
//!   lens shading) are applied, as in [`crate::RawFile::decode_raw`].
//! - **Black levels** per repeat position and channel, plus DNG per-row and per-column deltas
//!   ([`BlackLevels`]); the white level; the DNG linearization table.
//! - **CFA layout**: 2x2 Bayer, a larger RGB pattern (X-Trans), linear RGB or monochrome.
//! - **DNG opcode lists** 1, 2 and 3 as plain data ([`OpcodeInfo`]).
//! - **Colour**: the as-shot white balance, the camera matrix (white-balanced camera RGB →
//!   linear Rec.709), the DNG matrices as stored, and the DNG camera profile with its
//!   HueSatMap and LookTable when present.
//!
//! Normalised camera RGB is `(linearize(v) - black(x, y, c)) / (white - black(x, y, c))`,
//! never clipped: values above the white level stay above 1 ([`SensorData::normalised`] is
//! the plain CPU form of it).

use std::sync::Arc;

use rayon::prelude::*;

use crate::container::{ColorSource, Sensor};
use crate::develop::DevelopStats;
use crate::error::{RawError, Result};
use crate::{Cfa, MatrixSource, ProfileTransform, RawImage, Rect, WbSource};

pub use crate::makernote::{LensCorrection, LensMaker};
pub use crate::opcode::{OpcodeInfo, PointKind, lists_info as opcode_lists_info};
pub use crate::profile::HsvTableData;

/// How colours are laid out on the sensor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    /// A 2x2 Bayer pattern: colour (0 red, 1 green, 2 blue) at stored position
    /// `(y & 1) * 2 + (x & 1)`.
    Bayer([u8; 4]),
    /// A larger red/green/blue pattern whose every 3x3 window holds all three colours
    /// (Fujifilm X-Trans), anchored at the stored image's top-left.
    Pattern(Cfa),
    /// Three samples per pixel (DNG LinearRaw, Foveon, patterns demosaiced on decode).
    Linear,
    /// One sample per pixel without colour filters.
    Mono,
}

impl Layout {
    /// The colour at stored position (x, y); 1 (green) for linear and monochrome data.
    pub fn color(&self, x: u32, y: u32) -> u8 {
        match self {
            Layout::Bayer(p) => p[((y & 1) * 2 + (x & 1)) as usize],
            Layout::Pattern(c) => c.color(y, x),
            Layout::Linear | Layout::Mono => 1,
        }
    }
}

/// Black levels as the file gives them: a repeating pattern of levels per channel, plus
/// per-column and per-row deltas (DNG `BlackLevelRepeatDim`, `BlackLevel`,
/// `BlackLevelDeltaH`, `BlackLevelDeltaV`), in linearized units.
#[derive(Clone, Debug, PartialEq)]
pub struct BlackLevels {
    /// Pattern rows and columns.
    pub repeat: (u32, u32),
    /// Samples per pixel the levels are given for.
    pub cpp: u32,
    /// `levels[(row * cols + col) * cpp + channel]`.
    pub levels: Vec<f32>,
    /// Stored position (x, y) the pattern and the deltas start at (the DNG active area).
    pub origin: (u32, u32),
    /// Added per column from `origin.0` (empty: none).
    pub per_column: Vec<f32>,
    /// Added per row from `origin.1` (empty: none).
    pub per_row: Vec<f32>,
}

impl BlackLevels {
    /// Levels per 2x2 position (CFA) or per channel (linear data), no deltas.
    pub fn simple(black: [f32; 4], cpp: u32) -> BlackLevels {
        if cpp == 1 {
            BlackLevels {
                repeat: (2, 2),
                cpp: 1,
                levels: black.to_vec(),
                origin: (0, 0),
                per_column: Vec::new(),
                per_row: Vec::new(),
            }
        } else {
            BlackLevels {
                repeat: (1, 1),
                cpp,
                levels: (0..cpp as usize).map(|c| black[c.min(3)]).collect(),
                origin: (0, 0),
                per_column: Vec::new(),
                per_row: Vec::new(),
            }
        }
    }

    /// The black level of channel `c` at stored position (x, y).
    pub fn at(&self, x: u32, y: u32, c: u32) -> f32 {
        let (rows, cols) = (self.repeat.0.max(1), self.repeat.1.max(1));
        let (ox, oy) = self.origin;
        // The pattern repeats in both directions from the origin.
        let r = (y as i64 - oy as i64).rem_euclid(rows as i64) as u32;
        let k = (x as i64 - ox as i64).rem_euclid(cols as i64) as u32;
        let cpp = self.cpp.max(1);
        let i = ((r * cols + k) * cpp + c.min(cpp - 1)) as usize;
        let mut b = self.levels.get(i).copied().unwrap_or(0.0);
        if x >= ox
            && let Some(d) = self.per_column.get((x - ox) as usize)
        {
            b += d;
        }
        if y >= oy
            && let Some(d) = self.per_row.get((y - oy) as usize)
        {
            b += d;
        }
        b
    }

    /// The largest level anywhere (the deltas included).
    pub fn max(&self) -> f32 {
        let m = |v: &[f32]| v.iter().copied().fold(0f32, f32::max);
        m(&self.levels) + m(&self.per_column) + m(&self.per_row)
    }
}

/// DNG colour tags as stored (row-major 3x3 matrices; index 0 and 1 are the two
/// calibration illuminants).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DngColour {
    pub color_matrix: [Option<[f64; 9]>; 2],
    pub forward_matrix: [Option<[f64; 9]>; 2],
    pub camera_calibration: [Option<[f64; 9]>; 2],
    pub analog_balance: Option<[f64; 3]>,
    /// EXIF light source codes of the two illuminants.
    pub illuminants: [u32; 2],
    pub as_shot_neutral: Option<[f64; 3]>,
    pub as_shot_white_xy: Option<(f64, f64)>,
    /// BaselineExposure plus BaselineExposureOffset, in EV.
    pub baseline_exposure: f64,
    /// NoiseProfile (scale, offset) pairs for signal in 0..1 (variance = scale·x + offset):
    /// one pair for all planes or one per colour plane; empty when absent or invalid.
    pub noise_profile: Vec<[f64; 2]>,
}

/// Everything about the samples except the samples (see the module documentation).
#[derive(Clone, Debug)]
pub struct SensorMeta {
    pub layout: Layout,
    pub black: BlackLevels,
    /// White (saturation) level in linearized units, as the file states it.
    pub white: f32,
    /// Where the data shows it clips (at most `white`; see [`DevelopStats::white`]).
    pub clip_white: f32,
    /// DNG `LinearizationTable`: stored value → linearized value.
    pub linearization: Option<Arc<[u16]>>,
    /// DNG `OpcodeList1`, `OpcodeList2`, `OpcodeList3` as plain data (empty when absent).
    pub opcodes: [Vec<OpcodeInfo>; 3],
    /// Names of opcodes the file marks as required but this reader does not know.
    pub opcodes_unknown: Vec<&'static str>,
    /// Multipliers for R, G, B (G = 1): as shot, or a grey-world estimate when the file has
    /// none ([`SensorMeta::wb_source`]).
    pub wb: [f32; 3],
    pub wb_source: WbSource,
    /// White-balanced camera RGB → linear Rec.709 (D65), rows summing to 1.
    pub rgb_cam: [[f32; 3]; 3],
    pub matrix_source: MatrixSource,
    /// The DNG colour tags, for DNG files.
    pub dng: Option<DngColour>,
    /// The DNG camera profile with its look tables, when the file has one.
    pub profile: Option<Arc<ProfileTransform>>,
    /// EXIF orientation 1..=8.
    pub orientation: u16,
    /// The file's own lens correction tables (Sony and Fujifilm maker data), for Develop's
    /// lens correction; the viewer's develop does not apply them.
    pub lens: Option<Arc<LensCorrection>>,
    /// DNG `DefaultScale` as the stretch to square pixels (each factor at least 1, see
    /// [`crate::SensorInfo::square_size`]); `None` for square pixels.
    pub default_scale: Option<[f32; 2]>,
    /// Some sensor data was missing or corrupt (filled with zeros).
    pub truncated: bool,
}

/// Sensor samples as stored plus their metadata (see the module documentation).
#[derive(Clone, Debug)]
pub struct SensorData {
    /// Stored size in pixels (masked borders included).
    pub width: u32,
    pub height: u32,
    /// Samples per pixel: 1 (CFA or monochrome) or 3 (linear RGB).
    pub cpp: u32,
    /// `width * height * cpp` samples, row-major, as stored. For floating-point DNGs (HDR
    /// merges) the samples scaled to 16 bits and clipped there, as the viewer uses them.
    pub samples: Vec<u16>,
    /// Floating-point DNGs only: the same samples unclipped and at full precision, in the
    /// same units (the white level maps to 65535; values above it are kept). The develop
    /// uses these instead of [`SensorData::samples`] when present.
    pub float: Option<Arc<[f32]>>,
    /// The visible area, in stored coordinates.
    pub crop: Rect,
    pub meta: SensorMeta,
}

impl SensorData {
    /// The sensor binned 2×2 per colour for thumbnails: each sample is the mean of the four
    /// same-colour samples of its 4×4 block, so the Bayer pattern stays where it was at
    /// half the size (a quarter of the samples). A block with a sample at or above the clip
    /// level keeps its largest sample (clipping stays visible). The DNG noise profile is
    /// divided by four (the mean of four). `None` where the data cannot be binned this way:
    /// not Bayer, floating point, a linearization table, DNG opcode lists, black levels
    /// that vary by more than the 2×2 position, or an odd crop origin.
    pub fn half(&self) -> Option<SensorData> {
        let m = &self.meta;
        let b = &m.black;
        if self.cpp != 1
            || !matches!(m.layout, Layout::Bayer(_))
            || self.float.is_some()
            || m.linearization.is_some()
            || m.opcodes.iter().any(|l| !l.is_empty())
            || !matches!(b.repeat, (1 | 2, 1 | 2))
            || !b.per_column.is_empty()
            || !b.per_row.is_empty()
            || !b.origin.0.is_multiple_of(2)
            || !b.origin.1.is_multiple_of(2)
            || !self.crop.x.is_multiple_of(2)
            || !self.crop.y.is_multiple_of(2)
        {
            return None;
        }
        let (w, h) = (self.width as usize, self.height as usize);
        let (hw, hh) = ((w / 4) * 2, (h / 4) * 2);
        if hw < 2 || hh < 2 {
            return None;
        }
        let clip = m.clip_white.min(m.white).max(1.0);
        let s = &self.samples;
        let mut out = vec![0u16; hw * hh];
        let row = |y: usize, out: &mut [u16]| {
            let (py, by) = (y & 1, y >> 1);
            let (y0, y1) = (4 * by + py, 4 * by + py + 2);
            let (r0, r1) = (&s[y0 * w..y0 * w + w], &s[y1 * w..y1 * w + w]);
            for (x, o) in out.iter_mut().enumerate() {
                let (px, bx) = (x & 1, x >> 1);
                let (x0, x1) = (4 * bx + px, 4 * bx + px + 2);
                let v = [r0[x0], r0[x1], r1[x0], r1[x1]];
                let hi = v.iter().copied().max().unwrap_or(0);
                *o = if f32::from(hi) >= clip {
                    hi
                } else {
                    let sum: u32 = v.iter().map(|&q| u32::from(q)).sum();
                    ((sum + 2) / 4) as u16
                };
            }
        };
        out.par_chunks_mut(hw)
            .enumerate()
            .for_each(|(y, o)| row(y, o));
        let mut meta = m.clone();
        meta.black.origin = (0, 0);
        if let Some(d) = meta.dng.as_mut() {
            for p in &mut d.noise_profile {
                p[0] /= 4.0;
                p[1] /= 4.0;
            }
        }
        let cw = (self.crop.w / 2).min(hw as u32 - (self.crop.x / 2).min(hw as u32));
        let ch = (self.crop.h / 2).min(hh as u32 - (self.crop.y / 2).min(hh as u32));
        Some(SensorData {
            width: hw as u32,
            height: hh as u32,
            cpp: 1,
            samples: out,
            float: None,
            crop: Rect {
                x: self.crop.x / 2,
                y: self.crop.y / 2,
                w: cw,
                h: ch,
            },
            meta,
        })
    }

    /// An X-Trans sensor binned 3×3 into linear RGB for thumbnails: every 3×3 block from
    /// the stored top-left holds all three colours (five green, two red, two blue on
    /// Fujifilm's pattern), and each channel is the mean of its samples there, so no
    /// demosaic is needed at a third of the size (a ninth of the pixels). A channel with a
    /// sample at or above the clip level keeps its largest sample. The DNG noise profile
    /// is divided per channel by the fewest samples it averages. `None` where the data
    /// cannot be binned this way: not a pattern whose 3×3 blocks hold every colour,
    /// floating point, a linearization table, DNG opcode lists, or black levels that are
    /// not one level for the whole sensor.
    pub fn third(&self) -> Option<SensorData> {
        let m = &self.meta;
        let b = &m.black;
        let Layout::Pattern(cfa) = &m.layout else {
            return None;
        };
        let uniform = b.levels.windows(2).all(|p| p[0] == p[1]);
        if self.cpp != 1
            || self.float.is_some()
            || m.linearization.is_some()
            || m.opcodes.iter().any(|l| !l.is_empty())
            || !uniform
            || !b.per_column.is_empty()
            || !b.per_row.is_empty()
            || !cfa.width.is_multiple_of(3)
            || !cfa.height.is_multiple_of(3)
        {
            return None;
        }
        // Samples of each colour in every 3×3 block of the pattern.
        let mut fewest = [u32::MAX; 3];
        for by in 0..cfa.height / 3 {
            for bx in 0..cfa.width / 3 {
                let mut n = [0u32; 3];
                for y in 0..3 {
                    for x in 0..3 {
                        let c = usize::from(cfa.color(3 * by + y, 3 * bx + x));
                        n[c.min(2)] += 1;
                    }
                }
                for c in 0..3 {
                    fewest[c] = fewest[c].min(n[c]);
                }
            }
        }
        if fewest.contains(&0) {
            return None;
        }
        let (w, h) = (self.width as usize, self.height as usize);
        let (tw, th) = (w / 3, h / 3);
        if tw < 1 || th < 1 {
            return None;
        }
        let clip = m.clip_white.min(m.white).max(1.0);
        let samples = &self.samples;
        let layout = &m.layout;
        let mut out = vec![0u16; tw * th * 3];
        out.par_chunks_mut(tw * 3)
            .enumerate()
            .for_each(|(by, row)| {
                for (bx, o) in row.chunks_exact_mut(3).enumerate() {
                    let (mut sum, mut n, mut hi) = ([0u32; 3], [0u32; 3], [0u16; 3]);
                    for y in 3 * by..3 * by + 3 {
                        for x in 3 * bx..3 * bx + 3 {
                            let c = usize::from(layout.color(x as u32, y as u32)).min(2);
                            let v = samples[y * w + x];
                            sum[c] += u32::from(v);
                            n[c] += 1;
                            hi[c] = hi[c].max(v);
                        }
                    }
                    for c in 0..3 {
                        o[c] = if f32::from(hi[c]) >= clip {
                            hi[c]
                        } else {
                            ((sum[c] + n[c] / 2) / n[c].max(1)) as u16
                        };
                    }
                }
            });
        let mut meta = m.clone();
        meta.layout = Layout::Linear;
        let level = b.levels.first().copied().unwrap_or(0.0);
        meta.black = BlackLevels {
            repeat: (1, 1),
            cpp: 3,
            levels: vec![level; 3],
            origin: (0, 0),
            per_column: Vec::new(),
            per_row: Vec::new(),
        };
        if let Some(d) = meta.dng.as_mut()
            && let Some(&first) = d.noise_profile.first()
        {
            let p: Vec<[f64; 2]> = (0..3)
                .map(|c| {
                    let q = d.noise_profile.get(c).copied().unwrap_or(first);
                    let k = f64::from(fewest[c]);
                    [q[0] / k, q[1] / k]
                })
                .collect();
            d.noise_profile = p;
        }
        let x0 = self.crop.x.div_ceil(3).min(tw as u32);
        let y0 = self.crop.y.div_ceil(3).min(th as u32);
        let x1 = ((self.crop.x + self.crop.w) / 3).clamp(x0, tw as u32);
        let y1 = ((self.crop.y + self.crop.h) / 3).clamp(y0, th as u32);
        if x1 == x0 || y1 == y0 {
            return None;
        }
        Some(SensorData {
            width: tw as u32,
            height: th as u32,
            cpp: 3,
            samples: out,
            float: None,
            crop: Rect {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            },
            meta,
        })
    }

    /// The sample of channel `c` at stored position (x, y).
    pub fn sample(&self, x: u32, y: u32, c: u32) -> u16 {
        self.samples
            [((y as usize * self.width as usize) + x as usize) * self.cpp as usize + c as usize]
    }

    /// Normalised camera RGB of the visible area (black 0, white 1, unclipped; no opcode
    /// lists, no white balance): `crop.w * crop.h * cpp` values. A plain CPU form for
    /// checks; the develop does this on the GPU.
    pub fn normalised(&self) -> Vec<f32> {
        let m = &self.meta;
        let c = self.crop;
        let mut out = Vec::with_capacity(c.w as usize * c.h as usize * self.cpp as usize);
        for y in c.y..c.y + c.h {
            for x in c.x..c.x + c.w {
                for ch in 0..self.cpp {
                    let v = self.sample(x, y, ch);
                    let l = match &m.linearization {
                        Some(t) => f32::from(t[(v as usize).min(t.len() - 1)]),
                        None => f32::from(v),
                    };
                    let b = m.black.at(x, y, ch);
                    out.push((l - b) / (m.white - b).max(1.0));
                }
            }
        }
        out
    }
}

/// Builds the [`SensorData`] of a decode without opcode lists.
pub(crate) fn build(raw: RawImage, s: &Sensor) -> Result<SensorData> {
    let layout = match (&raw.cfa, raw.cpp) {
        (_, 3) => Layout::Linear,
        (None, 1) => Layout::Mono,
        (Some(cfa), 1) => match cfa.bayer2x2() {
            Some(p) => Layout::Bayer(p),
            None if crate::xtrans::supported(cfa) => Layout::Pattern(cfa.clone()),
            None => return Err(RawError::Unsupported("colour filter pattern")),
        },
        _ => return Err(RawError::Unsupported("samples per pixel")),
    };
    // Monochrome data has no statistics to gather.
    let stats = match layout {
        Layout::Mono => None,
        _ => Some(DevelopStats::of(&raw)?),
    };
    let black = match &s.black_area {
        Some(b) if b.cpp == raw.cpp => (**b).clone(),
        _ => BlackLevels::simple(raw.black, raw.cpp),
    };
    let dng = match &s.color {
        ColorSource::Dng(c) => Some(DngColour {
            color_matrix: c.cm,
            forward_matrix: c.fm,
            camera_calibration: c.cc,
            analog_balance: c.ab,
            illuminants: c.illuminant,
            as_shot_neutral: c.neutral,
            as_shot_white_xy: c.white_xy,
            baseline_exposure: c.exposure,
            noise_profile: c.noise.clone(),
        }),
        _ => None,
    };
    let (opcodes, unknown) = match &s.opcodes {
        Some(o) => ([o.list(0), o.list(1), o.list(2)], o.skipped().to_vec()),
        None => (Default::default(), Vec::new()),
    };
    let (wb, wb_source) = match stats {
        Some(st) => (st.wb, st.wb_source),
        None => ([1.0; 3], WbSource::AsShot),
    };
    Ok(SensorData {
        width: raw.width,
        height: raw.height,
        cpp: raw.cpp,
        crop: raw.crop,
        meta: SensorMeta {
            layout,
            black,
            white: raw.white,
            clip_white: stats.map_or(raw.white, |st| st.white()),
            linearization: raw.linearization.map(Arc::from),
            opcodes,
            opcodes_unknown: unknown,
            wb,
            wb_source,
            rgb_cam: raw.rgb_cam,
            matrix_source: raw.matrix_source,
            dng,
            profile: raw.profile,
            orientation: raw.orientation,
            lens: s.lens.clone(),
            default_scale: s.default_scale,
            truncated: raw.truncated,
        },
        samples: raw.data,
        float: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bayer(w: u32, h: u32, f: impl Fn(u32, u32) -> u16) -> SensorData {
        SensorData {
            width: w,
            height: h,
            cpp: 1,
            samples: (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .map(|(x, y)| f(x, y))
                .collect(),
            float: None,
            crop: Rect {
                x: 2,
                y: 0,
                w: w - 2,
                h,
            },
            meta: SensorMeta {
                layout: Layout::Bayer([0, 1, 1, 2]),
                black: BlackLevels::simple([64.0; 4], 1),
                white: 4000.0,
                clip_white: 4000.0,
                linearization: None,
                opcodes: Default::default(),
                opcodes_unknown: Vec::new(),
                wb: [2.0, 1.0, 1.5],
                wb_source: WbSource::AsShot,
                rgb_cam: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                matrix_source: MatrixSource::Generic,
                dng: None,
                profile: None,
                orientation: 1,
                lens: None,
                default_scale: None,
                truncated: false,
            },
        }
    }

    #[test]
    fn half_bins_each_colour_and_keeps_the_pattern() {
        let s = bayer(10, 8, |x, y| (100 + x * 7 + y * 31) as u16);
        let h = s.half().expect("binnable");
        assert_eq!((h.width, h.height), (4, 4));
        assert_eq!((h.crop.x, h.crop.y, h.crop.w, h.crop.h), (1, 0, 3, 4));
        for y in 0..4u32 {
            for x in 0..4u32 {
                let (px, py, bx, by) = (x & 1, y & 1, x >> 1, y >> 1);
                let xs = [4 * bx + px, 4 * bx + px + 2];
                let ys = [4 * by + py, 4 * by + py + 2];
                let sr = &s;
                let sum: u32 = ys
                    .iter()
                    .flat_map(|&yy| xs.iter().map(move |&xx| u32::from(sr.sample(xx, yy, 0))))
                    .sum();
                assert_eq!(u32::from(h.sample(x, y, 0)), (sum + 2) / 4, "({x}, {y})");
                // Same colour as the sensor's samples it came from.
                assert_eq!(s.meta.layout.color(xs[0], ys[0]), h.meta.layout.color(x, y));
            }
        }
        // A clipped sample keeps the block clipped.
        let c = bayer(8, 8, |x, y| if (x, y) == (2, 2) { 4000 } else { 500 });
        let ch = c.half().unwrap();
        assert_eq!(ch.sample(0, 0, 0), 4000);
        assert_eq!(ch.sample(1, 0, 0), 500);
        // Not binnable: X-Trans-like patterns, opcode lists, odd crops.
        let mut odd = bayer(8, 8, |_, _| 1);
        odd.crop.x = 1;
        assert!(odd.half().is_none());
        let mut lin = bayer(8, 8, |_, _| 1);
        lin.meta.linearization = Some(Arc::from(vec![0u16; 4]));
        assert!(lin.half().is_none());
    }

    #[test]
    fn third_bins_x_trans_into_rgb() {
        // Fujifilm's X-Trans pattern (0 red, 1 green, 2 blue).
        let rows = ["GGRGGB", "GGBGGR", "BRGRBG", "GGBGGR", "GGRGGB", "RBGBRG"];
        let colors: Vec<u8> = rows
            .iter()
            .flat_map(|r| r.bytes())
            .map(|c| match c {
                b'R' => 0,
                b'G' => 1,
                _ => 2,
            })
            .collect();
        let cfa = Cfa::new(6, 6, colors).unwrap();
        let mut s = bayer(13, 12, |x, y| (100 + x * 7 + y * 31) as u16);
        s.meta.layout = Layout::Pattern(cfa.clone());
        s.meta.black = BlackLevels::simple([64.0; 4], 1);
        s.crop = Rect {
            x: 1,
            y: 0,
            w: 12,
            h: 12,
        };
        let t = s.third().expect("binnable");
        assert_eq!((t.width, t.height, t.cpp), (4, 4, 3));
        assert_eq!(t.meta.layout, Layout::Linear);
        assert_eq!((t.crop.x, t.crop.y, t.crop.w, t.crop.h), (1, 0, 3, 4));
        assert_eq!(t.meta.black.at(2, 3, 2), 64.0);
        for by in 0..4u32 {
            for bx in 0..4u32 {
                let mut sum = [0u32; 3];
                let mut n = [0u32; 3];
                for y in 3 * by..3 * by + 3 {
                    for x in 3 * bx..3 * bx + 3 {
                        let c = usize::from(cfa.color(y, x));
                        sum[c] += u32::from(s.sample(x, y, 0));
                        n[c] += 1;
                    }
                }
                assert_eq!(n, [2, 5, 2]);
                for c in 0..3 {
                    let want = (sum[c] + n[c] / 2) / n[c];
                    assert_eq!(
                        u32::from(t.sample(bx, by, c as u32)),
                        want,
                        "({bx}, {by}) {c}"
                    );
                }
            }
        }
        // A clipped sample keeps its channel clipped; the others stay means.
        let mut c = bayer(6, 6, |x, y| if (x, y) == (2, 0) { 4000 } else { 500 });
        c.meta.layout = Layout::Pattern(cfa);
        let ct = c.third().unwrap();
        assert_eq!(ct.sample(0, 0, 0), 4000);
        assert_eq!(ct.sample(0, 0, 1), 500);
        // Bayer data and varying black levels are not binned this way.
        assert!(bayer(12, 12, |_, _| 1).third().is_none());
        let mut varying = c.clone();
        varying.meta.black = BlackLevels::simple([64.0, 65.0, 64.0, 64.0], 1);
        assert!(varying.third().is_none());
    }

    #[test]
    fn black_pattern_repeats_from_the_origin_with_deltas() {
        let b = BlackLevels {
            repeat: (2, 3),
            cpp: 1,
            levels: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            origin: (1, 1),
            per_column: vec![10.0, 20.0],
            per_row: vec![100.0],
        };
        assert_eq!(b.at(1, 1, 0), 1.0 + 10.0 + 100.0);
        assert_eq!(b.at(2, 1, 0), 2.0 + 20.0 + 100.0);
        assert_eq!(b.at(3, 2, 0), 6.0);
        // Before the origin the pattern still repeats; no deltas there.
        assert_eq!(b.at(0, 0, 0), 6.0);
        assert_eq!(b.max(), 6.0 + 20.0 + 100.0);
    }

    #[test]
    fn simple_levels_follow_the_2x2_position() {
        let b = BlackLevels::simple([1.0, 2.0, 3.0, 4.0], 1);
        assert_eq!(
            [b.at(0, 0, 0), b.at(1, 0, 0), b.at(0, 1, 0), b.at(3, 3, 0)],
            [1.0, 2.0, 3.0, 4.0]
        );
        let l = BlackLevels::simple([1.0, 2.0, 3.0, 0.0], 3);
        assert_eq!([l.at(5, 7, 0), l.at(5, 7, 2)], [1.0, 3.0]);
    }
}
