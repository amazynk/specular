//! The corpus against TwelveMonkeys' PICT reader (Java ImageIO; `tools/corpus/
//! fetch-twelvemonkeys.sh`, run by `tools/corpus/twelvemonkeys-oracle.java`): QuickDraw
//! vector drawing and text, QuickTime PNG / TIFF / BMP / raw images and 32-bit alpha, which
//! ImageMagick does not draw. Skipped when Java, the jars or the corpus
//! (`tools/corpus/fetch-conformance.sh src_twelvemonkeys src_ffmpeg_iffpict`) are missing.
//!
//! ```text
//! SPECULAR_TWELVEMONKEYS=<jar dir> cargo test -p specular-pict --test twelvemonkeys -- --nocapture
//! ```
//!
//! TwelveMonkeys draws vectors antialiased with Java2D and its own fonts onto a transparent
//! canvas; both sides are compared flattened on white, by the share of pixels more than 64
//! apart in some channel (text and edges differ in place).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn jars() -> Option<PathBuf> {
    let d = std::env::var_os("SPECULAR_TWELVEMONKEYS")
        .map(PathBuf::from)
        .or_else(|| {
            let t = std::env::var_os("CARGO_TARGET_DIR")?;
            Some(Path::new(&t).join("twelvemonkeys"))
        })?;
    d.join("imageio-pict-3.12.0.jar").is_file().then_some(d)
}

fn root() -> PathBuf {
    std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance")
        })
}

/// TwelveMonkeys' decode of each file: name → (w, h, straight RGBA).
fn oracle(
    jars: &Path,
    files: &[PathBuf],
) -> std::collections::HashMap<String, (u32, u32, Vec<u8>)> {
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("pict-twelvemonkeys");
    std::fs::create_dir_all(&tmp).unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tools/corpus/twelvemonkeys-oracle.java");
    let out = Command::new("java")
        .arg(format!("-Djava.io.tmpdir={}", tmp.display()))
        .arg(format!("-Duser.home={}", tmp.display()))
        .arg("-Djava.awt.headless=true")
        .arg("-cp")
        .arg(format!("{}/*", jars.display()))
        .arg(&script)
        .arg(&tmp)
        .args(files)
        .stderr(Stdio::null())
        .output()
        .expect("java");
    let mut m = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.get(1) != Some(&"OK") {
            continue;
        }
        let d = std::fs::read(tmp.join(format!("{}.rgba", f[0]))).unwrap();
        let nl = d.iter().position(|&b| b == b'\n').unwrap();
        let (w, h) = (f[2].parse().unwrap(), f[3].parse().unwrap());
        m.insert(f[0].to_string(), (w, h, d[nl + 1..].to_vec()));
    }
    m
}

fn on_white(p: &[u8]) -> [u8; 3] {
    let a = u32::from(p[3]);
    [0, 1, 2].map(|c| ((u32::from(p[c]) * a + 255 * (255 - a) + 127) / 255) as u8)
}

/// (share of pixels more than `tol` apart, mean difference, alpha mismatches over 2).
fn compare(a: &specular_pict::Image, b: &(u32, u32, Vec<u8>), tol: u8) -> (f64, f64, usize) {
    let (w, h) = (a.width.min(b.0) as usize, a.height.min(b.1) as usize);
    let (mut n, mut sum, mut alpha) = (0usize, 0u64, 0usize);
    for y in 0..h {
        for x in 0..w {
            let p = &a.pixels[(y * a.width as usize + x) * 4..][..4];
            let q = &b.2[(y * b.0 as usize + x) * 4..][..4];
            let (pa, qa) = (on_white(p), on_white(q));
            let d = (0..3).map(|c| pa[c].abs_diff(qa[c])).max().unwrap();
            sum += u64::from(d);
            if d > tol {
                n += 1;
            }
            if a.alpha && p[3].abs_diff(q[3]) > 2 {
                alpha += 1;
            }
        }
    }
    let t = (w * h).max(1) as f64;
    (n as f64 / t, sum as f64 / t, alpha)
}

/// The share of pixels allowed to differ: drawing and text differ in antialiasing, font and
/// glyph placement; bitmaps and QuickTime images must match.
fn allowed(img: &specular_pict::Image) -> f64 {
    if img.shapes > 0 { 0.10 } else { 0.001 }
}

/// Files TwelveMonkeys draws wrongly (checked by eye against Inside Macintosh and, where it
/// reads them, ImageMagick):
/// - `cow.pict`: nothing at all (its polygons);
/// - `4.pict`, `5.pict`: frames drawn outside the rectangle (QuickDraw frames inside it), so
///   the gaps between the swatches close;
/// - `6.pict`: InvertRect not drawn;
/// - `J19.pict`: the fallback bitmaps drawn over the QuickTime JPEGs (ImageMagick shows the
///   JPEGs, as we do);
/// - `P564B1400.pict`: the window's title bar bitmaps left out;
/// - `Picture14.pict`: sized by another rectangle than the frame;
/// - `P30946BDC.pict`: its QuickTime TIFF not decoded (the fallback text shown instead).
const ORACLE_WRONG: &[&str] = &[
    "P30946BDC.pict",
    "cow.pict",
    "4.pict",
    "5.pict",
    "6.pict",
    "J19.pict",
    "P564B1400.pict",
    "Picture14.pict",
];

#[test]
fn corpus_against_twelvemonkeys() {
    let Some(jars) = jars() else {
        eprintln!("TwelveMonkeys jars not fetched; skipped");
        return;
    };
    if Command::new("java")
        .arg("-version")
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        eprintln!("no java; skipped");
        return;
    }
    let mut files = Vec::new();
    for dir in [
        "twelvemonkeys/imageio/imageio-pict/src/test/resources/pict",
        "ffmpeg-samples",
    ] {
        let Ok(rd) = std::fs::read_dir(root().join(dir)) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "pict" | "pct") {
                files.push(p);
            }
        }
    }
    // A 32-bit picture with alpha, written by ImageMagick (which reads its alpha back as
    // opaque).
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("pict-twelvemonkeys");
    std::fs::create_dir_all(&tmp).unwrap();
    let alpha = tmp.join("magick-alpha.pict");
    if Command::new("magick")
        .args(["-size", "64x48", "gradient:red-blue", "(", "-size", "64x48"])
        .args([
            "gradient:white-black",
            ")",
            "-alpha",
            "off",
            "-compose",
            "CopyOpacity",
        ])
        .arg("-composite")
        .arg(format!("pict:{}", alpha.display()))
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        files.push(alpha);
    }
    files.sort();
    if files.is_empty() {
        eprintln!("PICT corpus not fetched; skipped");
        return;
    }
    let refs = oracle(&jars, &files);
    let mut failures = Vec::new();
    let mut compared = 0;
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(f).unwrap();
        let img = match specular_pict::decode(&data) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("{name}: {e}");
                continue;
            }
        };
        if std::env::var_os("SPECULAR_PICT_DUMP").is_some() {
            let mut d = format!("{} {}\n", img.width, img.height).into_bytes();
            d.extend_from_slice(&img.pixels);
            std::fs::write(tmp.join(format!("{name}.ours.rgba")), d).unwrap();
        }
        let Some(r) = refs.get(&name) else {
            eprintln!("{name}: no oracle");
            continue;
        };
        let (share, mean, alpha) = compare(&img, r, 64);
        let ok = share <= allowed(&img) && alpha == 0;
        let verdict = if ORACLE_WRONG.contains(&name.as_str()) {
            "oracle wrong, not counted"
        } else if ok {
            "ok"
        } else {
            failures.push(format!(
                "{name}: {:.2}% over 64, alpha {alpha}",
                share * 100.0
            ));
            "FAIL"
        };
        compared += 1;
        eprintln!(
            "{name}: {}x{} vs {}x{} shapes={} quicktime={} alpha={} {:.2}% over 64, mean {mean:.2} {verdict}",
            img.width,
            img.height,
            r.0,
            r.1,
            img.shapes,
            img.quicktime,
            img.alpha,
            share * 100.0
        );
    }
    eprintln!("{compared} compared, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}
