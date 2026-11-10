//! DSD WavPack against the reference decoder (wvunpack 5.8.1 `-r`: the DSD bytes,
//! bit-exact), and the PCM of [`specular_wavpack::DsdToPcm`] against a numpy computation of
//! the same filter, on the corpus of `tools/corpus/dsd-samples.py`:
//! `SPECULAR_LOSSLESS_CORPUS=<dir> cargo test -p specular-wavpack --test dsd -- --ignored --nocapture`
//! The small files in tests/data (`tools/corpus/dsd-samples.py OUT fixtures`) run always.

use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_wavpack::{DsdToPcm, decode_frame, frames, info};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

/// All the file's DSD bytes, and the blocks muted.
fn raw(d: &[u8]) -> (Vec<u8>, usize) {
    let ch = info(d).unwrap().channels;
    let mut out = Vec::new();
    let mut muted = 0;
    for fr in frames(d, usize::MAX) {
        let mut b = Vec::new();
        muted += decode_frame(&d[fr.offset..fr.offset + fr.len], None, ch, &mut b)
            .unwrap()
            .muted;
        out.extend(b.iter().map(|&v| v as u8));
    }
    (out, muted)
}

#[test]
fn fixtures_are_bit_exact() {
    // "fast", "high" and stored (incompressible) blocks, against wvunpack -r.
    for name in ["dsd-fast", "dsd-high", "dsd-stored"] {
        let d = fixture(&format!("{name}.wv"));
        let (got, muted) = raw(&d);
        assert_eq!(muted, 0, "{name}");
        assert!(got == fixture(&format!("{name}.raw")), "{name}");
    }
}

#[test]
fn facts_of_a_dsd64_file() {
    let i = info(&fixture("dsd-high.wv")).unwrap();
    let dsd = i.dsd.unwrap();
    assert_eq!((i.channels, i.sample_rate, i.float), (2, 88_200, true));
    assert_eq!(dsd.rate, 2_822_400);
    assert_eq!(specular_wavpack::dsd::name(dsd.rate), "DSD64");
    assert_eq!(dsd.samples, Some(35_280));
    assert_eq!(i.total_samples, Some(1_102));
    let f = dsd.filter;
    assert_eq!(
        (f.decimation, f.passband, f.stopband, f.taps),
        (4, 20_000, 44_100, 840)
    );
}

#[test]
fn a_damaged_block_is_dsd_silence() {
    let mut d = fixture("dsd-high.wv");
    let n = d.len();
    d[n - 100] ^= 0x10;
    let (got, muted) = raw(&d);
    assert_eq!(muted, 1);
    assert!(got.iter().all(|&b| b == 0x55));
}

#[test]
#[ignore = "needs the corpus from tools/corpus/dsd-samples.py"]
fn dsd_against_wvunpack_and_numpy() {
    let Some(dir) = std::env::var_os("SPECULAR_LOSSLESS_CORPUS")
        .map(|p| PathBuf::from(p).join("samples").join("dsd"))
    else {
        eprintln!("SPECULAR_LOSSLESS_CORPUS not set");
        return;
    };
    let manifest = std::fs::read_to_string(dir.join("manifest.tsv")).unwrap();
    let (mut total, mut exact, mut close) = (0, 0, 0);
    let mut bad = Vec::new();
    for line in manifest.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let d = std::fs::read(dir.join(f[0])).unwrap();
        let want_raw = std::fs::read(dir.join(format!("{}.raw", f[1]))).unwrap();
        let want_pcm: Vec<f32> = std::fs::read(dir.join(format!("{}.pcm", f[1])))
            .unwrap()
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        total += 1;
        let i = info(&d).unwrap();
        let dsd = i.dsd.unwrap();
        assert_eq!(i.channels, f[2].parse::<usize>().unwrap(), "{}", f[0]);
        assert_eq!(dsd.rate, f[3].parse::<u32>().unwrap(), "{}", f[0]);
        let ch = i.channels;
        let byte_rate = dsd.rate / 8;

        // The DSD bytes.
        let t = Instant::now();
        let mut raw = Vec::new();
        let mut blocks = Vec::new();
        let mut muted = 0;
        let mut first = None;
        for fr in frames(&d, usize::MAX) {
            let s = &d[fr.offset..fr.offset + fr.len];
            let h = specular_wavpack::Header::parse(s).unwrap();
            let base = *first.get_or_insert(h.block_index);
            let mut b = Vec::new();
            muted += decode_frame(s, None, ch, &mut b).unwrap().muted;
            raw.extend(b.iter().map(|&v| v as u8));
            blocks.push((h.block_index - base, b));
        }
        let t_dec = t.elapsed().as_secs_f64();

        // The PCM, block by block as the player feeds it.
        let t = Instant::now();
        let mut conv = DsdToPcm::new(byte_rate, ch);
        let mut pcm = Vec::new();
        for (idx, b) in &blocks {
            conv.run(*idx, b, &mut pcm);
        }
        let t_pcm = t.elapsed().as_secs_f64();
        let secs = raw.len() as f64 / ch as f64 / f64::from(byte_rate);
        let samples: u64 = frames(&d, usize::MAX)
            .iter()
            .map(|f| u64::from(f.samples))
            .sum();

        let raw_ok = raw == want_raw && muted == 0;
        let diff = pcm
            .iter()
            .zip(&want_pcm)
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        let pcm_ok = pcm.len() == want_pcm.len() && diff < 2e-5;
        let peak = want_pcm.iter().map(|v| v.abs()).fold(0f32, f32::max);
        eprintln!(
            "{} {}: {} ch {} bytes{} | PCM {} Hz × {} (diff {diff:.1e}, peak {peak:.2}){} | \
             decode {:.0} ms ({:.0}× real time), filter {:.0} ms ({:.0}×), taps {}",
            if raw_ok && pcm_ok { "OK " } else { "BAD" },
            f[0],
            ch,
            raw.len(),
            if raw_ok { "" } else { " RAW DIFFERS" },
            i.sample_rate,
            pcm.len() / ch,
            if pcm_ok { "" } else { " PCM DIFFERS" },
            t_dec * 1e3,
            secs / t_dec,
            t_pcm * 1e3,
            secs / t_pcm,
            conv.spec().taps,
        );
        assert_eq!(samples as usize, pcm.len() / ch, "frame sample counts");
        if let Some(n) = i.total_samples {
            assert_eq!(n, samples, "{}", f[0]);
        }
        if raw_ok {
            exact += 1;
        }
        if pcm_ok {
            close += 1;
        }
        if !(raw_ok && pcm_ok) {
            bad.push(f[0].to_owned());
        }
    }
    eprintln!("DSD bytes bit-exact: {exact}/{total}; PCM within 2e-5 of numpy: {close}/{total}");
    assert!(bad.is_empty(), "mismatches: {bad:?}");
}
