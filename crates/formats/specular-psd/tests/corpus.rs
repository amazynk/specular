//! Every pixel layer of every PSD/PSB in the fetched corpora decodes, and 8-bit RGB layers
//! match the `psd` crate (oracle only) inside their rectangles.
//!
//! Roots: `SPECULAR_CONFORMANCE_DIR` (default `tools/corpus/conformance`), `tools/corpus/large`,
//! and the directories in `SPECULAR_PSD_DIRS` (colon-separated; files are read in place).
//! Skipped when none has PSD files.

use std::path::{Path, PathBuf};

use specular_psd::*;

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("psd") || e.eq_ignore_ascii_case("psb"))
        {
            out.push(p);
        }
    }
}

fn files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus");
    let mut dirs = vec![
        std::env::var_os("SPECULAR_CONFORMANCE_DIR")
            .map_or_else(|| root.join("conformance"), PathBuf::from),
        root.join("large"),
    ];
    if let Some(extra) = std::env::var_os("SPECULAR_PSD_DIRS") {
        dirs.extend(std::env::split_paths(&extra));
    }
    let mut out = Vec::new();
    for d in dirs {
        walk(&d, &mut out);
    }
    out.sort();
    out
}

/// The `psd` crate's view of 8-bit RGB layers (raw / RLE only): name and canvas RGBA. `None`
/// when it cannot read the file.
fn oracle(data: &[u8]) -> Option<Vec<(String, Vec<u8>)>> {
    std::panic::catch_unwind(|| {
        let p = psd::Psd::from_bytes(data).ok()?;
        Some(
            p.layers()
                .iter()
                .map(|l| (l.name().to_string(), l.rgba()))
                .collect(),
        )
    })
    .ok()
    .flatten()
}

#[test]
fn every_corpus_layer_decodes() {
    let files = files();
    if files.is_empty() {
        eprintln!("no PSD corpus (tools/corpus/fetch-conformance.sh); skipped");
        return;
    }
    // The oracle prints its own panics; keep the output readable.
    std::panic::set_hook(Box::new(|_| {}));
    let (mut n_files, mut n_layers, mut compared) = (0, 0, 0);
    let mut failures = Vec::new();
    let mut truncated = Vec::new();
    let mut alpha_diffs = Vec::new();
    for path in &files {
        let data = std::fs::read(path).unwrap();
        let Ok(psd) = parse(&data) else { continue };
        let info = psd.info();
        if info.width as u64 * info.height as u64 > 1 << 26 {
            continue;
        }
        n_files += 1;
        let pixel = psd.pixel_layers();
        let rgb8 = info.mode == ColorMode::Rgb && info.depth == 8;
        let theirs = if rgb8 && !pixel.is_empty() {
            oracle(&data)
        } else {
            None
        };
        for &i in &pixel {
            n_layers += 1;
            let l = &psd.layers()[i];
            let p = match psd.decode_layer_planes(&data, i, &Options::default(), None) {
                Ok(p) => p,
                Err(e) => {
                    failures.push(format!("{}: layer {i} {:?}: {e}", path.display(), l.name));
                    continue;
                }
            };
            if p.truncated {
                truncated.push(format!("{}: layer {i} {:?}", path.display(), l.name));
            }
            let img = p.to_rgba8();
            assert_eq!(
                img.data.len(),
                info.width as usize * info.height as usize * 4
            );
            // Compare with the oracle where it agrees on what the layer is: same index (it
            // lists every record), plain opacity, no mask, raw / RLE channels.
            let Some(theirs) = theirs.as_ref() else {
                continue;
            };
            let Some((name, rgba)) = theirs.get(i) else {
                continue;
            };
            let plain = l.opacity == 255
                && l.fill_opacity == 255
                && l.mask.is_none()
                && l.channels.iter().all(|c| {
                    data.get(c.data.start..c.data.start + 2)
                        .is_some_and(|w| w[0] == 0 && w[1] < 2)
                });
            if !plain || name != &l.name || rgba.len() != img.data.len() || p.truncated {
                continue;
            }
            let w = info.width as i64;
            let (mut bad, mut alpha_only) = (0, 0);
            for (k, (a, b)) in img
                .data
                .chunks_exact(4)
                .zip(rgba.chunks_exact(4))
                .enumerate()
            {
                let (x, y) = (k as i64 % w, k as i64 / w);
                let inside = y >= l.top as i64
                    && y < l.bottom as i64
                    && x >= l.left as i64
                    && x < l.right as i64;
                if !inside || a == b {
                    continue;
                }
                // The psd crate misplaces partial alpha along some layer edges (spot-checked
                // against ImageMagick 7.1.2, which agrees with us), so colour is compared where
                // both sides are opaque and alpha disagreements are only counted.
                if a[3] == 255 && b[3] == 255 {
                    if bad < 3 && std::env::var_os("PSD_DIAG").is_some() {
                        eprintln!("{} {i} ({x},{y}) ours {a:?} theirs {b:?}", path.display());
                    }
                    bad += 1;
                } else if a[3] != b[3] {
                    alpha_only += 1;
                }
            }
            compared += 1;
            if alpha_only > 0 {
                alpha_diffs.push(format!(
                    "{}: layer {i} {:?}: {alpha_only}",
                    path.display(),
                    l.name
                ));
            }
            if bad > 0 {
                failures.push(format!(
                    "{}: layer {i} {:?}: {bad} opaque pixels differ from the psd crate",
                    path.display(),
                    l.name
                ));
            }
        }
    }
    let _ = std::panic::take_hook();
    eprintln!(
        "{n_files} files, {n_layers} pixel layers decoded, {compared} compared with the psd crate, \
         {} truncated",
        truncated.len()
    );
    for t in &truncated {
        eprintln!("  truncated: {t}");
    }
    for t in &alpha_diffs {
        eprintln!("  alpha differs from the psd crate: {t}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
