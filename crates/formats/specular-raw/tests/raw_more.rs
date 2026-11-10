//! Oracle comparison with rawpy (LibRaw) for the camera RAW corpus of
//! `tools/corpus/fetch-raw-more.sh` (MRW, CRW, X3F, 3FR/FFF, IIQ, MOS, MEF, DCR/KDC, ERF,
//! SRW, RW2 formats 7/8, RWL, ARI, SRF): the unpacked sensor values (expected bit-exact), black
//! and white levels, and loosely the linear develop (colour balance and structure).
//!
//!     SPECULAR_RAWPY=$CARGO_TARGET_DIR/venv/bin/python \
//!       cargo test -p specular-raw --test raw_more -- --ignored --nocapture
//!
//! `SPECULAR_RAW_MORE_CORPUS` overrides the corpus directory, `SPECULAR_RAW_MORE_ONLY` keeps
//! the paths that contain it. Prints one line per file and fails on any sensor mismatch.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use specular_raw::{DecodeOptions, OutputSize, RawImage};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else if !p
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        {
            out.push(p);
        }
    }
}

fn u32le(b: &[u8], i: usize) -> usize {
    u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()) as usize
}

struct Oracle {
    h: usize,
    w: usize,
    c: usize,
    data: Vec<u16>,
    facts: Vec<(String, String)>,
    dev: Option<(usize, usize, Vec<u16>)>,
}

impl Oracle {
    fn fact(&self, k: &str) -> Option<&str> {
        self.facts
            .iter()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v.as_str())
    }
    fn nums(&self, k: &str) -> Vec<f64> {
        self.fact(k)
            .unwrap_or("")
            .split(',')
            .filter_map(|v| v.parse().ok())
            .collect()
    }
}

fn u16s(b: &[u8]) -> Vec<u16> {
    b.chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn oracle(py: &str, file: &Path, tmp: &Path) -> Result<Oracle, String> {
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/quality/rawpy_sensor.py");
    let prefix = tmp.join("oracle");
    let out = Command::new(py)
        .arg("-I")
        .arg(&script)
        .arg(file)
        .arg(&prefix)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().last().unwrap_or("rawpy failed").to_string());
    }
    let facts = String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let raw = std::fs::read(prefix.with_extension("raw")).map_err(|e| e.to_string())?;
    let (h, w, c) = (u32le(&raw, 0), u32le(&raw, 1), u32le(&raw, 2));
    let dev = std::fs::read(prefix.with_extension("dev"))
        .ok()
        .map(|b| (u32le(&b, 0), u32le(&b, 1), u16s(&b[8..])));
    Ok(Oracle {
        h,
        w,
        c,
        data: u16s(&raw[12..]),
        facts,
        dev,
    })
}

/// Mismatching samples between our decode and the oracle, trying small offsets when the
/// stored sizes differ (one side may keep masked borders the other drops). Returns
/// (mismatches, compared, max |diff|, dx, dy).
fn compare_raw(raw: &RawImage, o: &Oracle) -> Option<(u64, u64, u32, i64, i64)> {
    let (w, h, c) = (raw.width as usize, raw.height as usize, raw.cpp as usize);
    if c != o.c {
        return None;
    }
    let cmp = |dx: i64, dy: i64, step: usize| {
        let (mut bad, mut n, mut maxd) = (0u64, 0u64, 0u32);
        for y in (0..h).step_by(step) {
            let oy = y as i64 + dy;
            if oy < 0 || oy as usize >= o.h {
                continue;
            }
            for x in 0..w {
                let ox = x as i64 + dx;
                if ox < 0 || ox as usize >= o.w {
                    continue;
                }
                for k in 0..c {
                    let a = raw.data[(y * w + x) * c + k];
                    let b = o.data[(oy as usize * o.w + ox as usize) * c + k];
                    n += 1;
                    if a != b {
                        bad += 1;
                        maxd = maxd.max((a as i32 - b as i32).unsigned_abs());
                    }
                }
            }
        }
        (bad, n, maxd)
    };
    if w == o.w && h == o.h {
        let (b, n, m) = cmp(0, 0, 1);
        if b > 0 {
            // Where the differences are: inside the visible crop or in the borders.
            let cr = raw.crop;
            let mut inside = 0u64;
            for y in cr.y as usize..(cr.y + cr.h) as usize {
                for x in cr.x as usize..(cr.x + cr.w) as usize {
                    for k in 0..c {
                        let i = (y * w + x) * c + k;
                        inside += (raw.data[i] != o.data[i]) as u64;
                    }
                }
            }
            println!("  {inside} of the {b} mismatches inside the crop {cr:?}");
            if inside == 0 {
                // Masked borders only (e.g. LibRaw leaves parts of the Quattro margins unfilled
                // and corrects AF pixels in a wider margin): counted as matching.
                return Some((0, n, m, 0, 0));
            }
        }
        return Some((b, n, m, 0, 0));
    }
    let (rx, ry) = (
        (w as i64 - o.w as i64).abs().min(256),
        (h as i64 - o.h as i64).abs().min(256),
    );
    let mut best = (u64::MAX, 0i64, 0i64);
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let (b, n, _) = cmp(dx, dy, 97);
            if n > 0 && b < best.0 {
                best = (b, dx, dy);
            }
        }
    }
    let (b, n, m) = cmp(best.1, best.2, 1);
    Some((b, n, m, best.1, best.2))
}

/// 24x24 grid of channel means of an interleaved RGB image.
fn grid(w: usize, h: usize, d: &[u16]) -> Vec<[f64; 3]> {
    const G: usize = 24;
    let mut out = vec![[0f64; 3]; G * G];
    let mut cnt = vec![0f64; G * G];
    for y in 0..h {
        for x in 0..w {
            let i = (y * G / h) * G + x * G / w;
            for c in 0..3 {
                out[i][c] += d[(y * w + x) * 3 + c] as f64;
            }
            cnt[i] += 1.0;
        }
    }
    for (o, n) in out.iter_mut().zip(&cnt) {
        for v in o.iter_mut() {
            *v /= n.max(1.0);
        }
    }
    out
}

/// (R/G and B/G of ours over theirs, luminance correlation of the grids).
fn compare_dev(ours: &specular_raw::RgbImage, o: &Oracle) -> Option<(f64, f64, f64)> {
    let (oh, ow, od) = o.dev.as_ref()?;
    let a = grid(ours.width as usize, ours.height as usize, &ours.data);
    let b = grid(*ow, *oh, od);
    let sum = |g: &[[f64; 3]], c: usize| g.iter().map(|p| p[c]).sum::<f64>();
    let ratio = |c: usize| (sum(&a, c) / sum(&a, 1)) / (sum(&b, c) / sum(&b, 1));
    let la: Vec<f64> = a.iter().map(|p| p[0] + p[1] + p[2]).collect();
    let lb: Vec<f64> = b.iter().map(|p| p[0] + p[1] + p[2]).collect();
    let n = la.len() as f64;
    let (ma, mb) = (la.iter().sum::<f64>() / n, lb.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in la.iter().zip(&lb) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    Some((ratio(0), ratio(2), sab / (saa * sbb).sqrt().max(1e-9)))
}

/// R/G and B/G of the mean linear colour of our develop over those of the largest embedded
/// preview (the camera's own rendering, sRGB decoded to linear): a white-balance check that
/// does not depend on the oracle's colour data.
fn vs_preview(ours: &specular_raw::RgbImage, data: &[u8], info: &specular_raw::RawInfo) -> String {
    let Some(p) = info.largest_preview() else {
        return "n/a".into();
    };
    let Ok(img) = specular_raw::decode_preview(data, p, 512) else {
        return "n/a".into();
    };
    let lin = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let mut theirs = [0f64; 3];
    for px in img.rgba.chunks_exact(4) {
        for c in 0..3 {
            theirs[c] += lin(px[c]);
        }
    }
    let mut mine = [0f64; 3];
    for px in ours.data.chunks_exact(3) {
        for c in 0..3 {
            mine[c] += px[c] as f64;
        }
    }
    format!(
        "R/G {:.3} B/G {:.3}",
        (mine[0] / mine[1]) / (theirs[0] / theirs[1]),
        (mine[2] / mine[1]) / (theirs[2] / theirs[1])
    )
}

#[test]
#[ignore = "oracle runner: needs tools/corpus/fetch-raw-more.sh and SPECULAR_RAWPY"]
fn raw_more_vs_rawpy() {
    let Ok(py) = std::env::var("SPECULAR_RAWPY") else {
        eprintln!("SPECULAR_RAWPY unset; skipping");
        return;
    };
    let root = std::env::var_os("SPECULAR_RAW_MORE_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/raw-more")
        });
    let only = std::env::var("SPECULAR_RAW_MORE_ONLY").unwrap_or_default();
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("raw_more");
    std::fs::create_dir_all(&tmp).unwrap();
    let mut paths = Vec::new();
    files(&root, &mut paths);
    paths.sort();
    let mut failures = Vec::new();
    // One line per file for tools/ci/oracle-pass-rates.sh.
    let line = |rel: &str, status: &str| {
        let ext = Path::new(rel)
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_uppercase())
            .unwrap_or_default();
        println!("ORACLE\tRAW/{ext}\t{rel}\t{status}");
    };
    for p in paths {
        let rel = p.strip_prefix(&root).unwrap_or(&p).display().to_string();
        if !rel.contains(&only) {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        let tp = Instant::now();
        let file = match specular_raw::parse(&data) {
            Ok(f) => f,
            Err(e) => {
                println!("{rel}: parse error {e}");
                line(&rel, "fail");
                failures.push(rel);
                continue;
            }
        };
        let parse_ms = tp.elapsed().as_secs_f64() * 1e3;
        let info = file.info();
        let fmt = info.format.map_or("?", |f| f.name());
        let opts = DecodeOptions::default();
        let t = Instant::now();
        let raw = match file.decode_raw_as_stored(&data, &opts) {
            Ok(r) => r,
            Err(e) => {
                println!(
                    "{rel}: [{fmt}] {} previews, decode error {e}",
                    info.previews.len()
                );
                // A plain refusal (a variant this crate does not decode) is not a failure.
                if matches!(e, specular_raw::RawError::Unsupported(_)) {
                    line(&rel, "skip");
                } else {
                    line(&rel, "fail");
                    failures.push(rel);
                }
                continue;
            }
        };
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let o = match oracle(&py, &p, &tmp) {
            Ok(o) => o,
            Err(e) => {
                println!(
                    "{rel}: [{fmt}] {}x{}x{} in {ms:.0} ms; no oracle ({e})",
                    raw.width, raw.height, raw.cpp
                );
                line(&rel, "skip");
                continue;
            }
        };
        // Four-colour sensors (CMYG Pro70, RGBE F828) are stored converted to RGB; their
        // mosaic before that is what LibRaw's raw_image holds (as in the rawrest test).
        let mosaic = (raw.cpp as usize != o.c)
            .then(|| file.decode_raw_mosaic(&data, &opts).ok())
            .flatten();
        let cmpd = compare_raw(mosaic.as_ref().unwrap_or(&raw), &o);
        let ok = cmpd.is_some_and(|(b, n, ..)| b == 0 && n > 0) && !raw.truncated;
        // Black per 2x2 position from LibRaw's per-colour levels.
        let ob = o.nums("black");
        let pat = o.nums("pattern");
        let pw = o
            .fact("pattern_w")
            .and_then(|v| v.parse().ok())
            .unwrap_or(2usize);
        let their_black: Vec<f64> = (0..4)
            .map(|p| {
                let (y, x) = (p / 2, p % 2);
                let ci = pat
                    .get((y % (pat.len() / pw.max(1)).max(1)) * pw + x % pw.max(1))
                    .copied()
                    .unwrap_or(p as f64) as usize;
                ob.get(ci).copied().unwrap_or(0.0)
            })
            .collect();
        let dev = specular_raw::develop(
            &raw,
            &DecodeOptions {
                size: OutputSize::Half,
                ..Default::default()
            },
        )
        .ok()
        .and_then(|img| {
            println!(
                "{rel}: model {:?} matrix {:?} wb {:?}; vs camera preview {}",
                info.model,
                img.matrix_source,
                img.wb_source,
                vs_preview(&img, &data, info)
            );
            compare_dev(&img, &o)
        });
        println!(
            "{rel}: [{fmt}] {}x{}x{} oracle {}x{}x{} in {ms:.0} ms (parse {parse_ms:.1} ms); raw {}; black {:?} vs {:?}; white {} vs {}; wb {:?} vs {}; dev {}",
            raw.width,
            raw.height,
            raw.cpp,
            o.w,
            o.h,
            o.c,
            match cmpd {
                Some((b, n, m, dx, dy)) =>
                    format!("{b}/{n} mismatches (max {m}) at offset {dx},{dy}"),
                None => "not comparable".into(),
            },
            raw.black,
            their_black,
            raw.white,
            o.fact("white").unwrap_or("?"),
            raw.wb,
            o.fact("wb").unwrap_or("?"),
            match dev {
                Some((r, b, c)) => format!("R/G {r:.3} B/G {b:.3} corr {c:.3}"),
                None => "n/a".into(),
            }
        );
        line(&rel, if ok { "pass" } else { "fail" });
        if !ok {
            failures.push(rel);
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(failures.is_empty(), "failures: {failures:?}");
}
