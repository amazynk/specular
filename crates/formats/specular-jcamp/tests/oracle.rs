//! Our reading of every file against the oracles' (tests/oracle.py): the `jcamp` package and
//! nmrglue for JCAMP-DX, spc_spectra for SPC.
//!
//! ```text
//! target/pcb-spectra/venv/bin/python -I crates/formats/specular-jcamp/tests/oracle.py \
//!     EXPECT tools/corpus/pcb-spectra/jcamp crates/formats/specular-jcamp/tests/data
//! SPECULAR_JCAMP_EXPECT=EXPECT cargo test -p specular-jcamp --test oracle -- --ignored --nocapture
//! ```
//!
//! Values agree within 1e-9 of the array's largest magnitude. An abscissa we show in ppm
//! where the oracle has Hz is compared up to the scale between them. Known disagreements
//! (oracle bugs) are listed in [`KNOWN`] with the reason.

use serde_json::Value;
use specular_spectra::Spectra;

/// Files where an oracle is wrong, and why (checked by hand).
const KNOWN: &[(&str, &str, &str)] = &[
    (
        "dupinc2.jdx",
        "jcamp",
        "jcamp drops the first ordinate of every ASDF line, also after lines not ending in \
         DIF form: 3731 ordinates against the file's ##NPOINTS=3734, which we read",
    ),
    (
        "o05.jdx",
        "jcamp",
        "jcamp decides AFFN or ASDF on the first line only and loses 1172 ordinates; nmrglue \
         and ##NPOINTS= agree with our 8192",
    ),
    (
        "xyinc2.jdx",
        "jcamp",
        "the file says ##NPOINTS=298 but holds 350 ordinates (jcamp's own x and y lengths \
         differ); we space 350 points between ##FIRSTX= and ##LASTX=, which the line \
         abscissas confirm",
    ),
    (
        "4d_map.spc",
        "spc",
        "spc_spectra reads the subfile exponent unsigned and resets negative ones to 0; \
         subfile 4 has exponent -1, the specification's signed char",
    ),
    (
        "m_evenz.spc",
        "spc",
        "negative subfile exponent (-1) read as 0 by spc_spectra, as for 4d_map.spc",
    ),
    (
        "negexp.spc",
        "spc",
        "fixture with exponent -2: spc_spectra resets negative exponents to 0 (4x larger)",
    ),
    (
        "truncated.jdx",
        "jcamp",
        "fixture cut short: 20 ordinates of a declared 25; jcamp spaces 25 abscissas for 20 \
         ordinates, we space 20 at the declared step",
    ),
    (
        "ycheck_fail.jdx",
        "jcamp",
        "fixture with a Y-check value one too high: jcamp continues the differences from the \
         wrong check value, we from the previous line's last ordinate (and warn), which is \
         what the generator encoded",
    ),
    (
        "m_xyxy.spc",
        "spc",
        "spc_spectra reads XYXY abscissas as scaled integers; the specification has them as \
         floats (ours: m/z 43.9 where it has 16943.6)",
    ),
    (
        "ms.spc",
        "spc",
        "XYXY abscissas as scaled integers in spc_spectra, as for m_xyxy.spc (ours: m/z 42)",
    ),
];

fn nums(v: &Value) -> Option<Vec<f64>> {
    v.as_array()
        .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
}

/// Why `ours` and `want` differ, if they do.
fn differ(ours: &[f64], want: &[f64]) -> Option<String> {
    if ours.len() != want.len() {
        return Some(format!("{} values, oracle {}", ours.len(), want.len()));
    }
    let scale = want
        .iter()
        .filter(|v| v.is_finite())
        .fold(0.0f64, |m, v| m.max(v.abs()))
        .max(1e-300);
    for (i, (a, b)) in ours.iter().zip(want).enumerate() {
        if a.is_nan() && b.is_nan() {
            continue;
        }
        let d = (a - b).abs();
        if d.is_nan() || d > 1e-9 * scale {
            return Some(format!("value {i}: {a} vs {b}"));
        }
    }
    None
}

/// `ours` against `want` up to a common factor (ppm against Hz).
fn differ_scaled(ours: &[f64], want: &[f64]) -> Option<String> {
    if ours.len() != want.len() || ours.len() < 2 {
        return differ(ours, want);
    }
    let n = ours.len() - 1;
    let k = (want[n] - want[0]) / (ours[n] - ours[0]);
    let scaled: Vec<f64> = ours.iter().map(|v| v * k).collect();
    differ(&scaled, want)
}

#[derive(Default)]
struct Tally {
    matched: usize,
    mismatched: usize,
    skipped: usize,
}

fn sorted_pairs(x: &[f64], y: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut idx: Vec<usize> = (0..x.len()).collect();
    idx.sort_by(|&a, &b| x[a].total_cmp(&x[b]));
    (
        idx.iter().map(|&i| x[i]).collect(),
        idx.iter().map(|&i| y[i]).collect(),
    )
}

fn check_jcamp(s: &Spectra, blocks: &[Value]) -> Result<(), String> {
    // The `jcamp` package reads no peak assignments, and of several blocks outside a LINK
    // block it keeps the last.
    let mut pages: Vec<&specular_spectra::Plot> = s
        .pages
        .iter()
        .filter(|p| {
            !p.notes
                .iter()
                .any(|n| n == specular_jcamp::jcamp::ASSIGNMENTS_NOTE)
        })
        .collect();
    if blocks.len() == 1 && pages.len() > 1 {
        pages = vec![pages[pages.len() - 1]];
    }
    if blocks.len() != pages.len() {
        return Err(format!(
            "{} pages, oracle {} blocks",
            pages.len(),
            blocks.len()
        ));
    }
    for (i, (p, b)) in pages.into_iter().zip(blocks).enumerate() {
        let t = &p.traces[0];
        let (Some(bx), Some(by)) = (nums(&b["x"]), nums(&b["y"])) else {
            return Err("oracle block without data".into());
        };
        let (bx, by, ox, oy) = if t.sticks {
            let (a, b) = sorted_pairs(&bx, &by);
            let (c, d) = sorted_pairs(&t.x, &t.y);
            (a, b, c, d)
        } else {
            (bx, by, t.x.clone(), t.y.clone())
        };
        if let Some(e) = differ(&oy, &by) {
            return Err(format!("page {i} y: {e}"));
        }
        let hz = b["xunits"]
            .as_str()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("HZ");
        let e = if p.x.unit == "ppm" && hz {
            differ_scaled(&ox, &bx)
        } else {
            differ(&ox, &bx)
        };
        if let Some(e) = e {
            return Err(format!("page {i} x: {e}"));
        }
    }
    Ok(())
}

#[test]
#[ignore]
fn corpus_matches_oracles() {
    // Starts the `ORACLE` lines on their own line (libtest prints `test NAME ... ` first).
    println!();
    let Some(dir) = std::env::var_os("SPECULAR_JCAMP_EXPECT") else {
        eprintln!("SPECULAR_JCAMP_EXPECT not set");
        return;
    };
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("expect dir")
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    let (mut jc, mut ng, mut sp) = (Tally::default(), Tally::default(), Tally::default());
    let mut ours_failed = 0;
    let mut unexpected = Vec::new();
    let mut unchecked: Vec<String> = Vec::new();
    for e in &entries {
        let v: Value = serde_json::from_slice(&std::fs::read(e).expect("reads")).expect("json");
        let path = v["file"].as_str().expect("file").to_string();
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let bytes = std::fs::read(&path).expect("input");
        let fmt = if v.get("spc").is_some() {
            "SPC"
        } else {
            "JCAMP"
        };
        // Per-file verdict for tools/ci/oracle-pass-rates.sh: skip until an oracle checks it.
        let mut status = "skip";
        let mut report = |t: &mut Tally, oracle: &str, r: Result<(), String>| match r {
            Ok(()) => {
                t.matched += 1;
                if status == "skip" {
                    status = "pass";
                }
            }
            Err(m) => {
                if let Some(k) = KNOWN.iter().find(|k| k.0 == name && k.1 == oracle) {
                    println!("known  {oracle:8} {name}: {m} ({})", k.2);
                    if status == "skip" {
                        status = "pass";
                    }
                } else {
                    unexpected.push(format!("{oracle} {name}"));
                    status = "fail";
                }
                println!("DIFFER {oracle:8} {name}: {m}");
                t.mismatched += 1;
            }
        };
        if let Some(spc) = v.get("spc") {
            let Some(subs) = spc["subs"].as_array() else {
                println!("skip   spc      {name}: oracle: {}", spc["error"]);
                sp.skipped += 1;
                println!("ORACLE\t{fmt}\t{name}\tskip");
                continue;
            };
            let s = match specular_jcamp::spc::parse(&bytes) {
                Ok(s) => s,
                Err(err) => {
                    println!("FAIL   ours     {name}: {err}");
                    ours_failed += 1;
                    println!("ORACLE\t{fmt}\t{name}\tfail");
                    continue;
                }
            };
            let r = (|| {
                if subs.len() != s.pages.len() {
                    return Err(format!(
                        "{} pages, oracle {} subfiles",
                        s.pages.len(),
                        subs.len()
                    ));
                }
                for (i, (p, sub)) in s.pages.iter().zip(subs).enumerate() {
                    let t = &p.traces[0];
                    let y = nums(&sub["y"]).unwrap_or_default();
                    if let Some(e) = differ(&t.y, &y) {
                        return Err(format!("subfile {i} y: {e}"));
                    }
                    let x = nums(&sub["x"]).unwrap_or_default();
                    if let Some(e) = differ(&t.x, &x) {
                        return Err(format!("subfile {i} x: {e}"));
                    }
                }
                Ok(())
            })();
            report(&mut sp, "spc", r);
            println!("ORACLE\t{fmt}\t{name}\t{status}");
            continue;
        }
        let s = match specular_jcamp::jcamp::parse(&bytes) {
            Ok(s) => Some(s),
            Err(err) => {
                println!("FAIL   ours     {name}: {err}");
                ours_failed += 1;
                None
            }
        };
        let Some(s) = s else {
            // Refused: right when neither oracle finds a spectrum either.
            let theirs = v["jcamp"]["blocks"]
                .as_array()
                .is_some_and(|b| !b.is_empty())
                || nums(&v["nmrglue"]["real"]).is_some();
            let status = if theirs { "fail" } else { "pass" };
            println!("ORACLE\t{fmt}\t{name}\t{status}");
            continue;
        };
        let before = jc.matched + ng.matched + jc.mismatched + ng.mismatched;
        match v["jcamp"]["blocks"].as_array() {
            Some(b) if !b.is_empty() => report(&mut jc, "jcamp", check_jcamp(&s, b)),
            _ => jc.skipped += 1,
        }
        match nums(&v["nmrglue"]["real"]) {
            Some(real) => {
                // nmrglue reads the first spectrum or FID (not peak tables).
                let page = s
                    .pages
                    .iter()
                    .find(|p| !p.traces[0].sticks)
                    .unwrap_or(&s.pages[0]);
                let r = differ(&page.traces[0].y, &real).map_or(Ok(()), Err);
                report(&mut ng, "nmrglue", r);
            }
            None => ng.skipped += 1,
        }
        println!("ORACLE\t{fmt}\t{name}\t{status}");
        if jc.matched + ng.matched + jc.mismatched + ng.mismatched == before {
            unchecked.push(name);
        }
    }
    println!("read by neither JCAMP-DX oracle: {}", unchecked.join(", "));
    for (n, t) in [("jcamp", &jc), ("nmrglue", &ng), ("spc", &sp)] {
        println!(
            "{n}: {} match, {} differ, {} not read by the oracle",
            t.matched, t.mismatched, t.skipped
        );
    }
    println!("ours failed on {ours_failed} files");
    assert!(
        unexpected.is_empty(),
        "unexplained differences: {unexpected:?}"
    );
}
