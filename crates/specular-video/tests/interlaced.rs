//! Interlaced H.264 clips (tools/corpus/video-refs.sh: x264 MBAFF and JVT conformance streams
//! remuxed with one field per sample): which samples make frames, and the player's frames
//! with hardware decoding on and off (same count, same timing, same pixels). Skips itself
//! without the reference set.

use std::path::{Path, PathBuf};
use std::time::Duration;

use specular_core::{CancelToken, Orientation};
use specular_decode::Frames;
use specular_h264::FieldScan;
use specular_video::mp4;
use specular_video::play::Clip;
use specular_video::sw_h264::SwH264;

fn refs() -> Option<PathBuf> {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/out/video-ref");
    if d.join("manifest.tsv").is_file() {
        Some(d)
    } else {
        eprintln!("skipped: run tools/corpus/video-refs.sh");
        None
    }
}

/// The interlaced clips of the manifest: name and frame count.
fn interlaced(d: &Path) -> Vec<(String, usize)> {
    std::fs::read_to_string(d.join("manifest.tsv"))
        .unwrap()
        .lines()
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .filter(|f| f[0].starts_with("h264-jvt-") || f[0].starts_with("h264-mbaff"))
        .map(|f| (f[0].to_string(), f[4].parse().unwrap()))
        .collect()
}

/// The lone-ending clip (not in the manifest): its frames are its reference's.
const LONE_END: &str = "h264-jvt-cvpa1-lone-end";

/// Every frame the player makes of a clip: delay and RGBA, with hardware decoding on or off,
/// and the frame count it announced. Plays it twice (the second loop must be the same).
fn play(path: &Path, hw: bool) -> (Vec<(Duration, Vec<u8>)>, Option<u32>) {
    specular_hwdec::set_enabled(hw);
    let clip = Clip::from_bytes(std::fs::read(path).unwrap()).unwrap();
    let size = (clip.track().width, clip.track().height);
    let (mut frames, info) = clip.frames(Orientation(1), size).unwrap();
    let cancel = CancelToken::new();
    let mut out = Vec::new();
    while let Some(f) = frames.next_frame(&cancel) {
        let f = f.unwrap();
        assert_eq!(f.index as usize, out.len(), "{path:?}");
        out.push((f.delay, f.raster.planes[0].data.clone()));
    }
    frames.rewind().unwrap();
    let mut n = 0;
    while let Some(f) = frames.next_frame(&cancel) {
        let f = f.unwrap();
        assert_eq!(f.index as usize, n, "{path:?}: second loop");
        assert!(
            f.raster.planes[0].data == out[n].1,
            "{path:?}: second loop frame {n}"
        );
        n += 1;
    }
    assert_eq!(n, out.len(), "{path:?}: second loop");
    (out, info.frame_count)
}

/// The slice-header scan counts a frame per field pair, per frame and per lone field: as many
/// as the decoder makes; and the clips really store fields per sample where they code fields.
#[test]
fn field_scan_counts_the_frames() {
    let Some(d) = refs() else { return };
    let mut clips = interlaced(&d);
    clips.push((LONE_END.into(), 89));
    assert!(
        clips.len() > 10,
        "interlaced clips missing: rerun video-refs.sh"
    );
    for (name, want) in clips {
        let data = std::fs::read(d.join("clips").join(format!("{name}.mp4"))).unwrap();
        let m = mp4::parse(&data, true).unwrap();
        let v = m.video.as_ref().unwrap();
        let mut scan = FieldScan::with_avcc(&v.config).unwrap();
        let fields: Vec<_> = v
            .samples
            .iter()
            .map(|s| scan.sample(mp4::sample_bytes(&data, s).unwrap()).unwrap())
            .collect();
        let frames = fields.iter().filter(|f| f.starts_frame).count();
        let seconds = fields.iter().filter(|f| f.completes_open).count();
        eprintln!(
            "{name}: {} samples, {frames} frames, {seconds} second fields",
            v.samples.len()
        );
        assert_eq!(frames, want, "{name}");
        // Every sample starts a frame or completes one.
        assert!(
            fields.iter().all(|f| f.starts_frame || f.completes_open),
            "{name}"
        );
        if name.starts_with("h264-mbaff") || name.contains("-cvma1") || name.contains("-cama1") {
            assert_eq!(seconds, 0, "{name}: MBAFF frames only");
        } else if !name.contains("camp-mbaff") && !name.contains("hcamff1") {
            assert!(seconds > 0, "{name}: no field per sample");
        }
        assert_eq!(
            fields.last().unwrap().leaves_open,
            name == LONE_END,
            "{name}"
        );
        // The software decoder labels each frame with the time of the sample that starts it.
        let mut starts: Vec<i64> = v
            .samples
            .iter()
            .zip(&fields)
            .filter(|(_, f)| f.starts_frame)
            .map(|(s, _)| s.cts)
            .collect();
        starts.sort_unstable();
        let mut dec = SwH264::new(&v.config).unwrap();
        let mut out = Vec::new();
        let cancel = CancelToken::new();
        for s in &v.samples {
            let b = mp4::sample_bytes(&data, s).unwrap();
            dec.step(Some((b, s.cts)), &cancel, &mut out).unwrap();
        }
        dec.step(None, &cancel, &mut out).unwrap();
        let mut labels: Vec<i64> = out.iter().map(|f| f.0).collect();
        labels.sort_unstable();
        assert_eq!(labels, starts, "{name}: frame labels");
    }
}

/// The player with hardware decoding on and off: one frame per field pair, labelled and
/// timed alike, the same pixels; a clip ending on a lone first field shows it (line-doubled)
/// both ways. One test: hardware decoding is switched process-wide.
#[test]
fn interlaced_clips_play_alike_with_and_without_hardware() {
    let Some(d) = refs() else { return };
    let mut clips = interlaced(&d);
    clips.push((LONE_END.into(), 89));
    for (name, want) in clips {
        let path = d.join("clips").join(format!("{name}.mp4"));
        let data = std::fs::read(&path).unwrap();
        let m = mp4::parse(&data, true).unwrap();
        let v = m.video.as_ref().unwrap();
        let ts = v.timescale as u128;
        let first = v.samples.iter().map(|s| s.cts).min().unwrap();
        let end = v
            .samples
            .iter()
            .map(|s| s.cts + s.duration as i64)
            .max()
            .unwrap();
        let (soft, count) = play(&path, false);
        assert_eq!(soft.len(), want, "{name}: software frames");
        assert_eq!(count, Some(want as u32), "{name}: announced frames");
        // The frames cover the clip: each field pair lasts both fields.
        let total: Duration = soft.iter().map(|f| f.0).sum();
        let span = Duration::from_micros(((end - first) as u128 * 1_000_000 / ts) as u64);
        assert!(
            total.abs_diff(span) <= Duration::from_millis(1) * want as u32,
            "{name}: {total:?} for a {span:?} clip"
        );
        let (hard, count) = play(&path, true);
        assert_eq!(
            count,
            Some(want as u32),
            "{name}: announced frames, hardware"
        );
        assert_eq!(hard.len(), soft.len(), "{name}: frames, hardware");
        for (k, (h, s)) in hard.iter().zip(&soft).enumerate() {
            assert_eq!(h.0, s.0, "{name}: frame {k} delay");
            assert!(h.1 == s.1, "{name}: frame {k} differs with hardware");
        }
        eprintln!("{name}: {want} frames, {total:?}, alike with and without hardware");
    }
    specular_hwdec::set_enabled(true);
}

/// The lone first field at the end of the clip comes out line-doubled: every other row is a
/// copy of the row above it.
#[test]
fn lone_last_field_is_line_doubled() {
    let Some(d) = refs() else { return };
    let data = std::fs::read(d.join("clips").join(format!("{LONE_END}.mp4"))).unwrap();
    let m = mp4::parse(&data, true).unwrap();
    let v = m.video.as_ref().unwrap();
    let mut dec = SwH264::new(&v.config).unwrap();
    let mut out = Vec::new();
    let cancel = CancelToken::new();
    for s in &v.samples {
        let b = mp4::sample_bytes(&data, s).unwrap();
        dec.step(Some((b, s.cts)), &cancel, &mut out).unwrap();
    }
    dec.step(None, &cancel, &mut out).unwrap();
    assert_eq!(out.len(), 89);
    let last = v.samples.last().unwrap().cts;
    let f = &out.iter().find(|f| f.0 == last).unwrap().1;
    let (w, h) = (f.width as usize, f.height as usize);
    let y = &f.planes[0];
    let stride = y.len() / h;
    let row = |r: usize| &y[r * stride..][..w];
    let top = (0..h / 2).all(|r| row(2 * r) == row(2 * r + 1));
    let bottom = (1..h / 2).all(|r| row(2 * r) == row(2 * r - 1));
    assert!(top || bottom, "the lone field is not line-doubled");
    // The other frames are the full clip's.
    let r = std::fs::read(d.join("ref").join(format!("{LONE_END}.yuv"))).unwrap();
    let fb = w * h * 3 / 2;
    out.sort_by_key(|f| f.0);
    for (k, (_, f)) in out.iter().enumerate().take(88) {
        assert!(f.planes.concat() == r[k * fb..][..fb], "frame {k}");
    }
}
