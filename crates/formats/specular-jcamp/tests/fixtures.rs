//! The synthetic fixtures (tests/gen_fixtures.py) against what the generator wrote into
//! them (tests/data/expect.json).

use std::path::Path;

use serde_json::Value;
use specular_spectra::{Error, Spectra};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .expect("fixture")
}

fn read(name: &str) -> Result<Spectra, Error> {
    let b = data(name);
    if name.ends_with(".spc") {
        specular_jcamp::spc::parse(&b)
    } else {
        specular_jcamp::jcamp::parse(&b)
    }
}

fn same(a: &[f64], b: &Value, what: &str) {
    let b: Vec<f64> = b
        .as_array()
        .expect("array")
        .iter()
        .map(|v| v.as_f64().expect("number"))
        .collect();
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert!(
            (x - y).abs() <= 1e-9 * y.abs().max(1e-6),
            "{what}: value {i}: {x} vs {y}"
        );
    }
}

#[test]
fn fixtures_hold_what_was_written() {
    let expect: Value = serde_json::from_slice(&data("expect.json")).expect("json");
    let mut n = 0;
    for (name, e) in expect.as_object().expect("object") {
        n += 1;
        let sniffed = if name.ends_with(".spc") {
            specular_jcamp::spc::sniff(&data(name))
        } else {
            specular_jcamp::jcamp::sniff(&data(name))
        };
        if let Some(word) = e.get("refused").and_then(Value::as_str) {
            match read(name) {
                Err(err) => {
                    let m = err.to_string();
                    assert!(m.contains(word), "{name}: {m}");
                }
                Ok(_) => panic!("{name}: read, should be refused"),
            }
            continue;
        }
        assert!(sniffed, "{name}: not sniffed");
        let s = read(name).unwrap_or_else(|err| panic!("{name}: {err}"));
        let pages = e["pages"].as_array().expect("pages");
        assert_eq!(s.pages.len(), pages.len(), "{name}: pages");
        for (i, (p, w)) in s.pages.iter().zip(pages).enumerate() {
            let t = &p.traces[0];
            same(&t.x, &w["x"], &format!("{name} page {i} x"));
            same(&t.y, &w["y"], &format!("{name} page {i} y"));
            if let Some(title) = w.get("title").and_then(Value::as_str) {
                assert_eq!(p.title, title, "{name} page {i}");
            }
            if let Some(u) = w.get("xunit").and_then(Value::as_str) {
                assert_eq!(p.x.unit, u, "{name} page {i}");
            }
            if let Some(t) = w.get("xtitle").and_then(Value::as_str) {
                assert_eq!(p.x.title, t, "{name} page {i}");
            }
            if let Some(st) = w.get("sticks").and_then(Value::as_bool) {
                assert_eq!(t.sticks, st, "{name} page {i}");
            }
            if let Some(x) = w.get("peak0").and_then(Value::as_f64) {
                assert_eq!(p.peaks[0].x, x, "{name} page {i}");
            }
        }
        let warnings = s.facts.warnings.join(" | ");
        if let Some(word) = e.get("warning").and_then(Value::as_str) {
            assert!(warnings.contains(word), "{name}: {warnings}");
        } else {
            assert!(warnings.is_empty(), "{name}: {warnings}");
        }
        if let Some(o) = e.get("origin").and_then(Value::as_str) {
            assert_eq!(s.facts.origin.as_deref(), Some(o), "{name}");
        }
        if let Some(o) = e.get("nucleus").and_then(Value::as_str) {
            assert_eq!(s.facts.nucleus.as_deref(), Some(o), "{name}");
        }
        if let Some(o) = e.get("solvent").and_then(Value::as_str) {
            assert_eq!(s.facts.solvent.as_deref(), Some(o), "{name}");
        }
        if let Some(f) = e.get("field").and_then(Value::as_array) {
            let (k, v) = (f[0].as_str().unwrap_or(""), f[1].as_str().unwrap_or(""));
            assert!(
                s.facts.fields.iter().any(|(a, b)| a == k && b == v),
                "{name}: {:?}",
                s.facts.fields
            );
        }
    }
    assert!(n >= 30, "{n} fixtures");
}

#[test]
fn random_bytes_are_not_claimed() {
    // A simple generator: no SPC header and no JCAMP-DX title in noise.
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..2000 {
        let mut b = vec![0u8; 1024];
        for v in &mut b {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *v = x as u8;
        }
        b[1] = [0x4B, 0x4C, 0x4D][(x % 3) as usize];
        assert!(!specular_jcamp::jcamp::sniff(&b));
        // Never panics, whatever the bytes.
        let _ = specular_jcamp::spc::parse(&b);
        let _ = specular_jcamp::jcamp::parse(&b);
    }
}
