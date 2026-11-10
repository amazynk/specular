//! Compares the composite with GIMP's own (the `.ref` files tools/corpus/fetch-xcf.sh makes
//! with GIMP's projection) over the XCF corpus in `$SPECULAR_LARGE_CORPUS/xcf`. Skips when
//! the corpus is absent. `XCF_ORACLE_VERBOSE=1` prints every file's error.
//!
//! References of images with a colour profile are made with the profile unassigned, so they
//! stay in the profile's encoding like this crate's output (colour management is the
//! viewer's job).

use specular_xcf::{Options, PixelFormat, decompress, f16_to_f32, parse};
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("SPECULAR_LARGE_CORPUS")?).join("xcf");
    dir.is_dir().then_some(dir)
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            files(&p, out);
        } else {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            if n.ends_with(".xcf")
                || n.ends_with(".xcf.gz")
                || n.ends_with(".xcf.bz2")
                || n.ends_with(".xcf.xz")
            {
                out.push(p);
            }
        }
    }
}

struct Reference {
    width: u32,
    height: u32,
    icc: bool,
    px: Vec<f32>,
}

fn reference(p: &Path) -> Option<Reference> {
    let b = std::fs::read(p).ok()?;
    if b.len() < 16 || &b[..4] != b"XREF" {
        return None;
    }
    let u = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let (width, height, flags) = (u(4), u(8), u(12));
    let px: Vec<f32> = b[16..]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    (px.len() == width as usize * height as usize * 4).then_some(Reference {
        width,
        height,
        icc: flags & 1 != 0,
        px,
    })
}

fn srgb(v: f32) -> f32 {
    let v = f64::from(v);
    (if v > 0.003_130_8 {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    } else {
        v * 12.92
    }) as f32
}

/// Our output as sRGB-coded straight RGBA floats.
fn ours(fmt: PixelFormat, data: &[u8]) -> Vec<f32> {
    let u16s = |c: &[u8]| -> Vec<f32> {
        c.chunks_exact(2)
            .map(|b| f32::from(u16::from_ne_bytes([b[0], b[1]])) / 65535.0)
            .collect()
    };
    let mut out = Vec::new();
    match fmt {
        PixelFormat::Gray8 => data.iter().for_each(|&v| {
            let g = f32::from(v) / 255.0;
            out.extend([g, g, g, 1.0]);
        }),
        PixelFormat::Gray16 => u16s(data).iter().for_each(|&g| out.extend([g, g, g, 1.0])),
        PixelFormat::GrayAlpha16 => u16s(data)
            .chunks_exact(2)
            .for_each(|p| out.extend([p[0], p[0], p[0], p[1]])),
        PixelFormat::Rgba8 => out.extend(data.iter().map(|&v| f32::from(v) / 255.0)),
        PixelFormat::Rgba16 => out = u16s(data),
        PixelFormat::Rgba16F => {
            let f: Vec<f32> = data
                .chunks_exact(2)
                .map(|b| f16_to_f32(u16::from_ne_bytes([b[0], b[1]])))
                .collect();
            for p in f.chunks_exact(4) {
                out.extend([srgb(p[0]), srgb(p[1]), srgb(p[2]), p[3]]);
            }
        }
    }
    out
}

#[derive(Default, Debug)]
struct Stats {
    max: f32,
    mean: f64,
    /// Fraction of pixels off by more than 2/255.
    over: f64,
}

/// Float images can hold any value: beyond 0..1 values are compared relatively (a quarter of
/// the log ratio, so 1% counts like 0.6/255; divide and dodge amplify tiny differences in the
/// transfer curves there), within the range half floats (the output format) hold.
fn squash(v: f32) -> f32 {
    // `srgb(65504.0)`: the largest sRGB-coded value a half-float linear output holds.
    const HALF_MAX_CODED: f32 = 107.15;
    let v = if v.is_nan() {
        0.0
    } else {
        v.clamp(-HALF_MAX_CODED, HALF_MAX_CODED)
    };
    if v.abs() <= 1.0 {
        v
    } else {
        v.signum() * (1.0 + 0.25 * v.abs().ln())
    }
}

fn compare(a: &[f32], b: &[f32]) -> Stats {
    let mut s = Stats::default();
    let mut over = 0usize;
    let n = a.len() / 4;
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let (pa, qa) = (p[3].clamp(0.0, 1.0), q[3].clamp(0.0, 1.0));
        let mut e = (pa - qa).abs();
        for k in 0..3 {
            e = e.max((squash(p[k]) * pa - squash(q[k]) * qa).abs());
        }
        s.max = s.max.max(e);
        s.mean += f64::from(e);
        if e > 2.0 / 255.0 {
            over += 1;
        }
    }
    s.mean /= n.max(1) as f64;
    s.over = over as f64 / n.max(1) as f64;
    s
}

/// Files that differ from GIMP for a reason not worth copying.
const KNOWN: &[(&str, &str)] = &[
    (
        "filters/blur.xcf",
        "Gaussian blur effect: the rightmost 7 columns differ by up to 11/255 (GEGL's recursive \
         filter ends differently at the right edge only); mean 0.3/255 elsewhere",
    ),
    (
        "filters/two.xcf",
        "invert then Gaussian blur effects: the same right-edge difference as filters/blur.xcf",
    ),
    (
        "gen/mode-floatl-16.xcf",
        "legacy Dodge over pure white layer pixels in a linear float image: GIMP's conversion \
     to perceptual rounds 1.0 slightly above 1, so 1 - layer goes negative and the result \
     flips to black; here it is white (as GIMP gives in 8-bit images)",
    ),
];

#[test]
fn matches_gimp() {
    let Some(dir) = corpus() else {
        eprintln!("no XCF corpus; skipping");
        return;
    };
    let verbose = std::env::var_os("XCF_ORACLE_VERBOSE").is_some();
    let mut list = Vec::new();
    files(&dir, &mut list);
    let mut failures = Vec::new();
    let mut compared = 0;
    for p in &list {
        let name = p.strip_prefix(&dir).unwrap().display().to_string();
        let raw = std::fs::read(p).unwrap();
        let data = match decompress(&raw) {
            Ok(Some(d)) => d,
            Ok(None) => raw,
            Err(e) => {
                failures.push(format!("{name}: container: {e}"));
                continue;
            }
        };
        // Compressed copies compare against the plain file's reference.
        let plain = p.to_string_lossy();
        let base = plain
            .strip_suffix(".gz")
            .or_else(|| plain.strip_suffix(".bz2"))
            .or_else(|| plain.strip_suffix(".xz"))
            .unwrap_or(&plain);
        let Some(r) = reference(Path::new(&format!("{base}.ref"))) else {
            eprintln!("{name}: no reference");
            continue;
        };
        let x = match parse(&data) {
            Ok(x) => x,
            Err(e) => {
                failures.push(format!("{name}: parse: {e}"));
                continue;
            }
        };
        let img = match x.render(
            &data,
            &Options {
                parallel: true,
                cancel: None,
            },
        ) {
            Ok(i) => i,
            Err(e) => {
                failures.push(format!("{name}: render: {e}"));
                continue;
            }
        };
        assert_eq!((img.width, img.height), (r.width, r.height), "{name}");
        let info = x.info();
        let mut px = ours(img.format, &img.data);
        if r.icc && info.precision.linear && img.format != PixelFormat::Rgba16F {
            // With a profile, linear-precision output stays linear (the viewer applies the
            // profile); the reference is coded with the sRGB curve.
            for p in px.chunks_exact_mut(4) {
                for v in &mut p[..3] {
                    *v = srgb(*v);
                }
            }
        }
        let s = compare(&px, &r.px);
        compared += 1;
        // 8-bit references are quantised, so allow a little more than a step; a handful of
        // pixels may differ more where GIMP's own 8-bit intermediate rounding shows.
        let known = KNOWN.iter().find(|(f, _)| name == *f).map(|(_, why)| why);
        let ok = s.mean < 1.5 / 255.0 && s.over < 0.01 || known.is_some();
        if verbose || !ok {
            eprintln!(
                "{name}: v{} {:?} {:?} max {:.4} mean {:.5} over {:.3}%{}",
                info.version,
                info.precision.component,
                img.format,
                s.max,
                s.mean,
                s.over * 100.0,
                match (ok, known) {
                    (_, Some(why)) => format!("  known: {why}"),
                    (true, None) => String::new(),
                    (false, None) => "  FAIL".into(),
                }
            );
        }
        if !ok {
            failures.push(name);
        }
    }
    eprintln!("{compared} files compared, {} failures", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}
