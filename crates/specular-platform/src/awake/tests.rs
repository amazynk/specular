use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;

/// A backend that records its calls.
#[derive(Clone, Default)]
struct Fake(Arc<Mutex<Vec<&'static str>>>);

impl Backend for Fake {
    fn hold(&mut self, _reason: &str) -> bool {
        self.0.lock().unwrap().push("hold");
        true
    }
    fn release(&mut self) {
        self.0.lock().unwrap().push("release");
    }
}

fn wait(a: &Awake) -> bool {
    a.seen
        .as_ref()
        .unwrap()
        .recv_timeout(Duration::from_secs(10))
        .expect("the worker answers")
}

#[test]
fn holds_while_on_and_lets_go_on_drop() {
    let f = Fake::default();
    let f2 = f.clone();
    let mut a = Awake::with(Some(Box::new(move || Box::new(f2) as Box<dyn Backend>)));
    a.set(true);
    assert!(wait(&a));
    // Again: no second hold.
    a.set(true);
    a.set(false);
    assert!(!wait(&a));
    a.set(true);
    assert!(wait(&a));
    drop(a);
    let t0 = std::time::Instant::now();
    while f.0.lock().unwrap().len() < 4 && t0.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(*f.0.lock().unwrap(), ["hold", "release", "hold", "release"]);
}

#[test]
fn tests_never_reach_the_system() {
    let mut a = Awake::new();
    a.set(true);
    assert!(a.is_on());
    assert!(a.tx.is_none());
}

#[cfg(all(unix, not(target_os = "macos")))]
mod bus {
    use std::collections::HashMap;

    use zbus::zvariant::{ObjectPath, OwnedValue};

    use super::*;
    use crate::test_bus::Bus;

    #[derive(Clone, Default)]
    struct Seen(Arc<Mutex<Vec<String>>>);

    struct FakeSaver(Seen);

    #[zbus::interface(name = "org.freedesktop.ScreenSaver")]
    impl FakeSaver {
        fn inhibit(&self, app: String, reason: String) -> u32 {
            self.0
                .0
                .lock()
                .unwrap()
                .push(format!("inhibit {app}: {reason}"));
            42
        }
        fn un_inhibit(&self, cookie: u32) {
            self.0.0.lock().unwrap().push(format!("uninhibit {cookie}"));
        }
    }

    struct FakePortal(Seen);

    #[zbus::interface(name = "org.freedesktop.portal.Inhibit")]
    impl FakePortal {
        async fn inhibit(
            &self,
            #[zbus(object_server)] server: &zbus::ObjectServer,
            window: String,
            flags: u32,
            options: HashMap<String, OwnedValue>,
        ) -> ObjectPath<'static> {
            let reason = String::try_from(options["reason"].try_clone().unwrap()).unwrap();
            self.0
                .0
                .lock()
                .unwrap()
                .push(format!("portal {window:?} {flags} {reason}"));
            let path =
                ObjectPath::try_from("/org/freedesktop/portal/desktop/request/1_1/t").unwrap();
            server.at(&path, FakeRequest(self.0.clone())).await.unwrap();
            path
        }
    }

    struct FakeRequest(Seen);

    #[zbus::interface(name = "org.freedesktop.portal.Request")]
    impl FakeRequest {
        fn close(&self) {
            self.0.0.lock().unwrap().push("close".into());
        }
    }

    fn settle(seen: &Seen, n: usize) -> Vec<String> {
        let t0 = std::time::Instant::now();
        while seen.0.lock().unwrap().len() < n && t0.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(5));
        }
        seen.0.lock().unwrap().clone()
    }

    #[test]
    fn screensaver_inhibit_and_release() {
        let Some(bus) = Bus::start("aw1") else {
            eprintln!("no dbus-daemon: skipped");
            return;
        };
        let seen = Seen::default();
        let _server = zbus::blocking::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at("/org/freedesktop/ScreenSaver", FakeSaver(seen.clone()))
            .unwrap()
            .name("org.freedesktop.ScreenSaver")
            .unwrap()
            .build()
            .unwrap();
        let addr = bus.address.clone();
        let mut a = Awake::with(Some(Box::new(move || {
            Box::new(linux::Bus::at(&addr)) as Box<dyn Backend>
        })));
        a.set(true);
        assert!(wait(&a));
        a.set(false);
        assert!(!wait(&a));
        assert_eq!(
            settle(&seen, 2),
            ["inhibit Specular: Playing a video", "uninhibit 42"]
        );
    }

    #[test]
    fn portal_when_no_screensaver() {
        let Some(bus) = Bus::start("aw2") else {
            eprintln!("no dbus-daemon: skipped");
            return;
        };
        let seen = Seen::default();
        let _server = zbus::blocking::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at("/org/freedesktop/portal/desktop", FakePortal(seen.clone()))
            .unwrap()
            .name("org.freedesktop.portal.Desktop")
            .unwrap()
            .build()
            .unwrap();
        let addr = bus.address.clone();
        let mut a = Awake::with(Some(Box::new(move || {
            Box::new(linux::Bus::at(&addr)) as Box<dyn Backend>
        })));
        a.set(true);
        assert!(wait(&a));
        // The viewer closes while playing: the hold is let go.
        drop(a);
        assert_eq!(settle(&seen, 2), ["portal \"\" 8 Playing a video", "close"]);
    }

    #[test]
    fn nothing_answers() {
        let Some(bus) = Bus::start("aw3") else {
            eprintln!("no dbus-daemon: skipped");
            return;
        };
        let _ = bus.connect();
        let addr = bus.address.clone();
        let mut a = Awake::with(Some(Box::new(move || {
            Box::new(linux::Bus::at(&addr)) as Box<dyn Backend>
        })));
        a.set(true);
        assert!(!wait(&a), "no hold taken");
    }
}
