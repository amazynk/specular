//! The DXF corpus (tools/corpus/fetch-dxf.sh) against ezdxf's reading of it
//! (tests/data/oracle.tsv, written by gen/ezdxf_oracle.py). Ignored by default: the corpus
//! is not in the tree.
//!
//! cargo test -p specular-cad --release --test corpus -- --ignored --nocapture

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use kurbo::Rect;
use specular_cad::{Limits, parse};
use specular_vector::Shape;

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_DXF_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/dxf")
        });
    if dir.is_dir() {
        Some(dir)
    } else {
        eprintln!(
            "no DXF corpus at {} (run tools/corpus/fetch-dxf.sh)",
            dir.display()
        );
        None
    }
}

struct Row {
    file: String,
    status: String,
    counts: BTreeMap<String, u64>,
    extents: Option<Rect>,
}

fn oracle() -> Vec<Row> {
    let tsv = include_str!("data/oracle.tsv");
    tsv.lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let counts = f[3]
                .split(',')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.to_string(), v.parse().unwrap()))
                .collect();
            let e: Vec<f64> = f[4].split(' ').filter_map(|v| v.parse().ok()).collect();
            Row {
                file: f[0].to_string(),
                status: f[1].to_string(),
                counts,
                extents: (e.len() == 4).then(|| Rect::new(e[0], e[1], e[2], e[3])),
            }
        })
        .collect()
}

/// How far apart two extents are, relative to the oracle's size: the largest edge offset
/// over the larger side.
fn extents_error(ours: Rect, theirs: Rect) -> f64 {
    let size = theirs.width().max(theirs.height()).max(1e-9);
    [
        ours.x0 - theirs.x0,
        ours.y0 - theirs.y0,
        ours.x1 - theirs.x1,
        ours.y1 - theirs.y1,
    ]
    .iter()
    .map(|d| d.abs())
    .fold(0.0, f64::max)
        / size
}

/// The model's extents without text, whose size depends on fonts: what the oracle measures.
fn geometry_extents(sheet: &specular_vector::Sheet) -> Option<Rect> {
    sheet
        .items
        .iter()
        .filter_map(|it| match &it.shape {
            Shape::Stroke { path, .. } | Shape::Fill { path, .. } => {
                Some(kurbo::Shape::bounding_box(path))
            }
            Shape::Point(p) => Some(Rect::from_points(*p, *p)),
            Shape::Text(_) => None,
        })
        .reduce(|a, b| a.union(b))
}

/// Entity types whose counts are not compared.
fn comparable(ty: &str) -> bool {
    !ty.is_empty()
}

/// Extents tolerance, relative to the drawing's size: both sides measure exact geometry
/// (text left out), so only flattening and rounding separate them.
const EXTENTS_TOLERANCE: f64 = 0.001;

/// Agreement with the file's stated extents that is accepted instead.
const STATED_TOLERANCE: f64 = 0.01;

/// Files where ezdxf's extents differ from ours for a known reason.
const KNOWN_EXTENTS: &[(&str, &str)] = &[
    (
        "lwpolyline_flip_test.dxf",
        "the extrusion is written in the AcDbEntity subclass, which ezdxf ignores",
    ),
    (
        "transform_hatch_pattern.dxf",
        "pattern hatches whose lines all miss their boundary draw nothing; ezdxf counts the boundary",
    ),
    ("ocs2wcs1.dxf", "as transform_hatch_pattern.dxf"),
    ("ocs2wcs2.dxf", "as transform_hatch_pattern.dxf"),
    (
        "block-hidden-entities.dxf",
        "invisible entities (60=1) in a block stay hidden; ezdxf's block expansion keeps them",
    ),
    (
        "closed_polyline_with_bulge.dxf",
        "a 0.5-wide polyline: ours covers its width and mitred corners, ezdxf the centre line",
    ),
    (
        "circle_radius_le_0.dxf",
        "circles of radius 0 or less are invalid and not drawn; ezdxf draws |r|",
    ),
];

#[test]
#[ignore]
fn corpus_oracle() {
    let Some(dir) = corpus() else { return };
    let rows = oracle();
    assert!(rows.len() > 200, "oracle.tsv has {} rows", rows.len());
    let (mut files, mut count_bad, mut ext_bad, mut ext_checked) = (0, 0, 0, 0);
    let (mut ext_stated, mut ext_known) = (0, 0);
    let mut failures = Vec::new();
    // ORACLE lines start their own line (the harness's "test ... " has none).
    println!();
    for r in &rows {
        let path = dir.join(&r.file);
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("missing {}", r.file);
            println!("ORACLE\tDXF\t{}\tskip", r.file);
            continue;
        };
        files += 1;
        let before = failures.len();
        let t = Instant::now();
        let scene = parse(&bytes, &Limits::default());
        let ms = t.elapsed().as_secs_f64() * 1e3;
        // ezdxf could not even split the file into entities: nothing to compare.
        let unreadable = r.status.starts_with("error") && r.counts.is_empty();
        let scene = match scene {
            Ok(s) => s,
            Err(e) => {
                if unreadable {
                    println!("ORACLE\tDXF\t{}\tskip", r.file);
                } else {
                    failures.push(format!("{}: {e} (ezdxf: {})", r.file, r.status));
                    println!("ORACLE\tDXF\t{}\tfail", r.file);
                }
                continue;
            }
        };
        if unreadable {
            // Nothing to compare: ezdxf could not read it.
            println!("ORACLE\tDXF\t{}\tskip", r.file);
            continue;
        }
        let ours: BTreeMap<String, u64> = scene
            .counts
            .iter()
            .filter(|(k, _)| comparable(k))
            .cloned()
            .collect();
        let theirs: BTreeMap<String, u64> = r
            .counts
            .iter()
            .filter(|(k, _)| comparable(k))
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        if ours != theirs {
            count_bad += 1;
            let mut diff = Vec::new();
            for k in ours.keys().chain(theirs.keys()) {
                let (a, b) = (ours.get(k), theirs.get(k));
                if a != b
                    && !diff
                        .iter()
                        .any(|d: &String| d.starts_with(&format!("{k}:")))
                {
                    diff.push(format!("{k}: ours {a:?} ezdxf {b:?}"));
                }
            }
            failures.push(format!("{}: counts differ: {}", r.file, diff.join(", ")));
        }
        // Infinite lines are clipped to the drawing, so they set its extents.
        let infinite = r.counts.contains_key("XLINE") || r.counts.contains_key("RAY");
        if let (Some(theirs), Some(model), false) = (r.extents, scene.sheets.first(), infinite) {
            ext_checked += 1;
            match geometry_extents(model) {
                Some(ours) => {
                    let err = extents_error(ours, theirs);
                    // The file's own $EXTMIN/$EXTMAX, written by the CAD program, is a
                    // second opinion where ezdxf's bounding box differs.
                    let stated = model
                        .file_extents
                        .is_some_and(|f| extents_error(ours, f) <= STATED_TOLERANCE);
                    if stated && err > EXTENTS_TOLERANCE {
                        ext_stated += 1;
                    }
                    let known = KNOWN_EXTENTS.iter().any(|k| r.file.ends_with(k.0));
                    if known && err > EXTENTS_TOLERANCE {
                        ext_known += 1;
                    }
                    if err > EXTENTS_TOLERANCE && !stated && !known {
                        ext_bad += 1;
                        failures.push(format!(
                            "{}: extents off by {:.1}%: ours {ours:?} ezdxf {theirs:?}",
                            r.file,
                            err * 100.0
                        ));
                    }
                }
                None if KNOWN_EXTENTS.iter().any(|k| r.file.ends_with(k.0)) => ext_known += 1,
                None => {
                    ext_bad += 1;
                    failures.push(format!("{}: no extents, ezdxf {theirs:?}", r.file));
                }
            }
        }
        if ms > 200.0 {
            eprintln!("{}: {ms:.0} ms", r.file);
        }
        // One line per file for tools/ci/oracle-pass-rates.sh.
        let status = if failures.len() > before {
            "fail"
        } else {
            "pass"
        };
        println!("ORACLE\tDXF\t{}\t{status}", r.file);
    }
    for f in &failures {
        eprintln!("{f}");
    }
    eprintln!(
        "{files} files; counts differ in {count_bad}; extents checked {ext_checked}, \
         off in {ext_bad}, matching the file's own instead in {ext_stated}, known in {ext_known}"
    );
    assert!(failures.is_empty(), "{} failures", failures.len());
}

/// Every corpus file: parse time, items, warnings.
#[test]
#[ignore]
fn corpus_survey() {
    let Some(dir) = corpus() else { return };
    let mut paths = Vec::new();
    let mut stack = vec![dir.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dxf")) {
                paths.push(p);
            }
        }
    }
    paths.sort();
    for p in paths {
        let bytes = std::fs::read(&p).unwrap();
        let t = Instant::now();
        let r = parse(&bytes, &Limits::default());
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let name = p.strip_prefix(&dir).unwrap().display().to_string();
        match r {
            Ok(s) => eprintln!(
                "{name}\t{ms:.1} ms\t{} sheets\t{} items\t{}",
                s.sheets.len(),
                s.item_count(),
                s.warnings.join("; ")
            ),
            Err(e) => eprintln!("{name}\t{ms:.1} ms\tERROR {e}"),
        }
    }
}

/// The largest file, in release: cargo test --release -- --ignored large_file_timing
#[test]
#[ignore]
fn large_file_timing() {
    let Some(dir) = corpus() else { return };
    let path = dir.join("large/floorplan-tiled.dxf");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("missing {}", path.display());
        return;
    };
    let mut best = f64::INFINITY;
    let mut items = 0;
    let mut entities = 0;
    for _ in 0..5 {
        let t = Instant::now();
        let s = parse(&bytes, &Limits::default()).unwrap();
        best = best.min(t.elapsed().as_secs_f64() * 1e3);
        items = s.item_count();
        entities = s.counts.iter().map(|c| c.1).sum::<u64>();
    }
    eprintln!(
        "{:.1} MB, {entities} entities, {items} items: best of 5 {best:.0} ms",
        bytes.len() as f64 / 1e6
    );
    if !cfg!(debug_assertions) {
        assert!(best < 1000.0, "{best:.0} ms");
    }
}

/// Prints one file's items: SPECULAR_DXF_FILE=path cargo test --test corpus dump -- --ignored
#[test]
#[ignore]
fn dump() {
    let Some(path) = std::env::var_os("SPECULAR_DXF_FILE") else {
        return;
    };
    let bytes = std::fs::read(path).unwrap();
    let s = parse(&bytes, &Limits::default()).unwrap();
    eprintln!("{} {:?} counts {:?}", s.version, s.release, s.counts);
    eprintln!("warnings {:?}", s.warnings);
    for sh in &s.sheets {
        eprintln!(
            "== {} {:?} extents {:?} file {:?} paper {:?} viewports {}",
            sh.name,
            sh.kind,
            sh.extents,
            sh.file_extents,
            sh.paper,
            sh.viewports.len()
        );
        for it in &sh.items {
            let what = match &it.shape {
                specular_vector::Shape::Stroke { .. } => "stroke".to_string(),
                specular_vector::Shape::Fill { .. } => "fill".to_string(),
                specular_vector::Shape::Point(_) => "point".to_string(),
                specular_vector::Shape::Text(t) => format!("text {:?}", t.plain()),
            };
            let b = match &it.shape {
                specular_vector::Shape::Stroke { path, .. }
                | specular_vector::Shape::Fill { path, .. } => {
                    Some(kurbo::Shape::bounding_box(path))
                }
                specular_vector::Shape::Point(p) => Some(Rect::from_points(*p, *p)),
                specular_vector::Shape::Text(t) => Some(Rect::from_points(
                    t.transform * kurbo::Point::ZERO,
                    t.transform * kurbo::Point::ZERO,
                )),
            };
            eprintln!(
                "  {} {:?} {what} {b:?}",
                s.layers[it.layer as usize].name, it.color
            );
        }
    }
}
