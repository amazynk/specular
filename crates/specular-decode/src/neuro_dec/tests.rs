use std::path::PathBuf;

use super::*;
use crate::Registry;

fn dir() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../formats/specular-neuro/tests/data"
    ))
}

/// Opens a fixture where it lies (so surfaces beside it are found).
fn open(name: &str) -> Result<Arc<dyn Session>> {
    let s = Source::open(dir().join(name)).expect(name);
    Registry::default().open(Arc::new(s))
}

/// Opens a fixture's bytes under a name with no folder to look in.
fn open_alone(name: &str, as_name: &str) -> Result<Arc<dyn Session>> {
    let data = std::fs::read(dir().join(name)).expect(name);
    Registry::default().open(Arc::new(Source::from_vec(as_name, data)))
}

fn colours(s: &dyn Session) -> Vec<[u8; 4]> {
    s.mesh()
        .expect("a mesh")
        .mesh
        .colors
        .clone()
        .expect("colours")
}

#[test]
fn surfaces_open_as_meshes() {
    for (file, vertices) in [
        ("sub.L.midthickness.surf.gii", 42),
        ("sub.R.midthickness.surf.gii", 42),
        ("surf_ascii.surf.gii", 0),
        ("surf_ascii_cm.surf.gii", 0),
        ("surf_b64_be.surf.gii", 0),
        ("surf_gz_be.surf.gii", 0),
    ] {
        let s = open(file).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(s.info().format, Format::Gifti, "{file}");
        let m = &s.mesh().expect("a mesh").mesh;
        if vertices > 0 {
            assert_eq!(m.positions.len(), vertices, "{file}");
        }
        assert!(m.triangle_count() > 0, "{file}");
        let f = s.neuro().and_then(|n| n.gifti.clone()).expect("facts");
        assert_eq!(f.role, "surface");
    }
}

#[test]
fn data_files_colour_the_surface_beside_them() {
    for (file, pages, labels) in [
        ("sub.L.thickness.func.gii", 3, false),
        ("sub.L.sulc.shape.gii", 1, false),
        ("sub.L.aparc.label.gii", 1, true),
    ] {
        let s = open(file).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(s.page_count(), pages, "{file}");
        let f = s.neuro().unwrap();
        let g = f.gifti.as_ref().unwrap();
        assert_eq!(
            g.surface.as_deref(),
            Some("sub.L.midthickness.surf.gii"),
            "{file}"
        );
        assert_eq!(f.scalars.is_some(), !labels, "{file}");
        let c = colours(s.as_ref());
        assert_eq!(c.len(), 42);
        let distinct: std::collections::HashSet<_> = c.iter().collect();
        assert!(distinct.len() > 1, "{file}: one colour");
        for p in 1..pages {
            let q = s.open_page(p).unwrap();
            assert_eq!(q.page_index(), p);
            assert!(matches!(q.page_label(), Some(PageLabel::Map { .. })));
            assert_ne!(colours(q.as_ref()), c, "{file} page {p}");
        }
    }
}

#[test]
fn data_alone_needs_a_surface() {
    let e = open_alone("sub.L.thickness.func.gii", "x.L.thickness.func.gii")
        .err()
        .expect("an error");
    assert_eq!(needs_surface(&e), Some(42), "{e}");
    // A CIFTI file of a hemisphere and nothing else, with no surface beside it.
    let e = open("alone/left_only.dscalar.nii").err().expect("an error");
    assert_eq!(needs_surface(&e), Some(42), "{e}");
    assert_eq!(needs_surface(&DecodeError::TooLarge), None);
}

#[test]
fn cifti_maps_colour_both_hemispheres() {
    for (file, labels) in [
        ("sub.dscalar.nii", false),
        ("sub.dlabel.nii", true),
        ("sub.dtseries.nii", false),
        ("sub.pscalar.nii", false),
    ] {
        let s = open(file).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(s.info().format, Format::Cifti, "{file}");
        let m = &s.mesh().expect("a mesh").mesh;
        assert_eq!(m.positions.len(), 84, "{file}");
        let f = s.neuro().unwrap().cifti.clone().unwrap();
        assert_eq!(f.shown, "surfaces");
        assert!(
            f.surfaces.iter().all(|s| s.2.is_some()),
            "{file}: {:?}",
            f.surfaces
        );
        assert_eq!(s.page_count() as usize, f.maps.max(1), "{file}");
        assert_eq!(s.neuro().unwrap().scalars.is_some(), !labels, "{file}");
        let c = colours(s.as_ref());
        // The two medial-wall vertices of the left cortex are grey.
        if !file.starts_with("sub.p") {
            assert_eq!(c.iter().filter(|&&c| c == NO_DATA).count(), 2, "{file}");
        }
        if s.page_count() > 1 {
            let q = s.open_page(s.page_count() - 1).unwrap();
            assert!(matches!(q.page_label(), Some(PageLabel::Map { .. })));
        }
    }
}

#[test]
fn cifti_without_surfaces_shows_its_voxels() {
    let s = open_alone("sub.dscalar.nii", "x.dscalar.nii").unwrap();
    assert_eq!(s.info().format, Format::Cifti);
    assert!(s.mesh().is_none());
    let f = s.neuro().unwrap().cifti.clone().unwrap();
    assert_eq!(f.shown, "voxels");
    assert_eq!(f.voxels, 40);
    let c = CancelToken::new();
    let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
    assert!(r.width > 0 && r.height > 0);
}

#[test]
fn tractograms_open() {
    for (file, format) in [
        ("v1.trk", Format::Trk),
        ("scalars.trk", Format::Trk),
        ("be_oblique.trk", Format::Trk),
        ("plain.tck", Format::Tck),
        ("be.tck", Format::Tck),
    ] {
        for name in [file, "tracts"] {
            let s = open_alone(file, name).unwrap_or_else(|e| panic!("{file}: {e}"));
            assert_eq!(s.info().format, format, "{file} as {name}");
            assert!(s.info().format.is_tracts());
            let t = &s.tracts().expect("tracts").tracts;
            assert!(t.streamlines() > 0);
            let f = s.neuro().unwrap().tracts.clone().unwrap();
            assert_eq!(f.points, t.points.len());
            assert!(f.mean_length.unwrap() > 0.0);
        }
    }
    let s = open("scalars.trk").unwrap();
    let f = s.neuro().unwrap().tracts.clone().unwrap();
    assert!(!f.scalars.is_empty() && !f.properties.is_empty());
}

#[test]
fn large_tractograms_are_subsampled() {
    let mut t = Tractogram::default();
    for s in 0..3000u32 {
        t.offsets.push(t.points.len() as u32);
        for k in 0..500 {
            t.points.push([s as f32, k as f32, 0.0]);
        }
    }
    t.offsets.push(t.points.len() as u32);
    let shown = t.subsample(MAX_SHOWN_POINTS);
    assert!(shown.points.len() <= MAX_SHOWN_POINTS);
    assert!(shown.points.len() > MAX_SHOWN_POINTS / 2);
}

#[test]
fn previews_draw_and_regions_crop() {
    for file in ["scalars.trk", "sub.L.thickness.func.gii", "sub.dlabel.nii"] {
        let s = open(file).unwrap();
        let i = s.info().clone();
        assert_eq!(
            i.width.max(i.height),
            crate::mesh_dec::PREVIEW_LONG_SIDE,
            "{file}"
        );
        let c = CancelToken::new();
        let full = s.decode(&DecodeRequest::full(4), &c).unwrap();
        let px = &full.planes[0].data;
        assert!(
            px.chunks_exact(4).filter(|p| p[3] > 0).count() > 200,
            "{file}"
        );
        let mut req = DecodeRequest::full(4);
        req.region = Some(crate::Rect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
        });
        let part = s.decode(&req, &c).unwrap();
        assert_eq!((part.width, part.height, part.origin), (30, 40, (10, 20)));
        let stride = full.planes[0].stride;
        assert_eq!(
            part.planes[0].data[..30 * 4],
            px[20 * stride + 10 * 4..20 * stride + 40 * 4]
        );
        assert!(s.memory_bytes() > 0);
    }
}

#[test]
fn direction_colours_follow_the_axes() {
    assert_eq!(
        direction_colour([0.0; 3], [2.0, 0.0, 0.0]),
        [255, 0, 0, 255]
    );
    assert_eq!(
        direction_colour([0.0; 3], [0.0, -1.0, 0.0]),
        [0, 255, 0, 255]
    );
    assert_eq!(direction_colour([0.0; 3], [0.0; 3]), [200, 200, 200, 255]);
}

#[test]
fn junk_is_an_error() {
    for (name, data) in [
        (
            "x.surf.gii",
            b"<?xml version=\"1.0\"?><GIFTI Version=\"1.0\"><DataArray".to_vec(),
        ),
        ("x.trk", b"TRACK\0\0\0".to_vec()),
        ("x.tck", b"mrtrix tracks\nEND\n".to_vec()),
        ("x.dscalar.nii", vec![0x1c, 0x02, 0, 0, 1, 2, 3]),
    ] {
        assert!(
            Registry::default()
                .open(Arc::new(Source::from_vec(name, data)))
                .is_err(),
            "{name}"
        );
    }
}

#[test]
fn mrtrix_images_open_as_volumes() {
    let dir = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../formats/specular-medvol/tests/data/mif"
    ));
    for f in ["neg.mif", "scaled.mif.gz", "vol4d.mih"] {
        let s = Registry::default()
            .open(Arc::new(Source::open(dir.join(f)).unwrap()))
            .unwrap_or_else(|e| panic!("{f}: {e}"));
        assert_eq!(s.info().format, Format::Mif, "{f}");
    }
}

#[test]
fn ranges_are_robust_and_diverge_around_zero() {
    let v: Vec<f32> = (-50..=50).map(|x| x as f32).collect();
    let (r, d) = scalar_range(&v);
    assert!(d && r.0 == -r.1);
    let v: Vec<f32> = (0..=100).map(|x| x as f32).collect();
    let (r, d) = scalar_range(&v);
    assert!(!d && r == (2.0, 98.0));
    assert_eq!(scalar_range(&[f32::NAN]), ((0.0, 1.0), false));
}

/// A dense connectivity file beside its surfaces opens on them, coloured by the first
/// seed's row; picking a vertex reads that seed's row, whose own vertex is the strongest
/// (1: the fixture's connectivity falls off with distance from the seed).
#[test]
fn dense_connectivity_colours_the_surfaces_by_the_picked_seed() {
    let s = open("sub.dconn.nii").unwrap();
    assert_eq!(s.info().format, Format::Cifti);
    let n = s.neuro().unwrap();
    let f = n.cifti.clone().unwrap();
    assert_eq!(f.shown, "surfaces");
    assert_eq!(f.dense, Some((f.shape[1], f.shape[0] * 4)));
    assert_eq!(s.page_count(), 1);
    let conn = n.connectivity.clone().expect("connectivity").0;
    assert_eq!(conn.seeds(), f.shape[1]);
    let m = &s.mesh().unwrap().mesh;
    assert_eq!(m.positions.len(), 84);
    // The two medial-wall vertices of the left cortex have no seed and no value.
    let seedless: Vec<u32> = (0..84)
        .filter(|&v| conn.seed_at_vertex(v).is_none())
        .collect();
    assert_eq!(seedless, [3, 17]);
    assert_eq!(
        colours(s.as_ref())
            .iter()
            .filter(|&&c| c == NO_DATA)
            .count(),
        2
    );
    for v in [0u32, 41, 42, 83] {
        let seed = conn.seed_at_vertex(v).unwrap();
        let values = conn.vertex_values(&conn.row(seed).unwrap());
        assert!((values[v as usize] - 1.0).abs() < 1e-6, "vertex {v}");
        let max = values
            .iter()
            .copied()
            .filter(|x| x.is_finite())
            .fold(f32::MIN, f32::max);
        assert!((max - 1.0).abs() < 1e-6);
        let (c, sc) = conn.surface(seed).unwrap();
        assert_eq!(c.len(), 84);
        assert_eq!(sc.values.len(), 84);
    }
    assert!(
        conn.seed_name(0).starts_with("CORTEX_LEFT"),
        "{}",
        conn.seed_name(0)
    );
    assert!(conn.row(conn.seeds() as u32).is_err());
}

/// A dense connectivity file over voxels alone opens as the first seed's row volume; a
/// position in the thalamus picks its seed, whose row is 1 at the seed and falls off with
/// distance (the fixture's 1 - d/10, d in voxels).
#[test]
fn dense_connectivity_over_voxels_is_a_volume_per_seed() {
    let s = open("alone/vox.dconn.nii").unwrap();
    let n = s.neuro().unwrap();
    let f = n.cifti.clone().unwrap();
    assert_eq!(f.shown, "voxels");
    assert_eq!(f.dense, Some((24, 40 * 8)));
    let conn = n.connectivity.clone().expect("connectivity").0;
    assert!(s.mesh().is_none());
    assert!(s.prepared_volume().is_some());
    // Voxel (2, 3, 2) of the 2 mm grid at (9, -12, -8), x flipped: RAS (5, -6, -4).
    let seed = conn.seed_at_ras([5.0, -6.0, -4.0]).expect("a seed");
    assert_eq!(seed, 0);
    assert_eq!(conn.seed_at_ras([5.4, -5.7, -4.6]), Some(0));
    // Voxel (4, 6, 3): the last of the thalamus.
    assert_eq!(conn.seed_at_ras([1.0, 0.0, -2.0]), Some(23));
    assert_eq!(conn.seed_name(23), "THALAMUS_LEFT (4, 6, 3)");
    // Putamen voxels are columns, not seeds; outside the grid there is nothing.
    assert_eq!(conn.seed_at_ras([-3.0, -2.0, 0.0]), None);
    assert_eq!(conn.seed_at_ras([500.0, 0.0, 0.0]), None);
    let vol = conn.volume_of_seed(23).unwrap();
    assert_eq!(vol.value(4, 6, 3, 0, 0), 1.0);
    // Putamen voxel (6, 5, 4): 3 voxels away.
    let d = (4.0f64 + 1.0 + 1.0).sqrt();
    assert!((vol.value(6, 5, 4, 0, 0) - (1.0 - d / 10.0)).abs() < 1e-6);
    assert_eq!(vol.value(0, 0, 0, 0, 0), 0.0);
    let first = s.prepared_volume().unwrap();
    assert_eq!(first.value(2, 3, 2, 0, 0), 1.0);
}

/// The tractograms beside a volume are found by extension and read as the 3D view reads
/// them.
#[test]
fn tracts_beside_a_volume_are_found_and_read() {
    let found = tracts_beside(&dir().join("sub.dscalar.nii"));
    let names: Vec<String> = found.iter().map(|p| file_name(p)).collect();
    assert_eq!(
        names,
        [
            "be.tck",
            "be_oblique.trk",
            "plain.tck",
            "scalars.trk",
            "v1.trk"
        ]
    );
    for p in &found {
        let t = read_tracts(p).unwrap();
        let s = Registry::default()
            .open(Arc::new(Source::open(p).unwrap()))
            .unwrap();
        assert_eq!(
            t.points,
            s.tracts().unwrap().tracts.points,
            "{}",
            p.display()
        );
    }
    assert!(tracts_beside(&dir().join("alone/vox.dconn.nii")).is_empty());
    assert!(read_tracts(&dir().join("sub.dscalar.nii")).is_err());
}

/// Times opening a large dense connectivity file (`SPECULAR_DCONN_BIG`, made by hand) and
/// fetching rows from it, first from disk and then from the page cache.
#[test]
#[ignore = "times a large file made by hand"]
fn dense_connectivity_row_timing() {
    use std::time::Instant;
    let Some(p) = std::env::var_os("SPECULAR_DCONN_BIG") else {
        return;
    };
    let t0 = Instant::now();
    let s = Registry::default()
        .open(Arc::new(Source::open(PathBuf::from(p)).unwrap()))
        .unwrap();
    let open = t0.elapsed();
    let conn = s.neuro().unwrap().connectivity.clone().unwrap().0;
    eprintln!(
        "open (first row as a volume): {:.1} ms; {} seeds, {} bytes a row",
        open.as_secs_f64() * 1e3,
        conn.seeds(),
        conn.row_bytes()
    );
    let n = conn.seeds() as u32;
    let seeds = [n / 2, n / 4, n - 1, n / 3, 3 * n / 4];
    for pass in ["cold", "warm"] {
        let mut ms = Vec::new();
        for &seed in &seeds {
            let t = Instant::now();
            let row = conn.row(seed).unwrap();
            ms.push((t.elapsed().as_secs_f64() * 1e5).round() / 100.0);
            assert_eq!(row.len(), conn.row_bytes() / 4);
        }
        let t = Instant::now();
        let _ = conn.volume_of_seed(seeds[0]).unwrap();
        eprintln!(
            "{pass}: rows {ms:?} ms; a row as a volume {:.1} ms",
            t.elapsed().as_secs_f64() * 1e3
        );
    }
}

/// Streamlines keep about one output pixel per 512 of the long side: a 1024-pixel rendering
/// draws them 2 pixels wide, so they survive fitting and thumbnails (the visual review found
/// 1-pixel lines nearly vanished).
#[test]
fn streamlines_scale_with_the_picture() {
    // Two parallel streamlines along one axis, far apart.
    let t = specular_neuro::Tractogram {
        points: vec![
            [0.0, 0.0, 0.0],
            [100.0, 0.0, 0.0],
            [0.0, 0.0, 60.0],
            [100.0, 0.0, 60.0],
        ],
        offsets: vec![0, 2, 4],
        scalars: Vec::new(),
        properties: Vec::new(),
    };
    // Coverage across the two lines in the middle column, in pixels (summed alpha).
    let thickness = |w: u32, h: u32| {
        let px = render_tracts(&t, w, h);
        let x = w as usize / 2;
        (0..h as usize)
            .map(|y| f64::from(px[(y * w as usize + x) * 4 + 3]) / 255.0)
            .sum::<f64>()
    };
    // Two lines, two pixels each at 1024; one pixel each at 512 and below.
    assert!(thickness(1024, 1024) >= 3.5, "{}", thickness(1024, 1024));
    assert!(thickness(256, 256) >= 1.5, "{}", thickness(256, 256));
}
