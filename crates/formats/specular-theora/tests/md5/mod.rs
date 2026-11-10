//! MD5 (RFC 1321) for comparing frames with ffmpeg's framemd5.
#![allow(clippy::needless_range_loop)]

pub struct Md5 {
    state: [u32; 4],
    buf: Vec<u8>,
    len: u64,
}

const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

impl Md5 {
    pub fn new() -> Md5 {
        Md5 {
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            buf: Vec::new(),
            len: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.len += data.len() as u64;
        self.buf.extend_from_slice(data);
        let full = self.buf.len() / 64 * 64;
        let blocks: Vec<u8> = self.buf.drain(..full).collect();
        for c in blocks.chunks_exact(64) {
            self.block(c);
        }
    }

    fn block(&mut self, c: &[u8]) {
        let m: Vec<u32> = c
            .chunks_exact(4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
            .collect();
        let [mut a, mut b, mut cc, mut d] = self.state;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & cc) | (!b & d), i),
                1 => ((d & b) | (!d & cc), (5 * i + 1) % 16),
                2 => (b ^ cc ^ d, (3 * i + 5) % 16),
                _ => (cc ^ (b | !d), (7 * i) % 16),
            };
            let k = ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32;
            let t = a.wrapping_add(f).wrapping_add(k).wrapping_add(m[g]);
            a = d;
            d = cc;
            cc = b;
            b = b.wrapping_add(t.rotate_left(S[i]));
        }
        for (s, v) in self.state.iter_mut().zip([a, b, cc, d]) {
            *s = s.wrapping_add(v);
        }
    }

    pub fn hex(mut self) -> String {
        let bits = self.len.wrapping_mul(8);
        let mut pad = vec![0x80u8];
        while (self.buf.len() + pad.len()) % 64 != 56 {
            pad.push(0);
        }
        pad.extend_from_slice(&bits.to_le_bytes());
        let len = self.len;
        self.update(&pad);
        self.len = len;
        self.state
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

#[test]
fn known_digests() {
    let h = |s: &[u8]| {
        let mut m = Md5::new();
        m.update(s);
        m.hex()
    };
    assert_eq!(h(b""), "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(
        h(b"The quick brown fox jumps over the lazy dog"),
        "9e107d9d372bb6826bd81d3542a419d6"
    );
}
