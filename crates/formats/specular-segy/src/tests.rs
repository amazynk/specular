use super::*;

/// A SEG-Y file in memory: `traces` × `ns` samples of format `fmt`, values `f(trace, k)`.
struct Build {
    big: bool,
    fmt: i16,
    ns: usize,
    rev: (u8, u8),
    ebcdic: bool,
    byte_order_word: bool,
    keys: Vec<(i32, i32)>,
    lengths: Option<Vec<usize>>,
}

impl Build {
    fn new(fmt: i16, ns: usize, keys: Vec<(i32, i32)>) -> Build {
        Build {
            big: true,
            fmt,
            ns,
            rev: (1, 0),
            ebcdic: true,
            byte_order_word: false,
            keys,
            lengths: None,
        }
    }

    fn bytes(&self, f: impl Fn(usize, usize) -> f64) -> Vec<u8> {
        let mut out = Vec::new();
        let card = "C 1 CLIENT: SPECULAR TEST";
        let mut text = vec![b' '; TEXT_BYTES];
        text[..card.len()].copy_from_slice(card.as_bytes());
        for c in 1..40 {
            text[c * 80] = b'C';
        }
        if self.ebcdic {
            for b in &mut text {
                *b = (0x40..=0xFFu8)
                    .find(|&e| ebcdic(e) == char::from(*b))
                    .unwrap_or(0x40);
            }
        }
        out.extend(text);
        let mut bin = vec![0u8; BINARY_BYTES];
        let put16 = |b: &mut Vec<u8>, at: usize, v: i16| {
            let a = if self.big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            };
            b[at - 1..at + 1].copy_from_slice(&a);
        };
        let put32 = |b: &mut [u8], at: usize, v: i32| {
            let a = if self.big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            };
            b[at - 1..at + 3].copy_from_slice(&a);
        };
        put16(&mut bin, 17, 4000);
        put16(&mut bin, 21, self.ns as i16);
        put16(&mut bin, 25, self.fmt);
        bin[300] = self.rev.0;
        bin[301] = self.rev.1;
        put16(&mut bin, 303, i16::from(self.lengths.is_none()));
        if self.byte_order_word {
            put32(&mut bin, 97, 0x0102_0304);
        }
        out.extend(bin);
        let fmt = SampleFormat::from_code(i64::from(self.fmt)).unwrap();
        for (t, &(il, xl)) in self.keys.iter().enumerate() {
            let ns = self.lengths.as_ref().map_or(self.ns, |l| l[t]);
            let mut h = [0u8; TRACE_HEADER];
            put32(&mut h, 1, t as i32 + 1);
            put32(&mut h, 21, 1000 + t as i32);
            put16_arr(&mut h, 71, -100, self.big);
            put32(&mut h, 181, 12_345_678 + t as i32);
            put32(&mut h, 185, 98_765_432);
            put32(&mut h, 189, il);
            put32(&mut h, 193, xl);
            put16_arr(&mut h, 109, 100, self.big);
            put16_arr(&mut h, 115, ns as i16, self.big);
            put16_arr(&mut h, 117, 4000, self.big);
            out.extend(h);
            for k in 0..ns {
                let v = f(t, k);
                let b: Vec<u8> = match fmt {
                    SampleFormat::F32 => {
                        let x = v as f32;
                        (if self.big {
                            x.to_be_bytes()
                        } else {
                            x.to_le_bytes()
                        })
                        .to_vec()
                    }
                    SampleFormat::I16 => {
                        let x = v as i16;
                        (if self.big {
                            x.to_be_bytes()
                        } else {
                            x.to_le_bytes()
                        })
                        .to_vec()
                    }
                    SampleFormat::Ibm32 => ibm_bytes(v as f32).to_vec(),
                    SampleFormat::I24 => {
                        let x = (v as i32).to_be_bytes();
                        if self.big {
                            x[1..].to_vec()
                        } else {
                            vec![x[3], x[2], x[1]]
                        }
                    }
                    SampleFormat::I8 => vec![v as i8 as u8],
                    _ => unimplemented!(),
                };
                out.extend(b);
            }
        }
        out
    }
}

fn put16_arr(b: &mut [u8], at: usize, v: i16, big: bool) {
    let a = if big {
        v.to_be_bytes()
    } else {
        v.to_le_bytes()
    };
    b[at - 1..at + 1].copy_from_slice(&a);
}

/// An f32 as IBM float, big-endian (exact for values with ≤ 21 significant bits).
fn ibm_bytes(v: f32) -> [u8; 4] {
    if v == 0.0 {
        return [0; 4];
    }
    let sign = if v < 0.0 { 0x80u32 << 24 } else { 0 };
    let mut m = f64::from(v.abs());
    let mut e = 64i32;
    while m >= 1.0 {
        m /= 16.0;
        e += 1;
    }
    while m < 1.0 / 16.0 {
        m *= 16.0;
        e -= 1;
    }
    let frac = (m * f64::from(1u32 << 24)) as u32;
    (sign | (e as u32) << 24 | frac).to_be_bytes()
}

fn grid(ni: i32, nx: i32) -> Vec<(i32, i32)> {
    (0..ni)
        .flat_map(|i| (0..nx).map(move |x| (100 + i, 20 + 2 * x)))
        .collect()
}

#[test]
fn ibm_big_endian_rev1() {
    let b = Build::new(1, 50, grid(3, 4));
    let s = Segy::from_bytes(b.bytes(|t, k| (t as f64 - 5.0) * 0.25 + k as f64)).unwrap();
    assert_eq!(s.kind, Kind::SegY);
    assert_eq!(s.endian, Endian::Big);
    assert!(s.ebcdic);
    assert_eq!(s.text[0], "C 1 CLIENT: SPECULAR TEST");
    assert_eq!(s.format, SampleFormat::Ibm32);
    assert_eq!((s.traces, s.samples), (12, 50));
    assert_eq!(s.interval, 4000.0);
    assert_eq!(s.delay, 100.0);
    assert_eq!(s.revision().as_deref(), Some("1.0"));
    assert_eq!(s.sample_position(10.0), 140.0);
    let t = s.trace(7).unwrap();
    assert_eq!(t[3], 0.5 + 3.0);
    let h = s.header(2).unwrap();
    assert_eq!(h.i32(189), 100);
    assert_eq!(h.i32(193), 24);
    assert_eq!(h.cdp_xy(), (123_456.80, 987_654.32));
    let g = s.geometry().unwrap();
    let v = g.volume.unwrap();
    assert_eq!(v.inlines, [100, 101, 102]);
    assert_eq!(v.crosslines, [20, 22, 24, 26]);
    let sec = v.inline_section(&s, 1).unwrap();
    assert_eq!(sec[3 * 4 + 2], s.trace(6).unwrap()[3]);
    let all = v.read(&s, |_| true).unwrap().unwrap();
    assert_eq!(all[(2 * 4 + 1) * 50 + 9], s.trace(9).unwrap()[9]);
    let xs = v.crossline_section(&s, 3).unwrap();
    assert_eq!(xs[5 * 3 + 2], s.trace(11).unwrap()[5]);
}

#[test]
fn little_endian_rev2_with_byte_order_word() {
    let mut b = Build::new(5, 20, grid(2, 3));
    b.big = false;
    b.rev = (2, 1);
    b.ebcdic = false;
    b.byte_order_word = true;
    let s = Segy::from_bytes(b.bytes(|t, k| t as f64 * 1.5 - k as f64)).unwrap();
    assert_eq!(s.endian, Endian::Little);
    assert!(!s.ebcdic);
    assert_eq!(s.revision().as_deref(), Some("2.1"));
    assert_eq!(s.trace(4).unwrap()[7], 6.0 - 7.0);
    assert!(s.binary.as_ref().unwrap().byte_order_word);
}

#[test]
fn little_endian_without_the_word() {
    let mut b = Build::new(3, 10, grid(2, 2));
    b.big = false;
    let s = Segy::from_bytes(b.bytes(|t, k| (t * 100 + k) as f64 - 150.0)).unwrap();
    assert_eq!(s.endian, Endian::Little);
    assert_eq!(s.format, SampleFormat::I16);
    assert_eq!(s.trace(3).unwrap()[4], 154.0);
}

#[test]
fn int24() {
    let b = Build::new(7, 8, grid(2, 2));
    let s = Segy::from_bytes(b.bytes(|t, k| -(t as f64) * 70_000.0 + k as f64)).unwrap();
    assert_eq!(s.trace(2).unwrap()[3], -140_000.0 + 3.0);
}

#[test]
fn variable_trace_length_is_refused() {
    let mut b = Build::new(5, 10, grid(2, 2));
    b.lengths = Some(vec![10, 12, 10, 11]);
    let e = Segy::from_bytes(b.bytes(|_, _| 0.0)).unwrap_err();
    assert!(
        matches!(
            e,
            Error::VariableLength {
                trace: 1,
                samples: 12,
                first: 10
            }
        ),
        "{e}"
    );
    assert!(e.to_string().contains("variable trace length"));
}

#[test]
fn cut_file_keeps_whole_traces() {
    let b = Build::new(5, 10, grid(2, 2));
    let mut bytes = b.bytes(|_, k| k as f64);
    bytes.truncate(bytes.len() - 7);
    let s = Segy::from_bytes(bytes).unwrap();
    assert_eq!(s.traces, 3);
    assert!(s.trailing > 0);
}

#[test]
fn fixed_point_format_is_named() {
    let b = Build::new(5, 4, grid(2, 2));
    let mut bytes = b.bytes(|_, _| 0.0);
    bytes[3224..3226].copy_from_slice(&4i16.to_be_bytes());
    let e = Segy::from_bytes(bytes).unwrap_err();
    assert!(e.to_string().contains("format 4"), "{e}");
}

#[test]
fn seismic_unix_both_byte_orders() {
    let b = Build::new(5, 30, grid(1, 5));
    for big in [false, true] {
        let mut bb = Build::new(5, 30, grid(1, 5));
        bb.big = big;
        let full = bb.bytes(|t, k| (t * 1000 + k) as f64);
        let su = full[TEXT_BYTES + BINARY_BYTES..].to_vec();
        let s = Segy::su_from_bytes(su).unwrap();
        assert_eq!(s.kind, Kind::Su);
        assert_eq!(s.big(), big);
        assert_eq!((s.traces, s.samples), (5, 30));
        assert_eq!(s.trace(4).unwrap()[29], 4029.0);
        let g = s.geometry().unwrap();
        assert!(g.volume.is_none());
        assert_eq!(g.cdp[2], 1002);
    }
    let _ = b;
}

#[test]
fn sniffing() {
    let b = Build::new(1, 10, grid(2, 2));
    let bytes = b.bytes(|_, _| 0.0);
    assert!(sniff(&bytes));
    assert!(!sniff(&bytes[..3000]));
    assert!(!sniff(&[0u8; 4000]));
}

mod hostile {
    use super::*;

    /// A rev 2 binary header whose first-trace offset is u64::MAX: no overflow, no traces.
    #[test]
    fn first_trace_offset_past_the_end() {
        let mut b = Build::new(5, 20, grid(2, 2));
        b.rev = (2, 0);
        let mut bytes = b.bytes(|_, _| 0.0);
        bytes[TEXT_BYTES + 320..TEXT_BYTES + 328].copy_from_slice(&u64::MAX.to_be_bytes());
        let s = Segy::from_bytes(bytes).unwrap();
        assert_eq!(s.traces, 0);
        assert!(s.trace(0).is_err());
        assert!(s.geometry().is_ok());
    }

    /// One-byte samples on a grid a fifth full: the volume comes out about 18 times the
    /// file, reserved from the memory budget before it is allocated (sections too).
    #[test]
    fn sparse_volume_reserved() {
        use specular_core::budget::with_limit;
        // Inlines 100 and 101 full (50 crosslines), one trace on inline 109: 101 traces on
        // a 10 × 50 grid.
        let mut keys = grid(2, 50);
        keys.push((109, 20));
        let b = Build::new(8, 2000, keys);
        let bytes = b.bytes(|t, k| ((t + k) % 100) as f64);
        assert!(bytes.len() < 240 << 10);
        let s = Segy::from_bytes(bytes).unwrap();
        let v = s.geometry().unwrap().volume.unwrap();
        assert_eq!((v.inlines.len(), v.crosslines.len()), (10, 50));
        // 500 cells × 2000 samples × 4 bytes: 4 MB.
        let r = with_limit(2 << 20, || v.read(&s, |_| true));
        assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
        // A crossline: 10 inlines × 2000 samples × 4 bytes, 80 KB.
        let r = with_limit(64 << 10, || v.crossline_section(&s, 0));
        assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
        let all = with_limit(8 << 20, || v.read(&s, |_| true))
            .unwrap()
            .unwrap();
        assert_eq!(all.len(), 500 * 2000);
        assert_eq!(all[7], 7.0);
        assert!(all[(2 * 50) * 2000].is_nan());
    }

    /// A 2^31-sample trace in a file with no traces: no allocation for a trace.
    #[test]
    fn huge_sample_count_without_data() {
        let b = Build::new(5, 0, Vec::new());
        let mut bytes = b.bytes(|_, _| 0.0);
        bytes[TEXT_BYTES + 68..TEXT_BYTES + 72].copy_from_slice(&i32::MAX.to_be_bytes());
        bytes.extend([0u8; 100]);
        let s = Segy::from_bytes(bytes).unwrap();
        assert_eq!(s.traces, 0);
        assert!(s.trace(0).is_err());
    }
}
