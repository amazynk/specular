//! The remote layer against the in-process test server (loopback only).

use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use specular_remote::testing::{ServerOptions, TestResponse, TestServer};
use specular_remote::{
    Auth, CancelToken, Client, ClientConfig, DiskCache, Error, RemoteFile, Request, Secret,
};

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/test-tmp/remote"
    ))
    .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn client() -> Client {
    Client::new(ClientConfig::for_tests()).unwrap()
}

fn data(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 + i / 251) as u8).collect()
}

#[test]
fn get_reuses_connections_and_reads_chunked_bodies() {
    let body = data(300_000);
    let b = body.clone();
    let srv = TestServer::start(ServerOptions::default(), move |r| match r.path() {
        "/chunked" => TestResponse::ok(b.clone()).chunked(7_001),
        _ => TestResponse::ok(b.clone()),
    })
    .unwrap();
    let c = client();
    let cancel = CancelToken::new();
    for _ in 0..3 {
        let r = c.get(&Request::get(srv.url("/plain")), &cancel).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body, body);
    }
    let r = c.get(&Request::get(srv.url("/chunked")), &cancel).unwrap();
    assert_eq!(r.body, body);
    assert_eq!(srv.requests(), 4);
    assert_eq!(
        srv.stats().connections.load(Ordering::Relaxed),
        1,
        "keep-alive reuses one connection"
    );
}

#[test]
fn range_requests_and_remote_file_blocks() {
    let body = data(1_000_000);
    let b = body.clone();
    let srv = TestServer::start(ServerOptions::default(), move |r| {
        TestResponse::ranged(r, &b, "\"v1\"")
    })
    .unwrap();
    let c = client();
    let cancel = CancelToken::new();
    let r = c
        .get(&Request::get(srv.url("/f")).range(1000..1010), &cancel)
        .unwrap();
    assert_eq!(r.status, 206);
    assert_eq!(r.body, &body[1000..1010]);
    assert_eq!(
        r.headers.content_range(),
        Some((1000, 1009, Some(1_000_000)))
    );
    // Range requests ask for the stored bytes, not a compressed variant.
    assert_eq!(
        srv.log().last().unwrap().header("accept-encoding"),
        Some("identity")
    );

    let f = RemoteFile::new(c, srv.url("/f"), Auth::None, None, 4 << 20);
    assert_eq!(f.len(&cancel).unwrap(), 1_000_000);
    let before = srv.requests();
    assert_eq!(f.read_at(10, 100, &cancel).unwrap(), &body[10..110]);
    assert_eq!(
        srv.requests(),
        before,
        "first block came with the size probe"
    );
    // Spans blocks 1..=3: one coalesced request.
    let a = 300_000u64;
    assert_eq!(
        f.read_at(a, 500_000, &cancel).unwrap(),
        &body[a as usize..a as usize + 500_000]
    );
    assert_eq!(srv.requests(), before + 1);
    // Cached now.
    assert_eq!(
        f.read_at(a + 5, 10, &cancel).unwrap(),
        &body[a as usize + 5..a as usize + 15]
    );
    assert_eq!(srv.requests(), before + 1);
    // Past the end.
    assert_eq!(f.read_at(999_990, 100, &cancel).unwrap(), &body[999_990..]);
    assert!(f.read_at(2_000_000, 10, &cancel).unwrap().is_empty());
}

#[test]
fn remote_file_without_range_support_still_reads() {
    let body = data(600_000);
    let b = body.clone();
    let srv = TestServer::start(ServerOptions::default(), move |_| {
        TestResponse::ok(b.clone())
    })
    .unwrap();
    let f = RemoteFile::new(client(), srv.url("/f"), Auth::None, None, 4 << 20);
    let cancel = CancelToken::new();
    assert_eq!(f.len(&cancel).unwrap(), 600_000);
    assert_eq!(
        f.read_at(500_000, 10, &cancel).unwrap(),
        &body[500_000..500_010]
    );
}

#[test]
fn auth_headers_and_401() {
    let srv = TestServer::start(ServerOptions::default(), |r| {
        match r.header("authorization") {
            Some("Basic dTpwdw==") | Some("Bearer t0k") => TestResponse::ok("yes"),
            _ => TestResponse::status(401).header("WWW-Authenticate", "Basic realm=\"x\""),
        }
    })
    .unwrap();
    let c = client();
    let cancel = CancelToken::new();
    let e = c.get(&Request::get(srv.url("/")), &cancel).unwrap_err();
    assert!(matches!(e, Error::Status(401)) && e.is_auth());
    assert_eq!(srv.requests(), 1, "401 is not retried");
    let basic = Auth::Basic {
        user: "u".into(),
        password: Secret::new("pw"),
    };
    assert_eq!(
        c.get(&Request::get(srv.url("/")).auth(&basic), &cancel)
            .unwrap()
            .body,
        b"yes"
    );
    let bearer = Auth::Bearer(Secret::new("t0k"));
    assert_eq!(
        c.get(&Request::get(srv.url("/")).auth(&bearer), &cancel)
            .unwrap()
            .body,
        b"yes"
    );
    // The request's Debug never shows the secret.
    assert!(!format!("{:?}", Request::get("http://x/").auth(&basic)).contains("pw"));
}

#[test]
fn errors_404_5xx_retries_and_retry_after() {
    let n = Arc::new(AtomicU32::new(0));
    let n2 = n.clone();
    let srv = TestServer::start(ServerOptions::default(), move |r| match r.path() {
        "/missing" => TestResponse::status(404),
        "/broken" => TestResponse::status(500),
        "/flaky" => {
            if n2.fetch_add(1, Ordering::SeqCst) < 2 {
                TestResponse::status(503).header("Retry-After", "0")
            } else {
                TestResponse::ok("ok")
            }
        }
        _ => TestResponse::status(502),
    })
    .unwrap();
    let c = client();
    let cancel = CancelToken::new();
    assert!(matches!(
        c.get(&Request::get(srv.url("/missing")), &cancel),
        Err(Error::Status(404))
    ));
    assert!(matches!(
        c.get(&Request::get(srv.url("/broken")), &cancel),
        Err(Error::Status(500))
    ));
    assert_eq!(srv.requests(), 2, "404 and 500 are final");
    assert_eq!(
        c.get(&Request::get(srv.url("/flaky")), &cancel)
            .unwrap()
            .body,
        b"ok"
    );
    assert_eq!(n.load(Ordering::SeqCst), 3);
    let before = srv.requests();
    assert!(matches!(
        c.get(&Request::get(srv.url("/gateway")), &cancel),
        Err(Error::Status(502))
    ));
    assert_eq!(srv.requests() - before, 3, "first try + 2 retries");
}

#[test]
fn timeouts_and_unreachable_hosts() {
    let srv = TestServer::start(ServerOptions::default(), |r| match r.path() {
        "/slow-head" => TestResponse::ok("late").delay(Duration::from_secs(3)),
        _ => TestResponse::ok(vec![1u8; 100_000]).cut(1000, Duration::from_secs(3)),
    })
    .unwrap();
    let c = Client::new(ClientConfig {
        response_timeout: Duration::from_millis(300),
        body_timeout: Some(Duration::from_millis(300)),
        retries: 0,
        ..ClientConfig::for_tests()
    })
    .unwrap();
    let cancel = CancelToken::new();
    let t = Instant::now();
    let e = c
        .get(&Request::get(srv.url("/slow-head")), &cancel)
        .unwrap_err();
    assert!(matches!(e, Error::Timeout), "{e:?}");
    assert!(e.is_offline());
    let e = c
        .get(&Request::get(srv.url("/stall")), &cancel)
        .unwrap_err();
    assert!(matches!(e, Error::Timeout), "{e:?}");
    assert!(t.elapsed() < Duration::from_secs(3));

    // Nothing listens on a fresh port of a dropped listener.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let e = c
        .get(&Request::get(format!("http://127.0.0.1:{port}/")), &cancel)
        .unwrap_err();
    assert!(e.is_offline(), "{e:?}");
}

#[test]
fn cancel_mid_stream() {
    let srv = TestServer::start(
        ServerOptions {
            bandwidth: Some(2_000_000),
            ..ServerOptions::default()
        },
        |_| TestResponse::ok(vec![7u8; 4_000_000]),
    )
    .unwrap();
    let c = client();
    let cancel = CancelToken::new();
    let t = Instant::now();
    let mut got = 0u64;
    let e = c
        .get_stream(&Request::get(srv.url("/big")), &cancel, |chunk, p| {
            got += chunk.len() as u64;
            assert_eq!(p.total, Some(4_000_000));
            if got > 200_000 {
                cancel.cancel();
            }
            ControlFlow::Continue(())
        })
        .unwrap_err();
    assert!(matches!(e, Error::Cancelled));
    assert!(
        t.elapsed() < Duration::from_millis(1500),
        "{:?}",
        t.elapsed()
    );
    assert!(got < 1_000_000);
    // A cancelled token stops before sending anything.
    let before = srv.requests();
    assert!(matches!(
        c.get(&Request::get(srv.url("/big")), &cancel),
        Err(Error::Cancelled)
    ));
    assert_eq!(srv.requests(), before);
    // Break ends a stream early without an error.
    let fresh = CancelToken::new();
    let head = c
        .get_stream(&Request::get(srv.url("/big")), &fresh, |_, _| {
            ControlFlow::Break(())
        })
        .unwrap();
    assert_eq!(head.status, 200);
}

#[test]
fn download_resumes_with_if_range() {
    let body = data(2_000_000);
    let b = body.clone();
    let cut = Arc::new(AtomicU32::new(1));
    let cut2 = cut.clone();
    let srv = TestServer::start(ServerOptions::default(), move |r| {
        let res = TestResponse::ranged(r, &b, "\"e1\"");
        if cut2.swap(0, Ordering::SeqCst) == 1 {
            res.cut(700_000, Duration::ZERO)
        } else {
            res
        }
    })
    .unwrap();
    let dir = scratch("resume");
    let path = dir.join("model.bin");
    let c = Client::new(ClientConfig {
        retries: 0,
        ..ClientConfig::for_tests()
    })
    .unwrap();
    let cancel = CancelToken::new();
    // First call: the connection drops at 700 kB.
    assert!(
        c.download_to(&Request::get(srv.url("/m")), &cancel, &path, |_| {})
            .is_err()
    );
    assert!(!path.exists());
    let part = specular_remote::http::part_path(&path);
    assert_eq!(std::fs::metadata(&part).unwrap().len(), 700_000);
    // Second call resumes from there.
    let mut first = None;
    let head = c
        .download_to(&Request::get(srv.url("/m")), &cancel, &path, |p| {
            first.get_or_insert(p.received);
        })
        .unwrap();
    assert_eq!(head.status, 206);
    assert_eq!(first, Some(700_000));
    assert_eq!(std::fs::read(&path).unwrap(), body);
    assert!(!part.exists());
    let last = srv.log().pop().unwrap();
    assert_eq!(last.header("range"), Some("bytes=700000-"));
    assert_eq!(last.header("if-range"), Some("\"e1\""));

    // With retries on, a drop mid-body resumes within one call.
    cut.store(1, Ordering::SeqCst);
    let path2 = dir.join("again.bin");
    client()
        .download_to(&Request::get(srv.url("/m")), &cancel, &path2, |_| {})
        .unwrap();
    assert_eq!(std::fs::read(&path2).unwrap(), body);
}

#[test]
fn download_restarts_when_the_file_changed() {
    let body = data(500_000);
    let b = body.clone();
    let srv = TestServer::start(ServerOptions::default(), move |r| {
        TestResponse::ranged(r, &b, "\"new\"")
    })
    .unwrap();
    let dir = scratch("changed");
    let path = dir.join("f.bin");
    let part = specular_remote::http::part_path(&path);
    std::fs::write(&part, vec![0u8; 1234]).unwrap();
    std::fs::write(
        format!("{}.meta", part.display()),
        format!("{}\n\"old\"", srv.url("/f")),
    )
    .unwrap();
    let head = client()
        .download_to(
            &Request::get(srv.url("/f")),
            &CancelToken::new(),
            &path,
            |_| {},
        )
        .unwrap();
    assert_eq!(head.status, 200);
    assert_eq!(std::fs::read(&path).unwrap(), body);
}

#[test]
fn disk_cache_hits_eviction_pins_and_reopen() {
    let dir = scratch("cache");
    let cache = DiskCache::open(&dir, 1000).unwrap();
    let k = |i: u32| DiskCache::key(&["srv", "study", &format!("{i}.dcm")]);
    for i in 0..4 {
        cache.insert_bytes(&k(i), &[i as u8; 300]).unwrap();
    }
    // 1200 > 1000: the oldest went.
    assert!(cache.get(&k(0)).is_none());
    assert!(cache.get(&k(1)).is_some());
    let s = cache.stats();
    assert_eq!((s.entries, s.bytes, s.hits, s.misses), (3, 900, 1, 1));
    // Pinned entries survive; the least recent unpinned one goes.
    let pin = cache.pin("srv/study/");
    cache.insert_bytes("other/a", &[9; 300]).unwrap();
    assert_eq!(cache.stats().entries, 3);
    assert!(cache.contains(&k(1)) && cache.contains(&k(2)) && cache.contains(&k(3)));
    drop(pin);
    // After unpinning, the budget is enforced again.
    assert!(cache.stats().bytes <= 1000);

    // Reopening rebuilds the index from the files.
    let n = cache.stats().entries;
    drop(cache);
    let again = DiskCache::open(&dir, 1000).unwrap();
    assert_eq!(again.stats().entries, n);
    again.remove_prefix("srv");
    assert!(!dir.join("srv").exists());
}

#[test]
fn cache_fetch_downloads_once() {
    let body = data(100_000);
    let b = body.clone();
    let srv = TestServer::start(ServerOptions::default(), move |r| {
        TestResponse::ranged(r, &b, "\"x\"")
    })
    .unwrap();
    let dir = scratch("fetch");
    let cache = DiskCache::open(&dir, 10 << 20).unwrap();
    let f = RemoteFile::new(
        client(),
        srv.url("/a.dcm"),
        Auth::None,
        Some(cache.clone()),
        1 << 20,
    );
    let cancel = CancelToken::new();
    let s1 = f.source(&cancel, |_| {}).unwrap();
    assert_eq!(s1.bytes(), &body[..]);
    let s2 = f.source(&cancel, |_| {}).unwrap();
    assert_eq!(s2.bytes(), &body[..]);
    assert_eq!(srv.requests(), 1);
    assert_eq!(cache.stats().hits, 1);
}

#[test]
fn open_at_reconnects_mid_body_and_guards_resumes() {
    use std::io::Read;
    let body = data(1_500_000);
    let b = body.clone();
    let etag = Arc::new(parking_lot::Mutex::new("\"v1\"".to_string()));
    let cuts = Arc::new(AtomicU32::new(2));
    let (e2, c2) = (etag.clone(), cuts.clone());
    let srv = TestServer::start(ServerOptions::default(), move |r| {
        let res = TestResponse::ranged(r, &b, &e2.lock());
        // The first two responses drop after 300 kB.
        if c2.load(Ordering::SeqCst) > 0 {
            c2.fetch_sub(1, Ordering::SeqCst);
            res.cut(300_000, Duration::ZERO)
        } else {
            res
        }
    })
    .unwrap();
    let dir = scratch("open-at");
    let cancel = CancelToken::new();
    let r = specular_remote::Resumer::new(client(), Some(dir.clone()));
    let req = Request::get(srv.url("/model.onnx"));

    // From 0: two drops, the stream reconnects with Range + If-Range each time.
    let mut o = r.open(&req, 0, &cancel).unwrap();
    assert_eq!((o.start, o.total), (0, Some(1_500_000)));
    assert!(!o.resumed(0));
    let mut got = Vec::new();
    o.body.read_to_end(&mut got).unwrap();
    assert_eq!(got, body);
    let log = srv.log();
    assert_eq!(log.len(), 3);
    assert_eq!(log[1].header("range"), Some("bytes=300000-"));
    assert_eq!(log[2].header("range"), Some("bytes=600000-"));
    assert_eq!(log[2].header("if-range"), Some("\"v1\""));

    // A later session (new Resumer, same folder) resumes a kept partial file safely.
    let r2 = specular_remote::Resumer::new(client(), Some(dir.clone()));
    assert_eq!(r2.validator(req.url()).as_deref(), Some("\"v1\""));
    let mut o = r2.open(&req, 1_000_000, &cancel).unwrap();
    assert!(o.resumed(1_000_000));
    assert_eq!(o.total, Some(1_500_000));
    let mut rest = Vec::new();
    o.body.read_to_end(&mut rest).unwrap();
    assert_eq!(rest, body[1_000_000..]);
    assert_eq!(srv.log().pop().unwrap().header("if-range"), Some("\"v1\""));

    // The file changed on the server: the whole body comes back, not a splice.
    *etag.lock() = "\"v2\"".into();
    let o = r2.open(&req, 1_000_000, &cancel).unwrap();
    assert_eq!(o.start, 0);
    assert!(!o.resumed(1_000_000));
    assert_eq!(r2.validator(req.url()).as_deref(), Some("\"v2\""));
    drop(o);

    // No validator: the error is returned rather than a blind splice.
    let plain = body.clone();
    let srv2 = TestServer::start(ServerOptions::default(), move |_| {
        TestResponse::ok(plain.clone()).cut(100_000, Duration::ZERO)
    })
    .unwrap();
    let mut o = client()
        .open_at(&Request::get(srv2.url("/x")), 0, None, &cancel)
        .unwrap();
    let mut sink = Vec::new();
    assert!(o.body.read_to_end(&mut sink).is_err());
    assert_eq!(sink.len(), 100_000);
    r2.forget(req.url());
    assert_eq!(r2.validator(req.url()), None);
}

#[test]
fn open_at_is_cancellable() {
    use std::io::Read;
    let body = data(4_000_000);
    let srv = TestServer::start(
        ServerOptions {
            bandwidth: Some(2_000_000),
            ..ServerOptions::default()
        },
        move |r| TestResponse::ranged(r, &body, "\"e\""),
    )
    .unwrap();
    let cancel = CancelToken::new();
    let mut o = client()
        .open_at(&Request::get(srv.url("/big")), 0, None, &cancel)
        .unwrap();
    let mut buf = vec![0; 65_536];
    assert!(o.body.read(&mut buf).unwrap() > 0);
    cancel.cancel();
    let t = Instant::now();
    assert!(o.body.read(&mut buf).is_err());
    assert!(t.elapsed() < Duration::from_secs(1));
}

#[test]
fn post_sends_its_body_once_and_never_prints_it() {
    let srv = TestServer::start(ServerOptions::default(), |r| {
        if r.method == "POST" && r.body == b"user=u&password=hunter2" {
            TestResponse::status(503)
        } else {
            TestResponse::ok(b"?".to_vec())
        }
    })
    .unwrap();
    let c = client();
    let cancel = CancelToken::new();
    let req = Request::post(
        srv.url("/login"),
        "application/x-www-form-urlencoded",
        b"user=u&password=hunter2".to_vec(),
    );
    assert!(!format!("{req:?}").contains("hunter2"));
    // 503 is retried for GET; a POST is sent once.
    assert!(matches!(c.post(&req, &cancel), Err(Error::Status(503))));
    assert_eq!(srv.requests(), 1);
    let r = srv.log().pop().unwrap();
    assert_eq!(r.method, "POST");
    assert_eq!(
        r.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    // A POST that succeeds hands back its body.
    let ok = Request::post(srv.url("/echo"), "text/plain", b"x".to_vec());
    assert_eq!(c.post(&ok, &cancel).unwrap().body, b"?");
}

#[test]
fn redirects_are_followed_unless_refused() {
    let srv = TestServer::start(ServerOptions::default(), |r| match r.path() {
        "/old" => TestResponse::status(302).header("Location", "/login"),
        _ => TestResponse::ok(b"login page".to_vec()),
    })
    .unwrap();
    let c = client();
    let cancel = CancelToken::new();
    let r = c.get(&Request::get(srv.url("/old")), &cancel).unwrap();
    assert_eq!(r.body, b"login page");
    assert!(matches!(
        c.get(&Request::get(srv.url("/old")).no_redirects(), &cancel),
        Err(Error::Status(302))
    ));
}
