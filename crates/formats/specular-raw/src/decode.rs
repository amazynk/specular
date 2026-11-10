//! Sensor data decoders: uncompressed packings, lossless JPEG strips/tiles (parallel), Canon CR2
//! slices, Nikon Huffman, Sony ARW2, Panasonic RW2 (v4) and Pentax Huffman.

use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::bits::BitReader;
use crate::color;
use crate::container::{Black, Codec, ColorSource, Packing, Sensor, Storage};
use crate::develop;
use crate::dngtile;
use crate::error::{RawError, Result};
use crate::huffman::Huffman;
use crate::lj92tile;
use crate::ljpeg;
use crate::region::{Tile, TileCache};
use crate::tiff::Endian;
use crate::{DecodeOptions, OutputSize, RawImage, Rect};

fn cancelled(opts: &DecodeOptions) -> bool {
    opts.cancel.is_some_and(|c| c.load(Ordering::Relaxed))
}

/// Decodes the sensor data of `s` and applies the DNG `OpcodeList1` and `OpcodeList2`.
pub(crate) fn decode_raw(
    d: &[u8],
    s: &Sensor,
    orientation: u16,
    opts: &DecodeOptions,
) -> Result<RawImage> {
    let mut raw = decode_stored(d, s, orientation, opts, false, None)?;
    if let Some(ops) = &s.opcodes {
        ops.apply_raw(&mut raw, opts.parallel);
        if cancelled(opts) {
            return Err(RawError::Cancelled);
        }
        raw.opcodes = Some(ops.clone());
    }
    Ok(raw)
}

/// The sensor samples a region decode of output region `reg` needs (see
/// [`develop::needed_window`]; the DNG opcodes may need more).
fn sensor_window(s: &Sensor, reg: Rect, size: OutputSize, parallel: bool) -> Rect {
    match &s.opcodes {
        Some(ops) => ops.sensor_window(s.crop, s.width, s.height, reg, size, parallel),
        None => develop::needed_window(s.crop, reg, size),
    }
}

/// The stored samples with no DNG opcode list applied (the vendors' calibrations are), for
/// [`crate::sensor`].
pub(crate) fn decode_unprocessed(
    d: &[u8],
    s: &Sensor,
    orientation: u16,
    opts: &DecodeOptions,
) -> Result<RawImage> {
    decode_stored(d, s, orientation, opts, false, None)
}

/// [`decode_unprocessed`] of a floating point DNG whose samples [`decode_float`] already
/// gave: the 16-bit samples come from those (as the block decoder would make them), so the
/// blocks are read once.
pub(crate) fn decode_unprocessed_from_float(
    d: &[u8],
    s: &Sensor,
    orientation: u16,
    opts: &DecodeOptions,
    float: &[f32],
    short: bool,
) -> Result<RawImage> {
    let mut raw = decode_stored(d, s, orientation, opts, false, Some(float))?;
    raw.truncated |= short;
    Ok(raw)
}

/// The stored samples before any demosaic into camera RGB (four-colour and odd patterns), for
/// comparisons with other readers.
pub(crate) fn decode_mosaic(
    d: &[u8],
    s: &Sensor,
    orientation: u16,
    opts: &DecodeOptions,
) -> Result<RawImage> {
    decode_stored(d, s, orientation, opts, true, None)
}

fn decode_stored(
    d: &[u8],
    s: &Sensor,
    orientation: u16,
    opts: &DecodeOptions,
    mosaic: bool,
    float: Option<&[f32]>,
) -> Result<RawImage> {
    if let Some(why) = s.unsupported {
        return Err(RawError::Unsupported(why));
    }
    let (w, h, cpp) = (s.width as usize, s.height as usize, s.cpp as usize);
    let n = w as u64 * h as u64 * cpp as u64;
    if n == 0 {
        return Err(RawError::Malformed("empty sensor"));
    }
    if n > opts.max_pixels {
        return Err(RawError::TooLarge(n));
    }
    // No supported codec gets below ~0.125 bytes per sample on real data: refuse to allocate
    // a huge buffer for a tiny (malicious) file.
    if n > (d.len() as u64).saturating_mul(64) {
        return Err(RawError::Malformed(
            "sensor size inconsistent with file size",
        ));
    }
    if cancelled(opts) {
        return Err(RawError::Cancelled);
    }
    // CFA data with several samples per photosite is one colour per photosite once added up.
    let colour_cpp = if s.cfa.is_some() { 1 } else { s.cpp };
    let (rgb_cam, matrix_source, wb, profile) = color::camera_matrix(&s.color, colour_cpp, s.wb);
    let full = Rect {
        x: 0,
        y: 0,
        w: s.width,
        h: s.height,
    };
    // A region decodes only the sensor window behind it when nothing global (masked black,
    // observed white, grey-world balance) needs the rest.
    let window = match opts.region {
        Some(_) if region_ok(s) && wb.is_some_and(wb_valid) => {
            let reg = develop::output_region(s.crop, opts)
                .ok_or(RawError::Malformed("empty output region"))?;
            sensor_window(s, reg, opts.size, opts.parallel)
        }
        _ => full,
    };
    let truncated = AtomicBool::new(false);
    if window != full {
        let mut data = vec![0u16; window.w as usize * window.h as usize * cpp];
        if let Storage::Blocks {
            blocks,
            tile_w,
            tile_h,
            codec,
        } = &s.storage
        {
            let g = Geometry {
                tw: *tile_w as usize,
                th: *tile_h as usize,
                w,
                h,
                cpp,
            };
            decode_blocks_window(d, blocks, g, *codec, window, &mut data, opts, &truncated)?;
        }
        if cancelled(opts) {
            return Err(RawError::Cancelled);
        }
        // `region_ok`: levels and white are known.
        let black = match &s.black {
            Black::Levels(b) => *b,
            Black::Masked { fallback, .. } => *fallback,
        };
        return Ok(RawImage {
            width: s.width,
            height: s.height,
            cpp: s.cpp,
            data,
            window,
            cfa: s.cfa.clone(),
            crop: s.crop,
            black,
            white: s.white.unwrap_or(1.0).max(1.0),
            linearization: s.linearization.clone(),
            wb,
            rgb_cam,
            matrix_source,
            profile: profile.map(std::sync::Arc::new),
            orientation,
            truncated: truncated.into_inner(),
            opcodes: None,
        });
    }
    let mut data = vec![0u16; n as usize];
    let mut white = s.white;
    match &s.storage {
        Storage::None => return Err(RawError::Unsupported("sensor data layout not supported")),
        Storage::Blocks { .. } if float.is_some_and(|f| f.len() == data.len()) => {
            // Rounded and clipped as `dngtile` does for float blocks.
            for (o, &v) in data.iter_mut().zip(float.unwrap_or_default()) {
                *o = if v > 0.0 { v.min(65535.0) + 0.5 } else { 0.0 } as u16;
            }
        }
        Storage::Blocks {
            blocks,
            tile_w,
            tile_h,
            codec,
        } => {
            decode_blocks(
                d,
                blocks,
                *tile_w as usize,
                *tile_h as usize,
                *codec,
                w,
                cpp,
                &mut data,
                opts,
                &truncated,
            )?;
        }
        Storage::Cr2 { range, slices } => {
            let src = d.get(range.clone()).unwrap_or(&[]);
            let (hdr, buf, t) = ljpeg::decode(src, n + 65536 * 4)?;
            let _ = hdr;
            if t || buf.len() < data.len() {
                truncated.store(true, Ordering::Relaxed);
            }
            unslice(&buf, &mut data, w, h, *slices);
        }
        Storage::Nikon { range, table, le } => {
            let src = d.get(range.clone()).unwrap_or(&[]);
            let meta = d
                .get(table.clone())
                .ok_or(RawError::Malformed("NEF: linearization table"))?;
            let (t, wl) = nikon(src, meta, *le, w, h, s.bits, &mut data)?;
            truncated.fetch_or(t, Ordering::Relaxed);
            if white.is_none() {
                white = Some(wl);
            }
        }
        Storage::SonyArw2 { range, curve } => {
            sony_arw2(d, range.clone(), curve, w, &mut data, opts, &truncated);
        }
        Storage::Cr2Ycc {
            range,
            slices,
            conv,
            mul,
        } => {
            let src = d.get(range.clone()).unwrap_or(&[]);
            let (_, lay, buf, t) =
                ljpeg::decode_subsampled(src, n + 65536 * 4, ljpeg::SubPrediction::Canon)?;
            if t {
                truncated.store(true, Ordering::Relaxed);
            }
            let line = lay.cols * lay.clrs();
            let mut stream = vec![0u16; line * lay.rows];
            unslice(&buf, &mut stream, line, lay.rows, *slices);
            let px = crate::ycc::place(&stream, lay, w, h, 16384, true);
            for (o, p) in data.chunks_exact_mut(3).zip(px) {
                o.copy_from_slice(&crate::ycc::to_rgb(p, *conv, *mul));
            }
        }
        Storage::SonyArw1 { range, rows } => {
            let src = d.get(range.clone()).unwrap_or(&[]);
            let t = sony_arw1(src, w, h, *rows as usize, &mut data);
            truncated.fetch_or(t, Ordering::Relaxed);
        }
        Storage::Panasonic { range, load_flags } => {
            let src = d.get(range.clone()).unwrap_or(&[]);
            let t = panasonic(src, *load_flags, w, h, &mut data);
            truncated.fetch_or(t, Ordering::Relaxed);
        }
        Storage::Pentax { range, table, le } => {
            let src = d.get(range.clone()).unwrap_or(&[]);
            let meta = table
                .as_ref()
                .and_then(|t| d.get(t.clone()))
                .ok_or(RawError::Unsupported("PEF without Huffman table (0x220)"))?;
            let t = pentax(src, meta, *le, w, &mut data)?;
            truncated.fetch_or(t, Ordering::Relaxed);
        }
        Storage::Fuji { range, header, cfa } => {
            let src = d
                .get(range.start.min(d.len())..range.end.min(d.len()))
                .unwrap_or(&[]);
            let t = crate::fuji::decode(header, src, cfa, &mut data, opts.parallel)
                .map_err(RawError::Malformed)?;
            truncated.fetch_or(t || range.end > d.len(), Ordering::Relaxed);
        }
        Storage::PanaBlocks {
            range,
            version,
            bits,
        } => {
            let src = d
                .get(range.start.min(d.len())..range.end.min(d.len()))
                .unwrap_or(&[]);
            let t = crate::pana::decode(src, *version, *bits, w, h, &mut data, opts.parallel);
            truncated.fetch_or(t, Ordering::Relaxed);
        }
        Storage::Olympus { range } => {
            let src = d
                .get(range.start.min(d.len())..range.end.min(d.len()))
                .unwrap_or(&[]);
            let t = crate::olympus::decode(src, w, h, &mut data);
            truncated.fetch_or(t, Ordering::Relaxed);
        }
        Storage::SuperCcd { range, geom } => {
            let src = d
                .get(range.start.min(d.len())..range.end.min(d.len()))
                .unwrap_or(&[]);
            let t = crate::superccd::decode(src, geom, &mut data, opts.parallel);
            truncated.fetch_or(t, Ordering::Relaxed);
        }
        Storage::Crx { range, header } => {
            let src = d
                .get(range.start.min(d.len())..range.end.min(d.len()))
                .unwrap_or(&[]);
            if range.end > d.len() {
                truncated.store(true, Ordering::Relaxed);
            }
            crate::crx::decode(header, src, &mut data, opts.parallel)
                .map_err(RawError::Malformed)?;
        }
        Storage::ProResRaw { range } => {
            if range.end > d.len() {
                truncated.store(true, Ordering::Relaxed);
            }
            let src = d
                .get(range.start.min(d.len())..range.end.min(d.len()))
                .unwrap_or(&[]);
            let h = specular_prores::raw::raw_header(src).map_err(prores_err)?;
            if h.width != s.width || h.height != s.height {
                return Err(RawError::Malformed("ProRes RAW: a frame of another size"));
            }
            specular_prores::raw::decode(
                src,
                &h,
                &mut data,
                specular_prores::raw::Output::Linear,
                opts.parallel,
            )
            .map_err(prores_err)?;
        }
        Storage::Vendor { range, codec } => {
            let r = crate::vendor::decode(d, range, codec, w, h, cpp, &mut data, opts)?;
            truncated.fetch_or(r.truncated, Ordering::Relaxed);
            if white.is_none() {
                white = r.white;
            }
        }
    }
    if cancelled(opts) {
        return Err(RawError::Cancelled);
    }
    if s.fields {
        data = interleave_fields(&data, w * cpp, h);
    }

    let black = match &s.black {
        Black::Levels(b) => *b,
        Black::Masked {
            area,
            also,
            fallback,
        } => masked_black(&data, w, h, cpp, &[Some(*area), *also]).unwrap_or(*fallback),
    };
    let white = match white {
        Some(v) => v,
        None => {
            let nominal = ((1u32 << s.bits.clamp(1, 16)) - 1) as f32;
            let max = observed_max(&data, w, cpp, s.crop, opts.parallel) as f32;
            if s.white_nominal && max <= nominal {
                nominal
            } else if max > nominal || max > 0.75 * nominal {
                max
            } else {
                nominal
            }
        }
    };
    let mut raw = RawImage {
        width: s.width,
        height: s.height,
        cpp: s.cpp,
        data,
        window: full,
        cfa: s.cfa.clone(),
        crop: s.crop,
        black,
        white: white.max(1.0),
        linearization: s.linearization.clone(),
        wb,
        rgb_cam,
        matrix_source,
        profile: profile.map(std::sync::Arc::new),
        orientation,
        truncated: truncated.into_inner(),
        opcodes: None,
    };
    if s.cpp > 1 && s.cfa.is_some() {
        combine_samples(&mut raw);
    }
    // Monochrome (one sample, no filter pattern): the same grey in all three channels.
    if !mosaic && raw.cpp == 1 && raw.cfa.is_none() {
        raw.data = raw.data.iter().flat_map(|&v| [v; 3]).collect();
        raw.cpp = 3;
        raw.black = [raw.black[0]; 4];
    }
    // Four-colour and other patterns the develop step cannot demosaic, and staggered layouts:
    // camera RGB here.
    if !mosaic
        && raw.cpp == 1
        && (s.stagger.is_some() || s.cfa.as_ref().is_some_and(crate::cfagen::supported))
    {
        crate::cfagen::to_rgb(
            &mut raw,
            opts.parallel,
            s.four.as_deref(),
            s.stagger.as_ref(),
        );
    }
    Ok(raw)
}

/// CFA data with several samples per photosite (DNG SamplesPerPixel 2 or more: the parts of
/// a dual- or quad-pixel sensor): each photosite is the sum of its parts (their mean when the
/// sum could overflow 16 bits). Linearization is applied first, as it maps each part.
fn combine_samples(raw: &mut RawImage) {
    let n = raw.cpp.max(1) as usize;
    let lut = raw.linearization.take().filter(|t| !t.is_empty());
    let lin = |v: u16| -> u32 {
        match &lut {
            Some(t) => u32::from(t[(v as usize).min(t.len() - 1)]),
            None => u32::from(v),
        }
    };
    let sum = raw.white * n as f32 <= 65535.0;
    let k = n as u32;
    raw.data = raw
        .data
        .chunks_exact(n)
        .map(|p| {
            let s: u32 = p.iter().map(|&v| lin(v)).sum();
            (if sum { s } else { (s + k / 2) / k }).min(65535) as u16
        })
        .collect();
    if sum {
        raw.white *= n as f32;
        raw.black = raw.black.map(|b| b * n as f32);
    }
    raw.cpp = 1;
}

/// Rows stored as two fields (rows `0, 2, 4, ...` then `1, 3, 5, ...`) back in order.
fn prores_err(e: specular_prores::Error) -> RawError {
    match e {
        specular_prores::Error::Unsupported(_) => {
            RawError::Unsupported("ProRes RAW: a frame version this decoder does not read")
        }
        specular_prores::Error::NeedsMemory(_) => RawError::TooLarge(0),
        specular_prores::Error::Malformed(_) => RawError::Malformed("ProRes RAW: damaged frame"),
    }
}

fn interleave_fields(data: &[u16], row: usize, h: usize) -> Vec<u16> {
    let mut out = vec![0u16; data.len()];
    let half = h.div_ceil(2);
    for (i, src) in data.chunks_exact(row).take(h).enumerate() {
        let y = if i < half { 2 * i } else { 2 * (i - half) + 1 };
        out[y * row..(y + 1) * row].copy_from_slice(src);
    }
    out
}

fn wb_valid(wb: [f32; 3]) -> bool {
    wb.iter().all(|v| v.is_finite() && *v > 0.0)
}

/// Whether a region can be decoded from the sensor window behind it alone (see
/// [`crate::RawFile::region_decodable`]). The white balance of a DNG colour profile is
/// checked again at decode time.
pub(crate) fn region_ok(s: &Sensor) -> bool {
    let c = s.crop;
    let crop_ok = c.w > 0
        && c.h > 0
        && c.x as u64 + c.w as u64 <= s.width as u64
        && c.y as u64 + c.h as u64 <= s.height as u64;
    let pattern_ok = (s.cpp == 3 && s.cfa.is_none())
        || (s.cpp == 1
            && s.stagger.is_none()
            && s.cfa.as_ref().is_some_and(|cfa| cfa.bayer2x2().is_some()));
    let wb_ok = s.wb.is_some_and(wb_valid)
        || matches!(s.color, ColorSource::Dng(_) | ColorSource::Identity);
    let tiles_ok =
        matches!(&s.storage, Storage::Blocks { tile_w, tile_h, .. } if *tile_w > 0 && *tile_h > 0);
    tiles_ok
        && !s.fields
        && crop_ok
        && pattern_ok
        && wb_ok
        && matches!(s.black, Black::Levels(_))
        && s.white.is_some()
}

/// See [`crate::RawFile::region_memory`].
pub(crate) fn region_memory(s: &Sensor, region: Rect, size: OutputSize) -> u64 {
    let opts = DecodeOptions {
        size,
        region: Some(region),
        ..Default::default()
    };
    let Some(reg) = develop::output_region(s.crop, &opts) else {
        return 0;
    };
    let win = sensor_window(s, reg, size, true);
    let cpp = s.cpp as u64;
    let mut m = (win.w as u64 * win.h as u64)
        .saturating_mul(cpp * 2)
        .saturating_add((reg.w as u64 * reg.h as u64).saturating_mul(6));
    if let Storage::Blocks {
        tile_w,
        tile_h,
        codec: Codec::Lj92 | Codec::Lj92Quad | Codec::Dng(_) | Codec::Ycc { .. },
        ..
    } = &s.storage
    {
        let (tw, th) = ((*tile_w).max(1) as u64, (*tile_h).max(1) as u64);
        let across = (win.x as u64 + win.w as u64).div_ceil(tw) - win.x as u64 / tw;
        let down = (win.y as u64 + win.h as u64).div_ceil(th) - win.y as u64 / th;
        m = m.saturating_add(
            across
                .saturating_mul(down)
                .saturating_mul(tw * th)
                .saturating_mul(cpp * 2),
        );
    }
    m
}

// ---------------------------------------------------------------------------------------------
// Black / white helpers

/// Mean per 2x2 position over an optical-black area.
/// Mean of the masked area per 2x2 position (CFA data) or per channel (`cpp` 3; the fourth
/// level repeats the second).
fn masked_black(
    data: &[u16],
    w: usize,
    h: usize,
    cpp: usize,
    areas: &[Option<Rect>],
) -> Option<[f32; 4]> {
    let mut sum = [0u64; 4];
    let mut cnt = [0u64; 4];
    for a in areas.iter().flatten() {
        let (x0, y0) = (a.x as usize, a.y as usize);
        let (x1, y1) = ((x0 + a.w as usize).min(w), (y0 + a.h as usize).min(h));
        for y in y0..y1 {
            for x in x0..x1 {
                if cpp == 3 {
                    for (k, v) in data[(y * w + x) * 3..][..3].iter().enumerate() {
                        sum[k] += *v as u64;
                        cnt[k] += 1;
                    }
                } else {
                    let p = (y & 1) * 2 + (x & 1);
                    sum[p] += data[y * w + x] as u64;
                    cnt[p] += 1;
                }
            }
        }
    }
    if cpp == 3 {
        sum[3] = sum[1];
        cnt[3] = cnt[1];
    }
    if cnt.iter().any(|&c| c < 16) {
        return None;
    }
    Some(std::array::from_fn(|p| sum[p] as f32 / cnt[p] as f32))
}

fn observed_max(data: &[u16], w: usize, cpp: usize, crop: Rect, parallel: bool) -> u16 {
    let rowlen = w * cpp;
    let (x0, x1) = (crop.x as usize * cpp, (crop.x + crop.w) as usize * cpp);
    let rows = data
        .chunks_exact(rowlen)
        .skip(crop.y as usize)
        .take(crop.h as usize);
    let rmax = |r: &[u16]| {
        r.get(x0..x1.min(r.len()))
            .and_then(|s| s.iter().copied().max())
            .unwrap_or(0)
    };
    if parallel {
        rows.collect::<Vec<_>>()
            .par_iter()
            .map(|r| rmax(r))
            .max()
            .unwrap_or(0)
    } else {
        rows.map(rmax).max().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------------------------
// Strips / tiles

#[allow(clippy::too_many_arguments)]
fn decode_blocks(
    d: &[u8],
    blocks: &[Range<usize>],
    tw: usize,
    th: usize,
    codec: Codec,
    w: usize,
    cpp: usize,
    out: &mut [u16],
    opts: &DecodeOptions,
    truncated: &AtomicBool,
) -> Result<()> {
    if tw == 0 || th == 0 {
        return Err(RawError::Malformed("zero tile size"));
    }
    let rowlen = w * cpp;
    let across = w.div_ceil(tw);
    if let Codec::Lj92 | Codec::Lj92Quad | Codec::Dng(_) | Codec::Ycc { .. } = codec {
        return decode_tiles(d, blocks, tw, th, codec, w, cpp, out, opts, truncated);
    }
    // A single uncompressed strip (RAF, many TIFF-based raws): cut it into row groups so the
    // unpacking runs in parallel.
    const SPLIT: usize = STRIP_SPLIT;
    let split: Vec<Range<usize>>;
    let (blocks, th) = match codec {
        Codec::Raw { packing, row_bytes } if blocks.len() == 1 && across == 1 && th > SPLIT => {
            let rb = if row_bytes == 0 {
                packed_row_bytes(packing, tw * cpp)
            } else {
                row_bytes
            };
            let r = blocks[0].clone();
            let h = out.len() / rowlen.max(1);
            split = (0..h.min(th))
                .step_by(SPLIT)
                .map(|y| {
                    let s = r.start.saturating_add(y.saturating_mul(rb)).min(r.end);
                    s..r.end.min(s.saturating_add(SPLIT.saturating_mul(rb)))
                })
                .collect();
            (&split[..], SPLIT)
        }
        _ => (blocks, th),
    };
    let band = th.saturating_mul(rowlen);
    let work = |(bi, chunk): (usize, &mut [u16])| -> Result<()> {
        if cancelled(opts) {
            return Err(RawError::Cancelled);
        }
        let rows = chunk.len() / rowlen;
        for tc in 0..across {
            let x0 = tc * tw;
            let cw = tw.min(w - x0);
            let Some(src) = blocks.get(bi * across + tc).and_then(|r| d.get(r.clone())) else {
                truncated.store(true, Ordering::Relaxed);
                continue;
            };
            match codec {
                Codec::Raw { packing, row_bytes } => {
                    let row_bytes = if row_bytes == 0 {
                        packed_row_bytes(packing, tw * cpp)
                    } else {
                        row_bytes
                    };
                    for y in 0..rows {
                        let dst = &mut chunk[y * rowlen + x0 * cpp..y * rowlen + (x0 + cw) * cpp];
                        let s = src.get(y * row_bytes..).unwrap_or(&[]);
                        if unpack(packing, s, dst) {
                            truncated.store(true, Ordering::Relaxed);
                        }
                    }
                }
                Codec::Lj92 | Codec::Lj92Quad | Codec::Dng(_) | Codec::Ycc { .. } => {}
            }
        }
        Ok(())
    };
    if opts.parallel {
        out.par_chunks_mut(band).enumerate().try_for_each(work)
    } else {
        out.chunks_mut(band).enumerate().try_for_each(work)
    }
}

/// Rows per group a single uncompressed strip is cut into.
const STRIP_SPLIT: usize = 64;

/// Tile grid of a [`Storage::Blocks`] sensor.
#[derive(Clone, Copy)]
struct Geometry {
    tw: usize,
    th: usize,
    w: usize,
    h: usize,
    cpp: usize,
}

thread_local! {
    static SCRATCH: RefCell<lj92tile::Scratch> = RefCell::new(lj92tile::Scratch::default());
}

/// The sensor samples of `win` (sensor coordinates) into `out` (`win.w * win.h * cpp`):
/// lossless JPEG tiles through the tile cache (each tile decoded whole once), uncompressed
/// blocks row by row.
#[allow(clippy::too_many_arguments)]
fn decode_blocks_window(
    d: &[u8],
    blocks: &[Range<usize>],
    g: Geometry,
    codec: Codec,
    win: Rect,
    out: &mut [u16],
    opts: &DecodeOptions,
    truncated: &AtomicBool,
) -> Result<()> {
    let Geometry { tw, th, w, h, cpp } = g;
    if tw == 0 || th == 0 {
        return Err(RawError::Malformed("zero tile size"));
    }
    let (wx, wy) = (win.x as usize, win.y as usize);
    let (ww, wh) = (win.w as usize, win.h as usize);
    if ww == 0 || wh == 0 || wx + ww > w || wy + wh > h {
        return Err(RawError::Malformed("region outside the sensor"));
    }
    let across = w.div_ceil(tw);
    let wrow = ww * cpp;
    let layout = match codec {
        Codec::Raw { packing, row_bytes } => {
            let rb = if row_bytes == 0 {
                packed_row_bytes(packing, tw * cpp)
            } else {
                row_bytes
            };
            let (tc0, tc1) = (wx / tw, (wx + ww - 1) / tw);
            let strip = blocks.len() == 1 && across == 1 && th > STRIP_SPLIT;
            let row = |tmp: &mut Vec<u16>, (r, dst): (usize, &mut [u16])| -> Result<()> {
                if r % 64 == 0 && cancelled(opts) {
                    return Err(RawError::Cancelled);
                }
                let y = wy + r;
                for tc in tc0..=tc1 {
                    let x0 = tc * tw;
                    let cw = tw.min(w - x0);
                    let Some(b) = blocks.get((y / th) * across + tc) else {
                        truncated.store(true, Ordering::Relaxed);
                        continue;
                    };
                    // The whole-sensor decode reads a single strip in groups of rows (see
                    // `decode_blocks`); a group past the end of the file is missing there.
                    let (b, ry) = if strip {
                        let g = (y / STRIP_SPLIT) * STRIP_SPLIT;
                        let s = b.start.saturating_add(g.saturating_mul(rb)).min(b.end);
                        let e = b.end.min(s.saturating_add(STRIP_SPLIT.saturating_mul(rb)));
                        (s..e, y - g)
                    } else {
                        (b.clone(), y % th)
                    };
                    let Some(src) = d.get(b) else {
                        truncated.store(true, Ordering::Relaxed);
                        continue;
                    };
                    tmp.clear();
                    tmp.resize(cw * cpp, 0);
                    let s = src.get(ry.saturating_mul(rb)..).unwrap_or(&[]);
                    if unpack(packing, s, tmp) {
                        truncated.store(true, Ordering::Relaxed);
                    }
                    // Columns of this block inside the window.
                    let a = x0.max(wx);
                    let b = (x0 + cw).min(wx + ww);
                    dst[(a - wx) * cpp..(b - wx) * cpp]
                        .copy_from_slice(&tmp[(a - x0) * cpp..(b - x0) * cpp]);
                }
                Ok(())
            };
            return if opts.parallel {
                out.par_chunks_mut(wrow)
                    .enumerate()
                    .try_for_each_init(Vec::new, row)
            } else {
                let mut tmp = Vec::new();
                out.chunks_mut(wrow)
                    .enumerate()
                    .try_for_each(|r| row(&mut tmp, r))
            };
        }
        Codec::Lj92 | Codec::Dng(_) | Codec::Ycc { .. } => lj92tile::Layout::RowMajor,
        Codec::Lj92Quad => lj92tile::Layout::Quad,
    };
    let local;
    let cache = match opts.tiles {
        Some(c) => c,
        None => {
            local = TileCache::new(u64::MAX);
            &local
        }
    };
    cache.bind(layout_sig(d, blocks, g, codec));
    let (tc0, tc1) = (wx / tw, (wx + ww - 1) / tw);
    let (tr0, tr1) = (wy / th, (wy + wh - 1) / th);
    let wanted: Vec<(usize, usize)> = (tr0..=tr1)
        .flat_map(|tr| (tc0..=tc1).map(move |tc| (tc, tr)))
        .collect();
    let get = |&(tc, tr): &(usize, usize)| -> Result<Arc<Tile>> {
        if cancelled(opts) {
            return Err(RawError::Cancelled);
        }
        let i = tr * across + tc;
        Ok(cache.get(i, || {
            let (x0, y0) = (tc * tw, tr * th);
            let row = tw.min(w - x0) * cpp;
            let rows = th.min(h - y0);
            let mut data = vec![0u16; row * rows];
            let src = blocks.get(i).and_then(|r| d.get(r.clone()));
            let bad = match src {
                Some(src) => {
                    let mut segs: Vec<&mut [u16]> = data.chunks_mut(row).collect();
                    if let Codec::Dng(t) = codec {
                        dngtile::decode_tile(src, t, tw, th, cpp, &mut segs)
                    } else if let Codec::Ycc { half } = codec {
                        crate::ycc::decode_tile(src, half as i32, tw, th, &mut segs)
                    } else {
                        SCRATCH.with(|s| {
                            lj92tile::decode_tile(
                                src,
                                layout,
                                tw * cpp,
                                th,
                                &mut segs,
                                &mut s.borrow_mut(),
                            )
                            .unwrap_or(true)
                        })
                    }
                }
                None => true,
            };
            Tile {
                data,
                row,
                truncated: bad,
            }
        }))
    };
    let tiles: Vec<Arc<Tile>> = if opts.parallel {
        wanted
            .par_iter()
            .with_min_len(1)
            .map(get)
            .collect::<Result<_>>()?
    } else {
        wanted.iter().map(get).collect::<Result<_>>()?
    };
    for (&(tc, tr), t) in wanted.iter().zip(&tiles) {
        if t.truncated {
            truncated.store(true, Ordering::Relaxed);
        }
        let (x0, y0) = (tc * tw, tr * th);
        let cw = t.row / cpp.max(1);
        let a = x0.max(wx);
        let b = (x0 + cw).min(wx + ww);
        let rows = t.data.len() / t.row.max(1);
        for y in y0.max(wy)..(y0 + rows).min(wy + wh) {
            let src = &t.data[(y - y0) * t.row..][(a - x0) * cpp..(b - x0) * cpp];
            out[(y - wy) * wrow + (a - wx) * cpp..][..src.len()].copy_from_slice(src);
        }
    }
    Ok(())
}

/// Identifies a tile layout (and the file behind it) for [`TileCache::bind`].
fn layout_sig(d: &[u8], blocks: &[Range<usize>], g: Geometry, codec: Codec) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    for v in [g.tw, g.th, g.w, g.h, g.cpp, blocks.len(), d.len()] {
        mix(v as u64);
    }
    mix(match codec {
        Codec::Lj92Quad => 1,
        Codec::Dng(_) => 2,
        Codec::Ycc { .. } => 3,
        _ => 0,
    });
    for b in blocks.first().into_iter().chain(blocks.last()) {
        mix(b.start as u64);
        mix(b.end as u64);
    }
    h
}

/// Lossless JPEG and other DNG tiles, decoded tile-parallel straight into `out`.
#[allow(clippy::too_many_arguments)]
fn decode_tiles(
    d: &[u8],
    blocks: &[Range<usize>],
    tw: usize,
    th: usize,
    codec: Codec,
    w: usize,
    cpp: usize,
    out: &mut [u16],
    opts: &DecodeOptions,
    truncated: &AtomicBool,
) -> Result<()> {
    let rowlen = w * cpp;
    let stride = tw * cpp;
    let across = w.div_ceil(tw);
    // Each tile's row segments inside the image, in block order.
    let mut tiles: Vec<Vec<&mut [u16]>> = Vec::new();
    for band in out.chunks_mut(th * rowlen) {
        let first = tiles.len();
        // `th` comes from the file; the band holds at most that many rows.
        let rows = th.min(band.len() / rowlen.max(1));
        tiles.extend((0..across).map(|_| Vec::with_capacity(rows)));
        for row in band.chunks_mut(rowlen) {
            for (tc, seg) in row.chunks_mut(stride).enumerate() {
                tiles[first + tc].push(seg);
            }
        }
    }
    let work = |s: &mut lj92tile::Scratch,
                (i, rows): (usize, &mut Vec<&mut [u16]>)|
     -> Result<()> {
        if cancelled(opts) {
            return Err(RawError::Cancelled);
        }
        let src = blocks.get(i).and_then(|r| d.get(r.clone()));
        let bad = match src {
            Some(src) => match codec {
                Codec::Dng(t) => dngtile::decode_tile(src, t, tw, th, cpp, rows),
                Codec::Ycc { half } => crate::ycc::decode_tile(src, half as i32, tw, th, rows),
                Codec::Lj92Quad => {
                    lj92tile::decode_tile(src, lj92tile::Layout::Quad, stride, th, rows, s)
                        .unwrap_or(true)
                }
                _ => lj92tile::decode_tile(src, lj92tile::Layout::RowMajor, stride, th, rows, s)
                    .unwrap_or(true),
            },
            None => true,
        };
        if bad {
            truncated.store(true, Ordering::Relaxed);
        }
        Ok(())
    };
    if opts.parallel {
        tiles
            .par_iter_mut()
            .enumerate()
            .with_min_len(1)
            .try_for_each_init(lj92tile::Scratch::default, work)
    } else {
        let mut s = lj92tile::Scratch::default();
        tiles
            .iter_mut()
            .enumerate()
            .try_for_each(|t| work(&mut s, t))
    }
}

/// The unclipped floating point samples of a floating point DNG (whole sensor, as stored,
/// scaled like the 16-bit samples: white at 65535), or `None` for any other storage.
pub(crate) fn decode_float(
    d: &[u8],
    s: &Sensor,
    opts: &DecodeOptions,
) -> Result<Option<(Vec<f32>, bool)>> {
    let Storage::Blocks {
        blocks,
        tile_w,
        tile_h,
        codec: Codec::Dng(t),
    } = &s.storage
    else {
        return Ok(None);
    };
    // Samples the decode reshapes (added up, demosaiced into camera RGB) go the 16-bit way.
    let reshaped = s
        .cfa
        .as_ref()
        .is_some_and(|c| s.cpp > 1 || s.stagger.is_some() || crate::cfagen::supported(c));
    if !t.float || s.unsupported.is_some() || reshaped {
        return Ok(None);
    }
    let (w, h, cpp) = (s.width as usize, s.height as usize, s.cpp as usize);
    let (tw, th) = (*tile_w as usize, *tile_h as usize);
    if tw == 0 || th == 0 {
        return Err(RawError::Malformed("zero tile size"));
    }
    let n = w as u64 * h as u64 * cpp as u64;
    if n == 0 || n > opts.max_pixels || n > (d.len() as u64).saturating_mul(64) {
        return Err(RawError::Malformed(
            "sensor size inconsistent with file size",
        ));
    }
    let mut out = vec![0f32; n as usize];
    let rowlen = w * cpp;
    let stride = tw * cpp;
    let across = w.div_ceil(tw);
    let mut tiles: Vec<Vec<&mut [f32]>> = Vec::new();
    for band in out.chunks_mut(th * rowlen) {
        let first = tiles.len();
        let rows = th.min(band.len() / rowlen.max(1));
        tiles.extend((0..across).map(|_| Vec::with_capacity(rows)));
        for row in band.chunks_mut(rowlen) {
            for (tc, seg) in row.chunks_mut(stride).enumerate() {
                tiles[first + tc].push(seg);
            }
        }
    }
    let t = *t;
    let short = AtomicBool::new(false);
    let work = |(i, rows): (usize, &mut Vec<&mut [f32]>)| -> Result<()> {
        if cancelled(opts) {
            return Err(RawError::Cancelled);
        }
        // A short or corrupt block leaves zeros, as the 16-bit decode does.
        let bad = match blocks.get(i).and_then(|r| d.get(r.clone())) {
            Some(src) => dngtile::decode_tile_f32(src, t, tw, th, cpp, rows),
            None => true,
        };
        if bad {
            short.store(true, Ordering::Relaxed);
        }
        Ok(())
    };
    if opts.parallel {
        tiles.par_iter_mut().enumerate().try_for_each(work)?;
    } else {
        tiles.iter_mut().enumerate().try_for_each(work)?;
    }
    Ok(Some((out, short.into_inner())))
}

fn packed_row_bytes(p: Packing, samples: usize) -> usize {
    match p {
        Packing::U8 => samples,
        Packing::U16Le | Packing::U16Be | Packing::U16Hi12(_) => samples * 2,
        Packing::Msb(b) => (samples * b as usize).div_ceil(8),
        Packing::Le12 => (samples * 12).div_ceil(8),
        Packing::Msb32Le(b) => (samples * b as usize).div_ceil(32) * 4,
        Packing::Msb16Le(b) => (samples * b as usize).div_ceil(16) * 2,
        Packing::Msb12Pad10 | Packing::Le12Pad10 => samples.div_ceil(10) * 16,
    }
}

/// Unpack one row; returns true if the source was short (missing samples stay zero).
pub(crate) fn unpack(p: Packing, src: &[u8], dst: &mut [u16]) -> bool {
    let n = dst.len();
    match p {
        Packing::U8 => {
            let m = n.min(src.len());
            for (o, &b) in dst[..m].iter_mut().zip(src) {
                *o = b as u16;
            }
            m < n
        }
        Packing::U16Le | Packing::U16Be => {
            let m = n.min(src.len() / 2);
            for (o, b) in dst[..m].iter_mut().zip(src.chunks_exact(2)) {
                *o = if p == Packing::U16Le {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                };
            }
            m < n
        }
        Packing::U16Hi12(le) => {
            let m = n.min(src.len() / 2);
            for (o, b) in dst[..m].iter_mut().zip(src.chunks_exact(2)) {
                let v = if le {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                };
                *o = v >> 4;
            }
            m < n
        }
        Packing::Le12 => {
            let mut i = 0;
            for b in src.chunks_exact(3) {
                if i >= n {
                    break;
                }
                dst[i] = b[0] as u16 | ((b[1] as u16 & 0xf) << 8);
                if i + 1 < n {
                    dst[i + 1] = (b[1] as u16 >> 4) | ((b[2] as u16) << 4);
                }
                i += 2;
            }
            i < n
        }
        Packing::Msb32Le(bits) => {
            let mut i = 0;
            if bits == 14 {
                // 7 words = 16 samples.
                for c in src.chunks_exact(28) {
                    if i + 16 > n {
                        break;
                    }
                    let wd = |j: usize| {
                        u32::from_le_bytes([c[4 * j], c[4 * j + 1], c[4 * j + 2], c[4 * j + 3]])
                            as u128
                    };
                    let a = (wd(0) << 96) | (wd(1) << 64) | (wd(2) << 32) | wd(3);
                    let b = (wd(3) << 96) | (wd(4) << 64) | (wd(5) << 32) | wd(6);
                    let out = &mut dst[i..i + 16];
                    for (k, o) in out[..9].iter_mut().enumerate() {
                        *o = ((a >> (114 - 14 * k)) & 0x3fff) as u16;
                    }
                    for (k, o) in out[9..].iter_mut().enumerate() {
                        *o = ((b >> (84 - 14 * k)) & 0x3fff) as u16;
                    }
                    i += 16;
                }
            }
            unpack_msb32le(bits, src.get(i / 16 * 28..).unwrap_or(&[]), &mut dst[i..])
        }
        Packing::Msb16Le(bits) => {
            let bits = bits.clamp(1, 16);
            let mask = (1u64 << bits) - 1;
            let mut acc: u64 = 0;
            let mut nb = 0u32;
            let mut words = src.chunks(2);
            for o in dst.iter_mut() {
                if nb < bits {
                    let Some(c) = words.next() else { return true };
                    acc = (acc << 16) | u16::from_le_bytes([c[0], *c.get(1).unwrap_or(&0)]) as u64;
                    nb += 16;
                }
                nb -= bits;
                *o = ((acc >> nb) & mask) as u16;
                acc &= (1u64 << nb) - 1;
            }
            false
        }
        Packing::Msb12Pad10 | Packing::Le12Pad10 => {
            let inner = if p == Packing::Msb12Pad10 {
                Packing::Msb(12)
            } else {
                Packing::Le12
            };
            let mut i = 0;
            for g in src.chunks(16) {
                if i >= n {
                    break;
                }
                let k = (n - i).min(10);
                let need = (k * 12).div_ceil(8);
                if g.len() < need {
                    unpack(inner, g, &mut dst[i..i + k]);
                    return true;
                }
                unpack(inner, &g[..need], &mut dst[i..i + k]);
                i += k;
            }
            i < n
        }
        Packing::Msb(bits) => {
            let bits = bits.clamp(1, 16);
            let mask = (1u64 << bits) - 1;
            if bits == 12 {
                let mut i = 0;
                for b in src.chunks_exact(3) {
                    if i + 1 >= n {
                        break;
                    }
                    dst[i] = ((b[0] as u16) << 4) | (b[1] as u16 >> 4);
                    dst[i + 1] = ((b[1] as u16 & 0xf) << 8) | b[2] as u16;
                    i += 2;
                }
                if i < n {
                    // Odd tail.
                    let at = i / 2 * 3;
                    match (src.get(at), src.get(at + 1)) {
                        (Some(&a), Some(&b)) => dst[i] = ((a as u16) << 4) | (b as u16 >> 4),
                        _ => return true,
                    }
                    i += 1;
                }
                return i < n;
            }
            let mut acc: u64 = 0;
            let mut nb = 0u32;
            let mut it = src.iter();
            for o in dst.iter_mut() {
                while nb < bits {
                    let Some(&b) = it.next() else { return true };
                    acc = (acc << 8) | b as u64;
                    nb += 8;
                }
                nb -= bits;
                *o = ((acc >> nb) & mask) as u16;
                acc &= (1u64 << nb) - 1;
            }
            false
        }
    }
}

/// MSB-first bits over little-endian 32-bit words (generic path of [`Packing::Msb32Le`]).
fn unpack_msb32le(bits: u32, src: &[u8], dst: &mut [u16]) -> bool {
    let bits = bits.clamp(1, 16);
    let mask = (1u64 << bits) - 1;
    let mut acc: u64 = 0;
    let mut nb = 0u32;
    let mut words = src.chunks(4);
    for o in dst.iter_mut() {
        if nb < bits {
            let Some(c) = words.next() else { return true };
            let mut b = [0u8; 4];
            b[..c.len()].copy_from_slice(c);
            acc = (acc << 32) | u32::from_le_bytes(b) as u64;
            nb += 32;
        }
        nb -= bits;
        *o = ((acc >> nb) & mask) as u16;
        acc &= (1u64 << nb) - 1;
    }
    false
}

// ---------------------------------------------------------------------------------------------
// Canon CR2

/// Canon stores the frame as vertical slices: `n` slices of `sw` columns then one of `lw`,
/// each filled top to bottom.
fn unslice(buf: &[u16], out: &mut [u16], w: usize, h: usize, slices: Option<[u32; 3]>) {
    let Some([cnt, sw, lw]) = slices else {
        let m = buf.len().min(out.len());
        out[..m].copy_from_slice(&buf[..m]);
        return;
    };
    let (cnt, sw, lw) = (cnt as usize, sw as usize, lw as usize);
    // Empty slices place nothing; skip them rather than looping up to 4e9 times.
    let cnt = if sw == 0 { 0 } else { cnt };
    let mut i = 0;
    for k in 0..=cnt {
        let wk = if k < cnt { sw } else { lw };
        let x0 = k * sw;
        if wk == 0 || x0 + wk > w {
            continue;
        }
        for y in 0..h {
            let Some(src) = buf.get(i..i + wk) else {
                return;
            };
            out[y * w + x0..y * w + x0 + wk].copy_from_slice(src);
            i += wk;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Nikon NEF Huffman (after dcraw's nikon_load_raw; the decoder tables are format constants)

const NIKON_TREE: [[u8; 32]; 6] = {
    const fn t(src: &[u8]) -> [u8; 32] {
        let mut o = [0u8; 32];
        let mut i = 0;
        while i < src.len() {
            o[i] = src[i];
            i += 1;
        }
        o
    }
    [
        // 12-bit lossy
        t(&[
            0, 1, 5, 1, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 5, 4, 3, 6, 2, 7, 1, 0, 8, 9, 11, 10,
            12,
        ]),
        // 12-bit lossy after split
        t(&[
            0, 1, 5, 1, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0x39, 0x5a, 0x38, 0x27, 0x16, 5, 4, 3,
            2, 1, 0, 11, 12, 12,
        ]),
        // 12-bit lossless
        t(&[
            0, 1, 4, 2, 3, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 4, 6, 3, 7, 2, 8, 1, 9, 0, 10, 11,
            12,
        ]),
        // 14-bit lossy
        t(&[
            0, 1, 4, 3, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 5, 6, 4, 7, 8, 3, 9, 2, 1, 0, 10, 11,
            12, 13, 14,
        ]),
        // 14-bit lossy after split
        t(&[
            0, 1, 5, 1, 1, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 8, 0x5c, 0x4b, 0x3a, 0x29, 7, 6, 5, 4,
            3, 2, 1, 0, 13, 14,
        ]),
        // 14-bit lossless
        t(&[
            0, 1, 4, 2, 2, 3, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 7, 6, 8, 5, 9, 4, 10, 3, 11, 12, 2, 0,
            1, 13, 14,
        ]),
    ]
};

fn nikon_huff(tree: usize) -> Result<Huffman> {
    let t = &NIKON_TREE[tree];
    let counts: [u8; 16] = t[..16].try_into().unwrap_or_default();
    Huffman::new(&counts, &t[16..], false).ok_or(RawError::Malformed("NEF Huffman table"))
}

/// Returns (truncated, white level).
fn nikon(
    src: &[u8],
    meta: &[u8],
    le: bool,
    w: usize,
    h: usize,
    bits: u32,
    out: &mut [u16],
) -> Result<(bool, f32)> {
    let en = Endian { le };
    let g2 = |p: usize| en.u16(meta, p);
    let (ver0, ver1) = (*meta.first().unwrap_or(&0), *meta.get(1).unwrap_or(&0));
    let mut p = 2;
    if ver0 == 0x49 || ver1 == 0x58 {
        p += 2110;
    }
    let mut tree = if ver0 == 0x46 { 2 } else { 0 };
    if bits == 14 {
        tree += 3;
    }
    let mut vpred = [[0u16; 2]; 2];
    for (i, v) in vpred.iter_mut().flatten().enumerate() {
        *v = g2(p + 2 * i).unwrap_or(0);
    }
    p += 8;
    let mut max = ((1usize << bits.min(16)) & 0x7fff).max(2);
    let csize = g2(p).unwrap_or(0) as usize;
    p += 2;
    let mut curve: Vec<u16> = (0..=0xffffu32).map(|i| i as u16).collect();
    let mut step = if csize > 1 { max / (csize - 1) } else { 0 };
    let mut split = 0usize;
    // Lossy type 2 ("D " / "D@"): csize knots spaced `step` apart, linearly interpolated. The
    // "D@" variant (D780, Z 5, Z 7II) packs the curve into a quarter of the code range, so its
    // knots sit step / 4 apart and the decoded values only reach max / 4.
    if ver0 == 0x44 && (ver1 == 0x20 || ver1 == 0x40 && step > 3) && step > 0 {
        if ver1 == 0x40 {
            step /= 4;
            max /= 4;
        }
        for i in 0..csize {
            if let (Some(v), Some(c)) = (g2(p + 2 * i), curve.get_mut(i * step)) {
                *c = v;
            }
        }
        for i in 0..max {
            let b = i - i % step;
            let (a, c) = (
                curve[b] as u32,
                *curve.get(b + step).unwrap_or(&curve[b]) as u32,
            );
            curve[i] =
                ((a * (step - i % step) as u32 + c * (i % step) as u32) / step as u32) as u16;
        }
        split = g2(562).unwrap_or(0) as usize;
    } else if ver0 != 0x46 && (2..=0x4001).contains(&csize) {
        for (i, v) in curve.iter_mut().enumerate().take(csize) {
            *v = g2(p + 2 * i).unwrap_or(*v);
        }
        max = csize;
    }
    while max > 2 && curve[max - 2] == curve[max - 1] {
        max -= 1;
    }
    let white = curve[max - 1] as f32;
    let mut huff = nikon_huff(tree)?;
    let mut br = BitReader::new(src, false);
    let mut hpred = [0u16; 2];
    for row in 0..h {
        if split > 0 && row == split {
            huff = nikon_huff(tree + 1)?;
        }
        let line = &mut out[row * w..(row + 1) * w];
        for (col, o) in line.iter_mut().enumerate() {
            let sym = huff.decode(&mut br);
            let len = sym & 15;
            let shl = sym >> 4;
            let mut diff: i32 = 0;
            if len > 0 {
                let v = br.bits(len.saturating_sub(shl));
                diff = ((((v << 1) + 1) << shl) >> 1) as i32;
                if diff & (1 << (len - 1)) == 0 {
                    diff -= (1 << len) - (shl == 0) as i32;
                }
            }
            if col < 2 {
                vpred[row & 1][col] = vpred[row & 1][col].wrapping_add(diff as u16);
                hpred[col] = vpred[row & 1][col];
            } else {
                hpred[col & 1] = hpred[col & 1].wrapping_add(diff as u16);
            }
            *o = curve[(hpred[col & 1] as i16).clamp(0, 0x3fff) as usize];
        }
    }
    Ok((br.overrun(), white))
}

// ---------------------------------------------------------------------------------------------
// Sony ARW 1.0: a single stream of Huffman-coded differences (JPEG-style categories and extra
// bits) accumulated across the whole image. Columns run right to left; within a column the
// even rows come first, then the odd rows. Codes, by the number of leading zero bits k before
// the first one: k = 0: `11` = 1, `10` = 2; k = 1: `011` = 0, `010` = 3; k >= 2: k + 1 bits
// for category k + 2; fifteen zeros: category 17.

fn arw1_category(br: &mut crate::bits::BitReader) -> u32 {
    if br.nbits() < 32 {
        br.refill();
    }
    let v = br.peek(15);
    let k = v.leading_zeros() - 17;
    match k {
        0 => {
            br.consume(2);
            if v & 0x2000 != 0 { 1 } else { 2 }
        }
        1 => {
            br.consume(3);
            if v & 0x1000 != 0 { 0 } else { 3 }
        }
        15 => {
            br.consume(15);
            17
        }
        k => {
            br.consume(k + 1);
            k + 2
        }
    }
}

/// Decode a `w x h` ARW 1.0 image coded with `rows` rows per column. Returns whether the
/// stream ran out or produced out-of-range samples.
fn sony_arw1(src: &[u8], w: usize, h: usize, rows: usize, out: &mut [u16]) -> bool {
    let mut br = crate::bits::BitReader::new(src, false);
    let mut sum: i32 = 0;
    let mut bad = false;
    for col in (0..w).rev() {
        let mut row = 0;
        while row < rows + 1 {
            if row == rows {
                row = 1;
            }
            let s = arw1_category(&mut br);
            let diff = match s {
                0 => 0,
                16 => -32768,
                s => crate::huffman::extend(br.bits(s), s),
            };
            sum = sum.wrapping_add(diff);
            bad |= sum >> 12 != 0;
            if row < h {
                out[row * w + col] = sum as u16;
            }
            row += 2;
        }
        if br.overrun() {
            return true;
        }
    }
    bad || br.overrun()
}

// ---------------------------------------------------------------------------------------------
// Sony ARW2: rows of 32-pixel blocks (two interleaved 16-pixel groups), each 16 bytes holding
// an 11-bit max/min, their positions and 14 7-bit deltas scaled by a shift.

fn sony_arw2(
    d: &[u8],
    range: Range<usize>,
    curve: &[u16],
    w: usize,
    out: &mut [u16],
    opts: &DecodeOptions,
    truncated: &AtomicBool,
) {
    let row = |(r, line): (usize, &mut [u16])| {
        let start = range.start + r * w;
        let src = if start + w <= range.end {
            d.get(start..start + w)
        } else {
            None
        };
        let Some(src) = src else {
            truncated.store(true, Ordering::Relaxed);
            return;
        };
        let b = |i: usize| *src.get(i).unwrap_or(&0) as u32;
        let mut col: usize = 0;
        let mut dp = 0;
        let mut pix = [0u32; 16];
        while col + 30 < w {
            let val = b(dp) | b(dp + 1) << 8 | b(dp + 2) << 16 | b(dp + 3) << 24;
            let max = 0x7ff & val;
            let min = 0x7ff & (val >> 11);
            let imax = (0x0f & (val >> 22)) as usize;
            let imin = (0x0f & (val >> 26)) as usize;
            let mut sh = 0;
            while sh < 4 && (0x80 << sh) <= max.saturating_sub(min) {
                sh += 1;
            }
            let mut bit = 30;
            for (i, px) in pix.iter_mut().enumerate() {
                if i == imax {
                    *px = max;
                } else if i == imin {
                    *px = min;
                } else {
                    let q = dp + (bit >> 3);
                    let v = (b(q) | b(q + 1) << 8) >> (bit & 7) & 0x7f;
                    *px = ((v << sh) + min).min(0x7ff);
                    bit += 7;
                }
            }
            for px in pix {
                if let Some(o) = line.get_mut(col) {
                    *o = curve[(px << 1) as usize];
                }
                col += 2;
            }
            col -= if col & 1 == 1 { 1 } else { 31 };
            dp += 16;
        }
    };
    if opts.parallel {
        out.par_chunks_mut(w).enumerate().for_each(row);
    } else {
        out.chunks_mut(w).enumerate().for_each(row);
    }
}

// ---------------------------------------------------------------------------------------------
// Panasonic RW2 (raw format <= 4): 0x4000-byte blocks read rotated by `load_flags`, bits taken
// backwards; 14-pixel groups with two interleaved predictors.

struct PanaBits<'a> {
    src: &'a [u8],
    pos: usize,
    buf: Box<[u8; 0x4001]>,
    vbits: u32,
    load_flags: usize,
    short: bool,
}

impl PanaBits<'_> {
    fn fill(&mut self, dst: Range<usize>) {
        let n = dst.len();
        let avail = self.src.len().saturating_sub(self.pos).min(n);
        self.buf[dst.start..dst.start + avail]
            .copy_from_slice(&self.src[self.pos..self.pos + avail]);
        self.buf[dst.start + avail..dst.end].fill(0);
        if avail < n {
            self.short = true;
        }
        self.pos += avail;
    }

    #[inline]
    fn bits(&mut self, n: u32) -> u32 {
        if self.vbits == 0 {
            let lf = self.load_flags;
            self.fill(lf..0x4000);
            self.fill(0..lf);
        }
        self.vbits = self.vbits.wrapping_sub(n) & 0x1ffff;
        let byte = ((self.vbits >> 3) ^ 0x3ff0) as usize;
        ((self.buf[byte] as u32 | (self.buf[byte + 1] as u32) << 8) >> (self.vbits & 7))
            & ((1 << n) - 1)
    }
}

fn panasonic(src: &[u8], load_flags: usize, w: usize, h: usize, out: &mut [u16]) -> bool {
    let mut pb = PanaBits {
        src,
        pos: 0,
        buf: Box::new([0; 0x4001]),
        vbits: 0,
        load_flags: load_flags.min(0x4000),
        short: false,
    };
    let mut pred = [0i32; 2];
    let mut nonz = [0i32; 2];
    let mut sh = 0;
    for row in 0..h {
        for col in 0..w {
            let i = col % 14;
            if i == 0 {
                pred = [0; 2];
                nonz = [0; 2];
            }
            if i % 3 == 2 {
                sh = 4 >> (3 - pb.bits(2));
            }
            let k = i & 1;
            if nonz[k] != 0 {
                let j = pb.bits(8) as i32;
                if j != 0 {
                    pred[k] -= 0x80 << sh;
                    if pred[k] < 0 || sh == 4 {
                        pred[k] &= !(-1i32 << sh);
                    }
                    pred[k] += j << sh;
                }
            } else {
                nonz[k] = pb.bits(8) as i32;
                if nonz[k] != 0 || i > 11 {
                    pred[k] = (nonz[k] << 4) | pb.bits(4) as i32;
                }
            }
            out[row * w + col] = pred[col & 1].clamp(0, 0xffff) as u16;
        }
        if pb.short && pb.pos >= src.len() && row + 1 < h {
            // Out of data: the rest stays zero.
            return true;
        }
    }
    pb.short
}

// ---------------------------------------------------------------------------------------------
// Pentax PEF Huffman (table from maker-note tag 0x220)

fn pentax(src: &[u8], meta: &[u8], le: bool, w: usize, out: &mut [u16]) -> Result<bool> {
    let en = Endian { le };
    let dep = ((en.u16(meta, 0).ok_or(RawError::Malformed("PEF table"))? as usize) + 12) & 15;
    let mut codes = [0u16; 15];
    let mut lens = [0u8; 15];
    for c in 0..dep {
        codes[c] = en
            .u16(meta, 14 + 2 * c)
            .ok_or(RawError::Malformed("PEF table"))?;
        lens[c] = *meta
            .get(14 + 2 * dep + c)
            .ok_or(RawError::Malformed("PEF table"))?;
    }
    // 12-bit lookup: (len << 8) | category.
    let mut huff = vec![0u16; 4097];
    for c in 0..dep {
        if lens[c] == 0 || lens[c] > 12 {
            continue;
        }
        let mut i = codes[c] as usize & 4095;
        let end = (codes[c] as usize + (4096 >> lens[c]) - 1) & 4095;
        while i <= end && i < 4096 {
            i += 1;
            huff[i] = (lens[c] as u16) << 8 | c as u16;
        }
    }
    let mut br = BitReader::new(src, false);
    let mut vpred = [[0u16; 2]; 2];
    let mut hpred = [0u16; 2];
    for (row, line) in out.chunks_exact_mut(w).enumerate() {
        for (col, o) in line.iter_mut().enumerate() {
            if br.nbits() < 32 {
                br.refill();
            }
            let e = huff[br.peek(12) as usize + 1];
            br.consume((e >> 8) as u32);
            let len = (e & 0xff) as u32;
            let diff: i32 = match len {
                0 => 0,
                16 => -32768,
                l if l < 16 => crate::huffman::extend(br.bits(l), l),
                _ => 0,
            };
            if col < 2 {
                vpred[row & 1][col] = vpred[row & 1][col].wrapping_add(diff as u16);
                hpred[col] = vpred[row & 1][col];
            } else {
                hpred[col & 1] = hpred[col & 1].wrapping_add(diff as u16);
            }
            *o = hpred[col & 1];
        }
    }
    Ok(br.overrun())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Samsung's 4704-wide sensors have masked strips left and right: the black is their
    /// mean (as LibRaw measures it; the NX100's left strip alone read 1.2 high).
    #[test]
    fn masked_black_over_two_strips() {
        let (w, h) = (24usize, 16usize);
        let mut d = vec![1000u16; w * h];
        for y in 0..h {
            for x in 0..4 {
                d[y * w + x] = 6;
                d[y * w + w - 4 + x] = 2;
            }
        }
        let r = |x: u32| Rect {
            x,
            y: 0,
            w: 4,
            h: 16,
        };
        let both = masked_black(&d, w, h, 1, &[Some(r(0)), Some(r(20))]).unwrap();
        assert_eq!(both, [4.0; 4]);
        let left = masked_black(&d, w, h, 1, &[Some(r(0)), None]).unwrap();
        assert_eq!(left, [6.0; 4]);
    }

    /// Olympus E-1 uncompressed ORF: 12 bits in the high end of 16-bit words. Read as plain
    /// 16-bit, every level was 16 times LibRaw's (white 65520) and the black from the
    /// makernote (in 12-bit units) no longer matched.
    #[test]
    fn unpack_high_12_of_16() {
        let src = [0xf0, 0xff, 0x10, 0x04];
        let mut le = [0u16; 2];
        assert!(!unpack(Packing::U16Hi12(true), &src, &mut le));
        assert_eq!(le, [4095, 65]);
        let mut be = [0u16; 2];
        assert!(!unpack(
            Packing::U16Hi12(false),
            &[0xff, 0xf0, 0x04, 0x10],
            &mut be
        ));
        assert_eq!(be, [4095, 65]);
        assert_eq!(packed_row_bytes(Packing::U16Hi12(true), 3), 6);
    }

    #[test]
    fn unpack_msb12_and_le12() {
        let src = [0xab, 0xcd, 0xef, 0x12, 0x34, 0x56];
        let mut o = [0u16; 4];
        assert!(!unpack(Packing::Msb(12), &src, &mut o));
        assert_eq!(o, [0xabc, 0xdef, 0x123, 0x456]);
        let mut g = [0u16; 4];
        assert!(!unpack(Packing::Msb(12), &src, &mut g[..3]));
        assert_eq!(&g[..3], &[0xabc, 0xdef, 0x123]);
        assert!(!unpack(Packing::Le12, &src, &mut o));
        assert_eq!(o, [0xdab, 0xefc, 0x412, 0x563]);
        // Generic path (10 bits).
        let mut t = [0u16; 2];
        assert!(!unpack(Packing::Msb(10), &[0xff, 0xc0, 0x10], &mut t));
        assert_eq!(t, [0x3ff, 0x001]);
        assert!(unpack(Packing::Msb(10), &[0xff], &mut t));
    }

    #[test]
    fn nikon_tables_build() {
        for t in 0..6 {
            assert!(nikon_huff(t).is_ok(), "tree {t}");
        }
    }

    #[test]
    fn unslice_places_columns() {
        // Two slices of width 2 then one of width 1, height 2: w = 5.
        let buf: Vec<u16> = (0..10).collect();
        let mut out = vec![0u16; 10];
        unslice(&buf, &mut out, 5, 2, Some([2, 2, 1]));
        assert_eq!(out, vec![0, 1, 4, 5, 8, 2, 3, 6, 7, 9]);
    }

    /// CR2 slices `[4e9, 0, w]`: billions of empty slices are skipped, the last one placed.
    #[test]
    fn unslice_empty_slices_hostile() {
        let buf: Vec<u16> = (0..8).collect();
        let mut out = vec![0u16; 8];
        unslice(&buf, &mut out, 4, 2, Some([u32::MAX, 0, 4]));
        assert_eq!(out, buf);
    }
}
