//! Scientific TIFF through the registry against tifffile: ImageJ hyperstacks and calibrated
//! 16-bit, plain float and multi-band TIFF, Zeiss LSM, MetaMorph STK, Molecular Dynamics GEL,
//! and Zeiss / FEI / TESCAN / Hitachi SEM images. The fixtures and their oracle files are made
//! by `specular-tiff/tests/data/scitiff/make.py`; every check counts mismatches and the test
//! prints the counts before failing on any.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use specular_core::{CancelToken, Source};
use specular_decode::science::{AxisKind, PixelSource, ScienceKind, ScienceSummary};
use specular_decode::{DecodeRequest, Registry, Session};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-tiff/tests/data/scitiff")
}

fn open(name: &str) -> Arc<dyn Session> {
    let src = Source::open(dir().join(name)).unwrap();
    Registry::default()
        .open(Arc::new(src))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn oracle(name: &str) -> Value {
    let text = std::fs::read_to_string(dir().join(name)).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn metres(unit: &str) -> f64 {
    match unit {
        "nm" => 1e-9,
        "µm" => 1e-6,
        "mm" => 1e-3,
        "m" => 1.0,
        u => panic!("unit {u}"),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1e-300)
}

/// The tifffile letter of one of our axes.
fn letter(a: &specular_decode::science::NdAxis) -> char {
    match (a.kind, a.name.as_str()) {
        (AxisKind::Channel, _) => 'C',
        (_, "Z") => 'Z',
        (_, "Time") => 'T',
        (k, n) => panic!("axis {n:?} {k:?}"),
    }
}

#[derive(Default, Debug)]
struct Tally {
    checks: usize,
    mismatches: Vec<String>,
    values: usize,
    value_mismatches: usize,
    one_ulp: usize,
}

impl Tally {
    fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        self.checks += 1;
        if !ok {
            self.mismatches.push(what());
        }
    }
}

fn pixel_check(t: &mut Tally, name: &str, sc: &ScienceSummary, want: &Value) {
    match want.as_array() {
        None => t.check(sc.pixel.is_none(), || {
            format!("{name}: pixel {:?}", sc.pixel)
        }),
        Some(w) => {
            let Some(p) = &sc.pixel else {
                t.check(false, || format!("{name}: no pixel size"));
                return;
            };
            let got = (p.x.step * metres(&p.unit_x), p.y.step * metres(&p.unit_y));
            let want = (w[0].as_f64().unwrap(), w[1].as_f64().unwrap());
            t.check(close(got.0, want.0) && close(got.1, want.1), || {
                format!("{name}: pixel {got:?} != {want:?}")
            });
        }
    }
}

fn science(name: &str, json: &str, t: &mut Tally) {
    let s = open(name);
    let o = oracle(json);
    let Some(sc) = s.science() else {
        t.check(false, || format!("{name}: no science summary"));
        return;
    };
    let shape: Vec<u32> = o["shape"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u32)
        .collect();
    let axes = o["axes"].as_str().unwrap();
    let dim = |c: char| axes.find(c).map(|i| shape[i]);
    t.check(
        (Some(sc.width), Some(sc.height)) == (dim('X'), dim('Y')),
        || format!("{name}: size {}×{}", sc.width, sc.height),
    );
    t.check(!sc.raster_only, || format!("{name}: raster only"));
    pixel_check(t, name, sc, &o["pixel"]);
    // Our axes are tifffile's beside Y and X (S as C).
    let theirs: String = axes
        .chars()
        .map(|c| if c == 'S' { 'C' } else { c })
        .filter(|c| !"YX".contains(*c))
        .collect();
    let ours: String = sc.axes.iter().map(letter).collect();
    let sorted = |s: &str| {
        let mut v: Vec<char> = s.chars().collect();
        v.sort_unstable();
        v
    };
    t.check(sorted(&ours) == sorted(&theirs), || {
        format!("{name}: axes {ours} != {theirs}")
    });
    for a in &sc.axes {
        let l = letter(a);
        let want_len = axes
            .find(if l == 'C' && axes.contains('S') {
                'S'
            } else {
                l
            })
            .map(|i| shape[i]);
        t.check(Some(a.len) == want_len, || {
            format!("{name}: axis {l} len {} != {want_len:?}", a.len)
        });
        let step = match l {
            'Z' => o.get("z_step"),
            'T' => o.get("t_step"),
            _ => None,
        }
        .and_then(Value::as_f64);
        if let Some(want) = step {
            let got = a.linear.map(|l| l.step);
            t.check(got.is_some_and(|g| close(g, want)), || {
                format!("{name}: axis {l} step {got:?} != {want}")
            });
            let unit = if l == 'T' { "s" } else { "µm" };
            t.check(a.unit == unit, || {
                format!("{name}: axis {l} unit {:?}", a.unit)
            });
        }
        if l == 'C'
            && let Some(want) = o.get("labels").and_then(Value::as_array)
            && want.len() as u32 == a.len
        {
            let want: Vec<&str> = want.iter().map(|v| v.as_str().unwrap()).collect();
            t.check(a.labels == want, || {
                format!("{name}: labels {:?}", a.labels)
            });
        }
    }
    if o.get("channel_axis").is_some() {
        t.check(
            sc.channel_axis
                .is_some_and(|c| sc.axes[c].kind == AxisKind::Channel),
            || format!("{name}: channel axis {:?}", sc.channel_axis),
        );
    }
    if let Some(r) = o.get("ranges").and_then(Value::as_array) {
        let want: Vec<(f64, f64)> = r
            .iter()
            .map(|p| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap()))
            .collect();
        t.check(sc.display_ranges == want, || {
            format!("{name}: ranges {:?} != {want:?}", sc.display_ranges)
        });
    }
    if let Some(l) = o.get("luts").and_then(Value::as_array) {
        let want: Vec<Vec<[u8; 3]>> = l
            .iter()
            .map(|t| {
                t.as_array()
                    .unwrap()
                    .iter()
                    .map(|e| [0, 1, 2].map(|k| e[k].as_u64().unwrap() as u8))
                    .collect()
            })
            .collect();
        let got: Vec<Vec<[u8; 3]>> = sc.luts.iter().map(|t| t.to_vec()).collect();
        t.check(got == want, || format!("{name}: {} LUTs differ", got.len()));
    }
    if let Some(c) = o.get("colours").and_then(Value::as_array) {
        let want: Vec<[f32; 3]> = c
            .iter()
            .map(|e| [0, 1, 2].map(|k| e[k].as_u64().unwrap() as f32 / 255.0))
            .collect();
        t.check(sc.channel_colours == want, || {
            format!("{name}: colours {:?}", sc.channel_colours)
        });
    }
    if let Some(u) = o.get("vunit").and_then(Value::as_str) {
        t.check(sc.unit.as_deref() == Some(u), || {
            format!("{name}: unit {:?}", sc.unit)
        });
    }
    if let Some(u) = o.get("title").and_then(Value::as_str) {
        t.check(sc.title.as_deref() == Some(u), || {
            format!("{name}: title {:?}", sc.title)
        });
    }
    if let Some(v) = o.get("voltage").and_then(Value::as_f64) {
        t.check(sc.voltage.is_some_and(|g| close(g, v)), || {
            format!("{name}: voltage {:?}", sc.voltage)
        });
    }
    // Every plane.
    let cancel = CancelToken::new();
    for p in o["planes"].as_array().unwrap() {
        let key = p["index"].as_object().unwrap();
        let index: Vec<u32> = sc
            .axes
            .iter()
            .map(|a| {
                key.get(&letter(a).to_string())
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32
            })
            .collect();
        let Some(Ok(plane)) = s.nd_plane(&index, &cancel) else {
            t.check(false, || format!("{name}: plane {index:?} unreadable"));
            continue;
        };
        let want = p["values"].as_array().unwrap();
        t.values += want.len();
        // Off by one unit in the last place counts apart: tifffile scales GEL values in
        // single precision, we in double.
        let ulps = |g: f32, w: f32| (g.to_bits() as i64 - w.to_bits() as i64).unsigned_abs();
        let mut ulp = 0;
        let bad = plane
            .values
            .iter()
            .zip(want)
            .filter(|(g, w)| match w.as_f64() {
                None => !g.is_nan(),
                Some(w) => match ulps(**g, w as f32) {
                    0 => false,
                    1 => {
                        ulp += 1;
                        false
                    }
                    _ => true,
                },
            })
            .count()
            + plane.values.len().abs_diff(want.len());
        t.value_mismatches += bad;
        t.one_ulp += ulp;
        t.check(bad == 0, || {
            format!("{name}: plane {index:?}: {bad} values differ")
        });
    }
    // The default view decodes.
    let r = s.decode(&DecodeRequest::full(1), &cancel);
    t.check(
        r.as_ref()
            .is_ok_and(|r| (r.width, r.height) == (sc.width, sc.height)),
        || format!("{name}: default view {:?}", r.err()),
    );
}

fn picture(name: &str, json: &str, t: &mut Tally) {
    let s = open(name);
    let o = oracle(json);
    let Some(sc) = s.science() else {
        t.check(false, || format!("{name}: no facts"));
        return;
    };
    t.check(sc.raster_only, || format!("{name}: not a picture"));
    pixel_check(t, name, sc, &o["pixel"]);
    t.check(sc.pixel_source == Some(PixelSource::Vendor), || {
        format!("{name}: pixel source {:?}", sc.pixel_source)
    });
    t.check(sc.kind == ScienceKind::ElectronMicroscopy, || {
        format!("{name}: kind {:?}", sc.kind)
    });
    if let Some(v) = o.get("voltage").and_then(Value::as_f64) {
        t.check(sc.voltage.is_some_and(|g| close(g, v)), || {
            format!("{name}: voltage {:?}", sc.voltage)
        });
    }
    let r = s.decode(&DecodeRequest::full(1), &CancelToken::new());
    t.check(r.is_ok_and(|r| (r.width, r.height) == (20, 12)), || {
        format!("{name}: decode")
    });
}

#[test]
fn scitiff_against_tifffile() {
    let mut t = Tally::default();
    for (file, json) in [
        ("hyperstack.tif", "hyperstack.json"),
        ("signed.tif", "signed.json"),
        ("float.tif", "float.json"),
        ("multiband.tif", "multiband.json"),
        ("stack.lsm", "stack.json"),
        ("stack.stk", "stack-stk.json"),
        ("fluoview.tif", "fluoview.json"),
        ("scan.gel", "scan.json"),
        ("fei.tif", "fei.json"),
        ("banded.tif", "banded.json"),
        ("composite-old.tif", "composite-old.json"),
    ] {
        science(file, json, &mut t);
    }
    for (file, json) in [
        ("zeiss.tif", "zeiss.json"),
        ("tescan.tif", "tescan.json"),
        ("hitachi.tif", "hitachi.json"),
    ] {
        picture(file, json, &mut t);
    }
    eprintln!(
        "scitiff oracle: {} checks, {} mismatches; {} values, {} mismatches, {} off by 1 ulp",
        t.checks,
        t.mismatches.len(),
        t.values,
        t.value_mismatches,
        t.one_ulp
    );
    assert!(t.mismatches.is_empty(), "{:#?}", t.mismatches);
}

#[test]
fn scitiff_kinds() {
    let kind = |f: &str| open(f).science().map(|s| s.kind);
    assert_eq!(kind("hyperstack.tif"), Some(ScienceKind::Microscopy));
    assert_eq!(kind("stack.lsm"), Some(ScienceKind::Microscopy));
    assert_eq!(kind("scan.gel"), Some(ScienceKind::Gridded));
    assert_eq!(kind("fei.tif"), Some(ScienceKind::ElectronMicroscopy));
    // A LSM is one image: its thumbnails are no pages.
    assert_eq!(open("stack.lsm").page_count(), 1);
    assert_eq!(open("hyperstack.tif").page_count(), 1);
}

/// The corpus directory (`tools/corpus/fetch-scitiff.sh`), if fetched.
fn corpus() -> Option<PathBuf> {
    let d = std::env::var_os("SPECULAR_SCITIFF_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/scitiff")
        });
    d.join("expected").is_dir().then_some(d)
}

/// One corpus file against tifffile: the pixel size, and for a science reading the axes and
/// every plane's count, NaNs, sum, extremes and every 97th value.
fn corpus_file(root: &Path, json: &Path, t: &mut Tally) {
    let o: Value = serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
    let name = json.file_stem().unwrap().to_string_lossy().into_owned();
    let src = Source::open(root.join("dl").join(&name)).unwrap();
    let s = match Registry::default().open(Arc::new(src)) {
        Ok(s) => s,
        Err(e) => return t.check(false, || format!("{name}: {e}")),
    };
    let Some(sc) = s.science() else {
        return t.check(false, || format!("{name}: no science summary"));
    };
    if !o["pixel"].is_null() {
        let unit = |u: &str| match u {
            "pm" => 1e-12,
            "nm" => 1e-9,
            "µm" => 1e-6,
            "mm" => 1e-3,
            "m" => 1.0,
            _ => f64::NAN,
        };
        let w = o["pixel"].as_array().unwrap();
        let got = sc
            .pixel
            .as_ref()
            .map(|p| (p.x.step * unit(&p.unit_x), p.y.step * unit(&p.unit_y)));
        let want = (w[0].as_f64().unwrap(), w[1].as_f64().unwrap());
        // tifffile reads some sizes as float32.
        let near = |a: f64, b: f64| (a - b).abs() <= 1e-6 * b.abs();
        t.check(
            got.is_some_and(|g| near(g.0, want.0) && near(g.1, want.1)),
            || format!("{name}: pixel {got:?} != {want:?}"),
        );
    }
    if sc.raster_only {
        // A picture with instrument facts: tifffile's planes are its samples.
        t.check(o["dtype"] == "uint8", || format!("{name}: raster only"));
        return;
    }
    let axes = o["axes"].as_str().unwrap();
    let theirs: String = axes
        .chars()
        .map(|c| if c == 'S' { 'C' } else { c })
        .filter(|c| !"YX".contains(*c))
        .collect();
    let ours: String = sc.axes.iter().map(letter).collect();
    let sorted = |s: &str| {
        let mut v: Vec<char> = s.chars().collect();
        v.sort_unstable();
        v
    };
    t.check(sorted(&ours) == sorted(&theirs), || {
        format!("{name}: axes {ours} != {theirs}")
    });
    if let Some(r) = o.get("ranges").and_then(Value::as_array) {
        let want: Vec<(f64, f64)> = r
            .iter()
            .map(|p| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap()))
            .collect();
        t.check(sc.display_ranges == want, || {
            format!("{name}: ranges {:?} != {want:?}", sc.display_ranges)
        });
    }
    let cancel = CancelToken::new();
    for p in o["planes"].as_array().unwrap() {
        let key = p["index"].as_object().unwrap();
        let index: Vec<u32> = sc
            .axes
            .iter()
            .map(|a| {
                key.get(&letter(a).to_string())
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32
            })
            .collect();
        let Some(Ok(plane)) = s.nd_plane(&index, &cancel) else {
            t.check(false, || format!("{name}: plane {index:?} unreadable"));
            continue;
        };
        let v = &plane.values;
        let finite: Vec<f64> = v
            .iter()
            .filter(|x| !x.is_nan())
            .map(|&x| f64::from(x))
            .collect();
        let sum: f64 = finite.iter().sum();
        let min = finite.iter().copied().fold(f64::INFINITY, f64::min);
        let max = finite.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let num = |k: &str| p[k].as_f64();
        t.check(v.len() as u64 == p["count"].as_u64().unwrap(), || {
            format!("{name}: plane {index:?} has {} values", v.len())
        });
        t.check(
            (v.len() - finite.len()) as u64 == p["nan"].as_u64().unwrap(),
            || format!("{name}: plane {index:?} NaNs"),
        );
        let want_sum = num("sum").unwrap();
        t.check(
            (sum - want_sum).abs() <= 1e-9 * want_sum.abs().max(1.0),
            || format!("{name}: plane {index:?} sum {sum} != {want_sum}"),
        );
        t.check(
            num("min").is_none_or(|w| w == min) && num("max").is_none_or(|w| w == max),
            || format!("{name}: plane {index:?} range {min}..{max}"),
        );
        let sample = p["sample"].as_array().unwrap();
        t.values += sample.len();
        let bad = sample
            .iter()
            .zip(v.iter().step_by(97))
            .filter(|(w, g)| match w.as_f64() {
                None => !g.is_nan(),
                Some(w) => w as f32 != **g,
            })
            .count()
            + sample.len().abs_diff(v.len().div_ceil(97));
        t.value_mismatches += bad;
        t.check(bad == 0, || {
            format!("{name}: plane {index:?}: {bad} values differ")
        });
    }
}

/// The fetched corpus (`tools/corpus/fetch-scitiff.sh`) against tifffile; skips without it.
#[test]
fn scitiff_corpus_against_tifffile() {
    let Some(root) = corpus() else {
        eprintln!("scitiff corpus absent: skipped");
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("expected"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    let mut t = Tally::default();
    for f in &files {
        corpus_file(&root, f, &mut t);
    }
    // ImageJ's own LUT files (NIH ICOL): grays is the identity, fire ends in white.
    for (name, first, last) in [
        ("grays.lut", [0, 0, 0], [255; 3]),
        ("fire.lut", [0, 0, 31], [255; 3]),
    ] {
        let Ok(b) = std::fs::read(root.join("dl").join(name)) else {
            continue;
        };
        let lut = specular_decode::science::read_lut(&b);
        t.check(
            lut.as_ref().is_some_and(|l| {
                l[0] == first
                    && l[255] == last
                    && (name != "grays.lut"
                        || l.iter().enumerate().all(|(i, e)| *e == [i as u8; 3]))
            }),
            || format!("{name}: {:?}", lut.map(|l| (l[0], l[128], l[255]))),
        );
    }
    eprintln!(
        "scitiff corpus: {} files, {} checks, {} mismatches; {} sampled values, {} mismatches",
        files.len(),
        t.checks,
        t.mismatches.len(),
        t.values,
        t.value_mismatches
    );
    assert!(t.mismatches.is_empty(), "{:#?}", t.mismatches);
}
