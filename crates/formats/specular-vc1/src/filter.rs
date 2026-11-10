//! Overlap smoothing (§8.5) and the in-loop deblocking filter (§8.6).

use crate::mc::Plane;

/// The overlap filter of four samples straddling an edge (§8.5): x0, x1 | x2, x3.
#[inline]
fn overlap4(x: [i32; 4], r0: i32, r1: i32) -> [i32; 4] {
    [
        (7 * x[0] + x[3] + r0) >> 3,
        (-x[0] + 7 * x[1] + x[2] + x[3] + r1) >> 3,
        (x[0] + x[1] + 7 * x[2] - x[3] + r0) >> 3,
        (x[0] + 7 * x[3] + r1) >> 3,
    ]
}

/// Smooths the vertical edge at column `x` over rows `y..y + 8` of a 16-bit plane.
pub fn overlap_v(p: &mut [i16], stride: usize, x: usize, y: usize) {
    for i in 0..8 {
        let (r0, r1) = if i % 2 == 0 { (4, 3) } else { (3, 4) };
        let row = &mut p[(y + i) * stride + x - 2..];
        let v = overlap4(
            [row[0].into(), row[1].into(), row[2].into(), row[3].into()],
            r0,
            r1,
        );
        for k in 0..4 {
            row[k] = v[k] as i16;
        }
    }
}

/// Smooths the horizontal edge at row `y` over columns `x..x + 8`.
pub fn overlap_h(p: &mut [i16], stride: usize, x: usize, y: usize) {
    for i in 0..8 {
        let (r0, r1) = if i % 2 == 0 { (4, 3) } else { (3, 4) };
        let at = |k: usize| (y + k - 2) * stride + x + i;
        let v = overlap4(
            [
                p[at(0)].into(),
                p[at(1)].into(),
                p[at(2)].into(),
                p[at(3)].into(),
            ],
            r0,
            r1,
        );
        for k in 0..4 {
            p[at(k)] = v[k] as i16;
        }
    }
}

/// Filters one pixel pair (Figures 82, 83): `s` steps across the edge, `p4` is the last
/// sample before it. Returns whether the pair was filtered by the clip rule (for the third
/// pair: whether the other three are filtered).
#[inline]
fn filter_pair(d: &mut [u8], p4: usize, s: isize, pquant: i32) -> bool {
    let at = |k: isize| p4 as isize + (k - 4) * s;
    let px = |d: &[u8], k: isize| i32::from(d[at(k) as usize]);
    let a0 = (2 * (px(d, 3) - px(d, 6)) - 5 * (px(d, 4) - px(d, 5)) + 4) >> 3;
    if a0.abs() >= pquant {
        return false;
    }
    let a1 = (2 * (px(d, 1) - px(d, 4)) - 5 * (px(d, 2) - px(d, 3)) + 4) >> 3;
    let a2 = (2 * (px(d, 5) - px(d, 8)) - 5 * (px(d, 6) - px(d, 7)) + 4) >> 3;
    let a3 = a1.abs().min(a2.abs());
    if a3 >= a0.abs() {
        return false;
    }
    let mut dd = 5 * (a0.signum() * a3 - a0) / 8;
    let clip = (px(d, 4) - px(d, 5)) / 2;
    if clip == 0 {
        return false;
    }
    if clip > 0 {
        dd = dd.clamp(0, clip);
    } else {
        dd = dd.clamp(clip, 0);
    }
    let p4v = px(d, 4) - dd;
    let p5v = px(d, 5) + dd;
    d[at(4) as usize] = p4v as u8;
    d[at(5) as usize] = p5v as u8;
    true
}

/// Filters a 4-sample segment of an edge. `p4` indexes the first segment sample just before
/// the edge, `s` steps across the edge, `along` steps along it.
#[inline]
pub fn segment(d: &mut [u8], p4: usize, s: isize, along: isize, pquant: i32) {
    let third = (p4 as isize + 2 * along) as usize;
    if filter_pair(d, third, s, pquant) {
        for k in [0isize, 1, 3] {
            filter_pair(d, (p4 as isize + k * along) as usize, s, pquant);
        }
    }
}

/// Filters the horizontal edge at row `y` (between rows y-1 and y), columns `x..x + 4`.
pub fn hseg(p: &mut Plane, x: usize, y: usize, pquant: i32) {
    let st = p.stride;
    segment(&mut p.data, (y - 1) * st + x, st as isize, 1, pquant);
}

/// Filters the vertical edge at column `x`, rows `y..y + 4`.
pub fn vseg(p: &mut Plane, x: usize, y: usize, pquant: i32) {
    let st = p.stride;
    segment(&mut p.data, y * st + x - 1, 1, st as isize, pquant);
}
