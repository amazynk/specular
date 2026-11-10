//! Bit-exact comparison with the reference decoder (`mac -d`, Monkey's Audio 13.43) on the
//! corpus made by `tools/corpus/fetch-lossless.sh`:
//! `SPECULAR_LOSSLESS_CORPUS=<dir> cargo test -p specular-ape --test oracle -- --ignored --nocapture`

use std::path::PathBuf;

fn corpus() -> Option<PathBuf> {
    std::env::var_os("SPECULAR_LOSSLESS_CORPUS").map(|p| PathBuf::from(p).join("samples"))
}

/// The whole file decoded as the reference's WAVE data bytes (float files as their bits).
fn decode(d: &[u8]) -> Result<Vec<u8>, String> {
    let info = specular_ape::info(d).map_err(|e| e.to_string())?;
    let mut dec = specular_ape::Decoder::new(&info).map_err(|e| e.to_string())?;
    let mut pcm = Vec::new();
    let mut out = Vec::new();
    for (i, f) in info.frames.iter().enumerate() {
        pcm.clear();
        dec.decode(&d[f.offset..f.offset + f.len], f.skip, f.blocks, &mut pcm)
            .map_err(|e| format!("frame {i}: {e}"))?;
        for &v in &pcm {
            match info.bits {
                8 => out.push((v + 128) as u8),
                16 => out.extend_from_slice(&(v as i16).to_le_bytes()),
                24 => out.extend_from_slice(&v.to_le_bytes()[..3]),
                _ => out.extend_from_slice(&v.to_le_bytes()),
            }
        }
    }
    Ok(out)
}

#[test]
#[ignore = "needs the corpus from tools/corpus/fetch-lossless.sh"]
fn bit_exact_against_mac() {
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
        if f.get(1) != Some(&"ape") || f.len() > 7 {
            continue;
        }
        total += 1;
        let d = std::fs::read(dir.join(f[0])).unwrap();
        let want = std::fs::read(dir.join(format!("{}.ref", f[0]))).unwrap();
        let t = std::time::Instant::now();
        match decode(&d) {
            Ok(got) if got == want => {
                exact += 1;
                eprintln!("ok   {} ({:.1} ms)", f[0], t.elapsed().as_secs_f64() * 1e3);
            }
            Ok(got) => {
                let at = got.iter().zip(&want).position(|(a, b)| a != b);
                eprintln!(
                    "DIFF {}: {} vs {} bytes, first at {at:?}",
                    f[0],
                    got.len(),
                    want.len()
                );
                bad.push(f[0].to_string());
            }
            Err(e) => {
                eprintln!("ERR  {}: {e}", f[0]);
                bad.push(f[0].to_string());
            }
        }
    }
    eprintln!("Monkey's Audio bit-exact: {exact}/{total}");
    assert!(bad.is_empty(), "differ: {bad:?}");
}
