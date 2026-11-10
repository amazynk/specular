//! ASTC block decoder (Khronos ASTC specification, 2D footprints 4×4 … 12×12 and 3D footprints
//! 3×3×3 … 6×6×6), LDR and HDR profiles.
//!
//! A block is 128 bits, read as a little-endian `u128`. It is either a void-extent block (one
//! constant colour) or a normal block: block mode (weight grid size, weight range, dual
//! plane), partition count and seed, colour endpoint modes (CEMs), colour endpoint values and
//! texel weights, both in bounded integer sequence encoding (BISE). Weights are stored from the
//! top of the block down, bit reversed.
//!
//! 3D blocks have their own block-mode table, a 3D partition hash and six 9-bit void-extent
//! coordinates, and infill the weight grid by simplex interpolation; one slice is output.
//!
//! Per texel: the weight grid is bilinearly infilled to the block footprint, the texel's
//! partition comes from the partition hash, and each channel interpolates its partition's two
//! 16-bit endpoints with a 6-bit weight (`(c0·(64 − w) + c1·w + 32) >> 6`).
//!
//! Output conversion:
//! * LDR profile, RGBA8: the top 8 bits of the interpolated 16-bit value (the
//!   `decode_unorm8` behaviour), not a rounded `v / 257`. In sRGB mode all four endpoint
//!   channels (alpha included) expand as `e << 8 | 0x80`, otherwise as `e << 8 | e`. This is
//!   bit for bit what astcenc writes for 8-bit outputs (`-dl` and `-ds`). HDR endpoints and
//!   error blocks give opaque magenta.
//! * HDR profile, half-float bits: LDR channels convert their 16-bit value `C` to half as
//!   `C / 65536` with the mantissa truncated (0xFFFF is exactly 1.0), as astcenc does; HDR
//!   channels go through the spec's LNS → FP16 mapping (values above the half range clamp to
//!   0x7BFF). Error blocks give opaque magenta (1, 0, 1, 1).

/// Every 2D footprint of the specification.
const FOOTPRINTS: [(usize, usize); 14] = [
    (4, 4),
    (5, 4),
    (5, 5),
    (6, 5),
    (6, 6),
    (8, 5),
    (8, 6),
    (8, 8),
    (10, 5),
    (10, 6),
    (10, 8),
    (10, 10),
    (12, 10),
    (12, 12),
];

/// Whether bw×bh is a valid 2D ASTC footprint.
pub(crate) fn valid_footprint(bw: usize, bh: usize) -> bool {
    FOOTPRINTS.contains(&(bw, bh))
}

/// Every 3D footprint of the specification.
const FOOTPRINTS_3D: [(usize, usize, usize); 10] = [
    (3, 3, 3),
    (4, 3, 3),
    (4, 4, 3),
    (4, 4, 4),
    (5, 4, 4),
    (5, 5, 4),
    (5, 5, 5),
    (6, 5, 5),
    (6, 6, 5),
    (6, 6, 6),
];

/// Whether bw×bh×bd is a valid ASTC footprint (bd 1: 2D).
pub(crate) fn valid_footprint_3d(bw: usize, bh: usize, bd: usize) -> bool {
    if bd <= 1 {
        valid_footprint(bw, bh)
    } else {
        FOOTPRINTS_3D.contains(&(bw, bh, bd))
    }
}

const MAX_TEXELS: usize = 216;
const ERROR_LDR: [u8; 4] = [255, 0, 255, 255];
const HALF_ONE: u16 = 0x3c00;
const ERROR_HDR: [u16; 4] = [HALF_ONE, 0, HALF_ONE, HALF_ONE];

// ---------------------------------------------------------------------------------------------
// Bounded integer sequence encoding
// ---------------------------------------------------------------------------------------------

/// Quantisation ranges in specification order: (levels, trits, quints, plain bits).
pub(crate) const RANGES: [(u16, bool, bool, u32); 21] = [
    (2, false, false, 1),
    (3, true, false, 0),
    (4, false, false, 2),
    (5, false, true, 0),
    (6, true, false, 1),
    (8, false, false, 3),
    (10, false, true, 1),
    (12, true, false, 2),
    (16, false, false, 4),
    (20, false, true, 2),
    (24, true, false, 3),
    (32, false, false, 5),
    (40, false, true, 3),
    (48, true, false, 4),
    (64, false, false, 6),
    (80, false, true, 4),
    (96, true, false, 5),
    (128, false, false, 7),
    (160, false, true, 5),
    (192, true, false, 6),
    (256, false, false, 8),
];

/// Bits used by `n` values of range `r`.
pub(crate) const fn ise_bits(n: u32, r: usize) -> u32 {
    let (_, t, q, b) = RANGES[r];
    n * b
        + if t {
            (8 * n).div_ceil(5)
        } else if q {
            (7 * n).div_ceil(3)
        } else {
            0
        }
}

const fn bit(v: usize, k: u32) -> u8 {
    ((v >> k) & 1) as u8
}

/// Trit block (8 bits) → five trits.
pub(crate) const TRITS: [[u8; 5]; 256] = {
    let mut t = [[0u8; 5]; 256];
    let mut i = 0;
    while i < 256 {
        let (c, t3, t4);
        if (i >> 2) & 7 == 7 {
            c = (((i >> 5) & 7) << 2) | (i & 3);
            t4 = 2;
            t3 = 2;
        } else {
            c = i & 0x1f;
            if (i >> 5) & 3 == 3 {
                t4 = 2;
                t3 = bit(i, 7);
            } else {
                t4 = bit(i, 7);
                t3 = ((i >> 5) & 3) as u8;
            }
        }
        let (t0, t1, t2);
        if c & 3 == 3 {
            t2 = 2;
            t1 = bit(c, 4);
            t0 = (bit(c, 3) << 1) | (bit(c, 2) & (bit(c, 3) ^ 1));
        } else if (c >> 2) & 3 == 3 {
            t2 = 2;
            t1 = 2;
            t0 = (c & 3) as u8;
        } else {
            t2 = bit(c, 4);
            t1 = ((c >> 2) & 3) as u8;
            t0 = (bit(c, 1) << 1) | (bit(c, 0) & (bit(c, 1) ^ 1));
        }
        t[i] = [t0, t1, t2, t3, t4];
        i += 1;
    }
    t
};

/// Quint block (7 bits) → three quints.
pub(crate) const QUINTS: [[u8; 3]; 128] = {
    let mut t = [[0u8; 3]; 128];
    let mut i = 0;
    while i < 128 {
        let (q0, q1, q2);
        if (i >> 1) & 3 == 3 && (i >> 5) & 3 == 0 {
            q2 = (bit(i, 0) << 2)
                | ((bit(i, 4) & (bit(i, 0) ^ 1)) << 1)
                | (bit(i, 3) & (bit(i, 0) ^ 1));
            q1 = 4;
            q0 = 4;
        } else {
            let c;
            if (i >> 1) & 3 == 3 {
                q2 = 4;
                c = (((i >> 3) & 3) << 3) | ((!(i >> 5) & 3) << 1) | (i & 1);
            } else {
                q2 = ((i >> 5) & 3) as u8;
                c = i & 0x1f;
            }
            if c & 7 == 5 {
                q1 = 4;
                q0 = ((c >> 3) & 3) as u8;
            } else {
                q1 = ((c >> 3) & 3) as u8;
                q0 = (c & 7) as u8;
            }
        }
        t[i] = [q0, q1, q2];
        i += 1;
    }
    t
};

/// Decode `out.len()` values of range `r` from the low bits of `seq` (bits past the
/// sequence must already be zero).
pub(crate) fn ise_decode(seq: u128, r: usize, out: &mut [u8]) {
    let (_, t, q, nb) = RANGES[r];
    let mut s = seq;
    let mut take = |n: u32| -> u32 {
        let v = (s & ((1u128 << n) - 1)) as u32;
        s >>= n;
        v
    };
    let n = out.len();
    if t {
        for g in (0..n).step_by(5) {
            let mut m = [0u32; 5];
            let mut tb = 0u32;
            m[0] = take(nb);
            tb |= take(2);
            m[1] = take(nb);
            tb |= take(2) << 2;
            m[2] = take(nb);
            tb |= take(1) << 4;
            m[3] = take(nb);
            tb |= take(2) << 5;
            m[4] = take(nb);
            tb |= take(1) << 7;
            let tr = TRITS[tb as usize];
            for k in 0..5.min(n - g) {
                out[g + k] = ((u32::from(tr[k]) << nb) | m[k]) as u8;
            }
        }
    } else if q {
        for g in (0..n).step_by(3) {
            let mut m = [0u32; 3];
            let mut qb = 0u32;
            m[0] = take(nb);
            qb |= take(3);
            m[1] = take(nb);
            qb |= take(2) << 3;
            m[2] = take(nb);
            qb |= take(2) << 5;
            let qu = QUINTS[qb as usize];
            for k in 0..3.min(n - g) {
                out[g + k] = ((u32::from(qu[k]) << nb) | m[k]) as u8;
            }
        }
    } else {
        for v in out.iter_mut() {
            *v = take(nb) as u8;
        }
    }
}

/// Colour value unquantisation to 0…255, per range (index = BISE value).
pub(crate) fn colour_unquant() -> &'static [[u8; 256]; 21] {
    static T: std::sync::OnceLock<Box<[[u8; 256]; 21]>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = Box::new([[0u8; 256]; 21]);
        for (r, row) in t.iter_mut().enumerate() {
            let (levels, tr, qu, nb) = RANGES[r];
            if (tr || qu) && nb == 0 {
                // Ranges 0..2 and 0..4 are never used for colour.
                continue;
            }
            for v in 0..levels as u32 {
                row[v as usize] = if !tr && !qu {
                    // Bit replication to 8 bits.
                    let mut x = v << (8 - nb);
                    let mut k = nb;
                    while k < 8 {
                        x |= x >> k;
                        k *= 2;
                    }
                    x as u8
                } else {
                    let d = v >> nb;
                    let m = v & ((1 << nb) - 1);
                    let bit = |k: u32| (m >> k) & 1;
                    let a = if m & 1 != 0 { 0x1ff } else { 0 };
                    let (b, c) = match (tr, nb) {
                        (true, 1) => (0, 204),
                        (false, 1) => (0, 113),
                        (true, 2) => {
                            let b = bit(1);
                            ((b << 8) | (b << 4) | (b << 2) | (b << 1), 93)
                        }
                        (false, 2) => {
                            let b = bit(1);
                            ((b << 8) | (b << 3) | (b << 2), 54)
                        }
                        (true, 3) => {
                            let (b, c) = (bit(1), bit(2));
                            ((c << 8) | (b << 7) | (c << 3) | (b << 2) | (c << 1) | b, 44)
                        }
                        (false, 3) => {
                            let (b, c) = (bit(1), bit(2));
                            ((c << 8) | (b << 7) | (c << 2) | (b << 1) | c, 26)
                        }
                        (true, 4) => {
                            let cb = (m >> 1) & 7;
                            ((cb << 6) | cb, 22)
                        }
                        (false, 4) => {
                            let cb = (m >> 1) & 7;
                            ((cb << 6) | (cb >> 1), 13)
                        }
                        (true, 5) => {
                            let cb = (m >> 1) & 15;
                            ((cb << 5) | (cb >> 2), 11)
                        }
                        (false, 5) => {
                            let cb = (m >> 1) & 15;
                            ((cb << 5) | (cb >> 3), 6)
                        }
                        (true, 6) => {
                            let cb = (m >> 1) & 31;
                            ((cb << 4) | (cb >> 4), 5)
                        }
                        _ => unreachable!(),
                    };
                    let mut x = d * c + b;
                    x ^= a;
                    ((a & 0x80) | (x >> 2)) as u8
                };
            }
        }
        t
    })
}

/// Weight unquantisation to 0…64, per weight range 0…11 (index = BISE value).
pub(crate) const WEIGHT_UNQUANT: [[u8; 32]; 12] = {
    let mut t = [[0u8; 32]; 12];
    let mut r = 0;
    while r < 12 {
        let (levels, tr, qu, nb) = RANGES[r];
        let mut v = 0u32;
        while v < levels as u32 {
            let x = if !tr && !qu {
                // Bit replication to 6 bits.
                let mut x = v << (6 - nb);
                let mut k = nb;
                while k < 6 {
                    x |= x >> k;
                    k *= 2;
                }
                x
            } else if nb == 0 {
                if tr {
                    [0, 32, 63][v as usize]
                } else {
                    [0, 16, 32, 47, 63][v as usize]
                }
            } else {
                let d = v >> nb;
                let m = v & ((1 << nb) - 1);
                let bb = (m >> 1) & 1;
                let a = if m & 1 != 0 { 0x7f } else { 0 };
                let (b, c) = if tr {
                    match nb {
                        1 => (0, 50),
                        2 => ((bb << 6) | (bb << 2) | bb, 23),
                        _ => {
                            let cc = (m >> 2) & 1;
                            ((cc << 6) | (bb << 5) | (cc << 1) | bb, 11)
                        }
                    }
                } else if nb == 1 {
                    (0, 28)
                } else {
                    ((bb << 6) | (bb << 1), 13)
                };
                let mut x = d * c + b;
                x ^= a;
                (a & 0x20) | (x >> 2)
            };
            t[r][v as usize] = if x > 32 { x as u8 + 1 } else { x as u8 };
            v += 1;
        }
        r += 1;
    }
    t
};

// ---------------------------------------------------------------------------------------------
// Partitions
// ---------------------------------------------------------------------------------------------

fn hash52(mut p: u32) -> u32 {
    p ^= p >> 15;
    p = p.wrapping_sub(p << 17);
    p = p.wrapping_add(p << 7);
    p = p.wrapping_add(p << 4);
    p ^= p >> 5;
    p = p.wrapping_add(p << 16);
    p ^= p >> 7;
    p ^= p >> 3;
    p ^= p << 6;
    p ^= p >> 17;
    p
}

/// Partition of every texel of a bw×bh×bd block (2D: bd = 1, z = 0).
fn partitions(seed: u32, count: u32, bw: usize, bh: usize, bd: usize, out: &mut [u8; MAX_TEXELS]) {
    let small = bw * bh * bd < 31;
    let seed = seed + (count - 1) * 1024;
    let r = hash52(seed);
    let mut s = [
        r & 0xf,
        (r >> 4) & 0xf,
        (r >> 8) & 0xf,
        (r >> 12) & 0xf,
        (r >> 16) & 0xf,
        (r >> 20) & 0xf,
        (r >> 24) & 0xf,
        (r >> 28) & 0xf,
        (r >> 18) & 0xf,
        (r >> 22) & 0xf,
        (r >> 26) & 0xf,
        r.rotate_left(2) & 0xf,
    ];
    for v in &mut s {
        *v *= *v;
    }
    let (sh1, sh2) = if seed & 1 != 0 {
        (
            if seed & 2 != 0 { 4 } else { 5 },
            if count == 3 { 6 } else { 5 },
        )
    } else {
        (
            if count == 3 { 6 } else { 5 },
            if seed & 2 != 0 { 4 } else { 5 },
        )
    };
    let sh3 = if seed & 0x10 != 0 { sh1 } else { sh2 };
    for (k, v) in s.iter_mut().enumerate() {
        *v >>= if k >= 8 {
            sh3
        } else if k % 2 == 0 {
            sh1
        } else {
            sh2
        };
    }
    for (i, o) in out[..bw * bh * bd].iter_mut().enumerate() {
        let (x, y, z) = (i % bw, (i / bw) % bh, i / (bw * bh));
        {
            let m = if small { 2 } else { 1 };
            let (xs, ys, zs) = (x as u32 * m, y as u32 * m, z as u32 * m);
            let a = (s[0] * xs + s[1] * ys + s[10] * zs + (r >> 14)) & 0x3f;
            let b = (s[2] * xs + s[3] * ys + s[11] * zs + (r >> 10)) & 0x3f;
            let c = if count >= 3 {
                (s[4] * xs + s[5] * ys + s[8] * zs + (r >> 6)) & 0x3f
            } else {
                0
            };
            let d = if count >= 4 {
                (s[6] * xs + s[7] * ys + s[9] * zs + (r >> 2)) & 0x3f
            } else {
                0
            };
            *o = if a >= b && a >= c && a >= d {
                0
            } else if b >= c && b >= d {
                1
            } else if c >= d {
                2
            } else {
                3
            };
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Colour endpoints
// ---------------------------------------------------------------------------------------------

/// A partition's endpoints as 16-bit interpolation inputs, and which channels are HDR (LNS).
#[derive(Clone, Copy, Default)]
struct Endpoints {
    e0: [u16; 4],
    e1: [u16; 4],
    hdr_rgb: bool,
    hdr_a: bool,
}

pub(crate) fn bit_transfer_signed(a: i32, b: i32) -> (i32, i32) {
    let b = (b >> 1) | (a & 0x80);
    let mut a = (a >> 1) & 0x3f;
    if a & 0x20 != 0 {
        a -= 0x40;
    }
    (a, b)
}

fn blue_contract(c: [i32; 4]) -> [i32; 4] {
    [(c[0] + c[2]) >> 1, (c[1] + c[2]) >> 1, c[2], c[3]]
}

fn clamp8(c: [i32; 4]) -> [i32; 4] {
    c.map(|v| v.clamp(0, 255))
}

/// LDR endpoint pair (8-bit) of CEMs 0, 1, 4, 5, 6, 8, 9, 10, 12, 13.
pub(crate) fn ldr_endpoints(cem: u8, v: &[i32]) -> ([i32; 4], [i32; 4]) {
    match cem {
        0 => ([v[0], v[0], v[0], 255], [v[1], v[1], v[1], 255]),
        1 => {
            let l0 = (v[0] >> 2) | (v[1] & 0xc0);
            let l1 = (l0 + (v[1] & 0x3f)).min(255);
            ([l0, l0, l0, 255], [l1, l1, l1, 255])
        }
        4 => ([v[0], v[0], v[0], v[2]], [v[1], v[1], v[1], v[3]]),
        5 => {
            let (d1, b0) = bit_transfer_signed(v[1], v[0]);
            let (d3, b2) = bit_transfer_signed(v[3], v[2]);
            let l1 = b0 + d1;
            ([b0, b0, b0, b2], clamp8([l1, l1, l1, b2 + d3]))
        }
        6 => (
            [
                (v[0] * v[3]) >> 8,
                (v[1] * v[3]) >> 8,
                (v[2] * v[3]) >> 8,
                255,
            ],
            [v[0], v[1], v[2], 255],
        ),
        8 | 12 => {
            let (a0, a1) = if cem == 12 { (v[6], v[7]) } else { (255, 255) };
            if v[1] + v[3] + v[5] >= v[0] + v[2] + v[4] {
                ([v[0], v[2], v[4], a0], [v[1], v[3], v[5], a1])
            } else {
                (
                    blue_contract([v[1], v[3], v[5], a1]),
                    blue_contract([v[0], v[2], v[4], a0]),
                )
            }
        }
        9 | 13 => {
            let (d1, b0) = bit_transfer_signed(v[1], v[0]);
            let (d3, b2) = bit_transfer_signed(v[3], v[2]);
            let (d5, b4) = bit_transfer_signed(v[5], v[4]);
            let (d7, b6) = if cem == 13 {
                bit_transfer_signed(v[7], v[6])
            } else {
                (0, 255)
            };
            if d1 + d3 + d5 >= 0 {
                (
                    clamp8([b0, b2, b4, b6]),
                    clamp8([b0 + d1, b2 + d3, b4 + d5, b6 + d7]),
                )
            } else {
                (
                    clamp8(blue_contract([b0 + d1, b2 + d3, b4 + d5, b6 + d7])),
                    clamp8(blue_contract([b0, b2, b4, b6])),
                )
            }
        }
        10 => (
            [
                (v[0] * v[3]) >> 8,
                (v[1] * v[3]) >> 8,
                (v[2] * v[3]) >> 8,
                v[4],
            ],
            [v[0], v[1], v[2], v[5]],
        ),
        _ => ([0; 4], [0; 4]),
    }
}

/// HDR luminance, CEM 2 (large range) and 3 (small range): 12-bit values.
fn hdr_luminance(cem: u8, v: &[i32]) -> (i32, i32) {
    if cem == 2 {
        if v[1] >= v[0] {
            (v[0] << 4, v[1] << 4)
        } else {
            ((v[1] << 4) + 8, (v[0] << 4) - 8)
        }
    } else {
        let (y0, d) = if v[0] & 0x80 != 0 {
            (
                ((v[1] & 0xe0) << 4) | ((v[0] & 0x7f) << 2),
                (v[1] & 0x1f) << 2,
            )
        } else {
            (
                ((v[1] & 0xf0) << 4) | ((v[0] & 0x7f) << 1),
                (v[1] & 0x0f) << 1,
            )
        };
        (y0, (y0 + d).min(0xfff))
    }
}

/// HDR RGB base + scale, CEM 7: 12-bit values.
fn hdr_rgb_scale(v: &[i32]) -> ([i32; 3], [i32; 3]) {
    let modeval = ((v[0] & 0xc0) >> 6) | ((v[1] & 0x80) >> 5) | ((v[2] & 0x80) >> 4);
    let (majcomp, mode) = if modeval & 0xc != 0xc {
        (modeval >> 2, modeval & 3)
    } else if modeval != 0xf {
        (modeval & 3, 4)
    } else {
        (0, 5)
    };
    let mut red = v[0] & 0x3f;
    let mut green = v[1] & 0x1f;
    let mut blue = v[2] & 0x1f;
    let mut scale = v[3] & 0x1f;
    let x0 = (v[1] >> 6) & 1;
    let x1 = (v[1] >> 5) & 1;
    let x2 = (v[2] >> 6) & 1;
    let x3 = (v[2] >> 5) & 1;
    let x4 = (v[3] >> 7) & 1;
    let x5 = (v[3] >> 6) & 1;
    let x6 = (v[3] >> 5) & 1;
    let ohm = 1 << mode;
    if ohm & 0x30 != 0 {
        green |= x0 << 6;
    }
    if ohm & 0x3a != 0 {
        green |= x1 << 5;
    }
    if ohm & 0x30 != 0 {
        blue |= x2 << 6;
    }
    if ohm & 0x3a != 0 {
        blue |= x3 << 5;
    }
    if ohm & 0x3d != 0 {
        scale |= x6 << 5;
    }
    if ohm & 0x2d != 0 {
        scale |= x5 << 6;
    }
    if ohm & 0x04 != 0 {
        scale |= x4 << 7;
    }
    if ohm & 0x3b != 0 {
        red |= x4 << 6;
    }
    if ohm & 0x04 != 0 {
        red |= x3 << 6;
    }
    if ohm & 0x10 != 0 {
        red |= x5 << 7;
    }
    if ohm & 0x0f != 0 {
        red |= x2 << 7;
    }
    if ohm & 0x05 != 0 {
        red |= x1 << 8;
    }
    if ohm & 0x0a != 0 {
        red |= x0 << 8;
    }
    if ohm & 0x05 != 0 {
        red |= x0 << 9;
    }
    if ohm & 0x02 != 0 {
        red |= x6 << 9;
    }
    if ohm & 0x01 != 0 {
        red |= x3 << 10;
    }
    if ohm & 0x02 != 0 {
        red |= x5 << 10;
    }
    let shamt = [1, 1, 2, 3, 4, 5][mode as usize];
    red <<= shamt;
    green <<= shamt;
    blue <<= shamt;
    scale <<= shamt;
    if mode != 5 {
        green = red - green;
        blue = red - blue;
    }
    match majcomp {
        1 => std::mem::swap(&mut red, &mut green),
        2 => std::mem::swap(&mut red, &mut blue),
        _ => {}
    }
    let c = |x: i32| x.clamp(0, 0xfff);
    (
        [c(red - scale), c(green - scale), c(blue - scale)],
        [c(red), c(green), c(blue)],
    )
}

/// HDR RGB direct, CEM 11: 16-bit values (12-bit << 4, or the 8/7-bit direct forms).
fn hdr_rgb(v: &[i32]) -> ([i32; 3], [i32; 3]) {
    let majcomp = ((v[4] & 0x80) >> 7) | ((v[5] & 0x80) >> 6);
    if majcomp == 3 {
        return (
            [v[0] << 8, v[2] << 8, (v[4] & 0x7f) << 9],
            [v[1] << 8, v[3] << 8, (v[5] & 0x7f) << 9],
        );
    }
    let mode = ((v[1] & 0x80) >> 7) | ((v[2] & 0x80) >> 6) | ((v[3] & 0x80) >> 5);
    let mut va = v[0] | ((v[1] & 0x40) << 2);
    let mut vb0 = v[2] & 0x3f;
    let mut vb1 = v[3] & 0x3f;
    let mut vc = v[1] & 0x3f;
    let mut vd0 = v[4] & 0x7f;
    let mut vd1 = v[5] & 0x7f;
    let dbits = [7, 6, 7, 6, 5, 6, 5, 6][mode as usize];
    let x0 = (v[2] >> 6) & 1;
    let x1 = (v[3] >> 6) & 1;
    let x2 = (v[4] >> 6) & 1;
    let x3 = (v[5] >> 6) & 1;
    let x4 = (v[4] >> 5) & 1;
    let x5 = (v[5] >> 5) & 1;
    let ohm = 1 << mode;
    if ohm & 0xa4 != 0 {
        va |= x0 << 9;
    }
    if ohm & 0x08 != 0 {
        va |= x2 << 9;
    }
    if ohm & 0x50 != 0 {
        va |= x4 << 9;
    }
    if ohm & 0x50 != 0 {
        va |= x5 << 10;
    }
    if ohm & 0xa0 != 0 {
        va |= x1 << 10;
    }
    if ohm & 0xc0 != 0 {
        va |= x2 << 11;
    }
    if ohm & 0x04 != 0 {
        vc |= x1 << 6;
    }
    if ohm & 0xe8 != 0 {
        vc |= x3 << 6;
    }
    if ohm & 0x20 != 0 {
        vc |= x2 << 7;
    }
    if ohm & 0x5b != 0 {
        vb0 |= x0 << 6;
        vb1 |= x1 << 6;
    }
    if ohm & 0x12 != 0 {
        vb0 |= x2 << 7;
        vb1 |= x3 << 7;
    }
    let shamt = (mode >> 1) ^ 3;
    va <<= shamt;
    vb0 <<= shamt;
    vb1 <<= shamt;
    vc <<= shamt;
    // Sign-extend the dbits-wide deltas.
    vd0 = ((vd0 << (32 - dbits)) >> (32 - dbits)) << shamt;
    vd1 = ((vd1 << (32 - dbits)) >> (32 - dbits)) << shamt;
    let c = |x: i32| x.clamp(0, 0xfff);
    let mut e1 = [c(va), c(va - vb0), c(va - vb1)];
    let mut e0 = [c(va - vc), c(va - vb0 - vc - vd0), c(va - vb1 - vc - vd1)];
    match majcomp {
        1 => {
            e0.swap(0, 1);
            e1.swap(0, 1);
        }
        2 => {
            e0.swap(0, 2);
            e1.swap(0, 2);
        }
        _ => {}
    }
    (e0.map(|x| x << 4), e1.map(|x| x << 4))
}

/// HDR alpha, CEM 15: 12-bit values.
fn hdr_alpha(v6: i32, v7: i32) -> (i32, i32) {
    let mode = ((v6 >> 7) & 1) | ((v7 >> 6) & 2);
    let (mut a, mut b) = (v6 & 0x7f, v7 & 0x7f);
    if mode == 3 {
        return (a << 5, b << 5);
    }
    a |= (b << (mode + 1)) & 0x780;
    b &= 0x3f >> mode;
    b ^= 0x20 >> mode;
    b -= 0x20 >> mode;
    a <<= 4 - mode;
    b <<= 4 - mode;
    b += a;
    (a, b.clamp(0, 0xfff))
}

/// Decode a partition's endpoints from its colour values.
fn endpoints(cem: u8, v: &[i32], srgb: bool) -> Endpoints {
    let ldr = |e: [i32; 4], srgb: bool| -> [u16; 4] {
        // sRGB decode mode expands with 0x80 on every channel, alpha included (as astcenc).
        e.map(|x| {
            let x = x as u16;
            (x << 8) | if srgb { 0x80 } else { x }
        })
    };
    match cem {
        2 | 3 => {
            let (y0, y1) = hdr_luminance(cem, v);
            let (y0, y1) = ((y0 << 4) as u16, (y1 << 4) as u16);
            Endpoints {
                e0: [y0, y0, y0, 0x7800],
                e1: [y1, y1, y1, 0x7800],
                hdr_rgb: true,
                hdr_a: true,
            }
        }
        7 => {
            let (a, b) = hdr_rgb_scale(v);
            Endpoints {
                e0: [
                    (a[0] << 4) as u16,
                    (a[1] << 4) as u16,
                    (a[2] << 4) as u16,
                    0x7800,
                ],
                e1: [
                    (b[0] << 4) as u16,
                    (b[1] << 4) as u16,
                    (b[2] << 4) as u16,
                    0x7800,
                ],
                hdr_rgb: true,
                hdr_a: true,
            }
        }
        11 | 14 | 15 => {
            let (a, b) = hdr_rgb(v);
            let (a0, a1, hdr_a) = match cem {
                11 => (0x7800, 0x7800, true),
                14 => (
                    ((v[6] << 8) | v[6]) as u16,
                    ((v[7] << 8) | v[7]) as u16,
                    false,
                ),
                _ => {
                    let (x, y) = hdr_alpha(v[6], v[7]);
                    ((x << 4) as u16, (y << 4) as u16, true)
                }
            };
            Endpoints {
                e0: [a[0] as u16, a[1] as u16, a[2] as u16, a0],
                e1: [b[0] as u16, b[1] as u16, b[2] as u16, a1],
                hdr_rgb: true,
                hdr_a,
            }
        }
        _ => {
            let (a, b) = ldr_endpoints(cem, v);
            Endpoints {
                e0: ldr(a, srgb),
                e1: ldr(b, srgb),
                hdr_rgb: false,
                hdr_a: false,
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Block decode
// ---------------------------------------------------------------------------------------------

/// Result of the shared decode: either a constant colour or interpolated 16-bit texels.
enum Block {
    Error,
    /// Void extent: UNORM16 (LDR) values.
    ConstLdr([u16; 4]),
    /// Void extent: half-float values.
    ConstHdr([u16; 4]),
    Texels {
        parts: [Endpoints; 4],
    },
}

struct Texels {
    /// Interpolated 16-bit values per texel.
    c: [[u16; 4]; MAX_TEXELS],
    /// Partition per texel.
    part: [u8; MAX_TEXELS],
}

/// Block mode → (grid width, grid height, weight range, dual plane), or None if reserved.
pub(crate) fn block_mode(m: u32) -> Option<(usize, usize, usize, bool)> {
    let a = ((m >> 5) & 3) as usize;
    let mut h = (m >> 9) & 1;
    let mut d = (m >> 10) & 1;
    let mut r = (m >> 4) & 1;
    let (x, y);
    if m & 3 != 0 {
        r |= (m & 3) << 1;
        let b = ((m >> 7) & 3) as usize;
        match (m >> 2) & 3 {
            0 => (x, y) = (b + 4, a + 2),
            1 => (x, y) = (b + 8, a + 2),
            2 => (x, y) = (a + 2, b + 8),
            _ => {
                let b = b & 1;
                if m & 0x100 != 0 {
                    (x, y) = (b + 2, a + 2);
                } else {
                    (x, y) = (a + 2, b + 6);
                }
            }
        }
    } else {
        r |= ((m >> 2) & 3) << 1;
        if (m >> 2) & 3 == 0 {
            return None;
        }
        let b = ((m >> 9) & 3) as usize;
        match (m >> 7) & 3 {
            0 => (x, y) = (12, a + 2),
            1 => (x, y) = (a + 2, 12),
            2 => {
                (x, y) = (a + 6, b + 6);
                d = 0;
                h = 0;
            }
            _ => match (m >> 5) & 3 {
                0 => (x, y) = (6, 10),
                1 => (x, y) = (10, 6),
                _ => return None,
            },
        }
    }
    let range = (r as usize) - 2 + 6 * h as usize;
    Some((x, y, range, d != 0))
}

/// 3D block mode → (grid width, height, depth, weight range, dual plane), or None if
/// reserved.
fn block_mode_3d(m: u32) -> Option<(usize, usize, usize, usize, bool)> {
    let a = ((m >> 5) & 3) as usize;
    let mut h = (m >> 9) & 1;
    let mut d = (m >> 10) & 1;
    let mut r = (m >> 4) & 1;
    let (x, y, z);
    if m & 3 != 0 {
        r |= (m & 3) << 1;
        let b = ((m >> 7) & 3) as usize;
        let c = ((m >> 2) & 3) as usize;
        (x, y, z) = (a + 2, b + 2, c + 2);
    } else {
        r |= ((m >> 2) & 3) << 1;
        if (m >> 2) & 3 == 0 {
            return None;
        }
        let b = ((m >> 9) & 3) as usize;
        if (m >> 7) & 3 != 3 {
            d = 0;
            h = 0;
        }
        match (m >> 7) & 3 {
            0 => (x, y, z) = (6, b + 2, a + 2),
            1 => (x, y, z) = (a + 2, 6, b + 2),
            2 => (x, y, z) = (a + 2, b + 2, 6),
            _ => match (m >> 5) & 3 {
                0 => (x, y, z) = (6, 2, 2),
                1 => (x, y, z) = (2, 6, 2),
                2 => (x, y, z) = (2, 2, 6),
                _ => return None,
            },
        }
    }
    let range = (r as usize) - 2 + 6 * h as usize;
    Some((x, y, z, range, d != 0))
}

/// Grid size, weight range and dual plane of a 2D (`bd` 1) or 3D block mode.
fn grid(mode: u32, bd: usize) -> Option<(usize, usize, usize, usize, bool)> {
    if bd > 1 {
        block_mode_3d(mode)
    } else {
        block_mode(mode).map(|(x, y, r, d)| (x, y, 1, r, d))
    }
}

fn decode(bits: u128, bw: usize, bh: usize, bd: usize, srgb: bool, tx: &mut Texels) -> Block {
    let mode = (bits & 0x7ff) as u32;
    if mode & 0x1ff == 0x1fc {
        return void_extent(bits, bd > 1);
    }
    let Some((gw, gh, gd, wrange, dual)) = grid(mode, bd) else {
        return Block::Error;
    };
    if gw > bw || gh > bh || gd > bd {
        return Block::Error;
    }
    let nparts = ((bits >> 11) & 3) as u32 + 1;
    let planes = if dual { 2 } else { 1 };
    let nweights = gw * gh * gd * planes;
    if nweights > 64 || (dual && nparts == 4) {
        return Block::Error;
    }
    let wbits = ise_bits(nweights as u32, wrange);
    if !(24..=96).contains(&wbits) {
        return Block::Error;
    }
    let below_weights = 128 - wbits;

    // Colour endpoint modes.
    let mut cems = [0u8; 4];
    let (colour_start, extra) = if nparts == 1 {
        cems[0] = ((bits >> 13) & 0xf) as u8;
        (17, 0)
    } else {
        let field = ((bits >> 23) & 0x3f) as u32;
        if field & 3 == 0 {
            let c = (field >> 2) as u8;
            cems[..nparts as usize].fill(c);
            (29, 0)
        } else {
            let extra = 3 * nparts - 4;
            let hi = ((bits >> (below_weights - extra)) & ((1u128 << extra) - 1)) as u32;
            let enc = field | (hi << 6);
            let base = (enc & 3) - 1;
            for (i, c) in cems[..nparts as usize].iter_mut().enumerate() {
                let cbit = (enc >> (2 + i)) & 1;
                let m = (enc >> (2 + nparts as usize + 2 * i)) & 3;
                *c = (((base + cbit) << 2) | m) as u8;
            }
            (29, extra)
        }
    };
    let ccs_bits = if dual { 2 } else { 0 };
    let Some(colour_bits) = below_weights.checked_sub(extra + ccs_bits + colour_start) else {
        return Block::Error;
    };
    let ccs = if dual {
        ((bits >> (below_weights - extra - 2)) & 3) as usize
    } else {
        4
    };
    let nvals: u32 = cems[..nparts as usize]
        .iter()
        .map(|&c| 2 * (u32::from(c >> 2) + 1))
        .sum();
    if nvals > 18 {
        return Block::Error;
    }
    let Some(crange) = (4..21).rev().find(|&r| ise_bits(nvals, r) <= colour_bits) else {
        return Block::Error;
    };

    // Colour values.
    let mut vals = [0u8; 18];
    let clen = ise_bits(nvals, crange);
    let seq = (bits >> colour_start) & ((1u128 << clen) - 1);
    ise_decode(seq, crange, &mut vals[..nvals as usize]);
    let uq = &colour_unquant()[crange];
    let mut parts = [Endpoints::default(); 4];
    let mut at = 0;
    for (p, &c) in parts.iter_mut().zip(&cems[..nparts as usize]) {
        let n = 2 * (usize::from(c >> 2) + 1);
        let mut v = [0i32; 8];
        for (d, &s) in v.iter_mut().zip(&vals[at..at + n]) {
            *d = i32::from(uq[s as usize]);
        }
        at += n;
        *p = endpoints(c, &v[..n], srgb);
    }

    // Weights: bit reversed from the top of the block.
    let rev = bits.reverse_bits() & ((1u128 << wbits) - 1);
    let mut wq = [0u8; 64];
    ise_decode(rev, wrange, &mut wq[..nweights]);
    let wu = &WEIGHT_UNQUANT[wrange];
    for w in &mut wq[..nweights] {
        *w = wu[*w as usize];
    }

    // Partition map.
    if nparts > 1 {
        let seed = ((bits >> 13) & 0x3ff) as u32;
        partitions(seed, nparts, bw, bh, bd, &mut tx.part);
    } else {
        tx.part[..bw * bh * bd].fill(0);
    }
    if bd > 1 {
        infill_3d(&wq, (gw, gh, gd), (bw, bh, bd), planes, ccs, &parts, tx);
        return Block::Texels { parts };
    }

    // Infill and interpolate.
    let full = gw == bw && gh == bh;
    let ds = (1024 + bw as u32 / 2) / (bw as u32 - 1);
    let dt = (1024 + bh as u32 / 2) / (bh as u32 - 1);
    let mut col = [(0usize, 0u32); 12];
    for (s, c) in col[..bw].iter_mut().enumerate() {
        let g = (ds * s as u32 * (gw as u32 - 1) + 32) >> 6;
        *c = ((g >> 4) as usize, g & 0xf);
    }
    for t in 0..bh {
        let g = (dt * t as u32 * (gh as u32 - 1) + 32) >> 6;
        let (jt, ft) = ((g >> 4) as usize, g & 0xf);
        for (s, &(js, fs)) in col[..bw].iter().enumerate() {
            let i = t * bw + s;
            let mut w = [0u32; 2];
            for (pl, w) in w.iter_mut().enumerate().take(planes) {
                *w = if full {
                    u32::from(wq[i * planes + pl])
                } else {
                    let v0 = js + jt * gw;
                    let at = |k: usize| u32::from(wq[(k.min(gw * gh - 1)) * planes + pl]);
                    let w11 = (fs * ft + 8) >> 4;
                    let w10 = ft - w11;
                    let w01 = fs - w11;
                    let w00 = 16 + w11 - fs - ft;
                    (at(v0) * w00
                        + at(v0 + 1) * w01
                        + at(v0 + gw) * w10
                        + at(v0 + gw + 1) * w11
                        + 8)
                        >> 4
                };
            }
            let e = &parts[tx.part[i] as usize];
            for ch in 0..4 {
                let wt = if ch == ccs { w[1] } else { w[0] };
                let (a, b) = (u32::from(e.e0[ch]), u32::from(e.e1[ch]));
                tx.c[i][ch] = ((a * (64 - wt) + b * wt + 32) >> 6) as u16;
            }
        }
    }
    Block::Texels { parts }
}

/// 3D weight infill (simplex interpolation over the grid, as the specification gives it) and
/// endpoint interpolation for every texel of a bw×bh×bd block.
fn infill_3d(
    wq: &[u8; 64],
    (gw, gh, gd): (usize, usize, usize),
    (bw, bh, bd): (usize, usize, usize),
    planes: usize,
    ccs: usize,
    parts: &[Endpoints; 4],
    tx: &mut Texels,
) {
    let step = |n: usize| (1024 + n as u32 / 2) / (n as u32 - 1);
    let (ds, dt, dr) = (step(bw), step(bh), step(bd));
    let n = gw * gh * gd;
    let (row, plane) = (gw, gw * gh);
    for z in 0..bd {
        let gz = (dr * z as u32 * (gd as u32 - 1) + 32) >> 6;
        for y in 0..bh {
            let gy = (dt * y as u32 * (gh as u32 - 1) + 32) >> 6;
            for x in 0..bw {
                let gx = (ds * x as u32 * (gw as u32 - 1) + 32) >> 6;
                let (fs, ft, fp) = (gx & 0xf, gy & 0xf, gz & 0xf);
                let q0 = ((gz >> 4) as usize * gh + (gy >> 4) as usize) * gw + (gx >> 4) as usize;
                let cas =
                    (u32::from(fs > ft) << 2) | (u32::from(ft > fp) << 1) | u32::from(fs > fp);
                let (s1, s2, w) = match cas {
                    7 => (1, row, [16 - fs, fs - ft, ft - fp, fp]),
                    3 => (row, 1, [16 - ft, ft - fs, fs - fp, fp]),
                    5 => (1, plane, [16 - fs, fs - fp, fp - ft, ft]),
                    4 => (plane, 1, [16 - fp, fp - fs, fs - ft, ft]),
                    2 => (row, plane, [16 - ft, ft - fp, fp - fs, fs]),
                    _ => (plane, row, [16 - fp, fp - ft, ft - fs, fs]),
                };
                let q = [q0, q0 + s1, q0 + s1 + s2, q0 + row + plane + 1];
                let i = (z * bh + y) * bw + x;
                let mut wt = [0u32; 2];
                for (pl, o) in wt.iter_mut().enumerate().take(planes) {
                    let mut acc = 8;
                    for k in 0..4 {
                        if w[k] > 0 {
                            acc += u32::from(wq[q[k].min(n - 1) * planes + pl]) * w[k];
                        }
                    }
                    *o = acc >> 4;
                }
                let e = &parts[tx.part[i] as usize];
                for ch in 0..4 {
                    let wv = if ch == ccs { wt[1] } else { wt[0] };
                    let (a, b) = (u32::from(e.e0[ch]), u32::from(e.e1[ch]));
                    tx.c[i][ch] = ((a * (64 - wv) + b * wv + 32) >> 6) as u16;
                }
            }
        }
    }
}

fn void_extent(bits: u128, three_d: bool) -> Block {
    let hdr = (bits >> 9) & 1 != 0;
    let c = [
        (bits >> 64) as u16,
        (bits >> 80) as u16,
        (bits >> 96) as u16,
        (bits >> 112) as u16,
    ];
    if three_d {
        // Six 9-bit extent coordinates from bit 10.
        let coord = |k: u32| ((bits >> (10 + 9 * k)) & 0x1ff) as u32;
        let all_ones = (0..6).all(|k| coord(k) == 0x1ff);
        if !all_ones && (coord(0) >= coord(1) || coord(2) >= coord(3) || coord(4) >= coord(5)) {
            return Block::Error;
        }
        return if hdr {
            Block::ConstHdr(c)
        } else {
            Block::ConstLdr(c)
        };
    }
    if (bits >> 10) & 3 != 3 {
        return Block::Error;
    }
    let coord = |k: u32| ((bits >> (12 + 13 * k)) & 0x1fff) as u32;
    let all_ones = (0..4).all(|k| coord(k) == 0x1fff);
    if !all_ones && (coord(0) >= coord(1) || coord(2) >= coord(3)) {
        return Block::Error;
    }
    if hdr {
        Block::ConstHdr(c)
    } else {
        Block::ConstLdr(c)
    }
}

/// Whether a block needs the HDR profile: an HDR void extent, or a normal block with an HDR
/// colour endpoint mode (2, 3, 7, 11, 14, 15). Only the header fields are read.
pub(crate) fn block_is_hdr(block: &[u8], bd: usize) -> bool {
    let bits = read_block(block);
    let mode = (bits & 0x7ff) as u32;
    if mode & 0x1ff == 0x1fc {
        return (bits >> 9) & 1 != 0;
    }
    let Some((gw, gh, gd, wrange, dual)) = grid(mode, bd) else {
        return false;
    };
    let nparts = ((bits >> 11) & 3) as u32 + 1;
    let nweights = gw * gh * gd * if dual { 2 } else { 1 };
    if nweights > 64 {
        return false;
    }
    let wbits = ise_bits(nweights as u32, wrange);
    if !(24..=96).contains(&wbits) {
        return false;
    }
    let hdr = |c: u32| matches!(c, 2 | 3 | 7 | 11 | 14 | 15);
    if nparts == 1 {
        return hdr(((bits >> 13) & 0xf) as u32);
    }
    let field = ((bits >> 23) & 0x3f) as u32;
    if field & 3 == 0 {
        return hdr(field >> 2);
    }
    let extra = 3 * nparts - 4;
    let below = 128 - wbits;
    let hi = ((bits >> (below - extra)) & ((1u128 << extra) - 1)) as u32;
    let enc = field | (hi << 6);
    let base = (enc & 3) - 1;
    (0..nparts as usize).any(|i| {
        let cbit = (enc >> (2 + i)) & 1;
        let m = (enc >> (2 + nparts as usize + 2 * i)) & 3;
        hdr(((base + cbit) << 2) | m)
    })
}

fn read_block(block: &[u8]) -> u128 {
    let mut b = [0u8; 16];
    let n = block.len().min(16);
    b[..n].copy_from_slice(&block[..n]);
    u128::from_le_bytes(b)
}

/// Decode one 16-byte block of bw×bh texels with the LDR profile into RGBA8, row-major,
/// `pitch` bytes per row (out starts at the block's top-left texel). Error blocks (and HDR
/// endpoints in LDR) give the spec error colour (magenta, opaque). A 3D block (`bd` > 1)
/// gives its slice `z`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_ldr(
    block: &[u8],
    bw: usize,
    bh: usize,
    bd: usize,
    z: usize,
    srgb: bool,
    out: &mut [u8],
    pitch: usize,
) {
    let fill = |out: &mut [u8], px: [u8; 4]| {
        for y in 0..bh {
            for x in 0..bw {
                out[y * pitch + x * 4..][..4].copy_from_slice(&px);
            }
        }
    };
    let mut tx = Texels {
        c: [[0; 4]; MAX_TEXELS],
        part: [0; MAX_TEXELS],
    };
    match decode(read_block(block), bw, bh, bd, srgb, &mut tx) {
        Block::Error | Block::ConstHdr(_) => fill(out, ERROR_LDR),
        Block::ConstLdr(c) => fill(out, c.map(|v| (v >> 8) as u8)),
        Block::Texels { parts } => {
            for y in 0..bh {
                for x in 0..bw {
                    let i = (z * bh + y) * bw + x;
                    let e = &parts[tx.part[i] as usize];
                    let px = if e.hdr_rgb || e.hdr_a {
                        ERROR_LDR
                    } else {
                        tx.c[i].map(|v| (v >> 8) as u8)
                    };
                    out[y * pitch + x * 4..][..4].copy_from_slice(&px);
                }
            }
        }
    }
}

/// HDR LNS value (16-bit) → half-float bits.
fn lns_to_half(c: u16) -> u16 {
    let e = c >> 11;
    let m = u32::from(c & 0x7ff);
    let mt = if m < 512 {
        3 * m
    } else if m < 1536 {
        4 * m - 512
    } else {
        5 * m - 2048
    };
    let h = (e << 10) | (mt >> 3) as u16;
    h.min(0x7bff)
}

/// UNORM16 → half bits, as C / 65536 with the mantissa truncated (0xffff maps to exactly 1.0).
/// Truncation rather than round-to-nearest matches astcenc bit for bit.
fn unorm16_to_half(c: u16) -> u16 {
    match c {
        0xffff => HALF_ONE,
        // Below 2^-14: half subnormal, exactly C * 2^8 units of 2^-24.
        0..=3 => c << 8,
        _ => {
            let n = 16 - c.leading_zeros(); // 3..=16
            let mant = ((u32::from(c) << (17 - n)) >> 6) & 0x3ff;
            (((n - 2) << 10) | mant) as u16
        }
    }
}

/// Decode with the HDR profile into RGBA half-float bits, row-major, `pitch` u16 elements per
/// row; slice `z` of a 3D block (`bd` > 1).
pub(crate) fn decode_hdr(
    block: &[u8],
    (bw, bh, bd): (usize, usize, usize),
    z: usize,
    out: &mut [u16],
    pitch: usize,
) {
    let fill = |out: &mut [u16], px: [u16; 4]| {
        for y in 0..bh {
            for x in 0..bw {
                out[y * pitch + x * 4..][..4].copy_from_slice(&px);
            }
        }
    };
    let mut tx = Texels {
        c: [[0; 4]; MAX_TEXELS],
        part: [0; MAX_TEXELS],
    };
    match decode(read_block(block), bw, bh, bd, false, &mut tx) {
        Block::Error => fill(out, ERROR_HDR),
        Block::ConstHdr(c) => fill(out, c),
        Block::ConstLdr(c) => fill(out, c.map(unorm16_to_half)),
        Block::Texels { parts } => {
            for y in 0..bh {
                for x in 0..bw {
                    let i = (z * bh + y) * bw + x;
                    let e = &parts[tx.part[i] as usize];
                    let c = tx.c[i];
                    let rgb = |v: u16| {
                        if e.hdr_rgb {
                            lns_to_half(v)
                        } else {
                            unorm16_to_half(v)
                        }
                    };
                    let a = if e.hdr_a {
                        lns_to_half(c[3])
                    } else {
                        unorm16_to_half(c[3])
                    };
                    out[y * pitch + x * 4..][..4].copy_from_slice(&[
                        rgb(c[0]),
                        rgb(c[1]),
                        rgb(c[2]),
                        a,
                    ]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ldr(block: u128, bw: usize, bh: usize, srgb: bool) -> Vec<u8> {
        let mut out = vec![0u8; bw * bh * 4];
        decode_ldr(&block.to_le_bytes(), bw, bh, 1, 0, srgb, &mut out, bw * 4);
        out
    }

    fn hdr(block: u128, bw: usize, bh: usize) -> Vec<u16> {
        let mut out = vec![0u16; bw * bh * 4];
        decode_hdr(&block.to_le_bytes(), (bw, bh, 1), 0, &mut out, bw * 4);
        out
    }

    /// Void-extent block with no extent and the given 16-bit colour.
    fn void(hdr: bool, c: [u16; 4]) -> u128 {
        let mut b: u128 = 0x1fc | (3 << 10) | (((1u128 << 52) - 1) << 12);
        if hdr {
            b |= 1 << 9;
        }
        for (k, v) in c.iter().enumerate() {
            b |= u128::from(*v) << (64 + 16 * k);
        }
        b
    }

    #[test]
    fn footprints() {
        assert!(valid_footprint(4, 4) && valid_footprint(12, 12) && valid_footprint(10, 5));
        assert!(!valid_footprint(3, 3) && !valid_footprint(12, 8) && !valid_footprint(5, 6));
    }

    #[test]
    fn void_extent_ldr() {
        let b = void(false, [0xff00, 0x8000, 0x0000, 0xffff]);
        for px in ldr(b, 6, 5, false).chunks_exact(4) {
            assert_eq!(px, [255, 128, 0, 255]);
        }
        let h = hdr(b, 4, 4);
        assert_eq!(&h[..4], &[unorm16_to_half(0xff00), 0x3800, 0, HALF_ONE]);
    }

    #[test]
    fn void_extent_hdr() {
        let c = [0x4000, 0x3c00, 0x0000, 0x3c00];
        let b = void(true, c);
        assert_eq!(&hdr(b, 8, 8)[..4], &c);
        // HDR void extent in the LDR profile is an error.
        assert_eq!(&ldr(b, 8, 8, false)[..4], &ERROR_LDR);
    }

    #[test]
    fn reserved_block_mode_is_error() {
        // Block mode 0: bits [1:0] and [3:2] zero is reserved.
        assert_eq!(&ldr(0, 4, 4, false)[..4], &ERROR_LDR);
        assert_eq!(&hdr(0, 4, 4)[..4], &ERROR_HDR);
        assert_eq!(block_mode(0b001_0001), Some((4, 2, 1, false)));
    }

    /// Arbitrary bits never panic (run in debug for overflow checks) in any footprint.
    #[test]
    fn random_blocks_never_panic() {
        let mut s = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        for (bw, bh) in FOOTPRINTS {
            for _ in 0..4000 {
                let b = u128::from(next()) | u128::from(next()) << 64;
                // Bias half the samples towards void extents.
                let b = if b & 1 == 0 { (b & !0x1ff) | 0x1fc } else { b };
                ldr(b, bw, bh, b & 2 != 0);
                hdr(b, bw, bh);
            }
        }
    }

    #[test]
    fn ise_tables() {
        // Every trit / quint combination appears among the encodings.
        let trits: std::collections::HashSet<_> = TRITS.iter().collect();
        assert_eq!(trits.len(), 243);
        let quints: std::collections::HashSet<_> = QUINTS.iter().collect();
        assert_eq!(quints.len(), 125);
        assert_eq!(ise_bits(5, 1), 8);
        assert_eq!(ise_bits(3, 3), 7);
        assert_eq!(ise_bits(16, 2), 32);
        assert_eq!(WEIGHT_UNQUANT[1][..3], [0, 32, 64]);
        assert_eq!(WEIGHT_UNQUANT[4][..6], [0, 64, 12, 52, 25, 39]);
        assert_eq!(colour_unquant()[4][..6], [0, 255, 51, 204, 102, 153]);
    }

    #[test]
    fn lns() {
        assert_eq!(lns_to_half(0x7800), HALF_ONE);
        assert_eq!(lns_to_half(0), 0);
        assert_eq!(lns_to_half(0xffff), 0x7bff);
        assert_eq!(unorm16_to_half(0xffff), HALF_ONE);
        assert_eq!(unorm16_to_half(0x8000), 0x3800);
        assert_eq!(unorm16_to_half(0xfffe), 0x3bff);
        assert_eq!(unorm16_to_half(4), 0x0400);
        assert_eq!(unorm16_to_half(3), 0x0300);
    }

    // ---- corpus: astcenc references (tools/corpus/texture/astc.sh) ----

    fn corpus() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("SPECULAR_LARGE_CORPUS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/large")
            });
        let d = base.join("texture/astc");
        d.is_dir().then_some(d)
    }

    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "astc") {
                out.push(p);
            }
        }
    }

    /// Uncompressed KTX1 → (width, height, channels, bytes per channel, data).
    fn ktx1(d: &[u8]) -> (usize, usize, usize, usize, Vec<u8>) {
        let u = |p: usize| u32::from_le_bytes(d[p..p + 4].try_into().unwrap()) as usize;
        let (gl_type, gl_format) = (u(16), u(24));
        let (w, h) = (u(36), u(40).max(1));
        let kvd = u(60);
        let size = u(64 + kvd);
        let data = d[68 + kvd..68 + kvd + size].to_vec();
        let ch = match gl_format {
            0x1908 => 4,
            0x1907 => 3,
            0x8227 => 2,
            0x1903 => 1,
            f => panic!("glFormat {f:#x}"),
        };
        let bpc = if gl_type == 0x140b { 2 } else { 1 };
        (w, h, ch, bpc, data)
    }

    /// Decode a .astc file with the LDR ('l', 's') or HDR ('h') profile.
    fn decode_file(a: &[u8], mode: char) -> (usize, usize, Vec<u16>) {
        let (bw, bh) = (a[4] as usize, a[5] as usize);
        let u24 = |p: usize| a[p] as usize | (a[p + 1] as usize) << 8 | (a[p + 2] as usize) << 16;
        let (w, h) = (u24(7), u24(10));
        let (nx, ny) = (w.div_ceil(bw), h.div_ceil(bh));
        let mut img = vec![0u16; w * h * 4];
        let mut b8 = vec![0u8; bw * bh * 4];
        let mut b16 = vec![0u16; bw * bh * 4];
        for by in 0..ny {
            for bx in 0..nx {
                let off = 16 + (by * nx + bx) * 16;
                let blk = &a[off..off + 16];
                if mode == 'h' {
                    decode_hdr(blk, (bw, bh, 1), 0, &mut b16, bw * 4);
                } else {
                    decode_ldr(blk, bw, bh, 1, 0, mode == 's', &mut b8, bw * 4);
                    for (d, s) in b16.iter_mut().zip(&b8) {
                        *d = u16::from(*s);
                    }
                }
                for y in 0..bh {
                    for x in 0..bw {
                        let (ix, iy) = (bx * bw + x, by * bh + y);
                        if ix < w && iy < h {
                            let s = (y * bw + x) * 4;
                            img[(iy * w + ix) * 4..][..4].copy_from_slice(&b16[s..s + 4]);
                        }
                    }
                }
            }
        }
        (w, h, img)
    }

    fn half_f(h: u16) -> f32 {
        let e = (h >> 10) & 0x1f;
        let m = f32::from(h & 0x3ff);
        let v = match e {
            0 => m / 16_777_216.0,
            31 => f32::INFINITY,
            _ => (1.0 + m / 1024.0) * 2f32.powi(i32::from(e) - 15),
        };
        if h & 0x8000 != 0 { -v } else { v }
    }

    /// Every .astc under the corpus against each astcenc reference (`<file>.astc.<d>.ktx`,
    /// d = l, s, h). Set ASTC_VERBOSE to list mismatching files.
    #[test]
    fn astcenc_corpus() {
        let Some(dir) = corpus() else { return };
        let mut files = Vec::new();
        walk(&dir, &mut files);
        files.sort();
        let verbose = std::env::var_os("ASTC_VERBOSE").is_some();
        let (mut nref, mut bad) = (0, Vec::new());
        let mut texels = 0u64;
        for f in &files {
            let a = std::fs::read(f).unwrap();
            for mode in ['l', 's', 'h'] {
                let r = f.with_extension(format!("astc.{mode}.ktx"));
                let Ok(rd) = std::fs::read(&r) else { continue };
                nref += 1;
                let (rw, rh, ch, bpc, data) = ktx1(&rd);
                let (w, h, img) = decode_file(&a, mode);
                assert_eq!((rw, rh), (w, h), "{}", r.display());
                let row = w * ch * bpc;
                let (mut mism, mut max_err, mut first) = (0u64, 0f32, None);
                for y in 0..h {
                    for x in 0..w {
                        for c in 0..4 {
                            let want = if c >= ch {
                                if bpc == 2 { HALF_ONE } else { 255 }
                            } else {
                                let p = y * row + (x * ch + c) * bpc;
                                if bpc == 2 {
                                    u16::from_le_bytes([data[p], data[p + 1]])
                                } else {
                                    u16::from(data[p])
                                }
                            };
                            let got = img[(y * w + x) * 4 + c];
                            if got != want {
                                mism += 1;
                                let e = if bpc == 2 {
                                    (half_f(got) - half_f(want)).abs()
                                        / half_f(want).abs().max(1e-3)
                                } else {
                                    f32::from(got.abs_diff(want))
                                };
                                max_err = max_err.max(e);
                                first.get_or_insert((x, y, c, got, want));
                            }
                        }
                    }
                }
                texels += (w * h) as u64;
                if mism > 0 {
                    if verbose {
                        eprintln!(
                            "{}: {mism} mismatches, max {max_err}, first {first:?}",
                            r.display()
                        );
                    }
                    bad.push((r.clone(), mism, max_err));
                }
            }
        }
        eprintln!(
            "astc corpus: {} files, {nref} references, {texels} texels, {} mismatching",
            files.len(),
            bad.len()
        );
        assert!(bad.is_empty(), "{bad:#?}");
    }

    /// Single-thread throughput on a 2048² image tiled from corpus blocks. Run with
    /// `cargo test --release -- --ignored --nocapture throughput`.
    #[test]
    #[ignore]
    fn throughput() {
        let Some(dir) = corpus() else { return };
        const N: usize = 2048;
        for (bw, bh) in [(4, 4), (6, 6), (8, 8), (12, 12)] {
            for (src, hdr_mode) in [("noise", false), ("gradient", false), ("hdr", true)] {
                let prof = if hdr_mode { "cH" } else { "cl" };
                let p = dir.join(format!("gen/{src}_{bw}x{bh}_{prof}_thorough.astc"));
                let Ok(a) = std::fs::read(&p) else { continue };
                let src_blocks: Vec<&[u8]> = a[16..].chunks_exact(16).collect();
                let (nx, ny) = (N.div_ceil(bw), N.div_ceil(bh));
                let blocks: Vec<u8> = (0..nx * ny)
                    .flat_map(|i| src_blocks[i % src_blocks.len()].to_vec())
                    .collect();
                let (w, h) = (nx * bw, ny * bh);
                let mut img8 = vec![0u8; w * h * 4];
                let mut img16 = vec![0u16; w * h * 4];
                let mut best = f64::MAX;
                for _ in 0..5 {
                    let t = std::time::Instant::now();
                    for by in 0..ny {
                        for bx in 0..nx {
                            let blk = &blocks[(by * nx + bx) * 16..][..16];
                            let o = by * bh * w * 4 + bx * bw * 4;
                            if hdr_mode {
                                decode_hdr(blk, (bw, bh, 1), 0, &mut img16[o..], w * 4);
                            } else {
                                decode_ldr(blk, bw, bh, 1, 0, false, &mut img8[o..], w * 4);
                            }
                        }
                    }
                    best = best.min(t.elapsed().as_secs_f64());
                }
                std::hint::black_box((&img8, &img16));
                eprintln!("{bw}x{bh} {src}: {:.1} Mpix/s", (N * N) as f64 / best / 1e6);
            }
        }
    }
}
