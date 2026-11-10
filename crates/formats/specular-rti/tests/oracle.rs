//! The fixtures in tests/data (written by gen/make_rti.py) against an independent numpy
//! reading of them written from the format descriptions (tests/data/expected.tsv, written by
//! gen/oracle.py): every coefficient plane's byte-to-value mapping, position-weighted checksum, byte sum and
//! 64 sampled bytes, and the relit colour of the same 64 pixels at four lights.
//!
//! When `SPECULAR_RTI_ORACLE` names another TSV and `SPECULAR_RTI_DIR` its files (the
//! fetched corpus, tools/corpus/fetch-rti.sh), those are checked the same way.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specular_rti::{Error, Look, Rti};

#[derive(Default)]
struct Case {
    file: Option<(String, u32, u32, usize)>,
    planes: Vec<(usize, f64, f64, u64, u64, Vec<u8>)>,
    relit: Vec<([f32; 3], Vec<f64>)>,
    error: Option<String>,
}

fn load(tsv: &Path) -> BTreeMap<String, Case> {
    let text = std::fs::read_to_string(tsv).expect("oracle tsv");
    let mut cases: BTreeMap<String, Case> = BTreeMap::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let c = cases.entry(f[1].to_owned()).or_default();
        let x = |i: usize| f[i].parse::<f64>().unwrap();
        match f[0] {
            "file" => {
                c.file = Some((
                    f[2].to_owned(),
                    f[3].parse().unwrap(),
                    f[4].parse().unwrap(),
                    f[5].parse().unwrap(),
                ));
            }
            "plane" => c.planes.push((
                f[2].parse().unwrap(),
                x(3),
                x(4),
                u64::from_str_radix(f[5], 16).unwrap(),
                f[6].parse().unwrap(),
                f[7].split(',').map(|v| v.parse().unwrap()).collect(),
            )),
            "relit" => c.relit.push((
                [x(2) as f32, x(3) as f32, x(4) as f32],
                f[5].split(',').map(|v| v.parse().unwrap()).collect(),
            )),
            "error" => c.error = Some(f[2].to_owned()),
            other => panic!("unknown row {other}"),
        }
    }
    cases
}

#[derive(Default)]
struct Tally {
    files: usize,
    planes: usize,
    bytes: usize,
    relit: usize,
    /// Sampled bytes that differ (each with the plane and the difference).
    byte_mismatches: Vec<String>,
    /// Planes whose hash or sum differs.
    plane_mismatches: Vec<String>,
    /// Scale/offset, size and relit-value mismatches.
    value_mismatches: Vec<String>,
    refused_too: Vec<String>,
}

fn open(path: &Path) -> Result<Rti, Error> {
    if path.is_dir() {
        let info =
            std::fs::read(path.join("info.json")).map_err(|e| Error::Io("info.json".into(), e))?;
        Rti::relight(&info, &mut |name| {
            std::fs::read(path.join(name)).map_err(|e| Error::Io(name.into(), e))
        })
    } else {
        Rti::read(&std::fs::read(path).map_err(|e| Error::Io(path.display().to_string(), e))?)
    }
}

/// The oracle's position-weighted checksum: Σ byte_i · ((i · 2654435761 + 1) mod 2³²).
fn wsum(bytes: impl Iterator<Item = u8>) -> u64 {
    bytes.enumerate().fold(0u64, |h, (i, b)| {
        let w = (i as u64).wrapping_mul(2_654_435_761).wrapping_add(1) & 0xFFFF_FFFF;
        h.wrapping_add(b as u64 * w)
    })
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * (1.0 + b.abs())
}

fn check_dir(dir: &Path, tsv: &Path) -> Tally {
    let mut t = Tally::default();
    for (name, case) in load(tsv) {
        let got = open(&dir.join(&name));
        let r = match (got, &case.error) {
            (Err(_), Some(_)) => {
                t.refused_too.push(name);
                continue;
            }
            (Err(e), None) => {
                t.value_mismatches.push(format!("{name}: not read: {e}"));
                continue;
            }
            (Ok(_), Some(e)) => {
                t.value_mismatches
                    .push(format!("{name}: read, the oracle could not ({e})"));
                continue;
            }
            (Ok(r), None) => r,
        };
        t.files += 1;
        let (container, w, h, planes) = case.file.clone().expect("file row");
        if (r.container.name(), r.width, r.height, r.planes) != (container.as_str(), w, h, planes) {
            t.value_mismatches.push(format!(
                "{name}: {} {}×{} {} planes, oracle {container} {w}×{h} {planes}",
                r.container.name(),
                r.width,
                r.height,
                r.planes
            ));
            continue;
        }
        let n = w as usize * h as usize;
        let samples: Vec<usize> = (0..64).map(|i| i * n / 64).collect();
        for (p, scale, offset, hash, sum, sampled) in &case.planes {
            t.planes += 1;
            let (l, c) = r.plane_location(*p).expect("plane in range");
            let layer = &r.layers[l];
            if !close(layer.scale[c] as f64, *scale, 1e-6)
                || !close(layer.offset[c] as f64, *offset, 1e-6)
            {
                t.value_mismatches.push(format!(
                    "{name} plane {p}: byte · {} + {}, oracle · {scale} + {offset}",
                    layer.scale[c], layer.offset[c]
                ));
            }
            let bytes = || layer.data.iter().skip(c).step_by(3).copied();
            let s: u64 = bytes().map(u64::from).sum();
            if wsum(bytes()) != *hash || s != *sum {
                t.plane_mismatches
                    .push(format!("{name} plane {p}: byte sum {s}, oracle {sum}"));
            }
            for (&i, &want) in samples.iter().zip(sampled) {
                t.bytes += 1;
                let got = layer.data[i * 3 + c];
                if got != want {
                    t.byte_mismatches
                        .push(format!("{name} plane {p} pixel {i}: {got}, oracle {want}"));
                }
            }
        }
        for (light, want) in &case.relit {
            let img = r.relight_image(&Look {
                light: *light,
                ..Look::default()
            });
            for (k, &i) in samples.iter().enumerate() {
                for ch in 0..3 {
                    t.relit += 1;
                    let got = img[i][ch] as f64;
                    let w = want[k * 3 + ch];
                    if !close(got, w, 2e-4) {
                        t.value_mismatches.push(format!(
                            "{name} light {light:?} pixel {i} channel {ch}: {got}, oracle {w}"
                        ));
                    }
                }
            }
        }
    }
    t
}

fn report(what: &str, t: &Tally) {
    eprintln!(
        "{what}: {} files, {} planes ({} with another hash or sum), {} sampled bytes ({} \
         mismatches), {} relit values; {} value mismatches; refused like the oracle: {:?}",
        t.files,
        t.planes,
        t.plane_mismatches.len(),
        t.bytes,
        t.byte_mismatches.len(),
        t.relit,
        t.value_mismatches.len(),
        t.refused_too
    );
}

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

#[test]
fn fixtures_match_the_numpy_reference() {
    let t = check_dir(&data(), &data().join("expected.tsv"));
    report("numpy oracle", &t);
    assert!(t.files >= 16, "only {} files", t.files);
    for list in [&t.value_mismatches, &t.plane_mismatches, &t.byte_mismatches] {
        assert!(list.is_empty(), "{:#?}", &list[..list.len().min(30)]);
    }
}

#[test]
fn corpus_matches_the_numpy_reference() {
    let (Some(tsv), Some(dir)) = (
        std::env::var_os("SPECULAR_RTI_ORACLE"),
        std::env::var_os("SPECULAR_RTI_DIR"),
    ) else {
        return;
    };
    let t = check_dir(Path::new(&dir), Path::new(&tsv));
    report("numpy oracle (corpus)", &t);
    for list in [&t.value_mismatches, &t.plane_mismatches, &t.byte_mismatches] {
        assert!(list.is_empty(), "{:#?}", &list[..list.len().min(30)]);
    }
}
