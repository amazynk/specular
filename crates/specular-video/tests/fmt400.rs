//! The fmt400 sound and video files (tests/data/fmt400*, made by gen_fixtures_fmt400.sh):
//! AU, CAF, Wave64, RF64, BW64, VOC, 8SVX, AVR, NIST SPHERE, IRCAM, TTA, Shorten, DSF,
//! DSDIFF, tracker modules (MOD, S3M, XM, IT), YUV4MPEG2 and CD+G. Recognised by their
//! content, decoded against ffmpeg's decode (sample for sample for PCM and the lossless
//! coders), modules against libopenmpt's render (loudness and pitch envelopes: two players
//! never mix alike), YUV4MPEG2 frames and CD+G pictures exactly.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source, YuvMatrix};
use specular_decode::{Format, Frames, Registry, Session};
use specular_video::audio;

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn open(name: &str) -> Arc<dyn Session> {
    specular_video::install();
    let src = Arc::new(Source::open(data(name)).unwrap());
    Registry::new().open(src).unwrap()
}

const SOUND: &[(&str, Format, &str)] = &[
    ("fmt400-au.au", Format::Au, "PCM"),
    ("fmt400-ulaw.snd", Format::Au, ""),
    ("fmt400-caf.caf", Format::Caf, ""),
    ("fmt400-f32.caf", Format::Caf, ""),
    ("fmt400-alac.caf", Format::Caf, "ALAC"),
    ("fmt400-w64.w64", Format::W64, ""),
    ("fmt400-rf64.rf64", Format::Wave, ""),
    ("fmt400-bw64.bw64", Format::Wave, ""),
    ("fmt400-voc.voc", Format::Voc, ""),
    ("fmt400-voc8.voc", Format::Voc, ""),
    ("fmt400-8svx.8svx", Format::Svx, "8SVX"),
    ("fmt400-fib.8svx", Format::Svx, "Fibonacci"),
    ("fmt400-avr.avr", Format::Avr, ""),
    ("fmt400-sph.sph", Format::Sphere, ""),
    ("fmt400-ircam.sf", Format::Ircam, ""),
    ("fmt400-tta.tta", Format::Tta, "TTA"),
    ("fmt400-tta24.tta", Format::Tta, "TTA"),
    ("fmt400-shn.shn", Format::Shorten, "Shorten"),
    ("fmt400-shn.sph", Format::Sphere, "Shorten"),
    ("fmt400-dsd.dsf", Format::Dsf, "DSD64"),
    ("fmt400-dsd.dff", Format::Dff, "DSD64"),
    ("fmt400.mod", Format::Tracker, "ProTracker"),
    ("fmt400.s3m", Format::Tracker, "Scream Tracker"),
    ("fmt400.xm", Format::Tracker, "FastTracker"),
    ("fmt400.it", Format::Tracker, "Impulse Tracker"),
];

fn decoded(name: &str) -> (Vec<f32>, usize, u32) {
    specular_video::install();
    let src = Arc::new(Source::open(data(name)).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let t = movie.audio.as_ref().unwrap();
    let (pcm, ch, rate) = audio::decode_track(t, &src).unwrap();
    (pcm, usize::from(ch), rate)
}

#[test]
fn recognised_and_playable() {
    for &(name, format, codec) in SOUND {
        let s = open(name);
        assert_eq!(s.info().format, format, "{name}");
        let v = s.video().expect("sound facts");
        assert!(v.sound.is_some(), "{name}");
        assert!(v.silent_reason.is_none(), "{name}: {:?}", v.silent_reason);
        let audio = v.audio.clone().unwrap_or_default();
        assert!(
            audio.contains(codec),
            "{name}: codec {audio:?} lacks {codec:?}"
        );
        let d = v.duration_us.unwrap_or(0);
        assert!((90_000..20_000_000).contains(&d), "{name}: {d} µs");
    }
}

/// (frames compared, frames in the oracle, largest difference in 16-bit steps).
fn against_ffmpeg(name: &str) -> (usize, usize, i32) {
    let (got, ch, _) = decoded(name);
    let want: Vec<i32> = std::fs::read(data(&format!("{}.pcm", name.rsplit_once('.').unwrap().0)))
        .unwrap()
        .chunks_exact(2)
        .map(|c| i32::from(i16::from_le_bytes([c[0], c[1]])))
        .collect();
    let n = got.len().min(want.len());
    let max = (0..n)
        .map(|i| {
            let g = (f64::from(got[i]) * 32768.0)
                .round()
                .clamp(-32768.0, 32767.0) as i32;
            (g - want[i]).abs()
        })
        .max()
        .unwrap_or(i32::MAX);
    (n / ch, want.len() / ch, max)
}

#[test]
fn pcm_and_lossless_match_ffmpeg() {
    for (name, tolerance) in [
        ("fmt400-au.au", 0),
        ("fmt400-ulaw.snd", 0),
        ("fmt400-caf.caf", 0),
        // Float and 24-bit to 16-bit: ffmpeg truncates, the test rounds.
        ("fmt400-f32.caf", 1),
        ("fmt400-alac.caf", 0),
        ("fmt400-w64.w64", 0),
        ("fmt400-rf64.rf64", 0),
        ("fmt400-bw64.bw64", 0),
        ("fmt400-voc.voc", 0),
        ("fmt400-voc8.voc", 0),
        ("fmt400-8svx.8svx", 0),
        ("fmt400-fib.8svx", 0),
        ("fmt400-avr.avr", 0),
        ("fmt400-sph.sph", 0),
        ("fmt400-ircam.sf", 0),
        ("fmt400-tta.tta", 0),
        ("fmt400-tta24.tta", 1),
        ("fmt400-shn.shn", 0),
        ("fmt400-shn.sph", 0),
    ] {
        let (n, total, max) = against_ffmpeg(name);
        eprintln!("{name}: {n} of {total} frames, max diff {max}");
        assert!(
            n * 100 >= total * 99 && n > 0,
            "{name}: {n} of {total} frames"
        );
        assert!(max <= tolerance, "{name}: differs by {max}");
    }
}

#[test]
fn dsd_in_both_containers_plays_the_same() {
    let (a, ch, rate) = decoded("fmt400-dsd.dsf");
    let (b, ch2, rate2) = decoded("fmt400-dsd.dff");
    assert_eq!((ch, rate), (2, 88_200));
    assert_eq!((ch2, rate2), (2, 88_200));
    assert_eq!(a.len(), b.len());
    assert!(a == b, "DSF and DSDIFF of the same bits decode alike");
    // The left channel is a 1 kHz tone, the right 3 kHz (after the filter settles).
    let zc = |c: usize| {
        let x: Vec<f32> = a.chunks_exact(2).map(|f| f[c]).skip(2000).collect();
        let n = x
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        n as f64 / 2.0 / (x.len() as f64 / 88_200.0)
    };
    let (l, r) = (zc(0), zc(1));
    eprintln!("DSD tones: {l:.0} Hz, {r:.0} Hz");
    assert!((l - 1000.0).abs() < 30.0 && (r - 3000.0).abs() < 60.0);
    let peak = a.iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!((0.3..0.6).contains(&peak), "{peak}");
}

/// libopenmpt's envelope: frames, then (loudness, zero crossings) every 20 ms.
fn envelope_oracle(m: &str) -> (usize, Vec<(f64, f64)>) {
    let t = std::fs::read_to_string(data(&format!("fmt400-{m}.env"))).unwrap();
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

#[test]
fn modules_follow_libopenmpt() {
    for m in ["mod", "s3m", "xm", "it"] {
        let (pcm, ch, rate) = decoded(&format!("fmt400.{m}"));
        assert_eq!((ch, rate), (2, 48_000));
        let mono: Vec<f64> = pcm
            .chunks_exact(2)
            .map(|f| f64::from(f[0] + f[1]) / 2.0 * 32768.0)
            .collect();
        let (frames, want) = envelope_oracle(m);
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
        // Loudness: the players' gains differ, so the level is taken relative to the median
        // ratio; the windows within 25 % of it count.
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
        // Pitch: zero crossings in the windows both play loudly.
        let mut close = 0;
        let mut loud = 0;
        for i in 0..n {
            if ours[i].0 > 300.0 && want[i].0 > 300.0 {
                loud += 1;
                let (a, b) = (ours[i].1, want[i].1);
                if (a - b).abs() <= 0.15 * b.max(4.0) {
                    close += 1;
                }
            }
        }
        if std::env::var_os("FMT400_ENV").is_some() {
            for i in (0..n).step_by(10) {
                eprintln!(
                    "{m} {i}: {:.0} {:.0} | {} {}",
                    ours[i].0, want[i].0, ours[i].1, want[i].1
                );
            }
        }
        let length = mono.len() as f64 / frames as f64;
        eprintln!(
            "{m}: length {:.3} of libopenmpt's, level within 25% in {level} of {} windows (gain {gain:.2}), pitch within 15% in {close} of {loud} windows",
            length,
            ratios.len()
        );
        assert!((0.97..1.03).contains(&length), "{m}: length ratio {length}");
        assert!(
            level * 100 >= ratios.len() * 85,
            "{m}: level in {level} of {}",
            ratios.len()
        );
        assert!(close * 100 >= loud * 75, "{m}: pitch in {close} of {loud}");
    }
}

#[test]
fn a_camcorder_mod_is_not_a_module() {
    // An MPEG programme stream named .mod: the pack header leaves it to the video path.
    let mut d = vec![0u8; 4096];
    d[..4].copy_from_slice(&[0, 0, 1, 0xBA]);
    d[4] = 0x44;
    d[1080..1084].copy_from_slice(b"M.K.");
    d[950] = 1;
    assert!(specular_tracker::sniff(&d).is_none());
    assert!(specular_video::sound_file::sniff(&d).is_none());
}

fn planes(r: &Raster) -> Vec<u8> {
    assert!(matches!(r.layout, PixelLayout::Yuv(_)), "{:?}", r.layout);
    let mut v = Vec::new();
    for p in &r.planes {
        let w = p.width as usize * p.bytes_per_pixel();
        for row in p.data.chunks(p.stride).take(p.height as usize) {
            v.extend_from_slice(&row[..w]);
        }
    }
    v
}

#[test]
fn y4m_frames_are_exact() {
    for (name, frames) in [
        ("fmt400-420.y4m", 6),
        ("fmt400-422p10.y4m", 6),
        ("fmt400-444.y4m", 6),
    ] {
        let s = open(name);
        assert_eq!(s.info().format, Format::Y4m, "{name}");
        let want = std::fs::read(data(&name.replace(".y4m", ".yuv"))).unwrap();
        let src = Arc::new(Source::open(data(name)).unwrap());
        let vs = specular_video::session::open(src, Format::Y4m).unwrap();
        let mut it = vs.video_frames().unwrap();
        it.set_deinterlace(false);
        let cancel = CancelToken::new();
        let (mut n, mut at, mut bad) = (0, 0, 0);
        while let Some(f) = it.next_frame(&cancel) {
            let got = planes(&f.unwrap().raster);
            if at + got.len() > want.len() || got != want[at..at + got.len()] {
                bad += 1;
            }
            at += got.len();
            n += 1;
        }
        eprintln!("{name}: {n} frames, {bad} differ");
        assert_eq!((n, bad, at), (frames, 0, want.len()), "{name}");
    }
}

/// A raster's RGB bytes (CD+G frames are G, B, R planes under the identity matrix).
fn rgb(r: &Raster) -> Vec<u8> {
    match &r.layout {
        PixelLayout::Yuv(e) if e.matrix == YuvMatrix::Identity => {
            let (g, b, red) = (&r.planes[0], &r.planes[1], &r.planes[2]);
            let mut out = Vec::new();
            for y in 0..g.height as usize {
                for x in 0..g.width as usize {
                    out.extend_from_slice(&[
                        red.data[y * red.stride + x],
                        g.data[y * g.stride + x],
                        b.data[y * b.stride + x],
                    ]);
                }
            }
            out
        }
        PixelLayout::Rgba => r.planes[0]
            .data
            .chunks(r.planes[0].stride)
            .take(r.height as usize)
            .flat_map(|row| {
                row[..4 * r.width as usize]
                    .chunks_exact(4)
                    .flat_map(|p| [p[0], p[1], p[2]])
                    .collect::<Vec<_>>()
            })
            .collect(),
        l => panic!("CD+G frame as {l:?}"),
    }
}

#[test]
fn cdg_pictures_match_ffmpeg() {
    let s = open("fmt400.cdg");
    assert_eq!(s.info().format, Format::Cdg);
    assert_eq!((s.info().width, s.info().height), (300, 216));
    let refs = std::fs::read(data("fmt400-cdg.ref")).unwrap();
    let mut want = std::collections::HashMap::new();
    let mut at = 0;
    while at + 8 <= refs.len() {
        let j = u32::from_le_bytes(refs[at..at + 4].try_into().unwrap()) as usize;
        let len = u32::from_le_bytes(refs[at + 4..at + 8].try_into().unwrap()) as usize;
        let mut px = Vec::new();
        for run in refs[at + 8..at + 8 + len].chunks_exact(5) {
            let n = usize::from(u16::from_le_bytes([run[0], run[1]]));
            for _ in 0..n {
                px.extend_from_slice(&run[2..5]);
            }
        }
        want.insert(j, px);
        at += 8 + len;
    }
    let src = Arc::new(Source::open(data("fmt400.cdg")).unwrap());
    let vs = specular_video::session::open(src, Format::Cdg).unwrap();
    let mut it = vs.video_frames().unwrap();
    let cancel = CancelToken::new();
    let mut k = 0;
    let mut checked = 0;
    while let Some(f) = it.next_frame(&cancel) {
        let f = f.unwrap();
        if let Some(w) = want.get(&k) {
            let got = rgb(&f.raster);
            let differ = got.iter().zip(w).filter(|(a, b)| a != b).count();
            eprintln!("CD+G frame {k}: {differ} bytes differ");
            if let Some(i) = (0..got.len()).find(|&i| got[i] != w[i]) {
                let p = i / 3;
                eprintln!(
                    "first at x {} y {}: {:?} want {:?}",
                    p % 300,
                    p / 300,
                    &got[3 * p..3 * p + 3],
                    &w[3 * p..3 * p + 3]
                );
            }
            assert_eq!(differ, 0, "frame {k}");
            checked += 1;
        }
        k += 1;
    }
    assert_eq!(checked, 3);
}

#[test]
fn cdg_plays_the_song_beside_it() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("fmt400-cdg");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(data("fmt400.cdg"), dir.join("song.cdg")).unwrap();
    std::fs::copy(data("fmt400.wav"), dir.join("song.wav")).unwrap();
    specular_video::install();
    let src = Arc::new(Source::open(dir.join("song.cdg")).unwrap());
    let s = Registry::new().open(src).unwrap();
    let v = s.video().unwrap();
    assert!(v.audio.is_some(), "the song beside it");
    assert!(v.silent_reason.is_none(), "{:?}", v.silent_reason);
}

#[test]
fn module_title_is_its_tag() {
    for (m, kind) in [
        ("mod", "ProTracker"),
        ("s3m", "Scream Tracker"),
        ("xm", "FastTracker"),
        ("it", "Impulse Tracker"),
    ] {
        let d = std::fs::read(data(&format!("fmt400.{m}"))).unwrap();
        let t = specular_video::sound_tags::read(&d);
        let get = |k: &str| t.fields.iter().find(|f| f.0 == k).map(|f| f.1.clone());
        let title = format!("fmt400 {m} test");
        assert_eq!(get("title").as_deref(), Some(title.as_str()));
        assert!(get("encoder").unwrap().starts_with(kind), "{m}");
    }
}
