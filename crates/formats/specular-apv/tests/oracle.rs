//! The FATE APV sample against FFmpeg's decode (tools/corpus/fetch-fmt500d.sh, then
//! tools/corpus/fmt500d-oracle.py). Run with `SPECULAR_FMT500D_CORPUS=$CARGO_TARGET_DIR/corpus/fmt500d
//! cargo test -p specular-apv --test oracle -- --ignored --nocapture`.

use std::path::Path;

#[test]
#[ignore]
fn fate_sample_matches_ffmpeg() {
    let Some(root) = std::env::var_os("SPECULAR_FMT500D_CORPUS") else {
        eprintln!("SPECULAR_FMT500D_CORPUS not set");
        return;
    };
    let root = Path::new(&root);
    let data = std::fs::read(root.join("profile_422-10.apv")).unwrap();
    let want = std::fs::read(root.join("gen/profile_422-10.yuv")).unwrap();
    assert!(specular_apv::is_raw_file(&data));
    let mut at = 0;
    let (mut frames, mut bad, mut max) = (0, 0usize, 0u16);
    for (off, len) in specular_apv::raw_units(&data) {
        let p = specular_apv::decode(&data[off..off + len]).unwrap();
        assert_eq!(p.damaged_tiles, 0);
        for c in 0..3 {
            let (w, h) = p.plane_size(c);
            for &v in &p.planes[c][..w * h] {
                let o = u16::from_le_bytes([want[at], want[at + 1]]);
                let d = v.abs_diff(o);
                bad += usize::from(d > 0);
                max = max.max(d);
                at += 2;
            }
        }
        frames += 1;
        println!(
            "frame {frames}: {}×{} {} {}-bit, profile {}",
            p.info.width,
            p.info.height,
            p.info.chroma.name(),
            p.info.bit_depth,
            p.info.profile_name()
        );
    }
    println!("profile_422-10.apv: {frames} frames, {bad} samples differ (max {max})");
    assert_eq!(at, want.len());
    assert_eq!(bad, 0);
}
