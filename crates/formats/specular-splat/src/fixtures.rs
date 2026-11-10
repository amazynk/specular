//! Generated splat files for tests: one scene written in every format read.

/// One splat as the 3DGS trainer stores it (in the file's frame).
#[derive(Clone, Copy, Debug)]
pub struct Gauss {
    pub pos: [f32; 3],
    pub log_scale: [f32; 3],
    /// w, x, y, z.
    pub rot: [f32; 4],
    pub dc: [f32; 3],
    pub logit: f32,
    /// Degree-3 rest coefficients, channel-major (as in PLY).
    pub rest: [f32; 45],
}

/// A small scene: a ring of elongated splats around a sphere of round ones, coloured by angle,
/// the ring's colour changing with the view (degree-1 harmonics). Up is −y (COLMAP).
pub fn scene(n: usize) -> Vec<Gauss> {
    let mut out = Vec::with_capacity(n);
    let ring = n / 2;
    for i in 0..n {
        let t = i as f32 / n.max(1) as f32;
        let mut rest = [0f32; 45];
        let g = if i < ring {
            let a = i as f32 / ring as f32 * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            // A tangent rotation about −y (up).
            let half = a / 2.0;
            rest[0] = 0.6 * c;
            rest[15 + 1] = 0.4;
            rest[30 + 2] = -0.5 * s;
            Gauss {
                pos: [2.0 * c, -0.2 * (3.0 * a).sin(), 2.0 * s],
                log_scale: [0.25f32.ln(), 0.05f32.ln(), 0.06f32.ln()],
                rot: [half.cos(), 0.0, -half.sin(), 0.0],
                dc: [c * 1.2, s * 1.2, 0.4],
                logit: 2.0,
                rest,
            }
        } else {
            // Fibonacci sphere.
            let k = (i - ring) as f32;
            let m = (n - ring) as f32;
            let y = 1.0 - 2.0 * (k + 0.5) / m;
            let r = (1.0 - y * y).sqrt();
            let phi = k * 2.399_963;
            Gauss {
                pos: [r * phi.cos(), y, r * phi.sin()],
                log_scale: [0.12f32.ln(); 3],
                rot: [1.0, 0.0, 0.0, 0.0],
                dc: [0.8 * t, 1.0 - t, 0.75 * (1.0 - y) - 0.5],
                logit: 3.0,
                rest,
            }
        };
        out.push(g);
    }
    out
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// The scene as an antimatter15 `.splat` (sorted as the original tool does: by importance).
pub fn splat_file(g: &[Gauss]) -> Vec<u8> {
    let mut out = Vec::with_capacity(g.len() * 32);
    for s in g {
        for v in s.pos {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in s.log_scale {
            out.extend_from_slice(&v.exp().to_le_bytes());
        }
        for c in s.dc {
            let v = 0.5 + crate::SH_C0 * c;
            out.push((v * 255.0).round().clamp(0.0, 255.0) as u8);
        }
        out.push((sigmoid(s.logit) * 255.0).round() as u8);
        let l = s.rot.iter().map(|v| v * v).sum::<f32>().sqrt();
        for v in s.rot {
            out.push(((v / l) * 128.0 + 128.0).round().clamp(0.0, 255.0) as u8);
        }
    }
    out
}

/// The scene as a 3DGS PLY with harmonics of `degree` (0–3), binary little-endian or ASCII.
pub fn ply_file(g: &[Gauss], degree: u8, ascii: bool) -> Vec<u8> {
    let k = crate::sh_coeffs(degree);
    let mut h = format!(
        "ply\nformat {} 1.0\nelement vertex {}\n",
        if ascii {
            "ascii"
        } else {
            "binary_little_endian"
        },
        g.len()
    );
    let mut names: Vec<String> = [
        "x", "y", "z", "nx", "ny", "nz", "f_dc_0", "f_dc_1", "f_dc_2",
    ]
    .map(String::from)
    .to_vec();
    names.extend((0..k * 3).map(|i| format!("f_rest_{i}")));
    names.extend(
        [
            "opacity", "scale_0", "scale_1", "scale_2", "rot_0", "rot_1", "rot_2", "rot_3",
        ]
        .map(String::from),
    );
    for n in &names {
        h += &format!("property float {n}\n");
    }
    h += "end_header\n";
    let mut out = h.into_bytes();
    for s in g {
        let mut row = vec![s.pos[0], s.pos[1], s.pos[2], 0.0, 0.0, 0.0];
        row.extend_from_slice(&s.dc);
        for ch in 0..3 {
            for j in 0..k {
                row.push(s.rest[ch * 15 + j]);
            }
        }
        row.push(s.logit);
        row.extend_from_slice(&s.log_scale);
        row.extend_from_slice(&s.rot);
        if ascii {
            let line: Vec<String> = row.iter().map(|v| format!("{v}")).collect();
            out.extend_from_slice(line.join(" ").as_bytes());
            out.push(b'\n');
        } else {
            for v in row {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    out
}

/// The scene as a SuperSplat compressed PLY (with colour ranges and degree-1 harmonics).
pub fn compressed_ply_file(g: &[Gauss]) -> Vec<u8> {
    let n = g.len();
    let chunks = n.div_ceil(256);
    let mut h = format!("ply\nformat binary_little_endian 1.0\nelement chunk {chunks}\n");
    for p in [
        "min_x",
        "min_y",
        "min_z",
        "max_x",
        "max_y",
        "max_z",
        "min_scale_x",
        "min_scale_y",
        "min_scale_z",
        "max_scale_x",
        "max_scale_y",
        "max_scale_z",
        "min_r",
        "min_g",
        "min_b",
        "max_r",
        "max_g",
        "max_b",
    ] {
        h += &format!("property float {p}\n");
    }
    h += &format!("element vertex {n}\n");
    for p in [
        "packed_position",
        "packed_rotation",
        "packed_scale",
        "packed_color",
    ] {
        h += &format!("property uint {p}\n");
    }
    h += &format!("element sh {n}\n");
    for i in 0..9 {
        h += &format!("property uchar f_rest_{i}\n");
    }
    h += "end_header\n";
    let mut out = h.into_bytes();
    let colour = |s: &Gauss| s.dc.map(|c| 0.5 + crate::SH_C0 * c);
    let mut ranges = Vec::new();
    for c in 0..chunks {
        let part = &g[c * 256..((c + 1) * 256).min(n)];
        let mut r = [
            f32::INFINITY,
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        let mut sr = r;
        let mut cr = r;
        for s in part {
            for a in 0..3 {
                r[a] = r[a].min(s.pos[a]);
                r[a + 3] = r[a + 3].max(s.pos[a]);
                sr[a] = sr[a].min(s.log_scale[a]);
                sr[a + 3] = sr[a + 3].max(s.log_scale[a]);
                cr[a] = cr[a].min(colour(s)[a]);
                cr[a + 3] = cr[a + 3].max(colour(s)[a]);
            }
        }
        for v in r.iter().chain(&sr).chain(&cr) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        ranges.push((r, sr, cr));
    }
    let q = |v: f32, lo: f32, hi: f32, bits: u32| {
        let m = ((1u32 << bits) - 1) as f32;
        let t = if hi > lo { (v - lo) / (hi - lo) } else { 0.0 };
        (t.clamp(0.0, 1.0) * m).round() as u32
    };
    for (i, s) in g.iter().enumerate() {
        let (r, sr, cr) = ranges[i / 256];
        let pack = |v: [f32; 3], lo: &[f32; 6]| {
            (q(v[0], lo[0], lo[3], 11) << 21)
                | (q(v[1], lo[1], lo[4], 10) << 11)
                | q(v[2], lo[2], lo[5], 11)
        };
        let l = s.rot.iter().map(|v| v * v).sum::<f32>().sqrt();
        let rot = s.rot.map(|v| v / l);
        let big = (0..4)
            .max_by(|&a, &b| rot[a].abs().total_cmp(&rot[b].abs()))
            .unwrap_or(0);
        let sign = if rot[big] < 0.0 { -1.0 } else { 1.0 };
        let mut word = (big as u32) << 30;
        let mut shift = 30;
        for (k, v) in rot.iter().enumerate() {
            if k == big {
                continue;
            }
            shift -= 10;
            let t = (v * sign) / std::f32::consts::SQRT_2 + 0.5;
            word |= ((t.clamp(0.0, 1.0) * 1023.0).round() as u32) << shift;
        }
        let c = colour(s);
        let cq = |a: usize| q(c[a], cr[a], cr[a + 3], 8);
        let alpha = (sigmoid(s.logit) * 255.0).round() as u32;
        for w in [
            pack(s.pos, &r),
            word,
            pack(s.log_scale, &sr),
            (cq(0) << 24) | (cq(1) << 16) | (cq(2) << 8) | alpha,
        ] {
            out.extend_from_slice(&w.to_le_bytes());
        }
    }
    for s in g {
        for ch in 0..3 {
            for j in 0..3 {
                let v = s.rest[ch * 15 + j];
                out.push(((v + 4.0) * 255.0 / 8.0).round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    out
}

/// The SPZ arrays of the scene (version 2 or 3 rotations), y up (the scene's y flipped,
/// with z, from COLMAP's frame), harmonics of `degree`.
fn spz_arrays(g: &[Gauss], version: u32, degree: u8) -> [Vec<u8>; 6] {
    let k = crate::sh_coeffs(degree);
    let mut a: [Vec<u8>; 6] = Default::default();
    for s in g {
        // RDF → RUB: y and z flip; so do the rotation's y and z and the harmonics that are
        // odd in them.
        let p = [s.pos[0], -s.pos[1], -s.pos[2]];
        for v in p {
            let q = (v * 4096.0).round() as i32;
            a[0].extend_from_slice(&q.to_le_bytes()[..3]);
        }
        a[1].push((sigmoid(s.logit) * 255.0).round() as u8);
        for c in s.dc {
            a[2].push((c * 0.15 * 255.0 + 127.5).round().clamp(0.0, 255.0) as u8);
        }
        for v in s.log_scale {
            a[3].push(((v + 10.0) * 16.0).round().clamp(0.0, 255.0) as u8);
        }
        let l = s.rot.iter().map(|v| v * v).sum::<f32>().sqrt();
        let [w, x, y, z] = s.rot.map(|v| v / l);
        let q = [x, -y, -z, w];
        if version >= 3 {
            let big = (0..4)
                .max_by(|&i, &j| q[i].abs().total_cmp(&q[j].abs()))
                .unwrap_or(3);
            let sign = if q[big] < 0.0 { -1.0 } else { 1.0 };
            let mut word = (big as u32) << 30;
            let mut shift = 30;
            for (i, v) in q.iter().enumerate() {
                if i == big {
                    continue;
                }
                shift -= 10;
                let v = v * sign;
                let m = ((v.abs() / std::f32::consts::FRAC_1_SQRT_2) * 511.0)
                    .round()
                    .min(511.0) as u32;
                word |= (m | if v < 0.0 { 0x200 } else { 0 }) << shift;
            }
            a[4].extend_from_slice(&word.to_le_bytes());
        } else {
            let sign = if q[3] < 0.0 { -1.0 } else { 1.0 };
            for v in &q[..3] {
                a[4].push(((v * sign) * 127.5 + 127.5).round().clamp(0.0, 255.0) as u8);
            }
        }
        // The basis functions odd in y and z change sign under (x, −y, −z).
        let flip = [
            -1.0f32, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0,
        ];
        for (j, f) in flip.iter().enumerate().take(k) {
            for ch in 0..3 {
                let v = s.rest[ch * 15 + j] * f;
                a[5].push((v * 128.0 + 128.0).round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    a
}

/// The scene as a legacy (gzip) `.spz` of `version` 2 or 3.
pub fn spz_file(g: &[Gauss], version: u32, degree: u8) -> Vec<u8> {
    let mut raw = b"NGSP".to_vec();
    raw.extend_from_slice(&version.to_le_bytes());
    raw.extend_from_slice(&(g.len() as u32).to_le_bytes());
    raw.extend_from_slice(&[degree, 12, 0, 0]);
    for a in spz_arrays(g, version, degree) {
        raw.extend_from_slice(&a);
    }
    gzip_stored(&raw)
}

/// The scene as a version 4 `.spz` (one raw-block Zstandard frame per attribute).
pub fn spz4_file(g: &[Gauss], degree: u8) -> Vec<u8> {
    let arrays: Vec<Vec<u8>> = spz_arrays(g, 4, degree)
        .into_iter()
        .filter(|a| !a.is_empty())
        .collect();
    let mut out = b"NGSP".to_vec();
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&(g.len() as u32).to_le_bytes());
    out.extend_from_slice(&[degree, 12, 0, arrays.len() as u8]);
    out.extend_from_slice(&32u32.to_le_bytes());
    out.extend_from_slice(&[0; 12]);
    let frames: Vec<Vec<u8>> = arrays.iter().map(|a| zstd_raw(a)).collect();
    for (f, a) in frames.iter().zip(&arrays) {
        out.extend_from_slice(&(f.len() as u64).to_le_bytes());
        out.extend_from_slice(&(a.len() as u64).to_le_bytes());
    }
    for f in frames {
        out.extend_from_slice(&f);
    }
    out
}

/// A Zstandard frame of raw blocks (no compression), with its content size.
pub fn zstd_raw(data: &[u8]) -> Vec<u8> {
    let mut out = 0xFD2F_B528u32.to_le_bytes().to_vec();
    // Single segment, 8-byte content size.
    out.push(0b1110_0000);
    out.extend_from_slice(&(data.len() as u64).to_le_bytes());
    let blocks: Vec<&[u8]> = if data.is_empty() {
        vec![&[]]
    } else {
        data.chunks(100_000).collect()
    };
    for (i, b) in blocks.iter().enumerate() {
        let last = u32::from(i + 1 == blocks.len());
        let h = last | ((b.len() as u32) << 3);
        out.extend_from_slice(&h.to_le_bytes()[..3]);
        out.extend_from_slice(b);
    }
    out
}

/// A gzip stream of stored deflate blocks.
pub fn gzip_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![&[]]
    } else {
        data.chunks(65535).collect()
    };
    for (i, c) in chunks.iter().enumerate() {
        out.push(u8::from(i + 1 == chunks.len()));
        out.extend_from_slice(&(c.len() as u16).to_le_bytes());
        out.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        out.extend_from_slice(c);
    }
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                0xedb8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
    }
    out.extend_from_slice(&(!crc).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}
