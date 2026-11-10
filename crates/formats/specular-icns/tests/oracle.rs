//! Public ICNS files against Pillow's per-entry readers (`is32`/`il32`/`it32` with their masks;
//! where the PNG / JPEG 2000 entries start and end) and, for the types Pillow lacks (`ICN#`,
//! `icm#`, `icsb`), rust-icns's expected PNGs (tools/corpus/fetch-oracle4.sh icns). Reads
//! `SPECULAR_ORACLE4_CORPUS`/icns, else `$CARGO_TARGET_DIR/corpus/oracle4/icns`; skips when
//! absent. Run with `cargo test -p specular-icns --test oracle -- --ignored --nocapture`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use specular_icns::{Icon, Kind, decode, parse};

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE4_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| PathBuf::from(t).join("corpus/oracle4"))
        })?;
    let dir = base.join("icns");
    dir.join("expect.tsv").is_file().then_some(dir)
}

#[test]
#[ignore]
fn corpus_matches_pillow() {
    let Some(dir) = corpus() else {
        eprintln!("oracle4 icns corpus missing: run tools/corpus/fetch-oracle4.sh icns");
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    // file -> (entries checked, failures)
    let mut per: BTreeMap<String, (u32, Vec<String>)> = BTreeMap::new();
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let (name, cc, what) = (f[0], f[1], f[2]);
        let data = std::fs::read(dir.join("files").join(name)).unwrap();
        let entry = per.entry(name.to_owned()).or_default();
        entry.0 += 1;
        let icns = match parse(&data) {
            Ok(i) => i,
            Err(e) => {
                entry.1.push(format!("{name}: {e}"));
                continue;
            }
        };
        let icon: Option<&Icon> = if cc == "*" {
            icns.icons.first()
        } else {
            icns.icons.iter().find(|i| i.name() == cc)
        };
        let Some(icon) = icon else {
            entry.1.push(format!("{name} {cc}: not listed"));
            continue;
        };
        if let Err(e) = check(&dir, &data, icon, &f[2..]) {
            entry.1.push(format!("{name} {cc} ({what}): {e}"));
        }
    }
    let mut failures = Vec::new();
    for (name, (n, errs)) in &per {
        let ok = if errs.is_empty() { "pass" } else { "fail" };
        println!("ORACLE\tICNS\t{name}\t{ok}");
        eprintln!("  {name}: {n} entries, {} failed", errs.len());
        failures.extend(errs.iter().cloned());
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

fn check(dir: &std::path::Path, data: &[u8], icon: &Icon, f: &[&str]) -> Result<(), String> {
    match f[0] {
        "embedded" => {
            let (start, len): (usize, usize) = (f[1].parse().unwrap(), f[2].parse().unwrap());
            if !matches!(icon.kind, Kind::Png | Kind::Jp2) {
                return Err(format!("kind {:?}", icon.kind));
            }
            if icon.data != (start..start + len) {
                return Err(format!("payload {:?} vs Pillow {start}+{len}", icon.data));
            }
            Ok(())
        }
        "rgba" => {
            let (w, h): (u32, u32) = (f[1].parse().unwrap(), f[2].parse().unwrap());
            let want = std::fs::read(dir.join("expect").join(f[3])).unwrap();
            let img = decode(data, icon).map_err(|e| e.to_string())?;
            if (img.width, img.height) != (w, h) {
                return Err(format!("{}x{} vs {w}x{h}", img.width, img.height));
            }
            let bad = img
                .pixels
                .chunks_exact(4)
                .zip(want.chunks_exact(4))
                // Fully transparent pixels carry no colour.
                .filter(|(a, b)| a != b && !(a[3] == 0 && b[3] == 0))
                .count();
            if bad > 0 {
                return Err(format!("{bad} of {} pixels differ", w * h));
            }
            Ok(())
        }
        other => Err(format!("oracle: {other} {}", f.get(1).unwrap_or(&""))),
    }
}
