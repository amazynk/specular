//! A web file played while it is fetched by byte ranges, from an in-process server on
//! 127.0.0.1 (nothing else is contacted).
//!
//! The oracle: a movie made by ffmpeg (a test tool only; skipped when it is not installed);
//! every sample read through the file being filled must equal the original's.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_remote::testing::{ServerOptions, TestRequest, TestResponse, TestServer};
use specular_remote::{Client, ClientConfig};
use specular_stream::{Options, Status, Stream, StreamKind};

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("stream-webfile")
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

fn options(dir: PathBuf) -> Options {
    let client = Client::new(ClientConfig::for_tests()).unwrap();
    let mut o = Options::new(dir, client);
    o.ahead_s = 1e9;
    o
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

/// A movie of a few megabytes with its index at the end (ffmpeg's default), made once.
fn big_movie() -> Option<&'static PathBuf> {
    static BIG: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    BIG.get_or_init(|| {
        if !have_ffmpeg() {
            return None;
        }
        let dir = tmp("media");
        let out = Command::new("ffmpeg")
            .current_dir(&dir)
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args([
                "-f",
                "lavfi",
                "-i",
                "testsrc2=rate=25:size=640x360:duration=20",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=330:duration=20",
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-bf",
                "2",
                "-b:v",
                "2500k",
                "-g",
                "25",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "96k",
                "-shortest",
                "big.mp4",
            ])
            .output()
            .expect("ffmpeg runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        Some(dir.join("big.mp4"))
    })
    .as_ref()
}

/// Serves one file with byte ranges (and `Accept-Ranges`) on every answer; `refuse`: ranges
/// starting there get a 503 (a server that stops sending part of it).
fn ranged_server(
    data: Arc<Vec<u8>>,
    bandwidth: Option<u64>,
    refuse: Option<std::ops::Range<u64>>,
) -> TestServer {
    TestServer::start(
        ServerOptions {
            bandwidth,
            ..ServerOptions::default()
        },
        move |r: &TestRequest| {
            if !r.path().starts_with("/v/big.") {
                return TestResponse::status(404);
            }
            let (start, end) = r
                .header("range")
                .and_then(|v| v.trim().strip_prefix("bytes="))
                .and_then(|v| v.split_once('-'))
                .map_or((None, None), |(a, b)| {
                    (a.parse::<u64>().ok(), b.parse::<u64>().ok())
                });
            if let (Some(no), Some(s)) = (&refuse, start) {
                if no.contains(&s) {
                    return TestResponse::status(503);
                }
                // A range running into the refused part stops short of it.
                let end = end.unwrap_or(data.len() as u64 - 1);
                if s < no.start && end >= no.start {
                    return TestResponse::new(206, data[s as usize..no.start as usize].to_vec())
                        .header(
                            "Content-Range",
                            format!("bytes {s}-{}/{}", no.start - 1, data.len()),
                        )
                        .header("Content-Type", "video/mp4");
                }
            }
            TestResponse::ranged(r, &data, "\"b\"").header("Content-Type", "video/mp4")
        },
    )
    .unwrap()
}

/// Every sample of each track of the movie in `data`: (picture, sound).
type Samples = (Vec<Option<Vec<u8>>>, Vec<Option<Vec<u8>>>);

fn all_samples(data: &[u8]) -> Samples {
    let m = specular_video::parse_movie(data, true).expect("the index is there");
    let v = m.video.as_ref().unwrap();
    let a = m.audio.as_ref().unwrap();
    (
        v.samples
            .iter()
            .map(|s| v.sample_data(data, s).map(|b| b.into_owned()))
            .collect(),
        a.samples
            .iter()
            .map(|s| a.sample_data(data, s).map(|b| b.into_owned()))
            .collect(),
    )
}

fn mismatches(got: &[Option<Vec<u8>>], want: &[Option<Vec<u8>>]) -> usize {
    got.iter()
        .zip(want)
        .filter(|(g, w)| g.is_none() || g != w)
        .count()
        + got.len().abs_diff(want.len())
}

/// A web file whose server takes ranges plays before it is all fetched: the index at the
/// end comes first, a read far ahead (a seek) waits for its bytes and gets them, and every
/// sample matches the original's.
#[test]
fn a_web_file_plays_while_fetched_and_seeks_by_ranges() {
    let Some(movie) = big_movie() else {
        eprintln!("ffmpeg not installed: skipped");
        return;
    };
    let data = Arc::new(std::fs::read(movie).unwrap());
    let total = data.len() as u64;
    let want = all_samples(&data);
    // About 4 s for the whole file.
    let server = ranged_server(Arc::clone(&data), Some(total / 4), None);
    let s = Stream::open(options(tmp("ranged")), &server.url("/v/big.mp4"));
    let t0 = Instant::now();
    let st = wait(&s, 30, |st| !st.chunks.is_empty());
    let offered = t0.elapsed();
    assert_eq!(st.kind, Some(StreamKind::File), "{:?}", st.failure);
    assert!(st.ranged);
    assert!(
        st.received < total / 2,
        "offered after {} of {total} bytes",
        st.received
    );
    let src = specular_core::Source::open(&st.chunks[0].path).unwrap();
    let b = src.bytes();
    let m = specular_video::parse_movie(b, true).unwrap();
    let v = m.video.as_ref().unwrap();
    // A seek near the end: its sample waits for its bytes, then matches.
    let late = v.samples.len() * 9 / 10;
    let t1 = Instant::now();
    let got = v.sample_data(b, &v.samples[late]).map(|b| b.into_owned());
    let seek_wait = t1.elapsed();
    assert_eq!(got, want.0[late], "the sample after a seek");
    let after_seek = s.status().received;
    assert!(
        after_seek < total,
        "the seek did not wait for the whole file"
    );
    let (gv, ga) = all_samples(b);
    let (bv, ba) = (mismatches(&gv, &want.0), mismatches(&ga, &want.1));
    eprintln!(
        "web file by ranges: {total} bytes; offered after {:.0} ms ({} bytes); a seek to 90 % read in {:.0} ms ({after_seek} bytes fetched then); picture {}/{} samples, {bv} mismatches; sound {}/{}, {ba} mismatches",
        offered.as_secs_f64() * 1e3,
        st.received,
        seek_wait.as_secs_f64() * 1e3,
        gv.len(),
        want.0.len(),
        ga.len(),
        want.1.len()
    );
    assert_eq!((bv, ba), (0, 0));
    let st = wait(&s, 30, |st| st.ended);
    assert!(st.failure.is_none(), "{:?}", st.failure);
    assert_eq!(st.chunks.len(), 1);
    assert_eq!(std::fs::read(&st.chunks[0].path).unwrap(), *data);
}

/// A part that never comes: the reader gives up after the timeout (no zeros read), and the
/// stream says it stalled.
#[test]
fn a_missing_part_times_out() {
    let Some(movie) = big_movie() else {
        eprintln!("ffmpeg not installed: skipped");
        return;
    };
    let data = Arc::new(std::fs::read(movie).unwrap());
    let m = specular_video::parse_movie(&data, true).unwrap();
    let v = m.video.as_ref().unwrap();
    let mid = v.samples[v.samples.len() / 2].offset;
    // The first bytes and the index (at the end) come; from the middle to the index nothing
    // does.
    let moov_at = data.windows(4).rposition(|w| w == b"moov").unwrap() as u64 - 4;
    let server = ranged_server(Arc::clone(&data), None, Some(mid - 4096..moov_at));
    let mut o = options(tmp("stall"));
    o.read_timeout = Duration::from_millis(600);
    let s = Stream::open(o, &server.url("/v/big.mp4"));
    let st = wait(&s, 30, |st| !st.chunks.is_empty());
    assert!(st.ranged, "{:?}", st.failure);
    let src = specular_core::Source::open(&st.chunks[0].path).unwrap();
    let b = src.bytes();
    let pm = specular_video::parse_movie(b, true).unwrap();
    let pv = pm.video.as_ref().unwrap();
    let t0 = Instant::now();
    let smp = &pv.samples[pv.samples.len() / 2];
    let got = pv.sample_data(b, smp);
    assert!(got.is_none(), "a hole is never read as zeros");
    assert!(t0.elapsed() < Duration::from_secs(10));
    assert!(s.stalled());
    assert!(!s.buffering());
}

/// A Matroska movie from ffmpeg: with cues (a file, cues at the end) or without (written to
/// a pipe: clusters of unknown size, no cues, no duration).
fn mkv_movie(cues: bool) -> Option<Vec<u8>> {
    if !have_ffmpeg() {
        return None;
    }
    // One folder per call: tests run in parallel and `tmp` empties the folder first.
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = tmp(&format!(
        "{}-{n}",
        if cues { "mkv-cues" } else { "mkv-pipe" }
    ));
    let mut c = Command::new("ffmpeg");
    c.current_dir(&dir)
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args([
            "-f",
            "lavfi",
            "-i",
            "testsrc2=rate=25:size=320x180:duration=24",
        ])
        .args(["-f", "lavfi", "-i", "sine=frequency=330:duration=24"])
        .args(["-c:v", "libx264", "-preset", "veryfast", "-bf", "2"])
        .args(["-b:v", "1200k", "-g", "25", "-pix_fmt", "yuv420p"])
        .args(["-c:a", "aac", "-b:a", "96k", "-shortest", "-f", "matroska"]);
    c.arg(if cues { "big.mkv" } else { "pipe:1" });
    let out = c.output().expect("ffmpeg runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(if cues {
        std::fs::read(dir.join("big.mkv")).unwrap()
    } else {
        out.stdout
    })
}

/// Every sample of each track, with its time in the whole stream (µs).
type Timed = Vec<(i64, Vec<u8>)>;

fn timed(data: &[u8], offset_us: i64) -> (Timed, Timed) {
    let m = specular_video::parse_movie(data, true).expect("a movie");
    let v = m.video.as_ref().unwrap();
    let a = m.audio.as_ref().unwrap();
    let pv = v
        .samples
        .iter()
        .map(|s| {
            (
                v.us(s.cts) + offset_us,
                v.sample_data(data, s).unwrap().into_owned(),
            )
        })
        .collect();
    let pa = a
        .samples
        .iter()
        .map(|s| {
            (
                a.us(s.cts) + offset_us,
                a.sample_data(data, s).unwrap().into_owned(),
            )
        })
        .collect();
    (pv, pa)
}

/// Samples of the pieces against the whole file's: bytes equal and times within 1 ms.
fn timed_mismatches(got: &Timed, want: &Timed) -> usize {
    if let Some((i, (g, w))) = got
        .iter()
        .zip(want)
        .enumerate()
        .find(|(_, (g, w))| g.1 != w.1 || (g.0 - w.0).abs() > 1000)
    {
        eprintln!(
            "first mismatch at {i}: {} µs ({} bytes) vs {} µs ({} bytes)",
            g.0,
            g.1.len(),
            w.0,
            w.1.len()
        );
    }
    got.iter()
        .zip(want)
        .filter(|(g, w)| g.1 != w.1 || (g.0 - w.0).abs() > 1000)
        .count()
        + got.len().abs_diff(want.len())
}

/// A Matroska web file plays in pieces while it is fetched by ranges: offered after a
/// small part, every sample of the pieces is the file's (bytes and times); with cues a
/// seek near the end is answered without fetching the rest, without cues the pieces come
/// as the clusters are read (unknown sizes too).
#[test]
fn a_matroska_web_file_plays_in_pieces() {
    for cues in [true, false] {
        let Some(movie) = mkv_movie(cues) else {
            eprintln!("ffmpeg not installed: skipped");
            return;
        };
        let data = Arc::new(movie);
        let total = data.len() as u64;
        let (wv, wa) = timed(&data, 0);
        // About 6 s for the whole file.
        let server = ranged_server(Arc::clone(&data), Some(total / 6), None);
        let mut o = options(tmp(if cues { "mkv-r-cues" } else { "mkv-r-pipe" }));
        o.piece_s = 4.0;
        let s = Stream::open(o, &server.url("/v/big.mkv"));
        let t0 = Instant::now();
        let st = wait(&s, 30, |st| !st.chunks.is_empty());
        let offered = t0.elapsed();
        assert_eq!(st.kind, Some(StreamKind::Pieces), "{:?}", st.failure);
        assert!(
            st.received < total / 2,
            "offered after {} of {total}",
            st.received
        );
        let offered_bytes = st.received;
        // All pieces, in order (nothing plays, so none is deleted).
        let st = wait(&s, 60, |st| st.ended);
        assert!(st.failure.is_none(), "{:?}", st.failure);
        let seek_ms;
        if cues {
            assert!(
                (st.duration.unwrap() - 24.0).abs() < 0.2,
                "{:?}",
                st.duration
            );
            // Another stream of it: a seek near the end at once; its piece comes before
            // the rest of the file.
            let s2 = Stream::open(
                {
                    let mut o = options(tmp("mkv-r-seek"));
                    o.piece_s = 4.0;
                    o
                },
                &server.url("/v/big.mkv"),
            );
            wait(&s2, 30, |st| !st.chunks.is_empty());
            let t1 = Instant::now();
            s2.seek(21.0);
            let st2 = wait(&s2, 30, |st| {
                st.seek.is_some() && s2.chunk_at(21.0).is_some()
            });
            seek_ms = Some(t1.elapsed().as_secs_f64() * 1e3);
            assert!(s2.chunk_at(21.0).is_some(), "{:?}", st2.failure);
            assert!(st2.received < total, "the seek fetched only its piece");
        } else {
            // Another stream of it: once three pieces were read, a seek back to 1 s opens
            // the first piece again; a seek past what was read waits for the reading.
            let mut o = options(tmp("mkv-r-back"));
            o.piece_s = 4.0;
            let s3 = Stream::open(o, &server.url("/v/big.mkv"));
            wait(&s3, 30, |st| st.chunks.len() >= 3);
            let t1 = Instant::now();
            s3.seek(1.0);
            let st3 = wait(&s3, 30, |st| {
                st.seek.is_some() && s3.chunk_at(1.0).is_some()
            });
            seek_ms = Some(t1.elapsed().as_secs_f64() * 1e3);
            let c = s3.chunk_at(1.0).expect("the first piece again");
            assert_eq!(c.start, 0.0, "{:?}", st3.failure);
            s3.seek(22.0);
            wait(&s3, 30, |st| {
                st.seek.is_some() && s3.chunk_at(22.0).is_some()
            });
            assert!(s3.chunk_at(22.0).is_some(), "read on to it");
        }
        let pieces = st.chunks.clone();
        let (mut gv, mut ga) = (Timed::new(), Timed::new());
        for c in &pieces {
            let b = std::fs::read(&c.path).unwrap();
            let (v, a) = timed(&b, (c.start * 1e6).round() as i64);
            gv.extend(v);
            ga.extend(a);
        }
        // The pieces' sound starts at their cut: the whole file's first sample is kept.
        let (bv, ba) = (timed_mismatches(&gv, &wv), timed_mismatches(&ga, &wa));
        eprintln!(
            "ORACLE\tStream/Matroska pieces\t{}\t{}\t{total} bytes, offered after {:.0} ms ({} bytes), {} pieces, seek (21 s with cues, back to 1 s without) {:?} ms; picture {}/{} samples, {bv} mismatches; sound {}/{}, {ba} mismatches",
            if cues { "with cues" } else { "piped, no cues" },
            if (bv, ba) == (0, 0) { "pass" } else { "fail" },
            offered.as_secs_f64() * 1e3,
            offered_bytes,
            pieces.len(),
            seek_ms.map(|m| m.round()),
            gv.len(),
            wv.len(),
            ga.len(),
            wa.len()
        );
        assert_eq!((bv, ba), (0, 0));
        // At every join the sound lines up with the picture within one frame.
        let paths: Vec<PathBuf> = pieces.iter().map(|c| c.path.clone()).collect();
        let off = join_offsets(&paths);
        let worst = off.iter().fold(0f64, |m, v| m.max(v.abs()));
        eprintln!(
            "ORACLE\tStream/A-V at Matroska joins\t{}\t{}\t{} joins, sound later than picture (ms): {:?}",
            if cues { "with cues" } else { "piped, no cues" },
            if worst <= 40.0 { "pass" } else { "fail" },
            off.len(),
            off.iter().map(|v| v.round() as i64).collect::<Vec<_>>()
        );
        assert!(worst <= 40.0, "{off:?}");
        if !cues {
            // Without a duration in the file: as long as it was read, to its last frame's
            // end; the last piece ends there too.
            assert!(
                (st.duration.unwrap() - 24.0).abs() < 0.15,
                "{:?}",
                st.duration
            );
            let last = pieces.last().unwrap();
            assert!(
                (last.start + last.seconds - st.duration.unwrap()).abs() < 1e-6,
                "{last:?}"
            );
        }
    }
}

/// Without cues but with its duration (a file whose Cues were renamed away): the last piece
/// ends at the file's end, each piece where the next starts; once every piece is made and
/// the worker idles, a seek is still answered, and it finishes again.
#[test]
fn matroska_last_piece_ends_at_the_end_and_a_late_seek_is_answered() {
    let Some(mut movie) = mkv_movie(true) else {
        eprintln!("ffmpeg not installed: skipped");
        return;
    };
    // The Cues element (at the end; the SeekHead names it too, earlier): another id.
    let at = movie
        .windows(4)
        .rposition(|w| w == [0x1C, 0x53, 0xBB, 0x6B])
        .unwrap();
    movie[at + 3] = 0x6C;
    let data = Arc::new(movie);
    let server = ranged_server(Arc::clone(&data), None, None);
    let mut o = options(tmp("mkv-r-end"));
    o.piece_s = 4.0;
    let s = Stream::open(o, &server.url("/v/big.mkv"));
    let st = wait(&s, 60, |st| st.ended);
    assert!(st.failure.is_none(), "{:?}", st.failure);
    assert_eq!(st.kind, Some(StreamKind::Pieces));
    assert!(st.chunks.len() >= 5, "{:?}", st.chunks);
    for w in st.chunks.windows(2) {
        assert!(
            (w[0].start + w[0].seconds - w[1].start).abs() < 1e-6,
            "{w:?}"
        );
    }
    let last = st.chunks.last().unwrap();
    assert!(
        (last.start + last.seconds - 24.0).abs() < 0.05,
        "the file's own end: {last:?}"
    );
    // The worker made everything; a seek now opens the piece holding the time.
    let t0 = Instant::now();
    s.seek(5.0);
    // (Not `wait`: it stops at `ended`, which holds until the worker takes the seek.)
    let answered = loop {
        let st = s.status();
        if st.seek.is_some_and(|(t, _)| t == 5.0) && s.chunk_at(5.0).is_some() {
            break true;
        }
        if t0.elapsed() > Duration::from_secs(30) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(answered, "{:?}", s.status().failure);
    let c = s.chunk_at(5.0).unwrap();
    assert!(c.start <= 5.0 && c.start + c.seconds > 5.0, "{c:?}");
    eprintln!(
        "seek after the end answered in {:.0} ms",
        t0.elapsed().as_secs_f64() * 1e3
    );
    let st3 = wait(&s, 60, |st| st.ended);
    assert!(st3.ended && st3.failure.is_none(), "{:?}", st3.failure);
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
