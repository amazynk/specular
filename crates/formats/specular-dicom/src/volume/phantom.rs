//! A synthetic CT phantom for tests, screenshots and benchmarks (no patient data): a body
//! cylinder of soft tissue in air with spheres and a rod of known Hounsfield units, defined
//! analytically in patient coordinates (mm, LPS, centred on the origin), as a [`Volume`] or as
//! the DICOM files of a series.

use super::{Geometry, Volume, VoxelKind};

/// Air outside the body.
pub const AIR: f32 = -1000.0;
/// The body cylinder (radius [`BODY_RADIUS`] about the S–I axis, |z| ≤ [`BODY_HALF_LENGTH`]).
pub const TISSUE: f32 = 40.0;
pub const BODY_RADIUS: f64 = 150.0;
pub const BODY_HALF_LENGTH: f64 = 200.0;

/// Spheres: centre (mm), radius (mm), HU. Bone on the patient's left, fat right-anterior-
/// superior, contrast posterior-inferior.
pub const SPHERES: [([f64; 3], f64, f32); 3] = [
    ([60.0, 0.0, 0.0], 30.0, 1000.0),
    ([-60.0, -30.0, 40.0], 25.0, -100.0),
    ([0.0, 70.0, -60.0], 20.0, 300.0),
];

/// A rod along A–P (y) at x = 0, z = [`ROD_Z`], radius [`ROD_RADIUS`], HU [`ROD`].
pub const ROD: f32 = 500.0;
pub const ROD_Z: f64 = 100.0;
pub const ROD_RADIUS: f64 = 12.0;

/// The phantom's value at patient position `p`.
pub fn hu(p: [f64; 3]) -> f32 {
    let [x, y, z] = p;
    if x * x + y * y > BODY_RADIUS * BODY_RADIUS || z.abs() > BODY_HALF_LENGTH {
        return AIR;
    }
    for (c, r, v) in SPHERES {
        let d = [x - c[0], y - c[1], z - c[2]];
        if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= r * r {
            return v;
        }
    }
    if x * x + (z - ROD_Z) * (z - ROD_Z) <= ROD_RADIUS * ROD_RADIUS && y.abs() < 120.0 {
        return ROD;
    }
    TISSUE
}

/// Axial slice positions of a `size` (columns, rows, slices) acquisition centred on the
/// origin, `pixel` mm in-plane, `spacing` mm apart, with a gantry `tilt` (degrees: each slice
/// shifted along y by `spacing · tan(tilt)` from the one before).
pub fn positions(size: [u32; 3], pixel: f64, spacing: f64, tilt: f64) -> Vec<[f64; 3]> {
    let half = |n: u32, s: f64| (n as f64 - 1.0) * s / 2.0;
    let shear = tilt.to_radians().tan() * spacing;
    (0..size[2])
        .map(|k| {
            let c = k as f64 - (size[2] as f64 - 1.0) / 2.0;
            [
                -half(size[0], pixel),
                -half(size[1], pixel) + c * shear,
                c * spacing,
            ]
        })
        .collect()
}

/// The phantom sampled at the voxel centres of axial slices (see [`positions`]); every slice
/// loaded.
pub fn volume(size: [u32; 3], pixel: f64, spacing: f64, tilt: f64) -> Volume {
    let pos = positions(size, pixel, spacing, tilt);
    let g = Geometry::new(
        &pos,
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [pixel, pixel],
        [size[0], size[1]],
    );
    let mut v = Volume::new(g, VoxelKind::I16, 1).expect("phantom fits in memory");
    let n = (size[0] * size[1]) as usize;
    let mut buf = vec![0f32; n];
    for k in 0..size[2] as usize {
        for j in 0..size[1] {
            for i in 0..size[0] {
                let p = v.geometry.to_patient([i as f64, j as f64, k as f64]);
                buf[(j * size[0] + i) as usize] = hu(p);
            }
        }
        v.put(k, &buf, size[0], size[1]).expect("slice fits");
    }
    v
}

/// The DICOM files (Part 10, explicit VR little endian, CT, 16-bit signed stored values,
/// rescale −1024) of the phantom's axial series (see [`positions`]), named `slice-NNNN.dcm`
/// and listed in a shuffled order so loading cannot rely on file names.
pub fn series(size: [u32; 3], pixel: f64, spacing: f64, tilt: f64) -> Vec<(String, Vec<u8>)> {
    let pos = positions(size, pixel, spacing, tilt);
    let mut out: Vec<(String, Vec<u8>)> = pos
        .iter()
        .enumerate()
        .map(|(k, p)| {
            (
                format!("slice-{k:04}.dcm"),
                slice_file(size, pixel, spacing, k, *p),
            )
        })
        .collect();
    // A fixed shuffle (multiplicative step coprime with the count).
    let n = out.len();
    if n > 2 {
        let mut step = n / 2 + 1;
        while gcd(step, n) != 1 {
            step += 1;
        }
        let mut shuffled = Vec::with_capacity(n);
        let mut taken: Vec<Option<(String, Vec<u8>)>> = out.drain(..).map(Some).collect();
        for i in 0..n {
            if let Some(f) = taken[(i * step) % n].take() {
                shuffled.push(f);
            }
        }
        out = shuffled;
    }
    out
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

const STUDY: &str = "2.25.1000000000000000000000000001";
const SERIES: &str = "2.25.1000000000000000000000000002";
const CT_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.2";
const EXPLICIT_LE: &str = "1.2.840.10008.1.2.1";

fn slice_file(size: [u32; 3], pixel: f64, spacing: f64, k: usize, p: [f64; 3]) -> Vec<u8> {
    let sop = format!("2.25.1000000000000000000000000003{k:05}");
    let ds = |v: f64| {
        format!("{v:.6}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    };
    let mut px = Vec::with_capacity(size[0] as usize * size[1] as usize * 2);
    for j in 0..size[1] {
        for i in 0..size[0] {
            let q = [p[0] + i as f64 * pixel, p[1] + j as f64 * pixel, p[2]];
            let stored = (hu(q) + 1024.0) as i16;
            px.extend(stored.to_le_bytes());
        }
    }
    let text = |s: &str| {
        let mut b = s.as_bytes().to_vec();
        if b.len() % 2 == 1 {
            b.push(b' ');
        }
        b
    };
    let uid = |s: &str| {
        let mut b = s.as_bytes().to_vec();
        if b.len() % 2 == 1 {
            b.push(0);
        }
        b
    };
    let us = |v: u16| v.to_le_bytes().to_vec();
    let mut meta = Vec::new();
    el(&mut meta, 0x0002_0001, b"OB", &[0, 1]);
    el(&mut meta, 0x0002_0002, b"UI", &uid(CT_IMAGE));
    el(&mut meta, 0x0002_0003, b"UI", &uid(&sop));
    el(&mut meta, 0x0002_0010, b"UI", &uid(EXPLICIT_LE));
    let mut f = vec![0u8; 128];
    f.extend(b"DICM");
    el(
        &mut f,
        0x0002_0000,
        b"UL",
        &(meta.len() as u32).to_le_bytes(),
    );
    f.extend(meta);
    let els: Vec<(u32, &[u8; 2], Vec<u8>)> = vec![
        (0x0008_0016, b"UI", uid(CT_IMAGE)),
        (0x0008_0018, b"UI", uid(&sop)),
        (0x0008_0060, b"CS", text("CT")),
        (0x0008_103E, b"LO", text("Phantom")),
        (0x0010_0010, b"PN", text("Phantom")),
        (0x0018_0050, b"DS", text(&ds(spacing))),
        (0x0020_000D, b"UI", uid(STUDY)),
        (0x0020_000E, b"UI", uid(SERIES)),
        (0x0020_0011, b"IS", text("1")),
        (0x0020_0013, b"IS", text(&(k + 1).to_string())),
        (
            0x0020_0032,
            b"DS",
            text(&format!("{}\\{}\\{}", ds(p[0]), ds(p[1]), ds(p[2]))),
        ),
        (0x0020_0037, b"DS", text("1\\0\\0\\0\\1\\0")),
        (0x0028_0002, b"US", us(1)),
        (0x0028_0004, b"CS", text("MONOCHROME2")),
        (0x0028_0010, b"US", us(size[1] as u16)),
        (0x0028_0011, b"US", us(size[0] as u16)),
        (
            0x0028_0030,
            b"DS",
            text(&format!("{}\\{}", ds(pixel), ds(pixel))),
        ),
        (0x0028_0100, b"US", us(16)),
        (0x0028_0101, b"US", us(16)),
        (0x0028_0102, b"US", us(15)),
        (0x0028_0103, b"US", us(1)),
        (0x0028_1050, b"DS", text("40")),
        (0x0028_1051, b"DS", text("400")),
        (0x0028_1052, b"DS", text("-1024")),
        (0x0028_1053, b"DS", text("1")),
        (0x7FE0_0010, b"OW", px),
    ];
    for (tag, vr, val) in &els {
        el(&mut f, *tag, vr, val);
    }
    f
}

fn el(out: &mut Vec<u8>, tag: u32, vr: &[u8; 2], val: &[u8]) {
    out.extend(((tag >> 16) as u16).to_le_bytes());
    out.extend((tag as u16).to_le_bytes());
    out.extend(vr);
    if matches!(vr, b"OB" | b"OW" | b"SQ" | b"UN" | b"UT") {
        out.extend([0, 0]);
        out.extend((val.len() as u32).to_le_bytes());
    } else {
        out.extend((val.len() as u16).to_le_bytes());
    }
    out.extend(val);
}
