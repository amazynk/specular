//! Dumps the linear develop (half size, as-shot white balance, linear Rec.709, before any
//! tone curve) of every file under `tools/corpus/large/raw` (or `SPECULAR_RAW_CORPUS`) for
//! `tools/quality/raw_vs_rawpy.py`, which compares it with rawpy (LibRaw) per camera.
//!
//!     SPECULAR_QUALITY_OUT=<dir> cargo test -p specular-raw --test rawpy_dump -- --ignored
//!
//! Writes `<dir>/<n>.rgb` (2x2 means: width and height as u32 LE, then width * height * 3
//! u16 LE) and `<dir>/manifest.tsv`.

use std::io::Write;
use std::path::{Path, PathBuf};

use specular_raw::{DecodeOptions, OutputSize};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

#[test]
#[ignore = "quality runner: needs the large RAW corpus and SPECULAR_QUALITY_OUT"]
fn dump_linear_develop() {
    let Some(out) = std::env::var_os("SPECULAR_QUALITY_OUT").map(PathBuf::from) else {
        eprintln!("SPECULAR_QUALITY_OUT unset; skipping");
        return;
    };
    let root = std::env::var_os("SPECULAR_RAW_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/large/raw")
        });
    let mut paths = Vec::new();
    files(&root, &mut paths);
    paths.sort();
    std::fs::create_dir_all(&out).unwrap();
    let mut manifest = String::from("n\tpath\twidth\theight\tmatrix\twb\tms\tsensor\n");
    // SPECULAR_QUALITY_SIZE=full develops at full size (then 4x4 means); SPECULAR_QUALITY_ONLY
    // keeps the paths that contain it.
    let full = std::env::var("SPECULAR_QUALITY_SIZE").is_ok_and(|v| v == "full");
    let only = std::env::var("SPECULAR_QUALITY_ONLY").unwrap_or_default();
    let opts = DecodeOptions {
        size: if full {
            OutputSize::Full
        } else {
            OutputSize::Half
        },
        ..Default::default()
    };
    let k: u32 = if full { 4 } else { 2 };
    for (n, p) in paths.iter().enumerate() {
        if !p.to_string_lossy().contains(&only) {
            continue;
        }
        let Ok(data) = std::fs::read(p) else { continue };
        if specular_raw::detect(&data).is_none() {
            continue;
        }
        let t = std::time::Instant::now();
        let raw = match specular_raw::parse(&data).and_then(|r| r.decode_raw(&data, &opts)) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}: {e}", p.display());
                continue;
            }
        };
        let facts = format!(
            "wb {:?} black {:?} white {} cpp {} rgb_cam {:?}",
            raw.wb, raw.black, raw.white, raw.cpp, raw.rgb_cam
        );
        let img = match specular_raw::develop(&raw, &opts) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("{}: {e}", p.display());
                continue;
            }
        };
        let ms = t.elapsed().as_millis();
        // Means over k x k blocks keep the dumps small (the runner reduces rawpy's
        // half-size output by 2 x 2 the same way).
        let (w, h) = (img.width / k, img.height / k);
        let mut bytes = Vec::with_capacity(w as usize * h as usize * 6);
        for y in 0..h {
            for x in 0..w {
                for c in 0..3 {
                    let mut s = 0u32;
                    for dy in 0..k {
                        for dx in 0..k {
                            let i = ((y * k + dy) * img.width + x * k + dx) as usize * 3 + c;
                            s += img.data[i] as u32;
                        }
                    }
                    let m = (s + k * k / 2) / (k * k);
                    bytes.extend_from_slice(&(m as u16).to_le_bytes());
                }
            }
        }
        let mut f = std::fs::File::create(out.join(format!("{n}.rgb"))).unwrap();
        f.write_all(&w.to_le_bytes()).unwrap();
        f.write_all(&h.to_le_bytes()).unwrap();
        f.write_all(&bytes).unwrap();
        let rel = p.strip_prefix(&root).unwrap_or(p);
        manifest += &format!(
            "{n}\t{}\t{}\t{}\t{:?}\t{:?}\t{ms}\t{}\n",
            rel.display(),
            w,
            h,
            img.matrix_source,
            img.wb_source,
            facts
        );
    }
    std::fs::write(out.join("manifest.tsv"), manifest).unwrap();
}
