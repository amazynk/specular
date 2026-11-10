//! A small MD5 (RFC 1321) for comparing frames with `ffmpeg -f framemd5`.

pub struct Md5 {
    s: [u32; 4],
    buf: Vec<u8>,
    len: u64,
    k: [u32; 64],
}

const R: [u32; 16] = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];

impl Md5 {
    pub fn new() -> Md5 {
        let mut k = [0u32; 64];
        for (i, v) in k.iter_mut().enumerate() {
            *v = ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32;
        }
        Md5 {
            s: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            buf: Vec::new(),
            len: 0,
            k,
        }
    }

    fn block(&mut self, b: &[u8]) {
        let m: Vec<u32> = b
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let [mut a, mut bb, mut c, mut d] = self.s;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((bb & c) | (!bb & d), i),
                1 => ((d & bb) | (!d & c), (5 * i + 1) % 16),
                2 => (bb ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (bb | !d), (7 * i) % 16),
            };
            let r = R[(i / 16) * 4 + i % 4];
            let t = d;
            d = c;
            c = bb;
            bb = bb.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(self.k[i])
                    .wrapping_add(m[g])
                    .rotate_left(r),
            );
            a = t;
        }
        self.s[0] = self.s[0].wrapping_add(a);
        self.s[1] = self.s[1].wrapping_add(bb);
        self.s[2] = self.s[2].wrapping_add(c);
        self.s[3] = self.s[3].wrapping_add(d);
    }

    pub fn update(&mut self, data: &[u8]) {
        self.len += data.len() as u64;
        let mut data = data;
        if !self.buf.is_empty() {
            let need = 64 - self.buf.len();
            let take = need.min(data.len());
            self.buf.extend_from_slice(&data[..take]);
            data = &data[take..];
            if self.buf.len() == 64 {
                let b = std::mem::take(&mut self.buf);
                self.block(&b);
            }
        }
        let mut chunks = data.chunks_exact(64);
        for c in &mut chunks {
            self.block(c);
        }
        self.buf.extend_from_slice(chunks.remainder());
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
        self.s
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

#[test]
fn known_vectors() {
    let h = |s: &str| {
        let mut m = Md5::new();
        m.update(s.as_bytes());
        m.hex()
    };
    assert_eq!(h(""), "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(h("abc"), "900150983cd24fb0d6963f7d28e17f72");
    assert_eq!(
        h("12345678901234567890123456789012345678901234567890123456789012345678901234567890"),
        "57edf4a22be3c955ac49da2e2107b67a"
    );
}
