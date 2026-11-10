//! Sigma X3F (Foveon) sensor codecs. Every pixel holds three samples, one per stacked layer
//! (top "blue", middle "green", bottom "red" as Sigma names them, stored R, G, B).
//!
//! * TRUE (DP1/DP2, SD14/SD15, Merrill and Quattro): each layer is a separate bit stream (MSB
//!   first, no stuffing) starting on a 16-byte boundary. A prefix code of at most 8 bits gives
//!   the bit length of a difference that follows (top bit clear: negative). Within a row each
//!   sample predicts from the previous sample of the same column parity; the first two samples
//!   of a row predict from the first two of the previous row of the same parity, seeded per
//!   layer. Quattro bodies store the lower two layers at half resolution in each direction
//!   (they are replicated over 2x2 pixels here) and correct the phase-detect AF pixels, whose
//!   lower layers read darker, from the nearest top-layer neighbour.
//! * Huffman 10-bit (SD9/SD10, SD14 at some settings, Polaroid X530): one code per sample
//!   from a table of 1024 left-aligned codes, mapped through a 1024-entry difference table and
//!   accumulated per layer along the row; rows start at their own offsets (a footer table), so
//!   they decode independently. Negative sums are lifted by the most negative value seen.
//!   With a row stride instead of a table, each pixel is one 32-bit word of three 10-bit
//!   indices into the difference table.

use rayon::prelude::*;

use crate::bits::BitReader;

fn u16le(b: &[u8], p: usize) -> u16 {
    b.get(p..p + 2)
        .map_or(0, |s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32le(b: &[u8], p: usize) -> u32 {
    b.get(p..p + 4)
        .map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Prefix code of the TRUE streams: `(code length, number of difference bits)` per 8-bit
/// peek; length 0 marks an unused code.
pub(crate) struct TrueTable {
    lut: [(u8, u8); 256],
}

impl TrueTable {
    /// Reads `(length, left-aligned code)` byte pairs up to a zero length; returns the table
    /// and the bytes consumed.
    pub(crate) fn read(b: &[u8]) -> (TrueTable, usize) {
        let mut lut = [(0u8, 0u8); 256];
        let mut p = 0;
        let mut index = 0usize;
        while let Some(&[len, code]) = b.get(p..p + 2).map(|s| [s[0], s[1]]).as_ref() {
            p += 2;
            if len == 0 {
                break;
            }
            if len <= 8 && index < 256 {
                let span = 1usize << (8 - len);
                let first = (code as usize >> (8 - len)) << (8 - len);
                for e in &mut lut[first..first + span] {
                    *e = (len, index.min(31) as u8);
                }
            }
            index += 1;
        }
        (TrueTable { lut }, p)
    }

    /// The next difference (needs a refill covering 40 bits).
    #[inline(always)]
    fn diff(&self, br: &mut BitReader) -> i32 {
        let (len, n) = self.lut[br.peek(8) as usize];
        if len == 0 {
            br.consume(1);
            return 0;
        }
        br.consume(len as u32);
        if n == 0 {
            return 0;
        }
        let n = n as u32;
        let v = br.peek(n) as i32;
        br.consume(n);
        if v >> (n - 1) == 0 {
            v - ((1i32 << n) - 1)
        } else {
            v
        }
    }
}

/// Decodes one TRUE layer of `cols x rows` samples into `out` (`keep` samples per row; extra
/// columns are dropped). Returns true if the stream ran out.
pub(crate) fn true_plane(
    src: &[u8],
    table: &TrueTable,
    seed: u16,
    cols: usize,
    rows: usize,
    keep: usize,
    out: &mut [u16],
) -> bool {
    let mut br = BitReader::new(src, false);
    let mut start = [[seed as i32; 2]; 2];
    let keep = keep.min(cols);
    for (row, dst) in out.chunks_exact_mut(keep.max(1)).take(rows).enumerate() {
        let mut acc = [0i32; 2];
        let parity = &mut start[row & 1];
        for col in 0..cols {
            br.refill();
            let diff = table.diff(&mut br);
            let odd = col & 1;
            let v = if col < 2 { parity[odd] } else { acc[odd] }.wrapping_add(diff);
            acc[odd] = v;
            if col < 2 {
                parity[odd] = v;
            }
            if let Some(d) = dst.get_mut(col) {
                *d = v as u16;
            }
        }
        if br.overrun() {
            return true;
        }
    }
    br.overrun()
}

/// `size` bytes of a 1-D predicted stream (CAMF type 5): each byte is the low byte of the
/// running sum of `bias` and the differences.
pub(crate) fn camf_bytes(src: &[u8], table: &TrueTable, bias: i32, size: usize) -> Vec<u8> {
    let mut br = BitReader::new(src, false);
    let mut acc = bias;
    let mut out = Vec::with_capacity(size);
    for _ in 0..size {
        br.refill();
        acc = acc.wrapping_add(table.diff(&mut br));
        out.push(acc as u8);
        if br.overrun() {
            out.resize(size, 0);
            break;
        }
    }
    out
}

/// TRUE image header: plane sizes (Quattro), seeds, prefix code and the three plane ranges.
struct TrueHeader {
    /// Columns and rows of each plane.
    dims: [(usize, usize); 3],
    seeds: [u16; 3],
    table: TrueTable,
    planes: [std::ops::Range<usize>; 3],
}

fn true_header(src: &[u8], quattro: bool, w: usize, h: usize) -> Option<TrueHeader> {
    let mut p = 0;
    let mut dims = [(w, h); 3];
    if quattro {
        for d in &mut dims {
            *d = (u16le(src, p) as usize, u16le(src, p + 2) as usize);
            p += 4;
        }
    }
    let seeds = [u16le(src, p), u16le(src, p + 2), u16le(src, p + 4)];
    p += 8;
    let (table, n) = TrueTable::read(src.get(p..)?);
    p += n;
    if quattro {
        p += 4;
    }
    let sizes = [u32le(src, p), u32le(src, p + 4), u32le(src, p + 8)];
    p += 12;
    let mut planes: [std::ops::Range<usize>; 3] = Default::default();
    for (r, s) in planes.iter_mut().zip(sizes) {
        let end = p.saturating_add(s as usize);
        *r = p.min(src.len())..end.min(src.len());
        // Planes start on 16-byte boundaries from the first plane.
        p = p.saturating_add((s as usize).div_ceil(16).saturating_mul(16));
    }
    Some(TrueHeader {
        dims,
        seeds,
        table,
        planes,
    })
}

/// Black level of the Quattro AF-pixel correction (the sensor pedestal).
const QUATTRO_PEDESTAL: i32 = 2047;

/// TRUE / Merrill / Quattro: decodes `w x h x 3` samples into `out`; `af` is the area the
/// Quattro AF-pixel grid covers (`x0, y0, x1, y1`, exclusive ends), if any. Returns true if
/// data ran out.
pub(crate) fn decode_true(
    src: &[u8],
    quattro: bool,
    af: Option<AfGrid>,
    w: usize,
    h: usize,
    out: &mut [u16],
    parallel: bool,
) -> bool {
    let Some(hd) = true_header(src, quattro, w, h) else {
        return true;
    };
    // Quattro with the lower layers at half resolution; otherwise every plane is w x h.
    let half = quattro && hd.dims[0].1 == h / 2;
    let keep: [usize; 3] = std::array::from_fn(|c| {
        if half && c < 2 {
            hd.dims[c].0.min(w.div_ceil(2))
        } else {
            hd.dims[c].0.min(w)
        }
    });
    let rows: [usize; 3] = std::array::from_fn(|c| {
        if half && c < 2 {
            hd.dims[c].1.min(h.div_ceil(2))
        } else {
            hd.dims[c].1.min(h)
        }
    });
    let mut planes: Vec<Vec<u16>> = (0..3).map(|c| vec![0u16; keep[c] * rows[c]]).collect();
    let job = |(c, p): (usize, &mut Vec<u16>)| {
        true_plane(
            &src[hd.planes[c].clone()],
            &hd.table,
            hd.seeds[c],
            hd.dims[c].0,
            rows[c],
            keep[c],
            p,
        ) || hd.planes[c].is_empty()
    };
    let truncated = if parallel {
        planes
            .par_iter_mut()
            .enumerate()
            .map(job)
            .reduce(|| false, |a, b| a | b)
    } else {
        planes
            .iter_mut()
            .enumerate()
            .map(job)
            .fold(false, |a, b| a | b)
    };
    if half && let Some(g) = af {
        if g.sd {
            af_correct_sd(&mut planes, keep, rows, &g);
        } else {
            af_correct(&mut planes, keep, &g, w, h);
        }
    }
    let (p0, rest) = planes.split_at(1);
    let (p1, p2) = rest.split_at(1);
    let (p0, p1, p2) = (&p0[0], &p1[0], &p2[0]);
    let fill = |(y, row): (usize, &mut [u16])| {
        let (yl, sl) = if half { (y / 2, 1) } else { (y, 0) };
        for (x, px) in row.chunks_exact_mut(3).enumerate() {
            let xl = x >> sl;
            px[0] = if yl < rows[0] && xl < keep[0] {
                p0[yl * keep[0] + xl]
            } else {
                0
            };
            px[1] = if yl < rows[1] && xl < keep[1] {
                p1[yl * keep[1] + xl]
            } else {
                0
            };
            px[2] = if y < rows[2] && x < keep[2] {
                p2[y * keep[2] + x]
            } else {
                0
            };
        }
    };
    let out = &mut out[..w * h * 3];
    if parallel {
        out.par_chunks_exact_mut(w * 3).enumerate().for_each(fill);
    } else {
        out.chunks_exact_mut(w * 3).enumerate().for_each(fill);
    }
    truncated
}

/// The phase-detect AF pixels of a Quattro sensor: every `step_x` columns and `step_y` rows
/// (full-resolution coordinates, multiples of the steps) inside `x0..x1, y0..y1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AfGrid {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
    pub step_x: u32,
    pub step_y: u32,
    /// The sd Quattro layout: the grid points are 2x2 blocks at `(x, y)` with `x % 16 == 8`
    /// and `y % 32 == 16`, whose top-layer pixels at column `x + 1` and whose lower-layer
    /// pixel are AF pixels (`step_x`, `step_y` unused).
    pub sd: bool,
}

/// Restores the lower layers of the AF pixels (they collect about a quarter of the light):
/// scaled by the top-layer ratio to the most similar neighbour two pixels away, averaged with
/// the pixel's own value times 3.75; times four where the top layer is too dark to compare.
fn af_correct(planes: &mut [Vec<u16>], keep: [usize; 3], g: &AfGrid, w: usize, h: usize) {
    let bl = QUATTRO_PEDESTAL;
    let top = |p: &[u16], x: usize, y: usize| -> i32 {
        p.get(y * keep[2] + x).copied().unwrap_or(0) as i32
    };
    let (sx, sy) = (g.step_x.max(2) as usize, g.step_y.max(2) as usize);
    let y_end = (g.y1 as usize).min(h.saturating_sub(2));
    let x_end = (g.x1 as usize).min(w.saturating_sub(2));
    let mut y = (g.y0 as usize).max(2).div_ceil(sy) * sy;
    while y <= y_end && y + 2 < h {
        let mut x = (g.x0 as usize).max(2).div_ceil(sx) * sx;
        while x <= x_end && x + 2 < w {
            let b = &planes[2];
            let here = top(b, x, y);
            // Neighbours in order: above, below, left, right; the first closest wins.
            let cands = [(x, y - 2), (x, y + 2), (x - 2, y), (x + 2, y)];
            let mut best = cands[0];
            for &c in &cands[1..] {
                if (top(b, c.0, c.1) - here).abs() < (top(b, best.0, best.1) - here).abs() {
                    best = c;
                }
            }
            let near = top(b, best.0, best.1);
            let (hx, hy) = (x / 2, y / 2);
            let (nx, ny) = (best.0 / 2, best.1 / 2);
            for (c, plane) in planes.iter_mut().take(2).enumerate() {
                let k = keep[c];
                let Some(&own) = plane.get(hy * k + hx) else {
                    continue;
                };
                let own = (own as i32).max(bl);
                let v = if here < bl + 16 || near < bl + 16 {
                    ((own - bl) * 4 + bl).min(16383)
                } else {
                    let m = (near - bl) as f32 / (here - bl) as f32;
                    let nb = (plane.get(ny * k + nx).copied().unwrap_or(0) as f32).max(bl as f32);
                    let a = (nb - bl as f32) * m + bl as f32;
                    let b2 = (own - bl) as f64 * 3.75 + bl as f64;
                    ((a as f64 + b2) / 2.0).min(16383.0) as i32
                };
                plane[hy * k + hx] = v.clamp(0, 65535) as u16;
            }
            x += sx;
        }
        y += sy;
    }
}

/// A grid of gains over the visible area, one per layer (vignetting / lens shading from the
/// file's calibration).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpatialGain {
    /// Per layer: rows, columns and row-major gains (nodes on the area's edges).
    pub grids: [(usize, usize, Vec<f32>); 3],
    /// The visible area the grids span, and the masked area that gives the black level.
    pub area: crate::Rect,
    pub dark: Option<crate::Rect>,
}

impl SpatialGain {
    fn gain(&self, c: usize, fy: f32, fx: f32) -> f32 {
        let (rows, cols, g) = &self.grids[c];
        let (y, x) = (fy * (rows - 1) as f32, fx * (cols - 1) as f32);
        let (y0, x0) = ((y as usize).min(rows - 1), (x as usize).min(cols - 1));
        let (y1, x1) = ((y0 + 1).min(rows - 1), (x0 + 1).min(cols - 1));
        let (ty, tx) = (y - y0 as f32, x - x0 as f32);
        let at = |r: usize, c: usize| g[r * cols + c];
        let top = at(y0, x0) * (1.0 - tx) + at(y0, x1) * tx;
        let bottom = at(y1, x0) * (1.0 - tx) + at(y1, x1) * tx;
        top * (1.0 - ty) + bottom * ty
    }

    /// Multiplies the black-subtracted samples of the area of `out` (`w x h x 3`) by the
    /// interpolated gains.
    pub(crate) fn apply(&self, out: &mut [u16], w: usize, h: usize, parallel: bool) {
        if self
            .grids
            .iter()
            .any(|(r, c, g)| *r < 2 || *c < 2 || g.len() < r * c)
        {
            return;
        }
        let a = self.area;
        let (x0, y0) = (a.x as usize, a.y as usize);
        let (x1, y1) = ((x0 + a.w as usize).min(w), (y0 + a.h as usize).min(h));
        if x1 <= x0 + 1 || y1 <= y0 + 1 {
            return;
        }
        let mut black = [0f32; 3];
        if let Some(d) = self.dark {
            let mut n = 0f64;
            let mut sum = [0f64; 3];
            for y in (d.y as usize..(d.y + d.h) as usize).take(h) {
                for x in (d.x as usize..(d.x + d.w) as usize).take(w) {
                    for c in 0..3 {
                        sum[c] += out[(y * w + x) * 3 + c] as f64;
                    }
                    n += 1.0;
                }
            }
            if n > 0.0 {
                black = sum.map(|s| (s / n) as f32);
            }
        }
        let (sw, sh) = ((x1 - x0 - 1) as f32, (y1 - y0 - 1) as f32);
        let row = |(y, line): (usize, &mut [u16])| {
            if y < y0 || y >= y1 {
                return;
            }
            let fy = (y - y0) as f32 / sh;
            for x in x0..x1 {
                let fx = (x - x0) as f32 / sw;
                for c in 0..3 {
                    let v = &mut line[x * 3 + c];
                    let d = *v as f32 - black[c];
                    if d > 0.0 {
                        *v = (black[c] + d * self.gain(c, fy, fx)).min(65535.0) as u16;
                    }
                }
            }
        };
        let out = &mut out[..w * h * 3];
        if parallel {
            out.par_chunks_mut(w * 3).enumerate().for_each(row);
        } else {
            out.chunks_mut(w * 3).enumerate().for_each(row);
        }
    }
}

/// sd Quattro AF pixels: each block's two top-layer AF pixels (one column, two rows) become
/// the mean of their left and right neighbours, its lower-layer pixel the mean of the
/// lower-layer pixels above and below (rows of AF blocks are far apart, columns close).
fn af_correct_sd(planes: &mut [Vec<u16>], keep: [usize; 3], rows: [usize; 3], g: &AfGrid) {
    let (x_end, y_end) = (g.x1 as usize, g.y1 as usize);
    let mut y = (g.y0 as usize).div_ceil(32) * 32 + 16;
    if y >= 32 + g.y0 as usize {
        y -= 32;
    }
    while y + 1 < y_end {
        let mut x = (g.x0 as usize).saturating_sub(8).div_ceil(16) * 16 + 8;
        while x + 2 < x_end {
            let top = &mut planes[2];
            let k = keep[2];
            for yy in [y, y + 1] {
                if yy < rows[2] && x + 2 < k {
                    let (l, r) = (top[yy * k + x] as u32, top[yy * k + x + 2] as u32);
                    top[yy * k + x + 1] = (l + r).div_ceil(2) as u16;
                }
            }
            let (hx, hy) = (x / 2, y / 2);
            for (c, plane) in planes.iter_mut().take(2).enumerate() {
                let k = keep[c];
                if hy >= 1 && hy + 1 < rows[c] && hx < k {
                    let (a, b) = (
                        plane[(hy - 1) * k + hx] as u32,
                        plane[(hy + 1) * k + hx] as u32,
                    );
                    plane[hy * k + hx] = (a + b).div_ceil(2) as u16;
                }
            }
            x += 16;
        }
        y += 32;
    }
}

/// Huffman / mapped 10-bit images (SD9, SD10, SD14, X530): `src` starts at the image data
/// (after the 28-byte section header). `stride` 0 means Huffman coded with a row-offset
/// footer. Returns whether data ran out and the offset added to every sample.
pub(crate) fn decode_huffman10(
    src: &[u8],
    stride: usize,
    w: usize,
    h: usize,
    out: &mut [u16],
    parallel: bool,
) -> (bool, i32) {
    let map: Vec<u16> = (0..1024).map(|i| u16le(src, 2 * i)).collect();
    let out = &mut out[..w * h * 3];
    if stride != 0 {
        let data = src.get(2048..).unwrap_or(&[]);
        let short = AtomicFlag::default();
        let row = |(y, dst): (usize, &mut [u16])| {
            let mut c = [0u16; 3];
            let base = y.saturating_mul(stride);
            for (x, px) in dst.chunks_exact_mut(3).enumerate() {
                let p = base + 4 * x;
                if p + 4 > data.len() {
                    short.set();
                }
                let v = u32le(data, p);
                for (k, s) in px.iter_mut().enumerate() {
                    c[k] = c[k].wrapping_add(map[((v >> (10 * k)) & 0x3ff) as usize]);
                    *s = if (c[k] as i16) > 0 { c[k] } else { 0 };
                }
            }
        };
        if parallel {
            out.par_chunks_exact_mut(w * 3).enumerate().for_each(row);
        } else {
            out.chunks_exact_mut(w * 3).enumerate().for_each(row);
        }
        return (short.get(), 0);
    }
    let codes: Vec<u32> = (0..1024).map(|i| u32le(src, 2048 + 4 * i)).collect();
    let tree = Huff10::new(&codes, &map);
    let start = 2048 + 4096;
    let footer = src.len().saturating_sub(h * 4).max(start);
    let data = src.get(start..footer).unwrap_or(&[]);
    let offsets: Vec<usize> = (0..h)
        .map(|y| u32le(src, footer + 4 * y) as usize)
        .collect();
    let short = AtomicFlag::default();
    let pass = |offset: i16, out: &mut [u16]| -> i32 {
        let row = |(y, dst): (usize, &mut [u16])| -> i32 {
            let mut c = [offset; 3];
            let mut min = 0i32;
            let Some(s) = data.get(offsets[y]..) else {
                short.set();
                dst.fill(0);
                return 0;
            };
            let mut br = BitReader::new(s, false);
            for px in dst.chunks_exact_mut(3) {
                br.refill();
                for (k, v) in px.iter_mut().enumerate() {
                    if br.nbits() < 32 {
                        br.refill();
                    }
                    c[k] = c[k].wrapping_add(tree.next(&mut br) as i16);
                    if c[k] < 0 {
                        min = min.min(c[k] as i32);
                        *v = 0;
                    } else {
                        *v = c[k] as u16;
                    }
                }
            }
            if br.overrun() {
                short.set();
            }
            min
        };
        if parallel {
            out.par_chunks_exact_mut(w * 3)
                .enumerate()
                .map(row)
                .reduce(|| 0, i32::min)
        } else {
            out.chunks_exact_mut(w * 3)
                .enumerate()
                .map(row)
                .fold(0, i32::min)
        }
    };
    let min = pass(0, out);
    let mut offset = 0;
    if min < 0 {
        offset = (-min).min(i16::MAX as i32);
        pass(offset as i16, out);
    }
    (short.get(), offset)
}

#[derive(Default)]
struct AtomicFlag(std::sync::atomic::AtomicBool);

impl AtomicFlag {
    fn set(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    fn get(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Huffman-coded 8-bit RGB thumbnails (format 11, SD9/SD10): 256 codes (length in the top 5
/// bits, right-aligned code below), the rows, then one 32-bit offset per row (from the start
/// of the rows). Each value is the difference from the pixel to the left (0 at the row
/// start), modulo 256; the code's index is the value.
pub(crate) fn decode_coded_thumb(src: &[u8], w: usize, h: usize) -> Option<Vec<u8>> {
    let codes: Vec<u32> = (0..256).map(|i| u32le(src, 4 * i)).collect();
    let map: Vec<u16> = (0..256).collect();
    let tree = Huff10::new(&codes, &map);
    let footer = src.len().checked_sub(h * 4)?;
    let data = src.get(1024..footer)?;
    let mut out = vec![0u8; w.checked_mul(h)?.checked_mul(3)?];
    for (y, row) in out.chunks_exact_mut(w * 3).enumerate() {
        let off = u32le(src, footer + 4 * y) as usize;
        let mut br = BitReader::new(data.get(off..).unwrap_or(&[]), false);
        let mut pred = [0u8; 3];
        for px in row.chunks_exact_mut(3) {
            for (k, v) in px.iter_mut().enumerate() {
                if br.nbits() < 32 {
                    br.refill();
                }
                pred[k] = pred[k].wrapping_add(tree.next(&mut br) as u8);
                *v = pred[k];
            }
        }
    }
    Some(out)
}

/// Codes of up to [`LUT_BITS`] bits through a table, longer ones by search.
struct Huff10 {
    /// `(length, value)`; length 0: no short code.
    lut: Vec<(u8, u16)>,
    long: Vec<(u32, u32, u16)>,
}

const LUT_BITS: u32 = 14;

impl Huff10 {
    /// `codes[i]`: length in the top 5 bits, right-aligned code in the low 27; value `map[i]`.
    fn new(codes: &[u32], map: &[u16]) -> Huff10 {
        let mut lut = vec![(0u8, 0u16); 1 << LUT_BITS];
        let mut long = Vec::new();
        for (i, &e) in codes.iter().enumerate() {
            if e == 0 {
                continue;
            }
            let len = e >> 27;
            let code = e & 0x7ff_ffff;
            if len == 0 || (len < 27 && code >> len != 0) {
                continue;
            }
            let value = map.get(i).copied().unwrap_or(i as u16);
            if len <= LUT_BITS {
                let first = (code << (LUT_BITS - len)) as usize;
                for e in &mut lut[first..first + (1 << (LUT_BITS - len))] {
                    *e = (len as u8, value);
                }
            } else {
                long.push((len, code, value));
            }
        }
        long.sort_unstable();
        Huff10 { lut, long }
    }

    /// The next value (needs 31 buffered bits); an unknown code reads as 0 after one bit.
    #[inline(always)]
    fn next(&self, br: &mut BitReader) -> u16 {
        let (len, v) = self.lut[br.peek(LUT_BITS) as usize];
        if len != 0 {
            br.consume(len as u32);
            return v;
        }
        for &(len, code, v) in &self.long {
            if br.peek(len) == code {
                br.consume(len);
                return v;
            }
        }
        br.consume(1);
        0
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn spatial_gain_scales_above_black() {
        let (w, h) = (6usize, 4usize);
        let mut out: Vec<u16> = vec![100; w * h * 3];
        // Row 0 is the dark area at 20.
        for v in &mut out[..w * 3] {
            *v = 20;
        }
        let g = SpatialGain {
            grids: std::array::from_fn(|c| (2, 2, vec![1.0 + c as f32; 4])),
            area: crate::Rect {
                x: 0,
                y: 1,
                w: 6,
                h: 3,
            },
            dark: Some(crate::Rect {
                x: 0,
                y: 0,
                w: 6,
                h: 1,
            }),
        };
        g.apply(&mut out, w, h, false);
        assert_eq!(&out[..3], &[20, 20, 20]);
        assert_eq!(&out[w * 3..w * 3 + 3], &[100, 180, 260]);
    }

    #[test]
    fn sd_quattro_af_pixels() {
        // Lower layers at half size (16 x 16), top layer 32 x 32; one AF block at (8, 16).
        let keep = [16, 16, 32];
        let rows = [16, 16, 32];
        let mut planes = vec![vec![500u16; 256], vec![600u16; 256], vec![1000u16; 1024]];
        planes[2][16 * 32 + 9] = 400;
        planes[2][17 * 32 + 9] = 450;
        planes[0][8 * 16 + 4] = 100;
        planes[1][8 * 16 + 4] = 100;
        let g = AfGrid {
            x0: 0,
            y0: 0,
            x1: 32,
            y1: 32,
            step_x: 32,
            step_y: 8,
            sd: true,
        };
        af_correct_sd(&mut planes, keep, rows, &g);
        assert_eq!(planes[2][16 * 32 + 9], 1000);
        assert_eq!(planes[2][17 * 32 + 9], 1000);
        assert_eq!(planes[0][8 * 16 + 4], 500);
        assert_eq!(planes[1][8 * 16 + 4], 600);
    }

    /// MSB-first bit writer.
    #[derive(Default)]
    struct W {
        out: Vec<u8>,
        acc: u64,
        n: u32,
    }

    impl W {
        fn put(&mut self, v: u32, n: u32) {
            for i in (0..n).rev() {
                self.acc = (self.acc << 1) | ((v >> i) & 1) as u64;
                self.n += 1;
                if self.n == 8 {
                    self.out.push(self.acc as u8);
                    self.acc = 0;
                    self.n = 0;
                }
            }
        }
        fn finish(mut self) -> Vec<u8> {
            while self.n != 0 {
                self.put(0, 1);
            }
            self.out
        }
    }

    // Prefix code: lengths 0..=3 bits of difference as codes 00, 01, 10, 110 (index = bits).
    const TABLE: [(u8, u8); 4] = [(2, 0x00), (2, 0x40), (2, 0x80), (3, 0xc0)];

    fn put_diff(w: &mut W, d: i32) {
        let n = if d == 0 {
            0
        } else {
            32 - d.unsigned_abs().leading_zeros()
        };
        let (len, code) = TABLE[n as usize];
        w.put((code >> (8 - len)) as u32, len as u32);
        if n > 0 {
            let v = if d < 0 { d + (1 << n) - 1 } else { d };
            w.put(v as u32, n);
        }
    }

    fn encode_plane(v: &[i32], cols: usize, seed: i32) -> Vec<u8> {
        let mut w = W::default();
        let mut start = [[seed; 2]; 2];
        for (row, r) in v.chunks(cols).enumerate() {
            let mut acc = [0; 2];
            for (col, &x) in r.iter().enumerate() {
                let odd = col & 1;
                let prev = if col < 2 {
                    start[row & 1][odd]
                } else {
                    acc[odd]
                };
                put_diff(&mut w, x - prev);
                acc[odd] = x;
                if col < 2 {
                    start[row & 1][odd] = x;
                }
            }
        }
        w.finish()
    }

    pub(crate) fn true_image(w: usize, h: usize, vals: &[[i32; 3]]) -> Vec<u8> {
        let mut b = Vec::new();
        let seed = 4;
        for _ in 0..3 {
            b.extend((seed as u16).to_le_bytes());
        }
        b.extend([0, 0]);
        for (l, c) in TABLE {
            b.extend([l, c]);
        }
        b.extend([0, 0]);
        let planes: Vec<Vec<u8>> = (0..3)
            .map(|c| encode_plane(&vals.iter().map(|p| p[c]).collect::<Vec<_>>(), w, seed))
            .collect();
        for p in &planes {
            b.extend((p.len() as u32).to_le_bytes());
        }
        for p in &planes {
            let mut p = p.clone();
            p.resize(p.len().div_ceil(16) * 16, 0);
            b.extend(p);
        }
        let _ = h;
        b
    }

    #[test]
    fn true_roundtrip() {
        let (w, h) = (5, 3);
        let vals: Vec<[i32; 3]> = (0..w * h)
            .map(|i| [i as i32 % 7, 3 + (i as i32 * 3) % 5, 6 - i as i32 % 4])
            .collect();
        let src = true_image(w, h, &vals);
        let mut out = vec![0u16; w * h * 3];
        for parallel in [false, true] {
            assert!(!decode_true(&src, false, None, w, h, &mut out, parallel));
            let want: Vec<u16> = vals.iter().flat_map(|p| p.map(|v| v as u16)).collect();
            assert_eq!(out, want);
        }
        // Truncated: flagged, no panic.
        assert!(decode_true(&src[..20], false, None, w, h, &mut out, false));
    }

    #[test]
    fn huffman10_rows_and_offset() {
        let (w, h) = (3, 2);
        // Codes: value index i has the 2-bit code i for i < 4; map: 0, +5, -3, +1.
        let mut src = Vec::new();
        let map = [0u16, 5, (-3i16) as u16, 1];
        for i in 0..1024 {
            src.extend(map.get(i).copied().unwrap_or(0).to_le_bytes());
        }
        for i in 0..1024u32 {
            let e = if i < 4 { (2 << 27) | i } else { 0 };
            src.extend(e.to_le_bytes());
        }
        let rows: [[usize; 9]; 2] = [[1, 1, 2, 0, 3, 2, 1, 1, 1], [2, 2, 2, 3, 3, 3, 0, 0, 0]];
        let mut offsets = Vec::new();
        let mut data = Vec::new();
        for r in rows {
            offsets.push(data.len() as u32);
            let mut wr = W::default();
            for i in r {
                wr.put(i as u32, 2);
            }
            data.extend(wr.finish());
        }
        src.extend(&data);
        for o in offsets {
            src.extend(o.to_le_bytes());
        }
        let mut out = vec![0u16; w * h * 3];
        assert_eq!(decode_huffman10(&src, 0, w, h, &mut out, false), (false, 6));
        // The third layer of row 0 reaches -6, so the second pass starts every sum at 6.
        assert_eq!(&out[..9], &[11, 11, 3, 11, 12, 0, 16, 17, 5]);
        assert_eq!(&out[9..], &[3, 3, 3, 4, 4, 4, 4, 4, 4]);
        let mut par = vec![0u16; w * h * 3];
        decode_huffman10(&src, 0, w, h, &mut par, true);
        assert_eq!(out, par);
    }

    #[test]
    fn hostile_tables() {
        let mut out = vec![0u16; 4 * 4 * 3];
        let junk = vec![0xffu8; 300];
        decode_true(&junk, true, None, 4, 4, &mut out, false);
        decode_true(&junk, false, None, 4, 4, &mut out, true);
        decode_huffman10(&junk, 0, 4, 4, &mut out, false);
        decode_huffman10(&junk, 7, 4, 4, &mut out, false);
        decode_huffman10(&[], 0, 4, 4, &mut out, false);
    }
}
