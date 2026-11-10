//! The DICOM JSON model (PS3.18 Annex F): QIDO-RS results and WADO-RS metadata.
//!
//! Mapped into the same [`Instance`] the Part 10 header scan produces, so series assembly,
//! ordering and gap detection are shared with local files.

use std::collections::BTreeMap;

use serde_json::Value;
use specular_dicom::Instance;

/// One attribute value.
#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    Text(String),
    Number(f64),
    /// Person Name: alphabetic, ideographic, phonetic groups.
    Person {
        alphabetic: String,
        ideographic: String,
        phonetic: String,
    },
    Item(JsonDataSet),
    Null,
}

/// One attribute.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JsonElement {
    pub vr: String,
    pub values: Vec<JsonValue>,
    /// Where bulk data (pixel data, big binary values) can be retrieved.
    pub bulk_data_uri: Option<String>,
    /// Base64 binary value, still encoded.
    pub inline_binary: Option<String>,
}

/// A data set: attributes by tag.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JsonDataSet(pub BTreeMap<u32, JsonElement>);

/// The JSON is not a DICOM JSON data set (or array of them).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError(pub String);

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DICOM JSON: {}", self.0)
    }
}

impl std::error::Error for JsonError {}

/// Parses a response body: an array of data sets (QIDO, metadata). An empty body is an empty
/// array (`204 No Content`).
pub fn parse_array(body: &[u8]) -> Result<Vec<JsonDataSet>, JsonError> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(Vec::new());
    }
    let v: Value = serde_json::from_slice(body).map_err(|e| JsonError(e.to_string()))?;
    match v {
        Value::Array(items) => items.iter().map(JsonDataSet::from_value).collect(),
        Value::Object(_) => Ok(vec![JsonDataSet::from_value(&v)?]),
        _ => Err(JsonError("expected an array".into())),
    }
}

fn parse_tag(k: &str) -> Option<u32> {
    (k.len() == 8).then(|| u32::from_str_radix(k, 16).ok())?
}

impl JsonDataSet {
    pub fn from_value(v: &Value) -> Result<JsonDataSet, JsonError> {
        Self::from_value_depth(v, 0)
    }

    fn from_value_depth(v: &Value, depth: u32) -> Result<JsonDataSet, JsonError> {
        if depth > 32 {
            return Err(JsonError("sequences nested too deeply".into()));
        }
        let obj = v
            .as_object()
            .ok_or_else(|| JsonError("data set is not an object".into()))?;
        let mut out = BTreeMap::new();
        for (k, e) in obj {
            let Some(tag) = parse_tag(k) else { continue };
            let Some(e) = e.as_object() else { continue };
            let vr = e
                .get("vr")
                .and_then(Value::as_str)
                .unwrap_or("UN")
                .to_string();
            let mut values = Vec::new();
            if let Some(Value::Array(vs)) = e.get("Value") {
                for x in vs {
                    values.push(match x {
                        Value::String(s) => JsonValue::Text(s.clone()),
                        Value::Number(n) => JsonValue::Number(n.as_f64().unwrap_or(f64::NAN)),
                        Value::Object(_) if vr == "SQ" => {
                            JsonValue::Item(Self::from_value_depth(x, depth + 1)?)
                        }
                        Value::Object(o) => {
                            let g = |n: &str| {
                                o.get(n).and_then(Value::as_str).unwrap_or("").to_string()
                            };
                            JsonValue::Person {
                                alphabetic: g("Alphabetic"),
                                ideographic: g("Ideographic"),
                                phonetic: g("Phonetic"),
                            }
                        }
                        Value::Null => JsonValue::Null,
                        Value::Bool(b) => JsonValue::Number(f64::from(u8::from(*b))),
                        Value::Array(_) => JsonValue::Null,
                    });
                }
            }
            out.insert(
                tag,
                JsonElement {
                    vr,
                    values,
                    bulk_data_uri: e
                        .get("BulkDataURI")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    inline_binary: e
                        .get("InlineBinary")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                },
            );
        }
        Ok(JsonDataSet(out))
    }

    pub fn get(&self, tag: u32) -> Option<&JsonElement> {
        self.0.get(&tag)
    }

    pub fn has(&self, tag: u32) -> bool {
        self.0.contains_key(&tag)
    }

    /// The `i`-th value as text (numbers formatted, person names alphabetic).
    pub fn text_n(&self, tag: u32, i: usize) -> Option<String> {
        Some(match self.get(tag)?.values.get(i)? {
            JsonValue::Text(s) => s.trim().to_string(),
            JsonValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            JsonValue::Person { alphabetic, .. } => alphabetic.trim().to_string(),
            JsonValue::Item(_) | JsonValue::Null => return None,
        })
    }

    /// All values as text joined with `\` (DICOM multi-value form), or `""`.
    pub fn text(&self, tag: u32) -> String {
        let Some(e) = self.get(tag) else {
            return String::new();
        };
        (0..e.values.len())
            .filter_map(|i| self.text_n(tag, i))
            .collect::<Vec<_>>()
            .join("\\")
    }

    /// All values as separate strings.
    pub fn texts(&self, tag: u32) -> Vec<String> {
        let n = self.get(tag).map_or(0, |e| e.values.len());
        (0..n).filter_map(|i| self.text_n(tag, i)).collect()
    }

    /// The `i`-th value as a number (numbers or numeric strings, as servers vary).
    pub fn number(&self, tag: u32, i: usize) -> Option<f64> {
        match self.get(tag)?.values.get(i)? {
            JsonValue::Number(n) => Some(*n).filter(|n| n.is_finite()),
            JsonValue::Text(s) => s.trim().parse().ok().filter(|n: &f64| n.is_finite()),
            _ => None,
        }
    }

    pub fn numbers<const N: usize>(&self, tag: u32) -> Option<[f64; N]> {
        let mut out = [0.0; N];
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.number(tag, i)?;
        }
        Some(out)
    }

    pub fn int(&self, tag: u32) -> Option<i64> {
        self.number(tag, 0).map(|n| n as i64)
    }

    /// Items of a sequence.
    pub fn items(&self, tag: u32) -> impl Iterator<Item = &JsonDataSet> {
        self.get(tag)
            .into_iter()
            .flat_map(|e| e.values.iter())
            .filter_map(|v| match v {
                JsonValue::Item(d) => Some(d),
                _ => None,
            })
    }

    pub fn item(&self, tag: u32) -> Option<&JsonDataSet> {
        self.items(tag).next()
    }

    /// The header fields series assembly needs, as for a Part 10 file.
    pub fn to_instance(&self) -> Instance {
        const PIXEL: [u32; 3] = [0x7FE0_0010, 0x7FE0_0008, 0x7FE0_0009];
        let has_pixels = PIXEL.iter().any(|t| self.has(*t)) || self.has(tags::ROWS);
        let frames = if has_pixels {
            self.number(tags::NUMBER_OF_FRAMES, 0)
                .filter(|n| *n >= 1.0 && *n <= f64::from(u32::MAX))
                .map_or(1, |n| n as u32)
        } else {
            0
        };
        let pos = |d: &JsonDataSet| d.numbers::<3>(tags::POSITION);
        let frame_positions: Vec<[f64; 3]> = self
            .items(tags::PER_FRAME)
            .filter_map(|it| it.item(tags::PLANE_POSITION).and_then(pos))
            .collect();
        let position = pos(self).or_else(|| frame_positions.first().copied());
        let orientation = self.numbers::<6>(tags::ORIENTATION).or_else(|| {
            let shared = self
                .item(tags::SHARED)
                .and_then(|s| s.item(tags::PLANE_ORIENTATION));
            let first = self
                .item(tags::PER_FRAME)
                .and_then(|i| i.item(tags::PLANE_ORIENTATION));
            shared
                .or(first)
                .and_then(|d| d.numbers::<6>(tags::ORIENTATION))
        });
        // Pixel Measures: the instance's own, else the shared or first frame's group.
        let measures = |tag: u32, i: usize| {
            self.number(tag, i).or_else(|| {
                let shared = self
                    .item(tags::SHARED)
                    .and_then(|s| s.item(tags::PIXEL_MEASURES));
                let first = self
                    .item(tags::PER_FRAME)
                    .and_then(|f| f.item(tags::PIXEL_MEASURES));
                shared
                    .and_then(|d| d.number(tag, i))
                    .or_else(|| first.and_then(|d| d.number(tag, i)))
            })
        };
        let positive = |x: Option<f64>| x.filter(|x| x.is_finite() && *x > 0.0);
        let pixel_spacing = match (
            positive(measures(tags::PIXEL_SPACING, 0)),
            positive(measures(tags::PIXEL_SPACING, 1)),
        ) {
            (Some(r), Some(c)) => Some([r, c]),
            _ => None,
        };
        let dim = |tag| {
            self.number(tag, 0)
                .filter(|n| *n >= 0.0 && *n <= f64::from(u32::MAX))
                .map_or(0, |n| n as u32)
        };
        let acquisition_time = self
            .text_n(tags::ACQUISITION_TIME, 0)
            .and_then(|s| parse_tm(&s))
            .or_else(|| parse_tm(self.text_n(tags::ACQUISITION_DATETIME, 0)?.get(8..)?));
        let sop_class = self.text(tags::SOP_CLASS);
        let presentation = sop_class == GSPS_SOP_CLASS;
        let references = if presentation {
            self.items(tags::REFERENCED_SERIES)
                .flat_map(|s| s.items(tags::REFERENCED_IMAGE))
                .filter_map(|i| i.text_n(tags::REFERENCED_SOP_INSTANCE, 0))
                .collect()
        } else {
            Vec::new()
        };
        let (presentation_created, presentation_label) = if presentation {
            (
                format!(
                    "{}{}",
                    self.text(tags::PRESENTATION_DATE),
                    self.text(tags::PRESENTATION_TIME)
                ),
                self.text(tags::CONTENT_LABEL),
            )
        } else {
            Default::default()
        };
        Instance {
            study_uid: self.text(tags::STUDY_UID),
            series_uid: self.text(tags::SERIES_UID),
            sop_uid: self.text(tags::SOP_UID),
            sop_class,
            modality: self.text(tags::MODALITY),
            series_number: self.int(tags::SERIES_NUMBER),
            series_description: self.text(tags::SERIES_DESCRIPTION),
            instance_number: self.int(tags::INSTANCE_NUMBER),
            acquisition_time,
            position,
            orientation,
            frame_positions: if frames > 1 {
                frame_positions
            } else {
                Vec::new()
            },
            slice_location: self.number(tags::SLICE_LOCATION, 0),
            frames,
            references,
            presentation_created,
            presentation_label,
            frame_of_reference: self.text(tags::FRAME_OF_REFERENCE),
            temporal_position: self.int(tags::TEMPORAL_POSITION),
            trigger_time: self
                .number(tags::TRIGGER_TIME, 0)
                .or_else(|| self.number(tags::FRAME_REFERENCE_TIME, 0)),
            rows: dim(tags::ROWS),
            columns: dim(tags::COLUMNS),
            pixel_spacing,
            slice_thickness: positive(measures(tags::SLICE_THICKNESS, 0)),
            // Anatomical Orientation Type, Body Part Examined.
            anatomy: specular_dicom::Anatomy::of(
                self.text_n(0x0010_2210, 0).as_deref(),
                self.text_n(0x0018_0015, 0).as_deref(),
            ),
        }
    }
}

/// Grayscale Softcopy Presentation State Storage.
pub const GSPS_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.11.1";

/// Tags used here and by QIDO.
pub mod tags {
    pub const SOP_CLASS: u32 = 0x0008_0016;
    pub const SOP_UID: u32 = 0x0008_0018;
    pub const STUDY_DATE: u32 = 0x0008_0020;
    pub const SERIES_DATE: u32 = 0x0008_0021;
    pub const STUDY_TIME: u32 = 0x0008_0030;
    pub const SERIES_TIME: u32 = 0x0008_0031;
    pub const ACQUISITION_TIME: u32 = 0x0008_0032;
    pub const ACQUISITION_DATETIME: u32 = 0x0008_002A;
    pub const ACCESSION: u32 = 0x0008_0050;
    pub const MODALITY: u32 = 0x0008_0060;
    pub const MODALITIES_IN_STUDY: u32 = 0x0008_0061;
    pub const REFERRING_PHYSICIAN: u32 = 0x0008_0090;
    pub const STUDY_DESCRIPTION: u32 = 0x0008_1030;
    pub const SERIES_DESCRIPTION: u32 = 0x0008_103E;
    pub const REFERENCED_SERIES: u32 = 0x0008_1115;
    pub const REFERENCED_IMAGE: u32 = 0x0008_1140;
    pub const REFERENCED_SOP_INSTANCE: u32 = 0x0008_1155;
    pub const RETRIEVE_URL: u32 = 0x0008_1190;
    pub const PATIENT_NAME: u32 = 0x0010_0010;
    pub const PATIENT_ID: u32 = 0x0010_0020;
    pub const PATIENT_BIRTH_DATE: u32 = 0x0010_0030;
    pub const PATIENT_SEX: u32 = 0x0010_0040;
    pub const BODY_PART: u32 = 0x0018_0015;
    pub const STUDY_UID: u32 = 0x0020_000D;
    pub const SERIES_UID: u32 = 0x0020_000E;
    pub const STUDY_ID: u32 = 0x0020_0010;
    pub const SERIES_NUMBER: u32 = 0x0020_0011;
    pub const INSTANCE_NUMBER: u32 = 0x0020_0013;
    pub const POSITION: u32 = 0x0020_0032;
    pub const ORIENTATION: u32 = 0x0020_0037;
    pub const SLICE_LOCATION: u32 = 0x0020_1041;
    pub const FRAME_OF_REFERENCE: u32 = 0x0020_0052;
    pub const TEMPORAL_POSITION: u32 = 0x0020_0100;
    pub const TRIGGER_TIME: u32 = 0x0018_1060;
    pub const FRAME_REFERENCE_TIME: u32 = 0x0054_1300;
    pub const STUDY_SERIES_COUNT: u32 = 0x0020_1206;
    pub const STUDY_INSTANCE_COUNT: u32 = 0x0020_1208;
    pub const SERIES_INSTANCE_COUNT: u32 = 0x0020_1209;
    pub const NUMBER_OF_FRAMES: u32 = 0x0028_0008;
    pub const ROWS: u32 = 0x0028_0010;
    pub const COLUMNS: u32 = 0x0028_0011;
    pub const PIXEL_SPACING: u32 = 0x0028_0030;
    pub const SLICE_THICKNESS: u32 = 0x0018_0050;
    pub const PIXEL_MEASURES: u32 = 0x0028_9110;
    pub const CONTENT_LABEL: u32 = 0x0070_0080;
    pub const PRESENTATION_DATE: u32 = 0x0070_0082;
    pub const PRESENTATION_TIME: u32 = 0x0070_0083;
    pub const SHARED: u32 = 0x5200_9229;
    pub const PER_FRAME: u32 = 0x5200_9230;
    pub const PLANE_POSITION: u32 = 0x0020_9113;
    pub const PLANE_ORIENTATION: u32 = 0x0020_9116;
    pub const TRANSFER_SYNTAX: u32 = 0x0002_0010;
    pub const AVAILABLE_TRANSFER_SYNTAX: u32 = 0x0008_3002;
}

/// A DICOM time (`HHMMSS.FFFFFF`, also `HH:MM:SS`) in seconds since midnight.
pub fn parse_tm(s: &str) -> Option<f64> {
    let s: String = s.trim().chars().filter(|c| *c != ':').collect();
    let (hms, frac) = s.split_once('.').unwrap_or((&s, ""));
    if hms.len() < 2 || !hms.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let field = |r: std::ops::Range<usize>| -> f64 {
        hms.get(r)
            .and_then(|x| x.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    let mut t = field(0..2) * 3600.0 + field(2..4) * 60.0 + field(4..6);
    if !frac.is_empty() && frac.bytes().all(|b| b.is_ascii_digit()) {
        t += format!("0.{frac}").parse::<f64>().unwrap_or(0.0);
    }
    Some(t)
}
