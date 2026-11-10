//! The codecs against astropy's (cfitsio's) output (`gen/codec_fixtures.py`), plus truncated
//! and corrupted tiles, which must fail cleanly.

use super::*;

const RICE: &str = include_str!("../../tests/data/codec_rice.tsv");
const HCOMPRESS: &str = include_str!("../../tests/data/codec_hcompress.tsv");
const PLIO: &str = include_str!("../../tests/data/codec_plio.tsv");

/// Data rows (after the comment and the column names), split at tabs.
fn rows(tsv: &str) -> impl Iterator<Item = Vec<&str>> {
    tsv.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|l| l.split('\t').collect())
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn values<T: std::str::FromStr>(s: &str) -> Vec<T>
where
    T::Err: std::fmt::Debug,
{
    s.split(',').map(|v| v.parse().unwrap()).collect()
}

/// A small deterministic generator for the corruption tests.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Copies of `data` with one to four random bytes (at `from` or later) changed.
fn mutants(data: &[u8], from: usize, count: usize, seed: u64) -> Vec<Vec<u8>> {
    let mut rng = Rng(seed | 1);
    (0..count)
        .map(|_| {
            let mut d = data.to_vec();
            if d.len() > from {
                for _ in 0..=rng.below(4) {
                    let i = from + rng.below(d.len() - from);
                    d[i] = match rng.below(3) {
                        0 => d[i] ^ (1 << rng.below(8)),
                        1 => rng.next() as u8,
                        _ => [0, 0xFF][rng.below(2)],
                    };
                }
            }
            d
        })
        .collect()
}

#[test]
fn rice_matches_astropy() {
    let mut n = 0;
    for r in rows(RICE) {
        let (bytepix, blocksize) = (r[1].parse().unwrap(), r[2].parse().unwrap());
        let want: Vec<i32> = values(r[4]);
        let mut out = vec![0; want.len()];
        rice(&hex(r[3]), bytepix, blocksize, &mut out).unwrap_or_else(|e| panic!("{}: {e}", r[0]));
        assert_eq!(out, want, "{}", r[0]);
        n += 1;
    }
    assert_eq!(n, 13);
}

#[test]
fn rice_truncated_and_corrupt() {
    for r in rows(RICE) {
        let (bytepix, blocksize) = (r[1].parse().unwrap(), r[2].parse().unwrap());
        let data = hex(r[3]);
        let mut out = vec![0; r[4].split(',').count()];
        // The last byte always holds bits of the last pixel.
        for len in 0..data.len() {
            assert!(
                rice(&data[..len], bytepix, blocksize, &mut out).is_err(),
                "{} cut to {len}",
                r[0]
            );
        }
        for m in mutants(&data, 0, 300, data.len() as u64) {
            let _ = rice(&m, bytepix, blocksize, &mut out);
        }
    }
    let mut out = [0; 4];
    assert!(rice(&[0; 16], 3, 32, &mut out).is_err());
    assert!(rice(&[0; 16], 4, 0, &mut out).is_err());
    // A split code of 31 with 4-byte pixels (only 0..=26 exist).
    assert!(rice(&[0, 0, 0, 0, 0xF8, 0, 0, 0], 4, 32, &mut out).is_err());
    assert_eq!(rice(&[], 2, 32, &mut []), Ok(()));
}

/// A minimal Rice encoder (for the round trip and the speed test): the split that makes the
/// block's unary parts average about one bit.
fn rice_encode<const B: usize>(pixels: &[i32], blocksize: usize) -> Vec<u8> {
    let (fsbits, fsmax, bbits) = match B {
        1 => (3, 6, 8),
        2 => (4, 14, 16),
        _ => (5, 25, 32),
    };
    let mask = u32::MAX >> (32 - 8 * B);
    let mut out: Vec<u8> = (pixels[0] as u32).to_be_bytes()[4 - B..].to_vec();
    let (mut acc, mut nacc) = (0u64, 0u32);
    let mut put = |v: u32, n: u32, out: &mut Vec<u8>| {
        for k in (0..n).rev() {
            acc = acc << 1 | u64::from(v >> k & 1);
            nacc += 1;
            if nacc == 8 {
                out.push(acc as u8);
                (acc, nacc) = (0, 0);
            }
        }
    };
    let mut last = pixels[0] as u32 & mask;
    for block in pixels.chunks(blocksize) {
        let diffs: Vec<u32> = block
            .iter()
            .map(|&p| {
                let p = p as u32 & mask;
                // Sign-extend the wrapped difference to the pixel width, then zig-zag.
                let d = p.wrapping_sub(last) & mask;
                last = p;
                let d = (d << (32 - 8 * B as u32)) as i32 >> (32 - 8 * B as u32);
                ((d << 1) ^ (d >> 31)) as u32
            })
            .collect();
        let sum: u64 = diffs.iter().map(|&d| u64::from(d)).sum();
        if sum == 0 {
            put(0, fsbits, &mut out);
            continue;
        }
        let mean = sum / diffs.len() as u64;
        let fs = 64 - mean.leading_zeros();
        if fs >= fsmax {
            put(fsmax + 1, fsbits, &mut out);
            for &d in &diffs {
                put(d, bbits, &mut out);
            }
        } else {
            put(fs + 1, fsbits, &mut out);
            for &d in &diffs {
                for _ in 0..d >> fs {
                    put(0, 1, &mut out);
                }
                put(1, 1, &mut out);
                put(d & ((1 << fs) - 1), fs, &mut out);
            }
        }
    }
    put(0, 7, &mut out);
    out
}

/// A smooth sky with stars and noise (`w × h`, values fit in `i16`).
fn synthetic(w: usize, h: usize) -> Vec<i32> {
    let mut rng = Rng(0x5eed);
    (0..w * h)
        .map(|k| {
            let (x, y) = ((k % w) as f64, (k / w) as f64);
            let sky = 1000.0 + 200.0 * (x / 300.0).sin() * (y / 500.0).cos();
            let star = if (x as usize % 97 < 3) && (y as usize % 89 < 3) {
                20000.0
            } else {
                0.0
            };
            let noise = (rng.next() % 41) as f64 - 20.0;
            (sky + star + noise) as i32
        })
        .collect()
}

#[test]
fn rice_round_trip() {
    let px = synthetic(300, 7);
    for bs in [16, 32] {
        let mut out = vec![0; px.len()];
        rice(&rice_encode::<2>(&px, bs), 2, bs, &mut out).unwrap();
        assert_eq!(out, px);
        rice(&rice_encode::<4>(&px, bs), 4, bs, &mut out).unwrap();
        assert_eq!(out, px);
        let bytes: Vec<i32> = px.iter().map(|&v| v & 0xFF).collect();
        rice(&rice_encode::<1>(&bytes, bs), 1, bs, &mut out).unwrap();
        assert_eq!(out, bytes);
    }
    // A unary run longer than the 64-bit buffer: split 0, 100 zeros, then the one (d = 100).
    let bits = format!("00001{}1", "0".repeat(100));
    let mut data = vec![0, 0, 0, 7];
    data.extend(bits.as_bytes().chunks(8).map(|c| {
        let c = std::str::from_utf8(c).unwrap();
        u8::from_str_radix(&format!("{c:0<8}"), 2).unwrap()
    }));
    let mut out = [0; 1];
    rice(&data, 4, 1, &mut out).unwrap();
    assert_eq!(out, [7 + 50]);
    assert!(rice(&data[..data.len() - 1], 4, 1, &mut out).is_err());
}

/// `cargo test -p specular-fits --release -- --ignored rice_speed --nocapture`
#[test]
#[ignore = "timing; run in release"]
fn rice_speed() {
    let (w, h) = (4096, 4096);
    let px = synthetic(w, h);
    // One tile per row, as fpack writes them.
    let tiles: Vec<Vec<u8>> = px.chunks(w).map(|r| rice_encode::<2>(r, 32)).collect();
    let bytes: usize = tiles.iter().map(Vec::len).sum();
    let mut out = vec![0; w * h];
    let mut best = f64::MAX;
    for _ in 0..5 {
        let t = std::time::Instant::now();
        for (tile, row) in tiles.iter().zip(out.chunks_mut(w)) {
            rice(tile, 2, 32, row).unwrap();
        }
        best = best.min(t.elapsed().as_secs_f64());
    }
    assert_eq!(out, px);
    println!(
        "Rice i16 {w}x{h}: {:.1} ms, {:.0} Mpix/s, {:.0} MB/s compressed ({:.2} bits/pixel), one thread",
        best * 1e3,
        (w * h) as f64 / best / 1e6,
        bytes as f64 / best / 1e6,
        bytes as f64 * 8.0 / (w * h) as f64
    );
}

#[test]
fn hcompress_matches_astropy() {
    let mut n = 0;
    for r in rows(HCOMPRESS) {
        let (nx, ny): (usize, usize) = (r[1].parse().unwrap(), r[2].parse().unwrap());
        let (scale, smooth): (i32, bool) = (r[3].parse().unwrap(), r[4] == "1");
        let want: Vec<i64> = values(r[6]);
        let d = hcompress(&hex(r[5]), smooth).unwrap_or_else(|e| panic!("{}: {e}", r[0]));
        assert_eq!((d.nx, d.ny, d.scale), (nx, ny, scale), "{}", r[0]);
        assert_eq!(d.data, want, "{}", r[0]);
        n += 1;
    }
    assert_eq!(n, 14);
}

#[test]
fn hcompress_smoothing_changes_lossy_output() {
    let r = rows(HCOMPRESS)
        .find(|r| r[0] == "star_64x40_scale16")
        .unwrap();
    let data = hex(r[5]);
    assert_ne!(
        hcompress(&data, true).unwrap().data,
        hcompress(&data, false).unwrap().data
    );
}

#[test]
fn hcompress_truncated_and_corrupt() {
    for r in rows(HCOMPRESS) {
        let data = hex(r[5]);
        let mut errors = 0;
        for len in 0..data.len() {
            errors += usize::from(hcompress(&data[..len], false).is_err());
        }
        // Only the trailing sign bits may be cut without an error being noticed.
        assert!(
            errors + 2 >= data.len(),
            "{}: {errors} of {}",
            r[0],
            data.len()
        );
        // Dimensions stay as given (they bound the work); everything after them is fair game.
        for m in mutants(&data, 10, 300, data.len() as u64) {
            let _ = hcompress(&m, true);
        }
    }
    let tile =
        |dims: &str, planes: &str| hex(&format!("dd99{dims}00000000{}{planes}00", "0".repeat(16)));
    let mut ok = tile("0000000400000004", "000000");
    assert!(hcompress(&ok, false).is_ok());
    ok[0] = 0xDE;
    assert!(hcompress(&ok, false).is_err());
    for dims in ["0000000000000004", "ffffffff00000004", "0000400100004000"] {
        assert!(hcompress(&tile(dims, "000000"), false).is_err(), "{dims}");
    }
    assert!(hcompress(&tile("0000000400000004", "000040"), false).is_err());
}

#[test]
fn plio_matches_astropy() {
    let mut n = 0;
    for r in rows(PLIO) {
        let npix: usize = r[1].parse().unwrap();
        let want: Vec<i32> = values(r[3]);
        let mut out = vec![-1; npix];
        plio(&hex(r[2]), npix, &mut out).unwrap_or_else(|e| panic!("{}: {e}", r[0]));
        assert_eq!(out, want, "{}", r[0]);
        n += 1;
    }
    assert_eq!(n, 7);
}

#[test]
fn plio_truncated_and_corrupt() {
    for r in rows(PLIO) {
        let npix: usize = r[1].parse().unwrap();
        let data = hex(r[2]);
        let mut out = vec![0; npix];
        for len in 0..data.len() - 1 {
            assert!(
                plio(&data[..len], npix, &mut out).is_err(),
                "{} cut to {len}",
                r[0]
            );
        }
        for m in mutants(&data, 0, 300, data.len() as u64) {
            let _ = plio(&m, npix, &mut out);
        }
    }
    let mut out = [0; 3];
    assert!(plio(&hex("00000007ff9c000e0000000000002004"), 4, &mut out).is_err());
    // Old-format header (length in word 2, instructions from word 3): 1 zero, 2 of value 1.
    plio(&hex("0000000000054001"), 3, &mut out).unwrap_err();
    plio(&hex("000000000005000140020000"), 3, &mut out).unwrap();
    assert_eq!(out, [0, 1, 1]);
    // Opcode 8 and above do not exist.
    assert!(plio(&hex("00000007ff9c0008000000000000800100"), 3, &mut out).is_err());
}
