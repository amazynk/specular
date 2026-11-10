//! DWAA/DWAB and flattened deep parts against OpenEXR's own output, from the corpus written by
//! `tools/corpus/exr-dwa.sh` (`<conformance>/exr-dwa`; `SPECULAR_CONFORMANCE_DIR` overrides
//! the conformance root). Skipped when the corpus is absent.

#![feature(f16)]

use std::path::{Path, PathBuf};

use specular_exr::{ChannelSelect, DecodeOptions, Pixels, SampleFormat, read_info};

fn corpus() -> Option<PathBuf> {
    let root = std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance")
        });
    let dir = root.join("exr-dwa");
    if dir.is_dir() {
        Some(root)
    } else {
        eprintln!("{} missing; skipped", dir.display());
        None
    }
}

struct OracleChannel {
    name: String,
    /// 0 UINT, 1 HALF, 2 FLOAT.
    ty: u8,
    xs: usize,
    ys: usize,
    px: Vec<f32>,
}

struct Oracle {
    width: usize,
    height: usize,
    chans: Vec<OracleChannel>,
}

fn read_oracle(p: &Path) -> Oracle {
    let d = std::fs::read(p).unwrap();
    assert_eq!(&d[..4], b"XEXR", "{}", p.display());
    let u32_at = |i: usize| u32::from_le_bytes(d[i..i + 4].try_into().unwrap()) as usize;
    let (width, height, n) = (u32_at(4), u32_at(8), u32_at(12));
    let mut i = 16;
    let mut chans = Vec::new();
    for _ in 0..n {
        let len = d[i] as usize;
        let name = String::from_utf8(d[i + 1..i + 1 + len].to_vec()).unwrap();
        i += 1 + len;
        let (ty, xs, ys) = (d[i], d[i + 1] as usize, d[i + 2] as usize);
        i += 3;
        chans.push(OracleChannel {
            name,
            ty,
            xs,
            ys,
            px: Vec::new(),
        });
    }
    for c in &mut chans {
        let n = width.div_ceil(c.xs) * height.div_ceil(c.ys);
        c.px = d[i..i + n * 4]
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        i += n * 4;
    }
    assert_eq!(i, d.len(), "{}", p.display());
    Oracle {
        width,
        height,
        chans,
    }
}

/// One channel of `part`, decoded at full resolution as f32, with the damaged-chunk count and the
/// decoded size.
fn decode_channel(data: &[u8], part: usize, name: &str) -> (Vec<f32>, usize, [usize; 2]) {
    let info = read_info(data).unwrap();
    let opts = DecodeOptions {
        part,
        channels: ChannelSelect::Slots([Some(name.into()), None, None, None]),
        format: SampleFormat::F32,
        ..Default::default()
    };
    let img = specular_exr::decode_with_info(data, &info, &opts).unwrap();
    let Pixels::F32(v) = img.pixels else {
        unreachable!()
    };
    (
        v.chunks_exact(4).map(|p| p[0]).collect(),
        img.damaged_chunks,
        [img.width as usize, img.height as usize],
    )
}

#[derive(Default, Debug)]
struct Diff {
    samples: usize,
    mismatched: usize,
    /// Largest difference in units in the last place of the channel's type.
    max_ulp: u32,
    max_abs: f32,
    /// Largest `|want - have| / max(|want|, |have|)`.
    max_rel: f32,
    /// Samples where one side is finite and the other is not, or the signs differ.
    class: usize,
    /// Finite differences beyond [`REL_TOL`] (plus [`ABS_FLOOR`]).
    out_of_tol: usize,
}

/// Our lossy decode is not bit-identical to OpenEXR's: about one sample in 4000 lands one half
/// ulp apart before the perceptual-to-linear table (the float IDCT's rounding order differs from
/// OpenEXR's SIMD kernels), which the table's slope widens to under 1% after it.
const REL_TOL: f32 = 0.01;
/// Below the smallest normal half the table rounds to zero or a subnormal.
const ABS_FLOOR: f32 = 6.2e-5;
/// At most this fraction of all samples may differ at all.
const MAX_MISMATCH_RATE: f64 = 1e-3;

fn ordered(bits: u32) -> i64 {
    if bits & 0x8000_0000 != 0 {
        -((bits & 0x7fff_ffff) as i64)
    } else {
        bits as i64
    }
}

fn compare(o: &Oracle, c: &OracleChannel, got: &[f32]) -> Diff {
    let mut d = Diff::default();
    let (w, h) = (o.width, o.height);
    let sw = w.div_ceil(c.xs);
    for y in (0..h).step_by(c.ys) {
        for x in (0..w).step_by(c.xs) {
            let want = c.px[(y / c.ys) * sw + x / c.xs];
            let have = got[y * w + x];
            d.samples += 1;
            if want.to_bits() == have.to_bits() || (want.is_nan() && have.is_nan()) {
                continue;
            }
            d.mismatched += 1;
            let ulp = if c.ty == 1 {
                let (a, b) = (want as f16, have as f16);
                let oa = ordered(
                    ((a.to_bits() as u32) & 0x7fff) | ((a.to_bits() as u32 & 0x8000) << 16),
                );
                let ob = ordered(
                    ((b.to_bits() as u32) & 0x7fff) | ((b.to_bits() as u32 & 0x8000) << 16),
                );
                (oa - ob).unsigned_abs() as u32
            } else {
                (ordered(want.to_bits()) - ordered(have.to_bits()))
                    .unsigned_abs()
                    .min(u32::MAX as u64) as u32
            };
            d.max_ulp = d.max_ulp.max(ulp);
            if want.is_finite() != have.is_finite()
                || want.is_nan() != have.is_nan()
                || (want != 0.0
                    && have != 0.0
                    && want.is_sign_negative() != have.is_sign_negative())
            {
                d.class += 1;
                continue;
            }
            let abs = (want - have).abs();
            if abs.is_finite() {
                d.max_abs = d.max_abs.max(abs);
                let mag = want.abs().max(have.abs());
                d.max_rel = d.max_rel.max(abs / mag);
                if abs > REL_TOL * mag + ABS_FLOOR {
                    d.out_of_tol += 1;
                }
            }
        }
    }
    d
}

#[test]
fn dwa_matches_openexr() {
    let Some(root) = corpus() else { return };
    let dir = root.join("exr-dwa/dwa");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "exr"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let mut total = Diff::default();
    for p in &files {
        let data = std::fs::read(p).unwrap();
        let o = read_oracle(&p.with_extension("oracle"));
        for c in &o.chans {
            let (got, damaged, size) = decode_channel(&data, 0, &c.name);
            assert_eq!(damaged, 0, "{} {}", p.display(), c.name);
            assert_eq!(size, [o.width, o.height], "{}", p.display());
            let d = compare(&o, c, &got);
            if d.mismatched > 0 {
                eprintln!(
                    "{} {}: {}/{} differ, max {} ulp, max abs {}, max rel {}",
                    p.file_name().unwrap().to_string_lossy(),
                    c.name,
                    d.mismatched,
                    d.samples,
                    d.max_ulp,
                    d.max_abs,
                    d.max_rel
                );
            }
            total.samples += d.samples;
            total.mismatched += d.mismatched;
            total.max_ulp = total.max_ulp.max(d.max_ulp);
            total.max_abs = total.max_abs.max(d.max_abs);
            total.max_rel = total.max_rel.max(d.max_rel);
            total.class += d.class;
            total.out_of_tol += d.out_of_tol;
        }
    }
    eprintln!("dwa: {} files, {total:?}", files.len());
    assert_eq!((total.class, total.out_of_tol), (0, 0), "{total:?}");
    assert!(
        total.mismatched as f64 <= MAX_MISMATCH_RATE * total.samples as f64,
        "{total:?}"
    );
}

/// Deep parts decode flattened exactly as the generator flattens OpenEXR's samples: the
/// openexr-images/v2 deep files and the synthetic `deep/syn_*.exr` (NONE / RLE / ZIPS,
/// scanline and tiled, offset data window, UINT, no `A`).
#[test]
fn deep_flattened_matches_openexr() {
    let Some(root) = corpus() else { return };
    let dir = root.join("exr-dwa/deep");
    let mut flats: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "flat"))
        .collect();
    flats.sort();
    assert!(!flats.is_empty());
    let mut samples = 0;
    for f in &flats {
        // <stem>.p<part>.flat
        let name = f.file_stem().unwrap().to_str().unwrap();
        let (stem, part) = name.rsplit_once(".p").unwrap();
        let part: usize = part.parse().unwrap();
        let local = dir.join(format!("{stem}.exr"));
        let src = if local.is_file() {
            local
        } else {
            let (d, n) = stem.split_once('_').unwrap();
            root.join(format!("openexr-images/v2/{d}/{n}.exr"))
        };
        let data = std::fs::read(&src).unwrap();
        let o = read_oracle(f);
        for c in &o.chans {
            let (got, damaged, size) = decode_channel(&data, part, &c.name);
            assert_eq!(damaged, 0, "{} {}", src.display(), c.name);
            assert_eq!(size, [o.width, o.height], "{}", src.display());
            let d = compare(&o, c, &got);
            assert_eq!(d.mismatched, 0, "{name} {}: {d:?}", c.name);
            samples += d.samples;
        }
    }
    eprintln!(
        "deep: {} parts, {samples} samples, all identical",
        flats.len()
    );
}
