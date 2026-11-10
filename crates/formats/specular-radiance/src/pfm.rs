//! Portable Float Map: `PF` (RGB), `Pf` (grey), `PF4` (RGBA, a common extension).

use crate::{ColorSpace, Encoding, Error, Format, Info, Orientation, Result, for_rows};

fn magic_channels(data: &[u8]) -> Option<(u8, usize)> {
    let (ch, n) = if data.starts_with(b"PF4") {
        (4, 3)
    } else if data.starts_with(b"PF") {
        (3, 2)
    } else if data.starts_with(b"Pf") {
        (1, 2)
    } else {
        return None;
    };
    data.get(n)
        .is_some_and(|b| b.is_ascii_whitespace())
        .then_some((ch, n))
}

pub(crate) fn probe(data: &[u8]) -> bool {
    magic_channels(data).is_some()
}

pub(crate) fn read_info(data: &[u8]) -> Result<Info> {
    let (channels, mut pos) = magic_channels(data).ok_or(Error::NotRecognized)?;
    // Three whitespace-separated ASCII tokens, then exactly one whitespace byte.
    let token = |pos: &mut usize| -> Option<&str> {
        while *pos < data.len() && data[*pos].is_ascii_whitespace() {
            *pos += 1;
        }
        let start = *pos;
        while *pos < data.len() && !data[*pos].is_ascii_whitespace() && *pos - start < 64 {
            *pos += 1;
        }
        std::str::from_utf8(data.get(start..*pos)?).ok()
    };
    let width: u32 = token(&mut pos)
        .and_then(|t| t.parse().ok())
        .ok_or(Error::InvalidHeader("width"))?;
    let height: u32 = token(&mut pos)
        .and_then(|t| t.parse().ok())
        .ok_or(Error::InvalidHeader("height"))?;
    let scale: f32 = token(&mut pos)
        .and_then(|t| t.parse().ok())
        .filter(|s: &f32| s.is_finite() && *s != 0.0)
        .ok_or(Error::InvalidHeader("scale"))?;
    if !data.get(pos).is_some_and(|b| b.is_ascii_whitespace()) {
        return Err(Error::InvalidHeader("no whitespace after scale"));
    }
    // Some writers emit "\r\n" here.
    if data.get(pos) == Some(&b'\r') && data.get(pos + 1) == Some(&b'\n') {
        pos += 1;
    }
    pos += 1;
    Ok(Info {
        format: Format::Pfm,
        width,
        height,
        channels,
        encoding: if scale < 0.0 {
            Encoding::F32Le
        } else {
            Encoding::F32Be
        },
        color_space: ColorSpace::Rgb,
        exposure: 1.0,
        color_correction: [1.0; 3],
        primaries: None,
        pixel_aspect: 1.0,
        orientation: Orientation::STANDARD,
        scale: scale.abs(),
        data_offset: pos,
    })
}

/// Row `y` (top-down), columns `cols`, as RGBA `f32` into `out` (`cols.len() * 4`), as
/// [`decode_into`] gives it: what is missing from a short file is zero with alpha one.
pub(crate) fn read_row(
    data: &[u8],
    info: &Info,
    y: usize,
    cols: std::ops::Range<usize>,
    out: &mut [f32],
) {
    let (w, h) = (info.width as usize, info.height as usize);
    let src_ch = info.channels as usize;
    let src_row = w * src_ch * 4;
    for px in out.chunks_exact_mut(4) {
        px.copy_from_slice(&[0.0, 0.0, 0.0, 1.0]);
    }
    let pixels = data.get(info.data_offset..).unwrap_or(&[]);
    let le = info.encoding == Encoding::F32Le;
    let f = |b: &[u8]| {
        let b = [b[0], b[1], b[2], b[3]];
        if le {
            f32::from_le_bytes(b)
        } else {
            f32::from_be_bytes(b)
        }
    };
    // File rows are bottom-up; a partial last row decodes as far as it goes.
    let r = h - 1 - y;
    let Some(src) = pixels.get(r * src_row + cols.start * src_ch * 4..) else {
        return;
    };
    let n = cols.len() * src_ch * 4;
    let src = &src[..src.len().min(n)];
    for (o, s) in out.chunks_exact_mut(4).zip(src.chunks_exact(src_ch * 4)) {
        if src_ch == 1 {
            o[..3].fill(f(s));
        } else {
            for (c, oc) in o.iter_mut().enumerate().take(src_ch) {
                *oc = f(&s[c * 4..]);
            }
        }
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
    let (w, h) = (info.width as usize, info.height as usize);
    let src_ch = info.channels as usize;
    let src_row = w * src_ch * 4;
    if w == 0 || h == 0 {
        return false;
    }
    let pixels = data.get(info.data_offset..).unwrap_or(&[]);
    let le = info.encoding == Encoding::F32Le;
    let truncated = (pixels.len() as u64) < src_row as u64 * h as u64;
    for_rows(out, w * nch, parallel, |y, row| {
        if nch == 4 {
            row.chunks_exact_mut(4).for_each(|p| p[3] = 1.0);
        }
        // File rows are bottom-up; a partial last row decodes as far as it goes.
        let r = h - 1 - y;
        let Some(src) = pixels.get(r * src_row..) else {
            return;
        };
        let src = &src[..src.len().min(src_row)];
        let f = |b: &[u8]| {
            let b = [b[0], b[1], b[2], b[3]];
            if le {
                f32::from_le_bytes(b)
            } else {
                f32::from_be_bytes(b)
            }
        };
        match (src_ch, nch) {
            (1, _) => {
                for (o, s) in row.chunks_exact_mut(nch).zip(src.chunks_exact(4)) {
                    let v = f(s);
                    o[..3].fill(v);
                    if nch == 4 {
                        o[3] = 1.0;
                    }
                }
            }
            (3, 3) | (4, 4) => {
                for (o, s) in row.iter_mut().zip(src.chunks_exact(4)) {
                    *o = f(s);
                }
            }
            _ => {
                for (o, s) in row.chunks_exact_mut(nch).zip(src.chunks_exact(src_ch * 4)) {
                    for (c, oc) in o.iter_mut().enumerate() {
                        *oc = if c < src_ch { f(&s[c * 4..]) } else { 1.0 };
                    }
                }
            }
        }
    });
    truncated
}
