//! Fidelity round 7: every movie of the fidelity7 corpus goes through the app's path
//! (`session::open`, software decoders, presentation order) twice: as decoded planes (woven,
//! no deinterlacing; the raster's own Y/Cb/Cr) and as the viewer shows them (the CPU twin of
//! the GPU conversion, `yuv::to_rgb`, with the encoding the session chose; orientation and
//! pixel aspect as facts). `tools/corpus/fidelity7-oracle.py check` compares the planes
//! bit for bit with libavcodec and a second decoder (dav1d and libaom, libvpx, the
//! conformance suites' own checksums), the shown frames with a reference conversion from
//! the tags ffprobe reads, and the facts with ffprobe's; it classifies each difference and
//! prints one `ORACLE` line per file.
//!
//! ```text
//! tools/corpus/fetch-fidelity7.sh            # matrices and conformance subsets (~150 MB)
//! SPECULAR_FIDELITY7_CORPUS=<dir> \
//!   cargo test -p specular-video --test fidelity7 -- --ignored --nocapture
//! ```
//!
//! `SPECULAR_FIDELITY_ONLY=h264,vp9` limits the groups (top-level folders);
//! `SPECULAR_FIDELITY_KEEP=1` keeps the dumps under `TMPDIR` (`fidelity7-<pid>/`);
//! `SPECULAR_FIDELITY7_FRAMES` caps the frames per file (default 24).

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source, yuv};
use specular_decode::{Format, Frames, Session};
use specular_video::session;

fn corpus() -> Option<PathBuf> {
    let marked = |p: PathBuf| p.join(".fidelity7").is_file().then_some(p);
    if let Some(d) = std::env::var_os("SPECULAR_FIDELITY7_CORPUS") {
        return Some(PathBuf::from(d));
    }
    // The pass-rate runner passes its corpus as SPECULAR_FIDELITY_CORPUS.
    if let Some(d) = std::env::var_os("SPECULAR_FIDELITY_CORPUS").and_then(|d| marked(d.into())) {
        return Some(d);
    }
    let t = std::env::var_os("CARGO_TARGET_DIR")?;
    marked(Path::new(&t).join("corpus/fidelity7"))
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // `src/`: elementary streams kept beside their remuxes; `jxl/`: the still-image
        // cases of the round (specular-decode's jxl_cmyk test).
        if name.starts_with('.') || name == "src" || name == "jxl" {
            continue;
        }
        if p.is_dir() {
            files(&p, out);
        } else if !matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("md5" | "json" | "txt" | "tsv" | "yuv" | "sha256" | "framemd5" | "whole")
        ) {
            out.push(p);
        }
    }
}

fn format_of(p: &Path) -> Format {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "mov" => Format::QuickTime,
        "mkv" => Format::Matroska,
        "webm" => Format::WebM,
        "ts" | "m2ts" => Format::MpegTs,
        "mpg" | "vob" => Format::MpegPs,
        "m2v" | "m1v" => Format::MpegVideo,
        "avi" => Format::Avi,
        "ogv" | "ogg" => Format::Ogg,
        "ivf" => Format::Ivf,
        "mxf" => Format::Mxf,
        "flv" => Format::Flv,
        "asf" | "wmv" => Format::Asf,
        "dv" => Format::Dv,
        _ => Format::Mp4,
    }
}

/// The planes of a frame, rows packed, 16-bit samples little-endian.
fn planes(r: &Raster, out: &mut Vec<u8>) {
    for p in &r.planes {
        let bps = p.stride / p.width.max(1) as usize;
        for row in p.data.chunks(p.stride).take(p.height as usize) {
            let row = &row[..p.width as usize * bps];
            if bps == 2 || bps == 8 {
                for s in row.chunks_exact(2) {
                    out.extend_from_slice(&u16::from_ne_bytes([s[0], s[1]]).to_le_bytes());
                }
            } else {
                out.extend_from_slice(row);
            }
        }
    }
}

/// The shown frame as 16-bit little-endian RGB.
fn shown(r: &Raster, out: &mut Vec<u8>) -> Option<String> {
    let rgb = match r.layout {
        PixelLayout::Yuv(_) => yuv::to_rgb(r)?,
        _ => r.clone(),
    };
    let p = &rgb.planes[0];
    let (w, h) = (rgb.width as usize, rgb.height as usize);
    let what = match rgb.layout {
        PixelLayout::Rgba => {
            for row in p.data.chunks(p.stride).take(h) {
                for px in row[..4 * w].chunks_exact(4) {
                    for &c in &px[..3] {
                        out.extend_from_slice(&(u16::from(c) * 257).to_le_bytes());
                    }
                }
            }
            "srgb".to_owned()
        }
        PixelLayout::Rgba16(e) => {
            for row in p.data.chunks(p.stride).take(h) {
                for px in row[..8 * w].chunks_exact(8) {
                    for c in 0..3 {
                        let v = u16::from_ne_bytes([px[2 * c], px[2 * c + 1]]);
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                }
            }
            format!("{:?}/{:?}", e.transfer, e.primaries)
        }
        _ => return None,
    };
    Some(what)
}

fn layout_facts(r: &Raster) -> String {
    match r.layout {
        PixelLayout::Yuv(e) => {
            let c = &r.planes[1];
            format!(
                "yuv\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{:?}",
                c.h_div,
                c.v_div,
                e.bits,
                e.matrix.code(),
                u8::from(e.full_range),
                u8::from(e.cosited_x),
                e.colour.transfer,
                e.colour.primaries
            )
        }
        l => format!("{l:?}\t-\t-\t-\t-\t-\t-\t-\t-").replace(' ', ""),
    }
}

/// One file: decodes up to `limit` frames, writes `<n>.planes` and `<n>.rgb` (every frame,
/// concatenated) and `<n>.frames` (a line per frame: width, height, layout facts), and
/// returns the manifest line.
fn dump(root: &Path, p: &Path, work: &Path, n: usize, limit: usize) -> String {
    let rel = p.strip_prefix(root).unwrap_or(p).display().to_string();
    let err = |e: &dyn std::fmt::Display| {
        let e = e.to_string().replace(['\t', '\n'], " ");
        format!("{rel}\terror\t{e}")
    };
    let r = std::panic::catch_unwind(|| {
        let src = match Source::open(p) {
            Ok(s) => Arc::new(s),
            Err(e) => return err(&e),
        };
        let s = match session::open(src, format_of(p)) {
            Ok(s) => s,
            Err(e) => {
                return format!(
                    "{rel}\trefused\t{}",
                    e.to_string().replace(['\t', '\n'], " ")
                );
            }
        };
        let info = s.info().clone();
        let facts = s.video().cloned();
        let track = s.movie().video.clone();
        let mut frames = match s.video_frames() {
            Ok(f) => f,
            Err(e) => {
                return format!(
                    "{rel}\trefused\t{}",
                    e.to_string().replace(['\t', '\n'], " ")
                );
            }
        };
        let interlaced = frames.deinterlacing();
        frames.set_deinterlace(false);
        let cancel = CancelToken::new();
        let (mut pl, mut rgb, mut lines) = (Vec::new(), Vec::new(), String::new());
        let mut count = 0;
        let mut failure = None;
        while count < limit {
            match frames.next_frame(&cancel) {
                None => break,
                Some(Err(e)) => {
                    failure = Some(e.to_string());
                    break;
                }
                Some(Ok(f)) => {
                    planes(&f.raster, &mut pl);
                    let what = shown(&f.raster, &mut rgb).unwrap_or_else(|| "-".into());
                    lines.push_str(&format!(
                        "{}\t{}\t{}\t{what}\n",
                        f.raster.width,
                        f.raster.height,
                        layout_facts(&f.raster)
                    ));
                    count += 1;
                }
            }
        }
        let stem = work.join(n.to_string());
        let w = |ext: &str, b: &[u8]| std::fs::write(stem.with_extension(ext), b).unwrap();
        w("planes", &pl);
        w("rgb", &rgb);
        w("frames", lines.as_bytes());
        let (par, shown_size) = track.as_ref().map_or(((1, 1), (0, 0)), |t| {
            (t.pixel_aspect.unwrap_or((1, 1)), t.shown_size())
        });
        let declared = facts.as_ref().map_or(0, |f| f.frames);
        format!(
            "{rel}\tok\t{}\t{count}\t{declared}\t{}\t{}\t{}\t{}:{}\t{}x{}\t{}\t{}",
            stem.display(),
            info.width,
            info.height,
            info.orientation.0,
            par.0,
            par.1,
            shown_size.0,
            shown_size.1,
            u8::from(interlaced),
            failure
                .unwrap_or_else(|| "-".into())
                .replace(['\t', '\n'], " ")
        )
    });
    r.unwrap_or_else(|_| err(&"panic"))
}

#[test]
#[ignore = "needs the fidelity7 corpus (tools/corpus/fetch-fidelity7.sh) and ffmpeg"]
fn fidelity7_video() {
    let Some(root) = corpus() else {
        eprintln!("fidelity7: no corpus (tools/corpus/fetch-fidelity7.sh); skipped");
        return;
    };
    // The app's own decoders: hardware decoding is a separate path with its own checks.
    if std::env::var_os("SPECULAR_FIDELITY7_HW").is_none() {
        specular_hwdec::set_enabled(false);
    }
    let py = std::env::var("SPECULAR_FIDELITY7_PY").unwrap_or_else(|_| "python3".into());
    let only: Vec<String> = std::env::var("SPECULAR_FIDELITY_ONLY")
        .map(|s| s.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    let limit = std::env::var("SPECULAR_FIDELITY7_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);
    let tmp = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let work = tmp.join(format!("fidelity7-{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    let mut list = Vec::new();
    files(&root, &mut list);
    let manifest = work.join("manifest.tsv");
    let mut m = std::fs::File::create(&manifest).unwrap();
    for (n, p) in list.iter().enumerate() {
        let group = p
            .strip_prefix(&root)
            .ok()
            .and_then(|r| r.components().next())
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .unwrap_or_default();
        if !only.is_empty() && !only.contains(&group) {
            continue;
        }
        // A suite's checksum of the whole sequence needs every frame.
        let mut whole = p.as_os_str().to_owned();
        whole.push(".whole");
        let limit = if Path::new(&whole).is_file() {
            100_000
        } else {
            limit
        };
        writeln!(m, "{}", dump(&root, p, &work, n, limit)).unwrap();
    }
    drop(m);
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/fidelity7-oracle.py");
    let out = Command::new(&py)
        .arg("-I")
        .arg(&script)
        .arg("check")
        .arg(&root)
        .arg(&manifest)
        .arg(limit.to_string())
        .output()
        .expect("run fidelity7-oracle.py");
    let text = String::from_utf8_lossy(&out.stdout);
    print!("{text}");
    let errs = String::from_utf8_lossy(&out.stderr);
    let tail: Vec<&str> = errs.lines().rev().take(20).collect();
    for l in tail.into_iter().rev() {
        eprintln!("{l}");
    }
    if std::env::var_os("SPECULAR_FIDELITY_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&work);
    }
    assert!(out.status.success(), "fidelity7-oracle.py failed");
    let unexpected: usize = text
        .lines()
        .find_map(|l| l.strip_prefix("#\tunexpected\t"))
        .and_then(|n| n.trim().parse().ok())
        .expect("the script's summary");
    assert_eq!(unexpected, 0, "movies differ from the references");
}
