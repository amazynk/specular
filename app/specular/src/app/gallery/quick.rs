//! Quick look and compare in a channel view (DESIGN.md §6.2). The thumbnails behind quick
//! look and compare (512 px at most) only show the image as it is; a channel, luminance or
//! alpha view needs the decoded image and the viewer's image shader. So while one is on, each
//! image shown (quick look's one, compare's 2–4) is opened like the viewer opens it, rendered
//! with the viewer's [`specular_render::ImageRenderer`] (the same options as the scene:
//! [`inspect::apply_backdrop`]) into a texture the size it takes on screen, and that texture is
//! drawn instead of the thumbnail. Until an image's decode is in, its thumbnail shows.
//!
//! The channel choice is the viewer's (one setting, `Shift+1…5`), so in compare it applies to
//! every image at once, as zoom and pan do; an image without alpha shows the normal view under
//! an alpha view, as in the viewer (`ChannelView::effective`).
//!
//! The similar-shots review (`similar.rs`) shows its large images the same way while they
//! are zoomed past fit or in a channel view. Zoomed, a pane is a window onto the image at
//! the review's shared zoom and centre, and the detail tiles it needs are region-decoded as
//! the viewer's are (in jobs of their own: the viewer keeps only its current image's), so the
//! 100 % compare zoom shows the real pixels. The 512 px thumbnail stands in until the image's
//! overview is on the GPU, and the overview until its tiles land.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use specular_core::{Orientation, Priority};
use specular_gfx::{Backdrop, Gpu, Rect, TexId, UiRenderer, wgpu};
use specular_inspect::ChannelView;
use specular_log::{TILES, trace};
use specular_render::{GpuImage, ImageRenderer, PxRect, RenderOptions, View, tiles};

use super::super::{App, Gfx, MAX_TILE_JOBS, TILE_BATCH, TileJob, inspect};
use super::Gallery;
use crate::loader::ImageId;

/// Quick look's image area in a view of `vw` × `vh` logical px (the caption goes below).
/// `cap`: the caption's height ([`Gallery::quick_caption_h`]), below the image.
pub(super) fn quick_area(vw: f32, vh: f32, cap: f32) -> Rect {
    Rect::new(24.0, 24.0, vw - 48.0, vh - 42.0 - cap)
}

/// Compare's `n` cells in a view of `vw` × `vh` logical px: side by side, or a 2×2 grid for
/// four on a narrow window. Each is the whole cell (the focus ring goes round it) and the
/// image area at its top, above a caption of height `cap`.
pub(super) fn compare_cells(vw: f32, vh: f32, n: usize, cap: f32) -> Vec<(Rect, Rect)> {
    if n == 0 {
        return Vec::new();
    }
    let (cols, rows) = if n == 4 && vw < vh * 2.0 {
        (2, 2)
    } else {
        (n, 1)
    };
    let pad = 16.0;
    let cw = (vw - pad * (cols + 1) as f32) / cols as f32;
    let ch = (vh - pad * (rows + 1) as f32) / rows as f32;
    (0..n)
        .map(|k| {
            let cell = Rect::new(
                pad + (k % cols) as f32 * (cw + pad),
                pad + (k / cols) as f32 * (ch + pad),
                cw,
                ch,
            );
            (cell, Rect::new(cell.x, cell.y, cell.w, cell.h - cap - 10.0))
        })
        .collect()
}

/// A display-space image of `dw` × `dh` fitted into `area` (logical px), never larger than
/// its own pixels (a small image stays sharp, centred), and snapped to whole physical
/// pixels: the rectangle in logical px and its size in physical px.
fn fit_snapped(dw: f32, dh: f32, area: Rect, scale: f32) -> (Rect, (u32, u32)) {
    fit_grown(dw, dh, area, scale, 1.0)
}

/// How far fitting a display-space image of `dw` × `dh` into `area` would enlarge it past
/// its own pixels (1: not at all, the image is at least as large as `area`).
pub(super) fn enlargement(dw: f32, dh: f32, area: Rect, scale: f32) -> f32 {
    let s = (area.w / dw.max(1.0)).min(area.h / dh.max(1.0)) * scale.max(0.01);
    if s.is_finite() { s.max(1.0) } else { 1.0 }
}

/// [`fit_snapped`] at `g` times its own pixels at most (a small image zoomed in by hand
/// grows until it fills `area`).
fn fit_grown(dw: f32, dh: f32, area: Rect, scale: f32, g: f32) -> (Rect, (u32, u32)) {
    let s = (area.w / dw.max(1.0))
        .min(area.h / dh.max(1.0))
        .min(g.max(1.0) / scale.max(0.01));
    let r = area.centered(dw * s, dh * s);
    let (x, y) = ((r.x * scale).round(), (r.y * scale).round());
    let w = (r.w * scale).round().max(1.0);
    let h = (r.h * scale).round().max(1.0);
    (
        Rect::new(x / scale, y / scale, w / scale, h / scale),
        (w as u32, h as u32),
    )
}

/// One image rendered through the image renderer this frame.
#[derive(Clone, Debug)]
pub(in crate::app) struct QuickPane {
    path: PathBuf,
    id: ImageId,
    stored: (u32, u32),
    orient: Orientation,
    /// Where the image goes (logical px, on physical pixels) and its size in physical px.
    rect: Rect,
    px: (u32, u32),
    /// The view drawn and the options it is drawn with.
    pub(super) channel: ChannelView,
    pub(super) opts: RenderOptions,
    /// Zoom over fit (1: the whole image) about `at`, a normalised display-space point
    /// (held far enough from the edges that the image covers the pane).
    pub(super) zoom: f32,
    pub(super) at: (f32, f32),
    /// Compare's overlay and difference view: the display size of the frame every image is
    /// placed in, top-left aligned at one scale (`zoom` and `at` are then the frame's). None:
    /// the image is its own frame.
    pub(super) frame: Option<(f32, f32)>,
    /// Where the image sits in the frame (compare's alignment; top-left by default).
    pub(super) place: specular_render::Place,
    /// The area the pane is fitted into (logical px) and physical px per logical px.
    area: Rect,
    scale: f32,
    /// How far the pane grows with the zoom ([`QuickPane::zoomed`]): 1 keeps it at fit.
    grow: f32,
}

impl QuickPane {
    /// The image `stored` px large (before `orient`) fitted into `area` at `scale` physical
    /// px per logical px, in the normal view until `channel` and `opts` are set.
    pub(super) fn new(
        path: PathBuf,
        id: ImageId,
        stored: (u32, u32),
        orient: Orientation,
        area: Rect,
        scale: f32,
    ) -> QuickPane {
        let (dw, dh) = if orient.swaps_axes() {
            (stored.1, stored.0)
        } else {
            stored
        };
        let (rect, px) = fit_snapped(dw as f32, dh as f32, area, scale);
        QuickPane {
            path,
            id,
            stored,
            orient,
            rect,
            px,
            channel: ChannelView::Normal,
            opts: RenderOptions::default(),
            zoom: 1.0,
            at: (0.5, 0.5),
            frame: None,
            place: specular_render::Place::IDENTITY,
            area,
            scale,
            grow: 1.0,
        }
    }

    /// The pane placed in `frame` (a display size at least the image's) fitted into `area`.
    pub(super) fn framed(mut self, frame: (f32, f32), area: Rect, scale: f32) -> QuickPane {
        let (rect, px) = fit_snapped(frame.0, frame.1, area, scale);
        (self.rect, self.px, self.frame) = (rect, px, Some(frame));
        (self.area, self.scale) = (area, scale);
        self
    }

    /// Compare's panes at `zoom`: a small image, at its own pixels at fit, grows with the
    /// zoom until it fills its area (where fitting would have enlarged it), and only then
    /// is cropped. A large image's pane stays at fit.
    pub(super) fn zoomed(mut self, zoom: f32) -> QuickPane {
        let (dw, dh) = self.frame.unwrap_or_else(|| self.display_size());
        self.grow = enlargement(dw, dh, self.area, self.scale);
        self.zoom = zoom;
        if self.grow > 1.0 {
            (self.rect, self.px) = fit_grown(dw, dh, self.area, self.scale, self.growth());
        }
        self
    }

    /// How far the pane has grown past fit at its zoom (1 at fit).
    fn growth(&self) -> f32 {
        self.zoom.max(1.0).min(self.grow.max(1.0))
    }

    /// The zoom past what the pane shows whole: how much of the frame is cropped away.
    fn crop_zoom(&self) -> f32 {
        self.zoom.max(1.0) / self.growth()
    }

    /// The image's display size (after its orientation).
    pub(super) fn display_size(&self) -> (f32, f32) {
        if self.orient.swaps_axes() {
            (self.stored.1 as f32, self.stored.0 as f32)
        } else {
            (self.stored.0 as f32, self.stored.1 as f32)
        }
    }

    /// Where the pane goes (logical px) and its size in physical px.
    pub(super) fn placed(&self) -> (Rect, (u32, u32)) {
        (self.rect, self.px)
    }

    /// The image as compare's metrics take it.
    pub(super) fn metric_image(&self) -> specular_render::MetricImage {
        specular_render::MetricImage {
            id: self.id,
            stored: self.stored,
            orient: self.orient,
            place: self.place,
        }
    }

    /// The frame's centre in view, held far enough from the edges that the frame covers
    /// the pane.
    fn centre(&self) -> (f32, f32) {
        let half = 0.5 / self.crop_zoom();
        (
            self.at.0.clamp(half, 1.0 - half),
            self.at.1.clamp(half, 1.0 - half),
        )
    }

    /// The view the pane is rendered with, into a target of its own size: the frame's
    /// centre in the middle, the image where its place puts it.
    pub(super) fn view(&self) -> View {
        let (pw, ph) = (self.px.0 as f32, self.px.1 as f32);
        let (dw, dh) = self.frame.unwrap_or_else(|| self.display_size());
        let z = self.crop_zoom();
        let c = self.centre();
        self.place.view(
            self.stored,
            self.orient,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: pw,
                h: ph,
            },
            (pw / dw.max(1.0)).min(ph / dh.max(1.0)) * z,
            (c.0 * dw, c.1 * dh),
        )
    }

    /// The frame rectangle (x0, y0, x1, y1) in view.
    pub(super) fn visible_frame(&self) -> (f32, f32, f32, f32) {
        let (dw, dh) = self.frame.unwrap_or_else(|| self.display_size());
        let half = 0.5 / self.crop_zoom();
        let c = self.centre();
        (
            (c.0 - half) * dw,
            (c.1 - half) * dh,
            (c.0 + half) * dw,
            (c.1 + half) * dh,
        )
    }
}

/// What quick look or compare renders through the image renderer this frame: the images
/// whose decode is in (the others show their thumbnails meanwhile).
#[derive(Debug, Default)]
pub(in crate::app) struct QuickPlan {
    pub(super) panes: Vec<QuickPane>,
    /// Compare's difference view: one per image against the first.
    pub(super) diffs: Vec<DiffPane>,
}

/// One difference of compare's difference view: panes `a` (the reference) and `b` of the
/// plan (rendered at the same size) in false colour, drawn at `rect`.
#[derive(Debug)]
pub(in crate::app) struct DiffPane {
    pub(super) a: usize,
    pub(super) b: usize,
    /// The compared image's place in the comparison (1…3).
    pub(super) image: usize,
    pub(super) rect: Rect,
    pub(super) px: (u32, u32),
    pub(super) params: specular_render::DiffParams,
}

/// A texture a channel view is rendered into, registered with the UI renderer.
pub(super) struct QuickTarget {
    tex: wgpu::Texture,
    view: wgpu::TextureView,
    pub(super) id: TexId,
}

impl QuickTarget {
    /// The texture (compare's probe copies texels from it).
    pub(super) fn texture(&self) -> &wgpu::Texture {
        &self.tex
    }
}

impl std::fmt::Debug for QuickTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuickTarget")
            .field("size", &self.tex.size())
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// A render target of `px` physical px.
fn target_texture(gpu: &Gpu, px: (u32, u32)) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("quick look"),
        size: wgpu::Extent3d {
            width: px.0,
            height: px.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: specular_gfx::HDR,
        // Copied from by compare's pixel probe.
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    (tex, view)
}

impl App {
    /// Before the frame: the images quick look or compare shows in a channel view through
    /// the image renderer (`size`: the window in logical px). Opens each image for it, and
    /// puts its overview on the GPU once decoded.
    pub(in crate::app) fn quick_plan(
        &mut self,
        size: (f32, f32),
        scale: f32,
        backdrop: Backdrop,
    ) -> Option<QuickPlan> {
        if !self.gallery.active {
            self.review_tiles(&[]);
            return None;
        }
        if self.gallery.similar.is_open() {
            return self.review_plan(size, scale, backdrop);
        }
        if self.gallery.compare.is_some() {
            return self.compare_plan(size, scale, backdrop);
        }
        self.review_tiles(&[]);
        let ga = &self.gallery;
        let cap = ga.quick_caption_h();
        let q = ga.quick.clone()?;
        let area = quick_area(size.0, size.1, cap);
        // The choice, before falling back on images without alpha.
        if self.chrome.channel_view(true) == ChannelView::Normal {
            return None;
        }
        let panes: Vec<QuickPane> = self
            .quick_pane(q, area, scale, backdrop, false)
            .into_iter()
            .collect();
        (!panes.is_empty()).then_some(QuickPlan {
            panes,
            diffs: Vec::new(),
        })
    }

    /// [`App::quick_plan`] for the similar-shots review: its large images through the image
    /// renderer while they are zoomed past fit or in a channel view, with their tiles.
    fn review_plan(
        &mut self,
        size: (f32, f32),
        scale: f32,
        backdrop: Backdrop,
    ) -> Option<QuickPlan> {
        let normal = self.chrome.channel_view(true) == ChannelView::Normal;
        let rp = self.gallery.similar.review_panes(size.0, size.1);
        let Some(rp) = rp.filter(|rp| rp.zoom > 1.0 || !normal) else {
            self.review_tiles(&[]);
            return None;
        };
        let zoomed = rp.zoom > 1.0;
        let mut panes = Vec::new();
        for (sig, main, cell) in rp.members {
            // A RAW + JPEG pair shows its JPEG (what its thumbnail shows) unless only its main
            // file is in the viewer's list.
            let only_main = self.path_index.contains(&self.entries, &main)
                && !self.path_index.contains(&self.entries, &sig);
            let path = if only_main { main } else { sig };
            if let Some(mut p) = self.quick_pane(path, cell, scale, backdrop, zoomed) {
                p.zoom = rp.zoom;
                p.at = rp.at;
                panes.push(p);
            }
        }
        self.review_tiles(&panes);
        (!panes.is_empty()).then_some(QuickPlan {
            panes,
            diffs: Vec::new(),
        })
    }

    /// Region decodes for the zoomed review panes' missing detail tiles, as
    /// `App::request_tiles` makes for the viewer's image; jobs nothing wants any more are
    /// cancelled (with no panes, all of them).
    pub(super) fn review_tiles(&mut self, panes: &[QuickPane]) {
        let zoomed: Vec<&QuickPane> = panes.iter().filter(|p| p.zoom > 1.0).collect();
        if zoomed.is_empty() && self.gallery.similar.tile_jobs.is_empty() {
            return;
        }
        let Some(gfx) = self.gfx.as_mut() else { return };
        let mut wants = Vec::new();
        if !zoomed.is_empty() {
            // The viewer's request starts a frame of the tile LRU only for a current image;
            // what the panes touch from here on is kept this frame (the viewer is hidden).
            gfx.renderer.pool.next_frame();
        }
        for p in zoomed {
            let (Some(img), Some(g)) = (self.images.get(&p.id), self.gpu_images.get(p.id)) else {
                continue;
            };
            let Some(session) = img.session.clone() else {
                continue;
            };
            // While the first whole level decodes, regions would build the row index twice
            // (see `request_tiles`); the level is on screen soon anyway.
            if g.overview.is_none()
                || !session.regions_ready() && img.levels.iter().any(|&s| s < img.rank())
            {
                continue;
            }
            if let Some((level, missing)) = gfx.renderer.wanted_tiles_also(g, &p.view())
                // A whole level of this scale decoding would decode the same pixels.
                && !img.levels.contains(&(1u32 << level))
            {
                wants.push((p.id, session, (g.width, g.height), level, missing));
            }
        }
        let jobs = &mut self.gallery.similar.tile_jobs;
        let before = jobs.len();
        jobs.retain(|j| {
            let keep = wants.iter().any(|w| w.0 == j.id && w.3 == j.level);
            if !keep {
                j.handle.cancel();
            }
            keep
        });
        if jobs.len() != before {
            trace!(target: TILES, "review: cancelled {} region decodes", before - jobs.len());
        }
        // The same share of the RAM budget as the viewer's region decodes; one always runs.
        let ram = match self.settings.performance.ram_cache {
            0 => self.hw.ram_cache_budget(),
            mib => (mib as u64) << 20,
        };
        let mem_budget = (ram / 8).clamp(128 << 20, 256 << 20);
        let mut mem_used: u64 = jobs.iter().map(|j| j.mem).sum();
        // Pane by pane, so the first images sharpen first rather than all at once.
        'panes: for (id, session, (w, h), level, missing) in wants {
            let inflight: HashSet<(u32, u32)> = jobs
                .iter()
                .filter(|j| j.id == id && j.level == level)
                .flat_map(|j| j.tiles.iter().copied())
                .collect();
            let todo: Vec<(u32, u32)> = missing
                .into_iter()
                .filter(|t| {
                    !inflight.contains(t) && !self.tiles_failed.contains(&(id, level, t.0, t.1))
                })
                .collect();
            for batch in tiles::batches(&todo, TILE_BATCH) {
                if jobs.len() >= MAX_TILE_JOBS {
                    break 'panes;
                }
                let rect = tiles::decode_rect(w, h, level, &batch);
                let req = specular_decode::DecodeRequest {
                    scale: 1 << level,
                    region: Some(specular_decode::Rect {
                        x: rect.0,
                        y: rect.1,
                        w: rect.2,
                        h: rect.3,
                    }),
                    parallel: true,
                    prepare_regions: false,
                };
                let mem = session.decode_memory(&req);
                if !jobs.is_empty() && mem_used + mem > mem_budget {
                    break 'panes;
                }
                mem_used += mem;
                let handle =
                    self.loader
                        .decode_tiles(id, session.clone(), level, rect, batch.clone());
                jobs.push(TileJob {
                    id,
                    level,
                    tiles: batch,
                    handle,
                    mem,
                });
                trace!(target: TILES, "review: img={id} L{level} decode {} tiles ({} in flight)", jobs.last().map_or(0, |j| j.tiles.len()), jobs.len());
            }
        }
    }

    /// A region decode finished or failed: true if it was one of the review's, whose tiles go
    /// up whether or not the image is the viewer's current one.
    pub(in crate::app) fn review_tiles_done(
        &mut self,
        id: ImageId,
        level: u8,
        tiles: &[(u32, u32)],
    ) -> bool {
        let jobs = &mut self.gallery.similar.tile_jobs;
        let n = jobs.len();
        jobs.retain(|j| !(j.id == id && j.level == level && j.tiles == tiles));
        if jobs.len() != n {
            trace!(target: TILES, "review: img={id} L{level} done ({} in flight)", jobs.len());
        }
        jobs.len() != n
    }

    /// One image of [`App::quick_plan`] in `area`, once it is decoded and on the GPU and
    /// shows other than the normal view (an alpha view of an image without alpha does not;
    /// `zoomed`, the normal view does too).
    pub(super) fn quick_pane(
        &mut self,
        path: PathBuf,
        area: Rect,
        scale: f32,
        backdrop: Backdrop,
        zoomed: bool,
    ) -> Option<QuickPane> {
        self.gallery_viewer_open(&path, Priority::Visible);
        let id = *self.by_path.get(&path)?;
        let img = self.images.get(&id)?;
        let info = img.info.as_ref()?;
        let has_alpha = info.has_alpha;
        let normal = self.chrome.channel_view(has_alpha) == ChannelView::Normal;
        if normal && !zoomed || img.overview.is_none() {
            return None;
        }
        let stored = (info.width, info.height);
        let orient = img.orient.unwrap_or(info.orientation);
        // Only the current image and its neighbours go up when decoded; this one may be neither.
        self.upload(id);
        if !self.gpu_images.contains(id) {
            return None;
        }
        let mut pane = QuickPane::new(path, id, stored, orient, area, scale);
        // Transparent around the image; under it the thumbnails' backdrop, or the view's.
        pane.opts = RenderOptions {
            hq: true,
            canvas: [0.0; 4],
            ..RenderOptions::default()
        };
        pane.channel = inspect::apply_backdrop(
            &mut pane.opts,
            backdrop,
            scale,
            &self.chrome,
            &self.settings,
            has_alpha,
        );
        Some(pane)
    }

    /// For screenshots: every image compare shows in a channel view is on the GPU (or
    /// failed), and so is every image the similar-shots review shows zoomed or in a channel
    /// view, with its tiles in. True without either or in the normal view.
    pub(in crate::app) fn compare_channel_settled(&self) -> bool {
        let on_gpu = |p: &Path| {
            self.by_path
                .get(p)
                .and_then(|id| self.images.get(id))
                .is_some_and(|img| img.gpu_rank.is_some() || img.error.is_some())
        };
        let ga = &self.gallery;
        if ga.similar.is_open() {
            let normal = self.chrome.channel_view(true) == ChannelView::Normal;
            return match ga.similar.review_panes(ga.vw, ga.vh) {
                Some(rp) if rp.zoom > 1.0 || !normal => {
                    ga.similar.tile_jobs.is_empty()
                        && rp.members.iter().all(|(s, m, _)| on_gpu(s) || on_gpu(m))
                }
                _ => true,
            };
        }
        let Some(c) = &ga.compare else {
            return true;
        };
        let normal = self.chrome.channel_view(true) == ChannelView::Normal;
        if !ga.cmp.renders(normal) {
            return true;
        }
        ga.similar.tile_jobs.is_empty()
            && ga.cmp.rendered(c).iter().all(|p| on_gpu(p))
            && ga.cmp.measured()
    }
}

impl Gallery {
    /// Before drawing: makes the textures for `plan` (or lets go of them with none) and tells
    /// [`Gallery::draw`] to show them.
    pub(in crate::app) fn prepare_quick(&mut self, gfx: &mut Gfx, plan: Option<&QuickPlan>) {
        self.prepare_panes(&gfx.gpu, &mut gfx.ui, plan);
    }

    /// [`Gallery::prepare_quick`] with a given GPU and UI renderer.
    pub(super) fn prepare_panes(
        &mut self,
        gpu: &Gpu,
        ui: &mut UiRenderer,
        plan: Option<&QuickPlan>,
    ) {
        let panes = plan.map_or(&[][..], |p| &p.panes[..]);
        while self.quick_targets.len() > panes.len() {
            if let Some(t) = self.quick_targets.pop() {
                ui.unregister_texture(t.id);
            }
        }
        for (k, pane) in panes.iter().enumerate() {
            let fits = self
                .quick_targets
                .get(k)
                .is_some_and(|t| (t.tex.width(), t.tex.height()) == pane.px);
            if fits {
                continue;
            }
            let (tex, view) = target_texture(gpu, pane.px);
            match self.quick_targets.get_mut(k) {
                Some(old) => {
                    ui.update_texture(gpu, old.id, &view);
                    (old.tex, old.view) = (tex, view);
                }
                None => {
                    let id = ui.register_texture(gpu, &view);
                    self.quick_targets.push(QuickTarget { tex, view, id });
                }
            }
        }
        self.quick_tex = panes
            .iter()
            .zip(&self.quick_targets)
            .map(|(p, t)| (p.path.clone(), t.id, p.rect, p.channel))
            .collect();
        let diffs = plan.map_or(&[][..], |p| &p.diffs[..]);
        while self.diff_targets.len() > diffs.len() {
            if let Some(t) = self.diff_targets.pop() {
                ui.unregister_texture(t.id);
            }
        }
        for (k, d) in diffs.iter().enumerate() {
            let fits = self
                .diff_targets
                .get(k)
                .is_some_and(|t| (t.tex.width(), t.tex.height()) == d.px);
            if fits {
                continue;
            }
            let (tex, view) = target_texture(gpu, d.px);
            match self.diff_targets.get_mut(k) {
                Some(old) => {
                    ui.update_texture(gpu, old.id, &view);
                    (old.tex, old.view) = (tex, view);
                }
                None => {
                    let id = ui.register_texture(gpu, &view);
                    self.diff_targets.push(QuickTarget { tex, view, id });
                }
            }
        }
        self.diff_tex = diffs
            .iter()
            .zip(&self.diff_targets)
            .map(|(d, t)| (d.image, t.id, d.rect))
            .collect();
    }

    /// Compare's difference of image `k` (1…3) against the first this frame: its texture and
    /// where it goes.
    pub(super) fn diff_tex(&self, k: usize) -> Option<(TexId, Rect)> {
        self.diff_tex.iter().find(|t| t.0 == k).map(|t| (t.1, t.2))
    }

    /// The channel view rendered for `path` this frame: its texture, where it goes and the
    /// view in it.
    pub(super) fn pane_tex(&self, path: &Path) -> Option<(TexId, Rect, ChannelView)> {
        self.quick_tex
            .iter()
            .find(|t| t.0 == path)
            .map(|t| (t.1, t.2, t.3))
    }

    /// The channel view on screen in quick look or compare, for screen readers: the first
    /// image's other than the normal view (an image without alpha shows the normal view
    /// under an alpha view), else the normal view.
    pub(super) fn channel_shown(&self) -> ChannelView {
        self.quick_tex
            .iter()
            .map(|t| t.3)
            .find(|c| *c != ChannelView::Normal)
            .unwrap_or(ChannelView::Normal)
    }
}

/// Renders `plan` into its textures (after [`Gallery::prepare_quick`], before the UI pass
/// that draws them).
pub(in crate::app) fn render_quick(
    gallery: &Gallery,
    gfx: &mut Gfx,
    images: &mut specular_render::GpuImageCache,
    enc: &mut wgpu::CommandEncoder,
    plan: &QuickPlan,
) {
    let mut drawn = vec![false; plan.panes.len()];
    for ((pane, t), drawn) in plan
        .panes
        .iter()
        .zip(&gallery.quick_targets)
        .zip(&mut drawn)
    {
        if let Some(img) = images.get(pane.id) {
            render_pane(&mut gfx.renderer, &gfx.gpu, enc, t, img, pane);
            *drawn = true;
        }
    }
    if let Some(c) = gallery.cmp.comparer.as_ref() {
        render_diffs(
            c,
            &gfx.gpu,
            enc,
            plan,
            &gallery.quick_targets,
            &gallery.diff_targets,
            &drawn,
        );
    }
}

/// Draws compare's differences of `plan` from its rendered panes (`drawn`).
pub(super) fn render_diffs(
    c: &specular_render::Comparer,
    gpu: &Gpu,
    enc: &mut wgpu::CommandEncoder,
    plan: &QuickPlan,
    panes: &[QuickTarget],
    targets: &[QuickTarget],
    drawn: &[bool],
) {
    for (d, t) in plan.diffs.iter().zip(targets) {
        if let (Some(a), Some(b)) = (panes.get(d.a), panes.get(d.b))
            && drawn.get(d.a) == Some(&true)
            && drawn.get(d.b) == Some(&true)
        {
            c.draw_diff(gpu, enc, &a.view, &b.view, &t.view, &d.params);
        }
    }
}

/// Renders one pane's image into its texture `t`.
pub(super) fn render_pane(
    renderer: &mut ImageRenderer,
    gpu: &Gpu,
    enc: &mut wgpu::CommandEncoder,
    t: &QuickTarget,
    img: &GpuImage,
    pane: &QuickPane,
) {
    renderer.render(
        gpu,
        enc,
        &t.view,
        pane.px,
        Some(img),
        &pane.view(),
        &pane.opts,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_look_fits_on_whole_pixels() {
        let area = quick_area(1600.0 / 1.25, 1000.0 / 1.25, 30.0);
        let (r, px) = fit_snapped(4000.0, 3000.0, area, 1.25);
        // Height-bound: the area's height, the width from the aspect ratio.
        assert_eq!(px.1, ((1000.0 / 1.25 - 72.0) * 1.25_f32).round() as u32);
        assert!((px.0 as f32 / px.1 as f32 - 4.0 / 3.0).abs() < 0.01);
        for v in [r.x, r.y, r.w, r.h] {
            let p = v * 1.25;
            assert!((p - p.round()).abs() < 1e-3, "{v}");
        }
        // Centred in the area.
        assert!(((r.x - area.x) - (area.x + area.w - r.x - r.w)).abs() <= 1.0);
    }

    /// A small image is shown at its own pixels, centred, at any scale factor: never
    /// enlarged in physical pixels.
    #[test]
    fn quick_look_never_enlarges_a_small_image() {
        for scale in [1.0_f32, 1.25, 2.0] {
            let area = quick_area(1600.0 / scale, 1000.0 / scale, 30.0);
            let (r, px) = fit_snapped(200.0, 100.0, area, scale);
            assert_eq!(px, (200, 100), "at {scale}");
            assert!(((r.x - area.x) - (area.x + area.w - r.x - r.w)).abs() <= 1.0);
            assert!(((r.y - area.y) - (area.y + area.h - r.y - r.h)).abs() <= 1.0);
        }
    }

    /// Compare's pane of a small image: 100 % at fit, then it grows with the zoom until it
    /// fills its area (where fitting would have enlarged it), and only then crops; zoom over
    /// fit keeps its meaning. A large image's pane stays at fit and crops at once.
    #[test]
    fn small_compare_panes_grow_with_the_zoom() {
        let area = Rect::new(0.0, 0.0, 800.0, 600.0);
        let pane = |zoom: f32, stored, scale| {
            QuickPane::new(
                PathBuf::from("/a.png"),
                1,
                stored,
                Orientation(1),
                area,
                scale,
            )
            .zoomed(zoom)
        };
        assert_eq!(enlargement(200.0, 100.0, area, 1.0), 4.0);
        assert_eq!(enlargement(200.0, 100.0, area, 2.0), 8.0);
        assert_eq!(enlargement(4000.0, 3000.0, area, 1.0), 1.0);
        for (zoom, px, whole) in [
            (1.0, (200, 100), true),
            (2.0, (400, 200), true),
            (4.0, (800, 400), true),
            (8.0, (800, 400), false),
        ] {
            let p = pane(zoom, (200, 100), 1.0);
            assert_eq!(p.placed().1, px, "zoom {zoom}");
            let (x0, y0, x1, y1) = p.view().visible_display();
            let shows_all = x0 <= 0.5 && y0 <= 0.5 && x1 >= 199.5 && y1 >= 99.5;
            assert_eq!(shows_all, whole, "zoom {zoom}: {x0} {y0} {x1} {y1}");
            // One image pixel takes `zoom` physical px.
            let v = p.view();
            let a = v.display_to_screen((0.0, 0.0));
            let b = v.display_to_screen((10.0, 0.0));
            assert!(((b.0 - a.0) - 10.0 * zoom).abs() < 1e-2, "zoom {zoom}");
            let (cx0, _, cx1, _) = p.visible_frame();
            assert!(((cx1 - cx0) - 200.0 / (zoom / 4.0).max(1.0)).abs() < 1e-3);
        }
        // At 2x the same area holds twice the pixels: 100 % is a smaller logical rect.
        let p = pane(1.0, (200, 100), 2.0);
        assert_eq!(p.placed().1, (200, 100));
        assert_eq!(pane(8.0, (200, 100), 2.0).placed().1, (1600, 800));
        // A large image: its pane is the fit at any zoom.
        let fit = pane(1.0, (4000, 3000), 1.0).placed();
        assert_eq!(pane(3.0, (4000, 3000), 1.0).placed(), fit);
        assert_eq!(fit.1, (800, 600));
    }

    #[test]
    fn zoomed_review_panes_stay_covered() {
        let area = Rect::new(0.0, 0.0, 400.0, 300.0);
        let mut pane = QuickPane::new(
            PathBuf::from("/a.jpg"),
            1,
            (4000, 3000),
            Orientation(1),
            area,
            1.0,
        );
        // Fit: the whole image.
        let v = pane.view();
        let (x0, y0, x1, y1) = v.visible_display();
        assert!(x0 <= 0.5 && y0 <= 0.5 && x1 >= 3999.5 && y1 >= 2999.5);
        // Ten times fit (100 % for this image) about a corner: the corner's tenth, no edge.
        pane.zoom = 10.0;
        pane.at = (0.0, 0.0);
        let (x0, y0, x1, y1) = pane.view().visible_display();
        assert!(x0.abs() < 0.5 && y0.abs() < 0.5, "{x0} {y0}");
        assert!(
            (x1 - 400.0).abs() < 0.5 && (y1 - 300.0).abs() < 0.5,
            "{x1} {y1}"
        );
        // About the centre.
        pane.at = (0.5, 0.5);
        let (x0, _, x1, _) = pane.view().visible_display();
        assert!(
            (x0 - 1800.0).abs() < 0.5 && (x1 - 2200.0).abs() < 0.5,
            "{x0} {x1}"
        );
    }

    #[test]
    fn compare_cells_tile_the_window() {
        let (vw, vh, cap) = (1200.0, 800.0, 30.0);
        assert!(compare_cells(vw, vh, 0, cap).is_empty());
        for n in 2..=4 {
            let cells = compare_cells(vw, vh, n, cap);
            assert_eq!(cells.len(), n);
            // A narrow window puts four in a 2×2 grid; fewer go in one row.
            let rows = if n == 4 { 2 } else { 1 };
            let mut tops: Vec<f32> = cells.iter().map(|c| c.0.y).collect();
            tops.dedup();
            assert_eq!(tops.len(), rows, "{n}");
            for (k, (cell, img)) in cells.iter().enumerate() {
                // In the window; the image area at the top of its cell, above the caption.
                assert!(cell.x >= 0.0 && cell.x + cell.w <= vw && cell.y + cell.h <= vh);
                assert_eq!((img.x, img.y, img.w), (cell.x, cell.y, cell.w));
                assert_eq!(img.y + img.h, cell.y + cell.h - cap - 10.0);
                // Clear of the next one in its row.
                if let Some((next, _)) = cells.get(k + 1)
                    && next.y == cell.y
                {
                    assert!(next.x >= cell.x + cell.w);
                }
            }
        }
        // Four on a wide window: one row.
        let wide = compare_cells(2000.0, 600.0, 4, cap);
        assert!(wide.iter().all(|c| c.0.y == wide[0].0.y));
    }

    #[test]
    fn compare_panes_fit_their_cells_on_whole_pixels() {
        let scale = 1.5;
        for (k, (_, area)) in compare_cells(900.0, 600.0, 3, 30.0).iter().enumerate() {
            // A portrait image turned on its side by its orientation fits as landscape.
            let pane = QuickPane::new(
                PathBuf::from(format!("/{k}.jpg")),
                k as ImageId,
                (3000, 4000),
                Orientation(6),
                *area,
                scale,
            );
            assert!(pane.px.0 > pane.px.1, "{:?}", pane.px);
            let r = pane.rect;
            assert!(r.x >= area.x - 0.5 && r.x + r.w <= area.x + area.w + 0.5);
            assert!(r.y >= area.y - 0.5 && r.y + r.h <= area.y + area.h + 0.5);
            for v in [r.x, r.y, r.w, r.h] {
                let p = v * scale;
                assert!((p - p.round()).abs() < 1e-3, "{v}");
            }
            assert_eq!(pane.channel, ChannelView::Normal);
        }
    }
}
