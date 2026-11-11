//! Protocol version 3 end to end: events over a real socket, with a stand-in window. A
//! subscriber gets the window's events in order and can still make requests; a slow one loses
//! events (and is told how many) without holding the window up; subscribers have no idle limit
//! and a budget of their own; the Windows key works wherever it is asked for; a hook's
//! program is known as such; and nothing listens on the network.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde_json::{Map, Value, json};
use specular_client::{Client, Request};
use specular_platform::ipc::{self, Stream};

use super::events::{Event, QUEUE};
use super::server::{Call, MAX_CONNECTIONS, MAX_SUBSCRIBERS, Options, Origin, Server};

fn test_dir(name: &str) -> PathBuf {
    ipc::scratch_dir(&format!("specular-v3-{name}")).unwrap()
}

/// What the stand-in window was asked, and by whom.
type Seen = Arc<Mutex<Vec<(Request, Origin)>>>;

/// A stand-in window: answers everything, remembering who asked what.
fn fake_window(calls: crossbeam_channel::Receiver<Call>) -> (std::thread::JoinHandle<()>, Seen) {
    let seen: Seen = Arc::default();
    let s2 = seen.clone();
    let t = std::thread::spawn(move || {
        while let Ok(c) = calls.recv() {
            s2.lock().push((c.req.clone(), c.origin.clone()));
            let quit = c.req == Request::Quit;
            let cmd = c.req.cmd();
            c.answer(Ok(json!({ "cmd": cmd })));
            if quit {
                break;
            }
        }
    });
    (t, seen)
}

/// A server for these tests, which talk the line protocol on raw connections: without the
/// key Windows asks for (`the_key_is_asked_for_where_the_server_wants_it` tests that).
fn start(name: &str, o: Options) -> (PathBuf, Server, std::thread::JoinHandle<()>) {
    let dir = test_dir(name);
    let server = Server::start_opts(&dir, Arc::new(|| {}), Options { key: false, ..o }).unwrap();
    let (w, _) = fake_window(server.calls.clone());
    (dir, server, w)
}

fn quick() -> Options {
    Options {
        ping: Duration::from_millis(150),
        key: false,
        ..Options::default()
    }
}

fn marked(i: usize) -> Event {
    Event::new("marked", vec![PathBuf::from(format!("/p/{i}.cr3"))])
        .with("mark", json!({"flag": "pick", "stars": 1, "label": null}))
}

fn user() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("source".into(), json!("user"));
    m
}

fn raw(path: &Path) -> (BufReader<Stream>, Stream) {
    let s = ipc::connect(path, Duration::from_secs(20)).unwrap();
    let w = s.try_clone().unwrap();
    (BufReader::new(s), w)
}

fn send(w: &mut Stream, line: &str) {
    assert!(try_send(w, line));
}

/// As [`send`], for a connection the server may have refused (and closed) already.
fn try_send(w: &mut Stream, line: &str) -> bool {
    w.write_all(format!("{line}\n").as_bytes()).is_ok()
}

fn read(r: &mut BufReader<Stream>) -> Value {
    let mut s = String::new();
    // A refused connection may be reset: nothing read.
    let _ = r.read_line(&mut s);
    serde_json::from_str(&s).unwrap_or(Value::Null)
}

fn until(what: &str, f: impl Fn() -> bool) {
    let t = Instant::now();
    while !f() {
        assert!(
            t.elapsed() < Duration::from_secs(30),
            "waited too long for {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn quit(server: &Server, w: std::thread::JoinHandle<()>, dir: &Path) {
    // Connections a test just closed may still count for a moment: retry while busy.
    let t = Instant::now();
    loop {
        match Client::connect(server.path()).unwrap().call(&Request::Quit) {
            Ok(_) => break,
            Err(e) if e.code == "busy" && t.elapsed() < Duration::from_secs(10) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("quit: {e:?}"),
        }
    }
    w.join().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn subscribers_get_events_in_order_and_can_still_ask() {
    let (dir, server, w) = start("order", quick());
    let bus = server.bus().clone();
    let mut c = Client::connect(server.path()).unwrap();
    let hello = c.hello().unwrap();
    assert_eq!(hello["protocol"], 3);
    assert_eq!(hello["protocols"], json!([1, 2, 3]));
    assert_eq!(hello["events"], json!(specular_client::protocol::EVENTS));
    assert_eq!(hello["capabilities"]["max_subscribers"], MAX_SUBSCRIBERS);
    let commands = hello["commands"].as_array().unwrap();
    assert!(commands.iter().any(|c| c == "subscribe"));
    bus.publish(&marked(0), &user());
    let mut ev = c.subscribe(&["marked"]).unwrap();
    let start = ev.seq;
    assert_eq!(start, 1, "the one before was numbered");
    for i in 1..=3 {
        bus.publish(&marked(i), &user());
    }
    bus.publish(&Event::new("opened", vec!["/p/x.jpg".into()]), &user());
    let mut last = start;
    for i in 1..=3 {
        let e = ev.next().unwrap().unwrap();
        assert_eq!(e["event"], "marked");
        assert_eq!(e["files"], json!([format!("/p/{i}.cr3")]));
        assert_eq!(e["mark"]["flag"], "pick");
        assert_eq!(e["source"], "user");
        let seq = e["seq"].as_u64().unwrap();
        assert!(seq > last, "{seq} after {last}");
        last = seq;
        assert!(e.get("id").is_none());
    }
    // A request in between: answered (with its id) while events keep coming.
    bus.publish(&marked(4), &user());
    let state = ev.client().call(&Request::State).unwrap();
    assert_eq!(state["cmd"], "state");
    let e = ev.next().unwrap().unwrap();
    assert_eq!(
        e["files"],
        json!(["/p/4.cr3"]),
        "kept while the answer was read"
    );
    // More names on the same connection.
    ev.client()
        .call(&Request::Subscribe {
            events: vec!["opened".into()],
        })
        .unwrap();
    bus.publish(&Event::new("opened", vec!["/p/y.jpg".into()]), &user());
    let e = ev.next().unwrap().unwrap();
    assert_eq!(e["event"], "opened");
    // Pings come but the iterator leaves them out; a raw reader sees them.
    let (mut r, mut wr) = raw(server.path());
    send(
        &mut wr,
        r#"{"v":3,"id":1,"cmd":"subscribe","events":["*"]}"#,
    );
    let a = read(&mut r);
    assert_eq!((a["id"].clone(), a["ok"].clone()), (json!(1), json!(true)));
    let p = read(&mut r);
    assert_eq!(p["event"], "ping");
    assert!(p["seq"].as_u64().is_some());
    drop((r, wr));
    // Unsubscribed: nothing more comes, and the connection is an ordinary one again.
    ev.unsubscribe().unwrap();
    until("the subscriber to go", || {
        bus.subscribers() == 1 || bus.subscribers() == 0
    });
    bus.publish(&marked(5), &user());
    assert_eq!(c.call(&Request::State).unwrap()["cmd"], "state");
    // A version 2 client cannot subscribe.
    c.set_version(2);
    let e = c
        .call(&Request::Subscribe {
            events: vec!["marked".into()],
        })
        .unwrap_err();
    assert_eq!(e.code, "version");
    let e = Client::connect(server.path())
        .unwrap()
        .call(&Request::Subscribe {
            events: vec!["clicked".into()],
        })
        .unwrap_err();
    assert_eq!(e.code, "unknown-event");
    quit(&server, w, &dir);
}

#[test]
fn a_slow_subscriber_loses_events_and_never_holds_the_window() {
    let (dir, server, w) = start("slow", quick());
    let bus = server.bus().clone();
    let (mut r, mut wr) = raw(server.path());
    send(
        &mut wr,
        r#"{"v":3,"id":1,"cmd":"subscribe","events":["marked"]}"#,
    );
    assert_eq!(read(&mut r)["ok"], true);
    // 10 000 events while the client reads nothing.
    let t = Instant::now();
    for i in 0..10_000 {
        bus.publish(&marked(i), &user());
    }
    let took = t.elapsed();
    assert!(took < Duration::from_secs(3), "publishing took {took:?}");
    // Now it reads: what was queued, then how many were lost.
    let mut got = 0u64;
    let mut lost = None;
    let mut last = 0;
    while lost.is_none() {
        let v = read(&mut r);
        match v["event"].as_str() {
            Some("marked") => {
                let s = v["seq"].as_u64().unwrap();
                assert!(s > last);
                last = s;
                got += 1;
            }
            Some("lost") => lost = v["count"].as_u64(),
            Some("ping") => {}
            _ => panic!("{v}"),
        }
    }
    let lost = lost.unwrap();
    assert!(got >= QUEUE as u64, "the queue's worth arrived: {got}");
    assert_eq!(got + lost, 10_000, "{got} + {lost}");
    eprintln!("slow subscriber: {got} delivered, {lost} lost, published in {took:?}");
    drop((r, wr));
    quit(&server, w, &dir);
}

#[test]
fn subscribers_have_no_idle_limit_and_a_budget_of_their_own() {
    let o = Options {
        ping: Duration::from_millis(100),
        idle: Duration::from_millis(400),
        ..Options::default()
    };
    let (dir, server, w) = start("budget", o);
    // An ordinary connection idle too long is dropped…
    let (mut r, _wr) = raw(server.path());
    let t = Instant::now();
    let mut s = String::new();
    assert_eq!(r.read_line(&mut s).unwrap_or(0), 0, "closed: {s}");
    assert!(t.elapsed() >= Duration::from_millis(300));
    // …a subscriber is not.
    let mut subs = Vec::new();
    for i in 0..MAX_SUBSCRIBERS {
        let (mut r, mut wr) = raw(server.path());
        send(
            &mut wr,
            &format!(r#"{{"v":3,"id":{i},"cmd":"subscribe","events":["*"]}}"#),
        );
        assert_eq!(read(&mut r)["ok"], true, "subscriber {i}");
        subs.push((r, wr));
    }
    std::thread::sleep(Duration::from_millis(1000));
    for (r, wr) in &mut subs {
        assert_eq!(read(r)["event"], "ping", "still there after the idle time");
        send(wr, r#"{"v":3,"id":"s","cmd":"state"}"#);
        // Pings may come before the answer.
        let a = loop {
            let v = read(r);
            if v.get("id").is_some() {
                break v;
            }
        };
        assert_eq!(a["id"], "s");
    }
    // The subscribers left all ordinary places free: 8 more connect…
    let mut held = Vec::new();
    while held.len() < MAX_CONNECTIONS {
        let (mut r, mut wr) = raw(server.path());
        let v = if try_send(&mut wr, r#"{"v":1,"cmd":"hello"}"#) {
            read(&mut r)
        } else {
            Value::Null
        };
        if v["ok"] == true {
            held.push((r, wr));
        } else {
            // A slot from the idle connection above may still be on its way back.
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    // …and the 9th is refused.
    let (mut r9, _w9) = raw(server.path());
    let v = read(&mut r9);
    assert_eq!(v["error"]["code"], "busy", "{v}");
    // A 5th subscriber is refused too (once it has an ordinary place to ask from).
    drop(held.pop());
    let deadline = Instant::now() + Duration::from_secs(20);
    let v = loop {
        let (mut r, mut wr) = raw(server.path());
        let v = if try_send(
            &mut wr,
            r#"{"v":3,"id":5,"cmd":"subscribe","events":["marked"]}"#,
        ) {
            read(&mut r)
        } else {
            Value::Null
        };
        if v["id"] == 5 || Instant::now() > deadline {
            break v;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(v["error"]["code"], "busy", "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("subscribe")
    );
    drop(held);
    drop(subs);
    quit(&server, w, &dir);
}

#[test]
fn the_key_is_asked_for_where_the_server_wants_it() {
    let o = Options {
        key: true,
        ..quick()
    };
    let dir = test_dir("key");
    let server = Server::start_opts(&dir, Arc::new(|| {}), o).unwrap();
    let (w, seen) = fake_window(server.calls.clone());
    let key_file = specular_client::protocol::key_path(server.path());
    assert!(key_file.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let m = std::fs::metadata(&key_file).unwrap().permissions().mode();
        assert_eq!(m & 0o777, 0o600);
    }
    // Without the key: nothing but a refusal, and the window hears nothing.
    let (mut r, mut wr) = raw(server.path());
    send(&mut wr, r#"{"v":3,"id":1,"cmd":"state"}"#);
    assert_eq!(read(&mut r)["error"]["code"], "refused");
    send(&mut wr, r#"{"v":3,"id":2,"cmd":"hello","key":"wrong"}"#);
    assert_eq!(read(&mut r)["error"]["code"], "refused");
    send(
        &mut wr,
        r#"{"v":3,"id":3,"cmd":"subscribe","events":["*"]}"#,
    );
    assert_eq!(read(&mut r)["error"]["code"], "refused");
    assert!(seen.lock().is_empty());
    // With it (the clients read the file next to the socket): all works.
    let key = std::fs::read_to_string(&key_file).unwrap();
    send(
        &mut wr,
        &format!(r#"{{"v":3,"id":4,"cmd":"hello","key":"{key}"}}"#),
    );
    assert_eq!(read(&mut r)["ok"], true);
    send(&mut wr, r#"{"v":3,"id":5,"cmd":"state"}"#);
    assert_eq!(read(&mut r)["ok"], true);
    let mut c = Client::connect(server.path()).unwrap();
    assert_eq!(c.call(&Request::State).unwrap()["cmd"], "state");
    let mut ac = super::client::Client::connect(server.path()).unwrap();
    assert_eq!(ac.call(&Request::State).unwrap()["cmd"], "state");
    drop((r, wr));
    Client::connect(server.path())
        .unwrap()
        .call(&Request::Quit)
        .unwrap();
    w.join().unwrap();
    drop(server);
    assert!(!key_file.exists(), "the key goes with the server");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hooks_program_is_known_as_such() {
    let dir = test_dir("hook");
    let server = Server::start_opts(&dir, Arc::new(|| {}), quick()).unwrap();
    let (w, seen) = fake_window(server.calls.clone());
    let (mut r, mut wr) = raw(server.path());
    send(
        &mut wr,
        r#"{"v":3,"id":1,"cmd":"hello","hook":"Upload picks"}"#,
    );
    assert_eq!(read(&mut r)["ok"], true);
    let mark = json!({"v": 3, "id": 2, "cmd": "mark", "files": [crate::remote::test_abs("/a.jpg")], "flag": "pick"});
    send(&mut wr, &mark.to_string());
    assert_eq!(read(&mut r)["ok"], true);
    let (mut r2, mut w2) = raw(server.path());
    send(&mut w2, r#"{"v":3,"id":1,"cmd":"state"}"#);
    assert_eq!(read(&mut r2)["ok"], true);
    let s = seen.lock().clone();
    assert!(matches!(&s[0], (Request::Mark { .. }, Origin::Hook(h)) if h == "Upload picks"));
    assert_eq!(s[1], (Request::State, Origin::Ipc));
    drop((r, wr, r2, w2));
    quit(&server, w, &dir);
}

/// Every socket the server opens is a Unix socket: none is TCP or UDP. Checked in a process
/// of its own (this test binary again, running one test), so other tests' sockets (HTTP
/// stand-ins) cannot muddle it.
#[cfg(target_os = "linux")]
#[test]
fn nothing_listens_on_the_network() {
    if std::env::var_os("SPECULAR_NET_PROBE").is_some() {
        return;
    }
    let exe = std::env::current_exe().unwrap();
    let out = std::process::Command::new(exe)
        .args([
            "--exact",
            "remote::tests_v3::network_probe",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("SPECULAR_NET_PROBE", "1")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{text}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("1 passed"), "{text}");
}

/// The other half of [`nothing_listens_on_the_network`] (does nothing unless asked).
#[cfg(target_os = "linux")]
#[test]
fn network_probe() {
    if std::env::var_os("SPECULAR_NET_PROBE").is_none() {
        return;
    }
    let dir = test_dir("net");
    let server = Server::start_opts(&dir, Arc::new(|| {}), quick()).unwrap();
    let (w, _) = fake_window(server.calls.clone());
    let mut c = Client::connect(server.path()).unwrap();
    c.hello().unwrap();
    let mut ev = c.subscribe(&["*"]).unwrap();
    server.bus().publish(&marked(1), &user());
    ev.next().unwrap().unwrap();
    // Our sockets, by inode.
    let mut ours = Vec::new();
    for e in std::fs::read_dir("/proc/self/fd").unwrap().flatten() {
        if let Ok(t) = std::fs::read_link(e.path()) {
            let t = t.to_string_lossy().into_owned();
            if let Some(n) = t.strip_prefix("socket:[").and_then(|x| x.strip_suffix(']')) {
                ours.push(n.to_owned());
            }
        }
    }
    assert!(ours.len() >= 3, "listener and both ends: {ours:?}");
    let inodes = |f: &str, col: usize| -> Vec<String> {
        // Bytes, not text: abstract socket names may be anything.
        let b = std::fs::read(format!("/proc/self/net/{f}")).unwrap_or_default();
        String::from_utf8_lossy(&b)
            .lines()
            .skip(1)
            .filter_map(|l| l.split_whitespace().nth(col).map(str::to_owned))
            .collect()
    };
    for f in ["tcp", "tcp6", "udp", "udp6", "raw", "raw6"] {
        let net = inodes(f, 9);
        let hit: Vec<&String> = ours.iter().filter(|n| net.contains(n)).collect();
        assert!(hit.is_empty(), "a {f} socket: {hit:?}");
    }
    assert!(
        std::fs::read("/proc/self/net/tcp").is_ok(),
        "the TCP table can be read"
    );
    let unix = inodes("unix", 6);
    let other: Vec<&String> = ours.iter().filter(|n| !unix.contains(n)).collect();
    assert!(
        other.is_empty(),
        "every socket is a Unix one: {other:?} of {ours:?}"
    );
    quit(&server, w, &dir);
}
