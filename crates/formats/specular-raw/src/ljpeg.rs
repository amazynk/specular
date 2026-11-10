//! Lossless JPEG (ITU T.81 process 14, "LJ92") decoder, as used by DNG and Canon CR2.
//!
//! Supports 2..=16-bit precision, 1..=4 interleaved components with 1x1 sampling, predictors
//! 1..=7, point transform and restart intervals. Output is row-major with components interleaved
//! (`width * components` samples per row).

use crate::bits::BitReader;
use crate::error::{RawError, Result};
use crate::huffman::Huffman;

/// Frame and scan parameters of a lossless JPEG stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lj92Header {
    pub width: u32,
    pub height: u32,
    pub components: u32,
    pub precision: u8,
    /// Predictor selection value (1..=7).
    pub predictor: u8,
    pub point_transform: u8,
    pub restart_interval: u16,
    /// Any component with sampling factors other than 1x1 (Canon sRAW/mRAW).
    pub subsampled: bool,
    /// Horizontal and vertical sampling factors of the first component.
    pub sampling: (u8, u8),
    /// Byte offset of the entropy-coded data.
    pub(crate) data_start: usize,
    /// Huffman table index per scan component (in frame order).
    pub(crate) tables: [u8; 4],
}

/// A DHT table as stored: code counts per length and the symbols.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct HuffSpec {
    pub(crate) counts: [u8; 16],
    pub(crate) symbols: [u8; 256],
    pub(crate) n: usize,
}

pub(crate) struct Parsed {
    pub(crate) header: Lj92Header,
    pub(crate) tables: [Option<HuffSpec>; 4],
}

fn be16(d: &[u8], p: usize) -> Result<usize> {
    d.get(p..p + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]) as usize)
        .ok_or(RawError::Malformed("lossless JPEG: truncated header"))
}

/// SOF3 fields: width, height, precision, component ids, subsampled, sampling of the first
/// component (horizontal, vertical).
type Frame = (u32, u32, u8, Vec<u8>, bool, (u8, u8));

pub(crate) fn parse(d: &[u8]) -> Result<Parsed> {
    parse_with(d, false)
}

/// Headers of a vendor stream that reuses the JPEG markers but codes its own way (Hasselblad,
/// Leaf): any predictor value is accepted.
pub(crate) fn parse_vendor(d: &[u8]) -> Result<Parsed> {
    parse_with(d, true)
}

fn parse_with(d: &[u8], any_predictor: bool) -> Result<Parsed> {
    if d.len() < 4 || d[0] != 0xff || d[1] != 0xd8 {
        return Err(RawError::Malformed("lossless JPEG: missing SOI"));
    }
    let mut p = 2;
    let mut tables: [Option<HuffSpec>; 4] = [None; 4];
    let mut frame: Option<Frame> = None;
    let mut restart = 0u16;
    loop {
        // Skip fill bytes / garbage up to the next marker.
        while p < d.len() && d[p] != 0xff {
            p += 1;
        }
        while p < d.len() && d[p] == 0xff {
            p += 1;
        }
        let Some(&m) = d.get(p) else {
            return Err(RawError::Malformed("lossless JPEG: no SOS"));
        };
        p += 1;
        if m == 0xd8 || (0xd0..=0xd7).contains(&m) || m == 0x01 {
            continue;
        }
        if m == 0xd9 {
            return Err(RawError::Malformed("lossless JPEG: EOI before SOS"));
        }
        let len = be16(d, p)?;
        if len < 2 || p + len > d.len() {
            return Err(RawError::Malformed("lossless JPEG: bad segment length"));
        }
        let seg = &d[p + 2..p + len];
        match m {
            0xc3 | 0xc7 => {
                if seg.len() < 6 {
                    return Err(RawError::Malformed("lossless JPEG: short SOF"));
                }
                let precision = seg[0];
                let height = u16::from_be_bytes([seg[1], seg[2]]) as u32;
                let width = u16::from_be_bytes([seg[3], seg[4]]) as u32;
                let nc = seg[5] as usize;
                if !(1..=4).contains(&nc) || seg.len() < 6 + 3 * nc {
                    return Err(RawError::Unsupported("lossless JPEG: component count"));
                }
                if !(2..=16).contains(&precision) {
                    return Err(RawError::Malformed("lossless JPEG: precision"));
                }
                let ids: Vec<u8> = (0..nc).map(|i| seg[6 + 3 * i]).collect();
                let sub = (0..nc).any(|i| seg[7 + 3 * i] != 0x11);
                let sampling = (seg[7] >> 4, seg[7] & 15);
                frame = Some((width, height, precision, ids, sub, sampling));
            }
            0xc0..=0xcf if m != 0xc4 && m != 0xc8 && m != 0xcc => {
                return Err(RawError::Unsupported("not a lossless JPEG frame"));
            }
            0xc4 => {
                let mut q = 0;
                while q + 17 <= seg.len() {
                    let class_id = seg[q];
                    let counts: [u8; 16] = seg[q + 1..q + 17].try_into().unwrap_or_default();
                    let n: usize = counts.iter().map(|&c| c as usize).sum();
                    let syms = seg
                        .get(q + 17..q + 17 + n)
                        .ok_or(RawError::Malformed("lossless JPEG: short DHT"))?;
                    if class_id >> 4 == 0 && (class_id & 15) < 4 && n <= 256 {
                        let mut symbols = [0u8; 256];
                        symbols[..n].copy_from_slice(syms);
                        tables[(class_id & 15) as usize] = Some(HuffSpec { counts, symbols, n });
                    }
                    q += 17 + n;
                }
            }
            0xdd => {
                restart = be16(d, p + 2)? as u16;
            }
            0xda => {
                let (width, height, precision, ids, subsampled, sampling) =
                    frame.ok_or(RawError::Malformed("lossless JPEG: SOS before SOF"))?;
                let ns = *seg
                    .first()
                    .ok_or(RawError::Malformed("lossless JPEG: short SOS"))?
                    as usize;
                if ns != ids.len() || seg.len() < 1 + 2 * ns + 3 {
                    return Err(RawError::Unsupported("lossless JPEG: non-interleaved scan"));
                }
                let mut tsel = [0u8; 4];
                for i in 0..ns {
                    let cid = seg[1 + 2 * i];
                    let fi = ids.iter().position(|&c| c == cid).unwrap_or(i);
                    if let Some(t) = tsel.get_mut(fi) {
                        *t = (seg[2 + 2 * i] >> 4) & 3;
                    }
                }
                let predictor = seg[1 + 2 * ns];
                let pt = seg[3 + 2 * ns] & 15;
                if !any_predictor && !(1..=7).contains(&predictor) {
                    return Err(RawError::Unsupported("lossless JPEG: predictor"));
                }
                if pt >= precision {
                    return Err(RawError::Malformed("lossless JPEG: point transform"));
                }
                let header = Lj92Header {
                    width,
                    height,
                    components: ids.len() as u32,
                    precision,
                    predictor,
                    point_transform: pt,
                    restart_interval: restart,
                    subsampled,
                    sampling,
                    data_start: p + len,
                    tables: tsel,
                };
                return Ok(Parsed { header, tables });
            }
            _ => {}
        }
        p += len;
    }
}

/// Parse the headers only.
pub fn read_header(data: &[u8]) -> Result<Lj92Header> {
    Ok(parse(data)?.header)
}

/// Decode into `out` (at least `width * height * components` samples). Returns the header and
/// whether the entropy-coded data was truncated/corrupt (missing samples are left as decoded
/// from zero bits).
#[allow(clippy::needless_range_loop)] // `c` indexes the tables and the interleaved row.
pub fn decode_into(data: &[u8], out: &mut [u16]) -> Result<(Lj92Header, bool)> {
    let Parsed { header: h, tables } = parse(data)?;
    if h.subsampled {
        return Err(RawError::Unsupported(
            "lossless JPEG: subsampled components (sRAW)",
        ));
    }
    let nc = h.components as usize;
    let w = h.width as usize;
    let rowlen = w * nc;
    let n = rowlen * h.height as usize;
    if out.len() < n {
        return Err(RawError::Malformed(
            "lossless JPEG: output buffer too small",
        ));
    }
    if n == 0 {
        return Ok((h, false));
    }
    let mut built: [Option<Huffman>; 4] = [None, None, None, None];
    for (b, t) in built.iter_mut().zip(&tables) {
        *b = t.and_then(|t| Huffman::new(&t.counts, &t.symbols[..t.n], true));
    }
    let mut hts: Vec<&Huffman> = Vec::with_capacity(nc);
    for c in 0..nc {
        hts.push(
            built[h.tables[c] as usize]
                .as_ref()
                .ok_or(RawError::Malformed("lossless JPEG: missing Huffman table"))?,
        );
    }
    let entropy = data.get(h.data_start..).unwrap_or(&[]);
    let mut br = BitReader::new(entropy, true);
    let pt = h.point_transform as u32;
    let initial = 1i32 << (h.precision as u32 - pt - 1);
    let mask = (1u32 << (h.precision as u32 - pt).min(16)) as i32 - 1;
    let ri = h.restart_interval as usize;
    let out = &mut out[..n];

    let mut mcus_left = if ri > 0 { ri } else { usize::MAX };
    // The first line (and the first line after a restart) is predicted from the left only.
    let mut first_line = true;
    for row in 0..h.height as usize {
        let (prev, cur) = out.split_at_mut(row * rowlen);
        let cur = &mut cur[..rowlen];
        let above = if row > 0 {
            &prev[(row - 1) * rowlen..]
        } else {
            &prev[..0]
        };
        if row > 0 {
            first_line = false;
        }
        if ri == 0 && (row == 0 || h.predictor == 1) {
            // Fast paths: the first column is predicted from above (or the initial value on
            // line 0), the others from the left.
            match nc {
                1 => decode_row_left::<1>(&mut br, &hts, cur, above, initial, mask),
                2 => decode_row_left::<2>(&mut br, &hts, cur, above, initial, mask),
                3 => decode_row_left::<3>(&mut br, &hts, cur, above, initial, mask),
                _ => decode_row_left::<4>(&mut br, &hts, cur, above, initial, mask),
            }
            continue;
        }
        for col in 0..w {
            let mut reset = row == 0 && col == 0;
            if ri > 0 {
                if mcus_left == 0 {
                    br.restart();
                    mcus_left = ri;
                    reset = true;
                    first_line = true;
                }
                mcus_left -= 1;
            }
            for c in 0..nc {
                let i = col * nc + c;
                let pred = if reset {
                    initial
                } else if col == 0 {
                    above[i] as i32
                } else if first_line {
                    cur[i - nc] as i32
                } else {
                    let ra = cur[i - nc] as i32;
                    let rb = above[i] as i32;
                    let rc = above[i - nc] as i32;
                    match h.predictor {
                        1 => ra,
                        2 => rb,
                        3 => rc,
                        4 => ra + rb - rc,
                        5 => ra + ((rb - rc) >> 1),
                        6 => rb + ((ra - rc) >> 1),
                        _ => (ra + rb) >> 1,
                    }
                };
                let d = hts[c].diff(&mut br);
                cur[i] = ((pred + d) & mask) as u16;
            }
        }
    }
    if pt > 0 {
        for v in out.iter_mut() {
            *v <<= pt;
        }
    }
    Ok((h, br.overrun()))
}

#[inline(always)]
fn decode_row_left<const NC: usize>(
    br: &mut BitReader,
    hts: &[&Huffman],
    cur: &mut [u16],
    above: &[u16],
    initial: i32,
    mask: i32,
) {
    let mut pred = [0i32; NC];
    for (c, p) in pred.iter_mut().enumerate() {
        *p = above.get(c).map_or(initial, |&v| v as i32);
    }
    let hts: [&Huffman; NC] = std::array::from_fn(|c| hts[c]);
    for px in cur.chunks_exact_mut(NC) {
        for c in 0..NC {
            let v = (pred[c] + hts[c].diff(br)) & mask;
            px[c] = v as u16;
            pred[c] = v;
        }
    }
}

/// Layout of a subsampled (YCbCr) lossless JPEG frame: the first component (luma) sampled
/// `h x v` (2x1 or 2x2), then two chroma components sampled 1x1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Subsampled {
    pub h: usize,
    pub v: usize,
    /// MCUs per row and MCU rows.
    pub cols: usize,
    pub rows: usize,
}

impl Subsampled {
    /// Samples per MCU: `h * v` luma (row-major within the block), then Cb and Cr.
    pub fn clrs(&self) -> usize {
        self.h * self.v + 2
    }
}

/// The layout of a subsampled frame, or `None` when the frame is not one this decoder takes
/// (three components, the first sampled 2x1 or 2x2, the others 1x1).
pub(crate) fn subsampled_layout(h: &Lj92Header) -> Option<Subsampled> {
    let (sh, sv) = (h.sampling.0 as usize, h.sampling.1 as usize);
    (h.subsampled && h.components == 3 && sh == 2 && (sv == 1 || sv == 2)).then(|| Subsampled {
        h: sh,
        v: sv,
        cols: (h.width as usize).div_ceil(sh),
        rows: (h.height as usize).div_ceil(sv),
    })
}

/// How the samples of a subsampled frame are predicted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubPrediction {
    /// Canon sRAW/mRAW (predictor 1 only): the luma samples of an MCU line form one chain, each
    /// predicted from the luma sample decoded before it; chroma from the same component one MCU
    /// to the left; the first MCU of a line from the same sample of the first MCU of the line
    /// above.
    Canon,
    /// ITU T.81: each component is predicted within its own sample grid (Sony lossless M/S).
    Standard,
}

/// Decode a subsampled frame (Canon sRAW/mRAW, Sony lossless M/S) into its MCU sample stream:
/// `rows` lines of `cols * clrs` samples.
pub(crate) fn decode_subsampled(
    data: &[u8],
    max_samples: u64,
    mode: SubPrediction,
) -> Result<(Lj92Header, Subsampled, Vec<u16>, bool)> {
    let Parsed { header: h, tables } = parse(data)?;
    let lay =
        subsampled_layout(&h).ok_or(RawError::Unsupported("lossless JPEG: subsampling layout"))?;
    let predictor_ok = match mode {
        SubPrediction::Canon => h.predictor == 1,
        SubPrediction::Standard => (1..=7).contains(&h.predictor),
    };
    if !predictor_ok || h.restart_interval != 0 {
        return Err(RawError::Unsupported(
            "lossless JPEG: subsampled frame with this predictor",
        ));
    }
    let clrs = lay.clrs();
    let line = lay.cols * clrs;
    let n = line as u64 * lay.rows as u64;
    if n > max_samples {
        return Err(RawError::TooLarge(n));
    }
    let mut built: [Option<Huffman>; 4] = [None, None, None, None];
    for (b, t) in built.iter_mut().zip(&tables) {
        *b = t.and_then(|t| Huffman::new(&t.counts, &t.symbols[..t.n], true));
    }
    let table = |c: usize| {
        built[h.tables[c] as usize]
            .as_ref()
            .ok_or(RawError::Malformed("lossless JPEG: missing Huffman table"))
    };
    let ny = lay.h * lay.v;
    let hts: Vec<&Huffman> = (0..clrs)
        .map(|k| table(if k < ny { 0 } else { k - ny + 1 }))
        .collect::<Result<_>>()?;
    let mut out = vec![0u16; n as usize];
    if n == 0 {
        return Ok((h, lay, out, false));
    }
    let mut br = BitReader::new(data.get(h.data_start..).unwrap_or(&[]), true);
    let pt = h.point_transform as u32;
    let mask = (1u32 << (h.precision as u32 - pt).min(16)) as i32 - 1;
    let first = 1i32 << (h.precision as u32 - pt - 1);
    if mode == SubPrediction::Standard {
        standard_subsampled(&mut out, lay, &hts, &mut br, h.predictor, first, mask);
        if pt > 0 {
            for v in out.iter_mut() {
                *v <<= pt;
            }
        }
        return Ok((h, lay, out, br.overrun()));
    }
    let mut vpred = vec![first; clrs];
    for row in out.chunks_exact_mut(line) {
        let mut luma = 0i32;
        for col in 0..lay.cols {
            for k in 0..clrs {
                let i = col * clrs + k;
                let pred = if k < ny && (col > 0 || k > 0) {
                    luma
                } else if col > 0 {
                    row[i - clrs] as i32
                } else {
                    vpred[k]
                };
                let v = (pred + hts[k].diff(&mut br)) & mask;
                if col == 0 {
                    vpred[k] = v;
                }
                if k < ny {
                    luma = v;
                }
                row[i] = v as u16;
            }
        }
    }
    if pt > 0 {
        for v in out.iter_mut() {
            *v <<= pt;
        }
    }
    Ok((h, lay, out, br.overrun()))
}

/// T.81 prediction of a subsampled frame into the MCU stream `out`: each component predicts
/// from its own neighbours (left `a`, above `b`, above-left `c`); the first line from the
/// left, the first column from above.
fn standard_subsampled(
    out: &mut [u16],
    lay: Subsampled,
    hts: &[&Huffman],
    br: &mut BitReader,
    predictor: u8,
    first: i32,
    mask: i32,
) {
    let clrs = lay.clrs();
    let line = lay.cols * clrs;
    let ny = lay.h * lay.v;
    // Stream index of component sample (y, x): luma in a (rows*v) x (cols*h) grid, chroma
    // `ny + j` in a rows x cols grid.
    let at = |k: usize, y: usize, x: usize| -> usize {
        if k < ny {
            (y / lay.v) * line + (x / lay.h) * clrs + (y % lay.v) * lay.h + x % lay.h
        } else {
            y * line + x * clrs + k
        }
    };
    for r in 0..lay.rows {
        for c in 0..lay.cols {
            for k in 0..clrs {
                let (y, x) = if k < ny {
                    (r * lay.v + k / lay.h, c * lay.h + k % lay.h)
                } else {
                    (r, c)
                };
                let kk = if k < ny { 0 } else { k };
                let pred = match (y, x) {
                    (0, 0) => first,
                    (0, _) => out[at(kk, y, x - 1)] as i32,
                    (_, 0) if k < ny && y % lay.v == 0 => out[at(kk, y - lay.v, x)] as i32,
                    (_, 0) => out[at(kk, y - 1, x)] as i32,
                    _ => {
                        let a = out[at(kk, y, x - 1)] as i32;
                        let b = out[at(kk, y - 1, x)] as i32;
                        let cc = out[at(kk, y - 1, x - 1)] as i32;
                        match predictor {
                            1 => a,
                            2 => b,
                            3 => cc,
                            4 => a + b - cc,
                            5 => a + ((b - cc) >> 1),
                            6 => b + ((a - cc) >> 1),
                            _ => (a + b) >> 1,
                        }
                    }
                };
                let v = (pred + hts[k].diff(br)) & mask;
                out[r * line + c * clrs + k] = v as u16;
            }
        }
    }
}

/// Decode into a new buffer, refusing frames with more than `max_samples` samples.
pub fn decode(data: &[u8], max_samples: u64) -> Result<(Lj92Header, Vec<u16>, bool)> {
    let h = read_header(data)?;
    let n = h.width as u64 * h.height as u64 * h.components as u64;
    if n > max_samples {
        return Err(RawError::TooLarge(n));
    }
    let mut out = vec![0u16; n as usize];
    let (h, t) = decode_into(data, &mut out)?;
    Ok((h, out, t))
}
