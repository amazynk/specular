use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::*;
use crate::lockfile;
use crate::testing::Hub;

/// A fresh folder under the target folder.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("samp-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    // A relative CARGO_TARGET_DIR would break the lock-file URL and the solver path.
    std::path::absolute(d).unwrap()
}

fn meta() -> Metadata {
    Metadata {
        name: "Specular".into(),
        description: "Image viewer".into(),
        version: "0.1".into(),
        doc_url: String::new(),
    }
}

fn wait<T>(rx: &mpsc::Receiver<T>) -> T {
    rx.recv_timeout(Duration::from_secs(10)).expect("timed out")
}

fn until(mut f: impl FnMut() -> bool) {
    let t = Instant::now();
    while !f() {
        assert!(t.elapsed() < Duration::from_secs(10), "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn with_the_test_hub() {
    let d = scratch("hub");
    let lock = d.join("lock");
    let hub = Hub::start(&lock).unwrap();
    // Found through SAMP_HUB, never the real ~/.samp.
    let info = lockfile::find(
        Some(&format!("std-lockurl:{}", lockfile::file_url(&lock))),
        None,
    )
    .unwrap();
    assert_eq!(info.secret, hub.secret());
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let c = Client::connect(&info, &meta(), move |m| {
        let _ = tx.lock().unwrap().send(m);
    })
    .unwrap();
    let clients = hub.clients();
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].1, "Specular");
    assert_eq!(clients[0].2.len(), SUBSCRIPTIONS.len());

    hub.notify(
        "image.load.fits",
        Value::map([
            ("url", Value::str("file://localhost/data/m51.fits")),
            ("name", Value::str("M51")),
        ]),
    );
    assert_eq!(
        wait(&rx),
        Incoming::LoadFits {
            url: "file://localhost/data/m51.fits".into(),
            name: Some("M51".into()),
            image_id: None
        }
    );
    // A call gets a reply.
    hub.call(
        "coord.pointAt.sky",
        Value::map([("ra", Value::str("370.5")), ("dec", Value::str("-3.25"))]),
    );
    assert_eq!(
        wait(&rx),
        Incoming::PointAt {
            ra: 10.5,
            dec: -3.25
        }
    );
    until(|| !hub.responses().is_empty());
    assert_eq!(hub.responses()[0].get_str("samp.status"), Some("samp.ok"));
    // A bad call gets an error reply, and nothing reaches the app.
    hub.call("coord.pointAt.sky", Value::map([("ra", Value::str("x"))]));
    until(|| hub.responses().len() == 2);
    assert_eq!(
        hub.responses()[1].get_str("samp.status"),
        Some("samp.error")
    );
    hub.notify(
        "table.select.rowList",
        Value::map([
            ("table-id", Value::str("t1")),
            (
                "row-list",
                Value::List(vec![Value::str("3"), Value::str("7"), Value::str("x")]),
            ),
        ]),
    );
    assert_eq!(
        wait(&rx),
        Incoming::SelectRows {
            table_id: Some("t1".into()),
            url: None,
            rows: vec![3, 7]
        }
    );
    // Sending.
    let to = c.send_image("file:///data/a.fits", "a.fits").unwrap();
    assert_eq!(to, ["c-probe"]);
    c.point_at(83.8221, -5.3911).unwrap();
    c.highlight_row(Some("t1"), "file:///data/t.vot", 4)
        .unwrap();
    let got = hub.received();
    assert_eq!(got[0].0, "image.load.fits");
    assert_eq!(got[0].1.get_str("url"), Some("file:///data/a.fits"));
    assert_eq!(got[1].1.get_str("ra"), Some("83.82210000"));
    assert_eq!(got[2].1.get_str("row"), Some("4"));
    assert_eq!(got[2].1.get_str("table-id"), Some("t1"));
    assert!(c.ping());
    assert!(c.clients().iter().any(|(_, n)| n == "Probe"));
    // The hub stopping.
    hub.notify("samp.hub.event.shutdown", Value::Map(vec![]));
    assert!(matches!(wait(&rx), Incoming::Disconnected(_)));
    c.disconnect();
    assert!(hub.clients().is_empty());
}

#[test]
fn messages_and_sources() {
    let m = |t: &str, p: Value| Value::map([("samp.mtype", Value::str(t)), ("samp.params", p)]);
    assert_eq!(interpret(&m("samp.app.ping", Value::Map(vec![]))), Ok(None));
    assert!(interpret(&m("image.load.fits", Value::Map(vec![]))).is_err());
    assert!(interpret(&m("spectrum.load.ssa-generic", Value::Map(vec![]))).is_err());
    assert!(
        interpret(&m(
            "coord.pointAt.sky",
            Value::map([("ra", Value::str("1")), ("dec", Value::str("91"))])
        ))
        .is_err()
    );
    assert_eq!(
        source("file:///a/b.fits"),
        Ok(Source::File(PathBuf::from("/a/b.fits")))
    );
    assert_eq!(
        source("http://127.0.0.1:2525/t.vot"),
        Ok(Source::Local("http://127.0.0.1:2525/t.vot".into()))
    );
    assert!(source("https://example.org/a.fits").is_err());
    assert!(source("http://10.1.2.3/a.fits").is_err());
}

/// The oracle: astropy's SAMP hub and an astropy client talk to ours (localhost only, with a
/// lock file in the target folder). Skipped without a Python that has astropy
/// (`SPECULAR_SAMP_PYTHON`, else `$CARGO_TARGET_DIR/venv/bin/python`).
#[test]
fn with_astropy_hub_and_client() {
    let python = std::env::var_os("SPECULAR_SAMP_PYTHON")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|d| PathBuf::from(d).join("venv/bin/python"))
        })
        .filter(|p| p.is_file());
    let Some(python) = python else {
        eprintln!("no Python with astropy: skipped");
        return;
    };
    let d = scratch("astropy");
    let lock = d.join("hub.lock");
    let script = d.join("oracle.py");
    std::fs::write(&script, ASTROPY_ORACLE).unwrap();
    let mut child = Command::new(&python)
        .arg("-I")
        .arg(&script)
        .arg(&lock)
        .env("HOME", &d)
        .env("XDG_CACHE_HOME", d.join("cache"))
        .env("XDG_CONFIG_HOME", d.join("config"))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap()).lines();
    // astropy's log lines are skipped.
    let mut line = || loop {
        let l = out.next().and_then(Result::ok).unwrap_or_default();
        if !(l.starts_with("INFO:") || l.starts_with("WARNING:")) {
            break l;
        }
    };
    let first = line();
    if first.starts_with("NOASTROPY") {
        let _ = child.wait();
        eprintln!("astropy.samp missing: skipped");
        return;
    }
    assert_eq!(first, "READY");
    let info = lockfile::find(
        Some(&format!("std-lockurl:{}", lockfile::file_url(&lock))),
        None,
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let c = Client::connect(&info, &meta(), move |m| {
        let _ = tx.lock().unwrap().send(m);
    })
    .unwrap();
    // The astropy client sends three messages; the call's reply is checked on its side.
    assert_eq!(
        wait(&rx),
        Incoming::LoadFits {
            url: "file:///data/m51.fits".into(),
            name: Some("M51".into()),
            image_id: None
        }
    );
    assert_eq!(
        wait(&rx),
        Incoming::PointAt {
            ra: 202.47,
            dec: 47.195
        }
    );
    assert_eq!(
        wait(&rx),
        Incoming::HighlightRow {
            table_id: Some("tab-1".into()),
            url: Some("file:///data/t.vot".into()),
            row: 12
        }
    );
    assert_eq!(line(), "CALL samp.ok");
    let names: Vec<String> = c.clients().into_iter().map(|(_, n)| n).collect();
    assert!(names.contains(&"Oracle".to_owned()), "{names:?}");
    c.send_image("file:///data/out.fits", "out.fits").unwrap();
    c.point_at(10.0, -20.5).unwrap();
    c.highlight_row(Some("tab-1"), "file:///data/t.vot", 3)
        .unwrap();
    let mut got = Vec::new();
    loop {
        let l = line();
        if l == "DONE" || l.is_empty() {
            break;
        }
        got.push(l);
    }
    c.disconnect();
    let _ = child.wait();
    assert_eq!(
        got,
        [
            "GOT image.load.fits name=out.fits url=file:///data/out.fits",
            "GOT coord.pointAt.sky dec=-20.50000000 ra=10.00000000",
            "GOT table.highlight.row row=3 table-id=tab-1 url=file:///data/t.vot",
        ]
    );
}

const ASTROPY_ORACLE: &str = r#"
import sys, time
try:
    from astropy.samp import SAMPHubServer, SAMPIntegratedClient
    from astropy.samp.lockfile_helpers import read_lockfile
    from astropy import log
    log.setLevel("ERROR")
except Exception:
    print("NOASTROPY", flush=True)
    sys.exit(0)

lock = sys.argv[1]
hub = SAMPHubServer(lockfile=lock, web_profile=False)
hub.start(wait=False)
print("READY", flush=True)
c = SAMPIntegratedClient(name="Oracle")
c.connect(hub_params=read_lockfile(lock))
got = []
def rec(private_key, sender_id, mtype, params, extra):
    got.append(mtype + " " + " ".join(f"{k}={params[k]}" for k in sorted(params)))
for m in ("image.load.fits", "coord.pointAt.sky", "table.highlight.row"):
    c.bind_receive_notification(m, rec)
ours = None
t = time.time()
while ours is None and time.time() - t < 20:
    for cid in c.get_registered_clients():
        if c.get_metadata(cid).get("samp.name") == "Specular":
            ours = cid
    time.sleep(0.05)
c.notify(ours, {"samp.mtype": "image.load.fits",
                "samp.params": {"url": "file:///data/m51.fits", "name": "M51"}})
r = c.ecall_and_wait(ours, "coord.pointAt.sky", "10", ra="202.47", dec="47.195")
c.notify(ours, {"samp.mtype": "table.highlight.row",
                "samp.params": {"table-id": "tab-1", "url": "file:///data/t.vot", "row": "12"}})
print("CALL " + r["samp.status"], flush=True)
t = time.time()
while len(got) < 3 and time.time() - t < 20:
    time.sleep(0.05)
for g in got:
    print("GOT " + g, flush=True)
print("DONE", flush=True)
c.disconnect()
hub.stop()
"#;
