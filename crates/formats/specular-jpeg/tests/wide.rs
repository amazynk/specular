//! Full-precision decoding (`decode_wide`) of the processes DICOM's retired transfer syntaxes
//! use: progressive (spectral selection, successive approximation), arithmetic and
//! hierarchical frames at 8 and 12 bits.
//!
//! * The coefficients of the 12-bit arithmetic fixtures are re-encoded here as sequential and
//!   progressive Huffman streams (a small test encoder below); every stream holding the same
//!   coefficients must decode to the same samples.
//! * The hierarchical fixtures come with the ISO reference libjpeg's decode (`tests/fixtures/
//!   make-hier.sh`); the stored values must match it as closely as the 8-bit path does.
//! * `tools/corpus/jpeg-wide-oracle.py` decodes the fixtures and the streams written by
//!   `write_oracle_streams` with pylibjpeg-libjpeg (the reference libjpeg); the oracle test
//!   compares them when that output exists.

use std::path::{Path, PathBuf};

use specular_jpeg::*;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture(name: &str) -> Vec<u8> {
    let p = fixtures().join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Natural-order index of zigzag position `k`.
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

#[derive(Default)]
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
            let b = (self.acc >> (self.n - 8)) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0);
            }
            self.n -= 8;
        }
    }
    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            let pad = 8 - self.n;
            self.put((1 << pad) - 1, pad);
        }
        self.out
    }
}

/// A Huffman table of fixed-length codes over `symbols` (length 8: at most 255 symbols).
struct Table {
    id: u8,
    class: u8,
    symbols: Vec<u8>,
}

impl Table {
    fn dc(id: u8) -> Table {
        Table {
            id,
            class: 0,
            symbols: (0..=15).collect(),
        }
    }
    /// Every run/size pair for sizes 1..=14, EOB and ZRL.
    fn ac(id: u8) -> Table {
        let mut symbols = vec![0x00, 0xF0];
        for r in 0..16u8 {
            for s in 1..=14u8 {
                symbols.push(r << 4 | s);
            }
        }
        Table {
            id,
            class: 1,
            symbols,
        }
    }
    fn emit(&self, b: &mut Bits, sym: u8) {
        let i = self.symbols.iter().position(|&s| s == sym).unwrap();
        b.put(i as u32, 8);
    }
    fn segment(&self, o: &mut Vec<u8>) {
        let mut counts = [0u8; 16];
        counts[7] = self.symbols.len() as u8;
        o.extend([0xFF, 0xC4]);
        o.extend(((2 + 1 + 16 + self.symbols.len()) as u16).to_be_bytes());
        o.push(self.class << 4 | self.id);
        o.extend(counts);
        o.extend(&self.symbols);
    }
}

fn magnitude(v: i32) -> (u32, u32) {
    let s = 32 - v.unsigned_abs().leading_zeros();
    let bits = if v < 0 { v - 1 } else { v } as u32;
    (s, bits)
}

/// One scan of a test script: components, spectral band, successive approximation.
#[derive(Clone, Copy)]
struct ScanDef {
    comps: &'static [usize],
    ss: usize,
    se: usize,
    ah: u32,
    al: u32,
}

const fn sd(comps: &'static [usize], ss: usize, se: usize, ah: u32, al: u32) -> ScanDef {
    ScanDef {
        comps,
        ss,
        se,
        ah,
        al,
    }
}

/// Spectral selection only (DICOM .53/.54 style).
const SPECTRAL: &[ScanDef] = &[
    sd(&[0, 1, 2], 0, 0, 0, 0),
    sd(&[0], 1, 5, 0, 0),
    sd(&[1], 1, 63, 0, 0),
    sd(&[2], 1, 63, 0, 0),
    sd(&[0], 6, 63, 0, 0),
];

/// Full progression (spectral selection and successive approximation, .55/.56 style).
const FULL: &[ScanDef] = &[
    sd(&[0, 1, 2], 0, 0, 0, 1),
    sd(&[0], 1, 5, 0, 2),
    sd(&[2], 1, 63, 0, 1),
    sd(&[1], 1, 63, 0, 1),
    sd(&[0], 6, 63, 0, 2),
    sd(&[0], 1, 63, 2, 1),
    sd(&[0, 1, 2], 0, 0, 1, 0),
    sd(&[2], 1, 63, 1, 0),
    sd(&[1], 1, 63, 1, 0),
    sd(&[0], 1, 63, 1, 0),
];

const GREY_FULL: &[ScanDef] = &[
    sd(&[0], 0, 0, 0, 2),
    sd(&[0], 1, 9, 0, 1),
    sd(&[0], 10, 63, 0, 1),
    sd(&[0], 0, 0, 2, 1),
    sd(&[0], 1, 63, 1, 0),
    sd(&[0], 0, 0, 1, 0),
];

/// Writes `comps` (coefficients as read by `read_coefficients`) as a Huffman JPEG of
/// `precision` bits: sequential (SOF1, one interleaved scan) or progressive (SOF2, `script`).
fn encode(
    w: u32,
    h: u32,
    precision: u8,
    comps: &[ComponentCoefficients],
    script: Option<&[ScanDef]>,
) -> Vec<u8> {
    let mut o = vec![0xFF, 0xD8];
    for (i, c) in comps.iter().enumerate() {
        // 16-bit table entries only where 8 bits cannot hold them (T.81 B.2.4.1).
        let wide = precision > 8 || c.quant.iter().any(|&q| q > 255);
        o.extend([0xFF, 0xDB]);
        o.extend((2u16 + 1 + if wide { 128 } else { 64 }).to_be_bytes());
        o.push(if wide { 0x10 } else { 0 } | i as u8);
        for &z in &ZIGZAG {
            if wide {
                o.extend(c.quant[z].to_be_bytes());
            } else {
                o.push(c.quant[z] as u8);
            }
        }
    }
    o.extend([0xFF, if script.is_some() { 0xC2 } else { 0xC1 }]);
    o.extend((8 + 3 * comps.len() as u16).to_be_bytes());
    o.push(precision);
    o.extend((h as u16).to_be_bytes());
    o.extend((w as u16).to_be_bytes());
    o.push(comps.len() as u8);
    for (i, c) in comps.iter().enumerate() {
        o.extend([c.id, c.h_samp << 4 | c.v_samp, i as u8]);
    }
    let (dc, ac) = (Table::dc(0), Table::ac(0));
    dc.segment(&mut o);
    ac.segment(&mut o);
    let hmax = comps.iter().map(|c| c.h_samp as u32).max().unwrap();
    let vmax = comps.iter().map(|c| c.v_samp as u32).max().unwrap();
    let (mx, my) = (w.div_ceil(8 * hmax), h.div_ceil(8 * vmax));
    let all: Vec<usize> = (0..comps.len()).collect();
    let seq = [ScanDef {
        comps: Box::leak(all.into_boxed_slice()),
        ss: 0,
        se: 63,
        ah: 0,
        al: 0,
    }];
    let script = script.unwrap_or(&seq);
    for s in script {
        o.extend([0xFF, 0xDA]);
        o.extend((6 + 2 * s.comps.len() as u16).to_be_bytes());
        o.push(s.comps.len() as u8);
        for &ci in s.comps {
            o.extend([comps[ci].id, 0x00]);
        }
        o.extend([s.ss as u8, s.se as u8, (s.ah << 4 | s.al) as u8]);
        let mut b = Bits::default();
        let mut preds = vec![0i32; comps.len()];
        let block = |b: &mut Bits, preds: &mut [i32], ci: usize, bx: u32, by: u32| {
            let c = &comps[ci];
            let i = (by * c.stride_blocks + bx) as usize * 64;
            let blk = &c.coefs[i..i + 64];
            if s.ss == 0 {
                let v = (blk[0] as i32) >> s.al;
                if s.ah == 0 {
                    let (n, bits) = magnitude(v - preds[ci]);
                    preds[ci] = v;
                    dc.emit(b, n as u8);
                    b.put(bits, n);
                } else {
                    b.put(v as u32 & 1, 1);
                }
            }
            let (ss, se) = (s.ss.max(1), s.se);
            if ss > se {
                return;
            }
            let val = |k: usize| -> i32 {
                let c = blk[ZIGZAG[k]] as i32;
                if c < 0 { -((-c) >> s.al) } else { c >> s.al }
            };
            if s.ah == 0 {
                let mut r = 0u32;
                for k in ss..=se {
                    let v = val(k);
                    if v == 0 {
                        r += 1;
                        continue;
                    }
                    while r > 15 {
                        ac.emit(b, 0xF0);
                        r -= 16;
                    }
                    let (n, bits) = magnitude(v);
                    ac.emit(b, (r << 4 | n) as u8);
                    b.put(bits, n);
                    r = 0;
                }
                if r > 0 {
                    ac.emit(b, 0x00);
                }
            } else {
                let eob = (ss..=se).rev().find(|&k| val(k).abs() == 1).unwrap_or(0);
                let mut r = 0u32;
                let mut corr: Vec<u32> = Vec::new();
                for k in ss..=se {
                    let a = val(k).abs();
                    if a == 0 {
                        r += 1;
                        continue;
                    }
                    while r > 15 && k <= eob {
                        ac.emit(b, 0xF0);
                        for &c in &corr {
                            b.put(c, 1);
                        }
                        corr.clear();
                        r -= 16;
                    }
                    if a > 1 {
                        corr.push(a as u32 & 1);
                        continue;
                    }
                    ac.emit(b, (r << 4 | 1) as u8);
                    b.put((val(k) > 0) as u32, 1);
                    for &c in &corr {
                        b.put(c, 1);
                    }
                    corr.clear();
                    r = 0;
                }
                if r > 0 || !corr.is_empty() {
                    ac.emit(b, 0x00);
                    for &c in &corr {
                        b.put(c, 1);
                    }
                }
            }
        };
        if s.comps.len() == 1 {
            let c = &comps[s.comps[0]];
            for by in 0..c.blocks_h {
                for bx in 0..c.blocks_w {
                    block(&mut b, &mut preds, s.comps[0], bx, by);
                }
            }
        } else {
            for y in 0..my {
                for x in 0..mx {
                    for &ci in s.comps {
                        let c = &comps[ci];
                        for v in 0..c.v_samp as u32 {
                            for hh in 0..c.h_samp as u32 {
                                let (bx, by) = (x * c.h_samp as u32 + hh, y * c.v_samp as u32 + v);
                                block(&mut b, &mut preds, ci, bx, by);
                            }
                        }
                    }
                }
            }
        }
        o.extend(b.finish());
    }
    o.extend([0xFF, 0xD9]);
    o
}

/// Greyscale `comps` keeps only the first component.
fn grey(mut comps: Vec<ComponentCoefficients>) -> Vec<ComponentCoefficients> {
    comps.truncate(1);
    comps[0].h_samp = 1;
    comps[0].v_samp = 1;
    comps
}

/// (name, stream) of every generated stream: the 12-bit fixtures' coefficients as sequential
/// and progressive Huffman, colour and grey, plus the source fixtures themselves.
fn streams() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for src in ["dac12_444_prog", "dac12_420_rst", "arith_dac_444_prog_rst"] {
        let jpg = fixture(&format!("{src}.jpg"));
        let precision = read_info(&jpg).unwrap().precision;
        let (w, h, comps) = read_coefficients(&jpg).unwrap();
        out.push((src.to_string(), jpg));
        out.push((format!("{src}-seq"), encode(w, h, precision, &comps, None)));
        out.push((
            format!("{src}-spectral"),
            encode(w, h, precision, &comps, Some(SPECTRAL)),
        ));
        out.push((
            format!("{src}-full"),
            encode(w, h, precision, &comps, Some(FULL)),
        ));
        let g = grey(comps);
        out.push((
            format!("{src}-grey-full"),
            encode(w, h, precision, &g, Some(GREY_FULL)),
        ));
    }
    out
}

#[test]
fn every_coding_of_the_same_coefficients_gives_the_same_samples() {
    let all = streams();
    for group in all.chunks(5) {
        let base = decode_wide(&group[0].1, None).unwrap();
        assert!(!base.truncated);
        for (name, s) in &group[1..4] {
            let img = decode_wide(s, None).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(img.precision, base.precision, "{name}");
            assert!(!img.truncated, "{name}");
            assert_eq!(img.planes.len(), base.planes.len(), "{name}");
            for (k, (a, b)) in img.planes.iter().zip(&base.planes).enumerate() {
                assert_eq!((a.width, a.height), (b.width, b.height), "{name} comp {k}");
                let bad = a.data.iter().zip(&b.data).filter(|(x, y)| x != y).count();
                assert_eq!(bad, 0, "{name} comp {k}: {bad} samples differ");
            }
        }
        let (name, s) = &group[4];
        let img = decode_wide(s, None).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(img.planes.len(), 1);
        // A lone component is its own MCU: its plane covers the frame at 1x1.
        let b = &base.planes[0];
        let a = &img.planes[0];
        assert_eq!((a.width, a.height), (base.width, base.height), "{name}");
        if (b.width, b.height) == (a.width, a.height) {
            assert_eq!(a.data, b.data, "{name}");
        }
    }
}

#[test]
fn progressive_12_bit_keeps_the_stored_values() {
    let jpg = fixture("dac12_444_prog.jpg");
    let img = decode_wide(&jpg, None).unwrap();
    assert_eq!(img.precision, 12);
    assert!(!img.lossless);
    let max = img.planes.iter().flat_map(|p| &p.data).max().copied();
    // Values above 255 survive: nothing was reduced to 8 bits.
    assert!(max.unwrap() > 255, "{max:?}");
}

/// (width, height, channels, samples) of a binary PGM/PPM, 8 or 16 bits.
fn pnm(b: &[u8]) -> (usize, usize, usize, Vec<u16>) {
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while b[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !b[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&b[s..i]).to_string());
    }
    i += 1;
    let ch = if fields[0] == "P6" { 3 } else { 1 };
    let (w, h, max): (usize, usize, u32) = (
        fields[1].parse().unwrap(),
        fields[2].parse().unwrap(),
        fields[3].parse().unwrap(),
    );
    let n = w * h * ch;
    let data = if max > 255 {
        b[i..i + 2 * n]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect()
    } else {
        b[i..i + n].iter().map(|&v| v as u16).collect()
    };
    (w, h, ch, data)
}

/// Hierarchical greyscale fixtures (and the colour one coded without a colour transform):
/// (name, largest difference, mismatches allowed per mille). The reference tool's integer
/// IDCT rounds a few samples differently from ours (exact, see `src/hier.rs`), more of them at
/// 12 bits (where the 8-bit display path hides the last bits), and a lossless
/// differential frame over a DCT frame (`grey_y0_lossless`) passes that on.
const HIER: &[(&str, u16, usize)] = &[
    ("grey_y2", 1, 30),
    ("grey_y3_arith", 1, 30),
    ("grey_y2_prog", 1, 35),
    ("grey_y0_lossless", 1, 15),
    ("grey12_y2", 1, 300),
    ("rgb_y2_nocolour", 1, 25),
];

#[test]
fn hierarchical_stored_values_match_reference_libjpeg() {
    for &(name, max_diff, per_mille) in HIER {
        let jpg = fixture(&format!("hier/{name}.jpg"));
        let ext = if name.starts_with("rgb") {
            "ppm"
        } else {
            "pgm"
        };
        let (w, h, ch, want) = pnm(&fixture(&format!("hier/{name}.{ext}")));
        let img = decode_wide(&jpg, None).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((img.width, img.height), (w as u32, h as u32), "{name}");
        assert_eq!(img.planes.len(), ch, "{name}");
        let mut bad = 0usize;
        let mut worst = 0u16;
        for (k, p) in img.planes.iter().enumerate() {
            assert_eq!((p.width, p.height), (w as u32, h as u32), "{name}");
            for (i, &v) in p.data.iter().enumerate() {
                let d = v.abs_diff(want[i * ch + k]);
                bad += (d != 0) as usize;
                worst = worst.max(d);
            }
        }
        eprintln!(
            "{name}: {bad} of {} samples differ, by at most {worst}",
            want.len()
        );
        assert!(worst <= max_diff, "{name}: off by {worst}");
        assert!(bad * 1000 <= want.len() * per_mille, "{name}: {bad} differ");
    }
}

#[test]
fn damaged_progressive_and_hierarchical_never_panic() {
    let mut cases: Vec<Vec<u8>> = streams().into_iter().map(|(_, s)| s).collect();
    for &(name, _, _) in HIER {
        cases.push(fixture(&format!("hier/{name}.jpg")));
    }
    for jpg in &cases {
        for cut in [jpg.len() / 3, jpg.len() / 2, jpg.len() - 40] {
            let _ = decode_wide(&jpg[..cut], None);
        }
        for i in 0..60usize {
            let mut bad = jpg.clone();
            let at = (i * 7919) % bad.len();
            bad[at] ^= 1 << (i % 8);
            let _ = decode_wide(&bad, None);
        }
    }
}

fn oracle_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/out/jpeg-wide")
}

/// Writes the generated streams for the oracle script:
/// `cargo test -p specular-jpeg --test wide -- --ignored write_oracle_streams`, then
/// `tools/corpus/jpeg-wide-oracle.py tools/corpus/out/jpeg-wide`.
#[test]
#[ignore]
fn write_oracle_streams() {
    let d = oracle_dir();
    std::fs::create_dir_all(&d).unwrap();
    for (name, s) in streams() {
        std::fs::write(d.join(format!("{name}.jpg")), s).unwrap();
    }
    for &(name, _, _) in HIER {
        std::fs::write(
            d.join(format!("hier-{name}.jpg")),
            fixture(&format!("hier/{name}.jpg")),
        )
        .unwrap();
    }
}

/// Every `NAME.jpg` with oracle output (`tools/corpus/jpeg-wide-oracle.py`: interleaved
/// little-endian samples in `NAME.TAG.u16`, `w h ch` in `NAME.TAG.txt`) against our stored
/// values. Components the oracle upsampled are compared at full resolution only.
/// * `turbo` (libjpeg-turbo, the islow IDCT we share): exact.
/// * `ref` (the reference libjpeg, its own integer IDCT): lossless frames exact (SOF11 files
///   from the arithmetic lossless tests' encoder, with their SOF3 twins, go in through
///   `SPECULAR_SOF11_SEEDS`), DCT samples within 2 (12 bit) or 1
///   (8 bit); hierarchical images pass the rounding on through every level.
#[test]
fn matches_reference_libjpeg_oracle() {
    let d = oracle_dir();
    let Ok(rd) = std::fs::read_dir(&d) else {
        eprintln!("no oracle output in {}; skipped", d.display());
        return;
    };
    let mut names: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jpg"))
        .collect();
    names.sort();
    let mut checked = 0;
    let mut fails = Vec::new();
    for p in names {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let bytes = std::fs::read(&p).unwrap();
        let img = decode_wide(&bytes, None).unwrap();
        // A SOF11 file's Huffman twin (same samples) is what libjpeg-turbo can decode.
        let twin = p.with_file_name(name.replace("sof11-", "sof3-") + ".jpg");
        let sources = [(&p, "turbo"), (&p, "ref"), (&twin, "turbo")];
        for (i, (src, tag)) in sources.into_iter().enumerate() {
            if i == 2 && twin == p {
                continue;
            }
            let Some((w, h, ch, want)) = oracle(src, tag) else {
                continue;
            };
            // The reference libjpeg (as pylibjpeg-libjpeg runs it) returns lossless samples
            // coded with a point transform offset; libjpeg-turbo and the SOF3/SOF11 twins
            // agree with each other.
            if tag == "ref" && img.lossless && point_transform(&bytes) != 0 {
                continue;
            }
            let tag = if i == 2 { "turbo (Huffman twin)" } else { tag };
            assert_eq!((img.width as usize, img.height as usize), (w, h), "{name}");
            let (mut bad, mut worst, mut n) = (0usize, 0u16, 0usize);
            for (k, pl) in img.planes.iter().enumerate().take(ch) {
                if (pl.width as usize, pl.height as usize) != (w, h) {
                    continue;
                }
                for (i, &v) in pl.data.iter().enumerate() {
                    let dlt = v.abs_diff(want[i * ch + k]);
                    bad += (dlt != 0) as usize;
                    worst = worst.max(dlt);
                    n += 1;
                }
            }
            eprintln!("{name} vs {tag}: {bad} of {n} samples differ (max {worst})");
            let (max_diff, per_mille) = match (tag, name.starts_with("hier-")) {
                (t, _) if t.starts_with("turbo") => (0, 0),
                _ if img.lossless => (0, 0),
                (_, false) if img.precision > 8 => (2, 100),
                (_, false) => (1, 40),
                (_, true) if img.precision > 8 => (1, 300),
                (_, true) => (1, 35),
            };
            if worst > max_diff || bad * 1000 > n.max(1) * per_mille {
                fails.push(format!("{name} vs {tag}"));
            }
            checked += 1;
        }
    }
    eprintln!("{checked} decodes checked against the oracles");
    assert!(fails.is_empty(), "outside the tolerance: {fails:?}");
}

/// The point transform (Al) of the first scan.
fn point_transform(jpg: &[u8]) -> u8 {
    let i = jpg.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
    let n = jpg[i + 4] as usize;
    jpg[i + 5 + 2 * n + 2] & 15
}

fn oracle(jpg: &Path, tag: &str) -> Option<(usize, usize, usize, Vec<u16>)> {
    let raw = std::fs::read(jpg.with_extension(format!("{tag}.u16"))).ok()?;
    let txt = std::fs::read_to_string(jpg.with_extension(format!("{tag}.txt"))).ok()?;
    let v: Vec<usize> = txt.split_whitespace().map(|s| s.parse().unwrap()).collect();
    let data = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    Some((v[0], v[1], v[2], data))
}
