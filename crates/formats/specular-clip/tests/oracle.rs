//! Clip Studio Paint files (clip_to_psd's and lycoris0731/clip's test files; tools/corpus/
//! fetch-oracle-samples.sh clip) against Python's sqlite3 and Pillow: the canvas size, the preview
//! blob and the size Pillow decodes it to, the layer names. Reads
//! `SPECULAR_ORACLE_SAMPLES_CORPUS`/clip, else `$CARGO_TARGET_DIR/corpus/oracle-samples/clip`;
//! skips when absent. Run with `cargo test -p specular-clip --test oracle -- --ignored
//! --nocapture`.

use std::path::PathBuf;

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE_SAMPLES_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(|t| PathBuf::from(t).join("corpus/oracle-samples"))
        })?;
    let dir = base.join("clip");
    dir.join("expect.tsv").is_file().then_some(dir)
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xCBF2_9CE4_8422_2325, |h, &x| {
        (h ^ u64::from(x)).wrapping_mul(0x0100_0000_01B3)
    })
}

#[test]
#[ignore]
fn corpus_matches_sqlite3_and_pillow() {
    let Some(dir) = corpus() else {
        eprintln!(
            "oracle-samples clip corpus missing: run tools/corpus/fetch-oracle-samples.sh clip"
        );
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    let mut failures = Vec::new();
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let name = f[0];
        let data = std::fs::read(dir.join("files").join(name)).unwrap();
        let r = specular_clip::parse(&data)
            .map_err(|e| e.to_string())
            .and_then(|c| {
                let canvas = c.canvas.ok_or("no canvas")?;
                let num = |i: usize| f[i].parse::<f64>().unwrap();
                if (canvas.width, canvas.height) != (num(1), num(2)) {
                    return Err(format!(
                        "canvas {}x{} vs {}x{}",
                        canvas.width, canvas.height, f[1], f[2]
                    ));
                }
                let (len, hash) = (
                    f[3].parse::<usize>().unwrap(),
                    u64::from_str_radix(f[4], 16).unwrap(),
                );
                if (c.preview.bytes.len(), fnv(&c.preview.bytes)) != (len, hash) {
                    return Err("preview blob differs".into());
                }
                if (f64::from(c.preview.width), f64::from(c.preview.height)) != (num(5), num(6)) {
                    return Err(format!(
                        "preview {}x{} vs Pillow {}x{}",
                        c.preview.width, c.preview.height, f[5], f[6]
                    ));
                }
                let names: Vec<&str> = c.layers.iter().map(|l| l.name.as_str()).collect();
                if names.join("|") != f[7] {
                    return Err("layer names differ".into());
                }
                Ok(())
            });
        println!(
            "ORACLE\tCLIP\t{name}\t{}",
            if r.is_ok() { "pass" } else { "fail" }
        );
        if let Err(e) = r {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
