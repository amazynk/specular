//! Industrial CT beyond the voxel oracle (tests/oracle.rs reads expected-indct.tsv): the
//! facts each header states, headerless volumes read with a given layout and the layouts
//! guessed for them, sniffing and the sidecar `.vol` / `.raw` files
//! (tests/data/expected-indct-extra.tsv, written by gen/make_indct.py).

use std::path::{Path, PathBuf};

use specular_medvol::raw::{self, RawLayout};
use specular_medvol::{DataType, Error, Kind, OrientationSource, ParamValue, open_path, sniff};

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn rows() -> Vec<Vec<String>> {
    std::fs::read_to_string(data().join("expected-indct-extra.tsv"))
        .expect("expected-indct-extra.tsv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split('\t').map(str::to_owned).collect())
        .collect()
}

fn dtype(s: &str) -> DataType {
    match s {
        "uint8" => DataType::U8,
        "uint16" => DataType::U16,
        "float32" => DataType::F32,
        t => panic!("type {t}"),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-5 * b.abs().max(1.0)
}

#[test]
fn headerless_volumes_read_with_their_layout() {
    let rows = rows();
    let mut values = 0;
    let mut mismatches = 0;
    for r in rows.iter().filter(|r| r[0] == "layout") {
        let name = &r[1];
        let path = data().join(name);
        let n = |i: usize| r[i].parse::<usize>().unwrap();
        let x = |i: usize| r[i].parse::<f64>().unwrap();
        // Refused until a layout is given.
        assert_eq!(open_path(&path).unwrap_err(), Error::NeedsLayout, "{name}");
        assert!(raw::needs_layout(&path), "{name}");
        raw::remember(
            &path,
            RawLayout {
                size: [n(2), n(3), n(4)],
                dtype: dtype(&r[5]),
                big_endian: r[6] == "1",
                offset: r[7].parse().unwrap(),
                spacing: [x(8), x(9), x(10)],
            },
        );
        assert!(!raw::needs_layout(&path), "{name}");
        let v = open_path(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(v.header.kind, Kind::Raw);
        assert_eq!(v.header.orientation, OrientationSource::Object);
        assert_eq!(v.header.spacing, [x(8), x(9), x(10)]);
        for w in rows.iter().filter(|w| w[0] == "val" && &w[1] == name) {
            let i = |k: usize| w[k].parse::<usize>().unwrap();
            let want: f64 = w[7].parse().unwrap();
            values += 1;
            if !close(v.value(i(2), i(3), i(4), i(5), i(6)), want) {
                mismatches += 1;
            }
        }
    }
    eprintln!("raw volumes: {values} values, {mismatches} mismatches against numpy");
    assert!(values > 100);
    assert_eq!(mismatches, 0);
}

#[test]
fn layouts_guessed_from_name_and_size() {
    for r in rows().iter().filter(|r| r[0] == "guess") {
        let len = std::fs::metadata(data().join(&r[1])).unwrap().len();
        let g = raw::guesses(&r[1], len);
        let n = |i: usize| r[i].parse::<usize>().unwrap();
        let first = g.first().unwrap_or_else(|| panic!("{}: no guess", r[1]));
        assert_eq!(first.size, [n(2), n(3), n(4)], "{}", r[1]);
        assert_eq!(first.dtype, dtype(&r[5]), "{}", r[1]);
        assert_eq!(first.offset, r[6].parse::<u64>().unwrap(), "{}", r[1]);
    }
}

#[test]
fn header_facts_match() {
    let mut checked = 0;
    for r in rows().iter().filter(|r| r[0] == "param") {
        let v = open_path(&data().join(&r[1])).unwrap_or_else(|e| panic!("{}: {e}", r[1]));
        let p = v
            .header
            .params
            .iter()
            .find(|p| p.key == r[2])
            .unwrap_or_else(|| panic!("{}: no {} in {:?}", r[1], r[2], v.header.params));
        match &p.value {
            ParamValue::Number(x, _) => {
                let want: f64 = r[3].parse().unwrap();
                assert!(
                    (x - want).abs() <= 1e-4 * want.abs().max(1.0),
                    "{}: {} {x}",
                    r[1],
                    r[2]
                );
            }
            ParamValue::Text(t) => assert_eq!(t, &r[3], "{}: {}", r[1], r[2]),
        }
        checked += 1;
    }
    assert!(checked >= 20, "{checked}");
}

#[test]
fn sniffing_and_sidecars() {
    let read = |n: &str| std::fs::read(data().join(n)).unwrap();
    assert_eq!(sniff(&read("txm_u16.txm")), Some(Kind::Txm));
    assert_eq!(sniff(&read("am_le.am")), Some(Kind::Amira));
    assert_eq!(
        sniff(&read("skyscan_bmp/sample_rec.log")),
        Some(Kind::SkyScan)
    );
    assert_eq!(sniff(&read("xtek_f32.xtekct")), Some(Kind::Xtek));
    assert_eq!(sniff(&read("xtek_u16.xtekVolume")), Some(Kind::Xtek));
    assert_eq!(sniff(&read("vgi_u16.vgi")), Some(Kind::Vgi));
    // The samples alone are nothing: they open through the header beside them.
    assert_eq!(sniff(&read("xtek_f32.vol")), None);
    assert_eq!(sniff(&read("vgi_u16.raw")), None);
    assert!(!raw::needs_layout(&data().join("xtek_f32.vol")));
    assert_eq!(
        raw::sibling_header(&data().join("vgi_u16.raw")),
        Some(data().join("vgi_u16.vgi"))
    );
    // A compound file that is not an Xradia one.
    let mut other = read("txm_u16.txm");
    let at = other
        .windows(20)
        .position(|w| {
            w == "ImageWidth"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        })
        .unwrap();
    other[at] = b'X';
    assert_eq!(sniff(&other), None);
}

#[test]
fn projections_and_reconstructions_told_apart() {
    let content = |n: &str| {
        let v = open_path(&data().join(n)).unwrap();
        v.header
            .params
            .iter()
            .find(|p| p.key == "content")
            .map(|p| p.value.clone())
    };
    let text = |s: &str| Some(ParamValue::Text(s.into()));
    assert_eq!(content("txrm_f32.txrm"), text("projections"));
    assert_eq!(content("txm_u16.txm"), text("reconstruction"));
    assert_eq!(content("txm_many.txm"), text("reconstruction"));
    // A mosaic is one image.
    let m = open_path(&data().join("mosaic.xrm")).unwrap();
    assert_eq!(m.header.layout.size, [15, 8, 1]);
}

/// Open and first-slice times of the samples (debug build; the numbers are only printed).
#[test]
fn timings() {
    for n in [
        "txrm_f32.txrm",
        "txm_many.txm",
        "am_rle.am",
        "am_zip.am",
        "skyscan_png/sample_rec.log",
        "xtek_f32.xtekct",
        "vgi_u16.vgi",
    ] {
        let t = std::time::Instant::now();
        let v = open_path(&data().join(n)).unwrap();
        let open = t.elapsed();
        let c = v.canonical();
        let _ = v.plane_f32(&c, c.dims[2] / 2, 0);
        eprintln!("{n}: open {open:?}, first slice {:?}", t.elapsed() - open);
    }
}
