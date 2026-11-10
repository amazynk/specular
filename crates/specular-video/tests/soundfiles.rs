//! Sound files without a picture (tests/data/snd-*, made by gen_fixtures_soundfiles.sh):
//! recognised by the registry, demuxed, decoded against ffmpeg's decode (`.pcm`), their
//! tags against mutagen's reading (`.tags`), drawn as cover, waveform and spectrogram, with
//! a playhead that plays.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Format, Registry, Session};
use specular_video::audio::{self, FakeOutput};

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

/// Every fixture with the format it is recognised as.
const FILES: &[(&str, Format)] = &[
    ("snd-mp3.mp3", Format::Mp3),
    ("snd-vbr.mp3", Format::Mp3),
    ("snd-mp2.mp2", Format::Mp3),
    ("snd-aac.aac", Format::Aac),
    ("snd-flac.flac", Format::Flac),
    ("snd-flac24.flac", Format::Flac),
    ("snd-wav.wav", Format::Wave),
    ("snd-f32.wav", Format::Wave),
    ("snd-51.wav", Format::Wave),
    ("snd-ulaw.wav", Format::Wave),
    ("snd-ima.wav", Format::Wave),
    ("snd-aiff.aiff", Format::Aiff),
    ("snd-sowt.aifc", Format::Aiff),
    ("snd-ac3.ac3", Format::Ac3),
    ("snd-eac3.eac3", Format::Ac3),
    ("snd-dts.dts", Format::Dts),
    ("snd-m4a.m4a", Format::M4a),
    ("snd-alac.m4a", Format::M4a),
    ("snd-vorbis.ogg", Format::OggAudio),
    ("snd-opus.opus", Format::OggAudio),
    ("snd-mka.mka", Format::Matroska),
];

fn oracle(name: &str) -> Vec<i16> {
    std::fs::read(data(&format!("{}.pcm", name.rsplit_once('.').unwrap().0)))
        .unwrap()
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

/// The decoded sound against ffmpeg's: the offset (in frames) where they line up best,
/// frames compared, the largest difference in 16-bit steps and the signal-to-error ratio.
fn against(name: &str) -> (i64, usize, i32, f64) {
    let (off, n, max, snr, _) = against_with(name, false);
    (off, n, max, snr)
}

/// [`against`], decoded for stereo or (`surround`) for surround speakers; and the channels.
fn against_with(name: &str, surround: bool) -> (i64, usize, i32, f64, usize) {
    let s = open(name);
    let src = Arc::new(Source::open(data(name)).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let t = movie.audio.as_ref().unwrap();
    let (pcm, ch, _rate) = if surround {
        audio::decode_track_surround(t, &src).unwrap()
    } else {
        audio::decode_track(t, &src).unwrap()
    };
    let ch = usize::from(ch);
    let got: Vec<i32> = pcm
        .iter()
        .map(|&v| (f64::from(v) * 32768.0).round().clamp(-32768.0, 32767.0) as i32)
        .collect();
    let want = oracle(name);
    assert!(s.video().is_some());
    // The edit's start (MP3's LAME delay) is where the oracle starts.
    let edit = t.edit.start.max(0) as usize;
    let score = |off: i64| -> (usize, i32, f64) {
        let (mut n, mut max, mut sig, mut err) = (0usize, 0i32, 0f64, 0f64);
        for (i, &w) in want.iter().enumerate() {
            let j = i as i64 + off * ch as i64;
            let Some(&g) = (j >= 0).then(|| got.get(j as usize)).flatten() else {
                continue;
            };
            let d = (g - i32::from(w)).abs();
            max = max.max(d);
            sig += f64::from(w) * f64::from(w);
            err += f64::from(d) * f64::from(d);
            n += 1;
        }
        let snr = if err == 0.0 {
            f64::INFINITY
        } else {
            10.0 * (sig / err).log10()
        };
        (n / ch, max, snr)
    };
    let mut best = (edit as i64, score(edit as i64));
    if best.1.1 > 2 {
        for off in -2048i64..=4096 {
            let s = score(off);
            if s.0 * 2 > want.len() / ch && s.2 > best.1.2 {
                best = (off, s);
            }
        }
    }
    let (off, (n, max, snr)) = best;
    eprintln!("{name}: offset {off}, {n} frames, max diff {max}, {snr:.1} dB");
    (off, n, max, snr, ch)
}

#[test]
fn recognised_and_opened() {
    for &(name, format) in FILES {
        let s = open(name);
        assert_eq!(s.info().format, format, "{name}");
        let v = s.video().expect("sound facts");
        assert!(v.sound.is_some(), "{name}");
        assert!(
            v.duration_us
                .is_some_and(|d| (90_000..400_000).contains(&d)),
            "{name}: {:?}",
            v.duration_us
        );
        assert!(v.silent_reason.is_none(), "{name}: {:?}", v.silent_reason);
        assert!(s.animation().is_some(), "{name}");
        assert_eq!((s.info().width, s.info().height), (1600, 1000));
    }
}

#[test]
fn lossless_and_pcm_match_ffmpeg() {
    for name in [
        "snd-flac.flac",
        "snd-flac24.flac",
        "snd-wav.wav",
        "snd-f32.wav",
        "snd-51.wav",
        "snd-ulaw.wav",
        "snd-ima.wav",
        "snd-aiff.aiff",
        "snd-sowt.aifc",
        "snd-alac.m4a",
        "snd-mka.mka",
    ] {
        let (off, n, max, _) = against(name);
        assert_eq!(off, 0, "{name}");
        assert!(n > 0, "{name}");
        // 5.1 is folded the same way by neither: compared per channel, so exact too.
        assert!(max <= 1, "{name}: {max}");
    }
}

#[test]
fn lossy_match_ffmpeg() {
    for (name, offset, min_snr) in [
        ("snd-mp3.mp3", None, 90.0),
        ("snd-vbr.mp3", None, 90.0),
        ("snd-mp2.mp2", Some(0), 80.0),
        ("snd-aac.aac", None, 60.0),
        ("snd-m4a.m4a", None, 60.0),
        ("snd-ac3.ac3", Some(0), 30.0),
        ("snd-eac3.eac3", Some(0), 30.0),
        ("snd-dts.dts", Some(0), 60.0),
        ("snd-vorbis.ogg", None, 60.0),
        ("snd-opus.opus", None, 50.0),
    ] {
        let (off, n, _max, snr) = against(name);
        if let Some(o) = offset {
            assert_eq!(off, o, "{name}");
        }
        assert!(n > 1000, "{name}");
        assert!(snr >= min_snr, "{name}: {snr:.1} dB");
    }
}

#[test]
fn gapless_mp3_starts_where_ffmpeg_does() {
    // The LAME frame's delay plus the decoder's 529: the oracle (trimmed by ffmpeg) lines
    // up at the edit's start.
    for name in ["snd-mp3.mp3", "snd-vbr.mp3"] {
        let src = Arc::new(Source::open(data(name)).unwrap());
        let m = specular_video::parse_movie(src.bytes(), true).unwrap();
        let t = m.audio.unwrap();
        assert!(t.edit.start > 529, "{name}: {:?}", t.edit);
        let (off, _, max, _) = against(name);
        assert_eq!(off, t.edit.start, "{name}");
        assert!(max <= 2, "{name}: {max}");
    }
}

/// mutagen's reading (`.tags`): (key, value) lines, and the cover as "MIME WxH".
fn tag_oracle(name: &str) -> (Vec<(String, String)>, Option<String>) {
    let text = std::fs::read_to_string(data(&format!("{}.tags", name.rsplit_once('.').unwrap().0)))
        .unwrap_or_default();
    let mut fields = Vec::new();
    let mut cover = None;
    for l in text.lines() {
        let (k, v) = l.split_once('=').unwrap();
        if k == "cover" {
            cover = Some(v.to_owned());
        } else {
            fields.push((k.to_owned(), v.to_owned()));
        }
    }
    (fields, cover)
}

#[test]
fn tags_match_mutagen() {
    let mut checked = 0;
    let mut mismatches = Vec::new();
    for &(name, _) in FILES {
        if name.ends_with(".mka") {
            continue;
        }
        let s = open(name);
        let facts = s.video().unwrap().sound.clone().unwrap();
        let (want, cover) = tag_oracle(name);
        for (k, v) in &want {
            checked += 1;
            let got = facts
                .fields
                .iter()
                .find(|(gk, _)| gk == k)
                .map(|(_, v)| v.as_str());
            if got != Some(v.as_str()) {
                mismatches.push(format!("{name}: {k}: {got:?} ≠ {v:?}"));
            }
        }
        // Nothing the oracle doesn't have (an ID3v1 copy adds nothing new).
        for (k, v) in &facts.fields {
            if !want.iter().any(|(wk, _)| wk == k) {
                mismatches.push(format!("{name}: extra {k}={v:?}"));
            }
        }
        if let Some(c) = cover {
            checked += 1;
            let (mime, size) = c.split_once(' ').unwrap();
            let want = format!(
                "{} {}",
                mime.trim_start_matches("image/").to_ascii_uppercase(),
                size.replace('x', " × ")
            );
            if facts.cover.as_deref() != Some(want.as_str()) {
                mismatches.push(format!("{name}: cover {:?} ≠ {want:?}", facts.cover));
            }
        } else if facts.cover.is_some() {
            mismatches.push(format!("{name}: extra cover {:?}", facts.cover));
        }
    }
    eprintln!("tags: {checked} checked, {} mismatches", mismatches.len());
    assert!(mismatches.is_empty(), "{mismatches:#?}");
    assert!(checked > 60, "{checked}");
}

#[test]
fn picture_thumbnail_and_playhead() {
    let cancel = CancelToken::new();
    // With a cover: the thumbnail is the cover.
    let s = open("snd-mp3.mp3");
    let thumb = s.decode_sampled(256, &cancel).unwrap().unwrap();
    assert_eq!((thumb.width, thumb.height), (64, 48));
    let full = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
    assert_eq!((full.width, full.height), (1600, 1000));
    // Without one: a sampled waveform at the size asked.
    let s = open("snd-wav.wav");
    let thumb = s.decode_sampled(256, &cancel).unwrap().unwrap();
    assert_eq!((thumb.width, thumb.height), (256, 160));
    // Some of the waveform is drawn (its colours, not just the lanes).
    let px = &thumb.planes[0].data;
    let lit = px
        .chunks_exact(4)
        .filter(|p| p[2] > 150 && p[0] < 160)
        .count();
    assert!(lit > 100, "{lit}");
    // The timeline: ten frames a second, each the page as it stands (the app draws the
    // playhead on the GPU); seeking lands on the frame.
    let n = s.animation().unwrap().frame_count.unwrap();
    assert_eq!(n, 3);
    let mut f = s.frames(1).unwrap();
    let delays = f.delays().unwrap();
    assert_eq!(delays.len(), 3);
    let total: std::time::Duration = delays.iter().sum();
    assert_eq!(total.as_millis(), 300);
    let a = f.next_frame(&cancel).unwrap().unwrap();
    let b = f.next_frame(&cancel).unwrap().unwrap();
    assert_eq!((a.index, b.index), (0, 1));
    assert_eq!(a.raster.planes[0].data, b.raster.planes[0].data);
    let page = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
    assert_eq!(a.raster.planes[0].data, page.planes[0].data);
    assert!(f.seek(2).unwrap());
    assert_eq!(f.next_frame(&cancel).unwrap().unwrap().index, 2);
    assert!(f.next_frame(&cancel).is_none());
}

#[test]
fn plays_through_the_player() {
    for &(name, _) in FILES {
        let src = Arc::new(Source::open(data(name)).unwrap());
        let p = specular_video::session::open_sound(src, Box::new(FakeOutput::new(48_000, 2)));
        assert!(p.is_ok(), "{name}: {:?}", p.err());
    }
}

#[test]
fn not_sound_files() {
    specular_video::install();
    // A movie is still a movie; a JPEG isn't sound.
    let s = open("alac.mp4");
    assert!(s.video().unwrap().sound.is_none());
    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
    jpeg.extend(std::iter::repeat_n(0u8, 64));
    assert_eq!(specular_video::sound_file::sniff(&jpeg), None);
}

/// Opening, the picture and the thumbnail of a long file (`SOUND_TIMING=path`).
#[test]
#[ignore]
fn timing() {
    let Some(path) = std::env::var_os("SOUND_TIMING") else {
        return;
    };
    specular_video::install();
    let cancel = CancelToken::new();
    let t = std::time::Instant::now();
    let s = Registry::new()
        .open(Arc::new(Source::open(&path).unwrap()))
        .unwrap();
    let open = t.elapsed();
    let t = std::time::Instant::now();
    s.decode(&DecodeRequest::full(1), &cancel).unwrap();
    let picture = t.elapsed();
    let t = std::time::Instant::now();
    s.decode_sampled(256, &cancel).unwrap().unwrap();
    let thumb = t.elapsed();
    let t = std::time::Instant::now();
    let mut f = s.frames(1).unwrap();
    for _ in 0..10 {
        f.next_frame(&cancel).unwrap().unwrap();
    }
    let frame = t.elapsed() / 10;
    eprintln!("{path:?}: open {open:?}, picture {picture:?}, thumbnail {thumb:?}, frame {frame:?}");
}

/// Surround (gen_fixtures_surround.sh) decoded as a surround device gets it, against
/// ffmpeg's decode channel by channel: 5.1 AC-3, E-AC-3 and DTS, 7.1 FLAC, AAC, Vorbis and Opus.
#[test]
fn surround_matches_ffmpeg() {
    for (name, channels, exact, min_snr) in [
        ("snd-51.wav", 6, true, 0.0),
        ("snd-flac-71.flac", 8, true, 0.0),
        ("snd-ac3-51.ac3", 6, false, 30.0),
        ("snd-eac3-51.eac3", 6, false, 30.0),
        ("snd-dts-51.dts", 6, false, 50.0),
        ("snd-aac-71.m4a", 8, false, 40.0),
        ("snd-vorbis-71.ogg", 8, false, 50.0),
        // Against libopus (ffmpeg's own Opus decoder is 14.6 dB off on the hybrid centre):
        // the CELT streams to ~84 dB, the hybrid centre (48.8 dB) and LFE (53.4 dB) as our
        // SILK synthesis goes (specular-opus surround_channels_match_libopus).
        ("snd-opus-71.opus", 8, false, 50.0),
    ] {
        let (off, n, max, snr, ch) = against_with(name, true);
        assert_eq!(ch, channels, "{name}");
        assert!(n > 1000, "{name}: {n}");
        if exact {
            assert_eq!(off, 0, "{name}");
            assert!(max <= 1, "{name}: {max}");
        } else {
            assert!(snr >= min_snr, "{name}: {snr:.1} dB");
        }
    }
}

/// What a fake device of `channels` in `order` plays of `name` at the start, and the
/// player's account of it.
fn played(
    name: &str,
    channels: u16,
    order: specular_video::speakers::DeviceOrder,
    frames: usize,
) -> (Vec<f32>, audio::OutputInfo, Vec<u16>) {
    let src = Arc::new(Source::open(data(name)).unwrap());
    let out = FakeOutput::new(48_000, channels).with_order(order);
    let p = specular_video::session::open_sound(src, Box::new(out.clone())).unwrap();
    p.play(std::time::Duration::ZERO);
    assert!(p.wait_queued(frames));
    out.pull(frames, std::time::Instant::now());
    let v = out.played.lock().clone();
    let info = p.output();
    let asked = out.asked.lock().clone();
    (v, info, asked)
}

/// 5.1 plays to each speaker of a 5.1 device in its platform's order, bit for bit; on a
/// stereo device it folds; 7.1 on 5.1 speakers folds the back pair in.
#[test]
fn surround_reaches_its_speakers() {
    use specular_video::speakers::DeviceOrder;
    let src = Arc::new(Source::open(data("snd-51.wav")).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let (pcm, ch, _) = audio::decode_track_surround(movie.audio.as_ref().unwrap(), &src).unwrap();
    assert_eq!(ch, 6);
    let n = 960;
    // WAVE L R C LFE Ls Rs to ALSA's FL FR RL RR FC LFE, and as is to Windows'.
    for (order, perm) in [
        (DeviceOrder::Alsa, [0, 1, 4, 5, 2, 3]),
        (DeviceOrder::Wave, [0, 1, 2, 3, 4, 5]),
    ] {
        let (v, info, asked) = played("snd-51.wav", 6, order, n);
        assert_eq!(asked, [6], "the sound's channels are asked for");
        assert!(!info.downmixed && !info.resampled, "{info:?}");
        assert_eq!((info.source_channels, info.format.channels), (6, 6));
        let mut wrong = 0;
        for f in 0..n {
            for (slot, &k) in perm.iter().enumerate() {
                if v[f * 6 + slot] != pcm[f * 6 + k] {
                    wrong += 1;
                }
            }
        }
        assert_eq!(wrong, 0, "{order:?}: samples off their speaker");
    }
    let (v, info, _) = played("snd-51.wav", 2, DeviceOrder::Alsa, n);
    assert!(info.downmixed && info.format.channels == 2, "{info:?}");
    assert!(v.iter().any(|&s| s != 0.0));
    let (_, info, _) = played("snd-flac-71.flac", 6, DeviceOrder::Wave, n);
    assert!(info.downmixed && info.source_channels == 8, "{info:?}");
    let (_, info, _) = played("snd-flac-71.flac", 8, DeviceOrder::CoreAudio, n);
    assert!(!info.downmixed && info.source_channels == 8, "{info:?}");
    // AC-3 5.1 on a 5.1 device: every channel decoded (not the stream's stereo fold).
    let (_, info, asked) = played("snd-ac3-51.ac3", 6, DeviceOrder::Wave, n);
    assert_eq!(asked, [6]);
    assert!(!info.downmixed && info.source_channels == 6, "{info:?}");
    let (_, info, _) = played("snd-ac3-51.ac3", 2, DeviceOrder::Wave, n);
    assert!(
        info.downmixed && info.source_channels == 6,
        "the stream's own fold: {info:?}"
    );
}

/// E-AC-3 7.1 by a dependent substream (specular-ac3's `e71` fixture: 5.1 plus a back
/// pair) is an 8-channel sound: each frame with its dependent frame is one sample, all 8
/// channels decode for a 7.1 device (the back pair sounding), and it plays undownmixed on
/// one.
#[test]
fn eac3_dependent_substreams_play_as_71() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-ac3/tests/data/e71.eac3");
    let src = Arc::new(Source::open(&path).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let t = movie.audio.as_ref().unwrap();
    assert_eq!(t.channels, 8);
    let (pcm, ch, _) = audio::decode_track_surround(t, &src).unwrap();
    assert_eq!(ch, 8);
    assert_eq!(pcm.len(), t.samples.len() * 1536 * 8);
    for c in 0..8 {
        let e: f64 = pcm
            .iter()
            .skip(c)
            .step_by(8)
            .map(|&v| f64::from(v).powi(2))
            .sum();
        assert!(e > 1.0, "channel {c} silent ({e})");
    }
    let out = FakeOutput::new(48_000, 8);
    let p = specular_video::session::open_sound(src, Box::new(out.clone())).unwrap();
    p.play(std::time::Duration::ZERO);
    assert!(p.wait_queued(960));
    let info = p.output();
    assert!(!info.downmixed && info.source_channels == 8, "{info:?}");
}

/// A sound moved to another device fades in from silence and out to silence: no click.
#[test]
fn fades_in_and_out() {
    let src = Arc::new(Source::open(data("snd-wav.wav")).unwrap());
    let out = FakeOutput::new(48_000, 2);
    let p = specular_video::session::open_sound(src, Box::new(out.clone())).unwrap();
    p.fade_in();
    p.play(std::time::Duration::from_millis(50));
    let fade = (audio::FADE.as_secs_f64() * 48_000.0) as usize;
    assert!(p.wait_queued(fade * 2));
    out.pull(fade * 2, std::time::Instant::now());
    let v = out.played.lock().clone();
    let peak = |a: &[f32]| a.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(peak(&v[..8]) < 0.01, "starts from silence");
    assert!(peak(&v[fade * 2..]) > 0.2, "then at full level");
    // Each step of the ramp is no bigger than the sound's own (under 0.12 here): no jump.
    assert!(v.windows(4).all(|w| (w[2] - w[0]).abs() < 0.15));
    p.fade_out();
    assert!(p.wait_queued(fade * 2));
    out.pull(fade * 2, std::time::Instant::now());
    let v = out.played.lock().clone();
    let tail = &v[v.len() - fade * 2..];
    assert!(peak(&tail[tail.len() - 8..]) < 0.01, "ends in silence");
    assert!(tail.windows(4).all(|w| (w[2] - w[0]).abs() < 0.15));
}

/// On a 5.1 device the live tap (meters, goniometer, spectrum) still gets stereo: the
/// heard sound folded from the device's order.
#[test]
fn live_tap_is_stereo_on_surround_output() {
    use specular_video::speakers::DeviceOrder;
    let src = Arc::new(Source::open(data("snd-51.wav")).unwrap());
    let out = FakeOutput::new(48_000, 6).with_order(DeviceOrder::Alsa);
    let p = specular_video::session::open_sound(src, Box::new(out.clone())).unwrap();
    p.play(std::time::Duration::ZERO);
    assert!(p.wait_queued(960));
    out.pull(960, std::time::Instant::now());
    let tap = p.tap();
    assert_eq!(tap.format(), Some((2, 48_000)));
    let played = out.played.lock().clone();
    let mut want = Vec::new();
    specular_video::speakers::device_to_stereo(&played, 6, DeviceOrder::Alsa, &mut want);
    let mut got = Vec::new();
    let h = tap.read_at(0.01, 240, &mut got).expect("heard");
    assert_eq!(h.channels, 2);
    assert!(got.iter().any(|&v| v != 0.0));
    // What the tap holds is the fold of what the device played.
    assert!(got.chunks(2).all(|f| want.chunks(2).any(|w| w == f)));
}

/// What a frame of the sound page's timeline costs: the page picture's (`frames`, for
/// exports and saved frames) and the viewer's frames without a picture (`clock_frames`,
/// the same times).
#[test]
fn page_frame_cost() {
    let cancel = CancelToken::new();
    let s = open("snd-wav.wav");
    let snd = s
        .as_any()
        .and_then(|a| a.downcast_ref::<specular_video::sound_view::SoundSession>())
        .unwrap();
    let cost = |mut f: Box<dyn specular_decode::Frames>| {
        let t = std::time::Instant::now();
        let (mut n, mut d) = (0u32, Vec::new());
        for _ in 0..20 {
            f.rewind().unwrap();
            while let Some(fr) = f.next_frame(&cancel) {
                let fr = fr.unwrap();
                d.push(fr.delay);
                std::hint::black_box(fr);
                n += 1;
            }
        }
        (t.elapsed() / n, d)
    };
    let (page, page_d) = cost(s.frames(1).unwrap());
    let (clock, clock_d) = cost(snd.clock_frames());
    eprintln!("page frames {page:?}, clock frames {clock:?} per frame");
    assert_eq!(page_d, clock_d);
    assert!(clock < page);
}
