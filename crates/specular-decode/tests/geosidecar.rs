//! Sidecar georeferencing and the gridded raster readers against the oracle: GDAL (through
//! rasterio) and PROJ (through pyproj) read the fixtures that
//! `crates/formats/specular-gis/gen/make_geosidecar_fixtures.py` writes (world files with
//! `.prj`, `.aux.xml`, Esri ASCII grids, DTED, ENVI, ESRI `.hdr`, ERDAS IMAGINE), and every
//! file is opened from disk as the viewer opens it. Compared: the transform, the EPSG code,
//! nodata, the centre pixel's longitude/latitude, every band's values and the wavelengths.
//!
//! The interpreter is `SPECULAR_PYTHON`, else the project's `target/gis-venv`, else a `venv`
//! in the target directory, else `python3`; without rasterio the tests skip.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use serde_json::Value as J;
use specular_core::{CancelToken, Source};
use specular_decode::Registry;
use specular_decode::geo::{GeoFacts, GeoRef};

fn python() -> PathBuf {
    if let Some(p) = std::env::var_os("SPECULAR_PYTHON") {
        return PathBuf::from(p);
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let target = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .map(Path::to_path_buf);
    manifest
        .ancestors()
        .map(|d| d.join("target/gis-venv/bin/python"))
        .chain(target.map(|t| t.join("venv/bin/python")))
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("python3"))
}

/// The fixture directory, generated once per test binary; `None` without the oracle.
fn fixtures() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("fixtures-geosidecar");
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .map(|d| d.join("crates/formats/specular-gis/gen/make_geosidecar_fixtures.py"))
            .find(|p| p.exists())?;
        let stamp = out.join("stamp");
        let fresh = match (std::fs::metadata(&stamp), std::fs::metadata(&script)) {
            (Ok(s), Ok(g)) => s.modified().ok() >= g.modified().ok(),
            _ => false,
        };
        if fresh {
            return Some(out);
        }
        let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("py-tmp");
        let _ = std::fs::create_dir_all(&tmp);
        match Command::new(python())
            .arg("-I")
            .arg(&script)
            .arg(&out)
            .env("TMPDIR", &tmp)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .output()
        {
            Ok(o) if o.status.success() => {
                std::fs::write(&stamp, b"ok").ok()?;
                Some(out)
            }
            Ok(o) => {
                eprintln!(
                    "fixture generation failed, oracle tests skipped:\n{}",
                    String::from_utf8_lossy(&o.stderr)
                );
                None
            }
            Err(e) => {
                eprintln!("no python ({e}), oracle tests skipped");
                None
            }
        }
    })
    .as_deref()
}

/// Mismatches by kind, with the first few described.
#[derive(Default)]
struct Tally {
    compared: [usize; 7],
    mismatched: [usize; 7],
    notes: Vec<String>,
    /// The kinds left out for the file being compared, and how many checks were.
    skipped_kinds: Vec<usize>,
    skipped: usize,
}

const KINDS: [&str; 7] = [
    "transform",
    "EPSG",
    "nodata",
    "lon/lat",
    "values",
    "wavelengths",
    "opens",
];

impl Tally {
    fn check(&mut self, kind: usize, ok: bool, what: impl FnOnce() -> String) {
        if self.skipped_kinds.contains(&kind) {
            self.skipped += 1;
            return;
        }
        self.compared[kind] += 1;
        if !ok {
            self.mismatched[kind] += 1;
            if self.notes.len() < 20 {
                self.notes.push(what());
            }
        }
    }
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * (1.0 + a.abs().max(b.abs()))
}

fn wavelength_nm(v: f64, unit: Option<&str>) -> f64 {
    match unit.map(str::to_ascii_lowercase).as_deref() {
        Some("micrometers" | "micrometres" | "microns" | "um") => v * 1e3,
        Some("millimeters" | "millimetres" | "mm") => v * 1e6,
        _ => v,
    }
}

/// Opens every entry of an oracle manifest from `dir` and compares it with GDAL's reading;
/// `values` files are in `vdir`. `skip` names (file, kind) pairs left out, kind one of
/// [`KINDS`] or `*` for the whole file.
fn compare(dir: &Path, vdir: &Path, expected: &[J], skip: &[(&str, &str)]) -> Tally {
    let reg = Registry::default();
    let cancel = CancelToken::new();
    let mut t = Tally::default();
    let mut values = 0usize;
    let mut files = 0usize;
    let mut slowest: Vec<(f64, String)> = Vec::new();
    for e in expected {
        let name = e["file"].as_str().unwrap();
        if e.get("error").is_some() || skip.iter().any(|(f, k)| *f == name && *k == "*") {
            continue;
        }
        t.skipped_kinds = skip
            .iter()
            .filter(|(f, _)| *f == name)
            .filter_map(|(_, k)| KINDS.iter().position(|x| x == k))
            .collect();
        files += 1;
        let start = Instant::now();
        let s = match reg.open(Arc::new(Source::open(dir.join(name)).unwrap())) {
            Ok(s) => s,
            Err(err) => {
                t.check(6, false, || format!("{name}: does not open ({err:?})"));
                continue;
            }
        };
        let (w, h) = (e["width"].as_u64().unwrap(), e["height"].as_u64().unwrap());
        let size = (u64::from(s.info().width), u64::from(s.info().height));
        t.check(6, size == (w, h), || {
            format!("{name}: {size:?}, GDAL {w}×{h}")
        });
        if size != (w, h) {
            continue;
        }
        let georeferenced = e
            .get("georeferenced")
            .is_none_or(|v| v.as_bool() == Some(true));
        if georeferenced {
            match s.geo() {
                Some(g) => compare_geo(name, e, &g, (w, h), &mut t),
                None => t.check(0, false, || format!("{name}: not georeferenced")),
            }
        }
        // A cube that opens on its colour composite has every band on its next page.
        let s = match s.page_label() {
            Some(specular_decode::PageLabel::Product {
                view: specular_decode::ProductView::Composite,
                ..
            }) => s.open_page(1).unwrap(),
            _ => s,
        };
        // Values, through the science view (only the grids and DEMs have one).
        let (Some(vname), Some(sci)) = (e.get("values").and_then(J::as_str), s.science()) else {
            continue;
        };
        let raw = std::fs::read(vdir.join(vname)).unwrap();
        let want: Vec<f32> = raw
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        let bands = e["bands"].as_u64().unwrap() as usize;
        let ours = sci.axes.first().map_or(1, |a| a.len as usize);
        t.check(6, ours == bands, || {
            format!("{name}: {ours} bands, GDAL {bands}")
        });
        let n = (w * h) as usize;
        for b in 0..bands.min(ours) {
            let index: Vec<u32> = if sci.axes.is_empty() {
                vec![]
            } else {
                vec![b as u32]
            };
            let plane = match s.nd_plane(&index, &cancel) {
                Some(Ok(p)) => p,
                r => {
                    t.check(4, false, || {
                        format!("{name} band {}: no plane ({r:?})", b + 1)
                    });
                    continue;
                }
            };
            let wb = &want[b * n..(b + 1) * n];
            let same =
                |(a, b): (&f32, &f32)| a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan());
            let bad = plane.values.iter().zip(wb).filter(|p| !same(*p)).count()
                + plane.values.len().abs_diff(n);
            values += n;
            t.check(4, bad == 0, || {
                let first = plane.values.iter().zip(wb).position(|p| !same(p));
                let at = first.map(|i| (i % w as usize, i / w as usize, plane.values[i], wb[i]));
                format!(
                    "{name} band {}: {bad} of {n} values differ, first (x, y, got, GDAL) {at:?}",
                    b + 1
                )
            });
        }
        if let Some(wl) = e.get("wavelengths").and_then(J::as_array) {
            let unit = e["wavelength_units"].as_str();
            let labels = sci
                .axes
                .first()
                .map(|a| a.labels.clone())
                .unwrap_or_default();
            let ok = labels.len() == wl.len()
                && labels.iter().zip(wl).all(|(l, v)| {
                    let want = wavelength_nm(v.as_f64().unwrap(), unit);
                    l.strip_suffix(" nm")
                        .and_then(|n| n.parse::<f64>().ok())
                        .is_some_and(|n| (n - want).abs() < 1e-3)
                });
            t.check(5, ok, || {
                format!(
                    "{name}: wavelengths {:?}…, GDAL {:?}…",
                    labels.first(),
                    wl.first()
                )
            });
        }
        slowest.push((start.elapsed().as_secs_f64() * 1e3, name.to_string()));
    }
    slowest.sort_by(|a, b| b.0.total_cmp(&a.0));
    eprintln!(
        "{files} files, {values} values, {} checks skipped",
        t.skipped
    );
    for (k, kind) in KINDS.iter().enumerate() {
        eprintln!(
            "  {kind:12} {} compared, {} mismatches",
            t.compared[k], t.mismatched[k]
        );
    }
    for (ms, name) in slowest.iter().take(3) {
        eprintln!("  open and every band's values: {ms:.0} ms {name}");
    }
    for n in &t.notes {
        eprintln!("  {n}");
    }
    t
}

fn compare_geo(name: &str, e: &J, g: &GeoRef, (w, h): (u64, u64), t: &mut Tally) {
    let want: Vec<f64> = e["transform"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let m = &g.to_map;
    let got = [m.a, m.b, m.c, m.d, m.e, m.f];
    t.check(
        0,
        got.iter().zip(&want).all(|(a, b)| close(*a, *b, 1e-12)),
        || format!("{name}: transform {got:?}, GDAL {want:?}"),
    );
    let epsg = g.crs.as_ref().and_then(|c| c.epsg);
    let want_epsg = e["epsg"].as_u64().map(|v| v as u32);
    if want_epsg.is_some() {
        t.check(1, epsg == want_epsg, || {
            format!("{name}: EPSG {epsg:?}, GDAL {want_epsg:?}")
        });
    }
    if let GeoFacts::Raster(r) = &g.facts {
        let ok = match &e["nodata"] {
            J::String(s) if s == "nan" => r.nodata.is_some_and(f64::is_nan),
            v => r.nodata == v.as_f64(),
        };
        t.check(2, ok, || {
            format!("{name}: nodata {:?}, GDAL {}", r.nodata, e["nodata"])
        });
        if let Some(scale) = e.get("scale").and_then(J::as_f64) {
            let off = e["offset"].as_f64().unwrap();
            assert_eq!(r.scale_offset, Some((scale, off)), "{name}");
            assert_eq!(r.unit.as_deref(), e["unit"].as_str(), "{name}");
            assert_eq!(r.band_name.as_deref(), e["description"].as_str(), "{name}");
        }
        assert!(!r.sidecars.is_empty() || r.keys_kind == "header", "{name}");
    }
    if let Some(ll) = e["centre_lonlat"].as_array() {
        let (x, y) = g.map_at(w as f64 / 2.0, h as f64 / 2.0);
        let got = g.crs.as_ref().and_then(|c| c.to_lonlat(x, y));
        let (lon, lat) = (ll[0].as_f64().unwrap(), ll[1].as_f64().unwrap());
        t.check(
            3,
            got.is_some_and(|(a, b)| (a - lon).abs() < 1e-7 && (b - lat).abs() < 1e-7),
            || format!("{name}: centre {got:?}, PROJ ({lon}, {lat})"),
        );
    }
}

#[test]
fn sidecars_and_grids_match_gdal() {
    let Some(dir) = fixtures() else { return };
    let expected: Vec<J> =
        serde_json::from_slice(&std::fs::read(dir.join("expected.json")).unwrap()).unwrap();
    eprintln!("geosidecar fixtures against GDAL:");
    let t = compare(dir, dir, &expected, &[]);
    assert!(t.compared[4] >= 20, "too few bands compared");
    assert_eq!(t.mismatched.iter().sum::<usize>(), 0, "{:#?}", t.notes);
}

/// Corpus checks GDAL answers differently by design: (file, kind).
const CORPUS_SKIP: &[(&str, &str)] = &[
    // GDAL reports the PNG's tRNS colour as nodata; the PNG decoder already makes those
    // pixels transparent, and a picture has no nodata of its own.
    ("gdal/png/test.png", "nodata"),
    // ESRI's Vertical Near-Side Perspective (ESRI:54049), which is not among the supported
    // projections: the CRS is named, not projected.
    ("gdal/hfa/esri_54049_pe_string_only_broken.img", "lon/lat"),
];

/// The fetched corpus (`tools/corpus/fetch-geosidecar.sh`) against its oracle: GDAL's
/// autotest grids, DTED, ENVI and IMAGINE files, a USGS 3DEP ASCII grid and an EnMAP scene
/// as ENVI. Skips without the corpus.
#[test]
fn corpus_matches_gdal() {
    let dir = std::env::var_os("SPECULAR_GEOSIDECAR_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/geosidecar")
        });
    let Ok(index) = std::fs::read(dir.join("expected/index.json")) else {
        eprintln!("no geosidecar corpus at {}; skipping", dir.display());
        return;
    };
    let expected: Vec<J> = serde_json::from_slice(&index).unwrap();
    eprintln!("geosidecar corpus against GDAL:");
    let t = compare(&dir, &dir.join("expected"), &expected, CORPUS_SKIP);
    assert_eq!(t.mismatched.iter().sum::<usize>(), 0, "{:#?}", t.notes);
}

/// The data file and its header both open the ENVI image; the header alone names it.
#[test]
fn envi_opens_from_either_file() {
    let Some(dir) = fixtures() else { return };
    let reg = Registry::default();
    for name in [
        "bsq.img",
        "bsq.hdr",
        "rotated.dat",
        "rotated.hdr",
        "elev.bil",
        "elev.hdr",
    ] {
        let s = reg
            .open(Arc::new(Source::open(dir.join(name)).unwrap()))
            .unwrap_or_else(|err| panic!("{name}: {err:?}"));
        assert!(s.geo().is_some(), "{name}");
        assert!(s.science().is_some(), "{name}");
    }
    // Listings show the data files and the content-only extensions.
    for name in [
        "bsq.img",
        "rotated.dat",
        "bip.raw",
        "grid.asc",
        "rle.img",
        "tile.dt0",
    ] {
        assert!(reg.supports_file(&dir.join(name)), "{name}");
    }
}

/// Without its world file and `.prj` a picture has no georeference; with them, the readout
/// names the companion files.
#[test]
fn world_file_names_its_sidecars() {
    let Some(dir) = fixtures() else { return };
    let s = Registry::default()
        .open(Arc::new(Source::open(dir.join("photo.jpg")).unwrap()))
        .unwrap();
    let g = s.geo().expect("georeferenced");
    let GeoFacts::Raster(r) = &g.facts else {
        panic!()
    };
    assert_eq!(r.sidecars, ["photo.JGW", "photo.prj"]);
    // The same bytes in memory: no companions are looked for.
    let bytes = std::fs::read(dir.join("photo.jpg")).unwrap();
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("photo.jpg", bytes)))
        .unwrap();
    assert!(s.geo().is_none());
}
