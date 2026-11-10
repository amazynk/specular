//! Software HEVC decode (specular-hevc's inter path) of the reference clips
//! (tools/corpus/video-refs.sh) against libavcodec's decode of the same files: HEVC decoding
//! is exact, so every sample of every frame must match. Skips itself without the reference
//! set. `SPECULAR_SWDEC_ONLY=name,name` limits the clips; `SPECULAR_SWDEC_DIR` (clips/, ref/)
//! points at another set and `SPECULAR_SWDEC_DUMP=<dir>` writes the decoded YUV there.

use std::path::{Path, PathBuf};

use specular_heic::hevc;
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

/// Decodes every picture of `data` (an MP4/MOV with an HEVC track) in presentation order.
fn decode_all(data: &[u8]) -> (Vec<hevc::Frame>, std::time::Duration) {
    let m = mp4::parse(data, true).unwrap();
    let v = m.video.as_ref().unwrap();
    assert_eq!(v.codec, Some(mp4::Codec::Hevc));
    let cfg = hevc::hvcc::parse(&v.config).unwrap();
    // `SPECULAR_SWDEC_THREADS` overrides the worker count (for frame-rate measurements).
    let threads = std::env::var("SPECULAR_SWDEC_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(4)));
    let mut dec = hevc::VideoDecoder::new(&cfg.nal_units, threads).unwrap();
    // Samples in decoding order; output comes back in output order, which is cts order.
    let t0 = std::time::Instant::now();
    let mut out = Vec::new();
    for (i, s) in v.samples.iter().enumerate() {
        let b = mp4::sample_bytes(data, s).unwrap();
        let nals = hevc::hvcc::split(b, cfg.length_size).unwrap();
        if let Err(e) = dec.decode(&nals, None) {
            panic!("sample {i}: {e}");
        }
        while let Some(d) = dec.next_output() {
            if d.damaged > 0 {
                eprintln!(
                    "output {}: {} of {} CTUs concealed",
                    out.len(),
                    d.damaged,
                    d.total
                );
            }
            out.push(d.frame);
        }
    }
    dec.flush();
    while let Some(d) = dec.next_output() {
        out.push(d.frame);
    }
    (out, t0.elapsed())
}

struct Stats {
    frames: usize,
    wrong: Vec<usize>,
    max_diff: u32,
}

fn check(d: &Path, name: &str) -> Stats {
    let mut path = d.join("clips").join(format!("{name}.mp4"));
    if !path.is_file() {
        path = d.join("clips").join(format!("{name}.mov"));
    }
    let data = std::fs::read(&path).unwrap();
    let (frames, took) = decode_all(&data);
    let f0 = &frames[0];
    let (w, h) = (f0.width as usize, f0.height as usize);
    let bps = if f0.bit_depth > 8 { 2 } else { 1 };
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let frame_bytes = (w * h + 2 * cw * ch) * bps;
    let r = std::fs::read(d.join("ref").join(format!("{name}.yuv"))).unwrap();
    eprintln!(
        "{name}: {} frames {w}x{h} in {took:?} ({:.1} fps)",
        frames.len(),
        frames.len() as f64 / took.as_secs_f64()
    );
    assert_eq!(r.len() / frame_bytes, frames.len(), "{name}: frame count");
    let mut st = Stats {
        frames: frames.len(),
        wrong: Vec::new(),
        max_diff: 0,
    };
    for (k, f) in frames.iter().enumerate() {
        let want = &r[k * frame_bytes..][..frame_bytes];
        let mut got = Vec::with_capacity(frame_bytes);
        for p in [&f.y, &f.cb, &f.cr] {
            for &s in p.iter() {
                if bps == 2 {
                    got.extend_from_slice(&s.to_le_bytes());
                } else {
                    got.push(s as u8);
                }
            }
        }
        assert_eq!(got.len(), frame_bytes, "{name}: plane sizes");
        if let Some(dump) = std::env::var_os("SPECULAR_SWDEC_DUMP") {
            use std::io::Write;
            let p = Path::new(&dump).join(format!("{name}.yuv"));
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(k > 0)
                .write(true)
                .truncate(k == 0)
                .open(p)
                .unwrap();
            f.write_all(&got).unwrap();
        }
        let mut diff = 0u32;
        let mut first = None;
        for i in 0..frame_bytes / bps {
            let (a, b) = if bps == 2 {
                (
                    u16::from_le_bytes([got[2 * i], got[2 * i + 1]]),
                    u16::from_le_bytes([want[2 * i], want[2 * i + 1]]),
                )
            } else {
                (u16::from(got[i]), u16::from(want[i]))
            };
            let dd = u32::from(a.abs_diff(b));
            if dd > 0 && first.is_none() {
                first = Some(i);
            }
            diff = diff.max(dd);
        }
        if diff > 0 {
            if st.wrong.is_empty() {
                let i = first.unwrap();
                let (plane, x, y) = if i < w * h {
                    ("Y", i % w, i / w)
                } else if i < w * h + cw * ch {
                    ("Cb", (i - w * h) % cw, (i - w * h) / cw)
                } else {
                    ("Cr", (i - w * h - cw * ch) % cw, (i - w * h - cw * ch) / cw)
                };
                eprintln!("{name}: frame {k} first mismatch {plane} ({x},{y}), max diff {diff}");
            }
            st.wrong.push(k);
        }
        st.max_diff = st.max_diff.max(diff);
    }
    st
}

#[test]
fn hevc_software_bit_exact() {
    // `SPECULAR_SWDEC_DIR` points at another set laid out the same way (clips/, ref/), whose
    // clips `SPECULAR_SWDEC_ONLY` names.
    let extra = std::env::var_os("SPECULAR_SWDEC_DIR").map(PathBuf::from);
    let Some(d) = extra.clone().or_else(refs) else {
        return;
    };
    let only = std::env::var("SPECULAR_SWDEC_ONLY").ok();
    let mut names = vec![
        "hevc-main",
        "hevc-main-odd",
        "hevc-weighted",
        "hevc-main10",
        "hevc-tools",
        "hevc-slices",
        "hevc-main10-pyramid",
        "live",
        "hevc-1080p",
        "hevc-4k",
    ];
    if extra.is_some() {
        names = only.as_deref().unwrap_or("").split(',').collect();
    }
    let mut bad = Vec::new();
    for name in names {
        if only
            .as_deref()
            .is_some_and(|o| !o.split(',').any(|n| n == name))
        {
            continue;
        }
        if !d.join("ref").join(format!("{name}.yuv")).is_file() {
            eprintln!("{name}: skipped (no reference)");
            continue;
        }
        let st = check(&d, name);
        if st.wrong.is_empty() {
            eprintln!("{name}: {} frames bit-exact", st.frames);
        } else {
            eprintln!(
                "{name}: {} of {} frames differ (max {}): {:?}",
                st.wrong.len(),
                st.frames,
                st.max_diff,
                &st.wrong[..st.wrong.len().min(12)]
            );
            bad.push(name);
        }
    }
    assert!(bad.is_empty(), "not bit-exact: {bad:?}");
}

/// With hardware decoding off, HEVC and H.264 clips play through the software decoders
/// (presentation order, every frame, prompt cancellation, a rewind that starts over
/// identically).
#[test]
fn software_playback_without_hardware() {
    use specular_core::{CancelToken, Orientation};
    use specular_decode::{DecodeError, Frames};
    use specular_video::play::Clip;
    let Some(d) = refs() else { return };
    specular_hwdec::set_enabled(false);
    for name in [
        "hevc-main",
        "hevc-main10-pyramid",
        "live",
        "h264-main-bframes",
    ] {
        let mut path = d.join("clips").join(format!("{name}.mp4"));
        if !path.is_file() {
            path = d.join("clips").join(format!("{name}.mov"));
        }
        if !path.is_file() {
            continue;
        }
        let clip = Clip::from_bytes(std::fs::read(&path).unwrap()).unwrap();
        let (tw, th) = (clip.track().width, clip.track().height);
        let opened = clip.frames(Orientation(1), (tw, th));
        let (mut frames, info) = opened.unwrap_or_else(|e| panic!("{name}: {e}"));
        let cancel = CancelToken::new();
        let t0 = std::time::Instant::now();
        let first = frames.next_frame(&cancel).unwrap().unwrap();
        eprintln!("{name}: first software frame after {:?}", t0.elapsed());
        assert_eq!(first.index, 0);
        let mut n = 1;
        while let Some(f) = frames.next_frame(&cancel) {
            assert_eq!(f.unwrap().index, n, "{name}");
            n += 1;
        }
        assert_eq!(Some(n), info.frame_count, "{name}");
        frames.rewind().unwrap();
        let cancelled = CancelToken::new();
        cancelled.cancel();
        assert!(
            matches!(
                frames.next_frame(&cancelled),
                Some(Err(DecodeError::Cancelled))
            ),
            "{name}"
        );
        frames.rewind().unwrap();
        let again = frames.next_frame(&cancel).unwrap().unwrap();
        assert_eq!(again.index, 0);
        assert_eq!(
            again.raster.planes[0].data, first.raster.planes[0].data,
            "{name}"
        );
    }
}
