//! The committed fixtures against nptdms's reading of them, and a random corpus
//! (`SPECULAR_TDMS_CORPUS=dir`, made by `gen/make_samples.py corpus`).

mod common;

use std::path::{Path, PathBuf};

use common::{Tally, compare};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn json(p: &Path) -> serde_json::Value {
    let s = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    serde_json::from_str(&s).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn fixtures_match_nptdms() {
    let dir = data_dir();
    let mut t = Tally::default();
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "tdms"))
        .collect();
    names.sort();
    for p in &names {
        let stem = p.file_stem().unwrap().to_str().unwrap();
        let data = std::fs::read(p).unwrap();
        compare(
            &mut t,
            stem,
            &data,
            &json(&dir.join("expected").join(format!("{stem}.json"))),
        );
    }
    println!("{t:#?}");
    assert!(names.len() >= 12);
    assert_eq!(t.files, names.len());
    assert_eq!(t.mismatches, 0, "{:#?}", t.notes);
}

#[test]
fn corpus_matches_nptdms() {
    let Some(dir) = std::env::var_os("SPECULAR_TDMS_CORPUS") else {
        eprintln!("SPECULAR_TDMS_CORPUS not set: skipped");
        return;
    };
    let dir = PathBuf::from(dir);
    let mut t = Tally::default();
    let mut n = 0;
    let t0 = std::time::Instant::now();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|e| e != "tdms") {
            continue;
        }
        n += 1;
        let data = std::fs::read(&p).unwrap();
        let exp = json(&PathBuf::from(format!("{}.json", p.display())));
        compare(
            &mut t,
            &p.file_name().unwrap().to_string_lossy(),
            &data,
            &exp,
        );
    }
    // Files nptdms itself fails on (complex interleaved data, several DAQmx buffers in
    // repeated chunks): ours must take them without panicking.
    let mut refused = (0, 0);
    for e in std::fs::read_dir(dir.join("refused")).into_iter().flatten() {
        let p = e.unwrap().path();
        let data = std::fs::read(&p).unwrap();
        refused.0 += 1;
        if let Ok(f) = specular_tdms::parse(&data) {
            refused.1 += 1;
            for c in f.groups.iter().flat_map(|g| &g.channels) {
                let mut buf = vec![0f64; c.len as usize];
                let _ = c.read(&data, 0, &mut buf);
            }
        }
    }
    println!(
        "corpus: {} files nptdms fails on, {} of them parsed",
        refused.0, refused.1
    );
    println!(
        "corpus: {n} files, {} parsed, {} channels, {} values, {} mismatches, {:.0} ms",
        t.files,
        t.channels,
        t.values,
        t.mismatches,
        t0.elapsed().as_secs_f64() * 1e3
    );
    assert_eq!(t.mismatches, 0, "{:#?}", t.notes);
}

#[test]
fn details() {
    let dir = data_dir();
    let f = specular_tdms::parse(&std::fs::read(dir.join("writer.tdms")).unwrap()).unwrap();
    let g = &f.groups[0];
    let c = g.channels.iter().find(|c| c.name == "f64").unwrap();
    assert_eq!(c.unit(), Some("V"));
    assert_eq!(c.waveform(), Some((0.5, 0.001, Some("s".to_owned()))));
    assert_eq!(c.dtype.describe(), "64-bit float");
    let when = f.property("when").unwrap();
    assert_eq!(when.text(), "2024-05-01T12:00:00.25Z");
    let text = g.channels.iter().find(|c| c.name == "text").unwrap();
    assert!(!text.is_numeric());
    assert_eq!(text.strings(&[], 0).unwrap().len(), 0);
    let be = std::fs::read(dir.join("big_endian.tdms")).unwrap();
    let f = specular_tdms::parse(&be).unwrap();
    assert!(f.big_endian && !f.interleaved && !f.daqmx && f.segments == 2);
    let f = specular_tdms::parse(&std::fs::read(dir.join("daqmx.tdms")).unwrap()).unwrap();
    assert!(f.daqmx);
    let c = &f.groups[0].channels[0];
    assert!(c.is_scaled() && c.is_numeric());
    assert_eq!(c.dtype.describe(), "DAQmx raw");
    let f = specular_tdms::parse(&std::fs::read(dir.join("incomplete.tdms")).unwrap()).unwrap();
    assert!(f.truncated);
    let f = specular_tdms::parse(&std::fs::read(dir.join("scaled.tdms")).unwrap()).unwrap();
    let thermo = f.groups[0]
        .channels
        .iter()
        .find(|c| c.name == "thermo")
        .unwrap();
    assert_eq!(
        thermo.scaling_problem(),
        Some("TDMS Thermocouple scaling is not supported")
    );
    // Partial reads and reads past the end.
    let data = std::fs::read(dir.join("chunks.tdms")).unwrap();
    let f = specular_tdms::parse(&data).unwrap();
    let x = &f.groups[0].channels[0];
    let mut all = vec![0f64; x.len as usize];
    x.read(&data, 0, &mut all).unwrap();
    for start in 0..x.len {
        let mut part = vec![0f64; (x.len - start) as usize];
        x.read(&data, start, &mut part).unwrap();
        assert_eq!(part[..], all[start as usize..]);
    }
    let mut one = [0f64; 1];
    assert!(x.read(&data, x.len, &mut one).is_err());
}

#[test]
fn every_prefix_and_bit_flip_is_safe() {
    let dir = data_dir();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|e| e != "tdms") {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        let exercise = |d: &[u8]| {
            if let Ok(f) = specular_tdms::parse(d) {
                for g in &f.groups {
                    for c in &g.channels {
                        let mut buf = vec![0f64; (c.len as usize).min(4096)];
                        let _ = c.read(d, 0, &mut buf);
                        let _ = c.strings(d, 64);
                    }
                }
            }
        };
        for n in 0..data.len() {
            exercise(&data[..n]);
        }
        let mut d = data.clone();
        for i in 0..d.len() {
            for bit in [0u8, 3, 7] {
                d[i] ^= 1 << bit;
                exercise(&d);
                d[i] ^= 1 << bit;
            }
        }
    }
}
