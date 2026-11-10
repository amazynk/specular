//! Brush masks: strokes stamped into a raster over the source frame.
//!
//! - **Resolution independent.** A stroke is points in frame fractions, a radius in
//!   fractions of the long side; the raster is made at the size the render needs (the
//!   image's pixels over the frame), so the preview and an export stamp the same strokes at
//!   their own sizes.
//! - **Dabs.** The first point of a stroke is one dab; each segment after it gets dabs
//!   spaced a quarter of the radius apart (at least half a texel), placed from the segment
//!   alone, so appending a point stamps exactly what a full re-stamp would.
//! - **A dab** is on fully within `hardness · radius` and falls off by a smoothstep to zero
//!   at the radius; it adds `flow · alpha` of what is left (pressure scales the flow, the size or both:
//!   [`crate::model::mask::PressureUse`])
//!   (`m += (1 − m)·k`), or with erase takes that much away (`m −= m·k`).
//! - **Storage**: u16 (1/65535 steps, so even a strong exposure change shows no banding),
//!   rows padded to an even count so two texels pack a word of the mask table.
//! - **Incremental.** A cache entry remembers the strokes it stamped; when the new list only
//!   extends them (points appended to the last stroke, strokes appended), only the new dabs
//!   are stamped and only their rows are uploaded ([`Raster::dirty`]).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::model::mask::Stroke;

/// A brush raster.
#[derive(Clone, Debug)]
pub struct Raster {
    /// Identifies this allocation (a resize makes a new one).
    pub id: u64,
    pub w: u32,
    pub h: u32,
    /// Texels per row (even).
    pub stride: u32,
    pub data: Vec<u16>,
    /// Bumped on every change.
    pub rev: u64,
    /// What changed since revision `from`: rows `y0..y1` (`None`: everything).
    pub dirty: Option<Dirty>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dirty {
    pub from: u64,
    pub y0: u32,
    pub y1: u32,
}

/// A raster id unique in the process: the engine knows a raster by its id across caches.
pub(crate) fn next_raster_id() -> u64 {
    static IDS: AtomicU64 = AtomicU64::new(1);
    IDS.fetch_add(1, Ordering::Relaxed)
}

impl Raster {
    pub(crate) fn new(id: u64, w: u32, h: u32) -> Raster {
        let stride = w.div_ceil(2) * 2;
        Raster {
            id,
            w,
            h,
            stride,
            data: vec![0; stride as usize * h as usize],
            rev: 0,
            dirty: None,
        }
    }

    /// Words in the mask table.
    pub fn words(&self) -> u64 {
        u64::from(self.stride / 2) * u64::from(self.h)
    }

    pub fn bytes(&self) -> u64 {
        self.data.len() as u64 * 2
    }

    /// Texel (x, y) as 0..1, zero outside.
    pub fn texel(&self, x: i64, y: i64) -> f32 {
        if x < 0 || y < 0 || x >= i64::from(self.w) || y >= i64::from(self.h) {
            return 0.0;
        }
        f32::from(self.data[y as usize * self.stride as usize + x as usize]) / 65535.0
    }

    /// Bilinear sample at frame point (fractions), as the kernels do.
    pub fn sample(&self, fx: f32, fy: f32) -> f32 {
        let gx = fx * self.w as f32 - 0.5;
        let gy = fy * self.h as f32 - 0.5;
        if gx < -1.0 || gy < -1.0 || gx > self.w as f32 || gy > self.h as f32 {
            return 0.0;
        }
        let (x0, y0) = (gx.floor(), gy.floor());
        let (tx, ty) = (gx - x0, gy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let top = lerp(self.texel(x0, y0), self.texel(x0 + 1, y0), tx);
        let bot = lerp(self.texel(x0, y0 + 1), self.texel(x0 + 1, y0 + 1), tx);
        lerp(top, bot, ty)
    }

    fn clear(&mut self) {
        self.data.fill(0);
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// One cached brush shape.
#[derive(Debug)]
pub struct BrushEntry {
    pub raster: Arc<Raster>,
    /// What is stamped in it.
    stamped: Vec<Stroke>,
    /// The raster's long side (the frame's long side in texels).
    long: u32,
}

impl BrushEntry {
    pub(crate) fn new(next_id: &mut u64, w: u32, h: u32, long: u32) -> BrushEntry {
        *next_id = next_raster_id();
        BrushEntry {
            raster: Arc::new(Raster::new(*next_id, w, h)),
            stamped: Vec::new(),
            long,
        }
    }

    /// Brings the raster to `strokes` at `w`×`h` (`long` texels per long side).
    pub(crate) fn update(
        &mut self,
        strokes: &[Stroke],
        w: u32,
        h: u32,
        long: u32,
        next_id: &mut u64,
    ) {
        if (self.raster.w, self.raster.h) != (w, h) || self.long != long {
            *self = BrushEntry::new(next_id, w, h, long);
        }
        if self.stamped == strokes {
            return;
        }
        let from = self.raster.rev;
        let r = Arc::make_mut(&mut self.raster);
        r.rev += 1;
        let long = self.long as f32;
        match extends(&self.stamped, strokes) {
            Some((stroke, point)) => {
                let mut rows: Option<(u32, u32)> = None;
                for (i, s) in strokes.iter().enumerate().skip(stroke) {
                    let first = if i == stroke { point } else { 0 };
                    if let Some(t) = stamp_stroke(r, s, first, long) {
                        rows = Some(rows.map_or(t, |(a, b)| (a.min(t.0), b.max(t.1))));
                    }
                }
                let (y0, y1) = rows.unwrap_or((0, 0));
                r.dirty = Some(Dirty { from, y0, y1 });
            }
            None => {
                r.clear();
                for s in strokes {
                    stamp_stroke(r, s, 0, long);
                }
                r.dirty = None;
            }
        }
        self.stamped = strokes.to_vec();
    }
}

/// Where `new` goes on from `old` (stroke index, first new point), if it only extends it.
fn extends(old: &[Stroke], new: &[Stroke]) -> Option<(usize, usize)> {
    if old.is_empty() {
        return Some((0, 0));
    }
    if new.len() < old.len() {
        return None;
    }
    let last = old.len() - 1;
    if old[..last] != new[..last] {
        return None;
    }
    let (a, b) = (&old[last], &new[last]);
    let same = a.radius == b.radius
        && a.hardness == b.hardness
        && a.flow == b.flow
        && a.erase == b.erase
        && a.pressure == b.pressure
        && b.points.len() >= a.points.len()
        && b.points[..a.points.len()] == a.points[..];
    if !same {
        return None;
    }
    if b.points.len() > a.points.len() {
        Some((last, a.points.len()))
    } else {
        Some((last + 1, 0))
    }
}

fn finite01(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Stamps the dabs of `s` from point `first` on; returns the rows touched.
fn stamp_stroke(r: &mut Raster, s: &Stroke, first: usize, long: f32) -> Option<(u32, u32)> {
    let rad = if s.radius.is_finite() {
        (s.radius.max(0.0) * long).max(0.7)
    } else {
        return None;
    };
    let hard = finite01(s.hardness);
    let flow = finite01(s.flow);
    let spacing = (0.25 * rad).max(0.5);
    let mut rows: Option<(u32, u32)> = None;
    let mut dab = |r: &mut Raster, p: [f32; 3]| {
        let (kf, kr) = s.pressure.factors(p[2]);
        if let Some(t) = stamp(
            r,
            [p[0], p[1]],
            (rad * kr).max(0.7),
            hard,
            flow * kf,
            s.erase,
        ) {
            rows = Some(rows.map_or(t, |(a, b)| (a.min(t.0), b.max(t.1))));
        }
    };
    let (w, h) = (r.w as f32, r.h as f32);
    let px = |p: &[f32; 3]| [p[0] * w, p[1] * h, finite01(p[2])];
    for i in first..s.points.len() {
        let b = px(&s.points[i]);
        if !(b[0].is_finite() && b[1].is_finite()) {
            continue;
        }
        if i == 0 {
            dab(r, b);
            continue;
        }
        let a = px(&s.points[i - 1]);
        let d = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        let n = if d.is_finite() {
            ((d / spacing).ceil() as u32).clamp(1, 1 << 16)
        } else {
            1
        };
        for k in 1..=n {
            let t = k as f32 / n as f32;
            dab(
                r,
                [
                    lerp(a[0], b[0], t),
                    lerp(a[1], b[1], t),
                    lerp(a[2], b[2], t),
                ],
            );
        }
    }
    rows
}

/// One dab at texel point `p` adding `k` (0..1); returns the rows touched.
fn stamp(
    r: &mut Raster,
    p: [f32; 2],
    rad: f32,
    hard: f32,
    k: f32,
    erase: bool,
) -> Option<(u32, u32)> {
    if k <= 0.0 {
        return None;
    }
    let x0 = ((p[0] - rad).floor().max(0.0)) as u32;
    let y0 = ((p[1] - rad).floor().max(0.0)) as u32;
    let x1 = ((p[0] + rad).ceil().max(0.0) as u32).min(r.w);
    let y1 = ((p[1] + rad).ceil().max(0.0) as u32).min(r.h);
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let inner = hard * rad;
    let stride = r.stride as usize;
    for y in y0..y1 {
        let dy = y as f32 + 0.5 - p[1];
        let row = &mut r.data[y as usize * stride..y as usize * stride + r.w as usize];
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - p[0];
            let d = (dx * dx + dy * dy).sqrt();
            if d >= rad {
                continue;
            }
            let a = 1.0 - smoothstep(inner, rad, d);
            let m = f32::from(row[x as usize]) / 65535.0;
            let v = if erase {
                m - m * k * a
            } else {
                m + (1.0 - m) * k * a
            };
            row[x as usize] = (v.clamp(0.0, 1.0) * 65535.0).round() as u16;
        }
    }
    Some((y0, y1))
}

/// Smoothstep from `e0` to `e1` (a step at `e0` when they meet).
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x >= e0 { 1.0 } else { 0.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
