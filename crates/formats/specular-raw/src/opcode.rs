//! DNG opcode lists (`OpcodeList1`, `OpcodeList2`, `OpcodeList3`): parsing and application.
//!
//! Each list runs at the stage the DNG specification gives it:
//!
//! * **List 1** on the stored raw samples (sensor coordinates), before linearization;
//! * **List 2** on the linearized, black-subtracted samples normalised to 0..1 (white = 1),
//!   still a CFA mosaic (or linear RGB), coordinates relative to the `ActiveArea`;
//! * **List 3** after the demosaic, on linear camera RGB (before the colour matrix), same
//!   coordinates.
//!
//! [`DngOpcodes::apply_raw`] runs lists 1 and 2 over the samples a [`RawImage`] holds (the
//! sensor decode calls it, so every develop sees corrected samples); list 2 leaves the samples
//! normalised (black 0, white 65535, no linearization table). List 3 runs inside
//! [`crate::develop`], tile by tile: each output tile develops the source rectangle its warps
//! need (found exactly, by mapping every output pixel), then warps and point operations
//! follow, then the colour matrix. A region therefore equals the same pixels of the whole
//! develop, and [`DngOpcodes::sensor_window`] gives the sensor samples a region decode needs.
//!
//! Implemented: `WarpRectilinear`, `WarpFisheye` (list 3; Catmull-Rom resampling, edges
//! clamped to the developed crop), `FixVignetteRadial`, `GainMap`, `MapTable`,
//! `MapPolynomial`, `DeltaPerRow`/`Column`, `ScalePerRow`/`Column` (any list),
//! `FixBadPixelsConstant`, `FixBadPixelsList` (lists 1 and 2: inverse-distance mean of the
//! same-colour neighbours within two pixels) and `TrimBounds` (intersected with the crop).
//! Anything else is skipped; a skipped opcode without the "optional" flag is reported in
//! [`DngOpcodes::skipped`].

use std::fmt;
use std::sync::atomic::Ordering;

use rayon::prelude::*;

use crate::error::{RawError, Result};
use crate::{Cfa, DecodeOptions, OutputSize, RawImage, Rect};

/// `OpcodeList1`, `OpcodeList2`, `OpcodeList3`.
pub(crate) const TAGS: [u16; 3] = [0xc740, 0xc741, 0xc74e];

const NAMES: [&str; 15] = [
    "opcode 0",
    "WarpRectilinear",
    "WarpFisheye",
    "FixVignetteRadial",
    "FixBadPixelsConstant",
    "FixBadPixelsList",
    "TrimBounds",
    "MapTable",
    "MapPolynomial",
    "GainMap",
    "DeltaPerRow",
    "DeltaPerColumn",
    "ScalePerRow",
    "ScalePerColumn",
    "WarpRectilinear2",
];

fn name(id: u32) -> &'static str {
    NAMES.get(id as usize).copied().unwrap_or("unknown opcode")
}

/// Neighbour radius of the bad-pixel fixes (the sensor margin each adds to a region decode).
const BAD_RADIUS: u32 = 2;

/// Big-endian reader over an opcode's parameters.
struct Rd<'a>(&'a [u8], usize);

impl Rd<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let s = self.0.get(self.1..self.1.checked_add(N)?)?;
        self.1 += N;
        s.try_into().ok()
    }
    fn u32(&mut self) -> Option<u32> {
        self.take::<4>().map(u32::from_be_bytes)
    }
    fn u16(&mut self) -> Option<u16> {
        self.take::<2>().map(u16::from_be_bytes)
    }
    fn f64(&mut self) -> Option<f64> {
        self.take::<8>()
            .map(f64::from_be_bytes)
            .filter(|v| v.is_finite())
    }
    fn f32(&mut self) -> Option<f32> {
        self.take::<4>()
            .map(f32::from_be_bytes)
            .filter(|v| v.is_finite())
    }
    fn rect(&mut self) -> Option<[u32; 4]> {
        Some([self.u32()?, self.u32()?, self.u32()?, self.u32()?])
    }
}

pub(crate) fn intersect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x as u64 + a.w as u64).min(b.x as u64 + b.w as u64);
    let y1 = (a.y as u64 + a.h as u64).min(b.y as u64 + b.h as u64);
    Rect {
        x: x0,
        y: y0,
        w: x1.saturating_sub(x0 as u64) as u32,
        h: y1.saturating_sub(y0 as u64) as u32,
    }
}

/// `top, left, bottom, right` relative to `origin` as an absolute rectangle (empty if inverted).
fn abs_rect(t: [u32; 4], origin: (u32, u32)) -> Rect {
    let [top, left, bottom, right] = t;
    let x = origin.0 as u64 + left as u64;
    let y = origin.1 as u64 + top as u64;
    Rect {
        x: x.min(u32::MAX as u64) as u32,
        y: y.min(u32::MAX as u64) as u32,
        w: right.saturating_sub(left),
        h: bottom.saturating_sub(top),
    }
}

/// Rectangle and planes an area-based opcode touches.
#[derive(Clone)]
struct Area {
    /// Absolute (sensor) coordinates, within the image bounds.
    r: Rect,
    plane: u32,
    planes: u32,
    row_pitch: u32,
    col_pitch: u32,
}

impl Area {
    fn read(rd: &mut Rd, origin: (u32, u32), bounds: Rect) -> Option<Area> {
        let r = intersect(abs_rect(rd.rect()?, origin), bounds);
        let a = Area {
            r,
            plane: rd.u32()?,
            planes: rd.u32()?,
            row_pitch: rd.u32()?,
            col_pitch: rd.u32()?,
        };
        (a.row_pitch > 0 && a.col_pitch > 0 && a.planes > 0).then_some(a)
    }
    fn whole(bounds: Rect) -> Area {
        Area {
            r: bounds,
            plane: 0,
            planes: u32::MAX,
            row_pitch: 1,
            col_pitch: 1,
        }
    }
    /// Row `y` is in the area (and on its pitch).
    fn row(&self, y: i64) -> bool {
        let t = self.r.y as i64;
        y >= t && y < t + self.r.h as i64 && (y - t) % self.row_pitch as i64 == 0
    }
    /// Index of row / column `v` along the pitch (`v` inside the area).
    fn row_index(&self, y: i64) -> usize {
        ((y - self.r.y as i64) / self.row_pitch as i64) as usize
    }
    fn col_index(&self, x: i64) -> usize {
        ((x - self.r.x as i64) / self.col_pitch as i64) as usize
    }
    fn planes(&self, cpp: usize) -> std::ops::Range<usize> {
        let a = (self.plane as usize).min(cpp);
        a..(self.plane as u64 + self.planes as u64).min(cpp as u64) as usize
    }
}

#[derive(Clone)]
struct GainMap {
    points_v: usize,
    points_h: usize,
    spacing_v: f64,
    spacing_h: f64,
    origin_v: f64,
    origin_h: f64,
    planes: usize,
    /// `[v][h][plane]`.
    gains: Vec<f32>,
}

impl GainMap {
    fn read(rd: &mut Rd) -> Option<GainMap> {
        let points_v = rd.u32()? as usize;
        let points_h = rd.u32()? as usize;
        let spacing_v = rd.f64()?;
        let spacing_h = rd.f64()?;
        let origin_v = rd.f64()?;
        let origin_h = rd.f64()?;
        let planes = rd.u32()? as usize;
        let n = points_v.checked_mul(points_h)?.checked_mul(planes)?;
        if n == 0 || n > 1 << 22 {
            return None;
        }
        if (points_v > 1 && spacing_v <= 0.0) || (points_h > 1 && spacing_h <= 0.0) {
            return None;
        }
        let gains = (0..n).map(|_| rd.f32()).collect::<Option<Vec<_>>>()?;
        Some(GainMap {
            points_v,
            points_h,
            spacing_v,
            spacing_h,
            origin_v,
            origin_h,
            planes,
            gains,
        })
    }
    /// Map coordinate (index and fraction) of relative position `rel` along an axis.
    fn coord(rel: f64, origin: f64, spacing: f64, points: usize) -> (usize, usize, f32) {
        if points < 2 {
            return (0, 0, 0.0);
        }
        let m = ((rel - origin) / spacing).clamp(0.0, (points - 1) as f64);
        let i = (m.floor() as usize).min(points - 1);
        (i, (i + 1).min(points - 1), (m - i as f64) as f32)
    }
}

/// A per-pixel operation on normalised samples.
#[derive(Clone)]
enum PointFn {
    Table(Vec<u16>),
    Poly(Vec<f64>),
    Gain(GainMap),
    /// Gain `1 + k0 r^2 + ... + k4 r^10` around `c` (absolute), `r` scaled by `inv_m2`.
    Vignette {
        k: [f64; 5],
        c: (f64, f64),
        inv_m2: f64,
    },
    Delta {
        per_row: bool,
        v: Vec<f32>,
    },
    Scale {
        per_row: bool,
        v: Vec<f32>,
    },
}

#[derive(Clone)]
struct PointOp {
    area: Area,
    f: PointFn,
    /// Image bounds (absolute) the relative positions of a gain map refer to.
    bounds: Rect,
}

/// One line of pixels for a point operation: pixel `k` sits at sensor row `iy`, column
/// `ix0 + k * step`, with centre `(xc0 + k * step, yc)`.
#[derive(Clone, Copy)]
struct Line {
    iy: i64,
    yc: f64,
    ix0: i64,
    xc0: f64,
    step: i64,
}

impl PointOp {
    /// Applies the operation to `v` (`cpp` normalised samples per pixel of `line`).
    fn apply(&self, line: Line, v: &mut [f32], cpp: usize, scratch: &mut Vec<f32>) {
        let a = &self.area;
        if !a.row(line.iy) {
            return;
        }
        let planes = a.planes(cpp);
        if planes.is_empty() {
            return;
        }
        let n = v.len() / cpp;
        // Pixels k0..k1 lie inside the area's columns.
        let step = line.step;
        let left = a.r.x as i64;
        let right = left + a.r.w as i64;
        let k0 = if line.ix0 >= left {
            0
        } else {
            ((left - line.ix0) + step - 1) / step
        };
        let k1 = if line.ix0 >= right {
            0
        } else {
            (((right - line.ix0) + step - 1) / step).min(n as i64)
        };
        if k0 >= k1 {
            return;
        }
        let on_pitch = |x: i64| a.col_pitch == 1 || (x - left) % a.col_pitch as i64 == 0;
        let b = self.bounds;
        // Per-line preparation.
        let mut row_val = 0f32;
        let mut row_ok = true;
        match &self.f {
            PointFn::Gain(g) => {
                let rel = (line.yc - b.y as f64) / (b.h.max(1)) as f64;
                let (i0, i1, f) = GainMap::coord(rel, g.origin_v, g.spacing_v, g.points_v);
                let w = g.points_h * g.planes;
                scratch.clear();
                scratch.extend((0..w).map(|j| {
                    let p = g.gains[i0 * w + j];
                    p + (g.gains[i1 * w + j] - p) * f
                }));
            }
            PointFn::Delta { per_row: true, v }
            | PointFn::Scale {
                per_row: true, v, ..
            } => match v.get(a.row_index(line.iy)) {
                Some(&x) => row_val = x,
                None => row_ok = false,
            },
            _ => {}
        }
        if !row_ok {
            return;
        }
        for k in k0..k1 {
            let x = line.ix0 + k * step;
            if !on_pitch(x) {
                continue;
            }
            let xc = line.xc0 + (k * step) as f64;
            let px = &mut v[k as usize * cpp..(k as usize + 1) * cpp];
            match &self.f {
                PointFn::Table(t) => {
                    for p in planes.clone() {
                        let i = (px[p] * 65535.0 + 0.5) as usize;
                        px[p] = t[i.min(t.len() - 1)] as f32 / 65535.0;
                    }
                }
                PointFn::Poly(c) => {
                    for p in planes.clone() {
                        let s = px[p] as f64;
                        let y = c.iter().rev().fold(0.0, |acc, &ci| acc * s + ci);
                        px[p] = (y as f32).clamp(0.0, 1.0);
                    }
                }
                PointFn::Gain(g) => {
                    let rel = (xc - b.x as f64) / (b.w.max(1)) as f64;
                    let (j0, j1, f) = GainMap::coord(rel, g.origin_h, g.spacing_h, g.points_h);
                    for p in planes.clone() {
                        let mp = (p - a.plane as usize).min(g.planes - 1);
                        let l = scratch[j0 * g.planes + mp];
                        let r = scratch[j1 * g.planes + mp];
                        px[p] = (px[p] * (l + (r - l) * f)).clamp(0.0, 1.0);
                    }
                }
                PointFn::Vignette { k, c, inv_m2 } => {
                    let (dx, dy) = (xc - c.0, line.yc - c.1);
                    let r2 = (dx * dx + dy * dy) * inv_m2;
                    let g = 1.0 + r2 * (k[0] + r2 * (k[1] + r2 * (k[2] + r2 * (k[3] + r2 * k[4]))));
                    for p in planes.clone() {
                        px[p] = (px[p] * g as f32).clamp(0.0, 1.0);
                    }
                }
                PointFn::Delta { per_row, v } | PointFn::Scale { per_row, v } => {
                    let d = if *per_row {
                        row_val
                    } else {
                        match v.get(a.col_index(x)) {
                            Some(&d) => d,
                            None => continue,
                        }
                    };
                    let add = matches!(self.f, PointFn::Delta { .. });
                    for p in planes.clone() {
                        px[p] = if add { px[p] + d } else { px[p] * d }.clamp(0.0, 1.0);
                    }
                }
            }
        }
    }
}

/// Bad pixels to replace by their neighbours.
#[derive(Clone)]
enum BadPixels {
    /// Samples equal to this value.
    Constant(u16),
    /// Listed points `(row, column)` and rectangles (absolute).
    List {
        points: std::collections::HashSet<(u32, u32)>,
        rects: Vec<Rect>,
    },
}

impl BadPixels {
    fn is_bad(&self, y: u32, x: u32, v: u16) -> bool {
        match self {
            BadPixels::Constant(c) => v == *c,
            BadPixels::List { points, rects } => {
                points.contains(&(y, x))
                    || rects
                        .iter()
                        .any(|r| x >= r.x && y >= r.y && x - r.x < r.w && y - r.y < r.h)
            }
        }
    }
}

/// `WarpRectilinear` / `WarpFisheye` coefficients.
#[derive(Clone)]
struct Warp {
    fisheye: bool,
    /// Per plane (R, G, B): `kr0..kr3, kt0, kt1` (fisheye: `kt` zero).
    k: [[f64; 6]; 3],
    identity: [bool; 3],
    /// Optical centre, relative to the image bounds.
    c: (f64, f64),
    bounds: Rect,
}

#[derive(Clone)]
enum Op {
    Point(PointOp),
    Bad(BadPixels, Rect),
    Warp(Warp),
}

/// The opcode lists of a DNG, parsed (see the module documentation). Shared by the
/// [`RawImage`]s decoded from the file.
#[derive(Clone)]
pub struct DngOpcodes {
    lists: [Vec<Op>; 3],
    applied: Vec<&'static str>,
    skipped: Vec<&'static str>,
    /// `TrimBounds` (absolute), intersected with the crop by the parser.
    pub(crate) trim: Option<Rect>,
    /// The crop before `TrimBounds` (set by the container parser).
    pub(crate) untrimmed_crop: Rect,
}

impl fmt::Debug for DngOpcodes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DngOpcodes")
            .field("applied", &self.applied)
            .field("skipped", &self.skipped)
            .field("trim", &self.trim)
            .finish()
    }
}

impl DngOpcodes {
    /// Names of the opcodes that are applied, in list order (each once).
    pub fn applied(&self) -> &[&'static str] {
        &self.applied
    }
    /// Names of the opcodes without the "optional" flag that are not applied (unknown,
    /// unsupported at their stage or malformed).
    pub fn skipped(&self) -> &[&'static str] {
        &self.skipped
    }
    /// Whether `OpcodeList1` or `OpcodeList2` replaces the sample at sensor row `y`, column
    /// `x` (absolute) whose stored value is `v`: a listed bad pixel or rectangle, or a sample
    /// equal to a `FixBadPixelsConstant` value. (Oracles that apply no opcodes differ there.)
    pub fn fixes_pixel(&self, y: u32, x: u32, v: u16) -> bool {
        self.lists[..2].iter().flatten().any(|op| match op {
            Op::Bad(b, area) => {
                x >= area.x
                    && y >= area.y
                    && x - area.x < area.w
                    && y - area.y < area.h
                    && b.is_bad(y, x, v)
            }
            _ => false,
        })
    }

    fn has(&self, list: usize) -> bool {
        !self.lists[list].is_empty()
    }
    /// List 3 has work for the develop.
    pub(crate) fn has_stage3(&self) -> bool {
        self.has(2)
    }

    /// Parses the three lists (`None` = absent) of a `width` x `height` image with
    /// `ActiveArea` `active` and `cpp` samples per pixel. `None` when there is nothing at all.
    pub(crate) fn parse(
        lists: [Option<&[u8]>; 3],
        width: u32,
        height: u32,
        active: Rect,
        cpp: u32,
    ) -> Option<DngOpcodes> {
        if lists.iter().all(Option::is_none) {
            return None;
        }
        let mut out = DngOpcodes {
            lists: [Vec::new(), Vec::new(), Vec::new()],
            applied: Vec::new(),
            skipped: Vec::new(),
            trim: None,
            untrimmed_crop: active,
        };
        let full = Rect {
            x: 0,
            y: 0,
            w: width,
            h: height,
        };
        // Image bounds of stages 2 and 3 (the active area, trimmed by TrimBounds).
        let mut bounds23 = active;
        for (li, list) in lists.iter().enumerate() {
            let Some(b) = list else { continue };
            let mut rd = Rd(b, 0);
            let Some(count) = rd.u32() else {
                out.skipped.push("malformed opcode list");
                continue;
            };
            for _ in 0..count.min(4096) {
                let (Some(id), Some(_version), Some(flags), Some(size)) =
                    (rd.u32(), rd.u32(), rd.u32(), rd.u32())
                else {
                    out.skipped.push("malformed opcode list");
                    break;
                };
                let Some(params) = rd.1.checked_add(size as usize).and_then(|e| b.get(rd.1..e))
                else {
                    out.skipped.push("malformed opcode list");
                    break;
                };
                rd.1 += size as usize;
                let optional = flags & 1 != 0;
                let (bounds, origin) = if li == 0 {
                    (full, (0, 0))
                } else {
                    (bounds23, (active.x, active.y))
                };
                let mut p = Rd(params, 0);
                let op = match id {
                    1 | 2 if li == 2 => warp(&mut p, id == 2, bounds, cpp).map(Some),
                    3 => vignette(&mut p, bounds).map(Some),
                    4 if li < 2 => p
                        .u32()
                        .map(|c| Some(Op::Bad(BadPixels::Constant(c.min(65535) as u16), bounds))),
                    5 if li < 2 => bad_list(&mut p, origin, bounds).map(Some),
                    6 => p.rect().map(|t| {
                        let r = intersect(abs_rect(t, origin), bounds);
                        if r.w > 0 && r.h > 0 {
                            if li > 0 {
                                bounds23 = r;
                            }
                            out.trim = Some(out.trim.map_or(r, |o| intersect(o, r)));
                        }
                        None
                    }),
                    7..=13 => point(&mut p, id, origin, bounds).map(Some),
                    _ => None,
                };
                match op {
                    Some(op) => {
                        if !out.applied.contains(&name(id)) {
                            out.applied.push(name(id));
                        }
                        if let Some(op) = op {
                            out.lists[li].push(op);
                        }
                    }
                    None if !optional && !out.skipped.contains(&name(id)) => {
                        out.skipped.push(name(id))
                    }
                    None => {}
                }
            }
        }
        // Warps that move nothing are dropped.
        out.lists[2].retain(|o| !matches!(o, Op::Warp(w) if w.identity == [true; 3]));
        Some(out)
    }
}

fn warp(p: &mut Rd, fisheye: bool, bounds: Rect, cpp: u32) -> Option<Op> {
    let n = p.u32()? as usize;
    if !(n == 1 || (n == 3 && cpp == 3)) && !(n == 3 && cpp == 1) {
        return None;
    }
    let per = if fisheye { 4 } else { 6 };
    let mut sets = Vec::with_capacity(n);
    for _ in 0..n {
        let mut k = [0f64; 6];
        for v in k.iter_mut().take(per) {
            *v = p.f64()?;
        }
        sets.push(k);
    }
    let c = (p.f64()?, p.f64()?);
    let k: [[f64; 6]; 3] = std::array::from_fn(|i| sets[i.min(n - 1)]);
    let identity = k.map(|k| !fisheye && k == [1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    Some(Op::Warp(Warp {
        fisheye,
        k,
        identity,
        c,
        bounds,
    }))
}

/// Centre (absolute, continuous) and `1 / m^2`, `m` the distance from the centre to the
/// farthest corner of `bounds`.
fn radial(c: (f64, f64), bounds: Rect) -> ((f64, f64), f64) {
    let cx = bounds.x as f64 + c.0 * bounds.w as f64;
    let cy = bounds.y as f64 + c.1 * bounds.h as f64;
    let (x0, x1) = (bounds.x as f64, bounds.x as f64 + bounds.w as f64);
    let (y0, y1) = (bounds.y as f64, bounds.y as f64 + bounds.h as f64);
    let m2 = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
        .iter()
        .map(|&(x, y)| (x - cx) * (x - cx) + (y - cy) * (y - cy))
        .fold(0.0, f64::max);
    ((cx, cy), if m2 > 0.0 { 1.0 / m2 } else { 0.0 })
}

fn vignette(p: &mut Rd, bounds: Rect) -> Option<Op> {
    let mut k = [0f64; 5];
    for v in &mut k {
        *v = p.f64()?;
    }
    let (c, inv_m2) = radial((p.f64()?, p.f64()?), bounds);
    Some(Op::Point(PointOp {
        area: Area::whole(bounds),
        f: PointFn::Vignette { k, c, inv_m2 },
        bounds,
    }))
}

fn bad_list(p: &mut Rd, origin: (u32, u32), bounds: Rect) -> Option<Op> {
    let _phase = p.u32()?;
    let np = p.u32()? as usize;
    let nr = p.u32()? as usize;
    if np > 1 << 22 || nr > 1 << 20 {
        return None;
    }
    // Reserve no more than the remaining bytes can hold (8 per point, 16 per rect).
    let left = p.0.len().saturating_sub(p.1);
    let mut points = std::collections::HashSet::with_capacity(np.min(left / 8));
    for _ in 0..np {
        let (y, x) = (p.u32()?, p.u32()?);
        points.insert((origin.1.saturating_add(y), origin.0.saturating_add(x)));
    }
    let mut rects = Vec::with_capacity(nr.min(p.0.len().saturating_sub(p.1) / 16));
    for _ in 0..nr {
        rects.push(intersect(abs_rect(p.rect()?, origin), bounds));
    }
    Some(Op::Bad(BadPixels::List { points, rects }, bounds))
}

fn point(p: &mut Rd, id: u32, origin: (u32, u32), bounds: Rect) -> Option<Op> {
    let area = Area::read(p, origin, bounds)?;
    let f = match id {
        7 => {
            let n = p.u32()? as usize;
            if n == 0 || n > 65536 {
                return None;
            }
            PointFn::Table((0..n).map(|_| p.u16()).collect::<Option<_>>()?)
        }
        8 => {
            let deg = p.u32()? as usize;
            if deg > 8 {
                return None;
            }
            PointFn::Poly((0..=deg).map(|_| p.f64()).collect::<Option<_>>()?)
        }
        9 => PointFn::Gain(GainMap::read(p)?),
        10..=13 => {
            let n = p.u32()? as usize;
            if n > 1 << 20 {
                return None;
            }
            let v = (0..n).map(|_| p.f32()).collect::<Option<Vec<_>>>()?;
            let per_row = id == 10 || id == 12;
            if id <= 11 {
                PointFn::Delta { per_row, v }
            } else {
                PointFn::Scale { per_row, v }
            }
        }
        _ => return None,
    };
    if area.r.w == 0 || area.r.h == 0 {
        // Nothing to touch: valid, applied, no work.
        return Some(Op::Point(PointOp {
            area: Area { planes: 0, ..area },
            f,
            bounds,
        }));
    }
    Some(Op::Point(PointOp { area, f, bounds }))
}

// ---------------------------------------------------------------------------------------------
// Plain descriptions for other renderers (the GPU develop)

/// One opcode of a list as plain data (see [`DngOpcodes::list`]), for renderers that apply the
/// lists themselves. Rectangles are absolute sensor coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum OpcodeInfo {
    /// A per-pixel operation on the samples of `rect` (rows and columns on their pitches),
    /// planes `plane..plane + planes` (clipped to the samples per pixel).
    Point {
        rect: Rect,
        plane: u32,
        planes: u32,
        row_pitch: u32,
        col_pitch: u32,
        /// The image bounds gain-map positions are relative to.
        bounds: Rect,
        kind: PointKind,
    },
    /// Samples equal to `value` inside `bounds` take the inverse-distance mean of the good
    /// same-colour samples within two pixels.
    FixBadPixelsConstant { value: u16, bounds: Rect },
    /// Listed points (row, column) and rectangles, fixed the same way.
    FixBadPixelsList {
        points: Vec<(u32, u32)>,
        rects: Vec<Rect>,
        bounds: Rect,
    },
    /// `WarpRectilinear` (or `WarpFisheye`): per plane `kr0..kr3, kt0, kt1`; the optical
    /// centre relative to `bounds`.
    Warp {
        fisheye: bool,
        k: [[f64; 6]; 3],
        centre: (f64, f64),
        bounds: Rect,
    },
}

/// The function of an [`OpcodeInfo::Point`].
#[derive(Clone, Debug, PartialEq)]
pub enum PointKind {
    /// `MapTable`: index = sample (0..=65535 scale), clamped to the table.
    MapTable(Vec<u16>),
    /// `MapPolynomial`: coefficients from degree 0.
    MapPolynomial(Vec<f64>),
    /// `GainMap`: `gains[(v * points_h + h) * planes + plane]`, bilinear, positions relative
    /// to the bounds.
    GainMap {
        points_v: u32,
        points_h: u32,
        spacing_v: f64,
        spacing_h: f64,
        origin_v: f64,
        origin_h: f64,
        planes: u32,
        gains: Vec<f32>,
    },
    /// `FixVignetteRadial`: gain `1 + k0 r^2 + ... + k4 r^10` around `centre` (absolute),
    /// `r^2` scaled by `inv_m2`.
    Vignette {
        k: [f64; 5],
        centre: (f64, f64),
        inv_m2: f64,
    },
    DeltaPerRow(Vec<f32>),
    DeltaPerColumn(Vec<f32>),
    ScalePerRow(Vec<f32>),
    ScalePerColumn(Vec<f32>),
}

/// The opcode lists of a `width` x `height` image (no `ActiveArea`, `cpp` samples per pixel)
/// as plain data, as a reader of that DNG sees them: for writers that hand an image to a
/// renderer without writing the file first.
pub fn lists_info(
    lists: [Option<&[u8]>; 3],
    width: u32,
    height: u32,
    cpp: u32,
) -> [Vec<OpcodeInfo>; 3] {
    let full = Rect {
        x: 0,
        y: 0,
        w: width,
        h: height,
    };
    match DngOpcodes::parse(lists, width, height, full, cpp) {
        Some(ops) => [ops.list(0), ops.list(1), ops.list(2)],
        None => Default::default(),
    }
}

impl DngOpcodes {
    /// List `i` (0 = `OpcodeList1`, 1, 2) as plain data, in order.
    pub fn list(&self, i: usize) -> Vec<OpcodeInfo> {
        let Some(ops) = self.lists.get(i) else {
            return Vec::new();
        };
        ops.iter()
            .map(|op| match op {
                Op::Point(p) => OpcodeInfo::Point {
                    rect: p.area.r,
                    plane: p.area.plane,
                    planes: p.area.planes,
                    row_pitch: p.area.row_pitch,
                    col_pitch: p.area.col_pitch,
                    bounds: p.bounds,
                    kind: match &p.f {
                        PointFn::Table(t) => PointKind::MapTable(t.clone()),
                        PointFn::Poly(c) => PointKind::MapPolynomial(c.clone()),
                        PointFn::Gain(g) => PointKind::GainMap {
                            points_v: g.points_v as u32,
                            points_h: g.points_h as u32,
                            spacing_v: g.spacing_v,
                            spacing_h: g.spacing_h,
                            origin_v: g.origin_v,
                            origin_h: g.origin_h,
                            planes: g.planes as u32,
                            gains: g.gains.clone(),
                        },
                        PointFn::Vignette { k, c, inv_m2 } => PointKind::Vignette {
                            k: *k,
                            centre: *c,
                            inv_m2: *inv_m2,
                        },
                        PointFn::Delta { per_row: true, v } => PointKind::DeltaPerRow(v.clone()),
                        PointFn::Delta { per_row: false, v } => {
                            PointKind::DeltaPerColumn(v.clone())
                        }
                        PointFn::Scale { per_row: true, v } => PointKind::ScalePerRow(v.clone()),
                        PointFn::Scale { per_row: false, v } => {
                            PointKind::ScalePerColumn(v.clone())
                        }
                    },
                },
                Op::Bad(BadPixels::Constant(v), bounds) => OpcodeInfo::FixBadPixelsConstant {
                    value: *v,
                    bounds: *bounds,
                },
                Op::Bad(BadPixels::List { points, rects }, bounds) => {
                    let mut points: Vec<(u32, u32)> = points.iter().copied().collect();
                    points.sort_unstable();
                    OpcodeInfo::FixBadPixelsList {
                        points,
                        rects: rects.clone(),
                        bounds: *bounds,
                    }
                }
                Op::Warp(w) => OpcodeInfo::Warp {
                    fisheye: w.fisheye,
                    k: w.k,
                    centre: w.c,
                    bounds: w.bounds,
                },
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Lists 1 and 2: the sensor samples

impl DngOpcodes {
    /// Runs lists 1 and 2 over the samples `raw` holds (its window, sensor coordinates).
    /// After list 2 the samples are normalised: black 0, white 65535, no linearization table.
    pub(crate) fn apply_raw(&self, raw: &mut RawImage, parallel: bool) {
        if self.has(0) {
            run_list(&self.lists[0], raw, None, parallel);
        }
        if self.has(1) {
            let (black, white) = (raw.black, raw.white);
            let lin = raw.linearization.take().filter(|t| !t.is_empty());
            let norm = Norm {
                black,
                inv: black.map(|b| 1.0 / (white - b).max(1.0)),
                lin,
            };
            run_list(&self.lists[1], raw, Some(&norm), parallel);
            raw.black = [0.0; 4];
            raw.white = 65535.0;
        }
    }

    /// The sensor samples a region decode of output region `reg` (non-empty, inside the
    /// output) needs: what the develop reads for the source rectangle of list 3's warps, plus
    /// the neighbours the bad-pixel fixes of lists 1 and 2 read, within the sensor.
    pub(crate) fn sensor_window(
        &self,
        crop: Rect,
        width: u32,
        height: u32,
        reg: Rect,
        size: OutputSize,
        parallel: bool,
    ) -> Rect {
        let src = self.stage3_source(crop, size, reg, parallel);
        let w = crate::develop::needed_window(crop, src, size);
        let bad = self.lists[..2]
            .iter()
            .flatten()
            .filter(|o| matches!(o, Op::Bad(..)))
            .count() as u32;
        let m = bad * BAD_RADIUS;
        let x0 = w.x.saturating_sub(m);
        let y0 = w.y.saturating_sub(m);
        let x1 = (w.x as u64 + w.w as u64 + m as u64).min(width as u64) as u32;
        let y1 = (w.y as u64 + w.h as u64 + m as u64).min(height as u64) as u32;
        Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }
}

/// Stored samples → normalised list 2 samples.
struct Norm {
    black: [f32; 4],
    inv: [f32; 4],
    lin: Option<Vec<u16>>,
}

impl Norm {
    /// Sample `v` at sensor `(y, x)`, plane `p` of `cpp`.
    #[inline(always)]
    fn get(&self, v: u16, y: usize, x: usize, p: usize, cpp: usize) -> f32 {
        let k = if cpp == 1 {
            (y & 1) * 2 + (x & 1)
        } else {
            p.min(3)
        };
        let l = match &self.lin {
            Some(t) => t[(v as usize).min(t.len() - 1)] as f32,
            None => v as f32,
        };
        ((l - self.black[k]) * self.inv[k]).clamp(0.0, 1.0)
    }
}

/// Runs `ops` over `raw`'s samples: runs of point operations fused into one pass over the
/// rows, bad-pixel fixes on their own. `norm`: list 2 (the first pass normalises).
fn run_list(ops: &[Op], raw: &mut RawImage, mut norm: Option<&Norm>, parallel: bool) {
    let win = raw.window;
    let cpp = raw.cpp as usize;
    let mut i = 0;
    while i < ops.len() {
        let j = i + ops[i..]
            .iter()
            .take_while(|o| matches!(o, Op::Point(_)))
            .count();
        let points: Vec<&PointOp> = ops[i..j]
            .iter()
            .filter_map(|o| match o {
                Op::Point(p) => Some(p),
                _ => None,
            })
            .collect();
        if !points.is_empty() || norm.is_some() {
            point_pass(raw, &points, norm.take(), parallel);
        }
        if let Some(Op::Bad(b, bounds)) = ops.get(j) {
            fix_bad(
                &mut raw.data,
                win,
                cpp,
                raw.cfa.as_ref(),
                *bounds,
                b,
                parallel,
            );
        }
        i = j + 1;
    }
    if let Some(n) = norm {
        point_pass(raw, &[], Some(n), parallel);
    }
}

fn point_pass(raw: &mut RawImage, ops: &[&PointOp], norm: Option<&Norm>, parallel: bool) {
    let win = raw.window;
    let cpp = raw.cpp as usize;
    let row_len = win.w as usize * cpp;
    if row_len == 0 {
        return;
    }
    let row = |(r, d): (usize, &mut [u16]), buf: &mut (Vec<f32>, Vec<f32>)| {
        let (v, scratch) = buf;
        let y = win.y as usize + r;
        v.clear();
        match norm {
            Some(n) => v.extend(
                d.iter()
                    .enumerate()
                    .map(|(i, &s)| n.get(s, y, win.x as usize + i / cpp, i % cpp, cpp)),
            ),
            None => v.extend(d.iter().map(|&s| s as f32 * (1.0 / 65535.0))),
        }
        let line = Line {
            iy: y as i64,
            yc: y as f64 + 0.5,
            ix0: win.x as i64,
            xc0: win.x as f64 + 0.5,
            step: 1,
        };
        for op in ops {
            op.apply(line, v, cpp, scratch);
        }
        for (o, &s) in d.iter_mut().zip(v.iter()) {
            *o = (s * 65535.0 + 0.5) as u16;
        }
    };
    let data = &mut raw.data[..row_len * win.h as usize];
    if parallel {
        data.par_chunks_mut(row_len)
            .enumerate()
            .for_each_init(|| (Vec::new(), Vec::new()), |b, x| row(x, b));
    } else {
        let mut b = (Vec::new(), Vec::new());
        data.chunks_mut(row_len)
            .enumerate()
            .for_each(|x| row(x, &mut b));
    }
}

/// Replaces bad samples (inside `bounds` and the window) by the inverse-distance weighted mean
/// of the good same-colour samples within [`BAD_RADIUS`] (CFA), or of the same plane
/// (linear RGB).
fn fix_bad(
    data: &mut [u16],
    win: Rect,
    cpp: usize,
    cfa: Option<&Cfa>,
    bounds: Rect,
    bad: &BadPixels,
    parallel: bool,
) {
    let area = intersect(win, bounds);
    if area.w == 0 || area.h == 0 {
        return;
    }
    let ww = win.w as usize;
    let at = |y: u32, x: u32, p: usize| -> u16 {
        data[((y - win.y) as usize * ww + (x - win.x) as usize) * cpp + p]
    };
    // Candidates: (y, x, plane).
    let cand: Vec<(u32, u32, usize)> = match bad {
        BadPixels::Constant(c) => {
            let scan = |y: u32| -> Vec<(u32, u32, usize)> {
                let mut v = Vec::new();
                for x in area.x..area.x + area.w {
                    for p in 0..cpp {
                        if at(y, x, p) == *c {
                            v.push((y, x, p));
                        }
                    }
                }
                v
            };
            let rows = area.y..area.y + area.h;
            if parallel {
                rows.into_par_iter().flat_map_iter(scan).collect()
            } else {
                rows.flat_map(scan).collect()
            }
        }
        BadPixels::List { points, rects } => {
            let mut v: Vec<(u32, u32, usize)> = points
                .iter()
                .map(|&(y, x)| (y, x))
                .chain(rects.iter().flat_map(|r| {
                    let r = intersect(*r, area);
                    (r.y..r.y + r.h).flat_map(move |y| (r.x..r.x + r.w).map(move |x| (y, x)))
                }))
                .filter(|&(y, x)| {
                    x >= area.x && y >= area.y && x - area.x < area.w && y - area.y < area.h
                })
                .flat_map(|(y, x)| (0..cpp).map(move |p| (y, x, p)))
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        }
    };
    let rad = BAD_RADIUS as i64;
    let fixed: Vec<Option<u16>> = cand
        .iter()
        .map(|&(y, x, p)| {
            let col = cfa.map(|c| c.color(y, x));
            let (mut sw, mut sv) = (0f32, 0f32);
            for dy in -rad..=rad {
                for dx in -rad..=rad {
                    if dy == 0 && dx == 0 {
                        continue;
                    }
                    let (ny, nx) = (y as i64 + dy, x as i64 + dx);
                    if ny < area.y as i64
                        || nx < area.x as i64
                        || ny >= (area.y + area.h) as i64
                        || nx >= (area.x + area.w) as i64
                    {
                        continue;
                    }
                    let (ny, nx) = (ny as u32, nx as u32);
                    if cfa.is_some_and(|c| Some(c.color(ny, nx)) != col) {
                        continue;
                    }
                    let v = at(ny, nx, p);
                    if bad.is_bad(ny, nx, v) {
                        continue;
                    }
                    let w = 1.0 / (dy * dy + dx * dx) as f32;
                    sw += w;
                    sv += w * v as f32;
                }
            }
            (sw > 0.0).then(|| (sv / sw + 0.5) as u16)
        })
        .collect();
    for (&(y, x, p), f) in cand.iter().zip(fixed) {
        if let Some(v) = f {
            data[((y - win.y) as usize * ww + (x - win.x) as usize) * cpp + p] = v;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// List 3: the develop

/// Output geometry for list 3: output pixel `(j, i)` has sensor centre
/// `(x0 + j * step, y0 + i * step)`; output is `ow` x `oh`.
#[derive(Clone, Copy)]
struct Geo {
    x0: f64,
    y0: f64,
    step: f64,
    /// Sensor column / row of output pixel 0 (integer positions for area opcodes).
    ix: i64,
    iy: i64,
    ow: usize,
    oh: usize,
}

impl Geo {
    fn new(crop: Rect, size: OutputSize) -> Geo {
        let (ow, oh) = crate::develop::output_size(crop, size);
        let (step, half) = match size {
            OutputSize::Full => (1.0, 0.5),
            OutputSize::Half => (2.0, 1.0),
        };
        Geo {
            x0: crop.x as f64 + half,
            y0: crop.y as f64 + half,
            step,
            ix: crop.x as i64,
            iy: crop.y as i64,
            ow: ow as usize,
            oh: oh as usize,
        }
    }
    fn line(&self, i: usize, j0: usize) -> Line {
        let s = self.step as i64;
        Line {
            iy: self.iy + i as i64 * s,
            yc: self.y0 + i as f64 * self.step,
            ix0: self.ix + j0 as i64 * s,
            xc0: self.x0 + j0 as f64 * self.step,
            step: s,
        }
    }
}

impl OpcodeInfo {
    /// For a warp: where output pixel (`x`, `y`) of a develop of `crop` at `size` reads plane
    /// `plane` from, in the same output pixels (pixel centres at `i + 0.5` read as `i`).
    /// This is the CPU develop's own mapping, for checking other implementations against it.
    /// `None` for other opcodes.
    pub fn warp_source(
        &self,
        crop: Rect,
        size: OutputSize,
        x: usize,
        y: usize,
        plane: usize,
    ) -> Option<(f32, f32)> {
        let OpcodeInfo::Warp {
            fisheye,
            k,
            centre,
            bounds,
        } = self
        else {
            return None;
        };
        let w = Warp {
            fisheye: *fisheye,
            k: *k,
            identity: [false; 3],
            c: *centre,
            bounds: *bounds,
        };
        let (mut sx, mut sy) = ([0f32], [0f32]);
        WarpGeo::new(&w, Geo::new(crop, size)).src(plane.min(2), y, x, &mut sx, &mut sy);
        Some((sx[0], sy[0]))
    }
}

/// Integer tap base and fraction of source coordinate `s` along an axis of `n` pixels.
#[inline(always)]
fn tap(s: f32, n: usize) -> (isize, f32) {
    let s = if s.is_finite() {
        s.clamp(-4.0, n as f32 + 4.0)
    } else {
        0.0
    };
    let f = s.floor();
    (f as isize, s - f)
}

/// Catmull-Rom weights for fraction `t`.
#[inline(always)]
fn cubic(t: f32) -> [f32; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        -0.5 * t3 + t2 - 0.5 * t,
        1.5 * t3 - 2.5 * t2 + 1.0,
        -1.5 * t3 + 2.0 * t2 + 0.5 * t,
        0.5 * t3 - 0.5 * t2,
    ]
}

/// A rectangle of camera RGB (output coordinates).
struct Buf {
    rect: Rect,
    data: Vec<u16>,
}

/// Warp evaluated for one output geometry.
struct WarpGeo<'a> {
    w: &'a Warp,
    g: Geo,
    c: (f64, f64),
    m: f64,
    inv_m: f64,
}

impl<'a> WarpGeo<'a> {
    fn new(w: &'a Warp, g: Geo) -> WarpGeo<'a> {
        let (c, inv_m2) = radial(w.c, w.bounds);
        let m = if inv_m2 > 0.0 {
            inv_m2.sqrt().recip()
        } else {
            1.0
        };
        WarpGeo {
            w,
            g,
            c,
            m,
            inv_m: 1.0 / m,
        }
    }

    /// Source coordinates (output pixels) of plane `p` for output row `i`, columns
    /// `j0..j0 + sx.len()`.
    fn src(&self, p: usize, i: usize, j0: usize, sx: &mut [f32], sy: &mut [f32]) {
        let g = &self.g;
        let k = &self.w.k[p];
        let dy = (g.y0 + i as f64 * g.step - self.c.1) * self.inv_m;
        let inv_step = 1.0 / g.step;
        for (n, (ox, oy)) in sx.iter_mut().zip(sy.iter_mut()).enumerate() {
            let dx = (g.x0 + (j0 + n) as f64 * g.step - self.c.0) * self.inv_m;
            let r2 = dx * dx + dy * dy;
            let (fx, fy) = if self.w.fisheye {
                let r = r2.sqrt();
                if r < 1e-12 {
                    (k[0] * dx, k[0] * dy)
                } else {
                    let t = r.atan();
                    let t2 = t * t;
                    let rd = t * (k[0] + t2 * (k[1] + t2 * (k[2] + t2 * k[3])));
                    let s = rd / r;
                    (s * dx, s * dy)
                }
            } else {
                let f = k[0] + r2 * (k[1] + r2 * (k[2] + r2 * k[3]));
                let dxy = 2.0 * dx * dy;
                (
                    f * dx + k[4] * dxy + k[5] * (r2 + 2.0 * dx * dx),
                    f * dy + k[5] * dxy + k[4] * (r2 + 2.0 * dy * dy),
                )
            };
            let (x, y) = (
                ((self.c.0 + self.m * fx - g.x0) * inv_step) as f32,
                ((self.c.1 + self.m * fy - g.y0) * inv_step) as f32,
            );
            // NaN reads like 0 in `tap`; mapping it here keeps the footprint's min / max exact.
            *ox = if x.is_nan() { 0.0 } else { x };
            *oy = if y.is_nan() { 0.0 } else { y };
        }
    }

    /// Source coordinates of the warp over output rectangle `r`, computed once for the
    /// footprint and the resampling.
    fn coords(&self, r: Rect) -> Coords {
        let n = r.w as usize;
        let len = n * r.h as usize;
        let mut planes = Vec::new();
        let mut of = [None; 3];
        for p in 0..3 {
            if self.w.identity[p] || len == 0 {
                continue;
            }
            if let Some(q) = (0..p).find(|&q| of[q].is_some() && self.w.k[q] == self.w.k[p]) {
                of[p] = of[q];
                continue;
            }
            let (mut sx, mut sy) = (vec![0f32; len], vec![0f32; len]);
            for (ri, (x, y)) in sx
                .chunks_exact_mut(n)
                .zip(sy.chunks_exact_mut(n))
                .enumerate()
            {
                self.src(p, r.y as usize + ri, r.x as usize, x, y);
            }
            of[p] = Some(planes.len());
            planes.push((sx, sy));
        }
        Coords { r, planes, of }
    }

    /// The output pixels the warp of `c.r` reads (taps clamped to the output).
    fn footprint(&self, c: &Coords) -> Rect {
        let (ow, oh) = (self.g.ow, self.g.oh);
        let r = c.r;
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        if c.of.contains(&None) {
            x0 = r.x as usize;
            y0 = r.y as usize;
            x1 = (r.x + r.w).max(1) as usize - 1;
            y1 = (r.y + r.h).max(1) as usize - 1;
        }
        let clamp = |v: isize, n: usize| v.clamp(0, n as isize - 1) as usize;
        // `tap` is monotonic, so the extreme coordinates give the extreme taps.
        let range = |v: &[f32]| {
            v.iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &s| {
                    (a.min(s), b.max(s))
                })
        };
        for (sx, sy) in &c.planes {
            let ((ax, bx), (ay, by)) = (range(sx), range(sy));
            x0 = x0.min(clamp(tap(ax, ow).0 - 1, ow));
            x1 = x1.max(clamp(tap(bx, ow).0 + 2, ow));
            y0 = y0.min(clamp(tap(ay, oh).0 - 1, oh));
            y1 = y1.max(clamp(tap(by, oh).0 + 2, oh));
        }
        if x0 > x1 || y0 > y1 {
            return r;
        }
        Rect {
            x: x0 as u32,
            y: y0 as u32,
            w: (x1 + 1 - x0) as u32,
            h: (y1 + 1 - y0) as u32,
        }
    }

    /// Warps `src` (which holds the footprint of `c.r`) into `c.r`.
    fn apply(&self, c: &Coords, src: &Buf) -> Buf {
        let (ow, oh) = (self.g.ow as isize, self.g.oh as isize);
        let r = c.r;
        let n = r.w as usize;
        let mut out = vec![0u16; n * r.h as usize * 3];
        let s = src.rect;
        let (sw, sh) = (s.w as usize, s.h as usize);
        let (sx0, sy0) = (s.x as isize, s.y as isize);
        // Clamped to the output, then into the source buffer (a no-op for a correct
        // footprint; keeps a wrong one from reading out of bounds).
        let cx = |v: isize| (v.clamp(0, ow - 1).clamp(sx0, sx0 + sw as isize - 1) - sx0) as usize;
        let cy = |v: isize| (v.clamp(0, oh - 1).clamp(sy0, sy0 + sh as isize - 1) - sy0) as usize;
        // Tap bases whose 4 x 4 taps need neither clamp (same values, no index arithmetic).
        let (fx0, fx1) = (sx0.max(0) + 1, (sx0 + sw as isize).min(ow) - 3);
        let (fy0, fy1) = (sy0.max(0) + 1, (sy0 + sh as isize).min(oh) - 3);
        let mut plane = Vec::new();
        for p in 0..3 {
            let Some(q) = c.of[p] else {
                for (ri, row) in out.chunks_exact_mut(n * 3).enumerate() {
                    let base = (cy(r.y as isize + ri as isize) * sw + cx(r.x as isize)) * 3 + p;
                    for (j, o) in row.chunks_exact_mut(3).enumerate() {
                        o[p] = src.data[base + j * 3];
                    }
                }
                continue;
            };
            plane.clear();
            plane.extend(src.data[p..].iter().step_by(3).map(|&v| v as f32));
            let (xs, ys) = &c.planes[q];
            for (ri, row) in out.chunks_exact_mut(n * 3).enumerate() {
                let (xr, yr) = (&xs[ri * n..][..n], &ys[ri * n..][..n]);
                for ((o, &sxv), &syv) in row.chunks_exact_mut(3).zip(xr).zip(yr) {
                    let (bx, tx) = tap(sxv, ow as usize);
                    let (by, ty) = tap(syv, oh as usize);
                    let wx = cubic(tx);
                    let wy = cubic(ty);
                    let mut acc = 0f32;
                    if bx >= fx0 && bx <= fx1 && by >= fy0 && by <= fy1 {
                        let (x, y) = ((bx - 1 - sx0) as usize, (by - 1 - sy0) as usize);
                        for (dy, &wyv) in wy.iter().enumerate() {
                            let t = &plane[(y + dy) * sw + x..][..4];
                            let h = wx[0] * t[0] + wx[1] * t[1] + wx[2] * t[2] + wx[3] * t[3];
                            acc += wyv * h;
                        }
                    } else {
                        let xi = [cx(bx - 1), cx(bx), cx(bx + 1), cx(bx + 2)];
                        for (dy, &wyv) in wy.iter().enumerate() {
                            let t = &plane[cy(by - 1 + dy as isize) * sw..];
                            let h = wx[0] * t[xi[0]]
                                + wx[1] * t[xi[1]]
                                + wx[2] * t[xi[2]]
                                + wx[3] * t[xi[3]];
                            acc += wyv * h;
                        }
                    }
                    o[p] = (acc + 0.5).clamp(0.0, 65535.0) as u16;
                }
            }
        }
        Buf { rect: r, data: out }
    }
}

/// Source coordinates of a warp over output rectangle `r` (row-major per distinct plane;
/// `of[p]` is `None` for an identity plane).
struct Coords {
    r: Rect,
    planes: Vec<(Vec<f32>, Vec<f32>)>,
    of: [Option<usize>; 3],
}

/// Output tile of the list 3 develop (rows x columns).
const TILE_H: usize = 64;
const TILE_W: usize = 256;

impl DngOpcodes {
    /// The output rectangle list 3 reads for output rectangle `r` (through its warps,
    /// last to first).
    /// With `keep`, also the warps' source coordinates (by opcode index).
    fn sources(&self, g: Geo, r: Rect, keep: bool) -> (Vec<Rect>, Vec<Option<Coords>>) {
        let ops = &self.lists[2];
        let mut rects = vec![r; ops.len() + 1];
        let mut coords: Vec<Option<Coords>> = ops.iter().map(|_| None).collect();
        for (i, op) in ops.iter().enumerate().rev() {
            rects[i] = match op {
                Op::Warp(w) => {
                    let wg = WarpGeo::new(w, g);
                    let c = wg.coords(rects[i + 1]);
                    let f = wg.footprint(&c);
                    if keep {
                        coords[i] = Some(c);
                    }
                    f
                }
                _ => rects[i + 1],
            };
        }
        (rects, coords)
    }

    /// The output rectangle list 3 reads for output region `reg` (the union over the develop's
    /// tiles; `reg` itself without warps).
    fn stage3_source(&self, crop: Rect, size: OutputSize, reg: Rect, parallel: bool) -> Rect {
        if !self.lists[2].iter().any(|o| matches!(o, Op::Warp(_))) {
            return reg;
        }
        let g = Geo::new(crop, size);
        let tiles = tiles(reg);
        let f = |t: &Rect| self.sources(g, *t, false).0[0];
        let rects: Vec<Rect> = if parallel {
            tiles.par_iter().map(f).collect()
        } else {
            tiles.iter().map(f).collect()
        };
        rects
            .into_iter()
            .reduce(|a, b| {
                let x0 = a.x.min(b.x);
                let y0 = a.y.min(b.y);
                let x1 = (a.x + a.w).max(b.x + b.w);
                let y1 = (a.y + a.h).max(b.y + b.h);
                Rect {
                    x: x0,
                    y: y0,
                    w: x1 - x0,
                    h: y1 - y0,
                }
            })
            .unwrap_or(reg)
    }

    /// Develops output region `reg` through list 3: for each tile, `base` develops the source
    /// rectangle to camera RGB (white balanced by `mul`, no colour matrix), the opcodes run,
    /// and `finish` converts a row of camera RGB in place.
    pub(crate) fn develop_stage3(
        &self,
        crop: Rect,
        reg: Rect,
        mul: [f32; 3],
        opts: &DecodeOptions,
        base: &(dyn Fn(Rect) -> Result<Vec<u16>> + Sync),
        finish: &(dyn Fn(&mut [u16]) + Sync),
    ) -> Result<Vec<u16>> {
        let g = Geo::new(crop, opts.size);
        let ow = reg.w as usize;
        let mut out = vec![0u16; ow * reg.h as usize * 3];
        let cancel = || opts.cancel.is_some_and(|c| c.load(Ordering::Relaxed));
        let band = |(bi, chunk): (usize, &mut [u16])| -> Result<()> {
            let rows = chunk.len() / (ow * 3);
            let mut scratch = Vec::new();
            let mut line = Vec::new();
            for tx in (0..ow).step_by(TILE_W) {
                if cancel() {
                    return Err(RawError::Cancelled);
                }
                let t = Rect {
                    x: reg.x + tx as u32,
                    y: reg.y + (bi * TILE_H) as u32,
                    w: (ow - tx).min(TILE_W) as u32,
                    h: rows as u32,
                };
                let (rects, coords) = self.sources(g, t, true);
                let mut buf = Buf {
                    rect: rects[0],
                    data: base(rects[0])?,
                };
                for (i, op) in self.lists[2].iter().enumerate() {
                    match op {
                        Op::Warp(w) => {
                            if let Some(c) = &coords[i] {
                                buf = WarpGeo::new(w, g).apply(c, &buf);
                            }
                        }
                        Op::Point(p) => point_rgb(p, g, &mut buf, mul, &mut line, &mut scratch),
                        Op::Bad(..) => {}
                    }
                }
                let tw = t.w as usize;
                for r in 0..rows {
                    let src = &mut buf.data[r * tw * 3..(r + 1) * tw * 3];
                    finish(src);
                    chunk[(r * ow + tx) * 3..(r * ow + tx + tw) * 3].copy_from_slice(src);
                }
            }
            Ok(())
        };
        let chunk = TILE_H * ow * 3;
        if opts.parallel {
            out.par_chunks_mut(chunk).enumerate().try_for_each(band)?;
        } else {
            out.chunks_mut(chunk).enumerate().try_for_each(band)?;
        }
        Ok(out)
    }
}

/// The develop tiles of `reg`.
fn tiles(reg: Rect) -> Vec<Rect> {
    let mut v = Vec::new();
    for ty in (0..reg.h as usize).step_by(TILE_H) {
        for tx in (0..reg.w as usize).step_by(TILE_W) {
            v.push(Rect {
                x: reg.x + tx as u32,
                y: reg.y + ty as u32,
                w: (reg.w as usize - tx).min(TILE_W) as u32,
                h: (reg.h as usize - ty).min(TILE_H) as u32,
            });
        }
    }
    v
}

/// A point opcode on white-balanced camera RGB (`v = x * mul * 65535`).
fn point_rgb(
    p: &PointOp,
    g: Geo,
    buf: &mut Buf,
    mul: [f32; 3],
    line: &mut Vec<f32>,
    scratch: &mut Vec<f32>,
) {
    let r = buf.rect;
    let w = r.w as usize;
    let to = mul.map(|m| m * 65535.0);
    let from = to.map(|t| 1.0 / t);
    for (ri, row) in buf.data.chunks_exact_mut(w * 3).enumerate() {
        line.clear();
        line.extend(row.iter().enumerate().map(|(i, &v)| v as f32 * from[i % 3]));
        p.apply(g.line(r.y as usize + ri, r.x as usize), line, 3, scratch);
        for (i, (o, &x)) in row.iter_mut().zip(line.iter()).enumerate() {
            *o = (x * to[i % 3] + 0.5).clamp(0.0, 65535.0) as u16;
        }
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A bad-pixel list claiming 4M points and 1M rects with no data behind them fails
    /// without reserving for them first.
    #[test]
    fn bad_list_counts_without_data() {
        let mut b = Vec::new();
        for v in [0u32, 1 << 22, 1 << 20] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        let mut p = Rd(&b, 0);
        let bounds = Rect {
            x: 0,
            y: 0,
            w: 16,
            h: 16,
        };
        assert!(bad_list(&mut p, (0, 0), bounds).is_none());
    }
}
