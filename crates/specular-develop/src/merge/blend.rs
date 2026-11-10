//! Multiband blending (Burt & Adelson, "A multiresolution spline with application to image
//! mosaics", ACM TOG 1983): each picture split into a Laplacian pyramid, each frame's
//! weights into a Gaussian one, and every band blended with its own blurred weights, so
//! fine detail switches sharply at a seam while broad brightness changes blend over a wide
//! band. Also the pyramids the focus stack and exposure fusion use.
//!
//! - **Kernel**: the binomial 5-tap `[1 4 6 4 1]/16`, separable, borders mirrored without
//!   repeating the edge sample; a level is `⌈n/2⌉` on each side.
//! - **Holes**: a frame's picture is filled where it has no value (NaN) by push-pull
//!   (weighted means down the pyramid, then each hole from the level above), so its
//!   Laplacian bands do not ring at its border; its weight there is 0.
//! - **Parts**: frames come as crops placed in the target (offsets multiples of 2^levels,
//!   so every level lines up exactly); only the target's pyramids span the whole area.
//!
//! In the raw panorama each CFA lattice (a half-size image of one colour) is blended on
//! its own, so the result is still a mosaic.

use rayon::prelude::*;

/// A single-channel float image.
#[derive(Clone, Debug, PartialEq)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub v: Vec<f32>,
}

impl Img {
    pub fn new(w: usize, h: usize, fill: f32) -> Img {
        Img {
            w,
            h,
            v: vec![fill; w * h],
        }
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize) -> f32 {
        self.v[y * self.w + x]
    }
}

const K: [f32; 5] = [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0];

/// Mirror index (… 2 1 | 0 1 2 … n−1 | n−2 …).
#[inline]
fn mirror(i: i64, n: usize) -> usize {
    let n = n as i64;
    if n == 1 {
        return 0;
    }
    let p = 2 * (n - 1);
    let mut i = i.rem_euclid(p);
    if i >= n {
        i = p - i;
    }
    i as usize
}

/// Smoothing by the kernel, then every second sample.
pub fn down(a: &Img) -> Img {
    let (w, h) = (a.w, a.h);
    let (dw, dh) = (w.div_ceil(2), h.div_ceil(2));
    // Horizontal pass at the kept columns.
    let mut t = vec![0f32; dw * h];
    t.par_chunks_mut(dw).enumerate().for_each(|(y, row)| {
        let src = &a.v[y * w..][..w];
        for (x, o) in row.iter_mut().enumerate() {
            let c = 2 * x as i64;
            let mut s = 0.0;
            for (k, kv) in K.iter().enumerate() {
                s += kv * src[mirror(c + k as i64 - 2, w)];
            }
            *o = s;
        }
    });
    let mut out = vec![0f32; dw * dh];
    out.par_chunks_mut(dw).enumerate().for_each(|(y, row)| {
        let c = 2 * y as i64;
        for (k, kv) in K.iter().enumerate() {
            let r = &t[mirror(c + k as i64 - 2, h) * dw..][..dw];
            for (o, v) in row.iter_mut().zip(r) {
                *o += kv * v;
            }
        }
    });
    Img {
        w: dw,
        h: dh,
        v: out,
    }
}

/// The level below at `w × h`: zeros between the samples, smoothed by twice the kernel.
pub fn up(a: &Img, w: usize, h: usize) -> Img {
    // Horizontal: output column x takes source columns (x + k − 2)/2 where even.
    let mut t = vec![0f32; w * a.h];
    t.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let src = &a.v[y * a.w..][..a.w];
        for (x, o) in row.iter_mut().enumerate() {
            let mut s = 0.0;
            for (k, kv) in K.iter().enumerate() {
                let p = x as i64 + k as i64 - 2;
                let p = mirror(p, w) as i64;
                if p % 2 == 0 {
                    s += 2.0 * kv * src[((p / 2) as usize).min(a.w - 1)];
                }
            }
            *o = s;
        }
    });
    let mut out = vec![0f32; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (k, kv) in K.iter().enumerate() {
            let p = mirror(y as i64 + k as i64 - 2, h) as i64;
            if p % 2 != 0 {
                continue;
            }
            let r = &t[((p / 2) as usize).min(a.h - 1) * w..][..w];
            for (o, v) in row.iter_mut().zip(r) {
                *o += 2.0 * kv * v;
            }
        }
    });
    Img { w, h, v: out }
}

/// Levels a `w × h` image can be split into, at most `max` (each at least 4 samples).
pub fn levels_for(w: usize, h: usize, max: usize) -> usize {
    let mut n = 1;
    let (mut a, mut b) = (w, h);
    while n < max && a.min(b) >= 8 {
        a = a.div_ceil(2);
        b = b.div_ceil(2);
        n += 1;
    }
    n
}

/// The Gaussian pyramid of `a` with `n` levels.
pub fn gaussian(a: &Img, n: usize) -> Vec<Img> {
    let mut g = vec![a.clone()];
    for _ in 1..n {
        let next = down(g.last().unwrap_or(a));
        g.push(next);
    }
    g
}

/// The Laplacian pyramid of `a` with `n` levels (the last is the coarsest Gaussian).
pub fn laplacian(a: &Img, n: usize) -> Vec<Img> {
    let g = gaussian(a, n);
    let mut l = Vec::with_capacity(n);
    for k in 0..n - 1 {
        let e = up(&g[k + 1], g[k].w, g[k].h);
        let mut b = g[k].clone();
        b.v.par_iter_mut()
            .zip(e.v.par_iter())
            .for_each(|(x, y)| *x -= y);
        l.push(b);
    }
    l.push(g[n - 1].clone());
    l
}

/// The image of a Laplacian pyramid.
pub fn collapse(l: &[Img]) -> Img {
    let mut a = l[l.len() - 1].clone();
    for k in (0..l.len() - 1).rev() {
        let mut e = up(&a, l[k].w, l[k].h);
        e.v.par_iter_mut()
            .zip(l[k].v.par_iter())
            .for_each(|(x, y)| *x += y);
        a = e;
    }
    a
}

/// `a` with its NaN holes filled by push-pull. `None` when it has no value at all.
pub fn fill(a: &Img) -> Option<Img> {
    if !a.v.iter().any(|v| v.is_finite()) {
        return None;
    }
    if a.v.iter().all(|v| v.is_finite()) {
        return Some(a.clone());
    }
    // Push: weighted means down to one sample.
    let mut vals = vec![Img {
        w: a.w,
        h: a.h,
        v: a.v
            .iter()
            .map(|v| if v.is_finite() { *v } else { 0.0 })
            .collect(),
    }];
    let mut wts = vec![Img {
        w: a.w,
        h: a.h,
        v: a.v
            .iter()
            .map(|v| f32::from(u8::from(v.is_finite())))
            .collect(),
    }];
    while vals.last().is_some_and(|v| v.w > 1 || v.h > 1) {
        let (Some(v), Some(m)) = (vals.last(), wts.last()) else {
            break;
        };
        let (dv, dm) = (down(v), down(m));
        vals.push(dv);
        wts.push(dm);
    }
    // Normalise every level where it has weight.
    for (v, m) in vals.iter_mut().zip(&wts) {
        for (x, w) in v.v.iter_mut().zip(&m.v) {
            *x = if *w > 1e-6 { *x / w } else { f32::NAN };
        }
    }
    // Pull: holes from the level above.
    for k in (0..vals.len() - 1).rev() {
        let coarse = &vals[k + 1];
        let c = Img {
            w: coarse.w,
            h: coarse.h,
            v: coarse
                .v
                .iter()
                .map(|v| if v.is_finite() { *v } else { 0.0 })
                .collect(),
        };
        let e = up(&c, vals[k].w, vals[k].h);
        let fine = &mut vals[k];
        let orig = (k == 0).then_some(&a.v);
        for (i, x) in fine.v.iter_mut().enumerate() {
            let keep = match orig {
                Some(o) => o[i].is_finite(),
                None => x.is_finite(),
            };
            if !keep {
                *x = e.v[i];
            } else if let Some(o) = orig {
                *x = o[i];
            }
        }
    }
    vals.into_iter().next()
}

/// A frame's crop placed in the target: its picture (NaN where it has none) and weights.
#[derive(Clone, Debug)]
pub struct Part {
    /// Offsets in the target, multiples of 2^(levels − 1).
    pub x0: usize,
    pub y0: usize,
    pub img: Img,
    pub weight: Img,
}

/// The blend of `parts` over a `w × h` target in `levels` bands; NaN where no part has
/// weight. `cancel` is polled between parts.
pub fn blend(
    parts: &[Part],
    w: usize,
    h: usize,
    levels: usize,
    cancel: &dyn Fn() -> bool,
) -> Option<Img> {
    let n = levels.max(1);
    let mut num: Vec<Img> = Vec::with_capacity(n);
    let mut den: Vec<Img> = Vec::with_capacity(n);
    let (mut lw, mut lh) = (w, h);
    for _ in 0..n {
        num.push(Img::new(lw, lh, 0.0));
        den.push(Img::new(lw, lh, 0.0));
        lw = lw.div_ceil(2);
        lh = lh.div_ceil(2);
    }
    let mut covered = vec![false; w * h];
    for p in parts {
        if cancel() {
            return None;
        }
        let Some(filled) = fill(&p.img) else { continue };
        for y in 0..p.img.h {
            for x in 0..p.img.w {
                if p.weight.at(x, y) > 0.0 && (p.y0 + y) < h && (p.x0 + x) < w {
                    covered[(p.y0 + y) * w + p.x0 + x] = true;
                }
            }
        }
        let l = laplacian(&filled, n);
        let g = gaussian(&p.weight, n);
        for k in 0..n {
            let (ox, oy) = (p.x0 >> k, p.y0 >> k);
            let (li, gi) = (&l[k], &g[k]);
            let (tn, td) = (&mut num[k], &mut den[k]);
            let tw = tn.w;
            let rows = li.h.min(tn.h.saturating_sub(oy));
            let cols = li.w.min(tw.saturating_sub(ox));
            tn.v.par_chunks_mut(tw)
                .zip(td.v.par_chunks_mut(tw))
                .skip(oy)
                .take(rows)
                .enumerate()
                .for_each(|(y, (rn, rd))| {
                    for x in 0..cols {
                        let wt = gi.v[y * gi.w + x];
                        rn[ox + x] += wt * li.v[y * li.w + x];
                        rd[ox + x] += wt;
                    }
                });
        }
    }
    let bands: Vec<Img> = num
        .into_iter()
        .zip(den)
        .map(|(mut a, d)| {
            a.v.par_iter_mut()
                .zip(d.v.par_iter())
                .for_each(|(x, w)| *x = if *w > 1e-12 { *x / w } else { 0.0 });
            a
        })
        .collect();
    let mut out = collapse(&bands);
    out.v
        .par_iter_mut()
        .zip(covered.par_iter())
        .for_each(|(x, &c)| *x = if c { x.max(0.0) } else { f32::NAN });
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(w: usize, h: usize) -> Img {
        Img {
            w,
            h,
            v: (0..w * h)
                .map(|i| {
                    let (x, y) = ((i % w) as f32, (i / w) as f32);
                    0.5 + 0.3 * (x * 0.37).sin() * (y * 0.23).cos() + 0.001 * x
                })
                .collect(),
        }
    }

    #[test]
    fn pyramid_round_trip_is_exact_to_rounding() {
        for (w, h) in [(64, 48), (37, 29), (5, 3)] {
            let a = picture(w, h);
            let n = levels_for(w, h, 7);
            let back = collapse(&laplacian(&a, n));
            let err =
                a.v.iter()
                    .zip(&back.v)
                    .map(|(x, y)| (x - y).abs())
                    .fold(0.0f32, f32::max);
            assert!(err < 1e-6, "{w}×{h}: {err:e}");
        }
    }

    #[test]
    fn blending_identical_pictures_is_the_identity() {
        let (w, h) = (96, 64);
        let a = picture(w, h);
        // Three frames with uneven weights, zero over parts of each.
        let part = |from: usize, to: usize, k: f32| {
            let mut wt = Img::new(w, h, 0.0);
            for y in 0..h {
                for x in from..to {
                    wt.v[y * w + x] = k * (1.0 + (x as f32 * 0.1).sin().abs());
                }
            }
            Part {
                x0: 0,
                y0: 0,
                img: a.clone(),
                weight: wt,
            }
        };
        let parts = vec![part(0, 64, 1.0), part(32, 96, 0.5), part(16, 64, 2.0)];
        let out = blend(&parts, w, h, 5, &|| false).expect("blended");
        let err =
            a.v.iter()
                .zip(&out.v)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0f32, f32::max);
        assert!(err < 1e-6, "{err:e}");
    }

    #[test]
    fn fill_keeps_values_and_closes_holes() {
        let mut a = picture(40, 30);
        for y in 10..20 {
            for x in 5..25 {
                a.v[y * 40 + x] = f32::NAN;
            }
        }
        let f = fill(&a).expect("filled");
        for (o, n) in a.v.iter().zip(&f.v) {
            assert!(n.is_finite());
            if o.is_finite() {
                assert_eq!(o, n);
            }
        }
    }
}
