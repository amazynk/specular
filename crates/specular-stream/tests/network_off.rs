//! With the "No network" switch on, a stream sends nothing (its own process: the switch is
//! process-wide).

use std::time::{Duration, Instant};

use specular_remote::testing::{ServerOptions, TestResponse, TestServer};
use specular_remote::{Client, ClientConfig};
use specular_stream::{Failure, Options, Stream};

#[test]
fn nothing_is_sent_with_the_network_off() {
    let server =
        TestServer::start(ServerOptions::default(), |_| TestResponse::ok(vec![0; 10])).unwrap();
    specular_core::net::off::set(specular_core::net::off::By::Flag, true);
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("stream-off");
    let client = Client::new(ClientConfig::for_tests()).unwrap();
    let s = Stream::open(Options::new(dir, client), &server.url("/live.m3u8"));
    let t0 = Instant::now();
    while !s.status().ended && t0.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(s.status().failure, Some(Failure::NetworkOff));
    assert_eq!(server.requests(), 0);
}

/// A stream served on this computer itself is no network: with the switch on, a client made
/// for it (`with_loopback`, as the player's is) plays it, and still asks nothing else.
#[test]
fn a_stream_from_this_computer_plays_with_the_network_off() {
    let server = TestServer::start(ServerOptions::default(), |r| match r.path() {
        "/rec/index.m3u8" => TestResponse::ok(
            "#EXTM3U\n#EXT-X-TARGETDURATION:2\n#EXTINF:2,\ns0.ts\n#EXTINF:2,\ns1.ts\n#EXT-X-ENDLIST\n",
        )
        .header("Content-Type", "application/vnd.apple.mpegurl"),
        // A playlist here whose segments are somewhere else.
        "/away/index.m3u8" => TestResponse::ok(
            "#EXTM3U\n#EXT-X-TARGETDURATION:2\n#EXTINF:2,\nhttp://192.0.2.1/s0.ts\n#EXT-X-ENDLIST\n",
        ),
        "/rec/s0.ts" => TestResponse::ok(vec![0x47; 188]),
        "/rec/s1.ts" => TestResponse::ok(vec![0x47; 376]),
        _ => TestResponse::status(404),
    })
    .unwrap();
    specular_core::net::off::set(specular_core::net::off::By::Flag, true);
    let tmp = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    let client = Client::new(ClientConfig::for_tests())
        .unwrap()
        .with_loopback();
    let wait = |s: &Stream| {
        let t0 = Instant::now();
        while !s.status().ended && t0.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(10));
        }
        s.status()
    };
    let s = Stream::open(
        Options::new(tmp.join("stream-off-local"), client.clone()),
        &server.url("/rec/index.m3u8"),
    );
    let st = wait(&s);
    assert_eq!(st.failure, None);
    let bytes: usize = st
        .chunks
        .iter()
        .map(|c| std::fs::read(&c.path).unwrap().len())
        .sum();
    assert_eq!(bytes, 188 + 376, "both segments, as pieces");
    // What it names elsewhere is not fetched: the stream stops, saying the network is off.
    let s = Stream::open(
        Options::new(tmp.join("stream-off-away"), client.clone()),
        &server.url("/away/index.m3u8"),
    );
    assert_eq!(wait(&s).failure, Some(Failure::NetworkOff));
    // Nor is an address elsewhere, for all the client can reach this computer.
    let s = Stream::open(
        Options::new(tmp.join("stream-off-else"), client),
        "http://192.0.2.1/live.m3u8",
    );
    assert_eq!(wait(&s).failure, Some(Failure::NetworkOff));
}
