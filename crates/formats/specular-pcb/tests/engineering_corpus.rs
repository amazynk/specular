//! IPC-2581 and ODB++ over the public samples (tools/corpus/fetch-engineering.sh), against the
//! independent counts of tools/corpus/pcb-xml-oracle.py:
//!
//! python3 -I tools/corpus/pcb-xml-oracle.py $CORPUS > $CORPUS/ipc2581-oracle.json
//! SPECULAR_ENGINEERING_CORPUS=$CORPUS cargo test -p specular-pcb --test engineering_corpus --
//! --ignored --nocapture

use std::path::{Path, PathBuf};

use specular_pcb::Limits;
use specular_vector::Shape;

fn corpus() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var_os("SPECULAR_ENGINEERING_CORPUS")?);
    d.is_dir().then_some(d)
}

fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            files(&p, ext, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
            out.push(p);
        }
    }
}

/// With `SPECULAR_ENGINEERING_RENDERS` set: every sheet of `scene` as `<dir>/<name>-<sheet>.png`
/// (1600 px, for looking at).
fn render(name: &str, scene: specular_vector::VectorScene) {
    use specular_vector::draw::{View, fit_transform};
    let Some(dir) = std::env::var_os("SPECULAR_ENGINEERING_RENDERS") else {
        return;
    };
    let fonts = specular_vector::fonts::Fonts::bundled_only();
    let p = specular_vector::prep::prepare(
        std::sync::Arc::new(scene),
        &fonts,
        specular_vector::prep::PrepLimits::default(),
    );
    for (i, sh) in p.sheets.iter().enumerate() {
        let Some(fit) = sh.fit else { continue };
        let size = (1600, 1200);
        let layers = vec![true; p.scene.layers.len()];
        let view = View {
            transform: fit_transform(fit, size, 8.0),
            size,
            dark: true,
            background: [20, 20, 24],
            lineweights: true,
            dpi_scale: 1.0,
            layers: &layers,
        };
        if let Some((rgba, _)) = specular_vector::raster::render(sh, &view) {
            let png =
                specular_png::encode(size.0, size.1, specular_png::ColorType::Rgba, &rgba).unwrap();
            let out = Path::new(&dir).join(format!("{name}-{i}-{}.png", sh.name));
            std::fs::write(out, png).unwrap();
        }
    }
}

/// A `.tgz` job's files, `.z` files unpacked.
fn job_files(p: &Path) -> Vec<(String, Vec<u8>)> {
    let gz = std::fs::read(p).unwrap();
    let tar = specular_png::inflate_gzip(&gz, 1 << 30).unwrap();
    let files = specular_pcb::odb::untar(&tar, 1 << 30).unwrap();
    files
        .into_iter()
        .map(|(n, b)| specular_pcb::odb::unpack(&n, b, 1 << 28).unwrap())
        .collect()
}

#[test]
#[ignore = "needs the corpus (tools/corpus/fetch-engineering.sh)"]
fn odb_samples_match_the_oracle() {
    let Some(root) = corpus() else {
        eprintln!("SPECULAR_ENGINEERING_CORPUS not set");
        return;
    };
    let oracle: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("odb-oracle.json")).expect("oracle json"),
    )
    .unwrap();
    let mut jobs = Vec::new();
    files(&root.join("odb"), "tgz", &mut jobs);
    let (mut checked, mut mismatches) = (0, 0);
    for p in jobs {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let t0 = std::time::Instant::now();
        let fs = job_files(&p);
        let s = specular_pcb::odb::scene(&fs, &Limits::default());
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let want = &oracle[&name];
        let s = match s {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{name}: {e}  MISMATCH");
                mismatches += 1;
                continue;
            }
        };
        let info = s.pcb.as_ref().unwrap();
        let count = |k: &str| s.counts.iter().find(|c| c.0 == k).map_or(0, |c| c.1);
        let got = [
            ("pads", count("Pad")),
            ("lines", count("Line")),
            ("arcs", count("Arc")),
            ("surfaces", count("Surface")),
            ("texts", count("Text")),
            ("components", info.components),
            ("holes", info.holes),
            ("layers", info.layers.len() as u64),
        ];
        let mut bad = Vec::new();
        for (k, v) in got {
            checked += 1;
            let w = want[k].as_u64().unwrap_or(u64::MAX);
            if w != v {
                bad.push(format!("{k} {v} vs {w}"));
            }
        }
        let size = info.size_mm.unwrap_or_default();
        if let Some(ws) = want["profile_mm"].as_array() {
            checked += 1;
            let (ww, wh) = (ws[0].as_f64().unwrap(), ws[1].as_f64().unwrap());
            if (size.0 - ww).abs() > 0.01 || (size.1 - wh).abs() > 0.01 {
                bad.push(format!("size {size:?} vs ({ww}, {wh})"));
            }
        }
        mismatches += bad.len();
        render(&name, s.clone());
        let items: usize = s.sheets.iter().map(|sh| sh.items.len()).sum();
        eprintln!(
            "{name}: {} layers, {:.1}×{:.1} mm, {items} items, {ms:.0} ms, warnings {:?}{}",
            info.layers.len(),
            size.0,
            size.1,
            s.warnings,
            if bad.is_empty() {
                String::new()
            } else {
                format!("  MISMATCH {}", bad.join("; "))
            }
        );
    }
    eprintln!("{checked} values checked, {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}

#[test]
#[ignore = "needs the corpus (tools/corpus/fetch-engineering.sh)"]
fn ipc2581_samples_match_the_oracle() {
    let Some(root) = corpus() else {
        eprintln!("SPECULAR_ENGINEERING_CORPUS not set");
        return;
    };
    let oracle: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("ipc2581-oracle.json")).expect("oracle json"),
    )
    .unwrap();
    let mut xs = Vec::new();
    files(&root.join("ipc2581"), "xml", &mut xs);
    let mut mismatches = 0;
    let mut checked = 0;
    for p in xs {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(&p).unwrap();
        let t0 = std::time::Instant::now();
        let r = specular_pcb::ipc2581::scene(&bytes, &Limits::default());
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let want = &oracle[&name];
        match r {
            Err(e) => {
                let refused =
                    want["ecad"].as_bool() == Some(false) || want["layers"].as_u64() == Some(0);
                eprintln!(
                    "{name}: {e} ({ms:.0} ms){}",
                    if refused { "" } else { "  MISMATCH" }
                );
                if !refused {
                    mismatches += 1;
                }
            }
            Ok(s) => {
                let info = s.pcb.as_ref().unwrap();
                let count = |k: &str| s.counts.iter().find(|c| c.0 == k).map_or(0, |c| c.1);
                let got = [
                    ("pads", count("Pad")),
                    ("lines", count("Line")),
                    ("arcs", count("Arc")),
                    ("contours", count("Contour")),
                    ("components", info.components),
                    ("holes", info.holes),
                    ("layers", info.layers.len() as u64),
                ];
                let mut bad = Vec::new();
                for (k, v) in got {
                    checked += 1;
                    let w = want[k].as_u64().unwrap_or(u64::MAX);
                    if w != v {
                        bad.push(format!("{k} {v} vs {w}"));
                    }
                }
                let size = info.size_mm.unwrap_or_default();
                let ws = want["profile_mm"].as_array();
                if let Some(ws) = ws {
                    checked += 1;
                    let (ww, wh) = (ws[0].as_f64().unwrap(), ws[1].as_f64().unwrap());
                    if (size.0 - ww).abs() > 0.01 || (size.1 - wh).abs() > 0.01 {
                        bad.push(format!("size {size:?} vs ({ww}, {wh})"));
                    }
                }
                if name.to_ascii_lowercase().contains("-full") {
                    render(&name, s.clone());
                }
                let items: usize = s.sheets.iter().map(|sh| sh.items.len()).sum();
                let texts = s.sheets[0]
                    .items
                    .iter()
                    .filter(|i| matches!(i.shape, Shape::Text(_)))
                    .count();
                mismatches += bad.len();
                eprintln!(
                    "{name}: {} layers, {:.1}×{:.1} mm, {items} items, {texts} refdes, {ms:.0} ms{}",
                    info.layers.len(),
                    size.0,
                    size.1,
                    if bad.is_empty() {
                        String::new()
                    } else {
                        format!("  MISMATCH {}", bad.join("; "))
                    }
                );
            }
        }
    }
    eprintln!("{checked} values checked, {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}
