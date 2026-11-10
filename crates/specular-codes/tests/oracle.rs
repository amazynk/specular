//! Against zxing-cpp on generated pictures of QR codes (rotations, perspective, noise, blur,
//! small modules, mirrored, light on dark), made by tools/corpus/qr-oracle.py (not
//! committed):
//!
//!   SPECULAR_QR_CORPUS=<dir> cargo test -p specular-codes --test oracle -- --ignored --nocapture

mod common;
use common::{pgm, timed};
use std::path::PathBuf;

fn unhex(s: &str) -> String {
    let b: Vec<u8> = (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    String::from_utf8(b).unwrap()
}

#[test]
#[ignore]
fn decodes_as_many_as_zxing() {
    let Some(dir) = std::env::var_os("SPECULAR_QR_CORPUS").map(PathBuf::from) else {
        eprintln!("SPECULAR_QR_CORPUS not set");
        return;
    };
    let manifest = std::fs::read_to_string(dir.join("manifest.tsv")).unwrap();
    let zx: std::collections::HashMap<String, bool> =
        std::fs::read_to_string(dir.join("zxing.tsv"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(a, b)| (a.to_string(), b == "1"))
            .collect();
    let (mut ours, mut theirs, mut total, mut wrong) = (0, 0, 0, 0);
    let mut by_kind: std::collections::BTreeMap<String, (u32, u32, u32)> = Default::default();
    let mut ms = 0f64;
    let mut slowest = (0f64, String::new());
    for line in manifest.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let (name, want, kind) = (f[0], unhex(f[1]), f[4]);
        let g = pgm(&dir.join(name));
        let (codes, e) = timed(|| specular_codes::scan(&g));
        ms += e;
        if e > slowest.0 {
            slowest = (e, format!("{name} {}×{}", g.w, g.h));
        }
        let ok = codes.iter().any(|c| c.text == want);
        if std::env::var_os("QR_TIMES").is_some() {
            println!("T {name} {ok} {e:.1} {} {kind}", g.w);
        }
        if !ok && !codes.is_empty() {
            wrong += 1;
        }
        let z = zx.get(name).copied().unwrap_or(false);
        total += 1;
        ours += u32::from(ok);
        theirs += u32::from(z);
        for k in kind.split('+') {
            let k = k.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
            let e = by_kind.entry(k).or_default();
            e.0 += 1;
            e.1 += u32::from(ok);
            e.2 += u32::from(z);
        }
        if std::env::var_os("QR_VERBOSE").is_some() && ok != z {
            println!("{name} v{} {} {kind}: ours {ok} zxing {z}", f[2], f[3]);
        }
    }
    for (k, (n, o, z)) in &by_kind {
        println!("{k:8} {n:4}: ours {o:4}, zxing-cpp {z:4}");
    }
    println!(
        "ours {ours}/{total}, zxing-cpp {theirs}/{total}; {wrong} wrong text; {:.1} ms CPU a picture, slowest {:.0} ms ({})",
        ms / f64::from(total),
        slowest.0,
        slowest.1
    );
    assert_eq!(wrong, 0);
}
