//! Bit-exact comparison with the reference decoder (wvunpack 5.8.1) on the corpus made by
//! `tools/corpus/fetch-lossless.sh`:
//! `SPECULAR_LOSSLESS_CORPUS=<dir> cargo test -p specular-wavpack --test oracle -- --ignored --nocapture`

use std::collections::HashMap;
use std::path::PathBuf;

fn corpus() -> Option<PathBuf> {
    std::env::var_os("SPECULAR_LOSSLESS_CORPUS").map(|p| PathBuf::from(p).join("samples"))
}

/// The whole file decoded (with its correction file when given), as the reference's bytes.
fn decode(d: &[u8], wvc: Option<&[u8]>) -> Result<(Vec<u8>, usize), String> {
    let info = specular_wavpack::info(d).map_err(|e| e.to_string())?;
    let cframes: HashMap<u64, specular_wavpack::Frame> = wvc
        .map(|c| {
            specular_wavpack::frames(c, usize::MAX)
                .into_iter()
                .map(|f| (f.index, f))
                .collect()
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    let mut pcm = Vec::new();
    let mut failed = 0;
    for f in specular_wavpack::frames(d, usize::MAX) {
        let frame = &d[f.offset..f.offset + f.len];
        let cf = wvc.and_then(|c| {
            cframes
                .get(&f.index)
                .map(|g| &c[g.offset..g.offset + g.len])
        });
        pcm.clear();
        match specular_wavpack::decode_frame(frame, cf, info.channels, &mut pcm) {
            Ok(r) => failed += r.muted,
            Err(e) => return Err(e.to_string()),
        }
        for &v in &pcm {
            match info.bytes {
                1 => out.push((v + 128) as u8),
                2 => out.extend_from_slice(&(v as i16).to_le_bytes()),
                3 => out.extend_from_slice(&v.to_le_bytes()[..3]),
                _ => out.extend_from_slice(&v.to_le_bytes()),
            }
        }
    }
    Ok((out, failed))
}

#[test]
#[ignore = "needs the corpus from tools/corpus/fetch-lossless.sh"]
fn bit_exact_against_wvunpack() {
    let Some(dir) = corpus() else {
        eprintln!("SPECULAR_LOSSLESS_CORPUS not set");
        return;
    };
    let manifest = std::fs::read_to_string(dir.join("manifest.tsv")).unwrap();
    let (mut total, mut exact) = (0, 0);
    let mut bad = Vec::new();
    for line in manifest.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        // FATE rows (nine fields) are the `fate` test's.
        if f.get(1) != Some(&"wavpack") || f.len() > 7 {
            continue;
        }
        let d = std::fs::read(dir.join(f[0])).unwrap();
        let wvc = (f[6] != "-").then(|| std::fs::read(dir.join(f[6])).unwrap());
        let mut runs = vec![(None, format!("{}.ref", f[0]))];
        if wvc.is_some() {
            runs.push((wvc.as_deref(), format!("{}.wvc.ref", f[0])));
        }
        for (c, refname) in runs {
            total += 1;
            let want = std::fs::read(dir.join(&refname)).unwrap();
            let t = std::time::Instant::now();
            match decode(&d, c) {
                Ok((got, failed)) => {
                    let ms = t.elapsed().as_secs_f64() * 1e3;
                    if got == want {
                        exact += 1;
                        eprintln!("ok   {refname} ({ms:.1} ms, {failed} muted blocks)");
                    } else {
                        let at = got.iter().zip(&want).position(|(a, b)| a != b);
                        eprintln!(
                            "DIFF {refname}: {} vs {} bytes, first at {at:?}, {failed} muted",
                            got.len(),
                            want.len()
                        );
                        bad.push(refname);
                    }
                }
                Err(e) => {
                    eprintln!("ERR  {refname}: {e}");
                    bad.push(refname);
                }
            }
        }
    }
    eprintln!("WavPack bit-exact: {exact}/{total}");
    assert!(bad.is_empty(), "differ: {bad:?}");
}
