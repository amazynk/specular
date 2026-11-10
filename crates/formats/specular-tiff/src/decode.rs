//! Region / scale decode pipeline.
//!
//! The output region (in scaled pixels) is covered by *items*: groups of `gx × gy` strips or
//! tiles whose source extent is a multiple of the residual scale factor, so every output pixel
//! belongs to exactly one item. Items are decoded independently (in parallel), each into a
//! small buffer of the output format that is then copied or box-filtered into its own disjoint slices of the
//! output, so no locking or unsafe code is needed.

use crate::convert::{Converter, box_reduce, fill_missing};
use crate::{
    Compression, DecodeOptions, Decoded, Error, Ifd, MAX_OUTPUT_BYTES, Page, Photometric,
    PixelFormat, Region, Result, row_bytes,
};
use crate::{ccitt, codec, imgcodec, lerc, variants};
use rayon::prelude::*;
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

/// Pixels per band when one unit's rows are converted in parallel: a few hundred µs of
/// work for the costlier conversions (CMYK through a profile, Lab, YCbCr, 16-bit).
const ROW_BAND_PIXELS: usize = 1 << 16;

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Pick the coarsest stored level whose factor `f` (a power of two) divides `scale`.
fn choose_level(p: &Page, scale: u32) -> (usize, u32) {
    let (w, h) = (p.width(), p.height());
    let mut best = (0, 1);
    for (i, l) in p.levels.iter().enumerate().skip(1) {
        let mut f = 2;
        while f <= scale {
            // Pyramids written tile by tile (whole-slide images) round their levels' sizes
            // to tiles or floor them: a pixel off per thousand still counts.
            let (vw, vh) = (w.div_ceil(f), h.div_ceil(f));
            if l.width.abs_diff(vw) <= 1.max(vw / 1000)
                && l.height.abs_diff(vh) <= 1.max(vh / 1000)
                && f > best.1
            {
                best = (i, f);
            }
            f *= 2;
        }
    }
    best
}

/// Geometry of one decode at one level.
/// How far a JPEG unit's frame may exceed its tile or strip (MCU padding, sloppy writers).
const JPEG_UNIT_SLACK: u32 = 64;

struct Geo {
    /// Stored level size.
    lw: u32,
    lh: u32,
    /// Level size implied by the page size (may differ by one pixel from the stored size).
    vw: u32,
    vh: u32,
    rf: u32,
    uw: u32,
    uh: u32,
    ucols: u32,
    urows: u32,
    iw_src: u32,
    ih_src: u32,
    iw_out: u32,
    ih_out: u32,
}

impl Geo {
    fn new(l: &Ifd, vw: u32, vh: u32, rf: u32) -> Geo {
        let (uw, uh) = (l.unit_w, l.unit_h);
        let (dx, dy) = (gcd(uw, rf), gcd(uh, rf));
        let (gx, gy) = (rf / dx, rf / dy);
        Geo {
            lw: l.width,
            lh: l.height,
            vw,
            vh,
            rf,
            uw,
            uh,
            ucols: l.width.div_ceil(uw),
            urows: l.height.div_ceil(uh),
            // `gx * uw` overflows for a wide odd strip width at a large scale (4194303 px at
            // 1/2048); it is only compared and clamped to the level size, so saturate it.
            iw_src: gx.saturating_mul(uw),
            ih_src: gy.saturating_mul(uh),
            // = gx * uw / rf, exactly.
            iw_out: uw / dx,
            ih_out: uh / dy,
        }
    }
}

fn clip(region: Option<Region>, w: u32, h: u32) -> Result<Region> {
    let r = region.unwrap_or(Region { x: 0, y: 0, w, h });
    if r.x >= w || r.y >= h || r.w == 0 || r.h == 0 {
        return Err(Error::BadRegion);
    }
    Ok(Region {
        x: r.x,
        y: r.y,
        w: r.w.min(w - r.x),
        h: r.h.min(h - r.y),
    })
}

fn alloc(n: u64) -> Result<Vec<u8>> {
    if n > MAX_OUTPUT_BYTES || n > isize::MAX as u64 {
        return Err(Error::TooLarge);
    }
    let mut v = Vec::new();
    v.try_reserve_exact(n as usize)
        .map_err(|_| Error::TooLarge)?;
    v.resize(n as usize, 0);
    Ok(v)
}

pub(crate) fn decode_page(
    p: &Page,
    data: &[u8],
    file_le: bool,
    opts: &DecodeOptions,
) -> Result<Decoded> {
    decode_as(p, data, file_le, opts, p.format_for(opts), 0, None)
}

/// [`crate::Tiff::decode_values`].
pub(crate) fn decode_page_values(
    p: &Page,
    data: &[u8],
    file_le: bool,
    band: usize,
    nodata: Option<f64>,
    opts: &DecodeOptions,
) -> Result<Decoded> {
    decode_as(p, data, file_le, opts, PixelFormat::GrayF32, band, nodata)
}

fn decode_as(
    p: &Page,
    data: &[u8],
    file_le: bool,
    opts: &DecodeOptions,
    fmt: PixelFormat,
    band: usize,
    nodata: Option<f64>,
) -> Result<Decoded> {
    p.check_supported()?;
    let s = opts.scale.max(1);
    if !s.is_power_of_two() {
        return Err(Error::Unsupported(format!("scale 1/{s}")));
    }
    let (w, h) = (p.width(), p.height());
    let reg = clip(opts.region, w.div_ceil(s), h.div_ceil(s))?;
    let (li, f) = choose_level(p, s);
    let level = &p.levels[li];
    let bpp = fmt.bytes_per_pixel();
    let mut out = alloc(reg.w as u64 * reg.h as u64 * bpp as u64)?;
    let geo = Geo::new(level, w.div_ceil(f), h.div_ceil(f), s / f);
    let ctx = Ctx {
        ifd: level,
        data,
        conv: Converter {
            ifd: level,
            target: fmt,
            le: file_le,
            cmyk: (level.photometric == crate::Photometric::Cmyk)
                .then(|| crate::cmyk_hook::converter(p.icc_profile(data)))
                .flatten(),
            // L*a*b* is device-independent: an embedded profile does not change it.
            lab: level
                .photometric
                .is_lab()
                .then(crate::lab_hook::converter)
                .flatten(),
            lab_coding: level.lab_coding(file_le),
            band,
            nodata,
        },
        fmt,
        bpp,
        geo,
        cancel: opts.cancel,
        truncated: AtomicBool::new(false),
        single_unit: level.offsets.len() <= 1,
    };
    ctx.run(reg, &mut out, opts.parallel)?;
    Ok(Decoded {
        width: reg.w,
        height: reg.h,
        origin: (reg.x, reg.y),
        format: fmt,
        stride: reg.w as usize * bpp,
        data: out,
        truncated: ctx.truncated.load(Ordering::Relaxed),
    })
}

pub(crate) fn memory_estimate(p: &Page, opts: &DecodeOptions) -> u64 {
    let s = opts.scale.max(1);
    let (w, h) = (p.width(), p.height());
    let Ok(reg) = clip(opts.region, w.div_ceil(s), h.div_ceil(s)) else {
        return 0;
    };
    let (li, f) = choose_level(p, s);
    let l = &p.levels[li];
    let bpp = p.format_for(opts).bytes_per_pixel() as u64;
    let geo = Geo::new(l, w.div_ceil(f), h.div_ceil(f), s / f);
    let out = reg.w as u64 * reg.h as u64 * bpp;
    let planes = if l.planar { l.samples as u64 } else { 1 };
    let unit_raw = row_bytes(l.unit_w, l.bits, if l.planar { 1 } else { l.samples })
        * l.unit_h as u64
        * planes;
    // JPEG units are decoded to planes plus an RGBA copy.
    let unit = if l.compression.is_jpeg() || l.compression.is_image_codec() {
        l.unit_w as u64 * l.unit_h as u64 * 7
    } else {
        unit_raw
    };
    let item = geo.iw_src.min(geo.lw) as u64 * geo.ih_src.min(geo.lh) as u64 * bpp;
    let items = (reg.w.div_ceil(geo.iw_out) as u64 + 1) * (reg.h.div_ceil(geo.ih_out) as u64 + 1);
    let threads = if opts.parallel {
        rayon::current_num_threads() as u64
    } else {
        1
    };
    out + threads.min(items) * (unit + item)
}

struct Ctx<'a> {
    ifd: &'a Ifd,
    data: &'a [u8],
    conv: Converter<'a>,
    fmt: PixelFormat,
    bpp: usize,
    geo: Geo,
    cancel: Option<&'a AtomicBool>,
    truncated: AtomicBool,
    /// The whole level is one strip: let JPEG decode it in parallel itself.
    single_unit: bool,
}

struct Item<'o> {
    ix: u32,
    iy: u32,
    rows: Vec<&'o mut [u8]>,
}

/// An item's source pixels at its level: the stored ones (`x0..x1`, `y0..y1`) and the
/// virtual size (`w` × `h`, from `x0`, `y0`) whose padding repeats the last stored column
/// and row.
struct Src {
    x0: u32,
    x1: u32,
    y0: u32,
    y1: u32,
    w: usize,
    h: usize,
}

/// A rectangle of decoded pixels to place: unit-local rows/cols and where they go.
struct Place {
    /// Rows and columns of the unit (unit-local).
    ry0: u32,
    ry1: u32,
    cx0: u32,
    cx1: u32,
    /// Destination top-left in the item buffer.
    dx: u32,
    dy: u32,
}

impl Ctx<'_> {
    fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|c| c.load(Ordering::Relaxed))
    }

    fn run(&self, reg: Region, out: &mut [u8], parallel: bool) -> Result<()> {
        let g = &self.geo;
        let bpp = self.bpp;
        let ix0 = reg.x / g.iw_out;
        let ix1 = (reg.x + reg.w - 1) / g.iw_out;
        let iy0 = reg.y / g.ih_out;
        let iy1 = (reg.y + reg.h - 1) / g.ih_out;
        let nix = (ix1 - ix0 + 1) as usize;
        let mut items: Vec<Item> = (iy0..=iy1)
            .flat_map(|iy| {
                (ix0..=ix1).map(move |ix| Item {
                    ix,
                    iy,
                    rows: Vec::new(),
                })
            })
            .collect();
        let stride = reg.w as usize * bpp;
        for (y, row) in out.chunks_mut(stride).enumerate() {
            let oy = reg.y + y as u32;
            let base = ((oy / g.ih_out - iy0) as usize) * nix;
            let mut rest = row;
            for k in 0..nix {
                let ix = ix0 + k as u32;
                let xs = (ix * g.iw_out).max(reg.x);
                let xe = ((ix + 1) * g.iw_out).min(reg.x + reg.w);
                let (a, b) = std::mem::take(&mut rest).split_at_mut((xe - xs) as usize * bpp);
                items[base + k].rows.push(a);
                rest = b;
            }
        }
        // Fewer items than threads (one large strip, a few big tiles): their rows are
        // converted in parallel bands too.
        let rows_parallel = parallel && items.len() < rayon::current_num_threads();
        let work = |it: Item| self.item(reg, it, rows_parallel);
        if parallel && items.len() > 1 {
            items.into_par_iter().try_for_each(work)
        } else {
            items.into_iter().try_for_each(work)
        }
    }

    fn item(&self, reg: Region, mut it: Item, rows_parallel: bool) -> Result<()> {
        if self.cancelled() {
            return Err(Error::Cancelled);
        }
        let g = &self.geo;
        let bpp = self.bpp;
        let ox0 = (it.ix * g.iw_out).max(reg.x);
        let ox1 = ((it.ix + 1) * g.iw_out).min(reg.x + reg.w);
        let oy0 = (it.iy * g.ih_out).max(reg.y);
        let oy1 = ((it.iy + 1) * g.ih_out).min(reg.y + reg.h);

        // JPEG tiles at a reduced scale: let the IDCT do the scaling.
        if self.ifd.compression.is_jpeg()
            && self.fmt != PixelFormat::GrayF32
            && g.rf > 1
            && g.rf <= 8
            && g.iw_src == g.uw
            && g.ih_src == g.uh
            && g.lw == g.vw
            && g.lh == g.vh
        {
            return self.jpeg_scaled_item(&mut it, ox0, ox1, oy0);
        }

        let sx0 = ox0 * g.rf;
        let sx1 = (ox1 * g.rf).min(g.vw);
        let sy0 = oy0 * g.rf;
        let sy1 = (oy1 * g.rf).min(g.vh);
        let ax0 = sx0.min(g.lw - 1);
        let ax1 = sx1.min(g.lw);
        let ay0 = sy0.min(g.lh - 1);
        let ay1 = sy1.min(g.lh);
        // One unit covers the item at a reduced scale (one large strip, or a tile per item):
        // convert and reduce band by band, never holding the full-resolution item.
        if g.rf > 1
            && !self.ifd.compression.is_jpeg()
            && (sx0, sy0) == (ax0, ay0)
            && ax0 / g.uw == (ax1 - 1) / g.uw
            && ay0 / g.uh == (ay1 - 1) / g.uh
        {
            let src = Src {
                x0: ax0,
                x1: ax1,
                y0: ay0,
                y1: ay1,
                w: (sx1 - sx0) as usize,
                h: (sy1 - sy0) as usize,
            };
            if self.reduce_unit(&src, &mut it.rows, rows_parallel)? {
                return Ok(());
            }
        }
        let bw = (sx1 - ax0) as usize;
        let bh = (sy1 - ay0) as usize;
        let bstride = bw * bpp;
        let mut buf = vec![0u8; bw * bh * bpp];
        let mut scratch = Vec::new();
        for uy in ay0 / g.uh..=(ay1 - 1) / g.uh {
            for ux in ax0 / g.uw..=(ax1 - 1) / g.uw {
                if self.cancelled() {
                    return Err(Error::Cancelled);
                }
                let (ux0, uy0) = (ux * g.uw, uy * g.uh);
                let pl = Place {
                    ry0: ay0.max(uy0) - uy0,
                    ry1: ay1.min(uy0 + g.uh) - uy0,
                    cx0: ax0.max(ux0) - ux0,
                    cx1: ax1.min(ux0 + g.uw) - ux0,
                    dx: ax0.max(ux0) - ax0,
                    dy: ay0.max(uy0) - ay0,
                };
                self.unit(ux, uy, &pl, &mut buf, bstride, &mut scratch, rows_parallel);
            }
        }
        // Replicate the last stored column / row into the virtual padding.
        let (aw, ah) = ((ax1 - ax0) as usize, (ay1 - ay0) as usize);
        if aw < bw {
            for y in 0..ah {
                let r = &mut buf[y * bstride..(y + 1) * bstride];
                let last = (aw - 1) * bpp;
                for x in aw..bw {
                    r.copy_within(last..last + bpp, x * bpp);
                }
            }
        }
        for y in ah..bh {
            buf.copy_within((ah - 1) * bstride..ah * bstride, y * bstride);
        }
        let off = (sy0 - ay0) as usize * bstride + (sx0 - ax0) as usize * bpp;
        let (sw, sh) = ((sx1 - sx0) as usize, (sy1 - sy0) as usize);
        if g.rf == 1 {
            for (j, d) in it.rows.iter_mut().enumerate() {
                let s = off + j * bstride;
                d.copy_from_slice(&buf[s..s + sw * bpp]);
            }
        } else {
            box_reduce(
                &buf[off..],
                bstride,
                sw,
                sh,
                self.fmt,
                g.rf as usize,
                &mut it.rows,
            );
        }
        Ok(())
    }

    /// [`Ctx::item`] at a reduced scale when one unit holds all of the item's stored pixels
    /// (`src`): the unit is decompressed once, then bands of whole output rows (in parallel
    /// if `rows_parallel`) convert their source rows into a small buffer and box-filter it
    /// straight into the output, with the same result as converting the whole item first.
    /// `Ok(false)`: the unit is missing or sparse, left to the general path.
    fn reduce_unit(&self, src: &Src, rows: &mut [&mut [u8]], rows_parallel: bool) -> Result<bool> {
        let g = &self.geo;
        let bpp = self.bpp;
        let rf = g.rf as usize;
        let (ux, uy) = (src.x0 / g.uw, src.y0 / g.uh);
        let (ux0, uy0) = (ux * g.uw, uy * g.uh);
        let uidx = (uy * g.ucols + ux) as usize;
        let Ok(raws) = self.unit_raws(uidx, src.y0 - uy0, src.y1 - uy0) else {
            return Ok(false);
        };
        if self.cancelled() {
            return Err(Error::Cancelled);
        }
        let rb = self.unit_row_bytes();
        let (cx0, cx1) = ((src.x0 - ux0) as usize, (src.x1 - ux0) as usize);
        let aw = cx1 - cx0;
        let bstride = src.w * bpp;
        // Output rows per band: about ROW_BAND_PIXELS source pixels.
        let band = (ROW_BAND_PIXELS / (src.w * rf).max(1)).max(1);
        let work = |j0: usize, out: &mut [&mut [u8]], buf: &mut Vec<u8>, scratch: &mut Vec<u8>| {
            let y0 = j0 * rf;
            let y1 = ((j0 + out.len()) * rf).min(src.h);
            if y0 >= y1 {
                return;
            }
            buf.clear();
            buf.resize((y1 - y0) * bstride, 0);
            let mut planes: Vec<&[u8]> = Vec::with_capacity(raws.len());
            for (y, d) in (y0..y1).zip(buf.chunks_exact_mut(bstride)) {
                // Rows past the stored ones repeat the last.
                let r = (src.y0 as usize + y).min(src.y1 as usize - 1) - uy0 as usize;
                planes.clear();
                planes.extend(raws.iter().map(|raw| &raw[r * rb..(r + 1) * rb]));
                self.conv
                    .row(&planes, cx0, cx1, &mut d[..aw * bpp], scratch);
                // Columns past the stored ones repeat the last.
                let last = (aw - 1) * bpp;
                for x in aw..src.w {
                    d.copy_within(last..last + bpp, x * bpp);
                }
            }
            box_reduce(buf, bstride, src.w, y1 - y0, self.fmt, rf, out);
        };
        if rows_parallel && rows.len() > band {
            rows.par_chunks_mut(band).enumerate().for_each_init(
                || (Vec::new(), Vec::new()),
                |(buf, scratch), (b, out)| work(b * band, out, buf, scratch),
            );
        } else {
            let (mut buf, mut scratch) = (Vec::new(), Vec::new());
            for (b, out) in rows.chunks_mut(band).enumerate() {
                if self.cancelled() {
                    return Err(Error::Cancelled);
                }
                work(b * band, out, &mut buf, &mut scratch);
            }
        }
        Ok(true)
    }

    /// Byte range of unit `idx` in the file; `None` if missing, empty range if sparse.
    fn unit_bytes(&self, idx: usize) -> Option<&[u8]> {
        let off = *self.ifd.offsets.get(idx)?;
        let cnt = *self.ifd.counts.get(idx)?;
        if cnt == 0 {
            return Some(&[]);
        }
        let len = self.data.len() as u64;
        if off >= len {
            return None;
        }
        let end = off.saturating_add(cnt).min(len);
        Some(&self.data[off as usize..end as usize])
    }

    fn mark_truncated(&self) {
        self.truncated.store(true, Ordering::Relaxed);
    }

    /// Fill a placement with missing-data colour.
    fn fill(&self, pl: &Place, buf: &mut [u8], bstride: usize, grey: bool) {
        for r in 0..(pl.ry1 - pl.ry0) as usize {
            let s = (pl.dy as usize + r) * bstride + pl.dx as usize * self.bpp;
            let n = (pl.cx1 - pl.cx0) as usize * self.bpp;
            fill_missing(&mut buf[s..s + n], self.fmt, grey);
        }
    }

    /// Decode one strip / tile (all planes) and convert the placement's pixels into `buf`
    /// (in parallel row bands if `rows_parallel` and the placement is large).
    #[allow(clippy::too_many_arguments)]
    fn unit(
        &self,
        ux: u32,
        uy: u32,
        pl: &Place,
        buf: &mut [u8],
        bstride: usize,
        scratch: &mut Vec<u8>,
        rows_parallel: bool,
    ) {
        let ifd = self.ifd;
        let g = &self.geo;
        let uidx = (uy * g.ucols + ux) as usize;
        if ifd.compression.is_jpeg() {
            let Some(src) = self.unit_bytes(uidx) else {
                self.mark_truncated();
                return self.fill(pl, buf, bstride, true);
            };
            if src.is_empty() {
                return self.fill(pl, buf, bstride, false);
            }
            let got = if ifd.planar && ifd.samples > 1 {
                self.jpeg_planar_unit(uidx)
            } else {
                self.jpeg_unit(src, uidx, specular_jpeg::Scale::Full)
            };
            match got {
                Some((px, w, h)) => {
                    let pw = w as usize * self.bpp;
                    for r in pl.ry0..pl.ry1 {
                        let d = (pl.dy + r - pl.ry0) as usize * bstride + pl.dx as usize * self.bpp;
                        let n = (pl.cx1 - pl.cx0) as usize * self.bpp;
                        if r < h && pl.cx1 <= w {
                            let s = r as usize * pw + pl.cx0 as usize * self.bpp;
                            buf[d..d + n].copy_from_slice(&px[s..s + n]);
                        } else {
                            fill_missing(&mut buf[d..d + n], self.fmt, true);
                        }
                    }
                }
                None => self.fill(pl, buf, bstride, true),
            }
            return;
        }
        let planes = if ifd.planar { ifd.samples as usize } else { 1 };
        let rb = self.unit_row_bytes();
        let raws = match self.unit_raws(uidx, pl.ry0, pl.ry1) {
            Ok(r) => r,
            Err(grey) => return self.fill(pl, buf, bstride, grey),
        };
        // Rows `r0..` of the placement into `dst` (whole rows of `buf` from row r0's).
        let (x0, x1) = (pl.cx0 as usize, pl.cx1 as usize);
        let (dx, n) = (pl.dx as usize * self.bpp, (x1 - x0) * self.bpp);
        let convert = |r0: usize, dst: &mut [u8], scratch: &mut Vec<u8>| {
            let mut rows: Vec<&[u8]> = Vec::with_capacity(planes);
            for (j, d) in dst.chunks_mut(bstride).enumerate() {
                rows.clear();
                let s = (r0 + j) * rb;
                rows.extend(raws.iter().map(|raw| &raw[s..s + rb]));
                self.conv.row(&rows, x0, x1, &mut d[dx..dx + n], scratch);
            }
        };
        let nrows = (pl.ry1 - pl.ry0) as usize;
        let start = pl.dy as usize * bstride;
        let end = (start + nrows * bstride).min(buf.len());
        let dst = &mut buf[start..end];
        let band = ROW_BAND_PIXELS.div_ceil(x1 - x0).max(1);
        if rows_parallel && nrows > band {
            dst.par_chunks_mut(band * bstride)
                .enumerate()
                .for_each_init(Vec::new, |scratch, (b, d)| {
                    convert(pl.ry0 as usize + b * band, d, scratch);
                });
        } else {
            convert(pl.ry0 as usize, dst, scratch);
        }
    }

    /// Bytes in one row of a unit's plane.
    fn unit_row_bytes(&self) -> usize {
        let ifd = self.ifd;
        row_bytes(
            self.geo.uw,
            ifd.bits,
            if ifd.planar { 1 } else { ifd.samples },
        ) as usize
    }

    /// Unit `uidx` (not JPEG) decompressed, every plane, rows `..ry1` (the predictor undone
    /// from `ry0`). `Err(grey)`: the unit is missing (`true`, marked truncated) or sparse.
    fn unit_raws(
        &self,
        uidx: usize,
        ry0: u32,
        ry1: u32,
    ) -> std::result::Result<Vec<Cow<'_, [u8]>>, bool> {
        let ifd = self.ifd;
        let g = &self.geo;
        let per_plane = (g.ucols * g.urows) as usize;
        let planes = if ifd.planar { ifd.samples as usize } else { 1 };
        let rb = self.unit_row_bytes();
        let need = ry1 as usize * rb;
        let spp = if ifd.planar { 1 } else { ifd.samples };
        let stored_spp = if ifd.planar { 1 } else { ifd.stored_samples() };
        let mut raws = Vec::with_capacity(planes);
        for p in 0..planes {
            match self.unit_bytes(p * per_plane + uidx) {
                None => {
                    self.mark_truncated();
                    return Err(true);
                }
                Some([]) => return Err(false),
                Some(src) => match ifd.stored {
                    None => raws.push(self.decompress(src, need, rb, ry0 as usize)),
                    Some(st) => {
                        let (sneed, srb) = st.need(g.uw, ry1, stored_spp);
                        let raw = self.decompress(src, sneed, srb, ry0 as usize);
                        let mut out = vec![0u8; need];
                        variants::expand(st, &raw, g.uw, ry1, spp, p, self.conv.le, &mut out);
                        raws.push(Cow::Owned(out));
                    }
                },
            }
        }
        Ok(raws)
    }

    /// Decompress the first `need` bytes of a unit, undo the predictor on rows from `first_row`.
    /// The result is always `need` bytes long (missing data is zero and marks truncation).
    fn decompress<'d>(
        &self,
        src: &'d [u8],
        need: usize,
        rb: usize,
        first_row: usize,
    ) -> Cow<'d, [u8]> {
        // libtiff, the one TIFF JBIG writer, reverses the entity's bits when FillOrder is 1
        // (and its reader undoes that); with FillOrder 2 it leaves them.
        let reversed = if self.ifd.compression == Compression::Jbig {
            !self.ifd.lsb_fill
        } else {
            self.ifd.lsb_fill
        };
        if reversed {
            let mut rev = src.to_vec();
            codec::reverse_bits(&mut rev);
            return Cow::Owned(self.decompress_msb(&rev, need, rb, first_row).into_owned());
        }
        self.decompress_msb(src, need, rb, first_row)
    }

    fn decompress_msb<'d>(
        &self,
        src: &'d [u8],
        need: usize,
        rb: usize,
        first_row: usize,
    ) -> Cow<'d, [u8]> {
        let ifd = self.ifd;
        let mut out: Cow<[u8]> = match ifd.compression {
            Compression::None if src.len() >= need => Cow::Borrowed(&src[..need]),
            _ => {
                let mut v = vec![0u8; need];
                let n = match ifd.compression {
                    Compression::None => {
                        let n = src.len().min(need);
                        v[..n].copy_from_slice(&src[..n]);
                        n
                    }
                    Compression::PackBits => codec::packbits(src, &mut v),
                    Compression::Lzw => codec::lzw(src, &mut v),
                    Compression::Deflate => codec::inflate(src, &mut v),
                    Compression::Zstd => codec::zstd(src, &mut v),
                    Compression::Lzma => codec::xz(src, &mut v),
                    Compression::CcittRle { .. } | Compression::Group3 | Compression::Group4 => {
                        let mode = match ifd.compression {
                            Compression::CcittRle {
                                word_aligned: false,
                            } => ccitt::Mode::Rle,
                            Compression::CcittRle { word_aligned: true } => ccitt::Mode::RleWord,
                            Compression::Group3 => ccitt::Mode::G3 {
                                two_d: ifd.fax_options & 1 != 0,
                            },
                            _ => ccitt::Mode::G4,
                        };
                        let (n, damaged) = ccitt::decode(src, mode, self.geo.uw, need / rb, &mut v);
                        if damaged {
                            self.mark_truncated();
                        }
                        n
                    }
                    Compression::Lerc => lerc::decode(src, ifd, self.conv.le, self.geo.uw, &mut v),
                    // LogLuv24 is three raw bytes a pixel.
                    Compression::SgiLog24 if ifd.stored == Some(variants::Stored::LogLuv24) => {
                        let n = src.len().min(need);
                        v[..n].copy_from_slice(&src[..n]);
                        n
                    }
                    Compression::SgiLog | Compression::SgiLog24 => {
                        let bytes = if ifd.stored == Some(variants::Stored::LogL) {
                            2
                        } else {
                            4
                        };
                        variants::sgilog(src, &mut v, rb, bytes)
                    }
                    Compression::ThunderScan => {
                        variants::thunderscan(src, &mut v, rb, self.geo.uw as usize)
                    }
                    Compression::Next => variants::next(src, &mut v, rb, self.geo.uw as usize),
                    Compression::PixarLog => codec::inflate(src, &mut v),
                    Compression::Jbig => codec::jbig(src, &mut v, rb),
                    Compression::Jpeg16 => self.jpeg16(src, &mut v, rb),
                    c if c.is_image_codec() => {
                        imgcodec::decode(src, ifd, self.conv.le, self.geo.uw, &mut v, self.cancel)
                    }
                    _ => 0,
                };
                if n < need {
                    self.mark_truncated();
                }
                Cow::Owned(v)
            }
        };
        // Borrowed data must be copied for the predictor.
        if let Cow::Borrowed(b) = &out
            && ifd.predictor != 1
        {
            out = Cow::Owned(b.to_vec());
        }
        if ifd.predictor != 1 {
            let o = out.to_mut();
            let stride = if ifd.planar { 1 } else { ifd.samples as usize };
            let mut tmp = Vec::new();
            for r in first_row..need / rb {
                let row = &mut o[r * rb..(r + 1) * rb];
                if ifd.predictor == 2 {
                    codec::unpredict_horizontal(row, ifd.stored_bits(), stride, self.conv.le);
                } else {
                    codec::unpredict_float(
                        row,
                        ifd.stored_bits() as usize / 8,
                        stride,
                        self.conv.le,
                        &mut tmp,
                    );
                }
            }
        }
        out
    }

    /// A 16-bit JPEG unit (lossless, SOF3) at full precision: the components as they are
    /// (libtiff applies no colour conversion to them), written into `out` as the unit's raw
    /// chunky samples in the file's byte order, `row` bytes a row. Returns the bytes written.
    fn jpeg16(&self, src: &[u8], out: &mut [u8], row: usize) -> usize {
        let tables = self.ifd.jpeg_tables.clone().and_then(|r| self.data.get(r));
        let stream: Cow<[u8]> = match tables {
            Some(t)
                if t.len() > 4
                    && t.starts_with(&[0xFF, 0xD8])
                    && src.starts_with(&[0xFF, 0xD8]) =>
            {
                let t = t.strip_suffix(&[0xFF, 0xD9]).unwrap_or(t);
                let mut v = Vec::with_capacity(t.len() + src.len());
                v.extend_from_slice(t);
                v.extend_from_slice(&src[2..]);
                Cow::Owned(v)
            }
            _ => Cow::Borrowed(src),
        };
        let Ok(img) = specular_jpeg::decode_wide(&stream, self.cancel) else {
            return 0;
        };
        if img.truncated {
            self.mark_truncated();
        }
        let spp = if self.ifd.planar {
            1
        } else {
            self.ifd.samples as usize
        };
        let uw = self.geo.uw as usize;
        if img.planes.len() != spp || row != uw * spp * 2 {
            return 0;
        }
        let le = self.conv.le;
        let rows = (out.len() / row).min(img.height as usize);
        for (y, r) in out.chunks_exact_mut(row).take(rows).enumerate() {
            for (c, p) in img.planes.iter().enumerate() {
                // Subsampled components (not written for lossless TIFF) repeat their samples.
                let sy =
                    (y * p.height as usize / img.height.max(1) as usize).min(p.height as usize - 1);
                for x in 0..uw.min(img.width as usize) {
                    let sx = x * p.width as usize / img.width.max(1) as usize;
                    let v = p.data.get(sy * p.width as usize + sx).copied().unwrap_or(0);
                    let d = &mut r[(x * spp + c) * 2..][..2];
                    d.copy_from_slice(&if le { v.to_le_bytes() } else { v.to_be_bytes() });
                }
            }
        }
        rows * row
    }

    /// A planar JPEG unit: one single-component JPEG per sample (grey or RGB, then alpha),
    /// each decoded and interleaved into the output format.
    fn jpeg_planar_unit(&self, uidx: usize) -> Option<(Vec<u8>, u32, u32)> {
        let ifd = self.ifd;
        let g = &self.geo;
        let per_plane = (g.ucols * g.urows) as usize;
        let opts = specular_jpeg::DecodeOptions {
            scale: specular_jpeg::Scale::Full,
            region: None,
            cancel: self.cancel,
            parallelism: specular_jpeg::Parallelism::Single,
        };
        let tables = ifd.jpeg_tables.clone().and_then(|r| self.data.get(r));
        let mut planes: Vec<Vec<u8>> = Vec::with_capacity(usize::from(ifd.samples));
        let (mut w, mut h) = (0u32, 0u32);
        for p in 0..usize::from(ifd.samples) {
            let src = self.unit_bytes(p * per_plane + uidx)?;
            let stream: Cow<[u8]> = match tables {
                Some(t)
                    if t.len() > 4
                        && t.starts_with(&[0xFF, 0xD8])
                        && src.starts_with(&[0xFF, 0xD8]) =>
                {
                    let t = t.strip_suffix(&[0xFF, 0xD9]).unwrap_or(t);
                    let mut v = Vec::with_capacity(t.len() + src.len());
                    v.extend_from_slice(t);
                    v.extend_from_slice(&src[2..]);
                    Cow::Owned(v)
                }
                _ => Cow::Borrowed(src),
            };
            let info = specular_jpeg::read_info(&stream).ok()?;
            if info.width > g.uw.saturating_add(JPEG_UNIT_SLACK)
                || info.height > g.uh.saturating_add(JPEG_UNIT_SLACK)
            {
                self.mark_truncated();
                return None;
            }
            let d = specular_jpeg::decode(&stream, &opts).ok()?;
            if d.truncated {
                self.mark_truncated();
            }
            if p == 0 {
                (w, h) = (d.width, d.height);
            } else if (d.width, d.height) != (w, h) {
                return None;
            }
            let pl = d.planes.first()?;
            let mut v = vec![0u8; w as usize * h as usize];
            for y in 0..h as usize {
                let row = pl.data.get(y * pl.stride..y * pl.stride + w as usize)?;
                v[y * w as usize..(y + 1) * w as usize].copy_from_slice(row);
            }
            planes.push(v);
        }
        let n = w as usize * h as usize;
        let grey = !matches!(ifd.photometric, Photometric::Rgb);
        let invert = ifd.photometric == Photometric::MinIsWhite;
        let alpha = ifd.alpha;
        match self.fmt {
            PixelFormat::Gray8 => {
                let mut out = planes.swap_remove(0);
                if invert {
                    out.iter_mut().for_each(|v| *v = 255 - *v);
                }
                Some((out, w, h))
            }
            PixelFormat::Rgba8 => {
                let mut out = vec![0u8; n * 4];
                for (i, px) in out.chunks_exact_mut(4).enumerate() {
                    let c = |k: usize| planes.get(k).map_or(0, |p| p[i]);
                    let (r, gg, b, next) = if grey {
                        let v = if invert { 255 - c(0) } else { c(0) };
                        (v, v, v, 1)
                    } else {
                        (c(0), c(1), c(2), 3)
                    };
                    let a = match alpha {
                        Some((k, _)) => planes.get(k).or(planes.get(next)).map_or(255, |p| p[i]),
                        None => 255,
                    };
                    let mut rgb = [r, gg, b];
                    if alpha.is_some_and(|(_, premul)| premul) && a != 0 && a != 255 {
                        for v in &mut rgb {
                            *v = ((u32::from(*v) * 255 + u32::from(a) / 2) / u32::from(a)).min(255)
                                as u8;
                        }
                    }
                    px.copy_from_slice(&[rgb[0], rgb[1], rgb[2], a]);
                }
                Some((out, w, h))
            }
            _ => None,
        }
    }

    /// Decode one JPEG unit to the output format. Returns pixels, width and height.
    fn jpeg_unit(
        &self,
        src: &[u8],
        uidx: usize,
        scale: specular_jpeg::Scale,
    ) -> Option<(Vec<u8>, u32, u32)> {
        let ifd = self.ifd;
        let tables = ifd.jpeg_tables.clone().and_then(|r| self.data.get(r));
        let stream: Cow<[u8]> = match (&ifd.ojpeg, tables) {
            (Some(oj), _) => {
                let g = &self.geo;
                let uy = uidx as u32 / g.ucols;
                let rows = if ifd.tiled {
                    g.uh
                } else {
                    g.uh.min(g.lh.saturating_sub(uy * g.uh))
                };
                Cow::Owned(oj.stream(self.data, ifd, src, g.uw, rows))
            }
            (None, tables) => match tables {
                Some(t)
                    if t.len() > 4
                        && t.starts_with(&[0xFF, 0xD8])
                        && src.starts_with(&[0xFF, 0xD8]) =>
                {
                    let t = if t.ends_with(&[0xFF, 0xD9]) {
                        &t[..t.len() - 2]
                    } else {
                        t
                    };
                    let mut v = Vec::with_capacity(t.len() + src.len());
                    v.extend_from_slice(t);
                    v.extend_from_slice(&src[2..]);
                    Cow::Owned(v)
                }
                _ => Cow::Borrowed(src),
            },
        };
        let opts = specular_jpeg::DecodeOptions {
            scale,
            region: None,
            cancel: self.cancel,
            parallelism: if self.single_unit {
                specular_jpeg::Parallelism::Auto
            } else {
                specular_jpeg::Parallelism::Single
            },
        };
        // The JPEG's own frame header sizes its decode: one claiming far more than its tile or
        // strip (a 65535² frame in a 256² tile) would be decoded whole, once per unit.
        let g = &self.geo;
        let fits = |len: u32, unit: u32, level: u32, units: u32| {
            len <= unit.saturating_add(JPEG_UNIT_SLACK)
                || (units == 1 && len <= level.saturating_add(JPEG_UNIT_SLACK))
        };
        match specular_jpeg::read_info(&stream) {
            Ok(i) if fits(i.width, g.uw, g.lw, g.ucols) && fits(i.height, g.uh, g.lh, g.urows) => {}
            _ => {
                self.mark_truncated();
                return None;
            }
        }
        // RGB or YCbCr plus an alpha sample: libtiff takes the four JPEG components as they
        // are; the JPEG rules would read them as CMYK.
        let with_alpha = ifd.samples == 4
            && ifd.alpha.is_some()
            && matches!(ifd.photometric, Photometric::Rgb | Photometric::YCbCr);
        let decoded = if with_alpha {
            specular_jpeg::decode_components(&stream, &opts)
        } else {
            specular_jpeg::decode(&stream, &opts)
        };
        let mut planes = match decoded {
            Ok(p) => p,
            Err(_) => {
                self.mark_truncated();
                return None;
            }
        };
        if planes.truncated {
            self.mark_truncated();
        }
        // libtiff decodes RGB-photometric JPEG without colour conversion, and YCbCr with it,
        // whatever the markers say.
        if planes.planes.len() == 3 || (with_alpha && planes.planes.len() == 4) {
            match ifd.photometric {
                Photometric::Rgb => planes.layout = specular_jpeg::PlanesLayout::Rgb,
                Photometric::YCbCr => planes.layout = specular_jpeg::PlanesLayout::YCbCr,
                // L*a*b* samples as stored, converted below.
                p if p.is_lab() => planes.layout = specular_jpeg::PlanesLayout::Rgb,
                _ => {}
            }
        }
        if let Some(rbw) = ifd.ojpeg.as_ref().and_then(|o| o.rbw)
            && planes.layout == specular_jpeg::PlanesLayout::YCbCr
        {
            // libtiff's YCbCr conversion: Y' = (Y − black)·255 / (white − black), chroma
            // likewise to ±127 around 128.
            for (i, p) in planes.planes.iter_mut().enumerate().take(3) {
                let (b, wh) = (rbw[2 * i], rbw[2 * i + 1]);
                let (range, base) = if i == 0 { (255.0, 0.0) } else { (127.0, 128.0) };
                let lut: [u8; 256] = std::array::from_fn(|v| {
                    (base + (v as f64 - b) * range / (wh - b))
                        .round()
                        .clamp(0.0, 255.0) as u8
                });
                for v in &mut p.data {
                    *v = lut[*v as usize];
                }
            }
        }
        let (w, h) = (planes.width, planes.height);
        let n = w as usize * h as usize;
        match self.fmt {
            PixelFormat::Gray8 => {
                let p0 = &planes.planes[0];
                let mut out = vec![0u8; n];
                if planes.layout == specular_jpeg::PlanesLayout::Rgba {
                    for (o, px) in out.iter_mut().zip(p0.data.chunks_exact(4)) {
                        *o = px[0];
                    }
                } else {
                    for y in 0..h as usize {
                        let s = y * p0.stride;
                        let row = p0.data.get(s..s + w as usize)?;
                        out[y * w as usize..(y + 1) * w as usize].copy_from_slice(row);
                    }
                }
                Some((out, w, h))
            }
            PixelFormat::Rgba8 => {
                let mut out = vec![0u8; n * 4];
                specular_jpeg::planes_to_rgba_into(
                    &planes,
                    specular_jpeg::Upsampling::Bilinear,
                    &mut out,
                    w as usize * 4,
                );
                if ifd.photometric.is_lab() {
                    self.jpeg_lab(&mut out);
                }
                if with_alpha && ifd.alpha.is_some_and(|(_, premul)| premul) {
                    for px in out.chunks_exact_mut(4) {
                        let a = u32::from(px[3]);
                        if a != 0 && a != 255 {
                            for c in &mut px[..3] {
                                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
                            }
                        }
                    }
                }
                Some((out, w, h))
            }
            // JPEG units are 8-bit: never wide.
            PixelFormat::Gray16
            | PixelFormat::GrayAlpha16
            | PixelFormat::Rgba16
            | PixelFormat::Rgba16F
            | PixelFormat::GrayF32 => None,
        }
    }

    /// L*a*b* JPEG pixels (L, a*, b*, 255 per pixel) converted in place, a short run at a
    /// time.
    fn jpeg_lab(&self, px: &mut [u8]) {
        let Some(coding) = self.conv.lab_coding else {
            return;
        };
        let mut tmp = [0u8; 4 * 256];
        let convert = |run: &mut [u8], tmp: &mut [u8; 4 * 256]| {
            let t = &mut tmp[..run.len()];
            t.copy_from_slice(run);
            let s = crate::LabSamples {
                channels: [&t[..], &t[1..], &t[2..]],
                step: 4,
                coding,
            };
            self.conv.lab_convert(&s, run);
        };
        if self.single_unit && px.len() >= 2 * ROW_BAND_PIXELS * 4 {
            px.par_chunks_mut(ROW_BAND_PIXELS * 4).for_each_init(
                || [0u8; 4 * 256],
                |tmp, band| {
                    for run in band.chunks_mut(4 * 256) {
                        convert(run, tmp);
                    }
                },
            );
        } else {
            for run in px.chunks_mut(4 * 256) {
                convert(run, &mut tmp);
            }
        }
    }

    /// One JPEG tile / strip decoded with DCT scaling straight into the output rows.
    fn jpeg_scaled_item(&self, it: &mut Item, ox0: u32, ox1: u32, oy0: u32) -> Result<()> {
        let g = &self.geo;
        let bpp = self.bpp;
        let uidx = (it.iy * g.ucols + it.ix) as usize;
        let scale = match g.rf {
            2 => specular_jpeg::Scale::Half,
            4 => specular_jpeg::Scale::Quarter,
            _ => specular_jpeg::Scale::Eighth,
        };
        let px = match self.unit_bytes(uidx) {
            None => {
                self.mark_truncated();
                None
            }
            Some([]) => {
                for r in it.rows.iter_mut() {
                    fill_missing(r, self.fmt, false);
                }
                return Ok(());
            }
            Some(src) => self.jpeg_unit(src, uidx, scale),
        };
        if self.cancelled() {
            return Err(Error::Cancelled);
        }
        let (bx, by) = (it.ix * g.iw_out, it.iy * g.ih_out);
        for (j, d) in it.rows.iter_mut().enumerate() {
            let y = oy0 - by + j as u32;
            match &px {
                Some((p, w, h)) if y < *h && ox1 - bx <= *w => {
                    let s = (y as usize * *w as usize + (ox0 - bx) as usize) * bpp;
                    d.copy_from_slice(&p[s..s + d.len()]);
                }
                _ => fill_missing(d, self.fmt, true),
            }
        }
        Ok(())
    }
}
