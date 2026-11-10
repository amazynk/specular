//! Drawings (DXF): the vectors drawn on the GPU at the exact zoom, a layers panel, the
//! paper colour and lineweights.
//!
//! The session's pixels are the sheet rasterised once on the CPU (light paper, like a plot):
//! that is what thumbnails, the overview and the very first frame show. Once vello's
//! pipelines are built (on a "cad-vello" thread, once per run) every scene render draws the
//! sheet itself over the image's rectangle, so lines stay sharp at any zoom (up to
//! [`DRAWING_MAX_SCALE`]) and layers, the paper and lineweights change without decoding
//! again. The keymap's drawing mode ([`Mode::Drawing`], `[drawing]` in `keymap.toml`) is on
//! for a drawing: by default `l` opens the layers panel, `Shift+L` shows every layer (or
//! the file's own choice again), `b` swaps dark and light paper (white, ACI 7, follows) and
//! `w` shows lineweights. Every action is in [`Group::Drawing`] (the Engineering profile).
//!
//! [`Mode::Drawing`]: specular_input::Mode::Drawing
//! [`Group::Drawing`]: specular_input::Group::Drawing

#[cfg(test)]
pub(super) mod shots;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

use specular_decode::drawing::specular_vector::Generated;
use specular_decode::drawing::specular_vector::draw::View as SheetView;
use specular_decode::drawing::specular_vector::kurbo;
use specular_decode::drawing::specular_vector::prep::Prepared;
use specular_decode::drawing::{DrawingPage, paper_colour};
use specular_gfx::{Align, DrawList, Gpu, Rect, Text, TextStyle, wgpu};
use specular_i18n::t;
use specular_input::Action;
use specular_render::View;
use specular_render::vector::VectorRenderer;
use specular_render::view::DRAWING_MAX_SCALE;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::App;
use crate::loader::ImageId;

/// AccessKit ids: the layers panel, then a check box per layer (science uses 0x5900…,
/// medical 0x5800…, cull 0x5000…).
const A11Y_ID: u64 = 0x5A00_0000_0000_0000;
const A11Y_LAYER: u64 = A11Y_ID + 0x100;
/// Layers past this many have no node of their own.
const A11Y_MAX: u64 = 0xF000;

/// Logical width of the layers panel.
const PANEL_W: f32 = 260.0;
/// Characters of a layer name shown before it is cut.
const NAME_CHARS: usize = 36;

/// Drawing state for the current image.
#[derive(Default)]
pub(super) struct Cad {
    /// The image the state belongs to, and its drawing once its session is open.
    id: Option<ImageId>,
    page: Option<DrawingPage>,
    /// The drawing the layer choices were made for (kept across its pages).
    prepared: Option<Arc<Prepared>>,
    /// Layers shown, by index into the scene's layers.
    layers: Vec<bool>,
    /// The layers panel is open (stays open across drawings), and its first row.
    panel: bool,
    scroll: usize,
    /// Paper chosen by hand (`Some(dark)`); `None` follows the theme.
    paper: Option<bool>,
    lineweights: bool,
    /// Bumped by anything that changes the picture.
    rev: u64,
    renderer: Option<VectorRenderer>,
    building: Option<(Receiver<Result<VectorRenderer, String>>, Instant)>,
    /// Building the renderer or drawing failed: the CPU picture stays.
    failed: bool,
    /// The panel and its rows as last drawn (logical px): layer and row.
    panel_rect: Option<Rect>,
    rows: Vec<(usize, Rect)>,
    /// When the current drawing was found, until its first GPU frame.
    shown_at: Option<Instant>,
}

/// The layers panel as drawn: its rect, each row's layer index and rect, and the scroll.
pub(super) type DrawnPanel = (Rect, Vec<(usize, Rect)>, usize);

/// What the frame needs to know about the drawing ([`App::cad_frame`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CadFrame {
    pub id: ImageId,
    pub dark: bool,
    /// Changes whenever the GPU picture would (a part of the scene key).
    pub key: u64,
}

/// One row of the layers panel.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LayerRow {
    pub index: usize,
    pub name: String,
    pub color: [u8; 3],
    pub on: bool,
}

/// The layers panel as drawn.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LayersPanel {
    pub rows: Vec<LayerRow>,
    pub shown: usize,
    pub total: usize,
    /// Rows above the first one listed.
    pub scroll: usize,
}

impl Cad {
    fn active(&self) -> bool {
        self.page.is_some()
    }

    fn bump(&mut self) {
        self.rev = self.rev.wrapping_add(1);
    }

    /// The GPU device was replaced: the vector renderer is built again (until then the
    /// CPU picture shows).
    pub(super) fn gpu_reset(&mut self) {
        self.renderer = None;
        self.building = None;
        self.failed = false;
        self.bump();
    }

    /// The vector renderer, once built (the map view draws with it too).
    pub(super) fn renderer_mut(&mut self) -> Option<&mut VectorRenderer> {
        self.renderer.as_mut()
    }

    /// The panel as drawn: rows to hit-test.
    pub(super) fn set_drawn(&mut self, drawn: Option<DrawnPanel>) {
        match drawn {
            Some((panel, rows, scroll)) => {
                self.panel_rect = Some(panel);
                self.rows = rows;
                self.scroll = scroll;
            }
            None => {
                self.panel_rect = None;
                self.rows.clear();
            }
        }
    }
}

impl App {
    /// Keeps the drawing state in step with the current image: finds its drawing, lets the
    /// zoom go deep, turns the keymap's drawing layer on, and builds the GPU renderer the
    /// first time a drawing shows. Cheap when nothing changed.
    pub(super) fn cad_sync(&mut self) {
        self.cad_poll();
        let cur = self.current_id();
        if cur != self.cad.id {
            self.cad.id = cur;
            self.cad.page = None;
            self.cad.shown_at = None;
            self.cad.bump();
        }
        if self.cad.page.is_none()
            && let Some(page) = cur
                .and_then(|id| self.images.get(&id))
                .filter(|i| i.error.is_none())
                .and_then(|i| i.session.as_ref())
                .and_then(|s| s.drawing().cloned())
        {
            let same = self
                .cad
                .prepared
                .as_ref()
                .is_some_and(|p| Arc::ptr_eq(p, &page.prepared));
            if !same {
                let scene = &page.prepared.scene;
                self.cad.layers = scene.layers.iter().map(|l| l.visible()).collect();
                self.cad.lineweights = scene.lineweight_display;
                self.cad.scroll = 0;
                self.cad.prepared = Some(page.prepared.clone());
            }
            self.cad.page = Some(page);
            self.cad.shown_at = Some(Instant::now());
            self.cad.bump();
            self.redraw = true;
        }
        let on = self.cad.active();
        self.view.set_max_scale(on.then_some(DRAWING_MAX_SCALE));
        let keys = on && !self.gallery_active();
        if self.keys.drawing != keys {
            self.keys.drawing = keys;
            self.keys_sync_modes();
        }
        let want = on || self.gis_wants_renderer();
        if want && self.cad.renderer.is_none() && self.cad.building.is_none() && !self.cad.failed {
            self.cad_build();
        }
    }

    /// Builds vello's pipelines on a thread of their own (it takes a while).
    fn cad_build(&mut self) {
        let Some(gpu) = self.gfx.as_ref().map(|g| g.gpu.clone()) else {
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let wake = self.loader.waker();
        let spawned = specular_thread::builder("cad-vello").spawn(move || {
            let r = VectorRenderer::new(&gpu);
            let _ = tx.send(r);
            wake();
        });
        match spawned {
            Ok(_) => self.cad.building = Some((rx, Instant::now())),
            Err(e) => {
                specular_log::warn!(target: specular_log::RENDER, "drawing renderer thread: {e}");
                self.cad.failed = true;
            }
        }
    }

    /// Takes the renderer once it is built.
    fn cad_poll(&mut self) {
        let Some((rx, t0)) = &self.cad.building else {
            return;
        };
        let r = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("the renderer thread ended".into()),
        };
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        self.cad.building = None;
        match r {
            Ok(v) => {
                specular_log::info!(target: specular_log::RENDER, "drawing renderer (vello) ready in {ms:.0} ms");
                self.cad.renderer = Some(v);
            }
            Err(e) => {
                specular_log::warn!(target: specular_log::RENDER, "drawing renderer (vello) failed, drawings stay CPU pictures: {e}");
                self.cad.failed = true;
            }
        }
        self.cad.bump();
        self.redraw = true;
    }

    /// Whether the current image is a drawing.
    pub(super) fn cad_active(&self) -> bool {
        self.cad.active() && self.cad.id == self.current_id()
    }

    /// The current image's drawing page, when it is a drawing.
    pub(super) fn cad_page(&self) -> Option<&DrawingPage> {
        self.cad.page.as_ref().filter(|_| self.cad_active())
    }

    /// The paper is dark: chosen by hand, else the theme's.
    fn cad_dark(&self, pal: &Palette) -> bool {
        self.cad
            .paper
            .unwrap_or(pal.mode == specular_theme::Mode::Dark)
    }

    /// The vector renderer is built.
    pub(super) fn cad_has_renderer(&self) -> bool {
        self.cad.renderer.is_some()
    }

    /// The paper of drawings and vector maps is dark.
    pub(super) fn cad_dark_paper(&self, pal: &Palette) -> bool {
        self.cad_dark(pal)
    }

    /// What the frame draws of the drawing, when it draws it on the GPU (the map view draws
    /// a vector map's layers itself).
    pub(super) fn cad_frame(&self, pal: &Palette) -> Option<CadFrame> {
        if !self.cad_active()
            || self.gallery_active()
            || self.cad.renderer.is_none()
            || self.gis_draws_vectors()
        {
            return None;
        }
        let dark = self.cad_dark(pal);
        Some(CadFrame {
            id: self.cad.id?,
            dark,
            key: self.cad.rev.wrapping_mul(2) | u64::from(dark),
        })
    }

    /// Drawing actions; true when `a` was one.
    pub(super) fn cad_action(&mut self, a: Action) -> bool {
        if !matches!(
            a,
            Action::DrawingLayers
                | Action::DrawingAllLayers
                | Action::DrawingPaper
                | Action::DrawingLineweights
        ) {
            return false;
        }
        if !self.cad_active() {
            return true;
        }
        let now = self.now();
        match a {
            Action::DrawingLayers => self.cad.panel = !self.cad.panel,
            Action::DrawingAllLayers => {
                let msg = if self.cad.layers.iter().all(|&on| on) {
                    let saved: Vec<bool> = self
                        .cad
                        .prepared
                        .as_ref()
                        .map(|p| p.scene.layers.iter().map(|l| l.visible()).collect())
                        .unwrap_or_default();
                    self.cad.layers = saved;
                    t!("drawing-layers-saved-toast")
                } else {
                    self.cad.layers.iter_mut().for_each(|on| *on = true);
                    t!("drawing-layers-all-toast")
                };
                self.chrome.toast(msg, None, now);
            }
            Action::DrawingPaper => {
                let dark = !self.cad_dark(&self.palette());
                self.cad.paper = Some(dark);
                let msg = if dark {
                    t!("drawing-paper-dark-toast")
                } else {
                    t!("drawing-paper-light-toast")
                };
                self.chrome.toast(msg, None, now);
            }
            Action::DrawingLineweights => {
                self.cad.lineweights = !self.cad.lineweights;
                let msg = if self.cad.lineweights {
                    t!("drawing-lineweights-on")
                } else {
                    t!("drawing-lineweights-off")
                };
                self.chrome.toast(msg, None, now);
            }
            _ => {}
        }
        self.cad.bump();
        self.redraw = true;
        true
    }

    /// Shows or hides layer `index`.
    fn cad_toggle(&mut self, index: usize) {
        if let Some(on) = self.cad.layers.get_mut(index) {
            *on = !*on;
            self.cad.bump();
            self.redraw = true;
        }
    }

    /// The layers panel to draw, when it is open over a drawing.
    pub(super) fn cad_panel(&self, pal: &Palette) -> Option<LayersPanel> {
        if !self.cad.panel
            || !self.cad_active()
            || self.gallery_active()
            || self.gis_draws_vectors()
        {
            return None;
        }
        let p = self.cad.prepared.as_ref()?;
        let dark = self.cad_dark(pal);
        let rows = p
            .scene
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| LayerRow {
                index: i,
                name: short_name(&shown_name(&l.name, l.generated)),
                color: l.color.on(dark, paper_colour(dark)),
                on: self.cad.layers.get(i).copied().unwrap_or(true),
            })
            .collect::<Vec<_>>();
        Some(LayersPanel {
            shown: rows.iter().filter(|r| r.on).count(),
            total: rows.len(),
            rows,
            scroll: self.cad.scroll,
        })
    }

    /// A press at logical `lp`: a layer's row toggles it; the rest of the panel takes the
    /// press. True when taken.
    pub(super) fn cad_press(&mut self, lp: (f32, f32)) -> bool {
        let Some(panel) = self.cad.panel_rect else {
            return false;
        };
        if !panel.contains(lp) {
            return false;
        }
        if let Some(&(i, _)) = self.cad.rows.iter().find(|(_, r)| r.contains(lp)) {
            self.cad_toggle(i);
        }
        true
    }

    /// The wheel over the layers panel scrolls it. True when taken.
    pub(super) fn cad_wheel(&mut self, dy: f32, lp: (f32, f32)) -> bool {
        if !self.cad.panel_rect.is_some_and(|r| r.contains(lp)) {
            return false;
        }
        let steps = (dy.abs().ceil() as usize).max(1) * 3;
        self.cad.scroll = if dy > 0.0 {
            self.cad.scroll.saturating_sub(steps)
        } else {
            self.cad.scroll + steps
        };
        self.redraw = true;
        true
    }

    /// A screen reader's action on a layer's check box; true when the node is the panel's.
    pub(super) fn cad_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let id = req.target_node.0;
        if !(A11Y_ID..A11Y_LAYER + A11Y_MAX).contains(&id) {
            return false;
        }
        if id >= A11Y_LAYER && req.action == accesskit::Action::Click {
            self.cad_toggle((id - A11Y_LAYER) as usize);
        }
        self.redraw = true;
        true
    }
}

/// Something that renders Fluent messages: the app's [`I18n`](specular_i18n::I18n), a
/// [`Localizer`](specular_i18n::Localizer), or a catalogue made up in a test.
pub(crate) trait Messages {
    fn tr(&self, id: &str) -> String;
    fn tr_args(&self, id: &str, args: &specular_i18n::FluentArgs<'_>) -> String;
}

impl Messages for specular_i18n::I18n {
    fn tr(&self, id: &str) -> String {
        specular_i18n::I18n::tr(self, id)
    }
    fn tr_args(&self, id: &str, args: &specular_i18n::FluentArgs<'_>) -> String {
        specular_i18n::I18n::tr_args(self, id, args)
    }
}

/// A name Specular made up for a layer or sheet ("Pen 3"), in the language of `m`.
pub(crate) fn generated_name_in(m: &impl Messages, g: Generated) -> String {
    match g {
        Generated::Pen(n) => t!(m => "cad-pen-name", n = n),
        Generated::Page(n) => t!(m => "cad-page-name", n = n),
        Generated::Layer(n) => t!(m => "cad-layer-name", n = n),
        Generated::Map => t!(m => "cad-map-name"),
        Generated::BoardLayers => t!(m => "cad-board-layers-name"),
        Generated::BoardTop => t!(m => "cad-board-top-name"),
        Generated::BoardBottom => t!(m => "cad-board-bottom-name"),
        Generated::ChartFrame => t!(m => "cad-chart-frame-name"),
        Generated::Trace(n) => t!(m => "cad-trace-name", n = n),
        Generated::Contours { negative: false } => t!(m => "cad-contours-positive-name"),
        Generated::Contours { negative: true } => t!(m => "cad-contours-negative-name"),
        Generated::Model3d => t!(m => "cad-model-3d-name"),
    }
}

/// How a layer or sheet is named on screen: the file's own name as it is, or the one
/// Specular made up in the reader's language. Every place that shows such a name uses this.
pub(crate) fn shown_name(name: &str, generated: Option<Generated>) -> String {
    match generated {
        Some(g) => generated_name_in(specular_i18n::I18n::global(), g),
        None => name.to_owned(),
    }
}

/// A layer name cut to fit the panel.
fn short_name(name: &str) -> String {
    if name.chars().count() <= NAME_CHARS {
        return name.to_owned();
    }
    let mut s: String = name.chars().take(NAME_CHARS - 1).collect();
    s.push('…');
    s
}

/// Draws the drawing over the scene target (after the image, under the overlays): sheet
/// coordinates to window pixels in `f64`, the paper over the image's rectangle in the
/// viewport. Logs what it took.
#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    cad: &mut Cad,
    gpu: &Gpu,
    enc: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    size: (u32, u32),
    view: &View,
    frame: &CadFrame,
    dpi_scale: f64,
) {
    let Some(page) = cad.page.as_ref() else {
        return;
    };
    let Some(sheet) = page.sheet() else {
        return;
    };
    let Some(r) = cad.renderer.as_mut() else {
        return;
    };
    let m = view.screen_from_stored_f64();
    let screen = kurbo::Affine::new([m[0], m[3], m[1], m[4], m[2], m[5]]);
    let ir = view.image_screen_rect();
    let vp = view.viewport;
    let rect = |x: f32, y: f32, w: f32, h: f32| {
        kurbo::Rect::new(
            f64::from(x),
            f64::from(y),
            f64::from(x + w),
            f64::from(y + h),
        )
    };
    let paper = rect(ir.x, ir.y, ir.w, ir.h).intersect(rect(vp.x, vp.y, vp.w, vp.h));
    let v = SheetView {
        transform: screen * page.to_stored,
        size,
        dark: frame.dark,
        background: paper_colour(frame.dark),
        lineweights: cad.lineweights,
        dpi_scale,
        layers: &cad.layers,
    };
    match r.render(gpu, sheet, &v, paper) {
        Ok(stats) => {
            r.blit(enc, target);
            specular_log::debug!(
                target: specular_log::RENDER,
                "drawing frame: {} drawn, {} culled, {} hidden; encode {:.2} ms, submit {:.2} ms",
                stats.walk.drawn,
                stats.walk.culled,
                stats.walk.hidden,
                stats.encode_ms,
                stats.submit_ms,
            );
            if let Some(t) = cad.shown_at.take() {
                specular_log::info!(
                    target: specular_log::RENDER,
                    "drawing: opened in {:.1} ms, first GPU frame {:.1} ms after it showed (encode {:.2} ms)",
                    page.open_ms,
                    t.elapsed().as_secs_f64() * 1e3,
                    stats.encode_ms,
                );
            }
        }
        Err(e) => {
            specular_log::warn!(target: specular_log::RENDER, "drawing renderer failed, drawings stay CPU pictures: {e}");
            cad.renderer = None;
            cad.failed = true;
        }
    }
}

/// Draws the layers panel at the bottom-start corner of `area` (logical x, y, w, h), with
/// as many rows as fit; returns the panel, each row listed (layer and rectangle) and the
/// first row listed.
pub(super) fn draw_panel(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    p: &LayersPanel,
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) -> DrawnPanel {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let st = TextStyle::new(FONT_SMALL * ts);
    let title = text.layout(
        &t!(
            "drawing-layers-title",
            shown = p.shown as i64,
            total = p.total as i64
        ),
        st,
        None,
        Align::Start,
    );
    let row_h = (FONT_SMALL * 1.9 * ts).round();
    let w = (PANEL_W * ts).min(aw - 2.0 * m).max(120.0);
    let max_h = (ah - 2.0 * m).max(row_h * 3.0) * 0.6;
    let fit = (((max_h - 2.0 * PAD - title.height - 4.0 * ts) / row_h).floor() as usize).max(1);
    let n = p.rows.len().min(fit);
    let scroll = p.scroll.min(p.rows.len() - n);
    let h = 2.0 * PAD + title.height + 4.0 * ts + n as f32 * row_h;
    let x = if rtl { ax + aw - m - w } else { ax + m };
    let y = ay + ah - m - h;
    let panel = Rect::new(x, y, w, h);
    dl.glass(panel, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let tx = if rtl {
        x + w - PAD - title.width
    } else {
        x + PAD
    };
    text.draw(dl, &title, tx, y + PAD, pal.text_muted);
    let box_s = 12.0 * ts;
    let sw_s = 10.0 * ts;
    let mut rows = Vec::with_capacity(n);
    let mut ry = y + PAD + title.height + 4.0 * ts;
    for r in &p.rows[scroll..scroll + n] {
        let row = Rect::new(x + PAD / 2.0, ry, w - PAD, row_h);
        let cy = ry + row_h / 2.0;
        // Check box, swatch, name; mirrored right to left.
        let (bx, sx) = if rtl {
            (x + w - PAD - box_s, x + w - PAD - box_s - 8.0 * ts - sw_s)
        } else {
            (x + PAD, x + PAD + box_s + 8.0 * ts)
        };
        let b = Rect::new(bx, cy - box_s / 2.0, box_s, box_s);
        if r.on {
            dl.rect(b, 3.0 * ts, pal.accent_fill);
            let (l, t) = (b.x, b.y);
            let s = box_s;
            dl.line(
                (l + 0.22 * s, t + 0.52 * s),
                (l + 0.42 * s, t + 0.72 * s),
                1.6 * ts,
                pal.accent_text,
            );
            dl.line(
                (l + 0.42 * s, t + 0.72 * s),
                (l + 0.78 * s, t + 0.30 * s),
                1.6 * ts,
                pal.accent_text,
            );
        } else {
            dl.rect(b, 3.0 * ts, pal.text_faint);
            dl.rect(b.inset(1.5 * ts), 2.0 * ts, pal.surface);
        }
        let [cr, cg, cb] = r.color;
        let swatch = Rect::new(sx, cy - sw_s / 2.0, sw_s, sw_s);
        dl.rect(swatch, 2.0 * ts, pal.text_faint);
        dl.rect(
            swatch.inset(1.0),
            2.0 * ts,
            Color::rgba(
                f32::from(cr) / 255.0,
                f32::from(cg) / 255.0,
                f32::from(cb) / 255.0,
                1.0,
            ),
        );
        let label = text.layout(&r.name, st, None, Align::Start);
        let lx = if rtl {
            sx - 8.0 * ts - label.width
        } else {
            sx + sw_s + 8.0 * ts
        };
        let ink = if r.on { pal.text } else { pal.text_muted };
        text.draw(dl, &label, lx, cy - label.height / 2.0, ink);
        rows.push((r.index, row));
        ry += row_h;
    }
    (panel, rows, scroll)
}

/// The AccessKit nodes: the panel as a group of check boxes, one per layer listed.
pub(super) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    p: &LayersPanel,
    panel: Rect,
    rows: &[(usize, Rect)],
    sf: f64,
) {
    use accesskit::{Node, NodeId, Role, Toggled};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * sf,
        y0: f64::from(r.y) * sf,
        x1: f64::from(r.x + r.w) * sf,
        y1: f64::from(r.y + r.h) * sf,
    };
    let mut g = Node::new(Role::Group);
    g.set_label(t!(
        "drawing-layers-title",
        shown = p.shown as i64,
        total = p.total as i64
    ));
    g.set_bounds(bounds(panel));
    let mut children = Vec::new();
    for (index, r) in rows {
        let Some(row) = p.rows.get(*index).filter(|_| (*index as u64) < A11Y_MAX) else {
            continue;
        };
        let id = NodeId(A11Y_LAYER + *index as u64);
        let mut n = Node::new(Role::CheckBox);
        n.set_label(row.name.clone());
        n.set_toggled(if row.on {
            Toggled::True
        } else {
            Toggled::False
        });
        n.set_bounds(bounds(*r));
        n.add_action(accesskit::Action::Click);
        g.push_child(id);
        children.push((id, n));
    }
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_ID));
    }
    tree.nodes.push((NodeId(A11Y_ID), g));
    tree.nodes.extend(children);
}

#[cfg(test)]
mod name_tests {
    use super::*;
    use fluent_bundle::{FluentBundle, FluentResource};
    use specular_i18n::{FluentArgs, I18n, langid};

    /// A German catalogue made up for the test (the real one gets these ids from the
    /// translators): what is shown comes from the `.ftl`, not from the reader.
    struct German(FluentBundle<FluentResource>);

    impl German {
        fn new() -> German {
            let ftl = "cad-pen-name = Stift { $n }\n\
                       cad-page-name = Seite { $n }\n\
                       cad-layer-name = Ebene { $n }\n\
                       cad-map-name = Karte\n";
            let mut b = FluentBundle::new(vec![langid!("de")]);
            b.set_use_isolating(false);
            b.add_resource(FluentResource::try_new(ftl.to_owned()).unwrap())
                .unwrap();
            German(b)
        }
    }

    impl Messages for German {
        fn tr(&self, id: &str) -> String {
            self.tr_args(id, &FluentArgs::new())
        }
        fn tr_args(&self, id: &str, args: &FluentArgs<'_>) -> String {
            let p = self.0.get_message(id).and_then(|m| m.value()).unwrap();
            let mut errors = Vec::new();
            self.0
                .format_pattern(p, Some(args), &mut errors)
                .into_owned()
        }
    }

    /// The made-up names are rendered from the catalogue in the reader's language; names
    /// from the file are shown as they are.
    #[test]
    fn generated_names_follow_the_catalogue() {
        let de = German::new();
        assert_eq!(generated_name_in(&de, Generated::Pen(3)), "Stift 3");
        assert_eq!(generated_name_in(&de, Generated::Page(2)), "Seite 2");
        assert_eq!(generated_name_in(&de, Generated::Layer(4)), "Ebene 4");
        assert_eq!(generated_name_in(&de, Generated::Map), "Karte");

        // English, as the readers store it, through the app's own catalogue.
        let strip = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
        let en = I18n::for_language(langid!("en"));
        for g in [
            Generated::Pen(3),
            Generated::Page(12),
            Generated::Layer(1),
            Generated::Map,
        ] {
            assert_eq!(strip(generated_name_in(&en, g)), g.english());
        }
        // The pseudo-locale shows that what is on screen goes through the catalogue.
        I18n::with_thread_language(langid!("en-XA"), || {
            let s = shown_name("Pen 7", Some(Generated::Pen(7)));
            assert_ne!(strip(s.clone()), "Pen 7");
            assert!(s.contains('7'), "{s}");
            // A name from the file is never translated.
            assert_eq!(shown_name("Model", None), "Model");
            assert_eq!(shown_name("Pen 7", None), "Pen 7");
        });
    }
}
