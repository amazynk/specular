//! Full-precision decoding for sample formats the display paths reduce to 8 bits: every T.81
//! process at 8 or 12 bits (DCT) or 2..=16 bits (lossless). Medical images (DICOM) need the
//! exact stored values, not a display rendering, so this path returns every component as a
//! plane of raw `u16` samples at its own (subsampled) resolution, without colour conversion.
//!
//! * Sequential Huffman DCT (SOF0/SOF1): a straightforward single-threaded decoder here
//!   (DICOM frames are small next to the photographs the main paths are tuned for), keeping
//!   what a cut stream holds.
//! * Progressive and arithmetic DCT (SOF2, SOF9, SOF10): the coefficient reader of the
//!   lossless transforms (`crate::transform`), then the same islow IDCT without the 8-bit
//!   reduction (bit-exact with libjpeg-turbo at 8 and 12 bits). Damaged data is an error.
//! * Lossless: the display path's decoders (`crate::lossless`, restart intervals in parallel
//!   on large frames; `crate::arith::lossless` for SOF11).
//! * Hierarchical: the final components of `crate::hier` at whole-sample precision.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{JpegError, Result};
use crate::huffman::{BitReader, HuffTable};
use crate::idct::idct_islow_raw;
use crate::marker::{self, Frame, Scan, ZIGZAG};

/// One decoded component.
#[derive(Clone, Debug)]
pub struct WidePlane {
    /// `ceil(W * h / Hmax)` by `ceil(H * v / Vmax)`.
    pub width: u32,
    pub height: u32,
    pub h_samp: u8,
    pub v_samp: u8,
    /// Row-major samples, `width * height`, in `0..2^precision`.
    pub data: Vec<u16>,
}

/// A frame decoded at full sample precision.
#[derive(Clone, Debug)]
pub struct WideImage {
    pub width: u32,
    pub height: u32,
    /// Sample precision of the frame (bits).
    pub precision: u8,
    /// True for a lossless (SOF3) frame.
    pub lossless: bool,
    /// Component ids from the frame header (for colour-model decisions by the caller).
    pub ids: Vec<u8>,
    pub planes: Vec<WidePlane>,
    /// The entropy-coded data ended early (missing samples are 0 / mid-grey).
    pub truncated: bool,
}

/// Largest frame decoded here (samples over all components, before allocation).
const MAX_SAMPLES: u64 = 1 << 30;

/// Decodes `data` (a complete JPEG stream) at full precision: every process of T.81 (Huffman
/// or arithmetic, sequential, progressive, lossless, hierarchical), except arithmetic-coded
/// lossless frames inside a hierarchical image. Progressive and arithmetic DCT frames and
/// hierarchical images with damaged entropy-coded data are refused ([`JpegError::Truncated`])
/// rather than shown in part.
pub fn decode_wide(data: &[u8], cancel: Option<&AtomicBool>) -> Result<WideImage> {
    let p = marker::parse(data, true)?;
    let f = &p.frame;
    if p.hierarchical {
        return hierarchical(data, &p, cancel);
    }
    let samples: u64 = f
        .comps
        .iter()
        .map(|c| comp_dims(f, c.h, c.v).0 as u64 * comp_dims(f, c.h, c.v).1 as u64)
        .sum();
    if samples > MAX_SAMPLES {
        return Err(JpegError::TooLarge(samples * 2));
    }
    let mut planes = Vec::with_capacity(f.comps.len());
    for c in &f.comps {
        let (w, h) = comp_dims(f, c.h, c.v);
        planes.push(WidePlane {
            width: w,
            height: h,
            h_samp: c.h as u8,
            v_samp: c.v as u8,
            data: Vec::new(),
        });
    }
    let truncated = if f.lossless && f.arithmetic {
        let (comps, truncated) = crate::arith::lossless::samples(&p, data, cancel)?;
        for (plane, (_, _, d)) in planes.iter_mut().zip(comps) {
            plane.data = d;
        }
        truncated
    } else if f.lossless {
        lossless(data, &p, &mut planes, cancel)?
    } else if f.arithmetic || f.progressive {
        let coefs = crate::transform::coefficients(&p, data)?;
        let blocks: Vec<&[i16]> = coefs.iter().map(|c| &c.coefs[..]).collect();
        idct_planes(f, &p.quant, &blocks, &mut planes, cancel)?;
        false
    } else {
        sequential(data, f, &p.quant, &p.scans, &mut planes, cancel)?
    };
    Ok(WideImage {
        width: f.width,
        height: f.height,
        precision: f.precision,
        lossless: f.lossless,
        ids: f.comps.iter().map(|c| c.id).collect(),
        planes,
        truncated: truncated || !p.saw_eoi && p.scans.len() > 1,
    })
}

fn comp_dims(f: &Frame, h: u32, v: u32) -> (u32, u32) {
    (
        (f.width as u64 * h as u64).div_ceil(f.max_h as u64) as u32,
        (f.height as u64 * v as u64).div_ceil(f.max_v as u64) as u32,
    )
}

fn cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|c| c.load(Ordering::Relaxed))
}

fn alloc<T: Clone>(n: usize, v: T) -> Result<Vec<T>> {
    let mut out = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| JpegError::TooLarge((n * std::mem::size_of::<T>()) as u64))?;
    out.resize(n, v);
    Ok(out)
}

fn reader<'a>(data: &'a [u8], s: &Scan) -> BitReader<'a> {
    BitReader {
        data: &data[..s.data_end.min(data.len())],
        pos: s.data_start.min(data.len()),
        buf: 0,
        bits: 0,
        marker_hit: false,
        zero_fill: 0,
    }
}

// ------------------------------------------------------------------------------- lossless

/// Lossless (SOF3) through [`crate::lossless`]: the stored values, cropped to each
/// component's extent and shifted back by the point transform. Large frames decode their
/// restart intervals in parallel. Returns whether the data ran out.
fn lossless(
    data: &[u8],
    p: &crate::marker::Parsed,
    planes: &mut [WidePlane],
    cancel: Option<&AtomicBool>,
) -> Result<bool> {
    let f = &p.frame;
    let pixels = f.width as u64 * f.height as u64;
    let threads = if pixels >= crate::AUTO_PARALLEL_MIN_PIXELS {
        rayon::current_num_threads()
    } else {
        1
    };
    let s = crate::lossless::decode_samples(p, data, f.height as usize, threads, cancel)?;
    for (plane, c) in planes.iter_mut().zip(&s.comps) {
        let (w, h) = (plane.width as usize, plane.height as usize);
        let mut out = alloc(w * h, 0u16)?;
        for (y, row) in out.chunks_exact_mut(w).enumerate() {
            let src = &c.data[y * c.stride..][..w];
            if c.pt == 0 {
                row.copy_from_slice(src);
            } else {
                for (o, &v) in row.iter_mut().zip(src) {
                    *o = v << c.pt;
                }
            }
        }
        plane.data = out;
    }
    Ok(s.truncated)
}

// ----------------------------------------------------------------------------- sequential

/// Sequential Huffman DCT (SOF0/SOF1): every scan into a coefficient buffer, then the islow
/// IDCT without the 8-bit reduction.
fn sequential(
    data: &[u8],
    f: &Frame,
    quant: &[[u16; 64]],
    scans: &[Scan],
    planes: &mut [WidePlane],
    cancel: Option<&AtomicBool>,
) -> Result<bool> {
    let mut coefs: Vec<Vec<i16>> = Vec::with_capacity(f.comps.len());
    for c in &f.comps {
        coefs.push(alloc(c.bw_pad as usize * c.bh_pad as usize * 64, 0i16)?);
    }
    let mut truncated = false;
    for s in scans {
        let mut br = reader(data, s);
        let ri = s.restart_interval as usize;
        let mut preds = vec![0i32; s.comps.len()];
        let tabs: Vec<(&HuffTable, &HuffTable)> = s
            .comps
            .iter()
            .map(|c| match (s.dc[c.td].as_deref(), s.ac[c.ta].as_deref()) {
                (Some(d), Some(a)) => Ok((d, a)),
                _ => Err(JpegError::Malformed("missing Huffman table")),
            })
            .collect::<Result<_>>()?;
        let mut mcu = 0usize;
        let mut restart = |br: &mut BitReader, preds: &mut [i32]| {
            if ri > 0 && mcu > 0 && mcu.is_multiple_of(ri) {
                br.restart();
                preds.fill(0);
            }
            mcu += 1;
        };
        if s.comps.len() == 1 {
            let ci = s.comps[0].ci;
            let c = &f.comps[ci];
            for by in 0..c.bh as usize {
                if cancelled(cancel) {
                    return Err(JpegError::Cancelled);
                }
                for bx in 0..c.bw as usize {
                    restart(&mut br, &mut preds);
                    let i = (by * c.bw_pad as usize + bx) * 64;
                    let blk = (&mut coefs[ci][i..i + 64]).try_into().unwrap();
                    block(&mut br, tabs[0], &mut preds[0], blk);
                }
            }
        } else {
            for my in 0..f.mcus_y as usize {
                if cancelled(cancel) {
                    return Err(JpegError::Cancelled);
                }
                for mx in 0..f.mcus_x as usize {
                    restart(&mut br, &mut preds);
                    for (k, sc) in s.comps.iter().enumerate() {
                        let c = &f.comps[sc.ci];
                        let (h, v) = (c.h as usize, c.v as usize);
                        for by in 0..v {
                            for bx in 0..h {
                                let i = ((my * v + by) * c.bw_pad as usize + mx * h + bx) * 64;
                                let blk = (&mut coefs[sc.ci][i..i + 64]).try_into().unwrap();
                                block(&mut br, tabs[k], &mut preds[k], blk);
                            }
                        }
                    }
                }
            }
        }
        truncated |= br.overrun();
    }
    let blocks: Vec<&[i16]> = coefs.iter().map(|c| &c[..]).collect();
    idct_planes(f, quant, &blocks, planes, cancel)?;
    Ok(truncated)
}

/// The islow IDCT of every block of every component (`coefs[ci]`: 64 natural-order
/// coefficients per block over the MCU-padded grid), without the 8-bit reduction.
fn idct_planes(
    f: &Frame,
    quant: &[[u16; 64]],
    coefs: &[&[i16]],
    planes: &mut [WidePlane],
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    let maxv = (1i32 << f.precision) - 1;
    let shift = 1i32 << (f.precision - 1);
    let pass1 = if f.precision > 8 { 1 } else { 2 };
    for (ci, plane) in planes.iter_mut().enumerate() {
        let c = &f.comps[ci];
        let (w, h) = (plane.width as usize, plane.height as usize);
        let mut out = alloc(w * h, 0u16)?;
        let mut px = [0i32; 64];
        for by in 0..c.bh as usize {
            if cancelled(cancel) {
                return Err(JpegError::Cancelled);
            }
            for bx in 0..c.bw as usize {
                let i = (by * c.bw_pad as usize + bx) * 64;
                let blk: &[i16; 64] = coefs[ci][i..i + 64].try_into().unwrap();
                idct_islow_raw(blk, &quant[ci], pass1, &mut px);
                for r in 0..8 {
                    let y = by * 8 + r;
                    if y >= h {
                        break;
                    }
                    for x in 0..8 {
                        let xx = bx * 8 + x;
                        if xx >= w {
                            break;
                        }
                        out[y * w + xx] =
                            (px[r * 8 + x].saturating_add(shift)).clamp(0, maxv) as u16;
                    }
                }
            }
        }
        plane.data = out;
    }
    Ok(())
}

// --------------------------------------------------------------------------- hierarchical

/// A hierarchical image (T.81 Annex J) through [`crate::hier`]: the final components at
/// their DHP sizes.
fn hierarchical(
    data: &[u8],
    p: &crate::marker::Parsed,
    cancel: Option<&AtomicBool>,
) -> Result<WideImage> {
    let f = &p.frame;
    let comps = crate::hier::components(data, p, cancel)?;
    let planes = comps
        .into_iter()
        .zip(&f.comps)
        .map(|((d, w, h), c)| WidePlane {
            width: w as u32,
            height: h as u32,
            h_samp: c.h as u8,
            v_samp: c.v as u8,
            data: d,
        })
        .collect();
    Ok(WideImage {
        width: f.width,
        height: f.height,
        precision: f.precision,
        lossless: f.lossless,
        ids: f.comps.iter().map(|c| c.id).collect(),
        planes,
        truncated: false,
    })
}

/// One sequential Huffman block (DC difference + AC run/size pairs), natural order.
#[inline]
fn block(
    br: &mut BitReader,
    (dc, ac): (&HuffTable, &HuffTable),
    pred: &mut i32,
    blk: &mut [i16; 64],
) {
    br.ensure(32);
    let s = br.decode_symbol(dc) & 15;
    if s != 0 {
        br.ensure(s);
        *pred = pred.wrapping_add(br.get_extend(s));
    }
    blk[0] = *pred as i16;
    let mut k = 1usize;
    while k < 64 {
        br.ensure(32);
        let sym = br.decode_symbol(ac);
        let (r, s) = ((sym >> 4) as usize, sym & 15);
        if s == 0 {
            if r == 15 {
                k += 16;
                continue;
            }
            break;
        }
        k += r;
        let v = br.get_extend(s);
        if k < 64 {
            blk[ZIGZAG[k] as usize] = v as i16;
        }
        k += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal lossless (SOF3) encoder for tests: one component, predictor `pred`, point
    /// transform 0, a fixed Huffman table covering categories 0..=16.
    pub(crate) fn encode_lossless(w: u16, h: u16, p: u8, pred: u8, px: &[u16], ri: u16) -> Vec<u8> {
        // Table: category c has a (c+1)-bit-ish code; use 17 symbols with lengths 2..=?:
        // counts: length 4 -> 9 codes? Simpler: all 17 symbols with length 5 (32 codes).
        let mut counts = [0u8; 16];
        counts[4] = 17;
        let vals: Vec<u8> = (0..=16).collect();
        let mut o = vec![0xFF, 0xD8];
        o.extend([0xFF, 0xC3, 0, 11, p]);
        o.extend(h.to_be_bytes());
        o.extend(w.to_be_bytes());
        o.extend([1, 1, 0x11, 0]);
        o.extend([0xFF, 0xC4]);
        o.extend(((2 + 1 + 16 + vals.len()) as u16).to_be_bytes());
        o.push(0x00);
        o.extend(counts);
        o.extend(&vals);
        if ri > 0 {
            o.extend([0xFF, 0xDD, 0, 4]);
            o.extend(ri.to_be_bytes());
        }
        o.extend([0xFF, 0xDA, 0, 8, 1, 1, 0x00, pred, 0, 0]);
        let mut bits = BitWriter::default();
        let init = 1i32 << (p - 1);
        let (w, h) = (w as usize, h as usize);
        let mut n = 0usize;
        let mut first_row = 0usize;
        let mut rst = 0u8;
        for y in 0..h {
            for x in 0..w {
                let mut restarted = false;
                if ri > 0 && n > 0 && n.is_multiple_of(ri as usize) {
                    bits.flush(&mut o);
                    o.extend([0xFF, 0xD0 + rst]);
                    rst = (rst + 1) & 7;
                    first_row = y;
                    restarted = true;
                }
                n += 1;
                let i = y * w + x;
                let g = |i: usize| px[i] as i32;
                let pv = if y == first_row {
                    if x == 0 || restarted { init } else { g(i - 1) }
                } else if x == 0 {
                    g(i - w)
                } else {
                    let (ra, rb, rc) = (g(i - 1), g(i - w), g(i - w - 1));
                    match pred {
                        1 => ra,
                        2 => rb,
                        3 => rc,
                        4 => ra + rb - rc,
                        5 => ra + ((rb - rc) >> 1),
                        6 => rb + ((ra - rc) >> 1),
                        _ => (ra + rb) >> 1,
                    }
                };
                let d = ((g(i) - pv) as i16) as i32;
                let cat = if d == -32768 {
                    16
                } else {
                    32 - d.unsigned_abs().leading_zeros()
                };
                bits.put(cat, 5);
                if (1..16).contains(&cat) {
                    let v = if d < 0 { d + (1 << cat) - 1 } else { d } as u32;
                    bits.put(v, cat);
                }
            }
        }
        bits.flush(&mut o);
        o.extend([0xFF, 0xD9]);
        o
    }

    #[derive(Default)]
    struct BitWriter {
        acc: u64,
        n: u32,
        bytes: Vec<u8>,
    }

    impl BitWriter {
        fn put(&mut self, v: u32, len: u32) {
            self.acc = (self.acc << len) | (v as u64 & ((1u64 << len) - 1));
            self.n += len;
            while self.n >= 8 {
                let b = (self.acc >> (self.n - 8)) as u8;
                self.bytes.push(b);
                if b == 0xFF {
                    self.bytes.push(0);
                }
                self.n -= 8;
            }
        }
        fn flush(&mut self, out: &mut Vec<u8>) {
            if self.n > 0 {
                let pad = 8 - self.n;
                self.put((1 << pad) - 1, pad);
            }
            out.append(&mut self.bytes);
        }
    }

    fn pattern(w: usize, h: usize, p: u8) -> Vec<u16> {
        let m = (1u32 << p) - 1;
        (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as u32, (i / w) as u32);
                ((x * 977 + y * 131 + (x * y) % 61 * 1013) & m) as u16
            })
            .collect()
    }

    #[test]
    fn lossless_every_predictor_and_precision_round_trips() {
        for p in [2u8, 8, 12, 15, 16] {
            for pred in 1..=7u8 {
                for ri in [0u16, 7, 13] {
                    let (w, h) = (13usize, 9usize);
                    let px = pattern(w, h, p);
                    let jpg = encode_lossless(w as u16, h as u16, p, pred, &px, ri);
                    let img = decode_wide(&jpg, None).unwrap();
                    assert_eq!(img.precision, p);
                    assert!(img.lossless);
                    assert_eq!(img.planes.len(), 1);
                    assert_eq!(img.planes[0].data, px, "p={p} pred={pred} ri={ri}");
                    assert!(!img.truncated);
                }
            }
        }
    }

    #[test]
    fn corrupt_and_truncated_lossless_never_panics() {
        let px = pattern(31, 17, 16);
        let jpg = encode_lossless(31, 17, 16, 1, &px, 0);
        for cut in (0..jpg.len()).step_by(7) {
            let _ = decode_wide(&jpg[..cut], None);
        }
        let mut bad = jpg.clone();
        for i in (40..bad.len() - 2).step_by(5) {
            bad[i] ^= 0x5A;
        }
        let _ = decode_wide(&bad, None);
    }
}
