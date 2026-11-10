use parking_lot::{Condvar, Mutex};
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Instant;

use specular_log::{JOBS, debug, trace};

/// Scheduling class; higher runs first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Idle = 0,
    Prefetch = 1,
    Near = 2,
    Visible = 3,
    Immediate = 4,
}

/// A shared cancellation flag. Cancelling is a single atomic store; decoders poll it at least
/// once per MCU row, so cancellation takes effect within microseconds to a millisecond.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> CancelToken {
        CancelToken::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn flag(&self) -> &AtomicBool {
        &self.0
    }
    /// Both are clones of one token.
    pub fn same(&self, other: &CancelToken) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

type Task = Box<dyn FnOnce(&CancelToken) + Send + 'static>;

struct Entry {
    priority: Priority,
    seq: u64,
    token: CancelToken,
    task: Task,
    /// What the job is, for the log and the stall watchdog (`img=7 level 1/8`).
    label: String,
    queued_at: Instant,
}

/// A job that is running.
struct Running {
    seq: u64,
    priority: Priority,
    label: String,
    started: Instant,
}

impl PartialEq for Entry {
    fn eq(&self, o: &Self) -> bool {
        self.priority == o.priority && self.seq == o.seq
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Entry {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        // Higher priority first, then FIFO (lower seq first).
        self.priority
            .cmp(&o.priority)
            .then_with(|| o.seq.cmp(&self.seq))
    }
}

struct Queue {
    heap: BinaryHeap<Entry>,
    seq: u64,
    shutdown: bool,
    /// Running jobs at [`FOREGROUND`] priority or above.
    foreground: usize,
    running: Vec<Running>,
    /// The last queued job reported as waiting on the foreground gate (logged once).
    gated_seq: u64,
}

struct Shared {
    queue: Mutex<Queue>,
    cv: Condvar,
    busy: AtomicUsize,
    /// Every job is background work: see [`JobPool::background`].
    background: bool,
    /// The pool's name; worker `i` is the thread `{name}-{i}`.
    name: String,
    /// Workers whose loop is running.
    alive: AtomicUsize,
    /// Jobs that panicked (each caught: the worker went on).
    panicked: AtomicUsize,
    /// Workers started to replace one that died, joined with the others on drop.
    replacements: Mutex<Vec<JoinHandle<()>>>,
}

/// Jobs at this priority or above are foreground: what the user is looking at.
pub(crate) const FOREGROUND: Priority = Priority::Near;

/// A prioritised worker pool. Jobs whose token is cancelled before they start are dropped
/// without running.
///
/// Background jobs ([`Priority::Prefetch`] and below) do not start while a foreground job is
/// running. Decoders split each job across every core (rayon), so a neighbour's prefetch
/// running beside the current image's decode roughly doubles the time to the current
/// image's pixels; it waits instead and starts the moment the foreground goes quiet.
pub struct JobPool {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
}

/// Handle to a submitted job.
#[derive(Clone, Debug)]
pub struct JobHandle {
    pub token: CancelToken,
}

impl JobHandle {
    pub fn cancel(&self) {
        self.token.cancel();
    }
}

impl JobPool {
    pub fn new(threads: usize, name: &str) -> JobPool {
        JobPool::with(threads, name, false)
    }

    /// A pool for work nobody is blocked on (thumbnails): its workers run at lowered OS
    /// priority for good (nice / thread priority) and their jobs one QoS class below the
    /// viewer's, so they never take the CPU from the UI thread or the viewer's decodes.
    /// Priorities still order the jobs within the pool.
    pub fn background(threads: usize, name: &str) -> JobPool {
        JobPool::with(threads, name, true)
    }

    fn with(threads: usize, name: &str, background: bool) -> JobPool {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                heap: BinaryHeap::new(),
                seq: 0,
                shutdown: false,
                foreground: 0,
                running: Vec::new(),
                gated_seq: 0,
            }),
            cv: Condvar::new(),
            busy: AtomicUsize::new(0),
            background,
            name: name.to_owned(),
            alive: AtomicUsize::new(0),
            panicked: AtomicUsize::new(0),
            replacements: Mutex::new(Vec::new()),
        });
        let threads = (0..threads.max(1))
            .map(|i| start_worker(&shared, i).expect("spawn worker"))
            .collect();
        JobPool { shared, threads }
    }

    /// Queue `f` with a fresh token.
    pub fn submit(
        &self,
        priority: Priority,
        f: impl FnOnce(&CancelToken) + Send + 'static,
    ) -> JobHandle {
        self.submit_with(priority, CancelToken::new(), f)
    }

    /// Queue `f` with an existing token (several jobs can share one).
    pub fn submit_with(
        &self,
        priority: Priority,
        token: CancelToken,
        f: impl FnOnce(&CancelToken) + Send + 'static,
    ) -> JobHandle {
        self.submit_labeled(priority, token, "job", f)
    }

    /// Queue `f` under `label`, which names the job in the log and in [`JobPool::dump`].
    pub fn submit_labeled(
        &self,
        priority: Priority,
        token: CancelToken,
        label: impl Into<String>,
        f: impl FnOnce(&CancelToken) + Send + 'static,
    ) -> JobHandle {
        let label = label.into();
        let mut q = self.shared.queue.lock();
        q.seq += 1;
        let seq = q.seq;
        debug!(target: JOBS, "submit #{seq} {label} {priority:?} (queued {}, running {})", q.heap.len() + 1, q.running.len());
        q.heap.push(Entry {
            priority,
            seq,
            token: token.clone(),
            task: Box::new(f),
            label,
            queued_at: Instant::now(),
        });
        drop(q);
        // Any idle worker may take it (one waiting on the foreground gate could not).
        self.shared.cv.notify_all();
        JobHandle { token }
    }

    /// Queue `f` under `label` and hand its result back: `Ok` with what it returned, or
    /// [`Panicked`](crate::isolate::Panicked) if it panicked (a decoder bug on this job's
    /// file), so a caller that waits always hears back. A job dropped unrun (the pool shut
    /// down) closes the channel instead.
    pub fn submit_result<T: Send + 'static>(
        &self,
        priority: Priority,
        token: CancelToken,
        label: impl Into<String>,
        f: impl FnOnce(&CancelToken) -> T + Send + 'static,
    ) -> std::sync::mpsc::Receiver<Result<T, crate::isolate::Panicked>> {
        let label = label.into();
        let what = label.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.submit_labeled(priority, token, label, move |t| {
            let _ = tx.send(crate::isolate::run(what, || f(t)));
        });
        rx
    }

    /// Number of queued (not yet started) jobs.
    pub fn queued(&self) -> usize {
        self.shared.queue.lock().heap.len()
    }

    /// Number of jobs currently running.
    pub fn running(&self) -> usize {
        self.shared.busy.load(Ordering::Relaxed)
    }

    pub fn threads(&self) -> usize {
        self.threads.len()
    }

    /// Workers running their loop now: [`JobPool::threads`] unless one is starting or a
    /// replacement for one that died is (a job's panic never ends its worker).
    pub fn alive(&self) -> usize {
        self.shared.alive.load(Ordering::Acquire)
    }

    /// Jobs that panicked since the pool started. Each was caught and logged with its label;
    /// its worker went on with the next job.
    pub fn panicked(&self) -> usize {
        self.shared.panicked.load(Ordering::Relaxed)
    }

    /// The running and queued jobs, one line each, most urgent first: what the stall
    /// watchdog prints.
    pub fn dump(&self) -> Vec<String> {
        let q = self.shared.queue.lock();
        let mut out: Vec<String> = q
            .running
            .iter()
            .map(|r| {
                format!(
                    "running #{} {:?} {} ({:.0} ms)",
                    r.seq,
                    r.priority,
                    r.label,
                    r.started.elapsed().as_secs_f64() * 1e3
                )
            })
            .collect();
        let mut queued: Vec<&Entry> = q.heap.iter().collect();
        queued.sort_by(|a, b| b.cmp(a));
        let gated = q.foreground > 0;
        out.extend(queued.into_iter().map(|e| {
            let state = if e.token.is_cancelled() {
                " cancelled"
            } else if gated && e.priority < FOREGROUND {
                " gated"
            } else {
                ""
            };
            format!(
                "queued #{} {:?} {} ({:.0} ms){state}",
                e.seq,
                e.priority,
                e.label,
                e.queued_at.elapsed().as_secs_f64() * 1e3
            )
        }));
        out
    }

    /// Drop every queued job that has been cancelled (they would be skipped anyway; this frees
    /// their memory early). Never waits: the draw thread calls it, so when a worker or a
    /// submitter holds the queue it does nothing and returns false (the next call, or the
    /// workers themselves, get them).
    pub fn purge_cancelled(&self) -> bool {
        let Some(mut q) = self.shared.queue.try_lock() else {
            return false;
        };
        let v: Vec<Entry> = std::mem::take(&mut q.heap).into_vec();
        q.heap = v.into_iter().filter(|e| !e.token.is_cancelled()).collect();
        true
    }
}

impl Drop for JobPool {
    fn drop(&mut self) {
        {
            let mut q = self.shared.queue.lock();
            q.shutdown = true;
            for e in q.heap.drain() {
                e.token.cancel();
            }
        }
        self.shared.cv.notify_all();
        // A job may hold the last reference to the pool (it then drops on a worker): that
        // worker cannot join itself, and it leaves its loop on its own after this job.
        let me = std::thread::current().id();
        let replacements = std::mem::take(&mut *self.shared.replacements.lock());
        for t in self.threads.drain(..).chain(replacements) {
            if t.thread().id() != me {
                let _ = t.join();
            }
        }
    }
}

fn start_worker(shared: &Arc<Shared>, i: usize) -> std::io::Result<JoinHandle<()>> {
    let s = shared.clone();
    // Counted from the spawn, so `alive` is right before the thread first runs.
    shared.alive.fetch_add(1, Ordering::AcqRel);
    let t = specular_thread::builder(format!("{}-{i}", shared.name)).spawn(move || worker(s, i));
    if t.is_err() {
        shared.alive.fetch_sub(1, Ordering::AcqRel);
    }
    t
}

/// Counts a worker as alive while its loop runs, and replaces it if it ever ends by a panic
/// (a job's own panic is caught in the loop; this is for one in the pool's bookkeeping).
struct Alive {
    s: Arc<Shared>,
    i: usize,
}

impl Drop for Alive {
    fn drop(&mut self) {
        self.s.alive.fetch_sub(1, Ordering::AcqRel);
        if std::thread::panicking() && !self.s.queue.lock().shutdown {
            specular_log::error!(target: JOBS, "worker {}-{} died; starting a replacement", self.s.name, self.i);
            match start_worker(&self.s, self.i) {
                Ok(t) => self.s.replacements.lock().push(t),
                Err(e) => specular_log::error!(target: JOBS, "replacement worker not started: {e}"),
            }
        }
    }
}

fn worker(s: Arc<Shared>, i: usize) {
    let _alive = Alive { s: s.clone(), i };
    if s.background {
        crate::qos::background_worker_starting();
    }
    loop {
        let entry = {
            let mut q = s.queue.lock();
            loop {
                if q.shutdown {
                    return;
                }
                // Cancelled jobs leave at once, gate or no gate.
                while q.heap.peek().is_some_and(|e| e.token.is_cancelled()) {
                    if let Some(e) = q.heap.pop() {
                        debug!(target: JOBS, "cancel #{} {} (before it started)", e.seq, e.label);
                    }
                }
                let gated = q
                    .heap
                    .peek()
                    .is_some_and(|e| e.priority < FOREGROUND && q.foreground > 0);
                if gated
                    && let Some(e) = q.heap.peek()
                    && e.seq != q.gated_seq
                {
                    let seq = e.seq;
                    debug!(target: JOBS, "gate: #{seq} {} waits for {} foreground job(s) (queued {})", e.label, q.foreground, q.heap.len());
                    q.gated_seq = seq;
                }
                if !gated && let Some(e) = q.heap.pop() {
                    if e.priority >= FOREGROUND {
                        q.foreground += 1;
                    }
                    q.running.push(Running {
                        seq: e.seq,
                        priority: e.priority,
                        label: e.label.clone(),
                        started: Instant::now(),
                    });
                    break e;
                }
                s.cv.wait(&mut q);
            }
        };
        let fg = entry.priority >= FOREGROUND;
        // OS scheduling class for this job (macOS QoS: P- or E-cores); a no-op elsewhere.
        crate::qos::job_starting(entry.priority, s.background);
        s.busy.fetch_add(1, Ordering::Relaxed);
        let (seq, token, task, label) = (entry.seq, entry.token, entry.task, entry.label);
        let started = Instant::now();
        debug!(target: JOBS, "start #{seq} {label} {:?} after {:.1} ms", entry.priority, (started - entry.queued_at).as_secs_f64() * 1e3);
        // A panicking decoder costs its job, never the worker or the process: logged with the
        // job's label (which names the file); the job's own guards report it to whoever waits.
        let ok = crate::isolate::run(format_args!("{} job #{seq} {label}", s.name), || {
            task(&token)
        })
        .is_ok();
        if !ok {
            s.panicked.fetch_add(1, Ordering::Relaxed);
        }
        s.busy.fetch_sub(1, Ordering::Relaxed);
        let ms = started.elapsed().as_secs_f64() * 1e3;
        let how = if !ok {
            "panicked"
        } else if token.is_cancelled() {
            "cancelled"
        } else {
            "done"
        };
        let mut q = s.queue.lock();
        q.running.retain(|r| r.seq != seq);
        debug!(target: JOBS, "finish #{seq} {label} {how} in {ms:.1} ms (queued {}, running {})", q.heap.len(), q.running.len());
        trace!(target: JOBS, "finish #{seq}: foreground {}", q.foreground);
        if fg {
            q.foreground -= 1;
            if q.foreground == 0 {
                s.cv.notify_all();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// A job that never finishes: the watchdog reports the image once, with its job running
    /// and the rest of the queue.
    #[test]
    fn watchdog_reports_a_stuck_job() {
        let pool = Arc::new(JobPool::new(1, "t"));
        let (release, stuck) = mpsc::channel::<()>();
        pool.submit_labeled(
            Priority::Immediate,
            CancelToken::new(),
            "img=7 2.jpeg level 1/1",
            move |_| {
                let _ = stuck.recv();
            },
        );
        pool.submit_labeled(
            Priority::Prefetch,
            CancelToken::new(),
            "img=8 next.jpg open",
            |_| {},
        );
        let (tx, rx) = mpsc::channel::<String>();
        let tx = parking_lot::Mutex::new(tx);
        let p = pool.clone();
        let wd = specular_log::Watchdog::with_sink(
            std::time::Duration::from_millis(80),
            move || p.dump(),
            move |m| {
                let _ = tx.lock().send(m.to_owned());
            },
        );
        wd.loading(7, "2.jpeg", "header 494×360");
        let msg = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the watchdog fired");
        assert!(
            msg.starts_with("img=7 2.jpeg stalled: loading for"),
            "{msg}"
        );
        assert!(
            msg.contains("state: header 494×360; pending jobs for it: 1"),
            "{msg}"
        );
        assert!(
            msg.contains("running #") && msg.contains("img=7 2.jpeg level 1/1"),
            "{msg}"
        );
        assert!(
            msg.contains("job queue (2 jobs)") && msg.contains("Prefetch img=8 next.jpg open"),
            "{msg}"
        );
        drop(wd);
        release.send(()).unwrap();
    }

    /// A neighbour's prefetch that hangs (like the progressive-JPEG rayon deadlock fixed in
    /// specular-jpeg) and holds every worker: the current image's job never starts. The
    /// watchdog still reports the current image, and its dump names the hung prefetch.
    #[test]
    fn watchdog_reports_an_image_starved_by_a_hung_prefetch() {
        let pool = Arc::new(JobPool::new(1, "t"));
        let (release, stuck) = mpsc::channel::<()>();
        let (started_tx, started) = mpsc::channel::<()>();
        pool.submit_labeled(
            Priority::Prefetch,
            CancelToken::new(),
            "img=3 prog.jpg level 1/2",
            move |_| {
                let _ = started_tx.send(());
                let _ = stuck.recv();
            },
        );
        started
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the prefetch started");
        pool.submit_labeled(
            Priority::Immediate,
            CancelToken::new(),
            "img=7 2.jpeg level 1/1",
            |_| {},
        );
        let (tx, rx) = mpsc::channel::<String>();
        let tx = parking_lot::Mutex::new(tx);
        let p = pool.clone();
        let wd = specular_log::Watchdog::with_sink(
            std::time::Duration::from_millis(80),
            move || p.dump(),
            move |m| {
                let _ = tx.lock().send(m.to_owned());
            },
        );
        wd.loading(7, "2.jpeg", "header 494×360");
        let msg = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the watchdog fired");
        assert!(
            msg.starts_with("img=7 2.jpeg stalled: loading for"),
            "{msg}"
        );
        assert!(msg.contains("pending jobs for it: 1"), "{msg}");
        assert!(
            msg.contains("running #") && msg.contains("Prefetch img=3 prog.jpg level 1/2"),
            "{msg}"
        );
        drop(wd);
        release.send(()).unwrap();
    }

    /// A job that panics (a decoder tripping on a bad file) costs only that job: the jobs
    /// queued behind it on the same single worker run, the worker is still there, and the
    /// foreground gate is released so background work still starts.
    #[test]
    fn a_panicking_job_costs_only_itself() {
        let pool = JobPool::new(1, "t");
        let (tx, rx) = mpsc::channel();
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        pool.submit(Priority::Immediate, move |_| {
            let _ = gate_rx.recv();
        });
        pool.submit_labeled(
            Priority::Immediate,
            CancelToken::new(),
            "img=1 specular-test-panic.jpg level 1/1",
            |_| crate::isolate::trip(std::path::Path::new("specular-test-panic.jpg")),
        );
        for n in 0..3 {
            let tx = tx.clone();
            pool.submit(Priority::Visible, move |_| tx.send(n).unwrap());
        }
        let txb = tx.clone();
        pool.submit(Priority::Prefetch, move |_| txb.send(9).unwrap());
        gate_tx.send(()).unwrap();
        let got: Vec<i32> = (0..4)
            .map(|_| rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap())
            .collect();
        assert_eq!(got, vec![0, 1, 2, 9]);
        assert_eq!(pool.panicked(), 1);
        assert_eq!(pool.alive(), 1);
        settle(|| pool.running() == 0 && pool.dump().is_empty());
    }

    /// A caller waiting on a job hears of its panic as a typed error, and the next job's
    /// result comes back on the same worker.
    #[test]
    fn a_waiting_caller_gets_the_panic_as_an_error() {
        let pool = JobPool::new(1, "t");
        let bad = pool.submit_result(Priority::Immediate, CancelToken::new(), "bad", |_| {
            crate::isolate::trip(std::path::Path::new("x-specular-test-panic.png"));
            1
        });
        let good = pool.submit_result(Priority::Immediate, CancelToken::new(), "good", |_| 2);
        let wait = std::time::Duration::from_secs(10);
        let p = bad.recv_timeout(wait).unwrap().unwrap_err();
        assert!(p.message.contains("test panic for"), "{p}");
        assert_eq!(good.recv_timeout(wait).unwrap(), Ok(2));
        assert_eq!(pool.alive(), 1);
        // Caught inside the job, so the pool's own boundary saw nothing.
        assert_eq!(pool.panicked(), 0);
    }

    /// Every worker keeps running after many panics, across every worker of the pool.
    #[test]
    fn workers_survive_panics_on_every_thread() {
        let pool = JobPool::new(3, "t");
        let (tx, rx) = mpsc::channel();
        for i in 0..30 {
            let tx = tx.clone();
            pool.submit(Priority::Visible, move |_| {
                if i % 2 == 0 {
                    panic!("job {i}");
                }
                tx.send(i).unwrap();
            });
        }
        let mut got: Vec<i32> = (0..15)
            .map(|_| rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap())
            .collect();
        got.sort_unstable();
        assert_eq!(got, (0..30).filter(|i| i % 2 == 1).collect::<Vec<_>>());
        settle(|| pool.panicked() == 15);
        assert_eq!(pool.alive(), 3);
    }

    /// Waits up to 10 s for `f` (the workers' bookkeeping after a job is not synchronised
    /// with what the job sent).
    fn settle(f: impl Fn() -> bool) {
        let t = Instant::now();
        while !f() {
            assert!(
                t.elapsed() < std::time::Duration::from_secs(10),
                "timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    #[test]
    fn priorities_and_cancel() {
        let pool = JobPool::new(1, "t");
        let (tx, rx) = mpsc::channel();
        // Block the single worker so the queue builds up.
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        pool.submit(Priority::Immediate, move |_| {
            let _ = gate_rx.recv();
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
        for (p, n) in [
            (Priority::Idle, 1),
            (Priority::Visible, 2),
            (Priority::Prefetch, 3),
        ] {
            let tx = tx.clone();
            pool.submit(p, move |_| tx.send(n).unwrap());
        }
        let tx2 = tx.clone();
        let h = pool.submit(Priority::Immediate, move |_| tx2.send(9).unwrap());
        h.cancel();
        gate_tx.send(()).unwrap();
        let got: Vec<i32> = (0..3).map(|_| rx.recv().unwrap()).collect();
        assert_eq!(got, vec![2, 3, 1]);
    }

    #[test]
    fn background_waits_for_foreground() {
        let pool = JobPool::new(4, "t");
        let (tx, rx) = mpsc::channel();
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        let tx1 = tx.clone();
        pool.submit(Priority::Immediate, move |_| {
            let _ = gate_rx.recv();
            tx1.send("fg").unwrap();
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
        let tx2 = tx.clone();
        pool.submit(Priority::Prefetch, move |_| tx2.send("bg").unwrap());
        // Three idle workers, but the prefetch must not start yet.
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(50))
                .is_err()
        );
        gate_tx.send(()).unwrap();
        assert_eq!(rx.recv().unwrap(), "fg");
        assert_eq!(rx.recv().unwrap(), "bg");
    }

    /// The last reference to the pool can be dropped on one of its own workers (a job that
    /// owns the loader); that must not try to join the current thread.
    #[test]
    fn dropping_the_pool_on_its_own_worker_does_not_panic() {
        let pool = Arc::new(JobPool::new(2, "t"));
        let (tx, rx) = mpsc::channel();
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        let mine = pool.clone();
        pool.submit(Priority::Immediate, move |_| {
            let _ = gate_rx.recv();
            drop(mine);
            tx.send(()).unwrap();
        });
        drop(pool);
        gate_tx.send(()).unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("job finished after dropping the pool");
    }

    #[test]
    fn purging_never_waits_for_the_queue() {
        let pool = JobPool::new(1, "t");
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        pool.submit(Priority::Immediate, move |_| {
            let _ = gate_rx.recv();
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
        let h = pool.submit(Priority::Visible, |_| {});
        pool.submit(Priority::Visible, |_| {});
        h.cancel();
        // Someone holds the queue: the purge returns at once and leaves it alone.
        let held = pool.shared.queue.lock();
        assert!(!pool.purge_cancelled());
        drop(held);
        assert_eq!(pool.queued(), 2);
        assert!(pool.purge_cancelled());
        assert_eq!(pool.queued(), 1);
        gate_tx.send(()).unwrap();
    }

    /// The calling thread's nice value (Linux).
    #[cfg(target_os = "linux")]
    fn nice() -> i32 {
        let stat = std::fs::read_to_string("/proc/thread-self/stat").unwrap();
        // Field 19; the name (field 2) may hold spaces, so count from its closing paren.
        let rest = &stat[stat.rfind(')').unwrap() + 1..];
        rest.split_whitespace().nth(16).unwrap().parse().unwrap()
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn background_pools_run_below_normal_priority() {
        let base = nice();
        let pool = JobPool::background(1, "t");
        let (tx, rx) = mpsc::channel();
        pool.submit(Priority::Visible, move |_| tx.send(nice()).unwrap());
        assert_eq!(rx.recv().unwrap(), (base + 10).min(19));
        // Ordinary pools are left alone.
        let pool = JobPool::new(1, "t");
        let (tx, rx) = mpsc::channel();
        pool.submit(Priority::Idle, move |_| tx.send(nice()).unwrap());
        assert_eq!(rx.recv().unwrap(), base);
    }
}
