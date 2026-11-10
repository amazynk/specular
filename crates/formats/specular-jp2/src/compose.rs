//! JPX composition (ISO/IEC 15444-2 M.11.10 and M.11.5): the frames of the composition box
//! (`comp`: the `copt` canvas and `inst` instruction sets) drawn from the file's compositing
//! layers.
//!
//! Compositing layer `i` is codestream `i` with its own headers ([`crate::decode_at`]), or the
//! one codestream its registration box (`creg`) names, placed on the layer's grid: codestream
//! sample `x` covers the layer grid from `x · XR + XO`, in steps of `XS` grid points per layer
//! pixel (and the same down). A layer built from several codestreams makes [`composition`]
//! give up, so callers show the codestreams instead. Each instruction crops its layer
//! (`XC, YC, WC, HC`), scales the crop to `WIDTH × HEIGHT` (nearest neighbour) and draws it
//! source-over at `XO, YO` on a transparent canvas, with the layer's opacity channel (`cdef`,
//! `opct` types 0 and 1, as the codestream decode delivers it) or chroma key (`opct` type 2).
//! Layers are taken in order and reused per `NEXT-USE`.
//!
//! A frame ends at an instruction with a non-zero `LIFE` (in ticks of the set's `TICK`
//! milliseconds; `0x7FFFFFFF` is forever). What a frame drew stays under the next ones where
//! the instruction's persistence bit is set and is cleared otherwise. Instruction sets play
//! `REPT + 1` times; one that repeats without end (`REPT` 65 535) loops the animation.
//! [`animation`] lists every frame (at most [`MAX_FRAMES`]), [`composition`] the first.

use crate::boxes::{self, be16, be32};
use crate::geom::{ceil_shr, try_vec};
use crate::{
    DecodeOptions, Decoder, Depth, Image, Jp2Error, MAX_OUTPUT_PIXELS, PixelFormat, Pixels, Result,
};
use rayon::prelude::*;

/// Instructions looked at, at most (instruction sets repeat up to 65 536 times).
const MAX_INSTRUCTIONS: usize = 4096;
/// Frames of an animation, at most.
pub const MAX_FRAMES: usize = 1024;
/// Placements over all frames (each frame lists what persists from earlier ones), at most.
const MAX_PLACEMENTS: usize = 1 << 16;

/// One frame of a JPX file's composition, from [`composition`] or [`animation`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Composition {
    /// The canvas (`copt`).
    pub width: u32,
    pub height: u32,
    /// What is drawn, bottom-most first (for later frames: what earlier ones left, then the
    /// frame's own instructions).
    pub placements: Vec<Placement>,
    /// Frames follow this one (an animation).
    pub animated: bool,
    /// How long the frame shows, in milliseconds (`None`: for ever, or a still).
    pub delay_ms: Option<u32>,
    /// Per compositing layer: its chroma key (`opct` type 2: the channel count, then the
    /// values at the channels' precisions).
    keys: Vec<Option<Vec<u8>>>,
    /// Per compositing layer: its codestream and its place on the layer grid.
    regs: Vec<Registration>,
}

/// The frames of a JPX animation, from [`animation`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Animation {
    /// Every frame, in display order.
    pub frames: Vec<Composition>,
    /// Times to play; 0: for ever.
    pub loop_count: u32,
}

/// One instruction of a [`Composition`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The compositing layer drawn.
    pub layer: usize,
    /// The part of the layer drawn (`XC, YC, WC, HC`, layer pixels); `None`: all of it.
    pub crop: Option<[u32; 4]>,
    /// Canvas position of the drawn part.
    pub x: u32,
    pub y: u32,
    /// Its size on the canvas (`WIDTH, HEIGHT`); `None`: the size of the crop.
    pub size: Option<(u32, u32)>,
}

/// Where a compositing layer's codestream lies on the layer grid (`creg`, M.11.7.7): sample
/// `x` of codestream `codestream` covers grid points from `x · xr + xo`; a layer pixel spans
/// `xs` grid points (and the same down).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Registration {
    codestream: usize,
    xs: u16,
    ys: u16,
    xr: u8,
    yr: u8,
    xo: u8,
    yo: u8,
}

impl Registration {
    fn plain(codestream: usize) -> Registration {
        Registration {
            codestream,
            xs: 1,
            ys: 1,
            xr: 1,
            yr: 1,
            xo: 0,
            yo: 0,
        }
    }

    fn is_identity(&self, layer: usize) -> bool {
        *self == Registration::plain(layer)
    }

    /// The layer size of a `w × h` codestream.
    fn layer_size(&self, (w, h): (u32, u32)) -> (u32, u32) {
        let f = |n: u32, r: u8, o: u8, s: u16| {
            ((u64::from(n) * u64::from(r) + u64::from(o)).div_ceil(u64::from(s))).min(1 << 30)
                as u32
        };
        (
            f(w, self.xr, self.xo, self.xs),
            f(h, self.yr, self.yo, self.ys),
        )
    }

    /// The codestream sample under the centre of layer pixel `s` along one axis (`n`
    /// samples, grid step `step`, codestream step `r`, offset `o`): the nearest one when the
    /// centre falls just outside but the pixel overlaps the codestream; `None` off it.
    fn sample(s: u64, n: u32, step: u16, r: u8, o: u8) -> Option<u32> {
        let (step, r, o, n) = (u64::from(step), u64::from(r), u64::from(o), u64::from(n));
        // The pixel spans grid points [s · step, (s + 1) · step); the codestream [o, o + n·r).
        let (start, end) = (s * step, (s + 1) * step);
        if end <= o || start >= o + n * r || n == 0 {
            return None;
        }
        // Centre, doubled.
        let g = (2 * s + 1) * step;
        let x = g.saturating_sub(2 * o) / (2 * r);
        Some(x.min(n - 1) as u32)
    }
}

impl Composition {
    /// Whether the frame is just codestream 0 of `size` as it is: drawing it shows what the
    /// first codestream does.
    pub fn plain(&self, size: (u32, u32)) -> bool {
        match self.placements.as_slice() {
            [p] => {
                p.layer == 0
                    && (p.x, p.y) == (0, 0)
                    && p.crop.is_none_or(|c| c == [0, 0, size.0, size.1])
                    && p.size.is_none_or(|s| s == size)
                    && (self.width, self.height) == size
                    && self.keys.first().is_none_or(Option::is_none)
                    && self.regs.first().is_none_or(|r| r.is_identity(0))
            }
            _ => false,
        }
    }

    /// Whether compositing layer `layer` has a chroma key.
    pub fn keyed(&self, layer: usize) -> bool {
        self.keys.get(layer).is_some_and(Option::is_some)
    }

    /// The codestream compositing layer `layer` draws (see [`crate::decode_at`]).
    pub fn codestream(&self, layer: usize) -> usize {
        self.regs.get(layer).map_or(layer, |r| r.codestream)
    }
}

/// The first frame of a JPX file's composition box, or `None` when there is none to draw
/// (not JPX, no `comp` box, an empty canvas, no instruction, a layer without its codestream)
/// or it registers layers in ways not supported (see the module docs).
pub fn composition(data: &[u8]) -> Option<Composition> {
    let a = animation(data)?;
    let animated = a.frames.len() > 1;
    let mut first = a.frames.into_iter().next()?;
    first.animated = animated;
    Some(first)
}

/// Every frame of a JPX file's composition box (one for a still composition); `None` as for
/// [`composition`].
pub fn animation(data: &[u8]) -> Option<Animation> {
    if !boxes::is_jp2(data) {
        return None;
    }
    let (mut jpx, mut comp, mut streams) = (false, None, 0usize);
    let mut headers = Vec::new();
    for (t, c) in boxes::boxes(&data[12..]) {
        match &t {
            b"ftyp" => jpx = c.get(0..4) == Some(b"jpx "),
            b"jplh" => headers.push(c),
            b"comp" if comp.is_none() => comp = Some(c),
            b"jp2c" | b"ftbl" => streams += 1,
            _ => {}
        }
    }
    let comp = comp.filter(|_| jpx)?;
    // Without layer headers the JP2 header describes the one layer.
    let layers = headers.len().max(1);
    let mut keys = vec![None; layers];
    let mut regs: Vec<Registration> = (0..layers).map(Registration::plain).collect();
    for (i, h) in headers.iter().enumerate().take(layers) {
        for (t, c) in boxes::boxes(h) {
            match &t {
                b"opct" if c.first() == Some(&2) => keys[i] = Some(c[1..].to_vec()),
                b"creg" => regs[i] = registration(c)?,
                _ => {}
            }
        }
    }
    // A layer whose codestream is missing ends the list of usable layers.
    let layers = regs.iter().take_while(|r| r.codestream < streams).count();
    let (mut width, mut height) = (0, 0);
    let mut sets = Vec::new();
    for (t, c) in boxes::boxes(comp) {
        match &t {
            b"copt" => (height, width) = (be32(c, 0)?, be32(c, 4)?),
            b"inst" => sets.push(c),
            _ => {}
        }
    }
    if width == 0 || height == 0 {
        return None;
    }
    let mut frames: Vec<Composition> = Vec::new();
    // What earlier frames left, and the current frame's instructions (with persistence).
    let mut kept: Vec<Placement> = Vec::new();
    let mut current: Vec<(Placement, bool)> = Vec::new();
    // Instruction index (over every set and repetition), and the layers waiting for reuse.
    let (mut k, mut next, mut total) = (0usize, 0usize, 0usize);
    let mut reuse: Vec<(usize, usize)> = Vec::new();
    let mut loop_count = 1;
    // Ends a frame: what was kept plus the frame's own; returns the placements it lists.
    fn end(
        current: &mut Vec<(Placement, bool)>,
        kept: &mut Vec<Placement>,
        frames: &mut Vec<Composition>,
        canvas: (u32, u32),
        delay_ms: Option<u32>,
    ) -> usize {
        let mut placements = kept.clone();
        placements.extend(current.iter().map(|c| c.0.clone()));
        let n = placements.len();
        kept.extend(current.drain(..).filter(|c| c.1).map(|c| c.0));
        frames.push(Composition {
            width: canvas.0,
            height: canvas.1,
            placements,
            animated: false,
            delay_ms,
            keys: Vec::new(),
            regs: Vec::new(),
        });
        n
    }
    let canvas = (width, height);
    'sets: for set in sets {
        let (ityp, rept, tick) = (be16(set, 0)?, be16(set, 2)?, be32(set, 4)?);
        let body = set.get(8..)?;
        let has = |bit: u16| (ityp >> bit) & 1 != 0;
        let size = [(0, 8), (1, 8), (2, 8), (5, 16)]
            .iter()
            .filter(|&&(b, _)| has(b))
            .map(|&(_, n)| n)
            .sum::<usize>();
        if size == 0 {
            return None;
        }
        // Played without end: once here, the whole animation loops.
        let times = if rept == u16::MAX {
            loop_count = 0;
            1
        } else {
            usize::from(rept) + 1
        };
        for _ in 0..times {
            for ins in body.chunks_exact(size) {
                if k >= MAX_INSTRUCTIONS || frames.len() >= MAX_FRAMES || total >= MAX_PLACEMENTS {
                    break 'sets;
                }
                let mut at = 0;
                let mut field = || {
                    let v = be32(ins, at);
                    at += 4;
                    v.unwrap_or(0)
                };
                let (x, y) = if has(0) { (field(), field()) } else { (0, 0) };
                let size = has(1).then(|| (field(), field()));
                let (life, next_use) = if has(2) { (field(), field()) } else { (0, 0) };
                let crop = has(5).then(|| [field(), field(), field(), field()]);
                let layer = match reuse.iter().position(|r| r.0 == k) {
                    Some(j) => reuse.swap_remove(j).1,
                    None => {
                        next += 1;
                        next - 1
                    }
                };
                if layer >= layers {
                    break 'sets;
                }
                if next_use > 0 {
                    reuse.push((k.saturating_add(next_use as usize), layer));
                }
                current.push((
                    Placement {
                        layer,
                        crop,
                        x,
                        y,
                        size,
                    },
                    life >> 31 != 0,
                ));
                k += 1;
                let ticks = life & 0x7fff_ffff;
                if ticks == 0x7fff_ffff {
                    // Shown for ever: the last frame.
                    end(&mut current, &mut kept, &mut frames, canvas, None);
                    loop_count = 1;
                    break 'sets;
                }
                if ticks != 0 {
                    let ms = u64::from(ticks) * u64::from(tick);
                    total += end(
                        &mut current,
                        &mut kept,
                        &mut frames,
                        canvas,
                        Some(ms.min(u64::from(u32::MAX)) as u32),
                    );
                }
            }
        }
    }
    if !current.is_empty() {
        end(&mut current, &mut kept, &mut frames, canvas, None);
    }
    if frames.is_empty() || frames[0].placements.is_empty() {
        return None;
    }
    if frames.len() == 1 {
        frames[0].delay_ms = None;
    }
    keys.truncate(layers);
    regs.truncate(layers);
    for f in &mut frames {
        f.keys = keys.clone();
        f.regs = regs.clone();
    }
    Some(Animation { frames, loop_count })
}

/// A `creg` box: one codestream (with its registration); `None` for a layer drawn from
/// several codestreams or an empty grid.
fn registration(c: &[u8]) -> Option<Registration> {
    let (xs, ys) = (be16(c, 0)?, be16(c, 2)?);
    match c.get(4..)? {
        &[c0, c1, xr, yr, xo, yo] if xs > 0 && ys > 0 && xr > 0 && yr > 0 => Some(Registration {
            codestream: usize::from(u16::from_be_bytes([c0, c1])),
            xs,
            ys,
            xr,
            yr,
            xo,
            yo,
        }),
        _ => None,
    }
}

/// A decoded layer.
struct Layer {
    img: Image,
    /// Discarded levels of `img`.
    reduce: u32,
    /// Full-resolution size: of the layer, and of its codestream.
    size: (u32, u32),
    cs_size: (u32, u32),
    /// Where the codestream lies on the layer grid.
    reg: Registration,
    /// Chroma key on the output's colour channels.
    key: Option<Vec<u16>>,
}

impl Layer {
    /// The column of `img` under the centre of layer column `s` (full resolution), if any.
    fn col(&self, s: u64) -> Option<u32> {
        let r = &self.reg;
        let x = Registration::sample(s, self.cs_size.0, r.xs, r.xr, r.xo)?;
        Some((x >> self.reduce).min(self.img.width.saturating_sub(1)))
    }

    /// The row of `img` under the centre of layer row `s` (full resolution), if any.
    fn row(&self, s: u64) -> Option<usize> {
        let r = &self.reg;
        let y = Registration::sample(s, self.cs_size.1, r.ys, r.yr, r.yo)?;
        Some((y >> self.reduce).min(self.img.height.saturating_sub(1)) as usize)
    }

    /// Straight RGBA in 0..=1 of pixel `(x, y)` of `img`.
    #[inline]
    fn get(&self, x: usize, y: usize) -> [f32; 4] {
        let w = self.img.width as usize;
        let (ch, px): (usize, [u32; 4]) = match &self.img.pixels {
            Pixels::U8(d) if self.img.format == PixelFormat::Gray8 => {
                let g = u32::from(d[y * w + x]);
                (1, [g, g, g, 255])
            }
            Pixels::U8(d) => {
                let p = &d[(y * w + x) * 4..][..4];
                (4, [p[0], p[1], p[2], p[3]].map(u32::from))
            }
            Pixels::U16(d) if self.img.format == PixelFormat::Gray16 => {
                let g = u32::from(d[y * w + x]);
                (1, [g, g, g, 65535])
            }
            Pixels::U16(d) => {
                let p = &d[(y * w + x) * 4..][..4];
                (4, [p[0], p[1], p[2], p[3]].map(u32::from))
            }
        };
        let max = if matches!(self.img.pixels, Pixels::U8(_)) {
            255.0
        } else {
            65535.0
        };
        if let Some(k) = &self.key {
            let n = k.len().min(if ch == 1 { 1 } else { 3 });
            if n > 0 && (0..n).all(|c| px[c] == u32::from(k[c])) {
                return [0.0; 4];
            }
        }
        px.map(|v| v as f32 / max)
    }
}

/// A placement in output pixels: its canvas columns `x0..x0 + cols.len()` and the layer
/// column each samples, its canvas rows and how to find the layer row of one.
struct Drawn<'a> {
    layer: &'a Layer,
    x0: usize,
    /// `None`: outside the layer's codestream (transparent).
    cols: Vec<Option<u32>>,
    rows: std::ops::Range<usize>,
    /// Full-resolution: destination top and height, crop top and height.
    dy: u64,
    dh: u64,
    cy: u64,
    ch: u64,
}

/// Canvas pixels `0..n` (at `r` discarded levels) whose centres fall in the destination span
/// `d..d + dn` (full resolution), with the crop coordinate (`c..c + cn`) each samples.
fn span(n: u32, r: u32, d: u64, dn: u64, c: u64, cn: u64) -> (usize, Vec<u64>) {
    let (mut first, mut out) = (0, Vec::new());
    // Centre of canvas pixel i, doubled: (2i + 1) · 2^r.
    let lo = (2 * d) >> r;
    let start = (lo.saturating_sub(1) / 2).min(u64::from(n));
    for i in start..u64::from(n) {
        let f = (2 * i + 1) << r;
        if f < 2 * d {
            continue;
        }
        if f >= 2 * (d + dn) {
            break;
        }
        if out.is_empty() {
            first = i as usize;
        }
        out.push(c + (f - 2 * d) * cn / (2 * dn));
    }
    (first, out)
}

/// Draws `comp` (a frame) from the layers of `data`: an RGBA image of the canvas at
/// `opts.reduce` discarded levels (`ceil(width / 2^reduce)` wide), 8- or 16-bit per
/// `opts.depth`. Layers are decoded at that reduction where they have the levels (else at
/// their smallest). A layer that fails to decode is left out and marks the result
/// [`Image::truncated`]. Regions are not supported.
pub fn decode_composite(data: &[u8], comp: &Composition, opts: &DecodeOptions) -> Result<Image> {
    Compositor::default().draw(data, comp, opts)
}

/// Draws the frames of one composition, keeping the decoded layers between frames (an
/// animation decodes each layer once).
#[derive(Default)]
pub struct Compositor {
    layers: Vec<Option<Layer>>,
    /// Layers that failed to decode.
    failed: Vec<bool>,
    /// The reduction and depth the kept layers were decoded at.
    at: Option<(u8, bool)>,
}

impl Compositor {
    /// Bytes of the decoded layers kept.
    pub fn memory_bytes(&self) -> usize {
        self.layers
            .iter()
            .flatten()
            .map(|l| match &l.img.pixels {
                Pixels::U8(d) => d.len(),
                Pixels::U16(d) => d.len() * 2,
            })
            .sum()
    }

    /// [`decode_composite`] with the layers decoded so far.
    pub fn draw(&mut self, data: &[u8], comp: &Composition, opts: &DecodeOptions) -> Result<Image> {
        if opts.region.is_some() {
            return Err(Jp2Error::InvalidRequest("regions of a JPX composition"));
        }
        let r = u32::from(opts.reduce.min(31));
        let (cw, chh) = (ceil_shr(comp.width, r), ceil_shr(comp.height, r));
        let area = u64::from(cw) * u64::from(chh);
        if area > MAX_OUTPUT_PIXELS {
            return Err(Jp2Error::TooLarge(area * 8));
        }
        let _res = specular_core::budget::reserve_n(area, 8)?;
        let sixteen = opts.depth == Depth::Sixteen;
        if self.at != Some((opts.reduce, sixteen)) {
            self.layers.clear();
            self.failed.clear();
            self.at = Some((opts.reduce, sixteen));
        }
        let mut truncated = false;
        for p in &comp.placements {
            if self.layers.len() <= p.layer {
                self.layers.resize_with(p.layer + 1, || None);
                self.failed.resize(p.layer + 1, false);
            }
            if self.layers[p.layer].is_some() || self.failed[p.layer] {
                truncated |= self.failed[p.layer];
                continue;
            }
            if crate::tile::cancelled(opts.cancel) {
                return Err(Jp2Error::Cancelled);
            }
            match decode_layer(data, comp, p.layer, opts) {
                Ok(l) => {
                    truncated |= l.img.truncated;
                    self.layers[p.layer] = Some(l);
                }
                Err(
                    e @ (Jp2Error::Cancelled | Jp2Error::TooLarge(_) | Jp2Error::NeedsMemory(_)),
                ) => {
                    return Err(e);
                }
                Err(_) => {
                    self.failed[p.layer] = true;
                    truncated = true;
                }
            }
        }
        render(&self.layers, comp, opts, (cw, chh), truncated)
    }
}

/// Draws the placements of `comp` from decoded `layers` on a `cw × chh` canvas.
fn render(
    layers: &[Option<Layer>],
    comp: &Composition,
    opts: &DecodeOptions,
    (cw, chh): (u32, u32),
    truncated: bool,
) -> Result<Image> {
    let r = u32::from(opts.reduce.min(31));
    let area = u64::from(cw) * u64::from(chh);
    let sixteen = opts.depth == Depth::Sixteen;
    let drawn: Vec<Drawn> = comp
        .placements
        .iter()
        .filter_map(|p| {
            let l = layers.get(p.layer)?.as_ref()?;
            let (lw, lh) = l.size;
            let [cx, cy, cwid, chei] = p.crop.unwrap_or([0, 0, lw, lh]);
            let (cx, cy) = (cx.min(lw), cy.min(lh));
            let (cwid, chei) = (cwid.min(lw - cx), chei.min(lh - cy));
            let (dw, dh) = p.size.unwrap_or((cwid, chei));
            if cwid == 0 || chei == 0 || dw == 0 || dh == 0 {
                return None;
            }
            let (x0, xs) = span(cw, r, p.x.into(), dw.into(), cx.into(), cwid.into());
            let (y0, ys) = span(chh, r, p.y.into(), dh.into(), cy.into(), chei.into());
            let cols = xs.iter().map(|&s| l.col(s)).collect();
            (!xs.is_empty() && !ys.is_empty()).then_some(Drawn {
                layer: l,
                x0,
                cols,
                rows: y0..y0 + ys.len(),
                dy: p.y.into(),
                dh: dh.into(),
                cy: cy.into(),
                ch: chei.into(),
            })
        })
        .collect();
    let n = area as usize * 4;
    let (mut p8, mut p16): (Vec<u8>, Vec<u16>) = if sixteen {
        (Vec::new(), try_vec(n)?)
    } else {
        (try_vec(n)?, Vec::new())
    };
    let row_px = cw as usize;
    let draw_row = |y: usize, acc: &mut Vec<[f32; 4]>| {
        acc.clear();
        acc.resize(row_px, [0.0; 4]);
        for d in &drawn {
            if !d.rows.contains(&y) {
                continue;
            }
            let f = (2 * y as u64 + 1) << r;
            let sy = d.cy + (f - 2 * d.dy) * d.ch / (2 * d.dh);
            let Some(ly) = d.layer.row(sy) else {
                continue;
            };
            for (a, lx) in acc[d.x0..].iter_mut().zip(&d.cols) {
                let Some(lx) = *lx else {
                    continue;
                };
                let s = d.layer.get(lx as usize, ly);
                let k = 1.0 - s[3];
                for c in 0..3 {
                    a[c] = s[c] * s[3] + a[c] * k;
                }
                a[3] = s[3] + a[3] * k;
            }
        }
    };
    let straight = |a: &[f32; 4], max: f32, out: &mut dyn FnMut(usize, f32)| {
        if a[3] <= 0.0 {
            return;
        }
        for c in 0..3 {
            out(c, (a[c] / a[3]).clamp(0.0, 1.0) * max + 0.5);
        }
        out(3, a[3].clamp(0.0, 1.0) * max + 0.5);
    };
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    crate::with_threads(opts.parallelism, area, || {
        let stop = |y: usize| {
            if y.is_multiple_of(64) && crate::tile::cancelled(opts.cancel) {
                cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            cancelled.load(std::sync::atomic::Ordering::Relaxed)
        };
        if sixteen {
            p16.par_chunks_mut(row_px * 4)
                .enumerate()
                .for_each_init(Vec::new, |acc, (y, row)| {
                    if stop(y) {
                        return;
                    }
                    draw_row(y, acc);
                    for (a, o) in acc.iter().zip(row.chunks_exact_mut(4)) {
                        straight(a, 65535.0, &mut |c, v| o[c] = v as u16);
                    }
                });
        } else {
            p8.par_chunks_mut(row_px * 4)
                .enumerate()
                .for_each_init(Vec::new, |acc, (y, row)| {
                    if stop(y) {
                        return;
                    }
                    draw_row(y, acc);
                    for (a, o) in acc.iter().zip(row.chunks_exact_mut(4)) {
                        straight(a, 255.0, &mut |c, v| o[c] = v as u8);
                    }
                });
        }
    });
    if cancelled.into_inner() {
        return Err(Jp2Error::Cancelled);
    }
    Ok(Image {
        width: cw,
        height: chh,
        origin: (0, 0),
        format: if sixteen {
            PixelFormat::Rgba16
        } else {
            PixelFormat::Rgba8
        },
        pixels: if sixteen {
            Pixels::U16(p16)
        } else {
            Pixels::U8(p8)
        },
        truncated,
        reduce: opts.reduce,
    })
}

/// Decodes compositing layer `i` for [`decode_composite`].
fn decode_layer(data: &[u8], comp: &Composition, i: usize, opts: &DecodeOptions) -> Result<Layer> {
    let reg = comp.regs.get(i).copied().unwrap_or(Registration::plain(i));
    let d = Decoder::new_layer(data, reg.codestream, i)?;
    let info = d.info();
    let reduce = opts.reduce.min(info.levels);
    // The key's values at the precision of each channel, scaled as the output is; keyed
    // layers deeper than 8 bits are decoded at 16 so that the key matches exactly.
    let raw = comp.keys.get(i).and_then(Option::as_deref);
    let prec = |c: usize| {
        if info.has_palette {
            info.output_precision
        } else {
            info.components
                .get(c)
                .or(info.components.last())
                .map_or(8, |c| c.precision)
        }
    };
    let deep = raw.is_some_and(|k| (0..usize::from(k[0])).any(|c| prec(c) > 8));
    let sixteen = opts.depth == Depth::Sixteen || deep;
    let key = raw.map(|k| {
        let max: u64 = if sixteen { 65535 } else { 255 };
        let mut at = 1;
        (0..usize::from(k[0]))
            .map_while(|c| {
                let p = u32::from(prec(c).clamp(1, 38));
                let nb = p.div_ceil(8) as usize;
                let v = k
                    .get(at..at + nb)?
                    .iter()
                    .fold(0u64, |v, &b| v << 8 | u64::from(b));
                at += nb;
                // As the decoder's output scaling: above 16 bits the top 16 are kept.
                let (v, p) = if p > 16 { (v >> (p - 16), 16) } else { (v, p) };
                let vmax = (1u64 << p) - 1;
                Some(((v.min(vmax) * max + vmax / 2) / vmax) as u16)
            })
            .collect::<Vec<u16>>()
    });
    let img = d.decode(&DecodeOptions {
        reduce,
        region: None,
        max_layers: opts.max_layers,
        depth: if sixteen {
            Depth::Sixteen
        } else {
            Depth::Eight
        },
        parallelism: opts.parallelism,
        cancel: opts.cancel,
    })?;
    if img.width == 0 || img.height == 0 {
        return Err(Jp2Error::Malformed("empty compositing layer"));
    }
    Ok(Layer {
        img,
        reduce: u32::from(reduce),
        size: reg.layer_size((info.width, info.height)),
        cs_size: (info.width, info.height),
        reg,
        key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_sample_pixel_centres() {
        // Full resolution: canvas pixels 2..5 of a 3-wide destination at 2, from crop 10..13.
        assert_eq!(span(8, 0, 2, 3, 10, 3), (2, vec![10, 11, 12]));
        // Doubled: each crop pixel twice.
        assert_eq!(span(8, 0, 1, 4, 0, 2), (1, vec![0, 0, 1, 1]));
        // Halved canvas: centres at 1, 3, 5, … of the full grid.
        assert_eq!(span(4, 1, 0, 8, 0, 8), (0, vec![1, 3, 5, 7]));
        // Off the canvas.
        assert_eq!(span(4, 0, 9, 2, 0, 2), (0, vec![]));
    }

    #[test]
    fn creg_registration() {
        let creg = |cdn: u16, xr: u8, xo: u8| {
            let mut c = vec![0, 2, 0, 1];
            c.extend(cdn.to_be_bytes());
            c.extend([xr, 1, xo, 0]);
            c
        };
        let r = registration(&creg(2, 1, 0)).unwrap();
        assert_eq!(r.codestream, 2);
        assert!(!r.is_identity(2));
        // Two grid points per layer pixel, one per sample: half size.
        assert_eq!(r.layer_size((10, 7)), (5, 7));
        // Layer pixel 1's centre is grid point 3: sample 3; offset 3 moves it to sample 0.
        assert_eq!(Registration::sample(1, 10, 2, 1, 0), Some(3));
        assert_eq!(Registration::sample(1, 10, 2, 1, 3), Some(0));
        assert_eq!(Registration::sample(0, 10, 2, 1, 3), None);
        assert_eq!(Registration::sample(9, 10, 2, 1, 0), None);
        assert!(Registration::plain(4).is_identity(4));
        // Several codestreams, or a zero grid: not taken.
        let mut two = creg(2, 1, 0);
        two.extend(creg(3, 1, 0)[4..].to_vec());
        assert!(registration(&two).is_none());
        assert!(registration(&creg(2, 0, 0)).is_none());
    }
}
