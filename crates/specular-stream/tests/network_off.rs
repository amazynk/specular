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
