//! Discovery and a whole cast against the in-process fake renderer (loopback only).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use specular_cast::session::{self, Cmd, Event, Media};
use specular_cast::testing::{FakeRenderer, Options};
use specular_cast::{Error, Found, Renderer, TransportState, device, find};

const WAIT: Duration = Duration::from_secs(10);

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/test-tmp/cast"
    ));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// A "movie" whose bytes say where they are.
fn media_file(name: &str, len: usize) -> (PathBuf, Vec<u8>) {
    let data: Vec<u8> = (0..len).map(|i| (i * 7 % 251) as u8).collect();
    let p = scratch(name);
    std::fs::write(&p, &data).unwrap();
    (p, data)
}

fn no_wake() -> find::Wake {
    Arc::new(|| {})
}

fn next_event(s: &session::Session, f: impl Fn(&Event) -> bool) -> Event {
    let t0 = std::time::Instant::now();
    while t0.elapsed() < WAIT {
        if let Some(e) = s.try_recv() {
            if f(&e) {
                return e;
            }
            if let Event::Failed(_) = e {
                return e;
            }
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    panic!("no event in time");
}

#[test]
fn finds_the_renderer() {
    let fake = FakeRenderer::start(Options {
        name: "Living Room & Co".into(),
        ..Options::default()
    })
    .unwrap();
    let f = find::find(fake.ssdp, Duration::from_millis(800), no_wake());
    let mut found = Vec::new();
    let t0 = std::time::Instant::now();
    loop {
        match f.try_recv() {
            Some(Found::Device(d)) => found.push(d),
            Some(Found::Done(e)) => {
                assert_eq!(e, None);
                break;
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
        assert!(t0.elapsed() < WAIT);
    }
    // Searched twice (UDP drops packets), found once.
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].name, "Living Room & Co");
    assert_eq!(found[0].udn, "uuid:fake-renderer-1");
    assert!(fake.state().searches >= 1);
}

#[test]
fn casts_a_file_and_drives_it() {
    let fake = FakeRenderer::start(Options::default()).unwrap();
    let d = device::fetch(&fake.location(), WAIT).unwrap();
    let (path, data) = media_file("movie.mp4", 100_000);
    let s = session::start(
        Renderer::new(d),
        Media {
            path,
            mime: "video/mp4".into(),
            title: "My <Movie>".into(),
            start_at: 42.0,
            duration: Some(216.0),
            play: true,
            speed: 1.0,
            subtitles: None,
        },
        no_wake(),
    );
    match next_event(&s, |e| matches!(e, Event::Started { .. })) {
        Event::Started { volume, subtitles } => {
            assert_eq!(volume, Some(30));
            assert!(!subtitles);
        }
        e => panic!("{e:?}"),
    }
    let st = fake.state();
    let names = st.action_names();
    assert_eq!(
        &names[..4],
        [
            "GetProtocolInfo",
            "SetAVTransportURI",
            "Play",
            "GetTransportInfo"
        ]
    );
    assert!(names.contains(&"Seek".to_string()), "{names:?}");
    let uri = st.uri.clone().unwrap();
    // Bound to the address that reaches the renderer, a random path, the file's name.
    assert!(uri.starts_with("http://127.0.0.1:"), "{uri}");
    let token = uri.split('/').nth(3).unwrap();
    assert_eq!(token.len(), 32);
    assert!(uri.ends_with("/movie.mp4"));
    let meta = st.metadata.clone().unwrap();
    assert!(
        meta.contains("<dc:title>My &lt;Movie&gt;</dc:title>"),
        "{meta}"
    );
    assert!(meta.contains("protocolInfo=\"http-get:*:video/mp4:DLNA.ORG_OP=01"));
    assert!(meta.contains("duration=\"0:03:36.000\""));
    // The renderer fetched the whole file and a range of it.
    assert!(fake.wait_for(WAIT, |s| s.fetches.len() == 2));
    let st = fake.state();
    let (whole, part) = (&st.fetches[0], &st.fetches[1]);
    assert_eq!(whole.status, 200);
    assert_eq!(whole.content_type.as_deref(), Some("video/mp4"));
    assert_eq!(whole.body.len(), data.len());
    let mismatches = whole.body.iter().zip(&data).filter(|(a, b)| a != b).count();
    assert_eq!(mismatches, 0);
    assert_eq!(part.status, 206);
    assert_eq!(part.content_range.as_deref(), Some("bytes 10-19/100000"));
    assert_eq!(part.body, data[10..20]);
    // Started where the app was.
    let at = st.now();
    assert!((42.0..50.0).contains(&at), "{at}");

    // Pause, seek, volume, mute: each reaches the renderer, and the status follows.
    s.send(Cmd::Pause);
    let e = next_event(
        &s,
        |e| matches!(e, Event::Status(st) if st.state == TransportState::Paused),
    );
    assert!(matches!(e, Event::Status(_)), "{e:?}");
    s.send(Cmd::Seek(100.0));
    let e = next_event(
        &s,
        |e| matches!(e, Event::Status(st) if st.position.is_some_and(|p| (p - 100.0).abs() < 0.5)),
    );
    match e {
        Event::Status(st) => assert_eq!(st.duration, Some(216.0)),
        e => panic!("{e:?}"),
    }
    s.send(Cmd::Volume(55));
    s.send(Cmd::Mute(true));
    assert!(fake.wait_for(WAIT, |s| s.volume == 55 && s.muted));
    s.send(Cmd::Play);
    assert!(fake.wait_for(WAIT, |s| s.transport == "PLAYING"));

    // Stop: the renderer stops and the file is no longer served.
    s.send(Cmd::Stop);
    assert_eq!(next_event(&s, |e| *e == Event::Stopped), Event::Stopped);
    assert_eq!(fake.state().transport, "STOPPED");
    let gone = specular_cast::http::request("GET", &uri, &[], &[], Duration::from_millis(500));
    assert!(gone.is_err(), "{gone:?}");
}

#[test]
fn the_server_serves_only_its_file() {
    let (path, data) = media_file("song.mp3", 5000);
    let srv =
        specular_cast::serve::FileServer::start("127.0.0.1".parse().unwrap(), &path, "audio/mpeg")
            .unwrap();
    let url = srv.url();
    let get = |u: &str, h: &[(&str, &str)]| {
        specular_cast::http::request("GET", u, h, &[], Duration::from_secs(2)).unwrap()
    };
    let r = get(&url, &[]);
    assert_eq!(r.status, 200);
    assert_eq!(r.body, data);
    assert_eq!(r.header("accept-ranges"), Some("bytes"));
    assert_eq!(r.header("transferMode.dlna.org"), Some("Streaming"));
    let r = get(&url, &[("Range", "bytes=-100")]);
    assert_eq!(r.status, 206);
    assert_eq!(r.body, data[4900..]);
    let r = get(&url, &[("Range", "bytes=6000-")]);
    assert_eq!(r.status, 416);
    let r = specular_cast::http::request("HEAD", &url, &[], &[], Duration::from_secs(2)).unwrap();
    assert_eq!((r.status, r.header("content-length")), (200, Some("5000")));
    // Another path, a guessed token, the token without the name: nothing.
    let root = format!("http://{}", specular_cast::http::authority(srv.addr()));
    for p in [
        "/",
        "/song.mp3",
        "/00000000000000000000000000000000/song.mp3",
    ] {
        assert_eq!(get(&format!("{root}{p}"), &[]).status, 404, "{p}");
    }
    let token_only = url.rsplit_once('/').unwrap().0;
    assert_eq!(get(token_only, &[]).status, 404);
    let post =
        specular_cast::http::request("POST", &url, &[], b"x", Duration::from_secs(2)).unwrap();
    assert_eq!(post.status, 405);
}

#[test]
fn refuses_a_type_the_renderer_does_not_list() {
    let fake = FakeRenderer::start(Options::default()).unwrap();
    let d = device::fetch(&fake.location(), WAIT).unwrap();
    let (path, _) = media_file("clip.webm", 1000);
    let s = session::start(
        Renderer::new(d),
        Media {
            path,
            mime: "video/webm".into(),
            title: "clip".into(),
            start_at: 0.0,
            duration: None,
            play: true,
            speed: 1.0,
            subtitles: None,
        },
        no_wake(),
    );
    assert_eq!(
        next_event(&s, |_| false),
        Event::Failed(Error::NotAccepted("video/webm".into()))
    );
    // Nothing was set or served.
    assert_eq!(fake.state().action_names(), ["GetProtocolInfo"]);
}

#[test]
fn a_refused_command_is_reported_and_the_session_goes_on() {
    let fake = FakeRenderer::start(Options {
        fail: vec!["Seek".into()],
        sink: None,
        ..Options::default()
    })
    .unwrap();
    let d = device::fetch(&fake.location(), WAIT).unwrap();
    // No ConnectionManager: the type is not checked first.
    assert!(d.connection.is_none());
    let (path, _) = media_file("tune.flac", 1000);
    let s = session::start(
        Renderer::new(d),
        Media {
            path,
            mime: "audio/flac".into(),
            title: "tune".into(),
            start_at: 0.0,
            duration: None,
            play: false,
            speed: 1.0,
            subtitles: None,
        },
        no_wake(),
    );
    assert!(matches!(
        next_event(&s, |e| matches!(e, Event::Started { .. })),
        Event::Started { .. }
    ));
    assert_eq!(fake.state().transport, "PAUSED_PLAYBACK");
    s.send(Cmd::Seek(5.0));
    assert_eq!(
        next_event(&s, |e| matches!(e, Event::Refused(..))),
        Event::Refused(Cmd::Seek(5.0), Error::Fault(501, "Action Failed".into()))
    );
    drop(s);
    assert!(fake.wait_for(WAIT, |s| s.transport == "STOPPED"));
}

/// With the no-network switch on (from the environment), neither the search nor a session
/// opens a socket: the fake sees nothing. Runs itself again in a child process, where the
/// process-wide switch is on.
#[test]
fn nothing_connects_with_the_network_off() {
    const CHILD: &str = "SPECULAR_CAST_NO_NETWORK_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "nothing_connects_with_the_network_off",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .env(specular_core::net::off::ENV, "1")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success() && text.contains("1 passed"), "{text}");
        return;
    }
    specular_core::net::off::apply_env();
    assert!(specular_core::net::off::is_off());
    let fake = FakeRenderer::start(Options::default()).unwrap();
    let f = find::find(fake.ssdp, Duration::from_millis(300), no_wake());
    let t0 = std::time::Instant::now();
    let done = loop {
        if let Some(Found::Done(e)) = f.try_recv() {
            break e;
        }
        assert!(t0.elapsed() < WAIT);
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(done, Some(Error::NetworkOff));
    assert_eq!(
        device::fetch(&fake.location(), WAIT).err(),
        Some(Error::NetworkOff)
    );
    let (path, _) = media_file("off.mp4", 10);
    assert!(
        specular_cast::serve::FileServer::start("127.0.0.1".parse().unwrap(), &path, "video/mp4")
            .is_err()
    );
    std::thread::sleep(Duration::from_millis(200));
    let st = fake.state();
    assert_eq!((st.searches, st.actions.len()), (0, 0));
}
