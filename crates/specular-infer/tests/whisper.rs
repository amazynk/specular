//! Whisper end to end on a real checkpoint, only when one is placed by hand:
//!
//! - `SPECULAR_WHISPER_MODEL`: a folder with `model.safetensors` and `vocab.json` from a
//!   Hugging Face `openai/whisper-*` repository.
//! - `SPECULAR_WHISPER_AUDIO`: a 16-bit PCM WAV, 16 kHz mono, of speech.
//! - `SPECULAR_WHISPER_EXPECT` (optional): words the transcript must hold (case and
//!   punctuation ignored), separated by `|`.
//!
//! - `SPECULAR_WHISPER_THREADS` (optional): CPU threads.
//! - `SPECULAR_WHISPER_ENGINE` (optional): `gpu` or `cpu` only.
//!
//! Skipped (passing) otherwise: tests never download. Prints the time per window and the
//! speed as a multiple of real time, on the GPU when there is one and on the CPU.

use std::path::PathBuf;
use std::time::Instant;

mod common;

use specular_infer::gpu::Context;
use specular_infer::whisper::decode::Options;
use specular_infer::whisper::{Whisper, cues, vad};

fn wav16k(bytes: &[u8]) -> Vec<f32> {
    // The "data" chunk of a canonical WAV.
    let mut i = 12;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let len =
            u32::from_le_bytes([bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]]) as usize;
        if id == b"data" {
            let end = (i + 8 + len).min(bytes.len());
            return bytes[i + 8..end]
                .chunks_exact(2)
                .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
                .collect();
        }
        i += 8 + len + (len & 1);
    }
    panic!("no data chunk");
}

/// The encoder graph of a small random Whisper, GPU against the CPU reference (with
/// attention whole and split into head groups), and a window read on each.
#[test]
fn encoder_on_the_gpu_matches_the_cpu() {
    use specular_infer::models::RandomWeights;
    use specular_infer::whisper::{Config, encoder_graph, testing};
    use specular_infer::{Feed, Tensor};
    let c = Config::test();
    let mel: Vec<f32> = (0..c.n_mels * c.frames())
        .map(|i| ((i * 7919 % 1000) as f32 / 500.0) - 1.0)
        .collect();
    let x = Tensor::f32(&[1, c.n_mels, c.frames()], mel);
    for split in [u64::MAX, (c.audio_ctx * c.audio_ctx * 4) as u64] {
        let mut w = RandomWeights {
            seed: 4,
            f16: false,
        };
        let g = encoder_graph(&c, &mut w, false, split).unwrap();
        common::check("whisper encoder", &g, &[Feed::Tensor(&x)], 2e-3, 2e-3);
    }
    let Some(ctx) = common::ctx() else { return };
    let audio: Vec<f32> = (0..16_000)
        .map(|i| ((i as f32) * 0.03).sin() * 0.3)
        .collect();
    let mut on_gpu = testing::tiny(8, Some(ctx), false);
    let mut on_cpu = testing::tiny(8, None, false);
    assert!(on_gpu.on_gpu() && !on_cpu.on_gpu());
    let a = on_gpu.encode(&on_gpu.mel(&audio)).unwrap();
    let b = on_cpu.encode(&on_cpu.mel(&audio)).unwrap();
    let la = on_gpu.detect_language(&a).unwrap();
    let lb = on_cpu.detect_language(&b).unwrap();
    assert_eq!(la[0].0, lb[0].0);
    assert!((la[0].1 - lb[0].1).abs() < 1e-3);
}

#[test]
fn transcribes_a_placed_recording() {
    let (Some(model), Some(audio)) = (
        std::env::var_os("SPECULAR_WHISPER_MODEL").map(PathBuf::from),
        std::env::var_os("SPECULAR_WHISPER_AUDIO").map(PathBuf::from),
    ) else {
        eprintln!("SPECULAR_WHISPER_MODEL / SPECULAR_WHISPER_AUDIO not set: skipped");
        return;
    };
    let pcm = wav16k(&std::fs::read(&audio).expect("audio"));
    let secs = pcm.len() as f64 / 16_000.0;
    let expect = std::env::var("SPECULAR_WHISPER_EXPECT").unwrap_or_default();
    let ctx = common::ctx().cloned();
    let engines: Vec<(&str, Option<&Context>)> = match &ctx {
        Some(c) => vec![("gpu", Some(c)), ("cpu", None)],
        None => vec![("cpu", None)],
    };
    let only = std::env::var("SPECULAR_WHISPER_ENGINE").unwrap_or_default();
    for (name, g) in engines {
        if !only.is_empty() && only != name {
            continue;
        }
        let t0 = Instant::now();
        let mut w = Whisper::load(
            &model.join("model.safetensors"),
            &model.join("vocab.json"),
            g,
            true,
        )
        .expect("loads");
        if let Some(n) = std::env::var("SPECULAR_WHISPER_THREADS")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            w.threads = n;
        }
        let load = t0.elapsed().as_secs_f64();
        let t1 = Instant::now();
        let regions = vad::speech(&pcm);
        let windows = vad::windows(&regions, &pcm, 30 * 16_000);
        let mut segs = Vec::new();
        for win in &windows {
            let part = &pcm[win.start..win.end];
            let off = win.start as f32 / 16_000.0;
            let got = w
                .transcribe(part, &Options::default(), &|| false, &mut |_, _, _| {})
                .expect("reads")
                .expect("not cancelled");
            segs.extend(got.into_iter().map(|mut s| {
                s.start += off;
                s.end += off;
                s
            }));
        }
        let took = t1.elapsed().as_secs_f64();
        let speech: Vec<(f64, f64)> = regions
            .iter()
            .map(|r| (r.start as f64 / 16_000.0, r.end as f64 / 16_000.0))
            .collect();
        let c = cues::cues(&segs, &speech);
        let text: String = c.iter().map(|q| q.text.replace('\n', " ") + " ").collect();
        eprintln!(
            "{name}: loaded in {load:.1} s; {secs:.1} s of sound in {took:.2} s = {:.1}x real time, {} windows",
            secs / took,
            windows.len()
        );
        eprintln!("  {:?}", w.stats());
        for q in c.iter().take(12) {
            eprintln!(
                "  {:7.2}–{:7.2}  {}",
                q.start,
                q.end,
                q.text.replace('\n', " / ")
            );
        }
        let norm = |s: &str| {
            s.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect::<String>()
        };
        let have = norm(&text);
        for want in expect.split('|').filter(|s| !s.is_empty()) {
            assert!(
                have.contains(&norm(want)),
                "{name}: {want:?} not in {have:?}"
            );
        }
        assert!(!c.is_empty());
    }
}
