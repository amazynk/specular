//! The fetched industrial CT corpus (tools/corpus/fetch-indct.sh: Zeiss Xradia scans, Amira
//! lattices, a SkyScan reconstruction, Nikon and Volume Graphics headers) against
//! tools/corpus/indct/expected.tsv, written by gen/corpus_indct.py with txrm2tiff, olefile,
//! PIL and an independent Amira reader. Skips when the corpus is absent; set
//! SPECULAR_INDCT_CORPUS to read it from elsewhere.
//!
//! The Nikon and Volume Graphics volumes themselves are many gigabytes, so only their headers
//! are fetched: each is read beside a sparse data file of the size it describes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_medvol::{open_path, read_header};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_INDCT_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/indct"),
        PathBuf::from,
    );
    dir.join("expected.tsv").exists().then_some(dir)
}

fn close(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-5 * b.abs().max(1.0)
}

#[derive(Default)]
struct Case {
    kind: String,
    dtype: String,
    volumes: usize,
    geom: Vec<([usize; 3], [f64; 3])>,
    vals: Vec<([usize; 5], f64)>,
}

#[test]
fn corpus_matches_the_oracle() {
    let Some(dir) = corpus() else {
        eprintln!("indct corpus absent: run tools/corpus/fetch-indct.sh");
        return;
    };
    let text = std::fs::read_to_string(dir.join("expected.tsv")).unwrap();
    let mut cases: BTreeMap<String, Case> = BTreeMap::new();
    let mut headers = Vec::new();
    for f in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split('\t').collect::<Vec<_>>())
    {
        let n = |i: usize| f[i].parse::<usize>().unwrap();
        let x = |i: usize| f[i].parse::<f64>().unwrap();
        match f[0] {
            "file" => {
                let c = cases.entry(f[1].to_owned()).or_default();
                c.kind = f[2].to_owned();
                c.dtype = f[3].to_owned();
                c.volumes = n(5);
            }
            "geom" => cases
                .entry(f[1].to_owned())
                .or_default()
                .geom
                .push(([n(2), n(3), n(4)], [x(5), x(6), x(7)])),
            "val" => cases
                .entry(f[1].to_owned())
                .or_default()
                .vals
                .push(([n(2), n(3), n(4), n(5), n(6)], x(7))),
            "hdr" => headers.push(f.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>()),
            t => panic!("unknown row {t}"),
        }
    }
    let (mut values, mut mismatches) = (0, 0);
    for (name, c) in &cases {
        let t = Instant::now();
        let v = open_path(&dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let open = t.elapsed();
        let canon = v.canonical();
        let t = Instant::now();
        let _ = v.plane_f32(&canon, canon.dims[2] / 2, 0);
        eprintln!(
            "{name}: {:?} {}, open {open:?}, middle slice {:?}",
            v.header.layout.size,
            v.header.dtype.name(),
            t.elapsed()
        );
        let h = &v.header;
        assert_eq!(h.kind.name(), c.kind, "{name}: kind");
        assert_eq!(h.dtype.name(), c.dtype, "{name}: type");
        assert_eq!(v.volumes(), c.volumes, "{name}: volumes");
        for (ijk, p) in &c.geom {
            let q = h.to_patient(ijk.map(|x| x as f64));
            for a in 0..3 {
                assert!(
                    close(q[a], p[a]),
                    "{name}: voxel {ijk:?} axis {a}: {} vs {}",
                    q[a],
                    p[a]
                );
            }
        }
        for ([i, j, k, vol, comp], want) in &c.vals {
            values += 1;
            if !close(v.value(*i, *j, *k, *vol, *comp), *want) {
                mismatches += 1;
            }
        }
    }
    // Headers alone: read beside a sparse data file the size they describe.
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("indct-headers");
    for r in &headers {
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let src = dir.join(&r[1]);
        let at = tmp.join(src.file_name().unwrap());
        let bytes = std::fs::read(&src).unwrap();
        std::fs::write(&at, &bytes).unwrap();
        let n = |i: usize| r[i].parse::<u64>().unwrap();
        let size = n(2) * n(3) * n(4) * n(9);
        std::fs::File::create(tmp.join(&r[8]))
            .unwrap()
            .set_len(size)
            .unwrap();
        let h = read_header(&bytes, &at).unwrap_or_else(|e| panic!("{}: {e}", r[1]));
        assert_eq!(
            h.layout.size,
            [n(2), n(3), n(4)].map(|x| x as usize),
            "{}",
            r[1]
        );
        for a in 0..3 {
            let want: f64 = r[5 + a].parse().unwrap();
            assert!(
                close(h.spacing[a], want),
                "{}: spacing {a}: {} vs {want}",
                r[1],
                h.spacing[a]
            );
        }
        assert_eq!(h.dtype.name(), r[10], "{}", r[1]);
        let _ = std::fs::remove_dir_all(&tmp);
    }
    // Amira colour maps: one-dimensional lattices, opened or refused but never a panic.
    for name in [
        "amira/borophagus/grayScale.am",
        "amira/borophagus/labels.am",
    ] {
        let r = open_path(&dir.join(name));
        eprintln!("{name}: {:?}", r.as_ref().map(|v| v.header.layout.size));
    }
    eprintln!(
        "indct corpus: {} volumes, {values} values, {mismatches} mismatches; {} headers",
        cases.len(),
        headers.len()
    );
    assert_eq!(mismatches, 0);
}
