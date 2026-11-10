//! The stall watchdog: one warning when the current image stays loading too long.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};

type Dump = Box<dyn Fn() -> Vec<String> + Send + Sync>;
type Sink = Box<dyn Fn(&str) + Send + Sync>;

struct Watch {
    id: u64,
    name: String,
    state: String,
    since: Instant,
    fired: bool,
}

struct State {
    threshold: Duration,
    current: Option<Watch>,
    shutdown: bool,
}

struct Shared {
    state: Mutex<State>,
    cv: Condvar,
    dump: Dump,
    sink: Sink,
}

/// Watches the image being loaded. When it has been loading for longer than the threshold
/// (2 s by default, `performance.stall_warning`), it logs one warning with the image's load
/// state, the image's pending jobs and the whole job queue. The warning goes out at `warn`
/// level, which every component logs unless `SPECULAR_LOG` turns it off, so a stuck load
/// always leaves a trace.
///
/// The thread sleeps without a timeout while nothing is loading: an idle viewer stays idle.
pub struct Watchdog {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Watchdog {
    /// `dump` lists the job queue, one job per line (lines about an image contain
    /// `img=<id> `).
    pub fn spawn(
        threshold: Duration,
        dump: impl Fn() -> Vec<String> + Send + Sync + 'static,
    ) -> Watchdog {
        Watchdog::with_sink(
            threshold,
            dump,
            |msg| crate::warn!(target: crate::JOBS, "{msg}"),
        )
    }

    /// Like [`Watchdog::spawn`], with the warning going to `sink` (tests).
    pub fn with_sink(
        threshold: Duration,
        dump: impl Fn() -> Vec<String> + Send + Sync + 'static,
        sink: impl Fn(&str) + Send + Sync + 'static,
    ) -> Watchdog {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                threshold,
                current: None,
                shutdown: false,
            }),
            cv: Condvar::new(),
            dump: Box::new(dump),
            sink: Box::new(sink),
        });
        let s = shared.clone();
        let thread = specular_thread::builder("wd-load")
            .spawn(move || {
                // A panic in `dump` or `sink` (app code) costs one warning: the watchdog
                // restarts and keeps watching.
                crate::service::supervise(
                    "wd-load",
                    crate::service::Policy::RESTART,
                    || s.state.lock().shutdown,
                    || run(&s),
                    |_| {},
                );
            })
            .ok();
        Watchdog { shared, thread }
    }

    /// Image `id` is loading and has reached `state`. A new id restarts the clock.
    pub fn loading(&self, id: u64, name: &str, state: &str) {
        let mut s = self.shared.state.lock();
        match &mut s.current {
            Some(w) if w.id == id => {
                if w.state != state {
                    w.state = state.to_owned();
                }
            }
            _ => {
                s.current = Some(Watch {
                    id,
                    name: name.to_owned(),
                    state: state.to_owned(),
                    since: Instant::now(),
                    fired: false,
                });
                self.shared.cv.notify_all();
            }
        }
    }

    /// Image `id` is no longer loading (sharp, failed, or no longer shown).
    pub fn done(&self, id: u64) {
        let mut s = self.shared.state.lock();
        if s.current.as_ref().is_some_and(|w| w.id == id) {
            s.current = None;
        }
    }

    /// Nothing is loading.
    pub fn clear(&self) {
        self.shared.state.lock().current = None;
    }

    pub fn set_threshold(&self, threshold: Duration) {
        self.shared.state.lock().threshold = threshold;
        self.shared.cv.notify_all();
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        self.shared.state.lock().shutdown = true;
        self.shared.cv.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(shared: &Shared) {
    let mut s = shared.state.lock();
    loop {
        if s.shutdown {
            return;
        }
        let threshold = s.threshold;
        let Some(w) = s.current.as_mut().filter(|w| !w.fired) else {
            shared.cv.wait(&mut s);
            continue;
        };
        let deadline = w.since + threshold;
        if Instant::now() < deadline {
            shared.cv.wait_until(&mut s, deadline);
            continue;
        }
        w.fired = true;
        let (id, name, state, age) = (w.id, w.name.clone(), w.state.clone(), w.since.elapsed());
        drop(s);
        crate::service::trip();
        let jobs = (shared.dump)();
        let tag = format!("img={id} ");
        let mine: Vec<&String> = jobs.iter().filter(|j| j.contains(&tag)).collect();
        let mut msg = format!(
            "img={id} {name} stalled: loading for {:.1} s, state: {state}; pending jobs for it: {}",
            age.as_secs_f64(),
            if mine.is_empty() {
                "none".to_owned()
            } else {
                mine.len().to_string()
            }
        );
        for j in &mine {
            msg.push_str(&format!("\n    {j}"));
        }
        msg.push_str(&format!("\n  job queue ({} jobs):", jobs.len()));
        for j in &jobs {
            msg.push_str(&format!("\n    {j}"));
        }
        (shared.sink)(&msg);
        s = shared.state.lock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// The tests share the thread name the panic hook is armed for: one at a time.
    static SERIAL: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

    #[test]
    fn fires_once_for_a_stuck_load_with_its_state_and_jobs() {
        let _one = SERIAL.lock();
        let (tx, rx) = mpsc::channel::<String>();
        let tx = parking_lot::Mutex::new(tx);
        // A fake stuck job: the queue has this image's level decode, never finishing.
        let dump = || {
            vec![
                "running  Immediate  img=7 level 1/1 (2.1 s)".to_owned(),
                "queued   Prefetch   img=8 open".to_owned(),
            ]
        };
        let wd = Watchdog::with_sink(Duration::from_millis(60), dump, move |m| {
            tx.lock().send(m.to_owned()).unwrap()
        });
        wd.loading(7, "2.jpeg", "requested");
        wd.loading(7, "2.jpeg", "header 494×360");
        let msg = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("watchdog fired");
        assert!(
            msg.starts_with("img=7 2.jpeg stalled: loading for"),
            "{msg}"
        );
        assert!(msg.contains("state: header 494×360"), "{msg}");
        assert!(
            msg.contains("pending jobs for it: 1\n    running  Immediate  img=7 level 1/1"),
            "{msg}"
        );
        assert!(
            msg.contains("job queue (2 jobs)") && msg.contains("img=8 open"),
            "{msg}"
        );
        // Once per load.
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        // A new image restarts the clock; one that finishes in time stays quiet.
        wd.loading(9, "a.jpg", "requested");
        wd.done(9);
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    }

    /// A panic while reporting (a test hook here; app code in `dump` or `sink` in life) costs
    /// that one warning: the thread restarts and the next stall is reported.
    #[test]
    fn restarts_after_a_panic_while_reporting() {
        let _one = SERIAL.lock();
        let (tx, rx) = mpsc::channel::<String>();
        let tx = parking_lot::Mutex::new(tx);
        crate::service::arm("wd-load", 1);
        let wd = Watchdog::with_sink(Duration::from_millis(30), Vec::new, move |m| {
            tx.lock().send(m.to_owned()).unwrap()
        });
        wd.loading(1, "a.jpg", "requested");
        // The first stall panics: nothing is reported for it.
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
        assert_eq!(crate::service::deaths("wd-load"), 1);
        // After the 1 s back-off the watchdog watches again.
        wd.loading(2, "b.jpg", "requested");
        let msg = rx.recv_timeout(Duration::from_secs(5)).expect("fired");
        assert!(msg.starts_with("img=2 b.jpg stalled"), "{msg}");
        // Dropping it during a back-off returns promptly.
        drop(wd);
    }

    #[test]
    fn quiet_when_loads_finish_in_time() {
        let _one = SERIAL.lock();
        let (tx, rx) = mpsc::channel::<String>();
        let tx = parking_lot::Mutex::new(tx);
        let wd = Watchdog::with_sink(Duration::from_millis(100), Vec::new, move |m| {
            tx.lock().send(m.to_owned()).unwrap()
        });
        for id in 0..5 {
            wd.loading(id, "x", "requested");
            std::thread::sleep(Duration::from_millis(20));
            wd.done(id);
        }
        assert!(rx.recv_timeout(Duration::from_millis(250)).is_err());
        drop(wd);
    }
}
