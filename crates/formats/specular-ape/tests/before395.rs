//! Monkey's Audio files from versions 3.80–3.94 (FFmpeg's FATE `luckynight-mac380-c2000.ape`
//! … `mac394b1-c4000.ape`, the normal and extra high levels; tools/corpus/fetch-ape-old.sh)
//! against the reference decoder (`mac -d`, Monkey's Audio 13.43, BSD, built from the SDK):
//! every whole frame `mac` decodes must come out bit-exact. The files are cut, so the frame
//! the cut runs through fails its checksum in both and the comparison ends there; ffmpeg's
//! decode agrees with `mac` over the same samples (checked by the fetch script).
//! `SPECULAR_APE_OLD=<dir> cargo test -p specular-ape --test before395 -- --nocapture`

use std::path::PathBuf;

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
fn old_versions_match_mac() {
    let Some(dir) = std::env::var_os("SPECULAR_APE_OLD").map(PathBuf::from) else {
        eprintln!("skipped: set SPECULAR_APE_OLD (tools/corpus/fetch-ape-old.sh)");
        return;
    };
    let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("luckynight-"))
                && p.extension().is_some_and(|e| e == "ape")
        })
        .collect();
    names.sort();
    let mut bad = Vec::new();
    for path in &names {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let Ok(w) = std::fs::read(path.with_extension("mac.wav")) else {
            continue;
        };
        let want = wav_data(&w);
        let d = std::fs::read(path).unwrap();
        let info = specular_ape::info(&d).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut dec = specular_ape::Decoder::new(&info).unwrap();
        let (mut got, mut pcm, mut frames) = (Vec::new(), Vec::new(), 0);
        let mut first_err = None;
        for f in &info.frames {
            pcm.clear();
            match dec.decode(&d[f.offset..f.offset + f.len], f.skip, f.blocks, &mut pcm) {
                Ok(()) => frames += 1,
                Err(e) => {
                    first_err = Some(e);
                    break;
                }
            }
            for &v in &pcm {
                got.extend_from_slice(&(v as i16).to_le_bytes());
            }
        }
        // Where we decode a frame past mac's last (mac reads ahead past the cut and gives
        // up), it must match ffmpeg's decode (NAME.ff.raw).
        let ff = std::fs::read(path.with_extension("ff.raw")).unwrap_or_default();
        let same = got.len() >= want.len()
            && got[..want.len()] == *want
            && (got.len() == want.len() || ff.get(..got.len()) == Some(&got[..]));
        let mismatch = got.iter().zip(want).filter(|(a, b)| a != b).count();
        eprintln!(
            "{name}: version {} level {}, {frames} frames ({} bytes, mac {}), {mismatch} bytes differ{}",
            info.version,
            info.level,
            got.len(),
            want.len(),
            first_err
                .map(|e| format!(", stopped: {e}"))
                .unwrap_or_default()
        );
        if !same {
            bad.push(name);
        }
    }
    assert!(!names.is_empty(), "no luckynight-mac3*.ape in the corpus");
    assert!(bad.is_empty(), "{bad:?}");
}
