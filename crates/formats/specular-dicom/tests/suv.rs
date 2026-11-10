//! SUV factors and SUVmax of the corpus PET series against tests/data/suv_oracle.tsv
//! (gen/suv_oracle.py, pydicom), plus TMTV on it. Skips when the corpus is absent.

use std::path::{Path, PathBuf};

use specular_dicom::suv::{Kind, Suv};
use specular_dicom::tmtv::{self, Region, Threshold};
use specular_dicom::volume::{Volume, VoxelKind};
use specular_dicom::{Dicom, Geometry};

fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_DICOM_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/dicom")
        });
    p.join("tcia-sts/pt").is_dir().then_some(p)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1e-12)
}

#[test]
fn factors_and_max_match_oracle() {
    let Some(root) = corpus() else {
        eprintln!("TCIA PET absent; skipping");
        return;
    };
    let oracle = include_str!("data/suv_oracle.tsv");
    for line in oracle.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        if f[0] == "series" {
            let want: f64 = f[1].parse().unwrap();
            let file = std::fs::read(root.join(f[2])).unwrap();
            let d = Dicom::parse(&file).unwrap();
            let s = Suv::read(&d, &file, None).unwrap();
            let m = d.modality_frame(&file, 0, None).unwrap();
            let (x, y): (u32, u32) = (f[3].parse().unwrap(), f[4].parse().unwrap());
            let got = m.value_at(x, y).unwrap() * s.bw;
            assert!(
                (got - want).abs() < 1e-6 * want,
                "SUVmax {got}, oracle {want}"
            );
            continue;
        }
        let file = std::fs::read(root.join(f[0])).unwrap();
        let d = Dicom::parse(&file).unwrap();
        let s = Suv::read(&d, &file, None).unwrap();
        let want: Vec<f64> = f[1..5].iter().map(|x| x.parse().unwrap()).collect();
        assert!(
            close(s.bw, want[0]),
            "{}: SUVbw {} vs {}",
            f[0],
            s.bw,
            want[0]
        );
        assert!(
            close(s.factor(Kind::Lbm).unwrap(), want[1]),
            "{}: SUVlbm",
            f[0]
        );
        assert!(
            close(s.factor(Kind::Bsa).unwrap(), want[2]),
            "{}: SUVbsa",
            f[0]
        );
        assert!(
            (s.delay_s - want[3]).abs() < 0.5,
            "{}: delay {}",
            f[0],
            s.delay_s
        );
    }
}

#[test]
fn ct_is_refused() {
    let Some(root) = corpus() else {
        eprintln!("TCIA PET absent; skipping");
        return;
    };
    let ct = std::fs::read_dir(root.join("tcia-sts/ct"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let file = std::fs::read(ct).unwrap();
    let d = Dicom::parse(&file).unwrap();
    assert_eq!(
        Suv::read(&d, &file, None).unwrap_err(),
        specular_dicom::suv::Refusal::NotPet
    );
}

#[test]
fn tmtv_over_the_series() {
    let Some(root) = corpus() else {
        eprintln!("TCIA PET absent; skipping");
        return;
    };
    // Stack the PET by position (the oracle's hottest voxel is in slice file 180).
    let mut slices: Vec<(f64, PathBuf)> = Vec::new();
    let mut first: Option<(Dicom, Vec<u8>)> = None;
    for e in std::fs::read_dir(root.join("tcia-sts/pt")).unwrap() {
        let p = e.unwrap().path();
        let file = std::fs::read(&p).unwrap();
        let d = Dicom::parse(&file).unwrap();
        let v = d.values(&file);
        let z = v.number(d.dataset.get(0x0020_0032).unwrap(), 2).unwrap();
        slices.push((z, p));
        if first.is_none() {
            first = Some((d, file));
        }
    }
    slices.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (d0, f0) = first.unwrap();
    let s = Suv::read(&d0, &f0, None).unwrap();
    let mut positions = Vec::new();
    let mut frames = Vec::new();
    for (_, p) in &slices {
        let file = std::fs::read(p).unwrap();
        let d = Dicom::parse(&file).unwrap();
        let v = d.values(&file);
        let e = d.dataset.get(0x0020_0032).unwrap();
        positions.push([0, 1, 2].map(|i| v.number(e, i).unwrap()));
        frames.push(d.modality_frame(&file, 0, None).unwrap());
    }
    let g = Geometry::new(
        &positions,
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [5.46875, 5.46875],
        [128, 128],
    );
    let mut vol = Volume::new(g, VoxelKind::of(&frames[0]), 1).unwrap();
    for (k, f) in frames.iter().enumerate() {
        vol.put_frame(k, f).unwrap();
    }
    let c = vol.geometry.centre();
    let big = Region::Sphere {
        centre: c,
        radius: 2000.0,
    };
    let t = tmtv::measure(&vol, s.bw, &big, Threshold::Suv(2.5)).unwrap();
    let want_max: f64 = include_str!("data/suv_oracle.tsv")
        .lines()
        .find(|l| l.starts_with("series"))
        .unwrap()
        .split('\t')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (t.suv_max - want_max).abs() < 1e-4 * want_max,
        "{} vs {want_max}",
        t.suv_max
    );
    let rel = tmtv::measure(&vol, s.bw, &big, Threshold::OfMax(0.41)).unwrap();
    assert!(rel.voxels <= t.voxels);
    eprintln!(
        "TCIA STS_010 whole body: SUV≥2.5 {:.1} mL in {} groups; ≥41% max {:.1} mL in {} groups; SUVmax {:.2}",
        t.ml, t.groups, rel.ml, rel.groups, t.suv_max
    );
}
