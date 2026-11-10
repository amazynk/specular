//! Real PDS3, PDS4 and VICAR products and GDAL's VICAR and PDS4 test files (tools/corpus/
//! fetch-oracle-samples.sh planetary) against pdr (PDS3, PDS4: scaled values, special constants
//! masked) and rms-vicar (VICAR): every band's values, in the oracle's stored order turned to ours
//! by the layout's display flips; complex samples as their magnitude. GDAL's own expected checksums
//! (its autotest) cover the VICAR files rms-vicar misreads. Reads
//! `SPECULAR_ORACLE_SAMPLES_CORPUS`/planetary, else
//! `$CARGO_TARGET_DIR/corpus/oracle-samples/planetary`; skips when absent. Run with `cargo test -p
//! specular-planetary --test oracle -- --ignored --nocapture`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specular_planetary::{Layout, Sample, pds3, pds4, vicar};

/// rms-vicar's readings that are wrong, with the evidence: its rows are not compared; GDAL's
/// checksums are.
const RMS_VICAR_BUGS: [(&str, &str); 4] = [
    (
        "vicar_byte_basic.vic",
        "ignores COMPRESS='BASIC': returns the compressed bytes as pixels",
    ),
    (
        "vicar_byte_basic2.vic",
        "ignores COMPRESS='BASIC2': returns the compressed bytes as pixels",
    ),
    (
        "vicar_int16_basic2.vic",
        "ignores COMPRESS='BASIC2': returns the compressed bytes as pixels",
    ),
    (
        "vicar_vax_cfloat32.vic",
        "reads REALFMT='VAX' complex samples as IEEE (values near 1e-34; GDAL: checksum 226)",
    ),
];

fn corpus() -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_ORACLE_SAMPLES_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(|t| PathBuf::from(t).join("corpus/oracle-samples"))
        })?;
    let dir = base.join("planetary");
    dir.join("expect.tsv").is_file().then_some(dir)
}

/// A file beside the label, whatever the case of its name.
fn beside(dir: &Path, name: &str) -> Vec<u8> {
    let want = name.to_ascii_lowercase();
    let p = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase()
                == want
        })
        .unwrap_or_else(|| panic!("{name} not beside the label"));
    std::fs::read(p).unwrap()
}

fn format_of(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".xml") {
        "PDS4"
    } else if lower.ends_with(".lbl") {
        "PDS3"
    } else {
        "VICAR"
    }
}

/// Our first image's layout and data bytes, by the file's kind.
fn ours(dir: &Path, name: &str) -> Result<(Layout, Vec<u8>), String> {
    let files = dir.join("files");
    let bytes = std::fs::read(files.join(name)).unwrap();
    match format_of(name) {
        "VICAR" => {
            let img = vicar::open(&bytes, 0).map_err(|e| e.to_string())?;
            let data = img.decoded.clone().unwrap_or(bytes);
            Ok((img.layout, data))
        }
        "PDS3" => {
            let p = pds3::parse(&bytes).map_err(|e| e.to_string())?;
            let img = p.images.into_iter().next().ok_or("no image")?;
            let data = match &img.pointer.file {
                Some(f) => beside(&files, f),
                None => bytes,
            };
            Ok((img.layout, data))
        }
        _ => {
            let p = pds4::parse(&bytes).map_err(|e| e.to_string())?;
            let img = p.images.into_iter().next().ok_or("no image")?;
            Ok((img.layout, beside(&files, &img.file)))
        }
    }
}

#[test]
#[ignore]
fn corpus_matches_pdr_and_rms_vicar() {
    let Some(dir) = corpus() else {
        eprintln!(
            "oracle-samples planetary corpus missing: run tools/corpus/fetch-oracle-samples.sh planetary"
        );
        return;
    };
    let tsv = std::fs::read_to_string(dir.join("expect.tsv")).unwrap();
    // file -> verdicts of its rows (None: no reference)
    let mut per: BTreeMap<String, Vec<Option<Result<(), String>>>> = BTreeMap::new();
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let name = f[0];
        let v = per.entry(name.to_owned()).or_default();
        if !dir.join("files").join(name).is_file() {
            continue;
        }
        let known = RMS_VICAR_BUGS.iter().find(|(n, _)| *n == name);
        let verdict = match (f[1], known) {
            ("gdal", _) => Some(ours(&dir, name).and_then(|(l, d)| gdal_checksum(&l, &d, &f))),
            ("error", _) => {
                let r = ours(&dir, name).map(|_| ());
                eprintln!("  {name}: oracle fails ({}); ours: {r:?}", f[2]);
                None
            }
            (_, Some((_, why))) => {
                eprintln!("  {name}: rms-vicar {why}");
                None
            }
            _ => Some(ours(&dir, name).and_then(|(l, d)| check(&dir, &l, &d, &f))),
        };
        v.push(verdict);
    }
    let mut failures = Vec::new();
    for (name, v) in &per {
        let errs: Vec<&String> = v
            .iter()
            .flatten()
            .filter_map(|r| r.as_ref().err())
            .collect();
        let status = if !errs.is_empty() {
            "fail"
        } else if v.iter().all(Option::is_none) {
            "skip"
        } else {
            "pass"
        };
        println!("ORACLE\t{}\t{name}\t{status}", format_of(name));
        failures.extend(errs.into_iter().map(|e| format!("{name}: {e}")));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// GDAL's `GDALRasterBand::Checksum()` of band 1: every value as a 32-bit integer (floats
/// rounded half up, NaN as `i32::MIN`; complex as real then imaginary), summed modulo
/// 65536 through a cycle of eleven primes.
fn gdal_checksum(l: &Layout, data: &[u8], f: &[&str]) -> Result<(), String> {
    const PRIMES: [i32; 11] = [7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43];
    let want: i32 = f[2].parse().unwrap();
    let int = |v: f64| -> i32 {
        if !v.is_finite() {
            i32::MIN
        } else {
            (v + 0.5).floor().clamp(-2_147_483_647.0, 2_147_483_647.0) as i32
        }
    };
    let values: Vec<i32> = match l.sample {
        Sample::Complex { bytes, real } => {
            let half = Sample::Float { bytes, real };
            let mut out = Vec::new();
            for y in 0..u64::from(l.height) {
                for x in 0..u64::from(l.width) {
                    let at = (l.offset + y * l.line_stride + x * l.sample_stride) as usize;
                    let b = usize::from(bytes);
                    out.push(int(half.read(&data[at..at + b])));
                    out.push(int(half.read(&data[at + b..at + 2 * b])));
                }
            }
            out
        }
        _ => {
            let (band, truncated) = l.band(data, 0);
            if truncated {
                return Err("truncated".into());
            }
            band.iter().map(|&v| int(f64::from(v))).collect()
        }
    };
    let (mut sum, mut p) = (0i32, 0);
    for v in values {
        sum = (sum + v % PRIMES[p]) & 0xFFFF;
        p = (p + 1) % 11;
    }
    if sum != want {
        return Err(format!(
            "GDAL checksum {sum}, GDAL's autotest expects {want}"
        ));
    }
    Ok(())
}

/// ISIS's 16-bit special pixel values (NULL, LRS, LIS, HIS, HRS), which pdr masks in PDS3
/// images whose labels name no special constants.
const ISIS_SPECIAL_16: [f32; 5] = [-32768.0, -32767.0, -32766.0, -32765.0, -32764.0];

fn check(dir: &Path, l: &Layout, data: &[u8], f: &[&str]) -> Result<(), String> {
    let (b, h, w): (u32, u32, u32) = (
        f[2].parse().unwrap(),
        f[3].parse().unwrap(),
        f[4].parse().unwrap(),
    );
    let complex = f[5] == "complex";
    let bands = if complex { b / 2 } else { b };
    if (l.bands, l.height, l.width) != (bands, h, w) {
        return Err(format!(
            "{}x{}x{} vs oracle {bands}x{h}x{w}",
            l.bands, l.height, l.width
        ));
    }
    let raw = std::fs::read(dir.join("expect").join(f[6])).unwrap();
    let want: Vec<f32> = raw
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let (w, h) = (w as usize, h as usize);
    let plane = w * h;
    let (mut isis, mut declared) = (0, 0);
    for band in 0..bands {
        let (got, truncated) = l.band(data, band);
        if truncated {
            return Err(format!("band {band}: truncated"));
        }
        let mut bad = 0;
        for y in 0..h {
            for x in 0..w {
                // Ours is in display order; the oracle's in stored order.
                let sy = if l.flip_y { h - 1 - y } else { y };
                let sx = if l.flip_x { w - 1 - x } else { x };
                let at = sy * w + sx;
                let o = if complex {
                    let base = 2 * band as usize * plane;
                    f64::from(want[base + at]).hypot(f64::from(want[base + plane + at])) as f32
                } else {
                    want[band as usize * plane + at]
                };
                let g = got[y * w + x];
                if (g.is_nan() && o.is_nan())
                    || (g - o).abs() <= 1e-6 * g.abs().max(o.abs()).max(1.0)
                {
                    continue;
                }
                if o.is_nan() && ISIS_SPECIAL_16.contains(&g) && l.nodata.is_empty() {
                    isis += 1;
                    continue;
                }
                // Ours masks a constant the label declares (PDS4 Special_Constants); pdr,
                // through pds4_tools, left it (byte_pds4_cart_1700.xml: missing_constant 74).
                if g.is_nan() && l.nodata.values.contains(&f64::from(o)) {
                    declared += 1;
                    continue;
                }
                if bad < 3 {
                    eprintln!("    band {band} ({x},{y}): ours {g}, oracle {o}");
                }
                bad += 1;
            }
        }
        if bad > 0 {
            return Err(format!("band {band}: {bad} of {plane} values differ"));
        }
    }
    if isis + declared > 0 {
        eprintln!(
            "  {}: {isis} ISIS special values pdr masks, {declared} declared constants ours masks",
            f[0]
        );
    }
    Ok(())
}
