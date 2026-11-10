//! Crop helpers for the editor (no pixels): aspect presets, keeping a crop inside the
//! straightened image, straighten by a drawn line, and composition guides.
//!
//! Crops are `[left, top, right, bottom]` fractions of the straightened frame (`w × h`
//! sensor pixels, see [`super`]); ratios are width over height in pixels.

/// The aspect presets the editor offers, by their stored names.
pub const ASPECTS: [&str; 7] = ["free", "original", "1:1", "4:3", "3:2", "16:9", "custom"];

/// Composition guides drawn over the crop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Guide {
    Thirds,
    Golden,
    Diagonal,
    Grid,
}

impl Guide {
    pub const ALL: [Guide; 4] = [Guide::Thirds, Guide::Golden, Guide::Diagonal, Guide::Grid];

    pub fn next(self) -> Guide {
        let i = Guide::ALL.iter().position(|g| *g == self).unwrap_or(0);
        Guide::ALL[(i + 1) % Guide::ALL.len()]
    }

    /// Line segments in crop fractions (0..1 across the crop), for a crop `aspect` (w/h).
    pub fn lines(self, aspect: f64) -> Vec<[(f64, f64); 2]> {
        let v = |x: f64| [(x, 0.0), (x, 1.0)];
        let h = |y: f64| [(0.0, y), (1.0, y)];
        match self {
            Guide::Thirds => vec![v(1.0 / 3.0), v(2.0 / 3.0), h(1.0 / 3.0), h(2.0 / 3.0)],
            Guide::Golden => {
                let g = 1.0 / (1.0 + 5f64.sqrt()) * 2.0;
                let a = 1.0 - g;
                vec![v(a), v(g), h(a), h(g)]
            }
            Guide::Grid => (1..8)
                .flat_map(|i| [v(f64::from(i) / 8.0), h(f64::from(i) / 8.0)])
                .collect(),
            Guide::Diagonal => {
                // 45° lines from each corner (in pixels, so they meet the long sides).
                let a = aspect.max(1e-6);
                if a >= 1.0 {
                    let d = 1.0 / a;
                    vec![
                        [(0.0, 0.0), (d, 1.0)],
                        [(0.0, 1.0), (d, 0.0)],
                        [(1.0, 0.0), (1.0 - d, 1.0)],
                        [(1.0, 1.0), (1.0 - d, 0.0)],
                    ]
                } else {
                    vec![
                        [(0.0, 0.0), (1.0, a)],
                        [(1.0, 0.0), (0.0, a)],
                        [(0.0, 1.0), (1.0, 1.0 - a)],
                        [(1.0, 1.0), (0.0, 1.0 - a)],
                    ]
                }
            }
        }
    }
}

/// The part of a straightened `w × h` frame that shows the image: the points that the map
/// `g` (straightened → oriented, both centred; see [`super::Plan::persp`]) sends inside the
/// oriented frame. With a turn alone or a valid keystone the region is convex, so a crop
/// whose four corners are inside is inside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub w: f64,
    pub h: f64,
    pub g: [[f64; 3]; 3],
}

impl Frame {
    /// Straightened by `angle` (radians, clockwise) only.
    pub fn turned(w: f64, h: f64, angle: f64) -> Frame {
        let (s, c) = angle.sin_cos();
        Frame {
            w,
            h,
            g: [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// With a perspective map.
    pub fn warped(w: f64, h: f64, g: [[f64; 3]; 3]) -> Frame {
        Frame { w, h, g }
    }

    /// Whether frame point (`x`, `y`) (pixels) shows the image.
    pub fn inside(&self, x: f64, y: f64) -> bool {
        let (w, h) = (self.w, self.h);
        match super::persp::apply(&self.g, (x - w / 2.0, y - h / 2.0)) {
            Some((px, py)) => px.abs() <= w / 2.0 + 1e-9 && py.abs() <= h / 2.0 + 1e-9,
            None => false,
        }
    }

    /// Whether all four corners of `crop` show the image.
    pub fn crop_inside(&self, crop: [f64; 4]) -> bool {
        let [l, t, r, b] = crop;
        [(l, t), (r, t), (l, b), (r, b)]
            .iter()
            .all(|&(x, y)| self.inside(x * self.w, y * self.h))
    }

    /// The image's outline in the frame (pixels): the oriented frame's corners mapped
    /// forward, clockwise from the top left; `None` when the map does not invert.
    pub fn outline(&self) -> Option<[(f64, f64); 4]> {
        let fwd = super::persp::inverse(&self.g)?;
        let (w, h) = (self.w / 2.0, self.h / 2.0);
        let mut out = [(0.0, 0.0); 4];
        for (o, c) in out.iter_mut().zip([(-w, -h), (w, -h), (w, h), (-w, h)]) {
            let q = super::persp::apply(&fwd, c)?;
            *o = (q.0 + w, q.1 + h);
        }
        Some(out)
    }
}

/// Whether frame point (`x`, `y`) (pixels of a `w × h` straightened frame) shows the image:
/// rotated back by `angle` (radians, clockwise) it lies inside the oriented image.
pub fn inside(x: f64, y: f64, w: f64, h: f64, angle: f64) -> bool {
    Frame::turned(w, h, angle).inside(x, y)
}

/// Whether all four corners of `crop` show the image.
pub fn crop_inside(crop: [f64; 4], w: f64, h: f64, angle: f64) -> bool {
    Frame::turned(w, h, angle).crop_inside(crop)
}

/// `crop` shrunk about its centre (keeping its shape) until it shows only the image; the
/// centre moves inside first if it is outside.
pub fn fit_inside(crop: [f64; 4], w: f64, h: f64, angle: f64) -> [f64; 4] {
    fit_inside_in(crop, &Frame::turned(w, h, angle))
}

/// [`fit_inside`] in any [`Frame`].
pub fn fit_inside_in(crop: [f64; 4], fr: &Frame) -> [f64; 4] {
    if fr.crop_inside(crop) {
        return crop;
    }
    let [l, t, r, b] = crop;
    let (mut cx, mut cy) = ((l + r) / 2.0, (t + b) / 2.0);
    if !fr.inside(cx * fr.w, cy * fr.h) {
        cx = 0.5;
        cy = 0.5;
    }
    let (hw, hh) = ((r - l) / 2.0, (b - t) / 2.0);
    let at = |k: f64| [cx - hw * k, cy - hh * k, cx + hw * k, cy + hh * k];
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..50 {
        let mid = 0.5 * (lo + hi);
        if fr.crop_inside(at(mid)) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    at(lo)
}

/// The largest crop of `ratio` (w/h pixels; `None`: the frame's own) centred in a `w × h`
/// frame straightened by `angle` that shows only the image.
pub fn largest(ratio: Option<f64>, w: f64, h: f64, angle: f64) -> [f64; 4] {
    largest_in(ratio, &Frame::turned(w, h, angle))
}

/// [`largest`] in any [`Frame`].
pub fn largest_in(ratio: Option<f64>, fr: &Frame) -> [f64; 4] {
    let (w, h) = (fr.w, fr.h);
    let ratio = ratio.unwrap_or(w / h.max(1e-9));
    // Full width or full height, whichever fits.
    let (cw, ch) = if ratio >= w / h {
        (1.0, w / ratio / h)
    } else {
        (h * ratio / w, 1.0)
    };
    let c = [
        0.5 - cw / 2.0,
        0.5 - ch / 2.0,
        0.5 + cw / 2.0,
        0.5 + ch / 2.0,
    ];
    fit_inside_in(c, fr)
}

/// `crop` given `ratio` (w/h pixels) about its centre, as large as fits in the frame
/// (`w × h`, straightened by `angle`) and no larger than its area allows.
pub fn with_ratio(crop: [f64; 4], ratio: f64, w: f64, h: f64, angle: f64) -> [f64; 4] {
    with_ratio_in(crop, ratio, &Frame::turned(w, h, angle))
}

/// [`with_ratio`] in any [`Frame`].
pub fn with_ratio_in(crop: [f64; 4], ratio: f64, fr: &Frame) -> [f64; 4] {
    let (w, h) = (fr.w, fr.h);
    let [l, t, r, b] = crop;
    let (cx, cy) = ((l + r) / 2.0, (t + b) / 2.0);
    // Keep the area (in pixels), change the shape.
    let area = (r - l) * w * (b - t) * h;
    let pw = (area * ratio).sqrt();
    let ph = pw / ratio;
    let (mut hw, mut hh) = (pw / w / 2.0, ph / h / 2.0);
    // Within the frame.
    let k = (cx / hw)
        .min((1.0 - cx) / hw)
        .min(cy / hh)
        .min((1.0 - cy) / hh)
        .min(1.0);
    hw *= k;
    hh *= k;
    fit_inside_in([cx - hw, cy - hh, cx + hw, cy + hh], fr)
}

/// The straighten angle (degrees, clockwise) that makes the line from `a` to `b` (screen
/// pixels of the shown image, y down) level, or plumb when it is nearer vertical.
pub fn level_angle(a: (f64, f64), b: (f64, f64)) -> Option<f64> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if dx.hypot(dy) < 4.0 {
        return None;
    }
    let t = dy.atan2(dx).to_degrees();
    // The nearest multiple of 90°; turning by the difference makes the line match it.
    let target = (t / 90.0).round() * 90.0;
    Some(target - t)
}

/// Which part of the crop a pointer at crop fractions (`u`, `v`) grabs (outside the crop:
/// `None`). `edge` is the grab distance in fractions of the crop (x, y).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grab {
    /// Bit 0 left, 1 top, 2 right, 3 bottom.
    Handle(u8),
    Move,
}

pub fn grab(u: f64, v: f64, edge: (f64, f64)) -> Option<Grab> {
    if u < -edge.0 || v < -edge.1 || u > 1.0 + edge.0 || v > 1.0 + edge.1 {
        return None;
    }
    let mut m = 0;
    if u.abs() <= edge.0 {
        m |= 1;
    }
    if v.abs() <= edge.1 {
        m |= 2;
    }
    if (u - 1.0).abs() <= edge.0 {
        m |= 4;
    }
    if (v - 1.0).abs() <= edge.1 {
        m |= 8;
    }
    Some(if m == 0 { Grab::Move } else { Grab::Handle(m) })
}

/// Drags `crop` (as it was when the drag began) by (`dx`, `dy`) frame fractions with
/// `grab`; keeps `ratio` (w/h pixels) when given and the crop inside the image.
#[allow(clippy::too_many_arguments)]
pub fn drag(
    crop: [f64; 4],
    g: Grab,
    dx: f64,
    dy: f64,
    ratio: Option<f64>,
    w: f64,
    h: f64,
    angle: f64,
) -> [f64; 4] {
    drag_in(crop, g, dx, dy, ratio, &Frame::turned(w, h, angle))
}

/// [`drag`] in any [`Frame`].
pub fn drag_in(
    crop: [f64; 4],
    g: Grab,
    dx: f64,
    dy: f64,
    ratio: Option<f64>,
    fr: &Frame,
) -> [f64; 4] {
    let (w, h) = (fr.w, fr.h);
    let min = 0.02;
    let [l, t, r, b] = crop;
    match g {
        Grab::Move => {
            let dx = dx.clamp(-l, 1.0 - r);
            let dy = dy.clamp(-t, 1.0 - b);
            let c = [l + dx, t + dy, r + dx, b + dy];
            if fr.crop_inside(c) {
                c
            } else {
                // Step back towards the start until it fits.
                let (mut lo, mut hi) = (0.0, 1.0);
                for _ in 0..40 {
                    let m = 0.5 * (lo + hi);
                    if fr.crop_inside([l + dx * m, t + dy * m, r + dx * m, b + dy * m]) {
                        lo = m;
                    } else {
                        hi = m;
                    }
                }
                [l + dx * lo, t + dy * lo, r + dx * lo, b + dy * lo]
            }
        }
        Grab::Handle(m) => {
            let mut c = crop;
            if m & 1 != 0 {
                c[0] = (l + dx).clamp(0.0, r - min);
            }
            if m & 4 != 0 {
                c[2] = (r + dx).clamp(l + min, 1.0);
            }
            if m & 2 != 0 {
                c[1] = (t + dy).clamp(0.0, b - min);
            }
            if m & 8 != 0 {
                c[3] = (b + dy).clamp(t + min, 1.0);
            }
            if let Some(ratio) = ratio {
                c = keep_ratio(c, m, ratio, w, h);
            }
            // Shrink back towards the fixed side until inside the image.
            if !fr.crop_inside(c) {
                let (mut lo, mut hi) = (0.0, 1.0);
                let mix = |k: f64| -> [f64; 4] {
                    std::array::from_fn(|i| crop[i] + (c[i] - crop[i]) * k)
                };
                if !fr.crop_inside(crop) {
                    return fit_inside_in(c, fr);
                }
                for _ in 0..40 {
                    let mm = 0.5 * (lo + hi);
                    if fr.crop_inside(mix(mm)) {
                        lo = mm;
                    } else {
                        hi = mm;
                    }
                }
                c = mix(lo);
            }
            c
        }
    }
}

/// Makes `c` (moved by handles `m`) take `ratio`, keeping the opposite side or corner.
fn keep_ratio(c: [f64; 4], m: u8, ratio: f64, w: f64, h: f64) -> [f64; 4] {
    let [mut l, mut t, mut r, mut b] = c;
    let pw = (r - l) * w;
    let ph = (b - t) * h;
    let horizontal = m & 5 != 0;
    let vertical = m & 10 != 0;
    // Corner: follow the larger relative change; an edge drives the other axis.
    let (pw, ph) = if horizontal && (!vertical || pw / ratio >= ph) {
        (pw, pw / ratio)
    } else {
        (ph * ratio, ph)
    };
    let (fw, fh) = (pw / w, ph / h);
    if m & 1 != 0 {
        l = r - fw;
    } else if m & 4 != 0 {
        r = l + fw;
    } else {
        let cx = (l + r) / 2.0;
        l = cx - fw / 2.0;
        r = cx + fw / 2.0;
    }
    if m & 2 != 0 {
        t = b - fh;
    } else if m & 8 != 0 {
        b = t + fh;
    } else {
        let cy = (t + b) / 2.0;
        t = cy - fh / 2.0;
        b = cy + fh / 2.0;
    }
    // Back inside 0..1 by scaling about the fixed point.
    let fx = if m & 1 != 0 { r } else { l };
    let fy = if m & 2 != 0 { b } else { t };
    let mut k: f64 = 1.0;
    for (v, f) in [(l, fx), (r, fx)] {
        if v < 0.0 {
            k = k.min(f / (f - v).max(1e-9));
        }
        if v > 1.0 {
            k = k.min((1.0 - f) / (v - f).max(1e-9));
        }
    }
    for (v, f) in [(t, fy), (b, fy)] {
        if v < 0.0 {
            k = k.min(f / (f - v).max(1e-9));
        }
        if v > 1.0 {
            k = k.min((1.0 - f) / (v - f).max(1e-9));
        }
    }
    let s = |v: f64, f: f64| f + (v - f) * k;
    [s(l, fx), s(t, fy), s(r, fx), s(b, fy)]
}
