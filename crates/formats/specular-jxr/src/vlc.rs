//! Variable-length code tables (T.832 clause 8) and the adaptive table selection of 8.8.

use std::sync::LazyLock;

use crate::bits::Bits;

/// A prefix code decoded by one table lookup on the next `max` bits.
pub(crate) struct Vlc {
    max: u32,
    /// `symbol | length << 8` for every `max`-bit prefix; 0 marks an invalid prefix.
    lut: Vec<u16>,
}

impl Vlc {
    /// Codes given as bit strings in symbol order.
    fn new(codes: &[&str]) -> Vlc {
        let max = codes.iter().map(|c| c.len()).max().unwrap_or(1) as u32;
        let mut lut = vec![0u16; 1 << max];
        for (sym, code) in codes.iter().enumerate() {
            let len = code.len() as u32;
            let v = u32::from_str_radix(code, 2).unwrap_or(0);
            let lo = (v << (max - len)) as usize;
            let hi = ((v + 1) << (max - len)) as usize;
            for e in &mut lut[lo..hi] {
                *e = sym as u16 | (len as u16) << 8;
            }
        }
        Vlc { max, lut }
    }

    #[inline]
    pub(crate) fn get(&self, b: &mut Bits) -> usize {
        let e = self.lut[b.peek(self.max) as usize];
        b.skip((e >> 8) as u32);
        (e & 0xFF) as usize
    }
}

pub(crate) struct Tables {
    pub abs_level: [Vlc; 2],
    pub num_cbphp: [Vlc; 2],
    pub num_blk_cbphp: [Vlc; 2],
    /// CHR_CBPHP, VAL_INC and CBPHP_CH_BLK.
    pub three: Vlc,
    /// REF_CBPHP1 as indices into `REF_CBPHP1_VAL`.
    pub ref_cbphp1: Vlc,
    pub num_ch_blk: Vlc,
    pub val_dc_yuv: Vlc,
    pub cbplp444: Vlc,
    pub cbplp42x: Vlc,
    /// DECODE_RUN for iMaxRun 2, 3 and 4 (symbol + 1 is the run).
    pub run: [Vlc; 3],
    pub run_index: Vlc,
    pub index_a: [Vlc; 4],
    pub index_b: Vlc,
    pub first_index: [Vlc; 5],
}

pub(crate) const REF_CBPHP1_VAL: [u32; 6] = [3, 5, 6, 9, 10, 12];

pub(crate) static T: LazyLock<Tables> = LazyLock::new(|| Tables {
    abs_level: [
        Vlc::new(&["01", "10", "11", "001", "0001", "00000", "00001"]),
        Vlc::new(&["1", "01", "001", "0001", "00001", "000000", "000001"]),
    ],
    num_cbphp: [
        Vlc::new(&["1", "01", "001", "0000", "0001"]),
        Vlc::new(&["1", "000", "001", "010", "011"]),
    ],
    num_blk_cbphp: [
        Vlc::new(&[
            "010", "00000", "0010", "00001", "00010", "1", "011", "00011", "0011",
        ]),
        Vlc::new(&[
            "1", "001", "010", "0001", "000001", "011", "00001", "0000000", "0000001",
        ]),
    ],
    three: Vlc::new(&["1", "01", "00"]),
    ref_cbphp1: Vlc::new(&["00", "01", "100", "101", "110", "111"]),
    num_ch_blk: Vlc::new(&["1", "01", "000", "001"]),
    val_dc_yuv: Vlc::new(&["10", "001", "00001", "0001", "11", "010", "00000", "011"]),
    cbplp444: Vlc::new(&["0", "100", "1010", "1011", "1100", "1101", "1110", "1111"]),
    cbplp42x: Vlc::new(&["0", "10", "110", "111"]),
    run: [
        Vlc::new(&["1", "0"]),
        Vlc::new(&["1", "01", "00"]),
        Vlc::new(&["1", "01", "001", "000"]),
    ],
    run_index: Vlc::new(&["1", "01", "001", "0000", "0001"]),
    index_a: [
        Vlc::new(&["1", "00000", "001", "00001", "01", "0001"]),
        Vlc::new(&["01", "0000", "10", "0001", "11", "001"]),
        Vlc::new(&["0000", "0001", "01", "10", "11", "001"]),
        Vlc::new(&["00000", "00001", "01", "1", "0001", "001"]),
    ],
    index_b: Vlc::new(&["0", "110", "10", "111"]),
    first_index: [
        Vlc::new(&[
            "00001", "000001", "0000000", "0000001", "00100", "010", "00101", "1", "00110", "0001",
            "00111", "011",
        ]),
        Vlc::new(&[
            "0010", "00010", "000000", "000001", "0011", "010", "00011", "11", "011", "100",
            "00001", "101",
        ]),
        Vlc::new(&[
            "11", "001", "0000000", "0000001", "00001", "010", "0000010", "011", "100", "101",
            "0000011", "0001",
        ]),
        Vlc::new(&[
            "001", "11", "0000000", "00001", "00010", "010", "0000001", "011", "00011", "100",
            "000001", "101",
        ]),
        Vlc::new(&[
            "010", "1", "0000001", "0001", "0000010", "011", "00000000", "0010", "0000011", "0011",
            "00000001", "00001",
        ]),
    ],
});

pub(crate) const ABS_LEVEL_DELTA: [i32; 7] = [1, 0, -1, -1, -1, -1, -1];
pub(crate) const NUM_CBPHP_DELTA: [i32; 5] = [0, -1, 0, 1, 1];
pub(crate) const NUM_BLK_CBPHP_DELTA9: [i32; 9] = [2, 2, 1, 1, -1, -2, -2, -2, -3];
pub(crate) const INDEX1_DELTA: [[i32; 6]; 3] = [
    [-1, 1, 1, 1, 0, 1],
    [-2, 0, 0, 2, 0, 0],
    [-1, -1, 0, 1, -2, 0],
];
pub(crate) const FIRST_INDEX_DELTA: [[i32; 12]; 4] = [
    [1, 1, 1, 1, 1, 0, 0, -1, 2, 1, 0, 0],
    [2, 2, -1, -1, -1, 0, -2, -1, 0, 0, -2, -1],
    [-1, 1, 0, 2, 0, 0, 0, 0, -2, 0, 1, 1],
    [0, 1, 0, 1, -2, 0, -1, -1, -2, -1, -2, -2],
];

/// AdaptiveVLC state (8.8.1).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Adapt {
    pub idx: u8,
    pub delta: u8,
    pub delta2: u8,
    pub d1: i32,
    pub d2: i32,
}

impl Adapt {
    pub(crate) const fn table1() -> Adapt {
        Adapt {
            idx: 0,
            delta: 0,
            delta2: 0,
            d1: 0,
            d2: 0,
        }
    }

    pub(crate) const fn table2() -> Adapt {
        Adapt {
            idx: 1,
            delta: 0,
            delta2: 1,
            d1: 0,
            d2: 0,
        }
    }

    pub(crate) fn adapt1(&mut self) {
        if self.d1 < -8 && self.idx != 0 {
            self.idx -= 1;
            self.d1 = 0;
        } else if self.d1 > 8 && self.idx != 1 {
            self.idx += 1;
            self.d1 = 0;
        } else {
            self.d1 = self.d1.clamp(-64, 64);
        }
    }

    pub(crate) fn adapt2(&mut self, max: u8) {
        let mut change = false;
        if self.d1 < -8 && self.idx != 0 {
            self.idx -= 1;
            change = true;
        } else if self.d2 > 8 && self.idx != max {
            self.idx += 1;
            change = true;
        }
        if change {
            self.d1 = 0;
            self.d2 = 0;
            if self.idx == max {
                self.delta = self.idx - 1;
                self.delta2 = self.idx - 1;
            } else if self.idx == 0 {
                self.delta = 0;
                self.delta2 = 0;
            } else {
                self.delta = self.idx - 1;
                self.delta2 = self.idx;
            }
        } else {
            self.d1 = self.d1.clamp(-64, 64);
            self.d2 = self.d2.clamp(-64, 64);
        }
    }
}
