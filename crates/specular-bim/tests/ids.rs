//! IDS checks against the buildingSMART implementers' test cases (fetched by
//! `tools/corpus/fetch-ids.sh`; skipped when absent): each `pass-*` or `fail-*` pair of an
//! `.ids` and an `.ifc` must pass or fail; `invalid-*` IDS files must be refused or fail.
//! When `tools/corpus/ids-oracle.py` has written `ifctester.tsv`, IfcOpenShell's ifctester
//! is the second oracle: the cases where it and Specular disagree are listed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use specular_bim::idscheck::{check, parse};
use specular_mesh::Limits;
use specular_mesh::ifc::data;

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_IDS_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/ids"),
        PathBuf::from,
    )
}

/// "pass", "fail" or "error" (the IDS or the model was refused).
fn run(ids: &Path, ifc: &Path) -> &'static str {
    let Ok(text) = std::fs::read_to_string(ids) else {
        return "error";
    };
    let Ok(spec) = parse(&text) else {
        return "error";
    };
    let Ok(bytes) = std::fs::read(ifc) else {
        return "error";
    };
    let Ok(model) = data::read(&bytes, &Limits::default()) else {
        return "error";
    };
    if check(&spec, &model).ok() {
        "pass"
    } else {
        "fail"
    }
}

#[test]
fn ids_test_cases() {
    let root = corpus();
    let Ok(dirs) = std::fs::read_dir(&root) else {
        eprintln!("skipped: no IDS corpus at {}", root.display());
        return;
    };
    let oracle: HashMap<String, String> = std::fs::read_to_string(root.join("ifctester.tsv"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut it = l.split('\t');
            let k = it.next()?.to_owned();
            it.next()?;
            Some((k, it.next()?.to_owned()))
        })
        .collect();
    let mut dirs: Vec<PathBuf> = dirs
        .flatten()
        .map(|d| d.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let (mut n, mut right, mut agree, mut compared) = (0, 0, 0, 0);
    let mut wrong = Vec::new();
    let mut differ = Vec::new();
    let t0 = std::time::Instant::now();
    for d in dirs {
        let facet = d.file_name().unwrap().to_string_lossy().into_owned();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&d)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "ids"))
            .collect();
        files.sort();
        for ids in files {
            let ifc = ids.with_extension("ifc");
            if !ifc.exists() {
                continue;
            }
            let name = ids.file_stem().unwrap().to_string_lossy().into_owned();
            let want = if name.starts_with("pass-") {
                "pass"
            } else {
                "fail"
            };
            let got = run(&ids, &ifc);
            n += 1;
            let ok = got == want || (want == "fail" && got == "error");
            if ok {
                right += 1;
            } else {
                wrong.push(format!("{facet}/{name}: want {want}, got {got}"));
            }
            let key = format!("{facet}/{name}");
            if let Some(o) = oracle.get(&key) {
                compared += 1;
                let same = o == got || (o != "pass" && got != "pass");
                if same {
                    agree += 1;
                } else {
                    differ.push(format!("{key}: ifctester {o}, Specular {got}"));
                }
            }
        }
    }
    for w in &wrong {
        eprintln!("wrong: {w}");
    }
    for d in &differ {
        eprintln!("differs from ifctester: {d}");
    }
    eprintln!(
        "IDS test cases: {right} of {n} right; agrees with ifctester on {agree} of {compared}; {:.0} ms",
        t0.elapsed().as_secs_f64() * 1e3
    );
    assert!(n > 300, "only {n} cases found");
    assert!(wrong.is_empty(), "{} cases wrong", wrong.len());
}
