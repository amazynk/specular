//! The DeltaVision samples in tests/data/dv against the `mrc` package's reading of them
//! (tests/data/dv/expected-dv.tsv, written by gen/make_dv.py): the Z, W and T counts, the
//! section order, pixel sizes, wavelengths, time stamps and every value.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specular_em::dv;

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/dv")
}

#[test]
fn deltavision_matches_the_mrc_package() {
    let text = std::fs::read_to_string(data().join("expected-dv.tsv")).unwrap();
    let mut files: BTreeMap<String, (Vec<u8>, dv::Dv)> = BTreeMap::new();
    let mut planes: BTreeMap<(String, usize), Vec<f32>> = BTreeMap::new();
    let (mut values, mut mismatches, mut facts, mut refused) = (0, 0, 0, 0);
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let n = |i: usize| f[i].parse::<usize>().unwrap();
        let x = |i: usize| f[i].parse::<f64>().unwrap();
        match f[0] {
            "file" => {
                let bytes = std::fs::read(data().join(f[1])).unwrap();
                assert!(dv::sniff(&bytes), "{}", f[1]);
                let d = dv::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", f[1]));
                assert_eq!((d.nz, d.nw, d.nt), (n(2), n(3), n(4)), "{}", f[1]);
                let seq = match d.sequence {
                    dv::Sequence::Ztw => 0,
                    dv::Sequence::Wzt => 1,
                    dv::Sequence::Zwt => 2,
                };
                assert_eq!(seq, n(5), "{}", f[1]);
                for (k, i) in [6, 7, 8].into_iter().enumerate() {
                    assert_eq!(f64::from(d.pixel[k]), x(i), "{} pixel {k}", f[1]);
                }
                facts += 5;
                files.insert(f[1].to_owned(), (bytes, d));
            }
            "wave" => {
                let d = &files[f[1]].1;
                assert_eq!(i64::from(d.waves[n(2)]), f[3].parse::<i64>().unwrap());
                facts += 1;
            }
            "time" => {
                let d = &files[f[1]].1;
                let s = d.section(0, 0, n(2));
                assert_eq!(
                    f64::from(d.sections[s].time),
                    x(3),
                    "{} time {}",
                    f[1],
                    n(2)
                );
                facts += 1;
            }
            "val" => {
                let (bytes, d) = &files[f[1]];
                let s = d.section(n(2), n(3), n(4));
                let p = planes.entry((f[1].to_owned(), s)).or_insert_with(|| {
                    let mut v = vec![0f32; d.mrc.nx * d.mrc.ny];
                    assert!(!d.mrc.read_plane(bytes, s, &mut v).unwrap());
                    v
                });
                let got = f64::from(p[n(6) * d.mrc.nx + n(5)]);
                values += 1;
                if got != x(7) {
                    mismatches += 1;
                }
            }
            "error" => {
                let bytes = std::fs::read(data().join(f[1])).unwrap();
                let e = dv::parse(&bytes).unwrap_err().to_string();
                assert!(e.contains(f[2]), "{}: {e}", f[1]);
                refused += 1;
            }
            t => panic!("unknown row {t}"),
        }
    }
    eprintln!(
        "DeltaVision oracle: {} files, {facts} facts, {values} values, {mismatches} mismatches, \
         {refused} refused",
        files.len()
    );
    assert!(values > 400);
    assert_eq!(mismatches, 0);
}
