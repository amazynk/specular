//! The map view (DESIGN.md §10 "Map view"): the photos of the gallery (as filtered) that have
//! a GPS position, on an offline base map, clustered per zoom level.
//!
//! - The base map (`specular_map`: Natural Earth land, lakes and borders in Web Mercator) is
//!   rendered on the GPU into a texture of its own before the UI pass, only when the camera,
//!   size or theme changed, and shown under the markers.
//! - Markers are the clusters of the camera's zoom level (cells of 64 px): a disc with the
//!   count for several photos, a pin for one. A click on a cluster zooms to its photos (at the
//!   deepest zoom, or when they share a spot, it selects them in the grid instead); a click
//!   on a pin opens the photo; hovering shows the photo's thumbnail (the first one of a
//!   cluster) with its name or place.
//! - Drag or the arrow keys pan, the wheel zooms about the pointer, `+`/`-` about the centre,
//!   `0` shows every photo; Tab moves between markers and Enter clicks one; Escape closes.
//! - The header and the credit mirror for right-to-left languages; the map does not (north is
//!   up and east is right everywhere). Text and markers grow with the text scale.
//! - A photo's pin shows which way the camera looked (EXIF `GPSImgDirection`), read in the
//!   background for the pins on screen.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_gfx::{DrawList, Gpu, Icon, Rect, TexId, Text, TextStyle, UiRenderer, wgpu};
use specular_i18n::{I18n, t};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_map::{
    Camera, Cluster, Clusters, MAX_ZOOM, MapRenderer, Style, declutter, project, unproject,
};
use specular_theme::{Color, Mode as Theme, Palette};
use specular_widgets::{PaletteItem, accesskit};

use super::draw::{Frame, text_at, text_in, truncate};
use super::{A11Y_BASE, App, EntryKind, Gallery, item, tile_in};

/// The map's palette codes.
pub(super) mod code {
    pub const OPEN: u64 = 700;
    pub const ZOOM_IN: u64 = 701;
    pub const ZOOM_OUT: u64 = 702;
    pub const FIT: u64 = 703;
    pub const CLOSE: u64 = 704;
    /// The photos (the selected ones, else all) as KMZ or KML.
    pub const EXPORT_KML: u64 = 705;
    /// As a KMZ with the full photos in it.
    pub const EXPORT_KMZ_FULL: u64 = 706;

    pub fn owns(c: u64) -> bool {
        (700..750).contains(&c)
    }
}

/// The map's screen-reader nodes: the group, its controls, then one per marker on screen.
pub(super) const A11Y_MAP: u64 = A11Y_BASE + (10 << 40);
const A11Y_CONTROL: u64 = 1;
const A11Y_STATUS: u64 = 50;
const A11Y_CONSENT: u64 = 51;
const A11Y_MARKER: u64 = 100;
/// Markers listed for screen readers at most (the nearest the centre first).
const A11Y_MARKERS: usize = 400;

const TITLE: TextStyle = TextStyle::new(15.0).weight(600.0);
const BODY: TextStyle = TextStyle::new(13.0);
const SMALL: TextStyle = TextStyle::new(11.0);
const COUNT: TextStyle = TextStyle::new(12.0).weight(650.0);

/// Header height at text scale 1, logical px.
const HEADER: f32 = 44.0;
/// Zoom levels per wheel notch and per key press.
const WHEEL_STEP: f64 = 0.5;
const KEY_ZOOM: f64 = 1.0;
/// The share of the view an arrow key pans.
const KEY_PAN: f64 = 0.2;
/// A press that moves less than this is a click, not a drag (logical px).
const CLICK_SLOP: f32 = 4.0;
/// While positions are still coming in, the clusters are rebuilt at most this often.
const REBUILD_EVERY: Duration = Duration::from_millis(300);
/// Room around the photos when the map fits them (logical px).
const FIT_MARGIN: f64 = 48.0;
/// The deepest zoom a fit goes to (a single photo is not shown at street level).
const FIT_MAX_ZOOM: f64 = 14.0;

/// The header's buttons, in reading order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MAct {
    ZoomOut,
    ZoomIn,
    Fit,
    Close,
    /// The online base map's explanation: agree to the server.
    BasemapAllow,
    /// The explanation: keep the offline map (turns the online base map off).
    BasemapDecline,
}

/// The buttons screen readers list when they are on screen (the explanation's last).
const CONTROLS: [MAct; 6] = [
    MAct::ZoomOut,
    MAct::ZoomIn,
    MAct::Fit,
    MAct::Close,
    MAct::BasemapDecline,
    MAct::BasemapAllow,
];

/// A marker on screen this frame.
#[derive(Clone, Copy, Debug)]
pub(super) struct Marker {
    /// The cluster of the cell, or the biggest of those joined because they overlapped.
    pub cluster: Cluster,
    /// Photos, of every cluster joined.
    pub count: u32,
    /// The clusters joined (start and length in `MapView::parts`).
    parts: (u32, u32),
    /// Centre, logical px in the window.
    pub at: (f32, f32),
    pub radius: f32,
}

/// A press on the map: where it started, where the pointer was last, whether it moved.
type Drag = ((f32, f32), (f32, f32), bool);

/// The map view's state; `open` while it shows instead of the grid.
#[derive(Default)]
pub(super) struct MapView {
    pub open: bool,
    pub cam: Camera,
    /// The map's area in the window this frame (under the header), logical px.
    pub area: Rect,
    clusters: Arc<Clusters>,
    /// Path of each clustered photo (the clusters' ids index it).
    paths: Vec<PathBuf>,
    /// Latitude and longitude of each clustered photo (as `paths`).
    positions: Vec<(f64, f64)>,
    /// What the clusters were built from: (generation, tiles, positions known).
    key: (u64, usize, usize),
    built_at: Option<Instant>,
    /// Fitted to the photos once they came (or to `target`).
    fitted: bool,
    /// Moved or zoomed by the user: positions coming in no longer refit it.
    touched: bool,
    /// "Show on map": the photo to centre on when the map opens.
    pub target: Option<PathBuf>,
    pub markers: Vec<Marker>,
    /// The clusters of each marker this frame.
    parts: Vec<Cluster>,
    pub hover: Option<usize>,
    /// The keyboard's marker: its cluster's first member (stable as the map moves).
    focus: Option<u32>,
    /// A press on the map: where it started, where the pointer was last, whether it moved.
    drag: Option<Drag>,
    pub hits: Vec<(Rect, MAct)>,
    /// Thumbnails the hover card wants (for the gallery's wish list).
    pub wanted: Vec<(PathBuf, u32)>,
    places: HashMap<(i32, i32), Option<String>>,
    /// Raster tiles over the base map (off unless the user turns them on).
    pub basemap: super::basemap::Basemap,
    /// The theme was dark at the last frame (vector tiles from a file follow it).
    pub dark: bool,
    // GPU side: the base map's renderer and texture, and what it last rendered.
    gpu: Option<MapRenderer>,
    pub tex: Option<TexId>,
    style: Option<Style>,
    rendered: Option<(Camera, f32, [u32; 4])>,
    scale: f32,
    /// The camera direction of photos (degrees clockwise from north), read for the pins on
    /// screen; `None` inside for a photo without one.
    directions: HashMap<PathBuf, Option<f32>>,
    /// The directions being read.
    direction_rx: Option<crossbeam_channel::Receiver<Directions>>,
    /// A drone photo's height above its take-off point (metres), read with its direction.
    altitudes: HashMap<PathBuf, f32>,
    /// A KML/KMZ export's outcome, to show.
    pub export_rx: Option<crossbeam_channel::Receiver<String>>,
}

/// Camera directions read for photos (degrees clockwise from north, `None` without one;
/// DJI gimbal or flight yaw before EXIF), and drone heights above take-off.
type Directions = Vec<(PathBuf, Option<f32>, Option<f32>)>;

impl MapView {
    /// The GPU device was replaced: the renderer, its texture and the tiles are made again.
    pub fn gpu_reset(&mut self) {
        self.gpu = None;
        self.tex = None;
        self.rendered = None;
        self.basemap.gpu_reset();
    }

    pub fn photos(&self) -> usize {
        self.clusters.len()
    }

    /// The camera's next frame: the map was asked to render (or skipped as unchanged).
    fn render_key(&self) -> Option<(Camera, f32, [u32; 4])> {
        let s = self.style?;
        Some((self.cam, self.scale, s.ocean.map(f32::to_bits)))
    }

    /// The marker that has photo `id` this frame.
    fn marker_of(&self, id: u32) -> Option<usize> {
        self.markers.iter().position(|m| {
            self.parts_of(m)
                .iter()
                .any(|c| self.clusters.members(c).contains(&id))
        })
    }

    fn parts_of(&self, m: &Marker) -> &[Cluster] {
        let (s, n) = (m.parts.0 as usize, m.parts.1 as usize);
        self.parts.get(s..s + n).unwrap_or_default()
    }

    /// Every photo of a marker (the ids of `paths`).
    fn members_of(&self, m: &Marker) -> Vec<u32> {
        self.parts_of(m)
            .iter()
            .flat_map(|c| self.clusters.members(c).iter().copied())
            .collect()
    }

    /// The world box of a marker's photos (x may pass 1 across the antimeridian).
    fn bounds_of(&self, m: &Marker) -> (f64, f64, f64, f64) {
        let mut out: Option<(f64, f64, f64, f64)> = None;
        for c in self.parts_of(m) {
            let mut b = self.clusters.bounds(c);
            if let Some(o) = &mut out {
                // The copy of the box nearest the first one.
                let shift = (b.0 - o.0 + 0.5).floor();
                b.0 -= shift;
                b.2 -= shift;
                *o = (o.0.min(b.0), o.1.min(b.1), o.2.max(b.2), o.3.max(b.3));
            } else {
                out = Some(b);
            }
        }
        out.unwrap_or((m.cluster.x, m.cluster.y, m.cluster.x, m.cluster.y))
    }

    /// Every photo on the map with its latitude and longitude.
    pub(in crate::app) fn photo_positions(&self) -> Vec<(PathBuf, (f64, f64))> {
        self.paths
            .iter()
            .cloned()
            .zip(self.positions.iter().copied())
            .collect()
    }

    /// A KML/KMZ export's outcome, once.
    pub(in crate::app) fn take_export(&mut self) -> Option<String> {
        let m = self.export_rx.as_ref()?.try_recv().ok()?;
        self.export_rx = None;
        Some(m)
    }

    fn first(&self, m: &Marker) -> Option<&Path> {
        let id = *self.clusters.members(&m.cluster).first()?;
        self.paths.get(id as usize).map(PathBuf::as_path)
    }

    /// Takes the directions read, and reads those of the pins on screen still unknown (at
    /// most 200 at a time, off the UI thread). True when new ones came (redraw).
    pub(in crate::app) fn feed_directions(&mut self, loader: &crate::loader::Loader) -> bool {
        let mut came = false;
        if let Some(rx) = &self.direction_rx
            && let Ok(v) = rx.try_recv()
        {
            came = !v.is_empty();
            for (p, d, h) in v {
                if let Some(h) = h {
                    self.altitudes.insert(p.clone(), h);
                }
                self.directions.insert(p, d);
            }
            self.direction_rx = None;
        }
        if self.direction_rx.is_some() || !self.open {
            return came;
        }
        let want: Vec<PathBuf> = self
            .markers
            .iter()
            .filter(|k| k.count == 1)
            .filter_map(|k| self.first(k))
            .filter(|p| !self.directions.contains_key(*p))
            .take(200)
            .map(Path::to_path_buf)
            .collect();
        if want.is_empty() {
            return came;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.direction_rx = Some(rx);
        loader.background("map-directions", move |l| {
            let v = want
                .into_iter()
                .map(|p| {
                    let f = super::drone::read(&p);
                    let d = f.heading.map(|h| h.0 as f32);
                    (p, d, f.relative_altitude.map(|h| h as f32))
                })
                .collect();
            let _ = tx.send(v);
            l.waker()();
        });
        came
    }
}

/// The base map's colours for a theme: muted, so the markers stand out.
pub(super) fn map_style(pal: &Palette) -> Style {
    let (ocean, land, coast, border) = match pal.mode {
        Theme::Dark => (0x16212c, 0x2a2f35, 0x46515c, 0x5c6670),
        _ => (0xc6d9e8, 0xf1eee8, 0x9cb2c4, 0xbdb3a8),
    };
    Style {
        ocean: Color::hex(ocean).to_linear_premul(),
        land: Color::hex(land).to_linear_premul(),
        coast: Color::hex(coast).to_linear_premul(),
        border: Color::hex(border).with_alpha(0.8).to_linear_premul(),
        coast_px: 1.0,
        border_px: 0.8,
    }
}

impl Gallery {
    /// The header's height at the current text scale.
    fn map_header(&self) -> f32 {
        (HEADER * self.text_scale.max(1.0)).round()
    }

    /// Clusters the shown photos with a position again when the tiles or the positions
    /// known changed (at most every [`REBUILD_EVERY`] while they are still coming in).
    pub(super) fn map_refresh(&mut self, force: bool) {
        let known = self.metas.len();
        let key = (
            self.generation,
            self.tiles.len(),
            if self.hide_gps { usize::MAX } else { known },
        );
        let m = &self.map;
        if !force
            && (m.key == key
                || m.built_at
                    .is_some_and(|t| t.elapsed() < REBUILD_EVERY && m.key.1 == key.1))
        {
            return;
        }
        let mut paths = Vec::new();
        let mut positions = Vec::new();
        let mut pts = Vec::new();
        for t in self.tiles.iter().filter(|_| !self.hide_gps) {
            let members: &[usize] = if t.members.is_empty() {
                std::slice::from_ref(&t.entry)
            } else {
                &t.members
            };
            for &e in members {
                let Some(entry) = self.entries.get(e) else {
                    continue;
                };
                if entry.kind != EntryKind::Image {
                    continue;
                }
                let Some((lat, lon)) = self
                    .metas
                    .get(entry.path.as_path())
                    .and_then(|m| m.position)
                else {
                    continue;
                };
                if !lat.is_finite() || !lon.is_finite() || (lat == 0.0 && lon == 0.0) {
                    continue;
                }
                pts.push((paths.len() as u32, project(f64::from(lat), f64::from(lon))));
                paths.push(entry.path.clone());
                positions.push((f64::from(lat), f64::from(lon)));
            }
        }
        let m = &mut self.map;
        m.clusters = Arc::new(Clusters::new(pts));
        m.paths = paths;
        m.positions = positions;
        m.key = key;
        m.built_at = Some(Instant::now());
        m.hover = None;
    }

    /// Fits the camera to every photo (or centres on the "Show on map" photo).
    fn map_fit(&mut self) {
        let m = &mut self.map;
        if let Some(t) = m.target.take()
            && let Some(i) = m.paths.iter().position(|p| *p == t)
        {
            let id = i as u32;
            let c = m.clusters.level(MAX_ZOOM as u8);
            if let Some(k) = c.iter().find(|k| m.clusters.members(k).contains(&id)) {
                m.cam.center = (k.x, k.y);
                m.cam.zoom = FIT_MAX_ZOOM;
                m.cam.clamp();
                m.focus = Some(id);
                return;
            }
        }
        match m.clusters.all_bounds() {
            Some(b) => m.cam.fit(b, FIT_MARGIN, FIT_MAX_ZOOM),
            None => {
                m.cam.zoom = m.cam.min_zoom();
                m.cam.center = (0.5, 0.5);
                m.cam.clamp();
            }
        }
    }

    /// Draws the map view over the gallery; true while it wants another frame soon
    /// (positions still coming in).
    pub(super) fn draw_map(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) -> bool {
        if !self.map.open {
            return false;
        }
        let pal = f.pal;
        let (w, h) = (self.vw, self.vh);
        let ts = self.text_scale.clamp(1.0, 3.0);
        let head = self.map_header();
        let area = Rect::new(0.0, head, w, (h - head).max(1.0));
        self.map_refresh(false);
        let m = &mut self.map;
        m.area = area;
        m.scale = f.scale;
        m.cam.size = (f64::from(area.w), f64::from(area.h));
        // Fitted once the photos are in: the "Show on map" photo as soon as its position is,
        // else every photo once all positions are known (the world until then).
        if !m.fitted {
            let waiting = self.map_waiting();
            let m = &mut self.map;
            let ready = match &m.target {
                Some(t) => !waiting || m.paths.contains(t),
                None => !waiting,
            };
            if ready {
                m.fitted = true;
            }
            if !m.touched && (ready || (m.target.is_none() && !m.clusters.is_empty())) {
                self.map_fit();
            }
        }
        let m = &mut self.map;
        m.cam.clamp();
        let style = map_style(pal);
        m.style = Some(style);
        m.hits.clear();

        // The base map (rendered before the UI pass; the ocean until the first one is in).
        let ocean = match pal.mode {
            Theme::Dark => Color::hex(0x16212c),
            _ => Color::hex(0xc6d9e8),
        };
        dl.rect(area, 0.0, ocean);
        if let Some(tex) = m.tex {
            dl.image(area, [0.0, 0.0, 1.0, 1.0], tex, 0.0);
        }
        // Raster tiles over it, when the user turned a base map on.
        m.dark = pal.mode == Theme::Dark;
        let cam = m.cam;
        m.basemap.draw(dl, &cam, area, f.scale);

        // Markers.
        let base_r = 15.0 * ts;
        let pin_r = 6.0 * ts;
        let loc = I18n::global().localizer();
        let ox = f64::from(area.x);
        let oy = f64::from(area.y);
        let on = m.clusters.on_screen(&m.cam, f64::from(base_r * 2.0));
        let style_count = TextStyle {
            size: COUNT.size * ts,
            ..COUNT
        };
        // Clusters of neighbouring cells can overlap: the smaller join the bigger.
        let radius_of = |n: u32| {
            if n == 1 {
                pin_r
            } else {
                base_r + 3.0 * ts * (n as f32).log10()
            }
        };
        let pos: Vec<(u32, (f64, f64))> = on.iter().map(|(c, p)| (c.count, *p)).collect();
        let parent = declutter(&pos, |n| f64::from(radius_of(n)));
        let mut order: Vec<usize> = (0..on.len()).collect();
        order.sort_unstable_by_key(|&i| (parent[i], i != parent[i], i));
        let mut parts = std::mem::take(&mut m.parts);
        parts.clear();
        let mut markers = Vec::with_capacity(on.len());
        let mut labels = Vec::new();
        let mut s = 0;
        while s < order.len() {
            let head = parent[order[s]];
            let mut e = s;
            let mut count = 0;
            while e < order.len() && parent[order[e]] == head {
                parts.push(on[order[e]].0);
                count += on[order[e]].0.count;
                e += 1;
            }
            let (c, (x, y)) = on[head];
            let at = ((x + ox) as f32, (y + oy) as f32);
            let radius = if count == 1 {
                pin_r
            } else {
                let label = loc.integer(i64::from(count));
                let tw = text.measure(&label, style_count);
                labels.push((markers.len(), label));
                radius_of(count).max(tw * 0.5 + 7.0 * ts)
            };
            markers.push(Marker {
                cluster: c,
                count,
                parts: (s as u32, (e - s) as u32),
                at,
                radius,
            });
            s = e;
        }
        m.parts = parts;
        // Hover: the marker under the pointer, the topmost (last drawn) first.
        let hover = self.pointer.filter(|p| area.contains(*p)).and_then(|p| {
            markers.iter().rposition(|k| {
                let (dx, dy) = (p.0 - k.at.0, p.1 - k.at.1);
                dx * dx + dy * dy <= (k.radius + 3.0) * (k.radius + 3.0)
            })
        });
        let m = &mut self.map;
        m.hover = if m.drag.is_some_and(|d| d.2) {
            None
        } else {
            hover
        };
        m.markers = markers;
        let focus = m.focus.and_then(|id| m.marker_of(id));
        let markers = std::mem::take(&mut m.markers);
        dl.push_clip(area);
        let ring = match pal.mode {
            Theme::Dark => Color::hex(0xf2f2f2),
            _ => Color::hex(0xffffff),
        };
        // Pins under clusters; a pin's wedge points where the camera looked.
        for k in markers.iter().filter(|k| k.count == 1) {
            if let Some(Some(d)) = m.first(k).and_then(|p| m.directions.get(p)) {
                let len = k.radius * 2.6;
                for off in [-22.0f32, 22.0] {
                    let a = (d + off).to_radians();
                    let q = (k.at.0 + len * a.sin(), k.at.1 - len * a.cos());
                    dl.line(k.at, q, 3.0 * ts, pal.shadow);
                    dl.line(k.at, q, 1.6 * ts, pal.accent);
                }
            }
            dl.circle((k.at.0, k.at.1 + 1.0), k.radius + 2.0, pal.shadow);
            dl.circle(k.at, k.radius + 1.5 * ts, ring);
            dl.circle(k.at, k.radius, pal.accent);
        }
        let mut li = labels.into_iter().peekable();
        for (i, k) in markers.iter().enumerate() {
            if k.count == 1 {
                continue;
            }
            dl.shadow(
                Rect::new(
                    k.at.0 - k.radius,
                    k.at.1 - k.radius,
                    k.radius * 2.0,
                    k.radius * 2.0,
                ),
                k.radius,
                pal.shadow,
                6.0,
                2.0,
            );
            dl.circle(k.at, k.radius + 2.0 * ts, ring);
            dl.circle(k.at, k.radius, pal.accent_fill);
            while li.peek().is_some_and(|l| l.0 < i) {
                li.next();
            }
            if let Some((_, label)) = li.next_if(|l| l.0 == i) {
                let r = Rect::new(
                    k.at.0 - k.radius,
                    k.at.1 - k.radius,
                    k.radius * 2.0,
                    k.radius * 2.0,
                );
                text_in(text, dl, &label, style_count, r, pal.accent_text);
            }
        }
        for (i, on) in [(hover, false), (focus, true)] {
            let Some(k) = i.and_then(|i| markers.get(i)) else {
                continue;
            };
            let r = k.radius + if on { 5.0 } else { 3.5 } * ts;
            let ring_r = Rect::new(k.at.0 - r, k.at.1 - r, r * 2.0, r * 2.0);
            dl.stroke_rect(ring_r, r, if on { pal.focus_ring } else { pal.text }, 2.0);
        }
        dl.pop_clip();
        self.map.markers = markers;

        // The hover card: the photo (the cluster's first) with its name or place.
        let card = hover.or(focus);
        self.map.wanted.clear();
        if let Some(k) = card.and_then(|i| self.map.markers.get(i).copied()) {
            self.draw_map_card(dl, text, pal, &k);
        }

        self.draw_map_header(dl, text, pal);
        self.draw_basemap_notes(dl, text, pal);
        // The credit, at the trailing bottom corner.
        let credit = match self.map.basemap.credit() {
            Some(c) => t!("map-credit-tiles", credit = specular_i18n::isolate(&c)),
            None => t!("map-credit"),
        };
        let small = self.ts(SMALL);
        let cw = text.measure(&credit, small) + 12.0;
        let ch = small.size + 8.0;
        let cr = self.flip(Rect::new(w - cw - 6.0, h - ch - 6.0, cw, ch));
        dl.rect(cr, 4.0, pal.surface.with_alpha(0.75));
        text_in(text, dl, &credit, small, cr, pal.text_muted);

        // Nothing to show: say why.
        if self.map.clusters.is_empty() {
            let msg = if self.hide_gps {
                t!("map-hidden")
            } else if self.map_waiting() {
                t!("map-loading")
            } else {
                t!("map-empty")
            };
            let body = self.ts(BODY);
            let mw = (text.measure(&msg, body) + 32.0).min(w - 32.0);
            let mh = body.size * 2.0 + 16.0;
            let r = Rect::new((w - mw) * 0.5, area.y + (area.h - mh) * 0.5, mw, mh);
            dl.shadow(r, 8.0, pal.shadow, 12.0, 3.0);
            dl.rect(r, 8.0, pal.surface_raised);
            let msg = truncate(text, &msg, body, mw - 24.0);
            text_in(text, dl, &msg, body, r, pal.text);
        }
        self.map_waiting()
    }

    /// The online base map's one-time explanation, while it is asked: its title and lines.
    fn basemap_consent_text(&self) -> Option<(String, [String; 4])> {
        let host = self.map.basemap.consent.as_ref()?;
        let size = format!("{} MB", self.map.basemap.cache_mb);
        Some((
            t!("map-basemap-consent-title"),
            [
                t!(
                    "map-basemap-consent-server",
                    host = specular_i18n::isolate(host)
                ),
                t!("map-basemap-consent-sees"),
                t!("map-basemap-consent-private"),
                t!("map-basemap-consent-cache", size = size),
            ],
        ))
    }

    /// The online base map's one-time explanation (a card with two buttons), or why the base
    /// map is not shown.
    fn draw_basemap_notes(&mut self, dl: &mut DrawList, text: &mut Text, pal: &Palette) {
        let area = self.map.area;
        let (title, body) = (self.ts(TITLE), self.ts(BODY));
        let ts = self.text_scale.clamp(1.0, 3.0);
        if let Some((head, lines)) = self.basemap_consent_text() {
            let allow = t!("map-basemap-allow");
            let decline = t!("map-basemap-decline");
            let pad = 20.0 * ts.min(1.5);
            let lh = (body.size * 1.5).round();
            let mut widest = text.measure(&head, title);
            for l in &lines {
                widest = widest.max(text.measure(l, body));
            }
            let cw = (widest + pad * 2.0).min(area.w - 32.0).max(200.0);
            let bh = (30.0 * ts).round();
            let ch = pad * 2.0 + title.size * 1.6 + lh * lines.len() as f32 + 16.0 + bh;
            let r = Rect::new(
                area.x + (area.w - cw) * 0.5,
                area.y + ((area.h - ch) * 0.5).max(8.0),
                cw,
                ch,
            );
            dl.shadow(r, 10.0, pal.shadow, 16.0, 4.0);
            dl.rect(r, 10.0, pal.surface_raised);
            let mut y = r.y + pad;
            let row = |y: f32, h: f32| self.flip(Rect::new(r.x + pad, y, cw - pad * 2.0, h));
            text_at(
                text,
                dl,
                &head,
                title,
                row(y, title.size * 1.4),
                pal.text,
                self.rtl,
            );
            y += title.size * 1.6;
            for l in &lines {
                let l = truncate(text, l, body, cw - pad * 2.0);
                text_at(text, dl, &l, body, row(y, lh), pal.text_muted, self.rtl);
                y += lh;
            }
            y += 16.0;
            // Buttons at the trailing edge: the agreement last.
            let aw = text.measure(&allow, body) + 28.0;
            let dw = text.measure(&decline, body) + 28.0;
            let ar = Rect::new(r.right() - pad - aw, y, aw, bh);
            let dr = Rect::new(ar.x - 8.0 - dw, y, dw, bh);
            for (br, label, act, accent) in [
                (dr, &decline, MAct::BasemapDecline, false),
                (ar, &allow, MAct::BasemapAllow, true),
            ] {
                let br = if self.rtl {
                    Rect::new(r.x + (r.right() - br.right()), br.y, br.w, br.h)
                } else {
                    br
                };
                let hover = self.pointer.is_some_and(|p| br.contains(p));
                let (fill, fg) = match (accent, hover) {
                    (true, _) => (pal.accent_fill, pal.accent_text),
                    (false, true) => (pal.surface_hover, pal.text),
                    (false, false) => (pal.surface, pal.text),
                };
                dl.rect(br, 6.0, fill);
                text_in(text, dl, label, body, br, fg);
                self.map.hits.push((br, act));
            }
        } else if let Some(e) = self.map.basemap.error.clone() {
            let msg = t!("map-basemap-error", reason = e);
            let small = self.ts(SMALL);
            let mw = (text.measure(&msg, small) + 16.0).min(area.w - 32.0);
            let mh = small.size + 10.0;
            let r = self.flip(Rect::new(area.x + 8.0, area.bottom() - mh - 8.0, mw, mh));
            dl.rect(r, 4.0, pal.surface.with_alpha(0.85));
            let msg = truncate(text, &msg, small, mw - 12.0);
            text_in(text, dl, &msg, small, r, pal.text);
        }
    }

    /// Positions still to come: the metadata of some tiles is not in yet.
    fn map_waiting(&self) -> bool {
        !self.tiles.is_empty()
            && !self.library.on
            && self.meta_complete.is_none_or(|c| c.0 != self.generation)
    }

    fn draw_map_header(&mut self, dl: &mut DrawList, text: &mut Text, pal: &Palette) {
        let w = self.vw;
        let hh = self.map_header();
        let ts = self.text_scale.clamp(1.0, 3.0);
        dl.rect(Rect::new(0.0, 0.0, w, hh), 0.0, pal.surface);
        dl.rect(Rect::new(0.0, hh - 1.0, w, 1.0), 0.0, pal.separator);
        let (title, body) = (self.ts(TITLE), self.ts(BODY));
        // Buttons from the trailing edge: Done, Show all, +, −.
        let bh = (28.0 * ts).round();
        let y = ((hh - bh) * 0.5).round();
        let mut x = w - 12.0;
        let mut place = |wd: f32| {
            x -= wd;
            let r = Rect::new(x, y, wd, bh);
            x -= 6.0;
            r
        };
        let done = t!("map-done");
        let fit = t!("map-show-all");
        let done_r = place(text.measure(&done, body) + 24.0);
        let fit_r = place(text.measure(&fit, body) + 20.0);
        let plus_r = place(bh);
        let minus_r = place(bh);
        let trailing = x;
        let cam = self.map.cam;
        let at_max = cam.zoom >= MAX_ZOOM - 1e-6;
        let at_min = cam.zoom <= cam.min_zoom() + 1e-6;
        for (r, act, label, icon, accent, off) in [
            (
                minus_r,
                MAct::ZoomOut,
                None,
                Some(Icon::Minus),
                false,
                at_min,
            ),
            (plus_r, MAct::ZoomIn, None, Some(Icon::Plus), false, at_max),
            (fit_r, MAct::Fit, Some(fit.as_str()), None, false, false),
            (done_r, MAct::Close, Some(done.as_str()), None, true, false),
        ] {
            let r = self.flip(r);
            let hover = !off && self.pointer.is_some_and(|p| r.contains(p));
            let (fill, fg) = match (accent, hover) {
                (true, _) => (pal.accent_fill, pal.accent_text),
                (false, true) => (pal.surface_hover, pal.text),
                (false, false) => (pal.surface_raised, pal.text),
            };
            dl.rect(r, 6.0, fill);
            let fg = if off { pal.text_faint } else { fg };
            match (label, icon) {
                (Some(l), _) => {
                    text_in(text, dl, l, body, r, fg);
                }
                (None, Some(i)) => {
                    let s = (16.0 * ts).round();
                    specular_gfx::draw_icon(dl, i, r.centered(s, s), fg, 1.6 * ts, false);
                }
                _ => {}
            }
            self.map.hits.push((r, act));
        }
        // Title and summary from the leading edge.
        let lead = 16.0;
        let tw = text.measure(&t!("map-title"), title);
        text_at(
            text,
            dl,
            &t!("map-title"),
            title,
            self.flip(Rect::new(lead, 0.0, tw, hh)),
            pal.text,
            self.rtl,
        );
        let n = self.map.photos();
        let summary = t!("map-summary", count = n as i64);
        let sx = lead + tw + 12.0;
        let room = (trailing - sx - 8.0).max(0.0);
        let summary = truncate(text, &summary, body, room);
        text_at(
            text,
            dl,
            &summary,
            body,
            self.flip(Rect::new(sx, 0.0, room, hh)),
            pal.text_muted,
            self.rtl,
        );
    }

    /// The card over a hovered (or focused) marker: the thumbnail of its first photo, and
    /// the photo's name (one) or the place and count (several).
    fn draw_map_card(&mut self, dl: &mut DrawList, text: &mut Text, pal: &Palette, k: &Marker) {
        let Some(path) = self.map.first(k).map(Path::to_path_buf) else {
            return;
        };
        let ts = self.text_scale.clamp(1.0, 3.0);
        let side = 132.0 * ts.min(1.6);
        let class = specular_thumbs::class_for(side * self.scale);
        self.map.wanted.push((path.clone(), class));
        let label = if k.count == 1 {
            let name = path
                .file_name()
                .map(|n| specular_i18n::isolate(&n.to_string_lossy()))
                .unwrap_or_default();
            // A drone photo: its height above the take-off point.
            match self.map.altitudes.get(&path) {
                Some(h) => specular_i18n::dot_join(&[
                    name,
                    t!(
                        "map-height-above-takeoff",
                        metres = specular_i18n::fixed(*h, 0)
                    ),
                ]),
                None => name,
            }
        } else {
            let n = t!("map-photos", count = i64::from(k.count));
            match self.map_place(&k.cluster) {
                Some(p) => specular_i18n::dot_join(&[p, n]),
                None => n,
            }
        };
        let small = self.ts(SMALL);
        let pad = 6.0;
        let cap = small.size + 8.0;
        let (iw, ih) = self
            .metas
            .get(path.as_path())
            .map(|m| (m.width.max(1) as f32, m.height.max(1) as f32))
            .unwrap_or((4.0, 3.0));
        let s = (side / iw).min(side / ih);
        let (tw, th) = ((iw * s).round().max(32.0), (ih * s).round().max(32.0));
        let cw = (tw + pad * 2.0).max(96.0);
        let chh = th + pad * 2.0 + cap;
        let area = self.map.area;
        let mut x = k.at.0 - cw * 0.5;
        let mut y = k.at.1 - k.radius - 8.0 - chh;
        if y < area.y + 4.0 {
            y = k.at.1 + k.radius + 8.0;
        }
        x = x.clamp(area.x + 4.0, (area.right() - cw - 4.0).max(area.x + 4.0));
        y = y.min(area.bottom() - chh - 4.0).max(area.y + 4.0);
        let r = Rect::new(x, y, cw, chh);
        dl.shadow(r, 8.0, pal.shadow, 14.0, 4.0);
        dl.rect(r, 8.0, pal.surface_raised);
        let img = Rect::new(x + (cw - tw) * 0.5, y + pad, tw, th);
        match self.atlas.as_mut().and_then(|a| a.best(&path, class)) {
            Some(pl) => dl.image_over(img, pl.uv, pl.tex, 4.0, self.backdrop),
            None => {
                let c = self
                    .averages
                    .get(path.as_path())
                    .map_or(pal.surface, |a| super::draw::avg_color(*a));
                dl.rect(img, 4.0, c);
            }
        }
        let lr = Rect::new(x + pad, y + pad + th, cw - pad * 2.0, cap);
        let label = truncate(text, &label, small, lr.w);
        text_in(text, dl, &label, small, lr, pal.text);
    }

    /// The nearest town to a cluster (when the place index is loaded; never waits for it).
    fn map_place(&mut self, c: &Cluster) -> Option<String> {
        let (lat, lon) = unproject(c.x, c.y);
        // About 1 km cells: one lookup per place, however the clusters regroup.
        let key = ((lat * 100.0) as i32, (lon * 100.0) as i32);
        if let Some(p) = self.map.places.get(&key) {
            return p.clone();
        }
        if !specular_metadata::geocoder_ready() {
            return None;
        }
        let p = specular_library::place_of(lat as f32, lon as f32).map(|p| p.city);
        self.map.places.insert(key, p.clone());
        p
    }

    /// Screen-reader nodes of the map; false when it is not open (the grid's are made).
    pub(super) fn map_a11y(&self, tree: &mut accesskit::TreeUpdate, scale: f64) -> bool {
        use accesskit::{Action, Live, Node, NodeId, Role};
        let m = &self.map;
        if !m.open {
            return false;
        }
        let bounds = |r: Rect| accesskit::Rect {
            x0: f64::from(r.x) * scale,
            y0: f64::from(r.y) * scale,
            x1: f64::from(r.x + r.w) * scale,
            y1: f64::from(r.y + r.h) * scale,
        };
        let group_id = NodeId(A11Y_MAP);
        let mut group = Node::new(Role::Group);
        group.set_label(t!("map-a11y-group"));
        group.set_bounds(bounds(Rect::new(0.0, 0.0, self.vw, self.vh)));
        let mut children = Vec::new();
        let mut nodes = Vec::new();
        // The base map's question: its text and its two buttons, a group of their own over
        // the map (read where it is seen, not among the map's buttons).
        let consent = self.basemap_consent_text().map(|(head, lines)| {
            let mut g = Node::new(Role::Group);
            g.set_label(head);
            g.set_description(lines.join(" "));
            g
        });
        let mut consent_box: Option<Rect> = None;
        let mut consent_children = Vec::new();
        for (i, a) in CONTROLS.iter().enumerate() {
            let Some(r) = m.hits.iter().find(|h| h.1 == *a).map(|h| h.0) else {
                continue;
            };
            let asked = matches!(a, MAct::BasemapDecline | MAct::BasemapAllow);
            let id = NodeId(A11Y_MAP + A11Y_CONTROL + i as u64);
            let mut n = Node::new(Role::Button);
            n.set_bounds(bounds(r));
            n.set_label(match a {
                MAct::ZoomOut => t!("map-zoom-out"),
                MAct::ZoomIn => t!("map-zoom-in"),
                MAct::Fit => t!("map-show-all"),
                MAct::Close => t!("map-done"),
                MAct::BasemapDecline => t!("map-basemap-decline"),
                MAct::BasemapAllow => t!("map-basemap-allow"),
            });
            n.add_action(Action::Click);
            if asked && consent.is_some() {
                consent_box = Some(consent_box.map_or(r, |b| {
                    let (x0, y0) = (b.x.min(r.x), b.y.min(r.y));
                    let (x1, y1) = (b.right().max(r.right()), (b.y + b.h).max(r.y + r.h));
                    Rect::new(x0, y0, x1 - x0, y1 - y0)
                }));
                consent_children.push(id);
            } else {
                children.push(id);
            }
            nodes.push((id, n));
        }
        if let Some(mut g) = consent {
            let id = NodeId(A11Y_MAP + A11Y_CONSENT);
            if let Some(b) = consent_box {
                g.set_bounds(bounds(b));
            }
            g.set_children(consent_children);
            children.push(id);
            nodes.push((id, g));
        }
        // What the view shows, read as it changes.
        let status_id = NodeId(A11Y_MAP + A11Y_STATUS);
        let mut status = Node::new(Role::Status);
        status.set_live(Live::Polite);
        status.set_label(if m.clusters.is_empty() {
            if self.hide_gps {
                t!("map-hidden")
            } else if self.map_waiting() {
                t!("map-loading")
            } else {
                t!("map-empty")
            }
        } else {
            t!(
                "map-a11y-status",
                count = m.markers.len() as i64,
                zoom = m.cam.zoom.round() as i64
            )
        });
        children.push(status_id);
        nodes.push((status_id, status));
        // Markers, nearest the centre first.
        let c = (m.area.x + m.area.w * 0.5, m.area.y + m.area.h * 0.5);
        let mut order: Vec<usize> = (0..m.markers.len())
            .filter(|&i| m.area.contains(m.markers[i].at))
            .collect();
        order.sort_by(|&a, &b| {
            let d = |i: usize| {
                let p = m.markers[i].at;
                (p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)
            };
            d(a).total_cmp(&d(b))
        });
        order.truncate(A11Y_MARKERS);
        // Those nearest the centre, read as they are seen: in lines from the top (markers
        // whose middles are within the line's smallest half-height), each in the reading
        // direction.
        order.sort_by(|&a, &b| m.markers[a].at.1.total_cmp(&m.markers[b].at.1));
        let mut lines: Vec<Vec<usize>> = Vec::new();
        for i in order {
            let k = &m.markers[i];
            match lines.last_mut() {
                Some(line)
                    if line.iter().all(|&j| {
                        let o = &m.markers[j];
                        k.at.1 - o.at.1 <= k.radius.min(o.radius)
                    }) =>
                {
                    line.push(i);
                }
                _ => lines.push(vec![i]),
            }
        }
        let order: Vec<usize> = lines
            .into_iter()
            .flat_map(|mut line| {
                line.sort_by(|&a, &b| {
                    let (xa, xb) = (m.markers[a].at.0, m.markers[b].at.0);
                    if self.rtl {
                        xb.total_cmp(&xa)
                    } else {
                        xa.total_cmp(&xb)
                    }
                });
                line
            })
            .collect();
        let focus = m.focus.and_then(|id| m.marker_of(id));
        for i in order
            .iter()
            .copied()
            .chain(focus.filter(|f| !order.contains(f)))
        {
            let k = &m.markers[i];
            let id = NodeId(A11Y_MAP + A11Y_MARKER + i as u64);
            let mut n = Node::new(Role::Button);
            let r = k.radius;
            n.set_bounds(bounds(Rect::new(k.at.0 - r, k.at.1 - r, r * 2.0, r * 2.0)));
            n.set_label(if k.count == 1 {
                m.first(k)
                    .and_then(Path::file_name)
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            } else {
                let n = t!("map-photos", count = i64::from(k.count));
                let (lat, lon) = unproject(k.cluster.x, k.cluster.y);
                let key = ((lat * 100.0) as i32, (lon * 100.0) as i32);
                match m.places.get(&key).cloned().flatten() {
                    Some(p) => format!("{p}, {n}"),
                    None => n,
                }
            });
            n.add_action(Action::Click);
            n.add_action(Action::Focus);
            children.push(id);
            nodes.push((id, n));
        }
        group.set_children(children);
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
            root.push_child(group_id);
        }
        tree.nodes.push((group_id, group));
        tree.nodes.extend(nodes);
        if tree.focus == NodeId(1) {
            tree.focus = focus.map_or(group_id, |i| NodeId(A11Y_MAP + A11Y_MARKER + i as u64));
        }
        true
    }
}

// ----- The base map on the GPU ------------------------------------------------------------

impl Gallery {
    /// Before drawing: the base map's renderer and texture for the map's area in a window of
    /// `size` logical px, or lets go of them when the map is closed.
    pub(in crate::app) fn prepare_map(
        &mut self,
        gpu: &Gpu,
        ui: &mut UiRenderer,
        scale: f32,
        size: (f32, f32),
    ) {
        let head = self.map_header();
        let m = &mut self.map;
        if m.open && self.active {
            m.area = Rect::new(0.0, head, size.0, (size.1 - head).max(0.0));
        }
        if !m.open || !self.active {
            m.basemap.release(ui);
            if let Some(t) = m.tex.take() {
                ui.unregister_texture(t);
            }
            m.gpu = None;
            m.rendered = None;
            return;
        }
        if m.area.w < 1.0 || m.area.h < 1.0 {
            return;
        }
        m.basemap.upload(gpu, ui);
        let r = m.gpu.get_or_insert_with(|| MapRenderer::new(gpu));
        let px = (
            (m.area.w * scale).round().max(1.0) as u32,
            (m.area.h * scale).round().max(1.0) as u32,
        );
        if r.ensure_target(gpu, px)
            && let Some(view) = r.view()
        {
            match m.tex {
                Some(t) => ui.update_texture(gpu, t, view),
                None => m.tex = Some(ui.register_texture(gpu, view)),
            }
            m.rendered = None;
        }
    }

    /// Renders the base map for this frame's camera (after [`Gallery::draw`], before the UI
    /// pass), when it changed.
    pub(in crate::app) fn render_map(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder) {
        let m = &mut self.map;
        if !m.open {
            return;
        }
        let (Some(r), Some(style), Some(key)) = (&m.gpu, m.style, m.render_key()) else {
            return;
        };
        if m.rendered == Some(key) {
            return;
        }
        // The camera in the texture's own size (the area may have changed since `prepare`).
        let mut cam = m.cam;
        if let Some((w, h)) = r.size() {
            let s = f64::from(m.scale.max(0.1));
            cam.size = (f64::from(w) / s, f64::from(h) / s);
        }
        r.render(gpu, enc, &cam, f64::from(m.scale), &style);
        m.rendered = Some(key);
    }
}

// ----- Input ------------------------------------------------------------------------------

impl App {
    /// Opens the map view (on the "Show on map" photo, if given).
    /// A base map tile came in.
    pub(in crate::app) fn on_basemap_tile(
        &mut self,
        generation: u64,
        key: (u8, u32, u32),
        tile: Option<(u32, u32, Arc<[u8]>)>,
    ) {
        self.gallery.map.basemap.tile_ready(generation, key, tile);
        if self.gallery.map.open {
            self.redraw = true;
        }
    }

    pub(in crate::app) fn map_open(&mut self, target: Option<PathBuf>) {
        if !self.gallery.active {
            self.gallery_enter();
            self.gallery.transition = None;
        }
        let ga = &mut self.gallery;
        ga.quick = None;
        ga.compare = None;
        let m = &mut ga.map;
        m.open = true;
        m.fitted = false;
        m.touched = false;
        m.target = target;
        m.drag = None;
        m.focus = None;
        ga.map_refresh(true);
        // Every tile's metadata is wanted for its position.
        ga.wish_key = 0;
        self.redraw = true;
    }

    pub(super) fn map_close(&mut self) {
        let m = &mut self.gallery.map;
        m.open = false;
        m.drag = None;
        m.hover = None;
        m.wanted.clear();
        self.gallery.wish_key = 0;
        self.redraw = true;
    }

    /// The map's palette entries (with the map open).
    pub(super) fn map_menu_items(&self) -> Vec<PaletteItem> {
        if !self.gallery.map.open {
            return Vec::new();
        }
        vec![
            item(code::ZOOM_IN, t!("map-zoom-in"), Some("+"), Icon::ZoomIn),
            item(code::ZOOM_OUT, t!("map-zoom-out"), Some("-"), Icon::ZoomOut),
            item(code::FIT, t!("map-show-all"), Some("0"), Icon::Fit),
            item(code::EXPORT_KML, t!("map-export-kml"), None, Icon::Download),
            item(
                code::EXPORT_KMZ_FULL,
                t!("map-export-kmz-full"),
                None,
                Icon::Download,
            ),
            item(code::CLOSE, t!("map-done"), Some("Esc"), Icon::Close),
        ]
    }

    pub(super) fn map_code(&mut self, c: u64) {
        match c {
            code::OPEN => {
                if self.gallery.map.open {
                    self.map_close();
                } else {
                    self.map_open(None);
                }
            }
            code::ZOOM_IN => self.map_act(MAct::ZoomIn),
            code::ZOOM_OUT => self.map_act(MAct::ZoomOut),
            code::FIT => self.map_act(MAct::Fit),
            code::EXPORT_KML => self.map_export_kml(false),
            code::EXPORT_KMZ_FULL => self.map_export_kml(true),
            code::CLOSE => self.map_close(),
            _ => {}
        }
        self.redraw = true;
    }

    fn map_act(&mut self, a: MAct) {
        let ga = &mut self.gallery;
        let m = &mut ga.map;
        m.touched = true;
        let mid = (m.cam.size.0 / 2.0, m.cam.size.1 / 2.0);
        match a {
            MAct::ZoomIn => m.cam.zoom_about(KEY_ZOOM, mid),
            MAct::ZoomOut => m.cam.zoom_about(-KEY_ZOOM, mid),
            MAct::Fit => {
                m.target = None;
                ga.map_fit();
            }
            MAct::Close => self.map_close(),
            MAct::BasemapAllow => {
                if let Some(host) = m.basemap.consent.clone() {
                    self.store.update(specular_settings::Origin::Program, |s| {
                        let a = &mut s.metadata.basemap_allowed;
                        if !a.split(',').any(|h| h.trim() == host) {
                            if !a.is_empty() {
                                a.push(',');
                            }
                            a.push_str(&host);
                        }
                    });
                    self.apply_settings(false);
                    self.gallery.map.basemap.allow(&self.loader);
                }
            }
            MAct::BasemapDecline => {
                self.store.update(specular_settings::Origin::Program, |s| {
                    s.metadata.basemap = specular_settings::Basemap::Off;
                });
                self.apply_settings(false);
            }
        }
        self.redraw = true;
    }

    /// A click (or Enter) on a marker: zoom into a cluster, or open a photo. A cluster that
    /// cannot split further (the deepest zoom, or photos taken at one spot) is selected in
    /// the grid instead, and the map closes on it.
    fn map_activate(&mut self, i: usize) {
        let ga = &mut self.gallery;
        let m = &mut ga.map;
        let Some(k) = m.markers.get(i).copied() else {
            return;
        };
        let ids = m.members_of(&k);
        let members: Vec<PathBuf> = ids
            .iter()
            .filter_map(|&id| m.paths.get(id as usize).cloned())
            .collect();
        m.focus = ids.first().copied();
        m.touched = true;
        if k.count == 1 {
            let Some(p) = members.first() else { return };
            if let Some(t) = tile_in(&ga.by_path, &ga.entry_tile, p) {
                ga.focus = Some(p.clone());
                self.gallery_open(t);
            }
            return;
        }
        let b = m.bounds_of(&k);
        let before = m.cam.level();
        let mut cam = m.cam;
        cam.fit(b, FIT_MARGIN.max(f64::from(k.radius) * 2.0), MAX_ZOOM);
        if cam.level() > before {
            m.cam = cam;
        } else {
            ga.selected = members.iter().cloned().collect();
            ga.focus = members.first().cloned();
            if let Some(t) = members
                .first()
                .and_then(|p| tile_in(&ga.by_path, &ga.entry_tile, p))
            {
                ga.set_focus(t);
            }
            self.map_close();
        }
        self.redraw = true;
    }

    /// A key with the map open; true when used.
    pub(super) fn map_key(&mut self, combo: Option<&KeyCombo>) -> bool {
        let Some(combo) = combo else { return true };
        let mods = combo.mods;
        if mods.ctrl || mods.alt || mods.meta {
            return false;
        }
        let m = &mut self.gallery.map;
        m.touched = true;
        let (w, h) = m.cam.size;
        match combo.key {
            Key::Named(NamedKey::Escape) => self.map_close(),
            Key::Named(NamedKey::ArrowLeft) => m.cam.pan((w * KEY_PAN, 0.0)),
            Key::Named(NamedKey::ArrowRight) => m.cam.pan((-w * KEY_PAN, 0.0)),
            Key::Named(NamedKey::ArrowUp) => m.cam.pan((0.0, h * KEY_PAN)),
            Key::Named(NamedKey::ArrowDown) => m.cam.pan((0.0, -h * KEY_PAN)),
            Key::Char('+' | '=') => self.map_act(MAct::ZoomIn),
            Key::Char('-' | '_') => self.map_act(MAct::ZoomOut),
            Key::Char('0') | Key::Named(NamedKey::Home) => self.map_act(MAct::Fit),
            Key::Named(NamedKey::Tab) => {
                let n = m.markers.len();
                if n == 0 {
                    return true;
                }
                // Reading order of the screen: top to bottom, then left to right.
                let mut order: Vec<usize> = (0..n).collect();
                order.sort_by(|&a, &b| {
                    let (pa, pb) = (m.markers[a].at, m.markers[b].at);
                    // total_cmp: a NaN position (bad GPS) must not break the sort.
                    pa.1.round()
                        .total_cmp(&pb.1.round())
                        .then(pa.0.total_cmp(&pb.0))
                        .then(a.cmp(&b))
                });
                let cur = m
                    .focus
                    .and_then(|id| m.marker_of(id))
                    .and_then(|i| order.iter().position(|&o| o == i));
                let next = match (cur, mods.shift) {
                    (None, false) => 0,
                    (None, true) => n - 1,
                    (Some(c), false) => (c + 1) % n,
                    (Some(c), true) => (c + n - 1) % n,
                };
                let i = order[next];
                m.focus = m.members_of(&m.markers[i]).first().copied();
            }
            Key::Named(NamedKey::Enter | NamedKey::Space) => {
                if let Some(i) = m.focus.and_then(|id| m.marker_of(id)) {
                    self.map_activate(i);
                }
            }
            _ => return false,
        }
        self.redraw = true;
        true
    }

    /// The pointer moved with the map open: drags pan it.
    pub(super) fn map_pointer_move(&mut self, p: (f32, f32)) {
        let m = &mut self.gallery.map;
        if let Some((start, last, moved)) = m.drag {
            let moved = moved || (p.0 - start.0).abs() + (p.1 - start.1).abs() > CLICK_SLOP;
            if moved {
                m.touched = true;
                m.cam
                    .pan((f64::from(p.0 - last.0), f64::from(p.1 - last.1)));
            }
            m.drag = Some((start, if moved { p } else { last }, moved));
        }
        self.redraw = true;
    }

    /// A mouse button with the map open.
    pub(super) fn map_button(&mut self, down: bool, b: specular_widgets::Button, p: (f32, f32)) {
        use specular_widgets::Button;
        let m = &mut self.gallery.map;
        if b != Button::Primary {
            return;
        }
        if down {
            if let Some(a) = m.hits.iter().find(|h| h.0.contains(p)).map(|h| h.1) {
                self.map_act(a);
                return;
            }
            if m.area.contains(p) {
                m.drag = Some((p, p, false));
            }
            return;
        }
        let Some((_, _, moved)) = m.drag.take() else {
            return;
        };
        if !moved
            && let Some(i) = m.markers.iter().rposition(|k| {
                let (dx, dy) = (p.0 - k.at.0, p.1 - k.at.1);
                dx * dx + dy * dy <= (k.radius + 3.0) * (k.radius + 3.0)
            })
        {
            self.map_activate(i);
        }
        self.redraw = true;
    }

    /// The wheel zooms about the pointer (a horizontal wheel pans).
    pub(super) fn map_wheel(&mut self, lines: (f32, f32), pixels: bool, p: (f32, f32)) {
        let m = &mut self.gallery.map;
        let (dx, dy) = if pixels {
            (lines.0 / 40.0, lines.1 / 40.0)
        } else {
            lines
        };
        m.touched = true;
        if dy.abs() > 1e-3 {
            let at = (f64::from(p.0 - m.area.x), f64::from(p.1 - m.area.y));
            m.cam.zoom_about(f64::from(dy) * WHEEL_STEP, at);
        }
        if dx.abs() > 1e-3 {
            m.cam.pan((f64::from(dx) * 40.0, 0.0));
        }
        self.redraw = true;
    }

    /// Screen-reader actions on the map's nodes; false when not ours.
    pub(in crate::app) fn map_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        use accesskit::Action;
        let target = req.target_node.0;
        if !self.gallery.active
            || !self.gallery.map.open
            || !(A11Y_MAP..A11Y_MAP + A11Y_MARKER + 1_000_000).contains(&target)
        {
            return false;
        }
        let i = target - A11Y_MAP;
        if i >= A11Y_MARKER {
            let k = (i - A11Y_MARKER) as usize;
            let m = &mut self.gallery.map;
            match req.action {
                Action::Click => self.map_activate(k),
                Action::Focus => {
                    if let Some(mk) = m.markers.get(k) {
                        m.focus = m.members_of(mk).first().copied();
                    }
                }
                _ => {}
            }
        } else if let Some(a) = i
            .checked_sub(A11Y_CONTROL)
            .and_then(|i| CONTROLS.get(i as usize).copied())
            && req.action == Action::Click
        {
            self.map_act(a);
        }
        self.redraw = true;
        true
    }

    /// `--show gallery-map…` for screenshots: the map of the folder's photos.
    pub(super) fn map_show(&mut self, rest: &str) -> bool {
        if rest != "-map" {
            return false;
        }
        self.map_open(None);
        true
    }
}
