//! Frame geometry: planes, blocks, super blocks and macro blocks, and their coded orders
//! (spec 2.3, 2.4). Block indices here are raster indices over all three planes (the planes
//! one after another, each bottom row first); coded order is kept as a list of them.

use crate::Chroma;

/// Pixels of padding around every reference plane (motion vectors reach 16 pixels out).
pub(crate) const BORDER: usize = 16;

/// Hilbert order of the 16 blocks of a super block, as (x, y) with y up.
const HILBERT: [(u32, u32); 16] = [
    (0, 0),
    (1, 0),
    (1, 1),
    (0, 1),
    (0, 2),
    (0, 3),
    (1, 3),
    (1, 2),
    (2, 2),
    (2, 3),
    (3, 3),
    (3, 2),
    (3, 1),
    (2, 1),
    (2, 0),
    (3, 0),
];

/// Hilbert order of the four macro blocks of a super block.
const MB_HILBERT: [(u32, u32); 4] = [(0, 0), (0, 1), (1, 1), (1, 0)];

#[derive(Clone, Copy)]
pub(crate) struct PlaneGeom {
    /// Size in blocks.
    pub(crate) bw: usize,
    pub(crate) bh: usize,
    /// Size in pixels.
    pub(crate) w: usize,
    pub(crate) h: usize,
    /// Raster index of the plane's first block.
    pub(crate) first: usize,
    /// Row stride of the padded buffer, and the offset of pixel (0, 0) in it.
    pub(crate) stride: usize,
    pub(crate) origin: usize,
    /// Buffer length.
    pub(crate) len: usize,
    /// Chroma subsampling shifts (0 for luma).
    pub(crate) sx: u32,
    pub(crate) sy: u32,
}

pub(crate) struct Layout {
    pub(crate) planes: [PlaneGeom; 3],
    pub(crate) nbs: usize,
    pub(crate) mbw: usize,
    pub(crate) mbh: usize,
    /// Raster block indices in coded order.
    pub(crate) coded_order: Vec<u32>,
    /// Per super block (coded order): the end of its run in `coded_order`.
    pub(crate) sb_end: Vec<u32>,
    /// Macro block raster indices (`my * mbw + mx`) in coded order.
    pub(crate) mb_order: Vec<u32>,
    /// Raster block index -> raster macro block index.
    pub(crate) blk_mb: Vec<u32>,
}

impl Layout {
    pub(crate) fn new(mbw: usize, mbh: usize, chroma: Chroma) -> Layout {
        let (sx, sy) = match chroma {
            Chroma::C420 => (1, 1),
            Chroma::C422 => (1, 0),
            Chroma::C444 => (0, 0),
        };
        let mut planes = [PlaneGeom {
            bw: 0,
            bh: 0,
            w: 0,
            h: 0,
            first: 0,
            stride: 0,
            origin: 0,
            len: 0,
            sx: 0,
            sy: 0,
        }; 3];
        let mut first = 0;
        for (p, g) in planes.iter_mut().enumerate() {
            let (px, py) = if p == 0 { (0, 0) } else { (sx, sy) };
            let w = (mbw * 16) >> px;
            let h = (mbh * 16) >> py;
            let stride = w + 2 * BORDER;
            *g = PlaneGeom {
                bw: w / 8,
                bh: h / 8,
                w,
                h,
                first,
                stride,
                origin: BORDER * stride + BORDER,
                len: stride * (h + 2 * BORDER),
                sx: px,
                sy: py,
            };
            first += g.bw * g.bh;
        }
        let nbs = first;
        let mut coded_order = Vec::with_capacity(nbs);
        let mut sb_end = Vec::new();
        let mut blk_mb = vec![0u32; nbs];
        for g in &planes {
            for sby in 0..g.bh.div_ceil(4) {
                for sbx in 0..g.bw.div_ceil(4) {
                    for &(hx, hy) in &HILBERT {
                        let x = sbx * 4 + hx as usize;
                        let y = sby * 4 + hy as usize;
                        if x < g.bw && y < g.bh {
                            coded_order.push((g.first + y * g.bw + x) as u32);
                        }
                    }
                    sb_end.push(coded_order.len() as u32);
                }
            }
            // Block -> macro block: luma 2x2 blocks per macro block, chroma by subsampling.
            let (mx_shift, my_shift) = if g.first == 0 {
                (1, 1)
            } else {
                (1 - g.sx, 1 - g.sy)
            };
            for y in 0..g.bh {
                for x in 0..g.bw {
                    blk_mb[g.first + y * g.bw + x] =
                        ((y >> my_shift) * mbw + (x >> mx_shift)) as u32;
                }
            }
        }
        let mut mb_order = Vec::with_capacity(mbw * mbh);
        for sby in 0..mbh.div_ceil(2) {
            for sbx in 0..mbw.div_ceil(2) {
                for &(hx, hy) in &MB_HILBERT {
                    let x = sbx * 2 + hx as usize;
                    let y = sby * 2 + hy as usize;
                    if x < mbw && y < mbh {
                        mb_order.push((y * mbw + x) as u32);
                    }
                }
            }
        }
        Layout {
            planes,
            nbs,
            mbw,
            mbh,
            coded_order,
            sb_end,
            mb_order,
            blk_mb,
        }
    }

    /// Raster index of block (x, y) of plane `p`.
    #[inline]
    pub(crate) fn blk(&self, p: usize, x: usize, y: usize) -> usize {
        let g = &self.planes[p];
        g.first + y * g.bw + x
    }
}
