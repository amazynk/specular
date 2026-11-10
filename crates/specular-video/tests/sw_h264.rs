//! Software H.264 (`specular-h264` behind [`specular_video::sw_h264`]) against libavcodec's
//! decode of the reference clips (tools/corpus/video-refs.sh): every sample of every frame
//! must match. Skips itself without the reference set.
//!
//! `SPECULAR_H264_BENCH=<clip.mp4>` with `--ignored bench` times a clip (time to the first
//! picture, frames per second) without comparing it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use specular_core::CancelToken;
use specular_hwdec::video::VideoFrame;
use specular_video::mp4;
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

/// Decodes a whole clip: pictures by composition time, time to the first, total time.
fn decode(data: &[u8]) -> (Vec<(i64, VideoFrame)>, Duration, Duration) {
    let m = mp4::parse(data, true).unwrap();
    let v = m.video.as_ref().unwrap();
    assert_eq!(v.codec, Some(mp4::Codec::H264));
    let cancel = CancelToken::new();
    let t0 = Instant::now();
    let mut first = None;
    let mut dec = SwH264::new(&v.config).unwrap();
    let mut out = Vec::new();
    for s in &v.samples {
        let b = mp4::sample_bytes(data, s).unwrap();
        dec.step(Some((b, s.cts)), &cancel, &mut out).unwrap();
        if first.is_none() && !out.is_empty() {
            first = Some(t0.elapsed());
        }
    }
    dec.step(None, &cancel, &mut out).unwrap();
    let took = t0.elapsed();
    out.sort_by_key(|f| f.0);
    (out, first.unwrap_or(took), took)
}

#[test]
fn software_decode_matches_the_reference() {
    let Some(d) = refs() else { return };
    let manifest = std::fs::read_to_string(d.join("manifest.tsv")).unwrap();
    let mut bad = Vec::new();
    for line in manifest.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.get(1) != Some(&"h264") {
            continue;
        }
        let name = f[0];
        let path = d.join("clips").join(format!("{name}.mp4"));
        let data = std::fs::read(&path).unwrap();
        let (frames, first, took) = decode(&data);
        let (w, h) = (
            f[2].parse::<usize>().unwrap(),
            f[3].parse::<usize>().unwrap(),
        );
        let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
        let frame_bytes = w * h + 2 * cw * ch;
        let r = std::fs::read(d.join("ref").join(format!("{name}.yuv"))).unwrap();
        assert_eq!(r.len() / frame_bytes, frames.len(), "{name}: frame count");
        let mut wrong = 0;
        for (k, (_, f)) in frames.iter().enumerate() {
            assert_eq!((f.width as usize, f.height as usize), (w, h), "{name}");
            if f.planes.concat() != r[k * frame_bytes..][..frame_bytes] {
                if wrong == 0 {
                    eprintln!("{name}: first mismatch in presentation frame {k}");
                }
                wrong += 1;
            }
        }
        eprintln!(
            "{name}: {} frames, first after {first:?}, {:.1} fps, {wrong} wrong",
            frames.len(),
            frames.len() as f64 / took.as_secs_f64()
        );
        if wrong != 0 {
            bad.push(format!("{name}: {wrong} of {} frames", frames.len()));
        }
    }
    assert!(bad.is_empty(), "mismatches: {bad:?}");
}

/// The player's frame source with hardware decoding off: H.264 plays in software, in
/// presentation order, and loops.
#[test]
fn player_falls_back_to_software() {
    use specular_core::Orientation;
    use specular_decode::Frames;
    use specular_video::play::Clip;
    let Some(d) = refs() else { return };
    specular_hwdec::set_enabled(false);
    let clip =
        Clip::from_bytes(std::fs::read(d.join("clips/h264-main-bframes.mp4")).unwrap()).unwrap();
    let size = (clip.track().width, clip.track().height);
    let (mut frames, info) = clip.frames(Orientation(1), size).unwrap();
    let cancel = CancelToken::new();
    let first = frames.next_frame(&cancel).unwrap().unwrap();
    assert_eq!((first.raster.width, first.raster.height), size);
    let mut n = 1;
    while let Some(f) = frames.next_frame(&cancel) {
        assert_eq!(f.unwrap().index, n);
        n += 1;
    }
    assert_eq!(Some(n), info.frame_count);
    frames.rewind().unwrap();
    let again = frames.next_frame(&cancel).unwrap().unwrap();
    assert_eq!(again.index, 0);
    assert_eq!(again.raster.planes[0].data, first.raster.planes[0].data);
}

#[test]
#[ignore]
fn bench() {
    let Some(path) = std::env::var_os("SPECULAR_H264_BENCH") else {
        return;
    };
    let data = std::fs::read(path).unwrap();
    let (frames, first, took) = decode(&data);
    let (w, h) = (frames[0].1.width, frames[0].1.height);
    eprintln!(
        "{w}x{h}: {} frames in {took:?}, first after {first:?}, {:.1} fps",
        frames.len(),
        frames.len() as f64 / took.as_secs_f64()
    );
    // `SPECULAR_H264_BENCH_REF=<raw yuv420p>`: also compare with a reference decode.
    if let Some(r) = std::env::var_os("SPECULAR_H264_BENCH_REF") {
        let r = std::fs::read(r).unwrap();
        let got: Vec<u8> = frames.iter().flat_map(|f| f.1.planes.concat()).collect();
        assert!(got == r, "differs from the reference");
        eprintln!("matches the reference");
    }
}
