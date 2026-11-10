//! The decoded YCbCr planes of a still image, unconverted ([`AvifFile::decode_planes`]): for
//! a viewer that converts them where they are shown (a shader), so showing an image costs a
//! copy of 1.5 bytes a pixel (3 above 8 bits) instead of a colour pass and 3–8 bytes.
//!
//! - At 1:1 the planes are copied as they are (a grid's tiles into one canvas, the `clap`
//!   crop cut out); chroma keeps its subsampling and its siting (the AV1 sequence header's
//!   `chroma_sample_position`: AVIF has no property for it), to be interpolated where it is
//!   shown.
//! - At 1/s the s×s luma block and the chroma interpolated at its siting under it (bilinear,
//!   as the 1:1 planes are shown) are averaged, alpha-weighted with alpha: 4:4:4 planes at
//!   the output size, the box average of the 1:1 image, as the RGB path's levels are.
//! - Alpha is a fourth plane of the colour's bits, full range (a limited-range or
//!   other-depth alpha image is rescaled).
//!
//! Monochrome images, pre-multiplied alpha, sequences without a still, and the rare layouts
//! the planes cannot carry exactly (a crop at an odd offset along a subsampled axis, grid
//! tiles of an odd size there or of mixed formats, an alpha image smaller than the colour)
//! come back as `None`: [`AvifFile::decode_with`] converts those.

use rayon::prelude::*;

use crate::backend::{Cicp, Picture, Planes, new_decoder_layers};
use crate::heif::{Crop, Meta};
use crate::{
    AvifFile, Error, Source, check_size, clap_of, decode_one, is_mono, item_input, norm_scale,
};

/// Output rows per band between cancellation checks.
const BAND: usize = 16;

/// One plane of a [`YuvImage`]: `width` × `height` samples, rows `stride` bytes apart, one
/// byte a sample at 8 bits, else a native-endian `u16`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YuvPlane {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub data: Vec<u8>,
    /// Subsampling against the image (1 or 2 across and down).
    pub h_div: u8,
    pub v_div: u8,
}

/// The YCbCr planes of a still at 1/`scale` (see the [module docs](self)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YuvImage {
    pub width: u32,
    pub height: u32,
    /// The downscale factor applied (1, 2, 4 or 8).
    pub scale: u32,
    /// Significant bits per sample (8, 10 or 12).
    pub bits: u8,
    /// CICP matrix coefficients, range, primaries and transfer: the container's `nclx` when
    /// it has one, else the bitstream's.
    pub matrix: u16,
    pub full_range: bool,
    pub primaries: u16,
    pub transfer: u16,
    /// Subsampled chroma on the even luma columns (AV1 `CSP_VERTICAL` and `CSP_COLOCATED`)
    /// and rows (`CSP_COLOCATED`); otherwise midway between them.
    pub cosited_x: bool,
    pub cosited_y: bool,
    /// Y, Cb and Cr, then straight alpha when the image has one.
    pub planes: Vec<YuvPlane>,
}

/// A decoded colour or alpha source: one picture, or a grid's tiles in one canvas.
enum Decoded {
    Pic(Box<dyn Picture>),
    Canvas(Canvas),
}

/// A grid's planes, tightly packed, with the format of its tiles.
struct Canvas {
    data: [Vec<u8>; 3],
    strides: [usize; 3],
    width: u32,
    height: u32,
    bits: u8,
    hbd: bool,
    ssx: bool,
    ssy: bool,
    cosited_x: bool,
    cosited_y: bool,
    mono: bool,
    cicp: Cicp,
}

impl Decoded {
    fn planes(&self) -> Planes<'_> {
        match self {
            Decoded::Pic(p) => p.planes(),
            Decoded::Canvas(c) => Planes {
                planes: [&c.data[0], &c.data[1], &c.data[2]],
                strides: c.strides,
                width: c.width,
                height: c.height,
                bits: c.bits,
                hbd: c.hbd,
                ssx: c.ssx,
                ssy: c.ssy,
                cosited_x: c.cosited_x,
                cosited_y: c.cosited_y,
                mono: c.mono,
                cicp: c.cicp,
            },
        }
    }
}

fn inconsistent() -> Error {
    Error::Decoder("decoded picture geometry is inconsistent".into())
}

/// Copies `rows` rows of `n` bytes from `src` (rows `sstride` apart, starting at byte `sx`)
/// to `dst` (rows `dstride` apart, starting at row `dy`, byte `dx`).
#[allow(clippy::too_many_arguments)]
fn copy_rows(
    src: &[u8],
    sstride: usize,
    sx: usize,
    dst: &mut [u8],
    dstride: usize,
    (dx, dy): (usize, usize),
    rows: usize,
    n: usize,
) -> Result<(), Error> {
    for r in 0..rows {
        let s = src
            .get(r * sstride + sx..r * sstride + sx + n)
            .ok_or_else(inconsistent)?;
        let d0 = (dy + r) * dstride + dx;
        dst.get_mut(d0..d0 + n)
            .ok_or_else(inconsistent)?
            .copy_from_slice(s);
    }
    Ok(())
}

/// Decodes `src`: an item's picture, or a grid's tiles copied into one canvas. `None` for
/// grids the planes cannot carry (odd tile sizes along a subsampled axis, tiles of mixed
/// formats).
fn decode_source(
    file: &AvifFile,
    meta: &Meta,
    src: &Source,
    dec: &mut dyn crate::backend::Av1Decoder,
    cancel: &dyn Fn() -> bool,
) -> Result<Option<Decoded>, Error> {
    let data = &file.data;
    let (grid, tiles) = match src {
        Source::Item(id) => {
            return Ok(Some(Decoded::Pic(decode_one(
                dec,
                item_input(meta, data, *id)?,
            )?)));
        }
        Source::Grid { grid, tiles } => (grid, tiles),
    };
    let (gw, gh) = (grid.width as usize, grid.height as usize);
    let mut canvas: Option<Canvas> = None;
    let (mut tile_w, mut tile_h) = (0, 0);
    for (i, &t) in tiles.iter().enumerate() {
        if cancel() {
            return Err(Error::Cancelled);
        }
        let pic = decode_one(dec, item_input(meta, data, t)?)?;
        let p = pic.planes();
        let c = match &mut canvas {
            Some(c) => {
                if p.width as usize != tile_w || p.height as usize != tile_h {
                    return Err(Error::Malformed("grid tiles differ in size".into()));
                }
                if (p.bits, p.hbd, p.ssx, p.ssy, p.mono) != (c.bits, c.hbd, c.ssx, c.ssy, c.mono) {
                    return Ok(None);
                }
                c
            }
            None => {
                (tile_w, tile_h) = (p.width as usize, p.height as usize);
                if tile_w * (grid.cols as usize) < gw || tile_h * (grid.rows as usize) < gh {
                    return Err(Error::Malformed(
                        "grid tiles do not cover the canvas".into(),
                    ));
                }
                // Tiles must start on whole chroma samples (MIAF requires it).
                if (p.ssx && grid.cols > 1 && tile_w % 2 == 1)
                    || (p.ssy && grid.rows > 1 && tile_h % 2 == 1)
                {
                    return Ok(None);
                }
                let bps = if p.hbd { 2 } else { 1 };
                let (cw, ch) = if p.mono {
                    (0, 0)
                } else {
                    (
                        gw.div_ceil(1 << u8::from(p.ssx)),
                        gh.div_ceil(1 << u8::from(p.ssy)),
                    )
                };
                let strides = [gw * bps, cw * bps, cw * bps];
                canvas.insert(Canvas {
                    data: [
                        vec![0; strides[0] * gh],
                        vec![0; strides[1] * ch],
                        vec![0; strides[2] * ch],
                    ],
                    strides,
                    width: grid.width,
                    height: grid.height,
                    bits: p.bits,
                    hbd: p.hbd,
                    ssx: p.ssx,
                    ssy: p.ssy,
                    cosited_x: p.cosited_x,
                    cosited_y: p.cosited_y,
                    mono: p.mono,
                    cicp: p.cicp,
                })
            }
        };
        let (r, col) = (i / grid.cols as usize, i % grid.cols as usize);
        let (x0, y0) = (col * tile_w, r * tile_h);
        if x0 >= gw || y0 >= gh {
            continue;
        }
        let bps = if c.hbd { 2 } else { 1 };
        let (w, h) = (tile_w.min(gw - x0), tile_h.min(gh - y0));
        copy_rows(
            p.planes[0],
            p.strides[0],
            0,
            &mut c.data[0],
            c.strides[0],
            (x0 * bps, y0),
            h,
            w * bps,
        )?;
        if !c.mono {
            let (sx, sy) = (usize::from(c.ssx), usize::from(c.ssy));
            let (cw, ch) = (w.div_ceil(1 << sx), h.div_ceil(1 << sy));
            for k in 1..3 {
                copy_rows(
                    p.planes[k],
                    p.strides[k],
                    0,
                    &mut c.data[k],
                    c.strides[k],
                    ((x0 >> sx) * bps, y0 >> sy),
                    ch,
                    cw * bps,
                )?;
            }
        }
    }
    Ok(canvas.map(Decoded::Canvas))
}

/// How alpha samples become the colour's bits, full range.
#[derive(Clone, Copy)]
struct AlphaMap {
    /// Samples already are: copied as they are.
    identity: bool,
    /// `out = clamp((v − offset) × scale, 0, max)`, rounded.
    offset: f32,
    scale: f32,
    max: f32,
}

impl AlphaMap {
    fn new(alpha: &Planes<'_>, bits: u8) -> AlphaMap {
        let max = ((1u32 << bits) - 1) as f32;
        let abits = alpha.bits.clamp(8, 16);
        let amax = ((1u32 << abits) - 1) as f32;
        let (offset, unit) = if alpha.cicp.full_range {
            (0.0, amax)
        } else {
            let k = (1u32 << (abits - 8)) as f32;
            (16.0 * k, 219.0 * k)
        };
        AlphaMap {
            identity: alpha.cicp.full_range && alpha.bits == bits,
            offset,
            scale: max / unit,
            max,
        }
    }

    #[inline(always)]
    fn map(&self, v: u32) -> u32 {
        if self.identity {
            v
        } else {
            ((v as f32 - self.offset) * self.scale + 0.5).clamp(0.0, self.max) as u32
        }
    }
}

/// One sample of a plane row (native-endian above 8 bits).
#[inline(always)]
fn at(row: &[u8], wide: bool, i: usize) -> u32 {
    if wide {
        u32::from(u16::from_ne_bytes([row[2 * i], row[2 * i + 1]]))
    } else {
        u32::from(row[i])
    }
}

/// Checks every plane of `p` holds `w` × `h` (chroma: subsampled) samples.
fn check(p: &Planes<'_>, w: usize, h: usize, chroma: bool) -> Result<(), Error> {
    let bps = if p.hbd { 2 } else { 1 };
    let need = |k: usize, w: usize, h: usize| {
        h == 0 || (p.strides[k] >= w * bps && p.planes[k].len() >= (h - 1) * p.strides[k] + w * bps)
    };
    let (cw, ch) = (
        w.div_ceil(1 << u8::from(p.ssx)),
        h.div_ceil(1 << u8::from(p.ssy)),
    );
    if (p.width as usize) < w
        || (p.height as usize) < h
        || !need(0, w, h)
        || (chroma && (!need(1, cw, ch) || !need(2, cw, ch)))
    {
        return Err(inconsistent());
    }
    Ok(())
}

/// The 1:1 planes of `c` (and alpha `a`) inside `crop`.
fn full_planes(
    c: &Planes<'_>,
    a: Option<(&Planes<'_>, AlphaMap)>,
    crop: Crop,
) -> Result<Vec<YuvPlane>, Error> {
    let bps = if c.hbd { 2 } else { 1 };
    let (x, y, w, h) = (
        crop.x as usize,
        crop.y as usize,
        crop.w as usize,
        crop.h as usize,
    );
    let mut out = Vec::with_capacity(4);
    let (sx, sy) = (usize::from(c.ssx), usize::from(c.ssy));
    for k in 0..3 {
        let (dx, dy) = if k == 0 { (0, 0) } else { (sx, sy) };
        let (pw, ph) = (
            (x + w).div_ceil(1 << dx) - (x >> dx),
            (y + h).div_ceil(1 << dy) - (y >> dy),
        );
        let stride = pw * bps;
        let mut data = vec![0u8; stride * ph];
        let src = &c.planes[k][(y >> dy) * c.strides[k]..];
        copy_rows(
            src,
            c.strides[k],
            (x >> dx) * bps,
            &mut data,
            stride,
            (0, 0),
            ph,
            stride,
        )?;
        out.push(YuvPlane {
            width: pw as u32,
            height: ph as u32,
            stride,
            data,
            h_div: 1 << dx,
            v_div: 1 << dy,
        });
    }
    if let Some((a, map)) = a {
        let stride = w * bps;
        let mut data = vec![0u8; stride * h];
        let src = &a.planes[0][y * a.strides[0]..];
        if map.identity && a.hbd == c.hbd {
            copy_rows(
                src,
                a.strides[0],
                x * bps,
                &mut data,
                stride,
                (0, 0),
                h,
                stride,
            )?;
        } else {
            for (r, row) in data.chunks_exact_mut(stride.max(1)).enumerate().take(h) {
                let s = src.get(r * a.strides[0]..).ok_or_else(inconsistent)?;
                for i in 0..w {
                    let v = map.map(at(s, a.hbd, x + i));
                    if c.hbd {
                        row[2 * i..2 * i + 2].copy_from_slice(&(v as u16).to_ne_bytes());
                    } else {
                        row[i] = v as u8;
                    }
                }
            }
        }
        out.push(YuvPlane {
            width: w as u32,
            height: h as u32,
            stride,
            data,
            h_div: 1,
            v_div: 1,
        });
    }
    Ok(out)
}

/// The chroma samples either side of luma sample `i` on an axis subsampled by `2^shift`
/// (`n` chroma samples) and the second one's weight in quarters, for chroma co-sited with
/// the even luma samples or midway between them (`specular_core::yuv`'s taps, which the
/// display interpolates with). Edges repeat the outermost sample.
fn quarter_tap(i: usize, n: usize, shift: usize, cosited: bool) -> (usize, usize, u32) {
    let last = n.saturating_sub(1);
    if shift == 0 {
        let c = i.min(last);
        return (c, c, 0);
    }
    // Position in chroma samples, in quarters: i / 2, less ¼ when centred.
    let q = (2 * i).saturating_sub(usize::from(!cosited));
    let (c0, w) = (q / 4, (q % 4) as u32);
    if c0 >= last {
        (last, last, 0)
    } else {
        (c0, c0 + 1, w)
    }
}

/// Box-averaged 4:4:4 planes of `crop` at 1/`s`, colour weighted by alpha when there is one.
fn reduced_planes(
    c: &Planes<'_>,
    a: Option<(&Planes<'_>, AlphaMap)>,
    crop: Crop,
    s: usize,
    parallel: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<YuvPlane>, Error> {
    let (rx, ry, rw, rh) = (
        crop.x as usize,
        crop.y as usize,
        crop.w as usize,
        crop.h as usize,
    );
    let (ow, oh) = (rw.div_ceil(s), rh.div_ceil(s));
    let wide = c.hbd;
    let bps = if wide { 2 } else { 1 };
    let (sx, sy) = (usize::from(c.ssx), usize::from(c.ssy));
    let nplanes = 3 + usize::from(a.is_some());
    let row_bytes = ow * bps;
    let mut data: Vec<Vec<u8>> = (0..nplanes).map(|_| vec![0u8; row_bytes * oh]).collect();
    // Chroma taps of every luma column and row of the region, weights in quarters: the
    // bilinear interpolation at the siting the 1:1 planes are shown with, so a reduced level
    // is the box average of the 1:1 image (as the RGB path's is).
    let (cw, ch) = (
        (c.width as usize).div_ceil(1 << sx),
        (c.height as usize).div_ceil(1 << sy),
    );
    let xt: Vec<(usize, usize, u32)> = (rx..rx + rw)
        .map(|x| quarter_tap(x, cw, sx, c.cosited_x))
        .collect();
    let yt: Vec<(usize, usize, u32)> = (ry..ry + rh)
        .map(|y| quarter_tap(y, ch, sy, c.cosited_y))
        .collect();
    let row = |oy: usize, out: &mut [&mut [u8]]| {
        let ys = ry + oy * s;
        let ye = (ys + s).min(ry + rh);
        for ox in 0..ow {
            let xs = rx + ox * s;
            let xe = (xs + s).min(rx + rw);
            // Alpha-weighted sums, and plain ones for a wholly transparent block; chroma
            // sums are 16 times the samples (bilinear weights in quarters).
            let (mut wsum, mut plain) = ([0u64; 4], [0u64; 3]);
            let mut n = 0u64;
            for y in ys..ye {
                let ly = &c.planes[0][y * c.strides[0]..];
                let (y0, y1, wy) = yt[y - ry];
                let rows = [1, 2].map(|k| {
                    (
                        &c.planes[k][y0 * c.strides[k]..],
                        &c.planes[k][y1 * c.strides[k]..],
                    )
                });
                let la = a.map(|(a, m)| (&a.planes[0][y * a.strides[0]..], a.hbd, m));
                for x in xs..xe {
                    let (x0, x1, wx) = xt[x - rx];
                    let chroma = |(r0, r1): (&[u8], &[u8])| {
                        let top = (4 - wx) * at(r0, wide, x0) + wx * at(r0, wide, x1);
                        let bottom = (4 - wx) * at(r1, wide, x0) + wx * at(r1, wide, x1);
                        (4 - wy) * top + wy * bottom
                    };
                    let v = [16 * at(ly, wide, x), chroma(rows[0]), chroma(rows[1])];
                    let al = la.map_or(1, |(r, w, m)| m.map(at(r, w, x)));
                    for k in 0..3 {
                        wsum[k] += u64::from(v[k]) * u64::from(al);
                        plain[k] += u64::from(v[k]);
                    }
                    wsum[3] += u64::from(al);
                    n += 1;
                }
            }
            let i = ox * bps;
            let put = |dst: &mut [u8], v: u64| {
                if wide {
                    dst[i..i + 2].copy_from_slice(&(v as u16).to_ne_bytes());
                } else {
                    dst[i] = v as u8;
                }
            };
            for k in 0..3 {
                let (d, n) = (16 * wsum[3], 16 * n);
                let v = (wsum[k] + d / 2)
                    .checked_div(d)
                    .unwrap_or_else(|| (plain[k] + n / 2) / n.max(1));
                put(out[k], v);
            }
            if a.is_some() {
                put(out[3], (wsum[3] + n / 2) / n.max(1));
            }
        }
    };
    // Bands of output rows: each band's rows of every plane.
    let bands = oh.div_ceil(BAND);
    let mut chunks: Vec<Vec<&mut [u8]>> = (0..bands).map(|_| Vec::with_capacity(4)).collect();
    if row_bytes > 0 {
        for plane in &mut data {
            for (b, ch) in plane.chunks_mut(row_bytes * BAND).enumerate() {
                chunks[b].push(ch);
            }
        }
    }
    let run = |b: usize, band: &mut Vec<&mut [u8]>| {
        let rows = band.first().map_or(0, |p| p.len() / row_bytes.max(1));
        for r in 0..rows {
            let mut out: Vec<&mut [u8]> = band
                .iter_mut()
                .map(|p| &mut p[r * row_bytes..(r + 1) * row_bytes])
                .collect();
            row(b * BAND + r, &mut out);
        }
    };
    if row_bytes > 0 {
        // Cancellation between groups of bands (the callback need not be `Sync`).
        let group = if parallel {
            rayon::current_num_threads().max(1) * 2
        } else {
            1
        };
        for (g, part) in chunks.chunks_mut(group).enumerate() {
            if cancel() {
                return Err(Error::Cancelled);
            }
            let base = g * group;
            if parallel {
                part.par_iter_mut()
                    .enumerate()
                    .for_each(|(i, band)| run(base + i, band));
            } else {
                for (i, band) in part.iter_mut().enumerate() {
                    run(base + i, band);
                }
            }
        }
    }
    drop(chunks);
    Ok(data
        .into_iter()
        .map(|d| YuvPlane {
            width: ow as u32,
            height: oh as u32,
            stride: row_bytes,
            data: d,
            h_div: 1,
            v_div: 1,
        })
        .collect())
}

impl AvifFile {
    /// The primary still's YCbCr planes at 1/`scale` (1, 2, 4 or 8; see the
    /// [module docs](self)), polling `cancel` between tiles and row bands. `Ok(None)` for
    /// what the planes do not carry (monochrome, pre-multiplied alpha, a sequence without a
    /// still, and the odd layouts the module docs list): [`AvifFile::decode_with`] decodes
    /// those.
    pub fn decode_planes(
        &self,
        scale: u32,
        parallel: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Option<YuvImage>, Error> {
        let (Some(still), Some(meta)) = (&self.still, &self.meta) else {
            return Ok(None);
        };
        if (still.alpha.is_some() && still.premultiplied) || is_mono(meta, &still.color) {
            return Ok(None);
        }
        let s = norm_scale(scale) as usize;
        let (full_w, full_h) = crate::still_size(meta, still)?;
        check_size(full_w, full_h)?;
        let crop = clap_of(still, full_w, full_h);
        // A crop at an odd offset along a subsampled axis moves the chroma siting off the
        // half-sample grid: known from `av1C` before anything is decoded.
        let odd_crop = |ssx: bool, ssy: bool| {
            s == 1 && crop.is_some_and(|c| (ssx && c.x % 2 == 1) || (ssy && c.y % 2 == 1))
        };
        if let Some(cfg) = crate::coded_config(meta, &still.color)
            && odd_crop(cfg.subsampling_x, cfg.subsampling_y)
        {
            return Ok(None);
        }
        let threads = if parallel { 0 } else { 1 };
        let mut dec = new_decoder_layers(threads, still.color_layers)?;
        let Some(color) = decode_source(self, meta, &still.color, dec.as_mut(), cancel)? else {
            return Ok(None);
        };
        drop(dec);
        let cp = color.planes();
        if cp.mono || odd_crop(cp.ssx, cp.ssy) {
            return Ok(None);
        }
        let (pw, ph) = (cp.width, cp.height);
        let c = crop.unwrap_or(Crop {
            x: 0,
            y: 0,
            w: pw,
            h: ph,
        });
        let region = Crop {
            x: c.x,
            y: c.y,
            w: c.w.min(pw.saturating_sub(c.x)),
            h: c.h.min(ph.saturating_sub(c.y)),
        };
        if region.w == 0 || region.h == 0 {
            return Err(Error::Malformed("crop outside the image".into()));
        }
        let (ew, eh) = (
            (region.x + region.w) as usize,
            (region.y + region.h) as usize,
        );
        check(&cp, ew, eh, true)?;
        let alpha = match &still.alpha {
            Some(src) => {
                let mut adec = new_decoder_layers(threads, still.alpha_layers)?;
                match decode_source(self, meta, src, adec.as_mut(), cancel)? {
                    Some(a) => Some(a),
                    None => return Ok(None),
                }
            }
            None => None,
        };
        let ap = alpha.as_ref().map(Decoded::planes);
        if let Some(a) = &ap {
            if (a.width as usize) < ew || (a.height as usize) < eh {
                return Ok(None);
            }
            check(a, ew, eh, false)?;
        }
        let a = ap.as_ref().map(|a| (a, AlphaMap::new(a, cp.bits)));
        if cancel() {
            return Err(Error::Cancelled);
        }
        let planes = if s == 1 {
            full_planes(&cp, a, region)?
        } else {
            reduced_planes(&cp, a, region, s, parallel, cancel)?
        };
        let (primaries, transfer, matrix, full_range) = match still.nclx {
            Some(n) => (n.primaries, n.transfer, n.matrix, n.full_range),
            None => (
                cp.cicp.primaries,
                cp.cicp.transfer,
                cp.cicp.matrix,
                cp.cicp.full_range,
            ),
        };
        Ok(Some(YuvImage {
            width: (region.w as usize).div_ceil(s) as u32,
            height: (region.h as usize).div_ceil(s) as u32,
            scale: s as u32,
            bits: cp.bits,
            matrix,
            full_range,
            primaries,
            transfer,
            cosited_x: cp.cosited_x,
            cosited_y: cp.cosited_y,
            planes,
        }))
    }

    /// Estimated peak memory in bytes for [`AvifFile::decode_planes`] at `scale`: the
    /// decoder's pictures, a grid's canvas, and the planes handed over.
    pub fn decode_planes_memory(&self, scale: u32) -> u64 {
        let s = u64::from(norm_scale(scale));
        let (w, h) = (u64::from(self.info.width), u64::from(self.info.height));
        let bps = if self.info.bit_depth > 8 { 2 } else { 1 };
        let alpha = u64::from(self.info.has_alpha);
        let factor = match self.info.chroma {
            "4:2:0" => 1.5,
            "4:2:2" => 2.0,
            _ => 3.0,
        };
        let yuv = ((w * h) as f64 * factor) as u64 * bps + alpha * w * h * bps;
        let out = if s == 1 {
            yuv
        } else {
            w.div_ceil(s) * h.div_ceil(s) * (3 + alpha) * bps
        };
        // Pictures (colour and alpha held together), plus a grid's canvas of the same size.
        let canvas = if self.info.grid.is_some() { yuv } else { 0 };
        yuv + canvas + out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");

    fn open(name: &str) -> AvifFile {
        AvifFile::open(std::fs::read(format!("{DATA}{name}")).unwrap()).unwrap()
    }

    fn sample(p: &YuvPlane, x: u32, y: u32) -> u32 {
        let o = y as usize * p.stride;
        if p.stride >= 2 * p.width as usize && p.width > 0 {
            u32::from(u16::from_ne_bytes([
                p.data[o + 2 * x as usize],
                p.data[o + 2 * x as usize + 1],
            ]))
        } else {
            u32::from(p.data[o + x as usize])
        }
    }

    /// The planes keep the coded subsampling, bits and siting; a grid's canvas equals the
    /// same image coded as one item.
    #[test]
    fn planes_follow_the_bitstream() {
        let f = open("rgb10_420.avif");
        let p = f.decode_planes(1, false, &|| false).unwrap().unwrap();
        assert_eq!((p.width, p.height), (f.info().width, f.info().height));
        assert_eq!((p.bits, p.planes.len()), (10, 3));
        assert_eq!((p.planes[1].h_div, p.planes[1].v_div), (2, 2));
        assert_eq!(p.planes[1].width, p.width.div_ceil(2));
        let c = open("siting420-colocated.avif")
            .decode_planes(1, true, &|| false)
            .unwrap()
            .unwrap();
        assert!(c.cosited_x && c.cosited_y);
        let u = open("siting420-unknown.avif")
            .decode_planes(1, true, &|| false)
            .unwrap()
            .unwrap();
        assert!(!u.cosited_x && !u.cosited_y);
        let k = open("siting422.avif")
            .decode_planes(1, true, &|| false)
            .unwrap()
            .unwrap();
        assert_eq!((k.planes[1].h_div, k.planes[1].v_div), (2, 1));
        for name in ["grid420-unknown", "grid420-colocated", "grid422"] {
            let g = open(&format!("{name}.avif"))
                .decode_planes(1, true, &|| false)
                .unwrap()
                .unwrap();
            let one = open(&format!("{name}.single.avif"))
                .decode_planes(1, true, &|| false)
                .unwrap()
                .unwrap();
            assert_eq!(g, one, "{name}");
        }
        // Monochrome comes as RGB.
        assert!(
            open("gray.avif")
                .decode_planes(1, true, &|| false)
                .unwrap()
                .is_none()
        );
    }

    /// The clean aperture: the planes of the uncropped single image, cut out.
    #[test]
    fn crops_cut_the_planes() {
        let g = open("grid420-cropped.avif");
        let one = open("grid420-cropped.single.avif");
        let a = g.decode_planes(1, true, &|| false).unwrap();
        let b = one.decode_planes(1, true, &|| false).unwrap();
        assert_eq!(a, b);
        let a = a.expect("an even crop offset");
        assert_eq!((a.width, a.height), (g.info().width, g.info().height));
    }

    /// A `clap` crop at an even offset cuts the planes out of the uncropped ones; at an odd
    /// one the 1:1 planes are refused (the chroma siting would move), reduced ones not.
    #[test]
    fn clap_crops() {
        let plain = open("rgb8_420.avif");
        let whole = plain.decode_planes(1, true, &|| false).unwrap().unwrap();
        let (w, h) = (whole.width, whole.height);
        let with_clap = |ox: u32, oy: u32| {
            let mut f = open("rgb8_420.avif");
            let still = f.still.as_mut().unwrap();
            // Four pixels narrower and shorter, moved by (ox, oy) from the centre.
            still.clap = Some([w - 4, 1, h - 4, 1, ox, 1, oy, 1]);
            f
        };
        let f = with_clap(0, 0);
        let p = f.decode_planes(1, true, &|| false).unwrap().unwrap();
        assert_eq!((p.width, p.height), (w - 4, h - 4));
        for (k, (q, o)) in p.planes.iter().zip(&whole.planes).enumerate() {
            let d = if k == 0 { 2 } else { 1 };
            for y in 0..q.height {
                for x in 0..q.width {
                    assert_eq!(sample(q, x, y), sample(o, x + d, y + d), "plane {k}");
                }
            }
        }
        let odd = with_clap(1, 0);
        assert!(odd.decode_planes(1, true, &|| false).unwrap().is_none());
        let r = odd.decode_planes(2, true, &|| false).unwrap().unwrap();
        assert_eq!(
            (r.width, r.height),
            ((w - 4).div_ceil(2), (h - 4).div_ceil(2))
        );
    }

    /// Alpha comes as a fourth full-size plane; at 1/s every plane is 4:4:4 at the output
    /// size, each sample its block's (alpha-weighted) mean.
    #[test]
    fn alpha_and_reduced_levels() {
        let f = open("alpha.avif");
        let full = f.decode_planes(1, false, &|| false).unwrap().unwrap();
        assert_eq!(full.planes.len(), 4);
        let a = &full.planes[3];
        assert_eq!((a.width, a.height), (full.width, full.height));
        for s in [2u32, 4, 8] {
            for parallel in [false, true] {
                let r = f.decode_planes(s, parallel, &|| false).unwrap().unwrap();
                assert_eq!(
                    (r.width, r.height, r.scale),
                    (full.width.div_ceil(s), full.height.div_ceil(s), s)
                );
                for p in &r.planes {
                    assert_eq!(
                        (p.width, p.height, p.h_div, p.v_div),
                        (r.width, r.height, 1, 1)
                    );
                }
                // Alpha is the plain block mean.
                for oy in 0..r.height {
                    for ox in 0..r.width {
                        let (mut sum, mut n) = (0, 0);
                        for y in oy * s..(oy * s + s).min(full.height) {
                            for x in ox * s..(ox * s + s).min(full.width) {
                                sum += sample(a, x, y);
                                n += 1;
                            }
                        }
                        assert_eq!(sample(&r.planes[3], ox, oy), (sum + n / 2) / n);
                    }
                }
            }
        }
        let cancelled = f.decode_planes(2, true, &|| true);
        assert_eq!(cancelled, Err(Error::Cancelled));
    }
}
