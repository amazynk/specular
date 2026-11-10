//! The medical summary for the info panel. Deliberately holds no patient-identifying
//! attributes (names, IDs, birth dates, addresses, institution or physician names, accession
//! numbers, dates and times of the study): only what describes the image.

use crate::charset::Charset;
use crate::display::functional;
use crate::parse::{DataSet, Element, Values};

/// What the info panel shows about a DICOM image.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    /// Modality (0008,0060), e.g. "CT".
    pub modality: Option<String>,
    /// Body Part Examined (0018,0015).
    pub body_part: Option<String>,
    pub study_description: Option<String>,
    pub series_description: Option<String>,
    /// Slice Thickness (0018,0050), millimetres.
    pub slice_thickness: Option<f64>,
    /// Pixel Spacing (0028,0030), millimetres: (row spacing, column spacing), in the patient.
    pub pixel_spacing: Option<(f64, f64)>,
    /// Imager Pixel Spacing (0018,1164), millimetres at the detector (projection images;
    /// not corrected for magnification).
    pub imager_pixel_spacing: Option<(f64, f64)>,
    pub columns: u32,
    pub rows: u32,
    pub frames: u32,
    pub bits_stored: u16,
    pub photometric: String,
    /// Transfer syntax name, e.g. "JPEG Lossless, first-order prediction".
    pub transfer_syntax: String,
    /// The pixel data has been lossy compressed (by this syntax or before).
    pub lossy: bool,
    /// An industrial inspection (DICONDE, ASTM E2339) rather than a patient's image.
    pub nde: Option<Nde>,
    /// The first frame's calibration for measurements (the spacing and its kind, or the
    /// ultrasound regions).
    pub calibration: crate::calib::Calibration,
    /// Pixel Aspect Ratio (0028,0034): (vertical, horizontal) size of a pixel.
    pub pixel_aspect_ratio: Option<(f64, f64)>,
}

impl Summary {
    /// How many times as tall as wide a stored pixel is shown: its row spacing over its
    /// column spacing when the image states a spacing, else its Pixel Aspect Ratio, else 1.
    /// Ultrasound regions may differ from each other: the Pixel Aspect Ratio decides there.
    pub fn pixel_aspect(&self) -> f64 {
        let a = match (self.calibration.spacing, self.pixel_spacing) {
            (Some(s), _) => s.y / s.x,
            // Volumes read by other decoders state only Pixel Spacing (row, column).
            (None, Some((r, c))) if self.calibration.regions.is_empty() => r / c,
            _ => self.pixel_aspect_ratio.map_or(1.0, |(v, h)| v / h),
        };
        // Within a part in a thousand is square (rounded spacings).
        if a.is_finite() && a > 0.0 && (a - 1.0).abs() > 1e-3 {
            a.clamp(0.1, 10.0)
        } else {
            1.0
        }
    }
}

/// What a DICONDE file says of the inspected component and the inspection (group 0014),
/// told apart from a medical image by that group or by a DICONDE SOP class. The component's
/// name and ID (which DICONDE keeps in the patient's name and ID) and evaluator, reviewer and
/// welder names and IDs are left out, as patient attributes are.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nde {
    /// Component Shape (0014,0050), e.g. "PIPE", "PLATE".
    pub component_shape: Option<String>,
    /// Component Manufacturer (0014,0028).
    pub component_manufacturer: Option<String>,
    /// Material Grade (0014,0042).
    pub material_grade: Option<String>,
    /// Material Thickness (0014,0030), millimetres.
    pub material_thickness: Option<f64>,
    /// Material Pipe Diameter (0014,0032), millimetres.
    pub pipe_diameter: Option<f64>,
    /// Environmental Conditions (0014,1040).
    pub environment: Option<String>,
    /// Image Quality Indicator Type (0014,40A0), with its material (0014,40A1) when given.
    pub iqi: Option<String>,
    /// Number of indications recorded (Indication Sequence (0014,2012) items).
    pub indications: usize,
    /// Peak tube voltage (KVP (0018,0060)), kV, or LINAC energy (0014,5002), MeV, as a
    /// number and its unit.
    pub source: Option<(f64, &'static str)>,
}

/// DICONDE storage SOP classes (eddy current, thermography, ultrasound waveform) share this
/// prefix; DICONDE CT and radiography use the medical classes with group 0014 attributes.
const DICONDE_SOP_PREFIX: &str = "1.2.840.10008.5.1.4.1.1.601.";

fn nde(r: &Reader) -> Option<Nde> {
    let ds = r.ds;
    let sop = r.text(0x0008_0016).unwrap_or_default();
    let group = ds.elements.iter().any(|e| e.tag >> 16 == 0x0014);
    if !group && !sop.starts_with(DICONDE_SOP_PREFIX) {
        return None;
    }
    let mm = |tag| r.number(ds.get(tag)).filter(|x| x.is_finite() && *x > 0.0);
    let iqi = r.text(0x0014_40A0).map(|t| match r.text(0x0014_40A1) {
        Some(m) => format!("{t} ({m})"),
        None => t,
    });
    let source = mm(0x0018_0060)
        .map(|kv| (kv, "kV"))
        .or_else(|| mm(0x0014_5002).map(|e| (e, "MeV")));
    Some(Nde {
        component_shape: r.text(0x0014_0050),
        component_manufacturer: r.text(0x0014_0028),
        material_grade: r.text(0x0014_0042),
        material_thickness: mm(0x0014_0030),
        pipe_diameter: mm(0x0014_0032),
        environment: r.text(0x0014_1040),
        iqi,
        indications: ds.get(0x0014_2012).map_or(0, |e| e.items.len()),
        source,
    })
}

pub(crate) struct Reader<'a> {
    pub ds: &'a DataSet,
    pub v: Values<'a>,
    pub cs: Charset,
}

impl Reader<'_> {
    pub fn text(&self, tag: u32) -> Option<String> {
        self.text_in(self.ds, tag)
    }

    pub fn text_in(&self, ds: &DataSet, tag: u32) -> Option<String> {
        let e = ds.get(tag)?;
        let s = self.cs.decode(self.v.text_bytes(e), e.vr);
        let s = s.trim().to_owned();
        (!s.is_empty()).then_some(s)
    }

    pub fn pair(&self, e: Option<&Element>) -> Option<(f64, f64)> {
        let e = e?;
        let a = self.v.number(e, 0)?;
        let b = self.v.number(e, 1).unwrap_or(a);
        (a > 0.0 && b > 0.0).then_some((a, b))
    }

    pub fn number(&self, e: Option<&Element>) -> Option<f64> {
        self.v.number(e?, 0)
    }
}

pub(crate) fn read(r: &Reader, frame0_measures: Option<&DataSet>) -> Summary {
    let ds = r.ds;
    let measures = frame0_measures.or_else(|| functional(ds, 0, 0x0028_9110));
    let pixel_spacing = r
        .pair(ds.get(0x0028_0030))
        .or_else(|| r.pair(measures.and_then(|m| m.get(0x0028_0030))));
    let slice_thickness = r
        .number(ds.get(0x0018_0050))
        .or_else(|| r.number(measures.and_then(|m| m.get(0x0018_0050))))
        .filter(|t| *t > 0.0);
    let lossy = r.text(0x0028_2110).as_deref() == Some("01");
    Summary {
        modality: r.text(0x0008_0060),
        body_part: r.text(0x0018_0015),
        study_description: r.text(0x0008_1030),
        series_description: r.text(0x0008_103E),
        slice_thickness,
        pixel_spacing,
        imager_pixel_spacing: r.pair(ds.get(0x0018_1164)),
        pixel_aspect_ratio: r.pair(ds.get(0x0028_0034)),
        calibration: crate::calib::Calibration::read(ds, &r.v, 0),
        lossy,
        nde: nde(r),
        ..Summary::default()
    }
}
