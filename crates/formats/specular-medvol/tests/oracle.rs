//! The samples in tests/data against nibabel's, pynrrd's and SimpleITK's reading of them
//! (tests/data/expected.tsv, written by gen/make_samples.py, and expected-mgh-minc.tsv,
//! written by gen/make_mgh_minc.py, and expected-indct.tsv, written by gen/make_indct.py from
//! txrm2tiff, PIL, tifffile, numpy and its own Amira reader, and expected-parrec-ecat.tsv,
//! written by gen/make_parrec_ecat.py from nibabel): every voxel value, patient
//! (LPS) positions of the corners, the orientation source, and the refusals.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specular_medvol::{DataType, MedVolume, OrientationSource, open_path};

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

#[derive(Default)]
struct Case {
    kind: String,
    dtype: String,
    orient: String,
    volumes: usize,
    geom: Vec<([usize; 3], [f64; 3])>,
    vals: Vec<([usize; 5], f64)>,
}

fn expected() -> (BTreeMap<String, Case>, Vec<(String, String)>) {
    let mut text = std::fs::read_to_string(data().join("expected.tsv")).expect("expected.tsv");
    text += &std::fs::read_to_string(data().join("expected-mgh-minc.tsv"))
        .expect("expected-mgh-minc.tsv");
    text +=
        &std::fs::read_to_string(data().join("expected-indct.tsv")).expect("expected-indct.tsv");
    text += &std::fs::read_to_string(data().join("expected-parrec-ecat.tsv"))
        .expect("expected-parrec-ecat.tsv");
    let mut cases: BTreeMap<String, Case> = BTreeMap::new();
    let mut errors = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let n = |i: usize| f[i].parse::<usize>().unwrap();
        let x = |i: usize| f[i].parse::<f64>().unwrap();
        match f[0] {
            "file" => {
                let c = cases.entry(f[1].to_owned()).or_default();
                c.kind = f[2].to_owned();
                c.dtype = f[3].to_owned();
                c.orient = f[4].to_owned();
                c.volumes = n(5);
            }
            "geom" => cases
                .entry(f[1].to_owned())
                .or_default()
                .geom
                .push(([n(2), n(3), n(4)], [x(5), x(6), x(7)])),
            "val" => cases
                .entry(f[1].to_owned())
                .or_default()
                .vals
                .push(([n(2), n(3), n(4), n(5), n(6)], x(7))),
            "error" => errors.push((f[1].to_owned(), f[2].to_owned())),
            t => panic!("unknown row {t}"),
        }
    }
    (cases, errors)
}

fn close(a: f64, b: f64, what: &str) {
    if a.is_nan() && b.is_nan() {
        return;
    }
    let tol = 1e-5 * b.abs().max(1.0);
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b}");
}

fn dtype_name(t: DataType) -> &'static str {
    t.name()
}

#[test]
fn every_sample_matches_the_oracle() {
    let (cases, _) = expected();
    assert!(cases.len() >= 35, "only {} cases", cases.len());
    for (name, c) in &cases {
        let v: MedVolume = open_path(&data().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let h = &v.header;
        assert_eq!(h.kind.name(), c.kind, "{name}: kind");
        assert_eq!(dtype_name(h.dtype), c.dtype, "{name}: type");
        assert_eq!(h.orientation.key(), c.orient, "{name}: orientation");
        assert_eq!(v.volumes(), c.volumes, "{name}: volumes");
        for (ijk, p) in &c.geom {
            let q = h.to_patient(ijk.map(|x| x as f64));
            for a in 0..3 {
                close(q[a], p[a], &format!("{name}: voxel {ijk:?} axis {a}"));
            }
        }
        assert!(!c.vals.is_empty(), "{name}: no values");
        for ([i, j, k, vol, comp], want) in &c.vals {
            let got = v.value(*i, *j, *k, *vol, *comp);
            close(
                got,
                *want,
                &format!("{name}: value at {:?}", (i, j, k, vol, comp)),
            );
        }
    }
}

#[test]
fn refusals_say_why() {
    let (_, errors) = expected();
    assert!(!errors.is_empty());
    for (name, words) in errors {
        let e = open_path(&data().join(&name)).expect_err(&name);
        let msg = e.to_string();
        assert!(msg.contains(&words), "{name}: {msg:?} lacks {words:?}");
    }
}

/// The canonical (displayed) order of every sample: each canonical voxel is the stored voxel
/// at the same patient position, and the axes point as DICOM's convention has them
/// (columns toward the left, rows toward the back, slices toward the head; a lone coronal
/// slice with rows toward the feet).
#[test]
fn canonical_order_follows_dicom_convention() {
    let (cases, _) = expected();
    for name in cases.keys() {
        let v = open_path(&data().join(name)).unwrap();
        let c = v.canonical();
        let h = &v.header;
        for k in 0..c.dims[2] {
            for j in 0..c.dims[1] {
                for i in 0..c.dims[0] {
                    let s = c.stored([i, j, k]);
                    let want = h.to_patient(s.map(|x| x as f64));
                    let got = c.position(i as f64, j as f64, k as f64);
                    if !c.skewed {
                        for a in 0..3 {
                            close(got[a], want[a], &format!("{name}: canonical {i},{j},{k}"));
                        }
                    }
                }
            }
        }
        if c.dims[2] > 1 {
            // A stack: as near axial as permuting and flipping can make it.
            assert!(c.row[0] > 0.0, "{name}: row {:?}", c.row);
            assert!(c.col[1] > 0.0, "{name}: col {:?}", c.col);
            assert!(c.step[2] > 0.0, "{name}: step {:?}", c.step);
        }
        // The plane and the per-voxel reader agree.
        let p = v.plane_f32(&c, c.dims[2] / 2, 0);
        assert_eq!(p.len(), c.dims[0] * c.dims[1]);
        if !h.is_colour() {
            for j in 0..c.dims[1] {
                for i in 0..c.dims[0] {
                    let s = c.stored([i, j, c.dims[2] / 2]);
                    let want = v.value(s[0], s[1], s[2], 0, 0);
                    close(
                        f64::from(p[j * c.dims[0] + i]),
                        want,
                        &format!("{name}: plane"),
                    );
                }
            }
        }
    }
}

#[test]
fn sagittal_stack_is_shown_axial() {
    // Stored (y, z, x): x is the third stored axis, z the second.
    let v = open_path(&data().join("sagittal.nii")).unwrap();
    let c = v.canonical();
    assert_eq!(c.perm, [2, 0, 1]);
    assert_eq!(c.dims, [3, 5, 4]);
    // RAS x became LPS -x: the columns are flipped.
    assert_eq!(c.flip, [true, true, false]);
}

#[test]
fn coronal_slice_stays_coronal() {
    let v = open_path(&data().join("coronal_slice.nii")).unwrap();
    let c = v.canonical();
    assert_eq!(c.dims[2], 1);
    // Columns toward the patient's left, rows toward the feet.
    assert!(c.row[0] > 0.99, "{:?}", c.row);
    assert!(c.col[2] < -0.99, "{:?}", c.col);
}

#[test]
fn nifti_header_facts() {
    let v = open_path(&data().join("scaled.nii")).unwrap();
    let h = &v.header;
    assert_eq!(h.scale, Some((0.25, -100.0)));
    assert_eq!(h.cal, Some((-50.0, 150.0)));
    assert_eq!(v.default_window(), (50.0, 200.0));
    assert_eq!(h.description.as_deref(), Some("scaled sample"));
    assert_eq!(h.intent.as_deref(), Some("z-score (activation)"));
    let a = open_path(&data().join("analyze.hdr")).unwrap();
    assert_eq!(a.header.orientation, OrientationSource::Analyze);
    assert!(!a.header.orientation.reliable());
    assert!(v.header.orientation.reliable());
    let rgb = open_path(&data().join("rgb.nii")).unwrap();
    assert!(rgb.header.is_colour());
    let c = rgb.canonical();
    assert!(
        matches!(rgb.plane(&c, 0, 0), specular_medvol::Plane::Rgb8(p) if p.len() == c.dims[0] * c.dims[1] * 3)
    );
}

#[test]
fn sniffing() {
    use specular_medvol::{Kind, sniff};
    let read = |n: &str| std::fs::read(data().join(n)).unwrap();
    assert_eq!(sniff(&read("dt_int16.nii")), Some(Kind::Nifti1));
    assert_eq!(sniff(&read("gzipped.nii.gz")), Some(Kind::Nifti1));
    assert_eq!(sniff(&read("nifti2.nii")[..64]), Some(Kind::Nifti2));
    assert_eq!(sniff(&read("pair.hdr")), Some(Kind::Nifti1));
    assert_eq!(sniff(&read("analyze.hdr")), Some(Kind::Analyze));
    assert_eq!(sniff(&read("enc_raw.nrrd")), Some(Kind::Nrrd));
    assert_eq!(sniff(&read("short.mha")), Some(Kind::MetaImage));
    assert_eq!(sniff(&read("meta_detached.mhd")), Some(Kind::MetaImage));
    assert_eq!(sniff(b"\x89PNG\r\n\x1a\n"), None);
    assert_eq!(sniff(&[0u8; 400]), None);
}

#[test]
fn parrec_opens_from_either_file() {
    let par = open_path(&data().join("par_tra.PAR")).unwrap();
    let rec = open_path(&data().join("par_tra.REC")).unwrap();
    assert_eq!(par.header, rec.header);
    assert_eq!(par.value(4, 3, 2, 1, 0), rec.value(4, 3, 2, 1, 0));
    assert!(par.header.orientation.reliable());
    let keys: Vec<&str> = par.header.params.iter().map(|p| p.key).collect();
    assert_eq!(keys, ["tr", "te", "flip", "date", "technique", "version"]);
    assert_eq!(par.header.description.as_deref(), Some("par_tra"));
}

#[test]
fn ecat_header_facts() {
    let v = open_path(&data().join("ecat_i16.v")).unwrap();
    let h = &v.header;
    // Scale 0.5 times calibration 2: none left.
    assert_eq!(h.scale, None);
    assert!(!h.orientation.reliable());
    assert_eq!(h.description.as_deref(), Some("brain study"));
    let keys: Vec<&str> = h.params.iter().map(|p| p.key).collect();
    assert_eq!(keys, ["isotope", "tracer", "units", "date"]);
    use specular_medvol::{Kind, sniff};
    assert_eq!(
        sniff(&std::fs::read(data().join("ecat_u8.v")).unwrap()),
        Some(Kind::Ecat)
    );
    assert_eq!(
        sniff(&std::fs::read(data().join("par_sag.PAR")).unwrap()),
        Some(Kind::ParRec)
    );
}
