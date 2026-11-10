//! QCELP-13 against the system ffmpeg's `qcelp` decoder (a floating-point decoder, so the
//! comparison is SNR and largest difference, not bit-exact).
//!
//! - `fixture`: committed `tests/data/random.qcp` (packets built by
//!   `tools/corpus/qcelp-fixtures.py`) and ffmpeg 8.0.1's decode of it (`random.s16`).
//! - `corpus` (ignored): every QCELP `.qcp` under `$SPECULAR_WTV_CORPUS` (default
//!   `target/corpus/wtv1`, fetched by `tools/corpus/fetch-wtv-qcelp.sh`), decoded
//!   by ffmpeg at run time.

use std::path::{Path, PathBuf};

use specular_qcelp::{Decoder, QCP_GUIDS, packet_len};

/// The packets of a QCELP `.qcp` file (RIFF `QLCM`), or None for another codec.
fn qcp_packets(d: &[u8]) -> Option<Vec<&[u8]>> {
    if d.len() < 12 || &d[0..4] != b"RIFF" || &d[8..12] != b"QLCM" {
        return None;
    }
    let mut at = 12;
    let mut guid_ok = false;
    let mut data: &[u8] = &[];
    while at + 8 <= d.len() {
        let id = &d[at..at + 4];
        let len = u32::from_le_bytes(d[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &d[at + 8..(at + 8 + len).min(d.len())];
        if id == b"fmt " && body.len() >= 18 {
            guid_ok = QCP_GUIDS.iter().any(|g| g == &body[2..18]);
        }
        if id == b"data" {
            data = body;
        }
        at += 8 + len + (len & 1);
    }
    if !guid_ok {
        return None;
    }
    let mut out = Vec::new();
    let mut p = 0;
    while p < data.len() {
        let n = packet_len(data[p]).unwrap_or(1);
        out.push(&data[p..(p + n).min(data.len())]);
        p += n;
    }
    Some(out)
}

fn ours(packets: &[&[u8]]) -> Vec<f32> {
    let mut d = Decoder::new();
    let mut out = Vec::new();
    for p in packets {
        d.decode(p, &mut out);
    }
    out
}

/// (SNR dB, largest difference in 16-bit units, best gain, lag) of ours against the oracle.
fn compare(ours: &[f32], want: &[i16]) -> (f64, f64, f64, i64) {
    let a: Vec<f64> = ours.iter().map(|&v| f64::from(v) * 32768.0).collect();
    let b: Vec<f64> = want.iter().map(|&v| f64::from(v)).collect();
    let (a, b) = (&a, &b);
    let mut best = (f64::MIN, 0.0, 1.0, 0i64);
    for lag in -2i64..=2 {
        let pairs = || {
            (0..a.len()).filter_map(move |i| {
                let j = i as i64 + lag;
                (j >= 0 && (j as usize) < b.len()).then(|| (a[i], b[j as usize]))
            })
        };
        let (mut sig, mut err, mut ab, mut aa, mut maxd) = (0.0, 0.0, 0.0, 0.0, 0f64);
        for (x, y) in pairs() {
            sig += y * y;
            err += (x - y) * (x - y);
            ab += x * y;
            aa += x * x;
            maxd = maxd.max((x - y).abs());
        }
        let snr = 10.0 * (sig / err.max(1e-9)).log10();
        if snr > best.0 {
            best = (snr, maxd, if aa > 0.0 { ab / aa } else { 0.0 }, lag);
        }
    }
    best
}

fn ffmpeg_s16(path: &Path) -> Option<Vec<i16>> {
    let out = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args(["-f", "s16le", "-ac", "1", "-"])
        .output()
        .ok()?;
    out.status.success().then(|| {
        out.stdout
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect()
    })
}

#[test]
fn fixture() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let d = std::fs::read(dir.join("random.qcp")).unwrap();
    let want: Vec<i16> = std::fs::read(dir.join("random.s16"))
        .unwrap()
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    let packets = qcp_packets(&d).expect("QCELP qcp");
    let got = ours(&packets);
    assert_eq!(got.len(), packets.len() * 160);
    let (snr, maxd, gain, lag) = compare(&got, &want);
    eprintln!("random.qcp: SNR {snr:.1} dB, max diff {maxd:.0}, gain {gain:.3}, lag {lag}");
    assert_eq!(lag, 0);
    assert!(snr > 26.0, "SNR {snr:.1} dB");
}

fn corpus_dir() -> PathBuf {
    std::env::var_os("SPECULAR_WTV_CORPUS").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/corpus/wtv1"),
        PathBuf::from,
    )
}

#[test]
#[ignore = "needs the corpus (tools/corpus/fetch-wtv-qcelp.sh) and ffmpeg"]
fn corpus() {
    let root = corpus_dir();
    let mut files = Vec::new();
    for sub in ["qcp", "suite-qcp"] {
        if let Ok(rd) = std::fs::read_dir(root.join(sub)) {
            files.extend(rd.flatten().map(|e| e.path()));
        }
    }
    files.sort();
    let mut worst = f64::MAX;
    let mut n = 0;
    for f in files {
        let d = std::fs::read(&f).unwrap();
        let Some(packets) = qcp_packets(&d) else {
            continue;
        };
        let Some(want) = ffmpeg_s16(&f) else {
            eprintln!("{}: ffmpeg failed", f.display());
            continue;
        };
        let t = std::time::Instant::now();
        let got = ours(&packets);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let (snr, maxd, gain, lag) = compare(&got, &want);
        if let Some(dir) = std::env::var_os("QCELP_DUMP") {
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            let b: Vec<u8> = got
                .iter()
                .flat_map(|v| ((v * 32768.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes())
                .collect();
            std::fs::write(Path::new(&dir).join(format!("{name}.ours")), b).unwrap();
            let b: Vec<u8> = want.iter().flat_map(|v| v.to_le_bytes()).collect();
            std::fs::write(Path::new(&dir).join(format!("{name}.ff")), b).unwrap();
        }
        eprintln!(
            "{}: {} frames ({} vs {} samples), SNR {snr:.1} dB, max diff {maxd:.0}, gain {gain:.3}, lag {lag}, {ms:.1} ms",
            f.file_name().unwrap().to_string_lossy(),
            packets.len(),
            got.len(),
            want.len()
        );
        worst = worst.min(snr);
        n += 1;
    }
    eprintln!("{n} files, worst SNR {worst:.1} dB");
    assert!(n > 0);
    assert!(worst > 15.0, "worst SNR {worst:.1} dB");
}
