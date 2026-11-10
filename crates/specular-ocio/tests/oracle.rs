//! specular-ocio against PyOpenColorIO (`tools/corpus/ocio-oracle.py`): the ACES 1.3 cg
//! and studio configs' display/view processors and a config of LUT files and every other
//! transform type, on test ramps; and the GPU bake (log2 shaper + 65³ table, looked up
//! trilinearly as the shader does) against the same answers.
//!
//! The oracle comes from `tools/corpus/fetch-remote-sensing.sh`
//! (SPECULAR_REMOTE_SENSING_CORPUS/ocio, else tools/corpus/remote-sensing/ocio); the test skips
//! without its `expected.jsonl`.

use std::path::{Path, PathBuf};

use specular_ocio::{Config, Processor, bake};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_REMOTE_SENSING_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/remote-sensing")
        })
        .join("ocio");
    if dir.join("expected.jsonl").is_file() {
        Some(dir)
    } else {
        eprintln!("no OCIO oracle in {}; skipping", dir.display());
        None
    }
}

fn nums(v: &str, key: &str) -> Vec<[f64; 3]> {
    // A tiny reader for the oracle's `"key": [[r, g, b], …]` arrays.
    let start = v
        .find(&format!("\"{key}\": ["))
        .map(|i| i + key.len() + 4)
        .unwrap();
    let mut depth = 0;
    let mut end = start;
    for (i, c) in v[start..].char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    v[start + 1..end - 1]
        .split("], [")
        .map(|t| {
            let n: Vec<f64> = t
                .trim_matches(|c| c == '[' || c == ']' || c == ' ')
                .split(',')
                .map(|x| x.trim().parse().unwrap())
                .collect();
            [n[0], n[1], n[2]]
        })
        .collect()
}

fn field(v: &str, key: &str) -> Option<String> {
    let i = v.find(&format!("\"{key}\": \""))? + key.len() + 5;
    let j = v[i..].find('"')? + i;
    Some(v[i..j].to_owned())
}

#[test]
fn ocio_matches_pyopencolorio() {
    let Some(dir) = corpus() else { return };
    let text = std::fs::read_to_string(dir.join("expected.jsonl")).unwrap();
    let mut configs = std::collections::HashMap::new();
    let (mut cases, mut refused) = (0, Vec::new());
    let mut worst: Vec<(f64, String)> = Vec::new();
    let mut worst_baked: Vec<(f64, String)> = Vec::new();
    for line in text.lines() {
        let cfg_name = field(line, "config").unwrap();
        let cfg: &Config = configs.entry(cfg_name.clone()).or_insert_with(|| {
            Config::load(&dir.join(format!("{cfg_name}.ocio"))).expect("config")
        });
        let src = field(line, "src").unwrap();
        let (p, what) = match (
            field(line, "display"),
            field(line, "view"),
            field(line, "dst"),
        ) {
            (Some(d), Some(v), _) => (
                Processor::display_view(cfg, &src, &d, &v),
                format!("{cfg_name}: {src} → {d} / {v}"),
            ),
            (_, _, Some(dst)) => (
                Processor::colorspaces(cfg, &src, &dst),
                format!("{cfg_name}: {src} → {dst}"),
            ),
            _ => panic!("bad case"),
        };
        let p = match p {
            Ok(p) => p,
            Err(e) => {
                refused.push(format!("{what}: {e}"));
                continue;
            }
        };
        cases += 1;
        let (input, want) = (nums(line, "in"), nums(line, "out"));
        let mut max = 0f64;
        for (i, w) in input.iter().zip(&want) {
            let o = p.apply(*i);
            // Relative above 1: OCIO computes in f32.
            for k in 0..3 {
                max = max.max((o[k] - w[k]).abs() / w[k].abs().max(1.0));
            }
        }
        if max > 2e-3 && std::env::var_os("OCIO_DEBUG").is_some() {
            for (i, w) in input.iter().zip(&want).take(400) {
                let o = p.apply(*i);
                if (0..3).any(|k| (o[k] - w[k]).abs() / w[k].abs().max(1.0) > 2e-3) {
                    eprintln!("    {what}: {i:?} → {o:?}, OCIO {w:?}");
                    break;
                }
            }
        }
        worst.push((max, what.clone()));
        // The GPU bake for display views of scene-linear inputs (display values within
        // 0..1 after encoding are what the table holds).
        let scene_linear = matches!(
            src.as_str(),
            "ACEScg" | "ACES2065-1" | "Linear Rec.709 (sRGB)"
        );
        if field(line, "display").is_some() && scene_linear {
            let b = bake(&p, 65, -12.0, 4.0);
            let mut mb = 0f64;
            let mut shown = 0;
            for (i, w) in input.iter().zip(&want) {
                // Inputs in the shaper's range whose display values the display can show
                // (a table cannot follow values far outside it).
                if i.iter().any(|&x| x < 2f64.powi(-12) || x > 16.0)
                    || w.iter().any(|&v| !(-1e-3..=1.001).contains(&v))
                {
                    continue;
                }
                shown += 1;
                let o = b.lookup([i[0] as f32, i[1] as f32, i[2] as f32]);
                // What an SDR display shows: both clipped to 0..1.
                if std::env::var_os("OCIO_DEBUG").is_some()
                    && (0..3).any(|k| {
                        (f64::from(o[k]).clamp(0.0, 1.0) - w[k].clamp(0.0, 1.0)).abs() > 0.02
                    })
                {
                    eprintln!(
                        "    baked {what}: {i:?} → {o:?}, OCIO {w:?}, cpu {:?}",
                        p.apply(*i)
                    );
                }
                for k in 0..3 {
                    mb = mb.max((f64::from(o[k]).clamp(0.0, 1.0) - w[k].clamp(0.0, 1.0)).abs());
                }
            }
            worst_baked.push((mb, format!("{what} ({shown} values)")));
        }
    }
    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
    worst_baked.sort_by(|a, b| b.0.total_cmp(&a.0));
    eprintln!(
        "ocio: {cases} cases against PyOpenColorIO, {} refused",
        refused.len()
    );
    for r in &refused {
        eprintln!("  refused: {r}");
    }
    for (e, w) in worst.iter().take(12) {
        eprintln!("  CPU max |Δ| {e:.2e}  {w}");
    }
    for (e, w) in worst_baked.iter().take(4) {
        eprintln!("  baked max |Δ| {e:.2e}  {w}");
    }
    // The viewer's own working space (linear Rec.709) is what the GPU table is indexed by.
    for (e, w) in worst_baked
        .iter()
        .filter(|(_, w)| w.contains("Linear Rec.709"))
    {
        eprintln!("  baked max |Δ| {e:.2e}  {w}");
    }
    let bad: Vec<_> = worst.iter().filter(|(e, _)| *e > 1e-3).collect();
    assert!(bad.is_empty(), "{bad:?}");
    let bad: Vec<_> = worst_baked.iter().filter(|(e, _)| *e > 0.1).collect();
    assert!(bad.is_empty(), "baked: {bad:?}");
}
