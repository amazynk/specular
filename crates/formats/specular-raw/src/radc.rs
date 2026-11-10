//! Kodak radial compression (RADC; TIFF compression 32867), the codec of the Kodak DC40 and
//! DC50.
//!
//! The sensor is coded in bands of four rows. Each band starts with a 6-bit scale per colour
//! plane (green, red, blue); the predictions carried over from the band above are rescaled by
//! the ratio of the new scale to the previous one. Each plane is coded right to left in pairs
//! of columns over two rows of a half-width working grid (green twice, red and blue once):
//! a token from a small prefix-code table either selects the next table and codes a 2x2 group
//! (differences from a neighbour prediction, or raw values for a flat area) or starts a run
//! of predicted groups with occasional steps. The grids are mapped back to the Bayer sites,
//! the second green of each pair is reconstructed from its neighbours, and a tone curve
//! expands the result to 14 bits.

use crate::bits::BitReader;

/// Prefix codes: (code length, value) pairs filling tables of 256 entries in order; 18 tables.
/// The values are the codec's data as in dcraw's `kodak_radc_load_raw` (Dave Coffin; not one
/// of dcraw's RESTRICTED functions), copied as data only.
#[rustfmt::skip]
const CODES: &[i8] = &[
    1,1, 2,3, 3,4, 4,2, 5,7, 6,5, 7,6, 7,8,
    1,0, 2,1, 3,3, 4,4, 5,2, 6,7, 7,6, 8,5, 8,8,
    2,1, 2,3, 3,0, 3,2, 3,4, 4,6, 5,5, 6,7, 6,8,
    2,0, 2,1, 2,3, 3,2, 4,4, 5,6, 6,7, 7,5, 7,8,
    2,1, 2,4, 3,0, 3,2, 3,3, 4,7, 5,5, 6,6, 6,8,
    2,3, 3,1, 3,2, 3,4, 3,5, 3,6, 4,7, 5,0, 5,8,
    2,3, 2,6, 3,0, 3,1, 4,4, 4,5, 4,7, 5,2, 5,8,
    2,4, 2,7, 3,3, 3,6, 4,1, 4,2, 4,5, 5,0, 5,8,
    2,6, 3,1, 3,3, 3,5, 3,7, 3,8, 4,0, 5,2, 5,4,
    2,0, 2,1, 3,2, 3,3, 4,4, 4,5, 5,6, 5,7, 4,8,
    1,0, 2,2, 2,-2,
    1,-3, 1,3,
    2,-17, 2,-5, 2,5, 2,17,
    2,-7, 2,2, 2,9, 2,18,
    2,-18, 2,-9, 2,-2, 2,7,
    2,-28, 2,28, 3,-49, 3,-9, 3,9, 4,49, 5,-79, 5,79,
    2,-1, 2,13, 2,26, 3,39, 4,-16, 5,55, 6,-37, 6,76,
    2,-26, 2,-13, 2,1, 3,-39, 4,16, 5,-55, 6,-76, 6,37,
];

/// Knots of the output tone curve (input, output), linear in between.
const CURVE: [(u32, u32); 6] = [
    (0, 0),
    (1280, 1344),
    (2320, 3616),
    (3328, 8000),
    (4095, 16383),
    (65535, 16383),
];

/// Saturation of the decoded samples.
pub(crate) const WHITE: u16 = 0x3fff;

/// Width of the working grid rows (half the widest sensor plus the edge).
const GRID: usize = 386;

/// The 19 lookup tables: for every 8-bit window, code length and value.
fn tables(cbpp: u32) -> Vec<[(u8, i8); 256]> {
    let mut t = vec![[(0u8, 0i8); 256]; 19];
    let mut k = 0usize;
    for pair in CODES.chunks_exact(2) {
        let (len, val) = (pair[0] as u8, pair[1]);
        for _ in 0..(256usize >> len) {
            if let Some(e) = t.get_mut(k / 256).and_then(|t| t.get_mut(k % 256)) {
                *e = (len, val);
            }
            k += 1;
        }
    }
    // Table 18: raw values of flat groups, 5 or 6 bits rounded to the middle of their step.
    let s = if cbpp == 243 { 2 } else { 3 };
    for (c, e) in t[18].iter_mut().enumerate() {
        *e = (8 - s, ((c >> s << s) | 1 << (s - 1)) as u8 as i8);
    }
    t
}

struct Tokens<'a> {
    br: BitReader<'a>,
    tables: Vec<[(u8, i8); 256]>,
}

impl Tokens<'_> {
    fn token(&mut self, table: usize) -> i32 {
        if self.br.nbits() < 8 {
            self.br.refill();
        }
        let (len, val) = self.tables[table.min(18)][self.br.peek(8) as usize];
        self.br.consume(len as u32);
        val as i32
    }
    fn bits(&mut self, n: u32) -> i32 {
        self.br.bits(n) as i32
    }
}

/// Decodes a `w x h` RADC sensor (`w <= 768`, `h` a multiple of 4 is expected) from `src`
/// into `out` (row-major). `cbpp` is the numerator of the file's CompressedBitsPerPixel.
/// Returns true when the data ran short.
pub(crate) fn decode(src: &[u8], cbpp: u32, w: usize, h: usize, out: &mut [u16]) -> bool {
    let half = w / 2;
    if half + 2 > GRID || out.len() < w * h {
        return true;
    }
    let mut t = Tokens {
        br: BitReader::new(src, false),
        tables: tables(cbpp),
    };
    let mut raw = vec![0u16; w * h];
    // Per plane, three rows of the working grid: the row above, then the two coded rows.
    let mut grid = [[[2048i16; GRID]; 3]; 3];
    let mut last = [16i32; 3];
    let set = |raw: &mut Vec<u16>, y: usize, x: usize, v: i32| {
        if y < h && x < w {
            raw[y * w + x] = v as u16;
        }
    };
    for row in (0..h).step_by(4) {
        let mul: [i32; 3] = std::array::from_fn(|_| t.bits(6));
        for c in 0..3 {
            // Rescale the carried predictions to the band's scale.
            let mut val = ((0x100_0000 / last[c] + 0x7ff) >> 12) * mul[c];
            let s = if val > 65564 { 10 } else { 12 };
            let round = (1 << (s - 1)) - 1;
            val <<= 12 - s;
            for line in grid[c].iter_mut() {
                for v in line.iter_mut() {
                    *v = ((*v as i64 * val as i64 + round as i64) >> s) as i16;
                }
            }
            last[c] = mul[c].max(1);
            let passes = if c == 0 { 2 } else { 1 };
            for pass in 0..passes {
                let g = &mut grid[c];
                g[1][half] = (mul[c] << 7) as i16;
                g[2][half] = (mul[c] << 7) as i16;
                let predict = |g: &[[i16; GRID]; 3], y: usize, x: usize| -> i32 {
                    if c == 0 {
                        (g[y - 1][x + 1] as i32 + 2 * g[y - 1][x] as i32 + g[y][x + 1] as i32) / 4
                    } else {
                        (g[y - 1][x] as i32 + g[y][x + 1] as i32) / 2
                    }
                };
                let mut tree = 1i32;
                let mut col = half;
                while col > 0 {
                    tree = t.token(tree as usize);
                    if tree != 0 {
                        col = col.saturating_sub(2);
                        for y in 1..3 {
                            for x in [col + 1, col] {
                                g[y][x] = if tree == 8 {
                                    (t.token(18) as u8 as i32 * mul[c]) as i16
                                } else {
                                    (t.token((tree + 10) as usize) * 16 + predict(g, y, x)) as i16
                                };
                            }
                        }
                    } else {
                        loop {
                            let reps = if col > 2 { t.token(9) + 1 } else { 1 };
                            let mut rep = 0;
                            while rep < 8 && rep < reps && col > 0 {
                                col = col.saturating_sub(2);
                                for y in 1..3 {
                                    for x in [col + 1, col] {
                                        g[y][x] = predict(g, y, x) as i16;
                                    }
                                }
                                if rep & 1 == 1 {
                                    let step = t.token(10) << 4;
                                    for line in &mut g[1..3] {
                                        for v in &mut line[col..col + 2] {
                                            *v = (*v as i32 + step) as i16;
                                        }
                                    }
                                }
                                rep += 1;
                            }
                            if reps != 9 {
                                break;
                            }
                        }
                    }
                }
                // Back to the Bayer sites: green on both diagonals of the band's halves, red
                // on even rows, blue on odd rows.
                for y in 0..2 {
                    for (x, &gv) in g[y + 1][..half].iter().enumerate() {
                        let v = if mul[c] == 0 {
                            0
                        } else {
                            ((gv as i32) << 4) / mul[c]
                        }
                        .max(0);
                        match c {
                            0 => set(&mut raw, row + pass * 2 + y, x * 2 + y, v),
                            _ => set(&mut raw, row + y * 2 + c - 1, x * 2 + 2 - c, v),
                        }
                    }
                }
                // The last coded row becomes the row above (green shifted by one column).
                let last_row = g[2];
                if c == 0 {
                    g[0][1..].copy_from_slice(&last_row[..GRID - 1]);
                } else {
                    g[0] = last_row;
                }
            }
        }
        // The other green of each pair: a difference coded around 2048 plus its neighbours.
        for y in row..(row + 4).min(h) {
            for x in 0..w {
                if (x + y) & 1 == 1 {
                    let l = if x > 0 { x - 1 } else { x + 1 };
                    let r = if x + 1 < w {
                        x + 1
                    } else {
                        x.saturating_sub(1)
                    };
                    let at = |x: usize| raw[y * w + x] as i32;
                    let v = (at(x) - 2048) * 2 + (at(l) + at(r)) / 2;
                    raw[y * w + x] = v.max(0) as u16;
                }
            }
        }
    }
    let lut = curve();
    for (o, &v) in out.iter_mut().zip(&raw) {
        *o = lut[v as usize];
    }
    t.br.overrun()
}

fn curve() -> Vec<u16> {
    let mut lut = vec![0u16; 65536];
    for k in CURVE.windows(2) {
        let ((x0, y0), (x1, y1)) = (k[0], k[1]);
        for c in x0..=x1 {
            let v = (c - x0) as f32 / (x1 - x0) as f32 * (y1 as f32 - y0 as f32) + y0 as f32 + 0.5;
            lut[c as usize] = v as u16;
        }
    }
    lut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_complete() {
        let t = tables(152);
        assert!(t.iter().all(|t| t.iter().all(|e| e.0 >= 1 && e.0 <= 8)));
        assert_eq!(t[0][0], (1, 1));
        assert_eq!(t[0][255], (7, 8));
        assert_eq!(t[18][0b1010_0000], (5, 0b1010_0100u8 as i8));
        assert_eq!(tables(243)[18][0b1010_0000], (6, 0b1010_0010u8 as i8));
    }

    #[test]
    fn curve_knots() {
        let c = curve();
        assert_eq!((c[0], c[1280], c[4095], c[65535]), (0, 1344, 16383, 16383));
    }

    #[test]
    fn hostile_input() {
        let mut out = vec![0u16; 64 * 8];
        for src in [&[][..], &[0xff; 3][..], &[0x00; 100][..], &[0x5a; 4000][..]] {
            let _ = decode(src, 0, 64, 8, &mut out);
        }
        // Too wide for the working grid.
        assert!(decode(&[0; 10], 0, 2000, 4, &mut vec![0; 8000]));
    }
}
