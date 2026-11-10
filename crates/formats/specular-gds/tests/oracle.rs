//! specular-gds against the gdstk oracle (tests/oracle.py): cells, top cells, flat shape
//! counts per layer/datatype, bounds, and every expanded polygon.
//!
//! The fixtures' expectations are committed (tests/gen_fixtures.py writes both). The
//! corpus run is ignored by default:
//!
//! ```sh
//! SPECULAR_GDS_CORPUS=tools/corpus/gdsii SPECULAR_GDS_EXPECT=$CARGO_TARGET_DIR/gds-expect \
//!     cargo test -p specular-gds --test oracle -- --ignored --nocapture
//! ```
//! with the expectations made by `oracle.py FILE.gds EXPECT/FILE.expect` per file.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use specular_gds::kurbo::{Point, Rect};
use specular_gds::{Library, Limits, hier, parse};

/// Formats like Python's `%.2f`, without a negative zero.
fn r2(v: f64) -> String {
    let s = format!("{v:.2}");
    if s == "-0.00" { "0.00".into() } else { s }
}

/// The canonical polygon key of oracle.py.
fn canon(pts: &[Point]) -> String {
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for &q in pts {
        if out
            .last()
            .is_none_or(|l| (q.x - l.x).abs() > 1e-9 || (q.y - l.y).abs() > 1e-9)
        {
            out.push(q);
        }
    }
    while out.len() > 1 {
        let (a, b) = (out[0], out[out.len() - 1]);
        if (a.x - b.x).abs() <= 1e-9 && (a.y - b.y).abs() <= 1e-9 {
            out.pop();
        } else {
            break;
        }
    }
    let mut i = 0usize;
    while out.len() >= 3 && i < out.len() {
        let n = out.len();
        let a = out[(i + n - 1) % n];
        let b = out[i];
        let c = out[(i + 1) % n];
        let u = b - a;
        let v = c - b;
        if u.cross(v).abs() <= 1e-9 * u.hypot() * v.hypot() {
            out.remove(i);
            i = i.saturating_sub(1);
        } else {
            i += 1;
        }
    }
    let mut area = 0.0;
    for k in 0..out.len() {
        let p0 = out[(k + out.len() - 1) % out.len()];
        let p1 = out[k];
        area += p0.x * p1.y - p1.x * p0.y;
    }
    if area < 0.0 {
        out.reverse();
    }
    let s: Vec<(String, String)> = out.iter().map(|p| (r2(p.x), r2(p.y))).collect();
    if s.is_empty() {
        return String::new();
    }
    let num = |t: &(String, String)| (t.0.parse::<f64>().unwrap(), t.1.parse::<f64>().unwrap());
    let mut k = 0;
    for j in 1..s.len() {
        let (a, b) = (num(&s[j]), num(&s[k]));
        if a.0 < b.0 || (a.0 == b.0 && a.1 < b.1) {
            k = j;
        }
    }
    let mut keyed: Vec<String> = Vec::with_capacity(s.len());
    for j in 0..s.len() {
        let (x, y) = &s[(k + j) % s.len()];
        keyed.push(format!("{x},{y}"));
    }
    keyed.join(";")
}

#[derive(Default, Debug)]
struct Expect {
    cells: BTreeSet<String>,
    tops: BTreeSet<String>,
    counts: BTreeMap<(String, u16, u16), u64>,
    bbox: BTreeMap<String, [f64; 4]>,
    /// Polygons by (top, layer, datatype), when the file was expanded.
    polys: Option<BTreeMap<(String, u16, u16), Vec<String>>>,
}

fn read_expect(p: &Path) -> Expect {
    let text = std::fs::read_to_string(p).unwrap();
    let mut e = Expect::default();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "cell" => {
                e.cells.insert(f[1].to_string());
            }
            "top" => {
                e.tops.insert(f[1].to_string());
            }
            "count" => {
                e.counts.insert(
                    (
                        f[1].to_string(),
                        f[2].parse().unwrap(),
                        f[3].parse().unwrap(),
                    ),
                    f[4].parse().unwrap(),
                );
            }
            "bbox" => {
                let v: Vec<f64> = f[2..6].iter().map(|x| x.parse().unwrap()).collect();
                e.bbox.insert(f[1].to_string(), [v[0], v[1], v[2], v[3]]);
            }
            "polys" => {
                e.polys.get_or_insert_default();
            }
            "poly" => {
                e.polys
                    .get_or_insert_default()
                    .entry((
                        f[1].to_string(),
                        f[2].parse().unwrap(),
                        f[3].parse().unwrap(),
                    ))
                    .or_default()
                    .push(f[4].to_string());
            }
            _ => {}
        }
    }
    e
}

#[derive(Default, Debug, Clone, Copy)]
struct Tally {
    files: usize,
    cells: usize,
    tops: usize,
    counts: usize,
    bboxes: usize,
    polygons: usize,
    /// Polygons compared vertex by vertex.
    polygons_compared: usize,
    /// Matched only by their bounds (round path ends, tolerance-dependent in gdstk).
    approx: usize,
    mismatches: usize,
}

impl std::ops::AddAssign for Tally {
    fn add_assign(&mut self, o: Tally) {
        self.files += o.files;
        self.cells += o.cells;
        self.tops += o.tops;
        self.counts += o.counts;
        self.bboxes += o.bboxes;
        self.polygons += o.polygons;
        self.polygons_compared += o.polygons_compared;
        self.approx += o.approx;
        self.mismatches += o.mismatches;
    }
}

fn key_bounds(k: &str) -> Option<Rect> {
    specular_gds::bounds_of(k.split(';').filter_map(|p| {
        let (x, y) = p.split_once(',')?;
        Some(Point::new(x.parse().ok()?, y.parse().ok()?))
    }))
}

/// Layers whose polygons gdstk gets wrong: it scales absolute (negative) path widths by
/// the magnification of the references above them. Their counts and bounds are still
/// compared where no magnification applies; the polygons are checked in the unit tests.
fn absolute_width_layers(lib: &Library) -> BTreeSet<(u16, u16)> {
    let magnified = lib
        .cells
        .iter()
        .flat_map(|c| &c.refs)
        .any(|r| r.strans.mag != 1.0);
    if !magnified {
        return BTreeSet::new();
    }
    lib.cells
        .iter()
        .flat_map(|c| &c.shapes)
        .filter(|s| matches!(s.kind, specular_gds::ShapeKind::Path { width, .. } if width < 0))
        .map(|s| lib.layers[s.layer as usize])
        .collect()
}

/// Compares a library with an expectation; prints what differs.
fn compare(name: &str, lib: &Library, e: &Expect, max_flat: usize) -> Tally {
    let mut t = Tally {
        files: 1,
        ..Tally::default()
    };
    let miss = |what: String, t: &mut Tally| {
        if t.mismatches < 20 {
            eprintln!("{name}: {what}");
        }
        t.mismatches += 1;
    };
    let cells: BTreeSet<String> = lib.cells.iter().map(|c| c.name.clone()).collect();
    t.cells = cells.len();
    for c in cells.symmetric_difference(&e.cells) {
        miss(format!("cell {c} only on one side"), &mut t);
    }
    let tops: BTreeSet<String> = lib
        .top
        .iter()
        .map(|&i| lib.cells[i as usize].name.clone())
        .collect();
    t.tops = tops.len();
    for c in tops.symmetric_difference(&e.tops) {
        miss(format!("top cell {c} only on one side"), &mut t);
    }
    let hulls = hier::hulls(lib);
    let flat = hier::flat_counts(lib);
    let skip = absolute_width_layers(lib);
    let total: u64 = lib.top.iter().map(|&i| flat[i as usize]).sum();
    for &top in &lib.top {
        let tn = lib.cells[top as usize].name.clone();
        if !e.tops.contains(&tn) {
            continue;
        }
        // Counts per layer.
        let mine = hier::flat_layer_counts(lib, top);
        let mut mine_map: BTreeMap<(u16, u16), u64> = BTreeMap::new();
        for (i, &n) in mine.iter().enumerate() {
            if n > 0 {
                mine_map.insert(lib.layers[i], n);
            }
        }
        let theirs: BTreeMap<(u16, u16), u64> = e
            .counts
            .iter()
            .filter(|(k, _)| k.0 == tn)
            .map(|(k, &v)| ((k.1, k.2), v))
            .collect();
        let keys: BTreeSet<(u16, u16)> = mine_map.keys().chain(theirs.keys()).copied().collect();
        for k in keys {
            t.counts += 1;
            let (a, b) = (mine_map.get(&k), theirs.get(&k));
            if a != b {
                miss(
                    format!("{tn} layer {}/{}: {a:?} shapes, gdstk {b:?}", k.0, k.1),
                    &mut t,
                );
            }
        }
        // Bounds.
        let mb = hier::hull_bounds(&hulls[top as usize]);
        match (mb, e.bbox.get(&tn)) {
            _ if !skip.is_empty() => {}
            (None, None) => {}
            (Some(m), Some(o)) => {
                t.bboxes += 1;
                let size = (o[2] - o[0]).max(o[3] - o[1]).max(1.0);
                let tol = 0.01 + 1e-9 * size;
                let d = [m.x0 - o[0], m.y0 - o[1], m.x1 - o[2], m.y1 - o[3]];
                if d.iter().any(|v| v.abs() > tol) {
                    // Round path ends are polygons of tolerance-dependent detail.
                    let w = (m.width() - (o[2] - o[0]))
                        .abs()
                        .max((m.height() - (o[3] - o[1])).abs());
                    if d.iter().all(|v| v.abs() <= 0.01 * size) && w <= 0.01 * size {
                        t.approx += 1;
                    } else {
                        miss(format!("{tn} bounds {m:?}, gdstk {o:?}"), &mut t);
                    }
                }
            }
            (a, b) => miss(format!("{tn} bounds {a:?}, gdstk {b:?}"), &mut t),
        }
        // Polygons.
        let Some(polys) = &e.polys else {
            continue;
        };
        if total > max_flat as u64 {
            continue;
        }
        let Ok(shapes) = hier::flatten(lib, top, max_flat) else {
            miss(format!("{tn}: flattening hit the cap"), &mut t);
            continue;
        };
        let mut mine: HashMap<(u16, u16), Vec<String>> = HashMap::new();
        for s in shapes {
            mine.entry(lib.layers[s.layer as usize])
                .or_default()
                .push(canon(&s.points));
        }
        let keys: BTreeSet<(u16, u16)> = mine
            .keys()
            .copied()
            .chain(polys.keys().filter(|k| k.0 == tn).map(|k| (k.1, k.2)))
            .collect();
        for k in keys.into_iter().filter(|k| !skip.contains(k)) {
            let mut a = mine.remove(&k).unwrap_or_default();
            let mut b = polys
                .get(&(tn.clone(), k.0, k.1))
                .cloned()
                .unwrap_or_default();
            a.sort();
            b.sort();
            t.polygons += a.len().max(b.len());
            // Exact matches by merging the sorted lists.
            let (mut i, mut j) = (0, 0);
            let mut ra = Vec::new();
            let mut rb = Vec::new();
            while i < a.len() && j < b.len() {
                match a[i].cmp(&b[j]) {
                    std::cmp::Ordering::Equal => {
                        t.polygons_compared += 1;
                        i += 1;
                        j += 1;
                    }
                    std::cmp::Ordering::Less => {
                        ra.push(&a[i]);
                        i += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        rb.push(&b[j]);
                        j += 1;
                    }
                }
            }
            ra.extend(&a[i..]);
            rb.extend(&b[j..]);
            // What is left may differ only in the detail of round ends: match by bounds.
            let mut used = vec![false; rb.len()];
            for pa in ra {
                let ba = key_bounds(pa);
                let found = ba.and_then(|ba| {
                    let tol = 0.5f64.max(0.02 * ba.width().max(ba.height()));
                    (0..rb.len()).find(|&q| {
                        !used[q]
                            && key_bounds(rb[q]).is_some_and(|bb| {
                                (ba.x0 - bb.x0).abs() <= tol
                                    && (ba.y0 - bb.y0).abs() <= tol
                                    && (ba.x1 - bb.x1).abs() <= tol
                                    && (ba.y1 - bb.y1).abs() <= tol
                            })
                    })
                });
                match found {
                    Some(q) => {
                        used[q] = true;
                        t.approx += 1;
                    }
                    None => miss(
                        format!("{tn} {}/{}: polygon {pa} not in gdstk", k.0, k.1),
                        &mut t,
                    ),
                }
            }
            for (q, pb) in rb.iter().enumerate() {
                if !used[q] {
                    miss(
                        format!("{tn} {}/{}: gdstk polygon {pb} missing", k.0, k.1),
                        &mut t,
                    );
                }
            }
        }
    }
    t
}

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

#[test]
fn fixtures_match_gdstk() {
    let mut total = Tally::default();
    for name in ["basic", "arrays", "paths", "texts", "raw", "deep"] {
        let bytes = std::fs::read(data().join(format!("{name}.gds"))).unwrap();
        let lib = parse(&bytes, Limits::default()).unwrap();
        let e = read_expect(&data().join(format!("{name}.expect")));
        let t = compare(name, &lib, &e, 300_000);
        eprintln!("{name}: {t:?}");
        assert_eq!(t.mismatches, 0, "{name}: {t:?}");
        total += t;
    }
    eprintln!("fixtures: {total:?}");
    // Every polygon of every fixture was compared, the round-ended ones by bounds.
    assert!(total.polygons_compared > 1000, "{total:?}");
    assert!(total.approx < total.polygons / 10, "{total:?}");
}

#[test]
fn broken_files_are_refused_clearly() {
    let read = |n: &str| std::fs::read(data().join(n)).unwrap();
    let e = parse(&read("truncated.gds"), Limits::default()).unwrap_err();
    assert!(matches!(e, specular_gds::GdsError::Truncated(_)), "{e}");
    let e = parse(&read("odd.gds"), Limits::default()).unwrap_err();
    assert!(matches!(e, specular_gds::GdsError::Malformed(_)), "{e}");
    let e = parse(&read("tiny.oas"), Limits::default()).unwrap_err();
    assert!(matches!(e, specular_gds::GdsError::Oasis), "{e}");
    assert!(e.to_string().contains("OASIS"));
    let lib = parse(&read("cycle.gds"), Limits::default()).unwrap();
    assert_eq!(lib.top.len(), 2, "{:?}", lib.warnings);
    assert_eq!(
        lib.warnings.iter().filter(|w| w.contains("cycle")).count(),
        2
    );
    assert!(lib.warnings.iter().any(|w| w.contains("MISSING")));
    assert!(hier::flatten(&lib, lib.top[0], 1000).is_ok());
}

#[test]
fn absolute_magnification_and_rotation() {
    // gdstk ignores the absolute flags; KLayout and the GDSII spec apply them: LEAF keeps
    // its size and orientation although MID is placed doubled and turned.
    let bytes = std::fs::read(data().join("absolute.gds")).unwrap();
    let lib = parse(&bytes, Limits::default()).unwrap();
    let top = lib.cell_index("TOP").unwrap();
    let flat = hier::flatten(&lib, top, 10).unwrap();
    let bounds = |layer: u16| {
        let s = flat
            .iter()
            .find(|s| lib.layers[s.layer as usize].0 == layer)
            .unwrap();
        let b = specular_gds::bounds_of(s.points.iter().copied()).unwrap();
        (b.x0, b.y0, b.x1, b.y1)
    };
    assert_eq!(bounds(1), (1000.0, 0.0, 1100.0, 10.0));
    // AW's path is 10 wide whatever the magnification (a negative WIDTH); its spine is
    // magnified threefold.
    let (x0, y0, x1, y1) = bounds(2);
    assert!((x0, x1) == (0.0, 300.0) && (y0 - 495.0).abs() < 1e-9 && (y1 - 505.0).abs() < 1e-9);
}

#[test]
#[ignore = "needs the corpus (tools/corpus/fetch-gdsii.sh) and its oracle output"]
fn corpus_matches_gdstk() {
    // Starts the `ORACLE` lines on their own line (libtest prints `test NAME ... ` first).
    println!();
    let corpus = PathBuf::from(std::env::var("SPECULAR_GDS_CORPUS").expect("SPECULAR_GDS_CORPUS"));
    let expect = PathBuf::from(std::env::var("SPECULAR_GDS_EXPECT").expect("SPECULAR_GDS_EXPECT"));
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![corpus.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("gds")) {
                files.push(p);
            }
        }
    }
    files.sort();
    if let Ok(only) = std::env::var("SPECULAR_GDS_ONLY") {
        files.retain(|f| f.to_string_lossy().contains(&only));
    }
    let mut total = Tally::default();
    let (mut refused, mut skipped) = (0, 0);
    for f in &files {
        let rel = f.strip_prefix(&corpus).unwrap();
        let exp = expect.join(rel).with_extension("expect");
        let bytes = std::fs::read(f).unwrap();
        // One line per file for tools/ci/oracle-pass-rates.sh.
        let verdict = |status: &str| println!("ORACLE\tGDS\t{}\t{status}", rel.display());
        let lib = match parse(&bytes, Limits::default()) {
            Ok(l) => l,
            Err(e) => {
                if exp.exists() {
                    eprintln!("{}: refused but gdstk read it: {e}", rel.display());
                    total.mismatches += 1;
                    verdict("fail");
                } else {
                    refused += 1;
                    // Refused by both: agreement.
                    verdict("pass");
                }
                continue;
            }
        };
        if !exp.exists() {
            // gdstk could not read it.
            skipped += 1;
            verdict("skip");
            continue;
        }
        let uses_abs = lib
            .cells
            .iter()
            .flat_map(|c| &c.refs)
            .any(|r| r.strans.abs_mag || r.strans.abs_angle);
        if uses_abs {
            skipped += 1;
            verdict("skip");
            continue;
        }
        let e = read_expect(&exp);
        let t = compare(&rel.display().to_string(), &lib, &e, 300_000);
        verdict(if t.mismatches > 0 { "fail" } else { "pass" });
        total += t;
    }
    eprintln!(
        "corpus: {} files, {refused} refused by both, {skipped} not compared: {total:?}",
        files.len()
    );
    assert_eq!(total.mismatches, 0, "{total:?}");
}
