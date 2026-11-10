//! Public XISF files (written by PixInsight and libXISF) against the xisf package, or, for embedded
//! data blocks the package cannot read, a small reader written from the XISF 1.0 spec in
//! tools/corpus/oracle-samples.py (tools/corpus/fetch-oracle-samples.sh xisf). Reads
//! `SPECULAR_ORACLE_SAMPLES_CORPUS`/xisf, else `$CARGO_TARGET_DIR/corpus/oracle-samples/xisf`;
//! skips when absent. Run with `cargo test -p specular-xisf --test oracle -- --ignored
//! --nocapture`.

use std::path::PathBuf;

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE_SAMPLES_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(|t| PathBuf::from(t).join("corpus/oracle-samples"))
        })?;
    let dir = base.join("xisf");
    dir.join("expect.tsv").is_file().then_some(dir)
}

#[test]
#[ignore]
fn corpus_matches_xisf_package() {
    let Some(dir) = corpus() else {
        eprintln!(
            "oracle-samples xisf corpus missing: run tools/corpus/fetch-oracle-samples.sh xisf"
        );
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    let mut failures = Vec::new();
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let name = f[0];
        let verdict = check(&dir, &f);
        let ok = if verdict.is_ok() { "pass" } else { "fail" };
        println!("ORACLE\tXISF\t{name}#{}\t{ok}", f[1]);
        if let Err(e) = verdict {
            failures.push(format!("{name}: {e}"));
        }
    }
    eprintln!(
        "xisf oracle: {} images, {} failed",
        tsv.lines().count(),
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

fn check(dir: &std::path::Path, f: &[&str]) -> Result<(), String> {
    let data = std::fs::read(dir.join("files").join(f[0])).unwrap();
    let x = specular_xisf::parse(&data).map_err(|e| e.to_string())?;
    if f[1] == "error" {
        return Err(format!("oracle refuses: {}", f[2]));
    }
    let i: usize = f[1].parse().unwrap();
    let img = x.images.get(i).ok_or("image missing")?;
    let (w, h, c): (u32, u32, u32) = (
        f[2].parse().unwrap(),
        f[3].parse().unwrap(),
        f[4].parse().unwrap(),
    );
    if (img.width, img.height, img.channels) != (w, h, c) {
        return Err(format!(
            "{}x{}x{} vs {w}x{h}x{c}",
            img.width, img.height, img.channels
        ));
    }
    if img.sample.name() != f[5] {
        return Err(format!("sample format {} vs {}", img.sample.name(), f[5]));
    }
    let kw: usize = f[7].parse().unwrap();
    if img.fits_keywords.len() != kw {
        return Err(format!("{} FITS keywords vs {kw}", img.fits_keywords.len()));
    }
    let raw = std::fs::read(dir.join("expect").join(f[8])).unwrap();
    let want: Vec<f32> = raw
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    let plane = (w * h) as usize;
    for ch in 0..c {
        let got = img.plane_f32(&data, ch, 0).map_err(|e| e.to_string())?;
        let exp = &want[ch as usize * plane..][..plane];
        let bad = got.iter().zip(exp).filter(|(a, b)| a != b).count();
        if bad > 0 {
            return Err(format!("channel {ch}: {bad} of {plane} samples differ"));
        }
    }
    Ok(())
}
