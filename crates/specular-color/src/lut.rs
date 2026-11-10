//! Multi-dimensional lookup tables and the ICC LUT pipelines built from them
//! (`lut8Type`, `lut16Type`, `lutAtoBType`, `lutBtoAType`).

use crate::curve::{Curve, unit};

/// Largest channel count of any stage (ICC allows 15).
pub const MAX_CH: usize = 16;

/// A colour lookup table: `inputs` dimensions with per-dimension grid sizes, `outputs` values
/// per grid point, normalized to [0, 1]. The first input varies slowest (ICC order).
#[derive(Clone, Debug)]
pub struct Clut {
    grid: Vec<usize>,
    strides: Vec<usize>,
    outputs: usize,
    data: Vec<f32>,
    trilinear: bool,
}

impl Clut {
    /// A table from its grid sizes (each ≥ 2), output count and data (`Π grid · outputs`
    /// values). `None` when the sizes don't add up.
    pub fn new(grid: Vec<usize>, outputs: usize, data: Vec<f32>) -> Option<Clut> {
        if grid.is_empty() || grid.len() > 15 || outputs == 0 || outputs > 15 {
            return None;
        }
        if grid.iter().any(|&g| g < 2) {
            return None;
        }
        let mut strides = vec![0; grid.len()];
        let mut s = outputs;
        for i in (0..grid.len()).rev() {
            strides[i] = s;
            s = s.checked_mul(grid[i])?;
        }
        if s != data.len() {
            return None;
        }
        Some(Clut {
            grid,
            strides,
            outputs,
            data,
            trilinear: false,
        })
    }

    /// Interpolates three-input tables trilinearly instead of tetrahedrally. Little CMS does
    /// this for tables indexed by Lab (output profiles' `B2A`), where the diagonal the
    /// tetrahedra share has no meaning; matching it keeps proofs and round trips identical.
    pub fn set_trilinear(&mut self, on: bool) {
        self.trilinear = on;
    }

    /// Number of input dimensions.
    pub fn inputs(&self) -> usize {
        self.grid.len()
    }

    /// Number of output values.
    pub fn outputs(&self) -> usize {
        self.outputs
    }

    /// Grid size of each input dimension.
    pub fn grid(&self) -> &[usize] {
        &self.grid
    }

    /// The values, `outputs` per grid point, the first input varying slowest.
    pub fn data(&self) -> &[f32] {
        &self.data
    }

    /// Interpolates at `input` (each clamped to [0, 1]) into `out[..outputs]`: tetrahedral for
    /// three inputs, linear over leading dimensions above that, bi-/linear below.
    pub fn eval(&self, input: &[f32], out: &mut [f32]) {
        let n = self.grid.len();
        let mut base = [0usize; MAX_CH];
        let mut frac = [0f32; MAX_CH];
        for d in 0..n {
            let g = self.grid[d] - 1;
            let p = unit(input[d]) * g as f32;
            let i = (p as usize).min(g - 1);
            base[d] = i;
            frac[d] = p - i as f32;
        }
        let off: usize = (0..n).map(|d| base[d] * self.strides[d]).sum();
        self.eval_from(0, off, &frac, out);
    }

    fn eval_from(&self, d: usize, off: usize, frac: &[f32; MAX_CH], out: &mut [f32]) {
        let n = self.grid.len();
        let o = self.outputs;
        match n - d {
            1 => {
                let s = self.strides[d];
                let f = frac[d];
                for (k, out_k) in out.iter_mut().enumerate().take(o) {
                    let a = self.data[off + k];
                    *out_k = a + (self.data[off + s + k] - a) * f;
                }
            }
            2 => {
                let (sx, sy) = (self.strides[d], self.strides[d + 1]);
                let (fx, fy) = (frac[d], frac[d + 1]);
                for (k, out_k) in out.iter_mut().enumerate().take(o) {
                    let v00 = self.data[off + k];
                    let v01 = self.data[off + sy + k];
                    let v10 = self.data[off + sx + k];
                    let v11 = self.data[off + sx + sy + k];
                    let a = v00 + (v01 - v00) * fy;
                    let b = v10 + (v11 - v10) * fy;
                    *out_k = a + (b - a) * fx;
                }
            }
            3 if self.trilinear => self.trilinear(
                off,
                [self.strides[d], self.strides[d + 1], self.strides[d + 2]],
                [frac[d], frac[d + 1], frac[d + 2]],
                out,
            ),
            3 => self.tetra(
                off,
                [self.strides[d], self.strides[d + 1], self.strides[d + 2]],
                [frac[d], frac[d + 1], frac[d + 2]],
                out,
            ),
            _ => {
                let mut lo = [0f32; MAX_CH];
                self.eval_from(d + 1, off, frac, &mut lo);
                let f = frac[d];
                if f == 0.0 {
                    out[..o].copy_from_slice(&lo[..o]);
                    return;
                }
                let mut hi = [0f32; MAX_CH];
                self.eval_from(d + 1, off + self.strides[d], frac, &mut hi);
                for k in 0..o {
                    out[k] = lo[k] + (hi[k] - lo[k]) * f;
                }
            }
        }
    }

    #[inline]
    fn trilinear(&self, off: usize, s: [usize; 3], f: [f32; 3], out: &mut [f32]) {
        let [fx, fy, fz] = f;
        let [sx, sy, sz] = s;
        let c = &self.data;
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        for k in 0..self.outputs {
            let at = |o: usize| c[off + o + k];
            let x00 = lerp(at(0), at(sx), fx);
            let x01 = lerp(at(sz), at(sx + sz), fx);
            let x10 = lerp(at(sy), at(sx + sy), fx);
            let x11 = lerp(at(sy + sz), at(sx + sy + sz), fx);
            out[k] = lerp(lerp(x00, x01, fz), lerp(x10, x11, fz), fy);
        }
    }

    #[inline]
    fn tetra(&self, off: usize, s: [usize; 3], f: [f32; 3], out: &mut [f32]) {
        let [rx, ry, rz] = f;
        let [sx, sy, sz] = s;
        let c = &self.data;
        let (s1, s2, w0, w1, w2);
        // Sort the fractions; walk the cube diagonal through the matching tetrahedron.
        if rx >= ry {
            if ry >= rz {
                (s1, s2, w0, w1, w2) = (sx, sx + sy, rx, ry, rz);
            } else if rx >= rz {
                (s1, s2, w0, w1, w2) = (sx, sx + sz, rx, rz, ry);
            } else {
                (s1, s2, w0, w1, w2) = (sz, sx + sz, rz, rx, ry);
            }
        } else if rz >= ry {
            (s1, s2, w0, w1, w2) = (sz, sy + sz, rz, ry, rx);
        } else if rz >= rx {
            (s1, s2, w0, w1, w2) = (sy, sy + sz, ry, rz, rx);
        } else {
            (s1, s2, w0, w1, w2) = (sy, sx + sy, ry, rx, rz);
        }
        let s3 = sx + sy + sz;
        for k in 0..self.outputs {
            let c0 = c[off + k];
            let c1 = c[off + s1 + k];
            let c2 = c[off + s2 + k];
            let c3 = c[off + s3 + k];
            out[k] = c0 + (c1 - c0) * w0 + (c2 - c1) * w1 + (c3 - c2) * w2;
        }
    }
}

/// One processing element of a LUT pipeline.
#[derive(Clone, Debug)]
pub enum Stage {
    /// One curve per channel.
    Curves(Vec<Curve>),
    /// `out = m · in + offset` on three channels.
    Matrix([f32; 9], [f32; 3]),
    /// A colour lookup table.
    Clut(Clut),
}

/// A pipeline of stages between `inputs` and `outputs` channels, all values normalized.
#[derive(Clone, Debug)]
pub struct Pipeline {
    /// Channels in.
    pub inputs: usize,
    /// Channels out.
    pub outputs: usize,
    /// Stages, applied in order.
    pub stages: Vec<Stage>,
}

impl Pipeline {
    /// Evaluates in place: reads `buf[..inputs]`, leaves the result in `buf[..outputs]`.
    pub fn eval(&self, buf: &mut [f32; MAX_CH]) {
        for s in &self.stages {
            match s {
                Stage::Curves(c) => {
                    for (v, c) in buf.iter_mut().zip(c) {
                        *v = c.eval(*v);
                    }
                }
                Stage::Matrix(m, o) => {
                    let v = [buf[0], buf[1], buf[2]];
                    for r in 0..3 {
                        buf[r] = m[r * 3] * v[0] + m[r * 3 + 1] * v[1] + m[r * 3 + 2] * v[2] + o[r];
                    }
                }
                Stage::Clut(t) => {
                    let mut o = [0f32; MAX_CH];
                    t.eval(&buf[..t.inputs()], &mut o);
                    buf[..t.outputs()].copy_from_slice(&o[..t.outputs()]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_clut(n: usize, g: usize) -> Clut {
        let mut data = Vec::new();
        let total = g.pow(n as u32);
        for i in 0..total {
            let mut idx = i;
            let mut v = vec![0.0; n];
            for d in (0..n).rev() {
                v[d] = (idx % g) as f32 / (g - 1) as f32;
                idx /= g;
            }
            data.extend(v);
        }
        Clut::new(vec![g; n], n, data).unwrap()
    }

    #[test]
    fn identity_tables_reproduce_input() {
        for n in 1..=5 {
            let t = identity_clut(n, 5);
            let input: Vec<f32> = (0..n).map(|i| 0.13 + 0.17 * i as f32).collect();
            let mut out = [0f32; MAX_CH];
            t.eval(&input, &mut out);
            for i in 0..n {
                assert!((out[i] - input[i]).abs() < 1e-5, "n={n} {out:?}");
            }
        }
    }

    #[test]
    fn rejects_bad_sizes() {
        assert!(Clut::new(vec![2, 2], 3, vec![0.0; 11]).is_none());
        assert!(Clut::new(vec![1, 2], 1, vec![0.0; 2]).is_none());
        assert!(Clut::new(vec![], 1, vec![]).is_none());
    }
}
