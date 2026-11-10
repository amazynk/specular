//! ProRes RAW frames against FFmpeg's `prores_raw` decoder (8.0.1, the system `ffmpeg`).
//!
//! Skipped unless `SPECULAR_RAW_VIDEO_DIR` names the corpus (`tools/corpus/fetch-raw-video.sh`)
//! and `SPECULAR_RAW_VIDEO_ORACLE` the folder of FFmpeg's output: for each `<name>.mov`, a
//! `<name>.bayer16` of all frames, `ffmpeg -i <name>.mov -map 0:v -f rawvideo -pix_fmt
//! bayer_rggb16le <name>.bayer16` (the fetch script writes them).
//!
//! FFmpeg 8.0 outputs the transform's 12-bit code shifted to 16 bits, before linearisation,
//! so the comparison is of [`Output::Coded`].

use std::path::PathBuf;

use specular_prores::raw::{self, Output};

fn be32(d: &[u8], o: usize) -> usize {
    u32::from_be_bytes(d[o..o + 4].try_into().unwrap()) as usize
}

/// Child boxes of `d[a..b]`: (type, payload start, end).
fn boxes(d: &[u8], a: usize, b: usize) -> Vec<([u8; 4], usize, usize)> {
    let mut out = Vec::new();
    let mut p = a;
    while p + 8 <= b {
        let mut s = be32(d, p);
        let mut h = 8;
        if s == 1 {
            s = u64::from_be_bytes(d[p + 8..p + 16].try_into().unwrap()) as usize;
            h = 16;
        }
        if s < h || p + s > b {
            break;
        }
        out.push((d[p + 4..p + 8].try_into().unwrap(), p + h, p + s));
        p += s;
    }
    out
}

fn find(d: &[u8], a: usize, b: usize, t: &[u8; 4]) -> Option<(usize, usize)> {
    boxes(d, a, b)
        .into_iter()
        .find(|x| &x.0 == t)
        .map(|x| (x.1, x.2))
}

/// The ProRes RAW samples of a movie whose video track has one sample per chunk.
fn frames(d: &[u8]) -> Vec<&[u8]> {
    let (ma, mb) = find(d, 0, d.len(), b"moov").expect("moov");
    for (t, a, b) in boxes(d, ma, mb) {
        if &t != b"trak" {
            continue;
        }
        let mut r = (a, b);
        for t in [b"mdia", b"minf", b"stbl"] {
            r = find(d, r.0, r.1, t).expect("stbl");
        }
        let (sa, _) = find(d, r.0, r.1, b"stsd").unwrap();
        if &d[sa + 12..sa + 16] != b"aprh" && &d[sa + 12..sa + 16] != b"aprn" {
            continue;
        }
        let (za, _) = find(d, r.0, r.1, b"stsz").unwrap();
        let (ca, _) = find(d, r.0, r.1, b"stco").unwrap();
        let n = be32(d, za + 8);
        return (0..n)
            .map(|i| {
                let size = be32(d, za + 12 + 4 * i);
                let off = be32(d, ca + 8 + 4 * i);
                &d[off..off + size]
            })
            .collect();
    }
    panic!("no ProRes RAW track")
}

#[test]
fn frames_match_ffmpeg() {
    let (Some(dir), Some(oracle)) = (
        std::env::var_os("SPECULAR_RAW_VIDEO_DIR"),
        std::env::var_os("SPECULAR_RAW_VIDEO_ORACLE"),
    ) else {
        eprintln!("SPECULAR_RAW_VIDEO_DIR / SPECULAR_RAW_VIDEO_ORACLE not set; skipping");
        return;
    };
    let mut seen = 0;
    let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "mov"))
        .collect();
    names.sort();
    for p in names {
        let want_path = PathBuf::from(&oracle)
            .join(p.file_stem().unwrap())
            .with_extension("bayer16");
        let Ok(want) = std::fs::read(&want_path) else {
            eprintln!("{}: no oracle output", p.display());
            continue;
        };
        let data = std::fs::read(&p).unwrap();
        let mut offset = 0;
        for (i, f) in frames(&data).into_iter().enumerate() {
            let h = raw::raw_header(f).unwrap();
            let n = (h.width * h.height) as usize;
            let mut got = vec![0u16; n];
            let t = std::time::Instant::now();
            raw::decode(f, &h, &mut got, Output::Coded, true).unwrap();
            let ms = t.elapsed().as_secs_f64() * 1e3;
            let mut legacy = vec![0u16; n];
            raw::decode(f, &h, &mut legacy, Output::CodedFfmpeg80, true).unwrap();
            let want: Vec<u16> = want[offset..offset + 2 * n]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            offset += 2 * n;
            let differ = |v: &[u16]| v.iter().zip(&want).filter(|(a, b)| a != b).count();
            let max = |v: &[u16]| {
                v.iter()
                    .zip(&want)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap_or(0)
            };
            // Blocks (8x8 of one component) where the end rule changes anything.
            let mut blocks = std::collections::HashSet::new();
            for (k, (a, b)) in got.iter().zip(&want).enumerate() {
                if a != b {
                    let (y, x) = (k / h.width as usize, k % h.width as usize);
                    blocks.insert((y / 16, x / 16, (y & 1) * 2 + (x & 1)));
                }
            }
            eprintln!(
                "{} frame {i}: {}×{} {} ({}), {ms:.0} ms; FFmpeg 8.0 end rule: {} of {n} \
                 samples differ; reference end rule: {} differ (max {}) in {} blocks",
                p.display(),
                h.width,
                h.height,
                String::from_utf8_lossy(&h.vendor),
                h.cfa_name(),
                differ(&legacy),
                differ(&got),
                max(&got),
                blocks.len(),
            );
            if let Some(d) = std::env::var_os("SPECULAR_RAW_VIDEO_DUMP") {
                let bytes: Vec<u8> = got.iter().flat_map(|v| v.to_le_bytes()).collect();
                std::fs::write(PathBuf::from(d).join(format!("frame{i}.ours16")), bytes).unwrap();
            }
            // Bit-exact with FFmpeg 8.0's reading; the reference rule differs only where 8.0
            // reads past a component's end (a stray coefficient: whole blocks off by a few
            // codes).
            assert_eq!(differ(&legacy), 0, "{} frame {i}", p.display());
            assert!(max(&got) <= 64 && blocks.len() * 64 >= differ(&got));
            seen += 1;
        }
    }
    assert!(seen > 0, "no ProRes RAW frames compared");
}
