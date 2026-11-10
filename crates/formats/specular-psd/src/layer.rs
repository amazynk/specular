//! One layer decoded alone onto the document canvas: its colour channels and transparency
//! placed at the layer's rectangle, the user mask and the opacities folded into transparency.
//!
//! Each channel is stored on its own (compression word, then raw / PackBits with a row table /
//! one zlib stream, with or without prediction) and covers the layer's rectangle, or the
//! mask's for mask channels; rectangles may reach past the canvas. Rows are read in order and
//! only the part over the canvas is kept, so a ZIP channel inflates a row at a time and never
//! needs a buffer the size of its rectangle. Channels decode in parallel.

use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::decode::{fill_grey, unpredict};
use crate::*;

/// Colour modes whose layers are decoded. Photoshop has no layers in Bitmap, Indexed and
/// Multichannel documents.
pub(crate) fn mode_supported(mode: ColorMode) -> bool {
    matches!(
        mode,
        ColorMode::Grayscale
            | ColorMode::Duotone
            | ColorMode::Rgb
            | ColorMode::Cmyk
            | ColorMode::Lab
    )
}

/// Largest side of a layer or mask rectangle (a row of it is buffered).
const MAX_SIDE: i64 = 1 << 20;

/// Rows between checks of the cancel flag.
const CANCEL_ROWS: usize = 64;

/// A rectangle in canvas coordinates, `bottom`/`right` exclusive.
#[derive(Clone, Copy, Debug)]
struct Rect {
    top: i64,
    left: i64,
    bottom: i64,
    right: i64,
}

impl Rect {
    fn new(top: i32, left: i32, bottom: i32, right: i32) -> Rect {
        Rect {
            top: top.into(),
            left: left.into(),
            bottom: bottom.into(),
            right: right.into(),
        }
    }
    fn width(&self) -> i64 {
        (self.right - self.left).max(0)
    }
    fn height(&self) -> i64 {
        (self.bottom - self.top).max(0)
    }
    fn check(&self) -> Result<()> {
        if self.width() > MAX_SIDE || self.height() > MAX_SIDE {
            return Err(Error::Malformed(format!(
                "layer rectangle {}×{}",
                self.width(),
                self.height()
            )));
        }
        Ok(())
    }
    /// Intersection with the canvas `w × h`, as canvas ranges (empty when disjoint).
    fn on_canvas(&self, w: usize, h: usize) -> (Range<usize>, Range<usize>) {
        let clip = |a: i64, b: i64, n: usize| {
            let a = a.clamp(0, n as i64) as usize;
            let b = b.clamp(0, n as i64) as usize;
            a..b.max(a)
        };
        (
            clip(self.top, self.bottom, h),
            clip(self.left, self.right, w),
        )
    }
}

/// A sample type of the planes.
trait Sample: Copy + Send + Sync + Default + 'static {
    const BYTES: usize;
    const MAX: Self;
    fn from_be(b: &[u8]) -> Self;
    fn from_u8(v: u8) -> Self;
    /// `self × m / MAX`.
    fn scale(self, m: Self) -> Self;
    /// A mask value at density `d` (255 = itself): `MAX − (MAX − self) × d / 255`.
    fn soften(self, d: u8) -> Self;
    fn wrap(v: Vec<Self>) -> Samples;
}

impl Sample for u8 {
    const BYTES: usize = 1;
    const MAX: u8 = 255;
    fn from_be(b: &[u8]) -> u8 {
        b[0]
    }
    fn from_u8(v: u8) -> u8 {
        v
    }
    fn scale(self, m: u8) -> u8 {
        ((self as u32 * m as u32 + 127) / 255) as u8
    }
    fn soften(self, d: u8) -> u8 {
        255 - (255 - self).scale(d)
    }
    fn wrap(v: Vec<u8>) -> Samples {
        Samples::U8(v)
    }
}

impl Sample for u16 {
    const BYTES: usize = 2;
    const MAX: u16 = 65535;
    fn from_be(b: &[u8]) -> u16 {
        u16::from_be_bytes([b[0], b[1]])
    }
    fn from_u8(v: u8) -> u16 {
        v as u16 * 257
    }
    fn scale(self, m: u16) -> u16 {
        ((self as u32 * m as u32 + 32767) / 65535) as u16
    }
    fn soften(self, d: u8) -> u16 {
        65535 - (65535 - self).scale(d as u16 * 257)
    }
    fn wrap(v: Vec<u16>) -> Samples {
        Samples::U16(v)
    }
}

impl Sample for f32 {
    const BYTES: usize = 4;
    const MAX: f32 = 1.0;
    fn from_be(b: &[u8]) -> f32 {
        f32::from_be_bytes([b[0], b[1], b[2], b[3]])
    }
    fn from_u8(v: u8) -> f32 {
        v as f32 / 255.0
    }
    fn scale(self, m: f32) -> f32 {
        self * m.clamp(0.0, 1.0)
    }
    fn soften(self, d: u8) -> f32 {
        1.0 - (1.0 - self.clamp(0.0, 1.0)) * d as f32 / 255.0
    }
    fn wrap(v: Vec<f32>) -> Samples {
        Samples::F32(v)
    }
}

enum Rows<'a> {
    Raw(&'a [u8]),
    Rle {
        data: &'a [u8],
        /// Row `i` is `data[offs[i]..offs[i + 1]]`; rows past the table are missing.
        offs: Vec<usize>,
    },
    Zip {
        z: Box<zlib_rs::Inflate>,
        input: &'a [u8],
        pred: bool,
        /// The next row the stream gives.
        next: usize,
        /// The stream ended or broke: later rows are missing.
        done: bool,
    },
}

/// The rows of one stored channel, read in increasing order.
struct ChannelRows<'a> {
    rows: Rows<'a>,
    /// Row bytes.
    rb: usize,
    depth: u8,
    tmp: Vec<u8>,
}

impl<'a> ChannelRows<'a> {
    /// `stored` is the channel's data: compression word, then the rows of a `w × h`
    /// rectangle. `None` when the data is missing.
    fn new(stored: &'a [u8], w: usize, h: usize, depth: u8, psb: bool) -> Result<Option<Self>> {
        let rb = w * (depth as usize / 8);
        let Some((comp, body)) = stored.split_first_chunk::<2>() else {
            return Ok(None);
        };
        let rows = match u16::from_be_bytes(*comp) {
            0 => Rows::Raw(body),
            1 => {
                let entry = if psb { 4 } else { 2 };
                let n = h.min(body.len() / entry);
                let (table, data) = body.split_at(n * entry);
                let mut offs = Vec::with_capacity(n + 1);
                let mut acc = 0usize;
                offs.push(0);
                for e in table.chunks_exact(entry) {
                    let len = if entry == 2 {
                        u16::from_be_bytes([e[0], e[1]]) as usize
                    } else {
                        u32::from_be_bytes([e[0], e[1], e[2], e[3]]) as usize
                    };
                    acc = acc.saturating_add(len);
                    offs.push(acc);
                }
                Rows::Rle { data, offs }
            }
            c @ (2 | 3) => Rows::Zip {
                z: Box::new(zlib_rs::Inflate::new(true, 15)),
                input: body,
                pred: c == 3,
                next: 0,
                done: false,
            },
            c => return Err(Error::Unsupported(format!("layer compression {c}"))),
        };
        Ok(Some(ChannelRows {
            rows,
            rb,
            depth,
            tmp: Vec::new(),
        }))
    }

    /// Stored (big-endian) row `y` into `out` (`rb` bytes); rows must be asked in increasing
    /// order. False when the row is missing or short (the rest is grey).
    fn row(&mut self, y: usize, out: &mut [u8]) -> bool {
        let rb = self.rb;
        let got = match &mut self.rows {
            Rows::Raw(d) => {
                let start = y.saturating_mul(rb).min(d.len());
                let have = (d.len() - start).min(rb);
                out[..have].copy_from_slice(&d[start..start + have]);
                have
            }
            Rows::Rle { data, offs } => match (offs.get(y), offs.get(y + 1)) {
                (Some(&a), Some(&b)) => {
                    let (a, b) = (a.min(data.len()), b.min(data.len()));
                    rle::unpack(&data[a..b], out)
                }
                _ => 0,
            },
            Rows::Zip {
                z,
                input,
                pred,
                next,
                done,
            } => {
                // Skip rows the caller does not want (above the canvas).
                let mut have = 0;
                while !*done && *next <= y {
                    have = inflate_row(z, input, out, done);
                    *next += 1;
                    if have < rb {
                        *done = true;
                    }
                }
                if *next != y + 1 {
                    have = 0;
                }
                if *pred && have == rb {
                    self.tmp.resize(rb, 0);
                    unpredict(out, &mut self.tmp, self.depth);
                }
                have
            }
        };
        if got < rb {
            fill_grey(&mut out[got..], self.depth);
        }
        got == rb
    }
}

/// Inflates the next `out.len()` bytes of the stream; returns how many it produced.
fn inflate_row(z: &mut zlib_rs::Inflate, input: &[u8], out: &mut [u8], done: &mut bool) -> usize {
    let mut have = 0;
    while have < out.len() {
        let at = (z.total_in() as usize).min(input.len());
        let (in0, out0) = (z.total_in(), z.total_out());
        let r = z.decompress(
            &input[at..],
            &mut out[have..],
            zlib_rs::InflateFlush::NoFlush,
        );
        have += (z.total_out() - out0) as usize;
        match r {
            Ok(zlib_rs::Status::StreamEnd) => {
                *done = true;
                break;
            }
            Ok(_) if z.total_in() != in0 || z.total_out() != out0 => {}
            _ => {
                *done = true;
                break;
            }
        }
    }
    have.min(out.len())
}

/// Shared facts of one layer decode.
struct Ctx<'a> {
    data: &'a [u8],
    w: usize,
    h: usize,
    depth: u8,
    psb: bool,
    cancel: Option<&'a AtomicBool>,
    truncated: AtomicBool,
    /// Fold the opacity and fill opacity into the transparency.
    fade: bool,
}

impl Ctx<'_> {
    fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|c| c.load(Ordering::Relaxed))
    }

    /// The stored data of channel `id` of `layer`, if recorded.
    fn channel<'l>(&self, layer: &'l Layer, id: i16) -> Option<&'l LayerChannel> {
        layer.channels.iter().find(|c| c.id == id)
    }

    /// Calls `f(canvas_y, row)` for each row of channel `ch` (covering `rect`) over the
    /// canvas, `row` holding the samples of the canvas columns `xs` as stored. Missing data
    /// reads grey and marks the decode truncated.
    fn for_rows(
        &self,
        ch: Option<&LayerChannel>,
        rect: Rect,
        mut f: impl FnMut(usize, &[u8]),
    ) -> Result<()> {
        rect.check()?;
        let (ys, xs) = rect.on_canvas(self.w, self.h);
        if ys.is_empty() || xs.is_empty() {
            return Ok(());
        }
        let (rw, rh) = (rect.width() as usize, rect.height() as usize);
        let bps = self.depth as usize / 8;
        let stored = ch
            .and_then(|c| self.data.get(c.data.clone()))
            .unwrap_or(&[]);
        let mut rows = ChannelRows::new(stored, rw, rh, self.depth, self.psb)?;
        let mut buf = vec![0u8; rw * bps];
        let x0 = (xs.start as i64 - rect.left) as usize * bps;
        let x1 = (xs.end as i64 - rect.left) as usize * bps;
        let mut ok = true;
        for (n, y) in ys.enumerate() {
            if n % CANCEL_ROWS == 0 && self.cancelled() {
                return Err(Error::Cancelled);
            }
            let ry = (y as i64 - rect.top) as usize;
            match rows.as_mut() {
                Some(r) => ok &= r.row(ry, &mut buf),
                None => {
                    fill_grey(&mut buf, self.depth);
                    ok = false;
                }
            }
            f(y, &buf[x0..x1]);
        }
        if !ok {
            self.truncated.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Fills `plane` (canvas-sized) over `rect` from channel `id`.
    fn fill_plane<T: Sample>(
        &self,
        layer: &Layer,
        id: i16,
        rect: Rect,
        plane: &mut [T],
    ) -> Result<()> {
        let w = self.w;
        let (_, xs) = rect.on_canvas(self.w, self.h);
        let ch = self.channel(layer, id);
        if id == -1 && ch.is_none() {
            // No transparency channel: opaque over the rectangle.
            let (ys, xs) = rect.on_canvas(self.w, self.h);
            for y in ys {
                plane[y * w + xs.start..y * w + xs.end].fill(T::MAX);
            }
            return Ok(());
        }
        self.for_rows(ch, rect, |y, row| {
            let dst = &mut plane[y * w + xs.start..y * w + xs.end];
            for (d, s) in dst.iter_mut().zip(row.chunks_exact(T::BYTES)) {
                *d = T::from_be(s);
            }
        })
    }

    /// Multiplies the transparency plane over the layer's rectangle by the user mask (its
    /// default outside the mask's rectangle) and the opacities.
    fn apply_mask<T: Sample>(&self, layer: &Layer, rect: Rect, alpha: &mut [T]) -> Result<()> {
        let w = self.w;
        let (ys, xs) = rect.on_canvas(self.w, self.h);
        if ys.is_empty() || xs.is_empty() {
            return Ok(());
        }
        let (op, fo) = if self.fade {
            (layer.opacity, layer.fill_opacity)
        } else {
            (255, 255)
        };
        let opacity = T::from_u8(op);
        let fill = T::from_u8(fo);
        let fade = |a: T| {
            let mut a = a;
            if op != 255 {
                a = a.scale(opacity);
            }
            if fo != 255 {
                a = a.scale(fill);
            }
            a
        };
        let mask = layer.mask.filter(|m| !m.disabled);
        let Some(m) = mask else {
            if op != 255 || fo != 255 {
                for y in ys {
                    for a in &mut alpha[y * w + xs.start..y * w + xs.end] {
                        *a = fade(*a);
                    }
                }
            }
            return Ok(());
        };
        let mrect = Rect::new(m.top, m.left, m.bottom, m.right);
        let default = T::from_u8(m.default).soften(m.density);
        // Outside the mask's rectangle: the default.
        let (mys, mxs) = mrect.on_canvas(self.w, self.h);
        for y in ys.clone() {
            let row = &mut alpha[y * w..(y + 1) * w];
            let inside = mys.contains(&y);
            for x in xs.clone() {
                if !(inside && mxs.contains(&x)) {
                    row[x] = fade(row[x].scale(default));
                }
            }
        }
        // Inside it (and the layer's): the mask channel.
        let ch = self.channel(layer, m.channel);
        self.for_rows(ch, mrect, |y, mrow| {
            if !ys.contains(&y) {
                return;
            }
            let row = &mut alpha[y * w..(y + 1) * w];
            for (x, s) in mxs.clone().zip(mrow.chunks_exact(T::BYTES)) {
                if xs.contains(&x) {
                    row[x] = fade(row[x].scale(T::from_be(s).soften(m.density)));
                }
            }
        })
    }

    fn decode<T: Sample>(&self, layer: &Layer, cc: usize, parallel: bool) -> Result<Vec<T>> {
        let plane = self.w * self.h;
        // A layer is drawn on the whole canvas: a tiny file must not ask for it.
        crate::proven(
            self.data.len(),
            plane * (cc + 1) * T::BYTES / 64,
            "layer data",
        )?;
        let mut out = vec![T::default(); plane * (cc + 1)];
        let rect = Rect::new(layer.top, layer.left, layer.bottom, layer.right);
        let job = |(c, p): (usize, &mut [T])| {
            let id = if c < cc { c as i16 } else { -1 };
            self.fill_plane(layer, id, rect, p)
        };
        if parallel {
            out.par_chunks_mut(plane).enumerate().try_for_each(job)?;
        } else {
            out.chunks_mut(plane).enumerate().try_for_each(job)?;
        }
        if self.cancelled() {
            return Err(Error::Cancelled);
        }
        self.apply_mask(layer, rect, &mut out[plane * cc..])?;
        Ok(out)
    }
}

pub(crate) fn planes(
    psd: &Psd,
    data: &[u8],
    index: usize,
    opts: &Options,
    cancel: Option<&AtomicBool>,
) -> Result<Planes> {
    planes_of(psd, data, index, opts, cancel, Layer::has_pixels, true)
}

/// A fill or shape layer's stored pixels (Photoshop keeps the rendered fill), when it has
/// a rectangle and colour data.
pub(crate) fn stored_pixels(l: &Layer) -> bool {
    l.bottom > l.top && l.right > l.left && l.channels.iter().any(|c| c.id >= 0 && c.data.len() > 2)
}

/// [`planes`] of a layer whose stored pixels are drawn ([`Layer::has_pixels`] or
/// [`stored_pixels`]); with `fade` false its opacity and fill opacity are left out.
pub(crate) fn drawn_planes(
    psd: &Psd,
    data: &[u8],
    index: usize,
    opts: &Options,
    cancel: Option<&AtomicBool>,
    fade: bool,
) -> Result<Planes> {
    planes_of(
        psd,
        data,
        index,
        opts,
        cancel,
        |l| l.has_pixels() || stored_pixels(l),
        fade,
    )
}

/// The user mask of layer `index` over the whole canvas, 0..1 (its default outside its
/// rectangle); `None` without an enabled mask.
pub(crate) fn mask_plane(
    psd: &Psd,
    data: &[u8],
    index: usize,
    cancel: Option<&AtomicBool>,
) -> Result<Option<Vec<f32>>> {
    let info = &psd.info;
    let Some(layer) = psd.layers.get(index) else {
        return Ok(None);
    };
    let Some(m) = layer.mask.filter(|m| !m.disabled) else {
        return Ok(None);
    };
    let (w, h) = (info.width as usize, info.height as usize);
    crate::proven(data.len(), w * h * 4 / 64, "mask data")?;
    let ctx = Ctx {
        data,
        w,
        h,
        depth: info.depth,
        psb: info.version == Version::Psb,
        cancel,
        truncated: AtomicBool::new(false),
        fade: true,
    };
    let d = f32::from(m.density) / 255.0;
    let soft = |v: f32| 1.0 - (1.0 - v) * d;
    let mut out = vec![soft(f32::from(m.default) / 255.0); w * h];
    let mrect = Rect::new(m.top, m.left, m.bottom, m.right);
    let (_, xs) = mrect.on_canvas(w, h);
    let bps = (info.depth as usize / 8).max(1);
    ctx.for_rows(ctx.channel(layer, m.channel), mrect, |y, row| {
        let dst = &mut out[y * w + xs.start..y * w + xs.end];
        for (d, s) in dst.iter_mut().zip(row.chunks_exact(bps)) {
            *d = soft(match bps {
                1 => f32::from(s[0]) / 255.0,
                2 => f32::from(u16::from_be_bytes([s[0], s[1]])) / 65535.0,
                _ => f32::from_be_bytes([s[0], s[1], s[2], s[3]]).clamp(0.0, 1.0),
            });
        }
    })?;
    Ok(Some(out))
}

fn planes_of(
    psd: &Psd,
    data: &[u8],
    index: usize,
    opts: &Options,
    cancel: Option<&AtomicBool>,
    drawn: impl Fn(&Layer) -> bool,
    fade: bool,
) -> Result<Planes> {
    let info = &psd.info;
    if !mode_supported(info.mode) {
        return Err(Error::Unsupported(format!(
            "layers in {:?} mode",
            info.mode
        )));
    }
    let layer = psd
        .layers
        .get(index)
        .ok_or_else(|| Error::Malformed(format!("no layer {index}")))?;
    if !drawn(layer) {
        return Err(Error::Unsupported(format!(
            "layer {index} has no pixels of its own"
        )));
    }
    let cc = info.mode.color_channels(info.channels);
    let ctx = Ctx {
        data,
        w: info.width as usize,
        h: info.height as usize,
        depth: info.depth,
        psb: info.version == Version::Psb,
        cancel,
        truncated: AtomicBool::new(false),
        fade,
    };
    let samples = match info.depth {
        8 => u8::wrap(ctx.decode::<u8>(layer, cc, opts.parallel)?),
        16 => u16::wrap(ctx.decode::<u16>(layer, cc, opts.parallel)?),
        32 => f32::wrap(ctx.decode::<f32>(layer, cc, opts.parallel)?),
        d => return Err(Error::Unsupported(format!("{d}-bit layers"))),
    };
    Ok(Planes {
        width: info.width,
        height: info.height,
        mode: info.mode,
        color_channels: cc,
        alpha: true,
        samples,
        truncated: ctx.truncated.into_inner(),
        palette: None,
        transparent_index: None,
        matted_on_white: false,
    })
}
