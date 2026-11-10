//! Linked views in medical mode (DESIGN.md §7.3): the series of a folder or disc side by side
//! in up to 3 × 3 viewports, linked through [`specular_dicom::linked`].
//!
//! `Shift+Y` (or "Compare series" in the palette) opens it over the image viewer in series
//! mode; Esc or the same key closes it. Each viewport shows one series, a slice at a time,
//! decoded off the UI thread ([`load`]) and windowed on the CPU into a texture.
//!
//! - **Viewports:** `Alt+J` steps the layouts, F6 / Shift+F6 or a click pick the active one;
//!   a series is dragged from the strip under the viewports (or out of the Studies browser)
//!   onto a viewport, or clicked into the active one.
//! - **Scrolling together:** series sharing a Frame of Reference (or linked by patient
//!   position, or aligned by hand) follow the active one to the slice at the same position
//!   along their normal ([`Stack::follow`]); a gap or the series' end is flagged.
//! - **Reference lines** (`Alt+H`): the active slice and its slab on the other viewports.
//! - **3D cursor:** Shift+click; the other viewports go to the slice holding the point.
//! - **Window sync** (`Shift+W`: off, same modality, all) and **zoom sync** (`Alt+Z`: the
//!   same logical pixels per millimetre and pan everywhere).
//! - **Priors** (`Shift+J`): another Frame of Reference is aligned at the slices shown, by an
//!   offset along the normal, kept for that pair of studies for the session; `Alt+Shift+J`
//!   picks how the active viewport follows (frame of reference, patient position, off).
//!
//! In a viewport: the wheel steps slices (Ctrl+wheel zooms), left or middle drag pans, right
//! drag sets the window; `.` `,` Page Up/Down and the arrow keys step the active one, the
//! digits pick window presets.

mod draw;
mod load;
#[cfg(test)]
mod shots;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use specular_dicom::Anatomy;
use specular_dicom::linked::{Fit, Link, Plane, Stack, reference_line};
use specular_dicom::series::{Scan, Series};
use specular_gfx::{Gpu, TexId, UiRenderer, wgpu};
use specular_i18n::t;
use specular_input::Action;
use specular_render::{PxRect, VoiShape, Window as WlWindow};
use specular_theme::Palette;
use specular_widgets::accesskit;

use super::App;
use super::medical::labels::{Edge, Side};
use super::medical::{CT_PRESETS, dragged, number, range_window};
use draw::{Control, Grid, Img, Layout, Mapping, Placed, PortScene, R, Scene};
use load::{Key, Slice};

/// Most viewports (3 × 3).
const PORTS: usize = 9;
/// Decoded slices kept for scrolling back, bytes.
const CACHE_BYTES: usize = 256 << 20;
/// A press moves this far (logical pixels) before it drags a series.
const DRAG_SLOP: f64 = 6.0;

/// Linked-views state: the open comparison, if any, and the manual alignments made this
/// session (kept when the viewports close).
#[derive(Default)]
pub(super) struct Linked {
    s: Option<Box<Session>>,
    /// Bumped whenever what is drawn changes.
    rev: u64,
    /// Manual alignments: per pair of studies (or of series within one study), ordered, the
    /// link from the first to the second.
    aligned: HashMap<(String, String), Link>,
}

impl Linked {
    /// The GPU device was replaced: the viewports' textures are made again.
    pub(super) fn gpu_reset(&mut self) {
        if let Some(s) = self.s.as_mut() {
            for p in &mut s.ports {
                p.tex = None;
            }
        }
        self.rev += 1;
    }
}

/// A series that can be shown: its files and its slices' geometry.
pub(crate) struct Entry {
    pub series: Series<PathBuf>,
    pub stack: Stack,
}

impl Entry {
    fn new(series: Series<PathBuf>) -> Option<Entry> {
        let stack = Stack::of(&series)?;
        stack
            .planes
            .iter()
            .all(|p| p.size[0] > 0 && p.size[1] > 0)
            .then_some(Entry { series, stack })
    }

    /// "3 · CT · Axial 1 mm".
    fn label(&self) -> String {
        let s = &self.series;
        let description = if s.description.is_empty() {
            "\u{2014}".to_owned()
        } else {
            s.description.clone()
        };
        t!(
            "linked-series",
            number = s
                .number
                .map_or_else(|| "\u{2014}".to_owned(), |n| n.to_string()),
            modality = s.modality.clone(),
            description = description
        )
    }

    /// The pair key of two entries' alignment, and whether `self` is its second.
    fn pair(&self, other: &Entry) -> ((String, String), bool) {
        let (a, b) = if self.series.study_uid != other.series.study_uid {
            (&self.series.study_uid, &other.series.study_uid)
        } else {
            (&self.series.series_uid, &other.series.series_uid)
        };
        if a <= b {
            ((a.clone(), b.clone()), false)
        } else {
            ((b.clone(), a.clone()), true)
        }
    }
}

/// How a viewport's series follows the others.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Follow {
    /// By Frame of Reference (or a manual alignment).
    #[default]
    Frame,
    /// By the patient positions as stored, whatever the Frame of Reference.
    Position,
    Off,
}

/// Which viewports a window change reaches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum WindowSync {
    Off,
    #[default]
    Modality,
    All,
}

/// A viewport's texture.
struct Tex {
    tex: wgpu::Texture,
    id: TexId,
    size: (u32, u32),
    /// What it shows: the slice and the window's bits.
    shows: (Key, [u64; 3]),
}

#[derive(Default)]
pub(crate) struct Port {
    pub set: Option<usize>,
    pub slice: usize,
    /// Logical pixels per mm (`None`: fit).
    pub zoom: Option<f64>,
    /// The image centre's offset from the viewport's centre, mm (right, down).
    pub pan: (f64, f64),
    /// The user's window (`None`: the file's).
    pub window: Option<WlWindow>,
    pub follow: Follow,
    /// The last sync: where the linked position fell, and how far from the slice shown.
    pub fit: Option<(Fit, f64)>,
    /// The planes cross the active one's: no scrolling together.
    pub crossing: bool,
    tex: Option<Tex>,
}

/// What a press started.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    Pan(usize, (f64, f64)),
    Window(usize, (f64, f64), WlWindow),
    Cursor(usize),
    /// A series picked up, where the press was, and whether it moves yet.
    Carry(Carry, (f64, f64), bool),
}

/// A series being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Carry {
    Entry(usize),
    /// A series of the Studies browser (its id there).
    Study(u64),
}

pub(crate) struct Session {
    pub entries: Vec<Entry>,
    pub ports: Vec<Port>,
    pub grid: Grid,
    pub active: usize,
    pub lines: bool,
    pub wsync: WindowSync,
    pub zsync: bool,
    /// The 3D cursor: the viewport it was placed in and the point in its coordinates.
    pub cursor: Option<(usize, [f64; 3])>,
    drag: Option<Drag>,
    pointer: Option<(f64, f64)>,
    layout: Option<Layout>,
    placed: Placed,
    /// The last scene drawn (for accessibility).
    last: Option<Scene>,
    cache: HashMap<Key, Arc<Slice>>,
    order: VecDeque<Key>,
    failed: HashMap<Key, String>,
    pending: std::collections::HashSet<Key>,
    loader: load::Loader,
    wheel: f64,
    /// A Studies browser series asked for: its id and the viewport it goes to.
    study: Option<(u64, usize)>,
    /// A press on a series of the open Studies browser: its id and where.
    study_press: Option<(u64, (f64, f64))>,
}

/// The effective window of a slice: the user's, else the file's, else its range.
fn window_for(user: Option<WlWindow>, s: Option<&Slice>) -> WlWindow {
    user.or_else(|| s.and_then(|s| s.window))
        .unwrap_or_else(|| {
            let (r, inv) = s.map_or(((0.0, 1.0), false), |s| (s.range, s.inverted));
            range_window(r, inv)
        })
}

fn window_bits(w: &WlWindow) -> [u64; 3] {
    [
        w.center.to_bits(),
        w.width.to_bits(),
        u64::from(w.invert) | (w.shape as u64) << 1,
    ]
}

impl Session {
    pub(crate) fn new(entries: Vec<Entry>, first: usize, loader: load::Loader) -> Session {
        let grid = draw::grid_for(entries.len());
        let mut ports: Vec<Port> = (0..PORTS).map(|_| Port::default()).collect();
        let n = entries.len();
        for (i, p) in ports.iter_mut().take(grid.count().min(n)).enumerate() {
            let e = (first + i) % n;
            p.set = Some(e);
            p.slice = entries[e].stack.middle();
        }
        let mut s = Session {
            entries,
            ports,
            grid,
            active: 0,
            lines: true,
            wsync: WindowSync::default(),
            zsync: false,
            cursor: None,
            drag: None,
            pointer: None,
            layout: None,
            placed: Placed::default(),
            last: None,
            cache: HashMap::new(),
            order: VecDeque::new(),
            failed: HashMap::new(),
            pending: Default::default(),
            loader,
            wheel: 0.0,
            study: None,
            study_press: None,
        };
        s.sync_from(0, &HashMap::new());
        s
    }

    fn visible(&self) -> usize {
        self.grid.count().min(PORTS)
    }

    pub(crate) fn entry(&self, port: usize) -> Option<&Entry> {
        self.entries.get(self.ports.get(port)?.set?)
    }

    pub(crate) fn plane(&self, port: usize) -> Option<&Plane> {
        let p = self.ports.get(port)?;
        self.entries.get(p.set?)?.stack.planes.get(p.slice)
    }

    fn key(&self, port: usize) -> Option<Key> {
        let p = self.ports.get(port)?;
        Some((p.set?, p.slice))
    }

    /// How viewport `a`'s coordinates map into `b`'s, when they are linked.
    pub(crate) fn link(
        &self,
        a: usize,
        b: usize,
        aligned: &HashMap<(String, String), Link>,
    ) -> Option<Link> {
        let (ea, eb) = (self.entry(a)?, self.entry(b)?);
        let (fa, fb) = (self.ports[a].follow, self.ports[b].follow);
        if fa == Follow::Off || fb == Follow::Off {
            return None;
        }
        if self.ports[a].set == self.ports[b].set
            || !ea.stack.frame_of_reference.is_empty()
                && ea.stack.frame_of_reference == eb.stack.frame_of_reference
        {
            return Some(Link::Frame);
        }
        let (key, reversed) = ea.pair(eb);
        if let Some(l) = aligned.get(&key) {
            return Some(if reversed { l.inverse() } else { *l });
        }
        (fa == Follow::Position || fb == Follow::Position).then_some(Link::Position)
    }

    /// Scrolling together: every other linked viewport goes to the slice at viewport `a`'s
    /// position.
    pub(crate) fn sync_from(&mut self, a: usize, aligned: &HashMap<(String, String), Link>) {
        let Some(src) = self.plane(a).copied() else {
            return;
        };
        for b in 0..self.visible() {
            if b == a || self.ports[b].set.is_none() {
                continue;
            }
            let link = self.link(a, b, aligned);
            let pick = link.and_then(|l| self.entry(b)?.stack.follow(&src, l));
            let p = &mut self.ports[b];
            match (link, pick) {
                (Some(_), Some(k)) => {
                    p.slice = k.slice;
                    p.fit = Some((k.fit, k.distance));
                    p.crossing = false;
                }
                (Some(_), None) => {
                    p.fit = None;
                    p.crossing = true;
                }
                (None, _) => {
                    p.fit = None;
                    p.crossing = false;
                }
            }
        }
        if let Some(p) = self.ports.get_mut(a) {
            p.fit = None;
            p.crossing = false;
        }
    }

    /// Steps viewport `port` by `d` slices, then the others follow.
    pub(crate) fn step(
        &mut self,
        port: usize,
        d: i64,
        aligned: &HashMap<(String, String), Link>,
    ) -> bool {
        let Some(n) = self.entry(port).map(|e| e.stack.planes.len()) else {
            return false;
        };
        let p = &mut self.ports[port];
        let to = (p.slice as i64 + d).clamp(0, n as i64 - 1) as usize;
        if to == p.slice {
            return false;
        }
        p.slice = to;
        self.sync_from(port, aligned);
        true
    }

    /// Shows slice `k` in viewport `port` (clamped), then the others follow.
    pub(crate) fn go_to(
        &mut self,
        port: usize,
        k: usize,
        aligned: &HashMap<(String, String), Link>,
    ) {
        let Some(n) = self.entry(port).map(|e| e.stack.planes.len()) else {
            return;
        };
        self.ports[port].slice = k.min(n.saturating_sub(1));
        self.sync_from(port, aligned);
    }

    /// The 3D cursor at pixel `px` of viewport `port`: every other linked viewport goes to
    /// the slice holding the point.
    pub(crate) fn place_cursor(
        &mut self,
        port: usize,
        px: (f64, f64),
        aligned: &HashMap<(String, String), Link>,
    ) {
        let Some(pl) = self.plane(port).copied() else {
            return;
        };
        let p = pl.at(px.0, px.1);
        self.cursor = Some((port, p));
        for b in 0..self.visible() {
            if b == port || self.ports[b].set.is_none() {
                continue;
            }
            let Some(link) = self.link(port, b, aligned) else {
                continue;
            };
            if let Some(l) = self.entry(b).and_then(|e| e.stack.locate(link.map(p))) {
                let q = &mut self.ports[b];
                q.slice = l.pick.slice;
                q.fit = Some((l.pick.fit, l.pick.distance));
                q.crossing = false;
            }
        }
    }

    /// Aligns every other shown series that is not linked by its Frame of Reference with the
    /// active one, at the slices shown. Returns how many.
    pub(crate) fn align(&mut self, aligned: &mut HashMap<(String, String), Link>) -> usize {
        let a = self.active;
        let Some(pa) = self.plane(a).copied() else {
            return 0;
        };
        let mut n = 0;
        for b in 0..self.visible() {
            if b == a || self.ports[b].follow == Follow::Off {
                continue;
            }
            let (Some(ea), Some(eb), Some(pb)) = (self.entry(a), self.entry(b), self.plane(b))
            else {
                continue;
            };
            if self.ports[a].set == self.ports[b].set
                || !ea.stack.frame_of_reference.is_empty()
                    && ea.stack.frame_of_reference == eb.stack.frame_of_reference
            {
                continue;
            }
            let link = Link::aligned(&pa, pb);
            let (key, reversed) = ea.pair(eb);
            aligned.insert(key, if reversed { link.inverse() } else { link });
            n += 1;
        }
        if n > 0 {
            self.sync_from(a, aligned);
        }
        n
    }

    /// Puts entry `e` in viewport `port` (at the slice linked to the active one, else its
    /// middle).
    pub(crate) fn show(
        &mut self,
        port: usize,
        e: usize,
        aligned: &HashMap<(String, String), Link>,
    ) {
        if e >= self.entries.len() || port >= PORTS {
            return;
        }
        let p = &mut self.ports[port];
        p.set = Some(e);
        p.slice = self.entries[e].stack.middle();
        p.zoom = None;
        p.pan = (0.0, 0.0);
        p.window = None;
        p.fit = None;
        p.crossing = false;
        if port != self.active && self.ports[self.active].set.is_some() {
            self.sync_from(self.active, aligned);
        } else {
            self.sync_from(port, aligned);
        }
    }

    fn zoom_of(&self, i: usize, r: R) -> Option<f64> {
        let pl = self.plane(i)?;
        Some(
            self.ports[i]
                .zoom
                .unwrap_or_else(|| Mapping::fit(r, pl.size, pl.spacing)),
        )
    }

    /// Viewport `i`'s pixel-to-screen mapping (with zoom sync, the active one's zoom and
    /// pan in millimetres).
    pub(crate) fn mapping(&self, i: usize) -> Option<Mapping> {
        let r = *self.layout.as_ref()?.ports.get(i)?;
        let pl = self.plane(i)?;
        let (zoom, pan) = if self.zsync && self.plane(self.active).is_some() {
            let ar = *self.layout.as_ref()?.ports.get(self.active)?;
            (self.zoom_of(self.active, ar)?, self.ports[self.active].pan)
        } else {
            (self.zoom_of(i, r)?, self.ports[i].pan)
        };
        Some(Mapping {
            port: r,
            size: pl.size,
            spacing: pl.spacing,
            zoom,
            pan,
        })
    }

    /// The viewport under logical point `p`.
    fn port_at(&self, p: (f64, f64)) -> Option<usize> {
        let l = self.layout.as_ref()?;
        (0..self.visible()).find(|&i| l.ports.get(i).is_some_and(|r| draw::contains(*r, p)))
    }

    /// Viewport `i`'s window.
    fn window(&self, i: usize) -> WlWindow {
        let s = self.key(i).and_then(|k| self.cache.get(&k));
        window_for(self.ports[i].window, s.map(Arc::as_ref))
    }

    /// Sets viewport `a`'s window and, by the sync, the others' (each keeps its inversion).
    pub(crate) fn set_window(&mut self, a: usize, w: WlWindow) {
        let modality = self.entry(a).map(|e| e.series.modality.clone());
        for b in 0..self.visible() {
            let reach = b == a
                || match self.wsync {
                    WindowSync::Off => false,
                    WindowSync::All => true,
                    WindowSync::Modality => {
                        self.entry(b).map(|e| &e.series.modality) == modality.as_ref()
                    }
                };
            if reach && self.ports[b].set.is_some() {
                let invert = if b == a {
                    w.invert
                } else {
                    self.window(b).invert
                };
                self.ports[b].window = Some(WlWindow { invert, ..w });
            }
        }
    }

    /// Back to the files' windows, by the sync.
    fn reset_window(&mut self, a: usize) {
        let modality = self.entry(a).map(|e| e.series.modality.clone());
        for b in 0..self.visible() {
            let reach = b == a
                || match self.wsync {
                    WindowSync::Off => false,
                    WindowSync::All => true,
                    WindowSync::Modality => {
                        self.entry(b).map(|e| &e.series.modality) == modality.as_ref()
                    }
                };
            if reach {
                self.ports[b].window = None;
            }
        }
    }

    /// Zooms viewport `i` by `k` (all of them with zoom sync, which follow the active one).
    fn zoom_by(&mut self, i: usize, k: f64) {
        let i = if self.zsync { self.active } else { i };
        let Some(r) = self.layout.as_ref().and_then(|l| l.ports.get(i).copied()) else {
            return;
        };
        if let Some(z) = self.zoom_of(i, r) {
            self.ports[i].zoom = Some((z * k).clamp(0.01, 400.0));
        }
    }

    fn fit_zoom(&mut self, i: usize) {
        let all = self.zsync;
        for (b, p) in self.ports.iter_mut().enumerate() {
            if all || b == i {
                p.zoom = None;
                p.pan = (0.0, 0.0);
            }
        }
    }

    /// Keeps the cache under its budget (the slices shown stay).
    fn insert(&mut self, k: Key, s: Arc<Slice>) {
        self.cache.insert(k, s);
        self.order.retain(|x| *x != k);
        self.order.push_back(k);
        let shown: Vec<Key> = (0..self.visible()).filter_map(|i| self.key(i)).collect();
        let mut bytes: usize = self.cache.values().map(|s| s.bytes()).sum();
        let mut i = 0;
        while bytes > CACHE_BYTES && i < self.order.len() {
            let k = self.order[i];
            if shown.contains(&k) {
                i += 1;
                continue;
            }
            self.order.remove(i);
            if let Some(s) = self.cache.remove(&k) {
                bytes -= s.bytes();
            }
        }
    }

    /// The slices the visible viewports need that are not in or on their way.
    fn wanted(&self) -> Vec<(usize, Key, PathBuf, u32)> {
        let mut out = Vec::new();
        for i in 0..self.visible() {
            let Some(k) = self.key(i) else { continue };
            if self.cache.contains_key(&k)
                || self.pending.contains(&k)
                || self.failed.contains_key(&k)
            {
                continue;
            }
            let e = &self.entries[k.0];
            let Some(&(item, frame)) = e.stack.source.get(k.1) else {
                continue;
            };
            let Some(it) = e.series.items.get(item) else {
                continue;
            };
            out.push((i, k, it.key.clone(), frame));
        }
        out
    }

    /// The bar's buttons.
    fn controls(&self) -> Vec<(Control, String, Option<bool>)> {
        let link = t!(
            "linked-link",
            mode = match self.ports[self.active].follow {
                Follow::Frame => "frame",
                Follow::Position => "position",
                Follow::Off => "off",
            }
        );
        vec![
            (
                Control::Layout,
                t!(
                    "linked-layout",
                    cols = i64::from(self.grid.cols),
                    rows = i64::from(self.grid.rows)
                ),
                None,
            ),
            (Control::Lines, t!("linked-lines"), Some(self.lines)),
            (
                Control::Window,
                t!(
                    "linked-window-sync",
                    mode = match self.wsync {
                        WindowSync::Off => "off",
                        WindowSync::Modality => "modality",
                        WindowSync::All => "all",
                    }
                ),
                None,
            ),
            (Control::Zoom, t!("linked-zoom"), Some(self.zsync)),
            (Control::Link, link, None),
            (Control::Align, t!("linked-align"), None),
            (Control::Close, t!("app-close"), None),
        ]
    }

    /// Everything drawn, with `tex` the viewports' textures (absent: no image yet).
    pub(crate) fn scene(
        &self,
        aligned: &HashMap<(String, String), Link>,
        labels: bool,
        tex: &dyn Fn(usize) -> Option<TexId>,
    ) -> Option<Scene> {
        let layout = self.layout.clone()?;
        let mut ports = Vec::new();
        let active_plane = self.plane(self.active).copied();
        for i in 0..self.visible() {
            let rect = *layout.ports.get(i)?;
            let mut ps = PortScene {
                rect,
                ..PortScene::default()
            };
            let (Some(e), Some(pl), Some(m)) = (self.entry(i), self.plane(i), self.mapping(i))
            else {
                ps.empty = true;
                ports.push(ps);
                continue;
            };
            let port = &self.ports[i];
            if let Some(id) = tex(i) {
                ps.image = Some(Img {
                    tex: id,
                    rect: draw::rect(m.image_rect()),
                });
            }
            // Readout: series, slice and position, window.
            ps.lines.push(e.label());
            let along = pl.position();
            ps.lines.push(t!(
                "linked-slice",
                n = (port.slice + 1) as i64,
                count = e.stack.planes.len() as i64,
                position = number((along * 10.0).round() / 10.0)
            ));
            let loaded = self.key(i).is_some_and(|k| self.cache.contains_key(&k));
            if let Some(err) = self.key(i).and_then(|k| self.failed.get(&k)) {
                ps.lines.push(err.clone());
            } else if loaded {
                let w = self.window(i);
                ps.lines.push(t!(
                    "medical-window",
                    center = number(w.center.round()),
                    width = number(w.width.round())
                ));
            } else {
                ps.lines.push(t!("omero-loading"));
            }
            if i != self.active
                && let Some(Link::Offset(d)) = self.link(self.active, i, aligned)
            {
                let mm = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                ps.lines.push(t!(
                    "linked-aligned",
                    offset = number((mm * 10.0).round() / 10.0)
                ));
            }
            // Flags: the linked position in a gap or outside, or not linked at all.
            if i != self.active && active_plane.is_some() {
                let link = self.link(self.active, i, aligned);
                ps.flag = match (link, port.fit, port.crossing) {
                    (None, _, _) => Some(t!("linked-not-linked")),
                    (Some(_), _, true) => None,
                    (Some(_), Some((Fit::Gap, d)), _) => Some(t!(
                        "linked-in-gap",
                        distance = number((d.abs() * 10.0).round() / 10.0)
                    )),
                    (Some(_), Some((Fit::Outside, d)), _) => Some(t!(
                        "linked-outside",
                        distance = number((d.abs() * 10.0).round() / 10.0)
                    )),
                    _ => None,
                };
            }
            // Reference lines of the active slice.
            if self.lines
                && i != self.active
                && let Some(src) = active_plane
                && let Some(link) = self.link(self.active, i, aligned)
                && let Some(rl) = reference_line(&src, pl, link, None)
            {
                let to = |s: specular_dicom::linked::Segment| {
                    let a = m.to_screen(s.a.0, s.a.1);
                    let b = m.to_screen(s.b.0, s.b.1);
                    [(a.0 as f32, a.1 as f32), (b.0 as f32, b.1 as f32)]
                };
                ps.refs.extend(rl.centre.map(to));
                ps.faces.extend(rl.slab.iter().flatten().map(|s| to(*s)));
            }
            // The 3D cursor, where it lies within this slice.
            if let Some((c, p)) = self.cursor {
                let link = if c == i {
                    Some(Link::Frame)
                } else {
                    self.link(c, i, aligned)
                };
                if let Some(link) = link {
                    let (x, y, d) = pl.project(link.map(p));
                    let half = (pl.thickness.max(e.stack.spacing) / 2.0).max(0.5);
                    if d.abs() <= half + 1e-6 && pl.contains(x, y) {
                        let s = m.to_screen(x, y);
                        ps.cursor = Some((s.0 as f32, s.1 as f32));
                    }
                }
            }
            if labels {
                ps.edges = edges(pl, rect, e.stack.anatomy);
            }
            ports.push(ps);
        }
        let shown = self.ports[self.active].set;
        let chips = self
            .entries
            .iter()
            .enumerate()
            .map(|(k, e)| {
                (
                    k,
                    t!(
                        "linked-chip",
                        label = e.label(),
                        count = e.stack.planes.len() as i64
                    ),
                    shown == Some(k),
                )
            })
            .collect();
        let ghost = match self.drag {
            Some(Drag::Carry(c, _, true)) => self.pointer.map(|p| {
                let label = match c {
                    Carry::Entry(k) => self.entries.get(k).map_or_else(String::new, Entry::label),
                    Carry::Study(_) => t!("dicom-series-group"),
                };
                (label, (p.0 as f32, p.1 as f32))
            }),
            _ => None,
        };
        let cursor = self.cursor.map(|(_, p)| {
            let [x, y, z] = p.map(|v| number((v * 10.0).round() / 10.0));
            t!("linked-cursor", x = x, y = y, z = z)
        });
        Some(Scene {
            layout,
            ports,
            active: self.active,
            controls: self.controls(),
            chips,
            ghost,
            cursor,
        })
    }
}

impl Session {
    /// A press at logical `lp`: a bar button (returned, for the caller to run), a series of
    /// the strip picked up, or a viewport made active and a drag started in it (Shift+left
    /// places the 3D cursor, left or middle pans, right sets the window).
    pub(crate) fn press(
        &mut self,
        lp: (f64, f64),
        b: specular_widgets::Button,
        shift: bool,
        aligned: &HashMap<(String, String), Link>,
    ) -> Option<Control> {
        use specular_widgets::Button;
        self.pointer = Some(lp);
        let fp = (lp.0 as f32, lp.1 as f32);
        if let Some(&(c, _)) = self.placed.controls.iter().find(|(_, r)| r.contains(fp)) {
            return (b == Button::Primary).then_some(c);
        }
        if let Some(&(k, _)) = self.placed.chips.iter().find(|(_, r)| r.contains(fp)) {
            if b == Button::Primary {
                self.drag = Some(Drag::Carry(Carry::Entry(k), lp, false));
            }
            return None;
        }
        let i = self.port_at(lp)?;
        let was = self.active;
        self.active = i;
        match b {
            Button::Primary if shift => {
                if let Some(m) = self.mapping(i) {
                    self.place_cursor(i, m.to_pixel(lp), aligned);
                    self.drag = Some(Drag::Cursor(i));
                }
            }
            Button::Primary | Button::Middle => self.drag = Some(Drag::Pan(i, lp)),
            Button::Secondary => {
                let w = self.window(i);
                self.drag = Some(Drag::Window(i, lp, w));
            }
        }
        if was != i {
            self.sync_from(i, aligned);
        }
        None
    }

    /// The pointer moved to logical `lp` during a drag; false without one.
    pub(crate) fn motion(
        &mut self,
        lp: (f64, f64),
        aligned: &HashMap<(String, String), Link>,
    ) -> bool {
        self.pointer = Some(lp);
        let Some(d) = self.drag else {
            return false;
        };
        match d {
            Drag::Pan(i, last) => {
                let j = if self.zsync { self.active } else { i };
                if let Some(m) = self.mapping(i) {
                    let p = &mut self.ports[j].pan;
                    p.0 += (lp.0 - last.0) / m.zoom;
                    p.1 += (lp.1 - last.1) / m.zoom;
                }
                self.drag = Some(Drag::Pan(i, lp));
            }
            Drag::Window(i, start, w0) => {
                let span = self
                    .key(i)
                    .and_then(|k| self.cache.get(&k))
                    .map_or(1.0, |c| c.range.1 - c.range.0);
                let w = dragged(w0, lp.0 - start.0, lp.1 - start.1, span);
                self.set_window(i, w);
            }
            Drag::Cursor(i) => {
                if let Some(m) = self.mapping(i) {
                    self.place_cursor(i, m.to_pixel(lp), aligned);
                }
            }
            Drag::Carry(c, at, moving) => {
                if !moving && (lp.0 - at.0).hypot(lp.1 - at.1) > DRAG_SLOP {
                    self.drag = Some(Drag::Carry(c, at, true));
                }
            }
        }
        true
    }

    /// The button went up at logical `lp`: a series carried is dropped on the viewport under
    /// it (a click on a strip series shows it in the active one). A Studies browser series
    /// comes back with its viewport, to be read first.
    pub(crate) fn release(
        &mut self,
        lp: (f64, f64),
        aligned: &HashMap<(String, String), Link>,
    ) -> Option<(u64, usize)> {
        self.pointer = Some(lp);
        self.study_press = None;
        let Some(Drag::Carry(c, _, moving)) = self.drag.take() else {
            return None;
        };
        let to = if moving {
            self.port_at(lp)?
        } else {
            self.active
        };
        match c {
            Carry::Entry(e) => {
                self.show(to, e, aligned);
                self.active = to;
                None
            }
            Carry::Study(id) => Some((id, to)),
        }
    }
}

/// The orientation letters at the edges of viewport `r` for slice `pl` (images are shown
/// unrotated: right is the row direction, down the column direction), in `anatomy`'s names
/// (a quadruped's "Le", "Cr", "D"… instead of L, H, A…).
fn edges(pl: &Plane, r: R, anatomy: Anatomy) -> Vec<Edge> {
    let neg = |d: [f64; 3]| d.map(|c| -c);
    let (x0, y0, x1, y1) = (r[0], r[1], r[0] + r[2], r[1] + r[3]);
    let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    [
        (Side::Left, neg(pl.row), (x0, my)),
        (Side::Right, pl.row, (x1, my)),
        (Side::Top, neg(pl.col), (mx, y0)),
        (Side::Bottom, pl.col, (mx, y1)),
    ]
    .into_iter()
    .filter_map(|(side, d, at)| {
        Some(Edge {
            side,
            text: anatomy.label(d)?,
            at: (at.0 as f32, at.1 as f32),
        })
    })
    .collect()
}

/// Uploads `rgba` into `tex` (or a new texture when the size changed).
fn upload(
    gpu: &Gpu,
    ui: &mut UiRenderer,
    tex: &mut Option<Tex>,
    w: u32,
    h: u32,
    rgba: &[u8],
    shows: (Key, [u64; 3]),
) {
    match tex {
        Some(t) if t.size == (w, h) => {
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &t.tex,
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
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
            t.shows = shows;
        }
        _ => {
            if let Some(old) = tex.take() {
                ui.unregister_texture(old.id);
            }
            let (t, id) = super::studies::upload_rgba(gpu, ui, w, h, rgba);
            *tex = Some(Tex {
                tex: t,
                id,
                size: (w, h),
                shows,
            });
        }
    }
}

impl Session {
    /// Brings the visible viewports' textures up to date.
    pub(crate) fn upload(&mut self, gpu: &Gpu, ui: &mut UiRenderer) {
        for i in 0..self.visible() {
            let Some(k) = self.key(i) else { continue };
            let Some(s) = self.cache.get(&k).cloned() else {
                continue;
            };
            let w = window_for(self.ports[i].window, Some(&s));
            let shows = (k, window_bits(&w));
            if self.ports[i].tex.as_ref().is_some_and(|t| t.shows == shows) {
                continue;
            }
            let rgba = s.rgba(&w);
            upload(
                gpu,
                ui,
                &mut self.ports[i].tex,
                s.width,
                s.height,
                &rgba,
                shows,
            );
        }
    }

    /// Frees the textures.
    pub(crate) fn free_textures(&mut self, ui: &mut UiRenderer) {
        for p in &mut self.ports {
            if let Some(t) = p.tex.take() {
                ui.unregister_texture(t.id);
            }
        }
    }

    pub(crate) fn tex_id(&self, i: usize) -> Option<TexId> {
        self.ports.get(i)?.tex.as_ref().map(|t| t.id)
    }
}

impl App {
    /// Linked views are open.
    pub(super) fn linked_active(&self) -> bool {
        self.linked.s.is_some()
    }

    /// Opens linked views on the series of series mode.
    fn linked_open(&mut self) {
        let now = self.now();
        if !self.series_active() {
            self.chrome.toast(t!("disabled-no-other-series"), None, now);
            return;
        }
        let active_uid = self.series_set().map(|s| s.series_uid.clone());
        let mut entries = Vec::new();
        let mut skipped = 0;
        for s in self.series_sets() {
            match Entry::new(s.clone()) {
                Some(e) => entries.push(e),
                None => skipped += 1,
            }
        }
        if entries.is_empty() {
            self.chrome.toast(t!("linked-no-geometry"), None, now);
            return;
        }
        if skipped > 0 {
            specular_log::debug!(target: specular_log::FOLDER, "linked views: {skipped} series without slice geometry left out");
        }
        let first = entries
            .iter()
            .position(|e| Some(&e.series.series_uid) == active_uid.as_ref())
            .unwrap_or(0);
        let n = entries.len();
        self.mpr_close();
        let loader = load::Loader::start(self.loader.waker());
        let mut s = Session::new(entries, first, loader);
        s.sync_from(0, &self.linked.aligned);
        self.linked.s = Some(Box::new(s));
        self.linked.rev += 1;
        self.chrome
            .toast(t!("linked-opened", count = n as i64), None, now);
        self.redraw = true;
    }

    pub(super) fn linked_close(&mut self) {
        if let Some(mut s) = self.linked.s.take() {
            if let Some(g) = self.gfx.as_mut() {
                s.free_textures(&mut g.ui);
            }
            self.linked.rev += 1;
            self.scene = None;
            self.redraw = true;
        }
    }

    fn linked_changed(&mut self) {
        self.linked.rev += 1;
        self.redraw = true;
    }

    /// Reads out the active viewport.
    fn linked_announce_active(&mut self) {
        let Some(s) = self.linked.s.as_ref() else {
            return;
        };
        let series = s
            .entry(s.active)
            .map_or_else(|| t!("linked-empty"), Entry::label);
        let text = t!("linked-active", n = (s.active + 1) as i64, series = series);
        self.chrome.announce_status(text);
    }

    /// A command while linked views are open (or opening them); true when taken.
    pub(super) fn linked_action(&mut self, a: Action) -> bool {
        if a == Action::CompareSeries {
            if self.linked_active() {
                self.linked_close();
            } else {
                self.linked_open();
            }
            return true;
        }
        if !self.linked_active() {
            return matches!(
                a,
                Action::CompareLayout
                    | Action::NextViewport
                    | Action::PrevViewport
                    | Action::ReferenceLines
                    | Action::SyncWindow
                    | Action::SyncZoom
                    | Action::LinkSeries
                    | Action::AlignSeries
            );
        }
        if a == Action::Mpr {
            // MPR opens on the series in place of the viewports.
            self.linked_close();
            return false;
        }
        let now = self.now();
        let aligned = &mut self.linked.aligned;
        let Some(s) = self.linked.s.as_mut() else {
            return false;
        };
        let active = s.active;
        let mut toast = None;
        let mut announce = false;
        match a {
            Action::CompareLayout => {
                let at = draw::GRIDS
                    .iter()
                    .position(|g| *g == s.grid)
                    .map_or(0, |i| i + 1);
                s.grid = draw::GRIDS[at % draw::GRIDS.len()];
                // New viewports take the series not shown yet.
                for i in 0..s.visible() {
                    if s.ports[i].set.is_none() {
                        let used: Vec<usize> = s.ports.iter().filter_map(|p| p.set).collect();
                        if let Some(e) = (0..s.entries.len()).find(|e| !used.contains(e)) {
                            s.show(i, e, aligned);
                        }
                    }
                }
                if s.active >= s.visible() {
                    s.active = 0;
                    announce = true;
                }
                s.layout = None;
                toast = Some(s.controls()[0].1.clone());
            }
            Action::NextViewport | Action::PrevViewport => {
                let n = s.visible();
                let d = if a == Action::NextViewport { 1 } else { n - 1 };
                s.active = (s.active + d) % n;
                s.sync_from(s.active, aligned);
                announce = true;
            }
            Action::ReferenceLines => s.lines = !s.lines,
            Action::SyncWindow => {
                s.wsync = match s.wsync {
                    WindowSync::Off => WindowSync::Modality,
                    WindowSync::Modality => WindowSync::All,
                    WindowSync::All => WindowSync::Off,
                };
                toast = Some(s.controls()[2].1.clone());
            }
            Action::SyncZoom => s.zsync = !s.zsync,
            Action::LinkSeries => {
                let p = &mut s.ports[active];
                p.follow = match p.follow {
                    Follow::Frame => Follow::Position,
                    Follow::Position => Follow::Off,
                    Follow::Off => Follow::Frame,
                };
                s.sync_from(active, aligned);
                toast = Some(s.controls()[4].1.clone());
            }
            Action::AlignSeries => {
                let n = s.align(aligned);
                toast = Some(t!("linked-aligned-count", count = n as i64));
            }
            Action::NextSlice | Action::NextPage | Action::NextImage | Action::PanPageDown => {
                s.step(active, 1, aligned);
            }
            Action::PrevSlice | Action::PrevPage | Action::PrevImage | Action::PanPageUp => {
                s.step(active, -1, aligned);
            }
            Action::FirstPage | Action::FirstImage => s.go_to(active, 0, aligned),
            Action::LastPage | Action::LastImage => s.go_to(active, usize::MAX, aligned),
            Action::WindowPreset(0) => s.reset_window(active),
            Action::WindowPreset(n) => {
                if let Some(&(_, width, level)) = CT_PRESETS.get(usize::from(n) - 1) {
                    let invert = s.window(active).invert;
                    s.set_window(
                        active,
                        WlWindow {
                            center: level,
                            width,
                            shape: VoiShape::Linear,
                            invert,
                        },
                    );
                }
            }
            Action::InvertGrey => {
                let w = s.window(active);
                s.ports[active].window = Some(WlWindow {
                    invert: !w.invert,
                    ..w
                });
            }
            Action::ZoomIn => s.zoom_by(active, 1.25),
            Action::ZoomOut => s.zoom_by(active, 0.8),
            Action::ZoomFit | Action::ZoomTo(_) => s.fit_zoom(active),
            _ => return false,
        }
        if let Some(t) = toast {
            self.chrome.toast(t, None, now);
        }
        if announce {
            self.linked_announce_active();
        }
        self.linked_changed();
        true
    }

    /// Esc: a drag, the 3D cursor, then the viewports.
    pub(super) fn linked_back(&mut self) -> bool {
        let Some(s) = self.linked.s.as_mut() else {
            return false;
        };
        if s.drag.take().is_some() || s.cursor.take().is_some() {
            self.linked_changed();
            return true;
        }
        self.linked_close();
        true
    }

    /// A press on a series of the open Studies browser (logical point): dragging it out
    /// carries it to a viewport.
    pub(super) fn linked_studies_press(&mut self, lp: (f32, f32)) {
        let id = self.chrome.studies_series_at(lp);
        if let Some(s) = self.linked.s.as_mut() {
            s.study_press = id.map(|id| (id, (f64::from(lp.0), f64::from(lp.1))));
        }
    }

    /// A button press or release at physical point `p`; true when taken.
    pub(super) fn linked_button(
        &mut self,
        p: (f32, f32),
        down: bool,
        b: specular_widgets::Button,
    ) -> bool {
        let scale = f64::from(self.dl.scale.max(0.1));
        let shift = self.mods.shift_key();
        let lp = (f64::from(p.0) / scale, f64::from(p.1) / scale);
        let Some(s) = self.linked.s.as_mut() else {
            return false;
        };
        let was = s.active;
        if !down {
            if let Some((id, to)) = s.release(lp, &self.linked.aligned) {
                s.study = Some((id, to));
                self.studies_open_series(id);
            }
        } else if let Some(c) = s.press(lp, b, shift, &self.linked.aligned) {
            self.linked_control(c);
            return true;
        }
        if self.linked.s.as_ref().is_some_and(|s| s.active != was) {
            self.linked_announce_active();
        }
        self.linked_changed();
        true
    }

    fn linked_control(&mut self, c: Control) {
        let a = match c {
            Control::Layout => Action::CompareLayout,
            Control::Lines => Action::ReferenceLines,
            Control::Window => Action::SyncWindow,
            Control::Zoom => Action::SyncZoom,
            Control::Link => Action::LinkSeries,
            Control::Align => Action::AlignSeries,
            Control::Close => Action::CompareSeries,
        };
        self.linked_action(a);
    }

    /// Pointer motion at physical `p`; true when taken.
    pub(super) fn linked_motion(&mut self, p: (f32, f32)) -> bool {
        let scale = f64::from(self.dl.scale.max(0.1));
        let lp = (f64::from(p.0) / scale, f64::from(p.1) / scale);
        let Some(s) = self.linked.s.as_mut() else {
            return false;
        };
        // A Studies browser series dragged out of the browser: the sheet closes and the
        // series follows the pointer.
        if let Some((id, at)) = s.study_press
            && (lp.0 - at.0).hypot(lp.1 - at.1) > 4.0 * DRAG_SLOP
        {
            s.study_press = None;
            s.pointer = Some(lp);
            s.drag = Some(Drag::Carry(Carry::Study(id), at, true));
            if self.chrome.studies_open() {
                self.chrome.close_modal();
            }
            self.linked_changed();
            return true;
        }
        if !s.motion(lp, &self.linked.aligned) {
            return false;
        }
        self.linked_changed();
        true
    }

    /// The wheel at physical `p`: slices of the viewport under it (Ctrl: zoom).
    pub(super) fn linked_wheel(&mut self, lines: (f32, f32), p: (f32, f32)) -> bool {
        let scale = f64::from(self.dl.scale.max(0.1));
        let lp = (f64::from(p.0) / scale, f64::from(p.1) / scale);
        let ctrl = self.mods.control_key();
        let Some(s) = self.linked.s.as_mut() else {
            return false;
        };
        let Some(i) = s.port_at(lp) else {
            return true;
        };
        let was = s.active;
        s.active = i;
        if ctrl {
            s.zoom_by(i, 1.1f64.powf(f64::from(lines.1)));
        } else {
            s.wheel -= f64::from(lines.1);
            let steps = s.wheel.trunc();
            s.wheel -= steps;
            if steps != 0.0 {
                s.step(i, steps as i64, &self.linked.aligned);
            }
        }
        if was != i {
            s.sync_from(i, &self.linked.aligned);
            self.linked_announce_active();
        }
        self.linked_changed();
        true
    }

    /// The viewports' rectangles for the image area `vp` (physical).
    pub(super) fn linked_layout(&mut self, vp: PxRect, rtl: bool) {
        let scale = f64::from(self.dl.scale.max(0.1));
        let ts = self.chrome.text_scale;
        let Some(s) = self.linked.s.as_mut() else {
            return;
        };
        let r = [
            f64::from(vp.x) / scale,
            f64::from(vp.y) / scale,
            f64::from(vp.w) / scale,
            f64::from(vp.h) / scale,
        ];
        let strip = if s.entries.is_empty() {
            0.0
        } else {
            draw::strip_height(ts)
        };
        let l = draw::layout(r, scale, rtl, s.grid, draw::bar_height(ts), strip);
        if s.layout.as_ref() != Some(&l) {
            s.layout = Some(l);
            self.linked.rev += 1;
        }
    }

    /// Takes the decoded slices, asks for the ones now needed; closes when series mode ended.
    pub(super) fn linked_tick(&mut self) {
        if self.linked.s.is_none() {
            return;
        }
        if !self.series_active() {
            self.linked_close();
            return;
        }
        let Some(s) = self.linked.s.as_mut() else {
            return;
        };
        let mut changed = false;
        for d in s.loader.poll() {
            s.pending.remove(&d.key);
            match d.slice {
                Ok(sl) => s.insert(d.key, sl),
                Err(e) => {
                    s.failed.insert(d.key, e);
                }
            }
            changed = true;
        }
        for (port, k, path, frame) in s.wanted() {
            s.pending.insert(k);
            s.loader.ask(port, k, path, frame);
        }
        if changed {
            self.linked_changed();
        }
    }

    /// Draws the viewports over the image area (uploading their slices first).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn linked_overlay(
        linked: &mut Linked,
        dl: &mut specular_gfx::DrawList,
        gpu: &Gpu,
        ui: &mut UiRenderer,
        text: &mut specular_gfx::Text,
        pal: &Palette,
        labels: bool,
        rtl: bool,
        ts: f32,
    ) {
        let Some(s) = linked.s.as_mut() else { return };
        s.upload(gpu, ui);
        let Some(sc) = s.scene(&linked.aligned, labels, &|i| s.tex_id(i)) else {
            return;
        };
        let placed = draw::draw(dl, text, &sc, pal, rtl, ts);
        s.placed = placed;
        s.last = Some(sc);
    }

    /// The viewports' AccessKit nodes.
    pub(super) fn linked_a11y(linked: &Linked, tree: &mut accesskit::TreeUpdate, scale: f64) {
        let Some(s) = linked.s.as_ref() else { return };
        if let Some(sc) = &s.last {
            draw::a11y(tree, sc, &s.placed, scale);
        }
    }

    /// Whether a Studies browser series is wanted in a viewport.
    pub(super) fn linked_wants_study(&self, id: u64) -> bool {
        self.linked
            .s
            .as_ref()
            .is_some_and(|s| s.study.is_some_and(|(x, _)| x == id))
    }

    /// Enter on a Studies browser series while comparing: it goes to the active viewport.
    pub(super) fn linked_study_to_active(&mut self, id: u64) -> bool {
        let Some(s) = self.linked.s.as_mut() else {
            return false;
        };
        s.study = Some((id, s.active));
        true
    }

    /// A Studies browser series' headers, read: its series join the strip and the largest
    /// shows in the viewport it was dropped on.
    pub(super) fn linked_take_study(&mut self, scan: Scan) {
        let now = self.now();
        let Some(s) = self.linked.s.as_mut() else {
            return;
        };
        let Some((_, port)) = s.study.take() else {
            return;
        };
        let mut best: Option<(usize, usize)> = None;
        for series in scan.series {
            // Already in the strip: shown from there.
            let k = match s.entries.iter().position(|e| {
                e.series.series_uid == series.series_uid && e.series.study_uid == series.study_uid
            }) {
                Some(k) => k,
                None => match Entry::new(series) {
                    Some(e) => {
                        s.entries.push(e);
                        s.entries.len() - 1
                    }
                    None => continue,
                },
            };
            let n = s.entries[k].stack.planes.len();
            if best.is_none_or(|(_, m)| n > m) {
                best = Some((k, n));
            }
        }
        match best {
            Some((k, _)) => {
                s.show(port.min(s.visible() - 1), k, &self.linked.aligned);
                s.active = port.min(s.visible() - 1);
                self.linked_announce_active();
            }
            None => {
                self.chrome.toast(t!("linked-no-geometry"), None, now);
            }
        }
        if self.chrome.studies_open() {
            self.chrome.close_modal();
        }
        self.linked_changed();
    }
}
