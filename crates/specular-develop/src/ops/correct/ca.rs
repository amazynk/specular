//! Lateral chromatic aberration: estimating red and blue shifts against green and fitting a
//! radial model.
//!
//! **Per tile.** Each colour sits on its own regular lattice (Bayer: red and blue every
//! second pixel; green as the mean of a cell's two greens at the cell centre; X-Trans and
//! other larger patterns: every colour interpolated at each 2x2 block's centre from its
//! sites around (normalised convolution); other images:
//! every pixel, box-reduced for speed). Per tile, Gauss–Newton (Lucas and Kanade 1981)
//! solves for the shift `d` and a gain and offset with `X(p + d) ≈ a·G(p) + b`, the
//! channel `X` sampled bilinearly on its lattice, so the lattices' own offsets are part of
//! the geometry and come out of `d` exactly. Tiles near clipping and flat tiles are left
//! out; a tile's weight is the gradient energy along its radial direction over its
//! residual.
//!
//! The samples are measured linear (DNG opcode list 1 point ops, LinearizationTable,
//! black and white, list 2 point ops such as MapTable, as the front end prepares them):
//! the per-tile gain and offset absorb only affine differences between the channels.
//!
//! **Radial model.** Lateral CA is a magnification difference, so each tile's radial shift
//! is fitted by weighted least squares to `s(ρ) = R·ρ·(a + b·ρ²)` (`ρ` = distance from the
//! centre over the half diagonal `R`), twice, dropping tiles beyond three robust standard
//! deviations the second time. The correction samples `X` at `p + (p − c)·(a + b·ρ²)`.

use rayon::prelude::*;
use specular_raw::SensorData;
use specular_raw::sensor::Layout;

use crate::ops::raw::linear::Linear;

/// Lattice points per tile side.
const TILE: usize = 48;
/// At most this many tiles are measured (evenly spread).
pub(crate) const MAX_TILES: usize = 900;
/// At most this many tiles on pattern sensors (X-Trans), whose lattice is interpolated
/// only where tiles read it: fewer tiles, less of the sensor read (the fit stays within a
/// few hundredths of a pixel, see the raw tests).
pub(crate) const PATTERN_TILES: usize = 360;
/// Gauss–Newton iterations per tile.
const ITERS: usize = 5;
/// The largest shift measured or applied, in pixels.
pub const MAX_SHIFT: f32 = 6.0;

/// One channel's radial model: the shift at `p` is `(p − c)·(a + b·ρ²)`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Radial {
    pub a: f32,
    pub b: f32,
}

/// The red and blue models of one image, in pixels of the measured grid (a raw's crop).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CaFit {
    pub red: Radial,
    pub blue: Radial,
    /// Centre and half diagonal.
    pub centre: [f32; 2],
    pub radius: f32,
    /// Tiles the fit used (red, blue).
    pub tiles: [usize; 2],
}

impl CaFit {
    /// The shift of `plane` (0 red, 2 blue) at `p`.
    pub fn shift(&self, plane: usize, p: [f32; 2]) -> [f32; 2] {
        let m = if plane == 0 { self.red } else { self.blue };
        let (dx, dy) = (p[0] - self.centre[0], p[1] - self.centre[1]);
        let rho2 = (dx * dx + dy * dy) / (self.radius * self.radius).max(1e-6);
        let k = m.a + m.b * rho2;
        [dx * k, dy * k]
    }
}

/// A colour on a regular lattice: value `v[j·w + i]` sits at
/// `(origin.0 + step·i, origin.1 + step·j)` in image pixels.
struct Lattice {
    w: usize,
    h: usize,
    origin: (f32, f32),
    step: f32,
    v: Vec<f32>,
}

impl Lattice {
    fn at(&self, i: usize, j: usize) -> f32 {
        self.v[j * self.w + i]
    }

    /// Bilinear value and gradient (per image pixel) at image position (x, y); `None`
    /// outside.
    fn sample(&self, x: f32, y: f32) -> Option<(f32, f32, f32)> {
        let u = (x - self.origin.0) / self.step;
        let v = (y - self.origin.1) / self.step;
        if !(u >= 1.0 && v >= 1.0 && u < self.w as f32 - 2.0 && v < self.h as f32 - 2.0) {
            return None;
        }
        let (i, j) = (u.floor() as usize, v.floor() as usize);
        let (fu, fv) = (u - i as f32, v - j as f32);
        let bl = |i: usize, j: usize| {
            let a = self.at(i, j) * (1.0 - fu) + self.at(i + 1, j) * fu;
            let b = self.at(i, j + 1) * (1.0 - fu) + self.at(i + 1, j + 1) * fu;
            a * (1.0 - fv) + b * fv
        };
        let val = bl(i, j);
        // Central differences of the bilinear surface, one lattice step each way.
        let gx = (bl(i + 1, j) - bl(i - 1, j)) / (2.0 * self.step);
        let gy = (bl(i, j + 1) - bl(i, j - 1)) / (2.0 * self.step);
        Some((val, gx, gy))
    }
}

/// Lattices of red, green and blue plus a validity mask on green's lattice.
struct Planes {
    r: Lattice,
    g: Lattice,
    b: Lattice,
    ok: Vec<bool>,
    /// The measured grid's size (image pixels).
    size: (f32, f32),
}

/// One tile's measurement: centre, radial shift and weight.
#[derive(Clone, Copy, Debug)]
struct Meas {
    rho: f32,
    s: f32,
    w: f32,
}

fn solve4(a: &mut [[f64; 5]; 4]) -> Option<[f64; 4]> {
    for c in 0..4 {
        let p = (c..4).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        let pivot = a[c];
        for (r, row) in a.iter_mut().enumerate() {
            if r != c {
                let f = row[c] / pivot[c];
                for (v, pv) in row.iter_mut().zip(pivot.iter()).skip(c) {
                    *v -= f * pv;
                }
            }
        }
    }
    Some(std::array::from_fn(|i| a[i][4] / a[i][i]))
}

/// Measures channel `x` against green over the green lattice points `[i0, i1) × [j0, j1)`:
/// (shift, radial information, residual variance).
fn tile(
    p: &Planes,
    x: &Lattice,
    i0: usize,
    i1: usize,
    j0: usize,
    j1: usize,
    u: [f32; 2],
) -> Option<([f32; 2], f64)> {
    let g = &p.g;
    // Start from the gain and offset of plain regression.
    let (mut sg, mut sx, mut sgg, mut sgx, mut n) = (0f64, 0f64, 0f64, 0f64, 0f64);
    for j in j0..j1 {
        for i in i0..i1 {
            if !p.ok[j * g.w + i] {
                continue;
            }
            let (px, py) = (
                g.origin.0 + g.step * i as f32,
                g.origin.1 + g.step * j as f32,
            );
            let Some((xv, _, _)) = x.sample(px, py) else {
                continue;
            };
            let gv = f64::from(g.at(i, j));
            sg += gv;
            sx += f64::from(xv);
            sgg += gv * gv;
            sgx += gv * f64::from(xv);
            n += 1.0;
        }
    }
    if n < 64.0 {
        return None;
    }
    let var_g = sgg / n - (sg / n).powi(2);
    if var_g <= 1e-12 {
        return None;
    }
    let mut a = (sgx / n - sg / n * sx / n) / var_g;
    let mut b = sx / n - a * sg / n;
    let mut d = [0f64; 2];
    let mut info = 0f64;
    let mut res = 0f64;
    for _ in 0..ITERS {
        let mut m = [[0f64; 5]; 4];
        let (mut hrr, mut rr, mut cnt) = (0f64, 0f64, 0f64);
        for j in j0..j1 {
            for i in i0..i1 {
                if !p.ok[j * g.w + i] {
                    continue;
                }
                let (px, py) = (
                    g.origin.0 + g.step * i as f32,
                    g.origin.1 + g.step * j as f32,
                );
                let Some((xv, gx, gy)) = x.sample(px + d[0] as f32, py + d[1] as f32) else {
                    continue;
                };
                let gv = f64::from(g.at(i, j));
                let r = f64::from(xv) - (a * gv + b);
                let jac = [f64::from(gx), f64::from(gy), -gv, -1.0];
                for (ri, row) in m.iter_mut().enumerate() {
                    for (ci, v) in row.iter_mut().take(4).enumerate() {
                        *v += jac[ri] * jac[ci];
                    }
                    row[4] -= jac[ri] * r;
                }
                let gr = f64::from(gx * u[0] + gy * u[1]);
                hrr += gr * gr;
                rr += r * r;
                cnt += 1.0;
            }
        }
        if cnt < 64.0 {
            return None;
        }
        let step = solve4(&mut m)?;
        d[0] += step[0];
        d[1] += step[1];
        a += step[2];
        b += step[3];
        if !(d[0].abs() <= f64::from(MAX_SHIFT) && d[1].abs() <= f64::from(MAX_SHIFT)) {
            return None;
        }
        info = hrr;
        res = rr / cnt;
    }
    // Weight: radial gradient energy over the residual (noise and model error).
    let w = info / (res + 1e-12);
    (w.is_finite() && info > 0.0).then_some(([d[0] as f32, d[1] as f32], w))
}

/// Fits `s = R·ρ·(a + b·ρ²)` (or `b = 0` with `cubic` false) by weighted least squares.
fn fit(m: &[Meas], radius: f32, cubic: bool) -> Option<Radial> {
    let solve = |m: &[Meas]| -> Option<Radial> {
        let (mut s11, mut s13, mut s33, mut t1, mut t3) = (0f64, 0f64, 0f64, 0f64, 0f64);
        for q in m {
            let (r1, r3) = (
                f64::from(radius * q.rho),
                f64::from(radius * q.rho * q.rho * q.rho),
            );
            let (w, s) = (f64::from(q.w), f64::from(q.s));
            s11 += w * r1 * r1;
            s13 += w * r1 * r3;
            s33 += w * r3 * r3;
            t1 += w * r1 * s;
            t3 += w * r3 * s;
        }
        if s11 <= 0.0 {
            return None;
        }
        let det = s11 * s33 - s13 * s13;
        if cubic && det > 1e-9 * s11 * s33 {
            Some(Radial {
                a: ((t1 * s33 - t3 * s13) / det) as f32,
                b: ((s11 * t3 - s13 * t1) / det) as f32,
            })
        } else {
            Some(Radial {
                a: (t1 / s11) as f32,
                b: 0.0,
            })
        }
    };
    if m.len() < 6 {
        return None;
    }
    let first = solve(m)?;
    let resid = |q: &Meas, f: &Radial| q.s - radius * q.rho * (f.a + f.b * q.rho * q.rho);
    let mut r: Vec<f32> = m.iter().map(|q| resid(q, &first).abs()).collect();
    r.sort_by(f32::total_cmp);
    let mad = r[r.len() / 2].max(0.01);
    let kept: Vec<Meas> = m
        .iter()
        .filter(|q| resid(q, &first).abs() <= 3.0 * 1.4826 * mad)
        .copied()
        .collect();
    if kept.len() < 6 {
        return Some(first);
    }
    solve(&kept)
}

/// The tiles measured on a `gw`×`gh` lattice (tile column, row): at most `max`, evenly
/// spread in raster order.
fn tile_list(gw: usize, gh: usize, tile_size: usize, max: usize) -> Vec<(usize, usize)> {
    let (tw, th) = (gw / tile_size, gh / tile_size);
    let total = tw * th;
    if total == 0 {
        return Vec::new();
    }
    let stride = total.div_ceil(max).max(1);
    (0..total)
        .step_by(stride)
        .map(|t| (t % tw, t / tw))
        .collect()
}

/// Lattice points a tile's measurement reads around it: the largest shift (in lattice
/// steps of 2 px) plus the bilinear sample and its central differences.
const MARGIN: usize = (MAX_SHIFT as usize).div_ceil(2) + 3;

/// Measures every tile (`tile` lattice points a side) and fits both channels.
fn estimate(p: &Planes, cubic: bool, tile_size: usize, max_tiles: usize) -> Option<CaFit> {
    let g = &p.g;
    let (w, h) = p.size;
    // Pixel centres sit at whole coordinates.
    let centre = [(w - 1.0) / 2.0, (h - 1.0) / 2.0];
    let radius = (w * w + h * h).sqrt() / 2.0;
    let tiles = tile_list(g.w, g.h, tile_size, max_tiles);
    if tiles.is_empty() {
        return None;
    }
    let measure = |x: &Lattice| -> Vec<Meas> {
        tiles
            .par_iter()
            .filter_map(|&(tx, ty)| {
                let (i0, j0) = (tx * tile_size, ty * tile_size);
                let cx = g.origin.0 + g.step * (i0 as f32 + tile_size as f32 / 2.0);
                let cy = g.origin.1 + g.step * (j0 as f32 + tile_size as f32 / 2.0);
                let (dx, dy) = (cx - centre[0], cy - centre[1]);
                let r = (dx * dx + dy * dy).sqrt();
                if r < 0.05 * radius {
                    return None;
                }
                let u = [dx / r, dy / r];
                let (d, wgt) = tile(p, x, i0, i0 + tile_size, j0, j0 + tile_size, u)?;
                Some(Meas {
                    rho: r / radius,
                    s: d[0] * u[0] + d[1] * u[1],
                    w: wgt as f32,
                })
            })
            .collect()
    };
    let mr = measure(&p.r);
    let mb = measure(&p.b);
    // Normalise the weights (their scale depends on the image's contrast).
    let norm = |m: Vec<Meas>| -> Vec<Meas> {
        let s: f32 = m.iter().map(|q| q.w).sum::<f32>() / m.len().max(1) as f32;
        m.into_iter()
            .map(|q| Meas {
                w: q.w / s.max(1e-30),
                ..q
            })
            .collect()
    };
    let (mr, mb) = (norm(mr), norm(mb));
    let red = fit(&mr, radius, cubic)?;
    let blue = fit(&mb, radius, cubic)?;
    Some(CaFit {
        red,
        blue,
        centre,
        radius,
        tiles: [mr.len(), mb.len()],
    })
}

/// Estimates a Bayer sensor's red and blue shifts (crop coordinates). `None` for other
/// layouts, small sensors, or pictures without enough structure.
pub fn estimate_bayer(s: &SensorData) -> Option<CaFit> {
    let Layout::Bayer(_) = s.meta.layout else {
        return None;
    };
    if s.cpp != 1 {
        return None;
    }
    let (cw, ch) = (s.crop.w as usize / 2, s.crop.h as usize / 2);
    if cw < 2 * TILE || ch < 2 * TILE {
        return None;
    }
    let m = &s.meta;
    let clip = clip_level(s);
    let lin = Linear::new(s);
    let at = |x: usize, y: usize| -> (u8, f32, bool) {
        let (sx, sy) = (s.crop.x + x as u32, s.crop.y + y as u32);
        let col = m.layout.color(sx, sy);
        let v = lin.value(lin.stored(sx, sy, 0), sx, sy, 0);
        (col, v, v >= clip_fraction(s, clip, sx, sy))
    };
    // Which corner of a cell holds red and blue.
    let mut pos = [(0f32, 0f32); 3];
    for dy in 0..2 {
        for dx in 0..2 {
            let (c, _, _) = at(dx, dy);
            if c != 1 {
                pos[usize::from(c)] = (dx as f32, dy as f32);
            }
        }
    }
    let mut r = vec![0f32; cw * ch];
    let mut g = vec![0f32; cw * ch];
    let mut b = vec![0f32; cw * ch];
    let mut clipped = vec![false; cw * ch];
    for j in 0..ch {
        for i in 0..cw {
            let k = j * cw + i;
            for dy in 0..2 {
                for dx in 0..2 {
                    let (c, v, cl) = at(2 * i + dx, 2 * j + dy);
                    match c {
                        0 => r[k] = v,
                        2 => b[k] = v,
                        _ => g[k] += 0.5 * v,
                    }
                    clipped[k] |= cl;
                }
            }
        }
    }
    // Leave out lattice points next to clipping.
    let ok = near_clipping(&clipped, cw, ch);
    let lat = |v: Vec<f32>, o: (f32, f32)| Lattice {
        w: cw,
        h: ch,
        origin: o,
        step: 2.0,
        v,
    };
    let planes = Planes {
        r: lat(r, pos[0]),
        g: lat(g, (0.5, 0.5)),
        b: lat(b, pos[2]),
        ok,
        size: (s.crop.w as f32, s.crop.h as f32),
    };
    estimate(&planes, true, TILE, MAX_TILES)
}

/// Estimates a CFA sensor's red and blue shifts (crop coordinates): Bayer through
/// [`estimate_bayer`], X-Trans and other larger patterns through [`estimate_pattern`].
pub fn estimate_mosaic(s: &SensorData) -> Option<CaFit> {
    match s.meta.layout {
        Layout::Bayer(_) => estimate_bayer(s),
        Layout::Pattern(_) => estimate_pattern(s),
        _ => None,
    }
}

/// Estimates the shifts of a sensor with a colour pattern larger than 2x2 (X-Trans). Its
/// sites are irregular, so each colour is interpolated at the centre of every 2x2 block by
/// normalised convolution of its sites in the 4x4 pixels around (a separable tent of
/// radius 2, the raw CA correction's kernel): three co-sited lattices of step 2.
pub fn estimate_pattern(s: &SensorData) -> Option<CaFit> {
    estimate_pattern_on(s, true, PATTERN_TILES)
}

/// [`estimate_pattern`] with at most `max_tiles` tiles; `decimate` interpolates only the
/// lattice points the measured tiles read (the same values; tests compare with the whole
/// lattice, and time the whole lattice with [`MAX_TILES`] as before).
pub(crate) fn estimate_pattern_on(
    s: &SensorData,
    decimate: bool,
    max_tiles: usize,
) -> Option<CaFit> {
    let Layout::Pattern(_) = &s.meta.layout else {
        return None;
    };
    if s.cpp != 1 {
        return None;
    }
    let (cw, ch) = (s.crop.w as usize / 2, s.crop.h as usize / 2);
    if cw < 2 * TILE + 2 || ch < 2 * TILE + 2 {
        return None;
    }
    let m = &s.meta;
    let lin = Linear::new(s);
    let clip = clip_level(s);
    // Per lattice row, the column spans the measured tiles read (all when not decimating).
    let mut spans: Vec<Vec<(usize, usize)>> = vec![Vec::new(); ch];
    if decimate {
        for (tx, ty) in tile_list(cw, ch, TILE, max_tiles) {
            let (i0, j0) = (tx * TILE, ty * TILE);
            let (a, b) = (i0.saturating_sub(MARGIN), (i0 + TILE + MARGIN).min(cw));
            for row in spans
                .iter_mut()
                .take((j0 + TILE + MARGIN).min(ch))
                .skip(j0.saturating_sub(MARGIN))
            {
                row.push((a, b));
            }
        }
        for row in &mut spans {
            row.sort_unstable();
            let mut merged: Vec<(usize, usize)> = Vec::with_capacity(row.len());
            for &(a, b) in row.iter() {
                match merged.last_mut() {
                    Some(l) if a <= l.1 => l.1 = l.1.max(b),
                    _ => merged.push((a, b)),
                }
            }
            *row = merged;
        }
    } else {
        spans.iter_mut().for_each(|r| r.push((0, cw)));
    }
    let rows: Vec<([Vec<f32>; 3], Vec<bool>)> = (0..ch)
        .into_par_iter()
        .map(|j| {
            let mut v: [Vec<f32>; 3] = std::array::from_fn(|_| vec![0f32; cw]);
            // Points no tile reads count as clipped (never measured).
            let mut clipped = vec![true; cw];
            let fw = s.crop.w as usize;
            for &(a, b) in &spans[j] {
                // The sensor columns this span reads.
                let (x0, x1) = ((2 * a).saturating_sub(1), (2 * b + 2).min(fw));
                // The four sensor rows under this lattice row, linearised once: colour,
                // value, clipped.
                let src: Vec<Option<Vec<(u8, f32, bool)>>> = (0..4)
                    .map(|dy| {
                        let y = (2 * j + dy) as i64 - 1;
                        (y >= 0 && y < i64::from(s.crop.h)).then(|| {
                            let sy = s.crop.y + y as u32;
                            (x0..x1)
                                .map(|x| {
                                    let sx = s.crop.x + x as u32;
                                    let c = m.layout.color(sx, sy).min(2);
                                    let val = lin.value(lin.stored(sx, sy, 0), sx, sy, 0);
                                    (c, val, val >= clip_fraction(s, clip, sx, sy))
                                })
                                .collect()
                        })
                    })
                    .collect();
                pattern_row(&src, x0, fw, a, b, &mut v, &mut clipped);
            }
            (v, clipped)
        })
        .collect();
    let mut planes: [Vec<f32>; 3] = std::array::from_fn(|_| Vec::with_capacity(cw * ch));
    let mut clipped = Vec::with_capacity(cw * ch);
    for (v, cl) in rows {
        for (p, r) in planes.iter_mut().zip(v) {
            p.extend(r);
        }
        clipped.extend(cl);
    }
    let ok = near_clipping(&clipped, cw, ch);
    let [r, g, b] = planes;
    let lat = |v: Vec<f32>| Lattice {
        w: cw,
        h: ch,
        origin: (0.5, 0.5),
        step: 2.0,
        v,
    };
    let planes = Planes {
        r: lat(r),
        g: lat(g),
        b: lat(b),
        ok,
        size: (s.crop.w as f32, s.crop.h as f32),
    };
    estimate(&planes, true, TILE, max_tiles)
}

/// Four sensor rows under a lattice row (`None` outside the crop): colour, linear value,
/// clipped, per column.
type SensorRows = [Option<Vec<(u8, f32, bool)>>];

/// One lattice row's points `a..b` from the four sensor rows `src` (columns from `x0`;
/// `fw` the crop's width): each colour's tent-weighted mean of its sites in the 4x4 pixels
/// around the block centre, and whether any of them is clipped.
fn pattern_row(
    src: &SensorRows,
    x0: usize,
    fw: usize,
    a: usize,
    b: usize,
    v: &mut [Vec<f32>; 3],
    clipped: &mut [bool],
) {
    const W: [f32; 4] = [0.25, 0.75, 0.75, 0.25];
    for i in a..b {
        let (mut sum, mut n) = ([0f32; 3], [0f32; 3]);
        let mut cl = false;
        for (row, wy) in src.iter().zip(W) {
            let Some(row) = row else { continue };
            for (dx, wx) in W.iter().enumerate() {
                let x = (2 * i + dx) as i64 - 1;
                if x < 0 || x as usize >= fw {
                    continue;
                }
                let (c, val, cc) = row[x as usize - x0];
                let c = usize::from(c);
                cl |= cc;
                sum[c] += wx * wy * val;
                n[c] += wx * wy;
            }
        }
        for c in 0..3 {
            v[c][i] = sum[c] / n[c].max(1e-6);
        }
        clipped[i] = cl;
    }
}

/// The clip level (as the front end's) in the samples' own units.
fn clip_level(s: &SensorData) -> f32 {
    let m = &s.meta;
    if s.float.is_some() && m.clip_white > 0.0 {
        m.clip_white
    } else if m.clip_white > 0.0 {
        m.clip_white.min(m.white)
    } else {
        m.white
    }
}

/// The linear value (black 0, white 1) from which the sample at (sx, sy) counts as clipped:
/// a little under the clip level, which is in linearised units (after any
/// LinearizationTable).
fn clip_fraction(s: &SensorData, clip: f32, sx: u32, sy: u32) -> f32 {
    let b = s.meta.black.at(sx, sy, 0);
    0.95 * (clip - b) / (s.meta.white - b).max(1.0)
}

/// Lattice points away from clipped cells (none of the eight neighbours or itself clipped).
fn near_clipping(clipped: &[bool], cw: usize, ch: usize) -> Vec<bool> {
    (0..cw * ch)
        .into_par_iter()
        .map(|k| {
            let (i, j) = ((k % cw) as i32, (k / cw) as i32);
            (-1..=1).all(|dy| {
                (-1..=1).all(|dx| {
                    let (x, y) = (i + dx, j + dy);
                    x < 0
                        || y < 0
                        || x >= cw as i32
                        || y >= ch as i32
                        || !clipped[(y as usize) * cw + x as usize]
                })
            })
        })
        .collect()
}

/// Estimates red and blue radial scales (fractions: `X(c + (p − c)·(1 + k))` lines up with
/// green) of an RGBA f32 image. `None` when the picture has too little structure.
pub fn estimate_rgb(w: u32, h: u32, rgba: &[f32]) -> Option<[f32; 2]> {
    let (w, h) = (w as usize, h as usize);
    if rgba.len() < w * h * 4 {
        return None;
    }
    // Box-reduce to about 2400 pixels on the long side.
    let f = w.max(h).div_ceil(2400).max(1);
    let (lw, lh) = (w / f, h / f);
    if lw < 2 * TILE || lh < 2 * TILE {
        return None;
    }
    let mut planes: [Vec<f32>; 3] = std::array::from_fn(|_| vec![0f32; lw * lh]);
    let mut ok = vec![true; lw * lh];
    let inv = 1.0 / (f * f) as f32;
    for j in 0..lh {
        for i in 0..lw {
            let mut s = [0f32; 3];
            let mut hi = 0f32;
            for y in j * f..(j + 1) * f {
                for x in i * f..(i + 1) * f {
                    let p = &rgba[(y * w + x) * 4..(y * w + x) * 4 + 3];
                    for c in 0..3 {
                        s[c] += p[c];
                        hi = hi.max(p[c]);
                    }
                }
            }
            let k = j * lw + i;
            for c in 0..3 {
                planes[c][k] = s[c] * inv;
            }
            ok[k] = hi < 0.98;
        }
    }
    let o = (f as f32 - 1.0) / 2.0;
    let [r, g, b] = planes;
    let lat = |v: Vec<f32>| Lattice {
        w: lw,
        h: lh,
        origin: (o, o),
        step: f as f32,
        v,
    };
    let p = Planes {
        r: lat(r),
        g: lat(g),
        b: lat(b),
        ok,
        size: (w as f32, h as f32),
    };
    let fit = estimate(&p, false, TILE, MAX_TILES)?;
    Some([fit.red.a, fit.blue.a])
}
