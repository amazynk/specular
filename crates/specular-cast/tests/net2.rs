//! Searching from each interface, the file server answering only the renderer, play speeds
//! and subtitles, against the in-process fake renderer (loopback only).

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use specular_cast::serve::FileServer;
use specular_cast::session::{self, Cmd, Event, Media};
use specular_cast::testing::{FakeRenderer, Options};
use specular_cast::{Renderer, device, find, ssdp};
use specular_core::CancelToken;

const WAIT: Duration = Duration::from_secs(10);

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/test-tmp/cast-net2"
    ));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

fn no_wake() -> find::Wake {
    Arc::new(|| {})
}

fn events_until(s: &session::Session, f: impl Fn(&Event) -> bool) -> Vec<Event> {
    let mut seen = Vec::new();
    let t0 = std::time::Instant::now();
    while t0.elapsed() < WAIT {
        match s.try_recv() {
            Some(e) => {
                let done = f(&e) || matches!(e, Event::Failed(_));
                seen.push(e);
                if done {
                    return seen;
                }
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    panic!("no event in time: {seen:?}");
}

fn movie(name: &str) -> PathBuf {
    let p = scratch(name);
    std::fs::write(&p, vec![7u8; 50_000]).unwrap();
    p
}

fn media(path: PathBuf, subtitles: Option<PathBuf>) -> Media {
    Media {
        path,
        mime: "video/mp4".into(),
        title: "Film".into(),
        start_at: 0.0,
        duration: Some(216.0),
        play: true,
        speed: 1.0,
        subtitles,
    }
}

/// A search leaves from each address given; one that is not this machine's is skipped.
#[test]
fn searches_from_each_interface() {
    let fake = FakeRenderer::start(Options::default()).unwrap();
    let mut found = Vec::new();
    ssdp::search_from(
        &[Ipv4Addr::new(192, 0, 2, 1), Ipv4Addr::LOCALHOST],
        fake.ssdp,
        Duration::from_millis(800),
        &CancelToken::new(),
        |a| found.push(a.location),
    )
    .unwrap();
    assert_eq!(found, [fake.location()]);
    assert!(fake.state().searches >= 1);
}

/// GET of `url` from this machine: the status line, or `None` when hung up on.
fn get(url: &str) -> Option<String> {
    let rest = url.strip_prefix("http://")?;
    let (host, path) = rest.split_once('/')?;
    let mut s = TcpStream::connect(host).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    write!(
        s,
        "GET /{path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    let text = String::from_utf8_lossy(&out);
    text.lines()
        .next()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
}

/// The server answers the renderer's address only (still under the random path).
#[test]
fn the_file_server_answers_only_the_renderer() {
    let p = movie("only.mp4");
    let other = FileServer::start_for(
        Ipv4Addr::LOCALHOST.into(),
        Some(Ipv4Addr::new(10, 9, 9, 9).into()),
        &[(&p, "video/mp4")],
    )
    .unwrap();
    assert_eq!(get(&other.url()), None, "another address is hung up on");
    let mine = FileServer::start_for(
        Ipv4Addr::LOCALHOST.into(),
        Some(Ipv4Addr::LOCALHOST.into()),
        &[(&p, "video/mp4")],
    )
    .unwrap();
    assert_eq!(get(&mine.url()).as_deref(), Some("HTTP/1.1 200 OK"));
    let wrong = mine.url().replace("only.mp4", "other.mp4");
    assert_eq!(get(&wrong).as_deref(), Some("HTTP/1.1 404 Not Found"));
}

/// Speeds the renderer lists are sent with Play; others leave it at 1×, said once.
#[test]
fn play_speeds() {
    let fake = FakeRenderer::start(Options {
        speeds: vec!["1".into(), "1/2".into(), "2".into()],
        ..Options::default()
    })
    .unwrap();
    let d = device::fetch(&fake.location(), WAIT).unwrap();
    assert!(d.av_transport.scpd.is_some());
    let s = session::start(Renderer::new(d), media(movie("speed.mp4"), None), no_wake());
    events_until(&s, |e| matches!(e, Event::Status(_)));
    s.send(Cmd::Speed(2.0));
    let ev = events_until(&s, |e| matches!(e, Event::Speed(_)));
    assert_eq!(ev.last(), Some(&Event::Speed(2.0)));
    assert!(fake.wait_for(WAIT, |s| s.speed == "2"));
    s.send(Cmd::Speed(0.5));
    events_until(&s, |e| matches!(e, Event::Speed(_)));
    assert!(fake.wait_for(WAIT, |s| s.speed == "1/2"));

    let plain = FakeRenderer::start(Options::default()).unwrap();
    let d = device::fetch(&plain.location(), WAIT).unwrap();
    let s = session::start(
        Renderer::new(d),
        media(movie("speed1.mp4"), None),
        no_wake(),
    );
    events_until(&s, |e| matches!(e, Event::Status(_)));
    s.send(Cmd::Speed(1.5));
    events_until(&s, |e| *e == Event::NormalSpeedOnly);
    s.send(Cmd::Speed(2.0));
    s.send(Cmd::Pause);
    std::thread::sleep(Duration::from_millis(1500));
    let mut again = 0;
    while let Some(e) = s.try_recv() {
        again += usize::from(e == Event::NormalSpeedOnly);
    }
    assert_eq!(again, 0, "said once");
    assert_eq!(plain.state().speed, "1");
}

/// Subtitles go as `sec:CaptionInfoEx` to a renderer announcing Samsung's capabilities (or
/// listing a subtitle type), served beside the file; not to one that does neither.
#[test]
fn subtitles_go_to_renderers_that_take_them() {
    let srt_path = scratch("film.srt");
    let srt = b"1\r\n00:00:01,000 --> 00:00:02,500\r\nHello\r\n\r\n".to_vec();
    std::fs::write(&srt_path, &srt).unwrap();
    for (opts, takes) in [
        (
            Options {
                sec: true,
                ..Options::default()
            },
            true,
        ),
        (
            Options {
                sink: Some("http-get:*:video/mp4:*,http-get:*:text/srt:*".into()),
                ..Options::default()
            },
            true,
        ),
        (Options::default(), false),
    ] {
        let sec = opts.sec;
        let fake = FakeRenderer::start(opts).unwrap();
        let d = device::fetch(&fake.location(), WAIT).unwrap();
        assert_eq!(d.sec_captions, sec);
        let s = session::start(
            Renderer::new(d),
            media(movie("subs.mp4"), Some(srt_path.clone())),
            no_wake(),
        );
        let ev = events_until(&s, |e| matches!(e, Event::Started { .. }));
        assert_eq!(
            ev.last(),
            Some(&Event::Started {
                volume: Some(30),
                subtitles: takes
            })
        );
        let meta = fake.state().metadata.unwrap_or_default();
        assert_eq!(meta.contains("sec:CaptionInfoEx"), takes, "{meta}");
        assert_eq!(meta.contains("text/srt"), takes);
        if sec {
            assert!(fake.wait_for(WAIT, |s| s.fetches.len() == 3));
            let f = &fake.state().fetches[2];
            assert_eq!(f.status, 200);
            assert_eq!(f.content_type.as_deref(), Some("text/srt"));
            let mismatches = f.body.iter().zip(&srt).filter(|(a, b)| a != b).count()
                + f.body.len().abs_diff(srt.len());
            eprintln!(
                "subtitles served: {} bytes, {mismatches} mismatches",
                f.body.len()
            );
            assert_eq!(mismatches, 0);
        }
    }
}

/// A subtitle delay changed while casting: the new SubRip is served at a new address and the
/// file loaded again where it was, paused as it was; the device reads the new timing.
#[test]
fn changed_subtitles_are_sent_again() {
    let first = scratch("again.srt");
    std::fs::write(&first, b"1\r\n00:00:40,000 --> 00:00:41,000\r\nHi\r\n\r\n").unwrap();
    let fake = FakeRenderer::start(Options {
        sec: true,
        ..Options::default()
    })
    .unwrap();
    let d = device::fetch(&fake.location(), WAIT).unwrap();
    let s = session::start(
        Renderer::new(d),
        media(movie("again.mp4"), Some(first)),
        no_wake(),
    );
    events_until(&s, |e| matches!(e, Event::Started { .. }));
    assert!(fake.wait_for(WAIT, |s| s.fetches.len() == 3));
    let old_uri = fake.state().uri.unwrap();
    s.send(Cmd::Seek(30.0));
    s.send(Cmd::Pause);
    events_until(&s, |e| {
        matches!(e, Event::Status(st) if st.position.is_some_and(|p| p >= 30.0)
            && st.state == specular_cast::TransportState::Paused)
    });
    // 500 ms later.
    let delayed = b"1\r\n00:00:40,500 --> 00:00:41,500\r\nHi\r\n\r\n".to_vec();
    let second = scratch("again-2.srt");
    std::fs::write(&second, &delayed).unwrap();
    s.send(Cmd::Subtitles(Some(second)));
    let ev = events_until(&s, |e| matches!(e, Event::Subtitles(_)));
    assert_eq!(ev.last(), Some(&Event::Subtitles(true)));
    assert!(fake.wait_for(WAIT, |s| s.fetches.len() == 6));
    let st = fake.state();
    let uri = st.uri.clone().unwrap();
    assert_ne!(uri, old_uri, "a new address, read afresh");
    let f = &st.fetches[5];
    assert_eq!((f.status, f.body.as_slice()), (200, delayed.as_slice()));
    assert_eq!(st.transport, "PAUSED_PLAYBACK", "paused as it was");
    assert!((st.now() - 30.0).abs() < 1.5, "where it was: {}", st.now());
    // The old address is no longer served.
    assert!(get(&old_uri).is_none());
    // No subtitles: loaded again without them.
    s.send(Cmd::Subtitles(None));
    let ev = events_until(&s, |e| matches!(e, Event::Subtitles(_)));
    assert_eq!(ev.last(), Some(&Event::Subtitles(false)));
    let meta = fake.state().metadata.unwrap_or_default();
    assert!(!meta.contains("CaptionInfoEx"), "{meta}");
}
