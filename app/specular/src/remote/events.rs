//! Events (protocol version 3, `docs/ipc.md` §9): the subscribers and the event lines.
//!
//! The window publishes an event once per user act ([`Bus::publish`]); each subscribed
//! connection has a bounded queue ([`QUEUE`] lines) that a thread of its own writes out
//! ([`super::server`]). Publishing never waits: a full queue drops the line and counts it, and
//! the subscriber gets `{"event":"lost","count":N}` once it has caught up. A slow client
//! never holds up the window.
//!
//! A large act goes out in parts (`part`, `parts`), each with at most [`MAX_PART_ITEMS`]
//! entries in its lists and under the line limit; a client joins them by putting each list
//! back together in order.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::{Receiver, Sender, TrySendError};
use parking_lot::Mutex;
use serde_json::{Map, Value, json};

use super::protocol::{self, path_value};

/// Lines queued per subscriber before the next ones are dropped (and counted as lost).
pub const QUEUE: usize = 1024;
/// The most entries one event line's lists hold (files, outputs, failures together).
pub const MAX_PART_ITEMS: usize = 10_000;
/// A part's lists stay under this many bytes, well inside the line limit.
const MAX_PART_BYTES: usize = protocol::MAX_LINE - 64 * 1024;

/// One event as the window describes it, before it is numbered and split.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub name: &'static str,
    pub files: Vec<PathBuf>,
    /// More lists that may be long (`outputs`, `failed`): split across parts like `files`.
    pub lists: Vec<(&'static str, Vec<Value>)>,
    /// Members every part carries (`mark`, `recipe`, `view`…).
    pub members: Map<String, Value>,
}

impl Event {
    pub fn new(name: &'static str, files: Vec<PathBuf>) -> Event {
        Event {
            name,
            files,
            lists: Vec::new(),
            members: Map::new(),
        }
    }

    /// With member `key`.
    pub fn with(mut self, key: &str, v: Value) -> Event {
        self.members.insert(key.to_owned(), v);
        self
    }

    /// With the list `key` (split across parts when long).
    pub fn with_list(mut self, key: &'static str, v: Vec<Value>) -> Event {
        self.lists.push((key, v));
        self
    }
}

/// What a subscriber asked for.
#[derive(Default)]
struct Filter {
    all: bool,
    names: Vec<String>,
}

impl Filter {
    fn wants(&self, name: &str) -> bool {
        self.all || self.names.iter().any(|n| n == name)
    }
}

/// One subscribed connection's queue.
pub struct Subscriber {
    filter: Mutex<Filter>,
    tx: Sender<Arc<str>>,
    lost: AtomicU64,
}

impl Subscriber {
    /// A subscriber and the receiving end its writer reads.
    pub fn new() -> (Arc<Subscriber>, Receiver<Arc<str>>) {
        let (tx, rx) = crossbeam_channel::bounded(QUEUE);
        (
            Arc::new(Subscriber {
                filter: Mutex::new(Filter::default()),
                tx,
                lost: AtomicU64::new(0),
            }),
            rx,
        )
    }

    /// Adds `names` (`*`: every event).
    pub fn add(&self, names: &[String]) {
        let mut f = self.filter.lock();
        for n in names {
            if n == "*" {
                f.all = true;
            } else if !f.names.contains(n) {
                f.names.push(n.clone());
            }
        }
    }

    /// Takes `names` away (`None` or `*`: all). True when nothing is left.
    pub fn remove(&self, names: Option<&[String]>) -> bool {
        let mut f = self.filter.lock();
        match names {
            None => *f = Filter::default(),
            Some(ns) if ns.iter().any(|n| n == "*") => *f = Filter::default(),
            Some(ns) => {
                f.names.retain(|n| !ns.contains(n));
                // "All but these" is not kept: taking a name from `*` leaves the named rest.
                if f.all {
                    f.all = false;
                    f.names = protocol::EVENTS
                        .iter()
                        .filter(|e| !ns.iter().any(|n| n == *e))
                        .map(|e| (*e).to_owned())
                        .collect();
                }
            }
        }
        !f.all && f.names.is_empty()
    }

    /// Lines dropped since the last call (the queue was full).
    pub fn take_lost(&self) -> u64 {
        self.lost.swap(0, Ordering::AcqRel)
    }

    fn offer(&self, line: &Arc<str>) {
        match self.tx.try_send(line.clone()) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.lost.fetch_add(1, Ordering::AcqRel);
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

/// One published line: its text and the object it holds (for hooks).
#[derive(Clone, Debug)]
pub struct Line {
    pub text: Arc<str>,
    pub value: Value,
}

/// The subscribers of one viewer and the event numbering.
#[derive(Default)]
pub struct Bus {
    seq: AtomicU64,
    subs: Mutex<Vec<Arc<Subscriber>>>,
}

impl Bus {
    pub fn new() -> Arc<Bus> {
        Arc::new(Bus::default())
    }

    /// The number of the last event published (0: none yet).
    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::Acquire)
    }

    /// Adds a subscriber; it gets every event numbered after the number returned.
    pub fn attach(&self, s: &Arc<Subscriber>) -> u64 {
        let mut subs = self.subs.lock();
        if !subs.iter().any(|x| Arc::ptr_eq(x, s)) {
            subs.push(s.clone());
        }
        // Read under the lock `publish` numbers under: nothing slips in between.
        self.seq()
    }

    pub fn detach(&self, s: &Arc<Subscriber>) {
        self.subs.lock().retain(|x| !Arc::ptr_eq(x, s));
    }

    /// Subscribed connections.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn subscribers(&self) -> usize {
        self.subs.lock().len()
    }

    /// Whether any subscriber wants `name`.
    pub fn wanted(&self, name: &str) -> bool {
        self.subs.lock().iter().any(|s| s.filter.lock().wants(name))
    }

    /// Numbers `e`, splits it into parts, and queues each part for the subscribers that want
    /// it; returns the lines (also for hooks). `extra` goes into every part (`source`, `hook`).
    pub fn publish(&self, e: &Event, extra: &Map<String, Value>) -> Vec<Line> {
        let parts = split(e);
        let n = parts.len();
        let time = utc_now();
        // Held while numbering and queueing (which never waits), so a subscriber attached
        // meanwhile either gets an event or is told a number past it.
        let guard = self.subs.lock();
        let subs: Vec<&Arc<Subscriber>> = guard
            .iter()
            .filter(|s| s.filter.lock().wants(e.name))
            .collect();
        let mut out = Vec::with_capacity(n);
        for (i, lists) in parts.into_iter().enumerate() {
            let seq = self.seq.fetch_add(1, Ordering::AcqRel) + 1;
            let mut o = Map::new();
            o.insert("v".into(), json!(3));
            o.insert("event".into(), json!(e.name));
            o.insert("seq".into(), json!(seq));
            o.insert("time".into(), json!(time));
            for (k, v) in lists {
                o.insert(k.into(), Value::Array(v));
            }
            for (k, v) in e.members.iter().chain(extra) {
                o.insert(k.clone(), v.clone());
            }
            if n > 1 {
                o.insert("part".into(), json!(i + 1));
                o.insert("parts".into(), json!(n));
            }
            let value = Value::Object(o);
            let text: Arc<str> = value.to_string().into();
            for s in &subs {
                s.offer(&text);
            }
            out.push(Line { text, value });
        }
        drop(subs);
        drop(guard);
        out
    }
}

/// `files` and the other lists of `e` in parts: each with at most [`MAX_PART_ITEMS`] entries
/// and [`MAX_PART_BYTES`] of them; every part names every list (empty where it has none).
fn split(e: &Event) -> Vec<Vec<(&'static str, Vec<Value>)>> {
    let files: Vec<Value> = e.files.iter().map(|p| path_value(p)).collect();
    let lists: Vec<(&'static str, &[Value])> = std::iter::once(("files", files.as_slice()))
        .chain(e.lists.iter().map(|(k, v)| (*k, v.as_slice())))
        .collect();
    let empty = || -> Vec<(&'static str, Vec<Value>)> {
        lists.iter().map(|(k, _)| (*k, Vec::new())).collect()
    };
    let mut parts = vec![empty()];
    let (mut items, mut bytes) = (0usize, 0usize);
    for (li, (_, list)) in lists.iter().enumerate() {
        for v in list.iter() {
            let len = v.to_string().len() + 1;
            if items > 0 && (items >= MAX_PART_ITEMS || bytes + len > MAX_PART_BYTES) {
                parts.push(empty());
                items = 0;
                bytes = 0;
            }
            if let Some(p) = parts.last_mut() {
                p[li].1.push(v.clone());
            }
            items += 1;
            bytes += len;
        }
    }
    parts
}

/// The `ping` line: sent to a subscriber after a minute without events.
pub fn ping(seq: u64) -> String {
    json!({"v": 3, "event": "ping", "seq": seq, "time": utc_now()}).to_string()
}

/// The `lost` line: `count` events were dropped because the client read too slowly.
pub fn lost(count: u64, seq: u64) -> String {
    json!({"v": 3, "event": "lost", "count": count, "seq": seq, "time": utc_now()}).to_string()
}

/// Now as RFC 3339 in UTC, to the millisecond (`2026-11-08T10:12:03.120Z`).
pub fn utc_now() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    utc_text(d.as_millis() as i64)
}

/// Milliseconds since 1970 as RFC 3339 in UTC.
pub fn utc_text(ms: i64) -> String {
    let (secs, milli) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days to the civil date (proleptic Gregorian), after Howard Hinnant's method.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{milli:03}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(utc_text(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(utc_text(1_794_133_923_120), "2026-11-08T10:32:03.120Z");
        assert_eq!(utc_text(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(utc_text(-1), "1969-12-31T23:59:59.999Z");
    }

    #[test]
    fn one_act_is_one_line_and_big_ones_go_in_parts() {
        let bus = Bus::new();
        let (s, rx) = Subscriber::new();
        s.add(&["marked".into()]);
        bus.attach(&s);
        let files: Vec<PathBuf> = (0..500)
            .map(|i| PathBuf::from(format!("/p/{i}.cr3")))
            .collect();
        let e = Event::new("marked", files).with("mark", json!({"flag": "pick"}));
        let mut extra = Map::new();
        extra.insert("source".into(), json!("user"));
        let lines = bus.publish(&e, &extra);
        assert_eq!(lines.len(), 1, "500 photos are one line");
        let v: Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert_eq!(v["files"].as_array().unwrap().len(), 500);
        assert_eq!(
            (v["event"].clone(), v["seq"].clone(), v["source"].clone()),
            (json!("marked"), json!(1), json!("user"))
        );
        assert!(v.get("part").is_none());
        assert_eq!(v["mark"]["flag"], "pick");
        // Not subscribed to `opened`: nothing queued, but it is numbered.
        bus.publish(&Event::new("opened", vec!["/p/x.jpg".into()]), &extra);
        assert!(rx.try_recv().is_err());
        assert_eq!(bus.seq(), 2);
        // 25 000 files: three parts, in order, each with its own number.
        let many: Vec<PathBuf> = (0..25_000)
            .map(|i| PathBuf::from(format!("/p/{i}.cr3")))
            .collect();
        let lines = bus.publish(&Event::new("marked", many.clone()), &extra);
        assert_eq!(lines.len(), 3);
        let mut joined = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            assert_eq!(l.value["part"], i + 1);
            assert_eq!(l.value["parts"], 3);
            assert_eq!(l.value["seq"], 3 + i);
            assert!(l.text.len() < protocol::MAX_LINE);
            joined.extend(l.value["files"].as_array().unwrap().iter().cloned());
        }
        let want: Vec<Value> = many.iter().map(|p| path_value(p)).collect();
        assert_eq!(joined, want);
        // Long paths: the byte limit splits first.
        let long: Vec<PathBuf> = (0..3000)
            .map(|i| PathBuf::from(format!("/{}/{i}", "x".repeat(1000))))
            .collect();
        let lines = bus.publish(&Event::new("marked", long), &extra);
        assert!(lines.len() >= 3);
        assert!(lines.iter().all(|l| l.text.len() < protocol::MAX_LINE));
        // Other lists follow the files and are split the same way.
        let e = Event::new("exported", vec!["/a".into()])
            .with_list("outputs", vec![json!(["/a", "/o/a.jpg"])])
            .with_list("failed", Vec::new());
        let l = &bus.publish(&e, &extra)[0];
        assert_eq!(l.value["outputs"], json!([["/a", "/o/a.jpg"]]));
        assert_eq!(l.value["failed"], json!([]));
    }

    #[test]
    fn a_full_queue_counts_what_it_drops() {
        let bus = Bus::new();
        let (s, rx) = Subscriber::new();
        s.add(&["*".into()]);
        bus.attach(&s);
        let t = std::time::Instant::now();
        for i in 0..10_000 {
            bus.publish(
                &Event::new("opened", vec![PathBuf::from(format!("/p/{i}"))]),
                &Map::new(),
            );
        }
        assert!(
            t.elapsed() < std::time::Duration::from_secs(5),
            "never waits"
        );
        assert_eq!(rx.len(), QUEUE);
        assert_eq!(s.take_lost(), (10_000 - QUEUE) as u64);
        assert_eq!(s.take_lost(), 0);
        let first: Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert_eq!(first["seq"], 1);
    }

    #[test]
    fn filters() {
        let (s, _rx) = Subscriber::new();
        s.add(&["marked".into(), "opened".into()]);
        assert!(!s.remove(Some(&["opened".into()])));
        assert!(s.filter.lock().wants("marked") && !s.filter.lock().wants("opened"));
        assert!(s.remove(Some(&["marked".into()])), "nothing left");
        s.add(&["*".into()]);
        assert!(!s.remove(Some(&["opened".into()])));
        let f = s.filter.lock();
        assert!(f.wants("exported") && !f.wants("opened"));
        drop(f);
        assert!(s.remove(None));
        let v: Value = serde_json::from_str(&lost(3, 9)).unwrap();
        assert_eq!(
            (v["event"].clone(), v["count"].clone()),
            (json!("lost"), json!(3))
        );
        let v: Value = serde_json::from_str(&ping(9)).unwrap();
        assert!(specular_client::protocol::CONTROL_EVENTS.contains(&v["event"].as_str().unwrap()));
    }
}
