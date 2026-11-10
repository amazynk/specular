//! Streams from an in-process server on 127.0.0.1 (nothing else is contacted): radio with
//! ICY titles, a whole file, HLS (MPEG-TS, AES-128, fragmented MP4, variants) and DASH.
//!
//! The oracle: media made by ffmpeg (a test tool only), served as HLS or DASH, must come out
//! of our pieces with the same samples, byte for byte and in order, as the progressive file
//! ffmpeg made from the same encode. Skipped when ffmpeg is not installed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_remote::testing::{ServerOptions, TestRequest, TestResponse, TestServer};
use specular_remote::{Client, ClientConfig};
use specular_stream::{Failure, Options, Status, Stream, StreamKind};

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("stream")
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

type Files = Arc<HashMap<String, (Vec<u8>, &'static str)>>;

/// Serves `files` by path, with byte ranges.
fn serve(files: Files) -> TestServer {
    TestServer::start(
        ServerOptions::default(),
        move |r: &TestRequest| match files.get(r.path()) {
            Some((data, ct)) if r.header("range").is_some() => {
                TestResponse::ranged(r, data, "\"e\"").header("Content-Type", *ct)
            }
            Some((data, ct)) => TestResponse::ok(data.clone()).header("Content-Type", *ct),
            None => TestResponse::status(404),
        },
    )
    .unwrap()
}

/// Every file under `dir` at `/prefix/<relative path>`.
fn add_dir(files: &mut HashMap<String, (Vec<u8>, &'static str)>, dir: &Path, prefix: &str) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            add_dir(files, &p, &format!("{prefix}/{name}"));
            continue;
        }
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let ct = match p.extension().and_then(|e| e.to_str()) {
            Some("m3u8") => "application/vnd.apple.mpegurl",
            Some("mpd") => "application/dash+xml",
            Some("ts") => "video/mp2t",
            Some("mp4" | "m4s") => "video/mp4",
            Some("mp3") => "audio/mpeg",
            _ => "application/octet-stream",
        };
        files.insert(format!("{prefix}/{name}"), (std::fs::read(&p).unwrap(), ct));
    }
}

fn options(dir: PathBuf) -> Options {
    let client = Client::new(ClientConfig::for_tests()).unwrap();
    let mut o = Options::new(dir, client);
    o.ahead_s = 1e9;
    o
}

/// Waits until `done` holds (or the stream ended), marking pieces played as they come.
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

/// Every sample of each track, in order: (video, sound).
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

/// Samples of all pieces, joined.
fn piece_samples(st: &Status) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut v = Vec::new();
    let mut a = Vec::new();
    for c in &st.chunks {
        let (cv, ca) = samples(&c.path);
        v.extend(cv);
        a.extend(ca);
    }
    (v, a)
}

/// How many samples differ, and how many more or fewer there are.
fn mismatches(got: &[Vec<u8>], want: &[Vec<u8>]) -> usize {
    got.iter().zip(want).filter(|(g, w)| g != w).count() + got.len().abs_diff(want.len())
}

/// An MP3 stream with ICY titles: pieces of whole frames that parse as the same sound,
/// the titles shown, and the station's name.
#[test]
fn internet_radio_with_titles() {
    if !have_ffmpeg() {
        eprintln!("ffmpeg not installed: skipped");
        return;
    }
    let dir = tmp("radio");
    ffmpeg(
        &dir,
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=330:duration=20",
            "-c:a",
            "libmp3lame",
            "-b:a",
            "128k",
            "radio.mp3",
        ],
    );
    let mp3 = std::fs::read(dir.join("radio.mp3")).unwrap();
    let metaint = 8192;
    let mut body = Vec::new();
    for (i, part) in mp3.chunks(metaint).enumerate() {
        body.extend_from_slice(part);
        if part.len() == metaint {
            let t = format!("StreamTitle='Song {}';", i / 10);
            if i % 10 == 0 {
                let n = t.len().div_ceil(16);
                body.push(n as u8);
                let mut b = t.into_bytes();
                b.resize(n * 16, 0);
                body.extend(b);
            } else {
                body.push(0);
            }
        }
    }
    let body = Arc::new(body);
    let server = TestServer::start(ServerOptions::default(), move |r: &TestRequest| {
        if r.path() == "/radio.pls" {
            return TestResponse::ok(format!(
                "[playlist]\nNumberOfEntries=1\nFile1=http://{}/live\nTitle1=Test FM\n",
                r.header("host").unwrap_or("")
            ))
            .header("Content-Type", "audio/x-scpls");
        }
        assert_eq!(r.header("icy-metadata"), Some("1"));
        TestResponse::ok((*body).clone())
            .header("Content-Type", "audio/mpeg")
            .header("icy-metaint", metaint.to_string())
            .header("icy-name", "Test FM")
            .chunked(1000)
    })
    .unwrap();
    let s = Stream::open(options(tmp("radio-out")), &server.url("/radio.pls"));
    // The server closes the body: the worker connects again a few times, then stops.
    let st = wait(&s, 60, |st| st.ended);
    assert_eq!(st.kind, Some(StreamKind::Radio));
    assert_eq!(st.title.as_deref(), Some("Test FM"));
    assert!(
        st.now_playing
            .as_deref()
            .is_some_and(|t| t.starts_with("Song"))
    );
    // The first connection's pieces: the sound, unchanged and whole (the oracle is the
    // file's own frames).
    let (_, want) = samples(&dir.join("radio.mp3"));
    let mut got = Vec::new();
    let mut secs = 0.0;
    for c in &st.chunks {
        assert_eq!(c.path.extension().unwrap(), "mp3");
        got.extend(samples(&c.path).1);
        secs += c.seconds;
        if got.len() >= want.len() {
            break;
        }
    }
    let bad = mismatches(&got[..want.len().min(got.len())], &want);
    eprintln!(
        "radio: {} frames, {bad} mismatches, {secs:.2} s",
        want.len()
    );
    assert_eq!(bad, 0);
    assert!(st.chunks.iter().all(|c| c.seconds <= 5.2));
}

/// A whole file: fetched into one file, byte for byte.
#[test]
fn a_whole_file() {
    let data: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect();
    let mut files = HashMap::new();
    files.insert("/v/clip.mp4".to_owned(), (data.clone(), "video/mp4"));
    let server = serve(Arc::new(files));
    let s = Stream::open(options(tmp("file")), &server.url("/v/clip.mp4?sig=1"));
    let st = wait(&s, 20, |st| !st.chunks.is_empty());
    assert_eq!(st.kind, Some(StreamKind::File));
    assert_eq!(st.chunks.len(), 1, "{:?}", st.failure);
    assert_eq!(st.chunks[0].path.file_name().unwrap(), "clip.mp4");
    assert_eq!(std::fs::read(&st.chunks[0].path).unwrap(), data);
    assert_eq!(st.total, Some(data.len() as u64));
    // Dropping the stream deletes its files.
    let dir = s.dir().to_path_buf();
    drop(s);
    let t0 = Instant::now();
    while dir.exists() && t0.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!dir.exists());
}

struct Made {
    dir: PathBuf,
}

/// The source movie and its HLS and DASH forms, made once.
fn made() -> Option<&'static Made> {
    static MADE: std::sync::OnceLock<Option<Made>> = std::sync::OnceLock::new();
    MADE.get_or_init(|| {
        if !have_ffmpeg() {
            return None;
        }
        let dir = tmp("media");
        let common = [
            "-c:v", "libx264", "-g", "50", "-keyint_min", "50", "-sc_threshold", "0", "-bf", "2",
            "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "96k", "-shortest",
        ];
        for (name, size) in [("src.mp4", "320x240"), ("small.mp4", "160x120")] {
            let mut a = vec![
                "-f", "lavfi", "-i", "testsrc=rate=25:duration=12", "-f", "lavfi", "-i",
                "sine=frequency=440:duration=12", "-s", size,
            ];
            a.extend_from_slice(&common);
            a.push(name);
            ffmpeg(&dir, &a);
        }
        ffmpeg(&dir, &["-i", "src.mp4", "-c", "copy", "whole.ts"]);
        for d in ["ts", "ts2", "aes", "fm", "dash"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        let hls = |src: &str, out: &str, extra: &[&str]| {
            let mut a = vec!["-i", src, "-c", "copy", "-f", "hls", "-hls_time", "2", "-hls_playlist_type", "vod"];
            a.extend_from_slice(extra);
            a.push(out);
            ffmpeg(&dir, &a);
        };
        hls("src.mp4", "ts/index.m3u8", &["-hls_segment_filename", "ts/s%03d.ts"]);
        hls("small.mp4", "ts2/index.m3u8", &["-hls_segment_filename", "ts2/s%03d.ts"]);
        std::fs::write(dir.join("aes/key.bin"), [7u8; 16]).unwrap();
        std::fs::write(
            dir.join("keyinfo"),
            format!("key.bin\n{}\n", dir.join("aes/key.bin").display()),
        )
        .unwrap();
        hls(
            "src.mp4",
            "aes/index.m3u8",
            &["-hls_key_info_file", "keyinfo", "-hls_segment_filename", "aes/s%03d.ts"],
        );
        hls(
            "src.mp4",
            "fm/index.m3u8",
            &["-hls_segment_type", "fmp4", "-hls_fmp4_init_filename", "init.mp4", "-hls_segment_filename", "fm/s%03d.m4s"],
        );
        ffmpeg(
            &dir,
            &["-i", "src.mp4", "-c", "copy", "-f", "dash", "-seg_duration", "2", "-use_template", "1", "-use_timeline", "1", "dash/manifest.mpd"],
        );
        std::fs::write(
            dir.join("master.m3u8"),
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=300000,RESOLUTION=160x120\nts2/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=900000,RESOLUTION=320x240\nts/index.m3u8\n",
        )
        .unwrap();
        Some(Made { dir })
    })
    .as_ref()
}

fn media_server(m: &Made) -> TestServer {
    let mut files = HashMap::new();
    add_dir(&mut files, &m.dir, "");
    serve(Arc::new(files))
}

/// ADTS frames of the samples' bytes joined (an MPEG-TS sound sample is a PES packet, and
/// muxers pack frames into PES packets differently).
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
    assert_eq!(at, all.len(), "whole ADTS frames");
    out
}

fn check_against(st: &Status, oracle: &Path, what: &str) {
    assert!(st.failure.is_none(), "{what}: {:?}", st.failure);
    let (gv, mut ga) = piece_samples(st);
    let (wv, mut wa) = samples(oracle);
    if oracle.extension().is_some_and(|e| e == "ts") {
        ga = adts_frames(&ga);
        wa = adts_frames(&wa);
    }
    let (bv, ba) = (mismatches(&gv, &wv), mismatches(&ga, &wa));
    eprintln!(
        "{what}: {} pieces; picture {}/{} samples, {bv} mismatches; sound {}/{}, {ba} mismatches",
        st.chunks.len(),
        gv.len(),
        wv.len(),
        ga.len(),
        wa.len()
    );
    assert_eq!((bv, ba), (0, 0), "{what}");
}

/// MPEG-TS segments, clear and AES-128: the pieces hold the whole stream's samples.
#[test]
fn hls_ts_and_aes128() {
    let Some(m) = made() else {
        eprintln!("ffmpeg not installed: skipped");
        return;
    };
    let server = media_server(m);
    for (path, name) in [("/ts/index.m3u8", "hls-ts"), ("/aes/index.m3u8", "hls-aes")] {
        let s = Stream::open(options(tmp(name)), &server.url(path));
        let st = wait(&s, 60, |st| st.ended);
        assert_eq!(st.kind, Some(StreamKind::Hls));
        assert!(!st.live);
        assert!((st.duration.unwrap() - 12.0).abs() < 0.5);
        assert!(
            st.chunks
                .iter()
                .all(|c| c.path.extension().unwrap() == "ts")
        );
        assert!(
            st.chunks[0].seconds <= 2.5,
            "the first piece is one segment"
        );
        check_against(&st, &m.dir.join("whole.ts"), name);
    }
}

/// Fragmented MP4 (HLS) and DASH (picture and sound apart) become plain MP4 pieces with the
/// progressive file's samples.
#[test]
fn hls_fmp4_and_dash() {
    let Some(m) = made() else {
        eprintln!("ffmpeg not installed: skipped");
        return;
    };
    let server = media_server(m);
    for (path, name, kind) in [
        ("/fm/index.m3u8", "hls-fmp4", StreamKind::Hls),
        ("/dash/manifest.mpd", "dash", StreamKind::Dash),
    ] {
        let s = Stream::open(options(tmp(name)), &server.url(path));
        let st = wait(&s, 60, |st| st.ended);
        assert_eq!(st.kind, Some(kind), "{:?}", st.failure);
        assert!(
            st.chunks
                .iter()
                .all(|c| c.path.extension().unwrap() == "mp4")
        );
        check_against(&st, &m.dir.join("src.mp4"), name);
    }
}

/// Variants: the lowest first (nothing measured), then the one chosen from the next piece.
#[test]
fn hls_variants_switch() {
    let Some(m) = made() else {
        eprintln!("ffmpeg not installed: skipped");
        return;
    };
    let server = media_server(m);
    let mut o = options(tmp("variants"));
    o.ahead_s = 3.0;
    o.piece_s = 2.0;
    o.variant = Some(0);
    let s = Stream::open(o, &server.url("/master.m3u8"));
    let st = wait(&s, 30, |st| !st.chunks.is_empty());
    assert_eq!(st.variants, ["160×120, 300 kbit/s", "320×240, 900 kbit/s"]);
    assert_eq!(st.variant, Some(0));
    s.choose(Some(1));
    s.playing(st.chunks[0].index);
    // Keep playing the pieces as they come so the worker goes on.
    let t0 = Instant::now();
    let st = loop {
        let st = s.status();
        if let Some(c) = st.chunks.last() {
            s.playing(c.index);
        }
        if st.ended || t0.elapsed() > Duration::from_secs(60) {
            break st;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(st.failure.is_none(), "{:?}", st.failure);
    assert_eq!(st.variant, Some(1));
    assert!(st.manual);
    let log = server.log();
    assert!(log.iter().any(|r| r.path().starts_with("/ts2/s")));
    assert!(log.iter().any(|r| r.path().starts_with("/ts/s")));
    // Pieces before the one playing are deleted.
    let last = st.chunks.last().unwrap().index;
    assert!(st.chunks.iter().all(|c| c.index + 1 >= last));
}

/// DRM, a page without yt-dlp, and a missing file are refused in words the app can say.
#[test]
fn refusals() {
    let mut files = HashMap::new();
    files.insert(
        "/drm.m3u8".to_owned(),
        (
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://x\",KEYFORMAT=\"com.apple.streamingkeydelivery\"\n#EXTINF:4,\na.ts\n#EXT-X-ENDLIST\n".to_vec(),
            "application/vnd.apple.mpegurl",
        ),
    );
    files.insert(
        "/drm.mpd".to_owned(),
        (
            br#"<MPD type="static" mediaPresentationDuration="PT4S"><Period><AdaptationSet mimeType="video/mp4"><ContentProtection schemeIdUri="urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed"/><SegmentTemplate media="$Number$.m4s" duration="2"/><Representation id="v" bandwidth="1"/></AdaptationSet></Period></MPD>"#.to_vec(),
            "application/dash+xml",
        ),
    );
    files.insert(
        "/watch".to_owned(),
        (
            b"<!DOCTYPE html><html><body>video</body></html>".to_vec(),
            "text/html",
        ),
    );
    let server = serve(Arc::new(files));
    for (path, want) in [
        ("/drm.m3u8", Failure::Drm),
        ("/drm.mpd", Failure::Drm),
        ("/nothing.mp3", Failure::Status(404)),
    ] {
        let s = Stream::open(options(tmp("refused")), &server.url(path));
        let st = wait(&s, 20, |st| st.ended);
        assert_eq!(st.failure, Some(want), "{path}");
        assert!(st.chunks.is_empty());
    }
    let s = Stream::open(options(tmp("page")), &server.url("/watch"));
    let st = wait(&s, 20, |st| st.ended);
    assert!(
        matches!(st.failure, Some(Failure::Page { .. })),
        "{:?}",
        st.failure
    );
}

/// A page opened with yt-dlp turned on: the program (a fake script here: the real one is
/// never run by tests) gives the media address, which plays like any other.
#[cfg(unix)]
#[test]
fn a_page_through_yt_dlp() {
    use std::os::unix::fs::PermissionsExt;
    let data = vec![0x47u8; 188 * 100];
    let mut files = HashMap::new();
    files.insert("/media/v.ts".to_owned(), (data.clone(), "video/mp2t"));
    files.insert(
        "/watch".to_owned(),
        (
            b"<!DOCTYPE html><html><body>a talk</body></html>".to_vec(),
            "text/html",
        ),
    );
    let server = serve(Arc::new(files));
    let dir = tmp("ytdlp");
    let fake = dir.join("yt-dlp");
    let args_file = dir.join("args.txt");
    std::fs::write(
        &fake,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\necho 'A Talk'\necho '{}'\n",
            args_file.display(),
            server.url("/media/v.ts")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut o = options(dir.join("out"));
    o.ytdlp = Some(fake);
    // The page is on the local server too: it is fetched first (to tell a page from a
    // stream without an extension), then handed to yt-dlp.
    let page = server.url("/watch?v=1");
    let s = Stream::open(o, &page);
    let st = wait(&s, 20, |st| !st.chunks.is_empty());
    assert_eq!(st.title.as_deref(), Some("A Talk"), "{:?}", st.failure);
    assert_eq!(std::fs::read(&st.chunks[0].path).unwrap(), data);
    let args = std::fs::read_to_string(&args_file).unwrap();
    assert!(args.ends_with(&format!("--\n{page}\n")), "{args}");
    let asked: Vec<String> = server.log().iter().map(|r| r.path().to_owned()).collect();
    assert_eq!(asked, ["/watch", "/media/v.ts"]);
}
