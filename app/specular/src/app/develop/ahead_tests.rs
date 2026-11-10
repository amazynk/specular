//! The next raw is read and developed ahead while the photo before is shown, and
//! opens from the engine without being read again.

#[allow(dead_code, clippy::all)]
#[path = "../../../../../crates/formats/specular-raw/tests/common/mod.rs"]
mod raw_common;

use std::path::Path;
use std::time::{Duration, Instant};

use raw_common::{DngCodec, DngSpec, write_dng};
use specular_shots::Harness;

use super::controls;
use super::preview::{Orient, Preview};
use crate::loader::Loader;

fn dng(path: &Path, w: u32, h: u32, level: f32) {
    let s = DngSpec::new(w, h, DngCodec::U16);
    let data: Vec<u16> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let v =
                level * (0.5 + 0.5 * (x as f32 / w as f32)) * (0.6 + 0.4 * (y as f32 / h as f32));
            (v * 65535.0).round() as u16
        })
        .collect();
    std::fs::write(path, write_dng(&s, &data)).expect("write");
}

/// Polls and uploads until `done`, at most 60 s.
fn until(p: &mut Preview, h: &mut Harness, loader: &Loader, done: impl Fn(&Preview) -> bool) {
    let t = Instant::now();
    while !done(p) {
        assert!(t.elapsed() < Duration::from_secs(60), "timed out");
        p.poll(loader);
        p.upload(&h.gpu, &mut h.ui);
        assert!(p.error.is_none(), "{:?}", p.error);
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn the_next_raw_opens_from_the_engine() {
    let Some(mut h) = Harness::new() else { return };
    let dir = std::env::temp_dir().join(format!("specular-develop-ahead-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let (a, b) = (dir.join("a.dng"), dir.join("b.dng"));
    dng(&a, 960, 640, 0.3);
    dng(&b, 960, 640, 0.6);
    let (loader, _events) = Loader::new(1);
    let mut p = Preview::default();
    let edit = controls::new_edit(&a);
    p.open(&a, &loader);
    p.request(&edit, Orient::default(), 512, true);
    until(&mut p, &mut h, &loader, |p| {
        p.texture(Orient::default(), true).is_some() && !p.busy()
    });

    p.want_next(Some((&b, controls::new_edit(&b))), &loader);
    assert_eq!(p.next_path(), Some(b.as_path()));
    until(&mut p, &mut h, &loader, Preview::next_staged);

    // Opened: nothing is read; the engine's source is taken.
    p.open(&b, &loader);
    assert!(!p.reading(), "read again");
    assert_eq!(p.next_path(), None);
    p.request(&controls::new_edit(&b), Orient::default(), 512, true);
    until(&mut p, &mut h, &loader, |p| {
        p.texture(Orient::default(), true).is_some() && !p.busy()
    });

    // Another photo lets the one ahead go; leaving lets everything go.
    p.want_next(Some((&a, controls::new_edit(&a))), &loader);
    p.want_next(None, &loader);
    assert_eq!(p.next_path(), None);
    p.close();
    let _ = std::fs::remove_dir_all(&dir);
}
