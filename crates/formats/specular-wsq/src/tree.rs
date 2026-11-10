//! The decomposition geometry: the 20 regions the wavelet transform splits (each region is
//! synthesised from its four quarters, the deepest first) and the 64 subbands the quantised
//! coefficients are stored in, in coding order. Odd lengths give one half a sample more than
//! the other; which half depends on the band, as the specification lays it out.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Region {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

/// A region of the transform tree, with whether its high band comes first along rows
/// (`inv_rw`) or columns (`inv_cl`).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Node {
    pub r: Region,
    pub inv_rw: bool,
    pub inv_cl: bool,
}

/// `len` cut in two; for odd lengths the larger part first when `larger_first`.
fn split(len: usize, larger_first: bool) -> (usize, usize) {
    if len.is_multiple_of(2) {
        (len / 2, len / 2)
    } else if larger_first {
        (len.div_ceil(2), len / 2)
    } else {
        (len / 2, len.div_ceil(2))
    }
}

/// Quarters of `parent` at `t[q..q + 4]` (only three when `three`): top-left, top-right,
/// bottom-left, bottom-right.
fn quarters(t: &mut [Node; 20], p: usize, q: usize, parent: Region, three: bool) {
    t[p].r = parent;
    let Region { x, y, w, h } = parent;
    let (a, b) = split(w, p != 4);
    let (c, d) = split(h, p != 5);
    t[q].r = Region { x, y, w: a, h: c };
    t[q + 1].r = Region {
        x: x + a,
        y,
        w: b,
        h: c,
    };
    t[q + 2].r = Region {
        x,
        y: y + c,
        w: a,
        h: d,
    };
    if !three {
        t[q + 3].r = Region {
            x: x + a,
            y: y + c,
            w: b,
            h: d,
        };
    }
}

pub(crate) fn wavelet_tree(width: usize, height: usize) -> [Node; 20] {
    let mut t = [Node::default(); 20];
    for i in [2, 4, 7, 9, 11, 13, 16, 18] {
        t[i].inv_rw = true;
    }
    for i in [3, 5, 8, 9, 12, 13, 17, 18] {
        t[i].inv_cl = true;
    }
    quarters(
        &mut t,
        0,
        1,
        Region {
            x: 0,
            y: 0,
            w: width,
            h: height,
        },
        true,
    );
    let (lx, lx2) = split(t[1].r.w, true);
    let (ly, ly2) = split(t[1].r.h, true);
    quarters(
        &mut t,
        4,
        6,
        Region {
            x: lx,
            y: 0,
            w: lx2,
            h: ly,
        },
        false,
    );
    quarters(
        &mut t,
        5,
        10,
        Region {
            x: 0,
            y: ly,
            w: lx,
            h: ly2,
        },
        false,
    );
    quarters(
        &mut t,
        14,
        15,
        Region {
            x: 0,
            y: 0,
            w: lx,
            h: ly,
        },
        false,
    );
    t[19].r = Region {
        x: 0,
        y: 0,
        w: t[15].r.w.div_ceil(2),
        h: t[15].r.h.div_ceil(2),
    };
    t
}

/// Four subbands of a region, larger halves first.
fn four(q: &mut [Region; 64], p: usize, r: Region) {
    let (a, b) = split(r.w, true);
    let (c, d) = split(r.h, true);
    q[p] = Region {
        x: r.x,
        y: r.y,
        w: a,
        h: c,
    };
    q[p + 1] = Region {
        x: r.x + a,
        y: r.y,
        w: b,
        h: c,
    };
    q[p + 2] = Region {
        x: r.x,
        y: r.y + c,
        w: a,
        h: d,
    };
    q[p + 3] = Region {
        x: r.x + a,
        y: r.y + c,
        w: b,
        h: d,
    };
}

/// Sixteen subbands of a region: its four quarters, each in four. `rw` / `cl`: the region is
/// a high band along columns / rows, so its odd halves fall the other way.
fn sixteen(q: &mut [Region; 64], p: usize, r: Region, rw: bool, cl: bool) {
    let (tx, t2x) = split(r.w, !cl);
    let (ty, t2y) = split(r.h, !rw);
    let (a, b) = split(tx, true);
    let (c, d) = split(ty, true);
    let (a2, b2) = split(t2x, false);
    let (c2, d2) = split(t2y, false);
    let (x, y) = (r.x, r.y);
    let mut put = |i: usize, x: usize, y: usize, w: usize, h: usize| {
        q[p + i] = Region { x, y, w, h };
    };
    put(0, x, y, a, c);
    put(1, x + a, y, b, c);
    put(2, x, y + c, a, d);
    put(3, x + a, y + c, b, d);
    put(4, x + tx, y, a2, c);
    put(5, x + tx + a2, y, b2, c);
    put(6, x + tx, y + c, a2, d);
    put(7, x + tx + a2, y + c, b2, d);
    put(8, x, y + ty, a, c2);
    put(9, x + a, y + ty, b, c2);
    put(10, x, y + ty + c2, a, d2);
    put(11, x + a, y + ty + c2, b, d2);
    put(12, x + tx, y + ty, a2, c2);
    put(13, x + tx + a2, y + ty, b2, c2);
    put(14, x + tx, y + ty + c2, a2, d2);
    put(15, x + tx + a2, y + ty + c2, b2, d2);
}

/// The 64 subbands; the first 60 carry coefficients.
pub(crate) fn subbands(w: &[Node; 20]) -> [Region; 64] {
    let mut q = [Region::default(); 64];
    // Later calls overwrite parts of earlier ones: subband 3 is refined by the last call,
    // and 48–50 by the one at 35.
    sixteen(&mut q, 3, w[14].r, false, false);
    sixteen(&mut q, 19, w[4].r, false, true);
    sixteen(&mut q, 48, w[0].r, false, false);
    sixteen(&mut q, 35, w[5].r, true, false);
    four(&mut q, 0, w[19].r);
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 60 coded subbands and the discarded first-level diagonal band tile the image.
    #[test]
    fn subbands_tile_the_image() {
        for (w, h) in [
            (512, 512),
            (545, 622),
            (301, 257),
            (77, 100),
            (64, 64),
            (1001, 999),
        ] {
            let t = wavelet_tree(w, h);
            let q = subbands(&t);
            let mut cover = vec![0u8; w * h];
            for r in q.iter().take(60) {
                for yy in r.y..r.y + r.h {
                    for xx in r.x..r.x + r.w {
                        cover[yy * w + xx] += 1;
                    }
                }
            }
            // The first-level diagonal (HH) band is not coded.
            let (a, _) = split(w, true);
            let (c, _) = split(h, true);
            for yy in 0..h {
                for xx in 0..w {
                    let want = u8::from(!(xx >= a && yy >= c));
                    assert_eq!(cover[yy * w + xx], want, "{w}x{h} at {xx},{yy}");
                }
            }
        }
    }
}
