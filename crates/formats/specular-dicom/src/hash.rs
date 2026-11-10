//! SHA-256 (FIPS 180-4) and HMAC-SHA-256 (RFC 2104), for the de-identifier's keyed UID
//! and pseudonym mapping, and a per-export random key.

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Incremental SHA-256.
#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    block: [u8; 64],
    fill: usize,
    total: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Sha256 {
        Sha256 {
            state: H0,
            block: [0; 64],
            fill: 0,
            total: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);
        if self.fill > 0 {
            let n = (64 - self.fill).min(data.len());
            self.block[self.fill..self.fill + n].copy_from_slice(&data[..n]);
            self.fill += n;
            data = &data[n..];
            if self.fill < 64 {
                return;
            }
            let b = self.block;
            compress(&mut self.state, &b);
            self.fill = 0;
        }
        let mut chunks = data.chunks_exact(64);
        for c in &mut chunks {
            compress(&mut self.state, c.try_into().unwrap_or(&[0; 64]));
        }
        let rest = chunks.remainder();
        self.block[..rest.len()].copy_from_slice(rest);
        self.fill = rest.len();
    }

    pub fn finish(mut self) -> [u8; 32] {
        let bits = self.total.wrapping_mul(8);
        let mut pad = [0u8; 72];
        pad[0] = 0x80;
        let n = if self.fill < 56 {
            56 - self.fill
        } else {
            120 - self.fill
        };
        pad[n..n + 8].copy_from_slice(&bits.to_be_bytes());
        let total = self.total;
        self.update(&pad[..n + 8]);
        self.total = total;
        let mut out = [0u8; 32];
        for (o, s) in out.chunks_exact_mut(4).zip(self.state) {
            o.copy_from_slice(&s.to_be_bytes());
        }
        out
    }
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (i, c) in block.chunks_exact(4).enumerate() {
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
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *s = s.wrapping_add(v);
    }
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finish()
}

/// HMAC-SHA-256 of `parts` (concatenated) under `key`.
pub fn hmac(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&sha256(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(&k.map(|b| b ^ 0x36));
    for p in parts {
        inner.update(p);
    }
    let ih = inner.finish();
    let mut outer = Sha256::new();
    outer.update(&k.map(|b| b ^ 0x5c));
    outer.update(&ih);
    outer.finish()
}

/// 32 random bytes: the kernel's random source, else a mix of process-local entropy (hash
/// seeds, time, pid, addresses) hashed together.
pub fn random_key() -> [u8; 32] {
    use std::io::Read;
    let mut k = [0u8; 32];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom")
        && f.read_exact(&mut k).is_ok()
    {
        return k;
    }
    let mut h = Sha256::new();
    for i in 0..8u64 {
        use std::hash::{BuildHasher, Hasher};
        let mut s = std::collections::hash_map::RandomState::new().build_hasher();
        s.write_u64(i);
        h.update(&s.finish().to_le_bytes());
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    h.update(&now.to_le_bytes());
    h.update(&std::process::id().to_le_bytes());
    h.update(&(&k as *const _ as usize).to_le_bytes());
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn nist_vectors() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let mut h = Sha256::new();
        let a = [b'a'; 1000];
        for _ in 0..1000 {
            h.update(&a);
        }
        assert_eq!(
            hex(&h.finish()),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn split_updates_match() {
        let data: Vec<u8> = (0..300u32).map(|i| (i * 7) as u8).collect();
        for cut in [0, 1, 55, 56, 63, 64, 65, 128, 299] {
            let mut h = Sha256::new();
            h.update(&data[..cut]);
            h.update(&data[cut..]);
            assert_eq!(h.finish(), sha256(&data), "cut {cut}");
        }
    }

    #[test]
    fn rfc4231_hmac() {
        // Test cases 1 and 2.
        assert_eq!(
            hex(&hmac(&[0x0b; 20], &[b"Hi There"])),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        assert_eq!(
            hex(&hmac(b"Jefe", &[b"what do ya want ", b"for nothing?"])),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // Test case 6: a key longer than the block.
        assert_eq!(
            hex(&hmac(
                &[0xaa; 131],
                &[b"Test Using Larger Than Block-Size Key - Hash Key First"]
            )),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn random_keys_differ() {
        assert_ne!(random_key(), random_key());
    }
}
