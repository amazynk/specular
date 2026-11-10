//! Fujifilm SuperCCD: photosites on a lattice turned by 45 degrees.
//!
//! The camera stores the photosites row by row along the turned lattice. Placed on an upright
//! grid (each stored row runs along a diagonal), they fill a diamond whose cells form an
//! ordinary 2x2 Bayer pattern. This module demosaics that grid and turns it back by 45 degrees
//! with bilinear sampling, which gives an upright camera RGB image (cpp 3) of about the
//! photosite count: `(fw - 1) * sqrt 2` wide, where `fw` is the photosite count along one
//! diagonal.

use rayon::prelude::*;

/// Layout of the stored photosites and of the upright image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Geometry {
    /// Stored samples per row and rows (16-bit little-endian samples).
    pub raw_w: u32,
    pub raw_h: u32,
    /// Masked margins of the stored rows.
    pub top: u32,
    pub left: u32,
    /// Photosites along one diagonal.
    pub fw: u32,
    /// Fujifilm layout flag: each stored row holds one diagonal (true) or two (false).
    pub one_diagonal: bool,
    /// The upright grid the photosites are placed on.
    pub grid_w: u32,
    pub grid_h: u32,
    /// The output image.
    pub out_w: u32,
    pub out_h: u32,
    /// Stored rows per photosite row: 2 on SuperCCD SR sensors (F700, F710) that store each
    /// row of high-sensitivity (S) photosites followed by the row of their low-sensitivity (R)
    /// partners; only the S rows are read.
    pub row_step: u32,
}

const STEP: f64 = std::f64::consts::FRAC_1_SQRT_2;

impl Geometry {
    /// From the stored size and the camera's frame size (RAF tag 0x121, in Fujifilm's own
    /// unturned coordinates). None for sizes that do not fit.
    pub(crate) fn new(
        raw_w: u32,
        raw_h: u32,
        frame_w: u32,
        frame_h: u32,
        one_diagonal: bool,
    ) -> Option<Geometry> {
        // Rows and samples per row of photosite data.
        let (h, w) = if one_diagonal {
            (frame_h.checked_mul(2)?, frame_w / 2)
        } else {
            (frame_h, frame_w)
        };
        if h == 0 || w < 4 || h > raw_h || w > raw_w {
            return None;
        }
        // Equal margins on both sides, rounded down to even.
        let top = ((raw_h - h) >> 2) << 1;
        let left = ((raw_w - w) >> 2) << 1;
        Geometry::with_margins(raw_w, raw_h, top, left, w, h, one_diagonal)
    }

    /// With explicit margins: `w x h` stored samples from (`left`, `top`).
    pub(crate) fn with_margins(
        raw_w: u32,
        raw_h: u32,
        top: u32,
        left: u32,
        w: u32,
        h: u32,
        one_diagonal: bool,
    ) -> Option<Geometry> {
        if left.checked_add(w)? > raw_w || top.checked_add(h)? > raw_h || w < 4 || h < 4 {
            return None;
        }
        let fw = if one_diagonal { w } else { w / 2 };
        let grid_w = (h >> u32::from(one_diagonal)).checked_add(fw)?;
        let grid_h = grid_w - 1;
        // The diamond fills about half the grid; anything much sparser is not a SuperCCD.
        let cells = grid_w as u64 * grid_h as u64;
        if fw < 4 || grid_h <= fw || cells > 8 * (w as u64 * h as u64) {
            return None;
        }
        let out_w = ((fw - 1) as f64 / STEP) as u32;
        let out_h = ((grid_h - (fw - 1)) as f64 / STEP) as u32;
        // The sampling below assumes every placed photosite lies on the grid.
        let rows = raw_h - 2 * top;
        if rows == 0 || out_w == 0 || out_h == 0 {
            return None;
        }
        Some(Geometry {
            raw_w,
            raw_h,
            top,
            left,
            fw,
            one_diagonal,
            grid_w,
            grid_h,
            out_w,
            out_h,
            row_step: 1,
        })
    }

    /// The same layout with each photosite row followed by `step - 1` other stored rows.
    pub(crate) fn with_row_step(self, step: u32) -> Option<Geometry> {
        (1..=4).contains(&step).then_some(Geometry {
            row_step: step,
            ..self
        })
    }

    /// Stored rows and samples per row that hold photosites.
    fn stored(&self) -> (u32, u32) {
        let cols = if self.one_diagonal {
            self.fw
        } else {
            self.fw * 2
        };
        (self.raw_h - 2 * self.top, cols.min(self.raw_w - self.left))
    }

    /// The stored (row, column) of grid cell (r, c), relative to the margins.
    fn source(&self, r: u32, c: u32) -> Option<(u32, u32)> {
        let (rows, cols) = self.stored();
        let (r, c, fw) = (r as i64, c as i64, self.fw as i64);
        let (row, col) = if self.one_diagonal {
            let row = r + c + 1 - fw;
            (row, (c - r + fw - 1 - (row & 1)) >> 1)
        } else {
            let col = c - r + fw - 1;
            (((r + c - fw + 1) - (col & 1)) >> 1, col)
        };
        (row >= 0 && col >= 0 && row < rows as i64 && col < cols as i64)
            .then_some((row as u32, col as u32))
    }

    /// Colour (0 R, 1 G, 2 B) of grid cell (r, c).
    fn color(&self, r: usize, c: usize) -> usize {
        // fw odd: R G / G B; even: G B / R G.
        let p: [usize; 4] = if self.fw & 1 == 1 {
            [0, 1, 1, 2]
        } else {
            [1, 2, 0, 1]
        };
        p[(r & 1) * 2 + (c & 1)]
    }

    /// Bytes of stored data the decode reads.
    pub(crate) fn data_len(&self) -> usize {
        let rows = (self.raw_h as usize * self.row_step as usize)
            .saturating_sub(self.row_step as usize - 1);
        self.raw_w as usize * rows * 2
    }
}

/// Decode `src` (stored samples) into `out` (`out_w x out_h` camera RGB, cpp 3). Returns true
/// when `src` was short (missing samples read as zero).
pub(crate) fn decode(src: &[u8], g: &Geometry, out: &mut [u16], parallel: bool) -> bool {
    let (gw, gh) = (g.grid_w as usize, g.grid_h as usize);
    let rw = g.raw_w as usize;
    let step = g.row_step.max(1) as usize;
    let sample = |row: u32, col: u32| -> u16 {
        let i = ((row + g.top) as usize * step * rw + (col + g.left) as usize) * 2;
        src.get(i..i + 2)
            .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
    };
    // Photosites on the upright grid (zero outside the diamond).
    let mut grid = vec![0u16; gw * gh];
    let place = |(r, line): (usize, &mut [u16])| {
        for (c, v) in line.iter_mut().enumerate() {
            if let Some((row, col)) = g.source(r as u32, c as u32) {
                *v = sample(row, col);
            }
        }
    };
    if parallel {
        grid.par_chunks_mut(gw).enumerate().for_each(place);
    } else {
        grid.chunks_mut(gw).enumerate().for_each(place);
    }
    // Green everywhere: Hamilton-Adams (gradient-directed, with a second-order correction from
    // the cell's own colour).
    let mut green = vec![0u16; gw * gh];
    let at = |r: isize, c: isize| -> i32 {
        let (r, c) = (
            r.clamp(0, gh as isize - 1) as usize,
            c.clamp(0, gw as isize - 1) as usize,
        );
        grid[r * gw + c] as i32
    };
    let fill_green = |(r, line): (usize, &mut [u16])| {
        for (c, o) in line.iter_mut().enumerate() {
            let x = grid[r * gw + c] as i32;
            if g.color(r, c) == 1 {
                *o = x as u16;
                continue;
            }
            let (ri, ci) = (r as isize, c as isize);
            let (gl, gr, gu, gd) = (
                at(ri, ci - 1),
                at(ri, ci + 1),
                at(ri - 1, ci),
                at(ri + 1, ci),
            );
            let lh = 2 * x - at(ri, ci - 2) - at(ri, ci + 2);
            let lv = 2 * x - at(ri - 2, ci) - at(ri + 2, ci);
            let dh = (gl - gr).abs() + lh.abs();
            let dv = (gu - gd).abs() + lv.abs();
            let h = (2 * (gl + gr) + lh) / 4;
            let v = (2 * (gu + gd) + lv) / 4;
            let est = if dh < dv {
                h
            } else if dv < dh {
                v
            } else {
                (h + v) / 2
            };
            *o = est.clamp(0, 65535) as u16;
        }
    };
    if parallel {
        green.par_chunks_mut(gw).enumerate().for_each(fill_green);
    } else {
        green.chunks_mut(gw).enumerate().for_each(fill_green);
    }
    // Full RGB of a grid cell: red and blue from the colour differences to green.
    let rgb = |r: usize, c: usize| -> [f32; 3] {
        let gv = |r: usize, c: usize| green[r * gw + c] as i32;
        let diff = |r: usize, c: usize| grid[r * gw + c] as i32 - gv(r, c);
        let gc = gv(r, c);
        let mut px = [0i32; 3];
        px[1] = gc;
        let own = g.color(r, c);
        let (r0, r1) = (r.saturating_sub(1), (r + 1).min(gh - 1));
        let (c0, c1) = (c.saturating_sub(1), (c + 1).min(gw - 1));
        if own == 1 {
            // Red and blue sit left/right in one direction and above/below in the other.
            let horiz = g.color(r, c ^ 1);
            let other = 2 - horiz;
            let h = (diff(r, c0) + diff(r, c1)) / 2;
            let v = (diff(r0, c) + diff(r1, c)) / 2;
            px[horiz] = gc + h;
            px[other] = gc + v;
        } else {
            px[own] = grid[r * gw + c] as i32;
            let d = (diff(r0, c0) + diff(r0, c1) + diff(r1, c0) + diff(r1, c1)) / 4;
            px[2 - own] = gc + d;
        }
        px.map(|v| v.max(0) as f32)
    };
    // Turn back by 45 degrees.
    let fw1 = (g.fw - 1) as f64;
    let ow = g.out_w as usize;
    let turn = |(row, line): (usize, &mut [u16])| {
        for (col, o) in line.chunks_exact_mut(3).enumerate() {
            let rf = fw1 + (row as f64 - col as f64) * STEP;
            let cf = (row + col) as f64 * STEP;
            if rf < 0.0 || cf < 0.0 {
                continue;
            }
            let (ur, uc) = (rf as usize, cf as usize);
            if ur + 2 > gh || uc + 2 > gw {
                continue;
            }
            let (fr, fc) = ((rf - ur as f64) as f32, (cf - uc as f64) as f32);
            let (a, b, c, d) = (
                rgb(ur, uc),
                rgb(ur, uc + 1),
                rgb(ur + 1, uc),
                rgb(ur + 1, uc + 1),
            );
            for k in 0..3 {
                let v = (a[k] * (1.0 - fc) + b[k] * fc) * (1.0 - fr)
                    + (c[k] * (1.0 - fc) + d[k] * fc) * fr;
                o[k] = v.clamp(0.0, 65535.0) as u16;
            }
        }
    };
    let out = &mut out[..ow * g.out_h as usize * 3];
    if parallel {
        out.par_chunks_mut(ow * 3).enumerate().for_each(turn);
    } else {
        out.chunks_mut(ow * 3).enumerate().for_each(turn);
    }
    src.len() < g.data_len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Forward placement (each stored photosite to its grid cell), as the inverse in
    /// `Geometry::source` must undo it.
    fn forward(g: &Geometry, row: u32, col: u32) -> (u32, u32) {
        let fw = g.fw;
        if g.one_diagonal {
            (fw - 1 - col + (row >> 1), col + ((row + 1) >> 1))
        } else {
            (fw - 1 + row - (col >> 1), row + ((col + 1) >> 1))
        }
    }

    #[test]
    fn placement_round_trips() {
        for one in [true, false] {
            let g = if one {
                Geometry::new(40, 60, 64, 28, true).unwrap()
            } else {
                Geometry::new(70, 30, 64, 28, false).unwrap()
            };
            let (rows, cols) = g.stored();
            let mut hit = vec![false; (g.grid_w * g.grid_h) as usize];
            for row in 0..rows {
                for col in 0..cols {
                    let (r, c) = forward(&g, row, col);
                    if r < g.grid_h && c < g.grid_w {
                        assert_eq!(g.source(r, c), Some((row, col)), "{one} {row} {col}");
                        hit[(r * g.grid_w + c) as usize] = true;
                    }
                }
            }
            // Every cell the inverse claims is placed by the forward mapping.
            for r in 0..g.grid_h {
                for c in 0..g.grid_w {
                    if g.source(r, c).is_some() {
                        assert!(hit[(r * g.grid_w + c) as usize], "{one} {r} {c}");
                    }
                }
            }
        }
    }

    #[test]
    fn flat_field_stays_flat() {
        // A grey card: every photosite 1000. The turned image is 1000 inside the diamond.
        let g = Geometry::new(40, 60, 64, 28, true).unwrap();
        let src: Vec<u8> = (0..g.raw_w * g.raw_h)
            .flat_map(|_| 1000u16.to_le_bytes())
            .collect();
        let mut out = vec![0u16; (g.out_w * g.out_h * 3) as usize];
        assert!(!decode(&src, &g, &mut out, false));
        let (w, h) = (g.out_w as usize, g.out_h as usize);
        let mid = &out[((h / 2) * w + w / 2) * 3..][..3];
        assert_eq!(mid, &[1000, 1000, 1000]);
        let mut par = vec![0u16; out.len()];
        decode(&src, &g, &mut par, true);
        assert_eq!(par, out);
    }

    #[test]
    fn sr_rows_are_skipped() {
        // S rows with a pattern, each followed by an R row of other values: the decode with a
        // row step of 2 equals the decode of the S rows alone.
        let g = Geometry::new(40, 60, 64, 28, true).unwrap();
        let s_row = |r: u32| -> Vec<u8> {
            (0..g.raw_w)
                .map(|c| ((r * 7 + c * 13) % 4000 + 100) as u16)
                .flat_map(|v| v.to_le_bytes())
                .collect()
        };
        let plain: Vec<u8> = (0..g.raw_h).flat_map(s_row).collect();
        let sr: Vec<u8> = (0..g.raw_h)
            .flat_map(|r| {
                let mut v = s_row(r);
                v.extend((0..g.raw_w).flat_map(|_| 60000u16.to_le_bytes()));
                v
            })
            .collect();
        let g2 = g.with_row_step(2).unwrap();
        assert_eq!(g2.data_len(), sr.len() - g.raw_w as usize * 2);
        let n = (g.out_w * g.out_h * 3) as usize;
        let (mut a, mut b) = (vec![0u16; n], vec![0u16; n]);
        assert!(!decode(&plain, &g, &mut a, false));
        assert!(!decode(&sr, &g2, &mut b, false));
        assert_eq!(a, b);
        assert!(g.with_row_step(0).is_none());
    }

    #[test]
    fn short_and_hostile_input() {
        let g = Geometry::new(40, 60, 64, 28, true).unwrap();
        let mut out = vec![0u16; (g.out_w * g.out_h * 3) as usize];
        assert!(decode(&[1, 2, 3], &g, &mut out, true));
        assert!(Geometry::new(40, 60, 0, 0, true).is_none());
        assert!(Geometry::new(40, 60, 64, 4000, true).is_none());
        assert!(Geometry::new(4, 4, 8, 4, false).is_none());
        assert!(Geometry::new(u32::MAX, u32::MAX, u32::MAX, u32::MAX, true).is_none());
    }
}
