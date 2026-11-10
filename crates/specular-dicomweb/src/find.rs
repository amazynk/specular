//! The same searches as [`crate::qido`] against a DIMSE server, by C-FIND
//! ([`specular_dimse::find`]): the study search form becomes Study Root keys, the matches
//! become [`StudyRow`]s and [`SeriesRow`]s.
//!
//! C-FIND has no paging: a page skips the matches before its offset and asks the server to
//! stop after it.

use specular_dimse::find::{self, Level, Match, Query};
use specular_dimse::{Options, Peer};
use specular_remote::CancelToken;

use crate::json::tags;
use crate::qido::{Page, Patient, SeriesRow, StudyQuery, StudyRow};

fn digits(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

/// The Study Root study-level query for `q` (matching keys as QIDO sends them, the
/// columns of the list as return keys).
pub fn study_query(q: &StudyQuery) -> Query {
    let mut keys: Vec<(u32, String)> = Vec::new();
    let name = q.patient_name.trim();
    if !name.is_empty() {
        let v = if name.contains(['*', '?']) {
            name.to_owned()
        } else {
            format!("{name}*")
        };
        keys.push((tags::PATIENT_NAME, v));
    }
    for (t, v) in [
        (tags::PATIENT_ID, &q.patient_id),
        (tags::ACCESSION, &q.accession),
        (tags::MODALITIES_IN_STUDY, &q.modality),
    ] {
        keys.push((t, v.trim().to_owned()));
    }
    let (a, b) = (digits(&q.date_from), digits(&q.date_to));
    keys.push((
        tags::STUDY_DATE,
        match (a.is_empty(), b.is_empty()) {
            (true, true) => String::new(),
            _ if a == b => a,
            _ => format!("{a}-{b}"),
        },
    ));
    for t in [
        tags::PATIENT_NAME,
        tags::STUDY_UID,
        tags::STUDY_TIME,
        tags::STUDY_DESCRIPTION,
        tags::STUDY_SERIES_COUNT,
        tags::STUDY_INSTANCE_COUNT,
        tags::PATIENT_BIRTH_DATE,
        tags::PATIENT_SEX,
        tags::REFERRING_PHYSICIAN,
    ] {
        if !keys.iter().any(|(k, _)| *k == t) {
            keys.push((t, String::new()));
        }
    }
    Query {
        level: Level::Study,
        keys,
    }
}

/// The series-level query for the series of `study`.
pub fn series_query(study: &str) -> Query {
    let mut keys = vec![(tags::STUDY_UID, study.trim().to_owned())];
    for t in [
        tags::SERIES_UID,
        tags::MODALITY,
        tags::SERIES_NUMBER,
        tags::SERIES_DESCRIPTION,
        tags::SERIES_INSTANCE_COUNT,
        tags::BODY_PART,
        tags::SERIES_DATE,
        tags::SERIES_TIME,
    ] {
        keys.push((t, String::new()));
    }
    Query {
        level: Level::Series,
        keys,
    }
}

fn count(m: &Match, tag: u32) -> Option<u32> {
    m.int(tag).and_then(|n| u32::try_from(n).ok())
}

impl StudyRow {
    pub fn from_match(m: &Match) -> StudyRow {
        StudyRow {
            study_uid: m.text(tags::STUDY_UID),
            date: m.text(tags::STUDY_DATE),
            time: m.text(tags::STUDY_TIME),
            description: m.text(tags::STUDY_DESCRIPTION),
            accession: m.text(tags::ACCESSION),
            modalities: m.texts(tags::MODALITIES_IN_STUDY),
            series_count: count(m, tags::STUDY_SERIES_COUNT),
            instance_count: count(m, tags::STUDY_INSTANCE_COUNT),
            patient: Patient {
                name: m.text(tags::PATIENT_NAME),
                id: m.text(tags::PATIENT_ID),
                birth_date: m.text(tags::PATIENT_BIRTH_DATE),
                sex: m.text(tags::PATIENT_SEX),
            },
        }
    }
}

impl SeriesRow {
    pub fn from_match(m: &Match) -> SeriesRow {
        SeriesRow {
            study_uid: m.text(tags::STUDY_UID),
            series_uid: m.text(tags::SERIES_UID),
            modality: m.text(tags::MODALITY),
            number: m.int(tags::SERIES_NUMBER),
            description: m.text(tags::SERIES_DESCRIPTION),
            instance_count: count(m, tags::SERIES_INSTANCE_COUNT),
            body_part: m.text(tags::BODY_PART),
            date: m.text(tags::SERIES_DATE),
            time: m.text(tags::SERIES_TIME),
        }
    }
}

/// Studies matching `q` (its `limit` and `offset` page the matches).
pub fn search_studies(
    peer: &Peer,
    opts: &Options,
    q: &StudyQuery,
    cancel: &CancelToken,
) -> specular_dimse::Result<Page<StudyRow>> {
    let f = find::find(
        peer,
        opts,
        &study_query(q),
        q.offset as usize,
        q.limit as usize,
        cancel,
    )?;
    let mut rows: Vec<StudyRow> = f.matches.iter().map(StudyRow::from_match).collect();
    rows.retain(|r| !r.study_uid.is_empty());
    Ok(Page {
        rows,
        offset: q.offset,
        more: f.more,
    })
}

/// The series of `study`, by series number.
pub fn search_series(
    peer: &Peer,
    opts: &Options,
    study: &str,
    limit: u32,
    offset: u32,
    cancel: &CancelToken,
) -> specular_dimse::Result<Page<SeriesRow>> {
    let f = find::find(
        peer,
        opts,
        &series_query(study),
        offset as usize,
        limit as usize,
        cancel,
    )?;
    let mut rows: Vec<SeriesRow> = f.matches.iter().map(SeriesRow::from_match).collect();
    for r in &mut rows {
        if r.study_uid.is_empty() {
            r.study_uid = study.to_owned();
        }
    }
    rows.retain(|r| !r.series_uid.is_empty());
    rows.sort_by_key(|r| r.number.unwrap_or(i64::MAX));
    Ok(Page {
        rows,
        offset,
        more: f.more,
    })
}
