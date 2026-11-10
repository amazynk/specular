//! Real SER captures (PlanetarySystemStacker's sample videos; tools/corpus/fetch-oracle4.sh
//! ser) against FFmpeg's ser demuxer (every frame's samples: sum, min, max and a
//! position-weighted sum) and a header and trailer reader written from the SER v3 description
//! (size, depth, colour, frame count, first and last timestamp). Reads
//! `SPECULAR_ORACLE4_CORPUS`/ser, else `$CARGO_TARGET_DIR/corpus/oracle4/ser`; skips when
//! absent. Run with `cargo test -p specular-ser --test oracle -- --ignored --nocapture`.

use std::collections::BTreeMap;
use std::path::PathBuf;

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE4_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| PathBuf::from(t).join("corpus/oracle4"))
        })?;
    let dir = base.join("ser");
    dir.join("expect.tsv").is_file().then_some(dir)
}

#[test]
#[ignore]
fn corpus_matches_ffmpeg() {
    let Some(dir) = corpus() else {
        eprintln!("oracle4 ser corpus missing: run tools/corpus/fetch-oracle4.sh ser");
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    let mut files: BTreeMap<&str, Vec<Vec<&str>>> = BTreeMap::new();
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        files.entry(f[0]).or_default().push(f[1..].to_vec());
    }
    let mut failures = Vec::new();
    for (name, rows) in &files {
        let data = std::fs::read(dir.join("files").join(name)).unwrap();
        let r = check(&data, rows);
        println!(
            "ORACLE\tSER\t{name}\t{}",
            if r.is_ok() { "pass" } else { "fail" }
        );
        if let Err(e) = r {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

fn check(data: &[u8], rows: &[Vec<&str>]) -> Result<(), String> {
    let mut s = specular_ser::parse(data).map_err(|e| e.to_string())?;
    s.settle_order(data);
    let h = &rows[0];
    assert_eq!(h[0], "header");
    let n = |i: usize| h[i].parse::<i64>().unwrap();
    let got = (
        i64::from(s.colour_id),
        i64::from(s.width),
        i64::from(s.height),
        i64::from(s.depth),
        i64::from(s.declared_frames),
        i64::from(s.frames),
    );
    if got != (n(1), n(2), n(3), n(4), n(5), n(6)) {
        return Err(format!("header {got:?} vs {:?}", &h[1..7]));
    }
    let ts = match (s.timestamps.first(), s.timestamps.last()) {
        (Some(a), Some(b)) => format!("{a},{b}"),
        _ => "-".into(),
    };
    if ts != h[7] {
        return Err(format!("timestamps {ts} vs {}", h[7]));
    }
    let mut buf = Vec::new();
    for r in &rows[1..] {
        let i: u32 = r[0].parse().unwrap();
        s.samples(data, i, &mut buf).ok_or("frame missing")?;
        let sum: u64 = buf.iter().map(|&v| u64::from(v)).sum();
        let lo = buf.iter().copied().min().unwrap_or(0);
        let hi = buf.iter().copied().max().unwrap_or(0);
        let weighted = buf.iter().enumerate().fold(0u64, |acc, (k, &v)| {
            acc.wrapping_add(u64::from(v) * (k as u64 % 65521 + 1))
        });
        let got = format!("{sum}\t{lo}\t{hi}\t{weighted}");
        let want = r[1..5].join("\t");
        if got != want {
            return Err(format!("frame {i}: {got} vs FFmpeg {want}"));
        }
    }
    Ok(())
}
