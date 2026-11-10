//! Hamamatsu NDPI: a TIFF whose every level is one JPEG strip (up to gigapixels wide) with
//! restart markers. A region is read by splicing the restart intervals that cover it into a
//! small JPEG of its own: the original headers with the frame size patched, the intervals'
//! entropy-coded segments renumbered, an EOI. The intervals' offsets come from the McuStarts
//! tag, else from one scan of the strip for RST markers (cached).
//!
//! Private tags: 65420 the NDPI format flag (1), 65421 source lens (objective magnification;
//! -1 the macro image, -2 the focus map), 65422/65423 offsets from the slide centre (nm),
//! 65424 focal plane (0 the nominal one), 65426 McuStarts (+ 65432 their high halves), 65442
//! scanner serial, 65449 key = value properties.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;

use parking_lot::Mutex;
use rayon::prelude::*;
use specular_jpeg as jp;
use specular_tiff as tf;

use crate::composite::box_reduce_rgba;
use crate::{
    AssociatedKind, Channel, Error, Level, Props, ReadOptions, Region, Result, Rgba, Slide, Vendor,
};

/// The NDPI format flag: present in every NDPI directory.
pub const TAG_FORMAT_FLAG: u16 = 65420;
const TAG_MAGNIFICATION: u16 = 65421;
const TAG_X_OFFSET: u16 = 65422;
const TAG_Y_OFFSET: u16 = 65423;
const TAG_FOCAL_PLANE: u16 = 65424;
const TAG_MCU_STARTS: u16 = 65426;
const TAG_MCU_STARTS_HIGH: u16 = 65432;
const TAG_SERIAL: u16 = 65442;
const TAG_PROPERTIES: u16 = 65449;

/// Largest level decoded whole (levels without restart markers), in decoded pixels.
const WHOLE_MAX_PIXELS: u64 = 1 << 26;
/// Rows of restart intervals per parallel band, at least.
const BAND_MIN_ROWS: u32 = 64;
/// Most restart intervals tracked per level (one u64 each).
const MAX_UNITS: u64 = 1 << 26;

/// JPEG headers of a level, parsed by marker (the SOF may give a size of zero for levels
/// wider than JPEG allows; the TIFF tags hold the size).
#[derive(Clone, Debug)]
struct Header {
    /// Offset of the SOF's height field (the width follows).
    sof_dims: usize,
    mcu_w: u32,
    mcu_h: u32,
    /// Restart interval in MCUs (0: none).
    restart: u32,
    /// The end of the SOS segment (entropy data start).
    data_start: usize,
}

fn be16(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(i)?, *d.get(i + 1)?]))
}

fn parse_header(d: &[u8]) -> Result<Header> {
    let bad = || Error::Malformed("NDPI JPEG headers".into());
    if be16(d, 0) != Some(0xFFD8) {
        return Err(bad());
    }
    let mut i = 2;
    let (mut sof, mut mcu, mut restart) = (None, None, 0u32);
    loop {
        // Fill bytes between segments.
        while d.get(i) == Some(&0xFF) && d.get(i + 1) == Some(&0xFF) {
            i += 1;
        }
        if d.get(i) != Some(&0xFF) {
            return Err(bad());
        }
        let m = *d.get(i + 1).ok_or_else(bad)?;
        let len = usize::from(be16(d, i + 2).ok_or_else(bad)?);
        if len < 2 || i + 2 + len > d.len() {
            return Err(bad());
        }
        let seg = &d[i + 4..i + 2 + len];
        match m {
            0xC0 | 0xC1 => {
                // precision, height, width, components, (id, sampling, table)...
                let n = usize::from(*seg.get(5).ok_or_else(bad)?);
                let (mut mh, mut mv) = (1u32, 1u32);
                for k in 0..n {
                    let s = *seg.get(6 + 3 * k + 1).ok_or_else(bad)?;
                    mh = mh.max(u32::from(s >> 4).max(1));
                    mv = mv.max(u32::from(s & 15).max(1));
                }
                if mh > 4 || mv > 4 {
                    return Err(bad());
                }
                sof = Some(i + 5);
                mcu = Some((8 * mh, 8 * mv));
            }
            0xC2..=0xCF if m != 0xC4 && m != 0xC8 && m != 0xCC => {
                return Err(Error::Unsupported("NDPI level not baseline JPEG".into()));
            }
            0xDD => restart = u32::from(be16(seg, 0).ok_or_else(bad)?),
            0xDA => {
                let (sof_dims, (mcu_w, mcu_h)) = (sof.ok_or_else(bad)?, mcu.ok_or_else(bad)?);
                return Ok(Header {
                    sof_dims,
                    mcu_w,
                    mcu_h,
                    restart,
                    data_start: i + 2 + len,
                });
            }
            _ => {}
        }
        i += 2 + len;
    }
}

/// Where the restart intervals of a level start (offsets in the strip), each with the end of
/// the strip appended.
fn scan_restarts(strip: &[u8], start: usize, cancel: Option<&AtomicBool>) -> Result<Vec<u64>> {
    let mut out = vec![start as u64];
    let mut i = start;
    while i + 1 < strip.len() {
        if out.len() % 4096 == 0 {
            crate::check_cancel(cancel)?;
        }
        let Some(p) = strip[i..strip.len() - 1].iter().position(|&b| b == 0xFF) else {
            break;
        };
        i += p;
        let m = strip[i + 1];
        if (0xD0..=0xD7).contains(&m) {
            if out.len() as u64 >= MAX_UNITS {
                return Err(Error::TooLarge);
            }
            out.push((i + 2) as u64);
        } else if m == 0xD9 {
            break;
        }
        i += 2;
    }
    Ok(out)
}

pub(crate) struct NdpiLevel {
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// The strip's range in the file.
    offset: usize,
    len: usize,
    header: Header,
    /// Restart interval starts from McuStarts (strip-relative), if given and plausible.
    starts_tag: Option<Vec<u64>>,
    /// Offsets known to start restart intervals (a VMS optimisation file's row starts):
    /// the scan for the others runs between them in parallel.
    hints: Option<Vec<u64>>,
    starts: OnceLock<std::result::Result<Vec<u64>, String>>,
}

impl NdpiLevel {
    fn strip<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        let end = self.offset.saturating_add(self.len).min(data.len());
        data.get(self.offset..end).unwrap_or(&[])
    }

    fn units_per_row(&self) -> Option<u32> {
        let h = &self.header;
        if h.restart == 0 {
            return None;
        }
        let mcus = self.width.div_ceil(h.mcu_w);
        mcus.is_multiple_of(h.restart).then_some(mcus / h.restart)
    }

    fn unit_rows(&self) -> u32 {
        self.height.div_ceil(self.header.mcu_h)
    }

    fn starts(&self, data: &[u8], cancel: Option<&AtomicBool>) -> Result<&[u64]> {
        let r = self.starts.get_or_init(|| {
            let strip = self.strip(data);
            let n = u64::from(self.units_per_row().unwrap_or(0)) * u64::from(self.unit_rows());
            if let Some(t) = &self.starts_tag
                && t.len() as u64 == n
                && t.first() == Some(&(self.header.data_start as u64))
                && t.windows(2).all(|w| w[0] < w[1])
                && t.last().is_some_and(|&l| l < strip.len() as u64)
            {
                return Ok(t.clone());
            }
            if let Some(h) = self.hints.as_ref().filter(|h| self.hints_valid(strip, h)) {
                let segs: Vec<(usize, usize)> = h
                    .iter()
                    .enumerate()
                    .map(|(k, &a)| {
                        let b = h.get(k + 1).map_or(strip.len(), |&b| b as usize);
                        (a as usize, b)
                    })
                    .collect();
                let parts: std::result::Result<Vec<Vec<u64>>, Error> = segs
                    .par_iter()
                    .map(|&(a, b)| {
                        let mut v = scan_restarts(&strip[..b], a, cancel)?;
                        v.retain(|&s| (s as usize) < b || b == strip.len());
                        Ok(v)
                    })
                    .collect();
                return match parts {
                    Ok(p) => {
                        let mut v: Vec<u64> = p.into_iter().flatten().collect();
                        v.dedup();
                        if v.len() as u64 >= n {
                            Ok(v)
                        } else {
                            Err(format!("{} of {n} restart intervals found", v.len()))
                        }
                    }
                    Err(e) => Err(e.to_string()),
                };
            }
            match scan_restarts(strip, self.header.data_start, cancel) {
                Ok(v) if v.len() as u64 >= n => Ok(v),
                Ok(v) => Err(format!("{} of {n} restart intervals found", v.len())),
                Err(e) => Err(e.to_string()),
            }
        });
        match r {
            Ok(v) => Ok(v),
            Err(e) => Err(Error::Malformed(e.clone())),
        }
    }

    /// Hints start at the entropy data, increase, and each follows a restart marker.
    fn hints_valid(&self, strip: &[u8], h: &[u64]) -> bool {
        h.first() == Some(&(self.header.data_start as u64))
            && h.windows(2).all(|w| w[0] < w[1])
            && h.iter().skip(1).all(|&o| {
                let o = o as usize;
                o >= 2
                    && o < strip.len()
                    && strip[o - 2] == 0xFF
                    && (0xD0..=0xD7).contains(&strip[o - 1])
            })
    }

    /// Restart interval offsets known to start rows (a VMS optimisation file's).
    pub(crate) fn set_hints(&mut self, h: Vec<u64>) {
        self.hints = Some(h);
    }

    /// Bytes of the cached interval offsets.
    pub(crate) fn starts_bytes(&self) -> usize {
        self.starts
            .get()
            .map_or(0, |s| s.as_ref().map_or(0, |v| v.len() * 8))
    }

    /// Region `r` of the 1/`f` downscale (`f` a power of two) of this level, read from its
    /// restart intervals; `whole` gives the level decoded whole at DCT scale `e` (cached by
    /// the caller) when it has none.
    pub(crate) fn read_scaled(
        &self,
        data: &[u8],
        f: u32,
        o: &ReadOptions,
        r: Region,
        whole: impl FnOnce(u32) -> Result<std::sync::Arc<Rgba>>,
    ) -> Result<Rgba> {
        let lr = Region {
            x: r.x.saturating_mul(f),
            y: r.y.saturating_mul(f),
            w: r.w.saturating_mul(f),
            h: r.h.saturating_mul(f),
        };
        let lr = Region::clip(Some(lr), self.width, self.height)
            .ok_or_else(|| Error::Malformed("region outside the level".into()))?;
        let e = f.min(8);
        match self.units_per_row() {
            Some(upr) if self.starts(data, o.cancel).is_ok() => {
                read_units(data, self, upr, lr, f, e, o, r)
            }
            _ => {
                let w = whole(e)?;
                Ok(crop(&box_reduce_rgba(&w, f / e), (0, 0), r))
            }
        }
    }

    /// The level decoded whole at DCT scale `e` (1, 2, 4 or 8).
    pub(crate) fn decode_whole(
        &self,
        data: &[u8],
        e: u32,
        cancel: Option<&AtomicBool>,
    ) -> Result<Rgba> {
        let (ow, oh) = (self.width.div_ceil(e), self.height.div_ceil(e));
        if u64::from(ow) * u64::from(oh) > WHOLE_MAX_PIXELS {
            return Err(Error::TooLarge);
        }
        let mut j = self.strip(data).to_vec();
        patch_dims(&mut j, &self.header, self.width, self.height)?;
        decode_jpeg(&j, e, cancel, true)
    }
}

/// A whole JPEG file as a level (a VMS image file): its size from the SOF.
pub(crate) fn jpeg_level(data: &[u8]) -> Result<NdpiLevel> {
    let header = parse_header(data)?;
    let height = u32::from(be16(data, header.sof_dims).unwrap_or(0));
    let width = u32::from(be16(data, header.sof_dims + 2).unwrap_or(0));
    if width == 0 || height == 0 {
        return Err(Error::Malformed("JPEG without a frame size".into()));
    }
    Ok(NdpiLevel {
        width,
        height,
        offset: 0,
        len: data.len(),
        header,
        starts_tag: None,
        hints: None,
        starts: OnceLock::new(),
    })
}

pub struct Ndpi {
    levels: Vec<NdpiLevel>,
    /// Macro (and other associated) images: whole JPEG strips.
    associated: Vec<NdpiLevel>,
    /// Levels decoded whole, by (level, DCT scale).
    whole: Mutex<HashMap<(usize, u32), std::sync::Arc<Rgba>>>,
}

fn level_of(d: &tf::Directories, data: &[u8], di: usize) -> Option<NdpiLevel> {
    let ifd = d.dirs[di].ifd.as_ref()?;
    if ifd.compression != tf::Compression::Jpeg || ifd.tiled {
        return None;
    }
    let (&off, &len) = (ifd.unit_offsets().first()?, ifd.unit_counts().first()?);
    if ifd.unit_offsets().len() != 1 {
        return None;
    }
    let (offset, len) = (usize::try_from(off).ok()?, usize::try_from(len).ok()?);
    let strip = data.get(offset..offset.saturating_add(len).min(data.len()))?;
    let header = parse_header(strip).ok()?;
    let starts_tag = d.uints(data, di, TAG_MCU_STARTS).map(|lo| {
        let hi = d.uints(data, di, TAG_MCU_STARTS_HIGH);
        lo.iter()
            .enumerate()
            .map(|(k, &v)| v | hi.as_ref().and_then(|h| h.get(k)).map_or(0, |h| h << 32))
            .collect()
    });
    Some(NdpiLevel {
        width: ifd.width,
        height: ifd.height,
        offset,
        len,
        header,
        starts_tag,
        hints: None,
        starts: OnceLock::new(),
    })
}

/// Open an NDPI from its directories.
pub(crate) fn open(data: &[u8], d: &tf::Directories) -> Result<Slide> {
    let mut levels = Vec::new();
    let mut associated = Vec::new();
    let mut assoc_kinds = Vec::new();
    let mut first_main = None;
    let real = |di: usize, tag| d.floats(data, di, tag).and_then(|v| v.first().copied());
    for di in d.main_chain() {
        let mag = real(di, TAG_MAGNIFICATION).unwrap_or(0.0);
        if mag == -2.0 {
            continue; // the focus map
        }
        // Other focal planes than the nominal one are skipped.
        if real(di, TAG_FOCAL_PLANE).is_some_and(|z| z != 0.0) && mag > 0.0 {
            continue;
        }
        let Some(l) = level_of(d, data, di) else {
            continue;
        };
        if mag == -1.0 {
            assoc_kinds.push((AssociatedKind::Macro, l.width, l.height));
            associated.push(l);
        } else {
            first_main.get_or_insert(di);
            levels.push(l);
        }
    }
    levels.sort_by_key(|l| std::cmp::Reverse(l.width));
    levels.dedup_by_key(|l| l.width);
    let base = levels
        .first()
        .ok_or_else(|| Error::Malformed("NDPI without readable levels".into()))?;
    let (w, h) = (base.width, base.height);
    let di = first_main.unwrap_or(0);
    let mut props = Props::new(Vendor::Hamamatsu);
    props.objective = real(di, TAG_MAGNIFICATION).filter(|m| *m > 0.0);
    let res = |tag| d.reals(data, di, tag).and_then(|v| v.first().copied());
    let unit = d
        .uints(data, di, 296)
        .and_then(|v| v.first().copied())
        .unwrap_or(2);
    if let (Some(xr), Some(yr)) = (res(282), res(283))
        && xr > 0.0
        && yr > 0.0
    {
        let k = if unit == 2 { 25_400.0 } else { 10_000.0 };
        props.mpp = Some((k / xr, k / yr));
    }
    let text = |tag| {
        d.ascii(data, di, tag)
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    };
    props.scanner = match (text(272), text(TAG_SERIAL)) {
        (Some(m), Some(s)) => Some(format!("{m} ({s})")),
        (m, s) => m.or(s),
    };
    props.date = text(306);
    // Signed nanometres (stored as SLONG, or as LONG holding a two's complement value).
    let offset = |tag| {
        let v = *d.uints(data, di, tag)?.first()?;
        Some(f64::from(v as u32 as i32))
    };
    props.centre_offset_nm = offset(TAG_X_OFFSET).zip(offset(TAG_Y_OFFSET));
    props.sample = "8-bit RGB, JPEG".to_owned();
    if let Some(p) = text(TAG_PROPERTIES) {
        for line in p.lines() {
            if let Some((k, v)) = line.split_once('=') {
                props
                    .properties
                    .push((k.trim().to_owned(), v.trim().to_owned()));
            }
        }
    }
    let lv = levels
        .iter()
        .map(|l| Level {
            width: l.width,
            height: l.height,
            downsample: f64::from(w) / f64::from(l.width.max(1)),
        })
        .collect();
    Ok(Slide {
        props,
        width: w,
        height: h,
        levels: lv,
        channels: vec![Channel::default()],
        size_z: 1,
        size_t: 1,
        associated: assoc_kinds,
        files: Vec::new(),
        backend: crate::Backend::Ndpi(Ndpi {
            levels,
            associated,
            whole: Mutex::new(HashMap::new()),
        }),
    })
}

fn dct_scale(f: u32) -> jp::Scale {
    match f {
        1 => jp::Scale::Full,
        2 => jp::Scale::Half,
        4 => jp::Scale::Quarter,
        _ => jp::Scale::Eighth,
    }
}

fn decode_jpeg(j: &[u8], e: u32, cancel: Option<&AtomicBool>, parallel: bool) -> Result<Rgba> {
    let p = jp::decode(
        j,
        &jp::DecodeOptions {
            scale: dct_scale(e),
            region: None,
            cancel,
            parallelism: if parallel {
                jp::Parallelism::Auto
            } else {
                jp::Parallelism::Single
            },
        },
    )?;
    Ok(Rgba {
        width: p.width,
        height: p.height,
        data: jp::planes_to_rgba(&p),
        truncated: p.truncated,
    })
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// `src` (whose top-left is at `origin` in the same pixels) cropped or padded (edge pixels
/// repeated) to `r`.
fn crop(src: &Rgba, origin: (u32, u32), r: Region) -> Rgba {
    let mut out = vec![0u8; r.w as usize * r.h as usize * 4];
    if src.width == 0 || src.height == 0 {
        return Rgba {
            width: r.w,
            height: r.h,
            data: out,
            truncated: true,
        };
    }
    for y in 0..r.h {
        let sy = (r.y + y).saturating_sub(origin.1).min(src.height - 1) as usize;
        let row = &mut out[y as usize * r.w as usize * 4..][..r.w as usize * 4];
        let sx0 = r.x.saturating_sub(origin.0).min(src.width - 1) as usize;
        let n = (src.width as usize - sx0).min(r.w as usize);
        let s = &src.data[(sy * src.width as usize + sx0) * 4..][..n * 4];
        row[..n * 4].copy_from_slice(s);
        for x in n..r.w as usize {
            row.copy_within((n - 1) * 4..n * 4, x * 4);
        }
    }
    Rgba {
        width: r.w,
        height: r.h,
        data: out,
        truncated: src.truncated,
    }
}

impl Ndpi {
    /// The level to read a 1/`scale` downscale from, with the remaining power-of-two factor.
    fn choose(&self, scale: u32) -> (usize, u32) {
        let (w, h) = (self.levels[0].width, self.levels[0].height);
        let mut best = (0, scale);
        for (i, l) in self.levels.iter().enumerate().skip(1) {
            let mut f = 2;
            while f <= scale {
                let (vw, vh) = (w.div_ceil(f), h.div_ceil(f));
                if l.width.abs_diff(vw) <= 1.max(vw / 1000)
                    && l.height.abs_diff(vh) <= 1.max(vh / 1000)
                    && scale / f < best.1
                {
                    best = (i, scale / f);
                }
                f *= 2;
            }
        }
        best
    }

    pub(crate) fn read(&self, data: &[u8], o: &ReadOptions) -> Result<Rgba> {
        let r = o
            .region
            .ok_or_else(|| Error::Malformed("no region".into()))?;
        let (li, f) = self.choose(o.scale);
        let l = &self.levels[li];
        // The region in level pixels.
        let lr = Region {
            x: r.x.saturating_mul(f),
            y: r.y.saturating_mul(f),
            w: r.w.saturating_mul(f),
            h: r.h.saturating_mul(f),
        };
        let lr = Region::clip(Some(lr), l.width, l.height)
            .ok_or_else(|| Error::Malformed("region outside the level".into()))?;
        let e = f.min(8);
        match l.units_per_row() {
            Some(upr) if l.starts(data, o.cancel).is_ok() => {
                read_units(data, l, upr, lr, f, e, o, r)
            }
            _ => self.read_whole(data, li, f, e, o, r),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn read_units(
    data: &[u8],
    l: &NdpiLevel,
    upr: u32,
    lr: Region,
    f: u32,
    e: u32,
    o: &ReadOptions,
    r: Region,
) -> Result<Rgba> {
    let h = &l.header;
    let unit_w = h.mcu_w * h.restart;
    // Snap the start outwards so that it lies on the f-pixel grid of the downscale.
    let step_x = unit_w / gcd(unit_w, f) * f;
    let step_y = h.mcu_h / gcd(h.mcu_h, f) * f;
    let x0 = lr.x / step_x * step_x;
    let y0 = lr.y / step_y * step_y;
    let c0 = x0 / unit_w;
    let c1 = (lr.x + lr.w).div_ceil(unit_w).min(upr);
    let rows = l.unit_rows();
    let r0 = y0 / h.mcu_h;
    let r1 = (lr.y + lr.h).div_ceil(h.mcu_h).min(rows);
    // Bands of whole unit rows, each a multiple of step_y pixels.
    let per = step_y / h.mcu_h;
    let band_rows = BAND_MIN_ROWS.div_ceil(per) * per;
    let starts = l.starts(data, o.cancel)?;
    let strip = l.strip(data);
    let bands: Vec<(u32, u32)> = (r0..r1)
        .step_by(band_rows as usize)
        .map(|b| (b, (b + band_rows).min(r1)))
        .collect();
    let inner_parallel = o.parallel && bands.len() == 1;
    let decode_band = |&(b0, b1): &(u32, u32)| -> Result<Rgba> {
        crate::check_cancel(o.cancel)?;
        let j = splice(strip, l, starts, upr, c0, c1, b0, b1)?;
        let px = decode_jpeg(&j, e, o.cancel, inner_parallel)?;
        Ok(box_reduce_rgba(&px, f / e))
    };
    let parts: Vec<Rgba> = if o.parallel && bands.len() > 1 {
        bands.par_iter().map(decode_band).collect::<Result<_>>()?
    } else {
        bands.iter().map(decode_band).collect::<Result<_>>()?
    };
    // Stack the bands (each is full width).
    let width = parts.first().map_or(0, |p| p.width);
    let mut all = Rgba {
        width,
        height: 0,
        data: Vec::new(),
        truncated: false,
    };
    for p in parts {
        if p.width != width {
            return Err(Error::Malformed("NDPI band widths differ".into()));
        }
        all.height += p.height;
        all.truncated |= p.truncated;
        all.data.extend_from_slice(&p.data);
    }
    Ok(crop(&all, (x0 / f, y0 / f), r))
}

impl Ndpi {
    fn read_whole(
        &self,
        data: &[u8],
        li: usize,
        f: u32,
        e: u32,
        o: &ReadOptions,
        r: Region,
    ) -> Result<Rgba> {
        let l = &self.levels[li];
        let whole = self.whole_level(data, l, li, e, o.cancel)?;
        let reduced = box_reduce_rgba(&whole, f / e);
        Ok(crop(&reduced, (0, 0), r))
    }

    fn whole_level(
        &self,
        data: &[u8],
        l: &NdpiLevel,
        li: usize,
        e: u32,
        cancel: Option<&AtomicBool>,
    ) -> Result<std::sync::Arc<Rgba>> {
        if let Some(w) = self.whole.lock().get(&(li, e)) {
            return Ok(w.clone());
        }
        let (ow, oh) = (l.width.div_ceil(e), l.height.div_ceil(e));
        if u64::from(ow) * u64::from(oh) > WHOLE_MAX_PIXELS {
            return Err(Error::TooLarge);
        }
        let mut j = l.strip(data).to_vec();
        patch_dims(&mut j, &l.header, l.width, l.height)?;
        let px = std::sync::Arc::new(decode_jpeg(&j, e, cancel, true)?);
        self.whole.lock().insert((li, e), px.clone());
        Ok(px)
    }

    pub(crate) fn read_associated(
        &self,
        data: &[u8],
        i: usize,
        cancel: Option<&AtomicBool>,
    ) -> Result<Rgba> {
        let l = self
            .associated
            .get(i)
            .ok_or_else(|| Error::Malformed("no such associated image".into()))?;
        let mut e = 1;
        while e < 8 && (l.width / e > 8192 || l.height / e > 8192) {
            e *= 2;
        }
        let mut j = l.strip(data).to_vec();
        patch_dims(&mut j, &l.header, l.width, l.height)?;
        decode_jpeg(&j, e, cancel, true)
    }

    pub(crate) fn memory_bytes(&self) -> usize {
        let starts: usize = self
            .levels
            .iter()
            .filter_map(|l| l.starts.get())
            .map(|s| s.as_ref().map_or(0, |v| v.len() * 8))
            .sum();
        starts
            + self
                .whole
                .lock()
                .values()
                .map(|w| w.data.len())
                .sum::<usize>()
    }

    pub(crate) fn trim_memory(&self) -> usize {
        let mut m = self.whole.lock();
        let n = m.values().map(|w| w.data.len()).sum();
        m.clear();
        n
    }
}

fn patch_dims(j: &mut [u8], h: &Header, w: u32, ht: u32) -> Result<()> {
    let (w, ht) = (
        u16::try_from(w).map_err(|_| Error::Unsupported("JPEG level wider than 65535".into()))?,
        u16::try_from(ht).map_err(|_| Error::Unsupported("JPEG level taller than 65535".into()))?,
    );
    let d = j
        .get_mut(h.sof_dims..h.sof_dims + 4)
        .ok_or_else(|| Error::Malformed("NDPI SOF".into()))?;
    d[..2].copy_from_slice(&ht.to_be_bytes());
    d[2..].copy_from_slice(&w.to_be_bytes());
    Ok(())
}

/// A JPEG of the restart intervals in columns `c0..c1` and rows `r0..r1`.
#[allow(clippy::too_many_arguments)]
fn splice(
    strip: &[u8],
    l: &NdpiLevel,
    starts: &[u64],
    upr: u32,
    c0: u32,
    c1: u32,
    r0: u32,
    r1: u32,
) -> Result<Vec<u8>> {
    let h = &l.header;
    let unit_w = h.mcu_w * h.restart;
    let x0 = c0 * unit_w;
    let y0 = r0 * h.mcu_h;
    let w = ((c1 - c0) * unit_w).min(l.width - x0);
    let ht = ((r1 - r0) * h.mcu_h).min(l.height - y0);
    let mut j = Vec::with_capacity(h.data_start + 64);
    j.extend_from_slice(&strip[..h.data_start]);
    patch_dims(&mut j, h, w, ht)?;
    let mut k = 0u8;
    let n = (c1 - c0) * (r1 - r0);
    for row in r0..r1 {
        for col in c0..c1 {
            let i = row as usize * upr as usize + col as usize;
            let a = *starts
                .get(i)
                .ok_or_else(|| Error::Truncated("NDPI intervals".into()))?;
            let b = starts.get(i + 1).copied().unwrap_or(strip.len() as u64);
            let (a, b) = (a as usize, (b as usize).min(strip.len()));
            if a >= b {
                return Err(Error::Truncated("NDPI intervals".into()));
            }
            let mut seg = &strip[a..b];
            // Drop the interval's own trailing RST or EOI.
            if let [rest @ .., 0xFF, m] = seg
                && ((0xD0..=0xD7).contains(m) || *m == 0xD9)
            {
                seg = rest;
            }
            j.extend_from_slice(seg);
            let last = (row - r0) * (c1 - c0) + (col - c0) + 1 == n;
            if !last {
                j.extend_from_slice(&[0xFF, 0xD0 + k]);
                k = (k + 1) & 7;
            }
        }
    }
    j.extend_from_slice(&[0xFF, 0xD9]);
    Ok(j)
}

/// The entropy data start of a JPEG (test fixtures).
#[cfg(any(test, feature = "fixtures"))]
pub(crate) fn tests_header_end(j: &[u8]) -> usize {
    parse_header(j).map_or(0, |h| h.data_start)
}

/// The restart interval starts of a JPEG (test fixtures).
#[cfg(any(test, feature = "fixtures"))]
pub(crate) fn tests_scan(j: &[u8], start: usize) -> Vec<u64> {
    scan_restarts(j, start, None).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_restart_scan() {
        let mut enc = Vec::new();
        let px: Vec<u8> = (0..64 * 32 * 3).map(|i| (i % 251) as u8).collect();
        let mut e = jpeg_encoder::Encoder::new(&mut enc, 90);
        e.set_restart_interval(2);
        e.set_sampling_factor(jpeg_encoder::SamplingFactor::R_4_2_0);
        e.encode(&px, 64, 32, jpeg_encoder::ColorType::Rgb).unwrap();
        let h = parse_header(&enc).unwrap();
        assert_eq!((h.mcu_w, h.mcu_h, h.restart), (16, 16, 2));
        let s = scan_restarts(&enc, h.data_start, None).unwrap();
        // 4 MCUs per row, 2 rows: 4 intervals.
        assert_eq!(s.len(), 4);
    }
}
