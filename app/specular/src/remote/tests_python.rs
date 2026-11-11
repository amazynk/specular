//! The Python client's tests (`clients/python/tests`), run against this server with a stand-in
//! window, in a virtual environment under the target folder (made on first use with the
//! system's site packages, so NumPy is used when installed; nothing is installed). Skipped
//! when there is no `python3`.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use serde_json::json;
use specular_client::{Nav, Request};
use specular_platform::ipc;

use super::events::{Bus, Event};
use super::server::{Call, Server};

fn venv() -> Option<PathBuf> {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let dir = base.join("py-venv");
    let py = dir.join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    });
    if py.exists() {
        return Some(py);
    }
    let ok = Command::new("python3")
        .args(["-m", "venv", "--system-site-packages"])
        .arg(&dir)
        .status()
        .is_ok_and(|s| s.success());
    ok.then_some(py).filter(|p| p.exists())
}

fn fake_window(
    calls: crossbeam_channel::Receiver<Call>,
    bus: Arc<Bus>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while let Ok(c) = calls.recv() {
            let r = match &c.req {
                // Marks are told as an event, as the window tells them.
                Request::Mark { files, mark } => {
                    let e = Event::new("marked", files.clone()).with(
                        "mark",
                        json!({
                            "flag": mark.flag.map(|f| f.as_str()),
                            "stars": mark.stars,
                            "label": mark.label.and_then(specular_client::protocol::label_name),
                        }),
                    );
                    let mut src = serde_json::Map::new();
                    src.insert("source".into(), json!("ipc"));
                    bus.publish(&e, &src);
                    json!({ "marked": files.len() })
                }
                Request::Navigate(Nav::Index(i)) => json!({ "index": i }),
                Request::Navigate(_) => json!({ "index": 0 }),
                Request::Open { files, .. } => json!({ "opened": files.len(), "missing": [] }),
                Request::Screenshot => {
                    c.answer_pixels(Ok(json!({})), (2, 2, vec![255; 16]));
                    continue;
                }
                Request::Quit => {
                    c.answer(Ok(json!({})));
                    break;
                }
                r => json!({ "cmd": r.cmd(), "debug": format!("{r:?}") }),
            };
            c.answer(Ok(r));
        }
    })
}

#[test]
#[cfg_attr(
    windows,
    ignore = "the Python client speaks over AF_UNIX sockets, which Python on Windows lacks"
)]
fn python_client() {
    let Some(py) = venv() else {
        eprintln!("python_client: no python3 with venv here, skipped");
        return;
    };
    let dir = ipc::scratch_dir("specular-py").unwrap();
    let server = Server::start(&dir, Arc::new(|| {})).unwrap();
    let window = fake_window(server.calls.clone(), server.bus().clone());
    let tests = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../clients/python/tests");
    let out = Command::new(&py)
        .args(["-B", "-m", "unittest", "discover", "-v", "-s"])
        .arg(&tests)
        .env("SPECULAR_SOCKET", server.path())
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stderr);
    let tail: Vec<&str> = text.lines().rev().take(40).collect();
    assert!(
        out.status.success(),
        "python tests failed:\n{}",
        tail.into_iter().rev().collect::<Vec<_>>().join("\n")
    );
    assert!(text.contains("OK"), "{text}");
    // Report whether NumPy took part.
    let skipped = text.contains("needs NumPy");
    eprintln!(
        "python_client: passed (NumPy {})",
        if skipped { "absent" } else { "used" }
    );
    specular_client::Client::connect(server.path())
        .unwrap()
        .call(&Request::Quit)
        .unwrap();
    window.join().unwrap();
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}
