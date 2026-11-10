//! STOW-RS (PS3.18 section 10.5): instances sent as `multipart/related;
//! type="application/dicom"`, and the Store Instances Response read per instance, as DICOM
//! JSON or as the Native DICOM Model XML (PS3.19 annex A), whichever the server sends.
//!
//! 200 means every instance was stored, 202 some (with warnings or failures), 409 none;
//! the response lists the stored ones in Referenced SOP Sequence (0008,1199), each with an
//! optional Warning Reason (0008,1196), and the others in Failed SOP Sequence (0008,1198)
//! with their Failure Reason (0008,1197).

use serde_json::{Map, Value};
use specular_remote::{CancelToken, Request};

use crate::json::JsonDataSet;
use crate::{DicomWeb, Error, Result};

const REFERENCED_SOP_SEQUENCE: u32 = 0x0008_1199;
const FAILED_SOP_SEQUENCE: u32 = 0x0008_1198;
const REFERENCED_SOP_INSTANCE: u32 = 0x0008_1155;
const FAILURE_REASON: u32 = 0x0008_1197;
const WARNING_REASON: u32 = 0x0008_1196;

/// What happened to one instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Stored,
    /// Stored with a warning (coerced, elements discarded); the reason's code.
    Warning(Option<u16>),
    /// Not stored; the Failure Reason (PS3.18 table 10.5.3-1 and PS3.4 annex B), when given.
    Failed(Option<u16>),
}

/// The server's answer to one STOW-RS request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreResponse {
    pub status: u16,
    /// By SOP Instance UID.
    pub instances: Vec<(String, Outcome)>,
}

impl StoreResponse {
    /// The outcome for `uid`: listed by the server, else what the status implies for
    /// instances it did not list (200: stored; anything else: failed).
    pub fn outcome(&self, uid: &str) -> Outcome {
        self.instances.iter().find(|(u, _)| u == uid).map_or(
            if self.status == 200 {
                Outcome::Stored
            } else {
                Outcome::Failed(None)
            },
            |(_, o)| o.clone(),
        )
    }
}

/// A boundary that occurs in none of `parts`.
fn boundary(parts: &[&[u8]]) -> String {
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x5eed, |d| d.as_nanos() as u64);
    loop {
        let b = format!("specular-stow-{seed:016x}");
        let delim = format!("\r\n--{b}");
        if parts
            .iter()
            .all(|p| memchr::memmem::find(p, delim.as_bytes()).is_none())
        {
            return b;
        }
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
    }
}

/// The request body and its Content-Type for Part 10 files `parts`.
pub fn body(parts: &[&[u8]]) -> (Vec<u8>, String) {
    let b = boundary(parts);
    let mut out = Vec::with_capacity(parts.iter().map(|p| p.len() + 96).sum());
    for p in parts {
        out.extend_from_slice(
            format!("--{b}\r\nContent-Type: application/dicom\r\n\r\n").as_bytes(),
        );
        out.extend_from_slice(p);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{b}--\r\n").as_bytes());
    (
        out,
        format!("multipart/related; type=\"application/dicom\"; boundary={b}"),
    )
}

fn reason(d: &JsonDataSet, tag: u32) -> Option<u16> {
    d.int(tag).and_then(|n| u16::try_from(n).ok())
}

/// Reads a Store Instances Response body (`content_type` picks JSON or XML; an empty body
/// lists nothing).
pub fn parse_response(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<StoreResponse> {
    let mut out = StoreResponse {
        status,
        instances: Vec::new(),
    };
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(out);
    }
    let xml = content_type.is_some_and(|c| c.to_ascii_lowercase().contains("xml"))
        || body.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'<');
    let v: Value = if xml {
        native_model(body)?
    } else {
        serde_json::from_slice(body).map_err(|e| Error::Json(crate::JsonError(e.to_string())))?
    };
    // Some servers wrap the data set in an array.
    let v = match v {
        Value::Array(mut a) if !a.is_empty() => a.swap_remove(0),
        v => v,
    };
    let d = JsonDataSet::from_value(&v)?;
    for i in d.items(REFERENCED_SOP_SEQUENCE) {
        let uid = i.text(REFERENCED_SOP_INSTANCE);
        let o = if i.has(WARNING_REASON) {
            Outcome::Warning(reason(i, WARNING_REASON))
        } else {
            Outcome::Stored
        };
        out.instances.push((uid, o));
    }
    for i in d.items(FAILED_SOP_SEQUENCE) {
        out.instances.push((
            i.text(REFERENCED_SOP_INSTANCE),
            Outcome::Failed(reason(i, FAILURE_REASON)),
        ));
    }
    Ok(out)
}

/// The Native DICOM Model (PS3.19 A.1) turned into the DICOM JSON model, for the elements
/// a response uses (values, sequences of items).
fn native_model(body: &[u8]) -> Result<Value> {
    let text = std::str::from_utf8(body)
        .map_err(|_| Error::Json(crate::JsonError("XML is not UTF-8".into())))?;
    let doc = roxmltree::Document::parse(text)
        .map_err(|e| Error::Json(crate::JsonError(format!("XML: {e}"))))?;
    let root = doc.root_element();
    Ok(Value::Object(attributes(root)))
}

fn attributes(node: roxmltree::Node) -> Map<String, Value> {
    let mut m = Map::new();
    for a in node.children().filter(|c| c.has_tag_name("DicomAttribute")) {
        let Some(tag) = a.attribute("tag") else {
            continue;
        };
        let vr = a.attribute("vr").unwrap_or("UN");
        let mut values: Vec<(u32, Value)> = Vec::new();
        for c in a.children().filter(|c| c.is_element()) {
            let n = c
                .attribute("number")
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            if c.has_tag_name("Item") {
                values.push((n, Value::Object(attributes(c))));
            } else if c.has_tag_name("Value") {
                let t = c.text().unwrap_or("").trim().to_owned();
                let v = match vr {
                    "US" | "UL" | "SS" | "SL" | "IS" | "DS" | "FL" | "FD" => t
                        .parse::<f64>()
                        .ok()
                        .and_then(|f| serde_json::Number::from_f64(f).map(Value::Number))
                        .unwrap_or(Value::String(t)),
                    _ => Value::String(t),
                };
                values.push((n, v));
            }
        }
        values.sort_by_key(|(n, _)| *n);
        let mut e = Map::new();
        e.insert("vr".into(), Value::String(vr.into()));
        if !values.is_empty() {
            e.insert(
                "Value".into(),
                Value::Array(values.into_iter().map(|(_, v)| v).collect()),
            );
        }
        m.insert(tag.to_ascii_uppercase(), Value::Object(e));
    }
    m
}

impl DicomWeb {
    /// Sends `parts` (Part 10 files) in one STOW-RS request, into `study` when given
    /// (`POST {base}/studies/{study}`; the server refuses instances of other studies), else
    /// `POST {base}/studies`. A 4xx with a Store Instances Response (409) is an answer, not
    /// an error; other failures (401, 413, 5xx, no connection) are errors.
    pub fn store(
        &self,
        parts: &[&[u8]],
        study: Option<&str>,
        cancel: &CancelToken,
    ) -> Result<StoreResponse> {
        let url = match study {
            Some(s) => self.url(&["studies", crate::uid(s)?], &[]),
            None => self.url(&["studies"], &[]),
        };
        let (b, ct) = body(parts);
        let req = Request::post(url, &ct, b)
            .auth(&self.auth)
            .accept("application/dicom+json, application/dicom+xml;q=0.9")
            .any_status();
        let res = self.client.post(&req, cancel)?;
        let listed = res
            .headers
            .content_type()
            .is_some_and(|c| c.contains("json") || c.contains("xml"));
        if res.status >= 400 && !(res.status == 409 && listed) {
            return Err(Error::Http(specular_remote::Error::Status(res.status)));
        }
        parse_response(res.status, res.headers.content_type(), &res.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_round_trips_through_the_multipart_reader() {
        let a = b"first\r\n--not-the-boundary".to_vec();
        let b = vec![0u8, 1, 2, 3];
        let (body, ct) = body(&[&a, &b]);
        let mut m = crate::multipart::Multipart::new(&ct).unwrap();
        let mut parts = Vec::new();
        m.feed(&body, |p| parts.push(p)).unwrap();
        m.finish().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].body, a);
        assert_eq!(parts[1].body, b);
        assert_eq!(parts[0].content_type(), "application/dicom");
    }

    #[test]
    fn json_and_xml_responses() {
        let json = br#"{
          "00081199": {"vr": "SQ", "Value": [
            {"00081155": {"vr": "UI", "Value": ["1.2.3.1"]}},
            {"00081155": {"vr": "UI", "Value": ["1.2.3.2"]}, "00081196": {"vr": "US", "Value": [45056]}}
          ]},
          "00081198": {"vr": "SQ", "Value": [
            {"00081155": {"vr": "UI", "Value": ["1.2.3.3"]}, "00081197": {"vr": "US", "Value": [272]}}
          ]}
        }"#;
        let r = parse_response(202, Some("application/dicom+json"), json).unwrap();
        assert_eq!(r.outcome("1.2.3.1"), Outcome::Stored);
        assert_eq!(r.outcome("1.2.3.2"), Outcome::Warning(Some(0xB000)));
        assert_eq!(r.outcome("1.2.3.3"), Outcome::Failed(Some(0x0110)));
        assert_eq!(r.outcome("1.2.3.9"), Outcome::Failed(None));
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<NativeDicomModel>
  <DicomAttribute tag="00081198" vr="SQ" keyword="FailedSOPSequence">
    <Item number="1">
      <DicomAttribute tag="00081155" vr="UI"><Value number="1">1.2.3.3</Value></DicomAttribute>
      <DicomAttribute tag="00081197" vr="US"><Value number="1">49152</Value></DicomAttribute>
    </Item>
  </DicomAttribute>
</NativeDicomModel>"#;
        let r = parse_response(409, Some("application/dicom+xml"), xml).unwrap();
        assert_eq!(
            r.instances,
            vec![("1.2.3.3".into(), Outcome::Failed(Some(0xC000)))]
        );
        // Empty body with 200: all stored.
        assert_eq!(
            parse_response(200, None, b"").unwrap().outcome("x"),
            Outcome::Stored
        );
    }
}
