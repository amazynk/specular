//! Wall-clock timings for the big astro captures `tools/corpus/fetch-astrowcs.sh src_big`
//! makes: open plus first frame of a 4 GB SER, and open plus debayer of a 60 MP colour-camera
//! FITS. Ignored; run with `--release --ignored --nocapture` and SPECULAR_ASTROWCS_CORPUS set.

use std::sync::Arc;
use std::time::Instant;

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Registry};

fn time(name: &str) {
    let Some(dir) = std::env::var_os("SPECULAR_ASTROWCS_CORPUS") else {
        eprintln!("SPECULAR_ASTROWCS_CORPUS not set; skipped");
        return;
    };
    let path = std::path::Path::new(&dir).join("big").join(name);
    let t = Instant::now();
    let src = Arc::new(Source::open(&path).unwrap());
    let s = Registry::default().open(src).unwrap();
    let opened = t.elapsed();
    let c = CancelToken::new();
    let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
    let first = t.elapsed();
    let plane = s.nd_plane(&[0], &c).map(|p| p.unwrap().values.len());
    let planed = t.elapsed() - first;
    eprintln!(
        "{name}: {}x{} open {:.1} ms, open + full decode {:.1} ms; nd plane {:?} values in {:.1} ms",
        r.width,
        r.height,
        opened.as_secs_f64() * 1e3,
        first.as_secs_f64() * 1e3,
        plane,
        planed.as_secs_f64() * 1e3,
    );
}

#[test]
#[ignore]
fn ser_4g_first_frame() {
    time("capture-4g.ser");
}

#[test]
#[ignore]
fn cfa_60mp_debayer() {
    time("cfa-60mp.fits");
}
