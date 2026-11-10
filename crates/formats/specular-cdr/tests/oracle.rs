//! CorelDRAW files of many versions (Sembiance's sample collection; tools/corpus/
//! fetch-oracle4.sh cdr) against a RIFF walk and zipfile in Python with Pillow decoding the
//! result: whether there is a preview, which one is shown first and its exact bytes (a DISP
//! chunk's DIB, an imhd BMP, a ZIP package's page or thumbnail member). Reads
//! `SPECULAR_ORACLE4_CORPUS`/cdr, else `$CARGO_TARGET_DIR/corpus/oracle4/cdr`; skips when
//! absent. Run with `cargo test -p specular-cdr --test oracle -- --ignored --nocapture`.

use std::path::PathBuf;

use specular_cdr::{ImageKind, parse};

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE4_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| PathBuf::from(t).join("corpus/oracle4"))
        })?;
    let dir = base.join("cdr");
    dir.join("expect.tsv").is_file().then_some(dir)
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xCBF2_9CE4_8422_2325, |h, &x| {
        (h ^ u64::from(x)).wrapping_mul(0x0100_0000_01B3)
    })
}

#[test]
#[ignore]
fn corpus_matches_python_and_pillow() {
    let Some(dir) = corpus() else {
        eprintln!("oracle4 cdr corpus missing: run tools/corpus/fetch-oracle4.sh cdr");
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    let mut failures = Vec::new();
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let name = f[0];
        let data = std::fs::read(dir.join("files").join(name)).unwrap();
        let ours = parse(&data);
        let r = match (f[1], &ours) {
            ("error", _) => {
                eprintln!("  {name}: oracle fails ({})", f[2]);
                println!("ORACLE\tCDR\t{name}\tskip");
                continue;
            }
            ("none", Err(_)) => Ok(()),
            ("none", Ok(c)) if c.previews.is_empty() => Ok(()),
            ("none", Ok(_)) => Err("a preview; the oracle finds none".to_owned()),
            (_, Err(e)) => Err(format!(
                "refused ({e}); the oracle finds a {} preview",
                f[1]
            )),
            (kind, Ok(c)) => match c.previews.first() {
                None => Err(format!("no preview; the oracle finds a {kind} one")),
                Some(p) => {
                    let bytes = p.data(&data);
                    // A DIB is given a 14-byte BMP file header here.
                    let payload = if kind == "dib" {
                        bytes.get(14..).unwrap_or_default()
                    } else {
                        bytes
                    };
                    let want_kind = if kind == "png" {
                        ImageKind::Png
                    } else {
                        ImageKind::Bmp
                    };
                    let (len, hash): (usize, u64) = (
                        f[2].parse().unwrap(),
                        u64::from_str_radix(f[3], 16).unwrap(),
                    );
                    if p.image != want_kind {
                        Err(format!("{:?} vs {kind}", p.image))
                    } else if (payload.len(), fnv(payload)) != (len, hash) {
                        Err(format!(
                            "{} preview bytes differ ({} vs {len})",
                            kind,
                            payload.len()
                        ))
                    } else {
                        Ok(())
                    }
                }
            },
        };
        println!(
            "ORACLE\tCDR\t{name}\t{}",
            if r.is_ok() { "pass" } else { "fail" }
        );
        if let Err(e) = r {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
