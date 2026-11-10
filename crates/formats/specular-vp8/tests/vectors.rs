//! The libvpx VP8 test vectors (vp80-00..06) against their per-frame MD5s, fetched by
//! `tools/corpus/specular-vp8-fetch.sh` (skipped when absent). Each is decoded with one
//! and with four threads. `SPECULAR_VP8_VEC=<substring>` limits the run.

mod common;

use std::path::PathBuf;

#[test]
fn vectors() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tools/corpus/video2/specular-vp8/vectors");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        eprintln!("skipped: run tools/corpus/specular-vp8-fetch.sh");
        return;
    };
    let filter = std::env::var("SPECULAR_VP8_VEC").unwrap_or_default();
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".ivf") && n.contains(filter.as_str()))
        .collect();
    names.sort();
    if names.is_empty() {
        eprintln!("skipped: run tools/corpus/specular-vp8-fetch.sh");
        return;
    }
    let (mut pass, mut fail) = (0, Vec::new());
    for n in &names {
        let data = std::fs::read(dir.join(n)).unwrap();
        let want = common::expected(
            &std::fs::read_to_string(dir.join(format!("{n}.md5"))).unwrap_or_default(),
        );
        let frames = common::ivf_frames(&data);
        let mut why = String::new();
        for threads in [1, 4] {
            let (got, err) = common::decode_md5s(&frames, threads);
            let m = common::mismatches(&got, &want);
            if m > 0 || err.is_some() {
                why = format!(
                    "{threads} threads: {m}/{} frames differ, {err:?}",
                    want.len()
                );
            }
        }
        if why.is_empty() {
            pass += 1;
        } else {
            fail.push(format!("{n}: {why}"));
        }
    }
    eprintln!("VP8 vectors: {pass}/{} bit-exact", names.len());
    for f in &fail {
        eprintln!("  {f}");
    }
    assert!(fail.is_empty(), "{} vectors differ", fail.len());
}
