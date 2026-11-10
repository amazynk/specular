//! The inverse transforms of Annex A: rows with the M-point transform and `(x + 4) >> 3`,
//! columns with the N-point transform and `(x + C + 64) >> 7`, C = 1 on the last four
//! outputs of the 8-point column transform.

/// Figure 157: the 8-point transform matrix, rows the basis functions.
const T8: [[i32; 8]; 8] = [
    [12, 12, 12, 12, 12, 12, 12, 12],
    [16, 15, 9, 4, -4, -9, -15, -16],
    [16, 6, -6, -16, -16, -6, 6, 16],
    [15, -4, -16, -9, 9, 16, 4, -15],
    [12, -12, -12, 12, 12, -12, -12, 12],
    [9, -16, 4, 15, -15, -4, 16, -9],
    [6, -16, 16, -6, -6, 16, -16, 6],
    [4, -9, 15, -16, 16, -15, 9, -4],
];

/// Figure 158: the 4-point transform matrix.
const T4: [[i32; 4]; 4] = [
    [17, 17, 17, 17],
    [22, 10, -10, -22],
    [17, -17, -17, 17],
    [10, -22, 22, -10],
];

#[inline]
fn row8(c: &[i32; 8]) -> [i32; 8] {
    let mut o = [0; 8];
    for (j, oj) in o.iter_mut().enumerate() {
        let mut s = 0;
        for i in 0..8 {
            s += c[i] * T8[i][j];
        }
        *oj = s;
    }
    o
}

#[inline]
fn row4(c: &[i32; 4]) -> [i32; 4] {
    let mut o = [0; 4];
    for (j, oj) in o.iter_mut().enumerate() {
        let mut s = 0;
        for i in 0..4 {
            s += c[i] * T4[i][j];
        }
        *oj = s;
    }
    o
}

/// Inverse transform of a `w` × `h` block (w, h ∈ {4, 8}) of dequantized coefficients in
/// `blk` (row-major, row stride 8), in place: the result is the residual, row stride 8.
pub fn inverse(blk: &mut [i32; 64], w: usize, h: usize) {
    // Rows.
    for y in 0..h {
        let row = &mut blk[y * 8..y * 8 + 8];
        if w == 8 {
            let mut c = [0; 8];
            c.copy_from_slice(row);
            if c.iter().all(|&v| v == 0) {
                continue;
            }
            let o = row8(&c);
            for x in 0..8 {
                row[x] = (o[x] + 4) >> 3;
            }
        } else {
            let mut c = [0; 4];
            c.copy_from_slice(&row[..4]);
            let o = row4(&c);
            for x in 0..4 {
                row[x] = (o[x] + 4) >> 3;
            }
        }
    }
    // Columns.
    for x in 0..w {
        if h == 8 {
            let mut c = [0; 8];
            for y in 0..8 {
                c[y] = blk[y * 8 + x];
            }
            let o = row8(&c);
            for y in 0..8 {
                let k = if y >= 4 { 1 } else { 0 };
                blk[y * 8 + x] = (o[y] + k + 64) >> 7;
            }
        } else {
            let mut c = [0; 4];
            for y in 0..4 {
                c[y] = blk[y * 8 + x];
            }
            let o = row4(&c);
            for y in 0..4 {
                blk[y * 8 + x] = (o[y] + 64) >> 7;
            }
        }
    }
}
