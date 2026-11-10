//! The planetary corpus (`tools/corpus/fetch-space.sh`) against its oracle: synthetic VICAR,
//! PDS3 and PDS4 files against their known values (and GDAL where it reads them), GDAL's
//! autotest files and NASA PDS products against GDAL (rasterio), fpack files against
//! astropy. Reads SPECULAR_SPACE_CORPUS, else tools/corpus/space; skips without it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::Value as J;
use specular_core::CancelToken;
use specular_decode::{Format, Registry};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_SPACE_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/space"));
    dir.join("expected/SUMMARY.json").is_file().then_some(dir)
}

/// Files the oracle reads that we refuse, with the reason we give (a substring).
const REFUSED: &[(&str, &str)] = &[
    // HiRISE: the image is a JPEG 2000 file (here GDAL's test swaps in a TIFF).
    ("gdal/pds/ESP_013951_1955_RED.LBL", "open that file"),
    // The data are in a ZIP archive.
    ("gdal/pds/PDS_WITH_ZIP_IMG.LBL", "open that file"),
];

/// Files whose values may differ from GDAL's, and why.
const GDAL_DIFFERS: &[(&str, &str)] = &[];

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|e| e == "json")
            && p.file_name().is_some_and(|n| n != "SUMMARY.json")
        {
            out.push(p);
        }
    }
}

#[derive(Default, Debug)]
struct Tally {
    files: usize,
    pixels: u64,
    mismatched: u64,
    /// No-data in our values only (the label's constants GDAL does not apply, rows past the
    /// end of a truncated file).
    ours_only_nodata: u64,
    /// No-data in GDAL's values only (GDAL's own default NULL values).
    oracle_only_nodata: u64,
    refused: usize,
    extra: usize,
}

fn expected_format(rel: &str) -> Option<Format> {
    let name = rel.rsplit('/').next()?;
    if rel.starts_with("fpack/") {
        Some(Format::Fpack)
    } else if name.starts_with("vicar_") || name.ends_with(".vic") {
        Some(Format::Vicar)
    } else if name.starts_with("pds3_") || name.to_ascii_lowercase().ends_with(".lbl") {
        Some(Format::Pds3)
    } else if name.ends_with(".xml") {
        Some(Format::Pds4)
    } else {
        None
    }
}

#[test]
fn space_corpus_matches_the_oracle() {
    let Some(dir) = corpus() else {
        eprintln!("no planetary corpus; skipping (tools/corpus/fetch-space.sh)");
        return;
    };
    let exp = dir.join("expected");
    let mut metas = Vec::new();
    walk(&exp, &mut metas);
    let reg = Registry::new();
    let cancel = CancelToken::new();
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    let mut failures = Vec::new();
    for m in metas {
        let rel = m
            .strip_prefix(&exp)
            .unwrap()
            .to_string_lossy()
            .trim_end_matches(".json")
            .replace('\\', "/");
        let meta: J = serde_json::from_slice(&std::fs::read(&m).unwrap()).unwrap();
        let group = rel.split('/').next().unwrap_or("").to_owned();
        let t = tallies.entry(group).or_default();
        let source = meta["source"].as_str().unwrap_or("none");
        let r = reg.open_path(&dir.join(&rel));
        if source == "none" {
            match &r {
                Ok(s) => {
                    t.extra += 1;
                    eprintln!(
                        "  {rel}: opens ({}), the oracle cannot",
                        s.info().format.name()
                    );
                }
                Err(e) => eprintln!("  {rel}: neither opens ({e})"),
            }
            continue;
        }
        let s = match r {
            Ok(s) => s,
            Err(e) => {
                let msg = e.to_string();
                match REFUSED.iter().find(|(f, _)| *f == rel) {
                    Some((_, why)) if msg.contains(why) => {
                        t.refused += 1;
                        eprintln!("  {rel}: refused as expected ({msg})");
                    }
                    _ => failures.push(format!("{rel}: {msg}")),
                }
                continue;
            }
        };
        if let Some(f) = expected_format(&rel)
            && s.info().format != f
            && !(f == Format::Pds3 && rel.ends_with(".img"))
        {
            failures.push(format!(
                "{rel}: opened as {:?}, expected {f:?}",
                s.info().format
            ));
        }
        let (w, h, nb) = (
            meta["width"].as_u64().unwrap() as u32,
            meta["height"].as_u64().unwrap() as u32,
            meta["bands"].as_u64().unwrap() as u32,
        );
        let sc = s.science().expect("science summary");
        let bands = sc.axes.first().map_or(1, |a| a.len);
        if (s.info().width, s.info().height, bands) != (w, h, nb) {
            failures.push(format!(
                "{rel}: {}×{}×{bands}, oracle {w}×{h}×{nb}",
                s.info().width,
                s.info().height
            ));
            continue;
        }
        let step = meta["step"].as_u64().unwrap() as usize;
        let (sw, sh) = (
            meta["sub_width"].as_u64().unwrap() as usize,
            meta["sub_height"].as_u64().unwrap() as usize,
        );
        let flip = meta["flip_y"].as_bool().unwrap_or(false);
        let raw = std::fs::read(exp.join(format!("{rel}.f32"))).unwrap();
        let want: Vec<f32> = raw
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        t.files += 1;
        let mut bad = 0u64;
        for b in 0..nb {
            let idx: Vec<u32> = if sc.axes.is_empty() { vec![] } else { vec![b] };
            let p = s.nd_plane(&idx, &cancel).unwrap().unwrap();
            for y in 0..sh {
                let sy = y * step;
                let py = if flip { h as usize - 1 - sy } else { sy };
                for x in 0..sw {
                    let got = p.values[py * w as usize + x * step];
                    let exp = want[(b as usize * sh + y) * sw + x];
                    t.pixels += 1;
                    match (got.is_nan(), exp.is_nan()) {
                        (true, true) => {}
                        (true, false) => t.ours_only_nodata += 1,
                        (false, true) => t.oracle_only_nodata += 1,
                        (false, false) => {
                            let tol = 1e-6 * exp.abs().max(1.0);
                            if (got - exp).abs() > tol {
                                bad += 1;
                                if bad <= 3 {
                                    eprintln!("  {rel}: band {b} ({x},{y}): {got} vs {exp}");
                                }
                            }
                        }
                    }
                }
            }
        }
        t.mismatched += bad;
        let known = source == "known" || source == "astropy";
        if bad > 0 && (known || !GDAL_DIFFERS.iter().any(|(f, _)| *f == rel)) {
            failures.push(format!("{rel}: {bad} values differ from {source}"));
        }
    }
    eprintln!("planetary corpus against its oracle:");
    for (g, t) in &tallies {
        eprintln!(
            "  {g:6} {:3} files, {:9} values compared, {} mismatched, {} no-data in ours only, \
             {} in the oracle's only; {} refused as expected, {} opened that the oracle cannot",
            t.files,
            t.pixels,
            t.mismatched,
            t.ours_only_nodata,
            t.oracle_only_nodata,
            t.refused,
            t.extra
        );
    }
    let summary: J =
        serde_json::from_slice(&std::fs::read(exp.join("SUMMARY.json")).unwrap()).unwrap();
    eprintln!(
        "  GDAL itself agrees with {} of {} synthetic files (disagrees on {}, cannot open {})",
        summary["gdal_agrees"],
        summary["synthetic"],
        summary["gdal_disagrees"].as_array().map_or(0, Vec::len),
        summary["gdal_fails"].as_array().map_or(0, Vec::len)
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Open and first-plane timings of the largest corpus images (under whatever load the
/// machine has).
#[test]
fn space_corpus_timings() {
    let Some(dir) = corpus() else { return };
    let reg = Registry::new();
    let cancel = CancelToken::new();
    for rel in [
        "nasa/lola/ldem_16.lbl",
        "nasa/m2020/NLF_0010_0667834979_567EBY_N0030000NCAM00130_00_0LLJ03.xml",
        "nasa/cassini-iss/N1454725799_1.IMG",
    ] {
        let p = dir.join(rel);
        if !p.is_file() {
            continue;
        }
        let t0 = Instant::now();
        let s = reg.open_path(&p).unwrap();
        let open = t0.elapsed();
        let t1 = Instant::now();
        let r = s
            .decode(&specular_decode::DecodeRequest::full(1), &cancel)
            .unwrap();
        let view = t1.elapsed();
        eprintln!(
            "  {rel}: {}×{} open {:.1} ms, first view {:.1} ms",
            r.width,
            r.height,
            open.as_secs_f64() * 1e3,
            view.as_secs_f64() * 1e3
        );
    }
}
