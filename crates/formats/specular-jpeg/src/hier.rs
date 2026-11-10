//! Hierarchical JPEG (ITU-T T.81 Annex J): a DHP segment, a non-differential frame per
//! component, then differential frames, each optionally after an EXP segment that doubles the
//! reference components horizontally and/or vertically.
//!
//! Each frame is split out as a stand-alone stream (the table segments seen so far, its SOF
//! with a differential code mapped to the matching non-differential one, its scans) and
//! decoded by the ordinary coefficient readers, which only drop DC prediction for differential
//! frames (`Frame::differential`). Lossless frames give their samples (non-differential) or
//! differences (differential: predictor 0, point transform applied). Every differential frame
//! adds its output to the upsampled reference components modulo 2^16 (J.2.1).
//!
//! Precision: the reference components keep [`FRAC`] fractional bits. DCT frames go through
//! an exact (`f64`) IDCT rounded to that precision (non-differential: level-shifted and
//! clamped; differential: neither), the J.1.1.2 filter truncates at that precision, and only
//! the final components are rounded to whole samples. A lossless differential frame works on
//! the whole-sample part (rounded down) of its reference. This is how the ISO reference
//! libjpeg, the one hierarchical encoder there is, reconstructs: decoding its files with
//! whole-sample references (as Annex J reads literally) leaves a third of the samples off by
//! one or two, and with these rules about 2% differ by one (IDCT rounding).
//!
//! The final components are cropped to the DHP size, reduced to 8 bits, then box-downscaled
//! and cropped like the lossless paths. The whole image is decoded serially for every call;
//! hierarchical files are rare and small. Arithmetic-coded lossless frames in a hierarchical
//! image are refused.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{JpegError, Result};
use crate::fallback::{out_rect, to_8bit};
use crate::huffman::BitReader;
use crate::marker::{Parsed, find_scan_end};
use crate::{DecodeOptions, Plane};

/// One frame of the hierarchical sequence as a stand-alone stream.
struct Job {
    /// EXP: double the reference horizontally / vertically first.
    exp: (bool, bool),
    differential: bool,
    stream: Vec<u8>,
}

/// A reconstructed component: samples modulo 2^16 with [`FRAC`] fractional bits (so modulo
/// 2^(16 + FRAC)).
struct Comp {
    id: u8,
    w: usize,
    h: usize,
    data: Vec<i32>,
}

/// Fractional bits of the reference components.
const FRAC: u32 = 4;
/// Reference values wrap modulo 2^(16 + FRAC).
const WRAP: i32 = (1 << (16 + FRAC)) - 1;

/// Largest image decoded here (samples over all components, any frame).
const MAX_SAMPLES: u64 = 1 << 28;

fn alloc<T: Clone>(n: usize, v: T) -> Result<Vec<T>> {
    let mut out = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| JpegError::TooLarge(n as u64 * std::mem::size_of::<T>() as u64))?;
    out.resize(n, v);
    Ok(out)
}

/// Splits the frames out of `data`.
fn split(data: &[u8]) -> Result<Vec<Job>> {
    let n = data.len();
    let mut tables: Vec<u8> = Vec::new();
    let mut jobs = Vec::new();
    let mut exp = (false, false);
    let mut cur: Option<Job> = None;
    let mut pos = 2usize;
    loop {
        while pos < n && data[pos] != 0xFF {
            pos += 1;
        }
        while pos + 1 < n && data[pos + 1] == 0xFF {
            pos += 1;
        }
        if pos + 1 >= n {
            break;
        }
        let m = data[pos + 1];
        match m {
            0xD8 | 0x01 | 0x00 | 0xD0..=0xD7 => {
                pos += 2;
                continue;
            }
            0xD9 => break,
            _ => {}
        }
        if pos + 4 > n {
            break;
        }
        let len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if len < 2 {
            return Err(JpegError::Malformed("marker length < 2"));
        }
        let end = pos + 2 + len;
        if end > n {
            break;
        }
        let seg = &data[pos..end];
        let sof = matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC);
        if (sof || m == 0xDF)
            && let Some(mut j) = cur.take()
        {
            j.stream.extend_from_slice(&[0xFF, 0xD9]);
            jobs.push(j);
        }
        match m {
            0xDF => {
                if len != 3 || seg[4] & 0xEE != 0 {
                    return Err(JpegError::Malformed("EXP"));
                }
                exp = (seg[4] >> 4 != 0, seg[4] & 15 != 0);
            }
            _ if sof => {
                let code = match m {
                    0xC5..=0xC7 => m - 4,
                    0xCD..=0xCF => m - 4,
                    c => c,
                };
                let mut stream = Vec::with_capacity(tables.len() + seg.len() + 4);
                stream.extend_from_slice(&[0xFF, 0xD8]);
                stream.extend_from_slice(&tables);
                stream.extend_from_slice(&[0xFF, code]);
                stream.extend_from_slice(&seg[2..]);
                cur = Some(Job {
                    exp,
                    differential: matches!(m, 0xC5..=0xC7 | 0xCD..=0xCF),
                    stream,
                });
                exp = (false, false);
            }
            0xDA => {
                let j = cur.as_mut().ok_or(JpegError::Malformed("SOS before SOF"))?;
                let scan_end = find_scan_end(data, end);
                j.stream.extend_from_slice(&data[pos..scan_end]);
                pos = scan_end;
                continue;
            }
            0xC4 | 0xDB | 0xCC | 0xDD => {
                tables.extend_from_slice(seg);
                if let Some(j) = cur.as_mut() {
                    j.stream.extend_from_slice(seg);
                }
            }
            0xDC => {
                if let Some(j) = cur.as_mut() {
                    j.stream.extend_from_slice(seg);
                }
            }
            _ => {}
        }
        pos = end;
    }
    if let Some(mut j) = cur.take() {
        j.stream.extend_from_slice(&[0xFF, 0xD9]);
        jobs.push(j);
    }
    if jobs.is_empty() || jobs[0].differential {
        return Err(JpegError::Malformed(
            "hierarchical image does not start with a non-differential frame",
        ));
    }
    Ok(jobs)
}

/// Component sizes of a frame: (id, width, height) in frame component order.
fn comp_dims(p: &Parsed) -> Vec<(u8, usize, usize)> {
    let f = &p.frame;
    f.comps
        .iter()
        .map(|c| {
            (
                c.id,
                (f.width as u64 * c.h as u64).div_ceil(f.max_h as u64) as usize,
                (f.height as u64 * c.v as u64).div_ceil(f.max_v as u64) as usize,
            )
        })
        .collect()
}

/// `cos((2x + 1) u pi / 16) * C(u) / 2` for the 8-point IDCT (A.3.3), `[x][u]`.
fn idct_table() -> [[f64; 8]; 8] {
    let mut t = [[0.0; 8]; 8];
    for (x, row) in t.iter_mut().enumerate() {
        for (u, v) in row.iter_mut().enumerate() {
            let c = if u == 0 {
                std::f64::consts::FRAC_1_SQRT_2
            } else {
                1.0
            };
            *v = c / 2.0 * ((2 * x + 1) as f64 * u as f64 * std::f64::consts::PI / 16.0).cos();
        }
    }
    t
}

/// Exact separable IDCT of a dequantised block (natural order), row-major output.
fn idct(blk: &[i16; 64], q: &[u16; 64], t: &[[f64; 8]; 8], out: &mut [f64; 64]) {
    let mut tmp = [0.0f64; 64];
    // Columns: tmp[y][u] = sum_v t[y][v] * F[v][u].
    for u in 0..8 {
        let col: [f64; 8] = std::array::from_fn(|v| blk[v * 8 + u] as f64 * q[v * 8 + u] as f64);
        if col.iter().all(|&c| c == 0.0) {
            continue;
        }
        for y in 0..8 {
            tmp[y * 8 + u] = (0..8).map(|v| t[y][v] * col[v]).sum();
        }
    }
    for y in 0..8 {
        let row = &tmp[y * 8..y * 8 + 8];
        for x in 0..8 {
            out[y * 8 + x] = (0..8).map(|u| t[x][u] * row[u]).sum();
        }
    }
}

/// Decodes one frame to per-component samples (non-differential) or differences
/// (differential), row-major at the component's size. DCT frames come with [`FRAC`] fractional
/// bits, lossless frames as whole samples.
fn decode_frame(p: &Parsed, data: &[u8]) -> Result<Vec<Vec<i32>>> {
    let f = &p.frame;
    let dims = comp_dims(p);
    if f.lossless {
        if f.arithmetic {
            return Err(JpegError::Unsupported(
                "arithmetic-coded lossless frames in a hierarchical image",
            ));
        }
        if f.differential {
            return diff_lossless(p, data, &dims);
        }
        let s = crate::lossless::decode_samples(p, data, f.height as usize, 1, None)?;
        if s.truncated {
            return Err(JpegError::Truncated("entropy-coded data"));
        }
        return dims
            .iter()
            .zip(&s.comps)
            .map(|(&(_, w, h), c)| {
                let mut out = alloc(w * h, 0i32)?;
                for (y, row) in out.chunks_exact_mut(w).enumerate() {
                    for (o, &v) in row.iter_mut().zip(&c.data[y * c.stride..][..w]) {
                        *o = ((v as u32) << c.pt) as u16 as i32;
                    }
                }
                Ok(out)
            })
            .collect();
    }
    let coefs = crate::transform::coefficients(p, data)?;
    let t = idct_table();
    let one = (1i32 << FRAC) as f64;
    let shift = (1i32 << (f.precision - 1)) << FRAC;
    let max = ((1i32 << f.precision) - 1) << FRAC;
    let mut out = Vec::with_capacity(dims.len());
    for (&(_, w, h), cc) in dims.iter().zip(&coefs) {
        let mut plane = alloc(w * h, 0i32)?;
        let mut px = [0f64; 64];
        for by in 0..h.div_ceil(8) {
            for bx in 0..w.div_ceil(8) {
                let i = (by * cc.stride_blocks as usize + bx) * 64;
                let blk: &[i16; 64] = cc.coefs[i..i + 64].try_into().unwrap();
                idct(blk, &cc.quant, &t, &mut px);
                for r in 0..8.min(h - by * 8) {
                    let row = &mut plane[(by * 8 + r) * w + bx * 8..];
                    for (x, o) in row.iter_mut().enumerate().take(8.min(w - bx * 8)) {
                        let v = (px[r * 8 + x] * one).round() as i32;
                        *o = if f.differential {
                            v
                        } else {
                            (v + shift).clamp(0, max)
                        };
                    }
                }
            }
        }
        out.push(plane);
    }
    Ok(out)
}

/// A differential lossless frame (Huffman): every difference is coded without prediction and
/// shifted left by the scan's point transform (J.2.3.2).
fn diff_lossless(p: &Parsed, data: &[u8], dims: &[(u8, usize, usize)]) -> Result<Vec<Vec<i32>>> {
    let f = &p.frame;
    let (mx, my) = (
        (f.width as usize).div_ceil(f.max_h as usize),
        (f.height as usize).div_ceil(f.max_v as usize),
    );
    // Padded to whole MCUs (interleaved scans write the padding too).
    let pads: Vec<(usize, usize)> = f
        .comps
        .iter()
        .map(|c| (mx * c.h as usize, my * c.v as usize))
        .collect();
    let mut planes: Vec<Vec<i32>> = pads
        .iter()
        .map(|&(w, h)| alloc(w * h, 0i32))
        .collect::<Result<_>>()?;
    for s in &p.scans {
        if s.ss != 0 {
            return Err(JpegError::Malformed(
                "differential lossless scan with a predictor",
            ));
        }
        let end = s.data_end.min(data.len());
        let mut br = BitReader {
            data: &data[..end],
            pos: s.data_start,
            buf: 0,
            bits: 0,
            marker_hit: false,
            zero_fill: 0,
        };
        let interleaved = s.comps.len() > 1;
        let (ux, uy) = if interleaved {
            (mx, my)
        } else {
            let (_, w, h) = dims[s.comps[0].ci];
            (w, h)
        };
        let ri = s.restart_interval;
        let mut left = ri;
        for y in 0..uy {
            for x in 0..ux {
                if ri != 0 {
                    if left == 0 {
                        br.restart();
                        left = ri;
                    }
                    left -= 1;
                }
                for sc in &s.comps {
                    let fc = &f.comps[sc.ci];
                    let t = s.dc[sc.td]
                        .as_deref()
                        .ok_or(JpegError::Malformed("missing lossless Huffman table"))?;
                    let (nh, nv) = if interleaved {
                        (fc.h as usize, fc.v as usize)
                    } else {
                        (1, 1)
                    };
                    let pw = pads[sc.ci].0;
                    for v in 0..nv {
                        for h in 0..nh {
                            br.ensure(32);
                            let ssss = br.decode_symbol(t) & 31;
                            let d = match ssss {
                                16 => 32768,
                                0 => 0,
                                s if s < 16 => {
                                    br.ensure(s);
                                    br.get_extend(s)
                                }
                                _ => return Err(JpegError::Malformed("lossless difference size")),
                            };
                            let (px, py) = if interleaved {
                                (x * nh + h, y * nv + v)
                            } else {
                                (x, y)
                            };
                            planes[sc.ci][py * pw + px] = d.wrapping_shl(s.al as u32);
                        }
                    }
                }
            }
            if br.overrun() {
                return Err(JpegError::Truncated("entropy-coded data"));
            }
        }
    }
    // Crop the MCU padding.
    Ok(planes
        .into_iter()
        .zip(pads)
        .zip(dims)
        .map(|((pl, (pw, _)), &(_, w, h))| {
            if pw == w {
                let mut pl = pl;
                pl.truncate(w * h);
                pl
            } else {
                (0..h)
                    .flat_map(|y| pl[y * pw..y * pw + w].to_vec())
                    .collect()
            }
        })
        .collect())
}

/// The J.1.1.2 upsampling filter (bilinear, truncating; right column and bottom line
/// replicated), horizontal first.
fn upsample(c: &Comp, (eh, ev): (bool, bool)) -> Result<Comp> {
    let (mut w, mut h, mut data) = (c.w, c.h, c.data.clone());
    if eh {
        let mut out = alloc(2 * w * h, 0i32)?;
        for (src, dst) in data.chunks_exact(w).zip(out.chunks_exact_mut(2 * w)) {
            for x in 0..w {
                let (a, b) = (src[x], src[(x + 1).min(w - 1)]);
                dst[2 * x] = a;
                dst[2 * x + 1] = (a + b) >> 1;
            }
        }
        w *= 2;
        data = out;
    }
    if ev {
        let mut out = alloc(2 * w * h, 0i32)?;
        for y in 0..h {
            let (a, b) = (&data[y * w..][..w], &data[(y + 1).min(h - 1) * w..][..w]);
            out[2 * y * w..][..w].copy_from_slice(a);
            for ((o, &a), &b) in out[(2 * y + 1) * w..][..w].iter_mut().zip(a).zip(b) {
                *o = (a + b) >> 1;
            }
        }
        h *= 2;
        data = out;
    }
    Ok(Comp {
        id: c.id,
        w,
        h,
        data,
    })
}

/// `c` cropped (or edge-extended) to `w` x `h`.
fn fit(c: Comp, w: usize, h: usize) -> Result<Vec<i32>> {
    if c.w == w && c.h >= h {
        let mut d = c.data;
        d.truncate(w * h);
        return Ok(d);
    }
    let mut out = alloc(w * h, 0i32)?;
    for y in 0..h {
        let src = &c.data[y.min(c.h - 1) * c.w..][..c.w];
        for (x, o) in out[y * w..][..w].iter_mut().enumerate() {
            *o = src[x.min(c.w - 1)];
        }
    }
    Ok(out)
}

/// The decoded area: size and origin in output pixels, and one plane per component.
pub(crate) struct Output {
    pub width: u32,
    pub height: u32,
    pub origin: (u32, u32),
    pub planes: Vec<Plane>,
}

/// Decodes the hierarchical image `p` (parsed from `data`) to its final components: whole
/// samples at the frame precision, each at its DHP size (row-major), in DHP order.
pub(crate) fn components(
    data: &[u8],
    p: &Parsed,
    cancel: Option<&AtomicBool>,
) -> Result<Vec<(Vec<u16>, usize, usize)>> {
    let cancelled = |c: Option<&AtomicBool>| c.is_some_and(|c| c.load(Ordering::Relaxed));
    let fin = &p.frame;
    let precision = fin.precision;
    let target = comp_dims(p);
    let total: u64 = target.iter().map(|&(_, w, h)| (w * h) as u64).sum();
    if total > MAX_SAMPLES {
        return Err(JpegError::TooLarge(total * 4));
    }
    let mut refs: Vec<Comp> = Vec::new();
    for job in split(data)? {
        if cancelled(cancel) {
            return Err(JpegError::Cancelled);
        }
        let mut fp = crate::marker::parse(&job.stream, true)?;
        fp.frame.differential = job.differential;
        if fp.frame.precision != precision {
            return Err(JpegError::Malformed(
                "frame precision differs from the DHP segment",
            ));
        }
        let dims = comp_dims(&fp);
        let samples: u64 = dims.iter().map(|&(_, w, h)| (w * h) as u64).sum();
        if samples > MAX_SAMPLES {
            return Err(JpegError::TooLarge(samples * 4));
        }
        let planes = decode_frame(&fp, &job.stream)?;
        let lossless = fp.frame.lossless;
        for ((id, w, h), plane) in dims.into_iter().zip(planes) {
            let slot = refs.iter().position(|c| c.id == id);
            let data = if job.differential {
                let r = slot.map(|i| &refs[i]).ok_or(JpegError::Malformed(
                    "differential frame without a reference component",
                ))?;
                let up = if job.exp.0 || job.exp.1 {
                    upsample(r, job.exp)?
                } else {
                    Comp {
                        id,
                        w: r.w,
                        h: r.h,
                        data: r.data.clone(),
                    }
                };
                let mut d = fit(up, w, h)?;
                for (o, &v) in d.iter_mut().zip(&plane) {
                    *o = if lossless {
                        // Whole samples (rounded down) plus the difference, modulo 2^16.
                        (((*o >> FRAC) + v) << FRAC) & WRAP
                    } else {
                        (*o + v) & WRAP
                    };
                }
                d
            } else if lossless {
                plane.into_iter().map(|v| v << FRAC).collect()
            } else {
                plane
            };
            let c = Comp { id, w, h, data };
            match slot {
                Some(i) => refs[i] = c,
                None => refs.push(c),
            }
        }
    }
    // Final components: DHP size, whole samples.
    let max = if precision >= 16 {
        u16::MAX as i32
    } else {
        (1i32 << precision) - 1
    };
    let mut full = Vec::with_capacity(target.len());
    for &(id, w, h) in &target {
        let i = refs
            .iter()
            .position(|c| c.id == id)
            .ok_or(JpegError::Malformed("DHP component never coded"))?;
        let r = &refs[i];
        let c = Comp {
            id,
            w: r.w,
            h: r.h,
            data: r.data.clone(),
        };
        let whole = |v: i32| -> u16 {
            // Whole samples modulo 2^16; DCT reconstructions may stray just below 0 or above
            // the maximum.
            let v = ((v + (1 << (FRAC - 1))) >> FRAC) as u16;
            if precision >= 16 {
                v
            } else {
                (v as i16 as i32).clamp(0, max) as u16
            }
        };
        full.push((fit(c, w, h)?.into_iter().map(whole).collect(), w, h));
    }
    Ok(full)
}

/// Decodes the hierarchical image `p` (parsed from `data`) to 8-bit component planes at
/// `opts.scale`, cropped to `opts.region`.
pub(crate) fn decode(data: &[u8], p: &Parsed, opts: &DecodeOptions) -> Result<Output> {
    let fin = &p.frame;
    let precision = fin.precision;
    let full: Vec<(Vec<u8>, usize, usize)> = components(data, p, opts.cancel)?
        .into_iter()
        .map(|(d, w, h)| {
            let d = if precision == 8 {
                d.into_iter().map(|v| v as u8).collect()
            } else {
                d.into_iter()
                    .map(|v| to_8bit(v, precision as u32))
                    .collect()
            };
            (d, w, h)
        })
        .collect();
    // Scale and region, in output pixels; the origin is aligned to whole chroma samples.
    let s = opts.scale.factor() as usize;
    let (hmax, vmax) = (fin.max_h as usize, fin.max_v as usize);
    let (x0, y0, x1, y1) = out_rect(
        fin.width as usize,
        fin.height as usize,
        opts.scale,
        opts.region,
    )?;
    let (x0, y0) = (x0 - x0 % hmax, y0 - y0 % vmax);
    let mut planes = Vec::with_capacity(full.len());
    for ((d, w, h), c) in full.into_iter().zip(&fin.comps) {
        let (ch, cv) = (c.h as usize, c.v as usize);
        let (sw, sh) = (w.div_ceil(s), h.div_ceil(s));
        let px0 = x0 * ch / hmax;
        let py0 = y0 * cv / vmax;
        let px1 = (x1 * ch).div_ceil(hmax).min(sw);
        let py1 = (y1 * cv).div_ceil(vmax).min(sh);
        let (pw, ph) = (px1 - px0, py1 - py0);
        let mut out = alloc(pw * ph, 0u8)?;
        for oy in 0..ph {
            let sy = (py0 + oy) * s;
            let ny = s.min(h - sy);
            for ox in 0..pw {
                let sx = (px0 + ox) * s;
                let nx = s.min(w - sx);
                let mut sum = 0u32;
                for yy in 0..ny {
                    for &v in &d[(sy + yy) * w + sx..][..nx] {
                        sum += v as u32;
                    }
                }
                let cnt = (nx * ny) as u32;
                out[oy * pw + ox] = ((sum + cnt / 2) / cnt) as u8;
            }
        }
        planes.push(Plane {
            width: pw as u32,
            height: ph as u32,
            stride: pw,
            data: out,
            h_samp: c.h as u8,
            v_samp: c.v as u8,
        });
    }
    Ok(Output {
        width: (x1 - x0) as u32,
        height: (y1 - y0) as u32,
        origin: (x0 as u32, y0 as u32),
        planes,
    })
}
