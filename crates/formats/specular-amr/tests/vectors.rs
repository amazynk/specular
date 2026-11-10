//! Bit-exactness against the 3GPP test sequences (TS 26.074 for AMR, TS 26.174 for
//! AMR-WB), fetched by tools/corpus/fetch-amr.sh (not committed):
//!
//!   SPECULAR_AMR_CORPUS=<dir> cargo test -p specular-amr --test vectors -- --ignored --nocapture

use specular_amr::{Frame, RxType, nb, wb};
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    std::env::var_os("SPECULAR_AMR_CORPUS").map(PathBuf::from)
}

fn words(p: &Path) -> Vec<i16> {
    std::fs::read(p)
        .unwrap()
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
            {
                v.push(p);
            }
        }
    }
    v.sort();
    v
}

fn with_ext(p: &Path, ext: &str) -> Option<PathBuf> {
    [ext.to_uppercase(), ext.to_lowercase()]
        .iter()
        .map(|e| p.with_extension(e))
        .find(|q| q.exists())
}

/// Per-file result: frames, samples differing, largest difference, SNR in dB.
struct Cmp {
    frames: usize,
    diff: usize,
    max: i32,
    snr: f64,
}

fn compare(got: &[i16], want: &[i16], frames: usize) -> Cmp {
    let n = got.len().min(want.len());
    let (mut sig, mut err, mut diff, mut max) = (0f64, 0f64, 0, 0i32);
    for i in 0..n {
        let d = got[i] as i32 - want[i] as i32;
        if d != 0 {
            diff += 1;
            max = max.max(d.abs());
        }
        sig += (want[i] as f64).powi(2);
        err += (d as f64).powi(2);
    }
    let snr = if err == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (sig / err).log10()
    };
    Cmp {
        frames,
        diff: diff + got.len().abs_diff(want.len()),
        max,
        snr,
    }
}

fn nb_tx_to_rx(tx: i16) -> RxType {
    match tx {
        0 => RxType::SpeechGood,
        1 => RxType::SidFirst,
        2 => RxType::SidUpdate,
        3 => RxType::NoData,
        4 => RxType::SpeechDegraded,
        5 => RxType::SpeechBad,
        6 => RxType::SidBad,
        _ => RxType::Onset,
    }
}

fn decode_nb_cod(cod: &[i16]) -> (Vec<i16>, usize) {
    let mut dec = nb::Decoder::new();
    let mut out = Vec::new();
    let mut frames = 0;
    for f in cod.chunks_exact(250) {
        let mut frame = Frame::empty(nb_tx_to_rx(f[0]), f[245] as u8);
        for i in 0..244 {
            frame.bits[i] = f[1 + i] as u8;
        }
        let mut s = [0i16; 160];
        dec.decode(&frame, &mut s);
        out.extend_from_slice(&s);
        frames += 1;
    }
    (out, frames)
}

fn report(name: &str, c: &Cmp) {
    println!(
        "{name:<28} frames {:>6} diff {:>7} max {:>5} snr {:>7.1} dB",
        c.frames, c.diff, c.max, c.snr
    );
}

#[test]
#[ignore]
fn amr_nb_test_sequences() {
    let Some(root) = corpus() else { return };
    let dir = root.join("nbv");
    let mut total = (0, 0, 0);
    for cod in files(&dir, "cod") {
        let Some(out) = with_ext(&cod, "out") else {
            continue;
        };
        let (got, frames) = decode_nb_cod(&words(&cod));
        let c = compare(&got, &words(&out), frames);
        report(&cod.strip_prefix(&dir).unwrap().display().to_string(), &c);
        total.0 += 1;
        total.1 += frames;
        if c.diff != 0 {
            total.2 += 1;
        }
    }
    println!(
        "AMR: {} sequences, {} frames, {} not bit-exact",
        total.0, total.1, total.2
    );
    assert!(total.0 > 0);
    assert_eq!(total.2, 0);
}

fn wb_decode_cod(cod: &[i16]) -> (Vec<i16>, usize) {
    let mut dec = wb::Decoder::default();
    let mut out = Vec::new();
    let mut frames = 0;
    let mut i = 0;
    while i + 3 <= cod.len() {
        let (tt, ft, mode) = (cod[i] as u16, cod[i + 1], cod[i + 2]);
        i += 3;
        let rx = if tt == 0x6b21 {
            // TX frame type
            match ft {
                0 => RxType::SpeechGood,
                1 => RxType::SidFirst,
                2 => RxType::SidUpdate,
                _ => RxType::NoData,
            }
        } else {
            match ft {
                0 => RxType::SpeechGood,
                1 => RxType::SpeechDegraded,
                2 => RxType::SpeechLost,
                3 => RxType::SpeechBad,
                4 => RxType::SidFirst,
                5 => RxType::SidUpdate,
                6 => RxType::SidBad,
                _ => RxType::NoData,
            }
        };
        let coding_mode = if matches!(
            rx,
            RxType::SidFirst | RxType::SidUpdate | RxType::NoData | RxType::SidBad
        ) {
            9
        } else {
            mode as usize
        };
        let n = wb::BITS[coding_mode];
        if i + n > cod.len() {
            break;
        }
        let mut frame = Frame::empty(rx, mode as u8);
        for k in 0..n {
            frame.bits[k] = (cod[i + k] > 0) as u8;
        }
        i += n;
        let mut s = [0i16; 320];
        dec.decode(&frame, &mut s);
        out.extend_from_slice(&s);
        frames += 1;
    }
    (out, frames)
}

#[test]
#[ignore]
fn amr_wb_test_sequences() {
    let Some(root) = corpus() else { return };
    let dir = root.join("wbv");
    let mut total = (0, 0, 0);
    for cod in files(&dir, "cod") {
        let Some(out) = with_ext(&cod, "out") else {
            continue;
        };
        let (got, frames) = wb_decode_cod(&words(&cod));
        let c = compare(&got, &words(&out), frames);
        report(&cod.strip_prefix(&dir).unwrap().display().to_string(), &c);
        total.0 += 1;
        total.1 += frames;
        if c.diff != 0 {
            total.2 += 1;
        }
    }
    println!(
        "AMR-WB: {} sequences, {} frames, {} not bit-exact",
        total.0, total.1, total.2
    );
    assert!(total.0 > 0);
    assert_eq!(total.2, 0);
}

/// Where our decode of each oracle fixture first departs from the reference decode
/// (`<name>.ref`, written by tools/corpus/amr-oracle.py into <corpus>/fixtures).
#[test]
#[ignore]
fn fixtures_against_reference() {
    let Some(root) = corpus() else { return };
    let dir = root.join("fixtures");
    let mut bad = 0;
    for p in files(&dir, "ref") {
        let src = p.with_extension("");
        let bytes = std::fs::read(&src).unwrap();
        let mut s = specular_amr::Stream::new(&bytes).unwrap();
        let got = s.decode_all();
        let want = words(&p);
        let n = s.info.codec.frame_len();
        let first = got.iter().zip(&want).position(|(a, b)| a != b);
        let c = compare(&got, &want, got.len() / n);
        report(&src.file_name().unwrap().to_string_lossy(), &c);
        if let Some(i) = first {
            println!("    first difference in frame {} (sample {})", i / n, i % n);
            bad += 1;
        }
    }
    assert_eq!(bad, 0);
}
