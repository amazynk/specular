//! Patient media: CDs, DVDs, USB sticks and folders of DICOM files, as patients, their
//! studies and the studies' series.
//!
//! - [`detect`] recognises a medium: a DICOMDIR file, a folder with a DICOMDIR (any case, also
//!   in a subfolder such as `DICOM/` or `IHE_PDI/`), or a folder of DICOM files without
//!   extensions.
//! - [`read_dicomdir`] reads the patient / study / series / image records of a DICOMDIR, with
//!   the series' icons, and finds the files on disk despite real-world quirks: File IDs in
//!   the wrong case, ISO 9660 version suffixes (`;1`), trailing dots, `/` instead of `\`.
//!   Files that are not there are counted as missing.
//! - [`scan`] reads the headers of every DICOM file in a folder tree instead (no DICOMDIR),
//!   off the UI thread, cancellable, handing results over in batches. Vendor viewers on the
//!   disc (programs, autorun files, web pages) are skipped by extension.
//!
//! Patient data (name, ID, birth date) is kept for the studies browser, which hides it by
//! default; `Debug` output redacts it.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::charset::Charset;
use crate::display::{Image, Rendered};
use crate::parse::{DataSet, Values};
use crate::series::{is_dicomdir_name, natural_cmp, read_header_with};
use crate::ts::Encoding;
use crate::{Dicom, Error, Result};

const PATIENT_NAME: u32 = 0x0010_0010;
const PATIENT_ID: u32 = 0x0010_0020;
const BIRTH_DATE: u32 = 0x0010_0030;
const STUDY_DATE: u32 = 0x0008_0020;
const STUDY_DESCRIPTION: u32 = 0x0008_1030;
const STUDY_UID: u32 = 0x0020_000D;
const SERIES_UID: u32 = 0x0020_000E;
const SOP_UID: u32 = 0x0008_0018;
const MODALITY: u32 = 0x0008_0060;
const SERIES_NUMBER: u32 = 0x0020_0011;
const SERIES_DESCRIPTION: u32 = 0x0008_103E;
const INSTANCE_NUMBER: u32 = 0x0020_0013;
const NUMBER_OF_FRAMES: u32 = 0x0028_0008;

const DIRECTORY_RECORDS: u32 = 0x0004_1220;
const FIRST_ROOT_RECORD: u32 = 0x0004_1200;
const NEXT_RECORD: u32 = 0x0004_1400;
const LOWER_RECORD: u32 = 0x0004_1420;
const RECORD_TYPE: u32 = 0x0004_1430;
const REFERENCED_FILE: u32 = 0x0004_1500;
const REFERENCED_SOP: u32 = 0x0004_1511;
const ICON_IMAGE_SEQUENCE: u32 = 0x0088_0200;

/// Most directory records followed.
const MAX_RECORDS: usize = 1 << 20;
/// Folders a [`detect`] looks into for a DICOMDIR or DICOM files, at most.
const DETECT_DIRS: usize = 48;
/// Files [`detect`] reads the first bytes of, at most.
const DETECT_SNIFFS: usize = 12;
/// Folder levels below the opened one searched for a DICOMDIR.
const DICOMDIR_DEPTH: usize = 2;
/// Folder levels below the opened one searched for DICOM files.
const FILES_DEPTH: usize = 6;

/// Extensions of files that are never DICOM: the viewers, autorun files and documents that
/// patient discs carry next to the images.
const NOT_DICOM: &[&str] = &[
    "exe", "dll", "inf", "ini", "bat", "cmd", "com", "msi", "sys", "ocx", "cab", "chm", "hlp",
    "lnk", "url", "ico", "icns", "htm", "html", "xhtml", "js", "css", "xml", "xsl", "xsd", "txt",
    "pdf", "rtf", "doc", "docx", "jar", "class", "so", "dylib", "app", "plist", "dat", "cfg",
    "config", "manifest", "json", "lic", "log", "sh", "bin", "pak", "zip", "jpg", "jpeg", "png",
    "gif", "bmp", "svg", "ttf", "otf", "woff", "woff2", "mp4", "avi", "wav", "md5", "sha1", "sig",
    "cat", "nfo", "csv",
];

// ----------------------------------------------------------------------------- the model

/// A medium's contents.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Medium {
    /// The DICOMDIR the contents come from (`None`: from the files' headers).
    pub dicomdir: Option<PathBuf>,
    pub patients: Vec<Patient>,
    /// Files a DICOMDIR names that are not on the medium.
    pub missing: usize,
}

/// A patient. `Debug` leaves out the identifying fields.
#[derive(Clone, Default, PartialEq)]
pub struct Patient {
    /// Patient's Name (PN, decoded, components separated by `^`, groups by `=`).
    pub name: String,
    pub id: String,
    /// Patient's Birth Date (`YYYYMMDD`, may be empty).
    pub birth_date: String,
    pub studies: Vec<Study>,
}

impl fmt::Debug for Patient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Patient")
            .field("name", &"<hidden>")
            .field("studies", &self.studies)
            .finish_non_exhaustive()
    }
}

/// A study. Its date is patient-identifying in combination; it is only shown in the studies
/// browser.
#[derive(Clone, Default, PartialEq)]
pub struct Study {
    pub uid: String,
    /// Study Date (`YYYYMMDD`, may be empty).
    pub date: String,
    pub description: String,
    pub series: Vec<MediaSeries>,
}

impl fmt::Debug for Study {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Study")
            .field("series", &self.series)
            .finish_non_exhaustive()
    }
}

impl Study {
    /// The modalities of the study's series, in order of first appearance.
    pub fn modalities(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for s in &self.series {
            if !s.modality.is_empty() && !out.contains(&s.modality.as_str()) {
                out.push(&s.modality);
            }
        }
        out
    }
}

/// A series on a medium.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaSeries {
    pub uid: String,
    pub modality: String,
    pub number: Option<i64>,
    pub description: String,
    /// The image files, in instance number order (then file name order).
    pub images: Vec<MediaFile>,
    /// Other objects of the series (presentation states, reports, ...).
    pub others: Vec<MediaFile>,
    /// The DICOMDIR's icon for the series (its own, else its first image's).
    pub icon: Option<Thumb>,
}

impl MediaSeries {
    /// Images whose file is missing.
    pub fn missing(&self) -> usize {
        self.images.iter().filter(|f| !f.present).count()
    }

    /// The files of the images that are on the medium.
    pub fn present(&self) -> impl Iterator<Item = &Path> {
        self.images
            .iter()
            .filter(|f| f.present)
            .map(|f| f.path.as_path())
    }

    /// The image to make a thumbnail of: the middle one present.
    pub fn middle(&self) -> Option<&Path> {
        let present: Vec<&Path> = self.present().collect();
        present.get(present.len() / 2).copied()
    }
}

/// One file of a series.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaFile {
    /// Where the file is (or would be, when it is missing).
    pub path: PathBuf,
    pub present: bool,
    pub sop_uid: String,
    pub instance_number: Option<i64>,
    /// Frames (multi-frame images), 1 when not given.
    pub frames: u32,
}

/// A small RGBA image (8 bits, sRGB, alpha 255).
#[derive(Clone, PartialEq, Eq)]
pub struct Thumb {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl fmt::Debug for Thumb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Thumb({}x{})", self.width, self.height)
    }
}

/// A Person Name (PN) for display: "Family, Given Middle" (prefix before the given names,
/// suffix last), from its first component group that is not empty (alphabetic, else
/// ideographic, else phonetic). Empty when the name is.
pub fn display_name(pn: &str) -> String {
    let Some(group) = pn
        .split('=')
        .map(str::trim)
        .find(|g| g.chars().any(|c| c != '^' && !c.is_whitespace()))
    else {
        return String::new();
    };
    let mut c = group.split('^').map(str::trim);
    let family = c.next().unwrap_or("");
    let given = c.next().unwrap_or("");
    let middle = c.next().unwrap_or("");
    let prefix = c.next().unwrap_or("");
    let suffix = c.next().unwrap_or("");
    let first: Vec<&str> = [prefix, given, middle]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    let mut out = family.to_owned();
    if !first.is_empty() {
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(&first.join(" "));
    }
    if !suffix.is_empty() {
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(suffix);
    }
    out
}

impl Medium {
    /// Studies on the medium.
    pub fn study_count(&self) -> usize {
        self.patients.iter().map(|p| p.studies.len()).sum()
    }

    /// Series with images on the medium.
    pub fn series_count(&self) -> usize {
        self.patients
            .iter()
            .flat_map(|p| &p.studies)
            .map(|s| s.series.len())
            .sum()
    }

    /// Adds files read by [`scan`].
    pub fn add(&mut self, headers: Vec<FileHeader>) {
        for h in headers {
            let p = match self
                .patients
                .iter()
                .position(|p| p.id == h.patient_id && p.name == h.patient_name)
            {
                Some(i) => i,
                None => {
                    self.patients.push(Patient {
                        name: h.patient_name.clone(),
                        id: h.patient_id.clone(),
                        birth_date: h.birth_date.clone(),
                        studies: Vec::new(),
                    });
                    self.patients.len() - 1
                }
            };
            let studies = &mut self.patients[p].studies;
            let st = match studies.iter().position(|s| s.uid == h.study_uid) {
                Some(i) => i,
                None => {
                    studies.push(Study {
                        uid: h.study_uid.clone(),
                        date: h.study_date.clone(),
                        description: h.study_description.clone(),
                        series: Vec::new(),
                    });
                    studies.len() - 1
                }
            };
            let series = &mut studies[st].series;
            let se = match series.iter().position(|s| s.uid == h.series_uid) {
                Some(i) => i,
                None => {
                    series.push(MediaSeries {
                        uid: h.series_uid.clone(),
                        modality: h.modality.clone(),
                        number: h.series_number,
                        description: h.series_description.clone(),
                        ..MediaSeries::default()
                    });
                    series.len() - 1
                }
            };
            let s = &mut series[se];
            let file = MediaFile {
                path: h.path,
                present: true,
                sop_uid: h.sop_uid,
                instance_number: h.instance_number,
                frames: h.frames,
            };
            if h.image {
                s.images.push(file);
            } else {
                s.others.push(file);
            }
        }
        self.sort();
    }

    /// Patients by name, studies newest first, series by number, images by instance number;
    /// series without images are left out of [`Study::series`] only when they have no
    /// images and no other objects.
    fn sort(&mut self) {
        let key = |f: &MediaFile| (f.instance_number.unwrap_or(i64::MAX), f.path.clone());
        for p in &mut self.patients {
            for st in &mut p.studies {
                for se in &mut st.series {
                    se.images.sort_by(|a, b| {
                        let (ka, kb) = (key(a), key(b));
                        ka.0.cmp(&kb.0).then_with(|| {
                            natural_cmp(&ka.1.to_string_lossy(), &kb.1.to_string_lossy())
                        })
                    });
                }
                st.series
                    .retain(|s| !s.images.is_empty() || !s.others.is_empty());
                st.series.sort_by(|a, b| {
                    // Series with images first, then by number, then description.
                    a.images
                        .is_empty()
                        .cmp(&b.images.is_empty())
                        .then(
                            a.number
                                .unwrap_or(i64::MAX)
                                .cmp(&b.number.unwrap_or(i64::MAX)),
                        )
                        .then_with(|| natural_cmp(&a.description, &b.description))
                });
            }
            p.studies.retain(|s| !s.series.is_empty());
            p.studies
                .sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.uid.cmp(&b.uid)));
        }
        self.patients.retain(|p| !p.studies.is_empty());
        self.patients
            .sort_by(|a, b| natural_cmp(&a.name, &b.name).then_with(|| a.id.cmp(&b.id)));
    }
}

// ----------------------------------------------------------------------------- detection

/// What a path is, as a medium.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Detected {
    /// A DICOMDIR (the file itself, or found in or below the folder).
    Dicomdir(PathBuf),
    /// A folder with DICOM files without extensions (and no DICOMDIR): scan it.
    Files(PathBuf),
}

/// Whether `path` is a medium: a DICOMDIR, a folder with a DICOMDIR in it or up to two
/// levels below, or a folder whose files (or whose subfolders' files) are DICOM files without
/// extensions. Reads a bounded number of folders and file starts.
pub fn detect(path: &Path) -> Option<Detected> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.is_file() {
        return path
            .file_name()
            .is_some_and(is_dicomdir_name)
            .then(|| Detected::Dicomdir(path.to_path_buf()));
    }
    if !meta.is_dir() {
        return None;
    }
    // Breadth first: a DICOMDIR at the top wins over one deeper down.
    let mut level = vec![path.to_path_buf()];
    let mut dirs_read = 0;
    let mut sniffs = 0;
    let mut dicom_files = false;
    for depth in 0..=FILES_DEPTH {
        let mut next = Vec::new();
        for dir in &level {
            if dirs_read >= DETECT_DIRS {
                break;
            }
            dirs_read += 1;
            let Ok(rd) = std::fs::read_dir(dir) else {
                continue;
            };
            let mut entries: Vec<(OsString, bool)> = rd
                .flatten()
                .filter_map(|e| Some((e.file_name(), e.file_type().ok()?.is_dir())))
                .collect();
            entries.sort_by(|a, b| natural_cmp(&a.0.to_string_lossy(), &b.0.to_string_lossy()));
            for (name, is_dir) in entries {
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
                let p = dir.join(&name);
                if is_dir {
                    next.push(p);
                } else if depth <= DICOMDIR_DEPTH && is_dicomdir_name(&name) {
                    return Some(Detected::Dicomdir(p));
                } else if !dicom_files && sniffs < DETECT_SNIFFS && extensionless(&p) {
                    sniffs += 1;
                    dicom_files = is_dicom_file(&p);
                }
            }
        }
        if dicom_files && depth >= DICOMDIR_DEPTH {
            break;
        }
        if next.is_empty() {
            break;
        }
        level = next;
    }
    dicom_files.then(|| Detected::Files(path.to_path_buf()))
}

/// No extension, or an all-digit one (UID file names).
fn extensionless(p: &Path) -> bool {
    match p.extension().and_then(|e| e.to_str()) {
        None => true,
        Some(e) => !e.is_empty() && e.bytes().all(|b| b.is_ascii_digit()),
    }
}

/// `DICM` at offset 128.
fn is_dicom_file(p: &Path) -> bool {
    let mut head = [0u8; 132];
    std::fs::File::open(p)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok()
        && &head[128..] == b"DICM"
}

// ------------------------------------------------------------------------ file resolution

/// Finds DICOMDIR File IDs on disk: each component matched exactly, else ignoring case, an
/// ISO 9660 version (`;1`) and trailing dots, on either side. Each folder is listed once.
#[derive(Default)]
pub struct Resolver {
    listings: HashMap<PathBuf, Option<Listing>>,
}

struct Listing {
    /// Entries by loose name: (name, is a folder), the exact spellings first.
    loose: HashMap<String, Vec<(OsString, bool)>>,
}

fn loose(name: &str) -> String {
    let mut n = name.trim();
    if let Some(i) = n.rfind(';')
        && n[i + 1..].bytes().all(|b| b.is_ascii_digit())
    {
        n = &n[..i];
    }
    n.trim_end_matches('.').to_lowercase()
}

/// Folders with the same loose name followed, at most, per component.
const RESOLVE_BRANCHES: usize = 4;

impl Resolver {
    fn listing(&mut self, dir: &Path) -> Option<&Listing> {
        self.listings
            .entry(dir.to_path_buf())
            .or_insert_with(|| {
                let rd = std::fs::read_dir(dir).ok()?;
                let mut l = Listing {
                    loose: HashMap::new(),
                };
                for e in rd.flatten() {
                    let name = e.file_name();
                    // Symbolic links count as what they point to.
                    let is_dir = e.path().is_dir();
                    l.loose
                        .entry(loose(&name.to_string_lossy()))
                        .or_default()
                        .push((name, is_dir));
                }
                Some(l)
            })
            .as_ref()
    }

    /// The file `components` name below `root`, if it is there. Where several entries match a
    /// component loosely (`DICOM` and `dicom`), the exact spelling is tried first.
    pub fn resolve(&mut self, root: &Path, components: &[String]) -> Option<PathBuf> {
        let (first, rest) = components.split_first()?;
        let mut options: Vec<(OsString, bool)> =
            self.listing(root)?.loose.get(&loose(first))?.clone();
        options.sort_by_key(|(n, _)| n.to_str() != Some(first.as_str()));
        for (name, is_dir) in options.into_iter().take(RESOLVE_BRANCHES) {
            let p = root.join(&name);
            match (rest.is_empty(), is_dir) {
                (true, false) => return Some(p),
                (false, true) => {
                    if let Some(found) = self.resolve(&p, rest) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }
}

/// The components of a Referenced File ID (`\`-separated; `/` from some writers), without
/// empty, `.` or `..` components.
pub fn file_id(raw: &str) -> Vec<String> {
    raw.split(['\\', '/'])
        .map(|c| c.trim().trim_end_matches('\0').to_owned())
        .filter(|c| !c.is_empty() && c != "." && c != "..")
        .collect()
}

// ------------------------------------------------------------------------------ DICOMDIR

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Patient,
    Study,
    Series,
    Leaf,
}

/// The records of a DICOMDIR in tree order, each with its parent's index: by the records'
/// offsets (next / lower level), else in sequence order by record type.
fn record_tree(d: &Dicom, v: &Values, records: &[DataSet]) -> Vec<(usize, Option<usize>)> {
    let level = |r: &DataSet| match v_text(v, r, RECORD_TYPE).as_str() {
        "PATIENT" => Level::Patient,
        "STUDY" => Level::Study,
        "SERIES" => Level::Series,
        _ => Level::Leaf,
    };
    let explicit = d.transfer_syntax.encoding != Encoding::ImplicitLittle;
    // Record offsets: from the start of the item header (8 bytes) before the first element.
    let start_of = |ds: &DataSet| -> Option<u64> {
        let e = ds.elements.first()?;
        let header = if explicit && e.vr.long_length() {
            12
        } else {
            8
        };
        (e.value.start as u64).checked_sub(header + 8)
    };
    let by_offset: HashMap<u64, usize> = if d.inflated.is_none() {
        records
            .iter()
            .enumerate()
            .filter_map(|(i, r)| Some((start_of(r)?, i)))
            .collect()
    } else {
        HashMap::new()
    };
    let offset = |ds: &DataSet, tag| -> Option<usize> {
        let e = ds.get(tag)?;
        let o = v.int(e).filter(|o| *o > 0)? as u64;
        by_offset.get(&o).copied()
    };
    let mut out = Vec::new();
    if let Some(root) = offset(&d.dataset, FIRST_ROOT_RECORD) {
        let mut visited = vec![false; records.len()];
        let mut stack: Vec<(usize, Option<usize>)> = vec![(root, None)];
        while let Some((mut i, parent)) = stack.pop() {
            loop {
                if out.len() >= MAX_RECORDS || visited[i] {
                    break;
                }
                visited[i] = true;
                out.push((i, parent));
                let next = offset(&records[i], NEXT_RECORD);
                if let Some(lower) = offset(&records[i], LOWER_RECORD) {
                    if let Some(n) = next {
                        stack.push((n, parent));
                    }
                    stack.push((lower, Some(i)));
                    break;
                }
                match next {
                    Some(n) => i = n,
                    None => break,
                }
            }
        }
        // Offsets that reach few of the records are broken: use the sequence order.
        if out.len() * 2 > records.len() {
            return out;
        }
        out.clear();
    }
    let (mut patient, mut study, mut series) = (None, None, None);
    for (i, r) in records.iter().enumerate() {
        let parent = match level(r) {
            Level::Patient => {
                (patient, study, series) = (Some(i), None, None);
                None
            }
            Level::Study => {
                (study, series) = (Some(i), None);
                patient
            }
            Level::Series => {
                series = Some(i);
                study
            }
            // Records that are only ever at the top level.
            Level::Leaf
                if matches!(
                    v_text(v, r, RECORD_TYPE).as_str(),
                    "HANGING PROTOCOL" | "PALETTE" | "IMPLANT" | "IMPLANT GROUP" | "IMPLANT ASSY"
                ) =>
            {
                None
            }
            Level::Leaf => series,
        };
        out.push((i, parent));
    }
    out
}

fn v_text(v: &Values, ds: &DataSet, tag: u32) -> String {
    ds.get(tag)
        .map(|e| String::from_utf8_lossy(v.text_bytes(e)).trim().to_owned())
        .unwrap_or_default()
}

/// Leaf record types with images.
fn is_image_record(kind: &str) -> bool {
    matches!(kind, "IMAGE" | "RT DOSE" | "" | "PRIVATE")
}

/// Reads the DICOMDIR at `path` and finds its files (relative to its folder).
pub fn read_dicomdir(path: &Path) -> Result<Medium> {
    let data = std::fs::read(path).map_err(|_| Error::Truncated)?;
    let root = path.parent().unwrap_or(Path::new("."));
    let mut m = dicomdir_medium(&data, root, &mut Resolver::default())?;
    m.dicomdir = Some(path.to_path_buf());
    Ok(m)
}

/// [`read_dicomdir`] of DICOMDIR bytes, its files resolved below `root`.
pub fn dicomdir_medium(data: &[u8], root: &Path, resolver: &mut Resolver) -> Result<Medium> {
    let d = Dicom::parse(data)?;
    let v = d.values(data);
    let mut m = Medium::default();
    let Some(seq) = d.dataset.get(DIRECTORY_RECORDS) else {
        return Ok(m);
    };
    let records = &seq.items;
    let top = Charset::of(&d.dataset, &v);
    let tree = record_tree(&d, &v, records);
    // Indices into the medium for patient / study / series records.
    let mut at: HashMap<usize, (usize, usize, usize)> = HashMap::new();
    let mut icons: Vec<(usize, usize, usize, usize)> = Vec::new();
    let mut parents: HashMap<usize, Option<usize>> = HashMap::new();
    for &(i, parent) in &tree {
        parents.insert(i, parent);
    }
    // A record's charset: its own, else the DICOMDIR's.
    let text = |r: &DataSet, tag: u32| -> String {
        let cs = top.within(r, &v);
        r.get(tag)
            .map(|e| cs.decode(v.text_bytes(e), e.vr).trim().to_owned())
            .unwrap_or_default()
    };
    let level = |r: &DataSet| match v_text(&v, r, RECORD_TYPE).as_str() {
        "PATIENT" => Level::Patient,
        "STUDY" => Level::Study,
        "SERIES" => Level::Series,
        _ => Level::Leaf,
    };
    // The nearest ancestor (or self) of a level.
    let ancestor = |mut i: usize, want: Level| -> Option<usize> {
        for _ in 0..8 {
            if level(&records[i]) == want {
                return Some(i);
            }
            i = (*parents.get(&i)?)?;
        }
        None
    };
    for &(i, _) in &tree {
        let r = &records[i];
        let lv = level(r);
        if lv == Level::Patient {
            continue;
        }
        // Top-level records that are not part of a patient's tree (hanging protocols,
        // private records) are left out.
        let study = ancestor(i, Level::Study);
        let series = ancestor(i, Level::Series);
        if (lv == Level::Series && study.is_none()) || (lv == Level::Leaf && series.is_none()) {
            continue;
        }
        // Make sure the record's patient / study / series exist (records may be missing).
        let patient = ancestor(i, Level::Patient);
        let pkey = patient.unwrap_or(usize::MAX);
        let p = match at.get(&pkey).map(|x| x.0) {
            Some(p) => p,
            None => {
                let key = pkey;
                let pr = patient.map(|p| &records[p]);
                m.patients.push(Patient {
                    name: pr.map(|r| text(r, PATIENT_NAME)).unwrap_or_default(),
                    id: pr.map(|r| text(r, PATIENT_ID)).unwrap_or_default(),
                    birth_date: pr.map(|r| text(r, BIRTH_DATE)).unwrap_or_default(),
                    studies: Vec::new(),
                });
                let p = m.patients.len() - 1;
                at.insert(key, (p, usize::MAX, usize::MAX));
                p
            }
        };
        {
            let key = study.unwrap_or(usize::MAX - 1 - p);
            let st = match at.get(&key).map(|x| x.1) {
                Some(s) => s,
                None => {
                    let sr = study.map(|s| &records[s]);
                    let studies = &mut m.patients[p].studies;
                    studies.push(Study {
                        uid: sr.map(|r| text(r, STUDY_UID)).unwrap_or_default(),
                        date: sr.map(|r| text(r, STUDY_DATE)).unwrap_or_default(),
                        description: sr.map(|r| text(r, STUDY_DESCRIPTION)).unwrap_or_default(),
                        series: Vec::new(),
                    });
                    let s = studies.len() - 1;
                    at.insert(key, (p, s, usize::MAX));
                    s
                }
            };
            if lv == Level::Study {
                continue;
            }
            let key = series.unwrap_or(usize::MAX - 1_000_000 - st);
            let se = match at.get(&key).map(|x| x.2) {
                Some(s) => s,
                None => {
                    let sr = series.map(|s| &records[s]);
                    let list = &mut m.patients[p].studies[st].series;
                    list.push(MediaSeries {
                        uid: sr.map(|r| text(r, SERIES_UID)).unwrap_or_default(),
                        modality: sr.map(|r| text(r, MODALITY)).unwrap_or_default(),
                        number: sr
                            .and_then(|r| r.get(SERIES_NUMBER))
                            .and_then(|e| v.number(e, 0))
                            .map(|n| n as i64),
                        description: sr.map(|r| text(r, SERIES_DESCRIPTION)).unwrap_or_default(),
                        ..MediaSeries::default()
                    });
                    let s = list.len() - 1;
                    at.insert(key, (p, st, s));
                    if let Some(sr) = series
                        && records[sr].get(ICON_IMAGE_SEQUENCE).is_some()
                    {
                        icons.push((sr, p, st, s));
                    }
                    s
                }
            };
            if lv == Level::Series {
                continue;
            }
            let Some(e) = r.get(REFERENCED_FILE) else {
                continue;
            };
            let id = file_id(&String::from_utf8_lossy(v.text_bytes(e)));
            if id.is_empty() {
                continue;
            }
            let found = resolver.resolve(root, &id);
            let mut path = root.to_path_buf();
            if found.is_none() {
                id.iter().for_each(|c| path.push(c));
            }
            let file = MediaFile {
                path: found.clone().unwrap_or(path),
                present: found.is_some(),
                sop_uid: v_text(&v, r, REFERENCED_SOP),
                instance_number: r
                    .get(INSTANCE_NUMBER)
                    .and_then(|e| v.number(e, 0))
                    .map(|n| n as i64),
                frames: r
                    .get(NUMBER_OF_FRAMES)
                    .and_then(|e| v.number(e, 0))
                    .map_or(1, |n| n.max(1.0) as u32),
            };
            if !file.present {
                m.missing += 1;
            }
            let s = &mut m.patients[p].studies[st].series[se];
            let kind = v_text(&v, r, RECORD_TYPE);
            if is_image_record(&kind) {
                if s.images.is_empty() && r.get(ICON_IMAGE_SEQUENCE).is_some() {
                    icons.push((i, p, st, se));
                }
                s.images.push(file);
            } else {
                s.others.push(file);
            }
        }
    }
    for (r, p, st, se) in icons {
        let s = &mut m.patients[p].studies[st].series[se];
        if s.icon.is_none()
            && let Some(Ok(img)) = d.icon_in(&records[r], data)
        {
            s.icon = Some(thumb(&img, 128));
        }
    }
    m.sort();
    Ok(m)
}

// ---------------------------------------------------------------------------------- scan

/// What a folder scan reads from one file. `Debug` leaves out the patient fields.
#[derive(Clone, Default, PartialEq)]
pub struct FileHeader {
    pub path: PathBuf,
    pub patient_name: String,
    pub patient_id: String,
    pub birth_date: String,
    pub study_uid: String,
    pub study_date: String,
    pub study_description: String,
    pub series_uid: String,
    pub modality: String,
    pub series_number: Option<i64>,
    pub series_description: String,
    pub sop_uid: String,
    pub instance_number: Option<i64>,
    pub frames: u32,
    /// The file has pixel data before the header ends (an image, not a report or a
    /// presentation state).
    pub image: bool,
}

impl fmt::Debug for FileHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileHeader")
            .field("modality", &self.modality)
            .field("series_number", &self.series_number)
            .field("instance_number", &self.instance_number)
            .finish_non_exhaustive()
    }
}

impl FileHeader {
    fn read(ds: &DataSet, v: &Values, stopped_at: Option<u32>) -> FileHeader {
        let cs = Charset::of(ds, v);
        let text = |tag: u32| {
            ds.get(tag)
                .map(|e| cs.decode(v.text_bytes(e), e.vr).trim().to_owned())
                .unwrap_or_default()
        };
        let int = |tag: u32| ds.get(tag).and_then(|e| v.number(e, 0)).map(|n| n as i64);
        FileHeader {
            path: PathBuf::new(),
            patient_name: text(PATIENT_NAME),
            patient_id: text(PATIENT_ID),
            birth_date: text(BIRTH_DATE),
            study_uid: text(STUDY_UID),
            study_date: text(STUDY_DATE),
            study_description: text(STUDY_DESCRIPTION),
            series_uid: text(SERIES_UID),
            modality: text(MODALITY),
            series_number: int(SERIES_NUMBER),
            series_description: text(SERIES_DESCRIPTION),
            sop_uid: text(SOP_UID),
            instance_number: int(INSTANCE_NUMBER),
            frames: int(NUMBER_OF_FRAMES).map_or(1, |n| n.max(1) as u32),
            image: stopped_at.is_some_and(|t| (0x7FE0_0008..=0x7FE0_0010).contains(&t)),
        }
    }
}

/// Bounds of a folder scan.
#[derive(Clone, Copy, Debug)]
pub struct ScanLimits {
    /// Files whose headers are read, at most.
    pub files: usize,
    /// Folder levels below the root.
    pub depth: usize,
    /// Longest wait between batches handed over.
    pub batch_every: Duration,
}

impl Default for ScanLimits {
    fn default() -> Self {
        ScanLimits {
            files: 100_000,
            depth: 12,
            batch_every: Duration::from_millis(150),
        }
    }
}

/// How far a scan is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// Files read so far.
    pub done: usize,
    /// Candidate files in all.
    pub total: usize,
    /// Files that were not DICOM or could not be read.
    pub skipped: usize,
    /// More files than [`ScanLimits::files`]: the rest are not read.
    pub capped: bool,
}

/// Whether a file found by a folder scan might be DICOM (before reading it).
fn candidate(name: &str) -> bool {
    if name.starts_with('.') || is_dicomdir_name(std::ffi::OsStr::new(name)) {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    if lower == "autorun" || lower.starts_with("autorun.") || lower == "thumbs.db" {
        return false;
    }
    match Path::new(&lower).extension().and_then(|e| e.to_str()) {
        Some(e) => !NOT_DICOM.contains(&e),
        None => true,
    }
}

/// The candidate files below `root`, in natural order folder by folder.
fn candidates(root: &Path, limits: &ScanLimits, cancel: &AtomicBool) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        // Folder loops through links.
        if let Ok(c) = dir.canonicalize()
            && !seen.insert(c)
        {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut files = Vec::new();
        let mut dirs = Vec::new();
        for e in rd.flatten() {
            let name = e.file_name();
            let s = name.to_string_lossy();
            if s.starts_with('.') {
                continue;
            }
            let p = e.path();
            if p.is_dir() {
                if depth < limits.depth {
                    dirs.push(p);
                }
            } else if candidate(&s) {
                files.push(p);
            }
        }
        files.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
        out.extend(files);
        if out.len() > limits.files {
            break;
        }
        dirs.sort_by(|a, b| natural_cmp(&b.to_string_lossy(), &a.to_string_lossy()));
        stack.extend(dirs.into_iter().map(|d| (d, depth + 1)));
    }
    Ok(out)
}

/// Reads the headers of the DICOM files below `root` (only the headers, never the pixel
/// data), handing them to `emit` in batches with the progress so far; the last call has every
/// file read. `Err(Cancelled)` once `cancel` is set.
pub fn scan(
    root: &Path,
    limits: ScanLimits,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(Vec<FileHeader>, Progress),
) -> Result<()> {
    let mut files = candidates(root, &limits, cancel)?;
    let mut progress = Progress::default();
    if files.len() > limits.files {
        files.truncate(limits.files);
        progress.capped = true;
    }
    progress.total = files.len();
    emit(Vec::new(), progress);
    let mut batch = Vec::new();
    let mut last = Instant::now();
    for p in files {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        match read_header_with(&p, &FileHeader::read) {
            Ok(Some(mut h)) if !h.study_uid.is_empty() || !h.series_uid.is_empty() => {
                h.path = p;
                batch.push(h);
            }
            _ => progress.skipped += 1,
        }
        progress.done += 1;
        if last.elapsed() >= limits.batch_every {
            emit(std::mem::take(&mut batch), progress);
            last = Instant::now();
        }
    }
    emit(batch, progress);
    Ok(())
}

// ------------------------------------------------------------------------------ thumbnails

/// A display image scaled to fit `max` x `max` (box filter), as 8-bit RGBA.
pub fn thumb(img: &Image, max: u32) -> Thumb {
    let (w, h) = (img.width.max(1), img.height.max(1));
    let scale = (max as f64 / w.max(h) as f64).min(1.0);
    let tw = ((w as f64 * scale).round() as u32).max(1);
    let th = ((h as f64 * scale).round() as u32).max(1);
    let px = |x: u32, y: u32| -> [u32; 4] {
        let i = (y * img.width + x) as usize;
        match &img.pixels {
            Rendered::Gray16(g) => {
                let v = g.get(i).map_or(0, |v| u32::from(*v >> 8));
                [v, v, v, 255]
            }
            Rendered::Rgba8(c) => {
                let p = c.get(i * 4..i * 4 + 4).unwrap_or(&[0, 0, 0, 255]);
                [p[0].into(), p[1].into(), p[2].into(), 255]
            }
            Rendered::Rgba16(c) => {
                let p = c.get(i * 4..i * 4 + 4).unwrap_or(&[0, 0, 0, 65535]);
                [
                    u32::from(p[0] >> 8),
                    u32::from(p[1] >> 8),
                    u32::from(p[2] >> 8),
                    255,
                ]
            }
        }
    };
    let mut rgba = Vec::with_capacity((tw * th * 4) as usize);
    for ty in 0..th {
        let y0 = (ty as u64 * h as u64 / th as u64) as u32;
        let y1 = (((ty + 1) as u64 * h as u64).div_ceil(th as u64) as u32).clamp(y0 + 1, h);
        for tx in 0..tw {
            let x0 = (tx as u64 * w as u64 / tw as u64) as u32;
            let x1 = (((tx + 1) as u64 * w as u64).div_ceil(tw as u64) as u32).clamp(x0 + 1, w);
            // At most 8 x 8 samples per output pixel.
            let sx = (x1 - x0).div_ceil(8).max(1);
            let sy = (y1 - y0).div_ceil(8).max(1);
            let (mut acc, mut n) = ([0u32; 4], 0u32);
            let mut y = y0;
            while y < y1 {
                let mut x = x0;
                while x < x1 {
                    let p = px(x.min(img.width - 1), y.min(img.height - 1));
                    for k in 0..4 {
                        acc[k] += p[k];
                    }
                    n += 1;
                    x += sx;
                }
                y += sy;
            }
            for a in acc {
                rgba.push((a / n.max(1)) as u8);
            }
        }
    }
    Thumb {
        width: tw,
        height: th,
        rgba,
    }
}

/// A thumbnail of a DICOM file: its Icon Image Sequence, else its middle frame rendered with
/// the default display mapping.
pub fn file_thumb(path: &Path, max: u32, cancel: &AtomicBool) -> Result<Thumb> {
    let data = std::fs::read(path).map_err(|_| Error::Truncated)?;
    let d = Dicom::parse(&data)?;
    if let Some(Ok(icon)) = d.icon(&data) {
        return Ok(thumb(&icon, max));
    }
    let frames = d.frame_count();
    if frames == 0 {
        return Err(Error::NoPixelData);
    }
    let m = d.modality_frame(&data, frames / 2, Some(cancel))?;
    Ok(thumb(&m.render_default()?, max))
}

#[cfg(test)]
#[path = "media_tests.rs"]
mod tests;
