//! Compares TIA series and Bruker BCF images and spectra with RosettaSciIO
//! (`gen/oracle_matsci.py`, `oracle_matsci.json` beside the files) on the committed fixtures
//! (`tests/data/matsci`) and on the corpus (`$SPECULAR_MATSCI_CORPUS/rsciio`, fetched by
//! `tools/corpus/fetch-matsci.sh`; skipped when absent): image planes (sums and pixels),
//! lateral and energy calibration, total-count maps and whole spectra at a few pixels.
//!
//! Deliberate differences: RosettaSciIO multiplies TIA steps by 1e9 ("nm", reciprocal-space
//! steps included) but keeps their offsets in metres, so steps are compared scaled and offsets
//! raw; we keep metres and 1/m. A BCF hypermap's x step is DX (RosettaSciIO uses DY for both
//! axes), so only its y step is compared; the BCF overview image is not given the map's
//! calibration.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;
use specular_em::dataset::{Dataset, Lateral, Page};
use specular_em::{bcf, tia};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/matsci")
}

fn corpus() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("SPECULAR_MATSCI_CORPUS")?).join("rsciio");
    dir.join("oracle_matsci.json").is_file().then_some(dir)
}

#[derive(Default)]
struct Tally {
    files: BTreeMap<&'static str, (usize, usize)>,
    failures: Vec<String>,
    open: Duration,
    slowest_at: Duration,
}

fn close(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-6 * a.abs().max(b.abs()) + 1e-9
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

fn usize_of(v: &Value) -> usize {
    v.as_u64().unwrap_or(0) as usize
}

/// Compares a calibration with an oracle axis (TIA steps in nm, reciprocal ones too).
fn axis(ours: Option<&Lateral>, a: &Value, what: &str, errs: &mut Vec<String>) {
    let units = a["units"].as_str().unwrap_or("");
    let (off, scale) = (f(&a["offset"]), f(&a["scale"]));
    match ours {
        None if matches!(units, "pix" | "None" | "") => {}
        None => errs.push(format!("{what}: no calibration, oracle {scale} {units}")),
        Some(l) => {
            let k = match l.unit.as_str() {
                "m" | "1/m" if units == "nm" => 1e9,
                _ => 1.0,
            };
            // RosettaSciIO rescales a TIA step to nm but leaves its offset in metres.
            if !close(l.offset, off) || !close(l.step * k, scale) {
                errs.push(format!(
                    "{what}: {} + {} {} vs {off} + {scale} {units}",
                    l.offset, l.step, l.unit
                ));
            }
        }
    }
}

fn image(page: &Page, file: &[u8], k: usize, sig: &Value, errs: &mut Vec<String>) {
    let shape: Vec<usize> = sig["shape"]
        .as_array()
        .unwrap()
        .iter()
        .map(usize_of)
        .collect();
    let (h, w) = (shape[shape.len() - 2], shape[shape.len() - 1]);
    if (page.width, page.height) != (w, h) {
        errs.push(format!("size {}x{} vs {w}x{h}", page.width, page.height));
        return;
    }
    for p in sig["planes"].as_array().unwrap() {
        let i = k + usize_of(&p["index"]);
        let Some((v, _)) = page.read_plane(file, i) else {
            errs.push(format!("plane {i} missing"));
            continue;
        };
        let sum: f64 = v
            .iter()
            .filter(|x| !x.is_nan())
            .map(|&x| f64::from(x))
            .sum();
        if !close(sum, f(&p["sum"])) {
            errs.push(format!("plane {i} sum {sum} vs {}", p["sum"]));
        }
        for px in p["pixels"].as_array().unwrap() {
            let (x, y) = (usize_of(&px[0]), usize_of(&px[1]));
            let ours = f64::from(v[y * w + x]);
            if !close(ours, f(&px[2])) {
                errs.push(format!("plane {i} ({x}, {y}) {ours} vs {}", px[2]));
            }
        }
    }
}

fn spectrum(page: &Page, file: &[u8], sig: &Value, t: &mut Tally, errs: &mut Vec<String>) {
    let Some(sp) = page.spectra.as_ref() else {
        errs.push("no spectra".into());
        return;
    };
    let shape: Vec<usize> = sig["shape"]
        .as_array()
        .unwrap()
        .iter()
        .map(usize_of)
        .collect();
    let channels = shape[shape.len() - 1];
    let n: usize = shape[..shape.len() - 1].iter().product();
    let width = usize_of(&sig["width"]);
    if sp.grid() != (width, n / width) || sp.channels() != channels {
        errs.push(format!(
            "grid {:?} x {} vs {width} x {} x {channels}",
            sp.grid(),
            sp.channels(),
            n / width
        ));
        return;
    }
    let ax = sig["axes"].as_array().unwrap();
    let e = &ax[ax.len() - 1];
    let ours = sp.axis();
    if !close(ours.offset, f(&e["offset"])) || !close(ours.step, f(&e["scale"])) {
        errs.push(format!("energy {ours:?} vs {e}"));
    }
    let totals = page.read_plane(file, 0).map(|p| p.0).unwrap_or_default();
    for (i, want) in sig["totals"].as_array().unwrap().iter().enumerate() {
        let got = totals.get(i).map_or(f64::NAN, |&v| f64::from(v));
        if !close(got, f(want)) {
            errs.push(format!("total at {i}: {got} vs {want}"));
            break;
        }
    }
    for s in sig["spectra"].as_array().unwrap() {
        let (x, y) = (usize_of(&s["x"]), usize_of(&s["y"]));
        let start = Instant::now();
        let got = sp.at(file, x, y);
        t.slowest_at = t.slowest_at.max(start.elapsed());
        let Some(got) = got else {
            errs.push(format!("no spectrum at ({x}, {y})"));
            continue;
        };
        let want = s["values"].as_array().unwrap();
        if let Some(c) = (0..channels).find(|&c| !close(f64::from(got[c]), f(&want[c]))) {
            errs.push(format!(
                "spectrum ({x}, {y}) channel {c}: {} vs {}",
                got[c], want[c]
            ));
        }
    }
}

fn check_tia(path: &Path, sigs: &Value, t: &mut Tally) -> Vec<String> {
    let mut errs = Vec::new();
    let ser = std::fs::read(path).unwrap();
    let (emi, k) = match tia::sidecar(path) {
        Some((p, k)) => (std::fs::read(p).ok(), k),
        None => (None, 0),
    };
    let start = Instant::now();
    let d = match tia::read_indexed(&ser, emi.as_deref(), k) {
        Ok(d) => d,
        Err(e) => return vec![format!("{e}")],
    };
    t.open += start.elapsed();
    let sig = &sigs[0];
    let page = &d.pages[0];
    // Some objects of an .emi are only a UUID.
    let described = emi.as_deref().is_some_and(|e| {
        tia::emi_objects(e)
            .get(k)
            .is_some_and(|o| o.windows(8).any(|w| w == b"<Microsc"))
    });
    if described && (page.voltage.is_none() || page.date.is_none() || page.mode.is_none()) {
        errs.push(format!(
            "emi facts missing: {:?} {:?} {:?} {:?}",
            page.voltage, page.date, page.mode, page.instrument
        ));
    }
    let ax = sig["axes"].as_array().unwrap();
    if sig["kind"] == "image" {
        image(page, &ser, 0, sig, &mut errs);
        axis(page.x.as_ref(), &ax[ax.len() - 1], "x", &mut errs);
        axis(page.y.as_ref(), &ax[ax.len() - 2], "y", &mut errs);
    } else {
        spectrum(page, &ser, sig, t, &mut errs);
        match ax.len() {
            3 => {
                axis(page.x.as_ref(), &ax[1], "x", &mut errs);
                axis(page.y.as_ref(), &ax[0], "y", &mut errs);
            }
            2 => axis(page.x.as_ref(), &ax[0], "x", &mut errs),
            _ => {}
        }
    }
    errs
}

fn check_bcf(path: &Path, sigs: &Value, t: &mut Tally) -> Vec<String> {
    let file = std::fs::read(path).unwrap();
    let start = Instant::now();
    let d: Dataset = match bcf::read(&file) {
        Ok(d) => d,
        Err(e) => {
            return if sigs.get("error").is_some() {
                Vec::new()
            } else {
                vec![format!("{e}")]
            };
        }
    };
    t.open += start.elapsed();
    let mut errs = Vec::new();
    let Some(sigs) = sigs.as_array() else {
        return vec!["opened a file the oracle rejects".into()];
    };
    // Image planes in order across the image pages, then the hypermaps.
    let planes: Vec<(&Page, usize)> = d
        .pages
        .iter()
        .filter(|p| p.spectra.is_none())
        .flat_map(|p| (0..p.planes.len()).map(move |k| (p, k)))
        .collect();
    let maps: Vec<&Page> = d.pages.iter().filter(|p| p.spectra.is_some()).collect();
    let images: Vec<&Value> = sigs.iter().filter(|s| s["kind"] == "image").collect();
    let spectra: Vec<&Value> = sigs.iter().filter(|s| s["kind"] == "spectrum").collect();
    if planes.len() != images.len() || maps.len() != spectra.len() {
        errs.push(format!(
            "{} planes, {} maps vs {} images, {} maps",
            planes.len(),
            maps.len(),
            images.len(),
            spectra.len()
        ));
    }
    for ((page, k), sig) in planes.iter().zip(&images) {
        image(page, &file, *k, sig, &mut errs);
        if page.planes[*k].label != sig["title"].as_str().unwrap_or("") {
            errs.push(format!(
                "label {:?} vs {}",
                page.planes[*k].label, sig["title"]
            ));
        }
        if !page.title.as_deref().unwrap_or("").ends_with("(Overview)") {
            let ax = sig["axes"].as_array().unwrap();
            axis(page.x.as_ref(), &ax[1], "image x", &mut errs);
            axis(page.y.as_ref(), &ax[0], "image y", &mut errs);
        }
    }
    for (page, sig) in maps.iter().zip(&spectra) {
        spectrum(page, &file, sig, t, &mut errs);
        axis(page.y.as_ref(), &sig["axes"][0], "map y", &mut errs);
    }
    errs
}

fn run(dir: &Path, t: &mut Tally) {
    let text = std::fs::read_to_string(dir.join("oracle_matsci.json")).unwrap();
    let oracle: BTreeMap<String, Value> = serde_json::from_str(&text).unwrap();
    for (rel, sigs) in &oracle {
        let path = dir.join(rel);
        let (group, errs) = if rel.ends_with(".bcf") {
            ("bcf", check_bcf(&path, sigs, t))
        } else if sigs.get("error").is_some() {
            continue;
        } else {
            ("tia", check_tia(&path, sigs, t))
        };
        let e = t.files.entry(group).or_default();
        e.0 += 1;
        if !errs.is_empty() {
            e.1 += 1;
            for m in errs {
                t.failures.push(format!("{rel}: {m}"));
            }
        }
    }
}

fn report(what: &str, t: &Tally) {
    for (g, (n, bad)) in &t.files {
        eprintln!("{what} {g}: {n} files, {bad} with mismatches");
    }
    eprintln!(
        "{what}: open {:?} in all, slowest spectrum {:?}",
        t.open, t.slowest_at
    );
    for f in &t.failures {
        eprintln!("  {f}");
    }
}

#[test]
fn fixtures_match_oracle() {
    let mut t = Tally::default();
    run(&fixtures(), &mut t);
    report("fixtures", &t);
    assert!(t.files.values().map(|v| v.0).sum::<usize>() >= 4);
    assert!(t.failures.is_empty());
}

#[test]
fn corpus_matches_oracle() {
    let Some(dir) = corpus() else {
        eprintln!("SPECULAR_MATSCI_CORPUS not set or not fetched; skipped");
        return;
    };
    let mut t = Tally::default();
    run(&dir, &mut t);
    report("corpus", &t);
    assert!(t.failures.is_empty());
}
