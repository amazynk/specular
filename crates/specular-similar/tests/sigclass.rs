//! HiDPI galleries make class-512 tiles, and with each one the class-256 thumbnail the
//! signature is computed from (`generate_also`). Where that comes from a finer decode than a
//! class-256 request would use (JPEG DCT and PNG reduced decodes: a scale finer) its pixels
//! differ a little, most on screenshots' one-pixel lines;
//! the signatures must still group the same.

use std::path::Path;

use specular_core::CancelToken;
use specular_similar::group::{DUP_DHASH, DUP_PHASH, threshold};
use specular_similar::sig::{SIG_CLASS, Signature};
use specular_thumbs::generate::{GenOptions, generate, generate_also};

const FILES: [&str; 8] = [
    "crates/specular-decode/data/jpeg-calibration.jpg",
    "crates/specular-widgets/tests/shots/viewer-dark-2x.png",
    "crates/specular-widgets/tests/shots/settings-dark-2x.png",
    "tools/bench/gate/webp-lossy.webp",
    "crates/specular-decode/data/heic-calibration.heic",
    "crates/formats/specular-jpeg/fuzz/seeds/arith.jpg",
    "crates/specular-widgets/tests/shots/palette-light-2x.png",
    "crates/specular-widgets/tests/shots/info-dark-2x.png",
];

#[test]
fn signatures_from_a_larger_class_group_the_same() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let reg = specular_decode::Registry::new();
    let c = CancelToken::new();
    let sig =
        |px: &specular_thumbs::pixels::Pixels| Signature::of_rgba(px.width, px.height, &px.rgba);
    let mut direct = Vec::new();
    let mut along = Vec::new();
    let mut inexact = 0;
    for name in FILES {
        let s = reg.open_path(Path::new(&format!("{root}/{name}"))).unwrap();
        let want = generate(&*s, SIG_CLASS, GenOptions::default(), &c, &mut |_| {}).unwrap();
        let (_, also) = generate_also(
            &*s,
            512,
            Some(SIG_CLASS),
            GenOptions::default(),
            &c,
            &mut |_| {},
        )
        .unwrap();
        let also = also.unwrap();
        inexact += usize::from(!also.exact);
        let (a, b) = (sig(&want), sig(&also.pixels));
        let (p, d, v) = (
            a.phash_distance(&b),
            a.dhash_distance(&b),
            a.visual_distance(&b),
        );
        eprintln!(
            "{name}: exact {} pHash {p} dHash {d} visual {v:.4}",
            also.exact
        );
        // The same picture: a duplicate of itself, and linked at the strictest sensitivity.
        assert!(
            p <= DUP_PHASH && d <= DUP_DHASH,
            "{name}: pHash {p} dHash {d}"
        );
        assert!(v < threshold(0.0) * 0.75, "{name}: visual {v}");
        direct.push(a);
        along.push(b);
    }
    assert!(inexact >= 1, "the JPEG takes a finer decode for 512");
    // Between different pictures every link decision (looks only, the strictest rule) is the
    // same at every sensitivity, with either signature on either side.
    for s in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let t = threshold(s) * 0.75;
        for i in 0..FILES.len() {
            for j in 0..FILES.len() {
                if i == j {
                    continue;
                }
                let want = direct[i].visual_distance(&direct[j]) <= t;
                for (x, y) in [(&along[i], &direct[j]), (&along[i], &along[j])] {
                    assert_eq!(
                        x.visual_distance(y) <= t,
                        want,
                        "{} / {} at {s}",
                        FILES[i],
                        FILES[j]
                    );
                }
            }
        }
    }
}
