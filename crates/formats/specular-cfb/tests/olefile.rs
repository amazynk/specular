//! The compound files in specular-medvol's tests/data against olefile's reading of them:
//! every stream it lists, with its size and FNV-1a 64 hash (the `cfb` rows of
//! expected-indct-extra.tsv, written by specular-medvol's gen/make_indct.py).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specular_cfb::{Cfb, Kind};

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../specular-medvol/tests/data")
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xCBF2_9CE4_8422_2325, |h, &x| {
        (h ^ u64::from(x)).wrapping_mul(0x0100_0000_01B3)
    })
}

#[test]
fn streams_match_olefile() {
    let text = std::fs::read_to_string(data().join("expected-indct-extra.tsv")).unwrap();
    let mut files: BTreeMap<String, Vec<(String, u64, u64)>> = BTreeMap::new();
    for f in text
        .lines()
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .filter(|f| f[0] == "cfb")
    {
        files.entry(f[1].to_owned()).or_default().push((
            f[2].to_owned(),
            f[3].parse().unwrap(),
            u64::from_str_radix(f[4], 16).unwrap(),
        ));
    }
    assert!(files.len() >= 5);
    let (mut streams, mut mismatches) = (0, 0);
    for (name, want) in &files {
        let bytes = std::fs::read(data().join(name)).unwrap();
        assert!(specular_cfb::sniff(&bytes), "{name}");
        let c = Cfb::open(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut listed: Vec<String> = c
            .list()
            .filter(|(_, e)| e.kind == Kind::Stream)
            .map(|(p, _)| p.to_owned())
            .collect();
        listed.sort_by_key(|p| p.to_ascii_lowercase());
        let mut names: Vec<String> = want.iter().map(|w| w.0.clone()).collect();
        names.sort_by_key(|p| p.to_ascii_lowercase());
        assert_eq!(listed, names, "{name}: streams");
        for (path, size, hash) in want {
            let b = c
                .read(path)
                .unwrap_or_else(|e| panic!("{name} {path}: {e}"));
            streams += 1;
            if b.len() as u64 != *size || fnv(&b) != *hash {
                mismatches += 1;
            }
            // Paths are found whatever their case.
            assert!(c.exists(&path.to_ascii_uppercase()), "{name} {path}");
        }
    }
    eprintln!(
        "{} files, {streams} streams, {mismatches} mismatches against olefile",
        files.len()
    );
    assert_eq!(mismatches, 0);
}
