//! The generated corpus against ImageMagick's own decode (tools/corpus/fetch-raster-legacy2.sh).
//! Run with `SPECULAR_RASTER_CORPUS_LEGACY2=tools/corpus/raster/legacy2 cargo test -p
//! specular-legacy2 --test oracle -- --ignored --nocapture`.

use specular_legacy2::{Image, Pixels, miff, tim, xwd};
use std::path::Path;

type InfoFn = fn(&[u8], u32) -> specular_legacy2::Result<specular_legacy2::Info>;
type DecodeFn = fn(&[u8], u32) -> specular_legacy2::Result<Image>;

/// Our pixels as 16-bit RGBA, and whether they were 8-bit.
fn rgba16(img: &Image) -> (Vec<u16>, bool) {
    match &img.pixels {
        Pixels::Gray8(v) => (
            v.iter()
                .flat_map(|&g| [g as u16 * 257, g as u16 * 257, g as u16 * 257, 65535])
                .collect(),
            true,
        ),
        Pixels::Rgba8(v) => (v.iter().map(|&x| x as u16 * 257).collect(), true),
        Pixels::Gray16(v) => (v.iter().flat_map(|&g| [g, g, g, 65535]).collect(), false),
        Pixels::Rgba16(v) => (v.clone(), false),
        Pixels::RgbaF32(v) => (
            v.chunks_exact(4)
                .flat_map(|p| {
                    let e = |x: f32| {
                        let x = x.clamp(0.0, 1.0);
                        let s = if x <= 0.003_130_8 {
                            x * 12.92
                        } else {
                            1.055 * x.powf(1.0 / 2.4) - 0.055
                        };
                        (s * 65535.0 + 0.5) as u16
                    };
                    [
                        e(p[0]),
                        e(p[1]),
                        e(p[2]),
                        (p[3].clamp(0.0, 1.0) * 65535.0 + 0.5) as u16,
                    ]
                })
                .collect(),
            false,
        ),
    }
}

#[test]
#[ignore]
fn corpus_matches_imagemagick() {
    let Some(root) = std::env::var_os("SPECULAR_RASTER_CORPUS_LEGACY2") else {
        eprintln!("SPECULAR_RASTER_CORPUS_LEGACY2 not set");
        return;
    };
    run(Path::new(&root));
}

/// Real PlayStation TIM and X11 XWD files (Sembiance's collection), 2-bit XWD and mixed-mode TIM
/// written by tools/corpus/oracle-samples.py, against ImageMagick in the same layout
/// (tools/corpus/fetch-oracle-samples.sh legacy2). Reads `SPECULAR_ORACLE_SAMPLES_CORPUS`/legacy2,
/// else `$CARGO_TARGET_DIR/corpus/oracle-samples/legacy2`.
#[test]
#[ignore]
fn oracle_samples_matches_imagemagick() {
    let Some(root) = std::env::var_os("SPECULAR_ORACLE_SAMPLES_CORPUS")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(|t| std::path::PathBuf::from(t).join("corpus/oracle-samples"))
        })
        .map(|b| b.join("legacy2"))
        .filter(|d| d.join("files").is_dir())
    else {
        eprintln!(
            "oracle-samples legacy2 corpus missing: run tools/corpus/fetch-oracle-samples.sh legacy2"
        );
        return;
    };
    run(&root);
}

fn run(root: &Path) {
    let mut names: Vec<_> = std::fs::read_dir(root.join("files"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    // format → (images, passed, mismatched pixels, max error, decode ms)
    let mut stats: std::collections::BTreeMap<&str, (u32, u32, u64, u32, f64)> = Default::default();
    let mut failures = Vec::new();
    for path in names {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let data = std::fs::read(&path).unwrap();
        let ext = name.rsplit('.').next().unwrap();
        let (fmt, info, decode): (&str, InfoFn, DecodeFn) = match ext {
            "miff" => ("MIFF", miff::info, miff::decode),
            "xwd" => ("XWD", xwd::info, xwd::decode),
            "tim" => ("TIM", tim::info, tim::decode),
            _ => continue,
        };
        let pages: u32 = std::fs::read_to_string(root.join("expect").join(format!("{name}.pages")))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let failed_before = failures.len();
        let mut matched = 0u32;
        let ours = info(&data, 0).map(|i| i.pages);
        // Colour spaces refused on purpose (ImageMagick's own encodings: Adobe98, ProPhoto,
        // Jzazbz, Oklch, YCC) are skipped, with their reason printed.
        if let Err(specular_legacy2::Error::Unsupported(why)) = &ours
            && name.starts_with("cs-")
        {
            eprintln!("  skipped {name}: {why}");
            println!("ORACLE\t{fmt}\t{name}\tskip");
            continue;
        }
        // ImageMagick refuses it (TIM mixed mode): no reference.
        if pages == 0 {
            eprintln!("  skipped {name}: ImageMagick reads no image; ours: {ours:?}");
            println!("ORACLE\t{fmt}\t{name}\tskip");
            continue;
        }
        if ours.as_ref().ok() != Some(&pages) {
            failures.push(format!("{name}: pages {ours:?}, ImageMagick {pages}"));
        }
        for p in 0..pages {
            let st = stats.entry(fmt).or_default();
            st.0 += 1;
            let Ok(want) = std::fs::read(root.join("expect").join(format!("{name}.{p}.rgba")))
            else {
                continue;
            };
            let t0 = std::time::Instant::now();
            let img = match decode(&data, p) {
                Ok(i) => i,
                Err(e) => {
                    failures.push(format!("{name}[{p}]: {e}"));
                    continue;
                }
            };
            st.4 += t0.elapsed().as_secs_f64() * 1e3;
            let (got, eight) = rgba16(&img);
            if got.len() * 2 != want.len() {
                failures.push(format!("{name}[{p}]: size {}x{}", img.width, img.height));
                continue;
            }
            let float = name.starts_with("float");
            let wide32 = name.ends_with("-32.miff") && !float;
            let tol: u32 = if float && name.contains("-16") {
                80
            } else if name.starts_with("cs-") && !name.ends_with("-8.miff") {
                // Other colour spaces: ImageMagick's constants differ from the published
                // ones in the last digits (at most 0.04 %).
                24
            } else if float || wide32 || name.starts_with("cmyk") || name.starts_with("cs-") {
                1
            } else {
                0
            };
            let mut bad = 0u64;
            let mut max = 0u32;
            for (i, (g, w)) in got.iter().zip(want.chunks_exact(2)).enumerate() {
                let mut w = u16::from_be_bytes([w[0], w[1]]) as u32;
                let mut g = *g as u32;
                if eight {
                    w = (w * 255 + 32767) / 65535;
                    g /= 257;
                }
                // TIM: ImageMagick shows every colour opaque; ours makes 0x0000 transparent.
                if fmt == "TIM" && i % 4 == 3 {
                    continue;
                }
                let e = g.abs_diff(w);
                max = max.max(e);
                if e > tol {
                    bad += 1;
                }
            }
            st.2 += bad;
            st.3 = st.3.max(max);
            if bad == 0 {
                st.1 += 1;
                matched += 1;
            } else {
                failures.push(format!("{name}[{p}]: {bad} samples differ, max {max}"));
            }
        }
        // One line per file for tools/ci/oracle-pass-rates.sh.
        let status = if failures.len() > failed_before {
            "fail"
        } else if matched > 0 {
            "pass"
        } else {
            "skip"
        };
        println!("ORACLE\t{fmt}\t{name}\t{status}");
    }
    for (f, (n, ok, bad, max, ms)) in &stats {
        eprintln!(
            "{f}: {ok}/{n} images match ImageMagick; {bad} differing samples, max error {max}; decode {ms:.1} ms total"
        );
    }
    for f in &failures {
        eprintln!("  {f}");
    }
    assert!(failures.is_empty(), "{} failures", failures.len());
}
