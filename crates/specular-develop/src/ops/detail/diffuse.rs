//! Diffuse or sharpen: an
//! anisotropic diffusion PDE run for a number of iterations on the scales of a wavelet
//! decomposition, in the line of Perona and Malik (1990) and Weickert (1998).
//!
//! **One iteration.** The current image `u` (linear RGB) is split by an à-trous B3 cascade:
//! `L₀ = u`, `L_{j+1}` = `L_j` blurred at spacing `2^j`. Band `j` has a low-frequency layer
//! `L_{j+1}` and a high-frequency layer `H_j = L_j − L_{j+1}`. Each band within the radius
//! window adds `dt · w_j · Σ_k speed_k · D_k`, four terms ("orders"):
//!
//! | order | layer diffused | direction from the gradient of |
//! |---|---|---|
//! | 1 | low frequency `L_{j+1}` | the low frequency |
//! | 2 | low frequency | the high frequency `H_j` |
//! | 3 | high frequency | the low frequency |
//! | 4 | high frequency | the high frequency |
//!
//! `D_k = w_n·F_nn + w_t·F_tt`: the layer's second derivatives across the guide's edge
//! (`n`, along the luminance gradient) and along it (`t`), on 3×3 stencils at the band's
//! spacing. Anisotropy `a` moves the weights with the Perona–Malik edge function
//! `g = 1 / (1 + (r/K)²)` of the relative gradient `r = |∇Y| / Y`: `a > 0` stops diffusion
//! across edges (`w_n = 1 − a·(1 − g)`), so it follows them; `a < 0` stops it along edges.
//! Positive speeds diffuse (blur, bloom, fill); negative speeds run the PDE backwards
//! (sharpen, deblur).
//!
//! - **Edge threshold** holds the backward (sharpening) terms off where the relative
//!   gradient is small, so flat areas and their noise are not sharpened.
//! - **Highlights** (`> 0`) limits the change to pixels whose largest channel is near or
//!   above it: inpainting clipped highlights from their surroundings.
//!
//! **Stability.** After each iteration every channel is held within the range of the
//! previous iterate over the stencils' reach, widened by `MARGIN / iterations` of that
//! range. Over all iterations a value can leave its neighbourhood's range by at most a
//! factor `e^{2·MARGIN}` of it, whatever the speeds, so nothing blows up
//! (tested across presets, extremes, f16 and f32).
//!
//! **Scale.** Radius and span are fractions of the image's long side (shown in percent), so the fit view and
//! the export work on the same structures. Band `j` has a
//! radius of `2^j` view pixels; its weight is 1 inside `radius ± span` and falls off as a
//! Gaussian in octaves outside (σ = ¾ octave), so the same bands count at every size. Bands finer than a view pixel cannot be seen at a reduced view, so a
//! fine sharpen shows its effect only from 100 % (the panel says so).
//!
//! **Draft.** While a slider of this module is dragged (the graph's focus) a draft runs
//! fewer iterations with a proportionally longer step: half of them at half size (its
//! change scaled up and added to the input) when the finest detail is 2 pixels or more,
//! else a sixth at full size without the weak coarse bands (weight ≤ 0.1). The render after
//! the drag refines it.
//!
//! **Passes**. One iteration makes the cascade's levels, then sums every
//! band's change in one pass per grid and writes the next iterate (no accumulator
//! picture). Coarse bands run on reduced grids ([`band_grids`]; approximate, ΔE p95 ≤ 0.01
//! against full grids). An interactive render whose input and program are unchanged (only
//! this module's blend moved) reuses the kept result.
//!
//! All four speeds at 0 is the identity (the node is left out).

use super::kernels::fw;
use crate::graph::{Kernel, NodeKind, Op, Persist, Placement, Program, Roi};
use crate::model::{Fields, ModuleKind, Params, Value};

/// Iterations, at most.
pub const MAX_ITERATIONS: u32 = 20;
/// Radius and span, at most (fraction of the long side: 1 %).
pub const MAX_RADIUS: f32 = 0.01;
/// The coarsest band (view pixels `2^MAX_BAND`).
pub const MAX_BAND: u32 = 8;
/// The time step of one iteration at speed 1.
pub const DT: f32 = 0.125;
/// The finest detail (pixels) a half-size draft may leave out.
const HALF_DRAFT_PX: f32 = 2.0;
/// Bands weaker than this end a full-size draft's cascade.
const DRAFT_MIN_WEIGHT: f32 = 0.1;
/// The longest step a draft may take.
const DT_DRAFT_MAX: f32 = 0.25;
/// How far all iterations together may widen a neighbourhood's range (see the module docs).
pub const MARGIN: f32 = 0.05;
/// How fast band weights fall outside the window (octaves per standard deviation).
const FALLOFF: f32 = 0.75;
/// Weights below this leave the band out.
const MIN_WEIGHT: f32 = 1e-3;
/// The long side assumed when the graph is built without the image's size.
pub const FALLBACK_LONG: f32 = 2048.0;

/// Field holding the image's long side in graph pixels (set by [`Op::place`]; never stored).
pub const LONG_FIELD: &str = "_long";
/// Field marking a draft render (set by [`Op::place`]; never stored).
pub const DRAFT_FIELD: &str = "_draft";

/// Diffuse or sharpen params (fields of [`crate::model::ModuleKind::DIFFUSE`], version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Diffuse {
    /// 1 … [`MAX_ITERATIONS`].
    pub iterations: u32,
    /// Centre of the radius window, a fraction of the long side (0 … [`MAX_RADIUS`]).
    pub radius: f32,
    /// Half width of the window, a fraction of the long side (0 … [`MAX_RADIUS`]).
    pub span: f32,
    /// Speeds of the four orders, −1 (sharpen) … 1 (diffuse).
    pub speed: [f32; 4],
    /// Anisotropy of the four orders, −1 (along gradients) … 1 (along edges).
    pub anisotropy: [f32; 4],
    /// Edge sensitivity, 0 … 1.
    pub edges: f32,
    /// Edge threshold for the sharpening terms, 0 … 1.
    pub threshold: f32,
    /// Change only highlights near or above this linear value (0 = everywhere), 0 … 4.
    pub highlights: f32,
    /// The image's long side in graph pixels ([`LONG_FIELD`]).
    pub long: f32,
    /// A quick render while dragging ([`DRAFT_FIELD`]).
    pub draft: bool,
}

impl Default for Diffuse {
    fn default() -> Self {
        Self {
            iterations: 4,
            radius: 0.0,
            span: 0.0005,
            speed: [0.0; 4],
            anisotropy: [0.0; 4],
            edges: 0.5,
            threshold: 0.0,
            highlights: 0.0,
            long: FALLBACK_LONG,
            draft: false,
        }
    }
}

/// Field names of the four speeds and anisotropies.
pub const SPEED_FIELDS: [&str; 4] = ["speed_1", "speed_2", "speed_3", "speed_4"];
pub const ANISOTROPY_FIELDS: [&str; 4] = [
    "anisotropy_1",
    "anisotropy_2",
    "anisotropy_3",
    "anisotropy_4",
];

fn get(f: &Fields, n: &str, d: f32, lo: f32, hi: f32) -> f32 {
    let v = f.f32_or(n, d);
    if v.is_finite() { v.clamp(lo, hi) } else { d }
}

impl Diffuse {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        // An integer, but a slider may write it as a float.
        let it = f.f32_or("iterations", d.iterations as f32);
        let it = if it.is_finite() {
            it.round() as i64
        } else {
            i64::from(d.iterations)
        };
        let long = f.f32_or(LONG_FIELD, d.long);
        Self {
            iterations: it.clamp(1, i64::from(MAX_ITERATIONS)) as u32,
            radius: get(f, "radius", d.radius, 0.0, MAX_RADIUS),
            span: get(f, "span", d.span, 0.0, MAX_RADIUS),
            speed: std::array::from_fn(|k| get(f, SPEED_FIELDS[k], 0.0, -1.0, 1.0)),
            anisotropy: std::array::from_fn(|k| get(f, ANISOTROPY_FIELDS[k], 0.0, -1.0, 1.0)),
            edges: get(f, "edges", d.edges, 0.0, 1.0),
            threshold: get(f, "threshold", d.threshold, 0.0, 1.0),
            highlights: get(f, "highlights", d.highlights, 0.0, 4.0),
            long: if long.is_finite() && long >= 1.0 {
                long
            } else {
                d.long
            },
            draft: f.bool_or(DRAFT_FIELD, false),
        }
    }

    /// The stored fields (not the image's size or the draft flag).
    pub fn to_fields(&self) -> Fields {
        let mut f = Fields::new()
            .with("iterations", Value::Int(i64::from(self.iterations)))
            .with("radius", Value::Float(self.radius))
            .with("span", Value::Float(self.span));
        for k in 0..4 {
            f.set(SPEED_FIELDS[k], Value::Float(self.speed[k]));
            f.set(ANISOTROPY_FIELDS[k], Value::Float(self.anisotropy[k]));
        }
        f.with("edges", Value::Float(self.edges))
            .with("threshold", Value::Float(self.threshold))
            .with("highlights", Value::Float(self.highlights))
    }

    pub fn default_fields() -> Fields {
        Self::default().to_fields()
    }

    pub(crate) fn of(p: &Params) -> Self {
        Self::from_fields(&p.to_fields())
    }

    pub fn is_identity(&self) -> bool {
        self.speed.iter().all(|s| *s == 0.0)
    }
}

/// The presets the panel offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Sharpen,
    Deblur,
    LocalContrast,
    Bloom,
    InpaintHighlights,
    Smooth,
}

impl Preset {
    pub const ALL: [Preset; 6] = [
        Preset::Sharpen,
        Preset::Deblur,
        Preset::LocalContrast,
        Preset::Bloom,
        Preset::InpaintHighlights,
        Preset::Smooth,
    ];

    /// The preset's params.
    pub fn params(self) -> Diffuse {
        let d = Diffuse::default();
        match self {
            Preset::Sharpen => Diffuse {
                iterations: 4,
                radius: 0.0,
                span: 0.0006,
                speed: [0.0, 0.0, -0.5, -0.25],
                anisotropy: [0.0, 0.0, -1.0, -1.0],
                edges: 0.5,
                threshold: 0.1,
                ..d
            },
            Preset::Deblur => Diffuse {
                iterations: 6,
                radius: 0.0,
                span: 0.001,
                speed: [-0.5, -0.25, -0.5, -0.25],
                anisotropy: [-1.0, -1.0, -1.0, -1.0],
                edges: 0.6,
                threshold: 0.1,
                ..d
            },
            Preset::LocalContrast => Diffuse {
                iterations: 2,
                radius: 0.006,
                span: 0.003,
                speed: [0.0, 0.0, -0.5, -0.25],
                anisotropy: [0.0, 0.0, 1.0, 1.0],
                edges: 0.4,
                threshold: 0.0,
                ..d
            },
            Preset::Bloom => Diffuse {
                iterations: 4,
                radius: 0.006,
                span: 0.004,
                speed: [1.0, 0.5, 0.0, 0.0],
                anisotropy: [0.0, 0.0, 0.0, 0.0],
                edges: 0.0,
                threshold: 0.0,
                ..d
            },
            Preset::InpaintHighlights => Diffuse {
                iterations: 10,
                radius: 0.001,
                span: 0.001,
                speed: [1.0, 1.0, 0.5, 0.0],
                anisotropy: [1.0, 1.0, 1.0, 0.0],
                edges: 0.3,
                threshold: 0.0,
                highlights: 0.95,
                ..d
            },
            Preset::Smooth => Diffuse {
                iterations: 4,
                radius: 0.0,
                span: 0.0005,
                speed: [0.0, 0.0, 0.75, 0.5],
                anisotropy: [0.0, 0.0, 1.0, 1.0],
                edges: 0.7,
                threshold: 0.0,
                ..d
            },
        }
    }

    /// The preset these fields are, if any (the stored fields only).
    pub fn matching(f: &Fields) -> Option<Preset> {
        let d = Diffuse::from_fields(f);
        Preset::ALL.into_iter().find(|p| {
            let q = p.params();
            Diffuse {
                long: d.long,
                draft: d.draft,
                ..q
            } == d
        })
    }

    /// Whether a fit view shows this preset as the export does (improvement 14, tested);
    /// the others say the detail shows only at 100 %. A 6000-pixel photo is assumed.
    pub fn scale_stable(self) -> bool {
        !needs_full_view(&self.params(), 6000.0, FULL_VIEW_PX)
    }
}

/// Detail finer than this many full-size pixels is lost in a reduced view.
pub const FULL_VIEW_PX: f32 = 3.0;

/// Whether these params change structures finer than `px` pixels of a photo whose long
/// side is `long`, which a reduced view cannot show, so the panel asks to check at 100 %.
/// Filling highlights is left out: it fills areas, whatever their detail.
pub fn needs_full_view(d: &Diffuse, long: f32, px: f32) -> bool {
    !d.is_identity() && d.highlights == 0.0 && (d.radius - d.span) * long < px
}

/// What one render runs: iterations, step, the bands' weights and the reach.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub iterations: u32,
    pub dt: f32,
    /// Weight of band `j` (radius `2^j` view pixels), up to the coarsest used.
    pub weights: Vec<f32>,
    /// The finest band with a weight.
    pub first: u32,
    /// The margin per iteration.
    pub margin: f32,
    /// `1/K` of the edge function.
    pub inv_k: f32,
    /// Relative gradient at which sharpening starts (0 = everywhere).
    pub threshold: f32,
    /// The work runs at half size and its change is scaled up (a draft).
    pub half: bool,
    /// A refined render too large for whole-picture scratch: the fine bands' levels are
    /// made stripe by stripe and the iterate is updated in place ([`program`]).
    pub striped: bool,
    /// The grid of each band: band `j` runs on every `2^grids[j]`-th pixel (see
    /// [`band_grids`]).
    pub grids: Vec<u32>,
}

impl Plan {
    /// The coarsest band.
    pub fn last(&self) -> u32 {
        self.weights.len().saturating_sub(1) as u32
    }

    /// Input pixels one iteration reads around each output pixel.
    pub fn reach(&self) -> u32 {
        let h = 1u32 << self.last();
        // The cascade to L_{J+1} (2·(2^{J+1} − 1)), then a 3×3 stencil at 2^J on it.
        4 * h - 2 + h
    }

    /// Scratch bytes of the program on a 3:2 picture whose long side is `long` (graph
    /// pixels): the two iterates, each band's level on its grid and the reduced grids' sums.
    pub fn scratch_bytes(&self, long: f32) -> u64 {
        let f = if self.half { 0.5 } else { 1.0 };
        let (w, h) = (long * f, long * f * 2.0 / 3.0);
        let px = f64::from(w) * f64::from(h);
        let q = |m: u32| 0.25f64.powi(m as i32);
        let top = self.grids.iter().copied().max().unwrap_or(0);
        let sums = (1..=top).map(|m| 3.0 * q(m)).sum::<f64>();
        if self.striped {
            let fine = self.grids.iter().take_while(|m| **m == 0).count();
            let coarse = self.grids[fine..].iter().map(|m| 3.0 * q(*m)).sum::<f64>();
            let (w, h) = (w as u32, h as u32);
            let n1 = if top > 0 {
                w.div_ceil(2) * h.div_ceil(2)
            } else {
                0
            };
            let st = Stripes::new(self, fine, w, h, n1);
            let words = 3.0 * px + (coarse + sums) * px + f64::from(st.words);
            return (4.0 * words) as u64;
        }
        let words = if self.half { 9.0 } else { 6.0 }
            + self.grids.iter().map(|m| 3.0 * q(*m)).sum::<f64>()
            + sums;
        (4.0 * words * px) as u64
    }

    pub fn halo(&self) -> u32 {
        // A reduced grid's edge sits up to a sample inside the region, and its sum is
        // interpolated from the sample beyond.
        let coarse = 2 << self.grids.iter().copied().max().unwrap_or(0);
        let h = self.iterations * (self.reach() + coarse) + 1;
        if self.half { 2 * h + 2 } else { h }
    }
}

/// The plan for `d` (with its image size and draft flag set) at the graph's own scale.
///
/// A draft runs at half size when its finest detail survives halving; filling highlights
/// fills areas, so it drafts at half size too.
///
/// A refined render runs in stripes when its whole-picture scratch would pass
/// [`REFINED_SCRATCH_MAX`] (24 MP: Bloom 1.4, Local contrast 1.8, Fill highlights 2.6 GiB):
/// one iterate updated in place, the coarse bands' levels on their reduced grids over the
/// whole picture, the fine bands' levels stripe by stripe with the halo their cascade
/// reads (see [`program`]). The same sums in the same order: the picture of the
/// whole-picture program, bit for bit. Filling highlights then puts its coarse bands on
/// reduced grids too (their change gated at each pixel, as the fine bands').
pub fn plan(d: &Diffuse) -> Plan {
    let detail = (d.radius - d.span) * d.long >= HALF_DRAFT_PX;
    if d.draft {
        return plan_at(d, detail || d.highlights > 0.0, false);
    }
    let full = plan_at(d, false, false);
    let over = full.scratch_bytes(d.long) > REFINED_SCRATCH_MAX || force_stripes();
    if over && !no_stripes() && full.iterations > 0 {
        plan_at(d, false, true)
    } else {
        full
    }
}

/// Scratch a refined render may take before it runs in stripes (bytes, for a 3:2 picture
/// of the long side).
pub const REFINED_SCRATCH_MAX: u64 = 512 << 20;

fn plan_at(d: &Diffuse, half: bool, striped: bool) -> Plan {
    let long = if half { 0.5 * d.long } else { d.long };
    let rc = d.radius * long;
    let rs = d.span * long;
    let mut weights: Vec<f32> = (0..=MAX_BAND)
        .map(|j| {
            let r = (1u32 << j) as f32;
            let hi = (rc + rs).max(1e-3);
            let lo = rc - rs;
            let out = if r > hi {
                (r / hi).log2()
            } else if lo > 0.0 && r < lo {
                (lo / r).log2()
            } else {
                0.0
            };
            let z = out / FALLOFF;
            let w = (-0.5 * z * z).exp();
            if w < MIN_WEIGHT { 0.0 } else { w }
        })
        .collect();
    // A full-size draft leaves out the weak coarse bands at the end (each costs two
    // passes over the picture).
    let cut = if d.draft && !half {
        DRAFT_MIN_WEIGHT
    } else {
        0.0
    };
    while weights.len() > 1 && weights.last().is_some_and(|w| *w <= cut) {
        weights.pop();
    }
    let first = weights.iter().position(|w| *w > 0.0).unwrap_or(0) as u32;
    let weights_len = weights.len() as u32;
    // No band in reach (a window far finer than a view pixel): nothing to do.
    let full = if weights.iter().any(|w| *w > 0.0) {
        d.iterations.max(1)
    } else {
        0
    };
    let (iterations, dt) = if d.draft && full > 0 {
        let k = full.div_ceil(if half { 2 } else { 6 });
        (k, (DT * full as f32 / k as f32).min(DT_DRAFT_MAX))
    } else {
        (full, DT)
    };
    Plan {
        iterations,
        dt,
        weights,
        first,
        margin: MARGIN / iterations.max(1) as f32,
        inv_k: 1.0 / (0.5 * 10f32.powf(-2.0 * d.edges)),
        threshold: 0.1 * d.threshold,
        half,
        striped,
        grids: {
            // Sharpening amplifies what interpolation misses: its coarse bands keep one
            // grid more.
            let from = if d.speed.iter().any(|s| *s < 0.0) {
                COARSE_FROM + 1
            } else {
                COARSE_FROM
            };
            (0..weights_len)
                .map(|j| {
                    if (d.highlights > 0.0 && !striped && !coarse_highlights()) || full_grid() {
                        0
                    } else {
                        j.saturating_sub(from).min(MAX_GRID)
                    }
                })
                .collect()
        },
    }
}

/// Band `j` from this one on runs on a grid of every `2^(j − COARSE_FROM)`-th pixel (one
/// band later when a speed sharpens).
const COARSE_FROM: u32 = 2;
/// The coarsest grid (every `2^MAX_GRID`-th pixel).
const MAX_GRID: u32 = 4;

/// The grid of each band (`2^m`: the spacing of its samples) on a `w`×`h` region: the
/// plan's, held so a grid keeps at least 4 samples across the region's short side.
///
/// **Coarse bands** (approximate; not when filling highlights). The cascade at spacing
/// `2^j` couples only pixels `2^j` apart, so on every `2^m`-th pixel (`m ≤ j`) its levels
/// are the full-size levels there, exactly, and so is band `j`'s change. Band `j ≥ 2`
/// works on every `2^(j−2)`-th pixel (at most every 16th), still 4 samples per band
/// radius (from `j ≥ 3`, 8 samples, when a speed sharpens: its error grows with each
/// iteration); its change is interpolated (bilinear, grid by grid from the coarsest) to the
/// pixels between. Samples sit at multiples of `2^m` in the whole picture, so tiles match
/// the whole image.
pub fn band_grids(pl: &Plan, w: u32, h: u32) -> Vec<u32> {
    let short = w.min(h).max(1);
    let cap = (short / 4).max(1).ilog2();
    pl.grids.iter().map(|m| (*m).min(cap)).collect()
}

/// Kinds of a level descriptor ([`LEVEL_WGSL`]).
const LV_F32: u32 = 0;
const LV_CASCADE: u32 = 1;
const LV_INPUT: u32 = 2;

/// A grid of the program's region: every `2^m`-th pixel of the picture's grid, at
/// multiples of `2^m` in the whole picture.
#[derive(Clone, Copy, Debug)]
struct GridAt {
    m: u32,
    /// Its first sample, in the picture grid's pixels.
    ox: u32,
    oy: u32,
    w: u32,
    h: u32,
}

impl GridAt {
    fn new(m: u32, x0: u32, y0: u32, w: u32, h: u32) -> GridAt {
        let f = 1u32 << m;
        let (ox, oy) = ((f - x0 % f) % f, (f - y0 % f) % f);
        GridAt {
            m,
            ox,
            oy,
            w: w.saturating_sub(ox).div_ceil(f).max(1),
            h: h.saturating_sub(oy).div_ceil(f).max(1),
        }
    }

    fn n(&self) -> u32 {
        self.w * self.h
    }
}

/// The descriptor of a picture stored at `off` on grid `at`, read from grid `by` (as fine
/// or coarser).
fn level(off: u32, kind: u32, at: &GridAt, by: &GridAt) -> [u32; 6] {
    [
        off,
        at.w,
        (by.ox - at.ox) >> at.m,
        (by.oy - at.oy) >> at.m,
        by.m - at.m,
        kind,
    ]
}

/// Scratch images (`3n` words each, of the region or, for a draft, of its half-size
/// copy): the iterate and the next (0, 1), for a draft the half-size input (2), then the
/// cascade's levels `L_1 … L_{J+1}` (each on its band's grid; in an f16 render two words
/// of packed f16 per pixel) and the sums of the reduced grids. A pixel's channels are
/// together (one cache line per stencil tap).
///
/// One iteration is a level step per band, then one band step per grid (coarsest first),
/// the last writing the next iterate: the change is summed in registers, not in a picture
/// (the passes are bound by memory traffic). The first iteration reads the input where it
/// lies (no copy) unless a draft, highlights' tile mask or no band needs one.
///
/// **Stripes** ([`Plan::striped`]). One iterate (0), updated in place. Per
/// iteration: the fine bands' levels (those on the picture's grid) on the first reduced
/// grid over the whole picture (two pictures used in turn; [`DF_LEVEL_SUB`] makes `L_1`
/// there), then the coarse levels and the reduced grids' sums as above; then stripe by
/// stripe of rows, the fine levels over the stripe and the halo their cascade and stencils
/// read ([`Stripes`]), and the stripe's next iterate into one of two stripe pictures. A
/// stripe is at least the reach of `u`, so once the next stripe is done the one before it
/// is read no more and is copied into the iterate ([`DF_COPY`]). Every value comes from
/// the same sums in the same order as on whole pictures.
pub fn program(d: &Diffuse, input: Roi) -> Program {
    #[cfg(test)]
    if LEGACY.load(std::sync::atomic::Ordering::Relaxed) {
        return super::diffuse_legacy::program(d, input);
    }
    let pl = plan(d);
    let f = if pl.half { 2 } else { 1 };
    let (w, h) = (input.w.div_ceil(f), input.h.div_ceil(f));
    let n = w * h;
    let mut prog = Program::default();
    let masked = d.highlights > 0.0 && pl.iterations > 0 && !no_hot_mask();
    let init = pl.half || masked || pl.iterations == 0;
    if init {
        prog.step(
            &DF_INIT,
            [w, h],
            vec![
                0,
                w,
                input.x,
                input.y,
                n,
                f,
                input.right(),
                input.bottom(),
                HALF_COPY * n,
            ],
        );
    }
    let bg = band_grids(&pl, w, h);
    let top = bg.iter().copied().max().unwrap_or(0);
    let gs: Vec<GridAt> = (0..=top)
        .map(|m| GridAt::new(m, input.x / f, input.y / f, w, h))
        .collect();
    let striped = pl.striped;
    // The bands on the picture's grid (a prefix: grids grow with the band).
    let fine = bg.iter().take_while(|m| **m == 0).count();
    let mut next = if pl.half {
        9 * n
    } else if striped {
        3 * n
    } else {
        6 * n
    };
    let mut take = |words: u32| {
        let o = next;
        next += words;
        o
    };
    // L_{j+1} lives on band j's grid; in stripes the fine levels live in the stripe's.
    let lv: Vec<u32> = bg
        .iter()
        .enumerate()
        .map(|(j, m)| {
            if striped && j < fine {
                u32::MAX
            } else {
                take(3 * gs[*m as usize].n())
            }
        })
        .collect();
    let sums: Vec<u32> = gs[1..].iter().map(|g| take(3 * g.n())).collect();
    let st = striped.then(|| Stripes::new(&pl, fine, w, h, gs.get(1).map_or(0, GridAt::n)));
    let sb = next;
    if let Some(s) = &st {
        next += s.words;
    }
    let mb = next;
    // Highlights only: the tiles within one iteration's reach of a pixel that can change
    // (see `DF_HOT`). The levels are made only there; elsewhere no change reads them.
    let (tw, tht) = (w.div_ceil(HOT_TILE), h.div_ceil(HOT_TILE));
    let nt = tw * tht;
    let mask = mb + 2 * nt;
    if masked {
        let reach = pl.reach().div_ceil(HOT_TILE) + 1;
        prog.step(&DF_HOT, [tw, tht], vec![w, h, n, tw, mb, fw(d.highlights)]);
        prog.step(
            &DF_DILATE,
            [tw, tht],
            vec![tw, tht, mb, mb + nt, reach, 1, 0],
        );
        prog.step(
            &DF_DILATE,
            [tw, tht],
            vec![tw, tht, mb + nt, mask, reach, 0, 1],
        );
    }
    // The cascade at f16 in f16 renders.
    let half = !f32_cascade();
    let kc = if half { LV_CASCADE } else { LV_F32 };
    let g0 = gs[0];
    let mut common = Vec::with_capacity(15);
    common.extend(d.speed.iter().map(|v| fw(*v)));
    common.extend(d.anisotropy.iter().map(|v| fw(*v)));
    common.extend_from_slice(&[
        fw(pl.inv_k),
        fw(pl.threshold),
        fw(d.highlights),
        1 << pl.first,
        1 << pl.last(),
        fw(pl.margin),
    ]);
    let lmask = masked.then_some((mask, tw));
    for it in 0..pl.iterations {
        let (u, to) = if striped {
            (0, 0)
        } else if it % 2 == 0 {
            (0, 3 * n)
        } else {
            (3 * n, 0)
        };
        let uk = if it == 0 && !init { LV_INPUT } else { LV_F32 };
        // L_j as a grid `by` reads it (whole pictures).
        let src = |j: usize, by: &GridAt| {
            if j == 0 {
                level(u, uk, &g0, by)
            } else if let Some(s) = &st
                && j == fine
            {
                level(sb + s.shadow[(fine - 1) % 2], kc, &gs[1], by)
            } else {
                level(lv[j - 1], kc, &gs[bg[j - 1] as usize], by)
            }
        };
        // Stripes: the fine levels on the first reduced grid, for the coarse ones.
        if let Some(s) = &st
            && fine < bg.len()
        {
            let g1 = gs[1];
            let mut words = vec![sb + s.shadow[0], g1.w, g1.h, g1.ox, g1.oy, g1.m, w, h];
            words.extend_from_slice(&level(u, uk, &g0, &g0));
            words.extend_from_slice(&[u32::from(half), 0]);
            let rows = (STRIPE / g1.w).max(1);
            let mut y0 = 0;
            while y0 < g1.h {
                let mut wd = words.clone();
                wd[15] = y0;
                prog.step(&DF_LEVEL_SUB, [g1.w, rows.min(g1.h - y0)], wd);
                y0 += rows;
            }
            for k in 2..=fine {
                level_step(
                    &mut prog,
                    sb + s.shadow[(k - 1) % 2],
                    &g1,
                    (1 << (k - 1)) >> g1.m,
                    level(sb + s.shadow[(k - 2) % 2], kc, &g1, &g1),
                    half,
                    None,
                    (0, g1.h),
                    0,
                );
            }
        }
        for (j, m) in bg.iter().enumerate() {
            if striped && j < fine {
                continue;
            }
            let g = gs[*m as usize];
            level_step(
                &mut prog,
                lv[j],
                &g,
                (1 << j) >> g.m,
                src(j, &g),
                half,
                lmask.filter(|_| g.m == 0),
                (0, g.h),
                0,
            );
        }
        let coarse = |g: &GridAt| {
            gs.get(g.m as usize + 1).map(|c| {
                [
                    1,
                    sums[g.m as usize],
                    c.w,
                    c.h,
                    (c.ox - g.ox) >> g.m,
                    (c.oy - g.oy) >> g.m,
                ]
            })
        };
        for g in gs.iter().rev() {
            let m = g.m as usize;
            if m == 0 && striped {
                break;
            }
            let fin = m == 0;
            let bands: Vec<Band> = (0..bg.len())
                .filter(|&j| bg[j] == g.m && pl.weights[j] > 0.0)
                .map(|j| Band {
                    weight: pl.weights[j] * pl.dt,
                    spacing: (1 << j) >> g.m,
                    lj: src(j, g),
                    lj1: level(lv[j], kc, g, g),
                })
                .collect();
            band_step(
                &mut prog,
                g,
                if fin { to } else { sums[m - 1] },
                level(u, uk, &g0, g),
                coarse(g),
                &common,
                fin,
                &bands,
                (0, g.h),
            );
        }
        let Some(s) = &st else { continue };
        // The picture's grid, stripe by stripe. The first iteration reading the input
        // writes the iterate directly.
        let direct = uk == LV_INPUT;
        let mut last: Option<(u32, u32, u32)> = None;
        let mut y0 = 0;
        let mut k = 0usize;
        while y0 < h {
            let y1 = (y0 + s.rows).min(h);
            // L_k over the stripe and its halo, as the picture's grid reads it.
            let at = |k: usize| y0.saturating_sub(s.halo[k - 1]);
            let sl = |k: usize| [sb + s.levels[k - 1], w, 0, 0u32.wrapping_sub(at(k)), 0, kc];
            for k in 1..=fine {
                let srck = if k == 1 {
                    level(u, uk, &g0, &g0)
                } else {
                    sl(k - 1)
                };
                level_step(
                    &mut prog,
                    sb + s.levels[k - 1],
                    &g0,
                    1 << (k - 1),
                    srck,
                    half,
                    lmask,
                    (at(k), (y1 + s.halo[k - 1]).min(h)),
                    at(k),
                );
            }
            let bands: Vec<Band> = (0..fine)
                .filter(|&j| pl.weights[j] > 0.0)
                .map(|j| Band {
                    weight: pl.weights[j] * pl.dt,
                    spacing: 1 << j,
                    lj: if j == 0 {
                        level(u, uk, &g0, &g0)
                    } else {
                        sl(j)
                    },
                    lj1: sl(j + 1),
                })
                .collect();
            let t = sb + s.temp[k % 2];
            let dest = if direct {
                0
            } else {
                t.wrapping_sub(3 * y0 * w)
            };
            band_step(
                &mut prog,
                &g0,
                dest,
                level(u, uk, &g0, &g0),
                coarse(&g0),
                &common,
                true,
                &bands,
                (y0, y1),
            );
            // The stripe before is read no more: into the iterate.
            if let Some((src, ya, yb)) = last.take() {
                copy_rows(&mut prog, src, 3 * ya * w, 3 * w, yb - ya);
            }
            if !direct {
                last = Some((t, y0, y1));
            }
            y0 = y1;
            k += 1;
        }
        if let Some((src, ya, yb)) = last {
            copy_rows(&mut prog, src, 3 * ya * w, 3 * w, yb - ya);
        }
    }
    prog.words = u64::from(mb) + if masked { 3 * u64::from(nt) } else { 0 };
    // The result depends on the input and these steps alone, so
    // an interactive render after a change of this module's blend (mask, opacity) reuses
    // it. Edits after the module resume from its checkpoint.
    prog.persist = Some(Persist {
        reset: Vec::new(),
        reuse: true,
    });
    prog
}

/// Stripe mode's pictures ([`program`]), in words from the start of their area: the two
/// stripes of the next iterate and each fine level over a stripe and its halo. Before the
/// stripes run, the same area holds the two pictures of the fine levels on the first
/// reduced grid.
#[derive(Clone, Debug)]
pub(crate) struct Stripes {
    /// Rows of a stripe (a multiple of 16, at least the reach of `u`).
    pub rows: u32,
    /// Rows above and below a stripe that `L_k` is made over (`halo[k − 1]`).
    pub halo: Vec<u32>,
    pub levels: Vec<u32>,
    pub temp: [u32; 2],
    pub shadow: [u32; 2],
    pub words: u32,
}

impl Stripes {
    /// For `fine` bands on the picture's grid of a `w`×`h` region; `n1` the samples of the
    /// first reduced grid (0 without coarse bands).
    pub(crate) fn new(pl: &Plan, fine: usize, w: u32, h: u32, n1: u32) -> Stripes {
        let fine = fine.max(1);
        // Band j reads L_j and L_{j+1} 2^j rows away; L_{k+1} reads L_k 2^{k+1} rows away.
        let mut halo = vec![0u32; fine];
        for k in (1..=fine).rev() {
            halo[k - 1] = if k == fine {
                1 << (k - 1)
            } else {
                halo[k] + (2 << k)
            };
        }
        // `u` is read by L_1 (2 rows beyond its halo) and by the clamp's coarsest stencil.
        let reach = (halo[0] + 2).max(1 << pl.last());
        #[allow(unused_mut)]
        let mut want = STRIPE / w.max(1);
        #[cfg(test)]
        {
            let r = STRIPE_ROWS.load(std::sync::atomic::Ordering::Relaxed);
            if r > 0 {
                want = r;
            }
        }
        let rows = want
            .max(reach)
            .next_multiple_of(16)
            .min(h.next_multiple_of(16));
        let mut o = 0u32;
        let mut take = |x: u32| {
            let a = o;
            o += x;
            a
        };
        let temp = [take(3 * w * rows), take(3 * w * rows)];
        // 16 rows more: a workgroup level step may finish its last rows there.
        let levels = halo
            .iter()
            .map(|hk| take(3 * w * ((rows + 2 * hk).min(h) + 16)))
            .collect();
        Stripes {
            rows,
            halo,
            levels,
            temp,
            shadow: [0, 3 * n1],
            words: o.max(6 * n1),
        }
    }
}

/// A level step over rows `rows` of grid `g` (in stripes of whole workgroup rows), its
/// destination's first row `org`.
#[allow(clippy::too_many_arguments)]
fn level_step(
    prog: &mut Program,
    dest: u32,
    g: &GridAt,
    spacing: u32,
    src: [u32; 6],
    half: bool,
    mask: Option<(u32, u32)>,
    rows: (u32, u32),
    org: u32,
) {
    let mut words = vec![dest, g.w, g.h, spacing];
    words.extend_from_slice(&src);
    let (m, tw) = mask.unwrap_or((0, 0));
    words.extend_from_slice(&[u32::from(half), u32::from(mask.is_some()), m, tw, 0, org]);
    let (k, wx) = if mask.is_some() || spacing > ROWS_MAX_SPACING {
        (&DF_LEVEL, g.w)
    } else {
        // Whole workgroups (16×16 pixels each).
        (&DF_LEVEL_ROWS, g.w.next_multiple_of(16))
    };
    // Stripes of whole workgroup rows, as the band steps.
    let step = (STRIPE / wx).max(16) / 16 * 16;
    let mut y0 = rows.0;
    while y0 < rows.1 {
        let mut wd = words.clone();
        wd[14] = y0;
        let hy = step.min(rows.1 - y0);
        let hy = if std::ptr::eq(k, &DF_LEVEL_ROWS) {
            hy.next_multiple_of(16)
        } else {
            hy
        };
        prog.step(k, [wx, hy], wd);
        y0 += step;
    }
}

/// One band of a band step: weight·dt, spacing (grid pixels), `L_j`'s and `L_{j+1}`'s
/// level descriptors.
struct Band {
    weight: f32,
    spacing: u32,
    lj: [u32; 6],
    lj1: [u32; 6],
}

/// The band step of grid `g` over `rows` (see [`DF_BANDS`]). Highlights gate only on the
/// picture's grid (`fin`): the reduced grids' sums are gated there with the rest.
#[allow(clippy::too_many_arguments)]
fn band_step(
    prog: &mut Program,
    g: &GridAt,
    to: u32,
    u: [u32; 6],
    coarse: Option<[u32; 6]>,
    common: &[u32],
    fin: bool,
    bands: &[Band],
    rows: (u32, u32),
) {
    let mut words = vec![g.w, g.h, to, u32::from(fin)];
    words.extend_from_slice(&u);
    words.extend_from_slice(&coarse.unwrap_or([0; 6]));
    words.extend_from_slice(common);
    if !fin {
        words[26] = 0;
    }
    let hl = words[26] != 0;
    words.push(bands.len() as u32);
    let row = words.len();
    words.push(0);
    let nb = bands.len() as u32;
    for b in bands {
        words.extend_from_slice(&[fw(b.weight), b.spacing]);
        words.extend_from_slice(&b.lj);
        words.extend_from_slice(&b.lj1);
    }
    // Stripes of rows, so no dispatch runs much longer than one band over a picture did
    // (a long dispatch can trip the driver's watchdog). Without highlights, whole
    // workgroups share their taps.
    let wg = !hl && !no_band_tiles();
    let step = (STRIPE / (g.w * nb.max(1))).max(1);
    let step = if wg { step.max(16) / 16 * 16 } else { step };
    let mut y0 = rows.0;
    while y0 < rows.1 {
        let mut wd = words.clone();
        wd[row] = y0;
        let hy = step.min(rows.1 - y0);
        if wg {
            let th = [g.w.next_multiple_of(16), hy.next_multiple_of(16)];
            prog.step(&DF_BANDS_WG, th, wd);
        } else {
            prog.step(&DF_BANDS, [g.w, hy], wd);
        }
        y0 += step;
    }
}

/// Copies `rows` rows of `row` words from `src` to `dst` ([`DF_COPY`]).
fn copy_rows(prog: &mut Program, src: u32, dst: u32, row: u32, rows: u32) {
    let step = (STRIPE / row.max(1)).max(1);
    let mut y = 0;
    while y < rows {
        let hy = step.min(rows - y);
        prog.step(&DF_COPY, [row, hy], vec![src + y * row, dst + y * row, row]);
        y += step;
    }
}

/// Pixel·bands one band step covers at most.
const STRIPE: u32 = 1 << 21;

/// Where a draft's half-size copy of the input starts (in planes of the region).
const HALF_COPY: u32 = 6;

/// Tests also run the earlier, slower program ([`super::diffuse_legacy`]).
#[cfg(test)]
pub(crate) static LEGACY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Tests run refined renders on whole-picture scratch whatever its size.
#[cfg(test)]
pub(crate) static NO_STRIPES: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn no_stripes() -> bool {
    #[cfg(test)]
    return NO_STRIPES.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(test))]
    false
}

/// Tests run refined renders in stripes whatever their size.
#[cfg(test)]
pub(crate) static FORCE_STRIPES: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn force_stripes() -> bool {
    #[cfg(test)]
    return FORCE_STRIPES.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(test))]
    false
}

/// Tests put the coarse bands of highlights on reduced grids on whole pictures too.
#[cfg(test)]
pub(crate) static COARSE_HIGHLIGHTS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn coarse_highlights() -> bool {
    #[cfg(test)]
    return COARSE_HIGHLIGHTS.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(test))]
    false
}

/// Tests' stripe height (rows; 0: the plan's).
#[cfg(test)]
pub(crate) static STRIPE_ROWS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Tests run the band steps per thread ([`DF_BANDS`]) only.
#[cfg(test)]
pub(crate) static NO_BAND_TILES: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn no_band_tiles() -> bool {
    #[cfg(test)]
    return NO_BAND_TILES.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(test))]
    false
}

/// Tests run every band on the full grid.
#[cfg(test)]
pub(crate) static FULL_GRID: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn full_grid() -> bool {
    #[cfg(test)]
    return FULL_GRID.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(test))]
    false
}
/// Nominal bytes per thread of a step (each picture read once and written once): `it`
/// bytes per iterate pixel, `cas` per cascade pixel. Timing reports only.
#[cfg(test)]
pub(crate) fn step_bytes(s: &crate::graph::Step, it: u64, cas: u64) -> u64 {
    use super::diffuse_legacy::{DF_ATROUS, DF_UPDATE};
    let w = &s.words;
    let src = |pc: u32| if pc != 0 { cas } else { it };
    // A level descriptor's kind: the input picture counts as an f16 or f32 read.
    let lv = |k: u32| if k == LV_CASCADE { cas } else { it };
    match s.kernel.name {
        n if n == DF_INIT.name => 16 + it * u64::from(w[5]).min(2),
        n if n == DF_HOT.name => 256 * it,
        n if n == DF_ATROUS.name => src(w[9]) + cas,
        n if n == DF_UPDATE.name => cas + src(w[24]) + it + if w[8] != 0 { it } else { 2 * it },
        n if n == DF_LEVEL.name || n == DF_LEVEL_ROWS.name => lv(w[9]) + cas,
        n if n == DF_BANDS.name || n == DF_BANDS_WG.name => {
            // u, every band's L_{j+1} (L_j is the band before's), the next iterate or sum.
            let nb = w[30] as usize;
            let bands: u64 = (0..nb).map(|b| lv(w[32 + 14 * b + 13])).sum();
            lv(w[9]) + bands + it + if w[10] != 0 { it / 4 } else { 0 }
        }
        _ => 0,
    }
}

/// What a step works on, for timing reports: the kernel and its band's spacing.
#[cfg(test)]
pub(crate) fn step_label(s: &crate::graph::Step) -> String {
    use super::diffuse_legacy::{DF_ATROUS, DF_UPDATE};
    let w = &s.words;
    match s.kernel.name {
        n if n == DF_ATROUS.name => format!("atrous s{}", w[5]),
        n if n == DF_UPDATE.name => format!("update s{}", w[7]),
        n if n == DF_LEVEL.name || n == DF_LEVEL_ROWS.name => {
            format!("level {}x{} s{}", w[1], w[2], w[3])
        }
        n if n == DF_BANDS.name || n == DF_BANDS_WG.name => {
            format!("bands {}x{} ({})", w[0], w[1], w[30])
        }
        n => n.to_owned(),
    }
}

/// Tests compare the f16 cascade with an f32 one.
#[cfg(test)]
pub(crate) static F32_CASCADE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(crate) fn f32_cascade() -> bool {
    #[cfg(test)]
    return F32_CASCADE.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(test))]
    false
}

/// Tests compare the masked bands with bands made everywhere.
#[cfg(test)]
pub(crate) static NO_HOT_MASK: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(crate) fn no_hot_mask() -> bool {
    #[cfg(test)]
    return NO_HOT_MASK.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(test))]
    false
}

/// Side of the tiles [`DF_HOT`] marks.
pub(crate) const HOT_TILE: u32 = 16;

/// Highlights only: marks (1) each [`HOT_TILE`]² tile of the first iterate `u` holding a
/// pixel above the highlights' smooth step, the only pixels that ever change (a pixel below
/// it keeps its value, so it stays below). Words: width, height, plane size, tiles per
/// row, mask, highlights.
pub static DF_HOT: Kernel = Kernel {
    name: "diffuse_hot_v2",
    atomic: false,
    helpers: &[],
    body: "    let w = pu(p);
    let h = pu(p + 1u);
    let hl = pf(p + 5u);
    var hot = 0.0;
    for (var b = 0u; b < 16u; b++) {
        let y = t.y * 16u + b;
        if (y >= h) { break; }
        for (var a = 0u; a < 16u; a++) {
            let x = t.x * 16u + a;
            if (x >= w) { break; }
            let i = 3u * (y * w + x);
            let m = max(S[i], max(S[i + 1u], S[i + 2u]));
            // As the update's test, negated (a NaN counts as hot).
            if (!(m <= 0.8 * hl)) { hot = 1.0; }
        }
    }
    S[pu(p + 4u) + t.y * pu(p + 3u) + t.x] = hot;",
};

/// The tile mask grown by word 4 tiles along x (word 5 = 1) or y (word 6 = 1). Words:
/// tiles per row, rows of tiles, source, destination, reach, along x, along y.
pub static DF_DILATE: Kernel = Kernel {
    name: "diffuse_dilate_v1",
    atomic: false,
    helpers: &[],
    body: "    let tw = i32(pu(p));
    let th = i32(pu(p + 1u));
    let r = i32(pu(p + 4u));
    let d = vec2<i32>(i32(pu(p + 5u)), i32(pu(p + 6u)));
    var v = 0.0;
    for (var k = -r; k <= r; k++) {
        let q = vec2<i32>(t.xy) + k * d;
        if (q.x >= 0 && q.y >= 0 && q.x < tw && q.y < th) {
            v = max(v, S[pu(p + 2u) + u32(q.y * tw + q.x)]);
        }
    }
    S[pu(p + 3u) + t.y * u32(tw) + t.x] = v;",
};

/// The image (in units of `3n` words) holding the result.
pub fn result_image(d: &Diffuse) -> u32 {
    let pl = plan(d);
    if pl.striped { 0 } else { pl.iterations % 2 }
}

/// Words: destination (an image), width, region x, region y, plane size, factor (1, or
/// 2 for a draft's half size: the mean of each 2×2 block within the region), region right,
/// bottom, the draft's copy of the input.
pub static DF_INIT: Kernel = Kernel {
    name: "diffuse_init_v2",
    atomic: false,
    helpers: &[],
    body: "    let f = pu(p + 5u);
    let x0 = pu(p + 2u) + f * t.x;
    let y0 = pu(p + 3u) + f * t.y;
    var c = vec3<f32>(0.0);
    var k = 0.0;
    for (var b = 0u; b < f; b++) {
        for (var a = 0u; a < f; a++) {
            if (x0 + a < pu(p + 6u) && y0 + b < pu(p + 7u)) {
                c += src_at(i32(x0 + a), i32(y0 + b)).rgb;
                k += 1.0;
            }
        }
    }
    c /= k;
    let i = pu(p) + 3u * (t.y * pu(p + 1u) + t.x);
    S[i] = c.x;
    S[i + 1u] = c.y;
    S[i + 2u] = c.z;
    if (f > 1u) {
        let o = pu(p + 8u) + 3u * (t.y * pu(p + 1u) + t.x);
        S[o] = c.x;
        S[o + 1u] = c.y;
        S[o + 2u] = c.z;
    }",
};

/// Shared WGSL of the diffusion steps (scratch `array<u32>`): an RGB pixel of an image,
/// f32 or, in the cascade (`hf`) of an f16 render, packed f16.
pub const DIFFUSE_WGSL: &str = "fn df_get(o: u32, i: u32, hf: bool) -> vec3<f32> {
    if (hf && STEP_F16) {
        let a = S[o + 2u * i];
        let b = S[o + 2u * i + 1u];
        return vec3<f32>(unpack2x16float(a), unpack2x16float(b).x);
    }
    let j = o + 3u * i;
    return vec3<f32>(bitcast<f32>(S[j]), bitcast<f32>(S[j + 1u]), bitcast<f32>(S[j + 2u]));
}
fn df_set(o: u32, i: u32, hf: bool, v: vec3<f32>) {
    if (hf && STEP_F16) {
        S[o + 2u * i] = pack2x16float(v.xy);
        S[o + 2u * i + 1u] = pack2x16float(vec2<f32>(v.z, 0.0));
        return;
    }
    let j = o + 3u * i;
    S[j] = bitcast<u32>(v.x);
    S[j + 1u] = bitcast<u32>(v.y);
    S[j + 2u] = bitcast<u32>(v.z);
}
fn df_at(o: u32, w: u32, h: u32, hf: bool, x: i32, y: i32) -> vec3<f32> {
    let cx = u32(clamp(x, 0, i32(w) - 1));
    let cy = u32(clamp(y, 0, i32(h) - 1));
    return df_get(o, cy * w + cx, hf);
}
fn df_px(o: u32, w: u32, h: u32, n: u32, x: i32, y: i32) -> vec3<f32> {
    return df_at(o, w, h, false, x, y);
}
const DF_Y = vec3<f32>(0.2627, 0.6780, 0.0593);
fn df_clamp(uo: u32, w: u32, h: u32, n: u32, x: i32, y: i32, hf: u32, hc: u32, mg: f32,
    ch: vec3<f32>) -> vec3<f32> {
    var lo = vec3<f32>(3.0e38);
    var hi = vec3<f32>(-3.0e38);
    for (var l = 0u; l < 2u; l++) {
        let s = i32(select(hf, hc, l == 1u));
        for (var j = -1; j <= 1; j++) {
            for (var k = -1; k <= 1; k++) {
                let v = df_px(uo, w, h, n, x + k * s, y + j * s);
                lo = min(lo, v);
                hi = max(hi, v);
            }
        }
    }
    let u = df_px(uo, w, h, n, x, y);
    let m = mg * (hi - lo);
    return clamp(u + ch, max(lo - m, min(lo, vec3<f32>(0.0))), hi + m);
}
fn df_add(up: vec3<f32>, sp: f32, gate: f32, d: vec3<f32>) -> vec3<f32> {
    if (sp > 0.0) { return up + sp * d; }
    return up + (sp * gate) * d;
}
fn df_term(xx: vec3<f32>, yy: vec3<f32>, xy: vec3<f32>, g: vec2<f32>, e: f32, a: f32) -> vec3<f32> {
    let gl = length(g);
    var nv = vec2<f32>(1.0, 0.0);
    if (gl > 1e-12) { nv = g / gl; }
    let nn = nv.x * nv.x * xx + 2.0 * nv.x * nv.y * xy + nv.y * nv.y * yy;
    let tt = xx + yy - nn;
    let wn = 1.0 - max(a, 0.0) * (1.0 - e);
    let wt = 1.0 - max(-a, 0.0) * (1.0 - e);
    return wn * nn + wt * tt;
}
";

/// Shared WGSL of the level and band steps: a picture read through a level descriptor at
/// word `q` (offset, row width, x and y of the reader's first sample, shift, kind: 0 f32
/// in the scratch, 1 the cascade (packed f16 in f16 renders), 2 the input picture), at a
/// reader's pixel clamped to its grid (`wm`×`hm`).
pub const LEVEL_WGSL: &str = "fn df_lv(q: u32, x: u32, y: u32) -> vec3<f32> {
    let d = pu(q + 4u);
    let sx = pu(q + 2u) + (x << d);
    let sy = pu(q + 3u) + (y << d);
    let k = pu(q + 5u);
    if (k == 2u) {
        return src_at(i32(pu(16u) + sx), i32(pu(17u) + sy)).rgb;
    }
    return df_get(pu(q), sy * pu(q + 1u) + sx, k == 1u);
}
fn df_tap(q: u32, wm: u32, hm: u32, x: i32, y: i32) -> vec3<f32> {
    let cx = u32(clamp(x, 0, i32(wm) - 1));
    let cy = u32(clamp(y, 0, i32(hm) - 1));
    return df_lv(q, cx, cy);
}
";

/// One à-trous B3 level on a grid. Words: destination, grid width, height, spacing (grid
/// pixels), the source's level descriptor (6 words, see [`LEVEL_WGSL`]), destination in
/// the cascade, highlight mask on, mask, tiles per row, first row (a step covers a
/// stripe of rows), the destination's first row (stripe mode; else 0).
pub static DF_LEVEL: Kernel = Kernel {
    name: "diffuse_level_v2",
    atomic: false,
    helpers: &[DIFFUSE_WGSL, LEVEL_WGSL],
    body: "//u32
    let wm = pu(p + 1u);
    let hm = pu(p + 2u);
    let st = i32(pu(p + 3u));
    let ty = t.y + pu(p + 14u);
    if (pu(p + 11u) != 0u
        && bitcast<f32>(S[pu(p + 12u) + (ty / 16u) * pu(p + 13u) + t.x / 16u]) == 0.0) {
        return;
    }
    var acc = vec3<f32>(0.0);
    for (var j = -2; j <= 2; j++) {
        let hy = select(select(0.0625, 0.25, abs(j) == 1), 0.375, j == 0);
        var row = vec3<f32>(0.0);
        for (var k = -2; k <= 2; k++) {
            let hx = select(select(0.0625, 0.25, abs(k) == 1), 0.375, k == 0);
            row += hx * df_tap(p + 4u, wm, hm, i32(t.x) + k * st, i32(ty) + j * st);
        }
        acc += hy * row;
    }
    df_set(pu(p), (ty - pu(p + 15u)) * wm + t.x, pu(p + 10u) != 0u, acc);",
};

/// Stripe mode: `L_1` (the cascade's first level, spacing 1) on a reduced grid, read from
/// `u` on the picture's grid (as [`DF_LEVEL`] at those pixels, the same sums in the same
/// order). Words: destination, grid width, height, its first sample (x, y, picture pixels),
/// its shift, picture width, height, `u`'s level descriptor (6, as the picture's grid reads
/// it), destination in the cascade, first row (a step covers a stripe of rows).
pub static DF_LEVEL_SUB: Kernel = Kernel {
    name: "diffuse_level_sub_v1",
    atomic: false,
    helpers: &[DIFFUSE_WGSL, LEVEL_WGSL],
    body: "//u32
    let m = pu(p + 5u);
    let x = i32(pu(p + 3u) + (t.x << m));
    let ty = t.y + pu(p + 15u);
    let y = i32(pu(p + 4u) + (ty << m));
    let wm = pu(p + 6u);
    let hm = pu(p + 7u);
    var acc = vec3<f32>(0.0);
    for (var j = -2; j <= 2; j++) {
        let hy = select(select(0.0625, 0.25, abs(j) == 1), 0.375, j == 0);
        var row = vec3<f32>(0.0);
        for (var k = -2; k <= 2; k++) {
            let hx = select(select(0.0625, 0.25, abs(k) == 1), 0.375, k == 0);
            row += hx * df_tap(p + 8u, wm, hm, x + k, y + j);
        }
        acc += hy * row;
    }
    df_set(pu(p), ty * pu(p + 1u) + t.x, pu(p + 14u) != 0u, acc);",
};

/// Stripe mode: rows of the next iterate copied from a stripe's buffer into the iterate.
/// Words: source, destination, words per row.
pub static DF_COPY: Kernel = Kernel {
    name: "diffuse_copy_v1",
    atomic: false,
    helpers: &[],
    body: "//u32
    let i = t.y * pu(p + 2u) + t.x;
    S[pu(p + 1u) + i] = S[pu(p) + i];",
};

/// The largest spacing [`DF_LEVEL_ROWS`] takes (its rows fill workgroup memory).
const ROWS_MAX_SPACING: u32 = 8;

/// [`DF_LEVEL`] (no highlight mask, spacing ≤ [`ROWS_MAX_SPACING`]) per workgroup of 16×16
/// pixels: the horizontal sums of the rows the vertical taps need go to workgroup memory
/// once (10 reads a pixel instead of 25), the same sums in the same order, so the same
/// values. Same words.
pub static DF_LEVEL_ROWS: Kernel = Kernel {
    name: "diffuse_level_rows_v2",
    atomic: false,
    helpers: &[DIFFUSE_WGSL, LEVEL_WGSL, LEVEL_ROWS_WGSL],
    body: "//u32
//workgroup
    let wm = pu(p + 1u);
    let hm = pu(p + 2u);
    let st = i32(pu(p + 3u));
    let tx = pu(2u) / 16u;
    let x0 = i32((w % tx) * 16u);
    let y0 = i32((w / tx) * 16u + pu(p + 14u));
    let rows = u32(16 + 4 * st);
    for (var k = li; k < rows * 16u; k += 256u) {
        let x = x0 + i32(k % 16u);
        let y = y0 - 2 * st + i32(k / 16u);
        var row = vec3<f32>(0.0);
        for (var a = -2; a <= 2; a++) {
            let hx = select(select(0.0625, 0.25, abs(a) == 1), 0.375, a == 0);
            row += hx * df_tap(p + 4u, wm, hm, x + a * st, y);
        }
        DF_ROWS[k] = row;
    }
    workgroupBarrier();
    let c = li % 16u;
    let r = li / 16u;
    let x = u32(x0) + c;
    let y = u32(y0) + r;
    var acc = vec3<f32>(0.0);
    for (var j = -2; j <= 2; j++) {
        let hy = select(select(0.0625, 0.25, abs(j) == 1), 0.375, j == 0);
        acc += hy * DF_ROWS[u32(i32(r) + (2 + j) * st) * 16u + c];
    }
    if (x < wm && y < hm) {
        df_set(pu(p), (y - pu(p + 15u)) * wm + x, pu(p + 10u) != 0u, acc);
    }",
};

/// Workgroup memory of [`DF_LEVEL_ROWS`]: 16 + 4·8 rows of 16 sums.
const LEVEL_ROWS_WGSL: &str = "var<workgroup> DF_ROWS: array<vec3<f32>, 768>;
";

/// The bands of one grid in one pass. Words: grid width, height, destination, final (the
/// grid of the picture: writes the next iterate, held within the range of `u`; else the
/// grid's sum of changes), `u`'s level descriptor (6), coarser sum present, its offset,
/// width, height, its first sample in this grid's pixels (x, y), speeds 1–4, anisotropies
/// 1–4, 1/K, edge threshold, highlights, finest and coarsest spacing (picture pixels),
/// margin, bands, first row (a step covers a stripe of rows); per band (14 words):
/// weight·dt, spacing (grid pixels), `L_j`'s and `L_{j+1}`'s level descriptors.
pub static DF_BANDS: Kernel = Kernel {
    name: "diffuse_bands_v3",
    atomic: false,
    helpers: &[DIFFUSE_WGSL, LEVEL_WGSL, BANDS_WGSL],
    body: "//u32
    let wm = pu(p);
    let hm = pu(p + 1u);
    let ty = t.y + pu(p + 31u);
    let x = i32(t.x);
    let y = i32(ty);
    let i = ty * wm + t.x;
    let fin = pu(p + 3u) != 0u;
    let hl = pf(p + 26u);
    // Highlights only (the picture's grid alone): a pixel below their smooth step keeps
    // its value.
    if (fin && hl > 0.0) {
        let c = df_lv(p + 4u, t.x, ty);
        if (max(c.x, max(c.y, c.z)) <= 0.8 * hl) {
            df_set(pu(p + 2u), i, false, c);
            return;
        }
    }
    var acc = vec3<f32>(0.0);
    let nb = pu(p + 30u);
    let sp = vec4<f32>(pf(p + 16u), pf(p + 17u), pf(p + 18u), pf(p + 19u));
    let hf = any(sp.yzw != vec3<f32>(0.0));
    for (var b = 0u; b < nb; b++) {
        let q = p + 32u + 14u * b;
        let s = i32(pu(q + 1u));
        // The stencil's columns and rows, clamped to the grid once.
        let xs = array<u32, 3>(u32(max(x - s, 0)), t.x, u32(min(x + s, i32(wm) - 1)));
        let ys = array<u32, 3>(u32(max(y - s, 0)), ty, u32(min(y + s, i32(hm) - 1)));
        var l: array<vec3<f32>, 9>;
        var pp: array<vec3<f32>, 9>;
        for (var k = 0u; k < 9u; k++) {
            l[k] = df_lv(q + 8u, xs[k % 3u], ys[k / 3u]);
        }
        // The high-frequency taps only when a term reads them.
        if (hf) {
            for (var k = 0u; k < 9u; k++) {
                pp[k] = df_lv(q + 2u, xs[k % 3u], ys[k / 3u]);
            }
        }
        var up = df_band(p, l, pp, hf);
        if (hl > 0.0) {
            let c = df_lv(p + 4u, t.x, ty);
            up *= smoothstep(0.8 * hl, hl, max(c.x, max(c.y, c.z)));
        }
        up *= pf(q);
        if (b != 0u) { up += acc; }
        acc = up;
    }
    df_finish(p, x, y, i, fin, acc);",
};

/// [`DF_BANDS`] per workgroup of 16×16 pixels (no highlights): the taps of each band whose
/// spacing is at most [`BANDS_WG_MAX_SPACING`] grid pixels are read once per workgroup
/// into workgroup memory (the clamped picture positions the stencils read, so the same
/// values in the same sums: bit for bit the picture of [`DF_BANDS`]). Same words; whole
/// workgroups (a stripe's width and rows rounded up to 16).
pub static DF_BANDS_WG: Kernel = Kernel {
    name: "diffuse_bands_wg_v2",
    atomic: false,
    helpers: &[DIFFUSE_WGSL, LEVEL_WGSL, BANDS_WGSL, BANDS_WG_WGSL],
    body: "//u32
//workgroup
    let wm = pu(p);
    let hm = pu(p + 1u);
    let gx = pu(2u) / 16u;
    let x0 = (w % gx) * 16u;
    let y0 = (w / gx) * 16u + pu(p + 31u);
    let c = li % 16u;
    let r = li / 16u;
    let tx = x0 + c;
    let ty = y0 + r;
    let ok = tx < wm && ty < hm;
    let x = i32(tx);
    let y = i32(ty);
    let i = ty * wm + tx;
    let fin = pu(p + 3u) != 0u;
    var acc = vec3<f32>(0.0);
    let nb = pu(p + 30u);
    let sp = vec4<f32>(pf(p + 16u), pf(p + 17u), pf(p + 18u), pf(p + 19u));
    let hf = any(sp.yzw != vec3<f32>(0.0));
    for (var b = 0u; b < nb; b++) {
        let q = p + 32u + 14u * b;
        let s = i32(pu(q + 1u));
        var l: array<vec3<f32>, 9>;
        var pp: array<vec3<f32>, 9>;
        if (s <= 4) {
            // The tile and its halo, clamped to the grid as the stencils clamp.
            let side = 16u + 2u * u32(s);
            for (var k = li; k < side * side; k += 256u) {
                let ax = u32(clamp(i32(x0 + k % side) - s, 0, i32(wm) - 1));
                let ay = u32(clamp(i32(y0 + k / side) - s, 0, i32(hm) - 1));
                let a = df_lv(q + 8u, ax, ay);
                DF_TL[3u * k] = a.x;
                DF_TL[3u * k + 1u] = a.y;
                DF_TL[3u * k + 2u] = a.z;
                if (hf) {
                    let e = df_lv(q + 2u, ax, ay);
                    DF_TP[3u * k] = e.x;
                    DF_TP[3u * k + 1u] = e.y;
                    DF_TP[3u * k + 2u] = e.z;
                }
            }
            workgroupBarrier();
            for (var k = 0u; k < 9u; k++) {
                let o = 3u * ((r + u32(s) * (k / 3u)) * side + c + u32(s) * (k % 3u));
                l[k] = vec3<f32>(DF_TL[o], DF_TL[o + 1u], DF_TL[o + 2u]);
                if (hf) {
                    pp[k] = vec3<f32>(DF_TP[o], DF_TP[o + 1u], DF_TP[o + 2u]);
                }
            }
            workgroupBarrier();
        } else {
            let xs = array<u32, 3>(u32(max(x - s, 0)), tx, u32(min(x + s, i32(wm) - 1)));
            let ys = array<u32, 3>(u32(max(y - s, 0)), ty, u32(min(y + s, i32(hm) - 1)));
            for (var k = 0u; k < 9u; k++) {
                l[k] = df_lv(q + 8u, xs[k % 3u], ys[k / 3u]);
            }
            if (hf) {
                for (var k = 0u; k < 9u; k++) {
                    pp[k] = df_lv(q + 2u, xs[k % 3u], ys[k / 3u]);
                }
            }
        }
        var up = df_band(p, l, pp, hf);
        up *= pf(q);
        if (b != 0u) { up += acc; }
        acc = up;
    }
    if (ok) {
        df_finish(p, x, y, i, fin, acc);
    }",
};

/// The largest band spacing (grid pixels) [`DF_BANDS_WG`] reads from workgroup memory.
pub const BANDS_WG_MAX_SPACING: u32 = 4;

/// Workgroup memory of [`DF_BANDS_WG`]: a 24×24 tile (16 and a halo of 4) of `L_{j+1}`
/// and of `L_j`, three f32 each.
const BANDS_WG_WGSL: &str = "var<workgroup> DF_TL: array<f32, 1728>;
var<workgroup> DF_TP: array<f32, 1728>;
";

/// One band's change at a pixel from its 3×3 taps of `L_{j+1}` (`l`) and `L_j` (`pp`,
/// read when `hf`), and the change's end: the coarser grid's sum and, on the picture's
/// grid, the next iterate held within the range of `u`.
const BANDS_WGSL: &str = "fn df_band(p: u32, l: array<vec3<f32>, 9>, pp: array<vec3<f32>, 9>,
    hf: bool) -> vec3<f32> {
    let sp = vec4<f32>(pf(p + 16u), pf(p + 17u), pf(p + 18u), pf(p + 19u));
    let lxx = l[5] - 2.0 * l[4] + l[3];
    let lyy = l[7] - 2.0 * l[4] + l[1];
    let lxy = 0.25 * (l[8] - l[6] - l[2] + l[0]);
    let gl = 0.5 * vec2<f32>(dot(l[5] - l[3], DF_Y), dot(l[7] - l[1], DF_Y));
    let lev = max(dot(l[4], DF_Y), 0.0) + 1e-3;
    let ik = pf(p + 24u);
    let rl = length(gl) / lev;
    let el = 1.0 / (1.0 + rl * ik * rl * ik);
    let thr = pf(p + 25u);
    var gate = 1.0;
    if (thr > 0.0) { gate = smoothstep(thr, 2.0 * thr, rl); }
    // A term whose speed is 0 adds nothing (0·d, d finite), so it is not computed.
    var up = vec3<f32>(0.0);
    if (sp.x != 0.0) {
        up = df_add(up, sp.x, gate, df_term(lxx, lyy, lxy, gl, el, pf(p + 20u)));
    }
    if (hf) {
        var h: array<vec3<f32>, 9>;
        for (var k = 0u; k < 9u; k++) {
            h[k] = pp[k] - l[k];
        }
        let hxx = h[5] - 2.0 * h[4] + h[3];
        let hyy = h[7] - 2.0 * h[4] + h[1];
        let hxy = 0.25 * (h[8] - h[6] - h[2] + h[0]);
        let gh = 0.5 * vec2<f32>(dot(h[5] - h[3], DF_Y), dot(h[7] - h[1], DF_Y));
        let rh = length(gh) / lev;
        let eh = 1.0 / (1.0 + rh * ik * rh * ik);
        if (sp.y != 0.0) {
            up = df_add(up, sp.y, gate, df_term(lxx, lyy, lxy, gh, eh, pf(p + 21u)));
        }
        if (sp.z != 0.0) {
            up = df_add(up, sp.z, gate, df_term(hxx, hyy, hxy, gl, el, pf(p + 22u)));
        }
        if (sp.w != 0.0) {
            up = df_add(up, sp.w, gate, df_term(hxx, hyy, hxy, gh, eh, pf(p + 23u)));
        }
    }
    return up;
}
fn df_finish(p: u32, x: i32, y: i32, i: u32, fin: bool, acc0: vec3<f32>) {
    var acc = acc0;
    let wm = pu(p);
    let hm = pu(p + 1u);
    // The coarser grid's sum, bilinear (its samples are every other pixel here).
    if (pu(p + 10u) != 0u) {
        let co = pu(p + 11u);
        let cw = pu(p + 12u);
        let ch = pu(p + 13u);
        let fx = clamp((f32(x) - f32(pu(p + 14u))) * 0.5, 0.0, f32(cw - 1u));
        let fy = clamp((f32(y) - f32(pu(p + 15u))) * 0.5, 0.0, f32(ch - 1u));
        let ix = u32(fx);
        let iy = u32(fy);
        let ax = fx - f32(ix);
        let ay = fy - f32(iy);
        let jx = min(ix + 1u, cw - 1u);
        let jy = min(iy + 1u, ch - 1u);
        let a = mix(df_get(co, iy * cw + ix, false), df_get(co, iy * cw + jx, false), ax);
        let c = mix(df_get(co, jy * cw + ix, false), df_get(co, jy * cw + jx, false), ax);
        var cs = mix(a, c, ay);
        // Highlights: the reduced grids' change is gated here, as the bands'.
        let hl = pf(p + 26u);
        if (fin && hl > 0.0) {
            let v = df_tap(p + 4u, wm, hm, x, y);
            cs *= smoothstep(0.8 * hl, hl, max(v.x, max(v.y, v.z)));
        }
        acc += cs;
    }
    if (!fin) {
        df_set(pu(p + 2u), i, false, acc);
        return;
    }
    // The next iterate: `u + change` held within the range of `u` over both 3×3 stencils,
    // widened by the margin.
    var lo = vec3<f32>(3.0e38);
    var hi = vec3<f32>(-3.0e38);
    for (var l = 0u; l < 2u; l++) {
        let s = i32(select(pu(p + 27u), pu(p + 28u), l == 1u));
        for (var j = -1; j <= 1; j++) {
            for (var k = -1; k <= 1; k++) {
                let v = df_tap(p + 4u, wm, hm, x + k * s, y + j * s);
                lo = min(lo, v);
                hi = max(hi, v);
            }
        }
    }
    let u = df_tap(p + 4u, wm, hm, x, y);
    let m = pf(p + 29u) * (hi - lo);
    df_set(pu(p + 2u), i, false, clamp(u + acc, max(lo - m, min(lo, vec3<f32>(0.0))), hi + m));
}
";
/// Diffuse or sharpen, param version 1.
pub struct DiffuseOp;

impl Op for DiffuseOp {
    fn name(&self) -> &'static str {
        "diffuse_v3"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Neighbourhood
    }
    fn halo(&self, p: &Params, _scale: f32) -> u32 {
        plan(&Diffuse::of(p)).halo()
    }
    fn heavy(&self) -> bool {
        true
    }
    fn param_words(&self) -> u32 {
        3
    }
    fn write_params(&self, p: &Params, _scale: f32, out: &mut [u32]) {
        let d = Diffuse::of(p);
        out[0] = result_image(&d);
        out[1] = u32::from(plan(&d).half);
        out[2] = HALF_COPY;
        #[cfg(test)]
        if LEGACY.load(std::sync::atomic::Ordering::Relaxed) {
            out[2] = 12;
        }
    }
    fn identity(&self, p: &Params) -> bool {
        Diffuse::of(p).is_identity()
    }
    fn place(&self, p: &Params, at: &Placement<'_>) -> Option<Params> {
        let mut f = p.to_fields();
        f.set(LONG_FIELD, Value::Float(at.image.0.max(at.image.1) as f32));
        f.set(DRAFT_FIELD, Value::Bool(at.draft));
        Some(Params::from_fields(ModuleKind::DIFFUSE, f))
    }
    fn local(&self, p: &Params, input: Roi, _scale: f32) -> Option<Program> {
        Some(program(&Diffuse::of(p), input))
    }
    fn wgsl(&self) -> &'static str {
        "    let c = src_at(x, y);
    let lx = x - i32(pu(16u));
    let ly = y - i32(pu(17u));
    if (pu(p + 1u) == 0u) {
        let rw = pu(18u);
        let n = rw * pu(19u);
        let i = 3u * (n * pu(p) + u32(ly) * rw + u32(lx));
        return vec4<f32>(lc(i), lc(i + 1u), lc(i + 2u), c.a);
    }
    // A draft: the half-size change, scaled up bilinearly.
    let gw = (pu(18u) + 1u) / 2u;
    let gh = (pu(19u) + 1u) / 2u;
    let n = gw * gh;
    let r = 3u * n * pu(p);
    let fx = clamp((f32(lx) + 0.5) * 0.5 - 0.5, 0.0, f32(gw - 1u));
    let fy = clamp((f32(ly) + 0.5) * 0.5 - 0.5, 0.0, f32(gh - 1u));
    let ix = u32(fx);
    let iy = u32(fy);
    let ax = fx - f32(ix);
    let ay = fy - f32(iy);
    let jx = min(ix + 1u, gw - 1u);
    let jy = min(iy + 1u, gh - 1u);
    var d = vec3<f32>(0.0);
    for (var k = 0u; k < 4u; k++) {
        let qx = select(ix, jx, (k & 1u) == 1u);
        let qy = select(iy, jy, (k & 2u) == 2u);
        let wt = select(1.0 - ax, ax, (k & 1u) == 1u) * select(1.0 - ay, ay, (k & 2u) == 2u);
        let i = 3u * (qy * gw + qx);
        let o = pu(p + 2u) * n + i;
        d += wt * vec3<f32>(lc(r + i) - lc(o), lc(r + i + 1u) - lc(o + 1u),
            lc(r + i + 2u) - lc(o + 2u));
    }
    return vec4<f32>(c.rgb + d, c.a);"
    }
}
