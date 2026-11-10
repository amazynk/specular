//! Intra prediction (8.3): 4x4 and 8x8 luma with the nine directional modes, 16x16 luma
//! and 4:2:0 / 4:2:2 chroma. Predictions are written straight into the picture buffer; the
//! residual is added on top afterwards. `max` is the largest sample value; mid-grey (its
//! half, rounded up) stands in for neighbours that are not available.

use crate::pixel::Pix;

/// Which neighbouring samples are available.
#[derive(Clone, Copy, Default)]
pub struct Avail {
    pub left: bool,
    pub top: bool,
    pub top_right: bool,
    pub top_left: bool,
}

#[inline(always)]
fn avg2(a: i32, b: i32) -> i32 {
    (a + b + 1) >> 1
}

#[inline(always)]
fn avg3(a: i32, b: i32, c: i32) -> i32 {
    (a + 2 * b + c + 2) >> 2
}

/// Gathers top (2N samples, the second half substituted when the top-right is missing),
/// left (N samples) and the corner. Missing samples read as mid-grey (never used by a
/// conforming stream).
fn edges<const N: usize, P: Pix>(
    buf: &[P],
    off: usize,
    stride: usize,
    a: Avail,
    grey: i32,
) -> ([i32; 16], [i32; 8], i32) {
    let mut top = [grey; 16];
    let mut left = [grey; 8];
    let mut corner = grey;
    if a.top {
        let t = off - stride;
        for (d, s) in top.iter_mut().zip(&buf[t..t + N]) {
            *d = s.i();
        }
        if a.top_right {
            for (d, s) in top[N..].iter_mut().zip(&buf[t + N..t + 2 * N]) {
                *d = s.i();
            }
        } else {
            let v = top[N - 1];
            top[N..2 * N].fill(v);
        }
    }
    if a.left {
        for (y, l) in left.iter_mut().enumerate().take(N) {
            *l = buf[off + y * stride - 1].i();
        }
    }
    if a.top_left {
        corner = buf[off - stride - 1].i();
    }
    (top, left, corner)
}

/// The directional modes shared by 4x4 and 8x8 prediction (8.3.1.2, 8.3.2.2) on prepared
/// edge samples: `t[x]` = p[x,-1] for x in 0..2N, `l[y]` = p[-1,y], `c` = p[-1,-1].
#[allow(clippy::too_many_arguments)]
fn directional<const N: usize, P: Pix>(
    buf: &mut [P],
    off: usize,
    stride: usize,
    mode: u8,
    t: &[i32; 16],
    l: &[i32; 8],
    c: i32,
    a: Avail,
    grey: i32,
) {
    // Edge accessors with -1 meaning the corner.
    let tp = |x: isize| if x < 0 { c } else { t[x as usize] };
    let lp = |y: isize| if y < 0 { c } else { l[y as usize] };
    let n = N as isize;
    for y in 0..n {
        let row = off + y as usize * stride;
        for x in 0..n {
            let v = match mode {
                0 => t[x as usize],
                1 => l[y as usize],
                3 => {
                    if x == n - 1 && y == n - 1 {
                        (t[2 * N - 2] + 3 * t[2 * N - 1] + 2) >> 2
                    } else {
                        let i = (x + y) as usize;
                        avg3(t[i], t[i + 1], t[i + 2])
                    }
                }
                4 => {
                    if x > y {
                        avg3(tp(x - y - 2), tp(x - y - 1), tp(x - y))
                    } else if x < y {
                        avg3(lp(y - x - 2), lp(y - x - 1), lp(y - x))
                    } else {
                        avg3(tp(0), c, lp(0))
                    }
                }
                5 => {
                    let z = 2 * x - y;
                    if z >= 0 && z & 1 == 0 {
                        avg2(tp(x - (y >> 1) - 1), tp(x - (y >> 1)))
                    } else if z >= 0 {
                        avg3(tp(x - (y >> 1) - 2), tp(x - (y >> 1) - 1), tp(x - (y >> 1)))
                    } else if z == -1 {
                        avg3(lp(0), c, tp(0))
                    } else {
                        avg3(lp(y - 2 * x - 1), lp(y - 2 * x - 2), lp(y - 2 * x - 3))
                    }
                }
                6 => {
                    let z = 2 * y - x;
                    if z >= 0 && z & 1 == 0 {
                        avg2(lp(y - (x >> 1) - 1), lp(y - (x >> 1)))
                    } else if z >= 0 {
                        avg3(lp(y - (x >> 1) - 2), lp(y - (x >> 1) - 1), lp(y - (x >> 1)))
                    } else if z == -1 {
                        avg3(lp(0), c, tp(0))
                    } else {
                        avg3(tp(x - 2 * y - 1), tp(x - 2 * y - 2), tp(x - 2 * y - 3))
                    }
                }
                7 => {
                    let i = (x + (y >> 1)) as usize;
                    if y & 1 == 0 {
                        avg2(t[i], t[i + 1])
                    } else {
                        avg3(t[i], t[i + 1], t[i + 2])
                    }
                }
                8 => {
                    let z = x + 2 * y;
                    let i = (y + (x >> 1)) as usize;
                    if z > 2 * n - 3 {
                        l[N - 1]
                    } else if z == 2 * n - 3 {
                        (l[N - 2] + 3 * l[N - 1] + 2) >> 2
                    } else if z & 1 == 0 {
                        avg2(l[i], l[i + 1])
                    } else {
                        avg3(l[i], l[i + 1], l[i + 2])
                    }
                }
                _ => {
                    // DC
                    let st: i32 = t[..N].iter().sum();
                    let sl: i32 = l[..N].iter().sum();
                    let sh = N.trailing_zeros();
                    let n = N as i32;
                    let dc = match (a.top, a.left) {
                        (true, true) => (st + sl + n) >> (sh + 1),
                        (true, false) => (st + (n >> 1)) >> sh,
                        (false, true) => (sl + (n >> 1)) >> sh,
                        _ => grey,
                    };
                    for yy in 0..N {
                        buf[off + yy * stride..off + yy * stride + N].fill(P::of(dc));
                    }
                    return;
                }
            };
            buf[row + x as usize] = P::of(v);
        }
    }
}

/// Intra_4x4 prediction of the block at `off`.
pub fn pred4x4<P: Pix>(buf: &mut [P], off: usize, stride: usize, mode: u8, a: Avail, max: i32) {
    let grey = (max + 1) >> 1;
    let (t, l, c) = edges::<4, P>(buf, off, stride, a, grey);
    match mode {
        0 => {
            for y in 0..4 {
                for (d, s) in buf[off + y * stride..off + y * stride + 4]
                    .iter_mut()
                    .zip(&t)
                {
                    *d = P::of(*s);
                }
            }
        }
        1 => {
            for y in 0..4 {
                buf[off + y * stride..off + y * stride + 4].fill(P::of(l[y]));
            }
        }
        _ => directional::<4, P>(buf, off, stride, mode, &t, &l, c, a, grey),
    }
}

/// Intra_8x8 prediction with reference sample filtering (8.3.2.2.1).
pub fn pred8x8<P: Pix>(buf: &mut [P], off: usize, stride: usize, mode: u8, a: Avail, max: i32) {
    let grey = (max + 1) >> 1;
    let (t, l, c) = edges::<8, P>(buf, off, stride, a, grey);
    let mut ft = t;
    let mut fl = l;
    let mut fc = c;
    if a.top {
        ft[0] = if a.top_left {
            avg3(c, t[0], t[1])
        } else {
            (3 * t[0] + t[1] + 2) >> 2
        };
        for x in 1..15 {
            ft[x] = avg3(t[x - 1], t[x], t[x + 1]);
        }
        ft[15] = (t[14] + 3 * t[15] + 2) >> 2;
    }
    if a.top_left {
        fc = match (a.top, a.left) {
            (true, true) => avg3(t[0], c, l[0]),
            (true, false) => (3 * c + t[0] + 2) >> 2,
            (false, true) => (3 * c + l[0] + 2) >> 2,
            (false, false) => c,
        };
    }
    if a.left {
        fl[0] = if a.top_left {
            avg3(c, l[0], l[1])
        } else {
            (3 * l[0] + l[1] + 2) >> 2
        };
        for y in 1..7 {
            fl[y] = avg3(l[y - 1], l[y], l[y + 1]);
        }
        fl[7] = (l[6] + 3 * l[7] + 2) >> 2;
    }
    match mode {
        0 => {
            for y in 0..8 {
                for (d, s) in buf[off + y * stride..off + y * stride + 8]
                    .iter_mut()
                    .zip(&ft)
                {
                    *d = P::of(*s);
                }
            }
        }
        1 => {
            for y in 0..8 {
                buf[off + y * stride..off + y * stride + 8].fill(P::of(fl[y]));
            }
        }
        _ => directional::<8, P>(buf, off, stride, mode, &ft, &fl, fc, a, grey),
    }
}

/// Intra_16x16 prediction (8.3.3).
pub fn pred16x16<P: Pix>(buf: &mut [P], off: usize, stride: usize, mode: u8, a: Avail, max: i32) {
    let grey = P::of((max + 1) >> 1);
    let mut top = [grey; 16];
    let mut left = [grey; 16];
    if a.top {
        top.copy_from_slice(&buf[off - stride..off - stride + 16]);
    }
    if a.left {
        for (y, l) in left.iter_mut().enumerate() {
            *l = buf[off + y * stride - 1];
        }
    }
    match mode {
        0 => {
            for y in 0..16 {
                buf[off + y * stride..off + y * stride + 16].copy_from_slice(&top);
            }
        }
        1 => {
            for y in 0..16 {
                buf[off + y * stride..off + y * stride + 16].fill(left[y]);
            }
        }
        3 => {
            let c = if a.top_left {
                buf[off - stride - 1]
            } else {
                grey
            };
            let tp = |x: isize| if x < 0 { c } else { top[x as usize] }.i();
            let lp = |y: isize| if y < 0 { c } else { left[y as usize] }.i();
            let mut hh = 0i32;
            let mut vv = 0i32;
            for i in 0..8isize {
                hh += (i as i32 + 1) * (tp(8 + i) - tp(6 - i));
                vv += (i as i32 + 1) * (lp(8 + i) - lp(6 - i));
            }
            let aa = 16 * (left[15].i() + top[15].i());
            let b = (5 * hh + 32) >> 6;
            let cc = (5 * vv + 32) >> 6;
            for y in 0..16 {
                for x in 0..16 {
                    let v = (aa + b * (x as i32 - 7) + cc * (y as i32 - 7) + 16) >> 5;
                    buf[off + y * stride + x] = P::clip(v, max);
                }
            }
        }
        _ => {
            let st: i32 = top.iter().map(|v| v.i()).sum();
            let sl: i32 = left.iter().map(|v| v.i()).sum();
            let dc = match (a.top, a.left) {
                (true, true) => P::of((st + sl + 16) >> 5),
                (true, false) => P::of((st + 8) >> 4),
                (false, true) => P::of((sl + 8) >> 4),
                _ => grey,
            };
            for y in 0..16 {
                buf[off + y * stride..off + y * stride + 16].fill(dc);
            }
        }
    }
}

/// Chroma prediction of one 8x8 (4:2:0) or 8x16 (4:2:2, `h` = 16) plane block (8.3.4);
/// modes 0 DC, 1 horizontal, 2 vertical, 3 plane. `halves` tells which halves of the left
/// edge are available: in an MBAFF frame they can belong to different macroblocks, and DC
/// prediction uses each 4x4 block's part on its own; `a.left` means both.
#[allow(clippy::too_many_arguments)]
pub fn pred_chroma<P: Pix>(
    buf: &mut [P],
    off: usize,
    stride: usize,
    mode: u8,
    a: Avail,
    halves: [bool; 2],
    h: usize,
    max: i32,
) {
    let grey = (max + 1) >> 1;
    let mut top = [grey; 8];
    let mut left = [grey; 16];
    if a.top {
        for (d, s) in top.iter_mut().zip(&buf[off - stride..off - stride + 8]) {
            *d = s.i();
        }
    }
    let half = h / 2;
    for (y, l) in left.iter_mut().enumerate().take(h) {
        if halves[y / half] {
            *l = buf[off + y * stride - 1].i();
        }
    }
    match mode {
        1 => {
            for y in 0..h {
                buf[off + y * stride..off + y * stride + 8].fill(P::of(left[y]));
            }
        }
        2 => {
            for y in 0..h {
                for (d, s) in buf[off + y * stride..off + y * stride + 8]
                    .iter_mut()
                    .zip(&top)
                {
                    *d = P::of(*s);
                }
            }
        }
        3 => {
            let c = if a.top_left {
                buf[off - stride - 1].i()
            } else {
                grey
            };
            let tp = |x: isize| if x < 0 { c } else { top[x as usize] };
            let lp = |y: isize| if y < 0 { c } else { left[y as usize] };
            // yCF (8.3.4.4): 4 for 4:2:2, where V sums 8 terms.
            let ycf = (h as isize - 8) / 2;
            let mut hh = 0i32;
            let mut vv = 0i32;
            for i in 0..4isize {
                hh += (i as i32 + 1) * (tp(4 + i) - tp(2 - i));
            }
            for i in 0..4 + ycf {
                vv += (i as i32 + 1) * (lp(4 + ycf + i) - lp(2 + ycf - i));
            }
            let aa = 16 * (left[h - 1] + top[7]);
            let b = (34 * hh + 32) >> 6;
            let cc = ((if h == 16 { 5 } else { 34 }) * vv + 32) >> 6;
            let yo = 3 + ycf as i32;
            for y in 0..h {
                for x in 0..8 {
                    let v = (aa + b * (x as i32 - 3) + cc * (y as i32 - yo) + 16) >> 5;
                    buf[off + y * stride + x] = P::clip(v, max);
                }
            }
        }
        _ => {
            for by in 0..h / 4 {
                for bx in 0..2 {
                    let lh = halves[by * 4 / half];
                    let st: i32 = top[bx * 4..bx * 4 + 4].iter().sum();
                    let sl: i32 = left[by * 4..by * 4 + 4].iter().sum();
                    let both = (st + sl + 4) >> 3;
                    let t = (st + 2) >> 2;
                    let l = (sl + 2) >> 2;
                    let dc = if (bx == 0) == (by == 0) {
                        // The top-left block and those off both edges: both edges.
                        match (a.top, lh) {
                            (true, true) => both,
                            (true, false) => t,
                            (false, true) => l,
                            _ => grey,
                        }
                    } else if bx == 1 {
                        // The top-right block prefers the top edge.
                        if a.top {
                            t
                        } else if lh {
                            l
                        } else {
                            grey
                        }
                    } else if lh {
                        // Left-column blocks below the top prefer the left edge.
                        l
                    } else if a.top {
                        t
                    } else {
                        grey
                    };
                    for y in 0..4 {
                        let o = off + (by * 4 + y) * stride + bx * 4;
                        buf[o..o + 4].fill(P::of(dc));
                    }
                }
            }
        }
    }
}
