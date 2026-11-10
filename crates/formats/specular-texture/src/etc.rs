//! ETC1, ETC2 (RGB, RGB with punch-through alpha, RGBA with an EAC alpha block) and EAC
//! R11 / RG11 (unsigned and signed) blocks, written from the Khronos Data Format
//! specification. Blocks are 64-bit big-endian words; texel `i = x·4 + y` (column-major) takes
//! its index bits from bit `i` (and bit `16 + i`) of the low word, or a 3-bit EAC index from
//! bits `45 − 3i …` of the whole word.

/// ETC1 intensity modifiers by table, indexed by the texel's (msb, lsb) bits:
/// 00 → +a, 01 → +b, 10 → −a, 11 → −b.
const MODIFIERS: [[i32; 4]; 8] = [
    [2, 8, -2, -8],
    [5, 17, -5, -17],
    [9, 29, -9, -29],
    [13, 42, -13, -42],
    [18, 60, -18, -60],
    [24, 80, -24, -80],
    [33, 106, -33, -106],
    [47, 183, -47, -183],
];

/// T and H mode distances.
const DISTANCES: [i32; 8] = [3, 6, 11, 16, 23, 32, 41, 64];

/// EAC modifiers by table index.
const EAC_MODIFIERS: [[i32; 8]; 16] = [
    [-3, -6, -9, -15, 2, 5, 8, 14],
    [-3, -7, -10, -13, 2, 6, 9, 12],
    [-2, -5, -8, -13, 1, 4, 7, 12],
    [-2, -4, -6, -13, 1, 3, 5, 12],
    [-3, -6, -8, -12, 2, 5, 7, 11],
    [-3, -7, -9, -11, 2, 6, 8, 10],
    [-4, -7, -8, -11, 3, 6, 7, 10],
    [-3, -5, -8, -11, 2, 4, 7, 10],
    [-2, -6, -8, -10, 1, 5, 7, 9],
    [-2, -5, -8, -10, 1, 4, 7, 9],
    [-2, -4, -8, -10, 1, 3, 7, 9],
    [-2, -5, -7, -10, 1, 4, 6, 9],
    [-3, -4, -7, -10, 2, 3, 6, 9],
    [-1, -2, -3, -10, 0, 1, 2, 9],
    [-4, -6, -8, -9, 3, 5, 7, 8],
    [-3, -5, -7, -9, 2, 4, 6, 8],
];

#[inline]
fn clamp8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

#[inline]
fn ext4(v: u32) -> i32 {
    ((v << 4) | v) as i32
}

#[inline]
fn ext5(v: u32) -> i32 {
    ((v << 3) | (v >> 2)) as i32
}

#[inline]
fn ext6(v: u32) -> i32 {
    ((v << 2) | (v >> 4)) as i32
}

#[inline]
fn ext7(v: u32) -> i32 {
    ((v << 1) | (v >> 6)) as i32
}

/// 3-bit two's complement.
#[inline]
fn sign3(v: u32) -> i32 {
    ((v as i32) << 29) >> 29
}

#[inline]
fn words(src: &[u8]) -> (u32, u32) {
    (
        u32::from_be_bytes([src[0], src[1], src[2], src[3]]),
        u32::from_be_bytes([src[4], src[5], src[6], src[7]]),
    )
}

/// The 2-bit index of texel (x, y): (msb << 1) | lsb.
#[inline]
fn index2(lo: u32, x: usize, y: usize) -> usize {
    let i = x * 4 + y;
    ((((lo >> (16 + i)) & 1) << 1) | ((lo >> i) & 1)) as usize
}

#[inline]
fn put(out: &mut [u8], pitch: usize, x: usize, y: usize, px: [u8; 4]) {
    let o = y * pitch + x * 4;
    out[o..o + 4].copy_from_slice(&px);
}

/// Decode an ETC1 / ETC2 RGB block (8 bytes) to RGBA8 4×4 with row pitch `pitch`. With
/// `punch` (ETC2 RGB8 A1) the differential bit is the "opaque" flag: when clear, index 10b
/// of the ETC1-style and T/H modes is transparent black and the ETC1-style ±a modifiers
/// become 0; the individual mode does not exist.
pub(crate) fn etc2_rgb(src: &[u8], punch: bool, out: &mut [u8], pitch: usize) {
    let (hi, lo) = words(src);
    let diff = hi & 2 != 0;
    let opaque = !punch || diff;
    if !punch && !diff {
        // Individual: two 4-bit colours.
        let c0 = [ext4(hi >> 28), ext4((hi >> 20) & 15), ext4((hi >> 12) & 15)];
        let c1 = [
            ext4((hi >> 24) & 15),
            ext4((hi >> 16) & 15),
            ext4((hi >> 8) & 15),
        ];
        subblocks(hi, lo, c0, c1, true, out, pitch);
        return;
    }
    let (r, g, b) = ((hi >> 27) & 31, (hi >> 19) & 31, (hi >> 11) & 31);
    let (dr, dg, db) = (
        sign3((hi >> 24) & 7),
        sign3((hi >> 16) & 7),
        sign3((hi >> 8) & 7),
    );
    let (r2, g2, b2) = (r as i32 + dr, g as i32 + dg, b as i32 + db);
    if !(0..32).contains(&r2) {
        t_mode(hi, lo, opaque, out, pitch);
    } else if !(0..32).contains(&g2) {
        h_mode(hi, lo, opaque, out, pitch);
    } else if !(0..32).contains(&b2) {
        planar(hi, lo, out, pitch);
    } else {
        let c0 = [ext5(r), ext5(g), ext5(b)];
        let c1 = [ext5(r2 as u32), ext5(g2 as u32), ext5(b2 as u32)];
        subblocks(hi, lo, c0, c1, opaque, out, pitch);
    }
}

/// ETC1-style decode: two half blocks (side by side, or stacked when flipped) with a base
/// colour and an intensity table each.
fn subblocks(
    hi: u32,
    lo: u32,
    c0: [i32; 3],
    c1: [i32; 3],
    opaque: bool,
    out: &mut [u8],
    pitch: usize,
) {
    let flip = hi & 1 != 0;
    let tables = [
        MODIFIERS[((hi >> 5) & 7) as usize],
        MODIFIERS[((hi >> 2) & 7) as usize],
    ];
    for y in 0..4 {
        for x in 0..4 {
            let second = if flip { y >= 2 } else { x >= 2 };
            let (c, t) = if second {
                (c1, tables[1])
            } else {
                (c0, tables[0])
            };
            let idx = index2(lo, x, y);
            let px = if opaque {
                let m = t[idx];
                [clamp8(c[0] + m), clamp8(c[1] + m), clamp8(c[2] + m), 255]
            } else if idx == 2 {
                [0, 0, 0, 0]
            } else {
                let m = if idx == 0 { 0 } else { t[idx] };
                [clamp8(c[0] + m), clamp8(c[1] + m), clamp8(c[2] + m), 255]
            };
            put(out, pitch, x, y, px);
        }
    }
}

/// Write four paint colours by 2-bit index; without `opaque`, index 2 is transparent black.
fn paint(lo: u32, colours: [[i32; 3]; 4], opaque: bool, out: &mut [u8], pitch: usize) {
    for y in 0..4 {
        for x in 0..4 {
            let idx = index2(lo, x, y);
            let px = if !opaque && idx == 2 {
                [0, 0, 0, 0]
            } else {
                let c = colours[idx];
                [clamp8(c[0]), clamp8(c[1]), clamp8(c[2]), 255]
            };
            put(out, pitch, x, y, px);
        }
    }
}

fn t_mode(hi: u32, lo: u32, opaque: bool, out: &mut [u8], pitch: usize) {
    let r1 = (((hi >> 27) & 3) << 2) | ((hi >> 24) & 3);
    let c1 = [ext4(r1), ext4((hi >> 20) & 15), ext4((hi >> 16) & 15)];
    let c2 = [
        ext4((hi >> 12) & 15),
        ext4((hi >> 8) & 15),
        ext4((hi >> 4) & 15),
    ];
    let d = DISTANCES[((((hi >> 2) & 3) << 1) | (hi & 1)) as usize];
    let add = |c: [i32; 3], d: i32| [c[0] + d, c[1] + d, c[2] + d];
    paint(lo, [c1, add(c2, d), c2, add(c2, -d)], opaque, out, pitch);
}

fn h_mode(hi: u32, lo: u32, opaque: bool, out: &mut [u8], pitch: usize) {
    let r1 = (hi >> 27) & 15;
    let g1 = (((hi >> 24) & 7) << 1) | ((hi >> 20) & 1);
    let b1 = (((hi >> 19) & 1) << 3) | ((hi >> 15) & 7);
    let r2 = (hi >> 11) & 15;
    let g2 = (hi >> 7) & 15;
    let b2 = (hi >> 3) & 15;
    let ge = ((r1 << 8) | (g1 << 4) | b1) >= ((r2 << 8) | (g2 << 4) | b2);
    let di = (((hi >> 2) & 1) << 2) | ((hi & 1) << 1) | u32::from(ge);
    let d = DISTANCES[di as usize];
    let c1 = [ext4(r1), ext4(g1), ext4(b1)];
    let c2 = [ext4(r2), ext4(g2), ext4(b2)];
    let add = |c: [i32; 3], d: i32| [c[0] + d, c[1] + d, c[2] + d];
    paint(
        lo,
        [add(c1, d), add(c1, -d), add(c2, d), add(c2, -d)],
        opaque,
        out,
        pitch,
    );
}

fn planar(hi: u32, lo: u32, out: &mut [u8], pitch: usize) {
    let w = (u64::from(hi) << 32) | u64::from(lo);
    let bits = |pos: u32, n: u32| ((w >> pos) & ((1 << n) - 1)) as u32;
    let ro = ext6(bits(57, 6));
    let go = ext7((bits(56, 1) << 6) | bits(49, 6));
    let bo = ext6((bits(48, 1) << 5) | (bits(43, 2) << 3) | bits(39, 3));
    let rh = ext6((bits(34, 5) << 1) | bits(32, 1));
    let gh = ext7(bits(25, 7));
    let bh = ext6(bits(19, 6));
    let rv = ext6(bits(13, 6));
    let gv = ext7(bits(6, 7));
    let bv = ext6(bits(0, 6));
    for y in 0..4i32 {
        for x in 0..4i32 {
            let f = |o: i32, h: i32, v: i32| clamp8((x * (h - o) + y * (v - o) + 4 * o + 2) >> 2);
            put(
                out,
                pitch,
                x as usize,
                y as usize,
                [f(ro, rh, rv), f(go, gh, gv), f(bo, bh, bv), 255],
            );
        }
    }
}

/// Decode an EAC alpha block (8 bytes, the first half of an ETC2 RGBA8 block) into byte 3 of
/// each RGBA8 texel.
pub(crate) fn eac_alpha(src: &[u8], out: &mut [u8], pitch: usize) {
    let w = u64::from_be_bytes([
        src[0], src[1], src[2], src[3], src[4], src[5], src[6], src[7],
    ]);
    let base = (w >> 56) as i32;
    let mult = ((w >> 52) & 15) as i32;
    let table = &EAC_MODIFIERS[((w >> 48) & 15) as usize];
    for x in 0..4 {
        for y in 0..4 {
            let i = x * 4 + y;
            let idx = ((w >> (45 - 3 * i)) & 7) as usize;
            out[y * pitch + x * 4 + 3] = clamp8(base + table[idx] * mult);
        }
    }
}

/// Decode an EAC R11 block (8 bytes) to 11-bit values in raster order: 0…2047 unsigned,
/// −1023…1023 signed.
pub(crate) fn eac11(src: &[u8], signed: bool) -> [i16; 16] {
    let w = u64::from_be_bytes([
        src[0], src[1], src[2], src[3], src[4], src[5], src[6], src[7],
    ]);
    let mult = ((w >> 52) & 15) as i32;
    let table = &EAC_MODIFIERS[((w >> 48) & 15) as usize];
    let (base, lo, hi) = if signed {
        (i32::from((w >> 56) as u8 as i8).max(-127) * 8, -1023, 1023)
    } else {
        (((w >> 56) as i32) * 8 + 4, 0, 2047)
    };
    let mut v = [0i16; 16];
    for x in 0..4 {
        for y in 0..4 {
            let i = x * 4 + y;
            let m = table[((w >> (45 - 3 * i)) & 7) as usize];
            let m = if mult == 0 { m } else { m * mult * 8 };
            v[y * 4 + x] = (base + m).clamp(lo, hi) as i16;
        }
    }
    v
}

/// An 11-bit EAC value to 8 bits for display: unsigned 0…2047, signed −1023…1023 → 0…255.
#[inline]
pub(crate) fn eac_to_8(v: i16, signed: bool) -> u8 {
    let v = i32::from(v);
    if signed {
        (((v + 1023) * 255 + 1023) / 2046) as u8
    } else {
        ((v * 255 + 1023) / 2047) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(block: [u8; 8], punch: bool) -> [u8; 64] {
        let mut out = [0u8; 64];
        etc2_rgb(&block, punch, &mut out, 16);
        out
    }

    #[test]
    fn individual_and_differential() {
        // Individual, colours 0x8 / 0x4 per channel, tables 0, all indices 0 (+2).
        let o = rgb([0x84, 0x84, 0x84, 0x00, 0, 0, 0, 0], false);
        assert_eq!(&o[..4], &[0x8a, 0x8a, 0x8a, 255]);
        assert_eq!(&o[8..12], &[0x46, 0x46, 0x46, 255]);
        // Differential, base 16, delta +1, table 7, flipped, all indices 11 (−b = −183).
        let o = rgb([0x81, 0x81, 0x81, 0xe3, 0xff, 0xff, 0xff, 0xff], false);
        assert_eq!(&o[..4], &[0, 0, 0, 255]);
        // Punch-through, not opaque: index 10 is transparent.
        let o = rgb([0x80, 0x80, 0x80, 0x00, 0xff, 0xff, 0x00, 0x00], true);
        assert_eq!(&o[..4], &[0, 0, 0, 0]);
        // Index 00 is the base colour without modifier.
        let o = rgb([0x80, 0x80, 0x80, 0x00, 0, 0, 0, 0], true);
        assert_eq!(&o[..4], &[0x84, 0x84, 0x84, 255]);
    }

    #[test]
    fn eac_values() {
        // Base 128, multiplier 1, table 13 (−1 −2 −3 −10 0 1 2 9), index 7 everywhere (+9).
        let b = [128, 0x1d, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        let mut out = [0u8; 64];
        eac_alpha(&b, &mut out, 16);
        assert_eq!(out[3], 137);
        assert_eq!(eac11(&b, false)[0], 128 * 8 + 4 + 72);
        // Signed: base −128 is read as −127; multiplier 0 adds the bare modifier.
        let b = [0x80, 0x0d, 0, 0, 0, 0, 0, 0];
        assert_eq!(eac11(&b, true)[5], -127 * 8 - 1);
        assert_eq!(eac_to_8(-1023, true), 0);
        assert_eq!(eac_to_8(1023, true), 255);
        assert_eq!(eac_to_8(0, true), 128);
        assert_eq!(eac_to_8(2047, false), 255);
    }

    /// Decode a raster of blocks in `mode` (an etc_ref mode): RGBA8, or 11-bit values.
    fn decode_image(mode: &str, w: usize, h: usize, input: &[u8]) -> (Vec<u8>, Vec<i16>) {
        let (bx, by) = (w.div_ceil(4), h.div_ceil(4));
        let bytes = if matches!(mode, "rgba8" | "rg11" | "rg11s") {
            16
        } else {
            8
        };
        let ch = if bytes == 16 { 2 } else { 1 };
        let (mut rgba, mut eleven) = (vec![0u8; w * h * 4], vec![0i16; w * h * ch]);
        for y in 0..by {
            for x in 0..bx {
                let b = &input[(y * bx + x) * bytes..][..bytes];
                let mut px = [0u8; 64];
                match mode {
                    "rgb" => etc2_rgb(b, false, &mut px, 16),
                    "rgba1" => etc2_rgb(b, true, &mut px, 16),
                    "rgba8" => {
                        etc2_rgb(&b[8..], false, &mut px, 16);
                        eac_alpha(&b[..8], &mut px, 16);
                    }
                    _ => {
                        for c in 0..ch {
                            let v = eac11(&b[8 * c..8 * c + 8], mode.ends_with('s'));
                            for (i, &v) in v.iter().enumerate() {
                                let (xx, yy) = (x * 4 + i % 4, y * 4 + i / 4);
                                if xx < w && yy < h {
                                    eleven[(yy * w + xx) * ch + c] = v;
                                }
                            }
                        }
                    }
                }
                for i in 0..16 {
                    let (xx, yy) = (x * 4 + i % 4, y * 4 + i / 4);
                    if xx < w && yy < h {
                        rgba[(yy * w + xx) * 4..][..4].copy_from_slice(&px[i * 4..i * 4 + 4]);
                    }
                }
            }
        }
        (rgba, eleven)
    }

    /// Every ETC / EAC reference of `tools/corpus/texture/blocks.sh` (ETCPACK decodes of
    /// random blocks and of the KTX-Software test files), bit-exact.
    #[test]
    fn matches_etcpack() {
        let mut n = 0;
        for dir in ["random", "files"] {
            for (mode, w, h, input, reference) in crate::block_refs(dir) {
                if mode.starts_with("pvrtc") {
                    continue;
                }
                let (rgba, eleven) = decode_image(&mode, w as usize, h as usize, &input);
                let bad = if matches!(mode.as_str(), "rgb" | "rgba1" | "rgba8") {
                    rgba.iter().zip(&reference).filter(|(a, b)| a != b).count()
                } else {
                    let want = reference
                        .chunks_exact(2)
                        .map(|c| i16::from_le_bytes([c[0], c[1]]));
                    eleven.iter().zip(want).filter(|(a, b)| **a != *b).count()
                };
                assert_eq!(bad, 0, "{dir} {mode} {w}x{h}: {bad} values differ");
                n += 1;
            }
        }
        eprintln!("{n} ETC / EAC references compared");
    }
}
