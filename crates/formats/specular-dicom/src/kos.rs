//! Key Object Selection documents (PS3.3 A.35.4, template TID 2010): a manifest that
//! references instances and titles them, as IHE Teaching File and Clinical Trial Export
//! (TCE) uses with the title "For Teaching" (DCM 113004) and a Key Object Description
//! (DCM 113012) holding the case notes.
//!
//! The document joins the study of the instances it references (one study: the first one's
//! patient and study are used) as a series of its own (modality KO).

use crate::dict::Vr;
use crate::write::{self, FileMeta, Object, Value};
use crate::{Dicom, Error, Result};

pub const KEY_OBJECT_SELECTION: &str = "1.2.840.10008.5.1.4.1.1.88.59";

/// One instance referenced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    pub sop_class: String,
    pub sop_instance: String,
    pub series: String,
    pub study: String,
}

/// What a referenced file contributes: its reference, and the patient and study attributes
/// the document copies (already anonymised in a teaching export).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub reference: Reference,
    pub patient_name: String,
    pub patient_id: String,
    pub study_date: String,
    pub study_time: String,
    pub accession: String,
    pub study_id: String,
}

fn text(d: &Dicom, file: &[u8], tag: u32) -> String {
    let v = d.values(file);
    d.dataset
        .get(tag)
        .map(|e| {
            String::from_utf8_lossy(v.text_bytes(e))
                .trim_end_matches(['\0', ' '])
                .trim()
                .to_owned()
        })
        .unwrap_or_default()
}

/// Reads what a document needs from a Part 10 file.
pub fn source(file: &[u8]) -> Result<Source> {
    let d = Dicom::parse(file)?;
    let t = |tag| text(&d, file, tag);
    let reference = Reference {
        sop_class: t(0x0008_0016),
        sop_instance: t(0x0008_0018),
        series: t(0x0020_000E),
        study: t(0x0020_000D),
    };
    if reference.sop_class.is_empty()
        || reference.sop_instance.is_empty()
        || reference.series.is_empty()
        || reference.study.is_empty()
    {
        return Err(Error::Malformed("an instance without its UIDs"));
    }
    Ok(Source {
        reference,
        patient_name: t(0x0010_0010),
        patient_id: t(0x0010_0020),
        study_date: t(0x0008_0020),
        study_time: t(0x0008_0030),
        accession: t(0x0008_0050),
        study_id: t(0x0020_0010),
    })
}

/// A new UID under 2.25 (PS3.5 B.2), from 128 random bits.
pub fn new_uid() -> String {
    let k = crate::hash::random_key();
    let mut b: [u8; 16] = k[..16].try_into().unwrap_or([0; 16]);
    b[6] = (b[6] & 0x0F) | 0x40;
    b[8] = (b[8] & 0x3F) | 0x80;
    format!("2.25.{}", u128::from_be_bytes(b))
}

fn code(value: &str, scheme: &str, meaning: &str) -> Object<'static> {
    let mut o = Object::default();
    o.put_text(0x0008_0100, Vr::SH, value);
    o.put_text(0x0008_0102, Vr::SH, scheme);
    o.put_text(0x0008_0104, Vr::LO, meaning);
    o
}

fn items(v: Vec<Object<'static>>) -> Value<'static> {
    Value::Items(v)
}

/// The document, as Part 10 bytes (Explicit VR Little Endian, UTF-8): `title` is the
/// document title code (value, meaning; scheme DCM), `description` the Key Object
/// Description text (left out when empty), `sources` the instances (at least one, of one
/// study), `when` the content date and time (`YYYYMMDD`, `HHMMSS`).
pub fn document(
    sources: &[Source],
    title: (&str, &str),
    description: &str,
    when: (&str, &str),
) -> Result<Vec<u8>> {
    let first = sources
        .first()
        .ok_or(Error::Malformed("a key object document without references"))?;
    let study = &first.reference.study;
    let mut o = Object::default();
    o.put_text(0x0008_0005, Vr::CS, "ISO_IR 192");
    o.put_text(0x0008_0016, Vr::UI, KEY_OBJECT_SELECTION);
    let instance = new_uid();
    o.put_text(0x0008_0018, Vr::UI, &instance);
    // Patient and General Study (copied from the first instance).
    o.put_text(0x0010_0010, Vr::PN, &first.patient_name);
    o.put_text(0x0010_0020, Vr::LO, &first.patient_id);
    o.put_text(0x0010_0030, Vr::DA, "");
    o.put_text(0x0010_0040, Vr::CS, "");
    o.put_text(0x0020_000D, Vr::UI, study);
    o.put_text(0x0008_0020, Vr::DA, &first.study_date);
    o.put_text(0x0008_0030, Vr::TM, &first.study_time);
    o.put_text(0x0008_0090, Vr::PN, "");
    o.put_text(0x0020_0010, Vr::SH, &first.study_id);
    o.put_text(0x0008_0050, Vr::SH, &first.accession);
    // Key Object Document Series.
    o.put_text(0x0008_0060, Vr::CS, "KO");
    o.put_text(0x0020_000E, Vr::UI, &new_uid());
    o.put_text(0x0020_0011, Vr::IS, "999");
    o.put(0x0008_1111, Vr::SQ, items(Vec::new()));
    // General Equipment.
    o.put_text(0x0008_0070, Vr::LO, "");
    // Key Object Document.
    o.put_text(0x0020_0013, Vr::IS, "1");
    o.put_text(0x0008_0023, Vr::DA, when.0);
    o.put_text(0x0008_0033, Vr::TM, when.1);
    // Current Requested Procedure Evidence: study → series → instances.
    let mut series: Vec<(&str, Vec<&Reference>)> = Vec::new();
    for s in sources {
        if s.reference.study != *study {
            return Err(Error::Malformed("instances of more than one study"));
        }
        match series.iter_mut().find(|(u, _)| *u == s.reference.series) {
            Some((_, v)) => v.push(&s.reference),
            None => series.push((&s.reference.series, vec![&s.reference])),
        }
    }
    let sop_item = |r: &Reference| {
        let mut i = Object::default();
        i.put_text(0x0008_1150, Vr::UI, &r.sop_class);
        i.put_text(0x0008_1155, Vr::UI, &r.sop_instance);
        i
    };
    let series_items: Vec<Object<'static>> = series
        .iter()
        .map(|(uid, refs)| {
            let mut i = Object::default();
            i.put_text(0x0020_000E, Vr::UI, uid);
            i.put(
                0x0008_1199,
                Vr::SQ,
                items(refs.iter().map(|r| sop_item(r)).collect()),
            );
            i
        })
        .collect();
    let mut evidence = Object::default();
    evidence.put_text(0x0020_000D, Vr::UI, study);
    evidence.put(0x0008_1115, Vr::SQ, items(series_items));
    o.put(0x0040_A375, Vr::SQ, items(vec![evidence]));
    // SR Document Content: the root container (TID 2010).
    o.put_text(0x0040_A040, Vr::CS, "CONTAINER");
    o.put(
        0x0040_A043,
        Vr::SQ,
        items(vec![code(title.0, "DCM", title.1)]),
    );
    o.put_text(0x0040_A050, Vr::CS, "SEPARATE");
    let mut template = Object::default();
    template.put_text(0x0008_0105, Vr::CS, "DCMR");
    template.put_text(0x0040_DB00, Vr::CS, "2010");
    o.put(0x0040_A504, Vr::SQ, items(vec![template]));
    o.put(0x0040_A372, Vr::SQ, items(Vec::new()));
    let mut content = Vec::new();
    if !description.trim().is_empty() {
        let mut t = Object::default();
        t.put_text(0x0040_A010, Vr::CS, "CONTAINS");
        t.put_text(0x0040_A040, Vr::CS, "TEXT");
        t.put(
            0x0040_A043,
            Vr::SQ,
            items(vec![code("113012", "DCM", "Key Object Description")]),
        );
        // UT holds up to 2³²−2 bytes; notes are far shorter.
        t.put_text(0x0040_A160, Vr::UT, description.trim());
        content.push(t);
    }
    for s in sources {
        let mut i = Object::default();
        i.put_text(0x0040_A010, Vr::CS, "CONTAINS");
        let image = !s
            .reference
            .sop_class
            .starts_with("1.2.840.10008.5.1.4.1.1.88")
            && !s
                .reference
                .sop_class
                .starts_with("1.2.840.10008.5.1.4.1.1.104");
        i.put_text(
            0x0040_A040,
            Vr::CS,
            if image { "IMAGE" } else { "COMPOSITE" },
        );
        i.put(0x0008_1199, Vr::SQ, items(vec![sop_item(&s.reference)]));
        content.push(i);
    }
    o.put(0x0040_A730, Vr::SQ, items(content));
    write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(series: &str, n: u32) -> Vec<u8> {
        let mut o = Object::default();
        o.put_text(0x0008_0016, Vr::UI, "1.2.840.10008.5.1.4.1.1.2");
        o.put_text(0x0008_0018, Vr::UI, &format!("2.25.{n}"));
        o.put_text(0x0010_0010, Vr::PN, "ANON1234");
        o.put_text(0x0010_0020, Vr::LO, "ANON1234");
        o.put_text(0x0020_000D, Vr::UI, "2.25.1");
        o.put_text(0x0020_000E, Vr::UI, series);
        write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE)).unwrap()
    }

    #[test]
    fn manifest_references_every_instance_by_series() {
        let files = [
            instance("2.25.10", 100),
            instance("2.25.10", 101),
            instance("2.25.11", 102),
        ];
        let sources: Vec<Source> = files.iter().map(|f| source(f).unwrap()).collect();
        let b = document(
            &sources,
            ("113004", "For Teaching"),
            "Case: a finding.",
            ("20261025", "120000"),
        )
        .unwrap();
        let d = Dicom::parse(&b).unwrap();
        let v = d.values(&b);
        let t = |tag| {
            String::from_utf8_lossy(v.text_bytes(d.dataset.get(tag).unwrap()))
                .trim_end_matches(['\0', ' '])
                .to_string()
        };
        assert_eq!(t(0x0008_0016), KEY_OBJECT_SELECTION);
        assert_eq!(t(0x0008_0060), "KO");
        assert_eq!(t(0x0020_000D), "2.25.1");
        assert_eq!(t(0x0010_0010), "ANON1234");
        let ev = d.dataset.item(0x0040_A375).unwrap();
        let n_series = ev.get(0x0008_1115).map(|e| e.items.len()).unwrap();
        assert_eq!(n_series, 2);
        let content = &d.dataset.get(0x0040_A730).unwrap().items;
        // The description, then one item per instance.
        assert_eq!(content.len(), 4);
        // Another study: refused.
        let mut other = sources.clone();
        other[2].reference.study = "2.25.2".into();
        assert!(document(&other, ("113004", "For Teaching"), "", ("", "")).is_err());
        assert!(new_uid().starts_with("2.25.") && new_uid().len() <= 64);
    }
}
