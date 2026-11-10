//! Terrain in the map view: slope and aspect under the pointer, contour lines with labels,
//! the slope and aspect legends, and the elevation model as a 3-D surface for the mesh
//! viewer with the map's colours draped over it.
//!
//! The slope and aspect colours themselves are drawn on the GPU by the science shader
//! ([`specular_render::Relief`]); everything here is the CPU side.

use std::sync::Arc;

use specular_decode::science::NdPlane;
use specular_decode::specular_mesh::{self, Bounds, ImageData, Material, Mesh, Part};
use specular_geodesy::geodesic;
use specular_geodesy::seam::GeoTransform;
use specular_geodesy::terrain::{self, Contour, Dem, Levels};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_render::{ASPECT_MAP, FLAT_GREY, Relief, SLOPE_MAP, SLOPE_MAX};
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette};

/// Cells traced for contours at most (larger grids are averaged down first).
const CONTOUR_CELLS: usize = 4_000_000;
/// Contour lines aimed for over the height range.
const CONTOUR_LINES: usize = 20;
/// Every this many contours is an index contour (bolder, labelled).
const INDEX_EVERY: u32 = 5;
/// Screen px between the points of a contour drawn (closer ones are skipped).
const MIN_STEP_PX: f32 = 1.5;
/// Vertices per side of the 3-D surface at most.
const MESH_SIDE: usize = 512;
/// The drape's texture side at most.
const DRAPE_SIDE: usize = 2048;

/// The ground metres from stored pixel (`x`, `y`) to the next one east along the row and
/// north down the column (negative when rows run south), through the map's transform.
pub(super) fn ground_steps(geo: &dyn GeoTransform, x: f64, y: f64) -> Option<[f64; 2]> {
    let (lon, lat) = geo.to_lonlat(x, y)?;
    let (lon_e, lat_e) = geo.to_lonlat(x + 1.0, y)?;
    let (lon_s, lat_s) = geo.to_lonlat(x, y + 1.0)?;
    let ex = geodesic::inverse(lat, lon, lat_e, lon_e).s12;
    let sy = geodesic::inverse(lat, lon, lat_s, lon_s).s12;
    let sign = if lat_s < lat { -1.0 } else { 1.0 };
    let east = if lon_e >= lon { 1.0 } else { -1.0 };
    (ex > 0.0 && sy > 0.0).then_some([ex * east, sy * sign])
}

/// A plane as an elevation grid: its values in metres per unit `metres`, stepping
/// `step` ground metres per stored pixel (the plane may be smaller than the image).
pub(super) fn dem<'a>(p: &'a NdPlane, size: (f64, f64), step: [f64; 2], metres: f64) -> Dem<'a> {
    let (kx, ky) = (
        size.0 / f64::from(p.width.max(1)),
        size.1 / f64::from(p.height.max(1)),
    );
    Dem {
        values: &p.values,
        w: p.width as usize,
        h: p.height as usize,
        step: [step[0] * kx, step[1] * ky],
        z: metres,
    }
}

/// The readout's terrain line at stored (`x`, `y`): "Slope 12.4° · faces SE (135°)".
pub(super) fn slope_line(d: &Dem, size: (f64, f64), x: f64, y: f64) -> Option<String> {
    let cx = (x / size.0 * d.w as f64).floor();
    let cy = (y / size.1 * d.h as f64).floor();
    if cx < 0.0 || cy < 0.0 {
        return None;
    }
    let (cx, cy) = (cx as usize, cy as usize);
    let slope = d.slope(cx, cy)?;
    let slope_s = specular_i18n::fixed(slope, 1);
    Some(match d.aspect(cx, cy) {
        Some(a) => t!(
            "map-slope-aspect",
            slope = slope_s,
            direction = compass_name(a),
            aspect = specular_i18n::fixed(a, 0)
        ),
        None => t!("map-slope-flat", slope = slope_s),
    })
}

/// The compass point's short name ("NE").
pub(super) fn compass_name(deg: f64) -> String {
    let id = ["n", "ne", "e", "se", "s", "sw", "w", "nw"][terrain::compass_point(deg)];
    t!("map-compass", point = id)
}

/// Contour lines of a height plane with their interval and unit.
#[derive(Clone, Debug)]
pub(super) struct Contours {
    pub levels: Levels,
    /// The heights' unit symbol for labels ("m", "ft", or empty).
    pub unit: String,
    /// In stored pixels of the image.
    pub lines: Arc<Vec<Contour>>,
    /// The plane they were traced on.
    pub plane: Arc<NdPlane>,
}

/// Traces contours of `p` (an image of `size` stored px) at an interval giving about
/// [`CONTOUR_LINES`] lines; a grid of more than [`CONTOUR_CELLS`] cells is averaged down.
pub(super) fn trace(p: &Arc<NdPlane>, size: (f64, f64), unit: &str) -> Option<Contours> {
    let (w, h) = (p.width as usize, p.height as usize);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &v in &p.values {
        if v.is_finite() {
            lo = lo.min(f64::from(v));
            hi = hi.max(f64::from(v));
        }
    }
    let interval = terrain::nice_interval(lo, hi, CONTOUR_LINES)?;
    let levels = Levels {
        base: 0.0,
        interval,
        index_every: INDEX_EVERY,
    };
    let f = ((w * h) as f64 / CONTOUR_CELLS as f64)
        .sqrt()
        .ceil()
        .max(1.0) as usize;
    let mut lines = if f > 1 {
        let (r, rw, rh) = terrain::reduce(&p.values, w, h, f);
        let mut c = terrain::contours(&r, rw, rh, &levels);
        for l in &mut c {
            for q in &mut l.points {
                *q = (q.0 * f as f32, q.1 * f as f32);
            }
        }
        c
    } else {
        terrain::contours(&p.values, w, h, &levels)
    };
    // Plane pixels to the image's stored pixels.
    let (kx, ky) = ((size.0 / w as f64) as f32, (size.1 / h as f64) as f32);
    if (kx - 1.0).abs() > 1e-6 || (ky - 1.0).abs() > 1e-6 {
        for l in &mut lines {
            for q in &mut l.points {
                *q = (q.0 * kx, q.1 * ky);
            }
        }
    }
    Some(Contours {
        levels,
        unit: unit.to_owned(),
        lines: Arc::new(lines),
        plane: p.clone(),
    })
}

/// A height as a contour label ("1250 m").
pub(super) fn level_text(level: f64, interval: f64, unit: &str) -> String {
    // As many decimals as the interval has (2.5 m: one).
    let decimals = (0..6)
        .find(|&d| {
            let k = interval * 10f64.powi(d);
            (k - k.round()).abs() < 1e-6
        })
        .unwrap_or(6) as usize;
    let v = format!("{level:.decimals$}");
    if unit.is_empty() {
        v
    } else {
        t!(
            "map-length",
            value = v,
            unit = specular_i18n::unit_symbol(unit)
        )
    }
}

/// Contour lines on screen (logical px) and where their labels go.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ContourView {
    /// (points, index contour).
    pub lines: Vec<(Vec<(f32, f32)>, bool)>,
    /// (label, centre).
    pub labels: Vec<(String, (f32, f32))>,
}

/// Places the contours on screen through `to_screen` (stored px to logical px) inside
/// `clip`, dropping points closer than [`MIN_STEP_PX`]; the index contours get a label at
/// the middle of their longest run on screen.
pub(super) fn view_of(
    c: &Contours,
    to_screen: impl Fn((f64, f64)) -> (f32, f32),
    clip: Rect,
) -> ContourView {
    let mut v = ContourView::default();
    let inside = |p: (f32, f32)| {
        p.0 >= clip.x - 20.0
            && p.1 >= clip.y - 20.0
            && p.0 <= clip.x + clip.w + 20.0
            && p.1 <= clip.y + clip.h + 20.0
    };
    for l in c.lines.iter() {
        let mut run: Vec<(f32, f32)> = Vec::new();
        let mut best: Option<(f32, (f32, f32))> = None;
        let mut flush = |run: &mut Vec<(f32, f32)>, v: &mut ContourView| {
            if run.len() > 1 {
                if l.index {
                    let len: f32 = run
                        .windows(2)
                        .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
                        .sum();
                    if best.is_none_or(|b| len > b.0) {
                        best = Some((len, run[run.len() / 2]));
                    }
                }
                v.lines.push((std::mem::take(run), l.index));
            } else {
                run.clear();
            }
        };
        for &(x, y) in &l.points {
            let q = to_screen((f64::from(x), f64::from(y)));
            if !inside(q) {
                flush(&mut run, &mut v);
                continue;
            }
            if let Some(last) = run.last()
                && (q.0 - last.0).hypot(q.1 - last.1) < MIN_STEP_PX
            {
                continue;
            }
            run.push(q);
        }
        flush(&mut run, &mut v);
        if let Some((len, at)) = best
            && len > 120.0
        {
            v.labels
                .push((level_text(l.level, c.levels.interval, &c.unit), at));
        }
    }
    v
}

/// The contour ink on the map.
pub(super) fn contour_ink(dark_paper: bool) -> Color {
    if dark_paper {
        Color::rgba(0.95, 0.82, 0.6, 0.85)
    } else {
        Color::rgba(0.45, 0.25, 0.08, 0.85)
    }
}

/// Draws contours and their labels (labels that would overlap one placed are left out).
pub(super) fn draw_contours(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &ContourView,
    clip: Rect,
    ts: f32,
) {
    // A light line with a dark edge reads over any colour of the map.
    let ink = contour_ink(true);
    let edge = Color::rgba(0.0, 0.0, 0.0, 0.45);
    dl.push_clip(clip);
    for (pts, index) in &v.lines {
        let w = if *index { 1.6 } else { 0.8 } * ts;
        for s in pts.windows(2) {
            dl.line(s[0], s[1], w + 1.4 * ts, edge);
        }
        for s in pts.windows(2) {
            dl.line(s[0], s[1], w, ink);
        }
    }
    let mut placed: Vec<Rect> = Vec::new();
    for (label, (x, y)) in &v.labels {
        let b = text.layout(
            label,
            TextStyle::new(FONT_SMALL * 0.9 * ts).tabular(),
            None,
            Align::Start,
        );
        let r = Rect::new(
            x - b.width / 2.0 - 3.0 * ts,
            y - b.height / 2.0 - 1.0 * ts,
            b.width + 6.0 * ts,
            b.height + 2.0 * ts,
        );
        if placed.iter().any(|p| overlaps(p, &r)) || !clip.contains((*x, *y)) {
            continue;
        }
        dl.rect(r, 3.0 * ts, pal.glass);
        text.draw(dl, &b, r.x + 3.0 * ts, r.y + 1.0 * ts, pal.text);
        placed.push(r);
    }
    dl.pop_clip();
}

/// Whether two rectangles overlap.
pub(super) fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

/// A legend: a title and a colour ramp with labelled ticks (slope, aspect, a weather map).
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Legend {
    pub title: String,
    /// The ramp's colours, display-coded, start to end.
    pub ramp: Vec<[f32; 3]>,
    /// (position 0..=1 along the ramp, label).
    pub ticks: Vec<(f32, String)>,
}

/// The legend of a slope or aspect view.
pub(super) fn relief_legend(kind: Relief) -> Option<Legend> {
    match kind {
        Relief::Shade => None,
        Relief::Slope => Some(Legend {
            title: t!("map-legend-slope"),
            ramp: (0..=32)
                .map(|i| SLOPE_MAP.sample(1.0 - i as f32 / 32.0))
                .collect(),
            ticks: [0.0, 15.0, 30.0, 45.0, SLOPE_MAX]
                .iter()
                .map(|d| {
                    let label = if *d == SLOPE_MAX {
                        t!("map-legend-degrees-more", degrees = *d as i64)
                    } else {
                        t!("map-legend-degrees", degrees = *d as i64)
                    };
                    (d / SLOPE_MAX, label)
                })
                .collect(),
        }),
        Relief::Aspect => Some(Legend {
            title: t!("map-legend-aspect"),
            ramp: (0..=32)
                .map(|i| ASPECT_MAP.sample(i as f32 / 32.0))
                .collect(),
            ticks: [0.0, 90.0, 180.0, 270.0, 360.0]
                .iter()
                .map(|d| (*d as f32 / 360.0, compass_name(*d)))
                .collect(),
        }),
    }
}

/// The legend's height at text scale `ts`.
pub(in crate::app) fn legend_height(ts: f32) -> f32 {
    2.0 * PAD + 3.0 * FONT_SMALL * ts + 14.0 * ts
}

/// Draws a legend at the start side of `area`, from `top` down (logical px).
#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn draw_legend(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    l: &Legend,
    area: (f32, f32, f32, f32),
    top: f32,
    rtl: bool,
    ts: f32,
) -> Rect {
    let (ax, _, aw, _) = area;
    let w = (220.0 * ts).min(aw - 2.0 * TOOLBAR_MARGIN);
    let h = legend_height(ts);
    let x = if rtl {
        ax + aw - TOOLBAR_MARGIN - w
    } else {
        ax + TOOLBAR_MARGIN
    };
    let r = Rect::new(x, top, w, h);
    dl.glass(r, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let st = TextStyle::new(FONT_SMALL * ts);
    let tb = text.layout(
        &l.title,
        st.weight(600.0),
        Some(w - 2.0 * PAD),
        Align::Start,
    );
    let tx = if rtl {
        r.x + r.w - PAD - tb.width
    } else {
        r.x + PAD
    };
    text.draw(dl, &tb, tx, r.y + PAD, pal.text);
    // The ramp runs with the reading direction.
    let ramp = Rect::new(
        r.x + PAD,
        r.y + PAD + tb.height + 4.0 * ts,
        w - 2.0 * PAD,
        10.0 * ts,
    );
    let n = l.ramp.len().max(1);
    for (i, c) in l.ramp.iter().enumerate() {
        let k = if rtl { n - 1 - i } else { i };
        let seg = Rect::new(
            ramp.x + ramp.w * k as f32 / n as f32,
            ramp.y,
            ramp.w / n as f32 + 0.5,
            ramp.h,
        );
        dl.rect(seg, 0.0, Color::rgba(c[0], c[1], c[2], 1.0));
    }
    dl.stroke_rect(ramp, 0.0, pal.glass_border, 1.0);
    // Labels that would run into the one before are left out (the last is kept).
    let mut taken: Vec<(f32, f32)> = Vec::new();
    let n = l.ticks.len();
    let mut order: Vec<usize> = (0..n).collect();
    if n > 1 {
        order.rotate_right(1);
        order.swap(0, 1);
    }
    for i in order {
        let (pos, label) = &l.ticks[i];
        let p = if rtl { 1.0 - pos } else { *pos };
        let px = ramp.x + ramp.w * p;
        dl.line(
            (px, ramp.y + ramp.h),
            (px, ramp.y + ramp.h + 3.0 * ts),
            ts,
            pal.text_muted,
        );
        let b = text.layout(label, st.tabular(), None, Align::Start);
        let lx = (px - b.width / 2.0).clamp(r.x + 2.0, r.x + r.w - 2.0 - b.width);
        let span = (lx - 4.0 * ts, lx + b.width + 4.0 * ts);
        if taken.iter().any(|t| span.0 < t.1 && t.0 < span.1) {
            continue;
        }
        taken.push(span);
        text.draw(dl, &b, lx, ramp.y + ramp.h + 4.0 * ts, pal.text_muted);
    }
    r
}

/// The surface of an elevation model for the mesh viewer: at most [`MESH_SIDE`] vertices
/// a side, metres east and north from the north-west corner and heights times
/// `exaggeration`, textured with `drape` (the map's colours, `dw`×`dh` RGBA covering the
/// model). Cells without data leave holes. `None` without enough data.
pub(super) fn surface(
    d: &Dem,
    exaggeration: f64,
    drape: (u32, u32, Vec<u8>),
    title: String,
) -> Option<specular_decode::MeshModel> {
    if d.w < 2 || d.h < 2 {
        return None;
    }
    let f = d.w.max(d.h).div_ceil(MESH_SIDE).max(1);
    let (nx, ny) = ((d.w - 1) / f + 1, (d.h - 1) / f + 1);
    let at = |i: usize, j: usize| d.values[(j * f).min(d.h - 1) * d.w + (i * f).min(d.w - 1)];
    let [sx, sy] = d.step;
    let mut positions = Vec::with_capacity(nx * ny);
    let mut uvs = Vec::with_capacity(nx * ny);
    let mut lo = f32::INFINITY;
    for j in 0..ny {
        for i in 0..nx {
            let v = at(i, j);
            if v.is_finite() {
                lo = lo.min(v);
            }
        }
    }
    if !lo.is_finite() {
        return None;
    }
    let ex = exaggeration * d.z;
    for j in 0..ny {
        for i in 0..nx {
            let (ci, cj) = ((i * f).min(d.w - 1), (j * f).min(d.h - 1));
            let v = at(i, j);
            let z = if v.is_finite() { v } else { lo };
            let x = (ci as f64 + 0.5) * sx.abs();
            // North is +y: rows running south go down.
            let y = -(cj as f64 + 0.5) * sy.abs() * if sy < 0.0 { 1.0 } else { -1.0 };
            positions.push([x as f32, y as f32, (f64::from(z - lo) * ex) as f32]);
            uvs.push([
                (ci as f32 + 0.5) / d.w as f32,
                (cj as f32 + 0.5) / d.h as f32,
            ]);
        }
    }
    let mut indices = Vec::new();
    let ok = |i: usize, j: usize| at(i, j).is_finite();
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            if !(ok(i, j) && ok(i + 1, j) && ok(i, j + 1) && ok(i + 1, j + 1)) {
                continue;
            }
            let a = (j * nx + i) as u32;
            let b = a + 1;
            let c = a + nx as u32;
            let e = c + 1;
            // Counter-clockwise seen from above (z up, y north).
            if sy < 0.0 {
                indices.extend([a, c, b, b, c, e]);
            } else {
                indices.extend([a, b, c, b, e, c]);
            }
        }
    }
    if indices.is_empty() {
        return None;
    }
    let normals = normals_of(&positions, &indices);
    let mut bounds = Bounds::default();
    for &i in &indices {
        bounds = bounds.union(&Bounds {
            min: positions[i as usize],
            max: positions[i as usize],
        });
    }
    let tris = (indices.len() / 3) as u32;
    let (dw, dh, rgba) = drape;
    let mut notes = Vec::new();
    if (exaggeration - 1.0).abs() > 1e-6 {
        notes.push(t!(
            "map-terrain-exaggeration",
            times = specular_i18n::fixed(exaggeration, 0)
        ));
    }
    let mesh = Mesh {
        format: None,
        positions,
        normals,
        colors: None,
        uvs: Some(uvs),
        indices,
        parts: vec![Part {
            first: 0,
            count: tris,
            material: Some(0),
        }],
        materials: vec![Material {
            name: t!("map-terrain-drape"),
            base_color: [1.0; 4],
            texture: Some(0),
        }],
        images: vec![ImageData::Missing {
            name: t!("map-terrain-drape"),
            reason: String::new(),
        }],
        unit: specular_mesh::Unit::Metre,
        up: specular_mesh::Up::Z,
        bounds,
        flat: false,
        file_normals: false,
        notes,
        title: Some(title),
        generator: None,
    };
    Some(specular_decode::MeshModel {
        mesh: Arc::new(mesh),
        textures: Arc::new(vec![Some(specular_mesh::preview::Texture {
            width: dw,
            height: dh,
            rgba,
        })]),
        ifc: None,
    })
}

/// Area-weighted vertex normals.
fn normals_of(p: &[[f32; 3]], idx: &[u32]) -> Vec<[f32; 3]> {
    let mut n = vec![[0.0f32; 3]; p.len()];
    for t in idx.chunks_exact(3) {
        let (a, b, c) = (p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let x = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for &k in t {
            let m = &mut n[k as usize];
            m[0] += x[0];
            m[1] += x[1];
            m[2] += x[2];
        }
    }
    for m in &mut n {
        let l = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
        *m = if l > 0.0 {
            [m[0] / l, m[1] / l, m[2] / l]
        } else {
            [0.0, 0.0, 1.0]
        };
    }
    n
}

/// A vertical exaggeration that makes relief readable: 1× unless the height range is under
/// an eighth of the model's width, then up to 10× (whole numbers).
pub(super) fn auto_exaggeration(d: &Dem) -> f64 {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &v in d.values {
        if v.is_finite() {
            lo = lo.min(f64::from(v));
            hi = hi.max(f64::from(v));
        }
    }
    let range = (hi - lo) * d.z;
    let width = (d.w as f64 * d.step[0].abs()).max(d.h as f64 * d.step[1].abs());
    if !(range > 0.0 && width > 0.0) {
        return 1.0;
    }
    (width / 8.0 / range).clamp(1.0, 10.0).floor()
}

/// The drape: the map's colours at most [`DRAPE_SIDE`] a side, `colour(x, y)` giving
/// display-coded RGB (or `None` for no data) at plane cell (`x`, `y`); contour lines burnt
/// in.
pub(super) fn drape(
    w: usize,
    h: usize,
    colour: impl Fn(usize, usize) -> Option<[f32; 3]>,
    contours: Option<&Contours>,
) -> (u32, u32, Vec<u8>) {
    let f = w.max(h).div_ceil(DRAPE_SIDE).max(1);
    let (dw, dh) = (w.div_ceil(f), h.div_ceil(f));
    let mut rgba = vec![0u8; dw * dh * 4];
    let grey = (FLAT_GREY * 255.0) as u8;
    for y in 0..dh {
        for x in 0..dw {
            let c = colour((x * f).min(w - 1), (y * f).min(h - 1));
            let o = (y * dw + x) * 4;
            match c {
                Some(c) => {
                    for k in 0..3 {
                        rgba[o + k] = (c[k].clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                }
                None => rgba[o..o + 3].copy_from_slice(&[grey; 3]),
            }
            rgba[o + 3] = 255;
        }
    }
    if let Some(c) = contours {
        let (kx, ky) = (
            dw as f32 / c.plane.width.max(1) as f32,
            dh as f32 / c.plane.height.max(1) as f32,
        );
        // The contours are in the image's stored px; the plane may be the image's size.
        for l in c.lines.iter() {
            let ink: [u8; 3] = if l.index { [60, 30, 10] } else { [110, 70, 40] };
            for s in l.points.windows(2) {
                let (a, b) = ((s[0].0 * kx, s[0].1 * ky), (s[1].0 * kx, s[1].1 * ky));
                let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as usize).max(1);
                for i in 0..=n {
                    let t = i as f32 / n as f32;
                    let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                    if x >= 0.0 && y >= 0.0 && (x as usize) < dw && (y as usize) < dh {
                        let o = (y as usize * dw + x as usize) * 4;
                        rgba[o..o + 3].copy_from_slice(&ink);
                    }
                }
            }
        }
    }
    (dw as u32, dh as u32, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_geodesy::seam::DegreeGrid;

    fn cone(n: u32) -> Arc<NdPlane> {
        let mut values = Vec::new();
        for y in 0..n {
            for x in 0..n {
                let (dx, dy) = (x as f32 - n as f32 / 2.0, y as f32 - n as f32 / 2.0);
                values.push(1000.0 - 10.0 * (dx * dx + dy * dy).sqrt());
            }
        }
        Arc::new(NdPlane {
            width: n,
            height: n,
            values,
            truncated: false,
        })
    }

    #[test]
    fn ground_steps_in_degrees() {
        let g = DegreeGrid {
            lon0: 8.0,
            dlon: 1.0 / 3600.0,
            lat0: 47.0,
            dlat: -1.0 / 3600.0,
            name: "WGS 84",
        };
        let [e, n] = ground_steps(&g, 10.0, 10.0).unwrap();
        assert!((e - 21.1).abs() < 0.2, "{e}");
        assert!((n + 30.9).abs() < 0.2, "{n}");
    }

    /// Ground steps through the map's CRS chain against PROJ (pyproj) and the WGS 84
    /// geodesic, for UTM, Swiss LV95, Web Mercator, degrees and a southern UTM zone
    /// (`crates/specular-geodesy/gen/terrain_oracle.py`).
    #[test]
    fn ground_steps_match_proj() {
        use specular_decode::geo::specular_gis::crs::from_epsg;
        use specular_decode::geo::specular_gis::geotiff::Affine2;
        use specular_decode::geo::{GeoFacts, GeoRef};
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/specular-geodesy/tests/data/ground-steps.tsv"
        ))
        .unwrap();
        let (mut n, mut worst) = (0, 0.0f64);
        for l in text.lines().filter(|l| !l.starts_with('#')) {
            let v: Vec<f64> = l.split('\t').map(|x| x.parse().unwrap()).collect();
            let g = super::super::seam::GeoRefTransform(Arc::new(GeoRef {
                crs: from_epsg(v[0] as u32),
                crs_note: None,
                to_map: Affine2 {
                    a: v[3],
                    b: 0.0,
                    c: v[1],
                    d: 0.0,
                    e: v[4],
                    f: v[2],
                },
                bounds: [0.0; 4],
                features: None,
                facts: GeoFacts::Raster(Default::default()),
            }));
            let [e, nn] = ground_steps(&g, v[5], v[6]).unwrap();
            let rel = ((e - v[7]) / v[7]).abs().max(((nn - v[8]) / v[8]).abs());
            worst = worst.max(rel);
            n += 1;
            assert!(rel < 1e-6, "{l}: ours {e} {nn}");
        }
        println!("ground steps: {n} points, worst relative difference {worst:.1e}");
        assert_eq!(n, 200);
    }

    #[test]
    fn contours_and_labels() {
        let p = cone(64);
        let c = trace(&p, (64.0, 64.0), "m").unwrap();
        assert_eq!(c.levels.interval, 20.0);
        assert!(c.lines.iter().any(|l| l.index && l.closed));
        let v = view_of(
            &c,
            |(x, y)| ((x * 8.0) as f32, (y * 8.0) as f32),
            Rect::new(0.0, 0.0, 512.0, 512.0),
        );
        assert!(!v.lines.is_empty());
        assert!(!v.labels.is_empty());
        assert!(
            v.labels.iter().all(|(l, _)| l.contains('m')),
            "{:?}",
            v.labels
        );
        assert_eq!(level_text(12.5, 2.5, ""), "12.5");
        assert_eq!(level_text(1200.0, 100.0, ""), "1200");
    }

    #[test]
    fn a_cone_in_3d() {
        let p = cone(40);
        let d = dem(&p, (40.0, 40.0), [10.0, -10.0], 1.0);
        let ex = auto_exaggeration(&d);
        assert!(ex >= 1.0);
        let tex = drape(40, 40, |_, _| Some([0.5, 0.6, 0.7]), None);
        let m = surface(&d, ex, tex, "cone".into()).unwrap();
        assert_eq!(m.mesh.positions.len(), 40 * 40);
        assert_eq!(m.mesh.indices.len(), 39 * 39 * 6);
        // The summit is the highest point and the normals there point up.
        let top = m.mesh.bounds.max[2];
        assert!(top > 0.0);
        let c = 20 * 40 + 20;
        assert!(m.mesh.normals[c][2] > 0.9, "{:?}", m.mesh.normals[c]);
        // North is +y: the first row (north edge) is the highest y.
        assert!(m.mesh.positions[0][1] > m.mesh.positions[39 * 40][1]);
        // Counter-clockwise from above: the first triangle's normal points up.
        let t = &m.mesh.indices[..3];
        let (a, b, cc) = (
            m.mesh.positions[t[0] as usize],
            m.mesh.positions[t[1] as usize],
            m.mesh.positions[t[2] as usize],
        );
        let z = (b[0] - a[0]) * (cc[1] - a[1]) - (b[1] - a[1]) * (cc[0] - a[0]);
        assert!(z > 0.0);
    }

    #[test]
    fn slope_readout() {
        let v: Vec<f32> = (0..25).map(|i| (i % 5) as f32 * 10.0).collect();
        let p = NdPlane {
            width: 5,
            height: 5,
            values: v,
            truncated: false,
        };
        let d = dem(&p, (5.0, 5.0), [10.0, -10.0], 1.0);
        let l = slope_line(&d, (5.0, 5.0), 2.5, 2.5).unwrap();
        let l = l.replace(['\u{2068}', '\u{2069}'], "");
        assert!(l.contains("45.0"), "{l}");
        assert!(l.contains("270"), "{l}");
    }
}
