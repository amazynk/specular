//! Comparing two revisions of a sheet: the sheet shown
//! and another one (another PDF page, a PDF and a scan, two scans) aligned and overlaid in
//! two colours, as a difference, or faded and blinked one into the other; the changed regions
//! are outlined as clouds and can become markup.
//!
//! Both sheets are rendered to grey copies of about 2048 px (`specular-markup`'s
//! [`Grey`]); the other is aligned onto this one automatically (Fourier–Mellin, then
//! sub-pixel patch matching) or from two point pairs the user clicks (then refined), and the
//! picture is composed off the UI thread and drawn over the image.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use specular_core::CancelToken;
use specular_gfx::winit::event_loop::ActiveEventLoop;
use specular_gfx::{DrawList, Gpu, Rect, TexId, Text, UiRenderer, wgpu};
use specular_i18n::t;
use specular_input::{Action, Group, Key, KeyCombo, NamedKey};
use specular_markup::P;
use specular_markup::align::{Aligned, Grey, Similarity, auto};
use specular_markup::compare::{View, changes, compose, warp};
use specular_theme::metrics::FONT_SMALL;
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::App;
use super::markup::{Button, bar};

#[cfg(test)]
mod shots;

const A11Y_BASE: u64 = 0x6600_0000_0000_0000;
/// Long side of the grey copies compared.
const SIDE: usize = 2048;
/// Seconds each sheet shows while blinking.
const BLINK_S: f64 = 0.6;
/// Slack (px of the copies) for lines that moved by rendering or alignment, and the gap
/// (px) that merges nearby changes into one region.
const SLACK: usize = 2;
const MERGE: usize = 14;
const MIN_CHANGED: usize = 24;

/// How the sheets are shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Mode {
    Overlay,
    Difference,
    Blend,
}

/// What a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    Other,
    Page,
    Auto,
    Pick,
    Mode(Mode),
    Blink,
    Slider,
    Clouds,
    Done,
}

/// Everything the compare bar shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CompareView {
    pub other: Option<String>,
    /// The other sheet's page and page count.
    pub page: Option<(u32, u32)>,
    pub mode: Mode,
    pub blend: f32,
    pub blink: bool,
    pub aligned: bool,
    pub changes: usize,
    pub status: String,
    pub focus: Option<usize>,
    /// Changed regions on screen (logical px).
    pub regions: Vec<Rect>,
}

impl CompareView {
    pub fn focusables(&self) -> Vec<Hit> {
        let mut v = vec![Hit::Other];
        if self.page.is_some_and(|p| p.1 > 1) {
            v.push(Hit::Page);
        }
        v.extend([
            Hit::Auto,
            Hit::Pick,
            Hit::Mode(Mode::Overlay),
            Hit::Mode(Mode::Difference),
            Hit::Mode(Mode::Blend),
            Hit::Blink,
            Hit::Clouds,
            Hit::Done,
        ]);
        if self.mode == Mode::Blend {
            v.push(Hit::Slider);
        }
        v
    }

    pub fn enabled(&self, h: Hit) -> bool {
        match h {
            Hit::Auto | Hit::Pick | Hit::Blink | Hit::Mode(_) | Hit::Slider => self.other.is_some(),
            Hit::Clouds => self.aligned && self.changes > 0,
            _ => true,
        }
    }

    pub fn label(&self, h: Hit) -> String {
        match h {
            Hit::Other => match &self.other {
                Some(n) => t!("sheets-other-named", name = n.clone()),
                None => t!("sheets-other"),
            },
            Hit::Page => {
                let (p, n) = self.page.unwrap_or((0, 1));
                t!("sheets-page", page = i64::from(p + 1), count = i64::from(n))
            }
            Hit::Auto => t!("sheets-auto"),
            Hit::Pick => t!("sheets-pick"),
            Hit::Mode(Mode::Overlay) => t!("sheets-overlay"),
            Hit::Mode(Mode::Difference) => t!("sheets-difference"),
            Hit::Mode(Mode::Blend) => t!("sheets-blend"),
            Hit::Blink => t!("sheets-blink"),
            Hit::Slider => t!(
                "sheets-slider",
                percent = (self.blend * 100.0).round() as i64
            ),
            Hit::Clouds => t!("sheets-clouds", count = self.changes as i64),
            Hit::Done => t!("sheets-done"),
        }
    }
}

/// Where the controls landed.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawn {
    pub bar: Rect,
    pub hits: Vec<(Rect, Hit)>,
    /// The slider's track.
    pub track: Option<Rect>,
}

const SLIDER_H: f32 = 30.0;

/// Draws the changed regions and the bar (the composed picture is drawn by the caller).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &CompareView,
    area: Rect,
    rtl: bool,
    ts: f32,
) -> Drawn {
    let focus = v.focus.and_then(|i| v.focusables().get(i).copied());
    dl.push_clip(area);
    let ring = Color::rgba(0.85, 0.1, 0.6, 1.0);
    for r in &v.regions {
        let pts = [
            P::new(f64::from(r.x), f64::from(r.y)),
            P::new(f64::from(r.right()), f64::from(r.y)),
            P::new(f64::from(r.right()), f64::from(r.bottom())),
            P::new(f64::from(r.x), f64::from(r.bottom())),
        ];
        let chord = f64::from((r.w.min(r.h) / 4.0).clamp(6.0, 18.0));
        let arcs = specular_markup::cloud::arcs(&pts, chord);
        let line = specular_markup::cloud::flatten(&arcs, 0.5);
        for w in line.windows(2) {
            dl.line(
                (w[0].x as f32, w[0].y as f32),
                (w[1].x as f32, w[1].y as f32),
                2.0,
                ring,
            );
        }
    }
    dl.pop_clip();
    let hits: Vec<Hit> = v
        .focusables()
        .into_iter()
        .filter(|h| *h != Hit::Slider)
        .collect();
    let buttons: Vec<Button> = hits
        .iter()
        .map(|h| Button {
            label: v.label(*h),
            enabled: v.enabled(*h),
            active: *h == Hit::Mode(v.mode) || (*h == Hit::Blink && v.blink) || *h == Hit::Done,
            focused: focus == Some(*h),
        })
        .collect();
    let slider = v.mode == Mode::Blend;
    let extra = if slider { SLIDER_H * ts.max(1.0) } else { 0.0 };
    let (r, rects, ..) = bar(
        dl,
        text,
        pal,
        &t!("sheets-title"),
        &buttons,
        None,
        &v.status,
        extra,
        area,
        rtl,
        ts,
    );
    let mut drawn = Drawn {
        bar: r,
        hits: rects.into_iter().zip(hits).collect(),
        track: None,
    };
    // The legend under the buttons' last line: the colours of the overlay.
    if v.mode == Mode::Overlay && v.other.is_some() {
        let st = specular_gfx::TextStyle::new(FONT_SMALL * ts);
        let items = [
            (Color::rgba(0.9, 0.1, 0.1, 1.0), t!("sheets-legend-this")),
            (Color::rgba(0.0, 0.75, 0.8, 1.0), t!("sheets-legend-other")),
            (Color::rgba(0.0, 0.0, 0.0, 1.0), t!("sheets-legend-both")),
        ];
        let blocks: Vec<_> = items
            .iter()
            .map(|(_, s)| text.layout(s, st, None, specular_gfx::Align::Start))
            .collect();
        let total: f32 = blocks.iter().map(|b| b.width + 22.0).sum();
        let y = r.bottom() + 6.0;
        let lr = Rect::new(
            r.x + (r.w - total - 16.0) / 2.0,
            y,
            total + 16.0,
            24.0 * ts.max(1.0),
        );
        dl.rect_border(lr, 6.0, pal.surface, pal.glass_border, 1.0);
        let mut x = lr.x + 8.0;
        let order: Vec<usize> = if rtl { vec![2, 1, 0] } else { vec![0, 1, 2] };
        for i in order {
            let b = &blocks[i];
            dl.rect(
                Rect::new(x, lr.y + (lr.h - 10.0) / 2.0, 10.0, 10.0),
                2.0,
                items[i].0,
            );
            text.draw(dl, b, x + 14.0, lr.y + (lr.h - b.height) / 2.0, pal.text);
            x += b.width + 22.0;
        }
    }
    if slider {
        let h = SLIDER_H * ts.max(1.0);
        let track = Rect::new(r.x + 24.0, r.bottom() - 12.0 - h - 14.0 * ts, r.w - 48.0, h);
        let y = track.y + track.h / 2.0;
        dl.rect(
            Rect::new(track.x, y - 2.0, track.w, 4.0),
            2.0,
            pal.separator,
        );
        let t = if rtl { 1.0 - v.blend } else { v.blend };
        let kx = track.x + track.w * t;
        dl.rect(
            Rect::new(track.x.min(kx), y - 2.0, (kx - track.x).abs(), 4.0),
            2.0,
            pal.accent,
        );
        dl.circle((kx, y), 8.0, pal.accent);
        if focus == Some(Hit::Slider) {
            dl.stroke_rect(track.inset(-3.0), 6.0, pal.focus_ring, 2.0);
        }
        drawn.track = Some(track);
        drawn.hits.push((track, Hit::Slider));
    }
    drawn
}

fn bounds(r: Rect, scale: f64) -> accesskit::Rect {
    accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    }
}

fn node(h: Hit) -> u64 {
    match h {
        Hit::Other => 1,
        Hit::Page => 2,
        Hit::Auto => 3,
        Hit::Pick => 4,
        Hit::Mode(Mode::Overlay) => 5,
        Hit::Mode(Mode::Difference) => 6,
        Hit::Mode(Mode::Blend) => 7,
        Hit::Blink => 8,
        Hit::Slider => 9,
        Hit::Clouds => 10,
        Hit::Done => 11,
    }
}

pub(super) fn target(id: u64, v: &CompareView) -> Option<Hit> {
    let k = id.checked_sub(A11Y_BASE)?;
    v.focusables().into_iter().find(|h| node(*h) == k)
}

/// The bar's AccessKit nodes: a toolbar of buttons (the modes as toggles) and the slider.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, v: &CompareView, d: &Drawn, scale: f64) {
    use accesskit::{Node, NodeId, Role, Toggled};
    let mut bar_n = Node::new(Role::Toolbar);
    bar_n.set_label(t!("sheets-title"));
    bar_n.set_description(v.status.clone());
    bar_n.set_bounds(bounds(d.bar, scale));
    let focusables = v.focusables();
    let focus_hit = v.focus.and_then(|i| focusables.get(i).copied());
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    let mut focus_id = None;
    for h in &focusables {
        let id = NodeId(A11Y_BASE + node(*h));
        let mut n = match h {
            Hit::Slider => {
                let mut n = Node::new(Role::Slider);
                n.set_numeric_value(f64::from(v.blend) * 100.0);
                n.set_min_numeric_value(0.0);
                n.set_max_numeric_value(100.0);
                n.set_numeric_value_step(5.0);
                n.add_action(accesskit::Action::Increment);
                n.add_action(accesskit::Action::Decrement);
                n
            }
            Hit::Mode(m) => {
                let mut n = Node::new(Role::Button);
                n.set_toggled(if *m == v.mode {
                    Toggled::True
                } else {
                    Toggled::False
                });
                n
            }
            Hit::Blink => {
                let mut n = Node::new(Role::Button);
                n.set_toggled(if v.blink {
                    Toggled::True
                } else {
                    Toggled::False
                });
                n
            }
            _ => Node::new(Role::Button),
        };
        n.set_label(v.label(*h));
        if let Some((r, _)) = d.hits.iter().find(|(_, x)| x == h) {
            n.set_bounds(bounds(*r, scale));
        }
        if v.enabled(*h) {
            if *h != Hit::Slider {
                n.add_action(accesskit::Action::Click);
            }
        } else {
            n.set_disabled();
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(*h) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    bar_n.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), bar_n));
    tree.nodes.extend(nodes);
    if let Some(id) = focus_id {
        tree.focus = id;
    }
}

// ---------------------------------------------------------------------------------------------
// State.

/// The other sheet.
struct Other {
    path: PathBuf,
    page: u32,
    pages: u32,
    grey: Arc<Grey>,
}

/// Picking two point pairs: this sheet's and the other's points so far (copies' px).
#[derive(Clone, Debug, Default)]
struct Pick {
    this: Vec<P>,
    other: Vec<P>,
}

/// A composed picture.
struct Composite {
    key: u64,
    w: u32,
    h: u32,
    /// Transposed (the view is turned a quarter).
    turned: bool,
    rgba: Option<Vec<u8>>,
}

/// The open comparison.
pub(super) struct Cmp {
    this_path: PathBuf,
    this_page: u32,
    this: Option<Arc<Grey>>,
    /// Copy px per stored px of this sheet.
    this_k: f64,
    other: Option<Other>,
    /// The other copy's pixels onto this copy's.
    t: Option<Similarity>,
    fit: Option<Aligned>,
    mode: Mode,
    blend: f32,
    blink: bool,
    blink_other: bool,
    blink_at: f64,
    pick: Option<Pick>,
    changes: Vec<[f64; 4]>,
    composite: Option<Composite>,
    /// The other sheet warped by `t`, kept for recomposing.
    warped: Option<(Similarity, Arc<Grey>)>,
    composing: Option<u64>,
    busy: Option<String>,
    note: Option<String>,
    focus: Option<usize>,
    drawn: Drawn,
    slider_drag: bool,
}

enum Done {
    This {
        path: PathBuf,
        page: u32,
        r: Result<(Grey, f64), String>,
    },
    Other {
        path: PathBuf,
        page: u32,
        r: Result<(Grey, u32), String>,
    },
    Aligned(Option<Aligned>),
    Composed {
        key: u64,
        w: u32,
        h: u32,
        turned: bool,
        rgba: Vec<u8>,
        warped: (Similarity, Arc<Grey>),
        changes: Vec<[f64; 4]>,
    },
}

/// The texture of the composed picture.
struct Tex {
    _tex: wgpu::Texture,
    id: TexId,
    key: u64,
    turned: bool,
}

pub(super) struct CompareState {
    pub open: Option<Cmp>,
    tex: Option<Tex>,
    retired: Vec<TexId>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
}

impl Default for CompareState {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        CompareState {
            open: None,
            tex: None,
            retired: Vec::new(),
            tx,
            rx,
        }
    }
}

impl std::fmt::Debug for CompareState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompareState")
            .field("open", &self.open.is_some())
            .finish_non_exhaustive()
    }
}

impl CompareState {
    /// The GPU device was replaced: the picture is composed and uploaded again.
    pub(super) fn gpu_reset(&mut self) {
        self.tex = None;
        self.retired.clear();
        if let Some(o) = self.open.as_mut() {
            o.composite = None;
        }
    }

    /// Uploads a composed picture that arrived, and frees retired textures.
    pub(super) fn upload(&mut self, gpu: &Gpu, ui: &mut UiRenderer) {
        for id in self.retired.drain(..) {
            ui.unregister_texture(id);
        }
        let Some(c) = self.open.as_mut().and_then(|o| o.composite.as_mut()) else {
            if self.open.is_none()
                && let Some(t) = self.tex.take()
            {
                ui.unregister_texture(t.id);
            }
            return;
        };
        let Some(rgba) = c.rgba.take() else { return };
        let (tex, id) = upload(gpu, ui, c.w, c.h, &rgba);
        if let Some(old) = self.tex.replace(Tex {
            _tex: tex,
            id,
            key: c.key,
            turned: c.turned,
        }) {
            ui.unregister_texture(old.id);
        }
    }

    pub(super) fn set_drawn(&mut self, d: Drawn) {
        if let Some(o) = self.open.as_mut() {
            o.drawn = d;
        }
    }

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &CompareView, scale: f64) {
        if let Some(o) = &self.open {
            a11y(tree, v, &o.drawn, scale);
        }
    }
}

fn upload(gpu: &Gpu, ui: &mut UiRenderer, w: u32, h: u32, rgba: &[u8]) -> (wgpu::Texture, TexId) {
    let size = wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sheet compare"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * w),
            rows_per_image: None,
        },
        size,
    );
    let view = tex.create_view(&Default::default());
    let id = ui.register_texture(gpu, &view);
    (tex, id)
}

/// A session's page as a grey copy of about [`SIDE`] px, and its px per stored px.
fn grey_of(s: &dyn specular_decode::Session) -> Result<(Grey, f64), String> {
    let info = s.info();
    let long = info.width.max(info.height).max(1) as usize;
    // The coarsest scale still at least SIDE long.
    let scale = info
        .scales
        .iter()
        .copied()
        .filter(|&k| long / k as usize >= SIDE || k == 1)
        .max()
        .unwrap_or(1);
    let r = s
        .decode(
            &specular_decode::DecodeRequest::full(scale),
            &CancelToken::new(),
        )
        .map_err(|e| e.to_string())?;
    let rgba = crate::clipboard::to_rgba(&r);
    let g = Grey::from_rgba(
        r.width as usize,
        r.height as usize,
        &rgba,
        r.width as usize * 4,
    );
    let (g, k) = g.reduce(SIDE);
    Ok((g, k * f64::from(r.width) / f64::from(info.width.max(1))))
}

fn key_of(mode: Mode, blend: f32, t: &Similarity, turned: bool, other: &Path, page: u32) -> u64 {
    let mut h = DefaultHasher::new();
    mode.hash(&mut h);
    ((blend * 100.0).round() as i32).hash(&mut h);
    for v in [t.a, t.b, t.tx, t.ty] {
        v.to_bits().hash(&mut h);
    }
    turned.hash(&mut h);
    other.hash(&mut h);
    page.hash(&mut h);
    h.finish()
}

/// Whether `view` shows stored x along the screen's y (turned a quarter).
fn turned(view: &specular_render::View) -> bool {
    let m = view.stored_from_screen().invert().m;
    m[0].abs() < m[1].abs()
}

/// The composed picture's texture and where it goes (logical px), with its uv.
pub(super) fn picture(
    st: &CompareState,
    view: &specular_render::View,
    dl_scale: f32,
) -> Option<(TexId, Rect, [f32; 4])> {
    let t = st.tex.as_ref()?;
    let o = st.open.as_ref()?;
    let c = o.composite.as_ref()?;
    if c.key != t.key || t.turned != turned(view) {
        return None;
    }
    let (w, h) = view.stored_size();
    let a = view.stored_from_screen().invert();
    let s = dl_scale.max(0.1);
    let p0 = a.apply((0.0, 0.0));
    let p1 = a.apply((w as f32, h as f32));
    let r = Rect::from_min_max(
        p0.0.min(p1.0) / s,
        p0.1.min(p1.1) / s,
        p0.0.max(p1.0) / s,
        p0.1.max(p1.1) / s,
    );
    // Flips: the texture read backwards along that axis.
    let m = a.m;
    let (fx, fy) = if t.turned {
        (m[1] < 0.0, m[3] < 0.0)
    } else {
        (m[0] < 0.0, m[4] < 0.0)
    };
    let uv = [
        if fx { 1.0 } else { 0.0 },
        if fy { 1.0 } else { 0.0 },
        if fx { -1.0 } else { 1.0 },
        if fy { -1.0 } else { 1.0 },
    ];
    Some((t.id, r, uv))
}

impl App {
    /// Opens the comparison over the current sheet.
    pub(super) fn sheets_open(&mut self) {
        if self.sheets.open.is_some() {
            return;
        }
        let Some(id) = self.current_id() else { return };
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let Some(session) = img.session.clone() else {
            return;
        };
        let (path, page) = (img.path.clone(), img.page);
        self.sheets.open = Some(Cmp {
            this_path: path.clone(),
            this_page: page,
            this: None,
            this_k: 1.0,
            other: None,
            t: None,
            fit: None,
            mode: Mode::Overlay,
            blend: 0.5,
            blink: false,
            blink_other: false,
            blink_at: 0.0,
            pick: None,
            changes: Vec::new(),
            composite: None,
            warped: None,
            composing: None,
            busy: Some(t!("sheets-reading")),
            note: None,
            focus: None,
            drawn: Drawn::default(),
            slider_drag: false,
        });
        let tx = self.sheets.tx.clone();
        self.loader.background("sheets-this", move |l| {
            let r = grey_of(&*session);
            let _ = tx.send(Done::This { path, page, r });
            l.waker()();
        });
        self.chrome.announce_status(t!("sheets-opened"));
        self.redraw = true;
        // A PDF with more pages: its next page is the other sheet to start with.
        let pages = self.images.get(&id).map_or(1, |i| i.pages).max(1);
        if pages > 1 {
            let p = self.images.get(&id).map(|i| i.path.clone());
            if let Some(p) = p {
                let next = (page + 1) % pages;
                self.sheets_load_other(p, next);
            }
        }
    }

    fn sheets_close(&mut self) {
        self.sheets.open = None;
        self.chrome.announce_status(t!("sheets-closed"));
        self.redraw = true;
    }

    /// Reads the other sheet's page `page`.
    fn sheets_load_other(&mut self, path: PathBuf, page: u32) {
        let registry = self.loader.registry();
        if let Some(o) = self.sheets.open.as_mut() {
            o.busy = Some(t!("sheets-reading"));
        }
        let tx = self.sheets.tx.clone();
        self.loader.background("sheets-other", move |l| {
            let r = registry
                .open_path(&path)
                .and_then(|s| if page > 0 { s.open_page(page) } else { Ok(s) })
                .map_err(|e| e.to_string())
                .and_then(|s| {
                    let pages = s.page_count().max(1);
                    grey_of(&*s).map(|(g, _)| (g, pages))
                });
            let _ = tx.send(Done::Other { path, page, r });
            l.waker()();
        });
    }

    fn sheets_pick_file(&mut self) {
        let Some(o) = self.sheets.open.as_ref() else {
            return;
        };
        let dir = o.this_path.parent().map(Path::to_path_buf);
        let registry = self.loader.registry();
        let tx = self.sheets.tx.clone();
        self.loader.background("sheets-pick", move |l| {
            let mut dlg = rfd::FileDialog::new();
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let Some(path) = dlg.pick_file() else { return };
            let r = registry
                .open_path(&path)
                .map_err(|e| e.to_string())
                .and_then(|s| {
                    let pages = s.page_count().max(1);
                    grey_of(&*s).map(|(g, _)| (g, pages))
                });
            let _ = tx.send(Done::Other { path, page: 0, r });
            l.waker()();
        });
    }

    /// Aligns the other sheet (from `start` when given) in the background.
    fn sheets_align(&mut self, start: Option<Similarity>) {
        let Some(o) = self.sheets.open.as_mut() else {
            return;
        };
        let (Some(a), Some(b)) = (o.this.clone(), o.other.as_ref().map(|x| x.grey.clone())) else {
            return;
        };
        o.busy = Some(t!("sheets-aligning"));
        let tx = self.sheets.tx.clone();
        self.loader.background("sheets-align", move |l| {
            let r = auto(&a, &b, start);
            let _ = tx.send(Done::Aligned(r));
            l.waker()();
        });
        self.redraw = true;
    }

    fn sheets_turned(&self) -> bool {
        turned(&self.view)
    }

    /// Composes the picture for the current settings when it is not there yet.
    fn sheets_compose(&mut self) {
        let turned = self.sheets_turned();
        let Some(o) = self.sheets.open.as_mut() else {
            return;
        };
        let (Some(a), Some(other), Some(t)) = (o.this.clone(), o.other.as_ref(), o.t) else {
            return;
        };
        let (mode, blend) = if o.pick.is_some() {
            let stage = o.pick.as_ref().map_or(0, |p| p.this.len() + p.other.len());
            (Mode::Blend, if stage % 2 == 0 { 0.0 } else { 1.0 })
        } else if o.blink {
            (Mode::Blend, if o.blink_other { 1.0 } else { 0.0 })
        } else {
            (o.mode, o.blend)
        };
        let key = key_of(mode, blend, &t, turned, &other.path, other.page);
        if o.composite.as_ref().is_some_and(|c| c.key == key) || o.composing == Some(key) {
            return;
        }
        o.composing = Some(key);
        let b = other.grey.clone();
        let warped = o.warped.clone().filter(|(wt, _)| *wt == t);
        let tx = self.sheets.tx.clone();
        self.loader.background("sheets-compose", move |l| {
            let w = match warped {
                Some((_, w)) => w,
                None => Arc::new(warp(&b, &t, a.w, a.h)),
            };
            let view = match mode {
                Mode::Overlay => View::Overlay,
                Mode::Difference => View::Difference,
                Mode::Blend => View::Blend(blend),
            };
            let mut rgba = compose(&a, &w, view);
            let (mut cw, mut ch) = (a.w as u32, a.h as u32);
            if turned {
                // Transposed: the texture's x is the sheet's y.
                let mut tr = vec![0u8; rgba.len()];
                for y in 0..a.h {
                    for x in 0..a.w {
                        let s = (y * a.w + x) * 4;
                        let d = (x * a.h + y) * 4;
                        tr[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
                    }
                }
                rgba = tr;
                std::mem::swap(&mut cw, &mut ch);
            }
            let found = changes(&a, &w, SLACK, MERGE, MIN_CHANGED);
            let _ = tx.send(Done::Composed {
                key,
                w: cw,
                h: ch,
                turned,
                rgba,
                warped: (t, w),
                changes: found,
            });
            l.waker()();
        });
    }

    pub(super) fn sheets_tick(&mut self) {
        let now = self.now();
        // Leaving the sheet ends the comparison.
        if let Some(o) = self.sheets.open.as_ref() {
            let same = self
                .current_id()
                .and_then(|id| self.images.get(&id))
                .is_some_and(|i| i.path == o.this_path && i.page == o.this_page);
            if !same {
                self.sheets.open = None;
                self.redraw = true;
            }
        }
        while let Ok(d) = self.sheets.rx.try_recv() {
            self.redraw = true;
            let Some(o) = self.sheets.open.as_mut() else {
                continue;
            };
            match d {
                Done::This { path, page, r } => {
                    if path != o.this_path || page != o.this_page {
                        continue;
                    }
                    o.busy = None;
                    match r {
                        Ok((g, k)) => {
                            o.this = Some(Arc::new(g));
                            o.this_k = k;
                        }
                        Err(e) => o.note = Some(t!("sheets-read-failed", reason = e)),
                    }
                    if o.other.is_some() {
                        self.sheets_align(None);
                    }
                }
                Done::Other { path, page, r } => {
                    o.busy = None;
                    match r {
                        Ok((g, pages)) => {
                            o.other = Some(Other {
                                path,
                                page,
                                pages,
                                grey: Arc::new(g),
                            });
                            o.t = None;
                            o.fit = None;
                            o.warped = None;
                            o.composite = None;
                            o.changes.clear();
                            o.pick = None;
                            if o.this.is_some() {
                                self.sheets_align(None);
                            }
                        }
                        Err(e) => o.note = Some(t!("sheets-read-failed", reason = e)),
                    }
                }
                Done::Aligned(r) => {
                    o.busy = None;
                    match r {
                        Some(a) if a.matches >= 3 => {
                            o.t = Some(a.t);
                            o.fit = Some(a);
                            o.note = None;
                        }
                        r => {
                            // No good match: the user's pairs as they are, or sizes alone.
                            let keep = r.map(|a| a.t).or(o.t);
                            o.t = keep.or_else(|| {
                                let (a, b) = (o.this.as_ref()?, &o.other.as_ref()?.grey);
                                Some(Similarity::new(
                                    a.w as f64 / b.w.max(1) as f64,
                                    0.0,
                                    0.0,
                                    0.0,
                                ))
                            });
                            o.fit = None;
                            o.note = Some(t!("sheets-align-failed"));
                        }
                    }
                    let msg = self.sheets_view().map(|v| v.status).unwrap_or_default();
                    self.chrome.announce_status(msg);
                }
                Done::Composed {
                    key,
                    w,
                    h,
                    turned,
                    rgba,
                    warped,
                    changes,
                } => {
                    if o.composing == Some(key) {
                        o.composing = None;
                    }
                    o.warped = Some(warped);
                    o.changes = changes;
                    o.composite = Some(Composite {
                        key,
                        w,
                        h,
                        turned,
                        rgba: Some(rgba),
                    });
                }
            }
        }
        if let Some(o) = self.sheets.open.as_mut()
            && o.blink
            && now >= o.blink_at
        {
            o.blink_other = !o.blink_other;
            o.blink_at = now + BLINK_S;
            self.redraw = true;
        }
        if self.sheets.open.is_some() {
            self.sheets_compose();
        }
    }

    /// When the blink next turns.
    pub(super) fn sheets_wake(&self, now: f64) -> Option<f64> {
        let o = self.sheets.open.as_ref()?;
        o.blink.then(|| (o.blink_at - now).max(0.0))
    }

    /// A copy point of this sheet on screen (logical px).
    fn sheets_screen(&self, p: P) -> (f32, f32) {
        let k = self
            .sheets
            .open
            .as_ref()
            .map_or(1.0, |o| o.this_k)
            .max(1e-9);
        let a = self.view.stored_from_screen().invert();
        let s = self.dl.scale.max(0.1);
        let q = a.apply(((p.x / k) as f32, (p.y / k) as f32));
        (q.0 / s, q.1 / s)
    }

    pub(super) fn sheets_view(&self) -> Option<CompareView> {
        let o = self.sheets.open.as_ref()?;
        let regions = if o.pick.is_none() {
            o.changes
                .iter()
                .map(|r| {
                    let a = self.sheets_screen(P::new(r[0], r[1]));
                    let b = self.sheets_screen(P::new(r[2], r[3]));
                    Rect::from_min_max(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
                })
                .collect()
        } else {
            Vec::new()
        };
        let status = if let Some(b) = &o.busy {
            b.clone()
        } else if let Some(p) = &o.pick {
            match p.this.len() + p.other.len() {
                0 => t!("sheets-pick-this", n = 1),
                1 => t!("sheets-pick-other", n = 1),
                2 => t!("sheets-pick-this", n = 2),
                _ => t!("sheets-pick-other", n = 2),
            }
        } else if let Some(n) = &o.note {
            n.clone()
        } else if o.other.is_none() {
            t!("sheets-choose")
        } else if let Some(f) = &o.fit {
            t!(
                "sheets-aligned",
                scale = specular_i18n::fixed(f.t.scale(), 4),
                angle = specular_i18n::fixed(f.t.angle().to_degrees(), 2),
                error = specular_i18n::fixed(f.rms / o.this_k.max(1e-9), 2),
                changes = o.changes.len() as i64
            )
        } else {
            String::new()
        };
        Some(CompareView {
            other: o.other.as_ref().map(|x| {
                x.path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            }),
            page: o.other.as_ref().map(|x| (x.page, x.pages)),
            mode: o.mode,
            blend: o.blend,
            blink: o.blink,
            aligned: o.t.is_some(),
            changes: o.changes.len(),
            status,
            focus: o.focus,
            regions,
        })
    }

    fn sheets_hit(&mut self, h: Hit) {
        let now = self.now();
        match h {
            Hit::Other => self.sheets_pick_file(),
            Hit::Page => {
                let next = self
                    .sheets
                    .open
                    .as_ref()
                    .and_then(|o| o.other.as_ref())
                    .map(|x| (x.path.clone(), (x.page + 1) % x.pages.max(1)));
                if let Some((p, n)) = next {
                    self.sheets_load_other(p, n);
                }
            }
            Hit::Auto => {
                // From the user's pairs when there are some, else from scratch.
                let start = self
                    .sheets
                    .open
                    .as_ref()
                    .filter(|o| o.fit.is_none())
                    .and_then(|o| o.t);
                self.sheets_align(start);
            }
            Hit::Pick => {
                if let Some(o) = self.sheets.open.as_mut() {
                    o.pick = Some(Pick::default());
                    o.blink = false;
                }
                self.chrome.announce_status(t!("sheets-pick-this", n = 1));
            }
            Hit::Mode(m) => {
                if let Some(o) = self.sheets.open.as_mut() {
                    o.mode = m;
                    o.blink = false;
                }
                self.chrome.announce_status(match m {
                    Mode::Overlay => t!("sheets-overlay"),
                    Mode::Difference => t!("sheets-difference"),
                    Mode::Blend => t!("sheets-blend"),
                });
            }
            Hit::Blink => {
                if let Some(o) = self.sheets.open.as_mut() {
                    o.blink = !o.blink;
                    o.blink_other = false;
                    o.blink_at = now + BLINK_S;
                }
            }
            Hit::Slider => {}
            Hit::Clouds => {
                let Some(o) = self.sheets.open.as_ref() else {
                    return;
                };
                let k = o.this_k.max(1e-9);
                let rects: Vec<[f64; 4]> = o
                    .changes
                    .iter()
                    .map(|r| [r[0] / k, r[1] / k, r[2] / k, r[3] / k])
                    .collect();
                let n = self.markup_add_clouds(&rects, &t!("sheets-cloud-text"));
                self.chrome
                    .toast(t!("sheets-clouds-added", count = n as i64), None, now);
            }
            Hit::Done => self.sheets_close(),
        }
        self.redraw = true;
    }

    fn sheets_set_blend(&mut self, v: f32) {
        if let Some(o) = self.sheets.open.as_mut() {
            o.blend = (v.clamp(0.0, 1.0) * 20.0).round() / 20.0;
            o.mode = Mode::Blend;
            o.blink = false;
        }
        self.redraw = true;
    }

    /// A click on the sheet while picking point pairs.
    fn sheets_pick_at(&mut self, p: (f32, f32)) {
        let (x, y) = self.view.stored_from_screen().apply(p);
        let Some(o) = self.sheets.open.as_mut() else {
            return;
        };
        let Some(pick) = o.pick.as_mut() else { return };
        let q = P::new(f64::from(x) * o.this_k, f64::from(y) * o.this_k);
        let stage = pick.this.len() + pick.other.len();
        if stage % 2 == 0 {
            pick.this.push(q);
        } else {
            // Shown through the current alignment: back to the other copy's px.
            let t = o.t.unwrap_or(Similarity::IDENTITY);
            pick.other.push(t.inverse().apply(q));
        }
        let stage = stage + 1;
        if stage < 4 {
            let msg = if stage % 2 == 0 {
                t!("sheets-pick-this", n = 2)
            } else {
                t!("sheets-pick-other", n = (stage / 2 + 1) as i64)
            };
            self.chrome.announce_status(msg);
            self.redraw = true;
            return;
        }
        let pick = o.pick.take().unwrap_or_default();
        let t = Similarity::from_pairs(pick.this[0], pick.this[1], pick.other[0], pick.other[1]);
        match t {
            Some(t) => {
                o.t = Some(t);
                o.fit = None;
                o.composite = None;
                self.sheets_align(Some(t));
            }
            None => {
                o.note = Some(t!("sheets-pick-same"));
            }
        }
        self.redraw = true;
    }

    pub(super) fn sheets_key(&mut self, combo: Option<&KeyCombo>, el: &ActiveEventLoop) -> bool {
        let Some(v) = self.sheets_view() else {
            return false;
        };
        let Some(combo) = combo else { return true };
        let focusables = v.focusables();
        let n = focusables.len();
        let focus = v.focus.and_then(|i| focusables.get(i).copied());
        let mods = combo.mods;
        let move_focus = |me: &mut App, d: i32| {
            if let Some(o) = me.sheets.open.as_mut() {
                let i = o.focus.map_or(if d > 0 { 0 } else { n - 1 }, |i| {
                    (i as i32 + d).rem_euclid(n as i32) as usize
                });
                o.focus = Some(i);
            }
            if let Some(v) = me.sheets_view()
                && let Some(h) = v.focus.and_then(|i| v.focusables().get(i).copied())
            {
                me.chrome.announce_status(v.label(h));
            }
        };
        let rtl = self.rtl();
        match combo.key {
            Key::Named(NamedKey::Escape) => {
                let o = self.sheets.open.as_mut();
                match o {
                    Some(o) if o.pick.is_some() => o.pick = None,
                    Some(o) if o.focus.is_some() => o.focus = None,
                    _ => self.sheets_close(),
                }
            }
            Key::Named(NamedKey::Tab) if mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight)
                if focus == Some(Hit::Slider) =>
            {
                let right = combo.key == Key::Named(NamedKey::ArrowRight);
                let up = right != rtl;
                let step = if mods.shift { 0.25 } else { 0.05 };
                self.sheets_set_blend(v.blend + if up { step } else { -step });
                let msg = self
                    .sheets_view()
                    .map(|v| v.label(Hit::Slider))
                    .unwrap_or_default();
                self.chrome.announce_status(msg);
            }
            Key::Named(NamedKey::Enter | NamedKey::Space) if focus.is_some() => {
                if let Some(h) = focus
                    && v.enabled(h)
                {
                    self.sheets_hit(h);
                }
            }
            Key::Char('b' | 'B') if !mods.ctrl && !mods.alt => self.sheets_hit(Hit::Blink),
            Key::Char('d' | 'D') if !mods.ctrl && !mods.alt => {
                self.sheets_hit(Hit::Mode(Mode::Difference));
            }
            Key::Char('o' | 'O') if !mods.ctrl && !mods.alt => {
                self.sheets_hit(Hit::Mode(Mode::Overlay));
            }
            Key::Char('a' | 'A') if !mods.ctrl && !mods.alt => self.sheets_hit(Hit::Auto),
            _ => {
                if let Some(a) = self.keymap.lookup(combo)
                    && (a.group() == Group::ZoomPan
                        || a == Action::ToggleFullscreen
                        || a == Action::CompareSheets)
                {
                    if a == Action::CompareSheets {
                        self.sheets_close();
                    } else {
                        self.action(a, el);
                    }
                }
            }
        }
        self.redraw = true;
        true
    }

    /// The left button while comparing; true when taken.
    pub(super) fn sheets_button(&mut self, p: (f32, f32), lp: (f32, f32), down: bool) -> bool {
        let Some(o) = self.sheets.open.as_mut() else {
            return false;
        };
        if !down {
            o.slider_drag = false;
            return o.drawn.bar.contains(lp);
        }
        let o = &*o;
        if let Some(h) = o
            .drawn
            .hits
            .iter()
            .find(|(r, _)| r.contains(lp))
            .map(|x| x.1)
        {
            if h == Hit::Slider {
                self.sheets_slide(lp);
                if let Some(o) = self.sheets.open.as_mut() {
                    o.slider_drag = true;
                }
            } else if self.sheets_view().is_some_and(|v| v.enabled(h)) {
                self.sheets_hit(h);
            }
            return true;
        }
        if o.drawn.bar.contains(lp) {
            return true;
        }
        if o.pick.is_some() {
            self.sheets_pick_at(p);
            return true;
        }
        false
    }

    fn sheets_slide(&mut self, lp: (f32, f32)) {
        let rtl = self.rtl();
        let Some(tr) = self.sheets.open.as_ref().and_then(|o| o.drawn.track) else {
            return;
        };
        let mut v = ((lp.0 - tr.x) / tr.w.max(1.0)).clamp(0.0, 1.0);
        if rtl {
            v = 1.0 - v;
        }
        self.sheets_set_blend(v);
    }

    /// Pointer motion while comparing: the slider follows a drag. True when taken.
    pub(super) fn sheets_motion(&mut self, p: (f32, f32)) -> bool {
        let dragging = self.sheets.open.as_ref().is_some_and(|o| o.slider_drag);
        if dragging {
            let lp = self.logical(p);
            self.sheets_slide(lp);
        }
        dragging
    }

    pub(super) fn sheets_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        if self.sheets.open.is_none() || req.target_node.0 & !0xFFFF != A11Y_BASE {
            return false;
        }
        if let Some(v) = self.sheets_view()
            && let Some(h) = target(req.target_node.0, &v)
        {
            match req.action {
                accesskit::Action::Click if v.enabled(h) => self.sheets_hit(h),
                accesskit::Action::Increment => self.sheets_set_blend(v.blend + 0.05),
                accesskit::Action::Decrement => self.sheets_set_blend(v.blend - 0.05),
                accesskit::Action::Focus => {
                    if let Some(i) = v.focusables().iter().position(|x| *x == h)
                        && let Some(o) = self.sheets.open.as_mut()
                    {
                        o.focus = Some(i);
                    }
                }
                _ => {}
            }
        }
        self.redraw = true;
        true
    }

    /// A background task panicked: nothing waits for it any more.
    pub(super) fn sheets_lost(&mut self, task: &str) {
        if task.starts_with("sheets-")
            && let Some(o) = self.sheets.open.as_mut()
        {
            o.busy = None;
            o.composing = None;
            o.note = Some(t!("sheets-failed"));
            self.redraw = true;
        }
    }

    /// `--show compare-sheets` (screenshots): the comparison with the PDF's next page.
    pub(super) fn sheets_show(&mut self, what: &str) -> bool {
        if what != "compare-sheets" {
            return false;
        }
        self.sheets_open();
        true
    }

    /// Nothing in flight (screenshots wait for this).
    pub(super) fn sheets_settled(&self) -> bool {
        let Some(o) = &self.sheets.open else {
            return true;
        };
        if o.busy.is_some() || o.composing.is_some() || o.this.is_none() {
            return false;
        }
        match (&o.other, &o.composite) {
            (Some(_), Some(c)) => c.rgba.is_none() && o.t.is_some(),
            (Some(_), None) => o.t.is_none() && o.note.is_some(),
            (None, _) => true,
        }
    }

    pub(super) fn sheets_action(&mut self, a: Action) -> bool {
        match a {
            Action::CompareSheets => {
                if self.sheets.open.is_some() {
                    self.sheets_close();
                } else {
                    self.sheets_open();
                }
            }
            _ => return false,
        }
        true
    }
}

/// The colour a composed pixel is drawn as, for the goldens: the composite as rectangles of
/// runs of equal colour, `cell` logical px per pixel.
#[cfg(test)]
pub(super) fn draw_rgba(
    dl: &mut DrawList,
    rgba: &[u8],
    w: usize,
    h: usize,
    at: (f32, f32),
    cell: f32,
) {
    for y in 0..h {
        let mut x = 0;
        while x < w {
            let i = (y * w + x) * 4;
            let c = [rgba[i], rgba[i + 1], rgba[i + 2]];
            let mut e = x + 1;
            while e < w && rgba[(y * w + e) * 4..(y * w + e) * 4 + 3] == c {
                e += 1;
            }
            if c != [255, 255, 255] {
                dl.rect(
                    Rect::new(
                        at.0 + x as f32 * cell,
                        at.1 + y as f32 * cell,
                        (e - x) as f32 * cell,
                        cell,
                    ),
                    0.0,
                    Color::rgba(
                        f32::from(c[0]) / 255.0,
                        f32::from(c[1]) / 255.0,
                        f32::from(c[2]) / 255.0,
                        1.0,
                    ),
                );
            }
            x = e;
        }
    }
}
