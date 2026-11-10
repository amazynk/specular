//! Pictures, slices and their coefficients (RDD 36 §5.3–§7).
//!
//! A slice codes each component separately: the DC coefficients of all its blocks first
//! (the first one absolute, the rest as differences whose codebook follows the previous
//! difference), then the AC coefficients interleaved across the blocks, position by
//! position in scan order, as (run, level, sign) with codebooks that adapt to the previous
//! run and level. Alpha is coded in raster order as differences and runs.

use rayon::prelude::*;

use crate::bits::Bits;
use crate::idct::{self, Arith};
use crate::{Alpha, Chroma, Frame, Header, Interlace, Options, Result, bad};

/// The progressive block scan (RDD 36 Figure 4): scan index to raster position.
pub const PROGRESSIVE_SCAN: [u8; 64] = [
    0, 1, 8, 9, 2, 3, 10, 11, 16, 17, 24, 25, 18, 19, 26, 27, 4, 5, 12, 20, 13, 6, 7, 14, 21, 28,
    29, 22, 15, 23, 30, 31, 32, 33, 40, 48, 41, 34, 35, 42, 49, 56, 57, 50, 43, 36, 37, 44, 51, 58,
    59, 52, 45, 38, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// The interlaced scan: the progressive one transposed (field blocks are taller than wide
/// in frame terms, so the scan favours vertical frequencies).
const INTERLACED_SCAN: [u8; 64] = {
    let mut t = [0u8; 64];
    let mut i = 0;
    while i < 64 {
        let p = PROGRESSIVE_SCAN[i];
        t[i] = (p % 8) * 8 + p / 8;
        i += 1;
    }
    t
};

/// A scan with each raster position transposed (`row * 8 + col` → `col * 8 + row`): the
/// coefficients are stored the way the transform reads them.
const fn transposed(scan: &[u8; 64]) -> [u8; 64] {
    let mut t = [0u8; 64];
    let mut i = 0;
    while i < 64 {
        t[i] = (scan[i] % 8) * 8 + scan[i] / 8;
        i += 1;
    }
    t
}

const PROGRESSIVE_T: [u8; 64] = transposed(&PROGRESSIVE_SCAN);
pub(crate) const INTERLACED_T: [u8; 64] = transposed(&INTERLACED_SCAN);

/// Codebook of the first DC coefficient.
const FIRST_DC: u8 = 0xB8;
/// Codebooks of the DC differences, by the previous difference's codeword (capped at 6).
const DC_CODEBOOK: [u8; 7] = [0x04, 0x28, 0x28, 0x4D, 0x4D, 0x70, 0x70];
/// Codebooks of the AC runs, by the previous run (capped at 15).
const RUN_CODEBOOK: [u8; 16] = [
    0x06, 0x06, 0x05, 0x05, 0x04, 0x29, 0x29, 0x29, 0x29, 0x28, 0x28, 0x28, 0x28, 0x28, 0x28, 0x4C,
];
/// Codebooks of the AC levels, by the previous level (capped at 9).
const LEVEL_CODEBOOK: [u8; 10] = [0x04, 0x0A, 0x05, 0x06, 0x04, 0x28, 0x28, 0x28, 0x28, 0x4C];

/// One slice: where it is in its picture and where its bytes are.
#[derive(Clone, Copy, Debug)]
struct Slice {
    mb_x: u32,
    mb_count: u32,
    offset: usize,
    size: usize,
}

/// One picture (the frame, or a field): its slices by macroblock row.
struct Picture {
    rows: Vec<Vec<Slice>>,
    /// Frame line of its first line (0 or 1 for a field) and the step between its lines.
    first_line: usize,
}

/// The plane geometry of a frame.
#[derive(Clone, Copy)]
struct Geometry {
    mb_w: u32,
    /// Macroblock rows per picture.
    mb_h: u32,
    /// 1 progressive, 2 interlaced.
    fields: usize,
    chroma_shift: u32,
    /// Luma samples per padded row.
    stride: usize,
    cstride: usize,
}

fn parse_pictures(frame: &[u8], h: &Header, g: &Geometry) -> Result<Vec<Picture>> {
    let mut at = h.pictures_at;
    let mut out = Vec::with_capacity(g.fields);
    for p in 0..g.fields {
        let ph = frame.get(at..at + 8).ok_or_else(|| bad("picture header"))?;
        let hdr = usize::from(ph[0] >> 3);
        let size = u32::from_be_bytes([ph[1], ph[2], ph[3], ph[4]]) as usize;
        let count = usize::from(u16::from_be_bytes([ph[5], ph[6]]));
        let log2_slice = (ph[7] >> 4) & 3;
        if hdr < 8 || size < hdr || at + size > frame.len() {
            return Err(bad("picture size"));
        }
        let pic = &frame[at..at + size];
        let index_at = hdr;
        let mut data_at = index_at + 2 * count;
        if data_at > pic.len() {
            return Err(bad("slice index"));
        }
        let mut rows = Vec::with_capacity(g.mb_h as usize);
        let mut k = 0;
        for _ in 0..g.mb_h {
            let mut row = Vec::new();
            let mut mb_x = 0;
            let mut n = 1u32 << log2_slice;
            while mb_x < g.mb_w {
                while g.mb_w - mb_x < n {
                    n >>= 1;
                }
                if k >= count {
                    return Err(bad("too few slices"));
                }
                let s = usize::from(u16::from_be_bytes([
                    pic[index_at + 2 * k],
                    pic[index_at + 2 * k + 1],
                ]));
                if data_at + s > pic.len() {
                    return Err(bad("slice past the picture"));
                }
                row.push(Slice {
                    mb_x,
                    mb_count: n,
                    offset: at + data_at,
                    size: s,
                });
                data_at += s;
                mb_x += n;
                k += 1;
            }
            rows.push(row);
        }
        if k != count {
            return Err(bad("slice count"));
        }
        let first_line = match (h.interlace, p) {
            (Interlace::Progressive, _) => 0,
            (Interlace::TopFirst, 0) | (Interlace::BottomFirst, 1) => 0,
            _ => 1,
        };
        out.push(Picture { rows, first_line });
        at += size;
    }
    Ok(out)
}

/// The rows of one macroblock row band of the frame (16 lines, or 32 when interlaced), as
/// little-endian `u16` samples.
struct Band<'a> {
    index: usize,
    y: &'a mut [u8],
    cb: &'a mut [u8],
    cr: &'a mut [u8],
    a: Option<&'a mut [u8]>,
}

/// Per-thread working memory.
struct Scratch {
    blocks: Vec<[i32; 64]>,
    alpha: Vec<u16>,
}

impl Scratch {
    fn new() -> Scratch {
        Scratch {
            blocks: vec![[0; 64]; 32],
            alpha: vec![0; 16 * 16 * 8],
        }
    }
}

/// Where a component's blocks go within its band.
struct Target<'a> {
    plane: &'a mut [u8],
    /// Samples per line.
    stride: usize,
    /// Line step (2 for a field).
    step: usize,
    first_line: usize,
}

pub fn decode_frame(frame: &[u8], h: Header, o: &Options) -> Result<Frame> {
    let fields = if h.interlace == Interlace::Progressive {
        1
    } else {
        2
    };
    let mb_w = h.width.div_ceil(16);
    let mb_h = h.height.div_ceil(16 * fields as u32);
    let chroma_shift = u32::from(h.chroma == Chroma::Yuv422);
    let stride = mb_w as usize * 16;
    let cstride = stride >> chroma_shift;
    let g = Geometry {
        mb_w,
        mb_h,
        fields,
        chroma_shift,
        stride,
        cstride,
    };
    let pictures = parse_pictures(frame, &h, &g)?;
    let lines = mb_h as usize * 16 * fields;
    let band_lines = 16 * fields;
    let with_alpha = o.alpha && h.alpha != Alpha::None;
    // The planes, sized from the header (up to 512 MiB at 8192² 4:4:4 with alpha), and the
    // largest plane cropped out of them: reserved from the memory budget for the decode.
    let wide = stride * (1 + usize::from(with_alpha)) + 2 * cstride;
    let cropped = if h.width as usize == stride {
        0
    } else {
        h.width as usize * h.height as usize
    };
    let _held = specular_core::budget::reserve_n((wide * lines + cropped) as u64, 2)?;
    let mut y = vec![0u8; 2 * stride * lines];
    let mut cb = vec![0u8; 2 * cstride * lines];
    let mut cr = vec![0u8; 2 * cstride * lines];
    let mut a = with_alpha.then(|| vec![0u8; 2 * stride * lines]);
    let mut bands: Vec<Band<'_>> = {
        let ys = y.chunks_mut(2 * stride * band_lines);
        let cbs = cb.chunks_mut(2 * cstride * band_lines);
        let crs = cr.chunks_mut(2 * cstride * band_lines);
        let mut alphas = a.as_mut().map(|a| a.chunks_mut(2 * stride * band_lines));
        ys.zip(cbs)
            .zip(crs)
            .enumerate()
            .map(|(index, ((y, cb), cr))| Band {
                index,
                y,
                cb,
                cr,
                a: alphas.as_mut().and_then(|it| it.next()),
            })
            .collect()
    };
    let arith = match h.chroma {
        Chroma::Yuv422 => Arith::B10,
        Chroma::Yuv444 => Arith::B12,
    };
    let depth = u32::from(h.bit_depth());
    let ctx = Ctx {
        frame,
        h: &h,
        g: &g,
        pictures: &pictures,
        arith: &arith,
        depth,
    };
    let run = |band: &mut Band<'_>| -> usize {
        let mut scratch = Scratch::new();
        let mut damaged = 0;
        for pic in ctx.pictures {
            for s in &pic.rows[band.index] {
                if ctx.slice(s, pic.first_line, band, &mut scratch).is_err() {
                    damaged += 1;
                    ctx.grey(s, pic.first_line, band);
                }
            }
        }
        damaged
    };
    let damaged_slices = if o.parallel {
        bands.par_iter_mut().map(run).sum()
    } else {
        bands.iter_mut().map(run).sum()
    };
    drop(bands);
    let (w, ht) = (h.width as usize, h.height as usize);
    let cw = (w + chroma_shift as usize) >> chroma_shift;
    let planes = [
        crop(y, stride, w, ht),
        crop(cb, cstride, cw, ht),
        crop(cr, cstride, cw, ht),
    ];
    let alpha = a.map(|a| crop(a, stride, w, ht));
    Ok(Frame {
        width: h.width,
        height: h.height,
        bit_depth: h.bit_depth(),
        chroma: h.chroma,
        header: h,
        planes,
        alpha,
        damaged_slices,
    })
}

/// The top-left `w`×`h` samples of a plane `stride` samples wide (in place when only rows
/// are cut).
fn crop(mut p: Vec<u8>, stride: usize, w: usize, h: usize) -> Vec<u8> {
    if w == stride {
        p.truncate(2 * w * h);
        return p;
    }
    let mut out = Vec::with_capacity(w * h * 2);
    for row in p.chunks_exact(2 * stride).take(h) {
        out.extend_from_slice(&row[..2 * w]);
    }
    out
}

/// Writes `v` as little-endian samples over `dst`.
#[inline(always)]
fn store(dst: &mut [u8], v: &[u16]) {
    for (d, v) in dst.chunks_exact_mut(2).zip(v) {
        d.copy_from_slice(&v.to_le_bytes());
    }
}

struct Ctx<'a> {
    frame: &'a [u8],
    h: &'a Header,
    g: &'a Geometry,
    pictures: &'a [Picture],
    arith: &'a Arith,
    depth: u32,
}

impl Ctx<'_> {
    /// Fills a slice that failed with mid-grey.
    fn grey(&self, s: &Slice, first_line: usize, band: &mut Band<'_>) {
        let mid = [1u16 << (self.depth - 1); 16 * 8];
        let x0 = s.mb_x as usize * 16;
        let n = s.mb_count as usize * 16;
        let cs = self.g.chroma_shift as usize;
        let step = self.g.fields;
        for l in 0..16 {
            let line = first_line + l * step;
            store(&mut band.y[2 * (line * self.g.stride + x0)..], &mid[..n]);
            for p in [&mut *band.cb, &mut *band.cr] {
                store(
                    &mut p[2 * (line * self.g.cstride + (x0 >> cs))..],
                    &mid[..n >> cs],
                );
            }
        }
    }

    fn slice(
        &self,
        s: &Slice,
        first_line: usize,
        band: &mut Band<'_>,
        sc: &mut Scratch,
    ) -> Result<()> {
        let data = &self.frame[s.offset..s.offset + s.size];
        if data.len() < 6 {
            return Err(bad("slice header"));
        }
        let hdr = usize::from(data[0] >> 3);
        let qi = u32::from(data[1]);
        if hdr < 6 || hdr > data.len() || qi == 0 {
            return Err(bad("slice header"));
        }
        let qscale = if qi > 128 { 128 + (qi - 128) * 4 } else { qi } as i32;
        let y_size = usize::from(u16::from_be_bytes([data[2], data[3]]));
        let u_size = usize::from(u16::from_be_bytes([data[4], data[5]]));
        let rest = data.len() - hdr;
        let v_size = if hdr >= 8 {
            usize::from(u16::from_be_bytes([data[6], data[7]]))
        } else {
            rest.checked_sub(y_size + u_size)
                .ok_or_else(|| bad("component sizes"))?
        };
        if y_size + u_size + v_size > rest {
            return Err(bad("component sizes"));
        }
        let mut at = hdr;
        let mut part = |n: usize| {
            let p = &data[at..at + n];
            at += n;
            p
        };
        let (yd, ud, vd) = (part(y_size), part(u_size), part(v_size));
        let ad = &data[at..];
        let scan = if self.h.interlace == Interlace::Progressive {
            &PROGRESSIVE_T
        } else {
            &INTERLACED_T
        };
        // Transposed, like the coefficients.
        let qmat = |m: &[u8; 64]| -> [i32; 64] {
            std::array::from_fn(|i| i32::from(m[(i % 8) * 8 + i / 8]) * qscale)
        };
        let lq = qmat(&self.h.luma_q);
        let cq = qmat(&self.h.chroma_q);
        let x0 = s.mb_x as usize * 16;
        let n = s.mb_count as usize;
        let cs = self.g.chroma_shift as usize;
        let step = self.g.fields;
        let target = |plane, stride| Target {
            plane,
            stride,
            step,
            first_line,
        };
        let mut ty = target(&mut *band.y, self.g.stride);
        let mut tcb = target(&mut *band.cb, self.g.cstride);
        let mut tcr = target(&mut *band.cr, self.g.cstride);
        self.component(yd, n * 4, scan, &lq, sc, |b, blk, out| {
            let mb = b / 4;
            let (bx, by) = (b & 1, (b >> 1) & 1);
            out.put(blk, x0 + mb * 16 + bx * 8, by * 8, &mut ty);
        })?;
        let cbpm = if cs == 1 { 2 } else { 4 };
        for (d, t) in [(ud, &mut tcb), (vd, &mut tcr)] {
            self.component(d, n * cbpm, scan, &cq, sc, |b, blk, out| {
                let mb = b / cbpm;
                let (x, y) = if cs == 1 {
                    ((x0 >> 1) + mb * 8, (b & 1) * 8)
                } else {
                    // 4:4:4 chroma blocks go down, then across (luma's go across first).
                    (x0 + mb * 16 + ((b >> 1) & 1) * 8, (b & 1) * 8)
                };
                out.put(blk, x, y, t);
            })?;
        }
        if let Some(a) = band.a.as_deref_mut() {
            let bits = if self.h.alpha == Alpha::Bits16 { 16 } else { 8 };
            let w = n * 16;
            let vals = &mut sc.alpha[..w * 16];
            alpha(ad, bits, self.depth, vals);
            let step = self.g.fields;
            for (l, row) in vals.chunks_exact(w).enumerate() {
                let line = first_line + l * step;
                store(&mut a[2 * (line * self.g.stride + x0)..], row);
            }
        }
        Ok(())
    }

    /// Decodes one component's `blocks` blocks and hands each, dequantised and transformed,
    /// to `place`.
    fn component(
        &self,
        data: &[u8],
        blocks: usize,
        scan: &[u8; 64],
        qmat: &[i32; 64],
        sc: &mut Scratch,
        mut place: impl FnMut(usize, &[i32; 64], &Out<'_>),
    ) -> Result<()> {
        let blk = &mut sc.blocks[..blocks];
        let mut b = Bits::new(data);
        let r = coefficients(&mut b, blk, scan);
        let out = Out {
            qmat,
            arith: self.arith,
        };
        for (i, c) in blk.iter_mut().enumerate() {
            place(i, c, &out);
            *c = [0; 64];
        }
        r
    }
}

/// Dequantisation and transform for one component.
struct Out<'a> {
    qmat: &'a [i32; 64],
    arith: &'a Arith,
}

impl Out<'_> {
    fn put(&self, blk: &[i32; 64], x: usize, y: usize, t: &mut Target<'_>) {
        let mut px = [0u16; 64];
        idct::put(blk, self.qmat, self.arith, &mut px);
        for (r, row) in px.chunks_exact(8).enumerate() {
            let line = t.first_line + (y + r) * t.step;
            store(&mut t.plane[2 * (line * t.stride + x)..], row);
        }
    }
}

/// The DC and AC levels of one component's blocks, placed by `scan` within each block.
fn coefficients(b: &mut Bits<'_>, blk: &mut [[i32; 64]], scan: &[u8; 64]) -> Result<()> {
    let damaged = |_: &Bits<'_>| bad("coefficients");
    // DC: the first absolute, then differences.
    let code = b.codeword(FIRST_DC).ok_or_else(|| damaged(b))?;
    let mut dc = signed(code);
    blk[0][0] = dc;
    let mut code = 5u32;
    let mut sign = 0i32;
    for c in blk.iter_mut().skip(1) {
        code = b
            .codeword(DC_CODEBOOK[code.min(6) as usize])
            .ok_or_else(|| damaged(b))?;
        if code == 0 {
            sign = 0;
        } else {
            sign ^= -((code & 1) as i32);
        }
        dc += ((((code + 1) >> 1) as i32) ^ sign) - sign;
        c[0] = dc;
    }
    // AC: runs and levels across the blocks, position by position.
    let n = blk.len();
    let log2 = n.trailing_zeros();
    let mask = n - 1;
    let max = 64usize << log2;
    let mut pos = mask;
    let mut run = 4u32;
    let mut level = 2u32;
    loop {
        if b.only_padding() {
            break;
        }
        run = b
            .codeword(RUN_CODEBOOK[run.min(15) as usize])
            .ok_or_else(|| damaged(b))?;
        pos += run as usize + 1;
        if pos >= max {
            return Err(damaged(b));
        }
        level = b
            .codeword(LEVEL_CODEBOOK[level.min(9) as usize])
            .ok_or_else(|| damaged(b))?
            + 1;
        let neg = b.bit();
        let v = if neg { -(level as i32) } else { level as i32 };
        blk[pos & mask][usize::from(scan[pos >> log2])] = v;
    }
    Ok(())
}

/// Zig-zag mapping of a codeword to a signed value: 0, -1, 1, -2, 2...
#[inline(always)]
fn signed(code: u32) -> i32 {
    ((code >> 1) as i32) ^ -((code & 1) as i32)
}

/// Decodes a slice's alpha (`out.len()` values in raster order) and scales it to `depth`.
fn alpha(data: &[u8], bits: u32, depth: u32, out: &mut [u16]) {
    let mut b = Bits::new(data);
    let mask = (1u32 << bits) - 1;
    let scale = |v: u32| -> u16 {
        if bits == 16 {
            (v >> (16 - depth)) as u16
        } else {
            ((v << (depth - 8)) | (v >> (16 - depth))) as u16
        }
    };
    let mut val = mask;
    let mut i = 0;
    let n = out.len();
    while i < n {
        loop {
            let d = if b.bit() {
                b.read(bits) as i32
            } else {
                let c = b.read(if bits == 16 { 7 } else { 4 }) as i32;
                let m = (c + 2) >> 1;
                if c & 1 != 0 { -m } else { m }
            };
            val = (val as i32 + d) as u32 & mask;
            out[i] = scale(val);
            i += 1;
            if i >= n || b.left() == 0 || !b.bit() {
                break;
            }
        }
        if i >= n {
            break;
        }
        let mut r = b.read(4) as usize;
        if r == 0 {
            r = b.read(11) as usize;
        }
        let r = r.min(n - i);
        out[i..i + r].fill(scale(val));
        i += r;
        if b.left() == 0 && r == 0 {
            // Damaged: nothing more to read.
            out[i..].fill(scale(val));
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_are_permutations() {
        for scan in [&PROGRESSIVE_SCAN, &INTERLACED_SCAN] {
            let mut seen = [false; 64];
            for &p in scan.iter() {
                assert!(!seen[p as usize]);
                seen[p as usize] = true;
            }
        }
        assert_eq!(&INTERLACED_SCAN[..8], &[0, 8, 1, 9, 16, 24, 17, 25]);
    }

    #[test]
    fn signed_mapping() {
        assert_eq!([0, 1, 2, 3, 4].map(signed), [0, -1, 1, -2, 2],);
    }
}
