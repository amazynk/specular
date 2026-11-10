//! Aztec codes against zxing-cpp on generated pictures (compact and full-range symbols of
//! every size, runes, every character mode, binary, ECI and GS1; rotations, perspective,
//! noise, blur, small modules, mirrored, light on dark), made by tools/corpus/aztec-oracle.py
//! (not committed; zxing-cpp's writer makes the symbols):
//!
//!   SPECULAR_AZ_CORPUS=<dir> cargo test -p specular-codes --test aztec_oracle -- --ignored --nocapture
//!
//! A code read with the wrong text fails the test.

mod common;
use common::{pgm, timed};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

fn unhex(s: &str) -> String {
    let b: Vec<u8> = (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    String::from_utf8(b).unwrap()
}

#[test]
#[ignore]
fn decodes_aztec_like_zxing() {
    let Some(dir) = std::env::var_os("SPECULAR_AZ_CORPUS").map(PathBuf::from) else {
        eprintln!("SPECULAR_AZ_CORPUS not set");
        return;
    };
    let manifest = std::fs::read_to_string(dir.join("manifest.tsv")).unwrap();
    let zx: HashMap<String, bool> = std::fs::read_to_string(dir.join("zxing.tsv"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(a, b)| (a.to_string(), b == "1"))
        .collect();
    let (mut ours, mut theirs, mut total, mut wrong) = (0, 0, 0, 0);
    let mut by_kind: BTreeMap<String, [u32; 3]> = Default::default();
    let mut ms = 0f64;
    let mut slowest = (0f64, String::new());
    for line in manifest.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let (name, want, size, payload, kind) = (f[0], unhex(f[1]), f[2], f[3], f[4]);
        let g = pgm(&dir.join(name));
        let (codes, e) = timed(|| specular_codes::scan(&g));
        ms += e;
        if e > slowest.0 {
            slowest = (e, format!("{name} {}×{}", g.w, g.h));
        }
        let az: Vec<_> = codes
            .iter()
            .filter(|c| c.kind.starts_with("Aztec"))
            .collect();
        let ok = az.iter().any(|c| c.text == want);
        if !ok && !az.is_empty() {
            wrong += 1;
            println!(
                "WRONG {name} {size} {payload} {kind}: {:?}",
                az.iter()
                    .map(|c| c.text.chars().take(40).collect::<String>())
                    .collect::<Vec<_>>()
            );
        }
        let z = zx.get(name).copied().unwrap_or(false);
        total += 1;
        ours += u32::from(ok);
        theirs += u32::from(z);
        let keys = kind
            .split('+')
            .map(|k| k.trim_end_matches(|c: char| c.is_ascii_digit()).to_string())
            .chain([format!("[{payload}]")]);
        for k in keys {
            let e = by_kind.entry(k).or_default();
            e[0] += 1;
            e[1] += u32::from(ok);
            e[2] += u32::from(z);
        }
        if std::env::var_os("AZ_VERBOSE").is_some() && ok != z {
            println!("{name} {size} {payload} {kind}: ours {ok} zxing {z}");
        }
    }
    for (k, [n, o, z]) in &by_kind {
        println!("{k:10} {n:4}: ours {o:4}, zxing-cpp {z:4}");
    }
    println!(
        "ours {ours}/{total}, zxing-cpp {theirs}/{total}; {wrong} wrong; {:.1} ms CPU a picture, slowest {:.0} ms ({})",
        ms / f64::from(total),
        slowest.0,
        slowest.1
    );
    assert_eq!(wrong, 0);
}
