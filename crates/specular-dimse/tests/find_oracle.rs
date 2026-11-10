//! C-FIND SCU against pynetdicom (tests/oracle/findscp.py) when `SPECULAR_PYNETDICOM`
//! names a Python with pynetdicom and pydicom; skipped with the reason otherwise. Checks
//! that pynetdicom reads our identifier as sent, that we read its matches (UTF-8 names,
//! multi-valued and binary-free numbers), and that C-CANCEL stops it.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use specular_core::CancelToken;
use specular_dimse::find::{self, Level, Query};
use specular_dimse::{Options, Peer};

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

fn findscp(out: &Path) -> Option<Py> {
    let Ok(py) = std::env::var("SPECULAR_PYNETDICOM") else {
        eprintln!("skipped: SPECULAR_PYNETDICOM (a Python with pynetdicom and pydicom) is not set");
        return None;
    };
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/oracle/findscp.py");
    let mut child = Command::new(py)
        .arg("-I")
        .arg(script)
        .arg(out)
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

fn read_json(p: &Path) -> serde_json::Value {
    for _ in 0..200 {
        if let Ok(t) = std::fs::read_to_string(p)
            && let Ok(v) = serde_json::from_str(&t)
        {
            return v;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the oracle wrote nothing to {}", p.display());
}

fn study_query(name: &str, date: &str) -> Query {
    Query {
        level: Level::Study,
        keys: vec![
            (0x0010_0010, name.into()),
            (0x0008_0020, date.into()),
            (0x0020_000D, String::new()),
            (0x0010_0020, String::new()),
            (0x0008_0030, String::new()),
            (0x0008_1030, String::new()),
            (0x0008_0061, String::new()),
            (0x0020_1206, String::new()),
            (0x0020_1208, String::new()),
        ],
    }
}

#[test]
fn pynetdicom_find() {
    let out = out_dir("dimse-oracle-find");
    let Some(py) = findscp(&out) else {
        return;
    };
    let peer = Peer {
        host: "127.0.0.1".into(),
        port: py.port,
        called_ae: "ORACLE".into(),
        calling_ae: "SPECULAR".into(),
    };
    let opts = Options {
        timeout: Duration::from_secs(10),
        ..Options::default()
    };
    let c = CancelToken::new();
    let mut mismatches = 0;

    // 0: everything.
    let all = find::find(&peer, &opts, &study_query("", ""), 0, 0, &c).unwrap();
    let uids: Vec<String> = all.matches.iter().map(|m| m.text(0x0020_000D)).collect();
    mismatches += usize::from(uids != ["1.2.1", "1.2.2", "1.2.3", "1.2.4"]);
    let asked = read_json(&out.join("find-0.json"));
    mismatches += usize::from(asked["QueryRetrieveLevel"] != "STUDY");
    mismatches += usize::from(asked["PatientName"] != "");
    let m3 = &all.matches[2];
    mismatches += usize::from(m3.texts(0x0008_0061) != ["CT", "SR"]);
    mismatches += usize::from(m3.int(0x0020_1206) != Some(2));
    mismatches += usize::from(m3.int(0x0020_1208) != Some(40));
    mismatches += usize::from(m3.text(0x0008_1030) != "Study 1.2.3");
    mismatches += usize::from(all.matches[3].text(0x0010_0010) != "Müller^Jörg");

    // 1: a non-ASCII prefix and a date range; pynetdicom must read them as sent.
    let q = find::find(&peer, &opts, &study_query("mül*", "20240201-"), 0, 0, &c).unwrap();
    let asked = read_json(&out.join("find-1.json"));
    mismatches += usize::from(asked["PatientName"] != "mül*");
    mismatches += usize::from(asked["StudyDate"] != "20240201-");
    mismatches += usize::from(asked["SpecificCharacterSet"] != "ISO_IR 192");
    mismatches += usize::from(q.matches.len() != 1 || q.matches[0].text(0x0020_000D) != "1.2.4");

    // 2: a limit: C-CANCEL after the first match.
    let one = find::find(&peer, &opts, &study_query("", ""), 0, 1, &c).unwrap();
    mismatches += usize::from(one.matches.len() != 1 || !one.more);
    mismatches += usize::from(read_json(&out.join("cancel-2.json")) != true);

    // 3: series of a study.
    let s = find::find(
        &peer,
        &opts,
        &Query {
            level: Level::Series,
            keys: vec![
                (0x0020_000D, "1.2.1".into()),
                (0x0020_000E, String::new()),
                (0x0020_0011, String::new()),
                (0x0008_0060, String::new()),
            ],
        },
        0,
        0,
        &c,
    )
    .unwrap();
    let got: Vec<(String, Option<i64>)> = s
        .matches
        .iter()
        .map(|m| (m.text(0x0020_000E), m.int(0x0020_0011)))
        .collect();
    mismatches += usize::from(
        got != [
            ("1.2.1.1".to_string(), Some(2)),
            ("1.2.1.2".to_string(), Some(1)),
        ],
    );
    eprintln!("pynetdicom C-FIND: 4 queries, {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}
