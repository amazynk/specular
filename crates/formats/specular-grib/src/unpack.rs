//! Unpacking a field's values: the packings, then the bitmap.

use crate::bits::{BitReader, f32_, unpack};
use crate::{Ccsds, Complex, Error, Field, MAX_POINTS, Packing, Result, Scale, bad, unsupported};

/// The field's values in point order, NaN where the bitmap or missing-value management says
/// so.
pub fn field(f: &Field, file: &[u8]) -> Result<Vec<f64>> {
    let data = file.get(f.data.clone()).ok_or(Error::Truncated)?;
    let n = f.values;
    let points = f.grid.points();
    if n.max(points) > MAX_POINTS {
        return unsupported(format!("{} values on {points} points", n));
    }
    // The unpacked values and, with a bitmap, the values spread over the points, held at once
    // while unpacking: reserved from the memory budget first (a constant field of a few bytes
    // can declare 2^27 of them).
    let _held = specular_core::budget::reserve_n(n.max(points) as u64, 16)?;
    let packed = match &f.packing {
        Packing::Simple(s) => simple(data, s, n)?,
        Packing::Complex(s, c) => complex(data, s, c, n)?,
        Packing::Jpeg2000(s) => jpeg2000(data, s, n)?,
        Packing::Png(s) => png(data, s, n)?,
        Packing::Ccsds(s, c) => ccsds(data, s, c, n)?,
        Packing::Ieee(b) => ieee(data, *b, n)?,
    };
    match &f.bitmap {
        None => {
            if packed.len() != points {
                return bad(format!("{} values for {points} points", packed.len()));
            }
            Ok(packed)
        }
        Some(r) => {
            let bm = file.get(r.clone()).ok_or(Error::Truncated)?;
            if bm.len() * 8 < points {
                return Err(Error::Truncated);
            }
            let mut out = Vec::with_capacity(points);
            let mut it = packed.into_iter();
            for k in 0..points {
                if bm[k >> 3] & (0x80 >> (k & 7)) != 0 {
                    out.push(it.next().ok_or_else(|| {
                        Error::Malformed("bitmap marks more points than the values".into())
                    })?);
                } else {
                    out.push(f64::NAN);
                }
            }
            Ok(out)
        }
    }
}

fn constant(s: &Scale, n: usize) -> Vec<f64> {
    vec![s.value(0.0); n]
}

fn scaled(raw: impl Iterator<Item = u32>, s: &Scale, n: usize) -> Vec<f64> {
    let (r, b, d) = (s.reference, s.bscale(), s.dscale());
    let mut out = Vec::with_capacity(n);
    out.extend(raw.map(|x| (r + f64::from(x) * b) * d));
    out
}

fn simple(data: &[u8], s: &Scale, n: usize) -> Result<Vec<f64>> {
    if s.bits == 0 {
        return Ok(constant(s, n));
    }
    let raw = unpack(data, u32::from(s.bits), n)?;
    Ok(scaled(raw.into_iter(), s, n))
}

/// Templates 5.2 and 5.3 (after g2clib's group layout).
fn complex(data: &[u8], s: &Scale, c: &Complex, n: usize) -> Result<Vec<f64>> {
    let ng = c.groups as usize;
    if n == 0 {
        return Ok(Vec::new());
    }
    if ng == 0 {
        return bad("complex packing without groups");
    }
    if ng > n {
        return bad("more groups than values");
    }
    let mut r = BitReader::new(data);
    let mut ivals = [0i64; 2];
    let mut minsd = 0i64;
    if c.order > 0 {
        let oct = c.octets as u32;
        if !(1..=4).contains(&oct) {
            return bad("extra descriptor octets");
        }
        let sm = |v: u32| {
            let sign = 1u32 << (8 * oct - 1);
            if v & sign != 0 {
                -i64::from(v & !sign)
            } else {
                i64::from(v)
            }
        };
        for v in ivals.iter_mut().take(c.order as usize) {
            *v = sm(r.get(8 * oct));
        }
        minsd = sm(r.get(8 * oct));
    }
    let nbits = u32::from(s.bits);
    if nbits > 32 || c.width_bits > 32 || c.length_bits > 32 {
        return bad("field widths");
    }
    let refs: Vec<u32> = (0..ng).map(|_| r.get(nbits)).collect();
    r.align();
    let widths: Vec<u32> = (0..ng)
        .map(|_| r.get(u32::from(c.width_bits)) + u32::from(c.width_ref))
        .collect();
    r.align();
    let mut lengths: Vec<usize> = (0..ng)
        .map(|_| {
            c.length_ref as usize + r.get(u32::from(c.length_bits)) as usize * c.length_inc as usize
        })
        .collect();
    r.align();
    lengths[ng - 1] = c.last_length as usize;
    if lengths.iter().try_fold(0usize, |a, &l| a.checked_add(l)) != Some(n) {
        return bad("group lengths don't add up to the values");
    }
    if widths.iter().any(|&w| w > 32) {
        return bad("group width");
    }

    // Integers, with missing values as None.
    let mut ints: Vec<i64> = Vec::with_capacity(n);
    let mut miss: Vec<bool> = if c.missing > 0 {
        vec![false; n]
    } else {
        Vec::new()
    };
    let all = |w: u32| if w >= 32 { u32::MAX } else { (1u32 << w) - 1 };
    let ref_missing1 = all(nbits);
    let ref_missing2 = ref_missing1.wrapping_sub(1);
    for g in 0..ng {
        let (rf, w, len) = (refs[g], widths[g], lengths[g]);
        if c.missing == 0 {
            if w == 0 {
                ints.extend(std::iter::repeat_n(i64::from(rf), len));
            } else {
                for _ in 0..len {
                    ints.push(i64::from(rf) + i64::from(r.get(w)));
                }
            }
            continue;
        }
        let m1 = all(w);
        let m2 = m1.wrapping_sub(1);
        for _ in 0..len {
            let k = ints.len();
            if w == 0 {
                let is_missing = rf == ref_missing1 || (c.missing == 2 && rf == ref_missing2);
                miss[k] = is_missing;
                ints.push(i64::from(rf));
            } else {
                let x = r.get(w);
                let is_missing = x == m1 || (c.missing == 2 && x == m2);
                miss[k] = is_missing;
                ints.push(i64::from(rf) + i64::from(x));
            }
        }
    }

    // Undo spatial differencing over the present values (wrapping, as hostile differences
    // can overflow).
    if c.order > 0 {
        let present: Vec<usize> = if miss.is_empty() {
            (0..n).collect()
        } else {
            (0..n).filter(|&k| !miss[k]).collect()
        };
        let o = c.order as usize;
        for (i, &k) in present.iter().enumerate() {
            if i < o {
                ints[k] = ivals[i];
            } else if o == 1 {
                ints[k] = ints[k]
                    .wrapping_add(minsd)
                    .wrapping_add(ints[present[i - 1]]);
            } else {
                ints[k] = ints[k]
                    .wrapping_add(minsd)
                    .wrapping_add(ints[present[i - 1]].wrapping_mul(2))
                    .wrapping_sub(ints[present[i - 2]]);
            }
        }
    }

    let (rv, b, d) = (s.reference, s.bscale(), s.dscale());
    Ok(ints
        .iter()
        .enumerate()
        .map(|(k, &x)| {
            if !miss.is_empty() && miss[k] {
                f64::NAN
            } else {
                (rv + x as f64 * b) * d
            }
        })
        .collect())
}

fn jpeg2000(data: &[u8], s: &Scale, n: usize) -> Result<Vec<f64>> {
    if s.bits == 0 || n == 0 {
        return Ok(constant(s, n));
    }
    let dec = specular_jp2::Decoder::new(data)
        .map_err(|e| Error::Malformed(format!("JPEG 2000: {e}")))?;
    let comps = dec
        .decode_components_i32()
        .map_err(|e| Error::Malformed(format!("JPEG 2000: {e}")))?;
    let (_, _, v) = comps
        .into_iter()
        .next()
        .ok_or_else(|| Error::Malformed("JPEG 2000 without components".into()))?;
    if v.len() < n {
        return bad(format!(
            "JPEG 2000 image of {} samples for {n} values",
            v.len()
        ));
    }
    Ok(scaled(v.into_iter().take(n).map(|x| x as u32), s, n))
}

fn png(data: &[u8], s: &Scale, n: usize) -> Result<Vec<f64>> {
    if s.bits == 0 || n == 0 {
        return Ok(constant(s, n));
    }
    if data.len() < 8 || &data[..8] != b"\x89PNG\r\n\x1a\n" {
        return bad("PNG signature");
    }
    let mut pos = 8;
    let mut ihdr = None;
    let mut idat = Vec::new();
    while pos + 8 <= data.len() {
        let len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let kind = &data[pos + 4..pos + 8];
        let body = data.get(pos + 8..pos + 8 + len).ok_or(Error::Truncated)?;
        match kind {
            b"IHDR" if len >= 13 => ihdr = Some(body),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        pos += 12 + len;
    }
    let h = ihdr.ok_or_else(|| Error::Malformed("PNG without IHDR".into()))?;
    let w = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as usize;
    let rows = u32::from_be_bytes([h[4], h[5], h[6], h[7]]) as usize;
    let depth = usize::from(h[8]);
    let channels = match h[9] {
        0 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        t => return unsupported(format!("PNG colour type {t}")),
    };
    if h[12] != 0 {
        return unsupported("interlaced PNG");
    }
    if !matches!(depth, 1 | 2 | 4 | 8 | 16) {
        return bad("PNG bit depth");
    }
    let pixel_bits = depth * channels;
    let stride = (w * pixel_bits).div_ceil(8);
    let bpp = pixel_bits.div_ceil(8).max(1);
    let total = rows
        .checked_mul(stride + 1)
        .ok_or_else(|| Error::Malformed("PNG size".into()))?;
    let raw = specular_png::inflate_zlib(&idat, total)
        .map_err(|e| Error::Malformed(format!("PNG: {e}")))?;
    if raw.len() < total {
        return Err(Error::Truncated);
    }
    let mut img = vec![0u8; rows * stride];
    for y in 0..rows {
        let filter = raw[y * (stride + 1)];
        let src = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        let (done, cur) = img.split_at_mut(y * stride);
        let prev = if y > 0 {
            &done[(y - 1) * stride..]
        } else {
            &[][..]
        };
        let cur = &mut cur[..stride];
        unfilter(filter, src, prev, cur, bpp)?;
    }
    // The samples fill whole pixels (writers round odd bit widths up to a PNG depth).
    if pixel_bits > 32 {
        return bad(format!("PNG of {pixel_bits}-bit pixels"));
    }
    let bits = pixel_bits as u32;
    let per_row_bits = w * pixel_bits;
    let mut out = Vec::with_capacity(n);
    let (rv, b, d) = (s.reference, s.bscale(), s.dscale());
    'rows: for y in 0..rows {
        let mut r = BitReader::new(&img[y * stride..(y + 1) * stride]);
        let mut used = 0;
        while used + bits as usize <= per_row_bits {
            let x = r.get(bits);
            out.push((rv + f64::from(x) * b) * d);
            used += bits as usize;
            if out.len() == n {
                break 'rows;
            }
        }
    }
    if out.len() < n {
        return Err(Error::Truncated);
    }
    Ok(out)
}

fn unfilter(filter: u8, src: &[u8], prev: &[u8], cur: &mut [u8], bpp: usize) -> Result<()> {
    let up = |i: usize| if prev.is_empty() { 0 } else { prev[i] };
    match filter {
        0 => cur.copy_from_slice(src),
        1 => {
            for i in 0..src.len() {
                let a = if i >= bpp { cur[i - bpp] } else { 0 };
                cur[i] = src[i].wrapping_add(a);
            }
        }
        2 => {
            for i in 0..src.len() {
                cur[i] = src[i].wrapping_add(up(i));
            }
        }
        3 => {
            for i in 0..src.len() {
                let a = if i >= bpp { u16::from(cur[i - bpp]) } else { 0 };
                cur[i] = src[i].wrapping_add(((a + u16::from(up(i))) / 2) as u8);
            }
        }
        4 => {
            for i in 0..src.len() {
                let a = if i >= bpp { i16::from(cur[i - bpp]) } else { 0 };
                let b = i16::from(up(i));
                let c = if i >= bpp {
                    i16::from(if prev.is_empty() { 0 } else { prev[i - bpp] })
                } else {
                    0
                };
                let p = a + b - c;
                let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                let pred = if pa <= pb && pa <= pc {
                    a
                } else if pb <= pc {
                    b
                } else {
                    c
                };
                cur[i] = src[i].wrapping_add(pred as u8);
            }
        }
        f => return bad(format!("PNG filter {f}")),
    }
    Ok(())
}

fn ccsds(data: &[u8], s: &Scale, c: &Ccsds, n: usize) -> Result<Vec<f64>> {
    if s.bits == 0 || n == 0 {
        return Ok(constant(s, n));
    }
    let p = specular_aec::Params {
        bits_per_sample: u32::from(s.bits),
        block_size: u32::from(c.block),
        rsi: u32::from(c.rsi),
        flags: c.flags,
    };
    let raw =
        specular_aec::decode(data, &p, n).map_err(|e| Error::Malformed(format!("CCSDS: {e}")))?;
    if raw.len() < n {
        return Err(Error::Truncated);
    }
    Ok(scaled(raw.into_iter(), s, n))
}

fn ieee(data: &[u8], bytes: u8, n: usize) -> Result<Vec<f64>> {
    let size = usize::from(bytes);
    if data.len() < n * size {
        return Err(Error::Truncated);
    }
    Ok(match bytes {
        4 => (0..n)
            .map(|k| f64::from(f32_(data, 4 * k).unwrap_or(f32::NAN)))
            .collect(),
        _ => data[..8 * n]
            .chunks_exact(8)
            .map(|b| f64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
            .collect(),
    })
}
