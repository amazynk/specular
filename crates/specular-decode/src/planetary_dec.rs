//! Planetary science images through the science path, read by `specular-planetary`: VICAR
//! files (`.vic`, `.img` starting with `LBLSIZE=`), PDS3 products (an attached label in the
//! data file, or a detached `.lbl` beside it) and PDS4 products (an XML label beside the
//! data). Each image object or array of a product is a page; bands are a channel axis, three
//! of them a colour composite. Values come out scaled, with the label's missing and invalid
//! constants as no-data (transparent); the observation and map projection facts go to the
//! info panel.
//!
//! A raw data file opened directly is read through the label beside it that names it
//! ([`label_beside`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_gis::sidecar;
use specular_planetary::{self as planetary, Layout, pds3, pds4, vicar};

use crate::science::{self, AxisKind, NdAxis, NdPlane, PlaneCache, ScienceKind, ScienceSummary};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

/// Pixels per band at most.
const MAX_PIXELS: u64 = 1 << 30;
/// Label keywords listed at most.
const MAX_KEYWORDS: usize = 8192;

/// Which reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Vicar,
    Pds3,
    Pds4,
}

pub struct PlanetaryDecoder(pub Kind);

impl Decoder for PlanetaryDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Vicar => Format::Vicar,
            Kind::Pds3 => Format::Pds3,
            Kind::Pds4 => Format::Pds4,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::Vicar => vicar::sniff(head),
            Kind::Pds3 => pds3::sniff(head),
            Kind::Pds4 => pds4::sniff(head),
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            Kind::Vicar => vicar::sniff(data),
            Kind::Pds3 => pds3::sniff(&data[..data.len().min(pds3::SNIFF)]),
            Kind::Pds4 => pds4::sniff(&data[..data.len().min(pds4::SNIFF)]),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Vicar => &["vic", "vicar"],
            // Detached labels; attached ones (`.img`) and PDS4's `.xml` go by content.
            Kind::Pds3 => &["lbl"],
            Kind::Pds4 => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let file = match self.0 {
            Kind::Vicar => open_vicar(source)?,
            Kind::Pds3 => open_pds3(source)?,
            Kind::Pds4 => open_pds4(source)?,
        };
        Ok(Arc::new(PlanetarySession::new(Arc::new(file), 0)?))
    }
}

fn mapped(e: planetary::Error) -> DecodeError {
    match e {
        planetary::Error::NotThisFormat => DecodeError::UnsupportedFormat,
        planetary::Error::Malformed(m) => DecodeError::Malformed(m),
        planetary::Error::Unsupported(m) => DecodeError::Unsupported(m),
        planetary::Error::TooLarge => DecodeError::TooLarge,
    }
}

/// One image of a file.
struct Page {
    /// The object or array it is ("IMAGE", "Array_2D_Image").
    name: String,
    data: Arc<Source>,
    layout: Layout,
    unit: Option<String>,
    /// Labels per band (wavelengths or names), and the band axis's name and unit.
    band_labels: Vec<String>,
    band_axis: String,
    band_unit: String,
}

/// An opened file: its pages and what its label says.
struct File {
    format: Format,
    source: Arc<Source>,
    pages: Vec<Page>,
    facts: Arc<planetary::Facts>,
    keywords: Arc<Vec<(String, String, String)>>,
}

fn keywords(kv: Vec<(String, String)>) -> Arc<Vec<(String, String, String)>> {
    Arc::new(
        kv.into_iter()
            .take(MAX_KEYWORDS)
            .map(|(k, v)| (k, v, String::new()))
            .collect(),
    )
}

/// A file named in a label, looked for beside the label (case-insensitively; only its base
/// name counts).
fn beside(label: &Source, name: &str) -> Result<Arc<Source>> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let found = crate::geo::on_disk(label)
        .then(|| sidecar::find_name(label.path(), base))
        .flatten()
        .ok_or_else(|| DecodeError::Unsupported(format!("the data file {base} is missing")))?;
    Ok(Arc::new(Source::open(&found)?))
}

/// Why a data file named in a label cannot be read raw, by its name.
fn encoded_data(name: &str) -> Option<String> {
    let ext = name.rsplit('.').next()?.to_ascii_lowercase();
    let what = match ext.as_str() {
        "jp2" | "j2k" | "jpx" => "JPEG 2000",
        "zip" => "ZIP",
        "gz" => "gzip",
        "tif" | "tiff" => "TIFF",
        "png" => "PNG",
        "jpg" | "jpeg" => "JPEG",
        _ => return None,
    };
    Some(format!(
        "the image is stored as a {what} file ({name}); open that file instead"
    ))
}

fn open_vicar(source: Arc<Source>) -> Result<File> {
    let mut img = vicar::open(source.bytes(), 0).map_err(mapped)?;
    let facts = vicar::facts(&img.label);
    let page = Page {
        name: "IMAGE".into(),
        data: match img.decoded.take() {
            // BASIC-compressed: the decompressed samples.
            Some(d) => Arc::new(Source::from_vec(source.path(), d)),
            None => source.clone(),
        },
        layout: img.layout,
        unit: None,
        band_labels: Vec::new(),
        band_axis: "Band".into(),
        band_unit: String::new(),
    };
    Ok(File {
        format: Format::Vicar,
        keywords: keywords(img.label.keywords(MAX_KEYWORDS)),
        source,
        pages: vec![page],
        facts: Arc::new(facts),
    })
}

/// "450.5 nm" with at most three decimals.
fn trimmed(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn wavelength_labels(w: &[f64], unit: Option<&str>) -> (Vec<String>, String) {
    let unit = unit
        .map(|u| match u.to_ascii_uppercase().as_str() {
            "MICROMETER" | "MICROMETERS" | "MICRON" | "MICRONS" | "UM" => "µm".to_owned(),
            "NANOMETER" | "NANOMETERS" | "NM" => "nm".to_owned(),
            _ => u.to_owned(),
        })
        .unwrap_or_default();
    let labels = w
        .iter()
        .map(|v| format!("{} {unit}", trimmed(*v)).trim().to_owned())
        .collect();
    (labels, unit)
}

fn open_pds3(source: Arc<Source>) -> Result<File> {
    let p = pds3::parse(source.bytes()).map_err(mapped)?;
    let mut pages = Vec::new();
    let mut reasons = p
        .refused
        .iter()
        .map(|r| format!("{}: {}", r.name, r.reason))
        .collect::<Vec<_>>();
    for img in p.images.clone() {
        let data = match &img.pointer.file {
            None => Ok(source.clone()),
            Some(name) => match encoded_data(name) {
                Some(why) => Err(DecodeError::Unsupported(why)),
                // Only a compressed copy is there (HiRISE: JPEG 2000).
                None => beside(&source, name).map_err(|e| match p.compressed.first() {
                    Some((file, enc)) => DecodeError::Unsupported(format!(
                        "the image is stored compressed ({}) as {file}; open that file \
                         instead",
                        if enc.is_empty() {
                            "unknown encoding"
                        } else {
                            enc
                        }
                    )),
                    None => e,
                }),
            },
        };
        let data = match data {
            Ok(d) => d,
            Err(DecodeError::Unsupported(why)) => {
                reasons.push(why);
                continue;
            }
            Err(e) => return Err(e),
        };
        let (band_labels, band_unit, band_axis) = if !img.wavelengths.is_empty() {
            let (l, u) = wavelength_labels(&img.wavelengths, img.wavelength_unit.as_deref());
            (l, u, "Wavelength")
        } else {
            (img.band_names.clone(), String::new(), "Band")
        };
        pages.push(Page {
            name: img.name,
            data,
            layout: img.layout,
            unit: img.unit,
            band_labels,
            band_axis: band_axis.into(),
            band_unit,
        });
    }
    if pages.is_empty() {
        return Err(DecodeError::Unsupported(match reasons.first() {
            Some(r) => format!("the PDS3 image cannot be shown ({r})"),
            None => "the PDS3 label describes no image".into(),
        }));
    }
    let mut kv = Vec::new();
    p.label.root.flatten(&mut kv, "", MAX_KEYWORDS);
    let mut facts = p.facts;
    // The embedded VICAR label: its items are listed too, and it fills facts the PDS3 label
    // lacks.
    if let Some(ptr) = &p.vicar_header {
        let bytes = match &ptr.file {
            None => Some(source.clone()),
            Some(name) => beside(&source, name).ok(),
        };
        if let Some(b) = bytes
            && let Some(at) = usize::try_from(ptr.offset).ok()
            && let Some(slice) = b.bytes().get(at..)
            && let Ok(l) = vicar::parse_label(slice)
        {
            facts = facts.or(vicar::facts(&l));
            kv.extend(
                l.keywords(MAX_KEYWORDS)
                    .into_iter()
                    .map(|(k, v)| (format!("VICAR.{k}"), v)),
            );
        }
    }
    Ok(File {
        format: Format::Pds3,
        source,
        pages,
        facts: Arc::new(facts),
        keywords: keywords(kv),
    })
}

fn open_pds4(source: Arc<Source>) -> Result<File> {
    let p = pds4::parse(source.bytes()).map_err(mapped)?;
    let mut pages = Vec::new();
    let mut reasons: Vec<String> = p
        .refused
        .iter()
        .map(|r| format!("{}: {}", r.class, r.reason))
        .collect();
    let mut files: Vec<(String, Arc<Source>)> = Vec::new();
    for img in p.images {
        let data = match files.iter().find(|(n, _)| *n == img.file) {
            Some((_, s)) => Ok(s.clone()),
            None => match encoded_data(&img.file) {
                Some(why) => Err(DecodeError::Unsupported(why)),
                None => beside(&source, &img.file),
            },
        };
        let data = match data {
            Ok(d) => d,
            Err(DecodeError::Unsupported(why)) => {
                reasons.push(why);
                continue;
            }
            Err(e) => return Err(e),
        };
        files.push((img.file.clone(), data.clone()));
        pages.push(Page {
            name: match &img.name {
                Some(n) => format!("{} {n}", img.class),
                None => img.class.clone(),
            },
            data,
            layout: img.layout,
            unit: img.unit,
            band_labels: Vec::new(),
            band_axis: img.band_axis.unwrap_or_else(|| "Band".into()),
            band_unit: String::new(),
        });
    }
    if pages.is_empty() {
        return Err(DecodeError::Unsupported(match reasons.first() {
            Some(r) => format!("the PDS4 product cannot be shown ({r})"),
            None => "the PDS4 label describes no image array".into(),
        }));
    }
    Ok(File {
        format: Format::Pds4,
        source,
        pages,
        facts: Arc::new(p.facts),
        keywords: keywords(p.keywords),
    })
}

/// The label beside raw data `path` that names it: a PDS3 `.lbl` or a PDS4 `.xml` with the
/// same stem, read when `path` itself is not recognised.
pub fn label_beside(path: &Path) -> Option<(Kind, PathBuf)> {
    let name = path.file_name()?.to_str()?;
    for ext in ["lbl", "xml"] {
        let Some(l) = sidecar::find(path, ext).filter(|l| l != path) else {
            continue;
        };
        let Some(b) = sidecar::read_limited(&l, 16 << 20) else {
            continue;
        };
        let text = String::from_utf8_lossy(&b);
        let names = |t: &str| t.to_ascii_lowercase().contains(&name.to_ascii_lowercase());
        let b = b.as_slice();
        if ext == "lbl" && pds3::sniff(b) && names(&text) {
            return Some((Kind::Pds3, l));
        }
        if ext == "xml" && pds4::sniff_observational(b) && names(&text) {
            return Some((Kind::Pds4, l));
        }
    }
    None
}

/// Opens raw data through the label beside it ([`label_beside`]).
pub(crate) fn open_through_label(kind: Kind, label: &Path) -> Result<Arc<dyn Session>> {
    PlanetaryDecoder(kind).open(Arc::new(Source::open(label)?))
}

struct PlanetarySession {
    file: Arc<File>,
    page: u32,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

impl PlanetarySession {
    fn new(file: Arc<File>, page: u32) -> Result<PlanetarySession> {
        let p = file
            .pages
            .get(page as usize)
            .ok_or_else(|| DecodeError::Unsupported("no such page".into()))?;
        let l = &p.layout;
        if u64::from(l.width) * u64::from(l.height) > MAX_PIXELS {
            return Err(DecodeError::TooLarge);
        }
        if l.bands > 100_000 {
            return Err(DecodeError::Malformed("band count".into()));
        }
        let mut sample = format!("{}, {}", l.sample.name(), l.interleave.name());
        if l.bands == 1 {
            sample = l.sample.name();
        }
        if l.scaled() {
            sample.push_str(", scaled");
        }
        let f = &file.facts;
        let title = f
            .title
            .clone()
            .or_else(|| f.product_id.clone())
            .or_else(|| {
                file.source
                    .path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_owned)
            });
        let mut summary = ScienceSummary {
            width: l.width,
            height: l.height,
            sample: sample.clone(),
            title,
            kind: ScienceKind::Astronomy,
            unit: p.unit.clone(),
            keywords: file.keywords.clone(),
            planetary: Some(file.facts.clone()),
            hdu: (file.pages.len() > 1).then(|| p.name.clone()),
            ..Default::default()
        };
        if l.bands > 1 {
            summary.axes.push(NdAxis {
                name: p.band_axis.clone(),
                len: l.bands,
                unit: p.band_unit.clone(),
                labels: if p.band_labels.len() == l.bands as usize {
                    p.band_labels.clone()
                } else {
                    Vec::new()
                },
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Channel,
            });
            // Three bands without wavelengths are a colour image (rover RGB products).
            if l.bands == 3 && p.band_unit.is_empty() {
                summary.channel_axis = Some(0);
            }
        }
        let alpha = !l.nodata.is_empty() || l.sample.is_float();
        let info = ImageInfo {
            format: file.format,
            width: l.width,
            height: l.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: alpha,
            bit_depth: l.sample.bits(),
            progressive: false,
            coding: sample,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(PlanetarySession {
            file,
            page,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn page(&self) -> &Page {
        &self.file.pages[self.page as usize]
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        let p = self.page();
        let band = index.first().copied().unwrap_or(0);
        if band >= p.layout.bands {
            return Err(DecodeError::Malformed("no such band".into()));
        }
        let (w, h) = (self.info.width, self.info.height);
        self.planes.get_or(index, cancel, || {
            let _res = specular_core::budget::reserve_n(u64::from(w) * u64::from(h), 4)?;
            let (values, truncated) = p.layout.band(p.data.bytes(), band);
            Ok(NdPlane {
                width: w,
                height: h,
                values,
                truncated,
            })
        })
    }

    /// The colour composite, or the shown band as zscale grey with no-data transparent.
    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        if self.summary.channel_axis.is_some() {
            let planes = (0..3)
                .map(|b| self.plane(&[b], cancel))
                .collect::<Result<Vec<_>>>()?;
            return Ok(science::auto_raster(&planes, None));
        }
        let idx = if self.summary.axes.is_empty() {
            Vec::new()
        } else {
            vec![0]
        };
        let p = self.plane(&idx, cancel)?;
        Ok(crate::georaster_dec::grey_rgba(&p))
    }
}

impl Session for PlanetarySession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.file.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode_sampled(&self, long_side: u32, cancel: &CancelToken) -> Option<Result<Raster>> {
        // Every n-th value at twice the thumbnail's size or more, so it is still filtered.
        let want = long_side.max(1).saturating_mul(2);
        let step = self.info.width.max(self.info.height) / want;
        if step <= 1 {
            return None;
        }
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        let p = self.page();
        let l = p.layout.subsampled(step);
        let plane = |b: u32| {
            let (values, truncated) = l.band(p.data.bytes(), b);
            Arc::new(NdPlane {
                width: l.width,
                height: l.height,
                values,
                truncated,
            })
        };
        Some(Ok(if self.summary.channel_axis.is_some() {
            science::auto_raster(&[plane(0), plane(1), plane(2)], None)
        } else {
            crate::georaster_dec::grey_rgba(&plane(0))
        }))
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
        let planes = if self.summary.channel_axis.is_some() {
            3
        } else {
            1
        };
        px * 4 * planes
            + px * 4
            + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
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
    fn page_count(&self) -> u32 {
        self.file.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.file.pages.len() > 1).then(|| PageLabel::Object {
            name: self.page().name.as_str().into(),
            class: self.file.format.name().into(),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(PlanetarySession::new(self.file.clone(), page)?))
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
    use crate::Registry;

    fn vicar_file() -> Vec<u8> {
        let values: Vec<f64> = (0..12).map(f64::from).collect();
        vicar::write(
            "HALF",
            "BSQ",
            "HIGH",
            "IEEE",
            (4, 3, 1),
            2,
            1,
            "PROPERTY='IDENTIFICATION'  TARGET_NAME='IO'  PROPERTY='MAP'  \
             MAP_PROJECTION_TYPE='SIMPLE_CYLINDRICAL'  MAP_SCALE=0.5",
            &values,
        )
    }

    #[test]
    fn vicar_in_memory() {
        let src = Arc::new(Source::from_vec("x.img", vicar_file()));
        let s = Registry::new().open(src).unwrap();
        assert_eq!(s.info().format, Format::Vicar);
        assert_eq!((s.info().width, s.info().height), (4, 3));
        let sc = s.science().unwrap();
        let f = sc.planetary.as_ref().unwrap();
        assert_eq!(f.target.as_deref(), Some("IO"));
        let m = f.map.as_ref().unwrap();
        assert_eq!(m.projection, "SIMPLE CYLINDRICAL");
        assert_eq!(m.metres_per_pixel, Some(500.0));
        let c = CancelToken::new();
        let p = s.nd_plane(&[], &c).unwrap().unwrap();
        assert_eq!(p.values[5], 5.0);
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!((r.width, r.height), (4, 3));
        crate::trim_check::assert_trims(s.as_ref(), &DecodeRequest::full(1));
    }

    #[test]
    fn pds3_attached_with_bands_and_pages() {
        let values: Vec<f64> = (0..4 * 3 * 3).map(f64::from).collect();
        let f = pds3::write_attached(
            (4, 3, 3),
            "MSB_INTEGER",
            16,
            "SAMPLE_INTERLEAVED",
            "MISSING_CONSTANT = 0",
            &values,
        );
        let src = Arc::new(Source::from_vec("x.IMG", f));
        let s = Registry::new().open(src).unwrap();
        assert_eq!(s.info().format, Format::Pds3);
        let sc = s.science().unwrap();
        assert_eq!(sc.channel_axis, Some(0));
        assert_eq!(
            sc.planetary.as_ref().unwrap().target.as_deref(),
            Some("MARS")
        );
        let c = CancelToken::new();
        let p = s.nd_plane(&[2], &c).unwrap().unwrap();
        assert_eq!(p.values[0], 24.0);
        let p0 = s.nd_plane(&[0], &c).unwrap().unwrap();
        assert!(p0.values[0].is_nan());
        let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(r.width, 4);
        assert_eq!(s.page_count(), 1);
    }

    #[test]
    fn detached_labels_on_disk() {
        let dir = std::env::temp_dir().join(format!("specular-pds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let data: Vec<u8> = (0..20u8).collect();
        std::fs::write(dir.join("SCENE.DAT"), &data).unwrap();
        std::fs::write(
            dir.join("scene.lbl"),
            "PDS_VERSION_ID = PDS3\nRECORD_BYTES = 5\n^IMAGE = (\"SCENE.DAT\", 2)\n\
             ^BROWSE_IMAGE = (\"scene.dat\", 1)\n\
             OBJECT = IMAGE\n LINES = 3\n LINE_SAMPLES = 5\n SAMPLE_BITS = 8\n\
             SAMPLE_TYPE = UNSIGNED_INTEGER\nEND_OBJECT = IMAGE\n\
             OBJECT = BROWSE_IMAGE\n LINES = 2\n LINE_SAMPLES = 2\n LINE_PREFIX_BYTES = 1\n\
             SAMPLE_BITS = 8\n SAMPLE_TYPE = UNSIGNED_INTEGER\nEND_OBJECT = BROWSE_IMAGE\nEND\n",
        )
        .unwrap();
        let reg = Registry::new();
        let s = reg.open_path(&dir.join("scene.lbl")).unwrap();
        assert_eq!(s.info().format, Format::Pds3);
        assert_eq!(s.page_count(), 2);
        let c = CancelToken::new();
        let p = s.nd_plane(&[], &c).unwrap().unwrap();
        assert_eq!(p.values[0], 5.0);
        let b = s.open_page(1).unwrap();
        assert_eq!(b.info().width, 2);
        assert!(matches!(b.page_label(), Some(PageLabel::Object { .. })));
        let p = b.nd_plane(&[], &c).unwrap().unwrap();
        assert_eq!(p.values[..2], [1.0, 2.0]);
        // The data file opened directly goes through its label.
        let d = reg.open_path(&dir.join("SCENE.DAT")).unwrap();
        assert_eq!(d.info().format, Format::Pds3);
        // PDS4 beside its data.
        std::fs::write(
            dir.join("p4.xml"),
            pds4::write_label(
                "Array_2D_Image",
                "scene.DAT",
                4,
                "UnsignedByte",
                &[("Line", 2), ("Sample", 8)],
                "",
            ),
        )
        .unwrap();
        let s = reg.open_path(&dir.join("p4.xml")).unwrap();
        assert_eq!(s.info().format, Format::Pds4);
        let p = s.nd_plane(&[], &c).unwrap().unwrap();
        assert_eq!(p.values[0], 4.0);
        assert_eq!(
            s.science()
                .unwrap()
                .planetary
                .as_ref()
                .unwrap()
                .target
                .as_deref(),
            Some("Moon")
        );
        // A missing data file and a JPEG 2000 one are refused by name.
        std::fs::write(
            dir.join("gone.lbl"),
            "PDS_VERSION_ID = PDS3\nRECORD_BYTES = 5\n^IMAGE = \"GONE.IMG\"\n\
             OBJECT = IMAGE\n LINES = 3\n LINE_SAMPLES = 5\nEND_OBJECT = IMAGE\nEND\n",
        )
        .unwrap();
        let e = reg.open_path(&dir.join("gone.lbl")).err().unwrap();
        assert!(
            matches!(&e, DecodeError::Unsupported(m) if m.contains("GONE.IMG")),
            "{e:?}"
        );
        std::fs::write(
            dir.join("jp.lbl"),
            "PDS_VERSION_ID = PDS3\n^IMAGE = \"X.JP2\"\nOBJECT = IMAGE\n LINES = 3\n\
             LINE_SAMPLES = 5\nEND_OBJECT = IMAGE\nEND\n",
        )
        .unwrap();
        let e = reg.open_path(&dir.join("jp.lbl")).err().unwrap();
        assert!(
            matches!(&e, DecodeError::Unsupported(m) if m.contains("JPEG 2000")),
            "{e:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
