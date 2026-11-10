//! Data Matrix against zxing-cpp and libdmtx on generated pictures (every encodation, square
//! and rectangular sizes; rotations, perspective, noise, blur, small modules, mirrored, light
//! on dark), made by tools/corpus/dm-oracle.py (not committed):
//!
//!   SPECULAR_DM_CORPUS=<dir> cargo test -p specular-codes --test dm_oracle -- --ignored --nocapture
//!
//! Compares bytes (Base 256 holds arbitrary bytes). A code read with the wrong bytes fails
//! the test.

use std::collections::{BTreeMap, HashMap};
mod common;
use common::{pgm, timed};
use std::path::{Path, PathBuf};

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn results(p: &Path) -> HashMap<String, bool> {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(a, b)| (a.to_string(), b == "1"))
        .collect()
}

#[test]
#[ignore]
fn decodes_data_matrix_like_the_oracles() {
    let Some(dir) = std::env::var_os("SPECULAR_DM_CORPUS").map(PathBuf::from) else {
        eprintln!("SPECULAR_DM_CORPUS not set");
        return;
    };
    let manifest = std::fs::read_to_string(dir.join("manifest.tsv")).unwrap();
    let zx = results(&dir.join("zxing.tsv"));
    let lib = results(&dir.join("dmtx.tsv"));
    let (mut ours, mut z_n, mut l_n, mut total, mut wrong) = (0, 0, 0, 0, 0);
    let mut by_kind: BTreeMap<String, [u32; 4]> = Default::default();
    let mut ms = 0f64;
    let mut slowest = (0f64, String::new());
    for line in manifest.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let (name, want, size, scheme, kind) = (f[0], unhex(f[1]), f[2], f[3], f[4]);
        let g = pgm(&dir.join(name));
        let (codes, e) = timed(|| specular_codes::scan(&g));
        ms += e;
        if e > slowest.0 {
            slowest = (e, format!("{name} {}×{}", g.w, g.h));
        }
        let dms: Vec<_> = codes.iter().filter(|c| c.kind == "Data Matrix").collect();
        let ok = dms.iter().any(|c| c.bytes == want);
        if !ok && !dms.is_empty() {
            wrong += 1;
            println!("WRONG {name} {size} {scheme} {kind}");
        }
        let z = zx.get(name).copied().unwrap_or(false);
        let l = lib.get(name).copied().unwrap_or(false);
        total += 1;
        ours += u32::from(ok);
        z_n += u32::from(z);
        l_n += u32::from(l);
        let keys = kind
            .split('+')
            .map(|k| k.trim_end_matches(|c: char| c.is_ascii_digit()).to_string())
            .chain([format!("[{scheme}]")]);
        for k in keys {
            let e = by_kind.entry(k).or_default();
            e[0] += 1;
            e[1] += u32::from(ok);
            e[2] += u32::from(z);
            e[3] += u32::from(l);
        }
        if std::env::var_os("DM_VERBOSE").is_some() && ok != z {
            println!("{name} {size} {scheme} {kind}: ours {ok} zxing {z} libdmtx {l}");
        }
    }
    for (k, [n, o, z, l]) in &by_kind {
        println!("{k:10} {n:4}: ours {o:4}, zxing-cpp {z:4}, libdmtx {l:4}");
    }
    println!(
        "ours {ours}/{total}, zxing-cpp {z_n}/{total}, libdmtx {l_n}/{total}; {wrong} wrong; {:.1} ms CPU a picture, slowest {:.0} ms ({})",
        ms / f64::from(total),
        slowest.0,
        slowest.1
    );
    assert_eq!(wrong, 0);
}
