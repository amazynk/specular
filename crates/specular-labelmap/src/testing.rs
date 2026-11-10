//! Minimal NIfTI-1 writing and analytic label volumes, for this crate's tests, the oracle
//! dump and the app's golden shots (not a general writer).

/// How the file states its orientation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Form {
    /// sform only (`qform_code` 0): rows of voxel → RAS.
    Sform([[f64; 4]; 3]),
    /// qform only: quaternion (b, c, d), offsets, `qfac`.
    Qform {
        quat: [f64; 3],
        offset: [f64; 3],
        qfac: f64,
    },
    /// Both, different: readers following nibabel use the sform.
    Both {
        sform: [[f64; 4]; 3],
        quat: [f64; 3],
        offset: [f64; 3],
        qfac: f64,
    },
}

/// The qform's voxel → RAS affine (NIfTI-1's quaternion formula).
pub fn qform_affine(
    quat: [f64; 3],
    offset: [f64; 3],
    pixdim: [f64; 3],
    qfac: f64,
) -> [[f64; 4]; 3] {
    let [b, c, d] = quat;
    let a = (1.0 - b * b - c * c - d * d).max(0.0).sqrt();
    let r = [
        [
            a * a + b * b - c * c - d * d,
            2.0 * (b * c - a * d),
            2.0 * (b * d + a * c),
        ],
        [
            2.0 * (b * c + a * d),
            a * a + c * c - b * b - d * d,
            2.0 * (c * d - a * b),
        ],
        [
            2.0 * (b * d - a * c),
            2.0 * (c * d + a * b),
            a * a + d * d - c * c - b * b,
        ],
    ];
    let z = [pixdim[0], pixdim[1], pixdim[2] * qfac];
    let mut m = [[0.0; 4]; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = r[i][j] * z[j];
        }
        m[i][3] = offset[i];
    }
    m
}

/// The affine a reader uses (sform first).
pub fn ras_affine(form: &Form, pixdim: [f64; 3]) -> [[f64; 4]; 3] {
    match *form {
        Form::Sform(s) | Form::Both { sform: s, .. } => s,
        Form::Qform { quat, offset, qfac } => qform_affine(quat, offset, pixdim, qfac),
    }
}

/// A NIfTI-1 `.nii` file of 16-bit signed labels, x fastest.
pub fn nifti(dims: [usize; 3], pixdim: [f64; 3], form: &Form, data: &[i16]) -> Vec<u8> {
    let mut h = vec![0u8; 352];
    let i16_at = |h: &mut Vec<u8>, o: usize, v: i16| h[o..o + 2].copy_from_slice(&v.to_le_bytes());
    let f32_at =
        |h: &mut Vec<u8>, o: usize, v: f64| h[o..o + 4].copy_from_slice(&(v as f32).to_le_bytes());
    h[0..4].copy_from_slice(&348i32.to_le_bytes());
    i16_at(&mut h, 40, 3);
    for (a, d) in dims.iter().enumerate() {
        i16_at(&mut h, 42 + 2 * a, *d as i16);
    }
    for a in 3..7 {
        i16_at(&mut h, 42 + 2 * a, 1);
    }
    i16_at(&mut h, 70, 4); // DT_INT16
    i16_at(&mut h, 72, 16);
    let qfac = match form {
        Form::Qform { qfac, .. } | Form::Both { qfac, .. } => *qfac,
        Form::Sform(_) => 1.0,
    };
    f32_at(&mut h, 76, qfac);
    for (a, p) in pixdim.iter().enumerate() {
        f32_at(&mut h, 80 + 4 * a, *p);
    }
    f32_at(&mut h, 108, 352.0);
    f32_at(&mut h, 112, 1.0);
    h[123] = 10; // xyzt_units: mm, s
    let (q, s) = match *form {
        Form::Sform(s) => (None, Some(s)),
        Form::Qform { quat, offset, .. } => (Some((quat, offset)), None),
        Form::Both {
            sform,
            quat,
            offset,
            ..
        } => (Some((quat, offset)), Some(sform)),
    };
    if let Some((quat, offset)) = q {
        i16_at(&mut h, 252, 1);
        for a in 0..3 {
            f32_at(&mut h, 256 + 4 * a, quat[a]);
            f32_at(&mut h, 268 + 4 * a, offset[a]);
        }
    }
    if let Some(s) = s {
        i16_at(&mut h, 254, 2);
        for (r, row) in s.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                f32_at(&mut h, 280 + 16 * r + 4 * c, *v);
            }
        }
    }
    h[344..348].copy_from_slice(b"n+1\0");
    for v in data {
        h.extend_from_slice(&v.to_le_bytes());
    }
    h
}

/// The affine with its values rounded to f32, as the file stores them.
pub fn as_stored(m: [[f64; 4]; 3]) -> [[f64; 4]; 3] {
    m.map(|r| r.map(|v| f64::from(v as f32)))
}

/// Voxel (i, j, k) → RAS under `m`.
pub fn apply(m: &[[f64; 4]; 3], v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2] + m[r][3])
}

/// Labels of spheres in RAS world space (centre, radius, label), sampled at each voxel centre
/// of a `dims` grid under `m` (later spheres over earlier).
pub fn spheres(dims: [usize; 3], m: &[[f64; 4]; 3], balls: &[([f64; 3], f64, i16)]) -> Vec<i16> {
    let mut out = vec![0i16; dims[0] * dims[1] * dims[2]];
    for k in 0..dims[2] {
        for j in 0..dims[1] {
            for i in 0..dims[0] {
                let p = apply(m, [i as f64, j as f64, k as f64]);
                for (c, r, l) in balls {
                    let d = ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2))
                        .sqrt();
                    if d <= *r {
                        out[(k * dims[1] + j) * dims[0] + i] = *l;
                    }
                }
            }
        }
    }
    out
}
