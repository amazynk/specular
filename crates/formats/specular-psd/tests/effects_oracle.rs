//! Each layer effect against the merged image Photoshop stored: for every layered 8/16-bit
//! RGB PSD/PSB under `$SPECULAR_FIDELITY_CORPUS/psd` with a merged image, the composite built
//! from the layers is compared with it over the pixels one effect kind changes (the composite
//! with and without that kind differ there by more than two codes). One
//! `ORACLE<TAB>PSD-effect/<kind><TAB>file<TAB>pass|fail` line per file and kind, then a `#`
//! line with the region's size and mean difference with the effect drawn and left out.
//! Passing: a mean premultiplied difference of at most 3/255 over the region, and the effect
//! closer to the merged image than leaving it out.
//!
//! Ignored: run with `--ignored --nocapture`. `SPECULAR_PSD_ONLY` keeps the files whose path
//! contains it. The corpus is `tools/corpus/fetch-fidelity.sh --round 5 psd psdag`.

use std::path::{Path, PathBuf};

use specular_psd::*;

const KINDS: &[&str] = &[
    "Drop Shadow",
    "Outer Glow",
    "Pattern Overlay",
    "Gradient Overlay",
    "Color Overlay",
    "Satin",
    "Inner Glow",
    "Inner Shadow",
    "Stroke",
    "Bevel & Emboss",
];

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

/// Premultiplied RGBA, 0..255.
fn premul(img: &Image) -> Vec<[f32; 4]> {
    img.data
        .chunks(4)
        .map(|p| {
            let a = f32::from(p[3]);
            [
                f32::from(p[0]) * a / 255.0,
                f32::from(p[1]) * a / 255.0,
                f32::from(p[2]) * a / 255.0,
                a,
            ]
        })
        .collect()
}

/// Writes `img` as `<dir>/<name>.<suffix>.pam` (RGBA).
fn write_pam(dir: &Path, name: &str, suffix: &str, img: &Image) {
    let mut v = format!(
        "P7\nWIDTH {}\nHEIGHT {}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n",
        img.width, img.height
    )
    .into_bytes();
    v.extend_from_slice(&img.data);
    let _ = std::fs::write(dir.join(format!("{name}.{suffix}.pam")), v);
}

fn diff(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    (0..4).map(|c| (a[c] - b[c]).abs()).sum::<f32>() / 4.0
}

#[test]
#[ignore = "oracle: needs the fidelity corpus"]
fn each_effect_matches_the_merged_image() {
    let Some(root) = std::env::var_os("SPECULAR_FIDELITY_CORPUS").map(PathBuf::from) else {
        println!("SPECULAR_FIDELITY_CORPUS unset: skipped");
        return;
    };
    let root = root.join("psd");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    if let Ok(only) = std::env::var("SPECULAR_PSD_ONLY") {
        files.retain(|f| f.to_string_lossy().contains(&only));
    }
    let dump = std::env::var_os("SPECULAR_PSD_DUMP").map(PathBuf::from);
    let opts = Options::default();
    let (mut pass, mut fail) = (0, 0);
    for f in &files {
        let rel = f.strip_prefix(&root).unwrap_or(f).display().to_string();
        if rel.starts_with("solo/") {
            continue;
        }
        let Ok(data) = std::fs::read(f) else { continue };
        let Ok(psd) = parse(&data) else { continue };
        let info = psd.info();
        if psd.layers().is_empty()
            || info.mode != ColorMode::Rgb
            || info.depth == 32
            || info.composite != Composite::Present
        {
            continue;
        }
        let Ok(merged) = psd.decode_planes(&data, &opts) else {
            continue;
        };
        let Ok(full) = psd.compose_layers(&data, &opts) else {
            continue;
        };
        let reference = premul(&merged.to_rgba8());
        let full_img = full.to_rgba8();
        let ours = premul(&full_img);
        let name = rel.replace('/', "_");
        if let Some(dir) = &dump {
            write_pam(dir, &name, "ours", &full_img);
            write_pam(dir, &name, "ref", &merged.to_rgba8());
            for i in 0..psd.layers().len() {
                if psd.describe_effects(&data, i).is_empty() {
                    continue;
                }
                match psd.decode_layer_planes(&data, i, &opts, None) {
                    Ok(p) => write_pam(dir, &name, &format!("L{i}"), &p.to_rgba8()),
                    Err(e) => {
                        println!("## layer {i}: {e}");
                        if let Ok(Some(v)) = psd.layer_from_settings(&data, i) {
                            let n = (info.width * info.height) as usize;
                            let data = (0..n)
                                .flat_map(|j| (0..4).map(move |c| j + c * n))
                                .map(|k| (v[k] * 255.0).round() as u8)
                                .collect();
                            let img = Image {
                                width: info.width,
                                height: info.height,
                                format: PixelFormat::Rgba8,
                                data,
                                truncated: false,
                            };
                            write_pam(dir, &name, &format!("L{i}"), &img);
                        }
                    }
                }
            }
        }
        let whole: f32 = ours
            .iter()
            .zip(&reference)
            .map(|(a, b)| diff(a, b))
            .sum::<f32>()
            / ours.len() as f32;
        println!("# {rel}: whole-image mean {whole:.2}");
        if std::env::var_os("SPECULAR_PSD_DESCRIBE").is_some() {
            for (i, l) in psd.layers().iter().enumerate() {
                let d = psd.describe_effects(&data, i);
                if !d.is_empty() {
                    println!(
                        "## layer {i} {:?} [{} {} {} {}] vis {} op {} fill {}\n{d}",
                        l.name,
                        l.left,
                        l.top,
                        l.right,
                        l.bottom,
                        l.visible,
                        l.opacity,
                        l.fill_opacity
                    );
                }
            }
        }
        for kind in KINDS {
            let Ok(without) = psd.compose_layers_without_effects(&data, &opts, &[kind]) else {
                continue;
            };
            let without = without.to_rgba8();
            if let Some(dir) = &dump {
                write_pam(dir, &name, &kind.replace([' ', '&'], ""), &without);
            }
            let without = premul(&without);
            let (mut n, mut e_with, mut e_without) = (0usize, 0f64, 0f64);
            for ((a, b), r) in ours.iter().zip(&without).zip(&reference) {
                if (0..4).any(|c| (a[c] - b[c]).abs() > 2.0) {
                    n += 1;
                    e_with += f64::from(diff(a, r));
                    e_without += f64::from(diff(b, r));
                }
            }
            if n < 16 {
                continue;
            }
            let (m_with, m_without) = (e_with / n as f64, e_without / n as f64);
            let ok = m_with <= 3.0 && m_with < m_without;
            if ok {
                pass += 1;
            } else {
                fail += 1;
            }
            println!(
                "ORACLE\tPSD-effect/{kind}\t{rel}\t{}",
                if ok { "pass" } else { "fail" }
            );
            println!("#   {kind}: {n} px, mean {m_with:.2} drawn, {m_without:.2} left out");
        }
    }
    println!("# psd-effect: {pass} pass, {fail} fail");
}
