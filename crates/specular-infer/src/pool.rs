//! The CPU engine's worker threads (`ai-infer-N`): started once, parked when idle.
//!
//! An op posts one job (a `Fn` that works through the op's shared queue of pieces) asking
//! for up to `threads - 1` helpers, runs it on the calling thread too, then closes the job
//! and waits for the helpers that joined. Helpers that had not joined by then never do, so
//! a thread the OS has not scheduled yet delays nothing.
//!
//! A worker out of work sleeps on a condition variable until the next job is posted, after
//! spinning on a counter (no system calls) for a configurable time, none by default. rayon's
//! pool, used before, made every worker yield (`sched_yield`) 32 times and steal-scan the
//! other workers' deques after every op before sleeping. At 16 threads on the 8-core,
//! 16-thread test machine (load 8–20), parking at once cut CPU time per run by about a
//! third (MobileNetV2 66 → 43 ms, ViT-Ti 180 → 130 ms; system time 14–32 → 2–8 ms) with
//! wall time the same or lower. Spinning cost CPU without making runs faster: hyperthread
//! siblings of spinning workers run the work slower (20 µs: +15% CPU, 50 µs: +45%, 200 µs:
//! +90% and slower runs). After a run the workers cost nothing either way (about 0.1 ms of
//! CPU per second, from the last sleepers settling).

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// How long an idle worker spins before sleeping, by default: not at all (see above).
pub const DEFAULT_SPIN: Duration = Duration::ZERO;

type Job = dyn Fn() + Sync;

struct Slot {
    id: u64,
    job: &'static Job,
    /// Helpers that may still join.
    wanted: usize,
    /// Helpers running the job now.
    active: usize,
    /// A helper's panic, raised again on the posting thread.
    panic: Option<Box<dyn std::any::Any + Send>>,
}

#[derive(Default)]
struct State {
    slots: Vec<Slot>,
    next_id: u64,
    /// Workers asleep on `wake`.
    asleep: usize,
    /// The pool was dropped: workers exit once no job wants them.
    shut: bool,
}

struct Shared {
    state: Mutex<State>,
    /// Bumped (under the lock) whenever a job is posted; spinning workers watch it.
    posted: AtomicU64,
    wake: Condvar,
    left: Condvar,
    spin: Duration,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// A set of worker threads ops are split over.
pub struct Pool {
    shared: Arc<Shared>,
    workers: usize,
}

static GLOBAL: OnceLock<Pool> = OnceLock::new();

impl Pool {
    /// Starts `workers` threads named `ai-infer-0`… that spin for `spin` when they run out of
    /// work before sleeping. Fewer if the OS refuses to start more.
    pub fn new(workers: usize, spin: Duration) -> Pool {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            posted: AtomicU64::new(0),
            wake: Condvar::new(),
            left: Condvar::new(),
            spin,
        });
        let mut started = 0;
        for i in 0..workers {
            let s = Arc::clone(&shared);
            let ok = specular_thread::builder(format!("ai-infer-{i}"))
                .spawn(move || work(&s))
                .is_ok();
            if !ok {
                break;
            }
            started += 1;
        }
        Pool {
            shared,
            workers: started,
        }
    }

    /// The pool CPU inference runs on: started on first use, with one worker per logical
    /// core unless [`Pool::init_global`] said otherwise.
    pub fn global() -> &'static Pool {
        GLOBAL.get_or_init(|| {
            let n = std::thread::available_parallelism().map_or(1, |n| n.get());
            Pool::new(n, DEFAULT_SPIN)
        })
    }

    /// Sizes the global pool before its first use; false (and nothing changes) if it has
    /// already started.
    pub fn init_global(workers: usize, spin: Duration) -> bool {
        let mut fresh = false;
        GLOBAL.get_or_init(|| {
            fresh = true;
            Pool::new(workers, spin)
        });
        fresh
    }

    /// Worker threads running.
    pub fn workers(&self) -> usize {
        self.workers
    }

    /// Runs `job` on this thread and on up to `helpers` workers that are free while it
    /// runs; returns when every copy has returned. `job` must split its work through
    /// shared state (each copy taking pieces until none are left), so any number of copies
    /// finishes it. A panic in any copy is raised here.
    pub fn run(&self, helpers: usize, job: &(dyn Fn() + Sync)) {
        let helpers = helpers.min(self.workers);
        if helpers == 0 {
            job();
            return;
        }
        // SAFETY: the erased reference is only reachable through the slot posted below,
        // and `Close::drop` (which runs whether `job` returns or unwinds) removes the slot
        // and waits until no helper is running it before this function returns, so no
        // use of it outlives the borrow. Helpers copy it out only under the lock while the
        // slot is open.
        #[allow(unsafe_code)]
        let job_static: &'static Job =
            unsafe { std::mem::transmute::<&(dyn Fn() + Sync + '_), &'static Job>(job) };
        let sh = &*self.shared;
        let id = {
            let mut st = sh.lock();
            let id = st.next_id;
            st.next_id += 1;
            st.slots.push(Slot {
                id,
                job: job_static,
                wanted: helpers,
                active: 0,
                panic: None,
            });
            sh.posted.fetch_add(1, Ordering::Release);
            // Sleepers already woken but not yet running still count as asleep, so this can
            // wake fewer than wanted; the posting thread then does more of the work itself.
            if helpers >= st.asleep {
                sh.wake.notify_all();
            } else {
                for _ in 0..helpers {
                    sh.wake.notify_one();
                }
            }
            id
        };
        let close = Close { sh, id };
        job();
        if let Some(p) = close.finish() {
            resume_unwind(p);
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        // No job can be open: `run` borrows the pool until its helpers have left.
        let mut st = self.shared.lock();
        st.shut = true;
        self.shared.posted.fetch_add(1, Ordering::Release);
        self.shared.wake.notify_all();
    }
}

/// Closes a posted job and waits for its helpers, also when the posting thread unwinds.
struct Close<'a> {
    sh: &'a Shared,
    id: u64,
}

impl Close<'_> {
    /// Closes the job; returns a helper's panic, if any.
    fn finish(self) -> Option<Box<dyn std::any::Any + Send>> {
        let p = self.close();
        std::mem::forget(self);
        p
    }

    fn close(&self) -> Option<Box<dyn std::any::Any + Send>> {
        let mut st = self.sh.lock();
        if let Some(s) = st.slots.iter_mut().find(|s| s.id == self.id) {
            s.wanted = 0;
        }
        loop {
            let i = st.slots.iter().position(|s| s.id == self.id)?;
            if st.slots[i].active == 0 {
                return st.slots.swap_remove(i).panic;
            }
            st = self.sh.left.wait(st).unwrap_or_else(|e| e.into_inner());
        }
    }
}

impl Drop for Close<'_> {
    fn drop(&mut self) {
        // Unwinding from the posting thread's own copy: wait, and drop any helper's panic.
        let _ = self.close();
    }
}

/// A worker: joins open jobs, spins a little when there are none, then sleeps.
fn work(sh: &Shared) {
    loop {
        let mut st = sh.lock();
        let open = st.slots.iter_mut().find(|s| s.wanted > 0).map(|s| {
            s.wanted -= 1;
            s.active += 1;
            (s.id, s.job)
        });
        if let Some((id, job)) = open {
            drop(st);
            let r = catch_unwind(AssertUnwindSafe(job));
            let mut st = sh.lock();
            if let Some(s) = st.slots.iter_mut().find(|s| s.id == id) {
                s.active -= 1;
                if let Err(p) = r {
                    s.panic.get_or_insert(p);
                }
                if s.active == 0 && s.wanted == 0 {
                    sh.left.notify_all();
                }
            }
            continue;
        }
        if st.shut {
            return;
        }
        let seen = sh.posted.load(Ordering::Acquire);
        drop(st);
        if spin_until_posted(sh, seen) {
            continue;
        }
        let mut st = sh.lock();
        st.asleep += 1;
        while sh.posted.load(Ordering::Acquire) == seen {
            st = sh.wake.wait(st).unwrap_or_else(|e| e.into_inner());
        }
        st.asleep -= 1;
    }
}

/// Spins (no system calls) until a job is posted after `seen` or `sh.spin` passes; true if
/// one was.
fn spin_until_posted(sh: &Shared, seen: u64) -> bool {
    if sh.spin.is_zero() {
        return false;
    }
    let start = Instant::now();
    loop {
        for _ in 0..64 {
            if sh.posted.load(Ordering::Acquire) != seen {
                return true;
            }
            std::hint::spin_loop();
        }
        if start.elapsed() >= sh.spin {
            return false;
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// How often each of `n` pieces was done, split on up to `helpers` + 1 threads.
    fn split(pool: &Pool, n: usize, helpers: usize) -> Vec<usize> {
        let next = AtomicUsize::new(0);
        let done: Vec<AtomicUsize> = (0..n).map(|_| AtomicUsize::new(0)).collect();
        pool.run(helpers, &|| {
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    break;
                }
                done[i].fetch_add(1, Ordering::Relaxed);
            }
        });
        done.into_iter().map(AtomicUsize::into_inner).collect()
    }

    fn on_worker() -> bool {
        std::thread::current()
            .name()
            .is_some_and(|n| n.starts_with("ai-infer-"))
    }

    #[test]
    fn every_piece_once() {
        for spin in [Duration::ZERO, Duration::from_micros(20)] {
            let pool = Pool::new(4, spin);
            assert_eq!(pool.workers(), 4);
            for helpers in [0, 1, 3, 4, 9] {
                for n in [0, 1, 7, 1000] {
                    assert!(split(&pool, n, helpers).iter().all(|&c| c == 1));
                }
            }
        }
    }

    #[test]
    fn concurrent_and_nested_jobs() {
        let pool = Pool::new(3, Duration::ZERO);
        std::thread::scope(|s| {
            for _ in 0..4 {
                s.spawn(|| {
                    for _ in 0..50 {
                        let next = AtomicUsize::new(0);
                        let sum = AtomicUsize::new(0);
                        pool.run(2, &|| {
                            while next.fetch_add(1, Ordering::Relaxed) < 8 {
                                // A job inside a job (helpers post them too).
                                let c = split(&pool, 16, 2);
                                sum.fetch_add(c.iter().sum(), Ordering::Relaxed);
                            }
                        });
                        assert_eq!(sum.into_inner(), 8 * 16);
                    }
                });
            }
        });
    }

    #[test]
    fn a_helper_panic_reaches_the_caller_after_every_helper_left() {
        let pool = Pool::new(4, Duration::ZERO);
        let running = AtomicUsize::new(0);
        let r = catch_unwind(AssertUnwindSafe(|| {
            pool.run(4, &|| {
                running.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(20));
                running.fetch_sub(1, Ordering::SeqCst);
                if on_worker() {
                    panic!("helper");
                }
            });
        }));
        assert_eq!(running.load(Ordering::SeqCst), 0);
        // Whether a helper joined in time depends on scheduling; if one did, its panic
        // came through.
        if let Err(p) = r {
            assert_eq!(p.downcast_ref::<&str>(), Some(&"helper"));
        }
        // The pool still works.
        assert!(split(&pool, 100, 4).iter().all(|&c| c == 1));
    }

    #[test]
    fn a_caller_panic_waits_for_the_helpers() {
        let pool = Pool::new(2, Duration::ZERO);
        let running = AtomicUsize::new(0);
        let r = catch_unwind(AssertUnwindSafe(|| {
            pool.run(2, &|| {
                running.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(20));
                running.fetch_sub(1, Ordering::SeqCst);
                if !on_worker() {
                    panic!("caller");
                }
            });
        }));
        assert!(r.is_err());
        assert_eq!(running.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn dropping_a_pool_ends_its_workers() {
        for spin in [Duration::ZERO, Duration::from_micros(50)] {
            let pool = Pool::new(3, spin);
            pool.run(3, &|| std::thread::sleep(Duration::from_millis(1)));
            let shared = Arc::downgrade(&pool.shared);
            drop(pool);
            let t = std::time::Instant::now();
            while shared.strong_count() > 0 {
                assert!(
                    t.elapsed() < Duration::from_secs(5),
                    "workers still running"
                );
                std::thread::yield_now();
            }
        }
    }
}
