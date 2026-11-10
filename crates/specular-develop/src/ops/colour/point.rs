//! Point colour (after Lightroom's Point Color): a colour picked in the
//! image, and how its neighbourhood moves.
//!
//! - **Target.** The picked colour as OkLab hue (degrees), saturation (C / L) and lightness
//!   (L), taken from the module's input ([`target_of`]).
//! - **Range.** A pixel's weight is the product of three smooth windows: hue distance,
//!   saturation distance and lightness distance, each 1 within half its width and falling
//!   to 0 at its width (a smoothstep). The widths are base widths (25°, 0.12, 0.25) times
//!   `2 · range` times `2 · <dimension>_range`, so 0.5 everywhere is the base. For a
//!   colourful target the hue window also fades out on near-neutral pixels, whose hue is
//!   noise.
//! - **Shift.** Hue (degrees), saturation (chroma × (1 + s)) and lightness (up to ±1
//!   EV-equivalent), each times the weight. Chroma moves by scaling a and b, so neutrals
//!   stay neutral.
//! - `show_range` draws the weight instead: what moves keeps its colour, the rest turns
//!   grey (the range overlay; the panel sets it while a range control is held).

use crate::colour::oklab;
use crate::graph::{NodeKind, Op};
use crate::model::{Fields, Params, Value};

use super::num;

/// Base widths: hue (degrees), saturation (C / L), lightness (OkLab L).
pub const BASE_WIDTH: [f32; 3] = [25.0, 0.12, 0.25];
/// Below this target saturation the target counts as neutral and every hue is in range.
pub const NEUTRAL: f32 = 0.04;

/// Point colour params (fields of [`crate::model::ModuleKind::POINT_COLOUR`], v1).
#[derive(Clone, Debug, PartialEq)]
pub struct PointColour {
    pub picked: bool,
    pub target: [f32; 3],
    pub range: f32,
    pub ranges: [f32; 3],
    pub shift: [f32; 3],
    pub show_range: bool,
}

/// The target (hue degrees, saturation, lightness) of a linear Rec.2020 colour.
pub fn target_of(rgb: [f64; 3]) -> [f32; 3] {
    let (l, c, h) = oklab::lch(oklab::from_rec2020_rel(rgb));
    [
        h.to_degrees().rem_euclid(360.0) as f32,
        (c / l.abs().max(1e-6)) as f32,
        l as f32,
    ]
}

impl PointColour {
    pub fn from_fields(f: &Fields) -> Self {
        let hue = f.f32_or("target_hue", 0.0);
        Self {
            picked: f.bool_or("picked", false),
            target: [
                if hue.is_finite() {
                    hue.rem_euclid(360.0)
                } else {
                    0.0
                },
                num(f, "target_saturation", 0.0, 0.0, 4.0),
                num(f, "target_lightness", 0.5, -1.0, 100.0),
            ],
            range: num(f, "range", 0.5, 0.0, 1.0),
            ranges: [
                num(f, "hue_range", 0.5, 0.0, 1.0),
                num(f, "saturation_range", 0.5, 0.0, 1.0),
                num(f, "lightness_range", 0.5, 0.0, 1.0),
            ],
            shift: [
                num(f, "shift_hue", 0.0, -60.0, 60.0),
                num(f, "shift_saturation", 0.0, -1.0, 1.0),
                num(f, "shift_lightness", 0.0, -1.0, 1.0),
            ],
            show_range: f.bool_or("show_range", false),
        }
    }

    pub fn defaults() -> Fields {
        let mut f = Fields::new().with("picked", Value::Bool(false));
        for (n, v) in [
            ("target_hue", 0.0),
            ("target_saturation", 0.0),
            ("target_lightness", 0.5),
            ("range", 0.5),
            ("hue_range", 0.5),
            ("saturation_range", 0.5),
            ("lightness_range", 0.5),
            ("shift_hue", 0.0),
            ("shift_saturation", 0.0),
            ("shift_lightness", 0.0),
        ] {
            f.set(n, Value::Float(v));
        }
        f
    }

    pub fn is_identity(&self) -> bool {
        !self.picked || (!self.show_range && self.shift.iter().all(|v| *v == 0.0))
    }

    pub fn uniforms(&self) -> [f32; WORDS] {
        let h = self.target[0].to_radians();
        let inv = |i: usize, base: f32| {
            let w = base * 2.0 * self.range * 2.0 * self.ranges[i];
            1.0 / w.max(1e-6)
        };
        [
            f32::from(u8::from(self.target[1] >= NEUTRAL)),
            h.cos(),
            h.sin(),
            self.target[1],
            self.target[2],
            inv(0, BASE_WIDTH[0].to_radians()),
            inv(1, BASE_WIDTH[1]),
            inv(2, BASE_WIDTH[2]),
            self.shift[0].to_radians(),
            self.shift[1],
            self.shift[2] / 3.0,
            f32::from(u8::from(self.show_range)),
        ]
    }
}

pub const WORDS: usize = 12;

/// Point colour, param version 1.
pub struct PointColourOp;

impl Op for PointColourOp {
    fn name(&self) -> &'static str {
        "point_colour_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        WORDS as u32
    }
    fn identity(&self, p: &Params) -> bool {
        PointColour::from_fields(&p.to_fields()).is_identity()
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        super::super::put(out, &PointColour::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        static H: std::sync::LazyLock<[&'static str; 2]> = std::sync::LazyLock::new(|| {
            [
                *crate::colour::oklab::OKLAB_WGSL,
                super::super::tone::kernels::COMMON_WGSL,
            ]
        });
        &*H
    }
    fn wgsl(&self) -> &'static str {
        "    let lab = dv_oklab(c.rgb);
    let ch = length(lab.yz);
    let s = ch / max(abs(lab.x), 1e-6);
    var w = 1.0 - smoothstep(0.5, 1.0, abs(s - pf(p + 3u)) * pf(p + 6u));
    w *= 1.0 - smoothstep(0.5, 1.0, abs(lab.x - pf(p + 4u)) * pf(p + 7u));
    if (pf(p) > 0.0) {
        var dh = 3.14159265;
        if (ch > 0.0) {
            dh = abs(atan2(lab.z * pf(p + 1u) - lab.y * pf(p + 2u), lab.y * pf(p + 1u) + lab.z * pf(p + 2u)));
        }
        w *= (1.0 - smoothstep(0.5, 1.0, dh * pf(p + 5u))) * smoothstep(0.0, 0.04, s);
    }
    if (pf(p + 11u) > 0.0) {
        let g = vec3<f32>(dot(c.rgb, DV_LUMA));
        return vec4<f32>(g + (c.rgb - g) * w, c.a);
    }
    let a = pf(p + 8u) * w;
    let cr = cos(a);
    let sr = sin(a);
    let fl = exp2(pf(p + 10u) * w);
    let k = max(1.0 + pf(p + 9u) * w, 0.0) * fl;
    let ab = vec2<f32>(lab.y * cr - lab.z * sr, lab.y * sr + lab.z * cr) * k;
    return vec4<f32>(dv_oklab_inv(vec3<f32>(lab.x * fl, ab)), c.a);"
    }
}
