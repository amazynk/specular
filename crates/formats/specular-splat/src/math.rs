//! Covariances from scale and rotation, frame changes and the real spherical-harmonic basis
//! in the 3DGS convention.

/// The zeroth harmonic's constant: colour = 0.5 + `SH_C0` × DC coefficient.
pub const SH_C0: f32 = 0.282_094_8;
const SH_C1: f32 = 0.488_602_5;
const SH_C2: [f32; 5] = [
    1.092_548_4,
    -1.092_548_4,
    0.315_391_57,
    -1.092_548_4,
    0.546_274_2,
];
const SH_C3: [f32; 7] = [
    -0.590_043_6,
    2.890_611_4,
    -0.457_045_8,
    0.373_176_33,
    -0.457_045_8,
    1.445_305_7,
    -0.590_043_6,
];

/// The 15 higher basis functions (degrees 1–3) at the unit direction `d`, in the order the
/// coefficients are stored.
pub fn sh_basis(d: [f32; 3]) -> [f32; 15] {
    let [x, y, z] = d;
    let (xx, yy, zz) = (x * x, y * y, z * z);
    [
        -SH_C1 * y,
        SH_C1 * z,
        -SH_C1 * x,
        SH_C2[0] * x * y,
        SH_C2[1] * y * z,
        SH_C2[2] * (2.0 * zz - xx - yy),
        SH_C2[3] * x * z,
        SH_C2[4] * (xx - yy),
        SH_C3[0] * y * (3.0 * xx - yy),
        SH_C3[1] * x * y * z,
        SH_C3[2] * y * (4.0 * zz - xx - yy),
        SH_C3[3] * z * (2.0 * zz - 3.0 * xx - 3.0 * yy),
        SH_C3[4] * x * (4.0 * zz - xx - yy),
        SH_C3[5] * z * (xx - yy),
        SH_C3[6] * x * (xx - 3.0 * yy),
    ]
}

/// The covariance R S Sᵀ Rᵀ of linear scales `s` and the quaternion `q` (w, x, y, z; any
/// length, normalised here), upper triangle xx, xy, xz, yy, yz, zz.
pub fn covariance(s: [f32; 3], q: [f32; 4]) -> [f32; 6] {
    let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    let [w, x, y, z] = if l > 0.0 && l.is_finite() {
        q.map(|v| v / l)
    } else {
        [1.0, 0.0, 0.0, 0.0]
    };
    let r = [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - w * z),
            2.0 * (x * z + w * y),
        ],
        [
            2.0 * (x * y + w * z),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - w * x),
        ],
        [
            2.0 * (x * z - w * y),
            2.0 * (y * z + w * x),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ];
    // M = R S; Σ = M Mᵀ.
    let m: [[f32; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| r[i][j] * s[j]));
    let dot = |a: usize, b: usize| m[a][0] * m[b][0] + m[a][1] * m[b][1] + m[a][2] * m[b][2];
    [
        dot(0, 0),
        dot(0, 1),
        dot(0, 2),
        dot(1, 1),
        dot(1, 2),
        dot(2, 2),
    ]
}

/// The full symmetric matrix of an upper triangle.
pub(crate) fn full(c: [f32; 6]) -> [[f32; 3]; 3] {
    [[c[0], c[1], c[2]], [c[1], c[3], c[4]], [c[2], c[4], c[5]]]
}

/// M Σ Mᵀ.
pub(crate) fn rotate_cov(m: &[[f32; 3]; 3], c: [f32; 6]) -> [f32; 6] {
    let s = full(c);
    let ms: [[f32; 3]; 3] = std::array::from_fn(|i| {
        std::array::from_fn(|j| m[i][0] * s[0][j] + m[i][1] * s[1][j] + m[i][2] * s[2][j])
    });
    let e = |i: usize, j: usize| ms[i][0] * m[j][0] + ms[i][1] * m[j][1] + ms[i][2] * m[j][2];
    [e(0, 0), e(0, 1), e(0, 2), e(1, 1), e(1, 2), e(2, 2)]
}

pub(crate) fn mul(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    m.map(|r| r[0] * v[0] + r[1] * v[1] + r[2] * v[2])
}

pub(crate) fn transpose(m: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| m[j][i]))
}

/// The logistic function (3DGS stores opacity as its inverse).
pub(crate) fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covariance_of_an_axis_aligned_splat() {
        let c = covariance([1.0, 2.0, 3.0], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(c, [1.0, 0.0, 0.0, 4.0, 0.0, 9.0]);
        // A quarter turn about z swaps the x and y variances.
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let c = covariance([1.0, 2.0, 3.0], [h, 0.0, 0.0, h]);
        assert!((c[0] - 4.0).abs() < 1e-5 && (c[3] - 1.0).abs() < 1e-5);
        assert!((c[5] - 9.0).abs() < 1e-5 && c[1].abs() < 1e-5);
    }

    #[test]
    fn frame_changes_keep_the_shape() {
        let c = covariance([1.0, 2.0, 3.0], [0.9, 0.1, -0.3, 0.2]);
        let m = crate::FileUp::NegY.to_canonical();
        let r = rotate_cov(&m, c);
        // The trace is invariant; y and z swap.
        assert!(((c[0] + c[3] + c[5]) - (r[0] + r[3] + r[5])).abs() < 1e-4);
        assert!((r[3] - c[5]).abs() < 1e-5 && (r[5] - c[3]).abs() < 1e-5);
    }

    #[test]
    fn the_basis_is_orthonormal_on_the_sphere() {
        // Monte Carlo over a Fibonacci sphere: ∫ Yi Yj = δij (for the real basis with
        // these constants and signs).
        let n = 20000;
        let mut g = [[0f64; 15]; 15];
        for k in 0..n {
            let z = 1.0 - 2.0 * (k as f32 + 0.5) / n as f32;
            let r = (1.0 - z * z).sqrt();
            let phi = k as f32 * 2.399_963;
            let b = sh_basis([r * phi.cos(), r * phi.sin(), z]);
            for i in 0..15 {
                for j in 0..15 {
                    g[i][j] += f64::from(b[i] * b[j]);
                }
            }
        }
        let area = 4.0 * std::f64::consts::PI / f64::from(n);
        for (i, row) in g.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!((v * area - want).abs() < 0.01, "{i},{j}: {}", v * area);
            }
        }
    }
}
