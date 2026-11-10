//! Composite decode: a row source (raw / PackBits / inflated ZIP) feeding either native planes
//! or a fused 8-bit composer. Rows of every channel are independent, so both paths run
//! row-parallel.

use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::convert::{compose_row, to_u8_row, unmatte_row};
use crate::*;

/// Big-endian sample bytes of a missing sample (mid grey; 0.214 = sRGB 0.5 for float).
pub(crate) fn fill_grey(buf: &mut [u8], depth: u8) {
    match depth {
        16 => buf
            .chunks_exact_mut(2)
            .for_each(|c| c.copy_from_slice(&[0x80, 0])),
        32 => buf
            .chunks_exact_mut(4)
            .for_each(|c| c.copy_from_slice(&0.214f32.to_be_bytes())),
        _ => buf.fill(GREY),
    }
}

enum Kind<'a> {
    Raw(&'a [u8]),
    Rle {
        data: &'a [u8],
        /// Start of row `i` in `data` is `offs[i]`, end `offs[i + 1]`.
        offs: Vec<usize>,
    },
    Zip {
        buf: Vec<u8>,
        /// Bytes the stream actually produced.
        valid: usize,
        pred: bool,
    },
}

/// Rows of the planes we need, as stored (big-endian), in file order: row `c * h + y`.
pub(crate) struct Source<'a> {
    kind: Kind<'a>,
    pub row_bytes: usize,
    pub depth: u8,
}

impl<'a> Source<'a> {
    pub fn new(psd: &Psd, data: &'a [u8], planes: usize) -> Result<Source<'a>> {
        let info = &psd.info;
        let comp = check_composite(info)?;
        let (w, h) = (info.width as usize, info.height as usize);
        let row_bytes = if info.depth == 1 {
            w.div_ceil(8)
        } else {
            w * (info.depth as usize / 8)
        };
        let needed_rows = planes * h;
        let img = &data[psd.image_data.clone()];
        let kind = match comp {
            Compression::Raw => {
                // Partial files still show what they have, but a header promising far more than
                // the file holds (corrupt dimensions) must not make us allocate for it.
                if (img.len() as u64) < (needed_rows * row_bytes) as u64 / 64 {
                    return Err(Error::Truncated("raw image data".into()));
                }
                Kind::Raw(img)
            }
            Compression::Rle => {
                let entry = if info.version == Version::Psb { 4 } else { 2 };
                let total_rows = info.channels as usize * h;
                let table_len = total_rows * entry;
                if img.len() < table_len {
                    return Err(Error::Truncated("RLE row-length table".into()));
                }
                let (table, rest) = img.split_at(table_len);
                // PackBits packs at most 64:1; a partial file still shows what it has, but
                // the row table alone (2 bytes a row) must not make us allocate the image.
                crate::proven(rest.len(), needed_rows * row_bytes / 64, "RLE image data")?;
                let mut offs = Vec::with_capacity(needed_rows + 1);
                let mut acc = 0usize;
                offs.push(0);
                for e in table.chunks_exact(entry).take(needed_rows) {
                    let n = if entry == 2 {
                        u16::from_be_bytes([e[0], e[1]]) as usize
                    } else {
                        u32::from_be_bytes([e[0], e[1], e[2], e[3]]) as usize
                    };
                    acc = acc.saturating_add(n);
                    offs.push(acc);
                }
                Kind::Rle { data: rest, offs }
            }
            Compression::Zip | Compression::ZipPrediction => {
                let len = needed_rows * row_bytes;
                // Deflate cannot expand more than ~1032:1; refuse to allocate for a stream that
                // could never fill the buffer.
                if (img.len() as u64).saturating_mul(1100) + 4096 < len as u64 {
                    return Err(Error::Truncated("ZIP image data".into()));
                }
                let (buf, valid) = inflate_all(img, len);
                Kind::Zip {
                    buf,
                    valid,
                    pred: comp == Compression::ZipPrediction && info.depth != 1,
                }
            }
        };
        Ok(Source {
            kind,
            row_bytes,
            depth: info.depth,
        })
    }

    /// Raw row `i`. Borrowed straight from the file when possible, else decoded into
    /// `scratch`. The flag is false when the row was incomplete (rest filled grey).
    #[inline]
    pub fn row<'s>(&'s self, i: usize, scratch: &'s mut Vec<u8>) -> (&'s [u8], bool) {
        let rb = self.row_bytes;
        match &self.kind {
            Kind::Raw(img) => {
                let start = i * rb;
                if let Some(r) = img.get(start..start + rb) {
                    return (r, true);
                }
                scratch.resize(rb, 0);
                let have = img.len().saturating_sub(start).min(rb);
                scratch[..have].copy_from_slice(&img[start.min(img.len())..][..have]);
                fill_grey(&mut scratch[have..], self.depth);
                (scratch, false)
            }
            Kind::Rle { data, offs } => {
                scratch.resize(rb, 0);
                let (a, b) = (offs[i].min(data.len()), offs[i + 1].min(data.len()));
                let got = rle::unpack(&data[a..b], scratch);
                let ok = got == rb;
                if !ok {
                    fill_grey(&mut scratch[got..], self.depth);
                }
                (scratch, ok)
            }
            Kind::Zip { buf, valid, pred } => {
                let start = i * rb;
                let have = valid.saturating_sub(start).min(rb);
                if !pred && have == rb {
                    return (&buf[start..start + rb], true);
                }
                scratch.resize(rb * 2, 0);
                let (row, tmp) = scratch.split_at_mut(rb);
                row[..have].copy_from_slice(&buf[start.min(buf.len())..][..have]);
                if *pred {
                    unpredict(row, tmp, self.depth);
                }
                if have < rb {
                    fill_grey(&mut row[have..], self.depth);
                }
                (&scratch[..rb], have == rb)
            }
        }
    }
}

/// Whether the stored merged image is a blank placeholder: every sample of each colour
/// channel the same. Only raw and RLE composites are looked at (a ZIP one would have to be
/// inflated whole). The first, middle and last rows of each channel are checked before the
/// rest, so a real image is told apart after a few rows; a cut-short image is not blank.
pub(crate) fn blank(psd: &Psd, data: &[u8]) -> bool {
    let info = &psd.info;
    if info.depth == 1 || !matches!(info.compression, Some(Compression::Raw | Compression::Rle)) {
        return false;
    }
    let cc = info.mode.color_channels(info.channels);
    let h = info.height as usize;
    let Ok(src) = Source::new(psd, data, cc) else {
        return false;
    };
    let bps = (info.depth as usize / 8).max(1);
    let mut scratch = Vec::new();
    let mut first: Vec<Option<Vec<u8>>> = vec![None; cc];
    let probes = (0..cc).flat_map(|c| [c * h, c * h + h / 2, c * h + h - 1]);
    for i in probes.chain(0..cc * h) {
        let (row, ok) = src.row(i, &mut scratch);
        if !ok || row.len() < bps {
            return false;
        }
        let v = first[i / h].get_or_insert_with(|| row[..bps].to_vec());
        if row.chunks_exact(bps).any(|s| s != v.as_slice()) {
            return false;
        }
    }
    true
}

/// Inflate `len` bytes from one zlib stream or several concatenated ones (Photoshop writes one
/// per channel). Returns the buffer and how many bytes were produced.
fn inflate_all(mut input: &[u8], len: usize) -> (Vec<u8>, usize) {
    let mut out = vec![0u8; len];
    let mut z = zlib_rs::Inflate::new(true, 15);
    let mut pos = 0;
    while pos < len && input.len() > 2 {
        let r = z.decompress(input, &mut out[pos..], zlib_rs::InflateFlush::Finish);
        let (used, made) = (z.total_in() as usize, z.total_out() as usize);
        pos += made.min(len - pos);
        if !matches!(r, Ok(zlib_rs::Status::StreamEnd)) || used == 0 {
            break;
        }
        input = &input[used.min(input.len())..];
        z.reset(true);
    }
    (out, pos)
}

/// Undo ZIP delta prediction on one big-endian row.
pub(crate) fn unpredict(row: &mut [u8], tmp: &mut [u8], depth: u8) {
    match depth {
        8 => {
            for i in 1..row.len() {
                row[i] = row[i].wrapping_add(row[i - 1]);
            }
        }
        16 => {
            let mut prev = 0u16;
            for c in row.chunks_exact_mut(2) {
                prev = prev.wrapping_add(u16::from_be_bytes([c[0], c[1]]));
                c.copy_from_slice(&prev.to_be_bytes());
            }
        }
        32 => {
            for i in 1..row.len() {
                row[i] = row[i].wrapping_add(row[i - 1]);
            }
            // Bytes are stored as planes: all MSBs, then the next byte of every sample…
            let w = row.len() / 4;
            for x in 0..w {
                for b in 0..4 {
                    tmp[x * 4 + b] = row[b * w + x];
                }
            }
            row[..w * 4].copy_from_slice(&tmp[..w * 4]);
        }
        _ => {}
    }
}

/// Run `f(state, row_index, row)` over `out` split into rows, in parallel if allowed.
pub(crate) fn for_rows<T: Send, S>(
    out: &mut [T],
    row_len: usize,
    parallel: bool,
    init: impl Fn() -> S + Sync + Send,
    f: impl Fn(&mut S, usize, &mut [T]) + Sync + Send,
) {
    if row_len == 0 {
        return;
    }
    if parallel {
        out.par_chunks_mut(row_len)
            .enumerate()
            .with_min_len(8)
            .for_each_init(&init, |s, (i, r)| f(s, i, r));
    } else {
        let mut s = init();
        for (i, r) in out.chunks_mut(row_len).enumerate() {
            f(&mut s, i, r);
        }
    }
}

fn plane_count(info: &Info) -> usize {
    info.mode.color_channels(info.channels) + info.has_alpha as usize
}

pub(crate) fn planes(psd: &Psd, data: &[u8], opts: &Options) -> Result<Planes> {
    let info = &psd.info;
    let np = plane_count(info);
    let src = Source::new(psd, data, np)?;
    let w = info.width as usize;
    let n = np * w * info.height as usize;
    let bad = AtomicBool::new(false);
    let note = |ok: bool| {
        if !ok {
            bad.store(true, Ordering::Relaxed);
        }
    };
    let samples = match info.depth {
        1 => {
            let mut out = vec![0u8; n];
            for_rows(&mut out, w, opts.parallel, Vec::new, |s, i, o| {
                let (r, ok) = src.row(i, s);
                note(ok);
                for (x, v) in o.iter_mut().enumerate() {
                    *v = if r[x >> 3] & (0x80 >> (x & 7)) != 0 {
                        0
                    } else {
                        255
                    };
                }
            });
            Samples::U8(out)
        }
        8 => {
            let mut out = vec![0u8; n];
            for_rows(&mut out, w, opts.parallel, Vec::new, |s, i, o| {
                let (r, ok) = src.row(i, s);
                note(ok);
                o.copy_from_slice(r);
            });
            Samples::U8(out)
        }
        16 => {
            let mut out = vec![0u16; n];
            for_rows(&mut out, w, opts.parallel, Vec::new, |s, i, o| {
                let (r, ok) = src.row(i, s);
                note(ok);
                for (v, b) in o.iter_mut().zip(r.chunks_exact(2)) {
                    *v = u16::from_be_bytes([b[0], b[1]]);
                }
            });
            Samples::U16(out)
        }
        _ => {
            let mut out = vec![0f32; n];
            for_rows(&mut out, w, opts.parallel, Vec::new, |s, i, o| {
                let (r, ok) = src.row(i, s);
                note(ok);
                for (v, b) in o.iter_mut().zip(r.chunks_exact(4)) {
                    *v = f32::from_be_bytes([b[0], b[1], b[2], b[3]]);
                }
            });
            Samples::F32(out)
        }
    };
    Ok(Planes {
        width: info.width,
        height: info.height,
        mode: info.mode,
        color_channels: info.mode.color_channels(info.channels),
        alpha: info.has_alpha,
        truncated: bad.into_inner(),
        palette: info.palette.clone(),
        transparent_index: info.transparent_index,
        matted_on_white: info.matted_on_white
            || (info.matte_unknown
                && crate::looks_matted(
                    &samples,
                    info.mode.color_channels(info.channels),
                    info.width as usize * info.height as usize,
                )),
        samples,
    })
}

/// [`crate::looks_matted`] on interleaved RGBA.
fn rgba8_looks_matted(px: &[u8]) -> bool {
    let mut seen = false;
    for p in px.chunks_exact(4) {
        let a = p[3] as i32;
        if a == 255 {
            continue;
        }
        seen = true;
        if p[..3].iter().any(|&c| (c as i32) < 255 - a - 1) {
            return false;
        }
    }
    seen
}

/// Output layout for an 8-bit composite.
pub(crate) fn out_format(mode: ColorMode, color_channels: usize, alpha: bool) -> PixelFormat {
    let grey = matches!(
        mode,
        ColorMode::Bitmap | ColorMode::Grayscale | ColorMode::Duotone
    ) || (mode == ColorMode::Multichannel && color_channels == 1);
    if grey && !alpha {
        PixelFormat::Gray8
    } else {
        PixelFormat::Rgba8
    }
}

struct RowScratch {
    raw: Vec<Vec<u8>>,
    conv: Vec<Vec<u8>>,
    /// The transparency row of a white-matted 32-bit composite, and the colour row being
    /// un-matted (linear light).
    alpha: Vec<f32>,
    lin: Vec<u8>,
}

/// Fused decode → 8-bit, row-parallel: for each output row fetch each plane's row, bring it to
/// 8 bits and compose. Never materialises the planar image.
pub(crate) fn rgba8(psd: &Psd, data: &[u8], opts: &Options) -> Result<Image> {
    let info = &psd.info;
    let cc = info.mode.color_channels(info.channels);
    let np = plane_count(info);
    let src = Source::new(psd, data, np)?;
    let (w, h) = (info.width as usize, info.height as usize);
    let format = out_format(info.mode, cc, info.has_alpha);
    let bpp = format.channels();
    let mut out = vec![0u8; w * h * bpp];
    let bad = AtomicBool::new(false);
    let pal = info.palette.as_deref();
    let ctx = convert::Compose {
        mode: info.mode,
        color_channels: cc,
        alpha: info.has_alpha,
        palette: pal,
        transparent_index: info.transparent_index,
        format,
    };
    let depth = info.depth;
    for_rows(
        &mut out,
        w * bpp,
        opts.parallel,
        || RowScratch {
            raw: vec![Vec::new(); np],
            conv: vec![Vec::new(); np],
            alpha: Vec::new(),
            lin: Vec::new(),
        },
        |s, y, o| {
            let RowScratch {
                raw,
                conv,
                alpha,
                lin,
            } = s;
            // 32-bit documents are matted in linear light: undo it there, before the curve.
            let linear_matte = depth == 32 && info.matted_on_white && info.has_alpha && np > cc;
            if linear_matte {
                let (r, _) = src.row(cc * h + y, lin);
                alpha.clear();
                alpha.extend(
                    r.chunks_exact(4)
                        .map(|b| f32::from_be_bytes([b[0], b[1], b[2], b[3]])),
                );
            }
            let mut rows: [&[u8]; 5] = [&[]; 5];
            for (c, (rs, cs)) in raw.iter_mut().zip(conv.iter_mut()).enumerate() {
                let (r, ok) = src.row(c * h + y, rs);
                if !ok {
                    bad.store(true, Ordering::Relaxed);
                }
                let is_alpha = c >= cc;
                if depth == 8 {
                    rows[c] = r;
                } else if linear_matte && !is_alpha {
                    cs.resize(w, 0);
                    convert::unmatte_f32_row(r, alpha, info.mode, cs);
                    rows[c] = cs;
                } else {
                    cs.resize(w, 0);
                    to_u8_row(r, depth, info.mode, is_alpha, cs);
                    rows[c] = cs;
                }
            }
            compose_row(&ctx, &rows[..np], o);
            if info.matted_on_white && format == PixelFormat::Rgba8 && depth != 32 {
                unmatte_row(o);
            }
        },
    );
    if !info.matted_on_white
        && info.matte_unknown
        && depth != 32
        && format == PixelFormat::Rgba8
        && rgba8_looks_matted(&out)
    {
        for o in out.chunks_exact_mut(w * bpp) {
            unmatte_row(o);
        }
    }
    Ok(Image {
        width: info.width,
        height: info.height,
        format,
        data: out,
        truncated: bad.into_inner(),
    })
}
