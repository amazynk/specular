//! The decode pipeline: inflate -> unfilter -> convert -> (downscale) -> output.

use std::borrow::Cow;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;

use crate::convert::{Converter, Packed, deinterleave_rgb};
use crate::error::{PngError, Result};
use crate::filter::{unfilter, unfilter_rows};
use crate::inflate::{InflateError, Inflater, MAX_MATCH, OUT_SLACK, Progress, WINDOW, adler32};
use crate::scale::Downscaler;
use crate::{
    AUTO_PARALLEL_MIN_PIXELS, DecodeOptions, Image, Layout, Output, Parallelism, Plane, PngInfo,
};

/// Fresh output produced per inflate call (and pipeline batch size).
const CHUNK: usize = 256 * 1024;

/// Adam7 passes: (x0, y0, dx, dy).
pub(crate) const ADAM7: [(usize, usize, usize, usize); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

/// Grid spacing (x, y) of the pixels known after Adam7 passes `1..=k`.
const ADAM7_GRID: [(usize, usize); 8] = [
    (8, 8),
    (8, 8),
    (4, 8),
    (4, 4),
    (2, 4),
    (2, 2),
    (1, 2),
    (1, 1),
];

pub(crate) fn pass_size(w: usize, h: usize, p: usize) -> (usize, usize) {
    let (x0, y0, dx, dy) = ADAM7[p];
    (
        if w > x0 { (w - x0).div_ceil(dx) } else { 0 },
        if h > y0 { (h - y0).div_ceil(dy) } else { 0 },
    )
}

pub(crate) fn row_bytes(info: &PngInfo, w: usize) -> usize {
    (w * info.color_type.channels() * info.bit_depth as usize).div_ceil(8)
}

pub(crate) fn alloc_zeroed(n: usize) -> Result<Vec<u8>> {
    let mut probe: Vec<u8> = Vec::new();
    probe
        .try_reserve_exact(n)
        .map_err(|_| PngError::TooLarge(n as u64))?;
    drop(probe);
    Ok(vec![0u8; n])
}

/// The zlib data of `ranges`, borrowed if it is one range.
pub(crate) fn gather<'a>(data: &'a [u8], ranges: &[Range<usize>]) -> Cow<'a, [u8]> {
    match ranges {
        [r] => Cow::Borrowed(&data[r.clone()]),
        _ => {
            let mut v = Vec::with_capacity(ranges.iter().map(|r| r.len()).sum());
            for r in ranges {
                v.extend_from_slice(&data[r.clone()]);
            }
            Cow::Owned(v)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Inflate window

/// Inflates into a sliding window that always keeps the last 32 KiB (and every unread byte).
pub(crate) struct Stream<'a> {
    inf: Inflater<'a>,
    buf: Vec<u8>,
    /// First unread byte.
    start: usize,
    /// Write position.
    end: usize,
    /// Bytes still wanted; the stream counts as finished once they are produced.
    wanted: usize,
    adler: Option<u32>,
    finished: Option<std::result::Result<(), InflateError>>,
}

impl<'a> Stream<'a> {
    pub(crate) fn new(
        inf: Inflater<'a>,
        max_read: usize,
        wanted: usize,
        verify: bool,
    ) -> Stream<'a> {
        let n = WINDOW + max_read + CHUNK + MAX_MATCH + OUT_SLACK;
        Stream {
            inf,
            buf: vec![0u8; n.min(wanted + WINDOW + MAX_MATCH + OUT_SLACK + max_read)],
            start: 0,
            end: 0,
            wanted,
            adler: verify.then_some(1),
            finished: None,
        }
    }

    /// Inflate more. Returns the range of new bytes in `buf` (empty once finished).
    fn fill(&mut self) -> Range<usize> {
        if self.finished.is_some() {
            return self.end..self.end;
        }
        let limit = self.buf.len() - OUT_SLACK;
        if limit - self.end < MAX_MATCH.max(CHUNK / 4).min(limit / 2) {
            let keep_from = self.start.min(self.end.saturating_sub(WINDOW));
            self.buf.copy_within(keep_from..self.end, 0);
            self.start -= keep_from;
            self.end -= keep_from;
        }
        let before = self.end;
        let r = self.inf.inflate(&mut self.buf, &mut self.end, limit);
        let new = before..self.end;
        if let Some(a) = self.adler.as_mut() {
            *a = adler32(*a, &self.buf[new.clone()]);
        }
        self.wanted = self.wanted.saturating_sub(new.len());
        match r {
            Ok(Progress::Done) => {
                self.finished = Some(match (self.adler, self.inf.adler_trailer()) {
                    (Some(a), Some(t)) if a != t => Err(InflateError::BadChecksum),
                    _ => Ok(()),
                });
            }
            Ok(Progress::OutputFull) => {
                if self.wanted == 0 && self.adler.is_none() {
                    self.finished = Some(Ok(()));
                } else if new.is_empty() && limit - self.end >= MAX_MATCH {
                    // Cannot happen (room was made); guard against spinning.
                    self.finished = Some(Err(InflateError::BadSymbol));
                }
            }
            Err(e) => self.finished = Some(Err(e)),
        }
        new
    }
}

/// Something that hands out the inflated stream in pieces of the requested size.
pub(crate) trait RowSource {
    /// The next `n` bytes, or `None` if the data ends first.
    fn take(&mut self, n: usize) -> Option<&[u8]>;
    /// Why the data ended (`None`: cleanly, or not yet).
    fn error(&self) -> Option<InflateError>;
}

impl RowSource for Stream<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        while self.end - self.start < n {
            if self.finished.is_some() {
                return None;
            }
            self.fill();
        }
        let s = self.start;
        self.start += n;
        Some(&self.buf[s..s + n])
    }
    fn error(&self) -> Option<InflateError> {
        self.finished.and_then(|r| r.err())
    }
}

/// Benchmark hook (not API): inflate the image data of `png` through the sliding window, as the
/// pipeline producer does. Returns the inflated size and its Adler-32 (computed only if `adler`).
#[doc(hidden)]
pub fn bench_inflate(png: &[u8], adler: bool) -> Result<(usize, u32)> {
    let info = crate::read_info(png)?;
    let (w, h) = (info.width as usize, info.height as usize);
    let total = data_size(&info, w, h, info.interlaced);
    let input = gather(png, &info.idat);
    let mut s = Stream::new(
        Inflater::new_zlib(&input).map_err(PngError::Data)?,
        0,
        total,
        false,
    );
    let (mut n, mut a) = (0, 1);
    loop {
        s.start = s.end;
        let new = s.fill();
        n += new.len();
        if adler {
            a = adler32(a, &s.buf[new]);
        }
        if let Some(st) = s.finished {
            st.map_err(PngError::Data)?;
            return Ok((n, a));
        }
    }
}

/// Pipeline consumer: receives inflated bytes from a producer thread.
enum Msg {
    Data(Vec<u8>),
    End(std::result::Result<(), InflateError>),
}

struct PipeSource {
    rx: mpsc::Receiver<Msg>,
    recycle: mpsc::Sender<Vec<u8>>,
    cur: Vec<u8>,
    off: usize,
    spill: Vec<u8>,
    status: Option<std::result::Result<(), InflateError>>,
}

impl PipeSource {
    fn next_batch(&mut self) -> bool {
        if self.status.is_some() {
            return false;
        }
        match self.rx.recv() {
            Ok(Msg::Data(v)) => {
                let old = std::mem::replace(&mut self.cur, v);
                let _ = self.recycle.send(old);
                self.off = 0;
                true
            }
            Ok(Msg::End(s)) => {
                self.status = Some(s);
                false
            }
            Err(_) => {
                self.status = Some(Err(InflateError::Truncated));
                false
            }
        }
    }
}

impl RowSource for PipeSource {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.cur.len() - self.off >= n {
            let o = self.off;
            self.off += n;
            return Some(&self.cur[o..o + n]);
        }
        self.spill.clear();
        self.spill.extend_from_slice(&self.cur[self.off..]);
        self.off = self.cur.len();
        while self.spill.len() < n {
            if !self.next_batch() {
                return None;
            }
            let need = (n - self.spill.len()).min(self.cur.len());
            self.spill.extend_from_slice(&self.cur[..need]);
            self.off = need;
        }
        Some(&self.spill[..n])
    }
    fn error(&self) -> Option<InflateError> {
        self.status.and_then(|r| r.err())
    }
}

/// Producer side of the pipeline.
fn produce(mut s: Stream<'_>, tx: mpsc::SyncSender<Msg>, recycle: mpsc::Receiver<Vec<u8>>) {
    let mut batch: Vec<u8> = Vec::with_capacity(CHUNK + MAX_MATCH * 4);
    loop {
        // Nothing is read through `take`: only the window needs keeping.
        s.start = s.end;
        let new = s.fill();
        batch.extend_from_slice(&s.buf[new]);
        let done = s.finished.is_some();
        if batch.len() >= CHUNK || (done && !batch.is_empty()) {
            let mut next = recycle.try_recv().unwrap_or_default();
            next.clear();
            if tx
                .send(Msg::Data(std::mem::replace(&mut batch, next)))
                .is_err()
            {
                return;
            }
        }
        if let Some(st) = s.finished {
            let _ = tx.send(Msg::End(st));
            return;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Output

/// A packed full-resolution image (Adam7 / APNG work buffer).
pub(crate) struct Canvas {
    pub packed: Packed,
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
    /// `data` in the memory budget, while the decode runs.
    res: specular_core::Reservation,
}

impl Canvas {
    pub(crate) fn new(packed: Packed, w: usize, h: usize) -> Result<Canvas> {
        let n = w
            .checked_mul(h)
            .and_then(|n| n.checked_mul(packed.bytes_per_pixel()))
            .ok_or(PngError::TooLarge(u64::MAX))?;
        let res = specular_core::budget::reserve(n as u64)?;
        Ok(Canvas {
            packed,
            w,
            h,
            data: alloc_zeroed(n)?,
            res,
        })
    }
    pub(crate) fn stride(&self) -> usize {
        self.w * self.packed.bytes_per_pixel()
    }
}

/// The output image under construction.
pub(crate) struct Out {
    layout: Layout,
    packed: Packed,
    w: usize,
    h: usize,
    planes: Vec<Plane>,
    /// The planes in the memory budget, while the decode runs.
    _res: specular_core::Reservation,
}

fn grey(packed: Packed) -> [u8; 8] {
    match packed {
        Packed::Rgba8 => [128, 128, 128, 255, 0, 0, 0, 0],
        Packed::Rgba16 => {
            let g = 0x8080u16.to_ne_bytes();
            [g[0], g[1], g[0], g[1], g[0], g[1], 0xff, 0xff]
        }
        Packed::Gray16 | Packed::GrayA16 => {
            let g = 0x8080u16.to_ne_bytes();
            [g[0], g[1], 0xff, 0xff, 0, 0, 0, 0]
        }
        _ => [128; 8],
    }
}

impl Out {
    fn new(layout: Layout, w: usize, h: usize) -> Result<Out> {
        let packed = packed_for(layout);
        let (n, bpp) = match layout {
            Layout::Rgb8Planar => (3, 1),
            _ => (1, packed.bytes_per_pixel()),
        };
        let stride = w.checked_mul(bpp).ok_or(PngError::TooLarge(u64::MAX))?;
        let size = stride.checked_mul(h).ok_or(PngError::TooLarge(u64::MAX))?;
        if (size as u64).saturating_mul(n) > isize::MAX as u64 {
            return Err(PngError::TooLarge(size as u64 * n));
        }
        let res = specular_core::budget::reserve_n(n, size as u64)?;
        let mut planes = Vec::with_capacity(n as usize);
        for _ in 0..n {
            planes.push(Plane {
                width: w as u32,
                height: h as u32,
                stride,
                data: alloc_zeroed(size)?,
            });
        }
        Ok(Out {
            layout,
            packed,
            w,
            h,
            planes,
            _res: res,
        })
    }

    fn store(&mut self, y: usize, row: &[u8]) {
        if self.layout == Layout::Rgb8Planar {
            let w = self.w;
            let [r, g, b] = &mut self.planes[..] else {
                return;
            };
            deinterleave_rgb(
                row,
                &mut r.data[y * w..(y + 1) * w],
                &mut g.data[y * w..(y + 1) * w],
                &mut b.data[y * w..(y + 1) * w],
            );
        } else {
            let p = &mut self.planes[0];
            let n = p.stride;
            p.data[y * n..(y + 1) * n].copy_from_slice(&row[..n]);
        }
    }

    fn fill_grey(&mut self, from_row: usize) {
        let g = grey(self.packed);
        let bpp = match self.layout {
            Layout::Rgb8Planar => 1,
            _ => self.packed.bytes_per_pixel(),
        };
        for p in &mut self.planes {
            let start = (from_row * p.stride).min(p.data.len());
            for px in p.data[start..].chunks_exact_mut(bpp) {
                px.copy_from_slice(&g[..bpp]);
            }
        }
    }

    fn into_image(self, truncated: bool) -> Image {
        Image {
            width: self.w as u32,
            height: self.h as u32,
            layout: self.layout,
            planes: self.planes,
            truncated,
        }
    }

    /// Build the output from a packed full-resolution canvas.
    fn from_canvas(layout: Layout, c: Canvas, scale: usize, workers: usize) -> Result<Out> {
        let (ow, oh) = (c.w.div_ceil(scale), c.h.div_ceil(scale));
        if scale == 1 && layout != Layout::Rgb8Planar {
            let stride = c.stride();
            return Ok(Out {
                layout,
                packed: c.packed,
                w: ow,
                h: oh,
                planes: vec![Plane {
                    width: ow as u32,
                    height: oh as u32,
                    stride,
                    data: c.data,
                }],
                _res: c.res,
            });
        }
        Out::from_canvas_par(layout, &c, scale, workers)
    }

    fn from_canvas_ref(layout: Layout, c: &Canvas, scale: usize) -> Result<Out> {
        Out::from_canvas_par(layout, c, scale, 1)
    }

    /// [`Out::from_canvas_ref`] with the output rows split over `workers` threads.
    fn from_canvas_par(layout: Layout, c: &Canvas, scale: usize, workers: usize) -> Result<Out> {
        let (ow, oh) = (c.w.div_ceil(scale), c.h.div_ceil(scale));
        let mut out = Out::new(layout, ow, oh)?;
        // A 16-bit canvas shown as 8-bit: rows are narrowed first.
        let narrow = c.packed == Packed::Rgba16 && out.packed != Packed::Rgba16;
        let packed = if narrow { Packed::Rgba8 } else { c.packed };
        if scale == 1 {
            let mut tmp = vec![0u8; if narrow { c.w * 4 } else { 0 }];
            for y in 0..c.h {
                out.store(y, canvas_row(c, y, narrow, &mut tmp));
            }
            return Ok(out);
        }
        let row_len = ow * packed.bytes_per_pixel();
        let per = oh.div_ceil(workers.max(1)).max(1);
        // Output rows `oys` of the downscaled canvas, packed.
        let band = |oys: Range<usize>| {
            let mut d = Downscaler::new(packed, c.w, scale);
            let mut tmp = vec![0u8; if narrow { c.w * 4 } else { 0 }];
            let mut rows = vec![0u8; oys.len() * row_len];
            for (oy, orow) in oys.zip(rows.chunks_exact_mut(row_len)) {
                for y in oy * scale..((oy + 1) * scale).min(c.h) {
                    d.add(canvas_row(c, y, narrow, &mut tmp));
                }
                d.emit(orow);
            }
            rows
        };
        if workers <= 1 || oh < 2 * workers {
            for (oy, r) in band(0..oh).chunks_exact(row_len).enumerate() {
                out.store(oy, r);
            }
            return Ok(out);
        }
        let parts: Vec<Vec<u8>> = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..oh)
                .step_by(per)
                .map(|y0| {
                    let band = &band;
                    specular_thread::spawn_scoped(sc, "dec-png-band", move || {
                        band(y0..(y0 + per).min(oh))
                    })
                })
                .collect();
            // A worker cannot panic (no fallible step); an empty part would leave grey rows.
            hs.into_iter()
                .map(|h| h.join().unwrap_or_default())
                .collect()
        });
        for (i, p) in parts.iter().enumerate() {
            for (k, r) in p.chunks_exact(row_len).enumerate() {
                out.store(i * per + k, r);
            }
        }
        Ok(out)
    }
}

/// Canvas row `y`, narrowed to 8 bits through `tmp` if `narrow`.
fn canvas_row<'a>(c: &'a Canvas, y: usize, narrow: bool, tmp: &'a mut [u8]) -> &'a [u8] {
    let stride = c.stride();
    let r = &c.data[y * stride..(y + 1) * stride];
    if !narrow {
        return r;
    }
    for (d, s) in tmp.iter_mut().zip(r.chunks_exact(2)) {
        let v = u16::from_ne_bytes([s[0], s[1]]);
        *d = crate::convert::r16((v >> 8) as u8, v as u8);
    }
    tmp
}

/// An image of `c` at 1/`scale` (`layout` must hold `c`'s samples: Rgba8 for Rgba8...).
pub(crate) fn canvas_image(layout: Layout, c: &Canvas, scale: usize) -> Result<Image> {
    Ok(Out::from_canvas_ref(layout, c, scale.max(1))?.into_image(false))
}

// ---------------------------------------------------------------------------------------------
// Row processing

/// Box-filter levels fed with the same rows, finest first. Each factor is a multiple of the one
/// before, and each level is built from the sums of the level below it (exact integer sums, so
/// every level equals a separate downscale of the input rows) rather than from the rows again.
pub(crate) struct Pyramid {
    levels: Vec<(Downscaler, Vec<u8>, usize)>,
}

impl Pyramid {
    /// `scales` ascending, all > 1, each dividing the next.
    fn new(packed: Packed, w: usize, scales: &[usize]) -> Pyramid {
        Pyramid {
            levels: scales
                .iter()
                .map(|&s| {
                    (
                        Downscaler::new(packed, w, s),
                        vec![0; w.div_ceil(s) * packed.bytes_per_pixel()],
                        s,
                    )
                })
                .collect(),
        }
    }

    /// Add one input row; `emit(level, row)` for every output row completed.
    fn add(&mut self, row: &[u8], emit: &mut dyn FnMut(usize, &[u8])) {
        if let Some(l) = self.levels.first_mut() {
            l.0.add(row);
        }
        self.cascade(false, emit);
    }

    /// Emit the partial rows left at the bottom (or where the data ended).
    fn finish(&mut self, emit: &mut dyn FnMut(usize, &[u8])) {
        self.cascade(true, emit);
    }

    fn cascade(&mut self, flush: bool, emit: &mut dyn FnMut(usize, &[u8])) {
        for i in 0..self.levels.len() {
            let (lo, hi) = self.levels.split_at_mut(i + 1);
            let (d, orow, s) = &mut lo[i];
            let ready = if flush { d.rows() > 0 } else { d.rows() == *s };
            if !ready {
                if flush {
                    continue;
                }
                // Coarser levels fill only from this one.
                break;
            }
            if let Some((next, _, _)) = hi.first_mut() {
                d.fold_into(next);
            }
            d.emit(orow);
            emit(i, orow);
        }
    }
}

/// Whether `scales` (ascending, > 1) form a chain a [`Pyramid`] can build.
fn is_chain(scales: &[usize]) -> bool {
    scales.first().is_none_or(|&s| s > 1) && scales.windows(2).all(|w| w[1] % w[0] == 0)
}

/// Unfilters non-interlaced rows and writes them (converted) to the full-size output and/or
/// downscaled to the levels of a [`Pyramid`]: one inflate and unfilter feeds every level.
struct RowSink<'a> {
    conv: &'a Converter,
    w: usize,
    h: usize,
    rb: usize,
    bpp: usize,
    /// Rows are unfiltered straight into the (single-plane) full-size output.
    direct: bool,
    prev: Vec<u8>,
    cur: Vec<u8>,
    packed_row: Vec<u8>,
    full: Option<Out>,
    pyr: Option<Pyramid>,
    /// One output per pyramid level, and its next row.
    outs: Vec<Out>,
    ys: Vec<usize>,
    /// Helper threads that downscale whole bands (threaded scaled decodes).
    pool: Option<BandPool>,
    /// Input rows done.
    rows: usize,
    /// Column segments for wavefront unfiltering (more than one: rows are unfiltered in blocks
    /// on the rayon pool).
    segs: Vec<Range<usize>>,
    block_src: Vec<u8>,
    block: Vec<u8>,
}

impl<'a> RowSink<'a> {
    /// `scales`: ascending, distinct; 1 is the full-size output, the others a [`Pyramid`]
    /// chain.
    fn new(
        conv: &'a Converter,
        info: &PngInfo,
        w: usize,
        h: usize,
        layout: Layout,
        scales: &[usize],
    ) -> Result<RowSink<'a>> {
        let rb = row_bytes(info, w);
        let full = match scales.first() {
            Some(1) => Some(Out::new(layout, w, h)?),
            _ => None,
        };
        let down: Vec<usize> = scales.iter().copied().filter(|&s| s > 1).collect();
        let mut outs = Vec::with_capacity(down.len());
        for &s in &down {
            outs.push(Out::new(layout, w.div_ceil(s), h.div_ceil(s))?);
        }
        let packed = packed_for(layout);
        let direct = full.is_some() && conv.identity() && layout != Layout::Rgb8Planar;
        Ok(RowSink {
            conv,
            w,
            h,
            rb,
            bpp: conv.filter_bpp(),
            direct,
            prev: vec![0; rb],
            cur: vec![0; rb],
            packed_row: vec![
                0;
                if direct {
                    0
                } else {
                    w * packed.bytes_per_pixel()
                }
            ],
            full,
            pyr: (!down.is_empty()).then(|| Pyramid::new(packed, w, &down)),
            ys: vec![0; outs.len()],
            outs,
            pool: None,
            rows: 0,
            segs: Vec::new(),
            block_src: Vec::new(),
            block: Vec::new(),
        })
    }

    fn packed(&self) -> Packed {
        self.full
            .as_ref()
            .or(self.outs.first())
            .map_or(Packed::Rgba8, |o| o.packed)
    }

    /// Process filtered row `y` (filter byte first). `false` for an unknown filter type.
    fn row(&mut self, y: usize, row: &[u8]) -> bool {
        let (ft, src) = (row[0], &row[1..]);
        if self.direct
            && let Some(full) = &mut self.full
        {
            let p = &mut full.planes[0];
            let stride = p.stride;
            let ok = if y == 0 {
                unfilter(ft, self.bpp, &self.prev, src, &mut p.data[..self.rb])
            } else {
                let (a, b) = p.data.split_at_mut(y * stride);
                unfilter(ft, self.bpp, &a[(y - 1) * stride..], src, &mut b[..self.rb])
            };
            if !ok {
                return false;
            }
            self.rows = y + 1;
            if self.pyr.is_some() {
                let r = &p.data[y * stride..(y + 1) * stride];
                feed_levels(
                    &mut self.pyr,
                    &mut self.pool,
                    &mut self.outs,
                    &mut self.ys,
                    r,
                );
            }
            return true;
        }
        if !unfilter(ft, self.bpp, &self.prev, src, &mut self.cur) {
            return false;
        }
        let packed: &[u8] = if self.conv.identity() {
            &self.cur
        } else {
            self.conv.convert(&self.cur, self.w, &mut self.packed_row);
            &self.packed_row
        };
        if let Some(full) = &mut self.full {
            full.store(y, packed);
        }
        feed_levels(
            &mut self.pyr,
            &mut self.pool,
            &mut self.outs,
            &mut self.ys,
            packed,
        );
        std::mem::swap(&mut self.prev, &mut self.cur);
        self.rows = y + 1;
        true
    }

    /// Rows per wavefront block (0: rows are unfiltered one by one).
    fn block_rows(&self) -> usize {
        if self.segs.len() < 2 {
            return 0;
        }
        ((8 << 20) / self.rb.max(1)).clamp(8, 256)
    }

    /// Process the filtered rows `src` (each `rb + 1` bytes) starting at row `y0`, unfiltered
    /// together by the wavefront. Returns the rows done (fewer at an unknown filter type).
    fn block(&mut self, y0: usize, src: &[u8]) -> usize {
        let rb = self.rb;
        if self.direct
            && let Some(full) = &mut self.full
        {
            let p = &mut full.planes[0];
            let stride = p.stride;
            let (a, b) = p.data.split_at_mut(y0 * stride);
            let prev = if y0 == 0 {
                &self.prev[..]
            } else {
                &a[(y0 - 1) * stride..(y0 - 1) * stride + rb]
            };
            let n = src.len() / (rb + 1);
            let done = unfilter_rows(self.bpp, rb, &self.segs, prev, src, &mut b[..n * rb]);
            if self.pyr.is_some() {
                for r in b[..done * stride].chunks_exact(stride) {
                    feed_levels(
                        &mut self.pyr,
                        &mut self.pool,
                        &mut self.outs,
                        &mut self.ys,
                        r,
                    );
                }
            }
            self.rows = y0 + done;
            return done;
        }
        let n = src.len() / (rb + 1);
        self.block.resize(n * rb, 0);
        let done = unfilter_rows(self.bpp, rb, &self.segs, &self.prev, src, &mut self.block);
        for (k, raw) in self.block[..done * rb].chunks_exact(rb).enumerate() {
            let packed: &[u8] = if self.conv.identity() {
                raw
            } else {
                self.conv.convert(raw, self.w, &mut self.packed_row);
                &self.packed_row
            };
            if let Some(full) = &mut self.full {
                full.store(y0 + k, packed);
            }
            feed_levels(
                &mut self.pyr,
                &mut self.pool,
                &mut self.outs,
                &mut self.ys,
                packed,
            );
        }
        if done > 0 {
            self.prev
                .copy_from_slice(&self.block[(done - 1) * rb..done * rb]);
        }
        self.rows = y0 + done;
        done
    }

    /// The images: the full-size one first (if asked for), then the pyramid levels.
    fn finish(mut self, complete: bool) -> Vec<Image> {
        if let (Some(p), Some(pyr)) = (self.pool.take(), &mut self.pyr) {
            p.finish(pyr, &mut self.outs, &mut self.ys);
        }
        if let Some(pyr) = &mut self.pyr {
            let (outs, ys) = (&mut self.outs, &mut self.ys);
            pyr.finish(&mut |i, r| {
                outs[i].store(ys[i], r);
                ys[i] += 1;
            });
        }
        let mut truncated = !complete || self.rows < self.h;
        for (o, &y) in self.outs.iter().zip(&self.ys) {
            truncated |= y < o.h;
        }
        let mut images = Vec::with_capacity(self.outs.len() + 1);
        if let Some(mut full) = self.full.take() {
            if self.rows < full.h {
                full.fill_grey(self.rows);
            }
            images.push(full.into_image(truncated));
        }
        for (mut o, y) in self.outs.into_iter().zip(self.ys) {
            if y < o.h {
                o.fill_grey(y);
            }
            images.push(o.into_image(truncated));
        }
        images
    }
}

/// Hand one converted full-size row to the pyramid (directly, or through the band pool).
fn feed_levels(
    pyr: &mut Option<Pyramid>,
    pool: &mut Option<BandPool>,
    outs: &mut [Out],
    ys: &mut [usize],
    row: &[u8],
) {
    let Some(pyr) = pyr else { return };
    match pool {
        Some(p) => p.push(row, pyr, outs),
        None => pyr.add(row, &mut |i, r| {
            outs[i].store(ys[i], r);
            ys[i] += 1;
        }),
    }
}

/// Downscaling on helper threads for a scaled decode: the row thread packs a band of converted
/// rows (as many as the coarsest factor), a worker (with its own [`Pyramid`]) turns it into the
/// output rows of every level. Results are bit-identical to downscaling in place; the partial
/// band at the bottom (or where data ends) is downscaled by the row thread.
struct BandPool {
    jobs: Option<mpsc::SyncSender<(usize, Vec<u8>)>>,
    done: mpsc::Receiver<(usize, Vec<Vec<u8>>, Vec<u8>)>,
    free: Vec<Vec<u8>>,
    band: Vec<u8>,
    rows: usize,
    /// Rows per band.
    size: usize,
    /// Output rows per band, per level.
    per: Vec<usize>,
    /// Band being filled.
    next: usize,
    pending: usize,
}

impl BandPool {
    /// Start `workers` downscaling threads in `scope`.
    fn start<'s>(
        scope: &'s std::thread::Scope<'s, '_>,
        workers: usize,
        packed: Packed,
        w: usize,
        scales: &[usize],
    ) -> BandPool {
        let (jtx, jrx) = mpsc::sync_channel::<(usize, Vec<u8>)>(workers * 2);
        let (dtx, drx) = mpsc::channel();
        let jrx = std::sync::Arc::new(std::sync::Mutex::new(jrx));
        let row_len = w * packed.bytes_per_pixel();
        let size = scales.last().copied().unwrap_or(1);
        for wi in 0..workers {
            let (jrx, dtx) = (jrx.clone(), dtx.clone());
            let scales = scales.to_vec();
            specular_thread::spawn_scoped(scope, format!("dec-png-dn-{wi}"), move || {
                let mut pyr = Pyramid::new(packed, w, &scales);
                loop {
                    let job = match jrx.lock() {
                        Ok(rx) => rx.recv(),
                        Err(_) => return,
                    };
                    let Ok((y, band)) = job else { return };
                    let mut rows = vec![Vec::new(); scales.len()];
                    for r in band.chunks_exact(row_len) {
                        pyr.add(r, &mut |i, o| rows[i].extend_from_slice(o));
                    }
                    if dtx.send((y, rows, band)).is_err() {
                        return;
                    }
                }
            });
        }
        BandPool {
            jobs: Some(jtx),
            done: drx,
            free: Vec::new(),
            band: Vec::with_capacity(row_len * size),
            rows: 0,
            size,
            per: scales.iter().map(|&s| size / s).collect(),
            next: 0,
            pending: 0,
        }
    }

    fn collect(&mut self, (y, rows, band): (usize, Vec<Vec<u8>>, Vec<u8>), outs: &mut [Out]) {
        for (i, r) in rows.iter().enumerate() {
            if let Some(len) = r.len().checked_div(self.per[i]) {
                for (k, row) in r.chunks_exact(len.max(1)).enumerate() {
                    outs[i].store(y * self.per[i] + k, row);
                }
            }
        }
        self.pending -= 1;
        self.free.push(band);
    }

    /// Add one converted row; `pyr` downscales in place if the workers are gone.
    fn push(&mut self, row: &[u8], pyr: &mut Pyramid, outs: &mut [Out]) {
        self.band.extend_from_slice(row);
        self.rows += 1;
        if self.rows == self.size {
            let mut next = self.free.pop().unwrap_or_default();
            next.clear();
            let band = std::mem::replace(&mut self.band, next);
            self.rows = 0;
            let sent = self.jobs.as_ref().map(|j| j.send((self.next, band)));
            match sent {
                Some(Ok(())) => self.pending += 1,
                Some(Err(mpsc::SendError((_, band)))) => {
                    self.jobs = None;
                    let mut ys: Vec<usize> = self.per.iter().map(|p| self.next * p).collect();
                    for r in band.chunks_exact(row.len()) {
                        pyr.add(r, &mut |i, o| {
                            outs[i].store(ys[i], o);
                            ys[i] += 1;
                        });
                    }
                }
                None => {}
            }
            self.next += 1;
        }
        while let Ok(r) = self.done.try_recv() {
            self.collect(r, outs);
        }
    }

    /// Wait for the outstanding bands, then add the partial band to `pyr` (its rows still to
    /// be flushed). Sets `ys` to the next output row of each level.
    fn finish(mut self, pyr: &mut Pyramid, outs: &mut [Out], ys: &mut [usize]) {
        self.jobs = None;
        while self.pending > 0 {
            match self.done.recv() {
                Ok(r) => self.collect(r, outs),
                Err(_) => break,
            }
        }
        for (y, p) in ys.iter_mut().zip(&self.per) {
            *y = self.next * p;
        }
        if let Some(len) = self.band.len().checked_div(self.rows) {
            for r in self.band.chunks_exact(len) {
                pyr.add(r, &mut |i, o| {
                    outs[i].store(ys[i], o);
                    ys[i] += 1;
                });
            }
        }
    }
}

/// Helper threads for downscaling (besides the inflate and row threads).
fn band_workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().saturating_sub(2).clamp(1, 2))
}

fn check_cancel(c: Option<&AtomicBool>) -> Result<()> {
    if c.is_some_and(|c| c.load(Ordering::Relaxed)) {
        Err(PngError::Cancelled)
    } else {
        Ok(())
    }
}

/// Feed `h` rows from `src` into `sink`. Returns the number of rows completed.
fn run_rows(
    src: &mut dyn RowSource,
    sink: &mut RowSink<'_>,
    h: usize,
    cancel: Option<&AtomicBool>,
) -> Result<usize> {
    let n = sink.rb + 1;
    let block = sink.block_rows();
    if block > 0 {
        // Wavefront: gather a block of filtered rows (one at a time, so a truncated stream
        // still yields every row it has), then unfilter them together.
        let mut buf = std::mem::take(&mut sink.block_src);
        let mut y = 0;
        while y < h {
            check_cancel(cancel)?;
            buf.clear();
            let want = block.min(h - y);
            let mut got = 0;
            while got < want {
                let Some(row) = src.take(n) else { break };
                buf.extend_from_slice(row);
                got += 1;
            }
            let done = if got > 0 { sink.block(y, &buf) } else { 0 };
            y += done;
            if done < want {
                break;
            }
        }
        sink.block_src = buf;
        return Ok(y);
    }
    for y in 0..h {
        if y % 32 == 0 {
            check_cancel(cancel)?;
        }
        let Some(row) = src.take(n) else {
            return Ok(y);
        };
        if !sink.row(y, row) {
            return Ok(y);
        }
    }
    Ok(h)
}

/// Column segments for wavefront unfiltering of `w x h` (a single one: row by row). Only for
/// threaded decodes of images with long rows and enough data to amortise the hand-offs, and
/// only when unfiltering rather than inflating limits the pipeline: weakly compressed data
/// (photos, about 1.3-3:1) keeps the inflate thread busier than the unfilter, and splitting
/// the rows then only adds hand-offs (+15-35 % wall time measured); highly compressed data
/// (40:1 and more) inflates several times faster than it unfilters.
/// [`Wavefront::On`] and [`Wavefront::Off`] skip the size and ratio gate.
fn wavefront_segments(
    info: &PngInfo,
    ranges: &[Range<usize>],
    w: usize,
    h: usize,
    bpp: usize,
    mode: crate::Wavefront,
) -> Vec<Range<usize>> {
    let rb = row_bytes(info, w);
    let raw = (rb as u64 + 1) * (h as u64);
    let packed: u64 = ranges.iter().map(|r| r.len() as u64).sum();
    let gate = raw < WAVEFRONT_MIN_BYTES.load(Ordering::Relaxed)
        || raw < packed.saturating_mul(WAVEFRONT_MIN_RATIO.load(Ordering::Relaxed));
    if mode == crate::Wavefront::Off || (mode == crate::Wavefront::Auto && gate) {
        return Vec::new();
    }
    let threads = rayon::current_num_threads();
    if threads < 2 {
        return Vec::new();
    }
    crate::filter::segments(rb, bpp, threads)
}

/// Unfiltered images smaller than this (in bytes) are unfiltered row by row.
static WAVEFRONT_MIN_BYTES: AtomicU64 = AtomicU64::new(32 << 20);
/// ... and so are images whose data compresses less than this (raw : compressed).
static WAVEFRONT_MIN_RATIO: AtomicU64 = AtomicU64::new(8);

/// Test and benchmark hook: the image size (bytes) and compression ratio from which rows are
/// unfiltered in parallel column segments (`0, 0`: always, so small images exercise that
/// path; `u64::MAX, _`: never). The output is identical either way.
#[doc(hidden)]
pub fn set_wavefront_min(bytes: u64, ratio: u64) {
    WAVEFRONT_MIN_BYTES.store(bytes, Ordering::Relaxed);
    WAVEFRONT_MIN_RATIO.store(ratio, Ordering::Relaxed);
}

/// Decode passes `0..passes` of an Adam7 image into a packed canvas on the grid (`gx`, `gy`).
/// Returns the canvas and whether all passes were complete.
#[allow(clippy::too_many_arguments)]
fn run_adam7(
    src: &mut dyn RowSource,
    info: &PngInfo,
    conv: &Converter,
    packed: Packed,
    w: usize,
    h: usize,
    passes: usize,
    cancel: Option<&AtomicBool>,
) -> Result<(Canvas, bool)> {
    let (gx, gy) = ADAM7_GRID[passes];
    let mut c = Canvas::new(packed, w.div_ceil(gx), h.div_ceil(gy))?;
    let bpp_out = packed.bytes_per_pixel();
    let cstride = c.stride();
    let fbpp = conv.filter_bpp();
    let mut prev = Vec::new();
    let mut cur = Vec::new();
    let mut prow = Vec::new();
    // Last complete pass, and rows done in the pass after it.
    let mut done_passes = 0;
    let mut partial_rows = 0;
    'passes: for (p, &(x0, y0, dx, dy)) in ADAM7.iter().enumerate().take(passes) {
        let (pw, ph) = pass_size(w, h, p);
        if pw == 0 || ph == 0 {
            done_passes = p + 1;
            continue;
        }
        let rb = row_bytes(info, pw);
        prev.clear();
        prev.resize(rb, 0);
        cur.resize(rb, 0);
        prow.resize(pw * bpp_out, 0);
        for r in 0..ph {
            if r % 32 == 0 {
                check_cancel(cancel)?;
            }
            let Some(row) = src.take(rb + 1) else {
                break 'passes;
            };
            if !unfilter(row[0], fbpp, &prev, &row[1..], &mut cur) {
                break 'passes;
            }
            conv.convert(&cur, pw, &mut prow);
            let y = (y0 + r * dy) / gy;
            let line = &mut c.data[y * cstride..(y + 1) * cstride];
            if dx == gx {
                // Pass pixels are consecutive on the grid (always for the last pass decoded).
                let x = x0 / gx;
                line[x * bpp_out..(x + pw) * bpp_out].copy_from_slice(&prow);
            } else {
                for (i, px) in prow.chunks_exact(bpp_out).enumerate() {
                    let x = (x0 + i * dx) / gx;
                    line[x * bpp_out..(x + 1) * bpp_out].copy_from_slice(px);
                }
            }
            std::mem::swap(&mut prev, &mut cur);
            partial_rows = r + 1;
        }
        done_passes = p + 1;
        partial_rows = 0;
    }
    let complete = done_passes == passes;
    if !complete {
        fill_from_grid(&mut c, done_passes, partial_rows, gx, gy);
    }
    Ok((c, complete))
}

/// After a truncated Adam7 decode, fill the pixels the missing passes would have set from the
/// coarser grid that is known (as browsers show a partially loaded interlaced image).
fn fill_from_grid(c: &mut Canvas, done_passes: usize, partial_rows: usize, gx: usize, gy: usize) {
    let bpp = c.packed.bytes_per_pixel();
    let stride = c.stride();
    // Known grid in canvas pixels.
    let (kx, ky) = ADAM7_GRID[done_passes.max(1)];
    let (kx, ky) = ((kx / gx).max(1), (ky / gy).max(1));
    // With no complete pass, only the first `partial_rows` pass-1 rows are known.
    let known_rows = if done_passes == 0 {
        (partial_rows * 8).div_ceil(gy)
    } else {
        c.h
    };
    let g = grey(c.packed);
    for y in 0..c.h {
        let sy = y - y % ky;
        for x in 0..c.w {
            let sx = x - x % kx;
            if (sx, sy) == (x, y) && y < known_rows {
                continue;
            }
            if y < known_rows {
                let s = sy * stride + sx * bpp;
                c.data.copy_within(s..s + bpp, y * stride + x * bpp);
            } else {
                c.data[y * stride + x * bpp..y * stride + (x + 1) * bpp].copy_from_slice(&g[..bpp]);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Entry points

pub(crate) fn output_layout(info: &PngInfo, output: Output) -> Layout {
    match output {
        Output::Auto8 => info.auto_layout(),
        Output::Rgba8 => Layout::Rgba8,
        Output::Rgba16 => Layout::Rgba16,
        Output::Auto16 => info.auto_layout16(),
    }
}

fn packed_for(layout: Layout) -> Packed {
    match layout {
        Layout::Gray8 => Packed::Gray8,
        Layout::Rgb8Planar => Packed::Rgb8,
        Layout::Rgba8 => Packed::Rgba8,
        Layout::Rgba16 => Packed::Rgba16,
        Layout::Gray16 => Packed::Gray16,
        Layout::GrayAlpha16 => Packed::GrayA16,
    }
}

/// Total inflated size of an image (filter bytes included).
fn data_size(info: &PngInfo, w: usize, h: usize, interlaced: bool) -> usize {
    if !interlaced {
        return h.saturating_mul(row_bytes(info, w) + 1);
    }
    (0..7)
        .map(|p| {
            let (pw, ph) = pass_size(w, h, p);
            if pw == 0 {
                0
            } else {
                ph.saturating_mul(row_bytes(info, pw) + 1)
            }
        })
        .fold(0usize, usize::saturating_add)
}

fn use_threads(par: Parallelism, w: usize, h: usize) -> bool {
    match par {
        Parallelism::Single => false,
        Parallelism::Threads => true,
        Parallelism::Auto => (w as u64) * (h as u64) >= AUTO_PARALLEL_MIN_PIXELS,
    }
}

/// DEFLATE expands at most ~1032:1. An image needing far more data than its compressed size
/// can give is mostly missing: refuse it rather than allocate (and grey-fill) a huge output for
/// a few bytes of input (a cheap memory-exhaustion attack).
pub(crate) fn check_plausible(filtered: usize, compressed: usize) -> Result<()> {
    const SMALL: usize = 64 << 20;
    if filtered > SMALL && filtered / 1100 > compressed {
        return Err(PngError::Truncated(
            "image data far too short for the image size",
        ));
    }
    Ok(())
}

/// Decode the default image.
pub fn decode(data: &[u8], opts: &DecodeOptions<'_>) -> Result<Image> {
    let info = crate::chunk::parse(data, opts.verify_checksums)?;
    if opts.verify_checksums && !info.idat_complete {
        return Err(PngError::Truncated("image data"));
    }
    let layout = output_layout(&info, opts.output);
    let (w, h) = (info.width as usize, info.height as usize);
    decode_stream(
        data,
        &info,
        &info.idat,
        w,
        h,
        layout,
        opts,
        info.idot.as_deref(),
    )
}

/// Decode one zlib stream (the default image or an APNG frame) of size `w x h`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_stream(
    data: &[u8],
    info: &PngInfo,
    ranges: &[Range<usize>],
    w: usize,
    h: usize,
    layout: Layout,
    opts: &DecodeOptions<'_>,
    idot: Option<&[crate::IdotPart]>,
) -> Result<Image> {
    let scale = opts.scale.max(1) as usize;
    let mut v = decode_levels(data, info, ranges, w, h, layout, opts, idot, &[scale])?;
    v.pop().ok_or(PngError::Truncated("no image rows"))
}

/// [`decode_stream`] into several scales at once (ascending, distinct; those above 1 must form
/// a chain where each divides the next). Images in the order of `scales`.
#[allow(clippy::too_many_arguments)]
fn decode_levels(
    data: &[u8],
    info: &PngInfo,
    ranges: &[Range<usize>],
    w: usize,
    h: usize,
    layout: Layout,
    opts: &DecodeOptions<'_>,
    idot: Option<&[crate::IdotPart]>,
    scales: &[usize],
) -> Result<Vec<Image>> {
    let packed = packed_for(layout);
    let conv = Converter::new(info, packed);
    let total = data_size(info, w, h, info.interlaced);
    let compressed: usize = ranges.iter().map(|r| r.len()).sum();
    check_plausible(total, compressed)?;
    let input = gather(data, ranges);
    let inf = Inflater::new_zlib(&input).map_err(PngError::Data)?;
    let threads = use_threads(opts.parallelism, w, h);
    let verify = opts.verify_checksums;

    if info.interlaced {
        let max_row = row_bytes(info, w) + 1;
        let (canvas, complete, err) = if threads {
            with_pipeline(Stream::new(inf, 0, total, verify), |src| {
                let r = run_adam7(src, info, &conv, packed, w, h, 7, opts.cancel);
                (r, src.error())
            })
        } else {
            let mut s = Stream::new(inf, max_row, total, verify);
            let r = run_adam7(&mut s, info, &conv, packed, w, h, 7, opts.cancel);
            (r, s.error())
        }
        .pipe(|(r, e)| r.map(|(c, ok)| (c, ok, e)))?;
        if !complete && verify {
            return Err(err.map_or(PngError::Truncated("image data"), PngError::Data));
        }
        let workers = if threads { band_workers() + 1 } else { 1 };
        // Reduced scales from the canvas first; full size last, as it may take the canvas.
        let mut images = Vec::with_capacity(scales.len());
        for &s in scales.iter().filter(|&&s| s > 1) {
            images.push(Out::from_canvas_par(layout, &canvas, s, workers)?.into_image(!complete));
        }
        if scales.first() == Some(&1) {
            let out = Out::from_canvas(layout, canvas, 1, workers)?;
            images.insert(0, out.into_image(!complete));
        }
        return Ok(images);
    }

    let mut sink = RowSink::new(&conv, info, w, h, layout, scales)?;
    if threads && !verify {
        sink.segs = wavefront_segments(info, ranges, w, h, conv.filter_bpp(), opts.wavefront);
    }
    // A threaded scaled decode also downscales on helper threads (the scope joins them).
    std::thread::scope(|sc| {
        let down: Vec<usize> = scales.iter().copied().filter(|&s| s > 1).collect();
        if threads && !down.is_empty() {
            sink.pool = Some(BandPool::start(sc, band_workers(), sink.packed(), w, &down));
        }
        let rb1 = sink.rb + 1;
        let (rows, err) = if let Some(parts) = idot.filter(|_| threads && !verify) {
            decode_idot(ranges, &input, parts, &mut sink, h, opts.cancel)?
        } else if threads {
            with_pipeline(Stream::new(inf, 0, total, verify), |src| {
                let r = run_rows(src, &mut sink, h, opts.cancel);
                (r, src.error())
            })
            .pipe(|(r, e)| r.map(|n| (n, e)))?
        } else {
            let mut s = Stream::new(inf, rb1, total, verify);
            let r = run_rows(&mut s, &mut sink, h, opts.cancel)?;
            (r, s.error())
        };
        if rows < h {
            if verify || rows == 0 {
                return Err(err.map_or(
                    if rows == 0 {
                        PngError::Truncated("no image rows")
                    } else {
                        PngError::Truncated("image data")
                    },
                    PngError::Data,
                ));
            }
        } else if verify && let Some(e) = err {
            return Err(PngError::Data(e));
        }
        Ok(sink.finish(rows == h))
    })
}

/// Decode the default image at several scales in one pass: the image data is inflated and
/// unfiltered once and feeds every output (scale 1 is the full-size image). Each image is
/// identical to [`decode`] at its scale. `opts.scale` is ignored. Returns the images in the
/// order of `scales`; repeated scales are decoded once and cloned.
pub fn decode_scales(data: &[u8], opts: &DecodeOptions<'_>, scales: &[u32]) -> Result<Vec<Image>> {
    let info = crate::chunk::parse(data, opts.verify_checksums)?;
    if opts.verify_checksums && !info.idat_complete {
        return Err(PngError::Truncated("image data"));
    }
    let layout = output_layout(&info, opts.output);
    let (w, h) = (info.width as usize, info.height as usize);
    let mut uniq: Vec<usize> = scales.iter().map(|&s| s.max(1) as usize).collect();
    uniq.sort_unstable();
    uniq.dedup();
    let down: Vec<usize> = uniq.iter().copied().filter(|&s| s > 1).collect();
    let images = if is_chain(&down) {
        decode_levels(
            data,
            &info,
            &info.idat,
            w,
            h,
            layout,
            opts,
            info.idot.as_deref(),
            &uniq,
        )?
    } else {
        // Not a chain (e.g. 2 and 3): one pass per scale.
        let mut v = Vec::with_capacity(uniq.len());
        for &s in &uniq {
            let o = DecodeOptions {
                scale: s as u32,
                ..*opts
            };
            v.push(decode_stream(
                data,
                &info,
                &info.idat,
                w,
                h,
                layout,
                &o,
                info.idot.as_deref(),
            )?);
        }
        v
    };
    let idx: Vec<usize> = scales
        .iter()
        .filter_map(|&s| uniq.iter().position(|&u| u == s.max(1) as usize))
        .collect();
    let mut slots: Vec<Option<Image>> = images.into_iter().map(Some).collect();
    let mut out = Vec::with_capacity(idx.len());
    for (k, &i) in idx.iter().enumerate() {
        let img = if idx[k + 1..].contains(&i) {
            slots[i].clone()
        } else {
            slots[i].take()
        };
        out.push(img.ok_or(PngError::Truncated("no image rows"))?);
    }
    Ok(out)
}

/// Box-filter an already decoded image (as [`decode`] returns it) to 1/`scale`, on up to
/// `workers` threads. Identical to decoding at that scale. `planes` are the image's planes
/// (data, stride) in [`Image::planes`] order.
pub fn downscale(
    layout: Layout,
    width: u32,
    height: u32,
    planes: &[(&[u8], usize)],
    scale: u32,
    workers: usize,
) -> Result<Image> {
    let (w, h, s) = (width as usize, height as usize, scale.max(1) as usize);
    let packed = packed_for(layout);
    let expect = if layout == Layout::Rgb8Planar { 3 } else { 1 };
    let row_len = w * packed.bytes_per_pixel();
    let plane_len = if layout == Layout::Rgb8Planar {
        w
    } else {
        row_len
    };
    let ok = planes.len() == expect
        && planes
            .iter()
            .all(|(d, st)| *st >= plane_len && (h == 0 || d.len() >= (h - 1) * st + plane_len));
    if !ok {
        return Err(PngError::Unsupported("plane layout"));
    }
    let (ow, oh) = (w.div_ceil(s), h.div_ceil(s));
    let mut out = Out::new(layout, ow, oh)?;
    let orow_len = ow * packed.bytes_per_pixel();
    // Input row `y`, packed (interleaved through `tmp` for planar RGB).
    let row = |y: usize, tmp: &mut Vec<u8>| {
        if layout == Layout::Rgb8Planar {
            tmp.resize(row_len, 0);
            let at = |p: usize| &planes[p].0[y * planes[p].1..y * planes[p].1 + w];
            for (((d, &r), &g), &b) in tmp.chunks_exact_mut(3).zip(at(0)).zip(at(1)).zip(at(2)) {
                d.copy_from_slice(&[r, g, b]);
            }
        } else {
            tmp.clear();
            tmp.extend_from_slice(&planes[0].0[y * planes[0].1..y * planes[0].1 + row_len]);
        }
    };
    let band = |oys: Range<usize>| {
        let mut d = Downscaler::new(packed, w, s);
        let mut tmp = Vec::new();
        let mut rows = vec![0u8; oys.len() * orow_len];
        for (oy, orow) in oys.zip(rows.chunks_exact_mut(orow_len.max(1))) {
            for y in oy * s..((oy + 1) * s).min(h) {
                row(y, &mut tmp);
                d.add(&tmp);
            }
            d.emit(orow);
        }
        rows
    };
    let workers = workers.clamp(1, oh.max(1));
    let per = oh.div_ceil(workers).max(1);
    let parts: Vec<Vec<u8>> = if workers == 1 {
        vec![band(0..oh)]
    } else {
        std::thread::scope(|sc| {
            let hs: Vec<_> = (0..oh)
                .step_by(per)
                .map(|y0| {
                    let band = &band;
                    specular_thread::spawn_scoped(sc, "dec-png-band", move || {
                        band(y0..(y0 + per).min(oh))
                    })
                })
                .collect();
            hs.into_iter()
                .map(|h| h.join().unwrap_or_default())
                .collect()
        })
    };
    let mut y = 0;
    for p in &parts {
        for r in p.chunks_exact(orow_len.max(1)) {
            if y < oh {
                out.store(y, r);
            }
            y += 1;
        }
    }
    if y < oh {
        out.fill_grey(y);
    }
    Ok(out.into_image(false))
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

/// Run `consume` on the calling thread against a [`PipeSource`] fed by a producer thread that
/// inflates `stream`.
fn with_pipeline<T>(stream: Stream<'_>, consume: impl FnOnce(&mut dyn RowSource) -> T) -> T {
    std::thread::scope(|sc| {
        let (tx, rx) = mpsc::sync_channel(4);
        let (rtx, rrx) = mpsc::channel();
        specular_thread::spawn_scoped(sc, "dec-png-inflate", move || produce(stream, tx, rrx));
        let mut src = PipeSource {
            rx,
            recycle: rtx,
            cur: Vec::new(),
            off: 0,
            spill: Vec::new(),
            status: None,
        };
        // Dropping `src` (receiver) stops the producer if the consumer bails out early.
        consume(&mut src)
    })
}

/// iDOT: parts after the first are inflated on helper threads (as raw DEFLATE starting at their
/// first IDAT chunk) while the first is streamed on this thread; then their rows are fed in
/// order. If a part fails to inflate on its own, the first stream simply continues into it.
#[allow(clippy::too_many_arguments)]
fn decode_idot(
    ranges: &[Range<usize>],
    input: &[u8],
    parts: &[crate::IdotPart],
    sink: &mut RowSink<'_>,
    h: usize,
    cancel: Option<&AtomicBool>,
) -> Result<(usize, Option<InflateError>)> {
    let rb1 = sink.rb + 1;
    // Offset of each part's data within `input` (the concatenated IDAT payloads).
    let mut offsets = Vec::with_capacity(parts.len());
    for p in parts {
        let mut off = 0usize;
        let mut found = None;
        for r in ranges {
            if r.start == p.file_offset + 8 {
                found = Some(off);
                break;
            }
            off += r.len();
        }
        offsets.push(found);
    }
    let usable = parts.len() >= 2
        && offsets[0] == Some(0)
        && offsets.iter().all(|o| o.is_some())
        && parts
            .last()
            .is_some_and(|p| (p.first_row + p.rows) as usize == h);
    let total = h * rb1;
    let inf = Inflater::new_zlib(input).map_err(PngError::Data)?;
    let mut first = Stream::new(inf, rb1, total, false);
    if !usable {
        let r = run_rows(&mut first, sink, h, cancel)?;
        return Ok((r, first.error()));
    }
    std::thread::scope(|sc| {
        let mut handles: Vec<Option<_>> = parts[1..]
            .iter()
            .zip(&offsets[1..])
            .enumerate()
            .map(|(i, (p, off))| {
                let start = off.unwrap_or(0);
                let end = offsets.get(i + 2).copied().flatten().unwrap_or(input.len());
                let want = p.rows as usize * rb1;
                let part = &input[start..end.max(start)];
                Some(specular_thread::spawn_scoped(
                    sc,
                    format!("dec-png-part-{i}"),
                    move || inflate_part(part, want, cancel),
                ))
            })
            .collect();
        let mut y = 0usize;
        // Still reading from `first` (it can continue into the next part only if it read all
        // previous parts itself).
        let mut own = true;
        let mut err = None;
        for (i, p) in parts.iter().enumerate() {
            let end = (p.first_row + p.rows) as usize;
            if (p.first_row as usize) != y {
                break;
            }
            if i > 0 {
                let r = handles[i - 1].take().and_then(|h| h.join().ok()).flatten();
                if let Some(buf) = r {
                    own = false;
                    let mut src = SliceSource { data: &buf, pos: 0 };
                    y = feed(&mut src, sink, y, end, cancel)?;
                    if y < end {
                        break;
                    }
                    continue;
                }
                if !own {
                    break;
                }
            }
            y = feed(&mut first, sink, y, end, cancel)?;
            if y < end {
                err = first.error();
                break;
            }
        }
        if y < h && !own && first.error().is_none() {
            // A helper's part was unusable after `first` was abandoned: decode the rest serially.
            let inf = Inflater::new_zlib(input).map_err(PngError::Data)?;
            let mut s = Stream::new(inf, rb1, total, false);
            for _ in 0..y {
                if s.take(rb1).is_none() {
                    return Ok((y, s.error()));
                }
            }
            y = feed(&mut s, sink, y, h, cancel)?;
            err = s.error();
        }
        Ok((y, err))
    })
}

struct SliceSource<'a> {
    data: &'a [u8],
    pos: usize,
}

impl RowSource for SliceSource<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.data.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(s)
    }
    fn error(&self) -> Option<InflateError> {
        None
    }
}

/// Feed rows `y..end` from `src`; returns the first row not done.
fn feed(
    src: &mut dyn RowSource,
    sink: &mut RowSink<'_>,
    y0: usize,
    end: usize,
    cancel: Option<&AtomicBool>,
) -> Result<usize> {
    let n = sink.rb + 1;
    for y in y0..end {
        if y % 32 == 0 {
            check_cancel(cancel)?;
        }
        let Some(row) = src.take(n) else {
            return Ok(y);
        };
        if !sink.row(y, row) {
            return Ok(y);
        }
    }
    Ok(end)
}

/// Inflate an iDOT part (raw DEFLATE) into a buffer of exactly `want` bytes.
fn inflate_part(input: &[u8], want: usize, cancel: Option<&AtomicBool>) -> Option<Vec<u8>> {
    let mut inf = Inflater::new(input);
    let mut out = Vec::new();
    out.try_reserve_exact(want + MAX_MATCH + OUT_SLACK).ok()?;
    out.resize(want + MAX_MATCH + OUT_SLACK, 0);
    let mut pos = 0;
    let full = want + MAX_MATCH;
    loop {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return None;
        }
        // Inflate in steps so cancellation is noticed.
        let limit = (pos + (4 << 20)).min(full);
        // Parts other than the last end at a flush point, not at a final block: having all
        // the rows is what counts.
        match inf.inflate(&mut out, &mut pos, limit) {
            Ok(Progress::OutputFull) if limit < full && pos < want => continue,
            _ => break,
        }
    }
    if pos != want {
        return None;
    }
    out.truncate(want);
    Some(out)
}

/// Decode an Adam7 file from its first passes only: `scale` 8, 4 or 2 decodes passes 1, 1-3 or
/// 1-5 (1/64, 1/16 or 1/4 of the image data) into a `ceil(W / scale) x ceil(H / scale)` image of
/// the pixels at multiples of `scale` (nearest sampling, no filtering). Other scales, and
/// non-interlaced files, decode normally at that scale.
pub fn decode_adam7_preview(data: &[u8], opts: &DecodeOptions<'_>) -> Result<Image> {
    let info = crate::chunk::parse(data, opts.verify_checksums)?;
    let passes = match opts.scale {
        8 => 1,
        4 => 3,
        2 => 5,
        _ => 0,
    };
    if !info.interlaced || passes == 0 {
        return decode(data, opts);
    }
    let layout = output_layout(&info, opts.output);
    let packed = packed_for(layout);
    let conv = Converter::new(&info, packed);
    let (w, h) = (info.width as usize, info.height as usize);
    let input = gather(data, &info.idat);
    let inf = Inflater::new_zlib(&input).map_err(PngError::Data)?;
    let want: usize = (0..passes)
        .map(|p| {
            let (pw, ph) = pass_size(w, h, p);
            if pw == 0 {
                0
            } else {
                ph.saturating_mul(row_bytes(&info, pw) + 1)
            }
        })
        .fold(0usize, usize::saturating_add);
    check_plausible(want, input.len())?;
    let mut s = Stream::new(inf, row_bytes(&info, w) + 1, want, false);
    let (c, complete) = run_adam7(&mut s, &info, &conv, packed, w, h, passes, opts.cancel)?;
    Ok(Out::from_canvas(layout, c, 1, 1)?.into_image(!complete))
}
