//! Radiance RGBE / XYZE.

use crate::{ColorSpace, Encoding, Error, Format, Info, Orientation, Result, for_rows};

/// Longest header we scan for the end-of-header blank line and the resolution string.
const MAX_HEADER: usize = 1 << 16;
/// New-style RLE is only used for scanline lengths in this range (Radiance `MINELEN`/`MAXELEN`).
const MIN_RLE_LEN: usize = 8;
const MAX_RLE_LEN: usize = 0x7fff;

pub(crate) fn probe(data: &[u8]) -> bool {
    // "#?" followed by a program name ("RADIANCE", "RGBE", ...) on the first line.
    data.len() >= 3
        && data.starts_with(b"#?")
        && data[2..]
            .iter()
            .take(64)
            .take_while(|&&b| b != b'\n')
            .all(|&b| b.is_ascii_graphic() || b == b' ' || b == b'\r')
}

fn parse_floats<const N: usize>(s: &str) -> Option<[f32; N]> {
    let mut out = [0f32; N];
    let mut it = s.split_ascii_whitespace();
    for o in &mut out {
        *o = it.next()?.parse().ok()?;
    }
    Some(out)
}

pub(crate) fn read_info(data: &[u8]) -> Result<Info> {
    if !probe(data) {
        return Err(Error::NotRecognized);
    }
    let head = &data[..data.len().min(MAX_HEADER)];
    let mut pos = 0usize;
    let next_line = |pos: &mut usize| -> Option<&[u8]> {
        let rest = &head[*pos..];
        let n = rest.iter().position(|&b| b == b'\n')?;
        *pos += n + 1;
        let line = &rest[..n];
        Some(line.strip_suffix(b"\r").unwrap_or(line))
    };
    next_line(&mut pos).ok_or(Error::InvalidHeader("unterminated magic line"))?;
    let mut color_space = ColorSpace::Rgb;
    let mut exposure = 1f32;
    let mut colorcorr = [1f32; 3];
    let mut primaries = None;
    let mut pixel_aspect = 1f32;
    loop {
        let line = next_line(&mut pos).ok_or(Error::InvalidHeader("no end of header"))?;
        if line.iter().all(|b| b.is_ascii_whitespace()) {
            break;
        }
        let Ok(line) = std::str::from_utf8(line) else {
            continue; // comments may contain anything
        };
        let line = line.trim_start();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "FORMAT" => {
                color_space = match value {
                    "32-bit_rle_rgbe" => ColorSpace::Rgb,
                    "32-bit_rle_xyze" => ColorSpace::Xyz,
                    _ => return Err(Error::Unsupported("FORMAT")),
                }
            }
            "EXPOSURE" => {
                if let Some([e]) = parse_floats::<1>(value)
                    && e.is_finite()
                    && e > 0.0
                {
                    exposure *= e;
                }
            }
            "COLORCORR" => {
                if let Some(c) = parse_floats::<3>(value)
                    && c.iter().all(|v| v.is_finite() && *v > 0.0)
                {
                    for (a, b) in colorcorr.iter_mut().zip(c) {
                        *a *= b;
                    }
                }
            }
            "PRIMARIES" => {
                if let Some(p) = parse_floats::<8>(value)
                    && p.iter().all(|v| v.is_finite())
                {
                    primaries = Some(p);
                }
            }
            "PIXASPECT" => {
                if let Some([a]) = parse_floats::<1>(value)
                    && a.is_finite()
                    && a > 0.0
                {
                    pixel_aspect *= a;
                }
            }
            _ => {}
        }
    }
    let res = next_line(&mut pos).ok_or(Error::InvalidHeader("no resolution string"))?;
    let res = std::str::from_utf8(res).map_err(|_| Error::InvalidHeader("resolution string"))?;
    let (orientation, width, height) =
        parse_resolution(res).ok_or(Error::InvalidHeader("resolution string"))?;
    Ok(Info {
        format: Format::Radiance,
        width,
        height,
        channels: 3,
        encoding: Encoding::Rgbe,
        color_space,
        exposure,
        color_correction: colorcorr,
        primaries,
        pixel_aspect,
        orientation,
        scale: 1.0,
        data_offset: pos,
    })
}

/// `-Y N +X M` and the seven other orders. Returns (orientation, width, height).
fn parse_resolution(s: &str) -> Option<(Orientation, u32, u32)> {
    let mut it = s.split_ascii_whitespace();
    let mut axis = || -> Option<(bool, u8, u32)> {
        let a = it.next()?.as_bytes();
        if a.len() != 2 {
            return None;
        }
        let plus = match a[0] {
            b'+' => true,
            b'-' => false,
            _ => return None,
        };
        let name = a[1];
        if name != b'X' && name != b'Y' {
            return None;
        }
        let n: u32 = it.next()?.parse().ok()?;
        Some((plus, name, n))
    };
    let (p1, a1, n1) = axis()?;
    let (p2, a2, n2) = axis()?;
    if a1 == a2 {
        return None;
    }
    let x_major = a1 == b'X';
    let (yplus, xplus, w, h) = if x_major {
        (p2, p1, n1, n2)
    } else {
        (p1, p2, n2, n1)
    };
    Some((
        Orientation {
            x_major,
            top_down: !yplus,
            left_to_right: xplus,
        },
        w,
        h,
    ))
}

/// Find the end of the scanline of `len` pixels starting at `pos` without decoding it.
/// `None` if the data is truncated or corrupt.
fn skip_line(data: &[u8], mut pos: usize, len: usize) -> Option<usize> {
    if is_new_rle(data, pos, len) {
        pos += 4;
        for _ in 0..4 {
            let mut n = 0usize;
            while n < len {
                let code = *data.get(pos)? as usize;
                if code > 128 {
                    n += code - 128;
                    pos += 2;
                } else {
                    n += code;
                    pos += 1 + code;
                }
            }
            if n != len || pos > data.len() {
                return None;
            }
        }
        Some(pos)
    } else {
        let mut n = 0usize;
        let mut shift = 0u32;
        while n < len {
            let p = data.get(pos..pos + 4)?;
            pos += 4;
            if p[0] == 1 && p[1] == 1 && p[2] == 1 {
                if shift > 16 {
                    return None;
                }
                n += (p[3] as usize) << shift;
                shift += 8;
            } else {
                n += 1;
                shift = 0;
            }
        }
        (n == len).then_some(pos)
    }
}

fn is_new_rle(data: &[u8], pos: usize, len: usize) -> bool {
    (MIN_RLE_LEN..=MAX_RLE_LEN).contains(&len)
        && data.get(pos..pos + 4).is_some_and(|h| {
            h[0] == 2
                && h[1] == 2
                && h[2] & 0x80 == 0
                && ((h[2] as usize) << 8 | h[3] as usize) == len
        })
}

/// Decode one scanline into planar `planes` (4 * len bytes: R.., G.., B.., E..). Returns false
/// on corrupt data (`planes` is then partially written).
fn decode_line(data: &[u8], mut pos: usize, len: usize, planes: &mut [u8]) -> bool {
    if is_new_rle(data, pos, len) {
        pos += 4;
        for plane in planes.chunks_exact_mut(len) {
            let mut n = 0usize;
            while n < len {
                let Some(&code) = data.get(pos) else {
                    return false;
                };
                let code = code as usize;
                if code > 128 {
                    let run = code - 128;
                    let (Some(&v), Some(dst)) = (data.get(pos + 1), plane.get_mut(n..n + run))
                    else {
                        return false;
                    };
                    dst.fill(v);
                    n += run;
                    pos += 2;
                } else {
                    let (Some(src), Some(dst)) = (
                        data.get(pos + 1..pos + 1 + code),
                        plane.get_mut(n..n + code),
                    ) else {
                        return false;
                    };
                    dst.copy_from_slice(src);
                    n += code;
                    pos += 1 + code;
                }
            }
        }
        true
    } else {
        let mut n = 0usize;
        let mut shift = 0u32;
        let mut prev = [0u8; 4];
        while n < len {
            let Some(p) = data.get(pos..pos + 4) else {
                return false;
            };
            pos += 4;
            if p[0] == 1 && p[1] == 1 && p[2] == 1 {
                if shift > 16 {
                    return false;
                }
                let count = (p[3] as usize) << shift;
                shift += 8;
                if n + count > len {
                    return false;
                }
                for x in n..n + count {
                    for (c, v) in prev.iter().enumerate() {
                        planes[c * len + x] = *v;
                    }
                }
                n += count;
            } else {
                prev = [p[0], p[1], p[2], p[3]];
                for (c, v) in prev.iter().enumerate() {
                    planes[c * len + n] = *v;
                }
                n += 1;
                shift = 0;
            }
        }
        true
    }
}

/// `2^(e - 136)` for every exponent byte (0 for e == 0).
pub(crate) fn exp_table() -> [f32; 256] {
    let mut t = [0f32; 256];
    for (e, v) in t.iter_mut().enumerate().skip(1) {
        *v = (e as f32 - 136.0).exp2();
    }
    t
}

/// Convert planar RGBE (`len` pixels) into interleaved `f32` with `nch` channels; `reverse`
/// writes right to left.
fn convert(
    planes: &[u8],
    len: usize,
    nch: usize,
    table: &[f32; 256],
    out: &mut [f32],
    reverse: bool,
) {
    let (r, rest) = planes.split_at(len);
    let (g, rest) = rest.split_at(len);
    let (b, e) = rest.split_at(len);
    let e = &e[..len];
    let px = |i: usize| {
        let m = table[e[i] as usize];
        [r[i] as f32 * m, g[i] as f32 * m, b[i] as f32 * m]
    };
    if nch == 4 {
        for (i, o) in out.chunks_exact_mut(4).enumerate() {
            let src = if reverse { len - 1 - i } else { i };
            let [rr, gg, bb] = px(src);
            o.copy_from_slice(&[rr, gg, bb, 1.0]);
        }
    } else {
        for (i, o) in out.chunks_exact_mut(3).enumerate() {
            let src = if reverse { len - 1 - i } else { i };
            o.copy_from_slice(&px(src));
        }
    }
}

/// Where each stored scanline starts (the cheap skip pass), and whether the data ends early
/// or is corrupt: the scanlines from the first bad one on are not listed, except that the bad
/// one itself is (it decodes as far as it goes).
pub(crate) fn index(data: &[u8], info: &Info) -> (Vec<usize>, bool) {
    let o = info.orientation;
    let (nlines, len) = if o.x_major {
        (info.width as usize, info.height as usize)
    } else {
        (info.height as usize, info.width as usize)
    };
    // Each scanline needs at least one byte per component, so the vector is bounded by the
    // file size.
    let mut starts = Vec::with_capacity(nlines.min(data.len()));
    if len == 0 {
        return (starts, false);
    }
    let mut pos = info.data_offset;
    for _ in 0..nlines {
        match skip_line(data, pos, len) {
            Some(end) => {
                starts.push(pos);
                pos = end;
            }
            None => {
                starts.push(pos);
                return (starts, true);
            }
        }
    }
    (starts, false)
}

/// Row `y` (top-down, after the orientation) of a file stored row by row (not x-major),
/// columns `cols`, as RGBA `f32` into `out` (`cols.len() * 4`). A row past the end of the
/// data is zero, alpha included (as [`decode_into`] leaves it).
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_row(
    data: &[u8],
    info: &Info,
    starts: &[usize],
    y: usize,
    cols: std::ops::Range<usize>,
    out: &mut [f32],
    planes: &mut Vec<u8>,
    table: &[f32; 256],
) {
    let o = info.orientation;
    debug_assert!(!o.x_major);
    let (nlines, len) = (info.height as usize, info.width as usize);
    let s = if o.top_down { y } else { nlines - 1 - y };
    let Some(&start) = starts.get(s) else {
        out.fill(0.0);
        return;
    };
    planes.clear();
    planes.resize(4 * len, 0);
    decode_line(data, start, len, planes);
    let (r, rest) = planes.split_at(len);
    let (g, rest) = rest.split_at(len);
    let (b, e) = rest.split_at(len);
    for (x, px) in cols.zip(out.chunks_exact_mut(4)) {
        let i = if o.left_to_right { x } else { len - 1 - x };
        let m = table[e[i] as usize];
        px.copy_from_slice(&[r[i] as f32 * m, g[i] as f32 * m, b[i] as f32 * m, 1.0]);
    }
}

/// Returns `truncated`.
pub(crate) fn decode_into(
    data: &[u8],
    info: &Info,
    nch: usize,
    parallel: bool,
    out: &mut [f32],
) -> bool {
    let o = info.orientation;
    let (nlines, len) = if o.x_major {
        (info.width as usize, info.height as usize)
    } else {
        (info.height as usize, info.width as usize)
    };
    if nlines == 0 || len == 0 {
        return false;
    }
    // Pass 1: scanline starts (a partial last scanline is still decoded as far as it goes).
    let (starts, truncated) = index(data, info);
    let table = exp_table();
    let line_len = len * nch;
    if !o.x_major {
        let decode_row = |y: usize, row: &mut [f32], planes: &mut Vec<u8>| {
            let s = if o.top_down { y } else { nlines - 1 - y };
            if let Some(&start) = starts.get(s) {
                planes.clear();
                planes.resize(4 * len, 0);
                decode_line(data, start, len, planes);
                convert(planes, len, nch, &table, row, !o.left_to_right);
            }
        };
        if parallel {
            use rayon::prelude::*;
            out.par_chunks_mut(line_len)
                .enumerate()
                .with_min_len(4)
                .for_each_init(Vec::new, |planes, (y, row)| decode_row(y, row, planes));
        } else {
            let mut planes = Vec::new();
            for (y, row) in out.chunks_mut(line_len).enumerate() {
                decode_row(y, row, &mut planes);
            }
        }
    } else {
        // Column-major file: decode scanlines (columns) in file order, then transpose.
        let mut tmp = match crate::alloc_f32((nlines * line_len) as u64) {
            Ok(t) => t,
            Err(_) => return true,
        };
        for_rows(&mut tmp, line_len, parallel, |s, col| {
            if let Some(&start) = starts.get(s) {
                let mut planes = vec![0u8; 4 * len];
                decode_line(data, start, len, &mut planes);
                convert(&planes, len, nch, &table, col, false);
            }
        });
        let w = info.width as usize;
        let tmp = &tmp;
        for_rows(out, w * nch, parallel, |y, row| {
            let f = if o.top_down { y } else { len - 1 - y };
            for x in 0..w {
                let s = if o.left_to_right { x } else { w - 1 - x };
                let src = &tmp[s * line_len + f * nch..][..nch];
                row[x * nch..][..nch].copy_from_slice(src);
            }
        });
    }
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_strings() {
        let (o, w, h) = parse_resolution("-Y 10 +X 20").unwrap();
        assert_eq!((o, w, h), (Orientation::STANDARD, 20, 10));
        let (o, w, h) = parse_resolution("+X 20 -Y 10").unwrap();
        assert!(o.x_major && o.top_down && o.left_to_right);
        assert_eq!((w, h), (20, 10));
        assert!(parse_resolution("-Y 10 +Y 20").is_none());
        assert!(parse_resolution("-Y 10").is_none());
        assert!(parse_resolution("-Y -1 +X 3").is_none());
    }
}
