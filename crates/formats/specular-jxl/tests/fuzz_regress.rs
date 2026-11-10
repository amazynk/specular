//! Inputs found by the fuzzer (`fuzz/`), checked against the invariant they broke.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::sync::mpsc;
use std::time::Duration;

use specular_jxl::JxlFile;

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// Run `f` on another thread; fail if it does not finish within a minute (a hang).
fn finishes(what: &str, f: impl FnOnce() + Send + 'static) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        f();
        let _ = tx.send(());
    });
    rx.recv_timeout(Duration::from_secs(60))
        .unwrap_or_else(|_| panic!("{what}: still running after 60 s"));
}

/// An animation with a frame that fails while being blended (`fuzz-failed-blend.jxl`, a fuzzer
/// input). That frame's render state was left "rendering", so a later frame blending onto it
/// waited for it forever, even single-threaded.
#[test]
fn failed_blend_does_not_hang_later_frames() {
    let d = data("fuzz-failed-blend.jxl");
    for parallel in [false, true] {
        let d = d.clone();
        finishes(&format!("parallel {parallel}"), move || {
            let f = JxlFile::open(&d).unwrap();
            for r in f.frames(2, parallel).unwrap().take(4) {
                let _ = r;
            }
        });
    }
}
