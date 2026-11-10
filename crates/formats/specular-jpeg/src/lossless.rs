//! Huffman lossless frames (SOF3, ITU-T T.81 Annex H): every predictor, point transform,
//! restart intervals, 2..=16-bit samples, any sampling factors and scan layout.
//!
//! Samples are decoded into per-component planes of raw (pre point-transform) `u16` values,
//! padded to whole MCUs. Two consumers share them: [`crate::decode_wide`] (stored values) and
//! the display path (8-bit RGBA, box-downscaled and cropped, the output `jpeg-decoder` gave
//! when it was the fallback for these files; it is now only the test oracle).
//!
//! Speed: the common shape (one sample per component per MCU: greyscale, or interleaved
//! 1x1 components, or any non-interleaved scan) runs a kernel specialised per predictor and
//! component count. A restart interval that is a whole number of rows starts each interval
//! like the first line of the image (H.1.2.1), so the intervals are independent: large images
//! decode them in parallel, every interval from its own byte range between RSTn markers.
//! Small images stay on the calling thread. Other shapes (subsampled interleaved scans,
//! restart intervals that end mid-row) take the general per-sample path.

use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::error::{JpegError, Result};
use crate::fallback::{out_rect, to_8bit};
use crate::huffman::{BitReader, HuffTable, LUT_BITS, extend};
use crate::marker::{Parsed, Scan, has_ff};
use crate::{DecodedPlanes, Plane, PlanesLayout, Region, Scale};

/// Largest frame decoded (samples over all components, before allocation).
const MAX_SAMPLES: u64 = 1 << 30;
/// Below this many samples in a scan the restart intervals are decoded serially even when
/// threads are offered (fan-out costs more than it saves).
/// (Unit tests fan out on any size, so their small images take the parallel paths too.)
const MIN_PARALLEL_SAMPLES: usize = if cfg!(test) { 1 } else { 1 << 19 };
/// Smallest unit of parallel work, in samples.
const MIN_TASK_SAMPLES: usize = if cfg!(test) { 1 } else { 1 << 16 };

/// One component's decoded samples.
pub(crate) struct Comp {
    /// Plane row stride (samples): MCUs per row times the horizontal sampling factor.
    pub stride: usize,
    /// Extent actually covering the component.
    pub cw: usize,
    pub ch: usize,
    /// Raw samples (before the point transform), `stride` x padded rows.
    pub data: Vec<u16>,
    /// Point transform of the scan that coded the component.
    pub pt: u8,
    pub h: usize,
    pub v: usize,
}

pub(crate) struct Samples {
    pub comps: Vec<Comp>,
    /// The entropy-coded data ended early or broke off.
    pub truncated: bool,
}

fn alloc_u16(n: usize) -> Result<Vec<u16>> {
    let mut probe: Vec<u16> = Vec::new();
    probe
        .try_reserve_exact(n)
        .map_err(|_| JpegError::TooLarge(n as u64 * 2))?;
    drop(probe);
    Ok(vec![0u16; n])
}

fn cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|c| c.load(Ordering::Relaxed))
}

fn reader<'a>(data: &'a [u8], start: usize, end: usize) -> BitReader<'a> {
    let end = end.min(data.len());
    BitReader {
        data: &data[..end],
        pos: start.min(end),
        buf: 0,
        bits: 0,
        marker_hit: false,
        zero_fill: 0,
    }
}

/// Decodes every scan of a lossless frame down to the MCU row holding image row `rows - 1`,
/// using up to `threads` threads of the current rayon pool.
pub(crate) fn decode_samples(
    p: &Parsed,
    data: &[u8],
    rows: usize,
    threads: usize,
    cancel: Option<&AtomicBool>,
) -> Result<Samples> {
    let f = &p.frame;
    let (w, h) = (f.width as usize, f.height as usize);
    let (hmax, vmax) = (f.max_h as usize, f.max_v as usize);
    let (mx, my) = (w.div_ceil(hmax), h.div_ceil(vmax));
    let samples: u64 = f
        .comps
        .iter()
        .map(|c| (mx * c.h as usize) as u64 * (my * c.v as usize) as u64)
        .sum();
    if samples > MAX_SAMPLES {
        return Err(JpegError::TooLarge(samples * 2));
    }
    // Every Huffman-coded sample costs at least one bit: refuse absurd allocations for files
    // that cannot hold anything near the declared size.
    let avail_bits = (data.len().saturating_sub(p.info.header_len) as u64) * 8;
    if samples > (64 << 20) && samples > avail_bits.saturating_mul(64) {
        return Err(JpegError::Truncated(
            "file far too short for the declared image size",
        ));
    }
    let mut comps = Vec::with_capacity(f.comps.len());
    for c in &f.comps {
        let (ch_, cv) = (c.h as usize, c.v as usize);
        comps.push(Comp {
            stride: mx * ch_,
            cw: (w * ch_).div_ceil(hmax),
            ch: (h * cv).div_ceil(vmax),
            data: alloc_u16(mx * ch_ * my * cv)?,
            pt: 0,
            h: ch_,
            v: cv,
        });
    }
    let mut truncated = false;
    for s in &p.scans {
        truncated |= scan(p, data, s, &mut comps, rows, threads, cancel)?;
    }
    Ok(Samples { comps, truncated })
}

/// Decodes one scan. Returns whether its data ran out.
fn scan(
    p: &Parsed,
    data: &[u8],
    s: &Scan,
    comps: &mut [Comp],
    rows: usize,
    threads: usize,
    cancel: Option<&AtomicBool>,
) -> Result<bool> {
    let f = &p.frame;
    let pred = s.ss;
    if !(1..=7).contains(&pred) {
        return Err(JpegError::Malformed("lossless predictor"));
    }
    if s.al >= f.precision {
        return Err(JpegError::Malformed("lossless point transform"));
    }
    let tables: Vec<&HuffTable> = s
        .comps
        .iter()
        .map(|c| {
            s.dc[c.td]
                .as_deref()
                .ok_or(JpegError::Malformed("missing lossless Huffman table"))
        })
        .collect::<Result<_>>()?;
    let interleaved = s.comps.len() > 1;
    let (hmax, vmax) = (f.max_h as usize, f.max_v as usize);
    // Units per row, unit rows, and the unit rows the output needs.
    let (ux, total, need) = if interleaved {
        let (mx, my) = (
            (f.width as usize).div_ceil(hmax),
            (f.height as usize).div_ceil(vmax),
        );
        (mx, my, rows.div_ceil(vmax).min(my))
    } else {
        let c = &comps[s.comps[0].ci];
        (c.cw, c.ch, (rows * c.v).div_ceil(vmax).min(c.ch))
    };
    for sc in &s.comps {
        comps[sc.ci].pt = s.al;
    }
    let init = 1i32 << (f.precision - s.al - 1);
    let ri = s.restart_interval as usize;
    let one_sample = !interleaved
        || s.comps
            .iter()
            .all(|sc| comps[sc.ci].h == 1 && comps[sc.ci].v == 1);
    if one_sample && s.comps.len() <= 4 && (ri == 0 || ri.is_multiple_of(ux)) {
        // Each scan component once, in scan order (a component listed twice is corrupt).
        let mut slots: Vec<Option<&mut Comp>> = comps.iter_mut().map(Some).collect();
        let mut planes: Vec<&mut [u16]> = Vec::with_capacity(s.comps.len());
        let mut stride = 0;
        for sc in &s.comps {
            let c = slots[sc.ci]
                .take()
                .ok_or(JpegError::Malformed("component repeated in a scan"))?;
            stride = c.stride;
            planes.push(&mut c.data);
        }
        let seg_rows = if ri == 0 { total.max(1) } else { ri / ux };
        let job = Job {
            data,
            s,
            tables: &tables,
            ux,
            stride,
            seg_rows,
            pred,
            init,
            cancel,
        };
        return job.run(planes, need, threads);
    }
    general(data, s, &tables, comps, ux, need, interleaved, init, cancel)
}

/// A scan of the common shape: one sample per component per unit, restart intervals of whole
/// rows (`seg_rows` unit rows each; the whole scan without restarts).
struct Job<'a> {
    data: &'a [u8],
    s: &'a Scan,
    tables: &'a [&'a HuffTable],
    ux: usize,
    stride: usize,
    seg_rows: usize,
    pred: u8,
    init: i32,
    cancel: Option<&'a AtomicBool>,
}

impl Job<'_> {
    fn run(&self, mut planes: Vec<&mut [u16]>, need: usize, threads: usize) -> Result<bool> {
        if need == 0 {
            return Ok(false);
        }
        let nk = planes.len();
        let nseg = need.div_ceil(self.seg_rows);
        let parallel = threads > 1 && nseg > 1 && need * self.ux * nk >= MIN_PARALLEL_SAMPLES;
        if parallel && let Some(ranges) = segment_ranges(self.data, self.s, nseg) {
            let seg_samples = self.seg_rows * self.ux * nk;
            let per_task = MIN_TASK_SAMPLES
                .div_ceil(seg_samples)
                .max(nseg.div_ceil(threads * 8))
                .max(1);
            let task_rows = per_task * self.seg_rows;
            // Per task: its rows of every plane.
            let mut tasks: Vec<Vec<&mut [u16]>> = (0..nseg.div_ceil(per_task))
                .map(|_| Vec::with_capacity(nk))
                .collect();
            for plane in planes.iter_mut() {
                let rows = &mut plane[..need * self.stride];
                for (t, chunk) in rows.chunks_mut(task_rows * self.stride).enumerate() {
                    tasks[t].push(chunk);
                }
            }
            let truncated = tasks
                .into_par_iter()
                .enumerate()
                .map(|(t, mut task)| -> Result<bool> {
                    let mut truncated = false;
                    let seg0 = t * per_task;
                    let mine = ranges.iter().enumerate().take(seg0 + per_task).skip(seg0);
                    for (seg, &(a, b)) in mine {
                        let mut br = reader(self.data, a, b);
                        let r0 = (seg - seg0) * self.seg_rows;
                        let n = self.seg_rows.min(need - seg * self.seg_rows);
                        self.segment(&mut br, &mut task, r0, n)?;
                        truncated |= br.overrun();
                    }
                    Ok(truncated)
                })
                .try_reduce(|| false, |a, b| Ok(a | b))?;
            return Ok(truncated);
        }
        let mut br = reader(self.data, self.s.data_start, self.s.data_end);
        let mut truncated = false;
        for seg in 0..nseg {
            if seg > 0 {
                truncated |= br.overrun();
                br.restart();
            }
            let r0 = seg * self.seg_rows;
            let n = self.seg_rows.min(need - r0);
            self.segment(&mut br, &mut planes, r0, n)?;
        }
        Ok(truncated | br.overrun())
    }

    /// Decodes rows `r0..r0 + n` of `planes` (local row numbers), the first of them predicted
    /// like the first line of the image.
    fn segment(
        &self,
        br: &mut BitReader,
        planes: &mut [&mut [u16]],
        r0: usize,
        n: usize,
    ) -> Result<()> {
        macro_rules! go {
            ($n:literal) => {{
                let tabs: [&HuffTable; $n] = std::array::from_fn(|k| self.tables[k]);
                let pl: &mut [&mut [u16]; $n] = planes.try_into().unwrap();
                match self.pred {
                    1 => self.rows::<$n, 1>(br, &tabs, pl, r0, n),
                    2 => self.rows::<$n, 2>(br, &tabs, pl, r0, n),
                    3 => self.rows::<$n, 3>(br, &tabs, pl, r0, n),
                    4 => self.rows::<$n, 4>(br, &tabs, pl, r0, n),
                    5 => self.rows::<$n, 5>(br, &tabs, pl, r0, n),
                    6 => self.rows::<$n, 6>(br, &tabs, pl, r0, n),
                    _ => self.rows::<$n, 7>(br, &tabs, pl, r0, n),
                }
            }};
        }
        match planes.len() {
            1 => go!(1),
            2 => go!(2),
            3 => go!(3),
            _ => go!(4),
        }
    }

    #[inline(always)]
    fn rows<const N: usize, const P: u8>(
        &self,
        br: &mut BitReader,
        tabs: &[&HuffTable; N],
        planes: &mut [&mut [u16]; N],
        r0: usize,
        n: usize,
    ) -> Result<()> {
        let (ux, stride) = (self.ux, self.stride);
        for r in r0..r0 + n {
            if cancelled(self.cancel) {
                return Err(JpegError::Cancelled);
            }
            let base = r * stride;
            if r == r0 {
                let mut left = [self.init; N];
                for x in 0..ux {
                    for k in 0..N {
                        let v = left[k].wrapping_add(diff(br, tabs[k])) as u16;
                        planes[k][base + x] = v;
                        left[k] = v as i32;
                    }
                }
                continue;
            }
            let mut rows = planes.each_mut().map(|p| {
                let (above, here) = p.split_at_mut(base);
                (&above[base - stride..][..ux], &mut here[..ux])
            });
            let mut left = [0i32; N];
            for k in 0..N {
                let v = (rows[k].0[0] as i32).wrapping_add(diff(br, tabs[k])) as u16;
                rows[k].1[0] = v;
                left[k] = v as i32;
            }
            for x in 1..ux {
                for k in 0..N {
                    let (above, here) = &mut rows[k];
                    let (rb, rc) = (above[x] as i32, above[x - 1] as i32);
                    let px = predict::<P>(left[k], rb, rc);
                    let v = px.wrapping_add(diff(br, tabs[k])) as u16;
                    here[x] = v;
                    left[k] = v as i32;
                }
            }
        }
        Ok(())
    }
}

/// Table H.1.
#[inline(always)]
fn predict<const P: u8>(ra: i32, rb: i32, rc: i32) -> i32 {
    match P {
        1 => ra,
        2 => rb,
        3 => rc,
        4 => ra + rb - rc,
        5 => ra + ((rb - rc) >> 1),
        6 => rb + ((ra - rc) >> 1),
        _ => (ra + rb) >> 1,
    }
}

/// One coded difference (F.1.2.1 with H.1.2.2: category 16 is 32768 without extra bits).
#[inline(always)]
fn diff(br: &mut BitReader, t: &HuffTable) -> i32 {
    br.ensure(32);
    let e = t.lut[br.peek(LUT_BITS) as usize];
    if e >> 16 != 0 {
        br.consume(e & 31);
        return (e >> 16) as u16 as i16 as i32;
    }
    let (sym, len) = if e != 0 {
        ((e >> 8) & 0xFF, e & 31)
    } else {
        t.decode_slow(br.peek(16))
    };
    br.consume(len);
    match sym {
        0 => 0,
        1..=15 => extend(br.get(sym), sym),
        _ => 32768,
    }
}

/// Byte ranges of the first `nseg` restart intervals of `s`, split at RST0..RST7 in sequence.
/// `None` when the markers are not all there in order (the serial path resyncs instead).
fn segment_ranges(data: &[u8], s: &Scan, nseg: usize) -> Option<Vec<(usize, usize)>> {
    let end = s.data_end.min(data.len());
    let d = &data[..end];
    let mut out = Vec::with_capacity(nseg);
    let (mut start, mut i, mut next) = (s.data_start.min(end), s.data_start.min(end), 0u8);
    while out.len() + 1 < nseg {
        // Skip eight bytes at a time while none is 0xFF.
        while let Some(w) = d.get(i..i + 8) {
            if has_ff(u64::from_be_bytes(w.try_into().unwrap())) {
                break;
            }
            i += 8;
        }
        if i + 1 >= end {
            return None;
        }
        if d[i] != 0xFF {
            i += 1;
            continue;
        }
        match d[i + 1] {
            0x00 => i += 2,
            0xFF => i += 1,
            m if m == 0xD0 + next => {
                out.push((start, i));
                start = i + 2;
                i = start;
                next = (next + 1) & 7;
            }
            _ => return None,
        }
    }
    out.push((start, end));
    Some(out)
}

/// Any other scan shape, one sample at a time: subsampled interleaved MCUs, restart intervals
/// that end mid-row (the first line of an interval is the rest of the row it starts in).
#[allow(clippy::too_many_arguments)]
fn general(
    data: &[u8],
    s: &Scan,
    tables: &[&HuffTable],
    comps: &mut [Comp],
    ux: usize,
    need: usize,
    interleaved: bool,
    init: i32,
    cancel: Option<&AtomicBool>,
) -> Result<bool> {
    let pred = s.ss;
    let mut br = reader(data, s.data_start, s.data_end);
    let ri = s.restart_interval as usize;
    // Per scan component: (component index, h, v).
    let units: Vec<(usize, usize, usize)> = s
        .comps
        .iter()
        .map(|c| {
            let fc = &comps[c.ci];
            if interleaved {
                (c.ci, fc.h, fc.v)
            } else {
                (c.ci, 1, 1)
            }
        })
        .collect();
    let mut mcu = 0usize;
    // Row (in plane coordinates, per component) where the current restart interval began:
    // its first line is predicted like the first line of the image.
    let mut first_row: Vec<usize> = vec![0; units.len()];
    let mut restart_pending = false;
    let mut truncated = false;
    for my in 0..need {
        if cancelled(cancel) {
            return Err(JpegError::Cancelled);
        }
        for mx in 0..ux {
            if ri > 0 && mcu > 0 && mcu.is_multiple_of(ri) {
                truncated |= br.overrun();
                br.restart();
                restart_pending = true;
            }
            mcu += 1;
            for (k, &(ci, h, v)) in units.iter().enumerate() {
                let c = &mut comps[ci];
                let pw = c.stride;
                let plane = &mut c.data;
                let t = tables[k];
                for by in 0..v {
                    for bx in 0..h {
                        let (x, y) = (mx * h + bx, my * v + by);
                        if restart_pending && bx == 0 && by == 0 {
                            first_row[k] = y;
                        }
                        let i = y * pw + x;
                        let px = if y == first_row[k] {
                            if x == 0 || (restart_pending && bx == 0 && by == 0) {
                                init
                            } else {
                                plane[i - 1] as i32
                            }
                        } else if x == 0 {
                            plane[i - pw] as i32
                        } else {
                            let ra = plane[i - 1] as i32;
                            let rb = plane[i - pw] as i32;
                            let rc = plane[i - pw - 1] as i32;
                            match pred {
                                1 => ra,
                                2 => rb,
                                3 => rc,
                                4 => ra + rb - rc,
                                5 => ra + ((rb - rc) >> 1),
                                6 => rb + ((ra - rc) >> 1),
                                _ => (ra + rb) >> 1,
                            }
                        };
                        plane[i] = px.wrapping_add(diff(&mut br, t)) as u16;
                    }
                }
            }
            restart_pending = false;
        }
    }
    Ok(truncated | br.overrun())
}

// ---------------------------------------------------------------------------------- output

/// One component as the display path reads it.
struct View<'a> {
    c: &'a Comp,
    /// Plane column of every image column (subsampled components only).
    cols: Option<Vec<u32>>,
    /// 8-bit value of every raw sample (when the image is large enough to amortise it).
    lut: Option<Box<[u8; 1 << 16]>>,
    precision: u32,
}

impl View<'_> {
    #[inline(always)]
    fn conv(&self, raw: u16) -> u8 {
        match &self.lut {
            Some(l) => l[raw as usize],
            None => self.slow(raw),
        }
    }

    #[inline(always)]
    fn slow(&self, raw: u16) -> u8 {
        let v = ((raw as u32) << self.c.pt) as u16;
        if self.precision == 8 {
            v as u8
        } else {
            to_8bit(v, self.precision)
        }
    }

    #[inline(always)]
    fn at(&self, row: &[u16], x: usize) -> u8 {
        let col = match &self.cols {
            Some(c) => c[x] as usize,
            None => x,
        };
        self.conv(row[col])
    }
}

/// Full-size samples to 8-bit RGBA (grey, RGB without colour transform, or inverted CMYK as
/// `jpeg-decoder` emits it), box-downscaled to `scale` and cropped to `region`.
pub(crate) fn display(
    p: &Parsed,
    s: &Samples,
    scale: Scale,
    region: Option<Region>,
    threads: usize,
) -> Result<DecodedPlanes> {
    let f = &p.frame;
    let (w, h) = (f.width as usize, f.height as usize);
    let (hmax, vmax) = (f.max_h as usize, f.max_v as usize);
    let nc = s.comps.len();
    if !matches!(nc, 1 | 3 | 4) {
        return Err(JpegError::Unsupported("lossless component count"));
    }
    let (x0, y0, x1, y1) = out_rect(w, h, scale, region)?;
    let sc = scale as usize;
    let (rw, rh) = (x1 - x0, y1 - y0);
    let (xs0, xs1) = (x0 * sc, (x1 * sc).min(w));
    let needed = (xs1 - xs0) * (((y1 * sc).min(h)) - y0 * sc) * nc;
    let precision = f.precision as u32;
    let views: Vec<View> = s
        .comps
        .iter()
        .map(|c| {
            let cols = (c.h != hmax).then(|| {
                (0..w)
                    .map(|x| (x * c.h / hmax) as u32)
                    .collect::<Vec<u32>>()
            });
            let mut v = View {
                c,
                cols,
                lut: None,
                precision,
            };
            if needed >= 1 << 16 && (precision != 8 || c.pt != 0) {
                let mut l = Box::new([0u8; 1 << 16]);
                for (r, o) in l.iter_mut().enumerate() {
                    *o = v.slow(r as u16);
                }
                v.lut = Some(l);
            }
            v
        })
        .collect();
    let mut out = crate::geom::alloc_zeroed(rw * rh * 4)?;
    // Source row `y` (image coordinates), columns xs0..xs1, as RGBA into `buf`.
    let fill = |y: usize, buf: &mut [u8]| {
        let mut rows: [&[u16]; 4] = [&[]; 4];
        for (row, v) in rows.iter_mut().zip(&views) {
            let r = y * v.c.v / vmax;
            *row = &v.c.data[r * v.c.stride..][..v.c.stride];
        }
        let px = buf.chunks_exact_mut(4).zip(xs0..xs1);
        match nc {
            1 => {
                let (v, row) = (&views[0], rows[0]);
                if v.cols.is_some() {
                    for (o, x) in px {
                        let g = v.at(row, x);
                        o.copy_from_slice(&[g, g, g, 255]);
                    }
                    return;
                }
                let px = buf.chunks_exact_mut(4).zip(&row[xs0..xs1]);
                match &v.lut {
                    Some(l) => px.for_each(|(o, &r)| {
                        let g = l[r as usize];
                        o.copy_from_slice(&[g, g, g, 255]);
                    }),
                    None => px.for_each(|(o, &r)| {
                        let g = v.slow(r);
                        o.copy_from_slice(&[g, g, g, 255]);
                    }),
                }
            }
            3 => {
                for (o, x) in px {
                    o.copy_from_slice(&[
                        views[0].at(rows[0], x),
                        views[1].at(rows[1], x),
                        views[2].at(rows[2], x),
                        255,
                    ]);
                }
            }
            _ => {
                for (o, x) in px {
                    let k = 255 - views[3].at(rows[3], x) as u32;
                    let f = |c: u8| (((255 - c as u32) * k + 127) / 255) as u8;
                    o.copy_from_slice(&[
                        f(views[0].at(rows[0], x)),
                        f(views[1].at(rows[1], x)),
                        f(views[2].at(rows[2], x)),
                        255,
                    ]);
                }
            }
        }
    };
    let job = |oy: usize, o: &mut [u8], scratch: &mut (Vec<u8>, Vec<u32>)| {
        if sc == 1 {
            fill(y0 + oy, o);
            return;
        }
        let (buf, acc) = scratch;
        buf.resize((xs1 - xs0) * 4, 0);
        acc.clear();
        acc.resize(rw * 4, 0);
        let (sy0, sy1) = ((y0 + oy) * sc, ((y0 + oy) * sc + sc).min(h));
        for y in sy0..sy1 {
            fill(y, buf);
            for (x, a) in acc.chunks_exact_mut(4).enumerate() {
                let b = &buf[x * sc * 4..((x * sc + sc).min(xs1 - xs0)) * 4];
                for p in b.chunks_exact(4) {
                    for c in 0..4 {
                        a[c] += p[c] as u32;
                    }
                }
            }
        }
        for (x, (o, a)) in o.chunks_exact_mut(4).zip(acc.chunks_exact(4)).enumerate() {
            let sx0 = (x0 + x) * sc;
            let n = (((sx0 + sc).min(w) - sx0) * (sy1 - sy0)) as u32;
            for c in 0..4 {
                o[c] = ((a[c] + n / 2) / n) as u8;
            }
        }
    };
    if rw > 0 {
        if threads > 1 && needed >= MIN_PARALLEL_SAMPLES {
            out.par_chunks_mut(rw * 4)
                .enumerate()
                .for_each_init(|| (Vec::new(), Vec::new()), |s, (oy, o)| job(oy, o, s));
        } else {
            let mut s = (Vec::new(), Vec::new());
            for (oy, o) in out.chunks_mut(rw * 4).enumerate() {
                job(oy, o, &mut s);
            }
        }
    }
    Ok(DecodedPlanes {
        width: rw as u32,
        height: rh as u32,
        origin: (x0 as u32, y0 as u32),
        layout: PlanesLayout::Rgba,
        planes: vec![Plane {
            width: rw as u32,
            height: rh as u32,
            stride: rw * 4,
            data: out,
            h_samp: 1,
            v_samp: 1,
        }],
        truncated: s.truncated,
    })
}

/// The display path: decode the rows the output needs, then [`display`].
pub(crate) fn decode(
    p: &Parsed,
    data: &[u8],
    scale: Scale,
    region: Option<Region>,
    cancel: Option<&AtomicBool>,
    threads: usize,
) -> Result<DecodedPlanes> {
    let f = &p.frame;
    let (w, h) = (f.width as usize, f.height as usize);
    let (_, _, _, y1) = out_rect(w, h, scale, region)?;
    let rows = (y1 * scale as usize).min(h);
    let s = decode_samples(p, data, rows, threads, cancel)?;
    tracing::debug!(target: "jpeg", scans = p.scans.len(), truncated = s.truncated, threads, "huffman lossless");
    display(p, &s, scale, region, threads)
}
