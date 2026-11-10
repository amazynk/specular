//! Label volumes over a series whose expected overlay is analytic: spheres in world space,
//! sampled into label files of every orientation kind, read back over an oblique series.

use std::path::Path;

use specular_dicom::volume::Geometry;

use super::testing::{Form, apply, as_stored, nifti, ras_affine, spheres};
use super::*;

/// A rotation about x by `ax` then z by `az` (degrees), columns scaled by `sp`.
pub(crate) fn rotated(ax: f64, az: f64, sp: [f64; 3], t: [f64; 3]) -> [[f64; 4]; 3] {
    let (a, b) = (ax.to_radians(), az.to_radians());
    let rx = [
        [1.0, 0.0, 0.0],
        [0.0, a.cos(), -a.sin()],
        [0.0, a.sin(), a.cos()],
    ];
    let rz = [
        [b.cos(), -b.sin(), 0.0],
        [b.sin(), b.cos(), 0.0],
        [0.0, 0.0, 1.0],
    ];
    let mut m = [[0.0; 4]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let r: f64 = (0..3).map(|k| rz[i][k] * rx[k][j]).sum();
            m[i][j] = r * sp[j];
        }
        m[i][3] = t[i];
    }
    m
}

/// Spheres (RAS centre, radius mm, label) inside every case's field of view.
pub(crate) const BALLS: [([f64; 3], f64, i16); 3] = [
    ([0.0, 0.0, 0.0], 9.0, 1),
    ([12.0, -6.0, 4.0], 6.0, 7),
    ([-8.0, 10.0, -5.0], 5.0, 300),
];

pub(crate) struct Case {
    pub name: &'static str,
    pub dims: [usize; 3],
    pub pixdim: [f64; 3],
    pub form: Form,
}

pub(crate) fn cases() -> Vec<Case> {
    let dims = [48, 44, 30];
    let pixdim = [1.0, 1.1, 1.4];
    // Centred so the field of view holds the spheres.
    let centre_in = |m: [[f64; 4]; 3], dims: [usize; 3]| {
        let mut m = m;
        let c = apply(
            &m,
            [
                dims[0] as f64 / 2.0,
                dims[1] as f64 / 2.0,
                dims[2] as f64 / 2.0,
            ],
        );
        for r in 0..3 {
            m[r][3] -= c[r];
        }
        m
    };
    let centre = |m| centre_in(m, dims);
    let oblique = as_stored(centre(rotated(20.0, 30.0, pixdim, [0.0; 3])));
    let mut sheared = centre(rotated(0.0, 10.0, pixdim, [0.0; 3]));
    // y gains a share of k: the axes are no longer perpendicular.
    sheared[1][2] += 0.45;
    let sheared = as_stored(centre(sheared));
    // LPI-like: x and y reversed, z down, axes permuted (j along z).
    let permuted = as_stored(centre_in(
        [
            [-pixdim[0], 0.0, 0.0, 0.0],
            [0.0, 0.0, -pixdim[2], 0.0],
            [0.0, -pixdim[1], 0.0, 0.0],
        ],
        [48, 30, 44],
    ));
    // A qform with qfac −1 (left-handed voxel axes), rotated about an oblique axis.
    let (qb, qc, qd) = (0.12f64, -0.2, 0.3);
    let qoff = {
        let m = super::testing::qform_affine([qb, qc, qd], [0.0; 3], pixdim, -1.0);
        let c = apply(
            &m,
            [
                dims[0] as f64 / 2.0,
                dims[1] as f64 / 2.0,
                dims[2] as f64 / 2.0,
            ],
        );
        c.map(|x| f64::from(-x as f32))
    };
    vec![
        Case {
            name: "sform-oblique",
            dims,
            pixdim,
            form: Form::Sform(oblique),
        },
        Case {
            name: "qform-qfac-minus",
            dims,
            pixdim,
            form: Form::Qform {
                quat: [qb, qc, qd],
                offset: qoff,
                qfac: -1.0,
            },
        },
        Case {
            name: "sform-over-qform",
            dims,
            pixdim,
            form: Form::Both {
                sform: oblique,
                quat: [0.5, 0.5, 0.5],
                offset: [40.0, 0.0, 0.0],
                qfac: 1.0,
            },
        },
        Case {
            name: "sform-sheared",
            dims,
            pixdim,
            form: Form::Sform(sheared),
        },
        Case {
            name: "sform-permuted",
            dims: [48, 30, 44],
            pixdim,
            form: Form::Sform(permuted),
        },
    ]
}

/// An oblique "DICOM" series around the origin: rows tilted 15° about z, the slices 10°.
pub(crate) fn series() -> Geometry {
    let (a, b) = (15f64.to_radians(), 10f64.to_radians());
    let row = [a.cos(), a.sin(), 0.0];
    let col0 = [-a.sin(), a.cos(), 0.0];
    let col = [col0[0] * b.cos(), col0[1] * b.cos(), b.sin()];
    let normal = cross(row, col);
    let (w, h, d) = (64u32, 60u32, 40u32);
    let pixel = [0.7, 0.75];
    let spacing = 0.9;
    let start = [0, 1, 2].map(|c| {
        -(row[c] * pixel[0] * f64::from(w)
            + col[c] * pixel[1] * f64::from(h)
            + normal[c] * spacing * f64::from(d))
            / 2.0
    });
    let positions: Vec<[f64; 3]> = (0..d)
        .map(|k| [0, 1, 2].map(|c| start[c] + normal[c] * spacing * f64::from(k)))
        .collect();
    Geometry::new(&positions, row, col, pixel, [w, h])
}

pub(crate) fn load(c: &Case) -> MedVolume {
    let m = ras_affine(&c.form, c.pixdim);
    let data = spheres(c.dims, &m, &BALLS);
    let bytes = nifti(c.dims, c.pixdim, &c.form, &data);
    let path = Path::new("labels.nii");
    let header = specular_medvol::read_header(&bytes, path).unwrap();
    specular_medvol::load(header, std::sync::Arc::new(bytes), path).unwrap()
}

/// The spheres' label at RAS point `p`, when `p` is clearly inside or outside (more than
/// `margin` mm from every surface).
fn analytic(p: [f64; 3], margin: f64) -> Option<i64> {
    let mut v = 0;
    for (c, r, l) in BALLS {
        let d = ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2)).sqrt();
        if (d - r).abs() <= margin {
            return None;
        }
        if d < r {
            v = i64::from(l);
        }
    }
    Some(v)
}

#[test]
fn every_orientation_kind_lines_up_with_an_oblique_series() {
    let g = series();
    for c in cases() {
        let vol = load(&c);
        let l = labels(&vol, None, c.name).unwrap();
        assert_eq!(
            l.segments.iter().map(|s| s.number).collect::<Vec<_>>(),
            [1, 7, 300],
            "{}",
            c.name
        );
        assert!(overlaps(&l, &g));
        // Nearest neighbour can be off by half a source voxel's diagonal (twice that when
        // a sheared grid is resampled first).
        let diag = (c.pixdim.iter().map(|x| x * x).sum::<f64>()).sqrt();
        let margin = if c.name.contains("sheared") {
            diag * 1.2
        } else {
            diag * 0.55
        };
        let (mut checked, mut bad) = ([0usize; 3], 0);
        for k in 0..g.dims[2] {
            for j in 0..g.dims[1] {
                for i in 0..g.dims[0] {
                    let p = g.to_patient([f64::from(i), f64::from(j), f64::from(k)]);
                    // LPS → RAS.
                    let ras = [-p[0], -p[1], p[2]];
                    let Some(want) = analytic(ras, margin) else {
                        continue;
                    };
                    let got = value_at(&l, p);
                    if got != want {
                        bad += 1;
                    }
                    if let Some(s) = [1, 7, 300].iter().position(|&x| x == want) {
                        checked[s] += 1;
                    }
                }
            }
        }
        assert_eq!(bad, 0, "{}", c.name);
        assert!(checked.iter().all(|&n| n > 50), "{} {checked:?}", c.name);
    }
}

#[test]
fn ras_lps_flip_is_not_a_mirror() {
    // The sphere at RAS (12, −6, 4) lies at LPS (−12, 6, 4); its mirror image is empty.
    let c = &cases()[0];
    let l = labels(&load(c), None, "x").unwrap();
    assert_eq!(value_at(&l, [-12.0, 6.0, 4.0]), 7);
    assert_eq!(value_at(&l, [12.0, -6.0, 4.0]), 0);
}

#[test]
fn colour_tables_name_and_colour_segments() {
    let lut = Lut::parse("1 255 0 0 1 1 1 \"Liver\"\n7 0 0 255 1 1 1 \"Kidney\"\n").unwrap();
    let l = labels(&load(&cases()[0]), Some(&lut), "x").unwrap();
    assert_eq!(l.segments[0].label, "Liver");
    assert_eq!(l.segments[1].colour, [0, 0, 255]);
    // Not in the table: no name, a generated colour.
    assert_eq!(l.segments[2].label, "");
    assert_eq!(l.segments[2].colour, palette(300));
    // Volumes in mL from the voxel count.
    let s = &l.segments[0];
    let voxel = 1.0 * 1.1 * 1.4 / 1000.0;
    assert!((s.ml - s.voxels as f64 * voxel).abs() < 1e-6);
    // A sphere of 9 mm: about 3.05 mL.
    assert!(
        (s.ml - 4.0 / 3.0 * std::f64::consts::PI * 729.0 / 1000.0).abs() < 0.2,
        "{}",
        s.ml
    );
    assert_eq!(l.source, LabelSource::Volume);
}

#[test]
fn refusals() {
    let c = &cases()[0];
    let zeros = nifti(c.dims, c.pixdim, &c.form, &vec![0; c.dims.iter().product()]);
    let p = Path::new("z.nii");
    let v = specular_medvol::load(
        specular_medvol::read_header(&zeros, p).unwrap(),
        std::sync::Arc::new(zeros),
        p,
    )
    .unwrap();
    assert_eq!(labels(&v, None, "z").err(), Some(Refusal::Empty));
}

/// Writes what `tools/corpus/pathology-oracle.py` checks against nibabel and SimpleITK:
/// each case's NIfTI file, the oblique series' grid and our label value at every voxel of
/// it. Run with `SPECULAR_LABELMAP_ORACLE=<dir> cargo test -p specular-labelmap -- --ignored`.
#[test]
#[ignore]
fn oracle_dump() {
    let Some(dir) = std::env::var_os("SPECULAR_LABELMAP_ORACLE").map(std::path::PathBuf::from)
    else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let g = series();
    let a = g.affine();
    let mut json = String::from("{\"cases\":[");
    for (n, c) in cases().iter().enumerate() {
        let m = ras_affine(&c.form, c.pixdim);
        let data = spheres(c.dims, &m, &BALLS);
        let file = format!("{}.nii", c.name);
        std::fs::write(dir.join(&file), nifti(c.dims, c.pixdim, &c.form, &data)).unwrap();
        let l = labels(&load(c), None, c.name).unwrap();
        let mut out = Vec::new();
        for k in 0..g.dims[2] {
            for j in 0..g.dims[1] {
                for i in 0..g.dims[0] {
                    let p = g.to_patient([f64::from(i), f64::from(j), f64::from(k)]);
                    out.extend_from_slice(&(value_at(&l, p) as u16).to_le_bytes());
                }
            }
        }
        let ours = format!("{}-ours.u16", c.name);
        std::fs::write(dir.join(&ours), out).unwrap();
        // ITK reads the qform where nibabel reads the sform, and refuses shear: compare it
        // only where both read the same affine.
        let (form, itk) = match c.form {
            Form::Sform(_) if c.name.contains("sheared") => ("sform (sheared)", false),
            Form::Sform(_) => ("sform", true),
            Form::Qform { .. } => ("qform", true),
            Form::Both { .. } => ("sform and a different qform", false),
        };
        if n > 0 {
            json.push(',');
        }
        let flat: Vec<String> = a.iter().flatten().map(|v| format!("{v}")).collect();
        json.push_str(&format!(
            "{{\"name\":\"{}\",\"nifti\":\"{file}\",\"ours\":\"{ours}\",\"nifti_form\":\"{form}\",\"itk\":{itk},\"target_affine\":[{}],\"target_dims\":[{},{},{}]}}",
            c.name,
            flat.join(","),
            g.dims[0],
            g.dims[1],
            g.dims[2]
        ));
    }
    json.push_str("]}");
    std::fs::write(dir.join("labelmap.json"), json).unwrap();
}
