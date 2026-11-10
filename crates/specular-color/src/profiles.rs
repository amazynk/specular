//! Built-in profiles: the working space, common RGB spaces, L*a*b* and a generic CMYK
//! printing model (no third-party profile data is bundled).

use crate::curve::Curve;
use crate::icc::{ColorSpace, Lut, PcsEncoding, Profile};
use crate::lut::Pipeline;
use crate::math::{D50, D65_XY, Mat3, Vec3, bt709_to_pcs, d50, lab_to_xyz, rgb_to_pcs};

fn base(color_space: ColorSpace, hash: u64) -> Profile {
    Profile {
        version: 4,
        class: *b"mntr",
        color_space,
        pcs_lab: false,
        white: D50,
        matrix: None,
        trc: None,
        gray_trc: None,
        a2b: [None, None, None],
        b2a: [None, None, None],
        model: None,
        hash,
    }
}

/// An RGB matrix/TRC profile from its colorants (XYZ D50 columns) and one curve.
pub fn matrix_shaper(m: Mat3, trc: Curve, hash: u64) -> Profile {
    let mut p = base(ColorSpace::Rgb, hash);
    p.matrix = Some(m);
    p.trc = Some([trc.clone(), trc.clone(), trc]);
    p
}

/// sRGB (IEC 61966-2-1).
pub fn srgb() -> Profile {
    matrix_shaper(bt709_to_pcs(), Curve::srgb(), 0x5352_4742_0000_0001)
}

/// Linear-light BT.709 / sRGB primaries, unclamped: the space Specular renders in.
pub fn linear_bt709() -> Profile {
    matrix_shaper(bt709_to_pcs(), Curve::Identity, 0x5352_4742_0000_0002)
}

/// Display P3 (P3 primaries, D65, sRGB transfer).
pub fn display_p3() -> Profile {
    matrix_shaper(
        rgb_to_pcs([0.680, 0.320], [0.265, 0.690], [0.150, 0.060], D65_XY),
        Curve::srgb(),
        0x5352_4742_0000_0003,
    )
}

/// Adobe RGB (1998).
pub fn adobe_rgb() -> Profile {
    matrix_shaper(
        rgb_to_pcs([0.64, 0.33], [0.21, 0.71], [0.15, 0.06], D65_XY),
        Curve::Gamma(563.0 / 256.0),
        0x5352_4742_0000_0004,
    )
}

/// BT.2020 primaries with the BT.709/2020 SDR transfer function.
pub fn bt2020() -> Profile {
    let a = 1.099_296_8_f32;
    let b = 0.018_053_97_f32;
    let trc = Curve::Param(
        3,
        [
            1.0 / 0.45,
            1.0 / a,
            (a - 1.0) / a,
            1.0 / 4.5,
            4.5 * b,
            0.0,
            0.0,
        ],
    );
    matrix_shaper(
        rgb_to_pcs([0.708, 0.292], [0.170, 0.797], [0.131, 0.046], D65_XY),
        trc,
        0x5352_4742_0000_0005,
    )
}

/// Gray with the sRGB transfer function (the usual assumption for untagged gray).
pub fn gray_srgb() -> Profile {
    let mut p = base(ColorSpace::Gray, 0x5352_4742_0000_0006);
    p.gray_trc = Some(Curve::srgb());
    p
}

/// CIE L*a*b* (D50) as a device space. Device values use the v4 normalization:
/// L/100, (a + 128)/255, (b + 128)/255.
pub fn lab() -> Profile {
    let mut p = base(ColorSpace::Lab, 0x5352_4742_0000_0007);
    // Abstract, like Little CMS's built-in L*a*b* profile: no black point of its own.
    p.class = *b"abst";
    p.pcs_lab = true;
    let id = Lut {
        pipe: Pipeline {
            inputs: 3,
            outputs: 3,
            stages: vec![],
        },
        pcs: PcsEncoding::LabV4,
    };
    p.a2b[0] = Some(id.clone());
    p.b2a[0] = Some(id);
    p
}

/// The generic CMYK profile used for untagged CMYK: an analytic printing model, see
/// [`generic_cmyk_xyz`].
pub fn generic_cmyk() -> Profile {
    let mut p = base(ColorSpace::Cmyk, 0x5352_4742_0000_0008);
    p.class = *b"prtr";
    p.pcs_lab = true;
    p.white = paper_xyz();
    p.model = Some(Model::GenericCmyk);
    p
}

/// Analytic device models standing in for profile tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Model {
    GenericCmyk,
}

impl Model {
    /// Device values → XYZ (D50, media relative).
    pub(crate) fn eval(self, v: &[f32]) -> Vec3 {
        match self {
            Model::GenericCmyk => generic_cmyk_xyz([v[0], v[1], v[2], v[3]]),
        }
    }
}

// Neugebauer primaries (CIELAB D50, absolute), typical of offset printing on coated paper
// (ISO 12647-2 PC1-like). Index bits: 1 = C, 2 = M, 4 = Y.
const PRIMARIES: [Vec3; 8] = [
    [95.0, 0.0, -2.0],    // paper
    [55.0, -37.0, -50.0], // C
    [48.0, 74.0, -3.0],   // M
    [24.0, 22.0, -46.0],  // C+M (blue)
    [89.0, -5.0, 93.0],   // Y
    [50.0, -65.0, 27.0],  // C+Y (green)
    [47.0, 68.0, 48.0],   // M+Y (red)
    [23.0, 0.0, 0.0],     // C+M+Y
];
const BLACK_INK: Vec3 = [16.0, 0.0, 0.0];
/// Darkest reachable reflectance (first-surface reflection), as L*.
const FLOOR_L: f32 = 7.0;
/// Mechanical dot gain at 50 % (CMY, K).
const TVI: [f32; 2] = [0.10, 0.12];
/// Yule–Nielsen n.
const YN: f32 = 2.0;

fn paper_xyz() -> Vec3 {
    lab_to_xyz(PRIMARIES[0])
}

fn primaries_rel() -> &'static [Vec3; 16] {
    static P: std::sync::OnceLock<[Vec3; 16]> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let paper = paper_xyz();
        let rel = |lab: Vec3| {
            let x = lab_to_xyz(lab);
            [
                x[0] * D50[0] / paper[0],
                x[1] * D50[1] / paper[1],
                x[2] * D50[2] / paper[2],
            ]
        };
        let k = rel(BLACK_INK);
        let w = rel(PRIMARIES[0]);
        let floor = lab_to_xyz([FLOOR_L, 0.0, 0.0]);
        let mut out = [[0f32; 3]; 16];
        for i in 0..8 {
            let r = rel(PRIMARIES[i]);
            out[i] = r;
            // Black overprint: transmission through the black film, above the surface floor.
            for c in 0..3 {
                let f = floor[c];
                out[i + 8][c] =
                    f + (r[c] - f).max(0.0) * (k[c] - f).max(0.0) / (w[c] - f).max(1e-6);
            }
        }
        // Applying the model's 1/n power once here.
        for p in &mut out {
            for c in p.iter_mut() {
                *c = c.max(0.0).powf(1.0 / YN);
            }
        }
        out
    })
}

/// The generic CMYK model: ink amounts (0 = none, 1 = solid) → XYZ (D50), media relative.
/// A Yule–Nielsen modified spectral-free Neugebauer model over 16 primaries with a parabolic
/// dot-gain curve.
pub fn generic_cmyk_xyz(ink: [f32; 4]) -> Vec3 {
    let p = primaries_rel();
    let gain = |a: f32, t: f32| {
        let a = crate::curve::unit(a);
        (a + t * 4.0 * a * (1.0 - a)).clamp(0.0, 1.0)
    };
    let c = gain(ink[0], TVI[0]);
    let m = gain(ink[1], TVI[0]);
    let y = gain(ink[2], TVI[0]);
    let k = gain(ink[3], TVI[1]);
    let mut acc = [0f32; 3];
    for (i, prim) in p.iter().enumerate() {
        let w = (if i & 1 != 0 { c } else { 1.0 - c })
            * (if i & 2 != 0 { m } else { 1.0 - m })
            * (if i & 4 != 0 { y } else { 1.0 - y })
            * (if i & 8 != 0 { k } else { 1.0 - k });
        for ch in 0..3 {
            acc[ch] += w * prim[ch];
        }
    }
    acc.map(|v| v.powf(YN))
}

/// ICC v4 bytes of an RGB display profile from D50 colorants (`m`'s columns, as
/// [`rgb_to_pcs`] gives them) and a pure power-law decoding exponent: for formats that state
/// their colour space as primaries and a gamma (PNG gAMA/cHRM) so it can travel as an
/// embedded profile. The media white is D50 (colorants are already adapted).
pub fn matrix_shaper_icc(m: &Mat3, gamma: f64, description: &str) -> Vec<u8> {
    matrix_icc(m, &[gamma], description)
}

/// As [`matrix_shaper_icc`], with the sRGB tone curve (IEC 61966-2-1) on every channel: for
/// linear data a decoder has already put through the sRGB curve, keeping its colorants.
pub fn matrix_srgb_curve_icc(m: &Mat3, description: &str) -> Vec<u8> {
    matrix_icc(
        m,
        &[2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045],
        description,
    )
}

/// `para` function type 0 (one parameter, a power) or 3 (five, the sRGB form).
fn matrix_icc(m: &Mat3, para: &[f64], description: &str) -> Vec<u8> {
    matrix_para_icc(m, para, description)
}

/// ICC v4 bytes of an RGB display profile from D50 colorants (`m`'s columns) and one
/// `parametricCurveType` for all three channels: `para` holds function type 0's single
/// exponent `[g]` or type 3's five values `[g, a, b, c, d]` (`Y = (aX + b)^g` for
/// `X ≥ d`, else `cX`). For export profiles (sRGB, Display P3, Rec.2020, an Adobe RGB
/// compatible space, and linear ones with `[1.0]`).
pub fn matrix_para_icc(m: &Mat3, para: &[f64], description: &str) -> Vec<u8> {
    fn s15(v: f64) -> [u8; 4] {
        ((v * 65536.0).round() as i32).to_be_bytes()
    }
    fn xyz(v: [f64; 3]) -> Vec<u8> {
        let mut t = b"XYZ \0\0\0\0".to_vec();
        for c in v {
            t.extend_from_slice(&s15(c));
        }
        t
    }
    let desc = {
        // multiLocalizedUnicode, one en-US record in UTF-16BE.
        let text: Vec<u8> = description
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        let mut t = b"mluc\0\0\0\0".to_vec();
        t.extend_from_slice(&1u32.to_be_bytes());
        t.extend_from_slice(&12u32.to_be_bytes());
        t.extend_from_slice(b"enUS");
        t.extend_from_slice(&(text.len() as u32).to_be_bytes());
        t.extend_from_slice(&28u32.to_be_bytes());
        t.extend_from_slice(&text);
        t
    };
    let kind: u16 = if para.len() == 5 { 3 } else { 0 };
    let mut trc = b"para\0\0\0\0".to_vec();
    trc.extend_from_slice(&kind.to_be_bytes());
    trc.extend_from_slice(&[0, 0]);
    for &v in para {
        trc.extend_from_slice(&s15(v));
    }
    let col = |i: usize| [m.0[0][i], m.0[1][i], m.0[2][i]];
    let bodies: [(&[u8; 4], Vec<u8>); 6] = [
        (b"desc", desc),
        (b"wtpt", xyz(d50())),
        (b"rXYZ", xyz(col(0))),
        (b"gXYZ", xyz(col(1))),
        (b"bXYZ", xyz(col(2))),
        (b"rTRC", trc),
    ];
    // Three TRC entries share one body.
    let count = bodies.len() + 2;
    let mut table = Vec::new();
    let mut data = Vec::new();
    let mut trc_at = (0, 0);
    let base = 128 + 4 + 12 * count;
    for (sig, body) in &bodies {
        let off = base + data.len();
        if *sig == b"rTRC" {
            trc_at = (off, body.len());
        }
        table.push((**sig, off, body.len()));
        data.extend_from_slice(body);
        data.resize(data.len().div_ceil(4) * 4, 0);
    }
    table.push((*b"gTRC", trc_at.0, trc_at.1));
    table.push((*b"bTRC", trc_at.0, trc_at.1));
    let size = base + data.len();
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(&(size as u32).to_be_bytes());
    out.extend_from_slice(&[0; 4]); // CMM
    out.extend_from_slice(&[4, 0x30, 0, 0]); // version 4.3
    out.extend_from_slice(b"mntrRGB XYZ ");
    out.extend_from_slice(&[0; 12]); // date
    out.extend_from_slice(b"acsp");
    out.extend_from_slice(&[0; 24]); // platform, flags, manufacturer, model, attributes
    out.extend_from_slice(&0u32.to_be_bytes()); // perceptual intent
    for c in d50() {
        out.extend_from_slice(&s15(c));
    }
    out.resize(128, 0);
    out.extend_from_slice(&(count as u32).to_be_bytes());
    for (sig, off, len) in table {
        out.extend_from_slice(&sig);
        out.extend_from_slice(&(off as u32).to_be_bytes());
        out.extend_from_slice(&(len as u32).to_be_bytes());
    }
    out.extend_from_slice(&data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icc::Profile;

    #[test]
    fn matrix_shaper_icc_parses_back() {
        let m = rgb_to_pcs([0.64, 0.33], [0.21, 0.71], [0.15, 0.06], D65_XY);
        let bytes = matrix_shaper_icc(&m, 1.8, "PNG gAMA/cHRM");
        let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(len as usize, bytes.len());
        let p = Profile::parse(&bytes).expect("parses");
        assert!(p.is_matrix_shaper());
        assert!(p.matrix.as_ref().is_some_and(|pm| pm.max_diff(&m) < 1e-4));
        for c in p.trc.as_ref().expect("curves") {
            assert!(
                matches!(c, Curve::Gamma(g) if (g - 1.8).abs() < 1e-4),
                "{c:?}"
            );
        }
        assert!((p.white[0] - D50[0]).abs() < 1e-4 && (p.white[2] - D50[2]).abs() < 1e-4);
    }

    /// From the fidelity corpus (32-bit PSD with a linear wide-gamut profile): the colorants with
    /// the sRGB curve, which must decode exactly as the sRGB transfer function.
    #[test]
    fn matrix_srgb_curve_icc_decodes_as_srgb() {
        let m = rgb_to_pcs([0.7347, 0.2653], [0.1596, 0.8404], [0.0366, 0.0001], D65_XY);
        let p = Profile::parse(&matrix_srgb_curve_icc(&m, "linear ProPhoto, sRGB coded"))
            .expect("parses");
        assert!(p.matrix.as_ref().is_some_and(|pm| pm.max_diff(&m) < 1e-4));
        let srgb = Curve::srgb();
        for c in p.trc.as_ref().expect("curves") {
            for i in 0..=64 {
                let x = i as f32 / 64.0;
                assert!((c.eval(x) - srgb.eval(x)).abs() < 2e-4, "{x}: {c:?}");
            }
        }
    }
}
