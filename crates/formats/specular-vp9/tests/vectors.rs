//! The libvpx VP9 test vectors (`tools/corpus/specular-vp9-fetch.sh`, remuxed to IVF) against
//! their per-frame MD5s. Frames past 16384 pixels wide must be refused in words. Skips without
//! the vectors (`SPECULAR_VP9_VECTORS` names their folder).

use std::path::PathBuf;

use specular_vp9::Error;

mod common;
use common::decode_md5s;

#[test]
fn libvpx_test_vectors() {
    let dir = std::env::var_os("SPECULAR_VP9_VECTORS").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tools/corpus/video2/specular-vp9")
        },
        PathBuf::from,
    );
    let Ok(list) = std::fs::read_dir(&dir) else {
        eprintln!("skipped: run tools/corpus/specular-vp9-fetch.sh");
        return;
    };
    let mut names: Vec<PathBuf> = list
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ivf"))
        .collect();
    names.sort();
    let (mut pass, mut refused, mut failed) = (0, 0, Vec::new());
    for path in &names {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let Ok(md5) = std::fs::read_to_string(path.with_extension("md5")) else {
            continue;
        };
        let want: Vec<&str> = md5
            .lines()
            .filter_map(|l| l.split_whitespace().next())
            .collect();
        let data = std::fs::read(path).unwrap();
        let t0 = std::time::Instant::now();
        let (got, err) = decode_md5s(&data, 4);
        if name.contains("yuv44") {
            eprintln!(
                "  {name}: {:.0} fps",
                got.len() as f64 / t0.elapsed().as_secs_f64()
            );
        }
        // Frames past the size limit (19200 wide in vp90-2-13-largescaling) are refused.
        if matches!(&err, Some(Error::Unsupported(m)) if m.contains("the limit is")) {
            refused += 1;
            println!("ORACLE\tVP9\t{name}\tpass");
            eprintln!("  {name}: refused, {err:?}");
            continue;
        }
        let bad = got.iter().zip(&want).filter(|(a, b)| a != b).count();
        let ok = err.is_none() && bad == 0 && got.len() == want.len();
        println!("ORACLE\tVP9\t{name}\t{}", if ok { "pass" } else { "fail" });
        if !ok {
            let first = got.iter().zip(&want).position(|(a, b)| a != b);
            failed.push(format!(
                "{name}: {bad} of {} frames differ (first {first:?}), {} decoded, {err:?}",
                want.len(),
                got.len()
            ));
        } else {
            pass += 1;
        }
    }
    eprintln!(
        "VP9 vectors: {pass} exact, {refused} refused (size), {} failed",
        failed.len()
    );
    for f in &failed {
        eprintln!("  {f}");
    }
    assert!(failed.is_empty(), "{} vectors failed", failed.len());
}
