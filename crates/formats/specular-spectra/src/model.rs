//! The model every reader produces: a file's [`Facts`] and its pages, each a [`Plot`].
//!
//! Readers keep values as stored (f64), in the file's own units, with the axis titles in
//! English ("Wavenumber", "Chemical shift"). [`crate::chart`] turns a plot into a sheet the
//! viewer draws.

use crate::Error;

/// What was measured; it decides the default look (an IR x axis runs right to left).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Technique {
    Infrared,
    Raman,
    UvVis,
    Fluorescence,
    Nmr,
    Mass,
    Chromatogram,
    #[default]
    Other,
}

impl Technique {
    /// The English name ("Infrared").
    pub fn english(self) -> &'static str {
        match self {
            Technique::Infrared => "Infrared",
            Technique::Raman => "Raman",
            Technique::UvVis => "UV-Vis",
            Technique::Fluorescence => "Fluorescence",
            Technique::Nmr => "NMR",
            Technique::Mass => "Mass spectrum",
            Technique::Chromatogram => "Chromatogram",
            Technique::Other => "Spectrum",
        }
    }
}

/// One axis: what it shows (English), its unit as written ("cm⁻¹", "ppm", "m/z"; may be
/// empty) and whether values grow to the left (wavenumbers, chemical shifts) or downwards.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Axis {
    pub title: String,
    pub unit: String,
    pub reversed: bool,
}

impl Axis {
    pub fn new(title: impl Into<String>, unit: impl Into<String>, reversed: bool) -> Axis {
        Axis {
            title: title.into(),
            unit: unit.into(),
            reversed,
        }
    }
}

/// One plotted series: points in x order as stored (x may run either way).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace {
    /// Shown in a legend when a plot has several; may be empty.
    pub name: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    /// Drawn as vertical sticks from zero (centroided mass spectra, peak tables) rather
    /// than a line.
    pub sticks: bool,
}

/// A 2-D spectrum sampled on a regular grid (2D NMR): row-major, `width` columns along x.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    pub z: Vec<f32>,
    /// The x values of the first and last column, and the y values of the first and last
    /// row (either may run downwards, as ppm do).
    pub x_range: (f64, f64),
    pub y_range: (f64, f64),
}

/// A labelled point: a peak from a peak table or a peak pick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Peak {
    pub x: f64,
    pub y: f64,
    /// Shown beside the marker (the x value, an assignment); may be empty.
    pub label: String,
}

/// What one page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plot {
    /// The page's name ("Spectrum", "TIC", "Scan 1532", "1r"); English.
    pub title: String,
    pub x: Axis,
    pub y: Axis,
    pub traces: Vec<Trace>,
    pub peaks: Vec<Peak>,
    /// A 2-D spectrum, drawn as contours under the traces.
    pub grid: Option<Grid>,
    /// Said with the plot, in English: how it was processed ("Phase corrected
    /// automatically: 0th order 12.3°, 1st order −4.0°"), what was left out.
    pub notes: Vec<String>,
}

/// What a file says about itself, for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    /// "JCAMP-DX", "SPC", "mzML", "mzXML", "Bruker NMR".
    pub format: String,
    /// The format's version as stored ("5.01", "new format", "1.1.0", "TopSpin 4.1").
    pub version: String,
    pub technique: Technique,
    pub title: Option<String>,
    /// Owner, origin or operator.
    pub origin: Option<String>,
    pub instrument: Option<String>,
    /// As stored.
    pub date: Option<String>,
    /// Spectra in the file and points in the first (or the largest).
    pub spectra: u64,
    pub points: u64,
    /// NMR: observed nucleus ("1H"), spectrometer frequency (MHz), solvent, pulse program.
    pub nucleus: Option<String>,
    pub frequency_mhz: Option<f64>,
    pub solvent: Option<String>,
    pub pulse_program: Option<String>,
    /// Other labelled fields as stored (JCAMP `##LABEL=`, acqus parameters, mzML cvParams
    /// worth showing): label and value, in file order, deduplicated.
    pub fields: Vec<(String, String)>,
    /// Problems that did not stop reading, deduplicated, English.
    pub warnings: Vec<String>,
}

/// A file with its pages. Readers that hold every page return [`Spectra`]; large files
/// (mzML) read a page when it is asked for.
pub trait SpectraFile: Send + Sync {
    fn facts(&self) -> &Facts;
    fn page_count(&self) -> usize;
    /// The page's name, without reading it ("TIC", "Scan 1532").
    fn page_name(&self, page: usize) -> String;
    fn page(&self, page: usize) -> Result<Plot, Error>;
}

/// A file read whole.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spectra {
    pub facts: Facts,
    pub pages: Vec<Plot>,
}

impl SpectraFile for Spectra {
    fn facts(&self) -> &Facts {
        &self.facts
    }
    fn page_count(&self) -> usize {
        self.pages.len()
    }
    fn page_name(&self, page: usize) -> String {
        self.pages
            .get(page)
            .map(|p| p.title.clone())
            .unwrap_or_default()
    }
    fn page(&self, page: usize) -> Result<Plot, Error> {
        self.pages
            .get(page)
            .cloned()
            .ok_or_else(|| Error::Malformed(format!("no page {page}")))
    }
}

/// Adds `w` to `warnings` unless it is there already.
pub fn warn(warnings: &mut Vec<String>, w: impl Into<String>) {
    let w = w.into();
    if !warnings.contains(&w) && warnings.len() < 64 {
        warnings.push(w);
    }
}
