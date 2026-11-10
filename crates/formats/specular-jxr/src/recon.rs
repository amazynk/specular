//! Sample reconstruction (T.832 9.9): first-level inverse transform on the DC grid, optional
//! first-level overlap filtering, second-level inverse transform and overlap filtering.

use rayon::prelude::*;

use crate::header::{ClrFmt, ImageHeader};
use crate::parse::Plane;
use crate::transform::{Chroma, View, first_level_mb, ict_plane, overlap2, overlap4};

/// Region boundaries (half-open) in units of `per_mb` samples per macroblock: the whole extent
/// for soft tiling, every tile for hard tiling.
fn regions(bounds: &[u32], per_mb: usize, hard: bool) -> Vec<(usize, usize)> {
    if hard {
        bounds
            .windows(2)
            .map(|w| (w[0] as usize * per_mb, w[1] as usize * per_mb))
            .collect()
    } else {
        vec![(0, *bounds.last().unwrap_or(&0) as usize * per_mb)]
    }
}

/// Reconstructs every component of `p` in place into extended-size sample planes.
pub(crate) fn reconstruct(p: &mut Plane, ih: &ImageHeader, parallel: bool) {
    let scaled = p.ph.scaled;
    let clr = p.ph.clr;
    for i in 0..p.ph.nc {
        let (sx, sy) = p.sub(i);
        let kind = match (i, clr) {
            (0, _) => Chroma::Full,
            (_, ClrFmt::Yuv420) => Chroma::C420,
            (_, ClrFmt::Yuv422) => Chroma::C422,
            _ => Chroma::Full,
        };
        let nb = p.nblk(i);
        let cw = p.cw[i];
        let mbw = p.mbw;
        let scale = scaled && i > 0;
        // First-level inverse transform, one macroblock row at a time.
        let rows_per_mb = 16 >> sy;
        let bcols = 4 >> sx;
        let mbw_samples = 16 >> sx;
        let one_row = |rows: &mut [i32]| {
            for mbx in 0..mbw {
                let at = |j: usize| (4 * (j / bcols)) * cw + mbx * mbw_samples + 4 * (j % bcols);
                let mut c = [0i32; 16];
                for (j, v) in c.iter_mut().enumerate().take(nb) {
                    *v = rows[at(j)];
                }
                first_level_mb(&mut c, kind, scale);
                for (j, &v) in c.iter().enumerate().take(nb) {
                    rows[at(j)] = v;
                }
            }
        };
        let d = &mut p.comp[i];
        if parallel {
            d.par_chunks_mut(rows_per_mb * cw).for_each(one_row);
        } else {
            d.chunks_mut(rows_per_mb * cw).for_each(one_row);
        }
        let hard = ih.hard_tiling;
        if ih.overlap == 2 {
            let cols = regions(&ih.tile_cols, 4 >> sx, hard);
            let rows = regions(&ih.tile_rows, 4 >> sy, hard);
            let v = View { xs: 4, ys: 4 * cw };
            if kind == Chroma::Full {
                overlap4(d, v, &cols, &rows, false);
            } else {
                overlap2(d, v, &cols, &rows);
            }
        }
        ict_plane(d, cw, parallel);
        if ih.overlap != 0 {
            let cols = regions(&ih.tile_cols, 16 >> sx, hard);
            let rows = regions(&ih.tile_rows, 16 >> sy, hard);
            overlap4(d, View { xs: 1, ys: cw }, &cols, &rows, parallel);
        }
    }
}
