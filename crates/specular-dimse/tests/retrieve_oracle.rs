//! C-GET and C-MOVE SCU against pynetdicom (tests/oracle/getscp.py) when
//! `SPECULAR_PYNETDICOM` names a Python with pynetdicom, pydicom and numpy; skipped with
//! the reason otherwise. Loopback only. Checks instance counts, that every pixel received
//! equals pydicom's decode of the file served, the identifiers pynetdicom read, C-MOVE to
//! our temporary SCP, and C-CANCEL.

mod common;

use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use common::{decoded, wrap};
use parking_lot::Mutex;
use specular_core::CancelToken;
use specular_dimse::listen::{Listen, move_into};
use specular_dimse::retrieve::{self, Incoming, Outcome, Storage, Target};
use specular_dimse::{Options, Peer};

const STUDY: &str = "1.2.826.0.1.3680043.10.77.1";
const CT: &str = "1.2.826.0.1.3680043.10.77.1.1";
const MR: &str = "1.2.826.0.1.3680043.10.77.1.2";

struct Py {
    child: std::process::Child,
    port: u16,
}

impl Drop for Py {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

fn out_dir(name: &str) -> PathBuf {
    let d = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/test-tmp"
    ))
    .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn getscp(out: &Path, move_port: u16, delay: f32) -> Option<Py> {
    let Ok(py) = std::env::var("SPECULAR_PYNETDICOM") else {
        eprintln!(
            "skipped: SPECULAR_PYNETDICOM (a Python with pynetdicom, pydicom, numpy) is not set"
        );
        return None;
    };
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/oracle/getscp.py");
    let mut child = Command::new(py)
        .arg("-I")
        .arg(script)
        .arg(out)
        .arg("SPECULAR")
        .arg(move_port.to_string())
        .arg(delay.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("python");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let port = line.trim().parse().expect("port line");
    Some(Py { child, port })
}

type Got = Arc<Mutex<Vec<(String, String, String, Vec<u8>)>>>;

#[derive(Default)]
struct Mem {
    got: Got,
}

struct MemIn {
    head: (String, String, String),
    data: Vec<u8>,
    got: Got,
}

impl Storage for Mem {
    fn begin(&mut self, c: &str, i: &str, ts: &str) -> io::Result<Box<dyn Incoming>> {
        Ok(Box::new(MemIn {
            head: (c.into(), i.into(), ts.into()),
            data: Vec::new(),
            got: self.got.clone(),
        }))
    }
}

impl Incoming for MemIn {
    fn write(&mut self, d: &[u8]) -> io::Result<()> {
        self.data.extend_from_slice(d);
        Ok(())
    }
    fn finish(self: Box<Self>) -> io::Result<()> {
        let (c, i, t) = self.head;
        self.got.lock().push((c, i, t, self.data));
        Ok(())
    }
}

fn opts() -> Options {
    Options {
        connect_timeout: Duration::from_secs(5),
        timeout: Duration::from_secs(20),
        max_pdu: 16384,
        tls: None,
    }
}

fn peer(port: u16) -> Peer {
    Peer {
        host: "127.0.0.1".into(),
        port,
        called_ae: "ORACLE".into(),
        calling_ae: "SPECULAR".into(),
    }
}

/// Pixel mismatches of what arrived against pydicom's decode of the served files.
fn mismatches(out: &Path, got: &Got) -> (usize, usize) {
    let mut bad = 0;
    let mut n = 0;
    for (c, i, ts, data) in got.lock().iter() {
        let want = std::fs::read(out.join("served").join(format!("{i}.px"))).unwrap();
        let want: Vec<u16> = want
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        let have = decoded(&wrap(data, c, i, ts));
        assert_eq!(have.len(), want.len(), "{i}");
        bad += have.iter().zip(&want).filter(|(a, b)| a != b).count();
        n += 1;
    }
    (n, bad)
}

fn req(out: &Path, n: usize) -> serde_json::Value {
    for _ in 0..200 {
        if let Ok(t) = std::fs::read_to_string(out.join(format!("req-{n}.json")))
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&t)
            && v.get("cancelled").is_some()
        {
            return v;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the oracle wrote no req-{n}.json");
}

fn get(port: u16, t: &Target) -> (Outcome, Got) {
    let mut m = Mem::default();
    let got = m.got.clone();
    let o = retrieve::get(
        &peer(port),
        &opts(),
        t,
        &mut m,
        &mut |_| {},
        Box::new(|| false),
        &CancelToken::new(),
    )
    .unwrap();
    (o, got)
}

#[test]
fn c_get_and_c_move_against_pynetdicom() {
    let out = out_dir("dimse-retrieve-oracle");
    let move_port = free_port();
    let Some(py) = getscp(&out, move_port, 0.0) else {
        return;
    };
    // C-GET, series level: the six CT slices, the RLE one as stored.
    let ct = Target {
        study: STUDY.into(),
        series: Some(CT.into()),
    };
    let (o, got) = get(py.port, &ct);
    assert_eq!(o.status, 0, "{o:?}");
    assert_eq!((o.progress.received, o.progress.counts.completed), (6, 6));
    assert!(got.lock().iter().any(|g| g.2 == "1.2.840.10008.1.2.5"));
    let (n, bad) = mismatches(&out, &got);
    eprintln!("C-GET series: {n} instances, {bad} pixel mismatches");
    assert_eq!((n, bad), (6, 0));
    let r = req(&out, 0);
    assert_eq!(r["QueryRetrieveLevel"], "SERIES");
    assert_eq!(r["SeriesInstanceUID"], CT);
    assert_eq!(r["calling"], "SPECULAR");

    // C-GET, study level: nine.
    let study = Target {
        study: STUDY.into(),
        series: None,
    };
    let (o, got) = get(py.port, &study);
    let (n, bad) = mismatches(&out, &got);
    eprintln!("C-GET study: {n} instances, {bad} pixel mismatches");
    assert_eq!((o.progress.received, n, bad), (9, 9, 0));

    // Nothing matches.
    let (o, _) = get(
        py.port,
        &Target {
            study: "1.2.3.4.5".into(),
            series: None,
        },
    );
    assert!(o.no_matches(), "{o:?}");

    // C-MOVE of the MR series to our temporary SCP.
    let listen = Listen {
        address: "127.0.0.1".parse().unwrap(),
        port: move_port,
        ae_title: "SPECULAR".into(),
    };
    let mut m = Mem::default();
    let got = m.got.clone();
    let mr = Target {
        study: STUDY.into(),
        series: Some(MR.into()),
    };
    let o = move_into(
        &peer(py.port),
        &opts(),
        &mr,
        &listen,
        &mut m,
        &mut |_| {},
        Box::new(|| false),
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(o.status, 0, "{o:?}");
    let (n, bad) = mismatches(&out, &got);
    eprintln!("C-MOVE series: {n} instances, {bad} pixel mismatches");
    assert_eq!((o.progress.received, n, bad), (3, 3, 0));
    let r = req(&out, 3);
    assert_eq!(r["kind"], "move");
    assert_eq!(r["destination"], "SPECULAR");

    // A destination pynetdicom does not know.
    let o = move_into(
        &peer(py.port),
        &opts(),
        &mr,
        &Listen {
            ae_title: "STRANGER".into(),
            ..listen.clone()
        },
        &mut Mem::default(),
        &mut |_| {},
        Box::new(|| false),
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(o.status, 0xA801, "{o:?}");
}

#[test]
fn c_cancel_stops_pynetdicom() {
    let out = out_dir("dimse-retrieve-oracle-cancel");
    let Some(py) = getscp(&out, free_port(), 0.3) else {
        return;
    };
    let mut m = Mem::default();
    let got = m.got.clone();
    let g2 = got.clone();
    let o = retrieve::get(
        &peer(py.port),
        &opts(),
        &Target {
            study: STUDY.into(),
            series: Some(CT.into()),
        },
        &mut m,
        &mut |_| {},
        Box::new(move || g2.lock().len() >= 2),
        &CancelToken::new(),
    )
    .unwrap();
    assert!(o.cancelled, "{o:?}");
    assert_eq!(o.status, 0xFE00, "{o:?}");
    let n = got.lock().len();
    eprintln!("C-CANCEL after {n} of 6");
    assert!((2..6).contains(&n), "{n}");
    assert_eq!(req(&out, 0)["cancelled"], true);
}
