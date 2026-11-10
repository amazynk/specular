//! Concealment of coding tree blocks that could not be decoded.
//!
//! Not part of the standard. A lost CTB is filled from what is above it (the
//! last sample row, repeated down), else from its left (the last sample
//! column, repeated across), else with mid-grey; it is visited in raster
//! order so a run of lost CTBs inherits from the first good one. Its side
//! information is reset so that the loop filters treat it as a flat block
//! that is neither deblocked nor offset itself.

use super::geom::Geometry;
use super::inter::RefPic;
use crate::filter::{CtbFilter, FilterInfo};
use crate::picture::{Picture, Plane};

/// What a lost CTB is attributed to.
#[derive(Clone, Copy)]
pub struct LostCtb {
    /// `SliceAddrRs` of the slice it belonged to, or its own address when no
    /// slice covered it.
    pub slice_addr: u32,
    /// The QP the deblocking filter sees for it.
    pub qp: i32,
}

/// Fills the `w` x `h` block at `(x0, y0)` of `p`.
fn fill(p: &mut Plane, x0: usize, y0: usize, w: usize, h: usize, grey: u16) {
    if w == 0 || h == 0 {
        return;
    }
    if y0 > 0 {
        let src = p.offset(x0, y0 - 1);
        for j in 0..h {
            let dst = p.offset(x0, y0 + j);
            p.data.copy_within(src..src + w, dst);
        }
    } else if x0 > 0 {
        for j in 0..h {
            let v = p.at(x0 - 1, y0 + j);
            let o = p.offset(x0, y0 + j);
            p.data[o..o + w].fill(v);
        }
    } else {
        for j in 0..h {
            let o = p.offset(x0, y0 + j);
            p.data[o..o + w].fill(grey);
        }
    }
}

/// Copies the `w` x `h` block at `(x0, y0)` of plane `c` of `src` into `p`,
/// when the two planes have the same size; false when they do not.
fn copy_from(
    p: &mut Plane,
    src: &RefPic,
    c: usize,
    (x0, y0): (usize, usize),
    (w, h): (usize, usize),
) -> bool {
    if src.dims(c) != (p.width, p.height) || p.oy != 0 {
        return false;
    }
    if h == 0 {
        return true;
    }
    src.wait_line(c, y0 + h - 1);
    for j in 0..h {
        let o = p.offset(x0, y0 + j);
        p.data[o..o + w].copy_from_slice(&src.line(c, y0 + j)[x0..x0 + w]);
    }
    true
}

/// Conceals every CTB for which `lost[rs]` is `Some`: from `src`, the
/// previous picture of a sequence, when given, else from the neighbours.
pub fn conceal(
    pic: &mut Picture,
    fi: &mut FilterInfo,
    geo: &Geometry,
    lost: &[Option<LostCtb>],
    bit_depth: (u8, u8),
    sub: (usize, usize),
    src: Option<&RefPic>,
) {
    for (rs, l) in lost.iter().enumerate() {
        if let Some(l) = l {
            conceal_ctb(pic, fi, geo, rs, *l, bit_depth, sub, src);
        }
    }
}

/// Conceals the CTB at raster address `rs`. The CTBs above it and left of
/// it must be final (decoded or concealed) and not yet loop-filtered.
#[allow(clippy::too_many_arguments)]
pub fn conceal_ctb(
    pic: &mut Picture,
    fi: &mut FilterInfo,
    geo: &Geometry,
    rs: usize,
    l: LostCtb,
    bit_depth: (u8, u8),
    sub: (usize, usize),
    src: Option<&RefPic>,
) {
    let ctb = 1usize << geo.ctb_log2;
    let (width, height) = (pic.y.width, pic.y.height);
    let grey_y = 1u16 << (bit_depth.0 - 1);
    let grey_c = 1u16 << (bit_depth.1 - 1);
    let (cx, cy) = (rs % geo.w_ctbs, rs / geo.w_ctbs);
    let (x0, y0) = (cx * ctb, cy * ctb);
    let (x1, y1) = ((x0 + ctb).min(width), (y0 + ctb).min(height));
    if !src.is_some_and(|s| copy_from(&mut pic.y, s, 0, (x0, y0), (x1 - x0, y1 - y0))) {
        fill(&mut pic.y, x0, y0, x1 - x0, y1 - y0, grey_y);
    }
    if let (sw @ 1.., sh @ 1..) = sub {
        let (cw, ch) = (pic.cb.width, pic.cb.height);
        let (a, b) = (x0 / sw, y0 / sh);
        let c = if x1 == width { cw } else { x1 / sw };
        let d = if y1 == height { ch } else { y1 / sh };
        let (w, h) = (c - a, d - b);
        if !src.is_some_and(|s| {
            copy_from(&mut pic.cb, s, 1, (a, b), (w, h))
                && copy_from(&mut pic.cr, s, 2, (a, b), (w, h))
        }) {
            fill(&mut pic.cb, a, b, w, h, grey_c);
            fill(&mut pic.cr, a, b, w, h, grey_c);
        }
    }
    let qp = l.qp.clamp(-128, 127) as i8;
    for y4 in (y0 / 4)..y1.div_ceil(4) {
        let row = (y4 - pic.oy4) * pic.min4_w;
        for x4 in (x0 / 4)..x1.div_ceil(4) {
            pic.qp_y[row + x4] = qp;
            pic.cu_flags[row + x4] = 0;
            if let Some(m) = pic.mvf.get_mut(row + x4) {
                *m = Default::default();
            }
        }
    }
    for y8 in (y0 / 8)..y1.div_ceil(8) {
        let row = (y8 - pic.oy8) * pic.min8_w;
        pic.edges[row + x0 / 8..row + x1.div_ceil(8)].fill(0);
    }
    fi.ctb[rs] = CtbFilter::default();
    fi.slice_addr[rs] = l.slice_addr;
}
