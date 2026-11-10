//! Reconstructed picture storage and per-block side information.

use crate::error::{Error, Result};
use alloc::vec::Vec;

/// A single colour plane of `u16` samples with an explicit stride.
#[derive(Debug, Clone)]
pub struct Plane {
    /// Sample storage, `stride * height` entries.
    pub data: Vec<u16>,
    /// Distance between vertically adjacent samples, in samples.
    pub stride: usize,
    /// Plane width in samples.
    pub width: usize,
    /// Plane height in samples (of the whole picture, not just the rows held).
    pub height: usize,
    /// First row held in `data`: zero for a whole picture, the top of the
    /// halo for a wavefront band.
    pub oy: usize,
    /// First column held in `data` (`stride` columns are held): zero for a
    /// whole picture, the left edge of the unit for a band.
    pub ox: usize,
}

impl Plane {
    /// Makes this plane hold columns `ox .. ox + cols` of rows
    /// `oy .. oy + rows` of a `width` x `height` plane, all zero, reusing
    /// the storage.
    pub fn reshape(
        &mut self,
        width: usize,
        height: usize,
        (ox, cols): (usize, usize),
        (oy, rows): (usize, usize),
    ) -> Result<()> {
        let n = cols.checked_mul(rows).ok_or(Error::TooLarge)?;
        if n > (1usize << 30) {
            return Err(Error::TooLarge);
        }
        self.data.clear();
        self.data.resize(n, 0);
        self.stride = cols;
        self.width = width;
        self.height = height;
        self.oy = oy;
        self.ox = ox;
        Ok(())
    }

    /// Offset of `(x, y)` into `data`.
    #[inline]
    pub fn offset(&self, x: usize, y: usize) -> usize {
        (y - self.oy) * self.stride + x - self.ox
    }

    /// Sample at `(x, y)`.
    #[inline]
    pub fn at(&self, x: usize, y: usize) -> u16 {
        self.data[self.offset(x, y)]
    }

    /// Writes the sample at `(x, y)`.
    #[inline]
    pub fn put(&mut self, x: usize, y: usize, v: u16) {
        let o = self.offset(x, y);
        self.data[o] = v;
    }
}

/// A mutable view of some rows of a [`Plane`], addressed in whole-plane
/// coordinates: the in-loop filters run on the whole plane or, in
/// parallel, on disjoint row bands of it.
#[derive(Debug)]
pub struct PlaneMut<'a> {
    /// Rows `oy ..` of the plane.
    pub data: &'a mut [u16],
    /// Distance between vertically adjacent samples, in samples.
    pub stride: usize,
    /// Plane width in samples.
    pub width: usize,
    /// Plane height in samples (of the whole plane).
    pub height: usize,
    /// First row held in `data`.
    pub oy: usize,
}

impl PlaneMut<'_> {
    /// Offset of `(x, y)` into `data`.
    #[inline]
    pub fn offset(&self, x: usize, y: usize) -> usize {
        (y - self.oy) * self.stride + x
    }

    /// Writes the sample at `(x, y)`.
    #[inline]
    pub fn put(&mut self, x: usize, y: usize, v: u16) {
        let o = self.offset(x, y);
        self.data[o] = v;
    }

    /// One past the last row held.
    #[inline]
    pub fn end(&self) -> usize {
        self.oy + self.data.len().checked_div(self.stride).unwrap_or(0)
    }
}

impl Plane {
    /// A view of every row held.
    pub fn view(&mut self) -> PlaneMut<'_> {
        PlaneMut {
            data: &mut self.data,
            stride: self.stride,
            width: self.width,
            height: self.height,
            oy: self.oy,
        }
    }

    /// A view of rows `a..b` (whole-plane coordinates, within the rows held).
    pub fn rows_mut(&mut self, a: usize, b: usize) -> PlaneMut<'_> {
        let s = (a.saturating_sub(self.oy) * self.stride).min(self.data.len());
        let e = (b.saturating_sub(self.oy) * self.stride).clamp(s, self.data.len());
        PlaneMut {
            data: &mut self.data[s..e],
            stride: self.stride,
            width: self.width,
            height: self.height,
            oy: a.max(self.oy),
        }
    }

    /// Disjoint views of the rows held, cut before each row of `cuts`
    /// (increasing; cuts outside the rows held are ignored).
    pub fn split_rows(&mut self, cuts: &[usize]) -> Vec<PlaneMut<'_>> {
        let (stride, width, height) = (self.stride, self.width, self.height);
        let held = self.data.len().checked_div(stride).unwrap_or(0);
        let end = self.oy + held;
        let mut oy = self.oy;
        let mut out = Vec::new();
        let mut rest: &mut [u16] = &mut self.data;
        for &c in cuts {
            if c <= oy || c >= end {
                continue;
            }
            let (a, b) = core::mem::take(&mut rest).split_at_mut((c - oy) * stride);
            out.push(PlaneMut {
                data: a,
                stride,
                width,
                height,
                oy,
            });
            rest = b;
            oy = c;
        }
        out.push(PlaneMut {
            data: rest,
            stride,
            width,
            height,
            oy,
        });
        out
    }
}

/// Bit flags stored per minimum transform block (4x4 luma samples).
pub mod flags {
    /// `cu_transquant_bypass_flag` is set for the covering coding unit.
    pub const TQ_BYPASS: u8 = 1;
    /// `pcm_flag` is set for the covering coding unit.
    pub const PCM: u8 = 2;
    /// The covering coding unit is inter predicted (`CuPredMode` is not
    /// `MODE_INTRA`).
    pub const INTER: u8 = 4;
    /// `cu_skip_flag` is set for the covering coding unit.
    pub const SKIP: u8 = 8;
    /// The covering luma transform block has non-zero coefficients.
    pub const NZ: u8 = 16;
}

/// Edge flags stored per 8x8 luma grid cell for the deblocking filter.
pub mod edge {
    /// A transform or prediction boundary runs along the cell's left side.
    pub const VER: u8 = 1;
    /// A transform or prediction boundary runs along the cell's top side.
    pub const HOR: u8 = 2;
    /// A prediction block boundary that is not a transform block boundary
    /// runs along the cell's left side.
    pub const PU_VER: u8 = 4;
    /// A prediction block boundary that is not a transform block boundary
    /// runs along the cell's top side.
    pub const PU_HOR: u8 = 8;
}

/// The motion of one 4x4 luma block (clause 8.5.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MvField {
    /// `MvL0` and `MvL1`, quarter luma samples.
    pub mv: [[i16; 2]; 2],
    /// `RefIdxL0` and `RefIdxL1`; -1 where the list is not used (both -1
    /// for an intra block).
    pub ref_idx: [i8; 2],
    /// The referenced pictures as indices into the picture's reference
    /// table, so that blocks of different slices compare.
    pub ref_id: [u8; 2],
}

impl Default for MvField {
    fn default() -> MvField {
        MvField {
            mv: [[0; 2]; 2],
            ref_idx: [-1; 2],
            ref_id: [0; 2],
        }
    }
}

impl MvField {
    /// `PredFlagLX`.
    #[inline]
    pub fn pred(&self, l: usize) -> bool {
        self.ref_idx[l] >= 0
    }
}

/// The picture being reconstructed, plus the side information later stages need.
#[derive(Debug)]
pub struct Picture {
    /// Luma plane.
    pub y: Plane,
    /// Cb plane; zero sized for monochrome.
    pub cb: Plane,
    /// Cr plane; zero sized for monochrome.
    pub cr: Plane,
    /// Width of the minimum-transform-block grid held: the stride of the
    /// per-4x4 arrays (of the whole grid unless this is a band).
    pub min4_w: usize,
    /// Height of the minimum-transform-block grid.
    pub min4_h: usize,
    /// `IntraPredModeY` per 4x4 luma block.
    pub intra_mode: Vec<u8>,
    /// `QpY` per 4x4 luma block.
    pub qp_y: Vec<i8>,
    /// Per-4x4 coding unit flags, see [`flags`].
    pub cu_flags: Vec<u8>,
    /// `CtDepth` per 4x4 luma block, used for `split_cu_flag` contexts.
    pub ct_depth: Vec<u8>,
    /// Width of the 8x8 deblocking grid held: the stride of `edges`.
    pub min8_w: usize,
    /// Height of the 8x8 deblocking grid.
    pub min8_h: usize,
    /// Per-8x8 edge flags, see [`edge`].
    pub edges: Vec<u8>,
    /// First 4x4 row held in the per-4x4 arrays (`oy / 4`).
    pub oy4: usize,
    /// First 8x8 row held in `edges` (`oy / 8`).
    pub oy8: usize,
    /// First 4x4 column held in the per-4x4 arrays (`ox / 4`).
    pub ox4: usize,
    /// First 8x8 column held in `edges` (`ox / 8`).
    pub ox8: usize,
    /// Motion per 4x4 luma block; empty for a picture decoded on its own.
    pub mvf: Vec<MvField>,
    /// Deblocking boundary strength per 4x4 luma block: bits 0-1 for the
    /// block's left edge, bits 2-3 for its top edge. Empty means every
    /// marked edge has strength 2 (an intra picture).
    pub bs: Vec<u8>,
}

impl Picture {
    /// Allocates all planes and side-information arrays for one picture.
    pub fn new(width: usize, height: usize, sub_w: usize, sub_h: usize) -> Result<Picture> {
        Picture::band(width, height, sub_w, sub_h, 0, height)
    }

    /// Allocates luma rows `oy .. oy + rows` of a picture, with the matching
    /// chroma rows and side information. `oy` must be a multiple of eight.
    pub fn band(
        width: usize,
        height: usize,
        sub_w: usize,
        sub_h: usize,
        oy: usize,
        rows: usize,
    ) -> Result<Picture> {
        let mut p = Picture::empty();
        p.reshape(width, height, (sub_w, sub_h), (0, width), (oy, rows))?;
        Ok(p)
    }

    /// A picture holding nothing, to be [`Picture::reshape`]d.
    pub fn empty() -> Picture {
        let plane = || Plane {
            data: Vec::new(),
            stride: 0,
            width: 0,
            height: 0,
            oy: 0,
            ox: 0,
        };
        Picture {
            y: plane(),
            cb: plane(),
            cr: plane(),
            min4_w: 0,
            min4_h: 0,
            intra_mode: Vec::new(),
            qp_y: Vec::new(),
            cu_flags: Vec::new(),
            ct_depth: Vec::new(),
            min8_w: 0,
            min8_h: 0,
            edges: Vec::new(),
            oy4: 0,
            oy8: 0,
            ox4: 0,
            ox8: 0,
            mvf: Vec::new(),
            bs: Vec::new(),
        }
    }

    /// Makes this picture a band holding luma columns `x0..x1` of rows
    /// `oy .. oy + rows` of a `width` x `height` picture, with the matching
    /// chroma and side information, everything as freshly allocated (no
    /// motion), reusing the storage. `x0` and `oy`
    /// must be multiples of eight.
    pub fn reshape(
        &mut self,
        width: usize,
        height: usize,
        (sub_w, sub_h): (usize, usize),
        (x0, x1): (usize, usize),
        (oy, rows): (usize, usize),
    ) -> Result<()> {
        debug_assert!(oy.is_multiple_of(8) && x0.is_multiple_of(8));
        let end = (oy + rows).min(height);
        let x1 = x1.clamp(x0, width);
        self.y
            .reshape(width, height, (x0, x1 - x0), (oy, end - oy))?;
        let (cw, ch) = match (width.checked_div(sub_w), height.checked_div(sub_h)) {
            (Some(a), Some(b)) => (a, b),
            _ => (0, 0),
        };
        let (coy, cend) = match (oy.checked_div(sub_h), end.checked_div(sub_h)) {
            (Some(a), Some(b)) if ch > 0 => (a, if end == height { ch } else { b }),
            _ => (0, 0),
        };
        let (cx0, cx1) = match (x0.checked_div(sub_w), x1.checked_div(sub_w)) {
            (Some(a), Some(b)) if cw > 0 => (a, if x1 == width { cw } else { b }),
            _ => (0, 0),
        };
        self.cb
            .reshape(cw, ch, (cx0, cx1 - cx0), (coy, cend - coy))?;
        self.cr
            .reshape(cw, ch, (cx0, cx1 - cx0), (coy, cend - coy))?;
        self.min4_h = height.div_ceil(4);
        self.min8_h = height.div_ceil(8);
        (self.ox4, self.ox8) = (x0 / 4, x0 / 8);
        (self.oy4, self.oy8) = (oy / 4, oy / 8);
        self.min4_w = x1.div_ceil(4) - self.ox4;
        self.min8_w = x1.div_ceil(8) - self.ox8;
        let n4 = self.min4_w * (end.div_ceil(4) - self.oy4);
        let n8 = self.min8_w * (end.div_ceil(8) - self.oy8);
        let reset = |v: &mut Vec<u8>, n: usize, x: u8| {
            v.clear();
            v.resize(n, x);
        };
        reset(&mut self.intra_mode, n4, 1);
        reset(&mut self.cu_flags, n4, 0);
        reset(&mut self.ct_depth, n4, 0);
        self.qp_y.clear();
        self.qp_y.resize(n4, 0);
        reset(&mut self.edges, n8, 0);
        self.mvf.clear();
        self.bs.clear();
        Ok(())
    }

    /// One past the last luma column held.
    #[inline]
    pub fn x_end(&self) -> usize {
        (self.y.ox + self.y.stride).min(self.y.width)
    }

    /// The indices into the per-4x4 arrays of luma columns `a..b` (within
    /// the columns held) of the 4x4 row holding luma row `y`.
    #[inline]
    pub fn range4(&self, a: usize, b: usize, y: usize) -> core::ops::Range<usize> {
        let row = (y / 4 - self.oy4) * self.min4_w;
        let (a4, b4) = (
            a / 4 - self.ox4,
            (b.div_ceil(4) - self.ox4).min(self.min4_w),
        );
        row + a4..row + b4
    }

    /// Where the per-4x4 arrays sit in the 4x4 grid.
    #[inline]
    pub fn g4(&self) -> Grid {
        Grid {
            w: self.min4_w,
            ox: self.ox4,
            oy: self.oy4,
        }
    }

    /// Index into the 4x4 side-information arrays for luma position `(x, y)`.
    ///
    /// Positions outside the picture clamp to the last block so that callers
    /// never index out of bounds.
    #[inline]
    pub fn idx4(&self, x: usize, y: usize) -> usize {
        let xi = (x >> 2).saturating_sub(self.ox4).min(self.min4_w - 1);
        let yi = (y >> 2).min(self.min4_h - 1).saturating_sub(self.oy4);
        (yi * self.min4_w + xi).min(self.qp_y.len() - 1)
    }

    /// Records the deblocking boundaries of a `size` x `size` block at `(x, y)`.
    ///
    /// Only boundaries that fall on the 8x8 luma grid are filtered, so an edge
    /// at an odd multiple of four samples is not recorded. The whole extent of
    /// each boundary is marked, not just the corner it starts at.
    pub fn mark_edge(&mut self, x: usize, y: usize, size: usize, kind: u8) {
        let xe = self.x_end();
        if x >= xe || y >= self.y.height || x < self.y.ox {
            return;
        }
        if kind & edge::VER != 0 && x & 7 == 0 {
            let col = (x >> 3) - self.ox8;
            let mut yy = y;
            while yy < (y + size).min(self.y.height) {
                if let Some(e) = self
                    .edges
                    .get_mut(((yy >> 3) - self.oy8) * self.min8_w + col)
                {
                    *e |= edge::VER;
                }
                yy += 8;
            }
        }
        if kind & edge::HOR != 0 && y & 7 == 0 {
            let row = ((y >> 3) - self.oy8) * self.min8_w;
            let mut xx = x;
            while xx < (x + size).min(xe) {
                if let Some(e) = self.edges.get_mut(row + (xx >> 3) - self.ox8) {
                    *e |= edge::HOR;
                }
                xx += 8;
            }
        }
    }

    /// Records the prediction block boundaries of the `w` x `h` block at
    /// `(x, y)`: its left and top sides, where they fall on the 8x8 grid.
    pub fn mark_pu_edges(&mut self, x: usize, y: usize, w: usize, h: usize) {
        let xe = self.x_end();
        if x >= xe || y >= self.y.height || x < self.y.ox {
            return;
        }
        if x & 7 == 0 {
            let mut yy = y & !7;
            while yy < (y + h).min(self.y.height) {
                if let Some(e) = self
                    .edges
                    .get_mut(((yy >> 3) - self.oy8) * self.min8_w + (x >> 3) - self.ox8)
                {
                    *e |= edge::PU_VER;
                }
                yy += 8;
            }
        }
        if y & 7 == 0 {
            let row = ((y >> 3) - self.oy8) * self.min8_w;
            let mut xx = x & !7;
            while xx < (x + w).min(xe) {
                if let Some(e) = self.edges.get_mut(row + (xx >> 3) - self.ox8) {
                    *e |= edge::PU_HOR;
                }
                xx += 8;
            }
        }
    }

    /// Fills a `w` x `h` rectangle of a per-4x4 array laid out as `g`.
    #[allow(clippy::too_many_arguments)]
    pub fn fill4_rect<T: Copy>(
        g: Grid,
        arr: &mut [T],
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        value: T,
    ) {
        let Some(x0) = (x >> 2).checked_sub(g.ox) else {
            return;
        };
        let y0 = (y >> 2) - g.oy;
        let nw = (w >> 2).min(g.w.saturating_sub(x0));
        for j in 0..h >> 2 {
            let row = (y0 + j) * g.w + x0;
            if let Some(s) = arr.get_mut(row..row + nw) {
                s.fill(value);
            }
        }
    }

    /// Fills a `size` x `size` square of a per-4x4 array laid out as `g`.
    pub fn fill4(g: Grid, arr: &mut [u8], x: usize, y: usize, size: usize, value: u8) {
        Picture::fill4_rect(g, arr, x, y, size, size, value);
    }
}

/// Where a per-4x4 array sits in the picture's 4x4 grid: its stride (the
/// columns held), and its first column and row.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub w: usize,
    pub ox: usize,
    pub oy: usize,
}
