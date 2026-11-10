//! The map view's tools past measuring: their state ([`Tools`]) and the app's side of them
//! (actions, background jobs, keys, the pointer and the readout lines). Each tool's own
//! logic and drawing lives in its module: [`terrain`], [`weather`], [`timebar`],
//! [`photos`], [`services`] and [`export`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_decode::science::NdPlane;
use specular_geodesy::time;
use specular_geodesy::track::Timeline;
use specular_gfx::Rect;
use specular_i18n::t;
use specular_input::{Action, Key, KeyCombo, NamedKey};
use specular_render::Relief;

use super::super::App;
use super::{Focus, Job, export, photos, services, terrain, timebar, weather};
use crate::loader::Copied;

/// The confirm sheet's item for writing geotags.
pub(in crate::app) const GEOTAG_ITEM: u64 = crate::app::items::MAP_GEOTAG;
/// Rows the geotagging preview lists at once.
pub(super) const GEOTAG_ROWS: usize = 8;
/// Seconds within which a second Copy extent copies WKT instead of the bounding box.
const EXTENT_AGAIN_S: f64 = 4.0;

/// The geotagging panel: the photos beside the track and what writing would do.
#[derive(Clone, Debug)]
pub(super) struct Geotag {
    pub plan: Arc<Vec<photos::Planned>>,
    pub scroll: usize,
    /// Writing (after the confirm sheet).
    pub writing: bool,
}

impl Geotag {
    pub(super) fn writes(&self) -> usize {
        self.plan
            .iter()
            .filter(|p| matches!(p.outcome, photos::Outcome::Write { .. }))
            .count()
    }
}

/// What the tools keep for the current map.
#[derive(Default)]
pub(in crate::app) struct Tools {
    /// Contour lines are on (kept across files), and those of the heights shown.
    pub(super) contours_on: bool,
    pub(super) contours: Option<terrain::Contours>,
    pub(super) contour_job: Option<usize>,
    /// Wind arrows are on (kept across files).
    pub(super) wind_on: bool,
    /// Where the wind components are: `None` not looked for yet, `Some(None)` none.
    pub(super) wind_source: Option<Option<weather::WindSource>>,
    pub(super) wind_source_job: bool,
    /// The components at the position shown.
    pub(super) wind: Option<(Vec<u32>, Arc<NdPlane>, Arc<NdPlane>)>,
    pub(super) wind_job: Option<Vec<u32>>,
    /// The weather colours were set for this map once (the field's default).
    pub(super) weather_done: bool,
    pub(super) time: Option<timebar::TimeBar>,
    pub(super) time_drawn: timebar::DrawnBar,
    pub(super) time_drag: bool,
    /// A GPS track with times: the file shown, or the first added track that has them.
    pub(super) track: Option<Arc<Timeline>>,
    pub(super) track_path: Option<PathBuf>,
    pub(super) track_job: bool,
    /// Photos along the track are shown (kept across files).
    pub(super) photos_on: bool,
    pub(super) scan: Option<photos::Scanned>,
    pub(super) scan_job: bool,
    /// The camera clock's offset from GPS time, seconds (camera ahead).
    pub(super) offset: f64,
    pub(super) offset_set: bool,
    pub(super) placed: Option<photos::Placed>,
    pub(super) geotag: Option<Geotag>,
    pub(super) geotag_drawn: services::Drawn,
    pub(super) export: Option<export::Panel>,
    pub(super) export_drawn: services::Drawn,
    /// The elevation model is shown in 3-D.
    pub(in crate::app) terrain_3d: bool,
    /// What exports draw with (made on the first, kept across files).
    pub(super) export_kit: Option<export::Kit>,
    /// The file shown, and whether the tools started on it.
    pub(super) source: Option<PathBuf>,
    pub(super) started: bool,
    /// The last Copy extent: when, so a second press copies WKT.
    pub(super) extent_at: Option<f64>,
}

impl Tools {
    /// What stays when another file is shown: the tools that are on.
    pub(super) fn carry(&mut self) -> Tools {
        Tools {
            contours_on: self.contours_on,
            wind_on: self.wind_on,
            photos_on: self.photos_on,
            export_kit: self.export_kit.take(),
            ..Tools::default()
        }
    }
}

/// The extent as text: a bounding box "west,south,east,north" or a WKT polygon.
pub(super) fn extent_text(e: [f64; 4], wkt: bool) -> String {
    let f = |v: f64| {
        let s = format!("{v:.6}");
        let s = s.trim_end_matches('0').trim_end_matches('.').to_owned();
        if s == "-0" { "0".to_owned() } else { s }
    };
    let [w, s, e2, n] = e;
    if wkt {
        format!(
            "POLYGON(({w} {s}, {e} {s}, {e} {n}, {w} {n}, {w} {s}))",
            w = f(w),
            s = f(s),
            e = f(e2),
            n = f(n)
        )
    } else {
        format!("{},{},{},{}", f(w), f(s), f(e2), f(n))
    }
}

impl App {
    /// The photo scan panicked: it may run again.
    pub(in crate::app) fn map_tools_lost(&mut self, task: &str) {
        if task == "map-photos" {
            self.map.t.scan_job = false;
        }
    }

    // ----- State -------------------------------------------------------------------------

    /// Slope, aspect, contours and 3-D need a ground step (a georeferenced elevation model).
    pub(super) fn gis_relief_ok(&self) -> bool {
        self.map.bands.len() == 1 && self.gis_dem().is_some()
    }

    /// The height plane with its ground step and metres per unit, at the map's centre.
    fn gis_dem(&self) -> Option<(Arc<NdPlane>, [f64; 2], f64)> {
        let (_, p) = self.map.bands.first()?;
        let geo = self.map.geo.as_ref()?;
        let (w, h) = self.map.size;
        let step = terrain::ground_steps(geo.as_ref(), w / 2.0, h / 2.0)?;
        let metres = self
            .science_position()
            .and_then(|(s, i)| super::seam::height_unit_metres(s.unit_at(&i)))
            .unwrap_or(1.0);
        self.map.heights.as_ref()?;
        Some((p.clone(), step, metres))
    }

    /// What the time slider would move through.
    pub(super) fn gis_time_source(&self) -> Option<timebar::Source> {
        if let Some((s, _)) = self.science_position()
            && !self.map.bands.is_empty()
            && let Some(a) = timebar::time_axis(&s)
        {
            return Some(timebar::Source::Grid {
                axis: a,
                len: s.axes[a].len,
                labels: s.axes[a].labels.clone(),
            });
        }
        self.map.t.track.clone().map(timebar::Source::Track)
    }

    /// Each frame: the shown planes into the map (a played step), the slider's play, the
    /// jobs the tools need.
    pub(super) fn gis_tools_sync(&mut self) {
        if !self.map.services.panel_set {
            self.map.services.panel_set = true;
            self.gis_services_settings_panel();
        }
        if !self.gis_on() {
            return;
        }
        self.gis_tools_start();
        // Science planes come in after the map was read (a GeoTIFF elevation model is a map
        // before its values are), and change under it when a time step or level is shown:
        // the heights and bands follow.
        if let Some((_, planes, _)) = self.science_for_map()
            && (planes.len() != self.map.bands.len()
                || !planes
                    .iter()
                    .zip(&self.map.bands)
                    .all(|(a, b)| Arc::ptr_eq(&a.1, &b.1)))
            && let Some((_, _, heights, bands)) = self.gis_sources()
        {
            self.map.heights = heights;
            self.map.bands = bands;
            self.map.bump();
            self.redraw = true;
        }
        let now = self.now();
        // The slider follows the planes stepped by other keys, and plays.
        let index = self.science_position().map(|p| p.1);
        if let Some(b) = self.map.t.time.as_mut()
            && let Some((axis, _)) = b.grid_index()
            && !b.playing
            && let Some(&i) = index.as_ref().and_then(|ix| ix.get(axis))
        {
            b.pos = f64::from(i);
        }
        let settled = self.science_settled();
        let mut step = None;
        if let Some(b) = self.map.t.time.as_mut()
            && b.tick(now, settled)
        {
            step = b.grid_index();
            self.map.bump();
            self.redraw = true;
        }
        if let Some((axis, i)) = step {
            self.science_step(axis, Some(i), 0);
        }
        if let Some(due) = self.map.t.time.as_ref().and_then(timebar::TimeBar::due) {
            let at = std::time::Instant::now()
                + std::time::Duration::from_secs_f64((due - now).clamp(0.0, 2.0));
            self.wake_by(at, crate::app::Cause::Science);
        }
        self.gis_tools_jobs();
    }

    /// Starts the background work the tools that are on need.
    fn gis_tools_jobs(&mut self) {
        let Some(id) = self.map.id else { return };
        // Contours of the heights shown.
        if self.map.t.contours_on
            && let Some((_, p)) = self.map.bands.first()
            && self.map.heights.is_some()
        {
            let key = Arc::as_ptr(p) as usize;
            let have = self
                .map
                .t
                .contours
                .as_ref()
                .is_some_and(|c| Arc::ptr_eq(&c.plane, p));
            if !have && self.map.t.contour_job != Some(key) {
                self.map.t.contour_job = Some(key);
                let (p, size) = (p.clone(), self.map.size);
                let unit = self
                    .science_position()
                    .and_then(|(s, i)| s.unit_at(&i).map(str::to_owned))
                    .map(|u| match super::seam::height_unit_metres(Some(&u)) {
                        Some(m) if (m - 1.0).abs() < 1e-9 => "m".to_owned(),
                        Some(_) if u.to_ascii_lowercase().contains('f') => "ft".to_owned(),
                        _ => u,
                    })
                    .unwrap_or_else(|| "m".to_owned());
                let tx = self.map.channel();
                self.loader.background("map-contours", move |l| {
                    let t0 = std::time::Instant::now();
                    if let Some(c) = terrain::trace(&p, size, &unit) {
                        specular_log::debug!(target: specular_log::UI, "map contours: {} lines in {:.1} ms", c.lines.len(), t0.elapsed().as_secs_f64() * 1e3);
                        let _ = tx.send(Job::Contours(id, c));
                    }
                    l.waker()();
                });
            }
        }
        // The wind's components: where they are, then the planes at the position shown.
        let position = self.science_position();
        if self.map.t.wind_source.is_none()
            && !self.map.t.wind_source_job
            && let Some((s, index)) = position.clone()
            && let Some(session) = self.images.get(&id).and_then(|i| i.session.clone())
        {
            self.map.t.wind_source_job = true;
            let tx = self.map.channel();
            self.loader.background("map-wind", move |l| {
                let w = weather::find_wind(&session, &s, &index);
                let _ = tx.send(Job::WindSource(id, w));
                l.waker()();
            });
        }
        if self.map.t.wind_on
            && let Some(Some(src)) = self.map.t.wind_source.clone()
            && let Some((s, index)) = position
        {
            // The components at the position shown (a GRIB grid's parameter stays theirs).
            let pa = s.axes.iter().position(|a| a.name == "parameter");
            let mut want = index.clone();
            if let Some(a) = pa {
                want[a] = 0;
            }
            let have = self.map.t.wind.as_ref().is_some_and(|w| w.0 == want);
            if !have && self.map.t.wind_job.as_ref() != Some(&want) {
                self.map.t.wind_job = Some(want.clone());
                let mut src = src;
                for (k, v) in index.iter().enumerate() {
                    if Some(k) != pa {
                        src.u.1[k] = *v;
                        src.v.1[k] = *v;
                    }
                }
                let tx = self.map.channel();
                self.loader.background("map-wind", move |l| {
                    if let Some((u, v)) = weather::read_wind(&src) {
                        let _ = tx.send(Job::Wind(id, want, u, v));
                    }
                    l.waker()();
                });
            }
        }
        // Photos beside the track.
        if self.map.t.photos_on || self.map.t.geotag.is_some() {
            self.gis_scan_photos();
        }
    }

    /// When the map is first checked: the file it shows (its track is read once the map
    /// view is on).
    pub(super) fn gis_tools_checked(&mut self, session: &Arc<dyn specular_decode::Session>) {
        self.map.t.source = Some(session.source().path().to_path_buf());
    }

    /// Once the map view is on for the file: its GPS track's times, and a weather grid's
    /// colours (unless the file or the user chose colours).
    fn gis_tools_start(&mut self) {
        if self.map.t.started || !self.map.checked {
            return;
        }
        self.map.t.started = true;
        if let Some(p) = self.map.t.source.clone() {
            self.gis_track_from(&p);
        }
        if !self.map.t.weather_done
            && let Some((s, index)) = self.science_position()
            && self.science_weather().is_none()
            && !self.science_user_lut()
            && s.luts.is_empty()
            && let Some(f) = weather::field(&s, &index)
        {
            self.map.t.weather_done = true;
            self.science_set_weather(Some(f.maps()[0]));
        }
    }

    /// Reads the times of a track file (the file shown or added), unless a track is known.
    pub(super) fn gis_track_from(&mut self, path: &Path) {
        if self.map.t.track.is_some() || self.map.t.track_job || !photos::is_track_file(path) {
            return;
        }
        let Some(id) = self.map.id else { return };
        self.map.t.track_job = true;
        self.map.t.track_path = Some(path.to_path_buf());
        let (p, tx) = (path.to_path_buf(), self.map.channel());
        self.loader.background("map-track", move |l| {
            let t = std::fs::read(&p)
                .ok()
                .and_then(|b| photos::timeline(&b))
                .map(Arc::new);
            let _ = tx.send(Job::Track(id, t));
            l.waker()();
        });
    }

    /// Reads the photos beside the track (once per map).
    fn gis_scan_photos(&mut self) {
        if self.map.t.scan.is_some() || self.map.t.scan_job {
            return;
        }
        let (Some(id), Some(dir)) = (
            self.map.id,
            self.map
                .t
                .track_path
                .as_ref()
                .and_then(|p| p.parent())
                .map(Path::to_path_buf),
        ) else {
            return;
        };
        self.map.t.scan_job = true;
        let tx = self.map.channel();
        self.loader.background("map-photos", move |l| {
            let v = Arc::new(photos::scan(&dir));
            let _ = tx.send(Job::Photos(id, v));
            l.waker()();
        });
    }

    /// A finished job of the tools.
    pub(super) fn gis_tools_job(&mut self, j: Job) {
        let cur = self.map.id;
        match j {
            Job::Contours(id, c) if Some(id) == cur => {
                self.map.t.contour_job = None;
                self.map.t.contours = Some(c);
            }
            Job::WindSource(id, w) if Some(id) == cur => {
                self.map.t.wind_source_job = false;
                self.map.t.wind_source = Some(w);
            }
            Job::Wind(id, at, u, v) if Some(id) == cur => {
                self.map.t.wind_job = None;
                self.map.t.wind = Some((at, u, v));
            }
            Job::Track(id, t) if Some(id) == cur => {
                self.map.t.track_job = false;
                if t.is_none() {
                    self.map.t.track_path = None;
                }
                self.map.t.track = t;
            }
            Job::Photos(id, v) if Some(id) == cur => {
                self.map.t.scan_job = false;
                self.map.t.scan = Some(v);
                if !self.map.t.offset_set
                    && let Some(t) = self.map.t.track.clone()
                    && let Some(s) = &self.map.t.scan
                {
                    self.map.t.offset = photos::guess_offset(&t, s);
                }
                self.gis_replan();
            }
            Job::Geotagged(id, n, failed) if Some(id) == cur => {
                let now = self.now();
                let msg = if failed.is_empty() {
                    t!("map-geotag-done", count = n as i64)
                } else {
                    t!(
                        "map-geotag-done-failed",
                        count = n as i64,
                        failed = failed.len() as i64,
                        reason = failed[0].1.clone()
                    )
                };
                self.chrome.toast(msg, None, now);
                // The photos have positions now: read them again.
                self.map.t.scan = None;
                self.map.t.geotag = None;
                if self.map.focus == Some(Focus::Geotag) {
                    self.map.focus = None;
                }
            }
            _ => return,
        }
        self.map.bump();
        self.redraw = true;
    }

    /// The photo positions and the geotagging preview for the offset now.
    fn gis_replan(&mut self) {
        let (Some(t), Some(s)) = (self.map.t.track.clone(), self.map.t.scan.clone()) else {
            return;
        };
        let plan = photos::plan(&t, &s, self.map.t.offset);
        self.map.t.placed = Some(photos::pins(&plan, &s));
        if let Some(g) = self.map.t.geotag.as_mut() {
            g.plan = Arc::new(plan);
        }
        self.map.bump();
        self.redraw = true;
    }

    // ----- Actions -----------------------------------------------------------------------

    /// The tools' actions (from [`App::gis_action`]).
    pub(super) fn gis_tool_action(&mut self, a: Action) {
        let now = self.now();
        match a {
            Action::MapSlope | Action::MapAspect => {
                let kind = if a == Action::MapSlope {
                    Relief::Slope
                } else {
                    Relief::Aspect
                };
                let msg = match self.science_set_relief(kind) {
                    Some(Some(Relief::Slope)) => t!("map-slope-on"),
                    Some(Some(Relief::Aspect)) => t!("map-aspect-on"),
                    Some(_) => t!("map-relief-off"),
                    None => t!("disabled-no-heights"),
                };
                self.chrome.toast(msg, None, now);
            }
            Action::MapContours => {
                self.map.t.contours_on = !self.map.t.contours_on;
                let msg = if self.map.t.contours_on {
                    t!("map-contours-on")
                } else {
                    t!("map-contours-off")
                };
                self.chrome.toast(msg, None, now);
            }
            Action::MapTerrain3d => self.gis_terrain_3d(),
            Action::MapTime => {
                if self.map.t.time.is_some() {
                    self.map.t.time = None;
                    if self.map.focus == Some(Focus::Time) {
                        self.map.focus = None;
                    }
                } else {
                    self.gis_time_open();
                }
            }
            Action::MapPlay => {
                if self.map.t.time.is_none() {
                    self.gis_time_open();
                }
                if let Some(b) = self.map.t.time.as_mut() {
                    b.playing = !b.playing;
                    b.next = now;
                }
            }
            Action::MapWind => {
                self.map.t.wind_on = !self.map.t.wind_on;
                let msg = if self.map.t.wind_on {
                    t!("map-wind-on")
                } else {
                    t!("map-wind-off")
                };
                self.chrome.toast(msg, None, now);
            }
            Action::MapWeatherColours => self.gis_weather_next(),
            Action::MapServices => self.gis_services_toggle(),
            Action::MapPhotosOnTrack => {
                self.map.t.photos_on = !self.map.t.photos_on;
                let msg = if self.map.t.photos_on {
                    t!("map-photos-on")
                } else {
                    t!("map-photos-off")
                };
                self.chrome.toast(msg, None, now);
            }
            Action::MapGeotag => {
                if self.map.t.geotag.is_some() {
                    self.map.t.geotag = None;
                    self.map.focus = None;
                } else {
                    self.map.t.geotag = Some(Geotag {
                        plan: Arc::new(Vec::new()),
                        scroll: 0,
                        writing: false,
                    });
                    self.map.focus = Some(Focus::Geotag);
                    self.gis_scan_photos();
                    self.gis_replan();
                }
            }
            Action::MapExport | Action::MapPrint => {
                let print = a == Action::MapPrint;
                if self.map.t.export.as_ref().is_some_and(|e| e.print == print) {
                    self.map.t.export = None;
                    self.map.focus = None;
                } else {
                    let scale = self.gis_current_scale();
                    self.map.t.export = Some(export::Panel::new(print, scale));
                    self.map.focus = Some(Focus::Export);
                }
            }
            Action::MapCopyExtent => self.gis_copy_extent(),
            _ => {}
        }
    }

    fn gis_time_open(&mut self) {
        let Some(src) = self.gis_time_source() else {
            return;
        };
        let bar = match src {
            timebar::Source::Grid { axis, len, labels } => {
                let at = self
                    .science_position()
                    .and_then(|(_, i)| i.get(axis).copied())
                    .unwrap_or(0);
                timebar::TimeBar::grid(axis, len, labels, at)
            }
            timebar::Source::Track(t) => timebar::TimeBar::track(t),
        };
        self.map.t.time = Some(bar);
        self.map.focus = Some(Focus::Time);
    }

    /// The next weather colour map for the grid shown (after the last, the file's own).
    fn gis_weather_next(&mut self) {
        let now = self.now();
        let field = self
            .science_position()
            .and_then(|(s, i)| weather::field(&s, &i));
        let list: &[specular_inspect::ColourMap] = match field {
            Some(f) => f.maps(),
            None => weather::GENERAL,
        };
        let cur = self.science_weather();
        let next = match cur.and_then(|c| list.iter().position(|m| *m == c)) {
            None if cur.is_none() => list.first().copied(),
            None => None,
            Some(i) => list.get(i + 1).copied(),
        };
        self.map.t.weather_done = true;
        self.science_set_weather(next);
        let msg = match next {
            Some(m) => t!("map-weather-map", name = m.name()),
            None => t!("map-weather-file"),
        };
        self.chrome.toast(msg, None, now);
    }

    /// Copies the area on screen: the bounding box, or WKT when pressed again soon after.
    fn gis_copy_extent(&mut self) {
        let now = self.now();
        let Some(e) = self.gis_view_extent() else {
            return;
        };
        let wkt = self
            .map
            .t
            .extent_at
            .is_some_and(|at| now - at < EXTENT_AGAIN_S);
        let text = extent_text(e, wkt);
        self.map.t.extent_at = (!wkt).then_some(now);
        self.map.copied = Some(text.clone());
        self.clipboard
            .copy_text(&self.loader, text, Copied::Coordinate);
        let msg = if wkt {
            t!("map-extent-copied-wkt")
        } else {
            t!("map-extent-copied")
        };
        self.chrome.toast(msg, None, now);
    }

    /// The area on screen in degrees: west, south, east, north (the map's part of it).
    pub(super) fn gis_view_extent(&self) -> Option<[f64; 4]> {
        let vp = self.view.viewport;
        let (w, h) = self.map.size;
        let f = self.view.stored_from_screen();
        let mut e: Option<[f64; 4]> = None;
        // Along the edges, for curved projections.
        for i in 0..=8 {
            let t = i as f32 / 8.0;
            for p in [
                (vp.x + vp.w * t, vp.y),
                (vp.x + vp.w * t, vp.y + vp.h),
                (vp.x, vp.y + vp.h * t),
                (vp.x + vp.w, vp.y + vp.h * t),
            ] {
                let (x, y) = f.apply(p);
                let s = (f64::from(x).clamp(0.0, w), f64::from(y).clamp(0.0, h));
                if let Some((lat, lon)) = self.gis_latlon(s) {
                    e = Some(e.map_or([lon, lat, lon, lat], |e| {
                        [e[0].min(lon), e[1].min(lat), e[2].max(lon), e[3].max(lat)]
                    }));
                }
            }
        }
        e
    }

    /// The map's scale on screen now (1 : this), at a 96-dpi screen.
    pub(super) fn gis_current_scale(&self) -> f64 {
        let mpp = self.gis_metres_per_px().unwrap_or(1.0);
        mpp / (0.0254 / 96.0)
    }

    /// The elevation model in the 3-D viewer, its colours draped over it; again: back.
    fn gis_terrain_3d(&mut self) {
        let now = self.now();
        let Some((p, step, metres)) = self.gis_dem() else {
            specular_log::debug!(target: specular_log::UI, "map 3d: no heights (bands {}, heights {}, geo {})", self.map.bands.len(), self.map.heights.is_some(), self.map.geo.is_some());
            self.chrome.toast(t!("disabled-no-heights"), None, now);
            return;
        };
        let size = self.map.size;
        let d = terrain::dem(&p, size, step, metres);
        let ex = terrain::auto_exaggeration(&d);
        let drape = match self.science_colourer() {
            Some(c) => terrain::drape(d.w, d.h, c, self.map.t.contours.as_ref()),
            None => terrain::drape(d.w, d.h, |_, _| None, None),
        };
        let title = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let Some(model) = terrain::surface(&d, ex, drape, title) else {
            self.chrome.toast(t!("disabled-no-heights"), None, now);
            return;
        };
        self.map.t.terrain_3d = true;
        self.mesh_show_terrain(model);
        let msg = if ex > 1.0 {
            t!(
                "map-terrain-3d-on-exaggerated",
                times = specular_i18n::fixed(ex, 0)
            )
        } else {
            t!("map-terrain-3d-on")
        };
        self.chrome.toast(msg, None, now);
    }

    /// Leaves the 3-D terrain (the map view comes back).
    pub(in crate::app) fn gis_terrain_closed(&mut self) {
        self.map.t.terrain_3d = false;
        self.map.bump();
        self.redraw = true;
    }

    // ----- Readout -----------------------------------------------------------------------

    /// The readout's lines of the tools at stored (`x`, `y`): slope and aspect, wind, the
    /// track's time.
    pub(super) fn gis_tool_lines(&self, x: f64, y: f64) -> Vec<String> {
        let mut v = Vec::new();
        if let Some((p, step, metres)) = self.gis_dem()
            && let Some(geo) = self.map.geo.as_ref()
        {
            // The ground step where the pointer is.
            let step = terrain::ground_steps(geo.as_ref(), x, y).unwrap_or(step);
            let d = terrain::dem(&p, self.map.size, step, metres);
            v.extend(terrain::slope_line(&d, self.map.size, x, y));
        }
        if self.map.t.wind_on
            && let Some((_, u, w)) = &self.map.t.wind
            && let Some((uu, vv)) = weather::wind_at(u, w, self.map.size, (x, y))
        {
            v.push(weather::wind_line(uu, vv, "m/s"));
        }
        v
    }

    // ----- Keys --------------------------------------------------------------------------

    pub(super) fn gis_time_key(&mut self, c: &KeyCombo) -> bool {
        let rtl = self.rtl();
        let now = self.now();
        let Some(b) = self.map.t.time.as_mut() else {
            return false;
        };
        let fwd = if rtl { -1.0 } else { 1.0 };
        match c.key {
            Key::Named(NamedKey::Space | NamedKey::Enter) => {
                b.playing = !b.playing;
                b.next = now;
            }
            Key::Named(NamedKey::ArrowRight) => b.step(fwd, c.mods.shift),
            Key::Named(NamedKey::ArrowLeft) => b.step(-fwd, c.mods.shift),
            Key::Named(NamedKey::Home) => b.pos = b.range().0,
            Key::Named(NamedKey::End) => b.pos = b.range().1,
            Key::Char('+' | '=') => b.speed *= 2.0,
            Key::Char('-') => b.speed = (b.speed / 2.0).max(1.0),
            _ => return false,
        }
        let step = b.grid_index();
        if let Some((axis, i)) = step {
            self.science_step(axis, Some(i), 0);
        }
        true
    }

    pub(super) fn gis_geotag_key(&mut self, c: &KeyCombo) -> bool {
        let rtl = self.rtl();
        let Some(g) = self.map.t.geotag.as_mut() else {
            return false;
        };
        if g.writing {
            return true;
        }
        let unit = if c.mods.shift {
            3600.0
        } else if c.mods.alt {
            1.0
        } else {
            60.0
        };
        let fwd = if rtl { -1.0 } else { 1.0 };
        let n = g.plan.len();
        match c.key {
            Key::Named(NamedKey::ArrowRight) => {
                self.map.t.offset += fwd * unit;
                self.map.t.offset_set = true;
                self.gis_replan();
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.map.t.offset -= fwd * unit;
                self.map.t.offset_set = true;
                self.gis_replan();
            }
            Key::Named(NamedKey::ArrowDown) => {
                g.scroll = (g.scroll + 1).min(n.saturating_sub(GEOTAG_ROWS))
            }
            Key::Named(NamedKey::ArrowUp) => g.scroll = g.scroll.saturating_sub(1),
            Key::Named(NamedKey::PageDown) => {
                g.scroll = (g.scroll + GEOTAG_ROWS).min(n.saturating_sub(GEOTAG_ROWS))
            }
            Key::Named(NamedKey::PageUp) => g.scroll = g.scroll.saturating_sub(GEOTAG_ROWS),
            Key::Char('g') if !c.mods.ctrl => self.gis_geotag_guess(),
            Key::Char('0') if !c.mods.ctrl => {
                self.map.t.offset = 0.0;
                self.map.t.offset_set = true;
                self.gis_replan();
            }
            Key::Named(NamedKey::Enter) => self.gis_geotag_ask(),
            _ => return false,
        }
        true
    }

    /// Guesses the camera clock's offset again (whole hours).
    pub(super) fn gis_geotag_guess(&mut self) {
        if let (Some(t), Some(s)) = (self.map.t.track.clone(), self.map.t.scan.clone()) {
            self.map.t.offset = photos::guess_offset(&t, &s);
            self.map.t.offset_set = true;
            self.gis_replan();
        }
    }

    /// Asks before writing the sidecars (this is a write to the user's folder).
    pub(super) fn gis_geotag_ask(&mut self) {
        let Some(g) = self.map.t.geotag.as_ref() else {
            return;
        };
        let n = g.writes();
        if n == 0 {
            let now = self.now();
            self.chrome.toast(t!("map-geotag-nothing"), None, now);
            return;
        }
        self.chrome.confirm(
            t!("map-geotag-confirm", count = n as i64),
            t!("map-geotag-confirm-ok"),
            GEOTAG_ITEM,
        );
        self.redraw = true;
    }

    /// The writing was confirmed: the sidecars are written off the UI thread.
    pub(in crate::app) fn gis_geotag_confirmed(&mut self) {
        let Some(id) = self.map.id else { return };
        let Some(g) = self.map.t.geotag.as_mut() else {
            return;
        };
        g.writing = true;
        let plan = g.plan.clone();
        let tx = self.map.channel();
        self.loader.background("map-geotag", move |l| {
            let (n, failed) = photos::write(&plan);
            let _ = tx.send(Job::Geotagged(id, n, failed));
            l.waker()();
        });
        self.redraw = true;
    }

    // ----- Pointer -----------------------------------------------------------------------

    /// A press on a tool's panel; true when taken.
    pub(super) fn gis_tools_press(&mut self, lp: (f32, f32)) -> bool {
        let d = self.map.t.time_drawn;
        if let Some(r) = d.play
            && r.contains(lp)
        {
            let now = self.now();
            if let Some(b) = self.map.t.time.as_mut() {
                b.playing = !b.playing;
                b.next = now;
            }
            self.map.focus = Some(Focus::Time);
            return true;
        }
        if let Some(r) = d.track
            && r.contains(lp)
        {
            self.map.t.time_drag = true;
            self.map.focus = Some(Focus::Time);
            self.gis_time_drag(lp, r);
            return true;
        }
        if d.panel.is_some_and(|r| r.contains(lp)) {
            self.map.focus = Some(Focus::Time);
            return true;
        }
        if self.gis_services_press(lp) {
            return true;
        }
        let g = self.map.t.geotag_drawn.clone();
        if g.rect.is_some_and(|r| r.contains(lp)) {
            self.map.focus = Some(Focus::Geotag);
            if let Some((h, _)) = g.hits.iter().find(|(_, r)| r.contains(lp)) {
                match h {
                    services::Hit::Button(services::Button::Write) => self.gis_geotag_ask(),
                    services::Hit::Button(services::Button::Guess) => self.gis_geotag_guess(),
                    services::Hit::Button(services::Button::Close) => {
                        self.map.t.geotag = None;
                        self.map.focus = None;
                    }
                    _ => {}
                }
            }
            return true;
        }
        let e = self.map.t.export_drawn.clone();
        if e.rect.is_some_and(|r| r.contains(lp)) {
            self.map.focus = Some(Focus::Export);
            if let Some((h, _)) = e.hits.iter().find(|(_, r)| r.contains(lp)) {
                self.gis_export_hit(*h);
            }
            return true;
        }
        false
    }

    fn gis_time_drag(&mut self, lp: (f32, f32), track: Rect) {
        let rtl = self.rtl();
        let Some(b) = self.map.t.time.as_mut() else {
            return;
        };
        b.pos = b.at_fraction(timebar::fraction_at(track, lp.0, rtl));
        let step = b.grid_index();
        if let Some((axis, i)) = step {
            self.science_step(axis, Some(i), 0);
        }
        self.map.bump();
        self.redraw = true;
    }

    pub(super) fn gis_tools_motion(&mut self, lp: (f32, f32)) {
        if self.map.t.time_drag
            && let Some(r) = self.map.t.time_drawn.track
        {
            self.gis_time_drag(lp, r);
        }
    }

    /// The button's release ends a drag of the slider; true when it did.
    pub(super) fn gis_tools_release(&mut self, _lp: (f32, f32)) -> bool {
        std::mem::take(&mut self.map.t.time_drag)
    }

    /// A screen reader's action on the time slider or its play button.
    pub(super) fn gis_time_a11y(
        &mut self,
        n: super::view::Node,
        req: &specular_widgets::accesskit::ActionRequest,
    ) {
        use specular_widgets::accesskit::{Action as A, ActionData};
        let now = self.now();
        let Some(b) = self.map.t.time.as_mut() else {
            return;
        };
        match (n, req.action) {
            (super::view::Node::TimePlay, A::Click) => {
                b.playing = !b.playing;
                b.next = now;
            }
            (_, A::Increment) => b.step(1.0, false),
            (_, A::Decrement) => b.step(-1.0, false),
            (_, A::SetValue) => {
                if let Some(ActionData::NumericValue(v)) = &req.data {
                    let (lo, hi) = b.range();
                    b.pos = v.clamp(lo, hi);
                    if b.grid_index().is_some() {
                        b.pos = b.pos.round();
                    }
                }
            }
            (_, A::Focus) => self.map.focus = Some(Focus::Time),
            _ => {}
        }
        let step = self
            .map
            .t
            .time
            .as_ref()
            .and_then(timebar::TimeBar::grid_index);
        if let Some((axis, i)) = step {
            self.science_step(axis, Some(i), 0);
        }
    }

    /// A screen reader's action on a row, button or field of the services, geotagging or
    /// export panel.
    pub(super) fn gis_panel_a11y(
        &mut self,
        n: super::view::Node,
        a: specular_widgets::accesskit::Action,
    ) {
        use super::view::{Node, PANELS};
        use specular_widgets::accesskit::Action as A;
        if !matches!(a, A::Click | A::Focus) {
            return;
        }
        let (k, which) = match n {
            Node::PanelRow(k, i) => (k, Some(Ok(i))),
            Node::PanelButton(k, i) => (k, Some(Err(i))),
            Node::PanelField => (0, None),
            _ => return,
        };
        let Some(&panel) = PANELS.get(k) else { return };
        self.map.focus = Some(panel);
        let buttons = match panel {
            Focus::Services => self
                .map
                .services
                .view(true)
                .map(|v| v.buttons)
                .unwrap_or_default(),
            Focus::Geotag => vec![
                services::Button::Write,
                services::Button::Guess,
                services::Button::Close,
            ],
            _ => self
                .map
                .t
                .export
                .as_ref()
                .map(|p| p.view(None, true).buttons)
                .unwrap_or_default(),
        };
        match (panel, which) {
            (Focus::Services, Some(Ok(i))) => {
                self.map.services.cursor = i;
                if a == A::Click {
                    self.gis_services_activate();
                }
            }
            (Focus::Services, Some(Err(i))) => {
                if let Some(b) = buttons.get(i) {
                    self.gis_services_button(*b);
                }
            }
            (Focus::Geotag, Some(Err(i))) => match buttons.get(i) {
                Some(services::Button::Write) => self.gis_geotag_ask(),
                Some(services::Button::Guess) => self.gis_geotag_guess(),
                Some(_) => {
                    self.map.t.geotag = None;
                    self.map.focus = None;
                }
                None => {}
            },
            (Focus::Export, Some(Ok(i))) => self.gis_export_hit(services::Hit::Row(i)),
            (Focus::Export, Some(Err(i))) => {
                if let Some(b) = buttons.get(i) {
                    self.gis_export_hit(services::Hit::Button(*b));
                }
            }
            _ => {}
        }
        self.map.bump();
        self.redraw = true;
    }

    /// The marker of the time slider on a track (lat, lon).
    pub(super) fn gis_time_marker(&self) -> Option<((f64, f64), String)> {
        let b = self.map.t.time.as_ref()?;
        let at = b.track_at()?;
        Some((at, time::format(b.pos)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extents_as_text() {
        let e = [8.5, 47.25, 8.75, 47.5];
        assert_eq!(extent_text(e, false), "8.5,47.25,8.75,47.5");
        assert_eq!(
            extent_text(e, true),
            "POLYGON((8.5 47.25, 8.75 47.25, 8.75 47.5, 8.5 47.5, 8.5 47.25))"
        );
        assert_eq!(extent_text([-0.0000001, 0.0, 1.0, 2.0], false), "0,0,1,2");
    }
}
