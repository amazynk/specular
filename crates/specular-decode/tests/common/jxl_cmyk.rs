//! An animated CMYK JPEG XL written bit by bit (ISO/IEC 18181-1), for tests: 8×8, 8-bit,
//! sRGB with one black extra channel, 100 ticks a second, lossless modular; each frame fills
//! C, M, Y and K with one value through a one-leaf MA tree (Zero predictor) and
//! single-symbol prefix codes, so its colours are known exactly.

/// LSB-first bit writer, as JPEG XL reads.
#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    n: usize,
}

impl Bits {
    fn put(&mut self, v: u32, bits: usize) {
        for i in 0..bits {
            if self.n.is_multiple_of(8) {
                self.out.push(0);
            }
            if v >> i & 1 != 0 {
                *self.out.last_mut().unwrap() |= 1 << (self.n % 8);
            }
            self.n += 1;
        }
    }
    fn bit(&mut self, b: bool) {
        self.put(u32::from(b), 1);
    }
    /// A U32 field's selector (`then` extra bits of `v` after it).
    fn sel(&mut self, s: u32) {
        self.put(s, 2);
    }
    fn pad(&mut self) {
        self.n = self.out.len() * 8;
    }
}

/// One frame's section: LfGlobal with a global one-leaf tree (Zero predictor) and a data
/// code that always gives `2 v` (UnpackSigned: `v`), so every sample of every channel is `v`.
fn section(v: u32) -> Vec<u8> {
    let mut b = Bits::default();
    b.bit(true); // LfChannelDequantization: all default
    b.bit(true); // GlobalModular: global tree present
    // Tree code over 6 contexts: no LZ77, simple clustering (all to cluster 0), prefix codes,
    // split exponent 15, alphabet of one symbol (0).
    b.bit(false);
    b.bit(true);
    b.put(0, 2);
    b.bit(true);
    b.put(15, 4);
    b.bit(false);
    // (The tree itself: property 0 = leaf, predictor 0, offset 0, multiplier 1; zero bits.)
    // Data code over the one leaf: no LZ77, prefix code, split 15, `count` symbols, simple
    // code of one symbol `2 v`.
    b.bit(false);
    b.bit(true);
    b.put(15, 4);
    let sym = 2 * v;
    let n = 31 - sym.leading_zeros();
    b.bit(true);
    b.put(n, 4);
    b.put(sym - (1 << n), n as usize);
    let count = 1 + sym;
    let alphabet_bits = count.next_power_of_two().trailing_zeros() as usize;
    b.put(1, 2); // HSKIP 1: simple code
    b.put(0, 2); // one symbol
    b.put(sym, alphabet_bits);
    // Modular stream header: the global tree, default weighted predictor, no transforms.
    b.bit(true);
    b.bit(true);
    b.sel(0);
    // Samples: zero bits each.
    b.out
}

fn frame(b: &mut Bits, v: u32, duration: u32, last: bool) {
    b.bit(false); // all_default
    b.put(0, 2); // regular frame
    b.put(1, 1); // modular
    b.sel(0); // flags
    b.bit(false); // no YCbCr
    b.sel(0); // upsampling 1
    b.sel(0); // black channel upsampling 1
    b.put(1, 2); // group size shift
    b.sel(0); // one pass
    b.bit(false); // no crop
    b.sel(0); // replace
    b.sel(0); // black channel: replace
    b.sel(2);
    b.put(duration, 8);
    b.bit(last);
    if !last {
        b.put(0, 2); // save_as_reference
    }
    b.sel(0); // no name
    b.bit(true); // restoration filter: default
    b.sel(0); // extensions
    // TOC: one entry.
    let s = section(v);
    b.bit(false);
    b.pad();
    b.sel(0);
    b.put(s.len() as u32, 10);
    b.pad();
    for byte in s {
        b.put(u32::from(byte), 8);
    }
}

/// 8×8, 8-bit, sRGB, one black channel, animated at 100 ticks a second; one frame per value.
pub fn animated_cmyk(values: &[u32]) -> Vec<u8> {
    let mut b = Bits::default();
    b.put(0xFF, 8);
    b.put(0x0A, 8);
    b.bit(true); // div8
    b.put(0, 5); // height 8
    b.put(1, 3); // width = height
    b.bit(false); // metadata: not all default
    b.bit(true); // extra fields
    b.put(0, 3); // orientation 1
    b.bit(false); // intrinsic size
    b.bit(false); // preview
    b.bit(true); // animation
    b.sel(0); // 100 ticks / s
    b.sel(0); // per 1
    b.sel(0); // loops forever
    b.bit(false); // timecodes
    b.bit(false); // integer samples
    b.sel(0); // 8 bits
    b.bit(true); // 16-bit buffers
    b.sel(1); // one extra channel
    b.bit(false); // not the default alpha
    b.sel(2);
    b.put(2, 4); // type 4: black
    b.bit(false);
    b.sel(0); // 8 bits
    b.sel(0); // dim shift 0
    b.sel(0); // no name
    b.bit(false); // not XYB
    b.bit(true); // colour encoding: sRGB
    b.bit(true); // tone mapping: default
    b.sel(0); // extensions
    b.bit(true); // default transform
    b.pad();
    for (i, &v) in values.iter().enumerate() {
        frame(&mut b, v, 10, i + 1 == values.len());
        b.pad();
    }
    b.out
}
