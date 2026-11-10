//! The CCSDS 121.0-B-2 test streams libaec ships (every option at 1 to 32 bits per sample,
//! restricted IDs, the low-entropy sets, the 32-bit SAR set with padded RSIs; tools/corpus/
//! fetch-oracle4.sh aec) against libaec's decode (through imagecodecs) and the CCSDS
//! reference data, both. Reads `SPECULAR_ORACLE4_CORPUS`/aec, else
//! `$CARGO_TARGET_DIR/corpus/oracle4/aec`; skips when absent. Run with
//! `cargo test -p specular-aec --test oracle -- --ignored --nocapture`.

use std::path::PathBuf;

use specular_aec::{Params, decode_bytes};

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE4_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| PathBuf::from(t).join("corpus/oracle4"))
        })?;
    let dir = base.join("aec");
    dir.join("expect.tsv").is_file().then_some(dir)
}

#[test]
#[ignore]
fn corpus_matches_libaec_and_ccsds() {
    let Some(dir) = corpus() else {
        eprintln!("oracle4 aec corpus missing: run tools/corpus/fetch-oracle4.sh aec");
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    let mut failures = Vec::new();
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let name = f[0];
        if f[1] == "error" {
            eprintln!("  {name}: libaec fails ({})", f[2]);
            println!("ORACLE\tAEC\t{name}\tskip");
            continue;
        }
        let num = |i: usize| f[i].parse::<u32>().unwrap();
        let p = Params {
            bits_per_sample: num(2),
            block_size: num(3),
            rsi: num(4),
            flags: num(5) as u8,
        };
        let count = num(6) as usize;
        let input = std::fs::read(dir.join("files").join(name)).unwrap();
        let reference = std::fs::read(dir.join("files").join(f[1])).unwrap();
        let libaec = std::fs::read(dir.join("expect").join(f[8])).unwrap();
        let r = match decode_bytes(&input, &p, count) {
            Err(e) => Err(format!("{name}: {e}")),
            Ok(out) if out != libaec => Err(format!(
                "{name}: differs from libaec at byte {:?}",
                out.iter().zip(&libaec).position(|(a, b)| a != b)
            )),
            Ok(out) if out[..reference.len().min(out.len())] != reference[..] => {
                Err(format!("{name}: differs from the CCSDS reference data"))
            }
            Ok(_) => Ok(()),
        };
        println!(
            "ORACLE\tAEC\t{name}\t{}",
            if r.is_ok() { "pass" } else { "fail" }
        );
        if let Err(e) = r {
            failures.push(e);
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
