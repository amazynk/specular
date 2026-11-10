//! De-identification: the PS3.15 Annex E Basic Application Level Confidentiality Profile
//! with its options, applied to a parsed file and written as a new Part 10 file.
//!
//! The action for each attribute comes from Table E.1-1 (generated into `deid_table.rs` by
//! `gen/deid.py`). Compound actions are resolved without knowing the IOD's attribute types,
//! keeping the result conformant: X/Z → Z, X/D, Z/D and X/Z/D → D, X/Z/U* → the sequence is
//! kept and its items de-identified. An option that keeps (K) wins over one that cleans (C),
//! which wins over the basic action. Beyond the table: every private attribute is removed;
//! overlays (60xx) and curves (50xx) are removed unless kept; unlisted person names are
//! emptied and unlisted dates follow the temporal option; every kept sequence has its items
//! de-identified too.
//!
//! UIDs are replaced by `2.25.` UIDs derived from HMAC-SHA-256 of the original under a
//! per-export random key ([`Session`]): the same original gives the same replacement across
//! every file of one export, so references between instances stay valid, while nothing links
//! them to the originals without the key. The patient name and ID become one pseudonym
//! derived the same way, so the files of one patient stay together. Modified dates are
//! shifted by one per-export number of days.
//!
//! Pixel data is copied untouched unless burned-in text masks are given; then each frame is
//! masked (see [`crate::mask`]) and the result is written uncompressed.

use std::borrow::Cow;
use std::sync::atomic::AtomicBool;

use crate::deid_table::ENTRIES;
use crate::dict::Vr;
use crate::hash::{hmac, random_key};
use crate::mask::{self, Rect};
use crate::write::{self, Elem, FileMeta, Object, Value};
use crate::{Dicom, Error};

/// Basic Profile action codes as Table E.1-1 writes them (PS3.15 E.1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)] // the standard's codes
pub(crate) enum Basic {
    /// Replace with a dummy value.
    D,
    /// Replace with a zero-length value.
    Z,
    /// Remove.
    X,
    /// Keep (in the standard's code set; no current row uses it as the basic action).
    #[allow(dead_code)]
    K,
    /// Clean: keep with identifying information removed (as for K).
    #[allow(dead_code)]
    C,
    /// Replace the UID with a consistent one.
    U,
    XZ,
    XD,
    ZD,
    XZD,
    /// X, Z, or replace the UIDs inside the sequence's items.
    XZU,
}

// Option column bits of the generated table.
const RTN_UIDS: u16 = 1 << 1;
const RTN_DEV_ID: u16 = 1 << 2;
const RTN_INST_ID: u16 = 1 << 3;
const RTN_PAT_CHARS: u16 = 1 << 4;
const RTN_FULL_DATES: u16 = 1 << 5;
const RTN_MOD_DATES: u16 = 1 << 6;
const CLEAN_DESC: u16 = 1 << 7;
const CLEAN_STRUCT: u16 = 1 << 8;
const CLEAN_GRAPH: u16 = 1 << 9;

type Entry = (u32, u32, Vr, Basic, u16, u16);

fn entry(tag: u32) -> Option<&'static Entry> {
    if let Ok(i) = ENTRIES.binary_search_by_key(&tag, |e| e.0)
        && ENTRIES[i].1 == u32::MAX
    {
        return Some(&ENTRIES[i]);
    }
    ENTRIES.iter().find(|e| e.1 != u32::MAX && tag & e.1 == e.0)
}

/// The VR Table E.1-1's attribute has, when the dictionary knows it.
pub(crate) fn table_vr(tag: u32) -> Option<Vr> {
    entry(tag).map(|e| e.2).filter(|&v| v != Vr::UN)
}

/// What happens to dates and times (the Retain Longitudinal Temporal Information options).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Temporal {
    /// Removed or emptied per the Basic Profile.
    #[default]
    Remove,
    /// Kept as they are (Retain Longitudinal Temporal Information with Full Dates).
    Full,
    /// Shifted by a per-export number of days (with Modified Dates).
    Modified,
}

/// Profile options (PS3.15 E.3). The defaults are the Basic Profile alone.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Options {
    pub temporal: Temporal,
    /// Retain Patient Characteristics (sex, age, size, weight, …).
    pub patient_characteristics: bool,
    /// Retain Device Identity (station name, serial numbers, …).
    pub device_identity: bool,
    /// Retain Institution Identity.
    pub institution_identity: bool,
    /// Clean Descriptors: keep descriptions and comments with identifying text removed,
    /// rather than removing them.
    pub clean_descriptors: bool,
    /// Clean Structured Content: keep structured reports' content, de-identified.
    pub clean_structured_content: bool,
    /// Retain UIDs.
    pub retain_uids: bool,
    /// Keep overlays, curves and graphic annotations (Clean Graphics).
    pub keep_overlays: bool,
}

impl Options {
    fn bits(&self) -> u16 {
        let mut b = 0;
        let mut set = |on: bool, bit: u16| {
            if on {
                b |= bit;
            }
        };
        set(self.retain_uids, RTN_UIDS);
        set(self.device_identity, RTN_DEV_ID);
        set(self.institution_identity, RTN_INST_ID);
        set(self.patient_characteristics, RTN_PAT_CHARS);
        set(self.temporal == Temporal::Full, RTN_FULL_DATES);
        set(self.temporal == Temporal::Modified, RTN_MOD_DATES);
        set(self.clean_descriptors, CLEAN_DESC);
        set(self.clean_structured_content, CLEAN_STRUCT);
        set(self.keep_overlays, CLEAN_GRAPH);
        b
    }

    /// De-identification Method Code Sequence codes (CID 7050, scheme DCM) and meanings.
    fn codes(&self, pixels_cleaned: bool) -> Vec<(&'static str, &'static str)> {
        let mut c = vec![("113100", "Basic Application Confidentiality Profile")];
        if pixels_cleaned {
            c.push(("113101", "Clean Pixel Data Option"));
        }
        if self.keep_overlays {
            c.push(("113103", "Clean Graphics Option"));
        }
        if self.clean_structured_content {
            c.push(("113104", "Clean Structured Content Option"));
        }
        if self.clean_descriptors {
            c.push(("113105", "Clean Descriptors Option"));
        }
        match self.temporal {
            Temporal::Remove => {}
            Temporal::Full => c.push(("113106", "Retain Longitudinal With Full Dates Option")),
            Temporal::Modified => {
                c.push(("113107", "Retain Longitudinal With Modified Dates Option"))
            }
        }
        if self.patient_characteristics {
            c.push(("113108", "Retain Patient Characteristics Option"));
        }
        if self.device_identity {
            c.push(("113109", "Retain Device Identity Option"));
        }
        if self.retain_uids {
            c.push(("113110", "Retain UIDs Option"));
        }
        if self.institution_identity {
            c.push(("113112", "Retain Institution Identity Option"));
        }
        c
    }
}

/// The secret of one export: UIDs, pseudonyms and the date shift derive from it.
#[derive(Clone)]
pub struct Session {
    key: [u8; 32],
    /// Days dates move back by (Modified Dates), 1 to 3650.
    pub shift_days: i32,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").finish_non_exhaustive()
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// A session with a fresh random key.
    pub fn new() -> Session {
        Session::with_key(random_key())
    }

    pub fn with_key(key: [u8; 32]) -> Session {
        let d = hmac(&key, &[b"date shift"]);
        let n = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
        Session {
            key,
            shift_days: 1 + (n % 3650) as i32,
        }
    }

    /// The replacement for `uid`: `2.25.` and a UUID (version 8, RFC 9562) made from the
    /// keyed hash, as a decimal integer (PS3.5 B.2). Empty stays empty.
    pub fn uid(&self, uid: &str) -> String {
        let uid = uid.trim_end_matches(['\0', ' ']);
        if uid.is_empty() {
            return String::new();
        }
        let h = hmac(&self.key, &[b"uid\0", uid.as_bytes()]);
        let mut b: [u8; 16] = h[..16].try_into().unwrap_or([0; 16]);
        b[6] = (b[6] & 0x0F) | 0x80;
        b[8] = (b[8] & 0x3F) | 0x80;
        format!("2.25.{}", u128::from_be_bytes(b))
    }

    /// The pseudonym for a patient (by original ID and name): `ANON` and 8 hex digits.
    pub fn pseudonym(&self, id: &str, name: &str) -> String {
        let h = hmac(
            &self.key,
            &[
                b"patient\0",
                id.trim().as_bytes(),
                b"\0",
                name.trim().as_bytes(),
            ],
        );
        format!("ANON{:02X}{:02X}{:02X}{:02X}", h[0], h[1], h[2], h[3])
    }
}

/// Whether a file may show identifying text in its pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Risk {
    /// Nothing suggests burned-in text.
    None,
    /// The modality or kind of object often carries burned-in text (ultrasound, secondary
    /// capture, X-ray angiography, …): check before export.
    Possible,
    /// Burned In Annotation is YES: export needs masks (or an explicit override).
    Present,
    /// An encapsulated document (PDF, CDA, …), whose content cannot be cleaned.
    Document,
}

/// Classifies `d` by Burned In Annotation, modality and SOP Class.
pub fn risk(d: &Dicom, file: &[u8]) -> Risk {
    let v = d.values(file);
    let text = |tag: u32| {
        d.dataset
            .get(tag)
            .map(|e| String::from_utf8_lossy(v.text_bytes(e)).trim().to_owned())
            .unwrap_or_default()
    };
    if d.dataset.get(0x0042_0011).is_some() {
        return Risk::Document;
    }
    if text(0x0028_0301).eq_ignore_ascii_case("YES") {
        return Risk::Present;
    }
    let modality = text(0x0008_0060);
    let class = text(0x0008_0016);
    let secondary = class.starts_with("1.2.840.10008.5.1.4.1.1.7");
    if d.pixel.is_some()
        && (secondary
            || matches!(
                modality.as_str(),
                "US" | "SC" | "XA" | "RF" | "ES" | "XC" | "GM" | "SM" | "OT" | "DOC" | "IVUS"
            )
            || class.starts_with("1.2.840.10008.5.1.4.1.1.6")
            || class.starts_with("1.2.840.10008.5.1.4.1.1.3"))
    {
        return Risk::Possible;
    }
    Risk::None
}

/// Why a file was not anonymised.
#[derive(Debug, thiserror::Error)]
pub enum AnonError {
    #[error(transparent)]
    Dicom(#[from] Error),
    /// Burned In Annotation is YES and no masks were given.
    #[error("burned-in annotation without masks")]
    BurnedIn,
    /// Encapsulated documents cannot be cleaned.
    #[error("encapsulated document")]
    Document,
}

/// One file to anonymise.
pub struct Request<'r> {
    pub options: &'r Options,
    pub session: &'r Session,
    /// Rectangles (image pixels) zeroed in every frame.
    pub masks: &'r [Rect],
    /// Export a file whose Burned In Annotation is YES without masks.
    pub allow_burned_in: bool,
    pub cancel: Option<&'r AtomicBool>,
}

/// An anonymised file.
#[derive(Debug)]
pub struct Anonymised {
    /// The Part 10 file.
    pub bytes: Vec<u8>,
    /// Pixel data masks were applied.
    pub masked: bool,
    /// Compressed pixel data was decoded and written uncompressed.
    pub transcoded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    Keep,
    Remove,
    Empty,
    Dummy,
    Remap,
    Clean,
    Shift,
    Recurse,
    Pseudonym,
}

const PATIENT_NAME: u32 = 0x0010_0010;
const PATIENT_ID: u32 = 0x0010_0020;

fn is_overlay_or_curve(tag: u32) -> bool {
    let g = tag >> 16;
    (0x5000..=0x501E).contains(&g) || (0x6000..=0x601E).contains(&g)
}

/// The action for one element.
fn action(tag: u32, vr: Vr, bits: u16) -> Act {
    if (tag >> 16) & 1 == 1 {
        return Act::Remove;
    }
    if tag == PATIENT_NAME || tag == PATIENT_ID {
        return Act::Pseudonym;
    }
    let act = match entry(tag) {
        Some(&(_, _, _, basic, keep, clean)) => {
            if keep & bits != 0 {
                Act::Keep
            } else if clean & bits != 0 {
                if clean & bits == RTN_MOD_DATES {
                    Act::Shift
                } else {
                    Act::Clean
                }
            } else {
                match basic {
                    Basic::K => Act::Keep,
                    Basic::X => Act::Remove,
                    Basic::Z | Basic::XZ => Act::Empty,
                    Basic::D | Basic::XD | Basic::ZD | Basic::XZD => Act::Dummy,
                    Basic::C => Act::Clean,
                    Basic::U => Act::Remap,
                    Basic::XZU => Act::Recurse,
                }
            }
        }
        None if is_overlay_or_curve(tag) => {
            if bits & CLEAN_GRAPH != 0 {
                Act::Keep
            } else {
                Act::Remove
            }
        }
        None => match vr {
            Vr::PN => Act::Empty,
            Vr::DA | Vr::DT => {
                if bits & RTN_FULL_DATES != 0 {
                    Act::Keep
                } else if bits & RTN_MOD_DATES != 0 {
                    Act::Shift
                } else {
                    Act::Empty
                }
            }
            // Free text the profile does not list may still quote a name or an ID: the
            // patient's identifying words are scrubbed from it.
            Vr::LO | Vr::SH | Vr::LT | Vr::ST | Vr::UT | Vr::UC => Act::Clean,
            _ => Act::Keep,
        },
    };
    // Sequences: anything kept has its items de-identified; dummies become the items
    // de-identified (there is no dummy sequence).
    match (vr, act) {
        (Vr::SQ, Act::Keep | Act::Clean | Act::Dummy | Act::Shift | Act::Remap) => Act::Recurse,
        (Vr::UI, Act::Dummy) => Act::Remap,
        (Vr::TM, Act::Shift) => Act::Keep,
        _ => act,
    }
}

/// Identifying words of the patient, removed from free text by the Clean actions.
struct Scrubber {
    words: Vec<String>,
}

impl Scrubber {
    fn of(obj: &Object) -> Scrubber {
        let mut words: Vec<String> = Vec::new();
        for tag in [
            PATIENT_NAME,
            PATIENT_ID,
            0x0010_1000, // Other Patient IDs
            0x0010_1001, // Other Patient Names
            0x0010_1005, // Patient's Birth Name
            0x0010_1060, // Patient's Mother's Birth Name
            0x0010_0030, // Patient's Birth Date
            0x0008_0050, // Accession Number
            0x0010_1040, // Patient's Address
            0x0010_2154, // Patient's Telephone Numbers
            0x0008_0090, // Referring Physician's Name
            0x0010_2180, // Occupation
            0x0038_0010, // Admission ID
        ] {
            if let Some(t) = obj.text(tag) {
                for w in t.split(['^', '=', '\\', ' ', ',']) {
                    let w = w.trim();
                    if w.len() >= 2 && !words.iter().any(|x| x.eq_ignore_ascii_case(w)) {
                        words.push(w.to_owned());
                    }
                }
            }
        }
        // Person names anywhere (operators, observers, names in structured content).
        fn names(o: &Object, words: &mut Vec<String>) {
            for e in &o.elements {
                match &e.value {
                    Value::Items(items) => items.iter().for_each(|i| names(i, words)),
                    _ if e.vr == Vr::PN => {
                        for w in e.text().split(['^', '=', '\\', ' ', ',']) {
                            let w = w.trim();
                            if w.len() >= 3 && !words.iter().any(|x| x.eq_ignore_ascii_case(w)) {
                                words.push(w.to_owned());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        names(obj, &mut words);
        // Longest first, so a whole ID goes before a part of it.
        words.sort_by_key(|w| std::cmp::Reverse(w.len()));
        Scrubber { words }
    }

    /// `text` with every identifying word (case-insensitive, at word boundaries) replaced.
    fn clean(&self, text: &[u8]) -> Vec<u8> {
        let mut out = text.to_vec();
        for w in &self.words {
            let w = w.as_bytes();
            let mut i = 0;
            while i + w.len() <= out.len() {
                let hit = out[i..i + w.len()].eq_ignore_ascii_case(w)
                    && (i == 0 || !out[i - 1].is_ascii_alphanumeric())
                    && out
                        .get(i + w.len())
                        .is_none_or(|c| !c.is_ascii_alphanumeric());
                if hit {
                    out.splice(i..i + w.len(), b"XXXX".iter().copied());
                    i += 4;
                } else {
                    i += 1;
                }
            }
        }
        out
    }
}

struct Ctx<'c> {
    bits: u16,
    session: &'c Session,
    scrub: Scrubber,
    pseudonym: String,
}

fn dummy(vr: Vr, len: usize) -> Vec<u8> {
    match vr {
        Vr::PN | Vr::LO | Vr::SH | Vr::LT | Vr::ST | Vr::UT | Vr::UC | Vr::CS | Vr::AE => {
            b"ANONYMOUS".to_vec()
        }
        Vr::DA => b"19000101".to_vec(),
        Vr::DT => b"19000101000000".to_vec(),
        Vr::TM => b"000000".to_vec(),
        Vr::AS => b"000Y".to_vec(),
        Vr::DS | Vr::IS => b"0".to_vec(),
        Vr::UR | Vr::UN | Vr::SQ => Vec::new(),
        // Binary: zeros of the same length.
        _ => vec![0; len],
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date; `None` for a year beyond ±9999, a
/// month outside 1–12 or a day outside 1–31 (bounded fields keep the arithmetic in range for
/// any digits a file holds).
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> Option<i64> {
    if !(-9999..=9999).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Moves a YYYYMMDD date back by `days`; `None` for anything else.
fn shift_date(s: &str, days: i32) -> Option<String> {
    let s = s.replace('.', "");
    if s.len() != 8 || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let y: i64 = s[..4].parse().ok()?;
    let m: i64 = s[4..6].parse().ok()?;
    let d: i64 = s[6..8].parse().ok()?;
    let (y, m, d) = civil_from_days(days_from_civil(y, m, d)? - days as i64);
    (0..=9999)
        .contains(&y)
        .then(|| format!("{y:04}{m:02}{d:02}"))
}

/// A DA or DT value with every date moved back; parts that are not full dates are dropped.
fn shift_value(text: &str, vr: Vr, days: i32) -> String {
    let one = |v: &str| -> String {
        if vr == Vr::DT {
            if v.len() < 8 {
                return String::new();
            }
            let (date, rest) = v.split_at(8);
            // The time (before any UTC offset) must be one: hour under 24, minute under 60,
            // second up to 60.
            let time = rest.split(['+', '-']).next().unwrap_or("");
            if !time.is_empty() && crate::suv::seconds(time).is_none() {
                return String::new();
            }
            shift_date(date, days).map_or(String::new(), |d| d + rest)
        } else {
            shift_date(v, days).unwrap_or_default()
        }
    };
    text.split('\\')
        .map(|v| {
            let v = v.trim();
            // Ranges: both ends move.
            match v.split_once('-') {
                Some((a, b)) if vr == Vr::DA => {
                    let a = if a.is_empty() { String::new() } else { one(a) };
                    let b = if b.is_empty() { String::new() } else { one(b) };
                    format!("{a}-{b}")
                }
                _ => one(v),
            }
        })
        .collect::<Vec<_>>()
        .join("\\")
}

fn apply<'a>(obj: &mut Object<'a>, ctx: &Ctx) {
    let mut out: Vec<Elem<'a>> = Vec::with_capacity(obj.elements.len());
    for mut e in std::mem::take(&mut obj.elements) {
        if e.tag >> 16 == 2 || e.tag == 0x7FE0_0010 && !matches!(e.value, Value::Items(_)) {
            out.push(e);
            continue;
        }
        let act = action(e.tag, e.vr, ctx.bits);
        match act {
            Act::Keep => {}
            Act::Remove => continue,
            Act::Empty => {
                e.value = match e.value {
                    Value::Items(_) => Value::Items(Vec::new()),
                    _ => Value::Bytes(Cow::Borrowed(&[])),
                }
            }
            Act::Dummy => {
                let len = e.bytes().len();
                e.value = Value::Bytes(Cow::Owned(dummy(e.vr, len)));
            }
            Act::Pseudonym => {
                e.value = Value::Bytes(Cow::Owned(ctx.pseudonym.as_bytes().to_vec()));
            }
            Act::Remap => {
                if e.vr == Vr::UI || e.vr == Vr::UN {
                    let t = e.text();
                    let mapped: Vec<String> = t.split('\\').map(|u| ctx.session.uid(u)).collect();
                    e.vr = Vr::UI;
                    e.value = Value::Bytes(Cow::Owned(mapped.join("\\").into_bytes()));
                } else {
                    e.value = Value::Bytes(Cow::Borrowed(&[]));
                }
            }
            Act::Clean => {
                if e.vr.is_text() && e.vr != Vr::UI {
                    let c = ctx.scrub.clean(e.bytes());
                    e.value = Value::Bytes(Cow::Owned(c));
                } else if !matches!(e.vr, Vr::OB | Vr::OW) {
                    // Binary or unknown content cannot be cleaned: removed.
                    continue;
                }
            }
            Act::Shift => {
                if matches!(e.vr, Vr::DA | Vr::DT) {
                    let s = shift_value(&e.text(), e.vr, ctx.session.shift_days);
                    e.value = Value::Bytes(Cow::Owned(s.into_bytes()));
                } else if !matches!(e.vr, Vr::TM) {
                    e.value = Value::Bytes(Cow::Borrowed(&[]));
                }
            }
            Act::Recurse => {
                if let Value::Items(items) = &mut e.value {
                    for i in items {
                        apply(i, ctx);
                    }
                }
            }
        }
        out.push(e);
    }
    obj.elements = out;
}

/// Anonymises one file (the Part 10 bytes, any transfer syntax we read).
pub fn anonymise(file: &[u8], req: &Request) -> Result<Anonymised, AnonError> {
    let d = Dicom::parse(file)?;
    match risk(&d, file) {
        Risk::Document => return Err(AnonError::Document),
        Risk::Present if req.masks.is_empty() && !req.allow_burned_in => {
            return Err(AnonError::BurnedIn);
        }
        _ => {}
    }
    let buf = d.buf(file);
    let mut obj = Object::from_dataset(&d.dataset, buf);
    let id = obj.text(PATIENT_ID).unwrap_or_default().into_owned();
    let name = obj.text(PATIENT_NAME).unwrap_or_default().into_owned();
    let ctx = Ctx {
        bits: req.options.bits(),
        session: req.session,
        scrub: Scrubber::of(&obj),
        pseudonym: req.session.pseudonym(&id, &name),
    };
    apply(&mut obj, &ctx);
    // Patient name and ID are always set, even when absent from the source.
    obj.put_text(PATIENT_NAME, Vr::PN, &ctx.pseudonym);
    obj.put_text(PATIENT_ID, Vr::LO, &ctx.pseudonym);
    crate::cancelled(req.cancel)?;

    let mut transcoded = false;
    let mut masked = false;
    if !req.masks.is_empty() && d.pixel.is_some() {
        let out = mask::apply(&d, file, &mut obj, req.masks, req.cancel)?;
        masked = out.masked;
        transcoded = out.transcoded;
    }

    // De-identification attributes (PS3.15 E.1.1).
    obj.put_text(0x0012_0062, Vr::CS, "YES");
    let codes = req.options.codes(masked);
    let mut method = vec!["Specular PS3.15 E Basic Profile"];
    for (_, meaning) in codes.iter().skip(1) {
        method.push(meaning);
    }
    obj.put_text(0x0012_0063, Vr::LO, &method.join("\\"));
    let items = codes
        .iter()
        .map(|(value, meaning)| {
            let mut i = Object::default();
            i.put_text(0x0008_0100, Vr::SH, value);
            i.put_text(0x0008_0102, Vr::SH, "DCM");
            i.put_text(0x0008_0104, Vr::LO, meaning);
            i
        })
        .collect();
    obj.put(0x0012_0064, Vr::SQ, Value::Items(items));
    let temporal = match req.options.temporal {
        Temporal::Remove => "REMOVED",
        Temporal::Full => "UNMODIFIED",
        Temporal::Modified => "MODIFIED",
    };
    obj.put_text(0x0028_0303, Vr::CS, temporal);
    if masked {
        obj.put_text(0x0028_0301, Vr::CS, "NO");
    }

    let encapsulated = matches!(
        obj.get(0x7FE0_0010).map(|e| &e.value),
        Some(Value::Fragments { .. })
    );
    let ts = if encapsulated && !transcoded {
        d.transfer_syntax.uid.clone()
    } else {
        write::EXPLICIT_LE.to_owned()
    };
    let meta = FileMeta::of(&obj, &ts);
    let bytes = write::part10(&obj, &meta)?;
    Ok(Anonymised {
        bytes,
        masked,
        transcoded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_lookups() {
        let e = entry(0x0010_0010).unwrap();
        assert_eq!((e.2, e.3), (Vr::PN, Basic::Z));
        assert_eq!(entry(0x0008_0018).unwrap().3, Basic::U);
        // Repeating groups match by mask.
        assert_eq!(entry(0x6002_3000).unwrap().0, 0x6000_3000);
        assert_eq!(entry(0x5010_2000).unwrap().0, 0x5000_0000);
        assert!(entry(0x0028_0010).is_none());
        // The generated table is sorted and complete enough to hold the core attributes.
        assert!(ENTRIES.windows(2).all(|w| w[0].0 <= w[1].0));
        assert!(ENTRIES.len() > 600);
    }

    #[test]
    fn actions_follow_options() {
        let none = Options::default().bits();
        // Basic profile.
        assert_eq!(action(0x0010_0030, Vr::DA, none), Act::Empty); // birth date: Z
        assert_eq!(action(0x0008_0080, Vr::LO, none), Act::Dummy); // institution X/Z/D
        assert_eq!(action(0x0008_103E, Vr::LO, none), Act::Remove); // series description X
        assert_eq!(action(0x0020_000D, Vr::UI, none), Act::Remap);
        assert_eq!(action(0x0008_1140, Vr::SQ, none), Act::Recurse); // X/Z/U*
        assert_eq!(action(0x0010_0010, Vr::PN, none), Act::Pseudonym);
        assert_eq!(action(0x0009_1001, Vr::LO, none), Act::Remove); // private
        assert_eq!(action(0x6000_0010, Vr::US, none), Act::Remove); // overlay rows
        assert_eq!(action(0x0008_0020, Vr::DA, none), Act::Empty); // study date Z
        assert_eq!(action(0x0028_0010, Vr::US, none), Act::Keep); // unlisted
        assert_eq!(action(0x0040_0244, Vr::DA, none), Act::Remove); // listed X date
        assert_eq!(action(0x0011_0010, Vr::LO, none), Act::Remove); // private creator
        assert_eq!(action(0x0033_1010, Vr::PN, none), Act::Remove);
        assert_eq!(action(0x0018_9999, Vr::PN, none), Act::Empty); // unlisted name
        // Options.
        let o = |f: fn(&mut Options)| {
            let mut x = Options::default();
            f(&mut x);
            x.bits()
        };
        let uids = o(|x| x.retain_uids = true);
        assert_eq!(action(0x0020_000D, Vr::UI, uids), Act::Keep);
        let full = o(|x| x.temporal = Temporal::Full);
        assert_eq!(action(0x0008_0020, Vr::DA, full), Act::Keep);
        let modif = o(|x| x.temporal = Temporal::Modified);
        assert_eq!(action(0x0008_0020, Vr::DA, modif), Act::Shift);
        assert_eq!(action(0x0008_0030, Vr::TM, modif), Act::Keep);
        let desc = o(|x| x.clean_descriptors = true);
        assert_eq!(action(0x0008_103E, Vr::LO, desc), Act::Clean);
        let dev = o(|x| x.device_identity = true);
        assert_eq!(action(0x0018_1000, Vr::LO, dev), Act::Keep);
        let pat = o(|x| x.patient_characteristics = true);
        assert_eq!(action(0x0010_0040, Vr::CS, pat), Act::Keep);
        assert_eq!(action(0x0010_0040, Vr::CS, none), Act::Empty);
        let ov = o(|x| x.keep_overlays = true);
        assert_eq!(action(0x6000_3000, Vr::OW, ov), Act::Clean);
        assert_eq!(action(0x6000_0010, Vr::US, ov), Act::Keep);
        // K wins over C: Station Name is kept by Device Identity and cleaned by Descriptors.
        let both = o(|x| {
            x.device_identity = true;
            x.clean_descriptors = true;
        });
        assert_eq!(action(0x0008_1010, Vr::SH, both), Act::Keep);
    }

    #[test]
    fn uid_remap_is_consistent_and_keyed() {
        let a = Session::with_key([1; 32]);
        let b = Session::with_key([2; 32]);
        let u = "1.2.840.113619.2.55.3.604688119.868.1234567890.123";
        assert_eq!(a.uid(u), a.uid(u));
        assert_eq!(a.uid(u), a.uid(&format!("{u}\0")));
        assert_ne!(a.uid(u), b.uid(u));
        assert_ne!(a.uid(u), a.uid("1.2.3"));
        let r = a.uid(u);
        assert!(r.starts_with("2.25.") && r.len() <= 64, "{r}");
        assert!(r[5..].bytes().all(|c| c.is_ascii_digit()) && !r[5..].starts_with('0'));
        // A version 8, RFC 9562 variant UUID.
        let n: u128 = r[5..].parse().unwrap();
        assert_eq!((n >> 76) & 0xF, 8);
        assert_eq!((n >> 62) & 0x3, 2);
        assert_eq!(a.uid(""), "");
        assert_eq!(a.pseudonym("123", "Doe^J"), a.pseudonym("123 ", "Doe^J"));
        assert_ne!(a.pseudonym("123", "Doe^J"), a.pseudonym("124", "Doe^J"));
        assert_eq!(a.pseudonym("1", "x").len(), 12);
    }

    #[test]
    fn dates_shift() {
        assert_eq!(shift_date("20240301", 1).as_deref(), Some("20240229"));
        assert_eq!(shift_date("20230301", 1).as_deref(), Some("20230228"));
        assert_eq!(shift_date("20000101", 365).as_deref(), Some("19990101"));
        assert_eq!(shift_date("2000.01.02", 1).as_deref(), Some("20000101"));
        assert_eq!(shift_date("2000", 1), None);
        assert_eq!(shift_date("20001301", 1), None);
        assert_eq!(
            shift_value("20240301\\20240302-", Vr::DA, 1),
            "20240229\\20240301-"
        );
        assert_eq!(
            shift_value("20240301120000.5+0100", Vr::DT, 1),
            "20240229120000.5+0100"
        );
        for z in [-800_000, -1, 0, 1, 19_000, 2_000_000] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), Some(z));
        }
    }

    #[test]
    fn dates_out_of_range() {
        for (y, m, d) in [
            (i64::MAX, 1, 1),
            (i64::MIN, 3, 1),
            (10_000, 1, 1),
            (-10_000, 1, 1),
            (2000, 0, 1),
            (2000, 13, 1),
            (2000, i64::MIN, 1),
            (2000, 1, 0),
            (2000, 1, 32),
            (2000, 1, i64::MAX),
        ] {
            assert_eq!(days_from_civil(y, m, d), None, "{y} {m} {d}");
        }
        assert_eq!(days_from_civil(9999, 12, 31), Some(2_932_896));
        assert_eq!(days_from_civil(1970, 1, 1), Some(0));
        // Times that are not times drop the value, like dates that are not dates.
        assert_eq!(shift_value("20240301250000", Vr::DT, 1), "");
        assert_eq!(shift_value("20240301126000", Vr::DT, 1), "");
        assert_eq!(shift_value("20240301120061+0100", Vr::DT, 1), "");
        assert_eq!(shift_value("2024030123", Vr::DT, 1), "2024022923");
        assert_eq!(shift_value("20240301-0500", Vr::DT, 1), "20240229-0500");
    }

    #[test]
    fn scrubber_cleans_words() {
        let mut o = Object::default();
        o.put_text(PATIENT_NAME, Vr::PN, "Doe^Jane^M");
        o.put_text(PATIENT_ID, Vr::LO, "MRN12345");
        let s = Scrubber::of(&o);
        assert_eq!(
            s.clean(b"Jane DOE (MRN12345) Janet"),
            b"XXXX XXXX (XXXX) Janet".to_vec()
        );
    }
}
