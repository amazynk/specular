//! HLS playlists that only grow (`EXT-X-PLAYLIST-TYPE:EVENT`: a recording while it is made),
//! and seeks after the last piece of a finite one. From an in-process server on 127.0.0.1;
//! the segments are stand-ins (MPEG-TS sync bytes and the segment's number), which the
//! worker joins into pieces without reading them.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use specular_remote::testing::{ServerOptions, TestResponse, TestServer};
use specular_remote::{Client, ClientConfig};
use specular_stream::{Options, Status, Stream, StreamKind};

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("growing")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn options(name: &str) -> Options {
    let client = Client::new(ClientConfig::for_tests()).unwrap();
    let mut o = Options::new(tmp(name), client);
    o.piece_s = 4.0;
    o
}

/// Segment `n`: 188 bytes, a sync byte then its number.
fn segment(n: usize) -> Vec<u8> {
    let mut v = vec![n as u8; 188];
    v[0] = 0x47;
    v
}

/// The numbers of the segments in the piece at `path`.
fn segments_in(path: &Path) -> Vec<usize> {
    std::fs::read(path)
        .unwrap()
        .chunks(188)
        .map(|c| usize::from(c[1]))
        .collect()
}

/// A playlist of 2 s segments `0..n`, of `kind` (`EVENT`, `VOD` or none), with `extra` tags.
fn playlist(n: usize, kind: &str, extra: &str, ended: bool) -> String {
    let mut s = String::from("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n");
    if !kind.is_empty() {
        s.push_str(&format!("#EXT-X-PLAYLIST-TYPE:{kind}\n"));
    }
    s.push_str(extra);
    s.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
    for i in 0..n {
        s.push_str(&format!("#EXTINF:2.0,\nseg{i}.ts\n"));
    }
    if ended {
        s.push_str("#EXT-X-ENDLIST\n");
    }
    s
}

/// Serves a recording of which `have` segments exist so far (`total` once it has ended).
fn serve(have: Arc<AtomicUsize>, total: usize, extra: &'static str) -> TestServer {
    TestServer::start(ServerOptions::default(), move |r| {
        let n = have.load(Ordering::Relaxed);
        let path = r.path();
        if path == "/rec.m3u8" {
            return TestResponse::ok(playlist(n, "EVENT", extra, n >= total))
                .header("Content-Type", "application/vnd.apple.mpegurl");
        }
        match path
            .strip_prefix("/seg")
            .and_then(|s| s.strip_suffix(".ts"))
            .and_then(|s| s.parse::<usize>().ok())
        {
            Some(i) if i < n => TestResponse::ok(segment(i)).header("Content-Type", "video/mp2t"),
            _ => TestResponse::status(404),
        }
    })
    .unwrap()
}

fn wait(s: &Stream, secs: u64, done: impl Fn(&Status) -> bool) -> Status {
    let t0 = Instant::now();
    loop {
        let st = s.status();
        if done(&st) || st.failure.is_some() || t0.elapsed() > Duration::from_secs(secs) {
            return st;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Every segment in the pieces of `st`, in order.
fn played(st: &Status) -> Vec<usize> {
    st.chunks
        .iter()
        .flat_map(|c| segments_in(&c.path))
        .collect()
}

/// A recording that says where to start (`EXT-X-START`, here its beginning) plays from
/// there, like a finite stream whose length grows: each new segment becomes a piece as the
/// playlist is read again, the length follows, and when it ends the stream is a finite one.
#[test]
fn a_growing_playlist_plays_from_its_start_and_follows_its_length() {
    let have = Arc::new(AtomicUsize::new(3));
    let server = serve(have.clone(), 8, "#EXT-X-START:TIME-OFFSET=0\n");
    let mut o = options("from-start");
    o.ahead_s = 1e9;
    let s = Stream::open(o, &server.url("/rec.m3u8"));
    let st = wait(&s, 20, |st| played(st).len() == 3);
    assert_eq!(st.failure, None);
    assert_eq!(st.kind, Some(StreamKind::Hls));
    assert!(st.growing && !st.live && !st.ended, "{st:?}");
    assert_eq!(st.duration, Some(6.0));
    assert_eq!(played(&st), [0, 1, 2]);
    assert_eq!(st.chunks[0].start, 0.0);
    // It grows: the new segments come as pieces, and the length says so.
    have.store(5, Ordering::Relaxed);
    let st = wait(&s, 20, |st| played(st).len() == 5);
    assert_eq!(played(&st), [0, 1, 2, 3, 4], "{:?}", st.failure);
    let st = wait(&s, 20, |st| st.duration == Some(10.0));
    assert!(st.growing && st.duration == Some(10.0), "{st:?}");
    let last = st.chunks.last().unwrap();
    assert_eq!(last.start + last.seconds, 10.0);
    // A seek back into what was recorded is answered while it still grows (by a piece
    // made for it: one kept from before may hold the time too).
    let playing = st.chunks.last().unwrap().index;
    s.playing(playing);
    s.seek(2.5);
    let st = wait(&s, 20, |st| st.seek.is_some() && s.chunk_at(2.5).is_some());
    let c = s.chunk_at(2.5).unwrap_or_else(|| panic!("{st:?}"));
    assert_eq!(Some(c.index), st.seek.map(|x| x.1));
    assert_eq!((c.start, segments_in(&c.path)[0]), (2.0, 1));
    // It ends: the rest comes, it is no longer growing, and no more pieces will come.
    have.store(8, Ordering::Relaxed);
    let st = wait(&s, 20, |st| st.ended);
    assert!(st.ended && !st.growing && st.failure.is_none(), "{st:?}");
    assert_eq!(st.duration, Some(16.0));
    let from_seek: Vec<usize> = st
        .chunks
        .iter()
        .filter(|k| k.index >= c.index)
        .flat_map(|k| segments_in(&k.path))
        .collect();
    assert_eq!(from_seek, [1, 2, 3, 4, 5, 6, 7]);
}

/// Without `EXT-X-START` a recording is joined near its end, as a live stream is, with all
/// of it to seek back in: the piece's place in the stream is its segment's.
#[test]
fn a_growing_playlist_is_joined_near_its_end_and_seeks_back() {
    let have = Arc::new(AtomicUsize::new(10));
    let server = serve(have.clone(), 12, "");
    let mut o = options("near-end");
    o.ahead_s = 1e9;
    let s = Stream::open(o, &server.url("/rec.m3u8"));
    let st = wait(&s, 20, |st| played(st).len() == 3);
    assert_eq!(st.failure, None);
    assert!(st.growing && !st.live, "{st:?}");
    assert_eq!(played(&st), [7, 8, 9], "three segments from the end");
    assert_eq!(st.chunks[0].start, 14.0);
    assert_eq!(st.duration, Some(20.0));
    s.playing(st.chunks[0].index);
    s.seek(0.0);
    let st = wait(&s, 20, |st| st.seek.is_some() && s.chunk_at(0.0).is_some());
    let c = s.chunk_at(0.0).unwrap_or_else(|| panic!("{st:?}"));
    assert_eq!((c.start, segments_in(&c.path)[0]), (0.0, 0));
}

/// A playlist with no segments yet (the recording has just begun) is waited for.
#[test]
fn a_growing_playlist_without_segments_yet_is_waited_for() {
    let have = Arc::new(AtomicUsize::new(0));
    let server = serve(have.clone(), 4, "");
    let s = Stream::open(options("empty"), &server.url("/rec.m3u8"));
    let st = wait(&s, 2, |st| !st.chunks.is_empty());
    assert!(
        st.chunks.is_empty() && st.failure.is_none() && !st.ended,
        "{st:?}"
    );
    assert!(st.growing);
    have.store(2, Ordering::Relaxed);
    let st = wait(&s, 20, |st| played(st).len() == 2);
    assert_eq!(played(&st), [0, 1], "{st:?}");
}

/// A finite stream whose pieces were all made still answers a seek: back to a piece
/// deleted since (the ones before the one playing go).
#[test]
fn a_finite_stream_answers_a_seek_after_its_last_piece() {
    let server = TestServer::start(ServerOptions::default(), |r| {
        let path = r.path();
        if path == "/vod.m3u8" {
            return TestResponse::ok(playlist(6, "VOD", "", true));
        }
        match path
            .strip_prefix("/seg")
            .and_then(|s| s.strip_suffix(".ts"))
            .and_then(|s| s.parse::<usize>().ok())
        {
            Some(i) => TestResponse::ok(segment(i)),
            None => TestResponse::status(404),
        }
    })
    .unwrap();
    let mut o = options("late-seek");
    o.ahead_s = 1e9;
    let s = Stream::open(o, &server.url("/vod.m3u8"));
    let st = wait(&s, 20, |st| st.ended);
    assert!(st.ended && st.failure.is_none() && !st.growing, "{st:?}");
    assert_eq!(played(&st), [0, 1, 2, 3, 4, 5]);
    assert_eq!(st.duration, Some(12.0));
    // The last piece plays: the earlier ones are deleted.
    let last = st.chunks.last().unwrap().index;
    s.playing(last);
    assert!(s.chunk_at(0.5).is_none());
    s.seek(0.5);
    let st = wait(&s, 20, |st| st.seek.is_some() && s.chunk_at(0.5).is_some());
    let c = s.chunk_at(0.5).unwrap_or_else(|| panic!("{st:?}"));
    assert_eq!((c.start, segments_in(&c.path)[0]), (0.0, 0));
    // And it says again that no more pieces will come, once they are made.
    let st = wait(&s, 20, |st| st.ended);
    assert!(st.ended && st.failure.is_none(), "{st:?}");
}
