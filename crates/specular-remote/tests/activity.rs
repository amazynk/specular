//! The "Downloads" indicator's reports: real requests against a small HTTP server written
//! here (loopback only, no outside server), read back from a private registry.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use specular_core::net::{Registry, State, What};
use specular_remote::{Auth, CancelToken, Client, ClientConfig, Error, RemoteFile, Request};

/// Serves `data` at any path: `Range` requests get 206, every body goes out in pieces of
/// `piece` bytes `gap` apart (so a download is seen in flight). One connection at a time,
/// keep-alive; stops when dropped.
struct Server {
    port: u16,
    stop: Arc<AtomicBool>,
}

impl Server {
    fn start(data: Vec<u8>, piece: usize, gap: Duration) -> Server {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.set_nonblocking(true).unwrap();
        let port = l.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let data = Arc::new(data);
        specular_thread::spawn("act-test-srv", move || {
            while !s.load(Ordering::Acquire) {
                match l.accept() {
                    Ok((c, _)) => {
                        let (d, s) = (data.clone(), s.clone());
                        specular_thread::spawn("act-test-conn", move || {
                            serve(c, &d, piece, gap, &s)
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Server { port, stop }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn serve(c: TcpStream, data: &[u8], piece: usize, gap: Duration, stop: &AtomicBool) {
    c.set_nonblocking(false).unwrap();
    let mut out = c.try_clone().unwrap();
    let mut r = BufReader::new(c);
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let head = line.starts_with("HEAD ");
        let mut range = None;
        loop {
            let mut h = String::new();
            if r.read_line(&mut h).unwrap_or(0) == 0 {
                return;
            }
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            if let Some(v) = h
                .split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("range"))
                .map(|(_, v)| v.trim())
                .and_then(|v| v.strip_prefix("bytes="))
                .and_then(|v| v.split_once('-'))
            {
                let a: usize = v.0.parse().unwrap();
                let b: usize = v.1.parse().unwrap_or(data.len() - 1);
                range = Some((a, b.min(data.len() - 1)));
            }
        }
        let (status, body) = match range {
            Some((a, b)) => ("206 Partial Content", &data[a..=b]),
            None => ("200 OK", data),
        };
        let mut h = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"v1\"\r\n",
            body.len()
        );
        if let Some((a, b)) = range {
            h += &format!("Content-Range: bytes {a}-{b}/{}\r\n", data.len());
        }
        h += "\r\n";
        if out.write_all(h.as_bytes()).is_err() {
            return;
        }
        if head {
            continue;
        }
        for p in body.chunks(piece) {
            if stop.load(Ordering::Acquire) || out.write_all(p).is_err() {
                return;
            }
            let _ = out.flush();
            if !gap.is_zero() {
                std::thread::sleep(gap);
            }
        }
    }
}

fn data(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 + i / 97) as u8).collect()
}

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/test-tmp/remote-activity"
    ))
    .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn wait_for(what: &str, mut f: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < Duration::from_secs(20), "timed out: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_remote_files_range_reads_are_one_line_of_image_tiles() {
    let body = data(16 << 20);
    let srv = Server::start(body.clone(), 16 * 1024, Duration::ZERO);
    let reg = Registry::new();
    let client = Client::new(ClientConfig::for_tests())
        .unwrap()
        .tracked(What::ImageTiles)
        .reporting_to(reg.clone());
    // The signed query string is the kind of thing that must never be shown.
    let url = srv.url("/scene.tif?X-Amz-Signature=SECRET");
    let f = RemoteFile::new(client, url, Auth::None, None, 1 << 20);
    let cancel = CancelToken::new();
    let a = f.read_at(0, 1000, &cancel).unwrap();
    let b = f.read_at(12 << 20, 20_000, &cancel).unwrap();
    assert_eq!(a, body[..1000]);
    assert_eq!(b, body[12 << 20..(12 << 20) + 20_000]);
    let snap = reg.snapshot();
    assert_eq!(snap.len(), 1, "{snap:?}");
    let s = &snap[0];
    assert_eq!(s.label.what, What::ImageTiles);
    assert_eq!(s.label.host, format!("127.0.0.1:{}", srv.port));
    assert!(!format!("{s:?}").contains("SECRET"));
    assert_eq!(s.state, State::Done);
    assert!(s.requests >= 2, "{s:?}");
    assert_eq!(s.finished, s.requests);
    assert_eq!(s.failed, 0);
    assert!(s.bytes >= 21_000, "{s:?}");
    assert_eq!(s.total, Some(s.bytes));
    assert_eq!(s.fraction, Some(1.0));
}

#[test]
fn a_download_reports_progress_in_flight_and_stops_from_the_registry() {
    let body = data(4 << 20);
    // About 1.3 MB/s: three seconds for the whole file.
    let srv = Server::start(body, 64 * 1024, Duration::from_millis(50));
    let reg = Registry::new();
    let client = Client::new(ClientConfig::for_tests())
        .unwrap()
        .tracked(What::Model("clip".into()))
        .reporting_to(reg.clone());
    let dir = scratch("cancel");
    let path = dir.join("model.bin");
    let p = path.clone();
    let url = srv.url("/models/clip.onnx");
    let job = specular_thread::spawn("act-test-get", move || {
        let mut seen = 0u64;
        let r = client.download_to(&Request::get(url), &CancelToken::new(), &p, |pr| {
            seen = pr.received;
        });
        (r, seen)
    });
    // In flight: a single download with its length, part done, its speed measured.
    let mut last = None;
    wait_for("progress with a speed", || {
        let snap = reg.snapshot();
        let Some(a) = snap.first() else { return false };
        last = Some(a.clone());
        a.speed.is_some() && a.bytes > 0
    });
    let a = last.unwrap();
    assert_eq!(a.state, State::Running);
    assert!(!a.grouped());
    assert_eq!(a.total, Some(4 << 20));
    let f = a.fraction.unwrap();
    assert!(f > 0.0 && f < 1.0, "{f}");
    assert!(
        a.eta.is_some(),
        "a known length at a measured speed has a time left"
    );
    assert!(a.cancellable);
    assert_eq!(a.label.what, What::Model("clip".into()));
    // Stop, as the panel's button does.
    assert!(reg.cancel(a.id));
    let (r, seen) = job.join().unwrap();
    assert!(matches!(r, Err(Error::Cancelled)), "{r:?}");
    assert!(seen < 4 << 20);
    assert!(
        !path.exists(),
        "a stopped download leaves only its part file"
    );
    let s = &reg.snapshot()[0];
    assert_eq!(s.state, State::Cancelled);
    assert!(!s.cancellable);
    assert_eq!(s.speed, None);
}

#[test]
fn a_resumable_body_stream_counts_its_resumed_part_and_ends_with_the_body() {
    let body = data(300_000);
    let srv = Server::start(body.clone(), 32 * 1024, Duration::ZERO);
    let reg = Registry::new();
    let client = Client::new(ClientConfig::for_tests())
        .unwrap()
        .reporting_to(reg.clone());
    let o = client
        .open_at(
            &Request::get(srv.url("/pack.bin")),
            100_000,
            Some("\"v1\""),
            &CancelToken::new(),
        )
        .unwrap();
    assert_eq!(o.start, 100_000);
    let a = &reg.snapshot()[0];
    assert_eq!(a.label.what, What::Data, "an untagged client is plain data");
    assert_eq!((a.state, a.total), (State::Running, Some(300_000)));
    assert!(a.bytes >= 100_000);
    let mut rest = Vec::new();
    let mut b = o.body;
    b.read_to_end(&mut rest).unwrap();
    assert_eq!(rest, body[100_000..]);
    let a = &reg.snapshot()[0];
    assert_eq!((a.state, a.bytes), (State::Done, 300_000));
    drop(b);
    assert_eq!(reg.snapshot()[0].finished, 1, "ended once");
}

#[test]
fn an_unreachable_server_shows_as_failed() {
    // A port nothing listens on.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let reg = Registry::new();
    let client = Client::new(ClientConfig {
        retries: 0,
        ..ClientConfig::for_tests()
    })
    .unwrap()
    .tracked(What::UpdateCheck)
    .reporting_to(reg.clone());
    let r = client.get(
        &Request::get(format!("http://127.0.0.1:{port}/releases.json")),
        &CancelToken::new(),
    );
    assert!(r.is_err());
    let a = &reg.snapshot()[0];
    assert_eq!(
        a.state,
        State::Failed(specular_core::net::Failure::Offline),
        "{a:?}"
    );
}
