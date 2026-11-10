//! The composite built from the layers against the merged image stored in the same file,
//! read by this crate (no outside tool): the file's own composite is the oracle. For every
//! layered RGB or grey PSD/PSB under `$SPECULAR_FIDELITY_CORPUS/psd` that stores a merged
//! image, [`Psd::compose_layers`] and [`Psd::decode_rgba8`] are compared premultiplied in
//! 8-bit; a file passes at a mean difference of at most 1.5 and at most 0.5 % of the pixels
//! off by more than 24 (the bar of `compose_oracle.rs`). Prints one
//! `ORACLE<TAB>PSD-merged/<group><TAB>file<TAB>pass|fail|skip` line per file, a `#` detail
//! line, then the totals.
//!
//! Skipped when the corpus is missing. Ignored: run with `--ignored --nocapture`.
//! `SPECULAR_PSD_ONLY` keeps the files whose path under `psd/` contains it. Per-type
//! adjustment copies in `solo/` are left out (their merged image is the original's).

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

/// Straight 8-bit image to premultiplied RGBA in 0..255.
fn premul(img: &Image) -> Vec<[f32; 4]> {
    let ch = img.format.channels();
    img.data
        .chunks_exact(ch)
        .map(|p| {
            let (c, a) = match ch {
                1 => ([p[0]; 3], 255),
                2 => ([p[0]; 3], p[1]),
                3 => ([p[0], p[1], p[2]], 255),
                _ => ([p[0], p[1], p[2]], p[3]),
            };
            let a = f32::from(a);
            [
                f32::from(c[0]) * a / 255.0,
                f32::from(c[1]) * a / 255.0,
                f32::from(c[2]) * a / 255.0,
                a,
            ]
        })
        .collect()
}

/// Mean difference and the fraction of pixels off by more than 24, or `None` when the
/// sizes differ. A merged image without transparency is Photoshop's composite flattened
/// on white, so ours is flattened the same way.
fn compare(ours: &Image, merged: &Image, flatten: bool) -> Option<(f64, f64)> {
    if (ours.width, ours.height) != (merged.width, merged.height) {
        return None;
    }
    let (mut a, mut b) = (premul(ours), premul(merged));
    // A merged image without transparency is the composite flattened on white.
    if flatten {
        for p in &mut a {
            let k = 255.0 - p[3];
            *p = [p[0] + k, p[1] + k, p[2] + k, 255.0];
        }
    }
    // A merged "transparency" that is zero everywhere is a saved selection: opaque.
    if b.iter().all(|p| p[3] == 0.0) {
        b = premul(&Image {
            format: PixelFormat::Rgba8,
            data: merged
                .data
                .chunks_exact(merged.format.channels())
                .flat_map(|p| {
                    let c = if p.len() >= 3 {
                        [p[0], p[1], p[2]]
                    } else {
                        [p[0]; 3]
                    };
                    [c[0], c[1], c[2], 255]
                })
                .collect(),
            ..merged.clone()
        });
    }
    let (mut sum, mut far) = (0f64, 0usize);
    for (p, q) in a.iter().zip(&b) {
        let mut worst = 0f32;
        for c in 0..4 {
            let d = (p[c] - q[c]).abs();
            sum += f64::from(d);
            worst = worst.max(d);
        }
        far += usize::from(worst > 24.0);
    }
    let n = a.len().max(1);
    Some((sum / (n * 4) as f64, far as f64 / n as f64))
}

#[test]
#[ignore = "oracle: needs the fidelity corpus"]
fn composite_from_layers_matches_the_stored_merged_image() {
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
    let (mut pass, mut fail, mut skip) = (0, 0, 0);
    let mut ms = 0u128;
    for f in &files {
        let rel = f.strip_prefix(&root).unwrap_or(f).display().to_string();
        if rel.starts_with("solo/") {
            continue;
        }
        let group = rel.split('/').next().unwrap_or(".");
        let Ok(data) = std::fs::read(f) else { continue };
        let Ok(psd) = parse(&data) else { continue };
        let info = psd.info();
        let why = if psd.layers().is_empty() {
            Some("no layers".to_string())
        } else if info.composite == Composite::Blank {
            Some("blank merged image (built from the layers instead)".into())
        } else if info.composite != Composite::Present {
            Some("no merged image".into())
        } else if !matches!(info.mode, ColorMode::Rgb | ColorMode::Grayscale) {
            Some(format!("{:?} colour mode", info.mode))
        } else if info.depth == 32 {
            Some("32-bit (linear light)".into())
        } else {
            None
        };
        if let Some(why) = why {
            skip += 1;
            println!("ORACLE\tPSD-merged/{group}\t{rel}\tskip\n# {rel}: {why}");
            continue;
        }
        let opts = Options::default();
        let t = std::time::Instant::now();
        let ours = psd.compose_layers(&data, &opts).map(|p| p.to_rgba8());
        ms += t.elapsed().as_millis();
        let merged = psd.decode_rgba8(&data, &opts);
        let (status, detail) = match (ours, merged) {
            (Ok(o), Ok(m)) => match compare(&o, &m, !info.has_alpha) {
                Some((mean, frac)) => {
                    let ok = mean <= 1.5 && frac <= 0.005;
                    let notes = psd.composite_notes(&data);
                    (
                        ok,
                        format!(
                            "mean {mean:.2}/255, {:.2}% beyond 24{}{}",
                            100.0 * frac,
                            if notes.not_drawn.is_empty() {
                                String::new()
                            } else {
                                format!("; not drawn: {}", notes.not_drawn.join(", "))
                            },
                            if notes.approximate.is_empty() {
                                String::new()
                            } else {
                                format!("; approximate: {}", notes.approximate.join(", "))
                            }
                        ),
                    )
                }
                None => (false, "size differs".into()),
            },
            (Err(e), _) => (false, format!("compose: {e}")),
            (_, Err(e)) => (false, format!("merged: {e}")),
        };
        if ok_count(status, &mut pass, &mut fail) {
            println!("ORACLE\tPSD-merged/{group}\t{rel}\tpass\n# {rel}: {detail}");
        } else {
            println!("ORACLE\tPSD-merged/{group}\t{rel}\tfail\n# {rel}: {detail}");
        }
    }
    println!("# psd-merged: {pass} pass, {fail} fail, {skip} skip; composing took {ms} ms");
}

fn ok_count(ok: bool, pass: &mut usize, fail: &mut usize) -> bool {
    if ok {
        *pass += 1;
    } else {
        *fail += 1;
    }
    ok
}
