//! Debug logging by component (DESIGN.md §12.5, "Debug logging by component").
//!
//! Every Specular crate logs through `tracing` with one of the fixed component targets below:
//!
//! ```ignore
//! use specular_log::{debug, DECODE};
//! debug!(target: DECODE, img = id, "scale 1/{scale}");
//! ```
//!
//! [`init`] reads `SPECULAR_LOG` (and the `--log` flag, same grammar, see [`Spec::parse`]),
//! `SPECULAR_LOG_FILE` and, for third-party crates, `RUST_LOG`. Unless asked for, Specular's
//! components log warnings and errors only and wgpu/naga/sctk only errors.
//!
//! Disabled logging costs one cached check per callsite: the filter is fixed at start-up, so
//! every callsite's interest is decided once and stored in the callsite.
//!
//! The [`Watchdog`] reports images that stay loading too long, whatever the log settings.

mod format;
mod recent;
mod repeat;
mod rotate;

pub use recent::recent_lines;
pub mod service;
mod spec;
pub mod timing;
mod watchdog;

pub use spec::{Spec, SpecWarning};
pub use tracing::{Level, debug, enabled, error, info, trace, warn};
pub use tracing_core::LevelFilter;
pub use watchdog::Watchdog;

use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

pub const STARTUP: &str = "startup";
pub const IO: &str = "io";
pub const FOLDER: &str = "folder";
pub const JOBS: &str = "jobs";
pub const DECODE: &str = "decode";
pub const JPEG: &str = "jpeg";
pub const CACHE: &str = "cache";
pub const PREFETCH: &str = "prefetch";
pub const TILES: &str = "tiles";
pub const GPU: &str = "gpu";
pub const RENDER: &str = "render";
pub const VIEW: &str = "view";
pub const INPUT: &str = "input";
pub const UI: &str = "ui";
pub const TEXT: &str = "text";
pub const COLOUR: &str = "colour";
pub const METADATA: &str = "metadata";
pub const I18N: &str = "i18n";
pub const SETTINGS: &str = "settings";

/// Every component with a one-line description: the single table both the filter parser and
/// `--log-components` use.
pub const COMPONENTS: &[(&str, &str)] = &[
    (
        STARTUP,
        "Process start, argument parsing, cold-start stages, shutdown",
    ),
    (IO, "File open, mmap, reads, file watching"),
    (
        FOLDER,
        "Folder scan, sorting, include-subfolders, neighbour list, file operations (trash/undo)",
    ),
    (
        JOBS,
        "Job pool: submit, start, finish, cancel, priority changes, gating, queue depth",
    ),
    (
        DECODE,
        "Format dispatch, decoder choice, scale and region requests, memory estimates, errors",
    ),
    (
        JPEG,
        "specular-jpeg internals: markers, scan script, parallel/speculative decisions, index builds",
    ),
    (
        CACHE,
        "RAM/VRAM caches, display cache, eviction, memory budget",
    ),
    (PREFETCH, "Prefetch decisions and hits/misses"),
    (
        TILES,
        "Pyramid levels, tile requests, batching, region decodes, uploads",
    ),
    (
        GPU,
        "Adapter/device, surface, pipelines, uploads, presents, device loss",
    ),
    (RENDER, "Frame scheduling, redraw reasons, present timing"),
    (VIEW, "Zoom, pan, fit, animations"),
    (
        INPUT,
        "Keys, mouse, touch, action dispatch, enabled/disabled decisions",
    ),
    (UI, "Widgets, panels, popovers, focus, tooltips"),
    (TEXT, "Font loading, shaping, glyph atlas"),
    (COLOUR, "ICC profiles, transforms, display profile"),
    (
        METADATA,
        "EXIF/XMP/ICC parsing, summary building, geocoding",
    ),
    (
        I18N,
        "Locale negotiation, catalogue loading, missing messages",
    ),
    (SETTINGS, "Settings load, save, watch, --set, validation"),
];

/// Crates that log through the `log` crate under their module path, and their component.
const CRATE_COMPONENTS: &[(&str, &str)] = &[
    ("specular_jpeg", JPEG),
    ("specular_settings", SETTINGS),
    ("specular_i18n", I18N),
    ("specular_metadata", METADATA),
    ("specular_decode", DECODE),
    ("specular_gfx", GPU),
    ("specular_render", RENDER),
    ("specular_widgets", UI),
    ("specular_core", JOBS),
    ("specular", STARTUP),
];

/// The component a target belongs to: a component name itself, or the module path of a
/// Specular crate that logs through `log`.
pub fn component_of(target: &str) -> Option<&'static str> {
    if let Some((c, _)) = COMPONENTS.iter().find(|(c, _)| *c == target) {
        return Some(c);
    }
    let krate = target.split("::").next().unwrap_or(target);
    CRATE_COMPONENTS
        .iter()
        .find(|(k, _)| *k == krate)
        .map(|(_, c)| *c)
}

/// The `--log-components` / `SPECULAR_LOG=help` text, generated from [`COMPONENTS`].
pub fn components_help() -> String {
    let w = COMPONENTS.iter().map(|(c, _)| c.len()).max().unwrap_or(0);
    let mut s = String::from("Log components (SPECULAR_LOG=… or --log …):\n\n");
    for (c, d) in COMPONENTS {
        s.push_str(&format!("  {c:<w$}  {d}\n"));
    }
    s.push_str(
        "\nSyntax: a comma-separated list of COMPONENT[:LEVEL]; `all` means every component.\n\
         LEVEL is off, error, warn, info, debug (the default) or trace; the last match wins.\n\
         Without SPECULAR_LOG every component logs warnings and errors.\n\n\
         Examples:\n  \
         SPECULAR_LOG=all                          everything at debug\n  \
         SPECULAR_LOG=decode,tiles,jobs            just these at debug\n  \
         SPECULAR_LOG=all:info,decode:trace,gpu    per-component levels\n  \
         SPECULAR_LOG_FILE=specular.log             also write the log to a file\n  \
         RUST_LOG=wgpu_core=info                  third-party crates (default: warn; wgpu, naga, sctk: error)\n",
    );
    s
}

static START: OnceLock<Instant> = OnceLock::new();

/// Record the process start time; later calls return the first. Log lines are stamped with
/// milliseconds since this instant.
pub fn mark_process_start() -> Instant {
    *START.get_or_init(Instant::now)
}

/// Milliseconds since [`mark_process_start`].
pub fn ms_since_start() -> f64 {
    mark_process_start().elapsed().as_secs_f64() * 1000.0
}

/// Installs the global subscriber. `cli` is the `--log` value, applied after `SPECULAR_LOG`
/// (so it wins where both name a component). Returns the warnings about the spec (unknown
/// components or levels), which are also logged on the `startup` component.
///
/// Call once, early in `main`; later calls do nothing.
pub fn init(cli: Option<&str>) -> Vec<SpecWarning> {
    init_with_file(cli, None)
}

/// [`init`], also writing the log to `default_file` when `SPECULAR_LOG_FILE` names no other
/// file (`SPECULAR_LOG_FILE=` with no value turns the file off). The file is size-capped:
/// past a few MB it is rotated to `<name>.1.log`, and two old files are kept.
pub fn init_with_file(cli: Option<&str>, default_file: Option<&Path>) -> Vec<SpecWarning> {
    let env = std::env::var("SPECULAR_LOG").ok();
    let spec = Spec::from_sources(env.as_deref(), cli);
    let file = match std::env::var_os("SPECULAR_LOG_FILE") {
        Some(f) if f.is_empty() => None,
        Some(f) => Some(std::path::PathBuf::from(f)),
        None => default_file.map(Path::to_path_buf),
    };
    let rust_log = std::env::var("RUST_LOG")
        .ok()
        .filter(|s| !s.trim().is_empty());
    format::install(&spec, file.as_deref(), rust_log.as_deref(), None);
    for w in &spec.warnings {
        warn!(target: STARTUP, "{w}");
    }
    if !spec.is_default() {
        debug!(target: STARTUP, spec = %spec, "logging");
    }
    spec.warnings
}

/// Tests: installs the process-wide logger with `spec`, writing lines into the returned
/// buffer instead of stderr. `None` when a logger is already installed.
#[doc(hidden)]
pub fn capture(spec: &str) -> Option<std::sync::Arc<parking_lot::Mutex<Vec<u8>>>> {
    let buf = std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
    format::install(&Spec::parse(spec), None, None, Some(buf.clone())).then_some(buf)
}

/// Whether `SPECULAR_LOG` or `value` (the `--log` flag) asks for the component list.
pub fn wants_help(cli: Option<&str>) -> bool {
    let is_help = |s: &str| {
        s.split(',')
            .any(|p| matches!(p.trim(), "help" | "?" | "list"))
    };
    std::env::var("SPECULAR_LOG")
        .ok()
        .is_some_and(|s| is_help(&s))
        || cli.is_some_and(is_help)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_covers_every_component_constant() {
        let consts = [
            STARTUP, IO, FOLDER, JOBS, DECODE, JPEG, CACHE, PREFETCH, TILES, GPU, RENDER, VIEW,
            INPUT, UI, TEXT, COLOUR, METADATA, I18N, SETTINGS,
        ];
        assert_eq!(consts.len(), COMPONENTS.len());
        let help = components_help();
        for c in consts {
            assert!(
                COMPONENTS.iter().any(|(n, _)| *n == c),
                "{c} missing from COMPONENTS"
            );
            assert!(
                help.lines().any(|l| l.split_whitespace().next() == Some(c)),
                "{c} missing from the listing"
            );
            // The parser knows it too.
            let s = Spec::parse(&format!("{c}:trace"));
            assert!(s.warnings.is_empty(), "{c}: {:?}", s.warnings);
            assert_eq!(s.level(c), LevelFilter::TRACE);
        }
        for (c, d) in COMPONENTS {
            assert!(!d.is_empty());
            assert!(help.contains(d), "{c}: description missing");
        }
    }

    #[test]
    fn maps_crate_targets_to_components() {
        assert_eq!(component_of("decode"), Some(DECODE));
        assert_eq!(component_of("specular_settings::store"), Some(SETTINGS));
        assert_eq!(component_of("specular_jpeg"), Some(JPEG));
        // specular-jpeg's tracing events use the bare target "jpeg".
        assert_eq!(component_of("jpeg"), Some(JPEG));
        assert_eq!(component_of("wgpu_core::device"), None);
        assert_eq!(component_of("decoder"), None);
    }
}
