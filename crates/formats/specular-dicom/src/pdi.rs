//! IHE Portable Data for Imaging (PDI) export: a folder ready for a CD, DVD or USB stick.
//!
//! Layout (IHE RAD TF-1 15, PS3.11 STD-GEN-CD / USB):
//!
//! ```text
//! DICOMDIR            Basic Directory: PATIENT / STUDY / SERIES / IMAGE records
//! DICOM/P0000001/S0000001/E0000001/I0000001 …   the instances, one Part 10 file each
//! README.TXT          what the medium holds and who wrote it
//! INDEX.HTM           the same, for a web browser
//! ```
//!
//! File IDs are at most 8 components of at most 8 characters from `A-Z 0-9 _` (PS3.10 8.2,
//! PS3.12 F.2). Instances are written as they arrive ([`Disc::add`]); [`Disc::finish`] writes
//! the directory and the index files. No viewer is bundled; [`ViewerBundle`] is the seam for
//! one.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

use crate::dict::Vr;
use crate::write::{self, FileMeta, Object, Track, Value};
use crate::{Dicom, Error};

/// Media Storage Directory Storage.
pub const DIRECTORY_SOP_CLASS: &str = "1.2.840.10008.1.3.10";

/// Why the export failed.
#[derive(Debug, thiserror::Error)]
pub enum PdiError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Dicom(#[from] Error),
    /// The output folder already holds a DICOMDIR or DICOM folder.
    #[error("the folder already holds a DICOM file-set")]
    NotEmpty,
    /// An instance without a SOP, Study or Series Instance UID cannot be referenced.
    #[error("no SOP, Study or Series Instance UID")]
    NoInstanceUid,
}

/// A viewer to put on the medium next to the file-set (the PDI optional viewer).
pub trait ViewerBundle {
    /// Copies the viewer under `root`; returns the lines README.TXT shows about it.
    fn install(&self, root: &Path) -> io::Result<Vec<String>>;
}

#[derive(Default)]
struct Instance {
    id: Vec<String>,
    sop_class: String,
    sop_instance: String,
    transfer_syntax: String,
    number: String,
}

#[derive(Default)]
struct SeriesRec {
    uid: String,
    modality: String,
    number: String,
    description: String,
    instances: Vec<Instance>,
}

#[derive(Default)]
struct StudyRec {
    uid: String,
    date: String,
    time: String,
    description: String,
    id: String,
    accession: String,
    series: Vec<SeriesRec>,
}

#[derive(Default)]
struct PatientRec {
    key: String,
    name: String,
    id: String,
    birth: String,
    charset: Option<Vec<u8>>,
    studies: Vec<StudyRec>,
}

/// What [`Disc::finish`] wrote.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Written {
    pub patients: usize,
    pub studies: usize,
    pub series: usize,
    pub instances: usize,
    /// Instances skipped because the same SOP Instance UID was already added.
    pub duplicates: usize,
    pub bytes: u64,
}

/// A PDI folder being written.
pub struct Disc {
    root: PathBuf,
    patients: Vec<PatientRec>,
    seen: HashMap<String, ()>,
    written: Written,
    anonymised: bool,
}

fn component(prefix: char, n: usize) -> String {
    format!("{prefix}{n:07}")
}

impl Disc {
    /// Starts a file-set in `root` (created if missing; refused if it already holds one).
    /// `anonymised` is said in the index files.
    pub fn create(root: &Path, anonymised: bool) -> Result<Disc, PdiError> {
        if root.join("DICOMDIR").exists() || root.join("DICOM").exists() {
            return Err(PdiError::NotEmpty);
        }
        std::fs::create_dir_all(root.join("DICOM"))?;
        Ok(Disc {
            root: root.to_owned(),
            patients: Vec::new(),
            seen: HashMap::new(),
            written: Written::default(),
            anonymised,
        })
    }

    /// Adds one instance (a Part 10 file, or a bare data set); returns where it was written.
    /// Files in Explicit VR Little Endian or an encapsulated syntax are copied as they are;
    /// others (implicit VR, big endian, deflated, no File Meta) are rewritten as Explicit VR
    /// Little Endian.
    pub fn add(&mut self, file: &[u8]) -> Result<Option<PathBuf>, PdiError> {
        let d = Dicom::parse(file)?;
        let obj = Object::from_dataset(&d.dataset, d.buf(file));
        let t = |tag: u32| obj.text(tag).unwrap_or_default().trim().to_owned();
        let sop_instance = t(0x0008_0018);
        if sop_instance.is_empty() || t(0x0020_000D).is_empty() || t(0x0020_000E).is_empty() {
            return Err(PdiError::NoInstanceUid);
        }
        if self.seen.insert(sop_instance.clone(), ()).is_some() {
            self.written.duplicates += 1;
            return Ok(None);
        }
        let as_is = file.get(128..132) == Some(b"DICM")
            && d.transfer_syntax.encoding == crate::Encoding::ExplicitLittle
            && !d.meta.elements.is_empty();
        // As it is, or encapsulated pixel data kept in its transfer syntax.
        let encapsulated = matches!(
            obj.get(0x7FE0_0010).map(|e| &e.value),
            Some(Value::Fragments { .. })
        );
        let ts = if as_is || encapsulated {
            d.transfer_syntax.uid.clone()
        } else {
            write::EXPLICIT_LE.to_owned()
        };

        let pkey = format!("{}\0{}", t(0x0010_0020), t(0x0010_0010));
        let pi = match self.patients.iter().position(|p| p.key == pkey) {
            Some(i) => i,
            None => {
                self.patients.push(PatientRec {
                    key: pkey,
                    name: t(0x0010_0010),
                    id: t(0x0010_0020),
                    birth: t(0x0010_0030),
                    charset: obj.get(0x0008_0005).map(|e| e.bytes().to_vec()),
                    studies: Vec::new(),
                });
                self.patients.len() - 1
            }
        };
        let study_uid = t(0x0020_000D);
        let p = &mut self.patients[pi];
        let si = match p.studies.iter().position(|s| s.uid == study_uid) {
            Some(i) => i,
            None => {
                p.studies.push(StudyRec {
                    uid: study_uid,
                    date: t(0x0008_0020),
                    time: t(0x0008_0030),
                    description: t(0x0008_1030),
                    id: t(0x0020_0010),
                    accession: t(0x0008_0050),
                    series: Vec::new(),
                });
                p.studies.len() - 1
            }
        };
        let series_uid = t(0x0020_000E);
        let s = &mut p.studies[si];
        let ei = match s.series.iter().position(|e| e.uid == series_uid) {
            Some(i) => i,
            None => {
                s.series.push(SeriesRec {
                    uid: series_uid,
                    modality: t(0x0008_0060),
                    number: t(0x0020_0011),
                    description: t(0x0008_103E),
                    instances: Vec::new(),
                });
                s.series.len() - 1
            }
        };
        let e = &mut s.series[ei];
        let id = vec![
            "DICOM".to_owned(),
            component('P', pi + 1),
            component('S', si + 1),
            component('E', ei + 1),
            component('I', e.instances.len() + 1),
        ];
        let path = id.iter().fold(self.root.clone(), |p, c| p.join(c));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let bytes = if as_is {
            std::fs::write(&path, file)?;
            file.len()
        } else {
            let out = write::part10(&obj, &FileMeta::of(&obj, &ts))?;
            std::fs::write(&path, &out)?;
            out.len()
        };
        self.written.bytes += bytes as u64;
        self.written.instances += 1;
        e.instances.push(Instance {
            id,
            sop_class: t(0x0008_0016),
            sop_instance,
            transfer_syntax: ts,
            number: t(0x0020_0013),
        });
        Ok(Some(path))
    }

    /// Writes DICOMDIR, README.TXT and INDEX.HTM (and the viewer, if any).
    pub fn finish(mut self, viewer: Option<&dyn ViewerBundle>) -> Result<Written, PdiError> {
        let viewer_lines = match viewer {
            Some(v) => v.install(&self.root)?,
            None => Vec::new(),
        };
        let dir = directory(&self.patients, &dicomdir_uid(&self.patients))?;
        std::fs::write(self.root.join("DICOMDIR"), &dir)?;
        self.written.bytes += dir.len() as u64;
        self.written.patients = self.patients.len();
        self.written.studies = self.patients.iter().map(|p| p.studies.len()).sum();
        self.written.series = self
            .patients
            .iter()
            .flat_map(|p| &p.studies)
            .map(|s| s.series.len())
            .sum();
        std::fs::write(
            self.root.join("README.TXT"),
            readme(&self.written, self.anonymised, &viewer_lines),
        )?;
        std::fs::write(
            self.root.join("INDEX.HTM"),
            index(&self.patients, self.anonymised),
        )?;
        Ok(std::mem::take(&mut self.written))
    }
}

/// A UID for the DICOMDIR instance, from what it lists (stable for the same content).
fn dicomdir_uid(patients: &[PatientRec]) -> String {
    let mut h = crate::hash::Sha256::new();
    for i in patients
        .iter()
        .flat_map(|p| &p.studies)
        .flat_map(|s| &s.series)
        .flat_map(|e| &e.instances)
    {
        h.update(i.sop_instance.as_bytes());
        h.update(b"\0");
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    h.update(&now.to_le_bytes());
    let d = h.finish();
    let mut b: [u8; 16] = d[..16].try_into().unwrap_or([0; 16]);
    b[6] = (b[6] & 0x0F) | 0x80;
    b[8] = (b[8] & 0x3F) | 0x80;
    format!("2.25.{}", u128::from_be_bytes(b))
}

fn record_type(sop_class: &str) -> &'static str {
    const P: &str = "1.2.840.10008.5.1.4.1.1.";
    let Some(rest) = sop_class.strip_prefix(P) else {
        return "IMAGE";
    };
    let major = rest.split('.').next().unwrap_or("");
    match major {
        "11" => "PRESENTATION",
        "88" => "SR DOCUMENT",
        "104" => "ENCAP DOC",
        "9" => "WAVEFORM",
        "66" => {
            if rest.starts_with("66.1") {
                "REGISTRATION"
            } else if rest.starts_with("66.2") {
                "FIDUCIAL"
            } else if rest.starts_with("66.4") {
                "IMAGE"
            } else {
                "RAW DATA"
            }
        }
        "481" => match rest {
            "481.2" => "RT DOSE",
            "481.3" => "RT STRUCTURE SET",
            "481.4" => "RT TREAT RECORD",
            "481.5" | "481.8" => "RT PLAN",
            _ => "IMAGE",
        },
        _ => "IMAGE",
    }
}

fn ul(v: u32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// One directory record with zeroed links (filled on the second pass).
fn record(kind: &str) -> Object<'static> {
    let mut r = Object::default();
    r.put_bytes(0x0004_1400, Vr::UL, ul(0));
    r.put_bytes(0x0004_1410, Vr::US, 0xFFFFu16.to_le_bytes().to_vec());
    r.put_bytes(0x0004_1420, Vr::UL, ul(0));
    r.put_text(0x0004_1430, Vr::CS, kind);
    r
}

/// Per record: (next sibling, first child) indices.
type Links = Vec<(Option<usize>, Option<usize>)>;

/// Records in file order with their links.
fn records(patients: &[PatientRec]) -> (Vec<Object<'static>>, Links) {
    let mut recs = Vec::new();
    let mut links: Links = Vec::new();
    // Indices of the previous sibling at each level, to link it to the next.
    fn link(links: &mut [(Option<usize>, Option<usize>)], prev: &mut Option<usize>, me: usize) {
        if let Some(p) = *prev {
            links[p].0 = Some(me);
        }
        *prev = Some(me);
    }
    let mut prev_p = None;
    for p in patients {
        let pi = recs.len();
        let mut r = record("PATIENT");
        if let Some(cs) = &p.charset {
            r.put_bytes(0x0008_0005, Vr::CS, cs.clone());
        }
        r.put_text(0x0010_0010, Vr::PN, &p.name);
        r.put_text(0x0010_0020, Vr::LO, &p.id);
        if !p.birth.is_empty() {
            r.put_text(0x0010_0030, Vr::DA, &p.birth);
        }
        recs.push(r);
        links.push((None, None));
        link(&mut links, &mut prev_p, pi);
        let mut prev_s = None;
        for s in &p.studies {
            let si = recs.len();
            let mut r = record("STUDY");
            if let Some(cs) = &p.charset {
                r.put_bytes(0x0008_0005, Vr::CS, cs.clone());
            }
            r.put_text(0x0008_0020, Vr::DA, &s.date);
            r.put_text(0x0008_0030, Vr::TM, &s.time);
            r.put_text(0x0008_0050, Vr::SH, &s.accession);
            r.put_text(0x0008_1030, Vr::LO, &s.description);
            r.put_text(0x0020_000D, Vr::UI, &s.uid);
            r.put_text(0x0020_0010, Vr::SH, &s.id);
            recs.push(r);
            links.push((None, None));
            if prev_s.is_none() {
                links[pi].1 = Some(si);
            }
            link(&mut links, &mut prev_s, si);
            let mut prev_e = None;
            for e in &s.series {
                let ei = recs.len();
                let mut r = record("SERIES");
                if let Some(cs) = &p.charset {
                    r.put_bytes(0x0008_0005, Vr::CS, cs.clone());
                }
                r.put_text(0x0008_0060, Vr::CS, &e.modality);
                if !e.description.is_empty() {
                    r.put_text(0x0008_103E, Vr::LO, &e.description);
                }
                r.put_text(0x0020_000E, Vr::UI, &e.uid);
                r.put_text(0x0020_0011, Vr::IS, &e.number);
                recs.push(r);
                links.push((None, None));
                if prev_e.is_none() {
                    links[si].1 = Some(ei);
                }
                link(&mut links, &mut prev_e, ei);
                let mut prev_i = None;
                for i in &e.instances {
                    let ii = recs.len();
                    let mut r = record(record_type(&i.sop_class));
                    r.put_text(0x0004_1500, Vr::CS, &i.id.join("\\"));
                    r.put_text(0x0004_1510, Vr::UI, &i.sop_class);
                    r.put_text(0x0004_1511, Vr::UI, &i.sop_instance);
                    r.put_text(0x0004_1512, Vr::UI, &i.transfer_syntax);
                    r.put_text(0x0020_0013, Vr::IS, &i.number);
                    recs.push(r);
                    links.push((None, None));
                    if prev_i.is_none() {
                        links[ei].1 = Some(ii);
                    }
                    link(&mut links, &mut prev_i, ii);
                }
            }
        }
    }
    (recs, links)
}

/// The DICOMDIR file.
fn directory(patients: &[PatientRec], uid: &str) -> Result<Vec<u8>, PdiError> {
    let (mut recs, links) = records(patients);
    let meta = FileMeta {
        sop_class: DIRECTORY_SOP_CLASS.to_owned(),
        sop_instance: uid.to_owned(),
        transfer_syntax: write::EXPLICIT_LE.to_owned(),
    };
    let build = |recs: &[Object<'static>], first: u32, last: u32| {
        let mut d = Object::default();
        d.put_text(0x0004_1130, Vr::CS, "SPECULAR");
        d.put_bytes(0x0004_1200, Vr::UL, ul(first));
        d.put_bytes(0x0004_1202, Vr::UL, ul(last));
        d.put_bytes(0x0004_1212, Vr::US, vec![0, 0]);
        d.put(0x0004_1220, Vr::SQ, Value::Items(recs.to_vec()));
        d
    };
    // Pass 1: where each record lands (links are fixed-size, so pass 2 keeps the layout).
    let mut track = Some(Track {
        tag: 0x0004_1220,
        offsets: Vec::new(),
    });
    write::part10_tracked(&build(&recs, 0, 0), &meta, &mut track)?;
    let offsets = track.map(|t| t.offsets).unwrap_or_default();
    let at = |i: Option<usize>| -> Result<u32, PdiError> {
        match i {
            None => Ok(0),
            Some(i) => offsets
                .get(i)
                .and_then(|&o| u32::try_from(o).ok())
                .ok_or(PdiError::Dicom(Error::TooLarge(
                    offsets.last().copied().unwrap_or(0) as u64,
                ))),
        }
    };
    for (r, &(next, child)) in recs.iter_mut().zip(&links) {
        r.put_bytes(0x0004_1400, Vr::UL, ul(at(next)?));
        r.put_bytes(0x0004_1420, Vr::UL, ul(at(child)?));
    }
    let roots: Vec<usize> = {
        let mut v = Vec::new();
        let mut i = if recs.is_empty() { None } else { Some(0) };
        while let Some(x) = i {
            v.push(x);
            i = links[x].0;
        }
        v
    };
    let first = at(roots.first().copied())?;
    let last = at(roots.last().copied())?;
    let mut track = None;
    Ok(write::part10_tracked(
        &build(&recs, first, last),
        &meta,
        &mut track,
    )?)
}

fn readme(w: &Written, anonymised: bool, viewer: &[String]) -> String {
    let mut s = String::new();
    let nl = "\r\n";
    let _ = write!(s, "DICOM medical images{nl}{nl}");
    let _ = write!(
        s,
        "This medium holds {} patient(s), {} study(ies), {} series and {} image(s) or other \
         DICOM objects, in the IHE Portable Data for Imaging (PDI) layout.{nl}{nl}",
        w.patients, w.studies, w.series, w.instances
    );
    if anonymised {
        let _ = write!(
            s,
            "The objects were de-identified (DICOM PS3.15 Annex E Basic Application Level \
             Confidentiality Profile). Names and IDs are pseudonyms.{nl}{nl}"
        );
    }
    let _ = write!(
        s,
        "DICOMDIR   the directory of the DICOM files (open it in a DICOM viewer){nl}\
         DICOM\\     the DICOM files{nl}\
         INDEX.HTM  this information for a web browser{nl}\
         README.TXT this file{nl}{nl}"
    );
    if viewer.is_empty() {
        let _ = write!(s, "No viewer is included. Use any DICOM viewer.{nl}{nl}");
    } else {
        for l in viewer {
            let _ = write!(s, "{l}{nl}");
        }
        s.push_str(nl);
    }
    let _ = write!(
        s,
        "Written by Specular {} (Implementation Class UID {}).{nl}",
        env!("CARGO_PKG_VERSION"),
        write::IMPLEMENTATION_CLASS_UID
    );
    s
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if c.is_ascii() && !c.is_ascii_control() => o.push(c),
            c if c.is_ascii_control() => o.push(' '),
            c => {
                let _ = write!(o, "&#{};", c as u32);
            }
        }
    }
    o
}

fn index(patients: &[PatientRec], anonymised: bool) -> String {
    let mut h = String::new();
    h.push_str(
        "<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\">\r\n<html><head>\
         <meta http-equiv=\"Content-Type\" content=\"text/html; charset=US-ASCII\">\
         <title>DICOM images</title></head><body>\r\n<h1>DICOM images</h1>\r\n",
    );
    if anonymised {
        h.push_str("<p>The images were de-identified; names and IDs are pseudonyms.</p>\r\n");
    }
    h.push_str(
        "<p>Open <a href=\"DICOMDIR\">DICOMDIR</a> in a DICOM viewer. \
         See also <a href=\"README.TXT\">README.TXT</a>.</p>\r\n",
    );
    for p in patients {
        let _ = write!(
            h,
            "<h2>{} ({})</h2>\r\n<ul>\r\n",
            esc(&p.name.replace('^', " ")),
            esc(&p.id)
        );
        for s in &p.studies {
            let _ = write!(
                h,
                "<li>{} {}: {}<ul>\r\n",
                esc(&s.date),
                esc(&s.description),
                esc(&s.id)
            );
            for e in &s.series {
                let _ = write!(
                    h,
                    "<li>{} {} {}: {} object(s), <a href=\"{}\">folder</a></li>\r\n",
                    esc(&e.modality),
                    esc(&e.number),
                    esc(&e.description),
                    e.instances.len(),
                    e.instances
                        .first()
                        .map(|i| i.id[..i.id.len() - 1].join("/"))
                        .unwrap_or_default()
                );
            }
            h.push_str("</ul></li>\r\n");
        }
        h.push_str("</ul>\r\n");
    }
    h.push_str("</body></html>\r\n");
    h
}
