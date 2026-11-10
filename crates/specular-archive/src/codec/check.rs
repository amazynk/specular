//! Integrity checks the codecs verify: CRC-64/XZ and SHA-256 (xz), XXH64 (Zstandard) and the
//! big-endian CRC-32 of bzip2. CRC-32 (the zlib one) comes from `crc32fast`.
//!
//! Written from the published definitions (ECMA-182, FIPS 180-4, the xxHash specification).

/// Tables for slice-by-8 CRC-64 (ECMA-182 polynomial, reflected, as xz uses it).
const CRC64_TABLES: [[u64; 256]; 8] = {
    const POLY: u64 = 0xC96C_5795_D787_0F42;
    let mut t = [[0u64; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u64;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { (c >> 1) ^ POLY } else { c >> 1 };
            k += 1;
        }
        t[0][i] = c;
        i += 1;
    }
    let mut j = 1;
    while j < 8 {
        let mut i = 0;
        while i < 256 {
            let prev = t[j - 1][i];
            t[j][i] = (prev >> 8) ^ t[0][(prev & 0xFF) as usize];
            i += 1;
        }
        j += 1;
    }
    t
};

/// A running CRC-64/XZ.
#[derive(Clone, Copy)]
pub(crate) struct Crc64(u64);

impl Crc64 {
    pub(crate) fn new() -> Crc64 {
        Crc64(!0)
    }

    pub(crate) fn update(&mut self, data: &[u8]) {
        let t = &CRC64_TABLES;
        let mut c = self.0;
        let mut chunks = data.chunks_exact(8);
        for w in &mut chunks {
            let v = c ^ u64::from_le_bytes(w.try_into().unwrap_or([0; 8]));
            c = t[7][(v & 0xFF) as usize]
                ^ t[6][((v >> 8) & 0xFF) as usize]
                ^ t[5][((v >> 16) & 0xFF) as usize]
                ^ t[4][((v >> 24) & 0xFF) as usize]
                ^ t[3][((v >> 32) & 0xFF) as usize]
                ^ t[2][((v >> 40) & 0xFF) as usize]
                ^ t[1][((v >> 48) & 0xFF) as usize]
                ^ t[0][(v >> 56) as usize];
        }
        for &b in chunks.remainder() {
            c = (c >> 8) ^ t[0][((c ^ u64::from(b)) & 0xFF) as usize];
        }
        self.0 = c;
    }

    pub(crate) fn finish(self) -> u64 {
        !self.0
    }
}

pub(crate) fn crc64(data: &[u8]) -> u64 {
    let mut c = Crc64::new();
    c.update(data);
    c.finish()
}

/// The table for bzip2's CRC-32 (polynomial 0x04C11DB7, not reflected).
const CRC32_BE_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = (i as u32) << 24;
        let mut k = 0;
        while k < 8 {
            c = if c & 0x8000_0000 != 0 {
                (c << 1) ^ 0x04C1_1DB7
            } else {
                c << 1
            };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

/// bzip2's block CRC: CRC-32, most significant bit first, inverted at both ends.
#[derive(Clone, Copy)]
pub(crate) struct Crc32Be(u32);

impl Crc32Be {
    pub(crate) fn new() -> Crc32Be {
        Crc32Be(!0)
    }

    #[inline]
    pub(crate) fn byte(&mut self, b: u8) {
        self.0 = (self.0 << 8) ^ CRC32_BE_TABLE[((self.0 >> 24) as u8 ^ b) as usize];
    }

    pub(crate) fn update(&mut self, data: &[u8]) {
        for &b in data {
            self.byte(b);
        }
    }

    pub(crate) fn finish(self) -> u32 {
        !self.0
    }
}

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256_block(h: &mut [u32; 8], block: &[u8]) {
    let mut w = [0u32; 64];
    for (i, c) in block.chunks_exact(4).enumerate().take(16) {
        w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = *h;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = hh
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(SHA256_K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        hh = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (x, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
        *x = x.wrapping_add(v);
    }
}

/// SHA-256 of `data`.
pub(crate) fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut chunks = data.chunks_exact(64);
    for c in &mut chunks {
        sha256_block(&mut h, c);
    }
    let rest = chunks.remainder();
    let mut tail = [0u8; 128];
    tail[..rest.len()].copy_from_slice(rest);
    tail[rest.len()] = 0x80;
    let n = if rest.len() < 56 { 64 } else { 128 };
    let bits = (data.len() as u64).wrapping_mul(8);
    tail[n - 8..n].copy_from_slice(&bits.to_be_bytes());
    for c in tail[..n].chunks_exact(64) {
        sha256_block(&mut h, c);
    }
    let mut out = [0u8; 32];
    for (o, v) in out.chunks_exact_mut(4).zip(h) {
        o.copy_from_slice(&v.to_be_bytes());
    }
    out
}

const P1: u64 = 0x9E37_79B1_85EB_CA87;
const P2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const P3: u64 = 0x1656_67B1_9E37_79F9;
const P4: u64 = 0x85EB_CA77_C2B2_AE63;
const P5: u64 = 0x27D4_EB2F_1656_67C5;

#[inline]
fn xxh_round(acc: u64, lane: u64) -> u64 {
    acc.wrapping_add(lane.wrapping_mul(P2))
        .rotate_left(31)
        .wrapping_mul(P1)
}

#[inline]
fn xxh_merge(acc: u64, v: u64) -> u64 {
    (acc ^ xxh_round(0, v)).wrapping_mul(P1).wrapping_add(P4)
}

fn le64(b: &[u8]) -> u64 {
    u64::from_le_bytes(b[..8].try_into().unwrap_or([0; 8]))
}

/// XXH64 of `data` with seed 0 (Zstandard's content checksum is its low 32 bits).
pub(crate) fn xxh64(data: &[u8]) -> u64 {
    let len = data.len() as u64;
    let mut rest = data;
    let mut h = if data.len() >= 32 {
        let mut v = [P1.wrapping_add(P2), P2, 0, 0u64.wrapping_sub(P1)];
        let mut stripes = data.chunks_exact(32);
        for s in &mut stripes {
            for (k, acc) in v.iter_mut().enumerate() {
                *acc = xxh_round(*acc, le64(&s[k * 8..]));
            }
        }
        rest = stripes.remainder();
        let mut h = v[0]
            .rotate_left(1)
            .wrapping_add(v[1].rotate_left(7))
            .wrapping_add(v[2].rotate_left(12))
            .wrapping_add(v[3].rotate_left(18));
        for acc in v {
            h = xxh_merge(h, acc);
        }
        h
    } else {
        P5
    };
    h = h.wrapping_add(len);
    let mut words = rest.chunks_exact(8);
    for w in &mut words {
        h ^= xxh_round(0, le64(w));
        h = h.rotate_left(27).wrapping_mul(P1).wrapping_add(P4);
    }
    let mut rest = words.remainder();
    if rest.len() >= 4 {
        let v = u64::from(u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]));
        h ^= v.wrapping_mul(P1);
        h = h.rotate_left(23).wrapping_mul(P2).wrapping_add(P3);
        rest = &rest[4..];
    }
    for &b in rest {
        h ^= u64::from(b).wrapping_mul(P5);
        h = h.rotate_left(11).wrapping_mul(P1);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(P2);
    h ^= h >> 29;
    h = h.wrapping_mul(P3);
    h ^= h >> 32;
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        // Check values from the xz file-format specification and common test vectors.
        assert_eq!(crc64(b"123456789"), 0x995D_C9BB_DF19_39FA);
        assert_eq!(crc64(b""), 0);
        let mut c = Crc32Be::new();
        c.update(b"123456789");
        assert_eq!(c.finish(), 0xFC89_1918);
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(xxh64(b""), 0xEF46_DB37_51D8_E999);
        assert_eq!(xxh64(b"a"), 0xD24E_C4F1_A98C_6E5B);
        assert_eq!(xxh64(b"abc"), 0x44BC_2CF5_AD77_0999);
        // Longer than one stripe, and both block paths of CRC-64.
        let long: Vec<u8> = (0..1000u32).map(|i| (i * 7 + 3) as u8).collect();
        let mut c = Crc64::new();
        c.update(&long[..13]);
        c.update(&long[13..]);
        assert_eq!(c.finish(), crc64(&long));
    }
}
