//! VC-2 pictures: the transform parameters (§11.3), the slices of the low-delay syntax
//! (Dirac specification 2.2.3 §13.5) and of the high-quality syntax (SMPTE ST 2042-1), inverse
//! quantisation (§13.2) and the inverse transform per component.
//!
//! A low-delay slice holds a 7-bit quantiser index, the length of its luma data, the luma
//! coefficients and then the two chroma components interleaved coefficient by coefficient,
//! each part a bounded block. A high-quality slice holds prefix bytes, an 8-bit quantiser index
//! and, for each component in turn, a length byte (times the slice size scaler) and its
//! coefficients. Slices are independent, so they decode in parallel.

use rayon::prelude::*;

use crate::bits::{Block, Reader, intlog2};
use crate::seq::Sequence;
use crate::wavelet::{self, Bands};
use crate::{Error, Result, bad};

/// Default quantisation matrices (Dirac specification 2.2.3 Annex E.1, Tables E.1–E.7), by
/// wavelet index and transform depth 0–4: the LL value, then HL, LH, HH per level.
static DEFAULT_QM: [[&[u32]; 5]; 7] = {
    const DD: [&[u32]; 5] = [
        &[0],
        &[5, 3, 3, 0],
        &[5, 3, 3, 0, 4, 4, 1],
        &[5, 3, 3, 0, 4, 4, 1, 5, 5, 2],
        &[5, 3, 3, 0, 4, 4, 1, 5, 5, 2, 6, 6, 3],
    ];
    [
        DD,
        [
            &[0],
            &[4, 2, 2, 0],
            &[4, 2, 2, 0, 4, 4, 2],
            &[4, 2, 2, 0, 4, 4, 2, 5, 5, 3],
            &[4, 2, 2, 0, 4, 4, 2, 5, 5, 3, 7, 7, 5],
        ],
        DD,
        [
            &[0],
            &[8, 4, 4, 0],
            &[12, 8, 8, 4, 4, 4, 0],
            &[16, 12, 12, 8, 8, 8, 4, 4, 4, 0],
            &[20, 16, 16, 12, 12, 12, 8, 8, 8, 4, 4, 4, 0],
        ],
        [
            &[0],
            &[8, 4, 4, 0],
            &[8, 4, 4, 0, 4, 4, 0],
            &[8, 4, 4, 0, 4, 4, 0, 4, 4, 0],
            &[8, 4, 4, 0, 4, 4, 0, 4, 4, 0, 4, 4, 0],
        ],
        [
            &[0],
            &[0, 4, 4, 8],
            &[0, 4, 4, 8, 8, 8, 12],
            &[0, 4, 4, 8, 8, 8, 12, 13, 13, 17],
            &[0, 4, 4, 8, 8, 8, 12, 13, 13, 17, 17, 17, 21],
        ],
        [
            &[0],
            &[3, 1, 1, 0],
            &[3, 1, 1, 0, 4, 4, 2],
            &[3, 1, 1, 0, 4, 4, 2, 6, 6, 5],
            &[3, 1, 1, 0, 4, 4, 2, 6, 6, 5, 9, 9, 7],
        ],
    ]
};

/// The deepest transform accepted.
const MAX_DEPTH: u32 = 8;

/// The syntax of a picture's slices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Syntax {
    LowDelay { numer: u32, denom: u32 },
    HighQuality { prefix: u32, scaler: u32 },
}

/// A picture's header and transform parameters.
#[derive(Clone, Debug)]
pub(crate) struct Header {
    pub number: u32,
    pub wavelet: u32,
    pub depth: usize,
    pub slices_x: usize,
    pub slices_y: usize,
    pub syntax: Syntax,
    /// LL, then HL, LH, HH per level.
    pub qm: Vec<u32>,
    /// Where the slices start in the data unit.
    pub data_at: usize,
}

/// Parses the picture header and transform parameters of a low-delay (`hq` false) or
/// high-quality picture data unit (after its parse info header).
pub(crate) fn header(d: &[u8], seq: &Sequence, hq: bool) -> Result<Header> {
    let mut r = Reader::new(d);
    let number = r.lit(4)?;
    let wavelet = r.uint()?;
    let depth = r.uint()?;
    if seq.major >= 3 {
        // Extended transform parameters: an asymmetric transform (a different horizontal
        // wavelet, horizontal-only levels).
        let ho_index = if r.bool()? { Some(r.uint()?) } else { None };
        let ho_depth = if r.bool()? { r.uint()? } else { 0 };
        if ho_index.is_some_and(|i| i != wavelet) || ho_depth != 0 {
            return Err(Error::Unsupported(
                "VC-2 asymmetric wavelet transform (version 3)".into(),
            ));
        }
    }
    if wavelet >= wavelet::COUNT {
        return Err(Error::Unsupported(format!("VC-2 wavelet filter {wavelet}")));
    }
    if depth > MAX_DEPTH {
        return Err(Error::Unsupported(format!(
            "VC-2 transform of {depth} levels"
        )));
    }
    let slices_x = r.uint()? as usize;
    let slices_y = r.uint()? as usize;
    let syntax = if hq {
        Syntax::HighQuality {
            prefix: r.uint()?,
            scaler: r.uint()?,
        }
    } else {
        Syntax::LowDelay {
            numer: r.uint()?,
            denom: r.uint()?,
        }
    };
    let (lw, lh) = seq.luma_size();
    if slices_x == 0 || slices_y == 0 || slices_x.saturating_mul(slices_y) > lw * lh {
        return Err(bad(format!("{slices_x}×{slices_y} slices")));
    }
    if let Syntax::LowDelay { denom: 0, .. } = syntax {
        return Err(bad("slice size denominator 0"));
    }
    let n = 1 + 3 * depth as usize;
    let qm = if r.bool()? {
        (0..n).map(|_| r.uint()).collect::<Result<Vec<_>>>()?
    } else {
        DEFAULT_QM[wavelet as usize]
            .get(depth as usize)
            .map(|m| m.to_vec())
            .ok_or_else(|| {
                bad(format!(
                    "a {depth}-level transform without its quantisation matrix"
                ))
            })?
    };
    let data_at = r.byte_pos();
    Ok(Header {
        number,
        wavelet,
        depth: depth as usize,
        slices_x,
        slices_y,
        syntax,
        qm,
        data_at,
    })
}

/// Inverse quantisation factors and offsets (§13.2.1) for an intra picture.
struct Quant {
    factor: Vec<i64>,
    offset: Vec<i64>,
}

impl Quant {
    fn new() -> Quant {
        let mut factor = Vec::with_capacity(256);
        let mut offset = Vec::with_capacity(256);
        for q in 0..256u32 {
            let f = if q / 4 >= 31 {
                // Beyond any real stream; saturates.
                1i64 << 33
            } else {
                let base = 1i64 << (q / 4);
                match q % 4 {
                    0 => 4 * base,
                    1 => (503_829 * base + 52_958) / 105_917,
                    2 => (665_857 * base + 58_854) / 117_708,
                    _ => (440_253 * base + 32_722) / 65_444,
                }
            };
            factor.push(f);
            offset.push(match q {
                0 => 1,
                1 => 2,
                _ => (f + 1) / 2,
            });
        }
        Quant { factor, offset }
    }

    #[inline]
    fn apply(&self, v: i32, q: usize) -> i32 {
        if v == 0 {
            return 0;
        }
        let m = i64::from(v.unsigned_abs().min(1 << 28));
        let m = ((m * self.factor[q] + self.offset[q] + 2) >> 2).min(i64::from(i32::MAX)) as i32;
        if v < 0 { -m } else { m }
    }
}

/// A rectangle of one subband: left, top, right, bottom.
type Rect = (usize, usize, usize, usize);

/// The geometry of one component's subbands.
struct Comp {
    /// Band size per level.
    sizes: Vec<(usize, usize)>,
}

impl Comp {
    fn new(w: usize, h: usize, depth: usize) -> Comp {
        let scale = 1usize << depth;
        let (pw, ph) = (w.div_ceil(scale) * scale, h.div_ceil(scale) * scale);
        let sizes = (0..=depth)
            .map(|level| {
                let k = if level == 0 { depth } else { depth - level + 1 };
                (pw >> k, ph >> k)
            })
            .collect();
        Comp { sizes }
    }

    fn padded(&self) -> (usize, usize) {
        let (w, h) = *self.sizes.last().unwrap_or(&(0, 0));
        if self.sizes.len() == 1 {
            (w, h)
        } else {
            (2 * w, 2 * h)
        }
    }

    /// §13.5.5.1: the part of the band at `level` slice (sx, sy) covers.
    fn rect(&self, level: usize, sx: usize, sy: usize, nx: usize, ny: usize) -> Rect {
        let (w, h) = self.sizes[level];
        (
            w * sx / nx,
            h * sy / ny,
            w * (sx + 1) / nx,
            h * (sy + 1) / ny,
        )
    }

    fn count(&self, sx: usize, sy: usize, nx: usize, ny: usize) -> usize {
        (0..self.sizes.len())
            .map(|level| {
                let (l, t, r, b) = self.rect(level, sx, sy, nx, ny);
                let bands = if level == 0 { 1 } else { 3 };
                bands * (r - l) * (b - t)
            })
            .sum()
    }
}

/// Bands in slice order: (level, orient, quantiser slot in the matrix).
fn band_order(depth: usize) -> Vec<(usize, usize, usize)> {
    let mut v = vec![(0, 0, 0)];
    for level in 1..=depth {
        for orient in 0..3 {
            v.push((level, orient, 1 + 3 * (level - 1) + orient));
        }
    }
    v
}

/// The coefficients of one slice, per component, in slice order.
type SliceCoeffs = [Vec<i32>; 3];

/// Decodes a picture's coefficients and runs the inverse transform: the three padded
/// components with their padded widths.
pub(crate) fn decode(d: &[u8], h: &Header, seq: &Sequence) -> Result<[(Vec<i32>, usize); 3]> {
    let (lw, lh) = seq.luma_size();
    let (cw, ch) = seq.chroma_size();
    let comps = [
        Comp::new(lw, lh, h.depth),
        Comp::new(cw, ch, h.depth),
        Comp::new(cw, ch, h.depth),
    ];
    let total: usize = comps
        .iter()
        .map(|c| {
            let (w, hh) = c.padded();
            w * hh
        })
        .sum();
    // Coefficients twice (slices and bands) plus the transform's working arrays.
    let _held = specular_core::budget::reserve(total as u64 * 4 * 4)?;
    let (nx, ny) = (h.slices_x, h.slices_y);
    let data = d.get(h.data_at..).unwrap_or(&[]);
    let order = band_order(h.depth);
    let quant = Quant::new();

    // Where every slice starts.
    let n = nx * ny;
    let mut starts = Vec::with_capacity(n + 1);
    match h.syntax {
        Syntax::LowDelay { numer, denom } => {
            let (numer, denom) = (u64::from(numer), u64::from(denom));
            for i in 0..=n as u64 {
                starts.push((i * numer / denom) as usize);
            }
        }
        Syntax::HighQuality { prefix, scaler } => {
            let mut p = 0usize;
            for _ in 0..n {
                starts.push(p);
                p = p.saturating_add(prefix as usize + 1);
                for _ in 0..3 {
                    let len = data.get(p).copied().unwrap_or(0) as usize;
                    p = p.saturating_add(1 + len * scaler as usize);
                }
            }
            starts.push(p);
        }
    }

    let slices: Vec<SliceCoeffs> = (0..n)
        .into_par_iter()
        .with_min_len(4)
        .map(|i| {
            let (sx, sy) = (i % nx, i / nx);
            let lo = starts[i].min(data.len());
            let hi = starts[i + 1].clamp(lo, data.len());
            let s = &data[lo..hi];
            let mut out: SliceCoeffs = [
                Vec::with_capacity(comps[0].count(sx, sy, nx, ny)),
                Vec::with_capacity(comps[1].count(sx, sy, nx, ny)),
                Vec::with_capacity(comps[2].count(sx, sy, nx, ny)),
            ];
            match h.syntax {
                Syntax::LowDelay { .. } => {
                    let bytes = starts[i + 1] - starts[i];
                    ld_slice(s, bytes, h, &comps, &order, &quant, (sx, sy), &mut out);
                }
                Syntax::HighQuality { prefix, scaler } => {
                    hq_slice(
                        s,
                        prefix,
                        scaler,
                        h,
                        &comps,
                        &order,
                        &quant,
                        (sx, sy),
                        &mut out,
                    );
                }
            }
            out
        })
        .collect();

    // Scatter the slices into the bands.
    let mut bands: Vec<Bands> = comps
        .iter()
        .map(|c| {
            let (pw, ph) = c.padded();
            Bands::new(pw, ph, h.depth)
        })
        .collect();
    for (i, s) in slices.iter().enumerate() {
        let (sx, sy) = (i % nx, i / nx);
        for (c, comp) in comps.iter().enumerate() {
            let mut it = s[c].iter();
            for &(level, orient, _) in &order {
                let (l, t, r, b) = comp.rect(level, sx, sy, nx, ny);
                let bw = comp.sizes[level].0;
                let band = &mut bands[c].data[level][orient];
                for y in t..b {
                    for v in &mut band[y * bw + l..y * bw + r] {
                        *v = it.next().copied().unwrap_or(0);
                    }
                }
            }
        }
    }
    drop(slices);
    // §13.5.1: low-delay DC bands are predicted across the whole picture (high-quality ones
    // are not).
    if matches!(h.syntax, Syntax::LowDelay { .. }) {
        for b in &mut bands {
            let (w, hh) = (b.w, b.h);
            dc_prediction(&mut b.data[0][0], w, hh);
        }
    }
    let wavelet = h.wavelet;
    let mut it = bands.into_iter();
    let (y, c1, c2) = (it.next(), it.next(), it.next());
    let (Some(y), Some(c1), Some(c2)) = (y, c1, c2) else {
        return Err(bad("components"));
    };
    let pw = [
        comps[0].padded().0,
        comps[1].padded().0,
        comps[2].padded().0,
    ];
    let (y, (c1, c2)) = rayon::join(
        || y.synthesise(wavelet),
        || rayon::join(|| c1.synthesise(wavelet), || c2.synthesise(wavelet)),
    );
    Ok([(y, pw[0]), (c1, pw[1]), (c2, pw[2])])
}

/// §13.3: intra DC prediction of a `w`×`h` LL band from the mean of the left, top-left and
/// top neighbours.
fn dc_prediction(band: &mut [i32], w: usize, h: usize) {
    for y in 0..h {
        for x in 0..w {
            let p = match (x > 0, y > 0) {
                (true, true) => {
                    let s = i64::from(band[y * w + x - 1])
                        + i64::from(band[(y - 1) * w + x - 1])
                        + i64::from(band[(y - 1) * w + x]);
                    // mean() rounds as the specification's integer mean: (sum + n//2)//n.
                    (s + 1).div_euclid(3) as i32
                }
                (true, false) => band[x - 1],
                (false, true) => band[(y - 1) * w],
                (false, false) => 0,
            };
            band[y * w + x] = band[y * w + x].wrapping_add(p);
        }
    }
}

fn quantisers(qindex: u32, qm: &[u32]) -> Vec<usize> {
    qm.iter()
        .map(|&m| qindex.saturating_sub(m).min(255) as usize)
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn ld_slice(
    s: &[u8],
    bytes: usize,
    h: &Header,
    comps: &[Comp; 3],
    order: &[(usize, usize, usize)],
    quant: &Quant,
    (sx, sy): (usize, usize),
    out: &mut SliceCoeffs,
) {
    let (nx, ny) = (h.slices_x, h.slices_y);
    let bits = 8 * bytes;
    let (qindex, length_bits, y_len) = if bits >= 8 {
        let mut b = Block::new(s, 0, bits);
        let q = b.nbits(7);
        let lb = intlog2(bits as u64 - 7);
        let y = b.nbits(lb) as usize;
        (q, lb as usize, y)
    } else {
        (0, 0, 0)
    };
    let q = quantisers(qindex, &h.qm);
    let left = bits.saturating_sub(7 + length_bits);
    let y_len = y_len.min(left);
    let mut b = Block::new(s, 7 + length_bits, y_len);
    for &(level, _, slot) in order {
        let (l, t, r, bo) = comps[0].rect(level, sx, sy, nx, ny);
        for _ in 0..(r - l) * (bo - t) {
            out[0].push(quant.apply(b.sint(), q[slot]));
        }
    }
    let mut b = Block::new(s, 7 + length_bits + y_len, left - y_len);
    for &(level, _, slot) in order {
        let (l, t, r, bo) = comps[1].rect(level, sx, sy, nx, ny);
        for _ in 0..(r - l) * (bo - t) {
            out[1].push(quant.apply(b.sint(), q[slot]));
            out[2].push(quant.apply(b.sint(), q[slot]));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn hq_slice(
    s: &[u8],
    prefix: u32,
    scaler: u32,
    h: &Header,
    comps: &[Comp; 3],
    order: &[(usize, usize, usize)],
    quant: &Quant,
    (sx, sy): (usize, usize),
    out: &mut SliceCoeffs,
) {
    let (nx, ny) = (h.slices_x, h.slices_y);
    let mut p = prefix as usize;
    let qindex = s.get(p).copied().unwrap_or(0);
    p += 1;
    let q = quantisers(u32::from(qindex), &h.qm);
    for (c, comp) in comps.iter().enumerate() {
        let len = s.get(p).copied().unwrap_or(0) as usize * scaler as usize;
        p += 1;
        let mut b = Block::new(s, 8 * p, 8 * len);
        for &(level, _, slot) in order {
            let (l, t, r, bo) = comp.rect(level, sx, sy, nx, ny);
            for _ in 0..(r - l) * (bo - t) {
                out[c].push(quant.apply(b.sint(), q[slot]));
            }
        }
        p += len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantisation_factors() {
        let q = Quant::new();
        // §13.2.1: index 0 gives 4, index 4 gives 8, index 8 gives 16.
        assert_eq!(&q.factor[..9], &[4, 5, 6, 7, 8, 10, 11, 13, 16]);
        assert_eq!(q.apply(0, 9), 0);
        assert_eq!(q.apply(1, 0), 1);
        assert_eq!(q.apply(-3, 0), -3);
        // Huge indices saturate instead of overflowing.
        assert!(q.apply(1 << 20, 255) > 0);
    }

    #[test]
    fn slice_geometry_covers_every_band() {
        let c = Comp::new(70, 46, 3);
        assert_eq!(c.padded(), (72, 48));
        let total: usize = (0..3)
            .flat_map(|sy| (0..5).map(move |sx| (sx, sy)))
            .map(|(sx, sy)| c.count(sx, sy, 5, 3))
            .sum();
        assert_eq!(total, 72 * 48);
    }
}
