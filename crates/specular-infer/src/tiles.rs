//! Tiled image-to-image inference (Develop's AI denoise): a network that takes a
//! fixed `size × size` input runs over an image of any size, tile by tile, and the tiles
//! are joined without visible seams.
//!
//! - **A global grid.** Cells of `stride × stride` pixels from the image's top left corner.
//!   Tile `(i, j)` gives the output over its cell grown by `overlap / 2` on every side (its
//!   *usable* area); it reads that area grown by `margin` more (real image around it,
//!   clamped at the image edge), and the margin's output is thrown away, so no tile edge
//!   (where the network sees padding) ever shows.
//! - **Feathering.** Within the usable area a pixel's weight ramps linearly across the
//!   `overlap` band at each side; two neighbouring tiles' weights add up to one, so a pixel
//!   in the band is a cross-fade. Every output pixel is the weighted sum over the tiles
//!   covering it, divided by their weights.
//! - **Any cells, the same result.** A run computes the cells asked for (with the tiles
//!   around them), so a pixel's value depends only on the grid: two runs over neighbouring
//!   regions agree where they meet, and a region computed for the 100 % view equals the
//!   same region of a whole-image run.
//! - **Bounded memory.** Tile rows run top to bottom; a cell row is handed out
//!   ([`Tiling::run`]'s `emit`) as soon as the tile row below it is done, so at most three
//!   cell rows of accumulators are held.

use crate::Error;
use crate::tensor::Tensor;

/// The tile layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tiling {
    /// Side of the network's input (a multiple of the network's side step).
    pub size: usize,
    /// Input thrown away at each side of a tile.
    pub margin: usize,
    /// Width of the cross-fade between neighbouring tiles (even).
    pub overlap: usize,
}

/// Pixels of an image, read by rectangles.
pub trait Pixels {
    /// The image's size.
    fn size(&self) -> (usize, usize);
    /// Fills `out` (planar: three `w × h` planes) with the pixels of the rectangle at
    /// `(x, y)`, coordinates outside the image clamped to its edge.
    fn fill(&self, x: i64, y: i64, w: usize, h: usize, out: &mut [f32]);
}

/// Planar RGB in memory (three `w × h` planes).
pub struct Planar<'a> {
    pub w: usize,
    pub h: usize,
    pub data: &'a [f32],
}

impl Pixels for Planar<'_> {
    fn size(&self) -> (usize, usize) {
        (self.w, self.h)
    }

    fn fill(&self, x: i64, y: i64, w: usize, h: usize, out: &mut [f32]) {
        let n = self.w * self.h;
        let cx = |v: i64| v.clamp(0, self.w as i64 - 1) as usize;
        let cy = |v: i64| v.clamp(0, self.h as i64 - 1) as usize;
        for c in 0..3 {
            let plane = &self.data[c * n..(c + 1) * n];
            for r in 0..h {
                let row = cy(y + r as i64) * self.w;
                let dst = &mut out[(c * h + r) * w..(c * h + r + 1) * w];
                for (k, d) in dst.iter_mut().enumerate() {
                    *d = plane[row + cx(x + k as i64)];
                }
            }
        }
    }
}

/// A rectangle of cells, `[x0, x1) × [y0, y1)` in cell units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cells {
    pub x0: usize,
    pub y0: usize,
    pub x1: usize,
    pub y1: usize,
}

/// One finished cell row: pixels `[x, x + w) × [y, y + h)` of the image, planar RGB.
pub struct Rows<'a> {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub data: &'a [f32],
}

impl Tiling {
    /// The tile layout NAFNet runs with: 512² inputs, 16 px margins, 32 px cross-fades.
    pub const DEFAULT: Tiling = Tiling {
        size: 512,
        margin: 16,
        overlap: 32,
    };

    /// 256² inputs, for wide networks (NAFNet width 64: 512² tiles exhaust an integrated
    /// GPU's memory and lose the device).
    pub const SMALL: Tiling = Tiling {
        size: 256,
        margin: 16,
        overlap: 32,
    };

    /// FFDNet's: no op reaches past [`crate::ffdnet::REACH`] pixels, so a 32-pixel margin
    /// makes tiles join exactly (the overlap only guards the last float bits).
    pub const FFDNET: Tiling = Tiling {
        size: 512,
        margin: 32,
        overlap: 16,
    };

    /// The tiles for a NAFNet of `width` channels.
    pub fn for_width(width: usize) -> Tiling {
        if width <= 32 {
            Tiling::DEFAULT
        } else {
            Tiling::SMALL
        }
    }

    /// Cell side in pixels.
    pub fn stride(&self) -> usize {
        self.size - 2 * self.margin - self.overlap
    }

    fn valid(&self) -> bool {
        self.overlap.is_multiple_of(2) && self.size > 2 * self.margin + self.overlap
    }

    /// The cells of a `w × h` image.
    pub fn cells(&self, w: usize, h: usize) -> (usize, usize) {
        let s = self.stride().max(1);
        (w.div_ceil(s), h.div_ceil(s))
    }

    /// The cells covering the pixel rectangle `[x0, x1) × [y0, y1)` of a `w × h` image.
    pub fn cells_of(
        &self,
        (w, h): (usize, usize),
        x0: usize,
        y0: usize,
        x1: usize,
        y1: usize,
    ) -> Cells {
        let s = self.stride().max(1);
        let (cw, ch) = self.cells(w, h);
        Cells {
            x0: (x0 / s).min(cw),
            y0: (y0 / s).min(ch),
            x1: x1.div_ceil(s).min(cw),
            y1: y1.div_ceil(s).min(ch),
        }
    }

    /// Network runs needed for `cells` of a `w × h` image.
    pub fn tiles_for(&self, (w, h): (usize, usize), cells: Cells) -> usize {
        let (cw, ch) = self.cells(w, h);
        let span = |a: usize, b: usize, n: usize| {
            if a >= b {
                0
            } else {
                (b + 1).min(n) - a.saturating_sub(1)
            }
        };
        span(cells.x0, cells.x1, cw) * span(cells.y0, cells.y1, ch)
    }

    /// The weight of usable-area coordinate `t` (0 .. stride + overlap).
    fn ramp(&self, t: usize) -> f32 {
        let o = self.overlap as f32;
        if o == 0.0 {
            return 1.0;
        }
        let u = (self.stride() + self.overlap) as f32;
        let t = t as f32 + 0.5;
        (t / o).min((u - t) / o).clamp(0.0, 1.0)
    }

    /// Runs `net` (a `[1, 3, size, size]` tensor in, the same shape out) over `cells` of
    /// `src`, handing each finished cell row to `emit`, top to bottom. `progress(done,
    /// total)` is called after each tile; when it returns false the run stops and returns
    /// `Ok(false)`.
    pub fn run(
        &self,
        src: &dyn Pixels,
        cells: Cells,
        net: &mut dyn FnMut(&Tensor) -> Result<Tensor, Error>,
        emit: &mut dyn FnMut(Rows<'_>),
        progress: &mut dyn FnMut(usize, usize) -> bool,
    ) -> Result<bool, Error> {
        if !self.valid() {
            return Err(Error::Shape(format!("bad tiling {self:?}")));
        }
        let (w, h) = src.size();
        let (cw, ch) = self.cells(w, h);
        if w == 0 || h == 0 || cells.x0 >= cells.x1.min(cw) || cells.y0 >= cells.y1.min(ch) {
            return Ok(true);
        }
        let (s, n, m, half) = (self.stride(), self.size, self.margin, self.overlap / 2);
        let u = s + self.overlap;
        let total = self.tiles_for((w, h), cells);
        let (tx0, tx1) = (cells.x0.saturating_sub(1), (cells.x1 + 1).min(cw));
        let (ty0, ty1) = (cells.y0.saturating_sub(1), (cells.y1 + 1).min(ch));
        // Output pixels of the cell rectangle.
        let ox0 = cells.x0 * s;
        let ox1 = (cells.x1 * s).min(w);
        let ow = ox1 - ox0;
        // Accumulators per cell row: (row, sum planes, weights).
        let mut acc: Vec<(usize, Vec<f32>, Vec<f32>)> = Vec::new();
        let rows_of = |j: usize| (j * s, ((j + 1) * s).min(h));
        let mut input = vec![0f32; 3 * n * n];
        let mut done = 0;
        for ty in ty0..ty1 {
            for j in ty.saturating_sub(1)..=(ty + 1) {
                if j >= cells.y0 && j < cells.y1.min(ch) && !acc.iter().any(|a| a.0 == j) {
                    let (r0, r1) = rows_of(j);
                    let len = (r1 - r0) * ow;
                    acc.push((j, vec![0.0; 3 * len], vec![0.0; len]));
                }
            }
            for tx in tx0..tx1 {
                // The usable area's top left, and the input's.
                let (ux, uy) = (
                    tx as i64 * s as i64 - half as i64,
                    ty as i64 * s as i64 - half as i64,
                );
                src.fill(ux - m as i64, uy - m as i64, n, n, &mut input);
                let out = net(&Tensor::f32(&[1, 3, n, n], input.clone()))?;
                if out.shape != [1, 3, n, n] {
                    return Err(Error::Shape(format!("tile output {:?}", out.shape)));
                }
                let ov = out.to_f32();
                for (j, sum, wt) in acc.iter_mut() {
                    let (r0, r1) = rows_of(*j);
                    let rh = r1 - r0;
                    let ya = (r0 as i64).max(uy) as usize;
                    let yb = (r1 as i64).min(uy + u as i64).max(ya as i64) as usize;
                    let xa = (ox0 as i64).max(ux) as usize;
                    let xb = (ox1 as i64).min(ux + u as i64).max(xa as i64) as usize;
                    for y in ya..yb {
                        let t_y = (y as i64 - uy) as usize;
                        let wy = self.ramp(t_y);
                        let iy = t_y + m;
                        for x in xa..xb {
                            let t_x = (x as i64 - ux) as usize;
                            let wgt = wy * self.ramp(t_x);
                            if wgt <= 0.0 {
                                continue;
                            }
                            let ix = t_x + m;
                            let o = (y - r0) * ow + (x - ox0);
                            wt[o] += wgt;
                            for c in 0..3 {
                                sum[c * rh * ow + o] += wgt * ov[(c * n + iy) * n + ix];
                            }
                        }
                    }
                }
                done += 1;
                if !progress(done, total) {
                    return Ok(false);
                }
            }
            // Cell rows whose last tile row is done.
            let mut finished: Vec<usize> = acc
                .iter()
                .map(|a| a.0)
                .filter(|&j| j < ty || ty + 1 == ty1)
                .collect();
            finished.sort_unstable();
            for j in finished {
                let i = acc.iter().position(|a| a.0 == j).expect("present");
                let (_, mut sum, wt) = acc.swap_remove(i);
                let (r0, r1) = rows_of(j);
                let len = (r1 - r0) * ow;
                for c in 0..3 {
                    for (v, &q) in sum[c * len..(c + 1) * len].iter_mut().zip(&wt) {
                        *v /= q.max(1e-6);
                    }
                }
                emit(Rows {
                    x: ox0,
                    y: r0,
                    w: ow,
                    h: r1 - r0,
                    data: &sum,
                });
            }
        }
        Ok(true)
    }
}
