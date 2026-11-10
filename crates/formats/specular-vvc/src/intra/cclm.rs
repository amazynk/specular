//! Cross-component linear model prediction (8.4.5.2.14).

use crate::picture::Plane;

use super::{L_CCLM, LT_CCLM, T_CCLM};

/// Inputs gathered by the caller (availability needs the decoder's maps).
pub struct CclmIn<'a> {
    pub mode: i32,
    /// Chroma block position and size, in chroma samples.
    pub xc: usize,
    pub yc: usize,
    pub w: usize,
    pub h: usize,
    pub avail_l: bool,
    pub avail_t: bool,
    pub num_top_right: usize,
    pub num_left_below: usize,
    /// Chroma references: `top[ 1 + x ]` = p[ x ][ −1 ], `left[ 1 + y ]` = p[ −1 ][ y ].
    pub top: &'a [i32],
    pub left: &'a [i32],
    pub sub_w: usize,
    pub sub_h: usize,
    pub vertical_collocated: bool,
    pub ctb_size: usize,
    pub bit_depth: u32,
}

const DIV_SIG: [i32; 16] = [0, 7, 6, 5, 5, 4, 4, 3, 3, 2, 2, 1, 1, 1, 1, 0];

pub fn predict(c: &CclmIn<'_>, luma: &Plane, out: &mut [i32]) {
    let (w, h) = (c.w, c.h);
    let (sw, sh) = (c.sub_w, c.sub_h);
    let num_t = if c.mode == LT_CCLM {
        if c.avail_t { w } else { 0 }
    } else if c.avail_t && c.mode == T_CCLM {
        w + c.num_top_right.min(h)
    } else {
        0
    };
    let num_l = if c.mode == LT_CCLM {
        if c.avail_l { h } else { 0 }
    } else if c.avail_l && c.mode == L_CCLM {
        h + c.num_left_below.min(w)
    } else {
        0
    };
    let max = (1i32 << c.bit_depth) - 1;
    if num_t == 0 && num_l == 0 {
        out[..w * h].fill(1 << (c.bit_depth - 1));
        return;
    }
    let xl = (c.xc * sw) as isize;
    let yl = (c.yc * sh) as isize;
    let ctb_boundary = (yl as usize) & (c.ctb_size - 1) == 0;
    // pY with the substitutions of step 2: positions outside the block on a missing side copy
    // the block's own first row or column.
    let py = |x: isize, y: isize| -> i32 {
        let (mut x, mut y) = (x, y);
        if y < 0 && !c.avail_t {
            y = 0;
        }
        if x < 0 && !c.avail_l {
            x = 0;
        }
        i32::from(luma.at((xl + x) as usize, (yl + y) as usize))
    };
    let ds = |x: isize, y: isize| -> i32 {
        if sw == 1 && sh == 1 {
            py(x, y)
        } else if sh == 1 {
            (py(sw as isize * x - 1, y)
                + 2 * py(sw as isize * x, y)
                + py(sw as isize * x + 1, y)
                + 2)
                >> 2
        } else {
            let (bx, by) = (sw as isize * x, sh as isize * y);
            if c.vertical_collocated {
                (py(bx, by - 1)
                    + py(bx - 1, by)
                    + 4 * py(bx, by)
                    + py(bx + 1, by)
                    + py(bx, by + 1)
                    + 4)
                    >> 3
            } else {
                (py(bx - 1, by)
                    + py(bx - 1, by + 1)
                    + 2 * py(bx, by)
                    + 2 * py(bx, by + 1)
                    + py(bx + 1, by)
                    + py(bx + 1, by + 1)
                    + 4)
                    >> 3
            }
        }
    };
    let both = c.avail_t && c.avail_l && c.mode == LT_CCLM;
    let is4 = if both { 0 } else { 1 };
    let mut sel_y = [0i32; 4];
    let mut sel_c = [0i32; 4];
    let mut n = 0;
    let pick = |num: usize, avail: bool, ok_mode: bool| -> (usize, usize, usize) {
        if avail && ok_mode {
            let start = num >> (2 + is4);
            let step = (num >> (1 + is4)).max(1);
            let cnt = num.min((1 + is4) << 1);
            (cnt, start, step)
        } else {
            (0, 0, 1)
        }
    };
    let (cnt_t, st_t, sp_t) = pick(num_t, c.avail_t, c.mode == LT_CCLM || c.mode == T_CCLM);
    let (cnt_l, st_l, sp_l) = pick(num_l, c.avail_l, c.mode == LT_CCLM || c.mode == L_CCLM);
    if num_t > 0 {
        for i in 0..cnt_t {
            let x = (st_t + i * sp_t) as isize;
            sel_c[n] = c.top[1 + x as usize];
            sel_y[n] = if sw == 1 && sh == 1 {
                py(x, -1)
            } else if sh != 1 && !ctb_boundary {
                let bx = sw as isize * x;
                if c.vertical_collocated {
                    (py(bx, -3) + py(bx - 1, -2) + 4 * py(bx, -2) + py(bx + 1, -2) + py(bx, -1) + 4)
                        >> 3
                } else {
                    (py(bx - 1, -1)
                        + py(bx - 1, -2)
                        + 2 * py(bx, -1)
                        + 2 * py(bx, -2)
                        + py(bx + 1, -1)
                        + py(bx + 1, -2)
                        + 4)
                        >> 3
                }
            } else {
                let bx = sw as isize * x;
                (py(bx - 1, -1) + 2 * py(bx, -1) + py(bx + 1, -1) + 2) >> 2
            };
            n += 1;
        }
    }
    if num_l > 0 {
        for i in 0..cnt_l {
            let y = (st_l + i * sp_l) as isize;
            sel_c[n] = c.left[1 + y as usize];
            let s = sw as isize;
            sel_y[n] = if sw == 1 && sh == 1 {
                py(-1, y)
            } else if sh == 1 {
                (py(-1 - s, y) + 2 * py(-s, y) + py(1 - s, y) + 2) >> 2
            } else {
                let by = sh as isize * y;
                if c.vertical_collocated {
                    (py(-s, by - 1)
                        + py(-1 - s, by)
                        + 4 * py(-s, by)
                        + py(1 - s, by)
                        + py(-s, by + 1)
                        + 4)
                        >> 3
                } else {
                    (py(-1 - s, by)
                        + py(-1 - s, by + 1)
                        + 2 * py(-s, by)
                        + 2 * py(-s, by + 1)
                        + py(1 - s, by)
                        + py(1 - s, by + 1)
                        + 4)
                        >> 3
                }
            };
            n += 1;
        }
    }
    if n == 2 {
        sel_y[3] = sel_y[0];
        sel_y[2] = sel_y[1];
        sel_y[0] = sel_y[1];
        sel_y[1] = sel_y[3];
        sel_c[3] = sel_c[0];
        sel_c[2] = sel_c[1];
        sel_c[0] = sel_c[1];
        sel_c[1] = sel_c[3];
    }
    let mut mn = [0usize, 2];
    let mut mx = [1usize, 3];
    if sel_y[mn[0]] > sel_y[mn[1]] {
        mn.swap(0, 1);
    }
    if sel_y[mx[0]] > sel_y[mx[1]] {
        mx.swap(0, 1);
    }
    if sel_y[mn[0]] > sel_y[mx[1]] {
        core::mem::swap(&mut mn, &mut mx);
    }
    if sel_y[mn[1]] > sel_y[mx[0]] {
        core::mem::swap(&mut mn[1], &mut mx[0]);
    }
    let max_y = (sel_y[mx[0]] + sel_y[mx[1]] + 1) >> 1;
    let max_c = (sel_c[mx[0]] + sel_c[mx[1]] + 1) >> 1;
    let min_y = (sel_y[mn[0]] + sel_y[mn[1]] + 1) >> 1;
    let min_c = (sel_c[mn[0]] + sel_c[mn[1]] + 1) >> 1;
    let diff = max_y - min_y;
    let (a, b, k) = if diff != 0 {
        let diff_c = max_c - min_c;
        let mut x = 31 - (diff as u32).leading_zeros() as i32;
        let norm = ((diff << 4) >> x) & 15;
        x += i32::from(norm != 0);
        let y = if diff_c != 0 {
            32 - (diff_c.unsigned_abs()).leading_zeros() as i32
        } else {
            0
        };
        let add = (1 << y) >> 1;
        let mut a = (diff_c * (DIV_SIG[norm as usize] | 8) + add) >> y;
        let k = if 3 + x - y < 1 { 1 } else { 3 + x - y };
        if 3 + x - y < 1 {
            a = a.signum() * 15;
        }
        let b = min_c - ((a * min_y) >> k);
        (a, b, k)
    } else {
        (0, min_c, 0)
    };
    for y in 0..h {
        for x in 0..w {
            let v = ((ds(x as isize, y as isize) * a) >> k) + b;
            out[y * w + x] = v.clamp(0, max);
        }
    }
}
