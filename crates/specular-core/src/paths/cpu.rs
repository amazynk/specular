//! CPU time of tracked work over all the threads that do it, for [`Run`](super::Run)s and
//! [`Busy`](super::Busy) holders ("CPU time (all threads)" in the hardware report).
//!
//! A run's work happens on the thread that started it (which does the work or waits for it)
//! and on helpers: the rayon pool, scoped threads that come and go, GPU driver threads. Only
//! the starting thread is known, so the process-wide [`Ledger`] reads, at every start and
//! finish of tracked work (an *event*), the process's CPU time and each open entry's own
//! thread CPU time:
//!
//! * **Own thread:** exact. Each entry is charged its starting thread's CPU time.
//! * **Helpers:** the rest of the process's CPU time between two events (the process total
//!   counts threads that have already exited) is shared between the entries open in that
//!   interval in proportion to their weight: the number of helper threads the path's prior
//!   expects (`cpu_per_mpx / wall_per_mpx - 1`), plus [`BASE_WEIGHT`] so that paths with
//!   no helpers of their own (GPU paths: driver threads) still take a little.
//!
//! Work that runs **alone** (one entry open from its start to its finish) is charged exactly
//! the process's CPU time over its run: every thread. **Overlapping** work is approximate:
//! the split is right when each entry's helpers are busy in proportion to its weight. A
//! single-threaded path beside a parallel decode takes [`BASE_WEIGHT`] / (weight sum) of the
//! decode's helpers; two parallel decodes split their helpers by weight rather than by who
//! used them. The total charged over an interval is still exactly what the process used, so
//! the error moves CPU time between overlapping entries and never invents or loses it.
//! Untracked work in the process (the UI thread, decodes without a selector entry) between
//! events is charged to whatever is open, as before this ledger; nothing is charged while
//! nothing is open. A starting thread that steals unrelated rayon jobs while it waits
//! counts them as its own.
//!
//! Linux only (thread CPU clocks by thread id); elsewhere [`open`] returns `None` and runs
//! record wall time only.

use parking_lot::Mutex;

/// Helper weight every entry has on top of its prior's (GPU driver threads and the like).
pub(super) const BASE_WEIGHT: f32 = 0.1;

/// One open entry.
#[derive(Clone, Debug)]
struct Slot {
    id: u64,
    /// The starting thread.
    tid: i32,
    weight: f32,
    /// The thread's CPU microseconds when the entry opened.
    own0: f64,
    /// The thread's CPU microseconds at the last event.
    own_last: f64,
    /// Helper CPU microseconds charged so far.
    helpers: f64,
    /// A thread clock could not be read: the total is not reported.
    ok: bool,
}

/// The process-wide ledger of open entries.
#[derive(Debug)]
pub(super) struct Ledger {
    /// Process CPU microseconds at the last event.
    last: f64,
    slots: Vec<Slot>,
    next: u64,
}

static LEDGER: Mutex<Ledger> = Mutex::new(Ledger::new());

impl Ledger {
    pub(super) const fn new() -> Ledger {
        Ledger {
            last: 0.0,
            slots: Vec::new(),
            next: 1,
        }
    }

    /// Charges the interval since the last event: `process` is the process's CPU time now,
    /// `own` reads a thread's CPU time.
    pub(super) fn settle(&mut self, process: f64, own: &dyn Fn(i32) -> Option<f64>) {
        let delta = (process - self.last).max(0.0);
        self.last = process;
        if self.slots.is_empty() {
            return;
        }
        // Each thread's own time counts once, even with nested entries on it.
        let mut own_sum = 0.0;
        for i in 0..self.slots.len() {
            let tid = self.slots[i].tid;
            let s = &mut self.slots[i];
            let d = match own(tid) {
                Some(now) => {
                    let d = (now - s.own_last).max(0.0);
                    s.own_last = now;
                    d
                }
                None => {
                    s.ok = false;
                    0.0
                }
            };
            if !self.slots[..i].iter().any(|o| o.tid == tid) {
                own_sum += d;
            }
        }
        let helpers = (delta - own_sum).max(0.0);
        let total: f64 = self.slots.iter().map(|s| f64::from(s.weight)).sum();
        if total > 0.0 {
            for s in &mut self.slots {
                s.helpers += helpers * f64::from(s.weight) / total;
            }
        }
    }

    /// Opens an entry for thread `tid` (whose CPU time is `own_now`) after settling.
    pub(super) fn open(&mut self, tid: i32, own_now: Option<f64>, weight: f32) -> u64 {
        let id = self.next;
        self.next += 1;
        self.slots.push(Slot {
            id,
            tid,
            weight: weight.max(0.0) + BASE_WEIGHT,
            own0: own_now.unwrap_or(0.0),
            own_last: own_now.unwrap_or(0.0),
            helpers: 0.0,
            ok: own_now.is_some(),
        });
        id
    }

    /// Closes entry `id` after settling: its CPU microseconds over all threads, if known.
    pub(super) fn close(&mut self, id: u64) -> Option<f64> {
        let i = self.slots.iter().position(|s| s.id == id)?;
        let s = self.slots.swap_remove(i);
        s.ok.then(|| (s.own_last - s.own0).max(0.0) + s.helpers)
    }
}

/// The weight of a path whose prior expects `cpu_per_mpx` CPU for `wall_per_mpx` wall time.
pub(super) fn weight(cpu_per_mpx: f32, wall_per_mpx: f32) -> f32 {
    if wall_per_mpx > 0.0 && cpu_per_mpx.is_finite() {
        (cpu_per_mpx / wall_per_mpx - 1.0).clamp(0.0, 256.0)
    } else {
        0.0
    }
}

/// Opens an entry for the calling thread; `None` where CPU clocks can't be read.
pub(super) fn open(weight: f32) -> Option<u64> {
    let process = process_us()?;
    let tid = current_tid();
    let mut l = LEDGER.lock();
    l.settle(process, &thread_us);
    Some(l.open(tid, thread_us(tid), weight))
}

/// Closes entry `id`: its CPU microseconds over all threads (see the module docs).
pub(super) fn close(id: u64) -> Option<f64> {
    let process = process_us();
    let mut l = LEDGER.lock();
    if let Some(p) = process {
        l.settle(p, &thread_us);
    }
    let cpu = l.close(id);
    cpu.filter(|_| process.is_some())
}

/// CPU time of the whole process so far (live and exited threads), microseconds.
pub(super) fn process_us() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        use nix::sys::resource::{UsageWho, getrusage};
        let u = getrusage(UsageWho::RUSAGE_SELF).ok()?;
        let us = |t: nix::sys::time::TimeVal| t.tv_sec() as f64 * 1e6 + t.tv_usec() as f64;
        Some(us(u.user_time()) + us(u.system_time()))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn current_tid() -> i32 {
    #[cfg(target_os = "linux")]
    {
        nix::unistd::gettid().as_raw()
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

/// CPU time of thread `tid` of this process, microseconds; `None` once it has exited.
pub(super) fn thread_us(tid: i32) -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        use nix::time::{ClockId, clock_gettime};
        // The kernel's per-thread CPU clock id (what pthread_getcpuclockid returns):
        // !tid << 3 | CPUCLOCK_PERTHREAD_MASK (4) | CPUCLOCK_SCHED (2).
        let id = ClockId::from_raw((!tid).wrapping_shl(3) | 6);
        let t = clock_gettime(id).ok()?;
        Some(t.tv_sec() as f64 * 1e6 + t.tv_nsec() as f64 / 1e3)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = tid;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own(table: &[(i32, f64)]) -> impl Fn(i32) -> Option<f64> + '_ {
        move |tid| table.iter().find(|t| t.0 == tid).map(|t| t.1)
    }

    /// Alone, an entry is charged everything the process used over its run.
    #[test]
    fn alone_takes_the_whole_process() {
        let mut l = Ledger::new();
        l.settle(1_000.0, &own(&[]));
        let id = l.open(7, Some(50.0), 10.0);
        // Its thread used 20 µs, helpers 380 µs.
        l.settle(1_400.0, &own(&[(7, 70.0)]));
        assert_eq!(l.close(id), Some(400.0));
        // Nothing open: nothing charged.
        l.settle(2_000.0, &own(&[]));
        assert!(l.slots.is_empty());
    }

    /// Overlapping: own threads exactly, helpers by weight, nothing lost or invented.
    #[test]
    fn overlap_splits_helpers_by_weight() {
        let mut l = Ledger::new();
        l.settle(0.0, &own(&[]));
        // A parallel decode (11 helpers expected) on thread 1.
        let a = l.open(1, Some(0.0), 11.0);
        l.settle(1_000.0, &own(&[(1, 100.0)]));
        // A single-threaded path joins on thread 2.
        let b = l.open(2, Some(0.0), 0.0);
        // Interval 2: 2 000 µs; own 100 (a) + 300 (b); helpers 1 600.
        l.settle(3_000.0, &own(&[(1, 200.0), (2, 300.0)]));
        let cb = l.close(b).unwrap();
        // b: its own 300 plus 0.1 / 11.2 of 1 600.
        let share = 1_600.0 * f64::from(BASE_WEIGHT) / f64::from(11.0 + 2.0 * BASE_WEIGHT);
        assert!((cb - (300.0 + share)).abs() < 1e-6, "{cb}");
        l.settle(3_500.0, &own(&[(1, 250.0)]));
        let ca = l.close(a).unwrap();
        // Everything the process used while either was open went to one of them.
        assert!((ca + cb - 3_500.0).abs() < 1e-6, "{ca} + {cb}");
    }

    /// Nested entries on one thread count its time once in the split.
    #[test]
    fn nested_entries_share_a_thread() {
        let mut l = Ledger::new();
        l.settle(0.0, &own(&[]));
        let outer = l.open(3, Some(0.0), 0.0);
        let inner = l.open(3, Some(0.0), 0.0);
        l.settle(100.0, &own(&[(3, 100.0)]));
        assert_eq!(l.close(inner), Some(100.0));
        assert_eq!(l.close(outer), Some(100.0));
    }

    /// A thread clock that can't be read makes the total unknown, not wrong.
    #[test]
    fn unreadable_thread_is_unknown() {
        let mut l = Ledger::new();
        l.settle(0.0, &own(&[]));
        let id = l.open(9, Some(0.0), 1.0);
        l.settle(500.0, &own(&[]));
        assert_eq!(l.close(id), None);
        assert_eq!(l.close(id), None);
    }

    #[test]
    fn weights_from_priors() {
        assert_eq!(weight(4_000.0, 4_000.0), 0.0);
        assert_eq!(weight(600.0, 1_200.0), 0.0);
        assert!((weight(13_500.0, 13_000.0 / 12.0) - 11.46).abs() < 0.01);
        assert_eq!(weight(1.0, 0.0), 0.0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn clocks_read_on_linux() {
        let tid = current_tid();
        let a = thread_us(tid).expect("own thread clock");
        let mut x = 0u64;
        while thread_us(tid).unwrap() - a < 2_000.0 {
            x = std::hint::black_box(x.wrapping_mul(31).wrapping_add(7));
        }
        assert!(process_us().unwrap() >= thread_us(tid).unwrap());
        // A thread that does not exist has no clock: above the largest pid_max (2^22), since
        // an exited thread's id may already be reused by another test's thread.
        assert_eq!(thread_us((1 << 22) + 1), None);
    }
}
