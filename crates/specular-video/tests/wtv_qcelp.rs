//! WTV, DVR-MS, QCP / 3GPP2 / QuickTime QCELP and EVRC, OGM sound: the corpus of
//! `tools/corpus/fetch-wtv-qcelp.sh` against the system ffmpeg (ignored tests), and small
//! fixtures built here.

#![allow(clippy::needless_range_loop)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_video::mp4::Movie;

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_WTV_CORPUS").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/corpus/wtv1"),
        PathBuf::from,
    )
}

fn files(sub: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(corpus().join(sub))
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn summary(m: &Movie) -> String {
    let mut s = format!("brand {:?}", String::from_utf8_lossy(&m.brand));
    if let Some(v) = &m.video {
        s += &format!(
            " | video {:?} {}x{} {} samples, {} sync, {:?} us",
            v.codec,
            v.width,
            v.height,
            v.samples.len(),
            v.samples.iter().filter(|x| x.sync).count(),
            v.duration_us()
        );
    }
    if let Some(a) = &m.audio {
        s += &format!(
            " | audio {} {} ch {} Hz {} samples",
            a.codec.name(),
            a.channels,
            a.sample_rate,
            a.samples.len()
        );
    }
    s += &format!(
        " | duration {:?} us, {} tags",
        m.duration_us(),
        m.tags.len()
    );
    s
}

#[test]
#[ignore = "needs the corpus (tools/corpus/fetch-wtv-qcelp.sh)"]
fn dump() {
    for sub in [
        "wtv",
        "synth",
        "dvr_ms",
        "qcp",
        "suite-qcp",
        "evrc",
        "qclp",
        "suite-3g2",
        "ogm",
    ] {
        for f in files(sub) {
            let d = std::fs::read(&f).unwrap();
            let r = specular_video::parse_movie(&d, true);
            if std::env::var_os("WTV_HEADS").is_some()
                && let Ok(m) = &r
                && let Some(v) = &m.video
            {
                for s in v.samples.iter().take(12) {
                    let b = v.sample_data(&d, s).unwrap_or_default();
                    eprintln!("  {} {} {:02x?}", s.size, s.cts, &b[..b.len().min(8)]);
                }
            }
            eprintln!(
                "{}: {}",
                f.display(),
                match &r {
                    Ok(m) => summary(m),
                    Err(e) => format!("error: {e}"),
                }
            );
        }
    }
}

/// The whole sound track decoded: samples, channels, rate.
fn decode(path: &Path) -> (Vec<f32>, u16, u32) {
    let src = Arc::new(specular_core::Source::open(path).unwrap());
    let movie = specular_video::parse_source(&src, true, None).unwrap();
    specular_video::audio::decode_track(movie.audio.as_ref().unwrap(), &src).unwrap()
}

/// ffmpeg's decode of the first sound stream as interleaved 16-bit.
fn ffmpeg_sound(path: &Path, channels: u16, rate: u32) -> Option<Vec<i16>> {
    ffmpeg_stream(path, 0, channels, rate)
}

/// ffmpeg's decode of sound stream `n`.
fn ffmpeg_stream(path: &Path, n: usize, channels: u16, rate: u32) -> Option<Vec<i16>> {
    let out = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .arg("-map")
        .arg(format!("0:a:{n}"))
        .args(["-f", "s16le", "-ac"])
        .arg(channels.to_string())
        .arg("-ar")
        .arg(rate.to_string())
        .arg("-")
        .output()
        .ok()?;
    out.status.success().then(|| {
        out.stdout
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect()
    })
}

/// (SNR dB, largest difference, lag in frames of ours against ffmpeg's), the lag searched
/// within `reach` frames on the first second.
fn compare(ours: &[f32], want: &[i16], ch: usize, reach: i64) -> (f64, f64, i64) {
    let a: Vec<f64> = ours.iter().map(|&v| f64::from(v) * 32768.0).collect();
    let b: Vec<f64> = want.iter().map(|&v| f64::from(v)).collect();
    let err_at = |lag: i64, limit: usize| {
        let (mut s, mut e, mut m) = (0.0, 0.0, 0f64);
        for i in 0..a.len().min(limit) {
            let j = i as i64 + lag * ch as i64;
            if j < 0 || j as usize >= b.len() {
                continue;
            }
            let (x, y) = (a[i], b[j as usize]);
            s += y * y;
            e += (x - y) * (x - y);
            m = m.max((x - y).abs());
        }
        (s, e, m)
    };
    let probe = 48_000 * ch;
    let lag = (-reach..=reach)
        .min_by(|&l, &k| {
            let (_, e1, _) = err_at(l, probe);
            let (_, e2, _) = err_at(k, probe);
            e1.total_cmp(&e2)
        })
        .unwrap_or(0);
    let (s, e, m) = err_at(lag, usize::MAX);
    (10.0 * (s / e.max(1e-9)).log10(), m, lag)
}

/// QCELP and EVRC in `.qcp`, 3GPP2 (`sqcp`, `mp4a`) and QuickTime (`Qclp`) against ffmpeg.
#[test]
#[ignore = "needs the corpus (tools/corpus/fetch-wtv-qcelp.sh) and ffmpeg"]
fn speech_matches_ffmpeg() {
    let mut worst = (f64::MAX, f64::MAX);
    let mut n = 0;
    for sub in ["qcp", "suite-qcp", "evrc", "qclp", "suite-3g2"] {
        for f in files(sub) {
            let src = Arc::new(specular_core::Source::open(&f).unwrap());
            let Ok(movie) = specular_video::parse_source(&src, true, None) else {
                eprintln!("{}: not parsed", f.display());
                continue;
            };
            let Some(a) = movie.audio.as_ref() else {
                eprintln!("{}: no sound track", f.display());
                continue;
            };
            let evrc = a.codec.name() == "EVRC";
            let t = std::time::Instant::now();
            let (got, ch, rate) = decode(&f);
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            let want = ffmpeg_sound(&f, ch, rate).unwrap();
            let (snr, maxd, lag) = compare(&got, &want, 1, 2);
            eprintln!(
                "{}: {} {} vs {} samples, SNR {snr:.1} dB, max diff {maxd:.0}, lag {lag}, {ms:.1} ms",
                f.strip_prefix(corpus()).unwrap().display(),
                a.codec.name(),
                got.len(),
                want.len()
            );
            // QuickTime `Qclp` plays its frames at the entry's 22 050 Hz, as ffmpeg does.
            let qt = f.extension().is_some_and(|e| e == "mov");
            assert_eq!((ch, rate), (1, if qt { 22_050 } else { 8000 }));
            let w = if evrc { &mut worst.1 } else { &mut worst.0 };
            *w = w.min(snr);
            n += 1;
        }
    }
    eprintln!(
        "{n} files; worst SNR QCELP {:.1} dB, EVRC {:.1} dB",
        worst.0, worst.1
    );
    assert!(n >= 20);
    assert!(worst.0 > 15.0 && worst.1 > 12.0);
}

/// The sound of WTV and DVR-MS recordings (MPEG audio, AC-3, AAC) against ffmpeg's.
#[test]
#[ignore = "needs the corpus (tools/corpus/fetch-wtv-qcelp.sh) and ffmpeg"]
fn tv_sound_matches_ffmpeg() {
    for sub in ["wtv", "synth", "dvr_ms"] {
        for f in files(sub) {
            let (got, ch, rate) = decode(&f);
            // ffmpeg lists every stream the recording describes, some without data: the
            // one that matches.
            let (snr, maxd, lag, want) = (0..4)
                .filter_map(|n| ffmpeg_stream(&f, n, ch, rate))
                .filter(|w| !w.is_empty())
                .map(|w| {
                    let (s, m, l) = compare(&got, &w, usize::from(ch), 4800);
                    (s, m, l, w)
                })
                .max_by(|a, b| a.0.total_cmp(&b.0))
                .unwrap();
            eprintln!(
                "{}: {} ch {} Hz, {} vs {} samples, SNR {snr:.1} dB, max diff {maxd:.0}, lag {lag}",
                f.strip_prefix(corpus()).unwrap().display(),
                ch,
                rate,
                got.len(),
                want.len()
            );
            // 5.1 sound is mixed down to stereo with other weights than ffmpeg's.
            let src = Arc::new(specular_core::Source::open(&f).unwrap());
            let m = specular_video::parse_source(&src, false, None).unwrap();
            if m.audio.is_some_and(|a| a.channels <= 2) {
                assert!(snr > 30.0, "{}", f.display());
            }
        }
    }
}

fn planes(r: &specular_core::Raster) -> Vec<u8> {
    let mut v = Vec::new();
    for p in &r.planes {
        let bps = p.stride / p.width as usize;
        for row in p.data.chunks(p.stride).take(p.height as usize) {
            v.extend_from_slice(&row[..p.width as usize * bps]);
        }
    }
    v
}

/// The pictures of WTV and DVR-MS recordings against ffmpeg's (the first `FRAMES`, 4:2:0).
#[test]
#[ignore = "needs the corpus (tools/corpus/fetch-wtv-qcelp.sh) and ffmpeg"]
fn tv_video_matches_ffmpeg() {
    use specular_decode::{Format, Frames};
    const FRAMES: usize = 120;
    for (sub, format) in [
        ("wtv", Format::Wtv),
        ("synth", Format::Wtv),
        ("dvr_ms", Format::Asf),
    ] {
        for f in files(sub) {
            let src = Arc::new(specular_core::Source::open(&f).unwrap());
            let s = specular_video::session::open(src, format).unwrap();
            let mut frames = s.video_frames().unwrap();
            frames.set_deinterlace(false);
            let cancel = specular_core::CancelToken::new();
            let mut ours = Vec::new();
            let mut size = (0, 0);
            while ours.len() < FRAMES
                && let Some(fr) = frames.next_frame(&cancel)
            {
                let fr = fr.unwrap();
                size = (fr.raster.width, fr.raster.height);
                ours.push(planes(&fr.raster));
            }
            // The integer IDCT our MPEG-2 decoder shares with ffmpeg's `-idct int`.
            let out = std::process::Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-threads", "1"])
                .args(["-flags", "+bitexact", "-idct", "int", "-i"])
                .arg(&f)
                .args(["-map", "0:v:0", "-fps_mode", "passthrough", "-frames:v"])
                .arg((FRAMES + 16).to_string())
                .args(["-f", "rawvideo", "-pix_fmt", "yuv420p", "-"])
                .output()
                .unwrap();
            let frame = (size.0 * size.1 * 3 / 2) as usize;
            let want: Vec<&[u8]> = out.stdout.chunks_exact(frame.max(1)).collect();
            // ffmpeg may show pictures before the first I picture (concealed): line up on
            // our first.
            let mad = |a: &[u8], b: &[u8]| {
                a.iter()
                    .zip(b)
                    .map(|(&x, &y)| u64::from(x.abs_diff(y)))
                    .sum::<u64>() as f64
                    / a.len().max(1) as f64
            };
            let shift = (0..want.len().min(16))
                .min_by(|&k, &j| {
                    let o = ours.first().map_or(&[][..], |o| o.as_slice());
                    mad(o, want[k]).total_cmp(&mad(o, want[j]))
                })
                .unwrap_or(0);
            if std::env::var_os("WTV_MAD").is_some() {
                for (i, (a, b)) in ours
                    .iter()
                    .zip(&want[shift.min(want.len())..])
                    .take(24)
                    .enumerate()
                {
                    eprintln!("  frame {i}: mean abs diff {:.2}", mad(a, b));
                }
            }
            let same = ours
                .iter()
                .zip(&want[shift.min(want.len())..])
                .filter(|(a, b)| a.as_slice() == **b)
                .count();
            eprintln!(
                "{}: {}x{}, {same} of {} frames bit-exact with ffmpeg (from its frame {shift})",
                f.strip_prefix(corpus()).unwrap().display(),
                size.0,
                size.1,
                ours.len()
            );
            assert!(!ours.is_empty());
            assert!(same * 10 >= ours.len() * 9, "{}", f.display());
        }
    }
}

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn fnv(b: &[u8], mut h: u64) -> u64 {
    for &x in b {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Zero crossings a second of a mono signal: twice its frequency for a sine.
fn crossings(s: &[f32], rate: u32) -> f64 {
    let n = s
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    n as f64 * f64::from(rate) / s.len().max(1) as f64
}

/// A WTV recording written by ffmpeg 8.0.1 (`tests/data/wtv-two-sound.wtv`: MPEG-2 video,
/// MP2 at 440 Hz and 660 Hz): its pictures bit-exact (the hash of ffmpeg's
/// `-idct int -flags +bitexact` decode), both sound tracks listed and playable.
#[test]
fn wtv_fixture() {
    use specular_decode::{Format, Frames};
    let path = data("wtv-two-sound.wtv");
    let d = std::fs::read(&path).unwrap();
    let m = specular_video::parse_movie(&d, true).unwrap();
    assert_eq!(&m.brand, b"WTV ");
    let v = m.video.as_ref().unwrap();
    assert_eq!((v.width, v.height, v.samples.len()), (160, 120, 25));
    let x = specular_video::extras::read(&d);
    assert_eq!(x.audio.len(), 2);
    assert_eq!(x.audio[0].codec, "MPEG audio");
    let src = Arc::new(specular_core::Source::open(&path).unwrap());
    for (track, hz) in x.audio.iter().zip([440.0, 660.0]) {
        let m = specular_video::parse_source(&src, true, Some(track.id)).unwrap();
        let (pcm, ch, rate) =
            specular_video::audio::decode_track(m.audio.as_ref().unwrap(), &src).unwrap();
        assert_eq!((ch, rate), (1, 48_000));
        let f = crossings(&pcm[4800..], rate) / 2.0;
        assert!((f - hz).abs() < 5.0, "track {}: {f} Hz", track.id);
    }
    let s = specular_video::session::open(src, Format::Wtv).unwrap();
    let mut frames = s.video_frames().unwrap();
    frames.set_deinterlace(false);
    let cancel = specular_core::CancelToken::new();
    let (mut h, mut n) = (0xcbf2_9ce4_8422_2325u64, 0);
    while let Some(f) = frames.next_frame(&cancel) {
        h = fnv(&planes(&f.unwrap().raster), h);
        n += 1;
    }
    assert_eq!((n, h), (25, 0x1f2d_ee66_e313_0515));
}

fn qcelp_data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-qcelp/tests/data")
}

/// The committed QCP fixture of `specular-qcelp` through the sound-file reader and player.
#[test]
fn qcp_fixture() {
    let dir = qcelp_data();
    let (pcm, ch, rate) = decode(&dir.join("random.qcp"));
    assert_eq!((ch, rate), (1, 8000));
    let want: Vec<i16> = std::fs::read(dir.join("random.s16"))
        .unwrap()
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    let (snr, _, lag) = compare(&pcm, &want, 1, 0);
    assert_eq!(lag, 0);
    assert!(snr > 26.0, "SNR {snr:.1} dB");
}

/// QCP variants refused in words; damaged files never panic.
#[test]
fn qcp_refusals() {
    let good = std::fs::read(qcelp_data().join("random.qcp")).unwrap();
    let mut smv = good.clone();
    let smv_guid = [
        0x75, 0x2B, 0x7C, 0x8D, 0x97, 0xA7, 0x49, 0xED, 0x98, 0x5E, 0xD5, 0x3C, 0x8C, 0xC7, 0x5F,
        0x84,
    ];
    smv[22..38].copy_from_slice(&smv_guid);
    let e = specular_video::parse_movie(&smv, true)
        .unwrap_err()
        .to_string();
    assert!(e.contains("SMV"), "{e}");
    for n in (0..good.len()).step_by(97) {
        let _ = specular_video::parse_movie(&good[..n], true);
    }
}
