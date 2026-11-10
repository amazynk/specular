//! A LandXML file as a vector drawing: the plan (surfaces coloured by elevation, contours,
//! breaklines, boundaries, alignments with their stationing) and a chart per alignment with
//! profiles.

use specular_vector::kurbo::{Affine, BezPath, Point, Rect};
use specular_vector::{
    AlignmentInfo, Color, FontSpec, HAlign, Item, Layer, Paragraph, Shape, Sheet, SheetKind,
    SheetUnits, Span, SurfaceInfo, SurveyInfo, TextBlock, Units, VAlign, VectorScene,
};

use crate::align::{profile_elevation, station_label};
use crate::{LandXml, Profile, tin};

/// Elevation ramp: low to high.
const RAMP: [[f64; 3]; 5] = [
    [40.0, 92.0, 160.0],
    [62.0, 150.0, 92.0],
    [228.0, 212.0, 112.0],
    [150.0, 100.0, 62.0],
    [242.0, 242.0, 242.0],
];
/// Colour bins of the elevation fill.
const BINS: usize = 24;
/// Contour segments drawn at most per surface.
const MAX_CONTOUR_SEGMENTS: usize = 2_000_000;

fn ramp(t: f64) -> Color {
    let t = t.clamp(0.0, 1.0) * (RAMP.len() - 1) as f64;
    let i = (t.floor() as usize).min(RAMP.len() - 2);
    let u = t - i as f64;
    let c = |k: usize| (RAMP[i][k] + (RAMP[i + 1][k] - RAMP[i][k]) * u).round() as u8;
    Color::Rgb(c(0), c(1), c(2))
}

/// [`ramp`] as sRGB with full alpha (the 3D model's vertex colours).
pub(crate) fn ramp_rgb(t: f64) -> [u8; 4] {
    match ramp(t) {
        Color::Rgb(r, g, b) => [r, g, b, 255],
        _ => [128, 128, 128, 255],
    }
}

fn layer(name: String, color: Color, weight: i16) -> Layer {
    Layer {
        name,
        generated: None,
        color,
        linetype: None,
        lineweight: weight,
        on: true,
        frozen: false,
        plot: true,
        items: 0,
    }
}

struct B {
    layers: Vec<Layer>,
    items: Vec<Item>,
}

impl B {
    fn layer(&mut self, name: String, color: Color, weight: i16) -> u32 {
        self.layers.push(layer(name, color, weight));
        (self.layers.len() - 1) as u32
    }

    fn push(&mut self, layer: u32, color: Option<Color>, shape: Shape) {
        let l = &mut self.layers[layer as usize];
        l.items += 1;
        self.items.push(Item {
            layer,
            outer: layer,
            color: color.unwrap_or(l.color),
            shape,
        });
    }

    fn stroke(&mut self, layer: u32, path: BezPath, weight: i16) {
        if path.elements().len() > 1 {
            self.push(
                layer,
                None,
                Shape::Stroke {
                    path,
                    weight,
                    dash: None,
                },
            );
        }
    }

    fn text(&mut self, layer: u32, at: Point, rot: f64, h: f64, s: String, ha: HAlign) {
        self.push(
            layer,
            None,
            Shape::Text(Box::new(TextBlock {
                transform: Affine::translate(at.to_vec2()) * Affine::rotate(rot),
                halign: ha,
                valign: VAlign::Middle,
                wrap: 0.0,
                fit: None,
                line_spacing: 1.0,
                paragraphs: vec![Paragraph {
                    spans: vec![Span::new(s, 0, h)],
                    align: None,
                }],
            })),
        );
    }
}

fn pt(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

/// The plan's bounds.
fn bounds(x: &LandXml) -> Option<Rect> {
    let mut r: Option<Rect> = None;
    let mut add = |p: [f64; 2]| {
        if p[0].is_finite() && p[1].is_finite() {
            let q = Point::new(p[0], p[1]);
            r = Some(r.map_or(Rect::from_points(q, q), |r| r.union_pt(q)));
        }
    };
    for s in &x.surfaces {
        for f in &s.faces {
            for &i in f {
                if let Some(p) = s.points.get(i as usize) {
                    add([p[0], p[1]]);
                }
            }
        }
    }
    for a in &x.alignments {
        for (_, p) in a.sample(10.0) {
            add(p);
        }
    }
    for p in &x.parcels {
        for g in &p.boundary {
            let (a, b) = g.ends();
            add(a);
            if let Some(b) = b {
                add(b);
            }
        }
    }
    for p in &x.points {
        add(p.at);
    }
    r
}

/// Unit code for sheets: metres 6, feet 2, US survey feet 21.
fn unit_code(x: &LandXml) -> i16 {
    match x.units.linear.to_ascii_lowercase().as_str() {
        "ussurveyfoot" => 21,
        "foot" | "intlfoot" => 2,
        "millimeter" => 4,
        "centimeter" => 5,
        "kilometer" => 7,
        "inch" => 1,
        "mile" => 3,
        _ => 6,
    }
}

/// The drawing of a LandXML file.
pub fn scene(x: &LandXml) -> VectorScene {
    let mut b = B {
        layers: Vec::new(),
        items: Vec::new(),
    };
    let extent = bounds(x).map_or(100.0, |r| r.width().hypot(r.height()).max(1e-6));
    let text_h = extent / 120.0;
    let mut info = SurveyInfo {
        crs: x.crs.as_ref().and_then(|c| c.describe()),
        linear_unit: x.units.linear.clone(),
        application: x.application.clone(),
        project: x.project.clone(),
        ..SurveyInfo::default()
    };
    let mut warnings = x.warnings.clone();
    let mut counts: Vec<(String, u64)> = Vec::new();

    // Surfaces: triangles in elevation bins, contours, breaklines, boundaries.
    for s in &x.surfaces {
        let Some(m) = tin::measures(s) else { continue };
        let range = m.z_max - m.z_min;
        let step = tin::interval(range, 15.0);
        let lf = b.layer(format!("Surface {}", s.name), ramp(0.5), 0);
        let mut bins: Vec<BezPath> = vec![BezPath::new(); BINS];
        for f in &s.faces {
            let c: Option<Vec<[f64; 3]>> = f
                .iter()
                .map(|&i| s.points.get(i as usize).copied())
                .collect();
            let Some(c) = c else { continue };
            let z = (c[0][2] + c[1][2] + c[2][2]) / 3.0;
            let t = if range > 0.0 {
                (z - m.z_min) / range
            } else {
                0.5
            };
            let k = ((t * BINS as f64) as usize).min(BINS - 1);
            let p = &mut bins[k];
            p.move_to((c[0][0], c[0][1]));
            p.line_to((c[1][0], c[1][1]));
            p.line_to((c[2][0], c[2][1]));
            p.close_path();
        }
        for (k, path) in bins.into_iter().enumerate() {
            if !path.elements().is_empty() {
                let col = ramp((k as f64 + 0.5) / BINS as f64);
                b.push(
                    lf,
                    Some(col),
                    Shape::Fill {
                        path,
                        even_odd: false,
                    },
                );
            }
        }
        let minor = b.layer(format!("Contours {}", s.name), Color::Rgb(90, 70, 50), 0);
        let major = b.layer(
            format!("Index contours {}", s.name),
            Color::Rgb(70, 50, 30),
            25,
        );
        let segs = tin::contours(s, step, MAX_CONTOUR_SEGMENTS);
        if segs.len() >= MAX_CONTOUR_SEGMENTS {
            crate::warn(
                &mut warnings,
                format!("Surface {}: only part of the contours are drawn.", s.name),
            );
        }
        let (mut pmin, mut pmaj) = (BezPath::new(), BezPath::new());
        for (z, a, c) in &segs {
            let idx = (z / step).round() as i64;
            let p = if idx.rem_euclid(5) == 0 {
                &mut pmaj
            } else {
                &mut pmin
            };
            p.move_to(pt(*a));
            p.line_to(pt(*c));
        }
        b.stroke(minor, pmin, 0);
        b.stroke(major, pmaj, 25);
        if !s.breaklines.is_empty() {
            let l = b.layer(
                format!("Breaklines {}", s.name),
                Color::Rgb(200, 40, 40),
                18,
            );
            for bl in &s.breaklines {
                let mut p = BezPath::new();
                p.move_to((bl[0][0], bl[0][1]));
                for q in &bl[1..] {
                    p.line_to((q[0], q[1]));
                }
                b.stroke(l, p, 18);
            }
        }
        if !s.boundaries.is_empty() {
            let l = b.layer(format!("Boundaries {}", s.name), Color::Foreground, 35);
            for bd in &s.boundaries {
                let mut p = BezPath::new();
                p.move_to(pt(bd.points[0]));
                for q in &bd.points[1..] {
                    p.line_to(pt(*q));
                }
                p.close_path();
                b.stroke(l, p, 35);
            }
        }
        counts.push(("Point".into(), s.points.len() as u64));
        counts.push(("Triangle".into(), s.faces.len() as u64));
        info.surfaces.push(SurfaceInfo {
            name: s.name.clone(),
            points: s.points.len() as u64,
            triangles: s.faces.len() as u64,
            area_2d: m.area_2d,
            area_3d: m.area_3d,
            volume: m.volume,
            z_min: m.z_min,
            z_max: m.z_max,
            contour_interval: step,
        });
    }

    // Alignments with stationing.
    let imperial = x.units.imperial;
    let mut plots = Vec::new();
    for a in &x.alignments {
        let len = a.computed_length();
        if a.elements.is_empty() {
            continue;
        }
        let l = b.layer(
            format!("Alignment {}", a.name),
            Color::Rgb(220, 40, 160),
            35,
        );
        let mut p = BezPath::new();
        for (i, (_, q)) in a.sample(extent / 2000.0).into_iter().enumerate() {
            if i == 0 {
                p.move_to(pt(q));
            } else {
                p.line_to(pt(q));
            }
        }
        b.stroke(l, p, 35);
        // Ticks every whole station interval (100 m or 100 ft; finer on short alignments),
        // labelled where there is room.
        let unit = 100.0;
        let step = if len < unit * 2.0 { unit / 5.0 } else { unit };
        let sl = b.layer(format!("Stations {}", a.name), Color::Foreground, 0);
        let mut ticks = BezPath::new();
        let first = (a.sta_start / step).ceil() * step;
        let mut k = 0;
        // Labels far enough apart not to run into each other.
        let every = ((text_h * 8.0 / step).ceil() as usize).max(1);
        let mut sta = first;
        while sta <= a.sta_start + len + 1e-6 && k < 20_000 {
            if let Some((q, h)) = a.at_station(sta) {
                let n = specular_vector::kurbo::Vec2::new(-h.sin(), h.cos());
                let c = pt(q);
                let tick = text_h * 0.8;
                ticks.move_to(c - n * tick);
                ticks.line_to(c + n * tick);
                if k % every == 0 {
                    let mut rot = h;
                    // Readable: never upside down.
                    if rot.cos() < 0.0 {
                        rot += std::f64::consts::PI;
                    }
                    b.text(
                        sl,
                        c + n * (tick * 2.5),
                        rot,
                        text_h,
                        station_label(sta, imperial, 0),
                        HAlign::Center,
                    );
                }
            }
            sta += step;
            k += 1;
        }
        b.stroke(sl, ticks, 0);
        // A stated length that the geometry does not reach.
        if let Some(stated) = a.length
            && (stated - len).abs() > 0.001 * stated.max(1.0)
        {
            crate::warn(
                &mut warnings,
                format!(
                    "Alignment {}: the elements add up to {len:.3}, the file says {stated:.3}.",
                    a.name
                ),
            );
        }
        info.alignments.push(AlignmentInfo {
            name: a.name.clone(),
            length: len,
            sta_start: a.sta_start,
            sta_end: a.sta_start + len,
            profiles: a.profiles.len() as u32,
        });
        if !a.profiles.is_empty() {
            plots.push(profile_plot(a, x));
        }
    }
    counts.push(("Alignment".into(), info.alignments.len() as u64));

    // Parcels: boundaries, each named at its label point with its area.
    if !x.parcels.is_empty() {
        let l = b.layer("Parcels".into(), Color::Rgb(230, 140, 30), 25);
        let tl = b.layer("Parcel labels".into(), Color::Rgb(230, 140, 30), 0);
        for p in &x.parcels {
            let mut path = BezPath::new();
            let mut at: Option<[f64; 2]> = None;
            for g in &p.boundary {
                let el = crate::Alignment {
                    elements: vec![g.clone()],
                    ..crate::Alignment::default()
                };
                for (i, (_, q)) in el.sample(extent / 2000.0).into_iter().enumerate() {
                    let near = at.is_some_and(|a| (a[0] - q[0]).hypot(a[1] - q[1]) < 1e-9);
                    if i == 0 && !near {
                        path.move_to(pt(q));
                    } else if i > 0 {
                        path.line_to(pt(q));
                    }
                    at = Some(q);
                }
            }
            b.stroke(l, path, 25);
            if let Some(c) = p.center {
                let mut s = p.name.clone();
                if let Some(a) = p.area {
                    s = format!("{s} ({a:.1})");
                }
                b.text(tl, pt(c), 0.0, text_h * 0.8, s, HAlign::Center);
            }
        }
        counts.push(("Parcel".into(), x.parcels.len() as u64));
    }
    // Survey points: a cross each, named.
    if !x.points.is_empty() {
        let l = b.layer("Survey points".into(), Color::Rgb(40, 160, 220), 0);
        let tl = b.layer("Survey point names".into(), Color::Rgb(40, 160, 220), 0);
        let r = text_h * 0.4;
        let mut marks = BezPath::new();
        // Names only where they can be told apart.
        let named = x.points.len() <= 5_000;
        for p in &x.points {
            let c = pt(p.at);
            marks.move_to((c.x - r, c.y));
            marks.line_to((c.x + r, c.y));
            marks.move_to((c.x, c.y - r));
            marks.line_to((c.x, c.y + r));
            if named && !p.name.is_empty() {
                b.text(
                    tl,
                    Point::new(c.x + r * 1.5, c.y),
                    0.0,
                    text_h * 0.7,
                    p.name.clone(),
                    HAlign::Left,
                );
            }
        }
        b.stroke(l, marks, 0);
        counts.push(("Survey point".into(), x.points.len() as u64));
    }
    counts.retain(|c| c.1 > 0);

    let code = unit_code(x);
    let mut plan = Sheet::new("Plan", SheetKind::Model);
    plan.items = std::mem::take(&mut b.items);
    plan.extents = bounds(x);
    plan.units = Some(SheetUnits {
        code,
        model: code,
        lunits: 2,
        luprec: 3,
        dimscale: 1.0,
        paper_mm: None,
    });
    let mut sheets = vec![plan];
    let mut layers = b.layers;

    // Profiles: a chart per alignment, its layers appended.
    if !plots.is_empty() {
        let n = plots.len();
        let sp = specular_spectra::Spectra {
            facts: specular_spectra::Facts::default(),
            pages: plots,
        };
        let cs = specular_spectra::chart::scene(&sp, 0..n);
        let off = layers.len() as u32;
        layers.extend(cs.layers);
        for mut s in cs.sheets {
            for it in &mut s.items {
                it.layer += off;
                it.outer += off;
            }
            if let Some(c) = s.chart.as_mut() {
                for l in &mut c.legend {
                    *l += off;
                }
            }
            sheets.push(s);
        }
    }
    VectorScene {
        format: "LandXML".into(),
        version: x.version.clone(),
        units: Units { code },
        layers,
        fonts: vec![FontSpec::default()],
        sheets,
        counts,
        warnings,
        survey: Some(Box::new(info)),
        ..VectorScene::default()
    }
}

/// The chart of an alignment's profiles: stations across, elevations up.
fn profile_plot(a: &crate::Alignment, x: &LandXml) -> specular_spectra::Plot {
    let unit = if x.units.imperial { "ft" } else { "m" };
    let mut traces = Vec::new();
    let mut peaks = Vec::new();
    for p in &a.profiles {
        match p {
            Profile::Design { name, pvis } => {
                let (s0, s1) = (pvis[0].station, pvis[pvis.len() - 1].station);
                let n = (((s1 - s0) / 1.0).ceil() as usize).clamp(2, 20_000);
                let mut xs = Vec::with_capacity(n + 1);
                let mut ys = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let s = s0 + (s1 - s0) * i as f64 / n as f64;
                    if let Some(e) = profile_elevation(pvis, s) {
                        xs.push(s);
                        ys.push(e);
                    }
                }
                for v in pvis {
                    peaks.push(specular_spectra::Peak {
                        x: v.station,
                        y: v.elevation,
                        label: format!("PVI {}", station_label(v.station, x.units.imperial, 2)),
                    });
                }
                traces.push(specular_spectra::Trace {
                    name: name.clone(),
                    x: xs,
                    y: ys,
                    sticks: false,
                });
            }
            Profile::Ground { name, points } => traces.push(specular_spectra::Trace {
                name: name.clone(),
                x: points.iter().map(|p| p[0]).collect(),
                y: points.iter().map(|p| p[1]).collect(),
                sticks: false,
            }),
        }
    }
    specular_spectra::Plot {
        title: format!("Profile {}", a.name),
        x: specular_spectra::Axis::new("Station", unit, false),
        y: specular_spectra::Axis::new("Elevation", unit, false),
        traces,
        peaks,
        grid: None,
        notes: Vec::new(),
    }
}
