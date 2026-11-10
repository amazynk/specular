//! Round trips through a small reference encoder written from the blue book (split-sample,
//! second-extension, zero-block and no-compression options; signed and unsigned mapping).
//! libaec's own streams are checked by the GRIB and HDF5 oracle tests.

use super::*;

struct W {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl W {
    fn put(&mut self, v: u64, k: u32) {
        for i in (0..k).rev() {
            self.acc = (self.acc << 1) | ((v >> i) & 1);
            self.n += 1;
            if self.n == 8 {
                self.out.push(self.acc as u8);
                self.acc = 0;
                self.n = 0;
            }
        }
    }
    fn fs(&mut self, m: u64) {
        for _ in 0..m {
            self.put(0, 1);
        }
        self.put(1, 1);
    }
    fn align(&mut self) {
        while self.n != 0 {
            self.put(0, 1);
        }
    }
}

fn map(x: i64, pred: i64, xmin: i64, xmax: i64) -> u64 {
    let d = x - pred;
    let theta = (pred - xmin).min(xmax - pred);
    if (0..=theta).contains(&d) {
        (2 * d) as u64
    } else if d < 0 && -d <= theta {
        (2 * -d - 1) as u64
    } else {
        (theta + d.abs()) as u64
    }
}

fn encode(x: &[i64], p: &Params) -> Vec<u8> {
    let n = p.bits_per_sample;
    let id_len = p.id_len();
    let id_max = (1u64 << id_len) - 1;
    let signed = p.flags & SIGNED != 0;
    let (xmin, xmax) = if signed {
        (-(1i64 << (n - 1)), (1i64 << (n - 1)) - 1)
    } else {
        (0, (1i64 << n) - 1)
    };
    let mask = (1u64 << n) - 1;
    let bs = p.block_size as usize;
    let rsi_len = bs * p.rsi as usize;
    let mut w = W {
        out: Vec::new(),
        acc: 0,
        n: 0,
    };
    for rsi in x.chunks(rsi_len) {
        let mut res: Vec<u64> = Vec::with_capacity(rsi_len);
        for (i, &v) in rsi.iter().enumerate() {
            res.push(if i == 0 {
                (v as u64) & mask
            } else {
                map(v, rsi[i - 1], xmin, xmax)
            });
        }
        // The last interval's last block repeats its final residual (zero after the first).
        while !res.len().is_multiple_of(bs) {
            res.push(0);
        }
        for (b, blk) in res.chunks(bs).enumerate() {
            let r = usize::from(b == 0);
            let body = &blk[r..];
            if body.iter().all(|&d| d == 0) {
                w.put(0, id_len);
                w.put(0, 1);
                if r == 1 {
                    w.put(blk[0], n);
                }
                w.fs(0);
                continue;
            }
            // Cheapest split k.
            let mut best = (u64::MAX, 0u32);
            let kmax = id_max.checked_sub(2).map(|k| (k as u32).min(n - 1));
            for k in kmax.into_iter().flat_map(|m| 0..=m) {
                let c: u64 = body.iter().map(|&d| (d >> k) + 1 + u64::from(k)).sum();
                if c < best.0 {
                    best = (c, k);
                }
            }
            // Second extension when every pair's sum stays small.
            let mut padded = vec![0u64; r];
            padded.extend_from_slice(body);
            let se: Option<u64> = padded
                .chunks(2)
                .map(|q| {
                    let s = q[0] + q[1];
                    (s <= 12).then(|| s * (s + 1) / 2 + q[1] + 1)
                })
                .sum();
            let raw = (bs as u64) * u64::from(n);
            if let Some(se) = se.filter(|&c| c < best.0 && c < raw) {
                let _ = se;
                w.put(0, id_len);
                w.put(1, 1);
                if r == 1 {
                    w.put(blk[0], n);
                }
                for q in padded.chunks(2) {
                    let s = q[0] + q[1];
                    w.fs(s * (s + 1) / 2 + q[1]);
                }
            } else if best.0 < raw {
                let k = best.1;
                w.put(u64::from(k) + 1, id_len);
                if r == 1 {
                    w.put(blk[0], n);
                }
                for &d in body {
                    w.fs(d >> k);
                }
                for &d in body {
                    w.put(d & ((1 << k) - 1), k);
                }
            } else {
                w.put(id_max, id_len);
                for &d in blk {
                    w.put(d, n);
                }
            }
        }
        if p.flags & PAD_RSI != 0 {
            w.align();
        }
    }
    w.align();
    w.out
}

fn lcg(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *seed >> 33
}

fn check(n: u32, flags: u8, bs: u32, rsi: u32, x: &[i64]) {
    let p = Params {
        bits_per_sample: n,
        block_size: bs,
        rsi,
        flags: flags | PREPROCESS,
    };
    let s = encode(x, &p);
    let got = decode(&s, &p, x.len()).unwrap();
    let mask = if n == 32 { u32::MAX } else { (1 << n) - 1 };
    for (i, (&g, &w)) in got.iter().zip(x).enumerate() {
        assert_eq!(g, (w as u32) & mask, "n={n} flags={flags} bs={bs} at {i}");
    }
    assert_eq!(got.len(), x.len());
}

#[test]
fn round_trips() {
    let mut seed = 7u64;
    for n in [1u32, 2, 3, 4, 7, 8, 9, 12, 16, 17, 24, 31] {
        for signed in [false, true] {
            for restricted in [false, true] {
                let (lo, hi) = if signed {
                    (-(1i64 << (n - 1)), (1i64 << (n - 1)) - 1)
                } else {
                    (0, (1i64 << n) - 1)
                };
                let mut x = Vec::new();
                let mut v = (lo + hi) / 2;
                for i in 0..3000 {
                    let step = match (i / 300) % 4 {
                        0 => 0,
                        1 => (lcg(&mut seed) % 5) as i64 - 2,
                        2 => (lcg(&mut seed) % 2000) as i64 - 1000,
                        _ => lcg(&mut seed) as i64,
                    };
                    v = if (i / 300) % 4 == 3 {
                        lo + (step.rem_euclid(hi - lo + 1))
                    } else {
                        (v + step).clamp(lo, hi)
                    };
                    x.push(v);
                }
                let mut flags = if signed { SIGNED } else { 0 };
                if restricted {
                    flags |= RESTRICTED;
                }
                for (bs, rsi) in [(8, 1), (16, 128), (32, 7), (64, 4096)] {
                    check(n, flags, bs, rsi, &x);
                    check(n, flags | PAD_RSI, bs, rsi, &x);
                }
            }
        }
    }
}

#[test]
fn second_extension_table() {
    // Codeword m covers pairs (a, b) in order of sum, then of b.
    let mut m = 0;
    for s in 0..13u8 {
        for b in 0..=s {
            let (sum, first) = SE[m];
            assert_eq!(sum, s);
            assert_eq!(m - usize::from(first), usize::from(b));
            m += 1;
        }
    }
}

#[test]
fn bytes_layout() {
    let p = Params {
        bits_per_sample: 12,
        block_size: 8,
        rsi: 2,
        flags: PREPROCESS | MSB,
    };
    let x: Vec<i64> = (0..20).map(|i| 0x123 + i).collect();
    let s = encode(&x, &p);
    let b = decode_bytes(&s, &p, 20).unwrap();
    assert_eq!(&b[..4], &[0x01, 0x23, 0x01, 0x24]);
    let le = Params {
        flags: PREPROCESS,
        ..p
    };
    let b = decode_bytes(&encode(&x, &le), &le, 20).unwrap();
    assert_eq!(&b[..4], &[0x23, 0x01, 0x24, 0x01]);
}

#[test]
fn truncated_and_bad_params() {
    let p = Params {
        bits_per_sample: 16,
        block_size: 16,
        rsi: 4,
        flags: PREPROCESS,
    };
    let x: Vec<i64> = (0..200).map(|i| i * 37 % 1000).collect();
    let s = encode(&x, &p);
    assert_eq!(decode(&s[..s.len() / 2], &p, 200), Err(Error::Truncated));
    let bad = Params { block_size: 7, ..p };
    assert!(matches!(decode(&s, &bad, 200), Err(Error::Params(_))));
}

mod hostile {
    use super::*;

    /// A huge scanline with a small block: `rsi * block` overflowed u32 before the parameter
    /// check ran.
    #[test]
    fn szip_huge_scanline() {
        let p = szip::SzParams {
            options: szip::RAW | szip::NN,
            bits_per_pixel: 16,
            pixels_per_block: 2,
            pixels_per_scanline: u32::MAX,
        };
        assert!(matches!(
            szip::decode(&[0; 8], &p, 64),
            Err(Error::Params(_))
        ));
    }

    /// A count of four billion samples from a header over a few bytes: no 16 GB reserve, just
    /// a truncation error.
    #[test]
    fn huge_count_tiny_input() {
        let p = Params {
            bits_per_sample: 16,
            block_size: 16,
            rsi: 4,
            flags: PREPROCESS,
        };
        assert_eq!(
            decode(&[0xff; 4], &p, u32::MAX as usize),
            Err(Error::Truncated)
        );
    }
}
