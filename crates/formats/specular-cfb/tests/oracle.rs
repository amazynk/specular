//! Public compound files (Apache POI's and olefile's test data: Word, Excel, Visio, Project and
//! Outlook files, 512- and 4096-byte sectors, fuzzer cases; tools/corpus/fetch-oracle-samples.sh
//! cfb) against olefile: every storage and stream it lists, each stream's size and FNV-1a 64 hash.
//! Reads `SPECULAR_ORACLE_SAMPLES_CORPUS`/cfb, else `$CARGO_TARGET_DIR/corpus/oracle-samples/cfb`;
//! skips when absent. Run with `cargo test -p specular-cfb --test oracle -- --ignored --nocapture`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use specular_cfb::{Cfb, Kind};

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE_SAMPLES_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(|t| PathBuf::from(t).join("corpus/oracle-samples"))
        })?;
    let dir = base.join("cfb");
    dir.join("expect.tsv").is_file().then_some(dir)
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xCBF2_9CE4_8422_2325, |h, &x| {
        (h ^ u64::from(x)).wrapping_mul(0x0100_0000_01B3)
    })
}

#[test]
#[ignore]
fn corpus_matches_olefile() {
    let Some(dir) = corpus() else {
        eprintln!(
            "oracle-samples cfb corpus missing: run tools/corpus/fetch-oracle-samples.sh cfb"
        );
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    let mut files: BTreeMap<&str, Vec<Vec<&str>>> = BTreeMap::new();
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        files.entry(f[0]).or_default().push(f[1..].to_vec());
    }
    let (mut streams, mut failures) = (0, Vec::new());
    for (name, want) in &files {
        let data = std::fs::read(dir.join("files").join(name)).unwrap();
        if want[0][0] == "error" {
            // Both refuse: they agree. olefile alone fails: no reference (deep-data.bin:
            // RecursionError on 400 nested storages, which ours lists to 64 levels).
            let ours = Cfb::open(&data).map(|_| ()).map_err(|e| e.to_string());
            eprintln!("  {name}: olefile fails ({}); ours: {ours:?}", want[0][1]);
            let v = if ours.is_err() { "pass" } else { "skip" };
            println!("ORACLE\tCFB\t{name}\t{v}");
            continue;
        }
        let r = check(&data, want, &mut streams);
        println!(
            "ORACLE\tCFB\t{name}\t{}",
            if r.is_ok() { "pass" } else { "fail" }
        );
        if let Err(e) = r {
            failures.push(format!("{name}: {e}"));
        }
    }
    eprintln!(
        "cfb oracle: {} files, {streams} streams, {} failed",
        files.len(),
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

fn check(data: &[u8], want: &[Vec<&str>], streams: &mut usize) -> Result<(), String> {
    let c = Cfb::open(data).map_err(|e| e.to_string())?;
    let mut ours: Vec<(String, &str)> = c
        .list()
        .filter(|(_, e)| e.kind != Kind::Root)
        .map(|(p, e)| {
            let k = if e.kind == Kind::Stream {
                "stream"
            } else {
                "storage"
            };
            (p.to_ascii_lowercase(), k)
        })
        .collect();
    ours.sort();
    let mut theirs: Vec<(String, &str)> = want
        .iter()
        .map(|w| (w[1].to_ascii_lowercase(), w[0]))
        .collect();
    theirs.sort();
    if ours != theirs {
        let extra: Vec<_> = ours.iter().filter(|x| !theirs.contains(x)).collect();
        let missing: Vec<_> = theirs.iter().filter(|x| !ours.contains(x)).collect();
        return Err(format!("listing: extra {extra:?}, missing {missing:?}"));
    }
    for w in want.iter().filter(|w| w[0] == "stream") {
        let (size, hash): (u64, u64) = (
            w[2].parse().unwrap(),
            u64::from_str_radix(w[3], 16).unwrap(),
        );
        let declared: u64 = w[4].parse().unwrap();
        *streams += 1;
        let b = match c.read(w[1]) {
            Ok(b) => b,
            // Damaged streams: olefile returns no bytes for a stream it cannot place (a start
            // sector past the file, an empty mini stream), or whatever its chain holds when
            // the declared size exceeds the file; ours refuses the stream. Both see damage.
            Err(e) if size == 0 && declared > 0 || declared > data.len() as u64 => {
                eprintln!(
                    "  {}: ours refuses ({e}); olefile reads {size} of {declared} bytes",
                    w[1]
                );
                continue;
            }
            Err(e) => return Err(format!("{}: {e}", w[1])),
        };
        if b.len() as u64 != size || fnv(&b) != hash {
            return Err(format!(
                "{}: {} bytes (hash {:016x}) vs olefile {size} ({hash:016x})",
                w[1],
                b.len(),
                fnv(&b)
            ));
        }
    }
    Ok(())
}
