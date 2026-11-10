//! SEG and RTSTRUCT segment volumes against tests/data/labels_oracle.tsv (highdicom decoding
//! for SEG, pydicom contours for RTSTRUCT; gen/labels_oracle.py), on the public corpus
//! (tools/corpus/fetch-dicom.sh). Skips when the corpus is absent.

use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_dicom::labels::{LabelSource, Labels};
use specular_dicom::{Dicom, rtstruct, seg};

fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_DICOM_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/dicom")
        });
    p.is_dir().then_some(p)
}

fn read(path: &Path) -> Labels {
    let file = std::fs::read(path).unwrap();
    let d = Dicom::parse(&file).unwrap();
    let t = Instant::now();
    let l = if path.to_string_lossy().contains("rts") {
        rtstruct::read(&d, &file).unwrap()
    } else {
        seg::read(&d, &file, None).unwrap()
    };
    eprintln!(
        "{}: {} segments, {} layers, {} KiB, {:.1} ms",
        path.display(),
        l.segments.len(),
        l.layers.len(),
        l.bytes() / 1024,
        t.elapsed().as_secs_f64() * 1e3
    );
    l
}

#[test]
fn volumes_match_oracle() {
    let Some(root) = corpus() else {
        eprintln!("DICOM corpus absent; skipping");
        return;
    };
    let oracle = include_str!("data/labels_oracle.tsv");
    let mut checked = 0;
    let mut cache: Option<(String, Labels)> = None;
    for line in oracle.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        let (kind, rel, number, voxels, ml) = (f[0], f[1], f[2], f[3], f[4]);
        let path = root.join(rel);
        if !path.is_file() {
            eprintln!("{rel} absent; skipping");
            continue;
        }
        let number: i64 = number.parse().unwrap();
        if kind == "SEG" && number == 0 {
            // A labelmap's background.
            continue;
        }
        if cache.as_ref().is_none_or(|(r, _)| r != rel) {
            cache = Some((rel.to_owned(), read(&path)));
        }
        let l = &cache.as_ref().unwrap().1;
        let s = l
            .segments
            .iter()
            .find(|s| s.number == number)
            .unwrap_or_else(|| panic!("{rel}: no segment {number}"));
        let want: f64 = ml.parse().unwrap();
        let tol = 1e-6 * want.max(1.0);
        assert!(
            (s.ml - want).abs() <= tol,
            "{rel} segment {number}: {} mL, oracle {want}",
            s.ml
        );
        if kind == "SEG" {
            assert_eq!(l.source, LabelSource::Seg);
            assert_eq!(
                s.voxels,
                voxels.parse::<u64>().unwrap(),
                "{rel} segment {number}"
            );
        } else {
            assert_eq!(l.source, LabelSource::RtStruct);
        }
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} oracle rows checked");
}

#[test]
fn seg_variants_agree() {
    // The same liver mask in every transfer syntax (incl. RLE and deflate of 1-bit frames,
    // which the oracle cannot decode, and big endian).
    let Some(root) = corpus() else {
        eprintln!("DICOM corpus absent; skipping");
        return;
    };
    let dir = root.join("pydicom-data/data_store/data");
    let base = dir.join("liver.dcm");
    if !base.is_file() {
        eprintln!("pydicom-data absent; skipping");
        return;
    }
    let want = read(&base);
    for v in [
        "deflate",
        "expb",
        "rle",
        "nonbyte_aligned_rle",
        "nonbyte_aligned_deflate",
    ] {
        let p = dir.join(format!("liver_{v}.dcm"));
        let l = read(&p);
        assert_eq!(l.segments[0].voxels, want.segments[0].voxels, "{v}");
        if !v.starts_with("nonbyte") {
            assert_eq!(l.layers, want.layers, "{v}");
        }
    }
}

#[test]
fn seg_references_and_colours() {
    let Some(root) = corpus() else {
        eprintln!("DICOM corpus absent; skipping");
        return;
    };
    let p = root.join("dcmqi/data/segmentations/liver.dcm");
    let ct = root.join("dcmqi/data/segmentations/ct-3slice/01.dcm");
    if !p.is_file() || !ct.is_file() {
        eprintln!("dcmqi absent; skipping");
        return;
    }
    let l = read(&p);
    let file = std::fs::read(&ct).unwrap();
    let d = Dicom::parse(&file).unwrap();
    let v = d.values(&file);
    let e = d.dataset.get(0x0020_0052).unwrap();
    let ct_for = std::str::from_utf8(v.text_bytes(e))
        .unwrap()
        .trim_end_matches('\0')
        .trim();
    assert_eq!(l.frame_of_reference, ct_for);
    assert_eq!(l.segments[0].label, "Liver");
    // Recommended Display CIELab of a liver tone: reddish brown.
    let c = l.segments[0].colour;
    assert!(c[0] > c[1] && c[0] > c[2], "{c:?}");
    // The mask lies over the CT: a voxel at the liver's centre of mass is inside.
    let layer = &l.layers[0].volume;
    let g = &layer.geometry;
    let specular_dicom::Voxels::U16(data) = &layer.voxels else {
        panic!("labels are U16")
    };
    let [w, h, _] = g.dims;
    let idx = data.iter().position(|&x| x != 0).unwrap();
    let (i, j, k) = (
        idx % w as usize,
        idx / w as usize % h as usize,
        idx / (w * h) as usize,
    );
    let p = g.to_patient([i as f64, j as f64, k as f64]);
    assert_eq!(l.at(p), vec![0]);
}
