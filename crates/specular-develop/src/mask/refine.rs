//! The refine pass of a mask: a node of its own before the masked module. Its local program
//! makes the mask over the node's region and refines it; the kernel writes the result into
//! the alpha channel, where the module's blend reads it. In order:
//!
//! 1. **Details measure** (when a details mask or `details` needs it): the log luminance,
//!    box-blurred by a third of the window, then its local standard deviation over the
//!    window ([`super::DETAIL_WINDOW`] of the long side). Lengths are fractions of the long
//!    side, so the preview and an export measure the same structure; white noise blurred
//!    over a fixed fraction of the frame has the same deviation at any scale.
//! 2. **The mask** (the program; a details leaf reads the measure), then `details` keeps it
//!    on detail or on flat areas. Ranges on the module's output see the module's own output
//!    when the module is a plain point op (its function is called here too); otherwise they
//!    see its input.
//! 3. **Feather**: the mask grows outwards by a soft edge (two box blurs, then
//!    `max(m, min(1, 2·blurred))`, so the inside stays).
//! 4. **Edge refinement**: the guided filter (He, Sun and Tang 2013, the tone equalizer's
//!    kernels) guided by the input's log luminance.
//! 5. **Blur** (two box blurs).
//!
//! Every step is always in the program; a step that is off gets no threads, so moving a
//! value never changes the kernels.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use crate::graph::{Kernel, NodeKind, Op, Program, Roi};
use crate::model::{Fields, ModuleKind, Params, Value};
use crate::ops::tone::kernels::{self, Alloc, BOX_H, BOX_V, NONE, fw, threads};

/// The refine node's kind (never in an edit).
pub const REFINE: ModuleKind = ModuleKind(0xFF02);

/// The mask at a pixel, its details measure and `details` (WGSL, shared by both mask
/// steps). `c` the input, `b` what output ranges test.
const EVAL_BODY: &str = "    let at = t.y * pu(p + 1u) + t.x;
    if (pu(p + 5u) != 0xFFFFFFFFu) { dv_detail = S[pu(p + 5u) + at]; }
    var m = dv_mask(pu(p + 4u), c.rgb, b.rgb, x, y);
    let dd = pf(p + 6u);
    if (dd != 0.0) {
        let th = abs(dd);
        let k = dv_ss(0.75 * th, 1.25 * th, dv_detail);
        m = m * select(1.0 - k, k, dd > 0.0);
    }
    S[pu(p) + at] = m;";

/// Words: plane, width, origin x, origin y, program, details plane (or none), details
/// (f32). The mask of the input at origin + t; output ranges test the input.
pub static MASK_EVAL: LazyLock<Kernel> = LazyLock::new(|| Kernel {
    name: "mask_eval_v2",
    atomic: false,
    helpers: &*super::shader::HELPERS,
    body: Box::leak(
        format!(
            "    let x = i32(pu(p + 2u) + t.x);
    let y = i32(pu(p + 3u) + t.y);
    let c = src_at(x, y);
    let b = c;
{EVAL_BODY}"
        )
        .into_boxed_str(),
    ),
});

/// [`MASK_EVAL`] with output ranges testing a point op's output: the op's function is
/// called on the input with its params from word 8. One kernel per op, made once.
fn mask_eval_with(op: &dyn Op) -> &'static Kernel {
    static MADE: LazyLock<Mutex<HashMap<&'static str, &'static Kernel>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let name = op.name();
    let mut made = MADE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(k) = made.get(name) {
        return k;
    }
    let mut helpers: Vec<&'static str> = super::shader::HELPERS.to_vec();
    for h in op.helpers() {
        if !helpers.contains(h) {
            helpers.push(h);
        }
    }
    let func: &'static str = Box::leak(
        format!(
            "fn op_{name}(c: vec4<f32>, px: i32, py: i32, p: u32) -> vec4<f32> {{\n{}\n}}\n",
            op.wgsl()
        )
        .into_boxed_str(),
    );
    helpers.push(func);
    let k: &'static Kernel = Box::leak(Box::new(Kernel {
        name: Box::leak(format!("mask_eval_v2_{name}").into_boxed_str()),
        atomic: false,
        helpers: Box::leak(helpers.into_boxed_slice()),
        body: Box::leak(
            format!(
                "    let x = i32(pu(p + 2u) + t.x);
    let y = i32(pu(p + 3u) + t.y);
    let c = src_at(x, y);
    let b = op_{name}(c, x, y, p + 8u);
{EVAL_BODY}"
            )
            .into_boxed_str(),
        ),
    }));
    made.insert(name, k);
    k
}

/// Words: moments (four planes: mean, mean, mean of squares twice), plane size,
/// destination, width. The standard deviation.
pub static DETAIL: Kernel = Kernel {
    name: "mask_detail_v1",
    atomic: false,
    helpers: &[],
    body: "    let at = t.y * pu(p + 3u) + t.x;
    let m = S[pu(p) + at];
    let q = S[pu(p) + 2u * pu(p + 1u) + at];
    S[pu(p + 2u) + at] = sqrt(max(q - m * m, 0.0));",
};

/// Words: mask, blurred, width. The mask grows by the blurred edge.
pub static GROW: Kernel = Kernel {
    name: "mask_grow_v1",
    atomic: false,
    helpers: &[],
    body: "    let at = t.y * pu(p + 2u) + t.x;
    S[pu(p) + at] = max(S[pu(p) + at], min(1.0, 2.0 * S[pu(p + 1u) + at]));",
};

/// Words: destination, mask, guide, coefficients (Ā then B̄, plane size apart), plane size,
/// width, on. The guided filter's result, or the mask as it is.
pub static APPLY: Kernel = Kernel {
    name: "mask_apply_v1",
    atomic: false,
    helpers: &[],
    body: "    let at = t.y * pu(p + 5u) + t.x;
    var m = S[pu(p + 1u) + at];
    if (pu(p + 6u) != 0u) {
        m = S[pu(p + 3u) + at] * S[pu(p + 2u) + at] + S[pu(p + 3u) + pu(p + 4u) + at];
    }
    S[pu(p) + at] = clamp(m, 0.0, 1.0);",
};

/// The node's params.
pub fn params(spec: &super::RefineSpec) -> Params {
    Params::Other(
        REFINE,
        Fields::new()
            .with("program", Value::Int(i64::from(spec.program)))
            .with("radius", Value::Float(spec.radius_per_scale))
            .with("eps", Value::Float(spec.eps))
            .with("detail", Value::Float(spec.detail_per_scale))
            .with("details", Value::Float(spec.details))
            .with("feather", Value::Float(spec.feather_per_scale))
            .with("blur", Value::Float(spec.blur_per_scale)),
    )
}

/// A length field in pixels at `scale`: 0 when off, else at least 1.
fn px(f: &Fields, key: &str, scale: f32) -> u32 {
    let r = f.f32_or(key, 0.0) * scale;
    if r.is_finite() && r > 0.0 {
        r.round().clamp(1.0, 512.0) as u32
    } else {
        0
    }
}

/// The radii of one render: details window, its pre-blur, feather, edge, blur.
struct Radii {
    detail: u32,
    pre: u32,
    feather: u32,
    edge: u32,
    blur: u32,
}

fn radii(p: &Params, scale: f32) -> Radii {
    let f = p.to_fields();
    let detail = px(&f, "detail", scale);
    // Two box passes of half the radius each spread over about the radius.
    let half = |r: u32| if r == 0 { 0 } else { r.div_ceil(2) };
    Radii {
        detail,
        pre: if detail == 0 {
            0
        } else {
            ((detail + 1) / 3).max(1)
        },
        feather: half(px(&f, "feather", scale)),
        edge: px(&f, "radius", scale),
        blur: half(px(&f, "blur", scale)),
    }
}

/// The refine op; with `module`, output ranges test that point op's output.
pub struct RefineOp {
    pub module: Option<(Arc<dyn Op>, Params)>,
}

impl RefineOp {
    /// The refine node of a module with `op`: output ranges see its output when `output`
    /// (the mask tests the output) and the op is a plain point op.
    pub fn for_module(op: &Arc<dyn Op>, params: &Params, output: bool) -> RefineOp {
        let plain =
            || op.kind() == NodeKind::Point && !op.has_stats(params) && op.data(params).is_none();
        RefineOp {
            module: (output && plain()).then(|| (op.clone(), params.clone())),
        }
    }
}

impl Op for RefineOp {
    fn name(&self) -> &'static str {
        "mask_refine_v2"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Neighbourhood
    }
    fn halo(&self, p: &Params, scale: f32) -> u32 {
        let r = radii(p, scale);
        r.pre + r.detail + 2 * r.feather + 2 * r.edge + 2 * r.blur + 2
    }
    fn param_words(&self) -> u32 {
        0
    }
    fn write_params(&self, _: &Params, _: f32, _: &mut [u32]) {}
    fn local(&self, p: &Params, input: Roi, scale: f32) -> Option<Program> {
        let f = p.to_fields();
        let prog_at = f.i64_or("program", 0).clamp(0, i64::from(u32::MAX)) as u32;
        let eps = f.f32_or("eps", 0.01).max(1e-6);
        let details = f.f32_or("details", 0.0);
        let details = if details.is_finite() { details } else { 0.0 };
        let r = radii(p, scale);
        let (w, h) = (input.w, input.h);
        let n = u64::from(w) * u64::from(h);
        let ps = n as u32;
        let mut a = Alloc(0);
        let i = a.take(n);
        let pm = a.take(n);
        let fm = a.take(n);
        let d = a.take(n);
        let out = a.take(2 * n);
        // Scratch each stage reuses.
        let mark = a;
        let mut prog = Program::default();
        prog.step(&kernels::LUM, [w, h], vec![i, w, input.x, input.y]);
        // 1. The details measure.
        let det = r.detail > 0;
        let th = threads(det, w, h);
        let ls = a.take(n);
        let t4 = a.take(4 * n);
        let m4 = a.take(4 * n);
        prog.step(&BOX_H, th, vec![i, t4, w, h, r.pre, 1, ps, 0, 0]);
        prog.step(&BOX_V, th, vec![t4, ls, w, h, r.pre, 1, ps, 0, 0]);
        prog.step(&BOX_H, th, vec![ls, t4, w, h, r.detail, 4, ps, 1, ls]);
        prog.step(&BOX_V, th, vec![t4, m4, w, h, r.detail, 4, ps, 0, 0]);
        prog.step(&DETAIL, th, vec![m4, ps, d, w]);
        let mut top = a.0;
        // 2. The mask.
        let mut words = vec![
            pm,
            w,
            input.x,
            input.y,
            prog_at,
            if det { d } else { NONE },
            fw(details),
            0,
        ];
        let kernel: &'static Kernel = match &self.module {
            Some((op, mp)) => {
                let mut pw = vec![0u32; op.param_words() as usize];
                op.write_params(mp, scale, &mut pw);
                words.extend(pw);
                mask_eval_with(op.as_ref())
            }
            None => &MASK_EVAL,
        };
        prog.step(kernel, [w, h], words);
        // 3. Feather.
        a = mark;
        let fa = a.take(n);
        let fb = a.take(n);
        let th = threads(r.feather > 0, w, h);
        prog.step(&BOX_H, th, vec![pm, fa, w, h, r.feather, 1, ps, 0, 0]);
        prog.step(&BOX_V, th, vec![fa, fb, w, h, r.feather, 1, ps, 0, 0]);
        prog.step(&BOX_H, th, vec![fb, fa, w, h, r.feather, 1, ps, 0, 0]);
        prog.step(&BOX_V, th, vec![fa, fb, w, h, r.feather, 1, ps, 0, 0]);
        prog.step(&GROW, th, vec![pm, fb, w]);
        top = top.max(a.0);
        // 4. Edge refinement.
        a = mark;
        let edge = r.edge > 0;
        kernels::guided(
            &mut prog,
            &mut a,
            i,
            pm,
            false,
            out,
            (w, h),
            r.edge.max(1),
            eps,
            edge,
        );
        top = top.max(a.0);
        prog.step(&APPLY, [w, h], vec![fm, pm, i, out, ps, w, u32::from(edge)]);
        // 5. Blur.
        a = mark;
        let fa = a.take(n);
        let th = threads(r.blur > 0, w, h);
        for _ in 0..2 {
            prog.step(&BOX_H, th, vec![fm, fa, w, h, r.blur, 1, ps, 0, 0]);
            prog.step(&BOX_V, th, vec![fa, fm, w, h, r.blur, 1, ps, 0, 0]);
        }
        prog.words = top.max(a.0);
        Some(prog)
    }
    fn wgsl(&self) -> &'static str {
        "    let c = src_at(x, y);
    let rw = pu(18u);
    let at = u32(y - i32(pu(17u))) * rw + u32(x - i32(pu(16u)));
    let n = rw * pu(19u);
    return vec4<f32>(c.rgb, clamp(lc(2u * n + at), 0.0, 1.0));"
    }
}

/// The overlay node's kind (never in an edit).
pub const OVERLAY: ModuleKind = ModuleKind(0xFF03);

/// Draws the focused module's mask (in the alpha) over the output: mode 1 tints it red,
/// mode 2 shows it alone in grey.
pub struct OverlayOp;

pub fn overlay_params(o: super::Overlay) -> Params {
    let mode = match o {
        super::Overlay::Tint => 1,
        super::Overlay::Grey => 2,
    };
    Params::Other(OVERLAY, Fields::new().with("mode", Value::Int(mode)))
}

impl Op for OverlayOp {
    fn name(&self) -> &'static str {
        "mask_overlay_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        1
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        out[0] = p.to_fields().i64_or("mode", 1).clamp(0, 2) as u32;
    }
    fn wgsl(&self) -> &'static str {
        "    let m = clamp(c.a, 0.0, 1.0);
    if (pu(p) == 2u) { return vec4<f32>(vec3<f32>(m), 1.0); }
    return vec4<f32>(mix(c.rgb, vec3<f32>(1.0, 0.0, 0.0), 0.5 * m), 1.0);"
    }
}
