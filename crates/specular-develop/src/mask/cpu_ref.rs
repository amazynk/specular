//! The CPU reference of masks and blends (tests only): the same definitions as the kernels,
//! evaluated straight from the edit model in f64.

use std::collections::HashMap;

use super::Space;
use super::brush::Raster;
use crate::colour::oklab::{from_rec2020_rel, to_rec2020_rel};
use crate::model::Edit;
use crate::model::mask::{Blend, BlendMode, Combine, MaskExpr, RangeChannel, Shape};

const LUMA: [f64; 3] = [0.2627, 0.6780, 0.0593];

pub fn ss(e0: f64, e1: f64, x: f64) -> f64 {
    if e1 <= e0 {
        return if x >= e0 { 1.0 } else { 0.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn edge(d: f64, w: f64) -> f64 {
    1.0 - ss(-w, 0.0, d)
}

fn rot(d: (f64, f64), deg: f32) -> (f64, f64) {
    let (sn, cs) = f64::from(deg).to_radians().sin_cos();
    (d.0 * cs + d.1 * sn, -d.0 * sn + d.1 * cs)
}

fn l(v: f32) -> f64 {
    f64::from(v).max(0.0)
}

fn c01(v: f32) -> f64 {
    f64::from(v).clamp(0.0, 1.0)
}

/// The mask of `shape` at frame point `f` (fractions); `aa` the pixel band (shape units).
pub fn shape_mask(shape: &Shape, sp: Space, f: (f64, f64), aa: f64, r: Option<&Raster>) -> f64 {
    let p = (f.0 * sp.ax, f.1 * sp.ay);
    let pos = |c: [f32; 2]| sp.pos(c);
    match shape {
        Shape::Ellipse {
            centre,
            radii,
            angle_deg,
            feather,
        } => {
            let c = pos(*centre);
            let (rx, ry) = (l(radii[0]).max(1e-6), l(radii[1]).max(1e-6));
            let q = rot((p.0 - c.0, p.1 - c.1), *angle_deg);
            let k1 = ((q.0 / (rx * rx)).powi(2) + (q.1 / (ry * ry)).powi(2)).sqrt();
            let d = if k1 < 1e-12 {
                -rx.min(ry)
            } else {
                let k0 = ((q.0 / rx).powi(2) + (q.1 / ry).powi(2)).sqrt();
                k0 * (k0 - 1.0) / k1
            };
            edge(d, (c01(*feather) * rx.min(ry)).max(aa))
        }
        Shape::Rectangle {
            centre,
            half,
            angle_deg,
            round,
            feather,
        } => {
            let c = pos(*centre);
            let (hx, hy) = (l(half[0]).max(1e-6), l(half[1]).max(1e-6));
            let m = hx.min(hy);
            let rr = c01(*round) * m;
            let q = rot((p.0 - c.0, p.1 - c.1), *angle_deg);
            let d = (q.0.abs() - hx + rr, q.1.abs() - hy + rr);
            let out = (d.0.max(0.0).powi(2) + d.1.max(0.0).powi(2)).sqrt();
            let sd = out + d.0.max(d.1).min(0.0) - rr;
            edge(sd, (c01(*feather) * m).max(aa))
        }
        Shape::Polygon { points, .. } => {
            let v: Vec<(f64, f64, f64)> = points
                .iter()
                .map(|q| {
                    let (x, y) = pos([q.x, q.y]);
                    (x, y, l(q.feather))
                })
                .collect();
            if v.len() < 3 {
                return 0.0;
            }
            let mut dmin = (p.0 - v[0].0).powi(2) + (p.1 - v[0].1).powi(2);
            let mut fw = v[0].2;
            let mut s = 1.0;
            let mut j = v.len() - 1;
            for i in 0..v.len() {
                let (vi, vj) = (v[i], v[j]);
                let e = (vj.0 - vi.0, vj.1 - vi.1);
                let w = (p.0 - vi.0, p.1 - vi.1);
                let ee = (e.0 * e.0 + e.1 * e.1).max(1e-20);
                let t = ((w.0 * e.0 + w.1 * e.1) / ee).clamp(0.0, 1.0);
                let q = (w.0 - e.0 * t, w.1 - e.1 * t);
                let d = q.0 * q.0 + q.1 * q.1;
                if d < dmin {
                    dmin = d;
                    fw = vi.2 + (vj.2 - vi.2) * t;
                }
                let c0 = p.1 >= vi.1;
                let c1 = p.1 < vj.1;
                let c2 = e.0 * w.1 > e.1 * w.0;
                if (c0 && c1 && c2) || (!c0 && !c1 && !c2) {
                    s = -s;
                }
                j = i;
            }
            edge(s * dmin.sqrt(), fw.max(aa))
        }
        Shape::Linear { start, end } => {
            let s = pos(*start);
            let e = pos(*end);
            let d = (e.0 - s.0, e.1 - s.1);
            let ee = d.0 * d.0 + d.1 * d.1;
            if ee < 1e-20 {
                return 0.0;
            }
            1.0 - ss(0.0, 1.0, ((p.0 - s.0) * d.0 + (p.1 - s.1) * d.1) / ee)
        }
        Shape::Radial {
            centre,
            inner,
            outer,
            angle_deg,
        } => {
            let c = pos(*centre);
            let q = rot((p.0 - c.0, p.1 - c.1), *angle_deg);
            let n = |r: [f32; 2]| {
                ((q.0 / l(r[0]).max(1e-6)).powi(2) + (q.1 / l(r[1]).max(1e-6)).powi(2)).sqrt()
            };
            let (ri, ro) = (n(*inner), n(*outer));
            if ri <= 1.0 {
                1.0
            } else if ro >= 1.0 {
                0.0
            } else {
                let t = (1.0 - 1.0 / ri) / (1.0 / ro - 1.0 / ri).max(1e-20);
                1.0 - ss(0.0, 1.0, t)
            }
        }
        Shape::Brush { .. } => r.map_or(0.0, |r| f64::from(r.sample(f.0 as f32, f.1 as f32))),
    }
}

fn chan(ch: RangeChannel, c: [f32; 3]) -> f64 {
    let c = c.map(f64::from);
    match ch {
        RangeChannel::Luminance => {
            let y = c[0] * LUMA[0] + c[1] * LUMA[1] + c[2] * LUMA[2];
            (y.max(1.525_878_906_25e-5) / 0.18).log2()
        }
        RangeChannel::Chroma => {
            let l = from_rec2020_rel(c);
            l[1].hypot(l[2])
        }
        RangeChannel::Hue => {
            let l = from_rec2020_rel(c);
            let h = l[2].atan2(l[1]).to_degrees();
            if h < 0.0 { h + 360.0 } else { h }
        }
        RangeChannel::Red => c[0],
        RangeChannel::Green => c[1],
        RangeChannel::Blue => c[2],
    }
}

/// Everything an expression needs besides the pixel.
pub struct Ctx<'a> {
    pub edit: &'a Edit,
    pub space: Space,
    pub aa: f64,
    pub rasters: HashMap<u32, &'a Raster>,
    /// AI rasters by key.
    pub ai: HashMap<[u8; 32], &'a Raster>,
    /// The details measure at the pixel being evaluated (EV).
    pub detail: std::cell::Cell<f64>,
}

/// Contrast `c` and offset `o` of a mask value (both clamped to -1..1).
fn level(m: f64, c: f32, o: f32) -> f64 {
    let k = |v: f32| f64::from(v).clamp(-1.0, 1.0);
    ((m - 0.5) * (1.0 + k(c)) + 0.5 + k(o)).clamp(0.0, 1.0)
}

/// The mask of `e` at frame point `f` for input `a` and output `b`.
pub fn expr_mask(cx: &Ctx<'_>, e: &MaskExpr, f: (f64, f64), a: [f32; 3], b: [f32; 3]) -> f64 {
    let r = match e {
        MaskExpr::All => 1.0,
        MaskExpr::Shape(id) => cx.edit.mask(*id).map_or(0.0, |s| {
            shape_mask(&s.shape, cx.space, f, cx.aa, cx.rasters.get(id).copied())
        }),
        MaskExpr::Range(rg) => {
            let mut l = rg.limits.map(f64::from);
            for i in 1..4 {
                l[i] = l[i].max(l[i - 1]);
            }
            let c = if rg.on_output { b } else { a };
            let mut x = chan(rg.channel, c);
            if rg.channel == RangeChannel::Hue {
                x = l[0] + ((x - l[0]).rem_euclid(360.0));
            }
            ss(l[0], l[1], x) * (1.0 - ss(l[2], l[3], x))
        }
        MaskExpr::ColourRange(c) => {
            let lab = from_rec2020_rel(a.map(f64::from));
            let d = c
                .colours
                .iter()
                .map(|k| {
                    let k = k.map(f64::from);
                    ((lab[0] - k[0]).powi(2) + (lab[1] - k[1]).powi(2) + (lab[2] - k[2]).powi(2))
                        .sqrt()
                })
                .fold(f64::INFINITY, f64::min);
            if c.colours.is_empty() {
                0.0
            } else {
                let r = l(c.distance);
                1.0 - ss(r, r + l(c.falloff), d)
            }
        }
        MaskExpr::FromModule { kind, instance } => cx
            .edit
            .module(*kind, *instance)
            .and_then(|m| m.blend.as_ref())
            .map_or(0.0, |bl| {
                let mut m = expr_mask(cx, &bl.mask, f, a, b);
                // The source's contrast and offset (opcode LEVEL, in f32 like the shader).
                let (c, o) = (bl.refine.contrast, bl.refine.offset);
                if (c.is_finite() && c != 0.0) || (o.is_finite() && o != 0.0) {
                    m = level(m, c, o);
                }
                if bl.invert { 1.0 - m } else { m }
            }),
        MaskExpr::Ai(m) => cx
            .ai
            .get(&m.key)
            .map_or(0.0, |r| f64::from(r.sample(f.0 as f32, f.1 as f32))),
        MaskExpr::Depth(d) => {
            let k = super::depth::key(&super::depth::photo(&cx.edit.source), &d.model);
            cx.ai.get(&k).map_or(0.0, |r| {
                let t = r.sample(f.0 as f32, f.1 as f32);
                let fe = if d.feather.is_finite() {
                    d.feather.clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let c = |v: f32| {
                    if v.is_finite() {
                        v.clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                };
                let (n, r) = (c(d.near), c(d.far));
                f64::from(
                    crate::model::mask::DepthRange {
                        model: String::new(),
                        near: n.min(r),
                        far: n.max(r),
                        feather: fe,
                    }
                    .weight(t),
                )
            })
        }
        MaskExpr::Details(d) => {
            let (t, s) = (f64::from(d.threshold), 0.5 * l(d.softness));
            ss(t - s, t + s, cx.detail.get())
        }
        MaskExpr::Invert(x) => 1.0 - expr_mask(cx, x, f, a, b),
        MaskExpr::Opacity(k, x) => expr_mask(cx, x, f, a, b) * c01(*k),
        MaskExpr::Combine(op, x, y) => {
            let (x, y) = (expr_mask(cx, x, f, a, b), expr_mask(cx, y, f, a, b));
            match op {
                Combine::Union => x.max(y),
                Combine::Intersect => x.min(y),
                Combine::Subtract => x * (1.0 - y),
                Combine::Exclude => (x - y).abs(),
            }
        }
    };
    r.clamp(0.0, 1.0)
}

fn enc(x: f64) -> f64 {
    let v = x.max(0.0);
    v / (v + 0.18)
}

fn dec(y: f64) -> f64 {
    let v = y.clamp(0.0, 0.9999_f32 as f64);
    0.18 * v / (1.0 - v)
}

fn lum(c: [f64; 3]) -> f64 {
    c[0] * LUMA[0] + c[1] * LUMA[1] + c[2] * LUMA[2]
}

/// The blend mode of input `a` and output `b`.
pub fn mode(m: BlendMode, a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let per = |f: &dyn Fn(f64, f64) -> f64| std::array::from_fn(|i| dec(f(enc(a[i]), enc(b[i]))));
    match m {
        BlendMode::Normal => b,
        BlendMode::Multiply => per(&|x, y| x * y),
        BlendMode::Divide => per(&|x, y| x / y.max(1e-6)),
        BlendMode::Add => per(&|x, y| x + y),
        BlendMode::Subtract => per(&|x, y| (x - y).max(0.0)),
        BlendMode::Lighten => std::array::from_fn(|i| a[i].max(b[i])),
        BlendMode::Darken => std::array::from_fn(|i| a[i].min(b[i])),
        BlendMode::Luminance => {
            let ya = lum(a);
            if ya <= 1e-9 {
                b
            } else {
                a.map(|v| v * lum(b) / ya)
            }
        }
        BlendMode::Colour => {
            let yb = lum(b);
            if yb <= 1e-9 {
                a
            } else {
                b.map(|v| v * lum(a) / yb)
            }
        }
        BlendMode::Hue | BlendMode::Chroma => {
            let (la, lb) = (from_rec2020_rel(a), from_rec2020_rel(b));
            let (ca, cb) = (la[1].hypot(la[2]), lb[1].hypot(lb[2]));
            if m == BlendMode::Hue {
                if cb < 1e-9 {
                    return a;
                }
                to_rec2020_rel([la[0], lb[1] * ca / cb, lb[2] * ca / cb])
            } else {
                if ca < 1e-9 {
                    return a;
                }
                to_rec2020_rel([la[0], la[1] * cb / ca, la[2] * cb / ca])
            }
        }
        BlendMode::Screen => per(&|x, y| 1.0 - (1.0 - x) * (1.0 - y)),
        BlendMode::Overlay => per(&|x, y| {
            if x < 0.5 {
                2.0 * x * y
            } else {
                1.0 - 2.0 * (1.0 - x) * (1.0 - y)
            }
        }),
        BlendMode::SoftLight => per(&|x, y| {
            if y <= 0.5 {
                x - (1.0 - 2.0 * y) * x * (1.0 - x)
            } else {
                let d = if x <= 0.25 {
                    ((16.0 * x - 12.0) * x + 4.0) * x
                } else {
                    x.sqrt()
                };
                x + (2.0 * y - 1.0) * (d - x)
            }
        }),
    }
}

/// The blended pixel of a module with `blend`: input `a`, its output `b`, at frame point `f`.
pub fn blend_pixel(cx: &Ctx<'_>, bl: &Blend, f: (f64, f64), a: [f32; 3], b: [f32; 3]) -> [f64; 3] {
    let bl = &*super::effective_blend(cx.edit, bl);
    let mut m = expr_mask(cx, &bl.mask, f, a, b);
    let k = |v: f32| f64::from(v).clamp(-1.0, 1.0);
    m = ((m - 0.5) * (1.0 + k(bl.refine.contrast)) + 0.5 + k(bl.refine.offset)).clamp(0.0, 1.0);
    if bl.invert {
        m = 1.0 - m;
    }
    let w = m * c01(bl.opacity);
    let (a64, b64) = (a.map(f64::from), b.map(f64::from));
    let r = mode(bl.mode, a64, b64);
    std::array::from_fn(|i| a64[i] + (r[i] - a64[i]) * w)
}
