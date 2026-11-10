//! Denoise, in sensor geometry before the warp.
//!
//! 1. **Variance stabilisation.** Each channel goes through the generalised Anscombe
//!    transform for its noise `a·v + b` ([`super::noise`]), so its noise has unit variance
//!    whatever the signal, then into an orthonormal opponent space (luma `Y`, chroma `U`,
//!    `V`), which keeps the noise unit and white.
//! 2. **Wavelet shrinkage.** An edge-avoiding à-trous transform (B3 spline; weights fall
//!    with the opponent distance to the centre, after Hanika, Dammertz and Lensch 2011)
//!    splits each channel into detail bands; each band is soft-thresholded at a multiple of
//!    its own noise level (exact for the linear filter, [`kernels::band_noise`]). Luma and
//!    chroma have their own strengths and per-scale profiles: chroma noise is coarse, so
//!    chroma keeps its strength at coarse scales.
//! 3. **Non-local means** (Buades, Coll and Morel 2005) on the luma: each pixel becomes the
//!    mean of the pixels in a small search window whose surrounding patches look like its
//!    own, compared on the shrunk luma.
//! 4. **Back:** the inverse opponent transform, then per channel the exact unbiased inverse
//!    of the Anscombe transform (Mäkitalo and Foi 2013, closed form), blended towards the
//!    algebraic inverse as the strengths go to zero.
//!
//! **Scale.** The noise model is the source buffer's own (the caller carries it through
//! binning and reductions, [`super::with_image`]). At view scale `s` the transform uses
//! `5 + log2 s` levels, and level `j` takes the strength of source level `j − log2 s`, so a
//! preview treats the same image structures as the full-size render. The search window is
//! smaller below scale ¾.
//!
//! **Detail** keeps fine luma texture: it narrows the edge-avoiding weights, lowers the
//! finest band's threshold and the non-local means strength.
//!
//! Luminance and colour at 0 is the identity (the node is left out), unless the AI mode
//! mixes in a network's result ([`super::ai`]): then the classic result is what the
//! network's result is blended with, and the draft where it is not there yet.

use std::sync::Arc;

use super::ai::{self, AiParams};
use super::kernels::{self, ATROUS, ATROUS_0, ATROUS_WG, NLM, NLM_WG, VST, band_noise, fw};
use super::noise::NoiseModel;
use crate::graph::{NodeKind, Op, OpData, Placement, Program, Roi};
use crate::model::{Fields, Params, Value};

/// Wavelet levels at view scale 1.
pub const LEVELS: u32 = 5;
/// Soft threshold at strength 1, in units of the band's noise.
const T_LUMA: f64 = 3.0;
const T_CHROMA: f64 = 3.5;
/// Per-level strength profiles, by source level.
const PROFILE_LUMA: [f64; 6] = [1.0, 0.8, 0.55, 0.35, 0.2, 0.1];
const PROFILE_CHROMA: [f64; 6] = [1.0, 1.0, 1.0, 0.9, 0.8, 0.7];
/// Non-local means: h at strength 1 (shrunk-luma units).
const NLM_H: f32 = 0.9;
/// Non-local means patch radius.
const PATCH: u32 = 1;
/// Non-local means search radius of a draft (the module dragged, or a photo's first
/// picture): 9 candidates instead of 49 or 81 (progressive
/// refinement).
const DRAFT_SEARCH: u32 = 1;
/// Hidden field set on a draft's params ([`Op::place`]); never stored.
const DRAFT_FIELD: &str = "draft_search";

/// Denoise params (fields of [`crate::model::ModuleKind::DENOISE`], version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Denoise {
    /// Luma strength, 0 … 1.
    pub luminance: f32,
    /// Chroma strength, 0 … 1.
    pub colour: f32,
    /// Fine luma detail kept, 0 … 1.
    pub detail: f32,
    /// Non-local means on the luma after the wavelets.
    pub nlm: bool,
    /// The source buffer's noise (set per image by the caller; the fallback without).
    pub noise: NoiseModel,
    /// A draft: the smaller non-local means search ([`DRAFT_SEARCH`]).
    pub draft: bool,
}

impl Default for Denoise {
    fn default() -> Self {
        Self {
            luminance: 0.0,
            colour: 0.0,
            detail: 0.5,
            nlm: true,
            noise: NoiseModel::FALLBACK,
            draft: false,
        }
    }
}

fn unit(f: &Fields, name: &str, d: f32) -> f32 {
    let v = f.f32_or(name, d);
    if v.is_finite() { v.clamp(0.0, 1.0) } else { d }
}

impl Denoise {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        Self {
            luminance: unit(f, "luminance", d.luminance),
            colour: unit(f, "colour", d.colour),
            detail: unit(f, "detail", d.detail),
            nlm: f.bool_or("nlm", d.nlm),
            noise: NoiseModel::from_fields(f).unwrap_or(d.noise).sane(),
            draft: f.bool_or(DRAFT_FIELD, false),
        }
    }

    /// The registry's default fields (the noise model is per image, not stored).
    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("luminance", Value::Float(d.luminance))
            .with("colour", Value::Float(d.colour))
            .with("detail", Value::Float(d.detail))
            .with("nlm", Value::Bool(d.nlm))
    }

    fn of(p: &Params) -> Self {
        Self::from_fields(&p.to_fields())
    }

    /// The non-local means search radius at view `scale`.
    fn search(&self, scale: f32) -> u32 {
        if self.draft {
            DRAFT_SEARCH
        } else {
            search(scale)
        }
    }

    /// Whether non-local means runs.
    fn nlm_on(&self) -> bool {
        self.nlm && self.luminance > 0.0
    }
}

/// Wavelet levels at view `scale` (at least one).
pub fn levels(scale: f32) -> u32 {
    if !(scale.is_finite() && scale > 0.0) {
        return 1;
    }
    (LEVELS as f32 + scale.log2())
        .round()
        .clamp(1.0, LEVELS as f32) as u32
}

/// The non-local means search radius at view `scale`.
pub fn search(scale: f32) -> u32 {
    if scale >= 0.75 { 4 } else { 3 }
}

/// Input pixels the denoiser reads around each output pixel.
pub fn halo(d: &Denoise, scale: f32) -> u32 {
    let j = levels(scale);
    let wav = 2 * ((1u32 << j) - 1);
    let nlm = if d.nlm_on() {
        d.search(scale) + PATCH
    } else {
        0
    };
    wav + nlm + 1
}

/// Per channel `1/a`, `b/a²` and the inverse's blend, as the program and kernel use them.
fn vst_words(d: &Denoise) -> ([f32; 3], [f32; 3], f32) {
    let n = d.noise;
    let ia = n.a.map(|a| 1.0 / a);
    let s2: [f32; 3] = std::array::from_fn(|c| n.b[c] / (n.a[c] * n.a[c]));
    (ia, s2, d.luminance.max(d.colour).min(1.0))
}

/// The denoise program over the `input` region at `scale`: the transform into three planes
/// (offset 0), the wavelet levels ping-ponging between planes 0 and 3n into the
/// accumulator at 6n, then non-local means from the accumulator's luma into plane 0.
pub fn program(d: &Denoise, input: Roi, scale: f32) -> Program {
    let (w, h) = (input.w, input.h);
    let n = w * h;
    let (ia, s2, _) = vst_words(d);
    let mut prog = Program::default();
    let j_max = levels(scale);
    let off = LEVELS - j_max;
    let noise = band_noise(j_max);
    let detail = f64::from(d.detail);
    let kr = 1.0 + 8.0 * (1.0 - detail);
    // Per level: thresholds Y, U, V and 1/σ_r².
    let level = |j: u32| -> [u32; 4] {
        let (sd, _) = noise[j as usize];
        let ss = if j == 0 { 1.0 } else { noise[j as usize - 1].1 };
        let src = (j + off) as usize;
        let fine = if src == 0 { 1.0 - 0.5 * detail } else { 1.0 };
        let ty = f64::from(d.luminance) * T_LUMA * PROFILE_LUMA[src] * sd * fine;
        let tc = f64::from(d.colour) * T_CHROMA * PROFILE_CHROMA[src] * sd;
        let ir = 1.0 / (kr * (6.0 * ss * ss + 1e-4));
        [fw(ty as f32), fw(tc as f32), fw(tc as f32), fw(ir as f32)]
    };
    // The levels per workgroup (the same values): the transform and the first level
    // in one pass, the others on their lattice.
    #[cfg(test)]
    let plain = PLAIN_ATROUS.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(test))]
    let plain = false;
    if !plain {
        let mut words = vec![3 * n, 6 * n, w, h, n, input.x, input.y];
        words.extend(ia.map(fw));
        words.extend(s2.map(fw));
        words.extend(level(0));
        words.push(u32::from(j_max == 1));
        prog.step(
            &ATROUS_0,
            [w.next_multiple_of(16), h.next_multiple_of(16)],
            words,
        );
    } else {
        prog.step(
            &VST,
            [w, h],
            vec![
                0,
                w,
                input.x,
                input.y,
                fw(ia[0]),
                fw(ia[1]),
                fw(ia[2]),
                fw(s2[0]),
                fw(s2[1]),
                fw(s2[2]),
                n,
            ],
        );
    }
    for j in (if plain { 0 } else { 1 })..j_max {
        let (cur, next) = if j % 2 == 0 { (0, 3 * n) } else { (3 * n, 0) };
        let (kernel, threads) = if plain || (1 << j) > kernels::ATROUS_WG_MAX_STEP {
            (&ATROUS, [w, h])
        } else {
            (&ATROUS_WG, kernels::atrous_wg_threads(w, h, 1 << j))
        };
        let mut words = vec![cur, next, 6 * n, w, h, n, 1 << j];
        words.extend(level(j));
        words.extend([u32::from(j == 0), u32::from(j + 1 == j_max)]);
        prog.step(kernel, threads, words);
    }
    let on = d.nlm_on();
    let hh = NLM_H * d.luminance * (1.0 - 0.5 * d.detail);
    let search = d.search(scale);
    // The workgroup form when its memory holds the reach (the same values).
    let wg = search + PATCH <= kernels::NLM_WG_REACH && PATCH == kernels::NLM_WG_PATCH;
    #[cfg(test)]
    let wg = wg && !PLAIN_NLM.load(std::sync::atomic::Ordering::Relaxed);
    let (kernel, threads) = if wg {
        (&NLM_WG, [w.next_multiple_of(16), h.next_multiple_of(16)])
    } else {
        (&NLM, [w, h])
    };
    prog.step(
        kernel,
        if on { threads } else { [0, 0] },
        vec![
            6 * n,
            0,
            w,
            h,
            search,
            PATCH,
            fw(1.0 / (hh * hh).max(1e-12)),
        ],
    );
    prog.words = 9 * u64::from(n);
    prog
}

/// Tests run the à-trous levels per thread ([`ATROUS`]) only.
#[cfg(test)]
pub(crate) static PLAIN_ATROUS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Tests run non-local means per thread ([`NLM`]) only.
#[cfg(test)]
pub(crate) static PLAIN_NLM: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Denoise, param version 1.
pub struct DenoiseOp;

impl Op for DenoiseOp {
    fn name(&self) -> &'static str {
        "denoise_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::MultiScale {
            levels: LEVELS as u8,
        }
    }
    fn halo(&self, p: &Params, scale: f32) -> u32 {
        halo(&Denoise::of(p), scale)
    }
    fn heavy(&self) -> bool {
        true
    }
    fn param_words(&self) -> u32 {
        10
    }
    fn write_params(&self, p: &Params, _scale: f32, out: &mut [u32]) {
        let a = AiParams::from_fields(&p.to_fields());
        out[8] = u32::from(a.on() && a.key.is_some());
        out[9] = fw(a.blend);
        let d = Denoise::of(p);
        let (ia, s2, m) = vst_words(&d);
        for c in 0..3 {
            out[c] = fw(ia[c]);
            out[3 + c] = fw(s2[c]);
        }
        out[6] = fw(m);
        out[7] = u32::from(d.nlm_on());
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[kernels::INVERSE_WGSL, ai::AI_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        let d = Denoise::of(p);
        let a = AiParams::from_fields(&p.to_fields());
        d.luminance == 0.0 && d.colour == 0.0 && !(a.on() && a.key.is_some())
    }
    fn data(&self, p: &Params) -> Option<Arc<OpData>> {
        ai::table_for(&AiParams::from_fields(&p.to_fields()))
    }
    fn place(&self, p: &Params, at: &Placement<'_>) -> Option<Params> {
        let mut f = p.to_fields();
        let ai = AiParams::from_fields(&f).on();
        if !ai && !at.draft {
            return None;
        }
        if at.draft {
            f.set(DRAFT_FIELD, Value::Bool(true));
        }
        if ai {
            let key = AiParams::from_fields(&f).key;
            ai::write_hidden(&mut f, key.as_ref(), Some(ai::grid(&at.geometry)));
        }
        let mut q = p.clone();
        q.set_fields(f);
        Some(q)
    }
    fn local(&self, p: &Params, input: Roi, scale: f32) -> Option<Program> {
        Some(program(&Denoise::of(p), input, scale))
    }
    fn wgsl(&self) -> &'static str {
        "    let c = src_at(x, y);
    let rw = pu(18u);
    let n = rw * pu(19u);
    let i = u32(y - i32(pu(17u))) * rw + u32(x - i32(pu(16u)));
    var yv = lc(6u * n + i);
    if (pu(p + 7u) != 0u) { yv = lc(i); }
    let u = lc(7u * n + i);
    let v = lc(8u * n + i);
    let dr = yv * 0.5773502691896258 + u * 0.7071067811865476 + v * 0.4082482904638631;
    let dg = yv * 0.5773502691896258 - v * 0.8164965809277261;
    let db = yv * 0.5773502691896258 - u * 0.7071067811865476 + v * 0.4082482904638631;
    let m = pf(p + 6u);
    var o = vec3<f32>(dv_gat_inv(dr, pf(p), pf(p + 3u), m), dv_gat_inv(dg, pf(p + 1u), pf(p + 4u), m),
        dv_gat_inv(db, pf(p + 2u), pf(p + 5u), m));
    if (pu(p + 8u) != 0u) {
        let a = dv_ai_at(x, y);
        o = mix(o, a.rgb, pf(p + 9u) * a.w);
    }
    return vec4<f32>(o, c.a);"
    }
}
