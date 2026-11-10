//! Geodata: where the pointer is on the map (map coordinates in the CRS's unit, and longitude
//! and latitude), a scale bar in metres, and the features of a vector map: the one under the
//! pointer is named in the readout, and a click lists its attributes in the info panel's
//! Feature group.
//!
//! GeoTIFFs shown as values (an elevation model) go through science mode, whose readout and
//! scale bar take the map position and metres from here ([`geo_position_lines`]); everything
//! else georeferenced (vector maps, RGB orthophotos, GeoPackage tiles) has its readout and
//! scale bar drawn by this module, in the same places.

use std::sync::Arc;

use specular_decode::geo::GeoRef;
use specular_decode::geo::specular_gis::crs::CrsKind;
use specular_decode::geo::specular_gis::scene::Hit;
use specular_i18n::t;
use specular_metadata::{SummaryGroup, SummaryItem};

use super::App;
use super::science::{Overlay, linear_scale_bar};
use crate::loader::ImageId;

/// Screen pixels around the pointer a feature is found within (points and lines).
const HIT_PX: f64 = 5.0;
/// Pointer travel (physical px) between press and release still counted as a click.
const CLICK_SLOP: f32 = 4.0;
/// Attributes listed for a clicked feature.
const MAX_ATTRIBUTES: usize = 200;

/// Geodata state for the current image.
#[derive(Default)]
pub(super) struct Geo {
    /// The image the state belongs to; its georeference once its session is open.
    id: Option<ImageId>,
    geo: Option<Arc<GeoRef>>,
    /// The session was asked (it had opened).
    checked: bool,
    /// The stored image's size (px), from the georeference's bounds.
    size: (f64, f64),
    readout: Vec<String>,
    hover: Option<Hit>,
    /// The clicked feature, listed in the info panel.
    picked: Option<Hit>,
    /// Where the primary button went down (physical px).
    press: Option<(f32, f32)>,
    /// Vector maps beside the image and geotagged photos, placed on it (stored px).
    overlay: Option<Arc<GeoOverlay>>,
}

/// Vector maps found beside a georeferenced image and the gallery's geotagged photos,
/// carried into the image's own CRS through WGS 84, in its stored pixels.
#[derive(Debug, Default)]
pub(crate) struct GeoOverlay {
    pub lines: Vec<Vec<(f32, f32)>>,
    pub pins: Vec<(f32, f32)>,
}

/// Vector files read beside an image at most, their size at most and the vertices kept.
const OVERLAY_FILES: usize = 8;
const OVERLAY_FILE_BYTES: u64 = 32 << 20;
const OVERLAY_VERTICES: usize = 500_000;

/// The overlay of `g` (the image at `path`): the vector maps in its folder and the photo
/// positions (latitude, longitude) that fall on it. Empty when the image's CRS cannot be
/// reached from WGS 84.
pub(crate) fn overlay_for(g: &GeoRef, path: &std::path::Path, photos: &[(f64, f64)]) -> GeoOverlay {
    use specular_decode::geo::specular_gis::{self, Geometry};
    let mut out = GeoOverlay::default();
    let Some(crs) = g
        .crs
        .as_ref()
        .filter(|c| c.convertible() && c.datum_shift.is_known())
    else {
        return out;
    };
    let Some(inv) = g.to_map.inverse() else {
        return out;
    };
    let (w, h) = stored_size(g);
    let inside = |p: (f64, f64)| p.0 >= -1.0 && p.1 >= -1.0 && p.0 <= w + 1.0 && p.1 <= h + 1.0;
    let place = |lon: f64, lat: f64| -> Option<(f64, f64)> {
        let (x, y) = crs.from_wgs84(lon, lat)?;
        let p = inv.apply(x, y);
        (p.0.is_finite() && p.1.is_finite()).then_some(p)
    };
    for &(lat, lon) in photos {
        if let Some(p) = place(lon, lat).filter(|p| inside(*p)) {
            out.pins.push((p.0 as f32, p.1 as f32));
        }
    }
    let Some(dir) = path.parent() else { return out };
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut files: Vec<_> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p != path)
        .filter_map(|p| {
            let ext = p.extension()?.to_string_lossy().to_ascii_lowercase();
            let kind = specular_gis::kind_from_extension(&ext)?;
            let meta = std::fs::metadata(&p).ok()?;
            (meta.is_file() && meta.len() <= OVERLAY_FILE_BYTES).then_some((p, kind))
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut vertices = 0usize;
    for (p, kind) in files.into_iter().take(OVERLAY_FILES) {
        let Ok(bytes) = std::fs::read(&p) else {
            continue;
        };
        let Ok(ds) = specular_gis::read(kind, &p, &bytes) else {
            continue;
        };
        let src = ds
            .crs
            .clone()
            .or_else(|| specular_gis::crs::from_epsg(4326));
        let Some(src) = src.filter(|c| c.convertible()) else {
            continue;
        };
        let to_px = |pt: [f64; 2]| -> Option<(f64, f64)> {
            let (lon, lat) = src
                .to_wgs84(pt[0], pt[1])
                .or_else(|| src.to_lonlat(pt[0], pt[1]))?;
            place(lon, lat)
        };
        let line = |pts: &[[f64; 2]], out: &mut GeoOverlay, vertices: &mut usize| {
            let mut cur: Vec<(f32, f32)> = Vec::new();
            let mut any_in = false;
            for &q in pts {
                match to_px(q) {
                    Some(p) => {
                        any_in |= inside(p);
                        cur.push((p.0 as f32, p.1 as f32));
                    }
                    None => {
                        if cur.len() > 1 && any_in {
                            *vertices += cur.len();
                            out.lines.push(std::mem::take(&mut cur));
                        }
                        cur.clear();
                    }
                }
            }
            if cur.len() > 1 && any_in {
                *vertices += cur.len();
                out.lines.push(cur);
            }
        };
        fn walk(g: &Geometry, f: &mut dyn FnMut(&[[f64; 2]], bool)) {
            match g {
                Geometry::Point(p) => f(std::slice::from_ref(p), true),
                Geometry::MultiPoint(v) => v.iter().for_each(|p| f(std::slice::from_ref(p), true)),
                Geometry::LineString(v) => f(v, false),
                Geometry::MultiLineString(v) | Geometry::Polygon(v) => {
                    v.iter().for_each(|l| f(l, false))
                }
                Geometry::MultiPolygon(v) => v.iter().flatten().for_each(|l| f(l, false)),
                Geometry::Collection(v) => v.iter().for_each(|g| walk(g, f)),
            }
        }
        for geom in ds
            .layers
            .iter()
            .flat_map(|l| &l.features)
            .filter_map(|f| f.geometry.as_ref())
        {
            if vertices > OVERLAY_VERTICES {
                break;
            }
            walk(geom, &mut |pts, point| {
                if point {
                    if let Some(p) = to_px(pts[0]).filter(|p| inside(*p)) {
                        out.pins.push((p.0 as f32, p.1 as f32));
                    }
                } else {
                    line(pts, &mut out, &mut vertices);
                }
            });
        }
    }
    out
}

/// The pointer's map position as readout lines: the map coordinates in the CRS's unit for a
/// projected CRS (or with no CRS), then longitude and latitude where they are known.
pub(super) fn geo_position_lines(g: &GeoRef, x: f64, y: f64) -> Vec<String> {
    let (mx, my) = g.map_at(x, y);
    let mut lines = Vec::new();
    let crs = g.crs.as_ref();
    lines.extend(geo_map_line(g, x, y));
    lines.extend(lonlat_line(crs, mx, my));
    lines
}

/// The map coordinates line of stored position (`x`, `y`) in the CRS's unit, for a
/// projected CRS (or with no CRS); `None` for longitude and latitude.
pub(super) fn geo_map_line(g: &GeoRef, x: f64, y: f64) -> Option<String> {
    let (mx, my) = g.map_at(x, y);
    let crs = g.crs.as_ref();
    if crs.is_some_and(|c| c.kind == CrsKind::Geographic) {
        return None;
    }
    let unit = crs
        .and_then(|c| c.unit.as_ref())
        .map(|u| match u.name.as_str() {
            "metre" | "meter" => "m".to_owned(),
            n => n.to_owned(),
        });
    Some(match unit {
        Some(u) => t!(
            "geo-map-position",
            x = specular_i18n::fixed_plain(mx, 2),
            y = specular_i18n::fixed_plain(my, 2),
            unit = specular_i18n::unit_symbol(&u)
        ),
        None => t!(
            "geo-map-position-plain",
            x = specular_i18n::fixed_plain(mx, 3),
            y = specular_i18n::fixed_plain(my, 3)
        ),
    })
}

/// The longitude and latitude line of map position (`mx`, `my`): on WGS 84 where the
/// datum's shift is known, else on the CRS's own datum (named), or a plain statement that
/// this projection cannot be converted. `None` without a CRS.
pub(super) fn lonlat_line(
    crs: Option<&specular_decode::geo::specular_gis::crs::Crs>,
    mx: f64,
    my: f64,
) -> Option<String> {
    let c = crs?;
    if c.kind == CrsKind::Other {
        return None;
    }
    if !c.convertible() {
        return Some(t!("geo-unsupported"));
    }
    let fmt = |v: f64| specular_i18n::fixed_plain(v, 6);
    let finite = |p: &(f64, f64)| p.0.is_finite() && p.1.is_finite();
    if let Some((lon, lat)) = c.to_wgs84(mx, my).filter(finite) {
        return Some(t!("geo-lonlat", lon = fmt(lon), lat = fmt(lat)));
    }
    let (lon, lat) = c.to_lonlat(mx, my).filter(finite)?;
    Some(match c.datum.as_deref().or(c.geographic.as_deref()) {
        Some(d) => t!(
            "geo-lonlat-datum",
            lon = fmt(lon),
            lat = fmt(lat),
            datum = d.to_owned()
        ),
        None => t!("geo-lonlat", lon = fmt(lon), lat = fmt(lat)),
    })
}

/// The scale bar of a georeferenced image at stored position (`x`, `y`), in metres:
/// `px_per_pixel` logical px per stored pixel.
pub(super) fn geo_scale_bar(
    g: &GeoRef,
    x: f64,
    y: f64,
    px_per_pixel: f64,
) -> Option<super::science::ScaleBar> {
    let m = g.metres_per_pixel(x, y)?;
    linear_scale_bar(m, "m", px_per_pixel)
}

/// The stored image's size in px: the map bounds' corners in stored pixels.
fn stored_size(g: &GeoRef) -> (f64, f64) {
    let Some(inv) = g.to_map.inverse() else {
        return (0.0, 0.0);
    };
    let b = g.bounds;
    let (mut w, mut h) = (0.0f64, 0.0f64);
    for (x, y) in [(b[0], b[1]), (b[2], b[1]), (b[0], b[3]), (b[2], b[3])] {
        let (px, py) = inv.apply(x, y);
        w = w.max(px.round());
        h = h.max(py.round());
    }
    (w, h)
}

/// A feature's name for the readout: its name-like attribute, else its id, else its number
/// in its layer.
pub(super) fn feature_name(g: &GeoRef, hit: Hit) -> Option<String> {
    let idx = g.features.as_ref()?;
    let layer = idx.dataset.layers.get(hit.layer)?;
    let attrs = layer.attributes(hit.feature);
    let named = ["name", "title", "label", "id"].iter().find_map(|want| {
        attrs
            .iter()
            .find(|(k, v)| k.eq_ignore_ascii_case(want) && !v.text().trim().is_empty())
            .map(|(_, v)| v.text())
    });
    let name = named
        .or_else(|| layer.features.get(hit.feature)?.id.clone())
        .unwrap_or_else(|| format!("#{}", hit.feature + 1));
    Some(if idx.dataset.layers.len() > 1 && !layer.name.is_empty() {
        t!(
            "geo-feature-in-layer",
            name = name,
            layer = layer.name.clone()
        )
    } else {
        t!("geo-feature", name = name)
    })
}

/// The info panel's Feature group: the clicked feature's attributes.
fn feature_group(g: &GeoRef, hit: Hit) -> Option<SummaryGroup> {
    let idx = g.features.as_ref()?;
    let layer = idx.dataset.layers.get(hit.layer)?;
    let f = layer.features.get(hit.feature)?;
    let mut items = Vec::new();
    if idx.dataset.layers.len() > 1 {
        items.push(SummaryItem {
            key: "layer",
            label: t!("geo-feature-layer"),
            // An unnamed layer reads as the layers panel names it ("Layer 2").
            value: super::cad::shown_name(
                &layer.name,
                layer.name.is_empty().then_some(
                    specular_decode::drawing::specular_vector::Generated::Layer(
                        hit.layer as u32 + 1,
                    ),
                ),
            ),
            detail: None,
        });
    }
    if let Some(geom) = &f.geometry {
        items.push(SummaryItem {
            key: "geometry",
            label: t!("geo-feature-geometry"),
            value: t!(
                "geo-feature-geometry-value",
                kind = geom.type_name(),
                vertices = geom.vertex_count() as i64
            ),
            detail: None,
        });
    }
    if let Some(id) = &f.id {
        items.push(SummaryItem {
            key: "feature_id",
            label: t!("geo-feature-id"),
            value: id.clone(),
            detail: None,
        });
    }
    for (k, v) in layer
        .attributes(hit.feature)
        .into_iter()
        .take(MAX_ATTRIBUTES)
    {
        let text = v.text();
        if text.trim().is_empty() {
            continue;
        }
        items.push(SummaryItem {
            key: "attribute",
            label: k,
            value: text,
            detail: None,
        });
    }
    Some(SummaryGroup {
        key: "feature",
        title: t!("geo-feature-group"),
        items,
    })
}

impl App {
    /// Keeps the geodata state in step with the current image (its session's georeference).
    pub(super) fn geo_sync(&mut self) {
        let cur = self.current_id();
        if cur != self.geo.id {
            self.geo = Geo {
                id: cur,
                ..Default::default()
            };
            self.redraw = true;
        }
        if self.geo.checked {
            return;
        }
        let Some(session) = cur
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.as_ref())
        else {
            return;
        };
        self.geo.geo = session.geo();
        self.geo.checked = true;
        self.geo.size = self.geo.geo.as_deref().map_or((0.0, 0.0), stored_size);
        // Vector maps beside it and geotagged photos, placed in the background.
        if let (Some(g), Some(id)) = (self.geo.geo.clone(), cur)
            && g.crs.as_ref().is_some_and(|c| c.convertible())
            && specular_decode::geo::on_disk_path(session.source())
        {
            let path = session.source().path().to_path_buf();
            let photos = self.gallery.photo_positions();
            self.loader.background("geo-overlay", move |l| {
                let overlay = overlay_for(&g, &path, &photos);
                if !overlay.lines.is_empty() || !overlay.pins.is_empty() {
                    l.post(crate::loader::Event::GeoOverlay {
                        id,
                        overlay: Arc::new(overlay),
                    });
                }
            });
        }
        if self.geo.geo.is_some() {
            let p = self.pointer;
            self.geo_readout_at(p);
        }
    }

    /// The current image's georeference (not over the gallery).
    pub(super) fn geo_current(&self) -> Option<&Arc<GeoRef>> {
        if self.gallery.active || self.geo.id != self.current_id() {
            return None;
        }
        self.geo.geo.as_ref()
    }

    /// Science mode shows this image (its readout and scale bar carry the geodata).
    fn geo_in_science(&self) -> bool {
        self.science_state().is_some()
    }

    /// The stored position under screen point `p`, within the image.
    fn geo_stored_at(&self, p: Option<(f32, f32)>) -> Option<(f64, f64)> {
        let (w, h) = self.geo.size;
        p.filter(|p| super::rect_contains(self.view.viewport, *p))
            .map(|p| self.view.stored_from_screen().apply(p))
            .map(|(x, y)| (f64::from(x), f64::from(y)))
            .filter(|&(x, y)| x >= 0.0 && y >= 0.0 && x < w && y < h)
    }

    /// The feature under screen point `p`.
    fn geo_hit_at(&self, p: (f32, f32)) -> Option<Hit> {
        let g = self.geo_current()?;
        let idx = g.features.as_ref()?;
        let (x, y) = self.geo_stored_at(Some(p))?;
        let map = g.map_at(x, y);
        let (px, py) = g.to_map.pixel_size();
        let scale = f64::from(self.view.scale()).max(1e-9);
        let tol = HIT_PX / scale * px.abs().max(py.abs());
        idx.hit([map.0, map.1], tol)
    }

    /// Rebuilds the readout for the pointer at `p` (screen px): the map position and the
    /// feature under it. Redraws when it changed.
    fn geo_readout_at(&mut self, p: Option<(f32, f32)>) {
        let Some(g) = self.geo_current().cloned() else {
            return;
        };
        let at = self.geo_stored_at(p);
        let hover = p.and_then(|p| self.geo_hit_at(p));
        let mut lines = Vec::new();
        if !self.geo_in_science()
            && let Some((x, y)) = at
        {
            // The map view writes positions in the chosen format.
            lines = self
                .gis_position_lines(x, y)
                .unwrap_or_else(|| geo_position_lines(&g, x, y));
        }
        if let Some(name) = hover.and_then(|h| feature_name(&g, h)) {
            lines.push(name);
        }
        if lines != self.geo.readout || hover != self.geo.hover {
            self.geo.readout = lines;
            self.geo.hover = hover;
            self.redraw = true;
        }
    }

    /// The readout read out to screen readers (the map position and the feature under the
    /// pointer). `None` without a georeferenced image, or in science mode (its readout has
    /// the position).
    pub(super) fn geo_cursor_lines(&self) -> Option<&[String]> {
        (self.geo_current().is_some() && !self.geo_in_science()).then_some(&self.geo.readout[..])
    }

    /// Pointer motion over the viewer: the readout follows. Never takes the event.
    pub(super) fn geo_motion(&mut self, p: (f32, f32)) {
        if self.geo_current().is_some() {
            self.geo_readout_at(Some(p));
        }
    }

    /// The primary button over the viewer: a click (no drag) picks the feature under it and
    /// lists its attributes in the info panel (a click on no feature clears them). Never takes
    /// the event.
    pub(super) fn geo_button(&mut self, p: (f32, f32), down: bool) {
        if self.geo_current().is_none_or(|g| g.features.is_none()) {
            return;
        }
        if down {
            self.geo.press = Some(p);
            return;
        }
        let Some(q) = self.geo.press.take() else {
            return;
        };
        if (p.0 - q.0).hypot(p.1 - q.1) > CLICK_SLOP {
            return;
        }
        let hit = self.geo_hit_at(p);
        if hit != self.geo.picked {
            self.geo.picked = hit;
            self.geo_patch();
        }
    }

    /// Adds (or replaces, or removes) the Feature group in the current image's metadata.
    pub(super) fn geo_patch(&mut self) {
        let Some(id) = self.geo.id else { return };
        let group = self
            .geo
            .geo
            .as_deref()
            .zip(self.geo.picked)
            .and_then(|(g, h)| feature_group(g, h));
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        let Some(r) = img.metadata.as_ref() else {
            return;
        };
        let has = r.summary.iter().any(|g| g.key == "feature");
        if group.is_none() && !has {
            return;
        }
        let mut r = (**r).clone();
        r.summary.retain(|g| g.key != "feature");
        if let Some(g) = group {
            // After the Geospatial group, else first.
            let at = r
                .summary
                .iter()
                .position(|g| g.key == "geospatial")
                .map_or(0, |i| i + 1);
            r.summary.insert(at, g);
        }
        img.metadata = Some(Arc::new(r));
        self.redraw = true;
    }

    /// The readout's lines when it shows (science mode shows its own).
    pub(super) fn geo_lines(&self) -> Option<Vec<String>> {
        self.geo_current()?;
        (!self.geo.readout.is_empty()).then(|| self.geo.readout.clone())
    }

    /// The placed overlay came in.
    pub(super) fn on_geo_overlay(&mut self, id: ImageId, overlay: Arc<GeoOverlay>) {
        if self.geo.id == Some(id) {
            self.geo.overlay = Some(overlay);
            self.redraw = true;
        }
    }

    /// The vector maps and photos over the image, in logical px (contours and markers).
    pub(super) fn geo_overlay_marks(
        &self,
    ) -> (
        Vec<super::science::GridLine>,
        Vec<super::science::sky::Marker>,
    ) {
        let (Some(_), Some(o)) = (self.geo_current(), self.geo.overlay.as_ref()) else {
            return (Vec::new(), Vec::new());
        };
        let ls = self.dl.scale.max(0.1);
        let to_screen = self.view.stored_from_screen().invert();
        let place = |p: (f32, f32)| {
            let q = to_screen.apply(p);
            (q.0 / ls, q.1 / ls)
        };
        let lines = o
            .lines
            .iter()
            .map(|l| super::science::GridLine {
                pts: l.iter().map(|&p| place(p)).collect(),
                label: String::new(),
            })
            .collect();
        let pins = o
            .pins
            .iter()
            .enumerate()
            .map(|(i, &p)| super::science::sky::Marker {
                at: place(p),
                cat: 0,
                row: i as u32,
            })
            .collect();
        (lines, pins)
    }

    /// The scale bar of a georeferenced image outside science mode, with the overlay.
    pub(super) fn geo_overlay(&self) -> Option<Overlay> {
        if self.geo_in_science() {
            return None;
        }
        let g = self.geo_current()?;
        let ls = self.dl.scale.max(0.1);
        let px_per_pixel = f64::from(self.view.scale() / ls);
        let vp = self.view.viewport;
        let c = self
            .view
            .stored_from_screen()
            .apply((vp.x + vp.w / 2.0, vp.y + vp.h / 2.0));
        let scale = self
            .gis_scale_bar()
            .or_else(|| geo_scale_bar(g, f64::from(c.0), f64::from(c.1), px_per_pixel));
        let (contours, markers) = self.geo_overlay_marks();
        if scale.is_none() && contours.is_empty() && markers.is_empty() {
            return None;
        }
        Some(Overlay {
            sliders: Vec::new(),
            scale,
            contours,
            markers,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_decode::geo::GeoFacts;
    use specular_decode::geo::specular_gis::crs::from_epsg;
    use specular_decode::geo::specular_gis::geotiff::Affine2;

    fn utm() -> GeoRef {
        GeoRef {
            crs: from_epsg(32633),
            crs_note: None,
            to_map: Affine2 {
                a: 10.0,
                b: 0.0,
                c: 500_000.0,
                d: 0.0,
                e: -10.0,
                f: 5_000_000.0,
            },
            bounds: [500_000.0, 4_995_000.0, 507_000.0, 5_000_000.0],
            features: None,
            facts: GeoFacts::Raster(Default::default()),
        }
    }

    #[test]
    fn position_lines_give_map_and_lonlat() {
        let l = geo_position_lines(&utm(), 0.0, 0.0);
        assert_eq!(l.len(), 2, "{l:?}");
        assert!(
            l[0].contains("500000.00") && l[0].contains("5000000.00"),
            "{l:?}"
        );
        assert!(l[0].contains('m'), "{l:?}");
        // 15° E is UTM 33's central meridian: easting 500 000 m lies on it.
        assert!(l[1].contains("15.000000"), "{l:?}");
    }

    /// A GeoJSON track beside a UTM image and a photo taken on it land on the image, carried
    /// from WGS 84 into UTM zone 33N (10 m pixels from 500 000 E, 5 000 000 N).
    #[test]
    fn neighbours_and_photos_are_placed_on_the_image() {
        let dir = std::env::temp_dir().join(format!("specular-geo-overlay-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let g = utm();
        let crs = g.crs.clone().unwrap();
        let (lon0, lat0) = crs.to_wgs84(500_100.0, 4_999_900.0).unwrap();
        let (lon1, lat1) = crs.to_wgs84(500_600.0, 4_999_400.0).unwrap();
        let track = format!(
            r#"{{"type":"FeatureCollection","features":[{{"type":"Feature","properties":{{}},"geometry":{{"type":"LineString","coordinates":[[{lon0},{lat0}],[{lon1},{lat1}]]}}}}]}}"#
        );
        std::fs::write(dir.join("track.geojson"), track).unwrap();
        let image = dir.join("ortho.tif");
        let o = overlay_for(&g, &image, &[(lat1, lon1), (10.0, 10.0)]);
        assert_eq!(o.lines.len(), 1, "{o:?}");
        let l = &o.lines[0];
        assert!(
            (l[0].0 - 10.0).abs() < 1e-3 && (l[0].1 - 10.0).abs() < 1e-3,
            "{l:?}"
        );
        assert!(
            (l[1].0 - 60.0).abs() < 1e-3 && (l[1].1 - 60.0).abs() < 1e-3,
            "{l:?}"
        );
        // The photo at the track's end; the one far away is left out.
        assert_eq!(o.pins.len(), 1);
        assert!((o.pins[0].0 - 60.0).abs() < 1e-3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scale_bar_is_in_metres() {
        // 10 m pixels at 1 logical px each: a 1 km bar is 100 px.
        let b = geo_scale_bar(&utm(), 350.0, 250.0, 1.0).expect("a bar");
        assert!(
            b.label.contains("km") || b.label.contains('m'),
            "{}",
            b.label
        );
        assert!(b.px > 8.0 && b.px <= 120.0, "{}", b.px);
    }
}
