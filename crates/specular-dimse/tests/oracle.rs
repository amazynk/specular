//! The SCU against independent SCPs on loopback: pynetdicom (tests/oracle/storescp.py) when
//! `SPECULAR_PYNETDICOM` names a Python with pynetdicom and pydicom, and dcmtk's `storescp`
//! when it is on the PATH. Each is skipped with the reason when missing.

mod common;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use common::*;
use specular_core::CancelToken;
use specular_dimse::assoc::{self, Association};
use specular_dimse::{Options, Peer, Status, store};

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

fn pynetdicom(out: &Path, syntaxes: &str, max_pdu: u32) -> Option<Py> {
    let Ok(py) = std::env::var("SPECULAR_PYNETDICOM") else {
        eprintln!("skipped: SPECULAR_PYNETDICOM (a Python with pynetdicom and pydicom) is not set");
        return None;
    };
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/oracle/storescp.py");
    let mut child = Command::new(py)
        .arg("-I")
        .arg(script)
        .arg(out)
        .arg(syntaxes)
        .arg(max_pdu.to_string())
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

fn peer(port: u16, called: &str) -> Peer {
    Peer {
        host: "127.0.0.1".into(),
        port,
        called_ae: called.into(),
        calling_ae: "SPECULAR".into(),
    }
}

fn opts() -> Options {
    Options {
        timeout: Duration::from_secs(10),
        ..Options::default()
    }
}

fn send(port: u16, called: &str, files: &[Vec<u8>]) -> Vec<specular_dimse::Sent> {
    let heads: Vec<_> = files.iter().map(|f| store::head(f).unwrap()).collect();
    let ctx = store::contexts(
        heads
            .iter()
            .map(|h| (h.sop_class.as_str(), h.transfer_syntax.as_str())),
    );
    let mut a =
        Association::request(&peer(port, called), &opts(), &ctx, &CancelToken::new()).unwrap();
    let sent: Vec<_> = files
        .iter()
        .map(|f| a.send_file(f, None, &mut |_| {}).unwrap())
        .collect();
    a.release().unwrap();
    sent
}

fn info(out: &Path, uid: &str) -> serde_json::Value {
    // The SCP writes after answering; give it a moment.
    let p = out.join(format!("{uid}.json"));
    for _ in 0..100 {
        if let Ok(t) = std::fs::read_to_string(&p)
            && let Ok(v) = serde_json::from_str(&t)
        {
            return v;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the oracle stored nothing for {uid}");
}

fn files() -> Vec<Vec<u8>> {
    vec![
        explicit(CT, "1.2.826.0.1.3680043.10.9.20.1", 64, 0),
        implicit(CT, "1.2.826.0.1.3680043.10.9.20.2", 64, 1),
        rle(CT, "1.2.826.0.1.3680043.10.9.20.3", 64, 2),
        explicit(MR, "1.2.826.0.1.3680043.10.9.20.4", 40, 3),
    ]
}

#[test]
fn pynetdicom_receives_as_stored() {
    let out = out_dir("dimse-oracle-stored");
    let Some(py) = pynetdicom(&out, "all", 4096) else {
        return;
    };
    assoc::verify(&peer(py.port, "ORACLE"), &opts(), &CancelToken::new()).unwrap();
    let files = files();
    let sent = send(py.port, "ORACLE", &files);
    let mut mismatches = 0;
    for (f, s) in files.iter().zip(&sent) {
        assert_eq!(Status::of(s.status), Status::Success);
        let h = store::head(f).unwrap();
        let v = info(&out, &h.sop_instance);
        assert_eq!(v["ts"], h.transfer_syntax.as_str());
        assert_eq!(v["calling"], "SPECULAR");
        assert_eq!(v["code"], "T-D0050");
        let raw = std::fs::read(out.join(format!("{}.raw", h.sop_instance))).unwrap();
        if raw != f[h.data_set..] {
            mismatches += 1;
        }
    }
    eprintln!(
        "pynetdicom, as stored: {} files, {mismatches} data sets differ",
        files.len()
    );
    assert_eq!(mismatches, 0);
}

#[test]
fn pynetdicom_receives_converted() {
    let out = out_dir("dimse-oracle-implicit");
    let Some(py) = pynetdicom(&out, IMPLICIT, 16384) else {
        return;
    };
    let files = files();
    let sent = send(py.port, "ORACLE", &files);
    let mut mismatches = 0;
    for (f, s) in files.iter().zip(&sent) {
        assert_eq!(s.transfer_syntax, IMPLICIT);
        let h = store::head(f).unwrap();
        let v = info(&out, &h.sop_instance);
        let size = v["rows"].as_u64().unwrap();
        assert_eq!(v["ts"], IMPLICIT);
        assert_eq!(v["pixel_len"].as_u64().unwrap(), size * size * 2);
        assert_eq!(v["patient"], "Test^Patient");
        assert_eq!(v["code"], "T-D0050");
        let raw = std::fs::read(out.join(format!("{}.raw", h.sop_instance))).unwrap();
        let back = wrap(&raw, &h.sop_class, &h.sop_instance, IMPLICIT);
        if decoded(&back) != decoded(f) {
            mismatches += 1;
        }
    }
    eprintln!(
        "pynetdicom, Implicit VR LE only: {} files, {mismatches} pixel mismatches",
        files.len()
    );
    assert_eq!(mismatches, 0);
}

#[test]
fn dcmtk_storescp() {
    let found = std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("storescp").is_file()));
    if !found {
        eprintln!("skipped: dcmtk's storescp is not installed");
        return;
    }
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let out = out_dir("dimse-oracle-dcmtk");
    let mut child = Command::new("storescp")
        .args(["-aet", "DCMTK", "-od"])
        .arg(&out)
        .arg(port.to_string())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assoc::verify(&peer(port, "DCMTK"), &opts(), &CancelToken::new()).unwrap();
    let files = files();
    let sent = send(port, "DCMTK", &files);
    let _ = child.kill();
    let _ = child.wait();
    assert!(sent.iter().all(|s| Status::of(s.status).stored()));
}
