//! Rasterising pages on the GPU, one band at a time.
//!
//! A page is drawn in tiles of at most [`TILE_W`]×[`BAND_H`] pixels: a white scene, each
//! placed image drawn through the viewer's own image renderer (colour management,
//! orientation, Lanczos resampling, transparency over white) with [`View::fixed`] clipped to
//! its rectangle, then captions and the header through the UI renderer, and the final pass
//! to 8-bit sRGB. Each tile is read back and its rows are copied into the band; a finished
//! band goes to the sink (the PDF writer), so a page of any size needs memory for one band.
//!
//! Images come from an [`ImageSource`], decoded at the coarsest scale that still gives the
//! printed size its pixels, and stay cached on the GPU between pages and previews within a
//! budget.

use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use specular_core::{Orientation, Raster};
use specular_gfx::{Align, Compositor, DrawList, Gpu, Rect, Text, TextStyle, UiRenderer, wgpu};
use specular_render::{GpuImage, ImageRenderer, PxRect, RenderOptions, View};
use specular_theme::Color;

use crate::layout::{
    CAPTION_PT, Document, HEADER_PT, LINE, MM_PER_IN, Page, RectMm, SCALE_PT, mm_to_pt, pt_to_mm,
};

/// Largest tile width and the band height (pixels).
pub const TILE_W: u32 = 4096;
pub const BAND_H: u32 = 512;

/// Pixels of decoded images kept on the GPU between pages (beyond the current page's).
const CACHE_PIXELS: u64 = 48_000_000;

/// Decodes running at once while a page's images load.
const DECODE_THREADS: usize = 3;

/// The output format of the final pass (8-bit, sRGB-encoded by the shader).
const OUT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Colours of the printed text.
const INK: Color = Color::hex(0x1a1a1a);
const INK_SOFT: Color = Color::hex(0x5a5a5a);
const RULE: Color = Color::hex(0xb4b4b4);
const MISSING: Color = Color::hex(0xececec);

/// A decoded image ready for upload.
pub struct Loaded {
    pub raster: Raster,
    /// Full stored size (before orientation).
    pub stored: (u32, u32),
    pub orientation: Orientation,
    pub lut: Option<specular_render::icc::SourceLut>,
}

impl std::fmt::Debug for Loaded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loaded")
            .field("stored", &self.stored)
            .field("raster", &(self.raster.width, self.raster.height))
            .finish()
    }
}

/// Where the document's images come from.
pub trait ImageSource: Send + Sync {
    /// A stable identity of item `index` (cache key).
    fn key(&self, index: usize) -> u64;
    /// Item `index`, decoded so that its displayed long side has at least `long_px` pixels
    /// where the file has them.
    fn load(&self, index: usize, long_px: u32) -> Result<Loaded, String>;
    /// How many images may load at once (a source that develops each photo on the GPU
    /// at its printed size loads one at a time to stay inside the memory budget).
    fn parallel(&self) -> usize {
        DECODE_THREADS
    }
}

/// What rendering a page found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageStats {
    /// Images that could not be decoded (drawn as grey boxes).
    pub missing: usize,
    pub width: u32,
    pub height: u32,
}

/// Why a page was not finished.
#[derive(Debug)]
pub enum RenderError {
    Cancelled,
    Io(io::Error),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Cancelled => f.write_str("cancelled"),
            RenderError::Io(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<io::Error> for RenderError {
    fn from(e: io::Error) -> Self {
        RenderError::Io(e)
    }
}

struct Cached {
    key: u64,
    /// Long side of the decode that was uploaded (stored pixels).
    have_long: u32,
    img: GpuImage,
    stored: (u32, u32),
    orientation: Orientation,
    pixels: u64,
    used: u64,
}

/// The resolution at which the page of `setup` has a long side of `long_px`.
pub fn preview_dpi(setup: &crate::layout::PageSetup, long_px: u32) -> f64 {
    let (mw, mh) = setup.size_mm();
    f64::from(long_px.max(16)) / (mw.max(mh) / MM_PER_IN)
}

/// A piece of text on the page, in points.
struct Label {
    text: String,
    style: TextStyle,
    /// Left, top, width.
    x: f32,
    y: f32,
    w: f32,
    align: Align,
    color: Color,
}

/// The page renderer: its own image and UI renderers on a shared GPU.
pub struct PageRenderer {
    gpu: Gpu,
    images: ImageRenderer,
    comp: Compositor,
    ui: UiRenderer,
    text: Text,
    tile: (u32, u32),
    max_tile: (u32, u32),
    out: wgpu::Texture,
    readback: wgpu::Buffer,
    pitch: u32,
    cache: Vec<Cached>,
    next_id: u64,
    clock: u64,
}

impl std::fmt::Debug for PageRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageRenderer")
            .field("tile", &self.tile)
            .field("cached", &self.cache.len())
            .finish()
    }
}

fn target(gpu: &Gpu, (w, h): (u32, u32)) -> (wgpu::Texture, wgpu::Buffer, u32) {
    let out = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("print tile"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let pitch =
        (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("print readback"),
        size: u64::from(pitch) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    (out, readback, pitch)
}

/// Middle-truncates `s` with an ellipsis to fit `max_w` points (file names keep their
/// start and their extension); bidi isolates in it stay balanced (see
/// [`specular_i18n::ellipsis_cut`]).
fn ellipsize(text: &mut Text, s: &str, style: TextStyle, max_w: f32) -> String {
    if s.is_empty() {
        return String::new();
    }
    specular_i18n::ellipsize_to_fit(s, true, |t| text.measure(t, style) <= max_w)
}

/// The labels of `page` (captions and header), fitted to their rectangles. For a
/// right-to-left UI (`rtl`) the header and the scale note are mirrored, and names (file
/// names, the title) are isolated in right-to-left paragraphs so they keep their own order.
fn labels(text: &mut Text, page: &Page, rtl: bool) -> Vec<Label> {
    // The reading direction's start and end.
    let (start, end) = if rtl {
        (Align::End, Align::Start)
    } else {
        (Align::Start, Align::End)
    };
    let name = |s: &str| {
        if rtl {
            specular_i18n::isolate_label(s, specular_i18n::Direction::Rtl)
        } else {
            s.to_owned()
        }
    };
    let mut out = Vec::new();
    let cap = TextStyle::new(CAPTION_PT as f32);
    let line = (CAPTION_PT * LINE) as f32;
    for p in &page.images {
        let r = &p.caption_rect;
        let w = mm_to_pt(r.w) as f32;
        for (i, s) in p.caption.iter().enumerate() {
            if s.is_empty() {
                continue;
            }
            let style = cap;
            // The first line is the file name; the second the date, already localised.
            let s = if i == 0 { name(s) } else { s.clone() };
            out.push(Label {
                text: ellipsize(text, &s, style, w),
                style,
                x: mm_to_pt(r.x) as f32,
                y: mm_to_pt(r.y) as f32 + i as f32 * line,
                w,
                align: Align::Center,
                color: if i == 0 { INK } else { INK_SOFT },
            });
        }
    }
    if let Some(m) = &page.scale {
        let st = TextStyle::new(SCALE_PT as f32);
        let below = mm_to_pt(m.bar.bottom() + 0.6) as f32;
        let (bx, bw) = (mm_to_pt(m.bar.x) as f32, mm_to_pt(m.bar.w) as f32);
        out.push(Label {
            text: "0".into(),
            style: st,
            x: bx,
            y: below,
            w: bw,
            align: Align::Start,
            color: INK,
        });
        out.push(Label {
            text: m.length.clone(),
            style: st,
            x: bx,
            y: below,
            w: bw,
            align: Align::End,
            color: INK,
        });
        let w = mm_to_pt(m.note_rect.w) as f32;
        out.push(Label {
            text: ellipsize(text, &m.note, st, w),
            style: st,
            x: mm_to_pt(m.note_rect.x) as f32,
            y: mm_to_pt(m.note_rect.y) as f32,
            w,
            align: start,
            color: INK_SOFT,
        });
    }
    if let Some(h) = &page.header {
        let x = mm_to_pt(h.rect.x) as f32;
        let y = mm_to_pt(h.rect.y) as f32;
        let w = mm_to_pt(h.rect.w) as f32;
        let mut bold = TextStyle::new(HEADER_PT as f32);
        bold.weight = 600.0;
        let soft = TextStyle::new(HEADER_PT as f32);
        let page_w = text.measure(&h.page, soft);
        out.push(Label {
            text: h.page.clone(),
            style: soft,
            x,
            y,
            w,
            align: end,
            color: INK_SOFT,
        });
        let title_w = (w - page_w - HEADER_PT as f32 * 2.0).max(0.0);
        out.push(Label {
            text: ellipsize(text, &name(&h.title), bold, title_w),
            style: bold,
            // The title's room is on the start side: at the right in a right-to-left UI.
            x: if rtl { x + w - title_w } else { x },
            y,
            w: title_w,
            align: start,
            color: INK,
        });
    }
    out
}

impl PageRenderer {
    /// A renderer on `gpu` (a clone of the app's device) with `text` for captions:
    /// [`Text::new`] in the app, [`Text::bundled_only`] in tests.
    pub fn new(gpu: Gpu, text: Text) -> PageRenderer {
        let images = ImageRenderer::new(&gpu, 64 << 20);
        let tile = (256, 256);
        let comp = Compositor::new(&gpu, OUT, 16, tile.0, tile.1);
        let ui = UiRenderer::new(&gpu, &comp);
        let (out, readback, pitch) = target(&gpu, tile);
        PageRenderer {
            gpu,
            images,
            comp,
            ui,
            text,
            tile,
            max_tile: (TILE_W, BAND_H),
            out,
            readback,
            pitch,
            cache: Vec::new(),
            next_id: 1,
            clock: 0,
        }
    }

    /// Largest texture side the GPU takes.
    pub fn max_texture(&self) -> u32 {
        self.images.max_texture_size()
    }

    /// Caps the tile size (tests of the seams between tiles and bands).
    pub fn set_max_tile(&mut self, w: u32, h: u32) {
        self.max_tile = (w.max(16), h.max(16));
    }

    fn set_tile(&mut self, tile: (u32, u32)) {
        if tile == self.tile {
            return;
        }
        self.tile = tile;
        self.comp.resize(&self.gpu, tile.0, tile.1);
        self.ui.compositor_resized(&self.gpu, &self.comp);
        (self.out, self.readback, self.pitch) = target(&self.gpu, tile);
    }

    /// Drops every cached image.
    pub fn clear_cache(&mut self) {
        for c in self.cache.drain(..) {
            self.images.release_image(c.img.id);
        }
    }

    /// Makes sure the images of `page` are on the GPU at `px_per_mm`; returns the indices
    /// that failed to load.
    fn load_page(
        &mut self,
        page: &Page,
        px_per_mm: f64,
        source: &dyn ImageSource,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<usize>, RenderError> {
        self.clock += 1;
        let max = self.max_texture();
        let mut wanted: Vec<(usize, u64, u32)> = Vec::new();
        for p in &page.images {
            let long = (p.image.w.max(p.image.h) * px_per_mm).ceil().max(1.0) as u32;
            let long = long.min(max);
            let key = source.key(p.item);
            match self.cache.iter_mut().find(|c| c.key == key) {
                Some(c) if c.have_long >= long || c.have_long >= c.stored.0.max(c.stored.1) => {
                    c.used = self.clock;
                }
                _ => wanted.push((p.item, key, long)),
            }
        }
        let mut failed = Vec::new();
        if !wanted.is_empty() {
            let (tx, rx) = mpsc::sync_channel::<(usize, u64, Result<Loaded, String>)>(1);
            let next = std::sync::atomic::AtomicUsize::new(0);
            let stop = std::sync::atomic::AtomicBool::new(false);
            let wanted = &wanted;
            let next = &next;
            let stop = &stop;
            std::thread::scope(|s| -> Result<(), RenderError> {
                let workers = source.parallel().clamp(1, DECODE_THREADS).min(wanted.len());
                for _ in 0..workers {
                    let tx = tx.clone();
                    specular_thread::builder("print-decode")
                        .spawn_scoped(s, move || {
                            loop {
                                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let Some(&(item, key, long)) = wanted.get(i) else {
                                    break;
                                };
                                if stop.load(std::sync::atomic::Ordering::Relaxed) {
                                    break;
                                }
                                // A decoder panic on this image fails it alone (listed as
                                // missing); the other pages print.
                                let r = specular_core::isolate::run("print image", || {
                                    source.load(item, long)
                                })
                                .unwrap_or_else(|p| Err(p.brief()));
                                if tx.send((item, key, r)).is_err() {
                                    break;
                                }
                            }
                        })
                        .map_err(RenderError::Io)?;
                }
                drop(tx);
                for (item, key, r) in rx {
                    if cancelled() {
                        // The decoders stop at their next item.
                        stop.store(true, std::sync::atomic::Ordering::Relaxed);
                        continue;
                    }
                    match r {
                        Ok(l) => self.upload(key, l),
                        Err(e) => {
                            specular_log::warn!("print: image {item}: {e}");
                            failed.push(item);
                        }
                    }
                }
                Ok(())
            })?;
            if cancelled() {
                return Err(RenderError::Cancelled);
            }
        }
        self.evict();
        Ok(failed)
    }

    fn upload(&mut self, key: u64, l: Loaded) {
        if let Some(i) = self.cache.iter().position(|c| c.key == key) {
            let c = self.cache.swap_remove(i);
            self.images.release_image(c.img.id);
        }
        let id = self.next_id;
        self.next_id += 1;
        let (w, h) = l.stored;
        let mut img = self
            .images
            .create_image(&self.gpu, id, w, h, l.lut.as_ref());
        self.images.upload_overview(&self.gpu, &mut img, &l.raster);
        let have = l.raster.width.max(l.raster.height) * l.raster.scale.max(1);
        self.cache.push(Cached {
            key,
            have_long: have,
            img,
            stored: l.stored,
            orientation: l.orientation,
            pixels: u64::from(l.raster.width) * u64::from(l.raster.height),
            used: self.clock,
        });
    }

    /// Drops the least recently used images beyond the budget, never the current page's.
    fn evict(&mut self) {
        let mut total: u64 = self.cache.iter().map(|c| c.pixels).sum();
        while total > CACHE_PIXELS {
            let Some((i, _)) = self
                .cache
                .iter()
                .enumerate()
                .filter(|(_, c)| c.used < self.clock)
                .min_by_key(|(_, c)| c.used)
            else {
                break;
            };
            let c = self.cache.swap_remove(i);
            total -= c.pixels;
            self.images.release_image(c.img.id);
        }
    }

    /// Waits (up to a few seconds) for the system fonts.
    fn wait_for_fonts(&self) {
        let t0 = Instant::now();
        while self.text.fonts_pending() && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Renders page `index` of `doc` at `dpi`, handing rows (tightly packed RGB8, whole
    /// rows, top to bottom) to `sink` a band at a time. `cancelled` is polled between tiles.
    pub fn render_page(
        &mut self,
        doc: &Document,
        index: usize,
        dpi: f64,
        source: &dyn ImageSource,
        sink: &mut dyn FnMut(&[u8]) -> io::Result<()>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PageStats, RenderError> {
        let page = &doc.pages[index];
        let (pw, ph) = doc.setup.size_px_at(dpi);
        let px_per_mm = dpi / MM_PER_IN;
        // Exact pixels per mm horizontally and vertically (the page rounds to whole pixels).
        let (mw, mh) = doc.setup.size_mm();
        let (sx, sy) = (f64::from(pw) / mw, f64::from(ph) / mh);
        let failed = self.load_page(page, px_per_mm, source, cancelled)?;
        self.wait_for_fonts();
        let max = self.max_texture();
        let tile = (
            pw.min(self.max_tile.0).min(max),
            ph.min(self.max_tile.1).min(max),
        );
        self.set_tile(tile);
        // Text in points: one logical unit per point.
        let scale = (dpi / 72.0) as f32;
        self.text.begin_frame(scale);
        let labels = labels(&mut self.text, page, doc.rtl);
        let header_rule = page.header.as_ref().map(|h| {
            let y = mm_to_pt(h.rect.bottom() + pt_to_mm(HEADER_PT) * 0.25) as f32;
            (
                mm_to_pt(h.rect.x) as f32,
                y,
                mm_to_pt(h.rect.right()) as f32,
            )
        });
        let px = |r: &RectMm| PxRect {
            x: (r.x * sx) as f32,
            y: (r.y * sy) as f32,
            w: (r.w * sx) as f32,
            h: (r.h * sy) as f32,
        };
        let opts = RenderOptions {
            hq: true,
            canvas: [1.0; 4],
            checker_a: [1.0; 4],
            checker_b: [1.0; 4],
            grid: None,
            nearest_from: f32::INFINITY,
            ..RenderOptions::default()
        };
        let row = pw as usize * 3;
        let mut band = vec![0u8; row * tile.1 as usize];
        let mut y0 = 0u32;
        while y0 < ph {
            let rows = (ph - y0).min(tile.1);
            let mut x0 = 0u32;
            while x0 < pw {
                if cancelled() {
                    self.text.end_frame();
                    return Err(RenderError::Cancelled);
                }
                let cols = (pw - x0).min(tile.0);
                let off = (x0 as f32, y0 as f32);
                let tile_rect = PxRect {
                    x: off.0,
                    y: off.1,
                    w: tile.0 as f32,
                    h: tile.1 as f32,
                };
                let mut enc = self.gpu.device.create_command_encoder(&Default::default());
                drop(enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("print clear"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.comp.scene.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                }));
                let mut dl = DrawList::new(scale);
                let to_pt = |v: f32| v / scale;
                for p in &page.images {
                    let clip = px(&p.clip);
                    if clip.x >= tile_rect.x + tile_rect.w
                        || clip.y >= tile_rect.y + tile_rect.h
                        || clip.x + clip.w <= tile_rect.x
                        || clip.y + clip.h <= tile_rect.y
                    {
                        continue;
                    }
                    let key = source.key(p.item);
                    let cached = (!failed.contains(&p.item))
                        .then(|| self.cache.iter().find(|c| c.key == key))
                        .flatten();
                    let Some(c) = cached else {
                        dl.rect(
                            Rect::new(
                                to_pt(clip.x - off.0),
                                to_pt(clip.y - off.1),
                                to_pt(clip.w),
                                to_pt(clip.h),
                            ),
                            0.0,
                            MISSING,
                        );
                        continue;
                    };
                    let image = px(&p.image);
                    let (dw, _) = c.orientation.display_size(c.stored.0, c.stored.1);
                    let s = image.w / dw.max(1) as f32;
                    let vp = PxRect {
                        x: clip.x - off.0,
                        y: clip.y - off.1,
                        w: clip.w,
                        h: clip.h,
                    };
                    let centre = (
                        (clip.x + clip.w * 0.5 - image.x) / s,
                        (clip.y + clip.h * 0.5 - image.y) / s,
                    );
                    let view = View::fixed(c.stored, c.orientation, vp, s, centre);
                    self.images.render_inset(
                        &self.gpu,
                        &mut enc,
                        &self.comp.scene.view,
                        tile,
                        &c.img,
                        &view,
                        &opts,
                        None,
                    );
                }
                let (ox, oy) = (to_pt(off.0), to_pt(off.1));
                if let Some(m) = &page.scale {
                    // The bar: alternating ink and paper, outlined.
                    let n = m.segments.max(1);
                    let seg = mm_to_pt(m.bar.w) as f32 / n as f32;
                    let (x, y) = (mm_to_pt(m.bar.x) as f32 - ox, mm_to_pt(m.bar.y) as f32 - oy);
                    let h = mm_to_pt(m.bar.h) as f32;
                    for i in 0..n {
                        let fill = if i % 2 == 0 { INK } else { Color::WHITE };
                        dl.rect_border(
                            Rect::new(x + seg * i as f32, y, seg, h),
                            0.0,
                            fill,
                            INK,
                            0.5,
                        );
                    }
                }
                if let Some((x0r, y, x1r)) = header_rule {
                    dl.line((x0r - ox, y - oy), (x1r - ox, y - oy), 0.5, RULE);
                }
                for l in &labels {
                    let block = self.text.layout(&l.text, l.style, None, Align::Start);
                    // Skip labels outside the tile (layout is cached; drawing is not).
                    let top = l.y - oy;
                    if top > tile.1 as f32 / scale || top + block.height < 0.0 {
                        continue;
                    }
                    let x = match l.align {
                        Align::Start => l.x,
                        Align::Center => l.x + (l.w - block.width) * 0.5,
                        Align::End => l.x + l.w - block.width,
                    };
                    self.text.draw(&mut dl, &block, x - ox, top, l.color);
                }
                self.comp.begin(&mut enc);
                self.ui.render(
                    &self.gpu,
                    &mut enc,
                    &self.comp.comp.view,
                    tile,
                    &dl,
                    &mut self.text,
                );
                self.comp
                    .finish(&mut enc, &self.out.create_view(&Default::default()));
                enc.copy_texture_to_buffer(
                    self.out.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &self.readback,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(self.pitch),
                            rows_per_image: Some(tile.1),
                        },
                    },
                    wgpu::Extent3d {
                        width: tile.0,
                        height: rows,
                        depth_or_array_layers: 1,
                    },
                );
                self.gpu.queue.submit([enc.finish()]);
                let slice = self
                    .readback
                    .slice(..u64::from(self.pitch) * u64::from(rows));
                slice.map_async(wgpu::MapMode::Read, |_| {});
                self.gpu
                    .wait()
                    .map_err(|e| io::Error::other(format!("GPU readback: {e}")))?;
                {
                    let data = slice
                        .get_mapped_range()
                        .map_err(|e| io::Error::other(format!("GPU readback: {e}")))?;
                    for (y, src) in data.chunks_exact(self.pitch as usize).enumerate() {
                        let dst = &mut band[y * row + x0 as usize * 3..][..cols as usize * 3];
                        for (d, s) in dst.chunks_exact_mut(3).zip(src.chunks_exact(4)) {
                            d.copy_from_slice(&s[..3]);
                        }
                    }
                }
                self.readback.unmap();
                x0 += cols;
            }
            sink(&band[..rows as usize * row])?;
            y0 += rows;
        }
        self.text.end_frame();
        Ok(PageStats {
            missing: failed.len(),
            width: pw,
            height: ph,
        })
    }

    /// Page `index` at `dpi` as RGBA8 (the dialog's preview; see [`preview_dpi`]).
    pub fn render_rgba(
        &mut self,
        doc: &Document,
        index: usize,
        dpi: f64,
        source: &dyn ImageSource,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(u32, u32, Vec<u8>), RenderError> {
        let (w, h) = doc.setup.size_px_at(dpi);
        let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
        self.render_page(
            doc,
            index,
            dpi,
            source,
            &mut |rows| {
                for p in rows.chunks_exact(3) {
                    rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
                }
                Ok(())
            },
            cancelled,
        )?;
        Ok((w, h, rgba))
    }

    /// Writes `doc` at its resolution as a PDF to `out`, calling `progress(page)` after each
    /// page.
    pub fn write_pdf<W: io::Write>(
        &mut self,
        doc: &Document,
        meta: &crate::pdf::Meta,
        out: W,
        source: &dyn ImageSource,
        progress: &mut dyn FnMut(usize),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(W, crate::pdf::Stats, usize), RenderError> {
        let mut pdf = crate::pdf::PdfWriter::new(out, meta)?;
        let dpi = f64::from(doc.setup.dpi.clamp(36, 1200));
        let (pw, ph) = doc.setup.size_px_at(dpi);
        let mut missing = 0;
        for i in 0..doc.pages.len() {
            pdf.begin_page(doc.setup.size_pt(), pw, ph)?;
            let st = self.render_page(
                doc,
                i,
                dpi,
                source,
                &mut |rows| pdf.write_rows(rows),
                cancelled,
            )?;
            missing += st.missing;
            pdf.end_page()?;
            progress(i + 1);
        }
        let (out, stats) = pdf.finish()?;
        Ok((out, stats, missing))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Caption, Item, Kind, PageSetup, layout};
    use specular_i18n::{FSI, PDI, RLM, isolates_balanced};

    /// A contact sheet of Hebrew and Arabic file names, two too long for their cell, under
    /// a long Hebrew title.
    fn rtl_sheet(rtl: bool) -> crate::Document {
        let names = [
            "צילום משפחתי ארוך מאוד מהחופשה בים המלח 0005.jpg",
            "صورة العائلة على شاطئ البحر في الصيف الماضي.png",
            "חוף.jpg",
            "IMG_0004 (עותק).jpg",
        ];
        let items: Vec<Item> = names
            .iter()
            .map(|n| Item {
                size: (300, 200),
                name: (*n).to_owned(),
                date: Some("10 באוק׳ 2026".into()),
                scale: None,
            })
            .collect();
        let kind = Kind::Sheet {
            cols: 4,
            rows: 5,
            caption: Caption::NameDate,
            header: true,
        };
        // "Summer holiday 2026 / Dead Sea, chosen photos of every day and night of the trip".
        let title = "חופשת הקיץ 2026 / ים המלח, צילומים נבחרים מכל הימים והלילות של הטיול, \
                     עם המשפחה, החברים והשכנים מהבניין הישן ברחוב הרצל בתל אביב";
        let doc = layout(PageSetup::default(), kind, &items, title, &|n, of| {
            format!("עמוד {n} מתוך {of}")
        });
        if rtl { doc.right_to_left() } else { doc }
    }

    #[test]
    fn right_to_left_labels_mirror_the_header_and_cut_inside_isolates() {
        let mut text = Text::bundled_only();
        let doc = rtl_sheet(true);
        let page = &doc.pages[0];
        let ls = labels(&mut text, page, true);
        for l in &ls {
            assert!(isolates_balanced(&l.text), "{:?}", l.text);
        }
        // The long Hebrew and Arabic names are cut in the middle, inside their isolate,
        // and keep their extension.
        for (i, ext) in [(0, ".jpg"), (1, ".png")] {
            let cap = &ls[i * 2].text;
            assert!(cap.starts_with(&format!("{RLM}{FSI}")), "{cap:?}");
            assert!(cap.ends_with(&format!("{ext}{PDI}")), "{cap:?}");
            assert_eq!(cap.matches('…').count(), 1, "{cap:?}");
            assert_eq!(cap.matches(FSI).count(), 1, "{cap:?}");
        }
        // The date line is not a name: left as it is.
        assert_eq!(ls[1].text, "10 באוק׳ 2026");
        // Header: the page number at the left, the title at the right, cut to fit.
        let h = page.header.as_ref().unwrap();
        let (x, w) = (mm_to_pt(h.rect.x) as f32, mm_to_pt(h.rect.w) as f32);
        let n = ls.len();
        let (num, title) = (&ls[n - 2], &ls[n - 1]);
        assert_eq!(num.text, "עמוד 1 מתוך 1");
        assert!(matches!(num.align, Align::Start) && num.x == x);
        assert!(matches!(title.align, Align::End));
        assert!(
            (title.x + title.w - (x + w)).abs() < 1e-3,
            "the title ends at the right"
        );
        assert!(title.text.contains('…') && title.text.starts_with(RLM));
        assert!(text.measure(&title.text, title.style) <= title.w);

        // Left to right: the header as before, names not wrapped.
        let doc = rtl_sheet(false);
        let ls = labels(&mut text, &doc.pages[0], false);
        let (num, title) = (&ls[n - 2], &ls[n - 1]);
        assert!(matches!(num.align, Align::End) && matches!(title.align, Align::Start));
        assert_eq!(title.x, x);
        assert!(!ls[4].text.contains(FSI), "{:?}", ls[4].text);
    }
}
