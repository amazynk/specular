//! Intra prediction (section 8.5.1).

use crate::frame::Pixel;

pub(crate) const DC_PRED: u8 = 0;
pub(crate) const V_PRED: u8 = 1;
pub(crate) const H_PRED: u8 = 2;
pub(crate) const D45_PRED: u8 = 3;
pub(crate) const D135_PRED: u8 = 4;
pub(crate) const D117_PRED: u8 = 5;
pub(crate) const D207_PRED: u8 = 7;
pub(crate) const D63_PRED: u8 = 8;
pub(crate) const TM_PRED: u8 = 9;

/// Where to predict: a tile's rows of a plane (columns from `x0`), the block's position in
/// plane samples and the edges of the decoded area.
pub(crate) struct IntraArgs {
    pub x: usize,
    pub y: usize,
    pub x0: usize,
    pub have_left: bool,
    pub have_above: bool,
    pub not_on_right: bool,
    pub tx_sz: usize,
    pub mode: u8,
    pub max_x: usize,
    pub max_y: usize,
    pub bit_depth: u8,
}

#[inline(always)]
fn r2(a: i32, b: i32) -> i32 {
    (a + b + 1) >> 1
}

#[inline(always)]
fn r3(a: i32, b: i32, c: i32) -> i32 {
    (a + 2 * b + c + 2) >> 2
}

pub(crate) fn predict<P: Pixel>(rows: &mut [&mut [P]], a: &IntraArgs) {
    let size = 4usize << a.tx_sz;
    let log2 = a.tx_sz as u32 + 2;
    let base = 1i32 << (a.bit_depth - 1);
    let (x, y, x0) = (a.x, a.y, a.x0);
    // above[k + 1] = aboveRow[k] for k = -1..2*size-1.
    let mut above = [0i32; 65];
    let mut left = [0i32; 32];
    if a.have_above {
        let row = &rows[y - 1];
        for i in 0..size {
            above[i + 1] = row[a.max_x.min(x + i) - x0].get();
        }
        if a.not_on_right && a.tx_sz == 0 {
            for i in size..2 * size {
                above[i + 1] = row[a.max_x.min(x + i) - x0].get();
            }
        } else {
            for i in size..2 * size {
                above[i + 1] = above[size];
            }
        }
        above[0] = if a.have_left {
            row[a.max_x.min(x - 1) - x0].get()
        } else {
            base + 1
        };
    } else {
        above[..=2 * size].fill(base - 1);
    }
    if a.have_left {
        for (i, l) in left.iter_mut().enumerate().take(size) {
            *l = rows[a.max_y.min(y + i)][x - 1 - x0].get();
        }
    } else {
        left[..size].fill(base + 1);
    }
    let ab = |k: isize| above[(k + 1) as usize];
    let max = (1 << a.bit_depth) - 1;
    let xo = x - x0;
    match a.mode {
        V_PRED => {
            for i in 0..size {
                for (j, d) in rows[y + i][xo..xo + size].iter_mut().enumerate() {
                    *d = P::put(above[j + 1]);
                }
            }
        }
        H_PRED => {
            for i in 0..size {
                let v = P::put(left[i]);
                rows[y + i][xo..xo + size].fill(v);
            }
        }
        TM_PRED => {
            for i in 0..size {
                for (j, d) in rows[y + i][xo..xo + size].iter_mut().enumerate() {
                    *d = P::put((above[j + 1] + left[i] - above[0]).clamp(0, max));
                }
            }
        }
        DC_PRED => {
            let v = match (a.have_left, a.have_above) {
                (true, true) => {
                    let s: i32 =
                        left[..size].iter().sum::<i32>() + above[1..=size].iter().sum::<i32>();
                    (s + size as i32) >> (log2 + 1)
                }
                (true, false) => {
                    let s: i32 = left[..size].iter().sum();
                    (s + (1 << (log2 - 1))) >> log2
                }
                (false, true) => {
                    let s: i32 = above[1..=size].iter().sum();
                    (s + (1 << (log2 - 1))) >> log2
                }
                (false, false) => base,
            };
            let v = P::put(v);
            for i in 0..size {
                rows[y + i][xo..xo + size].fill(v);
            }
        }
        _ => {
            let mut pred = [[0i32; 32]; 32];
            let s = size;
            match a.mode {
                D207_PRED => {
                    for j in 0..s {
                        pred[s - 1][j] = left[s - 1];
                    }
                    for i in 0..s - 1 {
                        pred[i][0] = r2(left[i], left[i + 1]);
                    }
                    for i in 0..s.saturating_sub(2) {
                        pred[i][1] = r3(left[i], left[i + 1], left[i + 2]);
                    }
                    pred[s - 2][1] = (left[s - 2] + 3 * left[s - 1] + 2) >> 2;
                    for j in 2..s {
                        for i in (0..s - 1).rev() {
                            pred[i][j] = pred[i + 1][j - 2];
                        }
                    }
                }
                D45_PRED => {
                    for i in 0..s {
                        for j in 0..s {
                            let k = (i + j) as isize;
                            pred[i][j] = if i + j + 2 < 2 * s {
                                r3(ab(k), ab(k + 1), ab(k + 2))
                            } else {
                                ab(2 * s as isize - 1)
                            };
                        }
                    }
                }
                D63_PRED => {
                    for i in 0..s {
                        for j in 0..s {
                            let k = (i / 2 + j) as isize;
                            pred[i][j] = if i & 1 == 1 {
                                r3(ab(k), ab(k + 1), ab(k + 2))
                            } else {
                                r2(ab(k), ab(k + 1))
                            };
                        }
                    }
                }
                D117_PRED => {
                    for j in 0..s {
                        pred[0][j] = r2(ab(j as isize - 1), ab(j as isize));
                    }
                    pred[1][0] = r3(left[0], ab(-1), ab(0));
                    for j in 1..s {
                        pred[1][j] = r3(ab(j as isize - 2), ab(j as isize - 1), ab(j as isize));
                    }
                    pred[2][0] = r3(ab(-1), left[0], left[1]);
                    for i in 3..s {
                        pred[i][0] = r3(left[i - 3], left[i - 2], left[i - 1]);
                    }
                    for i in 2..s {
                        for j in 1..s {
                            pred[i][j] = pred[i - 2][j - 1];
                        }
                    }
                }
                D135_PRED => {
                    pred[0][0] = r3(left[0], ab(-1), ab(0));
                    for j in 1..s {
                        pred[0][j] = r3(ab(j as isize - 2), ab(j as isize - 1), ab(j as isize));
                    }
                    pred[1][0] = r3(ab(-1), left[0], left[1]);
                    for i in 2..s {
                        pred[i][0] = r3(left[i - 2], left[i - 1], left[i]);
                    }
                    for i in 1..s {
                        for j in 1..s {
                            pred[i][j] = pred[i - 1][j - 1];
                        }
                    }
                }
                _ => {
                    // D153_PRED
                    pred[0][0] = r2(left[0], ab(-1));
                    for i in 1..s {
                        pred[i][0] = r2(left[i - 1], left[i]);
                    }
                    pred[0][1] = r3(left[0], ab(-1), ab(0));
                    pred[1][1] = r3(ab(-1), left[0], left[1]);
                    for i in 2..s {
                        pred[i][1] = r3(left[i - 2], left[i - 1], left[i]);
                    }
                    for j in 2..s {
                        pred[0][j] = r3(ab(j as isize - 3), ab(j as isize - 2), ab(j as isize - 1));
                    }
                    for i in 1..s {
                        for j in 2..s {
                            pred[i][j] = pred[i - 1][j - 2];
                        }
                    }
                }
            }
            for (i, pr) in pred.iter().enumerate().take(s) {
                for (d, v) in rows[y + i][xo..xo + s].iter_mut().zip(pr) {
                    *d = P::put(*v);
                }
            }
        }
    }
}
