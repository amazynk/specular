//! HDF4 and HDF-EOS2 (`specular-hdf4`) through the N-D science path.
//!
//! Every scientific dataset of rank two or more is a page (its further dimensions are
//! sliders), followed by the raster images. Values go through the dataset's packing
//! (`scale_factor`, `add_offset`, both conventions) with `_FillValue`, `missing_value` and
//! `valid_range` as no data. HDF-EOS grids in the GCTP projections `specular-gis` follows
//! carry a georeference; swaths give longitude and latitude through their geolocation
//! fields and dimension maps for the readout.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_gis::crs::{Crs, LinearUnit};
use specular_gis::geotiff::Affine2;
use specular_gis::proj::{Ellipsoid, Method};
use specular_hdf4::eos::{self, gctp};
use specular_hdf4::{Ctx, File, Image, PlaneSpec, Sds};

use crate::geo::{GeoFacts, GeoRef, RasterGeo};
use crate::science::{self, AxisKind, NdAxis, NdPlane, PlaneCache, ScienceKind, ScienceSummary};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

pub struct Hdf4Decoder;

impl Decoder for Hdf4Decoder {
    fn format(&self) -> Format {
        Format::Hdf4
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_hdf4::sniff(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["hdf", "h4", "hdf4", "he2", "hdfeos"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let model = Arc::new(Model::open(&source)?);
        Ok(Arc::new(Hdf4Session::new(source, model, 0)?))
    }
}

fn hdf4_err(e: specular_hdf4::Error) -> DecodeError {
    use specular_hdf4::Error as E;
    match e {
        E::NotHdf4 => DecodeError::UnsupportedFormat,
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
        E::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        E::Cancelled => DecodeError::Cancelled,
        e => DecodeError::Malformed(e.to_string()),
    }
}

/// The most values of one plane.
const MAX_PLANE: u64 = 1 << 28;
/// The most values of a geolocation array read for the readout.
const MAX_GEOLOCATION: u64 = 1 << 26;
/// The most labels read from a coordinate variable.
const MAX_LABELS: u64 = 4096;
/// The most inventory metadata entries listed as keywords.
const MAX_META_KEYWORDS: usize = 400;

/// What the info panel shows about an HDF4 file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hdf4Facts {
    /// The library version the file was written with, as stored ("NCSA HDF Version 4.2
    /// Release 15").
    pub library: Option<String>,
    pub datasets: u64,
    pub images: u64,
    pub vgroups: u64,
    pub vdatas: u64,
    pub eos: Option<EosFacts>,
}

/// The HDF-EOS2 structures of a file and the one the page belongs to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EosFacts {
    /// `HDFEOSVersion`.
    pub version: Option<String>,
    pub grids: Vec<String>,
    pub swaths: Vec<String>,
    pub points: u64,
    /// The grid or swath of the dataset shown.
    pub grid: Option<String>,
    pub swath: Option<String>,
    /// The grid's GCTP projection as written ("GCTP_SNSOID").
    pub projection: Option<String>,
    /// Columns and rows of the grid.
    pub grid_size: Option<(u64, u64)>,
    /// Upper-left and lower-right corners: x0, y0, x1, y1, metres or degrees.
    pub corners: Option<[f64; 4]>,
    pub corners_degrees: bool,
    /// Product short name, and the start and end of the data (inventory metadata).
    pub short_name: Option<String>,
    pub begin: Option<String>,
    pub end: Option<String>,
}

/// Longitude and latitude per image pixel from geolocation arrays (an HDF-EOS swath),
/// possibly coarser than the image: geolocation column = `x_map.0 · column + x_map.1`, rows
/// likewise.
#[derive(Clone, Debug, PartialEq)]
pub struct LonLatGrid {
    pub width: usize,
    pub height: usize,
    pub lon: Arc<[f32]>,
    pub lat: Arc<[f32]>,
    pub x_map: (f64, f64),
    pub y_map: (f64, f64),
}

impl LonLatGrid {
    /// Longitude and latitude (degrees) at a stored pixel position (continuous, (0, 0) the
    /// top-left corner), interpolated between geolocation points.
    pub fn at(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        if !(x.is_finite() && y.is_finite()) || self.width == 0 || self.height == 0 {
            return None;
        }
        let gx = (self.x_map.0 * x.floor() + self.x_map.1).clamp(0.0, (self.width - 1) as f64);
        let gy = (self.y_map.0 * y.floor() + self.y_map.1).clamp(0.0, (self.height - 1) as f64);
        let (x0, y0) = (gx.floor() as usize, gy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (tx, ty) = (gx - x0 as f64, gy - y0 as f64);
        let at = |c: usize, r: usize| {
            let k = r * self.width + c;
            let (lon, lat) = (f64::from(self.lon[k]), f64::from(self.lat[k]));
            (lon.abs() <= 360.0 && lat.abs() <= 90.0).then_some((lon, lat))
        };
        let corners = [at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1)];
        let weights = [
            (1.0 - tx) * (1.0 - ty),
            tx * (1.0 - ty),
            (1.0 - tx) * ty,
            tx * ty,
        ];
        if corners.iter().any(Option::is_none) {
            // Nearest valid point where a neighbour is missing.
            let k = (if ty < 0.5 { 0 } else { 2 }) + usize::from(tx >= 0.5);
            return corners[k];
        }
        let c: Vec<(f64, f64)> = corners.iter().flatten().copied().collect();
        let (lo, hi) = c
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
                (a.min(p.0), b.max(p.0))
            });
        // Across the antimeridian: unwrap the western points before blending.
        let wrap = hi - lo > 180.0;
        let (mut lon, mut lat) = (0.0, 0.0);
        for (p, w) in c.iter().zip(weights) {
            let l = if wrap && p.0 < 0.0 { p.0 + 360.0 } else { p.0 };
            lon += l * w;
            lat += p.1 * w;
        }
        if lon > 180.0 {
            lon -= 360.0;
        }
        Some((lon, lat))
    }
}

/// What a page shows.
#[derive(Clone, Copy, Debug)]
enum PageKind {
    /// A dataset by index in [`File::sds`], with the dimensions along x and y.
    Sds { index: usize, x: usize, y: usize },
    /// A raster image by index in [`File::images`].
    Image { index: usize },
}

type Lonlat = Option<(usize, usize, Arc<[f32]>, Arc<[f32]>)>;

struct Model {
    file: File,
    /// The folder external elements may come from.
    folder: Option<PathBuf>,
    pages: Vec<PageKind>,
    /// Geolocation arrays read for the readout, by (latitude, longitude) dataset.
    lonlat: Mutex<HashMap<(usize, usize), Lonlat>>,
}

/// Reads part of an external element's file: a plain name in `folder`, nothing else.
fn read_external(folder: &Path, name: &str, off: u64, len: u64) -> Option<Vec<u8>> {
    let name = name.trim_end_matches('\0').trim();
    if name.is_empty() || name.contains(['/', '\\', ':']) || name.starts_with('.') || len > 2 << 30
    {
        return None;
    }
    let mut f = specular_core::open_regular(&folder.join(name)).ok()?;
    f.seek(SeekFrom::Start(off)).ok()?;
    let mut out = Vec::new();
    f.take(len).read_to_end(&mut out).ok()?;
    Some(out)
}

impl Model {
    fn open(source: &Source) -> Result<Model> {
        let file = File::open(source.bytes()).map_err(hdf4_err)?;
        let mut pages = Vec::new();
        for (index, s) in file.sds.iter().enumerate() {
            if s.coord || s.shape.len() < 2 || s.nt.is_char() {
                continue;
            }
            let field_dims = file
                .eos
                .as_ref()
                .and_then(|e| {
                    e.grid_of(index)
                        .and_then(|g| g.field_of(index))
                        .or_else(|| e.swath_of(index).and_then(|w| w.field_of(index)))
                })
                .map(|f| f.dims.clone());
            let (x, y) = plane_dims(s, field_dims.as_deref());
            if s.shape[x] < 2 || s.shape[y] < 2 || s.shape.contains(&0) {
                continue;
            }
            pages.push(PageKind::Sds { index, x, y });
        }
        for (index, im) in file.images.iter().enumerate() {
            if im.width > 0 && im.height > 0 {
                pages.push(PageKind::Image { index });
            }
        }
        if pages.is_empty() {
            return Err(DecodeError::Unsupported(
                "no dataset of two or more dimensions and no raster image".into(),
            ));
        }
        let folder = source
            .path()
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf);
        Ok(Model {
            file,
            folder,
            pages,
            lonlat: Mutex::new(HashMap::new()),
        })
    }

    fn with_ctx<T>(&self, data: &[u8], f: impl FnOnce(Ctx) -> T) -> T {
        match &self.folder {
            Some(dir) => {
                let ext = |name: &str, off: u64, len: u64| read_external(dir, name, off, len);
                f(Ctx::with_external(data, &ext))
            }
            None => f(Ctx::new(data)),
        }
    }

    /// A latitude and a longitude array, as read once (rows × columns, degrees).
    fn geolocation(&self, data: &[u8], lat: usize, lon: usize) -> Lonlat {
        if let Some(v) = self.lonlat.lock().get(&(lat, lon)) {
            return v.clone();
        }
        let read = |i: usize| -> Option<(Vec<u64>, Arc<[f32]>)> {
            let s = self.file.sds.get(i)?;
            if s.shape.len() != 2 || s.len() > MAX_GEOLOCATION {
                return None;
            }
            let p = s.packing();
            let v = self
                .with_ctx(data, |ctx| self.file.read_sds_all(ctx, s, MAX_GEOLOCATION))
                .ok()?;
            Some((
                s.shape.clone(),
                v.iter().map(|&x| p.apply(x) as f32).collect(),
            ))
        };
        let got = match (read(lat), read(lon)) {
            (Some((a, la)), Some((b, lo))) if a == b => {
                Some((a[1] as usize, a[0] as usize, lo, la))
            }
            _ => None,
        };
        self.lonlat.lock().insert((lat, lon), got.clone());
        got
    }
}

/// Which dimensions run along x and y: an HDF-EOS grid's `XDim` and `YDim`; the first two of
/// a rank-3 dataset whose last dimension is short (bands last); else the last two.
fn plane_dims(s: &Sds, field_dims: Option<&[String]>) -> (usize, usize) {
    let rank = s.shape.len();
    let names: Vec<String> = match field_dims {
        Some(d) if d.len() == rank => d.iter().map(|n| n.to_ascii_lowercase()).collect(),
        _ => s.dims.iter().map(|d| d.name.to_ascii_lowercase()).collect(),
    };
    let find = |key: &str| names.iter().position(|n| n.split(':').next() == Some(key));
    if let (Some(x), Some(y)) = (find("xdim"), find("ydim"))
        && x != y
    {
        return (x, y);
    }
    if rank == 3 && s.shape[2] <= 16 && s.shape[0] > 16 && s.shape[1] > 16 {
        return (1, 0);
    }
    (rank - 1, rank - 2)
}

/// A dimension's name without the `:owner` suffix HDF-EOS and the SD interface add.
fn dim_label(name: &str) -> &str {
    name.split(':').next().unwrap_or(name)
}

/// Inventory and archive metadata objects with a value, as keywords.
fn meta_keywords(prefix: &str, root: &specular_hdf4::odl::Group, out: &mut Vec<Kw>) {
    let mut n = 0;
    for g in root.walk() {
        if !g.object {
            continue;
        }
        let Some(v) = g.get("VALUE") else {
            continue;
        };
        if n >= MAX_META_KEYWORDS {
            break;
        }
        n += 1;
        let v = match v {
            specular_hdf4::odl::Val::List(l) if l.len() == 1 => l[0].display(),
            v => v.display(),
        };
        out.push((format!("{prefix}: {}", g.name), v, String::new()));
    }
}

type Kw = (String, String, String);

fn kw(k: &str, v: impl Into<String>) -> Kw {
    (k.to_owned(), v.into(), String::new())
}

fn attr_value(v: &specular_hdf4::Value) -> String {
    match v.text() {
        Some(t) => t
            .trim_matches(|c: char| c == '\0' || c.is_whitespace())
            .to_owned(),
        None => v.display(16),
    }
}

/// Whether a file attribute is structure or inventory metadata (shown parsed).
fn is_eos_text(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    matches!(
        base.as_str(),
        "structmetadata" | "coremetadata" | "archivemetadata"
    )
}

/// The ellipsoid of GCTP parameters 0 and 1 (semi-major axis; semi-minor axis, or the
/// eccentricity squared, or 0 for a sphere) or else the GCTP sphere code.
fn gctp_ellipsoid(params: &[f64], sphere: Option<i32>) -> Option<(Ellipsoid, String)> {
    let p = |k: usize| params.get(k).copied().unwrap_or(0.0);
    let (a, b) = (p(0), p(1));
    if a > 0.0 {
        return Some(if b == 0.0 {
            (Ellipsoid { a, f: 0.0 }, format!("Sphere, radius {a} m"))
        } else if b > 1.0 {
            (
                Ellipsoid { a, f: (a - b) / a },
                format!("a = {a} m, b = {b} m"),
            )
        } else {
            (
                Ellipsoid {
                    a,
                    f: 1.0 - (1.0 - b).sqrt(),
                },
                format!("a = {a} m, e² = {b}"),
            )
        });
    }
    let ab = |name: &str, a: f64, b: f64| (Ellipsoid { a, f: (a - b) / a }, name.to_owned());
    let named = |name: &str, e: Ellipsoid| (e, name.to_owned());
    Some(match sphere.unwrap_or(0) {
        0 => named("Clarke 1866", Ellipsoid::CLARKE1866),
        1 => ab("Clarke 1880", 6_378_249.145, 6_356_514.869_55),
        2 => named("Bessel", Ellipsoid::BESSEL1841),
        3 => named("International 1967", Ellipsoid::inv(6_378_160.0, 298.25)),
        4 => named("International 1909", Ellipsoid::INTL1924),
        5 => named("WGS 72", Ellipsoid::WGS72),
        6 => ab("Everest", 6_377_276.345_2, 6_356_075.413_3),
        7 => ab("WGS 66", 6_378_145.0, 6_356_759.769_356),
        8 => named("GRS 1980", Ellipsoid::GRS80),
        9 => named("Airy", Ellipsoid::AIRY1830),
        10 => ab("Modified Everest", 6_377_304.063, 6_356_103.039),
        11 => ab("Modified Airy", 6_377_340.189, 6_356_034.448),
        12 => named("WGS 84", Ellipsoid::WGS84),
        13 => ab("Southeast Asia", 6_378_155.0, 6_356_773.320_5),
        14 => ab("Australian National", 6_378_160.0, 6_356_774.719),
        15 => named("Krassovsky", Ellipsoid::KRASSOVSKY),
        16 => ab("Hough", 6_378_270.0, 6_356_794.343_479),
        17 => ab("Mercury 1960", 6_378_166.0, 6_356_784.283_666),
        18 => ab("Modified Mercury 1968", 6_378_150.0, 6_356_768.337_303),
        19 => (
            Ellipsoid {
                a: 6_370_997.0,
                f: 0.0,
            },
            "Sphere, radius 6370997 m".to_owned(),
        ),
        _ => return None,
    })
}

/// A grid's projection method from its GCTP code and parameters (angles packed DMS), or why
/// not.
fn gctp_method(g: &eos::Grid) -> std::result::Result<Method, String> {
    let p = |k: usize| g.params.get(k).copied().unwrap_or(0.0);
    let d = |k: usize| eos::dms_to_deg(p(k));
    let code = g
        .proj_code
        .ok_or_else(|| format!("projection {}", g.projection))?;
    Ok(match code {
        gctp::UTM => {
            let zone = g.zone.unwrap_or(0);
            if zone == 0 {
                let (lon, lat) = (d(0), d(1));
                let z = (((lon + 180.0) / 6.0).floor() as i32).clamp(0, 59) + 1;
                Method::utm(z as u32, lat >= 0.0)
            } else if (1..=60).contains(&zone.abs()) {
                Method::utm(zone.unsigned_abs(), zone > 0)
            } else {
                return Err(format!("UTM zone {zone}"));
            }
        }
        gctp::TM => Method::TransverseMercator {
            lat0: d(5),
            lon0: d(4),
            k0: if p(2) > 0.0 { p(2) } else { 1.0 },
            fe: p(6),
            fn_: p(7),
        },
        gctp::ALBERS => Method::Albers {
            lat0: d(5),
            lon0: d(4),
            lat1: d(2),
            lat2: d(3),
            fe: p(6),
            fn_: p(7),
        },
        gctp::LAMCC => Method::LambertConic {
            lat0: d(5),
            lon0: d(4),
            lat1: d(2),
            lat2: d(3),
            k0: 1.0,
            fe: p(6),
            fn_: p(7),
        },
        gctp::MERCAT => Method::MercatorB {
            lon0: d(4),
            lat1: d(5),
            fe: p(6),
            fn_: p(7),
        },
        gctp::PS => {
            let lat_ts = d(5);
            Method::PolarStereographic {
                lat_ts: if lat_ts == 0.0 { 90.0 } else { lat_ts },
                lon0: d(4),
                south: lat_ts < 0.0,
                fe: p(6),
                fn_: p(7),
            }
        }
        gctp::LAMAZ => Method::LambertAzimuthal {
            lat0: d(5),
            lon0: d(4),
            fe: p(6),
            fn_: p(7),
        },
        gctp::SNSOID => Method::Sinusoidal {
            lon0: d(4),
            fe: p(6),
            fn_: p(7),
        },
        gctp::EQRECT => Method::Equirectangular {
            lat_ts: d(5),
            lon0: d(4),
            fe: p(6),
            fn_: p(7),
        },
        // EASE-Grid (BCEA, on its 6,371,228 m sphere) and other cylindrical equal-area
        // grids: true scale at parameter 5.
        gctp::CEA | gctp::BCEA => Method::CylindricalEqualArea {
            lat_ts: d(5),
            lon0: d(4),
            fe: p(6),
            fn_: p(7),
        },
        _ => return Err(format!("projection {}", g.projection)),
    })
}

/// A grid's georeference for a plane of `w × h`: `Err` says why there is none.
fn grid_geo(g: &eos::Grid, w: u32, h: u32) -> std::result::Result<GeoRef, String> {
    if g.xdim != u64::from(w) || g.ydim != u64::from(h) {
        return Err(format!(
            "the field is {w} × {h}, the grid {} × {}",
            g.xdim, g.ydim
        ));
    }
    let ((ulx, uly), (lrx, lry)) = g.corners().ok_or("no corners")?;
    let (crs, note) = if g.geographic() {
        let crs = specular_gis::crs::from_epsg(4326).ok_or("no WGS 84")?;
        (crs, None)
    } else {
        let method = gctp_method(g)?;
        let (ell, ell_name) = gctp_ellipsoid(&g.params, g.sphere)
            .ok_or_else(|| format!("sphere code {}", g.sphere.unwrap_or(-1)))?;
        let mut crs = Crs::custom_projected(
            format!("{} ({})", method.name(), g.projection),
            ell,
            method,
            LinearUnit::metre(),
        );
        crs.ellipsoid_name = Some(ell_name);
        (crs, Some(format!("HDF-EOS grid {}", g.name)))
    };
    let (fw, fh) = (f64::from(w), f64::from(h));
    let origin = g
        .origin
        .as_deref()
        .unwrap_or("HDFE_GD_UL")
        .to_ascii_uppercase();
    // Rows and columns start at the origin's corner.
    let (x0, sx) = if origin.ends_with('R') {
        (lrx, (ulx - lrx) / fw)
    } else {
        (ulx, (lrx - ulx) / fw)
    };
    let (y0, sy) = if origin.contains("_L") {
        (lry, (uly - lry) / fh)
    } else {
        (uly, (lry - uly) / fh)
    };
    let to_map = Affine2 {
        a: sx,
        b: 0.0,
        c: x0,
        d: 0.0,
        e: sy,
        f: y0,
    };
    if ![sx, sy, x0, y0].iter().all(|v| v.is_finite()) || sx == 0.0 || sy == 0.0 {
        return Err("degenerate corners".into());
    }
    let (ax, ay) = to_map.apply(0.0, 0.0);
    let (bx, by) = to_map.apply(fw, fh);
    Ok(GeoRef {
        crs: Some(crs),
        crs_note: note,
        to_map,
        bounds: [ax.min(bx), ay.min(by), ax.max(bx), ay.max(by)],
        features: None,
        facts: GeoFacts::Raster(RasterGeo {
            levels: vec![(w, h)],
            values: true,
            ..Default::default()
        }),
    })
}

/// `data = offset + increment · geo` as `geo = a · data + b`.
fn map_coeffs(m: &eos::DimMap) -> (f64, f64) {
    let (o, i) = (m.offset as f64, m.increment as f64);
    if m.increment > 0 {
        (1.0 / i, -o / i)
    } else if m.increment < 0 {
        (-i, -o)
    } else {
        (1.0, 0.0)
    }
}

struct Hdf4Session {
    source: Arc<Source>,
    model: Arc<Model>,
    page: u32,
    kind: PageKind,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

impl Hdf4Session {
    fn new(source: Arc<Source>, model: Arc<Model>, page: u32) -> Result<Hdf4Session> {
        let kind = *model
            .pages
            .get(page as usize)
            .ok_or_else(|| DecodeError::Malformed("no such dataset".into()))?;
        let f = &model.file;
        let data = source.bytes();
        let mut keywords: Vec<Kw> = Vec::new();
        let mut summary = ScienceSummary {
            kind: ScienceKind::Gridded,
            ..Default::default()
        };
        let mut facts = Hdf4Facts {
            library: f
                .version
                .as_ref()
                .map(|v| v.text.clone())
                .filter(|t| !t.is_empty()),
            datasets: f.sds.len() as u64,
            images: f.images.len() as u64,
            vgroups: f.vgroups.len() as u64,
            vdatas: f.vdatas.len() as u64,
            eos: None,
        };
        let (w, h, bits, sample);
        match kind {
            PageKind::Sds { index, x, y } => {
                let s = &f.sds[index];
                (w, h) = (s.shape[x] as u32, s.shape[y] as u32);
                if u64::from(w) * u64::from(h) > MAX_PLANE {
                    return Err(DecodeError::TooLarge);
                }
                bits = (s.nt.size() * 8) as u8;
                let packing = s.packing();
                let comp = model.with_ctx(data, |ctx| s.compression(f, ctx));
                sample = match comp {
                    Some(c) => format!("{}, {c}", s.nt.describe()),
                    None => s.nt.describe(),
                };
                for (k, &len) in s.shape.iter().enumerate() {
                    if k == x || k == y {
                        continue;
                    }
                    let name = s.dims.get(k).map_or("", |d| dim_label(&d.name)).to_owned();
                    let labels = coord_labels(&model, data, s, k);
                    summary.axes.push(NdAxis {
                        name,
                        len: len.min(u64::from(u32::MAX)) as u32,
                        unit: String::new(),
                        labels,
                        wcs_axis: None,
                        linear: None,
                        kind: AxisKind::Cube,
                    });
                }
                let long = s.text_attr("long_name");
                summary.dataset = Some(match &long {
                    Some(l) if *l != s.name => format!("{} — {l}", s.name),
                    _ => s.name.clone(),
                });
                summary.title = long;
                summary.unit = s.text_attr("units");
                keywords.push(kw("Dataset", s.name.clone()));
                keywords.push(kw(
                    "Dimensions",
                    s.dims
                        .iter()
                        .zip(&s.shape)
                        .map(|(d, n)| {
                            let u = if d.unlimited { ", unlimited" } else { "" };
                            format!("{} ({n}{u})", dim_label(&d.name))
                        })
                        .collect::<Vec<_>>()
                        .join(" × "),
                ));
                keywords.push(kw("Type", s.nt.describe()));
                keywords.push(kw("Storage", model.with_ctx(data, |ctx| s.storage(f, ctx))));
                if let Some(p) = packing.describe() {
                    keywords.push(kw("Packing", format!("value = {p}")));
                }
                for a in &s.attrs {
                    keywords.push(kw(&a.name, attr_value(&a.value)));
                }
                // HDF-EOS: the grid's georeference or the swath's geolocation.
                if let Some(e) = &f.eos {
                    let mut ef = eos_facts(e);
                    if let Some(g) = e.grid_of(index) {
                        ef.grid = Some(g.name.clone());
                        ef.projection = Some(g.projection.clone()).filter(|p| !p.is_empty());
                        ef.grid_size = Some((g.xdim, g.ydim));
                        if let Some(((a, b), (c, d))) = g.corners() {
                            ef.corners = Some([a, b, c, d]);
                            ef.corners_degrees = g.geographic();
                        }
                        match grid_geo(g, w, h) {
                            Ok(mut geo) => {
                                if let GeoFacts::Raster(r) = &mut geo.facts {
                                    r.unit = summary.unit.clone();
                                    r.band_name = Some(s.name.clone());
                                }
                                summary.geo = Some(Arc::new(geo));
                            }
                            Err(why) => {
                                keywords.push(kw("Georeference", format!("not followed: {why}")))
                            }
                        }
                    } else if let Some(sw) = e.swath_of(index) {
                        ef.swath = Some(sw.name.clone());
                        summary.lonlat = swath_lonlat(&model, data, sw, index, x, y).map(Arc::new);
                    }
                    facts.eos = Some(ef);
                }
                if summary.geo.is_none() && summary.lonlat.is_none() {
                    summary.lonlat = plain_lonlat(&model, data, s, x, y).map(Arc::new);
                }
            }
            PageKind::Image { index } => {
                let im = &f.images[index];
                (w, h) = (im.width, im.height);
                if u64::from(w) * u64::from(h) * u64::from(im.ncomp.max(1)) > MAX_PLANE {
                    return Err(DecodeError::TooLarge);
                }
                bits = (im.nt.size() * 8) as u8;
                let comp = model.with_ctx(data, |ctx| im.compression(f, ctx));
                let what = if im.ncomp >= 3 {
                    format!("{} components", im.ncomp)
                } else if im.palette.is_some() {
                    "palette".to_owned()
                } else {
                    "grey".to_owned()
                };
                sample = match comp {
                    Some(c) => format!("{}, {what}, {c}", im.nt.describe()),
                    None => format!("{}, {what}", im.nt.describe()),
                };
                summary.dataset = Some(im.name.clone());
                summary.raster_only = image_is_colour(im);
                keywords.push(kw("Image", im.name.clone()));
                keywords.push(kw("Interface", if im.gr { "GR" } else { "RIG / RI8" }));
                for a in &im.attrs {
                    keywords.push(kw(&a.name, attr_value(&a.value)));
                }
                if let Some(e) = &f.eos {
                    facts.eos = Some(eos_facts(e));
                }
            }
        }
        if let Some(v) = &facts.library {
            keywords.push(kw("Library", v.clone()));
        }
        for a in f.attrs.iter().chain(&f.image_attrs) {
            if !is_eos_text(&a.name) {
                keywords.push(kw(&format!(":{}", a.name), attr_value(&a.value)));
            }
        }
        if let Some(e) = &f.eos {
            if let Some(c) = &e.core {
                meta_keywords("CoreMetadata", c, &mut keywords);
            }
            if let Some(a) = &e.archive {
                meta_keywords("ArchiveMetadata", a, &mut keywords);
            }
        }
        for warning in &f.warnings {
            keywords.push(kw("Problem", warning.clone()));
        }
        summary.width = w;
        summary.height = h;
        summary.sample = sample.clone();
        summary.keywords = Arc::new(keywords);
        summary.hdf4 = Some(Arc::new(facts));
        let info = ImageInfo {
            format: Format::Hdf4,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: bits.max(8),
            progressive: false,
            coding: sample,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Hdf4Session {
            source,
            model,
            page,
            kind,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        let f = &self.model.file;
        let data = self.source.bytes();
        self.planes.get_or(index, cancel, || match self.kind {
            PageKind::Sds { index: i, x, y } => {
                let s = &f.sds[i];
                let mut pos = vec![0u64; s.shape.len()];
                let mut it = index.iter();
                for (k, p) in pos.iter_mut().enumerate() {
                    if k != x && k != y {
                        *p = it.next().copied().map_or(0, u64::from);
                    }
                }
                let spec = PlaneSpec {
                    x_dim: x,
                    y_dim: y,
                    index: pos,
                };
                let p = self
                    .model
                    .with_ctx(data, |ctx| f.read_sds_plane(ctx, s, &spec))
                    .map_err(hdf4_err)?;
                if cancel.is_cancelled() {
                    return Err(DecodeError::Cancelled);
                }
                let packing = s.packing();
                Ok(NdPlane {
                    width: p.width as u32,
                    height: p.height as u32,
                    values: p.values.iter().map(|&v| packing.apply(v) as f32).collect(),
                    truncated: p.truncated,
                })
            }
            PageKind::Image { index: i } => {
                let d = self.read_image(i)?;
                // Grey images only (colour ones are shown from their raster): the first
                // component.
                let n = d.ncomp.max(1);
                Ok(NdPlane {
                    width: d.width as u32,
                    height: d.height as u32,
                    values: d.values.iter().step_by(n).map(|&v| v as f32).collect(),
                    truncated: false,
                })
            }
        })
    }

    fn read_image(&self, i: usize) -> Result<specular_hdf4::ImageData> {
        let f = &self.model.file;
        self.model
            .with_ctx(self.source.bytes(), |ctx| f.read_image(ctx, &f.images[i]))
            .map_err(hdf4_err)
    }

    /// A palette or true-colour image as RGBA.
    fn colour_raster(&self, i: usize) -> Result<Raster> {
        let im = &self.model.file.images[i];
        let d = self.read_image(i)?;
        let n = d.width * d.height;
        let mut rgba = vec![255u8; n * 4];
        let wide = im.nt.size() > 1;
        let max = if wide {
            d.values.iter().fold(1.0f64, |m, &v| m.max(v))
        } else {
            255.0
        };
        let to8 = |v: f64| (v * 255.0 / max).round().clamp(0.0, 255.0) as u8;
        if d.ncomp >= 3 {
            for (px, s) in rgba.chunks_exact_mut(4).zip(d.values.chunks_exact(d.ncomp)) {
                for c in 0..3 {
                    px[c] = to8(s[c]);
                }
            }
        } else if let Some(p) = &im.palette {
            for (px, &v) in rgba
                .chunks_exact_mut(4)
                .zip(d.values.iter().step_by(d.ncomp.max(1)))
            {
                let e = p
                    .entries
                    .get(v.clamp(0.0, 255.0) as usize)
                    .copied()
                    .unwrap_or([0, 0, 0]);
                px[..3].copy_from_slice(&e);
            }
        }
        Ok(whole::interleaved(
            d.width as u32,
            d.height as u32,
            PixelLayout::Rgba,
            rgba,
            false,
        ))
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        if let PageKind::Image { index } = self.kind
            && self.summary.raster_only
        {
            return self.colour_raster(index);
        }
        let idx = vec![0; self.summary.axes.len()];
        Ok(science::auto_raster(&[self.plane(&idx, cancel)?], None))
    }
}

fn image_is_colour(im: &Image) -> bool {
    im.ncomp >= 3 || im.palette.is_some()
}

/// The HDF-EOS facts of the whole file.
fn eos_facts(e: &eos::Eos) -> EosFacts {
    EosFacts {
        version: e.version.clone(),
        grids: e.grids.iter().map(|g| g.name.clone()).collect(),
        swaths: e.swaths.iter().map(|s| s.name.clone()).collect(),
        points: e.points as u64,
        short_name: e.short_name(),
        begin: e.begin(),
        end: e.end(),
        ..Default::default()
    }
}

/// The values of the coordinate variable of dimension `k` as labels, when there is one of
/// the right length.
fn coord_labels(model: &Model, data: &[u8], s: &Sds, k: usize) -> Vec<String> {
    let (Some(d), Some(&len)) = (s.dims.get(k), s.shape.get(k)) else {
        return Vec::new();
    };
    if len > MAX_LABELS {
        return Vec::new();
    }
    let f = &model.file;
    let Some(c) = f
        .sds
        .iter()
        .find(|c| c.coord && c.name == d.name && c.shape == [len])
    else {
        return Vec::new();
    };
    match model.with_ctx(data, |ctx| f.read_sds_all(ctx, c, MAX_LABELS)) {
        Ok(v) if v.len() as u64 == len => v.iter().map(|x| format!("{x}")).collect(),
        _ => Vec::new(),
    }
}

/// Longitude and latitude of a swath field's pixels through the swath's geolocation fields
/// and dimension maps.
fn swath_lonlat(
    model: &Model,
    data: &[u8],
    sw: &eos::Swath,
    index: usize,
    x: usize,
    y: usize,
) -> Option<LonLatGrid> {
    let field = sw.field_of(index)?;
    let (lat, lon) = (sw.geo_field("Latitude")?, sw.geo_field("Longitude")?);
    if lat.dims.len() != 2 || field.dims.len() <= x.max(y) {
        return None;
    }
    let my = sw.map(&lat.dims[0], &field.dims[y])?;
    let mx = sw.map(&lat.dims[1], &field.dims[x])?;
    let (gw, gh, lon_v, lat_v) = model.geolocation(data, lat.sds?, lon.sds?)?;
    Some(LonLatGrid {
        width: gw,
        height: gh,
        lon: lon_v,
        lat: lat_v,
        x_map: map_coeffs(&mx),
        y_map: map_coeffs(&my),
    })
}

/// Longitude and latitude from datasets named so with the plane's shape (files without
/// HDF-EOS metadata).
fn plain_lonlat(model: &Model, data: &[u8], s: &Sds, x: usize, y: usize) -> Option<LonLatGrid> {
    if y > x || s.shape.len() < 2 {
        return None;
    }
    let shape = [s.shape[y], s.shape[x]];
    let f = &model.file;
    let find = |names: &[&str]| {
        f.sds
            .iter()
            .position(|c| c.shape == shape && names.iter().any(|n| c.name.eq_ignore_ascii_case(n)))
    };
    let lat = find(&["latitude", "lat"])?;
    let lon = find(&["longitude", "lon"])?;
    let (gw, gh, lon_v, lat_v) = model.geolocation(data, lat, lon)?;
    Some(LonLatGrid {
        width: gw,
        height: gh,
        lon: lon_v,
        lat: lat_v,
        x_map: (1.0, 0.0),
        y_map: (1.0, 0.0),
    })
}

impl Session for Hdf4Session {
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
        // Stored numbers as f64 while a plane is read, then f32 values.
        px * 12 + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.planes.memory_bytes() + self.model.file.cache_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.planes.clear() + self.model.file.clear_cache()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if self.summary.raster_only {
            "RGB"
        } else {
            "Greyscale"
        })
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
            bits: Some(u32::from(self.info.bit_depth)),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(Hdf4Session::new(
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
        if self.summary.raster_only {
            return None;
        }
        Some(self.plane(index, cancel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::GeoFacts;

    fn data_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-hdf4/tests/data")
    }

    fn open(name: &str) -> Arc<dyn Session> {
        let p = data_dir().join(name);
        let src = Source::from_vec(p.clone(), std::fs::read(&p).unwrap());
        crate::Registry::new().open(Arc::new(src)).unwrap()
    }

    /// The page showing a dataset or image of this name.
    fn page(s: &Arc<dyn Session>, name: &str) -> Arc<dyn Session> {
        (0..s.page_count())
            .map(|i| s.open_page(i).unwrap())
            .find(|p| {
                p.science()
                    .and_then(|x| x.dataset.as_deref())
                    .is_some_and(|d| d == name || d.starts_with(&format!("{name} —")))
            })
            .unwrap_or_else(|| panic!("no page {name}"))
    }

    #[test]
    fn modis_like_grid_is_georeferenced() {
        let s = open("eos_grid.hdf");
        assert_eq!(s.info().format, Format::Hdf4);
        assert!(s.page_count() >= 3);
        let p = page(&s, "sur_refl_b01");
        let sc = p.science().unwrap();
        assert_eq!((sc.width, sc.height), (48, 40));
        let g = p.geo().unwrap();
        assert!((g.to_map.a - 23165.64).abs() < 0.01, "{:?}", g.to_map);
        assert!((g.to_map.e + 23165.64).abs() < 0.01);
        let crs = g.crs.as_ref().unwrap();
        assert!(crs.name.contains("SNSOID"), "{}", crs.name);
        assert!(matches!(g.facts, GeoFacts::Raster(_)));
        let facts = sc.hdf4.as_ref().unwrap();
        let eos = facts.eos.as_ref().unwrap();
        assert_eq!(
            eos.grid.as_deref(),
            Some("MOD_Grid_500m_Surface_Reflectance")
        );
        assert_eq!(eos.projection.as_deref(), Some("GCTP_SNSOID"));
        assert_eq!(eos.grid_size, Some((48, 40)));
        assert!(eos.short_name.is_some() && eos.begin.is_some());
        assert!(facts.datasets >= 3);
        // Packing applied; the fill value is no data.
        let cancel = CancelToken::new();
        let plane = p.nd_plane(&[], &cancel).unwrap().unwrap();
        assert_eq!((plane.width, plane.height), (48, 40));
        assert!(plane.values.iter().any(|v| v.is_nan()));
        assert!(
            plane
                .values
                .iter()
                .any(|v| v.is_finite() && v.fract() != 0.0)
        );
        p.decode(&DecodeRequest::full(1), &cancel).unwrap();
        assert!(
            sc.keywords
                .iter()
                .any(|k| k.0.starts_with("CoreMetadata: "))
        );
        assert!(!sc.keywords.iter().any(|k| k.0.contains("StructMetadata")));
    }

    #[test]
    fn projections_follow_gctp() {
        let s = open("eos_proj.hdf");
        let mut geo = 0;
        for i in 0..s.page_count() {
            let p = s.open_page(i).unwrap();
            let name = p.science().unwrap().dataset.clone().unwrap();
            match p.geo() {
                Some(g) => {
                    geo += 1;
                    // The upper-left pixel's centre maps back inside the CRS's domain.
                    let crs = g.crs.as_ref().unwrap();
                    let (x, y) = g.to_map.apply(0.5, 0.5);
                    let ll = crs.to_lonlat(x, y);
                    assert!(
                        ll.is_some_and(|(lo, la)| lo.abs() <= 180.0 && la.abs() <= 90.0),
                        "{name}: {ll:?}"
                    );
                }
                None => assert!(name.contains("ISIN"), "{name} has no georeference"),
            }
        }
        assert_eq!(geo, 8);
    }

    /// EASE-Grid global (BCEA on the 6,371,228 m sphere, true scale at 30°): the 25 km grid
    /// is 1383 × 586 cells and its corners are about ±180° and ±86.72°.
    #[test]
    fn ease_grid_bcea() {
        let half_w = 17_334_193.537_5;
        let half_h = 7_344_784.825;
        let g = eos::Grid {
            name: "Northern Hemisphere".into(),
            xdim: 1383,
            ydim: 586,
            upper_left: Some((-half_w, half_h)),
            lower_right: Some((half_w, -half_h)),
            projection: "GCTP_BCEA".into(),
            proj_code: Some(gctp::BCEA),
            params: vec![
                6_371_228.0,
                6_371_228.0,
                0.0,
                0.0,
                0.0,
                30_000_000.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            zone: None,
            sphere: None,
            origin: None,
            registration: None,
            dims: Vec::new(),
            fields: Vec::new(),
        };
        let geo = grid_geo(&g, 1383, 586).unwrap();
        let crs = geo.crs.as_ref().unwrap();
        assert!(crs.name.contains("Cylindrical Equal Area"), "{}", crs.name);
        let (x, y) = geo.to_map.apply(0.0, 0.0);
        let (lon, lat) = crs.to_lonlat(x, y).unwrap();
        assert!((lon + 180.0).abs() < 1e-4, "{lon}");
        assert!((lat - 86.716_744).abs() < 1e-5, "{lat}");
        // The cells are 25,067.525 m wide.
        assert!((geo.to_map.a - 25_067.525).abs() < 1e-3, "{:?}", geo.to_map);
    }

    #[test]
    fn swath_gives_lonlat() {
        let s = open("eos_swath.hdf");
        let p = page(&s, "Radiance");
        let sc = p.science().unwrap();
        let ll = sc.lonlat.as_ref().expect("lon/lat");
        let (lon, lat) = ll.at(0.0, 0.0).unwrap();
        assert!(lon.abs() <= 180.0 && lat.abs() <= 90.0);
        let eos = sc.hdf4.as_ref().unwrap().eos.as_ref().unwrap();
        assert_eq!(eos.swath.as_deref(), Some("Swath_Test"));
    }

    #[test]
    fn rasters_and_extra_dimensions() {
        let s = open("raster.hdf");
        let cancel = CancelToken::new();
        for i in 0..s.page_count() {
            let p = s.open_page(i).unwrap();
            let r = p.decode(&DecodeRequest::full(1), &cancel).unwrap();
            assert_eq!((r.width, r.height), (p.info().width, p.info().height));
        }
        let p = page(&s, "gr_u16_lut");
        assert!(p.science().unwrap().raster_only);
        assert!(p.nd_plane(&[], &cancel).is_none());

        let s = open("sds_types.hdf");
        let p = page(&s, "cube4");
        let sc = p.science().unwrap();
        assert!(!sc.axes.is_empty());
        let last: Vec<u32> = sc.axes.iter().map(|a| a.len - 1).collect();
        p.nd_plane(&last, &cancel).unwrap().unwrap();
    }

    #[test]
    fn external_elements_from_the_same_folder() {
        let s = open("sds_ext.hdf");
        let p = page(&s, "external_i32");
        let plane = p.nd_plane(&[], &CancelToken::new()).unwrap().unwrap();
        assert!(plane.values.iter().all(|v| v.is_finite()));
        // Only plain names in the folder.
        let dir = data_dir();
        assert!(read_external(&dir, "sds_ext.dat", 0, 8).is_some());
        assert!(read_external(&dir, "../data/sds_ext.dat", 0, 8).is_none());
        assert!(read_external(&dir, "/etc/hostname", 0, 8).is_none());
        assert!(read_external(&dir, ".hidden", 0, 8).is_none());
    }

    #[test]
    fn hdf5_is_not_claimed() {
        assert!(!Hdf4Decoder.probe(b"\x89HDF\r\n\x1a\n\0\0\0\0"));
        assert!(Hdf4Decoder.probe(&[0x0e, 0x03, 0x13, 0x01, 0, 0]));
        let src = Source::from_vec("x.hdf", vec![0x0e, 0x03, 0x13, 0x01]);
        assert!(crate::Registry::new().open(Arc::new(src)).is_err());
    }

    #[test]
    fn lonlat_crosses_the_antimeridian() {
        let g = LonLatGrid {
            width: 2,
            height: 1,
            lon: Arc::from(vec![179.0f32, -179.0]),
            lat: Arc::from(vec![10.0f32, 10.0]),
            x_map: (0.5, 0.0),
            y_map: (1.0, 0.0),
        };
        let (lon, lat) = g.at(1.0, 0.0).unwrap();
        assert!(
            (lon - 180.0).abs() < 1e-6 || (lon + 180.0).abs() < 1e-6,
            "{lon}"
        );
        assert_eq!(lat, 10.0);
    }
}
