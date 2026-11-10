//! Inputs found by fuzzing (`fuzz/`), each built here from a small synthetic frame.

/// MSB-first bit writer.
#[derive(Default)]
struct Bits {
    d: Vec<u8>,
    n: usize,
}

impl Bits {
    fn put(&mut self, v: u64, len: u32) {
        for i in (0..len).rev() {
            if self.n.is_multiple_of(8) {
                self.d.push(0);
            }
            if (v >> i) & 1 == 1 {
                *self.d.last_mut().unwrap() |= 0x80 >> (self.n % 8);
            }
            self.n += 1;
        }
    }
    fn bits(&mut self, s: &str) {
        for c in s.bytes().filter(|c| *c != b' ') {
            self.put(u64::from(c == b'1'), 1);
        }
    }
}

/// An access unit of one 10-bit 4:0:0 frame `w × h`, one tile, whose luma data is `tile`.
fn access_unit(w: u32, h: u32, tile: &[u8]) -> Vec<u8> {
    let mut f = Bits::default();
    // frame_info: profile, level, band + reserved, width, height, chroma 0, depth 10 - 8,
    // capture distance, reserved.
    f.put(33, 8);
    f.put(30, 8);
    f.put(0, 8);
    f.put(u64::from(w), 24);
    f.put(u64::from(h), 24);
    f.put(0, 4);
    f.put(2, 4);
    f.put(0, 16);
    // reserved, no colour description, no matrices, the tile in macroblocks, no tile
    // sizes, reserved, aligned.
    f.put(0, 8);
    f.put(0, 2);
    f.put(u64::from(w.div_ceil(16)), 20);
    f.put(u64::from(h.div_ceil(16)), 20);
    f.put(0, 9);
    let mut frame = f.d;
    // The tile: its header (size, reserved, component size, QP), then the data.
    let mut t = 9u16.to_be_bytes().to_vec();
    t.extend_from_slice(&[0, 0]);
    t.extend_from_slice(&(tile.len() as u32).to_be_bytes());
    t.push(20);
    t.extend_from_slice(tile);
    frame.extend_from_slice(&(t.len() as u32).to_be_bytes());
    frame.extend(t);
    let mut au = b"aPv1".to_vec();
    au.extend_from_slice(&(frame.len() as u32 + 4).to_be_bytes());
    au.extend_from_slice(&[1, 0, 1, 0]);
    au.extend(frame);
    au
}

/// All-zero AC levels: a run of 63 (`01`, five `0`, `1`, then 30 in five bits).
const NO_AC: &str = "01 00000 1 11110";

#[test]
fn flat_frame_is_mid_grey() {
    let mut b = Bits::default();
    // The first DC difference is coded with k = 5 (a `1` and five bits), the rest with 0.
    b.bits("1 00000");
    b.bits(NO_AC);
    for _ in 0..3 {
        b.bits("1");
        b.bits(NO_AC);
    }
    let p = specular_apv::decode(&access_unit(16, 16, &b.d)).unwrap();
    assert_eq!(p.damaged_tiles, 0);
    assert!(p.planes[0].iter().all(|&v| v == 512));
}

/// DC differences near the code's largest (about 2^25) add up past `i32` over 128 blocks: the
/// predicted DC wrapped (an overflow panic in debug builds) instead of decoding the tile.
#[test]
fn dc_prediction_does_not_overflow() {
    let mut b = Bits::default();
    for _ in 0..128 {
        // `01`, 19 `0`s, `1`, 24 one bits: 2^25 + 2^24 - 1 + ... ; positive.
        b.bits("01");
        b.put(0, 19);
        b.bits("1");
        b.put((1 << 24) - 1, 24);
        b.bits("0");
        b.bits(NO_AC);
    }
    let p = specular_apv::decode(&access_unit(128, 64, &b.d)).unwrap();
    assert_eq!(p.planes[0].len(), 128 * 64);
}
