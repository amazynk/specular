//! Highlight reconstruction on the CFA: the statistics' host side.
//!
//! The GPU pass (`hl` in `ops/raw/correct.wgsl`) works per clipped sample with the
//! unclipped samples of the other colours around it (a 3x3 window holds every colour on
//! Bayer and X-Trans) and a **chromaticity** `χ` (camera RGB after white balance, summing
//! to 1):
//!
//! - the scale `s` is the larger of what the unclipped channels say (`Σ m_k / Σ χ_k`) and
//!   what the clipped ones demand (`v_j / χ_j`: a clipped value is a lower bound);
//! - a clipped sample of colour `c` becomes `max(v, s·χ_c)`.
//!
//! So clipped channels only rise, never fall, and they take the hue the scene has around
//! the clipped area. With every channel clipped the result is `χ` at the brightest level
//! the samples allow; with no colour around, `χ` is neutral, so a blown white stays white
//! and never turns magenta (green clipping first, before white balance, is what makes
//! magenta).
//!
//! **Where `χ` comes from (opposed, the default).** The GPU takes cells (2x2 Bayer, 3x3
//! X-Trans) and, per tile of [`TILE_CELLS`]² cells, sums the chromaticity of unclipped
//! cells within two cells of a clipped one (the border of each clipped area), weighted by
//! brightness squared, so the colour just below clipping counts most. [`chroma_grid`]
//! fills the tiles without such cells by push-pull from coarser levels, so each clipped
//! area gets the colour of its own border, blending smoothly between areas.
//!
//! **Rebuild** (large blown areas) adds, per connected area of blown blocks
//! ([`BLOCK_CELLS`]² cells, half or more of their cells clipped in every channel), the
//! colour of that area's own border (not a blend with its neighbours') and a brightness
//! dome: the border's brightness gradient continued inwards, flattening to zero slope at
//! the area's deepest point ([`rebuild`]). Small areas (under [`MIN_SEGMENT`] blocks) are
//! left to opposed.

/// Cells per tile side of the chromaticity grid.
pub const TILE_CELLS: u32 = 16;
/// Cells per block side of the rebuild grid.
pub const BLOCK_CELLS: u32 = 4;
/// Words per tile of the GPU's tile statistics: Σw·χ (3), Σw, candidates, clipped cells.
pub const TILE_WORDS: usize = 6;
/// Words per block of the GPU's block statistics: blown cells, unclipped cells, Σ RGB of
/// the unclipped cells (3), cells.
pub const BLOCK_WORDS: usize = 6;
/// Blocks a blown area needs before it is rebuilt.
pub const MIN_SEGMENT: usize = 4;
/// Candidates for full confidence in a tile's own colour.
const CONFIDENT: f32 = 8.0;
/// The largest relative brightness the dome adds (+2 EV).
const MAX_BOOST: f32 = 3.0;
/// The steepest border slope continued, relative brightness per block.
const MAX_SLOPE: f32 = 0.5;

const NEUTRAL: [f32; 3] = [1.0 / 3.0; 3];

/// The tile statistics of one image (row-major, [`TILE_WORDS`] per tile).
#[derive(Clone, Debug, PartialEq)]
pub struct TileStats {
    pub w: u32,
    pub h: u32,
    pub words: Vec<f32>,
}

/// A tile's (or coarser cell's) Σw·χ, Σw and candidates.
type Sums = ([f64; 3], f64, f64);

/// The chromaticity per tile (`w × h`, row-major), filled by push-pull.
pub fn chroma_grid(t: &TileStats) -> Vec<[f32; 3]> {
    // Level 0: sums and candidate counts.
    let mut levels: Vec<(u32, u32, Vec<Sums>)> = Vec::new();
    let base: Vec<_> = t
        .words
        .chunks_exact(TILE_WORDS)
        .map(|c| {
            (
                [f64::from(c[0]), f64::from(c[1]), f64::from(c[2])],
                f64::from(c[3]),
                f64::from(c[4]),
            )
        })
        .collect();
    levels.push((t.w.max(1), t.h.max(1), base));
    while {
        let (w, h, _) = &levels[levels.len() - 1];
        *w > 1 || *h > 1
    } {
        let (w, h, v) = &levels[levels.len() - 1];
        let (nw, nh) = (w.div_ceil(2), h.div_ceil(2));
        let mut n = vec![([0f64; 3], 0f64, 0f64); (nw * nh) as usize];
        for y in 0..*h {
            for x in 0..*w {
                let s = &v[(y * w + x) as usize];
                let d = &mut n[((y / 2) * nw + x / 2) as usize];
                for k in 0..3 {
                    d.0[k] += s.0[k];
                }
                d.1 += s.1;
                d.2 += s.2;
            }
        }
        levels.push((nw, nh, n));
    }
    // Pull: the coarsest level's mean (neutral without any border), then each finer level
    // blends its own mean with the coarser estimate by its confidence.
    let mean = |s: &([f64; 3], f64, f64)| -> Option<[f32; 3]> {
        let t = s.0[0] + s.0[1] + s.0[2];
        (s.1 > 0.0 && t > 0.0).then(|| s.0.map(|v| (v / t) as f32))
    };
    let (_, _, top) = &levels[levels.len() - 1];
    let mut est: Vec<[f32; 3]> = vec![mean(&top[0]).unwrap_or(NEUTRAL)];
    let mut est_w = 1u32;
    let mut est_h = 1u32;
    for li in (0..levels.len() - 1).rev() {
        let (w, h, v) = &levels[li];
        let mut out = Vec::with_capacity(v.len());
        for y in 0..*h {
            for x in 0..*w {
                // Bilinear sample of the coarser estimate at this cell's centre.
                let fx = ((x as f32 + 0.5) / 2.0 - 0.5).clamp(0.0, (est_w - 1) as f32);
                let fy = ((y as f32 + 0.5) / 2.0 - 0.5).clamp(0.0, (est_h - 1) as f32);
                let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
                let (x1, y1) = ((x0 + 1).min(est_w - 1), (y0 + 1).min(est_h - 1));
                let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
                let at = |xx: u32, yy: u32| est[(yy * est_w + xx) as usize];
                let c: [f32; 3] = std::array::from_fn(|k| {
                    let top = at(x0, y0)[k] * (1.0 - ax) + at(x1, y0)[k] * ax;
                    let bot = at(x0, y1)[k] * (1.0 - ax) + at(x1, y1)[k] * ax;
                    top * (1.0 - ay) + bot * ay
                });
                let s = &v[(y * w + x) as usize];
                let q = 1.0 - (-(s.2 as f32) / CONFIDENT).exp();
                out.push(match mean(s) {
                    Some(m) => std::array::from_fn(|k| q * m[k] + (1.0 - q) * c[k]),
                    None => c,
                });
            }
        }
        est = out;
        est_w = *w;
        est_h = *h;
    }
    est
}

/// The block statistics of one image (row-major, [`BLOCK_WORDS`] per block).
#[derive(Clone, Debug, PartialEq)]
pub struct BlockStats {
    pub w: u32,
    pub h: u32,
    pub words: Vec<f32>,
}

/// The rebuild data: per block its area (1-based; 0 outside) and relative brightness
/// boost, and per area its chromaticity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rebuild {
    pub w: u32,
    pub h: u32,
    pub area: Vec<u32>,
    pub boost: Vec<f32>,
    pub chroma: Vec<[f32; 3]>,
}

const N8: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Chamfer distance (1, √2) to the nearest block where `seed` holds, in blocks, with the
/// seed's label carried along.
fn chamfer(w: usize, h: usize, seed: &[bool], label: &[u32]) -> (Vec<f32>, Vec<u32>) {
    let mut d: Vec<f32> = seed
        .iter()
        .map(|&s| if s { 0.0 } else { f32::INFINITY })
        .collect();
    let mut l: Vec<u32> = seed
        .iter()
        .zip(label)
        .map(|(&s, &l)| if s { l } else { 0 })
        .collect();
    let s2 = std::f32::consts::SQRT_2;
    let fwd = [(-1i32, -1i32, s2), (0, -1, 1.0), (1, -1, s2), (-1, 0, 1.0)];
    for pass in 0..2 {
        let ys: Vec<usize> = if pass == 0 {
            (0..h).collect()
        } else {
            (0..h).rev().collect()
        };
        for y in ys {
            let xs: Vec<usize> = if pass == 0 {
                (0..w).collect()
            } else {
                (0..w).rev().collect()
            };
            for x in xs {
                let i = y * w + x;
                for &(dx, dy, c) in &fwd {
                    let (dx, dy) = if pass == 0 { (dx, dy) } else { (-dx, -dy) };
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if d[j] + c < d[i] {
                        d[i] = d[j] + c;
                        l[i] = l[j];
                    }
                }
            }
        }
    }
    (d, l)
}

/// Areas of blown blocks, their border colour and brightness domes (see the module
/// documentation).
pub fn rebuild(b: &BlockStats, grid: &[[f32; 3]], tiles_w: u32, tiles_h: u32) -> Rebuild {
    let (w, h) = (b.w as usize, b.h as usize);
    let n = w * h;
    let blk = |i: usize| &b.words[i * BLOCK_WORDS..(i + 1) * BLOCK_WORDS];
    let blown: Vec<bool> = (0..n)
        .map(|i| {
            let c = blk(i);
            c[5] > 0.0 && c[0] >= 0.5 * c[5]
        })
        .collect();
    // Label 8-connected areas.
    let mut area = vec![0u32; n];
    let mut sizes = Vec::new();
    let mut stack = Vec::new();
    for s in 0..n {
        if !blown[s] || area[s] != 0 {
            continue;
        }
        let id = sizes.len() as u32 + 1;
        let mut size = 0usize;
        area[s] = id;
        stack.push(s);
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            for (dx, dy) in N8 {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                if blown[j] && area[j] == 0 {
                    area[j] = id;
                    stack.push(j);
                }
            }
        }
        sizes.push(size);
    }
    // Drop small areas and number the rest from 1.
    let mut renum = vec![0u32; sizes.len() + 1];
    let mut kept = 0u32;
    for (i, &size) in sizes.iter().enumerate() {
        if size >= MIN_SEGMENT {
            kept += 1;
            renum[i + 1] = kept;
        }
    }
    for a in area.iter_mut() {
        *a = renum[*a as usize];
    }
    let inside: Vec<bool> = area.iter().map(|&a| a != 0).collect();
    let outside: Vec<bool> = inside.iter().map(|&v| !v).collect();
    // Depth inside (distance to the nearest block outside) and, outside, the distance to
    // the nearest area and its label.
    let (depth, _) = chamfer(w, h, &outside, &vec![0; n]);
    let (dist_out, near) = chamfer(w, h, &inside, &area);
    let lum = |i: usize| {
        let c = blk(i);
        if c[1] > 0.0 {
            Some((c[2] + c[3] + c[4]) / c[1])
        } else {
            None
        }
    };
    let areas = kept as usize;
    let mut slopes: Vec<Vec<f32>> = vec![Vec::new(); areas];
    let mut sum: Vec<([f64; 3], f64)> = vec![([0.0; 3], 0.0); areas];
    for i in 0..n {
        if inside[i] || near[i] == 0 || dist_out[i] > 2.5 {
            continue;
        }
        let a = near[i] as usize - 1;
        let Some(l1) = lum(i) else { continue };
        if l1 <= 0.0 {
            continue;
        }
        let c = blk(i);
        let wgt = f64::from(l1) * f64::from(l1);
        for k in 0..3 {
            sum[a].0[k] += wgt * f64::from(c[2 + k]) / f64::from(c[1]) / f64::from(l1);
        }
        sum[a].1 += wgt;
        if dist_out[i] > 1.5 {
            continue;
        }
        // Ring 1: compare with the neighbours one block further out.
        let (x, y) = ((i % w) as i32, (i / w) as i32);
        let (mut s2, mut n2) = (0f32, 0f32);
        for (dx, dy) in N8 {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            let j = ny as usize * w + nx as usize;
            if !inside[j]
                && dist_out[j] > dist_out[i] + 0.5
                && let Some(l2) = lum(j)
            {
                s2 += l2;
                n2 += 1.0;
            }
        }
        if n2 > 0.0 {
            slopes[a].push(((l1 - s2 / n2) / l1).clamp(0.0, MAX_SLOPE));
        }
    }
    let chroma: Vec<[f32; 3]> = (0..areas)
        .map(|a| {
            let (s, wt) = sum[a];
            let t = s[0] + s[1] + s[2];
            if wt > 0.0 && t > 0.0 {
                s.map(|v| (v / t) as f32)
            } else {
                // No usable border: the opposed grid at the area's first block.
                let i = area.iter().position(|&v| v == a as u32 + 1).unwrap_or(0);
                let (bx, by) = ((i % w) as u32, (i / w) as u32);
                let tx = (bx * BLOCK_CELLS / TILE_CELLS).min(tiles_w.saturating_sub(1));
                let ty = (by * BLOCK_CELLS / TILE_CELLS).min(tiles_h.saturating_sub(1));
                grid.get((ty * tiles_w + tx) as usize)
                    .copied()
                    .unwrap_or(NEUTRAL)
            }
        })
        .collect();
    let slope: Vec<f32> = slopes
        .into_iter()
        .map(|mut v| {
            if v.is_empty() {
                return 0.0;
            }
            v.sort_by(f32::total_cmp);
            v[v.len() / 2]
        })
        .collect();
    let mut deepest = vec![0f32; areas];
    for i in 0..n {
        if area[i] != 0 {
            let a = area[i] as usize - 1;
            deepest[a] = deepest[a].max(depth[i] - 0.5);
        }
    }
    let boost = (0..n)
        .map(|i| {
            if area[i] == 0 {
                return 0.0;
            }
            let a = area[i] as usize - 1;
            let d = (depth[i] - 0.5).max(0.0);
            let dmax = deepest[a].max(0.5);
            (slope[a] * (d - d * d / (2.0 * dmax))).clamp(0.0, MAX_BOOST)
        })
        .collect();
    Rebuild {
        w: b.w,
        h: b.h,
        area,
        boost,
        chroma,
    }
}
