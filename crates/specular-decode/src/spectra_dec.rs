//! Lab spectra through `specular-jcamp` (JCAMP-DX, SPC), `specular-mzml` (mzML, mzXML) and
//! `specular-nmr` (Bruker NMR folders): each page a plot, drawn by `specular-vector` like a
//! drawing whose sheet is a chart, the viewer drawing the axes over it.
//!
//! A page's scene is made when the page opens, so an mzML run of 100 000 spectra reads one
//! spectrum per page shown. The file's facts and the page's notes (how it was processed)
//! are the info panel's ([`Session::spectrum`]); the file's labelled fields its "all fields"
//! list ([`Session::header`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Raster, Source};
use specular_spectra::{SpectraFile, Technique};

use crate::{
    DecodeError, DecodeRequest, Decoder, Format, HeaderFacts, ImageInfo, PageLabel, Result, Session,
};

/// What the info panel shows about a spectrum file and its page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpectrumFacts {
    /// "JCAMP-DX", "SPC", "mzML", "mzXML", "Bruker NMR", and its version as stored.
    pub format: String,
    pub version: String,
    /// What was measured, in English ("Infrared", "NMR").
    pub technique: String,
    pub title: Option<String>,
    pub origin: Option<String>,
    pub instrument: Option<String>,
    pub date: Option<String>,
    pub spectra: u64,
    pub points: u64,
    pub nucleus: Option<String>,
    pub frequency_mhz: Option<f64>,
    pub solvent: Option<String>,
    pub pulse_program: Option<String>,
    /// The page shown (from 1) of how many, and its name ("Scan 1532").
    pub page: u32,
    pub pages: u32,
    pub page_name: String,
    /// The page's axes: title and unit.
    pub x_axis: (String, String),
    pub y_axis: (String, String),
    /// How the page was made (an automatic phase correction), English.
    pub notes: Vec<String>,
    /// Problems that did not stop reading, English.
    pub warnings: Vec<String>,
}

fn err(e: specular_spectra::Error) -> DecodeError {
    use specular_spectra::Error as E;
    match e {
        E::NotThisFormat => DecodeError::UnsupportedFormat,
        E::Truncated(s) => DecodeError::Truncated(s),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
    }
}

/// A source's bytes for readers that keep them (mzML pages are read when shown), with the
/// budget reservation of an inflated `.mzML.gz` held as long as they are.
struct SourceBytes {
    source: Arc<Source>,
    _hold: Option<specular_core::Reservation>,
}

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.source.bytes()
    }
}

/// The largest `.mzML.gz` / `.mzXML.gz` inflated (the memory budget usually refuses first).
const MAX_GZ: usize = 1 << 36;

fn is_gzip(data: &[u8]) -> bool {
    data.starts_with(&[0x1f, 0x8b])
}

/// A whole-file gzip (`.mzML.gz`, as repositories serve them) inflated into memory, its size
/// reserved from the budget as it grows; or the file itself.
fn gunzip(source: &Arc<Source>) -> Result<SourceBytes> {
    if !is_gzip(source.bytes()) {
        return Ok(SourceBytes {
            source: source.clone(),
            _hold: None,
        });
    }
    let data =
        specular_png::inflate_gzip_members(source.bytes(), MAX_GZ, None).map_err(|e| match e {
            specular_png::InflateError::NeedsMemory(n) => DecodeError::NeedsMemory(n),
            e => DecodeError::Malformed(format!("gzip stream: {e}")),
        })?;
    let hold = specular_core::budget::reserve(data.len() as u64)?;
    let inflated = Source::from_entry(source.path(), source, data);
    Ok(SourceBytes {
        source: Arc::new(inflated),
        _hold: Some(hold),
    })
}

/// Whether `path` is a data or parameter file of a Bruker experiment (`fid`, `ser`, `acqus`,
/// `pdata/<n>/1r` …), which opens its experiment.
pub(crate) fn is_bruker_data_file(path: &Path) -> bool {
    bruker_dir_of(path).is_some()
}

/// The experiment (or processing) folder a Bruker file belongs to.
fn bruker_dir_of(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    if !matches!(
        name,
        "fid" | "ser" | "acqus" | "acqu" | "acqu2s" | "1r" | "1i" | "2rr" | "procs" | "proc2s"
    ) {
        return None;
    }
    let dir = path.parent()?;
    specular_core::is_bruker_folder(dir).then(|| dir.to_path_buf())
}

/// JCAMP-DX, SPC, mzML, mzXML and Bruker NMR, by `Format`.
pub struct SpectraDecoder(pub Format);

impl Decoder for SpectraDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Format::Jcamp => specular_jcamp::jcamp::sniff(head),
            Format::Spc => specular_jcamp::spc::sniff(head),
            Format::Mzml => specular_mzml::sniff_mzml(head),
            Format::Mzxml => specular_mzml::sniff_mzxml(head),
            _ => false,
        }
    }
    /// Text formats: their signs may come after comments or a long prolog. mzML and mzXML
    /// also inside a whole-file gzip.
    fn probe_file(&self, data: &[u8]) -> bool {
        if matches!(self.0, Format::Mzml | Format::Mzxml) && is_gzip(data) {
            return specular_png::inflate_gzip_prefix(data, crate::PROBE_TEXT)
                .is_ok_and(|h| self.probe(&h));
        }
        self.probe(&data[..data.len().min(crate::PROBE_TEXT)])
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::Jcamp => &["jdx", "dx", "jcm", "jcamp"],
            Format::Spc => &["spc"],
            Format::Mzml => &["mzml", "mzml.gz"],
            Format::Mzxml => &["mzxml", "mzxml.gz"],
            _ => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let file: Arc<dyn SpectraFile> = match self.0 {
            Format::Jcamp => Arc::new(specular_jcamp::jcamp::parse(data).map_err(err)?),
            // Opened by extension too: the reader refuses what is not SPC (a Shimadzu .spc by
            // name).
            Format::Spc => Arc::new(specular_jcamp::spc::parse(data).map_err(err)?),
            Format::Mzml | Format::Mzxml => {
                let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(gunzip(&source)?);
                Arc::new(specular_mzml::open(bytes).map_err(err)?)
            }
            _ => {
                let dir = if source.is_folder() {
                    source.path().to_path_buf()
                } else {
                    bruker_dir_of(source.path()).ok_or(DecodeError::UnsupportedFormat)?
                };
                Arc::new(specular_nmr::open(&dir).map_err(err)?)
            }
        };
        if file.page_count() == 0 {
            return Err(DecodeError::Unsupported("no spectra in the file".into()));
        }
        session(source, file, self.0, 0)
    }
}

/// The session of page `page` of `file`.
fn session(
    source: Arc<Source>,
    file: Arc<dyn SpectraFile>,
    format: Format,
    page: u32,
) -> Result<Arc<dyn Session>> {
    let t0 = std::time::Instant::now();
    let p = page as usize;
    let plot = file.page(p).map_err(err)?;
    let scene = specular_spectra::chart::scene(file.as_ref(), [p]);
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    let inner = crate::dxf_dec::session(source.clone(), scene, format, ms)?;
    let f = file.facts();
    let facts = SpectrumFacts {
        format: f.format.clone(),
        version: f.version.clone(),
        technique: technique_name(f.technique).to_owned(),
        title: f.title.clone(),
        origin: f.origin.clone(),
        instrument: f.instrument.clone(),
        date: f.date.clone(),
        spectra: f.spectra,
        points: f.points,
        nucleus: f.nucleus.clone(),
        frequency_mhz: f.frequency_mhz,
        solvent: f.solvent.clone(),
        pulse_program: f.pulse_program.clone(),
        page: page + 1,
        pages: file.page_count() as u32,
        page_name: file.page_name(p),
        x_axis: (plot.x.title.clone(), plot.x.unit.clone()),
        y_axis: (plot.y.title.clone(), plot.y.unit.clone()),
        notes: plot.notes.clone(),
        warnings: f.warnings.clone(),
    };
    let header = (!f.fields.is_empty()).then(|| HeaderFacts {
        summary: Vec::new(),
        fields: f.fields.clone(),
    });
    Ok(Arc::new(SpectraSession {
        inner,
        file,
        format,
        page,
        facts,
        header,
    }))
}

fn technique_name(t: Technique) -> &'static str {
    t.english()
}

struct SpectraSession {
    /// The page's chart as a drawing session.
    inner: Arc<dyn Session>,
    file: Arc<dyn SpectraFile>,
    format: Format,
    page: u32,
    facts: SpectrumFacts,
    header: Option<HeaderFacts>,
}

impl Session for SpectraSession {
    fn info(&self) -> &ImageInfo {
        self.inner.info()
    }
    fn source(&self) -> &Arc<Source> {
        self.inner.source()
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
    }
    fn memory_bytes(&self) -> usize {
        self.inner.memory_bytes()
    }
    fn page_count(&self) -> u32 {
        self.file.page_count() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        let name = self.file.page_name(self.page as usize);
        (!name.is_empty()).then(|| PageLabel::Map {
            name: name.as_str().into(),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.file.page_count() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        session(
            self.inner.source().clone(),
            self.file.clone(),
            self.format,
            page,
        )
    }
    fn drawing(&self) -> Option<&crate::drawing::DrawingPage> {
        self.inner.drawing()
    }
    fn spectrum(&self) -> Option<&SpectrumFacts> {
        Some(&self.facts)
    }
    fn header(&self) -> Option<&HeaderFacts> {
        self.header.as_ref()
    }
}
