//! The tile scheduler.
//!
//! A request renders in one piece when its working set fits the memory budget and every
//! binding fits the device's limit. Otherwise it renders in square tiles of 2048, 1024, 512
//! or 256 output pixels, the largest that fits. Each tile carries its own halo chain
//! ([`Graph::rois`]) and samples clamp to the image edge, never the tile edge, so tiled
//! output is the same as untiled output.

use super::{Graph, Precision, Roi};

/// How a request may be split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tiling {
    /// Untiled when it fits, else the largest tile side that does.
    Auto,
    /// Never tiled (fails when it does not fit).
    Off,
    /// Tiles of this many output pixels a side (tests, export tuning).
    Side(u32),
}

/// Device and budget limits the plan must keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Bytes of one storage binding (`max_storage_buffer_binding_size`).
    pub max_binding: u64,
    /// Storage binding offsets are multiples of this (`min_storage_buffer_offset_alignment`).
    pub offset_align: u64,
    /// Bytes the intermediate buffers of one tile may use at once.
    pub working: u64,
}

/// The tiles of one request, in row-major order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TilePlan {
    /// None when untiled.
    pub side: Option<u32>,
    pub tiles: Vec<Roi>,
    /// The largest working set of any tile, in bytes.
    pub peak: u64,
}

/// Why a request cannot be planned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooLarge {
    /// Video memory the smallest plan tried would need.
    pub needs: u64,
}

const SIDES: [u32; 4] = [2048, 1024, 512, 256];

/// What one request looks like to the planner.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub out: Roi,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub precision: Precision,
    /// Bytes per pixel of the source buffer.
    pub source_bpp: u64,
}

/// Plans `shape` under `limits`.
pub fn plan(
    graph: &Graph,
    shape: &Shape,
    tiling: Tiling,
    limits: &Limits,
) -> Result<TilePlan, TooLarge> {
    let candidates: Vec<Option<u32>> = match tiling {
        Tiling::Off => vec![None],
        Tiling::Side(s) => vec![Some(s.max(16))],
        Tiling::Auto => std::iter::once(None)
            .chain(SIDES.iter().map(|&s| Some(s)))
            .collect(),
    };
    let mut needs = u64::MAX;
    for side in candidates {
        let tiles = split(shape.out, side);
        let mut peak = 0;
        let mut fits = true;
        for t in &tiles {
            let (p, ok) = cost(graph, shape, t, limits);
            peak = peak.max(p);
            fits &= ok;
        }
        if fits && peak <= limits.working {
            return Ok(TilePlan { side, tiles, peak });
        }
        needs = needs.min(peak);
    }
    Err(TooLarge { needs })
}

/// `out` in row-major tiles of `side` (or whole).
pub fn split(out: Roi, side: Option<u32>) -> Vec<Roi> {
    let Some(s) = side else {
        return vec![out];
    };
    let mut tiles = Vec::new();
    let mut y = out.y;
    while y < out.bottom() {
        let h = s.min(out.bottom() - y);
        let mut x = out.x;
        while x < out.right() {
            let w = s.min(out.right() - x);
            tiles.push(Roi::new(x, y, w, h));
            x += w;
        }
        y += h;
    }
    tiles
}

/// Bytes of a binding covering rows `rows` of a buffer `stride` pixels wide, from any start
/// row (the offset is aligned down, so up to one alignment more).
pub fn row_binding(rows: u32, stride: u32, bpp: u64, align: u64) -> u64 {
    u64::from(rows) * u64::from(stride) * bpp + align
}

/// The working set of one tile and whether every binding fits.
fn cost(graph: &Graph, s: &Shape, tile: &Roi, l: &Limits) -> (u64, bool) {
    let bpp = s.precision.bytes_per_pixel();
    let rois = graph.rois(*tile, s.width, s.height, s.scale);
    let n = rois.len();
    let mut fits = true;
    // The source is bound by rows of the whole image; the output by rows of the request.
    let first_in = rois.first().map_or(*tile, |r| r.input);
    fits &= row_binding(first_in.h, s.width, s.source_bpp, l.offset_align) <= l.max_binding;
    fits &= row_binding(tile.h, s.out.w, bpp, l.offset_align) <= l.max_binding;
    let mut peak = 0;
    let mut kept = 0;
    for (g, r) in rois.iter().enumerate() {
        let out_bytes = if g + 1 < n {
            r.output.pixels() * bpp
        } else {
            0
        };
        let in_bytes = if g > 0 { r.input.pixels() * bpp } else { 0 };
        // A local program's scratch (pyramids) lives while the group runs.
        let local = graph.local(g, r.input, s.scale).map_or(0, |p| p.words * 4);
        fits &= out_bytes <= l.max_binding && local <= l.max_binding;
        peak = peak.max(kept + in_bytes + out_bytes + local);
        if graph.groups[g].checkpoint && g + 1 < n {
            kept += out_bytes;
        }
    }
    (peak.max(kept), fits)
}
