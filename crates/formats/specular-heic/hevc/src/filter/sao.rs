//! Sample adaptive offset, ITU-T H.265 clause 8.7.3.
//!
//! SAO reads the deblocked picture and writes a second picture, so the filter
//! works from a snapshot of every plane taken before the first sample changes.

use super::deblock::clip3;
use super::{FilterInfo, SaoCtb};
use crate::picture::{Picture, Plane, PlaneMut, flags};
use alloc::vec::Vec;

/// Neighbour offsets of each `SaoEoClass`, clause 8.7.3.2.
const EO: [[(isize, isize); 2]; 4] = [
    [(-1, 0), (1, 0)],
    [(0, -1), (0, 1)],
    [(-1, -1), (1, 1)],
    [(1, -1), (-1, 1)],
];

/// `SaoTypeIdx` selecting band offset.
const BAND: u8 = 1;
/// `SaoTypeIdx` selecting edge offset.
const EDGE: u8 = 2;

/// Offset to add to the sample at `(px, py)`, or `None` to leave it alone.
///
/// `sw` and `sh` scale component coordinates back to luma coordinates so that
/// the CTB of a neighbour can be found; `addr` is the CTB being filtered.
#[allow(clippy::too_many_arguments)]
fn edge_offset(
    src: &Plane,
    info: &FilterInfo,
    s: &SaoCtb,
    addr: usize,
    px: usize,
    py: usize,
    sw: usize,
    sh: usize,
) -> Option<i32> {
    let rec = src.at(px, py) as i32;
    let mut idx = 2i32;
    for (dx, dy) in EO[(s.eo_class & 3) as usize] {
        let nx = px as isize + dx;
        let ny = py as isize + dy;
        if nx < 0 || ny < 0 || nx as usize >= src.width || ny as usize >= src.height {
            return None;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        if !info.may_cross(info.ctb_of(nx * sw, ny * sh), addr) {
            return None;
        }
        idx += (rec - src.at(nx, ny) as i32).signum();
    }
    let idx = match idx {
        2 => 0,
        0 => 1,
        1 => 2,
        v => v,
    };
    if idx == 0 {
        return Some(0);
    }
    s.offsets.get(idx as usize - 1).map(|v| *v as i32)
}

/// The component-sample rectangle `x0..x1` by `y0..y1` of CTB `addr`.
#[derive(Clone, Copy)]
struct Rect {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

/// Applies sample adaptive offset to one component of one coding tree block.
///
/// A CTB without any sample that must keep its value (transquant bypass, or
/// PCM with `pcm_loop_filter_disabled_flag`) goes through the row-wise
/// [`band_rows`] or [`edge_rows`]; the rare one with such samples through the
/// per-sample [`sao_ctb_checked`]. All three give the same samples.
#[allow(clippy::too_many_arguments)]
fn sao_ctb(
    dst: &mut PlaneMut<'_>,
    src: &Plane,
    cu_flags: &[u8],
    min4_w: usize,
    info: &FilterInfo,
    s: SaoCtb,
    addr: usize,
    sw: usize,
    sh: usize,
    bd: u8,
) {
    let (cx, cy) = (addr % info.pic_w_ctbs, addr / info.pic_w_ctbs);
    let r = Rect {
        x0: (cx << info.ctb_log2) / sw,
        y0: (cy << info.ctb_log2) / sh,
        x1: (((cx + 1) << info.ctb_log2) / sw).min(dst.width),
        y1: (((cy + 1) << info.ctb_log2) / sh).min(dst.height),
    };
    if r.x0 >= r.x1 || r.y0 >= r.y1 {
        return;
    }
    let max = (1i32 << bd) - 1;
    if has_unfiltered(cu_flags, min4_w, info, r, sw, sh) {
        sao_ctb_checked(dst, src, cu_flags, min4_w, info, s, addr, r, sw, sh, bd);
    } else if s.type_idx == BAND {
        band_rows(dst, src, &s, r, bd, max);
    } else {
        edge_rows(dst, src, info, &s, addr, (cx, cy), r, max);
    }
}

/// True when some sample of `r` lies in a coding unit SAO must leave alone.
fn has_unfiltered(
    cu_flags: &[u8],
    min4_w: usize,
    info: &FilterInfo,
    r: Rect,
    sw: usize,
    sh: usize,
) -> bool {
    let mask = flags::TQ_BYPASS
        | if info.pcm_loop_filter_disabled {
            flags::PCM
        } else {
            0
        };
    // The 4x4 luma units the per-sample lookup of `sao_ctb_checked` reads.
    let (bx0, bx1) = ((r.x0 * sw) >> 2, ((r.x1 - 1) * sw) >> 2);
    let (by0, by1) = ((r.y0 * sh) >> 2, ((r.y1 - 1) * sh) >> 2);
    (by0..=by1).any(|by| {
        let row = by * min4_w;
        cu_flags
            .get(row + bx0..=(row + bx1).min(cu_flags.len().saturating_sub(1)))
            .is_some_and(|f| f.iter().fold(0, |a, v| a | v) & mask != 0)
    })
}

/// Band offset over all of `r`.
fn band_rows(dst: &mut PlaneMut<'_>, src: &Plane, s: &SaoCtb, r: Rect, bd: u8, max: i32) {
    let shift = bd.saturating_sub(5);
    // Offset per band: the four bands from `sao_band_position` on, wrapping past band 31
    // to band 0 (bandTable, 8.7.3.2).
    // Selected by comparisons rather than a table, so the row loop vectorises.
    let o = s.offsets.map(i32::from);
    let bp = i32::from(s.band_position);
    let w = r.x1 - r.x0;
    for py in r.y0..r.y1 {
        let so = src.offset(r.x0, py);
        let dof = dst.offset(r.x0, py);
        let (Some(cur), Some(out)) = (src.data.get(so..so + w), dst.data.get_mut(dof..dof + w))
        else {
            return;
        };
        for (d, &c) in out.iter_mut().zip(cur) {
            let c = i32::from(c);
            let k = ((c >> shift) - bp) & 31;
            let add = if k == 0 {
                o[0]
            } else if k == 1 {
                o[1]
            } else if k == 2 {
                o[2]
            } else if k == 3 {
                o[3]
            } else {
                0
            };
            *d = (c + add).clamp(0, max) as u16;
        }
    }
}

/// `SaoOffsetVal` by `2 + Sign(rec - a) + Sign(rec - b)`, clause 8.7.3.2:
/// edgeIdx 0, 1 and 2 swap places, and edgeIdx 0 adds nothing.
fn edge_lut(s: &SaoCtb) -> [i32; 8] {
    let o = s.offsets.map(i32::from);
    [o[0], o[1], 0, o[2], o[3], 0, 0, 0]
}

/// Edge offset over all of `r`.
///
/// Whether a neighbour may be read depends only on which of the nine CTBs
/// around (and including) this one it falls in, so that is settled once per
/// CTB; the samples whose neighbours all lie in this CTB then run as plain
/// row loops, and only the rim goes sample by sample.
#[allow(clippy::too_many_arguments)]
fn edge_rows(
    dst: &mut PlaneMut<'_>,
    src: &Plane,
    info: &FilterInfo,
    s: &SaoCtb,
    addr: usize,
    (cx, cy): (usize, usize),
    r: Rect,
    max: i32,
) {
    let [(dxa, dya), (dxb, dyb)] = EO[(s.eo_class & 3) as usize];
    let lut = edge_lut(s);
    // allow[ry][rx]: the CTB at (cx + rx - 1, cy + ry - 1) is inside the
    // picture and may be read across into. A neighbour left of `x0` lies in
    // CTB column cx - 1 and one at `x1` in column cx + 1, because the
    // chroma subsampling factors divide the CTB size.
    let col_ok = [r.x0 > 0, true, r.x1 < src.width];
    let row_ok = [r.y0 > 0, true, r.y1 < src.height];
    let mut allow = [[false; 3]; 3];
    for (ry, row) in allow.iter_mut().enumerate() {
        for (rx, a) in row.iter_mut().enumerate() {
            if row_ok[ry] && col_ok[rx] {
                let n = (cy + ry - 1) * info.pic_w_ctbs + (cx + rx - 1);
                *a = info.may_cross(n, addr);
            }
        }
    }
    let region = |v: isize, lo: usize, hi: usize| {
        if v < lo as isize {
            0
        } else if v >= hi as isize {
            2
        } else {
            1
        }
    };
    let rim = |dst: &mut PlaneMut<'_>, px: usize, py: usize| {
        let rec = i32::from(src.at(px, py));
        let mut e = 2i32;
        for (dx, dy) in [(dxa, dya), (dxb, dyb)] {
            let (nx, ny) = (px as isize + dx, py as isize + dy);
            if !allow[region(ny, r.y0, r.y1)][region(nx, r.x0, r.x1)] {
                return;
            }
            e += (rec - i32::from(src.at(nx as usize, ny as usize))).signum();
        }
        dst.put(px, py, (rec + lut[e as usize & 7]).clamp(0, max) as u16);
    };
    // The samples whose two neighbours both lie inside `r`.
    let (mx, my) = (dxa.unsigned_abs(), dya.unsigned_abs());
    let (ix0, ix1) = (r.x0 + mx, r.x1.saturating_sub(mx).max(r.x0 + mx));
    let (iy0, iy1) = (r.y0 + my, r.y1.saturating_sub(my).max(r.y0 + my));
    for py in r.y0..r.y1 {
        if py < iy0 || py >= iy1 || ix0 >= ix1 {
            for px in r.x0..r.x1 {
                rim(dst, px, py);
            }
            continue;
        }
        for px in (r.x0..ix0).chain(ix1..r.x1) {
            rim(dst, px, py);
        }
        let w = ix1 - ix0;
        let at = |dx: isize, dy: isize| {
            src.offset((ix0 as isize + dx) as usize, (py as isize + dy) as usize)
        };
        let (c0, a0, b0) = (at(0, 0), at(dxa, dya), at(dxb, dyb));
        let dof = dst.offset(ix0, py);
        let (Some(cur), Some(na), Some(nb), Some(out)) = (
            src.data.get(c0..c0 + w),
            src.data.get(a0..a0 + w),
            src.data.get(b0..b0 + w),
            dst.data.get_mut(dof..dof + w),
        ) else {
            return;
        };
        edge_row(out, cur, na, nb, &lut, max);
    }
}

/// Edge offset of one run of samples whose neighbours are `na` and `nb`.
#[inline(always)]
fn edge_row(out: &mut [u16], cur: &[u16], na: &[u16], nb: &[u16], lut: &[i32; 8], max: i32) {
    for (((o, &c), &a), &b) in out.iter_mut().zip(cur).zip(na).zip(nb) {
        let c = i32::from(c);
        let e = (c - i32::from(a)).signum() + (c - i32::from(b)).signum();
        // `lut[2 + e]` as selects, so the row loop vectorises.
        let add = if e == -2 {
            lut[0]
        } else if e == -1 {
            lut[1]
        } else if e == 1 {
            lut[3]
        } else if e == 2 {
            lut[4]
        } else {
            0
        };
        *o = (c + add).clamp(0, max) as u16;
    }
}

/// [`sao_ctb`] one sample at a time, looking up every sample's coding unit.
#[allow(clippy::too_many_arguments)]
fn sao_ctb_checked(
    dst: &mut PlaneMut<'_>,
    src: &Plane,
    cu_flags: &[u8],
    min4_w: usize,
    info: &FilterInfo,
    s: SaoCtb,
    addr: usize,
    r: Rect,
    sw: usize,
    sh: usize,
    bd: u8,
) {
    let Rect { x0, y0, x1, y1 } = r;
    let max = (1i32 << bd) - 1;
    for py in y0..y1 {
        for px in x0..x1 {
            let (lx, ly) = (px * sw, py * sh);
            let f = cu_flags
                .get((ly >> 2) * min4_w + (lx >> 2))
                .map_or(0u8, |v| *v);
            if f & flags::TQ_BYPASS != 0 || (f & flags::PCM != 0 && info.pcm_loop_filter_disabled) {
                continue;
            }
            let rec = src.at(px, py) as i32;
            let delta = if s.type_idx == BAND {
                // Bands wrap past 31 to 0 (8.7.3.2).
                let k = ((rec >> bd.saturating_sub(5)) - s.band_position as i32) & 31;
                match s.offsets.get(k as usize) {
                    Some(v) if k < 4 => *v as i32,
                    _ => continue,
                }
            } else {
                match edge_offset(src, info, &s, addr, px, py, sw, sh) {
                    Some(v) => v,
                    None => continue,
                }
            };
            dst.put(px, py, clip3(0, max, rec + delta) as u16);
        }
    }
}

/// Applies the sample adaptive offset filter of clause 8.7.3 to `pic` in place.
///
/// Does nothing unless `info.sao_enabled`; coding tree blocks whose
/// `SaoTypeIdx` is zero keep their deblocked samples.
#[cfg(test)]
pub fn sao(pic: &mut Picture, info: &FilterInfo) {
    run(pic, info, 1);
}

/// [`sao`] on up to `threads` threads, one CTB row of one component per
/// job; the output does not depend on `threads`.
pub(super) fn run(pic: &mut Picture, info: &FilterInfo, threads: usize) {
    run_impl(pic, info, threads, false);
}

/// [`run`], with every CTB going through [`sao_ctb_checked`] when `checked`.
pub(super) fn run_impl(pic: &mut Picture, info: &FilterInfo, threads: usize, checked: bool) {
    if !info.sao_enabled || info.pic_w_ctbs == 0 {
        return;
    }
    let ncomp = if info.chroma_array_type == 0 { 1 } else { 3 };
    let n = info.ctb.len().min(info.pic_w_ctbs * info.pic_h_ctbs);
    // A component that no CTB applies SAO to keeps its deblocked samples, so
    // its snapshot would never be read. Not taking it is what makes a picture
    // that merely signals SAO in its parameter sets cost nothing here.
    let used = |c: usize| {
        info.ctb[..n]
            .iter()
            .any(|f| f.sao[c].type_idx == BAND || f.sao[c].type_idx == EDGE)
    };
    let snapshot = |c: usize, p: &Plane| {
        if c < ncomp && used(c) {
            p.clone()
        } else {
            Plane {
                data: Vec::new(),
                stride: 0,
                width: 0,
                height: 0,
                oy: 0,
                ox: 0,
            }
        }
    };
    let src = [
        snapshot(0, &pic.y),
        snapshot(1, &pic.cb),
        snapshot(2, &pic.cr),
    ];
    let (y, cb, cr, side) = super::parts(pic);
    let (w, rows) = (info.pic_w_ctbs, info.pic_h_ctbs);
    let mut jobs = Vec::new();
    for (c, plane) in [y, cb, cr].into_iter().enumerate().take(ncomp) {
        if src[c].data.is_empty() {
            continue;
        }
        let sh = if c == 0 { 1 } else { info.sub_h.max(1) };
        // Cut at the first row of each CTB row: band r is CTB row r.
        let cuts: Vec<usize> = (1..rows).map(|r| (r << info.ctb_log2) / sh).collect();
        jobs.extend(
            plane
                .split_rows(&cuts)
                .into_iter()
                .enumerate()
                .map(move |(r, band)| (c, r, band)),
        );
    }
    super::run_jobs(jobs, threads, |(c, r, mut dst)| {
        let (sw, sh) = if c == 0 {
            (1, 1)
        } else {
            (info.sub_w.max(1), info.sub_h.max(1))
        };
        let bd = if c == 0 {
            info.bit_depth_y
        } else {
            info.bit_depth_c
        };
        for addr in r * w..((r + 1) * w).min(n) {
            let s = info.ctb[addr].sao[c];
            if s.type_idx != BAND && s.type_idx != EDGE {
                continue;
            }
            let (dst, src) = (&mut dst, &src[c]);
            let (cu, m4) = (side.cu_flags, side.min4_w);
            if checked {
                let (cx, cy) = (addr % w, addr / w);
                let r = Rect {
                    x0: (cx << info.ctb_log2) / sw,
                    y0: (cy << info.ctb_log2) / sh,
                    x1: (((cx + 1) << info.ctb_log2) / sw).min(dst.width),
                    y1: (((cy + 1) << info.ctb_log2) / sh).min(dst.height),
                };
                sao_ctb_checked(dst, src, cu, m4, info, s, addr, r, sw, sh, bd);
            } else {
                sao_ctb(dst, src, cu, m4, info, s, addr, sw, sh, bd);
            }
        }
    });
}

/// CTB row `q` of every component as it leaves the loop filters, for the
/// row pipeline: a copy of its deblocked lines with SAO applied, read from
/// the deblocked picture, which is left as it is. The row is ready once the
/// deblocked samples down to the first line of the row below are in place.
/// A component of width zero gives no lines. The samples are those of
/// [`run`].
pub(super) fn row_out(pic: &Picture, info: &FilterInfo, q: usize) -> [Vec<u16>; 3] {
    let ncomp = if info.chroma_array_type == 0 { 1 } else { 3 };
    let w = info.pic_w_ctbs;
    let n = info.ctb.len().min(w * info.pic_h_ctbs);
    let addrs = (q * w).min(n)..((q + 1) * w).min(n);
    let mut out: [Vec<u16>; 3] = Default::default();
    for (c, plane) in [&pic.y, &pic.cb, &pic.cr].into_iter().enumerate() {
        let (sw, sh) = if c == 0 {
            (1, 1)
        } else {
            (info.sub_w.max(1), info.sub_h.max(1))
        };
        let y0 = ((q << info.ctb_log2) / sh).min(plane.height);
        let y1 = (((q + 1) << info.ctb_log2) / sh).min(plane.height);
        let width = plane.width;
        if y0 >= y1 || width == 0 {
            continue;
        }
        let mut data = Vec::with_capacity((y1 - y0) * width);
        for yy in y0..y1 {
            let o = plane.offset(0, yy);
            data.extend_from_slice(&plane.data[o..o + width]);
        }
        if info.sao_enabled && c < ncomp && w > 0 {
            let bd = if c == 0 {
                info.bit_depth_y
            } else {
                info.bit_depth_c
            };
            let mut dst = PlaneMut {
                data: &mut data,
                stride: width,
                width,
                height: plane.height,
                oy: y0,
            };
            for addr in addrs.clone() {
                let s = info.ctb[addr].sao[c];
                if s.type_idx != BAND && s.type_idx != EDGE {
                    continue;
                }
                sao_ctb(
                    &mut dst,
                    plane,
                    &pic.cu_flags,
                    pic.min4_w,
                    info,
                    s,
                    addr,
                    sw,
                    sh,
                    bd,
                );
            }
        }
        out[c] = data;
    }
    out
}

#[cfg(test)]
#[path = "sao_tests.rs"]
mod tests;
