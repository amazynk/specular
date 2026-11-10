//! The fixtures in tests/data (written by gen/make_segy.py) against segyio's reading of
//! them (tests/data/expected.tsv, written by gen/oracle.py): file facts, the textual
//! header, every standard trace header field of three traces per file, scaled CDP
//! coordinates and the sum of every trace, strided sample values, segyio's inline sections
//! of the full 3-D grids, and the cell of every trace of the volume with gaps.
//!
//! When `SPECULAR_SEGY_ORACLE` names another TSV and `SPECULAR_SEGY_DIR` its files (the
//! fetched corpus, tools/corpus/fetch-segy.sh), those are checked the same way.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specular_segy::{Endian, Kind, Segy};

/// segyio's geometry: inline and crossline bytes, inline and crossline numbers, offsets.
type Geom = (usize, usize, Vec<i32>, Vec<i32>, usize);

#[derive(Default)]
struct Case {
    file: Vec<String>,
    text: Vec<(usize, String)>,
    hdr: Vec<(usize, usize, i64)>,
    xy: Vec<(usize, f64, f64)>,
    tsum: Vec<(usize, f64)>,
    val: Vec<(usize, usize, f64)>,
    geom: Option<Geom>,
    sect: Vec<(usize, usize, usize, f64)>,
    cell: Vec<(i32, i32, usize)>,
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
        let n = |i: usize| f[i].parse::<usize>().unwrap();
        let x = |i: usize| f[i].parse::<f64>().unwrap();
        let list = |s: &str| {
            s.split(',')
                .map(|v| v.parse::<i32>().unwrap())
                .collect::<Vec<_>>()
        };
        match f[0] {
            "file" => c.file = f[2..].iter().map(|s| (*s).to_owned()).collect(),
            "text" => c
                .text
                .push((n(2), f.get(3).copied().unwrap_or("").to_owned())),
            "hdr" => c.hdr.push((n(2), n(3), f[4].parse().unwrap())),
            "xy" => c.xy.push((n(2), x(3), x(4))),
            "tsum" => c.tsum.push((n(2), x(3))),
            "val" => c.val.push((n(2), n(3), x(4))),
            "geom" => c.geom = Some((n(2), n(3), list(f[4]), list(f[5]), n(6))),
            "sect" => c.sect.push((n(2), n(3), n(4), x(5))),
            "cell" => c
                .cell
                .push((f[2].parse().unwrap(), f[3].parse().unwrap(), n(4))),
            "error" => c.error = Some(f[2].to_owned()),
            "skip" => {}
            t => panic!("unknown row {t}"),
        }
    }
    cases
}

/// Equal as f32 values (segyio returns f32 samples; NaN equals NaN).
fn same(a: f32, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b as f32 || (f64::from(a) - b).abs() <= 1e-6 * b.abs()
}

/// IBM floats below the smallest normal f32: segyio's bit-shifting conversion gets these
/// subnormals wrong (0xa1213e19 is -6.1057e-39 exactly; segyio says -4.56e-40), so both
/// sides being subnormal counts as agreement, and is tallied.
fn both_subnormal(a: f32, b: f64) -> bool {
    a.abs() < f32::MIN_POSITIVE && b.abs() < f64::from(f32::MIN_POSITIVE)
}

/// Files where segyio is known to be wrong, with the reason; counted, not compared.
const SEGYIO_WRONG: &[(&str, &str)] = &[(
    "segyio/stanzas-unknown-count.sgy",
    "an extended header count of -1 (stanzas up to ((SEG: EndText))) is read as 0 by segyio",
)];

#[derive(Default, Debug)]
struct Tally {
    files: usize,
    values: usize,
    mismatches: Vec<String>,
    /// Files segyio refused that are refused here too, and ones read anyway.
    refused_too: Vec<String>,
    opened_anyway: Vec<String>,
    /// Samples where both readings are f32 subnormals (see `both_subnormal`).
    subnormal: usize,
    /// Files skipped because segyio is known to misread them.
    segyio_wrong: Vec<String>,
}

impl Tally {
    fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        self.values += 1;
        if !ok {
            self.mismatches.push(what());
        }
    }
}

fn check_dir(dir: &Path, tsv: &Path) -> Tally {
    let cases = load(tsv);
    let mut t = Tally::default();
    for (name, c) in &cases {
        let path = dir.join(name);
        let opened = if name.ends_with(".su") {
            Segy::open(&path, true)
        } else {
            Segy::open(&path, false)
        };
        if let Some((_, why)) = SEGYIO_WRONG.iter().find(|w| w.0 == name) {
            t.check(opened.is_ok(), || format!("{name}: refused"));
            t.segyio_wrong.push(format!("{name}: {why}"));
            continue;
        }
        if let Some(e) = &c.error {
            // segyio refused it: a cut file must be refused (or read short) by us too; rev 2
            // layouts segyio 1.9 does not know are counted, not compared.
            match &opened {
                Err(_) => t.refused_too.push(name.clone()),
                Ok(s) => t.opened_anyway.push(format!(
                    "{name} (segyio: {e}; here rev {:?}, {:?}, {} traces of {} samples, {} \
                     trailing bytes, {} extra headers)",
                    s.revision(),
                    s.format,
                    s.traces,
                    s.samples,
                    s.trailing,
                    s.extra_headers
                )),
            }
            continue;
        }
        let s = match opened {
            Ok(s) => s,
            Err(e) => {
                t.mismatches.push(format!("{name}: {e}"));
                continue;
            }
        };
        t.files += 1;
        let f = &c.file;
        t.check((s.kind == Kind::Su) == (f[0] == "su"), || {
            format!("{name}: kind")
        });
        t.check((s.endian == Endian::Big) == (f[1] == "big"), || {
            format!("{name}: endian {:?}", s.endian)
        });
        t.check(s.format.code().to_string() == f[2], || {
            format!("{name}: format {:?} vs {}", s.format, f[2])
        });
        t.check(s.samples.to_string() == f[3], || format!("{name}: samples"));
        t.check(s.traces.to_string() == f[4], || {
            format!("{name}: traces {} vs {}", s.traces, f[4])
        });
        // segyio reports the binary interval as a signed 16-bit value; a negative one is
        // junk (the rev 2.1 field-limit files), read here as unsigned.
        let dt = f[5].parse::<f64>().unwrap();
        t.check(dt <= 0.0 || s.interval == dt, || {
            format!("{name}: interval {} vs {}", s.interval, f[5])
        });
        t.check(
            s.sample_position(0.0) == f[6].parse::<f64>().unwrap(),
            || format!("{name}: t0 {} vs {}", s.sample_position(0.0), f[6]),
        );
        if s.traces.to_string() != f[4] || s.samples.to_string() != f[3] {
            continue;
        }
        let ext = s.binary.as_ref().map_or(0, |b| b.extended_headers);
        t.check(ext.to_string() == f[7], || {
            format!("{name}: extended headers")
        });
        for (card, line) in &c.text {
            let ours = s.text.get(card - 1).map_or("", String::as_str);
            t.check(ours == line.trim_end(), || {
                format!("{name}: card {card}: {ours:?} vs {line:?}")
            });
        }
        for &(tr, byte, v) in &c.hdr {
            let h = s.header(tr).unwrap();
            let width = specular_segy::TRACE_FIELDS
                .iter()
                .find(|f| f.0 == byte)
                .map_or(4, |f| f.1);
            let ours = i64::from(h.get(byte, width));
            t.check(ours == v, || {
                format!("{name}: trace {tr} byte {byte}: {ours} vs {v}")
            });
        }
        let g = s.geometry().unwrap();
        for &(tr, x, y) in &c.xy {
            let [gx, gy] = g.coords[tr];
            t.check(
                (gx - x).abs() <= 1e-9 * x.abs().max(1.0)
                    && (gy - y).abs() <= 1e-9 * y.abs().max(1.0),
                || format!("{name}: trace {tr} xy ({gx}, {gy}) vs ({x}, {y})"),
            );
        }
        let mut traces = Vec::with_capacity(s.traces);
        for i in 0..s.traces {
            traces.push(s.trace(i).unwrap());
        }
        for &(tr, sum) in &c.tsum {
            let ours: f64 = traces[tr].iter().map(|&v| f64::from(v)).sum();
            t.check(
                (ours - sum).abs() <= 1e-6 * sum.abs().max(1.0) || (ours.is_nan() && sum.is_nan()),
                || format!("{name}: trace {tr} sum {ours} vs {sum}"),
            );
        }
        for &(tr, k, v) in &c.val {
            let ours = traces[tr][k];
            if ours != 0.0 && both_subnormal(ours, v) {
                t.subnormal += 1;
                continue;
            }
            t.check(same(ours, v), || {
                format!("{name}: trace {tr}[{k}] {ours} vs {v}")
            });
        }
        if let Some((_, _, _, _, offsets)) = &c.geom
            && *offsets > 1
        {
            // Pre-stack: segyio's offsets dimension; shown as traces here.
            t.check(g.volume.is_none(), || {
                format!("{name}: pre-stack read as a volume")
            });
        } else if let Some((ib, xb, il, xl, _)) = &c.geom {
            match &g.volume {
                Some(v) => {
                    t.check(v.key == (*ib, *xb), || format!("{name}: key {:?}", v.key));
                    t.check(&v.inlines == il && &v.crosslines == xl, || {
                        format!("{name}: grid {:?} × {:?}", v.inlines, v.crosslines)
                    });
                    t.check(v.gaps == 0, || format!("{name}: gaps in a full grid"));
                    let mut secs = BTreeMap::new();
                    for &(i, x, k, val) in &c.sect {
                        let sec = secs
                            .entry(i)
                            .or_insert_with(|| v.inline_section(&s, i).unwrap());
                        let ours = sec[k * v.crosslines.len() + x];
                        if ours != 0.0 && both_subnormal(ours, val) {
                            t.subnormal += 1;
                            continue;
                        }
                        t.check(same(ours, val), || {
                            format!("{name}: inline {i} xl {x} [{k}] {ours} vs {val}")
                        });
                    }
                }
                None => t
                    .mismatches
                    .push(format!("{name}: no volume, segyio found one")),
            }
        } else if !c.cell.is_empty() && !grid_like(&c.cell) {
            // One inline or crossline, or several traces per cell (pre-stack): a line.
            t.check(g.volume.is_none(), || {
                format!("{name}: a volume from {} traces", s.traces)
            });
        } else if !c.cell.is_empty() {
            match &g.volume {
                Some(v) => {
                    let cells = v.inlines.len() * v.crosslines.len();
                    t.check(v.gaps == cells - c.cell.len(), || {
                        format!("{name}: {} gaps of {cells}", v.gaps)
                    });
                    for &(il, xl, tr) in &c.cell {
                        let i = v.inlines.iter().position(|&n| n == il);
                        let x = v.crosslines.iter().position(|&n| n == xl);
                        let at = i.zip(x).and_then(|(i, x)| v.trace_at(i, x));
                        t.check(at == Some(tr), || {
                            format!("{name}: cell {il}/{xl}: {at:?} vs {tr}")
                        });
                    }
                }
                None => t.mismatches.push(format!("{name}: no volume")),
            }
        } else if !name.ends_with(".su") {
            t.check(g.volume.is_none(), || {
                format!("{name}: a volume where segyio has a line")
            });
        }
    }
    t
}

/// At least two inlines and two crosslines, one trace per cell, and a regular grid no
/// more than five times the traces (the crate's rule for a volume with gaps).
fn grid_like(cells: &[(i32, i32, usize)]) -> bool {
    use std::collections::BTreeSet;
    let il: BTreeSet<i64> = cells.iter().map(|c| i64::from(c.0)).collect();
    let xl: BTreeSet<i64> = cells.iter().map(|c| i64::from(c.1)).collect();
    let both: BTreeSet<(i32, i32)> = cells.iter().map(|c| (c.0, c.1)).collect();
    let span = |v: &BTreeSet<i64>| {
        let first = *v.first().unwrap();
        let gcd = v.iter().fold(0i64, |g, &x| {
            let (mut a, mut b) = (g, x - first);
            while b != 0 {
                (a, b) = (b, a % b);
            }
            a
        });
        (v.last().unwrap() - first) / gcd.max(1) + 1
    };
    il.len() >= 2
        && xl.len() >= 2
        && both.len() == cells.len()
        && span(&il).saturating_mul(span(&xl)) <= 5 * cells.len() as i64
}

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

#[test]
fn fixtures_match_segyio() {
    let t = check_dir(&data(), &data().join("expected.tsv"));
    eprintln!(
        "segyio oracle: {} files, {} values, {} mismatches",
        t.files,
        t.values,
        t.mismatches.len()
    );
    assert!(t.files >= 16, "only {} files", t.files);
    assert_eq!(t.refused_too, ["variable.sgy"]);
    assert!(t.opened_anyway.is_empty(), "{:?}", t.opened_anyway);
    assert!(
        t.mismatches.is_empty(),
        "{:#?}",
        &t.mismatches[..t.mismatches.len().min(30)]
    );
}

#[test]
fn corpus_matches_segyio() {
    let (Some(tsv), Some(dir)) = (
        std::env::var_os("SPECULAR_SEGY_ORACLE"),
        std::env::var_os("SPECULAR_SEGY_DIR"),
    ) else {
        return;
    };
    let t = check_dir(Path::new(&dir), Path::new(&tsv));
    eprintln!(
        "segyio oracle (corpus): {} files, {} values, {} mismatches, {} subnormal IBM samples \
         segyio misreads, segyio wrong on {:?}; segyio refused {} that are refused here too \
         {:?}, and {} read here anyway {:?}",
        t.files,
        t.values,
        t.mismatches.len(),
        t.subnormal,
        t.segyio_wrong,
        t.refused_too.len(),
        t.refused_too,
        t.opened_anyway.len(),
        t.opened_anyway
    );
    assert!(
        t.mismatches.is_empty(),
        "{:#?}",
        &t.mismatches[..t.mismatches.len().min(30)]
    );
}

#[test]
fn depth_and_gaps() {
    let s = Segy::open(&data().join("depth_volume.sgy"), false).unwrap();
    assert_eq!(s.domain, specular_segy::Domain::Depth);
    let s = Segy::open(&data().join("gaps_volume.sgy"), false).unwrap();
    assert_eq!(s.domain, specular_segy::Domain::Time);
    let v = s.geometry().unwrap().volume.unwrap();
    // Inline 20 is missing: a whole gap plane on the step-2 grid.
    let i20 = v.inlines.iter().position(|&n| n == 20).unwrap();
    assert!((0..v.crosslines.len()).all(|x| v.trace_at(i20, x).is_none()));
    let sec = v.inline_section(&s, i20).unwrap();
    assert!(sec.iter().all(|x| x.is_nan()));
    let all = v.read(&s, |_| true).unwrap().unwrap();
    assert_eq!(all.len(), v.inlines.len() * v.crosslines.len() * s.samples);
}
