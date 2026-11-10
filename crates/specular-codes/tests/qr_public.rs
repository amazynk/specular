//! QR codes in real-world photos (the ZXing project's black-box test pictures, Apache-2.0)
//! against zxing-cpp, fetched by tools/corpus/qr-public.py (not committed):
//!
//!   SPECULAR_QR_PUBLIC_CORPUS=<dir> cargo test -p specular-codes --test qr_public -- --ignored --nocapture
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
fn reads_real_photos_like_zxing() {
    let Some(dir) = std::env::var_os("SPECULAR_QR_PUBLIC_CORPUS").map(PathBuf::from) else {
        eprintln!("SPECULAR_QR_PUBLIC_CORPUS not set");
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
    let mut by_set: BTreeMap<String, [u32; 3]> = Default::default();
    let mut ms = 0f64;
    for line in manifest.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let (name, want, set) = (f[0], unhex(f[1]), f[2]);
        let g = pgm(&dir.join(name));
        let (codes, e) = timed(|| specular_codes::scan(&g));
        ms += e;
        let qr: Vec<_> = codes.iter().filter(|c| c.kind == "QR").collect();
        let ok = qr.iter().any(|c| c.text == want);
        if !ok {
            println!("MISS {name}");
        }
        // A photo may hold other codes too; a QR code read with other text is wrong.
        if !ok && !qr.is_empty() {
            wrong += 1;
            println!(
                "WRONG {name}: {:?}",
                qr.iter().map(|c| &c.text).collect::<Vec<_>>()
            );
        }
        let z = zx.get(name).copied().unwrap_or(false);
        total += 1;
        ours += u32::from(ok);
        theirs += u32::from(z);
        let e = by_set.entry(set.to_string()).or_default();
        e[0] += 1;
        e[1] += u32::from(ok);
        e[2] += u32::from(z);
        if std::env::var_os("QR_VERBOSE").is_some() && ok != z {
            println!("{name}: ours {ok} zxing {z}");
        }
    }
    for (k, [n, o, z]) in &by_set {
        println!("{k:9} {n:4}: ours {o:4}, zxing-cpp {z:4}");
    }
    println!(
        "ours {ours}/{total}, zxing-cpp {theirs}/{total}; {wrong} wrong; {:.1} ms CPU a picture",
        ms / f64::from(total)
    );
    assert_eq!(wrong, 0);
}
