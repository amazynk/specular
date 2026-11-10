//! HLS and DASH from an in-process server on 127.0.0.1: a seek outside the pieces fetched
//! (the worker goes on from the segment holding the time), and a TS variant whose sound is
//! a separate rendition (MPEG-TS, or packed ADTS with ID3 times) merged into its pieces.
//!
//! The oracle: media made by ffmpeg (a test tool only; skipped when it is not installed);
//! the pieces' samples must equal the original's, byte for byte, from the seek on.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_remote::testing::{ServerOptions, TestRequest, TestResponse, TestServer};
use specular_remote::{Client, ClientConfig};
use specular_stream::{Options, Status, Stream};

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("stream-pieces")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn have_ffmpeg() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn ffmpeg(dir: &Path, args: &[&str]) {
    let out = Command::new("ffmpeg")
        .current_dir(dir)
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .output()
        .expect("ffmpeg runs");
    assert!(
        out.status.success(),
        "ffmpeg {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

type Files = HashMap<String, Vec<u8>>;

fn serve(files: Files) -> TestServer {
    let files = Arc::new(files);
    TestServer::start(
        ServerOptions::default(),
        move |r: &TestRequest| match files.get(r.path()) {
            Some(data) => {
                let ct = match r.path().rsplit('.').next() {
                    Some("m3u8") => "application/vnd.apple.mpegurl",
                    Some("mpd") => "application/dash+xml",
                    Some("ts") => "video/mp2t",
                    Some("aac") => "audio/aac",
                    _ => "video/mp4",
                };
                TestResponse::ranged(r, data, "\"e\"").header("Content-Type", ct)
            }
            None => TestResponse::status(404),
        },
    )
    .unwrap()
}

fn add_dir(files: &mut Files, dir: &Path, prefix: &str) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if p.is_dir() {
            add_dir(files, &p, &format!("{prefix}/{name}"));
        } else {
            files.insert(format!("{prefix}/{name}"), std::fs::read(&p).unwrap());
        }
    }
}

fn options(dir: PathBuf) -> Options {
    let client = Client::new(ClientConfig::for_tests()).unwrap();
    Options::new(dir, client)
}

fn wait(s: &Stream, secs: u64, done: impl Fn(&Status) -> bool) -> Status {
    let t0 = Instant::now();
    loop {
        let st = s.status();
        if done(&st) || st.ended || t0.elapsed() > Duration::from_secs(secs) {
            return st;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Every sample of each track: (picture, sound).
fn samples(path: &Path) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let data = std::fs::read(path).unwrap();
    let m = specular_video::parse_movie(&data, true)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let v = m.video.as_ref().map_or(Vec::new(), |t| {
        t.samples
            .iter()
            .map(|s| t.sample_data(&data, s).unwrap().into_owned())
            .collect()
    });
    let a = m.audio.as_ref().map_or(Vec::new(), |t| {
        t.samples
            .iter()
            .map(|s| t.sample_data(&data, s).unwrap().into_owned())
            .collect()
    });
    (v, a)
}

/// ADTS frames of samples joined (TS sound samples are PES packets, packed differently).
fn adts_frames(samples: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let all: Vec<u8> = samples.concat();
    let mut out = Vec::new();
    let mut at = 0;
    while at + 7 <= all.len() {
        let Some((n, _)) = specular_stream::frames::adts(&all[at..]) else {
            break;
        };
        out.push(all[at..(at + n).min(all.len())].to_vec());
        at += n;
    }
    out
}

fn mismatches(got: &[Vec<u8>], want: &[Vec<u8>]) -> usize {
    got.iter().zip(want).filter(|(g, w)| g != w).count() + got.len().abs_diff(want.len())
}

struct Made {
    dir: PathBuf,
}

/// A 12 s movie at 25 fps (a key frame every 2 s), cut as HLS (TS, and TS with the sound
/// as a separate rendition) and DASH, and the same as one MPEG-TS file.
fn made() -> Option<&'static Made> {
    static MADE: std::sync::OnceLock<Option<Made>> = std::sync::OnceLock::new();
    MADE.get_or_init(|| {
        if !have_ffmpeg() {
            return None;
        }
        let dir = tmp("media");
        ffmpeg(
            &dir,
            &[
                "-f",
                "lavfi",
                "-i",
                "testsrc=rate=25:duration=12:size=320x240",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=12",
                "-c:v",
                "libx264",
                "-g",
                "50",
                "-keyint_min",
                "50",
                "-sc_threshold",
                "0",
                "-bf",
                "2",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "96k",
                "-shortest",
                "src.mp4",
            ],
        );
        ffmpeg(&dir, &["-i", "src.mp4", "-c", "copy", "whole.ts"]);
        ffmpeg(&dir, &["-i", "src.mp4", "-c", "copy", "-an", "picture.ts"]);
        for d in ["ts", "sep", "dash", "packed"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        ffmpeg(
            &dir,
            &[
                "-i",
                "src.mp4",
                "-c",
                "copy",
                "-f",
                "hls",
                "-hls_time",
                "2",
                "-hls_playlist_type",
                "vod",
                "-hls_segment_filename",
                "ts/s%03d.ts",
                "ts/index.m3u8",
            ],
        );
        ffmpeg(
            &dir,
            &[
                "-i",
                "src.mp4",
                "-map",
                "0:v",
                "-map",
                "0:a",
                "-c",
                "copy",
                "-f",
                "hls",
                "-hls_time",
                "2",
                "-hls_playlist_type",
                "vod",
                "-master_pl_name",
                "master.m3u8",
                "-var_stream_map",
                "v:0,agroup:aud a:0,agroup:aud",
                "-hls_segment_filename",
                "sep/%v_%03d.ts",
                "sep/%v.m3u8",
            ],
        );
        ffmpeg(
            &dir,
            &[
                "-i",
                "src.mp4",
                "-c",
                "copy",
                "-f",
                "dash",
                "-seg_duration",
                "2",
                "-use_template",
                "1",
                "-use_timeline",
                "1",
                "dash/manifest.mpd",
            ],
        );
        Some(Made { dir })
    })
    .as_ref()
}

/// Plays the pieces as they come (the worker keeps only a little ahead).
fn play_along(s: &Stream, secs: u64, done: impl Fn(&Status) -> bool) -> Status {
    let t0 = Instant::now();
    loop {
        let st = s.status();
        if done(&st) || st.ended || t0.elapsed() > Duration::from_secs(secs) {
            return st;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// HLS and DASH: a seek past what is fetched drops the pieces ahead and goes on from the
/// segment holding the time; that piece's samples are the original's from there.
#[test]
fn a_seek_outside_the_pieces() {
    let Some(m) = made() else {
        eprintln!("ffmpeg not installed: skipped");
        return;
    };
    let mut files = HashMap::new();
    add_dir(&mut files, &m.dir, "");
    let server = serve(files);
    let mp4 = samples(&m.dir.join("src.mp4"));
    let (ts_v, ts_a) = samples(&m.dir.join("whole.ts"));
    let ts = (ts_v, adts_frames(&ts_a));
    for (path, name) in [("/ts/index.m3u8", "hls"), ("/dash/manifest.mpd", "dash")] {
        // TS pieces against the whole stream as TS (sound as ADTS frames), MP4 against MP4.
        let (want_v, want_a) = if name == "hls" { &ts } else { &mp4 };
        let mut o = options(tmp(&format!("seek-{name}")));
        o.piece_s = 2.0;
        o.ahead_s = 2.0;
        let s = Stream::open(o, &server.url(path));
        let st = play_along(&s, 30, |st| st.chunks.len() >= 2);
        assert!(st.failure.is_none(), "{name}: {:?}", st.failure);
        assert_eq!(
            st.duration.map(f64::round),
            Some(12.0),
            "{name}: the whole length"
        );
        s.playing(st.chunks[0].index);
        assert!(s.chunk_at(9.0).is_none(), "{name}: not fetched yet");
        let t0 = Instant::now();
        s.seek(9.0);
        let st = wait(&s, 30, |_| s.chunk_at(9.0).is_some());
        let took = t0.elapsed();
        let c = s
            .chunk_at(9.0)
            .unwrap_or_else(|| panic!("{name}: {:?}", st.failure));
        let first = st.seek.unwrap().1;
        assert!(c.index >= first);
        assert!(
            (c.start - 8.0).abs() < 0.05,
            "{name}: starts at {}",
            c.start
        );
        // Only the piece playing is kept from before.
        assert!(
            st.chunks
                .iter()
                .all(|k| k.index == st.chunks[0].index || k.index >= first)
        );
        let (gv, ga) = samples(&c.path);
        let ga = if name == "hls" { adts_frames(&ga) } else { ga };
        // From 8 s: picture sample 200 on, sound frame 8 s × 48000 / 1024 = 375 on (DASH
        // sound segments start within the piece, so may begin a frame or so later).
        let v0 = 200;
        let bv = mismatches(&gv, &want_v[v0..v0 + gv.len()]);
        let a0 = want_a.iter().position(|w| ga.first() == Some(w));
        let ba = a0.map_or(ga.len(), |a0| {
            mismatches(&ga, &want_a[a0..(a0 + ga.len()).min(want_a.len())])
        });
        eprintln!(
            "{name} seek to 9 s: piece {} at {:.2} s ready in {:.0} ms; picture {} samples from 200, {bv} mismatches; sound {} frames from {a0:?}, {ba} mismatches",
            c.index,
            c.start,
            took.as_secs_f64() * 1e3,
            gv.len(),
            ga.len()
        );
        assert_eq!((bv, ba), (0, 0), "{name}");
        // MPEG-TS carries its sound up to a second ahead of the picture it goes with; a DASH
        // piece after a seek starts with the sound segment running at its start.
        assert!(
            a0.is_some_and(|a| (320..=380).contains(&a)),
            "{name}: sound from {a0:?}"
        );
    }
}

/// HLS whose variant names a sound group with its own playlist: MPEG-TS sound segments
/// (ffmpeg's `var_stream_map`) and packed ADTS segments with ID3 times are merged into the
/// TS pieces; picture and sound are the original's.
#[test]
fn ts_with_a_separate_sound_rendition() {
    let Some(m) = made() else {
        eprintln!("ffmpeg not installed: skipped");
        return;
    };
    let mut files = HashMap::new();
    add_dir(&mut files, &m.dir, "");
    // Packed ADTS: the original's sound frames in 2 s segments behind ID3 timestamps, the
    // picture as ffmpeg's TS segments without sound.
    let (_, whole_a) = samples(&m.dir.join("whole.ts"));
    let frames = adts_frames(&whole_a);
    let mut audio_pl = String::from(
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n#EXT-X-PLAYLIST-TYPE:VOD\n",
    );
    let per = 94; // frames of 1024 at 48 kHz: about 2 s
    for (i, seg) in frames.chunks(per).enumerate() {
        let pts: u64 = 126_000 + (i * per) as u64 * 1024 * 90_000 / 48_000;
        let owner = b"com.apple.streaming.transportStreamTimestamp\0";
        let mut frame = b"PRIV".to_vec();
        let body_len = owner.len() + 8;
        frame.extend_from_slice(&(body_len as u32).to_be_bytes());
        frame.extend_from_slice(&[0, 0]);
        frame.extend_from_slice(owner);
        frame.extend_from_slice(&pts.to_be_bytes());
        let n = frame.len();
        let mut b = b"ID3\x04\x00\x00".to_vec();
        b.extend_from_slice(&[
            (n >> 21) as u8 & 0x7F,
            (n >> 14) as u8 & 0x7F,
            (n >> 7) as u8 & 0x7F,
            n as u8 & 0x7F,
        ]);
        b.extend_from_slice(&frame);
        b.extend_from_slice(&seg.concat());
        files.insert(format!("/packed/a{i:03}.aac"), b);
        audio_pl.push_str(&format!(
            "#EXTINF:{:.6},\na{i:03}.aac\n",
            seg.len() as f64 * 1024.0 / 48_000.0
        ));
    }
    audio_pl.push_str("#EXT-X-ENDLIST\n");
    files.insert("/packed/audio.m3u8".into(), audio_pl.into_bytes());
    files.insert(
        "/packed/master.m3u8".into(),
        b"#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"Sound\",DEFAULT=YES,URI=\"audio.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=900000,AUDIO=\"a\"\n../sep/0.m3u8\n"
            .to_vec(),
    );
    let server = serve(files);
    let (want_v, want_a) = samples(&m.dir.join("whole.ts"));
    let want_a = adts_frames(&want_a);
    for (path, name) in [
        ("/sep/master.m3u8", "ts sound"),
        ("/packed/master.m3u8", "packed sound"),
    ] {
        let mut o = options(tmp(&format!("sep-{}", name.replace(' ', "-"))));
        o.ahead_s = 1e9;
        let s = Stream::open(o, &server.url(path));
        let st = wait(&s, 60, |st| st.ended);
        assert!(st.failure.is_none(), "{name}: {:?}", st.failure);
        assert!(
            st.chunks
                .iter()
                .all(|c| c.path.extension().unwrap() == "ts")
        );
        let mut gv = Vec::new();
        let mut ga = Vec::new();
        for c in &st.chunks {
            let (v, a) = samples(&c.path);
            assert!(!a.is_empty(), "{name}: piece {} has sound", c.index);
            gv.extend(v);
            ga.extend(a);
        }
        let ga = adts_frames(&ga);
        let (bv, ba) = (mismatches(&gv, &want_v), mismatches(&ga, &want_a));
        eprintln!(
            "{name}: {} pieces; picture {}/{} samples, {bv} mismatches; sound {}/{} frames, {ba} mismatches",
            st.chunks.len(),
            gv.len(),
            want_v.len(),
            ga.len(),
            want_a.len()
        );
        assert_eq!((bv, ba), (0, 0), "{name}");
    }
}

/// Where a piece's picture and sound start and end on its own time line (µs): first
/// presentation time, and the latest a sample ends (an MPEG-TS sound sample is a PES
/// packet of several frames: its own length counts).
fn spans(path: &Path) -> Option<[i64; 4]> {
    let data = std::fs::read(path).ok()?;
    let m = specular_video::parse_movie(&data, true).ok()?;
    let span = |t: Vec<(i64, i64)>| -> Option<(i64, i64)> {
        let first = t.iter().map(|x| x.0).min()?;
        let last = t.iter().map(|x| x.1).max()?;
        Some((first, last))
    };
    let (v, a) = (m.video.as_ref()?, m.audio.as_ref()?);
    // Presentation times: the edit list's start and empty edit applied, as the player does.
    let pv = |t: i64| v.us(t - v.edit.start) + v.edit.delay_us as i64;
    let pa = |t: i64| a.us(t - a.edit.start) + a.edit.delay_us as i64;
    let (v0, v1) = span(
        v.samples
            .iter()
            .map(|s| (pv(s.cts), pv(s.cts + i64::from(s.duration))))
            .collect(),
    )?;
    // ADTS frames in a sample (1024 samples each), else its stated length.
    let frames = |b: &[u8]| {
        let (mut n, mut p) = (0i64, 0usize);
        while p + 7 <= b.len() && b[p] == 0xFF && b[p + 1] & 0xF0 == 0xF0 {
            let len = (usize::from(b[p + 3] & 3) << 11)
                | (usize::from(b[p + 4]) << 3)
                | usize::from(b[p + 5] >> 5);
            if len < 7 {
                break;
            }
            n += 1;
            p += len;
        }
        n
    };
    let (a0, a1) = span(
        a.samples
            .iter()
            .map(|s| {
                let n = a.sample_data(&data, s).map_or(0, |b| frames(&b));
                let end = if n > 0 {
                    pa(s.cts) + n * 1024 * 1_000_000 / i64::from(a.sample_rate)
                } else {
                    pa(s.cts + i64::from(s.duration))
                };
                (pa(s.cts), end)
            })
            .collect(),
    )?;
    Some([v0, v1, a0, a1])
}

/// At each join of `pieces` played one after another (the picture of the next from its
/// first frame when the one before ends, the sound spliced on without a gap): how much
/// later the sound comes than the picture it goes with (ms; a constant shift of a piece's
/// time line cancels out).
fn join_offsets(pieces: &[PathBuf]) -> Vec<f64> {
    let s: Vec<[i64; 4]> = pieces.iter().map(|p| spans(p).expect("a movie")).collect();
    s.windows(2)
        .map(|w| {
            let ([_, ev, _, ea], [sv, _, sa, _]) = (w[0], w[1]);
            // The next sound plays from where this one ends: its delay against the picture.
            ((ea - ev) - (sa - sv)) as f64 / 1e3
        })
        .collect()
}

/// HLS (sound in the segments, and as its own rendition) and DASH: at every join of the
/// pieces the sound lines up with the picture within one frame (40 ms).
#[test]
fn sound_and_picture_line_up_at_piece_joins() {
    let Some(m) = made() else {
        eprintln!("ffmpeg not installed: skipped");
        return;
    };
    let mut files = HashMap::new();
    add_dir(&mut files, &m.dir, "");
    let server = serve(files);
    for (path, name) in [
        ("/ts/index.m3u8", "hls"),
        ("/sep/master.m3u8", "hls-rendition"),
        ("/dash/manifest.mpd", "dash"),
    ] {
        let mut o = options(tmp(&format!("joins-{name}")));
        o.piece_s = 2.0;
        o.ahead_s = 100.0;
        let s = Stream::open(o, &server.url(path));
        let st = wait(&s, 60, |st| st.ended);
        assert!(st.failure.is_none(), "{name}: {:?}", st.failure);
        let paths: Vec<PathBuf> = st.chunks.iter().map(|c| c.path.clone()).collect();
        assert!(paths.len() >= 4, "{name}: {} pieces", paths.len());
        let off = join_offsets(&paths);
        let worst = off.iter().fold(0f64, |m, v| m.max(v.abs()));
        eprintln!(
            "ORACLE\tStream/A-V at joins\t{name}\t{}\t{} joins, sound later than picture (ms): {:?}; worst {worst:.1} ms",
            if worst <= 40.0 { "pass" } else { "fail" },
            off.len(),
            off.iter().map(|v| v.round() as i64).collect::<Vec<_>>()
        );
        assert!(worst <= 40.0, "{name}: {off:?}");
    }
}
