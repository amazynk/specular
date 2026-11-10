//! Showing two aligned revisions of a sheet together, and finding what changed.
//!
//! - [`View::Overlay`]: this sheet's lines red, the other's cyan, lines on both black (each
//!   pixel is (other, this, this) as grey levels, so paper stays white).
//! - [`View::Difference`]: white where the sheets agree, darker the more they differ.
//! - [`View::Blend`]: one sheet faded into the other (0 this, 1 the other); blinking
//!   switches between 0 and 1.
//!
//! [`changes`] finds the regions where one sheet has lines the other has nowhere near (a
//! pixel or two of slack for alignment and rendering), merged into rectangles: the clouds of
//! a revision.

use crate::P;
use crate::align::{Grey, Similarity};

/// How the two sheets are shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    Overlay,
    Difference,
    Blend(f32),
}

/// `other` resampled into this sheet's `w × h` frame through `t` (its pixels onto these).
pub fn warp(other: &Grey, t: &Similarity, w: usize, h: usize) -> Grey {
    let inv = t.inverse();
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let q = inv.apply(P::new(x as f64 + 0.5, y as f64 + 0.5));
            px.push(other.sample(q.x, q.y).round() as u8);
        }
    }
    Grey { w, h, px }
}

/// RGBA of the two sheets (`this` and `other`, the same size) shown as `view`.
pub fn compose(this: &Grey, other: &Grey, view: View) -> Vec<u8> {
    let mut out = Vec::with_capacity(this.px.len() * 4);
    for (&a, &b) in this.px.iter().zip(&other.px) {
        let rgb = match view {
            View::Overlay => [b, a, a],
            View::Difference => {
                let d = 255 - a.abs_diff(b);
                [d, d, d]
            }
            View::Blend(t) => {
                let t = t.clamp(0.0, 1.0);
                let v = (f32::from(a) * (1.0 - t) + f32::from(b) * t).round() as u8;
                [v, v, v]
            }
        };
        out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
    }
    out
}

/// Darker than this is a line.
const INK: u8 = 160;

/// Each pixel's distance (in pixels, up to `r`) within which the mask has a set pixel: a
/// square dilation by `r`.
fn dilate(m: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    if r == 0 {
        return m.to_vec();
    }
    let mut t = vec![false; w * h];
    for y in 0..h {
        let row = &m[y * w..(y + 1) * w];
        // Running count over the window.
        let mut count: usize = row[..r.min(w)].iter().map(|&b| usize::from(b)).sum();
        for x in 0..w {
            if x + r < w {
                count += usize::from(row[x + r]);
            }
            if x > r {
                count -= usize::from(row[x - r - 1]);
            }
            t[y * w + x] = count > 0;
        }
    }
    let mut out = vec![false; w * h];
    for x in 0..w {
        let mut count = 0usize;
        for y in 0..(r.min(h)) {
            count += usize::from(t[y * w + x]);
        }
        for y in 0..h {
            if y + r < h {
                count += usize::from(t[(y + r) * w + x]);
            }
            if y > r {
                count -= usize::from(t[(y - r - 1) * w + x]);
            }
            out[y * w + x] = count > 0;
        }
    }
    out
}

/// The regions (x0, y0, x1, y1 in pixels of `this`) where either sheet has lines the other
/// lacks within `slack` px; changes closer than `merge` px are one region, and regions with
/// fewer than `min_px` changed pixels are dropped.
pub fn changes(
    this: &Grey,
    other: &Grey,
    slack: usize,
    merge: usize,
    min_px: usize,
) -> Vec<[f64; 4]> {
    let (w, h) = (this.w, this.h);
    let ia: Vec<bool> = this.px.iter().map(|&v| v < INK).collect();
    let ib: Vec<bool> = other.px.iter().map(|&v| v < INK).collect();
    let (da, db) = (dilate(&ia, w, h, slack), dilate(&ib, w, h, slack));
    let changed: Vec<bool> = (0..w * h)
        .map(|i| (ia[i] && !db[i]) || (ib[i] && !da[i]))
        .collect();
    let grown = dilate(&changed, w, h, merge);
    // Connected regions of the grown mask, counting the changed pixels in each.
    let mut label = vec![u32::MAX; w * h];
    let mut out = Vec::new();
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !grown[start] || label[start] != u32::MAX {
            continue;
        }
        let id = out.len() as u32;
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        let mut n = 0usize;
        label[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            if changed[i] {
                n += 1;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
            let mut visit = |j: usize| {
                if grown[j] && label[j] == u32::MAX {
                    label[j] = id;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
        out.push((n, [x0, y0, x1, y1]));
    }
    let pad = (merge / 2).max(2) as f64;
    out.into_iter()
        .filter(|(n, _)| *n >= min_px.max(1))
        .map(|(_, [x0, y0, x1, y1])| {
            [
                (x0 as f64 - pad).max(0.0),
                (y0 as f64 - pad).max(0.0),
                ((x1 + 1) as f64 + pad).min(w as f64),
                ((y1 + 1) as f64 + pad).min(h as f64),
            ]
        })
        .collect()
}
