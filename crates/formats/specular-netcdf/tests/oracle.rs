//! Every numeric variable of netCDF4-python-written files (classic CDF-1, CDF-2, CDF-5,
//! NetCDF-4 and NetCDF-4 classic model), read whole and plane by plane, against the values
//! the Python bindings give with masking and scaling on, bit for bit.

#[path = "../../specular-hdf5/tests/common/mod.rs"]
mod common;

use specular_netcdf::{Format, Netcdf, PlaneSpec, Value};

fn check(name: &str, format: Format) {
    let Some(dir) = common::fixtures() else {
        return;
    };
    let Ok(data) = std::fs::read(dir.join(format!("{name}.nc"))) else {
        eprintln!("{name}.nc not generated (no netCDF4 module), skipped");
        return;
    };
    let nc = Netcdf::open(&data, &|| false).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(nc.format, format, "{name}");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap();
    let mut checked = 0;
    for (path, shape, want) in common::expected(dir, name) {
        let i = nc
            .vars
            .iter()
            .position(|v| v.name == path)
            .unwrap_or_else(|| panic!("{name}: {path} not found"));
        let v = &nc.vars[i];
        assert_eq!(v.shape, shape, "{name}: {path} shape");
        let dims: Vec<String> = manifest["datasets"][&path]["dims"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(v.dims, dims, "{name}: {path} dims");
        let got = nc.read_all(&data, i, u64::MAX).unwrap();
        assert_eq!(common::mismatches(&got, &want), 0, "{name}: {path}");
        if shape.len() >= 2 {
            let r = shape.len();
            let mut index = vec![0u64; r];
            for d in 0..r - 2 {
                index[d] = shape[d] - 1;
            }
            let spec = PlaneSpec {
                x_dim: r - 1,
                y_dim: r - 2,
                index,
            };
            let p = nc.read_plane(&data, i, &spec, &|| false).unwrap();
            assert!(!p.truncated);
            let off = want.len() - p.values.len();
            assert_eq!(
                common::mismatches(&p.values, &want[off..]),
                0,
                "{name}: {path} plane"
            );
            // Every n-th sample (the thumbnail path).
            let (w, h) = (p.width, p.height);
            let want = &want;
            for step in [2usize, 3, 5] {
                let s = nc
                    .read_plane_strided(&data, i, &spec, step as u64, u64::MAX, &|| false)
                    .unwrap();
                assert!(!s.truncated);
                assert_eq!((s.width, s.height), (w.div_ceil(step), h.div_ceil(step)));
                let sub: Vec<f64> = (0..h)
                    .step_by(step)
                    .flat_map(|y| (0..w).step_by(step).map(move |x| want[off + y * w + x]))
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
    assert!(checked >= 5, "{name}: {checked}");
    // Attributes.
    let attrs = &manifest["attrs"];
    assert_eq!(
        nc.attr("title").and_then(Value::as_str),
        attrs["/"]["title"].as_str(),
        "{name}"
    );
    let tas = nc.var("tas").unwrap();
    assert_eq!(tas.text("units"), Some("K"));
    assert_eq!(tas.packing.scale, Some(0.01));
    assert!(tas.attr("DIMENSION_LIST").is_none());
    let time = nc.dims.iter().find(|d| d.name == "time").unwrap();
    assert!(time.unlimited && time.len == 3, "{name}: {time:?}");
    let (_, lat) = nc.coordinates(&data, "lat").unwrap();
    assert_eq!(lat.len(), 18);
    assert_eq!(lat[0], -85.0);
}

#[test]
fn cdf1() {
    check("cdf1", Format::Classic);
}

#[test]
fn cdf2() {
    check("cdf2", Format::Offset64);
}

#[test]
fn cdf5() {
    check("cdf5", Format::Data64);
}

#[test]
fn netcdf4() {
    check("nc4", Format::Netcdf4);
}

#[test]
fn netcdf4_classic_model() {
    check("nc4classic", Format::Netcdf4);
}

/// Truncated and corrupted copies never panic.
#[test]
fn damaged_files() {
    let Some(dir) = common::fixtures() else {
        return;
    };
    for name in ["cdf1", "cdf5", "nc4"] {
        let Ok(data) = std::fs::read(dir.join(format!("{name}.nc"))) else {
            continue;
        };
        let step = (data.len() / 97).max(1);
        for cut in (0..data.len()).step_by(step) {
            exercise(&data[..cut]);
        }
        let mut d = data.clone();
        for k in (0..d.len()).step_by(step) {
            d[k] ^= 0x5a;
            exercise(&d);
            d[k] ^= 0x5a;
        }
    }
}

fn exercise(data: &[u8]) {
    let Ok(nc) = Netcdf::open(data, &|| false) else {
        return;
    };
    for (i, v) in nc.vars.iter().enumerate() {
        let _ = nc.read_all(data, i, 1 << 20);
        if v.shape.len() >= 2 {
            let r = v.shape.len();
            let spec = PlaneSpec {
                x_dim: r - 1,
                y_dim: r - 2,
                index: vec![0; r],
            };
            let _ = nc.read_plane(data, i, &spec, &|| false);
        }
    }
}
