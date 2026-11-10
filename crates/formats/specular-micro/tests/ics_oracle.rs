//! ICS fixtures (`tests/data/ics`, written by `gen/ics-fixtures.py` with DIPlib's libics and
//! by hand) against DIPlib's reading of them (`ics-expected.tsv`: sum, min and max of every
//! sample, complex as magnitude).

use std::path::PathBuf;

use specular_micro::ics::{self, DataAt};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ics")
}

/// Every sample of the file, plane by plane.
fn samples(name: &str) -> Vec<f64> {
    let d = dir();
    let head = std::fs::read(d.join(name)).unwrap();
    let i = ics::parse(&head).unwrap_or_else(|e| panic!("{name}: {e}"));
    let file = match &i.data {
        DataAt::Inline(_) => head.clone(),
        DataAt::Ids(n) => std::fs::read(d.join(n)).unwrap(),
        DataAt::Source { file, .. } => std::fs::read(d.join(file)).unwrap(),
    };
    let stream = i.stream(&file).unwrap();
    let planes = i.plane_dims();
    let mut pos = vec![0usize; i.dims.len()];
    let mut out = Vec::new();
    let mut plane = vec![0f32; i.width() * i.height()];
    loop {
        let truncated = i.read_plane(&stream, &pos, 0, &mut plane).unwrap();
        assert!(!truncated, "{name}");
        out.extend(plane.iter().map(|&v| f64::from(v)));
        // Next position, fastest dimension first.
        let mut k = 0;
        loop {
            if k == planes.len() {
                return out;
            }
            let d = planes[k];
            pos[d] += 1;
            if pos[d] < i.dims[d].len {
                break;
            }
            pos[d] = 0;
            k += 1;
        }
    }
}

#[test]
fn matches_diplib() {
    let tsv = std::fs::read_to_string(dir().join("ics-expected.tsv")).unwrap();
    let mut n = 0;
    let mut mismatches = 0;
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let (name, sum, min, max): (&str, f64, f64, f64) = (
            f[0],
            f[3].parse().unwrap(),
            f[4].parse().unwrap(),
            f[5].parse().unwrap(),
        );
        let v = samples(name);
        let s: f64 = v.iter().sum();
        let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-6 * b.abs().max(1.0);
        if !(close(s, sum) && close(lo, min) && close(hi, max)) {
            mismatches += 1;
            eprintln!("{name}: sum {s} min {lo} max {hi}, DIPlib {sum} {min} {max}");
        }
        n += 1;
    }
    assert_eq!(n, 6);
    assert_eq!(mismatches, 0);
}

/// The public ICS files of tools/corpus/fetch-micro2.sh (libics' test images, one plain and
/// one with a compress `.ids.Z`; an SVI Huygens result with a detector dimension) against
/// DIPlib (`ics` rows of expected.tsv). Skips when the corpus is absent.
#[test]
fn corpus_matches_diplib() {
    let dir = std::env::var_os("SPECULAR_MICRO2_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/micro2")
        });
    let Ok(tsv) = std::fs::read_to_string(dir.join("expected.tsv")) else {
        eprintln!("micro2 corpus absent: run tools/corpus/fetch-micro2.sh");
        return;
    };
    let mut mismatches = 0;
    let mut n = 0;
    for line in tsv.lines().filter(|l| l.starts_with("ics\t")) {
        let f: Vec<&str> = line.split('\t').collect();
        let path = dir.join(f[1]);
        let head = std::fs::read(&path).unwrap();
        let mut i = ics::parse(&head).unwrap();
        let folder = path.parent().unwrap();
        let file = match &i.data {
            DataAt::Inline(_) => head.clone(),
            DataAt::Ids(name) => {
                let d = [name.clone(), format!("{name}.Z"), format!("{name}.gz")]
                    .iter()
                    .find_map(|n| std::fs::read(folder.join(n)).ok())
                    .unwrap();
                match d.as_slice() {
                    [0x1f, 0x9d, ..] => i.compression = ics::Compression::Compress,
                    [0x1f, 0x8b, ..] => i.compression = ics::Compression::Gzip,
                    _ => {}
                }
                d
            }
            DataAt::Source { file, .. } => std::fs::read(folder.join(file)).unwrap(),
        };
        let stream = i.stream(&file).unwrap();
        let planes = i.plane_dims();
        let mut pos = vec![0usize; i.dims.len()];
        let mut plane = vec![0f32; i.width() * i.height()];
        let (mut s, mut lo, mut hi) = (0f64, f64::INFINITY, f64::NEG_INFINITY);
        'all: loop {
            i.read_plane(&stream, &pos, 0, &mut plane).unwrap();
            for &v in &plane {
                let v = f64::from(v);
                s += v;
                lo = lo.min(v);
                hi = hi.max(v);
            }
            let mut k = 0;
            loop {
                if k == planes.len() {
                    break 'all;
                }
                let d = planes[k];
                pos[d] += 1;
                if pos[d] < i.dims[d].len {
                    break;
                }
                pos[d] = 0;
                k += 1;
            }
        }
        let want: Vec<f64> = f[4..7].iter().map(|v| v.parse().unwrap()).collect();
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-6 * b.abs().max(1.0);
        if !(close(s, want[0]) && close(lo, want[1]) && close(hi, want[2])) {
            mismatches += 1;
            eprintln!("{}: {s} {lo} {hi}, DIPlib {want:?}", f[1]);
        }
        n += 1;
    }
    eprintln!("ICS corpus: {n} files, {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}

#[test]
fn dimensions_units_and_channels() {
    let h = std::fs::read(dir().join("be_s16_chxyt.ics")).unwrap();
    let i = ics::parse(&h).unwrap();
    assert_eq!(i.channel, Some(0));
    assert_eq!((i.width(), i.height()), (10, 6));
    assert_eq!(i.emission, vec![450.0, 520.0, 610.0]);
    assert_eq!(i.dims[3].unit, "seconds");
    let v1 = ics::parse(&std::fs::read(dir().join("v1_u16_zyx.ics")).unwrap()).unwrap();
    assert_eq!(v1.version, "1.0");
    assert_eq!(v1.dims[2].scale, 0.5);
    assert_eq!(v1.data, DataAt::Ids("v1_u16_zyx.ids".into()));
    let t = ics::parse(&std::fs::read(dir().join("v2_u8_2ch.ics")).unwrap()).unwrap();
    assert_eq!(t.channel, Some(2));
}
