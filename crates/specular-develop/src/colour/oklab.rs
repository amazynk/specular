//! OkLab (B. Ottosson, "A perceptual color space for image processing", 2020): the
//! perceptual space the colour-grading ops work in, from and to linear Rec.2020.
//!
//! Why OkLab and not JzAzBz:
//! - **Exposure invariance.** Its non-linearity is a cube root, which is homogeneous:
//!   scaling a colour by `k` scales L, a and b by `k^(1/3)`. Hue and the chroma/lightness
//!   ratio (the ops' "saturation") do not change with exposure, so a grade made on one
//!   exposure holds on another, and scene values far above 1 need no absolute-luminance
//!   assumption. JzAzBz runs through the PQ curve, which needs a nits scale for scene data
//!   and bends hue and saturation with exposure.
//! - **Hue linearity.** Its hue lines were fitted to keep blue from drifting to purple,
//!   the main fault of CIELAB in grading.
//! - **Cost.** Two 3×3 products and three cube roots per conversion; JzAzBz adds two PQ
//!   curves (six `pow`).
//!
//! The matrices are Ottosson's published M1 (XYZ D65 → LMS) and M2 (LMS' → Lab); the
//! conversion is checked against colour-science (`tests/oklab_oracle.rs`). Negative
//! (out-of-gamut) values take a signed cube root, so the conversion is odd and invertible
//! everywhere.

use std::sync::LazyLock;

use super::{M3, inv, mul, mulv, rec2020_to_xyz, xyz_to_rec2020};

/// XYZ (D65) → LMS (Ottosson 2020).
pub const M1: M3 = [
    [0.818_933_010_1, 0.361_866_742_4, -0.128_859_713_7],
    [0.032_984_543_6, 0.929_311_871_5, 0.036_145_638_7],
    [0.048_200_301_8, 0.264_366_269_1, 0.633_851_707_0],
];

/// LMS' (cube roots) → Lab (Ottosson 2020).
pub const M2: M3 = [
    [0.210_454_255_3, 0.793_617_785_0, -0.004_072_046_8],
    [1.977_998_495_1, -2.428_592_205_0, 0.450_593_709_9],
    [0.025_904_037_1, 0.782_771_766_2, -0.808_675_766_0],
];

/// Linear Rec.2020 → LMS.
pub fn rec2020_to_lms() -> M3 {
    mul(&M1, &rec2020_to_xyz())
}

/// LMS → linear Rec.2020.
pub fn lms_to_rec2020() -> M3 {
    mul(&xyz_to_rec2020(), &inv(&M1).expect("M1 is invertible"))
}

fn m2_inv() -> M3 {
    inv(&M2).expect("M2 is invertible")
}

/// XYZ (D65) → OkLab.
pub fn xyz_to_oklab(xyz: [f64; 3]) -> [f64; 3] {
    mulv(&M2, mulv(&M1, xyz).map(f64::cbrt))
}

/// OkLab → XYZ (D65).
pub fn oklab_to_xyz(lab: [f64; 3]) -> [f64; 3] {
    let lms = mulv(&m2_inv(), lab).map(|v| v * v * v);
    mulv(&inv(&M1).expect("M1 is invertible"), lms)
}

/// Linear Rec.2020 → OkLab.
pub fn from_rec2020(rgb: [f64; 3]) -> [f64; 3] {
    mulv(&M2, mulv(&rec2020_to_lms(), rgb).map(f64::cbrt))
}

/// OkLab → linear Rec.2020.
pub fn to_rec2020(lab: [f64; 3]) -> [f64; 3] {
    let lms = mulv(&m2_inv(), lab).map(|v| v * v * v);
    mulv(&lms_to_rec2020(), lms)
}

/// Lightness, chroma and hue angle (radians, −π..π) of OkLab.
pub fn lch(lab: [f64; 3]) -> (f64, f64, f64) {
    (lab[0], lab[1].hypot(lab[2]), lab[2].atan2(lab[1]))
}

/// a / L and b / L of the working white (Rec.2020 D65, xy 0.3127 0.3290). Ottosson's
/// matrices were fitted to a slightly different D65, so its greys sit about 2e-4 · L off
/// a = b = 0.
pub fn white_ab() -> [f64; 2] {
    let w = from_rec2020([1.0; 3]);
    [w[1] / w[0], w[2] / w[0]]
}

/// OkLab with a and b measured from the working white (`a − L·a_w`, `b − L·b_w`): what the
/// grading ops work in, so every grey has a = b = 0 and neutrals stay neutral exactly. A
/// shear of OkLab, so lightness is the same and hues move by at most a few tenths of a
/// degree on dull colours.
pub fn from_rec2020_rel(rgb: [f64; 3]) -> [f64; 3] {
    let l = from_rec2020(rgb);
    let w = white_ab();
    [l[0], l[1] - l[0] * w[0], l[2] - l[0] * w[1]]
}

/// The inverse of [`from_rec2020_rel`].
pub fn to_rec2020_rel(lab: [f64; 3]) -> [f64; 3] {
    let w = white_ab();
    to_rec2020([lab[0], lab[1] + lab[0] * w[0], lab[2] + lab[0] * w[1]])
}

/// The four matrices as row-major f32 (what the kernels and their CPU twins use):
/// Rec.2020 → LMS, M2, M2⁻¹, LMS → Rec.2020; and the white's a / L, b / L.
pub struct OkF32 {
    pub to_lms: [f32; 9],
    pub m2: [f32; 9],
    pub m2_inv: [f32; 9],
    pub from_lms: [f32; 9],
    pub white: [f32; 2],
}

pub static OK_F32: LazyLock<OkF32> = LazyLock::new(|| {
    let f = |m: M3| -> [f32; 9] { std::array::from_fn(|i| m[i / 3][i % 3] as f32) };
    OkF32 {
        to_lms: f(rec2020_to_lms()),
        m2: f(M2),
        m2_inv: f(m2_inv()),
        from_lms: f(lms_to_rec2020()),
        white: white_ab().map(|v| v as f32),
    }
});

fn m3v(m: &[f32; 9], v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i * 3] * v[0] + m[i * 3 + 1] * v[1] + m[i * 3 + 2] * v[2])
}

/// Signed cube root as the kernels take it.
pub fn cbrt_f32(x: f32) -> f32 {
    if x == 0.0 {
        return 0.0;
    }
    x.signum() * x.abs().powf(1.0 / 3.0)
}

/// [`from_rec2020_rel`] in f32, the kernels' arithmetic (`dv_oklab`).
pub fn from_rec2020_f32(rgb: [f32; 3]) -> [f32; 3] {
    let k = &*OK_F32;
    let l = m3v(&k.m2, m3v(&k.to_lms, rgb).map(cbrt_f32));
    [l[0], l[1] - l[0] * k.white[0], l[2] - l[0] * k.white[1]]
}

/// [`to_rec2020_rel`] in f32, the kernels' arithmetic (`dv_oklab_inv`).
pub fn to_rec2020_f32(lab: [f32; 3]) -> [f32; 3] {
    let k = &*OK_F32;
    let l = [
        lab[0],
        lab[1] + lab[0] * k.white[0],
        lab[2] + lab[0] * k.white[1],
    ];
    m3v(&k.from_lms, m3v(&k.m2_inv, l).map(|v| v * v * v))
}

/// WGSL `dv_oklab(rgb)` and `dv_oklab_inv(lab)` with the matrices as constants (written
/// from [`OK_F32`], so the kernels and the CPU twins share every bit).
pub static OKLAB_WGSL: LazyLock<&'static str> = LazyLock::new(|| {
    let k = &*OK_F32;
    let mat = |name: &str, m: &[f32; 9]| {
        let v = |i: usize| format!("{:?}", m[i]);
        format!(
            "fn {name}(v: vec3<f32>) -> vec3<f32> {{\n    return vec3<f32>(\n        \
             {} * v.x + {} * v.y + {} * v.z,\n        \
             {} * v.x + {} * v.y + {} * v.z,\n        \
             {} * v.x + {} * v.y + {} * v.z);\n}}\n",
            v(0),
            v(1),
            v(2),
            v(3),
            v(4),
            v(5),
            v(6),
            v(7),
            v(8)
        )
    };
    let s = format!(
        "{}{}{}{}\
const DV_OK_W = vec2<f32>({:?}, {:?});
fn dv_cbrt(x: f32) -> f32 {{
    if (x == 0.0) {{ return 0.0; }}
    return sign(x) * pow(abs(x), 1.0 / 3.0);
}}
fn dv_oklab(c: vec3<f32>) -> vec3<f32> {{
    let l = dv_ok_to_lms(c);
    let lab = dv_ok_m2(vec3<f32>(dv_cbrt(l.x), dv_cbrt(l.y), dv_cbrt(l.z)));
    return vec3<f32>(lab.x, lab.yz - lab.x * DV_OK_W);
}}
fn dv_oklab_inv(lab: vec3<f32>) -> vec3<f32> {{
    let l = dv_ok_m2_inv(vec3<f32>(lab.x, lab.yz + lab.x * DV_OK_W));
    return dv_ok_from_lms(l * l * l);
}}
",
        mat("dv_ok_to_lms", &k.to_lms),
        mat("dv_ok_m2", &k.m2),
        mat("dv_ok_m2_inv", &k.m2_inv),
        mat("dv_ok_from_lms", &k.from_lms),
        k.white[0],
        k.white[1],
    );
    Box::leak(s.into_boxed_str())
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_is_l1_and_neutral_and_the_round_trip_holds() {
        // D65 white (Y = 1): L = 1, a and b near 0 (Ottosson fitted a slightly different
        // D65); measured from the working white, exactly 0.
        let w = from_rec2020([1.0; 3]);
        assert!(
            (w[0] - 1.0).abs() < 1e-4 && w[1].abs() < 1e-3 && w[2].abs() < 1e-3,
            "{w:?}"
        );
        let r = from_rec2020_rel([0.18; 3]);
        assert!(r[1].abs() < 1e-15 && r[2].abs() < 1e-15, "{r:?}");
        let g = from_rec2020_f32([0.18; 3]);
        assert!(g[1].abs() < 1e-7 && g[2].abs() < 1e-7, "{g:?}");
        let back = to_rec2020_rel(from_rec2020_rel([0.3, 0.1, 0.02]));
        assert!((back[0] - 0.3).abs() < 1e-12 && (back[2] - 0.02).abs() < 1e-12);
        for c in [
            [0.18, 0.18, 0.18],
            [1.0, 0.0, 0.0],
            [0.02, 0.5, 3.0],
            [-0.1, 0.4, 0.2],
            [12.0, 8.0, 0.5],
        ] {
            let back = to_rec2020(from_rec2020(c));
            assert!(
                (0..3).all(|i| (back[i] - c[i]).abs() < 1e-12),
                "{c:?} {back:?}"
            );
            let b32 = to_rec2020_f32(from_rec2020_f32(c.map(|v| v as f32)));
            assert!(
                (0..3).all(|i| (f64::from(b32[i]) - c[i]).abs() < 2e-5 * c[i].abs().max(1.0)),
                "{c:?} {b32:?}"
            );
        }
    }

    #[test]
    fn hue_and_saturation_do_not_move_with_exposure() {
        let c = [0.3, 0.12, 0.05];
        let (l0, c0, h0) = lch(from_rec2020(c));
        for k in [0.01, 0.5, 4.0, 300.0] {
            let (l1, c1, h1) = lch(from_rec2020(c.map(|v| v * k)));
            assert!((h1 - h0).abs() < 1e-12);
            assert!((c1 / l1 - c0 / l0).abs() < 1e-12);
        }
    }
}
