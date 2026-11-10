//! ASDF (`.asdf`, the Advanced Scientific Data Format of Roman and JWST) through
//! `specular-asdf`: one page per n-dimensional array, the science data first (`data`, then
//! `err` and `dq`), axes past the image plane as cube sliders. Rows are shown bottom-up as in
//! FITS. The YAML tree is read up front and listed as keywords; blocks are read and
//! decompressed only when a plane is wanted. External array files are read only from the
//! file's own folder. World coordinates are a TAN approximation of the pointing in
//! `meta.wcsinfo` (GWCS itself is not followed), marked approximate.

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use specular_asdf::{self as asdf, ArraySource, Asdf, NdArray};
use specular_core::{CancelToken, Orientation, Raster, Source};

use crate::science::{self, AsdfFacts, AxisKind, NdAxis, NdPlane, PlaneCache, ScienceSummary};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

pub struct AsdfDecoder;

/// Arrays offered as pages at most.
const MAX_PAGES: usize = 64;
/// Tree entries listed as keywords at most.
const MAX_KEYWORDS: usize = 4000;
/// Largest external array file read.
const MAX_EXTERNAL: u64 = 8 << 30;

fn map_err(e: asdf::Error) -> DecodeError {
    match e {
        asdf::Error::NotAsdf => DecodeError::UnsupportedFormat,
        asdf::Error::Malformed(s) => DecodeError::Malformed(s),
        asdf::Error::Unsupported(s) => DecodeError::Unsupported(s),
        asdf::Error::Yaml(y) => DecodeError::Unsupported(y.to_string()),
        asdf::Error::TooLarge => DecodeError::TooLarge,
        asdf::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

impl Decoder for AsdfDecoder {
    fn format(&self) -> Format {
        Format::Asdf
    }
    fn probe(&self, head: &[u8]) -> bool {
        asdf::sniff(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["asdf"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let file = Asdf::open(source.bytes()).map_err(map_err)?;
        let pages = pages(&file);
        if pages.is_empty() {
            return Err(DecodeError::Unsupported(
                "no image array in the ASDF file".into(),
            ));
        }
        let keywords: Vec<(String, String, String)> = asdf::flatten(&file.tree, MAX_KEYWORDS)
            .into_iter()
            .map(|(k, v)| (k, v, String::new()))
            .collect();
        let shared = Arc::new(Shared {
            facts: facts(&file),
            meta: asdf::science_meta(&file.tree),
            keywords: Arc::new(keywords),
            file,
            pages,
        });
        Ok(Arc::new(AsdfSession::new(source, shared, 0)?))
    }
}

/// The ASDF facts of an open file.
pub(crate) fn facts(file: &Asdf) -> AsdfFacts {
    AsdfFacts {
        version: file.version.clone(),
        standard: file.standard.clone(),
        library: file
            .library()
            .map(|(n, v)| format!("{n} {v}").trim().to_owned()),
        model: asdf::science_meta(&file.tree).model,
    }
}

/// Fills what the summary lacks from an ASDF tree's observation facts (Roman `roman.meta`,
/// JWST `meta`).
pub(crate) fn describe(summary: &mut ScienceSummary, m: &asdf::Meta) {
    let fill = |dst: &mut Option<String>, v: &Option<String>| {
        if dst.is_none() {
            dst.clone_from(v);
        }
    };
    fill(&mut summary.object, &m.target);
    fill(&mut summary.telescope, &m.telescope);
    fill(&mut summary.instrument, &m.instrument);
    fill(&mut summary.filter, &m.filter);
    fill(&mut summary.date_obs, &m.start_time);
    fill(&mut summary.detector, &m.detector);
    if summary.exposure.is_none() {
        summary.exposure = m.exposure_time;
    }
}

/// Whether an array of this name holds data quality flags (`dq`, `groupdq`, `pixeldq`).
pub(crate) fn is_flags_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with("dq")
}

/// The arrays shown as pages, as indices into `file.arrays`: the science model's `data`,
/// `err` and `dq` first, then the others in tree order.
fn pages(file: &Asdf) -> Vec<usize> {
    let mut v: Vec<(u8, usize)> = file
        .arrays
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            let s = a.squeezed();
            s.shape.len() >= 2 && s.shape.iter().all(|&n| n > 0 && n <= u64::from(u32::MAX))
        })
        .map(|(i, a)| {
            let top = matches!(a.parent(), "" | "roman");
            let rank = match a.name() {
                "data" | "sci" if top => 0,
                "err" if top => 1,
                "dq" if top => 2,
                _ if top => 3,
                _ => 4,
            };
            (rank, i)
        })
        .collect();
    v.sort_unstable();
    v.truncate(MAX_PAGES);
    v.into_iter().map(|(_, i)| i).collect()
}

/// What every page of a file shares.
struct Shared {
    file: Asdf,
    pages: Vec<usize>,
    facts: AsdfFacts,
    meta: asdf::Meta,
    keywords: Arc<Vec<(String, String, String)>>,
}

struct AsdfSession {
    source: Arc<Source>,
    shared: Arc<Shared>,
    page: u32,
    info: ImageInfo,
    summary: ScienceSummary,
    /// Decompressed or external source data kept while a cube's planes are read: (source
    /// key, bytes).
    held: Mutex<Vec<(String, Arc<Held>)>>,
    planes: PlaneCache,
    levels: Levels,
}

/// The data an array's source holds, borrowed from the file or held decompressed.
enum Bytes<'a> {
    Mapped(&'a [u8]),
    Held(Arc<Held>),
}

/// Decompressed source data, its memory reserved from the budget while it is held.
struct Held {
    bytes: Vec<u8>,
    _reserved: specular_core::Reservation,
}

impl std::ops::Deref for Bytes<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Bytes::Mapped(b) => b,
            Bytes::Held(v) => &v.bytes,
        }
    }
}

impl AsdfSession {
    fn new(source: Arc<Source>, shared: Arc<Shared>, page: u32) -> Result<AsdfSession> {
        let a = &shared.file.arrays[shared.pages[page as usize]];
        let s = a.squeezed();
        let nd = s.shape.len();
        let (w, h) = (s.shape[nd - 1], s.shape[nd - 2]);
        let (width, height) = (
            u32::try_from(w).map_err(|_| DecodeError::TooLarge)?,
            u32::try_from(h).map_err(|_| DecodeError::TooLarge)?,
        );
        let axes = s.shape[..nd - 2]
            .iter()
            .map(|&n| {
                Ok(NdAxis {
                    name: String::new(),
                    len: u32::try_from(n).map_err(|_| DecodeError::TooLarge)?,
                    unit: String::new(),
                    labels: Vec::new(),
                    wcs_axis: None,
                    linear: None,
                    kind: AxisKind::Cube,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        // The pointing describes the science image: arrays of its size share it.
        let main = &shared.file.arrays[shared.pages[0]].squeezed();
        let main_size = (
            main.shape[main.shape.len() - 1],
            main.shape[main.shape.len() - 2],
        );
        let wcs = ((w, h) == main_size)
            .then(|| asdf::approx_wcs_cards(&shared.file.tree, w, h))
            .flatten()
            .and_then(|cards| {
                let header = specular_fits::Header::from_pairs(
                    cards.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")),
                );
                specular_fits::wcs::Wcs::from_header(&header, &[w as usize, h as usize])
            })
            .map(Arc::new);
        let mut sample = a.dtype.name().to_owned();
        if a.big_endian && a.dtype.size() > 1 {
            sample += ", big-endian";
        }
        if let ArraySource::Block(i) = a.source
            && let Some(c) = shared.file.blocks.get(i).and_then(|b| b.compression_name())
        {
            sample += &format!(", {c}");
        }
        let mut summary = ScienceSummary {
            width,
            height,
            axes,
            wcs_approximate: wcs.is_some(),
            wcs,
            sample,
            dataset: Some(a.path.clone()),
            unit: a.unit.clone().filter(|u| !u.trim().is_empty()),
            keywords: shared.keywords.clone(),
            flags: a.dtype.is_int() && is_flags_name(a.name()),
            asdf: Some(shared.facts.clone()),
            ..Default::default()
        };
        describe(&mut summary, &shared.meta);
        let info = ImageInfo {
            format: Format::Asdf,
            width,
            height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: a.dtype.bits().min(64) as u8,
            progressive: false,
            coding: summary.sample.clone(),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(AsdfSession {
            source,
            shared,
            page,
            info,
            summary,
            held: Mutex::new(Vec::new()),
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn array(&self) -> &NdArray {
        &self.shared.file.arrays[self.shared.pages[self.page as usize]]
    }

    /// The source data of `x` (the page's array or its mask array).
    fn bytes(&self, x: &NdArray) -> Result<Bytes<'_>> {
        let data = self.source.bytes();
        let key = match &x.source {
            ArraySource::Inline(_) => return Ok(Bytes::Mapped(&[])),
            ArraySource::Block(i) => format!("#{i}"),
            ArraySource::External(name) => name.clone(),
        };
        if let Some((_, b)) = self.held.lock().iter().find(|(k, _)| *k == key) {
            return Ok(Bytes::Held(b.clone()));
        }
        let (bytes, reserved) = match &x.source {
            ArraySource::External(name) => external(self.source.path(), name)?,
            _ => match self
                .shared
                .file
                .array_data(data, x)
                .map_err(map_err)?
                .into_parts()
            {
                (std::borrow::Cow::Borrowed(b), _) => return Ok(Bytes::Mapped(b)),
                (std::borrow::Cow::Owned(v), r) => (v, r),
            },
        };
        let b = Arc::new(Held {
            bytes,
            _reserved: reserved,
        });
        let mut g = self.held.lock();
        if g.len() >= 2 {
            g.remove(0);
        }
        g.push((key, b.clone()));
        Ok(Bytes::Held(b))
    }

    fn read(&self, index: &[u32]) -> Result<NdPlane> {
        let a = self.array();
        if index.len() != self.summary.axes.len()
            || index
                .iter()
                .zip(&self.summary.axes)
                .any(|(&i, ax)| i >= ax.len)
        {
            return Err(DecodeError::Malformed("no such plane".into()));
        }
        let lead: Vec<u64> = index.iter().map(|&i| u64::from(i)).collect();
        let mut values = {
            let b = self.bytes(a)?;
            a.squeezed().plane(&b, &lead).map_err(map_err)?
        };
        if let Some(m) = &a.mask_array {
            let b = self.bytes(m)?;
            let mask = m.squeezed().plane(&b, &lead).map_err(map_err)?;
            NdArray::apply_mask(&mut values, &mask);
        }
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        if values.len() != w * h {
            return Err(DecodeError::Malformed("plane size".into()));
        }
        // Bottom-up as in FITS: the array's first row is shown at the bottom.
        let mut flipped = Vec::with_capacity(values.len());
        for row in values.chunks_exact(w.max(1)).rev() {
            flipped.extend_from_slice(row);
        }
        Ok(NdPlane {
            width: self.info.width,
            height: self.info.height,
            values: flipped,
            truncated: false,
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        let p = self.planes.get_or(index, cancel, || self.read(index));
        // A single plane needs its source no longer.
        if self.summary.planes() <= 1 {
            self.held.lock().clear();
        }
        p
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let first = self.plane(&vec![0; self.summary.axes.len()], cancel)?;
        Ok(science::auto_raster(&[first], None))
    }
}

/// The data of the external array file `name` beside `path`: a bare file name only, so
/// that a file cannot reach outside its own folder. Reserved from the budget while held.
fn external(path: &Path, name: &str) -> Result<(Vec<u8>, specular_core::Reservation)> {
    let bare = !name.is_empty()
        && !name.contains(['/', '\\', ':', '\0'])
        && name != "."
        && name != ".."
        && Path::new(name).file_name().is_some_and(|f| f == name);
    if !bare {
        return Err(DecodeError::Unsupported(format!(
            "external array file {name} is outside the file's folder"
        )));
    }
    let p = path.parent().unwrap_or(Path::new("")).join(name);
    let len = std::fs::metadata(&p)
        .map_err(|e| DecodeError::Malformed(format!("external array file {name}: {e}")))?
        .len();
    if len > MAX_EXTERNAL {
        return Err(DecodeError::TooLarge);
    }
    let file = specular_core::budget::reserve(len)?;
    let data = std::fs::read(&p)
        .map_err(|e| DecodeError::Malformed(format!("external array file {name}: {e}")))?;
    let (bytes, reserved) = asdf::external_data(&data).map_err(map_err)?.into_parts();
    Ok(match bytes {
        // The block is stored: the whole file is kept, under its own reservation.
        std::borrow::Cow::Borrowed(b) => {
            let b = b.to_vec();
            drop(data);
            (b, file)
        }
        std::borrow::Cow::Owned(v) => (v, reserved),
    })
}

impl Session for AsdfSession {
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
        let a = self.array();
        let block = match a.source {
            ArraySource::Block(i) => self
                .shared
                .file
                .blocks
                .get(i)
                .filter(|b| b.compression_name().is_some())
                .map_or(0, |b| b.data_size),
            ArraySource::External(_) => a.len().saturating_mul(a.dtype.size() as u64),
            ArraySource::Inline(_) => 0,
        };
        px * 8 + block + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
            + self.planes.memory_bytes()
            + self
                .held
                .lock()
                .iter()
                .map(|(_, b)| b.bytes.len())
                .sum::<usize>()
    }
    fn trim_memory(&self) -> usize {
        let held: usize = self.held.lock().drain(..).map(|(_, b)| b.bytes.len()).sum();
        self.levels.clear() + self.planes.clear() + held
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn page_count(&self) -> u32 {
        self.shared.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.shared.pages.len() > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: Some(u32::from(self.info.bit_depth)),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.shared.pages.len() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        Ok(Arc::new(AsdfSession::new(
            self.source.clone(),
            self.shared.clone(),
            page,
        )?))
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

    fn fixture(name: &str) -> Arc<Source> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-asdf/tests/data")
            .join(name);
        Arc::new(Source::open(&p).expect("fixture"))
    }

    #[test]
    fn roman_like_pages_meta_and_flags() {
        let s = AsdfDecoder.open(fixture("roman_like.asdf")).expect("open");
        let sc = s.science().expect("science");
        assert_eq!(sc.dataset.as_deref(), Some("roman.data"));
        assert!(
            sc.asdf
                .as_ref()
                .is_some_and(|f| f.version.starts_with("1."))
        );
        assert!(!sc.keywords.is_empty());
        assert!(!sc.flags);
        // The fake model's pointing is placeholders: no world coordinates.
        assert!(sc.wcs.is_none() && !sc.wcs_approximate);
        assert!(sc.telescope.is_some() && sc.instrument.is_some());
        let names: Vec<String> = (0..s.page_count())
            .map(|p| {
                s.open_page(p)
                    .expect("page")
                    .science()
                    .and_then(|x| x.dataset.clone())
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(names[..3], ["roman.data", "roman.err", "roman.dq"]);
        let dq = s.open_page(2).expect("dq");
        assert!(dq.science().expect("science").flags);
        let p = dq
            .nd_plane(&[], &CancelToken::new())
            .expect("plane")
            .expect("read");
        assert_eq!(p.values.len() as u32, p.width * p.height);
    }

    #[test]
    fn rows_are_bottom_up() {
        let src = fixture("dtypes_le.asdf");
        let data = src.bytes().to_vec();
        let file = Asdf::open(&data).expect("open");
        let s = AsdfDecoder.open(src).expect("session");
        let sc = s.science().expect("science").clone();
        let path = sc.dataset.clone().expect("dataset");
        let a = file.arrays.iter().find(|a| a.path == path).expect("array");
        let lead = vec![0u64; sc.axes.len()];
        let want = file
            .plane(&data, a, &lead, &mut |_| Err(asdf::Error::NotAsdf))
            .expect("plane");
        let got = s
            .nd_plane(&vec![0; sc.axes.len()], &CancelToken::new())
            .expect("plane")
            .expect("read");
        let w = got.width as usize;
        let h = got.height as usize;
        for y in 0..h {
            let a = &got.values[y * w..y * w + w];
            let b = &want[(h - 1 - y) * w..(h - y) * w];
            assert!(
                a.iter()
                    .zip(b)
                    .all(|(x, y)| x == y || x.is_nan() && y.is_nan())
            );
        }
    }

    #[test]
    fn jwst_fits_takes_its_metadata_from_the_asdf_extension() {
        let s = crate::fits_dec::FitsDecoder { fpack: false }
            .open(fixture("jwst_like.fits"))
            .expect("open");
        let sc = s.science().expect("science");
        assert_eq!(sc.axes.len(), 1, "SCI, ERR and DQ on one extension axis");
        assert_eq!(sc.axes[0].labels, ["SCI", "ERR", "DQ"]);
        assert!(!sc.flags_at(&[0]) && !sc.flags_at(&[1]) && sc.flags_at(&[2]));
        let f = sc.asdf.as_ref().expect("asdf facts");
        assert!(f.version.starts_with("1."));
        assert!(f.library.as_deref().is_some_and(|l| l.starts_with("asdf ")));
        assert_eq!(sc.object.as_deref(), Some("NGC 1234"));
        assert_eq!(sc.filter.as_deref(), Some("F200W"));
        assert_eq!(sc.exposure, Some(42.5));
        assert!(
            sc.wcs.is_some() && sc.wcs_approximate,
            "TAN from meta.wcsinfo"
        );
        assert!(
            sc.keywords
                .iter()
                .any(|(k, v, c)| k == "meta.instrument.detector" && v == "NRCA1" && c == "ASDF")
        );
    }

    const POINTED: &str = "#ASDF 1.0.0\n#ASDF_STANDARD 1.5.0\n%YAML 1.1\n%TAG ! tag:stsci.edu:asdf/\n--- !core/asdf-1.1.0\nroman:\n  meta:\n    telescope: ROMAN\n    instrument: {name: WFI, optical_element: F158}\n    wcsinfo: {ra_ref: 10.5, dec_ref: 41.2, roll_ref: 0.0, v3yangle: 0.0, vparity: -1}\n  data: !core/ndarray-1.0.0\n    data: [[1, 2, 3], [4, 5, 6]]\n    datatype: float32\n    shape: [2, 3]\n...\n";

    #[test]
    fn roman_pointing_gives_approximate_wcs() {
        let src = Arc::new(Source::from_vec(
            "/nowhere/pointed.asdf",
            POINTED.as_bytes().to_vec(),
        ));
        let s = AsdfDecoder.open(src).expect("open");
        let sc = s.science().expect("science");
        assert!(sc.wcs_approximate);
        let w = sc.wcs.as_ref().expect("wcs");
        // The reference point sits at the image centre.
        let (ra, dec) = w.sky_at(1.0, 0.5, &[]).expect("sky");
        assert!(
            (ra - 10.5).abs() < 1e-3 && (dec - 41.2).abs() < 1e-3,
            "{ra} {dec}"
        );
        // Inline data, bottom-up.
        let p = s
            .nd_plane(&[], &CancelToken::new())
            .expect("plane")
            .expect("read");
        assert_eq!(p.values, [4.0, 5.0, 6.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn external_names_stay_in_the_folder() {
        let p = Path::new("/nowhere/file.asdf");
        for bad in ["../x.asdf", "sub/x.asdf", "/etc/passwd", "..", "", "a\\b"] {
            assert!(
                matches!(external(p, bad), Err(DecodeError::Unsupported(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn external_arrays_read_beside_the_file() {
        let s = AsdfDecoder.open(fixture("external.asdf")).expect("open");
        for p in 0..s.page_count() {
            let page = s.open_page(p).expect("page");
            let n = page.science().expect("science").axes.len();
            page.nd_plane(&vec![0; n], &CancelToken::new())
                .expect("plane")
                .expect("read");
        }
    }
}
