//! QIDO-RS: search for studies, series and instances (PS3.18 10.6), a page at a time.

use specular_remote::CancelToken;

use crate::json::{JsonDataSet, parse_array, tags};
use crate::{DicomWeb, Result, uid};

/// Patient attributes of a study. Shown only behind "Show patient data"; `Debug` never prints
/// the values.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Patient {
    pub name: String,
    pub id: String,
    pub birth_date: String,
    pub sex: String,
}

impl std::fmt::Debug for Patient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Patient(<hidden>)")
    }
}

/// A study found by a search.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StudyRow {
    pub study_uid: String,
    /// `YYYYMMDD`.
    pub date: String,
    pub time: String,
    pub description: String,
    pub accession: String,
    pub modalities: Vec<String>,
    pub series_count: Option<u32>,
    pub instance_count: Option<u32>,
    pub patient: Patient,
}

/// A series of a study.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeriesRow {
    pub study_uid: String,
    pub series_uid: String,
    pub modality: String,
    pub number: Option<i64>,
    pub description: String,
    pub instance_count: Option<u32>,
    pub body_part: String,
    pub date: String,
    pub time: String,
}

/// An instance of a series.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstanceRow {
    pub sop_uid: String,
    pub sop_class: String,
    pub number: Option<i64>,
    pub frames: Option<u32>,
    pub rows: Option<u32>,
    pub columns: Option<u32>,
}

/// One page of results.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page<T> {
    pub rows: Vec<T>,
    /// Offset this page started at.
    pub offset: u32,
    /// The server probably has more (the page was full, or it said so).
    pub more: bool,
}

impl<T> Page<T> {
    /// Offset of the next page.
    pub fn next_offset(&self) -> u32 {
        self.offset + self.rows.len() as u32
    }
}

/// Study search filters. Empty fields are not sent.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct StudyQuery {
    /// Matched as a prefix (`smith` → `smith*`) unless it has wildcards already.
    pub patient_name: String,
    pub patient_id: String,
    /// `YYYYMMDD` (or `YYYY-MM-DD`); either end may be empty.
    pub date_from: String,
    pub date_to: String,
    pub modality: String,
    pub accession: String,
    pub limit: u32,
    pub offset: u32,
}

impl std::fmt::Debug for StudyQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The filters can identify a patient.
        f.debug_struct("StudyQuery")
            .field("modality", &self.modality)
            .field("limit", &self.limit)
            .field("offset", &self.offset)
            .finish_non_exhaustive()
    }
}

fn digits(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

impl StudyQuery {
    /// Query parameters, in a stable order.
    pub fn params(&self) -> Vec<(&'static str, String)> {
        let mut q = Vec::new();
        let name = self.patient_name.trim();
        if !name.is_empty() {
            let v = if name.contains(['*', '?']) {
                name.to_string()
            } else {
                format!("{name}*")
            };
            q.push(("PatientName", v));
        }
        for (k, v) in [
            ("PatientID", &self.patient_id),
            ("AccessionNumber", &self.accession),
            ("ModalitiesInStudy", &self.modality),
        ] {
            let v = v.trim();
            if !v.is_empty() {
                q.push((k, v.to_string()));
            }
        }
        let (a, b) = (digits(&self.date_from), digits(&self.date_to));
        if !a.is_empty() || !b.is_empty() {
            q.push(("StudyDate", if a == b { a } else { format!("{a}-{b}") }));
        }
        q.push((
            "includefield",
            "00081030,00080061,00201206,00201208,00100030,00100040,00080090".into(),
        ));
        paging(&mut q, self.limit, self.offset);
        q
    }
}

fn paging(q: &mut Vec<(&'static str, String)>, limit: u32, offset: u32) {
    if limit > 0 {
        q.push(("limit", limit.to_string()));
    }
    if offset > 0 {
        q.push(("offset", offset.to_string()));
    }
}

fn count(d: &JsonDataSet, tag: u32) -> Option<u32> {
    d.int(tag).and_then(|n| u32::try_from(n).ok())
}

impl StudyRow {
    pub fn from_json(d: &JsonDataSet) -> StudyRow {
        StudyRow {
            study_uid: d.text(tags::STUDY_UID),
            date: d.text(tags::STUDY_DATE),
            time: d.text(tags::STUDY_TIME),
            description: d.text(tags::STUDY_DESCRIPTION),
            accession: d.text(tags::ACCESSION),
            modalities: d.texts(tags::MODALITIES_IN_STUDY),
            series_count: count(d, tags::STUDY_SERIES_COUNT),
            instance_count: count(d, tags::STUDY_INSTANCE_COUNT),
            patient: Patient {
                name: d.text(tags::PATIENT_NAME),
                id: d.text(tags::PATIENT_ID),
                birth_date: d.text(tags::PATIENT_BIRTH_DATE),
                sex: d.text(tags::PATIENT_SEX),
            },
        }
    }
}

impl SeriesRow {
    pub fn from_json(d: &JsonDataSet) -> SeriesRow {
        SeriesRow {
            study_uid: d.text(tags::STUDY_UID),
            series_uid: d.text(tags::SERIES_UID),
            modality: d.text(tags::MODALITY),
            number: d.int(tags::SERIES_NUMBER),
            description: d.text(tags::SERIES_DESCRIPTION),
            instance_count: count(d, tags::SERIES_INSTANCE_COUNT),
            body_part: d.text(tags::BODY_PART),
            date: d.text(tags::SERIES_DATE),
            time: d.text(tags::SERIES_TIME),
        }
    }
}

impl InstanceRow {
    pub fn from_json(d: &JsonDataSet) -> InstanceRow {
        InstanceRow {
            sop_uid: d.text(tags::SOP_UID),
            sop_class: d.text(tags::SOP_CLASS),
            number: d.int(tags::INSTANCE_NUMBER),
            frames: count(d, tags::NUMBER_OF_FRAMES),
            rows: count(d, tags::ROWS),
            columns: count(d, tags::COLUMNS),
        }
    }
}

const JSON: &str = "application/dicom+json, application/json;q=0.9";

impl DicomWeb {
    fn qido(
        &self,
        segments: &[&str],
        params: &[(&str, String)],
        limit: u32,
        cancel: &CancelToken,
    ) -> Result<(Vec<JsonDataSet>, bool)> {
        let req = self
            .request(self.url(segments, params))
            .accept(JSON)
            .limit(256 << 20);
        let res = self.get(&req, cancel)?;
        let rows = if res.status == 204 {
            Vec::new()
        } else {
            parse_array(&res.body)?
        };
        // "299 …: There are more results" (PS3.18 8.3.4.1) or a full page.
        let warned = res
            .headers
            .get("warning")
            .is_some_and(|w| w.to_ascii_lowercase().contains("more"));
        let more = warned || (limit > 0 && rows.len() as u32 >= limit);
        Ok((rows, more))
    }

    /// `GET /studies?…`.
    pub fn search_studies(&self, q: &StudyQuery, cancel: &CancelToken) -> Result<Page<StudyRow>> {
        let (rows, more) = self.qido(&["studies"], &q.params(), q.limit, cancel)?;
        let mut rows: Vec<StudyRow> = rows.iter().map(StudyRow::from_json).collect();
        rows.retain(|r| !r.study_uid.is_empty());
        Ok(Page {
            rows,
            offset: q.offset,
            more,
        })
    }

    /// `GET /studies/{study}/series`.
    pub fn search_series(
        &self,
        study: &str,
        limit: u32,
        offset: u32,
        cancel: &CancelToken,
    ) -> Result<Page<SeriesRow>> {
        let mut q = vec![(
            "includefield",
            "00201209,0008103E,00180015,00080021,00080031".to_string(),
        )];
        paging(&mut q, limit, offset);
        let (rows, more) = self.qido(&["studies", uid(study)?, "series"], &q, limit, cancel)?;
        let mut rows: Vec<SeriesRow> = rows.iter().map(SeriesRow::from_json).collect();
        for r in &mut rows {
            if r.study_uid.is_empty() {
                r.study_uid = study.to_string();
            }
        }
        rows.retain(|r| !r.series_uid.is_empty());
        rows.sort_by_key(|r| r.number.unwrap_or(i64::MAX));
        Ok(Page { rows, offset, more })
    }

    /// `GET /studies/{study}/series/{series}/instances`.
    pub fn search_instances(
        &self,
        study: &str,
        series: &str,
        limit: u32,
        offset: u32,
        cancel: &CancelToken,
    ) -> Result<Page<InstanceRow>> {
        let mut q = vec![("includefield", "00280008,00280010,00280011".to_string())];
        paging(&mut q, limit, offset);
        let (rows, more) = self.qido(
            &["studies", uid(study)?, "series", uid(series)?, "instances"],
            &q,
            limit,
            cancel,
        )?;
        let mut rows: Vec<InstanceRow> = rows.iter().map(InstanceRow::from_json).collect();
        rows.retain(|r| !r.sop_uid.is_empty());
        Ok(Page { rows, offset, more })
    }
}
