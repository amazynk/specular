//! Georeferenced grids through the science path, with map coordinates and a metre scale
//! bar: Esri ASCII grids, DTED elevation tiles, ENVI and ESRI `.hdr`-labelled raw rasters
//! (`.bil`/`.bsq`/`.bip` and kin) and ERDAS IMAGINE (`.img`) files, all read by
//! `specular-gis`. Single-band grids show as values (zscale grey, no-data transparent);
//! multi-band ones get a band axis labelled by wavelength (in nm) or band name.
//!
//! ENVI cubes with default bands (or wavelengths covering the visible) open on a page of
//! those bands as red, green and blue, with every band on the next page; bands the `bbl`
//! marks bad are left out of the spectrum at the cursor. Complex ENVI data shows through
//! [`crate::sar`]: magnitude, phase, dB and multilook.
//!
//! Also ER Mapper (`.ers` beside its data), SAGA grids (`.sgrd` with `.sdat`, or the
//! `.sg-grd-z` ZIP), Arc/Info binary grids (a folder of `.adf` files, coloured by a `.clr`
//! beside it) and BSB/KAP nautical charts (palette colours, placed by their reference
//! points).
//!
//! Also [`GeoWrap`]: a plain raster (JPEG, PNG…) that has no georeference of its own gets
//! the one its sidecar files give (world file, `.prj`, `.aux.xml`).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use specular_core::nd::{Limits, Stretch};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_gis::crs::Crs;
use specular_gis::geotiff::Affine2;
use specular_gis::{aig, asc, bsb, dted, envi, ers, hfa, saga, sidecar, usgsdem};

use crate::geo::{GeoRef, RasterGeo};
use crate::ident::Confidence;
use crate::science::{
    self, AxisKind, NdAxis, NdPlane, PixelSpectra, PlaneCache, ScienceKind, ScienceSummary,
    Spectrum, SpectrumSource, SpectrumX,
};
use crate::whole::{self, Levels};
use crate::{
    Alpha, AnimationInfo, CameraInfo, DecodeError, DecodeRequest, Decoder, EncapsulatedFacts,
    Format, Frames, ImageInfo, MedicalSummary, MeshModel, ModalityFrame, PageLabel, PointsModel,
    ProductView, Result, Session, VolumeFacts, sar,
};

/// Pixels per band at most.
const MAX_PIXELS: u64 = 1 << 28;
/// Bytes of a file looked at by the content probes (ASCII grid and ESRI label headers).
const PROBE: usize = 4096;
/// Header lines listed at most.
const MAX_KEYWORDS: usize = 4096;
/// The most a gzip ENVI data file may inflate to.
const MAX_GZ: usize = 8 << 30;

/// Which reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Esri ASCII grid (`.asc`), by content (PGP armour shares the extension).
    Asc,
    /// DTED levels 0–2.
    Dted,
    /// An ENVI or ESRI `.hdr` label (by content) or the raw data beside one.
    Envi,
    /// ERDAS IMAGINE (`.img`), by its `EHFA_HEADER_TAG`.
    Hfa,
    /// USGS DEM (`.dem`), by its A record's fixed-column codes.
    UsgsDem,
    /// ER Mapper (`.ers`).
    Ers,
    /// SAGA grid (`.sgrd`, `.sg-grd-z`).
    Saga,
    /// Arc/Info binary grid: its folder, or its `hdr.adf`.
    Aig,
    /// BSB/KAP chart (`.kap`), or the `.bsb` description naming its panels.
    Bsb,
    /// GDAL virtual raster (`.vrt`).
    Vrt,
}

pub struct GeoRasterDecoder(pub Kind);

/// Whether the start of a file is an ESRI `.hdr` label (not an ASCII grid, which also
/// names NROWS and NCOLS).
fn is_esri_label(head: &[u8]) -> bool {
    let t = String::from_utf8_lossy(&head[..head.len().min(PROBE)]);
    !t.contains('\0') && envi::sniff_esri(&t) && !asc::sniff(head)
}

impl Decoder for GeoRasterDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Asc => Format::EsriAscii,
            Kind::Dted => Format::Dted,
            Kind::Envi => Format::Envi,
            Kind::Hfa => Format::ErdasImagine,
            Kind::UsgsDem => Format::UsgsDem,
            Kind::Ers => Format::ErMapper,
            Kind::Saga => Format::SagaGrid,
            Kind::Aig => Format::ArcInfoGrid,
            Kind::Bsb => Format::BsbChart,
            Kind::Vrt => Format::GdalVrt,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::Asc => asc::sniff(head),
            Kind::Dted => dted::sniff(head),
            Kind::Envi => envi::sniff_envi(head) || (!head.is_empty() && is_esri_label(head)),
            Kind::Hfa => hfa::sniff(head),
            Kind::UsgsDem => usgsdem::sniff(head),
            Kind::Ers => ers::sniff(head),
            Kind::Saga => saga::sniff(head) || saga::sniff_zip(head),
            Kind::Aig => aig::sniff(head),
            Kind::Bsb => bsb::sniff(head) || bsb::sniff_description(head),
            Kind::Vrt => specular_gis::vrt::sniff(head),
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        self.probe(&data[..data.len().min(PROBE)])
    }
    /// A USGS DEM has no signature: fixed-column codes in their ranges are likely.
    fn confidence(&self, data: &[u8]) -> Option<Confidence> {
        let c = match self.0 {
            Kind::UsgsDem => Confidence::Likely,
            _ => Confidence::Certain,
        };
        self.probe_file(data).then_some(c)
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Dted => &["dt0", "dt1", "dt2"],
            Kind::UsgsDem => &["dem"],
            // Raw data named for its interleave; it needs its `.hdr` (see `envi_data`).
            Kind::Envi => &["bil", "bsq", "bip"],
            Kind::Ers => &["ers"],
            // The `.sdat` data opens through its header (see `header_beside`).
            Kind::Saga => &["sgrd", "sg-grd-z"],
            Kind::Bsb => &["kap", "bsb"],
            Kind::Vrt => &["vrt"],
            // A grid is its folder (`specular_core::is_aig_folder`); `hdr.adf` by content.
            // `.asc` (PGP armour) and `.img` (disk images) are claimed by content only.
            _ => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(match self.0 {
            Kind::Asc => GeoRasterSession::asc(source)?,
            Kind::Dted => GeoRasterSession::dted(source)?,
            Kind::Envi => GeoRasterSession::envi(source, 0)?,
            Kind::Hfa => GeoRasterSession::hfa(source)?,
            Kind::UsgsDem => GeoRasterSession::usgs_dem(source)?,
            Kind::Ers => GeoRasterSession::ers(source)?,
            Kind::Saga => GeoRasterSession::saga(source)?,
            Kind::Aig => GeoRasterSession::aig(source)?,
            Kind::Bsb => GeoRasterSession::bsb(source)?,
            Kind::Vrt => GeoRasterSession::vrt(source)?,
        }))
    }
}

/// The header that describes raw data at `path`: the `.sgrd` beside a `.sdat`, or the
/// `<name>.ers` beside ER Mapper data (read by [`crate::Registry::open`] for files no
/// probe recognises).
pub fn header_beside(path: &Path) -> Option<std::path::PathBuf> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if ext.as_deref() == Some("sdat") {
        return sidecar::find(path, "sgrd");
    }
    let h = ers::header_file(path)?;
    sidecar::read_limited(&h, PROBE as u64)
        .is_some_and(|b| ers::sniff(&b))
        .then_some(h)
}

/// Whether `path` is raw data described by an ENVI or ESRI `.hdr` beside it (read and
/// cached by [`crate::sniff`] for listings; [`crate::Registry::open`] asks it for files no
/// probe recognises).
pub fn envi_data(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if ext == "hdr" || !envi::DATA_EXTENSIONS.contains(&ext.as_str()) {
        return false;
    }
    envi::header_file(path)
        .and_then(|h| sidecar::read_limited(&h, PROBE as u64 * 16))
        .is_some_and(|h| envi::sniff_envi(&h) || is_esri_label(&h))
}

fn info(format: Format, w: u32, h: u32, bits: u8, coding: String, alpha: bool) -> ImageInfo {
    ImageInfo {
        format,
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: alpha,
        bit_depth: bits,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    }
}

fn kw(k: &str, v: String) -> (String, String, String) {
    (k.to_owned(), v, String::new())
}

fn mapped(e: String) -> DecodeError {
    DecodeError::Malformed(e)
}

fn check_size(w: u32, h: u32) -> Result<()> {
    if w == 0 || h == 0 {
        return Err(DecodeError::Malformed("empty grid".into()));
    }
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(DecodeError::TooLarge);
    }
    Ok(())
}

fn stem(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// A wavelength in nanometres, from a value in `unit`, when the unit is a length.
fn nanometres(v: f64, unit: Option<&str>) -> Option<f64> {
    Some(
        v * match unit? {
            "nm" => 1.0,
            "µm" => 1e3,
            "mm" => 1e6,
            "cm" => 1e7,
            "m" => 1e9,
            _ => return None,
        },
    )
}

/// "450.5 nm" with at most three decimals.
fn trimmed(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_owned()
}

/// The band values of a raw raster's pixel against the bands' wavelengths (an ENVI
/// hyperspectral cube).
#[derive(Debug)]
struct BandSpectra {
    data: Arc<Source>,
    layout: specular_gis::rawgrid::RawLayout,
    nodata: Option<f64>,
    x: Arc<[f64]>,
    unit: String,
    /// Per band, whether it is good (ENVI `bbl`); empty when all are.
    good: Vec<bool>,
}

impl SpectrumSource for BandSpectra {
    fn grid(&self) -> (usize, usize) {
        (self.layout.width as usize, self.layout.height as usize)
    }
    fn channels(&self) -> usize {
        self.layout.bands as usize
    }
    fn x_axis(&self) -> (String, String) {
        ("Wavelength".into(), self.unit.clone())
    }
    fn at(&self, x: usize, y: usize, _index: &[u32]) -> Option<Spectrum> {
        let l = &self.layout;
        let b = self.data.bytes();
        let n = l.sample.bytes();
        let base = l.offset + y as u64 * l.row_stride + x as u64 * l.pixel_stride;
        let values = (0..u64::from(l.bands))
            .map(|k| {
                let at = usize::try_from(base + k * l.band_stride).ok()?;
                let v = l.sample.read(b.get(at..at.checked_add(n)?)?, l.big_endian);
                Some(match self.nodata {
                    _ if !self.good.get(k as usize).copied().unwrap_or(true) => f32::NAN,
                    Some(nd) if v == nd || (nd.is_nan() && v.is_nan()) => f32::NAN,
                    _ => v as f32,
                })
            })
            .collect::<Option<Vec<f32>>>()?;
        Some(Spectrum {
            y: values,
            x: SpectrumX::Values(self.x.clone()),
            sticks: false,
        })
    }
}

/// What a session reads its planes from.
enum Data {
    Asc(asc::Header),
    Dted(dted::Header),
    UsgsDem(Box<usgsdem::Header>),
    /// The raw data file (the opened source, or the one an opened header names).
    Raw {
        data: Arc<Source>,
        layout: specular_gis::rawgrid::RawLayout,
        nodata: Option<f64>,
    },
    /// The image, and its external spill file (`.ige`) when its layers live there.
    Hfa(Box<hfa::Image>, Option<Arc<Source>>),
    /// A SAGA grid's header and data.
    Saga(Box<saga::Header>, Arc<Source>),
    /// An Arc/Info grid and its folder.
    Aig(Box<aig::Grid>, std::path::PathBuf),
    /// A chart and its palette indices (decoded once, for the three colour planes).
    Bsb(Box<bsb::Chart>, std::sync::OnceLock<(Vec<u8>, bool)>),
    /// A virtual raster's bands, put together from its sources.
    Vrt(Box<crate::vrt_dec::VrtPlan>),
}

struct GeoRasterSession {
    source: Arc<Source>,
    info: ImageInfo,
    summary: ScienceSummary,
    geo: Option<Arc<GeoRef>>,
    data: Data,
    planes: PlaneCache,
    levels: Levels,
    /// The bands (0-based) a composite page shows as red, green and blue.
    composite: Option<[u32; 3]>,
    /// The page shown and how many there are (an ENVI cube: composite, then all bands).
    page: u32,
    pages: u32,
    /// Complex samples, shown through [`crate::sar`].
    complex: bool,
    /// The colour planes are 8-bit colours shown as they are (no stretch).
    exact: bool,
    /// Colours of values (an Arc/Info grid's `.clr`), for the picture.
    palette: Option<Arc<std::collections::HashMap<i64, [u8; 3]>>>,
}

/// A raw raster's complex samples.
struct RawComplex<'a> {
    data: &'a Source,
    layout: &'a specular_gis::rawgrid::RawLayout,
}

impl sar::ComplexSource for RawComplex<'_> {
    fn band(&self, b: u32, _cancel: &CancelToken) -> Result<(Vec<[f32; 2]>, bool)> {
        Ok(self.layout.band_complex(self.data.bytes(), b))
    }
}

/// The bands of a cube to show as red, green and blue (0-based): its three `default
/// bands`, else (with wavelengths in nm covering 470–630) the good ones nearest 640, 550
/// and 460 nm.
fn composite_bands(l: &envi::Labelled) -> Option<[u32; 3]> {
    let n = l.layout.bands;
    let good = |b: u32| l.good_bands.get(b as usize).copied().unwrap_or(true);
    if l.default_bands.len() == 3 {
        let b: Vec<u32> = l.default_bands.iter().map(|&b| b.wrapping_sub(1)).collect();
        if b.iter().all(|&b| b < n) {
            return Some([b[0], b[1], b[2]]);
        }
    }
    if l.wavelengths.len() != n as usize {
        return None;
    }
    let unit = l.wavelength_unit.as_deref();
    let nm: Vec<f64> = l
        .wavelengths
        .iter()
        .map(|&w| nanometres(w, unit))
        .collect::<Option<_>>()?;
    let (lo, hi) = nm
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &w| {
            (a.min(w), b.max(w))
        });
    if lo > 470.0 || hi < 630.0 {
        return None;
    }
    let nearest = |t: f64| {
        (0..n).filter(|&b| good(b)).min_by(|&a, &b| {
            (nm[a as usize] - t)
                .abs()
                .total_cmp(&(nm[b as usize] - t).abs())
        })
    };
    Some([nearest(640.0)?, nearest(550.0)?, nearest(460.0)?])
}

/// The georeference of a grid: its own transform and CRS, a CRS from a `.prj` beside it
/// when it has none, the header lines as the info panel's fields.
struct Georef {
    transform: Option<Affine2>,
    crs: Option<Crs>,
    crs_note: Option<String>,
    nodata: Option<f64>,
    keys: Vec<(String, String)>,
}

impl Georef {
    fn build(self, data: &Source, (w, h): (u32, u32)) -> Option<Arc<GeoRef>> {
        let mut sidecars = Vec::new();
        let mut crs = self.crs;
        let mut transform = self.transform;
        let mut gdal = Default::default();
        let mut scale_offset = None;
        let mut unit = None;
        let mut band_name = None;
        let mut nodata = self.nodata;
        // A `.prj` (Esri grids), a world file (an ESRI label without map keys) and GDAL's
        // `.aux.xml` add what the header lacks.
        let s = if crate::geo::on_disk(data) {
            sidecar::georef(data.path(), transform.is_none())
        } else {
            Default::default()
        };
        if !s.is_empty() {
            sidecars = s.files.clone();
            transform = transform.or(s.transform);
            if crs.is_none() {
                crs = s.crs.clone();
            }
            if let Some(aux) = &s.aux {
                gdal = aux.to_gdal();
                (scale_offset, unit, band_name) = crate::geo::band_roles(&gdal);
                nodata = nodata.or(aux.band(1).and_then(|b| b.nodata));
            }
        }
        let to_map = transform?;
        Some(Arc::new(crate::geo::raster(
            to_map,
            crs,
            self.crs_note,
            (w, h),
            RasterGeo {
                levels: vec![(w, h)],
                nodata,
                keys: self.keys,
                keys_kind: "header",
                gdal,
                values: true,
                scale_offset,
                unit,
                band_name,
                sidecars,
                ..Default::default()
            },
        )))
    }
}

/// A single grid's summary.
fn grid_summary(w: u32, h: u32, sample: String, title: Option<String>) -> ScienceSummary {
    ScienceSummary {
        width: w,
        height: h,
        sample,
        title,
        kind: ScienceKind::Gridded,
        ..Default::default()
    }
}

impl GeoRasterSession {
    fn new(
        source: Arc<Source>,
        info: ImageInfo,
        mut summary: ScienceSummary,
        georef: Georef,
        data: Data,
    ) -> GeoRasterSession {
        let file = match &data {
            Data::Raw { data, .. } => data.clone(),
            _ => source.clone(),
        };
        summary.keywords = Arc::new(
            georef
                .keys
                .iter()
                .take(MAX_KEYWORDS)
                .map(|(k, v)| kw(k, v.clone()))
                .collect(),
        );
        let geo = georef.build(&file, (info.width, info.height));
        if let Some(crate::geo::GeoFacts::Raster(r)) = geo.as_deref().map(|g| &g.facts) {
            if summary.unit.is_none() {
                summary.unit = r.unit.clone();
            }
            if summary.title.is_none() {
                summary.title = r.band_name.clone();
            }
        }
        summary.geo = geo.clone();
        GeoRasterSession {
            source,
            info,
            summary,
            geo,
            data,
            planes: PlaneCache::new(),
            levels: Levels::new(),
            composite: None,
            page: 0,
            pages: 1,
            complex: false,
            exact: false,
            palette: None,
        }
    }

    fn asc(source: Arc<Source>) -> Result<GeoRasterSession> {
        let h = asc::header(source.bytes()).map_err(mapped)?;
        check_size(h.ncols, h.nrows)?;
        let summary = grid_summary(h.ncols, h.nrows, "decimal text".into(), stem(source.path()));
        let info = info(
            Format::EsriAscii,
            h.ncols,
            h.nrows,
            32,
            "Esri ASCII grid".into(),
            h.nodata.is_some(),
        );
        let georef = Georef {
            transform: Some(h.transform),
            crs: None,
            crs_note: None,
            nodata: h.nodata,
            keys: h.keys.clone(),
        };
        Ok(Self::new(source, info, summary, georef, Data::Asc(h)))
    }

    fn dted(source: Arc<Source>) -> Result<GeoRasterSession> {
        let h = dted::header(source.bytes()).map_err(mapped)?;
        check_size(h.lon_lines, h.lat_points)?;
        let level = h.level.clone().unwrap_or_else(|| "DTED".into());
        let mut summary = grid_summary(
            h.lon_lines,
            h.lat_points,
            "16-bit signed magnitude, big-endian".into(),
            stem(source.path()),
        );
        summary.unit = Some("m".into());
        let info = info(Format::Dted, h.lon_lines, h.lat_points, 16, level, true);
        // DTED is on WGS 84, older tiles on WGS 72.
        let datum = h.horizontal_datum.as_deref().unwrap_or("WGS84");
        let code = match datum.replace([' ', '-'], "").to_ascii_uppercase().as_str() {
            "WGS84" => Some(4326),
            "WGS72" => Some(4322),
            _ => None,
        };
        let crs = code.and_then(specular_gis::crs::from_epsg);
        let crs_note = crs.is_none().then(|| datum.to_owned());
        let georef = Georef {
            transform: Some(h.transform()),
            crs,
            crs_note,
            nodata: Some(f64::from(dted::VOID)),
            keys: h.facts(),
        };
        Ok(Self::new(source, info, summary, georef, Data::Dted(h)))
    }

    fn usgs_dem(source: Arc<Source>) -> Result<GeoRasterSession> {
        let h = usgsdem::header(source.bytes()).map_err(mapped)?;
        check_size(h.width, h.height)?;
        let mut summary = grid_summary(
            h.width,
            h.height,
            "ASCII profiles, 16-bit integers".into(),
            stem(source.path()),
        );
        summary.unit = Some(h.unit().into());
        let info = info(
            Format::UsgsDem,
            h.width,
            h.height,
            16,
            format!("USGS DEM level {}", h.level),
            true,
        );
        let crs = h.epsg().and_then(specular_gis::crs::from_epsg);
        let crs_note = crs
            .is_none()
            .then(|| format!("State Plane zone {}", h.zone));
        let georef = Georef {
            transform: Some(h.transform()),
            crs,
            crs_note,
            nodata: Some(f64::from(usgsdem::VOID)),
            keys: h.facts(),
        };
        Ok(Self::new(
            source,
            info,
            summary,
            georef,
            Data::UsgsDem(Box::new(h)),
        ))
    }

    /// An ENVI or ESRI-labelled raster; `page` 0 is the composite of a cube that has one,
    /// 1 its bands.
    fn envi(source: Arc<Source>, page: u32) -> Result<GeoRasterSession> {
        let head = &source.bytes()[..source.len().min(PROBE)];
        let opened_header = envi::sniff_envi(head) || is_esri_label(head);
        let (text, data) = if opened_header {
            let text = String::from_utf8_lossy(source.bytes()).into_owned();
            let p = envi::data_file(source.path()).ok_or_else(|| {
                DecodeError::Unsupported("the data file beside the header is missing".into())
            })?;
            (text, Arc::new(Source::open(&p)?))
        } else {
            let h = envi::header_file(source.path())
                .ok_or_else(|| DecodeError::Unsupported("no .hdr header beside the data".into()))?;
            let text = sidecar::read_text(&h)
                .ok_or_else(|| DecodeError::Unsupported("unreadable .hdr header".into()))?;
            (text, source.clone())
        };
        let l = envi::read_header(&text, data.path()).map_err(mapped)?;
        let lay = l.layout;
        check_size(lay.width, lay.height)?;
        // `file compression = 1`: the data file is gzip.
        let gzip = l.kind == envi::HeaderKind::Envi
            && l.keys
                .iter()
                .any(|(k, v)| k == "file compression" && v.trim() == "1");
        let data = if gzip && data.bytes().starts_with(&[0x1f, 0x8b]) {
            // Room for the samples the header names, with slack for trailing bytes.
            let want = usize::try_from(lay.end().saturating_mul(2).saturating_add(1 << 16))
                .unwrap_or(usize::MAX)
                .min(MAX_GZ);
            let bytes = specular_png::inflate_gzip(data.bytes(), want).map_err(|e| match e {
                specular_png::InflateError::NeedsMemory(n) => DecodeError::NeedsMemory(n),
                _ => DecodeError::Malformed("gzip".into()),
            })?;
            Arc::new(Source::from_vec(data.path(), bytes))
        } else {
            data
        };
        if lay.bands == 0 || lay.bands > 100_000 {
            return Err(DecodeError::Malformed("band count".into()));
        }
        let format = match l.kind {
            envi::HeaderKind::Envi => Format::Envi,
            envi::HeaderKind::Esri => Format::EsriBil,
        };
        let sample = format!(
            "{}, {}{}",
            lay.sample.name(),
            lay.interleave.name(),
            if lay.sample.bytes() > 1 {
                if lay.big_endian {
                    ", big-endian"
                } else {
                    ", little-endian"
                }
            } else {
                ""
            }
        );
        let title = l.description.clone().or_else(|| stem(data.path()));
        let mut summary = grid_summary(lay.width, lay.height, sample.clone(), title);
        let complex = lay.sample.is_complex();
        let composite = (!complex && lay.bands > 3)
            .then(|| composite_bands(&l))
            .flatten();
        if composite.is_some_and(|_| page > 1) || (composite.is_none() && page > 0) {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        if lay.bands > 1 {
            // Wavelengths in nm when they are lengths, else in the header's unit.
            let unit = l.wavelength_unit.as_deref();
            let labels: Vec<String> = if l.wavelengths.len() == lay.bands as usize {
                l.wavelengths
                    .iter()
                    .map(|&w| match nanometres(w, unit) {
                        Some(nm) => format!("{} nm", trimmed(nm)),
                        None => format!("{} {}", trimmed(w), unit.unwrap_or(""))
                            .trim()
                            .into(),
                    })
                    .collect()
            } else if l.band_names.len() == lay.bands as usize {
                l.band_names.clone()
            } else {
                Vec::new()
            };
            let wavelengths = l.wavelengths.len() == lay.bands as usize;
            summary.axes.push(NdAxis {
                name: if wavelengths { "Wavelength" } else { "Band" }.into(),
                len: lay.bands,
                unit: if wavelengths && nanometres(1.0, unit).is_some() {
                    "nm".into()
                } else {
                    unit.unwrap_or("").into()
                },
                labels,
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Channel,
            });
            // ENVI's default bands: the first one (1-based) shows first; three named
            // ones in band order make an RGB composite.
            if let Some(&b) = l.default_bands.first()
                && b >= 1
                && b <= lay.bands
            {
                summary.start = vec![b - 1];
            }
            if lay.bands == 3 && !wavelengths && !complex {
                summary.channel_axis = Some(0);
            }
            // The composite page: those three bands as the channel axis.
            if let Some(c) = composite
                && page == 0
            {
                let a = &mut summary.axes[0];
                a.len = 3;
                a.labels = if a.labels.len() == lay.bands as usize {
                    c.iter().map(|&b| a.labels[b as usize].clone()).collect()
                } else {
                    c.iter().map(|&b| format!("{}", b + 1)).collect()
                };
                summary.channel_axis = Some(0);
                summary.start = Vec::new();
            }
            // A hyperspectral cube: each pixel's bands against their wavelengths.
            if wavelengths && lay.bands >= 4 {
                let nm = nanometres(1.0, unit).is_some();
                let x: Arc<[f64]> = l
                    .wavelengths
                    .iter()
                    .map(|&w| {
                        if nm {
                            nanometres(w, unit).unwrap_or(w)
                        } else {
                            w
                        }
                    })
                    .collect();
                summary.spectra = Some(PixelSpectra::new(
                    Arc::new(BandSpectra {
                        data: data.clone(),
                        layout: lay,
                        nodata: l.nodata,
                        x,
                        unit: if nm {
                            "nm".into()
                        } else {
                            unit.unwrap_or("").into()
                        },
                        good: l.good_bands.clone(),
                    }),
                    lay.width,
                    lay.height,
                ));
            }
        }
        let mut keys = l.keys.clone();
        keys.truncate(MAX_KEYWORDS);
        let info = info(
            format,
            lay.width,
            lay.height,
            lay.sample.bits(),
            sample,
            l.nodata.is_some(),
        );
        let georef = Georef {
            transform: l.transform,
            crs: l.crs.clone(),
            crs_note: l.crs_note.clone(),
            nodata: l.nodata,
            keys,
        };
        if complex {
            summary.axes.extend(sar::axes());
            summary.start = sar::start(lay.bands > 1);
        }
        let data = Data::Raw {
            data,
            layout: lay,
            nodata: l.nodata,
        };
        let mut s = Self::new(source, info, summary, georef, data);
        s.complex = complex;
        s.page = page;
        if composite.is_some() {
            s.pages = 2;
            s.composite = composite.filter(|_| page == 0);
        }
        Ok(s)
    }

    fn hfa(source: Arc<Source>) -> Result<GeoRasterSession> {
        let img = hfa::open(source.bytes()).map_err(mapped)?;
        check_size(img.width, img.height)?;
        // Large images keep their tiles in a spill file beside the .img, which the file
        // names (only its base name is used: it is looked for beside the .img).
        let spill_name = img.spill_files().into_iter().next();
        let spill = match &spill_name {
            Some(name) => {
                let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
                let found = sidecar::find_name(source.path(), base).ok_or_else(|| {
                    DecodeError::Unsupported(format!("the spill file {base} is missing"))
                })?;
                Some(Arc::new(Source::open(&found)?))
            }
            None => None,
        };
        let first = img
            .bands
            .first()
            .ok_or_else(|| DecodeError::Malformed("no layers".into()))?;
        if let Some(why) = &first.unreadable
            && img.bands.iter().all(|b| b.unreadable.is_some())
        {
            return Err(DecodeError::Unsupported(why.clone()));
        }
        let sample = first.pixel.name().to_owned();
        let n = img.bands.len() as u32;
        let mut summary = grid_summary(img.width, img.height, sample.clone(), stem(source.path()));
        if n > 1 {
            summary.axes.push(NdAxis {
                name: "Layer".into(),
                len: n,
                unit: String::new(),
                labels: img.bands.iter().map(|b| b.name.clone()).collect(),
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Channel,
            });
            // Three or four 8-bit layers are a colour image.
            if (n == 3 || n == 4) && img.bands.iter().all(|b| b.pixel.bits() == 8) {
                summary.channel_axis = Some(0);
            }
        }
        let alpha = img
            .bands
            .iter()
            .any(|b| b.nodata.is_some() || b.blocks.iter().any(|k| !k.valid));
        let info = info(
            Format::ErdasImagine,
            img.width,
            img.height,
            first.pixel.bits().min(255) as u8,
            sample,
            alpha,
        );
        let georef = Georef {
            transform: img.transform,
            crs: img.crs.clone(),
            crs_note: img.crs_note.clone(),
            nodata: first.nodata,
            keys: img.facts.clone(),
        };
        Ok(Self::new(
            source,
            info,
            summary,
            georef,
            Data::Hfa(Box::new(img), spill),
        ))
    }

    fn ers(source: Arc<Source>) -> Result<GeoRasterSession> {
        let text = String::from_utf8_lossy(source.bytes()).into_owned();
        let h = ers::parse(&text).map_err(refused)?;
        let lay = h.layout;
        check_size(lay.width, lay.height)?;
        let missing =
            || DecodeError::Unsupported("the data file beside the header is missing".into());
        let p = crate::geo::on_disk(&source)
            .then(|| ers::data_file(source.path(), &h))
            .flatten()
            .ok_or_else(missing)?;
        let data = Arc::new(Source::open(&p)?);
        let sample = format!("{}, BIL{}", lay.sample.name(), byte_order(&lay));
        let mut summary = grid_summary(lay.width, lay.height, sample.clone(), stem(source.path()));
        summary.unit = h.unit.clone();
        if lay.bands > 1 {
            let labels = if h.band_names.len() == lay.bands as usize {
                h.band_names.clone()
            } else {
                Vec::new()
            };
            let rgb = lay.bands == 3
                && (lay.sample.bytes() == 1
                    || labels
                        .iter()
                        .zip(["red", "green", "blue"])
                        .all(|(l, c)| l.eq_ignore_ascii_case(c)));
            summary.axes.push(NdAxis {
                name: "Band".into(),
                len: lay.bands,
                unit: String::new(),
                labels,
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Channel,
            });
            if rgb {
                summary.channel_axis = Some(0);
            }
        }
        let info = info(
            Format::ErMapper,
            lay.width,
            lay.height,
            lay.sample.bits(),
            sample,
            h.nodata.is_some(),
        );
        let georef = Georef {
            transform: h.transform,
            crs: h.crs.clone(),
            crs_note: h.crs_note.clone(),
            nodata: h.nodata,
            keys: h.keys.clone(),
        };
        let data = Data::Raw {
            data,
            layout: lay,
            nodata: h.nodata,
        };
        Ok(Self::new(source, info, summary, georef, data))
    }

    fn saga(source: Arc<Source>) -> Result<GeoRasterSession> {
        let head = &source.bytes()[..source.len().min(PROBE)];
        let (text, data, prj) = if saga::sniff_zip(head) {
            let z = saga::read_zip(source.bytes(), MAX_GZ as u64).map_err(mapped)?;
            let data = Arc::new(Source::from_vec(source.path(), z.data));
            (z.header, data, z.prj)
        } else {
            let missing =
                || DecodeError::Unsupported("the .sdat data beside the header is missing".into());
            let p = crate::geo::on_disk(&source)
                .then(|| sidecar::find(source.path(), "sdat"))
                .flatten()
                .ok_or_else(missing)?;
            let text = String::from_utf8_lossy(source.bytes()).into_owned();
            (text, Arc::new(Source::open(&p)?), None)
        };
        let h = saga::parse(&text).map_err(refused)?;
        let lay = h.layout;
        check_size(lay.width, lay.height)?;
        let sample = format!("{}{}", lay.sample.name(), byte_order(&lay));
        let title = h
            .name
            .clone()
            .or_else(|| h.description.clone())
            .or_else(|| stem(source.path()));
        let mut summary = grid_summary(lay.width, lay.height, sample.clone(), title);
        summary.unit = h.unit.clone();
        let info = info(
            Format::SagaGrid,
            lay.width,
            lay.height,
            lay.sample.bits(),
            sample,
            h.nodata.is_some(),
        );
        let crs = prj.as_deref().and_then(specular_gis::worldfile::parse_prj);
        let georef = Georef {
            transform: Some(h.transform),
            crs,
            crs_note: None,
            nodata: h.nodata.map(|n| n.0),
            keys: h.keys.clone(),
        };
        Ok(Self::new(
            source,
            info,
            summary,
            georef,
            Data::Saga(Box::new(h), data),
        ))
    }

    fn aig(source: Arc<Source>) -> Result<GeoRasterSession> {
        let dir = if source.is_folder() {
            source.path().to_path_buf()
        } else if crate::geo::on_disk(&source) {
            source
                .path()
                .parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| DecodeError::Unsupported("no grid folder".into()))?
        } else {
            return Err(DecodeError::Unsupported(
                "an Arc/Info grid is read from its folder".into(),
            ));
        };
        let file = |n: &str| aig_file(&dir, n);
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_owned();
        // `<grid>.clr` in the grid's folder or beside it.
        let clr = sidecar::find_name(&dir.join("hdr.adf"), &format!("{name}.clr"))
            .or_else(|| sidecar::find_name(&dir, &format!("{name}.clr")))
            .and_then(|p| sidecar::read_text(&p));
        let g = aig::open(&file, clr.as_deref()).map_err(refused)?;
        check_size(g.width, g.height)?;
        let sample = if g.integer {
            "32-bit integer cells"
        } else {
            "32-bit float cells"
        }
        .to_string();
        let title = (!name.is_empty()).then(|| name.clone());
        let summary = grid_summary(g.width, g.height, sample.clone(), title);
        let info = info(Format::ArcInfoGrid, g.width, g.height, 32, sample, true);
        let crs_note = g
            .crs
            .is_none()
            .then(|| g.prj.as_deref().and_then(|p| p.lines().next()))
            .flatten()
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|l| !l.is_empty());
        let georef = Georef {
            transform: Some(g.transform()),
            crs: g.crs.clone(),
            crs_note,
            nodata: None,
            keys: g.facts(),
        };
        let palette =
            (!g.palette.is_empty()).then(|| Arc::new(g.palette.iter().copied().collect()));
        let mut s = Self::new(source, info, summary, georef, Data::Aig(Box::new(g), dir));
        s.palette = palette;
        Ok(s)
    }

    fn bsb(source: Arc<Source>) -> Result<GeoRasterSession> {
        let head = &source.bytes()[..source.len().min(8192)];
        if !bsb::sniff(head) && bsb::sniff_description(head) {
            // A `.bsb` chart description: its first panel, beside it.
            let text: String = source.bytes().iter().map(|&b| char::from(b)).collect();
            let panel = bsb::first_panel(&text)
                .ok_or_else(|| DecodeError::Malformed("the chart names no panel".into()))?;
            let base = panel
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&panel)
                .to_owned();
            let p = crate::geo::on_disk(&source)
                .then(|| sidecar::find_name(source.path(), &base))
                .flatten()
                .ok_or_else(|| {
                    DecodeError::Unsupported(format!("the chart panel {base} is missing"))
                })?;
            let panel = Arc::new(Source::open(&p)?);
            if !bsb::sniff(&panel.bytes()[..panel.len().min(8192)]) {
                return Err(DecodeError::Malformed(format!("{base} is not a chart")));
            }
            return Self::bsb(panel);
        }
        let c = bsb::parse(source.bytes()).map_err(mapped)?;
        check_size(c.width, c.height)?;
        let colours = c.palette.len().saturating_sub(1);
        let sample = format!(
            "{}-bit palette, {colours} colours, run-length coded",
            c.depth
        );
        let mut summary = grid_summary(c.width, c.height, sample.clone(), c.name.clone());
        summary.axes.push(NdAxis {
            name: "Colour".into(),
            len: 3,
            unit: String::new(),
            labels: vec!["Red".into(), "Green".into(), "Blue".into()],
            wcs_axis: None,
            linear: None,
            kind: AxisKind::Channel,
        });
        summary.channel_axis = Some(0);
        let info = info(Format::BsbChart, c.width, c.height, 8, sample, false);
        let placed = c.georef();
        let pr = c.projection.clone().unwrap_or_default();
        let crs_note = placed.as_ref().and_then(|(_, _, miss, own)| {
            if !own {
                let what = if pr.is_empty() {
                    "an unnamed projection"
                } else {
                    &pr
                };
                Some(format!(
                    "placed by its reference points in longitude and latitude (the chart is in {what}); up to {miss:.1} px off"
                ))
            } else {
                (*miss > 2.0)
                    .then(|| format!("placed by its reference points; up to {miss:.1} px off"))
            }
        });
        let mut keys = c.keys.clone();
        keys.truncate(MAX_KEYWORDS);
        let georef = Georef {
            transform: placed.as_ref().map(|p| p.0),
            crs: placed.map(|p| p.1),
            crs_note,
            nodata: None,
            keys,
        };
        let mut s = Self::new(
            source,
            info,
            summary,
            georef,
            Data::Bsb(Box::new(c), std::sync::OnceLock::new()),
        );
        s.exact = true;
        Ok(s)
    }

    fn vrt(source: Arc<Source>) -> Result<GeoRasterSession> {
        let plan = crate::vrt_dec::VrtPlan::open(source.clone())?;
        let v = &plan.vrt;
        check_size(v.width, v.height)?;
        let n = v.bands.len() as u32;
        let first = &v.bands[0];
        let all_byte = v.bands.iter().all(|b| b.data_type == "Byte");
        let types: Vec<&str> = {
            let mut t: Vec<&str> = v.bands.iter().map(|b| b.data_type.as_str()).collect();
            t.dedup();
            t
        };
        let sources: usize = v.bands.iter().map(|b| b.sources.len()).sum();
        let sample = format!(
            "{}, {sources} source{}",
            types.join("/"),
            if sources == 1 { "" } else { "s" }
        );
        let bits = specular_gis::vrt::type_bytes(&first.data_type).unwrap_or(1) * 8;
        let mut summary = grid_summary(v.width, v.height, sample.clone(), stem(source.path()));
        summary.unit = first.unit.clone();
        let interp = |b: &specular_gis::vrt::Band| {
            b.color_interp
                .as_deref()
                .map(str::to_ascii_lowercase)
                .unwrap_or_default()
        };
        let mut exact = false;
        if n > 1 {
            let labels: Vec<String> = v
                .bands
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    b.description
                        .clone()
                        .or_else(|| b.color_interp.clone())
                        .unwrap_or_else(|| format!("{}", i + 1))
                })
                .collect();
            summary.axes.push(NdAxis {
                name: "Band".into(),
                len: n,
                unit: String::new(),
                labels,
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Channel,
            });
            let rgb = n >= 3
                && interp(&v.bands[0]) == "red"
                && interp(&v.bands[1]) == "green"
                && interp(&v.bands[2]) == "blue";
            if rgb || (all_byte && (n == 3 || n == 4)) {
                summary.channel_axis = Some(0);
                exact = all_byte;
            }
        }
        let palette = (n == 1 && !first.palette.is_empty()).then(|| {
            Arc::new(
                first
                    .palette
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i as i64, [c[0], c[1], c[2]]))
                    .collect(),
            )
        });
        let info = info(
            Format::GdalVrt,
            v.width,
            v.height,
            bits.min(64) as u8,
            sample,
            first.nodata.is_some(),
        );
        let crs = v
            .srs
            .as_deref()
            .and_then(specular_gis::worldfile::parse_prj);
        let crs_note = (crs.is_none())
            .then(|| v.srs.clone())
            .flatten()
            .map(|s| s.chars().take(120).collect());
        let mut keys = vec![
            ("Size".to_owned(), format!("{} × {}", v.width, v.height)),
            ("Bands".to_owned(), n.to_string()),
        ];
        for (i, b) in v.bands.iter().enumerate().take(64) {
            let files: Vec<&str> = b.sources.iter().take(4).map(|s| s.file.trim()).collect();
            let more = b.sources.len().saturating_sub(4);
            keys.push((
                format!("Band {}", i + 1),
                format!(
                    "{}{}: {}{}",
                    b.data_type,
                    b.nodata.map_or(String::new(), |n| format!(", no data {n}")),
                    files.join(", "),
                    if more > 0 {
                        format!(" and {more} more")
                    } else {
                        String::new()
                    }
                ),
            ));
        }
        let georef = Georef {
            transform: v.transform,
            crs,
            crs_note,
            nodata: first.nodata,
            keys,
        };
        let mut s = Self::new(source, info, summary, georef, Data::Vrt(Box::new(plan)));
        s.exact = exact;
        s.palette = palette;
        Ok(s)
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        if self.complex
            && let Data::Raw { data, layout, .. } = &self.data
        {
            let src = RawComplex { data, layout };
            let size = (self.info.width, self.info.height);
            return sar::plane(&src, &self.planes, size, index, layout.bands > 1, cancel);
        }
        let mut band = index.first().copied().unwrap_or(0);
        if band >= self.summary.axes.first().map_or(1, |a| a.len) {
            return Err(DecodeError::Malformed("no such band".into()));
        }
        if let Some(c) = self.composite {
            band = c[band as usize];
        }
        let (w, h) = (self.info.width, self.info.height);
        self.planes.get_or(index, cancel, || {
            // The band's f32 values (a DTED tile's header alone can ask for 400 MB).
            let _res = specular_core::budget::reserve_n(u64::from(w) * u64::from(h), 4)?;
            let (values, truncated) = match &self.data {
                Data::Asc(hd) => {
                    let (v, t, _) = asc::values(self.source.bytes(), hd);
                    (v, t)
                }
                Data::Dted(hd) => hd.values(self.source.bytes()),
                Data::UsgsDem(hd) => hd.values(self.source.bytes()),
                Data::Raw {
                    data,
                    layout,
                    nodata,
                } => layout.band(data.bytes(), band, *nodata),
                Data::Hfa(img, spill) => img.band_values_with(
                    self.source.bytes(),
                    spill.as_ref().map(|s| s.bytes()),
                    band as usize,
                ),
                Data::Saga(h, data) => h.values(data.bytes()),
                Data::Vrt(plan) => plan.band(band, cancel)?,
                Data::Aig(g, dir) => g.values(&|n| aig_file(dir, n)),
                Data::Bsb(c, idx) => {
                    let (ix, bad) = idx.get_or_init(|| c.indices(self.source.bytes()));
                    let ch = band as usize;
                    let v = ix
                        .iter()
                        .map(|&i| {
                            c.palette
                                .get(usize::from(i))
                                .map_or(0.0, |p| f32::from(p[ch]))
                        })
                        .collect();
                    (v, *bad)
                }
            };
            if values.len() != w as usize * h as usize {
                return Err(DecodeError::Malformed("band size".into()));
            }
            Ok(NdPlane {
                width: w,
                height: h,
                values,
                truncated,
            })
        })
    }

    /// The picture for thumbnails and the CPU path: the colour composite, or the first
    /// plane as zscale grey with no-data transparent.
    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        if self.complex
            && let Data::Raw { data, layout, .. } = &self.data
        {
            let src = RawComplex { data, layout };
            let size = (self.info.width, self.info.height);
            return sar::picture(&src, &self.planes, size, layout.bands > 1, cancel);
        }
        if self.summary.channel_axis.is_some() {
            let planes = (0..3)
                .map(|b| self.plane(&[b], cancel))
                .collect::<Result<Vec<_>>>()?;
            if self.exact {
                return Ok(exact_rgba(&planes));
            }
            return Ok(science::auto_raster(&planes, None));
        }
        let idx = match self.summary.axes.len() {
            0 => Vec::new(),
            _ => vec![self.summary.start.first().copied().unwrap_or(0)],
        };
        let p = self.plane(&idx, cancel)?;
        if let Some(pal) = &self.palette {
            return Ok(palette_rgba(&p, pal));
        }
        Ok(grey_rgba(&p))
    }
}

/// A reader's refusal: a variant it declines by name is unsupported, else the file is
/// damaged.
fn refused(e: String) -> DecodeError {
    if e.contains("not supported") || e.contains("not a raster") {
        DecodeError::Unsupported(e)
    } else {
        DecodeError::Malformed(e)
    }
}

/// ", big-endian" or ", little-endian" for multi-byte samples.
fn byte_order(l: &specular_gis::rawgrid::RawLayout) -> &'static str {
    match (l.sample.bytes() > 1, l.big_endian) {
        (false, _) => "",
        (true, true) => ", big-endian",
        (true, false) => ", little-endian",
    }
}

/// A file of an Arc/Info grid's folder by its lower-case name (any case on disk).
fn aig_file(dir: &Path, name: &str) -> Option<Vec<u8>> {
    let p = sidecar::find_name(&dir.join(name), name)?;
    sidecar::read_limited(&p, 1 << 32)
}

/// Three 8-bit colour planes as they are, as RGBA (NaN transparent).
pub(crate) fn exact_rgba(planes: &[Arc<NdPlane>]) -> Raster {
    let (w, h) = (planes[0].width, planes[0].height);
    let n = w as usize * h as usize;
    let mut rgba = vec![0u8; n * 4];
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        let mut opaque = true;
        for (c, p) in planes.iter().take(3).enumerate() {
            let v = p.values[i];
            opaque &= v.is_finite();
            px[c] = v.clamp(0.0, 255.0).round() as u8;
        }
        px[3] = if opaque { 255 } else { 0 };
    }
    let truncated = planes.iter().any(|p| p.truncated);
    whole::interleaved(w, h, PixelLayout::Rgba, rgba, truncated)
}

/// A plane's values in the colours of a value table (NaN and values without a colour
/// transparent).
fn palette_rgba(p: &NdPlane, pal: &std::collections::HashMap<i64, [u8; 3]>) -> Raster {
    let mut rgba = vec![0u8; p.values.len() * 4];
    for (px, v) in rgba.chunks_exact_mut(4).zip(&p.values) {
        if v.is_finite()
            && let Some(c) = pal.get(&(*v as i64))
        {
            px.copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    whole::interleaved(p.width, p.height, PixelLayout::Rgba, rgba, p.truncated)
}

/// A plane as zscale grey RGBA, non-finite values transparent.
pub(crate) fn grey_rgba(p: &NdPlane) -> Raster {
    let grey =
        specular_core::nd::to_grey(&p.values, p.width as usize, Limits::ZScale, Stretch::Linear);
    let mut rgba = vec![0u8; grey.len() * 4];
    for ((px, &g), v) in rgba.chunks_exact_mut(4).zip(&grey).zip(&p.values) {
        if v.is_finite() {
            px.copy_from_slice(&[g, g, g, 255]);
        }
    }
    whole::interleaved(p.width, p.height, PixelLayout::Rgba, rgba, p.truncated)
}

impl Session for GeoRasterSession {
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
        px * 8 + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        let sources = match &self.data {
            Data::Vrt(plan) => plan.memory_bytes(),
            _ => 0,
        };
        self.levels.memory_bytes() + self.planes.memory_bytes() + sources
    }
    fn trim_memory(&self) -> usize {
        let sources = match &self.data {
            Data::Vrt(plan) => plan.clear(),
            _ => 0,
        };
        self.levels.clear() + self.planes.clear() + sources
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if self.summary.channel_axis.is_some() {
            "RGB"
        } else if self.summary.axes.is_empty() {
            "Greyscale"
        } else {
            "Multichannel"
        })
    }
    fn geo(&self) -> Option<Arc<GeoRef>> {
        self.geo.clone()
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.plane(index, cancel))
    }
    fn page_count(&self) -> u32 {
        self.pages
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.pages > 1).then_some(PageLabel::Product {
            view: if self.composite.is_some() {
                ProductView::Composite
            } else {
                ProductView::Bands
            },
            metres: None,
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if self.pages < 2 || page >= self.pages {
            return Err(DecodeError::Unsupported("no pages".into()));
        }
        Ok(Arc::new(GeoRasterSession::envi(self.source.clone(), page)?))
    }
}

/// A session with the georeference its sidecar files give ([`crate::geo::sidecar_for`]),
/// or (`paged`) a page of a set the wrapped session belongs to (a STAC item's assets);
/// everything else is the wrapped session's.
pub(crate) struct GeoWrap {
    pub inner: Arc<dyn Session>,
    pub geo: Option<Arc<GeoRef>>,
    pub paged: Option<Paged>,
}

/// The page set a wrapped session is a page of.
pub(crate) struct Paged {
    pub count: u32,
    pub index: u32,
    pub label: Option<PageLabel>,
    pub open: Arc<dyn Fn(u32) -> Result<Arc<dyn Session>> + Send + Sync>,
}

impl Session for GeoWrap {
    fn info(&self) -> &ImageInfo {
        self.inner.info()
    }
    fn source(&self) -> &Arc<Source> {
        self.inner.source()
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        self.inner.decode_preview()
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn prepare_regions(&self, cancel: &CancelToken) -> Result<()> {
        self.inner.prepare_regions(cancel)
    }
    fn regions_ready(&self) -> bool {
        self.inner.regions_ready()
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
    }
    fn open_memory(&self, levels: &[u32]) -> u64 {
        self.inner.open_memory(levels)
    }
    fn memory_bytes(&self) -> usize {
        self.inner.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.inner.trim_memory()
    }
    fn scaled_decode_cheap(&self) -> bool {
        self.inner.scaled_decode_cheap()
    }
    fn want_levels(&self, scales: &[u32]) {
        self.inner.want_levels(scales)
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.inner.animation()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        self.inner.frames(scale)
    }
    fn page_count(&self) -> u32 {
        self.paged
            .as_ref()
            .map_or_else(|| self.inner.page_count(), |p| p.count)
    }
    fn page_index(&self) -> u32 {
        self.paged
            .as_ref()
            .map_or_else(|| self.inner.page_index(), |p| p.index)
    }
    fn page_label(&self) -> Option<PageLabel> {
        match &self.paged {
            Some(p) => p.label.clone(),
            None => self.inner.page_label(),
        }
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        match &self.paged {
            Some(p) => (p.open)(page),
            None => self.inner.open_page(page),
        }
    }
    fn medical(&self) -> Option<&MedicalSummary> {
        self.inner.medical()
    }
    fn modality_frame(&self, cancel: &CancelToken) -> Option<Result<Arc<ModalityFrame>>> {
        self.inner.modality_frame(cancel)
    }
    fn modality_frame_at(
        &self,
        index: u32,
        cancel: &CancelToken,
    ) -> Option<Result<Arc<ModalityFrame>>> {
        self.inner.modality_frame_at(index, cancel)
    }
    fn drawing(&self) -> Option<&crate::drawing::DrawingPage> {
        self.inner.drawing()
    }
    fn geo(&self) -> Option<Arc<GeoRef>> {
        self.geo.clone().or_else(|| self.inner.geo())
    }
    fn research_volume(&self) -> Option<&VolumeFacts> {
        self.inner.research_volume()
    }
    fn encapsulated(&self) -> Option<&EncapsulatedFacts> {
        self.inner.encapsulated()
    }
    fn science(&self) -> Option<&ScienceSummary> {
        self.inner.science()
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        self.inner.nd_plane(index, cancel)
    }
    fn debayer(&self) -> Option<specular_core::nd::Cfa> {
        self.inner.debayer()
    }
    fn can_debayer(&self) -> bool {
        self.inner.can_debayer()
    }
    fn set_debayer(&self, cfa: Option<specular_core::nd::Cfa>) {
        self.inner.set_debayer(cfa)
    }
    fn mesh(&self) -> Option<&MeshModel> {
        self.inner.mesh()
    }
    fn molecule(&self) -> Option<&crate::MoleculeModel> {
        self.inner.molecule()
    }
    fn tracts(&self) -> Option<&crate::TractsModel> {
        self.inner.tracts()
    }
    fn neuro(&self) -> Option<&crate::neuro_dec::NeuroFacts> {
        self.inner.neuro()
    }
    fn points(&self) -> Option<&PointsModel> {
        self.inner.points()
    }
    fn splats(&self) -> Option<&crate::SplatModel> {
        self.inner.splats()
    }
    fn cine_timing(&self) -> Option<&[Duration]> {
        self.inner.cine_timing()
    }
    fn camera(&self) -> Option<&CameraInfo> {
        self.inner.camera()
    }
    fn embedded_icc(&self) -> Option<&[u8]> {
        self.inner.embedded_icc()
    }
    fn embedded_xmp(&self) -> Option<Vec<u8>> {
        self.inner.embedded_xmp()
    }
    fn colour_model(&self) -> Option<&'static str> {
        self.inner.colour_model()
    }
    fn alpha(&self) -> Option<Alpha> {
        self.inner.alpha()
    }
    fn colour_space(&self) -> Option<&'static str> {
        self.inner.colour_space()
    }
    fn chroma_subsampling(&self) -> Option<&'static str> {
        self.inner.chroma_subsampling()
    }
    fn has_gain_map(&self) -> bool {
        self.inner.has_gain_map()
    }
    fn gain_map(&self, cancel: &CancelToken) -> Option<specular_core::GainMap> {
        self.inner.gain_map(cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wavelength_labels() {
        assert_eq!(nanometres(1.25, Some("µm")), Some(1250.0));
        assert_eq!(nanometres(450.0, Some("nm")), Some(450.0));
        assert_eq!(nanometres(3.0, Some("GHz")), None);
        assert_eq!(trimmed(450.5), "450.5");
        assert_eq!(trimmed(2200.0), "2200");
    }

    #[test]
    fn ascii_grid_in_memory() {
        let t = b"ncols 3\nnrows 2\nxllcorner 100\nyllcorner 200\ncellsize 10\nNODATA_value -9999\n1 2 3\n4 -9999 6\n";
        let src = Arc::new(Source::from_vec("grid.asc", t.to_vec()));
        let s = crate::Registry::new().open(src).unwrap();
        assert_eq!(s.info().format, Format::EsriAscii);
        let g = s.geo().unwrap();
        assert_eq!(g.bounds, [100.0, 200.0, 130.0, 220.0]);
        assert!(s.science().unwrap().geo.is_some());
        let c = CancelToken::new();
        let p = s.nd_plane(&[], &c).unwrap().unwrap();
        assert_eq!(p.values[..3], [1.0, 2.0, 3.0]);
        assert!(p.values[4].is_nan());
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(r.layout, PixelLayout::Rgba);
        // No-data is transparent.
        assert_eq!(r.planes[0].data[4 * 4 + 3], 0);
        assert_eq!(r.planes[0].data[3], 255);
        crate::trim_check::assert_trims(s.as_ref(), &DecodeRequest::full(1));
    }

    #[test]
    fn dted_in_memory() {
        let vals: Vec<Option<i16>> = (0..20).map(|i| (i != 6).then_some(i as i16)).collect();
        let d = dted::write(1, (46, 6), (300, 300), 4, 5, &vals);
        let src = Arc::new(Source::from_vec("n46_e006.dt1", d));
        let s = crate::Registry::new().open(src).unwrap();
        assert_eq!(s.info().format, Format::Dted);
        let g = s.geo().unwrap();
        assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(4326));
        assert_eq!(s.science().unwrap().unit.as_deref(), Some("m"));
        let p = s.nd_plane(&[], &CancelToken::new()).unwrap().unwrap();
        assert!(p.values[6].is_nan());
        assert_eq!(p.values[7], 7.0);
    }

    /// A DTED header claiming 9000 × 9000 posts (324 MB of f32) over 20 of them: the band is
    /// refused under a 64 MiB memory budget before it is allocated.
    #[test]
    fn dted_within_memory_budget() {
        let vals: Vec<Option<i16>> = (0..20).map(|i| Some(i as i16)).collect();
        let mut d = dted::write(1, (46, 6), (300, 300), 4, 5, &vals);
        d[47..51].copy_from_slice(b"9000"); // longitude lines
        d[51..55].copy_from_slice(b"9000"); // latitude points
        let src = Arc::new(Source::from_vec("n46_e006.dt1", d));
        let s = crate::Registry::new().open(src).unwrap();
        assert_eq!((s.info().width, s.info().height), (9000, 9000));
        let r = specular_core::budget::with_limit(64 << 20, || {
            s.nd_plane(&[], &CancelToken::new()).unwrap()
        });
        let Err(DecodeError::NeedsMemory(n)) = r else {
            panic!("{:?}", r.map(|_| ()))
        };
        assert!(n.needed >= 324_000_000 && n.needed > n.free, "{n:?}");
    }
}
