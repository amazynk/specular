//! Service threads behind a panic boundary: long-lived loops (folder and settings watches,
//! card polling, the stall watchdog, read-ahead, platform listeners) that must not end
//! silently.
//!
//! [`supervise`] runs a service's body on the calling thread. When the body returns, so does
//! `supervise`. When it panics, the panic is logged (the panic hook has logged where, this
//! says what stopped and what happens next), `died` is called so the owner can make its state
//! honest (cancel a gesture in flight, clear a busy flag, drop a stale level), and the
//! [`Policy`] decides: [`Policy::Stop`] ends the service, [`Policy::Restart`] runs the body
//! again after a back-off that doubles up to its maximum (and starts over once the body ran
//! for longer than that maximum).
//!
//! This is the lowest-level form of `specular_core::isolate::run`, for crates below
//! `specular-core` (`specular-platform`, `specular-settings`, `specular-sandbox`, this one);
//! job boundaries above it use `isolate::run`.
//!
//! Test hook: [`arm`] makes the next [`trip`] on a thread of that name panic (debug builds;
//! release builds compile both to nothing).

use std::panic::AssertUnwindSafe;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// How a service's back-off grows: `first` after the first panic, doubling up to `max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Backoff {
    pub first: Duration,
    pub max: Duration,
}

/// What happens after a service's body panicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// The service ends (its owner shows or logs that it stopped).
    Stop,
    /// The body runs again after the back-off.
    Restart(Backoff),
}

impl Policy {
    /// The usual restart: 1 s, doubling to 60 s.
    pub const RESTART: Policy = Policy::Restart(Backoff {
        first: Duration::from_secs(1),
        max: Duration::from_secs(60),
    });
}

/// One panic of a service, as `died` sees it.
#[derive(Clone, Debug)]
pub struct Death<'a> {
    /// The service (its thread name).
    pub service: &'a str,
    /// What the panic said.
    pub message: &'a str,
    /// Panics of this service so far, this one included.
    pub deaths: u32,
    /// When the body runs again; `None` when the service stops.
    pub restart_in: Option<Duration>,
}

/// Panics per service since the process started (for diagnostics and tests).
static DEATHS: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());

/// How often `service` has panicked in this process (counted after its `died` ran).
pub fn deaths(service: &str) -> u32 {
    DEATHS
        .lock()
        .iter()
        .find(|(s, _)| s == service)
        .map_or(0, |(_, n)| *n)
}

fn count(service: &str) {
    let mut d = DEATHS.lock();
    match d.iter_mut().find(|(s, _)| s == service) {
        Some((_, n)) => *n += 1,
        None => d.push((service.to_owned(), 1)),
    }
}

/// Runs `body` for `service` (see the module docs). Returns the body's value when it
/// returned, `None` when the service stopped after a panic or `quit` said so during a
/// back-off (checked every 50 ms).
pub fn supervise<T>(
    service: &str,
    policy: Policy,
    quit: impl Fn() -> bool,
    mut body: impl FnMut() -> T,
    mut died: impl FnMut(&Death<'_>),
) -> Option<T> {
    let mut deaths = 0u32;
    let mut delay = match policy {
        Policy::Restart(b) => b.first,
        Policy::Stop => Duration::ZERO,
    };
    loop {
        let t0 = Instant::now();
        let payload = match std::panic::catch_unwind(AssertUnwindSafe(&mut body)) {
            Ok(v) => return Some(v),
            Err(p) => p,
        };
        let message = if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_owned()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "panic with a non-text payload".to_owned()
        };
        // Dropping a payload runs foreign code (its Drop) that could panic again.
        std::mem::forget(payload);
        deaths += 1;
        let restart_in = match policy {
            Policy::Stop => None,
            Policy::Restart(b) => {
                if t0.elapsed() >= b.max {
                    // It ran well for a while: this is a new problem, not a loop.
                    delay = b.first;
                }
                Some(delay)
            }
        };
        match restart_in {
            Some(d) => crate::error!(target: crate::JOBS,
                "service {service} stopped: {message}; restarting in {} s", d.as_secs_f32()),
            None => crate::error!(target: crate::JOBS,
                "service {service} stopped: {message}; not restarted"),
        }
        died(&Death {
            service,
            message: &message,
            deaths,
            restart_in,
        });
        // Counted once the owner has made its state honest (tests wait for this).
        count(service);
        let (Some(wait), Policy::Restart(b)) = (restart_in, policy) else {
            return None;
        };
        let until = Instant::now() + wait;
        loop {
            if quit() {
                return None;
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            std::thread::sleep(left.min(Duration::from_millis(50)));
        }
        delay = (delay * 2).min(b.max);
    }
}

/// Armed test panics: thread name and how many trips are left.
#[cfg(debug_assertions)]
static ARMED: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());

/// Test hook: the next `times` calls of [`trip`] on a thread named `thread` panic (debug
/// builds only; a no-op in release builds).
pub fn arm(thread: &str, times: u32) {
    #[cfg(debug_assertions)]
    {
        let mut a = ARMED.lock();
        a.retain(|(t, _)| t != thread);
        if times > 0 {
            a.push((thread.to_owned(), times));
        }
    }
    #[cfg(not(debug_assertions))]
    let _ = (thread, times);
}

/// Test hook: panics when [`arm`] armed the current thread's name (debug builds; release
/// builds compile it to nothing). Put it where a service handles one unit of work.
#[inline]
pub fn trip() {
    #[cfg(debug_assertions)]
    {
        let Some(name) = std::thread::current().name().map(str::to_owned) else {
            return;
        };
        let fire = {
            let mut a = ARMED.lock();
            match a.iter_mut().position(|(t, _)| *t == name) {
                Some(i) => {
                    a[i].1 -= 1;
                    if a[i].1 == 0 {
                        a.remove(i);
                    }
                    true
                }
                None => false,
            }
        };
        if fire {
            panic!("test panic in service {name}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    fn quick() -> Policy {
        Policy::Restart(Backoff {
            first: Duration::from_millis(5),
            max: Duration::from_millis(20),
        })
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn restarts_after_a_panic_and_returns_the_value() {
        let t = std::thread::Builder::new()
            .name("test-svc-a".into())
            .spawn(|| {
                arm("test-svc-a", 2);
                let runs = AtomicU32::new(0);
                let mut seen = Vec::new();
                let v = supervise(
                    "test-svc-a",
                    quick(),
                    || false,
                    || {
                        runs.fetch_add(1, Ordering::Relaxed);
                        trip();
                        42
                    },
                    |d| seen.push((d.deaths, d.restart_in.is_some())),
                );
                (v, runs.load(Ordering::Relaxed), seen)
            })
            .unwrap();
        let (v, runs, seen) = t.join().unwrap();
        assert_eq!(v, Some(42));
        assert_eq!(runs, 3);
        assert_eq!(seen, [(1, true), (2, true)]);
        assert_eq!(deaths("test-svc-a"), 2);
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn stop_policy_ends_after_one_panic() {
        let t = std::thread::Builder::new()
            .name("test-svc-b".into())
            .spawn(|| {
                arm("test-svc-b", 1);
                let mut told = None;
                let v = supervise(
                    "test-svc-b",
                    Policy::Stop,
                    || false,
                    trip,
                    |d| told = Some(d.restart_in),
                );
                (v, told)
            })
            .unwrap();
        let (v, told) = t.join().unwrap();
        assert_eq!(v, None);
        assert_eq!(told, Some(None));
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn quit_during_backoff_ends_at_once() {
        let quit = Arc::new(AtomicBool::new(false));
        let q = quit.clone();
        let t0 = Instant::now();
        let t = std::thread::Builder::new()
            .name("test-svc-c".into())
            .spawn(move || {
                supervise(
                    "test-svc-c",
                    Policy::RESTART,
                    || q.load(Ordering::Relaxed),
                    || panic!("always"),
                    |_| q.store(true, Ordering::Relaxed),
                )
            })
            .unwrap();
        assert_eq!(t.join().unwrap(), None::<()>);
        assert!(t0.elapsed() < Duration::from_millis(900));
    }

    #[test]
    fn backoff_doubles_to_its_maximum() {
        let mut waits = Vec::new();
        let n = AtomicU32::new(0);
        supervise(
            "test-svc-d",
            Policy::Restart(Backoff {
                first: Duration::from_millis(10),
                max: Duration::from_millis(40),
            }),
            || false,
            || {
                if n.fetch_add(1, Ordering::Relaxed) < 4 {
                    panic!("again");
                }
            },
            |d| waits.push(d.restart_in.unwrap().as_millis()),
        );
        assert_eq!(waits, [10, 20, 40, 40]);
    }
}
