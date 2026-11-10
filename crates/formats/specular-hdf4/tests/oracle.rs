//! The fixtures against what pyhdf (the HDF4 C library) reads from them: `tests/data/expected.tsv`,
//! written by `gen/oracle.py expected`.

use std::collections::HashMap;
use std::path::PathBuf;

use specular_hdf4::{Ctx, File, Value};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn unesc(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn trim(s: &str) -> &str {
    s.trim_end_matches(['\n', '\0', ' '])
}

fn nums(s: &str) -> Vec<f64> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split(' ').map(|x| x.parse::<f64>().unwrap()).collect()
}

fn mismatches(got: &[f64], exp: &[f64], tol: f64) -> usize {
    if got.len() != exp.len() {
        return exp.len().max(1);
    }
    got.iter()
        .zip(exp)
        .filter(|&(g, e)| !((g - e).abs() <= tol || (g.is_nan() && e.is_nan())))
        .count()
}

#[test]
fn fixtures_match_the_c_library() {
    let dir = data_dir();
    let tsv = std::fs::read_to_string(dir.join("expected.tsv")).unwrap();
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    let mut opened: HashMap<String, File> = HashMap::new();
    let ext_dir = dir.clone();
    let ext = move |name: &str, off: u64, len: u64| -> Option<Vec<u8>> {
        let b = std::fs::read(ext_dir.join(name)).ok()?;
        Some(b.get(off as usize..(off + len) as usize)?.to_vec())
    };
    let (mut items, mut bad_items, mut bad_values) = (0, 0, 0);
    for line in tsv.lines() {
        let c: Vec<&str> = line.split('\t').collect();
        let (file, kind, name, vals) = (c[0], c[1], c[2], c[5]);
        let data = files
            .entry(file.into())
            .or_insert_with(|| std::fs::read(dir.join(file)).unwrap());
        let f = opened
            .entry(file.into())
            .or_insert_with(|| File::open(data).unwrap());
        let ctx = Ctx::with_external(data, &ext);
        items += 1;
        let text = |v: &Value| v.text().map(|t| trim(&t).to_owned());
        let bad = match kind {
            "sds" => {
                let s = f
                    .sds_by_name(name)
                    .unwrap_or_else(|| panic!("{file}: no {name}"));
                let got = f.read_sds_all(ctx, s, 1 << 24).unwrap();
                mismatches(&got, &nums(vals), 0.0)
            }
            "sattr" | "gattr" => {
                let v = if kind == "sattr" {
                    let (s, a) = name.split_once('/').unwrap();
                    f.sds_by_name(s).and_then(|s| s.attr(a))
                } else {
                    f.attr(name)
                };
                let v = v.unwrap_or_else(|| panic!("{file}: no attribute {name}"));
                match vals.strip_prefix("s:") {
                    Some(t) => usize::from(text(v).as_deref() != Some(trim(&unesc(t)))),
                    None => mismatches(&v.as_f64s().unwrap(), &nums(vals), 0.0),
                }
            }
            "ri" => {
                let im = f.images.iter().find(|i| i.name == name).unwrap();
                let got = f.read_image(ctx, im).unwrap().values;
                let tol = if im.is_jpeg() { 2.0 } else { 0.0 };
                mismatches(&got, &nums(vals), tol)
            }
            "vdata" => {
                let (v, field) = name.split_once('/').unwrap();
                let vd = f.vdatas.iter().find(|d| d.name == v).unwrap();
                let k = vd
                    .fields
                    .iter()
                    .position(|x| x.name == field)
                    .unwrap_or_else(|| panic!("{name}: fields {:?}", vd.fields));
                let bytes = f.vdata_bytes(ctx, vd).unwrap();
                let got = vd.field_values(&bytes, k).unwrap();
                match vals.strip_prefix("s:") {
                    Some(t) => {
                        let s: String = got.iter().map(|&x| x as u8 as char).collect();
                        usize::from(s != unesc(t))
                    }
                    None => mismatches(&got, &nums(vals), 0.0),
                }
            }
            "vattr" => {
                let (v, a) = name.split_once('/').unwrap();
                let vd = f.vdatas.iter().find(|d| d.name == v).unwrap();
                let attrs = f.vdata_attrs(ctx, vd);
                let at = attrs.iter().find(|x| x.name == a).unwrap();
                let t = vals.strip_prefix("s:").unwrap();
                usize::from(text(&at.value).as_deref() != Some(trim(&unesc(t))))
            }
            k => panic!("unknown kind {k}"),
        };
        if bad > 0 {
            eprintln!("MISMATCH {file} {kind} {name}: {bad}");
            bad_items += 1;
            bad_values += bad;
        }
    }
    assert!(items > 150, "{items} items");
    assert_eq!((bad_items, bad_values), (0, 0));
}

#[test]
fn external_elements_need_a_reader() {
    let data = std::fs::read(data_dir().join("sds_ext.hdf")).unwrap();
    let f = File::open(&data).unwrap();
    let s = f.sds_by_name("external_i32").unwrap();
    assert!(f.read_sds_all(Ctx::new(&data), s, 1 << 20).is_err());
    let refuse = |_: &str, _: u64, _: u64| -> Option<Vec<u8>> { None };
    assert!(
        f.read_sds_all(Ctx::with_external(&data, &refuse), s, 1 << 20)
            .is_err()
    );
}

#[test]
fn hdf_eos_grid_and_swath() {
    let data = std::fs::read(data_dir().join("eos_grid.hdf")).unwrap();
    let f = File::open(&data).unwrap();
    let eos = f.eos.as_ref().unwrap();
    assert_eq!(eos.version.as_deref(), Some("HDFEOS_V2.19"));
    let g = &eos.grids[0];
    assert_eq!(g.name, "MOD_Grid_500m_Surface_Reflectance");
    assert_eq!((g.xdim, g.ydim), (48, 40));
    assert_eq!(g.proj_code, Some(specular_hdf4::eos::gctp::SNSOID));
    let (dx, dy) = g.pixel_size().unwrap();
    assert!((dx - 23165.64).abs() < 0.01 && (dy + 23165.64).abs() < 0.01);
    assert_eq!(g.fields.len(), 3);
    assert!(g.fields.iter().all(|x| x.sds.is_some()));
    assert!(eos.short_name().is_some());
    assert!(eos.begin().is_some());

    let data = std::fs::read(data_dir().join("eos_swath.hdf")).unwrap();
    let f = File::open(&data).unwrap();
    let s = &f.eos.as_ref().unwrap().swaths[0];
    assert_eq!(s.name, "Swath_Test");
    assert_eq!(s.maps.len(), 2);
    assert!(s.geo_field("latitude").and_then(|x| x.sds).is_some());
    let m = s.map("GeoXtrack", "DataXtrack").unwrap();
    assert_eq!(m.geo_index(5.0), 2.0);

    let data = std::fs::read(data_dir().join("eos_proj.hdf")).unwrap();
    let f = File::open(&data).unwrap();
    let names: Vec<&str> = f
        .eos
        .as_ref()
        .unwrap()
        .grids
        .iter()
        .map(|g| g.name.as_str())
        .collect();
    assert_eq!(names.len(), 9, "{names:?}");
}

#[test]
fn refuses_what_is_not_hdf4() {
    assert!(File::open(b"\x89HDF\r\n\x1a\n").is_err());
    assert!(File::open(&[0x0e, 0x03, 0x13, 0x01]).is_err());
    let mut b = vec![0x0e, 0x03, 0x13, 0x01];
    b.extend_from_slice(&[0, 10, 0, 0, 0, 0]); // a DD block of 10 DDs past the end
    assert!(File::open(&b).is_err());
}
