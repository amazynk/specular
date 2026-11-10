//! GRIB editions 1 and 2 (`specular-grib`) through the N-D science path.
//!
//! Every message is a plane on its grid. The messages on one grid make a page; within it
//! the parameters, levels, forecast times and ensemble members that vary become sliders
//! (a combination the file lacks shows as all missing). Lat/lon, Gaussian, rotated, polar
//! stereographic and Lambert grids carry a georeference for the readout and the coastlines.

use std::collections::HashMap;
use std::sync::Arc;

use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_grib::{Field, Grid};

use crate::geo::{GeoFacts, GeoRef, RasterGeo};
use crate::science::{self, AxisKind, NdAxis, NdPlane, PlaneCache, ScienceKind, ScienceSummary};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

/// Grid points past which a grid is refused. A constant field (no packed bits) needs no
/// data, so a few bytes can declare any `Ni`×`Nj`, and every plane is that many `f32`s.
const MAX_POINTS: u64 = specular_grib::MAX_POINTS as u64;

pub struct GribDecoder;

impl Decoder for GribDecoder {
    fn format(&self) -> Format {
        Format::Grib
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_grib::sniff(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["grib", "grb", "grib1", "grib2", "grb1", "grb2"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let model = Arc::new(Model::scan(source.bytes())?);
        Ok(Arc::new(GribSession::new(source, model, 0)?))
    }
}

fn grib_err(e: specular_grib::Error) -> DecodeError {
    match e {
        specular_grib::Error::Unsupported(s) => DecodeError::Unsupported(s),
        specular_grib::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        e => DecodeError::Malformed(e.to_string()),
    }
}

/// The messages on one grid.
struct Page {
    grid: Arc<Grid>,
    /// Parameter labels and units, in file order.
    params: Vec<(String, String)>,
    levels: Vec<String>,
    times: Vec<String>,
    members: Vec<String>,
    /// Field index by (parameter, level, time, member) position.
    map: HashMap<[usize; 4], usize>,
}

struct Model {
    fields: Vec<Field>,
    pages: Vec<Page>,
}

/// Distinct keys in first-seen order.
fn distinct<K: PartialEq + Clone>(keys: impl Iterator<Item = K>) -> Vec<K> {
    let mut out: Vec<K> = Vec::new();
    for k in keys {
        if !out.contains(&k) {
            out.push(k);
        }
    }
    out
}

fn time_key(f: &Field) -> (i64, i64, Option<i64>, Option<&'static str>) {
    (
        f.reference.unix() + f.forecast.valid_seconds(),
        f.forecast.start,
        f.forecast.end,
        f.forecast.statistic,
    )
}

fn member_label(m: Option<u32>) -> String {
    match m {
        Some(m) => format!("member {m}"),
        None => "—".to_owned(),
    }
}

impl Model {
    fn scan(data: &[u8]) -> Result<Model> {
        let fields = specular_grib::scan(data).map_err(grib_err)?;
        let mut grids: Vec<Arc<Grid>> = Vec::new();
        for f in &fields {
            if !grids.iter().any(|g| Arc::ptr_eq(g, &f.grid)) {
                grids.push(f.grid.clone());
            }
        }
        let pages = grids
            .into_iter()
            .map(|grid| {
                let on: Vec<usize> = (0..fields.len())
                    .filter(|&i| Arc::ptr_eq(&fields[i].grid, &grid))
                    .collect();
                let params = distinct(
                    on.iter()
                        .map(|&i| (fields[i].param.label(), fields[i].param.units.clone())),
                );
                let mut levels = distinct(on.iter().map(|&i| {
                    let l = &fields[i].level;
                    (l.sort_key(), l.label())
                }));
                levels.sort_by(|a, b| {
                    (a.0.0)
                        .cmp(&b.0.0)
                        .then(a.0.1.total_cmp(&b.0.1))
                        .then(a.1.cmp(&b.1))
                });
                let mut times = distinct(on.iter().map(|&i| time_key(&fields[i])));
                times.sort();
                let mut members =
                    distinct(on.iter().map(|&i| fields[i].ensemble.map(|e| e.member)));
                members.sort();
                let mut map = HashMap::new();
                for &i in &on {
                    let f = &fields[i];
                    let p = params.iter().position(|p| p.0 == f.param.label());
                    let l = levels.iter().position(|l| l.1 == f.level.label());
                    let t = times.iter().position(|t| *t == time_key(f));
                    let m = members
                        .iter()
                        .position(|m| *m == f.ensemble.map(|e| e.member));
                    if let (Some(p), Some(l), Some(t), Some(m)) = (p, l, t, m) {
                        map.entry([p, l, t, m]).or_insert(i);
                    }
                }
                let times = times
                    .iter()
                    .map(|t| {
                        let f = &fields[on
                            .iter()
                            .copied()
                            .find(|&i| time_key(&fields[i]) == *t)
                            .unwrap_or(on[0])];
                        format!("{} ({})", f.valid(), f.forecast.label())
                    })
                    .collect();
                Page {
                    grid,
                    params,
                    levels: levels.into_iter().map(|l| l.1).collect(),
                    times,
                    members: members.into_iter().map(member_label).collect(),
                    map,
                }
            })
            .collect();
        Ok(Model { fields, pages })
    }
}

/// A centre's name (WMO common code table C-11), for the commonest producers.
fn centre_name(c: u16) -> Option<&'static str> {
    Some(match c {
        7 => "NCEP",
        34 => "JMA",
        54 => "Canadian Meteorological Centre",
        58 => "US Navy FNMOC",
        74 => "UK Met Office",
        78 => "DWD",
        80 => "CNMCA Rome",
        82 => "SMHI",
        84 | 85 => "Météo-France",
        86 => "FMI",
        88 => "MET Norway",
        94 => "DMI",
        96 => "HNMS Athens",
        98 => "ECMWF",
        99 => "KNMI",
        _ => return None,
    })
}

struct GribSession {
    source: Arc<Source>,
    model: Arc<Model>,
    page: u32,
    info: ImageInfo,
    summary: ScienceSummary,
    /// Which positions of [parameter, level, time, member] the summary's axes stand for.
    axis_of: Vec<usize>,
    planes: PlaneCache,
    levels: Levels,
}

fn kw(k: &str, v: String) -> (String, String, String) {
    (k.to_owned(), v, String::new())
}

impl GribSession {
    fn new(source: Arc<Source>, model: Arc<Model>, page: u32) -> Result<GribSession> {
        let pg = model
            .pages
            .get(page as usize)
            .ok_or_else(|| DecodeError::Malformed("no such grid".into()))?;
        let (w, h) = pg.grid.plane_size();
        if u64::from(w) * u64::from(h) > MAX_POINTS {
            return Err(DecodeError::TooLarge);
        }
        let first = &model.fields[*pg.map.values().min().unwrap_or(&0)];
        let page_fields: Vec<&Field> = model
            .fields
            .iter()
            .filter(|f| Arc::ptr_eq(&f.grid, &pg.grid))
            .collect();

        let mut axes = Vec::new();
        let mut axis_of = Vec::new();
        let mut units_by = None;
        let lists: [(&str, Vec<String>); 4] = [
            ("parameter", pg.params.iter().map(|p| p.0.clone()).collect()),
            ("level", pg.levels.clone()),
            ("time", pg.times.clone()),
            ("member", pg.members.clone()),
        ];
        for (k, (name, labels)) in lists.into_iter().enumerate() {
            if labels.len() < 2 {
                continue;
            }
            if k == 0 {
                units_by = Some((axes.len(), pg.params.iter().map(|p| p.1.clone()).collect()));
            }
            axis_of.push(k);
            axes.push(NdAxis {
                name: name.into(),
                len: labels.len() as u32,
                unit: String::new(),
                labels,
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Cube,
            });
        }
        let unit = (pg.params.len() == 1)
            .then(|| pg.params[0].1.clone())
            .filter(|u| !u.is_empty());

        let mut packings = distinct(page_fields.iter().map(|f| f.packing.name()));
        packings.truncate(4);
        let editions = distinct(page_fields.iter().map(|f| f.edition));
        let sample = format!(
            "GRIB{}, {}",
            editions
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("/"),
            packings.join(", ")
        );

        let grid = &pg.grid;
        let mut keywords = vec![
            kw("Grid", grid.name()),
            kw(
                "Points",
                if grid.pl.is_some() {
                    format!("{} (shown as {w} × {h})", grid.points())
                } else {
                    format!("{w} × {h}")
                },
            ),
        ];
        if let Some(s) = grid.spacing() {
            keywords.push(kw("Spacing", s));
        }
        if let Some(r) = &grid.rotation {
            keywords.push(kw(
                "Rotated pole",
                format!(
                    "{:.4}° lat, {:.4}° lon, {}° rotation",
                    r.pole_lat, r.pole_lon, r.angle
                ),
            ));
        }
        keywords.push(kw("Earth", grid.earth.name.clone()));
        keywords.push(kw(
            "Centre",
            match centre_name(first.centre) {
                Some(n) => format!("{n} ({})", first.centre),
                None => first.centre.to_string(),
            },
        ));
        let refs = distinct(page_fields.iter().map(|f| f.reference));
        keywords.push(kw(
            "Reference time",
            match refs.as_slice() {
                [r] => format!("{r} UTC"),
                [a, .., b] => format!("{a} … {b} UTC ({} times)", refs.len()),
                [] => String::new(),
            },
        ));
        keywords.push(kw("Packing", packings.join(", ")));
        keywords.push(kw("Messages", page_fields.len().to_string()));
        if let Some(size) = page_fields
            .iter()
            .find_map(|f| f.ensemble.and_then(|e| e.size))
        {
            keywords.push(kw("Ensemble size", size.to_string()));
        }
        for (label, units) in &pg.params {
            keywords.push(("Parameter".into(), label.clone(), units.clone()));
        }
        if pg.levels.len() == 1 {
            keywords.push(kw("Level", pg.levels[0].clone()));
        }
        if pg.times.len() == 1 {
            keywords.push(kw("Valid", format!("{} UTC", pg.times[0])));
        }

        let band_name = (pg.params.len() == 1).then(|| pg.params[0].0.clone());
        let geo = grid.geo().map(|g| {
            Arc::new(GeoRef {
                crs: Some(g.crs),
                crs_note: g.note,
                to_map: g.to_map,
                bounds: g.bounds,
                features: None,
                facts: GeoFacts::Raster(RasterGeo {
                    levels: vec![(w, h)],
                    values: true,
                    unit: unit.clone(),
                    band_name: band_name.clone(),
                    ..Default::default()
                }),
            })
        });
        // Open on the file's first message on this grid: the first position of every axis
        // (first parameter, lowest level, earliest time) need not hold a field at all.
        let first_key = pg
            .map
            .iter()
            .min_by_key(|&(_, fi)| *fi)
            .map_or([0; 4], |(k, _)| *k);
        let start: Vec<u32> = axis_of.iter().map(|&a| first_key[a] as u32).collect();
        let summary = ScienceSummary {
            width: w,
            height: h,
            axes,
            units_by,
            start,
            sample: sample.clone(),
            unit,
            title: band_name,
            dataset: Some(grid.name()),
            kind: ScienceKind::Gridded,
            keywords: Arc::new(keywords),
            geo,
            ..Default::default()
        };
        let info = ImageInfo {
            format: Format::Grib,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 32,
            progressive: false,
            coding: sample,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(GribSession {
            source,
            model,
            page,
            info,
            summary,
            axis_of,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn page(&self) -> &Page {
        &self.model.pages[self.page as usize]
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        let mut key = [0usize; 4];
        for (&k, &i) in self.axis_of.iter().zip(index) {
            key[k] = i as usize;
        }
        self.planes.get_or(index, cancel, || {
            let (w, h) = (self.info.width, self.info.height);
            let Some(&fi) = self.page().map.get(&key) else {
                let _r = specular_core::budget::reserve_n(u64::from(w) * u64::from(h), 4)?;
                return Ok(NdPlane {
                    width: w,
                    height: h,
                    values: vec![f32::NAN; w as usize * h as usize],
                    truncated: false,
                });
            };
            let p = self.model.fields[fi]
                .plane(self.source.bytes())
                .map_err(grib_err)?;
            Ok(NdPlane {
                width: p.width,
                height: p.height,
                values: p.values,
                truncated: false,
            })
        })
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let idx = if self.summary.start.len() == self.summary.axes.len() {
            self.summary.start.clone()
        } else {
            vec![0; self.summary.axes.len()]
        };
        Ok(science::auto_raster(&[self.plane(&idx, cancel)?], None))
    }
}

impl Session for GribSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.auto(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        px * 12 + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.planes.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("Greyscale")
    }
    fn page_count(&self) -> u32 {
        self.model.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.model.pages.len() > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: None,
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(GribSession::new(
            self.source.clone(),
            self.model.clone(),
            page,
        )?))
    }
    fn geo(&self) -> Option<Arc<GeoRef>> {
        self.summary.geo.clone()
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.plane(index, cancel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Arc<Source> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-grib/tests/data")
            .join(name);
        Arc::new(Source::from_vec(name, std::fs::read(&p).unwrap()))
    }

    #[test]
    fn axes_and_geo() {
        let s = crate::Registry::new()
            .open(fixture("simple_axes.grib2"))
            .unwrap();
        assert_eq!(s.info().format, Format::Grib);
        let sc = s.science().unwrap();
        assert_eq!(sc.kind, ScienceKind::Gridded);
        let names: Vec<_> = sc.axes.iter().map(|a| a.name.as_str()).collect();
        assert!(
            names.contains(&"parameter") && names.contains(&"time"),
            "{names:?}"
        );
        let (axis, units) = sc.units_by.as_ref().unwrap();
        assert_eq!(sc.axes[*axis].labels.len(), units.len());
        let g = s.geo().unwrap();
        assert!(g.crs.is_some());
        let cancel = CancelToken::new();
        let idx = vec![0; sc.axes.len()];
        let p = s.nd_plane(&idx, &cancel).unwrap().unwrap();
        assert_eq!((p.width, p.height), (sc.width, sc.height));
        assert!(p.values.iter().any(|v| v.is_finite()));
        // Every combination gives a plane (all missing where the file lacks it).
        let last: Vec<u32> = sc.axes.iter().map(|a| a.len - 1).collect();
        s.nd_plane(&last, &cancel).unwrap().unwrap();
        s.decode(&DecodeRequest::full(1), &cancel).unwrap();
    }

    #[test]
    fn grids_are_pages() {
        let s = crate::Registry::new().open(fixture("grids.grib2")).unwrap();
        assert!(s.page_count() >= 7);
        let cancel = CancelToken::new();
        for k in 0..s.page_count() {
            let p = s.open_page(k).unwrap();
            assert_eq!(p.page_index(), k);
            assert!(p.geo().is_some(), "page {k}");
            p.decode(&DecodeRequest::full(1), &cancel).unwrap();
        }
        let e1 = crate::Registry::new()
            .open(fixture("edition1.grib1"))
            .unwrap();
        assert!(e1.info().coding.starts_with("GRIB1"));
    }

    /// The first view shows the file's first message, not the first position of every axis,
    /// which these files have no field for: they opened as an empty (black) plane (the
    /// visual review sheets).
    #[test]
    fn opens_on_the_first_message() {
        let cancel = CancelToken::new();
        for name in [
            "edition1.grib1",
            "products.grib2",
            "packings.grib2",
            "mixed.grb",
        ] {
            let s = crate::Registry::new().open(fixture(name)).unwrap();
            let sc = s.science().unwrap();
            assert_eq!(sc.start.len(), sc.axes.len(), "{name}");
            let p = s.nd_plane(&sc.start, &cancel).unwrap().unwrap();
            assert!(p.values.iter().all(|v| v.is_finite()), "{name}");
            let r = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
            let px = &r.planes[0].data;
            let (lo, hi) = px
                .iter()
                .fold((255u8, 0u8), |(a, b), &v| (a.min(v), b.max(v)));
            assert!(hi > lo + 100, "{name}: grey {lo}..{hi}");
        }
    }
}

/// Synthetic hostile inputs: none may panic or blow up memory.
#[cfg(test)]
mod hostile {
    use super::*;

    /// The first message of a fixture as a 65535x65535 constant field (0 bits per value):
    /// 4 KB declaring a 17 GB plane is refused at open.
    #[test]
    fn constant_field_of_huge_grid_is_refused() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-grib/tests/data/simple_axes.grib2");
        let file = std::fs::read(&p).unwrap();
        let total = u64::from_be_bytes(file[8..16].try_into().unwrap()) as usize;
        let mut m = file[..total].to_vec();
        // Section 3 (template 3.0) at 37: Ni, Nj; section 5 at 143: bits per value.
        assert_eq!((m[37 + 4], m[143 + 4]), (3, 5));
        m[37 + 30..37 + 38].copy_from_slice(&[0, 0, 0xFF, 0xFF, 0, 0, 0xFF, 0xFF]);
        m[143 + 19] = 0;
        let r = crate::Registry::new().open(Arc::new(Source::from_vec("t.grib2", m)));
        assert!(matches!(r, Err(DecodeError::TooLarge)), "{:?}", r.err());
    }
}
