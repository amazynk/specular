//! Deluxe Paint art with colour cycling (Sembiance's ILBM collection; tools/corpus/
//! fetch-oracle-samples.sh iff) against FFmpeg's ILBM decoder (the still image) and the cycling
//! that the CRNG / CCRT chunks define, stepped tick by tick on a 1/60 s clock by oracle-samples.py:
//! each frame's tick and palette. Reads `SPECULAR_ORACLE_SAMPLES_CORPUS`/iff, else
//! `$CARGO_TARGET_DIR/corpus/oracle-samples/iff`; skips when absent. Run with `cargo test -p
//! specular-iff --test cycling_oracle -- --ignored --nocapture`.

use std::collections::BTreeMap;
use std::path::PathBuf;

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE_SAMPLES_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(|t| PathBuf::from(t).join("corpus/oracle-samples"))
        })?;
    let dir = base.join("iff");
    dir.join("expect.tsv").is_file().then_some(dir)
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xCBF2_9CE4_8422_2325, |h, &x| {
        (h ^ u64::from(x)).wrapping_mul(0x0100_0000_01B3)
    })
}

#[test]
#[ignore]
fn corpus_matches_ffmpeg_and_cycling_chunks() {
    let Some(dir) = corpus() else {
        eprintln!(
            "oracle-samples iff corpus missing: run tools/corpus/fetch-oracle-samples.sh iff"
        );
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    let mut per: BTreeMap<&str, Vec<Vec<&str>>> = BTreeMap::new();
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        per.entry(f[0]).or_default().push(f[1..].to_vec());
    }
    let mut failures = Vec::new();
    for (name, rows) in &per {
        let data = std::fs::read(dir.join("files").join(name)).unwrap();
        let mut errs = Vec::new();
        let mut cycled = false;
        for r in rows {
            let res = match r[0] {
                "image" => still(&dir, &data, r),
                _ if r[1] == "drng" || r[1] == "none" => Ok(()),
                _ => {
                    cycled = true;
                    cycling(&data, r)
                }
            };
            if let Err(e) = res {
                errs.push(format!("{name}: {e}"));
            }
        }
        let fmt = if cycled { "IFF/cycling" } else { "IFF" };
        let ok = if errs.is_empty() { "pass" } else { "fail" };
        println!("ORACLE\t{fmt}\t{name}\t{ok}");
        failures.extend(errs);
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

fn still(dir: &std::path::Path, data: &[u8], r: &[&str]) -> Result<(), String> {
    let (w, h): (u32, u32) = (r[1].parse().unwrap(), r[2].parse().unwrap());
    let img = specular_iff::decode(data).map_err(|e| e.to_string())?;
    if (img.width, img.height) != (w, h) {
        return Err(format!("{}x{} vs FFmpeg {w}x{h}", img.width, img.height));
    }
    let want = std::fs::read(dir.join("expect").join(r[3])).unwrap();
    // OCS-era palettes: FFmpeg keeps 0xA0 as is, ours widens it to 0xAA as the Amiga showed
    // it (Extra Half-Brite halves too, in 4-bit precision): compare at 4 bits, widened.
    let ocs = r[4] == "1";
    let mut bad = 0;
    for (i, (a, b)) in img
        .pixels
        .chunks_exact(4)
        .zip(want.chunks_exact(3))
        .enumerate()
    {
        let b: Vec<u8> = if ocs {
            b.iter().map(|&v| (v >> 4) * 17).collect()
        } else {
            b.to_vec()
        };
        if a[..3] != b[..] {
            if bad < 2 {
                eprintln!("    pixel {i}: ours {a:?}, FFmpeg {b:?}");
            }
            bad += 1;
        }
    }
    if bad > 0 {
        return Err(format!("{bad} of {} pixels differ from FFmpeg", w * h));
    }
    Ok(())
}

fn cycling(data: &[u8], r: &[&str]) -> Result<(), String> {
    let frames = specular_iff::cycling(data)
        .map_err(|e| e.to_string())?
        .ok_or("no cycling")?;
    let base: u64 = r[1].parse().unwrap();
    if fnv(&frames[0].cmap) != base {
        return Err("frame 0 is not the file's CMAP".into());
    }
    let events: Vec<(u64, u64)> = r[2]
        .split(' ')
        .map(|e| {
            let (t, h) = e.split_once(':').unwrap();
            (t.parse().unwrap(), u64::from_str_radix(h, 16).unwrap())
        })
        .collect();
    // Frame k + 1 starts on the tick where the oracle's k-th step falls; delays are whole
    // ticks in milliseconds rounded down, so ceil(ms * 60 / 1000) gives the ticks back.
    let mut tick = 0u64;
    for (k, pair) in frames.windows(2).enumerate() {
        tick += (u64::from(pair[0].delay_ms) * 60).div_ceil(1000);
        let Some(&(t, h)) = events.get(k) else {
            break;
        };
        if tick != t {
            return Err(format!("frame {}: tick {tick} vs {t}", k + 1));
        }
        if fnv(&pair[1].cmap) != h {
            return Err(format!("frame {} (tick {t}): palette differs", k + 1));
        }
    }
    Ok(())
}
