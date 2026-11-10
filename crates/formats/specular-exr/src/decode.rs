//! Chunk location and parallel decode into interleaved RGBA (deep chunks flattened).

use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;

use crate::codec::{self, BlockGeom, ChanGeom, Layout, Scratch, num_samples};
use crate::deep;
use crate::half::Sample;
use crate::header::{Compression, ExrInfo, Level, Part, PixelType, Reader, Storage};

/// Where each output slot (R, G, B, A) takes its data from.
#[derive(Clone, Debug)]
pub(crate) struct Mapping {
    /// For each channel of the part, the output slots it is written to.
    pub slots_of: Vec<Vec<usize>>,
    /// Some slot has no channel and alpha must be set to one.
    pub alpha_one: bool,
}

/// One block of the selected level: rectangle relative to the level origin and its chunk.
#[derive(Clone, Copy, Debug)]
struct Block {
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
}

pub(crate) struct Plan<'a> {
    pub data: &'a [u8],
    pub part: &'a Part,
    pub level: Level,
    /// Absolute coordinates of the level's top-left pixel (the data window origin).
    pub ox: i32,
    pub oy: i32,
    pub width: usize,
    pub height: usize,
    /// Rows per band (lines per chunk or tile height).
    band_h: usize,
    /// Blocks per band (tiles across, or 1).
    per_band: usize,
    bw: usize,
    /// Chunk data ranges, `per_band * bands` entries in band-major order.
    chunks: Vec<Option<Range<usize>>>,
    /// The memory budget held for `chunks`.
    _res: specular_core::Reservation,
}

/// Largest expansion of a chunk: DWA's zlib-compressed DC values (about 1032:1) each become an
/// 8x8 block of halves (64:1), about 66000:1 in all; zlib, RLE, PIZ and B44 stay far below.
const MAX_CHUNK_RATIO: usize = 1 << 17;

/// The largest expansion of a chunk under `c`, with room to spare: what a tiny chunk may ask
/// the scratch buffers for (one codec's ratio for all let a 20 KB file size 2.5 GB of scratch
/// per worker).
fn chunk_ratio(c: Compression) -> usize {
    use Compression::*;
    match c {
        // Stored chunks are exact; a short one is damaged.
        None => 2,
        // A two-byte run gives 128 bytes.
        Rle => 128,
        // zlib (about 1032:1), and PXR24's 24-bit floats widened to 32 bits.
        Zips | Zip | Pxr24 => 2048,
        // B44A's 3-byte flat block gives 16 halves.
        B44 | B44a => 64,
        // PIZ's run-length Huffman codes: a few bits give a run of 255 values.
        Piz => 1 << 14,
        // Zstandard reaches about 32000:1 on runs.
        Zstd => 1 << 16,
        Dwaa | Dwab | Htj2k256 | Htj2k32 | Lj2k | Unknown(_) => MAX_CHUNK_RATIO,
    }
}

fn read_offset(data: &[u8], pos: usize) -> Option<u64> {
    Reader::new(data, pos).u64().ok()
}

/// Parse a chunk header at `pos`. Returns (part, coordinates, data range) where the coordinates
/// are `[y]` or `[tx, ty, lx, ly]` (padded with zeros). A deep chunk's range starts at its three
/// `u64` sizes and covers the sample-count table and the sample data.
fn chunk_at(
    data: &[u8],
    pos: usize,
    multipart: bool,
    tiled: bool,
    deep: bool,
) -> Option<(i32, [i32; 4], Range<usize>)> {
    let mut r = Reader::new(data, pos);
    let part = if multipart { r.i32().ok()? } else { 0 };
    let mut c = [0i32; 4];
    let n = if tiled { 4 } else { 1 };
    for v in c.iter_mut().take(n) {
        *v = r.i32().ok()?;
    }
    let start = r.pos;
    let size = if deep {
        let (table, packed) = (r.u64().ok()?, r.u64().ok()?);
        r.u64().ok()?;
        24u64.checked_add(table)?.checked_add(packed)?
    } else {
        u64::try_from(r.i32().ok()?).ok()?
    };
    let start = if deep { start } else { r.pos };
    let end = start.checked_add(usize::try_from(size).ok()?)?;
    // A chunk cut short by truncation is still returned; decompression decides.
    Some((part, c, start..end.min(data.len())))
}

impl<'a> Plan<'a> {
    pub fn new(
        data: &'a [u8],
        info: &'a ExrInfo,
        part_index: usize,
        level: Level,
    ) -> Result<Self, specular_core::NeedsMemory> {
        let part = &info.parts[part_index];
        let (width, height) = (level.width as usize, level.height as usize);
        let (band_h, bw, per_band) = match part.tile_desc() {
            Some(t) => (
                t.y_size as usize,
                t.x_size as usize,
                width.div_ceil(t.x_size as usize),
            ),
            None => (
                part.compression.lines_per_chunk().unwrap_or(1) as usize,
                width,
                1,
            ),
        };
        let bands = height.div_ceil(band_h);
        let n = bands * per_band;
        // Index of the level's first chunk in the offset table.
        let mut base = 0u64;
        for l in &part.levels {
            if l.lx == level.lx && l.ly == level.ly {
                break;
            }
            let (a, b) = part.tiles_in_level(l);
            base = base.saturating_add(a * b);
        }
        let tiled = part.tile_desc().is_some();
        let deep = part.is_deep();
        // 1x1 tiles make the table several times the size of the pixels.
        let _res =
            specular_core::budget::reserve_n(n as u64, size_of::<Option<Range<usize>>>() as u64)?;
        let mut plan = Plan {
            data,
            part,
            level,
            ox: part.data_window.x_min,
            oy: part.data_window.y_min,
            width,
            height,
            band_h,
            per_band,
            bw,
            chunks: vec![None; n],
            _res,
        };
        let mut missing = 0usize;
        for i in 0..n {
            let slot = base.saturating_add(i as u64);
            let found = (slot < part.chunk_count)
                .then(|| {
                    let at = usize::try_from(slot).ok()?.checked_mul(8)?;
                    part.offset_table.checked_add(at)
                })
                .flatten()
                .and_then(|p| read_offset(data, p))
                .and_then(|off| usize::try_from(off).ok())
                .and_then(|off| chunk_at(data, off, info.multipart, tiled, deep))
                .filter(|(p, c, _)| *p as usize == part_index && plan.slot_of(c) == Some(i));
            match found {
                Some((_, _, r)) => plan.chunks[i] = Some(r),
                None => missing += 1,
            }
        }
        if missing > 0 {
            plan.recover(info, part_index);
        }
        Ok(plan)
    }

    /// Index into `chunks` of a chunk with the given header coordinates.
    fn slot_of(&self, c: &[i32; 4]) -> Option<usize> {
        match self.part.tile_desc() {
            Some(_) => {
                let (tx, ty) = (usize::try_from(c[0]).ok()?, usize::try_from(c[1]).ok()?);
                if c[2] != self.level.lx as i32
                    || c[3] != self.level.ly as i32
                    || tx >= self.per_band
                {
                    return None;
                }
                let i = ty.checked_mul(self.per_band)?.checked_add(tx)?;
                (i < self.chunks.len()).then_some(i)
            }
            None => {
                let dy = c[0] as i64 - self.oy as i64;
                if dy < 0 || dy % self.band_h as i64 != 0 {
                    return None;
                }
                let i = usize::try_from(dy / self.band_h as i64).ok()?;
                (i < self.chunks.len()).then_some(i)
            }
        }
    }

    /// Rebuild missing offsets by walking the chunks that follow the offset tables (as OpenEXR
    /// does for files whose table was never written).
    fn recover(&mut self, info: &ExrInfo, part_index: usize) {
        let Some(last) = info.parts.last() else {
            return;
        };
        let mut pos = last
            .offset_table
            .saturating_add(last.chunk_count.saturating_mul(8).min(usize::MAX as u64) as usize);
        let data = self.data;
        while pos < data.len() {
            let mut r = Reader::new(data, pos);
            let p = if info.multipart {
                match r.i32() {
                    Ok(p) if p >= 0 && (p as usize) < info.parts.len() => p as usize,
                    _ => return,
                }
            } else {
                0
            };
            let part = &info.parts[p];
            let tiled = matches!(part.storage, Storage::Tiled(_) | Storage::DeepTiled(_));
            let Some((_, c, range)) = chunk_at(data, pos, info.multipart, tiled, part.is_deep())
            else {
                return;
            };
            if p == part_index
                && let Some(i) = self.slot_of(&c)
                && self.chunks[i].is_none()
            {
                self.chunks[i] = Some(range.clone());
            }
            let next = range.end;
            if next <= pos {
                return;
            }
            pos = next;
        }
    }

    fn block(&self, band: usize, k: usize) -> Block {
        let y0 = band * self.band_h;
        let x0 = k * self.bw;
        Block {
            x0,
            x1: (x0 + self.bw).min(self.width),
            y0,
            y1: (y0 + self.band_h).min(self.height),
        }
    }

    fn geom(&self, b: &Block, g: &mut BlockGeom) {
        let ax0 = self.ox + b.x0 as i32;
        let ax1 = self.ox + b.x1 as i32 - 1;
        let ay0 = self.oy + b.y0 as i32;
        let h = b.y1 - b.y0;
        g.y0 = ay0;
        g.h = h;
        g.chans.clear();
        g.total = 0;
        for c in &self.part.channels {
            let nx = num_samples(c.x_sampling, ax0, ax1);
            let ny = num_samples(c.y_sampling, ay0, ay0 + h as i32 - 1);
            let bytes = c.pixel_type.bytes();
            g.total += nx * ny * bytes;
            g.chans.push(ChanGeom {
                nx,
                ny,
                bytes,
                ty: c.pixel_type,
                p_linear: c.p_linear,
                ys: c.y_sampling,
            });
        }
    }

    /// Decode into `out` (`width * height * 4`). Returns the number of damaged/missing chunks.
    pub fn decode<T: Sample>(&self, map: &Mapping, out: &mut [T], parallel: bool) -> usize {
        let damaged = AtomicUsize::new(0);
        let row_len = self.width * 4;
        let band_len = row_len * self.band_h;
        let run = |st: &mut (Scratch, BlockGeom), (band, rows): (usize, &mut [T])| {
            if map.alpha_one {
                for px in rows.chunks_exact_mut(4) {
                    px[3] = T::ONE;
                }
            }
            for k in 0..self.per_band {
                let b = self.block(band, k);
                if !self.decode_block(&b, band * self.per_band + k, map, rows, st) {
                    damaged.fetch_add(1, Ordering::Relaxed);
                }
            }
        };
        if parallel {
            out.par_chunks_mut(band_len)
                .enumerate()
                .for_each_init(|| (Scratch::default(), BlockGeom::default()), run);
        } else {
            let mut st = (Scratch::default(), BlockGeom::default());
            out.chunks_mut(band_len)
                .enumerate()
                .for_each(|x| run(&mut st, x));
        }
        self.fill_subsampled_rows(map, out);
        damaged.into_inner()
    }

    /// Decode one block into the band's rows. Returns false if the chunk is missing or corrupt.
    fn decode_block<T: Sample>(
        &self,
        b: &Block,
        idx: usize,
        map: &Mapping,
        rows: &mut [T],
        (s, g): &mut (Scratch, BlockGeom),
    ) -> bool {
        let Some(range) = self.chunks[idx].clone() else {
            return false;
        };
        let raw = &self.data[range];
        self.geom(b, g);
        if g.total > raw.len().saturating_mul(chunk_ratio(self.part.compression)) {
            // More than any codec can expand to (thousands of channels over a tiny chunk):
            // refuse before the scratch buffers are sized from the header.
            return false;
        }
        let comp = self.part.compression;
        if self.part.is_deep() {
            let (w, h) = (b.x1 - b.x0, b.y1 - b.y0);
            if deep::flatten(
                raw,
                comp,
                &self.part.channels,
                w,
                h,
                &mut s.deep,
                &mut s.out,
            )
            .is_none()
                || s.out.len() != g.total
            {
                return false;
            }
            self.convert(b, g, &s.out, Layout::Lines, map, rows);
            return true;
        }
        let (bytes, layout): (&[u8], Layout) = if comp == Compression::None || raw.len() >= g.total
        {
            match raw.get(..g.total) {
                Some(r) => (r, Layout::Lines),
                None => return false,
            }
        } else {
            match codec::decompress(comp, raw, g, &self.part.channels, s) {
                Some(l) => (&s.out[..], l),
                None => return false,
            }
        };
        self.convert(b, g, bytes, layout, map, rows);
        true
    }

    fn convert<T: Sample>(
        &self,
        b: &Block,
        g: &BlockGeom,
        bytes: &[u8],
        layout: Layout,
        map: &Mapping,
        rows: &mut [T],
    ) {
        let row_len = self.width * 4;
        let ax0 = self.ox + b.x0 as i32;
        // Planar: start of each channel's plane, and sampled-line counters.
        let mut plane = [0usize; 64];
        let nch = g.chans.len();
        let mut planes_vec;
        let plane_start: &mut [usize] = if nch <= 64 {
            &mut plane[..nch]
        } else {
            planes_vec = vec![0usize; nch];
            &mut planes_vec
        };
        if layout == Layout::Planar {
            let mut acc = 0;
            for (c, ch) in g.chans.iter().enumerate() {
                plane_start[c] = acc;
                acc += ch.nx * ch.ny * ch.bytes;
            }
        }
        let mut off = 0usize;
        for (li, y) in (g.y0..g.y0 + g.h as i32).enumerate() {
            let row = &mut rows[li * row_len..][..row_len];
            for (c, ch) in g.chans.iter().enumerate() {
                if !g.sampled(c, y) {
                    continue;
                }
                let n = ch.nx * ch.bytes;
                let src = match layout {
                    Layout::Lines => {
                        let s = &bytes[off..off + n];
                        off += n;
                        s
                    }
                    Layout::Planar => {
                        let s = &bytes[plane_start[c]..plane_start[c] + n];
                        plane_start[c] += n;
                        s
                    }
                };
                let slots = &map.slots_of[c];
                if slots.is_empty() {
                    continue;
                }
                let xs = self.part.channels[c].x_sampling;
                let dst = &mut row[b.x0 * 4..b.x1 * 4];
                for &slot in slots {
                    write_samples(dst, slot, src, ch.ty, xs, ax0);
                }
            }
        }
    }

    /// Rows per band (lines per chunk, or the tile height).
    pub fn band_rows(&self) -> usize {
        self.band_h
    }

    /// Decode the bands covering level rows `rows`, only the blocks meeting columns `cols`,
    /// into `buf` (full level width, interleaved RGBA; other columns stay zero). Returns the
    /// first level row held in `buf` and the number of damaged or missing chunks.
    pub fn decode_window<T: Sample>(
        &self,
        map: &Mapping,
        rows: Range<usize>,
        cols: Range<usize>,
        buf: &mut Vec<T>,
        st: &mut (Scratch, BlockGeom),
    ) -> (usize, usize) {
        let bands = self.height.div_ceil(self.band_h);
        let b0 = (rows.start / self.band_h).min(bands);
        let b1 = rows.end.div_ceil(self.band_h).clamp(b0, bands);
        let first = b0 * self.band_h;
        let last = (b1 * self.band_h).min(self.height).max(first);
        let row_len = self.width * 4;
        buf.clear();
        buf.resize((last - first) * row_len, T::ZERO);
        if map.alpha_one {
            for px in buf.chunks_exact_mut(4) {
                px[3] = T::ONE;
            }
        }
        let k0 = (cols.start / self.bw).min(self.per_band);
        let k1 = cols.end.div_ceil(self.bw).clamp(k0, self.per_band);
        let mut damaged = 0;
        for band in b0..b1 {
            let y0 = band * self.band_h - first;
            let y1 = ((band + 1) * self.band_h).min(self.height) - first;
            let rows = &mut buf[y0 * row_len..y1 * row_len];
            for k in k0..k1 {
                let b = self.block(band, k);
                if !self.decode_block(&b, band * self.per_band + k, map, rows, st) {
                    damaged += 1;
                }
            }
        }
        (first, damaged)
    }

    /// [`Plan::fill_subsampled_rows`] on a window of the level starting at row `first`: rows
    /// whose source row lies outside the window are left as they are.
    pub fn fill_subsampled_window<T: Sample>(&self, map: &Mapping, buf: &mut [T], first: usize) {
        let row_len = self.width * 4;
        let n = buf.len() / row_len.max(1);
        for (c, ch) in self.part.channels.iter().enumerate() {
            let ys = ch.y_sampling;
            if ys == 1 || map.slots_of[c].is_empty() {
                continue;
            }
            for y in first..first + n {
                let abs = self.oy + y as i32;
                let rem = abs.rem_euclid(ys) as usize;
                if rem == 0 {
                    continue;
                }
                let Some((a, b, t)) = between(y, rem, ys as usize, first, first + n) else {
                    continue;
                };
                let (d, sa, sb) = (
                    (y - first) * row_len,
                    (a - first) * row_len,
                    (b - first) * row_len,
                );
                for &slot in &map.slots_of[c] {
                    for x in 0..self.width {
                        let (va, vb) = (
                            buf[sa + x * 4 + slot].to_f32(),
                            buf[sb + x * 4 + slot].to_f32(),
                        );
                        buf[d + x * 4 + slot] = T::from_f32(va * (1.0 - t) + vb * t);
                    }
                }
            }
        }
    }

    /// Fill rows that a vertically subsampled channel has no samples for, from the nearest
    /// sampled row above (or below at the top edge).
    fn fill_subsampled_rows<T: Sample>(&self, map: &Mapping, out: &mut [T]) {
        let row_len = self.width * 4;
        for (c, ch) in self.part.channels.iter().enumerate() {
            let ys = ch.y_sampling;
            if ys == 1 || map.slots_of[c].is_empty() {
                continue;
            }
            for y in 0..self.height {
                let abs = self.oy + y as i32;
                let rem = abs.rem_euclid(ys) as usize;
                if rem == 0 {
                    continue;
                }
                let Some((a, b, t)) = between(y, rem, ys as usize, 0, self.height) else {
                    continue;
                };
                for &slot in &map.slots_of[c] {
                    for x in 0..self.width {
                        let (va, vb) = (
                            out[a * row_len + x * 4 + slot].to_f32(),
                            out[b * row_len + x * 4 + slot].to_f32(),
                        );
                        out[y * row_len + x * 4 + slot] = T::from_f32(va * (1.0 - t) + vb * t);
                    }
                }
            }
        }
    }
}

/// For row `y`, `rem` rows past a sampled row of a channel subsampled every `ys` rows: the
/// sampled rows above and below within `lo..hi` and the weight of the lower one (the one row
/// there is at the edges, weight 0).
fn between(y: usize, rem: usize, ys: usize, lo: usize, hi: usize) -> Option<(usize, usize, f32)> {
    let above = y.checked_sub(rem).filter(|&a| a >= lo && a < hi);
    let below = Some(y + (ys - rem)).filter(|&b| b >= lo && b < hi);
    match (above, below) {
        (Some(a), Some(b)) => Some((a, b, rem as f32 / ys as f32)),
        (Some(a), None) => Some((a, a, 0.0)),
        (None, Some(b)) => Some((b, b, 0.0)),
        (None, None) => None,
    }
}

/// Write one line of one channel into slot `slot` of the interleaved RGBA `dst`.
#[inline]
fn write_samples<T: Sample>(
    dst: &mut [T],
    slot: usize,
    src: &[u8],
    ty: PixelType,
    xs: i32,
    ax0: i32,
) {
    if xs == 1 {
        match ty {
            PixelType::Half => {
                for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(2)) {
                    d[slot] = T::from_half(u16::from_le_bytes([s[0], s[1]]));
                }
            }
            PixelType::Float => {
                for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    d[slot] = T::from_f32(f32::from_le_bytes([s[0], s[1], s[2], s[3]]));
                }
            }
            PixelType::Uint => {
                for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    d[slot] = T::from_f32(u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32);
                }
            }
        }
        return;
    }
    // Horizontal subsampling: samples sit on the pixels at multiples of `xs`; the pixels
    // between take the line between the two either side (the edge sample past the ends).
    // Nearest-neighbour put luminance-chroma files 50 % further from their RGB originals
    // than OpenEXR's own reconstruction.
    let bytes = ty.bytes();
    let n = src.len() / bytes;
    if n == 0 {
        return;
    }
    let at = |k: i64| -> f32 {
        let s = &src[k.clamp(0, n as i64 - 1) as usize * bytes..][..bytes];
        match ty {
            PixelType::Half => crate::half::h2f(u16::from_le_bytes([s[0], s[1]])),
            PixelType::Float => f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
            PixelType::Uint => u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32,
        }
    };
    let xs = i64::from(xs);
    let first = (ax0 as i64).div_euclid(xs) + i64::from((ax0 as i64).rem_euclid(xs) != 0);
    for (i, d) in dst.chunks_exact_mut(4).enumerate() {
        let abs = ax0 as i64 + i as i64;
        let (k, r) = (abs.div_euclid(xs) - first, abs.rem_euclid(xs));
        d[slot] = if r == 0 {
            T::from_f32(at(k))
        } else {
            let t = r as f32 / xs as f32;
            T::from_f32(at(k) * (1.0 - t) + at(k + 1) * t)
        };
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// Subsampled chroma between its samples: linear (it was the sample to the left, which
    /// put luminance-chroma files 50 % further from their RGB originals than OpenEXR's own
    /// reconstruction).
    #[test]
    fn subsampled_samples_interpolate() {
        let src: Vec<u8> = [0.0f32, 1.0, 3.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let mut dst = vec![0f32; 6 * 4];
        write_samples(&mut dst, 1, &src, PixelType::Float, 2, 0);
        let got: Vec<f32> = dst.chunks_exact(4).map(|p| p[1]).collect();
        assert_eq!(got, [0.0, 0.5, 1.0, 2.0, 3.0, 3.0]);
        // Starting on an odd pixel: the first sample sits on pixel 2 (absolute), one in.
        let mut dst = vec![0f32; 3 * 4];
        write_samples(&mut dst, 0, &src, PixelType::Float, 2, 1);
        let got: Vec<f32> = dst.chunks_exact(4).map(|p| p[0]).collect();
        assert_eq!(got, [0.0, 0.0, 0.5]);
        // Rows: between the sampled rows, the edge row alone past the last.
        assert_eq!(between(3, 1, 2, 0, 8), Some((2, 4, 0.5)));
        assert_eq!(between(7, 1, 2, 0, 8), Some((6, 6, 0.0)));
        assert_eq!(between(1, 1, 2, 2, 8), Some((2, 2, 0.0)));
    }

    /// What a 20 KB chunk may size its scratch buffers to: one ratio for every codec let it
    /// ask 2.5 GB (per worker) of any of them; only DWA and HTJ2K keep that ratio now.
    #[test]
    fn scratch_per_tiny_chunk() {
        use Compression::*;
        let chunk = 20 << 10;
        for c in [None, Rle, Zips, Zip, Pxr24, B44, B44a, Piz] {
            assert!(chunk * chunk_ratio(c) <= 512 << 20, "{c:?}");
        }
        assert_eq!(chunk_ratio(Dwaa), MAX_CHUNK_RATIO);
    }
}
