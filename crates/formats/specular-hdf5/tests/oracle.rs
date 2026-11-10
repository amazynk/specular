//! Every numeric dataset of the h5py-written fixtures, read whole and plane by plane, against
//! numpy's values, bit for bit.

mod common;

use specular_hdf5::{File, PlaneSpec, Value, emd};

fn check_file(name: &str, ext: &str) -> usize {
    let Some(dir) = common::fixtures() else {
        return 0;
    };
    let data = std::fs::read(dir.join(format!("{name}{ext}"))).unwrap();
    let file = File::open(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert!(file.warnings.is_empty(), "{name}: {:?}", file.warnings);
    let mut checked = 0;
    for (path, shape, want) in common::expected(dir, name) {
        let i = file
            .index_of(&path)
            .unwrap_or_else(|| panic!("{name}: {path} not found"));
        let ds = file.object(i).and_then(|o| o.dataset()).unwrap();
        assert_eq!(ds.shape(), shape.as_slice(), "{name}: {path} shape");
        let got: Vec<f64> = file
            .read_all(&data, i, u64::MAX)
            .unwrap_or_else(|e| panic!("{name}: {path}: {e}"));
        let bad = common::mismatches(&got, &want);
        assert_eq!(bad, 0, "{name}: {path}: {bad} mismatches");
        // The last plane through the plane reader too (y = second-to-last, x = last dim).
        if shape.len() >= 2 {
            let r = shape.len();
            let mut index = vec![0u64; r];
            for d in 0..r - 2 {
                index[d] = shape[d] - 1;
            }
            let spec = PlaneSpec {
                x_dim: r - 1,
                y_dim: r - 2,
                index: index.clone(),
            };
            let p = file.read_plane::<f64>(&data, i, &spec, &|| false).unwrap();
            assert!(!p.truncated);
            let plane = (shape[r - 1] * shape[r - 2]) as usize;
            let off = want.len() - plane;
            assert_eq!(
                common::mismatches(&p.values, &want[off..]),
                0,
                "{name}: {path} plane"
            );
            // And transposed.
            let spec = PlaneSpec {
                x_dim: r - 2,
                y_dim: r - 1,
                index,
            };
            let t = file.read_plane::<f64>(&data, i, &spec, &|| false).unwrap();
            let (w, h) = (t.width, t.height);
            for y in 0..h {
                for x in 0..w {
                    let a = t.values[y * w + x];
                    let b = p.values[x * h + y];
                    assert!(a.to_bits() == b.to_bits() || a.is_nan() && b.is_nan());
                }
            }
            // Every n-th sample of the last plane (the thumbnail path), against numpy's.
            let (w, h) = (shape[r - 1] as usize, shape[r - 2] as usize);
            let mut index = vec![0u64; r];
            for d in 0..r - 2 {
                index[d] = shape[d] - 1;
            }
            for step in [2usize, 3, 7] {
                let spec = PlaneSpec {
                    x_dim: r - 1,
                    y_dim: r - 2,
                    index: index.clone(),
                };
                let s = file
                    .read_plane_strided::<f64>(&data, i, &spec, step as u64, u64::MAX, &|| false)
                    .unwrap_or_else(|e| panic!("{name}: {path} strided: {e}"));
                assert!(!s.truncated);
                assert_eq!((s.width, s.height), (w.div_ceil(step), h.div_ceil(step)));
                let sub: Vec<f64> = (0..h)
                    .step_by(step)
                    .flat_map(|y| (0..w).step_by(step).map(move |x| y * w + x))
                    .map(|k| want[off + k])
                    .collect();
                assert_eq!(
                    common::mismatches(&s.values, &sub),
                    0,
                    "{name}: {path} strided by {step}"
                );
            }
        }
        checked += 1;
    }
    checked
}

#[test]
fn old_style_file() {
    let n = check_file("old", ".h5");
    if n == 0 {
        return;
    }
    assert!(n >= 80, "{n}");
    let dir = common::fixtures().unwrap();
    let data = std::fs::read(dir.join("old.h5")).unwrap();
    let f = File::open(&data).unwrap();
    assert_eq!(f.superblock_version, 0);
    let root = f.get("/").unwrap();
    assert_eq!(
        root.attr("title").and_then(Value::as_str),
        Some("old style")
    );
    assert_eq!(
        root.attr("fixed").and_then(Value::as_str),
        Some("fixed string")
    );
    assert_eq!(
        root.attr("vlen").and_then(Value::as_str),
        Some("variable string")
    );
    assert_eq!(root.attr("answer").and_then(Value::as_f64), Some(42.0));
    assert_eq!(
        root.attr("scales").and_then(Value::as_f64s),
        Some(vec![1.5, -2.25, 3.0])
    );
    assert_eq!(f.children("/many").count(), 70);
    let d = f.get("/chunked/deflate").unwrap().dataset().unwrap();
    assert_eq!(d.filter_names(), ["shuffle", "deflate", "fletcher32"]);
}

#[test]
fn new_style_file() {
    let n = check_file("new", ".h5");
    if n == 0 {
        return;
    }
    let dir = common::fixtures().unwrap();
    let data = std::fs::read(dir.join("new.h5")).unwrap();
    let f = File::open(&data).unwrap();
    assert_eq!(f.superblock_version, 3);
    let root = f.get("/").unwrap();
    assert_eq!(root.attr("a11").and_then(Value::as_f64), Some(2.75));
    let big = root.attr("big").and_then(Value::as_f64s).unwrap();
    assert_eq!(big.len(), 20000);
    assert_eq!(big[19999], 19999.0);
    assert_eq!(f.children("/dense").count(), 40);
    let attrs = common::expected_attrs(dir, "new");
    assert_eq!(attrs["/"]["title"], "new style");
}

#[test]
fn user_block() {
    check_file("userblock", ".h5");
}

#[test]
fn plugin_filters() {
    let n = check_file("plugins", ".h5");
    if n > 0 {
        assert!(n >= 30, "{n}");
    }
}

#[test]
fn packing_filters_and_blosc2() {
    let n = check_file("packers", ".h5");
    if n > 0 {
        assert!(n >= 70, "{n}");
    }
}

#[test]
fn szip_filter() {
    let n = check_file("szip", ".h5");
    if n > 0 {
        assert!(n >= 24, "{n}");
    }
}

/// Every numeric field of compound datasets, against numpy's field values.
#[test]
fn compound_fields() {
    for name in ["compound_old", "compound_new"] {
        let n = check_file(name, ".h5");
        if n == 0 {
            return;
        }
        assert_eq!(n, 26, "{name}");
        let dir = common::fixtures().unwrap();
        let data = std::fs::read(dir.join(format!("{name}.h5"))).unwrap();
        let f = File::open(&data).unwrap();
        let names: Vec<&str> = f
            .get("/table")
            .and_then(|o| o.dataset())
            .unwrap()
            .fields()
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "x", "y", "pos.a", "pos.b", "arr[0]", "arr[1]", "arr[2]", "z"
            ]
        );
        // Field views are not group members.
        assert!(f.children("/").all(|o| !o.path.contains('[')));
        assert_eq!(f.fields().count(), 26);
    }
}

/// Virtual datasets mapping from this file: strided, partial (fill value), mosaic, stack and
/// interleaved mappings, through every reader.
#[test]
fn virtual_datasets() {
    for name in ["virtual_local", "virtual_local_v110"] {
        let n = check_file(name, ".h5");
        if n == 0 {
            return;
        }
        assert_eq!(n, 10, "{name}");
    }
}

/// A virtual dataset whose frames live in other files, one of them missing (its frame holds
/// the fill value).
#[test]
fn virtual_dataset_from_other_files() {
    let Some(dir) = common::fixtures() else {
        return;
    };
    let master = std::fs::read(dir.join("vds_master.h5")).unwrap();
    let parts: Vec<(String, Vec<u8>)> = (0..3)
        .map(|k| {
            let n = format!("vds_part{k}.h5");
            let d = std::fs::read(dir.join(&n)).unwrap();
            (n, d)
        })
        .collect();
    let files: Vec<File> = parts.iter().map(|(_, d)| File::open(d).unwrap()).collect();
    let f = File::open(&master).unwrap();
    let i = f.index_of("/frames").unwrap();
    let ds = f.objects[i].dataset().unwrap();
    assert_eq!(ds.storage(), "virtual");
    assert_eq!(
        ds.virtual_files(),
        [
            "vds_part0.h5",
            "vds_part1.h5",
            "vds_part2.h5",
            "vds_part3.h5"
        ]
    );
    let want = common::read_f64(&dir.join("vds_master.bin"));
    let resolve = |name: &str| {
        let k = parts.iter().position(|(n, _)| n == name)?;
        Some((&files[k], parts[k].1.as_slice()))
    };
    let plane = 25 * 35;
    for t in 0..4u64 {
        let spec = PlaneSpec {
            x_dim: 2,
            y_dim: 1,
            index: vec![t, 0, 0],
        };
        let p = f
            .read_plane_in::<f64>(&master, i, &spec, &|| false, &resolve)
            .unwrap();
        let off = t as usize * plane;
        assert_eq!(
            common::mismatches(&p.values, &want[off..off + plane]),
            0,
            "frame {t}"
        );
        let s = f
            .read_plane_strided_in::<f64>(&master, i, &spec, 3, u64::MAX, &|| false, &resolve)
            .unwrap();
        assert_eq!((s.width, s.height), (12, 9));
        assert_eq!(s.values[13], want[off + 3 * 35 + 3]);
    }
    // Without the other files every frame holds the fill value.
    let spec = PlaneSpec {
        x_dim: 2,
        y_dim: 1,
        index: vec![0, 0, 0],
    };
    let p = f.read_plane::<f64>(&master, i, &spec, &|| false).unwrap();
    assert!(p.values.iter().all(|&v| v == 3.0));
}

#[test]
fn emd_files() {
    check_file("berkeley", ".emd");
    check_file("velox", ".emd");
    let Some(dir) = common::fixtures() else {
        return;
    };
    let data = std::fs::read(dir.join("berkeley.emd")).unwrap();
    let f = File::open(&data).unwrap();
    assert_eq!(emd::detect(&f), Some(emd::Convention::Berkeley));
    let im = emd::images(&f, &data);
    assert_eq!(im.len(), 1);
    assert_eq!(im[0].path, "/data/haadf/data");
    assert_eq!((im[0].x_dim, im[0].y_dim), (1, 0));
    assert_eq!(im[0].pixel_size, Some((0.25, 0.25, "nm".to_string())));
    assert_eq!(im[0].instrument.as_deref(), Some("TEAM 0.5"));

    let data = std::fs::read(dir.join("velox.emd")).unwrap();
    let f = File::open(&data).unwrap();
    assert_eq!(emd::detect(&f), Some(emd::Convention::Velox));
    let im = emd::images(&f, &data);
    assert_eq!(im.len(), 1);
    assert_eq!(im[0].pixel_size, Some((2.5e-10, 2.5e-10, "m".to_string())));
    assert_eq!(im[0].pixel_offset, Some((-7e-9, -5e-9)));
    assert_eq!(im[0].detector.as_deref(), Some("HAADF"));
    assert_eq!(im[0].title, "HAADF-A");
    assert_eq!(im[0].instrument.as_deref(), Some("Talos F200X"));
    assert_eq!(im[0].date.as_deref(), Some("2023-11-14T22:13:20"));
    assert_eq!((im[0].x_dim, im[0].y_dim), (1, 0));
}

#[test]
fn netcdf4_files_open_as_hdf5() {
    let Some(dir) = common::fixtures() else {
        return;
    };
    for name in ["nc4.nc", "nc4classic.nc"] {
        let data = std::fs::read(dir.join(name)).unwrap();
        let f = File::open(&data).unwrap();
        assert!(f.warnings.is_empty(), "{:?}", f.warnings);
        let tas = f.get("/tas").unwrap();
        assert!(matches!(tas.attr("DIMENSION_LIST"), Some(Value::Refs(r)) if r.len() == 3));
        let Some(Value::Refs(r)) = tas.attr("DIMENSION_LIST") else {
            unreachable!()
        };
        assert_eq!(f.ref_path(r[1][0]), Some("/lat"));
    }
}

/// Truncated and corrupted copies of the fixtures never panic, whatever is read from them.
#[test]
fn damaged_files() {
    let Some(dir) = common::fixtures() else {
        return;
    };
    for name in ["old.h5", "new.h5", "plugins.h5", "velox.emd"] {
        let data = std::fs::read(dir.join(name)).unwrap();
        let step = (data.len() / 61).max(1);
        for cut in (0..data.len()).step_by(step) {
            exercise(&data[..cut]);
        }
        // Corrupt the metadata-dense start of the file more finely than the rest.
        let mut d = data.clone();
        let fine = data.len().min(8192);
        let points = (0..fine)
            .step_by(53)
            .chain((fine..data.len()).step_by(step));
        for k in points {
            d[k] ^= 0xff;
            exercise(&d);
            d[k] ^= 0xff;
        }
    }
}

fn exercise(data: &[u8]) {
    let Ok(f) = File::open(data) else {
        return;
    };
    let _ = emd::images(&f, data);
    let ids: Vec<(usize, usize)> = f
        .datasets()
        .map(|(i, _, ds)| (i, ds.shape().len()))
        .collect();
    for (i, r) in ids {
        let _ = f.read_all::<f32>(data, i, 1 << 20);
        if r >= 2 {
            let spec = PlaneSpec {
                x_dim: r - 1,
                y_dim: r - 2,
                index: vec![0; r],
            };
            let _ = f.read_plane::<f32>(data, i, &spec, &|| false);
        }
    }
}
