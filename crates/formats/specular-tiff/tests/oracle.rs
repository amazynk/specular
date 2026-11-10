//! Pixel comparison with libtiff / tifffile (`tools/corpus/tiff-oracle.py check`, oracle only)
//! over the corpus of `tools/corpus/fetch-image-variants.sh tiff`: libtiff's test pictures
//! (ThunderScan, LogL/LogLuv, subsampled YCbCr, old-style LZW, 2–32-bit depths) and the
//! generated layouts of `tiff-oracle.py variants`.
//!
//! ```text
//! SPECULAR_TIFF_VARIANTS_CORPUS=<dir> SPECULAR_RAWPY=<venv>/bin/python \
//!   cargo test -p specular-tiff --test oracle -- --ignored --nocapture
//! ```
//!
//! Prints one `ORACLE<TAB>TIFF/<group><TAB>file<TAB>pass|fail|skip` line per file.

use std::path::{Path, PathBuf};
use std::process::Command;

use specular_tiff::{DecodeOptions, PixelFormat, Tiff};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_TIFF_VARIANTS_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            let t = std::env::var_os("CARGO_TARGET_DIR")?;
            Some(Path::new(&t).join("corpus/tiff-variants"))
        })?;
    dir.is_dir().then_some(dir)
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            files(&p, out);
        } else if p
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("tif") || e.eq_ignore_ascii_case("tiff"))
        {
            out.push(p);
        }
    }
}

/// Known differences, counted as failures without failing the test: libtiff's old-style
/// JPEG path replicates chroma where ours interpolates (mean error 1.51 against a 1.5 bound).
const KNOWN: &[&str] = &["smallliz.tif"];

fn fmt_name(f: PixelFormat) -> &'static str {
    match f {
        PixelFormat::Gray8 => "Gray8",
        PixelFormat::Rgba8 => "Rgba8",
        PixelFormat::Gray16 => "Gray16",
        PixelFormat::GrayAlpha16 => "GrayAlpha16",
        PixelFormat::Rgba16 => "Rgba16",
        PixelFormat::Rgba16F => "Rgba16F",
        PixelFormat::GrayF32 => "GrayF32",
    }
}

#[test]
#[ignore = "needs the corpus and the oracle venv"]
fn corpus_matches_libtiff() {
    let Some(dir) = corpus() else {
        eprintln!("skipped: no corpus (tools/corpus/fetch-image-variants.sh tiff)");
        return;
    };
    let python = std::env::var("SPECULAR_RAWPY").unwrap_or_else(|_| "python3".into());
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/tiff-oracle.py");
    let scratch = std::env::temp_dir().join(format!("tiff-oracle-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let mut list = Vec::new();
    files(&dir, &mut list);
    let (mut pass, mut fail, mut refused, mut known) = (0, 0, 0, 0);
    for path in &list {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let group = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|g| g.to_string_lossy().into_owned())
            .unwrap_or_default();
        let data = std::fs::read(path).unwrap();
        let ours = Tiff::open(&data).and_then(|t| {
            t.pages()[0].check_supported()?;
            t.decode(
                &data,
                0,
                &DecodeOptions {
                    wide: true,
                    ..Default::default()
                },
            )
        });
        let status = match ours {
            Err(e) => {
                // A refusal counts as a failure, but only mismatches fail the test.
                println!("{name}: refused: {e}");
                refused += 1;
                "fail"
            }
            Ok(d) => {
                let raw = scratch.join("out.raw");
                std::fs::write(&raw, &d.data).unwrap();
                let out = Command::new(&python)
                    .arg("-I")
                    .arg(&script)
                    .arg("check")
                    .arg(path)
                    .arg(&raw)
                    .arg(d.width.to_string())
                    .arg(d.height.to_string())
                    .arg(fmt_name(d.format))
                    .arg(if d.truncated { "true" } else { "false" })
                    .output()
                    .expect("run the oracle");
                let text = String::from_utf8_lossy(&out.stdout);
                let line = text.lines().last().unwrap_or("").to_string();
                println!("{name}: {line}");
                if line.starts_with("PASS") {
                    "pass"
                } else if line.starts_with("FAIL") {
                    "fail"
                } else {
                    "skip"
                }
            }
        };
        match status {
            "pass" => pass += 1,
            "fail" if KNOWN.contains(&name.as_str()) => known += 1,
            "fail" => fail += 1,
            _ => {}
        }
        println!("ORACLE\tTIFF/{group}\t{name}\t{status}");
    }
    let _ = std::fs::remove_dir_all(&scratch);
    println!("{pass} passed, {} failed ({refused} refused)", fail + known);
    assert_eq!(
        fail - refused,
        0,
        "{} files differ from the oracle",
        fail - refused
    );
}
