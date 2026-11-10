#[test]
fn tests_never_reach_the_system() {
    assert!(super::MediaControls::start(Box::new(|| {})).is_none());
}

#[test]
fn file_urls() {
    use std::path::Path;
    assert_eq!(
        super::file_url(Path::new("/home/a/Now playing/art-1.png")),
        "file:///home/a/Now%20playing/art-1.png"
    );
    assert_eq!(
        super::file_url(Path::new("/x/\u{5e9}#?.png")),
        "file:///x/%D7%A9%23%3F.png"
    );
    assert_eq!(
        super::file_url(Path::new(r"C:\Users\a\art.png")),
        "file:///C:/Users/a/art.png"
    );
}

#[cfg(all(unix, not(target_os = "macos")))]
mod mpris {
    use std::time::{Duration, Instant};

    use zbus::zvariant::{OwnedValue, Value};

    use super::super::mpris::{PATH, start};
    use super::super::{MediaEvent, NowPlaying};
    use crate::test_bus::Bus;

    fn name() -> String {
        format!(
            "org.mpris.MediaPlayer2.specular.instance{}",
            std::process::id()
        )
    }

    fn get(conn: &zbus::blocking::Connection, iface: &str, prop: &str) -> OwnedValue {
        let m = conn
            .call_method(
                Some(name().as_str()),
                PATH,
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &(iface, prop),
            )
            .unwrap();
        m.body().deserialize::<OwnedValue>().unwrap()
    }

    fn wait<T>(mut f: impl FnMut() -> Option<T>) -> T {
        let t0 = Instant::now();
        loop {
            if let Some(v) = f() {
                return v;
            }
            assert!(t0.elapsed() < Duration::from_secs(10), "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn presses_come_back_and_state_is_read() {
        let Some(bus) = Bus::start("mp1") else {
            eprintln!("no dbus-daemon: skipped");
            return;
        };
        let mut c = start(Some(&bus.address), Box::new(|| {})).expect("served");
        let client = bus.connect();
        let player = "org.mpris.MediaPlayer2.Player";
        for (method, want) in [
            ("PlayPause", MediaEvent::PlayPause),
            ("Next", MediaEvent::Next),
            ("Previous", MediaEvent::Previous),
        ] {
            client
                .call_method(Some(name().as_str()), PATH, Some(player), method, &())
                .unwrap();
            let got = wait(|| c.poll().into_iter().next());
            assert_eq!(got, want);
        }
        client
            .call_method(
                Some(name().as_str()),
                PATH,
                Some(player),
                "Seek",
                &(-5_000_000i64,),
            )
            .unwrap();
        assert_eq!(
            wait(|| c.poll().into_iter().next()),
            MediaEvent::Seek(-5_000_000)
        );
        let id =
            zbus::zvariant::ObjectPath::try_from("/app/specular/Specular/track/current").unwrap();
        client
            .call_method(
                Some(name().as_str()),
                PATH,
                Some(player),
                "SetPosition",
                &(id, 7_000_000i64),
            )
            .unwrap();
        assert_eq!(
            wait(|| c.poll().into_iter().next()),
            MediaEvent::SetPosition(7_000_000)
        );

        assert_eq!(
            String::try_from(get(&client, player, "PlaybackStatus")).unwrap(),
            "Stopped"
        );
        c.update(NowPlaying {
            playing: Some(true),
            title: "Holiday.mkv".into(),
            length_us: 90_000_000,
            position_us: 12_000_000,
            art: Some("/tmp/x y/art.png".into()),
            can_next: true,
            can_previous: false,
        });
        wait(|| {
            (String::try_from(get(&client, player, "PlaybackStatus")).ok()? == "Playing")
                .then_some(())
        });
        assert_eq!(
            i64::try_from(get(&client, player, "Position")).unwrap(),
            12_000_000
        );
        assert!(bool::try_from(get(&client, player, "CanGoNext")).unwrap());
        let meta = get(&client, player, "Metadata");
        let Value::Dict(d) = &*meta else {
            panic!("a dict")
        };
        let title: String = d.get::<&str, String>(&"xesam:title").unwrap().unwrap();
        assert_eq!(title, "Holiday.mkv");
        let art: String = d.get::<&str, String>(&"mpris:artUrl").unwrap().unwrap();
        assert_eq!(art, "file:///tmp/x%20y/art.png");
        let identity =
            String::try_from(get(&client, "org.mpris.MediaPlayer2", "Identity")).unwrap();
        assert_eq!(identity, "Specular");
    }
}
