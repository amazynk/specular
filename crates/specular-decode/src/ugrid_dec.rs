//! Unstructured meshes in NetCDF (UGRID: FVCOM, ADCIRC, SCHISM, Delft3D FM, xugrid;
//! ICON grids and output; MPAS meshes), read by `specular-ugrid`, through the N-D science
//! path.
//!
//! Each data variable on the faces or nodes is a page; its other dimensions are the cube
//! axes, labelled with their coordinates (CF times, vertical levels). The mesh is painted
//! onto the planes at a resolution set by its face count (face values flat, node values
//! interpolated across each triangle, NaN off the mesh), so the colour maps, stretches and
//! limits of science mode apply; lon/lat meshes are equirectangular with a georeference (the
//! coastlines and graticule), projected meshes carry a pixel calibration. The readout asks
//! the mesh itself ([`MeshProbe`]): the face (or nearest node) under the pointer and the
//! value there, exact rather than from the painted pixel; the wireframe comes from the
//! mesh's edges. A mesh without data shows the nodes of each face.

use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_gis::geotiff::Affine2;
use specular_netcdf::{self as netcdf, Netcdf};
use specular_ugrid::{self as ugrid, Coords, DataVar, Display, Frame, Location, Locator, Topology};

use crate::geo::{GeoFacts, GeoRef, RasterGeo};
use crate::hdf5_dec::{coord_labels, map_nc};
use crate::imzml_dec::{lut_raster, viridis_lut};
use crate::science::{
    AxisKind, Linear, MeshFacts, MeshHit, MeshLocation, MeshProbe, NdAxis, NdPlane,
    PixelCalibration, PlaneCache, ScienceKind, ScienceSummary,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Format, ImageInfo, PageLabel, Result, Session};

/// Pages listed at most.
const MAX_PAGES: usize = 1024;
/// Positions on an axis given labels at most.
const MAX_LABELS: u64 = 1 << 16;
/// Header lines listed at most.
const MAX_KEYWORDS: usize = 4096;
/// The painted image's longer side: three pixels across an average face, within these.
const MIN_SIDE: f64 = 1024.0;
const MAX_SIDE: f64 = 4096.0;

pub(crate) fn map_ugrid(e: ugrid::Error) -> DecodeError {
    match e {
        ugrid::Error::NotUgrid => DecodeError::UnsupportedFormat,
        ugrid::Error::Malformed(s) => DecodeError::Malformed(s),
        ugrid::Error::TooLarge => DecodeError::TooLarge,
        ugrid::Error::Cancelled => DecodeError::Cancelled,
        ugrid::Error::Netcdf(e) => map_nc(e),
    }
}

/// The mesh of a file, shared by its pages.
struct Model {
    nc: Netcdf,
    source: Arc<Source>,
    topo: Topology,
    display: Display,
    locator: Locator,
    frame: Frame,
    vars: Vec<DataVar>,
    /// Nodes of each face, the values of a mesh without data.
    face_sizes: Arc<Vec<f32>>,
    facts: MeshFacts,
    geo: Option<Arc<GeoRef>>,
    pixel: Option<PixelCalibration>,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UgridModel")
            .field("facts", &self.facts)
            .field("frame", &self.frame)
            .finish()
    }
}

/// The image size for a mesh's extent: square pixels, the longer side from the face count.
fn frame_for(extent: [f64; 4], faces: u64) -> Frame {
    let side = ((faces as f64).sqrt() * 3.0).clamp(MIN_SIDE, MAX_SIDE);
    let (ex, ey) = (extent[2] - extent[0], extent[3] - extent[1]);
    let (w, h) = if !(ex > 0.0 && ey > 0.0) {
        (side, side)
    } else if ex >= ey {
        (side, (side * ey / ex).round().max(1.0))
    } else {
        ((side * ex / ey).round().max(1.0), side)
    };
    let mut e = extent;
    if ex.is_nan() || ex <= 0.0 {
        e[2] = e[0] + ey.max(1.0);
    }
    if ey.is_nan() || ey <= 0.0 {
        e[3] = e[1] + ex.max(1.0);
    }
    Frame::over(e, w as usize, h as usize)
}

/// Opens the mesh `topo` of `nc`: one page per data variable (or the face sizes of a
/// grid without data).
pub(crate) fn open(source: Arc<Source>, nc: Netcdf, topo: Topology) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let mesh = ugrid::Mesh::load(&nc, data, &topo, &|| false).map_err(map_ugrid)?;
    let display = Display::build(&mesh);
    if display.tris.is_empty() {
        return Err(DecodeError::Malformed(
            "the mesh has no faces to draw".into(),
        ));
    }
    let locator = Locator::build(&display);
    let frame = frame_for(display.extent, mesh.faces() as u64);
    let face_sizes = Arc::new(
        (0..mesh.faces())
            .map(|f| mesh.face(f).len() as f32)
            .collect(),
    );
    let mut vars = ugrid::variables(&nc, &topo);
    vars.truncate(MAX_PAGES);
    let geographic = display.geographic;
    let unit = match &mesh.coords {
        Coords::LonLat => None,
        Coords::Projected { unit } => unit.clone(),
    };
    let facts = MeshFacts {
        name: topo.name.clone(),
        long_name: topo.long_name.clone(),
        convention: match topo.convention {
            ugrid::Convention::Ugrid => "UGRID",
            ugrid::Convention::Icon => "ICON",
            ugrid::Convention::Mpas => "MPAS",
        },
        nodes: mesh.nodes.len() as u64,
        faces: mesh.faces() as u64,
        edges: topo.edges.unwrap_or(display.edge_count),
        triangles: mesh.kinds.triangles,
        quads: mesh.kinds.quads,
        polygons: mesh.kinds.polygons,
        max_face_nodes: mesh.kinds.max_nodes,
        geographic,
        unit: unit.clone(),
        location: None,
        split_faces: display.split_faces,
    };
    let (w, h) = (frame.width as f64, frame.height as f64);
    let geo = geographic.then(|| {
        let to_map = Affine2 {
            a: frame.dx,
            b: 0.0,
            c: frame.x0,
            d: 0.0,
            e: -frame.dy,
            f: frame.y1,
        };
        let (x0, y0) = to_map.apply(0.0, 0.0);
        let (x1, y1) = to_map.apply(w, h);
        Arc::new(GeoRef {
            crs: specular_gis::crs::from_epsg(4326),
            crs_note: Some("lon/lat coordinates, WGS 84 assumed".into()),
            to_map,
            bounds: [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)],
            features: None,
            facts: GeoFacts::Raster(RasterGeo {
                levels: vec![(frame.width as u32, frame.height as u32)],
                values: true,
                ..Default::default()
            }),
        })
    });
    let pixel = (!geographic).then(|| {
        let u = unit.clone().unwrap_or_default();
        PixelCalibration {
            x: Linear {
                offset: frame.x0 + frame.dx / 2.0,
                step: frame.dx,
            },
            y: Linear {
                offset: frame.y1 - frame.dy / 2.0,
                step: -frame.dy,
            },
            unit_x: u.clone(),
            unit_y: u,
        }
    });
    let model = Arc::new(Model {
        nc,
        source: source.clone(),
        topo,
        display,
        locator,
        frame,
        vars,
        face_sizes,
        facts,
        geo,
        pixel,
    });
    Ok(Arc::new(UgridSession::new(model, 0)?))
}

/// One page's mesh: what the readout and the wireframe ask.
struct MeshPage {
    model: Arc<Model>,
    /// The data variable shown, `None` for the face sizes.
    var: Option<usize>,
    location: Location,
    facts: MeshFacts,
    /// The values last read: the plane position and one value per face or node.
    values: Mutex<Option<LastValues>>,
}

/// A plane position and its values (one per face or node).
type LastValues = (Vec<u32>, Arc<Vec<f32>>);

impl std::fmt::Debug for MeshPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshPage")
            .field("var", &self.var)
            .field("facts", &self.facts)
            .finish()
    }
}

impl MeshPage {
    fn values(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<Vec<f32>>> {
        let Some(k) = self.var else {
            return Ok(self.model.face_sizes.clone());
        };
        if let Some((i, v)) = &*self.values.lock()
            && i == index
        {
            return Ok(v.clone());
        }
        let m = &self.model;
        let (v, _) = ugrid::read_values(&m.nc, m.source.bytes(), &m.vars[k], index, &|| {
            cancel.is_cancelled()
        })
        .map_err(map_ugrid)?;
        let v = Arc::new(v);
        *self.values.lock() = Some((index.to_vec(), v.clone()));
        Ok(v)
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<NdPlane> {
        let values = self.values(index, cancel)?;
        let m = &self.model;
        let f = &m.frame;
        Ok(NdPlane {
            width: f.width as u32,
            height: f.height as u32,
            values: ugrid::raster(&m.display, &values, self.location, f),
            truncated: false,
        })
    }
}

impl MeshProbe for MeshPage {
    fn facts(&self) -> &MeshFacts {
        &self.facts
    }

    fn at(&self, x: f64, y: f64, index: &[u32]) -> Option<MeshHit> {
        let m = &self.model;
        let d = &m.display;
        let (dx, dy) = m.frame.to_display(x, y);
        let hit = m.locator.locate(d, dx, dy).or_else(|| {
            // A point past the longitude window, where a split face's copy is not.
            d.geographic
                .then(|| {
                    let wrapped = d.west + (dx - d.west).rem_euclid(360.0);
                    m.locator.locate(d, wrapped, dy)
                })
                .flatten()
        })?;
        let values = self.values(index, &CancelToken::new()).ok();
        let get = |i: u32| {
            values
                .as_ref()
                .and_then(|v| v.get(i as usize))
                .map_or(f64::NAN, |&v| f64::from(v))
        };
        let (value, node) = match self.location {
            Location::Face => (get(hit.face), None),
            Location::Node => (
                (0..3).map(|k| hit.weights[k] * get(hit.nodes[k])).sum(),
                Some(hit.nearest_node()),
            ),
        };
        let x = if d.geographic {
            (dx + 180.0).rem_euclid(360.0) - 180.0
        } else {
            dx
        };
        Some(MeshHit {
            face: hit.face,
            node,
            value,
            x,
            y: dy,
        })
    }

    fn edges(&self, visible: [f64; 4], max: usize) -> Option<Vec<[f32; 4]>> {
        let m = &self.model;
        ugrid::segments_in(&m.display, &m.frame, visible, max)
    }
}

/// Labels for the positions along dimension `dim` of variable `v`: its coordinate variable,
/// else a one-dimensional variable on it that `v`'s `coordinates` names or that says what
/// it is (a vertical level's `positive`, an `axis`, a `standard_name`).
fn axis_meta(nc: &Netcdf, data: &[u8], v: &netcdf::Variable, dim: &str, len: u64) -> NdAxis {
    let named: Vec<&str> = v
        .text("coordinates")
        .map(|c| c.split_whitespace().collect())
        .unwrap_or_default();
    let coord = nc.var(dim).filter(|c| c.dims.len() == 1).or_else(|| {
        let on_dim = |c: &&netcdf::Variable| c.dims.len() == 1 && c.dims[0] == dim;
        named
            .iter()
            .filter_map(|n| nc.var(n))
            .find(on_dim)
            .or_else(|| {
                nc.vars.iter().filter(on_dim).find(|c| {
                    c.attr("positive").is_some()
                        || c.attr("axis").is_some()
                        || c.attr("standard_name").is_some()
                })
            })
    });
    let unit = coord.and_then(|c| c.text("units")).unwrap_or("").to_owned();
    let mut labels = Vec::new();
    if let Some(c) = coord
        && len <= MAX_LABELS
        && let Some(i) = nc.vars.iter().position(|x| std::ptr::eq(x, c))
        && let Ok(vals) = nc.read_all(data, i, len)
        && vals.len() as u64 == len
    {
        labels = coord_labels(&vals, &unit, c.text("calendar"));
    }
    NdAxis {
        name: coord
            .filter(|c| c.name != dim)
            .and_then(|c| c.text("long_name").or_else(|| c.text("standard_name")))
            .unwrap_or(dim)
            .to_owned(),
        len: len.min(u64::from(u32::MAX)) as u32,
        unit,
        labels,
        wcs_axis: None,
        linear: None,
        kind: AxisKind::Cube,
    }
}

struct UgridSession {
    model: Arc<Model>,
    page: u32,
    probe: Arc<MeshPage>,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

impl UgridSession {
    fn new(model: Arc<Model>, page: u32) -> Result<UgridSession> {
        let m = &model;
        let data = m.source.bytes();
        let nc = &m.nc;
        let dv = m.vars.get(page as usize);
        let mut facts = m.facts.clone();
        let mut keywords = Vec::new();
        let mut axes = Vec::new();
        let (location, sample, unit, dataset);
        match dv {
            Some(dv) => {
                let v = &nc.vars[dv.var];
                location = dv.location;
                facts.location = Some(match dv.location {
                    Location::Face => MeshLocation::Face,
                    Location::Node => MeshLocation::Node,
                });
                sample = v.describe();
                unit = v.text("units").map(str::to_owned);
                let title = v.text("long_name").or_else(|| v.text("standard_name"));
                dataset = Some(match title {
                    Some(t) => format!("{} — {t}", v.name),
                    None => v.name.clone(),
                });
                for &d in &dv.axes {
                    axes.push(axis_meta(nc, data, v, &v.dims[d], v.shape[d]));
                }
                keywords.extend(
                    v.attrs
                        .iter()
                        .map(|a| (a.name.clone(), a.value.display(16), String::new())),
                );
            }
            None => {
                location = Location::Face;
                sample = "32-bit float".into();
                unit = None;
                dataset = None;
            }
        }
        keywords.extend(
            nc.attrs
                .iter()
                .map(|a| (format!(":{}", a.name), a.value.display(16), String::new())),
        );
        keywords.truncate(MAX_KEYWORDS);
        let text_attr = |k: &str| {
            nc.attr(k)
                .and_then(netcdf::Value::as_str)
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        let probe = Arc::new(MeshPage {
            model: model.clone(),
            var: dv.map(|_| page as usize),
            location,
            facts,
            values: Mutex::new(None),
        });
        let (w, h) = (m.frame.width as u32, m.frame.height as u32);
        let geo = m.geo.as_ref().map(|g| {
            let mut g = (**g).clone();
            if let GeoFacts::Raster(r) = &mut g.facts {
                r.unit = unit.clone();
                r.band_name = dv.map(|dv| nc.vars[dv.var].name.clone());
            }
            Arc::new(g)
        });
        let summary = ScienceSummary {
            width: w,
            height: h,
            start: vec![0; axes.len()],
            axes,
            sample: sample.clone(),
            unit,
            dataset,
            title: Some(m.topo.name.clone()),
            object: text_attr("title"),
            instrument: ["source", "model", "institution"]
                .iter()
                .find_map(|k| text_attr(k)),
            kind: ScienceKind::Gridded,
            keywords: Arc::new(keywords),
            geo,
            pixel: m.pixel.clone(),
            luts: vec![viridis_lut()],
            mesh: Some(probe.clone()),
            ..Default::default()
        };
        let info = ImageInfo {
            format: Format::NetCdf,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: true,
            bit_depth: 32,
            progressive: false,
            coding: format!("{} mesh, {sample}", m.facts.convention),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(UgridSession {
            model,
            page,
            probe,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes
            .get_or(index, cancel, || self.probe.plane(index, cancel))
    }
}

impl Session for UgridSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.model.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| {
                let p = self.plane(&self.summary.start, cancel)?;
                Ok(lut_raster(&p, &viridis_lut()))
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        let elements = self.model.facts.nodes.max(self.model.facts.faces);
        px * 8 + elements * 12 + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
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
    fn geo(&self) -> Option<Arc<GeoRef>> {
        self.summary.geo.clone()
    }
    fn page_count(&self) -> u32 {
        self.model.vars.len().max(1) as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.model.vars.len() > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: Some(32),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.model.vars.len().max(1) {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        Ok(Arc::new(UgridSession::new(self.model.clone(), page)?))
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("Greyscale")
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

    #[test]
    fn frame_sizes() {
        let f = frame_for([-180.0, -90.0, 180.0, 90.0], 1 << 20);
        assert_eq!((f.width, f.height), (3072, 1536));
        let f = frame_for([0.0, 0.0, 10.0, 40.0], 100);
        assert_eq!((f.width, f.height), (256, 1024));
        let f = frame_for([5.0, 5.0, 5.0, 5.0], 1);
        assert_eq!((f.width, f.height), (1024, 1024));
        assert!(f.dx > 0.0 && f.dy > 0.0);
    }
}
