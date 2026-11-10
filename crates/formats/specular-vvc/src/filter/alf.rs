//! Adaptive loop filter (8.8.5): luma classification and 7x7 diamond filters, chroma 5x5
//! diamond filters, and the cross-component filters, with the ALF virtual boundary above
//! each CTB bottom and the boundary padding of 8.8.5.5 and 8.8.5.6.

use super::alf_tables::{CLASS_TO_FILT_MAP, FIX_FILT_COEFF};
use super::{Band, Shared, bands, run_jobs};
use crate::picture::Picture;
use crate::ps::alf::{AlfAps, clip_value};

/// The boundary positions of 8.8.5.5 for one sample, in luma samples (-128 when none).
#[derive(Clone, Copy, Debug)]
struct Clip {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
    top_left: bool,
    bot_right: bool,
}

/// Per-CTB facts for 8.8.5.5: which CTB edges stop the filter.
#[derive(Clone, Copy, Debug)]
struct CtbBounds {
    x: i32,
    y: i32,
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
    top_left: bool,
    bot_right: bool,
}

fn ctb_bounds(sh: &Shared<'_>, rx: u32, ry: u32) -> CtbBounds {
    let pps = sh.ctx.pps;
    let me = (ry * sh.w_ctbs + rx) as usize;
    let other = |dx: i32, dy: i32| -> Option<usize> {
        let nx = rx as i32 + dx;
        let ny = ry as i32 + dy;
        if nx < 0 || ny < 0 || nx >= sh.w_ctbs as i32 || ny >= sh.h_ctbs as i32 {
            None
        } else {
            Some((ny as u32 * sh.w_ctbs + nx as u32) as usize)
        }
    };
    let stop = |n: Option<usize>| -> bool {
        let Some(n) = n else { return false };
        (sh.ctb_tile[me] != sh.ctb_tile[n] && !pps.loop_filter_across_tiles)
            || (sh.slice_of_ctb(me) != sh.slice_of_ctb(n) && !pps.loop_filter_across_slices)
            || (sh.ctb_subpic[me] != sh.ctb_subpic[n] && !sh.lf_across_subpic(me))
    };
    let diag = |n: Option<usize>| -> bool {
        n.is_some_and(|n| {
            sh.slice_of_ctb(me) != sh.slice_of_ctb(n) && !pps.loop_filter_across_slices
        })
    };
    CtbBounds {
        x: (rx << sh.log2_ctb) as i32,
        y: (ry << sh.log2_ctb) as i32,
        left: stop(other(-1, 0)),
        right: stop(other(1, 0)),
        top: stop(other(0, -1)),
        bottom: stop(other(0, 1)),
        top_left: diag(other(-1, -1)),
        bot_right: diag(other(1, 1)),
    }
}

/// ALF boundary position derivation (8.8.5.5) for luma position (x, y) inside the CTB.
fn clip_pos(sh: &Shared<'_>, b: &CtbBounds, x: i32, y: i32, vb_offset: i32) -> Clip {
    let ctb = 1i32 << sh.log2_ctb;
    let vbx = sh.ctx.vb_x;
    let vby = sh.ctx.vb_y;
    let pic_h = sh.pic_h as i32;
    let mut c = Clip {
        left: -128,
        right: -128,
        top: -128,
        bottom: -128,
        top_left: b.top_left,
        bot_right: b.bot_right,
    };
    if y - (ctb - vb_offset) >= 0 {
        c.top = b.y + ctb - vb_offset;
    } else if let Some(&v) = vby
        .iter()
        .find(|&&v| (0..3).contains(&(b.y + y - v as i32)))
    {
        c.top = v as i32;
    } else if y < 3 && b.top {
        c.top = b.y;
    }
    if let Some(&v) = vby.iter().find(|&&v| {
        let v = v as i32;
        v != pic_h - 1 && v != 0 && (1..5).contains(&(v - b.y - y))
    }) {
        c.bottom = v as i32;
    } else if (1..5).contains(&(ctb - vb_offset - y)) {
        c.bottom = b.y + ctb - vb_offset;
    } else if ctb - y < 5 && b.bottom {
        c.bottom = b.y + ctb;
    }
    if let Some(&v) = vbx
        .iter()
        .find(|&&v| (0..3).contains(&(b.x + x - v as i32)))
    {
        c.left = v as i32;
    } else if x < 3 && b.left {
        c.left = b.x;
    }
    if let Some(&v) = vbx
        .iter()
        .find(|&&v| (1..5).contains(&(v as i32 - b.x - x)))
    {
        c.right = v as i32;
    } else if ctb - x < 5 && b.right {
        c.right = b.x + ctb;
    }
    c
}

/// The sample padding process (8.8.5.6), positions already clipped to the picture.
#[derive(Clone, Copy)]
struct Pad {
    pic_w: i32,
    pic_h: i32,
    xc: i32,
    yc: i32,
    cw: i32,
    ch: i32,
    top: i32,
    bottom: i32,
    left: i32,
    right: i32,
    tl: bool,
    br: bool,
}

impl Pad {
    fn new(c: &Clip, b: &CtbBounds, sh: &Shared<'_>, (sx, sy): (u32, u32)) -> Pad {
        let ctb = 1i32 << sh.log2_ctb;
        let dv = |v: i32, s: u32| if v < 0 { v } else { v >> s };
        Pad {
            pic_w: (sh.pic_w >> sx) as i32,
            pic_h: (sh.pic_h >> sy) as i32,
            xc: b.x >> sx,
            yc: b.y >> sy,
            cw: ctb >> sx,
            ch: ctb >> sy,
            top: dv(c.top, sy),
            bottom: dv(c.bottom, sy),
            left: dv(c.left, sx),
            right: dv(c.right, sx),
            tl: c.top_left,
            br: c.bot_right,
        }
    }

    #[inline]
    fn apply(&self, x: i32, y: i32) -> (usize, usize) {
        let mut x = x.clamp(0, self.pic_w - 1);
        let mut y = y.clamp(0, self.pic_h - 1);
        if self.top >= 0 {
            y = y.clamp(self.top, self.pic_h - 1);
        }
        if self.bottom >= 0 {
            y = y.clamp(0, self.bottom - 1);
        }
        if self.left >= 0 {
            x = x.clamp(self.left, self.pic_w - 1);
        }
        if self.right >= 0 {
            x = x.clamp(0, self.right - 1);
        }
        if self.tl && self.top < 0 && self.left < 0 && x < self.xc && y < self.yc {
            x = self.xc;
        }
        if self.br
            && self.bottom < 0
            && self.right < 0
            && x > self.xc + self.cw - 1
            && y > self.yc + self.ch - 1
        {
            x = self.xc + self.cw - 1;
        }
        (x as usize, y as usize)
    }
}

/// transposeIdx-dependent tap order (equations 1442 to 1445).
const TRANSPOSE: [[usize; 12]; 4] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    [9, 4, 10, 8, 1, 5, 11, 7, 3, 0, 2, 6],
    [0, 3, 2, 1, 8, 7, 6, 5, 4, 9, 10, 11],
    [9, 8, 10, 4, 3, 7, 11, 5, 1, 0, 2, 6],
];

/// The luma filter (f, c) of a CTB: per class (25).
type LumaFilters = [([i32; 12], [i32; 12]); 25];

fn luma_filters(set: u8, aps: Option<&AlfAps>, bd: u32) -> LumaFilters {
    let mut out = [([0i32; 12], [0i32; 12]); 25];
    for (k, o) in out.iter_mut().enumerate() {
        if set < 16 {
            let f = FIX_FILT_COEFF[usize::from(CLASS_TO_FILT_MAP[usize::from(set)][k])];
            o.0 = f.map(i32::from);
            o.1 = [1 << bd; 12];
        } else if let Some(a) = aps {
            for j in 0..12 {
                o.0[j] = i32::from(a.luma_coeff[k][j]);
                o.1[j] = clip_value(bd, a.luma_clip[k][j]);
            }
        }
    }
    out
}

/// Classification of one 4x4 block (8.8.5.3): (filtIdx, transposeIdx).
#[allow(clippy::too_many_arguments)]
fn classify(
    src: &[u16],
    stride: usize,
    pad: &Pad,
    gx: i32,
    gy: i32,
    y4: i32,
    ctb: i32,
    pic_bottom_near: bool,
    bd: u32,
) -> (usize, usize) {
    let (min_y, max_y, ac) = if y4 == ctb - 8 && !pic_bottom_near {
        (-2, 3, 3)
    } else if y4 == ctb - 4 && !pic_bottom_near {
        (0, 5, 3)
    } else {
        (-2, 5, 2)
    };
    // Padded rows -3..6 and columns -3..6 around the block.
    let mut rows = [0usize; 10];
    let mut cols = [0usize; 10];
    for t in 0..10 {
        rows[t] = pad.apply(gx, gy + t as i32 - 3).1;
        cols[t] = pad.apply(gx + t as i32 - 3, gy).0;
    }
    let corner = pad.tl || pad.br;
    let at = |i: i32, j: i32| -> i32 {
        if corner {
            let (x, y) = pad.apply(gx + i, gy + j);
            i32::from(src[y * stride + x])
        } else {
            i32::from(src[rows[(j + 3) as usize] * stride + cols[(i + 3) as usize]])
        }
    };
    let (mut sh_, mut sv, mut sd0, mut sd1) = (0i32, 0i32, 0i32, 0i32);
    for j in min_y..=max_y {
        for i in -2..=5i32 {
            if (i & 1) != (j & 1) {
                continue;
            }
            let c2 = at(i, j) << 1;
            sh_ += (c2 - at(i - 1, j) - at(i + 1, j)).abs();
            sv += (c2 - at(i, j - 1) - at(i, j + 1)).abs();
            sd0 += (c2 - at(i - 1, j - 1) - at(i + 1, j + 1)).abs();
            sd1 += (c2 - at(i + 1, j - 1) - at(i - 1, j + 1)).abs();
        }
    }
    let (hv1, hv0, dir_hv) = if sv > sh_ { (sv, sh_, 1) } else { (sh_, sv, 3) };
    let (d1, d0, dir_d) = if sd0 > sd1 {
        (sd0, sd1, 0)
    } else {
        (sd1, sd0, 2)
    };
    let diag = i64::from(d1) * i64::from(hv0) > i64::from(hv1) * i64::from(d0);
    let (hvd1, hvd0) = if diag { (d1, d0) } else { (hv1, hv0) };
    let dir1 = if diag { dir_d } else { dir_hv };
    let dir2 = if diag { dir_hv } else { dir_d };
    let dir_s = if i64::from(hvd1) * 2 > 9 * i64::from(hvd0) {
        2
    } else if hvd1 > 2 * hvd0 {
        1
    } else {
        0
    };
    const VAR: [usize; 16] = [0, 1, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 4];
    let act = ((i64::from(sh_ + sv) * ac) >> (bd - 1)).clamp(0, 15) as usize;
    let mut filt = VAR[act];
    if dir_s != 0 {
        filt += (((dir1 & 1) << 1) + dir_s) * 5;
    }
    const TT: [usize; 8] = [0, 1, 0, 2, 2, 3, 1, 3];
    (filt, TT[dir1 * 2 + (dir2 >> 1)])
}

/// The luma CTB filter (8.8.5.2).
#[allow(clippy::too_many_arguments)]
fn luma_ctb(
    sh: &Shared<'_>,
    src: &[u16],
    out: &mut Band<'_>,
    stride: usize,
    rx: u32,
    ry: u32,
    filters: &LumaFilters,
    bd: u32,
) {
    let b = ctb_bounds(sh, rx, ry);
    let ctb = 1i32 << sh.log2_ctb;
    let pw = sh.pic_w as i32;
    let ph = sh.pic_h as i32;
    let w = ctb.min(pw - b.x);
    let h = ctb.min(ph - b.y);
    // The CTB bottom is the picture bottom and the virtual boundary would fall below it.
    let at_bottom = b.y + ctb >= ph;
    let near = at_bottom && ph - b.y <= ctb - 4;
    let max = (1i32 << bd) - 1;
    for by in (0..h).step_by(4) {
        for bx in (0..w).step_by(4) {
            let c = clip_pos(sh, &b, bx, by, 4);
            let pad = Pad::new(&c, &b, sh, (0, 0));
            let (filt, tr) = classify(src, stride, &pad, b.x + bx, b.y + by, by, ctb, near, bd);
            let (f, cl) = &filters[filt];
            let idx = &TRANSPOSE[tr];
            for y in by..(by + 4).min(h) {
                let gy = b.y + y;
                if (gy as usize) < out.y0 || gy as usize >= out.y0 + out.rows {
                    continue;
                }
                let (shift, y1, y2, y3) = if !near && (y == ctb - 5 || y == ctb - 4) {
                    (10, 0, 0, 0)
                } else if !near && (y == ctb - 6 || y == ctb - 3) {
                    (7, 1, 1, 1)
                } else if !near && (y == ctb - 7 || y == ctb - 2) {
                    (7, 1, 2, 2)
                } else {
                    (7, 1, 2, 3)
                };
                for x in bx..(bx + 4).min(w) {
                    let c = clip_pos(sh, &b, x, y, 4);
                    let pad = Pad::new(&c, &b, sh, (0, 0));
                    let gx = b.x + x;
                    let separable = !pad.tl && !pad.br;
                    let mut rows = [0usize; 7];
                    let mut cols = [0usize; 7];
                    if separable {
                        for t in 0..7 {
                            rows[t] = pad.apply(gx, gy + t as i32 - 3).1 * stride;
                            cols[t] = pad.apply(gx + t as i32 - 3, gy).0;
                        }
                    }
                    let s = |i: i32, j: i32| -> i32 {
                        if separable {
                            i32::from(src[rows[(j + 3) as usize] + cols[(i + 3) as usize]])
                        } else {
                            let (px, py) = pad.apply(gx + i, gy + j);
                            i32::from(src[py * stride + px])
                        }
                    };
                    let curr = s(0, 0);
                    let t = |k: usize, a: (i32, i32), bb: (i32, i32)| -> i32 {
                        let cc = cl[idx[k]];
                        f[idx[k]]
                            * ((s(a.0, a.1) - curr).clamp(-cc, cc)
                                + (s(bb.0, bb.1) - curr).clamp(-cc, cc))
                    };
                    let sum = t(0, (0, y3), (0, -y3))
                        + t(1, (1, y2), (-1, -y2))
                        + t(2, (0, y2), (0, -y2))
                        + t(3, (-1, y2), (1, -y2))
                        + t(4, (2, y1), (-2, -y1))
                        + t(5, (1, y1), (-1, -y1))
                        + t(6, (0, y1), (0, -y1))
                        + t(7, (-1, y1), (1, -y1))
                        + t(8, (-2, y1), (2, -y1))
                        + t(9, (3, 0), (-3, 0))
                        + t(10, (2, 0), (-2, 0))
                        + t(11, (1, 0), (-1, 0));
                    let v = curr + ((sum + (1 << (shift - 1))) >> shift);
                    out.data[(gy as usize - out.y0) * stride + gx as usize] =
                        v.clamp(0, max) as u16;
                }
            }
        }
    }
}

/// The chroma CTB filter (8.8.5.4).
#[allow(clippy::too_many_arguments)]
fn chroma_ctb(
    sh: &Shared<'_>,
    src: &[u16],
    out: &mut Band<'_>,
    stride: usize,
    (sx, sy): (u32, u32),
    rx: u32,
    ry: u32,
    f: &[i32; 6],
    cl: &[i32; 6],
    bd: u32,
) {
    let b = ctb_bounds(sh, rx, ry);
    let ctb = 1i32 << sh.log2_ctb;
    let cw = ctb >> sx;
    let chh = ctb >> sy;
    let pw = (sh.pic_w >> sx) as i32;
    let ph = (sh.pic_h >> sy) as i32;
    let xc = b.x >> sx;
    let yc = b.y >> sy;
    let w = cw.min(pw - xc);
    let h = chh.min(ph - yc);
    let at_bottom = b.y + ctb >= sh.pic_h as i32;
    let near = at_bottom && (sh.pic_h as i32) - (yc << sy) < ctb - 4;
    let max = (1i32 << bd) - 1;
    let vb = 2 << sy;
    for y in 0..h {
        let gy = yc + y;
        if (gy as usize) < out.y0 || gy as usize >= out.y0 + out.rows {
            continue;
        }
        let (shift, y1, y2) = if !near && (y == chh - 2 || y == chh - 3) {
            (10, 0, 0)
        } else if !near && (y == chh - 1 || y == chh - 4) {
            (7, 1, 1)
        } else {
            (7, 1, 2)
        };
        for x in 0..w {
            let c = clip_pos(sh, &b, x << sx, y << sy, vb);
            let pad = Pad::new(&c, &b, sh, (sx, sy));
            let gx = xc + x;
            let s = |i: i32, j: i32| -> i32 {
                let (px, py) = pad.apply(gx + i, gy + j);
                i32::from(src[py * stride + px])
            };
            let curr = s(0, 0);
            let t = |k: usize, a: (i32, i32), bb: (i32, i32)| -> i32 {
                f[k] * ((s(a.0, a.1) - curr).clamp(-cl[k], cl[k])
                    + (s(bb.0, bb.1) - curr).clamp(-cl[k], cl[k]))
            };
            let sum = t(0, (0, y2), (0, -y2))
                + t(1, (1, y1), (-1, -y1))
                + t(2, (0, y1), (0, -y1))
                + t(3, (-1, y1), (1, -y1))
                + t(4, (2, 0), (-2, 0))
                + t(5, (1, 0), (-1, 0));
            let v = curr + ((sum + (1 << (shift - 1))) >> shift);
            out.data[(gy as usize - out.y0) * stride + gx as usize] = v.clamp(0, max) as u16;
        }
    }
}

/// The cross-component filter (8.8.5.7) for one CTB: `luma` is the luma before ALF,
/// `out` holds the chroma after its own ALF and receives the result.
#[allow(clippy::too_many_arguments)]
fn cc_ctb(
    sh: &Shared<'_>,
    luma: &[u16],
    lstride: usize,
    out: &mut Band<'_>,
    stride: usize,
    (sx, sy): (u32, u32),
    rx: u32,
    ry: u32,
    f: &[i16; 7],
    bd: u32,
) {
    let b = ctb_bounds(sh, rx, ry);
    let ctb = 1i32 << sh.log2_ctb;
    let cw = ctb >> sx;
    let chh = ctb >> sy;
    let pw = (sh.pic_w >> sx) as i32;
    let ph = (sh.pic_h >> sy) as i32;
    let xc = b.x >> sx;
    let yc = b.y >> sy;
    let w = cw.min(pw - xc);
    let h = chh.min(ph - yc);
    let at_bottom = b.y + ctb >= sh.pic_h as i32;
    let near = at_bottom && (sh.pic_h as i32) - (yc << sy) <= ctb - 4;
    let max = (1i32 << bd) - 1;
    let lim = 1i32 << (bd - 1);
    let f: [i32; 7] = f.map(i32::from);
    for y in 0..h {
        let gy = yc + y;
        if (gy as usize) < out.y0 || gy as usize >= out.y0 + out.rows {
            continue;
        }
        let ly = y << sy;
        let (p1, p2) = if !near && (ly == ctb - 5 || ly == ctb - 4) {
            (0, 0)
        } else if !near && (ly == ctb - 6 || ly == ctb - 3) {
            (1, 1)
        } else {
            (1, 2)
        };
        let keep = sy == 0 && (y == ctb - 3 || y == ctb - 4);
        for x in 0..w {
            let gx = xc + x;
            let o = (gy as usize - out.y0) * stride + gx as usize;
            if keep {
                continue;
            }
            let c = clip_pos(sh, &b, x << sx, y << sy, 4);
            let pad = Pad::new(&c, &b, sh, (0, 0));
            let lx = gx << sx;
            let lyy = gy << sy;
            let s = |i: i32, j: i32| -> i32 {
                let (px, py) = pad.apply(lx + i, lyy + j);
                i32::from(luma[py * lstride + px])
            };
            let c0 = s(0, 0);
            let sum = f[0] * (s(0, -p1) - c0)
                + f[1] * (s(-1, 0) - c0)
                + f[2] * (s(1, 0) - c0)
                + f[3] * (s(-1, p1) - c0)
                + f[4] * (s(0, p1) - c0)
                + f[5] * (s(1, p1) - c0)
                + f[6] * (s(0, p2) - c0);
            let scaled = ((sum + 64) >> 7).clamp(-lim, lim - 1);
            let curr = i32::from(out.data[o]);
            out.data[o] = (curr + scaled).clamp(0, max) as u16;
        }
    }
}

/// The adaptive loop filter process (8.8.5.1).
pub(super) fn run(pic: &mut Picture, sh: &Shared<'_>, threads: usize) {
    let ctx = sh.ctx;
    if !ctx.slices.iter().any(|s| s.alf.enabled) {
        return;
    }
    let bd = pic.bit_depth;
    let (sx, sy) = match pic.chroma_format_idc {
        1 => (1, 1),
        2 => (1, 0),
        _ => (0, 0),
    };
    let n = (sh.w_ctbs * sh.h_ctbs) as usize;
    let aps = |id: u8| ctx.alf_aps.get(usize::from(id)).and_then(|a| a.as_deref());
    // The luma before ALF: the source of luma ALF and of CC-ALF.
    let luma_src = pic.planes[0].data.clone();
    let lstride = pic.planes[0].stride;
    let any_luma = (0..n).any(|c| ctx.alf.get(c).is_some_and(|a| a.enabled[0]));
    if any_luma {
        let plane = &mut pic.planes[0];
        let stride = plane.stride;
        let ctb_rows = 1usize << sh.log2_ctb;
        let cuts: Vec<usize> = (0..=sh.h_ctbs as usize)
            .map(|r| (r * ctb_rows).min(plane.h))
            .collect();
        let jobs: Vec<(u32, Band<'_>)> = bands(&mut plane.data, stride, &cuts)
            .into_iter()
            .enumerate()
            .map(|(r, b)| (r as u32, b))
            .collect();
        let src = &luma_src;
        run_jobs(jobs, threads, |(ry, mut band)| {
            for rx in 0..sh.w_ctbs {
                let c = (ry * sh.w_ctbs + rx) as usize;
                let (Some(a), Some(s)) = (ctx.alf.get(c), sh.slice(c)) else {
                    continue;
                };
                if !a.enabled[0] || !s.alf.enabled {
                    continue;
                }
                let ap = if a.luma_set >= 16 {
                    match s.alf.luma_aps.get(usize::from(a.luma_set - 16)) {
                        Some(&id) => aps(id),
                        None => continue,
                    }
                } else {
                    None
                };
                if a.luma_set >= 16 && ap.is_none_or(|p| !p.luma) {
                    continue;
                }
                let filters = luma_filters(a.luma_set, ap, bd);
                luma_ctb(sh, src, &mut band, stride, rx, ry, &filters, bd);
            }
        });
    }
    if pic.chroma_format_idc == 0 || pic.planes.len() < 3 {
        return;
    }
    for ci in 1..3 {
        let plane = &mut pic.planes[ci];
        let stride = plane.stride;
        let src = plane.data.clone();
        let ctb_rows = (1usize << sh.log2_ctb) >> sy;
        let cuts: Vec<usize> = (0..=sh.h_ctbs as usize)
            .map(|r| (r * ctb_rows).min(plane.h))
            .collect();
        let jobs: Vec<(u32, Band<'_>)> = bands(&mut plane.data, stride, &cuts)
            .into_iter()
            .enumerate()
            .map(|(r, b)| (r as u32, b))
            .collect();
        let src = &src;
        let luma = &luma_src;
        run_jobs(jobs, threads, |(ry, mut band)| {
            // Chroma ALF of the CTB row, then CC-ALF on top of it (CC-ALF of a row only
            // reads luma and its own samples).
            for rx in 0..sh.w_ctbs {
                let c = (ry * sh.w_ctbs + rx) as usize;
                let (Some(a), Some(s)) = (ctx.alf.get(c), sh.slice(c)) else {
                    continue;
                };
                let on = if ci == 1 { s.alf.cb } else { s.alf.cr };
                if !a.enabled[ci] || !s.alf.enabled || !on {
                    continue;
                }
                let Some(ap) = aps(s.alf.chroma_aps) else {
                    continue;
                };
                let alt = usize::from(a.chroma_alt[ci - 1]);
                let (Some(fc), Some(cc)) = (ap.chroma_coeff.get(alt), ap.chroma_clip.get(alt))
                else {
                    continue;
                };
                let f = fc.map(i32::from);
                let cl = cc.map(|k| clip_value(bd, k));
                chroma_ctb(sh, src, &mut band, stride, (sx, sy), rx, ry, &f, &cl, bd);
            }
            for rx in 0..sh.w_ctbs {
                let c = (ry * sh.w_ctbs + rx) as usize;
                let (Some(a), Some(s)) = (ctx.alf.get(c), sh.slice(c)) else {
                    continue;
                };
                let idc = usize::from(a.cc[ci - 1]);
                let (on, id) = if ci == 1 {
                    (s.alf.cc_cb, s.alf.cc_cb_aps)
                } else {
                    (s.alf.cc_cr, s.alf.cc_cr_aps)
                };
                if idc == 0 || !on {
                    continue;
                }
                let Some(ap) = aps(id) else { continue };
                let list = if ci == 1 {
                    &ap.cc_cb_coeff
                } else {
                    &ap.cc_cr_coeff
                };
                let Some(f) = list.get(idc - 1) else { continue };
                cc_ctb(
                    sh,
                    luma,
                    lstride,
                    &mut band,
                    stride,
                    (sx, sy),
                    rx,
                    ry,
                    f,
                    bd,
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad_free(w: i32, h: i32) -> Pad {
        Pad {
            pic_w: w,
            pic_h: h,
            xc: 0,
            yc: 0,
            cw: 64,
            ch: 64,
            top: -128,
            bottom: -128,
            left: -128,
            right: -128,
            tl: false,
            br: false,
        }
    }

    #[test]
    fn classification_of_flat_and_striped_blocks() {
        let w = 32usize;
        let flat = vec![512u16; w * w];
        let p = pad_free(32, 32);
        // No activity: class 0; ties pick dirHV 3 and dirD 2, so transposeIdx 3.
        assert_eq!(classify(&flat, w, &p, 8, 8, 8, 64, false, 10), (0, 3));
        // Vertical stripes: strong horizontal gradient, dirHV 3 (horizontal activity).
        let stripes: Vec<u16> = (0..w * w)
            .map(|i| if (i % w).is_multiple_of(2) { 0 } else { 1023 })
            .collect();
        let (filt, tr) = classify(&stripes, w, &p, 8, 8, 8, 64, false, 10);
        // Activity saturates (avgVar 4), dirS 2, dir1 = dirHV = 3: 4 + ((1 << 1) + 2) * 5.
        assert_eq!(filt, 24);
        assert_eq!(tr, [0, 1, 0, 2, 2, 3, 1, 3][3 * 2 + (2 >> 1)]);
    }

    #[test]
    fn padding_clamps_to_boundaries() {
        let mut p = pad_free(100, 100);
        p.top = 60;
        p.bottom = 64;
        assert_eq!(p.apply(5, 58), (5, 60));
        assert_eq!(p.apply(-3, 70), (0, 63));
        assert_eq!(FIX_FILT_COEFF[0], [0, 0, 2, -3, 1, -4, 1, 7, -1, 1, -1, 5]);
        assert_eq!(CLASS_TO_FILT_MAP[0][0], 8);
    }
}
