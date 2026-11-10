//! The output transform (stage [E] of the pipeline): display-linear Rec.2020 from the
//! view transform → the target space's linear values, gamut compressed into it and clipped
//! to `[0, peak]`.
//!
//! For the screen the target is the renderer's **storage space**: linear BT.709 rotated by
//! the display's `to_storage` matrix (`specular_color::gpu::Display`), so a wide-gamut
//! display keeps its gamut. Export spaces (ICC, HDR) are set by the export.
//!
//! It is not a module of the edit: the caller appends it with
//! [`Graph::build_for`](crate::graph::Graph::build_for).

use crate::colour::{self, M3};
use crate::graph::{NodeKind, Op};
use crate::model::{Fields, ModuleKind, Params, Value};

/// The output node's kind (never stored in an edit).
pub const OUTPUT: ModuleKind = ModuleKind(0xFF00);

/// Where the developed image goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputSpace {
    /// Linear Rec.2020 → the target's linear RGB, row-major.
    pub from_rec2020: [f32; 9],
    /// The largest value kept (1 for SDR).
    pub peak: f32,
    /// Gamut compression strength into the target (0 = clip only).
    pub gamut: f32,
}

impl OutputSpace {
    /// Linear sRGB / BT.709, SDR.
    pub fn rec709() -> OutputSpace {
        OutputSpace {
            from_rec2020: super::m3_f32(&colour::rec2020_to_rec709()),
            peak: 1.0,
            gamut: 1.0,
        }
    }

    /// The renderer's storage space: `to_storage` maps linear BT.709 to it (row-major).
    pub fn storage(to_storage: &[f32; 9]) -> OutputSpace {
        let s: M3 =
            std::array::from_fn(|i| std::array::from_fn(|j| f64::from(to_storage[i * 3 + j])));
        OutputSpace {
            from_rec2020: super::m3_f32(&colour::mul(&s, &colour::rec2020_to_rec709())),
            ..OutputSpace::rec709()
        }
    }

    /// The node's params (hashed into the cache keys).
    pub fn params(&self) -> Params {
        Params::Other(
            OUTPUT,
            Fields::new()
                .with("from_rec2020", Value::Floats(self.from_rec2020.to_vec()))
                .with("peak", Value::Float(self.peak))
                .with("gamut", Value::Float(self.gamut)),
        )
    }
}

/// The output op.
pub struct OutputOp;

/// Gamut threshold in the target: compression starts 10 % inside the boundary.
pub const THRESHOLD: f32 = 0.9;

/// The 16 uniforms: matrix (9), threshold, strength, limits (3), power, peak.
pub fn uniforms(p: &Params) -> [f32; 16] {
    let f = p.to_fields();
    let id = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let m = f.floats_or("from_rec2020", id);
    let m = if m.iter().all(|v| v.is_finite()) {
        m
    } else {
        id
    };
    let peak = f.f32_or("peak", 1.0);
    let peak = if peak.is_finite() {
        peak.clamp(0.01, 10000.0)
    } else {
        1.0
    };
    let gamut = f.f32_or("gamut", 1.0);
    let gamut = if gamut.is_finite() {
        gamut.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let l = colour::GAMUT_LIMITS;
    let mut u = [0f32; 16];
    u[..9].copy_from_slice(&m);
    u[9] = THRESHOLD;
    u[10] = gamut;
    u[11] = l[0] as f32;
    u[12] = l[1] as f32;
    u[13] = l[2] as f32;
    u[14] = colour::GAMUT_POWER as f32;
    u[15] = peak;
    u
}

impl Op for OutputOp {
    fn name(&self) -> &'static str {
        "output_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        16
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        super::put(out, &uniforms(p));
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[super::MAT3_WGSL, super::colour_calibration::GAMUT_WGSL]
    }
    fn wgsl(&self) -> &'static str {
        "    let v = dv_gamut(dv_mat3(p, c.rgb), p + 9u);
    return vec4<f32>(clamp(v, vec3<f32>(0.0), vec3<f32>(pf(p + 15u))), c.a);"
    }
}
