//! The per-machine CPU/GPU path selector's app side (DESIGN.md §12.3.1): settings
//! (`performance.processing`, the `fast_yuv_upload` and `hardware_decode` overrides), the
//! machine fingerprint and timing table in the cache folder, idle-time calibration, the power
//! state and the hardware report's rows. The decisions are made where the work is
//! ([`specular_core::paths`]): the decoders and the renderer's upload.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use specular_core::HardwareProfile;
use specular_core::paths::{self, GpuClass, Mode, Override, Task, Trust, selector};
use specular_gfx::{Gpu, wgpu};
use specular_render::{COLOUR_STEPS, ImageRenderer};
use specular_settings::{Locations, PathMode, PathOverride, Settings};

mod panel;

/// The timing table's file in the cache folder.
const TABLE_FILE: &str = "paths-v1.txt";
/// Quiet time (no frame drawn, nothing decoding) before calibration runs.
const IDLE_AFTER: Duration = Duration::from_secs(3);
/// How often the power state is read and the timing table saved (when it changed).
const POLL_EVERY: Duration = Duration::from_secs(30);
/// Calibration steps between frames: each is a few to a few tens of milliseconds.
const STEP_EVERY: Duration = Duration::from_millis(100);

struct State {
    table: Option<PathBuf>,
    colour_step: usize,
    /// The calibration thread: the decoders, then local inference.
    decoders: Option<JoinHandle<Background>>,
    /// The calibration thread's work is all done.
    decoders_done: bool,
    /// The decoders' part still has to run (false once it finished, or when a calibrated
    /// table only lacks the inference part).
    decoders_needed: bool,
    /// Rerunning only the background calibration on a calibrated table that lacks some of it.
    recalibrating: bool,
    /// Whether the table was checked for missing decoder calibrations this session.
    rechecked: bool,
    next_poll: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State {
    table: None,
    colour_step: 0,
    decoders: None,
    decoders_done: false,
    decoders_needed: true,
    recalibrating: false,
    rechecked: false,
    next_poll: None,
});

/// What the calibration thread finished.
#[derive(Clone, Copy, Debug, Default)]
struct Background {
    decoders: bool,
    inference: bool,
}

/// Set while the app is idle; the decoder calibration thread stops as soon as it clears.
static IDLE: AtomicBool = AtomicBool::new(false);

/// The hardware decoder probe's state: bits of `PROBE_*`.
static PROBE: AtomicU8 = AtomicU8::new(0);
const PROBE_INIT: u8 = 1;
const PROBE_RELEASED: u8 = 2;
const PROBE_STARTED: u8 = 4;

/// Applies the settings. Cheap and idempotent: called on every settings change and once the
/// renderer exists. Images already on the GPU keep the path they were uploaded with.
pub(super) fn apply(s: &Settings, _renderer: Option<&mut ImageRenderer>) {
    let p = &s.performance;
    let sel = selector();
    let mode = match p.processing {
        PathMode::Automatic => Mode::Automatic,
        PathMode::PreferGpu => Mode::PreferGpu,
        PathMode::CpuOnly => Mode::CpuOnly,
    };
    sel.set_mode(mode);
    let over = |o: PathOverride| match o {
        PathOverride::Auto => Override::Auto,
        PathOverride::On => Override::On,
        PathOverride::Off => Override::Off,
    };
    sel.set_override(Task::YcbcrToRgb, over(p.fast_yuv_upload));
    sel.set_override(Task::JpegDecode, over(p.hardware_decode));
    sel.set_override(Task::HeicDecode, over(p.hardware_decode));
    specular_hwdec::set_enabled(mode != Mode::CpuOnly && p.hardware_decode != PathOverride::Off);
    panel::mark_stale();
}

/// The selector's machine description and fingerprint for `gpu`. Shared with `--hw-report`.
pub(crate) fn describe_machine(hw: &HardwareProfile, gpu: Option<&Gpu>) {
    let class = match gpu.map(|g| g.info.device_type) {
        None => GpuClass::None,
        Some(wgpu::DeviceType::DiscreteGpu) => GpuClass::Discrete,
        Some(wgpu::DeviceType::IntegratedGpu) => GpuClass::Integrated,
        Some(wgpu::DeviceType::Cpu) => GpuClass::Software,
        Some(_) => GpuClass::Unknown,
    };
    let sel = selector();
    sel.set_machine(class, hw.decode_threads());
    let (name, driver) = gpu.map_or((String::new(), String::new()), |g| {
        (
            g.info.name.clone(),
            format!("{} {}", g.info.driver, g.info.driver_info),
        )
    });
    let fp = paths::fingerprint(
        &name,
        driver.trim(),
        &hw.cpu_model,
        env!("CARGO_PKG_VERSION"),
    );
    let cache = Locations::detect().cache_dir;
    specular_hwdec::heic::set_cache_dir(cache.clone());
    let table = cache.join(TABLE_FILE);
    let loaded = sel.load_file(&table, &fp);
    specular_log::debug!(target: specular_log::DECODE,
        "path selector: {} ({})", table.display(),
        if loaded { "loaded" } else { "new machine fingerprint" });
    STATE.lock().table = Some(table);
}

/// Once the GPU exists: the machine fingerprint and the timing table. The hardware decoders
/// are probed once [`release_probe`] was called too.
pub(super) fn init(hw: &HardwareProfile, gpu: &Gpu) {
    describe_machine(hw, Some(gpu));
    memory_readings(hw, gpu);
    PROBE.fetch_or(PROBE_INIT, Ordering::Relaxed);
    start_probe();
}

/// Passes Develop's budget the GPU's dedicated memory (where the platform says: amdgpu on
/// Linux) and the machine's RAM ([`specular_develop::gpu::pool::set_memory_readings`]).
pub(super) fn memory_readings(hw: &HardwareProfile, gpu: &Gpu) {
    let vram = gpu.video_memory();
    let ram = (hw.ram_total > 0).then_some(hw.ram_total);
    specular_develop::gpu::pool::set_memory_readings(vram, ram);
    specular_log::debug!(target: specular_log::GPU, "memory readings: video {:?} MiB, RAM {:?} MiB", vram.map(|b| b >> 20), ram.map(|b| b >> 20));
}

/// Lets the hardware decoder probe run (on a background thread: opening VA-API loads the
/// driver, tens of milliseconds) once [`init`] ran too. Called when start-up is over (the
/// first image sharp, failed or a second late): during it, the probe's library loading holds
/// the dynamic loader's lock and competes for cores with the first image's upload and present.
pub(super) fn release_probe() {
    PROBE.fetch_or(PROBE_RELEASED, Ordering::Relaxed);
    start_probe();
}

fn start_probe() {
    const READY: u8 = PROBE_INIT | PROBE_RELEASED;
    if PROBE
        .compare_exchange(
            READY,
            READY | PROBE_STARTED,
            Ordering::Relaxed,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return;
    }
    let _ = specular_thread::builder("hwdec-probe").spawn(|| {
        background_thread();
        specular_hwdec::probe();
    });
}

/// Idle work, from the event loop: power state, saving the table, and calibration while
/// nothing happens (`last_frame` long ago, nothing decoding). Returns when to be woken next.
pub(super) fn tick(
    gpu: &Gpu,
    renderer: &mut ImageRenderer,
    last_frame: Instant,
    busy: bool,
) -> Option<Instant> {
    let now = Instant::now();
    let sel = selector();
    let mut st = STATE.lock();
    st.table.as_ref()?;
    if st.next_poll.is_none_or(|t| now >= t) {
        st.next_poll = Some(now + POLL_EVERY);
        sel.set_power_saving(specular_platform::power_state().saving());
        save(&st);
    }
    let mut wake = st.next_poll;
    let idle_at = last_frame + IDLE_AFTER;
    // The calibration thread's own run counts as load too.
    let own = usize::from(st.decoders.as_ref().is_some_and(|h| !h.is_finished()));
    let idle = !busy && sel.inflight() <= own && now >= idle_at;
    IDLE.store(idle, Ordering::Relaxed);
    if sel.calibrated() && !st.recalibrating {
        let decoders = !st.rechecked && specular_decode::calibrate::missing();
        if st.rechecked || !(decoders || specular_infer::calibrate::missing()) {
            st.rechecked = true;
            return wake;
        }
        // Calibrated before a decoder or inference calibration existed: run the missing
        // background part only, once.
        st.rechecked = true;
        st.recalibrating = true;
        st.decoders_needed = decoders;
        st.decoders_done = false;
        st.colour_step = COLOUR_STEPS;
    }
    if !idle {
        let at = if busy {
            now + IDLE_AFTER
        } else {
            idle_at.max(now + STEP_EVERY)
        };
        return wake.map(|w| w.min(at)).or(Some(at));
    }
    // GPU colour paths on this thread, one short step per tick.
    if st.colour_step < COLOUR_STEPS {
        if st.colour_step == 0 && sel.trust(Task::YcbcrToRgb, 1) == Trust::Unverified {
            renderer.verify_colour(gpu);
        }
        renderer.calibrate_colour_step(gpu, st.colour_step);
        st.colour_step += 1;
    }
    // Then the decoders on a background thread (not beside the colour steps, which would
    // skew both), stopping whenever the app gets busy.
    if let Some(h) = st.decoders.take_if(|h| h.is_finished()) {
        match h.join() {
            Ok(done) => {
                st.decoders_needed &= !done.decoders;
                st.decoders_done = done.decoders && done.inference;
            }
            Err(_) => {
                // Not retried this session (it would panic again each idle period); the
                // paths keep their defaults.
                specular_log::warn!(target: specular_log::JOBS, "perf: calibration stopped; defaults kept");
                st.decoders_done = true;
            }
        }
    }
    if st.colour_step >= COLOUR_STEPS && !st.decoders_done && st.decoders.is_none() {
        let decoders = st.decoders_needed;
        // Cheap (two handle clones); the shaders are only built on the thread.
        let ctx = specular_infer::gpu::Context::new(&gpu.device, &gpu.queue);
        let threads = usize::from(sel.threads()).max(1);
        st.decoders = specular_thread::builder("perf-calibrate")
            .spawn(move || {
                // Normal priority: the paths are timed as an image the user waits for would
                // run (at background priority, other programs' load would skew the serial
                // path, which runs on this thread, against the parallel one on the pool).
                paths::set_current_objective(paths::Objective::Background);
                let stop = || !IDLE.load(Ordering::Relaxed);
                let decoders = !decoders || specular_decode::calibrate::run(&stop);
                // Local inference (DESIGN §10, `Task::Inference`): the GPU self-test, then a
                // ViT-Ti/16 at 224² timed on both engines, between the same idle checks.
                let inference = decoders
                    && (!specular_infer::calibrate::missing()
                        || specular_infer::calibrate::calibrate(&ctx, threads, &stop));
                Background {
                    decoders,
                    inference,
                }
            })
            .ok();
    }
    if st.colour_step >= COLOUR_STEPS && st.decoders_done {
        st.recalibrating = false;
        st.decoders_needed = true;
        sel.set_calibrated();
        // The machine is measured now: Local AI may suit another model size.
        super::ai::recheck();
        specular_log::info!(target: specular_log::DECODE, "path selector calibrated: {}",
            sel.report_lines(crate::hw_report::REPORT_MPX).join("; "));
        save(&st);
        return wake;
    }
    let at = now + STEP_EVERY;
    wake = Some(wake.map_or(at, |w| w.min(at)));
    wake
}

/// Whether the idle-time calibration still has work to do (the app gates wait for it: its
/// end can draw a frame).
pub(super) fn calibrating() -> bool {
    let st = STATE.lock();
    st.table.is_some() && (!selector().calibrated() || st.recalibrating)
}

/// The selector's snapshot for Settings › Performance, with `Task::Inference` at the size it
/// works on (one 224² thumbnail, where it is calibrated) instead of the photo size.
pub(crate) fn snapshot() -> paths::Snapshot {
    snapshot_of(selector())
}

fn snapshot_of(sel: &paths::PathSelector) -> paths::Snapshot {
    sel.snapshot()
}

/// Marks the current thread as background work: low OS priority, and the selector picks the
/// cheapest path rather than the fastest.
fn background_thread() {
    specular_platform::set_thread_class(specular_platform::ThreadClass::Background);
    paths::set_current_objective(paths::Objective::Background);
}

/// Saves the timing table if anything was learned since the last save.
fn save(st: &State) {
    if let Some(path) = &st.table
        && selector().take_dirty() > 0
        && let Err(e) = selector().save_file(path)
    {
        specular_log::warn!(target: specular_log::DECODE,
            "path selector: cannot save {}: {e}", path.display());
    }
}

/// Saves the timing table now (on exit).
pub(super) fn flush() {
    IDLE.store(false, Ordering::Relaxed);
    save(&STATE.lock());
}
