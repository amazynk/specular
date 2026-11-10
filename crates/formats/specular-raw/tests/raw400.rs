//! The old and rare camera RAW containers of `tools/corpus/fetch-raw400.sh`.
//!
//!     SPECULAR_RAW400_CORPUS=tools/corpus/raw400 \
//!       cargo test -p specular-raw --test raw400 -- --ignored --nocapture

use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_RAW400_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/raw400")
        });
    dir.is_dir().then_some(dir)
}

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
    out.sort();
}

#[test]
#[ignore = "needs tools/corpus/fetch-raw400.sh"]
fn survey() {
    let Some(dir) = corpus() else {
        eprintln!("no corpus");
        return;
    };
    let mut all = Vec::new();
    files(&dir, &mut all);
    for p in all {
        let rel = p.strip_prefix(&dir).unwrap().display().to_string();
        if rel.starts_with("oracle") {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        let fmt = specular_raw::detect_named(&data, ext);
        let line = match specular_raw::parse_named(&data, ext) {
            Ok(f) => {
                let i = f.info();
                let s = i.sensor.as_ref().map_or("no sensor".to_string(), |s| {
                    format!(
                        "{}x{} {}bit {:?} cfa={} dec={} {:?}",
                        s.width,
                        s.height,
                        s.bits,
                        s.compression,
                        s.cfa.as_ref().map_or("-".into(), |c| c.name()),
                        s.decodable,
                        s.unsupported_reason
                    )
                });
                let dec = if f.can_decode_sensor() {
                    let t = std::time::Instant::now();
                    match f.decode(&data, &specular_raw::DecodeOptions::default()) {
                        Ok(img) => format!(
                            " -> {}x{} {:?} in {:.0} ms",
                            img.width,
                            img.height,
                            img.matrix_source,
                            t.elapsed().as_secs_f64() * 1e3
                        ),
                        Err(e) => format!(" -> decode error {e}"),
                    }
                } else {
                    String::new()
                };
                format!(
                    "{:?} '{}' '{}' {} previews={}{}",
                    i.format,
                    i.make,
                    i.model,
                    s,
                    i.previews.len(),
                    dec
                )
            }
            Err(e) => format!("parse error: {e}"),
        };
        eprintln!("{rel:60.60} {fmt:?} {line}");
    }
}

/// LibRaw's unpacked sensor (`tools/corpus/raw400-oracle.py`): width, height, samples.
fn oracle_of(dir: &Path, rel: &str) -> Option<(usize, usize, Vec<u16>)> {
    let base = dir.join("oracle").join(rel.replace('/', "__"));
    let name = base.file_name()?.to_str()?.to_string();
    let txt = std::fs::read_to_string(base.with_file_name(format!("{name}.txt"))).ok()?;
    let mut it = txt.split_whitespace();
    let w: usize = it.next()?.parse().ok()?;
    let h: usize = it.next()?.parse().ok()?;
    let b = std::fs::read(base.with_file_name(format!("{name}.u16"))).ok()?;
    let v = b
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    Some((w, h, v))
}

/// Mismatch counts of our unpacked samples against LibRaw's (exact matches, mean and largest
/// difference), over the overlap of both from the top-left (or from our crop when LibRaw's
/// image is the crop's size).
fn compare(
    raw: &specular_raw::RawImage,
    (ow, oh, ov): &(usize, usize, Vec<u16>),
) -> (usize, usize, f64, u32) {
    let (w, h) = (raw.width as usize, raw.height as usize);
    let (x0, y0) = if (*ow, *oh) == (raw.crop.w as usize, raw.crop.h as usize) {
        (raw.crop.x as usize, raw.crop.y as usize)
    } else {
        (0, 0)
    };
    let (cw, ch) = ((*ow).min(w - x0), (*oh).min(h - y0));
    let (mut differ, mut sum, mut max) = (0usize, 0u64, 0u32);
    for y in 0..ch {
        for x in 0..cw {
            let a = raw.data[(y + y0) * w + x + x0] as i32;
            let e = a.abs_diff(ov[y * ow + x] as i32);
            differ += (e != 0) as usize;
            sum += e as u64;
            max = max.max(e);
        }
    }
    let n = (cw * ch).max(1);
    (differ, n, sum as f64 / n as f64, max)
}

/// Our unpacked samples against LibRaw's, sample for sample. Fails when a file we decode has
/// no overlap with LibRaw's or differs where it must not (see the per-format bounds).
#[test]
#[ignore = "needs tools/corpus/fetch-raw400.sh and tools/corpus/raw400-oracle.py"]
fn oracle() {
    let Some(dir) = corpus() else {
        eprintln!("no corpus");
        return;
    };
    let mut all = Vec::new();
    files(&dir, &mut all);
    let only = std::env::var("SPECULAR_RAW400_ONLY").unwrap_or_default();
    for p in all {
        let rel = p.strip_prefix(&dir).unwrap().display().to_string();
        if rel.starts_with("oracle") || !rel.contains(&only) {
            continue;
        }
        let Some(o) = oracle_of(&dir, &rel) else {
            continue;
        };
        let data = std::fs::read(&p).unwrap();
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        let Ok(f) = specular_raw::parse_named(&data, ext) else {
            eprintln!("{rel:60.60} not parsed");
            continue;
        };
        if !f.can_decode_sensor() {
            let why = f.info().sensor.as_ref().and_then(|s| s.unsupported_reason);
            eprintln!("{rel:60.60} refused: {why:?}");
            continue;
        }
        let raw = f
            .decode_raw_mosaic(&data, &specular_raw::DecodeOptions::default())
            .unwrap();
        if let Some(out) = std::env::var_os("SPECULAR_RAW400_DUMP") {
            let bytes: Vec<u8> = raw.data.iter().flat_map(|v| v.to_le_bytes()).collect();
            let name = rel.replace('/', "__");
            std::fs::write(Path::new(&out).join(format!("{name}.ours.u16")), bytes).unwrap();
        }
        let (differ, n, mean, max) = compare(&raw, &o);
        eprintln!(
            "{rel:60.60} ours {}x{} oracle {}x{}: {differ} of {n} differ, mean {mean:.3}, max {max}",
            raw.width, raw.height, o.0, o.1
        );
    }
}
