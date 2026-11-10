//! mzML (indexed or not) and mzXML mass spectrometry files, with our own readers.
//!
//! Opening reads the header and finds where every spectrum starts: from the file's index
//! when it matches the contents, else by one fast pass over the bytes for start tags. A
//! spectrum is read only when its page is asked for. The pages are an overview (the file's
//! TIC chromatogram, else one built from the MS1 spectra), the other chromatograms, then a
//! page per spectrum: m/z against intensity, as sticks when centroided.
//!
//! Binary arrays: 16/32/64-bit floats, 32/64-bit integers, zlib, MS-Numpress (linear,
//! positive integer, short logged float, each also followed by zlib). zstd and truncation
//! compression are refused by name.

pub mod codec;
mod mzml;
mod mzxml;
mod xml;

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use memchr::memmem;
use specular_spectra::{Axis, Facts, Peak, Plot, SpectraFile, Trace, warn};

pub use specular_spectra::Error;

/// Spectra listed at most; past it the rest are left out with a warning.
pub const MAX_ENTRIES: usize = 200_000;
/// Chromatograms whose headers are read at open to find the TIC, at most.
const MAX_CHROM_HEADERS: usize = 4096;
/// Time spent summing intensities for a TIC the spectra do not state.
const SUM_BUDGET: Duration = Duration::from_secs(3);
/// Bytes at the start that the sniffers look at.
const SNIFF: usize = 16 << 10;

/// The file's bytes, shared.
pub type Data = Arc<dyn AsRef<[u8]> + Send + Sync>;

fn has(h: &[u8], n: &[u8]) -> bool {
    memmem::find(h, n).is_some()
}

/// Whether the start of a file is an mzML document (not imzML: those name the imaging
/// vocabulary and belong to the imzML reader).
pub fn sniff_mzml(head: &[u8]) -> bool {
    let h = &head[..head.len().min(SNIFF)];
    (has(h, b"<mzML") || has(h, b"<indexedmzML")) && !(has(h, b"\"IMS\"") || has(h, b"IMS:1000"))
}

/// Whether the start of a file is an mzXML document.
pub fn sniff_mzxml(head: &[u8]) -> bool {
    has(&head[..head.len().min(SNIFF)], b"<mzXML")
}

/// Which format a file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Mzml,
    Mzxml,
}

/// Where a spectrum or chromatogram starts, and its id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub offset: usize,
    pub id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Polarity {
    Positive,
    Negative,
}

/// What the x array of a record is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum XKind {
    #[default]
    None,
    Mz,
    Wavelength,
    Time,
}

/// One spectrum or chromatogram as stored.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    pub id: String,
    /// Its position in the file's list.
    pub index: usize,
    pub ms_level: Option<u32>,
    pub centroid: Option<bool>,
    pub polarity: Option<Polarity>,
    /// Retention time (scan start time), minutes.
    pub rt_min: Option<f64>,
    pub tic: Option<f64>,
    pub base_mz: Option<f64>,
    pub base_intensity: Option<f64>,
    pub precursor_mz: Option<f64>,
    pub charge: Option<i32>,
    pub default_length: Option<usize>,
    pub x_kind: XKind,
    /// The x array as stored (time in its own unit: multiply by `x_to_minutes`).
    pub x: Vec<f64>,
    pub x_to_minutes: f64,
    pub y: Vec<f64>,
    /// A chromatogram: the total ion current one, and its kind's name.
    pub is_tic: bool,
    pub kind_name: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Overview {
    /// The file's own TIC chromatogram (index into the chromatograms).
    Chromatogram(usize),
    /// Built from the spectra.
    Built,
}

/// An open mzML or mzXML file.
pub struct MzFile {
    data: Data,
    kind: Kind,
    facts: Facts,
    groups: mzml::Groups,
    spectra: Vec<Entry>,
    chromatograms: Vec<Entry>,
    overview: Option<Overview>,
    /// Chromatograms that have a page of their own (all but the overview's).
    chrom_pages: Vec<usize>,
    built_tic: OnceLock<Result<Plot, Error>>,
}

/// Opens an mzML or mzXML file (which one, by content).
pub fn open(data: Data) -> Result<MzFile, Error> {
    let bytes: &[u8] = (*data).as_ref();
    let kind = if sniff_mzml(bytes) {
        Kind::Mzml
    } else if sniff_mzxml(bytes) {
        Kind::Mzxml
    } else {
        return Err(Error::NotThisFormat);
    };
    let mut warnings = Vec::new();
    let (mut facts, groups, spectra, chromatograms, capped) = match kind {
        Kind::Mzml => {
            let h = mzml::header(bytes)?;
            let (sp, ch, capped) = mzml::entries(bytes, &mut warnings);
            (h.facts, h.groups, sp, ch, capped)
        }
        Kind::Mzxml => {
            let f = mzxml::header(bytes)?;
            let (sp, capped) = mzxml::entries(bytes, &mut warnings);
            (f, mzml::Groups::new(), sp, Vec::new(), capped)
        }
    };
    if capped {
        warn(
            &mut warnings,
            format!("Only the first {MAX_ENTRIES} spectra are listed."),
        );
    }
    if spectra.is_empty() && chromatograms.is_empty() {
        return Err(Error::Malformed(
            "no spectra or chromatograms in the file".into(),
        ));
    }
    // The TIC among the chromatograms, by its kind (headers only) or its id.
    let mut tic = None;
    for (i, e) in chromatograms.iter().enumerate().take(MAX_CHROM_HEADERS) {
        if let Ok(r) = mzml::record(bytes, e, i, &groups, true)
            && r.is_tic
        {
            tic = Some(i);
            break;
        }
    }
    if tic.is_none() {
        tic = chromatograms
            .iter()
            .position(|e| e.id.eq_ignore_ascii_case("tic"));
    }
    let overview = match tic {
        Some(i) => Some(Overview::Chromatogram(i)),
        None if !spectra.is_empty() => Some(Overview::Built),
        None => None,
    };
    let chrom_pages = (0..chromatograms.len())
        .filter(|&i| Some(i) != tic)
        .collect();
    facts.spectra = spectra.len() as u64;
    facts.points = spectra
        .first()
        .and_then(|e| xml::start_tag(bytes, e.offset))
        .and_then(|t| {
            xml::tag_attr(t, b"defaultArrayLength").or_else(|| xml::tag_attr(t, b"peaksCount"))
        })
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    if !chromatograms.is_empty() {
        facts
            .fields
            .push(("Chromatograms".into(), chromatograms.len().to_string()));
    }
    for w in warnings {
        warn(&mut facts.warnings, w);
    }
    Ok(MzFile {
        data,
        kind,
        facts,
        groups,
        spectra,
        chromatograms,
        overview,
        chrom_pages,
        built_tic: OnceLock::new(),
    })
}

/// What a page is.
enum PageKind {
    Overview(Overview),
    Chromatogram(usize),
    Spectrum(usize),
}

impl MzFile {
    fn bytes(&self) -> &[u8] {
        (*self.data).as_ref()
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn spectrum_count(&self) -> usize {
        self.spectra.len()
    }

    pub fn chromatogram_count(&self) -> usize {
        self.chromatograms.len()
    }

    pub fn spectrum_entries(&self) -> &[Entry] {
        &self.spectra
    }

    /// Spectrum `i`, arrays decoded.
    pub fn spectrum(&self, i: usize) -> Result<Record, Error> {
        self.read_spectrum(i, false)
    }

    fn read_spectrum(&self, i: usize, header_only: bool) -> Result<Record, Error> {
        let e = self
            .spectra
            .get(i)
            .ok_or_else(|| Error::Malformed(format!("no spectrum {i}")))?;
        match self.kind {
            Kind::Mzml => mzml::record(self.bytes(), e, i, &self.groups, header_only),
            Kind::Mzxml => mzxml::record(self.bytes(), e, i, header_only),
        }
    }

    /// Chromatogram `i`, arrays decoded.
    pub fn chromatogram(&self, i: usize) -> Result<Record, Error> {
        let e = self
            .chromatograms
            .get(i)
            .ok_or_else(|| Error::Malformed(format!("no chromatogram {i}")))?;
        mzml::record(self.bytes(), e, i, &self.groups, false)
    }

    fn page_kind(&self, page: usize) -> Option<PageKind> {
        let mut p = page;
        if let Some(o) = self.overview {
            if p == 0 {
                return Some(PageKind::Overview(o));
            }
            p -= 1;
        }
        if let Some(&c) = self.chrom_pages.get(p) {
            return Some(PageKind::Chromatogram(c));
        }
        p -= self.chrom_pages.len();
        (p < self.spectra.len()).then_some(PageKind::Spectrum(p))
    }

    /// The overview built from the spectra: each MS1 spectrum's TIC at its retention time.
    fn build_tic(&self) -> Result<Plot, Error> {
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        let mut all_rt = true;
        let mut summed = 0usize;
        let mut cut = None;
        let mut failed = 0usize;
        let started = Instant::now();
        for i in 0..self.spectra.len() {
            let r = match self.read_spectrum(i, true) {
                Ok(r) => r,
                Err(_) => {
                    failed += 1;
                    continue;
                }
            };
            if r.ms_level.is_some_and(|l| l != 1) {
                continue;
            }
            let tic = match r.tic {
                Some(t) => t,
                None => {
                    if started.elapsed() > SUM_BUDGET {
                        cut = Some(i);
                        break;
                    }
                    match self.spectrum(i) {
                        Ok(f) => {
                            summed += 1;
                            f.y.iter().sum()
                        }
                        Err(_) => {
                            failed += 1;
                            continue;
                        }
                    }
                }
            };
            all_rt &= r.rt_min.is_some();
            xs.push((r.rt_min, i));
            ys.push(tic);
        }
        let mut notes = vec![format!(
            "Built from the total ion current of {} MS1 spectra",
            ys.len()
        )];
        if summed > 0 {
            notes.push(format!("{summed} of them summed from their intensities"));
        }
        if let Some(i) = cut {
            notes.push(format!(
                "Spectra from {} on were left out: summing them took too long",
                i + 1
            ));
        }
        if failed > 0 {
            notes.push(format!("{failed} spectra could not be read"));
        }
        let (x, axis) = if all_rt {
            (
                xs.iter().map(|(t, _)| t.unwrap_or(0.0)).collect(),
                Axis::new("Retention time", "min", false),
            )
        } else {
            (
                xs.iter().map(|(_, i)| (*i + 1) as f64).collect(),
                Axis::new("Spectrum", "", false),
            )
        };
        Ok(Plot {
            title: "TIC".into(),
            x: axis,
            y: Axis::new("Intensity", "", false),
            traces: vec![Trace {
                name: String::new(),
                x,
                y: ys,
                sticks: false,
            }],
            peaks: Vec::new(),
            grid: None,
            notes: vec![notes.join("; ")],
        })
    }
}

/// A page's name for a spectrum: "Scan N" from an id with `scan=N` (or an mzXML scan
/// number), else "Spectrum N" by position.
fn spectrum_name(kind: Kind, e: &Entry, i: usize) -> String {
    if kind == Kind::Mzxml && !e.id.is_empty() {
        return format!("Scan {}", e.id);
    }
    if let Some(n) =
        e.id.split_whitespace()
            .find_map(|t| t.strip_prefix("scan="))
            .filter(|n| !n.is_empty())
    {
        return format!("Scan {n}");
    }
    format!("Spectrum {}", i + 1)
}

fn short(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".into()
    } else {
        s.to_string()
    }
}

fn chromatogram_plot(r: &Record, title: String) -> Plot {
    let n = r.x.len().min(r.y.len());
    let x: Vec<f64> = r.x[..n].iter().map(|v| v * r.x_to_minutes).collect();
    let mut notes = Vec::new();
    if let Some(k) = &r.kind_name {
        notes.push(k.clone());
    }
    if let Some(p) = r.precursor_mz {
        notes.push(format!("Precursor m/z {}", short(p)));
    }
    Plot {
        title,
        x: Axis::new("Retention time", "min", false),
        y: Axis::new("Intensity", "", false),
        traces: vec![Trace {
            name: String::new(),
            x,
            y: r.y[..n].to_vec(),
            sticks: false,
        }],
        peaks: Vec::new(),
        grid: None,
        // Each note on its own: the app joins them in its direction.
        notes,
    }
}

/// The highest local maxima, at most `n`, apart by at least 1 % of the x range.
fn top_peaks(x: &[f64], y: &[f64], n: usize) -> Vec<Peak> {
    let len = x.len().min(y.len());
    if len == 0 {
        return Vec::new();
    }
    let (lo, hi) = x[..len]
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    let gap = (hi - lo).abs() * 0.01;
    let mut cand: Vec<usize> = (0..len)
        .filter(|&i| {
            let l = if i > 0 { y[i - 1] } else { f64::NEG_INFINITY };
            let r = if i + 1 < len {
                y[i + 1]
            } else {
                f64::NEG_INFINITY
            };
            y[i] > 0.0 && y[i] >= l && y[i] >= r
        })
        .collect();
    cand.sort_by(|&a, &b| y[b].total_cmp(&y[a]).then(a.cmp(&b)));
    let mut out: Vec<Peak> = Vec::new();
    for i in cand {
        if out.len() >= n {
            break;
        }
        if out.iter().all(|p| (p.x - x[i]).abs() > gap) {
            out.push(Peak {
                x: x[i],
                y: y[i],
                label: short(x[i]),
            });
        }
    }
    out
}

fn spectrum_plot(r: &Record, title: String) -> Plot {
    let n = r.x.len().min(r.y.len());
    let x_axis = match r.x_kind {
        XKind::Wavelength => Axis::new("Wavelength", "nm", false),
        XKind::Time => Axis::new("Time", "min", false),
        _ => Axis::new("m/z", "", false),
    };
    let x: Vec<f64> = if r.x_kind == XKind::Time {
        r.x[..n].iter().map(|v| v * r.x_to_minutes).collect()
    } else {
        r.x[..n].to_vec()
    };
    let y = r.y[..n].to_vec();
    let mut notes = Vec::new();
    if let Some(l) = r.ms_level {
        notes.push(format!("MS{l}"));
    }
    if let Some(t) = r.rt_min {
        notes.push(format!("RT {} min", short(t)));
    }
    match r.polarity {
        Some(Polarity::Positive) => notes.push("Positive".into()),
        Some(Polarity::Negative) => notes.push("Negative".into()),
        None => {}
    }
    if let Some(p) = r.precursor_mz {
        let mut s = format!("Precursor m/z {}", short(p));
        if let Some(z) = r.charge {
            let sign = if z < 0 { '−' } else { '+' };
            s.push_str(&format!(", charge {}{sign}", z.unsigned_abs()));
        }
        notes.push(s);
    }
    if let Some(b) = r.base_mz {
        notes.push(format!("Base peak m/z {}", short(b)));
    }
    match r.centroid {
        Some(true) => notes.push("Centroided".into()),
        Some(false) => notes.push("Profile".into()),
        None => {}
    }
    let peaks = if r.x_kind == XKind::Mz {
        top_peaks(&x, &y, 5)
    } else {
        Vec::new()
    };
    Plot {
        title,
        x: x_axis,
        y: Axis::new("Intensity", "", false),
        traces: vec![Trace {
            name: String::new(),
            x,
            y,
            sticks: r.centroid == Some(true),
        }],
        peaks,
        grid: None,
        // Each note on its own: the app joins them in its direction.
        notes,
    }
}

impl SpectraFile for MzFile {
    fn facts(&self) -> &Facts {
        &self.facts
    }

    fn page_count(&self) -> usize {
        usize::from(self.overview.is_some()) + self.chrom_pages.len() + self.spectra.len()
    }

    fn page_name(&self, page: usize) -> String {
        match self.page_kind(page) {
            Some(PageKind::Overview(_)) => "TIC".into(),
            Some(PageKind::Chromatogram(c)) => self.chromatograms[c].id.clone(),
            Some(PageKind::Spectrum(s)) => spectrum_name(self.kind, &self.spectra[s], s),
            None => String::new(),
        }
    }

    fn page(&self, page: usize) -> Result<Plot, Error> {
        let name = self.page_name(page);
        match self.page_kind(page) {
            Some(PageKind::Overview(Overview::Chromatogram(c))) => {
                Ok(chromatogram_plot(&self.chromatogram(c)?, name))
            }
            Some(PageKind::Overview(Overview::Built)) => {
                self.built_tic.get_or_init(|| self.build_tic()).clone()
            }
            Some(PageKind::Chromatogram(c)) => Ok(chromatogram_plot(&self.chromatogram(c)?, name)),
            Some(PageKind::Spectrum(s)) => Ok(spectrum_plot(&self.spectrum(s)?, name)),
            None => Err(Error::Malformed(format!("no page {page}"))),
        }
    }
}

#[cfg(test)]
mod tests;
