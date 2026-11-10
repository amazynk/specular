//! A fake DICOMweb server for tests, screenshots and measurements: synthetic studies (made-up
//! names, a geometric phantom, never patient data) served from an in-process loopback
//! [`TestServer`], made on the fly and deterministic.
//!
//! It answers QIDO-RS (studies, series, instances; filters, paging, 204 when empty),
//! WADO-RS (instances and whole series as multipart/related with a quoted boundary and a
//! `transfer-syntax` parameter, frames, metadata with bulk data URIs, rendered PNG), and can
//! require credentials, refuse explicit transfer-syntax lists (406), or fail the next requests.
//! STOW-RS (`POST /studies`, `POST /studies/{uid}`) keeps what it receives in
//! [`FakeServer::stored`] and answers with a Store Instances Response (JSON, or XML with
//! [`FakeConfig::stow_xml`]); instances named in [`FakeConfig::stow_fail`] fail.

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use serde_json::{Map, Value, json};
use specular_remote::testing::{ServerOptions, TestRequest, TestResponse, TestServer};

/// Explicit VR little endian, what the fake stores and sends.
pub const TS: &str = "1.2.840.10008.1.2.1";
const CT_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.2";

#[derive(Clone, Debug)]
pub struct FakeSeries {
    pub uid: String,
    pub number: i64,
    pub modality: String,
    pub description: String,
    pub slices: u32,
    pub rows: u16,
    pub columns: u16,
    /// Slice spacing, mm.
    pub spacing: f64,
}

#[derive(Clone, Debug)]
pub struct FakeStudy {
    pub uid: String,
    /// Made-up names (`FAKE^ALPHA`).
    pub patient_name: String,
    pub patient_id: String,
    pub birth_date: String,
    pub sex: String,
    pub date: String,
    pub time: String,
    pub accession: String,
    pub description: String,
    pub series: Vec<FakeSeries>,
}

/// Credentials the fake asks for.
#[derive(Clone, Debug)]
pub enum FakeAuth {
    Basic { user: String, password: String },
    Bearer(String),
}

/// Everything the fake serves.
#[derive(Clone, Debug, Default)]
pub struct FakeConfig {
    pub studies: Vec<FakeStudy>,
    pub auth: Option<FakeAuth>,
    /// Answer 406 to an `Accept` without `transfer-syntax=*`.
    pub only_any_syntax: bool,
    /// Send instance bodies chunked, in pieces of this size.
    pub chunked: Option<usize>,
    /// Never send the `Warning: 299 … more results` header (clients must page by count).
    pub no_more_warning: bool,
    /// STOW-RS: these SOP Instance UIDs fail (Failure Reason 0xA700, out of resources).
    pub stow_fail: Vec<String>,
    /// STOW-RS: answer in the Native DICOM Model XML instead of JSON.
    pub stow_xml: bool,
    /// STOW-RS: answer 413 (Payload Too Large) to request bodies above this many bytes.
    pub stow_max_body: Option<usize>,
}

const NAMES: [&str; 8] = [
    "FAKE^ALPHA",
    "FAKE^BRAVO",
    "FAKE^CHARLIE",
    "FAKE^DELTA",
    "TEST^ECHO",
    "TEST^FOXTROT",
    "PHANTOM^GOLF",
    "PHANTOM^HOTEL",
];
const MODALITIES: [&str; 4] = ["CT", "MR", "CT", "US"];

impl FakeConfig {
    /// `n` studies of two series each (`slices` slices of `size`² pixels in the first).
    pub fn synthetic(n: usize, slices: u32, size: u16) -> FakeConfig {
        let studies = (0..n)
            .map(|i| {
                let uid = format!("2.25.77{:04}", i + 1);
                let modality = MODALITIES[i % MODALITIES.len()].to_string();
                FakeStudy {
                    patient_name: NAMES[i % NAMES.len()].to_string(),
                    patient_id: format!("PID{:05}", 100 + i),
                    birth_date: format!("19{:02}0{}1{}", 40 + (i * 7) % 60, 1 + i % 9, i % 10),
                    sex: ["F", "M", "O"][i % 3].into(),
                    date: format!("2025{:02}{:02}", 1 + i % 12, 1 + (i * 3) % 28),
                    time: format!("{:02}{:02}00", 8 + i % 10, (i * 13) % 60),
                    accession: format!("ACC{:06}", 4200 + i),
                    description: format!("Synthetic {modality} study {}", i + 1),
                    series: vec![
                        FakeSeries {
                            uid: format!("{uid}.1"),
                            number: 1,
                            modality: modality.clone(),
                            description: "Axial phantom".into(),
                            slices,
                            rows: size,
                            columns: size,
                            spacing: 2.5,
                        },
                        FakeSeries {
                            uid: format!("{uid}.2"),
                            number: 2,
                            modality: modality.clone(),
                            description: "Scout".into(),
                            slices: 1,
                            rows: size,
                            columns: size,
                            spacing: 1.0,
                        },
                    ],
                    uid,
                }
            })
            .collect();
        FakeConfig {
            studies,
            ..FakeConfig::default()
        }
    }

    fn study(&self, uid: &str) -> Option<&FakeStudy> {
        self.studies.iter().find(|s| s.uid == uid)
    }
}

impl FakeStudy {
    fn modalities(&self) -> Vec<String> {
        let mut m: Vec<String> = self.series.iter().map(|s| s.modality.clone()).collect();
        m.dedup();
        m
    }
}

/// The running fake. The service root is [`base`](Self::base).
pub struct FakeServer {
    pub server: TestServer,
    /// The next this many instance/frame requests fail with 503.
    pub fail_next: Arc<AtomicU32>,
    /// Instance requests answered so far.
    pub instance_requests: Arc<AtomicU32>,
    /// While set, every request waits here (cancel tests).
    pub stall: Arc<AtomicBool>,
    /// Part 10 files received by STOW-RS, in order.
    pub stored: Arc<parking_lot::Mutex<Vec<Vec<u8>>>>,
    /// STOW-RS requests received (413s included).
    pub posts: Arc<AtomicU32>,
}

impl FakeServer {
    pub fn start(config: FakeConfig, options: ServerOptions) -> std::io::Result<FakeServer> {
        let fail_next = Arc::new(AtomicU32::new(0));
        let instance_requests = Arc::new(AtomicU32::new(0));
        let stall = Arc::new(AtomicBool::new(false));
        let stored = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let posts = Arc::new(AtomicU32::new(0));
        let cfg = Arc::new(config);
        let (f, n, st, kept, pc) = (
            fail_next.clone(),
            instance_requests.clone(),
            stall.clone(),
            stored.clone(),
            posts.clone(),
        );
        let server = TestServer::start(options, move |r| {
            while st.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if r.method == "POST" {
                pc.fetch_add(1, Ordering::Relaxed);
                if cfg.stow_max_body.is_some_and(|m| r.body.len() > m) {
                    return TestResponse::status(413);
                }
                return stow(&cfg, r, &kept);
            }
            handle(&cfg, r, &f, &n)
        })?;
        Ok(FakeServer {
            server,
            fail_next,
            instance_requests,
            stall,
            stored,
            posts,
        })
    }

    /// The service root (`http://127.0.0.1:port/dicom-web`).
    pub fn base(&self) -> String {
        self.server.url("/dicom-web")
    }
}

fn authorised(cfg: &FakeConfig, r: &TestRequest) -> bool {
    let Some(a) = &cfg.auth else { return true };
    let want = match a {
        FakeAuth::Basic { user, password } => {
            let secret = specular_remote::Auth::Basic {
                user: user.clone(),
                password: specular_remote::Secret::new(password.clone()),
            };
            secret
                .header_value()
                .map(|s| s.expose().to_string())
                .unwrap_or_default()
        }
        FakeAuth::Bearer(t) => format!("Bearer {t}"),
    };
    r.header("authorization") == Some(want.as_str())
}

/// STOW-RS: every `application/dicom` part is checked (Part 10, SOP Instance UID, the
/// study of the URL) and kept, unless listed in `stow_fail`.
fn stow(
    cfg: &FakeConfig,
    r: &TestRequest,
    kept: &parking_lot::Mutex<Vec<Vec<u8>>>,
) -> TestResponse {
    if !authorised(cfg, r) {
        return TestResponse::status(401).header("WWW-Authenticate", "Basic realm=\"fake\"");
    }
    let Some(path) = r.path().strip_prefix("/dicom-web/") else {
        return TestResponse::status(404);
    };
    let segs: Vec<&str> = path.split('/').collect();
    let study = match segs.as_slice() {
        ["studies"] => None,
        ["studies", st] => Some(*st),
        _ => return TestResponse::status(405),
    };
    let ct = r.header("content-type").unwrap_or("");
    let (mt, _) = crate::multipart::media_type(ct);
    if mt != "multipart/related" {
        return TestResponse::status(415);
    }
    let Ok(mut m) = crate::multipart::Multipart::new(ct) else {
        return TestResponse::status(400);
    };
    let mut parts = Vec::new();
    if m.feed(&r.body, |p| parts.push(p)).is_err() || m.finish().is_err() {
        return TestResponse::status(400);
    }
    let mut ok = Vec::new();
    let mut failed = Vec::new();
    for p in parts {
        if p.content_type() != "application/dicom" {
            continue;
        }
        let Ok(d) = specular_dicom::Dicom::parse(&p.body) else {
            failed.push((String::new(), String::new(), 0xC000u16));
            continue;
        };
        let v = d.values(&p.body);
        let t = |tag: u32| {
            d.dataset
                .get(tag)
                .map(|e| {
                    String::from_utf8_lossy(v.bytes(e))
                        .trim_end_matches(['\0', ' '])
                        .to_string()
                })
                .unwrap_or_default()
        };
        let (class, inst, st) = (t(0x0008_0016), t(0x0008_0018), t(0x0020_000D));
        if study.is_some_and(|s| s != st) {
            failed.push((class, inst, 0xA900));
        } else if cfg.stow_fail.contains(&inst) {
            failed.push((class, inst, 0xA700));
        } else {
            ok.push((class, inst));
            kept.lock().push(p.body);
        }
    }
    let status = match (ok.is_empty(), failed.is_empty()) {
        (_, true) => 200,
        (false, false) => 202,
        (true, false) => 409,
    };
    let refs = |rows: &[(String, String)]| -> Value {
        Value::Array(
            rows.iter()
                .map(|(c, i)| json!({"00081150": text("UI", c), "00081155": text("UI", i)}))
                .collect(),
        )
    };
    if cfg.stow_xml {
        let mut x =
            String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<NativeDicomModel>\n");
        let item = |x: &mut String, n: usize, c: &str, i: &str, reason: Option<u16>| {
            let _ = write!(
                x,
                "<Item number=\"{n}\"><DicomAttribute tag=\"00081150\" vr=\"UI\"><Value number=\"1\">{c}</Value></DicomAttribute><DicomAttribute tag=\"00081155\" vr=\"UI\"><Value number=\"1\">{i}</Value></DicomAttribute>"
            );
            if let Some(r) = reason {
                let _ = write!(
                    x,
                    "<DicomAttribute tag=\"00081197\" vr=\"US\"><Value number=\"1\">{r}</Value></DicomAttribute>"
                );
            }
            x.push_str("</Item>");
        };
        if !ok.is_empty() {
            x.push_str("<DicomAttribute tag=\"00081199\" vr=\"SQ\">");
            for (n, (c, i)) in ok.iter().enumerate() {
                item(&mut x, n + 1, c, i, None);
            }
            x.push_str("</DicomAttribute>\n");
        }
        if !failed.is_empty() {
            x.push_str("<DicomAttribute tag=\"00081198\" vr=\"SQ\">");
            for (n, (c, i, r)) in failed.iter().enumerate() {
                item(&mut x, n + 1, c, i, Some(*r));
            }
            x.push_str("</DicomAttribute>\n");
        }
        x.push_str("</NativeDicomModel>\n");
        return TestResponse::new(status, x).header("Content-Type", "application/dicom+xml");
    }
    let mut d = Map::new();
    if !ok.is_empty() {
        d.insert("00081199".into(), json!({"vr": "SQ", "Value": refs(&ok)}));
    }
    if !failed.is_empty() {
        let rows: Vec<Value> = failed
            .iter()
            .map(|(c, i, r)| {
                json!({"00081150": text("UI", c), "00081155": text("UI", i),
                    "00081197": {"vr": "US", "Value": [r]}})
            })
            .collect();
        d.insert("00081198".into(), json!({"vr": "SQ", "Value": rows}));
    }
    TestResponse::new(
        status,
        serde_json::to_vec(&Value::Object(d)).unwrap_or_default(),
    )
    .header("Content-Type", "application/dicom+json")
}

fn json_response(rows: Vec<Value>) -> TestResponse {
    TestResponse::ok(serde_json::to_vec(&Value::Array(rows)).unwrap_or_default())
        .header("Content-Type", "application/dicom+json")
}

fn handle(cfg: &FakeConfig, r: &TestRequest, fail: &AtomicU32, n: &AtomicU32) -> TestResponse {
    if !authorised(cfg, r) {
        return TestResponse::status(401).header("WWW-Authenticate", "Basic realm=\"fake\"");
    }
    let Some(path) = r.path().strip_prefix("/dicom-web/") else {
        return TestResponse::status(404);
    };
    let segs: Vec<&str> = path.split('/').collect();
    match segs.as_slice() {
        ["studies"] => qido_studies(cfg, r),
        ["studies", st, "series"] => match cfg.study(st) {
            Some(s) => page(
                r,
                cfg,
                s.series.iter().map(|se| series_json(s, se)).collect(),
            ),
            None => TestResponse::status(404),
        },
        ["studies", st, "series", se, rest @ ..] => {
            let Some((study, series)) = find(cfg, st, se) else {
                return TestResponse::status(404);
            };
            match rest {
                ["instances"] => page(
                    r,
                    cfg,
                    (0..series.slices)
                        .map(|i| dataset(study, series, i, false))
                        .collect(),
                ),
                ["metadata"] => json_response(
                    (0..series.slices)
                        .map(|i| {
                            let mut d = dataset(study, series, i, true);
                            let uri = format!(
                                "{}/studies/{}/series/{}/instances/{}/frames/1",
                                r.header("host")
                                    .map(|h| format!("http://{h}/dicom-web"))
                                    .unwrap_or_default(),
                                study.uid,
                                series.uid,
                                sop(series, i)
                            );
                            d["7FE00010"] = json!({ "vr": "OW", "BulkDataURI": uri });
                            d
                        })
                        .collect(),
                ),
                [] => {
                    if let Some(res) = refuse(cfg, r, fail, n) {
                        return res;
                    }
                    let parts: Vec<Vec<u8>> = (0..series.slices)
                        .map(|i| part10(study, series, i))
                        .collect();
                    multipart(cfg, &parts)
                }
                ["instances", inst, more @ ..] => {
                    let Some(i) = (0..series.slices).find(|i| sop(series, *i) == *inst) else {
                        return TestResponse::status(404);
                    };
                    match more {
                        [] => {
                            if let Some(res) = refuse(cfg, r, fail, n) {
                                return res;
                            }
                            multipart(cfg, &[part10(study, series, i)])
                        }
                        ["frames", list] => {
                            if fail_now(fail) {
                                return TestResponse::status(503);
                            }
                            let px = pixels(series, i);
                            let count = list.split(',').count();
                            octets(&vec![px; count])
                        }
                        ["rendered"] | ["frames", _, "rendered"] => rendered(series, i),
                        _ => TestResponse::status(404),
                    }
                }
                _ => TestResponse::status(404),
            }
        }
        _ => TestResponse::status(404),
    }
}

fn fail_now(fail: &AtomicU32) -> bool {
    fail.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_sub(1))
        .is_ok()
}

/// 503 (injected) or 406 (syntax list refused) before an instance response.
fn refuse(
    cfg: &FakeConfig,
    r: &TestRequest,
    fail: &AtomicU32,
    n: &AtomicU32,
) -> Option<TestResponse> {
    if fail_now(fail) {
        return Some(TestResponse::status(503));
    }
    let accept = r.header("accept").unwrap_or("*/*");
    let any = accept.contains("transfer-syntax=*") || accept.contains("*/*");
    if !(accept.contains(TS) || any) || (cfg.only_any_syntax && !any) {
        return Some(TestResponse::status(406));
    }
    n.fetch_add(1, Ordering::Relaxed);
    None
}

fn find<'a>(cfg: &'a FakeConfig, st: &str, se: &str) -> Option<(&'a FakeStudy, &'a FakeSeries)> {
    let s = cfg.study(st)?;
    Some((s, s.series.iter().find(|x| x.uid == se)?))
}

/// Applies `limit`/`offset` and the "more results" warning.
fn page(r: &TestRequest, cfg: &FakeConfig, rows: Vec<Value>) -> TestResponse {
    let offset: usize = r.param("offset").and_then(|v| v.parse().ok()).unwrap_or(0);
    let limit: usize = r
        .param("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(usize::MAX);
    let total = rows.len();
    let rows: Vec<Value> = rows.into_iter().skip(offset).take(limit).collect();
    if rows.is_empty() {
        return TestResponse::status(204);
    }
    let more = offset + rows.len() < total;
    let mut res = json_response(rows);
    if more && !cfg.no_more_warning {
        res = res.header(
            "Warning",
            "299 fake: There are more results than were returned",
        );
    }
    res
}

fn matches(pattern: &str, value: &str) -> bool {
    let (p, v) = (pattern.to_ascii_uppercase(), value.to_ascii_uppercase());
    match p.strip_suffix('*') {
        Some(prefix) => v.starts_with(prefix),
        None => p == v,
    }
}

fn qido_studies(cfg: &FakeConfig, r: &TestRequest) -> TestResponse {
    let q = |k: &str| r.param(k).filter(|v| !v.is_empty());
    let rows = cfg
        .studies
        .iter()
        .filter(|s| {
            q("PatientName").is_none_or(|p| matches(&p, &s.patient_name))
                && q("PatientID").is_none_or(|p| p == s.patient_id)
                && q("AccessionNumber").is_none_or(|p| p == s.accession)
                && q("ModalitiesInStudy").is_none_or(|m| s.modalities().contains(&m))
                && q("StudyDate").is_none_or(|d| {
                    let (a, b) = d.split_once('-').unwrap_or((&d, &d));
                    (a.is_empty() || s.date.as_str() >= a) && (b.is_empty() || s.date.as_str() <= b)
                })
        })
        .map(study_json)
        .collect();
    page(r, cfg, rows)
}

fn text(vr: &str, v: &str) -> Value {
    json!({ "vr": vr, "Value": [v] })
}

fn study_json(s: &FakeStudy) -> Value {
    let instances: u32 = s.series.iter().map(|x| x.slices).sum();
    json!({
        "0020000D": text("UI", &s.uid),
        "00080020": text("DA", &s.date),
        "00080030": text("TM", &s.time),
        "00080050": text("SH", &s.accession),
        "00081030": text("LO", &s.description),
        "00080061": { "vr": "CS", "Value": s.modalities() },
        "00100010": { "vr": "PN", "Value": [{ "Alphabetic": s.patient_name }] },
        "00100020": text("LO", &s.patient_id),
        "00100030": text("DA", &s.birth_date),
        "00100040": text("CS", &s.sex),
        "00201206": { "vr": "IS", "Value": [s.series.len()] },
        "00201208": { "vr": "IS", "Value": [instances] },
    })
}

fn series_json(s: &FakeStudy, se: &FakeSeries) -> Value {
    json!({
        "0020000D": text("UI", &s.uid),
        "0020000E": text("UI", &se.uid),
        "00080060": text("CS", &se.modality),
        "00200011": { "vr": "IS", "Value": [se.number] },
        "0008103E": text("LO", &se.description),
        "00201209": { "vr": "IS", "Value": [se.slices] },
        "00180015": text("CS", "PHANTOM"),
        "00080021": text("DA", &s.date),
        "00080031": text("TM", &s.time),
    })
}

fn sop(se: &FakeSeries, i: u32) -> String {
    format!("{}.{}", se.uid, i + 1)
}

/// One element: (tag, VR, value).
enum Val {
    Text(String),
    Nums(Vec<f64>),
    Us(u16),
    Person(String),
}

fn elements(st: &FakeStudy, se: &FakeSeries, i: u32) -> Vec<(u32, &'static str, Val)> {
    use Val::*;
    let z = -(i as f64) * se.spacing;
    let half = f64::from(se.columns) * 0.35;
    vec![
        (0x0008_0016, "UI", Text(CT_IMAGE.into())),
        (0x0008_0018, "UI", Text(sop(se, i))),
        (0x0008_0020, "DA", Text(st.date.clone())),
        (0x0008_0030, "TM", Text(st.time.clone())),
        (0x0008_0050, "SH", Text(st.accession.clone())),
        (0x0008_0060, "CS", Text(se.modality.clone())),
        (0x0008_1030, "LO", Text(st.description.clone())),
        (0x0008_103E, "LO", Text(se.description.clone())),
        (0x0010_0010, "PN", Person(st.patient_name.clone())),
        (0x0010_0020, "LO", Text(st.patient_id.clone())),
        (0x0018_0050, "DS", Nums(vec![se.spacing])),
        (0x0020_000D, "UI", Text(st.uid.clone())),
        (0x0020_000E, "UI", Text(se.uid.clone())),
        (0x0020_0011, "IS", Nums(vec![se.number as f64])),
        (0x0020_0013, "IS", Nums(vec![f64::from(i + 1)])),
        (0x0020_0032, "DS", Nums(vec![-half, -half, z])),
        (0x0020_0037, "DS", Nums(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0])),
        (0x0020_1041, "DS", Nums(vec![z])),
        (0x0028_0002, "US", Us(1)),
        (0x0028_0004, "CS", Text("MONOCHROME2".into())),
        (0x0028_0010, "US", Us(se.rows)),
        (0x0028_0011, "US", Us(se.columns)),
        (0x0028_0030, "DS", Nums(vec![0.7, 0.7])),
        (0x0028_0100, "US", Us(16)),
        (0x0028_0101, "US", Us(12)),
        (0x0028_0102, "US", Us(11)),
        (0x0028_0103, "US", Us(1)),
        (0x0028_1050, "DS", Nums(vec![40.0])),
        (0x0028_1051, "DS", Nums(vec![400.0])),
        (0x0028_1052, "DS", Nums(vec![-1024.0])),
        (0x0028_1053, "DS", Nums(vec![1.0])),
    ]
}

fn num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// The DICOM JSON of instance `i` (without pixel data). `full`: every attribute (metadata),
/// else the QIDO instance attributes.
fn dataset(st: &FakeStudy, se: &FakeSeries, i: u32, full: bool) -> Value {
    let mut m = Map::new();
    for (tag, vr, v) in elements(st, se, i) {
        if !full
            && !matches!(
                tag,
                0x0008_0016 | 0x0008_0018 | 0x0020_0013 | 0x0028_0010 | 0x0028_0011
            )
        {
            continue;
        }
        let value = match v {
            Val::Text(s) => json!([s]),
            Val::Nums(n) => json!(n),
            Val::Us(u) => json!([u]),
            Val::Person(p) => json!([{ "Alphabetic": p }]),
        };
        m.insert(format!("{tag:08X}"), json!({ "vr": vr, "Value": value }));
    }
    Value::Object(m)
}

fn put(out: &mut Vec<u8>, tag: u32, vr: &str, value: &[u8]) {
    out.extend_from_slice(&((tag >> 16) as u16).to_le_bytes());
    out.extend_from_slice(&(tag as u16).to_le_bytes());
    out.extend_from_slice(vr.as_bytes());
    if matches!(vr, "OB" | "OW" | "SQ" | "UN" | "UT") {
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    } else {
        out.extend_from_slice(&(value.len() as u16).to_le_bytes());
    }
    out.extend_from_slice(value);
}

fn padded(s: &str, vr: &str) -> Vec<u8> {
    let mut b = s.as_bytes().to_vec();
    if b.len() % 2 == 1 {
        b.push(if vr == "UI" { 0 } else { b' ' });
    }
    b
}

/// A CT-like phantom slice, Hounsfield units + 1024 (rescale −1024): an elliptical body,
/// a bone ring, and a sphere whose cross-section changes from slice to slice.
pub fn phantom(rows: u16, columns: u16, i: u32, slices: u32) -> Vec<u16> {
    let (w, h) = (f32::from(columns), f32::from(rows));
    let t = if slices > 1 {
        i as f32 / (slices - 1) as f32
    } else {
        0.5
    };
    let sphere = (0.18 * (1.0 - (2.0 * t - 1.0).powi(2)).max(0.0).sqrt()) * w;
    let (sx, sy) = (w * 0.62, h * 0.45);
    let mut out = Vec::with_capacity(usize::from(rows) * usize::from(columns));
    for y in 0..rows {
        let fy = (f32::from(y) + 0.5 - h / 2.0) / (h * 0.36);
        for x in 0..columns {
            let fx = (f32::from(x) + 0.5 - w / 2.0) / (w * 0.42);
            let r = fx * fx + fy * fy;
            let mut hu = if r > 1.0 {
                -1000.0
            } else if r > 0.82 {
                700.0
            } else {
                40.0 + 20.0 * ((f32::from(x) * 0.11).sin() * (f32::from(y) * 0.07).cos())
            };
            let (dx, dy) = (f32::from(x) - sx, f32::from(y) - sy);
            if r <= 0.82 && dx * dx + dy * dy < sphere * sphere {
                hu = 180.0;
            }
            out.push((hu + 1024.0).clamp(0.0, 4095.0) as u16);
        }
    }
    out
}

fn pixels(se: &FakeSeries, i: u32) -> Vec<u8> {
    phantom(se.rows, se.columns, i, se.slices)
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Instance `i` as a Part 10 file (explicit VR little endian).
pub fn part10(st: &FakeStudy, se: &FakeSeries, i: u32) -> Vec<u8> {
    let mut meta = Vec::new();
    put(&mut meta, 0x0002_0001, "OB", &[0, 1]);
    put(&mut meta, 0x0002_0002, "UI", &padded(CT_IMAGE, "UI"));
    put(&mut meta, 0x0002_0003, "UI", &padded(&sop(se, i), "UI"));
    put(&mut meta, 0x0002_0010, "UI", &padded(TS, "UI"));
    put(&mut meta, 0x0002_0012, "UI", &padded("2.25.1234567", "UI"));
    let mut out = vec![0u8; 128];
    out.extend_from_slice(b"DICM");
    put(
        &mut out,
        0x0002_0000,
        "UL",
        &(meta.len() as u32).to_le_bytes(),
    );
    out.extend_from_slice(&meta);
    for (tag, vr, v) in elements(st, se, i) {
        let bytes = match v {
            Val::Text(s) | Val::Person(s) => padded(&s, vr),
            Val::Nums(n) => padded(
                &n.iter().map(|v| num(*v)).collect::<Vec<_>>().join("\\"),
                vr,
            ),
            Val::Us(u) => u.to_le_bytes().to_vec(),
        };
        put(&mut out, tag, vr, &bytes);
    }
    put(&mut out, 0x7FE0_0010, "OW", &pixels(se, i));
    out
}

const BOUNDARY: &str = "fake:boundary/42";

fn multipart_body(ct: &str, parts: &[Vec<u8>]) -> Vec<u8> {
    let mut body = b"preamble, ignored\r\n".to_vec();
    for p in parts {
        let mut h = String::new();
        let _ = write!(
            h,
            "--{BOUNDARY}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\n\r\n",
            p.len()
        );
        body.extend_from_slice(h.as_bytes());
        body.extend_from_slice(p);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\nepilogue\r\n").as_bytes());
    body
}

fn multipart(cfg: &FakeConfig, parts: &[Vec<u8>]) -> TestResponse {
    let body = multipart_body(&format!("application/dicom; transfer-syntax={TS}"), parts);
    let mut res = TestResponse::ok(body).header(
        "Content-Type",
        format!(
            "multipart/related; type=\"application/dicom\"; transfer-syntax={TS}; boundary=\"{BOUNDARY}\""
        ),
    );
    if let Some(c) = cfg.chunked {
        res = res.chunked(c);
    }
    res
}

fn octets(parts: &[Vec<u8>]) -> TestResponse {
    TestResponse::ok(multipart_body(
        &format!("application/octet-stream; transfer-syntax={TS}"),
        parts,
    ))
    .header(
        "Content-Type",
        format!("multipart/related; type=\"application/octet-stream\"; boundary=\"{BOUNDARY}\""),
    )
}

fn rendered(se: &FakeSeries, i: u32) -> TestResponse {
    let px: Vec<u8> = phantom(se.rows, se.columns, i, se.slices)
        .into_iter()
        .map(|v| {
            let hu = f32::from(v) - 1024.0;
            (((hu - 40.0) / 400.0 + 0.5).clamp(0.0, 1.0) * 255.0) as u8
        })
        .collect();
    match specular_png::encode(
        u32::from(se.columns),
        u32::from(se.rows),
        specular_png::ColorType::Gray,
        &px,
    ) {
        Ok(png) => TestResponse::ok(png).header("Content-Type", "image/png"),
        Err(_) => TestResponse::status(500),
    }
}
