//! The front end's per-sample linearisation on the CPU (DNG opcode list 1 point ops, the
//! LinearizationTable, black and white, list 2 point ops): the same f32 arithmetic as the
//! `prep` kernel, for host-side measurements (raw CA) and the CPU reference.

use specular_raw::SensorData;
use specular_raw::sensor::{OpcodeInfo, PointKind};

pub(crate) fn points(v: &[OpcodeInfo]) -> Vec<&OpcodeInfo> {
    v.iter()
        .filter(|o| matches!(o, OpcodeInfo::Point { .. }))
        .collect()
}

fn gm_coord(rel: f32, origin: f32, spacing: f32, points: u32) -> (usize, usize, f32) {
    if points < 2 {
        return (0, 0, 0.0);
    }
    let m = ((rel - origin) / spacing).clamp(0.0, (points - 1) as f32);
    let i = m.floor().min((points - 1) as f32);
    (
        i as usize,
        (i + 1.0).min((points - 1) as f32) as usize,
        m - i,
    )
}

#[allow(clippy::too_many_arguments)]
fn point_op(o: &OpcodeInfo, v: f32, ix: u32, iy: u32, xc: f32, yc: f32, p: u32, cpp: u32) -> f32 {
    let OpcodeInfo::Point {
        rect,
        plane,
        planes,
        row_pitch,
        col_pitch,
        bounds,
        kind,
    } = o
    else {
        return v;
    };
    if !(ix >= rect.x && iy >= rect.y && ix - rect.x < rect.w && iy - rect.y < rect.h) {
        return v;
    }
    if !(iy - rect.y).is_multiple_of(*row_pitch) || !(ix - rect.x).is_multiple_of(*col_pitch) {
        return v;
    }
    if p < (*plane).min(cpp) || p >= (plane + (*planes).min(64)).min(cpp) {
        return v;
    }
    match kind {
        PointKind::MapTable(t) => {
            let i = ((v * 65535.0 + 0.5).max(0.0) as usize).min(t.len() - 1);
            f32::from(t[i]) / 65535.0
        }
        PointKind::MapPolynomial(c) => {
            let mut y = 0f32;
            for &k in c.iter().rev() {
                y = y * v + k as f32;
            }
            y
        }
        PointKind::GainMap {
            points_v,
            points_h,
            spacing_v,
            spacing_h,
            origin_v,
            origin_h,
            planes: gp,
            gains,
        } => {
            let bw = bounds.w.max(1) as f32;
            let bh = bounds.h.max(1) as f32;
            let (i0, i1, fy) = gm_coord(
                (yc - bounds.y as f32) / bh,
                *origin_v as f32,
                *spacing_v as f32,
                *points_v,
            );
            let (j0, j1, fx) = gm_coord(
                (xc - bounds.x as f32) / bw,
                *origin_h as f32,
                *spacing_h as f32,
                *points_h,
            );
            let (ph, gp) = (*points_h as usize, *gp as usize);
            let mp = ((p - plane) as usize).min(gp - 1);
            let g = |i: usize, j: usize| gains[(i * ph + j) * gp + mp];
            let l = g(i0, j0) + (g(i1, j0) - g(i0, j0)) * fy;
            let r = g(i0, j1) + (g(i1, j1) - g(i0, j1)) * fy;
            v * (l + (r - l) * fx)
        }
        PointKind::Vignette { k, centre, inv_m2 } => {
            let k = k.map(|v| v as f32);
            let dx = xc - centre.0 as f32;
            let dy = yc - centre.1 as f32;
            let r2 = (dx * dx + dy * dy) * *inv_m2 as f32;
            let g = 1.0 + r2 * (k[0] + r2 * (k[1] + r2 * (k[2] + r2 * (k[3] + r2 * k[4]))));
            v * g
        }
        PointKind::DeltaPerRow(d)
        | PointKind::DeltaPerColumn(d)
        | PointKind::ScalePerRow(d)
        | PointKind::ScalePerColumn(d) => {
            let row = matches!(kind, PointKind::DeltaPerRow(_) | PointKind::ScalePerRow(_));
            let i = if row {
                (iy - rect.y) / row_pitch
            } else {
                (ix - rect.x) / col_pitch
            } as usize;
            let Some(&d) = d.get(i) else { return v };
            if matches!(
                kind,
                PointKind::DeltaPerRow(_) | PointKind::DeltaPerColumn(_)
            ) {
                v + d
            } else {
                v * d
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_list(
    l: &[&OpcodeInfo],
    v0: f32,
    ix: u32,
    iy: u32,
    xc: f32,
    yc: f32,
    p: u32,
    cpp: u32,
) -> f32 {
    l.iter()
        .fold(v0, |v, o| point_op(o, v, ix, iy, xc, yc, p, cpp))
}

/// Stored samples to linear values (black 0, white 1, no white balance) as the front end's
/// `prep` makes them, without bad-pixel repair.
pub(crate) struct Linear<'a> {
    s: &'a SensorData,
    l1: Vec<&'a OpcodeInfo>,
    l2: Vec<&'a OpcodeInfo>,
}

impl<'a> Linear<'a> {
    pub(crate) fn new(s: &'a SensorData) -> Linear<'a> {
        let m = &s.meta;
        Linear {
            s,
            l1: points(&m.opcodes[0]),
            l2: points(&m.opcodes[1]),
        }
    }

    /// The stored value of channel `c` at stored position (sx, sy).
    pub(crate) fn stored(&self, sx: u32, sy: u32, c: u32) -> f32 {
        let s = self.s;
        if let Some(f) = &s.float {
            let cpp = s.cpp as usize;
            return f[(sy as usize * s.width as usize + sx as usize) * cpp + c as usize];
        }
        f32::from(s.sample(sx, sy, c))
    }

    /// The linear value of a stored value `v` of channel `c` at stored position (sx, sy).
    pub(crate) fn value(&self, v: f32, sx: u32, sy: u32, c: u32) -> f32 {
        let s = self.s;
        let m = &s.meta;
        let (xc, yc) = (sx as f32 + 0.5, sy as f32 + 0.5);
        let mut v = v;
        if !self.l1.is_empty() {
            v = run_list(&self.l1, v / 65535.0, sx, sy, xc, yc, c, s.cpp) * 65535.0;
        }
        if let Some(t) = &m.linearization
            && !t.is_empty()
        {
            let i = ((v + 0.5).clamp(0.0, 65535.0) as usize).min(t.len() - 1);
            v = f32::from(t[i]);
        }
        let b = m.black.at(sx, sy, c);
        v = (v - b) / (m.white - b).max(1.0);
        if !self.l2.is_empty() {
            v = run_list(&self.l2, v, sx, sy, xc, yc, c, s.cpp);
        }
        v
    }
}
