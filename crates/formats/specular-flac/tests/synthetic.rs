//! Hand-built frames for what ffmpeg's encoder does not write: odd bit depths from
//! STREAMINFO, escape-coded partitions, constant subframes, high LPC orders, every header
//! code for block size and rate, and the 33-bit side channel of 32-bit stereo.

use specular_flac::{Decoder, Error, StreamInfo, frames};

struct W {
    out: Vec<u8>,
    acc: u8,
    n: u32,
}

impl W {
    fn new() -> W {
        W {
            out: Vec::new(),
            acc: 0,
            n: 0,
        }
    }
    fn bit(&mut self, b: bool) {
        self.acc = self.acc << 1 | u8::from(b);
        self.n += 1;
        if self.n == 8 {
            self.out.push(self.acc);
            self.acc = 0;
            self.n = 0;
        }
    }
    fn put(&mut self, v: u64, bits: u32) {
        for i in (0..bits).rev() {
            self.bit(v >> i & 1 != 0);
        }
    }
    fn put_signed(&mut self, v: i64, bits: u32) {
        assert!(bits == 64 || (v >= -(1 << (bits - 1)) && v < 1 << (bits - 1)));
        self.put(v as u64 & ((1u128 << bits) - 1) as u64, bits);
    }
    fn align(&mut self) {
        while self.n != 0 {
            self.bit(false);
        }
    }
}

fn crc8(d: &[u8]) -> u8 {
    let mut c = 0u8;
    for &b in d {
        c ^= b;
        for _ in 0..8 {
            c = if c & 0x80 != 0 { c << 1 ^ 7 } else { c << 1 };
        }
    }
    c
}

fn crc16(d: &[u8]) -> u16 {
    let mut c = 0u16;
    for &b in d {
        c ^= u16::from(b) << 8;
        for _ in 0..8 {
            c = if c & 0x8000 != 0 {
                c << 1 ^ 0x8005
            } else {
                c << 1
            };
        }
    }
    c
}

#[derive(Clone)]
enum Res {
    Rice(u32),
    /// Escape with this many raw bits.
    Raw(u32),
}

#[derive(Clone)]
enum Sub {
    Constant,
    Verbatim,
    Fixed(usize, bool, u32, Vec<Res>),
    /// coefficients (newest sample first), precision, shift, 5-bit params, partition order.
    Lpc(Vec<i64>, u32, u32, bool, u32, Vec<Res>),
}

struct Chan {
    samples: Vec<i64>,
    bits: u32,
    wasted: u32,
    sub: Sub,
}

fn residual(w: &mut W, res: &[i64], order: usize, five: bool, porder: u32, parts: &[Res]) {
    w.put(u64::from(five), 2);
    w.put(u64::from(porder), 4);
    let n = res.len() + order;
    let psize = n >> porder;
    assert_eq!(parts.len(), 1 << porder);
    let mut i = 0;
    for (p, part) in parts.iter().enumerate() {
        let count = if p == 0 { psize - order } else { psize };
        let pb = if five { 5 } else { 4 };
        match *part {
            Res::Rice(k) => {
                w.put(u64::from(k), pb);
                for &r in &res[i..i + count] {
                    let f = if r >= 0 {
                        (r as u64) << 1
                    } else {
                        ((-r as u64) << 1) - 1
                    };
                    for _ in 0..f >> k {
                        w.bit(false);
                    }
                    w.bit(true);
                    w.put(f & ((1 << k) - 1), k);
                }
            }
            Res::Raw(nb) => {
                w.put((1 << pb) - 1, pb);
                w.put(u64::from(nb), 5);
                for &r in &res[i..i + count] {
                    if nb > 0 {
                        w.put_signed(r, nb);
                    } else {
                        assert_eq!(r, 0);
                    }
                }
            }
        }
        i += count;
    }
}

fn subframe(w: &mut W, c: &Chan) {
    let s: Vec<i64> = c.samples.iter().map(|&v| v >> c.wasted).collect();
    assert!(c.samples.iter().all(|&v| v == (v >> c.wasted) << c.wasted));
    let bits = c.bits - c.wasted;
    let kind = match &c.sub {
        Sub::Constant => 0,
        Sub::Verbatim => 1,
        Sub::Fixed(o, ..) => 8 + *o as u64,
        Sub::Lpc(co, ..) => 31 + co.len() as u64,
    };
    w.put(kind << 1 | u64::from(c.wasted > 0), 8);
    if c.wasted > 0 {
        w.put(1, c.wasted);
    }
    match &c.sub {
        Sub::Constant => {
            assert!(s.iter().all(|&v| v == s[0]));
            w.put_signed(s[0], bits);
        }
        Sub::Verbatim => s.iter().for_each(|&v| w.put_signed(v, bits)),
        Sub::Fixed(order, five, porder, parts) => {
            let o = *order;
            s[..o].iter().for_each(|&v| w.put_signed(v, bits));
            let res: Vec<i64> = (o..s.len())
                .map(|i| {
                    let p = match o {
                        0 => 0,
                        1 => s[i - 1],
                        2 => 2 * s[i - 1] - s[i - 2],
                        3 => 3 * s[i - 1] - 3 * s[i - 2] + s[i - 3],
                        _ => 4 * s[i - 1] - 6 * s[i - 2] + 4 * s[i - 3] - s[i - 4],
                    };
                    s[i] - p
                })
                .collect();
            residual(w, &res, o, *five, *porder, parts);
        }
        Sub::Lpc(co, precision, shift, five, porder, parts) => {
            let o = co.len();
            s[..o].iter().for_each(|&v| w.put_signed(v, bits));
            w.put(u64::from(precision - 1), 4);
            w.put(u64::from(*shift), 5);
            co.iter().for_each(|&v| w.put_signed(v, *precision));
            let res: Vec<i64> = (o..s.len())
                .map(|i| {
                    let sum: i64 = (0..o).map(|j| co[j] * s[i - 1 - j]).sum();
                    s[i] - (sum >> shift)
                })
                .collect();
            residual(w, &res, o, *five, *porder, parts);
        }
    }
}

/// One frame. `rate` is (code, extra bytes); `bits_code` 0 defers to STREAMINFO.
fn frame(
    variable: bool,
    number: u64,
    rate: (u8, &[u8]),
    chan_code: u8,
    bits_code: u8,
    chans: &[Chan],
) -> Vec<u8> {
    let n = chans[0].samples.len();
    let mut h = vec![0xFF, 0xF8 | u8::from(variable)];
    let (bcode, bextra): (u8, Vec<u8>) = match n {
        192 => (1, vec![]),
        576 | 1152 | 2304 | 4608 => (2 + (n / 576).trailing_zeros() as u8, vec![]),
        256 | 512 | 1024 | 2048 | 4096 | 8192 | 16384 | 32768 => {
            (8 + (n / 256).trailing_zeros() as u8, vec![])
        }
        1..=256 => (6, vec![(n - 1) as u8]),
        _ => (7, ((n - 1) as u16).to_be_bytes().to_vec()),
    };
    h.push(bcode << 4 | rate.0);
    h.push(chan_code << 4 | bits_code << 1);
    // UTF-8 style coded number.
    if number < 0x80 {
        h.push(number as u8);
    } else {
        let mut extra = 1;
        while number >> (6 * extra) >= 1 << (6 - extra) {
            extra += 1;
        }
        let lead = (0xFF00u16 >> (extra + 1)) as u8;
        h.push(lead | (number >> (6 * extra)) as u8);
        for i in (0..extra).rev() {
            h.push(0x80 | (number >> (6 * i) & 0x3F) as u8);
        }
    }
    h.extend_from_slice(&bextra);
    h.extend_from_slice(rate.1);
    h.push(crc8(&h));
    let mut w = W::new();
    w.out = h;
    for c in chans {
        subframe(&mut w, c);
    }
    w.align();
    let crc = crc16(&w.out);
    w.out.extend_from_slice(&crc.to_be_bytes());
    w.out
}

fn info(rate: u32, channels: u8, bits: u8) -> StreamInfo {
    StreamInfo {
        min_block: 16,
        max_block: 65535,
        min_frame: 0,
        max_frame: 0,
        sample_rate: rate,
        channels,
        bits,
        total_samples: 0,
        md5: [0; 16],
    }
}

/// A test signal within `bits` bits.
fn wave(n: usize, bits: u32, phase: f64) -> Vec<i64> {
    let amp = ((1i64 << (bits - 1)) - 1) as f64;
    (0..n)
        .map(|i| {
            let t = i as f64 * 0.013 + phase;
            ((t.sin() * 0.8 + (t * 7.3).sin() * 0.15) * amp).round() as i64
        })
        .collect()
}

fn check(data: &[u8], info: Option<StreamInfo>, want: &[Vec<i64>]) -> Decoder {
    let mut d = Decoder::new(info);
    let mut out = Vec::new();
    let n = d.decode_i32(data, &mut out).unwrap();
    assert_eq!(n, want[0].len());
    assert_eq!(d.channels(), want.len());
    for (i, s) in out.iter().enumerate() {
        assert_eq!(
            i64::from(*s),
            want[i % want.len()][i / want.len()],
            "sample {i}"
        );
    }
    d
}

#[test]
fn four_bit_mono_with_escapes() {
    let n = 100;
    let s = wave(n, 4, 0.0);
    let mk = |sub| Chan {
        samples: s.clone(),
        bits: 4,
        wasted: 0,
        sub,
    };
    let mut data = frame(
        false,
        0,
        (4, &[]),
        0,
        0,
        &[mk(Sub::Fixed(
            2,
            false,
            2,
            vec![Res::Raw(5), Res::Rice(1), Res::Raw(3), Res::Rice(0)],
        ))],
    );
    data.extend(frame(false, 1, (4, &[]), 0, 0, &[mk(Sub::Verbatim)]));
    let flat = Chan {
        samples: vec![-3; n],
        bits: 4,
        wasted: 0,
        sub: Sub::Constant,
    };
    data.extend(frame(false, 2, (0, &[]), 0, 0, &[flat]));
    let mut want = s.clone();
    want.extend(&s);
    want.extend(vec![-3; n]);
    let d = check(&data, Some(info(8000, 1, 4)), &[want]);
    assert_eq!((d.bits(), d.sample_rate()), (4, 8000));
    assert_eq!(frames(&data).count(), 3);
    // Without STREAMINFO the bit depth is unknown.
    let r = Decoder::new(None).decode_i32(&data, &mut Vec::new());
    assert!(matches!(r, Err(Error::Malformed(_))));
}

#[test]
fn twelve_bit_left_side_lpc_five_bit_params() {
    let n = 300;
    let l = wave(n, 12, 0.0);
    let r = wave(n, 12, 0.7);
    let side: Vec<i64> = l.iter().zip(&r).map(|(a, b)| a - b).collect();
    let lpc = |s: Vec<i64>, bits| Chan {
        samples: s,
        bits,
        wasted: 0,
        sub: Sub::Lpc(
            vec![1800, -1100, 300],
            12,
            10,
            true,
            2,
            vec![Res::Rice(3), Res::Rice(17), Res::Raw(14), Res::Rice(2)],
        ),
    };
    let data = frame(
        true,
        123_456,
        (13, &11025u16.to_be_bytes()),
        8,
        2,
        &[lpc(l.clone(), 12), lpc(side, 13)],
    );
    let d = check(&data, None, &[l, r]);
    assert_eq!((d.bits(), d.sample_rate()), (12, 11025));
}

#[test]
fn twenty_bit_side_right_wasted_bits() {
    let n = 1000;
    let l: Vec<i64> = wave(n, 17, 0.0).iter().map(|v| v * 8).collect();
    let r: Vec<i64> = wave(n, 17, 2.0).iter().map(|v| v * 8).collect();
    let side: Vec<i64> = l.iter().zip(&r).map(|(a, b)| a - b).collect();
    let fx = |s: Vec<i64>, bits| Chan {
        samples: s,
        bits,
        wasted: 3,
        sub: Sub::Fixed(
            4,
            false,
            3,
            vec![
                Res::Rice(6),
                Res::Raw(20),
                Res::Rice(9),
                Res::Rice(14),
                Res::Raw(0),
                Res::Rice(5),
                Res::Rice(8),
                Res::Rice(7),
            ],
        ),
    };
    // Raw(0) needs an all-zero residual partition: use a constant run there.
    let mut l = l;
    let mut r = r;
    for i in 496..625 {
        l[i] = 0;
        r[i] = 0;
    }
    let side2: Vec<i64> = l.iter().zip(&r).map(|(a, b)| a - b).collect();
    let _ = side;
    let data = frame(
        false,
        7,
        (14, &2205u16.to_be_bytes()),
        9,
        5,
        &[fx(side2, 21), fx(r.clone(), 20)],
    );
    let d = check(&data, None, &[l, r]);
    assert_eq!((d.bits(), d.sample_rate()), (20, 22050));
}

#[test]
fn thirty_two_bit_mid_side_needs_33_bits() {
    let n = 192;
    let l: Vec<i64> = (0..n).map(|i| i64::from(i32::MAX) - i as i64 * 3).collect();
    let r: Vec<i64> = (0..n).map(|i| i64::from(i32::MIN) + i as i64 * 5).collect();
    let mid: Vec<i64> = l.iter().zip(&r).map(|(a, b)| (a + b) >> 1).collect();
    let side: Vec<i64> = l.iter().zip(&r).map(|(a, b)| a - b).collect();
    let data = frame(
        true,
        1 << 35,
        (12, &[48]),
        10,
        7,
        &[
            Chan {
                samples: mid,
                bits: 32,
                wasted: 0,
                sub: Sub::Fixed(1, true, 0, vec![Res::Rice(4)]),
            },
            Chan {
                samples: side,
                bits: 33,
                wasted: 0,
                sub: Sub::Verbatim,
            },
        ],
    );
    let d = check(&data, None, &[l.clone(), r.clone()]);
    assert_eq!((d.bits(), d.sample_rate()), (32, 48000));
    // And left/side, side/right with the 33-bit side channel predicted.
    let side: Vec<i64> = l.iter().zip(&r).map(|(a, b)| a - b).collect();
    let sub = || Sub::Fixed(2, true, 1, vec![Res::Rice(3), Res::Rice(3)]);
    let data = frame(
        false,
        0,
        (10, &[]),
        8,
        7,
        &[
            Chan {
                samples: l.clone(),
                bits: 32,
                wasted: 0,
                sub: sub(),
            },
            Chan {
                samples: side.clone(),
                bits: 33,
                wasted: 0,
                sub: sub(),
            },
        ],
    );
    check(&data, None, &[l.clone(), r.clone()]);
    let data = frame(
        false,
        0,
        (10, &[]),
        9,
        7,
        &[
            Chan {
                samples: side,
                bits: 33,
                wasted: 0,
                sub: Sub::Lpc(vec![2, -1], 3, 0, true, 0, vec![Res::Rice(3)]),
            },
            Chan {
                samples: r.clone(),
                bits: 32,
                wasted: 0,
                sub: Sub::Verbatim,
            },
        ],
    );
    let mut f = Vec::new();
    Decoder::new(None).decode(&data, &mut f).unwrap();
    assert!(f.iter().all(|v| (-1.0..=1.0).contains(v)));
    check(&data, None, &[l, r]);
}

#[test]
fn eight_bit_three_channels_order_32() {
    let n = 4096;
    let chans: Vec<Vec<i64>> = (0..3).map(|c| wave(n, 8, f64::from(c))).collect();
    let mut co = vec![0i64; 32];
    co[0] = 16384 - 1;
    co[1] = -8000;
    co[31] = 77;
    let subs: Vec<Chan> = chans
        .iter()
        .enumerate()
        .map(|(i, s)| Chan {
            samples: s.clone(),
            bits: 8,
            wasted: 0,
            sub: match i {
                0 => Sub::Lpc(co.clone(), 15, 14, false, 4, vec![Res::Rice(5); 16]),
                1 => Sub::Lpc(vec![3, -2, 1], 4, 1, false, 0, vec![Res::Rice(6)]),
                _ => Sub::Fixed(0, false, 0, vec![Res::Rice(6)]),
            },
        })
        .collect();
    let data = frame(false, 3, (9, &[]), 2, 1, &subs);
    let d = check(&data, None, &chans);
    assert_eq!((d.bits(), d.sample_rate()), (8, 44100));
}

#[test]
fn twenty_four_bit_wide_lpc_and_header_codes() {
    // Block sizes and rate codes of every kind, one frame each.
    let rates: [(u8, &[u8], u32); 11] = [
        (1, &[], 88200),
        (2, &[], 176400),
        (3, &[], 192000),
        (5, &[], 16000),
        (6, &[], 22050),
        (7, &[], 24000),
        (8, &[], 32000),
        (10, &[], 48000),
        (11, &[], 96000),
        (12, &[96], 96000),
        (13, &[0xAC, 0x44], 44100),
    ];
    let sizes = [192, 576, 1152, 2304, 4608, 256, 512, 8192, 16, 65535, 4000];
    for ((code, extra, hz), n) in rates.into_iter().zip(sizes) {
        let s = wave(n, 24, 0.3);
        let data = frame(
            false,
            1,
            (code, extra),
            0,
            6,
            &[Chan {
                samples: s.clone(),
                bits: 24,
                wasted: 0,
                sub: Sub::Lpc(vec![8192, -4096], 15, 12, true, 0, vec![Res::Rice(20)]),
            }],
        );
        let d = check(&data, None, &[s]);
        assert_eq!(d.sample_rate(), hz, "rate code {code}");
    }
}

#[test]
fn forbidden_codes_are_refused() {
    let s = wave(192, 16, 0.0);
    let good = frame(
        false,
        0,
        (9, &[]),
        0,
        4,
        &[Chan {
            samples: s.clone(),
            bits: 16,
            wasted: 0,
            sub: Sub::Lpc(vec![1, 0], 2, 0, false, 0, vec![Res::Rice(14)]),
        }],
    );
    let mut d = Decoder::new(None);
    let mut out = Vec::new();
    d.decode_i32(&good, &mut out).unwrap();
    let hdr_len = 6;
    let refix = |mut f: Vec<u8>| {
        f[hdr_len - 1] = crc8(&f[..hdr_len - 1]);
        let l = f.len();
        let c = crc16(&f[..l - 2]);
        f[l - 2..].copy_from_slice(&c.to_be_bytes());
        f
    };
    // Reserved block size, rate 15, channel 11, depth 3, reserved bit.
    for (i, v) in [(2, 0x09u8), (2, 0xCF), (3, 0xB8), (3, 0x06), (3, 0x09)] {
        let mut f = good.clone();
        f[i] = v;
        let f = refix(f);
        assert!(d.decode_i32(&f, &mut out).is_err(), "byte {i} = {v:#x}");
    }
    // Precision code 15 and a negative shift (LPC order 2: header byte, 2x16 warm-up bits).
    let p = hdr_len + 1 + 4;
    let mut f = good.clone();
    f[p] |= 0xF0;
    assert!(d.decode_i32(&refix(f), &mut out).is_err());
    let mut f = good.clone();
    f[p] |= 0x08;
    assert!(d.decode_i32(&refix(f), &mut out).is_err());
    // Header CRC-8.
    let mut f = good.clone();
    f[hdr_len - 1] ^= 1;
    assert!(d.decode_i32(&f, &mut out).is_err());
    // Still fine afterwards.
    out.clear();
    d.decode_i32(&good, &mut out).unwrap();
    assert_eq!(out.len(), 192);
}
