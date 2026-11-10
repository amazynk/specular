//! Unstructured meshes through the registry: classic and NetCDF-4 UGRID, FVCOM and ICON
//! files open as mesh sessions; the face and value the readout gets at a stored pixel
//! position against xugrid (the rows specular-ugrid's `gen/oracle.py` wrote); the painted
//! planes; the wireframe; grid-only meshes from the corpus (SPECULAR_UGRID_DIR) and the
//! time to open a large mesh and paint its first plane (SPECULAR_UGRID_BIG, a file).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use specular_core::{CancelToken, Source};
use specular_decode::science::{MeshLocation, ScienceSummary};
use specular_decode::{DecodeRequest, Registry, Session};

fn open(path: &Path) -> Arc<dyn Session> {
    let data = std::fs::read(path).unwrap();
    let name = path.file_name().unwrap().to_str().unwrap();
    Registry::default()
        .open(Arc::new(Source::from_vec(name, data)))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn var_name(s: &ScienceSummary) -> Option<&str> {
    s.dataset.as_deref().map(|d| d.split(" — ").next().unwrap())
}

/// Mesh coordinates to the stored pixel position.
fn to_pixel(s: &ScienceSummary, x: f64, y: f64) -> (f64, f64) {
    if let Some(g) = &s.geo {
        let west = g.bounds[0];
        let x = west + (x - west).rem_euclid(360.0);
        g.to_map.inverse().unwrap().apply(x, y)
    } else {
        let c = s.pixel.as_ref().unwrap();
        (
            (x - c.x.offset) / c.x.step + 0.5,
            (y - c.y.offset) / c.y.step + 0.5,
        )
    }
}

fn same(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-5 * (1.0 + b.abs())
}

/// The rows of `dir/expected.tsv` through the sessions: (checked, mismatches) per file.
fn against_xugrid(dir: &Path) -> BTreeMap<String, (usize, usize)> {
    let text = std::fs::read_to_string(dir.join("expected.tsv")).unwrap();
    let mut sessions: BTreeMap<String, Vec<Arc<dyn Session>>> = BTreeMap::new();
    let mut tally: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut notes = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let (file, name, index) = (f[0], f[1], f[2]);
        let x: f64 = f[3].parse().unwrap();
        let y: f64 = f[4].parse().unwrap();
        let face: i64 = f[5].parse().unwrap();
        let pages = sessions.entry(file.to_owned()).or_insert_with(|| {
            let s = open(&dir.join(file));
            (0..s.page_count())
                .map(|p| s.open_page(p).unwrap())
                .collect()
        });
        let page = if name == "-" {
            Some(&pages[0])
        } else {
            pages
                .iter()
                .find(|p| var_name(p.science().unwrap()) == Some(name))
        };
        let Some(page) = page else {
            continue;
        };
        let s = page.science().unwrap();
        let probe = s.mesh.as_ref().expect("a mesh session");
        let idx: Vec<u32> = if index == "-" {
            Vec::new()
        } else {
            index.split(',').map(|s| s.parse().unwrap()).collect()
        };
        let (px, py) = to_pixel(s, x, y);
        let hit = probe.at(px, py, &idx);
        let t = tally.entry(file.to_owned()).or_default();
        t.0 += 1;
        let got = hit.map_or(-1, |h| i64::from(h.face));
        if got != face {
            t.1 += 1;
            notes.push(format!("{file} at {x} {y}: face {got}, xugrid {face}"));
            continue;
        }
        if name == "-" || f[6] == "-" {
            continue;
        }
        let want: f64 = if f[6] == "nan" {
            f64::NAN
        } else {
            f[6].parse().unwrap()
        };
        let value = hit.map_or(f64::NAN, |h| h.value);
        if !same(value, want) {
            t.1 += 1;
            notes.push(format!(
                "{file} {name}[{index}] at {x} {y}: {value}, xugrid {want}"
            ));
        }
    }
    for (k, (n, b)) in &tally {
        eprintln!("{k}: {n} checked through the session, {b} mismatches");
    }
    for n in notes.iter().take(20) {
        eprintln!("  {n}");
    }
    tally
}

#[test]
fn fixtures_through_the_registry() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-ugrid/tests/data");
    let tally = against_xugrid(&dir);
    assert_eq!(tally.len(), 6);
    assert!(tally.values().map(|t| t.0).sum::<usize>() > 1000);
    assert_eq!(tally.values().map(|t| t.1).sum::<usize>(), 0);

    // FVCOM (classic, 64-bit offset): zeta on the nodes over time, u on the faces over
    // time and sigma layers; lon/lat with a georeference.
    let s = open(&dir.join("fvcom_like.nc"));
    assert_eq!(s.page_count(), 2);
    let sc = s.science().unwrap();
    assert_eq!(var_name(sc), Some("zeta"));
    assert!(sc.geo.is_some() && sc.pixel.is_none());
    let m = sc.mesh.as_ref().unwrap().facts();
    assert_eq!((m.convention, m.faces, m.triangles), ("UGRID", 40, 40));
    assert_eq!(m.location, Some(MeshLocation::Node));
    assert_eq!(sc.axes.len(), 1);
    assert_eq!(sc.axes[0].len, 2);
    let u = s.open_page(1).unwrap();
    let su = u.science().unwrap();
    assert_eq!(su.axes.len(), 2);
    assert_eq!(
        su.mesh.as_ref().unwrap().facts().location,
        Some(MeshLocation::Face)
    );

    // The painted plane: finite where the mesh is, NaN past it, and it differs between
    // the time steps.
    let c = CancelToken::new();
    let a = s.nd_plane(&[0], &c).unwrap().unwrap();
    let b = s.nd_plane(&[1], &c).unwrap().unwrap();
    assert_eq!((a.width, a.height), (sc.width, sc.height));
    let finite = a.values.iter().filter(|v| v.is_finite()).count();
    assert!(finite > a.values.len() / 2, "{finite}");
    assert_ne!(a.values, b.values);

    // The wireframe: every edge over the whole image, none past a cap.
    let probe = sc.mesh.as_ref().unwrap();
    let all = [0.0, 0.0, f64::from(sc.width), f64::from(sc.height)];
    let e = probe.edges(all, 100_000).unwrap();
    assert_eq!(e.len() as u64, m.edges);
    assert!(probe.edges(all, 10).is_none());

    // Projected x and y (classic): a pixel calibration instead.
    let s = open(&dir.join("tri_start0.nc"));
    let sc = s.science().unwrap();
    assert!(sc.geo.is_none() && sc.pixel.is_some());

    // ICON (NetCDF-4): lon/lat with a georeference; a thumbnail decodes.
    let s = open(&dir.join("icon_data.nc"));
    let sc = s.science().unwrap();
    assert!(sc.geo.is_some() && sc.pixel.is_none());
    assert_eq!(sc.mesh.as_ref().unwrap().facts().convention, "ICON");
    let r = s.decode(&DecodeRequest::full(8), &c).unwrap();
    assert!(r.width > 0 && r.height > 0);

    // Mixed triangles, quads and a pentagon with fill values (NetCDF-4, start_index 1).
    let s = open(&dir.join("mixed_fill_start1.nc"));
    let m = s.science().unwrap().mesh.as_ref().unwrap().facts().clone();
    assert!(m.triangles > 0 && m.quads > 0 && m.polygons == 1 && m.max_face_nodes == 5);
}

#[test]
fn corpus_through_the_registry() {
    let Some(dir) = std::env::var_os("SPECULAR_UGRID_DIR") else {
        return;
    };
    let dir = Path::new(&dir);
    if !dir.join("expected.tsv").exists() {
        return;
    }
    let tally = against_xugrid(dir);
    assert_eq!(tally.values().map(|t| t.1).sum::<usize>(), 0);
    // Grids without data: one page counting each face's nodes.
    for f in ["ux_outCSne30.nc", "ux_quad_hexagon.nc"] {
        let p = dir.join(f);
        if !p.exists() {
            continue;
        }
        let s = open(&p);
        let sc = s.science().unwrap();
        let m = sc.mesh.as_ref().unwrap().facts();
        if m.location.is_none() {
            assert_eq!(s.page_count(), 1);
            let pl = s.nd_plane(&[], &CancelToken::new()).unwrap().unwrap();
            let max = pl
                .values
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .fold(0.0, f32::max);
            assert_eq!(max as u32, m.max_face_nodes, "{f}");
        }
    }
}

#[test]
fn big_mesh_timing() {
    let Some(path) = std::env::var_os("SPECULAR_UGRID_BIG") else {
        return;
    };
    let t = Instant::now();
    let s = open(Path::new(&path));
    let opened = t.elapsed();
    let sc = s.science().unwrap();
    let start = sc.start.clone();
    let p = s.nd_plane(&start, &CancelToken::new()).unwrap().unwrap();
    let painted = t.elapsed();
    let m = sc.mesh.as_ref().unwrap().facts();
    eprintln!(
        "{} faces, {} nodes: opened in {:.0} ms, first {}x{} plane at {:.0} ms",
        m.faces,
        m.nodes,
        opened.as_secs_f64() * 1e3,
        p.width,
        p.height,
        painted.as_secs_f64() * 1e3
    );
    let t = Instant::now();
    let probe = sc.mesh.as_ref().unwrap();
    let mut hits = 0;
    for k in 0..10_000 {
        let x = f64::from(k % 100) / 100.0 * f64::from(sc.width);
        let y = f64::from(k / 100) / 100.0 * f64::from(sc.height);
        hits += usize::from(probe.at(x, y, &start).is_some());
    }
    eprintln!(
        "10000 readouts: {hits} on the mesh, {:.1} µs each",
        t.elapsed().as_secs_f64() * 1e2
    );
}
