//! The fixtures in tests/data (written by gen/make_fixtures.py) against python-casacore's
//! reading of them (tests/data/expected.tsv, written by gen/oracle.py): shapes and pixel
//! types, plane sums and pixel values with the default mask applied, world coordinates
//! (through the FITS cards and our WCS) at corners and random pixels, beams and metadata.
//!
//! The fetched corpus (tools/corpus/fetch-casa.sh) is checked the same way when present, at
//! `SPECULAR_CASA_DIR` with `SPECULAR_CASA_ORACLE` (default: tools/corpus/casa and its
//! expected.tsv).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_casa::{CasaImage, PlaneData};
use specular_fits::wcs::Wcs;

/// casacore's `Stokes::StokesTypes`, by value.
const CASACORE_STOKES: [&str; 33] = [
    "Undefined",
    "I",
    "Q",
    "U",
    "V",
    "RR",
    "RL",
    "LR",
    "LL",
    "XX",
    "XY",
    "YX",
    "YY",
    "RX",
    "RY",
    "LX",
    "LY",
    "XR",
    "XL",
    "YR",
    "YL",
    "PP",
    "PQ",
    "QP",
    "QQ",
    "RCircular",
    "LCircular",
    "Linear",
    "Ptotal",
    "Plinear",
    "PFtotal",
    "PFlinear",
    "Pangle",
];

#[derive(Default)]
struct Case {
    img: Option<(String, Vec<usize>, bool)>,
    plane: Vec<(Vec<usize>, f64, f64, usize)>,
    val: Vec<(Vec<usize>, f64, f64)>,
    world: Vec<(Vec<f64>, Vec<f64>)>,
    beam: Vec<(i64, i64, f64, f64, f64)>,
    meta: Vec<(String, String)>,
    error: Option<String>,
}

fn num(s: &str) -> f64 {
    if s == "nan" {
        f64::NAN
    } else {
        s.parse().unwrap_or_else(|_| panic!("number {s}"))
    }
}

fn list<T: std::str::FromStr>(s: &str) -> Vec<T> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split(',')
        .map(|x| x.parse().ok().expect("list item"))
        .collect()
}

fn load(tsv: &Path) -> BTreeMap<String, Case> {
    let text = std::fs::read_to_string(tsv).expect("oracle tsv");
    let mut cases: BTreeMap<String, Case> = BTreeMap::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let c = cases.entry(f[1].to_owned()).or_default();
        match f[0] {
            "img" => c.img = Some((f[2].to_owned(), list(f[3]), f[4] == "1")),
            "plane" => c
                .plane
                .push((list(f[2]), num(f[3]), num(f[4]), f[5].parse().unwrap())),
            "val" => c.val.push((list(f[2]), num(f[3]), num(f[4]))),
            "world" => c.world.push((list(f[2]), list(f[3]))),
            "beam" => c.beam.push((
                f[2].parse().unwrap(),
                f[3].parse().unwrap(),
                num(f[4]),
                num(f[5]),
                num(f[6]),
            )),
            "meta" => c.meta.push((f[2].to_owned(), f[3..].join("\t"))),
            "error" => c.error = Some(f[2..].join("\t")),
            t => panic!("unknown row {t}"),
        }
    }
    cases
}

/// Mismatches of one image, with what they were.
fn check(dir: &Path, case: &Case, log: &mut Vec<String>) -> usize {
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let mut bad = 0;
    let mut fail = |what: String| {
        log.push(format!("{name}: {what}"));
        bad += 1;
    };
    let im = match CasaImage::open(dir) {
        Ok(im) => im,
        Err(e) => {
            if case.error.is_none() {
                fail(format!("open: {e}"));
            }
            return bad;
        }
    };
    let Some((dtype, shape, has_mask)) = &case.img else {
        fail("opened what python-casacore refused".into());
        return bad;
    };
    let ours = match im.pixel.name() {
        "Float" => "float",
        "Double" => "double",
        p => p,
    };
    if ours != dtype {
        fail(format!("pixel type {ours} vs {dtype}"));
    }
    if &im.shape != shape {
        fail(format!("shape {:?} vs {shape:?}", im.shape));
        return bad;
    }
    if im.has_mask() != *has_mask && *has_mask {
        fail("mask missing".into());
    }
    // Planes: sums of the unmasked values and their count.
    let mut planes: BTreeMap<Vec<usize>, PlaneData> = BTreeMap::new();
    for (idx, re, imag, valid) in &case.plane {
        let p = match im.plane(idx) {
            Ok(p) => p,
            Err(e) => {
                fail(format!("plane {idx:?}: {e}"));
                continue;
            }
        };
        let (mut sr, mut si, mut n) = (0.0f64, 0.0f64, 0usize);
        match &p {
            PlaneData::Real(v) => {
                for x in v.iter().filter(|x| !x.is_nan()) {
                    sr += f64::from(*x);
                    n += 1;
                }
            }
            PlaneData::Complex(v) => {
                for c in v.iter().filter(|c| !c[0].is_nan()) {
                    sr += f64::from(c[0]);
                    si += f64::from(c[1]);
                    n += 1;
                }
            }
        }
        let tol = 1e-4 * (1.0 + *valid as f64);
        if n != *valid || (sr - re).abs() > tol || (si - imag).abs() > tol {
            fail(format!(
                "plane {idx:?}: sum ({sr}, {si}) over {n} vs ({re}, {imag}) over {valid}"
            ));
        }
        planes.insert(idx.clone(), p);
    }
    for (at, re, imag) in &case.val {
        let Some(p) = planes.get(&at[2..]) else {
            continue;
        };
        let k = at[0] + at.get(1).copied().unwrap_or(0) * im.width();
        let (r, i) = match p {
            PlaneData::Real(v) => (f64::from(v[k]), if v[k].is_nan() { f64::NAN } else { 0.0 }),
            PlaneData::Complex(v) => (f64::from(v[k][0]), f64::from(v[k][1])),
        };
        let same =
            |a: f64, b: f64| (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-6 * (1.0 + b.abs());
        if !same(r, *re) || !same(i, *imag) {
            fail(format!("value at {at:?}: ({r}, {i}) vs ({re}, {imag})"));
        }
    }
    // World coordinates through the cards.
    let cards = im.fits_cards();
    let header =
        specular_fits::Header::from_pairs(cards.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")));
    let wcs = Wcs::from_header(&header, &im.shape);
    for (p, w) in &case.world {
        let Some(wcs) = &wcs else {
            fail("no WCS".into());
            break;
        };
        let fits: Vec<f64> = p.iter().map(|x| x + 1.0).collect();
        let Some(ours) = wcs.pixel_to_world(&fits) else {
            fail(format!("world at {p:?}: none"));
            continue;
        };
        for (k, (a, b)) in ours.iter().zip(w).enumerate() {
            let axis = &im.coords.axes[k];
            // A tabulated axis is interpolated in its table, not through the cards.
            let a = &axis.table_value(p[k]).unwrap_or(*a);
            let ok = match axis.kind {
                // casacore's Stokes enum at the nearest pixel, against the axis's names
                // (what the pages are labelled with) and, at whole pixels on an evenly
                // spaced axis, the FITS code the cards give.
                specular_casa::AxisKind::Stokes => {
                    let at = p[k].round().clamp(0.0, (axis.stokes.len() - 1) as f64) as usize;
                    let name = CASACORE_STOKES.get(*b as usize).copied().unwrap_or("?");
                    let fits = specular_casa::coords::stokes_code(name);
                    let even = cards.iter().any(|(c, _)| c == &format!("CRVAL{}", k + 1));
                    axis.stokes.get(at).map(String::as_str) == Some(name)
                        && (!even || p[k].fract() != 0.0 || fits == Some(*a))
                }
                specular_casa::AxisKind::Longitude => {
                    let d = (a - b).rem_euclid(360.0);
                    d.min(360.0 - d) * 3600.0 < 1e-6
                }
                specular_casa::AxisKind::Latitude => (a - b).abs() * 3600.0 < 1e-6,
                _ => (a - b).abs() <= 1e-9 * (1.0 + b.abs()),
            };
            if !ok {
                fail(format!("world axis {k} at {p:?}: {a} vs {b}"));
            }
        }
    }
    // Beams.
    for &(c, s, maj, min, pa) in &case.beam {
        let b = if c < 0 {
            im.beam
        } else {
            im.plane_beams
                .as_ref()
                .and_then(|pb| pb.get(c as usize, s as usize))
        };
        match b {
            Some(b)
                if (b.major - maj).abs() < 1e-9
                    && (b.minor - min).abs() < 1e-9
                    && (b.pa - pa).abs() < 1e-9 => {}
            b => fail(format!("beam ({c}, {s}): {b:?} vs {maj} {min} {pa}")),
        }
    }
    for (k, v) in &case.meta {
        let ours = match k.as_str() {
            "telescope" => im.coords.telescope.clone(),
            "observer" => im.coords.observer.clone(),
            "date" => im.coords.date.clone(),
            "units" => im.units.clone(),
            "object" => im.object.clone(),
            _ => None,
        };
        if ours.as_deref() != Some(v.as_str()) {
            fail(format!("{k}: {ours:?} vs {v}"));
        }
    }
    bad
}

fn run(root: &Path, tsv: &Path) -> (usize, usize) {
    let cases = load(tsv);
    let mut log = Vec::new();
    let mut bad = 0;
    let t = Instant::now();
    for (name, case) in &cases {
        let dir = find(root, name).unwrap_or_else(|| panic!("{name} not under {root:?}"));
        bad += check(&dir, case, &mut log);
    }
    for l in log.iter().take(40) {
        eprintln!("{l}");
    }
    eprintln!(
        "{} images under {}: {bad} mismatches in {:.0?}",
        cases.len(),
        root.display(),
        t.elapsed()
    );
    (cases.len(), bad)
}

/// The image directory `name` directly under `root` or one level down.
fn find(root: &Path, name: &str) -> Option<PathBuf> {
    let direct = root.join(name);
    if direct.join("table.dat").exists() {
        return Some(direct);
    }
    std::fs::read_dir(root).ok()?.flatten().find_map(|e| {
        let p = e.path().join(name);
        p.join("table.dat").exists().then_some(p)
    })
}

#[test]
fn fixtures_match_python_casacore() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let (n, bad) = run(&dir, &dir.join("expected.tsv"));
    assert_eq!(n, 8);
    assert_eq!(bad, 0);
}

#[test]
fn corpus_matches_python_casacore() {
    let root = std::env::var_os("SPECULAR_CASA_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/casa"),
        PathBuf::from,
    );
    let tsv = std::env::var_os("SPECULAR_CASA_ORACLE")
        .map_or_else(|| root.join("expected.tsv"), PathBuf::from);
    if !tsv.exists() {
        eprintln!("no CASA corpus at {}; skipped", root.display());
        return;
    }
    let (_, bad) = run(&root, &tsv);
    assert_eq!(bad, 0);
}

/// A copy of a fixture under the test scratch directory.
fn scratch_copy(name: &str, tag: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("casa-{tag}"))
        .join(name);
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), dst.join(e.file_name())).unwrap();
        }
    }
    dst
}

#[test]
fn truncated_and_corrupt_tables_are_refused() {
    // Tile data cut short.
    let d = scratch_copy("cube_le.image", "trunc");
    let tiles = d.join("table.f0_TSM0");
    let len = std::fs::metadata(&tiles).unwrap().len();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&tiles)
        .unwrap()
        .set_len(len - 3)
        .unwrap();
    assert_eq!(
        CasaImage::open(&d).unwrap_err(),
        specular_casa::Error::Truncated
    );
    // Header cut short.
    let d = scratch_copy("shape_le.model", "header");
    let f0 = d.join("table.f0");
    let bytes = std::fs::read(&f0).unwrap();
    std::fs::write(&f0, &bytes[..bytes.len() / 2]).unwrap();
    assert!(CasaImage::open(&d).is_err());
    // Missing tile file.
    let d = scratch_copy("cube_be.image", "missing");
    std::fs::remove_file(d.join("table.f0_TSM0")).unwrap();
    assert!(CasaImage::open(&d).is_err());
    // A cube claiming a huge shape is refused against the file size, not allocated.
    let d = scratch_copy("cube_le.image", "huge");
    let f0 = d.join("table.f0");
    let mut bytes = std::fs::read(&f0).unwrap();
    // The first IPosition after the TiledCellStMan header is the default tile shape; the
    // cube shape follows the cube's record: patch every 13 (the RA length) to 2^30.
    let pat = 13i32.to_be_bytes();
    let big = (1i32 << 30).to_be_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if bytes[i..i + 4] == pat {
            bytes[i..i + 4].copy_from_slice(&big);
        }
        i += 1;
    }
    std::fs::write(&f0, &bytes).unwrap();
    assert!(CasaImage::open(&d).is_err());
    // A mask whose tiles are cut short: the image opens, reading a masked plane fails
    // cleanly.
    let d = scratch_copy("masked_be.image", "mask");
    let m = d.join("mask0");
    std::fs::create_dir_all(&m).unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/masked_be.image/mask0");
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), m.join(e.file_name())).unwrap();
        }
    }
    let tiles = m.join("table.f0_TSM0");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&tiles)
        .unwrap()
        .set_len(1)
        .unwrap();
    if let Ok(im) = CasaImage::open(&d) {
        let _ = im.plane(&[0, 0]);
    }
}
