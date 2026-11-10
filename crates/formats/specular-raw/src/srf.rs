//! Sony SRF (DSC-F828): 14-bit samples, big-endian 16-bit, under an XOR stream.
//!
//! The stream is a lagged-Fibonacci style generator of 32-bit words: four seed words from a
//! linear congruential sequence of the key (multiplier 48828125, increment 1), the first 127
//! words filled from shifted XORs of earlier ones, then each new word is the XOR of the words
//! 1 and 65 places ahead in a 128-word ring. Each word, big-endian, is XORed onto four data
//! bytes. The file holds a key at a fixed place (a byte at 200896 gives the word offset of the
//! big-endian key word); that key decrypts 40 bytes at 164600, whose bytes 22..26 (little-
//! endian) are the key of the sensor data. The sensor data is one stream over all rows.

/// The XOR stream.
struct Pad {
    ring: [u32; 128],
    p: usize,
}

impl Pad {
    fn new(key: u32) -> Pad {
        let mut ring = [0u32; 128];
        let mut k = key;
        for r in ring.iter_mut().take(4) {
            k = k.wrapping_mul(48_828_125).wrapping_add(1);
            *r = k;
        }
        ring[3] = ring[3] << 1 | (ring[0] ^ ring[2]) >> 31;
        for p in 4..127 {
            ring[p] = (ring[p - 4] ^ ring[p - 2]) << 1 | (ring[p - 3] ^ ring[p - 1]) >> 31;
        }
        Pad { ring, p: 127 }
    }

    fn next(&mut self) -> u32 {
        let q = self.p;
        self.p += 1;
        let v = self.ring[(q + 1) & 127] ^ self.ring[(q + 65) & 127];
        self.ring[q & 127] = v;
        v
    }

    /// XORs the stream onto `data` (whole words; a partial tail is left as is).
    fn apply(&mut self, data: &mut [u8]) {
        for w in data.chunks_exact_mut(4) {
            for (b, k) in w.iter_mut().zip(self.next().to_be_bytes()) {
                *b ^= k;
            }
        }
    }
}

/// Where the F828's sensor data starts.
pub(crate) const DATA: usize = 862_144;

/// The key of the sensor data, from the file's key block.
pub(crate) fn key(d: &[u8]) -> Option<u32> {
    let at = 200_896 + *d.get(200_896)? as usize * 4;
    let k = u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?);
    let mut head: [u8; 40] = d.get(164_600..164_640)?.try_into().ok()?;
    Pad::new(k).apply(&mut head);
    Some(u32::from_le_bytes(head[22..26].try_into().ok()?))
}

/// Decrypts and unpacks `out.len()` samples from `src`; true when the data ran short.
pub(crate) fn decode(src: &[u8], key: u32, out: &mut [u16]) -> bool {
    let n = out.len() * 2;
    let mut buf = src[..n.min(src.len())].to_vec();
    buf.resize(n.next_multiple_of(4), 0);
    Pad::new(key).apply(&mut buf);
    for (o, b) in out.iter_mut().zip(buf.chunks_exact(2)) {
        *o = u16::from_be_bytes([b[0], b[1]]);
    }
    src.len() < n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_is_its_own_inverse() {
        let plain: Vec<u8> = (0..64u32)
            .flat_map(|v| ((v * 300) as u16).to_be_bytes())
            .collect();
        let mut enc = plain.clone();
        Pad::new(0x1234_5678).apply(&mut enc);
        assert_ne!(enc, plain);
        let mut out = vec![0u16; 64];
        assert!(!decode(&enc, 0x1234_5678, &mut out));
        assert_eq!(
            out,
            (0..64u32).map(|v| (v * 300) as u16).collect::<Vec<_>>()
        );
        // The stream does not repeat within its ring.
        let mut p = Pad::new(7);
        let a: Vec<u32> = (0..128).map(|_| p.next()).collect();
        let b: Vec<u32> = (0..128).map(|_| p.next()).collect();
        assert_ne!(a, b);
    }

    #[test]
    fn short_and_missing_data() {
        let mut out = vec![0u16; 8];
        assert!(decode(&[1, 2, 3], 1, &mut out));
        assert!(key(&[0; 100]).is_none());
        let mut f = vec![0u8; 200_900];
        f[200_896] = 0;
        assert!(key(&f).is_some());
    }
}
