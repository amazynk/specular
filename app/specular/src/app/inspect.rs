//! Inspection tools in the viewer (DESIGN.md §6.2): the clipping and focus-peaking overlay,
//! the histogram of what the window shows and the pixel inspector with its loupe.
//!
//! - The overlay and the loupe are part of the scene: [`InspectScene`] goes into the scene
//!   key, and [`draw_scene`] draws them right after the image.
//! - The histogram and the pixel probe are GPU readbacks that never stall a frame.
//!   [`App::tick_inspect`] runs on every event-loop turn: it records a request into its own
//!   encoder and submits it when what the window shows (or the pointer's pixel) changed, and
//!   polls the result on later turns, waking the loop every [`POLL`] while one is in flight
//!   but drawing a frame only when a panel's content changed. Idle tools cost nothing.
//! - The decoded (source) value of the pixel comes from the full-resolution overview when
//!   there is one, else from a small region decode on the loader pool once the pointer has
//!   rested on a pixel for [`SOURCE_DEBOUNCE`] (8-bit images whose decoder decodes regions
//!   cheaply).

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use specular_core::{CancelToken, Priority, Raster};
use specular_decode::{DecodeRequest, Rect};
use specular_gfx::Backdrop;
use specular_gfx::winit::event_loop::{ActiveEventLoop, ControlFlow};
use specular_input::Action;
use specular_inspect::{ChannelView, SourceValue, source_pixel};
use specular_log::{RENDER, debug, trace};
use specular_render::{
    GpuImage, HiLo, HistogramRequest, Inspector, InspectorBuild, OverlayOptions, PxRect,
    RenderOptions, View,
};
use specular_settings::Settings;
use specular_theme::{CHECKER_CELL, Color};
use specular_widgets::{Chrome, PixelInfo, SourceState};

use super::{App, Gfx, rect_contains, settings_color};
use crate::loader::{ImageId, OverviewKind};

/// How often an in-flight readback is polled.
const POLL: Duration = Duration::from_millis(2);
/// How long the pointer rests on a pixel before its source value is decoded.
const SOURCE_DEBOUNCE: Duration = Duration::from_millis(90);
/// Side of the block decoded around the inspected pixel (one MCU of 4:2:0 JPEG).
const SOURCE_BLOCK: u32 = 16;
/// Loupe magnification: logical px per image pixel (the widget's centre marker is this big).
const LOUPE_CELL: f32 = 9.0;
/// While the view moves (zoom animation, drag, fling), the histogram is asked for at most
/// this often; the one for the settled view follows as soon as it stops.
const HIST_MOVING_EVERY: Duration = Duration::from_millis(100);
/// The inspector's pipelines are built in the background once the app has been idle with an
/// image up this long (or at once when a tool is switched on).
const PREWARM_AFTER: Duration = Duration::from_millis(500);
/// The pixel inspector's loupe asks for full-resolution tiles only when decoding them takes
/// at most this much memory: a JPEG region is ~1 MB, but a PNG without a row index decodes
/// the whole image for any region (1.9 GB and 7 s for 36000×18000), which hovering at fit
/// must not start.
const LOUPE_TILE_MEM: u64 = 64 << 20;

/// What the inspection tools add to the scene (part of its cache key).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct InspectScene {
    overlay: Option<OverlayOptions>,
    loupe: Option<PixelLoupe>,
}

impl InspectScene {
    /// The pixel inspector's loupe shows (it reads full-resolution tiles).
    pub(super) fn has_loupe(&self) -> bool {
        self.loupe.is_some()
    }
}

/// The pixel inspector's loupe: where (physical px) and what it shows.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PixelLoupe {
    rect: PxRect,
    /// Display-space centre (the middle of the inspected pixel).
    center: (f32, f32),
    scale: f32,
}

/// What a histogram shows; asked again when it changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HistKey {
    id: ImageId,
    rev: u64,
    view_rev: u64,
    tiles: usize,
    req: HistogramRequest,
}

/// The probed pixel and the data it was read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProbeKey {
    id: ImageId,
    rev: u64,
    tiles: usize,
    px: (u32, u32),
}

/// A finished source-value decode: the block around the pixel.
struct SourceBlock {
    id: ImageId,
    /// Request serial, to drop results of superseded requests.
    serial: u64,
    raster: Option<Arc<Raster>>,
}

pub(super) struct InspectState {
    inspector: Option<Inspector>,
    /// The inspector's pipelines compiling in the background (started once the first image
    /// is up, or by the first tool).
    build: Option<InspectorBuild>,
    /// When the last histogram was asked for, and how many were (for the log).
    hist_at: Option<Instant>,
    hist_requests: u64,
    /// Since when the app has been idle with an image up (for the prewarm).
    idle_since: Option<Instant>,
    /// Distinct histograms skipped while the view moved (for the log).
    hist_skipped: u64,
    hist_skipped_key: Option<HistKey>,
    hist_done: Option<HistKey>,
    probe_done: Option<ProbeKey>,
    in_flight: Option<(Option<HistKey>, Option<ProbeKey>)>,
    /// Image of the histogram shown (cleared when another image comes up).
    hist_id: Option<ImageId>,
    /// Image of the pixel shown.
    pixel_id: Option<ImageId>,
    /// Source decode: the pixel waiting for it and when it is due, the job in flight.
    source_due: Option<(ImageId, (u32, u32), Instant)>,
    source_job: Option<CancelToken>,
    source_serial: u64,
    /// The last decoded block (answers pixels inside it at once).
    block: Option<(ImageId, Arc<Raster>)>,
    tx: Sender<SourceBlock>,
    rx: Receiver<SourceBlock>,
    /// When the loop should wake for the tools (poll, debounce).
    wake: Option<Instant>,
    /// Readbacks completed, for the log.
    readbacks: u64,
    /// Screenshots: the pointer is held right of centre (the real one may be anywhere).
    shot_pointer: bool,
}

impl InspectState {
    /// The GPU device was replaced: the inspector is built again and its readings asked for
    /// again.
    pub(super) fn gpu_reset(&mut self) {
        self.inspector = None;
        self.build = None;
        self.in_flight = None;
        self.hist_done = None;
        self.probe_done = None;
    }
}

impl Default for InspectState {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        InspectState {
            inspector: None,
            build: None,
            hist_at: None,
            hist_requests: 0,
            idle_since: None,
            hist_skipped: 0,
            hist_skipped_key: None,
            hist_done: None,
            probe_done: None,
            in_flight: None,
            hist_id: None,
            pixel_id: None,
            source_due: None,
            source_job: None,
            source_serial: 0,
            block: None,
            tx,
            rx,
            wake: None,
            readbacks: 0,
            shot_pointer: false,
        }
    }
}

fn byte(v: i64) -> u8 {
    v.clamp(0, 255) as u8
}

/// The overlay options the settings ask for, when an overlay is on.
fn overlay_options(chrome: &Chrome, s: &Settings) -> Option<OverlayOptions> {
    let t = chrome.inspect.tools;
    if !t.clipping && !t.peaking {
        return None;
    }
    let i = &s.inspection;
    Some(OverlayOptions {
        clipping: t.clipping,
        peaking: t.peaking,
        clip_low: byte(i.clip_shadow),
        clip_high: byte(i.clip_highlight),
        peaking_threshold: specular_inspect::peaking_threshold(
            i.peaking_sensitivity.clamp(1, 100) as u32
        ),
        peaking_color: settings_color(i.peaking_color).to_linear_premul(),
        highlight_color: settings_color(i.highlight_color).to_linear_premul(),
        shadow_color: settings_color(i.shadow_color).to_linear_premul(),
    })
}

/// Light checkerboard of the "alpha over checkerboard" view: higher contrast than the usual
/// dark one, so soft alpha edges show against both squares.
const ALPHA_CHECKER: [Color; 2] = [Color::hex(0xFFFFFF), Color::hex(0xB4B4B4)];

/// Applies the channel view to the scene options: the shader mode, and for the checkerboard
/// and matte alpha views the backdrop behind the image. Returns the view drawn.
pub(super) fn apply_channel(
    opts: &mut RenderOptions,
    chrome: &Chrome,
    s: &Settings,
    has_alpha: bool,
) -> ChannelView {
    let view = chrome.channel_view(has_alpha);
    opts.channel = view;
    apply_display_aids(opts, chrome, s);
    match view {
        ChannelView::AlphaChecker => {
            opts.checker_a = ALPHA_CHECKER[0].to_linear_premul();
            opts.checker_b = ALPHA_CHECKER[1].to_linear_premul();
        }
        ChannelView::AlphaMatte => {
            let matte = settings_color(s.inspection.alpha_matte_color)
                .with_alpha(1.0)
                .to_linear_premul();
            opts.checker_a = matte;
            opts.checker_b = matte;
        }
        _ => {}
    }
    view
}

/// The colour-vision-safe display aids in a render's options (DESIGN.md §6.2): false colour,
/// the channel-colour preset and HiLo, marked in the clipping warning colours.
pub(super) fn apply_display_aids(opts: &mut RenderOptions, chrome: &Chrome, s: &Settings) {
    let tools = chrome.inspect.tools;
    opts.false_colour = chrome.inspect.false_colour();
    opts.channel_colours = tools.channel_colours;
    opts.hilo = tools.hilo.then(|| {
        let lin = |c: specular_settings::Color| {
            let l = settings_color(c).with_alpha(1.0).to_linear_premul();
            [l[0], l[1], l[2]]
        };
        HiLo {
            high: lin(s.inspection.highlight_color),
            low: lin(s.inspection.shadow_color),
        }
    });
}

/// The backdrop under transparency and the channel view in a render's options, for the
/// viewer's scene and for quick look in the gallery alike. `scale` is physical px per logical
/// px; `Backdrop::None` shows `opts.canvas`. Returns the channel view drawn.
pub(super) fn apply_backdrop(
    opts: &mut RenderOptions,
    backdrop: Backdrop,
    scale: f32,
    chrome: &Chrome,
    s: &Settings,
    has_alpha: bool,
) -> ChannelView {
    let canvas = opts.canvas;
    // Screen-fixed cells, the same size as the gallery's (physical px here).
    (opts.checker_a, opts.checker_b, opts.checker_cell) = match backdrop {
        Backdrop::Checker { a, b, cell } => (
            a.to_linear_premul(),
            b.to_linear_premul(),
            (cell * scale).round().max(1.0),
        ),
        Backdrop::Solid(c) => (c.to_linear_premul(), c.to_linear_premul(), 1.0e6),
        Backdrop::None => (canvas, canvas, 1.0e6),
    };
    let channel = apply_channel(opts, chrome, s, has_alpha);
    // The alpha-over-checkerboard view needs cells even when transparency is a solid colour.
    if channel == ChannelView::AlphaChecker {
        opts.checker_cell = (CHECKER_CELL * scale).round().max(1.0);
    }
    channel
}

fn histogram_request(s: &Settings) -> HistogramRequest {
    HistogramRequest {
        clip_low: byte(s.inspection.clip_shadow),
        clip_high: byte(s.inspection.clip_highlight),
    }
}

/// The stored pixel under window point `p` (physical px), if it is on the image.
fn stored_pixel(view: &View, p: (f32, f32)) -> Option<(u32, u32)> {
    if !rect_contains(view.viewport, p) {
        return None;
    }
    let s = view.stored_from_screen().apply(p);
    let (w, h) = view.stored_size();
    (s.0 >= 0.0 && s.1 >= 0.0 && s.0 < w as f32 && s.1 < h as f32)
        .then_some((s.0 as u32, s.1 as u32))
}

/// The block of [`SOURCE_BLOCK`] pixels holding `px`, clipped to a `w × h` image.
fn source_block(px: (u32, u32), w: u32, h: u32) -> Rect {
    let x = px.0 / SOURCE_BLOCK * SOURCE_BLOCK;
    let y = px.1 / SOURCE_BLOCK * SOURCE_BLOCK;
    Rect {
        x,
        y,
        w: SOURCE_BLOCK.min(w.saturating_sub(x)),
        h: SOURCE_BLOCK.min(h.saturating_sub(y)),
    }
}

/// The inspection part of the scene key for the frame being drawn (after the chrome, which
/// places the loupe). `ui` is physical px per logical px.
pub(super) fn scene(
    chrome: &Chrome,
    settings: &Settings,
    view: &View,
    pointer: Option<(f32, f32)>,
    image: bool,
    ui: f32,
) -> InspectScene {
    if !image {
        return InspectScene::default();
    }
    let loupe = chrome.pixel_loupe().and_then(|r| {
        let p = pointer?;
        stored_pixel(view, p)?;
        let d = view.screen_to_display(p);
        Some(PixelLoupe {
            rect: PxRect {
                x: (r.x * ui).round(),
                y: (r.y * ui).round(),
                w: (r.w * ui).round(),
                h: (r.h * ui).round(),
            },
            center: (d.0.floor() + 0.5, d.1.floor() + 0.5),
            scale: LOUPE_CELL * ui,
        })
    });
    InspectScene {
        overlay: overlay_options(chrome, settings),
        loupe,
    }
}

/// Draws the overlay and the loupe over the scene, which holds `img` drawn with `view` and
/// `opts`.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_scene(
    st: &mut InspectState,
    gfx: &mut Gfx,
    enc: &mut specular_gfx::wgpu::CommandEncoder,
    size: (u32, u32),
    img: &GpuImage,
    view: &View,
    opts: &RenderOptions,
    sc: &InspectScene,
) {
    // Until the inspector's pipelines are built (a few frames after the first tool comes on
    // at worst) the overlay is missing; its arrival redraws the scene.
    if let Some(o) = &sc.overlay
        && let Some(ins) = &st.inspector
    {
        ins.draw_overlay(
            &gfx.gpu,
            &gfx.renderer,
            enc,
            &gfx.comp.scene.view,
            size,
            img,
            view,
            opts,
            o,
        );
    }
    if let Some(l) = &sc.loupe {
        let lv = View::fixed(
            view.stored_size(),
            view.orientation(),
            l.rect,
            l.scale,
            l.center,
        );
        let lo = RenderOptions {
            hq: true,
            grid: None,
            // Pixels as squares: the loupe is for reading single values.
            nearest_from: 0.0,
            ..*opts
        };
        gfx.renderer.render_inset(
            &gfx.gpu,
            enc,
            &gfx.comp.scene.view,
            size,
            img,
            &lv,
            &lo,
            None,
        );
        // The clipping / peaking overlay over the loupe too, at its magnification.
        if let Some(o) = &sc.overlay
            && let Some(ins) = &st.inspector
        {
            ins.draw_overlay(
                &gfx.gpu,
                &gfx.renderer,
                enc,
                &gfx.comp.scene.view,
                size,
                img,
                &lv,
                &lo,
                o,
            );
        }
    }
}

impl App {
    /// The pixel inspector's loupe as a view, for asking full-resolution tiles for its area
    /// (it covers a few dozen image pixels, so at most a handful of level-0 tiles).
    pub(super) fn pixel_loupe_view(&self) -> Option<View> {
        let sc = scene(
            &self.chrome,
            &self.settings,
            &self.view,
            self.pointer,
            self.shown.is_some(),
            self.dl.scale,
        );
        let l = sc.loupe?;
        Some(View::fixed(
            self.view.stored_size(),
            self.view.orientation(),
            l.rect,
            l.scale,
            l.center,
        ))
    }

    /// The pixel loupe's missing tiles `(level, tiles)` if decoding them is cheap (see
    /// [`LOUPE_TILE_MEM`]).
    pub(super) fn pixel_loupe_tiles_cheap(
        session: &dyn specular_decode::Session,
        size: (u32, u32),
        want: &(u8, Vec<(u32, u32)>),
    ) -> bool {
        let (level, tiles) = want;
        if tiles.is_empty() {
            return true;
        }
        let r = specular_render::tiles::decode_rect(size.0, size.1, *level, tiles);
        let req = DecodeRequest {
            scale: 1 << level,
            region: Some(Rect {
                x: r.0,
                y: r.1,
                w: r.2,
                h: r.3,
            }),
            parallel: true,
            prepare_regions: false,
        };
        session.decode_memory(&req) <= LOUPE_TILE_MEM
    }

    /// Polls readbacks and source decodes, follows the pointer, and starts new requests.
    /// Runs on every event-loop turn; sets `redraw` only when a panel changed.
    pub(super) fn tick_inspect(&mut self) {
        let tools = self.chrome.inspect.tools;
        self.inspect.wake = None;
        if self.inspect.shot_pointer {
            let vp = self.view.viewport;
            self.pointer = Some((vp.x + vp.w * 0.62, vp.y + vp.h * 0.45));
        }
        self.poll_build(tools.clipping || tools.peaking || tools.histogram || tools.pixel);
        self.poll_readback();
        self.poll_source();
        // The image the tools look at: the current one, shown as a single image.
        let cur = self
            .current_id()
            .filter(|id| self.shown == Some(*id) && !self.spread_active());
        self.update_legend(cur);

        // Histogram panel.
        if !tools.histogram {
            self.inspect.hist_done = None;
            if self.chrome.inspect.histogram.take().is_some() {
                self.redraw = true;
            }
        } else if self.inspect.hist_id != cur {
            self.inspect.hist_id = cur;
            self.inspect.hist_done = None;
            if self.chrome.inspect.histogram.take().is_some() {
                self.redraw = true;
            }
        }

        // Pixel inspector: the pixel under the pointer.
        let px = if (tools.pixel || self.cursor_wants_pixel()) && self.chrome.modal().is_none() {
            cur.zip(self.pointer.and_then(|p| stored_pixel(&self.view, p)))
        } else {
            None
        };
        self.follow_pointer(px);

        self.request_readback(cur, px);

        let busy = self.inspect.inspector.as_ref().is_some_and(Inspector::busy);
        if busy {
            self.inspect.wake = Some(Instant::now() + POLL);
        }
        if let Some((_, _, due)) = self.inspect.source_due {
            self.inspect.wake = Some(self.inspect.wake.map_or(due, |w| w.min(due)));
        }
    }

    /// Keeps the colour bar's legend in step with the image on screen while false colour is
    /// on: medical mode's window in modality units, an auto-ranged image's range in its
    /// samples' own scale (once the GPU has found it), else the full range of the stored
    /// values (0‥255, or 0‥2^bits − 1).
    fn update_legend(&mut self, cur: Option<ImageId>) {
        let map = self.chrome.inspect.false_colour();
        let legend = map.zip(cur).and_then(|(map, id)| {
            let (lo, hi, unit) = match self.medical_legend() {
                Some(r) => r,
                None => {
                    let bits = self.images.get(&id)?.info.as_ref()?.bit_depth;
                    let g = self.gpu_images.get(id)?;
                    match (g.auto_range_bits(), g.auto_range()) {
                        (Some(b), Some([lo, hi])) => {
                            let top = f64::from((1u32 << b.min(16)) - 1);
                            let v = |c: u16| (f64::from(c) * top / 65535.0).round();
                            (v(lo), v(hi), String::new())
                        }
                        (Some(_), None) => {
                            // Still being read back.
                            self.inspect.wake = Some(Instant::now() + POLL);
                            return None;
                        }
                        (None, _) => {
                            let top = if bits > 8 && bits <= 16 {
                                f64::from((1u32 << bits) - 1)
                            } else {
                                255.0
                            };
                            (0.0, top, String::new())
                        }
                    }
                }
            };
            Some(specular_inspect::Legend { map, lo, hi, unit })
        });
        if legend != self.chrome.inspect.legend {
            self.chrome.inspect.legend = legend;
            self.redraw = true;
        }
    }

    /// Screenshots (`--show`): `clipping`, `peaking`, `histogram`, `pixel-inspector` (with
    /// the pointer right of centre), and the channel views `channel-red`, `-green`, `-blue`,
    /// `-alpha`, `-alpha-checker`, `-alpha-matte`, `-luma`, and the display aids
    /// `false-colour`, `hilo`, `channel-colours`, `colour-vision-protanopia` (`-deuteranopia`,
    /// `-tritanopia`, `-achromatopsia`).
    pub(super) fn inspect_show(&mut self, s: &str) {
        let (a, presses) = match s {
            "channel-red" => (Action::ChannelRed, 1),
            "channel-green" => (Action::ChannelGreen, 1),
            "channel-blue" => (Action::ChannelBlue, 1),
            "channel-alpha" => (Action::ChannelAlpha, 1),
            "channel-alpha-checker" => (Action::ChannelAlpha, 2),
            "channel-alpha-matte" => (Action::ChannelAlpha, 3),
            "channel-luma" => (Action::ChannelLuma, 1),
            "false-colour" => (Action::ToggleFalseColour, 1),
            "hilo" => (Action::ToggleHiLo, 1),
            "channel-colours" => (Action::NextChannelColours, 1),
            "colour-vision-protanopia" => (Action::SimulateProtanopia, 1),
            "colour-vision-deuteranopia" => (Action::SimulateDeuteranopia, 1),
            "colour-vision-tritanopia" => (Action::SimulateTritanopia, 1),
            "colour-vision-achromatopsia" => (Action::SimulateAchromatopsia, 1),
            _ => (Action::Back, 0),
        };
        for _ in 0..presses {
            self.chrome.handle_action(a);
        }
        if presses > 0 {
            return;
        }
        let a = match s {
            "clipping" => Action::ToggleClipping,
            "peaking" => Action::ToggleFocusPeaking,
            "histogram" => Action::ToggleHistogram,
            "pixel-inspector" => {
                self.inspect.shot_pointer = true;
                Action::TogglePixelInspector
            }
            _ => return,
        };
        self.chrome.handle_action(a);
    }

    /// Picks up the inspector once its background build is done, and starts the build once
    /// the first image is up and its tiles are in (or as soon as a tool is on).
    fn poll_build(&mut self, wanted: bool) {
        if self.inspect.inspector.is_some() {
            return;
        }
        if let Some(b) = self.inspect.build.as_mut() {
            if let Some(ins) = b.try_take() {
                self.inspect.inspector = Some(ins);
                self.inspect.build = None;
                // A scene drawn without the overlay while it was building.
                if self.chrome.inspect.tools.clipping || self.chrome.inspect.tools.peaking {
                    self.scene = None;
                    self.redraw = true;
                }
            } else if b.failed() {
                self.inspect.build = None;
                debug!(target: RENDER, "inspect: background build failed, building here");
                if let Some(gfx) = self.gfx.as_ref() {
                    self.inspect.inspector = Some(Inspector::new(&gfx.gpu, &gfx.renderer));
                }
            }
            return;
        }
        // Unasked, wait until the first image has been up and its tiles in for a moment, so
        // the compile (30–110 ms on one core) stays out of the way of start-up work.
        let idle = self.shown.is_some() && self.tile_jobs.is_empty();
        if !wanted {
            if !idle {
                self.inspect.idle_since = None;
                return;
            }
            let now = Instant::now();
            let due = *self.inspect.idle_since.get_or_insert(now) + PREWARM_AFTER;
            if now < due {
                self.inspect.wake = Some(self.inspect.wake.map_or(due, |w| w.min(due)));
                return;
            }
        }
        let Some(gfx) = self.gfx.as_ref() else { return };
        let wake = self.loader.waker();
        self.inspect.build = Some(Inspector::spawn(&gfx.gpu, &gfx.renderer, move || wake()));
    }

    /// Lets the loop wake for the tools' polling and debounce without drawing a frame.
    pub(super) fn inspect_control_flow(&self, el: &ActiveEventLoop) {
        let Some(t) = self.inspect.wake else { return };
        match el.control_flow() {
            ControlFlow::Wait => el.set_control_flow(ControlFlow::WaitUntil(t)),
            ControlFlow::WaitUntil(u) if t < u => el.set_control_flow(ControlFlow::WaitUntil(t)),
            _ => {}
        }
    }

    fn poll_readback(&mut self) {
        let (Some(ins), Some(gfx)) = (self.inspect.inspector.as_mut(), self.gfx.as_ref()) else {
            return;
        };
        let Some((hist, probe)) = self.inspect.in_flight else {
            return;
        };
        let rb = ins.poll(&gfx.gpu);
        if rb.is_none() && ins.busy() {
            return;
        }
        self.inspect.in_flight = None;
        let Some(rb) = rb else {
            // The mapping failed: ask again.
            debug!(target: RENDER, "inspect: readback failed");
            return;
        };
        self.inspect.readbacks += 1;
        if self.inspect.readbacks.is_power_of_two() {
            debug!(target: RENDER, "inspect: readback #{} in {:.2} ms (record {:.2} ms)", self.inspect.readbacks, rb.latency_ms, ins.last_record_ms);
        }
        if let Some(k) = hist
            && self.chrome.inspect.tools.histogram
            && self.inspect.hist_id == Some(k.id)
        {
            let h = rb.histogram.map(Box::new);
            if self.chrome.inspect.histogram != h {
                self.chrome.inspect.histogram = h;
                self.redraw = true;
            }
        }
        if let Some(k) = probe
            && let Some(p) = self.chrome.inspect.pixel.as_mut()
            && self.inspect.pixel_id == Some(k.id)
            && (p.x, p.y) == k.px
            && p.display != rb.probe
        {
            p.display = rb.probe;
            self.redraw = true;
        }
    }

    fn poll_source(&mut self) {
        while let Ok(b) = self.inspect.rx.try_recv() {
            if b.serial != self.inspect.source_serial {
                continue;
            }
            self.inspect.source_job = None;
            let value = b
                .raster
                .as_ref()
                .zip(self.chrome.inspect.pixel)
                .filter(|_| self.inspect.pixel_id == Some(b.id))
                .and_then(|(r, p)| source_pixel(r, p.x, p.y))
                .filter(|v| self.source_fits(b.id, v));
            if let Some(r) = b.raster {
                self.inspect.block = Some((b.id, r));
            }
            if let Some(p) = self.chrome.inspect.pixel.as_mut()
                && self.inspect.pixel_id == Some(b.id)
            {
                let s = value.map_or(SourceState::Unavailable, SourceState::Value);
                if p.source != s {
                    p.source = s;
                    self.redraw = true;
                }
            }
        }
    }

    /// Updates the inspected pixel and its source value for the pointer's pixel `px`.
    fn follow_pointer(&mut self, px: Option<(ImageId, (u32, u32))>) {
        let Some((id, (x, y))) = px else {
            self.cancel_source();
            if self.chrome.inspect.pixel.take().is_some() {
                self.redraw = true;
            }
            return;
        };
        let same = self.inspect.pixel_id == Some(id)
            && self
                .chrome
                .inspect
                .pixel
                .is_some_and(|p| (p.x, p.y) == (x, y));
        if !same {
            let source = self.source_now(id, (x, y));
            // The displayed colour of the last pixel stays until the probe answers (one
            // readback, a few ms), so the panel does not change height on every move.
            let display = self
                .chrome
                .inspect
                .pixel
                .filter(|_| self.inspect.pixel_id == Some(id))
                .and_then(|p| p.display);
            self.chrome.inspect.pixel = Some(PixelInfo {
                x,
                y,
                source,
                display,
            });
            self.inspect.pixel_id = Some(id);
            self.redraw = true;
        }
        // Start the source decode once the pointer has rested.
        if let Some((sid, spx, due)) = self.inspect.source_due
            && Instant::now() >= due
        {
            self.inspect.source_due = None;
            if (sid, spx) == (id, (x, y)) {
                self.start_source_job(id, spx);
            }
        }
    }

    fn cancel_source(&mut self) {
        self.inspect.source_due = None;
        if let Some(t) = self.inspect.source_job.take() {
            t.cancel();
        }
        self.inspect.source_serial += 1;
    }

    /// Whether `v` holds the file's samples: 8-bit values for 8-bit files, 16-bit ones for
    /// deeper files (a decoder that narrows 16 bits to 8 gives no source value).
    fn source_fits(&self, id: ImageId, v: &SourceValue) -> bool {
        let depth = self
            .images
            .get(&id)
            .and_then(|i| i.info.as_ref())
            .map_or(0, |i| i.bit_depth);
        match v.bits() {
            8 => depth == 8,
            _ => depth > 8,
        }
    }

    /// The source value of pixel `px` if it is at hand, else whether one is coming.
    fn source_now(&mut self, id: ImageId, px: (u32, u32)) -> SourceState {
        self.cancel_source();
        let Some(img) = self.images.get(&id) else {
            return SourceState::Unavailable;
        };
        let Some(info) = img.info.as_ref() else {
            return SourceState::Unavailable;
        };
        // Animation frames differ from the stored first frame; samples of fewer than 8 bits
        // are stored widened.
        if info.bit_depth < 8 || self.animated_current() {
            return SourceState::Unavailable;
        }
        if let Some((OverviewKind::Level(1), r)) = &img.overview
            && let Some(v) = source_pixel(r, px.0, px.1)
            && self.source_fits(id, &v)
        {
            return SourceState::Value(v);
        }
        if let Some((bid, r)) = &self.inspect.block
            && *bid == id
            && let Some(v) = source_pixel(r, px.0, px.1)
            && self.source_fits(id, &v)
        {
            return SourceState::Value(v);
        }
        if !info.fast_regions || img.session.is_none() {
            return SourceState::Unavailable;
        }
        self.inspect.source_due = Some((id, px, Instant::now() + SOURCE_DEBOUNCE));
        SourceState::Pending
    }

    /// Decodes the block around `px` at full resolution on the loader pool.
    fn start_source_job(&mut self, id: ImageId, px: (u32, u32)) {
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let (Some(session), Some(info)) = (img.session.clone(), img.info.as_ref()) else {
            return;
        };
        let rect = source_block(px, info.width, info.height);
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        let token = CancelToken::new();
        self.inspect.source_job = Some(token.clone());
        self.inspect.source_serial += 1;
        let serial = self.inspect.source_serial;
        let tx = self.inspect.tx.clone();
        let wake = self.loader.waker();
        let label = format!("img={id} source pixel {},{}", px.0, px.1);
        self.loader
            .pool()
            .submit_labeled(Priority::Near, token, label, move |cancel| {
                let t0 = Instant::now();
                let raster = if !session.regions_ready() && session.prepare_regions(cancel).is_err()
                {
                    None
                } else {
                    let req = DecodeRequest {
                        scale: 1,
                        region: Some(rect),
                        parallel: false,
                        prepare_regions: false,
                    };
                    session.decode(&req, cancel).ok().map(Arc::new)
                };
                if cancel.is_cancelled() {
                    return;
                }
                debug!(target: RENDER, "img={id} source block {}×{}+{}+{} in {:.2} ms", rect.w, rect.h, rect.x, rect.y, t0.elapsed().as_secs_f64() * 1e3);
                let _ = tx.send(SourceBlock { id, serial, raster });
                wake();
            });
    }

    /// Records and submits a histogram and/or probe request when what they would show
    /// changed and none is in flight.
    fn request_readback(&mut self, cur: Option<ImageId>, px: Option<(ImageId, (u32, u32))>) {
        let tools = self.chrome.inspect.tools;
        if !tools.histogram && px.is_none() {
            return;
        }
        let Some(id) = cur else { return };
        let Some(gfx) = self.gfx.as_ref() else { return };
        let Some(ins) = self.inspect.inspector.as_mut() else {
            return;
        };
        if ins.busy() {
            return;
        }
        let Some(img) = self.gpu_images.get(id) else {
            return;
        };
        let tiles = gfx.renderer.pool.len();
        let hist = tools
            .histogram
            .then(|| HistKey {
                id,
                rev: img.revision,
                view_rev: self.view.revision,
                tiles,
                req: histogram_request(&self.settings),
            })
            .filter(|k| self.inspect.hist_done != Some(*k));
        // While the view moves, at most one histogram per HIST_MOVING_EVERY; the settled
        // view's one goes out on the first turn after it stops (its key differs).
        let moving = self.view.is_animating() || self.view.is_dragging();
        let now = Instant::now();
        let hist = match (hist, self.inspect.hist_at) {
            (Some(k), Some(at)) if moving && now < at + HIST_MOVING_EVERY => {
                if self.inspect.hist_skipped_key != Some(k) {
                    self.inspect.hist_skipped_key = Some(k);
                    self.inspect.hist_skipped += 1;
                }
                let due = at + HIST_MOVING_EVERY;
                self.inspect.wake = Some(self.inspect.wake.map_or(due, |w| w.min(due)));
                None
            }
            (h, _) => h,
        };
        let probe = px
            .map(|(pid, px)| ProbeKey {
                id: pid,
                rev: img.revision,
                tiles,
                px,
            })
            .filter(|k| self.inspect.probe_done != Some(*k));
        if hist.is_none() && probe.is_none() {
            return;
        }
        let mut enc =
            gfx.gpu
                .device
                .create_command_encoder(&specular_gfx::wgpu::CommandEncoderDescriptor {
                    label: Some("inspect"),
                });
        let ok = ins.request(
            &gfx.gpu,
            &gfx.renderer,
            &mut enc,
            img,
            &self.view,
            hist.map(|k| k.req),
            probe.map(|k| k.px),
        );
        // Asked for (answered or not: nothing resident yet is asked again when tiles come).
        if hist.is_some() {
            self.inspect.hist_done = hist;
            self.inspect.hist_at = Some(now);
            self.inspect.hist_requests += 1;
            trace!(target: RENDER, "inspect: histogram #{} (view {}; {} skipped while moving)",
                self.inspect.hist_requests, if moving { "moving" } else { "settled" }, self.inspect.hist_skipped);
        }
        if probe.is_some() {
            self.inspect.probe_done = probe;
        }
        if !ok {
            return;
        }
        gfx.gpu.queue.submit([enc.finish()]);
        ins.after_submit();
        self.inspect.in_flight = Some((hist, probe));
    }
}

#[cfg(test)]
mod shots;

#[cfg(test)]
mod tests {
    use super::*;
    use specular_core::Orientation;

    #[test]
    fn blocks_are_aligned_and_clipped() {
        let r = source_block((37, 5), 40, 100);
        assert_eq!((r.x, r.y, r.w, r.h), (32, 0, 8, 16));
        let r = source_block((0, 99), 40, 100);
        assert_eq!((r.x, r.y, r.w, r.h), (0, 96, 16, 4));
    }

    #[test]
    fn the_pointer_pixel_follows_the_orientation() {
        let vp = PxRect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        // 1:1, the 40×20 image centred in the window.
        let v = View::fixed((40, 20), Orientation::NORMAL, vp, 1.0, (20.0, 10.0));
        assert_eq!(stored_pixel(&v, (30.5, 40.5)), Some((0, 0)));
        assert_eq!(stored_pixel(&v, (69.5, 59.5)), Some((39, 19)));
        assert_eq!(stored_pixel(&v, (29.0, 45.0)), None);
        assert_eq!(stored_pixel(&v, (150.0, 45.0)), None);
        // Rotated a quarter turn clockwise (EXIF 6): displayed 20×40; the top-left display
        // pixel is the stored bottom-left one.
        let v = View::fixed((40, 20), Orientation(6), vp, 1.0, (10.0, 20.0));
        assert_eq!(stored_pixel(&v, (40.5, 30.5)), Some((0, 19)));
    }
}
