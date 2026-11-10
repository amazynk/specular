//! Operating-system hooks that affect speed (DESIGN.md §12.2, §12.5, §12.7).
//!
//! One portable API; the real work is per platform. macOS has the most (`src/macos/`); Linux and
//! Windows have power, battery and thermal state (`src/power/`), Linux batched read-ahead
//! (`src/readahead/`), Windows EcoQoS; Linux and Windows memory pressure (`src/pressure/`).
//! Elsewhere every hook is a cheap no-op or returns
//! "unknown", so callers never need their own `cfg`.
//!
//! * [`set_thread_class`]: tell the scheduler how urgent the current thread's work is (macOS QoS
//!   classes; on Apple Silicon this is what places work on performance or efficiency cores).
//! * [`lower_thread_priority`]: a worker that only ever does background work (thumbnails) gives
//!   way to the UI thread for good (Linux nice, Windows thread priority).
//! * [`core_clusters`], [`cpu_brand`], [`physical_cores`]: CPU facts for the hardware report.
//! * [`advise_will_read`], [`set_uncached`], [`set_read_ahead`]: file I/O advice.
//! * [`readahead::warm`]: reads the neighbour files about to be opened into the page cache in one
//!   batch (io_uring on Linux).
//! * [`watch_memory_pressure`], [`memory_pressure`], [`power_state`]: system signals, folded into
//!   [`prefetch_depth`] and [`cache_budget`] so caches shrink and prefetch backs off.
//! * [`Activity`]: keeps App Nap away while the user is watching something move.
//! * [`awake::Awake`]: keeps the screen awake while a movie plays.
//! * [`main_screen`]: refresh rate and EDR headroom of the main display.
//! * [`mark_ui_thread`], [`assert_not_ui_thread`]: debug builds fail fast when blocking work
//!   (the Library's catalogue) runs on the UI thread.

#![cfg_attr(unix, feature(peer_credentials_unix_socket))]
#![cfg_attr(windows, feature(windows_unix_domain_sockets))]

use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[cfg(windows)]
mod app_id;
pub mod awake;
#[cfg(windows)]
mod console;
pub mod display_profile;
pub mod drag;
pub mod file_chooser;
pub mod gesture;
pub mod instance;
pub mod integration;
pub mod ipc;
pub mod launch;
#[cfg(target_os = "macos")]
mod macos;
pub mod media;
pub mod open_with;
pub mod path;
pub mod pen;
#[cfg(any(target_os = "linux", windows))]
mod power;
#[cfg(any(target_os = "linux", windows))]
mod pressure;
pub mod print;
pub mod readahead;
pub mod share;
pub mod thumbnail;
pub mod title_bar;
pub mod ui_thread;
pub mod url_scheme;
pub mod volumes;

pub use ui_thread::{UiThreadScope, assert_not_ui_thread, is_ui_thread, mark_ui_thread};

/// How urgent a thread's work is. On macOS these are the QoS classes; higher classes run on
/// performance cores and are throttled less.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThreadClass {
    /// The UI and render thread, and GPU start-up (`QOS_CLASS_USER_INTERACTIVE`).
    Interactive,
    /// Decoding what the user is looking at or about to look at (`QOS_CLASS_USER_INITIATED`).
    UserInitiated,
    /// Prefetching neighbours (`QOS_CLASS_UTILITY`).
    Utility,
    /// Thumbnails, indexing, anything nobody waits for (`QOS_CLASS_BACKGROUND`; also throttles
    /// its disk I/O).
    Background,
}

/// Sets the scheduling class of the calling thread. Cheap when the class is unchanged (a
/// thread-local check), one system call otherwise.
///
/// * macOS: the QoS class.
/// * Windows: EcoQoS for `Utility` and `Background` (efficiency cores, lower clocks), high QoS
///   for the rest.
/// * Linux: nothing here; `specular-core`'s placement pins pool workers to core types.
pub fn set_thread_class(class: ThreadClass) {
    #[cfg(target_os = "macos")]
    macos::qos::set_current(class);
    #[cfg(windows)]
    power::windows::set_thread_class(class);
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = class;
}

/// How far [`lower_thread_priority`] lowers a thread on Linux, in nice steps.
#[cfg(target_os = "linux")]
const NICE_STEPS: i32 = 10;

/// Lowers the calling thread's CPU priority for the rest of its life, so it never competes
/// with the UI thread. For pools whose every job is background work (thumbnails); called once
/// as each worker starts.
///
/// * Linux: nice +10 on this thread (Linux keeps a nice value per thread). Not per job:
///   an unprivileged process cannot raise its nice value again.
/// * Windows: `THREAD_PRIORITY_BELOW_NORMAL` (and EcoQoS through each job's
///   [`set_thread_class`]).
/// * macOS: nothing here; such pools cap the per-job QoS class instead ([`set_thread_class`]).
///
/// Failure only means the thread keeps its priority.
pub fn lower_thread_priority() {
    #[cfg(target_os = "linux")]
    {
        // `None` is the calling thread on Linux (setpriority's "process" is the task).
        let now = rustix::process::getpriority_process(None).unwrap_or(0);
        let _ = rustix::process::setpriority_process(None, (now + NICE_STEPS).min(19));
    }
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
        };
        // SAFETY: GetCurrentThread takes no arguments and returns a pseudo-handle for the
        // calling thread, valid for as long as the thread runs; it cannot fail.
        #[allow(unsafe_code)]
        let me = unsafe { GetCurrentThread() };
        // SAFETY: `me` is the calling thread's pseudo-handle (always valid in this thread, no
        // close needed) and THREAD_PRIORITY_BELOW_NORMAL is a documented priority value.
        #[allow(unsafe_code)]
        let _ = unsafe { SetThreadPriority(me, THREAD_PRIORITY_BELOW_NORMAL) };
    }
}

/// One class of CPU cores (Apple Silicon "performance level").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreCluster {
    /// What the OS calls it ("Performance", "Efficiency").
    pub name: String,
    pub physical: usize,
    pub logical: usize,
}

/// Core classes, fastest first. Empty when all cores are alike or the OS does not say
/// (Intel Macs, and other systems for now).
pub fn core_clusters() -> Vec<CoreCluster> {
    #[cfg(target_os = "macos")]
    return macos::sysctl::core_clusters();
    #[cfg(not(target_os = "macos"))]
    Vec::new()
}

/// The CPU's marketing name ("Apple M3 Pro"), if the platform layer knows it.
pub fn cpu_brand() -> Option<String> {
    #[cfg(target_os = "macos")]
    return macos::sysctl::string("machdep.cpu.brand_string");
    // Windows: the processor's brand string, through sysinfo (no CPU usage sampling).
    #[cfg(windows)]
    {
        let mut s = sysinfo::System::new();
        s.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing());
        s.cpus()
            .first()
            .map(|c| c.brand().trim().to_owned())
            .filter(|b| !b.is_empty())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    None
}

/// Windows: attaches to the console of the terminal Specular was started from, unless its
/// output is redirected (the viewer is a GUI-subsystem program, so it has no console of its
/// own). Returns whether it attached. Elsewhere a program keeps its terminal: false.
pub fn attach_parent_console() -> bool {
    #[cfg(windows)]
    return console::attach_parent_console();
    #[cfg(not(windows))]
    false
}

/// Windows: gives the process the AppUserModelID `id`, the one the installer's Start-menu
/// shortcut carries, so the windows, a pinned shortcut and the jump list (with its recent files)
/// are one taskbar button. Call before the first window. Nothing elsewhere.
pub fn set_app_user_model_id(id: &str) {
    #[cfg(windows)]
    app_id::set(id);
    #[cfg(not(windows))]
    let _ = id;
}

/// Windows: detaches from the console [`attach_parent_console`] attached to, so the window no
/// longer depends on that terminal. Nothing elsewhere.
pub fn detach_console() {
    #[cfg(windows)]
    console::detach_console();
}

/// Windows: shows a startup error in a message box when there is no terminal to print it to
/// (started from Explorer). `title` and `text` are NUL-terminated UTF-16 made at compile time.
/// Nothing elsewhere, where the error is printed to the terminal.
pub fn error_box_without_terminal(title: &[u16], text: &[u16]) {
    #[cfg(windows)]
    console::error_box_without_terminal(title, text);
    #[cfg(not(windows))]
    let _ = (title, text);
}

/// Physical cores, if the platform layer knows them.
pub fn physical_cores() -> Option<usize> {
    #[cfg(target_os = "macos")]
    return macos::sysctl::int("hw.physicalcpu").and_then(|n| usize::try_from(n).ok());
    #[cfg(windows)]
    return sysinfo::System::physical_core_count().filter(|&n| n > 0);
    #[cfg(not(any(target_os = "macos", windows)))]
    None
}

/// Windows: the build of Windows running (19041 is Windows 10 version 2004), if it can be read.
/// None elsewhere.
pub fn windows_build() -> Option<u32> {
    #[cfg(windows)]
    return sysinfo::System::kernel_version()
        .and_then(|v| v.split('.').next().and_then(|b| b.trim().parse().ok()));
    #[cfg(not(windows))]
    None
}

/// Whether the GPU shares memory with the CPU (Apple Silicon), as far as the platform layer
/// knows. `None` when unknown.
pub fn unified_memory() -> Option<bool> {
    #[cfg(target_os = "macos")]
    return Some(
        cfg!(target_arch = "aarch64") || macos::sysctl::int("hw.optional.arm64") == Some(1),
    );
    #[cfg(not(target_os = "macos"))]
    None
}

/// Asks the OS to start reading `len` bytes at `offset` into the page cache in the background
/// (macOS `F_RDADVISE`). Called when an image file is opened, so the decoder's page faults find
/// the data already on its way. Disabled with `SPECULAR_RDADVISE=0` for A/B measurement.
pub fn advise_will_read(file: &File, offset: u64, len: u64) {
    #[cfg(target_os = "macos")]
    macos::file::rdadvise(file, offset, len);
    #[cfg(not(target_os = "macos"))]
    let _ = (file, offset, len);
}

/// Bypasses the page cache for reads through this descriptor (macOS `F_NOCACHE`). For one-shot
/// streaming reads of huge files (thumbnailing a folder of 200 MB RAWs) that would otherwise
/// evict everything useful. Does not affect memory maps. Returns whether it took effect.
pub fn set_uncached(file: &File, on: bool) -> bool {
    #[cfg(target_os = "macos")]
    return macos::file::fcntl_flag(file, libc::F_NOCACHE, on);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (file, on);
        false
    }
}

/// Turns the kernel's sequential read-ahead for this descriptor on or off (macOS `F_RDAHEAD`).
/// Off helps random access into huge files (tiled TIFF, archives). Returns whether it took effect.
pub fn set_read_ahead(file: &File, on: bool) -> bool {
    #[cfg(target_os = "macos")]
    return macos::file::fcntl_flag(file, libc::F_RDAHEAD, on);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (file, on);
        false
    }
}

/// System memory pressure, as the OS reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum MemoryPressure {
    #[default]
    Normal = 0,
    Warn = 1,
    Critical = 2,
}

static PRESSURE: AtomicU8 = AtomicU8::new(0);
static PRESSURE_CHANGED: AtomicBool = AtomicBool::new(false);

/// The latest memory-pressure level ([`MemoryPressure::Normal`] until a notification says
/// otherwise, and always on systems without a watcher).
pub fn memory_pressure() -> MemoryPressure {
    match PRESSURE.load(Ordering::Relaxed) {
        0 => MemoryPressure::Normal,
        1 => MemoryPressure::Warn,
        _ => MemoryPressure::Critical,
    }
}

/// Tests that set the process-wide pressure level take this first.
#[cfg(test)]
pub(crate) static PRESSURE_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn set_memory_pressure(p: MemoryPressure) -> bool {
    let changed = PRESSURE.swap(p as u8, Ordering::Relaxed) != p as u8;
    if changed {
        PRESSURE_CHANGED.store(true, Ordering::Release);
    }
    changed
}

/// Whether the memory-pressure level changed since the last call: the UI thread polls this
/// after a wake-up and, if so, re-plans prefetch and trims caches.
pub fn memory_pressure_changed() -> bool {
    PRESSURE_CHANGED.swap(false, Ordering::AcqRel)
}

type Wake = std::sync::Arc<dyn Fn() + Send + Sync>;

/// The callback of [`watch_memory_pressure`] (the first one registered).
static PRESSURE_WAKE: std::sync::OnceLock<Wake> = std::sync::OnceLock::new();

/// A pressure source saw `level`: records it and, when it changed, wakes the UI thread.
fn report_memory_pressure(level: MemoryPressure) {
    if set_memory_pressure(level) {
        specular_log::info!(target: specular_log::CACHE, "memory pressure: {level:?}");
        if let Some(wake) = PRESSURE_WAKE.get() {
            wake();
        }
    }
}

/// Starts listening for memory-pressure changes. `wake` runs on an OS thread whenever the
/// level changes; it should wake the UI thread, which then reads [`memory_pressure`] (through
/// [`prefetch_depth`] and [`cache_budget`]) and trims its caches. Calling it again replaces
/// nothing: the first watcher stays.
///
/// * macOS: a libdispatch memory-pressure source.
/// * Linux: PSI triggers on `/proc/pressure/memory`, and `memory.events` of the nearest cgroup
///   v2 ancestor with a memory limit (`pressure/linux.rs`); periodic PSI reads where triggers
///   are not permitted; nothing without `/proc/pressure`.
/// * Windows: the low and high memory resource notifications (`pressure/windows.rs`).
pub fn watch_memory_pressure(wake: impl Fn() + Send + Sync + 'static) {
    let mut fresh = false;
    let wake = PRESSURE_WAKE.get_or_init(|| {
        fresh = true;
        std::sync::Arc::new(wake)
    });
    if !fresh {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let wake = wake.clone();
        macos::pressure::watch(Box::new(move || wake()));
    }
    #[cfg(not(target_os = "macos"))]
    let _ = wake;
    #[cfg(any(target_os = "linux", windows))]
    pressure::watch();
}

/// Reports `level` as a pressure source does (records it; when it changed, runs
/// [`watch_memory_pressure`]'s callback): for tests and simulations. The OS source reports its
/// own level again at its next change.
#[doc(hidden)]
pub fn simulate_memory_pressure(level: MemoryPressure) {
    report_memory_pressure(level);
}

/// The machine's thermal state (macOS `NSProcessInfo.thermalState`; Linux thermal-zone trip
/// points and power-profiles-daemon; always `Nominal` on Windows).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Thermal {
    #[default]
    Nominal,
    Fair,
    Serious,
    Critical,
}

/// Thermal state, low-power mode and power source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PowerState {
    pub thermal: Thermal,
    /// macOS Low Power Mode, the Linux `power-saver` profile, Windows battery saver.
    pub low_power: bool,
    /// Running on battery (no AC adapter supplying power). Linux and Windows; `false` on macOS
    /// for now.
    pub on_battery: bool,
    /// Combined charge of the system batteries, if there are any and the OS says.
    pub battery_percent: Option<u8>,
}

/// At or below this charge on battery, behave as in low-power mode.
pub const LOW_BATTERY_PERCENT: u8 = 20;

impl PowerState {
    /// On battery, in a low-power mode or under thermal pressure: prefer lower energy over
    /// speed (DESIGN.md §12.2, §12.3.1).
    pub fn saving(&self) -> bool {
        self.low_power || self.on_battery || self.thermal >= Thermal::Serious
    }

    /// Low-power mode, or on battery with little charge left.
    pub fn low_energy(&self) -> bool {
        self.low_power
            || (self.on_battery
                && self
                    .battery_percent
                    .is_some_and(|p| p <= LOW_BATTERY_PERCENT))
    }
}

/// The current power state; a couple of microseconds (two Objective-C messages on macOS, one
/// call on Windows, a cached sample on Linux that a helper thread refreshes when it is more than
/// a few seconds old).
pub fn power_state() -> PowerState {
    #[cfg(target_os = "macos")]
    return macos::power::state();
    #[cfg(target_os = "linux")]
    return power::linux::state();
    #[cfg(windows)]
    return power::windows::state();
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    PowerState::default()
}

/// The prefetch depth (images on each side) to use right now, given the user's setting:
/// unchanged normally, smaller under memory pressure, on a hot machine, on battery or in a
/// low-power mode.
pub fn prefetch_depth(setting: usize) -> usize {
    throttle_depth(setting, memory_pressure(), power_state())
}

fn throttle_depth(depth: usize, pressure: MemoryPressure, power: PowerState) -> usize {
    let mut d = depth;
    match pressure {
        MemoryPressure::Normal => {}
        MemoryPressure::Warn => d = d.div_ceil(2),
        MemoryPressure::Critical => d = 0,
    }
    match power.thermal {
        Thermal::Nominal | Thermal::Fair => {}
        Thermal::Serious => d = d.min(1),
        Thermal::Critical => d = 0,
    }
    if power.low_energy() {
        d = d.min(1);
    } else if power.on_battery {
        d = d.min(2);
    }
    d
}

/// A cache budget in bytes scaled for memory pressure: halved on a warning, a quarter when
/// critical.
pub fn cache_budget(bytes: u64) -> u64 {
    scale_budget(bytes, memory_pressure())
}

fn scale_budget(bytes: u64, pressure: MemoryPressure) -> u64 {
    match pressure {
        MemoryPressure::Normal => bytes,
        MemoryPressure::Warn => bytes / 2,
        MemoryPressure::Critical => bytes / 4,
    }
}

/// While alive, the OS treats the app as busy with something the user is watching: no App Nap
/// (timer coalescing, throttled run loop) even when the window is hidden or occluded. Hold one
/// during slideshows, animation playback and benchmarks; drop it when they stop.
#[must_use = "the activity ends when this is dropped"]
pub struct Activity {
    #[cfg(target_os = "macos")]
    _inner: macos::power::ActivityToken,
}

impl std::fmt::Debug for Activity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Activity")
    }
}

/// Begins an [`Activity`]. `latency_critical` also asks for the lowest timer latency (frame
/// pacing of animations and slideshows).
pub fn begin_activity(reason: &str, latency_critical: bool) -> Activity {
    #[cfg(target_os = "macos")]
    return Activity {
        _inner: macos::power::begin(reason, latency_critical),
    };
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (reason, latency_critical);
        Activity {}
    }
}

/// Facts about the main display.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ScreenInfo {
    /// Highest refresh rate (120 on ProMotion panels).
    pub max_fps: Option<u32>,
    /// EDR headroom right now (1.0 = SDR white is the brightest the panel shows).
    pub edr_current: Option<f32>,
    /// EDR headroom the panel can reach (16 on XDR panels).
    pub edr_potential: Option<f32>,
}

/// The main display's refresh rate and EDR headroom. Main thread only (AppKit); `None`
/// elsewhere, off the main thread, or without a display.
pub fn main_screen() -> Option<ScreenInfo> {
    #[cfg(target_os = "macos")]
    return macos::screen::main_screen();
    #[cfg(not(target_os = "macos"))]
    None
}

/// A private D-Bus daemon for tests (and, with `test-support`, other crates' tests).
#[cfg(all(any(test, feature = "test-support"), unix, not(target_os = "macos")))]
#[doc(hidden)]
pub mod test_bus;

#[cfg(test)]
mod tests {
    use super::*;

    /// The hardware report's CPU line: a name, and no more physical cores than logical ones.
    #[cfg(windows)]
    #[test]
    fn cpu_brand_and_physical_cores() {
        let logical = std::thread::available_parallelism().unwrap().get();
        let t = std::time::Instant::now();
        let (brand, physical) = (cpu_brand(), physical_cores());
        eprintln!(
            "{brand:?}, {physical:?} physical of {logical} logical ({:?})",
            t.elapsed()
        );
        assert!(brand.is_some_and(|b| !b.is_empty()));
        assert!(physical.is_some_and(|p| (1..=logical).contains(&p)));
    }

    #[test]
    fn prefetch_backs_off() {
        let ok = PowerState::default();
        assert_eq!(throttle_depth(4, MemoryPressure::Normal, ok), 4);
        assert_eq!(throttle_depth(4, MemoryPressure::Warn, ok), 2);
        assert_eq!(throttle_depth(3, MemoryPressure::Warn, ok), 2);
        assert_eq!(throttle_depth(1, MemoryPressure::Warn, ok), 1);
        assert_eq!(throttle_depth(4, MemoryPressure::Critical, ok), 0);
        let hot = PowerState {
            thermal: Thermal::Serious,
            ..ok
        };
        assert_eq!(throttle_depth(4, MemoryPressure::Normal, hot), 1);
        assert!(hot.saving());
        let lpm = PowerState {
            thermal: Thermal::Fair,
            low_power: true,
            ..ok
        };
        assert_eq!(throttle_depth(4, MemoryPressure::Normal, lpm), 1);
        let melting = PowerState {
            thermal: Thermal::Critical,
            ..ok
        };
        assert_eq!(throttle_depth(4, MemoryPressure::Normal, melting), 0);
        let battery = PowerState {
            on_battery: true,
            battery_percent: Some(80),
            ..ok
        };
        assert!(battery.saving() && !battery.low_energy());
        assert_eq!(throttle_depth(4, MemoryPressure::Normal, battery), 2);
        assert_eq!(throttle_depth(1, MemoryPressure::Normal, battery), 1);
        let low = PowerState {
            battery_percent: Some(LOW_BATTERY_PERCENT),
            ..battery
        };
        assert_eq!(throttle_depth(4, MemoryPressure::Normal, low), 1);
        // A battery charging on AC is nothing to save, however empty.
        let ac = PowerState {
            battery_percent: Some(5),
            ..ok
        };
        assert!(!ac.saving() && !ac.low_energy());
        assert_eq!(throttle_depth(4, MemoryPressure::Normal, ac), 4);
    }

    #[test]
    fn budgets_shrink() {
        assert_eq!(scale_budget(800, MemoryPressure::Normal), 800);
        assert_eq!(scale_budget(800, MemoryPressure::Warn), 400);
        assert_eq!(scale_budget(800, MemoryPressure::Critical), 200);
    }

    #[test]
    fn portable_defaults() {
        let _one = PRESSURE_TEST.lock().unwrap_or_else(|e| e.into_inner());
        // On a machine without notifications the level stays Normal, and the hooks are no-ops.
        assert_eq!(memory_pressure(), MemoryPressure::Normal);
        set_thread_class(ThreadClass::Utility);
        set_thread_class(ThreadClass::UserInitiated);
        let _a = begin_activity("test", false);
        let f = File::open(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
        advise_will_read(&f, 0, 1);
        watch_memory_pressure(|| {});
        assert!(set_memory_pressure(MemoryPressure::Warn));
        assert!(memory_pressure_changed());
        assert!(!memory_pressure_changed());
        assert!(!set_memory_pressure(MemoryPressure::Warn));
        assert_eq!(cache_budget(8), 4);
        assert!(set_memory_pressure(MemoryPressure::Normal));
    }
}
