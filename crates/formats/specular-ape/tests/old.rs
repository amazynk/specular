//! Monkey's Audio files from versions 3.95–3.98 (file versions 3950–3989), made by the
//! encoders of the time and found in public archives (`tools/corpus/fetch-old-ape.py`),
//! against `mac -d` 13.43: whole files bit-exact (cut files over the frames both decode).
//! `SPECULAR_LOSSLESS_CORPUS=<dir> cargo test -p specular-ape --test old -- --ignored --nocapture`

use std::path::PathBuf;
use std::time::Instant;

/// The WAVE file's `data` chunk.
fn wav_data(w: &[u8]) -> &[u8] {
    let mut at = 12;
    while at + 8 <= w.len() {
        let len = u32::from_le_bytes([w[at + 4], w[at + 5], w[at + 6], w[at + 7]]) as usize;
        if &w[at..at + 4] == b"data" {
            return &w[at + 8..(at + 8 + len).min(w.len())];
        }
        at += 8 + len + (len & 1);
    }
    &[]
}

#[test]
#[ignore = "needs the corpus from tools/corpus/fetch-old-ape.py"]
fn old_versions_against_mac() {
    let Some(dir) = std::env::var_os("SPECULAR_LOSSLESS_CORPUS")
        .map(|p| PathBuf::from(p).join("samples").join("old-ape"))
    else {
        eprintln!("SPECULAR_LOSSLESS_CORPUS not set");
        return;
    };
    let manifest = std::fs::read_to_string(dir.join("manifest.tsv")).unwrap();
    let (mut total, mut exact) = (0, 0);
    let mut bad = Vec::new();
    for line in manifest.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let cut = f.get(2) == Some(&"cut");
        total += 1;
        let d = std::fs::read(dir.join(f[0])).unwrap();
        let w = std::fs::read(dir.join(f[1])).unwrap();
        let want = wav_data(&w);
        let info = specular_ape::info(&d).unwrap();
        let mut dec = specular_ape::Decoder::new(&info).unwrap();
        let mut pcm = Vec::new();
        let mut got = Vec::new();
        let mut failed = 0;
        let t = Instant::now();
        for fr in &info.frames {
            pcm.clear();
            if dec
                .decode(
                    &d[fr.offset..fr.offset + fr.len],
                    fr.skip,
                    fr.blocks,
                    &mut pcm,
                )
                .is_err()
            {
                failed += 1;
                if cut {
                    break;
                }
                pcm.resize(fr.blocks as usize * usize::from(info.channels), 0);
            }
            for &v in &pcm {
                match info.bits {
                    8 => got.push((v + 128) as u8),
                    16 => got.extend_from_slice(&(v as i16).to_le_bytes()),
                    24 => got.extend_from_slice(&v.to_le_bytes()[..3]),
                    _ => got.extend_from_slice(&v.to_le_bytes()),
                }
            }
        }
        let secs = t.elapsed().as_secs_f64();
        let n = if cut {
            got.len().min(want.len())
        } else {
            got.len()
        };
        let same = n > 0 && got[..n] == want[..n] && (cut || got.len() == want.len());
        let audio_s = want.len() as f64
            / (f64::from(info.channels) * f64::from(info.bits / 8) * f64::from(info.sample_rate));
        eprintln!(
            "{} {}: version {}, level {}, {} bit, {} ch, {} frames ({failed} failed), {:.1} s of \
             sound in {:.0} ms ({:.0}× real time)",
            if same { "ok  " } else { "DIFF" },
            f[0],
            info.version,
            info.level,
            info.bits,
            info.channels,
            info.frames.len(),
            audio_s,
            secs * 1e3,
            audio_s / secs
        );
        if same {
            exact += 1;
        } else {
            let at = got.iter().zip(want).position(|(a, b)| a != b);
            eprintln!(
                "     {} vs {} bytes, first difference at {at:?}",
                got.len(),
                want.len()
            );
            bad.push(f[0].to_owned());
        }
    }
    eprintln!("Monkey's Audio 3950–3989: {exact}/{total} bit-exact");
    assert!(bad.is_empty(), "differ: {bad:?}");
}
