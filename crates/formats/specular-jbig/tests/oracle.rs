//! Corpus check against the images the files were made from (JBIG is lossless) and JBIG-KIT's
//! decode: `tools/corpus/fetch-raster-jbig.sh`, then
//! `SPECULAR_RASTER_CORPUS_JBIG=tools/corpus/raster/jbig/files cargo test -p specular-jbig
//! --test oracle -- --ignored --nocapture`.

mod common;

use common::read_pnm;
use specular_jbig::{Pixels, decode, info};
use std::path::Path;
use std::time::Instant;

#[test]
#[ignore]
fn corpus_matches_sources() {
    let Some(dir) = std::env::var_os("SPECULAR_RASTER_CORPUS_JBIG") else {
        eprintln!("SPECULAR_RASTER_CORPUS_JBIG not set; skipping");
        return;
    };
    let dir = Path::new(&dir);
    let expect = std::fs::read_to_string(dir.join("../expect.tsv")).expect("expect.tsv");
    let (mut pass, mut fail, mut kit_fail) = (0, 0, 0);
    let mut total_px = 0u64;
    let mut secs = 0f64;
    for line in expect.lines() {
        let Some((name, src)) = line.split_once('\t') else {
            continue;
        };
        let data = std::fs::read(dir.join(name)).unwrap();
        let want = read_pnm(&std::fs::read(dir.join(src)).unwrap());
        let t0 = Instant::now();
        let got = decode(&data, 0);
        secs += t0.elapsed().as_secs_f64();
        let got = match got {
            Ok(g) => g,
            Err(e) => {
                println!("FAIL {name}: {e}");
                println!("ORACLE\tJBIG\t{name}\tfail");
                fail += 1;
                continue;
            }
        };
        let i = info(&data, 0).unwrap();
        assert_eq!((i.width, i.height), (got.width, got.height));
        let px: Vec<u16> = match &got.pixels {
            Pixels::Gray8(v) => v.iter().map(|&x| u16::from(x)).collect(),
            Pixels::Gray16(v) => v.clone(),
            _ => unreachable!(),
        };
        total_px += px.len() as u64;
        let wide = matches!(got.pixels, Pixels::Gray16(_));
        let want_px = want.scaled(if wide { 65535 } else { 255 });
        let mism = if (got.width, got.height) != (want.width, want.height) {
            usize::MAX
        } else {
            px.iter().zip(&want_px).filter(|(a, b)| a != b).count()
        };
        // JBIG-KIT's own decode of the file, where it has one.
        let ext = Path::new(src).extension().unwrap().to_str().unwrap();
        let kit = dir.join(format!("{}.jbigkit.{ext}", name.trim_end_matches(".jbg")));
        if let Ok(k) = std::fs::read(&kit) {
            let k = read_pnm(&k);
            if k.samples != want.samples {
                kit_fail += 1;
                println!("note: JBIG-KIT's decode of {name} differs from its source");
            }
        }
        if mism == 0 && !got.truncated {
            pass += 1;
            println!("ORACLE\tJBIG\t{name}\tpass");
        } else {
            fail += 1;
            println!("ORACLE\tJBIG\t{name}\tfail");
            println!(
                "FAIL {name}: {} mismatching pixels{}",
                if mism == usize::MAX {
                    "size differs, all".to_string()
                } else {
                    mism.to_string()
                },
                if got.truncated { ", truncated" } else { "" }
            );
        }
    }
    // The refused variant.
    if let Ok(d) = std::fs::read(dir.join("refuse_dl1.jbg")) {
        let e = decode(&d, 0).unwrap_err();
        println!("refuse_dl1.jbg: {e}");
        assert!(matches!(e, specular_jbig::Error::Unsupported(_)));
    }
    println!(
        "JBIG corpus: {pass} pass, {fail} fail of {}; JBIG-KIT differs from source on {kit_fail}; \
         {:.1} Mpixel in {:.3} s ({:.1} Mpixel/s, debug build)",
        pass + fail,
        total_px as f64 / 1e6,
        secs,
        total_px as f64 / 1e6 / secs.max(1e-9)
    );
    assert_eq!(fail, 0);
}
