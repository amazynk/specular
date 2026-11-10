//! Hardware decode of the reference clips (tools/corpus/video-refs.sh) against libavcodec's
//! decode of the same files: H.264 and HEVC decoding is exact, so every sample of every
//! frame must match. Skips itself without the reference set or without a hardware decoder.

use std::path::{Path, PathBuf};

use specular_hwdec::video::{VideoCodec, VideoDecoder};
use specular_video::mp4;

fn refs() -> Option<PathBuf> {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/out/video-ref");
    if d.join("manifest.tsv").is_file() {
        Some(d)
    } else {
        eprintln!("skipped: run tools/corpus/video-refs.sh");
        None
    }
}

struct Stats {
    frames: usize,
    max_diff: u32,
    wrong_frames: usize,
}

/// Decodes `name` and compares it with its reference; `None` when the hardware can't decode it.
fn check(d: &Path, name: &str) -> Option<Stats> {
    let mut path = d.join("clips").join(format!("{name}.mp4"));
    if !path.is_file() {
        path = d.join("clips").join(format!("{name}.mov"));
    }
    let data = std::fs::read(&path).unwrap();
    let m = mp4::parse(&data, true).unwrap();
    let v = m.video.as_ref().unwrap();
    let codec = match v.codec {
        Some(mp4::Codec::H264) => VideoCodec::H264,
        Some(mp4::Codec::Hevc) => VideoCodec::Hevc,
        c => panic!("{name}: {c:?}"),
    };
    let t0 = std::time::Instant::now();
    let mut dec = match VideoDecoder::new(codec, &v.config) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{name}: skipped ({e})");
            return None;
        }
    };
    let mut frames = Vec::new();
    for (i, s) in v.samples.iter().enumerate() {
        let b = mp4::sample_bytes(&data, s).unwrap();
        if let Err(e) = dec.decode(b, i as i64) {
            panic!("{name}: sample {i}: {e}");
        }
        while let Some((tag, f)) = dec.next_output() {
            if frames.is_empty() {
                eprintln!("{name}: first frame after {:?}", t0.elapsed());
            }
            frames.push((v.samples[tag as usize].cts, f));
        }
    }
    dec.flush();
    while let Some((tag, f)) = dec.next_output() {
        frames.push((v.samples[tag as usize].cts, f));
    }
    let took = t0.elapsed();
    frames.sort_by_key(|f| f.0);
    let (w, h) = (v.width as usize, v.height as usize);
    let bps = if frames[0].1.bit_depth > 8 { 2 } else { 1 };
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let frame_bytes = (w * h + 2 * cw * ch) * bps;
    let r = std::fs::read(d.join("ref").join(format!("{name}.yuv"))).unwrap();
    assert_eq!(r.len() / frame_bytes, frames.len(), "{name}: frame count");
    let mut st = Stats {
        frames: frames.len(),
        max_diff: 0,
        wrong_frames: 0,
    };
    for (k, (_, f)) in frames.iter().enumerate() {
        assert_eq!((f.width as usize, f.height as usize), (w, h), "{name}");
        let want = &r[k * frame_bytes..][..frame_bytes];
        let got: Vec<u8> = f.planes.concat();
        assert_eq!(got.len(), frame_bytes, "{name}: plane sizes");
        let diff = if bps == 2 {
            got.chunks_exact(2)
                .zip(want.chunks_exact(2))
                .map(|(a, b)| {
                    u16::from_le_bytes([a[0], a[1]]).abs_diff(u16::from_le_bytes([b[0], b[1]]))
                        as u32
                })
                .max()
                .unwrap_or(0)
        } else {
            got.iter()
                .zip(want)
                .map(|(a, b)| a.abs_diff(*b) as u32)
                .max()
                .unwrap_or(0)
        };
        if diff != 0 {
            st.wrong_frames += 1;
            if st.wrong_frames == 1 {
                eprintln!("{name}: first mismatch in presentation frame {k} (max diff {diff})");
            }
        }
        st.max_diff = st.max_diff.max(diff);
    }
    eprintln!(
        "{name}: {} frames in {took:?} ({:.2} ms/frame), {} wrong, max diff {}",
        st.frames,
        took.as_secs_f64() * 1e3 / st.frames as f64,
        st.wrong_frames,
        st.max_diff
    );
    Some(st)
}

#[test]
fn hardware_decode_matches_the_reference() {
    let Some(d) = refs() else { return };
    specular_hwdec::set_enabled(true);
    let manifest = std::fs::read_to_string(d.join("manifest.tsv")).unwrap();
    let mut bad = Vec::new();
    for line in manifest.lines() {
        let name = line.split('\t').next().unwrap();
        if let Some(only) = std::env::var_os("SPECULAR_VIDEO_CLIP")
            && only != name
        {
            continue;
        }
        if let Some(st) = check(&d, name)
            && st.wrong_frames != 0
        {
            bad.push(format!(
                "{name}: {} of {} frames",
                st.wrong_frames, st.frames
            ));
        }
    }
    assert!(bad.is_empty(), "mismatches: {bad:?}");
}

/// The player's frame source: presentation order, RGBA in the still's frame, and a rewind
/// that starts over with the same first picture (looping).
#[test]
fn frames_play_in_order_and_loop() {
    use specular_core::{CancelToken, Orientation};
    use specular_decode::Frames;
    use specular_video::play::Clip;
    let Some(d) = refs() else { return };
    specular_hwdec::set_enabled(true);
    for name in ["h264-main-bframes", "hevc-main", "live"] {
        let mut path = d.join("clips").join(format!("{name}.mp4"));
        if !path.is_file() {
            path = d.join("clips").join(format!("{name}.mov"));
        }
        if !path.is_file() {
            continue;
        }
        let t0 = std::time::Instant::now();
        let clip = Clip::from_bytes(std::fs::read(&path).unwrap()).unwrap();
        let (tw, th) = (clip.track().width, clip.track().height);
        let upright = if clip.track().rotation % 180 == 90 {
            (th, tw)
        } else {
            (tw, th)
        };
        // A still twice the clip's size, stored turned (orientation 6): frames come out in the
        // still's stored frame, turned the other way.
        let display = (upright.0 * 2, upright.1 * 2);
        let (mut frames, info) = match clip.frames(Orientation(6), display) {
            Ok(f) => f,
            Err(e) => panic!("{name}: {e}"),
        };
        let cancel = CancelToken::new();
        let first = frames.next_frame(&cancel).unwrap().unwrap();
        eprintln!("{name}: first RGBA frame after {:?}", t0.elapsed());
        assert_eq!(first.index, 0);
        assert_eq!(
            (first.raster.width, first.raster.height),
            (upright.1, upright.0),
            "{name}"
        );
        let mut n = 1;
        let mut total = first.delay;
        while let Some(f) = frames.next_frame(&cancel) {
            let f = f.unwrap();
            assert_eq!(f.index, n, "{name}");
            total += f.delay;
            n += 1;
        }
        assert_eq!(Some(n), info.frame_count, "{name}");
        eprintln!("{name}: {n} frames, {total:?} per loop");
        frames.rewind().unwrap();
        let again = frames.next_frame(&cancel).unwrap().unwrap();
        assert_eq!(again.index, 0);
        assert_eq!(
            again.raster.planes[0].data, first.raster.planes[0].data,
            "{name}"
        );
    }
}

/// Hardware H.264 against the software decoder, picture by picture: the reference set's H.264
/// clips, and every `.mp4` in `SPECULAR_H264_CLIPS` (interlaced conformance streams remuxed
/// with `ffmpeg -c copy`, x264 MBAFF clips). A clip the hardware refuses (High 10, 4:2:2)
/// must still decode in software. Skips itself without a hardware H.264 decoder.
#[test]
fn hardware_h264_matches_software() {
    use specular_core::CancelToken;
    use specular_video::sw_h264::SwH264;
    specular_hwdec::set_enabled(true);
    let mut paths = Vec::new();
    if let Some(d) = refs() {
        let manifest = std::fs::read_to_string(d.join("manifest.tsv")).unwrap();
        for line in manifest.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.get(1) == Some(&"h264") {
                paths.push(d.join("clips").join(format!("{}.mp4", f[0])));
            }
        }
    }
    if let Some(dir) = std::env::var_os("SPECULAR_H264_CLIPS") {
        let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "mp4"))
            .collect();
        v.sort();
        paths.extend(v);
    }
    let mut bad = Vec::new();
    let (mut hw_clips, mut sw_only) = (0, 0);
    for path in &paths {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(path).unwrap();
        let m = mp4::parse(&data, true).unwrap();
        let v = m.video.as_ref().unwrap();
        let cancel = CancelToken::new();
        let mut sw = SwH264::new(&v.config).unwrap();
        let mut want = Vec::new();
        for s in &v.samples {
            let b = mp4::sample_bytes(&data, s).unwrap();
            sw.step(Some((b, s.cts)), &cancel, &mut want).unwrap();
        }
        sw.step(None, &cancel, &mut want).unwrap();
        want.sort_by_key(|f| f.0);
        let mut dec = match VideoDecoder::new(VideoCodec::H264, &v.config) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{name}: software only ({e}), {} frames", want.len());
                assert!(!want.is_empty(), "{name}: no software frames");
                sw_only += 1;
                continue;
            }
        };
        let mut got = Vec::new();
        let mut failed = None;
        for (i, s) in v.samples.iter().enumerate() {
            let b = mp4::sample_bytes(&data, s).unwrap();
            if let Err(e) = dec.decode(b, i as i64) {
                failed = Some(format!("sample {i}: {e}"));
                break;
            }
            while let Some((tag, f)) = dec.next_output() {
                got.push((v.samples[tag as usize].cts, f));
            }
        }
        dec.flush();
        while let Some((tag, f)) = dec.next_output() {
            got.push((v.samples[tag as usize].cts, f));
        }
        if let Some(e) = failed {
            // The player restarts such a clip in software.
            eprintln!("{name}: hardware stopped ({e}); software plays it");
            sw_only += 1;
            continue;
        }
        hw_clips += 1;
        got.sort_by_key(|f| f.0);
        let mut wrong = 0;
        if got.len() != want.len() {
            bad.push(format!(
                "{name}: {} hardware frames, {} software",
                got.len(),
                want.len()
            ));
            continue;
        }
        for (k, ((_, a), (_, b))) in got.iter().zip(&want).enumerate() {
            let same = (a.width, a.height, a.bit_depth) == (b.width, b.height, b.bit_depth)
                && a.planes == b.planes;
            if !same {
                if wrong == 0 {
                    eprintln!("{name}: first mismatch in presentation frame {k}");
                }
                wrong += 1;
            }
        }
        eprintln!("{name}: {} frames, {wrong} wrong", got.len());
        if wrong != 0 {
            bad.push(format!("{name}: {wrong} of {} frames", got.len()));
        }
    }
    eprintln!("{hw_clips} clips compared, {sw_only} software only");
    assert!(bad.is_empty(), "mismatches: {bad:?}");
}
