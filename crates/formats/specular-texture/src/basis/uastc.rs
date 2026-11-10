//! UASTC LDR 4x4 block decoding to RGBA8.
//!
//! Written from the UASTC texture specification (Binomial, public domain / CC0) and the ASTC
//! chapter of the Khronos Data Format specification. UASTC is a 19-mode subset of 4x4 LDR ASTC
//! with a simpler 128-bit layout: a Huffman-coded mode in the low bits, transcoder hints, then
//! (per mode) partition pattern, component selector, endpoint trits/quints, endpoint bits and
//! plain weight indices, all LSB-first. Endpoints are unquantised and interpolated exactly like
//! ASTC (16-bit expansion, 6-bit weights, `srgb = false`).

/// Mode index for the low 7 bits of a block (normative UASTC table; 19 is reserved).
const HUFF_MODES: [u8; 128] = [
    11, 0, 10, 3, 11, 15, 12, 7, 11, 18, 10, 5, 11, 14, 12, 9, 11, 0, 10, 4, 11, 16, 12, 8, 11, 18,
    10, 6, 11, 2, 12, 13, 11, 0, 10, 3, 11, 17, 12, 7, 11, 18, 10, 5, 11, 14, 12, 9, 11, 0, 10, 4,
    11, 1, 12, 8, 11, 18, 10, 6, 11, 2, 12, 13, 11, 0, 10, 3, 11, 19, 12, 7, 11, 18, 10, 5, 11, 14,
    12, 9, 11, 0, 10, 4, 11, 16, 12, 8, 11, 18, 10, 6, 11, 2, 12, 13, 11, 0, 10, 3, 11, 17, 12, 7,
    11, 18, 10, 5, 11, 14, 12, 9, 11, 0, 10, 4, 11, 1, 12, 8, 11, 18, 10, 6, 11, 2, 12, 13,
];

const VOID_EXTENT: usize = 8;

/// Per-mode layout.
#[derive(Clone, Copy)]
struct Mode {
    /// First bit of the endpoint data (trit/quint groups, then the per-value bits).
    endpoints: u8,
    /// ASTC endpoint quantisation range index.
    range: u8,
    /// Endpoint components: 2 (LA, CEM 4), 3 (RGB, CEM 8) or 4 (RGBA, CEM 12).
    comps: u8,
    subsets: u8,
    dual_plane: bool,
    weight_bits: u8,
    /// Partition pattern index field (bit, width); width 0 for one subset.
    pattern: (u8, u8),
    /// Component selector field bit (2 bits), 0 when absent.
    ccs: u8,
}

#[allow(clippy::too_many_arguments)] // table constructor
const fn mode(
    endpoints: u8,
    range: u8,
    comps: u8,
    subsets: u8,
    dual_plane: bool,
    weight_bits: u8,
    pattern: (u8, u8),
    ccs: u8,
) -> Mode {
    Mode {
        endpoints,
        range,
        comps,
        subsets,
        dual_plane,
        weight_bits,
        pattern,
        ccs,
    }
}

/// Modes 0..=18 (bit positions from the UASTC mode layouts; mode 8 is void-extent).
const MODES: [Mode; 19] = [
    mode(19, 19, 3, 1, false, 4, (0, 0), 0),
    mode(21, 20, 3, 1, false, 2, (0, 0), 0),
    mode(25, 8, 3, 2, false, 3, (20, 5), 0),
    mode(24, 7, 3, 3, false, 2, (20, 4), 0),
    mode(25, 12, 3, 2, false, 2, (20, 5), 0),
    mode(20, 20, 3, 1, false, 3, (0, 0), 0),
    mode(22, 18, 3, 1, true, 2, (0, 0), 20),
    mode(25, 12, 3, 2, false, 2, (20, 5), 0),
    mode(0, 0, 4, 0, false, 0, (0, 0), 0),
    mode(33, 8, 4, 2, false, 2, (28, 5), 0),
    mode(20, 13, 4, 1, false, 4, (0, 0), 0),
    mode(21, 13, 4, 1, true, 2, (0, 0), 19),
    mode(20, 19, 4, 1, false, 3, (0, 0), 0),
    mode(30, 20, 4, 1, true, 1, (0, 0), 28),
    mode(28, 20, 4, 1, false, 2, (0, 0), 0),
    mode(30, 20, 2, 1, false, 4, (0, 0), 0),
    mode(34, 20, 2, 2, false, 2, (29, 5), 0),
    // Dual-plane LA: the component selector is always 3 (alpha).
    mode(29, 20, 2, 1, true, 2, (0, 0), 0),
    mode(19, 11, 3, 1, false, 5, (0, 0), 0),
];

/// ASTC integer sequence ranges: (bits, trits, quints) per range index.
const RANGES: [(u32, u32, u32); 21] = [
    (1, 0, 0),
    (0, 1, 0),
    (2, 0, 0),
    (0, 0, 1),
    (1, 1, 0),
    (3, 0, 0),
    (1, 0, 1),
    (2, 1, 0),
    (4, 0, 0),
    (2, 0, 1),
    (3, 1, 0),
    (5, 0, 0),
    (3, 0, 1),
    (4, 1, 0),
    (6, 0, 0),
    (4, 0, 1),
    (5, 1, 0),
    (7, 0, 0),
    (5, 0, 1),
    (6, 1, 0),
    (8, 0, 0),
];

/// ASTC endpoint unquantisation of `(trit or quint) << bits | bits` to 0..=255.
const fn unquantise(range: usize, v: u32) -> u8 {
    let (n, trits, quints) = RANGES[range];
    if trits == 0 && quints == 0 {
        // Bit replication.
        let mut out = 0u32;
        let mut have = 0;
        while have < 8 {
            out = out << n | v;
            have += n;
        }
        return (out >> (have - 8)) as u8;
    }
    if n == 0 {
        return 0; // ranges 1 and 3 (unused by UASTC)
    }
    let d = v >> n;
    let m = v & ((1 << n) - 1);
    let a = if m & 1 != 0 { 0x1ff } else { 0 };
    let hi = m >> 1;
    let (b, c) = if trits != 0 {
        match n {
            1 => (0, 204),
            2 => (hi * 0b1_0001_0110, 93),
            3 => (hi << 7 | hi << 2 | hi, 44),
            4 => (hi << 6 | hi, 22),
            5 => (hi << 5 | hi >> 2, 11),
            _ => (hi << 4 | hi >> 4, 5),
        }
    } else {
        match n {
            1 => (0, 113),
            2 => (hi * 0b1_0000_1100, 54),
            3 => (hi << 7 | hi << 1 | hi >> 1, 26),
            4 => (hi << 6 | hi >> 1, 13),
            _ => (hi << 5 | hi >> 3, 6),
        }
    };
    let t = (d * c + b) ^ a;
    ((a & 0x80) | t >> 2) as u8
}

const fn unquantise_table() -> [[u8; 256]; 21] {
    let mut t = [[0u8; 256]; 21];
    let mut r = 0;
    while r < 21 {
        let mut v = 0;
        while v < 256 {
            t[r][v] = unquantise(r, v as u32);
            v += 1;
        }
        r += 1;
    }
    t
}

static UNQUANT: [[u8; 256]; 21] = unquantise_table();

/// 6-bit weights (0..=64) by weight index, for 1..=5 bit indices (ASTC weight unquantisation).
const WEIGHTS: [&[u8]; 6] = [
    &[0],
    &[0, 64],
    &[0, 21, 43, 64],
    &[0, 9, 18, 27, 37, 46, 55, 64],
    &[0, 4, 8, 12, 17, 21, 25, 29, 35, 39, 43, 47, 52, 56, 60, 64],
    &[
        0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 34, 36, 38, 40, 42, 44, 46, 48,
        50, 52, 54, 56, 58, 60, 62, 64,
    ],
];

/// Subset per texel for the 2-subset modes 2, 4, 9 and 16 (normative UASTC table).
const PATTERNS2: [[u8; 16]; 30] = [
    [0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
    [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1],
    [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
    [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 0],
    [0, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1],
    [1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 0, 0],
    [1, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
    [1, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0],
    [0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 1, 1],
    [1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0],
    [0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0],
    [1, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 0, 1, 1],
    [0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0],
    [1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1],
    [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0],
    [1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
    [1, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0],
];

const PATTERNS2_ANCHORS: [[u8; 2]; 30] = [
    [0, 2],
    [0, 3],
    [1, 0],
    [0, 3],
    [7, 0],
    [0, 2],
    [3, 0],
    [7, 0],
    [0, 11],
    [2, 0],
    [0, 7],
    [11, 0],
    [3, 0],
    [8, 0],
    [0, 4],
    [12, 0],
    [1, 0],
    [8, 0],
    [0, 1],
    [0, 2],
    [0, 4],
    [8, 0],
    [1, 0],
    [0, 2],
    [4, 0],
    [0, 1],
    [4, 0],
    [1, 0],
    [4, 0],
    [1, 0],
];

/// Subset per texel for the 3-subset mode 3.
const PATTERNS3: [[u8; 16]; 11] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 1, 1, 2, 2],
    [1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 2, 2, 2, 2],
    [1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 2],
    [1, 1, 1, 1, 2, 2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 2, 0, 1, 1, 2, 0, 1, 1, 2, 0, 1, 1, 2, 0],
    [0, 1, 1, 2, 0, 1, 1, 2, 0, 1, 1, 2, 0, 1, 1, 2],
    [0, 2, 1, 1, 0, 2, 1, 1, 0, 2, 1, 1, 0, 2, 1, 1],
    [2, 0, 0, 0, 2, 0, 0, 0, 2, 1, 1, 1, 2, 1, 1, 1],
    [2, 0, 1, 2, 2, 0, 1, 2, 2, 0, 1, 2, 2, 0, 1, 2],
    [1, 1, 1, 1, 0, 0, 0, 0, 2, 2, 2, 2, 1, 1, 1, 1],
    [0, 0, 2, 2, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 2, 2],
];

const PATTERNS3_ANCHORS: [[u8; 3]; 11] = [
    [0, 8, 10],
    [8, 0, 12],
    [4, 0, 12],
    [8, 0, 4],
    [3, 0, 2],
    [0, 1, 3],
    [0, 2, 1],
    [1, 9, 0],
    [1, 2, 0],
    [4, 0, 8],
    [0, 6, 2],
];

/// Subset per texel for mode 7 (2-subset ASTC patterns that map to 3-subset BC7 ones).
const PATTERNS_BC7_3: [[u8; 16]; 19] = [
    [0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0],
    [1, 1, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1],
    [0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0],
    [0, 0, 0, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
    [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0],
    [0, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 0],
    [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
    [0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 0],
    [1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 0],
    [0, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 0],
    [1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
];

const PATTERNS_BC7_3_ANCHORS: [[u8; 2]; 19] = [
    [0, 4],
    [0, 2],
    [2, 0],
    [0, 7],
    [8, 0],
    [0, 1],
    [0, 3],
    [0, 1],
    [2, 0],
    [0, 1],
    [0, 8],
    [2, 0],
    [0, 1],
    [0, 7],
    [12, 0],
    [2, 0],
    [9, 0],
    [0, 2],
    [4, 0],
];

const ONE_SUBSET: [u8; 16] = [0; 16];

/// Output for blocks with a reserved mode or partition pattern index.
const INVALID: [u8; 4] = [255, 0, 255, 255];

#[inline(always)]
fn field(v: u128, pos: u32, n: u32) -> u32 {
    (v.checked_shr(pos).unwrap_or(0) as u32) & ((1u32 << n) - 1)
}

/// Decodes one 16-byte UASTC LDR 4x4 block to 4x4 RGBA8 at `out` with row `pitch`. Blocks with
/// a reserved mode or pattern decode to magenta.
pub(crate) fn decode_block(block: &[u8], out: &mut [u8], pitch: usize) {
    let mut px = [0u8; 64];
    if !unpack(block, &mut px) {
        for p in px.chunks_exact_mut(4) {
            p.copy_from_slice(&INVALID);
        }
    }
    for y in 0..4 {
        out[y * pitch..y * pitch + 16].copy_from_slice(&px[y * 16..y * 16 + 16]);
    }
}

fn unpack(block: &[u8], px: &mut [u8; 64]) -> bool {
    let v = u128::from_le_bytes(block[..16].try_into().unwrap());
    let mode = usize::from(HUFF_MODES[(v & 127) as usize]);
    if mode == VOID_EXTENT {
        let c = [
            field(v, 5, 8),
            field(v, 13, 8),
            field(v, 21, 8),
            field(v, 29, 8),
        ]
        .map(|c| c as u8);
        for p in px.chunks_exact_mut(4) {
            p.copy_from_slice(&c);
        }
        return true;
    }
    let Some(m) = MODES.get(mode) else {
        return false;
    };

    let (part, anchors): (&[u8; 16], u32) = if m.subsets > 1 {
        let p = field(v, u32::from(m.pattern.0), u32::from(m.pattern.1)) as usize;
        let mask = |a: &[u8]| a.iter().fold(0u32, |m, &t| m | 1 << t);
        if m.subsets == 3 {
            match PATTERNS3.get(p) {
                Some(t) => (t, mask(&PATTERNS3_ANCHORS[p])),
                None => return false,
            }
        } else if mode == 7 {
            match PATTERNS_BC7_3.get(p) {
                Some(t) => (t, mask(&PATTERNS_BC7_3_ANCHORS[p])),
                None => return false,
            }
        } else {
            match PATTERNS2.get(p) {
                Some(t) => (t, mask(&PATTERNS2_ANCHORS[p])),
                None => return false,
            }
        }
    } else {
        (&ONE_SUBSET, 1)
    };
    // Channel interpolated with the second plane's weights (4: none).
    let ccs = match (m.dual_plane, m.ccs) {
        (false, _) => 4,
        (true, 0) => 3,
        (true, bit) => field(v, u32::from(bit), 2) as usize,
    };

    // Endpoint values: trit/quint groups first, then each value's low bits.
    let range = usize::from(m.range);
    let (bits, trits, quints) = RANGES[range];
    let count = usize::from(m.comps) * 2 * usize::from(m.subsets);
    let mut vals = [0u32; 18];
    let mut pos = u32::from(m.endpoints);
    if trits != 0 || quints != 0 {
        let (group, base, widths): (usize, u32, &[u32]) = if trits != 0 {
            (5, 3, &[0, 2, 4, 5, 7, 8])
        } else {
            (3, 5, &[0, 3, 5, 7])
        };
        for chunk in vals[..count].chunks_mut(group) {
            let n = widths[chunk.len()];
            let mut g = field(v, pos, n);
            pos += n;
            for d in chunk {
                *d = g % base;
                g /= base;
            }
        }
    }
    let table = &UNQUANT[range];
    for d in &mut vals[..count] {
        let low = field(v, pos, bits);
        pos += bits;
        *d = u32::from(table[(*d << bits | low) as usize & 255]);
    }

    // Endpoint colours per subset.
    let mut e0 = [[0u32; 4]; 3];
    let mut e1 = [[0u32; 4]; 3];
    let per = usize::from(m.comps) * 2;
    for s in 0..usize::from(m.subsets) {
        let e = &vals[s * per..s * per + per];
        (e0[s], e1[s]) = match m.comps {
            2 => ([e[0], e[0], e[0], e[2]], [e[1], e[1], e[1], e[3]]),
            3 => ([e[0], e[2], e[4], 255], [e[1], e[3], e[5], 255]),
            _ => ([e[0], e[2], e[4], e[6]], [e[1], e[3], e[5], e[7]]),
        };
        for c in 0..4 {
            e0[s][c] *= 257;
            e1[s][c] *= 257;
        }
    }

    // Weights in raster order (plane 0, plane 1 per texel); anchors store one bit less.
    let wbits = u32::from(m.weight_bits);
    let wtab = WEIGHTS[m.weight_bits as usize];
    let planes = if m.dual_plane { 2 } else { 1 };
    for i in 0..16 {
        let n = wbits - (anchors >> i & 1);
        let mut w = [0u32; 2];
        for wp in w.iter_mut().take(planes) {
            *wp = u32::from(wtab[field(v, pos, n) as usize]);
            pos += n;
        }
        let s = usize::from(part[i]);
        for c in 0..4 {
            let w = if c == ccs { w[1] } else { w[0] };
            let k = (e0[s][c] * (64 - w) + e1[s][c] * w + 32) >> 6;
            px[i * 4 + c] = (k >> 8) as u8;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (BISE integer, unquantised value) pairs from the UASTC specification's endpoint encoding
    /// tables.
    #[test]
    fn unquantisation_matches_spec_tables() {
        let r7 = [
            (0, 0),
            (4, 23),
            (8, 46),
            (2, 69),
            (6, 92),
            (10, 116),
            (11, 139),
            (7, 163),
            (3, 186),
            (9, 209),
            (5, 232),
            (1, 255),
        ];
        for (x, y) in r7 {
            assert_eq!(UNQUANT[7][x], y, "range 7, {x}");
        }
        let r4 = [(0, 0), (2, 51), (4, 102), (5, 153), (3, 204), (1, 255)];
        for (x, y) in r4 {
            assert_eq!(UNQUANT[4][x], y, "range 4, {x}");
        }
        let r6 = [
            (0, 0),
            (2, 28),
            (4, 56),
            (6, 84),
            (8, 113),
            (9, 142),
            (7, 171),
            (5, 199),
            (3, 227),
            (1, 255),
        ];
        for (x, y) in r6 {
            assert_eq!(UNQUANT[6][x], y, "range 6, {x}");
        }
        let r9 = [
            (0, 0),
            (4, 13),
            (8, 27),
            (12, 40),
            (16, 54),
            (2, 67),
            (19, 134),
            (17, 201),
            (1, 255),
        ];
        for (x, y) in r9 {
            assert_eq!(UNQUANT[9][x], y, "range 9, {x}");
        }
        let r10 = [
            (0, 0),
            (8, 11),
            (16, 22),
            (2, 33),
            (23, 134),
            (15, 145),
            (9, 244),
            (1, 255),
        ];
        for (x, y) in r10 {
            assert_eq!(UNQUANT[10][x], y, "range 10, {x}");
        }
        let r12 = [
            (0, 0),
            (8, 6),
            (16, 13),
            (24, 19),
            (32, 26),
            (2, 32),
            (39, 132),
            (31, 138),
            (7, 158),
        ];
        for (x, y) in r12 {
            assert_eq!(UNQUANT[12][x], y, "range 12, {x}");
        }
        let r13 = [
            (0, 0),
            (16, 5),
            (32, 11),
            (2, 16),
            (18, 21),
            (34, 27),
            (46, 124),
            (47, 131),
        ];
        for (x, y) in r13 {
            assert_eq!(UNQUANT[13][x], y, "range 13, {x}");
        }
        let r15 = [
            (0, 0),
            (16, 3),
            (32, 6),
            (48, 9),
            (64, 13),
            (2, 16),
            (70, 61),
            (10, 80),
        ];
        for (x, y) in r15 {
            assert_eq!(UNQUANT[15][x], y, "range 15, {x}");
        }
        let r16 = [
            (0, 0),
            (32, 2),
            (64, 5),
            (2, 8),
            (34, 10),
            (66, 13),
            (10, 40),
            (42, 43),
            (16, 64),
            (48, 67),
        ];
        for (x, y) in r16 {
            assert_eq!(UNQUANT[16][x], y, "range 16, {x}");
        }
        let r18 = [
            (0, 0),
            (32, 1),
            (64, 3),
            (96, 4),
            (128, 6),
            (2, 8),
            (104, 36),
            (136, 38),
        ];
        for (x, y) in r18 {
            assert_eq!(UNQUANT[18][x], y, "range 18, {x}");
        }
        let r19 = [
            (0, 0),
            (64, 1),
            (128, 2),
            (2, 4),
            (66, 5),
            (130, 6),
            (142, 30),
            (16, 32),
            (1, 255),
        ];
        for (x, y) in r19 {
            assert_eq!(UNQUANT[19][x], y, "range 19, {x}");
        }
        // Binary ranges replicate bits.
        assert_eq!(
            (UNQUANT[8][0xa], UNQUANT[11][0x11], UNQUANT[20][0x5c]),
            (0xaa, 0x8c, 0x5c)
        );
    }

    #[test]
    fn weight_tables_are_astc_unquantisation() {
        for bits in 1..=5u32 {
            for (v, &w) in WEIGHTS[bits as usize].iter().enumerate() {
                let v = v as u32;
                // Replicate to 6 bits, then map 33..=63 up by one.
                let mut r = 0u32;
                let mut have = 0;
                while have < 6 {
                    r = r << bits | v;
                    have += bits;
                }
                let mut r = r >> (have - 6);
                if r > 32 {
                    r += 1;
                }
                assert_eq!(u32::from(w), r, "{bits} bits, {v}");
            }
        }
    }

    #[test]
    fn mode_codes() {
        // Huffman codes from the specification: (code, length) per mode.
        let codes = [
            (0x1, 4),
            (0x35, 6),
            (0x1D, 5),
            (0x3, 5),
            (0x13, 5),
            (0xB, 5),
            (0x1B, 5),
            (0x7, 5),
            (0x17, 5),
            (0xF, 5),
            (0x2, 3),
            (0x0, 2),
            (0x6, 3),
            (0x1F, 5),
            (0xD, 5),
            (0x5, 7),
            (0x15, 6),
            (0x25, 6),
            (0x9, 4),
            (0x45, 7),
        ];
        for (b, &m) in HUFF_MODES.iter().enumerate() {
            let (code, len) = codes[usize::from(m)];
            assert_eq!(b & ((1 << len) - 1), code, "byte {b:#x} -> mode {m}");
        }
    }

    fn block(fields: &[(u32, u32, u32)]) -> [u8; 16] {
        let mut v = 0u128;
        for &(pos, n, val) in fields {
            assert!(val < 1 << n);
            v |= u128::from(val) << pos;
        }
        v.to_le_bytes()
    }

    #[test]
    fn void_extent_and_invalid() {
        // Mode 8 code 0x17 (5 bits), RGBA at bits 5, 13, 21, 29.
        let b = block(&[
            (0, 5, 0x17),
            (5, 8, 10),
            (13, 8, 20),
            (21, 8, 30),
            (29, 8, 40),
        ]);
        let mut out = [0u8; 64];
        decode_block(&b, &mut out, 16);
        assert!(out.chunks_exact(4).all(|p| p == [10, 20, 30, 40]));
        // Reserved mode 19 (code 0x45, 7 bits).
        decode_block(&block(&[(0, 7, 0x45)]), &mut out, 16);
        assert!(out.chunks_exact(4).all(|p| p == INVALID));
        // Mode 2 with pattern index 31 (only 30 patterns).
        decode_block(&block(&[(0, 5, 0x1D), (20, 5, 31)]), &mut out, 16);
        assert!(out.chunks_exact(4).all(|p| p == INVALID));
    }

    #[test]
    fn mode1_gradient() {
        // Mode 1: RGB, 8-bit endpoints at 21, 2-bit weights at 69 (texel 0: 1 bit).
        let mut f = vec![
            (0, 6, 0x35),
            (21, 8, 0),
            (29, 8, 255),
            (37, 8, 10),
            (45, 8, 10),
            (53, 8, 255),
            (61, 8, 0),
        ];
        let mut pos = 69;
        for i in 0..16u32 {
            let n = if i == 0 { 1 } else { 2 };
            f.push((pos, n, (i % 4) & ((1 << n) - 1)));
            pos += n;
        }
        let mut out = [0u8; 64];
        decode_block(&block(&f), &mut out, 16);
        let interp =
            |l: u32, h: u32, w: u32| (((l * 257 * (64 - w) + h * 257 * w + 32) >> 6) >> 8) as u8;
        for i in 0..16 {
            let w = [0, 21, 43, 64][i % 4];
            let w = if i == 0 { 0 } else { w };
            assert_eq!(
                out[i * 4..i * 4 + 4],
                [interp(0, 255, w), 10, interp(255, 0, w), 255],
                "texel {i}"
            );
        }
    }
}
