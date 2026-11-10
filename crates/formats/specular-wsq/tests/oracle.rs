//! Every file of the WSQ corpus against NBIS's decoding (run tools/corpus/fetch-raster-wsq.sh,
//! then `SPECULAR_RASTER_CORPUS_WSQ=tools/corpus/raster/wsq cargo test -p specular-wsq --test
//! oracle -- --ignored --nocapture`).

mod common;

use common::*;
use specular_wsq::{Pixels, decode};
use std::time::Instant;

#[test]
#[ignore]
fn corpus_matches_nbis() {
    let Some(root) = std::env::var_os("SPECULAR_RASTER_CORPUS_WSQ") else {
        eprintln!("SPECULAR_RASTER_CORPUS_WSQ not set");
        return;
    };
    let root = std::path::PathBuf::from(root);
    let mut names: Vec<_> = std::fs::read_dir(root.join("files"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "wsq"))
        .collect();
    names.sort();
    let (mut pass, mut fail, mut no_oracle) = (0, 0, 0);
    let mut total_px = 0u64;
    let mut secs = 0f64;
    for p in &names {
        let stem = p.file_stem().unwrap().to_string_lossy();
        let Ok(e) = std::fs::read(root.join("expect").join(format!("{stem}.pgm"))) else {
            no_oracle += 1;
            println!("ORACLE\tWSQ\t{stem}\tskip");
            continue;
        };
        let (w, h, want) = pgm(&e);
        let data = std::fs::read(p).unwrap();
        let t = Instant::now();
        let r = decode(&data, 0);
        secs += t.elapsed().as_secs_f64();
        match r {
            Ok(img) => {
                let Pixels::Gray8(px) = &img.pixels else {
                    panic!()
                };
                let (n, m) = diff(px, &want);
                if (img.width, img.height) == (w, h) && n == 0 {
                    pass += 1;
                    println!("ORACLE\tWSQ\t{stem}\tpass");
                } else {
                    fail += 1;
                    eprintln!("MISMATCH {stem}: {n} pixels differ, max {m}");
                    println!("ORACLE\tWSQ\t{stem}\tfail");
                }
                total_px += u64::from(w) * u64::from(h);
            }
            Err(e) => {
                fail += 1;
                eprintln!("ERROR {stem}: {e}");
                println!("ORACLE\tWSQ\t{stem}\tfail");
            }
        }
    }
    eprintln!(
        "WSQ corpus: {} files, {pass} identical to NBIS, {fail} mismatched, {no_oracle} without oracle; {:.1} MP in {:.0} ms ({:.1} MP/s)",
        names.len(),
        total_px as f64 / 1e6,
        secs * 1e3,
        total_px as f64 / 1e6 / secs.max(1e-9)
    );
    assert_eq!(fail, 0);
}
