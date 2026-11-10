//! Against libavcodec: the ffmpeg-encoded streams of `tests/data` (see `gen_fixtures.sh`)
//! decode bit-exactly to its PCM, and to the MD5 their STREAMINFO carries.

use std::path::Path;

use specular_flac::{Decoder, Error, StreamInfo, frames};

/// (name, bits per sample of the reference PCM file).
const FIXTURES: [(&str, u32); 16] = [
    ("c0", 16),
    ("c5", 16),
    ("c8", 16),
    ("c12", 16),
    ("fixed", 16),
    ("verb", 16),
    ("ls", 16),
    ("rs", 16),
    ("ms", 16),
    ("small", 16),
    ("m8k", 16),
    ("wasted", 16),
    ("s24", 32),
    ("s24ms", 32),
    ("s32", 32),
    ("s51", 16),
];

fn read(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

/// Splits a native file into (metadata with "fLaC", frame bytes).
fn split(file: &[u8]) -> (&[u8], &[u8]) {
    assert_eq!(&file[..4], b"fLaC");
    let mut p = 4;
    loop {
        let h = &file[p..p + 4];
        let len = (usize::from(h[1]) << 16) | (usize::from(h[2]) << 8) | usize::from(h[3]);
        p += 4 + len;
        if h[0] & 0x80 != 0 {
            return file.split_at(p);
        }
    }
}

/// Reference PCM as integers at the stream's bit depth.
fn pcm(name: &str, pcm_bits: u32, bits: u32) -> Vec<i32> {
    let raw = read(&format!("{name}.pcm"));
    if pcm_bits == 16 {
        raw.chunks_exact(2)
            .map(|c| i32::from(i16::from_le_bytes([c[0], c[1]])))
            .collect()
    } else {
        raw.chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) >> (32 - bits))
            .collect()
    }
}

#[test]
fn bit_exact_against_libavcodec_and_md5() {
    let mut total_samples = 0;
    for (name, pcm_bits) in FIXTURES {
        let file = read(&format!("{name}.flac"));
        let (meta, body) = split(&file);
        let info = StreamInfo::from_config(meta).unwrap();
        let want = pcm(name, pcm_bits, u32::from(info.bits));
        let ch = usize::from(info.channels);

        // Frame by frame, as containers hand them over.
        let mut d = Decoder::new(Some(info));
        let mut got = Vec::new();
        let mut nframes = 0;
        let mut sizes = std::collections::BTreeSet::new();
        for f in frames(body) {
            let n = d.decode_i32(f, &mut got).unwrap();
            sizes.insert(n);
            nframes += 1;
        }
        assert_eq!(d.channels(), ch, "{name}");
        assert_eq!(d.bits(), info.bits, "{name}");
        assert_eq!(d.sample_rate(), info.sample_rate, "{name}");
        assert_eq!(got.len(), want.len(), "{name}: sample count");
        let exact = got.iter().zip(&want).filter(|(a, b)| a == b).count();
        assert_eq!(exact, want.len(), "{name}: bit-exact samples");
        assert_eq!(got.len() as u64, info.total_samples * ch as u64, "{name}");
        assert_eq!(md5_of(&got, info.bits), info.md5, "{name}: MD5");

        // All frames in one call, without STREAMINFO when the frames carry everything.
        let mut d = Decoder::new(Some(info));
        let mut all = Vec::new();
        let n = d.decode_i32(body, &mut all).unwrap();
        assert_eq!(n * ch, want.len(), "{name}");
        assert_eq!(all, got, "{name}: one call");

        // Floats are the integers scaled.
        let mut fl = Vec::new();
        Decoder::new(Some(info)).decode(body, &mut fl).unwrap();
        let scale = (1u64 << (info.bits - 1)) as f32;
        assert!(fl.iter().zip(&got).all(|(&f, &i)| f == i as f32 / scale));
        assert!(fl.iter().all(|f| (-1.0..=1.0).contains(f)));

        eprintln!(
            "{name}: {} Hz {ch} ch {} bits, {nframes} frames (block sizes {sizes:?}), {exact}/{} samples bit-exact, MD5 ok",
            info.sample_rate,
            info.bits,
            want.len()
        );
        total_samples += exact;
    }
    eprintln!("all fixtures: {total_samples} samples bit-exact");
}

#[test]
fn frames_without_streaminfo() {
    // Every ffmpeg fixture codes rate and depth in the frame header.
    for (name, _) in FIXTURES {
        let file = read(&format!("{name}.flac"));
        let (meta, body) = split(&file);
        let info = StreamInfo::from_config(meta).unwrap();
        let mut d = Decoder::new(None);
        assert_eq!(d.channels(), 0);
        let mut a = Vec::new();
        d.decode_i32(body, &mut a).unwrap();
        let mut b = Vec::new();
        Decoder::new(Some(info)).decode_i32(body, &mut b).unwrap();
        assert_eq!(a, b, "{name}");
        assert_eq!(d.sample_rate(), info.sample_rate);
    }
}

#[test]
fn config_forms() {
    let file = read("c5.flac");
    let (meta, _) = split(&file);
    let info = StreamInfo::from_config(meta).unwrap();
    assert_eq!((info.sample_rate, info.channels, info.bits), (44100, 2, 16));
    let block = &meta[4..42];
    // MP4 dfLa: version and flags, then the blocks.
    let mut dfla = vec![0, 0, 0, 0];
    dfla.extend_from_slice(&[0x80 | block[0], block[1], block[2], block[3]]);
    dfla.extend_from_slice(&block[4..]);
    assert_eq!(StreamInfo::from_config(&dfla), Ok(info));
    // Ogg FLAC first packet.
    let mut ogg = b"\x7fFLAC\x01\x00\x00\x01fLaC".to_vec();
    ogg.extend_from_slice(block);
    assert_eq!(ogg.len(), 51);
    assert_eq!(StreamInfo::from_config(&ogg), Ok(info));
    ogg[5] = 2;
    assert!(matches!(
        StreamInfo::from_config(&ogg),
        Err(Error::Unsupported(_))
    ));
    assert_eq!(StreamInfo::parse(&block[4..]), Ok(info));
    assert_eq!(StreamInfo::from_config(&block[4..]), Ok(info));
    assert!(StreamInfo::from_config(&meta[..20]).is_err());
    assert!(StreamInfo::parse(&block[4..30]).is_err());
}

#[test]
fn damage_never_panics_and_crc_is_checked() {
    let file = read("small.flac");
    let (meta, body) = split(&file);
    let info = StreamInfo::from_config(meta).unwrap();
    let list: Vec<&[u8]> = frames(body).collect();

    // A flipped bit in the audio data: CRC-16 mismatch, samples still out, decoder usable.
    let mut f = list[1].to_vec();
    let mid = f.len() / 2;
    f[mid] ^= 0x10;
    let mut d = Decoder::new(Some(info));
    let mut out = Vec::new();
    let r = d.decode_i32(&f, &mut out);
    assert!(matches!(r, Err(Error::Malformed(_))), "{r:?}");
    let mut good = Vec::new();
    d.decode_i32(list[2], &mut good).unwrap();
    let mut want = Vec::new();
    Decoder::new(Some(info))
        .decode_i32(list[2], &mut want)
        .unwrap();
    assert_eq!(good, want);

    // Truncations and byte damage everywhere.
    let mut seed = 0x1234_5678u32;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let mut errors = 0;
    for f in &list {
        for cut in (0..f.len()).step_by(7) {
            if d.decode_i32(&f[..cut], &mut out).is_err() {
                errors += 1;
            }
        }
        for _ in 0..200 {
            let mut g = f.to_vec();
            for _ in 0..1 + rnd() % 4 {
                let i = rnd() as usize % g.len();
                g[i] = rnd() as u8;
            }
            out.clear();
            let _ = d.decode_i32(&g, &mut out);
            let _ = d.decode(&g, &mut Vec::new());
        }
    }
    assert!(errors > 0);
    // Garbage and empty input.
    let junk: Vec<u8> = (0..5000).map(|_| rnd() as u8).collect();
    let _ = d.decode_i32(&junk, &mut out);
    assert_eq!(d.decode_i32(&[], &mut out), Ok(0));
    assert_eq!(frames(&[]).count(), 0);
    let _ = frames(&junk).count();
}

/// MD5 of the samples as STREAMINFO defines it: little-endian, (bits + 7) / 8 bytes each.
fn md5_of(samples: &[i32], bits: u8) -> [u8; 16] {
    let bytes = usize::from(bits).div_ceil(8);
    let mut data = Vec::with_capacity(samples.len() * bytes);
    for &s in samples {
        data.extend_from_slice(&s.to_le_bytes()[..bytes]);
    }
    md5(&data)
}

/// RFC 1321 MD5, test oracle only.
fn md5(msg: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let k: Vec<u32> = (0..64)
        .map(|i| ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32)
        .collect();
    let mut h = [0x67452301u32, 0xefcdab89, 0x98badcfe, 0x10325476];
    let mut m = msg.to_vec();
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&((msg.len() as u64) * 8).to_le_bytes());
    for chunk in m.chunks_exact(64) {
        let w: Vec<u32> = chunk
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let [mut a, mut b, mut c, mut d] = h;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let t = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(k[i])
                    .wrapping_add(w[g])
                    .rotate_left(S[i]),
            );
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 16];
    for (o, x) in out.chunks_exact_mut(4).zip(h) {
        o.copy_from_slice(&x.to_le_bytes());
    }
    out
}

#[test]
fn md5_known_answer() {
    let hex = |d: [u8; 16]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(hex(md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(
        hex(md5(b"The quick brown fox jumps over the lazy dog")),
        "9e107d9d372bb6826bd81d3542a419d6"
    );
}
