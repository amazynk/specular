//! Motion JPEG, JPEG 2000, FLV and ASF (tests/data, made by
//! tests/data/gen_fixtures_videocodecs_extra.sh with ffmpeg, whose decodes are the oracle): every
//! frame exact, seeks, facts and refusals. With `SPECULAR_VIDEOCODECS_EXTRA_CORPUS` set
//! (tools/corpus/fetch-videocodecs-extra.sh), public samples in their containers too, one `ORACLE`
//! line per file.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{Format, Frames, Session};
use specular_video::session::{self, VideoSession};

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn open_path(path: &Path, format: Format) -> VideoSession {
    let src = Arc::new(Source::open(path).unwrap());
    session::open(src, format).unwrap()
}

fn open(name: &str, format: Format) -> VideoSession {
    open_path(&data(name), format)
}

fn planes(r: &Raster) -> Vec<u8> {
    assert!(matches!(r.layout, PixelLayout::Yuv(_)), "{:?}", r.layout);
    let mut v = Vec::new();
    for p in &r.planes {
        let bps = p.stride / p.width as usize;
        for row in p.data.chunks(p.stride).take(p.height as usize) {
            v.extend_from_slice(&row[..p.width as usize * bps]);
        }
    }
    v
}

/// Bytes per sample of a raster's planes.
fn sample_bytes(r: &Raster) -> usize {
    let p = &r.planes[0];
    p.stride / p.width as usize
}

/// The largest difference between two runs of samples (`bps` bytes each, little-endian).
fn worst(got: &[u8], want: &[u8], bps: usize) -> u32 {
    if bps == 2 {
        got.chunks_exact(2)
            .zip(want.chunks_exact(2))
            .map(|(a, b)| {
                let (a, b) = (
                    u16::from_le_bytes([a[0], a[1]]),
                    u16::from_le_bytes([b[0], b[1]]),
                );
                u32::from(a.abs_diff(b))
            })
            .max()
            .unwrap_or(0)
    } else {
        got.iter()
            .zip(want)
            .map(|(&a, &b)| u32::from(a.abs_diff(b)))
            .max()
            .unwrap_or(0)
    }
}

/// Frames decoded (woven, not deinterlaced), how many differ from `want` (ffmpeg's rawvideo)
/// by more than `tol` in any sample, the bytes compared, and the samples that differ at all.
fn frames_within(s: &VideoSession, want: &[u8], tol: u32) -> (usize, usize, usize, usize) {
    frames_upto(s, want, tol, usize::MAX)
}

/// [`frames_within`] over the first `limit` frames.
fn frames_upto(
    s: &VideoSession,
    want: &[u8],
    tol: u32,
    limit: usize,
) -> (usize, usize, usize, usize) {
    let mut frames = s.video_frames().unwrap();
    frames.set_deinterlace(false);
    let cancel = CancelToken::new();
    let (mut n, mut at, mut bad, mut off) = (0, 0, 0, 0);
    while n < limit
        && let Some(f) = frames.next_frame(&cancel)
    {
        let f = f.unwrap();
        if f.index as usize != n {
            eprintln!("frame {n} came as {}", f.index);
            bad += 1;
            break;
        }
        let bps = sample_bytes(&f.raster);
        let got = planes(&f.raster);
        match want.get(at..at + got.len()) {
            Some(w) if w == &got[..] => {}
            Some(w) => {
                off += got
                    .chunks(bps)
                    .zip(w.chunks(bps))
                    .filter(|(a, b)| a != b)
                    .count();
                if worst(&got, w, bps) > tol {
                    bad += 1;
                }
            }
            None => bad += 1,
        }
        at += got.len();
        n += 1;
    }
    (n, bad, at, off)
}

/// All frames' planes (for looking at a mismatch).
fn dump(s: &VideoSession, to: &Path) {
    let mut frames = s.video_frames().unwrap();
    frames.set_deinterlace(false);
    let cancel = CancelToken::new();
    let mut v = Vec::new();
    while let Some(f) = frames.next_frame(&cancel) {
        v.extend(planes(&f.unwrap().raster));
    }
    std::fs::write(to, v).unwrap();
}

fn all_frames_exact(name: &str, format: Format, frames: usize) {
    all_frames_within(name, format, frames, 0);
}

/// Every frame within `tol` of ffmpeg's in every sample (JPEG: libjpeg-turbo's integer IDCT
/// here, ffmpeg's own there; JPEG 2000 9/7: floating point here, fixed point there).
fn all_frames_within(name: &str, format: Format, frames: usize, tol: u32) {
    let s = open(name, format);
    let ext = name.rsplit('.').next().unwrap();
    let want = std::fs::read(data(&name.replace(ext, "yuv"))).unwrap();
    let (n, bad, at, off) = frames_within(&s, &want, tol);
    if off > 0 {
        eprintln!("{name}: {off} samples differ by at most {tol}");
    }
    if bad > 0
        && let Some(d) = std::env::var_os("SPECULAR_VIDEOCODECS_EXTRA_DUMP")
    {
        dump(&s, &Path::new(&d).join(format!("{name}.got")));
    }
    assert_eq!(bad, 0, "{name}: {bad} of {n} frames differ from ffmpeg");
    assert_eq!(at, want.len(), "{name}: not as many frames as ffmpeg");
    assert_eq!(n, frames, "{name}");
    assert_eq!(Some(n as u32), s.animation().and_then(|a| a.frame_count));
    eprintln!("{name}: {n} frames match ffmpeg");
}

#[test]
fn motion_jpeg_matches_ffmpeg() {
    all_frames_within("mjpeg.avi", Format::Avi, 10, 1);
    all_frames_within("mjpeg420.mov", Format::QuickTime, 10, 1);
    all_frames_within("mjpeg-nodht.mkv", Format::Matroska, 10, 1);
}

#[test]
fn jpeg_2000_video_matches_ffmpeg() {
    all_frames_within("mjp2.mov", Format::QuickTime, 10, 1);
    all_frames_within("j2k-422-10.mxf", Format::Mxf, 10, 1);
    all_frames_within("j2k.mxf", Format::Mxf, 1, 1);
}

#[test]
fn vp9_444_matches_ffmpeg() {
    all_frames_exact("vp9-444.webm", Format::WebM, 5);
}

#[test]
fn flash_video_matches_ffmpeg() {
    all_frames_exact("h264-aac.flv", Format::Flv, 10);
    let s = open("h264-aac.flv", Format::Flv);
    let f = s.video().unwrap();
    assert_eq!(f.audio.as_deref().map(|a| a.starts_with("AAC")), Some(true));
}

#[test]
fn sorenson_spark_matches_ffmpeg() {
    all_frames_exact("spark.flv", Format::Flv, 10);
}

#[test]
fn asf_matches_ffmpeg() {
    all_frames_exact("mpeg4.wmv", Format::Asf, 10);
    let s = open("mpeg4.wmv", Format::Asf);
    assert_eq!(
        s.video()
            .unwrap()
            .audio
            .as_deref()
            .map(|a| a.starts_with("PCM")),
        Some(true)
    );
}

#[test]
fn refusals_say_why() {
    let s = open("msmpeg4.wmv", Format::Asf);
    assert_eq!(
        s.video().unwrap().unplayable.as_deref(),
        Some("Windows Media Video 7/8 video")
    );
}

/// Every frame after a seek is the one decoding in order gives.
#[test]
fn seeks_land_on_the_right_frame() {
    for (name, format) in [
        ("mjpeg.avi", Format::Avi),
        ("mjp2.mov", Format::QuickTime),
        ("mpeg4.wmv", Format::Asf),
        ("h264-aac.flv", Format::Flv),
    ] {
        let s = open(name, format);
        let cancel = CancelToken::new();
        // Decoding in order is checked against ffmpeg above.
        let mut frames = s.video_frames().unwrap();
        frames.set_deinterlace(false);
        let mut seq = Vec::new();
        while let Some(f) = frames.next_frame(&cancel) {
            seq.push(planes(&f.unwrap().raster));
        }
        let mut frames = s.video_frames().unwrap();
        frames.set_deinterlace(false);
        for k in [7usize, 2, 9, 0, 5] {
            frames.seek(k as u32).unwrap();
            let f = frames.next_frame(&cancel).unwrap().unwrap();
            assert_eq!(f.index as usize, k);
            assert!(
                planes(&f.raster) == seq[k],
                "{name}: frame {k} after a seek"
            );
        }
    }
}

/// Public samples (samples.ffmpeg.org, FATE) and clips remuxed from them, in their containers,
/// against ffmpeg's decodes (`NAME.yuv` beside each, made by
/// tools/corpus/fetch-videocodecs-extra.sh; `list.tsv` gives each file's tolerance and how many
/// frames to compare, or that it must be refused in words).
#[test]
fn corpus_matches_ffmpeg() {
    let Some(dir) = std::env::var_os("SPECULAR_VIDEOCODECS_EXTRA_CORPUS") else {
        eprintln!("SPECULAR_VIDEOCODECS_EXTRA_CORPUS not set: skipped");
        return;
    };
    let dir = PathBuf::from(dir);
    let list = std::fs::read_to_string(dir.join("list.tsv")).unwrap_or_default();
    let mut failed = Vec::new();
    for line in list
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let [name, fmt, label, tol, frames] = f[..] else {
            continue;
        };
        let format = match fmt {
            "avi" => Format::Avi,
            "mov" => Format::QuickTime,
            "mp4" => Format::Mp4,
            "mkv" => Format::Matroska,
            "mxf" => Format::Mxf,
            "flv" => Format::Flv,
            "asf" => Format::Asf,
            "ts" => Format::MpegTs,
            "dv" => Format::Dv,
            _ => continue,
        };
        let path = dir.join(name);
        let s = open_path(&path, format);
        let why = s.video().unwrap().unplayable.clone();
        if frames == "refuse" {
            // Refused in words (a playable file here would be news: check it against ffmpeg).
            let ok = why.is_some();
            println!(
                "ORACLE\t{label}\t{name}\t{}",
                if ok { "pass" } else { "fail" }
            );
            eprintln!("{name}: refused: {why:?}");
            if !ok {
                failed.push(name.to_owned());
            }
            continue;
        }
        if let Some(why) = why {
            println!("ORACLE\t{label}\t{name}\tfail");
            eprintln!("{name}: refused: {why}");
            failed.push(name.to_owned());
            continue;
        }
        if let Some(n) = frames.strip_prefix("play:") {
            // No reference to compare with: the frames must decode.
            let n: usize = n.parse().unwrap();
            let mut it = s.video_frames().unwrap();
            let cancel = CancelToken::new();
            let got = (0..n)
                .filter(|_| it.next_frame(&cancel).is_some_and(|f| f.is_ok()))
                .count();
            println!("ORACLE\t{label}\t{name}\tskip");
            eprintln!("{name}: {got} of {n} frames decode (no reference)");
            if got != n {
                failed.push(name.to_owned());
            }
            continue;
        }
        let Ok(want) = std::fs::read(path.with_extension("yuv")) else {
            println!("ORACLE\t{label}\t{name}\tskip");
            continue;
        };
        let (tol, limit): (u32, usize) = (tol.parse().unwrap(), frames.parse().unwrap());
        let t = Instant::now();
        let (n, bad, at, off) = frames_upto(&s, &want, tol, limit);
        let ok = bad == 0 && at == want.len();
        println!(
            "ORACLE\t{label}\t{name}\t{}",
            if ok { "pass" } else { "fail" }
        );
        eprintln!(
            "{name}: {bad} of {n} frames differ by more than {tol} ({off} samples differ; {at} bytes vs {}), {:.0} fps",
            want.len(),
            n as f64 / t.elapsed().as_secs_f64(),
        );
        if !ok {
            if let Some(d) = std::env::var_os("SPECULAR_VIDEOCODECS_EXTRA_DUMP") {
                let base = Path::new(name)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                dump(&s, &Path::new(&d).join(format!("{base}.got")));
            }
            failed.push(name.to_owned());
        }
    }
    assert!(failed.is_empty(), "differ from ffmpeg: {failed:?}");
}
