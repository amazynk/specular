//! Every file of the FLIF corpus (tools/corpus/fetch-editors-flif.sh) against the reference
//! decoder's output (`flif -d` as PAM/PNM). Skips when the corpus is absent.

use specular_flif::{Options, PixelFormat, decode};
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_FLIF_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/editors/flif/flif")
        });
    p.is_dir().then_some(p)
}

/// A PNM/PAM: width, height, channels, maxval, samples.
struct Pnm {
    w: usize,
    h: usize,
    c: usize,
    max: u32,
    v: Vec<u32>,
}

fn read_pnm(path: &Path) -> Option<Pnm> {
    let d = std::fs::read(path).ok()?;
    let mut pos = 0;
    let mut token = |d: &[u8]| -> Option<String> {
        while pos < d.len() && d[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let s = pos;
        while pos < d.len() && !d[pos].is_ascii_whitespace() {
            pos += 1;
        }
        Some(String::from_utf8_lossy(&d[s..pos]).into_owned())
    };
    let magic = token(&d)?;
    let (w, h, c, max);
    if magic == "P7" {
        let (mut ww, mut hh, mut cc, mut mm) = (0, 0, 0, 0);
        loop {
            let t = token(&d)?;
            match t.as_str() {
                "WIDTH" => ww = token(&d)?.parse().ok()?,
                "HEIGHT" => hh = token(&d)?.parse().ok()?,
                "DEPTH" => cc = token(&d)?.parse().ok()?,
                "MAXVAL" => mm = token(&d)?.parse().ok()?,
                "TUPLTYPE" => {
                    token(&d)?;
                }
                "ENDHDR" => break,
                _ => return None,
            }
        }
        (w, h, c, max) = (ww, hh, cc, mm);
    } else {
        c = match magic.as_str() {
            "P5" => 1,
            "P6" => 3,
            _ => return None,
        };
        w = token(&d)?.parse().ok()?;
        h = token(&d)?.parse().ok()?;
        max = token(&d)?.parse().ok()?;
    }
    pos += 1;
    let n = w * h * c;
    let body = d.get(pos..)?;
    let v = if max > 255 {
        body.chunks_exact(2)
            .take(n)
            .map(|b| u32::from(u16::from_be_bytes([b[0], b[1]])))
            .collect()
    } else {
        body.iter()
            .take(n)
            .map(|&b| u32::from(b))
            .collect::<Vec<_>>()
    };
    (v.len() == n).then_some(Pnm { w, h, c, max, v })
}

fn refs(flif: &Path) -> Vec<Pnm> {
    let stem = flif.with_extension("");
    let one = stem.with_extension("pam");
    if let Some(p) = read_pnm(&one) {
        return vec![p];
    }
    let mut out = Vec::new();
    for i in 0..1000 {
        let f = PathBuf::from(format!("{}-{i:03}.pam", stem.display()));
        match read_pnm(&f) {
            Some(p) => out.push(p),
            None => break,
        }
    }
    out
}

/// Our pixels as samples of the reference's maxval and channel count.
fn ours(fmt: PixelFormat, data: &[u8], c: usize, max: u32) -> Vec<u32> {
    let full: u32 = match fmt {
        PixelFormat::Gray8 | PixelFormat::Rgba8 => 255,
        _ => 65535,
    };
    let samples: Vec<u32> = match fmt {
        PixelFormat::Gray8 | PixelFormat::Rgba8 => data.iter().map(|&b| u32::from(b)).collect(),
        _ => data
            .chunks_exact(2)
            .map(|b| u32::from(u16::from_ne_bytes([b[0], b[1]])))
            .collect(),
    };
    let src_c = if matches!(fmt, PixelFormat::Gray8 | PixelFormat::Gray16) {
        1
    } else {
        4
    };
    let mut out = Vec::new();
    for px in samples.chunks_exact(src_c) {
        for &s in &px[..c.min(src_c)] {
            // Back to the stored scale (exact inverse of the rounding up-scale).
            out.push(if max == full {
                s
            } else {
                (s * max + full / 2) / full
            });
        }
    }
    out
}

#[test]
fn reference_decoder_matches() {
    let Some(dir) = corpus() else {
        eprintln!("FLIF corpus absent: skipped");
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "flif"))
        .collect();
    files.sort();
    let (mut pass, mut mismatch, mut errors, mut noref) = (0, 0, 0, 0);
    let mut failures = Vec::new();
    // One line per file for tools/ci/oracle-pass-rates.sh.
    let line = |name: &str, status: &str| println!("ORACLE\tFLIF\t{name}\t{status}");
    for f in &files {
        let data = std::fs::read(f).unwrap();
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        let r = refs(f);
        let img = std::panic::catch_unwind(|| decode(&data, &Options::default()));
        let img = match img {
            Ok(Ok(i)) => i,
            Ok(Err(e)) => {
                if r.is_empty() {
                    noref += 1;
                    line(&name, "skip");
                } else {
                    errors += 1;
                    failures.push(format!("{name}: error {e}"));
                    line(&name, "fail");
                }
                continue;
            }
            Err(_) => {
                errors += 1;
                failures.push(format!("{name}: PANIC"));
                line(&name, "fail");
                continue;
            }
        };
        if r.is_empty() {
            noref += 1;
            line(&name, "skip");
            continue;
        }
        if r.len() != img.frames.len() {
            mismatch += 1;
            line(&name, "fail");
            failures.push(format!(
                "{name}: {} frames, reference {}",
                img.frames.len(),
                r.len()
            ));
            continue;
        }
        let mut bad = None;
        for (fi, (fr, p)) in img.frames.iter().zip(&r).enumerate() {
            if (p.w, p.h) != (img.width as usize, img.height as usize) {
                bad = Some(format!(
                    "size {}x{} vs {}x{}",
                    img.width, img.height, p.w, p.h
                ));
                break;
            }
            let o = ours(img.format, &fr.data, p.c, p.max);
            if o.len() != p.v.len() {
                bad = Some(format!(
                    "channels: ours {} samples, reference {} ({} ch)",
                    o.len(),
                    p.v.len(),
                    p.c
                ));
                break;
            }
            if let Some(i) = o.iter().zip(&p.v).position(|(a, b)| a != b) {
                let n = o.iter().zip(&p.v).filter(|(a, b)| a != b).count();
                let px = i / p.c;
                bad = Some(format!(
                    "frame {fi}: {n} samples differ, first at x={} y={} ch {}: {} vs {}",
                    px % p.w,
                    px / p.w,
                    i % p.c,
                    o[i],
                    p.v[i]
                ));
                break;
            }
        }
        line(&name, if bad.is_none() { "pass" } else { "fail" });
        match bad {
            None => pass += 1,
            Some(b) => {
                mismatch += 1;
                failures.push(format!("{name}: {b}"));
            }
        }
    }
    for l in &failures {
        eprintln!("  {l}");
    }
    eprintln!(
        "FLIF oracle: {} files, {pass} pass, {mismatch} mismatch, {errors} errors, {noref} without reference",
        files.len()
    );
    assert_eq!((mismatch, errors), (0, 0), "{failures:#?}");
}

/// Decode timings of the ~3 MP bench files (`--ignored`; the machine may be loaded).
#[test]
#[ignore]
fn bench_timings() {
    let Some(dir) = corpus() else {
        return;
    };
    for name in [
        "bench-3mp-N.flif",
        "bench-3mp-I.flif",
        "photo0-I.flif",
        "photo0-N.flif",
    ] {
        let Ok(d) = std::fs::read(dir.join(name)) else {
            continue;
        };
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let img = decode(&d, &Options::default()).unwrap();
            best = best.min(t.elapsed().as_secs_f64());
            assert!(!img.frames.is_empty());
        }
        let t = std::time::Instant::now();
        let opts = Options {
            scale: 8,
            ..Options::default()
        };
        let s = decode(&d, &opts).unwrap();
        let scaled = t.elapsed().as_secs_f64();
        eprintln!(
            "{name}: {:.0} ms full, {:.0} ms at 1/8 ({}x{})",
            best * 1e3,
            scaled * 1e3,
            s.width,
            s.height
        );
    }
}
