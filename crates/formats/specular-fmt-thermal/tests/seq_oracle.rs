//! FLIR sequences against independent readings (gen/seq_oracle.py): frames cut by their
//! headers, each read by exiftool (counts; JPEG-LS through CharLS), converted to degrees
//! Celsius in numpy, and SEQ frames read again by flirpy. Counts must match exactly, times
//! to the millisecond, temperatures within 0.01 degrees of both.
//!
//! The synthetic fixtures (gen/make_seq_fixtures.py) are checked against
//! tests/data/seq_expected.tsv. When `SPECULAR_SEQ_ORACLE` names another TSV and
//! `SPECULAR_SEQ_DIR` its files (tools/corpus/fetch-thermal-seq.sh), those are checked too.

use std::collections::BTreeMap;
use std::path::Path;

use specular_fmt_thermal::{Kind, Pixels, index_sequence, is_sequence, sniff};

#[derive(Default)]
struct Case {
    frames: usize,
    size: (usize, usize),
    times: BTreeMap<usize, Option<f64>>,
    cond: BTreeMap<usize, [f64; 7]>,
    /// (frame, pixel, raw, celsius, flirpy celsius)
    px: Vec<(usize, usize, u16, f64, Option<f64>)>,
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
        let n = |i: usize| f[i].parse::<f64>().unwrap_or(f64::NAN);
        match f[0] {
            "seq" => {
                c.frames = f[2].parse().unwrap();
                c.size = (f[3].parse().unwrap(), f[4].parse().unwrap());
            }
            "frame" => {
                c.times.insert(f[2].parse().unwrap(), f[3].parse().ok());
            }
            "cond" => {
                c.cond
                    .insert(f[2].parse().unwrap(), std::array::from_fn(|i| n(3 + i)));
            }
            "px" => c.px.push((
                f[2].parse().unwrap(),
                f[3].parse().unwrap(),
                f[4].parse().unwrap(),
                n(5),
                f[6].parse().ok(),
            )),
            "error" => c.error = Some(f[2].to_owned()),
            t => panic!("unknown row {t}"),
        }
    }
    cases
}

#[derive(Default, Debug)]
struct Tally {
    files: usize,
    frames: usize,
    oracle_refused: usize,
    raw_values: usize,
    raw_mismatches: usize,
    temperatures: usize,
    temperature_mismatches: usize,
    max_abs_diff: f64,
    flirpy_values: usize,
    flirpy_mismatches: usize,
    flirpy_max_abs_diff: f64,
    other_mismatches: usize,
}

fn check(dir: &Path, cases: &BTreeMap<String, Case>) -> Tally {
    let mut t = Tally::default();
    for (name, c) in cases {
        if let Some(e) = &c.error {
            eprintln!("{name}: oracle: {e}");
            t.oracle_refused += 1;
            continue;
        }
        let data = std::fs::read(dir.join(name)).expect("file");
        assert!(is_sequence(&data), "{name}: not seen as a sequence");
        let seq = match index_sequence(&data) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{name}: we refused: {e}");
                t.other_mismatches += 1;
                continue;
            }
        };
        t.files += 1;
        if seq.len() != c.frames || (seq.width, seq.height) != c.size {
            eprintln!(
                "{name}: {} frames {}x{} vs {} {:?}",
                seq.len(),
                seq.width,
                seq.height,
                c.frames,
                c.size
            );
            t.other_mismatches += 1;
            continue;
        }
        let mut frame = None;
        let mut temps = Vec::new();
        for &(k, i, raw, celsius, flirpy) in &c.px {
            if frame.as_ref().is_none_or(|(fk, _)| *fk != k) {
                let th = seq.frame(&data, k).expect("frame");
                t.frames += 1;
                if let Some(o) = c.cond.get(&k) {
                    let q = &th.conditions;
                    let ours = [
                        q.emissivity,
                        q.distance,
                        q.reflected,
                        q.atmospheric,
                        q.window_temperature,
                        q.window_transmission,
                        q.humidity,
                    ];
                    for (j, (a, b)) in ours.iter().zip(o).enumerate() {
                        if a.is_none_or(|a| (a - b).abs() > 1e-6) {
                            eprintln!("{name} frame {k}: condition {j}: {a:?} vs {b}");
                            t.other_mismatches += 1;
                        }
                    }
                }
                let want = c.times.get(&k).copied().flatten();
                let got = seq.frames[k].time;
                let ok = match (got, want) {
                    (Some(g), Some(w)) => (g - w).abs() < 1.5e-3,
                    (None, None) => true,
                    _ => false,
                };
                if !ok {
                    eprintln!("{name} frame {k}: time {got:?} vs {want:?}");
                    t.other_mismatches += 1;
                }
                temps = th.temperatures(&th.conditions);
                frame = Some((k, th));
            }
            let (_, th) = frame.as_ref().unwrap();
            let Pixels::Counts { values, .. } = &th.pixels else {
                panic!("{name}: no counts");
            };
            t.raw_values += 1;
            if values[i] != raw {
                if t.raw_mismatches < 5 {
                    eprintln!("{name} frame {k}: raw {i}: {} vs {raw}", values[i]);
                }
                t.raw_mismatches += 1;
            }
            let ours = f64::from(temps[i]);
            t.temperatures += 1;
            let both_nan = ours.is_nan() && celsius.is_nan();
            let d = (ours - celsius).abs();
            if !both_nan && d.is_finite() {
                t.max_abs_diff = t.max_abs_diff.max(d);
            }
            if !both_nan && (d.is_nan() || d > 0.01) {
                if t.temperature_mismatches < 5 {
                    eprintln!("{name} frame {k}: temperature {i}: {ours} vs {celsius}");
                }
                t.temperature_mismatches += 1;
            }
            if let Some(fp) = flirpy {
                t.flirpy_values += 1;
                let d = (ours - fp).abs();
                t.flirpy_max_abs_diff = t.flirpy_max_abs_diff.max(d);
                if d.is_nan() || d > 0.01 {
                    t.flirpy_mismatches += 1;
                }
            }
        }
    }
    t
}

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

#[test]
fn fixtures_match_the_oracles() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let t = check(&dir, &load(&dir.join("seq_expected.tsv")));
    eprintln!("fixtures: {t:?}");
    assert_eq!((t.files, t.frames), (2, 10));
    assert_eq!(t.flirpy_values, 6 * 16 * 12);
    assert_eq!(
        t.raw_mismatches + t.temperature_mismatches + t.flirpy_mismatches + t.other_mismatches,
        0,
        "{t:?}"
    );
}

#[test]
fn corpus_matches_the_oracles() {
    let (Ok(tsv), Ok(dir)) = (
        std::env::var("SPECULAR_SEQ_ORACLE"),
        std::env::var("SPECULAR_SEQ_DIR"),
    ) else {
        return;
    };
    let t = check(Path::new(&dir), &load(Path::new(&tsv)));
    eprintln!("corpus: {t:?}");
    assert!(t.files > 0);
    assert_eq!(
        t.raw_mismatches + t.temperature_mismatches + t.flirpy_mismatches + t.other_mismatches,
        0,
        "{t:?}"
    );
}

#[test]
fn kinds_timing_and_frames() {
    let seq = data("seq_raw.seq");
    assert_eq!(sniff(&seq), Some(Kind::FlirSeq));
    let s = index_sequence(&seq).unwrap();
    assert_eq!(s.kind, Kind::FlirSeq);
    assert_eq!(s.len(), 6);
    assert_eq!(s.frame_rate, Some(25.0));
    assert!((s.seconds(5) - 0.2).abs() < 1e-6);
    assert!((s.duration(5) - 0.04).abs() < 1e-6);
    // Frame 4 carries its own Planck O: its counts differ, its temperatures do not.
    let f3 = s.frame(&seq, 3).unwrap();
    let f4 = s.frame(&seq, 4).unwrap();
    let p = |t: &specular_fmt_thermal::Thermal| match &t.pixels {
        Pixels::Counts {
            mapping: specular_fmt_thermal::Mapping::Planck(p),
            ..
        } => p.o,
        _ => f64::NAN,
    };
    assert_eq!((p(&f3), p(&f4)), (-7142.0, -7100.0));
    let (a, b) = (
        f3.temperatures(&f3.conditions),
        f4.temperatures(&f4.conditions),
    );
    assert!((a[0] - b[0]).abs() < 0.01, "{} {}", a[0], b[0]);

    let csq = data("csq_jls.csq");
    assert_eq!(sniff(&csq), Some(Kind::FlirCsq));
    let s = index_sequence(&csq).unwrap();
    assert_eq!((s.kind, s.len(), s.frame_rate), (Kind::FlirCsq, 4, None));
    assert!((s.rate - 30.0).abs() < 0.5, "{}", s.rate);
    // One frame of the SEQ is a still, not a sequence.
    let one = &seq[..s.frames[0].len.min(seq.len())];
    let first = index_sequence(&seq).unwrap().frames[0];
    assert!(!is_sequence(&seq[..first.len]), "{}", one.len());
    assert_eq!(sniff(&seq[..first.len]), Some(Kind::FlirFff));
}

#[test]
fn truncated_and_corrupt_sequences_fail_cleanly() {
    let seq = data("seq_raw.seq");
    for cut in [70, 200, 1000, seq.len() / 2, seq.len() - 1] {
        let d = &seq[..cut];
        if let Ok(s) = index_sequence(d) {
            for k in 0..s.len() {
                let _ = s.frame(d, k);
            }
        }
    }
    let csq = data("csq_jls.csq");
    let mut bad = csq.clone();
    let n = bad.len();
    for b in &mut bad[n / 2..n / 2 + 64] {
        *b ^= 0x5a;
    }
    if let Ok(s) = index_sequence(&bad) {
        for k in 0..s.len() {
            let _ = s.frame(&bad, k);
        }
    }
}
