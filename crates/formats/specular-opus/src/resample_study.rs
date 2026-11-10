//! A study of the SILK upsampler against the reference decoder (tests only): how far our
//! output is from the RFC test vectors' decodes in SILK-only streams, and the correction
//! filter that best maps one to the other (least squares). Black-box: only the reference's
//! decoded output is used.
//!
//! `cargo test -p specular-opus --lib resample_study -- --ignored --nocapture`
//! (needs tools/corpus/specular-opus-fetch.sh).

use std::path::Path;

use crate::resample::study::{KERNEL, kaiser};
use crate::{Decoder, Head};

fn corpus() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/video2/specular-opus")
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

/// Our decode and the reference's of vector `n` (stereo, 48 kHz).
pub(crate) fn decode_vector(n: u32) -> Option<(Vec<f64>, Vec<f64>)> {
    let dir = corpus();
    let bit = std::fs::read(dir.join(format!("testvector{n:02}.bit"))).ok()?;
    let dec = std::fs::read(dir.join(format!("testvector{n:02}.dec"))).ok()?;
    let head = Head::parse(&[
        b'O', b'p', b'u', b's', b'H', b'e', b'a', b'd', 1, 2, 0, 0, 0x80, 0xbb, 0, 0, 0, 0, 0,
    ])
    .unwrap();
    let mut d = Decoder::new(&head).unwrap();
    let mut pcm = Vec::new();
    for p in read_bit(&bit) {
        let _ = d.decode(&p, &mut pcm);
    }
    let ours: Vec<f64> = pcm.iter().map(|&v| f64::from(v)).collect();
    let r: Vec<f64> = dec
        .chunks_exact(2)
        .map(|c| f64::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
        .collect();
    let n = ours.len().min(r.len());
    Some((ours[..n].to_vec(), r[..n].to_vec()))
}

pub(crate) fn snr(r: &[f64], t: &[f64]) -> f64 {
    let s: f64 = r.iter().map(|a| a * a).sum();
    let e: f64 = r.iter().zip(t).map(|(a, b)| (a - b) * (a - b)).sum();
    10.0 * (s / e.max(1e-30)).log10()
}

/// Solves `a x = b` (a symmetric positive definite, `n`×`n`) by Gaussian elimination.
fn solve(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Vec<f64> {
    for c in 0..n {
        let piv = (c..n)
            .max_by(|&i, &j| a[i * n + c].abs().total_cmp(&a[j * n + c].abs()))
            .unwrap();
        if piv != c {
            for k in 0..n {
                a.swap(c * n + k, piv * n + k);
            }
            b.swap(c, piv);
        }
        let d = a[c * n + c];
        for i in c + 1..n {
            let f = a[i * n + c] / d;
            if f != 0.0 {
                for k in c..n {
                    a[i * n + k] -= f * a[c * n + k];
                }
                b[i] -= f * b[c];
            }
        }
    }
    let mut x = vec![0.0; n];
    for c in (0..n).rev() {
        let mut s = b[c];
        for k in c + 1..n {
            s -= a[c * n + k] * x[k];
        }
        x[c] = s / a[c * n + c];
    }
    x
}

/// The filter `g` (taps `k`, centred at `d`) that best maps our stereo output to the
/// reference's: `ref[t] ≈ Σ g[j] ours[t - j + d]` on each channel.
///
/// `ridge` pulls the correction toward none below `low_hz` (a virtual training signal of
/// noise below `low_hz` that must pass unchanged, `ridge` times the vector's energy;
/// `low_hz` 24000 is a plain ridge).
fn fit(ours: &[f64], r: &[f64], k: usize, d: usize, ridge: f64, low_hz: f64) -> Vec<f64> {
    let n = ours.len() / 2;
    let mut a = vec![0.0; k * k];
    let mut b = vec![0.0; k];
    // Toeplitz from the autocorrelation (edges ignored).
    let mut ac = vec![0.0; k];
    for c in 0..2 {
        let x: Vec<f64> = (0..n).map(|i| ours[2 * i + c]).collect();
        let y: Vec<f64> = (0..n).map(|i| r[2 * i + c]).collect();
        for (lag, v) in ac.iter_mut().enumerate() {
            *v += (lag..n).map(|t| x[t] * x[t - lag]).sum::<f64>();
        }
        for (j, v) in b.iter_mut().enumerate() {
            // ours index t - j + d
            *v += (k..n - k).map(|t| x[t + d - j] * y[t]).sum::<f64>();
        }
    }
    for i in 0..k {
        for j in 0..k {
            a[i * k + j] = ac[i.abs_diff(j)];
        }
        a[i * k + i] *= 1.0 + 1e-9;
    }
    // Autocorrelation of white noise low-passed at `low_hz`, unit power.
    let fc = (low_hz / 48_000.0).min(0.5);
    let rl = |m: usize| {
        if m == 0 {
            1.0
        } else {
            let x = std::f64::consts::PI * 2.0 * fc * m as f64;
            x.sin() / x
        }
    };
    for i in 0..k {
        for j in 0..k {
            a[i * k + j] += ridge * ac[0] * rl(i.abs_diff(j));
        }
        b[i] += ridge * ac[0] * rl(i.abs_diff(d));
    }
    solve(a, b, k)
}

fn apply(ours: &[f64], g: &[f64], d: usize) -> Vec<f64> {
    let n = ours.len() / 2;
    let mut out = vec![0.0; ours.len()];
    for c in 0..2 {
        for t in 0..n {
            let mut s = 0.0;
            for (j, gj) in g.iter().enumerate() {
                let i = t as i64 - j as i64 + d as i64;
                if i >= 0 && (i as usize) < n {
                    s += gj * ours[2 * i as usize + c];
                }
            }
            out[2 * t + c] = s;
        }
    }
    out
}

/// `fit` with its band below `hz` replaced by `base`'s: `fit + lp * (base - fit)`, `lp` a
/// zero-phase Kaiser-windowed sinc low-pass of half-width `half` (48 kHz samples); what
/// would fall before time 0 is dropped (its energy printed).
fn keep_low(fit: &[f64], base: &[f64], hz: f64, half: usize) -> Vec<f64> {
    let n = fit.len().max(base.len());
    let diff: Vec<f64> = (0..n)
        .map(|i| base.get(i).copied().unwrap_or(0.0) - fit.get(i).copied().unwrap_or(0.0))
        .collect();
    let fc = hz / 48_000.0;
    let beta = 8.0f64;
    let i0 = |x: f64| {
        let (mut sum, mut term, h) = (1.0, 1.0, x * x / 4.0);
        for k in 1..60 {
            term *= h / (k * k) as f64;
            sum += term;
        }
        sum
    };
    let mut lp: Vec<f64> = (0..=2 * half)
        .map(|i| {
            let t = i as f64 - half as f64;
            let s = if t == 0.0 {
                2.0 * fc
            } else {
                (2.0 * std::f64::consts::PI * fc * t).sin() / (std::f64::consts::PI * t)
            };
            let r = t / half as f64;
            s * i0(beta * (1.0 - r * r).max(0.0).sqrt()) / i0(beta)
        })
        .collect();
    let sum: f64 = lp.iter().sum();
    lp.iter_mut().for_each(|v| *v /= sum);
    let mut out = vec![0.0; n + half];
    out[..fit.len()].copy_from_slice(fit);
    let mut lost = 0.0;
    for (i, d) in diff.iter().enumerate() {
        for (j, l) in lp.iter().enumerate() {
            let at = i as i64 + j as i64 - half as i64;
            if at >= 0 {
                out[at as usize] += d * l;
            } else {
                lost += (d * l).powi(2);
            }
        }
    }
    eprintln!("  low band kept below {hz} Hz; {lost:.2e} dropped before time 0");
    out
}

/// Our decode and libopus's of every `wbNN.opus` in `dir` (tools/corpus/specular-opus-wb.sh),
/// joined, each channel twice (the layout [`fit`] takes); only SILK wideband packets.
fn decode_corpus(dir: &Path) -> (Vec<f64>, Vec<f64>) {
    let (mut ours, mut r) = (Vec::new(), Vec::new());
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    for path in names
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "opus"))
    {
        let packets = crate::surround_study::ogg_packets(&std::fs::read(path).unwrap());
        let head = Head::parse(&packets[0]).unwrap();
        let mut d = Decoder::new(&head).unwrap();
        let mut pcm = Vec::new();
        let mut other = 0;
        for p in &packets[2..] {
            let toc = crate::packet::Toc::parse(p[0]);
            if toc.mode != crate::packet::Mode::Silk
                || toc.bandwidth != crate::packet::Bandwidth::Wide
            {
                other += 1;
            }
            d.decode(p, &mut pcm).unwrap();
        }
        if other > 0 {
            eprintln!(
                "  {:?}: skipped, {other} packets not SILK wideband",
                path.file_name().unwrap()
            );
            continue;
        }
        let pcm = &pcm[usize::from(head.pre_skip)..];
        let want = std::fs::read(path.with_extension("pcm")).unwrap();
        let want: Vec<f64> = want
            .chunks_exact(2)
            .map(|c| f64::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
            .collect();
        let n = want.len().min(pcm.len());
        eprintln!(
            "  {:?}: {:.2} dB before the fit",
            path.file_name().unwrap(),
            snr(
                &want[..n],
                &pcm[..n].iter().map(|&v| f64::from(v)).collect::<Vec<_>>()
            )
        );
        for i in 0..n {
            ours.extend([f64::from(pcm[i]); 2]);
            r.extend([want[i]; 2]);
        }
    }
    (ours, r)
}

/// Decodes with these kernels in place of the measured ones (by input rate in kHz).
pub(crate) fn with_kernels<T>(kernels: &[(usize, Vec<f32>)], f: impl FnOnce() -> T) -> T {
    let old = KERNEL.with(|k| std::mem::replace(&mut *k.borrow_mut(), kernels.to_vec()));
    let out = f();
    KERNEL.with(|k| *k.borrow_mut() = old);
    out
}

fn env(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Fits the corrected kernels on vectors 2–4 (correction `SPECULAR_OPUS_FIT_TAPS` taps,
/// default 401, ridge toward no correction `SPECULAR_OPUS_FIT_RIDGE`, default 0), then
/// reports every SILK and hybrid vector and the 7.1 fixture with them.
#[test]
#[ignore = "a study aid; needs the RFC test vectors"]
fn resample_study() {
    let out_dir = std::env::var_os("SPECULAR_OPUS_FIT_OUT");
    let k = env("SPECULAR_OPUS_FIT_TAPS", 401.0) as usize | 1;
    let ridge = env("SPECULAR_OPUS_FIT_RIDGE", 0.0);
    let d = k / 2;
    let mut table = String::from(
        "#![allow(clippy::excessive_precision)] // printed as measured, rounded to f32 on use\n\n",
    );
    let mut kernels = Vec::new();
    let wb = env("SPECULAR_OPUS_FIT_WB_VECTOR", 4.0) as u32;
    for (n, fs) in [(2u32, 8usize), (3, 12), (wb, 16)] {
        let base = kaiser(fs);
        let wb_corpus = std::env::var_os("SPECULAR_OPUS_WB_CORPUS").filter(|_| fs == 16);
        let got = with_kernels(&[(fs, base.clone())], || match &wb_corpus {
            Some(dir) => Some(decode_corpus(Path::new(dir))),
            None => decode_vector(n),
        });
        let Some((ours, r)) = got else {
            eprintln!("skipped: run tools/corpus/specular-opus-fetch.sh");
            return;
        };
        // Only the wideband kernel is regularised (see the module notes).
        let (ridge, low) = if fs == 16 {
            (ridge, env("SPECULAR_OPUS_FIT_LOW_HZ", 24_000.0))
        } else {
            (0.0, 24_000.0)
        };
        let g = fit(&ours, &r, k, d, ridge, low);
        let fixed = apply(&ours, &g, d);
        eprintln!(
            "vector {n} ({fs} kHz): SNR {:.2} dB, with the fitted correction {:.2} dB",
            snr(&r, &ours),
            snr(&r, &fixed)
        );
        // The corrected upsampler: g after ours, causal part kept.
        let h: Vec<f64> = base.iter().map(|&v| f64::from(v)).collect();
        let mut hn = vec![0.0; h.len() + k];
        let mut lost = 0.0;
        let mut total = 0.0;
        for (m, hm) in h.iter().enumerate() {
            for (j, gj) in g.iter().enumerate() {
                let at = m as i64 + j as i64 - d as i64;
                let v = hm * gj;
                total += v * v;
                if at >= 0 && (at as usize) < hn.len() {
                    hn[at as usize] += v;
                } else {
                    lost += v * v;
                }
            }
        }
        // Below `SPECULAR_OPUS_FIT_FLAT_HZ` (0: off) the sinc is kept: the correction's low
        // band follows the vector's content (our SILK synthesis against the reference's),
        // not the reference's upsampler.
        let flat = env("SPECULAR_OPUS_FIT_FLAT_HZ", 0.0);
        if flat > 0.0 {
            hn = keep_low(
                &hn,
                &h,
                flat,
                env("SPECULAR_OPUS_FIT_FLAT_HALF", 96.0) as usize,
            );
        }
        while hn.last().is_some_and(|v| v.abs() < 1e-7) {
            hn.pop();
        }
        eprintln!(
            "  kernel {} taps at 48 kHz, {:.2e} of its energy before time 0",
            hn.len(),
            lost / total
        );
        table.push_str(&format!(
            "pub(crate) const FIT_{fs}: [f32; {}] = [\n",
            hn.len()
        ));
        for c in hn.chunks(6) {
            let row: Vec<String> = c.iter().map(|v| format!("{:.9e}", *v as f32)).collect();
            table.push_str(&format!("    {},\n", row.join(", ")));
        }
        table.push_str("];\n");
        kernels.push((fs, hn.iter().map(|&v| v as f32).collect::<Vec<f32>>()));
    }
    let base: Vec<(usize, Vec<f32>)> = [8, 12, 16].iter().map(|&fs| (fs, kaiser(fs))).collect();
    with_kernels(&base, || {
        if let Some((per, _)) = crate::surround_study::study() {
            let all: Vec<String> = per.iter().map(|p| format!("{:.1}", p.0)).collect();
            eprintln!(
                "check: centre gain {:+.4} dB delay {:+.4}, LFE gain {:+.4} dB delay {:+.4}",
                per[2].1, per[2].2, per[3].1, per[3].2
            );
            eprintln!("check: 7.1 channels with the Kaiser sinc {}", all.join(" "));
        }
    });
    with_kernels(&kernels, || {
        for n in [2, 3, 4, 5, 6, 12] {
            let (ours, r) = decode_vector(n).unwrap();
            eprintln!("check: vector {n}: {:.2} dB", snr(&r, &ours));
        }
        if let Some((per, _)) = crate::surround_study::study() {
            let all: Vec<String> = per.iter().map(|p| format!("{:.1}", p.0)).collect();
            eprintln!(
                "check: centre gain {:+.4} dB delay {:+.4}, LFE gain {:+.4} dB delay {:+.4}",
                per[2].1, per[2].2, per[3].1, per[3].2
            );
            eprintln!("check: 7.1 channels {}", all.join(" "));
        }
    });
    if let Some(dir) = out_dir {
        std::fs::write(Path::new(&dir).join("resample_fit.rs"), table).unwrap();
    }
}
