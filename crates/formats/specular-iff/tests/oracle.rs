//! Files written by netpbm's `ppmtoilbm` and real-world files from the conformance corpus,
//! decoded and compared with netpbm's `ilbmtoppm`. Skipped when netpbm is not installed or
//! the corpus is not fetched (`tools/corpus/fetch-conformance.sh src_twelvemonkeys
//! src_ffmpeg_iffpict`). `SPECULAR_CONFORMANCE_DIR` overrides the corpus root.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use specular_iff::{Kind, decode, info};

fn have(tool: &str) -> bool {
    Command::new(tool)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("specular-iff-oracle");
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// Parses a binary PPM (P6, maxval 255) into RGB.
fn ppm(d: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut fields = Vec::new();
    let mut i = 2;
    while fields.len() < 3 {
        while d[i].is_ascii_whitespace() {
            i += 1;
        }
        if d[i] == b'#' {
            while d[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let s = i;
        while d[i].is_ascii_digit() {
            i += 1;
        }
        fields.push(
            std::str::from_utf8(&d[s..i])
                .unwrap()
                .parse::<u32>()
                .unwrap(),
        );
    }
    assert_eq!(&d[..2], b"P6");
    assert_eq!(fields[2], 255, "maxval");
    (fields[0], fields[1], d[i + 1..].to_vec())
}

/// `ilbmtoppm` output for `file`, or None when it refuses the file.
fn oracle(file: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let out = Command::new("ilbmtoppm")
        .arg(file)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !(out.status.success() && out.stdout.starts_with(b"P6")) {
        return None;
    }
    let (w, h, mut rgb) = ppm(&out.stdout);
    // ilbmtoppm keeps 4-bit palettes (0xA0) as they are; the Amiga showed them as 0xAA.
    if rgb.iter().all(|v| v & 0x0f == 0) {
        rgb.iter_mut().for_each(|v| *v |= *v >> 4);
    }
    Some((w, h, rgb))
}

/// FFmpeg's IFF decoder output as RGB (a second opinion), or None.
fn ffmpeg(file: &Path, w: u32, h: u32) -> Option<Vec<u8>> {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    (out.status.success() && out.stdout.len() == (w * h * 3) as usize).then_some(out.stdout)
}

/// (pixels differing, largest channel difference) between our RGBA and the oracle's RGB.
fn diff(rgba: &[u8], rgb: &[u8]) -> (usize, u8) {
    let mut n = 0;
    let mut m = 0;
    for (a, b) in rgba.chunks_exact(4).zip(rgb.chunks_exact(3)) {
        let d = (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap();
        if d > 0 {
            n += 1;
            m = m.max(d);
        }
    }
    (n, m)
}

fn pattern(w: u32, h: u32, colours: u32) -> Vec<u8> {
    let mut v = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = (x / 3 + y / 2 * 7 + (x ^ y)) % colours;
            v.extend(if colours > 256 {
                [(x * 3) as u8, (y * 7 + x) as u8, ((x ^ y) * 5) as u8]
            } else {
                [(i * 37) as u8, (i * 91) as u8, (i * 13) as u8]
            });
        }
    }
    v
}

/// Largest channel difference allowed against FFmpeg, which differs from the Amiga display
/// (and from us) in two documented ways: it does not widen 4-bit palettes (`0xA0` stays
/// `0xA0`, we show `0xAA`) and its HAM8 modify replicates the six bits instead of keeping
/// the previous pixel's low two bits (AGA behaviour).
fn ffmpeg_tolerance(kind: Kind, planes: u8, rgba: &[u8]) -> u8 {
    let widened = rgba
        .chunks_exact(4)
        .all(|p| p[..3].iter().all(|v| v >> 4 == v & 15));
    match kind {
        Kind::Ham if planes == 8 => 3,
        Kind::Indexed | Kind::Ehb | Kind::Ham if widened => 15,
        _ => 0,
    }
}

#[test]
fn ppmtoilbm_written_files() {
    if !have("ppmtoilbm") || !have("ilbmtoppm") {
        eprintln!("netpbm not installed; skipped");
        return;
    }
    let (w, h) = (75, 29);
    // HAM: ilbmtoppm's HAM expansion differs from the hardware's; FFmpeg is the oracle.
    for (name, colours, args, kind, exact_vs_source) in [
        ("planes1", 2, &[][..], Kind::Indexed, true),
        (
            "planes3_raw",
            8,
            &["-cmethod", "none"][..],
            Kind::Indexed,
            true,
        ),
        ("planes5", 32, &[][..], Kind::Indexed, true),
        (
            "planes8",
            200,
            &["-maxplanes", "8"][..],
            Kind::Indexed,
            true,
        ),
        ("deep24", 100_000, &["-24force"][..], Kind::Rgb24, true),
        (
            "deep24_raw",
            100_000,
            &["-24force", "-cmethod", "none"][..],
            Kind::Rgb24,
            true,
        ),
        ("ham6", 100_000, &["-ham6"][..], Kind::Ham, false),
        ("ham8", 100_000, &["-ham8"][..], Kind::Ham, false),
    ] {
        let mut src = format!("P6\n{w} {h}\n255\n").into_bytes();
        let rgb = pattern(w, h, colours);
        src.extend(&rgb);
        let p = tmp(&format!("{name}.ppm"));
        std::fs::write(&p, &src).unwrap();
        let out = Command::new("ppmtoilbm")
            .args(args)
            .arg(&p)
            .stderr(Stdio::null())
            .output()
            .unwrap();
        assert!(out.status.success(), "{name}: ppmtoilbm failed");
        let f = tmp(&format!("{name}.iff"));
        std::fs::write(&f, &out.stdout).unwrap();
        let i = info(&out.stdout).unwrap();
        assert_eq!(i.kind, kind, "{name}");
        let img = decode(&out.stdout).unwrap();
        assert!(!img.truncated, "{name}");
        if kind == Kind::Ham {
            let Some(ff) = ffmpeg(&f, w, h) else {
                eprintln!("{name}: ffmpeg not available; skipped");
                continue;
            };
            let (_, m) = diff(&img.pixels, &ff);
            let tol = ffmpeg_tolerance(kind, i.planes, &img.pixels);
            assert!(m <= tol, "{name}: differs from ffmpeg by {m}");
            // Still close to the source: HAM is lossy, but not wildly so.
            let (_, m) = diff(&img.pixels, &rgb);
            assert!(m < 160, "{name}: {m} from source");
            continue;
        }
        let (ow, oh, orgb) = oracle(&f).expect("ilbmtoppm");
        assert_eq!((img.width, img.height), (ow, oh), "{name}");
        let (n, m) = diff(&img.pixels, &orgb);
        assert_eq!((n, m), (0, 0), "{name}: differs from ilbmtoppm");
        if exact_vs_source {
            assert_eq!(
                diff(&img.pixels, &rgb),
                (0, 0),
                "{name}: differs from source"
            );
        }
    }
}

fn corpus_files() -> Vec<PathBuf> {
    let root = std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance")
        });
    let mut v = Vec::new();
    for dir in [
        "twelvemonkeys/imageio/imageio-iff/src/test/resources/iff",
        "ffmpeg-samples",
    ] {
        let Ok(rd) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "iff" | "lbm" | "pbm" | "ilbm" | "deep") {
                v.push(p);
            }
        }
    }
    v.sort();
    v
}

/// Every corpus file must match `ilbmtoppm` exactly or FFmpeg within
/// [`ffmpeg_tolerance`]. The two disagree with each other and with the hardware on
/// Extra Half-Brite files whose `CMAP` carries junk in entries 32–63 (ilbmtoppm uses them)
/// and on HAM, so a single oracle is not enough.
#[test]
fn corpus_matches_oracles() {
    let files = corpus_files();
    if files.is_empty() {
        eprintln!("IFF corpus not fetched; skipped");
        return;
    }
    let netpbm = have("ilbmtoppm");
    let mut failures = Vec::new();
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(f).unwrap();
        let t = std::time::Instant::now();
        let r = decode(&data);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let img = match r {
            Ok(img) => img,
            Err(e) => {
                // FORM DEEP (TVPaint) is a different format.
                assert!(name.ends_with(".deep"), "{name}: {e}");
                eprintln!("{name}: {e}");
                continue;
            }
        };
        let i = info(&data).unwrap();
        let size = (img.width, img.height);
        let netpbm_diff = netpbm
            .then(|| oracle(f))
            .flatten()
            .filter(|(w, h, _)| (*w, *h) == size)
            .map(|(_, _, rgb)| diff(&img.pixels, &rgb));
        let ffmpeg_diff = ffmpeg(f, img.width, img.height).map(|rgb| diff(&img.pixels, &rgb));
        let tol = ffmpeg_tolerance(i.kind, i.planes, &img.pixels);
        let ok = netpbm_diff == Some((0, 0)) || ffmpeg_diff.is_some_and(|(_, m)| m <= tol);
        let line = format!(
            "{name}: {}x{} {} {ms:.1} ms truncated={} ilbmtoppm {:?} ffmpeg {:?} (tolerance {tol})",
            img.width,
            img.height,
            i.coding(),
            img.truncated,
            netpbm_diff,
            ffmpeg_diff,
        );
        eprintln!("{line}");
        if !ok && (netpbm_diff.is_some() || ffmpeg_diff.is_some()) {
            failures.push(line);
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
