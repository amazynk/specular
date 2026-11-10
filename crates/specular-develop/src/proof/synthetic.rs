//! A synthetic printer profile made here (no vendor data): an RGB "printer" whose paper is
//! warm and darker than white, whose black is a dark grey and whose colours are duller than
//! sRGB, as ICC v2 with `lut16Type` tables for the perceptual and colorimetric intents.
//! The tests, the screenshots and the manual's example use it.
//!
//! Model (device `d` in 0..1, linear sRGB primaries, media-relative):
//! `t = black + (1 − black) · d^1.8`, then crosstalk `c = (1 − k)·t + k·mean(t)` (k = 0.3),
//! paper white at D50 after the relative intent's scaling. The colorimetric `B2A1` inverts
//! the model and clips each channel; the perceptual `B2A0` first compresses chroma and maps
//! L* 0 onto the printer's black.

use crate::colour::{self, M3};
use specular_color::icc::PcsEncoding;
use specular_color::math::{D50, lab_to_xyz, xyz_to_lab};

const GAMMA: f64 = 1.8;
const BLACK: f64 = 0.035;
const CROSSTALK: f64 = 0.3;
/// The paper, as linear sRGB under D65 (warm, a little darker than white).
const PAPER: [f64; 3] = [0.94, 0.92, 0.84];
/// The perceptual intent's chroma scale.
const PERCEPTUAL_CHROMA: f64 = 0.82;

/// The profile's description.
pub const DESCRIPTION: &str = "Specular test printer (synthetic)";

fn d65_to_d50() -> M3 {
    let w65 = colour::xy_to_xyz(colour::D65_XY);
    let w50 = colour::xy_to_xyz(colour::D50_XY);
    colour::adaptation_xyz(colour::Cat::Bradford, w65, w50)
}

/// Linear sRGB (D65) → XYZ D50.
fn rgb_to_d50() -> M3 {
    colour::mul(
        &d65_to_d50(),
        &colour::rgb_to_xyz(&colour::REC709_PRIMARIES, colour::D65_XY),
    )
}

fn crosstalk() -> M3 {
    let k = CROSSTALK;
    let o = k / 3.0;
    [
        [1.0 - k + o, o, o],
        [o, 1.0 - k + o, o],
        [o, o, 1.0 - k + o],
    ]
}

/// Device → XYZ D50, media-relative (paper = D50).
pub fn forward(d: [f64; 3]) -> [f64; 3] {
    let t = d.map(|v| BLACK + (1.0 - BLACK) * v.clamp(0.0, 1.0).powf(GAMMA));
    let c = colour::mulv(&crosstalk(), t);
    let xyz = colour::mulv(&rgb_to_d50(), c);
    // Scale so the relative white is exactly D50.
    let w = colour::mulv(&rgb_to_d50(), [1.0; 3]);
    [
        xyz[0] * f64::from(D50[0]) / w[0],
        xyz[1] * f64::from(D50[1]) / w[1],
        xyz[2] * f64::from(D50[2]) / w[2],
    ]
}

/// XYZ D50 (media-relative) → device, each channel clipped to the gamut.
pub fn inverse(xyz: [f64; 3]) -> [f64; 3] {
    let w = colour::mulv(&rgb_to_d50(), [1.0; 3]);
    let xyz = [
        xyz[0] * w[0] / f64::from(D50[0]),
        xyz[1] * w[1] / f64::from(D50[1]),
        xyz[2] * w[2] / f64::from(D50[2]),
    ];
    let c = colour::mulv(&colour::inv(&rgb_to_d50()).expect("invertible"), xyz);
    let t = colour::mulv(&colour::inv(&crosstalk()).expect("invertible"), c);
    t.map(|v| {
        ((v - BLACK) / (1.0 - BLACK))
            .clamp(0.0, 1.0)
            .powf(1.0 / GAMMA)
    })
}

/// L* of the printer's black (relative).
fn black_l() -> f64 {
    let x = forward([0.0; 3]).map(|v| v as f32);
    f64::from(xyz_to_lab(x)[0])
}

/// Perceptual PCS → colorimetric PCS (L* onto the printer's range, chroma compressed).
fn perceptual_to_relative(lab: [f64; 3]) -> [f64; 3] {
    let lb = black_l();
    [
        lb + (100.0 - lb) * lab[0] / 100.0,
        lab[1] * PERCEPTUAL_CHROMA,
        lab[2] * PERCEPTUAL_CHROMA,
    ]
}

fn relative_to_perceptual(lab: [f64; 3]) -> [f64; 3] {
    let lb = black_l();
    [
        ((lab[0] - lb) / (100.0 - lb) * 100.0).max(0.0),
        lab[1] / PERCEPTUAL_CHROMA,
        lab[2] / PERCEPTUAL_CHROMA,
    ]
}

fn lab_of(xyz: [f64; 3]) -> [f64; 3] {
    xyz_to_lab(xyz.map(|v| v as f32)).map(f64::from)
}

fn xyz_of(lab: [f64; 3]) -> [f64; 3] {
    lab_to_xyz(lab.map(|v| v as f32)).map(f64::from)
}

fn s15(v: f64) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

fn u16v(v: f64) -> [u8; 2] {
    ((v.clamp(0.0, 1.0) * 65535.0).round() as u16).to_be_bytes()
}

/// A `lut16Type` with identity matrix and curves; `f` maps a grid node (normalized
/// inputs) to normalized outputs.
fn mft2(inputs: usize, outputs: usize, grid: usize, f: &dyn Fn(&[f64]) -> Vec<f64>) -> Vec<u8> {
    let mut b = b"mft2\0\0\0\0".to_vec();
    b.extend_from_slice(&[inputs as u8, outputs as u8, grid as u8, 0]);
    for (i, v) in [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
        .iter()
        .enumerate()
    {
        let _ = i;
        b.extend_from_slice(&s15(*v));
    }
    let n = 2u16;
    b.extend_from_slice(&n.to_be_bytes());
    b.extend_from_slice(&n.to_be_bytes());
    let curve = [u16v(0.0), u16v(1.0)].concat();
    for _ in 0..inputs {
        b.extend_from_slice(&curve);
    }
    let nodes = grid.pow(inputs as u32);
    let mut idx = vec![0usize; inputs];
    for _ in 0..nodes {
        let x: Vec<f64> = idx.iter().map(|&i| i as f64 / (grid - 1) as f64).collect();
        for v in f(&x) {
            b.extend_from_slice(&u16v(v));
        }
        // First input slowest.
        for k in (0..inputs).rev() {
            idx[k] += 1;
            if idx[k] < grid {
                break;
            }
            idx[k] = 0;
        }
    }
    for _ in 0..outputs {
        b.extend_from_slice(&curve);
    }
    b
}

fn desc(text: &str) -> Vec<u8> {
    let mut b = b"desc\0\0\0\0".to_vec();
    b.extend_from_slice(&(text.len() as u32 + 1).to_be_bytes());
    b.extend_from_slice(text.as_bytes());
    b.push(0);
    b.extend_from_slice(&[0; 8]);
    b.extend_from_slice(&[0; 3]);
    b.extend_from_slice(&[0; 67]);
    b
}

fn xyz_tag(v: [f64; 3]) -> Vec<u8> {
    let mut b = b"XYZ \0\0\0\0".to_vec();
    for c in v {
        b.extend_from_slice(&s15(c));
    }
    b
}

/// The profile's bytes.
pub fn icc() -> Vec<u8> {
    let enc = PcsEncoding::LabV2;
    let a2b1 = |x: &[f64]| {
        enc.encode(forward([x[0], x[1], x[2]]).map(|v| v as f32))
            .map(f64::from)
            .to_vec()
    };
    let a2b0 = |x: &[f64]| {
        let lab = relative_to_perceptual(lab_of(forward([x[0], x[1], x[2]])));
        enc.encode(xyz_of(lab).map(|v| v as f32))
            .map(f64::from)
            .to_vec()
    };
    let b2a1 = |x: &[f64]| {
        let xyz = enc
            .decode([x[0] as f32, x[1] as f32, x[2] as f32])
            .map(f64::from);
        inverse(xyz).to_vec()
    };
    let b2a0 = |x: &[f64]| {
        let xyz = enc
            .decode([x[0] as f32, x[1] as f32, x[2] as f32])
            .map(f64::from);
        inverse(xyz_of(perceptual_to_relative(lab_of(xyz)))).to_vec()
    };
    let paper = colour::mulv(&rgb_to_d50(), PAPER);
    let tags: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", desc(DESCRIPTION)),
        (b"wtpt", xyz_tag(paper)),
        (b"A2B0", mft2(3, 3, 17, &a2b0)),
        (b"A2B1", mft2(3, 3, 17, &a2b1)),
        (b"B2A0", mft2(3, 3, 33, &b2a0)),
        (b"B2A1", mft2(3, 3, 33, &b2a1)),
    ];
    let mut table = Vec::new();
    let mut data = Vec::new();
    let start = 128 + 4 + 12 * tags.len();
    for (sig, body) in &tags {
        table.extend_from_slice(*sig);
        table.extend_from_slice(&((start + data.len()) as u32).to_be_bytes());
        table.extend_from_slice(&(body.len() as u32).to_be_bytes());
        data.extend_from_slice(body);
        while data.len() % 4 != 0 {
            data.push(0);
        }
    }
    let size = start + data.len();
    let mut h = Vec::with_capacity(size);
    h.extend_from_slice(&(size as u32).to_be_bytes());
    h.extend_from_slice(b"spec");
    h.extend_from_slice(&0x0210_0000u32.to_be_bytes());
    h.extend_from_slice(b"prtrRGB Lab ");
    for v in [2026u16, 11, 1, 0, 0, 0] {
        h.extend_from_slice(&v.to_be_bytes());
    }
    h.extend_from_slice(b"acsp");
    h.extend_from_slice(&[0; 4 + 4 + 4 + 4 + 8 + 4]);
    for c in D50 {
        h.extend_from_slice(&s15(f64::from(c)));
    }
    h.extend_from_slice(b"spec");
    h.resize(128, 0);
    h.extend_from_slice(&(tags.len() as u32).to_be_bytes());
    h.extend_from_slice(&table);
    h.extend_from_slice(&data);
    h
}
