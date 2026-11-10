//! Debug logging: every event of the crate uses target `jpeg`, the documented debug events are
//! emitted (frame, scan script, path, request, memory, index build, errors with offsets), and
//! no event is emitted when no collector is installed.

mod common;

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use specular_jpeg::*;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

/// Records (level, target, message, other fields) of every event.
struct Collect {
    events: Mutex<Vec<(Level, String, String)>>,
    ids: AtomicU64,
}

struct Text(String);

impl Visit for Text {
    fn record_debug(&mut self, f: &Field, v: &dyn std::fmt::Debug) {
        self.0.push_str(&format!(" {}={v:?}", f.name()));
    }
}

impl Subscriber for Collect {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(self.ids.fetch_add(1, Ordering::Relaxed) + 1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, e: &Event<'_>) {
        let mut t = Text(String::new());
        e.record(&mut t);
        let m = e.metadata();
        self.events
            .lock()
            .unwrap()
            .push((*m.level(), m.target().to_string(), t.0));
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

fn jpeg(w: u32, h: u32, args: &[&str]) -> Option<Vec<u8>> {
    if !common::tools_available() {
        eprintln!("skipping: cjpeg/djpeg/magick not installed");
        return None;
    }
    let src = common::source_ppm(w, h, 5);
    Some(common::cjpeg(
        &src,
        &format!("trace_{w}x{h}_{}", args.join("_")),
        args,
    ))
}

#[test]
fn debug_events_use_target_jpeg() {
    let Some(prog) = jpeg(2048, 1536, &["-quality", "92", "-progressive"]) else {
        return;
    };
    let Some(base) = jpeg(2048, 1536, &["-quality", "92"]) else {
        return;
    };
    let mut bad = prog.clone();
    // A broken SOS header (component count 0) somewhere after the frame.
    let sos = bad.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
    bad[sos + 4] = 0;

    // Without a collector nothing is recorded (and nothing panics).
    decode(&prog, &DecodeOptions::default()).unwrap();

    let c: &'static Collect = Box::leak(Box::new(Collect {
        events: Mutex::new(Vec::new()),
        ids: AtomicU64::new(0),
    }));
    struct Fwd(&'static Collect);
    impl Subscriber for Fwd {
        fn enabled(&self, m: &Metadata<'_>) -> bool {
            self.0.enabled(m)
        }
        fn new_span(&self, a: &Attributes<'_>) -> Id {
            self.0.new_span(a)
        }
        fn record(&self, i: &Id, r: &Record<'_>) {
            self.0.record(i, r)
        }
        fn record_follows_from(&self, a: &Id, b: &Id) {
            self.0.record_follows_from(a, b)
        }
        fn event(&self, e: &Event<'_>) {
            self.0.event(e)
        }
        fn enter(&self, i: &Id) {
            self.0.enter(i)
        }
        fn exit(&self, i: &Id) {
            self.0.exit(i)
        }
    }
    // Global, so that events on pool threads are seen too.
    tracing::subscriber::set_global_default(Fwd(c)).unwrap();

    let region = DecodeOptions {
        scale: Scale::Half,
        region: Some(Region {
            x: 100,
            y: 200,
            w: 300,
            h: 100,
        }),
        ..Default::default()
    };
    decode(&prog, &DecodeOptions::default()).unwrap();
    decode(&base, &region).unwrap();
    build_index(&prog).unwrap();
    assert!(decode(&bad, &DecodeOptions::default()).is_err());
    assert!(decode(&prog[..sos], &DecodeOptions::default()).is_err());

    let ev = c.events.lock().unwrap();
    assert!(!ev.is_empty());
    for (_, target, text) in ev.iter() {
        assert_eq!(target, "jpeg", "event with another target: {text}");
    }
    let has = |lvl: Level, s: &str| ev.iter().any(|(l, _, t)| *l == lvl && t.contains(s));
    for s in [
        "message=frame",
        "message=marker",
        "message=scan",
        "message=path",
        "message=decode request",
        "message=memory",
        "message=index built",
        "message=progressive pipeline",
        "region=Some(Region",
        "message=header parse failed",
        "message=decode failed",
    ] {
        assert!(has(Level::DEBUG, s), "no debug event {s:?}");
    }
    // The broken SOS is reported with its offset.
    let off = format!("offset={sos}");
    assert!(
        ev.iter()
            .any(|(_, _, t)| t.contains("header parse failed") && t.contains(&off)),
        "no error at offset {sos}"
    );
    assert!(has(Level::TRACE, "message=band"), "no per-band trace");
}
