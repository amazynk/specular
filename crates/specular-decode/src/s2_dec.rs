//! Sentinel-2 products as one image: a `.SAFE` folder, its `MTD_MSIL1C.xml` /
//! `MTD_MSIL2A.xml`, or a ZIP holding the SAFE folder.
//!
//! Pages are views at a resolution, coarse first (60 m, then 20 m, then 10 m): true colour
//! (B04, B03, B02), false colour (B08, B04, B03), short-wave infrared (B12, B8A, B04),
//! agriculture (B11, B08, B02), all bands one at a time, the scene classification (L2A SCL;
//! L1C: the MSK_CLASSI cloud and snow mask) and the cloud probability (L2A). Values are
//! reflectance, `(DN + offset) / quantification`, with DN 0 (no data) left out. Each band
//! comes from its file at the page's resolution when the product has one (L2A), else from
//! the nearest finer file by block means, else the coarser one repeated
//! ([`specular_gis::sentinel2::resample`]). The tile's CRS and grid georeference every page.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_gis::sentinel2::{self as s2, BANDS, Level, RESOLUTIONS};

use crate::geo::{GeoRef, RasterGeo};
use crate::science::{
    self, AxisKind, ColourTable, NdAxis, NdPlane, PlaneCache, ScienceKind, ScienceSummary,
};
use crate::whole::Levels;
use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, ProductView, Result, Session,
};

/// Largest metadata file read.
const MAX_XML: u64 = 64 << 20;
/// Largest band file read from a ZIP.
const MAX_MEMBER: u64 = 1 << 30;

pub struct Sentinel2Decoder;

/// Where the product's files are.
enum Files {
    Folder(PathBuf),
    /// The ZIP and the SAFE folder's path inside it (ending in `/`, or empty).
    Zip {
        source: Arc<Source>,
        zip: specular_ora::zip::Zip,
        prefix: String,
    },
}

impl Files {
    fn open(&self, rel: &str) -> Result<Arc<Source>> {
        match self {
            Files::Folder(root) => {
                let p = root.join(rel);
                Ok(Arc::new(Source::open(&p).map_err(|_| missing(rel))?))
            }
            Files::Zip {
                source,
                zip,
                prefix,
            } => {
                let name = format!("{prefix}{rel}");
                let e = zip.find(&name).ok_or_else(|| missing(rel))?;
                let bytes = zip
                    .read(source.bytes(), e, MAX_MEMBER)
                    .map_err(|e| DecodeError::Malformed(e.to_string()))?;
                Ok(Arc::new(
                    Source::from_vec(&name, bytes.into_owned()).within(source),
                ))
            }
        }
    }

    fn text(&self, rel: &str) -> Result<String> {
        let s = self.open(rel)?;
        if s.len() as u64 > MAX_XML {
            return Err(DecodeError::TooLarge);
        }
        Ok(String::from_utf8_lossy(s.bytes()).into_owned())
    }
}

fn missing(rel: &str) -> DecodeError {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    DecodeError::Unsupported(format!("the product file {name} is missing"))
}

/// What a page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layer {
    /// A band of [`BANDS`].
    Band(usize),
    /// L2A scene classification.
    Scl,
    /// L2A cloud probability, percent.
    CloudProbability,
    /// L1C MSK_CLASSI: 1 opaque cloud, 2 cirrus, 3 snow.
    Classi,
}

/// One product, shared by its pages.
struct Product {
    files: Files,
    meta: s2::Product,
    tile: s2::Tile,
    name: String,
    /// The pages: a view and its resolution.
    pages: Vec<(ProductView, u32)>,
    /// The product's own small true-colour preview (`QI_DATA/*_PVI.jp2`), decoded once:
    /// thumbnails and the first picture of true-colour pages.
    preview: std::sync::OnceLock<Option<Arc<Raster>>>,
}

/// The bands of a composite view, red first.
fn composite(view: ProductView) -> Option<[&'static str; 3]> {
    Some(match view {
        ProductView::TrueColour => ["B04", "B03", "B02"],
        ProductView::FalseColour => ["B08", "B04", "B03"],
        ProductView::Swir => ["B12", "B8A", "B04"],
        ProductView::Agriculture => ["B11", "B08", "B02"],
        _ => return None,
    })
}

impl Product {
    fn open(source: &Arc<Source>) -> Result<Product> {
        let (files, name) = if source.is_folder() {
            let root = source.path().to_path_buf();
            (Files::Folder(root.clone()), product_name(&root))
        } else if source.bytes().starts_with(b"PK\x03\x04") {
            let zip = specular_ora::zip::Zip::parse(source.bytes())
                .map_err(|e| DecodeError::Malformed(e.to_string()))?;
            let prefix = zip_prefix(&zip).ok_or(DecodeError::UnsupportedFormat)?;
            let name = prefix
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .map(str::to_owned)
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| product_name(source.path()));
            (
                Files::Zip {
                    source: source.clone(),
                    zip,
                    prefix,
                },
                name,
            )
        } else {
            // The product metadata itself: its folder is the product.
            let root = source
                .path()
                .parent()
                .ok_or(DecodeError::UnsupportedFormat)?
                .to_path_buf();
            (Files::Folder(root.clone()), product_name(&root))
        };
        let mtd = ["MTD_MSIL2A.xml", "MTD_MSIL1C.xml"]
            .iter()
            .find_map(|n| files.text(n).ok())
            .ok_or_else(|| missing("MTD_MSIL2A.xml"))?;
        let meta = s2::parse_product(&mtd).map_err(DecodeError::Malformed)?;
        let granule = meta
            .granule()
            .ok_or_else(|| DecodeError::Malformed("no granule".into()))?;
        let tile = s2::parse_tile(&files.text(&format!("{granule}/MTD_TL.xml"))?)
            .map_err(DecodeError::Malformed)?;
        let mut p = Product {
            files,
            meta,
            tile,
            name,
            pages: Vec::new(),
            preview: std::sync::OnceLock::new(),
        };
        let mut views = vec![
            ProductView::TrueColour,
            ProductView::FalseColour,
            ProductView::Swir,
            ProductView::Agriculture,
            ProductView::Bands,
        ];
        let has = |rel: &str| match &p.files {
            Files::Folder(root) => root.join(rel).is_file(),
            Files::Zip { zip, prefix, .. } => zip.find(&format!("{prefix}{rel}")).is_some(),
        };
        if p.meta.level == Level::L2A {
            if p.meta.band_file("SCL", 20).is_some() {
                views.push(ProductView::SceneClass);
            }
            if has(&p.cloud_file(20)) || has(&p.cloud_file(60)) {
                views.push(ProductView::CloudProbability);
            }
        } else if has(&p.classi_file()) {
            views.push(ProductView::SceneClass);
        }
        p.pages = RESOLUTIONS
            .iter()
            .flat_map(|&r| views.iter().map(move |&v| (v, r)))
            .collect();
        Ok(p)
    }

    fn granule(&self) -> &str {
        self.meta.granule().unwrap_or("")
    }

    /// The product's true-colour preview image, when it has one that decodes.
    fn pvi(&self) -> Option<Arc<Raster>> {
        let dir = format!("{}/QI_DATA/", self.granule());
        let name = match &self.files {
            Files::Folder(root) => std::fs::read_dir(root.join(&dir))
                .ok()?
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .find(|n| n.ends_with("_PVI.jp2"))
                .map(|n| format!("{dir}{n}"))?,
            Files::Zip { zip, prefix, .. } => zip
                .entries
                .iter()
                .filter_map(|e| e.name.strip_prefix(prefix.as_str()))
                .find(|n| n.starts_with(&dir) && n.ends_with("_PVI.jp2"))?
                .to_owned(),
        };
        let src = self.files.open(&name).ok()?;
        let s = crate::Registry::default().open(src).ok()?;
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .ok()?;
        Some(Arc::new(r))
    }

    fn cloud_file(&self, r: u32) -> String {
        format!("{}/QI_DATA/MSK_CLDPRB_{r}m.jp2", self.granule())
    }

    fn classi_file(&self) -> String {
        format!("{}/QI_DATA/MSK_CLASSI_B00.jp2", self.granule())
    }

    /// The layers a view's planes come from.
    fn layers(&self, view: ProductView) -> Vec<Layer> {
        let band = |n: &str| Layer::Band(s2::Product::band_index(n).unwrap_or(0));
        if let Some(c) = composite(view) {
            return c.iter().map(|n| band(n)).collect();
        }
        match view {
            ProductView::SceneClass if self.meta.level == Level::L2A => vec![Layer::Scl],
            ProductView::SceneClass => vec![Layer::Classi],
            ProductView::CloudProbability => vec![Layer::CloudProbability],
            _ => (0..BANDS.len())
                .filter(|&k| self.meta.band_file(BANDS[k].0, 60).is_some())
                .map(Layer::Band)
                .collect(),
        }
    }

    /// A layer's values on the grid of resolution `r`.
    fn layer(&self, layer: Layer, r: u32) -> Result<Vec<f32>> {
        let size = self
            .tile
            .size(r)
            .ok_or_else(|| DecodeError::Malformed("no grid".into()))?;
        let (rel, fr, categorical) = match layer {
            Layer::Band(k) => {
                let (f, fr) = self
                    .meta
                    .band_file(BANDS[k].0, r)
                    .ok_or_else(|| missing(BANDS[k].0))?;
                (format!("{f}.jp2"), fr, false)
            }
            Layer::Scl => {
                let (f, fr) = self
                    .meta
                    .band_file("SCL", r)
                    .ok_or_else(|| missing("SCL"))?;
                (format!("{f}.jp2"), fr, true)
            }
            Layer::CloudProbability => {
                let fr = if r == 60 && self.files.open(&self.cloud_file(60)).is_ok() {
                    60
                } else {
                    20
                };
                (self.cloud_file(fr), fr, false)
            }
            Layer::Classi => (self.classi_file(), 60, true),
        };
        let src = self.files.open(&rel)?;
        let comps = native_samples(src.bytes()).map_err(|e| match e {
            specular_jp2::Jp2Error::TooLarge(_) => DecodeError::TooLarge,
            e => DecodeError::Malformed(format!("{rel}: {e}")),
        })?;
        let first = comps
            .first()
            .ok_or_else(|| DecodeError::Malformed("no samples".into()))?;
        let fsize = (first.width, first.height);
        let values: Vec<f32> = match layer {
            Layer::Band(k) => {
                let (o, q) = (self.meta.offsets[k], self.meta.quantification);
                first
                    .samples
                    .iter()
                    .map(|&d| {
                        if d == 0 {
                            f32::NAN
                        } else {
                            ((f64::from(d) + o) / q) as f32
                        }
                    })
                    .collect()
            }
            Layer::Scl => first
                .samples
                .iter()
                .map(|&d| if d == 0 { f32::NAN } else { f32::from(d) })
                .collect(),
            Layer::CloudProbability => first.samples.iter().map(|&d| f32::from(d)).collect(),
            Layer::Classi => (0..first.samples.len())
                .map(|i| {
                    let set = |c: usize| comps.get(c).is_some_and(|c| c.samples[i] != 0);
                    if set(0) {
                        1.0
                    } else if set(1) {
                        2.0
                    } else if set(2) {
                        3.0
                    } else {
                        0.0
                    }
                })
                .collect(),
        };
        if categorical && fr < r {
            // Classes are picked, not averaged: each block's first sample.
            let k = (r / fr) as usize;
            let (w, h) = (size.0 as usize, size.1 as usize);
            let sw = fsize.0 as usize;
            let mut out = vec![f32::NAN; w * h];
            for y in 0..h {
                for x in 0..w {
                    if let Some(&v) = values.get(y * k * sw + x * k)
                        && x * k < sw
                    {
                        out[y * w + x] = v;
                    }
                }
            }
            return Ok(out);
        }
        Ok(s2::resample(&values, fsize, fr, r, size))
    }
}

/// A JPEG 2000 file's samples as stored, per component. A single grey component of up to
/// 16 bits is decoded on the thread pool at 16 bits and mapped back to its own precision
/// (the 16-bit scaling `v·65535/(2^p−1)`, rounded, is one to one); others through the
/// component decode.
fn native_samples(bytes: &[u8]) -> specular_jp2::Result<Vec<specular_jp2::RawComponent>> {
    let dec = specular_jp2::Decoder::new(bytes)?;
    let info = dec.info();
    let single = info.components.len() == 1
        && info.gray
        && !info.has_palette
        && !info.components[0].signed
        && (1..=16).contains(&info.components[0].precision);
    if !single {
        return dec.decode_components(0, None);
    }
    let vmax = (1u64 << info.components[0].precision) - 1;
    let img = dec.decode(&specular_jp2::DecodeOptions {
        depth: specular_jp2::Depth::Sixteen,
        ..Default::default()
    })?;
    if img.truncated {
        return Err(specular_jp2::Jp2Error::Malformed(
            "truncated or corrupt tile data",
        ));
    }
    let specular_jp2::Pixels::U16(px) = img.pixels else {
        return dec.decode_components(0, None);
    };
    let samples = px
        .into_iter()
        .map(|o| ((u64::from(o) * vmax + 32767) / 65535) as u16)
        .collect();
    Ok(vec![specular_jp2::RawComponent {
        width: img.width,
        height: img.height,
        samples,
    }])
}

/// The product's name from its folder ("S2A_MSIL2A_…").
fn product_name(p: &Path) -> String {
    p.file_name()
        .and_then(|n| n.to_str())
        .map(|n| {
            n.trim_end_matches(".SAFE")
                .trim_end_matches(".zip")
                .to_owned()
        })
        .unwrap_or_default()
}

/// The SAFE folder inside a ZIP: the path before its product metadata.
fn zip_prefix(zip: &specular_ora::zip::Zip) -> Option<String> {
    zip.entries.iter().find_map(|e| {
        let n = &e.name;
        ["MTD_MSIL2A.xml", "MTD_MSIL1C.xml"].iter().find_map(|m| {
            let p = n.strip_suffix(m)?;
            (p.is_empty() || p.ends_with('/') && p.matches('/').count() == 1).then(|| p.to_owned())
        })
    })
}

/// Whether the ZIP at `path` holds a Sentinel-2 product (opened as one image rather than
/// browsed).
pub fn is_sentinel2_archive(path: &Path) -> bool {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
    {
        return false;
    }
    Source::open(path).is_ok_and(|s| {
        s.bytes().starts_with(b"PK\x03\x04")
            && specular_ora::zip::Zip::parse(s.bytes()).is_ok_and(|z| zip_prefix(&z).is_some())
    })
}

/// ESA's colours of the scene classes 0–11.
fn scl_lut() -> Arc<ColourTable> {
    const C: [[u8; 3]; 12] = [
        [0, 0, 0],       // no data
        [255, 0, 0],     // saturated or defective
        [47, 47, 47],    // dark area
        [100, 50, 0],    // cloud shadow
        [0, 160, 0],     // vegetation
        [255, 230, 90],  // not vegetated
        [0, 0, 255],     // water
        [128, 128, 128], // unclassified
        [192, 192, 192], // cloud, medium probability
        [255, 255, 255], // cloud, high probability
        [100, 200, 255], // thin cirrus
        [255, 150, 255], // snow or ice
    ];
    let mut t = [[0u8; 3]; 256];
    // The plane's display range is 0–11 mapped over the table's 256 entries.
    for (i, e) in t.iter_mut().enumerate() {
        *e = C[(i * 12 / 256).min(11)];
    }
    Arc::new(t)
}

/// Colours of the L1C mask classes 0–3: clear, opaque cloud, cirrus, snow.
fn classi_lut() -> Arc<ColourTable> {
    const C: [[u8; 3]; 4] = [[0, 0, 0], [255, 255, 255], [100, 200, 255], [255, 150, 255]];
    let mut t = [[0u8; 3]; 256];
    for (i, e) in t.iter_mut().enumerate() {
        *e = C[(i * 4 / 256).min(3)];
    }
    Arc::new(t)
}

struct S2Session {
    source: Arc<Source>,
    product: Arc<Product>,
    page: u32,
    view: ProductView,
    res: u32,
    layers: Vec<Layer>,
    info: ImageInfo,
    summary: ScienceSummary,
    geo: Option<Arc<GeoRef>>,
    planes: PlaneCache,
    levels: Levels,
}

impl S2Session {
    fn new(source: Arc<Source>, product: Arc<Product>, page: u32) -> Result<S2Session> {
        let &(view, res) = product
            .pages
            .get(page as usize)
            .ok_or_else(|| DecodeError::Unsupported("no such page".into()))?;
        let (w, h) = product
            .tile
            .size(res)
            .ok_or_else(|| DecodeError::Malformed("no grid".into()))?;
        let layers = product.layers(view);
        let m = &product.meta;
        let level = match m.level {
            Level::L1C => "Level-1C",
            Level::L2A => "Level-2A",
        };
        let mut keys: Vec<(String, String)> = vec![
            ("PRODUCT_URI".into(), product.name.clone()),
            ("SPACECRAFT_NAME".into(), m.spacecraft.clone()),
            ("PROCESSING_LEVEL".into(), level.into()),
            ("PRODUCT_TYPE".into(), m.product_type.clone()),
            ("PROCESSING_BASELINE".into(), m.baseline.clone()),
            ("PRODUCT_START_TIME".into(), m.start_time.clone()),
            ("TILE_ID".into(), product.tile.tile_id.clone()),
            ("SENSING_TIME".into(), product.tile.sensing_time.clone()),
            ("HORIZONTAL_CS_NAME".into(), product.tile.cs_name.clone()),
            (
                "QUANTIFICATION_VALUE".into(),
                format!("{}", m.quantification),
            ),
        ];
        if let Some(c) = m.cloud_cover {
            keys.push(("Cloud_Coverage_Assessment".into(), format!("{c:.2}")));
        }
        keys.retain(|(_, v)| !v.is_empty());
        let crs = product.tile.epsg.and_then(specular_gis::crs::from_epsg);
        let geo = product.tile.transform(res).map(|t| {
            Arc::new(crate::geo::raster(
                t,
                crs.clone(),
                crs.is_none().then(|| product.tile.cs_name.clone()),
                (w, h),
                RasterGeo {
                    levels: vec![(w, h)],
                    keys: keys.clone(),
                    keys_kind: "header",
                    values: true,
                    ..Default::default()
                },
            ))
        });
        let band_label = |k: usize| format!("{} · {} nm", BANDS[k].0, BANDS[k].2);
        let mut summary = ScienceSummary {
            width: w,
            height: h,
            sample: "JPEG 2000, 15-bit".into(),
            title: Some(product.name.clone()),
            kind: ScienceKind::Gridded,
            geo: geo.clone(),
            keywords: Arc::new(
                keys.iter()
                    .map(|(k, v)| (k.clone(), v.clone(), String::new()))
                    .collect(),
            ),
            date_obs: Some(m.start_time.clone()).filter(|t| !t.is_empty()),
            instrument: Some("MSI".into()),
            telescope: Some(m.spacecraft.clone()).filter(|t| !t.is_empty()),
            ..Default::default()
        };
        match view {
            ProductView::SceneClass => {
                summary.luts = vec![if m.level == Level::L2A {
                    scl_lut()
                } else {
                    classi_lut()
                }];
                summary.display_ranges = vec![if m.level == Level::L2A {
                    (0.0, 11.0)
                } else {
                    (0.0, 3.0)
                }];
                summary.classes = Some(if m.level == Level::L2A {
                    crate::science::ClassScheme::Sentinel2Scl
                } else {
                    crate::science::ClassScheme::Sentinel2L1cMask
                });
            }
            ProductView::CloudProbability => {
                summary.unit = Some("%".into());
                summary.display_ranges = vec![(0.0, 100.0)];
            }
            _ => {
                summary.axes.push(NdAxis {
                    name: "Band".into(),
                    len: layers.len() as u32,
                    unit: String::new(),
                    labels: layers
                        .iter()
                        .map(|l| match l {
                            Layer::Band(k) => band_label(*k),
                            _ => String::new(),
                        })
                        .collect(),
                    wcs_axis: None,
                    linear: None,
                    kind: AxisKind::Channel,
                });
                if composite(view).is_some() {
                    summary.channel_axis = Some(0);
                }
            }
        }
        let info = ImageInfo {
            format: Format::Sentinel2,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: true,
            bit_depth: 16,
            progressive: false,
            coding: format!("{} {level}, JPEG 2000", m.spacecraft),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(S2Session {
            source,
            product,
            page,
            view,
            res,
            layers,
            info,
            summary,
            geo,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        let k = index.first().copied().unwrap_or(0) as usize;
        let layer = *self
            .layers
            .get(k)
            .ok_or_else(|| DecodeError::Malformed("no such band".into()))?;
        let (w, h) = (self.info.width, self.info.height);
        self.planes.get_or(index, cancel, || {
            let _res = specular_core::budget::reserve_n(u64::from(w) * u64::from(h), 8)?;
            let values = self.product.layer(layer, self.res)?;
            Ok(NdPlane {
                width: w,
                height: h,
                values,
                truncated: false,
            })
        })
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        if self.summary.channel_axis.is_some() {
            let planes = (0..3)
                .map(|b| self.plane(&[b], cancel))
                .collect::<Result<Vec<_>>>()?;
            return Ok(science::auto_raster(&planes, None));
        }
        let p = self.plane(&[0], cancel)?;
        Ok(crate::georaster_dec::grey_rgba(&p))
    }
}

impl Session for S2Session {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        if self.view != ProductView::TrueColour {
            return None;
        }
        let p = self.product.preview.get_or_init(|| self.product.pvi());
        p.as_ref().map(|r| Ok((**r).clone()))
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
        let (w, h) = (self.info.width, self.info.height);
        u64::from(w) * u64::from(h) * 3 * 8 + crate::whole::level_bytes(w, h, req.scale, 4)
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
        Some(if self.summary.channel_axis.is_some() {
            "RGB"
        } else if self.summary.axes.is_empty() {
            "Greyscale"
        } else {
            "Multichannel"
        })
    }
    fn page_count(&self) -> u32 {
        self.product.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        Some(PageLabel::Product {
            view: self.view,
            metres: Some(self.res),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(S2Session::new(
            self.source.clone(),
            self.product.clone(),
            page,
        )?))
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
}

impl Decoder for Sentinel2Decoder {
    fn format(&self) -> Format {
        Format::Sentinel2
    }
    fn probe(&self, head: &[u8]) -> bool {
        s2::sniff_product(head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        if data.starts_with(b"PK\x03\x04") {
            return specular_ora::zip::Zip::parse(data).is_ok_and(|z| zip_prefix(&z).is_some());
        }
        self.probe(&data[..data.len().min(crate::PROBE_TEXT)])
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let product = Arc::new(Product::open(&source)?);
        Ok(Arc::new(S2Session::new(source, product, 0)?))
    }
}
