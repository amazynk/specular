//! What the map view draws each frame: the GPU layers ([`render`]), the overlays and panels
//! ([`draw`]) from a [`MapView`] built before the chrome borrows the text system, and their
//! AccessKit nodes ([`a11y`]).

use specular_decode::drawing::paper_colour;
use specular_decode::drawing::specular_vector::draw::{LayerStyle, View as SheetView};
use specular_decode::drawing::specular_vector::kurbo;
use specular_decode::geo::specular_gis::model::{Geometry, Pt};
use specular_geodesy::profile::Profile;
use specular_geodesy::units::Units;
use specular_gfx::{DrawList, Gpu, Rect, Text, wgpu};
use specular_i18n::t;
use specular_render::View;
use specular_theme::Palette;
use specular_theme::metrics::TOOLBAR_MARGIN;
use specular_widgets::accesskit;

use super::super::App;
use super::{
    Focus, Identify, Map, furniture, layers, measure, photos, services, table, terrain, timebar,
    tools, weather,
};
use crate::loader::ImageId;

/// AccessKit ids (cad 0x5A…, measure 0x5C…, mesh 0x5E…).
const A11Y_ID: u64 = 0x5D00_0000_0000_0000;
const A11Y_PANEL: u64 = A11Y_ID + 1;
const A11Y_ADD: u64 = A11Y_ID + 2;
const A11Y_TABLE: u64 = A11Y_ID + 3;
const A11Y_SEARCH: u64 = A11Y_ID + 4;
const A11Y_GOTO: u64 = A11Y_ID + 5;
const A11Y_TOOL: u64 = A11Y_ID + 6;
const A11Y_CHART: u64 = A11Y_ID + 7;
const A11Y_SCALE: u64 = A11Y_ID + 8;
const A11Y_NORTH: u64 = A11Y_ID + 9;
const A11Y_OVERVIEW: u64 = A11Y_ID + 10;
const A11Y_IDENTIFY: u64 = A11Y_ID + 11;
const A11Y_TIME: u64 = A11Y_ID + 12;
const A11Y_TIME_PLAY: u64 = A11Y_ID + 13;
const A11Y_SERVICES: u64 = A11Y_ID + 14;
const A11Y_GEOTAG: u64 = A11Y_ID + 15;
const A11Y_EXPORT: u64 = A11Y_ID + 16;
const A11Y_LEGEND: u64 = A11Y_ID + 17;
const A11Y_PANEL_ROW: u64 = A11Y_ID + 0x200;
const A11Y_PANEL_BUTTON: u64 = A11Y_ID + 0x800;
const A11Y_PANEL_FIELD: u64 = A11Y_ID + 0xC00;
const A11Y_CONTROL: u64 = A11Y_ID + 0x100;
const A11Y_LAYER: u64 = A11Y_ID + 0x1000;
const A11Y_HEADER: u64 = A11Y_ID + 0x10_0000;
const A11Y_ROW: u64 = A11Y_ID + 0x100_0000;
const A11Y_END: u64 = A11Y_ID + 0x1000_0000;
/// Vertices of selected features drawn as highlights, at most.
const HIGHLIGHT_MAX: usize = 50_000;

/// A map node a screen reader acts on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Node {
    Layer(usize),
    Control(layers::Control),
    Header(usize),
    Row(usize),
    Search,
    GoTo,
    Add,
    /// The time slider (its value is set by a screen reader), and its play button.
    Time,
    TimePlay,
    /// A row, a button or the field of the services, geotagging or export panel.
    PanelRow(usize, usize),
    PanelButton(usize, usize),
    PanelField,
    /// A panel or label: nothing to do.
    Other,
}

/// The panels whose rows and buttons screen readers act on, by index.
pub(super) const PANELS: [Focus; 3] = [Focus::Services, Focus::Geotag, Focus::Export];

pub(super) fn a11y_target(id: u64) -> Option<Node> {
    if !(A11Y_ID..A11Y_END).contains(&id) {
        return None;
    }
    Some(match id {
        A11Y_ADD => Node::Add,
        A11Y_SEARCH => Node::Search,
        A11Y_GOTO => Node::GoTo,
        A11Y_TIME => Node::Time,
        A11Y_TIME_PLAY => Node::TimePlay,
        _ if (A11Y_PANEL_FIELD..A11Y_PANEL_FIELD + 0x100).contains(&id) => Node::PanelField,
        _ if (A11Y_PANEL_BUTTON..A11Y_PANEL_FIELD).contains(&id) => {
            let k = (id - A11Y_PANEL_BUTTON) as usize;
            Node::PanelButton(k / 0x100, k % 0x100)
        }
        _ if (A11Y_PANEL_ROW..A11Y_PANEL_BUTTON).contains(&id) => {
            let k = (id - A11Y_PANEL_ROW) as usize;
            Node::PanelRow(k / 0x200, k % 0x200)
        }
        _ if id >= A11Y_ROW => Node::Row((id - A11Y_ROW) as usize),
        _ if id >= A11Y_HEADER => Node::Header((id - A11Y_HEADER) as usize),
        _ if id >= A11Y_LAYER => Node::Layer((id - A11Y_LAYER) as usize),
        _ if id >= A11Y_CONTROL => {
            Node::Control(*layers::Control::ALL.get((id - A11Y_CONTROL) as usize)?)
        }
        // The panels themselves and their labels.
        _ => Node::Other,
    })
}

/// What the frame needs for the GPU layers ([`App::map_frame`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct MapFrame {
    pub id: ImageId,
    /// Changes whenever the GPU picture would.
    pub key: u64,
    pub dark: bool,
    /// Draw the paper under the layers (a vector map), else over the image.
    pub paper: bool,
}

/// The panels as drawn, for hit tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct Drawn {
    pub(super) panel: layers::DrawnPanel,
    pub(super) table: table::DrawnTable,
    pub(super) chart: measure::DrawnChart,
    pub(super) goto: Option<Rect>,
    pub(super) tool: Option<Rect>,
    pub(super) identify: Option<Rect>,
    pub(super) overview: Option<Rect>,
    pub(super) north: Option<Rect>,
    pub(super) time: timebar::DrawnBar,
    pub(super) services: services::Drawn,
    pub(super) geotag: services::Drawn,
    pub(super) export: services::Drawn,
    pub(super) legend: Option<Rect>,
    /// Where the input method's window opens for the field with the keys.
    pub(super) ime: Option<Rect>,
}

/// The measuring tool on screen.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ToolView {
    pub title: String,
    pub lines: Vec<String>,
    /// The geodesics through the points (and to the pointer), logical px.
    pub path: Vec<(f32, f32)>,
    pub closed: bool,
    pub points: Vec<(f32, f32)>,
    pub cross: Option<(f32, f32)>,
    pub focused: bool,
}

/// Points on screen, logical px.
type Polyline = Vec<(f32, f32)>;

/// Everything derived for this frame's overlays.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct MapView {
    pub(super) panel: Option<layers::Panel>,
    pub(super) overview: Option<furniture::Overview>,
    pub(super) north: Option<f64>,
    pub(super) tool: Option<ToolView>,
    pub(super) chart: Option<(Profile, Option<usize>, Units)>,
    /// The chart's cursor on the map.
    pub(super) chart_point: Option<(f32, f32)>,
    pub(super) goto: Option<furniture::GoTo>,
    pub(super) identify: Option<Identify>,
    /// Selected features: lines (logical px) and points.
    pub(super) highlight: (Vec<Polyline>, Polyline),
    pub(super) pin: Option<(f32, f32)>,
    pub(super) table_name: String,
    pub(super) scale: Option<String>,
    /// Contour lines and their labels on screen.
    pub(super) contours: Option<terrain::ContourView>,
    /// Wind arrows on screen.
    pub(super) arrows: Vec<weather::Arrow>,
    /// The time slider, and whether it has the keys.
    pub(super) time: Option<(timebar::TimeBar, bool)>,
    /// Photos along the track: where (logical px) and their camera direction.
    pub(super) pins: Vec<((f32, f32), Option<f64>)>,
    /// The track's position at the slider's time, and the time.
    pub(super) track_marker: Option<((f32, f32), String)>,
    pub(super) services: Option<services::View>,
    pub(super) geotag: Option<services::View>,
    pub(super) export: Option<services::View>,
    pub(super) legend: Option<terrain::Legend>,
}

/// The chosen layer's controls (Remove for an added one).
pub(super) fn controls_for(l: &layers::MapLayer) -> Vec<layers::Control> {
    layers::Control::ALL
        .into_iter()
        .filter(|c| *c != layers::Control::Remove || l.added.is_some())
        .collect()
}

/// A feature's lines and points on screen.
fn outline(
    app: &App,
    l: &layers::MapLayer,
    g: &Geometry,
    lines: &mut Vec<Vec<(f32, f32)>>,
    points: &mut Vec<(f32, f32)>,
    budget: &mut usize,
) {
    let geo = match app.map.geo.as_ref() {
        Some(g) => g.clone(),
        None => return,
    };
    let to = |p: Pt| {
        l.coords
            .to_stored(p, geo.as_ref())
            .map(|s| app.gis_screen_of_stored(s))
    };
    let mut line = |pts: &[Pt], close: bool, lines: &mut Vec<Vec<(f32, f32)>>| {
        if *budget < pts.len() {
            return;
        }
        *budget -= pts.len();
        let mut v: Vec<(f32, f32)> = pts.iter().filter_map(|p| to(*p)).collect();
        if close && let Some(f) = v.first().copied() {
            v.push(f);
        }
        lines.push(v);
    };
    match g {
        Geometry::Point(p) => points.extend(to(*p)),
        Geometry::MultiPoint(v) => points.extend(v.iter().filter_map(|p| to(*p))),
        Geometry::LineString(v) => line(v, false, lines),
        Geometry::MultiLineString(v) => v.iter().for_each(|l| line(l, false, lines)),
        Geometry::Polygon(r) => r.iter().for_each(|r| line(r, true, lines)),
        Geometry::MultiPolygon(ps) => ps.iter().flatten().for_each(|r| line(r, true, lines)),
        Geometry::Collection(gs) => {
            for g in gs {
                outline(app, l, g, lines, points, budget);
            }
        }
    }
}

/// Builds the frame's view of the map.
pub(super) fn build(app: &App) -> MapView {
    let m = &app.map;
    let units = app.gis_units();
    let panel = m.panel.then(|| {
        let n = m.layers.len();
        let rows = (0..n)
            .rev()
            .map(|i| {
                let l = &m.layers[i];
                layers::Row {
                    index: i,
                    name: l.shown_name(),
                    colour: l.ink(),
                    visible: l.visible,
                    opacity: l.opacity,
                    features: l.feature_count(),
                    kind: l.kinds().first().copied(),
                    added: l.added.is_some(),
                    chosen: m.chosen == Some(i),
                    focused: m.focus == Some(Focus::Layers)
                        && m.chosen == Some(i)
                        && m.control.is_none(),
                }
            })
            .collect();
        let chosen = m.chosen.and_then(|i| m.layers.get(i));
        layers::Panel {
            rows,
            controls: chosen.map(controls_for).unwrap_or_default(),
            focused_control: m.control.filter(|_| m.focus == Some(Focus::Layers)),
            confirm: m
                .confirm_remove
                .then(|| chosen.map(|l| l.shown_name()))
                .flatten(),
            scroll: m.panel_scroll,
            adding: m.adding > 0,
        }
    });
    // The overview: the map's outline and the part on screen, in degrees.
    let overview = m.overview.then(|| {
        let (w, h) = m.size;
        let ring_of = |pts: &[(f64, f64)]| -> Vec<(f64, f64)> {
            pts.iter()
                .filter_map(|&s| app.gis_latlon(s).map(|(lat, lon)| (lon, lat)))
                .collect()
        };
        let edge = |k: usize| {
            let mut v = Vec::new();
            for i in 0..k {
                let t = i as f64 / k as f64;
                v.push((w * t, 0.0));
            }
            for i in 0..k {
                let t = i as f64 / k as f64;
                v.push((w, h * t));
            }
            for i in 0..k {
                let t = i as f64 / k as f64;
                v.push((w * (1.0 - t), h));
            }
            for i in 0..k {
                let t = i as f64 / k as f64;
                v.push((0.0, h * (1.0 - t)));
            }
            v
        };
        let extent = ring_of(&edge(4));
        let vp = app.view.viewport;
        let f = app.view.stored_from_screen();
        let corners: Vec<(f64, f64)> = [
            (vp.x, vp.y),
            (vp.x + vp.w, vp.y),
            (vp.x + vp.w, vp.y + vp.h),
            (vp.x, vp.y + vp.h),
        ]
        .iter()
        .map(|p| {
            let (x, y) = f.apply(*p);
            (f64::from(x).clamp(0.0, w), f64::from(y).clamp(0.0, h))
        })
        .collect();
        let visible = ring_of(&corners);
        let window = furniture::window_around(&extent).unwrap_or([-180.0, -90.0, 180.0, 90.0]);
        furniture::Overview {
            window,
            extent,
            visible,
        }
    });
    let tool = m.tool.as_ref().map(|t| {
        let hover = m.hover.filter(|_| !t.done);
        let lines = measure::result_lines(t, hover, units);
        let mut pts = t.pts.clone();
        pts.extend(hover);
        let closed = t.kind == measure::Kind::Area && pts.len() > 2;
        let mpp = app.gis_metres_per_px().unwrap_or(1.0);
        let path = measure::densify(&pts, closed, mpp * 6.0)
            .into_iter()
            .filter_map(|p| app.gis_screen_of(p))
            .collect();
        let title = match t.kind {
            measure::Kind::Distance => t!("palette-map-measure-distance"),
            measure::Kind::Area => t!("palette-map-measure-area"),
            measure::Kind::Profile => t!("palette-map-elevation-profile"),
        };
        ToolView {
            title,
            lines,
            path,
            closed,
            points: t.pts.iter().filter_map(|p| app.gis_screen_of(*p)).collect(),
            cross: t.cross.map(|s| app.gis_screen_of_stored(s)),
            focused: m.focus == Some(Focus::Tool),
        }
    });
    let chart = m.profile.clone().map(|p| (p, m.profile_cursor, units));
    let chart_point = m
        .profile
        .as_ref()
        .zip(m.profile_cursor)
        .and_then(|(p, i)| p.samples.get(i))
        .and_then(|s| app.gis_screen_of((s.lat, s.lon)));
    let mut lines = Vec::new();
    let mut points = Vec::new();
    let mut budget = HIGHLIGHT_MAX;
    for &(li, f) in &m.selected {
        let Some(l) = m.layers.get(li).filter(|l| l.visible) else {
            continue;
        };
        if let Some(g) = l.data.layers[l.layer]
            .features
            .get(f)
            .and_then(|f| f.geometry.as_ref())
        {
            outline(app, l, g, &mut lines, &mut points, &mut budget);
        }
    }
    let table_name = m
        .table
        .as_ref()
        .and_then(|t| m.layers.get(t.layer))
        .map(|l| l.shown_name())
        .unwrap_or_default();
    // The tools' overlays, in logical px over the viewport.
    let ls = f64::from(app.dl.scale.max(0.1));
    let vp = app.view.viewport;
    let clip = Rect::new(
        vp.x / app.dl.scale.max(0.1),
        vp.y / app.dl.scale.max(0.1),
        vp.w / app.dl.scale.max(0.1),
        vp.h / app.dl.scale.max(0.1),
    );
    let contours =
        m.t.contours
            .as_ref()
            .filter(|_| m.t.contours_on)
            .map(|c| terrain::view_of(c, |s| app.gis_screen_of_stored(s), clip));
    let arrows = match (&m.t.wind, m.geo.clone()) {
        (Some((_, u, v)), Some(geo)) if m.t.wind_on => {
            let inv = app.view.stored_from_screen();
            let size = m.size;
            weather::arrows(
                u,
                v,
                size,
                geo.as_ref(),
                clip,
                |q| {
                    let (x, y) = inv.apply((q.0 * ls as f32, q.1 * ls as f32));
                    let s = (f64::from(x), f64::from(y));
                    (s.0 >= 0.0 && s.1 >= 0.0 && s.0 <= size.0 && s.1 <= size.1).then_some(s)
                },
                |s| app.gis_screen_of_stored(s),
                weather::ARROW_SPACING,
            )
        }
        _ => Vec::new(),
    };
    let pins = match &m.t.placed {
        Some(p) if m.t.photos_on => p
            .iter()
            .filter_map(|(_, lat, lon, d)| app.gis_screen_of((*lat, *lon)).map(|q| (q, *d)))
            .collect(),
        _ => Vec::new(),
    };
    let track_marker = app
        .gis_time_marker()
        .and_then(|(ll, t)| app.gis_screen_of(ll).map(|q| (q, t)));
    let focus = m.focus;
    let geotag =
        m.t.geotag
            .as_ref()
            .map(|g| geotag_view(m, g, focus == Some(Focus::Geotag)));
    let export = m.t.export.as_ref().map(|p| {
        let l = app.gis_export_layout().and_then(Result::ok);
        p.view(l, focus == Some(Focus::Export))
    });
    let legend = app
        .science_relief()
        .and_then(|h| terrain::relief_legend(h.kind));
    MapView {
        panel,
        overview,
        north: app.gis_north(),
        tool,
        chart,
        chart_point,
        goto: m.goto.clone(),
        identify: m.identify.clone(),
        highlight: (lines, points),
        pin: m.pin.and_then(|(ll, _)| app.gis_screen_of(ll)),
        table_name,
        scale: app.gis_scale_bar().map(|b| b.label),
        contours,
        arrows,
        time: m.t.time.clone().map(|b| (b, focus == Some(Focus::Time))),
        pins,
        track_marker,
        services: m
            .services
            .open
            .then(|| m.services.view(focus == Some(Focus::Services)))
            .flatten(),
        geotag,
        export,
        legend,
    }
}

/// The geotagging panel's view.
pub(super) fn geotag_view(m: &Map, g: &tools::Geotag, focused: bool) -> services::View {
    let mut lines = vec![t!(
        "map-geotag-offset",
        offset = specular_i18n::isolate_ltr(&specular_geodesy::time::format_offset(m.t.offset))
    )];
    if m.t.scan.is_none() {
        lines.push(t!("map-geotag-reading"));
    } else {
        lines.push(t!(
            "map-geotag-summary",
            count = g.writes() as i64,
            total = g.plan.len() as i64
        ));
    }
    lines.push(t!("map-geotag-sidecars"));
    if g.writing {
        lines.push(t!("map-geotag-writing"));
    }
    let rows = g
        .plan
        .iter()
        .map(|p| services::Row {
            title: p
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            sub: photos::outcome_text(p),
            enabled: matches!(p.outcome, photos::Outcome::Write { .. }),
        })
        .collect();
    services::View {
        title: t!("map-geotag-title"),
        lines,
        field: None,
        rows,
        cursor: usize::MAX,
        scroll: g.scroll,
        buttons: vec![
            services::Button::Write,
            services::Button::Guess,
            services::Button::Close,
        ],
        focused,
    }
}

/// A photo's pin, with its camera direction as a wedge when known (`dir`: degrees
/// clockwise from north; drawn as if north were up).
pub(super) fn draw_photo_pin(dl: &mut DrawList, p: (f32, f32), dir: Option<f64>, ts: f32) {
    let ink = specular_theme::Color::rgba(0.15, 0.55, 0.95, 1.0);
    let halo = specular_theme::Color::rgba(0.0, 0.0, 0.0, 0.55);
    if let Some(d) = dir {
        let len = 16.0 * ts;
        for off in [-22.0f64, 22.0] {
            let a = (d + off).to_radians();
            let q = (p.0 + len * a.sin() as f32, p.1 - len * a.cos() as f32);
            dl.line(p, q, 3.0 * ts, halo);
            dl.line(p, q, 1.5 * ts, ink);
        }
    }
    dl.circle(p, 5.5 * ts, halo);
    dl.circle(p, 4.0 * ts, ink);
    dl.circle(p, 1.6 * ts, specular_theme::Color::rgba(1.0, 1.0, 1.0, 1.0));
}

/// Draws the overlays and panels; returns where they are.
#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    map: &Map,
    v: &MapView,
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) -> Drawn {
    let mut d = Drawn::default();
    let clip = Rect::new(area.0, area.1, area.2, area.3);
    // On the map: the selection, the measuring path, the chart's cursor and the Go to pin.
    dl.push_clip(clip);
    let hl = table::highlight(pal);
    for l in &v.highlight.0 {
        for w in l.windows(2) {
            dl.line(w[0], w[1], 4.0 * ts, hl.with_alpha(0.35));
            dl.line(w[0], w[1], 1.6 * ts, hl);
        }
    }
    for p in &v.highlight.1 {
        dl.circle(*p, 6.0 * ts, hl.with_alpha(0.35));
        dl.circle(*p, 3.5 * ts, hl);
    }
    if let Some(t) = &v.tool {
        let ink = specular_theme::Color::rgba(1.0, 0.85, 0.1, 1.0);
        for w in t.path.windows(2) {
            dl.line(
                w[0],
                w[1],
                3.5 * ts,
                specular_theme::Color::rgba(0.0, 0.0, 0.0, 0.55),
            );
            dl.line(w[0], w[1], 2.0 * ts, ink);
        }
        for p in &t.points {
            dl.circle(
                *p,
                4.5 * ts,
                specular_theme::Color::rgba(0.0, 0.0, 0.0, 0.6),
            );
            dl.circle(*p, 3.0 * ts, ink);
        }
        if let Some((x, y)) = t.cross {
            let r = 9.0 * ts;
            dl.line((x - r, y), (x + r, y), 1.5 * ts, ink);
            dl.line((x, y - r), (x, y + r), 1.5 * ts, ink);
        }
    }
    if let Some(p) = v.chart_point {
        dl.circle(p, 5.0 * ts, pal.text);
        dl.circle(p, 3.0 * ts, pal.accent);
    }
    if let Some((x, y)) = v.pin {
        dl.circle((x, y), 7.0 * ts, pal.danger.with_alpha(0.35));
        dl.circle((x, y), 3.5 * ts, pal.danger);
    }
    dl.pop_clip();
    // The tools' overlays: contours, wind, photos, the track's position in time.
    if let Some(c) = &v.contours {
        terrain::draw_contours(dl, text, pal, c, clip, ts);
    }
    if !v.arrows.is_empty() {
        weather::draw_arrows(dl, &v.arrows, clip, ts);
    }
    dl.push_clip(clip);
    // North is up for the pins' wedges: turned with the map when it is turned.
    let turn = v.north.unwrap_or(0.0).to_degrees();
    for (p, d) in &v.pins {
        draw_photo_pin(dl, *p, d.map(|d| d + turn), ts);
    }
    if let Some((p, when)) = &v.track_marker {
        dl.circle(*p, 8.0 * ts, pal.accent.with_alpha(0.35));
        dl.circle(*p, 4.5 * ts, pal.accent);
        let b = text.layout(
            when,
            specular_gfx::TextStyle::new(specular_theme::metrics::FONT_SMALL * ts).tabular(),
            None,
            specular_gfx::Align::Start,
        );
        let r = Rect::new(
            p.0 + 10.0 * ts,
            p.1 - b.height / 2.0 - 2.0 * ts,
            b.width + 8.0 * ts,
            b.height + 4.0 * ts,
        );
        dl.rect(r, 3.0 * ts, pal.glass);
        text.draw(dl, &b, r.x + 4.0 * ts, r.y + 2.0 * ts, pal.text);
    }
    dl.pop_clip();
    // Top start: the layer panel.
    if let Some(p) = &v.panel {
        d.panel = layers::draw_panel(dl, text, pal, p, area, rtl, ts);
    }
    if let Some(l) = &v.legend {
        let top = d
            .panel
            .rect
            .map_or(area.1 + TOOLBAR_MARGIN, |r| r.y + r.h + 8.0 * ts);
        d.legend = Some(terrain::draw_legend(dl, text, pal, l, area, top, rtl, ts));
    }
    // Top end: the overview, the north arrow under it, then identify.
    let mut end_y = area.1 + TOOLBAR_MARGIN;
    if let Some(o) = &v.overview {
        let r = furniture::draw_overview(dl, pal, o, area, rtl, ts);
        end_y = r.y + r.h + 8.0 * ts;
        d.overview = Some(r);
    }
    if let Some(a) = v.north {
        let r = furniture::draw_north(dl, text, pal, a, area, d.overview, rtl, ts);
        end_y = r.y + r.h + 8.0 * ts;
        d.north = Some(r);
    }
    if let Some(i) = &v.identify {
        d.identify = Some(furniture::draw_identify(
            dl, text, pal, &i.title, &i.rows, i.more, area, end_y, rtl, ts,
        ));
    }
    // Top centre: Go to, then the tool's result.
    let mut top = area.1 + TOOLBAR_MARGIN;
    // The composition goes to the field with the keys.
    let preedit = |f: Focus| {
        if map.focus == Some(f) {
            map.preedit.as_str()
        } else {
            ""
        }
    };
    if let Some(g) = &v.goto {
        let mut lines = vec![t!("map-goto-hint")];
        lines.extend(g.error.clone());
        let (r, ime) = furniture::draw_box(
            dl,
            text,
            pal,
            &t!("palette-map-go-to-coordinate"),
            Some((&g.text, preedit(Focus::GoTo))),
            &lines,
            area,
            top,
            rtl,
            ts,
        );
        top = r.y + r.h + 8.0 * ts;
        d.goto = Some(r);
        if map.focus == Some(Focus::GoTo) {
            d.ime = ime;
        }
    }
    if let Some(t) = &v.tool {
        let (r, _) =
            furniture::draw_box(dl, text, pal, &t.title, None, &t.lines, area, top, rtl, ts);
        if t.focused {
            dl.stroke_rect(r, specular_theme::metrics::RADIUS_MD, pal.accent, 1.5 * ts);
        }
        d.tool = Some(r);
        top = r.y + r.h + 8.0 * ts;
    }
    for (pv, slot, pre) in [
        (&v.services, &mut d.services, preedit(Focus::Services)),
        (&v.geotag, &mut d.geotag, ""),
        (&v.export, &mut d.export, ""),
    ] {
        if let Some(pv) = pv {
            *slot = services::draw(dl, text, pal, pv, pre, area, top, rtl, ts);
            if let Some(r) = slot.rect {
                top = r.y + r.h + 8.0 * ts;
            }
        }
    }
    // Bottom: the table, and the chart above it.
    let mut floor = None;
    if let Some(t) = &map.table
        && let Some(l) = map.layers.get(t.layer)
    {
        let sel = map.selected_in(t.layer);
        let tv = table::TableView {
            table: t,
            layer: &l.data.layers[l.layer],
            name: v.table_name.clone(),
            selected: &sel,
            focused: map.focus == Some(Focus::Table),
            preedit: preedit(Focus::Table),
        };
        d.table = table::draw(dl, text, pal, &tv, area, rtl, ts);
        floor = d.table.rect.map(|r| r.y);
    }
    if let Some((b, focused)) = &v.time {
        d.time = timebar::draw(dl, text, pal, b, *focused, area, floor, rtl, ts);
        floor = d.time.panel.map(|r| r.y).or(floor);
    }
    if let Some((p, cursor, u)) = &v.chart {
        d.chart = measure::draw_chart(dl, text, pal, p, *cursor, *u, area, floor, rtl, ts);
        if map.focus == Some(Focus::Chart)
            && let Some(r) = d.chart.panel
        {
            dl.stroke_rect(r, specular_theme::metrics::RADIUS_MD, pal.accent, 1.5 * ts);
        }
    }
    match map.focus {
        Some(Focus::Table) => d.ime = d.ime.or(d.table.ime),
        Some(Focus::Services) => d.ime = d.ime.or(d.services.ime),
        _ => {}
    }
    d
}

/// The AccessKit nodes of the map's panels.
pub(in crate::app) fn a11y(tree: &mut accesskit::TreeUpdate, map: &Map, v: &MapView, sf: f64) {
    use accesskit::{Node as N, NodeId, Role, Toggled};
    let d = &map.drawn;
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * sf,
        y0: f64::from(r.y) * sf,
        x1: f64::from(r.x + r.w) * sf,
        y1: f64::from(r.y + r.h) * sf,
    };
    let mut root = N::new(Role::Group);
    root.set_label(t!("keys-mode-map"));
    let mut nodes: Vec<(NodeId, N)> = Vec::new();
    // The layer panel: a list of check boxes, the chosen layer's buttons, Add.
    if let (Some(p), Some(r)) = (&v.panel, d.panel.rect) {
        let mut g = N::new(Role::List);
        g.set_label(t!(
            "map-layers-title",
            shown = p.rows.iter().filter(|r| r.visible).count() as i64,
            total = p.rows.len() as i64
        ));
        g.set_bounds(bounds(r));
        for row in &p.rows {
            let Some(rr) = d
                .panel
                .hits
                .iter()
                .find_map(|(h, r)| (*h == layers::PanelHit::Row(row.index)).then_some(*r))
            else {
                continue;
            };
            let id = NodeId(A11Y_LAYER + row.index as u64);
            let mut n = N::new(Role::CheckBox);
            n.set_label(layers::row_text(row));
            n.set_toggled(if row.visible {
                Toggled::True
            } else {
                Toggled::False
            });
            n.set_bounds(bounds(rr));
            if row.chosen {
                n.set_selected(true);
            }
            n.add_action(accesskit::Action::Click);
            n.add_action(accesskit::Action::Focus);
            g.push_child(id);
            nodes.push((id, n));
        }
        for (h, rr) in &d.panel.hits {
            let (id, label) = match h {
                layers::PanelHit::Control(c) => {
                    let k = layers::Control::ALL
                        .iter()
                        .position(|x| x == c)
                        .unwrap_or(0);
                    (NodeId(A11Y_CONTROL + k as u64), c.label())
                }
                layers::PanelHit::Add => (NodeId(A11Y_ADD), t!("palette-map-add-layer")),
                layers::PanelHit::ConfirmRemove => {
                    (NodeId(A11Y_CONTROL + 0x80), t!("map-layer-remove"))
                }
                layers::PanelHit::ConfirmKeep => {
                    (NodeId(A11Y_CONTROL + 0x81), t!("map-layer-keep"))
                }
                _ => continue,
            };
            let mut n = N::new(Role::Button);
            n.set_label(label);
            n.set_bounds(bounds(*rr));
            n.add_action(accesskit::Action::Click);
            g.push_child(id);
            nodes.push((id, n));
        }
        if let Some(q) = &p.confirm {
            let id = NodeId(A11Y_CONTROL + 0x82);
            let mut n = N::new(Role::Label);
            n.set_label(t!("map-layer-remove-ask", name = q.clone()));
            g.push_child(id);
            nodes.push((id, n));
        }
        root.push_child(NodeId(A11Y_PANEL));
        nodes.push((NodeId(A11Y_PANEL), g));
    }
    // The table: its search field, the column headers and the rows shown.
    if let (Some(t), Some(r)) = (&map.table, d.table.rect)
        && let Some(l) = map.layers.get(t.layer)
    {
        let layer = &l.data.layers[l.layer];
        let mut g = N::new(Role::Table);
        g.set_label(t!(
            "map-table-title",
            layer = v.table_name.clone(),
            shown = t.rows.len() as i64,
            total = layer.features.len() as i64,
            selected = map.selected_in(t.layer).len() as i64
        ));
        g.set_bounds(bounds(r));
        let mut search = N::new(Role::SearchInput);
        search.set_label(t!("map-table-search"));
        search.set_value(t.search.clone());
        if let Some(sr) = d
            .table
            .hits
            .iter()
            .find_map(|(h, r)| (*h == table::TableHit::Search).then_some(*r))
        {
            search.set_bounds(bounds(sr));
        }
        search.add_action(accesskit::Action::Focus);
        g.push_child(NodeId(A11Y_SEARCH));
        nodes.push((NodeId(A11Y_SEARCH), search));
        for (c, cr) in &d.table.columns {
            let id = NodeId(A11Y_HEADER + *c as u64);
            let mut n = N::new(Role::ColumnHeader);
            n.set_label(layer.fields()[*c].clone());
            n.set_bounds(bounds(*cr));
            if let Some((s, desc)) = t.sort
                && s == *c
            {
                n.set_sort_direction(if desc {
                    accesskit::SortDirection::Descending
                } else {
                    accesskit::SortDirection::Ascending
                });
            }
            n.add_action(accesskit::Action::Click);
            g.push_child(id);
            nodes.push((id, n));
        }
        let sel = map.selected_in(t.layer);
        for (h, rr) in &d.table.hits {
            let table::TableHit::Row(i) = h else { continue };
            let Some(&f) = t.rows.get(*i) else { continue };
            let id = NodeId(A11Y_ROW + *i as u64);
            if id.0 >= A11Y_END {
                continue;
            }
            let mut n = N::new(Role::Row);
            n.set_label(table::row_label(layer, f));
            n.set_selected(sel.contains(&f));
            n.set_bounds(bounds(*rr));
            n.add_action(accesskit::Action::Click);
            n.add_action(accesskit::Action::Focus);
            g.push_child(id);
            nodes.push((id, n));
        }
        if let Some(name) = layer.fields().get(t.col) {
            let id = NodeId(A11Y_TABLE + 0x10);
            let mut n = N::new(Role::Label);
            n.set_label(table::stats_line(
                name,
                &table::stats(layer, t.col, &t.rows),
            ));
            g.push_child(id);
            nodes.push((id, n));
        }
        root.push_child(NodeId(A11Y_TABLE));
        nodes.push((NodeId(A11Y_TABLE), g));
    }
    let label_node = |id: u64,
                      role: Role,
                      label: String,
                      r: Option<Rect>,
                      root: &mut N,
                      nodes: &mut Vec<(NodeId, N)>| {
        let mut n = N::new(role);
        n.set_label(label);
        if let Some(r) = r {
            n.set_bounds(bounds(r));
        }
        root.push_child(NodeId(id));
        nodes.push((NodeId(id), n));
    };
    if let Some(g) = &v.goto {
        let mut n = N::new(Role::TextInput);
        n.set_label(t!("palette-map-go-to-coordinate"));
        n.set_value(g.text.clone());
        if let Some(e) = &g.error {
            n.set_description(e.clone());
        }
        if let Some(r) = d.goto {
            n.set_bounds(bounds(r));
        }
        n.add_action(accesskit::Action::Focus);
        root.push_child(NodeId(A11Y_GOTO));
        nodes.push((NodeId(A11Y_GOTO), n));
    }
    if let Some(t) = &v.tool {
        let mut lines = vec![t.title.clone()];
        lines.extend(t.lines.iter().cloned());
        label_node(
            A11Y_TOOL,
            Role::Status,
            lines.join(". "),
            d.tool,
            &mut root,
            &mut nodes,
        );
    }
    if let Some((p, cursor, u)) = &v.chart {
        let mut s = measure::profile_summary(p, *u);
        if let Some(c) = cursor.and_then(|i| measure::cursor_text(p, i, *u)) {
            s = format!("{s}. {c}");
        }
        label_node(
            A11Y_CHART,
            Role::Figure,
            format!("{}: {s}", t!("palette-map-elevation-profile")),
            d.chart.panel,
            &mut root,
            &mut nodes,
        );
    }
    if let Some(s) = &v.scale {
        label_node(
            A11Y_SCALE,
            Role::Label,
            t!("map-scale-label", length = s.clone()),
            None,
            &mut root,
            &mut nodes,
        );
    }
    if let Some(a) = v.north {
        let deg = (a.to_degrees().rem_euclid(360.0)).round() as i64;
        label_node(
            A11Y_NORTH,
            Role::Image,
            t!("map-north-label", degrees = deg),
            d.north,
            &mut root,
            &mut nodes,
        );
    }
    if v.overview.is_some() {
        label_node(
            A11Y_OVERVIEW,
            Role::Image,
            t!("palette-map-overview"),
            d.overview,
            &mut root,
            &mut nodes,
        );
    }
    if let Some(i) = &v.identify {
        let mut s = vec![i.title.clone()];
        s.extend(i.rows.iter().map(|(k, v)| format!("{k}: {v}")));
        label_node(
            A11Y_IDENTIFY,
            Role::Group,
            s.join(", "),
            d.identify,
            &mut root,
            &mut nodes,
        );
    }
    if let (Some((b, _)), Some(r)) = (&v.time, d.time.panel) {
        let mut n = N::new(Role::Slider);
        n.set_label(t!("palette-map-time"));
        n.set_value(b.label());
        let (lo, hi) = b.range();
        n.set_min_numeric_value(lo);
        n.set_max_numeric_value(hi);
        n.set_numeric_value(b.pos);
        n.set_bounds(bounds(r));
        n.add_action(accesskit::Action::Increment);
        n.add_action(accesskit::Action::Decrement);
        n.add_action(accesskit::Action::SetValue);
        n.add_action(accesskit::Action::Focus);
        root.push_child(NodeId(A11Y_TIME));
        nodes.push((NodeId(A11Y_TIME), n));
        if let Some(pr) = d.time.play {
            let mut n = N::new(Role::Button);
            n.set_label(if b.playing {
                t!("map-time-pause")
            } else {
                t!("map-time-play")
            });
            n.set_bounds(bounds(pr));
            n.add_action(accesskit::Action::Click);
            root.push_child(NodeId(A11Y_TIME_PLAY));
            nodes.push((NodeId(A11Y_TIME_PLAY), n));
        }
    }
    for (k, (pv, pd, id)) in [
        (&v.services, &d.services, A11Y_SERVICES),
        (&v.geotag, &d.geotag, A11Y_GEOTAG),
        (&v.export, &d.export, A11Y_EXPORT),
    ]
    .into_iter()
    .enumerate()
    {
        let (Some(pv), Some(r)) = (pv, pd.rect) else {
            continue;
        };
        let mut g = N::new(Role::Group);
        g.set_label(pv.title.clone());
        g.set_description(pv.lines.join(" "));
        g.set_bounds(bounds(r));
        // The buttons are drawn from the row's end; a screen reader reads them in their own
        // order, which is the reading order (after the rows and the field above them).
        let mut hits: Vec<_> = pd.hits.iter().collect();
        hits.sort_by_key(|(h, _)| match h {
            services::Hit::Button(b) => (1, pv.buttons.iter().position(|x| x == b)),
            _ => (0, None),
        });
        for (h, hr) in hits {
            let (nid, mut n) = match h {
                services::Hit::Row(i) => {
                    let Some(row) = pv.rows.get(*i) else { continue };
                    let mut n = N::new(Role::ListItem);
                    n.set_label(format!("{}: {}", row.title, row.sub));
                    if *i == pv.cursor {
                        n.set_selected(true);
                    }
                    if row.enabled {
                        n.add_action(accesskit::Action::Click);
                    } else {
                        n.set_disabled();
                    }
                    (
                        NodeId(A11Y_PANEL_ROW + (k * 0x200 + (*i).min(0x1ff)) as u64),
                        n,
                    )
                }
                services::Hit::Button(b) => {
                    let i = pv.buttons.iter().position(|x| x == b).unwrap_or(0);
                    let mut n = N::new(Role::Button);
                    n.set_label(b.label());
                    n.add_action(accesskit::Action::Click);
                    (NodeId(A11Y_PANEL_BUTTON + (k * 0x100 + i) as u64), n)
                }
                services::Hit::Field => {
                    let mut n = N::new(Role::TextInput);
                    n.set_label(pv.title.clone());
                    n.set_value(pv.field.clone().unwrap_or_default());
                    n.add_action(accesskit::Action::Focus);
                    (NodeId(A11Y_PANEL_FIELD + k as u64), n)
                }
            };
            n.set_bounds(bounds(*hr));
            g.push_child(nid);
            nodes.push((nid, n));
        }
        root.push_child(NodeId(id));
        nodes.push((NodeId(id), g));
    }
    if let (Some(l), Some(r)) = (&v.legend, d.legend) {
        let ticks: Vec<String> = l.ticks.iter().map(|t| t.1.clone()).collect();
        label_node(
            A11Y_LEGEND,
            Role::Figure,
            format!("{}: {}", l.title, ticks.join(", ")),
            Some(r),
            &mut root,
            &mut nodes,
        );
    }
    if nodes.is_empty() {
        return;
    }
    if let Some((_, r)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        r.push_child(NodeId(A11Y_ID));
    }
    tree.nodes.push((NodeId(A11Y_ID), root));
    tree.nodes.extend(nodes);
}

/// Draws the map's layers over the scene target with the panel's styles.
#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn render(
    map: &mut Map,
    renderer: &mut specular_render::vector::VectorRenderer,
    gpu: &Gpu,
    enc: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    size: (u32, u32),
    view: &View,
    frame: &MapFrame,
    dpi_scale: f64,
) {
    let Some((_, prepared)) = map.scene.clone() else {
        return;
    };
    let Some(sheet) = prepared.sheets.first().cloned() else {
        return;
    };
    let styles: Vec<LayerStyle> = map.layers.iter().map(layers::MapLayer::style).collect();
    let visible: Vec<bool> = map.layers.iter().map(|l| l.visible).collect();
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
        transform: screen,
        size,
        dark: frame.dark,
        background: paper_colour(frame.dark),
        lineweights: true,
        dpi_scale,
        layers: &visible,
    };
    match renderer.render_styled(gpu, &sheet, &v, &styles, paper, frame.paper) {
        Ok(stats) => {
            renderer.blit(enc, target);
            specular_log::debug!(
                target: specular_log::RENDER,
                "map frame: {} drawn, {} culled; encode {:.2} ms",
                stats.walk.drawn,
                stats.walk.culled,
                stats.encode_ms,
            );
        }
        Err(e) => {
            specular_log::warn!(target: specular_log::RENDER, "map layers not drawn: {e}");
        }
    }
}
