//! Where each spot's work lives on the GPU: the table the kernels read (`dt`), the chunks of
//! the scratch buffer (`S` in the steps, `lc` in the kernel) and the steps.
//!
//! # Chunks
//!
//! A chunk is a grid of `vec4<f32>` cells on a spot's **lattice**: cell `(i, j)` is picture
//! pixel `(ox + i·s, oy + j·s)`. The stride `s` is 1 unless a spot is very large, when its
//! solve runs on a coarser lattice and the kernel interpolates (the harmonic correction is
//! smooth). Per spot:
//!
//! | Chunk | Region | Holds |
//! |---|---|---|
//! | `G0` | the spot's box, one cell of margin | heal/fill: the correction `g` (free cells) or the edge values, and the free flag in `w` |
//! | `E_l`, `R_l` | `G0` halved `l` times | multigrid level `l`: error and free flag; right-hand side |
//! | `LTH`, `LT` | the box (plus the blur reach) | the low layer under the target: rows blurred, then columns |
//! | `LSH`, `LS` | the box moved by the offset | the low layer under the source |
//! | `BH`, `B` | the box | blur spots: the layer blurred along rows, then columns |
//!
//! # Table (`dt`, f32 holding whole numbers exactly)
//!
//! | Words | Meaning |
//! |---|---|
//! | 0–3 | spot count, spot records' start, path points' start, scratch cells |
//! | spot record ([`SW`] words) | see [`Layout::new`] |
//! | job tables | per step kind and level: [`JW`] words per job |
//!
//! A step's threads run over all cells of its jobs; a thread finds its job by binary search
//! of the jobs' first cells. A spot is skipped in a tile whose input does not hold every
//! pixel it reads (its "read box"); the halo makes sure every tile that shows the spot does.
//!
//! # Multigrid
//!
//! Cell-centred: restriction averages the residuals of the four children, prolongation is
//! bilinear, smoothing red–black Gauss–Seidel; a coarse cell is free when all four children are (a
//! coarse edge reaching past the fine one makes the cycle diverge).
//! Each solve runs [`CYCLES`] V-cycles of [`PRE`]/[`POST`] sweeps; the coarsest level (side
//! ≤ [`COARSEST`]) by [`COARSE_SWEEPS`] lexicographic SOR sweeps.
//! While a spot is dragged (the graph's focus) a draft runs [`DRAFT_CYCLES`]; the next render
//! without focus refines it. Spots not dragged keep their refined solves through a draft.
//!
//! A V-cycle runs as few dispatches: levels over [`TAIL_CELLS`] by tiles in workgroup memory
//! (one [`shader::DOWN`] and one [`shader::UP`] per level), the rest of each spot's cycle by
//! one workgroup ([`shader::TAIL`]); the same values as one dispatch per sweep, restriction
//! and prolongation (tested bit for bit).

use super::shader;
use super::{Layer, Placed, PlacedSpot, SpotMode, patch};
use crate::graph::{Kernel, Persist, Program, Roi};

/// Words per spot record.
pub const SW: u32 = 64;
/// Words per job.
pub const JW: u32 = 32;
/// V-cycles per solve.
pub const CYCLES: u32 = 10;
/// V-cycles of a draft (a spot being dragged); the render after the drag refines it.
pub const DRAFT_CYCLES: u32 = 1;
/// Red–black sweeps before and after the coarse correction.
pub const PRE: u32 = 3;
pub const POST: u32 = 3;
/// A level with no side longer than this is solved directly.
pub const COARSEST: u32 = 8;
/// Gauss–Seidel sweeps of the coarsest level.
pub const COARSE_SWEEPS: u32 = 120;
/// Cells of one spot's solve grid before its lattice gets coarser.
pub const SPOT_CELLS: u64 = 1 << 20;
/// Cells all chunks may take together (16 bytes each).
pub const TOTAL_CELLS: u64 = 6 << 20;
/// Threads per row of a step.
const ROW: u32 = 4096;
/// A level of at most this many cells is solved in the spot's one-workgroup tail
/// ([`shader::TAIL`]); larger ones by tiles ([`shader::DOWN`], [`shader::UP`]). The tail's
/// 256 threads take 16 cells each per sweep. Measured (`tail_size_sweep`, 10
/// spots at 1280, 2560 and 5120 px, load 33–38): 1024 to 16384 cells within the noise
/// (±40 %) at every size, 32768 slower at all three; no size-dependent choice showed.
pub const TAIL_CELLS: u32 = 256 * 16;
/// Workgroups per row of a fused step.
const ROW_GROUPS: u32 = 4096;

/// A chunk: offset (in cells), size, lattice origin and stride.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chunk {
    pub off: u64,
    pub w: u32,
    pub h: u32,
    pub ox: i32,
    pub oy: i32,
    pub s: u32,
}

impl Chunk {
    pub fn cells(&self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }

    fn words(&self) -> [f32; 6] {
        [
            self.off as f32,
            self.w as f32,
            self.h as f32,
            self.ox as f32,
            self.oy as f32,
            self.s as f32,
        ]
    }

    /// The chunk grown by `mx`, `my` cells on each side.
    fn grown(&self, mx: u32, my: u32) -> Chunk {
        Chunk {
            off: 0,
            w: self.w + 2 * mx,
            h: self.h + 2 * my,
            ox: self.ox - (mx * self.s) as i32,
            oy: self.oy - (my * self.s) as i32,
            s: self.s,
        }
    }

    fn shifted(&self, d: [i32; 2]) -> Chunk {
        Chunk {
            ox: self.ox + d[0],
            oy: self.oy + d[1],
            ..*self
        }
    }

    /// Pixels the chunk spans: x0, y0, x1, y1 (inclusive).
    fn span(&self) -> [i64; 4] {
        let s = i64::from(self.s);
        [
            i64::from(self.ox),
            i64::from(self.oy),
            i64::from(self.ox) + (i64::from(self.w) - 1).max(0) * s,
            i64::from(self.oy) + (i64::from(self.h) - 1).max(0) * s,
        ]
    }
}

/// One spot's chunks.
#[derive(Clone, Debug, Default)]
pub struct SpotChunks {
    pub g0: Option<Chunk>,
    /// Levels 1.. of the solve: (error, right-hand side).
    pub levels: Vec<(Chunk, Chunk)>,
    pub lth: Option<Chunk>,
    pub lt: Option<Chunk>,
    pub lsh: Option<Chunk>,
    pub ls: Option<Chunk>,
    pub bh: Option<Chunk>,
    pub b: Option<Chunk>,
    /// The first level solved in the tail (levels before it go by tiles).
    pub tail: usize,
    /// Per tile level, the copy of its error the pre-smoothing writes.
    pub copies: Vec<Chunk>,
}

impl SpotChunks {
    /// Level `l`'s error and right-hand side (level 0: `G0`, which has none).
    fn level(&self, l: usize) -> Option<(Chunk, Option<Chunk>)> {
        if l == 0 {
            return self.g0.map(|g| (g, None));
        }
        self.levels.get(l - 1).map(|&(e, r)| (e, Some(r)))
    }
}

/// One job of a step: the spot and up to four chunks; the threads run over `x`'s cells.
#[derive(Clone, Copy, Debug)]
struct Job {
    spot: u32,
    x: Chunk,
    y: Option<Chunk>,
    z: Option<Chunk>,
    w: Option<Chunk>,
    level: u32,
    /// Word 27 of the record.
    extra: u32,
    /// Cells (threads, or workgroups of a fused step) of the job.
    cells: u64,
}

/// A job table in `dt`.
#[derive(Clone, Copy, Debug, Default)]
struct Table {
    at: u32,
    jobs: u32,
    cells: u64,
}

/// The layout of one placed retouch.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    /// The table the kernels read.
    pub dt: Vec<f32>,
    /// Scratch cells (`vec4<f32>` each).
    pub cells: u64,
    /// Cells counted against [`TOTAL_CELLS`] (all but the tile levels' copies, so the
    /// lattices are what they were before the copies).
    budget: u64,
    /// Input pixels needed around each output pixel.
    pub halo: u32,
    pub chunks: Vec<SpotChunks>,
    /// Levels below the finest of the deepest solve.
    pub depth: u32,
    /// Fewer V-cycles ([`DRAFT_CYCLES`]).
    pub draft: bool,
    low_h: Table,
    low_v: Table,
    blur_h: Table,
    blur_v: Table,
    init: Table,
    smooth: Vec<Table>,
    restrict: Vec<Table>,
    prolong: Vec<Table>,
    coarse: Table,
    down: Vec<Table>,
    up: Vec<Table>,
    tail_t: Table,
}

fn gauss_reach(sigma: f32) -> u32 {
    (3.0 * sigma.max(0.3)).ceil() as u32
}

/// The box of pixels a spot may change: x0, y0, x1, y1 inclusive (unclipped).
fn target_box(s: &PlacedSpot) -> [i64; 4] {
    let r = f64::from(s.radius);
    let pts: Vec<[f32; 2]> = if s.path.is_empty() {
        vec![s.centre]
    } else {
        s.path.clone()
    };
    let mut b = [i64::MAX, i64::MAX, i64::MIN, i64::MIN];
    for p in pts {
        let (x, y) = (f64::from(p[0]), f64::from(p[1]));
        b[0] = b[0].min((x - r).floor() as i64 - 1);
        b[1] = b[1].min((y - r).floor() as i64 - 1);
        b[2] = b[2].max((x + r).ceil() as i64 + 1);
        b[3] = b[3].max((y + r).ceil() as i64 + 1);
    }
    b
}

/// The spots that touch the picture, with their boxes (the table's spots, in order).
fn live_spots(p: &Placed) -> Vec<(&PlacedSpot, [i64; 4])> {
    let (iw, ih) = (p.image.0.max(1), p.image.1.max(1));
    let pic = [0, 0, i64::from(iw) - 1, i64::from(ih) - 1];
    p.spots
        .iter()
        .filter(|s| s.opacity > 0.0)
        .map(|s| (s, target_box(s)))
        .filter(|(_, b)| b[2] >= 0 && b[3] >= 0 && b[0] < pic[2] + 1 && b[1] < pic[3] + 1)
        .map(|(s, b)| {
            (
                s,
                [
                    b[0].max(-2),
                    b[1].max(-2),
                    b[2].min(pic[2] + 2),
                    b[3].min(pic[3] + 2),
                ],
            )
        })
        .collect()
}

fn union(a: [i64; 4], b: [i64; 4]) -> [i64; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

fn grow(a: [i64; 4], mx: i64, my: i64) -> [i64; 4] {
    [a[0] - mx, a[1] - my, a[2] + mx, a[3] + my]
}

fn clip(a: [i64; 4], w: u32, h: u32) -> [i64; 4] {
    [
        a[0].clamp(0, i64::from(w) - 1),
        a[1].clamp(0, i64::from(h) - 1),
        a[2].clamp(0, i64::from(w) - 1),
        a[3].clamp(0, i64::from(h) - 1),
    ]
}

impl Layout {
    /// The table with each removal patch's texels appended (record word 48 set to where
    /// they start); a patch gone from the store, or past [`patch::MAX_PATCH_WORDS`], leaves
    /// its spot a plain heal (word 48 = 0).
    pub fn patched(mut self, p: &Placed) -> Vec<f32> {
        let sr = self.dt.get(1).copied().unwrap_or(0.0) as usize;
        let n = self.dt.first().copied().unwrap_or(0.0) as usize;
        let live = live_spots(p);
        let mut used = 0usize;
        for (i, (sp, _)) in live.iter().enumerate().take(n) {
            let w = sr + i * SW as usize + 48;
            if self.dt.get(w).copied() != Some(-1.0) {
                continue;
            }
            self.dt[w] = 0.0;
            let Some(pt) = &sp.patch else { continue };
            let found = patch::store()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&pt.key);
            let Some(texels) = found.filter(|t| t.w == pt.w && t.h == pt.h) else {
                continue;
            };
            if used + texels.rgb.len() > patch::MAX_PATCH_WORDS
                || self.dt.len() + texels.rgb.len() >= 1 << 24
            {
                continue;
            }
            self.dt[w] = self.dt.len() as f32;
            self.dt.extend_from_slice(&texels.rgb);
            used += texels.rgb.len();
        }
        self.dt
    }

    /// Lays out `p`.
    ///
    /// Spot record words: 0–1 centre, 2–3 offset, 4 radius, 5 feather, 6 opacity, 7 blur σ,
    /// 8 mode, 9 layer, 10 path start, 11 path points, 12–15 changed box (x0, y0, x1, y1,
    /// end-exclusive, on the picture), 16–19 read box (the same way), 20 `G0`, 26 `LT`,
    /// 32 `LS`, 38 `B` (six words each: offset, w, h, ox, oy, s; w = 0 when absent),
    /// 44 split σ, 45–47 the solve's signature, 48 a removal patch's texels in the table
    /// (0: none; [`Layout::patched`] appends them), 49–50 its size, 51–56 its map from
    /// picture pixels to texels.
    pub fn new(p: &Placed) -> Layout {
        let live = live_spots(p);
        // Strides: each solve within SPOT_CELLS, then all chunks within TOTAL_CELLS.
        let mut strides: Vec<u32> = live
            .iter()
            .map(|(_, b)| {
                let area = ((b[2] - b[0] + 1) * (b[3] - b[1] + 1)).max(1) as f64;
                (area / SPOT_CELLS as f64).sqrt().ceil().max(1.0) as u32
            })
            .collect();
        let mut out = Layout::default();
        for _ in 0..16 {
            out = Layout::with_strides(p, &live, &strides);
            if out.budget <= TOTAL_CELLS {
                break;
            }
            let k = ((out.budget as f64 / TOTAL_CELLS as f64).sqrt().ceil() as u32).max(2);
            for s in &mut strides {
                *s = s.saturating_mul(k);
            }
        }
        out
    }

    fn with_strides(p: &Placed, live: &[(&PlacedSpot, [i64; 4])], strides: &[u32]) -> Layout {
        let (iw, ih) = (p.image.0.max(1), p.image.1.max(1));
        let ms = |s: u32| gauss_reach(p.split).div_ceil(s);
        let mut cells = 0u64;
        let mut alloc = |c: Chunk| -> Chunk {
            let c = Chunk { off: cells, ..c };
            cells += c.cells();
            c
        };
        let mut chunks = Vec::with_capacity(live.len());
        let mut halo = 0u32;
        let mut reads = Vec::with_capacity(live.len());
        let mut depth = 0u32;
        for (&(sp, b), &s) in live.iter().zip(strides) {
            let base = Chunk {
                off: 0,
                w: ((b[2] - b[0]) as u32).div_ceil(s) + 1,
                h: ((b[3] - b[1]) as u32).div_ceil(s) + 1,
                ox: b[0] as i32,
                oy: b[1] as i32,
                s,
            };
            let mut c = SpotChunks::default();
            let split = sp.layer != Layer::All;
            let mb = gauss_reach(sp.blur).div_ceil(s).max(1);
            let mut read = grow(base.span(), 1, 1);
            if matches!(sp.mode, SpotMode::Heal | SpotMode::Fill) {
                let g0 = alloc(base);
                let (mut w, mut h) = (g0.w, g0.h);
                while w.max(h) > COARSEST {
                    w = w.div_ceil(2);
                    h = h.div_ceil(2);
                    let lvl = Chunk {
                        off: 0,
                        w,
                        h,
                        ox: 0,
                        oy: 0,
                        s: 1,
                    };
                    let e = alloc(lvl);
                    let r = alloc(lvl);
                    c.levels.push((e, r));
                }
                depth = depth.max(c.levels.len() as u32);
                c.g0 = Some(g0);
            }
            if split {
                let mt = if sp.mode == SpotMode::Blur { mb + 1 } else { 1 };
                let lt = base.grown(mt, mt);
                let lth = lt.grown(0, ms(s));
                read = union(read, grow(lth.span(), i64::from(gauss_reach(p.split)), 0));
                c.lth = Some(alloc(lth));
                c.lt = Some(alloc(lt));
                if sp.mode.has_source() {
                    let m = if sp.mode == SpotMode::Fill { 4 } else { 1 };
                    let ls = base.shifted(sp.offset).grown(m, m);
                    let lsh = ls.grown(0, ms(s));
                    read = union(read, grow(lsh.span(), i64::from(gauss_reach(p.split)), 0));
                    c.lsh = Some(alloc(lsh));
                    c.ls = Some(alloc(ls));
                }
            }
            if sp.mode == SpotMode::Blur {
                let bh = base.grown(0, mb);
                read = union(read, grow(bh.span(), i64::from(mb * s), 0));
                c.bh = Some(alloc(bh));
                c.b = Some(alloc(base));
            }
            if sp.mode.has_source() {
                read = union(
                    read,
                    grow(
                        base.shifted(sp.offset).span(),
                        5 + i64::from(s),
                        5 + i64::from(s),
                    ),
                );
            }
            let read = clip(read, iw, ih);
            let tb = clip(b, iw, ih);
            let reach = [
                read[2] - tb[0],
                tb[2] - read[0],
                read[3] - tb[1],
                tb[3] - read[1],
            ]
            .into_iter()
            .max()
            .unwrap_or(0)
            .max(0);
            halo = halo.max(u32::try_from(reach).unwrap_or(u32::MAX));
            reads.push((tb, read));
            chunks.push(c);
        }
        // The tile levels' copies, after everything else.
        let budget = cells;
        let mut tiled = 0u32;
        for c in &mut chunks {
            if c.g0.is_none() {
                continue;
            }
            let mut b = 0;
            while b < c.levels.len()
                && c.level(b)
                    .is_some_and(|(e, _)| e.cells() > u64::from(tail_cells()))
            {
                b += 1;
            }
            c.tail = b;
            for l in 0..b {
                let (e, _) = c.level(l).unwrap_or_default();
                c.copies.push(Chunk { off: cells, ..e });
                cells += e.cells();
            }
            tiled = tiled.max(b as u32);
        }
        // The table.
        let n = live.len();
        let mut dt = vec![0f32; 4];
        dt[0] = n as f32;
        // Whole numbers up to TOTAL_CELLS (< 2^24) are exact in f32.
        dt[3] = cells as f32;
        let sr = dt.len();
        dt[1] = sr as f32;
        dt.resize(sr + n * SW as usize, 0.0);
        dt[2] = dt.len() as f32;
        for (i, ((sp, _), c)) in live.iter().zip(&chunks).enumerate() {
            let at = dt.len();
            dt.extend(sp.path.iter().flatten());
            let (tb, rd) = reads[i];
            let r = &mut dt[sr + i * SW as usize..sr + (i + 1) * SW as usize];
            r[0] = sp.centre[0];
            r[1] = sp.centre[1];
            r[2] = sp.offset[0] as f32;
            r[3] = sp.offset[1] as f32;
            r[4] = sp.radius;
            r[5] = sp.feather;
            r[6] = sp.opacity;
            r[7] = sp.blur;
            r[8] = sp.mode.index() as f32;
            r[9] = sp.layer.index() as f32;
            r[10] = at as f32;
            r[11] = sp.path.len() as f32;
            r[12] = tb[0] as f32;
            r[13] = tb[1] as f32;
            r[14] = (tb[2] + 1) as f32;
            r[15] = (tb[3] + 1) as f32;
            r[16] = rd[0] as f32;
            r[17] = rd[1] as f32;
            r[18] = (rd[2] + 1) as f32;
            r[19] = (rd[3] + 1) as f32;
            for (k, ch) in [c.g0, c.lt, c.ls, c.b].into_iter().enumerate() {
                if let Some(ch) = ch {
                    r[20 + 6 * k..26 + 6 * k].copy_from_slice(&ch.words());
                }
            }
            r[44] = p.split;
            if let Some(pt) = &sp.patch {
                r[48] = -1.0;
                r[49] = pt.w as f32;
                r[50] = pt.h as f32;
                r[51..57].copy_from_slice(&pt.affine);
            }
            // The solve's signature: everything it depends on besides the input (which
            // the engine checks), so a kept scratch is reused only for the same solve.
            // A patch counts by its key (its texels are found by it).
            let mut hsh = blake3::Hasher::new_derive_key("specular-develop retouch solve v1");
            if let Some(pt) = &sp.patch {
                hsh.update(&pt.key);
            }
            for v in r[..45]
                .iter()
                .chain(&r[48..57])
                .chain(&sp.path.iter().flatten().copied().collect::<Vec<_>>())
            {
                hsh.update(&v.to_bits().to_le_bytes());
            }
            let d = hsh.finalize();
            for (k, pair) in d.as_bytes()[..6].chunks_exact(2).enumerate() {
                r[45 + k] = f32::from(u16::from_le_bytes([pair[0], pair[1]]));
            }
        }
        let mut out = Layout {
            cells,
            budget,
            halo,
            depth,
            draft: p.draft,
            ..Layout::default()
        };
        let job = |spot: usize, x: Chunk| Job {
            spot: spot as u32,
            x,
            y: None,
            z: None,
            w: None,
            level: 0,
            extra: 0,
            cells: x.cells(),
        };
        let table = |dt: &mut Vec<f32>, jobs: Vec<Job>| -> Table {
            let at = dt.len() as u32;
            let mut start = 0u64;
            for j in &jobs {
                let mut rec = [0f32; JW as usize];
                rec[0] = start as f32;
                rec[1] = j.spot as f32;
                for (k, ch) in [Some(j.x), j.y, j.z, j.w].into_iter().enumerate() {
                    if let Some(ch) = ch {
                        rec[2 + 6 * k..8 + 6 * k].copy_from_slice(&ch.words());
                    }
                }
                rec[26] = j.level as f32;
                rec[27] = j.extra as f32;
                dt.extend_from_slice(&rec);
                start += j.cells;
            }
            Table {
                at,
                jobs: jobs.len() as u32,
                cells: start,
            }
        };
        let mut low_h = Vec::new();
        let mut low_v = Vec::new();
        let mut blur_h = Vec::new();
        let mut blur_v = Vec::new();
        let mut init = Vec::new();
        let mut coarse = Vec::new();
        for (i, c) in chunks.iter().enumerate() {
            for (h, v) in [(c.lth, c.lt), (c.lsh, c.ls)] {
                if let (Some(h), Some(v)) = (h, v) {
                    low_h.push(job(i, h));
                    low_v.push(Job {
                        y: Some(h),
                        ..job(i, v)
                    });
                }
            }
            if let (Some(bh), Some(b)) = (c.bh, c.b) {
                blur_h.push(Job {
                    y: c.lt,
                    ..job(i, bh)
                });
                blur_v.push(Job {
                    y: Some(bh),
                    ..job(i, b)
                });
            }
            if let Some(g0) = c.g0 {
                init.push(Job {
                    y: c.lt,
                    z: c.ls,
                    ..job(i, g0)
                });
                let (e, r, level) = match c.levels.last() {
                    Some(&(e, r)) => (e, Some(r), c.levels.len() as u32),
                    None => (g0, None, 0),
                };
                coarse.push(Job {
                    y: r,
                    level,
                    cells: 1,
                    ..job(i, e)
                });
            }
        }
        out.low_h = table(&mut dt, low_h);
        out.low_v = table(&mut dt, low_v);
        out.blur_h = table(&mut dt, blur_h);
        out.blur_v = table(&mut dt, blur_v);
        out.init = table(&mut dt, init);
        for l in 0..depth {
            let mut sm = Vec::new();
            let mut re = Vec::new();
            let mut pr = Vec::new();
            for (i, c) in chunks.iter().enumerate() {
                let Some(g0) = c.g0 else { continue };
                if (c.levels.len() as u32) <= l {
                    continue;
                }
                let (fe, fr) = if l == 0 {
                    (g0, None)
                } else {
                    let (e, r) = c.levels[l as usize - 1];
                    (e, Some(r))
                };
                let (ce, cr) = c.levels[l as usize];
                sm.push(Job {
                    y: fr,
                    level: l,
                    ..job(i, fe)
                });
                re.push(Job {
                    y: Some(cr),
                    z: Some(fe),
                    w: fr,
                    level: l,
                    ..job(i, ce)
                });
                pr.push(Job {
                    z: Some(ce),
                    level: l,
                    ..job(i, fe)
                });
            }
            out.smooth.push(table(&mut dt, sm));
            out.restrict.push(table(&mut dt, re));
            out.prolong.push(table(&mut dt, pr));
        }
        out.coarse = table(&mut dt, coarse);
        // The fused steps: tile levels down and up, then each spot's tail.
        let tiles =
            |c: Chunk, side: u32| u64::from(c.w.div_ceil(side)) * u64::from(c.h.div_ceil(side));
        for l in 0..tiled as usize {
            let mut dn = Vec::new();
            let mut up = Vec::new();
            for (i, c) in chunks.iter().enumerate() {
                if l >= c.tail {
                    continue;
                }
                let (Some((fe, fr)), Some((ce, cr))) = (c.level(l), c.level(l + 1)) else {
                    continue;
                };
                let copy = c.copies[l].off as u32;
                dn.push(Job {
                    y: cr,
                    z: Some(fe),
                    w: fr,
                    level: l as u32,
                    extra: copy,
                    cells: tiles(ce, 8),
                    ..job(i, ce)
                });
                up.push(Job {
                    y: fr,
                    z: Some(ce),
                    level: l as u32,
                    extra: copy,
                    cells: tiles(fe, 16),
                    ..job(i, fe)
                });
            }
            out.down.push(table(&mut dt, dn));
            out.up.push(table(&mut dt, up));
        }
        let mut tl = Vec::new();
        for (i, c) in chunks.iter().enumerate() {
            let Some((e, r)) = c.level(c.tail) else {
                continue;
            };
            tl.push(Job {
                y: r,
                z: c.level(c.tail + 1).map(|(e, _)| e),
                level: c.tail as u32,
                extra: (c.levels.len() - c.tail) as u32,
                cells: 1,
                ..job(i, e)
            });
        }
        out.tail_t = table(&mut dt, tl);
        out.dt = dt;
        out.chunks = chunks;
        out
    }

    /// The steps (the same for every tile: each spot checks its read box itself).
    ///
    /// The scratch persists between interactive renders ([`Persist`]): after the cells it
    /// holds per spot the signature its solve was made with, a "solved" flag and its
    /// V-cycles, so a spot whose signature is unchanged keeps its solution (a draft also
    /// keeps a refined one) and only moved or new spots are solved (all of them after the
    /// input changes, when the engine starts a new scratch).
    pub fn program(&self, _input: Roi) -> Program {
        let spots = self.dt.first().map_or(0, |&n| n as u32);
        let cycles = if self.draft { DRAFT_CYCLES } else { CYCLES };
        let sig = |k: &'static Kernel| crate::graph::Step {
            kernel: k,
            threads: [spots, 1],
            words: vec![cycles],
        };
        let mut prog = Program {
            words: self.cells.max(1) * 4 + 8 * u64::from(spots),
            steps: vec![sig(&shader::CHECK)],
            persist: Some(Persist {
                reset: vec![sig(&shader::RESET)],
                reuse: false,
            }),
        };
        let mut run = |k: &'static Kernel, t: &Table, extra: &[u32]| {
            if t.jobs == 0 {
                return;
            }
            let total = t.cells.min(u64::from(u32::MAX)) as u32;
            let tx = total.clamp(1, ROW);
            let ty = total.div_ceil(tx);
            let mut words = vec![t.at, t.jobs, total];
            words.extend_from_slice(extra);
            // The coarse solve: one workgroup row per job (see the kernel).
            let threads = if std::ptr::eq(k, &shader::COARSE) {
                [t.jobs * shader::COARSE_SPACING, 1]
            } else {
                [tx, ty]
            };
            prog.step(k, threads, words);
        };
        run(&shader::LOW_H, &self.low_h, &[]);
        run(&shader::LOW_V, &self.low_v, &[]);
        run(&shader::BLUR_H, &self.blur_h, &[]);
        run(&shader::BLUR_V, &self.blur_v, &[]);
        run(&shader::INIT, &self.init, &[]);
        let depth = self.depth as usize;
        if !stepwise() {
            // One workgroup per tile or tail (see `shader::DOWN`).
            let mut fused = |k: &'static Kernel, t: &Table, extra: &[u32]| {
                if t.jobs == 0 {
                    return;
                }
                let total = t.cells.min(u64::from(u32::MAX / 16)) as u32;
                let gx = total.clamp(1, ROW_GROUPS);
                let gy = total.div_ceil(gx);
                let mut words = vec![t.at, t.jobs, total];
                words.extend_from_slice(extra);
                prog.step(k, [16 * gx, 16 * gy], words);
            };
            for _ in 0..cycles {
                for t in &self.down {
                    fused(&shader::DOWN, t, &[]);
                }
                fused(&shader::TAIL, &self.tail_t, &[COARSE_SWEEPS]);
                for t in self.up.iter().rev() {
                    fused(&shader::UP, t, &[]);
                }
            }
            prog.step(&shader::SIGN, [spots, 1], vec![cycles]);
            return prog;
        }
        for _ in 0..cycles {
            for l in 0..depth {
                for _ in 0..PRE {
                    run(&shader::SMOOTH, &self.smooth[l], &[0]);
                    run(&shader::SMOOTH, &self.smooth[l], &[1]);
                }
                run(&shader::RESTRICT, &self.restrict[l], &[]);
            }
            run(&shader::COARSE, &self.coarse, &[COARSE_SWEEPS]);
            for l in (0..depth).rev() {
                run(&shader::PROLONG, &self.prolong[l], &[]);
                for _ in 0..POST {
                    run(&shader::SMOOTH, &self.smooth[l], &[0]);
                    run(&shader::SMOOTH, &self.smooth[l], &[1]);
                }
            }
        }
        prog.step(&shader::SIGN, [spots, 1], vec![cycles]);
        prog
    }
}

/// Tests compare the fused steps with one dispatch per sweep, restriction and
/// prolongation (the solve as first written).
#[cfg(test)]
pub(crate) static STEPWISE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Tests and timing runs try other tail sizes (0: [`TAIL_CELLS`]).
#[cfg(test)]
pub(crate) static TAIL_OVERRIDE: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);

fn tail_cells() -> u32 {
    #[cfg(test)]
    {
        let o = TAIL_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed);
        if o > 0 {
            return o;
        }
    }
    TAIL_CELLS
}

fn stepwise() -> bool {
    #[cfg(test)]
    return STEPWISE.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(test))]
    false
}
