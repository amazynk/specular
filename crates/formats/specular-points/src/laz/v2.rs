//! Pointwise items (LASzip compressor 2, item version 2): one arithmetic stream for all items
//! of a point, in item order. Point10 (formats 0–5), GPS time, RGB and extra bytes.

use super::arith::{Decoder, IntCoder, Median5, Model, clamp, fold};
use super::tables::{RETURN_MAP, return_level};
use crate::las::Rec;

/// The Point10 item's state: the last point's 20 raw bytes, decoded.
pub(crate) struct Point10 {
    x: i32,
    y: i32,
    z: i32,
    intensity: u16,
    /// Return number (bits 0–2), number of returns (3–5), scan direction (6), edge (7).
    bits: u8,
    class: u8,
    scan_angle: u8,
    user_data: u8,
    psid: u16,
    changed: Model,
    bit_byte: Vec<Option<Model>>,
    classification: Vec<Option<Model>>,
    user: Vec<Option<Model>>,
    scan_angle_models: [Model; 2],
    ic_intensity: IntCoder,
    ic_psid: IntCoder,
    ic_dx: IntCoder,
    ic_dy: IntCoder,
    ic_z: IntCoder,
    med_x: [Median5; 16],
    med_y: [Median5; 16],
    last_intensity: [u16; 16],
    last_height: [i32; 8],
}

#[inline]
fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

#[inline]
fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

impl Point10 {
    /// From the chunk's first point (20 bytes).
    pub(crate) fn new(raw: &[u8]) -> Point10 {
        let none = || (0..256).map(|_| None).collect::<Vec<_>>();
        Point10 {
            x: i32_at(raw, 0),
            y: i32_at(raw, 4),
            z: i32_at(raw, 8),
            // The format starts from a zero intensity, not the first point's.
            intensity: 0,
            bits: raw[14],
            class: raw[15],
            scan_angle: raw[16],
            user_data: raw[17],
            psid: u16_at(raw, 18),
            changed: Model::new(64),
            bit_byte: none(),
            classification: none(),
            user: none(),
            scan_angle_models: [Model::new(256), Model::new(256)],
            ic_intensity: IntCoder::new(16, 4),
            ic_psid: IntCoder::new(16, 1),
            ic_dx: IntCoder::new(32, 2),
            ic_dy: IntCoder::new(32, 22),
            ic_z: IntCoder::new(32, 20),
            med_x: [Median5::default(); 16],
            med_y: [Median5::default(); 16],
            last_intensity: [0; 16],
            last_height: [0; 8],
        }
    }

    pub(crate) fn read(&mut self, d: &mut Decoder) {
        let changed = d.symbol(&mut self.changed);
        if changed != 0 {
            if changed & 32 != 0 {
                let m = self.bit_byte[self.bits as usize].get_or_insert_with(|| Model::new(256));
                self.bits = d.symbol(m) as u8;
            }
            let (r, n) = (u32::from(self.bits & 7), u32::from((self.bits >> 3) & 7));
            let m = RETURN_MAP[n as usize][r as usize] as usize;
            if changed & 16 != 0 {
                let v =
                    self.ic_intensity
                        .decode(d, i32::from(self.last_intensity[m]), m.min(3) as u32);
                self.intensity = v as u16;
                self.last_intensity[m] = self.intensity;
            } else {
                self.intensity = self.last_intensity[m];
            }
            if changed & 8 != 0 {
                let model =
                    self.classification[self.class as usize].get_or_insert_with(|| Model::new(256));
                self.class = d.symbol(model) as u8;
            }
            if changed & 4 != 0 {
                let dir = usize::from((self.bits >> 6) & 1);
                let v = d.symbol(&mut self.scan_angle_models[dir]);
                self.scan_angle = fold(v as i32 + i32::from(self.scan_angle));
            }
            if changed & 2 != 0 {
                let model =
                    self.user[self.user_data as usize].get_or_insert_with(|| Model::new(256));
                self.user_data = d.symbol(model) as u8;
            }
            if changed & 1 != 0 {
                self.psid = self.ic_psid.decode(d, i32::from(self.psid), 0) as u16;
            }
        }
        let (r, n) = (u32::from(self.bits & 7), u32::from((self.bits >> 3) & 7));
        let m = RETURN_MAP[n as usize][r as usize] as usize;
        let l = return_level(n, r);
        let one = u32::from(n == 1);

        let diff = self.ic_dx.decode(d, self.med_x[m].get(), one);
        self.x = self.x.wrapping_add(diff);
        self.med_x[m].add(diff);

        let k = self.ic_dx.k;
        let diff = self.ic_dy.decode(
            d,
            self.med_y[m].get(),
            one + if k < 20 { k & !1 } else { 20 },
        );
        self.y = self.y.wrapping_add(diff);
        self.med_y[m].add(diff);

        let k = (self.ic_dx.k + self.ic_dy.k) / 2;
        self.z = self.ic_z.decode(
            d,
            self.last_height[l],
            one + if k < 18 { k & !1 } else { 18 },
        );
        self.last_height[l] = self.z;
    }

    /// The current point into `rec` (formats 0–5: a 5-bit class).
    pub(crate) fn put(&self, rec: &mut Rec) {
        rec.x = self.x;
        rec.y = self.y;
        rec.z = self.z;
        rec.intensity = self.intensity;
        rec.class = self.class & 0x1F;
        rec.returns = (self.bits & 7) | (((self.bits >> 3) & 7) << 4);
    }

    /// The first point of a chunk, which is stored raw (with its own intensity).
    pub(crate) fn put_first(raw: &[u8], rec: &mut Rec) {
        rec.x = i32_at(raw, 0);
        rec.y = i32_at(raw, 4);
        rec.z = i32_at(raw, 8);
        rec.intensity = u16_at(raw, 12);
        rec.class = raw[15] & 0x1F;
        rec.returns = (raw[14] & 7) | (((raw[14] >> 3) & 7) << 4);
    }
}

const MULTI: i32 = 500;
const MULTI_MINUS: i32 = -10;
const MULTI_UNCHANGED: u32 = (MULTI - MULTI_MINUS + 1) as u32;
const MULTI_CODE_FULL: u32 = (MULTI - MULTI_MINUS + 2) as u32;
const MULTI_TOTAL: u32 = (MULTI - MULTI_MINUS + 6) as u32;

/// GPS time (item version 2): up to four interleaved sequences, each predicted by a multiple
/// of its last difference.
pub(crate) struct GpsTime {
    multi: Model,
    zero_diff: Model,
    ic: IntCoder,
    last: usize,
    next: usize,
    time: [i64; 4],
    diff: [i32; 4],
    extreme: [u32; 4],
}

impl GpsTime {
    pub(crate) fn new(raw: &[u8]) -> GpsTime {
        let mut b = [0u8; 8];
        b.copy_from_slice(&raw[..8]);
        GpsTime {
            multi: Model::new(MULTI_TOTAL),
            zero_diff: Model::new(6),
            ic: IntCoder::new(32, 9),
            last: 0,
            next: 0,
            time: [i64::from_le_bytes(b), 0, 0, 0],
            diff: [0; 4],
            extreme: [0; 4],
        }
    }

    fn full(&mut self, d: &mut Decoder) {
        self.next = (self.next + 1) & 3;
        let hi = self
            .ic
            .decode(d, (self.time[self.last] as u64 >> 32) as i32, 8);
        let lo = d.int();
        self.time[self.next] = (((hi as u32 as u64) << 32) | u64::from(lo)) as i64;
        self.last = self.next;
        self.diff[self.last] = 0;
        self.extreme[self.last] = 0;
    }

    /// The diff after a multiplied prediction, adopting it as the sequence's difference after
    /// four extreme ones in a row.
    fn extreme_diff(&mut self, d: &mut Decoder, pred: i32, ctx: u32) -> i32 {
        let diff = self.ic.decode(d, pred, ctx);
        let l = self.last;
        self.extreme[l] += 1;
        if self.extreme[l] > 3 {
            self.diff[l] = diff;
            self.extreme[l] = 0;
        }
        diff
    }

    pub(crate) fn read(&mut self, d: &mut Decoder) {
        // A corrupt stream could switch sequences forever; four switches visit them all.
        for _ in 0..16 {
            let l = self.last;
            if self.diff[l] == 0 {
                let multi = d.symbol(&mut self.zero_diff);
                if multi == 1 {
                    self.diff[l] = self.ic.decode(d, 0, 0);
                    self.time[l] = self.time[l].wrapping_add(i64::from(self.diff[l]));
                    self.extreme[l] = 0;
                } else if multi == 2 {
                    self.full(d);
                } else if multi > 2 {
                    self.last = (l + multi as usize - 2) & 3;
                    continue;
                }
            } else {
                let multi = d.symbol(&mut self.multi);
                let last_diff = self.diff[l];
                if multi == 1 {
                    let v = self.ic.decode(d, last_diff, 1);
                    self.time[l] = self.time[l].wrapping_add(i64::from(v));
                    self.extreme[l] = 0;
                } else if multi < MULTI_UNCHANGED {
                    let m = multi as i32;
                    let diff = if multi == 0 {
                        self.extreme_diff(d, 0, 7)
                    } else if m < MULTI {
                        let ctx = if m < 10 { 2 } else { 3 };
                        self.ic.decode(d, m.wrapping_mul(last_diff), ctx)
                    } else if m == MULTI {
                        self.extreme_diff(d, MULTI.wrapping_mul(last_diff), 4)
                    } else {
                        let m = MULTI - m;
                        if m > MULTI_MINUS {
                            self.ic.decode(d, m.wrapping_mul(last_diff), 5)
                        } else {
                            self.extreme_diff(d, MULTI_MINUS.wrapping_mul(last_diff), 6)
                        }
                    };
                    self.time[l] = self.time[l].wrapping_add(i64::from(diff));
                } else if multi == MULTI_CODE_FULL {
                    self.full(d);
                } else if multi > MULTI_CODE_FULL {
                    self.last = (l + (multi - MULTI_CODE_FULL) as usize) & 3;
                    continue;
                }
            }
            break;
        }
    }
}

/// RGB (item version 2 for Point10 formats; the same coding as the layered RGB14 item):
/// which bytes changed, then each changed byte as a difference, green and blue predicted from
/// red's change.
pub(crate) struct Rgb {
    used: Model,
    diff: [Model; 6],
    pub(crate) last: [u16; 3],
}

impl Rgb {
    pub(crate) fn new(raw: &[u8]) -> Rgb {
        Rgb {
            used: Model::new(128),
            diff: std::array::from_fn(|_| Model::new(256)),
            last: [0, 2, 4].map(|o| u16_at(raw, o)),
        }
    }

    pub(crate) fn read(&mut self, d: &mut Decoder) {
        let last = self.last;
        let sym = d.symbol(&mut self.used);
        let lo = |v: u16| i32::from(v & 0xFF);
        let hi = |v: u16| i32::from(v >> 8);
        let mut c = [0u16; 3];
        c[0] = if sym & 1 != 0 {
            u16::from(fold(d.symbol(&mut self.diff[0]) as i32 + lo(last[0])))
        } else {
            last[0] & 0xFF
        };
        c[0] |= if sym & 2 != 0 {
            u16::from(fold(d.symbol(&mut self.diff[1]) as i32 + hi(last[0]))) << 8
        } else {
            last[0] & 0xFF00
        };
        if sym & (1 << 6) != 0 {
            let mut diff = lo(c[0]) - lo(last[0]);
            c[1] = if sym & 4 != 0 {
                let corr = d.symbol(&mut self.diff[2]) as i32;
                u16::from(fold(corr + clamp(diff + lo(last[1]))))
            } else {
                last[1] & 0xFF
            };
            c[2] = if sym & 16 != 0 {
                let corr = d.symbol(&mut self.diff[4]) as i32;
                diff = (diff + (lo(c[1]) - lo(last[1]))) / 2;
                u16::from(fold(corr + clamp(diff + lo(last[2]))))
            } else {
                last[2] & 0xFF
            };
            let mut diff = hi(c[0]) - hi(last[0]);
            c[1] |= if sym & 8 != 0 {
                let corr = d.symbol(&mut self.diff[3]) as i32;
                u16::from(fold(corr + clamp(diff + hi(last[1])))) << 8
            } else {
                last[1] & 0xFF00
            };
            c[2] |= if sym & 32 != 0 {
                let corr = d.symbol(&mut self.diff[5]) as i32;
                diff = (diff + (hi(c[1]) - hi(last[1]))) / 2;
                u16::from(fold(corr + clamp(diff + hi(last[2])))) << 8
            } else {
                last[2] & 0xFF00
            };
        } else {
            c[1] = c[0];
            c[2] = c[0];
        }
        self.last = c;
    }
}

/// Extra bytes (item version 2): each byte a difference to its last value.
pub(crate) struct Bytes {
    models: Vec<Model>,
    last: Vec<u8>,
}

impl Bytes {
    pub(crate) fn new(raw: &[u8]) -> Bytes {
        Bytes {
            models: raw.iter().map(|_| Model::new(256)).collect(),
            last: raw.to_vec(),
        }
    }

    pub(crate) fn read(&mut self, d: &mut Decoder) {
        for (m, b) in self.models.iter_mut().zip(&mut self.last) {
            *b = fold(i32::from(*b) + d.symbol(m) as i32);
        }
    }
}
