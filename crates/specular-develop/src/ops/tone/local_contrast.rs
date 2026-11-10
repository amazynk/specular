//! Local contrast: the fast local Laplacian filter (Aubry, Paris,
//! Hasinoff, Kautz and Durand 2014) on `log2` luminance, with every channel scaled by the
//! change, so hue and saturation stay.
//!
//! The remapping `r(i) = i + amount·d·(1 − (d/2σ)²)²` with `d = i − γ` (zero from
//! |d| = 2σ on) boosts (amount > 0) or smooths (amount < 0) differences below about σ EV
//! around each γ and leaves differences over 2σ, the edges, exactly as they are: no halos.
//! (Aubry et al.'s Gaussian remapping has tails that lift both sides of a strong edge.) γ takes K samples over the image's range (its
//! 0.5–99.5th percentiles from the statistics grid, padded by 1 EV), and each pyramid
//! coefficient interpolates the two samples that bracket its Gaussian level value.
//!
//! - **Texture** (fine scale): the filter over the node's input region with up to three
//!   levels (details up to about 8 source pixels), as a multi-scale node. Its pyramids are
//!   anchored to image coordinates and its halo covers every level, so tiles match the
//!   whole image. At smaller view scales it uses fewer levels, so it always works on the
//!   same source-pixel scales.
//! - **Clarity** (coarse scale): the filter on the statistics grid, levels 1–6 (grid
//!   level 0 is left to texture), then brought to the pixel by guided upsampling: the
//!   change is fitted as `Ā·I + B̄` against the grid's luminance (He and Sun 2015), so it
//!   follows the full-resolution edges.
//! - Both at 0 is the identity, and their steps then do no work.

use super::kernels::{self, Alloc, BINS, fw, threads};
use crate::graph::{NodeKind, Op, Persist, Program, Roi};
use crate::model::{Fields, Params};

/// γ samples of the fast local Laplacian filter.
pub const SAMPLES: u32 = 12;
/// Texture levels at view scale 1 (details up to 2³ source pixels).
pub const TEXTURE_LEVELS: u32 = 3;
/// Most clarity levels on the grid.
pub const CLARITY_LEVELS: u32 = 6;
/// Range padding around the percentiles, EV.
const RANGE_PAD: f32 = 1.0;
/// Guided upsampling: radius in grid pixels and ε (EV²).
const UP_RADIUS: u32 = 2;
const UP_EPS: f32 = 0.01;

/// Local contrast params (fields of [`crate::model::ModuleKind::LOCAL_CONTRAST`],
/// version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct LocalContrast {
    /// Fine-scale detail, −1 (smooth) … 1 (twice the detail).
    pub texture: f32,
    /// Coarse-scale contrast, −1 … 1.
    pub clarity: f32,
    /// Detail threshold of texture and clarity, EV.
    pub texture_sigma: f32,
    pub clarity_sigma: f32,
}

impl Default for LocalContrast {
    fn default() -> Self {
        Self {
            texture: 0.0,
            clarity: 0.0,
            texture_sigma: 1.0,
            clarity_sigma: 1.0,
        }
    }
}

fn finite(v: f32, d: f32, lo: f32, hi: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { d }
}

impl LocalContrast {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        Self {
            texture: finite(f.f32_or("texture", 0.0), 0.0, -1.0, 1.0),
            clarity: finite(f.f32_or("clarity", 0.0), 0.0, -1.0, 1.0),
            texture_sigma: finite(
                f.f32_or("texture_sigma", d.texture_sigma),
                d.texture_sigma,
                0.05,
                4.0,
            ),
            clarity_sigma: finite(
                f.f32_or("clarity_sigma", d.clarity_sigma),
                d.clarity_sigma,
                0.05,
                4.0,
            ),
        }
    }
}

/// Texture levels at view `scale`: the same source-pixel scales at every view size.
pub fn texture_levels(scale: f32) -> u32 {
    if !(scale.is_finite() && scale > 0.0) {
        return 0;
    }
    (TEXTURE_LEVELS as f32 + scale.log2())
        .round()
        .clamp(0.0, TEXTURE_LEVELS as f32) as u32
}

/// The region the texture pyramids cover inside `input`: its start rounded up to a
/// multiple of 2^levels (unless at the image edge), so every level's samples sit on the
/// same image positions in every tile.
pub fn aligned(input: Roi, levels: u32) -> Roi {
    let al = (1u32 << levels) - 1;
    let up = |v: u32| if v == 0 { 0 } else { (v + al) & !al };
    let (x, y) = (up(input.x), up(input.y));
    Roi::new(x, y, input.right() - x, input.bottom() - y)
}

/// Statistics layout: 0–1 grid size, 2–3 γ₀ and Δγ, 4–5 percentiles; Ā of clarity from
/// word 16, B̄ after it.
pub const STATS_A: u32 = 16;

/// Local contrast, param version 1.
pub struct LocalContrastOp;

impl Op for LocalContrastOp {
    fn name(&self) -> &'static str {
        "local_contrast_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::MultiScale {
            levels: TEXTURE_LEVELS as u8,
        }
    }
    fn halo(&self, p: &Params, scale: f32) -> u32 {
        let l = texture_levels(scale);
        if l == 0 || LocalContrast::from_fields(&p.to_fields()).texture == 0.0 {
            // Without texture the kernel reads only its own pixel and the statistics.
            0
        } else {
            5 << l
        }
    }
    fn param_words(&self) -> u32 {
        6
    }
    fn write_params(&self, p: &Params, scale: f32, out: &mut [u32]) {
        let lc = LocalContrast::from_fields(&p.to_fields());
        out[0] = fw(lc.texture);
        out[1] = fw(lc.clarity);
        out[2] = texture_levels(scale);
        out[3] = SAMPLES;
        out[4] = fw(lc.texture);
        out[5] = fw(lc.texture_sigma);
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[
            kernels::COMMON_WGSL,
            kernels::GRID_WGSL,
            kernels::LEVEL0_WGSL,
            LC_WGSL,
        ]
    }
    fn identity(&self, p: &Params) -> bool {
        let lc = LocalContrast::from_fields(&p.to_fields());
        lc.texture == 0.0 && lc.clarity == 0.0
    }
    fn has_stats(&self, _: &Params) -> bool {
        true
    }
    fn stats(&self, p: &Params, w: u32, h: u32, _: f32) -> Option<Program> {
        Some(stats_program(
            &LocalContrast::from_fields(&p.to_fields()),
            w,
            h,
        ))
    }
    fn local(&self, p: &Params, input: Roi, scale: f32) -> Option<Program> {
        let lc = LocalContrast::from_fields(&p.to_fields());
        let levels = texture_levels(scale);
        (levels > 0).then(|| texture_program(&lc, input, levels))
    }
    fn wgsl(&self) -> &'static str {
        "    let c = src_at(x, y);
    let yl = dot(c.rgb, DV_LUMA);
    if (yl <= 0.0) { return c; }
    let l = log2(max(yl, 1.52587890625e-5));
    var d = 0.0;
    let tl = pu(p + 2u);
    if (pf(p) != 0.0 && tl > 0u) {
        let al = (1u << tl) - 1u;
        let rx = pu(16u);
        let ry = pu(17u);
        let ax = select((rx + al) & ~al, 0u, rx == 0u);
        let ay = select((ry + al) & ~al, 0u, ry == 0u);
        let aw = rx + pu(18u) - ax;
        let ah = ry + pu(19u) - ay;
        let lv = dv_region_levels(aw, ah, tl);
        if (lv > 0u) {
            // Words for dv_level0: levels, K, amount, σ (the last three are params 3–5).
            d += dv_lc_level0(l, u32(x) - ax, u32(y) - ay, aw, ah, lv, p) - l;
        }
    }
    if (pf(p + 1u) != 0.0) {
        let n = u32(st(0u)) * u32(st(1u));
        d += dv_grid(16u, x, y) * l + dv_grid(16u + n, x, y);
    }
    return vec4<f32>(c.rgb * exp2(d), c.a);"
    }
}

const LC_WGSL: &str = "fn dv_region_levels(w: u32, h: u32, cap: u32) -> u32 {
    var n = 0u;
    var a = w;
    var b = h;
    loop {
        if (n >= cap || (a + 1u) / 2u < 4u || (b + 1u) / 2u < 4u) { break; }
        a = (a + 1u) / 2u;
        b = (b + 1u) / 2u;
        n += 1u;
    }
    return n;
}
fn dv_lc_level0(g: f32, x: u32, y: u32, aw: u32, ah: u32, lv: u32, p: u32) -> f32 {
    let k = pu(p + 3u);
    let amt = pf(p + 4u);
    let sig = pf(p + 5u);
    let w1 = (aw + 1u) / 2u;
    let h1 = (ah + 1u) / 2u;
    let n1 = w1 * h1;
    let g1 = 16u;
    let r1 = g1 + n1;
    let out1 = select(g1, r1 + k * n1, lv >= 2u);
    let g0 = lc(0u);
    let dg = lc(1u);
    let jf = clamp((g - g0) / dg, 0.0, f32(k - 1u));
    let j0 = min(u32(floor(jf)), k - 2u);
    let f = jf - f32(j0);
    let a0 = dv_remap(g, g0 + f32(j0) * dg, amt, sig) - dv_lup(r1 + j0 * n1, w1, h1, x, y);
    let a1 = dv_remap(g, g0 + f32(j0 + 1u) * dg, amt, sig)
        - dv_lup(r1 + (j0 + 1u) * n1, w1, h1, x, y);
    return mix(a0, a1, f) + dv_lup(out1, w1, h1, x, y);
}
";

/// The statistics on a `w`×`h` grid: the γ range, and clarity's Ā, B̄.
pub fn stats_program(lc: &LocalContrast, w: u32, h: u32) -> Program {
    let n = u64::from(w) * u64::from(h);
    let mut a = Alloc(u64::from(STATS_A));
    let ab = a.take(2 * n);
    let l0 = a.take(n);
    let out0 = a.take(n);
    let hist = a.take(u64::from(BINS));
    // Without clarity only the range is used: no room for the filter.
    let base = a.0;
    let mut prog = Program::default();
    prog.step(&kernels::PUT, [2, 1], vec![0, fw(w as f32), fw(h as f32)]);
    prog.step(&kernels::LUM, [w, h], vec![l0, w, 0, 0]);
    prog.step(&kernels::HIST_CLEAR, [BINS, 1], vec![hist]);
    kernels::hist_add(
        &mut prog,
        true,
        (w, h),
        [0, l0, 0, 0, w, hist, fw(-24.0), fw(8.0), 0],
    );
    kernels::hist_pct(
        &mut prog,
        true,
        vec![hist, fw(-24.0), fw(8.0), 4, 2, fw(0.005), fw(0.995)],
    );
    prog.step(&kernels::RANGE, [1, 1], vec![4, 2, fw(RANGE_PAD), SAMPLES]);
    let levels = kernels::max_levels(w, h, CLARITY_LEVELS);
    let on = lc.clarity != 0.0 && levels > 1;
    kernels::local_laplacian(
        &mut prog,
        &mut a,
        l0,
        (0, 0),
        Some(out0),
        (w, h),
        levels.max(1),
        SAMPLES,
        2,
        lc.clarity,
        lc.clarity_sigma,
        1,
        on,
    );
    kernels::guided(
        &mut prog,
        &mut a,
        l0,
        out0,
        true,
        ab,
        (w, h),
        UP_RADIUS,
        UP_EPS,
        on,
    );
    prog.words = if on { a.0 } else { base };
    prog
}

/// The texture levels a region can hold (each level at least 4 pixels a side).
pub fn region_levels(r: Roi, levels: u32) -> u32 {
    levels.min(kernels::max_levels(r.w, r.h, levels))
}

/// Texture over `input`: the γ range from the statistics, then the filter over the
/// aligned region down to level 1; the kernel collapses level 0 (see
/// [`kernels::LEVEL0_WGSL`]). G₁, R₁, out₁ from word 16 (level 1 reads the luminance of
/// the input itself: no level-0 plane).
pub fn texture_program(lc: &LocalContrast, input: Roi, levels: u32) -> Program {
    let r = aligned(input, levels);
    let mut a = Alloc(16);
    let lv = region_levels(r, levels);
    let on = lc.texture != 0.0 && lv > 0;
    let mut prog = Program::default();
    prog.step(&kernels::COPY_ST, threads(on, 2, 1), vec![2, 0]);
    // Level 1 reads the input's luminance itself (no level-0 plane).
    kernels::local_laplacian(
        &mut prog,
        &mut a,
        kernels::NONE,
        (r.x, r.y),
        None,
        (r.w, r.h),
        lv.max(1),
        SAMPLES,
        0,
        lc.texture,
        lc.texture_sigma,
        0,
        on,
    );
    // Without texture no step does work: no room for the pyramids.
    prog.words = if on { a.0 } else { 16 };
    // The pyramids are a function of the input and the program (the γ range comes
    // from the input's statistics): kept, a later change resumes before local contrast
    // without making them again, and its kernel runs with the point groups after it.
    prog.persist = Some(Persist {
        reset: Vec::new(),
        reuse: true,
    });
    prog
}
