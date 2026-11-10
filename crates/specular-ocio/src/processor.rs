//! A display/view processor: the operations from an input colour space through a view to
//! a display, run on the CPU in f64, and its bake for the GPU: a log2 shaper over the
//! scene-linear range and a 3D table of the display values (half floats, red fastest).

use crate::config::{ColorSpace, Config, Transform};
use crate::ops::{Negative, Op};
use crate::{Error, Result, builtin, files};

/// A processor's operations.
#[derive(Clone, Debug, PartialEq)]
pub struct Processor {
    pub ops: Vec<Op>,
}

/// Transforms nested at most (colour-space transforms that name each other).
const MAX_NESTING: usize = 16;

fn neg_style(s: &str, default: Negative) -> Negative {
    match s.to_ascii_lowercase().as_str() {
        "mirror" => Negative::Mirror,
        "pass_thru" | "passthru" => Negative::PassThru,
        "linear" => Negative::Linear,
        "clamp" => Negative::Clamp,
        _ => default,
    }
}

struct Builder<'a> {
    config: &'a Config,
    depth: usize,
}

impl Builder<'_> {
    fn transform(&mut self, t: &Transform, inverse: bool, out: &mut Vec<Op>) -> Result<()> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return Err(Error::Config(
                "the config's transforms refer to each other in a loop".into(),
            ));
        }
        let r = self.transform_inner(t, inverse, out);
        self.depth -= 1;
        r
    }

    fn transform_inner(&mut self, t: &Transform, inverse: bool, out: &mut Vec<Op>) -> Result<()> {
        let mut ops = Vec::new();
        let own_inverse;
        match t {
            Transform::Group(children, inv) => {
                for c in children {
                    self.transform(c, false, &mut ops)?;
                }
                own_inverse = *inv;
            }
            Transform::Matrix { m, offset, inverse } => {
                ops.push(Op::Matrix {
                    m: [m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]],
                    offset: [offset[0], offset[1], offset[2]],
                });
                own_inverse = *inverse;
            }
            Transform::Exponent {
                value,
                style,
                inverse,
            } => {
                ops.push(Op::Exponent {
                    e: [value[0], value[1], value[2]],
                    neg: neg_style(style, Negative::Clamp),
                    inverse: false,
                });
                own_inverse = *inverse;
            }
            Transform::ExponentWithLinear {
                gamma,
                offset,
                style,
                inverse,
            } => {
                ops.push(Op::ExpLinear {
                    g: [gamma[0], gamma[1], gamma[2]],
                    o: [offset[0], offset[1], offset[2]],
                    neg: neg_style(style, Negative::Linear),
                    inverse: false,
                });
                own_inverse = *inverse;
            }
            Transform::Log { base, inverse } => {
                ops.push(Op::Log {
                    base: *base,
                    log_slope: [1.0; 3],
                    log_offset: [0.0; 3],
                    lin_slope: [1.0; 3],
                    lin_offset: [0.0; 3],
                    lin_break: None,
                    linear_slope: None,
                    inverse: false,
                });
                own_inverse = *inverse;
            }
            Transform::LogAffine {
                base,
                log_slope,
                log_offset,
                lin_slope,
                lin_offset,
                inverse,
            } => {
                ops.push(Op::Log {
                    base: *base,
                    log_slope: *log_slope,
                    log_offset: *log_offset,
                    lin_slope: *lin_slope,
                    lin_offset: *lin_offset,
                    lin_break: None,
                    linear_slope: None,
                    inverse: false,
                });
                own_inverse = *inverse;
            }
            Transform::LogCamera {
                base,
                log_slope,
                log_offset,
                lin_slope,
                lin_offset,
                lin_break,
                linear_slope,
                inverse,
            } => {
                ops.push(Op::Log {
                    base: *base,
                    log_slope: *log_slope,
                    log_offset: *log_offset,
                    lin_slope: *lin_slope,
                    lin_offset: *lin_offset,
                    lin_break: Some(*lin_break),
                    linear_slope: *linear_slope,
                    inverse: false,
                });
                own_inverse = *inverse;
            }
            Transform::Range {
                min_in,
                max_in,
                min_out,
                max_out,
                clamp,
                inverse,
            } => {
                let (scale, offset) = match (min_in, max_in, min_out, max_out) {
                    (Some(a), Some(b), Some(c), Some(d)) if b != a => {
                        let s = (d - c) / (b - a);
                        (s, c - a * s)
                    }
                    (Some(a), _, Some(c), _) => (1.0, c - a),
                    (_, Some(b), _, Some(d)) => (1.0, d - b),
                    _ => (1.0, 0.0),
                };
                if *inverse {
                    // An inverse range maps the outputs back to the inputs.
                    let (scale, offset) = match (min_out, max_out, min_in, max_in) {
                        (Some(a), Some(b), Some(c), Some(d)) if b != a => {
                            let s = (d - c) / (b - a);
                            (s, c - a * s)
                        }
                        _ => (1.0, -offset),
                    };
                    ops.push(Op::Range {
                        scale,
                        offset,
                        lo: if *clamp { *min_in } else { None },
                        hi: if *clamp { *max_in } else { None },
                    });
                } else {
                    ops.push(Op::Range {
                        scale,
                        offset,
                        lo: if *clamp { *min_out } else { None },
                        hi: if *clamp { *max_out } else { None },
                    });
                }
                own_inverse = false;
            }
            Transform::Cdl {
                slope,
                offset,
                power,
                sat,
                clamp,
                inverse,
            } => {
                ops.push(Op::Cdl {
                    slope: *slope,
                    offset: *offset,
                    power: *power,
                    sat: *sat,
                    clamp: *clamp,
                    inverse: *inverse,
                });
                own_inverse = false;
            }
            Transform::File {
                src,
                interpolation,
                inverse,
            } => {
                let path = self.config.find_file(src)?;
                ops = files::read(&path, interpolation)?;
                own_inverse = *inverse;
            }
            Transform::ColorSpace { src, dst, inverse } => {
                let (s, d) = if *inverse { (dst, src) } else { (src, dst) };
                let a = self.cs(s)?.clone();
                let b = self.cs(d)?.clone();
                self.between(&a, &b, &mut ops)?;
                own_inverse = false;
            }
            Transform::DisplayView { .. } => {
                return Err(Error::Unsupported(
                    "display-view transforms inside colour spaces are not supported".into(),
                ));
            }
            Transform::Builtin { style, inverse } => {
                ops = builtin::ops(style, *inverse)?;
                own_inverse = false;
            }
            Transform::Unknown(tag) => {
                return Err(Error::Unsupported(format!("the {tag} is not supported")));
            }
        }
        let inv = own_inverse ^ inverse;
        if inv {
            out.extend(builtin::inverted(ops)?);
        } else {
            out.extend(ops);
        }
        Ok(())
    }

    fn cs(&self, n: &str) -> Result<&ColorSpace> {
        self.config
            .colorspace(n)
            .ok_or_else(|| Error::Missing(format!("the colour space {n} is not in the config")))
    }

    /// Colour space → its reference space.
    fn enter_reference(&mut self, cs: &ColorSpace, out: &mut Vec<Op>) -> Result<()> {
        if cs.is_data {
            return Ok(());
        }
        match (&cs.to_ref, &cs.from_ref) {
            (Some(t), _) => self.transform(t, false, out),
            (None, Some(t)) => self.transform(t, true, out),
            (None, None) => Ok(()),
        }
    }

    /// Reference space → colour space.
    fn leave_reference(&mut self, cs: &ColorSpace, out: &mut Vec<Op>) -> Result<()> {
        if cs.is_data {
            return Ok(());
        }
        match (&cs.from_ref, &cs.to_ref) {
            (Some(t), _) => self.transform(t, false, out),
            (None, Some(t)) => self.transform(t, true, out),
            (None, None) => Ok(()),
        }
    }

    /// Between two colour spaces of the same reference kind (scene or display); scene to
    /// display goes through the default view transform.
    fn between(&mut self, a: &ColorSpace, b: &ColorSpace, out: &mut Vec<Op>) -> Result<()> {
        if a.is_data || b.is_data {
            return Ok(());
        }
        self.enter_reference(a, out)?;
        if a.display != b.display {
            let vt = self
                .config
                .default_view_transform
                .as_deref()
                .and_then(|n| self.config.view_transform(n))
                .or(self.config.view_transforms.first())
                .ok_or_else(|| {
                    Error::Config("scene and display colour spaces need a view transform".into())
                })?
                .clone();
            self.view_transform(&vt, !a.display, out)?;
        }
        self.leave_reference(b, out)
    }

    /// A view transform from the scene reference to the display reference (`forward`), or
    /// back.
    fn view_transform(
        &mut self,
        vt: &crate::config::ViewTransform,
        forward: bool,
        out: &mut Vec<Op>,
    ) -> Result<()> {
        let none = || Error::Config(format!("the view transform {} has no transform", vt.name));
        if forward {
            match (&vt.from_scene, &vt.to_display) {
                (Some(t), _) => self.transform(t, false, out),
                (None, Some(t)) => self.transform(t, false, out),
                _ => match &vt.to_scene {
                    Some(t) => self.transform(t, true, out),
                    None => Err(none()),
                },
            }
        } else {
            match &vt.to_scene {
                Some(t) => self.transform(t, false, out),
                None => match &vt.from_scene {
                    Some(t) => self.transform(t, true, out),
                    None => Err(none()),
                },
            }
        }
    }
}

impl Processor {
    /// Input colour space → display through view.
    pub fn display_view(
        config: &Config,
        input: &str,
        display: &str,
        view: &str,
    ) -> Result<Processor> {
        let mut b = Builder { config, depth: 0 };
        let src = b.cs(input)?.clone();
        let v = config
            .views(display)
            .into_iter()
            .find(|v| v.name == view)
            .or_else(|| {
                config
                    .displays
                    .iter()
                    .find(|(n, _)| n == display)
                    .and_then(|(_, vs)| vs.iter().find(|v| v.name == view))
            })
            .ok_or_else(|| {
                Error::Missing(format!("the view {view} of {display} is not in the config"))
            })?
            .clone();
        if !v.looks.trim().is_empty() {
            return Err(Error::Unsupported(format!(
                "the view {view} applies looks, which are not supported yet"
            )));
        }
        let mut ops = Vec::new();
        let dst = b.cs(&v.colorspace)?.clone();
        // Data passes unchanged, whatever the view.
        if src.is_data || dst.is_data {
            return Ok(Processor { ops });
        }
        match &v.view_transform {
            Some(vt_name) => {
                let vt = config
                    .view_transform(vt_name)
                    .ok_or_else(|| {
                        Error::Missing(format!("the view transform {vt_name} is not in the config"))
                    })?
                    .clone();
                b.enter_reference(&src, &mut ops)?;
                b.view_transform(&vt, !src.display, &mut ops)?;
                b.leave_reference(&dst, &mut ops)?;
            }
            None => b.between(&src, &dst, &mut ops)?,
        }
        Ok(Processor { ops })
    }

    /// Colour space → colour space.
    pub fn colorspaces(config: &Config, src: &str, dst: &str) -> Result<Processor> {
        let mut b = Builder { config, depth: 0 };
        let (a, c) = (b.cs(src)?.clone(), b.cs(dst)?.clone());
        let mut ops = Vec::new();
        b.between(&a, &c, &mut ops)?;
        Ok(Processor { ops })
    }

    pub fn apply(&self, c: [f64; 3]) -> [f64; 3] {
        let mut c = c;
        for op in &self.ops {
            op.apply(&mut c);
        }
        c
    }
}

/// A processor baked for the GPU.
#[derive(Clone, Debug, PartialEq)]
pub struct Baked {
    /// Points per axis of the table.
    pub size: usize,
    /// The shaper: input `x` is looked up at `(log2(max(x, 2^lo)) - lo) / (hi - lo)`.
    pub log2_lo: f32,
    pub log2_hi: f32,
    /// `size³` RGBA texels, red fastest, as f32.
    pub texels: Vec<[f32; 4]>,
}

impl Baked {
    /// The shaped coordinate of a linear value.
    pub fn shape(&self, x: f32) -> f32 {
        let l = x.max(self.log2_lo.exp2()).log2();
        ((l - self.log2_lo) / (self.log2_hi - self.log2_lo)).clamp(0.0, 1.0)
    }

    /// What the GPU computes for `c` (trilinear lookup of the shaped coordinates), on the
    /// CPU: for comparing a bake with its processor.
    pub fn lookup(&self, c: [f32; 3]) -> [f32; 3] {
        let n = self.size;
        let mut i0 = [0usize; 3];
        let mut f = [0f32; 3];
        for k in 0..3 {
            let x = self.shape(c[k]) * (n - 1) as f32;
            let i = (x.floor() as usize).min(n - 2);
            i0[k] = i;
            f[k] = x - i as f32;
        }
        let at = |r: usize, g: usize, b: usize| self.texels[(b * n + g) * n + r];
        let mut o = [0f32; 3];
        for (k, ok) in o.iter_mut().enumerate() {
            let mut acc = 0.0;
            for (dr, wr) in [(0, 1.0 - f[0]), (1, f[0])] {
                for (dg, wg) in [(0, 1.0 - f[1]), (1, f[1])] {
                    for (db, wb) in [(0, 1.0 - f[2]), (1, f[2])] {
                        acc += wr * wg * wb * at(i0[0] + dr, i0[1] + dg, i0[2] + db)[k];
                    }
                }
            }
            *ok = acc;
        }
        o
    }
}

/// Bakes `p` into a `size`³ table over the log2 range `[lo, hi]` of each input channel,
/// its display values clipped to 0..1 (SDR displays).
pub fn bake(p: &Processor, size: usize, log2_lo: f32, log2_hi: f32) -> Baked {
    let n = size.max(2);
    let mut texels = Vec::with_capacity(n * n * n);
    let at = |i: usize| {
        let t = i as f64 / (n - 1) as f64;
        (f64::from(log2_lo) + t * f64::from(log2_hi - log2_lo)).exp2()
    };
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                // Clipped to what an SDR display shows, so cells across an edge of the
                // gamut don't interpolate far-out values into visible ones.
                let o = p
                    .apply([at(r), at(g), at(b)])
                    .map(|v| v.clamp(0.0, 1.0) as f32);
                texels.push([o[0], o[1], o[2], 1.0]);
            }
        }
    }
    Baked {
        size: n,
        log2_lo,
        log2_hi,
        texels,
    }
}
