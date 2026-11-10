//! Power source, battery level and thermal state on Linux.
//!
//! * `/sys/class/power_supply`: AC adapters (`Mains`, `USB`) and system batteries (status and
//!   level). Peripheral batteries (mice, headsets: `scope` = `Device`) are ignored.
//! * `/sys/class/thermal`: each enabled zone's temperature against its trip points. At or past a
//!   `passive` trip the kernel is already throttling; `hot` and `critical` come before shutdown.
//! * power-profiles-daemon (or tuned-ppd) over D-Bus: the `power-saver` profile counts as low-power
//!   mode, and `PerformanceDegraded` = `high-operating-temperature` as thermal pressure.
//!
//! [`state`] is called on hot paths (prefetch planning), so it returns a cached sample. The sysfs
//! part is read inline the first time (a few dozen small files); refreshes, and every D-Bus call,
//! happen on a helper thread that only runs when a caller found the sample stale, so nothing
//! wakes up while the app is idle.

use std::path::Path;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::{PowerState, Thermal};

/// How old a sample may be before a caller asks for a new one.
const STALE: Duration = Duration::from_secs(5);

/// Within this many m°C below a zone's `passive` trip the state is [`Thermal::Fair`].
const FAIR_MARGIN: i64 = 10_000;

/// Trip points at or below this (m°C) are placeholders (many `x86_pkg_temp` zones report 0).
const MIN_TRIP: i64 = 20_000;

struct Cache {
    sample: Option<(PowerState, Instant)>,
    refreshing: bool,
}

static CACHE: Mutex<Cache> = Mutex::new(Cache {
    sample: None,
    refreshing: false,
});
static WAKE: Condvar = Condvar::new();

pub(crate) fn state() -> PowerState {
    let now = Instant::now();
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let first = c.sample.is_none();
    let (s, at) = *c
        .sample
        .get_or_insert_with(|| (read_sysfs(Path::new("/sys")), now));
    if (first || now.duration_since(at) >= STALE) && !c.refreshing {
        // Stale, or the first call (whose sample has no power profile yet): refresh in the
        // background. The caller gets the last sample now.
        c.refreshing = true;
        drop(c);
        start_helper();
        WAKE.notify_one();
    }
    s
}

fn start_helper() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        // A panic costs one refresh: `refreshing` is cleared (or no caller would ask again
        // and the sample would stay old for good) and the helper restarts after a back-off.
        let spawned = specular_thread::builder("plat-power").spawn(|| {
            specular_log::service::supervise(
                "plat-power",
                specular_log::service::Policy::RESTART,
                || false,
                helper,
                |_| CACHE.lock().unwrap_or_else(|e| e.into_inner()).refreshing = false,
            );
        });
        if spawned.is_err() {
            // No helper: `refreshing` stays set, and with no sample every call reads sysfs
            // inline.
            CACHE.lock().unwrap_or_else(|e| e.into_inner()).sample = None;
        }
    });
}

fn helper() {
    let mut profiles = Profiles::default();
    loop {
        {
            let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
            while !c.refreshing {
                c = WAKE.wait(c).unwrap_or_else(|e| e.into_inner());
            }
        }
        specular_log::service::trip();
        let mut s = read_sysfs(Path::new("/sys"));
        if let Some((saver, hot)) = profiles.read() {
            s.low_power |= saver;
            if hot {
                s.thermal = s.thermal.max(Thermal::Serious);
            }
        }
        let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        c.sample = Some((s, Instant::now()));
        c.refreshing = false;
    }
}

/// The power and thermal state from a sysfs tree rooted at `root` (normally `/sys`).
pub(crate) fn read_sysfs(root: &Path) -> PowerState {
    let (on_battery, battery_percent) = supplies(&root.join("class/power_supply"));
    PowerState {
        thermal: thermal(&root.join("class/thermal")),
        low_power: false,
        on_battery,
        battery_percent,
    }
}

fn read(dir: &Path, file: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(file))
        .ok()
        .map(|s| s.trim().to_owned())
}

fn read_i64(dir: &Path, file: &str) -> Option<i64> {
    read(dir, file)?.parse().ok()
}

/// Whether the machine runs on battery, and the combined level of its system batteries.
fn supplies(dir: &Path) -> (bool, Option<u8>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (false, None);
    };
    let (mut batteries, mut ac_online, mut discharging, mut charging) = (0, false, false, false);
    let (mut now_sum, mut full_sum, mut energy_ok) = (0i64, 0i64, true);
    let mut capacities = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        match read(&p, "type").as_deref() {
            Some("Mains" | "USB") => ac_online |= read(&p, "online").as_deref() == Some("1"),
            Some("Battery") if read(&p, "scope").as_deref() != Some("Device") => {
                // `present` is missing on some drivers; absent means present.
                if read(&p, "present").as_deref() == Some("0") {
                    continue;
                }
                batteries += 1;
                match read(&p, "status").as_deref() {
                    Some("Discharging") => discharging = true,
                    Some("Charging" | "Full") => charging = true,
                    _ => {}
                }
                if let Some(c) = read_i64(&p, "capacity") {
                    capacities.push(c.clamp(0, 100));
                }
                let pair = read_i64(&p, "energy_now")
                    .zip(read_i64(&p, "energy_full"))
                    .or_else(|| read_i64(&p, "charge_now").zip(read_i64(&p, "charge_full")));
                match pair {
                    Some((n, f)) if f > 0 => {
                        now_sum += n.max(0);
                        full_sum += f;
                    }
                    _ => energy_ok = false,
                }
            }
            _ => {}
        }
    }
    if batteries == 0 {
        return (false, None);
    }
    // A battery that says it is discharging wins over an adapter entry (a USB-C port can report
    // "online" while supplying too little); otherwise no adapter online and not charging.
    let on_battery = discharging || (!ac_online && !charging);
    let percent = if energy_ok && full_sum > 0 {
        Some((now_sum * 100 / full_sum).clamp(0, 100) as u8)
    } else if !capacities.is_empty() {
        Some((capacities.iter().sum::<i64>() / capacities.len() as i64) as u8)
    } else {
        None
    };
    (on_battery, percent)
}

/// The hottest enabled zone's state against its own trip points.
fn thermal(dir: &Path) -> Thermal {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Thermal::Nominal;
    };
    let mut worst = Thermal::Nominal;
    for e in entries.flatten() {
        if !e.file_name().to_string_lossy().starts_with("thermal_zone") {
            continue;
        }
        let z = e.path();
        if read(&z, "mode").as_deref() == Some("disabled") {
            continue;
        }
        let Some(temp) = read_i64(&z, "temp") else {
            continue;
        };
        let (mut passive, mut hot) = (None::<i64>, None::<i64>);
        for i in 0..32 {
            let Some(kind) = read(&z, &format!("trip_point_{i}_type")) else {
                break;
            };
            let Some(t) = read_i64(&z, &format!("trip_point_{i}_temp")).filter(|&t| t > MIN_TRIP)
            else {
                continue;
            };
            match kind.as_str() {
                "passive" => passive = Some(passive.map_or(t, |p| p.min(t))),
                "hot" | "critical" => hot = Some(hot.map_or(t, |h| h.min(t))),
                _ => {} // `active` trips switch fans on; not pressure.
            }
        }
        let zone = if hot.is_some_and(|h| temp >= h) {
            Thermal::Critical
        } else if passive.is_some_and(|p| temp >= p) {
            Thermal::Serious
        } else if passive.is_some_and(|p| temp >= p - FAIR_MARGIN)
            || hot.is_some_and(|h| temp >= h - 2 * FAIR_MARGIN)
        {
            Thermal::Fair
        } else {
            Thermal::Nominal
        };
        worst = worst.max(zone);
    }
    worst
}

/// power-profiles-daemon over the system bus (the `zbus` connection colord lookups use too).
#[derive(Default)]
struct Profiles {
    conn: Option<zbus::blocking::Connection>,
    /// The bus or the daemon is missing: stop asking.
    gone: bool,
}

impl Profiles {
    /// (`power-saver` profile active, performance degraded by temperature).
    fn read(&mut self) -> Option<(bool, bool)> {
        if self.gone {
            return None;
        }
        if self.conn.is_none() {
            match zbus::blocking::Connection::system() {
                Ok(c) => self.conn = Some(c),
                Err(e) => {
                    specular_log::debug!(target: specular_log::STARTUP, "power profiles: no system bus: {e}");
                    self.gone = true;
                    return None;
                }
            }
        }
        let conn = self.conn.as_ref()?;
        // The current name first, then the one before power-profiles-daemon 0.20.
        for (dest, path) in [
            (
                "org.freedesktop.UPower.PowerProfiles",
                "/org/freedesktop/UPower/PowerProfiles",
            ),
            ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"),
        ] {
            // No property cache: a caching proxy subscribes to PropertiesChanged (AddMatch,
            // GetAll, RemoveMatch) on every poll, waking the bus threads far more than two Gets.
            let Ok(proxy) = zbus::blocking::proxy::Builder::<zbus::blocking::Proxy>::new(conn)
                .destination(dest)
                .and_then(|b| b.path(path))
                .and_then(|b| b.interface(dest))
                .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No))
                .and_then(|b| b.build())
            else {
                continue;
            };
            let Ok(profile) = proxy.get_property::<String>("ActiveProfile") else {
                continue;
            };
            let degraded = proxy
                .get_property::<String>("PerformanceDegraded")
                .unwrap_or_default();
            return Some((
                profile == "power-saver",
                degraded.contains("high-operating-temperature"),
            ));
        }
        specular_log::debug!(target: specular_log::STARTUP, "power profiles: daemon not running");
        self.gone = true;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A panic in the helper clears `refreshing`, so the next stale sample asks again, and
    /// the restarted helper answers.
    #[test]
    fn helper_restarts_after_a_panic() {
        let deadline = Instant::now() + Duration::from_secs(15);
        specular_log::service::arm("plat-power", 1);
        {
            // As at the first call: no sample, so `state` asks the helper for one.
            let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
            c.sample = None;
            c.refreshing = false;
        }
        let _ = state();
        while specular_log::service::deaths("plat-power") == 0 {
            assert!(Instant::now() < deadline, "the helper never panicked");
            {
                // A refresh another test asked for may have finished ours: ask again.
                let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
                if !c.refreshing {
                    c.refreshing = true;
                    WAKE.notify_all();
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // `died` cleared `refreshing`; ask for a refresh as `state` would.
        CACHE.lock().unwrap_or_else(|e| e.into_inner()).refreshing = true;
        WAKE.notify_all();
        while CACHE.lock().unwrap_or_else(|e| e.into_inner()).refreshing {
            assert!(Instant::now() < deadline, "the helper did not restart");
            WAKE.notify_all();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// A fake `/sys` under the test's target directory.
    struct FakeSys(PathBuf);

    impl FakeSys {
        fn new(name: &str) -> FakeSys {
            // Under the build's target directory (never the system temp directory).
            let target = std::env::var_os("CARGO_TARGET_DIR").map_or_else(
                || PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target")),
                PathBuf::from,
            );
            let root = target.join("test-scratch").join("fake-sys").join(name);
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            FakeSys(root)
        }

        fn put(&self, dir: &str, files: &[(&str, &str)]) -> &Self {
            let d = self.0.join(dir);
            std::fs::create_dir_all(&d).unwrap();
            for (f, v) in files {
                std::fs::write(d.join(f), format!("{v}\n")).unwrap();
            }
            self
        }

        fn state(&self) -> PowerState {
            read_sysfs(&self.0)
        }
    }

    const PS: &str = "class/power_supply";

    #[test]
    fn desktop_without_sysfs_entries() {
        let s = FakeSys::new("empty").state();
        assert_eq!(s, PowerState::default());
        // No /sys at all.
        assert_eq!(read_sysfs(Path::new("/nonexistent")), PowerState::default());
    }

    #[test]
    fn laptop_on_ac_and_on_battery() {
        let sys = FakeSys::new("laptop");
        sys.put(&format!("{PS}/AC"), &[("type", "Mains"), ("online", "1")])
            .put(
                &format!("{PS}/BAT0"),
                &[
                    ("type", "Battery"),
                    ("status", "Charging"),
                    ("capacity", "57"),
                    ("scope", "System"),
                ],
            )
            // A wireless mouse: never counts.
            .put(
                &format!("{PS}/hidpp_battery_0"),
                &[
                    ("type", "Battery"),
                    ("scope", "Device"),
                    ("status", "Discharging"),
                    ("capacity", "5"),
                ],
            );
        let s = sys.state();
        assert!(!s.on_battery);
        assert_eq!(s.battery_percent, Some(57));

        sys.put(&format!("{PS}/AC"), &[("online", "0")]).put(
            &format!("{PS}/BAT0"),
            &[("status", "Discharging"), ("capacity", "18")],
        );
        let s = sys.state();
        assert!(s.on_battery);
        assert_eq!(s.battery_percent, Some(18));
        assert!(s.saving());
    }

    #[test]
    fn two_batteries_are_weighted_by_energy() {
        let sys = FakeSys::new("two");
        sys.put(
            &format!("{PS}/BAT0"),
            &[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("energy_now", "10000000"),
                ("energy_full", "20000000"),
                ("capacity", "50"),
            ],
        )
        .put(
            &format!("{PS}/BAT1"),
            &[
                ("type", "Battery"),
                ("status", "Unknown"),
                ("energy_now", "60000000"),
                ("energy_full", "60000000"),
                ("capacity", "100"),
            ],
        );
        let s = sys.state();
        assert!(s.on_battery);
        // 70 of 80 Wh, not the 75% mean of the two capacities.
        assert_eq!(s.battery_percent, Some(87));
    }

    #[test]
    fn usb_c_charger_and_discharging_battery() {
        let sys = FakeSys::new("usbc");
        sys.put(
            &format!("{PS}/ucsi-source-psy-USBC000:001"),
            &[("type", "USB"), ("online", "1")],
        )
        .put(
            &format!("{PS}/BAT0"),
            &[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("capacity", "40"),
            ],
        );
        assert!(
            sys.state().on_battery,
            "a weak charger still drains the battery"
        );
        sys.put(&format!("{PS}/BAT0"), &[("status", "Not charging")]);
        assert!(!sys.state().on_battery, "held at a charge limit on AC");
        // A battery without an AC entry that is not charging: on battery.
        let lone = FakeSys::new("lone");
        lone.put(
            &format!("{PS}/BAT0"),
            &[("type", "Battery"), ("status", "Unknown"), ("present", "1")],
        );
        assert!(lone.state().on_battery);
        // An empty bay.
        let bay = FakeSys::new("bay");
        bay.put(
            &format!("{PS}/BAT1"),
            &[("type", "Battery"), ("present", "0")],
        );
        assert_eq!(bay.state(), PowerState::default());
    }

    fn zone(sys: &FakeSys, n: u32, temp: i64, trips: &[(&str, i64)]) {
        let dir = format!("class/thermal/thermal_zone{n}");
        sys.put(&dir, &[("temp", &temp.to_string()), ("mode", "enabled")]);
        for (i, (kind, t)) in trips.iter().enumerate() {
            sys.put(
                &dir,
                &[
                    (&format!("trip_point_{i}_type"), kind),
                    (&format!("trip_point_{i}_temp"), &t.to_string()),
                ],
            );
        }
    }

    #[test]
    fn thermal_trip_points() {
        let sys = FakeSys::new("thermal");
        // A cooling device directory is not a zone.
        sys.put(
            "class/thermal/cooling_device0",
            &[("type", "Processor"), ("cur_state", "3")],
        );
        // x86_pkg_temp: placeholder passive trip at 0 must not read as "throttling".
        zone(&sys, 0, 45_000, &[("passive", 0)]);
        zone(
            &sys,
            1,
            50_000,
            &[
                ("active", 45_000),
                ("passive", 95_000),
                ("critical", 105_000),
            ],
        );
        assert_eq!(sys.state().thermal, Thermal::Nominal);
        zone(
            &sys,
            1,
            88_000,
            &[
                ("active", 45_000),
                ("passive", 95_000),
                ("critical", 105_000),
            ],
        );
        assert_eq!(sys.state().thermal, Thermal::Fair);
        zone(
            &sys,
            1,
            96_000,
            &[
                ("active", 45_000),
                ("passive", 95_000),
                ("critical", 105_000),
            ],
        );
        assert_eq!(sys.state().thermal, Thermal::Serious);
        zone(
            &sys,
            1,
            106_000,
            &[
                ("active", 45_000),
                ("passive", 95_000),
                ("critical", 105_000),
            ],
        );
        assert_eq!(sys.state().thermal, Thermal::Critical);
        // A zone with only a critical trip: Fair within 20 °C of it.
        zone(&sys, 1, 30_000, &[]);
        zone(&sys, 2, 90_000, &[("critical", 105_000)]);
        assert_eq!(sys.state().thermal, Thermal::Fair);
        // Disabled zones are ignored.
        sys.put("class/thermal/thermal_zone2", &[("mode", "disabled")]);
        assert_eq!(sys.state().thermal, Thermal::Nominal);
    }

    #[test]
    fn cached_state_is_cheap() {
        let first = state();
        let t = Instant::now();
        for _ in 0..1000 {
            let _ = state();
        }
        // Cached: far below the cost of reading sysfs a thousand times.
        assert!(t.elapsed() < Duration::from_millis(50), "{:?}", t.elapsed());
        let _ = first;
    }
}
