//! Public TDMS files written by LabVIEW and NI DIAdem (tools/corpus/fetch-oracle-samples.sh tdms)
//! against npTDMS: properties, channel lengths, values, raw and DAQmx scaler data, as dumped by
//! gen/make_samples.py. Reads `SPECULAR_ORACLE_SAMPLES_CORPUS`/tdms, else
//! `$CARGO_TARGET_DIR/corpus/oracle-samples/tdms`; skips when absent. Run with
//! `cargo test -p specular-tdms --test oracle -- --ignored --nocapture`.

mod common;

use std::path::PathBuf;

use common::{Tally, compare};

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE_SAMPLES_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(|t| PathBuf::from(t).join("corpus/oracle-samples"))
        })?;
    let dir = base.join("tdms");
    dir.join("expect.tsv").is_file().then_some(dir)
}

#[test]
#[ignore]
fn corpus_matches_nptdms() {
    let Some(dir) = corpus() else {
        eprintln!(
            "oracle-samples tdms corpus missing: run tools/corpus/fetch-oracle-samples.sh tdms"
        );
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    let mut t = Tally::default();
    let mut failed = Vec::new();
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let name = f[0];
        let data = std::fs::read(dir.join("files").join(name)).unwrap();
        if f[1] == "error" {
            // npTDMS refuses it: ours must not panic either way.
            let ok = specular_tdms::parse(&data).is_ok();
            eprintln!("  {name}: npTDMS fails ({}); ours parses: {ok}", f[2]);
            println!("ORACLE\tTDMS\t{name}\tskip");
            continue;
        }
        let text = std::fs::read_to_string(dir.join("expect").join(f[2])).unwrap();
        let exp: serde_json::Value = serde_json::from_str(&text).unwrap();
        let before = t.mismatches;
        compare(&mut t, name, &data, &exp);
        let ok = t.mismatches == before;
        println!("ORACLE\tTDMS\t{name}\t{}", if ok { "pass" } else { "fail" });
        if !ok {
            failed.push(name.to_owned());
        }
    }
    eprintln!(
        "tdms oracle: {} files, {} channels, {} values, {} mismatches",
        t.files, t.channels, t.values, t.mismatches
    );
    assert!(failed.is_empty(), "{failed:?}\n{:#?}", t.notes);
}
