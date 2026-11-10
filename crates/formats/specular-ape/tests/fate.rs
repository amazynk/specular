//! FATE's Monkey's Audio samples (FFmpeg's test suite) against `mac -d`, on the corpus of
//! `tools/corpus/fetch-lossless.sh`. Cut files are compared over the frames both decoded;
//! files from before version 3.80 are refused by name.
//! `SPECULAR_LOSSLESS_CORPUS=<dir> cargo test -p specular-ape --test fate -- --ignored --nocapture`

use std::path::PathBuf;

/// The frames decoded until the first that fails, as WAVE data bytes.
fn decode(d: &[u8]) -> Result<(Vec<u8>, usize), String> {
    let info = specular_ape::info(d).map_err(|e| e.to_string())?;
    let mut dec = specular_ape::Decoder::new(&info).map_err(|e| e.to_string())?;
    let mut pcm = Vec::new();
    let mut out = Vec::new();
    let mut ok = 0;
    for f in &info.frames {
        pcm.clear();
        if dec
            .decode(&d[f.offset..f.offset + f.len], f.skip, f.blocks, &mut pcm)
            .is_err()
        {
            break;
        }
        ok += 1;
        for &v in &pcm {
            match info.bits {
                8 => out.push((v + 128) as u8),
                16 => out.extend_from_slice(&(v as i16).to_le_bytes()),
                24 => out.extend_from_slice(&v.to_le_bytes()[..3]),
                _ => out.extend_from_slice(&v.to_le_bytes()),
            }
        }
    }
    Ok((out, ok))
}

#[test]
#[ignore = "needs the corpus from tools/corpus/fetch-lossless.sh"]
fn fate_samples_against_mac() {
    let Some(dir) =
        std::env::var_os("SPECULAR_LOSSLESS_CORPUS").map(|p| PathBuf::from(p).join("samples"))
    else {
        return;
    };
    let manifest = std::fs::read_to_string(dir.join("manifest.tsv")).unwrap();
    let (mut total, mut exact, mut refused) = (0, 0, 0);
    let mut bad = Vec::new();
    for line in manifest.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.get(1) != Some(&"ape") || f.len() < 9 {
            continue;
        }
        total += 1;
        let d = std::fs::read(dir.join(f[0])).unwrap();
        let want = std::fs::read(dir.join(f[7])).unwrap();
        match decode(&d) {
            Err(e) => {
                refused += 1;
                eprintln!("REFUSED {}: {e}", f[0]);
            }
            Ok((got, frames)) => {
                let n = got.len().min(want.len());
                if got[..n] == want[..n] {
                    exact += 1;
                    eprintln!(
                        "ok   {} ({frames} frames, {} bytes ours, {} the reference's)",
                        f[0],
                        got.len(),
                        want.len()
                    );
                } else {
                    let at = got.iter().zip(&want).position(|(a, b)| a != b);
                    eprintln!("DIFF {}: first at {at:?}", f[0]);
                    bad.push(f[0].to_string());
                }
            }
        }
    }
    eprintln!("FATE Monkey's Audio: {exact}/{total} agree, {refused} refused");
    assert!(bad.is_empty(), "differ: {bad:?}");
}
