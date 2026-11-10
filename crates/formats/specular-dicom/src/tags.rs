//! The tag browser's view of a data set (DESIGN §7.4): every element, sequences with their
//! items, as rows in document order with their depth and subtree size, so a list can expand
//! and collapse them. Values are short previews; patient-identifying attributes are marked
//! ([`TagRow::identifying`]) so the browser can keep them hidden until asked.

use crate::Dicom;
use crate::charset::Charset;
use crate::dict::{self, Vr};
use crate::parse::{DataSet, Element, Values};

/// Most rows built for one file (enhanced multi-frame files repeat their functional groups
/// per frame); the rest are counted in [`TagTree::omitted`].
pub const MAX_ROWS: usize = 250_000;
/// Characters of a text value kept for its preview.
const PREVIEW_CHARS: usize = 96;
/// Numbers of a binary numeric value kept for its preview.
const PREVIEW_NUMBERS: usize = 12;
/// Bytes of a binary value kept for its preview.
const PREVIEW_BYTES: usize = 16;

/// What a row shows in its value column.
#[derive(Clone, Debug, PartialEq)]
pub enum TagValue {
    /// Text or numbers, already shortened; `more`: the value went on.
    Text {
        text: String,
        more: bool,
    },
    /// Binary data: the first bytes in hex, and the length.
    Binary {
        hex: String,
        len: u64,
    },
    /// A sequence of `n` items.
    Items(usize),
    /// Encapsulated pixel data of `n` fragments.
    Fragments(usize),
    /// An item of a sequence (1-based).
    Item(usize),
    Empty,
}

/// One row of the tree.
#[derive(Clone, Debug, PartialEq)]
pub struct TagRow {
    /// Nesting: 0 for the file's elements, 1 for an item of a top-level sequence, 2 for its
    /// elements and so on.
    pub depth: u16,
    /// The element's tag (`0` for item rows).
    pub tag: u32,
    /// The dictionary keyword (empty for private and unknown tags, and items).
    pub keyword: &'static str,
    /// The VR (empty for items).
    pub vr: &'static str,
    /// Value length in bytes (items for sequences).
    pub length: u64,
    pub value: TagValue,
    /// Rows below this one (its items and their elements).
    pub descendants: u32,
    /// Patient-identifying, by itself or inside such a sequence.
    pub identifying: bool,
    /// From the file meta information (group 0002).
    pub meta: bool,
}

impl TagRow {
    /// `(gggg,eeee)`.
    pub fn tag_text(&self) -> String {
        format!("({:04X},{:04X})", self.tag >> 16, self.tag & 0xFFFF)
    }
}

/// The rows of a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagTree {
    pub rows: Vec<TagRow>,
    /// Rows not built past [`MAX_ROWS`].
    pub omitted: usize,
}

impl TagTree {
    /// Rows of the file meta information and the data set of `d` (parsed from `file`).
    pub fn build(d: &Dicom, file: &[u8]) -> TagTree {
        let mut t = TagTree::default();
        let meta_v = Values {
            buf: file,
            big: false,
        };
        let v = d.values(file);
        let cs = Charset::of(&d.dataset, &v);
        let mut b = Builder { t: &mut t, cs };
        b.dataset(&d.meta, &meta_v, 0, false, true);
        b.dataset(&d.dataset, &v, 0, false, false);
        t
    }

    /// Patient-identifying rows that are not inside another such row.
    pub fn identifying_count(&self) -> usize {
        let mut n = 0;
        let mut skip_to = 0;
        for (i, r) in self.rows.iter().enumerate() {
            if i < skip_to {
                continue;
            }
            if r.identifying {
                n += 1;
                skip_to = i + 1 + r.descendants as usize;
            }
        }
        n
    }
}

struct Builder<'a> {
    t: &'a mut TagTree,
    cs: Charset,
}

impl Builder<'_> {
    fn push(&mut self, r: TagRow) -> Option<usize> {
        if self.t.rows.len() >= MAX_ROWS {
            self.t.omitted += 1;
            return None;
        }
        self.t.rows.push(r);
        Some(self.t.rows.len() - 1)
    }

    fn dataset(&mut self, ds: &DataSet, v: &Values, depth: u16, hidden: bool, meta: bool) {
        for e in &ds.elements {
            self.element(e, v, depth, hidden, meta);
        }
    }

    fn element(&mut self, e: &Element, v: &Values, depth: u16, hidden: bool, meta: bool) {
        let identifying = hidden || identifying(e.tag, e.vr);
        let keyword = dict::keyword(e.tag).unwrap_or("");
        let (length, value) = if let Some(f) = &e.fragments {
            let len: u64 = f.items.iter().map(|r| r.len() as u64).sum();
            (len, TagValue::Fragments(f.items.len()))
        } else if e.vr == Vr::SQ {
            (e.items.len() as u64, TagValue::Items(e.items.len()))
        } else {
            (e.value.len() as u64, preview(e, v, self.cs))
        };
        let Some(at) = self.push(TagRow {
            depth,
            tag: e.tag,
            keyword,
            vr: vr_name(e.vr),
            length,
            value,
            descendants: 0,
            identifying,
            meta,
        }) else {
            return;
        };
        for (k, item) in e.items.iter().enumerate() {
            let Some(item_at) = self.push(TagRow {
                depth: depth + 1,
                tag: 0,
                keyword: "",
                vr: "",
                length: item.elements.len() as u64,
                value: TagValue::Item(k + 1),
                descendants: 0,
                identifying,
                meta,
            }) else {
                break;
            };
            // An item may carry its own Specific Character Set (PS3.5 §6.1.2.5.4).
            let outer = self.cs;
            self.cs = outer.within(item, v);
            self.dataset(item, v, depth + 2, identifying, meta);
            self.cs = outer;
            let n = self.t.rows.len() - item_at - 1;
            self.t.rows[item_at].descendants = n as u32;
        }
        let n = self.t.rows.len() - at - 1;
        self.t.rows[at].descendants = n as u32;
    }
}

/// Whether an element can identify the patient: the Patient group (0010), person names,
/// dates, the study's identifiers, times and places (the attributes the info panel's
/// summary leaves out), and text in private groups (vendors keep copies of these there).
pub fn identifying(tag: u32, vr: Vr) -> bool {
    let group = (tag >> 16) as u16;
    if group == 0x0010 || matches!(vr, Vr::PN | Vr::DA | Vr::DT) {
        return true;
    }
    if group & 1 == 1 && group > 0x0008 {
        return matches!(
            vr,
            Vr::LO
                | Vr::SH
                | Vr::LT
                | Vr::ST
                | Vr::UT
                | Vr::UC
                | Vr::PN
                | Vr::DA
                | Vr::DT
                | Vr::TM
                | Vr::UN
        );
    }
    IDENTIFYING.binary_search(&tag).is_ok()
}

/// Further identifying attributes (sorted): study, series, acquisition and content times;
/// accession number; institution, department, station, physicians and operators; the
/// study ID; admission, visit and order identifiers; referenced patients.
pub(crate) const IDENTIFYING: &[u32] = &[
    0x0008_0030, // StudyTime
    0x0008_0031, // SeriesTime
    0x0008_0032, // AcquisitionTime
    0x0008_0033, // ContentTime
    0x0008_0034, // OverlayTime
    0x0008_0035, // CurveTime
    0x0008_0050, // AccessionNumber
    0x0008_0080, // InstitutionName
    0x0008_0081, // InstitutionAddress
    0x0008_0082, // InstitutionCodeSequence
    0x0008_0090, // ReferringPhysicianName
    0x0008_0092, // ReferringPhysicianAddress
    0x0008_0094, // ReferringPhysicianTelephoneNumbers
    0x0008_0096, // ReferringPhysicianIdentificationSequence
    0x0008_1010, // StationName
    0x0008_1040, // InstitutionalDepartmentName
    0x0008_1048, // PhysiciansOfRecord
    0x0008_1049, // PhysiciansOfRecordIdentificationSequence
    0x0008_1050, // PerformingPhysicianName
    0x0008_1052, // PerformingPhysicianIdentificationSequence
    0x0008_1060, // NameOfPhysiciansReadingStudy
    0x0008_1062, // PhysiciansReadingStudyIdentificationSequence
    0x0008_1070, // OperatorsName
    0x0008_1072, // OperatorIdentificationSequence
    0x0008_1080, // AdmittingDiagnosesDescription
    0x0008_1084, // AdmittingDiagnosesCodeSequence
    0x0008_1120, // ReferencedPatientSequence
    0x0018_1000, // DeviceSerialNumber
    0x0018_1030, // ProtocolName (often carries names)
    0x0020_0010, // StudyID
    0x0020_4000, // ImageComments
    0x0032_1032, // RequestingPhysician
    0x0032_1033, // RequestingService
    0x0032_4000, // StudyComments
    0x0038_0010, // AdmissionID
    0x0038_0300, // CurrentPatientLocation
    0x0038_0400, // PatientInstitutionResidence
    0x0038_0500, // PatientState
    0x0040_0006, // ScheduledPerformingPhysicianName
    0x0040_0241, // PerformedStationAETitle
    0x0040_0242, // PerformedStationName
    0x0040_0243, // PerformedLocation
    0x0040_0244, // PerformedProcedureStepStartDate
    0x0040_0245, // PerformedProcedureStepStartTime
    0x0040_0253, // PerformedProcedureStepID
    0x0040_1001, // RequestedProcedureID
    0x0040_2016, // PlacerOrderNumberImagingServiceRequest
    0x0040_2017, // FillerOrderNumberImagingServiceRequest
    0x0040_A123, // PersonName
];

fn vr_name(vr: Vr) -> &'static str {
    use Vr::*;
    match vr {
        AE => "AE",
        AS => "AS",
        AT => "AT",
        CS => "CS",
        DA => "DA",
        DS => "DS",
        DT => "DT",
        FD => "FD",
        FL => "FL",
        IS => "IS",
        LO => "LO",
        LT => "LT",
        OB => "OB",
        OD => "OD",
        OF => "OF",
        OL => "OL",
        OV => "OV",
        OW => "OW",
        PN => "PN",
        SH => "SH",
        SL => "SL",
        SQ => "SQ",
        SS => "SS",
        ST => "ST",
        SV => "SV",
        TM => "TM",
        UC => "UC",
        UI => "UI",
        UL => "UL",
        UN => "UN",
        UR => "UR",
        US => "US",
        UT => "UT",
        UV => "UV",
    }
}

fn preview(e: &Element, v: &Values, cs: Charset) -> TagValue {
    if e.value.is_empty() {
        return TagValue::Empty;
    }
    let numbers = |it: &mut dyn Iterator<Item = String>| {
        let parts: Vec<String> = (&mut *it).take(PREVIEW_NUMBERS).collect();
        let more = it.next().is_some();
        if parts.is_empty() {
            return TagValue::Empty;
        }
        let text = parts.join("\\");
        TagValue::Text { text, more }
    };
    let b = v.bytes(e);
    let n = |size: usize| b.chunks_exact(size);
    let big = v.big;
    match e.vr {
        vr if vr.is_text() => {
            let s = cs.decode(v.text_bytes(e), e.vr);
            let mut text = String::new();
            let mut more = false;
            for (i, c) in s.chars().enumerate() {
                if i >= PREVIEW_CHARS {
                    more = true;
                    break;
                }
                text.push(if c.is_control() { ' ' } else { c });
            }
            TagValue::Text { text, more }
        }
        Vr::US => numbers(&mut v.u16s(e).map(|x| x.to_string())),
        Vr::SS => numbers(&mut v.u16s(e).map(|x| (x as i16).to_string())),
        Vr::UL => numbers(&mut v.u32s(e).map(|x| x.to_string())),
        Vr::SL => numbers(&mut v.u32s(e).map(|x| (x as i32).to_string())),
        Vr::UV => numbers(&mut v.u64s(e).map(|x| x.to_string())),
        Vr::SV => numbers(&mut v.u64s(e).map(|x| (x as i64).to_string())),
        Vr::FL => numbers(&mut n(4).map(|c| {
            let a: [u8; 4] = [c[0], c[1], c[2], c[3]];
            let x = if big {
                f32::from_be_bytes(a)
            } else {
                f32::from_le_bytes(a)
            };
            format!("{x}")
        })),
        Vr::FD => numbers(&mut n(8).map(|c| {
            let a: [u8; 8] = c.try_into().unwrap_or([0; 8]);
            let x = if big {
                f64::from_be_bytes(a)
            } else {
                f64::from_le_bytes(a)
            };
            format!("{x}")
        })),
        Vr::AT => {
            let mut it = v.u16s(e);
            let mut pairs = Vec::new();
            while let (Some(g), Some(el)) = (it.next(), it.next()) {
                pairs.push(format!("({g:04X},{el:04X})"));
            }
            numbers(&mut pairs.into_iter())
        }
        _ => {
            let hex: Vec<String> = b
                .iter()
                .take(PREVIEW_BYTES)
                .map(|x| format!("{x:02X}"))
                .collect();
            TagValue::Binary {
                hex: hex.join(" "),
                len: b.len() as u64,
            }
        }
    }
}
