//! The own decoders against the reference libraries (libbz2, liblzma, libzstd, zlib-rs), which
//! encode the test streams and decode them for comparison: output must be byte-equal. Then
//! hostile input (truncation, corruption, bombs) and, ignored by default, a speed report:
//!
//! ```sh
//! cargo test --release -p specular-archive --lib codec::oracle::speed -- --ignored --nocapture
//! ```

use std::io::{Read, Write};
use std::time::Instant;

use super::{CodecError, Threads, bzip2, inflate64, xz, zstd};

/// Deterministic test data: text-like runs, noise, long repeats and zero runs, `len` bytes.
pub(crate) fn sample(seed: u64, len: usize) -> Vec<u8> {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    let words: [&[u8]; 8] = [
        b"page ",
        b"comic ",
        b"the ",
        b"\n",
        b"<Page Image=\"",
        b"\"/>",
        b"chapter ",
        b"0123 ",
    ];
    let mut v = Vec::with_capacity(len);
    while v.len() < len {
        let r = next();
        match r % 8 {
            0..=3 => {
                for _ in 0..(r >> 8) % 40 {
                    v.extend_from_slice(words[(next() % 8) as usize]);
                }
            }
            4 => {
                for _ in 0..(r >> 8) % 3000 {
                    v.push(next() as u8);
                }
            }
            5 if v.len() > 100 => {
                let back = 1 + (r >> 8) as usize % v.len().min(70_000);
                let n = (r >> 32) as usize % 600;
                let start = v.len() - back;
                for i in 0..n {
                    v.push(v[start + i]);
                }
            }
            6 => v.resize(v.len() + (r >> 8) as usize % 5000, (r >> 40) as u8),
            _ => v.extend_from_slice(&(r as u32).to_le_bytes()),
        }
    }
    v.truncate(len);
    v
}

fn samples() -> Vec<Vec<u8>> {
    let mut v = vec![Vec::new(), vec![7], b"aaaa".to_vec(), vec![0; 5000]];
    for (i, n) in [100, 4096, 70_000, 300_000, 2_000_000]
        .into_iter()
        .enumerate()
    {
        v.push(sample(i as u64 + 1, n));
    }
    v
}

pub(crate) fn bz2(data: &[u8], level: u32) -> Vec<u8> {
    let mut e = ::bzip2::write::BzEncoder::new(Vec::new(), ::bzip2::Compression::new(level));
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

pub(crate) fn xz_easy(data: &[u8], preset: u32, check: xz2::stream::Check) -> Vec<u8> {
    let s = xz2::stream::Stream::new_easy_encoder(preset, check).unwrap();
    let mut e = xz2::write::XzEncoder::new_stream(Vec::new(), s);
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

/// Several blocks (liblzma's threaded encoder, `block` bytes each).
fn xz_blocks(data: &[u8], block: u64) -> Vec<u8> {
    let s = xz2::stream::MtStreamBuilder::new()
        .threads(2)
        .block_size(block)
        .preset(1)
        .check(xz2::stream::Check::Crc64)
        .encoder()
        .unwrap();
    let mut e = xz2::write::XzEncoder::new_stream(Vec::new(), s);
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

/// `None` when this liblzma cannot encode the branch filter: lzma-sys builds its own copy (on
/// Windows always) with only the x86 and SPARC filters, where the system's has them all.
fn xz_filtered(data: &[u8], x86: bool) -> Option<Vec<u8>> {
    let opts = xz2::stream::LzmaOptions::new_preset(1).unwrap();
    let mut f = xz2::stream::Filters::new();
    if x86 {
        f.x86();
    } else {
        f.arm();
    }
    f.lzma2(&opts);
    let s = match xz2::stream::Stream::new_stream_encoder(&f, xz2::stream::Check::Crc32) {
        Ok(s) => s,
        Err(xz2::stream::Error::Options) => return None,
        Err(e) => panic!("{e}"),
    };
    let mut e = xz2::write::XzEncoder::new_stream(Vec::new(), s);
    e.write_all(data).unwrap();
    Some(e.finish().unwrap())
}

pub(crate) fn zst(data: &[u8], level: i32, checksum: bool) -> Vec<u8> {
    let mut e = ::zstd::stream::Encoder::new(Vec::new(), level).unwrap();
    e.include_checksum(checksum).unwrap();
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn raw_deflate(data: &[u8], level: i32) -> Vec<u8> {
    let mut out = vec![0u8; data.len() + data.len() / 8 + 64];
    let cfg = zlib_rs::DeflateConfig {
        level,
        window_bits: -15,
        ..Default::default()
    };
    let (done, rc) = zlib_rs::compress_slice(&mut out, data, cfg);
    assert_eq!(rc, zlib_rs::ReturnCode::Ok);
    let n = done.len();
    out.truncate(n);
    out
}

fn ours_bz2(c: &[u8], limit: usize, threads: usize) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    bzip2::decode(
        c,
        &mut out,
        limit,
        Threads {
            n: threads,
            start: None,
        },
    )?;
    assert!(out.len() <= limit);
    Ok(out)
}

fn ours_xz(c: &[u8], limit: usize, threads: usize) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    xz::decode(
        c,
        &mut out,
        limit,
        1 << 30,
        Threads {
            n: threads,
            start: None,
        },
    )?;
    assert!(out.len() <= limit);
    Ok(out)
}

fn ours_zst(c: &[u8], limit: usize) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    zstd::decode(c, &mut out, limit)?;
    assert!(out.len() <= limit);
    Ok(out)
}

#[test]
fn bzip2_matches_libbz2() {
    for (i, d) in samples().iter().enumerate() {
        for level in [1, 9] {
            let c = bz2(d, level);
            for threads in [1, 4] {
                let got = ours_bz2(&c, usize::MAX, threads);
                assert!(
                    got.as_deref() == Ok(&d[..]),
                    "sample {i} level {level} t{threads}"
                );
            }
        }
    }
    // Concatenated streams (pbzip2, `cat`), then trailing junk.
    let (a, b) = (sample(10, 250_000), sample(11, 1000));
    let mut c = bz2(&a, 1);
    c.extend(bz2(&b, 9));
    let want = [a, b].concat();
    assert_eq!(ours_bz2(&c, usize::MAX, 1).unwrap(), want);
    assert_eq!(ours_bz2(&c, usize::MAX, 3).unwrap(), want);
    c.extend_from_slice(b"junk");
    assert_eq!(ours_bz2(&c, usize::MAX, 2).unwrap(), want);
}

#[test]
fn xz_matches_liblzma() {
    use xz2::stream::Check;
    for (i, d) in samples().iter().enumerate() {
        for (preset, check) in [
            (0, Check::None),
            (1, Check::Crc32),
            (6, Check::Crc64),
            (2, Check::Sha256),
        ] {
            let c = xz_easy(d, preset, check);
            let got = ours_xz(&c, usize::MAX, 1);
            assert!(got.as_deref() == Ok(&d[..]), "sample {i} preset {preset}");
        }
    }
    let d = sample(20, 3_000_000);
    let c = xz_blocks(&d, 256 << 10);
    for threads in [1, 2, 5] {
        assert!(ours_xz(&c, usize::MAX, threads).as_deref() == Ok(&d[..]));
    }
    for x86 in [true, false] {
        let c = match xz_filtered(&d[..500_000], x86) {
            Some(c) => c,
            // Checked where liblzma has the ARM encoder (the system's, as on Linux CI).
            None if !x86 => {
                eprintln!("xz ARM filter not checked: this liblzma cannot encode it");
                continue;
            }
            None => panic!("liblzma cannot encode the x86 filter"),
        };
        assert!(ours_xz(&c, usize::MAX, 1).as_deref() == Ok(&d[..500_000]));
    }
    // Concatenated streams with padding between them, then junk.
    let mut c = xz_easy(&d[..1000], 1, Check::Crc32);
    c.extend_from_slice(&[0; 8]);
    c.extend(xz_easy(&d[1000..5000], 1, Check::Crc64));
    c.extend_from_slice(b"junk");
    assert!(ours_xz(&c, usize::MAX, 1).as_deref() == Ok(&d[..5000]));
}

#[test]
fn zstd_matches_libzstd() {
    for (i, d) in samples().iter().enumerate() {
        for level in [-5, 1, 3, 9, 19] {
            for checksum in [false, true] {
                let c = zst(d, level, checksum);
                let got = ours_zst(&c, usize::MAX);
                assert!(got.as_deref() == Ok(&d[..]), "sample {i} level {level}");
            }
        }
    }
    // Streaming encoder without a content size, a long window, concatenated frames.
    let d = sample(30, 3_000_000);
    let mut e = ::zstd::stream::Encoder::new(Vec::new(), 5).unwrap();
    e.include_contentsize(false).unwrap();
    e.long_distance_matching(true).unwrap();
    e.window_log(24).unwrap();
    for part in d.chunks(77_777) {
        e.write_all(part).unwrap();
        e.flush().unwrap();
    }
    let mut c = e.finish().unwrap();
    assert!(ours_zst(&c, usize::MAX).as_deref() == Ok(&d[..]));
    c.extend(zst(b"tail", 1, true));
    assert_eq!(
        ours_zst(&c, usize::MAX).unwrap(),
        [&d[..], b"tail"].concat()
    );
    // libzstd's own decoder agrees on the data used.
    let mut back = Vec::new();
    ::zstd::stream::Decoder::new(&c[..])
        .unwrap()
        .read_to_end(&mut back)
        .unwrap();
    assert_eq!(back.len(), d.len() + 4);
}

#[test]
fn deflate_matches_zlib_rs() {
    for (i, d) in samples().iter().enumerate() {
        for level in [0, 1, 6, 9] {
            let c = raw_deflate(d, level);
            let mut out = Vec::new();
            let used = inflate64::inflate(&c, false, &mut out, usize::MAX);
            assert_eq!(used, Ok(c.len()), "sample {i} level {level}");
            assert!(out == *d, "sample {i} level {level}");
        }
    }
}

/// Every cut of a stream is an error (never a panic); corrupted bytes never panic and never
/// pass the limit.
type Decode<'a> = &'a dyn Fn(&[u8], usize) -> Result<Vec<u8>, CodecError>;

fn hostile(name: &str, c: &[u8], len: usize, decode: Decode) {
    let step = (c.len() / 300).max(1);
    for cut in (0..c.len())
        .step_by(step)
        .chain(c.len().saturating_sub(16)..c.len())
    {
        assert!(
            decode(&c[..cut], usize::MAX).is_err(),
            "{name}: cut at {cut} of {}",
            c.len()
        );
    }
    let mut s = 0x1234_5678_u64;
    for _ in 0..400 {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let mut bad = c.to_vec();
        let at = (s as usize) % bad.len();
        bad[at] ^= 1 << ((s >> 32) % 8);
        if let Ok(out) = decode(&bad, len + 10) {
            assert!(out.len() <= len + 10, "{name}");
        }
    }
}

#[test]
fn hostile_streams() {
    let d = sample(40, 400_000);
    let n = d.len();
    hostile("bzip2", &bz2(&d, 9), n, &|c, l| ours_bz2(c, l, 1));
    hostile("bzip2 mt", &bz2(&d, 1), n, &|c, l| ours_bz2(c, l, 3));
    hostile(
        "xz",
        &xz_easy(&d, 6, xz2::stream::Check::Crc64),
        n,
        &|c, l| ours_xz(c, l, 1),
    );
    hostile("xz blocks", &xz_blocks(&d, 64 << 10), n, &|c, l| {
        ours_xz(c, l, 3)
    });
    hostile("zstd", &zst(&d, 3, true), n, &|c, l| ours_zst(c, l));
    hostile("zstd 19", &zst(&d, 19, false), n, &|c, l| ours_zst(c, l));
}

#[test]
fn checksums_are_checked() {
    let d = sample(41, 50_000);
    // The stored checks are the last bytes before the stream footer / at the end.
    let mut c = bz2(&d, 9);
    let n = c.len();
    c[n - 2] ^= 0x10; // combined CRC
    assert_eq!(ours_bz2(&c, usize::MAX, 1), Err(CodecError::Checksum));
    let mut c = zst(&d, 3, true);
    let n = c.len();
    c[n - 1] ^= 1;
    assert_eq!(ours_zst(&c, usize::MAX), Err(CodecError::Checksum));
    for check in [
        xz2::stream::Check::Crc32,
        xz2::stream::Check::Crc64,
        xz2::stream::Check::Sha256,
    ] {
        let mut c = xz_easy(&d, 1, check);
        // Block check: just before the index (index of one record: 8..12 bytes + footer 12).
        let footer = 12;
        let index =
            (u32::from_le_bytes(c[c.len() - 8..c.len() - 4].try_into().unwrap()) as usize + 1) * 4;
        let at = c.len() - footer - index - 1;
        c[at] ^= 0x80;
        assert_eq!(ours_xz(&c, usize::MAX, 1), Err(CodecError::Checksum));
    }
}

#[test]
fn bombs_stop_at_the_limit() {
    let zeros = vec![0u8; 64 << 20];
    let limit = 1 << 20;
    let bz = bz2(&zeros, 9);
    for t in [1, 4] {
        let mut out = Vec::new();
        let r = bzip2::decode(&bz, &mut out, limit, Threads { n: t, start: None });
        assert_eq!(r, Err(CodecError::Limit));
        assert!(out.capacity() <= 2 * limit, "bzip2 kept {}", out.capacity());
    }
    let x = xz_easy(&zeros, 6, xz2::stream::Check::Crc64);
    let mut out = Vec::new();
    assert_eq!(
        xz::decode(&x, &mut out, limit, 1 << 30, Threads::ONE),
        Err(CodecError::Limit)
    );
    assert!(out.capacity() <= 2 * limit);
    let z = zst(&zeros, 3, false);
    let mut out = Vec::new();
    assert_eq!(zstd::decode(&z, &mut out, limit), Err(CodecError::Limit));
    assert!(out.capacity() <= 2 * limit);
    // Without a content size the frame is only found too large while decoding.
    let mut e = ::zstd::stream::Encoder::new(Vec::new(), 3).unwrap();
    e.include_contentsize(false).unwrap();
    e.write_all(&zeros).unwrap();
    let z = e.finish().unwrap();
    let mut out = Vec::new();
    assert_eq!(zstd::decode(&z, &mut out, limit), Err(CodecError::Limit));
    assert!(out.capacity() <= 2 * limit);
}

/// bzip2 blocks reserve from the caller's budget while they decode, on whatever thread: a
/// budget smaller than one block's output refuses the stream even outside `reserving` (where
/// the output vector itself is the caller's to count), and nothing stays reserved after.
#[test]
fn bzip2_blocks_reserve_while_decoding() {
    use specular_core::budget::{MemoryBudget, with_budget};
    use std::sync::Arc;
    // Level 1: 100 000 run-length symbols per block, about 5 MB of zeros each.
    let zeros = vec![0u8; 48 << 20];
    let bz = bz2(&zeros, 1);
    for t in [1, 4] {
        let small = Arc::new(MemoryBudget::fixed(2 << 20));
        let r = with_budget(small.clone(), || ours_bz2(&bz, usize::MAX, t));
        assert!(
            matches!(r, Err(CodecError::Memory(_))),
            "{t} thread(s): {r:?}"
        );
        assert_eq!(small.reserved(), 0);
        // One block fits, a batch of eight in flight does not: the helpers' blocks that
        // could not reserve are decoded again in order, and the output is right.
        let one = Arc::new(MemoryBudget::fixed(12 << 20));
        let r = with_budget(one.clone(), || ours_bz2(&bz, usize::MAX, t));
        assert!(r.is_ok_and(|v| v == zeros), "{t} thread(s)");
        assert_eq!(one.reserved(), 0);
    }
}

/// Decode speed of ours against the reference library, MB/s of output.
#[test]
#[ignore = "speed report; run in release"]
fn speed() {
    let d = sample(50, 64 << 20);
    let mb = d.len() as f64 / 1e6;
    let time = |f: &mut dyn FnMut() -> usize| {
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = Instant::now();
            assert_eq!(f(), d.len());
            best = best.min(t.elapsed().as_secs_f64());
        }
        mb / best
    };
    let report = |name: &str, ours: f64, oracle: f64| {
        println!(
            "{name:<28} ours {ours:8.1} MB/s   reference {oracle:8.1} MB/s   ({:.2}x)",
            ours / oracle
        );
    };

    let c = raw_deflate(&d, 6);
    let ours = time(&mut || {
        let mut out = Vec::new();
        inflate64::inflate(&c, false, &mut out, usize::MAX).unwrap();
        out.len()
    });
    let oracle = time(&mut || {
        let mut out = vec![0u8; d.len()];
        crate::inflate::inflater().inflate(&c, &mut out).unwrap()
    });
    report("deflate (vs zlib-rs)", ours, oracle);

    let c = bz2(&d, 9);
    for t in [1, 8] {
        let ours = time(&mut || ours_bz2(&c, usize::MAX, t).unwrap().len());
        let oracle = time(&mut || {
            let mut out = Vec::new();
            ::bzip2::read::MultiBzDecoder::new(&c[..])
                .read_to_end(&mut out)
                .unwrap();
            out.len()
        });
        report(&format!("bzip2 -9, {t} thread(s)"), ours, oracle);
    }

    for (name, c) in [
        (
            "xz -6, one block",
            xz_easy(&d, 6, xz2::stream::Check::Crc64),
        ),
        ("xz -1, 4 MiB blocks", xz_blocks(&d, 4 << 20)),
    ] {
        for t in [1, 8] {
            let ours = time(&mut || ours_xz(&c, usize::MAX, t).unwrap().len());
            let oracle = time(&mut || {
                let mut out = Vec::new();
                xz2::read::XzDecoder::new_multi_decoder(&c[..])
                    .read_to_end(&mut out)
                    .unwrap();
                out.len()
            });
            report(&format!("{name}, {t} thread(s)"), ours, oracle);
        }
    }

    for level in [3, 19] {
        let c = zst(&d, level, true);
        let ours = time(&mut || ours_zst(&c, usize::MAX).unwrap().len());
        let oracle = time(&mut || ::zstd::bulk::decompress(&c, d.len()).unwrap().len());
        let pure = time(&mut || {
            let mut out = Vec::new();
            ruzstd_decode(&c, &mut out);
            out.len()
        });
        report(&format!("zstd -{level} (vs libzstd)"), ours, oracle);
        report(&format!("zstd -{level} (vs ruzstd)"), ours, pure);
    }
}

fn ruzstd_decode(c: &[u8], out: &mut Vec<u8>) {
    let mut r = ruzstd::decoding::StreamingDecoder::new(c).unwrap();
    r.read_to_end(out).unwrap();
}
