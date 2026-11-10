//! Photo CD against Pillow and ImageMagick (tools/corpus/retro2-oracle.py). Run with
//! `SPECULAR_RETRO2_CORPUS=<folder> cargo test -p specular-retro2 --test oracle -- --ignored
//! --nocapture`.

use specular_retro2::{Hints, Kind, decode};
use std::path::Path;

/// (largest difference, mean difference, samples differing by more than 3).
fn compare(ours: &[u8], theirs: &[u8]) -> (u8, f64, usize) {
    let mut max = 0u8;
    let mut sum = 0u64;
    let mut over = 0usize;
    let mut n = 0u64;
    for (p, q) in ours.chunks_exact(4).zip(theirs.chunks_exact(3)) {
        for c in 0..3 {
            let d = p[c].abs_diff(q[c]);
            max = max.max(d);
            sum += d as u64;
            n += 1;
            if d > 3 {
                over += 1;
            }
        }
    }
    (max, sum as f64 / n.max(1) as f64, over)
}

#[test]
#[ignore]
fn photo_cd_matches_pillow() {
    let Some(root) = std::env::var_os("SPECULAR_RETRO2_CORPUS") else {
        eprintln!("SPECULAR_RETRO2_CORPUS not set");
        return;
    };
    let root = Path::new(&root);
    let data = std::fs::read(root.join("synthetic.pcd")).unwrap();
    let img = decode(Kind::PhotoCd, &data, &Hints::default(), 0).unwrap();
    assert_eq!((img.width, img.height), (768, 512));
    let pil = std::fs::read(root.join("synthetic.pil.rgb")).unwrap();
    let (max, mean, over) = compare(&img.rgba, &pil);
    eprintln!("Pillow: max {max}, mean {mean:.3}, {over} samples off by more than 3");
    assert!(max <= 3 && over == 0);
    // ImageMagick compresses PhotoYCC's range differently (its own table): reported only.
    if let Ok(im) = std::fs::read(root.join("synthetic.im.rgb")) {
        let (max, mean, over) = compare(&img.rgba, &im);
        eprintln!("ImageMagick: max {max}, mean {mean:.3}, {over} samples off by more than 3");
    }
}
