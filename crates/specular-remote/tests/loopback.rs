//! This computer itself and the no-network switch (`Client::with_loopback`): with the switch
//! on, a client made for it reaches a loopback server and nothing else, follows a redirect
//! only to this computer, and waits for an answer that is held back, however long, until it
//! is cancelled. A plain client stays refused (`no_network.rs`).
//!
//! Loopback only. Its own test binary (the switch is process-wide), with one test.

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use specular_core::net::off::{self, By};
use specular_remote::testing::{ServerOptions, TestResponse, TestServer};
use specular_remote::{CancelToken, Client, ClientConfig, Error, Request};

#[test]
fn this_computer_is_reached_with_the_network_off_and_nothing_else() {
    let started = Instant::now();
    // Requests for `/held` are held until 2.5 s after the start: longer than an attempt
    // waits (1 s in tests) and than every attempt a plain request gets together.
    let held = Arc::new(AtomicU64::new(0));
    let h = held.clone();
    let server = TestServer::start(ServerOptions::default(), move |r| match r.path() {
        "/f.bin" => TestResponse::ok(b"here".to_vec()),
        "/to-here" => TestResponse::status(302).header("Location", "/f.bin"),
        "/to-here-absolute" => {
            let host = r.header("host").unwrap_or("127.0.0.1").to_owned();
            TestResponse::status(307).header("Location", format!("http://{host}/f.bin"))
        }
        // TEST-NET-1 (RFC 5737): no such host; a connection attempt would time out.
        "/away" => TestResponse::status(302).header("Location", "http://192.0.2.1/f.bin"),
        "/away-by-name" => TestResponse::status(301).header("Location", "//example.invalid/f.bin"),
        "/loop" => TestResponse::status(302).header("Location", "/loop"),
        "/held" => {
            h.fetch_add(1, Ordering::Relaxed);
            let left = Duration::from_millis(2500).saturating_sub(started.elapsed());
            if left.is_zero() {
                TestResponse::ok(b"at last".to_vec())
            } else {
                // Never answered: the client gives this attempt up first.
                TestResponse::ok(b"too late".to_vec()).delay(left + Duration::from_secs(5))
            }
        }
        _ => TestResponse::status(404),
    })
    .unwrap();
    let plain = Client::new(ClientConfig::for_tests()).unwrap();
    let local = plain.with_loopback();
    let cancel = CancelToken::new();
    let get = |c: &Client, path: &str| c.get(&Request::get(server.url(path)), &cancel);
    let port = server.url("").rsplit(':').next().unwrap().to_owned();

    off::set(By::Setting, true);
    let off_err = |e: Error, what: &str| assert!(e.is_network_off(), "{what}: {e:?}");

    // This computer answers the client made for it, by address and by name; the plain
    // client stays refused, and the switch says so for each.
    assert!(plain.refuses(&server.url("/f.bin")));
    assert!(!local.refuses(&server.url("/f.bin")));
    assert!(local.refuses("http://192.0.2.1/f.bin"));
    off_err(get(&plain, "/f.bin").unwrap_err(), "the plain client");
    assert_eq!(server.requests(), 0);
    assert_eq!(get(&local, "/f.bin").unwrap().body, b"here");
    let by_name = format!("http://localhost:{port}/f.bin");
    assert_eq!(
        local.get(&Request::get(&by_name), &cancel).unwrap().body,
        b"here"
    );
    let mut streamed = Vec::new();
    local
        .get_stream(&Request::get(server.url("/f.bin")), &cancel, |c, _| {
            streamed.extend_from_slice(c);
            ControlFlow::Continue(())
        })
        .unwrap();
    assert_eq!(streamed, b"here");
    assert_eq!(
        local
            .head(&Request::head(server.url("/f.bin")), &cancel)
            .unwrap()
            .status,
        200
    );

    // Anything else is refused before a socket opens, for the client made for this
    // computer too: other hosts, and names and spellings that only look like it.
    let refused0 = off::refused();
    let t0 = Instant::now();
    for elsewhere in [
        "http://192.0.2.1/f.bin".to_owned(),
        "http://example.invalid/f.bin".to_owned(),
        format!("http://127.0.0.1:{port}@192.0.2.1/f.bin"),
        format!("http://localhost.example.invalid:{port}/f.bin"),
        format!("http://127.1:{port}/f.bin"),
        format!("http://0.0.0.0:{port}/f.bin"),
    ] {
        off_err(
            local.get(&Request::get(&elsewhere), &cancel).unwrap_err(),
            &elsewhere,
        );
    }
    assert!(off::refused() >= refused0 + 6);

    // A redirect is followed to this computer, and refused when it leaves it (nothing
    // connects: the refusals come at once, not after a connection timed out).
    assert_eq!(get(&local, "/to-here").unwrap().body, b"here");
    assert_eq!(get(&local, "/to-here-absolute").unwrap().body, b"here");
    off_err(get(&local, "/away").unwrap_err(), "a redirect away");
    off_err(get(&local, "/away-by-name").unwrap_err(), "a redirect away");
    assert!(
        t0.elapsed() < Duration::from_secs(2),
        "refused without connecting: {:?}",
        t0.elapsed()
    );
    assert!(matches!(
        get(&local, "/loop").unwrap_err(),
        Error::Protocol(_)
    ));
    // Told not to follow redirects, it does not.
    assert!(matches!(
        local
            .get(
                &Request::get(server.url("/to-here")).no_redirects(),
                &cancel
            )
            .unwrap_err(),
        Error::Status(302)
    ));

    // An answer held back is waited for: asked again each time an attempt runs out, past
    // the attempts a request gets otherwise, until it comes.
    let t0 = Instant::now();
    assert_eq!(get(&local, "/held").unwrap().body, b"at last");
    let asked = held.load(Ordering::Relaxed);
    assert!(
        asked >= 2 && started.elapsed() >= Duration::from_millis(2500),
        "asked {asked} times in {:?}",
        t0.elapsed()
    );

    // A caller that asks again itself gets the wait back after the time it gave.
    let t0 = Instant::now();
    let stuck = TestServer::start(ServerOptions::default(), |_| {
        TestResponse::ok(Vec::new()).delay(Duration::from_secs(30))
    })
    .unwrap();
    let once = Request::get(stuck.url("/never")).local_wait(Duration::from_millis(300));
    assert!(matches!(
        local.get(&once, &cancel).unwrap_err(),
        Error::Timeout
    ));
    assert!(
        t0.elapsed() < Duration::from_secs(2) && stuck.requests() == 1,
        "{:?}, asked {} times",
        t0.elapsed(),
        stuck.requests()
    );

    // Such a wait given up is no failed download (the panel counts those).
    let reg = specular_core::net::Registry::new();
    let shown = local
        .clone()
        .tracked(specular_core::net::What::Stream)
        .reporting_to(reg.clone());
    assert!(matches!(
        shown.get(&once, &cancel).unwrap_err(),
        Error::Timeout
    ));
    let snap = reg.snapshot();
    assert_eq!(snap.len(), 1, "{snap:?}");
    assert_eq!((snap[0].finished, snap[0].failed), (1, 0), "{snap:?}");

    // And a wait is over as soon as it is cancelled (within one attempt).
    let c2 = CancelToken::new();
    let (c3, url) = (c2.clone(), stuck.url("/never"));
    let l2 = local.clone();
    let t0 = Instant::now();
    #[allow(clippy::disallowed_methods)]
    let waiting = std::thread::spawn(move || l2.get(&Request::get(url), &c3));
    std::thread::sleep(Duration::from_millis(300));
    c2.cancel();
    assert!(matches!(waiting.join().unwrap(), Err(Error::Cancelled)));
    assert!(
        t0.elapsed() < Duration::from_secs(4),
        "cancelled after {:?}",
        t0.elapsed()
    );

    // With the network on, a redirect off this computer is followed as ever (here: to a
    // server of its own on another loopback port, standing in for one elsewhere).
    off::set(By::Setting, false);
    assert_eq!(get(&plain, "/to-here").unwrap().body, b"here");
    assert_eq!(get(&local, "/to-here").unwrap().body, b"here");
}
