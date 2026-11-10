//! Explicit weighted prediction (8.4.2.3): the weight tables written into P and B slice
//! headers, their application to motion-compensated predictions, and their estimation from
//! the source and reference pictures.
//!
//! * **Fades** (to and from black or a colour) are a P picture with one weight and offset
//!   per plane: `cur ≈ w·prev + o`.
//! * **Cross-fades** between two pictures are a B picture predicting from two *past*
//!   pictures — the previous one and an *anchor* kept as a long-term reference (the last
//!   picture before the fade began) — with explicit bi-predictive weights: if
//!   `cur = (1 − aₜ)·A + aₜ·B` and `prev = (1 − aₜ₋₁)·A + aₜ₋₁·B`, then
//!   `cur = α·prev + β·A` exactly, with `α = aₜ / aₜ₋₁` and `β = 1 − aₜ − α·(1 − aₜ₋₁)`.
//!   A least-squares fit finds α, β (and an offset) per plane without knowing the blend.

/// One explicit weight and offset (8-bit samples).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wp {
    pub w: i32,
    pub o: i32,
}

/// A pred_weight_table: per list (L0, L1) and plane (Y, Cb, Cr).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weights {
    pub luma_log2: u32,
    pub chroma_log2: u32,
    pub wp: [[Wp; 3]; 2],
}

#[inline]
fn clip8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

impl Weights {
    /// The default (flags off) table.
    pub fn identity(luma_log2: u32, chroma_log2: u32) -> Weights {
        let l = Wp {
            w: 1 << luma_log2,
            o: 0,
        };
        let c = Wp {
            w: 1 << chroma_log2,
            o: 0,
        };
        Weights {
            luma_log2,
            chroma_log2,
            wp: [[l, c, c]; 2],
        }
    }

    #[inline]
    pub fn log2(&self, plane: usize) -> u32 {
        if plane == 0 {
            self.luma_log2
        } else {
            self.chroma_log2
        }
    }

    /// Whether entry (list, plane) is the default (its flag is written as 0).
    #[inline]
    pub fn is_default(&self, list: usize, plane: usize) -> bool {
        let p = self.wp[list][plane];
        p.w == 1 << self.log2(plane) && p.o == 0
    }

    /// Whether single-list prediction from `list` leaves samples unchanged.
    #[inline]
    pub fn list_is_default(&self, list: usize) -> bool {
        (0..3).all(|p| self.is_default(list, p))
    }

    /// Single-list weighted prediction in place (8-15).
    pub fn apply1(&self, list: usize, plane: usize, s: &mut [u8]) {
        if self.is_default(list, plane) {
            return;
        }
        let Wp { w, o } = self.wp[list][plane];
        let l = self.log2(plane);
        if l >= 1 {
            let r = 1 << (l - 1);
            for v in s {
                *v = clip8(((i32::from(*v) * w + r) >> l) + o);
            }
        } else {
            for v in s {
                *v = clip8(i32::from(*v) * w + o);
            }
        }
    }

    /// Bi-predictive weighted prediction (8-16) of `p0` (L0) and `p1` (L1) into `p0`.
    pub fn apply2(&self, plane: usize, p0: &mut [u8], p1: &[u8]) {
        let (a, b) = (self.wp[0][plane], self.wp[1][plane]);
        let l = self.log2(plane);
        let o = (a.o + b.o + 1) >> 1;
        let r = 1 << l;
        for (x, &y) in p0.iter_mut().zip(p1) {
            *x = clip8(((i32::from(*x) * a.w + i32::from(y) * b.w + r) >> (l + 1)) + o);
        }
    }
}

/// Default bi-prediction (8-13), for streams without weighted prediction.
pub fn average(p0: &mut [u8], p1: &[u8]) {
    for (x, &y) in p0.iter_mut().zip(p1) {
        *x = ((u16::from(*x) + u16::from(y) + 1) >> 1) as u8;
    }
}

/// A plane reduced by 4×4 box averaging (cheap global statistics that tolerate noise and
/// slow motion).
pub struct Thumb {
    pub w: usize,
    pub h: usize,
    pub px: Vec<f32>,
}

impl Thumb {
    /// Reduces the `w`×`h` plane at `p` (stride `stride`).
    pub fn new(p: &[u8], stride: usize, w: usize, h: usize) -> Thumb {
        let (tw, th) = (w / 4, h / 4);
        let mut px = vec![0f32; tw * th];
        for ty in 0..th {
            for tx in 0..tw {
                let mut s = 0u32;
                for y in 0..4 {
                    let o = (ty * 4 + y) * stride + tx * 4;
                    s += p[o..o + 4].iter().map(|&v| u32::from(v)).sum::<u32>();
                }
                px[ty * tw + tx] = s as f32 / 16.0;
            }
        }
        Thumb { w: tw, h: th, px }
    }

    /// A further `k`×`k` reduction (coarser statistics that tolerate more motion).
    pub fn reduce(&self, k: usize) -> Thumb {
        let (tw, th) = (self.w / k, self.h / k);
        let mut px = vec![0f32; tw * th];
        for ty in 0..th {
            for tx in 0..tw {
                let mut s = 0f32;
                for y in 0..k {
                    let o = (ty * k + y) * self.w + tx * k;
                    s += self.px[o..o + k].iter().sum::<f32>();
                }
                px[ty * tw + tx] = s / (k * k) as f32;
            }
        }
        Thumb { w: tw, h: th, px }
    }
}

/// Sum of squared differences of two thumbnails after mapping `b` by `f`.
fn sse(a: &Thumb, f: impl Fn(usize) -> f32) -> f64 {
    a.px.iter()
        .enumerate()
        .map(|(i, &v)| f64::from(v - f(i)).powi(2))
        .sum()
}

/// The fitted single-reference weight (w, o) of one plane: `cur ≈ w·r + o`, from the
/// ratio of standard deviations (insensitive to motion, unlike a regression slope) and the
/// means.
fn fit1(cur: &Thumb, r: &Thumb) -> (f64, f64) {
    let n = cur.px.len().max(1) as f64;
    let mean = |t: &Thumb| t.px.iter().map(|&v| f64::from(v)).sum::<f64>() / n;
    let (mc, mr) = (mean(cur), mean(r));
    let var = |t: &Thumb, m: f64| {
        t.px.iter()
            .map(|&v| (f64::from(v) - m).powi(2))
            .sum::<f64>()
    };
    let (vc, vr) = (var(cur, mc), var(r, mr));
    let w = if vr > n { (vc / vr).sqrt() } else { 1.0 };
    (w, mc - w * mr)
}

/// Least-squares `cur ≈ α·p + β·a + o` of one plane; `None` when p and a are (nearly)
/// collinear.
fn fit2(cur: &Thumb, p: &Thumb, a: &Thumb) -> Option<(f64, f64, f64)> {
    let n = cur.px.len() as f64;
    let (mut sp, mut sa, mut sc) = (0f64, 0f64, 0f64);
    let (mut spp, mut saa, mut spa, mut spc, mut sac) = (0f64, 0f64, 0f64, 0f64, 0f64);
    for i in 0..cur.px.len() {
        let (x, y, z) = (f64::from(p.px[i]), f64::from(a.px[i]), f64::from(cur.px[i]));
        sp += x;
        sa += y;
        sc += z;
        spp += x * x;
        saa += y * y;
        spa += x * y;
        spc += x * z;
        sac += y * z;
    }
    // Centred normal equations for α, β; o from the means.
    let cpp = spp - sp * sp / n;
    let caa = saa - sa * sa / n;
    let cpa = spa - sp * sa / n;
    let cpc = spc - sp * sc / n;
    let cac = sac - sa * sc / n;
    let det = cpp * caa - cpa * cpa;
    if det <= 1e-3 * cpp * caa || cpp <= n || caa <= n {
        return None;
    }
    let al = (cpc * caa - cac * cpa) / det;
    let be = (cac * cpp - cpc * cpa) / det;
    let o = (sc - al * sp - be * sa) / n;
    Some((al, be, o))
}

/// Quantised single-list weights for a P picture, with their gain: `None` when weighting
/// does not clearly beat plain prediction.
pub fn estimate_p(cur: &[Thumb; 3], prev: &[Thumb; 3]) -> Option<Weights> {
    let (ll, cl) = (6u32, 6u32);
    let mut t = Weights::identity(ll, cl);
    let mut any = false;
    let mut plain = 0.0;
    let mut weighted = 0.0;
    for k in 0..3 {
        let (w, o) = fit1(&cur[k], &prev[k]);
        let l = t.log2(k);
        let wq = ((w * f64::from(1 << l)).round() as i32).clamp(-128, 127);
        let oq = (o.round() as i32).clamp(-128, 127);
        let wp = Wp { w: wq, o: oq };
        let (wf, of) = (wq as f32 / (1 << l) as f32, oq as f32);
        let e_plain = sse(&cur[k], |i| prev[k].px[i]);
        let e_w = sse(&cur[k], |i| prev[k].px[i] * wf + of);
        plain += e_plain;
        weighted += e_w;
        if wp != t.wp[0][k] && e_w < e_plain {
            t.wp[0][k] = wp;
            any = true;
        }
    }
    (any && weighted < 0.8 * plain).then_some(t)
}

/// Explicit bi-predictive weights for a cross-fade from `anchor` through `prev`: `None` unless
/// they predict clearly better than single-reference prediction from `prev`. When a picture
/// moves (a cross-fade into a Ken Burns picture), only coarser statistics see the blend.
pub fn estimate_b(cur: &[Thumb; 3], prev: &[Thumb; 3], anchor: &[Thumb; 3]) -> Option<Weights> {
    estimate_b_at(cur, prev, anchor).or_else(|| {
        let r = |t: &[Thumb; 3]| [t[0].reduce(4), t[1].reduce(4), t[2].reduce(4)];
        if r(cur)[1].px.len() < 16 {
            return None;
        }
        estimate_b_at(&r(cur), &r(prev), &r(anchor))
    })
}

fn estimate_b_at(cur: &[Thumb; 3], prev: &[Thumb; 3], anchor: &[Thumb; 3]) -> Option<Weights> {
    let mut fits = [(0f64, 0f64, 0f64); 3];
    for k in 0..3 {
        fits[k] = match fit2(&cur[k], &prev[k], &anchor[k]) {
            Some(f) => f,
            // Flat chroma (grey pictures): same blend as luma, no offset.
            None if k > 0 => (fits[0].0, fits[0].1, 0.0),
            None => return None,
        };
    }
    // The largest denominator whose weights are representable: |w| ≤ 127 and
    // −128 ≤ w0 + w1 ≤ 128 (127 at denominator 7).
    let quant = |l: u32, f: (f64, f64, f64)| -> Option<(Wp, Wp)> {
        let s = f64::from(1u32 << (l + 1));
        let w0 = (f.0 * s).round() as i32;
        let w1 = (f.1 * s).round() as i32;
        let hi = if l == 7 { 127 } else { 128 };
        let o = (f.2.round() as i32).clamp(-128, 127);
        ((-128..=127).contains(&w0)
            && (-128..=127).contains(&w1)
            && (-128..=hi).contains(&(w0 + w1)))
        .then_some((Wp { w: w0, o }, Wp { w: w1, o }))
    };
    let pick = |fs: &[(f64, f64, f64)]| {
        (0..=7u32)
            .rev()
            .find(|&l| fs.iter().all(|&f| quant(l, f).is_some()))
    };
    let ll = pick(&fits[..1])?;
    let cl = pick(&fits[1..])?;
    let mut t = Weights::identity(ll, cl);
    let mut e_b = 0.0;
    let mut e_p = 0.0;
    for k in 0..3 {
        let l = t.log2(k);
        let (a, b) = quant(l, fits[k])?;
        t.wp[0][k] = a;
        t.wp[1][k] = b;
        let s = (1u32 << (l + 1)) as f32;
        let (fa, fb, fo) = (a.w as f32 / s, b.w as f32 / s, a.o as f32);
        e_b += sse(&cur[k], |i| prev[k].px[i] * fa + anchor[k].px[i] * fb + fo);
        let (w, o) = fit1(&cur[k], &prev[k]);
        e_p += sse(&cur[k], |i| prev[k].px[i] * w as f32 + o as f32)
            .min(sse(&cur[k], |i| prev[k].px[i]));
    }
    (e_b < 0.5 * e_p).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_weights_leave_samples() {
        let t = Weights::identity(5, 5);
        let mut s = [0u8, 1, 127, 128, 255];
        t.apply1(0, 0, &mut s);
        assert_eq!(s, [0, 1, 127, 128, 255]);
        let mut a = [10u8, 11, 255];
        t.apply2(0, &mut a, &[11, 11, 0]);
        let mut b = [10u8, 11, 255];
        average(&mut b, &[11, 11, 0]);
        assert_eq!(a, b);
    }

    #[test]
    fn crossfade_fit_is_exact() {
        let (w, h) = (64usize, 64usize);
        let pa: Vec<u8> = (0..w * h).map(|i| ((i * 37) % 200) as u8 + 20).collect();
        let pb: Vec<u8> = (0..w * h)
            .map(|i| ((i * 91 + 13) % 180) as u8 + 30)
            .collect();
        let blend = |a: f32| -> Vec<u8> {
            pa.iter()
                .zip(&pb)
                .map(|(&x, &y)| (f32::from(x) * (1.0 - a) + f32::from(y) * a).round() as u8)
                .collect()
        };
        let th = |p: &[u8]| Thumb::new(p, w, w, h);
        let three = |p: &[u8]| [th(p), th(p), th(p)];
        let t = estimate_b(&three(&blend(0.4)), &three(&blend(0.3)), &three(&pa)).unwrap();
        let s = f64::from(1u32 << (t.luma_log2 + 1));
        // α = 0.4 / 0.3, β = 1 − 0.4 − α·0.7.
        let al = 0.4 / 0.3;
        assert!((f64::from(t.wp[0][0].w) / s - al).abs() < 0.02, "{t:?}");
        assert!(
            (f64::from(t.wp[1][0].w) / s - (0.6 - al * 0.7)).abs() < 0.02,
            "{t:?}"
        );
    }
}
