//! The IFC checks on real buildings (fetched by `tools/corpus/fetch-ifc-checks.sh`; skipped
//! when absent): the model information read against IfcOpenShell (the `*.oracle.json`
//! files `ifc-data-oracle.py` writes), a model compared with itself (no changes), and
//! timings of reading, clash detection, comparison and quantity take-off.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::Value as J;
use specular_bim::{clash, diff, qto};
use specular_mesh::Limits;
use specular_mesh::ifc::data::{self, IfcData, Object};

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_IFC_CHECKS_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/ifc-checks"),
        PathBuf::from,
    )
}

fn same_text(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0),
        _ => false,
    }
}

/// Mismatches by what: class, name, predefined, type, container, sets, properties.
fn compare(
    d: &IfcData,
    oracle: &serde_json::Map<String, J>,
) -> (usize, HashMap<&'static str, usize>, Vec<String>) {
    let mut bad: HashMap<&'static str, usize> = HashMap::new();
    let mut shown = Vec::new();
    let mut props = 0;
    let mut note = |k: &'static str, msg: String, bad: &mut HashMap<&'static str, usize>| {
        let n = bad.entry(k).or_insert(0);
        *n += 1;
        if *n <= 3 {
            shown.push(format!("{k}: {msg}"));
        }
    };
    for (g, o) in oracle {
        let Some(ours) = d.by_guid(g) else {
            note("missing", g.clone(), &mut bad);
            continue;
        };
        let s = |k: &str| o.get(k).and_then(J::as_str).unwrap_or("");
        if ours.class != s("class") {
            note(
                "class",
                format!("{g} {} vs {}", ours.class, s("class")),
                &mut bad,
            );
        }
        if ours.name().unwrap_or_default() != s("name") {
            note("name", g.clone(), &mut bad);
        }
        let pd = d.predefined_type(ours).unwrap_or_default();
        if pd != s("predefined") {
            note(
                "predefined",
                format!("{g} {} {pd:?} vs {:?}", ours.class, s("predefined")),
                &mut bad,
            );
        }
        let guid_of = |id: Option<u32>| {
            id.and_then(|i| d.get(i))
                .map(|x: &Object| x.global_id.clone())
                .unwrap_or_default()
        };
        if guid_of(ours.type_object) != s("type") {
            note("type", g.clone(), &mut bad);
        }
        let container = d
            .container(ours)
            .map(|c| c.global_id.clone())
            .unwrap_or_default();
        if container != s("container") {
            let class = |g: &str| d.by_guid(g).map(|x| x.class.clone()).unwrap_or_default();
            note(
                "container",
                format!(
                    "{g} {}: {} vs {}",
                    ours.class,
                    class(&container),
                    class(s("container"))
                ),
                &mut bad,
            );
        }
        let ps = d.psets(ours);
        for (set, values) in o.get("psets").and_then(J::as_object).into_iter().flatten() {
            let Some(mine) = ps.iter().find(|p| &p.name == set) else {
                note("set", format!("{g} {set}"), &mut bad);
                continue;
            };
            for (name, v) in values.as_object().into_iter().flatten() {
                props += 1;
                let want = v.as_str().unwrap_or("");
                match mine.get(name) {
                    None => note("property", format!("{g} {set}.{name} missing"), &mut bad),
                    Some(p) if !same_text(&p.text(), want) => note(
                        "property",
                        format!("{g} {set}.{name} {:?} vs {want:?}", p.text()),
                        &mut bad,
                    ),
                    _ => {}
                }
            }
        }
    }
    (props, bad, shown)
}

#[test]
fn ifc_checks_corpus() {
    let root = corpus();
    let Ok(dir) = std::fs::read_dir(&root) else {
        eprintln!("skipped: no corpus at {}", root.display());
        return;
    };
    let mut files: Vec<PathBuf> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ifc" || x == "ifczip"))
        .collect();
    files.sort();
    let (mut objects, mut props, mut mismatches) = (0, 0, 0);
    for f in files {
        let bytes = std::fs::read(&f).unwrap();
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        let t0 = Instant::now();
        let d = data::read(&bytes, &Limits::default()).unwrap();
        let t_data = t0.elapsed().as_secs_f64();
        if let Ok(o) = std::fs::read(f.with_extension("oracle.json")) {
            let o: serde_json::Map<String, J> = serde_json::from_slice(&o).unwrap();
            let (n, bad, shown) = compare(&d, &o);
            objects += o.len();
            props += n;
            let total: usize = bad.values().sum();
            mismatches += total;
            eprintln!(
                "{name}: {} objects, {n} properties against IfcOpenShell: {total} mismatches {bad:?}",
                o.len()
            );
            for s in shown {
                eprintln!("  {s}");
            }
        }
        let t1 = Instant::now();
        let m = specular_mesh::ifc::read(&bytes, &Limits::default(), None).unwrap();
        let t_mesh = t1.elapsed().as_secs_f64();
        let mut elements = m.elements.clone();
        elements.sort_by_key(|e| e.first_vertex);
        let model = clash::Model {
            positions: &m.mesh.positions,
            indices: &m.mesh.indices,
            elements: &elements,
        };
        let all: Vec<usize> = (0..elements.len()).collect();
        let t2 = Instant::now();
        let tol = 0.01 / m.mesh_metres;
        let hard = clash::run(
            &model,
            &all,
            &all,
            clash::Mode::Hard { tolerance: tol },
            &|_, _| false,
        );
        let t_clash = t2.elapsed().as_secs_f64();
        let side = diff::Side {
            data: &d,
            elements: &elements,
            mesh: &m.mesh,
            mesh_metres: m.mesh_metres,
            origin_metres: m.origin_metres,
        };
        let t3 = Instant::now();
        let changes = diff::diff(&side, &side);
        let t_diff = t3.elapsed().as_secs_f64();
        assert!(
            changes.is_empty(),
            "{name}: {} changes against itself: {:?}",
            changes.len(),
            changes.first()
        );
        let t4 = Instant::now();
        let rows = qto::rows(&d, &elements, &m.mesh, m.mesh_metres);
        let t_qto = t4.elapsed().as_secs_f64();
        // For reading back with another spreadsheet reader (openpyxl).
        if let Some(dir) = std::env::var_os("SPECULAR_DUMP_XLSX") {
            let l = qto::Labels {
                columns: std::array::from_fn(|i| format!("c{i}")),
                from_file: "file".into(),
                computed: "computed".into(),
                mixed: "mixed".into(),
            };
            let same = |k: &str| k.to_owned();
            let sheets = vec![
                ("Elements".to_owned(), qto::element_table(&rows, &l, &same)),
                (
                    "By class".to_owned(),
                    qto::group_table(&qto::groups(&rows, qto::By::Class), "Class", &l, &same),
                ),
            ];
            let out = Path::new(&dir).join(format!("{name}.xlsx"));
            std::fs::write(&out, specular_bim::export::xlsx(&sheets)).unwrap();
        }
        let from_file = rows
            .iter()
            .filter(|r| {
                r.volume.is_some_and(|q| q.from_file) || r.area.is_some_and(|q| q.from_file)
            })
            .count();
        eprintln!(
            "{name}: {} elements, {} triangles; data {:.2} s, mesh {:.2} s, hard clashes {} in {:.2} s, self-diff {:.2} s, quantities {:.2} s ({from_file} with file quantities)",
            elements.len(),
            m.mesh.triangle_count(),
            t_data,
            t_mesh,
            hard.len(),
            t_clash,
            t_diff,
            t_qto
        );
    }
    eprintln!(
        "model information: {objects} objects, {props} properties, {mismatches} mismatches against IfcOpenShell"
    );
    assert_eq!(mismatches, 0);
}
