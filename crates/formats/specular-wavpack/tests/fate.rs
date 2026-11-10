//! FATE's WavPack samples (FFmpeg's test suite) against the reference decoder wvunpack, on
//! the corpus of `tools/corpus/fetch-lossless.sh`. Cut ("partial") files are compared over
//! what both decoded; DSD (`dsd.wv`) is the `dsd` test's.
//! `SPECULAR_LOSSLESS_CORPUS=<dir> cargo test -p specular-wavpack --test fate -- --ignored --nocapture`

use std::path::PathBuf;

fn decode(d: &[u8]) -> Result<(Vec<u8>, usize), String> {
    let info = specular_wavpack::info(d).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut pcm = Vec::new();
    let mut muted = 0;
    for f in specular_wavpack::frames(d, usize::MAX) {
        pcm.clear();
        let r = specular_wavpack::decode_frame(
            &d[f.offset..f.offset + f.len],
            None,
            info.channels,
            &mut pcm,
        )
        .map_err(|e| e.to_string())?;
        muted += r.muted;
        for &v in &pcm {
            match info.bytes {
                1 => out.push((v + 128) as u8),
                2 => out.extend_from_slice(&(v as i16).to_le_bytes()),
                3 => out.extend_from_slice(&v.to_le_bytes()[..3]),
                _ => out.extend_from_slice(&v.to_le_bytes()),
            }
        }
    }
    Ok((out, muted))
}

#[test]
#[ignore = "needs the corpus from tools/corpus/fetch-lossless.sh"]
fn fate_samples_against_wvunpack() {
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
        if f.get(1) != Some(&"wavpack") || f.len() < 9 {
            continue;
        }
        let d = std::fs::read(dir.join(f[0])).unwrap();
        if specular_wavpack::info(&d).is_ok_and(|i| i.dsd.is_some()) {
            eprintln!("dsd  {} (the dsd test's)", f[0]);
            continue;
        }
        total += 1;
        let want = std::fs::read(dir.join(f[7])).unwrap();
        let cut = f[8] != "0";
        match decode(&d) {
            Err(e) => {
                refused += 1;
                eprintln!("REFUSED {}: {e}", f[0]);
            }
            Ok((got, muted)) => {
                let n = got.len().min(want.len());
                let same = got[..n] == want[..n] && (cut || got.len() == want.len());
                if same && n > 0 {
                    exact += 1;
                    let note = if cut { ", cut file" } else { "" };
                    eprintln!("ok   {} ({n} bytes{note}, {muted} muted)", f[0]);
                } else {
                    let at = got.iter().zip(&want).position(|(a, b)| a != b);
                    eprintln!(
                        "DIFF {}: {} vs {} bytes, first at {at:?}",
                        f[0],
                        got.len(),
                        want.len()
                    );
                    bad.push(f[0].to_string());
                }
            }
        }
    }
    eprintln!("FATE WavPack: {exact}/{total} bit-exact, {refused} refused");
    assert!(bad.is_empty(), "differ: {bad:?}");
}
