//! Deblocking boundary strength of a picture with inter blocks (8.7.2.4).
//!
//! Derived once the whole picture, or a CTB row of it, is final, for every
//! edge segment the parsers marked, into [`Picture::bs`].

use crate::picture::{MvField, Picture, edge, flags};

/// True when two motion vectors differ by a whole luma sample or more.
#[inline]
fn far(a: [i16; 2], b: [i16; 2]) -> bool {
    (i32::from(a[0]) - i32::from(b[0])).abs() >= 4 || (i32::from(a[1]) - i32::from(b[1])).abs() >= 4
}

/// The motion part of the derivation: 1 when the two blocks predict from
/// different pictures, a different number of vectors, or vectors a sample
/// or more apart.
fn motion(p: &MvField, q: &MvField) -> u8 {
    let np = usize::from(p.pred(0)) + usize::from(p.pred(1));
    let nq = usize::from(q.pred(0)) + usize::from(q.pred(1));
    if np != nq {
        return 1;
    }
    if np == 1 {
        let lp = usize::from(!p.pred(0));
        let lq = usize::from(!q.pred(0));
        if p.ref_id[lp] != q.ref_id[lq] {
            return 1;
        }
        return u8::from(far(p.mv[lp], q.mv[lq]));
    }
    let (p0, p1, q0, q1) = (p.ref_id[0], p.ref_id[1], q.ref_id[0], q.ref_id[1]);
    if !((p0 == q0 && p1 == q1) || (p0 == q1 && p1 == q0)) {
        return 1;
    }
    let straight = far(p.mv[0], q.mv[0]) || far(p.mv[1], q.mv[1]);
    let crossed = far(p.mv[0], q.mv[1]) || far(p.mv[1], q.mv[0]);
    u8::from(if p0 != p1 {
        if p0 == q0 { straight } else { crossed }
    } else {
        straight && crossed
    })
}

/// The boundary strength between 4x4 blocks `p` and `q` (indices into the
/// per-4x4 arrays); `tu` when the edge is a transform block edge.
fn strength(pic: &Picture, p: usize, q: usize, tu: bool) -> u8 {
    let (fp, fq) = (pic.cu_flags[p], pic.cu_flags[q]);
    if fp & flags::INTER == 0 || fq & flags::INTER == 0 {
        return 2;
    }
    if tu && (fp | fq) & flags::NZ != 0 {
        return 1;
    }
    motion(&pic.mvf[p], &pic.mvf[q])
}

/// Fills [`Picture::bs`] (allocated, zero) for the marked edges of the 8x8
/// grid rows `by0..by1`. Reads the motion of those rows and of the row of
/// 4x4 blocks above them.
pub fn derive_rows(pic: &mut Picture, by0: usize, by1: usize) {
    let n4 = pic.cu_flags.len();
    if pic.mvf.len() != n4 || pic.bs.len() != n4 {
        return;
    }
    let mut bs = core::mem::take(&mut pic.bs);
    let (w4, h4) = (pic.min4_w, pic.min4_h);
    for by in by0..by1.min(pic.min8_h) {
        for bx in 0..pic.min8_w {
            let e = pic.edges[by * pic.min8_w + bx];
            if e == 0 {
                continue;
            }
            for k in 0..2 {
                // Vertical edge: the two 4-row segments of the cell's left side.
                let (x4, y4) = (bx * 2, by * 2 + k);
                if e & (edge::VER | edge::PU_VER) != 0 && x4 > 0 && y4 < h4 {
                    let q = y4 * w4 + x4;
                    bs[q] |= strength(pic, q - 1, q, e & edge::VER != 0);
                }
                // Horizontal edge: the two 4-column segments of its top side.
                let (x4, y4) = (bx * 2 + k, by * 2);
                if e & (edge::HOR | edge::PU_HOR) != 0 && y4 > 0 && x4 < w4 {
                    let q = y4 * w4 + x4;
                    bs[q] |= strength(pic, q - w4, q, e & edge::HOR != 0) << 2;
                }
            }
        }
    }
    pic.bs = bs;
}
