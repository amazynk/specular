//! Structured reports (PS3.3 C.17): the content tree of Basic, Enhanced, Comprehensive and
//! Comprehensive 3D SR documents (and the other SR storage classes built on the same module,
//! such as Key Object Selection, Measurement and X-Ray Radiation Dose reports).
//!
//! The root content item is the document itself (always a CONTAINER); every item carries a
//! relationship to its parent, a value type, a concept name and a value. Items added by
//! reference (Referenced Content Item Identifier) are kept as a pointer to the target's path
//! and resolved for display. TID 1500 measurement reports are recognised by their template
//! identifier or by their document title (LOINC 126000 / DCM 126000).
//!
//! Spatial coordinates are kept as written: SCOORD in image pixel columns and rows (the top
//! left corner of the top left pixel is 0,0) of the image item they are SELECTED FROM, and
//! SCOORD3D in patient mm in their frame of reference.

use crate::charset::Charset;
use crate::dict::Vr;
use crate::parse::{DataSet, Element};
use crate::{Dicom, Error, Result, Values};

const SOP_CLASS: u32 = 0x0008_0016;
const SOP_INSTANCE: u32 = 0x0008_0018;
const STUDY_UID: u32 = 0x0020_000D;
const CODE_VALUE: u32 = 0x0008_0100;
const CODING_SCHEME: u32 = 0x0008_0102;
const CODE_MEANING: u32 = 0x0008_0104;
const LONG_CODE_VALUE: u32 = 0x0008_0119;
const URN_CODE_VALUE: u32 = 0x0008_0120;
const REFERENCED_SOP_SEQUENCE: u32 = 0x0008_1199;
const REFERENCED_SOP_CLASS: u32 = 0x0008_1150;
const REFERENCED_SOP_INSTANCE: u32 = 0x0008_1155;
const REFERENCED_FRAMES: u32 = 0x0008_1160;
const REFERENCED_SEGMENTS: u32 = 0x0062_000B;
const MEASUREMENT_UNITS: u32 = 0x0040_08EA;
const RELATIONSHIP_TYPE: u32 = 0x0040_A010;
const VALUE_TYPE: u32 = 0x0040_A040;
const CONCEPT_NAME: u32 = 0x0040_A043;
const CONTINUITY: u32 = 0x0040_A050;
const DATETIME: u32 = 0x0040_A120;
const DATE: u32 = 0x0040_A121;
const TIME: u32 = 0x0040_A122;
const PERSON_NAME: u32 = 0x0040_A123;
const UID: u32 = 0x0040_A124;
const TEMPORAL_RANGE_TYPE: u32 = 0x0040_A130;
const SAMPLE_POSITIONS: u32 = 0x0040_A132;
const TIME_OFFSETS: u32 = 0x0040_A138;
const DATETIMES: u32 = 0x0040_A13A;
const TEXT_VALUE: u32 = 0x0040_A160;
const FLOATING_POINT: u32 = 0x0040_A161;
const RATIONAL_NUMERATOR: u32 = 0x0040_A162;
const RATIONAL_DENOMINATOR: u32 = 0x0040_A163;
const CONCEPT_CODE: u32 = 0x0040_A168;
const MEASURED_VALUE: u32 = 0x0040_A300;
const NUMERIC_QUALIFIER: u32 = 0x0040_A301;
const NUMERIC_VALUE: u32 = 0x0040_A30A;
const COMPLETION: u32 = 0x0040_A491;
const VERIFICATION: u32 = 0x0040_A493;
const TEMPLATE_SEQUENCE: u32 = 0x0040_A504;
const CONTENT_SEQUENCE: u32 = 0x0040_A730;
const TEMPLATE_ID: u32 = 0x0040_DB00;
const REFERENCED_CONTENT_ITEM: u32 = 0x0040_DB73;
const GRAPHIC_DATA: u32 = 0x0070_0022;
const GRAPHIC_TYPE: u32 = 0x0070_0023;
const FRAME_OF_REFERENCE_REF: u32 = 0x3006_0024;
const TABLE_ROWS: u32 = 0x0040_A600;

/// Most content items read (a sanity cap far above real reports).
const MAX_ITEMS: usize = 1 << 18;

/// The SR storage classes and their names.
const CLASSES: &[(&str, Kind)] = &[
    ("1.2.840.10008.5.1.4.1.1.88.11", Kind::Basic),
    ("1.2.840.10008.5.1.4.1.1.88.22", Kind::Enhanced),
    ("1.2.840.10008.5.1.4.1.1.88.33", Kind::Comprehensive),
    ("1.2.840.10008.5.1.4.1.1.88.34", Kind::Comprehensive3d),
    ("1.2.840.10008.5.1.4.1.1.88.35", Kind::Extensible),
    ("1.2.840.10008.5.1.4.1.1.88.40", Kind::Procedure),
    ("1.2.840.10008.5.1.4.1.1.88.50", Kind::Mammography),
    ("1.2.840.10008.5.1.4.1.1.88.59", Kind::KeyObjects),
    ("1.2.840.10008.5.1.4.1.1.88.65", Kind::Chest),
    ("1.2.840.10008.5.1.4.1.1.88.67", Kind::RadiationDose),
    (
        "1.2.840.10008.5.1.4.1.1.88.68",
        Kind::RadiopharmaceuticalDose,
    ),
    ("1.2.840.10008.5.1.4.1.1.88.69", Kind::ColonCad),
    ("1.2.840.10008.5.1.4.1.1.88.70", Kind::ImplantationPlan),
    ("1.2.840.10008.5.1.4.1.1.88.71", Kind::AcquisitionContext),
    ("1.2.840.10008.5.1.4.1.1.88.72", Kind::Simplified),
    ("1.2.840.10008.5.1.4.1.1.88.73", Kind::PatientDose),
    ("1.2.840.10008.5.1.4.1.1.88.74", Kind::PlannedImagingAgent),
    ("1.2.840.10008.5.1.4.1.1.88.75", Kind::PerformedImagingAgent),
    (
        "1.2.840.10008.5.1.4.1.1.88.76",
        Kind::EnhancedXRayRadiationDose,
    ),
];

/// The SR storage class of a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Basic,
    Enhanced,
    Comprehensive,
    Comprehensive3d,
    Extensible,
    Procedure,
    Mammography,
    KeyObjects,
    Chest,
    RadiationDose,
    RadiopharmaceuticalDose,
    ColonCad,
    ImplantationPlan,
    AcquisitionContext,
    Simplified,
    PatientDose,
    PlannedImagingAgent,
    PerformedImagingAgent,
    EnhancedXRayRadiationDose,
}

impl Kind {
    /// English name of the storage class (DICOM terms, not translated).
    pub fn name(self) -> &'static str {
        match self {
            Kind::Basic => "Basic Text SR",
            Kind::Enhanced => "Enhanced SR",
            Kind::Comprehensive => "Comprehensive SR",
            Kind::Comprehensive3d => "Comprehensive 3D SR",
            Kind::Extensible => "Extensible SR",
            Kind::Procedure => "Procedure Log",
            Kind::Mammography => "Mammography CAD SR",
            Kind::KeyObjects => "Key Object Selection",
            Kind::Chest => "Chest CAD SR",
            Kind::RadiationDose => "X-Ray Radiation Dose SR",
            Kind::RadiopharmaceuticalDose => "Radiopharmaceutical Radiation Dose SR",
            Kind::ColonCad => "Colon CAD SR",
            Kind::ImplantationPlan => "Implantation Plan SR",
            Kind::AcquisitionContext => "Acquisition Context SR",
            Kind::Simplified => "Simplified Adult Echo SR",
            Kind::PatientDose => "Patient Radiation Dose SR",
            Kind::PlannedImagingAgent => "Planned Imaging Agent Administration SR",
            Kind::PerformedImagingAgent => "Performed Imaging Agent Administration SR",
            Kind::EnhancedXRayRadiationDose => "Enhanced X-Ray Radiation Dose SR",
        }
    }
}

/// The SR storage class of `sop_class`, if it is one.
pub fn kind_of(sop_class: &str) -> Option<Kind> {
    CLASSES
        .iter()
        .find(|(uid, _)| *uid == sop_class)
        .map(|&(_, k)| k)
}

/// A coded concept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Code {
    pub value: String,
    pub scheme: String,
    pub meaning: String,
}

impl Code {
    /// The meaning, or the code value when the meaning is empty.
    pub fn label(&self) -> &str {
        if self.meaning.is_empty() {
            &self.value
        } else {
            &self.meaning
        }
    }

    /// Whether this is code `value` in `scheme`.
    pub fn is(&self, scheme: &str, value: &str) -> bool {
        self.scheme == scheme && self.value == value
    }

    /// The unit symbol for a UCUM unit (its code value: "mm", "cm2", "ml"), the meaning for
    /// other schemes, empty for the UCUM unity "1" and for UCUM annotations ("{ratio}").
    pub fn unit_symbol(&self) -> String {
        if self.scheme == "UCUM" {
            if self.value == "1" || (self.value.starts_with('{') && self.value.ends_with('}')) {
                String::new()
            } else {
                self.value.clone()
            }
        } else {
            self.label().to_owned()
        }
    }
}

/// A reference to a composite object (an image, a waveform, a segmentation…).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SopRef {
    pub sop_class: String,
    pub sop_instance: String,
    /// Referenced frames (1-based), empty for all.
    pub frames: Vec<u32>,
    /// Referenced segments, empty for all.
    pub segments: Vec<u32>,
}

/// A numeric measurement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measurement {
    /// The Numeric Value as written (DS text), empty when there is none.
    pub text: String,
    /// The value: the Floating Point Value when present, else the Numeric Value.
    pub value: Option<f64>,
    pub unit: Option<Code>,
    /// Why there is no value (Numeric Value Qualifier), or a qualifier of the value.
    pub qualifier: Option<Code>,
}

impl Measurement {
    /// The value and unit as text ("12.5 mm"), or the qualifier when there is no value.
    pub fn display(&self) -> String {
        let unit = self
            .unit
            .as_ref()
            .map(Code::unit_symbol)
            .unwrap_or_default();
        let number = if !self.text.is_empty() {
            self.text.clone()
        } else if let Some(v) = self.value {
            format_number(v)
        } else {
            return self
                .qualifier
                .as_ref()
                .map(|q| q.label().to_owned())
                .unwrap_or_default();
        };
        if unit.is_empty() {
            number
        } else {
            format!("{number} {unit}")
        }
    }
}

/// A shortest round-trip decimal for `v` (no trailing zeros).
pub fn format_number(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// The value of a content item.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Container {
        /// SEPARATE (false) or CONTINUOUS (true).
        continuous: bool,
        /// Template Identifier (TID), when the container names one.
        template: Option<String>,
    },
    Text(String),
    Num(Measurement),
    Code(Code),
    DateTime(String),
    Date(String),
    Time(String),
    PersonName(String),
    UidRef(String),
    Image(SopRef),
    Composite(SopRef),
    Waveform(SopRef),
    /// Image coordinates (columns, rows), in the image the item is SELECTED FROM.
    Scoord {
        graphic_type: String,
        points: Vec<[f32; 2]>,
    },
    /// Patient coordinates (mm) in a frame of reference.
    Scoord3d {
        graphic_type: String,
        points: Vec<[f64; 3]>,
        frame_of_reference: String,
    },
    Tcoord {
        range_type: String,
        /// Sample positions, time offsets (s) or datetimes, whichever is present, as text.
        values: Vec<String>,
    },
    /// A table (TABLE value type): rows of cells, as text.
    Table(Vec<Vec<String>>),
    /// An item added by reference: the 1-based path of the target from the root.
    Reference(Vec<u32>),
    /// A value type this reader does not know.
    Other,
}

/// One content item and its children.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// Relationship to the parent (CONTAINS, HAS PROPERTIES, INFERRED FROM…); empty for the root.
    pub relationship: String,
    /// The value type as written (CONTAINER, NUM, CODE, TEXT…).
    pub value_type: String,
    pub concept: Option<Code>,
    pub value: Value,
    pub children: Vec<Node>,
}

impl Node {
    /// The concept name's meaning, or empty.
    pub fn name(&self) -> &str {
        self.concept.as_ref().map(Code::label).unwrap_or("")
    }

    /// The value as one line of text (empty for containers).
    pub fn display_value(&self) -> String {
        match &self.value {
            Value::Container { .. } | Value::Other => String::new(),
            Value::Text(s)
            | Value::DateTime(s)
            | Value::Date(s)
            | Value::Time(s)
            | Value::UidRef(s) => s.clone(),
            Value::PersonName(s) => person_name(s),
            Value::Num(m) => m.display(),
            Value::Code(c) => c.label().to_owned(),
            Value::Image(r) | Value::Composite(r) | Value::Waveform(r) => {
                let mut s = r.sop_instance.clone();
                if !r.frames.is_empty() {
                    s.push_str(&format!(
                        " [{}]",
                        r.frames
                            .iter()
                            .map(u32::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    ));
                }
                s
            }
            Value::Scoord {
                graphic_type,
                points,
            } => format!(
                "{graphic_type} {}",
                points
                    .iter()
                    .take(4)
                    .map(|p| format!(
                        "({}, {})",
                        format_number(p[0] as f64),
                        format_number(p[1] as f64)
                    ))
                    .collect::<Vec<_>>()
                    .join(" ")
                    + if points.len() > 4 { " …" } else { "" }
            ),
            Value::Scoord3d {
                graphic_type,
                points,
                ..
            } => format!(
                "{graphic_type} {}",
                points
                    .iter()
                    .take(3)
                    .map(|p| format!("({}, {}, {})", round2(p[0]), round2(p[1]), round2(p[2])))
                    .collect::<Vec<_>>()
                    .join(" ")
                    + if points.len() > 3 { " …" } else { "" }
            ),
            Value::Tcoord { range_type, values } => {
                format!("{range_type} {}", values.join(", "))
            }
            Value::Table(rows) => format!("{} rows", rows.len()),
            Value::Reference(path) => format!(
                "→ {}",
                path.iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(".")
            ),
        }
    }

    /// Number of items in this subtree.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Node::count).sum::<usize>()
    }

    /// The image this item points into: its own reference for IMAGE, else the IMAGE child a
    /// SCOORD is SELECTED FROM.
    pub fn image_ref(&self) -> Option<&SopRef> {
        match &self.value {
            Value::Image(r) => Some(r),
            _ => self.children.iter().find_map(|c| match &c.value {
                Value::Image(r)
                    if c.relationship == "SELECTED FROM" || c.relationship.is_empty() =>
                {
                    Some(r)
                }
                _ => None,
            }),
        }
    }

    /// Where a viewer can jump to show this item, if anywhere.
    pub fn jump(&self) -> Option<Jump> {
        match &self.value {
            Value::Scoord { points, .. } => {
                let r = self.image_ref()?;
                Some(Jump::Image {
                    sop_instance: r.sop_instance.clone(),
                    frame: r.frames.first().copied(),
                    point: centroid2(points),
                })
            }
            Value::Image(r) => Some(Jump::Image {
                sop_instance: r.sop_instance.clone(),
                frame: r.frames.first().copied(),
                point: None,
            }),
            Value::Scoord3d {
                points,
                frame_of_reference,
                ..
            } => {
                let c = centroid3(points)?;
                Some(Jump::Patient {
                    frame_of_reference: frame_of_reference.clone(),
                    point: c,
                })
            }
            _ => None,
        }
    }
}

/// A target of a jump from a report item.
#[derive(Clone, Debug, PartialEq)]
pub enum Jump {
    /// An image by SOP Instance UID, at a frame (1-based) and an image point (columns, rows).
    Image {
        sop_instance: String,
        frame: Option<u32>,
        point: Option<[f32; 2]>,
    },
    /// A point in patient mm in a frame of reference.
    Patient {
        frame_of_reference: String,
        point: [f64; 3],
    },
}

fn centroid2(p: &[[f32; 2]]) -> Option<[f32; 2]> {
    if p.is_empty() {
        return None;
    }
    let n = p.len() as f32;
    let (x, y) = p.iter().fold((0.0, 0.0), |(x, y), q| (x + q[0], y + q[1]));
    Some([x / n, y / n])
}

fn centroid3(p: &[[f64; 3]]) -> Option<[f64; 3]> {
    if p.is_empty() {
        return None;
    }
    let n = p.len() as f64;
    let mut c = [0.0; 3];
    for q in p {
        for k in 0..3 {
            c[k] += q[k] / n;
        }
    }
    Some(c)
}

fn round2(v: f64) -> String {
    format_number((v * 100.0).round() / 100.0)
}

/// A DICOM person name ("Family^Given^Middle^Prefix^Suffix", groups by "=") as
/// "Prefix Given Middle Family Suffix" of its first (alphabetic) group.
pub fn person_name(pn: &str) -> String {
    let group = pn.split('=').next().unwrap_or("");
    let c: Vec<&str> = group.split('^').map(str::trim).collect();
    let at = |i: usize| c.get(i).copied().unwrap_or("");
    [at(3), at(1), at(2), at(0), at(4)]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A structured report.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub kind: Kind,
    pub sop_instance: String,
    pub study_uid: String,
    /// The root container (the document title is its concept name).
    pub root: Node,
    /// Template of the root container ("1500" for a TID 1500 measurement report).
    pub template: Option<String>,
    /// Completion Flag (PARTIAL, COMPLETE).
    pub completion: String,
    /// Verification Flag (UNVERIFIED, VERIFIED).
    pub verification: String,
    /// Content items not read (past the cap).
    pub dropped: usize,
}

impl Report {
    /// The document title (the root's concept name).
    pub fn title(&self) -> &str {
        self.root.name()
    }

    /// Whether this is a TID 1500 Measurement Report.
    pub fn is_measurement_report(&self) -> bool {
        self.template.as_deref() == Some("1500")
            || self.root.concept.as_ref().is_some_and(|c| {
                c.is("DCM", "126000") || c.is("LN", "126000") || c.is("DCM", "126001")
            })
    }

    /// Reads the content tree of SR document `d` (parsed from `file`).
    pub fn read(d: &Dicom, file: &[u8]) -> Result<Report> {
        let ds = &d.dataset;
        let v = d.values(file);
        let cs = Charset::of(ds, &v);
        let r = Reader { v, cs };
        let sop_class = r.text(ds, SOP_CLASS);
        let kind = kind_of(&sop_class)
            .ok_or_else(|| Error::Unsupported("not a DICOM structured report".into()))?;
        if ds.get(VALUE_TYPE).is_none() && ds.get(CONTENT_SEQUENCE).is_none() {
            return Err(Error::Malformed("structured report without content"));
        }
        let mut budget = MAX_ITEMS;
        let mut dropped = 0;
        let root = r.node(ds, cs, &mut budget, &mut dropped);
        let template = match &root.value {
            Value::Container { template, .. } => template.clone(),
            _ => None,
        };
        Ok(Report {
            kind,
            sop_instance: r.text(ds, SOP_INSTANCE),
            study_uid: r.text(ds, STUDY_UID),
            template,
            completion: r.text(ds, COMPLETION),
            verification: r.text(ds, VERIFICATION),
            root: Node {
                relationship: String::new(),
                ..root
            },
            dropped,
        })
    }

    /// The node at a 1-based path from the root (an empty path is the root).
    pub fn at(&self, path: &[u32]) -> Option<&Node> {
        let mut n = &self.root;
        for &i in path {
            n = n.children.get((i as usize).checked_sub(1)?)?;
        }
        Some(n)
    }

    /// Every node with its 1-based path, depth first.
    pub fn walk(&self) -> Vec<(Vec<u32>, &Node)> {
        fn go<'a>(n: &'a Node, path: &mut Vec<u32>, out: &mut Vec<(Vec<u32>, &'a Node)>) {
            out.push((path.clone(), n));
            for (i, c) in n.children.iter().enumerate() {
                path.push(i as u32 + 1);
                go(c, path, out);
                path.pop();
            }
        }
        let mut out = Vec::new();
        go(&self.root, &mut Vec::new(), &mut out);
        out
    }

    /// Every composite object the report references (images, waveforms, segmentations…),
    /// deduplicated by SOP Instance UID.
    pub fn references(&self) -> Vec<&SopRef> {
        let mut out: Vec<&SopRef> = Vec::new();
        for (_, n) in self.walk() {
            if let Value::Image(r) | Value::Composite(r) | Value::Waveform(r) = &n.value
                && !out.iter().any(|o| o.sop_instance == r.sop_instance)
            {
                out.push(r);
            }
        }
        out
    }

    /// Number of NUM items (measurements).
    pub fn measurements(&self) -> usize {
        self.walk()
            .iter()
            .filter(|(_, n)| matches!(n.value, Value::Num(_)))
            .count()
    }

    /// The report as indented lines of text: (depth, line), the root at depth 0.
    pub fn lines(&self, show_names: bool) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        for (path, n) in self.walk() {
            let value = match &n.value {
                Value::PersonName(_) if !show_names => "•••".to_owned(),
                _ => n.display_value(),
            };
            let name = n.name();
            let line = match (name.is_empty(), value.is_empty()) {
                (false, false) => format!("{name}: {value}"),
                (false, true) => name.to_owned(),
                (true, false) => value,
                (true, true) => n.value_type.clone(),
            };
            out.push((path.len(), line));
        }
        out
    }
}

struct Reader<'a> {
    v: Values<'a>,
    cs: Charset,
}

impl Reader<'_> {
    fn text_cs(&self, ds: &DataSet, tag: u32, cs: &Charset) -> String {
        ds.get(tag)
            .map(|e| cs.decode(self.v.text_bytes(e), e.vr).trim().to_owned())
            .unwrap_or_default()
    }

    fn text(&self, ds: &DataSet, tag: u32) -> String {
        self.text_cs(ds, tag, &self.cs)
    }

    fn code(&self, it: &DataSet, cs: &Charset) -> Code {
        let mut value = self.text_cs(it, CODE_VALUE, cs);
        if value.is_empty() {
            value = self.text_cs(it, LONG_CODE_VALUE, cs);
        }
        if value.is_empty() {
            value = self.text_cs(it, URN_CODE_VALUE, cs);
        }
        Code {
            value,
            scheme: self.text_cs(it, CODING_SCHEME, cs),
            meaning: self.text_cs(it, CODE_MEANING, cs),
        }
    }

    fn code_in(&self, ds: &DataSet, tag: u32, cs: &Charset) -> Option<Code> {
        let it = ds.item(tag)?;
        let cs = cs.within(it, &self.v);
        Some(self.code(it, &cs))
    }

    fn uints(&self, e: &Element) -> Vec<u32> {
        match e.vr {
            Vr::IS | Vr::DS => (0..self.v.count(e))
                .filter_map(|i| self.v.number(e, i))
                .map(|x| x as u32)
                .collect(),
            Vr::UL | Vr::OL => self.v.u32s(e).collect(),
            _ => self.v.u16s(e).map(u32::from).collect(),
        }
    }

    fn sop_ref(&self, ds: &DataSet) -> SopRef {
        let Some(it) = ds.item(REFERENCED_SOP_SEQUENCE) else {
            return SopRef::default();
        };
        SopRef {
            sop_class: self.text(it, REFERENCED_SOP_CLASS),
            sop_instance: self.text(it, REFERENCED_SOP_INSTANCE),
            frames: it
                .get(REFERENCED_FRAMES)
                .map(|e| self.uints(e))
                .unwrap_or_default(),
            segments: it
                .get(REFERENCED_SEGMENTS)
                .map(|e| self.uints(e))
                .unwrap_or_default(),
        }
    }

    fn floats(&self, e: &Element) -> Vec<f64> {
        match e.vr {
            Vr::FL | Vr::OF => (0..e.value.len() / 4)
                .filter_map(|i| {
                    self.v.number(
                        &Element {
                            vr: Vr::FL,
                            ..e.clone()
                        },
                        i,
                    )
                })
                .collect(),
            Vr::FD | Vr::OD => (0..e.value.len() / 8)
                .filter_map(|i| {
                    self.v.number(
                        &Element {
                            vr: Vr::FD,
                            ..e.clone()
                        },
                        i,
                    )
                })
                .collect(),
            _ => (0..self.v.count(e))
                .filter_map(|i| self.v.number(e, i))
                .collect(),
        }
    }

    fn measurement(&self, ds: &DataSet, cs: &Charset) -> Measurement {
        let mut m = Measurement {
            qualifier: self.code_in(ds, NUMERIC_QUALIFIER, cs),
            ..Measurement::default()
        };
        if let Some(it) = ds.item(MEASURED_VALUE) {
            let cs = cs.within(it, &self.v);
            m.text = self.text_cs(it, NUMERIC_VALUE, &cs);
            m.value = it
                .get(FLOATING_POINT)
                .and_then(|e| self.v.number(e, 0))
                .or_else(|| {
                    let n = it.get(RATIONAL_NUMERATOR).and_then(|e| self.v.int(e))?;
                    let d = it.get(RATIONAL_DENOMINATOR).and_then(|e| self.v.int(e))?;
                    (d != 0).then(|| n as f64 / d as f64)
                })
                .or_else(|| it.get(NUMERIC_VALUE).and_then(|e| self.v.number(e, 0)));
            m.unit = self.code_in(it, MEASUREMENT_UNITS, &cs);
            if m.qualifier.is_none() {
                m.qualifier = self.code_in(it, NUMERIC_QUALIFIER, &cs);
            }
        }
        m
    }

    fn value(&self, ds: &DataSet, vt: &str, cs: &Charset) -> Value {
        match vt {
            "CONTAINER" => Value::Container {
                continuous: self.text(ds, CONTINUITY) == "CONTINUOUS",
                template: ds
                    .item(TEMPLATE_SEQUENCE)
                    .map(|t| self.text(t, TEMPLATE_ID))
                    .filter(|s| !s.is_empty()),
            },
            "TEXT" => Value::Text(self.text_cs(ds, TEXT_VALUE, cs)),
            "NUM" => Value::Num(self.measurement(ds, cs)),
            "CODE" => Value::Code(self.code_in(ds, CONCEPT_CODE, cs).unwrap_or_default()),
            "DATETIME" => Value::DateTime(self.text(ds, DATETIME)),
            "DATE" => Value::Date(self.text(ds, DATE)),
            "TIME" => Value::Time(self.text(ds, TIME)),
            "PNAME" => Value::PersonName(self.text_cs(ds, PERSON_NAME, cs)),
            "UIDREF" => Value::UidRef(self.text(ds, UID)),
            "IMAGE" => Value::Image(self.sop_ref(ds)),
            "COMPOSITE" => Value::Composite(self.sop_ref(ds)),
            "WAVEFORM" => Value::Waveform(self.sop_ref(ds)),
            "SCOORD" => {
                let data = ds
                    .get(GRAPHIC_DATA)
                    .map(|e| self.floats(e))
                    .unwrap_or_default();
                Value::Scoord {
                    graphic_type: self.text(ds, GRAPHIC_TYPE),
                    points: data
                        .chunks_exact(2)
                        .map(|c| [c[0] as f32, c[1] as f32])
                        .collect(),
                }
            }
            "SCOORD3D" => {
                let data = ds
                    .get(GRAPHIC_DATA)
                    .map(|e| self.floats(e))
                    .unwrap_or_default();
                Value::Scoord3d {
                    graphic_type: self.text(ds, GRAPHIC_TYPE),
                    points: data.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
                    frame_of_reference: self.text(ds, FRAME_OF_REFERENCE_REF),
                }
            }
            "TCOORD" => {
                let values = if let Some(e) = ds.get(SAMPLE_POSITIONS) {
                    self.uints(e).iter().map(u32::to_string).collect()
                } else if let Some(e) = ds.get(TIME_OFFSETS) {
                    self.floats(e).into_iter().map(format_number).collect()
                } else if let Some(e) = ds.get(DATETIMES) {
                    (0..self.v.count(e))
                        .filter_map(|i| self.v.text_n(e, i).map(str::to_owned))
                        .collect()
                } else {
                    Vec::new()
                };
                Value::Tcoord {
                    range_type: self.text(ds, TEMPORAL_RANGE_TYPE),
                    values,
                }
            }
            "TABLE" => Value::Table(
                ds.get(TABLE_ROWS)
                    .map(|e| {
                        e.items
                            .iter()
                            .map(|row| {
                                row.elements
                                    .iter()
                                    .map(|c| {
                                        cs.decode(self.v.text_bytes(c), c.vr).trim().to_owned()
                                    })
                                    .collect()
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
            _ => Value::Other,
        }
    }

    fn node(&self, ds: &DataSet, cs: Charset, budget: &mut usize, dropped: &mut usize) -> Node {
        let cs = cs.within(ds, &self.v);
        let relationship = self.text(ds, RELATIONSHIP_TYPE);
        if let Some(e) = ds.get(REFERENCED_CONTENT_ITEM)
            && ds.get(VALUE_TYPE).is_none()
        {
            let path = self.uints(e);
            return Node {
                relationship,
                value_type: String::new(),
                concept: None,
                value: Value::Reference(path),
                children: Vec::new(),
            };
        }
        let value_type = self.text(ds, VALUE_TYPE);
        let value = self.value(ds, &value_type, &cs);
        let mut children = Vec::new();
        for it in ds
            .get(CONTENT_SEQUENCE)
            .map(|e| e.items.as_slice())
            .unwrap_or_default()
        {
            if *budget == 0 {
                *dropped += 1;
                continue;
            }
            *budget -= 1;
            children.push(self.node(it, cs, budget, dropped));
        }
        Node {
            relationship,
            value_type,
            concept: self.code_in(ds, CONCEPT_NAME, &cs),
            value,
            children,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn person_names_read_in_order() {
        assert_eq!(person_name("Doe^Jane^Q^Dr^PhD"), "Dr Jane Q Doe PhD");
        assert_eq!(person_name("Doe^John"), "John Doe");
        assert_eq!(person_name("Smith=スミス"), "Smith");
        assert_eq!(person_name(""), "");
    }

    #[test]
    fn units_show_ucum_symbols() {
        let c = |v: &str, s: &str, m: &str| Code {
            value: v.into(),
            scheme: s.into(),
            meaning: m.into(),
        };
        assert_eq!(c("mm", "UCUM", "millimeter").unit_symbol(), "mm");
        assert_eq!(c("1", "UCUM", "no units").unit_symbol(), "");
        assert_eq!(c("{ratio}", "UCUM", "ratio").unit_symbol(), "");
        assert_eq!(
            c("[hnsf'U]", "UCUM", "Hounsfield unit").unit_symbol(),
            "[hnsf'U]"
        );
        assert_eq!(c("X", "99L", "widgets").unit_symbol(), "widgets");
        let m = Measurement {
            text: "12.50".into(),
            value: Some(12.5),
            unit: Some(c("mm", "UCUM", "mm")),
            qualifier: None,
        };
        assert_eq!(m.display(), "12.50 mm");
        let q = Measurement {
            qualifier: Some(c("114006", "DCM", "Measurement failure")),
            ..Measurement::default()
        };
        assert_eq!(q.display(), "Measurement failure");
    }

    #[test]
    fn numbers_format_shortest() {
        assert_eq!(format_number(3.0), "3");
        assert_eq!(format_number(-2.5), "-2.5");
        assert_eq!(format_number(0.1), "0.1");
    }

    #[test]
    fn classes_are_known() {
        assert_eq!(
            kind_of("1.2.840.10008.5.1.4.1.1.88.34"),
            Some(Kind::Comprehensive3d)
        );
        assert_eq!(kind_of("1.2.840.10008.5.1.4.1.1.2"), None);
    }
}
