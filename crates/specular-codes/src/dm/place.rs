//! ECC 200 module placement (ISO/IEC 16022, annex F): which codeword bit each module of the
//! mapping matrix holds. Codewords run along diagonals in the "utah" shape, with four corner
//! shapes where the diagonals meet the matrix's edges; bit 1 is a codeword's most
//! significant.

/// (codeword, bit 0..8 with 0 the most significant) for each module, row by row; `None` for
/// the spare modules some sizes leave at the bottom right.
pub fn layout(nrow: usize, ncol: usize) -> Vec<Option<(u16, u8)>> {
    let (nr, nc) = (nrow as isize, ncol as isize);
    let mut a: Vec<Option<(u16, u8)>> = vec![None; nrow * ncol];
    let set = |a: &mut Vec<Option<(u16, u8)>>, mut r: isize, mut c: isize, cw: u16, bit: u8| {
        if r < 0 {
            r += nr;
            c += 4 - ((nr + 4) % 8);
        }
        if c < 0 {
            c += nc;
            r += 4 - ((nc + 4) % 8);
        }
        if (0..nr).contains(&r) && (0..nc).contains(&c) {
            a[(r * nc + c) as usize] = Some((cw, bit));
        }
    };
    let utah = |a: &mut Vec<Option<(u16, u8)>>, r: isize, c: isize, cw: u16| {
        let shape = [
            (r - 2, c - 2),
            (r - 2, c - 1),
            (r - 1, c - 2),
            (r - 1, c - 1),
            (r - 1, c),
            (r, c - 2),
            (r, c - 1),
            (r, c),
        ];
        for (b, (y, x)) in shape.into_iter().enumerate() {
            set(a, y, x, cw, b as u8);
        }
    };
    let corner = |a: &mut Vec<Option<(u16, u8)>>, cells: [(isize, isize); 8], cw: u16| {
        for (b, (y, x)) in cells.into_iter().enumerate() {
            set(a, y, x, cw, b as u8);
        }
    };
    let (mut cw, mut r, mut c) = (0u16, 4isize, 0isize);
    loop {
        if r == nr && c == 0 {
            corner(
                &mut a,
                [
                    (nr - 1, 0),
                    (nr - 1, 1),
                    (nr - 1, 2),
                    (0, nc - 2),
                    (0, nc - 1),
                    (1, nc - 1),
                    (2, nc - 1),
                    (3, nc - 1),
                ],
                cw,
            );
            cw += 1;
        }
        if r == nr - 2 && c == 0 && nc % 4 != 0 {
            corner(
                &mut a,
                [
                    (nr - 3, 0),
                    (nr - 2, 0),
                    (nr - 1, 0),
                    (0, nc - 4),
                    (0, nc - 3),
                    (0, nc - 2),
                    (0, nc - 1),
                    (1, nc - 1),
                ],
                cw,
            );
            cw += 1;
        }
        if r == nr - 2 && c == 0 && nc % 8 == 4 {
            corner(
                &mut a,
                [
                    (nr - 3, 0),
                    (nr - 2, 0),
                    (nr - 1, 0),
                    (0, nc - 2),
                    (0, nc - 1),
                    (1, nc - 1),
                    (2, nc - 1),
                    (3, nc - 1),
                ],
                cw,
            );
            cw += 1;
        }
        if r == nr + 4 && c == 2 && nc % 8 == 0 {
            corner(
                &mut a,
                [
                    (nr - 1, 0),
                    (nr - 1, nc - 1),
                    (0, nc - 3),
                    (0, nc - 2),
                    (0, nc - 1),
                    (1, nc - 3),
                    (1, nc - 2),
                    (1, nc - 1),
                ],
                cw,
            );
            cw += 1;
        }
        // Up and to the right.
        loop {
            if r < nr && c >= 0 && a[(r * nc + c) as usize].is_none() {
                utah(&mut a, r, c, cw);
                cw += 1;
            }
            r -= 2;
            c += 2;
            if !(r >= 0 && c < nc) {
                break;
            }
        }
        r += 1;
        c += 3;
        // Down and to the left.
        loop {
            if r >= 0 && c < nc && a[(r * nc + c) as usize].is_none() {
                utah(&mut a, r, c, cw);
                cw += 1;
            }
            r += 2;
            c -= 2;
            if !(r < nr && c >= 0) {
                break;
            }
        }
        r += 3;
        c += 1;
        if !(r < nr || c < nc) {
            break;
        }
    }
    a
}

/// The fixed pattern in the bottom-right spare modules: dark at the corner and diagonally
/// above-left of it, light elsewhere.
pub fn spare_dark(nrow: usize, ncol: usize, r: usize, c: usize) -> bool {
    (r == nrow - 1 && c == ncol - 1) || (r == nrow - 2 && c == ncol - 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dm::tables::SIZES;

    /// Every codeword gets its eight bits exactly once.
    #[test]
    fn every_bit_once() {
        for s in SIZES {
            let (nr, nc) = s.mapping();
            let a = layout(nr, nc);
            let n = s.data + s.ecc;
            let mut seen = vec![0u8; n];
            for (cw, bit) in a.iter().flatten() {
                assert!((*cw as usize) < n, "{s:?}");
                seen[*cw as usize] |= 1 << bit;
            }
            assert!(seen.iter().all(|&b| b == 0xFF), "{s:?}");
            assert!(a.iter().filter(|m| m.is_none()).count() <= 4);
        }
    }
}
