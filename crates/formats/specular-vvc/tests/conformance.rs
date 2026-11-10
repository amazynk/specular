//! JVET conformance bitstreams against ffmpeg's native VVC decoder (the oracle files written
//! by tools/corpus/fetch-vvc.sh into tools/corpus/conformance/vvc/, gitignored).
//!
//! For each stream: the first output picture is compared sample by sample with
//! `<name>.f0.yuv`, and, for streams whose pictures are all intra, every picture's MD5 with
//! `<name>.framemd5`. Mismatches are reported per stream; the test fails on any mismatch in
//! a stream that decodes. `SPECULAR_VVC_STREAMS=A,B` limits the run to some streams.

use std::path::{Path, PathBuf};
use std::time::Instant;

use md5::{Digest, Md5};
use specular_vvc::{ChromaFormat, Decoded, Frame, Options, decode_intra_pictures, nal};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance/vvc")
}

/// The frame as ffmpeg's rawvideo output: planes, 8-bit or 16-bit little-endian samples.
fn raw(f: &Frame) -> Vec<u8> {
    let mut out = Vec::new();
    let (cw, ch) = f.chroma_size();
    let push = |out: &mut Vec<u8>, plane: &[u16], stride: u32, w: u32, h: u32| {
        for y in 0..h {
            for x in 0..w {
                let v = plane[(y * stride + x) as usize];
                if f.bit_depth > 8 {
                    out.extend_from_slice(&v.to_le_bytes());
                } else {
                    out.push(v as u8);
                }
            }
        }
    };
    push(&mut out, &f.y, f.y_stride, f.width, f.height);
    if f.chroma != ChromaFormat::Monochrome {
        push(&mut out, &f.cb, f.c_stride, cw, ch);
        push(&mut out, &f.cr, f.c_stride, cw, ch);
    }
    out
}

/// Mismatching samples per plane between our frame and the oracle bytes.
fn mismatches(f: &Frame, oracle: &[u8]) -> [usize; 3] {
    let ours = raw(f);
    let bps = if f.bit_depth > 8 { 2 } else { 1 };
    let (cw, ch) = f.chroma_size();
    let sizes = [
        (f.width * f.height) as usize,
        (cw * ch) as usize,
        (cw * ch) as usize,
    ];
    let mut out = [0usize; 3];
    let mut off = 0;
    for p in 0..3 {
        for i in 0..sizes[p] {
            let a = &ours.get(off + i * bps..off + (i + 1) * bps);
            let b = &oracle.get(off + i * bps..off + (i + 1) * bps);
            if a != b {
                out[p] += 1;
            }
        }
        off += sizes[p] * bps;
    }
    out
}

fn md5_hex(b: &[u8]) -> String {
    let d = Md5::digest(b);
    d.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn jvet_conformance_against_ffmpeg() {
    let dir = corpus();
    let Ok(manifest) = std::fs::read_to_string(dir.join("manifest.tsv")) else {
        eprintln!("VVC corpus absent (run tools/corpus/fetch-vvc.sh); skipped");
        return;
    };
    let only: Option<Vec<String>> = std::env::var("SPECULAR_VVC_STREAMS")
        .ok()
        .map(|s| s.split(',').map(str::to_owned).collect());
    let threads = std::env::var("SPECULAR_VVC_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let mut bad = Vec::new();
    for line in manifest.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        let name = cols[0];
        if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == name)) {
            continue;
        }
        let Ok(bits) = std::fs::read(dir.join(format!("{name}.bit"))) else {
            continue;
        };
        let units = nal::split_annexb(&bits);
        let t = Instant::now();
        let mut pics: Vec<Decoded> = Vec::new();
        let r = decode_intra_pictures(
            &units,
            &Options {
                threads,
                cancel: None,
            },
            &mut |d| {
                pics.push(d);
                false
            },
        );
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let skipped = match &r {
            Ok(n) => *n,
            Err(e) => {
                eprintln!("{name}: {e} ({} intra pictures before)", pics.len());
                if pics.is_empty() {
                    // Only a feature we refuse by design may yield nothing; any other error fails.
                    if !matches!(e, specular_vvc::Error::Unsupported(_)) {
                        bad.push(format!("{name}: {e}"));
                    }
                    continue;
                }
                0
            }
        };
        let f0 = std::fs::read(dir.join(format!("{name}.f0.yuv"))).unwrap_or_default();
        // The first output picture is the lowest POC of the first coded video sequence.
        let first = pics.iter().min_by_key(|p| (p.cvs, p.poc));
        if let (Ok(d), Ok(k)) = (
            std::env::var("SPECULAR_VVC_DUMP"),
            std::env::var("SPECULAR_VVC_DUMP_INDEX"),
        ) {
            let mut sorted: Vec<&Decoded> = pics.iter().collect();
            sorted.sort_by_key(|p| (p.cvs, p.poc));
            if let Some(p) = k.parse::<usize>().ok().and_then(|k| sorted.get(k)) {
                let _ = std::fs::write(
                    Path::new(&d).join(format!("{name}.ours{k}.yuv")),
                    raw(&p.frame),
                );
            }
        }
        let mm = first.map(|p| mismatches(&p.frame, &f0)).unwrap_or([0; 3]);
        if let (Ok(d), Some(p)) = (std::env::var("SPECULAR_VVC_DUMP"), first) {
            let _ = std::fs::write(
                Path::new(&d).join(format!("{name}.ours.yuv")),
                raw(&p.frame),
            );
        }
        let damaged: usize = pics.iter().map(|p| p.damaged).sum();
        let mut md5_ok = None;
        if skipped == 0
            && let Ok(fm) = std::fs::read_to_string(dir.join(format!("{name}.framemd5")))
        {
            let want: Vec<&str> = fm
                .lines()
                .filter(|l| !l.starts_with('#'))
                .filter_map(|l| l.rsplit(',').next().map(str::trim))
                .collect();
            let mut sorted: Vec<&Decoded> = pics.iter().collect();
            sorted.sort_by_key(|p| (p.cvs, p.poc));
            let got: Vec<String> = sorted.iter().map(|p| md5_hex(&raw(&p.frame))).collect();
            let same = got.len() == want.len() && got.iter().zip(&want).all(|(a, b)| a == b);
            let matching = got.iter().zip(&want).filter(|(a, b)| a == b).count();
            let wrong: Vec<usize> = (0..got.len().min(want.len()))
                .filter(|&i| got[i] != want[i])
                .take(8)
                .collect();
            if !wrong.is_empty() {
                eprintln!(
                    "{name}: first mismatching frames (POC order) {wrong:?}, POCs {:?}",
                    wrong.iter().map(|&i| sorted[i].poc).collect::<Vec<_>>()
                );
            }
            md5_ok = Some((same, matching, want.len()));
        }
        eprintln!(
            "{name}: {} intra pictures ({} inter skipped) in {ms:.0} ms; first picture mismatches Y {} Cb {} Cr {}; damaged CTUs {damaged}; frame MD5s {}",
            pics.len(),
            skipped,
            mm[0],
            mm[1],
            mm[2],
            md5_ok
                .map(|(s, m, n)| format!("{m}/{n}{}", if s { " (all equal)" } else { "" }))
                .unwrap_or_else(|| "-".into()),
        );
        if mm != [0; 3] || damaged > 0 || md5_ok.is_some_and(|(s, _, _)| !s) {
            bad.push(name.to_owned());
        }
    }
    assert!(bad.is_empty(), "streams with mismatches: {bad:?}");
}
