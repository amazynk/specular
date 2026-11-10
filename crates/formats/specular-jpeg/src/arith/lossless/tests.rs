//! SOF11 against its own ground truth and against Huffman (SOF3) twins. No tool on hand writes
//! arithmetic lossless files (libjpeg-turbo 2.x has no lossless mode, ImageMagick and ffmpeg
//! none with arithmetic coding), so the tests carry a minimal encoder written from T.81 (Annex D
//! encoder flow charts, H.1 prediction and statistics): every file is checked against the
//! samples it was made from, and, where `jpeg-decoder` decodes the SOF3 twin correctly (point
//! transform 0, no restarts, no subsampling), against the twin through the public API. The
//! Huffman lossless (SOF3) decoder is tested here too, with the same encoder.

use super::super::QE;
use crate::{DecodeOptions, DecodedPlanes, Parallelism, Region, Scale, decode};

/// T.81 D.1 binary arithmetic encoder (register layout of Table D.2, carry handling and byte
/// stuffing of Figures D.8–D.11, flush of D.13/D.14).
struct Enc {
    c: u64,
    a: u32,
    ct: u32,
    /// Stacked 0xFF bytes waiting for a possible carry.
    st: usize,
    out: Vec<u8>,
}

impl Enc {
    fn new() -> Enc {
        Enc {
            c: 0,
            a: 0x10000,
            ct: 11,
            st: 0,
            out: Vec::new(),
        }
    }

    fn code(&mut self, bin: &mut u8, bit: bool) {
        let (mut state, mut mps) = ((*bin & 0x7F) as usize, *bin >> 7 != 0);
        let (qe, nlps, nmps, switch) = QE[state];
        let qe = qe as u32;
        self.a -= qe;
        if bit == mps {
            if self.a < 0x8000 {
                if self.a < qe {
                    self.c += self.a as u64;
                    self.a = qe;
                }
                state = nmps as usize;
                self.renorm();
            }
        } else {
            if self.a >= qe {
                self.c += self.a as u64;
                self.a = qe;
            }
            if switch {
                mps = !mps;
            }
            state = nlps as usize;
            self.renorm();
        }
        *bin = state as u8 | (mps as u8) << 7;
    }

    fn renorm(&mut self) {
        loop {
            self.a <<= 1;
            self.c <<= 1;
            self.ct -= 1;
            if self.ct == 0 {
                self.byte_out();
                self.ct = 8;
            }
            if self.a >= 0x8000 {
                break;
            }
        }
    }

    fn byte_out(&mut self) {
        let t = self.c >> 19;
        if t > 0xFF {
            // Carry into the last byte; the stacked 0xFF bytes become zeros.
            let b = self.out.last_mut().expect("carry before the first byte");
            *b = b.wrapping_add(1);
            if *b == 0xFF {
                self.out.push(0);
            }
            for _ in 0..self.st {
                self.out.push(0);
            }
            self.st = 0;
            assert_ne!(t & 0xFF, 0xFF);
            self.out.push(t as u8);
        } else if t == 0xFF {
            self.st += 1;
        } else {
            for _ in 0..self.st {
                self.out.extend_from_slice(&[0xFF, 0]);
            }
            self.st = 0;
            self.out.push(t as u8);
        }
        self.c &= 0x7FFFF;
    }

    fn flush(mut self) -> Vec<u8> {
        let mut t = (self.c + self.a as u64 - 1) & 0xFFFF_0000;
        if t < self.c {
            t += 0x8000;
        }
        self.c = t << self.ct;
        self.byte_out();
        self.c <<= 8;
        self.byte_out();
        for _ in 0..self.st {
            self.out.extend_from_slice(&[0xFF, 0]);
        }
        // Discard_final_zeros: trailing zeros that are not stuffing.
        while self.out.len() >= 2
            && self.out[self.out.len() - 1] == 0
            && self.out[self.out.len() - 2] != 0xFF
        {
            self.out.pop();
        }
        if self.out == [0] {
            self.out.clear();
        }
        self.out
    }
}

/// Huffman bit writer (MSB first, 0xFF stuffing, 1-padding).
struct Bits {
    acc: u64,
    n: u32,
    out: Vec<u8>,
}

impl Bits {
    fn put(&mut self, v: u32, len: u32) {
        if len == 0 {
            return;
        }
        self.acc = (self.acc << len) | (v as u64 & ((1u64 << len) - 1));
        self.n += len;
        while self.n >= 8 {
            self.n -= 8;
            let b = (self.acc >> self.n) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0);
            }
        }
    }
    fn finish(mut self) -> Vec<u8> {
        let pad = (8 - self.n % 8) % 8;
        self.put((1 << pad) - 1, pad);
        self.out
    }
}

#[derive(Clone)]
struct Comp {
    id: u8,
    h: usize,
    v: usize,
    /// DC conditioning table (arithmetic) / Huffman table index.
    t: usize,
}

#[derive(Clone)]
struct Spec {
    precision: u8,
    w: usize,
    h: usize,
    comps: Vec<Comp>,
    /// Component indices per scan.
    scans: Vec<Vec<usize>>,
    predictor: u8,
    pt: u8,
    /// Restart interval in MCU rows.
    ri_rows: usize,
    /// DAC (L, U) per table.
    dac: [(u8, u8); 2],
}

impl Spec {
    fn max(&self) -> (usize, usize) {
        (
            self.comps.iter().map(|c| c.h).max().unwrap(),
            self.comps.iter().map(|c| c.v).max().unwrap(),
        )
    }
    /// Padded plane size of component `k`.
    fn plane(&self, k: usize) -> (usize, usize) {
        let (hm, vm) = self.max();
        (
            self.w.div_ceil(hm) * self.comps[k].h,
            self.h.div_ceil(vm) * self.comps[k].v,
        )
    }
    /// Sample extent of component `k`.
    fn extent(&self, k: usize) -> (usize, usize) {
        let (hm, vm) = self.max();
        (
            (self.w * self.comps[k].h).div_ceil(hm),
            (self.h * self.comps[k].v).div_ceil(vm),
        )
    }
}

/// Category of a difference for the conditioning (0 zero, 1/2 small +/-, 3/4 large +/-).
fn category(v: i32, l: u8, u: u8) -> u8 {
    if v == 0 {
        return 0;
    }
    let sz = v.unsigned_abs() - 1;
    let mag = if sz == 0 {
        0
    } else {
        1i32 << (31 - sz.leading_zeros())
    };
    let neg = (v < 0) as u8;
    if mag < (1i32 << l) >> 1 {
        0
    } else if mag > (1i32 << u) >> 1 {
        3 + neg
    } else {
        1 + neg
    }
}

/// F.1.4.1 / H.1.2.3: code difference `v` with S0 = `s0`, magnitude bins from `x1`.
fn code_diff(e: &mut Enc, st: &mut [u8; 256], s0: usize, x1: usize, v: i32) {
    if v == 0 {
        e.code(&mut st[s0], false);
        return;
    }
    e.code(&mut st[s0], true);
    let neg = v < 0;
    e.code(&mut st[s0 + 1], neg);
    let sz = v.unsigned_abs() - 1;
    let sp = s0 + 2 + neg as usize;
    if sz == 0 {
        e.code(&mut st[sp], false);
        return;
    }
    e.code(&mut st[sp], true);
    let mut m = 1u32;
    let mut i = x1;
    while sz >= 2 * m {
        e.code(&mut st[i], true);
        m <<= 1;
        i += 1;
    }
    e.code(&mut st[i], false);
    i += 14;
    let mut bit = m >> 1;
    while bit != 0 {
        e.code(&mut st[i], sz & bit != 0);
        bit >>= 1;
    }
}

/// The difference of `x` from prediction `p`, modulo 2^16, as coded (-32767..=32768).
fn modulo_diff(x: i32, p: i32) -> i32 {
    let d = (x - p).rem_euclid(1 << 16);
    if d <= 32768 { d } else { d - 65536 }
}

/// Encode `planes` (padded, per component) as SOF11 (`arith`) or SOF3.
fn encode(sp: &Spec, planes: &[Vec<u16>], arith: bool) -> Vec<u8> {
    let mut f = vec![0xFF, 0xD8];
    let seg = |f: &mut Vec<u8>, m: u8, body: &[u8]| {
        f.extend_from_slice(&[0xFF, m]);
        f.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
        f.extend_from_slice(body);
    };
    let (hm, vm) = sp.max();
    let mut sof = vec![sp.precision];
    sof.extend_from_slice(&(sp.h as u16).to_be_bytes());
    sof.extend_from_slice(&(sp.w as u16).to_be_bytes());
    sof.push(sp.comps.len() as u8);
    for c in &sp.comps {
        sof.extend_from_slice(&[c.id, (c.h << 4 | c.v) as u8, 0]);
    }
    seg(&mut f, if arith { 0xCB } else { 0xC3 }, &sof);
    if arith {
        let mut dac = Vec::new();
        for (t, &(l, u)) in sp.dac.iter().enumerate() {
            dac.extend_from_slice(&[t as u8, u << 4 | l]);
        }
        seg(&mut f, 0xCC, &dac);
    } else {
        for t in 0..2u8 {
            let mut dht = vec![t];
            let mut counts = [0u8; 16];
            counts[4] = 17;
            dht.extend_from_slice(&counts);
            dht.extend(0..17u8);
            seg(&mut f, 0xC4, &dht);
        }
    }
    let p = sp.precision as i32;
    let pt = sp.pt as u32;
    for scan in &sp.scans {
        let interleaved = scan.len() > 1;
        let (ux, uy) = if interleaved {
            (sp.w.div_ceil(hm), sp.h.div_ceil(vm))
        } else {
            sp.extent(scan[0])
        };
        let ri = sp.ri_rows * ux;
        seg(&mut f, 0xDD, &(ri as u16).to_be_bytes());
        let mut sos = vec![scan.len() as u8];
        for &k in scan {
            sos.extend_from_slice(&[sp.comps[k].id, (sp.comps[k].t << 4) as u8]);
        }
        sos.extend_from_slice(&[sp.predictor, 0, sp.pt]);
        seg(&mut f, 0xDA, &sos);
        // Per component: categories of the coded differences.
        let mut cats: Vec<Vec<u8>> = scan
            .iter()
            .map(|&k| {
                let (pw, ph) = sp.plane(k);
                vec![0; pw * ph]
            })
            .collect();
        let mut e = Enc::new();
        let mut hb = Bits {
            acc: 0,
            n: 0,
            out: Vec::new(),
        };
        let mut st = [[0u8; 256]; 4];
        let mut first_row = vec![0usize; scan.len()];
        let mut rst = 0u8;
        for row in 0..uy {
            if ri > 0 && row > 0 && (row * ux).is_multiple_of(ri) {
                if arith {
                    f.extend(std::mem::replace(&mut e, Enc::new()).flush());
                } else {
                    f.extend(
                        std::mem::replace(
                            &mut hb,
                            Bits {
                                acc: 0,
                                n: 0,
                                out: Vec::new(),
                            },
                        )
                        .finish(),
                    );
                }
                f.extend_from_slice(&[0xFF, 0xD0 + rst]);
                rst = (rst + 1) & 7;
                st = [[0u8; 256]; 4];
                for (j, &k) in scan.iter().enumerate() {
                    first_row[j] = row * if interleaved { sp.comps[k].v } else { 1 };
                }
            }
            for u in 0..ux {
                for (j, &k) in scan.iter().enumerate() {
                    let c = &sp.comps[k];
                    let (nh, nv) = if interleaved { (c.h, c.v) } else { (1, 1) };
                    let (pw, _) = sp.plane(k);
                    let pl = &planes[k];
                    let r = |i: usize| (pl[i] >> pt) as i32;
                    for by in 0..nv {
                        let y = row * nv + by;
                        let first = y == first_row[j];
                        for bx in 0..nh {
                            let x = u * nh + bx;
                            let i = y * pw + x;
                            let pred = if first {
                                if x == 0 {
                                    1 << (p - pt as i32 - 1)
                                } else {
                                    r(i - 1)
                                }
                            } else if x == 0 {
                                r(i - pw)
                            } else {
                                let (ra, rb, rc) = (r(i - 1), r(i - pw), r(i - pw - 1));
                                match sp.predictor {
                                    1 => ra,
                                    2 => rb,
                                    3 => rc,
                                    4 => ra + rb - rc,
                                    5 => ra + ((rb - rc) >> 1),
                                    6 => rb + ((ra - rc) >> 1),
                                    _ => (ra + rb) >> 1,
                                }
                            };
                            let v = modulo_diff(r(i), pred);
                            let (l, uu) = sp.dac[c.t];
                            if arith {
                                let ca = if x == 0 { 0 } else { cats[j][i - 1] as usize };
                                let cb = if first { 0 } else { cats[j][i - pw] as usize };
                                let x1 = if cb >= 3 { 129 } else { 100 };
                                code_diff(&mut e, &mut st[c.t], 20 * ca + 4 * cb, x1, v);
                                cats[j][i] = category(v, l, uu);
                            } else {
                                let s = if v == 0 {
                                    0
                                } else {
                                    32 - v.unsigned_abs().leading_zeros()
                                };
                                hb.put(s, 5);
                                if s < 16 {
                                    hb.put(if v < 0 { (v - 1) as u32 } else { v as u32 }, s);
                                }
                            }
                        }
                    }
                }
            }
        }
        f.extend(if arith { e.flush() } else { hb.finish() });
    }
    f.extend_from_slice(&[0xFF, 0xD9]);
    f
}

/// Test samples (padded planes): smooth gradients, noise, flat runs and full-range jumps.
fn samples(sp: &Spec, seed: u32) -> Vec<Vec<u16>> {
    let max = (1u32 << sp.precision) - 1;
    let mut x = seed.wrapping_mul(0x9E37_79B9) | 1;
    let mut rnd = move || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    };
    (0..sp.comps.len())
        .map(|k| {
            let (pw, ph) = sp.plane(k);
            let mut v = Vec::with_capacity(pw * ph);
            for y in 0..ph {
                for xx in 0..pw {
                    let r = rnd();
                    let s = match (y / 4 + k) % 5 {
                        0 => ((xx * 37 + y * 11) as u32 * (max / 64 + 1)) & max,
                        1 => r & max,
                        2 => (max / 2).saturating_add(r % 5).min(max).saturating_sub(2),
                        3 => {
                            if r & 1 == 0 {
                                0
                            } else {
                                max
                            }
                        }
                        _ => {
                            if (xx / 3) % 2 == 0 {
                                max / 3
                            } else {
                                r % (max + 1)
                            }
                        }
                    };
                    v.push(s as u16);
                }
            }
            v
        })
        .collect()
}

/// The decoder's full-size 8-bit RGBA output for `planes`, computed directly.
fn expected(sp: &Spec, planes: &[Vec<u16>]) -> Vec<u8> {
    let (hm, vm) = sp.max();
    let p = sp.precision as u32;
    let mut out = Vec::with_capacity(sp.w * sp.h * 4);
    for y in 0..sp.h {
        for x in 0..sp.w {
            let s: Vec<u8> = (0..sp.comps.len())
                .map(|k| {
                    let c = &sp.comps[k];
                    let (pw, _) = sp.plane(k);
                    let raw = planes[k][y * c.v / vm * pw + x * c.h / hm];
                    let v = (raw >> sp.pt) << sp.pt;
                    if p == 8 {
                        v as u8
                    } else {
                        let m = (1u32 << p) - 1;
                        ((v as u32 * 255 + m / 2) / m) as u8
                    }
                })
                .collect();
            match s.len() {
                1 => out.extend_from_slice(&[s[0], s[0], s[0], 255]),
                3 => out.extend_from_slice(&[s[0], s[1], s[2], 255]),
                _ => {
                    let k = 255 - s[3] as u32;
                    let f = |c: u8| (((255 - c as u32) * k + 127) / 255) as u8;
                    out.extend_from_slice(&[f(s[0]), f(s[1]), f(s[2]), 255]);
                }
            }
        }
    }
    out
}

fn opts(scale: Scale, region: Option<Region>) -> DecodeOptions<'static> {
    DecodeOptions {
        scale,
        region,
        cancel: None,
        parallelism: Parallelism::Single,
    }
}

fn comps(layout: &str) -> (Vec<Comp>, Vec<Vec<usize>>) {
    let c = |id, h, v, t| Comp { id, h, v, t };
    match layout {
        "gray" => (vec![c(1, 1, 1, 0)], vec![vec![0]]),
        "rgb" => (
            vec![c(1, 1, 1, 0), c(2, 1, 1, 1), c(3, 1, 1, 1)],
            vec![vec![0, 1, 2]],
        ),
        "rgb_scans" => (
            vec![c(1, 1, 1, 0), c(2, 1, 1, 1), c(3, 1, 1, 0)],
            vec![vec![1], vec![0, 2]],
        ),
        "420" => (
            vec![c(1, 2, 2, 0), c(2, 1, 1, 1), c(3, 1, 1, 1)],
            vec![vec![0, 1, 2]],
        ),
        "421_scans" => (
            vec![c(1, 2, 1, 0), c(2, 1, 1, 1), c(3, 1, 2, 1)],
            vec![vec![0], vec![1, 2]],
        ),
        _ => (
            vec![c(1, 1, 1, 0), c(2, 1, 1, 1), c(3, 1, 1, 0), c(4, 1, 1, 1)],
            vec![vec![0, 1, 2, 3]],
        ),
    }
}

fn check(sp: &Spec, seed: u32) {
    let planes = samples(sp, seed);
    let a = encode(sp, &planes, true);
    let what = format!(
        "P{} {}x{} {}c scans {:?} pred {} pt {} ri {} dac {:?}",
        sp.precision,
        sp.w,
        sp.h,
        sp.comps.len(),
        sp.scans,
        sp.predictor,
        sp.pt,
        sp.ri_rows,
        sp.dac
    );
    let full = decode(&a, &opts(Scale::Full, None)).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(!full.truncated, "{what}: flagged truncated");
    assert_eq!((full.width, full.height), (sp.w as u32, sp.h as u32));
    let want = expected(sp, &planes);
    let got = &full.planes[0];
    for y in 0..sp.h {
        let row = &got.data[y * got.stride..][..sp.w * 4];
        if row != &want[y * sp.w * 4..][..sp.w * 4] {
            panic!("{what}: row {y} differs from the source samples");
        }
    }
    // Without EOI the last scan's data runs to the end of the file: still complete.
    let open = decode(&a[..a.len() - 2], &opts(Scale::Full, None)).unwrap();
    assert!(open.planes == full.planes, "{what}: differs without EOI");
    // The SOF3 twin through jpeg-decoder, where that decodes correctly.
    // (`jpeg-decoder` also rejects lossless files with more than one scan.)
    let simple = sp.pt == 0
        && sp.ri_rows == 0
        && sp.scans.len() == 1
        && sp.comps.iter().all(|c| c.h == 1 && c.v == 1);
    let scales = [Scale::Full, Scale::Half, Scale::Eighth];
    if simple {
        let hf = encode(sp, &planes, false);
        for scale in scales {
            let x = decode(&a, &opts(scale, None)).unwrap();
            let y = decode(&hf, &opts(scale, None)).unwrap_or_else(|e| panic!("{what}: twin {e}"));
            assert!(x == y, "{what} {scale:?}: differs from the SOF3 twin");
        }
    }
    // Region decodes (each scan stops below the region) equal the crop of the full decode.
    for scale in scales {
        let whole = decode(&a, &opts(scale, None)).unwrap();
        let r = Region {
            x: whole.width / 3,
            y: whole.height / 4,
            w: (whole.width / 2).max(1),
            h: (whole.height / 3).max(1),
        };
        let d = decode(&a, &opts(scale, Some(r))).unwrap();
        crop_equal(&whole, &d, &what);
    }
}

/// [`check`], with one `ORACLE<TAB>JPEG/SOF11<TAB>case<TAB>pass|fail` line per case for
/// tools/ci/oracle-pass-rates.sh (the source samples and the SOF3 twin are the oracles).
fn check_case(sp: &Spec, seed: u32) {
    let case = format!(
        "P{} {}x{} {}c {}scan pred {} pt {} ri {} seed {seed}",
        sp.precision,
        sp.w,
        sp.h,
        sp.comps.len(),
        sp.scans.len(),
        sp.predictor,
        sp.pt,
        sp.ri_rows
    );
    let r = std::panic::catch_unwind(|| check(sp, seed));
    let status = if r.is_ok() { "pass" } else { "fail" };
    println!("ORACLE\tJPEG/SOF11\t{case}\t{status}");
    if let Err(e) = r {
        std::panic::resume_unwind(e);
    }
}

fn crop_equal(full: &DecodedPlanes, reg: &DecodedPlanes, what: &str) {
    let (f, r) = (&full.planes[0], &reg.planes[0]);
    let (ox, oy) = (reg.origin.0 as usize, reg.origin.1 as usize);
    for y in 0..r.height as usize {
        let a = &f.data[(oy + y) * f.stride + ox * 4..][..r.width as usize * 4];
        let b = &r.data[y * r.stride..][..r.width as usize * 4];
        assert_eq!(a, b, "{what}: region row {y}");
    }
}

#[test]
fn lossless_arith_matrix() {
    let mut seed = 1;
    for layout in ["gray", "rgb", "rgb_scans", "420", "421_scans", "cmyk"] {
        let (cs, scans) = comps(layout);
        for &(w, h) in &[(1, 1), (17, 9), (40, 23), (3, 70)] {
            for precision in [2u8, 7, 8, 12, 16] {
                for predictor in 1..=7u8 {
                    seed += 1;
                    let pt = match seed % 3 {
                        0 => 0,
                        1 => 1.min(precision - 1),
                        _ => (precision / 2).min(precision - 1),
                    };
                    let ri_rows = [0, 0, 1, 3][(seed / 3 % 4) as usize];
                    let dac = [[(0, 1), (0, 1)], [(2, 5), (0, 0)], [(1, 1), (4, 9)]]
                        [(seed / 5 % 3) as usize];
                    let sp = Spec {
                        precision,
                        w,
                        h,
                        comps: cs.clone(),
                        scans: scans.clone(),
                        predictor,
                        pt,
                        ri_rows,
                        dac,
                    };
                    check_case(&sp, seed);
                    // The twin comparison needs pt 0 and no restarts: run that variant too.
                    if pt != 0 || ri_rows != 0 {
                        check_case(
                            &Spec {
                                pt: 0,
                                ri_rows: 0,
                                ..sp
                            },
                            seed,
                        );
                    }
                }
            }
        }
    }
}

/// Differences of exactly 32768 (category 16) and the largest magnitudes at 16 bits.
#[test]
fn lossless_arith_extremes() {
    for predictor in 1..=7 {
        for dac in [(0, 1), (0, 15), (15, 15)] {
            let (cs, scans) = comps("gray");
            let sp = Spec {
                precision: 16,
                w: 33,
                h: 7,
                comps: cs,
                scans,
                predictor,
                pt: 0,
                ri_rows: 0,
                dac: [dac, dac],
            };
            let planes = vec![
                (0..33 * 7)
                    .map(|i| match i % 4 {
                        0 => 0,
                        1 => 32768,
                        2 => 65535,
                        _ => (i * 7919 % 65536) as u16,
                    })
                    .collect::<Vec<u16>>(),
            ];
            let a = encode(&sp, &planes, true);
            let d = decode(&a, &opts(Scale::Full, None)).unwrap();
            assert!(!d.truncated);
            let want = expected(&sp, &planes);
            for y in 0..7 {
                assert_eq!(
                    &d.planes[0].data[y * d.planes[0].stride..][..33 * 4],
                    &want[y * 33 * 4..][..33 * 4],
                    "pred {predictor} dac {dac:?} row {y}"
                );
            }
            let hf = encode(&sp, &planes, false);
            assert!(d == decode(&hf, &opts(Scale::Full, None)).unwrap());
        }
    }
}

/// Cut and corrupted SOF11 files: no panic, a cut file is flagged, a bad predictor or point
/// transform is an error.
#[test]
fn lossless_arith_cut_and_corrupt() {
    let (cs, scans) = comps("rgb");
    let sp = Spec {
        precision: 12,
        w: 61,
        h: 47,
        comps: cs,
        scans,
        predictor: 4,
        pt: 0,
        ri_rows: 0,
        dac: [(0, 1), (2, 4)],
    };
    let planes = samples(&sp, 9);
    let a = encode(&sp, &planes, true);
    let sos = a.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
    let hdr = sos + 2 + u16::from_be_bytes([a[sos + 2], a[sos + 3]]) as usize;
    // A cut within the last few bytes, or after the first few, can leave a valid code stream
    // (its zero fill decodes to the end at a fraction of a bit per sample): not flagged.
    for cut in (hdr + 8..a.len() - 2 - 8).step_by(7) {
        let d = decode(&a[..cut], &opts(Scale::Full, None)).unwrap();
        assert!(d.truncated, "cut {cut} not flagged");
    }
    let mut x = 0x1234_5678u32;
    for _ in 0..200 {
        let mut bad = a.clone();
        for _ in 0..3 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let at = hdr + x as usize % (a.len() - hdr);
            bad[at] = (x >> 9) as u8;
        }
        let _ = decode(&bad, &opts(Scale::Full, None));
        let _ = decode(&bad, &opts(Scale::Quarter, None));
    }
    for (ss_al, what) in [
        ([0u8, 0], "predictor 0"),
        ([8, 0], "predictor 8"),
        ([1, 12], "pt"),
    ] {
        let mut bad = a.clone();
        bad[hdr - 3] = ss_al[0];
        bad[hdr - 1] = ss_al[1];
        assert!(decode(&bad, &opts(Scale::Full, None)).is_err(), "{what}");
    }
}

/// Writes a spread of SOF11 files into `$SPECULAR_SOF11_SEEDS` (fuzzing seeds).
#[test]
#[ignore]
fn lossless_arith_write_seeds() {
    let Some(dir) = std::env::var_os("SPECULAR_SOF11_SEEDS") else {
        return;
    };
    let mut n = 0;
    for layout in ["gray", "rgb_scans", "420", "cmyk"] {
        for (precision, predictor, pt, ri_rows) in [
            (8, 1, 0, 0),
            (12, 4, 2, 1),
            (16, 7, 0, 2),
            (12, 4, 2, 0),
            (12, 4, 0, 1),
            (8, 1, 1, 0),
        ] {
            let (cs, scans) = comps(layout);
            let sp = Spec {
                precision,
                w: 24,
                h: 16,
                comps: cs,
                scans,
                predictor,
                pt,
                ri_rows,
                dac: [(1, 3), (0, 1)],
            };
            let a = encode(&sp, &samples(&sp, n), true);
            std::fs::write(std::path::Path::new(&dir).join(format!("sof11-{n}.jpg")), a).unwrap();
            let a = encode(&sp, &samples(&sp, n), false);
            std::fs::write(std::path::Path::new(&dir).join(format!("sof3-{n}.jpg")), a).unwrap();
            n += 1;
        }
    }
}

#[test]
fn lossless_arith_cancel() {
    let (cs, scans) = comps("gray");
    let sp = Spec {
        precision: 8,
        w: 20,
        h: 20,
        comps: cs,
        scans,
        predictor: 1,
        pt: 0,
        ri_rows: 0,
        dac: [(0, 1), (0, 1)],
    };
    let a = encode(&sp, &samples(&sp, 3), true);
    let stop = std::sync::atomic::AtomicBool::new(true);
    let o = DecodeOptions {
        cancel: Some(&stop),
        ..opts(Scale::Full, None)
    };
    assert!(matches!(decode(&a, &o), Err(crate::JpegError::Cancelled)));
}

// ------------------------------------------------------------------ Huffman lossless (SOF3)
//
// The same encoder writes Huffman twins: the native SOF3 decoder (`crate::lossless`) is checked
// against the source samples (every layout, restart intervals, point transforms), its parallel
// restart-interval path against the serial one, `decode_wide` against the stored values, and
// the display output against `jpeg-decoder` (the oracle) wherever that decodes correctly.

/// `jpeg-decoder`'s output turned into 8-bit RGBA by the rule the old fallback used.
fn oracle_rgba(data: &[u8]) -> Option<Vec<u8>> {
    let mut d = jpeg_decoder::Decoder::new(data);
    let px = d.decode().ok()?;
    let info = d.info()?;
    let npx = info.width as usize * info.height as usize;
    let nc = match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 | jpeg_decoder::PixelFormat::L16 => 1,
        jpeg_decoder::PixelFormat::RGB24 => 3,
        jpeg_decoder::PixelFormat::CMYK32 => 4,
    };
    let bytes = px.len() / (npx * nc);
    let p = crate::read_info(data).ok()?.precision as u32;
    let s = |i: usize| -> u8 {
        if bytes == 1 {
            px[i]
        } else {
            crate::fallback::to_8bit(u16::from_ne_bytes([px[2 * i], px[2 * i + 1]]), p)
        }
    };
    let mut out = Vec::with_capacity(npx * 4);
    for i in 0..npx {
        match nc {
            1 => out.extend_from_slice(&[s(i), s(i), s(i), 255]),
            3 => out.extend_from_slice(&[s(3 * i), s(3 * i + 1), s(3 * i + 2), 255]),
            _ => {
                let k = 255 - s(4 * i + 3) as u32;
                let f = |c: u8| (((255 - c as u32) * k + 127) / 255) as u8;
                out.extend_from_slice(&[f(s(4 * i)), f(s(4 * i + 1)), f(s(4 * i + 2)), 255]);
            }
        }
    }
    Some(out)
}

fn threads(o: DecodeOptions<'static>) -> DecodeOptions<'static> {
    DecodeOptions {
        parallelism: Parallelism::Threads(4),
        ..o
    }
}

/// Returns whether `jpeg-decoder` was compared (and matched).
fn huff_check(sp: &Spec, seed: u32) -> bool {
    let planes = samples(sp, seed);
    let a = encode(sp, &planes, false);
    let what = format!(
        "SOF3 P{} {}x{} {}c scans {:?} pred {} pt {} ri {}",
        sp.precision,
        sp.w,
        sp.h,
        sp.comps.len(),
        sp.scans,
        sp.predictor,
        sp.pt,
        sp.ri_rows,
    );
    let full = decode(&a, &opts(Scale::Full, None)).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(!full.truncated, "{what}: flagged truncated");
    let want = expected(sp, &planes);
    let got = &full.planes[0];
    for y in 0..sp.h {
        if got.data[y * got.stride..][..sp.w * 4] != want[y * sp.w * 4..][..sp.w * 4] {
            panic!("{what}: row {y} differs from the source samples");
        }
    }
    // Stored values.
    let wide = crate::decode_wide(&a, None).unwrap_or_else(|e| panic!("{what}: wide {e}"));
    assert!(!wide.truncated, "{what}: wide flagged truncated");
    for (k, pl) in wide.planes.iter().enumerate() {
        let (ew, eh) = sp.extent(k);
        let (pw, _) = sp.plane(k);
        assert_eq!((pl.width as usize, pl.height as usize), (ew, eh), "{what}");
        for y in 0..eh {
            for x in 0..ew {
                let v = (planes[k][y * pw + x] >> sp.pt) << sp.pt;
                assert_eq!(pl.data[y * ew + x], v, "{what}: wide comp {k} at {x},{y}");
            }
        }
    }
    for scale in [Scale::Full, Scale::Half, Scale::Eighth] {
        let one = decode(&a, &opts(scale, None)).unwrap();
        let par = decode(&a, &threads(opts(scale, None))).unwrap();
        assert!(one == par, "{what} {scale:?}: parallel differs from serial");
        let r = Region {
            x: one.width / 3,
            y: one.height / 4,
            w: (one.width / 2).max(1),
            h: (one.height / 3).max(1),
        };
        let d = decode(&a, &opts(scale, Some(r))).unwrap();
        crop_equal(&one, &d, &what);
        let d = decode(&a, &threads(opts(scale, Some(r)))).unwrap();
        crop_equal(&one, &d, &what);
    }
    // The oracle, where it is right: no point transform, no restarts, one interleaved scan of
    // 1x1 components (it rejects or misdecodes the rest).
    let simple = sp.pt == 0
        && sp.ri_rows == 0
        && sp.scans.len() == 1
        && sp.comps.iter().all(|c| c.h == 1 && c.v == 1);
    if simple {
        let o = oracle_rgba(&a).unwrap_or_else(|| panic!("{what}: jpeg-decoder failed"));
        assert!(o == got.data, "{what}: differs from jpeg-decoder");
    }
    simple
}

#[test]
fn lossless_huffman_matrix() {
    let (mut seed, mut oracle) = (100, 0);
    for layout in ["gray", "rgb", "rgb_scans", "420", "421_scans", "cmyk"] {
        let (cs, scans) = comps(layout);
        for &(w, h) in &[(1, 1), (17, 9), (40, 23), (3, 70), (64, 33)] {
            for precision in [2u8, 7, 8, 12, 16] {
                for predictor in 1..=7u8 {
                    seed += 1;
                    let pt = match seed % 3 {
                        0 => 0,
                        1 => 1.min(precision - 1),
                        _ => (precision / 2).min(precision - 1),
                    };
                    let ri_rows = [0, 1, 2, 5][(seed / 3 % 4) as usize];
                    let sp = Spec {
                        precision,
                        w,
                        h,
                        comps: cs.clone(),
                        scans: scans.clone(),
                        predictor,
                        pt,
                        ri_rows,
                        dac: [(0, 1), (0, 1)],
                    };
                    oracle += huff_check(&sp, seed) as u32;
                    if pt != 0 || ri_rows != 0 {
                        let sp = Spec {
                            pt: 0,
                            ri_rows: 0,
                            ..sp
                        };
                        oracle += huff_check(&sp, seed) as u32;
                    }
                }
            }
        }
    }
    assert!(oracle > 200, "only {oracle} oracle comparisons");
}

/// Differences of exactly 32768 (category 16) and full-range jumps at 16 bits, with restarts.
#[test]
fn lossless_huffman_extremes() {
    for predictor in 1..=7 {
        for ri_rows in [0, 1, 3] {
            let (cs, scans) = comps("gray");
            let sp = Spec {
                precision: 16,
                w: 33,
                h: 7,
                comps: cs,
                scans,
                predictor,
                pt: 0,
                ri_rows,
                dac: [(0, 1), (0, 1)],
            };
            let mut planes = samples(&sp, predictor as u32);
            for (i, v) in planes[0].iter_mut().enumerate() {
                *v = if (i / 3) % 2 == 0 {
                    0
                } else {
                    0x8000 + (i % 2) as u16 * 0x7FFF
                };
            }
            let a = encode(&sp, &planes, false);
            let what = format!("pred {predictor} ri {ri_rows}");
            let d = decode(&a, &opts(Scale::Full, None)).unwrap();
            assert_eq!(d.planes[0].data, expected(&sp, &planes), "{what}");
            let wide = crate::decode_wide(&a, None).unwrap();
            assert_eq!(wide.planes[0].data, planes[0], "{what}");
        }
    }
}

/// Cut and corrupted files never panic, cuts are flagged, and a broken restart marker sends
/// the parallel path back to the serial one (same output).
#[test]
fn lossless_huffman_cut_and_corrupt() {
    for ri_rows in [0, 2] {
        let (cs, scans) = comps("rgb");
        let sp = Spec {
            precision: 12,
            w: 61,
            h: 47,
            comps: cs,
            scans,
            predictor: 4,
            pt: 0,
            ri_rows,
            dac: [(0, 1), (0, 1)],
        };
        let planes = samples(&sp, 21);
        let a = encode(&sp, &planes, false);
        let sos = a.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
        let hdr = sos + 2 + u16::from_be_bytes([a[sos + 2], a[sos + 3]]) as usize;
        for cut in (hdr + 8..a.len() - 2 - 8).step_by(7) {
            let d = decode(&a[..cut], &opts(Scale::Full, None)).unwrap();
            assert!(d.truncated, "ri {ri_rows} cut {cut} not flagged");
            let p = decode(&a[..cut], &threads(opts(Scale::Full, None))).unwrap();
            assert!(p.truncated, "ri {ri_rows} cut {cut} not flagged (parallel)");
            let w = crate::decode_wide(&a[..cut], None).unwrap();
            assert!(w.truncated, "ri {ri_rows} cut {cut} not flagged (wide)");
        }
        let mut x = 0x1234_5678u32;
        for _ in 0..200 {
            let mut bad = a.clone();
            for _ in 0..3 {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                let at = hdr + x as usize % (a.len() - hdr);
                bad[at] = (x >> 9) as u8;
            }
            let _ = decode(&bad, &opts(Scale::Full, None));
            let _ = decode(&bad, &threads(opts(Scale::Quarter, None)));
            let _ = crate::decode_wide(&bad, None);
        }
        if ri_rows > 0 {
            // RST2 renumbered: the parallel split refuses it, the serial reader resyncs.
            let mut bad = a.clone();
            let at = (hdr..bad.len() - 1)
                .find(|&i| bad[i] == 0xFF && bad[i + 1] == 0xD2)
                .unwrap();
            bad[at + 1] = 0xD5;
            let one = decode(&bad, &opts(Scale::Full, None)).unwrap();
            let par = decode(&bad, &threads(opts(Scale::Full, None))).unwrap();
            assert!(one == par, "broken RST: parallel differs from serial");
        }
    }
}

#[test]
fn lossless_huffman_cancel() {
    let (cs, scans) = comps("gray");
    let sp = Spec {
        precision: 8,
        w: 20,
        h: 20,
        comps: cs,
        scans,
        predictor: 1,
        pt: 0,
        ri_rows: 2,
        dac: [(0, 1), (0, 1)],
    };
    let a = encode(&sp, &samples(&sp, 3), false);
    let stop = std::sync::atomic::AtomicBool::new(true);
    for par in [Parallelism::Single, Parallelism::Threads(4)] {
        let o = DecodeOptions {
            cancel: Some(&stop),
            parallelism: par,
            ..opts(Scale::Full, None)
        };
        assert!(matches!(decode(&a, &o), Err(crate::JpegError::Cancelled)));
    }
    assert!(matches!(
        crate::decode_wide(&a, Some(&stop)),
        Err(crate::JpegError::Cancelled)
    ));
}
