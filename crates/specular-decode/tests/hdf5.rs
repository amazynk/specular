//! HDF5, NetCDF and EMD through the registry: which arrays become pages and in what order,
//! the axes, coordinates and pixel size the science view gets, north-up NetCDF grids, and the
//! planes against the h5py / netCDF4-python values (the fixtures of specular-hdf5). Skipped
//! when no Python with h5py is available.

#[path = "../../formats/specular-hdf5/tests/common/mod.rs"]
mod common;

use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::science::{AxisKind, ScienceKind};
use specular_decode::{DecodeRequest, Format, Registry, Session};

fn open(name: &str) -> Option<(Arc<dyn Session>, std::path::PathBuf)> {
    let dir = common::fixtures()?;
    let path = dir.join(name);
    let data = std::fs::read(&path).ok()?;
    let s = Registry::default()
        .open(Arc::new(Source::from_vec(name, data)))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    Some((s, dir.to_path_buf()))
}

fn plane(s: &dyn Session, index: &[u32]) -> Vec<f32> {
    s.nd_plane(index, &CancelToken::new())
        .unwrap()
        .unwrap()
        .values
        .clone()
}

fn want(dir: &std::path::Path, file: &str, path: &str) -> (Vec<u64>, Vec<f64>) {
    common::expected(dir, file)
        .into_iter()
        .find(|(p, _, _)| p == path)
        .map(|(_, s, v)| (s, v))
        .unwrap_or_else(|| panic!("{file}: {path} not in the manifest"))
}

fn same(got: &[f32], want: &[f64]) -> usize {
    let w: Vec<f32> = want.iter().map(|&v| v as f32).collect();
    got.iter()
        .zip(&w)
        .filter(|(a, b)| !(a.to_bits() == b.to_bits() || a.is_nan() && b.is_nan()))
        .count()
        + got.len().abs_diff(w.len())
}

#[test]
fn hdf5_pages_ranked() {
    let Some((s, dir)) = open("old.h5") else {
        return;
    };
    assert_eq!(s.info().format, Format::Hdf5);
    assert!(s.page_count() > 10, "{}", s.page_count());
    // Tiny 2×2 datasets of /many are not pages.
    for p in 0..s.page_count() {
        let pg = s.open_page(p).unwrap();
        let sc = pg.science().unwrap();
        assert!(sc.width * sc.height >= 16, "page {p}: {:?}", sc.dataset);
    }
    // The 100×130 deflate dataset, read through the session, matches h5py.
    let deflate = (0..s.page_count())
        .map(|p| s.open_page(p).unwrap())
        .find(|p| p.science().unwrap().dataset.as_deref() == Some("/chunked/deflate"))
        .unwrap();
    assert_eq!((deflate.info().width, deflate.info().height), (130, 100));
    assert!(
        deflate.info().coding.contains("deflate"),
        "{}",
        deflate.info().coding
    );
    let (_, v) = want(&dir, "old", "/chunked/deflate");
    assert_eq!(same(&plane(&*deflate, &[]), &v), 0);
    // A cube: one slider, the last plane right.
    let cube = (0..s.page_count())
        .map(|p| s.open_page(p).unwrap())
        .find(|p| p.science().unwrap().dataset.as_deref() == Some("/chunked/cube"))
        .unwrap();
    let sc = cube.science().unwrap();
    assert_eq!(sc.axes.len(), 1);
    assert_eq!(sc.axes[0].len, 5);
    let (_, v) = want(&dir, "old", "/chunked/cube");
    assert_eq!(same(&plane(&*cube, &[4]), &v[4 * 40 * 30..]), 0);
    // The default raster decodes.
    let r = cube
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!((r.width, r.height), (30, 40));
}

#[test]
fn hdf5_rgb() {
    let Some((s, _)) = open("new.h5") else {
        return;
    };
    let rgb = (0..s.page_count())
        .map(|p| s.open_page(p).unwrap())
        .find(|p| p.science().unwrap().dataset.as_deref() == Some("/rgb"))
        .unwrap();
    let sc = rgb.science().unwrap();
    assert_eq!((sc.width, sc.height), (32, 24));
    assert_eq!(sc.channel_axis, Some(0));
    assert_eq!(sc.axes[0].kind, AxisKind::Channel);
    assert_eq!(sc.axes[0].labels, ["R", "G", "B"]);
    rgb.decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
}

#[test]
fn netcdf_classic_and_4() {
    for name in ["cdf1.nc", "cdf2.nc", "cdf5.nc", "nc4.nc", "nc4classic.nc"] {
        let Some((s, dir)) = open(name) else {
            return;
        };
        assert_eq!(s.info().format, Format::NetCdf, "{name}");
        // tas (3 × 18 × 24) first: the largest data variable, with a time slider.
        let sc = s.science().unwrap();
        let first = sc.dataset.as_deref().unwrap();
        assert!(
            first.starts_with("tas") || first.starts_with("chunked"),
            "{name}: {first}"
        );
        let tas = (0..s.page_count())
            .map(|p| s.open_page(p).unwrap())
            .find(|p| p.science().unwrap().dataset.as_deref() == Some("tas"))
            .unwrap();
        let sc = tas.science().unwrap();
        assert_eq!((sc.width, sc.height), (24, 18), "{name}");
        assert_eq!(sc.unit.as_deref(), Some("K"));
        assert_eq!(sc.axes.len(), 1);
        assert_eq!(sc.axes[0].name, "time");
        assert_eq!(
            sc.axes[0].labels,
            ["2000-01-01", "2000-02-01", "2000-02-29"],
            "{name}"
        );
        assert!(sc.keywords.iter().any(|(k, _, _)| k == ":title"), "{name}");
        // Latitude grows northwards in the file: shown north up, so rows are reversed.
        let stem = name.trim_end_matches(".nc");
        let (_, v) = want(&dir, stem, "tas");
        let got = plane(&*tas, &[2]);
        let mut flipped = Vec::new();
        for row in v[2 * 18 * 24..].chunks(24).rev() {
            flipped.extend_from_slice(row);
        }
        assert_eq!(same(&got, &flipped), 0, "{name}");
        // 1-D coordinate variables are not pages.
        for p in 0..s.page_count() {
            let d = s
                .open_page(p)
                .unwrap()
                .science()
                .unwrap()
                .dataset
                .clone()
                .unwrap();
            assert!(!["lat", "lon", "time"].contains(&d.as_str()), "{name}: {d}");
        }
    }
}

#[test]
fn emd_berkeley_and_velox() {
    let Some((s, dir)) = open("berkeley.emd") else {
        return;
    };
    assert_eq!(s.info().format, Format::Emd);
    assert_eq!(s.page_count(), 1);
    let sc = s.science().unwrap();
    assert_eq!((sc.width, sc.height), (64, 48));
    let px = sc.pixel.as_ref().unwrap();
    assert_eq!(
        (px.x.step, px.y.step, px.unit_x.as_str()),
        (0.25, 0.25, "nm")
    );
    assert_eq!(sc.kind, ScienceKind::ElectronMicroscopy);
    assert_eq!(sc.instrument.as_deref(), Some("TEAM 0.5"));
    let (_, v) = want(&dir, "berkeley", "/data/haadf/data");
    assert_eq!(same(&plane(&*s, &[]), &v), 0);

    let Some((s, dir)) = open("velox.emd") else {
        return;
    };
    assert_eq!(s.info().format, Format::Emd);
    let sc = s.science().unwrap();
    assert_eq!((sc.width, sc.height), (56, 40));
    assert_eq!(sc.axes.len(), 1);
    assert_eq!(sc.axes[0].len, 2);
    let px = sc.pixel.as_ref().unwrap();
    assert_eq!((px.x.step, px.unit_x.as_str()), (2.5e-10, "m"));
    assert_eq!((px.x.offset, px.y.offset), (-7e-9, -5e-9));
    assert_eq!(sc.instrument.as_deref(), Some("Talos F200X, HAADF"));
    // Frame 1 of the (40, 56, 2) stack.
    let path = common::expected(&dir, "velox")
        .into_iter()
        .find(|(p, _, _)| p.ends_with("/Data"))
        .unwrap();
    let frame: Vec<f64> = path.2.iter().skip(1).step_by(2).copied().collect();
    assert_eq!(same(&plane(&*s, &[1]), &frame), 0);
    s.decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
}

/// Open and first-plane times of the files `make_speed.py` writes into
/// `SPECULAR_HDF5_SPEED` (run with `--ignored --nocapture`).
#[test]
#[ignore]
fn speed() {
    let Some(dir) = std::env::var_os("SPECULAR_HDF5_SPEED") else {
        return;
    };
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    names.sort();
    for path in names {
        let t0 = std::time::Instant::now();
        let s = Registry::default().open_path(&path).unwrap();
        let open = t0.elapsed();
        let t1 = std::time::Instant::now();
        let p = s.nd_plane(
            &vec![0; s.science().unwrap().axes.len()],
            &CancelToken::new(),
        );
        let p = p.unwrap().unwrap();
        let plane = t1.elapsed();
        let t2 = std::time::Instant::now();
        s.decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        let raster = t2.elapsed();
        println!(
            "{}: {}×{} {}: open {:.1} ms, first plane {:.1} ms, auto-stretched raster {:.1} ms",
            path.file_name().unwrap().to_string_lossy(),
            p.width,
            p.height,
            s.info().coding,
            open.as_secs_f64() * 1e3,
            plane.as_secs_f64() * 1e3,
            raster.as_secs_f64() * 1e3,
        );
    }
}

/// Truncated files never panic; a cancelled plane read fails and is not remembered.
#[test]
fn damaged_and_cancelled() {
    let Some((_, dir)) = open("plugins.h5") else {
        return;
    };
    let data = std::fs::read(dir.join("plugins.h5")).unwrap();
    // Truncated copies open or fail cleanly, and their planes read or fail cleanly.
    for cut in (0..data.len()).step_by(data.len() / 37 + 1) {
        let src = Arc::new(Source::from_vec("cut.h5", data[..cut].to_vec()));
        if let Ok(s) = Registry::default().open(src) {
            let _ = s.decode(&DecodeRequest::full(1), &CancelToken::new());
        }
    }
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("p.h5", data)))
        .unwrap();
    let c = CancelToken::new();
    c.cancel();
    assert!(s.nd_plane(&[], &c).unwrap().is_err());
    // A cancelled read is not remembered.
    assert!(s.nd_plane(&[], &CancelToken::new()).unwrap().is_ok());
}

/// Opens a fixture from disk (linked files resolve next to it).
fn open_path(path: &std::path::Path) -> specular_decode::Result<Arc<dyn Session>> {
    Registry::default().open(Arc::new(Source::open(path).unwrap()))
}

fn pages(s: &Arc<dyn Session>) -> Vec<Arc<dyn Session>> {
    (0..s.page_count())
        .map(|p| s.open_page(p).unwrap())
        .collect()
}

#[test]
fn nexus_default_chain() {
    let Some((s, dir)) = open("nexus.nxs") else {
        return;
    };
    assert_eq!(s.info().format, Format::Nexus);
    // The @default signal comes first, before the larger decoy.
    let sc = s.science().unwrap();
    assert_eq!(
        sc.dataset.as_deref(),
        Some("/scan/plot/counts — detector counts")
    );
    assert_eq!((sc.width, sc.height), (30, 20));
    assert_eq!(sc.kind, ScienceKind::Gridded);
    assert_eq!(sc.unit.as_deref(), Some("counts"));
    assert_eq!(sc.axes.len(), 1);
    let energy = &sc.axes[0];
    assert_eq!(energy.name, "energy");
    assert_eq!(energy.unit, "keV");
    assert_eq!(energy.len, 3);
    let lin = energy.linear.unwrap();
    assert!((lin.at(2.0) - 9.0).abs() < 1e-9, "{lin:?}");
    // y is "." in @axes: no pixel calibration from x alone.
    assert!(sc.pixel.is_none(), "{:?}", sc.pixel);
    let (_, v) = want(&dir, "nexus", "/scan/plot/counts");
    assert_eq!(same(&plane(&*s, &[2]), &v[2 * 600..]), 0);
    // The decoy is still a page.
    assert!(
        pages(&s)
            .iter()
            .any(|p| p.science().unwrap().dataset.as_deref() == Some("/other/data"))
    );
}

#[test]
fn eiger_external_links() {
    let Some(dir) = common::fixtures() else {
        return;
    };
    let s = open_path(&dir.join("eiger_master.h5")).unwrap();
    assert_eq!(s.info().format, Format::Nexus);
    let sc = s.science().unwrap();
    assert_eq!((sc.width, sc.height), (50, 40));
    assert_eq!(sc.axes.len(), 1);
    assert_eq!(sc.axes[0].len, 5, "{:?}", sc.axes);
    assert_eq!(sc.detector.as_deref(), Some("Dectris EIGER2 Si 500K"));
    let (_, a) = want(dir, "eiger_data_000001", "/entry/data/data");
    let (_, b) = want(dir, "eiger_data_000002", "/entry/data/data");
    let n = 2000;
    for k in 0..5 {
        let w = if k < 3 {
            &a[k * n..(k + 1) * n]
        } else {
            &b[(k - 3) * n..(k - 2) * n]
        };
        assert_eq!(same(&plane(&*s, &[k as u32]), w), 0, "frame {k}");
    }

    // Links out of the master's folder are not followed unless the target was opened.
    let esc = dir.join("escape/eiger_master.h5");
    let text = match open_path(&esc) {
        Ok(s) => s
            .science()
            .unwrap()
            .keywords
            .iter()
            .map(|(k, v, _)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; "),
        Err(e) => e.to_string(),
    };
    assert!(text.contains("outside the file's folder"), "{text}");
    specular_decode::allow_linked_file(&dir.join("eiger_data_000001.h5"));
    specular_decode::allow_linked_file(&dir.join("eiger_data_000002.h5"));
    let s = open_path(&esc).unwrap();
    assert_eq!(s.science().unwrap().axes[0].len, 5);
    assert_eq!(same(&plane(&*s, &[4]), &b[n..]), 0);
}

#[test]
fn imaris_levels_channels_time() {
    let Some((s, dir)) = open("imaris.ims") else {
        return;
    };
    assert_eq!(s.info().format, Format::Imaris);
    assert_eq!((s.info().width, s.info().height), (50, 40));
    let sc = s.science().unwrap();
    assert_eq!(sc.axes.len(), 3, "{:?}", sc.axes);
    let ch = sc
        .axes
        .iter()
        .find(|a| a.kind == AxisKind::Channel)
        .unwrap();
    assert_eq!(ch.labels, ["DAPI", "GFP"]);
    let z = sc.axes.iter().find(|a| a.len == 6).unwrap();
    assert!((z.linear.unwrap().step - 2.0).abs() < 1e-9, "{z:?}");
    let px = sc.pixel.as_ref().unwrap();
    assert!(
        (px.x.step - 0.5).abs() < 1e-12 && px.unit_x == "µm",
        "{px:?}"
    );
    // Channel 1, time point 1, z 5: the stored block is cropped to 50 × 40.
    let pos: Vec<u32> = sc
        .axes
        .iter()
        .map(|a| if a.len == 6 { 5 } else { 1 })
        .collect();
    let (_, v) = want(
        &dir,
        "imaris",
        "/DataSet/ResolutionLevel 0/TimePoint 1/Channel 1/Data",
    );
    let want0: Vec<f64> = (0..40)
        .flat_map(|y| (0..50).map(move |x| (5 * 48 + y) * 64 + x))
        .map(|i| v[i])
        .collect();
    assert_eq!(same(&plane(&*s, &pos), &want0), 0);
    // A half-size view reads resolution level 1.
    let r = s
        .decode(&DecodeRequest::full(2), &CancelToken::new())
        .unwrap();
    assert_eq!((r.width, r.height), (25, 20));
    // A thumbnail samples the coarsest level still twice its size: level 1 for 10 pixels.
    let t = s.decode_sampled(10, &CancelToken::new()).unwrap().unwrap();
    assert_eq!((t.width, t.height), (25, 20));
    let bad: usize = t
        .planes
        .iter()
        .zip(&r.planes)
        .map(|(a, b)| a.data.iter().zip(&b.data).filter(|(x, y)| x != y).count())
        .sum();
    assert_eq!(bad, 0);
    // Level 2 (12 × 10) is too small for 8 pixels; level 1 by every other sample.
    let t = s.decode_sampled(6, &CancelToken::new()).unwrap().unwrap();
    assert_eq!((t.width, t.height), (13, 10));
}

/// Thumbnails sample every n-th value of the h5py / netCDF4 fixtures' first plane.
#[test]
fn sampled_thumbnails() {
    for name in ["cdf1.nc", "nc4.nc", "berkeley.emd", "velox.emd", "new.h5"] {
        let Some((s, _)) = open(name) else {
            return;
        };
        let (w, h) = (s.info().width, s.info().height);
        let long = w.max(h);
        let c = CancelToken::new();
        assert!(s.decode_sampled(long, &c).is_none(), "{name}");
        if long < 8 {
            continue;
        }
        let side = long / 4;
        let step = long / (side * 2);
        let r = s.decode_sampled(side, &c).unwrap().unwrap();
        assert_eq!(
            (r.width, r.height),
            (w.div_ceil(step), h.div_ceil(step)),
            "{name}"
        );
    }
}

#[test]
fn bag_grid() {
    let Some((s, dir)) = open("bag.bag") else {
        return;
    };
    assert_eq!(s.info().format, Format::Bag);
    assert_eq!(s.page_count(), 2);
    let sc = s.science().unwrap();
    assert_eq!(sc.dataset.as_deref(), Some("/BAG_root/elevation"));
    assert_eq!((sc.width, sc.height), (40, 30));
    let px = sc.pixel.as_ref().unwrap();
    assert_eq!((px.x.offset, px.x.step), (500000.0, 2.0));
    assert_eq!((px.y.offset, px.y.step), (4000116.0, -4.0));
    assert!(sc.keywords.iter().any(|(_, v, _)| v.contains("32618")));
    // Rows are stored south first: the last displayed row is file row 0, its first four
    // cells no-data.
    let (_, v) = want(&dir, "bag", "/BAG_root/elevation");
    let got = plane(&*s, &[]);
    for y in 0..30 {
        for x in 0..40 {
            let w = v[(29 - y) * 40 + x];
            let g = got[y * 40 + x];
            if w >= 1e5 {
                assert!(g.is_nan(), "({x},{y})");
            } else {
                assert_eq!(g, w as f32, "({x},{y})");
            }
        }
    }
}
