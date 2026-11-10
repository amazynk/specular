//! Text, negative layers and step-and-repeat against the independent oracle
//! (tools/corpus/pcb-text-oracle.py) on the generated fixtures (pcb-text-fixtures.py):
//!
//! ```text
//! python3 -I tools/corpus/pcb-text-fixtures.py $ENGINEERING_CORPUS $OUT
//! python3 -I tools/corpus/pcb-text-oracle.py $OUT > $OUT/pcb-text-oracle.json
//! SPECULAR_PCB_TEXT_FIXTURES=$OUT \
//!   cargo test -p specular-pcb --test pcb_text_corpus -- --ignored --nocapture
//! ```
//!
//! Every shape drawn on a layer's view (its box and whether it is drawn clear) is compared as
//! a multiset within 2 µm; a negative layer's first shape must be its profile, drawn dark.

use std::path::{Path, PathBuf};

use specular_pcb::Limits;
use specular_vector::kurbo::{BezPath, PathEl, Shape as _};
use specular_vector::{Color, Shape, VectorScene};

const TOL: f64 = 0.002;

fn fixtures() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var_os("SPECULAR_PCB_TEXT_FIXTURES")?);
    d.join("pcb-text-oracle.json").exists().then_some(d)
}

type Shp = ([f64; 4], bool);

/// Shapes of the layer named `layer` on the "Layers" sheet of `group` (or the first).
fn shapes(s: &VectorScene, group: Option<&str>, layer: &str) -> Option<Vec<Shp>> {
    let li = s
        .layers
        .iter()
        .position(|l| l.name.ends_with(&format!("({layer})")))? as u32;
    let sheet = s
        .sheets
        .iter()
        .find(|sh| sh.name == "Layers" && sh.group.as_deref() == group)?;
    let mut out = Vec::new();
    for it in sheet.items.iter().filter(|i| i.layer == li) {
        let Shape::Fill { path, .. } = &it.shape else {
            continue;
        };
        let clear = it.color == Color::Clear;
        let mut sub = BezPath::new();
        let mut flush = |sub: &mut BezPath| {
            if !sub.elements().is_empty() {
                let b = sub.bounding_box();
                out.push(([b.x0, b.y0, b.x1, b.y1], clear));
                *sub = BezPath::new();
            }
        };
        for el in path.elements() {
            if matches!(el, PathEl::MoveTo(_)) {
                flush(&mut sub);
            }
            sub.push(*el);
        }
        flush(&mut sub);
    }
    Some(out)
}

fn want(v: &serde_json::Value) -> Vec<Shp> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let a = s.as_array().unwrap();
            let f = |i: usize| a[i].as_f64().unwrap();
            ([f(0), f(1), f(2), f(3)], a[4].as_bool().unwrap())
        })
        .collect()
}

/// Mismatches between two multisets of shapes (greedy matching within [`TOL`]).
fn compare(have: &[Shp], want: &[Shp]) -> usize {
    let mut left: Vec<Shp> = have.to_vec();
    let mut bad = 0;
    for w in want {
        let hit = left
            .iter()
            .position(|h| h.1 == w.1 && h.0.iter().zip(&w.0).all(|(a, b)| (a - b).abs() <= TOL));
        match hit {
            Some(i) => {
                left.swap_remove(i);
            }
            None => bad += 1,
        }
    }
    bad + left.len()
}

fn read_dir(root: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) {
    for e in std::fs::read_dir(root).unwrap() {
        let e = e.unwrap();
        let name = format!("{prefix}/{}", e.file_name().to_string_lossy());
        if e.file_type().unwrap().is_dir() {
            read_dir(&e.path(), &name, out);
        } else {
            out.push((name, std::fs::read(e.path()).unwrap()));
        }
    }
}

#[test]
#[ignore = "needs the generated fixtures (tools/corpus/pcb-text-fixtures.py)"]
fn text_negative_and_panels_match_the_oracle() {
    let Some(root) = fixtures() else {
        eprintln!("SPECULAR_PCB_TEXT_FIXTURES not set or without pcb-text-oracle.json");
        return;
    };
    let oracle: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("pcb-text-oracle.json")).unwrap()).unwrap();
    let (mut checked, mut mismatches) = (0usize, 0usize);
    for (job, steps) in oracle["odb"].as_object().unwrap() {
        let mut files = Vec::new();
        read_dir(&root.join("odb").join(job), job, &mut files);
        let t0 = std::time::Instant::now();
        let s = specular_pcb::odb::scene(&files, &Limits::default()).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let grouped = s.sheets.iter().any(|sh| sh.group.is_some());
        for (step, layers) in steps.as_object().unwrap() {
            for (layer, w) in layers.as_object().unwrap() {
                let w = want(w);
                let group = grouped.then_some(step.as_str());
                let Some(h) = shapes(&s, group, layer) else {
                    eprintln!("{job}/{step}/{layer}: not drawn  MISMATCH");
                    mismatches += 1;
                    continue;
                };
                let bad = compare(&h, &w);
                // A negative layer: its profile first, dark.
                let neg_ok = !w.first().is_some_and(|f| !f.1 && h.first() != Some(f))
                    || h.first().is_some_and(|f| {
                        !f.1 && f.0.iter().zip(&w[0].0).all(|(a, b)| (a - b).abs() <= TOL)
                    });
                checked += w.len();
                mismatches += bad + usize::from(!neg_ok);
                eprintln!(
                    "{job}/{step}/{layer}: {} shapes ({} clear), {ms:.0} ms{}",
                    w.len(),
                    w.iter().filter(|s| s.1).count(),
                    if bad > 0 || !neg_ok {
                        format!("  MISMATCH {bad}")
                    } else {
                        String::new()
                    }
                );
            }
        }
        eprintln!("{job}: warnings {:?}", s.warnings);
    }
    for (file, layers) in oracle["ipc2581"].as_object().unwrap() {
        let b = std::fs::read(root.join("ipc2581").join(file)).unwrap();
        let s = specular_pcb::ipc2581::scene(&b, &Limits::default()).unwrap();
        for (layer, w) in layers.as_object().unwrap() {
            let w = want(w);
            let h = shapes(&s, None, layer).unwrap_or_default();
            let bad = compare(&h, &w);
            checked += w.len();
            mismatches += bad;
            eprintln!(
                "{file}/{layer}: {} shapes{}",
                w.len(),
                if bad > 0 {
                    format!("  MISMATCH {bad}")
                } else {
                    String::new()
                }
            );
        }
        eprintln!("{file}: warnings {:?}", s.warnings);
    }
    eprintln!("{checked} shapes checked, {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}
