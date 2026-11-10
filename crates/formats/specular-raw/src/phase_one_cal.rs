//! Phase One sensor calibration, applied to the decoded IIQ L / S samples before the
//! demosaic (the corrections Capture One makes from the data each back writes into its files).
//!
//! * **Black.** A level for the whole sensor (tag 0x21d), corrected per row (0x223: one signed
//!   offset for the columns left of the split column 0x222, one for the right) and per column
//!   (0x225: above and below the split row 0x224). The black is subtracted here, clipping at 0.
//! * **Calibration block** (tag 0x110): a little directory of entries, each applied in file
//!   order to the black-subtracted samples:
//!   - 0x400 defects: bad pixels (type 129) become the mean of the eight nearest photosites
//!     of their colour; bad columns (131, 137) are filled from the neighbouring columns of
//!     their colour (the green of the red rows from its four diagonal neighbours without the
//!     one furthest from their mean, the other photosites from their colour two away: left
//!     and right, and the four diagonals).
//!   - flat fields: a grid of gains (header: origin, extent and node spacing) interpolated
//!     bilinearly between nodes. 0x401 holds floats for every colour, 0x410 and 0x416
//!     16-bit fixed-point gains (1.0 = 32768) for every colour, 0x40b separate red and blue
//!     gains (green is left alone).
//!   - 0x419 / 0x41a: polynomial tone curves (0x419 on the columns right of the split, 0x41a
//!     for the whole sensor, a cubic offset).
//!   - 0x41e: a gain per sensor quadrant (the four read-out channels).
//!   - 0x41f: a linearisation curve per quadrant (16 points against their mean), and 0x431:
//!     the same for four-tile sensors (7 reference levels and a per-mille ratio per
//!     quadrant). Both through a natural cubic spline.
//!
//! Entries this decoder does not know are skipped.

use std::ops::Range;

use rayon::prelude::*;

/// What the calibration of one IIQ file needs, from its headers (no sample data).
#[derive(Clone, Debug, Default)]
pub(crate) struct Calibration {
    /// Black level of the whole sensor.
    pub black: i32,
    /// First column of the right half, first row of the bottom half (0: none).
    pub split_col: u32,
    pub split_row: u32,
    /// Per row, two signed 16-bit offsets (left, right of `split_col`).
    pub row_offsets: Option<Range<usize>>,
    /// Per column, two signed 16-bit offsets (above, below `split_row`).
    pub col_offsets: Option<Range<usize>>,
    /// The calibration block.
    pub block: Option<Range<usize>>,
    /// Colour (0 red, 1 green, 2 blue) per position `(y & 1) * 2 + (x & 1)` of the stored
    /// sensor.
    pub colors: [u8; 4],
}

/// Counts of what [`Calibration::apply`] did (for tests and diagnostics).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Applied {
    pub defect_pixels: usize,
    pub defect_columns: usize,
    pub flat_fields: usize,
    pub curves: usize,
}

struct Sensor<'a> {
    px: &'a mut [u16],
    w: usize,
    h: usize,
    colors: [u8; 4],
}

impl Sensor<'_> {
    /// The green of the red rows: interpolated from its diagonal greens. The other green
    /// (in the blue rows) is filled like red and blue, from its own colour two away, as
    /// Phase One's other readers do.
    fn red_row_green(&self, y: usize, x: usize) -> bool {
        self.colors[(y & 1) * 2 + (x & 1)] == 1 && self.colors[(y & 1) * 2 + (!x & 1)] == 0
    }
    /// The sample at `(y, x)`, 0 outside the sensor.
    fn at(&self, y: isize, x: isize) -> i32 {
        if y < 0 || x < 0 || y as usize >= self.h || x as usize >= self.w {
            0
        } else {
            self.px[y as usize * self.w + x as usize] as i32
        }
    }
}

fn le16(b: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(p..p + 2)?.try_into().ok()?))
}

fn le32(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(p..p + 4)?.try_into().ok()?))
}

fn lef32(b: &[u8], p: usize) -> Option<f32> {
    le32(b, p).map(f32::from_bits).filter(|v| v.is_finite())
}

impl Calibration {
    /// Subtracts the black and applies the calibration block to the `w x h` samples `px`.
    pub(crate) fn apply(
        &self,
        d: &[u8],
        px: &mut [u16],
        w: usize,
        h: usize,
        parallel: bool,
    ) -> Applied {
        let px = &mut px[..w * h];
        self.subtract_black(d, px, w, h, parallel);
        let mut done = Applied::default();
        let Some(block) = self.block.as_ref().and_then(|r| d.get(r.clone())) else {
            return done;
        };
        let mut s = Sensor {
            px,
            w,
            h,
            colors: self.colors,
        };
        for (tag, data) in entries(block) {
            match tag {
                0x400 => defects(&mut s, data, &mut done),
                0x401 => {
                    done.flat_fields += flat_field(&mut s, data, Gains::Float, parallel) as usize
                }
                0x410 | 0x416 => {
                    done.flat_fields += flat_field(&mut s, data, Gains::Fixed, parallel) as usize
                }
                0x40b => {
                    done.flat_fields += flat_field(&mut s, data, Gains::RedBlue, parallel) as usize
                }
                0x419 | 0x41a => {
                    if let Some(lut) = poly_curve(tag, data) {
                        let from = if tag == 0x419 {
                            self.split_col as usize
                        } else {
                            0
                        };
                        map_region(&mut s, &lut, 0..h, from.min(w)..w, parallel);
                        done.curves += 1;
                    }
                }
                0x41e if quadrant_gains(&mut s, data, self.split_col, self.split_row, parallel) => {
                    done.curves += 1;
                }
                0x41f | 0x431
                    if quadrant_curves(
                        &mut s,
                        tag,
                        data,
                        self.split_col,
                        self.split_row,
                        parallel,
                    ) =>
                {
                    done.curves += 1;
                }
                _ => {}
            }
        }
        done
    }

    fn subtract_black(&self, d: &[u8], px: &mut [u16], w: usize, h: usize, parallel: bool) {
        let table = |r: &Option<Range<usize>>, n: usize| -> Option<Vec<[i32; 2]>> {
            let b = d.get(r.clone()?)?;
            (b.len() >= n * 4).then(|| {
                b.chunks_exact(4)
                    .take(n)
                    .map(|c| {
                        [
                            i16::from_le_bytes([c[0], c[1]]) as i32,
                            i16::from_le_bytes([c[2], c[3]]) as i32,
                        ]
                    })
                    .collect()
            })
        };
        let rows = table(&self.row_offsets, h);
        let cols = table(&self.col_offsets, w);
        let (sc, sr) = (self.split_col as usize, self.split_row as usize);
        let black = self.black;
        let work = |(y, line): (usize, &mut [u16])| {
            let rb = rows.as_ref().map_or([0, 0], |t| t[y]);
            let below = (y >= sr) as usize;
            for (x, v) in line.iter_mut().enumerate() {
                let mut b = black - rb[(x >= sc) as usize];
                if let Some(c) = &cols {
                    b -= c[x][below];
                }
                *v = (*v as i32 - b).clamp(0, 65535) as u16;
            }
        };
        if black == 0 && rows.is_none() && cols.is_none() {
            return;
        }
        if parallel {
            px.par_chunks_mut(w).enumerate().for_each(work);
        } else {
            px.chunks_mut(w).enumerate().for_each(work);
        }
    }
}

/// The entries of a calibration block: (tag, data).
fn entries(b: &[u8]) -> Vec<(u32, &[u8])> {
    let mut out = Vec::new();
    if b.get(..2) != Some(b"II") {
        return out;
    }
    let Some(dir) = le32(b, 8).map(|v| v as usize) else {
        return out;
    };
    let n = le32(b, dir).unwrap_or(0).min(4096) as usize;
    for k in 0..n {
        let p = dir + 8 + 12 * k;
        let (Some(tag), Some(len), Some(at)) = (le32(b, p), le32(b, p + 4), le32(b, p + 8)) else {
            break;
        };
        let (at, len) = (at as usize, len as usize);
        if let Some(data) = b.get(at..at.saturating_add(len)) {
            out.push((tag, data));
        }
    }
    out
}

/// Same-colour neighbours of a Bayer photosite: the four diagonal ones and the four two
/// rows or columns away (green), or the four two away and the four diagonal two away (red
/// and blue).
const NEAR_GREEN: [(isize, isize); 8] = [
    (-1, -1),
    (-1, 1),
    (1, -1),
    (1, 1),
    (-2, 0),
    (0, -2),
    (0, 2),
    (2, 0),
];
const NEAR_OTHER: [(isize, isize); 8] = [
    (-2, 0),
    (0, -2),
    (0, 2),
    (2, 0),
    (-2, -2),
    (-2, 2),
    (2, -2),
    (2, 2),
];

fn defects(s: &mut Sensor, data: &[u8], done: &mut Applied) {
    for e in data.chunks_exact(8) {
        let (Some(x), Some(y), Some(kind)) = (le16(e, 0), le16(e, 2), le16(e, 4)) else {
            continue;
        };
        let (x, y) = (x as usize, y as usize);
        if x >= s.w {
            continue;
        }
        match kind {
            129 if y < s.h => {
                let near = if s.red_row_green(y, x) {
                    &NEAR_GREEN
                } else {
                    &NEAR_OTHER
                };
                let sum: i32 = near
                    .iter()
                    .map(|&(dy, dx)| s.at(y as isize + dy, x as isize + dx))
                    .sum();
                s.px[y * s.w + x] = ((sum + 4) >> 3) as u16;
                done.defect_pixels += 1;
            }
            131 | 137 => {
                for y in 0..s.h {
                    let (yi, xi) = (y as isize, x as isize);
                    let v = if s.red_row_green(y, x) {
                        let n: [i32; 4] = std::array::from_fn(|i| {
                            s.at(yi + NEAR_GREEN[i].0, xi + NEAR_GREEN[i].1)
                        });
                        let sum: i32 = n.iter().sum();
                        // Leave out the neighbour furthest from the mean (the first of equals).
                        let mut worst = 0;
                        for i in 1..4 {
                            if ((n[i] << 2) - sum).abs() > ((n[worst] << 2) - sum).abs() {
                                worst = i;
                            }
                        }
                        ((sum - n[worst]) as f32 / 3.0 + 0.5) as i32
                    } else {
                        // Left and right (weight sqrt(2) / 4 each) and the four diagonal
                        // neighbours two away ((2 - sqrt(2)) / 8 each).
                        let diag: i32 = NEAR_OTHER[4..]
                            .iter()
                            .map(|&(dy, dx)| s.at(yi + dy, xi + dx))
                            .sum();
                        let side = s.at(yi, xi - 2) + s.at(yi, xi + 2);
                        (0.5 + diag as f32 * 0.073_223_3 + side as f32 * 0.353_553_4) as i32
                    };
                    s.px[y * s.w + x] = v.clamp(0, 65535) as u16;
                }
                done.defect_columns += 1;
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gains {
    /// One float gain per node, for every colour.
    Float,
    /// One 16-bit gain per node (1.0 = 32768), for every colour.
    Fixed,
    /// Two 16-bit gains per node: red, blue.
    RedBlue,
}

/// Applies one flat field; false when its header is unusable.
fn flat_field(s: &mut Sensor, data: &[u8], kind: Gains, parallel: bool) -> bool {
    let head: Vec<usize> = (0..6)
        .filter_map(|i| le16(data, 2 * i))
        .map(usize::from)
        .collect();
    let [x0, y0, fw, fh, sx, sy] = <[usize; 6]>::try_from(head).unwrap_or([0; 6]);
    if fw == 0 || fh == 0 || sx == 0 || sy == 0 {
        return false;
    }
    let (wide, high) = (fw.div_ceil(sx), fh.div_ceil(sy));
    let planes = if kind == Gains::RedBlue { 2 } else { 1 };
    let size = if kind == Gains::Float { 4 } else { 2 };
    if data.len() < 16 + wide * high * planes * size {
        return false;
    }
    let node = |j: usize, i: usize, k: usize| -> f32 {
        let p = 16 + ((j * wide + i) * planes + k) * size;
        match kind {
            Gains::Float => lef32(data, p).unwrap_or(1.0),
            _ => le16(data, p).unwrap_or(32768) as f32 / 32768.0,
        }
    };
    // Rows and columns covered: the last node row and column only bound the interpolation.
    let rows = y0..(y0 + fh).saturating_sub(sy).min(s.h);
    let cols = x0.min(s.w)..(x0 + fw).saturating_sub(sx).min(s.w);
    if rows.is_empty() || cols.is_empty() {
        return true;
    }
    // Per covered row, the gains interpolated down the node columns (stepped by the per-row
    // increment, accumulated from the first node row).
    let mut col_gain = vec![0f32; planes * wide];
    let mut slope = vec![0f32; planes * wide];
    for i in 0..wide {
        for k in 0..planes {
            col_gain[k * wide + i] = node(0, i, k);
        }
    }
    let mut per_row: Vec<Vec<f32>> = Vec::with_capacity(rows.len());
    'cells: for j in 1..high {
        for i in 0..wide {
            for k in 0..planes {
                slope[k * wide + i] = (node(j, i, k) - col_gain[k * wide + i]) / sy as f32;
            }
        }
        let start = y0 + (j - 1) * sy;
        for y in start..start + sy {
            if y >= rows.end {
                break 'cells;
            }
            per_row.push(col_gain.clone());
            for (g, d) in col_gain.iter_mut().zip(&slope) {
                *g += d;
            }
        }
    }
    let colors = s.colors;
    let w = s.w;
    let work = |(r, line): (usize, &mut [u16])| {
        let Some(g) = per_row.get(r) else {
            return;
        };
        let y = rows.start + r;
        for i in 1..wide {
            let mut gain = [0f32; 2];
            let mut step = [0f32; 2];
            for k in 0..planes {
                gain[k] = g[k * wide + i - 1];
                step[k] = (g[k * wide + i] - gain[k]) / sx as f32;
            }
            let start = x0 + (i - 1) * sx;
            for x in start..(start + sx).min(cols.end) {
                let k = match kind {
                    Gains::RedBlue => match colors[(y & 1) * 2 + (x & 1)] {
                        0 => Some(0),
                        2 => Some(1),
                        _ => None,
                    },
                    _ => Some(0),
                };
                if let Some(k) = k {
                    let v = &mut line[x];
                    *v = (*v as f32 * gain[k]).clamp(0.0, 65535.0) as u16;
                }
                for k in 0..planes {
                    gain[k] += step[k];
                }
            }
        }
    };
    let region = &mut s.px[rows.start * w..rows.end * w];
    if parallel {
        region.par_chunks_mut(w).enumerate().for_each(work);
    } else {
        region.chunks_mut(w).enumerate().for_each(work);
    }
    true
}

/// The 0x419 / 0x41a polynomial as a lookup table. 0x419 has a term for the sensor
/// temperature (coefficient 6, reference 7), left out here as LibRaw does: with the
/// temperature of tag 0x210 the halves of a P 45+ sample differ more at the split column
/// (0.56% against 0.31%).
fn poly_curve(tag: u32, data: &[u8]) -> Option<Vec<u16>> {
    let mut lut = vec![0u16; 65536];
    if tag == 0x419 {
        let p: Vec<f32> = (0..8)
            .map(|i| lef32(data, 4 + 4 * i))
            .collect::<Option<_>>()?;
        let lin = p[3] - p[7] * p[6] + 1.0;
        for (i, o) in lut.iter_mut().enumerate() {
            let x = i as f32;
            *o = ((p[5] * x + lin) * x + p[1]).clamp(0.0, 65535.0) as u16;
        }
    } else {
        let p: Vec<f32> = (0..4).map(|i| lef32(data, 4 * i)).collect::<Option<_>>()?;
        for (i, o) in lut.iter_mut().enumerate() {
            let x = i as f32;
            let v = ((p[3] * x + p[2]) * x + p[1]) * x + p[0];
            *o = (v + x).clamp(0.0, 65535.0) as u16;
        }
    }
    Some(lut)
}

fn map_region(s: &mut Sensor, lut: &[u16], rows: Range<usize>, cols: Range<usize>, parallel: bool) {
    let w = s.w;
    let region = &mut s.px[rows.start * w..rows.end.min(s.h) * w];
    let work = |line: &mut [u16]| {
        for v in &mut line[cols.clone()] {
            *v = lut[*v as usize];
        }
    };
    if parallel {
        region.par_chunks_mut(w).for_each(work);
    } else {
        region.chunks_mut(w).for_each(work);
    }
}

/// The four quadrants as (rows, columns, index `top/bottom * 2 + left/right`).
fn quadrants(s: &Sensor, sc: u32, sr: u32) -> [(Range<usize>, Range<usize>, usize); 4] {
    let (sc, sr) = ((sc as usize).min(s.w), (sr as usize).min(s.h));
    [
        (0..sr, 0..sc, 0),
        (0..sr, sc..s.w, 1),
        (sr..s.h, 0..sc, 2),
        (sr..s.h, sc..s.w, 3),
    ]
}

/// 0x41e: a gain (1 + value) per quadrant, at words 4 (top left), 10 (top right), 14 (bottom
/// left) and 18 (bottom right).
fn quadrant_gains(s: &mut Sensor, data: &[u8], sc: u32, sr: u32, parallel: bool) -> bool {
    let Some(g) = [4, 10, 14, 18]
        .iter()
        .map(|&i| lef32(data, 4 * i).map(|v| 1.0 + v))
        .collect::<Option<Vec<f32>>>()
    else {
        return false;
    };
    for (rows, cols, q) in quadrants(s, sc, sr) {
        let lut: Vec<u16> = (0..65536)
            .map(|v| (g[q] * v as f32).clamp(0.0, 65535.0) as u16)
            .collect();
        map_region(s, &lut, rows, cols, parallel);
    }
    true
}

/// 0x41f and 0x431: a spline per quadrant through its calibration points.
fn quadrant_curves(
    s: &mut Sensor,
    tag: u32,
    data: &[u8],
    sc: u32,
    sr: u32,
    parallel: bool,
) -> bool {
    let word = |i: usize| le32(data, 4 * i);
    let mut knots: [Vec<(f64, f64)>; 4] = Default::default();
    if tag == 0x41f {
        // 16 measured levels per quadrant; the target is their mean over the quadrants.
        let Some(v) = (0..64).map(word).collect::<Option<Vec<u32>>>() else {
            return false;
        };
        let lc = |q: usize, i: usize| v[q * 16 + i] & 0xffff;
        let target: Vec<u32> = (0..16)
            .map(|i| ((0..4).map(|q| lc(q, i)).sum::<u32>() + 2) >> 2)
            .collect();
        for (q, k) in knots.iter_mut().enumerate() {
            if lc(q, 15) == 0 {
                return false;
            }
            k.push((0.0, 0.0));
            for (i, &t) in target.iter().enumerate() {
                k.push((lc(q, i) as f64, t as f64));
            }
            let top = (target[15] as u64 * 65535 / lc(q, 15) as u64) as f64;
            k.push((top, top));
            k.push((65535.0, 65535.0));
        }
    } else {
        // Seven reference levels, then per quadrant the ratio (x 10000) the quadrant reads.
        let Some(v) = (0..35).map(word).collect::<Option<Vec<u32>>>() else {
            return false;
        };
        for (q, k) in knots.iter_mut().enumerate() {
            k.push((0.0, 0.0));
            for i in 0..7 {
                let r = (v[i] & 0xffff) as u64;
                let m = (v[7 + q * 7 + i] & 0xffff) as u64;
                k.push((r as f64, (r * m / 10000) as f64));
            }
            k.push((65535.0, 65535.0));
        }
    }
    // Quadrant order of the tables: top left, top right, bottom left, bottom right.
    for (rows, cols, q) in quadrants(s, sc, sr) {
        let Some(lut) = spline_lut(&knots[q]) else {
            return false;
        };
        map_region(s, &lut, rows, cols, parallel);
    }
    true
}

/// Natural cubic spline through `knots` (in 0..=65535) as a 16-bit lookup table. The last
/// knots may step back (a calibration point past full scale): every input takes the last
/// segment that spans it, inputs no segment spans map to 0.
fn spline_lut(knots: &[(f64, f64)]) -> Option<Vec<u16>> {
    let n = knots.len();
    if n < 3 {
        return None;
    }
    let x: Vec<f64> = knots.iter().map(|k| k.0 / 65535.0).collect();
    let y: Vec<f64> = knots.iter().map(|k| k.1 / 65535.0).collect();
    let h: Vec<f64> = (0..n - 1).map(|i| x[i + 1] - x[i]).collect();
    if h.iter().any(|&v| v == 0.0 || !v.is_finite()) {
        return None;
    }
    // Second derivatives m (m[0] = m[n-1] = 0) from the tridiagonal system.
    let mut m = vec![0f64; n];
    let mut diag = vec![0f64; n];
    let mut rhs = vec![0f64; n];
    for i in 1..n - 1 {
        diag[i] = 2.0 * (h[i - 1] + h[i]);
        rhs[i] = 6.0 * ((y[i + 1] - y[i]) / h[i] - (y[i] - y[i - 1]) / h[i - 1]);
    }
    for i in 2..n - 1 {
        let f = h[i - 1] / diag[i - 1];
        diag[i] -= f * h[i - 1];
        rhs[i] -= f * rhs[i - 1];
    }
    for i in (1..n - 1).rev() {
        m[i] = (rhs[i] - if i + 1 < n - 1 { h[i] * m[i + 1] } else { 0.0 }) / diag[i];
    }
    if m.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let mut lut = vec![0u16; 65536];
    for seg in 0..n - 1 {
        let hh = h[seg];
        if hh <= 0.0 {
            continue;
        }
        let lo = (x[seg] * 65535.0).ceil().max(0.0) as usize;
        let hi = (x[seg + 1] * 65535.0).floor().min(65535.0) as usize;
        for (i, o) in lut.iter_mut().enumerate().take(hi + 1).skip(lo) {
            let a = i as f64 / 65535.0 - x[seg];
            let v = y[seg]
                + ((y[seg + 1] - y[seg]) / hh - (2.0 * m[seg] + m[seg + 1]) * hh / 6.0) * a
                + m[seg] / 2.0 * a * a
                + (m[seg + 1] - m[seg]) / (6.0 * hh) * a * a * a;
            *o = if v < 0.0 {
                0
            } else if v >= 1.0 {
                65535
            } else {
                (v * 65535.0 + 0.5) as u16
            };
        }
    }
    Some(lut)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(v: &[u16]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    /// A calibration block holding `entries` (tag, data).
    fn block(entries: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let mut b = b"II\0\0\0\0\0\0".to_vec();
        b.extend(0u32.to_le_bytes());
        let mut at = Vec::new();
        for (_, data) in entries {
            at.push(b.len() as u32);
            b.extend(data);
        }
        let dir = b.len() as u32;
        b[8..12].copy_from_slice(&dir.to_le_bytes());
        b.extend((entries.len() as u32).to_le_bytes());
        b.extend(0u32.to_le_bytes());
        for ((tag, data), a) in entries.iter().zip(at) {
            for v in [*tag, data.len() as u32, a] {
                b.extend(v.to_le_bytes());
            }
        }
        b
    }

    fn cal(file: &[u8], block_at: usize) -> Calibration {
        Calibration {
            black: 100,
            block: Some(block_at..file.len()),
            colors: [0, 1, 1, 2],
            ..Default::default()
        }
    }

    #[test]
    fn black_tables() {
        let (w, h) = (4usize, 2usize);
        let mut f = le(&[5, -7i16 as u16, 0, 3]); // rows: [5, -7], [0, 3]
        f.extend(le(&[1, 2, 1, 2, 1, 2, 1, 2])); // columns: [1, 2] each
        let c = Calibration {
            black: 100,
            split_col: 2,
            split_row: 1,
            row_offsets: Some(0..8),
            col_offsets: Some(8..24),
            ..Default::default()
        };
        let mut px = vec![200u16; w * h];
        px[0] = 50;
        c.apply(&f, &mut px, w, h, false);
        // Row 0, left: black 100 - 5 - 1 = 94; right: 100 + 7 - 1 = 106. Row 1: 100 - 0 - 2,
        // 100 - 3 - 2.
        assert_eq!(px, [0, 106, 94, 94, 102, 102, 105, 105]);
    }

    #[test]
    fn flat_field_and_defects() {
        let (w, h) = (8usize, 8usize);
        // Gains 1.0 at the left nodes, 1.5 at the right ones, nodes every 4 pixels from 0.
        let mut ff = le(&[0, 0, 8, 8, 4, 4, 0, 0]);
        for _ in 0..2 {
            ff.extend(le(&[32768, 49152]));
        }
        let mut defect = le(&[3, 3, 129, 0]);
        defect.extend(le(&[9, 3, 129, 0])); // outside: skipped
        let b = block(&[(0x410, ff), (0x400, defect)]);
        let c = cal(&b, 0);
        let mut px = vec![1100u16; w * h];
        let done = c.apply(&b, &mut px, w, h, true);
        assert_eq!(done.flat_fields, 1);
        assert_eq!(done.defect_pixels, 1);
        // Black 100 off, then gain 1 + x / 8 (columns 0..4 covered).
        assert_eq!(&px[..5], &[1000, 1125, 1250, 1375, 1000]);
        assert_eq!(px[4 * w + 1], 1000, "rows past the last node row stay");
    }

    #[test]
    fn spline_is_identity_on_a_line() {
        let k: Vec<(f64, f64)> = [0.0, 1000.0, 30000.0, 65535.0]
            .iter()
            .map(|&v| (v, v))
            .collect();
        let lut = spline_lut(&k).unwrap();
        for i in [0usize, 1, 500, 20000, 65535] {
            assert!((lut[i] as i64 - i as i64).abs() <= 1, "{i} -> {}", lut[i]);
        }
        assert!(spline_lut(&[(0.0, 0.0), (0.0, 1.0), (2.0, 2.0)]).is_none());
        // A last knot past full scale still gives a curve up to 65535.
        let k = [
            (0.0, 0.0),
            (30000.0, 31000.0),
            (66000.0, 66000.0),
            (65535.0, 65535.0),
        ];
        let lut = spline_lut(&k).unwrap();
        assert!(
            lut[65535] > 64000 && lut[30000] == 31000,
            "{} {}",
            lut[65535],
            lut[30000]
        );
    }

    #[test]
    fn hostile_blocks() {
        let mut px = vec![0u16; 16];
        for b in [
            vec![],
            b"II".to_vec(),
            block(&[(0x410, le(&[0, 0, 4, 4, 0, 0, 0, 0]))]),
            block(&[(0x410, le(&[0, 0, 65535, 65535, 1, 1, 0, 0]))]),
            block(&[
                (0x41f, vec![0; 260]),
                (0x431, vec![0xff; 144]),
                (0x41e, vec![1; 3]),
            ]),
            block(&[(0x400, le(&[0, 0, 131, 0, 3, 3, 137, 0]))]),
        ] {
            let c = cal(&b, 0);
            c.apply(&b, &mut px, 4, 4, false);
        }
    }
}
