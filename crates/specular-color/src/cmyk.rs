//! Fast CPU conversion of CMYK and L*a*b* pixels to 8-bit sRGB while decoding.
//!
//! CMYK profiles built the usual way (`A2B` input curves, a 4D table, output curves into a
//! L*a*b* or XYZ PCS) are converted from the profile's own table, the way the profile
//! defines it: the input curves are folded into per-byte cell lookups, the table is
//! interpolated like ICC CMMs do (linearly over C, tetrahedrally over M, Y and K), and the
//! rest of the way to sRGB (output curves, PCS decoding, black point compensation, the
//! matrix to sRGB primaries, the sRGB curve) is computed per pixel. Nothing is resampled,
//! so the result is the exact transform to within about half a code (8-bit rounding, and
//! table values and fractions held to 16 and 24 bits).
//!
//! Other profiles (the built-in generic CMYK, other pipelines, tables too big to keep in
//! cache) go through a 4D table baked from the exact transform (tetrahedral over C, M and
//! Y, linear over K, in fixed point, then one lookup per channel to 8-bit sRGB):
//! - Nodes are denser towards full ink, where press behaviour bends most (dot gain, ink
//!   limits, black generation) and where dark colours make errors most visible.
//! - The table holds unclipped values, so the gamut boundary (inks driving a channel below
//!   zero) is not a kink the grid has to follow.
//! - Values are linear light through a gamma with an offset, `(v + A)^(1/2.4) − A^(1/2.4)`,
//!   continued by its tangent below zero: close enough to perceptual in the shadows, and
//!   smooth through zero, unlike the sRGB curve, whose slope jumps there.
//!
//! Speed, both ways: the linear axis is the table's innermost, so one 16-byte load per
//! tetrahedron vertex reads both of its slices; the tetrahedron is picked without branches
//! (random inks would mispredict them); tables stay within a core's L2 cache; the steps
//! after a profile's table run eight pixels at a time.

use crate::curve::Curve;
use crate::icc::{ColorSpace, Error, PcsEncoding, Profile};
use crate::lut::{Clut, Stage};
use crate::math::{D50, Mat3, bt709_to_pcs, linear_to_srgb, mul3};
use crate::profiles;
use crate::transform::{Intent, Transform};
use std::simd::prelude::*;
use std::simd::{StdFloat, simd_swizzle};
use std::sync::OnceLock;

/// Nodes per axis (C, M, Y, K) of a baked table.
const NODES: [usize; 4] = [17, 17, 17, 21];

/// Most nodes in a source table converted directly (17⁴ fits; about 600 KB).
const MAX_SOURCE_NODES: usize = 100_000;

/// Offset of the baked table's gamma curve, in linear light.
const A: f32 = 0.2;

/// Baked table units per unit of the table curve (which spans 0..0.57 over linear 0..1).
const SCALE: f32 = 16384.0;

/// Baked table values are clamped to ±`LIMIT` units (about linear −0.9..2.3), which keeps
/// the C interpolation's products within `i32`.
const LIMIT: f32 = 15872.0;

/// Ink amounts (0..=1) at `n` nodes, denser towards solid ink: steps shrink by a factor of
/// about 2.2 from no ink to solid ink.
fn dense_nodes(n: usize) -> Vec<f32> {
    let mut v: Vec<f32> = (0..n)
        .map(|i| {
            let u = 1.0 - i as f32 / (n - 1) as f32;
            1.0 - u * (0.6 + 0.4 * u)
        })
        .collect();
    v[0] = 0.0;
    v[n - 1] = 1.0;
    v
}

/// Linear light → the table curve: `(v + A)^(1/2.4) − A^(1/2.4)`, continued below zero
/// by its tangent there (so values far outside the gamut stay on a smooth, unclamped line).
fn to_table(v: f32) -> f32 {
    if v >= 0.0 {
        (v + A).powf(1.0 / 2.4) - A.powf(1.0 / 2.4)
    } else {
        v * (A.powf(1.0 / 2.4 - 1.0) / 2.4)
    }
}

/// Table curve (in table units, rounded) → 8-bit sRGB; values below zero map to entry 0,
/// values past the end to the last entry (255).
fn decode_table() -> &'static [u8] {
    static T: OnceLock<Vec<u8>> = OnceLock::new();
    T.get_or_init(|| {
        let a0 = A.powf(1.0 / 2.4);
        let len = (to_table(1.0) * SCALE).ceil() as usize + 2;
        (0..len)
            .map(|j| {
                let lin = (j as f32 / SCALE + a0).powf(2.4) - A;
                (linear_to_srgb(lin.clamp(0.0, 1.0)) * 255.0 + 0.5) as u8
            })
            .collect()
    })
}

/// Entries of [`sqrt_encode_table`], less one.
const ENC_TOP: f32 = 8191.0;

/// Square root of linear light (0..=1, in 8192 steps) → 8-bit sRGB: steps are finest in
/// the shadows, where the sRGB curve is steepest.
fn sqrt_encode_table() -> &'static [u8] {
    static T: OnceLock<Vec<u8>> = OnceLock::new();
    T.get_or_init(|| {
        (0..=ENC_TOP as usize)
            .map(|j| {
                let r = j as f32 / ENC_TOP;
                (linear_to_srgb(r * r) * 255.0 + 0.5) as u8
            })
            .collect()
    })
}

/// Tetrahedra over three axes with strides `a`, `b`, `c`: the two vertices after the cell's
/// first as offsets from it, indexed by [`select`] on the axes' fractions (the two
/// impossible patterns get a valid entry). The weights are the fractions sorted descending.
fn tetra(a: u32, b: u32, c: u32) -> [(u32, u32); 8] {
    [
        (c, c + b), // 000: c > b > a
        (c, c + b), // 001: impossible
        (b, b + c), // 010: b ≥ c > a
        (b, b + a), // 011: b > a ≥ c
        (c, c + a), // 100: c > a ≥ b
        (a, a + c), // 101: a ≥ c > b
        (a, a + b), // 110: impossible
        (a, a + b), // 111: a ≥ b ≥ c
    ]
}

/// The order of three fractions, without branches (random inks would mispredict them):
/// `(fa ≥ fb) · 4 + (fb ≥ fc) · 2 + (fa ≥ fc)`.
#[inline(always)]
fn select<T: PartialOrd>(fa: T, fb: T, fc: T) -> usize {
    (usize::from(fa >= fb) << 2) | (usize::from(fb >= fc) << 1) | usize::from(fa >= fc)
}

/// CMYK → 8-bit sRGB.
#[derive(Clone, Debug)]
pub struct CmykConverter {
    kind: Kind,
}

#[derive(Clone, Debug)]
enum Kind {
    Naive,
    Source(Box<Source>),
    Baked(Box<Baked>),
}

/// Zeros after a table, so the 8-lane load at the last cell stays in bounds.
const PAD: usize = 2;

/// A profile's own 4D table and what follows it, converted directly.
#[derive(Clone, Debug)]
struct Source {
    /// The table's values × 65535, three per node, index `((m·N_Y + y)·N_K + k)·N_C + c`,
    /// then [`PAD`] zeros. C is innermost, so the two C slices a pixel blends are adjacent.
    v: Vec<u16>,
    /// Per ink byte, through the input curve: offset of the cell's first node along that
    /// axis (in values) and the fraction into the cell. C, M, Y, K.
    axes: Box<[[(u32, f32); 256]; 4]>,
    /// See [`tetra`] (over M, Y and K).
    tetra: [(u32, u32); 8],
    /// Offset of the cell's far corner (M + Y + K strides).
    s3: u32,
    /// Per table output: its value (×65535) → the PCS term it feeds.
    post: [Term; 3],
    /// The PCS is L*a*b*: the terms are `f(Y)`, `a*/500` and `−b*/200`, so `f(X)`, `f(Y)` and
    /// `f(Z)` are `t1 + t0`, `t0`, `t2 + t0`, and `f⁻¹` of those is XYZ / white. Else the
    /// terms are X, Y and Z.
    lab: bool,
    /// XYZ (or XYZ / white) → linear sRGB, with the PCS adjustment folded in: rows, the
    /// offset last.
    m: [[f32; 4]; 3],
    /// [`sqrt_encode_table`].
    enc: &'static [u8],
}

/// One table output (×65535) → its PCS term.
#[derive(Clone, Debug)]
enum Term {
    /// `v · a + b`: no output curve.
    Affine(f32, f32),
    /// Through an output curve: piecewise linear over [`POST_STEPS`] steps, each step's
    /// start value and its rise.
    Table(Box<[(f32, f32); POST_STEPS]>),
}

/// Steps of the output-curve terms.
const POST_STEPS: usize = 1024;

/// Pixels converted together through the post-table stage.
const BLOCK: usize = 8;
type Fx = Simd<f32, BLOCK>;

/// `v` (within 0..2²²) rounded to the nearest integer, without the saturating conversion
/// (which isn't vectorised): adding 2²³ leaves the integer in the low mantissa bits.
#[inline(always)]
fn round_to_int(v: Fx) -> Simd<u32, BLOCK> {
    (v + Fx::splat(8_388_608.0)).to_bits() & Simd::splat(0x7f_ffff)
}

impl Term {
    #[inline(always)]
    fn eval(&self, v: Fx) -> Fx {
        match self {
            Term::Affine(a, b) => v * Fx::splat(*a) + Fx::splat(*b),
            Term::Table(t) => {
                let p = v * Fx::splat(POST_STEPS as f32 / 65535.0);
                let i =
                    round_to_int(p - Fx::splat(0.5)).simd_min(Simd::splat(POST_STEPS as u32 - 1));
                let f = p - i.cast::<f32>();
                let i = i.to_array();
                let e: [(f32, f32); BLOCK] = std::array::from_fn(|j| t[i[j] as usize % POST_STEPS]);
                Fx::from_array(e.map(|e| e.0)) + Fx::from_array(e.map(|e| e.1)) * f
            }
        }
    }
}

impl Source {
    /// The direct conversion for `src`, when its device side is a 4D table (with input and
    /// output curves, or none) small enough to keep in cache, into a L*a*b* or XYZ PCS.
    fn new(src: &Profile, intent: Intent, t: &Transform) -> Option<Source> {
        if src.model.is_some() {
            return None;
        }
        let lut = src.a2b_for(intent.tag())?;
        let (input, clut, output): (Option<&[Curve]>, &Clut, Option<&[Curve]>) =
            match lut.pipe.stages.as_slice() {
                [Stage::Curves(i), Stage::Clut(t)] => (Some(i), t, None),
                [Stage::Curves(i), Stage::Clut(t), Stage::Curves(o)] => (Some(i), t, Some(o)),
                [Stage::Clut(t)] => (None, t, None),
                [Stage::Clut(t), Stage::Curves(o)] => (None, t, Some(o)),
                _ => return None,
            };
        let g = clut.grid();
        if g.len() != 4
            || clut.outputs() != 3
            || input.is_some_and(|c| c.len() != 4)
            || output.is_some_and(|c| c.len() != 3)
            || g.iter().product::<usize>() > MAX_SOURCE_NODES
        {
            return None;
        }
        let [gc, gm, gy, gk] = [g[0], g[1], g[2], g[3]];
        // Reorder: ICC order has C slowest; here C is innermost.
        let data = clut.data();
        let mut v = Vec::with_capacity(data.len() + PAD);
        for m in 0..gm {
            for y in 0..gy {
                for k in 0..gk {
                    for c in 0..gc {
                        let o = (((c * gm + m) * gy + y) * gk + k) * 3;
                        v.extend(data[o..o + 3].iter().map(|x| (x * 65535.0).round() as u16));
                    }
                }
            }
        }
        v.extend([0; PAD]);
        let (sc, sk) = (3, gc * 3);
        let (sy, sm) = (gk * sk, gy * gk * sk);
        let strides = [sc, sm, sy, sk];
        let axes = Box::new(std::array::from_fn(|d| {
            let top = (g[d] - 1) as f32;
            std::array::from_fn(|b| {
                let x = b as f32 / 255.0;
                let x = input.map_or(x, |c| c[d].eval(x)).clamp(0.0, 1.0) * top;
                let i = (x as usize).min(g[d] - 2);
                ((i * strides[d]) as u32, x - i as f32)
            })
        }));
        // After the table: output curves, then the PCS terms.
        let (enc, lab) = (lut.pcs, lut.pcs != PcsEncoding::Xyz);
        let term = |ch: usize, x: f32| -> f32 {
            let x = output.map_or(x, |c| c[ch].eval(x));
            let (l, ab) = match enc {
                PcsEncoding::LabV2 => (x * (100.0 * 65535.0 / 65280.0), x * (65535.0 / 256.0)),
                PcsEncoding::LabV4 => (x * 100.0, x * 255.0),
                PcsEncoding::Xyz => return x * (65535.0 / 32768.0),
            };
            [
                (l + 16.0) / 116.0,
                (ab - 128.0) / 500.0,
                (128.0 - ab) / 200.0,
            ][ch]
        };
        let post = std::array::from_fn(|ch| {
            if output.is_none_or(|c| c[ch].is_identity()) {
                let b = term(ch, 0.0);
                return Term::Affine((term(ch, 1.0) - b) / 65535.0, b);
            }
            let at: Vec<f32> = (0..=POST_STEPS)
                .map(|i| term(ch, i as f32 / POST_STEPS as f32))
                .collect();
            Term::Table(Box::new(std::array::from_fn(|i| {
                (at[i], at[i + 1] - at[i])
            })))
        });
        // XYZ → linear sRGB with the PCS adjustment folded in: `M · (s ∘ xyz + o)`.
        let to_rgb = profiles::linear_bt709().matrix?.inverse()?;
        let (s, o) = t.pcs_adjust();
        let white = if lab { D50 } else { [1.0; 3] };
        let off = mul3(&to_rgb.to_f32(), o);
        let m = std::array::from_fn(|r| {
            let row = to_rgb.0[r];
            let e = |j: usize| (row[j] * f64::from(s[j]) * f64::from(white[j])) as f32;
            [e(0), e(1), e(2), off[r]]
        });
        Some(Source {
            v,
            axes,
            tetra: tetra(sm as u32, sy as u32, sk as u32),
            s3: (sm + sy + sk) as u32,
            post,
            lab,
            m,
            enc: sqrt_encode_table(),
        })
    }

    /// One pixel of ink amounts → the table's outputs (×65535).
    #[inline(always)]
    fn interpolate(&self, ink: [u8; 4]) -> f32x4 {
        let a = &*self.axes;
        let (oc, fc) = a[0][ink[0] as usize];
        let (om, fm) = a[1][ink[1] as usize];
        let (oy, fy) = a[2][ink[2] as usize];
        let (ok, fk) = a[3][ink[3] as usize];
        let base = (oc + om + oy + ok) as usize;
        let (s1, s2) = self.tetra[select(fm, fy, fk)];
        let w0 = fm.max(fy).max(fk);
        let w2 = fm.min(fy).min(fk);
        let w1 = fm + fy + fk - w0 - w2;
        // Each vertex: both C slices (lanes 0..3 and 3..6) in one load.
        let t = &self.v;
        let ld = |o: usize| -> f32x8 { u16x8::from_slice(&t[o..o + 8]).cast() };
        let c0 = ld(base);
        let c1 = ld(base + s1 as usize);
        let c2 = ld(base + s2 as usize);
        let c3 = ld(base + self.s3 as usize);
        let w = f32x8::splat;
        let v = c0 + (c1 - c0) * w(w0) + (c2 - c1) * w(w1) + (c3 - c2) * w(w2);
        // Linear over C.
        let lo = simd_swizzle!(v, [0, 1, 2, 0]);
        let hi = simd_swizzle!(v, [3, 4, 5, 3]);
        lo + (hi - lo) * f32x4::splat(fc)
    }

    /// Up to [`BLOCK`] pixels (4 bytes each) → RGBA.
    #[inline(always)]
    fn convert_block(&self, ink: &[u8], rgba: &mut [u8]) {
        let mut v = [[0f32; BLOCK]; 3];
        for (j, p) in ink.chunks_exact(4).enumerate() {
            let x = self.interpolate([p[0], p[1], p[2], p[3]]);
            v[0][j] = x[0];
            v[1][j] = x[1];
            v[2][j] = x[2];
        }
        let t = [0, 1, 2].map(|ch| self.post[ch].eval(Fx::from_array(v[ch])));
        let x = if self.lab {
            // f⁻¹ of f(X), f(Y), f(Z).
            const D: f32 = 6.0 / 29.0;
            let finv = |t: Fx| {
                let lin = (t - Fx::splat(4.0 / 29.0)) * Fx::splat(3.0 * D * D);
                t.simd_gt(Fx::splat(D)).select(t * t * t, lin)
            };
            [finv(t[1] + t[0]), finv(t[0]), finv(t[2] + t[0])]
        } else {
            t
        };
        let rgb = self.m.map(|r| {
            let s = Fx::splat;
            let v = x[0] * s(r[0]) + x[1] * s(r[1]) + x[2] * s(r[2]) + s(r[3]);
            round_to_int(v.simd_clamp(s(0.0), s(1.0)).sqrt() * s(ENC_TOP)).to_array()
        });
        for (j, o) in rgba.chunks_exact_mut(4).take(ink.len() / 4).enumerate() {
            let e = |c: usize| self.enc[rgb[c][j] as usize];
            o.copy_from_slice(&[e(0), e(1), e(2), 255]);
        }
    }

    fn convert(&self, ink: [u8; 4]) -> [u8; 3] {
        let mut o = [0u8; 4];
        self.convert_block(&ink, &mut o);
        [o[0], o[1], o[2]]
    }
}

/// A table baked from the exact transform, and its per-byte axis lookups.
#[derive(Clone, Debug)]
struct Baked {
    /// Values on the table curve × [`SCALE`], unclipped, three per node, index
    /// `((c·N_M + m)·N_Y + y)·N_K + k`, then [`PAD`] zeros. K is innermost, so the two K
    /// slices a pixel blends are adjacent.
    v: Vec<i16>,
    /// Per ink byte: offset of the cell's first node along that axis (in `i16`s) and the
    /// fraction into the cell, in 1/256ths (up to 256 at the last node). C, M, Y, K.
    axes: Box<[[(u32, u32); 256]; 4]>,
    /// See [`tetra`] (over C, M and Y).
    tetra: [(u32, u32); 8],
    /// Offset of the cell's far corner (C + M + Y strides).
    s3: u32,
    /// Table curve (rounded table units, from zero) → 8-bit sRGB.
    dec: &'static [u8],
}

impl Baked {
    fn new(t: &Transform) -> Baked {
        let ink = NODES.map(dense_nodes);
        let [nc, nm, ny, nk] = NODES;
        let (sk, sy) = (3, nk * 3);
        let (sm, sc) = (ny * sy, nm * ny * sy);
        let mut v = Vec::with_capacity(nc * nm * ny * nk * 3 + PAD);
        let mut o = [0f32; 3];
        for &c in &ink[0] {
            for &m in &ink[1] {
                for &y in &ink[2] {
                    for &k in &ink[3] {
                        t.eval(&[c, m, y, k], &mut o);
                        v.extend(
                            o.map(|x| (to_table(x) * SCALE).round().clamp(-LIMIT, LIMIT) as i16),
                        );
                    }
                }
            }
        }
        v.extend([0; PAD]);
        let strides = [sc, sm, sy, sk];
        let axes = Box::new(std::array::from_fn(|d| {
            let (ink, n) = (&ink[d], NODES[d]);
            std::array::from_fn(|b| {
                let x = b as f32 / 255.0;
                let i = (1..n - 1).take_while(|&i| ink[i] <= x).count();
                let f = ((x - ink[i]) / (ink[i + 1] - ink[i]) * 256.0).round() as u32;
                ((i * strides[d]) as u32, f.min(256))
            })
        }));
        Baked {
            v,
            axes,
            tetra: tetra(sc as u32, sm as u32, sy as u32),
            s3: (sc + sm + sy) as u32,
            dec: decode_table(),
        }
    }

    /// One pixel of ink amounts → sRGB.
    #[inline(always)]
    fn convert(&self, ink: [u8; 4]) -> [u8; 3] {
        let a = &*self.axes;
        let (oc, fc) = a[0][ink[0] as usize];
        let (om, fm) = a[1][ink[1] as usize];
        let (oy, fy) = a[2][ink[2] as usize];
        let (ok, fk) = a[3][ink[3] as usize];
        let base = (oc + om + oy + ok) as usize;
        // Tetrahedron over (c, m, y), weights in 1/256ths.
        let (s1, s2) = self.tetra[select(fc, fm, fy)];
        let w0 = fc.max(fm).max(fy);
        let w2 = fc.min(fm).min(fy);
        let w1 = fc + fm + fy - w0 - w2;
        // Each vertex: both K slices (lanes 0..3 and 3..6) in one load.
        let t = &self.v;
        let ld = |o: usize| -> i32x8 { i16x8::from_slice(&t[o..o + 8]).cast() };
        let c0 = ld(base);
        let c1 = ld(base + s1 as usize);
        let c2 = ld(base + s2 as usize);
        let c3 = ld(base + self.s3 as usize);
        let w = |x: u32| i32x8::splat(x as i32);
        let v = c0 * w(256 - w0) + c1 * w(w0 - w1) + c2 * w(w1 - w2) + c3 * w(w2);
        // Linear over K.
        let lo = simd_swizzle!(v, [0, 1, 2, 0]);
        let hi = simd_swizzle!(v, [3, 4, 5, 3]);
        let v = lo + (((hi - lo) * i32x4::splat(fk as i32)) >> 8);
        // 8 fractional bits from the weights.
        let top = i32x4::splat(self.dec.len() as i32 - 1);
        let i = ((v + i32x4::splat(128)) >> 8).simd_clamp(i32x4::splat(0), top);
        let i = i.to_array();
        [
            self.dec[i[0] as usize],
            self.dec[i[1] as usize],
            self.dec[i[2] as usize],
        ]
    }
}

impl CmykConverter {
    /// The naive conversion (`R = (1 − C)(1 − K)` …), used when colour management is off.
    pub fn naive() -> CmykConverter {
        CmykConverter { kind: Kind::Naive }
    }

    /// The conversion for a CMYK profile (relative to sRGB output).
    pub fn new(src: &Profile, intent: Intent, bpc: bool) -> Result<CmykConverter, Error> {
        if src.color_space != ColorSpace::Cmyk {
            return Err(Error::Unsupported("not a CMYK profile"));
        }
        // To unclipped linear BT.709 (sRGB primaries).
        let t = Transform::new(src, &profiles::linear_bt709(), intent, bpc)?;
        let kind = match Source::new(src, intent, &t) {
            Some(s) => Kind::Source(Box::new(s)),
            None => Kind::Baked(Box::new(Baked::new(&t))),
        };
        Ok(CmykConverter { kind })
    }

    /// The generic (built-in) CMYK conversion.
    pub fn generic(intent: Intent, bpc: bool) -> CmykConverter {
        CmykConverter::new(&profiles::generic_cmyk(), intent, bpc)
            .unwrap_or_else(|_| CmykConverter::naive())
    }

    /// True for the naive conversion.
    pub fn is_naive(&self) -> bool {
        matches!(self.kind, Kind::Naive)
    }

    /// Converts one pixel of ink amounts (0 = none, 255 = solid) to sRGB.
    #[inline]
    pub fn convert(&self, ink: [u8; 4]) -> [u8; 3] {
        match &self.kind {
            Kind::Source(t) => t.convert(ink),
            Kind::Baked(t) => t.convert(ink),
            Kind::Naive => naive(ink),
        }
    }

    /// Converts a row: `ink` holds 4 bytes per pixel, `rgba` receives 4 bytes per pixel with
    /// alpha 255.
    pub fn convert_row(&self, ink: &[u8], rgba: &mut [u8]) {
        fn run(ink: &[u8], rgba: &mut [u8], f: impl Fn([u8; 4]) -> [u8; 3]) {
            for (i, o) in ink.chunks_exact(4).zip(rgba.chunks_exact_mut(4)) {
                let [r, g, b] = f([i[0], i[1], i[2], i[3]]);
                o.copy_from_slice(&[r, g, b, 255]);
            }
        }
        match &self.kind {
            Kind::Source(t) => {
                for (i, o) in ink.chunks(4 * BLOCK).zip(rgba.chunks_mut(4 * BLOCK)) {
                    t.convert_block(i, o);
                }
            }
            Kind::Baked(t) => run(ink, rgba, |p| t.convert(p)),
            Kind::Naive => run(ink, rgba, naive),
        }
    }
}

/// `R = (1 − C)(1 − K)` …
#[inline]
fn naive(ink: [u8; 4]) -> [u8; 3] {
    let k = 255 - ink[3] as u32;
    let f = |v: u8| (((255 - v as u32) * k + 127) / 255) as u8;
    [f(ink[0]), f(ink[1]), f(ink[2])]
}

/// How L*a*b* samples are stored, for [`LabConverter::convert_samples`]: channel `c` of a
/// pixel is `sample · scale[c] + offset[c]` (L 0..100, a/b about −128..127), the sample read
/// as an unsigned or (`signed[c]`) two's-complement integer of 8 or (`wide`) 16 bits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabCoding {
    pub wide: bool,
    /// Byte order of 16-bit samples.
    pub big_endian: bool,
    pub signed: [bool; 3],
    pub scale: [f32; 3],
    pub offset: [f32; 3],
}

impl LabCoding {
    /// TIFF CIELab: L 0..max ↦ 0..100, a/b signed (16-bit: in 1/256 units).
    pub fn cielab(wide: bool, big_endian: bool) -> LabCoding {
        let (max, ab) = if wide {
            (65535.0, 1.0 / 256.0)
        } else {
            (255.0, 1.0)
        };
        LabCoding {
            wide,
            big_endian,
            signed: [false, true, true],
            scale: [100.0 / max, ab, ab],
            offset: [0.0; 3],
        }
    }

    /// ICC (v4) encoding, as PSD and TIFF ICCLab: L 0..max ↦ 0..100, a/b offset by half the
    /// range (16-bit: ÷ 257 − 128).
    pub fn icclab(wide: bool, big_endian: bool) -> LabCoding {
        let (max, ab) = if wide {
            (65535.0, 1.0 / 257.0)
        } else {
            (255.0, 1.0)
        };
        LabCoding {
            wide,
            big_endian,
            signed: [false; 3],
            scale: [100.0 / max, ab, ab],
            offset: [0.0, -128.0, -128.0],
        }
    }

    /// Linear ranges: sample 0..max ↦ `range[c].0..range[c].1` (TIFF ITULab and its Decode
    /// tag).
    pub fn ranges(wide: bool, big_endian: bool, range: [(f32, f32); 3]) -> LabCoding {
        let max = if wide { 65535.0 } else { 255.0 };
        LabCoding {
            wide,
            big_endian,
            signed: [false; 3],
            scale: range.map(|(lo, hi)| (hi - lo) / max),
            offset: range.map(|(lo, _)| lo),
        }
    }
}

/// L*a*b* (D50) → 8-bit sRGB (media-relative colorimetric, Bradford-adapted; out-of-gamut
/// colours are clipped). Eight pixels at a time in f32 SIMD: f⁻¹, then the PCS → linear
/// sRGB matrix (D50 white folded in), then the square-root encode table CMYK uses.
#[derive(Clone, Debug)]
pub struct LabConverter {
    /// Linear sRGB from f⁻¹(fx), f⁻¹(fy), f⁻¹(fz), rows.
    m: [[f32; 3]; 3],
    enc: &'static [u8],
}

impl Default for LabConverter {
    fn default() -> Self {
        let inv = bt709_to_pcs().inverse().unwrap_or(Mat3::IDENTITY).to_f32();
        LabConverter {
            m: std::array::from_fn(|r| std::array::from_fn(|c| inv[r * 3 + c] * D50[c])),
            enc: sqrt_encode_table(),
        }
    }
}

impl LabConverter {
    /// Converts one colour (L 0..100, a/b −128..127).
    #[inline]
    pub fn convert(&self, lab: [f32; 3]) -> [u8; 3] {
        let mut o = [0u8; 4];
        let at = |c: usize| Fx::splat(lab[c]);
        self.convert_block([at(0), at(1), at(2)], &mut o);
        [o[0], o[1], o[2]]
    }

    /// Planar 8-bit rows (L 0..255 ↦ 0..100, a/b offset by 128, as in PSD and ICC Lab) →
    /// RGBA, alpha 255. The rows may differ in length: the shortest one is converted.
    pub fn convert_planes8(&self, l: &[u8], a: &[u8], b: &[u8], rgba: &mut [u8]) {
        self.planes([l, a, b], 100.0 / 255.0, 1.0, rgba);
    }

    /// Planar 16-bit rows (L 0..65535 ↦ 0..100, a/b ÷ 257 offset by 128) → RGBA, alpha 255.
    pub fn convert_planes16(&self, l: &[u16], a: &[u16], b: &[u16], rgba: &mut [u8]) {
        self.planes([l, a, b], 100.0 / 65535.0, 1.0 / 257.0, rgba);
    }

    /// Strided samples of any of the usual encodings (TIFF CIELab / ICCLab / ITULab, chunky
    /// or planar, with or without extra samples) → RGBA, alpha 255, one pixel per 4 bytes of
    /// `rgba`. `channels[c]` starts at the first pixel's sample `c`; consecutive pixels are
    /// `step` bytes apart in each, so chunky rows need no copy. Channels too short for
    /// `rgba` shorten the conversion (to whole pixels).
    pub fn convert_samples(
        &self,
        channels: [&[u8]; 3],
        step: usize,
        coding: &LabCoding,
        rgba: &mut [u8],
    ) {
        let bytes = if coding.wide { 2 } else { 1 };
        let step = step.max(1);
        let fits = |c: &[u8]| {
            if c.len() < bytes {
                0
            } else {
                (c.len() - bytes) / step + 1
            }
        };
        let n = channels
            .iter()
            .map(|c| fits(c))
            .min()
            .unwrap_or(0)
            .min(rgba.len() / 4);
        // A signed sample is its two's complement flipped to offset binary, with the offset
        // folded into the additive term.
        let half = if coding.wide { 32768.0 } else { 128.0 };
        let xor: [u32; 3] = std::array::from_fn(|c| if coding.signed[c] { half as u32 } else { 0 });
        let add: [f32; 3] = std::array::from_fn(|c| {
            coding.offset[c]
                - if coding.signed[c] {
                    half * coding.scale[c]
                } else {
                    0.0
                }
        });
        let rgba = &mut rgba[..n * 4];
        match (coding.wide, coding.big_endian) {
            (false, _) => self.strided(channels, step, n, xor, coding.scale, add, rgba, |s, i| {
                s[i] as u32
            }),
            (true, false) => {
                self.strided(channels, step, n, xor, coding.scale, add, rgba, |s, i| {
                    u16::from_le_bytes([s[i], s[i + 1]]) as u32
                })
            }
            (true, true) => {
                self.strided(channels, step, n, xor, coding.scale, add, rgba, |s, i| {
                    u16::from_be_bytes([s[i], s[i + 1]]) as u32
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    fn strided(
        &self,
        ch: [&[u8]; 3],
        step: usize,
        n: usize,
        xor: [u32; 3],
        scale: [f32; 3],
        add: [f32; 3],
        rgba: &mut [u8],
        get: impl Fn(&[u8], usize) -> u32,
    ) {
        for (i, o) in (0..n).step_by(BLOCK).zip(rgba.chunks_mut(BLOCK * 4)) {
            let k = (n - i).min(BLOCK);
            let load = |c: usize| {
                let mut v = [0f32; BLOCK];
                for (j, d) in v[..k].iter_mut().enumerate() {
                    *d = (get(ch[c], (i + j) * step) ^ xor[c]) as f32;
                }
                Fx::from_array(v) * Fx::splat(scale[c]) + Fx::splat(add[c])
            };
            self.convert_block([load(0), load(1), load(2)], o);
        }
    }

    #[inline(always)]
    fn planes<T: Copy + Into<f32>>(&self, p: [&[T]; 3], ls: f32, cs: f32, rgba: &mut [u8]) {
        let n = p
            .iter()
            .map(|p| p.len())
            .min()
            .unwrap_or(0)
            .min(rgba.len() / 4);
        for (i, o) in (0..n)
            .step_by(BLOCK)
            .zip(rgba[..n * 4].chunks_mut(BLOCK * 4))
        {
            let k = (n - i).min(BLOCK);
            let load = |c: usize, s: f32, off: f32| {
                let mut v = [0f32; BLOCK];
                for (d, &x) in v.iter_mut().zip(&p[c][i..i + k]) {
                    *d = x.into() * s - off;
                }
                Fx::from_array(v)
            };
            self.convert_block(
                [load(0, ls, 0.0), load(1, cs, 128.0), load(2, cs, 128.0)],
                o,
            );
        }
    }

    /// Up to [`BLOCK`] colours (L, a, b lanes) → RGBA (`rgba.len() / 4` of them).
    #[inline(always)]
    fn convert_block(&self, lab: [Fx; 3], rgba: &mut [u8]) {
        let s = Fx::splat;
        let fy = (lab[0] + s(16.0)) * s(1.0 / 116.0);
        let fx = fy + lab[1] * s(1.0 / 500.0);
        let fz = fy - lab[2] * s(1.0 / 200.0);
        const D: f32 = 6.0 / 29.0;
        let finv = |t: Fx| {
            let lin = (t - s(4.0 / 29.0)) * s(3.0 * D * D);
            t.simd_gt(s(D)).select(t * t * t, lin)
        };
        let x = [finv(fx), finv(fy), finv(fz)];
        let rgb = self.m.map(|r| {
            let v = x[0] * s(r[0]) + x[1] * s(r[1]) + x[2] * s(r[2]);
            // The bound keeps a NaN (from `convert`'s floats) inside the table.
            round_to_int(v.simd_clamp(s(0.0), s(1.0)).sqrt() * s(ENC_TOP))
                .simd_min(Simd::splat(ENC_TOP as u32))
                .to_array()
        });
        for (j, o) in rgba.chunks_exact_mut(4).enumerate() {
            let e = |c: usize| self.enc[rgb[c][j] as usize];
            o.copy_from_slice(&[e(0), e(1), e(2), 255]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naive_matches_formula() {
        let c = CmykConverter::naive();
        assert_eq!(c.convert([0, 0, 0, 0]), [255, 255, 255]);
        assert_eq!(c.convert([255, 0, 0, 0]), [0, 255, 255]);
        assert_eq!(c.convert([0, 0, 0, 255]), [0, 0, 0]);
    }

    #[test]
    fn table_matches_direct_transform() {
        let p = std::env::var("CMYK_ICC")
            .ok()
            .and_then(|f| Profile::parse(&std::fs::read(f).ok()?).ok())
            .unwrap_or_else(profiles::generic_cmyk);
        let conv = CmykConverter::new(&p, Intent::Relative, true).unwrap();
        let t = Transform::new(&p, &profiles::srgb(), Intent::Relative, true).unwrap();
        let mut worst = 0f32;
        let mut sum_de = 0f32;
        let mut max_de = 0f32;
        let mut o = [0f32; 3];
        let mut seed = 1u32;
        for _ in 0..20000 {
            let mut ink = [0u8; 4];
            for v in &mut ink {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *v = (seed >> 24) as u8;
            }
            let got = conv.convert(ink);
            t.eval(&ink.map(|v| v as f32 / 255.0), &mut o);
            let want = o.map(|v| v.clamp(0.0, 1.0) * 255.0);
            for c in 0..3 {
                worst = worst.max((got[c] as f32 - want[c]).abs());
            }
            let lab = |v: [f32; 3]| {
                let l = v.map(|x| crate::math::srgb_to_linear(x / 255.0));
                crate::math::xyz_to_lab(crate::math::mul3(&bt709_to_pcs().to_f32(), l))
            };
            let de = crate::math::delta_e2000(lab(got.map(|v| v as f32)), lab(want));
            sum_de += de;
            max_de = max_de.max(de);
        }
        let mean = sum_de / 20000.0;
        eprintln!("CMYK table: worst 8-bit {worst}, mean ΔE00 {mean}, max ΔE00 {max_de}");
        // Near-solid ink is checked more closely (to one code) in tests/cmyk_table.rs; here
        // random inks over the whole cube, mostly mid-range. The 8-bit worst case (about 2
        // codes) sits in deep shadows, where an 8-bit code is a small colour difference.
        assert!(worst <= 3.0, "worst 8-bit {worst}");
        assert!(max_de < 1.5 && mean < 0.3, "ΔE mean {mean} max {max_de}");
    }

    /// A profile whose `A2B` is input curves → a 9⁴ table of the generic model → output
    /// curves (each optional), into `pcs`.
    fn table_profile(pcs: PcsEncoding, input: bool, output: bool) -> Profile {
        use crate::icc::Lut;
        use crate::lut::{Clut, Pipeline};
        let g = 9;
        let mut data = Vec::new();
        for i in 0..g * g * g * g {
            let ink = [i / (g * g * g), i / (g * g) % g, i / g % g, i % g]
                .map(|n| n as f32 / (g - 1) as f32);
            let e = pcs.encode(profiles::generic_cmyk_xyz(ink));
            // Through the inverse of the output curves below, so the profile stays plausible.
            let e = if output {
                [e[0].powf(1.0 / 0.8), e[1], e[2].powf(1.0 / 1.1)]
            } else {
                e
            };
            // 16-bit, as stored in a profile.
            data.extend(e.map(|v| (v.clamp(0.0, 1.0) * 65535.0).round() / 65535.0));
        }
        let mut stages = Vec::new();
        if input {
            stages.push(Stage::Curves(
                [1.4, 1.2, 1.0, 0.7].map(Curve::Gamma).to_vec(),
            ));
        }
        stages.push(Stage::Clut(Clut::new(vec![g; 4], 3, data).unwrap()));
        if output {
            stages.push(Stage::Curves([0.8, 1.0, 1.1].map(Curve::Gamma).to_vec()));
        }
        let lut = Lut {
            pipe: Pipeline {
                inputs: 4,
                outputs: 3,
                stages,
            },
            pcs,
        };
        let mut p = profiles::generic_cmyk();
        p.model = None;
        p.a2b = [Some(lut.clone()), Some(lut.clone()), Some(lut)];
        p
    }

    #[test]
    fn source_table_matches_direct_transform() {
        for (pcs, input, output) in [
            (PcsEncoding::LabV2, true, true),
            (PcsEncoding::LabV4, false, true),
            (PcsEncoding::Xyz, true, false),
            (PcsEncoding::Xyz, false, false),
        ] {
            let p = table_profile(pcs, input, output);
            for (intent, bpc) in [
                (Intent::Relative, true),
                (Intent::Relative, false),
                (Intent::Perceptual, true),
                (Intent::Absolute, false),
            ] {
                let conv = CmykConverter::new(&p, intent, bpc).unwrap();
                assert!(matches!(conv.kind, Kind::Source(_)));
                let t = Transform::new(&p, &profiles::srgb(), intent, bpc).unwrap();
                let mut o = [0f32; 3];
                let mut worst = 0f32;
                let mut seed = 7u32;
                let mut ink = vec![0u8; 4 * 4099];
                for v in &mut ink {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    *v = (seed >> 24) as u8;
                }
                ink[..8].copy_from_slice(&[0, 0, 0, 0, 255, 255, 255, 255]);
                // Rows of odd lengths: the last block is partial.
                let mut rgba = vec![0u8; ink.len()];
                conv.convert_row(&ink, &mut rgba);
                for (i, px) in ink.chunks_exact(4).enumerate() {
                    let px = [px[0], px[1], px[2], px[3]];
                    let got = conv.convert(px);
                    assert_eq!(got[..], rgba[i * 4..i * 4 + 3], "{px:?}");
                    assert_eq!(rgba[i * 4 + 3], 255);
                    t.eval(&px.map(|v| v as f32 / 255.0), &mut o);
                    for c in 0..3 {
                        worst = worst.max((got[c] as f32 - o[c].clamp(0.0, 1.0) * 255.0).abs());
                    }
                }
                assert!(
                    worst < 0.56,
                    "{pcs:?} {input} {output} {intent:?} {bpc}: {worst}"
                );
            }
        }
    }

    /// Chunky (with an extra sample) and planar strided samples of every coding and byte
    /// order give the colours of their decoded L*a*b* values.
    #[test]
    fn lab_samples_strided() {
        let c = LabConverter::default();
        let n = 1003;
        let mut seed = 11u32;
        let raw: Vec<u8> = (0..n * 4 * 2)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 24) as u8
            })
            .collect();
        for wide in [false, true] {
            for be in [false, true] {
                let b = if wide { 2 } else { 1 };
                for coding in [
                    LabCoding::cielab(wide, be),
                    LabCoding::icclab(wide, be),
                    LabCoding::ranges(wide, be, [(0.0, 100.0), (-85.0, 85.0), (-75.0, 125.0)]),
                ] {
                    let value = |s: &[u8], ch: usize| {
                        let u = match (wide, be) {
                            (false, _) => s[0] as i32,
                            (true, false) => u16::from_le_bytes([s[0], s[1]]) as i32,
                            (true, true) => u16::from_be_bytes([s[0], s[1]]) as i32,
                        };
                        let half = if wide { 32768 } else { 128 };
                        let v = if coding.signed[ch] && u >= half {
                            u - 2 * half
                        } else {
                            u
                        };
                        v as f32 * coding.scale[ch] + coding.offset[ch]
                    };
                    // Chunky, 4 samples per pixel (the 4th ignored).
                    let mut got = vec![0u8; n * 4];
                    let st = 4 * b;
                    c.convert_samples([&raw[..], &raw[b..], &raw[2 * b..]], st, &coding, &mut got);
                    // Planar: the first three quarters of `raw` as planes.
                    let pl = n * b;
                    let mut got_p = vec![0u8; n * 4];
                    c.convert_samples(
                        [&raw[..pl], &raw[pl..2 * pl], &raw[2 * pl..3 * pl]],
                        b,
                        &coding,
                        &mut got_p,
                    );
                    for i in 0..n {
                        let px = &raw[i * st..];
                        let want =
                            c.convert([value(px, 0), value(&px[b..], 1), value(&px[2 * b..], 2)]);
                        let ctx = format!("{coding:?} pixel {i}");
                        for k in 0..3 {
                            assert!(got[i * 4 + k].abs_diff(want[k]) <= 1, "{ctx}");
                        }
                        assert_eq!(got[i * 4 + 3], 255);
                        let want = c.convert([
                            value(&raw[i * b..], 0),
                            value(&raw[pl + i * b..], 1),
                            value(&raw[2 * pl + i * b..], 2),
                        ]);
                        for k in 0..3 {
                            assert!(got_p[i * 4 + k].abs_diff(want[k]) <= 1, "planar {ctx}");
                        }
                    }
                    // Short channels shorten the conversion to whole pixels.
                    let mut short = vec![7u8; 16];
                    c.convert_samples([&raw[..2 * b], &raw[..], &raw[..]], b, &coding, &mut short);
                    assert_eq!(&short[8..], &[7; 8]);
                }
            }
        }
    }

    #[test]
    fn lab_white_and_black() {
        let c = LabConverter::default();
        assert_eq!(c.convert([100.0, 0.0, 0.0]), [255, 255, 255]);
        assert_eq!(c.convert([0.0, 0.0, 0.0]), [0, 0, 0]);
        let mid = c.convert([50.0, 0.0, 0.0]);
        assert!(mid.iter().all(|v| (*v as i32 - 119).abs() <= 1), "{mid:?}");
    }
}
