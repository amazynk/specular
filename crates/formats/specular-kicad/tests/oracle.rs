//! Every board and schematic of the corpus against tests/oracle.py's reading (its own
//! S-expression walk and kiutils):
//!
//! ```text
//! tools/corpus/fetch-pcb-spectra.sh kicad
//! target/pcb-spectra/venv/bin/python -I crates/formats/specular-kicad/tests/oracle.py \
//!     tools/corpus/pcb-spectra/kicad tools/corpus/pcb-spectra/kicad-expect.jsonl
//! SPECULAR_KICAD_CORPUS=tools/corpus/pcb-spectra/kicad \
//! SPECULAR_KICAD_EXPECT=tools/corpus/pcb-spectra/kicad-expect.jsonl \
//!     cargo test -p specular-kicad --test oracle -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::time::Instant;

use serde_json::Value;
use specular_kicad::{Error, Limits, pcb, sch};
use specular_vector::VectorScene;

fn count(s: &VectorScene, what: &str) -> u64 {
    s.counts.iter().find(|c| c.0 == what).map_or(0, |c| c.1)
}

fn get(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(Value::as_u64).unwrap_or(0)
}

#[test]
#[ignore]
fn corpus_matches_the_oracle() {
    // Starts the `ORACLE` lines on their own line (libtest prints `test NAME ... ` first).
    println!();
    let (Some(corpus), Some(expect)) = (
        std::env::var_os("SPECULAR_KICAD_CORPUS"),
        std::env::var_os("SPECULAR_KICAD_EXPECT"),
    ) else {
        eprintln!("SPECULAR_KICAD_CORPUS / SPECULAR_KICAD_EXPECT not set");
        return;
    };
    let corpus = PathBuf::from(corpus);
    let text = std::fs::read_to_string(expect).expect("expect file");
    let limits = Limits::default();
    let (mut files, mut checks, mut mismatches, mut ki_checks, mut ki_mismatches) = (0, 0, 0, 0, 0);
    let mut slowest = (0.0f64, String::new());
    for line in text.lines() {
        let rec: Value = serde_json::from_str(line).expect("json");
        let rel = rec["file"].as_str().unwrap().to_owned();
        let path = corpus.join(&rel);
        let bytes = std::fs::read(&path).expect("corpus file");
        files += 1;
        // One line per file for tools/ci/oracle-pass-rates.sh.
        let fmt = format!(
            "KiCad/{}",
            path.extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default()
        );
        let verdict = |status: &str| println!("ORACLE\t{fmt}\t{rel}\t{status}");
        let before = mismatches + ki_mismatches;
        let mut bad = |what: &str, ours: String, theirs: String, ki: bool| {
            if ki {
                ki_mismatches += 1;
            } else {
                mismatches += 1;
            }
            println!("MISMATCH {rel}: {what}: ours {ours}, oracle {theirs}");
        };
        let t0 = Instant::now();
        match rec["kind"].as_str().unwrap() {
            "legacy" => {
                checks += 1;
                match pcb::scene(&bytes, &limits) {
                    Err(Error::Unsupported(_)) => {}
                    r => bad(
                        "refusal",
                        format!("{:?}", r.map(|s| s.format)),
                        "legacy".into(),
                        false,
                    ),
                }
            }
            "broken" => verdict("skip"),
            "pcb" => {
                let s = match pcb::scene(&bytes, &limits) {
                    Ok(s) => s,
                    Err(e) => {
                        bad("open", format!("{e}"), "a board".into(), false);
                        verdict("fail");
                        continue;
                    }
                };
                let info = s.pcb.as_deref().unwrap();
                let ours = [
                    ("footprints", info.components),
                    ("pads", count(&s, "pad")),
                    ("segments", count(&s, "segment")),
                    ("arcs", count(&s, "arc")),
                    ("vias", info.vias),
                    ("zones", count(&s, "zone")),
                ];
                for (k, v) in ours {
                    checks += 1;
                    if v != get(&rec, k) {
                        bad(k, v.to_string(), get(&rec, k).to_string(), false);
                    }
                    if let Some(ki) = rec.get("kiutils") {
                        ki_checks += 1;
                        if v != get(ki, k) {
                            bad(
                                &format!("kiutils {k}"),
                                v.to_string(),
                                get(ki, k).to_string(),
                                true,
                            );
                        }
                    }
                }
                checks += 1;
                if info.nets != get(&rec, "nets") {
                    bad(
                        "nets",
                        info.nets.to_string(),
                        get(&rec, "nets").to_string(),
                        false,
                    );
                }
                if let Some(e) = rec.get("edge").and_then(Value::as_array) {
                    checks += 1;
                    let (w, h) = (e[0].as_f64().unwrap(), e[1].as_f64().unwrap());
                    let ok = info
                        .size_mm
                        .is_some_and(|(ow, oh)| (ow - w).abs() < 0.05 && (oh - h).abs() < 0.05);
                    if !ok {
                        bad(
                            "outline size",
                            format!("{:?}", info.size_mm),
                            format!("{w:.3} × {h:.3}"),
                            false,
                        );
                    }
                }
            }
            "sch" => {
                let dir = path.parent().unwrap().to_owned();
                let resolve = |f: &str| std::fs::read(dir.join(f)).ok();
                let s = match sch::scene(&bytes, &resolve, &limits) {
                    Ok(s) => s,
                    Err(e) => {
                        bad("open", format!("{e}"), "a schematic".into(), false);
                        verdict("fail");
                        continue;
                    }
                };
                let info = s.pcb.as_deref().unwrap();
                let ours = [
                    ("pages", s.sheets.len() as u64),
                    ("symbols", info.components),
                    ("power", count(&s, "power symbol")),
                    ("wires", count(&s, "wire")),
                    ("buses", count(&s, "bus")),
                    ("labels", count(&s, "label")),
                    ("directives", count(&s, "directive label")),
                    ("junctions", count(&s, "junction")),
                    ("sheets", count(&s, "sheet")),
                ];
                for (k, v) in ours {
                    checks += 1;
                    if v != get(&rec, k) {
                        bad(k, v.to_string(), get(&rec, k).to_string(), false);
                    }
                }
                // kiutils reads one file: compare with the root page alone.
                if let Some(ki) = rec.get("kiutils") {
                    let none = |_: &str| None;
                    let r = sch::scene(&bytes, &none, &limits).unwrap();
                    let ri = r.pcb.as_deref().unwrap();
                    let root = [
                        ("symbols", ri.components + count(&r, "power symbol")),
                        ("wires", count(&r, "wire")),
                        ("buses", count(&r, "bus")),
                        ("labels", count(&r, "label")),
                        ("junctions", count(&r, "junction")),
                        ("sheets", count(&r, "sheet")),
                    ];
                    for (k, v) in root {
                        ki_checks += 1;
                        if v != get(ki, k) {
                            bad(
                                &format!("kiutils {k}"),
                                v.to_string(),
                                get(ki, k).to_string(),
                                true,
                            );
                        }
                    }
                }
            }
            k => panic!("kind {k}"),
        }
        if rec["kind"].as_str() != Some("broken") {
            let failed = mismatches + ki_mismatches > before;
            verdict(if failed { "fail" } else { "pass" });
        }
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        if ms > slowest.0 {
            slowest = (ms, format!("{rel} ({} KB)", bytes.len() / 1024));
        }
    }
    println!(
        "{files} files: {mismatches} of {checks} checks differ from the oracle's own reading, \
         {ki_mismatches} of {ki_checks} from kiutils; slowest {:.0} ms: {}",
        slowest.0, slowest.1
    );
    assert_eq!(mismatches, 0);
    assert_eq!(ki_mismatches, 0);
}

/// Where tracks end: on a pad or via of their layer, or on another track's end. A turned or
/// mirrored footprint drawn wrong leaves its tracks dangling.
#[test]
#[ignore]
fn tracks_end_on_pads() {
    use specular_kicad::sexpr;
    use specular_vector::kurbo::{Point, Shape as _};
    let Some(corpus) = std::env::var_os("SPECULAR_KICAD_CORPUS") else {
        eprintln!("SPECULAR_KICAD_CORPUS not set");
        return;
    };
    let mut files = Vec::new();
    let mut stack = vec![PathBuf::from(corpus)];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "kicad_pcb") {
                files.push(p);
            }
        }
    }
    files.sort();
    let (mut ends, mut dangling) = (0usize, 0usize);
    for f in files {
        let bytes = std::fs::read(&f).unwrap();
        let Ok(pads) = pcb::pad_copper(&bytes) else {
            continue;
        };
        if pcb::scene(&bytes, &Limits::default()).is_err() {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        let root = sexpr::parse(&text).unwrap();
        let mut tracks: Vec<(String, Point, Point)> = Vec::new();
        for n in root.lists().filter(|n| n.is("segment") || n.is("arc")) {
            let p = |k: &str| {
                let c = n.child(k)?;
                Some(Point::new(c.num(0)?, -c.num(1)?))
            };
            if let (Some(a), Some(b), Some(l)) = (p("start"), p("end"), n.value("layer")) {
                tracks.push((l.to_owned(), a, b));
            }
        }
        let mut file_dangling = 0;
        for (i, (layer, a, b)) in tracks.iter().enumerate() {
            for q in [*a, *b] {
                ends += 1;
                let on_pad = pads.iter().any(|(l, path)| {
                    (l == layer || l == "Vias")
                        && path.bounding_box().inflate(0.01, 0.01).contains(q)
                        && path.winding(q) != 0
                });
                let on_track = tracks.iter().enumerate().any(|(j, (l, c, d))| {
                    j != i && l == layer && (c.distance(q) < 1e-3 || d.distance(q) < 1e-3)
                });
                if !on_pad && !on_track {
                    dangling += 1;
                    file_dangling += 1;
                    if file_dangling <= 3 {
                        let near = pads
                            .iter()
                            .filter(|(l, _)| l == layer || l == "Vias")
                            .map(|(_, p)| (p.bounding_box().center().distance(q), p.bounding_box()))
                            .min_by(|a, b| a.0.total_cmp(&b.0));
                        println!("  {layer} ({:.4}, {:.4}) nearest {near:?}", q.x, -q.y);
                    }
                }
            }
        }
        if file_dangling > 0 {
            println!("{}: {file_dangling} dangling track ends", f.display());
        }
    }
    println!("{dangling} of {ends} track ends land on no pad, via or track");
}
