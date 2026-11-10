//! The composite built from the layers against the merged image Photoshop stored, as
//! psd-tools reads it (oracle only, Python; without a merged image, psd-tools' composite). For
//! every layered RGB or grey PSD/PSB under `$SPECULAR_FIDELITY_CORPUS/psd`,
//! [`Psd::compose_layers`] is written out
//! as RGBA and `tools/corpus/psd-compose-oracle.py` compares it, printing one
//! `ORACLE<TAB>PSD-compose/<group><TAB>file<TAB>pass|fail|skip` line per file (counted by
//! `tools/ci/oracle-pass-rates.sh`, runner `fidelity-extra-psd-compose`).
//!
//! `SPECULAR_RAWPY` names the Python with psd-tools (default `python3`). Skipped when the
//! corpus is missing. Ignored: run with `--ignored --nocapture`. `SPECULAR_PSD_ONLY` keeps
//! the files whose path under `psd/` contains it.
//!
//! The corpus is `tools/corpus/fetch-fidelity.sh --extra psd psdag` plus the per-type
//! adjustment copies of `tools/corpus/psd-adjust-solo.py` in `psd/solo/` (compared with
//! psd-tools' composite, for the adjustment types psd-tools draws).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use specular_psd::*;

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("psd") || e.eq_ignore_ascii_case("psb"))
        {
            out.push(p);
        }
    }
}

#[test]
#[ignore = "oracle: needs the fidelity corpus and psd-tools"]
fn composite_from_layers_matches_the_merged_image() {
    let Some(root) = std::env::var_os("SPECULAR_FIDELITY_CORPUS").map(PathBuf::from) else {
        println!("SPECULAR_FIDELITY_CORPUS unset: skipped");
        return;
    };
    let root = root.join("psd");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    if let Ok(only) = std::env::var("SPECULAR_PSD_ONLY") {
        files.retain(|f| {
            f.strip_prefix(&root)
                .unwrap_or(f)
                .to_string_lossy()
                .contains(&only)
        });
    }
    if files.is_empty() {
        println!("no corpus at {}: skipped", root.display());
        return;
    }
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("psd-compose");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let mut manifest = String::new();
    for (k, f) in files.iter().enumerate() {
        let rel = f.strip_prefix(&root).unwrap_or(f).display().to_string();
        let Ok(data) = std::fs::read(f) else { continue };
        let Ok(psd) = parse(&data) else {
            let _ = writeln!(manifest, "{rel}\t{}\tskip\tunreadable", f.display());
            continue;
        };
        let info = psd.info();
        let why = if psd.layers().is_empty() {
            Some("no layers")
        } else if !matches!(info.mode, ColorMode::Rgb | ColorMode::Grayscale) {
            Some("colour mode left to the merged-image oracle")
        } else if info.depth == 32 {
            Some("32-bit (linear light)")
        } else {
            None
        };
        if let Some(why) = why {
            let _ = writeln!(manifest, "{rel}\t{}\tskip\t{why}", f.display());
            continue;
        }
        match psd.compose_layers(&data, &Options::default()) {
            Ok(p) => {
                let img = p.to_rgba8();
                let raw = out.join(format!("{k}.raw"));
                std::fs::write(&raw, &img.data).unwrap();
                let _ = writeln!(
                    manifest,
                    "{rel}\t{}\t{}x{}x{}\t{}",
                    f.display(),
                    img.width,
                    img.height,
                    img.format.channels(),
                    raw.display()
                );
            }
            Err(e) => {
                let _ = writeln!(manifest, "{rel}\t{}\tfail\t{e}", f.display());
            }
        }
    }
    let mpath = out.join("manifest.tsv");
    std::fs::write(&mpath, manifest).unwrap();
    let py = std::env::var("SPECULAR_RAWPY").unwrap_or_else(|_| "python3".into());
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/psd-compose-oracle.py");
    let st = std::process::Command::new(py)
        .arg("-I")
        .arg(script)
        .arg(&mpath)
        .status()
        .expect("python");
    assert!(st.success(), "psd-compose-oracle.py failed");
}

/// Shape layers drawn from their settings (fill, vector mask, `vstk` stroke) against the
/// pixels Photoshop stored for them, over the layer's rectangle: one
/// `ORACLE<TAB>PSD-shape/<group><TAB>file#layer<TAB>pass|fail` line per shape with a stroke,
/// premultiplied 8-bit, passing at a mean difference of at most 1.5 and at most 0.5 % of the
/// pixels off by more than 24 (the composite oracle's bar).
#[test]
#[ignore = "oracle: needs the fidelity corpus"]
fn shapes_from_settings_match_their_stored_pixels() {
    let Some(root) = std::env::var_os("SPECULAR_FIDELITY_CORPUS").map(PathBuf::from) else {
        println!("SPECULAR_FIDELITY_CORPUS unset: skipped");
        return;
    };
    let root = root.join("psd");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    let (mut pass, mut fail) = (0, 0);
    for f in &files {
        let rel = f.strip_prefix(&root).unwrap_or(f).display().to_string();
        if rel.starts_with("solo/") {
            continue;
        }
        let Ok(data) = std::fs::read(f) else { continue };
        let Ok(psd) = parse(&data) else { continue };
        let info = psd.info();
        if info.mode != ColorMode::Rgb || info.depth != 8 {
            continue;
        }
        let (w, h) = (info.width as usize, info.height as usize);
        for (i, l) in psd.layers().iter().enumerate() {
            let stroke = l.blocks.iter().any(|(k, _)| k == b"vstk");
            if !stroke || l.fill.is_none() || l.vector_mask.is_none() {
                continue;
            }
            let Ok(stored) = psd.decode_layer_planes(&data, i, &Options::default(), None) else {
                continue;
            };
            let Ok(Some(ours)) = psd.layer_from_settings(&data, i) else {
                continue;
            };
            let Samples::U8(s) = &stored.samples else {
                continue;
            };
            let n = w * h;
            let (x0, x1) = (l.left.max(0) as usize, (l.right.max(0) as usize).min(w));
            let (y0, y1) = (l.top.max(0) as usize, (l.bottom.max(0) as usize).min(h));
            let (mut sum, mut far, mut count) = (0f64, 0usize, 0usize);
            for y in y0..y1 {
                for x in x0..x1 {
                    let j = y * w + x;
                    // Stored alpha folds in the opacities; ours does not.
                    let k = f32::from(l.opacity) / 255.0 * f32::from(l.fill_opacity) / 255.0;
                    let (sa, oa) = (f32::from(s[3 * n + j]), ours[3 * n + j] * 255.0 * k);
                    let mut worst = (sa - oa).abs();
                    for c in 0..3 {
                        let sv = f32::from(s[c * n + j]) * sa / 255.0;
                        let ov = ours[c * n + j] * 255.0 * oa / 255.0;
                        worst = worst.max((sv - ov).abs());
                        sum += f64::from((sv - ov).abs());
                    }
                    sum += f64::from((sa - oa).abs());
                    far += usize::from(worst > 24.0);
                    count += 1;
                }
            }
            if count == 0 {
                continue;
            }
            let mean = sum / (count * 4) as f64;
            let frac = far as f64 / count as f64;
            let ok = mean <= 1.5 && frac <= 0.005;
            if ok {
                pass += 1;
            } else {
                fail += 1;
            }
            let group = rel.split('/').next().unwrap_or(".");
            println!(
                "ORACLE\tPSD-shape/{group}\t{rel}#{i}\t{}",
                if ok { "pass" } else { "fail" }
            );
            println!(
                "# {rel}#{i} {:?}: mean {mean:.2}/255, {:.2}% beyond 24",
                l.name,
                100.0 * frac
            );
        }
    }
    println!("# psd-shape: {pass} pass, {fail} fail");
}
