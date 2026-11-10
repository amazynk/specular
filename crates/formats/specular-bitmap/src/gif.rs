//! GIF87a/GIF89a.
//!
//! [`Gif::parse`] walks the block structure once (no LZW) and records every frame's geometry,
//! palette, transparency, disposal and delay, plus the loop count; it is cheap even for long
//! animations. [`Decode`] renders the *first frame* on a transparent canvas at any scale or
//! region: the frame's LZW data is decoded once into a 1-byte-per-pixel index buffer (cached in
//! the [`Gif`]), and every scale/region is then rendered from it in parallel, expanding the
//! palette on the fly and de-interlacing by row mapping.
//!
//! [`Animator`] composites the frames in order (disposal: none/keep, background → transparent,
//! previous → restore) into a full-size RGBA canvas, for playback.
//!
//! Images and shown frames: an image without a graphic control extension (so without a
//! delay) is drawn together with the next image, and they show as one frame, as the pygif
//! test suite expects. When no image has the extension, images that tile the picture (no
//! two overlap: "high-colour" GIFs made of many 256-colour pieces) show as one still picture
//! unless the file loops; otherwise each image is a frame (players give them a default
//! delay). An explicit delay of 0 stays a frame of its own, as browsers
//! play it (the suite would merge those too).

use crate::lzw::Table;
use crate::scale::{Plan, Rows, fill_grey, render};
use crate::{Cancel, Decode, Error, Image, Info, Options, PixelFormat, Result, u16le};
use std::sync::{Arc, Mutex};

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a")
}

/// What happens to a frame's area before the next frame is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposal {
    /// Leave it (0 "unspecified" and 1 "do not dispose").
    Keep,
    /// Clear the frame's rectangle to transparent.
    Background,
    /// Restore the canvas as it was before the frame.
    Previous,
}

/// One frame's header facts.
#[derive(Clone, Debug)]
pub struct Frame {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
    /// Delay after this frame in 1/100 s, as stored.
    pub delay_cs: u16,
    /// A graphic control extension came before this image.
    pub graphic_control: bool,
    pub disposal: Disposal,
    pub transparent: Option<u8>,
    pub interlaced: bool,
    /// Local colour table: file offset and entry count.
    local_palette: Option<(usize, usize)>,
    min_code_size: u8,
    /// Offset of the first data sub-block.
    data: usize,
}

impl Frame {
    /// Delay in milliseconds as stored (0 stays 0; players clamp).
    pub fn delay_ms(&self) -> u32 {
        self.delay_cs as u32 * 10
    }
}

/// Animation facts.
#[derive(Clone, Debug)]
pub struct Animation {
    /// Every image in the file, in order.
    pub frames: Vec<Frame>,
    /// NETSCAPE2.0 loop count: `Some(0)` = forever, `None` = play once.
    pub loop_count: Option<u16>,
    /// The shown frames: each a run of images drawn together (see the module notes).
    pub shown: Vec<std::ops::Range<usize>>,
}

impl Animation {
    /// No two images overlap (pieces of one picture, not frames drawn over each other).
    fn tiles(frames: &[Frame]) -> bool {
        let r = |f: &Frame| (f.left, f.top, f.left + f.width, f.top + f.height);
        frames.iter().enumerate().all(|(i, a)| {
            let a = r(a);
            frames[..i].iter().all(|b| {
                let b = r(b);
                a.2 <= b.0 || b.2 <= a.0 || a.3 <= b.1 || b.3 <= a.1
            })
        })
    }

    /// Groups images into shown frames: an image without a graphic control extension joins
    /// the next image; with no such extension at all, the images form one picture when they
    /// tile it (no two overlap) and the file does not loop, else each is a frame.
    fn group(frames: &[Frame], loops: bool) -> Vec<std::ops::Range<usize>> {
        let n = frames.len();
        if !frames.iter().any(|f| f.graphic_control) {
            return if loops || !Animation::tiles(frames) {
                (0..n).map(|i| i..i + 1).collect()
            } else {
                std::iter::once(0..n).collect()
            };
        }
        let mut out = Vec::new();
        let mut start = 0;
        for (i, f) in frames.iter().enumerate() {
            if f.graphic_control {
                out.push(start..i + 1);
                start = i + 1;
            }
        }
        if start < n {
            out.push(start..n);
        }
        out
    }
}

/// First frame as palette indices.
struct Indices {
    data: Vec<u8>,
    /// Pixels actually decoded (the rest is missing).
    decoded: usize,
}

pub struct Gif {
    info: Info,
    global_palette: Option<(usize, usize)>,
    anim: Animation,
    /// The first frame's indices, decoded by the first request. Held only to clone or swap
    /// the `Arc`, so [`Gif::memory_bytes`] and [`Gif::trim`] never wait for an LZW decode.
    first: Mutex<Option<Arc<Indices>>>,
    /// Held while the first frame decodes, so concurrent requests share one decode.
    first_work: Mutex<()>,
}

impl std::fmt::Debug for Gif {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gif")
            .field("info", &self.info)
            .field("frames", &self.anim.frames.len())
            .finish()
    }
}

/// Skip a chain of data sub-blocks starting at `p`; returns the offset after the terminator.
fn skip_blocks(d: &[u8], mut p: usize) -> Option<usize> {
    loop {
        let n = *d.get(p)? as usize;
        p += 1;
        if n == 0 {
            return Some(p);
        }
        p += n;
    }
}

/// Join a chain of data sub-blocks (stops quietly at the end of the data).
fn join_blocks(d: &[u8], mut p: usize) -> Vec<u8> {
    let mut v = Vec::new();
    while let Some(&n) = d.get(p) {
        if n == 0 {
            break;
        }
        let e = (p + 1 + n as usize).min(d.len());
        v.extend_from_slice(&d[p + 1..e]);
        p += 1 + n as usize;
    }
    v
}

fn read_palette(d: &[u8], pal: Option<(usize, usize)>) -> Box<[[u8; 4]; 256]> {
    let mut out = Box::new([[0u8, 0, 0, 255]; 256]);
    match pal {
        Some((off, n)) => {
            for (i, e) in out.iter_mut().enumerate().take(n) {
                if let Some(c) = d.get(off + i * 3..off + i * 3 + 3) {
                    *e = [c[0], c[1], c[2], 255];
                }
            }
        }
        None => {
            // No colour table at all: grey ramp.
            for (i, e) in out.iter_mut().enumerate() {
                *e = [i as u8, i as u8, i as u8, 255];
            }
        }
    }
    out
}

impl Gif {
    pub fn parse(data: &[u8]) -> Result<Gif> {
        if data.len() < 13 {
            return Err(Error::Truncated("GIF header".into()));
        }
        if !probe(data) {
            return Err(Error::Malformed("not a GIF file".into()));
        }
        let mut sw = u16le(data, 6).unwrap_or(0) as u32;
        let mut sh = u16le(data, 8).unwrap_or(0) as u32;
        let flags = data[10];
        let mut p = 13;
        let global_palette = if flags & 0x80 != 0 {
            let n = 2usize << (flags & 7);
            let g = (p, n);
            p += n * 3;
            Some(g)
        } else {
            None
        };
        let mut frames = Vec::new();
        let mut loop_count = None;
        let mut has_loop = false;
        // Graphic control extension pending for the next image.
        let mut gce: Option<(u16, Disposal, Option<u8>)> = None;
        while let Some(&b) = data.get(p) {
            match b {
                0x2C => {
                    let Some(hd) = data.get(p + 1..p + 10) else {
                        break;
                    };
                    let left = u16le(hd, 0).unwrap_or(0) as u32;
                    let top = u16le(hd, 2).unwrap_or(0) as u32;
                    let w = u16le(hd, 4).unwrap_or(0) as u32;
                    let h = u16le(hd, 6).unwrap_or(0) as u32;
                    let fl = hd[8];
                    p += 10;
                    let local_palette = if fl & 0x80 != 0 {
                        let n = 2usize << (fl & 7);
                        let l = (p, n);
                        p += n * 3;
                        Some(l)
                    } else {
                        None
                    };
                    let Some(&min) = data.get(p) else { break };
                    p += 1;
                    let graphic_control = gce.is_some();
                    let (delay_cs, disposal, transparent) =
                        gce.take().unwrap_or((0, Disposal::Keep, None));
                    frames.push(Frame {
                        graphic_control,
                        left,
                        top,
                        width: w,
                        height: h,
                        delay_cs,
                        disposal,
                        transparent,
                        interlaced: fl & 0x40 != 0,
                        local_palette,
                        min_code_size: min,
                        data: p,
                    });
                    match skip_blocks(data, p) {
                        Some(e) => p = e,
                        None => break,
                    }
                }
                0x21 => {
                    let Some(&label) = data.get(p + 1) else { break };
                    let body = p + 2;
                    if label == 0xF9 && data.get(body) == Some(&4) {
                        if let Some(g) = data.get(body + 1..body + 5) {
                            let disposal = match (g[0] >> 2) & 7 {
                                2 => Disposal::Background,
                                3 => Disposal::Previous,
                                _ => Disposal::Keep,
                            };
                            let delay = u16::from_le_bytes([g[1], g[2]]);
                            let tr = (g[0] & 1 != 0).then_some(g[3]);
                            gce = Some((delay, disposal, tr));
                        }
                    } else if label == 0xFF
                        && data.get(body) == Some(&11)
                        && data
                            .get(body + 1..body + 12)
                            .is_some_and(|a| a == b"NETSCAPE2.0" || a == b"ANIMEXTS1.0")
                    {
                        has_loop = true;
                        let s = body + 12;
                        if data.get(s) == Some(&3) && data.get(s + 1) == Some(&1) {
                            loop_count = u16le(data, s + 2);
                        }
                    }
                    match skip_blocks(data, body) {
                        Some(e) => p = e,
                        None => break,
                    }
                }
                0x3B => break,
                // Stray bytes (some encoders pad with zeros): skip.
                0x00 => p += 1,
                _ => break,
            }
        }
        let Some(f0) = frames.first() else {
            return Err(Error::Truncated("GIF without an image".into()));
        };
        if sw == 0 || sh == 0 {
            sw = f0.left + f0.width;
            sh = f0.top + f0.height;
        }
        crate::check_dims(sw, sh)?;
        // LZW gives at most 4096 pixels a code.
        crate::proven(data.len(), sw, sh, 4096)?;
        let has_alpha = f0.transparent.is_some()
            || f0.left > 0
            || f0.top > 0
            || f0.left + f0.width < sw
            || f0.top + f0.height < sh;
        let n = Animation::group(&frames, has_loop).len();
        let pal_bits = f0
            .local_palette
            .or(global_palette)
            .map_or(8, |(_, n)| n.trailing_zeros() as u8);
        Ok(Gif {
            info: Info {
                width: sw,
                height: sh,
                has_alpha,
                alpha: crate::AlphaKind::Key,
                gray: false,
                model: crate::Model::Palette,
                bit_depth: pal_bits,
                coding: format!(
                    "GIF {}-colour palette{}{}",
                    1u32 << pal_bits,
                    if f0.interlaced { ", interlaced" } else { "" },
                    if n > 1 {
                        format!(", {n} frames")
                    } else {
                        String::new()
                    }
                ),
                random_access: false,
            },
            global_palette,
            anim: Animation {
                shown: Animation::group(&frames, has_loop),
                frames,
                loop_count,
            },
            first: Mutex::new(None),
            first_work: Mutex::new(()),
        })
    }

    pub fn animation(&self) -> &Animation {
        &self.anim
    }

    /// Memory held by the cached first-frame indices.
    pub fn memory_bytes(&self) -> usize {
        self.cached().map_or(0, |i| i.data.len())
    }

    /// Drops the cached first-frame indices (1 byte per pixel; the next decode runs the
    /// frame's LZW again, ~0.2 s for 24 MP) and returns their bytes. A decode in progress is
    /// not waited for: it keeps the indices it makes.
    pub fn trim(&self) -> usize {
        let taken = self.first.lock().unwrap_or_else(|e| e.into_inner()).take();
        taken.map_or(0, |i| i.data.len())
    }

    /// The parsed structure without the cached indices (for a one-off [`Animator`]).
    fn clone_header(&self) -> Gif {
        Gif {
            info: self.info.clone(),
            global_palette: self.global_palette,
            anim: self.anim.clone(),
            first: Mutex::new(None),
            first_work: Mutex::new(()),
        }
    }

    fn cached(&self) -> Option<Arc<Indices>> {
        self.first.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn palette(&self, d: &[u8], f: &Frame) -> Box<[[u8; 4]; 256]> {
        let mut pal = read_palette(d, f.local_palette.or(self.global_palette));
        if let Some(t) = f.transparent {
            pal[t as usize] = [0, 0, 0, 0];
        }
        pal
    }

    /// LZW-decode a frame into indices in stored (possibly interlaced) row order.
    /// Only the stored rows up to the last one shown on a canvas `canvas_h` rows high are
    /// kept, and never more pixels than the LZW data can produce (each code of 3+ bits yields
    /// at most 4096), so a frame declared far larger than the canvas or its data cannot
    /// make a huge buffer.
    fn frame_indices(d: &[u8], f: &Frame, table: &mut Table, canvas_h: u32) -> Indices {
        let visible = canvas_h.saturating_sub(f.top).min(f.height);
        let rows = if f.interlaced {
            (0..visible)
                .map(|y| interlaced_row(y, f.height) + 1)
                .max()
                .unwrap_or(0)
        } else {
            visible
        };
        let input = join_blocks(d, f.data);
        let n = (f.width as usize * rows as usize).min((input.len() * 8 / 3 + 1) * 4096);
        let mut data = vec![0u8; n];
        let r = table.decode(f.min_code_size, &input, &mut data);
        Indices {
            data,
            decoded: if r.truncated {
                r.written
            } else {
                n.max(r.written)
            },
        }
    }

    fn first_indices(&self, d: &[u8], cancel: Cancel) -> Result<Arc<Indices>> {
        if let Some(i) = self.cached() {
            return Ok(i);
        }
        let _work = self.first_work.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = self.cached() {
            return Ok(i);
        }
        if cancel() {
            return Err(Error::Cancelled);
        }
        let f = &self.anim.frames[0];
        let i = Arc::new(Gif::frame_indices(
            d,
            f,
            &mut Table::new(),
            self.info.height,
        ));
        *self.first.lock().unwrap_or_else(|e| e.into_inner()) = Some(i.clone());
        Ok(i)
    }
}

/// Stored row of display row `y` in an interlaced image of height `h`.
fn interlaced_row(y: u32, h: u32) -> u32 {
    let n1 = h.div_ceil(8);
    let n2 = h.saturating_sub(4).div_ceil(8);
    let n3 = h.saturating_sub(2).div_ceil(4);
    match y % 8 {
        0 => y / 8,
        4 => n1 + y / 8,
        2 | 6 => n1 + n2 + y / 4,
        _ => n1 + n2 + n3 + y / 2,
    }
}

struct FrameRows<'a> {
    f: &'a Frame,
    idx: &'a Indices,
    pal: &'a [[u8; 4]; 256],
}

impl Rows for FrameRows<'_> {
    fn row(&self, y: u32, x0: u32, x1: u32, out: &mut [u8]) {
        let f = self.f;
        out.fill(0);
        if y < f.top || y >= f.top + f.height {
            return;
        }
        let fy = y - f.top;
        let sy = if f.interlaced {
            interlaced_row(fy, f.height)
        } else {
            fy
        };
        let a = x0.max(f.left);
        let b = x1.min(f.left + f.width);
        if a >= b {
            return;
        }
        let rs = sy as usize * f.width as usize;
        let s = rs + (a - f.left) as usize;
        let e = rs + (b - f.left) as usize;
        let o = &mut out[(a - x0) as usize * 4..(b - x0) as usize * 4];
        let ok = e.min(self.idx.decoded).max(s);
        let k = ok - s;
        if k > 0 {
            crate::px::expand(&self.idx.data[s..ok], self.pal, &mut o[..k * 4]);
        }
        fill_grey(&mut o[k * 4..], 4);
    }
}

impl Decode for Gif {
    fn info(&self) -> &Info {
        &self.info
    }

    fn decode(&self, data: &[u8], opts: &Options, cancel: Cancel) -> Result<Image> {
        if self.anim.shown.first().is_some_and(|r| r.len() > 1) {
            // The first shown frame combines several images: composite them.
            let mut a = Animator::new(Arc::new(self.clone_header()));
            let (canvas, _, truncated) = match a.next_frame(data, cancel) {
                Some(r) => r?,
                None => return Err(Error::Truncated("GIF without an image".into())),
            };
            let mut img = scale_rgba(canvas, self.info.width, self.info.height, opts, cancel)?;
            img.truncated |= truncated;
            return Ok(img);
        }
        let plan = Plan::new(self.info.width, self.info.height, opts)?;
        let idx = self.first_indices(data, cancel)?;
        let f = &self.anim.frames[0];
        let pal = self.palette(data, f);
        let rows = FrameRows {
            f,
            idx: &idx,
            pal: &pal,
        };
        let out = render(&rows, 4, !self.info.has_alpha, &plan, opts.parallel, cancel)?;
        Ok(Image {
            width: plan.rw,
            height: plan.rh,
            format: PixelFormat::Rgba8,
            data: out,
            truncated: idx.decoded < idx.data.len(),
        })
    }
}

/// Plays the frames of a [`Gif`] in order, compositing onto a full-size RGBA canvas. Holds
/// one canvas (plus one saved copy while a "restore previous" frame is showing), never all
/// frames.
pub struct Animator {
    gif: Arc<Gif>,
    table: Box<Table>,
    canvas: Vec<u8>,
    saved: Option<Vec<u8>>,
    /// The next shown frame.
    next: usize,
    /// Disposal and index of the frame drawn last.
    pending: Option<(Disposal, usize)>,
}

impl Animator {
    pub fn new(gif: Arc<Gif>) -> Animator {
        let n = gif.info.width as usize * gif.info.height as usize * 4;
        Animator {
            gif,
            table: Table::new(),
            canvas: vec![0; n],
            saved: None,
            next: 0,
            pending: None,
        }
    }

    /// Index of the shown frame the next call returns.
    pub fn position(&self) -> usize {
        self.next
    }

    /// Start again at frame 0.
    pub fn rewind(&mut self) {
        self.canvas.fill(0);
        self.saved = None;
        self.next = 0;
        self.pending = None;
    }

    /// Composite the next shown frame (all its images). Returns the canvas (RGBA, the
    /// image's size), the frame's delay as stored (1/100 s, its last image's) and whether
    /// its data was truncated; `None` after the last frame.
    pub fn next_frame(
        &mut self,
        data: &[u8],
        cancel: Cancel,
    ) -> Option<Result<(&[u8], u16, bool)>> {
        let gif = self.gif.clone();
        let range = gif.anim.shown.get(self.next)?.clone();
        let mut truncated = false;
        let mut delay = 0;
        for i in range {
            match self.draw(&gif, data, i, cancel) {
                Ok((d, t)) => {
                    delay = d;
                    truncated |= t;
                }
                Err(e) => return Some(Err(e)),
            }
        }
        self.next += 1;
        Some(Ok((&self.canvas, delay, truncated)))
    }

    /// Disposes of the image drawn last and draws image `i`: (its delay, truncated).
    fn draw(&mut self, gif: &Gif, data: &[u8], i: usize, cancel: Cancel) -> Result<(u16, bool)> {
        let f = &gif.anim.frames[i];
        if cancel() {
            return Err(Error::Cancelled);
        }
        let (cw, ch) = (gif.info.width as usize, gif.info.height as usize);
        let rect = |f: &Frame| {
            let x0 = (f.left as usize).min(cw);
            let x1 = ((f.left + f.width) as usize).min(cw);
            let y0 = (f.top as usize).min(ch);
            let y1 = ((f.top + f.height) as usize).min(ch);
            (x0, x1, y0, y1)
        };
        // Dispose of the previous frame.
        if let Some((disp, i)) = self.pending.take() {
            match disp {
                Disposal::Keep => {}
                Disposal::Background => {
                    let (x0, x1, y0, y1) = rect(&gif.anim.frames[i]);
                    for y in y0..y1 {
                        self.canvas[(y * cw + x0) * 4..(y * cw + x1) * 4].fill(0);
                    }
                }
                Disposal::Previous => {
                    if let Some(s) = self.saved.take() {
                        self.canvas = s;
                    }
                }
            }
        }
        if f.disposal == Disposal::Previous {
            match &mut self.saved {
                Some(s) => s.copy_from_slice(&self.canvas),
                None => self.saved = Some(self.canvas.clone()),
            }
        }
        let idx = Gif::frame_indices(data, f, &mut self.table, ch as u32);
        let pal = gif.palette(data, f);
        let fw = f.width as usize;
        let (x0, x1, y0, y1) = rect(f);
        for y in y0..y1 {
            let fy = (y - f.top as usize) as u32;
            let sy = if f.interlaced {
                interlaced_row(fy, f.height)
            } else {
                fy
            } as usize;
            let s = sy * fw;
            let row = &mut self.canvas[(y * cw + x0) * 4..(y * cw + x1) * 4];
            let n = x1 - x0;
            let ok = idx.decoded.saturating_sub(s).min(n);
            for (o, &i) in row
                .chunks_exact_mut(4)
                .zip(idx.data.get(s..s + ok).unwrap_or(&[]))
            {
                let c = pal[i as usize];
                if c[3] != 0 {
                    o.copy_from_slice(&c);
                }
            }
            fill_grey(&mut row[ok * 4..], 4);
        }
        self.pending = Some((f.disposal, i));
        Ok((f.delay_cs, idx.decoded < idx.data.len()))
    }
}

/// Downscale / crop a full-resolution RGBA canvas (e.g. an [`Animator`] frame).
pub fn scale_rgba(
    canvas: &[u8],
    width: u32,
    height: u32,
    opts: &Options,
    cancel: Cancel,
) -> Result<Image> {
    struct Canvas<'a>(&'a [u8], usize);
    impl Rows for Canvas<'_> {
        fn row(&self, y: u32, x0: u32, x1: u32, out: &mut [u8]) {
            let s = (y as usize * self.1 + x0 as usize) * 4;
            out.copy_from_slice(&self.0[s..s + (x1 - x0) as usize * 4]);
        }
    }
    if canvas.len() < width as usize * height as usize * 4 {
        return Err(Error::BadRequest("canvas smaller than its size".into()));
    }
    let plan = Plan::new(width, height, opts)?;
    let data = render(
        &Canvas(canvas, width as usize),
        4,
        false,
        &plan,
        opts.parallel,
        cancel,
    )?;
    Ok(Image {
        width: plan.rw,
        height: plan.rh,
        format: PixelFormat::Rgba8,
        data,
        truncated: false,
    })
}
