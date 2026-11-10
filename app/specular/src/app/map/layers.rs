//! The map's layers: the file's own and those added from other files, in drawing order
//! (the first at the bottom), each with its visibility, opacity and simple style; the scene
//! the GPU draws them from (every layer in stored pixels of the current image); hit tests;
//! and the layer panel.
//!
//! Colours come from the file where it has them (KML styles, GeoJSON simplestyle), else
//! from the layer palette; a colour, line width or point size chosen in the panel applies
//! over the file's. Visibility, opacity and the chosen style change the drawing without
//! building the scene again ([`LayerStyle`]); adding, removing and reordering layers
//! rebuild it.

use std::path::PathBuf;
use std::sync::Arc;

use specular_decode::drawing::specular_vector::draw::LayerStyle;
use specular_decode::drawing::specular_vector::kurbo::{BezPath, Point, Rect as KRect};
use specular_decode::drawing::specular_vector::{
    Color as VColor, FontSpec, Generated, Item, Layer as VLayer, Shape, Sheet, SheetKind, Units,
    VectorScene,
};
use specular_decode::geo::specular_gis::crs::{Crs, CrsKind};
use specular_decode::geo::specular_gis::geotiff::Affine2;
use specular_decode::geo::specular_gis::model::{Dataset, GeomClass, Geometry, Pt};
use specular_decode::geo::specular_gis::scene::layer_colors;
use specular_decode::geo::specular_gis::style::{FeatureStyle, simplestyle};
use specular_geodesy::seam::GeoTransform;
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, RADIUS_SM, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette};

/// Line widths and point radii the panel steps through, logical px.
pub(super) const WIDTHS: [f32; 4] = [1.0, 1.5, 2.5, 4.0];
pub(super) const POINTS: [f32; 4] = [2.0, 3.0, 4.5, 7.0];
/// Opacity steps of the panel's − and + (and the keys).
pub(super) const OPACITY_STEP: f32 = 0.1;
/// Fills are drawn at this alpha unless the file says otherwise.
const FILL_ALPHA: u8 = 0x60;
/// Lineweights in hundredths of a millimetre for lines and outlines (as `specular-gis`).
const LINE_WEIGHT: i16 = 35;
const OUTLINE_WEIGHT: i16 = 18;
/// Panel width, logical px.
pub(super) const PANEL_W: f32 = 290.0;
/// Characters of a layer name before it is cut.
const NAME_CHARS: usize = 30;

/// How a layer's coordinates become the current image's stored pixels.
#[derive(Clone)]
pub(super) enum Coords {
    /// The image's own features: an affine (dataset → stored).
    Stored(Affine2),
    /// Longitude and latitude in degrees.
    LonLat,
    /// Another CRS, through longitude and latitude.
    Crs(Box<Crs>),
}

impl Coords {
    /// For a dataset added over the map: its CRS, or degrees when it has none (GeoJSON,
    /// GPX and KML are in degrees by definition).
    pub(super) fn of(crs: Option<&Crs>) -> Coords {
        match crs {
            Some(c) if c.kind != CrsKind::Geographic => Coords::Crs(Box::new(c.clone())),
            _ => Coords::LonLat,
        }
    }

    /// A dataset position in stored pixels.
    pub(super) fn to_stored(&self, p: Pt, geo: &dyn GeoTransform) -> Option<(f64, f64)> {
        match self {
            Coords::Stored(a) => Some(a.apply(p[0], p[1])),
            Coords::LonLat => geo.from_lonlat(p[0], p[1]),
            Coords::Crs(c) => {
                let (lon, lat) = c.to_lonlat(p[0], p[1])?;
                geo.from_lonlat(lon, lat)
            }
        }
        .filter(|(x, y)| x.is_finite() && y.is_finite())
    }

    /// The dataset's coordinates at a stored pixel position.
    pub(super) fn dataset_at(&self, p: (f64, f64), geo: &dyn GeoTransform) -> Option<Pt> {
        let q = match self {
            Coords::Stored(a) => Some(a.inverse()?.apply(p.0, p.1)),
            Coords::LonLat => geo.to_lonlat(p.0, p.1),
            Coords::Crs(c) => {
                let (lon, lat) = geo.to_lonlat(p.0, p.1)?;
                c.from_lonlat(lon, lat)
            }
        }?;
        (q.0.is_finite() && q.1.is_finite()).then_some([q.0, q.1])
    }
}

/// One layer of the map.
#[derive(Clone)]
pub(super) struct MapLayer {
    /// The name the file gives (empty: `generated` names it).
    pub name: String,
    pub generated: Option<Generated>,
    pub data: Arc<Dataset>,
    /// Which of `data.layers`.
    pub layer: usize,
    pub coords: Coords,
    pub visible: bool,
    /// 0–1.
    pub opacity: f32,
    /// The palette colour (when the file has none).
    pub colour: [u8; 3],
    /// Chosen in the panel, over the file's colours.
    pub user_colour: Option<[u8; 3]>,
    pub line_px: Option<f32>,
    pub point_px: Option<f32>,
    /// The file's style of each feature.
    pub styles: Arc<Vec<Option<FeatureStyle>>>,
    /// Added from this file (it can be removed again; the file stays on disk).
    pub added: Option<PathBuf>,
    /// Each feature's bounds in the dataset's coordinates (hit tests).
    pub boxes: Arc<Vec<Option<[f64; 4]>>>,
}

impl MapLayer {
    /// The name shown (a made-up one in the reader's language).
    pub(super) fn shown_name(&self) -> String {
        super::super::cad::shown_name(&self.name, self.generated)
    }

    /// The colour lines and points are drawn in when nothing else says.
    pub(super) fn ink(&self) -> [u8; 3] {
        self.user_colour.unwrap_or(self.colour)
    }

    /// The style the GPU applies over the scene's own.
    pub(super) fn style(&self) -> LayerStyle {
        LayerStyle {
            stroke: self.user_colour,
            fill: self.user_colour.map(|[r, g, b]| [r, g, b, FILL_ALPHA]),
            fill_alpha: Some(
                self.styles
                    .iter()
                    .flatten()
                    .find_map(|s| s.fill.map(|f| f[3]))
                    .unwrap_or(FILL_ALPHA),
            ),
            opacity: if self.visible { self.opacity } else { 0.0 },
            line_px: self.line_px.map(f64::from),
            dot_px: self.point_px.map(f64::from),
        }
    }

    /// The geometry types the layer holds, most first ("Polygon", "Point").
    pub(super) fn kinds(&self) -> Vec<GeomClass> {
        let mut n = [0usize; 3];
        for f in &self.data.layers[self.layer].features {
            if let Some(g) = &f.geometry {
                n[g.class() as usize] += 1;
            }
        }
        let mut k: Vec<GeomClass> = [GeomClass::Polygon, GeomClass::Line, GeomClass::Point]
            .into_iter()
            .filter(|c| n[*c as usize] > 0)
            .collect();
        k.sort_by_key(|c| std::cmp::Reverse(n[*c as usize]));
        k
    }

    pub(super) fn feature_count(&self) -> usize {
        self.data.layers[self.layer].features.len()
    }
}

/// The layers of one dataset that have features, in its order: `styles` are its features'
/// styles by feature id (KML), else simplestyle attributes are read.
pub(super) fn layers_of(
    data: &Arc<Dataset>,
    coords: Coords,
    kml: Option<&std::collections::HashMap<String, FeatureStyle>>,
    added: Option<PathBuf>,
    first_colour: usize,
) -> Vec<MapLayer> {
    data.layers
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.features.is_empty())
        .map(|(li, l)| {
            let styles: Vec<Option<FeatureStyle>> = l
                .features
                .iter()
                .map(|f| match kml {
                    Some(k) => f.id.as_ref().and_then(|id| k.get(id).copied()),
                    // S-57 charts: chart colours by object class.
                    None if data.format == "S-57" => {
                        specular_decode::geo::specular_gis::s57::style(
                            &l.name,
                            l.fields(),
                            &f.values,
                        )
                    }
                    None => simplestyle(l.fields(), &f.values),
                })
                .collect();
            let boxes = l
                .features
                .iter()
                .map(|f| f.geometry.as_ref().and_then(Geometry::bounds))
                .collect();
            let (c, _) = layer_colors(first_colour + li);
            let colour = match c {
                VColor::Rgb(r, g, b) => [r, g, b],
                _ => [31, 119, 180],
            };
            let name = if l.name.is_empty() && data.layers.len() == 1 {
                added
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default()
            } else {
                l.name.clone()
            };
            MapLayer {
                generated: name.is_empty().then_some(Generated::Layer(li as u32 + 1)),
                name,
                data: data.clone(),
                layer: li,
                coords: coords.clone(),
                visible: true,
                opacity: 1.0,
                colour,
                user_colour: None,
                line_px: None,
                point_px: None,
                styles: Arc::new(styles),
                added: added.clone(),
                boxes: Arc::new(boxes),
            }
        })
        .collect()
}

fn path_of(
    rings: &[Vec<Pt>],
    close: bool,
    l: &MapLayer,
    geo: &dyn GeoTransform,
) -> Option<BezPath> {
    let mut p = BezPath::new();
    let mut any = false;
    for r in rings {
        let mut started = false;
        for q in r {
            let Some((x, y)) = l.coords.to_stored(*q, geo) else {
                started = false;
                continue;
            };
            if started {
                p.line_to((x, y));
            } else {
                p.move_to((x, y));
                started = true;
                any = true;
            }
        }
        if close && started {
            p.close_path();
        }
    }
    any.then_some(p)
}

fn rgb(c: [u8; 4]) -> VColor {
    VColor::Rgb(c[0], c[1], c[2])
}

/// One feature's items into the polygon, line and point passes of its layer.
fn emit(
    g: &Geometry,
    li: u32,
    l: &MapLayer,
    style: Option<&FeatureStyle>,
    geo: &dyn GeoTransform,
    passes: &mut [Vec<Item>; 3],
) {
    let [r, gg, b] = l.colour;
    let ink = VColor::Rgb(r, gg, b);
    let item = |color, shape| Item {
        layer: li,
        outer: li,
        color,
        shape,
    };
    let weight = |w: Option<f32>, default: i16| {
        w.map_or(default, |px| {
            (f64::from(px) * 25.4 / 96.0 * 100.0).round() as i16
        })
    };
    let stroke = style.and_then(|s| s.stroke);
    let width = style.and_then(|s| s.width_px);
    let polygon = |rings: &[Vec<Pt>], passes: &mut [Vec<Item>; 3]| {
        let Some(p) = path_of(rings, true, l, geo) else {
            return;
        };
        let fill = style.and_then(|s| s.fill);
        if fill.is_none_or(|f| f[3] > 0) {
            passes[0].push(item(
                fill.map_or(ink, rgb),
                Shape::Fill {
                    path: p.clone(),
                    even_odd: true,
                },
            ));
        }
        if stroke.is_none_or(|s| s[3] > 0) {
            passes[0].push(item(
                stroke.map_or(ink, rgb),
                Shape::Stroke {
                    path: p,
                    weight: weight(width, OUTLINE_WEIGHT),
                    dash: None,
                },
            ));
        }
    };
    let line = |ls: &[Vec<Pt>], passes: &mut [Vec<Item>; 3]| {
        if let Some(p) = path_of(ls, false, l, geo) {
            passes[1].push(item(
                stroke.map_or(ink, rgb),
                Shape::Stroke {
                    path: p,
                    weight: weight(width, LINE_WEIGHT),
                    dash: None,
                },
            ));
        }
    };
    let point = |q: &Pt, passes: &mut [Vec<Item>; 3]| {
        if let Some((x, y)) = l.coords.to_stored(*q, geo) {
            let c = style.and_then(|s| s.marker).map_or(ink, rgb);
            passes[2].push(item(c, Shape::Point(Point::new(x, y))));
        }
    };
    match g {
        Geometry::Point(q) => point(q, passes),
        Geometry::MultiPoint(v) => v.iter().for_each(|q| point(q, passes)),
        Geometry::LineString(v) => line(std::slice::from_ref(v), passes),
        Geometry::MultiLineString(v) => line(v, passes),
        Geometry::Polygon(r) => polygon(r, passes),
        Geometry::MultiPolygon(ps) => {
            let all: Vec<Vec<Pt>> = ps.iter().flatten().cloned().collect();
            polygon(&all, passes);
        }
        Geometry::Collection(gs) => {
            for g in gs {
                emit(g, li, l, style, geo, passes);
            }
        }
    }
}

/// The scene of every layer in stored pixels of the current image (y down), each layer's
/// polygons, lines and points in turn, bottom layer first.
pub(super) fn build_scene(layers: &[MapLayer], geo: &dyn GeoTransform) -> VectorScene {
    let mut items = Vec::new();
    let mut vlayers = Vec::with_capacity(layers.len());
    let mut bounds: Option<KRect> = None;
    for (li, l) in layers.iter().enumerate() {
        let mut passes: [Vec<Item>; 3] = Default::default();
        let src = &l.data.layers[l.layer];
        for (fi, f) in src.features.iter().enumerate() {
            if let Some(g) = &f.geometry {
                let style = l.styles.get(fi).and_then(Option::as_ref);
                emit(g, li as u32, l, style, geo, &mut passes);
            }
        }
        let n: usize = passes.iter().map(Vec::len).sum();
        for it in passes.iter().flatten() {
            let b = match &it.shape {
                Shape::Fill { path, .. } | Shape::Stroke { path, .. } => {
                    specular_decode::drawing::specular_vector::kurbo::Shape::bounding_box(path)
                }
                Shape::Point(p) => KRect::from_points(*p, *p),
                Shape::Text(_) => continue,
            };
            bounds = Some(bounds.map_or(b, |a| a.union(b)));
        }
        items.extend(passes.into_iter().flatten());
        let [r, g, b] = l.colour;
        vlayers.push(VLayer {
            name: l.name.clone(),
            generated: l.generated,
            color: VColor::Rgb(r, g, b),
            linetype: None,
            lineweight: LINE_WEIGHT,
            on: true,
            frozen: false,
            plot: true,
            items: n as u64,
        });
    }
    let mut sheet = Sheet::generated(Generated::Map, SheetKind::Model);
    sheet.items = items;
    sheet.extents = bounds;
    VectorScene {
        layer_opacity: Vec::new(),
        pcb: None,
        survey: None,
        format: "map".to_owned(),
        version: String::new(),
        release: None,
        units: Units { code: 0 },
        layers: vlayers,
        linetypes: Vec::new(),
        fonts: vec![FontSpec::default()],
        sheets: vec![sheet],
        counts: Vec::new(),
        warnings: Vec::new(),
        lineweight_display: true,
        point_mode: 0,
        point_size: 0.0,
        codepage: None,
        dot_radius: specular_decode::geo::specular_gis::scene::DOT_RADIUS,
        plot: None,
        layout: None,
    }
}

/// A layer's extent in stored pixels (x0, y0, x1, y1): its bounds' corners and edge
/// midpoints converted (a layer in another CRS bends).
pub(super) fn extent(l: &MapLayer, geo: &dyn GeoTransform) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    let mut add = |p: Pt| {
        if let Some((x, y)) = l.coords.to_stored(p, geo) {
            b = Some(match b {
                None => [x, y, x, y],
                Some(b) => [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)],
            });
        }
    };
    let all = l.boxes.iter().flatten().fold(None::<[f64; 4]>, |acc, q| {
        Some(match acc {
            None => *q,
            Some(a) => [
                a[0].min(q[0]),
                a[1].min(q[1]),
                a[2].max(q[2]),
                a[3].max(q[3]),
            ],
        })
    })?;
    for i in 0..=4 {
        let t = f64::from(i) / 4.0;
        let x = all[0] + (all[2] - all[0]) * t;
        let y = all[1] + (all[3] - all[1]) * t;
        add([x, all[1]]);
        add([x, all[3]]);
        add([all[0], y]);
        add([all[2], y]);
    }
    b
}

fn seg_dist2(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (x, y) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
    x * x + y * y
}

fn near_line(l: &[Pt], p: Pt, tol: f64) -> bool {
    let t2 = tol * tol;
    match l {
        [a] => seg_dist2(p, *a, *a) <= t2,
        _ => l.windows(2).any(|w| seg_dist2(p, w[0], w[1]) <= t2),
    }
}

fn ring_contains(r: &[Pt], p: Pt) -> bool {
    let mut inside = false;
    let n = r.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (r[i], r[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Whether `g` is at `p` within `tol` (dataset units): points and lines by distance, areas
/// inside (even-odd) or on their outline.
pub(super) fn touches(g: &Geometry, p: Pt, tol: f64) -> bool {
    let inside = |rings: &[Vec<Pt>]| rings.iter().filter(|r| ring_contains(r, p)).count() % 2 == 1;
    match g {
        Geometry::Point(q) => seg_dist2(p, *q, *q) <= tol * tol,
        Geometry::MultiPoint(v) => v.iter().any(|q| seg_dist2(p, *q, *q) <= tol * tol),
        Geometry::LineString(l) => near_line(l, p, tol),
        Geometry::MultiLineString(ls) => ls.iter().any(|l| near_line(l, p, tol)),
        Geometry::Polygon(r) => inside(r) || r.iter().any(|r| near_line(r, p, tol)),
        Geometry::MultiPolygon(ps) => ps
            .iter()
            .any(|r| inside(r) || r.iter().any(|r| near_line(r, p, tol))),
        Geometry::Collection(gs) => gs.iter().any(|g| touches(g, p, tol)),
    }
}

/// The topmost feature of a layer at dataset position `p` within `tol`: points before
/// lines before areas, later features first.
pub(super) fn hit_layer(l: &MapLayer, p: Pt, tol: f64) -> Option<usize> {
    let feats = &l.data.layers[l.layer].features;
    let mut best: Option<(GeomClass, usize)> = None;
    for (i, b) in l.boxes.iter().enumerate().rev() {
        let Some(b) = b else { continue };
        if p[0] < b[0] - tol || p[0] > b[2] + tol || p[1] < b[1] - tol || p[1] > b[3] + tol {
            continue;
        }
        let Some(g) = &feats[i].geometry else {
            continue;
        };
        let class = g.class();
        if best.is_some_and(|(c, _)| c >= class) {
            continue;
        }
        if touches(g, p, tol) {
            best = Some((class, i));
            if class == GeomClass::Point {
                break;
            }
        }
    }
    best.map(|(_, i)| i)
}

/// What the layer panel shows of one layer.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    /// Index into the map's layers.
    pub index: usize,
    pub name: String,
    pub colour: [u8; 3],
    pub visible: bool,
    pub opacity: f32,
    pub features: usize,
    /// Its kind of geometry, most first ("Polygon", "Line", "Point").
    pub kind: Option<GeomClass>,
    pub added: bool,
    pub chosen: bool,
    pub focused: bool,
}

/// The chosen layer's style controls, in the order the keyboard walks them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Control {
    Visible,
    OpacityDown,
    OpacityUp,
    Colour,
    Width,
    Point,
    Up,
    Down,
    Zoom,
    Table,
    Remove,
}

impl Control {
    pub(super) const ALL: [Control; 11] = [
        Control::Visible,
        Control::OpacityDown,
        Control::OpacityUp,
        Control::Colour,
        Control::Width,
        Control::Point,
        Control::Up,
        Control::Down,
        Control::Zoom,
        Control::Table,
        Control::Remove,
    ];

    pub(super) fn label(self) -> String {
        match self {
            Control::Visible => t!("map-layer-visible"),
            Control::OpacityDown => t!("map-layer-opacity-down"),
            Control::OpacityUp => t!("map-layer-opacity-up"),
            Control::Colour => t!("map-layer-colour"),
            Control::Width => t!("map-layer-width"),
            Control::Point => t!("map-layer-point"),
            Control::Up => t!("map-layer-up"),
            Control::Down => t!("map-layer-down"),
            Control::Zoom => t!("palette-map-zoom-to-layer"),
            Control::Table => t!("palette-map-attribute-table"),
            Control::Remove => t!("map-layer-remove"),
        }
    }
}

/// The layer panel as drawn.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Panel {
    /// Top layer first.
    pub rows: Vec<Row>,
    /// The controls shown for the chosen layer (Remove only for an added one).
    pub controls: Vec<Control>,
    pub focused_control: Option<Control>,
    /// The confirmation of removing the chosen layer is open.
    pub confirm: Option<String>,
    pub scroll: usize,
    /// A layer is being added (its file is read).
    pub adding: bool,
}

/// What a press in the panel hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PanelHit {
    Row(usize),
    Check(usize),
    Control(Control),
    Add,
    ConfirmRemove,
    ConfirmKeep,
}

/// The panel as drawn: its rectangle and the hit targets.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct DrawnPanel {
    pub rect: Option<Rect>,
    pub hits: Vec<(PanelHit, Rect)>,
    pub rows_shown: usize,
}

fn short(name: &str) -> String {
    if name.chars().count() <= NAME_CHARS {
        return name.to_owned();
    }
    let mut s: String = name.chars().take(NAME_CHARS - 1).collect();
    s.push('…');
    s
}

fn kind_name(k: Option<GeomClass>) -> String {
    match k {
        Some(GeomClass::Polygon) => t!("map-kind-area"),
        Some(GeomClass::Line) => t!("map-kind-line"),
        Some(GeomClass::Point) => t!("map-kind-point"),
        None => t!("map-kind-empty"),
    }
}

/// What a screen reader says for a row.
pub(super) fn row_text(r: &Row) -> String {
    t!(
        "map-layer-row",
        name = r.name.clone(),
        kind = kind_name(r.kind),
        features = r.features as i64,
        opacity = (r.opacity * 100.0).round() as i64
    )
}

fn colour(c: [u8; 3]) -> Color {
    Color::rgba(
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        1.0,
    )
}

/// Draws a control's icon in its button `r` (lines and dots: no font has them all).
fn draw_icon(dl: &mut DrawList, c: Control, r: Rect, ink: Color, ts: f32) {
    let (cx, cy) = r.center();
    let s = 5.0 * ts;
    let lw = 1.5 * ts;
    match c {
        // An eye: a lens and its pupil.
        Control::Visible => {
            dl.line((cx - s, cy), (cx, cy - s * 0.6), lw, ink);
            dl.line((cx, cy - s * 0.6), (cx + s, cy), lw, ink);
            dl.line((cx - s, cy), (cx, cy + s * 0.6), lw, ink);
            dl.line((cx, cy + s * 0.6), (cx + s, cy), lw, ink);
            dl.circle((cx, cy), 1.8 * ts, ink);
        }
        Control::OpacityDown => dl.line((cx - s, cy), (cx + s, cy), lw, ink),
        Control::OpacityUp => {
            dl.line((cx - s, cy), (cx + s, cy), lw, ink);
            dl.line((cx, cy - s), (cx, cy + s), lw, ink);
        }
        Control::Colour => dl.rect(Rect::new(cx - s, cy - s, 2.0 * s, 2.0 * s), 2.0 * ts, ink),
        // Lines of three widths.
        Control::Width => {
            for (k, w) in [(-1.0, 0.8), (0.0, 1.6), (1.0, 2.6)] {
                let y = cy + k * s * 0.8;
                dl.line((cx - s, y), (cx + s, y), w * ts, ink);
            }
        }
        Control::Point => {
            dl.circle((cx - s * 0.6, cy), 1.2 * ts, ink);
            dl.circle((cx + s * 0.4, cy), 2.4 * ts, ink);
        }
        Control::Up | Control::Down => {
            let d = if c == Control::Up { -1.0 } else { 1.0 };
            dl.line((cx, cy - s * d), (cx, cy + s * d), lw, ink);
            dl.line((cx, cy + s * d), (cx - s * 0.6, cy + s * d * 0.3), lw, ink);
            dl.line((cx, cy + s * d), (cx + s * 0.6, cy + s * d * 0.3), lw, ink);
        }
        // Four corners: the whole extent.
        Control::Zoom => {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let (x, y) = (cx + sx * s, cy + sy * s);
                dl.line((x, y), (x - sx * s * 0.6, y), lw, ink);
                dl.line((x, y), (x, y - sy * s * 0.6), lw, ink);
            }
        }
        // A grid: the table.
        Control::Table => {
            dl.stroke_rect(
                Rect::new(cx - s, cy - s, 2.0 * s, 2.0 * s),
                ts,
                ink,
                lw * 0.8,
            );
            dl.line(
                (cx - s, cy - s * 0.3),
                (cx + s, cy - s * 0.3),
                lw * 0.8,
                ink,
            );
            dl.line(
                (cx - s, cy + s * 0.35),
                (cx + s, cy + s * 0.35),
                lw * 0.8,
                ink,
            );
            dl.line(
                (cx - s * 0.2, cy - s),
                (cx - s * 0.2, cy + s),
                lw * 0.8,
                ink,
            );
        }
        Control::Remove => {
            dl.line(
                (cx - s * 0.8, cy - s * 0.8),
                (cx + s * 0.8, cy + s * 0.8),
                lw,
                ink,
            );
            dl.line(
                (cx + s * 0.8, cy - s * 0.8),
                (cx - s * 0.8, cy + s * 0.8),
                lw,
                ink,
            );
        }
    }
}

/// Draws the panel at the top-start corner of `area`; RTL mirrors it.
pub(super) fn draw_panel(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    p: &Panel,
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) -> DrawnPanel {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let st = TextStyle::new(FONT_SMALL * ts);
    let title = text.layout(
        &t!(
            "map-layers-title",
            shown = p.rows.iter().filter(|r| r.visible).count() as i64,
            total = p.rows.len() as i64
        ),
        st.weight(600.0),
        None,
        Align::Start,
    );
    let row_h = (FONT_SMALL * 2.6 * ts).round();
    let ctl_h = (22.0 * ts).round();
    let w = (PANEL_W * ts).min(aw - 2.0 * m).max(160.0);
    let has_controls = !p.controls.is_empty();
    let extra = if has_controls { ctl_h + 6.0 * ts } else { 0.0 }
        + if p.confirm.is_some() {
            2.0 * row_h
        } else {
            0.0
        };
    let add_h = row_h;
    let max_h = (ah - 2.0 * m).max(row_h * 4.0) * 0.7;
    let fixed = 2.0 * PAD + title.height + 6.0 * ts + extra + add_h;
    let fit = (((max_h - fixed) / row_h).floor() as usize).max(1);
    let n = p.rows.len().min(fit);
    let scroll = p.scroll.min(p.rows.len().saturating_sub(n));
    let h = fixed + n as f32 * row_h;
    let x = if rtl { ax + aw - m - w } else { ax + m };
    let y = ay + m;
    let panel = Rect::new(x, y, w, h);
    let mut hits = Vec::new();
    dl.glass(panel, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    // Mirrors a rectangle inside the panel for right to left.
    let mx = |r: Rect| {
        if rtl {
            Rect::new(2.0 * x + w - r.x - r.w, r.y, r.w, r.h)
        } else {
            r
        }
    };
    let tr = mx(Rect::new(x + PAD, y + PAD, title.width, title.height));
    text.draw(dl, &title, tr.x, tr.y, pal.text_muted);
    let mut ry = y + PAD + title.height + 6.0 * ts;
    let box_s = 12.0 * ts;
    let sw = 10.0 * ts;
    for r in p.rows.iter().skip(scroll).take(n) {
        let row = Rect::new(x + PAD / 2.0, ry, w - PAD, row_h);
        if r.chosen {
            dl.rect(row, RADIUS_SM, pal.accent.with_alpha(0.18));
        }
        if r.focused {
            dl.stroke_rect(row, RADIUS_SM, pal.accent, 1.5 * ts);
        }
        let cy = ry + row_h / 2.0;
        let b = mx(Rect::new(x + PAD, cy - box_s / 2.0, box_s, box_s));
        if r.visible {
            dl.rect(b, 3.0 * ts, pal.accent_fill);
            let s = box_s;
            dl.line(
                (b.x + 0.22 * s, b.y + 0.52 * s),
                (b.x + 0.42 * s, b.y + 0.72 * s),
                1.6 * ts,
                pal.accent_text,
            );
            dl.line(
                (b.x + 0.42 * s, b.y + 0.72 * s),
                (b.x + 0.78 * s, b.y + 0.30 * s),
                1.6 * ts,
                pal.accent_text,
            );
        } else {
            dl.rect(b, 3.0 * ts, pal.text_faint);
            dl.rect(b.inset(1.5 * ts), 2.0 * ts, pal.surface);
        }
        let swr = mx(Rect::new(x + PAD + box_s + 8.0 * ts, cy - sw / 2.0, sw, sw));
        let ink = colour(r.colour).with_alpha(0.25 + 0.75 * r.opacity);
        match r.kind {
            Some(GeomClass::Point) => dl.circle(swr.center(), sw / 2.0, ink),
            Some(GeomClass::Line) => {
                dl.line((swr.x, swr.y + sw), (swr.x + sw, swr.y), 2.0 * ts, ink)
            }
            _ => dl.rect(swr, 2.0 * ts, ink),
        }
        let name = text.layout(&short(&r.name), st, None, Align::Start);
        let sub = text.layout(
            &t!(
                "map-layer-sub",
                kind = kind_name(r.kind),
                features = r.features as i64,
                opacity = (r.opacity * 100.0).round() as i64
            ),
            TextStyle::new(FONT_SMALL * 0.85 * ts).tabular(),
            None,
            Align::Start,
        );
        let lx0 = x + PAD + box_s + 8.0 * ts + sw + 8.0 * ts;
        let ty = cy - (name.height + sub.height) / 2.0;
        let nr = mx(Rect::new(lx0, ty, name.width, name.height));
        let ink = if r.visible { pal.text } else { pal.text_muted };
        text.draw(dl, &name, nr.x, nr.y, ink);
        let sr = mx(Rect::new(lx0, ty + name.height, sub.width, sub.height));
        text.draw(dl, &sub, sr.x, sr.y, pal.text_muted);
        hits.push((PanelHit::Check(r.index), b.inset(-4.0 * ts)));
        hits.push((PanelHit::Row(r.index), row));
        ry += row_h;
    }
    // The chosen layer's controls.
    if has_controls {
        ry += 3.0 * ts;
        let bw = ((w - PAD * 2.0) / p.controls.len() as f32).min(26.0 * ts);
        for (i, c) in p.controls.iter().enumerate() {
            let r = mx(Rect::new(x + PAD + i as f32 * bw, ry, bw - 2.0 * ts, ctl_h));
            let focused = p.focused_control == Some(*c);
            dl.rect_border(
                r,
                RADIUS_SM,
                pal.surface_raised,
                if focused { pal.accent } else { pal.separator },
                if focused { 1.5 * ts } else { 1.0 },
            );
            let ink = if *c == Control::Colour {
                p.rows
                    .iter()
                    .find(|r| r.chosen)
                    .map_or(pal.text, |r| colour(r.colour))
            } else {
                pal.text
            };
            draw_icon(dl, *c, r, ink, ts);
            hits.push((PanelHit::Control(*c), r));
        }
        ry += ctl_h + 3.0 * ts;
    }
    if let Some(name) = &p.confirm {
        let q = text.layout(
            &t!("map-layer-remove-ask", name = name.clone()),
            st,
            Some(w - 2.0 * PAD),
            Align::Start,
        );
        let qr = mx(Rect::new(x + PAD, ry, q.width, q.height));
        text.draw(dl, &q, qr.x, qr.y, pal.text);
        let by = ry + q.height.max(row_h * 0.9) + 2.0 * ts;
        let (rb, kb) = (
            text.layout(&t!("map-layer-remove"), st, None, Align::Start),
            text.layout(&t!("map-layer-keep"), st, None, Align::Start),
        );
        let r1 = mx(Rect::new(x + PAD, by, rb.width + 16.0 * ts, row_h * 0.8));
        let r2 = mx(Rect::new(
            x + PAD + rb.width + 24.0 * ts,
            by,
            kb.width + 16.0 * ts,
            row_h * 0.8,
        ));
        dl.rect(r1, RADIUS_SM, pal.danger);
        text.draw(
            dl,
            &rb,
            r1.x + 8.0 * ts,
            r1.y + (r1.h - rb.height) / 2.0,
            Color::WHITE,
        );
        dl.rect_border(r2, RADIUS_SM, pal.surface_raised, pal.separator, 1.0);
        text.draw(
            dl,
            &kb,
            r2.x + 8.0 * ts,
            r2.y + (r2.h - kb.height) / 2.0,
            pal.text,
        );
        hits.push((PanelHit::ConfirmRemove, r1));
        hits.push((PanelHit::ConfirmKeep, r2));
        ry = by + row_h;
    }
    // Add a layer.
    let add = text.layout(
        &if p.adding {
            t!("map-layer-adding")
        } else {
            t!("palette-map-add-layer")
        },
        st,
        None,
        Align::Start,
    );
    let ar = mx(Rect::new(
        x + PAD,
        ry + 2.0 * ts,
        add.width + 12.0 * ts,
        row_h - 6.0 * ts,
    ));
    dl.rect_border(ar, RADIUS_SM, pal.surface_raised, pal.separator, 1.0);
    text.draw(
        dl,
        &add,
        ar.x + 6.0 * ts,
        ar.y + (ar.h - add.height) / 2.0,
        pal.text,
    );
    hits.push((PanelHit::Add, ar));
    DrawnPanel {
        rect: Some(panel),
        hits,
        rows_shown: n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_decode::geo::specular_gis::model::Layer;
    use specular_geodesy::seam::LonLatAffine;

    /// Stored px = degrees × 100, y down from 50° N.
    fn geo() -> LonLatAffine {
        LonLatAffine {
            m: [0.01, 0.0, 0.0, 0.0, -0.01, 50.0],
            name: "WGS 84",
        }
    }

    fn data() -> Arc<Dataset> {
        let mut a = Layer::new("areas");
        a.push(
            None,
            Some(Geometry::Polygon(vec![vec![
                [1.0, 41.0],
                [2.0, 41.0],
                [2.0, 42.0],
                [1.0, 42.0],
            ]])),
            vec![
                ("fill".into(), Value::Text("#ff0000".into())),
                ("fill-opacity".into(), Value::Float(0.25)),
            ],
        );
        let mut p = Layer::new("");
        p.push(None, Some(Geometry::Point([1.5, 41.5])), vec![]);
        p.push(
            None,
            Some(Geometry::LineString(vec![[0.0, 40.0], [3.0, 40.0]])),
            vec![],
        );
        Arc::new(Dataset {
            format: "GeoJSON",
            layers: vec![a, p],
            ..Dataset::default()
        })
    }

    use specular_decode::geo::specular_gis::model::Value;

    #[test]
    fn layers_scene_extent_and_hits() {
        let d = data();
        let ls = layers_of(&d, Coords::LonLat, None, None, 0);
        assert_eq!(ls.len(), 2);
        assert_eq!(ls[0].name, "areas");
        assert_eq!(ls[1].generated, Some(Generated::Layer(2)));
        // The file's fill and its alpha.
        assert_eq!(ls[0].styles[0].and_then(|s| s.fill), Some([255, 0, 0, 64]));
        assert_eq!(ls[0].style().fill_alpha, Some(64));
        assert_eq!(ls[1].style().fill_alpha, Some(FILL_ALPHA));
        let g = geo();
        let s = build_scene(&ls, &g);
        let items = &s.sheets[0].items;
        // Layer 0's fill and outline, then layer 1's line before its point.
        assert_eq!(items.len(), 4);
        assert!(matches!(items[0].shape, Shape::Fill { .. }));
        assert_eq!(items[0].color, VColor::Rgb(255, 0, 0));
        assert!(matches!(items[2].shape, Shape::Stroke { .. }));
        assert!(matches!(items[3].shape, Shape::Point(_)));
        assert_eq!(items[3].layer, 1);
        // In stored px: lon 1.5 → x 150, lat 41.5 → y 850.
        let Shape::Point(pt) = items[3].shape else {
            unreachable!()
        };
        assert!((pt.x - 150.0).abs() < 1e-9 && (pt.y - 850.0).abs() < 1e-9);
        let e = extent(&ls[0], &g).unwrap();
        assert!(
            (e[0] - 100.0).abs() < 1e-9 && (e[3] - 900.0).abs() < 1e-9,
            "{e:?}"
        );
        // Hits in degrees: the point over the area, the line by distance.
        assert_eq!(hit_layer(&ls[1], [1.5, 41.5], 0.01), Some(0));
        assert_eq!(hit_layer(&ls[1], [2.0, 40.005], 0.01), Some(1));
        assert_eq!(hit_layer(&ls[0], [1.2, 41.2], 0.01), Some(0));
        assert_eq!(hit_layer(&ls[0], [5.0, 45.0], 0.01), None);
        // Back from stored px.
        let q = ls[0].coords.dataset_at((150.0, 850.0), &g).unwrap();
        assert!((q[0] - 1.5).abs() < 1e-9 && (q[1] - 41.5).abs() < 1e-9);
        // The panel's choices change the GPU style, not the scene.
        let mut l = ls[0].clone();
        l.user_colour = Some([0, 0, 255]);
        l.line_px = Some(4.0);
        l.opacity = 0.5;
        l.visible = false;
        let st = l.style();
        assert_eq!(st.stroke, Some([0, 0, 255]));
        assert_eq!(st.line_px, Some(4.0));
        assert_eq!(st.opacity, 0.0);
        assert_eq!(l.kinds(), vec![GeomClass::Polygon]);
    }
}
