//! DNG strip/tile codecs other than lossless JPEG: deflate (integer and floating point samples,
//! with the TIFF horizontal and floating point predictors and the DNG X2/X4 variants),
//! uncompressed floating point, baseline ("lossy") JPEG and JPEG XL.
//!
//! Every block decodes on its own into the rows of the image it covers, so the caller runs
//! blocks in parallel. Floating point samples are scaled to 16 bits by [`DngTile::scale`]
//! (the container maps the white level to 65535).

use crate::error::{RawError, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Uncompressed (used for floating point samples only).
    Raw,
    /// Compression 8 (zlib).
    Deflate,
    /// Compression 34892: baseline JPEG, 8-bit.
    Jpeg,
    /// Compression 52546.
    Jxl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DngTile {
    pub kind: Kind,
    /// Bits per stored sample: 8 or 16 (integer, or packed `1..16` big-endian), 16, 24 or 32
    /// (floating point).
    pub bits: u8,
    pub float: bool,
    /// TIFF Predictor: 1 none, 2 horizontal, 3 floating point; DNG 34892/34893 horizontal X2/X4,
    /// 34894/34895 floating point X2/X4.
    pub predictor: u16,
    /// File byte order (integer samples and unpredicted floats).
    pub le: bool,
    /// Floating point samples: multiplier to the 16-bit range, as `f32::to_bits`.
    pub scale: u32,
}

impl DngTile {
    /// Whether this combination of sample format and predictor can be decoded.
    pub(crate) fn valid(&self) -> bool {
        match self.kind {
            Kind::Jpeg => matches!(self.bits, 8 | 12) && !self.float,
            Kind::Jxl => (1..=16).contains(&self.bits) && self.predictor == 1,
            Kind::Raw | Kind::Deflate if self.float => {
                matches!(self.bits, 16 | 24 | 32)
                    && matches!(self.predictor, 1 | 3 | 34894 | 34895)
                    && (self.kind == Kind::Deflate || self.predictor == 1)
                    && f32::from_bits(self.scale).is_finite()
            }
            Kind::Raw => false,
            Kind::Deflate => {
                (1..=16).contains(&self.bits)
                    && (matches!(self.predictor, 1)
                        || matches!(self.predictor, 2 | 34892 | 34893)
                            && matches!(self.bits, 8 | 16))
            }
        }
    }

    /// Bytes of one stored row of `samples` samples.
    fn row_bytes(&self, samples: usize) -> usize {
        (samples * self.bits as usize).div_ceil(8)
    }
}

/// Decodes one block (stored as `tw x th` pixels of `cpp` samples) into `rows`: the block's
/// rows inside the image (at most `th`), each `cw * cpp` samples (`cw <= tw`). Returns true
/// when the block was short or corrupt (missing samples stay zero).
pub(crate) fn decode_tile(
    src: &[u8],
    t: DngTile,
    tw: usize,
    th: usize,
    cpp: usize,
    rows: &mut [&mut [u16]],
) -> bool {
    let n = rows.len().min(th);
    let rows = &mut rows[..n];
    match t.kind {
        Kind::Raw | Kind::Deflate => samples(src, t, tw, cpp, rows),
        Kind::Jpeg => jpeg(src, tw, cpp, rows).unwrap_or(true),
        Kind::Jxl => jxl(src, cpp, rows).unwrap_or(true),
    }
}

/// The stored bytes of a block's `rows` rows of `rb` bytes: inflated for Deflate. Also
/// whether the block was short or corrupt; `None` when it cannot be read at all.
fn stored_bytes(src: &[u8], t: DngTile, need: usize) -> Option<(std::borrow::Cow<'_, [u8]>, bool)> {
    match t.kind {
        Kind::Deflate => {
            // zlib: CMF (deflate, window <= 32K), FLG (no preset dictionary); the Adler-32
            // trailer is not needed.
            let ok = src.len() >= 2
                && src[0] & 15 == 8
                && src[0] >> 4 <= 7
                && src[1] & 0x20 == 0
                && (u16::from_be_bytes([src[0], src[1]])).is_multiple_of(31);
            if !ok {
                return None;
            }
            match specular_png::inflate_raw_prefix(&src[2..], need) {
                Ok(v) => {
                    let short = v.len() < need;
                    Some((std::borrow::Cow::Owned(v), short))
                }
                Err(_) => None,
            }
        }
        _ => Some((std::borrow::Cow::Borrowed(src), src.len() < need)),
    }
}

/// One stored row of floating point samples with the predictor undone: the samples' bytes
/// in `row` (or `tmp`), and whether they are big-endian.
fn float_row<'a>(
    t: DngTile,
    s: &[u8],
    cpp: usize,
    tw: usize,
    row: &'a mut [u8],
    tmp: &'a mut [u8],
) -> (&'a [u8], bool) {
    let nb = t.bits as usize / 8;
    row.copy_from_slice(s);
    let stride = match t.predictor {
        3 => cpp,
        34894 => 2 * cpp,
        34895 => 4 * cpp,
        _ => 0,
    };
    if stride == 0 {
        return (row, !t.le);
    }
    // Undo the byte differencing, then gather the byte planes (most significant first) back
    // into samples.
    for i in stride..row.len() {
        row[i] = row[i].wrapping_add(row[i - stride]);
    }
    let m = tw * cpp;
    for i in 0..m {
        for b in 0..nb {
            tmp[i * nb + b] = row[b * m + i];
        }
    }
    (tmp, true)
}

fn samples(src: &[u8], t: DngTile, tw: usize, cpp: usize, rows: &mut [&mut [u16]]) -> bool {
    let rb = t.row_bytes(tw * cpp);
    let need = rb * rows.len();
    let Some((bytes, mut bad)) = stored_bytes(src, t, need) else {
        return true;
    };
    let mut row = vec![0u8; rb];
    let mut tmp = vec![0u8; rb];
    let scale = f32::from_bits(t.scale);
    for (y, out) in rows.iter_mut().enumerate() {
        let Some(s) = bytes.get(y * rb..(y + 1) * rb) else {
            bad = true;
            break;
        };
        let n = out.len();
        if t.float {
            let nb = t.bits as usize / 8;
            let (b, be) = float_row(t, s, cpp, tw, &mut row, &mut tmp);
            to_u16_float(b, nb, be, scale, out);
            continue;
        }
        let stride = match t.predictor {
            2 => cpp,
            34892 => 2 * cpp,
            34893 => 4 * cpp,
            _ => 0,
        };
        match t.bits {
            8 => {
                for (o, &b) in out.iter_mut().zip(s) {
                    *o = b as u16;
                }
            }
            16 => {
                for (o, b) in out.iter_mut().zip(s.chunks_exact(2)) {
                    *o = if t.le {
                        u16::from_le_bytes([b[0], b[1]])
                    } else {
                        u16::from_be_bytes([b[0], b[1]])
                    };
                }
            }
            b => {
                // Packed big-endian: unpack the whole stored row (the predictor is refused for
                // packed samples, so only the visible columns are needed).
                crate::decode::unpack(crate::container::Packing::Msb(b as u32), s, out);
            }
        }
        if stride > 0 {
            // Whole stored row: the prediction runs across the padding columns of a tile, but
            // only from left to right, so the visible columns need nothing beyond them.
            let mask = if t.bits == 8 { 0xff } else { 0xffff };
            for i in stride..n {
                out[i] = out[i].wrapping_add(out[i - stride]) & mask;
            }
        }
    }
    bad
}

/// As [`decode_tile`] for floating point blocks, into unclipped `f32` samples scaled by
/// [`DngTile::scale`] (NaN becomes 0). Returns true when the block was short or corrupt.
pub(crate) fn decode_tile_f32(
    src: &[u8],
    t: DngTile,
    tw: usize,
    th: usize,
    cpp: usize,
    rows: &mut [&mut [f32]],
) -> bool {
    let n = rows.len().min(th);
    let rows = &mut rows[..n];
    if !t.float || !matches!(t.kind, Kind::Raw | Kind::Deflate) {
        return true;
    }
    let rb = t.row_bytes(tw * cpp);
    let Some((bytes, mut bad)) = stored_bytes(src, t, rb * rows.len()) else {
        return true;
    };
    let mut row = vec![0u8; rb];
    let mut tmp = vec![0u8; rb];
    let scale = f32::from_bits(t.scale);
    let nb = t.bits as usize / 8;
    for (y, out) in rows.iter_mut().enumerate() {
        let Some(s) = bytes.get(y * rb..(y + 1) * rb) else {
            bad = true;
            break;
        };
        let (b, be) = float_row(t, s, cpp, tw, &mut row, &mut tmp);
        for (o, c) in out.iter_mut().zip(b.chunks_exact(nb)) {
            let v = float_sample(c, be) * scale;
            *o = if v.is_nan() { 0.0 } else { v };
        }
    }
    bad
}

/// One floating point sample of 2, 3 or 4 bytes.
fn float_sample(b: &[u8], be: bool) -> f32 {
    match b.len() {
        2 => {
            let h = if be {
                u16::from_be_bytes([b[0], b[1]])
            } else {
                u16::from_le_bytes([b[0], b[1]])
            };
            f16_to_f32(h)
        }
        3 => {
            let h = if be {
                (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32
            } else {
                (b[2] as u32) << 16 | (b[1] as u32) << 8 | b[0] as u32
            };
            f24_to_f32(h)
        }
        _ => {
            let a = [b[0], b[1], b[2], b[3]];
            if be {
                f32::from_be_bytes(a)
            } else {
                f32::from_le_bytes(a)
            }
        }
    }
}

/// Floating point samples (`nb` bytes each, big- or little-endian) to 16 bits.
fn to_u16_float(src: &[u8], nb: usize, be: bool, scale: f32, out: &mut [u16]) {
    for (o, b) in out.iter_mut().zip(src.chunks_exact(nb)) {
        let v = float_sample(b, be) * scale;
        // NaN and negative values become 0.
        *o = if v > 0.0 { v.min(65535.0) + 0.5 } else { 0.0 } as u16;
    }
}

/// IEEE 754 half precision.
pub(crate) fn f16_to_f32(h: u16) -> f32 {
    let s = (h as u32 & 0x8000) << 16;
    let e = (h as u32 >> 10) & 0x1f;
    let m = h as u32 & 0x3ff;
    let bits = match e {
        0 if m == 0 => s,
        0 => {
            // Subnormal: m * 2^-24.
            let v = m as f32 * f32::from_bits(0x3380_0000);
            return if s != 0 { -v } else { v };
        }
        31 => s | 0x7f80_0000 | m << 13,
        _ => s | (e + 112) << 23 | m << 13,
    };
    f32::from_bits(bits)
}

/// The DNG 24-bit float: sign, 7-bit exponent (bias 63), 16-bit mantissa.
pub(crate) fn f24_to_f32(h: u32) -> f32 {
    let s = (h & 0x80_0000) << 8;
    let e = (h >> 16) & 0x7f;
    let m = h & 0xffff;
    let bits = match e {
        0 if m == 0 => s,
        0 => {
            // Subnormal: m * 2^(-62 - 16).
            let v = m as f32 * 2f32.powi(-78);
            return if s != 0 { -v } else { v };
        }
        127 => s | 0x7f80_0000 | m << 7,
        _ => s | (e + 64) << 23 | m << 7,
    };
    f32::from_bits(bits)
}

/// Whether a JPEG stream's frame is DCT coded (SOF0/1/2) rather than lossless (SOF3).
pub(crate) fn is_dct_jpeg(d: &[u8]) -> bool {
    let mut i = 2;
    if d.get(..2) != Some(&[0xff, 0xd8][..]) {
        return false;
    }
    while i + 4 <= d.len() {
        if d[i] != 0xff {
            return false;
        }
        let m = d[i + 1];
        if m == 0xff {
            i += 1;
            continue;
        }
        match m {
            0xc0..=0xc2 => return true,
            0xc3 | 0xda | 0xd9 => return false,
            _ => {}
        }
        let len = u16::from_be_bytes([d[i + 2], d[i + 3]]) as usize;
        i += 2 + len;
    }
    false
}

/// DCT JPEG block (compression 34892, or 7 with a DCT frame as some cinema cameras write):
/// one component at 8 or 12 bits (CFA data, stored values), or three 8-bit components
/// (LinearRaw, YCbCr converted to RGB). The frame's samples fill the block in raster order,
/// whatever the frame's own shape (a 976 x 1104 tile may be stored as a 1952 x 552 frame).
fn jpeg(src: &[u8], tw: usize, cpp: usize, rows: &mut [&mut [u16]]) -> Result<bool> {
    let bad_tile = |_| RawError::Malformed("DNG lossy JPEG tile");
    let info = specular_jpeg::read_info(src).map_err(bad_tile)?;
    let (fw, fh) = (info.width as usize, info.height as usize);
    // Never decode more than the block holds (plus a partial MCU row).
    let cap = tw.saturating_mul(rows.len() + 16).saturating_add(4096);
    if fw.saturating_mul(fh) > cap {
        return Err(RawError::Malformed("DNG JPEG tile larger than its block"));
    }
    let nc = info.components.len();
    let (stream, truncated): (Vec<u16>, bool) = if nc == 1 && cpp == 1 {
        let wide = specular_jpeg::decode_wide(src, None).map_err(bad_tile)?;
        let plane = wide.planes.into_iter().next().ok_or(RawError::Malformed(
            "DNG lossy JPEG tile without components",
        ))?;
        (plane.data, wide.truncated)
    } else if nc == 3 && cpp == 3 && info.precision == 8 {
        let opts = specular_jpeg::DecodeOptions {
            parallelism: specular_jpeg::Parallelism::Single,
            ..Default::default()
        };
        let p = specular_jpeg::decode(src, &opts).map_err(bad_tile)?;
        let rgba = specular_jpeg::planes_to_rgba(&p);
        let v = rgba
            .chunks_exact(4)
            .flat_map(|px| [px[0] as u16, px[1] as u16, px[2] as u16])
            .collect();
        (v, p.truncated)
    } else {
        return Err(RawError::Unsupported("DNG lossy JPEG: component count"));
    };
    Ok(fill(&stream, tw, cpp, rows) || truncated)
}

/// Pixels (`cpp` samples each) in raster order of a `tw`-wide block into `rows`. Returns true
/// when the stream is too short for the rows.
fn fill(stream: &[u16], tw: usize, cpp: usize, rows: &mut [&mut [u16]]) -> bool {
    let stride = tw * cpp;
    let mut short = false;
    for (y, out) in rows.iter_mut().enumerate() {
        let start = y * stride;
        let src = stream.get(start..).unwrap_or(&[]);
        let m = out.len().min(src.len());
        out[..m].copy_from_slice(&src[..m]);
        short |= m < out.len();
    }
    short
}

/// JPEG XL block: integer samples, one or three channels.
fn jxl(src: &[u8], cpp: usize, rows: &mut [&mut [u16]]) -> Result<bool> {
    let f =
        specular_jxl::JxlFile::open(src).map_err(|_| RawError::Malformed("DNG JPEG XL tile"))?;
    // Bound the decode by what the block can hold (a tile is never larger than the image).
    let (w, h) = (f.info().width as usize, f.info().height as usize);
    let cw = rows.first().map_or(0, |r| r.len() / cpp.max(1));
    if w > cw.saturating_mul(2).saturating_add(4096) || h > rows.len().saturating_mul(2) + 4096 {
        return Err(RawError::Malformed(
            "DNG JPEG XL tile larger than its block",
        ));
    }
    let s = f
        .decode_samples()
        .map_err(|_| RawError::Malformed("DNG JPEG XL tile"))?;
    let ch = s.channels as usize;
    if ch != cpp {
        return Err(RawError::Unsupported("DNG JPEG XL: channel count"));
    }
    let (w, h) = (s.width as usize, s.height as usize);
    let mut bad = !s.complete || h < rows.len();
    for (y, out) in rows.iter_mut().enumerate().take(h) {
        let row = &s.data[y * w * ch..(y + 1) * w * ch];
        if row.len() < out.len() {
            bad = true;
        }
        let m = out.len().min(row.len());
        out[..m].copy_from_slice(&row[..m]);
    }
    Ok(bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zlib(data: &[u8]) -> Vec<u8> {
        // Stored (uncompressed) deflate blocks inside a zlib wrapper.
        let mut out = vec![0x78, 0x01];
        let mut chunks = data.chunks(65535).peekable();
        if chunks.peek().is_none() {
            out.extend([1, 0, 0, 0xff, 0xff]);
        }
        while let Some(c) = chunks.next() {
            out.push(chunks.peek().is_none() as u8);
            let n = c.len() as u16;
            out.extend(n.to_le_bytes());
            out.extend((!n).to_le_bytes());
            out.extend(c);
        }
        out.extend([0, 0, 0, 0]);
        out
    }

    fn tile(kind: Kind, bits: u8, float: bool, predictor: u16, scale: f32) -> DngTile {
        DngTile {
            kind,
            bits,
            float,
            predictor,
            le: true,
            scale: scale.to_bits(),
        }
    }

    fn run(
        src: &[u8],
        t: DngTile,
        tw: usize,
        th: usize,
        cpp: usize,
        cw: usize,
    ) -> (Vec<u16>, bool) {
        let mut out = vec![0u16; cw * cpp * th];
        let mut rows: Vec<&mut [u16]> = out.chunks_mut(cw * cpp).collect();
        let bad = decode_tile(src, t, tw, th, cpp, &mut rows);
        (out, bad)
    }

    #[test]
    fn half_and_24_bit_floats() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x7bff), 65504.0);
        assert_eq!(f16_to_f32(0x0001), 2f32.powi(-24));
        assert!(f16_to_f32(0x7e00).is_nan());
        assert_eq!(f24_to_f32(0x3f_0000), 1.0);
        assert_eq!(f24_to_f32(0x40_8000), 3.0);
        assert_eq!(f24_to_f32(0xbf_0000), -1.0);
    }

    /// Floating point predictor encoder (TIFF Technical Note 3), the inverse of the decoder.
    fn fp_predict(vals: &[f32], nb: usize, stride: usize) -> Vec<u8> {
        let m = vals.len();
        let mut planes = vec![0u8; m * nb];
        for (i, v) in vals.iter().enumerate() {
            let be: Vec<u8> = match nb {
                4 => v.to_be_bytes().to_vec(),
                2 => {
                    // Round-trip only exact halves in the test.
                    let b = v.to_bits();
                    let e = ((b >> 23) & 0xff) as i32 - 127 + 15;
                    let h = ((b >> 16) & 0x8000) | ((e as u32) << 10) | ((b >> 13) & 0x3ff);
                    (h as u16).to_be_bytes().to_vec()
                }
                _ => unreachable!(),
            };
            for b in 0..nb {
                planes[b * m + i] = be[b];
            }
        }
        for i in (stride..planes.len()).rev() {
            planes[i] = planes[i].wrapping_sub(planes[i - stride]);
        }
        planes
    }

    #[test]
    fn deflate_float_predictors() {
        let (tw, th, cpp) = (6usize, 3usize, 1usize);
        let vals: Vec<f32> = (0..tw * th).map(|i| (i * 37 % 200) as f32 + 0.5).collect();
        for (pred, step) in [(3u16, 1usize), (34894, 2), (34895, 4)] {
            for nb in [2usize, 4] {
                let mut raw = Vec::new();
                for r in vals.chunks(tw) {
                    raw.extend(fp_predict(r, nb, step * cpp));
                }
                let t = tile(Kind::Deflate, nb as u8 * 8, true, pred, 2.0);
                let (out, bad) = run(&zlib(&raw), t, tw, th, cpp, tw);
                assert!(!bad);
                let want: Vec<u16> = vals.iter().map(|v| (v * 2.0 + 0.5) as u16).collect();
                assert_eq!(out, want, "predictor {pred} bytes {nb}");
            }
        }
    }

    #[test]
    fn deflate_int_predictors_and_partial_tiles() {
        // A 4x2 tile of RGB, only 3 columns and 1 row inside the image.
        let (tw, th, cpp) = (4usize, 2usize, 3usize);
        let vals: Vec<u16> = (0..tw * th * cpp)
            .map(|i| (i * 1009 % 65536) as u16)
            .collect();
        for (pred, step) in [(1u16, 0usize), (2, 1), (34892, 2), (34893, 4)] {
            let mut enc = vals.clone();
            if step > 0 {
                for r in enc.chunks_mut(tw * cpp) {
                    for i in (step * cpp..r.len()).rev() {
                        r[i] = r[i].wrapping_sub(r[i - step * cpp]);
                    }
                }
            }
            let bytes: Vec<u8> = enc.iter().flat_map(|v| v.to_le_bytes()).collect();
            let t = tile(Kind::Deflate, 16, false, pred, 1.0);
            let (out, bad) = run(&zlib(&bytes), t, tw, th, cpp, 3);
            assert!(!bad);
            let want: Vec<u16> = vals
                .chunks(tw * cpp)
                .flat_map(|r| r[..3 * cpp].to_vec())
                .collect();
            assert_eq!(out, want, "predictor {pred}");
        }
    }

    #[test]
    fn uncompressed_float_byte_orders() {
        let t = DngTile {
            le: false,
            ..tile(Kind::Raw, 24, true, 1, 1.0)
        };
        // 1.0 and 3.0 as big-endian 24-bit floats.
        let (out, bad) = run(&[0x3f, 0, 0, 0x40, 0x80, 0], t, 2, 1, 1, 2);
        assert_eq!((out, bad), (vec![1, 3], false));
        let t = tile(Kind::Raw, 32, true, 1, 100.0);
        let src: Vec<u8> = [0.25f32, -1.0, f32::NAN, 1e9]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let (out, _) = run(&src, t, 4, 1, 1, 4);
        assert_eq!(out, vec![25, 0, 0, 65535]);
    }

    #[test]
    fn hostile_blocks_flag_and_never_panic() {
        let t = tile(Kind::Deflate, 16, true, 3, 1.0);
        for src in [
            &[][..],
            &[0x78],
            &[0x78, 0x01, 0xff, 0xff],
            &[0x00, 0x00, 1, 2],
        ] {
            assert!(run(src, t, 8, 8, 1, 8).1);
        }
        // Short stream: rows past the end stay zero.
        let t = tile(Kind::Deflate, 16, true, 1, 1.0);
        let (out, bad) = run(&zlib(&[0, 0x3c, 0, 0x3c]), t, 2, 2, 1, 2);
        assert!(bad);
        assert_eq!(out, vec![1, 1, 0, 0]);
        for kind in [Kind::Jpeg, Kind::Jxl] {
            let t = tile(kind, 8, false, 1, 1.0);
            for src in [&[][..], &[0xff, 0xd8, 0xff], &[0xff, 0x0a, 0, 0, 0]] {
                assert!(run(src, t, 16, 16, 3, 16).1);
            }
        }
    }

    #[test]
    fn validity() {
        assert!(tile(Kind::Deflate, 16, true, 34894, 1.0).valid());
        assert!(!tile(Kind::Deflate, 16, true, 2, 1.0).valid());
        assert!(!tile(Kind::Raw, 16, true, 3, 1.0).valid());
        assert!(!tile(Kind::Deflate, 12, false, 2, 1.0).valid());
        assert!(tile(Kind::Deflate, 12, false, 1, 1.0).valid());
        assert!(!tile(Kind::Jpeg, 16, false, 1, 1.0).valid());
    }
}
