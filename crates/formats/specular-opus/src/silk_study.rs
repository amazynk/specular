//! A study of our SILK reconstruction against the reference decoder at the internal rate
//! (tests only): libopus, used as a black box, decodes each stream at 8, 12 and 16 kHz, so
//! frames coded at that rate come out without its resampler; we compare those with our
//! output before upsampling, frame by frame, split by bandwidth, channels, signal type and
//! loss case (a concealed packet, the frames after it, a packet rebuilt from LBRR data).
//!
//! `tools/corpus/specular-opus-silk.sh`, then
//! `cargo test -p specular-opus --lib silk_study -- --ignored --nocapture`
//! (`SPECULAR_SILK_ONLY=<name part>` limits the inputs).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{Decoder, Head};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/video2/specular-opus-silk")
}

fn vectors() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/video2/specular-opus")
}

/// The loss pattern of `specular-opus-silk-oracle.py`.
pub(crate) fn lost(i: usize) -> bool {
    (i >= 5 && (i - 5).is_multiple_of(9)) || (23..=25).contains(&i)
}

fn read_bit(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut p = 0;
    while p + 8 <= data.len() {
        let len = u32::from_be_bytes(data[p..p + 4].try_into().unwrap()) as usize;
        p += 8;
        out.push(data[p..p + len].to_vec());
        p += len;
    }
    out
}

fn head(channels: u8) -> Head {
    let mut h = b"OpusHead".to_vec();
    h.extend_from_slice(&[1, channels, 0, 0, 0x80, 0xbb, 0, 0, 0, 0, 0]);
    Head::parse(&h).unwrap()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Loss {
    None,
    Conceal,
    Fec,
}

/// One frame of ours at the internal rate, placed on the 48 kHz time line.
pub(crate) struct Placed {
    pub(crate) pos48: usize,
    pub(crate) fs_khz: usize,
    pub(crate) voiced: bool,
    /// 0: a normal frame; 1: concealed; 2: the two packets after a loss; 3: rebuilt from LBRR.
    pub(crate) case: u8,
    /// For a concealed frame: its place in the run of lost frames (1, 2, ...).
    pub(crate) run: usize,
    pub(crate) pcm: Vec<f32>,
}

pub(crate) fn decode(packets: &[Vec<u8>], channels: usize, loss: Loss) -> Vec<Placed> {
    let mut d = Decoder::new(&head(channels as u8)).unwrap();
    d.streams[0].silk_mut().tap = Some(Vec::new());
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut pcm = Vec::new();
    let mut since_loss = 99;
    let mut run = 0;
    for (i, p) in packets.iter().enumerate() {
        pcm.clear();
        let is_lost = loss != Loss::None && lost(i);
        let got = if is_lost {
            since_loss = 0;
            let next = packets.get(i + 1).filter(|_| !lost(i + 1));
            match (loss, next) {
                (Loss::Fec, Some(n)) => d.decode_fec(n, &mut pcm).unwrap_or(0),
                _ => d.decode(&[], &mut pcm).unwrap_or(0),
            }
        } else {
            since_loss += 1;
            d.decode(p, &mut pcm).unwrap_or(0)
        };
        let frames = std::mem::take(d.streams[0].silk_mut().tap.as_mut().unwrap());
        let mut off = 0;
        for f in frames {
            let n = f.pcm.len() / channels;
            let case = if is_lost && loss == Loss::Fec && !f.lost {
                3
            } else if is_lost || f.lost {
                1
            } else if since_loss <= 2 {
                2
            } else {
                0
            };
            run = if case == 1 { run + 1 } else { 0 };
            out.push(Placed {
                pos48: pos + off,
                fs_khz: f.fs_khz,
                voiced: f.voiced,
                case,
                run,
                pcm: f.pcm,
            });
            off += n * 48 / f.fs_khz;
        }
        pos += got;
    }
    out
}

#[derive(Default)]
struct Stat {
    frames: usize,
    exact: usize,
    sig: f64,
    err: f64,
    max: i32,
}

impl Stat {
    fn snr(&self) -> f64 {
        10.0 * (self.sig.max(1e-9) / self.err.max(1e-9)).log10()
    }
}

fn q16(v: f32) -> i32 {
    (v * 32768.0).round().clamp(-32768.0, 32767.0) as i32
}

/// Our output's lead over libopus at its internal rate, in samples.
fn delay(rate: usize) -> isize {
    match rate {
        8 => 4,
        12 => 9,
        _ => 12,
    }
}

/// Compares ours with the reference at `rate`, frame by frame, at sample delay `delay`.
#[allow(clippy::too_many_arguments)]
fn compare(
    ours: &[Placed],
    r: &[i16],
    channels: usize,
    rate: usize,
    delay: isize,
    key: &str,
    name: &str,
    stats: &mut BTreeMap<String, Stat>,
) {
    for f in ours.iter().filter(|f| f.fs_khz == rate) {
        let start = (f.pos48 * rate / 48) as isize + delay;
        let n = f.pcm.len() / channels;
        if start < 0 || (start as usize + n) * channels > r.len() {
            continue;
        }
        let mut st = Stat::default();
        let mut first = usize::MAX;
        for k in 0..n * channels {
            let a = i32::from(r[start as usize * channels + k]);
            let b = q16(f.pcm[k]);
            st.sig += f64::from(a) * f64::from(a);
            st.err += f64::from(a - b) * f64::from(a - b);
            st.max = st.max.max((a - b).abs());
            if a != b && first == usize::MAX {
                first = k / channels;
            }
        }
        if std::env::var("SPECULAR_SILK_FRAMES").is_ok() {
            eprintln!(
                "frame {name} {key} pos {} fs {rate} voiced {} case {}: {:.1} dB max {} first {first}",
                f.pos48 / 960,
                f.voiced,
                f.case,
                10.0 * (st.sig.max(1e-9) / st.err.max(1e-9)).log10(),
                st.max
            );
        }
        if f.case == 1 {
            // Level of ours against the reference's concealment, by place in the run.
            let (mut eo, mut er) = (0f64, 0f64);
            for k in 0..n * channels {
                let a = f64::from(r[start as usize * channels + k]);
                let b = f64::from(q16(f.pcm[k]));
                er += a * a;
                eo += b * b;
            }
            let e = stats
                .entry(format!(
                    "level {key} {} run {}",
                    if f.voiced { "voiced" } else { "unvoiced" },
                    f.run.min(6)
                ))
                .or_default();
            e.frames += 1;
            e.sig += er;
            e.err += eo;
        }
        let kind = match f.case {
            1 => "lost",
            2 => "after-loss",
            3 => "lbrr",
            _ if f.voiced => "voiced",
            _ => "other",
        };
        let bw = match rate {
            8 => "NB",
            12 => "MB",
            _ => "WB",
        };
        let ch = if channels == 2 { "stereo" } else { "mono" };
        for k in [
            format!("{key} {bw} {ch} {kind}"),
            format!("{key} all {kind}"),
            format!("{key} {bw} all"),
            format!("file {key} {name} {bw}"),
        ] {
            let e = stats.entry(k).or_default();
            e.frames += 1;
            e.exact += usize::from(st.err == 0.0);
            e.sig += st.sig;
            e.err += st.err;
            e.max = e.max.max(st.max);
        }
    }
}

fn inputs() -> Vec<(String, PathBuf, usize)> {
    let mut v = Vec::new();
    for n in ["02", "03", "04", "05", "06", "12"] {
        v.push((
            format!("testvector{n}"),
            vectors().join(format!("testvector{n}.bit")),
            2,
        ));
    }
    if let Ok(rd) = std::fs::read_dir(dir()) {
        let mut names: Vec<String> = rd
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.ends_with(".bit") && !n.starts_with("testvector"))
            .collect();
        names.sort();
        for n in names {
            let stem = n.trim_end_matches(".bit").to_string();
            let ch = if stem.contains("-s") { 2 } else { 1 };
            v.push((stem, dir().join(&n), ch));
        }
    }
    let only = std::env::var("SPECULAR_SILK_ONLY").ok();
    v.retain(|(n, _, _)| only.as_deref().is_none_or(|o| n.contains(o)));
    v
}

/// The per-category results over every input: (key, frames, exact, SNR dB, max LSB).
pub(crate) fn run(losses: &[Loss]) -> Vec<(String, usize, usize, f64, i32)> {
    let mut stats: BTreeMap<String, Stat> = BTreeMap::new();
    for (name, bit, channels) in inputs() {
        let Ok(data) = std::fs::read(&bit) else {
            continue;
        };
        let packets = read_bit(&data);
        for &loss in losses {
            let (suf, key) = match loss {
                Loss::None => ("", "clean"),
                Loss::Conceal => (".plc", "plc"),
                Loss::Fec => (".fec", "fec"),
            };
            let ours = decode(&packets, channels, loss);
            if let Ok(dump) = std::env::var("SPECULAR_SILK_DUMP") {
                // Ours on the reference's time line at each rate (s16le), for plotting.
                for rate in [8usize, 12, 16] {
                    let mut v = Vec::new();
                    for f in ours.iter().filter(|f| f.fs_khz == rate) {
                        let start = ((f.pos48 * rate / 48) as isize + delay(rate)).max(0) as usize
                            * channels;
                        if v.len() < start + f.pcm.len() {
                            v.resize(start + f.pcm.len(), 0i16);
                        }
                        for (k, &x) in f.pcm.iter().enumerate() {
                            v[start + k] = q16(x) as i16;
                        }
                    }
                    if !v.is_empty() {
                        let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
                        std::fs::write(format!("{dump}/{name}.{rate}{suf}.ours.pcm"), b).unwrap();
                    }
                }
            }
            for rate in [8usize, 12, 16] {
                let Ok(r) = std::fs::read(dir().join(format!("{name}.{rate}{suf}.pcm"))) else {
                    continue;
                };
                let r: Vec<i16> = r
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();
                compare(
                    &ours,
                    &r,
                    channels,
                    rate,
                    delay(rate),
                    key,
                    &name,
                    &mut stats,
                );
            }
        }
    }
    stats
        .into_iter()
        .map(|(k, s)| (k.clone(), s.frames, s.exact, s.snr(), s.max))
        .collect()
}

#[test]
#[ignore = "a study aid; needs tools/corpus/specular-opus-silk.sh"]
fn silk_study() {
    if !dir().exists() {
        eprintln!("skipped: run tools/corpus/specular-opus-silk.sh");
        return;
    }
    // Which delay lines us up with the reference at each rate?
    for (name, bit, channels) in
        inputs()
            .into_iter()
            .take(if std::env::var("SPECULAR_SILK_DELAYS").is_ok() {
                3
            } else {
                0
            })
    {
        let packets = read_bit(&std::fs::read(&bit).unwrap());
        let ours = decode(&packets, channels, Loss::None);
        for rate in [8usize, 12, 16] {
            let Ok(r) = std::fs::read(dir().join(format!("{name}.{rate}.pcm"))) else {
                continue;
            };
            let r: Vec<i16> = r
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
            let key = format!("d {} all", ["NB", "MB", "WB"][(rate - 8) / 4]);
            let best = (-4..=24)
                .filter_map(|delay| {
                    let mut st = BTreeMap::new();
                    compare(&ours, &r, channels, rate, delay, "d", &name, &mut st);
                    st.get(&key).map(|s| (delay, s.snr(), s.frames))
                })
                .max_by(|a, b| a.1.total_cmp(&b.1));
            eprintln!("{name} {rate} kHz: best delay {best:?}");
        }
    }
    // Speed: SILK wideband vector 4 to 48 kHz stereo.
    if let Ok(data) = std::fs::read(vectors().join("testvector04.bit")) {
        let packets = read_bit(&data);
        let t = std::time::Instant::now();
        let mut d = Decoder::new(&head(2)).unwrap();
        let mut pcm = Vec::new();
        for p in &packets {
            let _ = d.decode(p, &mut pcm);
        }
        let secs = pcm.len() as f64 / 2.0 / 48_000.0;
        eprintln!(
            "vector 4: {secs:.1} s decoded in {:.1} ms ({:.0}x real time)",
            t.elapsed().as_secs_f64() * 1e3,
            secs / t.elapsed().as_secs_f64()
        );
    }
    for (k, frames, exact, snr, max) in run(&[Loss::None, Loss::Conceal, Loss::Fec]) {
        eprintln!("{k:32} {frames:6} frames {exact:6} exact {snr:7.2} dB max {max} LSB");
    }
}

/// The regression gate of the study (runs when tools/corpus/specular-opus-silk.sh has been
/// run; skips otherwise): every SILK frame matches libopus at the internal rate except at a
/// switch of internal rate (libopus's resampler changes there, which we don't follow); a
/// frame rebuilt from LBRR data matches whenever the frame before it did; concealment is
/// within 3 dB of libopus's level over the first three lost frames.
#[test]
fn silk_matches_libopus_at_the_internal_rate() {
    if !dir().exists() {
        eprintln!("skipped: run tools/corpus/specular-opus-silk.sh");
        return;
    }
    let mut frames = 0;
    let mut exact = 0;
    let mut switch_misses = 0;
    let (mut lbrr, mut lbrr_exact) = (0, 0);
    for (name, bit, channels) in inputs() {
        let Ok(data) = std::fs::read(&bit) else {
            continue;
        };
        let packets = read_bit(&data);
        for loss in [Loss::None, Loss::Fec] {
            let suf = if loss == Loss::Fec { ".fec" } else { "" };
            let ours = decode(&packets, channels, loss);
            for rate in [8usize, 12, 16] {
                let Ok(r) = std::fs::read(dir().join(format!("{name}.{rate}{suf}.pcm"))) else {
                    continue;
                };
                let r: Vec<i16> = r
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();
                let mut prev_exact = false;
                for (i, f) in ours.iter().enumerate() {
                    if f.fs_khz != rate {
                        prev_exact = false;
                        continue;
                    }
                    let start = (f.pos48 * rate / 48) as isize + delay(rate);
                    let n = f.pcm.len();
                    if start < 0 || start as usize * channels + n > r.len() {
                        continue;
                    }
                    let same = f
                        .pcm
                        .iter()
                        .zip(&r[start as usize * channels..])
                        .all(|(a, b)| q16(*a) == i32::from(*b));
                    let switch = ours.get(i + 1).is_some_and(|g| g.fs_khz != rate)
                        || (i > 0 && ours[i - 1].fs_khz != rate);
                    if loss == Loss::None {
                        frames += 1;
                        exact += usize::from(same);
                        if !same {
                            assert!(
                                switch,
                                "{name} frame {} at {rate} kHz differs from libopus",
                                f.pos48 / 960
                            );
                            switch_misses += 1;
                        }
                    } else if f.case == 3 && prev_exact {
                        lbrr += 1;
                        lbrr_exact += usize::from(same);
                    }
                    prev_exact = same;
                }
            }
        }
    }
    eprintln!(
        "{exact} of {frames} frames exact ({switch_misses} at rate switches); LBRR {lbrr_exact} of {lbrr}"
    );
    assert!(frames > 10_000);
    assert_eq!(lbrr, lbrr_exact);
    assert!(lbrr > 50);
    let levels = run(&[Loss::Conceal]);
    for (k, f, _, db, _) in levels.iter().filter(|e| {
        e.0.starts_with("level")
            && (e.0.ends_with("run 1") || e.0.ends_with("run 2") || e.0.ends_with("run 3"))
    }) {
        eprintln!("{k}: {db:+.2} dB over {f} frames");
        assert!(db.abs() < 3.0, "{k}: {db:.2} dB");
    }
}
