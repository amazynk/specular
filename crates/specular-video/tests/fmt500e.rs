//! The fmt500e movies and modules against ffmpeg (tools/corpus/fetch-fmt500e.sh fetches them,
//! tools/corpus/fmt500e-oracle.py writes ffmpeg's decodes beside them): PlayStation `.str`
//! MDEC pictures plane by plane and XA ADPCM sound sample by sample. Skipped unless
//! `SPECULAR_FMT500E_CORPUS` names the corpus folder.

use std::path::PathBuf;
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{Format, Frames, Registry};
use specular_video::audio;

fn corpus(name: &str) -> Option<PathBuf> {
    let Some(root) = std::env::var_os("SPECULAR_FMT500E_CORPUS") else {
        eprintln!("SPECULAR_FMT500E_CORPUS not set: skipped");
        return None;
    };
    let p = PathBuf::from(root).join(name);
    if !p.is_file() {
        eprintln!("{} missing: skipped", p.display());
        return None;
    }
    Some(p)
}

#[test]
fn psx_str_matches_ffmpeg() {
    let Some(path) = corpus("str/descent-partial.str") else {
        return;
    };
    specular_video::install();
    let src = Arc::new(Source::open(&path).unwrap());
    let s = Registry::new().open(src.clone()).unwrap();
    assert_eq!(s.info().format, Format::PsxStr);
    assert_eq!((s.info().width, s.info().height), (320, 240));

    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let v = movie.video.as_ref().unwrap();
    let fps = f64::from(v.timescale) / f64::from(v.samples[0].duration);
    eprintln!(
        "STR: {fps} fps, {} frames, timescale {}",
        v.samples.len(),
        v.timescale
    );
    assert_eq!(fps, 15.0);
    let want = std::fs::read(path.with_file_name("descent.yuv")).unwrap();
    let (w, h) = (320usize, 240usize);
    let frame = w * h * 3 / 2;
    assert_eq!(v.samples.len(), want.len() / frame, "frame count");
    let (mut differ, mut worst, mut total) = (0usize, 0u8, 0usize);
    let started = std::time::Instant::now();
    for (k, smp) in v.samples.iter().enumerate() {
        let data = v.sample_data(src.bytes(), smp).unwrap();
        let p = specular_oldvideo::mdec::decode_planes(&data, w, h).unwrap();
        let r = &want[k * frame..(k + 1) * frame];
        let (ry, rest) = r.split_at(w * h);
        let (rcb, rcr) = rest.split_at(w * h / 4);
        for (got, want) in [(&p.y, ry), (&p.cb, rcb), (&p.cr, rcr)] {
            for (a, b) in got.iter().zip(want) {
                let d = a.abs_diff(*b);
                total += 1;
                if d > 0 {
                    differ += 1;
                }
                worst = worst.max(d);
            }
        }
    }
    eprintln!(
        "STR pictures: {} frames in {:?}; {differ} of {total} samples differ from ffmpeg, worst by {worst}",
        v.samples.len(),
        started.elapsed()
    );
    // ffmpeg's integer inverse DCT against the exact one: off by one at most, in 2 % of samples at most.
    assert!(worst <= 1, "worst {worst}");
    assert!(differ * 50 < total, "{differ} of {total}");

    // The frames play through the session.
    let vs = specular_video::session::open(src.clone(), Format::PsxStr).unwrap();
    let mut it = vs.video_frames().unwrap();
    let f = it.next_frame(&CancelToken::new()).unwrap().unwrap();
    assert_eq!((f.raster.width, f.raster.height), (320, 240));

    let a = movie.audio.as_ref().unwrap();
    assert_eq!((a.channels, a.sample_rate), (2, 37_800));
    let (pcm, ch, rate) = audio::decode_track(a, &src).unwrap();
    assert_eq!((ch, rate), (2, 37_800));
    let want: Vec<i16> = std::fs::read(path.with_file_name("descent.s16"))
        .unwrap()
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    assert_eq!(pcm.len(), want.len(), "sound samples");
    let got: Vec<i16> = pcm.iter().map(|&x| (x * 32768.0).round() as i16).collect();
    let differ = got.iter().zip(&want).filter(|(a, b)| a != b).count();
    eprintln!(
        "XA sound: {differ} of {} samples differ from ffmpeg",
        want.len()
    );
    assert_eq!(differ, 0);
}

/// libopenmpt's envelope of a module: frames, then (loudness, zero crossings) every 20 ms.
fn envelope(path: &std::path::Path) -> (usize, Vec<(f64, f64)>) {
    let mut p = path.as_os_str().to_owned();
    p.push(".env");
    let t = std::fs::read_to_string(p).unwrap();
    let mut lines = t.lines();
    let frames = lines.next().unwrap().parse().unwrap();
    let env = lines
        .map(|l| {
            let mut it = l.split(' ').map(|v| v.parse::<f64>().unwrap());
            (it.next().unwrap(), it.next().unwrap())
        })
        .collect();
    (frames, env)
}

/// The older tracker modules against libopenmpt's render: the song's length, and its
/// loudness and pitch (zero crossings) 20 ms at a time. Two players never mix alike, so the
/// level is taken relative to the median ratio.
#[test]
fn modules_follow_libopenmpt() {
    // (module, its kind, the least share of windows whose level and whose pitch agree, %):
    // regression gates under what was measured (zero crossings of a whole mix are a rough
    // pitch; real songs drift apart where the two players' effects differ).
    const MODULES: &[(&str, &str, usize, usize)] = &[
        ("fonetag.669", "Composer 669", 25, 8),
        ("bliss.mtm", "MultiTracker", 85, 35),
        ("galaxy.mtm", "MultiTracker", 85, 30),
        ("westend.stm", "Scream Tracker 2", 48, 11),
        ("sunny.stm", "Scream Tracker 2", 65, 22),
        ("blackbox.okt", "Oktalyzer", 85, 45),
        ("piano.okt", "Oktalyzer", 85, 35),
        ("mb16.med", "OctaMED", 45, 15),
        ("fairy.med", "OctaMED", 85, 38),
        ("hometown.med", "OctaMED", 85, 40),
        ("mandzuria.med", "OctaMED", 55, 18),
        ("buzzare.ult", "UltraTracker", 85, 38),
        ("backinbusiness.ult", "UltraTracker", 85, 45),
        ("communion.far", "Farandole", 30, 12),
        ("pond.far", "Farandole", 50, 15),
    ];
    let only = std::env::var("FMT500E_ONLY").ok();
    let mut failed = Vec::new();
    for &(name, kind, level_gate, pitch_gate) in MODULES {
        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }
        let Some(path) = corpus(&format!("trackers/{name}")) else {
            return;
        };
        specular_video::install();
        let src = Arc::new(Source::open(&path).unwrap());
        let s = Registry::new().open(src.clone()).unwrap();
        assert_eq!(s.info().format, Format::Tracker, "{name}");
        let codec = s.video().and_then(|v| v.audio.clone()).unwrap_or_default();
        assert!(codec.contains(kind), "{name}: {codec}");
        let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
        let t = movie.audio.as_ref().unwrap();
        let started = std::time::Instant::now();
        let (pcm, ch, rate) = audio::decode_track(t, &src).unwrap();
        let took = started.elapsed();
        assert_eq!((ch, rate), (2, 48_000));
        let mono: Vec<f64> = pcm
            .chunks_exact(2)
            .map(|f| f64::from(f[0] + f[1]) / 2.0 * 32768.0)
            .collect();
        let (frames, want) = envelope(&path);
        let ours: Vec<(f64, f64)> = mono
            .chunks_exact(960)
            .map(|w| {
                let rms = (w.iter().map(|x| x * x).sum::<f64>() / 960.0).sqrt();
                let zc = w
                    .windows(2)
                    .filter(|p| (p[0] < 0.0) != (p[1] < 0.0))
                    .count();
                (rms, zc as f64)
            })
            .collect();
        let n = ours.len().min(want.len());
        let mut ratios: Vec<f64> = (0..n)
            .filter(|&i| ours[i].0 > 300.0 && want[i].0 > 300.0)
            .map(|i| ours[i].0 / want[i].0)
            .collect();
        ratios.sort_by(f64::total_cmp);
        let gain = ratios.get(ratios.len() / 2).copied().unwrap_or(1.0);
        let level = ratios
            .iter()
            .filter(|r| (0.75..1.33).contains(&(*r / gain)))
            .count();
        let (mut close, mut loud) = (0, 0);
        for i in 0..n {
            if ours[i].0 > 300.0 && want[i].0 > 300.0 {
                loud += 1;
                let (a, b) = (ours[i].1, want[i].1);
                if (a - b).abs() <= 0.15 * b.max(4.0) {
                    close += 1;
                }
            }
        }
        if std::env::var_os("FMT500E_ENV").is_some() {
            for i in (0..n).step_by(25) {
                eprintln!(
                    "{name} {i}: {:.0} {:.0} | {} {}",
                    ours[i].0, want[i].0, ours[i].1, want[i].1
                );
            }
        }
        let length = mono.len() as f64 / frames as f64;
        let level_pc = level * 100 / ratios.len().max(1);
        let pitch_pc = close * 100 / loud.max(1);
        eprintln!(
            "{name}: rendered in {took:?}; length {length:.3} of libopenmpt's; level within 25% in {level} of {} windows ({level_pc}%, gain {gain:.2}); pitch within 15% in {close} of {loud} ({pitch_pc}%)",
            ratios.len()
        );
        if !(0.95..1.05).contains(&length)
            || level_pc < level_gate
            || pitch_pc < pitch_gate
            || loud * 2 < n
        {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}
