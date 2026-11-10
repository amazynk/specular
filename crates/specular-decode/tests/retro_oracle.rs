//! Retro-format fixtures against reference decodes (ImageMagick, Pillow), made by
//! `tools/corpus/retro-oracle.py`. Runs when `SPECULAR_RETRO_CORPUS` names its output folder;
//! prints the mismatching channel values per file and, with `SPECULAR_RETRO_GOLDENS` set,
//! writes each decode as `<file>.ours.pam` for eye checks.

use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeRequest, Registry};
use std::path::Path;
use std::sync::Arc;

/// Decodes `path` at full size to RGBA.
fn rgba(reg: &Registry, path: &Path) -> (u32, u32, Vec<u8>, String) {
    let s = reg
        .open(Arc::new(Source::open(path).unwrap()))
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    let name = s.info().format.name().to_string();
    if !matches!(r.layout, PixelLayout::Gray | PixelLayout::Rgba) {
        // Planar YCbCr (the JPEG in a QuickTime image): the crate's own conversion.
        let (w, h, px) = specular_decode::decode_rgba8(&std::fs::read(path).unwrap()).unwrap();
        return (w, h, px, name);
    }
    let p = &r.planes[0];
    let px = r.layout.pixel_bytes();
    let mut out = Vec::with_capacity(r.width as usize * r.height as usize * 4);
    for y in 0..r.height as usize {
        for x in 0..r.width as usize {
            let s = &p.data[y * p.stride + x * px..][..px];
            match r.layout {
                PixelLayout::Gray => out.extend_from_slice(&[s[0], s[0], s[0], 255]),
                PixelLayout::Rgba => out.extend_from_slice(s),
                _ => unreachable!(),
            }
        }
    }
    (r.width, r.height, out, name)
}

#[test]
fn retro_fixtures_match_oracles() {
    let Some(dir) = std::env::var_os("SPECULAR_RETRO_CORPUS") else {
        eprintln!("SPECULAR_RETRO_CORPUS not set: skipped");
        return;
    };
    let goldens = std::env::var_os("SPECULAR_RETRO_GOLDENS").is_some();
    let reg = Registry::new();
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            !n.ends_with(".oracle")
                && !n.ends_with(".pam")
                && !n.ends_with(".pal")
                && !n.ends_with(".png")
                && !n.ends_with(".pcx")
        })
        .collect();
    files.sort();
    let mut failed = Vec::new();
    for f in files {
        let (w, h, ours, format) = rgba(&reg, &f);
        if goldens {
            let mut pam = format!(
                "P7\nWIDTH {w}\nHEIGHT {h}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n"
            )
            .into_bytes();
            pam.extend_from_slice(&ours);
            std::fs::write(format!("{}.ours.pam", f.display()), pam).unwrap();
        }
        let oracle = f.with_file_name(format!(
            "{}.oracle",
            f.file_name().unwrap().to_string_lossy()
        ));
        let Ok(o) = std::fs::read(&oracle) else {
            eprintln!("{}: {format} {w}×{h} (no oracle)", f.display());
            continue;
        };
        let nl = o.iter().position(|&b| b == b'\n').unwrap();
        let dims: Vec<u32> = std::str::from_utf8(&o[..nl])
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let want = &o[nl + 1..];
        if (dims[0], dims[1]) != (w, h) {
            failed.push(format!("{}: size {w}×{h}, oracle {dims:?}", f.display()));
            continue;
        }
        let diffs: Vec<u8> = ours.iter().zip(want).map(|(a, b)| a.abs_diff(*b)).collect();
        let mismatched = diffs.iter().filter(|&&d| d != 0).count();
        let max = diffs.iter().copied().max().unwrap_or(0);
        eprintln!(
            "{}: {format} {w}×{h}, {mismatched} of {} channel values differ, max {max}",
            f.display(),
            diffs.len()
        );
        // A JPEG inside a QuickTime image: IDCT rounding differs from libjpeg-turbo's.
        // DXT1: Pillow rounds the interpolated colours differently from the texture
        // decoder (bit-exact with bcdec); alpha must agree.
        let dxt = f.file_name().is_some_and(|n| n == "dxt1.blp");
        if dxt {
            let alpha_bad = ours
                .iter()
                .zip(want)
                .skip(3)
                .step_by(4)
                .filter(|(a, b)| a != b)
                .count();
            if alpha_bad > 0 {
                failed.push(format!("{}: {alpha_bad} alpha values differ", f.display()));
            }
        }
        let tolerance = if f.extension().is_some_and(|e| e == "qtif") {
            4
        } else if dxt {
            8
        } else {
            0
        };
        if max > tolerance {
            failed.push(format!("{}: max difference {max}", f.display()));
        }
    }
    // Pillow's MPO stereo pair: two pages, labelled left and right.
    let mpo = Path::new(&dir).join("pair.mpo");
    if mpo.exists() {
        let s = reg.open(Arc::new(Source::open(&mpo).unwrap())).unwrap();
        eprintln!("pair.mpo: {} pages, {}", s.page_count(), s.info().coding);
        assert_eq!(s.page_count(), 2);
        assert_eq!(
            s.page_label(),
            Some(specular_decode::PageLabel::Stereo { right: false })
        );
        let r = s.open_page(1).unwrap();
        assert_eq!(
            r.page_label(),
            Some(specular_decode::PageLabel::Stereo { right: true })
        );
    }
    assert!(failed.is_empty(), "{failed:#?}");
}
