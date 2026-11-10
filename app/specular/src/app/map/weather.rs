//! Weather grids in the map view: what a NetCDF or GRIB field is (temperature, rain, wind,
//! pressure, humidity, cloud, an anomaly), the colour maps that suit it, and wind arrows
//! from its eastward and northward components (u and v).
//!
//! The u and v planes come from the same session (a GRIB grid's parameter axis) or from
//! the file's other pages (a NetCDF variable each), at the position shown; they are read on
//! a background thread.

use std::sync::Arc;

use specular_decode::Session;
use specular_decode::science::{NdPlane, ScienceSummary};
use specular_geodesy::seam::GeoTransform;
use specular_gfx::{DrawList, Rect};
use specular_i18n::t;
use specular_inspect::ColourMap;
use specular_theme::Color;

/// Screen px between wind arrows.
pub(super) const ARROW_SPACING: f32 = 42.0;
/// The longest arrow, screen px (the fastest wind in view).
const ARROW_MAX: f32 = 34.0;

/// What a grid's values are, for its colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    Temperature,
    Precipitation,
    Wind,
    Pressure,
    Humidity,
    Cloud,
    /// Departures from a mean, either side of zero.
    Anomaly,
}

impl Field {
    /// The colour maps that suit the field, the default first.
    pub(super) fn maps(self) -> &'static [ColourMap] {
        use ColourMap as M;
        match self {
            Field::Temperature => &[M::Thermal, M::Roma, M::Batlow],
            Field::Precipitation => &[M::Rain, M::Dense, M::Oslo],
            Field::Wind => &[M::Speed, M::Lajolla, M::Viridis],
            Field::Pressure => &[M::Haline, M::Viridis, M::Deep],
            Field::Humidity => &[M::Dense, M::Haline, M::Ice],
            Field::Cloud => &[M::Ice, M::Oslo, M::Cividis],
            Field::Anomaly => &[M::Balance, M::Vik, M::Curl],
        }
    }
}

/// The weather colour maps for a grid whose field is not recognised.
pub(super) const GENERAL: &[ColourMap] = &[
    ColourMap::Thermal,
    ColourMap::Rain,
    ColourMap::Speed,
    ColourMap::Haline,
    ColourMap::Balance,
];

/// The text naming what a grid shows: its variable name, title, standard name and unit.
fn names(s: &ScienceSummary, index: &[u32]) -> (String, String) {
    let mut name = String::new();
    for t in [s.dataset.as_deref(), s.title.as_deref()]
        .into_iter()
        .flatten()
    {
        name.push_str(t);
        name.push(' ');
    }
    for (k, v, _) in s.keywords.iter() {
        if matches!(
            k.as_str(),
            "standard_name" | "long_name" | "GRIB_shortName" | "shortName"
        ) {
            name.push_str(v.trim_matches('"'));
            name.push(' ');
        }
    }
    // A GRIB grid's parameter at the position shown.
    if let Some(a) = param_axis(s)
        && let Some(l) = s.axes[a]
            .labels
            .get(index.get(a).copied().unwrap_or(0) as usize)
    {
        name.push_str(l);
    }
    (
        name.to_ascii_lowercase(),
        s.unit_at(index).unwrap_or_default().to_ascii_lowercase(),
    )
}

/// The axis of a GRIB grid's parameters.
fn param_axis(s: &ScienceSummary) -> Option<usize> {
    s.axes.iter().position(|a| a.name == "parameter")
}

/// What the grid at `index` shows, when it is a weather field.
pub(super) fn field(s: &ScienceSummary, index: &[u32]) -> Option<Field> {
    let (n, unit) = names(s, index);
    let has = |w: &[&str]| w.iter().any(|k| n.contains(k));
    let word = |w: &str| {
        n.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|x| x == w)
    };
    if has(&["anomal", "departure"]) {
        return Some(Field::Anomaly);
    }
    if has(&["temperature", "dewpoint", "dew point"])
        || word("t2m")
        || word("2t")
        || word("tmp")
        || word("sst")
        || (unit == "k" || unit == "degc" || unit == "°c" || unit == "c" || unit == "celsius")
    {
        return Some(Field::Temperature);
    }
    if has(&["precip", "rain", "snowfall", "convective_precipitation"])
        || word("tp")
        || word("pr")
        || word("apcp")
    {
        return Some(Field::Precipitation);
    }
    if has(&["wind", "gust"]) || word("10si") || word("ws") || word("u10") || word("v10") {
        return Some(Field::Wind);
    }
    if has(&["pressure", "geopotential"])
        || word("msl")
        || word("sp")
        || word("prmsl")
        || unit == "pa"
        || unit == "hpa"
    {
        return Some(Field::Pressure);
    }
    if has(&["humidity"]) || word("rh") || word("q") {
        return Some(Field::Humidity);
    }
    if has(&["cloud"]) || word("tcc") {
        return Some(Field::Cloud);
    }
    None
}

/// A colour map as a colour table (display-coded bytes) for the science view.
pub(in crate::app) fn table_of(m: ColourMap) -> Arc<specular_decode::science::ColourTable> {
    let mut t = [[0u8; 3]; 256];
    for (o, c) in t.iter_mut().zip(m.table().iter()) {
        *o = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    }
    Arc::new(t)
}

/// Where a grid's wind components are read from.
#[derive(Clone)]
pub(super) struct WindSource {
    pub u: (Arc<dyn Session>, Vec<u32>),
    pub v: (Arc<dyn Session>, Vec<u32>),
}

/// Whether a component's name is eastward (`'u'`) or northward (`'v'`) wind.
fn component(name: &str) -> Option<char> {
    let n = name.to_ascii_lowercase();
    if n.contains("eastward_wind")
        || n.contains("x_wind")
        || n.contains("u component")
        || n.contains("u-component")
    {
        return Some('u');
    }
    if n.contains("northward_wind")
        || n.contains("y_wind")
        || n.contains("v component")
        || n.contains("v-component")
    {
        return Some('v');
    }
    let first = n
        .split(|c: char| !c.is_ascii_alphanumeric())
        .find(|w| !w.is_empty())?;
    match first {
        "u" | "u10" | "10u" | "u100" | "100u" | "ugrd" | "uwnd" | "ua" | "uas" => Some('u'),
        "v" | "v10" | "10v" | "v100" | "100v" | "vgrd" | "vwnd" | "va" | "vas" => Some('v'),
        _ => None,
    }
}

/// The wind components of a grid shown at `index`: a GRIB grid's u and v parameters at
/// the same level and time, else the file's other variables named u and v with the same
/// shape. Opens the file's pages: call off the UI thread.
pub(super) fn find_wind(
    session: &Arc<dyn Session>,
    s: &ScienceSummary,
    index: &[u32],
) -> Option<WindSource> {
    if let Some(a) = param_axis(s) {
        let labels = &s.axes[a].labels;
        let find = |want: char| labels.iter().position(|l| component(l) == Some(want));
        if let (Some(u), Some(v)) = (find('u'), find('v')) {
            let mut iu = index.to_vec();
            let mut iv = index.to_vec();
            iu[a] = u as u32;
            iv[a] = v as u32;
            return Some(WindSource {
                u: (session.clone(), iu),
                v: (session.clone(), iv),
            });
        }
    }
    let pages = session.page_count().min(64);
    if pages < 2 {
        return None;
    }
    let (mut u, mut v) = (None, None);
    for p in 0..pages {
        let other = if p == session.page_index() {
            session.clone()
        } else {
            match session.open_page(p) {
                Ok(o) => o,
                Err(_) => continue,
            }
        };
        let Some(os) = other.science() else { continue };
        if (os.width, os.height) != (s.width, s.height) || os.axes.len() != s.axes.len() {
            continue;
        }
        let mut name = os.dataset.clone().unwrap_or_default();
        for (k, val, _) in os.keywords.iter() {
            if k == "standard_name" {
                name = format!("{} {name}", val.trim_matches('"'));
            }
        }
        let c = component(&name).or_else(|| component(os.dataset.as_deref().unwrap_or("")));
        let slot = match c {
            Some('u') => &mut u,
            Some('v') => &mut v,
            _ => continue,
        };
        if slot.is_none() {
            *slot = Some((other, index.to_vec()));
        }
    }
    Some(WindSource { u: u?, v: v? })
}

/// The u and v planes of a wind source.
pub(super) fn read_wind(w: &WindSource) -> Option<(Arc<NdPlane>, Arc<NdPlane>)> {
    let cancel = specular_core::CancelToken::new();
    let u = w.u.0.nd_plane(&w.u.1, &cancel)?.ok()?;
    let v = w.v.0.nd_plane(&w.v.1, &cancel)?.ok()?;
    (u.width == v.width && u.height == v.height).then_some((u, v))
}

/// The wind at a stored position (bilinear in the plane's cells): u, v in the planes'
/// unit.
pub(super) fn wind_at(
    u: &NdPlane,
    v: &NdPlane,
    size: (f64, f64),
    s: (f64, f64),
) -> Option<(f64, f64)> {
    let (w, h) = (u.width as usize, u.height as usize);
    if w == 0 || h == 0 {
        return None;
    }
    let fx = (s.0 / size.0 * w as f64 - 0.5).clamp(0.0, (w - 1) as f64);
    let fy = (s.1 / size.1 * h as f64 - 0.5).clamp(0.0, (h - 1) as f64);
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
    let mut out = [0.0; 2];
    for (k, p) in [u, v].iter().enumerate() {
        let (mut sum, mut wsum) = (0.0, 0.0);
        for (x, y, wt) in [
            (x0, y0, (1.0 - tx) * (1.0 - ty)),
            (x1, y0, tx * (1.0 - ty)),
            (x0, y1, (1.0 - tx) * ty),
            (x1, y1, tx * ty),
        ] {
            let a = p.values[y * w + x];
            if a.is_finite() && wt > 0.0 {
                sum += f64::from(a) * wt;
                wsum += wt;
            }
        }
        if wsum <= 0.0 {
            return None;
        }
        out[k] = sum / wsum;
    }
    Some((out[0], out[1]))
}

/// The direction the wind blows from (meteorological), degrees clockwise from north.
pub(super) fn from_direction(u: f64, v: f64) -> f64 {
    (u.atan2(v).to_degrees() + 180.0).rem_euclid(360.0)
}

/// An arrow before its length: its centre, its direction on screen and the speed.
type Placed = ((f32, f32), (f32, f32), f64);

/// One arrow on screen: its tail and head (logical px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Arrow {
    pub tail: (f32, f32),
    pub head: (f32, f32),
}

/// Arrows on a screen grid over `clip` (logical px): `stored_at` maps a screen point to
/// stored px (`None` off the map); the direction on screen follows the map's projection
/// through `geo` and `screen_of` (stored px to logical px). Lengths are relative to the
/// fastest wind among the arrows.
#[allow(clippy::too_many_arguments)]
pub(super) fn arrows(
    u: &NdPlane,
    v: &NdPlane,
    size: (f64, f64),
    geo: &dyn GeoTransform,
    clip: Rect,
    stored_at: impl Fn((f32, f32)) -> Option<(f64, f64)>,
    screen_of: impl Fn((f64, f64)) -> (f32, f32),
    spacing: f32,
) -> Vec<Arrow> {
    // (centre, unit direction on screen, speed).
    let mut raw: Vec<Placed> = Vec::new();
    let nx = (clip.w / spacing).floor() as usize;
    let ny = (clip.h / spacing).floor() as usize;
    for j in 0..ny.max(1) {
        for i in 0..nx.max(1) {
            let c = (
                clip.x + spacing * (i as f32 + 0.5),
                clip.y + spacing * (j as f32 + 0.5),
            );
            let Some(s) = stored_at(c) else { continue };
            let Some((uu, vv)) = wind_at(u, v, size, s) else {
                continue;
            };
            let speed = uu.hypot(vv);
            if !(speed.is_finite() && speed > 0.0) {
                continue;
            }
            // A step downwind of about a hundredth of a degree, on screen.
            let Some((lon, lat)) = geo.to_lonlat(s.0, s.1) else {
                continue;
            };
            let k = 0.01 / speed;
            let (lon2, lat2) = (
                lon + uu * k / lat.to_radians().cos().max(0.05),
                lat + vv * k,
            );
            let Some(q) = geo.from_lonlat(lon2, lat2) else {
                continue;
            };
            let (a, b) = (screen_of(s), screen_of(q));
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let n = dx.hypot(dy);
            if n <= 0.0 {
                continue;
            }
            raw.push((c, (dx / n, dy / n), speed));
        }
    }
    let max = raw.iter().map(|r| r.2).fold(0.0, f64::max);
    raw.into_iter()
        .map(|(c, d, sp)| {
            let len = (ARROW_MAX * (sp / max).sqrt() as f32).max(6.0);
            Arrow {
                tail: (c.0 - d.0 * len / 2.0, c.1 - d.1 * len / 2.0),
                head: (c.0 + d.0 * len / 2.0, c.1 + d.1 * len / 2.0),
            }
        })
        .collect()
}

/// Draws the arrows: a dark halo, then the shaft and head.
pub(super) fn draw_arrows(dl: &mut DrawList, a: &[Arrow], clip: Rect, ts: f32) {
    dl.push_clip(clip);
    let ink = Color::rgba(1.0, 1.0, 1.0, 0.95);
    let halo = Color::rgba(0.0, 0.0, 0.0, 0.55);
    for ar in a {
        let (dx, dy) = (ar.head.0 - ar.tail.0, ar.head.1 - ar.tail.1);
        let n = dx.hypot(dy).max(1e-3);
        let (ux, uy) = (dx / n, dy / n);
        let h = 5.0 * ts;
        let l = (
            ar.head.0 - ux * h - uy * h * 0.6,
            ar.head.1 - uy * h + ux * h * 0.6,
        );
        let r = (
            ar.head.0 - ux * h + uy * h * 0.6,
            ar.head.1 - uy * h - ux * h * 0.6,
        );
        for (w, c) in [(3.2 * ts, halo), (1.4 * ts, ink)] {
            dl.line(ar.tail, ar.head, w, c);
            dl.line(l, ar.head, w, c);
            dl.line(r, ar.head, w, c);
        }
    }
    dl.pop_clip();
}

/// The readout's wind line: "Wind 7.2 m/s from 245° (SW)".
pub(super) fn wind_line(u: f64, v: f64, unit: &str) -> String {
    let speed = u.hypot(v);
    let dir = from_direction(u, v);
    t!(
        "map-wind-at",
        speed = specular_i18n::fixed(speed, 1),
        unit = unit.to_owned(),
        degrees = specular_i18n::fixed(dir, 0),
        direction = super::terrain::compass_name(dir)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_decode::science::{AxisKind, NdAxis};
    use specular_geodesy::seam::DegreeGrid;

    fn summary(dataset: &str, unit: &str) -> ScienceSummary {
        ScienceSummary {
            width: 4,
            height: 3,
            dataset: Some(dataset.into()),
            unit: Some(unit.into()),
            ..Default::default()
        }
    }

    #[test]
    fn fields_by_name_and_unit() {
        assert_eq!(
            field(&summary("t2m — 2 metre temperature", "K"), &[]),
            Some(Field::Temperature)
        );
        assert_eq!(
            field(&summary("tp — Total precipitation", "m"), &[]),
            Some(Field::Precipitation)
        );
        assert_eq!(
            field(&summary("msl — Mean sea level pressure", "Pa"), &[]),
            Some(Field::Pressure)
        );
        assert_eq!(
            field(&summary("t2m_anomaly", "K"), &[]),
            Some(Field::Anomaly)
        );
        assert_eq!(field(&summary("elevation", "m"), &[]), None);
        // A GRIB grid's parameter axis.
        let mut s = summary("", "");
        s.dataset = None;
        s.axes = vec![NdAxis {
            name: "parameter".into(),
            len: 2,
            unit: String::new(),
            labels: vec![
                "10u (10 metre U wind component)".into(),
                "10v (10 metre V wind component)".into(),
            ],
            wcs_axis: None,
            linear: None,
            kind: AxisKind::Field,
        }];
        assert_eq!(field(&s, &[1]), Some(Field::Wind));
        assert_eq!(component("10u (10 metre U wind component)"), Some('u'));
        assert_eq!(component("v — northward_wind"), Some('v'));
        assert_eq!(component("vo — vorticity"), None);
        for f in [Field::Temperature, Field::Anomaly] {
            assert!(!f.maps().is_empty());
        }
        assert!(Field::Anomaly.maps()[0].is_diverging());
    }

    #[test]
    fn wind_direction_and_arrows() {
        // From the west (blowing east): 270°.
        assert!((from_direction(5.0, 0.0) - 270.0).abs() < 1e-9);
        // From the north: 0°.
        assert!(from_direction(0.0, -3.0).abs() < 1e-9);
        let plane = |v: f32| NdPlane {
            width: 10,
            height: 10,
            values: vec![v; 100],
            truncated: false,
        };
        let (u, v) = (plane(4.0), plane(0.0));
        let g = DegreeGrid {
            lon0: 0.0,
            dlon: 0.1,
            lat0: 50.0,
            dlat: -0.1,
            name: "WGS 84",
        };
        let a = arrows(
            &u,
            &v,
            (10.0, 10.0),
            &g,
            Rect::new(0.0, 0.0, 400.0, 400.0),
            |p| Some((f64::from(p.0) / 40.0, f64::from(p.1) / 40.0)),
            |s| ((s.0 * 40.0) as f32, (s.1 * 40.0) as f32),
            ARROW_SPACING,
        );
        assert!(!a.is_empty());
        for ar in &a {
            // Eastward on screen: head to the right of the tail, level.
            assert!(ar.head.0 > ar.tail.0 + 10.0, "{ar:?}");
            assert!((ar.head.1 - ar.tail.1).abs() < 1e-3, "{ar:?}");
        }
        let l = wind_line(-3.0, -4.0, "m/s").replace(['\u{2068}', '\u{2069}'], "");
        assert!(l.contains("5.0") && l.contains("37"), "{l}");
    }
}
