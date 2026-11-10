//! HEIC on the hardware HEVC decoder: [`VaapiCore`], a [`HevcCore`] for `specular-heic`, so the
//! grid assembly, alpha, colour and orientation stay shared with the software cores.
//!
//! **What goes to the hardware.** Each coded picture (grid tile or single image) is parsed in
//! Rust (`crate::hevc`): intra (IRAP) pictures of 8- or 10-bit 4:2:0 without range / SCC /
//! multilayer extensions, inside the decoder's size limits (VCN: from 64×64). Anything else,
//! and any picture the driver fails on, is decoded by the software core instead, per tile.
//! The device is VA-API on Linux and D3D12 Video on Windows (`crate::HevcDevice`).
//!
//! **Pipelining.** A batch (the tiles of a grid) runs on one worker thread that owns the device
//! for the batch: up to `DEPTH` pictures are submitted to a pool of surfaces before the oldest
//! is waited for and read back (`vaGetImage`, a D3D12 read-back copy), so the decoder works
//! while the CPU copies.
//!
//! **Safety nets.**
//! * *Timeout:* every picture must come back within the core's timeout (default
//!   [`DEFAULT_TIMEOUT`]; libva ≥ 2.9 also bounds the wait itself with `vaSyncSurface2`). A
//!   timeout marks the device wedged for the session ([`wedged`]), blocklists the path and
//!   decodes the rest in software.
//! * *Crash canary:* a file in the cache folder exists while the hardware decodes. If the
//!   process dies in the driver, the next start finds it, logs it and blocklists the path
//!   ([`check_canary`]).
//! * *First-decode check:* the first hardware batch of a session (and every batch the path
//!   selector marks `verify`) is also decoded in software and compared sample for sample;
//!   a difference switches the hardware off for the session and blocklists it. The software
//!   result is used either way.
//! * *Errors:* a driver error blocklists the path ([`Task::HeicDecode`]); the tile and the
//!   rest of the batch are decoded in software.
//!
//! **Choice.** With [`Policy::Selector`] (the default) the path selector decides per image
//! ([`Task::HeicDecode`], timed wall and CPU), which honours "Automatic / Prefer GPU / CPU
//! only" and the `performance.hardware_decode` override; [`crate::set_enabled`] is the off
//! switch. [`Policy::Always`] (benchmarks, tests) uses the hardware whenever it is usable, and
//! delivers its result without the software comparison.

use specular_core::paths::{
    Engine, Fallback, Request, Task, note_checked, note_fallback, selector,
};
use specular_heic::{BatchSink, CoreError, CoreInput, CoreOutput, DEFAULT_CORE, HevcCore};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::time::Duration;

/// How long one picture may take on the hardware before the device counts as wedged.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

/// The canary's file name in the cache folder.
pub const CANARY_FILE: &str = "hwdec-heic.canary";

/// 0 not checked yet, 1 matched software, 2 differed (off for the session).
static CHECK: AtomicU8 = AtomicU8::new(0);
/// A hardware decode timed out: the device is not used again this session.
static WEDGED: AtomicBool = AtomicBool::new(false);
/// The previous run died while decoding HEIC on the hardware.
static CRASHED: AtomicBool = AtomicBool::new(false);
static CANARY_CHECKED: std::sync::Once = std::sync::Once::new();
static CACHE_DIR: parking_lot::Mutex<Option<PathBuf>> = parking_lot::Mutex::new(None);

/// Sets the folder the crash canary lives in (the app's cache folder). Without it: the
/// platform cache folder (`$XDG_CACHE_HOME/specular`, else `~/.cache/specular`).
pub fn set_cache_dir(dir: PathBuf) {
    *CACHE_DIR.lock() = Some(dir);
}

/// Where the crash canary is written while the hardware decodes.
pub fn canary_path() -> Option<PathBuf> {
    let dir = CACHE_DIR.lock().clone().or_else(|| {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
        Some(base.join("specular"))
    })?;
    Some(dir.join(CANARY_FILE))
}

/// Looks for a canary left by a previous run (once per process): one means that run died in
/// the driver while decoding, so the hardware HEVC path is blocklisted (kept in the timing
/// table) and off for this session. Returns whether it is off for that reason.
pub fn check_canary() -> bool {
    CANARY_CHECKED.call_once(|| {
        let Some(path) = canary_path() else {
            return;
        };
        if path.exists() {
            CRASHED.store(true, Ordering::Relaxed);
            selector().block(
                Task::HeicDecode,
                Engine::HwDecoder,
                "the previous run crashed in the hardware HEVC decoder",
            );
            specular_log::warn!(target: specular_log::DECODE,
                "hardware HEIC decoding is off: the previous run crashed while decoding on it \
                 ({} was left behind)", path.display());
            let _ = std::fs::remove_file(&path);
        }
    });
    CRASHED.load(Ordering::Relaxed)
}

/// Whether a hardware decode timed out this session (the device is then left alone).
pub fn wedged() -> bool {
    WEDGED.load(Ordering::Relaxed)
}

/// A hardware decode (of any kind) timed out: leave the device alone for the session.
// Only the VA-API paths (Linux) can time out so far.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn mark_wedged() {
    WEDGED.store(true, Ordering::Relaxed);
}

/// Whether the hardware HEVC path may be tried at all: not crashed last time, not wedged, not
/// failed the software comparison, and a device with an HEVC Main decoder.
pub fn usable() -> bool {
    !check_canary() && !wedged() && CHECK.load(Ordering::Relaxed) != 2 && hevc_limits().is_some()
}

#[cfg(target_os = "linux")]
fn hevc_limits() -> Option<crate::hevc::Limits> {
    crate::backend()?.va.try_lock()?.hevc_limits()
}

#[cfg(windows)]
fn hevc_limits() -> Option<crate::hevc::Limits> {
    crate::backend()?.d3d.try_lock()?.hevc_limits()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn hevc_limits() -> Option<()> {
    None
}

/// The process's hardware core: one long-lived [`VaapiCore`] with [`Policy::Selector`] (the path
/// selector picks hardware or software per image), created on first use and shared by every
/// HEIC decode (the VA-API device behind it is opened once per process as well).
pub fn shared() -> &'static VaapiCore {
    static CORE: std::sync::OnceLock<VaapiCore> = std::sync::OnceLock::new();
    CORE.get_or_init(VaapiCore::new)
}

/// Records the outcome of a comparison of hardware against software output made outside a
/// core (calibration on a built-in sample): like the first-decode check, a match trusts the
/// path for the session (no second check on the first real image) and a difference switches
/// it off and blocklists it.
pub fn record_check(same: bool) {
    CHECK.store(if same { 1 } else { 2 }, Ordering::Relaxed);
    selector().verified(Task::HeicDecode, Engine::HwDecoder, same);
}

/// Who decides whether a batch goes to the hardware.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Policy {
    /// The path selector, per image (and [`crate::set_enabled`] as the off switch).
    #[default]
    Selector,
    /// Whenever the hardware is usable (benchmarks and tests).
    Always,
}

/// Injected faults, for tests.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fault {
    #[default]
    None,
    /// The first submission fails like a driver error.
    SubmitError,
    /// The worker stops answering (a wedged driver).
    Hang,
    /// The process aborts while the canary is down (a driver crash).
    Abort,
    /// The first picture read back differs from software by one sample.
    Corrupt,
}

/// The hardware HEVC core, with a software core as the reference and per-tile fallback.
#[derive(Debug)]
pub struct VaapiCore {
    software: Box<dyn HevcCore>,
    policy: Policy,
    timeout: Duration,
    fault: Fault,
    hw_tiles: AtomicUsize,
    sw_tiles: AtomicUsize,
}

impl Default for VaapiCore {
    fn default() -> Self {
        Self::new()
    }
}

impl VaapiCore {
    /// [`DEFAULT_CORE`] as the software core, [`Policy::Selector`], [`DEFAULT_TIMEOUT`].
    pub fn new() -> Self {
        Self {
            software: Box::new(DEFAULT_CORE),
            policy: Policy::Selector,
            timeout: DEFAULT_TIMEOUT,
            fault: Fault::None,
            hw_tiles: AtomicUsize::new(0),
            sw_tiles: AtomicUsize::new(0),
        }
    }

    /// The software core (reference and fallback).
    pub fn with_software(mut self, core: Box<dyn HevcCore>) -> Self {
        self.software = core;
        self
    }

    pub fn with_policy(mut self, p: Policy) -> Self {
        self.policy = p;
        self
    }

    /// How long one picture may take on the hardware.
    pub fn with_timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    #[doc(hidden)]
    pub fn with_fault(mut self, f: Fault) -> Self {
        self.fault = f;
        self
    }

    /// Pictures decoded so far on the hardware and in software (by this core).
    pub fn stats(&self) -> (usize, usize) {
        (
            self.hw_tiles.load(Ordering::Relaxed),
            self.sw_tiles.load(Ordering::Relaxed),
        )
    }

    #[cfg(any(target_os = "linux", windows))]
    fn software(&self, input: &CoreInput<'_>) -> Result<CoreOutput, CoreError> {
        self.sw_tiles.fetch_add(1, Ordering::Relaxed);
        self.software.decode(input)
    }

    fn software_batch(&self, inputs: &[CoreInput<'_>], parallel: bool, sink: &BatchSink<'_>) {
        self.sw_tiles.fetch_add(inputs.len(), Ordering::Relaxed);
        self.software.decode_batch(inputs, parallel, sink);
    }

    fn run(&self, inputs: &[CoreInput<'_>], parallel: bool, sink: &BatchSink<'_>) {
        if inputs.is_empty() {
            return;
        }
        let gate = usable() && (self.policy == Policy::Always || crate::switched_on());
        let choice = (self.policy == Policy::Selector).then(|| {
            let px = inputs
                .iter()
                .map(|i| i.crop.map_or(0, |(w, h)| u64::from(w) * u64::from(h)))
                .sum();
            selector().choose(Task::HeicDecode, &Request::new(px, 0).verifying())
        });
        let hardware = gate && choice.is_none_or(|c| c.engine == Engine::HwDecoder);
        if !gate && choice.is_some_and(|c| c.engine == Engine::HwDecoder) {
            note_fallback(Fallback::Unavailable);
        }
        if !hardware {
            let run = choice.map(|c| selector().start(&c.reference()));
            self.software_batch(inputs, parallel, sink);
            if let Some(run) = run {
                run.finish();
            }
            return;
        }
        // `Always` (benchmarks, tests) delivers the hardware result as it is.
        let verify = choice.is_some_and(|c| c.verify || CHECK.load(Ordering::Relaxed) == 0);
        let run = choice.map(|c| selector().start(&c));
        let t0 = std::time::Instant::now();
        let o = self.hardware(inputs, parallel, verify, sink);
        if verify && o.hardware > 0 {
            note_checked();
        } else if o.hardware == 0 && o.failure.is_none() {
            note_fallback(Fallback::Unsupported);
        }
        specular_log::debug!(target: specular_log::DECODE,
            "HEIC on the hardware decoder: {} pictures, {} on the hardware, {:.1} ms{}",
            inputs.len(), o.hardware, t0.elapsed().as_secs_f64() * 1e3,
            if verify { " (checked against software)" } else { "" });
        if let Some(why) = &o.failure {
            note_fallback(Fallback::Failed);
            specular_log::warn!(target: specular_log::DECODE,
                "hardware HEVC decode failed, using software: {why}");
            match run {
                Some(run) => run.fail(why),
                None => selector().block(Task::HeicDecode, Engine::HwDecoder, why),
            }
        } else if let Some(run) = run
            && o.hardware == inputs.len()
            && !verify
        {
            run.finish();
        }
        if let Some(same) = o.matched {
            CHECK.store(if same { 1 } else { 2 }, Ordering::Relaxed);
            selector().verified(Task::HeicDecode, Engine::HwDecoder, same);
            if same {
                specular_log::info!(target: specular_log::DECODE,
                    "hardware HEVC decode verified: identical to software");
            } else {
                specular_log::warn!(target: specular_log::DECODE,
                    "hardware HEVC decode differs from software; using software for this session");
            }
        }
    }
}

/// What a hardware batch did.
#[derive(Debug, Default)]
struct Outcome {
    /// Pictures decoded on the hardware.
    hardware: usize,
    /// A driver error or timeout (the path is blocklisted).
    failure: Option<String>,
    /// With verification: whether every compared picture matched software.
    matched: Option<bool>,
}

impl HevcCore for VaapiCore {
    fn name(&self) -> &'static str {
        "vaapi"
    }

    fn decode(&self, input: &CoreInput<'_>) -> Result<CoreOutput, CoreError> {
        let out = parking_lot::Mutex::new(None);
        self.run(std::slice::from_ref(input), false, &|_, r| {
            *out.lock() = Some(r);
            true
        });
        out.into_inner().unwrap_or_else(|| {
            Err(CoreError::Invalid(
                "no result from the hardware core".into(),
            ))
        })
    }

    fn decode_batch(&self, inputs: &[CoreInput<'_>], parallel: bool, sink: &BatchSink<'_>) {
        self.run(inputs, parallel, sink);
    }
}

fn cancelled(inputs: &[CoreInput<'_>]) -> bool {
    inputs
        .iter()
        .any(|i| i.cancel.is_some_and(|c| c.load(Ordering::Relaxed)))
}

#[cfg(not(any(target_os = "linux", windows)))]
impl VaapiCore {
    fn hardware(
        &self,
        inputs: &[CoreInput<'_>],
        parallel: bool,
        _verify: bool,
        sink: &BatchSink<'_>,
    ) -> Outcome {
        let _ = cancelled(inputs);
        self.software_batch(inputs, parallel, sink);
        Outcome::default()
    }
}

#[cfg(any(target_os = "linux", windows))]
mod hw {
    use super::*;
    use crate::hevc::{self, Limits};
    use crate::{HevcDevice, HevcPlanes};
    use specular_heic::VuiColour;
    use specular_heic::heif::{Chroma, HeifFrame, HeifPixelFormat, Plane};
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::Instant;

    /// Pictures in flight (surfaces in the pool).
    const DEPTH: usize = 4;

    /// One picture, owned by the worker thread.
    struct Job {
        hvcc: Arc<[u8]>,
        data: Vec<u8>,
        crop: Option<(u32, u32)>,
    }

    enum Msg {
        Done(usize, Box<CoreOutput>),
        /// Not for the hardware (unsupported, malformed, device busy): software.
        Reject(usize, String),
        /// The driver failed: this and every picture not yet done go to software.
        Fail(String),
        End,
    }

    /// Removes the canary when the batch ends (also on a panic). A crash in the driver
    /// leaves it behind for the next start.
    struct Canary(Option<PathBuf>);

    impl Canary {
        fn down() -> Canary {
            let path = canary_path();
            if let Some(p) = &path {
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(p, format!("{}\n", std::process::id()));
            }
            Canary(path)
        }
    }

    impl Drop for Canary {
        fn drop(&mut self) {
            if let Some(p) = &self.0 {
                let _ = std::fs::remove_file(p);
            }
        }
    }

    impl VaapiCore {
        pub(super) fn hardware(
            &self,
            inputs: &[CoreInput<'_>],
            parallel: bool,
            verify: bool,
            sink: &BatchSink<'_>,
        ) -> Outcome {
            let n = inputs.len();
            // The worker may outlive this call (a wedged driver), so it gets owned copies.
            let mut jobs = Vec::with_capacity(n);
            let mut last: Option<Arc<[u8]>> = None;
            for i in inputs {
                let hvcc = match &last {
                    Some(h) if **h == *i.hvcc => h.clone(),
                    _ => Arc::from(i.hvcc),
                };
                last = Some(hvcc.clone());
                jobs.push(Job {
                    hvcc,
                    data: i.data.to_vec(),
                    crop: i.crop,
                });
            }
            let stop = Arc::new(AtomicBool::new(false));
            let (tx, rx) = mpsc::channel();
            let (worker_stop, timeout, fault) = (stop.clone(), self.timeout, self.fault);
            let spawned = specular_thread::builder("hwdec-heic")
                .spawn(move || worker(jobs, &tx, &worker_stop, timeout, fault));
            if let Err(e) = spawned {
                self.software_batch(inputs, parallel, sink);
                return Outcome {
                    failure: Some(format!("cannot start the decode thread: {e}")),
                    ..Outcome::default()
                };
            }

            let mut o = Outcome::default();
            let mut done = vec![false; n];
            let halt = AtomicBool::new(false);
            let differs = AtomicBool::new(false);
            let compared = AtomicUsize::new(0);
            let mut was_cancelled = false;
            rayon::scope(|s| {
                let rx = rx; // owned by this closure: a `Receiver` is not `Sync`
                // A picture for the software core: on the pool when parallel.
                let halt_ref = &halt;
                let soft = |i: usize| {
                    let halt = halt_ref;
                    let job = move |_: &rayon::Scope<'_>| {
                        if !halt.load(Ordering::Relaxed) && !sink(i, self.software(&inputs[i])) {
                            halt.store(true, Ordering::Relaxed);
                        }
                    };
                    if parallel { s.spawn(job) } else { job(s) }
                };
                let mut deadline = Instant::now() + timeout;
                loop {
                    if cancelled(inputs) {
                        was_cancelled = true;
                        break;
                    }
                    if halt.load(Ordering::Relaxed) {
                        break;
                    }
                    let now = Instant::now();
                    let wait = deadline
                        .saturating_duration_since(now)
                        .min(Duration::from_millis(50));
                    match rx.recv_timeout(wait) {
                        Ok(Msg::Done(i, out)) => {
                            deadline = Instant::now() + timeout;
                            done[i] = true;
                            o.hardware += 1;
                            self.hw_tiles.fetch_add(1, Ordering::Relaxed);
                            if verify {
                                let (differs, compared, halt) = (&differs, &compared, &halt);
                                s.spawn(move |_| {
                                    let sw = self.software(&inputs[i]);
                                    if let Ok(sw) = &sw {
                                        compared.fetch_add(1, Ordering::Relaxed);
                                        if !same(&out, sw) {
                                            differs.store(true, Ordering::Relaxed);
                                        }
                                    }
                                    if !sink(i, sw) {
                                        halt.store(true, Ordering::Relaxed);
                                    }
                                });
                            } else if !sink(i, Ok(*out)) {
                                halt.store(true, Ordering::Relaxed);
                            }
                        }
                        Ok(Msg::Reject(i, why)) => {
                            deadline = Instant::now() + timeout;
                            specular_log::debug!(target: specular_log::DECODE,
                                "HEIC picture {i} not for the hardware: {why}");
                            done[i] = true;
                            soft(i);
                        }
                        Ok(Msg::Fail(why)) => {
                            if why.contains("timed out") {
                                // vaSyncSurface2 gave up: the engine is stuck.
                                WEDGED.store(true, Ordering::Relaxed);
                            }
                            o.failure = Some(why);
                            break;
                        }
                        Ok(Msg::End) => break,
                        Err(RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
                        Err(RecvTimeoutError::Timeout) => {
                            WEDGED.store(true, Ordering::Relaxed);
                            o.failure = Some(format!(
                                "no picture back within {} ms: the decoder is not used again \
                                 this session",
                                timeout.as_millis()
                            ));
                            break;
                        }
                        Err(RecvTimeoutError::Disconnected) => {
                            o.failure = Some("the decode thread ended early".into());
                            break;
                        }
                    }
                }
                stop.store(true, Ordering::Relaxed);
                if was_cancelled {
                    if let Some(i) = done.iter().position(|d| !d) {
                        sink(i, Err(CoreError::Cancelled));
                    }
                    return;
                }
                if halt.load(Ordering::Relaxed) {
                    return;
                }
                for (i, _) in done.iter().enumerate().filter(|(_, d)| !**d) {
                    soft(i);
                }
            });
            if verify && compared.load(Ordering::Relaxed) > 0 {
                o.matched = Some(!differs.load(Ordering::Relaxed));
            }
            o
        }
    }

    fn same(a: &CoreOutput, b: &CoreOutput) -> bool {
        a.frame == b.frame && a.vui == b.vui
    }

    /// The batch on the device: parse, submit up to [`DEPTH`] ahead, read back in order.
    fn worker(
        jobs: Vec<Job>,
        tx: &mpsc::Sender<Msg>,
        stop: &AtomicBool,
        timeout: Duration,
        fault: Fault,
    ) {
        let send = |m: Msg| {
            let _ = tx.send(m);
        };
        let reject_all = |why: &str| {
            for i in 0..jobs.len() {
                send(Msg::Reject(i, why.into()));
            }
            send(Msg::End);
        };
        let Some(b) = crate::backend() else {
            return reject_all("no hardware decoder");
        };
        #[cfg(target_os = "linux")]
        let Some(mut guard) = b.va.try_lock_for(timeout) else {
            return reject_all("the hardware decoder is busy");
        };
        #[cfg(windows)]
        let Some(mut guard) = b.d3d.try_lock_for(timeout) else {
            return reject_all("the hardware decoder is busy");
        };
        let dev: &mut dyn HevcDevice = &mut *guard;
        let Some(lim) = dev.hevc_limits() else {
            return reject_all("no HEVC decoder");
        };
        let _canary = Canary::down();
        match fault {
            Fault::Abort => std::process::abort(),
            Fault::Hang => std::thread::sleep(timeout * 10),
            _ => {}
        }
        let r = run_jobs(dev, &jobs, &lim, &send, stop, timeout, fault);
        dev.hevc_release();
        match r {
            Ok(()) => send(Msg::End),
            Err(e) => send(Msg::Fail(e)),
        }
    }

    /// A picture in flight: its job, pool slot, read-back region and colour signal.
    struct Flight {
        index: usize,
        slot: usize,
        region: (u32, u32, u32, u32),
        vui: Option<VuiColour>,
    }

    fn run_jobs(
        dev: &mut dyn HevcDevice,
        jobs: &[Job],
        lim: &Limits,
        send: &dyn Fn(Msg),
        stop: &AtomicBool,
        timeout: Duration,
        fault: Fault,
    ) -> Result<(), String> {
        let ns = u64::try_from(timeout.as_nanos()).unwrap_or(u64::MAX);
        let mut flights: VecDeque<Flight> = VecDeque::new();
        let mut pool: Option<((u32, u32, bool), usize)> = None;
        let mut next_slot = 0usize;
        let mut corrupt = fault == Fault::Corrupt;
        let mut read_front =
            |dev: &mut dyn HevcDevice, flights: &mut VecDeque<Flight>| -> Result<(), String> {
                let Some(f) = flights.pop_front() else {
                    return Ok(());
                };
                let planes = dev
                    .hevc_read(f.slot, ns, f.region)
                    .map_err(|e| e.to_string())?;
                let mut out = to_output(planes, f.vui)?;
                if std::mem::take(&mut corrupt) {
                    out.frame.planes[0].data[0] ^= 1;
                }
                send(Msg::Done(f.index, Box::new(out)));
                Ok(())
            };
        for (i, job) in jobs.iter().enumerate() {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let pic = match hevc::parse(&job.hvcc, &job.data, lim) {
                Ok(p) => p,
                Err(r) => {
                    send(Msg::Reject(i, r.to_string()));
                    continue;
                }
            };
            let (vw, vh) = pic.out_size();
            let (w, h) = job.crop.unwrap_or((vw, vh));
            if w == 0 || h == 0 || w > vw || h > vh {
                send(Msg::Reject(
                    i,
                    format!("ispe {w}×{h} does not fit {vw}×{vh}"),
                ));
                continue;
            }
            let key = (pic.coded_w, pic.coded_h, pic.bit_depth > 8);
            if pool.is_none_or(|(k, _)| k != key) {
                while !flights.is_empty() {
                    read_front(dev, &mut flights)?;
                }
                let n = DEPTH.min(jobs.len() - i);
                match dev.hevc_prepare(key.0, key.1, key.2, n) {
                    Ok(()) => pool = Some((key, n)),
                    Err(e @ crate::Error::Unsupported(_)) => {
                        pool = None;
                        send(Msg::Reject(i, e.to_string()));
                        continue;
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
            let surfaces = pool.map_or(1, |(_, n)| n);
            while flights.len() >= surfaces {
                read_front(dev, &mut flights)?;
            }
            if fault == Fault::SubmitError && i == 0 {
                return Err("vaEndPicture: injected failure".into());
            }
            let slot = next_slot % surfaces;
            next_slot += 1;
            dev.hevc_submit(&pic, slot).map_err(|e| e.to_string())?;
            flights.push_back(Flight {
                index: i,
                slot,
                region: (pic.crop[0], pic.crop[2], w, h),
                vui: pic.signal.map(|s| VuiColour {
                    full_range: s.video_full_range_flag,
                    description: s.colour_description.map(|d| {
                        (
                            d.colour_primaries,
                            d.transfer_characteristics,
                            d.matrix_coeffs,
                        )
                    }),
                }),
            });
        }
        while !flights.is_empty() {
            read_front(dev, &mut flights)?;
        }
        Ok(())
    }

    fn to_output(p: HevcPlanes, vui: Option<VuiColour>) -> Result<CoreOutput, String> {
        let format =
            HeifPixelFormat::new(Chroma::Yuv420, p.bit_depth, false).map_err(|e| e.to_string())?;
        let bps = 1 + usize::from(p.bit_depth > 8);
        let planes = p
            .planes
            .into_iter()
            .enumerate()
            .map(|(i, data)| Plane {
                stride: format.plane_dims(i, p.width, p.height).0 as usize * bps,
                data,
            })
            .collect();
        let frame = HeifFrame {
            width: p.width,
            height: p.height,
            format,
            planes,
        };
        frame.validate().map_err(|e| e.to_string())?;
        Ok(CoreOutput {
            frame,
            vui,
            concealed_ctus: 0,
        })
    }
}
