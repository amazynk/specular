//! Public Radiance and PFM files against OpenImageIO's decode of them (tools/corpus/
//! fetch-oracle4.sh radiance). Reads `SPECULAR_ORACLE4_CORPUS`/radiance, else
//! `$CARGO_TARGET_DIR/corpus/oracle4/radiance`; skips when absent. Run with
//! `cargo test -p specular-radiance --test oracle -- --ignored --nocapture`.

use std::path::PathBuf;

use specular_radiance::{DecodeOptions, OutputChannels, decode};

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE4_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| PathBuf::from(t).join("corpus/oracle4"))
        })?;
    let dir = base.join("radiance");
    dir.join("expect.tsv").is_file().then_some(dir)
}

#[test]
#[ignore]
fn corpus_matches_openimageio() {
    let Some(dir) = corpus() else {
        eprintln!("oracle4 radiance corpus missing: run tools/corpus/fetch-oracle4.sh radiance");
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    let mut failures = Vec::new();
    let mut checked = 0;
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let name = f[0];
        let fmt = if name.ends_with(".pfm") {
            "PFM"
        } else {
            "RGBE"
        };
        let data = std::fs::read(dir.join("files").join(name)).unwrap();
        let ours = decode(
            &data,
            &DecodeOptions {
                channels: OutputChannels::Rgb,
                ..DecodeOptions::default()
            },
        );
        let verdict = match (ours, f[1]) {
            (Err(e), "error") => {
                eprintln!("  {name}: both refuse ({e}; OIIO: {})", f[2]);
                Ok(())
            }
            (Err(e), _) => Err(format!("{name}: ours fails: {e}")),
            (Ok(_), "error") => Err(format!("{name}: OIIO fails ({}), ours decodes", f[2])),
            (Ok(img), _) => compare(&dir, name, &img, &f),
        };
        checked += 1;
        match verdict {
            Ok(()) => println!("ORACLE\t{fmt}\t{name}\tpass"),
            Err(e) => {
                println!("ORACLE\t{fmt}\t{name}\tfail");
                failures.push(e);
            }
        }
    }
    eprintln!(
        "radiance oracle: {checked} files, {} failed",
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

fn compare(
    dir: &std::path::Path,
    name: &str,
    img: &specular_radiance::Image,
    f: &[&str],
) -> Result<(), String> {
    let (w, h, c): (u32, u32, usize) = (
        f[1].parse().unwrap(),
        f[2].parse().unwrap(),
        f[3].parse().unwrap(),
    );
    if (img.width, img.height) != (w, h) {
        return Err(format!(
            "{name}: {}x{} vs OIIO {w}x{h}",
            img.width, img.height
        ));
    }
    let raw = std::fs::read(dir.join("expect").join(format!("{name}.f32"))).unwrap();
    let want: Vec<f32> = raw
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    // OIIO multiplies PFM samples by the header's |scale|; ours (like Pillow) returns them as
    // stored and reports the scale in `Info` (hopper_be.pfm has 2.5).
    let scale = img.info.scale;
    let mut worst = 0f32;
    let mut bad = 0usize;
    for (i, px) in img.data.chunks_exact(3).enumerate() {
        for (k, &g) in px.iter().enumerate() {
            let g = g * scale;
            // Grey PFM: OIIO returns one channel, ours replicates it.
            let o = want[i * c + k.min(c - 1)];
            let e = (g - o).abs() / o.abs().max(1e-30);
            if !(e <= 1e-6 || (g - o).abs() <= 1e-30) {
                bad += 1;
                worst = worst.max(e);
            }
        }
    }
    if bad > 0 {
        return Err(format!(
            "{name}: {bad} samples differ, worst relative error {worst}"
        ));
    }
    Ok(())
}
