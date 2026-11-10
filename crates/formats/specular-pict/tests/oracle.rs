//! Files written by netpbm's `ppmtopict` and ImageMagick, and real-world files from the
//! conformance corpus, decoded and compared with ImageMagick's PICT reader. Skipped when
//! the tools or the corpus are missing (`tools/corpus/fetch-conformance.sh
//! src_twelvemonkeys src_ffmpeg_iffpict`). `SPECULAR_CONFORMANCE_DIR` overrides the corpus
//! root.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use specular_pict::{decode, info, sole_jpeg};

fn have(tool: &str, arg: &str) -> bool {
    Command::new(tool)
        .arg(arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("specular-pict-oracle");
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// Parses a binary PPM (P6, maxval 255).
fn ppm(d: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut fields = Vec::new();
    let mut i = 2;
    while fields.len() < 3 {
        while d.get(i)?.is_ascii_whitespace() {
            i += 1;
        }
        if d[i] == b'#' {
            while *d.get(i)? != b'\n' {
                i += 1;
            }
            continue;
        }
        let s = i;
        while d.get(i)?.is_ascii_digit() {
            i += 1;
        }
        fields.push(std::str::from_utf8(&d[s..i]).ok()?.parse::<u32>().ok()?);
    }
    (d.starts_with(b"P6") && fields[2] == 255).then(|| (fields[0], fields[1], d[i + 1..].to_vec()))
}

/// ImageMagick's rendering of `file`, flattened on white.
fn magick(file: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let out = Command::new("magick")
        .arg(format!("pict:{}", file.display()))
        .args(["-background", "white", "-flatten", "-depth", "8", "ppm:-"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    ppm(&out.stdout)
}

/// (pixels differing by more than `tol`, largest difference).
fn diff(rgba: &[u8], rgb: &[u8], tol: u8) -> (usize, u8) {
    let mut n = 0;
    let mut m = 0;
    for (a, b) in rgba.chunks_exact(4).zip(rgb.chunks_exact(3)) {
        let d = (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap();
        m = m.max(d);
        if d > tol {
            n += 1;
        }
    }
    (n, m)
}

/// Files whose frame does not start at (0, 0): ImageMagick draws them offset by the frame
/// origin (checked by eye: ours matches the picture's own coordinates).
const MAGICK_MISPLACES: &[&str] = &["tess19.pct", "CatDV==2.0=1=.pict"];

/// Files ImageMagick renders as a blank canvas (checked by eye: ours shows the picture).
const MAGICK_BLANK: &[&str] = &["Picture14.pict"];

/// Like [`diff`] over the area both images cover (ImageMagick sizes some pictures by their
/// clip region or pixmap bounds rather than the frame), skipping pixels ImageMagick leaves
/// black where nothing was drawn and we show QuickDraw's white.
fn overlap_diff(
    img: &specular_pict::Image,
    ow: u32,
    oh: u32,
    rgb: &[u8],
    tol: u8,
) -> (usize, u8, usize) {
    let (w, h) = (img.width.min(ow) as usize, img.height.min(oh) as usize);
    let (mut n, mut m, mut total) = (0, 0, 0);
    for y in 0..h {
        for x in 0..w {
            let a = &img.pixels[(y * img.width as usize + x) * 4..][..3];
            let b = &rgb[(y * ow as usize + x) * 3..][..3];
            if a == [255; 3] && b == [0; 3] {
                continue;
            }
            total += 1;
            let d = (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap();
            m = m.max(d);
            if d > tol {
                n += 1;
            }
        }
    }
    (n, m, total)
}

fn pattern(w: u32, h: u32, colours: u32) -> Vec<u8> {
    let mut v = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = (x / 3 + y / 2 * 7 + (x ^ y)) % colours.max(1);
            v.extend(if colours > 256 {
                [(x * 3) as u8, (y * 7 + x) as u8, ((x ^ y) * 5) as u8]
            } else {
                [(i * 37) as u8, (i * 91) as u8, (i * 13) as u8]
            });
        }
    }
    v
}

#[test]
fn tool_written_files() {
    let (w, h) = (75u32, 29u32);
    let mut ran = 0;
    for (name, colours, tool, tol) in [
        ("ppmtopict8", 200u32, "ppmtopict", 0u8),
        ("magick_rgb", 100_000, "magick", 0),
        ("magick_pal", 16, "magick", 0),
        ("magick_jpeg", 100_000, "magick-jpeg", 40),
    ] {
        let exe = if tool == "ppmtopict" {
            "ppmtopict"
        } else {
            "magick"
        };
        if !have(exe, "-version") {
            continue;
        }
        let mut src = format!("P6\n{w} {h}\n255\n").into_bytes();
        let rgb = pattern(w, h, colours);
        src.extend(&rgb);
        let p = tmp(&format!("{name}.ppm"));
        std::fs::write(&p, &src).unwrap();
        let f = tmp(&format!("{name}.pict"));
        let ok = match tool {
            "ppmtopict" => {
                let out = Command::new("ppmtopict")
                    .arg(&p)
                    .stderr(Stdio::null())
                    .output()
                    .unwrap();
                std::fs::write(&f, &out.stdout).unwrap();
                out.status.success()
            }
            _ => {
                let mut c = Command::new("magick");
                c.arg(&p);
                if tool == "magick-jpeg" {
                    c.args(["-compress", "JPEG", "-quality", "92"]);
                }
                c.arg(format!("pict:{}", f.display()))
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            }
        };
        assert!(ok, "{name}: {tool} failed");
        let data = std::fs::read(&f).unwrap();
        let i = info(&data).unwrap();
        assert_eq!((i.width, i.height), (w, h), "{name}");
        let img = decode(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!img.truncated, "{name}");
        let (n, m) = diff(&img.pixels, &rgb, tol);
        eprintln!("{name}: vs source max {m}, {n} over {tol}");
        assert_eq!(n, 0, "{name}: {n} pixels off by more than {tol} (max {m})");
        if tool == "magick-jpeg" {
            assert_eq!(img.jpegs, 1);
        }
        ran += 1;
    }
    if ran == 0 {
        eprintln!("neither netpbm nor ImageMagick installed; skipped");
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
        "twelvemonkeys/imageio/imageio-pict/src/test/resources/pict",
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
            if matches!(ext.as_str(), "pict" | "pct" | "pic") {
                v.push(p);
            }
        }
    }
    v.sort();
    v
}

/// Prints a line per corpus file: decode time, what was drawn, and the difference from
/// ImageMagick. Fails when more than 0.1% of the compared pixels differ beyond the
/// tolerance (0, 7 for 16-bit pixels, 48 for JPEG).
#[test]
fn corpus_against_imagemagick() {
    let files = corpus_files();
    if files.is_empty() {
        eprintln!("PICT corpus not fetched; skipped");
        return;
    }
    let im = have("magick", "-version");
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
                eprintln!("{name}: {e}");
                continue;
            }
        };
        let i = info(&data).unwrap();
        if std::env::var_os("SPECULAR_PICT_DUMP").is_some() {
            let mut p = format!("P6\n{} {}\n255\n", img.width, img.height).into_bytes();
            p.extend(img.pixels.chunks_exact(4).flat_map(|c| [c[0], c[1], c[2]]));
            std::fs::write(tmp(&format!("{name}.ppm")), p).unwrap();
        }
        let o = if im { magick(f) } else { None };
        let verdict = match &o {
            Some(_) if img.shapes > 0 => {
                "magick does not draw QuickDraw shapes; see tests/twelvemonkeys.rs".to_string()
            }
            Some(_) if MAGICK_MISPLACES.contains(&name.as_str()) => {
                "magick ignores the frame origin; not compared".to_string()
            }
            Some(_) if MAGICK_BLANK.contains(&name.as_str()) => {
                "magick renders nothing; not compared".to_string()
            }
            Some((ow, oh, rgb)) => {
                let tol = if img.jpegs > 0 {
                    48
                } else if name.contains("16") {
                    7 // 5-bit channels: we replicate bits, ImageMagick shifts
                } else {
                    0
                };
                let (n, m, total) = overlap_diff(&img, *ow, *oh, rgb, tol);
                if n * 1000 > total {
                    failures.push(format!("{name}: {n}/{total} px over {tol} (max {m})"));
                }
                format!("vs magick ({ow}x{oh}): {n}/{total} px over {tol} (max {m})")
            }
            None => "no oracle".into(),
        };
        eprintln!(
            "{name}: {}x{} v{} {ms:.1} ms bitmaps={} jpegs={} sole_jpeg={} truncated={} {verdict}",
            img.width,
            img.height,
            i.version,
            img.bitmaps,
            img.jpegs,
            sole_jpeg(&data).is_some(),
            img.truncated
        );
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The atoms of a QuickTime movie, depth first: (type, body).
fn atoms<'a>(d: &'a [u8], out: &mut Vec<([u8; 4], &'a [u8])>) {
    let mut p = 0;
    while p + 8 <= d.len() {
        let size = u32::from_be_bytes(d[p..p + 4].try_into().unwrap()) as usize;
        let kind: [u8; 4] = d[p + 4..p + 8].try_into().unwrap();
        if size < 8 || p + size > d.len() {
            break;
        }
        let body = &d[p + 8..p + size];
        out.push((kind, body));
        if matches!(&kind, b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl") {
            atoms(body, out);
        }
        p += size;
    }
}

/// The first video sample description (as a QuickTime image description) and the first
/// frame of a one-track movie.
fn mov_first_frame(mov: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut list = Vec::new();
    atoms(mov, &mut list);
    let get = |k: &[u8; 4]| list.iter().find(|(t, _)| t == k).map(|(_, b)| *b);
    let stsd = get(b"stsd")?;
    let n = u32::from_be_bytes(stsd.get(8..12)?.try_into().ok()?) as usize;
    let desc = stsd.get(8..8 + n)?.to_vec();
    let stco = get(b"stco")?;
    let off = u32::from_be_bytes(stco.get(8..12)?.try_into().ok()?) as usize;
    let stsz = get(b"stsz")?;
    let fixed = u32::from_be_bytes(stsz.get(4..8)?.try_into().ok()?) as usize;
    let size = if fixed != 0 {
        fixed
    } else {
        u32::from_be_bytes(stsz.get(12..16)?.try_into().ok()?) as usize
    };
    Some((desc, mov.get(off..off + size)?.to_vec()))
}

/// A version 2 picture of `w`×`h` holding one CompressedQuickTime image.
fn qt_pict(w: u16, h: u16, desc: &[u8], frame: &[u8]) -> Vec<u8> {
    let mut b = vec![0u8; 68];
    for (at, v) in [(2, 0x0001_0000u32), (18, 0x0001_0000), (34, 0x4000_0000)] {
        b[at..at + 4].copy_from_slice(&v.to_be_bytes());
    }
    b[56..58].copy_from_slice(&h.to_be_bytes());
    b[58..60].copy_from_slice(&w.to_be_bytes());
    let mut desc = desc.to_vec();
    desc[44..48].copy_from_slice(&(frame.len() as u32).to_be_bytes());
    b.extend(&desc);
    b.extend(frame);
    let mut v = vec![0u8; 512];
    v.extend([0, 0, 0, 0, 0, 0]);
    v.extend(h.to_be_bytes());
    v.extend(w.to_be_bytes());
    v.extend([0x00, 0x11, 0x02, 0xff, 0x0c, 0x00, 0xff, 0xfe, 0, 0]);
    v.extend(0x0048_0000u32.to_be_bytes());
    v.extend(0x0048_0000u32.to_be_bytes());
    v.extend([0, 0, 0, 0]);
    v.extend(h.to_be_bytes());
    v.extend(w.to_be_bytes());
    v.extend([0; 4]);
    v.extend([0x82, 0x00]);
    v.extend((b.len() as u32).to_be_bytes());
    v.extend(b);
    if v.len() % 2 == 1 {
        v.push(0);
    }
    v.extend([0x00, 0xff]);
    v
}

/// QuickTime Animation, Graphics, Apple Video and Cinepak pictures: FFmpeg encodes a test
/// pattern to a movie, its first frame goes into a CompressedQuickTime opcode, and FFmpeg's
/// own decoding of the movie is the reference.
#[test]
fn quicktime_video_codecs_against_ffmpeg() {
    if !have("ffmpeg", "-version") {
        eprintln!("skipped: no ffmpeg");
        return;
    }
    // Cinepak and grey Animation want widths in fours (testsrc2 makes even sizes).
    let (w, h) = (96u16, 64u16);
    let cases: &[(&str, &str, &[&str])] = &[
        ("qtrle-24", "qtrle", &["-pix_fmt", "rgb24"]),
        ("qtrle-16", "qtrle", &["-pix_fmt", "rgb555be"]),
        ("qtrle-32", "qtrle", &["-pix_fmt", "argb"]),
        ("qtrle-grey", "qtrle", &["-pix_fmt", "gray"]),
        ("rpza", "rpza", &["-pix_fmt", "rgb555be"]),
        ("smc", "smc", &["-pix_fmt", "pal8"]),
        ("cinepak", "cinepak", &["-pix_fmt", "rgb24"]),
    ];
    let mut failures = Vec::new();
    for (name, codec, args) in cases {
        let mov = tmp(&format!("{name}.mov"));
        let ok = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!("testsrc2=size={w}x{h}:rate=1:duration=1"))
            .args(["-frames:v", "1", "-c:v", codec])
            .args(*args)
            .arg(&mov)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            eprintln!("{name}: ffmpeg could not encode it, skipped");
            continue;
        }
        let reference = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&mov)
            .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .output()
            .unwrap()
            .stdout;
        let bytes = std::fs::read(&mov).unwrap();
        let (desc, frame) = mov_first_frame(&bytes).expect("movie layout");
        let pict = qt_pict(w, h, &desc, &frame);
        let img = match decode(&pict) {
            Ok(i) => i,
            Err(e) => {
                failures.push(format!("{name}: {e}"));
                continue;
            }
        };
        assert_eq!((img.width, img.height), (u32::from(w), u32::from(h)));
        let (n, m) = diff(&img.pixels, &reference, 0);
        println!("{name}: {n} pixels differ (largest {m})");
        if n > 0 {
            failures.push(format!("{name}: {n} pixels differ (largest {m})"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
