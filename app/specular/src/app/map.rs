//! The map view (the Maps profile): georeferenced images and vector maps with their layers,
//! coordinates in the chosen format, geodesic measuring, elevation profiles, identify and
//! the attribute table, a scale bar, north arrow and overview map.
//!
//! The view is on while the Maps tools are on (Settings › Profile, or the toolbar chip for
//! the session) and the image has a georeference ([`seam`]). Its keymap layer
//! ([`Mode::Map`], `[maps]` in `keymap.toml`) sits over the image's drawing or science layer.
//! A vector map's features are drawn by the map view itself ([`layers`]): every layer of the
//! file, and layers added from other files, in one GPU scene in stored pixels, with the
//! layer panel's visibility, opacity and styles applied as it draws. Panels take the keys
//! while one has focus (Tab walks them, Esc closes the focused one); every control has an
//! AccessKit node ([`view`]).
//!
//! The tools of [`tools`]: slope, aspect and contours of elevation models and the terrain
//! in 3-D ([`terrain`]); the time slider ([`timebar`]), weather colours and wind arrows
//! ([`weather`]); online map services ([`services`]); photos and geotagging along GPS
//! tracks ([`photos`]); and the map's export and printing to scale ([`export`]).
//!
//! [`Mode::Map`]: specular_input::Mode::Map

pub(super) mod export;
mod furniture;
mod layers;
mod measure;
pub(super) mod photos;
pub(crate) mod seam;
pub(super) mod services;
mod table;
mod terrain;
mod timebar;
mod tools;
mod view;
pub(super) mod weather;

pub(super) use tools::GEOTAG_ITEM;
pub(super) use weather::table_of as weather_table;

#[cfg(test)]
mod shots;

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use crossbeam_channel::Receiver;
use specular_decode::drawing::specular_vector::fonts::Fonts;
use specular_decode::drawing::specular_vector::prep::{PrepLimits, Prepared, prepare};
use specular_decode::geo::GeoRef;
use specular_decode::geo::specular_gis::model::Dataset;
use specular_decode::geo::specular_gis::style::{FeatureStyle, kml_styles, kmz_styles};
use specular_decode::science::NdPlane;
use specular_geodesy::coords::{self, CoordFormat};
use specular_geodesy::geodesic;
use specular_geodesy::profile::Profile;
use specular_geodesy::seam::{GeoTransform, HeightSource};
use specular_geodesy::units::Units;
use specular_gfx::winit::event_loop::ActiveEventLoop;
use specular_i18n::t;
use specular_input::{Action, Capability, Key, KeyCombo, MapState, NamedKey};
use specular_log::{UI, debug};
use specular_settings::{CoordinateFormat, Origin, Units as SettingUnits};

use super::App;
use crate::loader::{Copied, ImageId};
pub(super) use view::{MapFrame, MapView, a11y, draw, render};

/// Pointer travel (logical px) between press and release still counted as a click.
const CLICK_SLOP: f32 = 4.0;
/// Screen px around the pointer a feature is found within.
const HIT_PX: f64 = 5.0;
/// Attributes the identify panel lists before "and N more".
const IDENTIFY_ROWS: usize = 12;
/// The keyboard crosshair's step, logical px (Shift: ten times).
const CROSS_STEP: f64 = 4.0;
/// Seconds the Go to pin stays.
const PIN_SECONDS: f64 = 6.0;

/// A georeference, the session's own, heights and named bands ([`App::gis_sources`]).
pub(super) type Sources = (
    Arc<dyn GeoTransform>,
    Option<Arc<GeoRef>>,
    Option<Arc<dyn HeightSource>>,
    Vec<(String, Arc<NdPlane>)>,
);

/// The panel that has the keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Focus {
    Layers,
    Table,
    GoTo,
    Tool,
    Chart,
    Time,
    Services,
    Geotag,
    Export,
}

/// What a click identified.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Identify {
    pub title: String,
    pub rows: Vec<(String, String)>,
    pub more: usize,
}

/// Something read in the background for the map.
enum Job {
    /// The image's own layers and their scene.
    Base(ImageId, Vec<layers::MapLayer>, Arc<Prepared>),
    /// The scene for the layers at revision `u64`.
    Scene(ImageId, u64, Arc<Prepared>),
    /// A file added as layers, or why it can't be.
    Added(ImageId, PathBuf, Result<Vec<layers::MapLayer>, String>),
    /// Contour lines of the heights shown.
    Contours(ImageId, terrain::Contours),
    /// Where the grid's wind components are, if it has them.
    WindSource(ImageId, Option<weather::WindSource>),
    /// The wind components at a position.
    Wind(ImageId, Vec<u32>, Arc<NdPlane>, Arc<NdPlane>),
    /// A GPS track's fixes in time (the file shown, or an added layer).
    Track(ImageId, Option<Arc<specular_geodesy::track::Timeline>>),
    /// The photos beside the track.
    Photos(ImageId, photos::Scanned),
    /// Geotagging is done: sidecars written, and the failures.
    Geotagged(ImageId, usize, Vec<(PathBuf, String)>),
    /// An online service answered.
    Service(services::Reply),
    /// The map was exported (or printed): the file, or why not.
    Exported(export::Done),
}

/// The map view's state for the current image.
#[derive(Default)]
pub(super) struct Map {
    id: Option<ImageId>,
    checked: bool,
    /// The image's georeference ([`seam`]).
    geo: Option<Arc<dyn GeoTransform>>,
    georef: Option<Arc<GeoRef>>,
    /// The image's own features are drawn by the map view.
    vector: bool,
    /// The stored image's size, px.
    size: (f64, f64),
    heights: Option<Arc<dyn HeightSource>>,
    /// A science image's planes with their names (identify lists each).
    bands: Vec<(String, Arc<NdPlane>)>,
    layers: Vec<layers::MapLayer>,
    /// The layer the panel works on.
    chosen: Option<usize>,
    panel: bool,
    panel_scroll: usize,
    control: Option<layers::Control>,
    confirm_remove: bool,
    table: Option<table::Table>,
    overview: bool,
    tool: Option<measure::Tool>,
    profile: Option<Profile>,
    profile_cursor: Option<usize>,
    goto: Option<furniture::GoTo>,
    /// An input method's composition for the field with the keys (Go to, the table's
    /// search, a service's address); empty: none.
    preedit: String,
    /// Where Go to went (lat, lon), until when.
    pin: Option<((f64, f64), f64)>,
    identify: Option<Identify>,
    /// Selected features: (map layer, feature).
    selected: BTreeSet<(usize, usize)>,
    focus: Option<Focus>,
    /// The scene drawn and the layer revision it is for.
    scene: Option<(u64, Arc<Prepared>)>,
    /// Bumped when layers are added, removed or reordered (the scene is built again).
    layers_rev: u64,
    building: Option<u64>,
    jobs: Option<(crossbeam_channel::Sender<Job>, Receiver<Job>)>,
    adding: usize,
    /// Bumped by anything that changes the GPU picture.
    rev: u64,
    press: Option<(f32, f32)>,
    /// The pointer's position on the map (lat, lon).
    hover: Option<(f64, f64)>,
    drawn: view::Drawn,
    /// The last coordinate copied (the clipboard is the system's; tests read this).
    pub(super) copied: Option<String>,
    /// The tools ([`tools`]).
    pub(super) t: tools::Tools,
    /// The services panel and the saved servers (kept across files).
    pub(super) services: services::Services,
}

impl Map {
    fn bump(&mut self) {
        self.rev = self.rev.wrapping_add(1);
    }

    /// A text field has the keys: Go to, the table's search or a service's address.
    pub(super) fn typing(&self) -> bool {
        match self.focus {
            Some(Focus::GoTo) => self.goto.is_some(),
            Some(Focus::Table) => self
                .table
                .as_ref()
                .is_some_and(|t| t.focus == table::Focus::Search),
            Some(Focus::Services) => {
                matches!(self.services.page, Some(services::Page::Add { .. }))
            }
            _ => false,
        }
    }

    /// An input method's composition (empty: none) for the field with the keys.
    pub(super) fn set_preedit(&mut self, s: &str) {
        let s = if self.typing() { s } else { "" };
        if self.preedit != s {
            self.preedit = s.to_owned();
            self.bump();
        }
    }

    /// Where the input method's candidate window opens: at the field's composition.
    pub(in crate::app) fn ime_area(&self) -> Option<specular_gfx::Rect> {
        self.typing().then_some(self.drawn.ime).flatten()
    }

    /// The panels as drawn this frame.
    pub(super) fn set_drawn(&mut self, d: view::Drawn) {
        let rows = d.table.rows_shown;
        self.t.time_drawn = d.time;
        self.services.drawn = d.services.clone();
        self.t.geotag_drawn = d.geotag.clone();
        self.t.export_drawn = d.export.clone();
        self.drawn = d;
        if let Some(t) = self.table.as_mut() {
            t.keep_cursor(rows);
        }
    }

    fn channel(&mut self) -> crossbeam_channel::Sender<Job> {
        self.jobs
            .get_or_insert_with(crossbeam_channel::unbounded)
            .0
            .clone()
    }

    /// Selected features of map layer `layer`.
    fn selected_in(&self, layer: usize) -> BTreeSet<usize> {
        self.selected
            .iter()
            .filter(|(l, _)| *l == layer)
            .map(|(_, f)| *f)
            .collect()
    }

    /// The table shows the selection again (filter, order).
    fn table_refresh(&mut self) {
        let Some(li) = self.table.as_ref().map(|t| t.layer) else {
            return;
        };
        let Some(l) = self.layers.get(li) else {
            self.table = None;
            return;
        };
        let sel = self.selected_in(li);
        let data = l.data.clone();
        let layer = l.layer;
        if let Some(t) = self.table.as_mut() {
            t.refresh(&data.layers[layer], &sel);
        }
    }

    /// The open panels the keyboard walks, in order.
    fn stops(&self) -> Vec<Focus> {
        let mut v = Vec::new();
        if self.panel {
            v.push(Focus::Layers);
        }
        if self.table.is_some() {
            v.push(Focus::Table);
        }
        if self.goto.is_some() {
            v.push(Focus::GoTo);
        }
        if self.tool.is_some() {
            v.push(Focus::Tool);
        }
        if self.profile.is_some() {
            v.push(Focus::Chart);
        }
        if self.t.time.is_some() {
            v.push(Focus::Time);
        }
        if self.services.open {
            v.push(Focus::Services);
        }
        if self.t.geotag.is_some() {
            v.push(Focus::Geotag);
        }
        if self.t.export.is_some() {
            v.push(Focus::Export);
        }
        v
    }
}

/// The settings' coordinate format.
fn format_of(f: CoordinateFormat) -> CoordFormat {
    match f {
        CoordinateFormat::Decimal => CoordFormat::Decimal,
        CoordinateFormat::Dms => CoordFormat::Dms,
        CoordinateFormat::Utm => CoordFormat::Utm,
        CoordinateFormat::Mgrs => CoordFormat::Mgrs,
    }
}

fn setting_of(f: CoordFormat) -> CoordinateFormat {
    match f {
        CoordFormat::Decimal => CoordinateFormat::Decimal,
        CoordFormat::Dms => CoordinateFormat::Dms,
        CoordFormat::Utm => CoordinateFormat::Utm,
        CoordFormat::Mgrs => CoordinateFormat::Mgrs,
    }
}

/// Regions measuring in feet and miles.
const IMPERIAL_REGIONS: [&str; 3] = ["US", "LR", "MM"];

/// The map's units: chosen, else Metadata › Units, else the region's.
pub(super) fn units_of(maps: SettingUnits, metadata: SettingUnits, region: Option<&str>) -> Units {
    match (maps, metadata) {
        (SettingUnits::Metric, _) | (SettingUnits::Auto, SettingUnits::Metric) => Units::Metric,
        (SettingUnits::Imperial, _) | (SettingUnits::Auto, SettingUnits::Imperial) => {
            Units::Imperial
        }
        (SettingUnits::Auto, SettingUnits::Auto) => {
            let r = region.unwrap_or("").to_ascii_uppercase();
            if IMPERIAL_REGIONS.contains(&r.as_str()) {
                Units::Imperial
            } else {
                Units::Metric
            }
        }
    }
}

/// The feature styles of a KML or KMZ file, read again for its colours.
fn file_styles(path: &std::path::Path, format: &str) -> Option<HashMap<String, FeatureStyle>> {
    let kmz = match format {
        "KML" => false,
        "KMZ" => true,
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    Some(if kmz {
        kmz_styles(&bytes)
    } else {
        kml_styles(&bytes)
    })
}

/// Builds the GPU scene of `layers`.
fn prepare_scene(layers: &[layers::MapLayer], geo: &dyn GeoTransform) -> Arc<Prepared> {
    let t0 = std::time::Instant::now();
    let scene = layers::build_scene(layers, geo);
    let p = Arc::new(prepare(
        Arc::new(scene),
        &Fonts::shared(),
        PrepLimits::default(),
    ));
    debug!(target: UI, "map scene: {} layers in {:.1} ms", layers.len(), t0.elapsed().as_secs_f64() * 1e3);
    p
}

/// A file's layers to add over a map, read on a background thread.
fn read_layers(
    path: &std::path::Path,
    first_colour: usize,
) -> Result<Vec<layers::MapLayer>, String> {
    let s = specular_decode::Registry::default()
        .open_path(path)
        .map_err(|e| crate::error_text::reason(&e))?;
    let g = s.geo().ok_or_else(|| t!("map-add-not-map"))?;
    let idx = g
        .features
        .as_ref()
        .ok_or_else(|| t!("map-add-not-vector"))?;
    let data: Arc<Dataset> = idx.dataset.clone();
    if data.crs.is_none() && data.format == "Shapefile" {
        return Err(t!("map-add-no-crs"));
    }
    let styles = file_styles(path, data.format);
    Ok(layers::layers_of(
        &data,
        layers::Coords::of(data.crs.as_ref()),
        styles.as_ref(),
        Some(path.to_path_buf()),
        first_colour,
    ))
}

impl App {
    /// The current image's georeference as the map view reads it, with its heights and
    /// bands, or `None`.
    pub(super) fn gis_sources(&self) -> Option<Sources> {
        let g = self.geo_current().cloned();
        let sci = self.science_for_map();
        let transform: Arc<dyn GeoTransform> = match (&g, &sci) {
            (Some(g), _) if g.crs.is_some() => Arc::new(seam::GeoRefTransform(g.clone())),
            (_, Some((s, _, _))) => match (&s.geo, &s.lonlat, &s.pixel) {
                (Some(g), _, _) if g.crs.is_some() => Arc::new(seam::GeoRefTransform(g.clone())),
                (_, Some(ll), _) => Arc::new(seam::SwathTransform(ll.clone())),
                // Pixels stepping in degrees on gridded data (SRTM), never the sky.
                (_, _, Some(p)) if s.kind == specular_decode::science::ScienceKind::Gridded => {
                    Arc::new(seam::degree_grid(p)?)
                }
                _ => return None,
            },
            _ => return None,
        };
        let mut heights: Option<Arc<dyn HeightSource>> = None;
        let mut bands = Vec::new();
        if let Some((s, planes, relief)) = sci {
            if let [(_, p)] = planes.as_slice() {
                let metres = seam::height_unit_metres(s.unit.as_deref())
                    .filter(|_| relief || s.unit.is_some());
                if let Some(m) = metres {
                    heights = Some(Arc::new(seam::PlaneHeights {
                        plane: p.clone(),
                        metres: m,
                    }));
                }
            }
            bands = planes;
        }
        Some((transform, g, heights, bands))
    }

    /// The image has a georeference the map view can show.
    pub(super) fn gis_available(&self) -> bool {
        !self.gallery_active()
            && self.map.id == self.current_id()
            && self.map.geo.is_some()
            && !self.map.t.terrain_3d
    }

    /// The map view is on: the Maps tools are on and the image is a map.
    pub(super) fn gis_on(&self) -> bool {
        self.gis_available() && self.profile().is_on(Capability::Maps)
    }

    /// `--show map-…` (screenshots): the map view is on and has the values it shows.
    pub(super) fn gis_shot_ready(&self) -> bool {
        let science = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.as_ref())
            .is_some_and(|s| s.science().is_some());
        self.gis_on()
            && (!science || !self.map.bands.is_empty())
            && self.map.t.started
            && !self.map.t.track_job
    }

    /// What the map actions depend on ([`specular_input::ActionState::map`]): set for every
    /// map, so the profile chip offers the Maps tools.
    pub(super) fn gis_state(&self) -> Option<MapState> {
        if !self.gis_available() {
            return None;
        }
        let lonlat = self.map.geo.as_ref().is_some_and(|g| {
            let (w, h) = self.map.size;
            g.to_lonlat(w / 2.0, h / 2.0).is_some()
        });
        Some(MapState {
            lonlat,
            features: self.map.layers.iter().any(|l| l.feature_count() > 0),
            heights: self.map.heights.is_some() && self.gis_relief_ok(),
            time: self.gis_time_source().is_some(),
            wind: self.map.t.wind_source.as_ref().is_some_and(Option::is_some),
            track: self.map.t.track.is_some(),
            grid: !self.map.bands.is_empty(),
        })
    }

    /// The map view draws the image's features (in place of the drawing view).
    pub(super) fn gis_draws_vectors(&self) -> bool {
        self.gis_on() && self.map.vector
    }

    /// Wants the drawing renderer (a raster map with layers added over it).
    pub(super) fn gis_wants_renderer(&self) -> bool {
        self.gis_on() && !self.map.layers.is_empty()
    }

    fn gis_units(&self) -> Units {
        let lang = specular_i18n::I18n::global()
            .localizer()
            .format_language()
            .clone();
        let region = lang.region.as_ref().map(|r| r.as_str().to_owned());
        units_of(
            self.settings.maps.units,
            self.settings.metadata.units,
            region.as_deref(),
        )
    }

    fn gis_format(&self) -> CoordFormat {
        format_of(self.settings.maps.coordinate_format)
    }

    /// Keeps the state in step with the current image; polls background work.
    pub(super) fn gis_sync(&mut self) {
        self.gis_poll();
        self.gis_tools_sync();
        let cur = self.current_id();
        if cur != self.map.id {
            let keep_overview = self.map.overview;
            let panel = self.map.panel;
            let t = self.map.t.carry();
            self.map = Map {
                id: cur,
                overview: keep_overview,
                panel,
                jobs: self.map.jobs.take(),
                copied: self.map.copied.take(),
                services: std::mem::take(&mut self.map.services),
                t,
                ..Map::default()
            };
            self.redraw = true;
        }
        if let Some((_, until)) = self.map.pin
            && self.now() > until
        {
            self.map.pin = None;
            self.redraw = true;
        }
        if self.map.checked || cur.is_none() {
            return;
        }
        // Wait for the session (and science mode's planes, when it shows the image).
        let Some(session) = cur
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.clone())
        else {
            return;
        };
        if session.science().is_some()
            && self.science_for_map().is_none()
            && session.geo().is_none()
        {
            return;
        }
        self.map.checked = true;
        let Some((geo, georef, heights, bands)) = self.gis_sources() else {
            return;
        };
        let (w, h) = (
            f64::from(session.info().width),
            f64::from(session.info().height),
        );
        self.map.size = (w, h);
        self.map.geo = Some(geo.clone());
        self.map.heights = heights;
        self.map.bands = bands;
        self.map.georef = georef.clone();
        self.map.bump();
        // A vector map's own layers, read off the UI thread (KML styles read the file).
        if let Some(g) = georef
            && let Some(idx) = g.features.as_ref()
            && let Some(inv) = g.to_map.inverse()
        {
            self.map.vector = true;
            let data = idx.dataset.clone();
            let path = session.source().path().to_path_buf();
            let tx = self.map.channel();
            let id = cur.unwrap_or_default();
            self.map.building = Some(0);
            self.loader.background("map-layers", move |l| {
                let styles = file_styles(&path, data.format);
                let ls =
                    layers::layers_of(&data, layers::Coords::Stored(inv), styles.as_ref(), None, 0);
                let p = prepare_scene(&ls, geo.as_ref());
                let _ = tx.send(Job::Base(id, ls, p));
                l.waker()();
            });
        }
        self.gis_tools_checked(&session);
        let p = self.pointer;
        if let Some(p) = p {
            self.gis_hover(p);
        }
        self.redraw = true;
    }

    /// Takes what the background threads finished.
    fn gis_poll(&mut self) {
        let Some((_, rx)) = &self.map.jobs else {
            return;
        };
        let jobs: Vec<Job> = rx.try_iter().collect();
        for j in jobs {
            match j {
                Job::Base(id, ls, p) if Some(id) == self.map.id => {
                    let n = ls.len();
                    // Added layers may already be there (a quick add): the own ones go under.
                    let mut all = ls;
                    all.append(&mut self.map.layers);
                    self.map.layers = all;
                    if n == self.map.layers.len() {
                        self.map.scene = Some((self.map.layers_rev, p));
                        self.map.building = None;
                    } else {
                        self.gis_rebuild();
                    }
                    if self.map.chosen.is_none() && !self.map.layers.is_empty() {
                        self.map.chosen = Some(self.map.layers.len() - 1);
                    }
                    self.map.bump();
                    self.redraw = true;
                }
                Job::Scene(id, rev, p) if Some(id) == self.map.id => {
                    if rev == self.map.layers_rev {
                        self.map.scene = Some((rev, p));
                        self.map.building = None;
                    }
                    self.map.bump();
                    self.redraw = true;
                }
                Job::Added(id, path, r) if Some(id) == self.map.id => {
                    self.map.adding = self.map.adding.saturating_sub(1);
                    if r.as_ref().is_ok_and(|l| !l.is_empty()) {
                        self.gis_track_from(&path);
                    }
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let now = self.now();
                    match r {
                        Ok(ls) if !ls.is_empty() => {
                            let n = ls.len();
                            self.map.layers.extend(ls);
                            self.map.chosen = Some(self.map.layers.len() - 1);
                            self.map.panel = true;
                            self.gis_rebuild();
                            self.chrome.toast(
                                t!("map-added", name = name, layers = n as i64),
                                None,
                                now,
                            );
                        }
                        Ok(_) => {
                            let why = t!("map-add-not-vector");
                            self.chrome.toast(
                                t!("map-add-failed", name = name, reason = why),
                                None,
                                now,
                            );
                        }
                        Err(why) => {
                            self.chrome.toast(
                                t!("map-add-failed", name = name, reason = why),
                                None,
                                now,
                            );
                        }
                    }
                    self.redraw = true;
                }
                Job::Service(r) => self.gis_service_reply(r),
                Job::Exported(d) => self.gis_exported(d),
                j => self.gis_tools_job(j),
            }
        }
    }

    /// Builds the scene again (layers added, removed or reordered).
    fn gis_rebuild(&mut self) {
        self.map.layers_rev = self.map.layers_rev.wrapping_add(1);
        let (Some(id), Some(geo)) = (self.map.id, self.map.geo.clone()) else {
            return;
        };
        let rev = self.map.layers_rev;
        let ls = self.map.layers.clone();
        let tx = self.map.channel();
        self.map.building = Some(rev);
        self.loader.background("map-scene", move |l| {
            let p = prepare_scene(&ls, geo.as_ref());
            let _ = tx.send(Job::Scene(id, rev, p));
            l.waker()();
        });
        self.map.table_refresh();
    }

    /// Asks for a file to add as layers.
    fn gis_add_dialog(&mut self) {
        let (Some(id), true) = (self.map.id, self.gis_on()) else {
            return;
        };
        let tx = self.map.channel();
        let first = self.map.layers.len();
        let title = t!("palette-map-add-layer");
        let filter = t!("map-add-filter");
        self.map.adding += 1;
        self.loader.background("dlg-map-add", move |l| {
            let Some(p) = rfd::FileDialog::new()
                .set_title(&title)
                .add_filter(
                    filter,
                    &["geojson", "json", "shp", "gpkg", "kml", "kmz", "gpx"],
                )
                .pick_file()
            else {
                let _ = tx.send(Job::Added(id, PathBuf::new(), Ok(Vec::new())));
                return;
            };
            let r = read_layers(&p, first);
            let _ = tx.send(Job::Added(id, p, r));
            l.waker()();
        });
    }

    // ----- Positions -------------------------------------------------------------------

    /// Stored px of a screen point (physical px), on the image.
    fn gis_stored_at(&self, p: (f32, f32)) -> Option<(f64, f64)> {
        if !super::rect_contains(self.view.viewport, p) {
            return None;
        }
        let (x, y) = self.view.stored_from_screen().apply(p);
        let (x, y) = (f64::from(x), f64::from(y));
        let (w, h) = self.map.size;
        (x >= 0.0 && y >= 0.0 && x <= w && y <= h).then_some((x, y))
    }

    /// (lat, lon) of a stored position.
    fn gis_latlon(&self, s: (f64, f64)) -> Option<(f64, f64)> {
        let (lon, lat) = self.map.geo.as_ref()?.to_lonlat(s.0, s.1)?;
        Some((lat, lon))
    }

    /// Screen (logical px) of a (lat, lon).
    fn gis_screen_of(&self, ll: (f64, f64)) -> Option<(f32, f32)> {
        let (px, py) = self.map.geo.as_ref()?.from_lonlat(ll.1, ll.0)?;
        Some(self.gis_screen_of_stored((px, py)))
    }

    fn gis_screen_of_stored(&self, (px, py): (f64, f64)) -> (f32, f32) {
        let m = self.view.screen_from_stored_f64();
        let ls = f64::from(self.dl.scale.max(0.1));
        (
            ((m[0] * px + m[1] * py + m[2]) / ls) as f32,
            ((m[3] * px + m[4] * py + m[5]) / ls) as f32,
        )
    }

    /// The centre of the viewport in stored px.
    fn gis_centre(&self) -> (f64, f64) {
        let vp = self.view.viewport;
        let (x, y) = self
            .view
            .stored_from_screen()
            .apply((vp.x + vp.w / 2.0, vp.y + vp.h / 2.0));
        (f64::from(x), f64::from(y))
    }

    /// The readout's position lines at stored (`x`, `y`), in the chosen format, while the
    /// map view is on: the CRS's own coordinates for a projected map, then the position.
    pub(super) fn gis_position_lines(&self, x: f64, y: f64) -> Option<Vec<String>> {
        if !self.gis_on() {
            return None;
        }
        let mut lines = Vec::new();
        if let Some(g) = &self.map.georef
            && let Some(l) = super::geo::geo_map_line(g, x, y)
        {
            lines.push(l);
        }
        if let Some((lat, lon)) = self.gis_latlon((x, y))
            && let Some(l) = furniture::position_line(lat, lon, self.gis_format())
        {
            lines.push(l);
        }
        if !self.map.bands.is_empty() {
            // Science mode's readout gives the values.
        } else if let Some(h) = self.map.heights.as_ref().and_then(|h| h.height_at(x, y)) {
            lines.push(t!(
                "map-height",
                height = measure::length_text(h, Units::Metric)
            ));
        }
        lines.extend(self.gis_tool_lines(x, y));
        Some(lines)
    }

    /// Ground metres per logical px at the viewport's centre, along the screen's x.
    fn gis_metres_per_px(&self) -> Option<f64> {
        let vp = self.view.viewport;
        let ls = self.dl.scale.max(0.1);
        let c = (vp.x + vp.w / 2.0, vp.y + vp.h / 2.0);
        let d = 50.0 * ls;
        let a = self.view.stored_from_screen().apply((c.0 - d, c.1));
        let b = self.view.stored_from_screen().apply((c.0 + d, c.1));
        let pa = self.gis_latlon((f64::from(a.0), f64::from(a.1)))?;
        let pb = self.gis_latlon((f64::from(b.0), f64::from(b.1)))?;
        let s = geodesic::inverse(pa.0, pa.1, pb.0, pb.1).s12;
        (s.is_finite() && s > 0.0).then_some(s / 100.0)
    }

    /// The scale bar while the map view is on, in the map's units.
    pub(super) fn gis_scale_bar(&self) -> Option<super::science::ScaleBar> {
        if !self.gis_on() {
            return None;
        }
        furniture::scale_bar(self.gis_metres_per_px()?, self.gis_units())
    }

    /// Clockwise angle of north from screen up at the viewport's centre (radians), when it
    /// is not up.
    fn gis_north(&self) -> Option<f64> {
        let geo = self.map.geo.as_ref()?;
        let c = self.gis_centre();
        let (lon, lat) = geo.to_lonlat(c.0, c.1)?;
        let mpp = self.gis_metres_per_px()?;
        // A step north of about 30 px.
        let dlat = (mpp * 30.0 / 111_000.0).clamp(1e-7, 1.0);
        let (lat2, sign) = if lat + dlat <= 90.0 {
            (lat + dlat, 1.0)
        } else {
            (lat - dlat, -1.0)
        };
        let q = geo.from_lonlat(lon, lat2)?;
        let a = self.gis_screen_of_stored(c);
        let b = self.gis_screen_of_stored(q);
        let (dx, dy) = (f64::from(b.0 - a.0) * sign, f64::from(b.1 - a.1) * sign);
        if dx.hypot(dy) < 1e-6 {
            return None;
        }
        let angle = dx.atan2(-dy);
        (angle.to_degrees().abs() > furniture::NORTH_SLACK).then_some(angle)
    }

    // ----- Actions ---------------------------------------------------------------------

    /// Map actions (and the drawing's layer actions while the map view draws the layers);
    /// true when `a` was one.
    pub(super) fn gis_action(&mut self, a: Action) -> bool {
        let drawing = matches!(a, Action::DrawingLayers | Action::DrawingAllLayers);
        if drawing && !self.gis_draws_vectors() {
            return false;
        }
        if a.mode() != specular_input::Mode::Map && !drawing {
            return false;
        }
        if !self.gis_on() {
            return true;
        }
        let now = self.now();
        match a {
            Action::MapLayers | Action::DrawingLayers => {
                self.map.panel = !self.map.panel;
                self.map.focus = self.map.panel.then_some(Focus::Layers);
                if self.map.chosen.is_none() && !self.map.layers.is_empty() {
                    self.map.chosen = Some(self.map.layers.len() - 1);
                }
            }
            Action::DrawingAllLayers => {
                for l in &mut self.map.layers {
                    l.visible = true;
                }
                self.chrome.toast(t!("drawing-layers-all-toast"), None, now);
            }
            Action::MapAddLayer => self.gis_add_dialog(),
            Action::MapZoomToLayer => self.gis_zoom_to_layer(),
            Action::MapAttributeTable => self.gis_toggle_table(None),
            Action::MapCoordinateFormat => {
                let next = self.gis_format().next();
                self.store.update(Origin::Program, |s| {
                    s.maps.coordinate_format = setting_of(next);
                });
                self.apply_settings(false);
                let name = t!("map-format-name", format = next.id());
                self.chrome
                    .toast(t!("map-format-toast", format = name), None, now);
                self.gis_refresh_readout();
            }
            Action::MapUnits => {
                let next = match self.gis_units() {
                    Units::Metric => SettingUnits::Imperial,
                    Units::Imperial => SettingUnits::Metric,
                };
                self.store.update(Origin::Program, |s| s.maps.units = next);
                self.apply_settings(false);
                let msg = match next {
                    SettingUnits::Imperial => t!("map-units-imperial"),
                    _ => t!("map-units-metric"),
                };
                self.chrome.toast(msg, None, now);
            }
            Action::MapCopyCoordinate => self.gis_copy_coordinate(),
            Action::MapGoToCoordinate => {
                if self.map.goto.is_some() {
                    self.map.goto = None;
                    self.map.focus = None;
                } else {
                    self.map.goto = Some(furniture::GoTo::default());
                    self.map.focus = Some(Focus::GoTo);
                }
            }
            Action::MapMeasureDistance => self.gis_tool(measure::Kind::Distance),
            Action::MapMeasureArea => self.gis_tool(measure::Kind::Area),
            Action::MapElevationProfile => self.gis_tool(measure::Kind::Profile),
            Action::MapOverview => self.map.overview = !self.map.overview,
            a => self.gis_tool_action(a),
        }
        self.map.bump();
        self.redraw = true;
        true
    }

    /// Opens a tool, or closes it when it is the one open.
    fn gis_tool(&mut self, kind: measure::Kind) {
        if self.map.tool.as_ref().is_some_and(|t| t.kind == kind) {
            self.map.tool = None;
            self.map.focus = None;
            return;
        }
        self.map.tool = Some(measure::Tool::new(kind));
        self.map.focus = Some(Focus::Tool);
        if kind != measure::Kind::Profile {
            self.map.profile = None;
        }
    }

    /// Opens or closes the attribute table of `layer` (else the chosen layer, else the
    /// first with features).
    fn gis_toggle_table(&mut self, layer: Option<usize>) {
        if layer.is_none() && self.map.table.is_some() {
            self.map.table = None;
            if self.map.focus == Some(Focus::Table) {
                self.map.focus = None;
            }
            return;
        }
        let li = layer
            .or(self.map.chosen)
            .filter(|&i| {
                self.map
                    .layers
                    .get(i)
                    .is_some_and(|l| l.feature_count() > 0)
            })
            .or_else(|| self.map.layers.iter().position(|l| l.feature_count() > 0));
        let Some(li) = li else { return };
        self.map.table = Some(table::Table::new(li));
        self.map.chosen = Some(li);
        self.map.focus = Some(Focus::Table);
        self.map.table_refresh();
    }

    /// Shows the whole of the chosen layer (else the map).
    fn gis_zoom_to_layer(&mut self) {
        let Some(geo) = self.map.geo.clone() else {
            return;
        };
        let e = self
            .map
            .chosen
            .and_then(|i| self.map.layers.get(i))
            .and_then(|l| layers::extent(l, geo.as_ref()));
        match e {
            Some(e) => self.gis_show_stored(e),
            None => {
                self.view.zoom_fit();
                self.after_view_change();
            }
        }
    }

    /// Fits the stored rectangle `[x0, y0, x1, y1]` into the view (a point gets some room).
    fn gis_show_stored(&mut self, e: [f64; 4]) {
        let pad = ((e[2] - e[0]).max(e[3] - e[1]) * 0.05).max(8.0);
        let r = (
            (e[0] - pad) as f32,
            (e[1] - pad) as f32,
            (e[2] + pad) as f32,
            (e[3] + pad) as f32,
        );
        self.view.show_stored_rect(r, None);
        self.after_view_change();
    }

    /// Copies the position under the pointer (else of the centre) in the chosen format.
    fn gis_copy_coordinate(&mut self) {
        let at = self
            .pointer
            .and_then(|p| self.gis_stored_at(p))
            .unwrap_or_else(|| self.gis_centre());
        let Some((lat, lon)) = self.gis_latlon(at) else {
            return;
        };
        let Some(text) = coords::format(lat, lon, self.gis_format()) else {
            return;
        };
        self.map.copied = Some(text.clone());
        self.clipboard
            .copy_text(&self.loader, text, Copied::Coordinate);
    }

    /// Enter in Go to: parse and centre there.
    fn gis_go(&mut self) {
        let Some(text) = self.map.goto.as_ref().map(|g| g.text.clone()) else {
            return;
        };
        let parsed = coords::parse(&text);
        let err = |e: coords::ParseError| match e {
            coords::ParseError::Empty => t!("map-goto-empty"),
            coords::ParseError::Unrecognised => t!("map-goto-unrecognised"),
            coords::ParseError::OutOfRange => t!("map-goto-range"),
        };
        let (lat, lon) = match parsed {
            Ok(p) => p,
            Err(e) => {
                if let Some(g) = self.map.goto.as_mut() {
                    g.error = Some(err(e));
                }
                return;
            }
        };
        let Some(s) = self.map.geo.as_ref().and_then(|g| g.from_lonlat(lon, lat)) else {
            if let Some(g) = self.map.goto.as_mut() {
                g.error = Some(t!("map-goto-no-way"));
            }
            return;
        };
        let (w, h) = self.map.size;
        let now = self.now();
        if s.0 < 0.0 || s.1 < 0.0 || s.0 > w || s.1 > h {
            if let Some(g) = self.map.goto.as_mut() {
                g.error = Some(t!("map-goto-outside"));
            }
            return;
        }
        let k = 0.5;
        let scale = Some(self.view.target_scale());
        self.view.show_stored_rect(
            (
                (s.0 - k) as f32,
                (s.1 - k) as f32,
                (s.0 + k) as f32,
                (s.1 + k) as f32,
            ),
            scale,
        );
        self.after_view_change();
        self.map.pin = Some(((lat, lon), now + PIN_SECONDS));
        self.map.goto = None;
        self.map.focus = None;
        let shown = coords::format(lat, lon, self.gis_format()).unwrap_or(text);
        self.chrome
            .toast(t!("map-goto-toast", position = shown), None, now);
    }

    // ----- Identify and selection -------------------------------------------------------

    /// The topmost visible feature at stored `s`: (map layer, feature).
    fn gis_hit(&self, s: (f64, f64)) -> Option<(usize, usize)> {
        let geo = self.map.geo.as_ref()?;
        let scale = f64::from(self.view.scale()).max(1e-9);
        let tol_px = HIT_PX * f64::from(self.dl.scale.max(0.1)) / scale;
        for (li, l) in self.map.layers.iter().enumerate().rev() {
            if !l.visible || l.opacity <= 0.0 {
                continue;
            }
            let Some(p) = l.coords.dataset_at(s, geo.as_ref()) else {
                continue;
            };
            let Some(q) = l.coords.dataset_at((s.0 + tol_px, s.1), geo.as_ref()) else {
                continue;
            };
            let tol = (q[0] - p[0]).hypot(q[1] - p[1]);
            if let Some(f) = layers::hit_layer(l, p, tol) {
                return Some((li, f));
            }
        }
        None
    }

    /// What identify lists for a feature.
    fn gis_identify_feature(&self, li: usize, f: usize) -> Option<Identify> {
        let l = self.map.layers.get(li)?;
        let src = &l.data.layers[l.layer];
        let feat = src.features.get(f)?;
        let name = ["name", "title", "label"]
            .iter()
            .find_map(|want| {
                src.fields()
                    .iter()
                    .position(|k| k.eq_ignore_ascii_case(want))
                    .and_then(|i| feat.values.get(i))
                    .map(|v| v.text())
                    .filter(|t| !t.trim().is_empty())
            })
            .unwrap_or_else(|| format!("#{}", f + 1));
        let mut rows: Vec<(String, String)> = Vec::new();
        rows.push((t!("geo-feature-layer"), l.shown_name()));
        if let Some(g) = &feat.geometry {
            rows.push((
                t!("geo-feature-geometry"),
                t!(
                    "geo-feature-geometry-value",
                    kind = g.type_name(),
                    vertices = g.vertex_count() as i64
                ),
            ));
        }
        let attrs: Vec<(String, String)> = src
            .fields()
            .iter()
            .zip(&feat.values)
            .map(|(k, v)| (k.clone(), v.text()))
            .filter(|(_, v)| !v.trim().is_empty())
            .collect();
        let more = attrs.len().saturating_sub(IDENTIFY_ROWS);
        rows.extend(attrs.into_iter().take(IDENTIFY_ROWS));
        Some(Identify {
            title: name,
            rows,
            more,
        })
    }

    /// What identify lists for a raster cell: the value in each band.
    fn gis_identify_cell(&self, s: (f64, f64)) -> Option<Identify> {
        let (x, y) = (s.0.floor() as u32, s.1.floor() as u32);
        let mut rows = Vec::new();
        if !self.map.bands.is_empty() {
            for (k, (name, p)) in self.map.bands.iter().enumerate() {
                let v = p.value(x, y).filter(|v| v.is_finite());
                let name = if name.is_empty() {
                    t!("map-band", n = (k + 1) as i64)
                } else {
                    name.clone()
                };
                rows.push((
                    name,
                    v.map_or_else(|| t!("map-no-data"), |v| table::stat_value(f64::from(v))),
                ));
            }
        } else {
            let img = self.current_id().and_then(|id| self.images.get(&id))?;
            let (_, r) = img.overview.as_ref()?;
            use specular_inspect::SourceValue as V;
            let v = specular_inspect::source_pixel(r, x, y)?;
            let vals: Vec<String> = match v {
                V::Gray(g) => vec![g.to_string()],
                V::Rgb(c) | V::YCbCr { rgb: c, .. } => c.iter().map(u8::to_string).collect(),
                V::Rgba(c) => c.iter().map(u8::to_string).collect(),
                V::Gray16(g) => vec![g.to_string()],
                V::GrayAlpha16(c) => c.iter().map(u16::to_string).collect(),
                V::Rgba16(c) => c.iter().map(u16::to_string).collect(),
            };
            for (k, v) in vals.into_iter().enumerate() {
                rows.push((t!("map-band", n = (k + 1) as i64), v));
            }
        }
        if let Some(h) = self
            .map
            .heights
            .as_ref()
            .and_then(|h| h.height_at(s.0, s.1))
        {
            rows.push((
                t!("map-height-label"),
                measure::length_text(h, Units::Metric),
            ));
        }
        if rows.is_empty() {
            return None;
        }
        Some(Identify {
            title: t!("map-cell", x = i64::from(x), y = i64::from(y)),
            rows,
            more: 0,
        })
    }

    /// A click on the map at stored `s`: the feature under it is identified and selected
    /// (with Shift added to or taken from the selection); else the raster cell.
    fn gis_click(&mut self, s: (f64, f64), shift: bool) {
        match self.gis_hit(s) {
            Some((li, f)) => {
                if shift {
                    if !self.map.selected.remove(&(li, f)) {
                        self.map.selected.insert((li, f));
                    }
                } else {
                    self.map.selected.clear();
                    self.map.selected.insert((li, f));
                }
                self.map.identify = self.gis_identify_feature(li, f);
                self.map.chosen = Some(li);
                if self.map.table.as_ref().is_some_and(|t| t.layer != li) {
                    self.gis_toggle_table(Some(li));
                }
                self.map.table_refresh();
                let rows = self.map.drawn.table.rows_shown;
                if let Some(t) = self.map.table.as_mut() {
                    t.reveal(f, rows);
                }
            }
            None => {
                if !shift {
                    self.map.selected.clear();
                    self.map.table_refresh();
                }
                self.map.identify = if self.map.vector {
                    None
                } else {
                    self.gis_identify_cell(s)
                };
            }
        }
        self.redraw = true;
    }

    /// Selects table row `row` alone (or toggles it with `toggle`), and shows it.
    fn gis_table_select(&mut self, row: usize, toggle: bool, zoom: bool) {
        let Some(t) = self.map.table.as_ref() else {
            return;
        };
        let li = t.layer;
        let Some(&f) = t.rows.get(row) else { return };
        if toggle {
            if !self.map.selected.remove(&(li, f)) {
                self.map.selected.insert((li, f));
            }
        } else {
            self.map.selected.clear();
            self.map.selected.insert((li, f));
        }
        if let Some(t) = self.map.table.as_mut() {
            t.cursor = row;
        }
        self.map.identify = self.gis_identify_feature(li, f);
        if zoom && let Some(geo) = self.map.geo.clone() {
            let l = &self.map.layers[li];
            if let Some(b) = l.boxes.get(f).copied().flatten() {
                let mut e: Option<[f64; 4]> = None;
                for p in [[b[0], b[1]], [b[2], b[1]], [b[0], b[3]], [b[2], b[3]]] {
                    if let Some((x, y)) = l.coords.to_stored(p, geo.as_ref()) {
                        e = Some(e.map_or([x, y, x, y], |e| {
                            [e[0].min(x), e[1].min(y), e[2].max(x), e[3].max(y)]
                        }));
                    }
                }
                if let Some(e) = e {
                    self.gis_show_stored(e);
                }
            }
        }
        if self.map.table.as_ref().is_some_and(|t| t.selected_only) {
            self.map.table_refresh();
        }
        self.redraw = true;
    }

    // ----- Pointer ---------------------------------------------------------------------

    fn gis_refresh_readout(&mut self) {
        if let Some(p) = self.pointer {
            self.gis_hover(p);
        }
        self.redraw = true;
    }

    /// The pointer over the map: the tool's rubber band and the chart's cursor follow.
    fn gis_hover(&mut self, p: (f32, f32)) {
        let ll = self.gis_stored_at(p).and_then(|s| self.gis_latlon(s));
        if ll != self.map.hover {
            self.map.hover = ll;
            if self.map.tool.is_some() {
                self.redraw = true;
            }
        }
    }

    /// Pointer motion; never takes it.
    pub(super) fn gis_motion(&mut self, p: (f32, f32)) {
        if !self.gis_on() {
            return;
        }
        self.gis_hover(p);
        let lp = self.logical(p);
        self.gis_tools_motion(lp);
        if let (Some(prof), Some(plot)) = (self.map.profile.as_ref(), self.map.drawn.chart.plot) {
            let over = self.map.drawn.chart.panel.is_some_and(|r| r.contains(lp));
            let i = over
                .then(|| measure::sample_at(prof, plot, lp.0, self.rtl()))
                .flatten();
            if over && i != self.map.profile_cursor {
                self.map.profile_cursor = i;
                self.redraw = true;
            }
        }
    }

    /// The wheel over a panel scrolls it. True when taken.
    pub(super) fn gis_wheel(&mut self, dy: f32, lp: (f32, f32)) -> bool {
        if !self.gis_on() {
            return false;
        }
        let steps = (dy.abs().ceil() as usize).max(1) * 3;
        if self.map.drawn.table.rect.is_some_and(|r| r.contains(lp)) {
            if let Some(t) = self.map.table.as_mut() {
                let max = t.rows.len().saturating_sub(self.map.drawn.table.rows_shown);
                t.scroll = if dy > 0.0 {
                    t.scroll.saturating_sub(steps)
                } else {
                    (t.scroll + steps).min(max)
                };
            }
            self.redraw = true;
            return true;
        }
        if self.map.drawn.panel.rect.is_some_and(|r| r.contains(lp)) {
            self.map.panel_scroll = if dy > 0.0 {
                self.map.panel_scroll.saturating_sub(1)
            } else {
                (self.map.panel_scroll + 1).min(self.map.layers.len().saturating_sub(1))
            };
            self.redraw = true;
            return true;
        }
        false
    }

    /// The primary button. Presses in a panel are taken; a click (no drag) on the map adds
    /// a measuring point or identifies. True when taken.
    pub(super) fn gis_button(&mut self, p: (f32, f32), lp: (f32, f32), down: bool) -> bool {
        if !self.gis_on() {
            return false;
        }
        if down {
            if self.gis_panel_press(lp) {
                self.map.press = None;
                self.redraw = true;
                return true;
            }
            self.map.press = Some(lp);
            return false;
        }
        if self.gis_tools_release(lp) {
            return true;
        }
        let Some(q) = self.map.press.take() else {
            return false;
        };
        if (lp.0 - q.0).hypot(lp.1 - q.1) > CLICK_SLOP {
            return false;
        }
        let Some(s) = self.gis_stored_at(p) else {
            return false;
        };
        if let Some(t) = self.map.tool.as_mut() {
            if let Some((lon, lat)) = self.map.geo.as_ref().and_then(|g| g.to_lonlat(s.0, s.1)) {
                t.add((lat, lon));
                self.map.focus = Some(Focus::Tool);
            }
            self.redraw = true;
            return false;
        }
        self.gis_click(s, self.mods.shift_key());
        false
    }

    /// A press over one of the panels; true when it was over one.
    fn gis_panel_press(&mut self, lp: (f32, f32)) -> bool {
        if self.gis_tools_press(lp) {
            return true;
        }
        let d = self.map.drawn.clone();
        if let Some(r) = d.panel.rect
            && r.contains(lp)
        {
            self.map.focus = Some(Focus::Layers);
            if let Some((h, _)) = d.panel.hits.iter().find(|(_, r)| r.contains(lp)) {
                self.gis_panel_hit(*h);
            }
            return true;
        }
        if let Some(r) = d.table.rect
            && r.contains(lp)
        {
            self.map.focus = Some(Focus::Table);
            let hit = d
                .table
                .hits
                .iter()
                .find(|(_, r)| r.contains(lp))
                .map(|h| h.0);
            match hit {
                Some(table::TableHit::Header(c)) => {
                    if let Some(t) = self.map.table.as_mut() {
                        t.col = c;
                        t.sort_by(c);
                    }
                    self.map.table_refresh();
                }
                Some(table::TableHit::Row(i)) => {
                    let shift = self.mods.shift_key();
                    let ctrl = self.mods.control_key();
                    if let Some(t) = self.map.table.as_mut() {
                        t.focus = table::Focus::Rows;
                    }
                    self.gis_table_select(i, shift || ctrl, false);
                }
                Some(table::TableHit::Search) => {
                    if let Some(t) = self.map.table.as_mut() {
                        t.focus = table::Focus::Search;
                    }
                }
                Some(table::TableHit::SelectedOnly) => {
                    if let Some(t) = self.map.table.as_mut() {
                        t.selected_only = !t.selected_only;
                    }
                    self.map.table_refresh();
                }
                Some(table::TableHit::Close) => {
                    self.map.table = None;
                    self.map.focus = None;
                }
                None => {}
            }
            return true;
        }
        if d.chart.panel.is_some_and(|r| r.contains(lp)) {
            self.map.focus = Some(Focus::Chart);
            return true;
        }
        if d.goto.is_some_and(|r| r.contains(lp)) {
            self.map.focus = Some(Focus::GoTo);
            return true;
        }
        if d.tool.is_some_and(|r| r.contains(lp)) {
            self.map.focus = Some(Focus::Tool);
            return true;
        }
        if d.identify.is_some_and(|r| r.contains(lp)) {
            self.map.identify = None;
            return true;
        }
        if d.overview.is_some_and(|r| r.contains(lp)) || d.north.is_some_and(|r| r.contains(lp)) {
            return true;
        }
        // A press on the map leaves the panels.
        if self.map.focus != Some(Focus::Tool) {
            self.map.focus = None;
        }
        false
    }

    /// Runs what a panel target does.
    fn gis_panel_hit(&mut self, h: layers::PanelHit) {
        use layers::{Control, PanelHit};
        match h {
            PanelHit::Row(i) => {
                self.map.chosen = Some(i);
                self.map.control = None;
                self.map.confirm_remove = false;
            }
            PanelHit::Check(i) => {
                if let Some(l) = self.map.layers.get_mut(i) {
                    l.visible = !l.visible;
                }
                self.map.chosen = Some(i);
            }
            PanelHit::Control(c) => {
                self.map.control = Some(c);
                self.gis_control(c);
            }
            PanelHit::Add => self.gis_add_dialog(),
            PanelHit::ConfirmRemove => self.gis_remove_chosen(),
            PanelHit::ConfirmKeep => self.map.confirm_remove = false,
        }
        let _ = Control::ALL;
        self.map.bump();
    }

    /// One of the chosen layer's controls.
    fn gis_control(&mut self, c: layers::Control) {
        use layers::Control as C;
        let Some(i) = self.map.chosen.filter(|&i| i < self.map.layers.len()) else {
            return;
        };
        let step = |v: Option<f32>, list: &[f32]| {
            let at = v.and_then(|v| list.iter().position(|x| (*x - v).abs() < 1e-3));
            Some(list[at.map_or(0, |k| (k + 1) % list.len())])
        };
        let n = self.map.layers.len();
        match c {
            C::Visible => {
                let l = &mut self.map.layers[i];
                l.visible = !l.visible;
            }
            C::OpacityDown | C::OpacityUp => {
                let l = &mut self.map.layers[i];
                let d = if c == C::OpacityUp { 1.0 } else { -1.0 } * layers::OPACITY_STEP;
                l.opacity = ((l.opacity + d) * 10.0).round().clamp(0.0, 10.0) / 10.0;
            }
            C::Colour => {
                let l = &mut self.map.layers[i];
                let k = (0..8)
                    .map(
                        |k| match specular_decode::geo::specular_gis::scene::layer_colors(k).0 {
                            specular_decode::drawing::specular_vector::Color::Rgb(r, g, b) => {
                                [r, g, b]
                            }
                            _ => [0, 0, 0],
                        },
                    )
                    .collect::<Vec<_>>();
                let at = k
                    .iter()
                    .position(|c| *c == l.ink())
                    .map_or(0, |p| (p + 1) % 8);
                l.user_colour = Some(k[at]);
            }
            C::Width => {
                let l = &mut self.map.layers[i];
                l.line_px = step(l.line_px, &layers::WIDTHS);
            }
            C::Point => {
                let l = &mut self.map.layers[i];
                l.point_px = step(l.point_px, &layers::POINTS);
            }
            C::Up | C::Down => {
                // Up the panel is up the drawing order.
                let j = if c == C::Up { i + 1 } else { i.wrapping_sub(1) };
                if j < n {
                    self.map.layers.swap(i, j);
                    self.map.chosen = Some(j);
                    let swap = |l: usize| {
                        if l == i {
                            j
                        } else if l == j {
                            i
                        } else {
                            l
                        }
                    };
                    self.map.selected = self
                        .map
                        .selected
                        .iter()
                        .map(|&(l, f)| (swap(l), f))
                        .collect();
                    if let Some(t) = self.map.table.as_mut() {
                        t.layer = swap(t.layer);
                    }
                    self.gis_rebuild();
                }
            }
            C::Zoom => self.gis_zoom_to_layer(),
            C::Table => self.gis_toggle_table(Some(i)),
            C::Remove => {
                if self.map.layers[i].added.is_some() {
                    self.map.confirm_remove = true;
                }
            }
        }
        self.map.bump();
        self.redraw = true;
    }

    /// Removes the chosen layer (one added from a file; the file stays on disk).
    fn gis_remove_chosen(&mut self) {
        self.map.confirm_remove = false;
        let Some(i) = self
            .map
            .chosen
            .filter(|&i| self.map.layers.get(i).is_some_and(|l| l.added.is_some()))
        else {
            return;
        };
        let l = self.map.layers.remove(i);
        let shift = |x: usize| if x > i { x - 1 } else { x };
        self.map.selected = self
            .map
            .selected
            .iter()
            .filter(|(li, _)| *li != i)
            .map(|&(li, f)| (shift(li), f))
            .collect();
        if self.map.table.as_ref().is_some_and(|t| t.layer == i) {
            self.map.table = None;
        } else if let Some(t) = self.map.table.as_mut() {
            t.layer = shift(t.layer);
        }
        self.map.chosen = (!self.map.layers.is_empty()).then(|| i.min(self.map.layers.len() - 1));
        self.map.control = None;
        self.gis_rebuild();
        let now = self.now();
        self.chrome
            .toast(t!("map-removed", name = l.shown_name()), None, now);
    }

    // ----- Keys ------------------------------------------------------------------------

    /// A panel has the keys.
    pub(super) fn gis_wants_keys(&self) -> bool {
        self.gis_on() && self.map.focus.is_some()
    }

    /// A key while a panel has focus; false when it is not the panel's (the keymap gets
    /// it).
    pub(super) fn gis_key(
        &mut self,
        combo: Option<&KeyCombo>,
        text: Option<&str>,
        _el: &ActiveEventLoop,
    ) -> bool {
        let Some(focus) = self.map.focus else {
            return false;
        };
        let Some(c) = combo else {
            // Text without a shortcut combination (composed characters).
            return self.gis_text(focus, text);
        };
        let shift = c.mods.shift;
        let plain = !c.mods.ctrl && !c.mods.alt && !c.mods.meta;
        if c.key == Key::Named(NamedKey::Tab) && !c.mods.ctrl && !c.mods.alt {
            let stops = self.map.stops();
            if stops.is_empty() {
                return false;
            }
            let at = stops.iter().position(|f| *f == focus).unwrap_or(0) as isize;
            let n = stops.len() as isize;
            let next = at + if shift { -1 } else { 1 };
            // Past the last (or before the first) the map gets the keys back.
            self.map.focus = (0..n).contains(&next).then(|| stops[next as usize]);
            self.map.preedit.clear();
            self.redraw = true;
            return true;
        }
        if c.key == Key::Named(NamedKey::Escape) {
            match focus {
                Focus::GoTo => self.map.goto = None,
                Focus::Tool => self.map.tool = None,
                Focus::Chart => {
                    self.map.profile = None;
                    self.map.tool = None;
                }
                Focus::Table => self.map.table = None,
                Focus::Time => self.map.t.time = None,
                Focus::Services if self.map.services.back() => {
                    self.redraw = true;
                    return true;
                }
                Focus::Services => self.map.services.open = false,
                Focus::Geotag => self.map.t.geotag = None,
                Focus::Export => self.map.t.export = None,
                Focus::Layers if self.map.confirm_remove => {
                    self.map.confirm_remove = false;
                    self.redraw = true;
                    return true;
                }
                Focus::Layers => self.map.panel = false,
            }
            self.map.focus = None;
            self.map.preedit.clear();
            self.map.bump();
            self.redraw = true;
            return true;
        }
        let used = match focus {
            Focus::GoTo => self.gis_goto_key(c, text),
            Focus::Tool => self.gis_tool_key(c, plain),
            Focus::Chart => self.gis_chart_key(c),
            Focus::Table => self.gis_table_key(c, text, plain),
            Focus::Layers => self.gis_layers_key(c, plain),
            Focus::Time => self.gis_time_key(c),
            Focus::Services => self.gis_services_key(c, text),
            Focus::Geotag => self.gis_geotag_key(c),
            Focus::Export => self.gis_export_key(c),
        };
        if used {
            self.map.bump();
            self.redraw = true;
        }
        used
    }

    /// Typed text for a field.
    fn gis_text(&mut self, focus: Focus, text: Option<&str>) -> bool {
        let Some(s) = text.filter(|s| !s.chars().any(char::is_control)) else {
            return false;
        };
        match focus {
            Focus::GoTo => {
                if let Some(g) = self.map.goto.as_mut() {
                    g.text.push_str(s);
                    g.error = None;
                }
            }
            Focus::Services => return self.gis_services_text(s),
            Focus::Table
                if self
                    .map
                    .table
                    .as_ref()
                    .is_some_and(|t| t.focus == table::Focus::Search) =>
            {
                if let Some(t) = self.map.table.as_mut() {
                    t.search.push_str(s);
                    t.scroll = 0;
                    t.cursor = 0;
                }
                self.map.table_refresh();
            }
            _ => return false,
        }
        self.redraw = true;
        true
    }

    /// A map field has the keys (input method commits go to it).
    pub(super) fn gis_wants_text(&self) -> bool {
        self.gis_on() && self.map.typing()
    }

    /// An input method's composition for the map's field.
    pub(super) fn gis_set_preedit(&mut self, s: &str) {
        let on = self.gis_on();
        self.map.set_preedit(if on { s } else { "" });
    }

    /// An input method's commit, typed into the map's field.
    pub(super) fn gis_commit(&mut self, s: &str) {
        self.map.preedit.clear();
        if let Some(f) = self.map.focus
            && self.gis_text(f, Some(s))
        {
            self.map.bump();
        }
        self.redraw = true;
    }

    /// Pasted text for the Go to field, when it is still open.
    pub(super) fn map_goto_paste(&mut self, s: &str) {
        if let Some(g) = self.map.goto.as_mut() {
            g.paste(s);
            self.map.bump();
        }
    }

    fn gis_goto_key(&mut self, c: &KeyCombo, text: Option<&str>) -> bool {
        match c.key {
            Key::Named(NamedKey::Enter) => self.gis_go(),
            Key::Named(NamedKey::Backspace) => {
                if let Some(g) = self.map.goto.as_mut() {
                    g.text.pop();
                    g.error = None;
                }
            }
            Key::Named(NamedKey::Delete) => {
                if let Some(g) = self.map.goto.as_mut() {
                    g.text.clear();
                    g.error = None;
                }
            }
            // Ctrl+V and Shift+Insert paste (read off the UI thread).
            Key::Char(ch) if (c.mods.ctrl || c.mods.meta) && ch.eq_ignore_ascii_case(&'v') => {
                self.paste_text(crate::app::integration::PasteTo::MapGoTo);
            }
            Key::Named(NamedKey::Insert) if c.mods.shift => {
                self.paste_text(crate::app::integration::PasteTo::MapGoTo);
            }
            _ if !c.mods.ctrl && !c.mods.alt => return self.gis_text(Focus::GoTo, text),
            // Other shortcuts stay inert while typing.
            _ => {}
        }
        true
    }

    fn gis_tool_key(&mut self, c: &KeyCombo, plain: bool) -> bool {
        if !plain {
            return false;
        }
        let step = CROSS_STEP * if c.mods.shift { 10.0 } else { 1.0 };
        let move_by = |dx: f64, dy: f64, me: &mut App| {
            let centre = me.gis_centre();
            let s = f64::from(me.view.scale()).max(1e-9) / f64::from(me.dl.scale.max(0.1));
            if let Some(t) = me.map.tool.as_mut() {
                let (x, y) = t.cross.unwrap_or(centre);
                t.cross = Some((x + dx * step / s, y + dy * step / s));
            }
        };
        match c.key {
            Key::Named(NamedKey::ArrowLeft) => move_by(-1.0, 0.0, self),
            Key::Named(NamedKey::ArrowRight) => move_by(1.0, 0.0, self),
            Key::Named(NamedKey::ArrowUp) => move_by(0.0, -1.0, self),
            Key::Named(NamedKey::ArrowDown) => move_by(0.0, 1.0, self),
            Key::Named(NamedKey::Space) => {
                let at = self
                    .map
                    .tool
                    .as_ref()
                    .and_then(|t| t.cross)
                    .unwrap_or_else(|| self.gis_centre());
                if let Some(ll) = self.gis_latlon(at)
                    && let Some(t) = self.map.tool.as_mut()
                {
                    t.add(ll);
                }
            }
            Key::Named(NamedKey::Backspace) => {
                if let Some(t) = self.map.tool.as_mut() {
                    t.done = false;
                    t.pts.pop();
                }
                self.map.profile = None;
            }
            Key::Named(NamedKey::Enter) => self.gis_finish_tool(),
            _ => return false,
        }
        true
    }

    /// Enter in a tool: the measurement is done; a profile is charted.
    fn gis_finish_tool(&mut self) {
        let Some(t) = self.map.tool.as_mut() else {
            return;
        };
        if !t.finish() {
            return;
        }
        if t.kind == measure::Kind::Profile {
            let pts = t.pts.clone();
            let (Some(geo), Some(h)) = (self.map.geo.clone(), self.map.heights.clone()) else {
                return;
            };
            let p = measure::elevation(&pts, |lat, lon| {
                let (x, y) = geo.from_lonlat(lon, lat)?;
                h.height_at(x, y)
            });
            self.map.profile = Some(p);
            self.map.profile_cursor = None;
            self.map.focus = Some(Focus::Chart);
        }
    }

    fn gis_chart_key(&mut self, c: &KeyCombo) -> bool {
        let Some(n) = self.map.profile.as_ref().map(|p| p.samples.len()) else {
            return false;
        };
        if n == 0 {
            return false;
        }
        // Distance runs with the reading direction.
        let fwd = if self.rtl() { -1 } else { 1 };
        let step = if c.mods.shift { 16 } else { 1 } as isize;
        let cur = self.map.profile_cursor.map_or(-1, |i| i as isize);
        let next = match c.key {
            Key::Named(NamedKey::ArrowRight) => cur + fwd * step,
            Key::Named(NamedKey::ArrowLeft) => cur - fwd * step,
            Key::Named(NamedKey::Home) => 0,
            Key::Named(NamedKey::End) => n as isize - 1,
            _ => return false,
        };
        self.map.profile_cursor = Some(next.clamp(0, n as isize - 1) as usize);
        true
    }

    fn gis_table_key(&mut self, c: &KeyCombo, text: Option<&str>, plain: bool) -> bool {
        let Some(t) = self.map.table.as_ref() else {
            return false;
        };
        let rows_shown = self.map.drawn.table.rows_shown.max(1);
        let n = t.rows.len();
        if t.focus == table::Focus::Search {
            match c.key {
                Key::Named(NamedKey::Backspace) => {
                    if let Some(t) = self.map.table.as_mut() {
                        t.search.pop();
                    }
                    self.map.table_refresh();
                }
                Key::Named(NamedKey::Enter | NamedKey::ArrowDown) => {
                    if let Some(t) = self.map.table.as_mut() {
                        t.focus = table::Focus::Rows;
                    }
                }
                _ if plain || c.mods.shift && !c.mods.ctrl && !c.mods.alt => {
                    return self.gis_text(Focus::Table, text) || true;
                }
                _ => {}
            }
            return true;
        }
        let fields = self
            .map
            .layers
            .get(t.layer)
            .map_or(0, |l| l.data.layers[l.layer].fields().len());
        let (cursor, col) = (t.cursor, t.col);
        let to_row = |me: &mut App, r: usize| {
            if let Some(t) = me.map.table.as_mut() {
                t.cursor = r.min(n.saturating_sub(1));
                t.keep_cursor(rows_shown);
            }
        };
        match (c.key, c.mods.ctrl) {
            (Key::Named(NamedKey::ArrowDown), _) => to_row(self, cursor + 1),
            (Key::Named(NamedKey::ArrowUp), _) => to_row(self, cursor.saturating_sub(1)),
            (Key::Named(NamedKey::PageDown), _) => to_row(self, cursor + rows_shown),
            (Key::Named(NamedKey::PageUp), _) => to_row(self, cursor.saturating_sub(rows_shown)),
            (Key::Named(NamedKey::Home), _) => to_row(self, 0),
            (Key::Named(NamedKey::End), _) => to_row(self, n.saturating_sub(1)),
            (Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight), _) => {
                let right = c.key == Key::Named(NamedKey::ArrowRight);
                let fwd = right != self.rtl();
                let shown = self.map.drawn.table.columns.len().max(1);
                if let Some(t) = self.map.table.as_mut() {
                    t.col = if fwd {
                        (col + 1).min(fields.saturating_sub(1))
                    } else {
                        col.saturating_sub(1)
                    };
                    if t.col < t.col_scroll {
                        t.col_scroll = t.col;
                    } else if t.col >= t.col_scroll + shown {
                        t.col_scroll = t.col + 1 - shown;
                    }
                }
            }
            (Key::Named(NamedKey::Space), false) => self.gis_table_select(cursor, true, false),
            (Key::Named(NamedKey::Enter), false) => self.gis_table_select(cursor, false, true),
            (Key::Char('s'), false) if plain => {
                if let Some(t) = self.map.table.as_mut() {
                    t.sort_by(col);
                }
                self.map.table_refresh();
            }
            (Key::Char('f'), true) | (Key::Char('/'), false) => {
                if let Some(t) = self.map.table.as_mut() {
                    t.focus = table::Focus::Search;
                }
            }
            (Key::Char('a'), true) => {
                let li = t.layer;
                let rows: Vec<usize> = t.rows.clone();
                self.map.selected.extend(rows.into_iter().map(|f| (li, f)));
            }
            _ => return false,
        }
        true
    }

    fn gis_layers_key(&mut self, c: &KeyCombo, plain: bool) -> bool {
        use layers::Control;
        let n = self.map.layers.len();
        if n == 0 {
            return false;
        }
        if self.map.confirm_remove {
            if c.key == Key::Named(NamedKey::Enter) {
                self.gis_remove_chosen();
            }
            return true;
        }
        let chosen = self.map.chosen.unwrap_or(n - 1).min(n - 1);
        // The panel lists the top layer first: Down goes down the list.
        match (c.key, c.mods.alt) {
            (Key::Named(NamedKey::ArrowDown), false) => {
                self.map.chosen = Some(chosen.saturating_sub(1));
                self.map.control = None;
            }
            (Key::Named(NamedKey::ArrowUp), false) => {
                self.map.chosen = Some((chosen + 1).min(n - 1));
                self.map.control = None;
            }
            (Key::Named(NamedKey::ArrowUp), true) => self.gis_control(Control::Up),
            (Key::Named(NamedKey::ArrowDown), true) => self.gis_control(Control::Down),
            (Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight), false) => {
                let fwd = (c.key == Key::Named(NamedKey::ArrowRight)) != self.rtl();
                let list = view::controls_for(&self.map.layers[chosen]);
                let at = self
                    .map
                    .control
                    .and_then(|k| list.iter().position(|x| *x == k));
                self.map.control = match (at, fwd) {
                    (None, true) => list.first().copied(),
                    (None, false) => list.last().copied(),
                    (Some(i), true) => list.get(i + 1).copied(),
                    (Some(i), false) => i.checked_sub(1).and_then(|i| list.get(i).copied()),
                };
            }
            (Key::Named(NamedKey::Space | NamedKey::Enter), false) => match self.map.control {
                Some(k) => self.gis_control(k),
                None if c.key == Key::Named(NamedKey::Space) => self.gis_control(Control::Visible),
                None => self.gis_control(Control::Zoom),
            },
            (Key::Char('+' | '='), false) => self.gis_control(Control::OpacityUp),
            (Key::Char('-'), false) => self.gis_control(Control::OpacityDown),
            (Key::Named(NamedKey::Delete), false) => self.gis_control(Control::Remove),
            _ => return false,
        }
        let _ = plain;
        true
    }

    /// `--show map-…` (screenshots): runs that map action, e.g. `map-layers`.
    pub(super) fn gis_show(&mut self, what: &str) -> bool {
        match Action::from_id(what) {
            Some(a) if a.mode() == specular_input::Mode::Map => {
                self.gis_action(a);
                true
            }
            _ => false,
        }
    }

    // ----- Frame -----------------------------------------------------------------------

    /// What the frame draws of the map, before the chrome borrows the text system.
    pub(super) fn gis_view(&self) -> Option<MapView> {
        if !self.gis_on() {
            return None;
        }
        Some(view::build(self))
    }

    /// The GPU part of the frame, when the map view draws layers.
    pub(super) fn gis_frame(&self, pal: &specular_theme::Palette) -> Option<MapFrame> {
        if !self.gis_on() || self.map.layers.is_empty() || !self.cad_has_renderer() {
            return None;
        }
        let (rev, _) = self.map.scene.as_ref()?;
        Some(MapFrame {
            id: self.map.id?,
            key: self.map.rev ^ rev.rotate_left(32),
            dark: self.cad_dark_paper(pal),
            paper: self.map.vector,
        })
    }

    /// The top of the attribute table (logical px), so the readout and scale bar sit above
    /// it.
    pub(super) fn gis_floor(&self, area: (f32, f32, f32, f32)) -> Option<f32> {
        if !self.gis_on() || self.map.table.is_none() {
            return None;
        }
        let ts = self.chrome.text_scale;
        let h =
            table::height(area.3, ts).min(area.3 - 2.0 * specular_theme::metrics::TOOLBAR_MARGIN);
        Some(area.1 + area.3 - specular_theme::metrics::TOOLBAR_MARGIN - h)
    }

    /// A screen reader's action on a map node; true when the node is the map's.
    pub(super) fn gis_a11y_request(
        &mut self,
        req: &specular_widgets::accesskit::ActionRequest,
    ) -> bool {
        let Some(t) = view::a11y_target(req.target_node.0) else {
            return false;
        };
        use specular_widgets::accesskit::Action as A;
        match (t, req.action) {
            (view::Node::Layer(i), A::Click) => {
                if let Some(l) = self.map.layers.get_mut(i) {
                    l.visible = !l.visible;
                }
                self.map.chosen = Some(i);
            }
            (view::Node::Layer(i), A::Focus) => {
                self.map.chosen = Some(i);
                self.map.focus = Some(Focus::Layers);
            }
            (view::Node::Control(c), A::Click) => self.gis_control(c),
            (view::Node::Row(r), A::Click) => self.gis_table_select(r, true, false),
            (view::Node::Row(r), A::Focus) => {
                if let Some(t) = self.map.table.as_mut() {
                    t.cursor = r;
                    t.focus = table::Focus::Rows;
                }
                self.map.focus = Some(Focus::Table);
            }
            (view::Node::Header(c), A::Click) => {
                if let Some(t) = self.map.table.as_mut() {
                    t.col = c;
                    t.sort_by(c);
                }
                self.map.table_refresh();
            }
            (view::Node::Search, A::Focus) => {
                if let Some(t) = self.map.table.as_mut() {
                    t.focus = table::Focus::Search;
                }
                self.map.focus = Some(Focus::Table);
            }
            (view::Node::GoTo, A::Focus) => self.map.focus = Some(Focus::GoTo),
            (view::Node::Add, A::Click) => self.gis_add_dialog(),
            (view::Node::Time | view::Node::TimePlay, _) => self.gis_time_a11y(t, req),
            (
                view::Node::PanelRow(..) | view::Node::PanelButton(..) | view::Node::PanelField,
                _,
            ) => self.gis_panel_a11y(t, req.action),
            _ => {}
        }
        self.map.bump();
        self.redraw = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An input method composes in the map field with the keys (Go to, a service's
    /// address) and nowhere else: moving the keys elsewhere drops the composition.
    #[test]
    fn the_field_with_the_keys_takes_the_composition() {
        let mut m = Map {
            goto: Some(furniture::GoTo::default()),
            focus: Some(Focus::GoTo),
            ..Map::default()
        };
        assert!(m.typing());
        m.set_preedit("東京");
        assert_eq!(m.preedit, "東京");
        m.focus = Some(Focus::Tool);
        assert!(!m.typing());
        m.set_preedit("x");
        assert_eq!(m.preedit, "");
        m.focus = Some(Focus::Services);
        assert!(!m.typing(), "the list page has no field");
        m.services.page = Some(services::Page::Add {
            text: "https://".into(),
            error: None,
        });
        assert!(m.typing());
        m.set_preedit("é");
        assert_eq!(m.preedit, "é");
        // Nothing drawn yet: no window for the input method.
        assert_eq!(m.ime_area(), None);
        m.drawn.ime = Some(specular_gfx::Rect::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(
            m.ime_area(),
            Some(specular_gfx::Rect::new(1.0, 2.0, 3.0, 4.0))
        );
        m.focus = None;
        assert_eq!(m.ime_area(), None);
    }

    #[test]
    fn units_follow_the_settings_then_the_region() {
        use SettingUnits as S;
        assert_eq!(
            units_of(S::Imperial, S::Metric, Some("DE")),
            Units::Imperial
        );
        assert_eq!(units_of(S::Metric, S::Imperial, Some("US")), Units::Metric);
        assert_eq!(units_of(S::Auto, S::Imperial, Some("DE")), Units::Imperial);
        assert_eq!(units_of(S::Auto, S::Auto, Some("us")), Units::Imperial);
        assert_eq!(units_of(S::Auto, S::Auto, Some("GB")), Units::Metric);
        assert_eq!(units_of(S::Auto, S::Auto, None), Units::Metric);
    }

    #[test]
    fn formats_map_to_settings_and_back() {
        use specular_settings::SettingEnum;
        for f in CoordFormat::ALL {
            assert_eq!(format_of(setting_of(f)), f);
            assert_eq!(setting_of(f).as_str(), f.id());
        }
    }
}
