//! Moving through a model and reviewing it: walk and fly, section planes and the section box
//! with hatched caps and measuring on the cut, saved views (a sidecar next to the model) and
//! BCF topics.
//!
//! - Walk (`Alt+W`) and fly (`Alt+F`): W A S D or the arrows move, a drag looks around,
//!   Shift goes faster, Q and E (or Page Down and Page Up) go down and up when flying; Esc or
//!   the same key returns to the orbit camera. Walking keeps the eye 1.6 m over the floor
//!   under the feet, climbs steps up to 0.4 m and stops at walls ([`specular_render_3d::nav`],
//!   against a [`NavScene`] built once per model off the window thread; until it is ready,
//!   the walker stays on the model box's floor and passes through walls).
//! - Sections: `Alt+X` adds a plane (up to six) facing the view through the model's centre,
//!   `Shift+X` turns the section box on and off, `Alt+Z` flips the chosen plane,
//!   `Alt+Shift+X` removes every cut. Each plane and box face has a handle to drag along its
//!   normal. The cut is closed by a cap per material, hatched. `Shift+M` measures on the cut:
//!   two clicks, snapped to the cut's outline.
//! - Views (`Alt+P`): `Shift+S` saves the camera, sections and hidden elements under a name;
//!   in the list Enter goes there (animated), F2 renames, Delete removes (after asking), P
//!   saves the view as a PNG. Views are kept in `<model>.specular-views.json` next to the
//!   model; where that cannot be written they last for the session.
//! - BCF (`Alt+I`): O opens a `.bcfzip` or `.bcf`; the list shows each topic's title,
//!   status, priority, assignee and comments; Enter jumps to its viewpoint (camera,
//!   clipping planes, and on an IFC model its components' selection, visibility and
//!   colours by GUID), ← → step through its viewpoints, N adds a topic and C a comment with
//!   the current view and a snapshot, S steps the status, E edits the title, Ctrl+S exports
//!   a `.bcfzip`. The model is never written.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};
use specular_bim::apply::{self, Frame, MeshCamera};
use specular_bim::bcf::{
    self as bcf, Archive, Comment, Components, Topic, Version, Viewpoint, VisInfo,
};
use specular_bim::views::{self, SavedView, ViewsFile};
use specular_decode::MeshModel;
use specular_decode::specular_mesh::Bounds;
use specular_gfx::winit::event::{ElementState, KeyEvent};
use specular_gfx::winit::keyboard::{KeyCode, PhysicalKey};
use specular_gfx::{Align, DrawList, Rect, Text, TextBlock, TextStyle};
use specular_i18n::{Direction, I18n, join_parts, t};
use specular_input::{Action, Key, KeyCombo, NamedKey};
use specular_render_3d::bvh::NavScene;
use specular_render_3d::section::{self, Caps};
use specular_render_3d::{
    Body, Camera, MAX_PLANES, MeshGpu, NAV_FOV, Nav, NavInput, NavMode, Pose, Projection,
    SectionPlane, Sections, Shading,
};
use specular_theme::Palette;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, TOOLBAR_MARGIN};
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

use super::{MeshView, size_line};
use crate::app::App;

/// "Remove this view?" (the confirmation's item).
pub(in crate::app) const VIEW_DELETE_ITEM: u64 = crate::app::items::BIM_VIEW_DELETE;

/// AccessKit ids: the panel, its rows, the prompt (the readout uses 0x5E00…).
const A11Y_PANEL: u64 = 0x5E10_0000_0000_0000;
const A11Y_ROW: u64 = A11Y_PANEL + 0x100;
const A11Y_PROMPT: u64 = A11Y_PANEL + 0x10;
const A11Y_HANDLE: u64 = A11Y_PANEL + 0x20;

/// The panel's width (logical px) and rows shown at most.
const PANEL_W: f32 = 360.0;
pub(super) const MAX_ROWS: usize = 12;
/// Characters of a row before it is cut.
const ROW_CHARS: usize = 52;
/// A handle's radius and how near a press must be (logical px).
const HANDLE_R: f32 = 7.0;
const HANDLE_HIT: f32 = 12.0;
/// Radians turned per logical px dragged while walking or flying, and per second by keys.
const LOOK_PER_PX: f64 = 0.004;
const TURN_RATE: f64 = 1.6;
/// How long going to a view takes (s).
const ANIM_S: f64 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum PanelKind {
    Views,
    Topics,
    /// Model checks: IDS, clashes, changes, quantities.
    Checks,
    /// The models in the scene.
    Models,
    /// A CAD model's PMI and saved views.
    Pmi,
}

/// A drag handle: a section plane or a face of the section box (+x, −x, +y, −y, +z, −z).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Handle {
    Plane(usize),
    BoxFace(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asking {
    NewView,
    Rename(usize),
    NewTopic,
    Comment,
    Title,
}

struct Prompt {
    asking: Asking,
    field: LineEdit,
    /// An input method's composition at the caret.
    preedit: String,
}

/// What the next captured frame is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Capture {
    /// Saved as a PNG (the user picks where).
    Png,
    /// The snapshot of a topic's viewpoint.
    Snapshot { topic: usize, viewpoint: usize },
}

enum Done {
    Opened(PathBuf, Result<Box<Archive>, String>),
    Exported(Result<PathBuf, String>),
    Png(Result<PathBuf, String>),
}

#[derive(Clone, Copy, Debug, Default)]
struct Held {
    fwd: bool,
    back: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    turn_l: bool,
    turn_r: bool,
    fast: bool,
}

impl Held {
    fn any(&self) -> bool {
        self.fwd
            || self.back
            || self.left
            || self.right
            || self.up
            || self.down
            || self.turn_l
            || self.turn_r
    }
}

struct Anim {
    from: Pose,
    to: Pose,
    t0: Instant,
    sections: Option<Sections>,
}

/// The open BCF archive.
struct Bcf {
    path: Option<PathBuf>,
    archive: Archive,
    /// Changed since it was opened or exported.
    dirty: bool,
    /// The chosen topic's viewpoint shown.
    viewpoint: usize,
}

/// Measuring on the cut: the first point, then the measured pair.
#[derive(Clone, Copy, Debug, Default)]
struct Measure {
    first: Option<[f64; 3]>,
    done: Option<([f64; 3], [f64; 3])>,
}

/// A handle as drawn: which, where (logical px), and its normal on screen (logical px per
/// model unit).
type Placed = (Handle, (f32, f32), (f64, f64));

/// A measurement on screen: its ends (logical px) and its label.
pub(in crate::app) type Measured = ((f32, f32), (f32, f32), String);

/// Walk, fly, sections, views and BCF state of the current model.
pub(in crate::app) struct Bim {
    pub nav: Option<Nav>,
    held: Held,
    look: (f64, f64),
    last: Option<Instant>,
    /// What walking collides with, once built (`building` until then).
    scene: Option<Arc<NavScene>>,
    pub(super) building: Option<Receiver<NavScene>>,
    pub sections: Sections,
    /// The handle chosen last (flip acts on it).
    chosen: Option<Handle>,
    caps: Option<Caps>,
    pub(super) caps_gpu: Option<MeshGpu>,
    /// Handles as last drawn: which, where (logical px) and the normal's direction on screen
    /// (logical px per model unit).
    handles: Vec<Placed>,
    drag: Option<(Handle, (f32, f32))>,
    views: ViewsFile,
    sidecar: Option<PathBuf>,
    views_loaded: bool,
    anim: Option<Anim>,
    pub panel: Option<PanelKind>,
    pub(super) row: usize,
    prompt: Option<Prompt>,
    /// The panel as last drawn and its rows (logical px).
    pub(in crate::app) panel_rect: Option<Rect>,
    pub(in crate::app) rows: Vec<(usize, Rect)>,
    /// Where the prompt's input method candidate window opens (as last drawn).
    pub(in crate::app) ime: Option<Rect>,
    /// The prompt's field as last drawn (a press there puts the caret).
    pub(in crate::app) field: Option<PromptDrawn>,
    bcf: Option<Bcf>,
    /// The model restyled by a BCF viewpoint's components (uploaded in its place).
    pub(super) restyled: Option<MeshModel>,
    /// Elements hidden by the view or viewpoint shown (IFC GUIDs).
    hidden: Vec<String>,
    measure: Option<Measure>,
    pub(in crate::app) capture: Option<Capture>,
    /// IDS, clashes, changes and quantities of an IFC model.
    pub(super) checks: super::checks::Checks,
    /// The other IFC models in the scene, when any were added.
    pub(super) fed: super::federation::Fed,
    tx: Sender<Done>,
    rx: Receiver<Done>,
}

impl Bim {
    /// A press at `lp` (logical px; `now` in seconds, `shift` extends) in the prompt's
    /// field: the caret goes there. False when no prompt is open or the press is not in it.
    fn prompt_press(&mut self, lp: (f32, f32), now: f64, shift: bool) -> bool {
        let (Some(p), Some((r, block, x0))) = (self.prompt.as_mut(), &self.field) else {
            return false;
        };
        if !r.contains(lp) {
            return false;
        }
        let at = block.hit_byte(lp.0 - x0).min(p.field.text().len());
        p.field.press(at, lp.0, now, shift);
        true
    }

    /// The pointer at `x` with the button held after a press in the prompt's field: the
    /// selection runs from the press to there. False when no press is being dragged.
    fn prompt_drag(&mut self, x: f32) -> bool {
        let (Some(p), Some((_, block, x0))) = (self.prompt.as_mut(), &self.field) else {
            return false;
        };
        if !p.field.dragging() {
            return false;
        }
        let at = block.hit_byte(x - x0).min(p.field.text().len());
        p.field.drag_to(at);
        true
    }

    /// Where the prompt's input method candidate window opens.
    pub(in crate::app) fn ime_area(&self) -> Option<Rect> {
        self.prompt.as_ref().and(self.ime)
    }
}

impl Default for Bim {
    fn default() -> Bim {
        let (tx, rx) = crossbeam_channel::unbounded();
        Bim {
            nav: None,
            held: Held::default(),
            look: (0.0, 0.0),
            last: None,
            scene: None,
            building: None,
            sections: Sections::default(),
            chosen: None,
            caps: None,
            caps_gpu: None,
            handles: Vec::new(),
            drag: None,
            views: ViewsFile::new(""),
            sidecar: None,
            views_loaded: false,
            anim: None,
            panel: None,
            row: 0,
            prompt: None,
            panel_rect: None,
            field: None,
            rows: Vec::new(),
            ime: None,
            bcf: None,
            restyled: None,
            hidden: Vec::new(),
            measure: None,
            capture: None,
            checks: super::checks::Checks::default(),
            fed: super::federation::Fed::default(),
            tx,
            rx,
        }
    }
}

impl Bim {
    /// Kept across files: the panel shown and the open BCF archive (it is about a project,
    /// not one file).
    pub(super) fn carry(&mut self) -> Bim {
        Bim {
            panel: self.panel,
            bcf: self.bcf.take(),
            checks: self.checks.carry(),
            ..Bim::default()
        }
    }

    pub fn nav_mode(&self) -> NavMode {
        self.nav.map_or(NavMode::Orbit, |n| n.mode)
    }

    pub(super) fn sections_changed(&mut self) {
        self.caps = None;
        self.caps_gpu = None;
    }
}

/// One row of the panel.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct PanelRow {
    pub index: usize,
    pub text: String,
    pub detail: Option<String>,
}

/// The panel as drawn.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct PanelView {
    pub title: String,
    pub rows: Vec<PanelRow>,
    pub empty: Option<String>,
    pub chosen: Option<usize>,
    /// Lines under the list: the chosen topic's comments.
    pub notes: Vec<String>,
    pub hints: String,
    /// The prompt: its label, text, caret (byte), an input method's composition there and
    /// the selected bytes.
    pub prompt: Option<(String, String, usize, String, Range<usize>)>,
}

/// A handle to draw: where, its colour role, and its normal's end on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct HandleView {
    pub at: (f32, f32),
    pub tip: (f32, f32),
    pub chosen: bool,
    pub handle: Handle,
}

/// What the overlay draws over the 3D view.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct Overlay {
    pub handles: Vec<HandleView>,
    /// A measurement: its ends on screen and its label.
    pub measure: Option<Measured>,
    pub first: Option<(f32, f32)>,
}

/// `s` cut to `n` characters, the last `…`. Right to left (where rows are joined with
/// isolates), bidi controls are not counted and isolates stay balanced.
fn cut(s: &str, n: usize) -> String {
    if I18n::global().direction().is_rtl() {
        return specular_i18n::truncate(s, n);
    }
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let mut o: String = s.chars().take(n.saturating_sub(1)).collect();
        o.push('…');
        o
    }
}

fn add3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn centre(b: &Bounds) -> [f64; 3] {
    b.centre().map(f64::from)
}

fn unit3(a: [f64; 3]) -> [f64; 3] {
    let l = dot3(a, a).sqrt();
    if l > 0.0 { a.map(|c| c / l) } else { a }
}

/// Eased blend of two poses.
fn blend(a: &Pose, b: &Pose, t: f64) -> Pose {
    let k = t * t * (3.0 - 2.0 * t);
    let mix = |x: [f64; 3], y: [f64; 3]| std::array::from_fn(|i| x[i] + (y[i] - x[i]) * k);
    Pose {
        eye: mix(a.eye, b.eye),
        direction: unit3(mix(a.direction, b.direction)),
        up: unit3(mix(a.up, b.up)),
        projection: if t < 0.5 { a.projection } else { b.projection },
        fov: a.fov + (b.fov - a.fov) * k,
        height: a.height + (b.height - a.height) * k,
        distance: a.distance + (b.distance - a.distance) * k,
    }
}

/// The user's name for authoring BCF topics and comments.
pub(super) fn author() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Specular".to_owned())
}

/// The statuses S steps through: the archive's own, else BCF's usual ones.
fn statuses(a: &Archive) -> Vec<String> {
    if a.extensions.topic_statuses.is_empty() {
        ["Open", "In Progress", "Resolved", "Closed"]
            .map(str::to_owned)
            .to_vec()
    } else {
        a.extensions.topic_statuses.clone()
    }
}

impl MeshView {
    /// The model's frame in BCF's (metres, the project's origin).
    fn bcf_frame(&self) -> Frame {
        let Some(m) = self.model.as_ref() else {
            return Frame::default();
        };
        if let Some(ifc) = &m.ifc {
            return Frame {
                metres: ifc.mesh_metres,
                origin: ifc.origin_metres,
            };
        }
        Frame {
            metres: m.mesh.unit.metres().unwrap_or(1.0),
            origin: [0.0; 3],
        }
    }

    fn metres(&self) -> Option<f64> {
        let m = self.model.as_ref()?;
        m.ifc
            .as_ref()
            .map(|i| i.mesh_metres)
            .or_else(|| m.mesh.unit.metres())
    }

    /// The pose of the current camera.
    pub(super) fn pose(&self) -> Pose {
        let (w, h) = self.size();
        self.camera.pose(&self.bounds, w, h)
    }

    fn set_pose(&mut self, p: &Pose) {
        let (w, h) = self.size();
        self.camera = Camera::from_pose(p, &self.bounds, w, h);
        if self.camera.projection != self.projection && self.camera.eye.is_none() {
            self.projection = self.camera.projection;
        }
        self.changed();
    }

    /// Animates the camera to `to` (back to the orbit camera first).
    pub(super) fn animate_to(&mut self, to: Pose) {
        if self.bim.nav.take().is_some() {
            self.camera.fov = specular_render_3d::FOV_Y;
        }
        let from = self.pose();
        self.bim.anim = Some(Anim {
            from,
            to,
            t0: Instant::now(),
            sections: None,
        });
        self.changed();
    }

    /// Ends an animation at once.
    #[cfg(test)]
    pub(super) fn finish_anim(&mut self) {
        if let Some(a) = self.bim.anim.take() {
            self.set_pose(&a.to);
        }
    }

    /// Starts building what walking collides with (once per model).
    fn ensure_scene(&mut self) {
        if self.bim.scene.is_some() || self.bim.building.is_some() {
            return;
        }
        // A tileset: the tiles near the walker, refreshed as it moves.
        if self.tiles.is_some() {
            self.tiles_collision();
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        if let Some(m) = self.model.as_ref() {
            let mesh = m.mesh.clone();
            specular_thread::spawn("ren-nav-bvh", move || {
                let t0 = Instant::now();
                let s = NavScene::from_mesh(&mesh);
                specular_log::info!(
                    target: specular_log::RENDER,
                    "walk: {} triangles under a BVH in {:.0} ms",
                    s.len(),
                    t0.elapsed().as_secs_f64() * 1e3
                );
                let _ = tx.send(s);
            });
        } else if let Some(c) = self.cloud.as_ref() {
            let loading = c.loading.clone();
            let b = self.bounds;
            specular_thread::spawn("ren-nav-bvh", move || {
                let Some(Ok(cloud)) = loading.result() else {
                    return;
                };
                let pts = &cloud.positions;
                let _ = tx.send(NavScene::from_points(pts, point_radius(&b, pts.len())));
            });
        } else {
            return;
        }
        self.bim.building = Some(rx);
    }
}

/// The radius walking gives each of `n` points in `b`: a point stands for the area around
/// it, the mean spacing on the box's floor.
pub(super) fn point_radius(b: &Bounds, n: usize) -> f32 {
    let s = b.size();
    let area = f64::from(s[0].max(1e-6) * s[1].max(1e-6));
    let r = (area / n.max(1) as f64).sqrt() as f32 * 0.75;
    r.max(1e-4)
}

impl MeshView {
    /// The drawn geometry changed (3D Tiles swapped tiles): walking collides with the new
    /// geometry once it is built, the old until then; section caps are cut again.
    pub(super) fn geometry_changed(&mut self) {
        self.bim.building = None;
        self.bim.caps = None;
        self.bim.caps_gpu = None;
        let old = self.bim.scene.take();
        if self.bim.nav.is_some() {
            self.ensure_scene();
            self.bim.scene = old;
        }
    }

    /// Triangles or points walking collides with now (tests).
    #[cfg(test)]
    pub(super) fn collision_len(&self) -> Option<usize> {
        self.bim.scene.as_ref().map(|s| s.len())
    }

    /// Waits for the collision scene being built and takes it (tests).
    #[cfg(test)]
    pub(super) fn collision_wait(&mut self) {
        if let Some(rx) = self.bim.building.take()
            && let Ok(s) = rx.recv_timeout(std::time::Duration::from_secs(20))
        {
            self.bim.scene = Some(Arc::new(s));
        }
    }

    /// The caps of the current sections (computed when they change).
    pub(super) fn caps_mesh(&mut self) -> Option<&specular_decode::specular_mesh::Mesh> {
        if self.bim.sections.is_empty() {
            self.bim.caps = None;
            return None;
        }
        if self.bim.caps.is_none() {
            let model = self.bim.restyled.as_ref().or(self.model.as_ref())?;
            let scene = self
                .bim
                .scene
                .as_deref()
                .filter(|_| self.bim.restyled.is_none());
            let t0 = Instant::now();
            let c = section::caps(&model.mesh, scene, &self.bim.sections);
            specular_log::debug!(
                target: specular_log::RENDER,
                "sections: {} outlines, caps in {:.1} ms",
                c.loops.len(),
                t0.elapsed().as_secs_f64() * 1e3
            );
            self.bim.caps = Some(c);
        }
        self.bim.caps.as_ref()?.mesh.as_ref()
    }
}

impl MeshView {
    /// The length measured, in the model's unit.
    fn measure_label(&self) -> Option<String> {
        let (a, b) = self.bim.measure.as_ref()?.done?;
        let d = dot3(sub3(b, a), sub3(b, a)).sqrt();
        let unit = self
            .model
            .as_ref()
            .and_then(|m| m.mesh.unit.symbol())
            .unwrap_or("");
        let loc = specular_i18n::I18n::global().localizer();
        let digits = if d >= 100.0 {
            0
        } else if d >= 1.0 {
            2
        } else {
            4
        };
        // Millimetres as the UI's language writes them (as the medical and MPR overlays).
        Some(crate::app::measure_tool::length_with_unit(
            loc.number(d, digits),
            unit,
        ))
    }

    /// The panel to draw.
    pub(super) fn panel(&self) -> Option<PanelView> {
        let b = &self.bim;
        let kind = b.panel?;
        let prompt = b.prompt.as_ref().map(|p| {
            let label = match p.asking {
                Asking::NewView => t!("bim-note", what = "ask-view-name"),
                Asking::Rename(_) => t!("bim-note", what = "ask-rename"),
                Asking::NewTopic => t!("bcf-note", what = "ask-title"),
                Asking::Comment => t!("bcf-note", what = "ask-comment"),
                Asking::Title => t!("bcf-note", what = "ask-title"),
            };
            (
                label,
                p.field.text().to_owned(),
                p.field.caret(),
                p.preedit.clone(),
                p.field.selection(),
            )
        });
        Some(match kind {
            PanelKind::Views => PanelView {
                title: t!("bim-views-title", count = b.views.views.len() as i64),
                rows: b
                    .views
                    .views
                    .iter()
                    .enumerate()
                    .map(|(i, v)| PanelRow {
                        index: i,
                        text: v.name.clone(),
                        detail: None,
                    })
                    .collect(),
                empty: b
                    .views
                    .views
                    .is_empty()
                    .then(|| t!("bim-note", what = "views-empty")),
                chosen: (!b.views.views.is_empty()).then_some(b.row),
                notes: Vec::new(),
                hints: t!("bim-note", what = "views-hints"),
                prompt,
            },
            PanelKind::Topics => {
                let (title, rows, notes) = match &b.bcf {
                    Some(x) => {
                        let file = x
                            .path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| t!("bcf-note", what = "new-file"));
                        let mark = if x.dirty { " •" } else { "" };
                        let title = format!(
                            "{}{mark}",
                            t!(
                                "bcf-title",
                                count = x.archive.topics.len() as i64,
                                file = file
                            )
                        );
                        let rows = x
                            .archive
                            .topics
                            .iter()
                            .enumerate()
                            .map(|(i, tp)| PanelRow {
                                index: i,
                                text: tp.title.clone(),
                                detail: Some(topic_detail(tp)),
                            })
                            .collect();
                        let notes = x
                            .archive
                            .topics
                            .get(b.row)
                            .map(|tp| {
                                let mut n: Vec<String> = Vec::new();
                                if !tp.viewpoints.is_empty() {
                                    n.push(t!(
                                        "bcf-viewpoint-of",
                                        at = (x.viewpoint.min(tp.viewpoints.len() - 1) + 1) as i64,
                                        count = tp.viewpoints.len() as i64
                                    ));
                                }
                                // Each line reads in its comment's own direction (a Latin
                                // comment in the Hebrew UI starts with its author on the left).
                                n.extend(tp.comments.iter().rev().take(4).map(|c| {
                                    let text = c.text.replace('\n', " ");
                                    cut(
                                        &I18n::global().join_labelled(&c.author, &text, ": "),
                                        ROW_CHARS + 6,
                                    )
                                }));
                                n
                            })
                            .unwrap_or_default();
                        (title, rows, notes)
                    }
                    None => (t!("bcf-note", what = "title-none"), Vec::new(), Vec::new()),
                };
                let empty = rows.is_empty().then(|| t!("bcf-note", what = "empty"));
                PanelView {
                    title,
                    chosen: (!rows.is_empty()).then_some(b.row),
                    rows,
                    empty,
                    notes,
                    hints: t!("bcf-note", what = "hints"),
                    prompt,
                }
            }
            PanelKind::Checks => self.checks_panel(),
            PanelKind::Models => self.fed_panel(),
            PanelKind::Pmi => self.pmi_panel(),
        })
    }

    /// The handles and the measurement to draw over the view (logical px).
    pub(in crate::app) fn overlay(&mut self, scale: f64) -> Option<Overlay> {
        let m = &*self;
        let r = m.rect?;
        let (w, h) = m.size();
        let p = m.camera.project(&m.bounds, w, h, 0.0);
        let to_l = |q: [f64; 3]| -> Option<(f32, f32)> {
            let (x, y, _) = p.to_px(q, w, h)?;
            Some((
                ((x + f64::from(r.x)) / scale) as f32,
                ((y + f64::from(r.y)) / scale) as f32,
            ))
        };
        let c = centre(&m.bounds);
        let reach = f64::from(m.bounds.radius()) * 0.25;
        let mut handles = Vec::new();
        let mut list: Vec<(Handle, [f64; 3], [f64; 3])> = Vec::new();
        for (i, pl) in m.bim.sections.planes.iter().enumerate() {
            // The model's centre on the plane.
            let at = sub3(c, pl.normal.map(|x| x * pl.distance(c)));
            list.push((Handle::Plane(i), at, pl.normal));
        }
        if let Some(b) = &m.bim.sections.section_box {
            let bc = centre(b);
            for f in 0..6 {
                let k = f / 2;
                let mut at = bc;
                let mut n = [0.0; 3];
                if f % 2 == 0 {
                    at[k] = f64::from(b.max[k]);
                    n[k] = 1.0;
                } else {
                    at[k] = f64::from(b.min[k]);
                    n[k] = -1.0;
                }
                list.push((Handle::BoxFace(f), at, n));
            }
        }
        let mut drawn = Vec::new();
        for (hd, at, n) in list {
            let (Some(a), Some(t)) = (to_l(at), to_l(add3(at, n.map(|x| x * reach)))) else {
                continue;
            };
            let dir = (f64::from(t.0 - a.0) / reach, f64::from(t.1 - a.1) / reach);
            drawn.push((hd, a, dir));
            handles.push(HandleView {
                at: a,
                tip: t,
                chosen: m.bim.chosen == Some(hd),
                handle: hd,
            });
        }
        let measure = m.bim.measure.as_ref().and_then(|ms| {
            let (a, b) = ms.done?;
            Some((to_l(a)?, to_l(b)?, m.measure_label()?))
        });
        let first = m
            .bim
            .measure
            .as_ref()
            .and_then(|ms| ms.first)
            .and_then(to_l);
        self.bim.handles = drawn;
        Some(Overlay {
            handles,
            measure,
            first,
        })
    }

    /// The readout's extra lines: walking or flying, the cuts, the measurement.
    pub(in crate::app) fn review_lines(&self) -> Vec<String> {
        let m = self;
        let mut v = Vec::new();
        if let Some(n) = m.bim.nav {
            let loc = specular_i18n::I18n::global().localizer();
            let unit = m
                .model
                .as_ref()
                .and_then(|x| x.mesh.unit.symbol())
                .unwrap_or("");
            match n.mode {
                NavMode::Walk => {
                    let eye = specular_i18n::quantity(&loc.number(n.body.eye, 2), unit);
                    v.push(t!("mesh-nav", what = "walk-line", eye = eye));
                    v.push(t!("mesh-nav", what = "walk-keys"));
                    if m.bim.scene.is_none() {
                        v.push(t!("mesh-nav", what = "preparing"));
                    }
                }
                _ => {
                    v.push(t!("mesh-nav", what = "fly-line"));
                    v.push(t!("mesh-nav", what = "fly-keys"));
                }
            }
        }
        let s = &m.bim.sections;
        if !s.is_empty() {
            let mut l = t!("bim-sections-line", planes = s.planes.len() as i64);
            if let Some(b) = &s.section_box {
                let unit = m
                    .model
                    .as_ref()
                    .and_then(|x| x.mesh.unit.symbol())
                    .unwrap_or("");
                l = format!(
                    "{l} · {}",
                    t!(
                        "bim-box-line",
                        size = size_line(b.size().map(f64::from), unit)
                    )
                );
            }
            v.push(l);
        }
        if m.bim.measure.is_some() {
            v.push(match m.measure_label() {
                Some(d) => t!("bim-measure-line", length = d),
                None => t!("bim-note", what = "measure-hint"),
            });
        }
        v
    }
}

impl App {
    /// The current file's path.
    pub(super) fn bim_path(&self) -> Option<PathBuf> {
        self.current.and_then(|i| self.entries.get(i)).cloned()
    }

    /// Walk or fly (`mode`), or back to orbit when already in it.
    pub(in crate::app) fn bim_nav(&mut self, mode: NavMode) {
        let now = self.now();
        let m = &mut self.mesh;
        if m.bim.nav_mode() == mode || mode == NavMode::Orbit {
            if m.bim.nav.take().is_some() {
                let (w, h) = m.size();
                m.camera.to_orbit(&m.bounds, w, h);
                m.camera.fov = specular_render_3d::FOV_Y;
                m.changed();
                self.chrome.toast(t!("mesh-nav", what = "orbit"), None, now);
            }
            self.redraw = true;
            return;
        }
        if m.bounds.is_empty() {
            return;
        }
        m.ensure_scene();
        let (w, h) = m.size();
        let body = Body::for_model(&m.bounds, m.metres());
        let mut nav = Nav::start(mode, &m.camera, &m.bounds, w, h, body);
        nav.settle(m.bim.scene.as_deref());
        m.bim.nav = Some(nav);
        m.bim.held = Held::default();
        m.bim.last = None;
        m.bim.anim = None;
        m.camera = nav.camera(NAV_FOV);
        m.changed();
        let msg = match mode {
            NavMode::Walk => t!("mesh-nav", what = "walk-toast"),
            _ => t!("mesh-nav", what = "fly-toast"),
        };
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// Each frame: the scene being built, walking, flying, a view being gone to, dialogs.
    pub(in crate::app) fn bim_tick(&mut self) {
        self.bim_poll();
        self.checks_poll();
        self.fed_poll();
        let m = &mut self.mesh;
        if let Some(rx) = &m.bim.building
            && let Ok(s) = rx.try_recv()
        {
            m.bim.scene = Some(Arc::new(s));
            m.bim.building = None;
            m.bim.caps = None;
            if let Some(n) = m.bim.nav.as_mut() {
                n.settle(m.bim.scene.as_deref());
                m.camera = n.camera(NAV_FOV);
                m.changed();
            }
            self.redraw = true;
        }
        let now = Instant::now();
        if let Some(a) = &m.bim.anim {
            let t = ((now - a.t0).as_secs_f64() / ANIM_S).min(1.0);
            let p = blend(&a.from, &a.to, t);
            let done = t >= 1.0;
            let sections = done.then(|| a.sections.clone()).flatten();
            m.set_pose(&p);
            if done {
                m.bim.anim = None;
                if let Some(s) = sections {
                    m.bim.sections = s;
                    m.bim.sections_changed();
                }
            }
            self.redraw = true;
        }
        let Some(mut nav) = m.bim.nav else {
            m.bim.last = None;
            return;
        };
        let dt = m.bim.last.map_or(0.0, |l| (now - l).as_secs_f64());
        let h = m.bim.held;
        let axis = |p: bool, n: bool| f64::from(u8::from(p)) - f64::from(u8::from(n));
        let input = NavInput {
            forward: axis(h.fwd, h.back),
            strafe: axis(h.right, h.left),
            rise: axis(h.up, h.down),
            turn: axis(h.turn_l, h.turn_r) * TURN_RATE,
            tilt: 0.0,
            look: std::mem::take(&mut m.bim.look),
            fast: h.fast,
        };
        let moved = nav.step(m.bim.scene.as_deref(), &input, dt);
        m.bim.nav = Some(nav);
        if moved || input.look != (0.0, 0.0) {
            m.camera = nav.camera(NAV_FOV);
            m.changed();
            self.redraw = true;
        }
        // Keep ticking while something moves (keys held, falling).
        if h.any() || !nav.grounded && nav.mode == NavMode::Walk {
            m.bim.last = Some(now);
            self.redraw = true;
        } else {
            m.bim.last = None;
        }
    }

    /// Dialog results.
    fn bim_poll(&mut self) {
        let now = self.now();
        while let Ok(d) = self.mesh.bim.rx.try_recv() {
            match d {
                Done::Opened(path, Ok(archive)) => {
                    let n = archive.topics.len();
                    self.mesh.bim.bcf = Some(Bcf {
                        path: Some(path),
                        archive: *archive,
                        dirty: false,
                        viewpoint: 0,
                    });
                    self.mesh.bim.row = 0;
                    self.mesh.bim.panel = Some(PanelKind::Topics);
                    self.chrome
                        .toast(t!("bcf-opened", count = n as i64), None, now);
                }
                Done::Opened(_, Err(e)) => {
                    self.chrome
                        .toast(t!("bim-failed", what = "open", reason = e), None, now);
                }
                Done::Exported(Ok(p)) => {
                    if let Some(b) = self.mesh.bim.bcf.as_mut() {
                        b.dirty = false;
                    }
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    self.chrome
                        .toast(t!("bim-saved-file", file = name), None, now);
                }
                Done::Exported(Err(e)) | Done::Png(Err(e)) => {
                    self.chrome
                        .toast(t!("bim-failed", what = "save", reason = e), None, now);
                }
                Done::Png(Ok(p)) => {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    self.chrome
                        .toast(t!("bim-saved-file", file = name), None, now);
                }
            }
            self.redraw = true;
        }
    }

    /// Loads the model's saved views once (from the sidecar next to it).
    fn bim_views_load(&mut self) {
        if self.mesh.bim.views_loaded {
            return;
        }
        self.mesh.bim.views_loaded = true;
        let Some(path) = self.mesh.id.and_then(|_| self.bim_path()) else {
            return;
        };
        let side = views::sidecar_path(&path);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.mesh.bim.views = ViewsFile::new(&name);
        match views::load(&side) {
            Ok(Some(f)) => self.mesh.bim.views = f,
            Ok(None) => {}
            Err(e) => {
                specular_log::warn!(target: specular_log::UI, "views: {}: {e}", side.display());
            }
        }
        self.mesh.bim.sidecar = Some(side);
    }

    /// Writes the views (where it cannot, they last for the session).
    fn bim_views_save(&mut self) {
        let Some(side) = self.mesh.bim.sidecar.clone() else {
            return;
        };
        if let Err(e) = views::save(&side, &self.mesh.bim.views) {
            let now = self.now();
            self.chrome.toast(
                t!(
                    "bim-failed",
                    what = "views",
                    reason = crate::error_text::io_reason(&e)
                ),
                None,
                now,
            );
        }
    }

    /// The current view as a saved one named `name`.
    fn bim_view_now(&self, name: &str) -> SavedView {
        let m = &self.mesh;
        let p = m.pose();
        SavedView {
            name: name.to_owned(),
            eye: p.eye,
            direction: p.direction,
            up: p.up,
            projection: p.projection.id().to_owned(),
            fov_degrees: p.fov.to_degrees(),
            height: p.height,
            distance: p.distance,
            planes: m
                .bim
                .sections
                .planes
                .iter()
                .map(|s| [s.normal[0], s.normal[1], s.normal[2], s.offset])
                .collect(),
            section_box: m
                .bim
                .sections
                .section_box
                .map(|b| [b.min.map(f64::from), b.max.map(f64::from)]),
            hidden: m.bim.hidden.clone(),
            shading: Some(m.shading.id().to_owned()),
        }
    }

    /// Goes to a saved view (animated).
    fn bim_go_view(&mut self, i: usize) {
        let Some(v) = self.mesh.bim.views.views.get(i).cloned() else {
            return;
        };
        let to = Pose {
            eye: v.eye,
            direction: v.direction,
            up: v.up,
            projection: if v.projection == "orthographic" {
                Projection::Orthographic
            } else {
                Projection::Perspective
            },
            fov: v.fov_degrees.to_radians(),
            height: v.height,
            distance: v.distance,
        };
        let sections = Sections {
            planes: v
                .planes
                .iter()
                .map(|p| SectionPlane {
                    normal: [p[0], p[1], p[2]],
                    offset: p[3],
                })
                .take(MAX_PLANES)
                .collect(),
            section_box: v.section_box.map(|b| Bounds {
                min: b[0].map(|x| x as f32),
                max: b[1].map(|x| x as f32),
            }),
        };
        if let Some(s) = v.shading.as_deref()
            && let Some(sh) = Shading::ALL.into_iter().find(|x| x.id() == s)
        {
            self.mesh.shading = sh;
        }
        self.bim_set_hidden(v.hidden.clone(), None);
        self.bim_animate_to(to, Some(sections));
        let now = self.now();
        self.chrome.announce_status(v.name.clone());
        let _ = now;
    }

    pub(super) fn bim_animate_to(&mut self, to: Pose, sections: Option<Sections>) {
        let m = &mut self.mesh;
        if m.bim.nav.take().is_some() {
            m.camera.fov = specular_render_3d::FOV_Y;
        }
        let from = m.pose();
        m.bim.anim = Some(Anim {
            from,
            to,
            t0: Instant::now(),
            sections,
        });
        m.changed();
        self.redraw = true;
    }

    /// Hides `hidden` (IFC GUIDs) and applies `components` (a BCF viewpoint's) on an IFC
    /// model; elsewhere nothing changes.
    fn bim_set_hidden(&mut self, hidden: Vec<String>, components: Option<&Components>) {
        let m = &mut self.mesh;
        m.bim.hidden = hidden;
        let Some(model) = m.model.as_ref() else {
            return;
        };
        let Some(ifc) = model.ifc.clone() else {
            return;
        };
        let comps = match components {
            Some(c) => c.clone(),
            None => Components {
                default_visibility: true,
                exceptions: m.bim.hidden.iter().map(|g| bcf::Component::of(g)).collect(),
                ..Components::default()
            },
        };
        let plain = comps.default_visibility
            && comps.exceptions.is_empty()
            && comps.selection.is_empty()
            && comps.coloring.is_empty();
        if plain {
            if m.bim.restyled.take().is_some() {
                m.gpu = None;
                m.bim.sections_changed();
            }
            return;
        }
        let (looks, stats) = apply::looks(&ifc.elements, &comps);
        let mesh = apply::restyle(&model.mesh, &ifc.elements, &looks);
        // Hidden elements, for saving the view again.
        m.bim.hidden = ifc
            .elements
            .iter()
            .zip(&looks)
            .filter(|(_, l)| l.hidden)
            .map(|(e, _)| e.global_id.clone())
            .collect();
        specular_log::debug!(
            target: specular_log::UI,
            "bcf: {} hidden, {} coloured, {} selected, {} not in the model",
            stats.hidden,
            stats.coloured,
            stats.selected,
            stats.missing
        );
        m.bim.restyled = Some(MeshModel {
            mesh: Arc::new(mesh),
            textures: model.textures.clone(),
            ifc: Some(ifc),
        });
        m.gpu = None;
        m.bim.sections_changed();
        m.changed();
    }

    /// The topic's viewpoint `vi` shown: camera, clipping planes, components.
    fn bim_go_viewpoint(&mut self, topic: usize, vi: usize) {
        let frame = self.mesh.bcf_frame();
        let Some(vp) = self
            .mesh
            .bim
            .bcf
            .as_ref()
            .and_then(|b| b.archive.topics.get(topic))
            .and_then(|t| t.viewpoints.get(vi))
            .cloned()
        else {
            return;
        };
        let Some(vis) = vp.vis else { return };
        let sections = Sections {
            planes: vis
                .clipping_planes
                .iter()
                .take(MAX_PLANES)
                .map(|p| {
                    let (n, d) = apply::plane_from_bcf(p, &frame);
                    SectionPlane {
                        normal: n,
                        offset: d,
                    }
                })
                .collect(),
            section_box: None,
        };
        self.bim_set_hidden(Vec::new(), vis.components.as_ref());
        match vis.camera {
            Some(c) => {
                let mc = apply::camera_from_bcf(&c, &frame);
                let distance = dot3(sub3(centre(&self.mesh.bounds), mc.eye), mc.direction).abs();
                let to = Pose {
                    eye: mc.eye,
                    direction: mc.direction,
                    up: mc.up,
                    projection: if mc.perspective {
                        Projection::Perspective
                    } else {
                        Projection::Orthographic
                    },
                    fov: mc.fov,
                    height: mc.height,
                    distance,
                };
                self.bim_animate_to(to, Some(sections));
            }
            None => {
                self.mesh.bim.sections = sections;
                self.mesh.bim.sections_changed();
                self.mesh.changed();
            }
        }
        if let Some(b) = self.mesh.bim.bcf.as_mut() {
            b.viewpoint = vi;
        }
        self.redraw = true;
    }

    /// The current view as a BCF viewpoint (a snapshot is taken on the next frame).
    fn bim_viewpoint_now(&self) -> Viewpoint {
        let m = &self.mesh;
        let frame = m.bcf_frame();
        let p = m.pose();
        let (w, h) = m.size();
        let cam = MeshCamera {
            eye: p.eye,
            direction: p.direction,
            up: p.up,
            perspective: p.projection == Projection::Perspective,
            fov: p.fov,
            height: p.height,
        };
        let guid = specular_bim::ids::new_guid();
        let components = m
            .model
            .as_ref()
            .and_then(|x| x.ifc.as_ref())
            .map(|_| Components {
                default_visibility: true,
                exceptions: m.bim.hidden.iter().map(|g| bcf::Component::of(g)).collect(),
                ..Components::default()
            });
        Viewpoint {
            guid: guid.clone(),
            file: None,
            snapshot_name: None,
            snapshot: None,
            index: None,
            vis: Some(VisInfo {
                guid,
                components,
                camera: Some(apply::camera_to_bcf(&cam, &frame, w / h.max(1.0))),
                lines: Vec::new(),
                clipping_planes: m
                    .bim
                    .sections
                    .all_planes()
                    .iter()
                    .map(|s| apply::plane_to_bcf(s.normal, s.offset, &frame))
                    .collect(),
                bitmaps: Vec::new(),
            }),
        }
    }

    /// A press or release at physical `p` in the 3D view; true when taken (handles, the
    /// panel's rows, measuring, looking around).
    pub(in crate::app) fn bim_button(&mut self, p: (f32, f32), down: bool) -> bool {
        let scale = self.dl.scale.max(0.1);
        let lp = (p.0 / scale, p.1 / scale);
        if !down
            && let Some(p) = self.mesh.bim.prompt.as_mut()
            && p.field.dragging()
        {
            p.field.release();
            return true;
        }
        if down && self.bim_prompt_press(lp) {
            return true;
        }
        if !down {
            let was = self.mesh.bim.drag.take().is_some();
            if was {
                self.mesh.bim.sections_changed();
                self.redraw = true;
            }
            return was;
        }
        if let Some(r) = self.mesh.bim.panel_rect
            && r.contains(lp)
        {
            if let Some((i, _)) = self.mesh.bim.rows.iter().find(|(_, r)| r.contains(lp)) {
                self.mesh.bim.row = *i;
                self.bim_enter();
            }
            self.redraw = true;
            return true;
        }
        let hit = self.mesh.bim.handles.iter().find(|(_, c, _)| {
            let d = (c.0 - lp.0).hypot(c.1 - lp.1);
            d <= HANDLE_HIT
        });
        if let Some((h, _, _)) = hit {
            self.mesh.bim.drag = Some((*h, lp));
            self.mesh.bim.chosen = Some(*h);
            self.redraw = true;
            return true;
        }
        if self.mesh.bim.measure.is_some() {
            self.bim_measure_click(p);
            return true;
        }
        false
    }

    /// Pointer motion at physical `p`: drags a handle, or looks around while walking or
    /// flying. True when taken.
    pub(in crate::app) fn bim_motion(&mut self, p: (f32, f32)) -> bool {
        let scale = self.dl.scale.max(0.1);
        let lp = (p.0 / scale, p.1 / scale);
        if self.mesh.bim.prompt_drag(lp.0) {
            self.redraw = true;
            return true;
        }
        let Some((h, last)) = self.mesh.bim.drag else {
            return false;
        };
        let Some(&(_, _, dir)) = self.mesh.bim.handles.iter().find(|(x, _, _)| *x == h) else {
            return false;
        };
        // Model units along the normal per logical px moved along its screen direction.
        let l2 = dir.0 * dir.0 + dir.1 * dir.1;
        if l2 <= 1e-12 {
            return true;
        }
        let d = (f64::from(lp.0 - last.0) * dir.0 + f64::from(lp.1 - last.1) * dir.1) / l2;
        let m = &mut self.mesh;
        let s = &mut m.bim.sections;
        match h {
            Handle::Plane(i) => {
                if let Some(pl) = s.planes.get_mut(i) {
                    pl.offset += d;
                }
            }
            Handle::BoxFace(f) => {
                if let Some(b) = s.section_box.as_mut() {
                    let k = f / 2;
                    if f % 2 == 0 {
                        b.max[k] = (b.max[k] + d as f32).max(b.min[k] + 1e-6);
                    } else {
                        b.min[k] = (b.min[k] - d as f32).min(b.max[k] - 1e-6);
                    }
                }
            }
        }
        m.bim.drag = Some((h, lp));
        m.bim.sections_changed();
        m.changed();
        self.redraw = true;
        true
    }

    /// A drag of `d` logical px while walking or flying turns the view.
    pub(in crate::app) fn bim_look(&mut self, d: (f64, f64)) -> bool {
        if self.mesh.bim.nav.is_none() {
            return false;
        }
        self.mesh.bim.look.0 -= d.0 * LOOK_PER_PX;
        self.mesh.bim.look.1 -= d.1 * LOOK_PER_PX;
        self.redraw = true;
        true
    }

    /// A click while measuring on the cut: the first point, then the second (snapped to
    /// the cut's outline when near it).
    fn bim_measure_click(&mut self, p: (f32, f32)) {
        let now = self.now();
        let dl_scale = f64::from(self.dl.scale.max(0.1));
        let m = &mut self.mesh;
        let Some(r) = m.rect else { return };
        let (w, h) = (f64::from(r.w.max(1.0)), f64::from(r.h.max(1.0)));
        let (o, d) = m
            .camera
            .ray(&m.bounds, f64::from(p.0 - r.x), f64::from(p.1 - r.y), w, h);
        // The nearest kept point on a cutting plane along the ray.
        let all = m.bim.sections.all_planes();
        let eps = f64::from(m.bounds.radius()) * 1e-6;
        let mut best: Option<(f64, [f64; 3])> = None;
        for pl in &all {
            let den = dot3(pl.normal, d);
            if den.abs() < 1e-12 {
                continue;
            }
            let t = (pl.offset - dot3(pl.normal, o)) / den;
            if t <= 0.0 {
                continue;
            }
            let q = add3(o, d.map(|c| c * t));
            if m.bim.sections.keeps(q, eps * 10.0) && best.is_none_or(|b| t < b.0) {
                best = Some((t, q));
            }
        }
        let Some((_, mut q)) = best else {
            self.chrome
                .toast(t!("bim-note", what = "measure-off-cut"), None, now);
            return;
        };
        let upp = m.units_per_px() * dl_scale;
        if let Some(c) = m.bim.caps.as_ref()
            && let Some(s) = section::snap_to_cut(c, q, 10.0 * upp)
        {
            q = s;
        }
        let ms = m.bim.measure.get_or_insert_with(Measure::default);
        match ms.first.take() {
            None => {
                ms.first = Some(q);
                ms.done = None;
            }
            Some(a) => ms.done = Some((a, q)),
        }
        m.changed();
        self.redraw = true;
    }

    /// Runs a walk, section, view or BCF action; true when taken.
    pub(in crate::app) fn bim_action(&mut self, a: Action) -> bool {
        let now = self.now();
        match a {
            Action::MeshWalk => self.bim_nav(NavMode::Walk),
            Action::MeshFly => self.bim_nav(NavMode::Fly),
            Action::MeshSectionPlane => {
                let m = &mut self.mesh;
                if m.bim.sections.planes.len() >= MAX_PLANES {
                    self.chrome
                        .toast(t!("bim-planes-full", count = MAX_PLANES as i64), None, now);
                    return true;
                }
                // Facing the view: along the axis nearest the view direction, cutting away
                // the half towards the viewer.
                let f = m.camera.forward();
                let k = (0..3)
                    .max_by(|&x, &y| f[x].abs().total_cmp(&f[y].abs()))
                    .unwrap_or(2);
                let mut n = [0.0; 3];
                n[k] = -f[k].signum();
                let pl = SectionPlane::through(centre(&m.bounds), n);
                m.bim.sections.planes.push(pl);
                m.bim.chosen = Some(Handle::Plane(m.bim.sections.planes.len() - 1));
                m.bim.sections_changed();
                m.changed();
                self.chrome
                    .toast(t!("bim-note", what = "plane-added"), None, now);
            }
            Action::MeshSectionBox => {
                let m = &mut self.mesh;
                if m.bim.sections.section_box.take().is_none() {
                    let b = m.bounds;
                    let s = b.size();
                    let inset: [f32; 3] = std::array::from_fn(|k| s[k] * 0.15);
                    m.bim.sections.section_box = Some(Bounds {
                        min: std::array::from_fn(|k| b.min[k] + inset[k]),
                        max: std::array::from_fn(|k| b.max[k] - inset[k]),
                    });
                    m.bim.chosen = Some(Handle::BoxFace(4));
                    self.chrome
                        .toast(t!("bim-note", what = "box-on"), None, now);
                } else {
                    self.chrome
                        .toast(t!("bim-note", what = "box-off"), None, now);
                }
                m.bim.sections_changed();
                m.changed();
            }
            Action::MeshSectionFlip => {
                let m = &mut self.mesh;
                let i = match m.bim.chosen {
                    Some(Handle::Plane(i)) => Some(i),
                    _ => m.bim.sections.planes.len().checked_sub(1),
                };
                if let Some(pl) = i.and_then(|i| m.bim.sections.planes.get_mut(i)) {
                    *pl = pl.flipped();
                    m.bim.sections_changed();
                    m.changed();
                }
            }
            Action::MeshSectionClear => {
                let m = &mut self.mesh;
                if !m.bim.sections.is_empty() {
                    m.bim.sections = Sections::default();
                    m.bim.chosen = None;
                    m.bim.measure = None;
                    m.bim.sections_changed();
                    m.changed();
                    self.chrome
                        .toast(t!("bim-note", what = "cleared"), None, now);
                }
            }
            Action::MeshCutMeasure => {
                let m = &mut self.mesh;
                if m.bim.measure.take().is_none() {
                    if m.bim.sections.is_empty() {
                        self.chrome
                            .toast(t!("bim-note", what = "measure-needs-cut"), None, now);
                        return true;
                    }
                    m.bim.measure = Some(Measure::default());
                    self.chrome
                        .toast(t!("bim-note", what = "measure-on"), None, now);
                }
                m.changed();
            }
            Action::MeshViewpoints => {
                self.bim_views_load();
                self.bim_toggle_panel(PanelKind::Views);
            }
            Action::MeshSaveViewpoint => {
                self.bim_views_load();
                self.mesh.bim.panel = Some(PanelKind::Views);
                self.bim_ask(Asking::NewView);
            }
            Action::MeshBcf => self.bim_toggle_panel(PanelKind::Topics),
            Action::MeshChecks => {
                self.bim_toggle_panel(PanelKind::Checks);
                if self.mesh.bim.panel == Some(PanelKind::Checks) {
                    self.checks_opened();
                }
            }
            Action::MeshModels => self.bim_toggle_panel(PanelKind::Models),
            Action::MeshAddModel => self.fed_add_dialog(false),
            Action::MeshAddModelFolder => self.fed_add_dialog(true),
            _ => return false,
        }
        self.redraw = true;
        true
    }

    pub(super) fn bim_toggle_panel(&mut self, k: PanelKind) {
        if self.mesh.bim.panel == Some(PanelKind::Checks) {
            self.checks_closed();
        }
        let b = &mut self.mesh.bim;
        b.panel = if b.panel == Some(k) { None } else { Some(k) };
        b.row = 0;
        b.prompt = None;
        self.redraw = true;
    }

    fn bim_ask(&mut self, asking: Asking) {
        let b = &self.mesh.bim;
        let text = match asking {
            Asking::NewView => b.views.free_name(&t!("bim-note", what = "view-name")),
            Asking::Rename(i) => b
                .views
                .views
                .get(i)
                .map(|v| v.name.clone())
                .unwrap_or_default(),
            Asking::Title => b
                .bcf
                .as_ref()
                .and_then(|x| x.archive.topics.get(b.row))
                .map(|t| t.title.clone())
                .unwrap_or_default(),
            Asking::NewTopic | Asking::Comment => String::new(),
        };
        let mut field = LineEdit::new(&text);
        field.set_caret(text.len(), true);
        self.mesh.bim.prompt = Some(Prompt {
            asking,
            field,
            preedit: String::new(),
        });
        self.redraw = true;
    }

    /// The prompt's answer.
    fn bim_answer(&mut self) {
        let Some(p) = self.mesh.bim.prompt.take() else {
            return;
        };
        let text = p.field.text().trim().to_owned();
        let now = self.now();
        match p.asking {
            Asking::NewView => {
                let name = if text.is_empty() {
                    self.mesh
                        .bim
                        .views
                        .free_name(&t!("bim-note", what = "view-name"))
                } else {
                    text
                };
                let v = self.bim_view_now(&name);
                self.mesh.bim.views.views.push(v);
                self.mesh.bim.row = self.mesh.bim.views.views.len() - 1;
                self.bim_views_save();
                self.chrome
                    .toast(t!("bim-view-saved", name = name), None, now);
            }
            Asking::Rename(i) => {
                if !text.is_empty()
                    && let Some(v) = self.mesh.bim.views.views.get_mut(i)
                {
                    v.name = text;
                    self.bim_views_save();
                }
            }
            Asking::NewTopic => {
                let title = if text.is_empty() {
                    t!("bcf-note", what = "untitled")
                } else {
                    text
                };
                self.bim_new_topic(title);
            }
            Asking::Comment => {
                if !text.is_empty() {
                    self.bim_new_comment(text);
                }
            }
            Asking::Title => {
                let row = self.mesh.bim.row;
                if !text.is_empty()
                    && let Some(b) = self.mesh.bim.bcf.as_mut()
                    && let Some(t) = b.archive.topics.get_mut(row)
                {
                    t.title = text;
                    t.modified_date = Some(specular_bim::ids::now());
                    t.modified_author = Some(author());
                    b.dirty = true;
                }
            }
        }
        self.redraw = true;
    }

    /// A new topic about the current view (its snapshot taken on the next frame).
    fn bim_new_topic(&mut self, title: String) {
        let vp = self.bim_viewpoint_now();
        let now_s = specular_bim::ids::now();
        let file = self
            .bim_path()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
        let b = self.mesh.bim.bcf.get_or_insert_with(|| Bcf {
            path: None,
            archive: Archive {
                version: Version::V3_0,
                version_id: "3.0".into(),
                ..Archive::default()
            },
            dirty: true,
            viewpoint: 0,
        });
        let index = b.archive.topics.len() as i32 + 1;
        let t = Topic {
            guid: specular_bim::ids::new_guid(),
            topic_type: Some("Issue".into()),
            status: Some(statuses(&b.archive)[0].clone()),
            title,
            index: Some(index),
            creation_date: now_s.clone(),
            creation_author: author(),
            files: file
                .map(|f| {
                    vec![bcf::HeaderFile {
                        is_external: true,
                        filename: Some(f.clone()),
                        date: Some(now_s),
                        reference: Some(f),
                        ..bcf::HeaderFile::default()
                    }]
                })
                .unwrap_or_default(),
            viewpoints: vec![vp],
            ..Topic::default()
        };
        b.archive.topics.push(t);
        b.dirty = true;
        b.viewpoint = 0;
        let ti = b.archive.topics.len() - 1;
        self.mesh.bim.row = ti;
        self.mesh.bim.capture = Some(Capture::Snapshot {
            topic: ti,
            viewpoint: 0,
        });
        self.mesh.changed();
        let now = self.now();
        self.chrome
            .toast(t!("bcf-note", what = "topic-added"), None, now);
    }

    fn bim_new_comment(&mut self, text: String) {
        let vp = self.bim_viewpoint_now();
        let row = self.mesh.bim.row;
        let Some(b) = self.mesh.bim.bcf.as_mut() else {
            return;
        };
        let Some(t) = b.archive.topics.get_mut(row) else {
            return;
        };
        let vg = vp.guid.clone();
        t.viewpoints.push(vp);
        let vi = t.viewpoints.len() - 1;
        t.comments.push(Comment {
            guid: specular_bim::ids::new_guid(),
            date: specular_bim::ids::now(),
            author: author(),
            text,
            viewpoint: Some(vg),
            modified_date: None,
            modified_author: None,
        });
        b.dirty = true;
        b.viewpoint = vi;
        self.mesh.bim.capture = Some(Capture::Snapshot {
            topic: row,
            viewpoint: vi,
        });
        self.mesh.changed();
        let now = self.now();
        self.chrome
            .toast(t!("bcf-note", what = "comment-added"), None, now);
    }

    /// Enter in the panel: go to the chosen view or topic.
    fn bim_enter(&mut self) {
        let row = self.mesh.bim.row;
        match self.mesh.bim.panel {
            Some(PanelKind::Views) => self.bim_go_view(row),
            Some(PanelKind::Topics)
                if self
                    .mesh
                    .bim
                    .bcf
                    .as_ref()
                    .is_some_and(|b| row < b.archive.topics.len()) =>
            {
                self.bim_go_viewpoint(row, 0);
            }
            Some(PanelKind::Checks) => self.checks_enter(),
            Some(PanelKind::Models) => self.fed_enter(),
            Some(PanelKind::Pmi) => self.pmi_enter(),
            _ => {}
        }
    }

    /// Asks before removing the chosen view.
    fn bim_ask_delete(&mut self) {
        let Some(v) = self.mesh.bim.views.views.get(self.mesh.bim.row) else {
            return;
        };
        let title = t!("bim-view-delete-title", name = v.name.clone());
        self.chrome.confirm_destructive(
            title,
            t!("bim-note", what = "delete-detail"),
            t!("bim-note", what = "delete-ok"),
            VIEW_DELETE_ITEM,
        );
        self.redraw = true;
    }

    /// The confirmation's answer: the chosen view goes.
    pub(in crate::app) fn bim_delete_confirmed(&mut self) {
        let i = self.mesh.bim.row;
        if i < self.mesh.bim.views.views.len() {
            self.mesh.bim.views.views.remove(i);
            self.mesh.bim.row = i.min(self.mesh.bim.views.views.len().saturating_sub(1));
            self.bim_views_save();
        }
        self.redraw = true;
    }

    fn bim_open_bcf(&mut self) {
        let tx = self.mesh.bim.tx.clone();
        let dir = self
            .bim_path()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        self.loader.background("dlg-bcf-open", move |l| {
            let mut dlg = rfd::FileDialog::new().add_filter("BCF", &["bcfzip", "bcf"]);
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let Some(path) = dlg.pick_file() else { return };
            let r = std::fs::read(&path)
                .map_err(|e| crate::error_text::io_reason(&e))
                .and_then(|b| bcf::read(&b).map(Box::new).map_err(|e| e.to_string()));
            let _ = tx.send(Done::Opened(path, r));
            l.waker()();
        });
    }

    fn bim_export_bcf(&mut self) {
        let Some(b) = self.mesh.bim.bcf.as_ref() else {
            return;
        };
        let archive = b.archive.clone();
        let version = archive.version;
        let name = b
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .or_else(|| {
                self.bim_path()
                    .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            })
            .unwrap_or_else(|| "topics".into());
        let dir = self
            .bim_path()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let model = self.bim_path();
        let tx = self.mesh.bim.tx.clone();
        self.loader.background("dlg-bcf-export", move |l| {
            let mut dlg = rfd::FileDialog::new()
                .add_filter("BCF", &["bcfzip"])
                .set_file_name(format!("{name}.bcfzip"));
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let Some(out) = dlg.save_file() else { return };
            // Never over the model.
            let r = if model.as_deref() == Some(out.as_path()) {
                Err(t!("bim-note", what = "not-over-model"))
            } else {
                std::fs::write(&out, specular_bim::write(&archive, version))
                    .map(|()| out)
                    .map_err(|e| crate::error_text::io_reason(&e))
            };
            let _ = tx.send(Done::Exported(r));
            l.waker()();
        });
    }

    /// The captured frame (RGBA8, `size`): cropped to the 3D view, as a snapshot or a PNG.
    pub(in crate::app) fn bim_captured(&mut self, rgba: &[u8], size: (u32, u32)) {
        let Some(cap) = self.mesh.bim.capture.take() else {
            return;
        };
        let Some(r) = self.mesh.rect else { return };
        let (x0, y0) = (r.x.max(0.0) as u32, r.y.max(0.0) as u32);
        let w = (r.w as u32).min(size.0.saturating_sub(x0)).max(1);
        let h = (r.h as u32).min(size.1.saturating_sub(y0)).max(1);
        let mut out = Vec::with_capacity(w as usize * h as usize * 4);
        for y in y0..y0 + h {
            let s = (y as usize * size.0 as usize + x0 as usize) * 4;
            if let Some(line) = rgba.get(s..s + w as usize * 4) {
                out.extend_from_slice(line);
            }
        }
        if out.len() != w as usize * h as usize * 4 {
            return;
        }
        let png = match specular_png::encode(w, h, specular_png::ColorType::Rgba, &out) {
            Ok(p) => p,
            Err(e) => {
                let now = self.now();
                self.chrome.toast(
                    t!("bim-failed", what = "save", reason = e.to_string()),
                    None,
                    now,
                );
                return;
            }
        };
        match cap {
            Capture::Snapshot { topic, viewpoint } => {
                if let Some(vp) = self
                    .mesh
                    .bim
                    .bcf
                    .as_mut()
                    .and_then(|b| b.archive.topics.get_mut(topic))
                    .and_then(|t| t.viewpoints.get_mut(viewpoint))
                {
                    vp.snapshot = Some(png);
                }
            }
            Capture::Png => {
                let tx = self.mesh.bim.tx.clone();
                let name = self
                    .mesh
                    .bim
                    .views
                    .views
                    .get(self.mesh.bim.row)
                    .map_or_else(|| "view".to_owned(), |v| v.name.clone());
                let dir = self
                    .bim_path()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()));
                let model = self.bim_path();
                self.loader.background("dlg-view-png", move |l| {
                    let mut dlg = rfd::FileDialog::new()
                        .add_filter("PNG", &["png"])
                        .set_file_name(format!("{name}.png"));
                    if let Some(d) = dir {
                        dlg = dlg.set_directory(d);
                    }
                    let Some(out) = dlg.save_file() else { return };
                    let r = if model.as_deref() == Some(out.as_path()) {
                        Err(t!("bim-note", what = "not-over-model"))
                    } else {
                        std::fs::write(&out, png)
                            .map(|()| out)
                            .map_err(|e| crate::error_text::io_reason(&e))
                    };
                    let _ = tx.send(Done::Png(r));
                    l.waker()();
                });
            }
        }
    }

    /// A capture is due: the frame is drawn without the panel, handles and readout.
    pub(in crate::app) fn bim_capturing(&self) -> bool {
        self.mesh.bim.capture.is_some() && self.mesh_active()
    }

    /// Keys while walking or flying (held keys move), and the panel's keys while it is
    /// open. True when taken.
    pub(in crate::app) fn bim_key(&mut self, ev: &KeyEvent) -> bool {
        if !self.mesh_active() {
            return false;
        }
        let pressed = ev.state == ElementState::Pressed;
        // The prompt takes every key.
        if self.mesh.bim.prompt.is_some() {
            if pressed {
                let combo = self.key_combo(ev);
                let text = ev.text.as_ref().map(|t| t.to_string());
                self.bim_prompt_key(combo.as_ref(), text.as_deref());
            }
            return true;
        }
        if self.mesh.bim.nav.is_some() {
            let shift_key = matches!(
                ev.physical_key,
                PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight)
            );
            if shift_key {
                self.mesh.bim.held.fast = pressed;
                return false;
            }
            let mode = self.mesh.bim.nav_mode();
            let h = &mut self.mesh.bim.held;
            let slot: Option<&mut bool> = match ev.physical_key {
                PhysicalKey::Code(KeyCode::KeyW | KeyCode::ArrowUp) => Some(&mut h.fwd),
                PhysicalKey::Code(KeyCode::KeyS | KeyCode::ArrowDown) => Some(&mut h.back),
                PhysicalKey::Code(KeyCode::KeyA) => Some(&mut h.left),
                PhysicalKey::Code(KeyCode::KeyD) => Some(&mut h.right),
                PhysicalKey::Code(KeyCode::ArrowLeft) => Some(&mut h.turn_l),
                PhysicalKey::Code(KeyCode::ArrowRight) => Some(&mut h.turn_r),
                PhysicalKey::Code(KeyCode::KeyE | KeyCode::PageUp) if mode == NavMode::Fly => {
                    Some(&mut h.up)
                }
                PhysicalKey::Code(KeyCode::KeyQ | KeyCode::PageDown) if mode == NavMode::Fly => {
                    Some(&mut h.down)
                }
                _ => None,
            };
            let alt = self.mods.alt_key() || self.mods.control_key() || self.mods.super_key();
            if let Some(s) = slot.filter(|_| !alt) {
                *s = pressed;
                h.fast = self.mods.shift_key();
                self.redraw = true;
                return true;
            }
            if pressed && matches!(ev.physical_key, PhysicalKey::Code(KeyCode::Escape)) {
                self.bim_nav(NavMode::Orbit);
                return true;
            }
        }
        if !pressed || self.mesh.bim.panel.is_none() {
            return false;
        }
        let Some(c) = self.key_combo(ev) else {
            return false;
        };
        self.bim_panel_key(&c)
    }

    /// A press at `lp` (logical px) in the prompt's field puts the caret there (Shift
    /// extends, two select the word, three all); false when not in the field.
    fn bim_prompt_press(&mut self, lp: (f32, f32)) -> bool {
        let now = self.now();
        let shift = self.mods.shift_key();
        if !self.mesh.bim.prompt_press(lp, now, shift) {
            return false;
        }
        self.redraw = true;
        true
    }

    /// The prompt is open: input method commits go to it.
    pub(in crate::app) fn bim_wants_text(&self) -> bool {
        self.mesh.bim.prompt.is_some()
    }

    /// An input method's composition (empty: none) for the prompt.
    pub(in crate::app) fn bim_set_preedit(&mut self, s: &str) {
        if let Some(p) = self.mesh.bim.prompt.as_mut() {
            p.preedit = s.to_owned();
            self.redraw = true;
        }
    }

    /// An input method's commit: typed into the prompt.
    pub(in crate::app) fn bim_commit(&mut self, s: &str) {
        if let Some(p) = self.mesh.bim.prompt.as_mut() {
            p.preedit.clear();
        }
        self.bim_prompt_key(None, Some(s));
    }

    fn bim_prompt_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) {
        if let Some(c) = combo {
            match c.key {
                Key::Named(NamedKey::Enter) => {
                    self.bim_answer();
                    return;
                }
                Key::Named(NamedKey::Escape) => {
                    self.mesh.bim.prompt = None;
                    self.redraw = true;
                    return;
                }
                _ => {}
            }
            let rtl = self.rtl();
            let mut clip = PendingClipboard::default();
            let used = self
                .mesh
                .bim
                .prompt
                .as_mut()
                .is_some_and(|p| p.field.key(c.key, c.mods, rtl, &mut clip).used());
            if used {
                self.redraw = true;
                return;
            }
            if c.mods.ctrl || c.mods.alt || c.mods.meta {
                return;
            }
        }
        if let Some(s) = text {
            let s: String = s.chars().filter(|c| !c.is_control()).collect();
            if !s.is_empty()
                && let Some(p) = self.mesh.bim.prompt.as_mut()
            {
                p.field.insert(&s);
                self.redraw = true;
            }
        }
    }

    /// The panel's keys; true when taken.
    fn bim_panel_key(&mut self, c: &KeyCombo) -> bool {
        let kind = self.mesh.bim.panel;
        let n = self.bim_rows_len();
        let plain = !c.mods.ctrl && !c.mods.alt && !c.mods.meta;
        let ch = |k: &Key| match k {
            Key::Char(x) => Some(x.to_ascii_lowercase()),
            _ => None,
        };
        if (kind == Some(PanelKind::Checks) && self.checks_key(c))
            || (kind == Some(PanelKind::Pmi) && self.pmi_key(c))
        {
            self.redraw = true;
            return true;
        }
        if kind == Some(PanelKind::Models) && self.fed_key(c) {
            self.redraw = true;
            return true;
        }
        match (&c.key, kind) {
            (Key::Named(NamedKey::Escape), _) => {
                if kind == Some(PanelKind::Checks) {
                    self.checks_closed();
                }
                self.mesh.bim.panel = None;
            }
            (Key::Named(NamedKey::ArrowDown), _) if plain && n > 0 => {
                self.mesh.bim.row = (self.mesh.bim.row + 1).min(n - 1);
                self.bim_announce_row();
            }
            (Key::Named(NamedKey::ArrowUp), _) if plain && n > 0 => {
                self.mesh.bim.row = self.mesh.bim.row.saturating_sub(1);
                self.bim_announce_row();
            }
            (Key::Named(NamedKey::Enter), _) if plain => self.bim_enter(),
            (Key::Named(NamedKey::F(2)), Some(PanelKind::Views)) if n > 0 => {
                self.bim_ask(Asking::Rename(self.mesh.bim.row));
            }
            (Key::Named(NamedKey::Delete), Some(PanelKind::Views)) if n > 0 => {
                self.bim_ask_delete();
            }
            (k, Some(PanelKind::Views)) if plain && ch(k) == Some('n') => {
                self.bim_ask(Asking::NewView);
            }
            (k, Some(PanelKind::Views)) if plain && ch(k) == Some('p') && n > 0 => {
                // The view at once (no animation), then a frame captured.
                self.bim_go_view(self.mesh.bim.row);
                if let Some(an) = self.mesh.bim.anim.take() {
                    self.mesh.set_pose(&an.to);
                    if let Some(sec) = an.sections {
                        self.mesh.bim.sections = sec;
                        self.mesh.bim.sections_changed();
                    }
                }
                self.mesh.bim.capture = Some(Capture::Png);
                self.mesh.changed();
            }
            (Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight), Some(PanelKind::Topics))
                if plain =>
            {
                let row = self.mesh.bim.row;
                let fwd = matches!(c.key, Key::Named(NamedKey::ArrowRight)) != self.rtl();
                if let Some(b) = self.mesh.bim.bcf.as_ref()
                    && let Some(t) = b.archive.topics.get(row)
                    && !t.viewpoints.is_empty()
                {
                    let k = t.viewpoints.len();
                    let v = if fwd {
                        (b.viewpoint + 1) % k
                    } else {
                        (b.viewpoint + k - 1) % k
                    };
                    self.bim_go_viewpoint(row, v);
                }
            }
            (k, Some(PanelKind::Topics)) if c.mods.ctrl && ch(k) == Some('s') => {
                self.bim_export_bcf();
            }
            (k, Some(PanelKind::Topics)) if plain && ch(k) == Some('o') => self.bim_open_bcf(),
            (k, Some(PanelKind::Topics)) if plain && ch(k) == Some('n') => {
                self.bim_ask(Asking::NewTopic);
            }
            (k, Some(PanelKind::Topics)) if plain && ch(k) == Some('c') && n > 0 => {
                self.bim_ask(Asking::Comment);
            }
            (k, Some(PanelKind::Topics)) if plain && ch(k) == Some('e') && n > 0 => {
                self.bim_ask(Asking::Title);
            }
            (k, Some(PanelKind::Topics)) if plain && ch(k) == Some('s') && n > 0 => {
                let row = self.mesh.bim.row;
                if let Some(b) = self.mesh.bim.bcf.as_mut() {
                    let st = statuses(&b.archive);
                    if let Some(t) = b.archive.topics.get_mut(row) {
                        let i = t
                            .status
                            .as_ref()
                            .and_then(|s| st.iter().position(|x| x == s))
                            .map_or(0, |i| (i + 1) % st.len());
                        t.status = Some(st[i].clone());
                        t.modified_date = Some(specular_bim::ids::now());
                        t.modified_author = Some(author());
                        b.dirty = true;
                        let msg = st[i].clone();
                        self.chrome.announce_status(msg);
                    }
                }
            }
            _ => return false,
        }
        self.redraw = true;
        true
    }

    fn bim_rows_len(&self) -> usize {
        match self.mesh.bim.panel {
            Some(PanelKind::Views) => self.mesh.bim.views.views.len(),
            Some(PanelKind::Topics) => self
                .mesh
                .bim
                .bcf
                .as_ref()
                .map_or(0, |b| b.archive.topics.len()),
            Some(PanelKind::Checks) => self.checks_rows_len(),
            Some(PanelKind::Models) => self.mesh.fed_rows_len(),
            Some(PanelKind::Pmi) => self.mesh.pmi_rows_len(),
            None => 0,
        }
    }

    fn bim_announce_row(&mut self) {
        if let Some(v) = self.bim_panel()
            && let Some(r) = v.rows.iter().find(|r| Some(r.index) == v.chosen)
        {
            let s = match &r.detail {
                Some(d) => format!("{}, {d}", r.text),
                None => r.text.clone(),
            };
            self.chrome.announce_status(s);
        }
    }

    /// The panel to draw.
    pub(in crate::app) fn bim_panel(&self) -> Option<PanelView> {
        if !self.mesh_active() || self.bim_capturing() {
            return None;
        }
        self.mesh.panel()
    }

    /// The handles and the measurement to draw over the view (logical px).
    pub(in crate::app) fn bim_overlay(&mut self) -> Option<Overlay> {
        if !self.mesh_active() || self.bim_capturing() {
            self.mesh.bim.handles.clear();
            return None;
        }
        let scale = f64::from(self.dl.scale.max(0.1));
        self.mesh.overlay(scale)
    }

    /// The readout's extra lines: walking or flying, the cuts, the measurement.
    pub(in crate::app) fn bim_lines(&self) -> Vec<String> {
        self.mesh.review_lines()
    }
}

/// "Open · High · alice · 3 comments".
fn topic_detail(t: &Topic) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(s) = t.status.as_deref().filter(|s| !s.is_empty()) {
        parts.push(s.to_owned());
    }
    if let Some(p) = t.priority.as_deref().filter(|s| !s.is_empty()) {
        parts.push(p.to_owned());
    }
    if let Some(a) = t.assigned_to.as_deref().filter(|s| !s.is_empty()) {
        parts.push(a.to_owned());
    }
    parts.push(t!("bcf-comments", count = t.comments.len() as i64));
    specular_i18n::dot_join(&parts)
}

/// Draws the handles and the measurement (logical px).
pub(in crate::app) fn draw_overlay(dl: &mut DrawList, pal: &Palette, o: &Overlay, ts: f32) {
    for h in &o.handles {
        let col = if h.chosen { pal.accent } else { pal.text };
        dl.line(h.at, h.tip, 2.0 * ts, col);
        dl.circle(h.at, (HANDLE_R + 1.5) * ts, pal.canvas);
        dl.circle(h.at, HANDLE_R * ts, col);
        dl.circle(h.tip, 3.0 * ts, col);
    }
    if let Some(p) = o.first {
        dl.circle(p, 4.0 * ts, pal.accent);
    }
    if let Some((a, b, _)) = &o.measure {
        dl.line(*a, *b, 2.0 * ts, pal.accent);
        dl.circle(*a, 4.0 * ts, pal.accent);
        dl.circle(*b, 4.0 * ts, pal.accent);
    }
}

/// The prompt's field as drawn: its rectangle, its laid-out text and the x that text starts
/// at (logical px).
pub(in crate::app) type PromptDrawn = (Rect, Arc<TextBlock>, f32);

/// The panel as drawn: its rectangle, each row's, where the prompt's input method window
/// opens and the prompt's field.
pub(in crate::app) type PanelDrawn = (Rect, Vec<(usize, Rect)>, Option<Rect>, Option<PromptDrawn>);

/// Draws the panel at the top-end corner of `area` (logical x, y, w, h): the title, the
/// rows (the chosen one marked), notes, the prompt and the keys. Returns the panel, each
/// row's rectangle, where the prompt's input method candidate window opens and the
/// prompt's field.
pub(in crate::app) fn draw_panel(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    p: &PanelView,
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) -> PanelDrawn {
    let (ax, ay, aw, _ah) = area;
    let m = TOOLBAR_MARGIN;
    let st = TextStyle::new(FONT_SMALL * ts);
    let w = (PANEL_W * ts).min(aw - 2.0 * m).max(160.0);
    let inner = w - 2.0 * PAD;
    let lay = |text: &mut Text, s: &str| text.layout(s, st, Some(inner), Align::Start);
    let title = lay(text, &p.title);
    let row_h = (FONT_SMALL * 1.9 * ts).round();
    // The rows may be a window of a longer list: scroll by the chosen row's place in them.
    let first = p
        .chosen
        .and_then(|c| p.rows.iter().position(|r| r.index == c))
        .map_or(0, |c| c.saturating_sub(MAX_ROWS - 1))
        .min(p.rows.len().saturating_sub(MAX_ROWS));
    let shown: Vec<&PanelRow> = p.rows.iter().skip(first).take(MAX_ROWS).collect();
    let notes: Vec<_> = p.notes.iter().map(|n| lay(text, n)).collect();
    let empty = p.empty.as_ref().map(|e| lay(text, e));
    let hints = lay(text, &p.hints);
    // The prompt's text on one line, with the composition at the caret.
    let prompt = p.prompt.as_ref().map(|(l, s, caret, pre, sel)| {
        let (shown, range) = specular_widgets::line_edit::compose(s, *caret, pre);
        (
            lay(text, l),
            text.layout(&shown, st, None, Align::Start),
            range,
            sel.clone(),
        )
    });
    let gap = 6.0 * ts;
    let mut h = 2.0 * PAD + title.height + gap;
    h += shown.len() as f32 * row_h;
    h += empty.as_ref().map_or(0.0, |e| e.height + gap);
    h += notes.iter().map(|n| n.height).sum::<f32>() + if notes.is_empty() { 0.0 } else { gap };
    if let Some((l, s, ..)) = &prompt {
        h += l.height + s.height.max(row_h) + gap;
    }
    h += hints.height + gap;
    let x = if rtl { ax + m } else { ax + aw - m - w };
    let y = ay + m;
    let panel = Rect::new(x, y, w, h);
    dl.glass(panel, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let tx = |b: f32| if rtl { x + w - PAD - b } else { x + PAD };
    let mut cy = y + PAD;
    text.draw(dl, &title, tx(title.width), cy, pal.text);
    cy += title.height + gap;
    let mut rows = Vec::with_capacity(shown.len());
    for r in shown {
        let rr = Rect::new(x + PAD / 2.0, cy, w - PAD, row_h);
        let chosen = p.chosen == Some(r.index);
        if chosen {
            dl.rect(rr, 4.0 * ts, pal.accent_soft);
        }
        // A name from the file (a model's file name, an annotation's "100 ±0.2") keeps its
        // own order, and in a right-to-left UI the row reads right to left from it.
        let dir = if rtl { Direction::Rtl } else { Direction::Ltr };
        let label = join_parts(&[&r.text, r.detail.as_deref().unwrap_or("")], " — ", dir);
        let b = text.layout(&cut(&label, ROW_CHARS), st, None, Align::Start);
        let ink = if chosen { pal.text } else { pal.text_muted };
        text.draw(dl, &b, tx(b.width), cy + (row_h - b.height) / 2.0, ink);
        rows.push((r.index, rr));
        cy += row_h;
    }
    if let Some(e) = &empty {
        text.draw(dl, e, tx(e.width), cy, pal.text_muted);
        cy += e.height + gap;
    }
    if !notes.is_empty() {
        cy += gap / 2.0;
        for n in &notes {
            text.draw(dl, n, tx(n.width), cy, pal.text_muted);
            cy += n.height;
        }
        cy += gap / 2.0;
    }
    let mut ime = None;
    let mut field_drawn = None;
    if let Some((l, s, range, sel)) = &prompt {
        text.draw(dl, l, tx(l.width), cy, pal.text_muted);
        cy += l.height;
        let field = Rect::new(x + PAD / 2.0, cy, w - PAD, s.height.max(row_h));
        dl.rect_border(field, 4.0 * ts, pal.surface, pal.focus_ring, 1.5 * ts);
        let fy = cy + (field.h - s.height) / 2.0;
        // The caret where the shaper puts it (right to left and mixed text too), the text
        // scrolled to keep it in view.
        let room = Rect::new(x + PAD, field.y, w - 2.0 * PAD, field.h);
        let cx = s.caret_x(range.end);
        let x0 = specular_widgets::line_edit::field_x0(s, cx, room, rtl || s.rtl);
        dl.push_clip(room.inset_xy(-2.0, 0.0));
        if range.is_empty() {
            for (a, b) in s.range_spans(sel.clone()) {
                dl.rect(Rect::new(x0 + a, fy, b - a, s.height), 0.0, pal.selection);
            }
        }
        text.draw(dl, s, x0, fy, pal.text);
        let caret = Rect::new(x0 + cx - 0.75, fy, 1.5, s.height);
        dl.line((x0 + cx, fy), (x0 + cx, fy + s.height), 1.5 * ts, pal.text);
        ime = Some(if range.is_empty() {
            caret
        } else {
            specular_widgets::line_edit::draw_preedit_underline(
                dl,
                s,
                x0,
                range.clone(),
                fy + s.height,
                pal.text_muted,
            );
            specular_widgets::line_edit::ime_area(s, x0, range.clone(), fy, s.height, room)
        });
        dl.pop_clip();
        field_drawn = Some((field, s.clone(), x0));
        cy += field.h + gap;
    }
    text.draw(dl, &hints, tx(hints.width), cy, pal.text_faint);
    (panel, rows, ime, field_drawn)
}

/// The panel's AccessKit nodes: a list of its rows (the chosen one selected) and the
/// prompt's text field.
pub(in crate::app) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    p: &PanelView,
    panel: Rect,
    rows: &[(usize, Rect)],
    sf: f64,
) {
    use accesskit::{Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * sf,
        y0: f64::from(r.y) * sf,
        x1: f64::from(r.x + r.w) * sf,
        y1: f64::from(r.y + r.h) * sf,
    };
    let mut g = Node::new(Role::List);
    g.set_label(p.title.clone());
    g.set_description(p.hints.clone());
    g.set_bounds(bounds(panel));
    let mut nodes = Vec::new();
    for (i, r) in rows {
        let Some(row) = p.rows.iter().find(|x| x.index == *i) else {
            continue;
        };
        let id = NodeId(A11Y_ROW + (*i as u64).min(0xFFFF));
        let mut n = Node::new(Role::ListItem);
        n.set_label(match &row.detail {
            Some(d) => format!("{}, {d}", row.text),
            None => row.text.clone(),
        });
        n.set_bounds(bounds(*r));
        n.set_selected(p.chosen == Some(*i));
        n.add_action(accesskit::Action::Click);
        g.push_child(id);
        nodes.push((id, n));
    }
    if let Some((label, value, ..)) = &p.prompt {
        let id = NodeId(A11Y_PROMPT);
        let mut n = Node::new(Role::TextInput);
        n.set_label(label.clone());
        n.set_value(value.clone());
        n.set_bounds(bounds(panel));
        g.push_child(id);
        nodes.push((id, n));
    }
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_PANEL));
    }
    tree.nodes.push((NodeId(A11Y_PANEL), g));
    tree.nodes.extend(nodes);
}

/// The section handles as nodes (labelled by plane or box face).
pub(in crate::app) fn handle_nodes(
    tree: &mut accesskit::TreeUpdate,
    handles: &[HandleView],
    rtl: bool,
    sf: f64,
) {
    use accesskit::{Node, NodeId, Role};
    // In reading order: top to bottom, then along the line.
    let mut order: Vec<&HandleView> = handles.iter().collect();
    order.sort_by(|a, b| {
        let (ya, yb) = (a.at.1, b.at.1);
        let (xa, xb) = if rtl {
            (-a.at.0, -b.at.0)
        } else {
            (a.at.0, b.at.0)
        };
        ya.total_cmp(&yb).then(xa.total_cmp(&xb))
    });
    for (k, h) in order.into_iter().enumerate() {
        let mut n = Node::new(Role::Button);
        n.set_label(match h.handle {
            Handle::Plane(i) => t!("bim-handle", kind = "plane", n = (i + 1) as i64),
            Handle::BoxFace(f) => t!("bim-handle", kind = "box", n = (f + 1) as i64),
        });
        let r = HANDLE_HIT as f64;
        n.set_bounds(accesskit::Rect {
            x0: (f64::from(h.at.0) - r) * sf,
            y0: (f64::from(h.at.1) - r) * sf,
            x1: (f64::from(h.at.0) + r) * sf,
            y1: (f64::from(h.at.1) + r) * sf,
        });
        tree.nodes.push((NodeId(A11Y_HANDLE + k as u64), n));
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
            root.push_child(NodeId(A11Y_HANDLE + k as u64));
        }
    }
}

#[cfg(test)]
mod shots;

#[cfg(test)]
mod tests {
    use super::*;

    /// A press in the prompt's field puts the caret where it lands and a drag selects
    /// what the pointer passes over; outside the field, or with no prompt, nothing.
    #[test]
    fn the_prompt_takes_presses_and_drags() {
        let name = "Level 2 north";
        let mut text = Text::bundled_only();
        text.begin_frame(1.0);
        let block = text.layout(name, TextStyle::new(14.0), None, Align::Start);
        let x0 = 40.0;
        let x_of = |b: usize| x0 + block.caret_x(b);
        let mut b = Bim::default();
        let field = Rect::new(30.0, 100.0, 300.0, 24.0);
        b.field = Some((field, block.clone(), x0));
        assert!(!b.prompt_press((x_of(3), 110.0), 1.0, false), "no prompt");
        b.prompt = Some(Prompt {
            asking: Asking::NewView,
            field: LineEdit::new(name),
            preedit: String::new(),
        });
        assert!(
            !b.prompt_press((x_of(3), 140.0), 1.0, false),
            "below the field"
        );
        assert!(!b.prompt_drag(x_of(5)), "no press");
        assert!(b.prompt_press((x_of(6), 110.0), 1.0, false));
        let p = b.prompt.as_ref().expect("open");
        assert_eq!(p.field.caret(), 6);
        assert!(b.prompt_drag(x_of(13)));
        let p = b.prompt.as_mut().expect("open");
        assert_eq!(p.field.selected_text(), "2 north");
        p.field.release();
        assert!(!b.prompt_drag(x_of(1)));
    }

    #[test]
    fn poses_blend_smoothly() {
        let a = Pose {
            eye: [0.0; 3],
            direction: [1.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            projection: Projection::Perspective,
            fov: 0.6,
            height: 1.0,
            distance: 1.0,
        };
        let b = Pose {
            eye: [10.0, 0.0, 0.0],
            direction: [0.0, 1.0, 0.0],
            fov: 1.0,
            ..a
        };
        assert_eq!(blend(&a, &b, 0.0), a);
        let e = blend(&a, &b, 1.0);
        assert!((e.eye[0] - 10.0).abs() < 1e-12 && (e.direction[1] - 1.0).abs() < 1e-12);
        let mid = blend(&a, &b, 0.5);
        assert!((mid.eye[0] - 5.0).abs() < 1e-12);
        assert!((dot3(mid.direction, mid.direction) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn topics_read_as_one_line() {
        let t = Topic {
            title: "Clash".into(),
            status: Some("Open".into()),
            priority: Some("High".into()),
            assigned_to: Some("alice".into()),
            comments: vec![Comment::default(); 3],
            ..Topic::default()
        };
        let d = topic_detail(&t);
        assert!(d.starts_with("Open · High · alice · "), "{d}");
        assert_eq!(cut("abcdef", 4), "abc…");
    }
}
