//! GIMP 2.10+ layer modes: what each `GimpLayerMode` blends, in which colour space it
//! blends and composites, and the four composite modes (union, clip to backdrop, clip to
//! layer, intersection) with their subtractive variants. Written from GIMP's documented
//! maths; the per-pixel rules (including which degenerate pixels keep the backdrop) follow
//! GIMP's operations so results match its projection.

use crate::legacy::Legacy;
use crate::pixel::{linear_to_srgb, srgb_to_linear};

/// A colour space colours are composited in (blending may use another, see [`Mode::blend`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Space {
    Linear,
    /// sRGB-coded ("perceptual" and "from colour profile" both land here: without colour
    /// management of the layer stack the profile's curve is taken as sRGB's).
    NonLinear,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BlendSpace {
    Linear,
    NonLinear,
    /// CIE L*a*b* (D50).
    Lab,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Composite {
    Union,
    ClipToBackdrop,
    ClipToLayer,
    Intersection,
}

/// Per-pixel blend functions of the generic modes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Blend {
    Normal,
    Addition,
    Subtract,
    Burn,
    Dodge,
    Divide,
    Darken,
    Lighten,
    Difference,
    Exclusion,
    GrainExtract,
    GrainMerge,
    HardMix,
    Hardlight,
    LinearBurn,
    LinearLight,
    Multiply,
    Overlay,
    PinLight,
    Screen,
    Softlight,
    VividLight,
    HslColor,
    HsvHue,
    HsvSaturation,
    HsvValue,
    LchChroma,
    LchColor,
    LchHue,
    LchLightness,
    LumaDarken,
    LumaLighten,
    Luminance,
    ColorErase,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Blend(Blend),
    Legacy(Legacy),
    Dissolve,
    Behind,
    Erase,
    Merge,
    Split,
    /// Replace and pass-through.
    Replace,
    AntiErase,
}

/// A layer's mode with its composite mode and spaces resolved.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mode {
    pub kind: Kind,
    pub composite: Composite,
    /// `None`: any (the backdrop's).
    pub space: Option<Space>,
    /// `None`: the composite space.
    pub blend: Option<BlendSpace>,
    pub subtractive: bool,
    /// The mode changes layer pixels even with no backdrop (dissolve, anti-erase); others,
    /// as the bottom of a stack, just place the layer.
    pub affects_source: bool,
}

const BLEND_IMM: u8 = 1;
const SPACE_IMM: u8 = 2;
const COMPOSITE_IMM: u8 = 4;
const SUB: u8 = 8;
const ALL_IMM: u8 = BLEND_IMM | SPACE_IMM | COMPOSITE_IMM;

pub(crate) const PASS_THROUGH: u32 = 61;
/// GIMP 2.10's normal mode (`GIMP_LAYER_MODE_NORMAL`).
pub(crate) const NORMAL: u32 = 28;

/// Defaults of mode `m` (0..=63): kind, composite, composite space, blend space, flags.
fn table(m: u32) -> (Kind, Composite, Option<Space>, Option<BlendSpace>, u8) {
    use BlendSpace as B;
    use Composite::*;
    use Space::*;
    let lin = Some(Linear);
    let nl = Some(NonLinear);
    let legacy = |k: Legacy| {
        (
            Kind::Legacy(k),
            ClipToBackdrop,
            nl,
            Some(B::NonLinear),
            ALL_IMM,
        )
    };
    let gen_ = |b: Blend, bs: Option<B>, f: u8| (Kind::Blend(b), ClipToBackdrop, lin, bs, f);
    let perc = Some(B::NonLinear);
    let blin = Some(B::Linear);
    match m {
        0 => (Kind::Blend(Blend::Normal), Union, nl, None, ALL_IMM),
        1 => (Kind::Dissolve, Union, None, None, BLEND_IMM | SPACE_IMM),
        2 => (Kind::Behind, Union, nl, None, ALL_IMM),
        3 => legacy(Legacy::Multiply),
        4 => legacy(Legacy::Screen),
        // Old broken overlay is GIMP's soft light.
        5 | 19 => legacy(Legacy::Softlight),
        6 => legacy(Legacy::Difference),
        7 => legacy(Legacy::Addition),
        8 => legacy(Legacy::Subtract),
        9 => legacy(Legacy::Darken),
        10 => legacy(Legacy::Lighten),
        11 => legacy(Legacy::HsvHue),
        12 => legacy(Legacy::HsvSaturation),
        13 => legacy(Legacy::HslColor),
        14 => legacy(Legacy::HsvValue),
        15 => legacy(Legacy::Divide),
        16 => legacy(Legacy::Dodge),
        17 => legacy(Legacy::Burn),
        18 => legacy(Legacy::Hardlight),
        20 => legacy(Legacy::GrainExtract),
        21 => legacy(Legacy::GrainMerge),
        22 => (
            Kind::Blend(Blend::ColorErase),
            ClipToBackdrop,
            nl,
            Some(B::NonLinear),
            ALL_IMM | SUB,
        ),
        23 => gen_(Blend::Overlay, perc, 0),
        24 => gen_(Blend::LchHue, Some(B::Lab), BLEND_IMM),
        25 => gen_(Blend::LchChroma, Some(B::Lab), BLEND_IMM),
        26 => gen_(Blend::LchColor, Some(B::Lab), BLEND_IMM),
        27 => gen_(Blend::LchLightness, Some(B::Lab), BLEND_IMM),
        29 => (Kind::Behind, Union, lin, None, BLEND_IMM),
        30 => gen_(Blend::Multiply, blin, 0),
        31 => gen_(Blend::Screen, perc, 0),
        32 => gen_(Blend::Difference, perc, 0),
        33 => gen_(Blend::Addition, blin, 0),
        34 => gen_(Blend::Subtract, blin, 0),
        35 => gen_(Blend::Darken, None, BLEND_IMM),
        36 => gen_(Blend::Lighten, None, BLEND_IMM),
        37 => gen_(Blend::HsvHue, perc, BLEND_IMM),
        38 => gen_(Blend::HsvSaturation, perc, BLEND_IMM),
        39 => gen_(Blend::HslColor, perc, BLEND_IMM),
        40 => gen_(Blend::HsvValue, perc, BLEND_IMM),
        41 => gen_(Blend::Divide, blin, 0),
        42 => gen_(Blend::Dodge, perc, 0),
        43 => gen_(Blend::Burn, perc, 0),
        44 => gen_(Blend::Hardlight, perc, 0),
        45 => gen_(Blend::Softlight, perc, 0),
        46 => gen_(Blend::GrainExtract, perc, 0),
        47 => gen_(Blend::GrainMerge, perc, 0),
        48 => gen_(Blend::VividLight, perc, 0),
        49 => gen_(Blend::PinLight, perc, 0),
        50 => gen_(Blend::LinearLight, perc, 0),
        51 => gen_(Blend::HardMix, perc, 0),
        52 => gen_(Blend::Exclusion, perc, 0),
        53 => gen_(Blend::LinearBurn, perc, 0),
        54 => gen_(Blend::LumaDarken, blin, 0),
        55 => gen_(Blend::LumaLighten, blin, 0),
        56 => gen_(Blend::Luminance, blin, BLEND_IMM),
        57 => gen_(Blend::ColorErase, blin, SUB),
        58 => (Kind::Erase, ClipToBackdrop, lin, None, BLEND_IMM | SUB),
        59 => (Kind::Merge, Union, lin, None, BLEND_IMM),
        60 => (
            Kind::Split,
            ClipToBackdrop,
            None,
            None,
            BLEND_IMM | SPACE_IMM | SUB,
        ),
        61 => (Kind::Replace, Union, lin, None, BLEND_IMM | COMPOSITE_IMM),
        62 => (Kind::Replace, Union, lin, None, BLEND_IMM),
        63 => (Kind::AntiErase, Union, None, None, BLEND_IMM | SPACE_IMM),
        // 28 and anything unknown: Normal.
        _ => (Kind::Blend(Blend::Normal), Union, lin, None, BLEND_IMM),
    }
}

/// Resolves a layer's mode and its `PROP_COMPOSITE_MODE` / `PROP_COMPOSITE_SPACE` /
/// `PROP_BLEND_SPACE` values (0 = automatic) the way GIMP does: modes that fix a setting
/// ignore the property.
pub(crate) fn resolve(mode: u32, composite: u32, space: u32, blend: u32) -> Mode {
    let (kind, mut comp, mut sp, mut bs, flags) = table(mode);
    if flags & COMPOSITE_IMM == 0 {
        comp = match composite {
            1 => Composite::Union,
            2 => Composite::ClipToBackdrop,
            3 => Composite::ClipToLayer,
            4 => Composite::Intersection,
            _ => comp,
        };
    }
    if flags & SPACE_IMM == 0 {
        sp = match space {
            1 | 3 => Some(Space::Linear),
            2 | 4 => Some(Space::NonLinear),
            _ => sp,
        };
    }
    if flags & BLEND_IMM == 0 {
        bs = match blend {
            1 => Some(BlendSpace::Linear),
            2 | 4 => Some(BlendSpace::NonLinear),
            3 => Some(BlendSpace::Lab),
            _ => bs,
        };
    }
    Mode {
        kind,
        composite: comp,
        space: sp,
        blend: bs,
        subtractive: flags & SUB != 0,
        affects_source: matches!(kind, Kind::Dissolve | Kind::AntiErase),
    }
}

impl Mode {
    /// Whether pixels where the layer is fully transparent come out as the backdrop
    /// unchanged, so only the layer's area needs processing.
    pub(crate) fn identity_outside(&self) -> bool {
        matches!(self.composite, Composite::Union | Composite::ClipToBackdrop)
            && self.kind != Kind::Replace
    }

    /// Which of (layer, backdrop) areas the result covers: GIMP's "included region".
    pub(crate) fn included(&self) -> (bool, bool) {
        match self.composite {
            Composite::Union => (true, true),
            Composite::ClipToBackdrop => (false, true),
            Composite::ClipToLayer => (true, false),
            Composite::Intersection => (false, false),
        }
    }
}

// ---- colour space conversions -------------------------------------------------------------

/// Linear sRGB → XYZ (D50, Bradford-adapted), as babl defines sRGB.
const RGB_TO_XYZ: [[f64; 3]; 3] = [
    [0.436_074_7, 0.385_064_9, 0.143_080_4],
    [0.222_504_5, 0.716_878_6, 0.060_616_9],
    [0.013_932_2, 0.097_104_5, 0.714_173_3],
];
const WHITE: [f64; 3] = [0.964_22, 1.0, 0.825_21];
const LAB_EPS: f64 = 216.0 / 24389.0;
const LAB_KAPPA: f64 = 24389.0 / 27.0;

fn invert(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let c = |a: usize, b: usize, c: usize, d: usize| m[a][b] * m[c][d] - m[a][d] * m[c][b];
    [
        [
            c(1, 1, 2, 2) / det,
            -c(0, 1, 2, 2) / det,
            c(0, 1, 1, 2) / det,
        ],
        [
            -c(1, 0, 2, 2) / det,
            c(0, 0, 2, 2) / det,
            -c(0, 0, 1, 2) / det,
        ],
        [
            c(1, 0, 2, 1) / det,
            -c(0, 0, 2, 1) / det,
            c(0, 0, 1, 1) / det,
        ],
    ]
}

fn xyz_to_rgb() -> &'static [[f64; 3]; 3] {
    static M: std::sync::OnceLock<[[f64; 3]; 3]> = std::sync::OnceLock::new();
    M.get_or_init(|| invert(RGB_TO_XYZ))
}

fn mul(m: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

fn lab_f(t: f64) -> f64 {
    if t > LAB_EPS {
        t.cbrt()
    } else {
        (LAB_KAPPA * t + 16.0) / 116.0
    }
}

fn lab_finv(f: f64) -> f64 {
    let t = f * f * f;
    if t > LAB_EPS {
        t
    } else {
        (116.0 * f - 16.0) / LAB_KAPPA
    }
}

fn linear_to_lab(c: [f32; 3]) -> [f32; 3] {
    let xyz = mul(&RGB_TO_XYZ, c.map(f64::from));
    let f = [
        lab_f(xyz[0] / WHITE[0]),
        lab_f(xyz[1] / WHITE[1]),
        lab_f(xyz[2] / WHITE[2]),
    ];
    [
        (116.0 * f[1] - 16.0) as f32,
        (500.0 * (f[0] - f[1])) as f32,
        (200.0 * (f[1] - f[2])) as f32,
    ]
}

fn lab_to_linear(c: [f32; 3]) -> [f32; 3] {
    let fy = (f64::from(c[0]) + 16.0) / 116.0;
    let fx = fy + f64::from(c[1]) / 500.0;
    let fz = fy - f64::from(c[2]) / 200.0;
    let xyz = [
        lab_finv(fx) * WHITE[0],
        if f64::from(c[0]) > LAB_KAPPA * LAB_EPS {
            fy * fy * fy
        } else {
            f64::from(c[0]) / LAB_KAPPA
        } * WHITE[1],
        lab_finv(fz) * WHITE[2],
    ];
    mul(xyz_to_rgb(), xyz).map(|v| v as f32)
}

#[inline]
fn to_blend(c: [f32; 3], from: Space, to: BlendSpace) -> [f32; 3] {
    match (from, to) {
        (Space::Linear, BlendSpace::Linear) | (Space::NonLinear, BlendSpace::NonLinear) => c,
        (Space::Linear, BlendSpace::NonLinear) => c.map(linear_to_srgb),
        (Space::NonLinear, BlendSpace::Linear) => c.map(srgb_to_linear),
        (Space::Linear, BlendSpace::Lab) => linear_to_lab(c),
        (Space::NonLinear, BlendSpace::Lab) => linear_to_lab(c.map(srgb_to_linear)),
    }
}

#[inline]
fn from_blend(c: [f32; 3], from: BlendSpace, to: Space) -> [f32; 3] {
    match (from, to) {
        (BlendSpace::Linear, Space::Linear) | (BlendSpace::NonLinear, Space::NonLinear) => c,
        (BlendSpace::NonLinear, Space::Linear) => c.map(srgb_to_linear),
        (BlendSpace::Linear, Space::NonLinear) => c.map(linear_to_srgb),
        (BlendSpace::Lab, Space::Linear) => lab_to_linear(c),
        (BlendSpace::Lab, Space::NonLinear) => lab_to_linear(c).map(linear_to_srgb),
    }
}

// ---- blend functions ---------------------------------------------------------------------

const EPS: f32 = 1e-6;

use specular_blend::float::{self as fl, safe_div};
use specular_blend::{Mode as Shared, blend1};

/// Luminance weights GIMP uses for the luma modes.
const LUMA: [f32; 3] = [0.222_488_4, 0.716_903_7, 0.060_607_91];
/// babl's sRGB luminance (the `Y` row of [`RGB_TO_XYZ`]).
const Y_SRGB: [f32; 3] = [0.222_504_5, 0.716_878_6, 0.060_616_9];

#[inline]
fn dot(w: [f32; 3], c: [f32; 3]) -> f32 {
    w[0] * c[0] + w[1] * c[1] + w[2] * c[2]
}

#[inline]
fn per_channel(i: [f32; 3], l: [f32; 3], f: impl Fn(f32, f32) -> f32) -> [f32; 3] {
    [f(i[0], l[0]), f(i[1], l[1]), f(i[2], l[2])]
}

#[inline]
fn min3(c: [f32; 3]) -> f32 {
    c[0].min(c[1]).min(c[2])
}

#[inline]
fn max3(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2])
}

/// The blended colour and its alpha for a pixel where both alphas are non-zero.
#[inline]
fn blend(b: Blend, i: [f32; 3], l: [f32; 3], l_a: f32) -> ([f32; 3], f32) {
    let c = match b {
        Blend::Normal => l,
        // The separable modes: the unclamped float formulas shared with the other readers.
        Blend::Addition => per_channel(i, l, fl::add),
        Blend::Subtract => per_channel(i, l, fl::subtract),
        Blend::Burn => per_channel(i, l, fl::color_burn),
        Blend::Dodge => per_channel(i, l, fl::color_dodge),
        Blend::Divide => per_channel(i, l, fl::divide),
        Blend::Darken => per_channel(i, l, |i, l| blend1(Shared::Darken, i, l)),
        Blend::Lighten => per_channel(i, l, |i, l| blend1(Shared::Lighten, i, l)),
        Blend::Difference => per_channel(i, l, |i, l| blend1(Shared::Difference, i, l)),
        Blend::Exclusion => per_channel(i, l, fl::exclusion),
        Blend::GrainExtract => per_channel(i, l, fl::grain_extract),
        Blend::GrainMerge => per_channel(i, l, fl::grain_merge),
        Blend::HardMix => per_channel(i, l, |i, l| blend1(Shared::HardMix, i, l)),
        Blend::Hardlight => per_channel(i, l, fl::hard_light),
        Blend::LinearBurn => per_channel(i, l, fl::linear_burn),
        Blend::LinearLight => per_channel(i, l, fl::linear_light),
        Blend::Multiply => per_channel(i, l, |i, l| blend1(Shared::Multiply, i, l)),
        Blend::Overlay => per_channel(i, l, fl::overlay),
        Blend::PinLight => per_channel(i, l, fl::pin_light),
        Blend::Screen => per_channel(i, l, fl::screen),
        Blend::Softlight => per_channel(i, l, fl::soft_light_pegtop),
        Blend::VividLight => per_channel(i, l, fl::vivid_light),
        Blend::HslColor => {
            let dest_l = (min3(i) + max3(i)) / 2.0;
            let src_l = (min3(l) + max3(l)) / 2.0;
            if src_l.abs() > EPS && (1.0 - src_l).abs() > EPS {
                let dest_high = dest_l > 0.5;
                let src_high = src_l > 0.5;
                let dl = dest_l.min(1.0 - dest_l);
                let sl = src_l.min(1.0 - src_l);
                let ratio = dl / sl;
                let mut offset = 0.0;
                if dest_high {
                    offset += 1.0 - 2.0 * dl;
                }
                if src_high {
                    offset += 2.0 * dl - ratio;
                }
                l.map(|v| v * ratio + offset)
            } else {
                [dest_l; 3]
            }
        }
        Blend::HsvHue => {
            let (smin, smax) = (min3(l), max3(l));
            let sdelta = smax - smin;
            if sdelta > EPS {
                let (dmin, dmax) = (min3(i), max3(i));
                let ddelta = dmax - dmin;
                let ds = if dmax != 0.0 { ddelta / dmax } else { 0.0 };
                let ratio = ds * dmax / sdelta;
                let offset = dmax - smax * ratio;
                l.map(|v| v * ratio + offset)
            } else {
                i
            }
        }
        Blend::HsvSaturation => {
            let (dmin, dmax) = (min3(i), max3(i));
            let ddelta = dmax - dmin;
            if ddelta > EPS {
                let (smin, smax) = (min3(l), max3(l));
                let sdelta = smax - smin;
                let ss = if smax != 0.0 { sdelta / smax } else { 0.0 };
                let ratio = ss * dmax / ddelta;
                let offset = (1.0 - ratio) * dmax;
                i.map(|v| v * ratio + offset)
            } else {
                [dmax; 3]
            }
        }
        Blend::HsvValue => {
            let dv = max3(i);
            let sv = max3(l);
            if dv.abs() > EPS {
                let ratio = sv / dv;
                i.map(|v| v * ratio)
            } else {
                [sv; 3]
            }
        }
        Blend::LchChroma => {
            let c1 = i[1].hypot(i[2]);
            if c1 > EPS {
                let c2 = l[1].hypot(l[2]);
                [i[0], c2 * i[1] / c1, c2 * i[2] / c1]
            } else {
                i
            }
        }
        Blend::LchColor => [i[0], l[1], l[2]],
        Blend::LchHue => {
            let c2 = l[1].hypot(l[2]);
            if c2 > EPS {
                let c1 = i[1].hypot(i[2]);
                [i[0], c1 * l[1] / c2, c1 * l[2] / c2]
            } else {
                i
            }
        }
        Blend::LchLightness => [l[0], i[1], i[2]],
        Blend::LumaDarken => {
            if dot(LUMA, i) <= dot(LUMA, l) {
                i
            } else {
                l
            }
        }
        Blend::LumaLighten => {
            if dot(LUMA, i) >= dot(LUMA, l) {
                i
            } else {
                l
            }
        }
        Blend::Luminance => {
            let ratio = safe_div(dot(Y_SRGB, l), dot(Y_SRGB, i));
            i.map(|v| v * ratio)
        }
        Blend::ColorErase => {
            let mut a = 0.0f32;
            for k in 0..3 {
                let col = i[k].clamp(0.0, 1.0);
                let bg = l[k].clamp(0.0, 1.0);
                if (col - bg).abs() > EPS {
                    let v = if col > bg {
                        (col - bg) / (1.0 - bg)
                    } else {
                        (bg - col) / bg
                    };
                    a = a.max(v);
                }
            }
            return if a > EPS {
                let inv = 1.0 / a;
                (per_channel(i, l, |c, b| (c - b) * inv + b), a)
            } else {
                ([0.0; 3], a)
            };
        }
    };
    (c, l_a)
}

// ---- compositing --------------------------------------------------------------------------

/// Where (and what) to composite.
pub(crate) struct Run<'a> {
    pub mode: &'a Mode,
    /// The space `inp` and `layer` are in.
    pub space: Space,
    pub opacity: f32,
    /// Canvas position of the first pixel (dissolve's noise is tied to it).
    pub x: i32,
    pub y: i32,
    /// The backdrop is empty (bottom of a stack): GIMP then just places the layer.
    pub last: bool,
}

/// Composites `layer` (RGBA, straight alpha) onto `inp` in place. `mask` is per-pixel layer
/// coverage.
pub(crate) fn apply(run: &Run, inp: &mut [f32], layer: &[f32], mask: Option<&[f32]>) {
    let mode = run.mode;
    let op = run.opacity;
    let mut composite = mode.composite;
    if run.last {
        if !mode.affects_source {
            for (n, (o, l)) in inp
                .chunks_exact_mut(4)
                .zip(layer.chunks_exact(4))
                .enumerate()
            {
                o[..3].copy_from_slice(&l[..3]);
                let mut a = l[3] * op;
                if let Some(m) = mask {
                    a *= m[n];
                }
                o[3] = a;
            }
            return;
        }
        composite = Composite::Union;
    }
    let px = inp
        .chunks_exact_mut(4)
        .zip(layer.chunks_exact(4))
        .enumerate();
    let cov = |n: usize| mask.map_or(1.0, |m| m[n]);
    match mode.kind {
        Kind::Legacy(k) => crate::legacy::apply(k, inp, layer, mask, op),
        Kind::Blend(b) => {
            let blend_space = mode.blend.filter(|s| {
                !matches!(
                    (s, run.space),
                    (BlendSpace::Linear, Space::Linear) | (BlendSpace::NonLinear, Space::NonLinear)
                )
            });
            for (n, (o, l)) in px {
                let (in_a, l_a) = (o[3], l[3]);
                let i = [o[0], o[1], o[2]];
                let lc = [l[0], l[1], l[2]];
                let (comp, comp_a) = if in_a != 0.0 && l_a != 0.0 {
                    if b == Blend::Normal {
                        (lc, l_a)
                    } else if let Some(bs) = blend_space {
                        let (c, a) = blend(
                            b,
                            to_blend(i, run.space, bs),
                            to_blend(lc, run.space, bs),
                            l_a,
                        );
                        (from_blend(c, bs, run.space), a)
                    } else {
                        blend(b, i, lc, l_a)
                    }
                } else {
                    // Unconstrained in GIMP; never used below.
                    ([0.0; 3], if mode.subtractive { 0.0 } else { l_a })
                };
                let m = cov(n);
                if mode.subtractive {
                    composite_sub(composite, o, l, comp, comp_a, op, m);
                } else {
                    composite_normal(composite, o, l, comp, comp_a, op, m);
                }
            }
        }
        Kind::Dissolve => {
            let table = crate::rng::dissolve_table();
            let y = run.y.rem_euclid(4096) as usize;
            let mut rng = crate::rng::Rand::new(table[y]);
            for _ in 0..run.x.max(0) {
                rng.int();
            }
            for (n, (o, l)) in px {
                let value = l[3] * op * 255.0 * cov(n);
                if (rng.below_255() as f32) >= value {
                    if !matches!(composite, Composite::Union | Composite::ClipToBackdrop) {
                        o[3] = 0.0;
                    }
                } else {
                    o[..3].copy_from_slice(&l[..3]);
                    if matches!(composite, Composite::Union | Composite::ClipToLayer) {
                        o[3] = 1.0;
                    }
                }
            }
        }
        Kind::Behind => {
            for (n, (o, l)) in px {
                let in_a = o[3];
                let la = l[3] * op * cov(n);
                match composite {
                    Composite::Union => {
                        let new_a =
                            (f64::from(la) + (1.0 - f64::from(la)) * f64::from(in_a)) as f32;
                        if new_a != 0.0 {
                            let r = in_a / new_a;
                            for b in 0..3 {
                                o[b] = o[b] * r + l[b] * (1.0 - r);
                            }
                        } else {
                            o[..3].copy_from_slice(&l[..3]);
                        }
                        o[3] = new_a;
                    }
                    Composite::ClipToBackdrop => {
                        if in_a == 0.0 {
                            o[..3].copy_from_slice(&l[..3]);
                        }
                    }
                    Composite::ClipToLayer => {
                        for b in 0..3 {
                            o[b] = if la != 0.0 {
                                l[b] + (o[b] - l[b]) * in_a
                            } else {
                                l[b]
                            };
                        }
                        o[3] = la;
                    }
                    Composite::Intersection => {
                        let new_a = in_a * la;
                        if new_a == 0.0 {
                            o[..3].copy_from_slice(&l[..3]);
                        }
                        o[3] = new_a;
                    }
                }
            }
        }
        Kind::Erase => {
            for (n, (o, l)) in px {
                let in_a = o[3];
                let la = l[3] * op * cov(n);
                match composite {
                    Composite::Union => {
                        let new_a = in_a + la - 2.0 * in_a * la;
                        if new_a != 0.0 {
                            let r = (1.0 - in_a) * la / new_a;
                            for b in 0..3 {
                                o[b] = r * l[b] + (1.0 - r) * o[b];
                            }
                        }
                        o[3] = new_a;
                    }
                    Composite::ClipToBackdrop => o[3] = (1.0 - la) * in_a,
                    Composite::ClipToLayer => {
                        let new_a = (1.0 - in_a) * la;
                        if new_a != 0.0 {
                            o[..3].copy_from_slice(&l[..3]);
                        }
                        o[3] = new_a;
                    }
                    Composite::Intersection => o[3] = 0.0,
                }
            }
        }
        Kind::Merge => {
            for (n, (o, l)) in px {
                let in_a = o[3];
                let mut la = l[3] * op * cov(n);
                match composite {
                    Composite::Union => {
                        let ia = in_a.min(1.0 - la);
                        let new_a = ia + la;
                        if new_a != 0.0 {
                            let r = la / new_a;
                            for b in 0..3 {
                                o[b] += (l[b] - o[b]) * r;
                            }
                        }
                        o[3] = new_a;
                    }
                    Composite::ClipToBackdrop => {
                        la -= 1.0 - in_a;
                        if la > 0.0 {
                            let r = la / in_a;
                            for b in 0..3 {
                                o[b] += (l[b] - o[b]) * r;
                            }
                        }
                    }
                    Composite::ClipToLayer => {
                        if la != 0.0 {
                            o[..3].copy_from_slice(&l[..3]);
                        }
                        o[3] = la;
                    }
                    Composite::Intersection => {
                        la = (la - (1.0 - in_a)).max(0.0);
                        if la != 0.0 {
                            o[..3].copy_from_slice(&l[..3]);
                        }
                        o[3] = la;
                    }
                }
            }
        }
        Kind::Split => {
            for (n, (o, l)) in px {
                let in_a = o[3];
                let la = l[3] * op * cov(n);
                match composite {
                    Composite::Union => {
                        if la <= in_a {
                            o[3] = in_a - la;
                        } else {
                            o[..3].copy_from_slice(&l[..3]);
                            o[3] = la - in_a;
                        }
                    }
                    Composite::ClipToBackdrop => o[3] = (in_a - la).max(0.0),
                    Composite::ClipToLayer => {
                        let new_a = (la - in_a).max(0.0);
                        if new_a != 0.0 {
                            o[..3].copy_from_slice(&l[..3]);
                        }
                        o[3] = new_a;
                    }
                    Composite::Intersection => o[3] = 0.0,
                }
            }
        }
        Kind::Replace => {
            for (n, (o, l)) in px {
                let in_a = o[3];
                let v = op * cov(n);
                match composite {
                    Composite::Union => {
                        let new_a = (l[3] - in_a) * v + in_a;
                        let mut r = v;
                        if new_a != 0.0 {
                            r *= l[3] / new_a;
                        }
                        for b in 0..3 {
                            o[b] += (l[b] - o[b]) * r;
                        }
                        o[3] = new_a;
                    }
                    Composite::ClipToBackdrop => o[3] = in_a * (1.0 - v),
                    Composite::ClipToLayer => {
                        o[..3].copy_from_slice(&l[..3]);
                        o[3] = l[3] * v;
                    }
                    Composite::Intersection => o.fill(0.0),
                }
            }
        }
        Kind::AntiErase => {
            for (n, (o, l)) in px {
                let in_a = o[3];
                let v = op * cov(n);
                o[3] = match composite {
                    Composite::Union => {
                        (f64::from(in_a) + (1.0 - f64::from(in_a)) * f64::from(l[3] * v)) as f32
                    }
                    Composite::ClipToBackdrop => in_a,
                    Composite::ClipToLayer => l[3] * v,
                    Composite::Intersection => in_a * l[3] * v,
                };
            }
        }
    }
}

#[inline]
fn composite_normal(
    mode: Composite,
    o: &mut [f32],
    l: &[f32],
    comp: [f32; 3],
    comp_a: f32,
    op: f32,
    m: f32,
) {
    let in_a = o[3];
    match mode {
        Composite::Union => {
            let la = l[3] * op * m;
            let new_a = la + (1.0 - la) * in_a;
            if la == 0.0 || new_a == 0.0 {
            } else if in_a == 0.0 {
                o[..3].copy_from_slice(&l[..3]);
            } else {
                let r = la / new_a;
                for b in 0..3 {
                    o[b] = r * (in_a * (comp[b] - l[b]) + l[b] - o[b]) + o[b];
                }
            }
            o[3] = new_a;
        }
        Composite::ClipToBackdrop => {
            let la = comp_a * op * m;
            if in_a != 0.0 && la != 0.0 {
                for b in 0..3 {
                    o[b] = comp[b] * la + o[b] * (1.0 - la);
                }
            }
        }
        Composite::ClipToLayer => {
            let la = l[3] * op * m;
            if la == 0.0 {
            } else if in_a == 0.0 {
                o[..3].copy_from_slice(&l[..3]);
            } else {
                for b in 0..3 {
                    o[b] = comp[b] * in_a + l[b] * (1.0 - in_a);
                }
            }
            o[3] = la;
        }
        Composite::Intersection => {
            let new_a = in_a * comp_a * op * m;
            if new_a != 0.0 {
                o[..3].copy_from_slice(&comp);
            }
            o[3] = new_a;
        }
    }
}

#[inline]
fn composite_sub(
    mode: Composite,
    o: &mut [f32],
    l: &[f32],
    comp: [f32; 3],
    comp_a: f32,
    op: f32,
    m: f32,
) {
    let in_a = o[3];
    let la = l[3] * op * m;
    match mode {
        Composite::Union => {
            let new_a = in_a + la - (2.0 - comp_a) * in_a * la;
            if la == 0.0 || new_a == 0.0 {
            } else if in_a == 0.0 {
                o[..3].copy_from_slice(&l[..3]);
            } else {
                let r = in_a / new_a;
                let coeff = 1.0 / in_a - 1.0;
                for b in 0..3 {
                    o[b] = r * (la * (comp_a * comp[b] + coeff * l[b] - o[b]) + o[b]);
                }
            }
            o[3] = new_a;
        }
        Composite::ClipToBackdrop => {
            let ca = comp_a * la;
            let new_a = 1.0 - la + ca;
            if in_a != 0.0 && ca != 0.0 {
                let r = ca / new_a;
                for b in 0..3 {
                    o[b] = comp[b] * r + o[b] * (1.0 - r);
                }
            }
            o[3] = new_a * in_a;
        }
        Composite::ClipToLayer => {
            let ca = comp_a * in_a;
            let new_a = 1.0 - in_a + ca;
            if la == 0.0 {
            } else if in_a == 0.0 {
                o[..3].copy_from_slice(&l[..3]);
            } else {
                let r = ca / new_a;
                for b in 0..3 {
                    o[b] = comp[b] * r + l[b] * (1.0 - r);
                }
            }
            o[3] = new_a * la;
        }
        Composite::Intersection => {
            let new_a = in_a * l[3] * comp_a * op * m;
            if new_a != 0.0 {
                o[..3].copy_from_slice(&comp);
            }
            o[3] = new_a;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(mode: u32, inp: [f32; 4], layer: [f32; 4], op: f32) -> [f32; 4] {
        let m = resolve(mode, 0, 0, 0);
        let mut o = inp;
        let r = Run {
            mode: &m,
            space: Space::Linear,
            opacity: op,
            x: 0,
            y: 0,
            last: false,
        };
        apply(&r, &mut o, &layer, None);
        o
    }

    #[test]
    fn normal_over() {
        let o = run(28, [1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0], 0.25);
        assert_eq!(o, [0.75, 0.0, 0.25, 1.0]);
        let o = run(28, [1.0, 0.0, 0.0, 0.5], [0.0, 0.0, 1.0, 0.5], 1.0);
        assert!((o[3] - 0.75).abs() < 1e-6);
        assert!((o[0] - 1.0 / 3.0).abs() < 1e-6 && (o[2] - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn multiply_clips_to_backdrop() {
        let o = run(30, [0.5, 0.5, 0.5, 0.0], [0.5, 0.5, 0.5, 1.0], 1.0);
        assert_eq!(o[3], 0.0);
        let o = run(30, [0.5, 0.4, 1.0, 1.0], [0.5, 0.5, 0.5, 1.0], 1.0);
        assert_eq!(o, [0.25, 0.2, 0.5, 1.0]);
    }

    #[test]
    fn lab_round_trip() {
        for c in [
            [0.2f32, 0.5, 0.9],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.001, 0.02, 0.5],
        ] {
            let back = lab_to_linear(linear_to_lab(c));
            for k in 0..3 {
                assert!((back[k] - c[k]).abs() < 1e-5, "{c:?} → {back:?}");
            }
        }
        let w = linear_to_lab([1.0; 3]);
        assert!((w[0] - 100.0).abs() < 1e-3 && w[1].abs() < 1e-2 && w[2].abs() < 1e-2);
    }

    #[test]
    fn erase_and_replace() {
        let o = run(58, [0.3, 0.3, 0.3, 1.0], [0.0, 0.0, 0.0, 0.5], 1.0);
        assert_eq!(o[3], 0.5);
        let o = run(62, [0.3, 0.3, 0.3, 1.0], [0.9, 0.9, 0.9, 0.5], 1.0);
        assert_eq!(o, [0.9, 0.9, 0.9, 0.5]);
    }

    #[test]
    fn immutable_settings_ignored() {
        // Normal (legacy) fixes everything; Normal lets the composite mode change.
        let m = resolve(0, 3, 1, 1);
        assert_eq!(m.composite, Composite::Union);
        assert_eq!(m.space, Some(Space::NonLinear));
        let m = resolve(28, 3, 2, 1);
        assert_eq!(m.composite, Composite::ClipToLayer);
        assert_eq!(m.space, Some(Space::NonLinear));
        assert_eq!(m.blend, None);
    }
}
