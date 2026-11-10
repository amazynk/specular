//! Named threads. Every thread Specular starts has a short, distinct name, so `top -H`,
//! `ps -L`, debuggers, profilers and the log (which prints the thread name) say which part of
//! the program a thread belongs to.
//!
//! # The scheme
//!
//! * At most [`MAX_LEN`] (15) bytes: Linux truncates longer names (`specular-host-watchdog`
//!   showed as `specular-host-w`).
//! * Lowercase ASCII kebab-case: `a-z`, `0-9` and single `-` between words.
//! * No `specular-` prefix: the process name already says that.
//! * `area-role`, with the area one of the prefixes below; pool members and indexed workers
//!   end in `-N` (`dec-jpeg-3`), and the name must still fit with a two-digit `N`.
//!
//! | Prefix    | Area                         | Examples                                         |
//! |-----------|------------------------------|--------------------------------------------------|
//! | `rayon-`  | rayon's global pool          | `rayon-N` ([`GLOBAL_POOL`])                       |
//! | `dec-`    | decoding                     | `dec-N` (the decode pool), `dec-jpeg-N`, `dec-jxl-N`, `dec-jp2-N`, `dec-png-part-N`, `dec-vp8-recon`, `dec-hevc-wf-N`, `dec-h265-lf-N`, `dec-h264-N`, `dec-av1-N`, `dec-linear-N` |
//! | `hwdec-`  | hardware decoding            | `hwdec-heic`, `hwdec-probe`                      |
//! | `enc-`    | encoders                     | `enc-conv-N`                                     |
//! | `host-`   | the sandboxed decode host    | `host-spawn`                                     |
//! | `wd-`     | watchdogs                    | `wd-load`, `wd-host`, `wd-parent`, `wd-thumb`    |
//! | `thumb-`  | thumbnails                   | `thumb-N` (pool), `thumb-feed`, `thumb-list-N`   |
//! | `gal-`    | gallery                      | `gal-list`, `gal-atlas`, `gal-cover-N`           |
//! | `lib-`    | library (catalogue)          | `lib-index`, `lib-open`, `lib-hash-N`            |
//! | `sim-`    | similar images               | `sim-scan`, `sim-focus-N`                        |
//! | `cull-`   | culling                      | `cull-bursts`, `cull-slot`                       |
//! | `ai-`     | Local AI and inference       | `ai-index`, `ai-search`, `ai-get-text`, `ai-infer-N` |
//! | `dcm-`    | DICOM (local)                | `dcm-series`, `dcm-media`, `dcm-st-read`         |
//! | `dcmweb-` | DICOMweb                     | `dcmweb-load`, `dcmweb-get-N`                    |
//! | `mpr-`    | MPR (and `vr-` for volume rendering) | `mpr-open`                               |
//! | `vx-`     | video export                 | `vx-render`, `vx-decode`, `vx-encode`            |
//! | `anim-`   | animation playback           | `anim-play`                                      |
//! | `print-`  | printing                     | `print-render`, `print-decode`                   |
//! | `conv-`   | batch conversion             | `conv-batch`, `conv-job-N` (pool)                |
//! | `anon-`   | anonymised export            | `anon-export`, `anon-progress`                   |
//! | `book-`   | comics and books             | `book-comicinfo`                                 |
//! | `meta-`   | metadata                     | `meta-geocoder`                                  |
//! | `gpu-`    | GPU and renderer start-up    | `gpu-init`, `gpu-renderer`, `gpu-compare`        |
//! | `ren-`    | CPU-side rendering helpers   | `ren-colour-N`                                   |
//! | `font-`   | fonts                        | `font-load`, `font-system`, `font-warm`          |
//! | `disp-`   | display profile and cache    | `disp-profile`, `disp-cache`                     |
//! | `perf-`   | performance tuning           | `perf-calibrate`, `perf-history`                 |
//! | `set-`    | settings and key map         | `set-watch`, `set-keymap`                        |
//! | `io-`     | file system work             | `io-readahead`, `io-scan-dir`, `io-trash`        |
//! | `clip-`   | clipboard                    | `clip-copy`, `clip-paste`                        |
//! | `int-`    | desktop integration          | `int-apply`, `int-open-with`, `int-reveal`       |
//! | `dlg-`    | file dialogs                 | `dlg-open`, `dlg-export`, `dlg-print`            |
//! | `rc-`     | remote control (IPC server)  | `rc-accept`, `rc-client`                         |
//! | `net-`    | network (remote files)       | `net-test-srv`                                   |
//! | `arc-`    | archives                     | `arc-7z`, `arc-7z-unit-N`                        |
//! | `plat-`   | platform services            | `plat-memory`, `plat-power`, `plat-child`        |
//! | `wl-`, `x11-` | display-server events    | `wl-drag`, `wl-gesture`, `x11-drag`              |
//! | `test-`   | test hooks                   | `test-then-open`                                 |
//! | `bench-`, `corpus-` | developer tools    | `bench-stdout`, `corpus-write`                   |
//!
//! # Helpers
//!
//! * [`spawn`] (instead of `std::thread::spawn`) and [`builder`] (instead of
//!   `std::thread::Builder::new`, for a stack size or a spawn error to handle);
//! * [`spawn_scoped`] (instead of `std::thread::Scope::spawn`);
//! * [`on_stack`] runs a closure on a thread with a given stack size (hostile-input tests);
//! * [`pool_name`] for `rayon::ThreadPoolBuilder::thread_name`, and with the `rayon` feature
//!   [`rayon_pool`] (instead of `rayon::ThreadPoolBuilder::new`).
//!
//! The std and rayon originals are `disallowed-methods` in the workspace `clippy.toml`; tests,
//! examples and benches may use them. Debug builds check every name against the scheme
//! ([`check`]) and log a warning for one off it; a unit test of this crate checks every name
//! literal in the workspace, and fails on one off it.

use std::thread::{Builder, JoinHandle, Scope, ScopedJoinHandle};

/// Linux's limit for a thread name (16 bytes with the terminating NUL).
pub const MAX_LEN: usize = 15;

/// Prefix of rayon's global pool (`rayon-N`), built by the app (and by
/// `specular_core::placement` on Linux) before anything else touches rayon.
pub const GLOBAL_POOL: &str = "rayon";

/// Why `name` breaks the scheme, or `Ok`. `name` is a full thread name (or a pool prefix,
/// which [`pool_name`] checks with a two-digit index appended).
pub fn check(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("empty");
    }
    if name.len() > MAX_LEN {
        return Err("longer than 15 bytes (Linux truncates it)");
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("not lowercase ASCII kebab-case");
    }
    if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
        return Err("empty word between dashes");
    }
    if name.starts_with("specular") {
        return Err("starts with `specular` (the process name says that)");
    }
    Ok(())
}

/// Debug builds: warns (with the caller's location) when `name` breaks the scheme. Only a
/// warning: the thread still starts under that name (Linux shows its first 15 bytes), as a
/// name is not worth a crash. The workspace test
/// `every_thread_name_in_the_workspace_follows_the_scheme` is what keeps names on the scheme.
#[track_caller]
fn debug_check(name: &str) {
    if cfg!(debug_assertions)
        && let Err(why) = check(name)
    {
        log::warn!(
            "thread name {name:?} (at {}): {why}",
            std::panic::Location::caller()
        );
    }
}

/// A [`std::thread::Builder`] with `name` set (debug builds warn when it is off the scheme).
#[track_caller]
#[allow(clippy::disallowed_methods)] // The one place that may.
pub fn builder(name: impl Into<String>) -> Builder {
    let name = name.into();
    debug_check(&name);
    Builder::new().name(name)
}

/// [`std::thread::spawn`] with a name: panics if the thread cannot be started, as that does.
#[track_caller]
pub fn spawn<F, T>(name: impl Into<String>, f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    builder(name).spawn(f).expect("failed to spawn thread")
}

/// [`std::thread::Scope::spawn`] with a name: panics if the thread cannot be started, as that
/// does.
#[track_caller]
pub fn spawn_scoped<'scope, 'env, F, T>(
    scope: &'scope Scope<'scope, 'env>,
    name: impl Into<String>,
    f: F,
) -> ScopedJoinHandle<'scope, T>
where
    F: FnOnce() -> T + Send + 'scope,
    T: Send + 'scope,
{
    builder(name)
        .spawn_scoped(scope, f)
        .expect("failed to spawn thread")
}

/// Runs `f` on a thread (`test-stack`) with a stack of `bytes`, and returns its result; a
/// panic in `f` is resumed here. For hostile-input tests: input nested deeper than a
/// decoder's cap overflows a small stack (aborting the test) unless the cap holds.
#[track_caller]
pub fn on_stack<F, T>(bytes: usize, f: F) -> T
where
    F: FnOnce() -> T + Send,
    T: Send,
{
    std::thread::scope(|scope| {
        let handle = builder("test-stack")
            .stack_size(bytes)
            .spawn_scoped(scope, f)
            .expect("failed to spawn thread");
        handle
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

/// Names a pool's threads `{prefix}-{i}`, for `rayon::ThreadPoolBuilder::thread_name`. Debug
/// builds warn when `prefix` leaves no room for a two-digit index.
#[track_caller]
pub fn pool_name(prefix: &'static str) -> impl Fn(usize) -> String + Send + Sync + 'static {
    debug_check(&format!("{prefix}-99"));
    move |i| format!("{prefix}-{i}")
}

/// A rayon pool builder whose threads are named [`pool_name`]`(prefix)`.
///
/// A job `spawn`ed on the pool that panics is dropped instead of aborting the process
/// (rayon's default without a handler); the panic hook has already logged it. Panics under
/// `install`, `scope` and the parallel iterators still reach their caller, whose boundary
/// turns them into an error. Callers may set their own handler.
#[cfg(feature = "rayon")]
#[track_caller]
#[allow(clippy::disallowed_methods)] // The one place that may.
pub fn rayon_pool(prefix: &'static str) -> rayon::ThreadPoolBuilder {
    rayon::ThreadPoolBuilder::new()
        .thread_name(pool_name(prefix))
        .panic_handler(drop)
}

#[cfg(test)]
mod tests;
