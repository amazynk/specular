//! Sweep of the lens-related summary over real files, read-only and in place.
//!
//! ```sh
//! SWEEP_PATHS=tools/corpus/large/raw/Canon \
//!   cargo test -p specular-metadata --test lens_sweep -- --ignored --nocapture
//! ```
//!
//! Prints, per file, the camera model and the lens group (lens, focal range, stabilisation) and
//! the focus mode: nothing identifying (no serials, positions, names or dates). Pass
//! `SWEEP_ANON=1` to print a running number instead of the file name (for private folders),
//! and `SWEEP_DUMP=<dir>` to write each file's Exif TIFF structure there as `<number>.tif` (to
//! compare against exiftool; the structures keep the GPS and serials the files hold).

use std::path::{Path, PathBuf};

use specular_metadata::*;

const EXT: &[&str] = &[
    "cr2", "cr3", "crw", "jpg", "jpeg", "tif", "tiff", "dng", "srw", "arw", "sr2", "rw2", "rwl",
    "orf",
];

fn walk(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_dir() {
        if let Ok(rd) = std::fs::read_dir(p) {
            let mut v: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
            v.sort();
            for c in v {
                walk(&c, out);
            }
        }
    } else if p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXT.contains(&e.to_ascii_lowercase().as_str()))
    {
        out.push(p.to_owned());
    }
}

#[test]
#[ignore]
fn lens_sweep() {
    // Paths, or a file listing them (`SWEEP_LIST`), separated by ':' or newlines.
    let paths = std::env::var("SWEEP_PATHS").ok().or_else(|| {
        std::env::var("SWEEP_LIST")
            .ok()
            .and_then(|f| std::fs::read_to_string(f).ok())
    });
    let Some(paths) = paths else {
        return;
    };
    let anon = std::env::var("SWEEP_ANON").is_ok();
    let dump = std::env::var_os("SWEEP_DUMP").map(PathBuf::from);
    let l = Localizer::new("en".parse().unwrap());
    let mut files = Vec::new();
    for p in paths.split([':', '\n']).filter(|p| !p.trim().is_empty()) {
        walk(Path::new(p.trim()), &mut files);
    }
    for (n, p) in files.iter().enumerate() {
        let Ok(data) = std::fs::read(p) else { continue };
        let exif = if data.starts_with(&[0xFF, 0xD8]) {
            jpeg::scan(&data)
                .ok()
                .and_then(|s| s.exif.map(<[u8]>::to_vec))
        } else {
            specular_decode::exif_tiff(&data).map(|c| c.into_owned())
        };
        let Some(exif) = exif else {
            println!("{}: no exif", name(p, n, anon));
            continue;
        };
        if let Some(d) = &dump {
            std::fs::write(d.join(format!("{n}.tif")), &exif).expect("dump");
        }
        let r = read(
            &MetadataInput {
                exif_tiff: Some(&exif),
                ..Default::default()
            },
            &l,
        );
        let mut line = Vec::new();
        for g in &r.summary {
            for i in &g.items {
                let keep = matches!(
                    (g.key, i.key),
                    ("camera", "camera")
                        | ("lens", _)
                        | ("exposure", "focus_mode")
                        | ("exposure", "focal_length")
                        | ("exposure", "iso")
                        | ("exposure", "aperture")
                        | ("exposure", "shutter")
                );
                if keep {
                    let d = i
                        .detail
                        .as_deref()
                        .map(|d| format!(" ({})", strip_isolates(d)))
                        .unwrap_or_default();
                    line.push(format!("{}={}{d}", i.key, strip_isolates(&i.value)));
                }
            }
        }
        let errs: Vec<&String> = r
            .warnings
            .iter()
            .filter(|w| !w.contains("location"))
            .collect();
        println!("{}: {} {:?}", name(p, n, anon), line.join(" | "), errs);
    }
}

fn name(p: &Path, n: usize, anon: bool) -> String {
    if anon {
        format!("#{n} .{}", p.extension().unwrap().to_string_lossy())
    } else {
        p.display().to_string()
    }
}
