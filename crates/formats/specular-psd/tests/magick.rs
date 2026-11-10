//! Files written by a real third-party encoder (ImageMagick), decoded and compared with the
//! source pixels. Skipped when `magick` is not installed.
mod common;

use std::path::PathBuf;
use std::process::Command;

use specular_psd::*;

fn have_magick() -> bool {
    Command::new("magick").arg("-version").output().is_ok()
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("specular-psd-magick");
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// Write an RGBA PAM source and convert it with `args`; returns (source RGBA, PSD bytes).
fn make(name: &str, w: u32, h: u32, alpha: bool, args: &[&str]) -> (Vec<u8>, Vec<u8>) {
    let mut rgba = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let a = if alpha {
                ((x * 255) / (w - 1)) as u8
            } else {
                255
            };
            rgba.extend([(x * 9 + y) as u8, (y * 13) as u8, ((x ^ y) * 5) as u8, a]);
        }
    }
    let src = tmp(&format!("{name}.pam"));
    let mut pam =
        format!("P7\nWIDTH {w}\nHEIGHT {h}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n")
            .into_bytes();
    pam.extend(&rgba);
    std::fs::write(&src, pam).unwrap();
    let out = tmp(&format!("{name}.psd"));
    let st = Command::new("magick")
        .arg(&src)
        .args(args)
        .arg(&out)
        .status()
        .unwrap();
    assert!(st.success());
    (rgba, std::fs::read(out).unwrap())
}

#[test]
fn imagemagick_written_files() {
    if !have_magick() {
        eprintln!("magick not installed; skipped");
        return;
    }
    let (w, h) = (61, 37);
    for (name, alpha, args, tol) in [
        ("rgb8_rle", false, &["-alpha", "off"][..], 0),
        (
            "rgb8_zip",
            false,
            &["-alpha", "off", "-compress", "Zip"][..],
            0,
        ),
        (
            "rgb8_raw",
            false,
            &["-alpha", "off", "-compress", "None"][..],
            0,
        ),
        ("rgb16", false, &["-alpha", "off", "-depth", "16"][..], 1),
        ("rgba8", true, &[][..], 0),
        (
            "gray8",
            false,
            &["-alpha", "off", "-colorspace", "Gray"][..],
            -1,
        ),
    ] {
        let (src, f) = make(name, w, h, alpha, args);
        let psd = parse(&f).unwrap_or_else(|e| panic!("{name}: {e}"));
        let img = psd
            .decode_rgba8(&f, &Options::default())
            .unwrap_or_else(|e| panic!("{name}: {e} {:?}", psd.info()));
        assert!(!img.truncated, "{name}");
        if tol < 0 {
            assert_eq!(img.format, PixelFormat::Gray8, "{name}");
            continue;
        }
        assert_eq!(psd.info().has_alpha, alpha, "{name}: {:?}", psd.info());
        for (i, (a, b)) in img.data.chunks(4).zip(src.chunks(4)).enumerate() {
            let n = if alpha { 4 } else { 3 };
            for c in 0..n {
                // Fully transparent pixels carry no colour.
                if alpha && b[3] == 0 && c < 3 {
                    continue;
                }
                assert!(
                    (a[c] as i32 - b[c] as i32).abs() <= tol,
                    "{name} px {i} ch {c}: {a:?} vs {b:?}"
                );
            }
        }
    }
    // CMYK: only check it decodes and lands near the source (naive conversion, no ICC).
    let (src, f) = make(
        "cmyk8",
        w,
        h,
        false,
        &["-alpha", "off", "-colorspace", "CMYK"],
    );
    let psd = parse(&f).unwrap();
    assert_eq!(psd.info().mode, ColorMode::Cmyk);
    let img = psd.decode_rgba8(&f, &Options::default()).unwrap();
    let err: i64 = img
        .data
        .chunks(4)
        .zip(src.chunks(4))
        .map(|(a, b)| {
            (0..3)
                .map(|c| (a[c] as i64 - b[c] as i64).abs())
                .sum::<i64>()
        })
        .sum();
    assert!(
        err / (w * h * 3) as i64 <= 3,
        "mean CMYK error {}",
        err / (w * h * 3) as i64
    );
}

/// An RGBA PAM of `w`×`h` (alpha a gradient when `alpha`), written for magick.
fn pam(name: &str, w: u32, h: u32, alpha: bool, seed: u32) -> (Vec<u8>, PathBuf) {
    let mut rgba = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let a = if alpha {
                (x * 255 / (w - 1)) as u8
            } else {
                255
            };
            rgba.extend([
                (x * 17 + seed) as u8,
                (y * 29 + seed * 3) as u8,
                ((x ^ y) * 7) as u8,
                a,
            ]);
        }
    }
    let path = tmp(&format!("{name}.pam"));
    let mut f =
        format!("P7\nWIDTH {w}\nHEIGHT {h}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n")
            .into_bytes();
    f.extend(&rgba);
    std::fs::write(&path, f).unwrap();
    (rgba, path)
}

/// Layered files written by ImageMagick: each layer page shows that layer's source pixels at
/// its offset (clipped to the canvas) and nothing else.
#[test]
fn imagemagick_layers() {
    if !have_magick() {
        eprintln!("magick not installed; skipped");
        return;
    }
    let (cw, ch) = (23u32, 17u32);
    let (_, canvas) = pam("layered_canvas", cw, ch, false, 1);
    // (w, h, alpha, x, y): one inside with a ramp of alpha, one hanging off the left and bottom.
    let specs = [(9u32, 7u32, true, 5i32, 3i32), (12, 6, false, -4, 13)];
    let layers: Vec<_> = specs
        .iter()
        .enumerate()
        .map(|(i, &(w, h, a, ..))| pam(&format!("layered_{i}"), w, h, a, 40 * i as u32 + 7).0)
        .collect();
    for (name, args, tol) in [
        ("layered8_rle", &["-depth", "8", "-compress", "RLE"][..], 0),
        ("layered8_zip", &["-depth", "8", "-compress", "Zip"][..], 0),
        ("layered8_raw", &["-depth", "8", "-compress", "None"][..], 0),
        (
            "layered16_zip",
            &["-depth", "16", "-compress", "Zip"][..],
            1,
        ),
    ] {
        let mut cmd = Command::new("magick");
        cmd.arg(&canvas);
        for (i, &(_, _, _, x, y)) in specs.iter().enumerate() {
            cmd.arg("(")
                .arg(tmp(&format!("layered_{i}.pam")))
                .arg("-page")
                .arg(format!("{x:+}{y:+}"))
                .arg(")");
        }
        let out = tmp(&format!("{name}.psd"));
        assert!(cmd.args(args).arg(&out).status().unwrap().success());
        let f = std::fs::read(&out).unwrap();
        let psd = parse(&f).unwrap_or_else(|e| panic!("{name}: {e}"));
        let pages = psd.pixel_layers();
        assert_eq!(pages.len(), specs.len(), "{name}: {:?}", psd.layers());
        // Panel order is top-most first: the last layer magick wrote comes first.
        for (page, &index) in pages.iter().enumerate() {
            let k = specs.len() - 1 - page;
            let (w, h, alpha, x0, y0) = specs[k];
            let src = &layers[k];
            let planes = psd
                .decode_layer_planes(&f, index, &Options::default(), None)
                .unwrap_or_else(|e| panic!("{name} layer {k}: {e}"));
            assert!(!planes.truncated, "{name} layer {k}");
            let img = planes.to_rgba8();
            for y in 0..ch as i32 {
                for x in 0..cw as i32 {
                    let o = ((y as u32 * cw + x as u32) * 4) as usize;
                    let got = &img.data[o..o + 4];
                    let (lx, ly) = (x - x0, y - y0);
                    if lx < 0 || ly < 0 || lx >= w as i32 || ly >= h as i32 {
                        assert_eq!(got[3], 0, "{name} layer {k} ({x},{y}) outside");
                        continue;
                    }
                    let s = ((ly as u32 * w + lx as u32) * 4) as usize;
                    let want = &src[s..s + 4];
                    let n = if want[3] == 0 { 1 } else { 4 };
                    for c in (0..4).rev().take(n) {
                        assert!(
                            (got[c] as i32 - want[c] as i32).abs() <= tol,
                            "{name} layer {k} ({x},{y}): {got:?} vs {want:?} (alpha {alpha})"
                        );
                    }
                }
            }
        }
    }
}
