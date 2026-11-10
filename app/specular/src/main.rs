//! Specular Preview: the fastest JPEG viewer.
//!
//! Startup is ordered for the first pixels: the file is opened and its embedded thumbnail
//! and 1/8 decode start on worker threads before anything else, the GPU device is created
//! on another thread, and the window is created on the main thread meanwhile.

// The remote-control socket (remote/) calls methods of `specular_platform::ipc::Stream`, which
// is std's AF_UNIX stream on Windows too; the gate applies to every crate that calls them.
#![cfg_attr(windows, feature(windows_unix_domain_sockets))]
// Windows: a GUI program, so starting it from Explorer opens no console window; `main`
// attaches to the terminal's console for the command-line modes. Test builds stay console
// programs, so the test harness's output reaches the terminal.
#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod a11y;
mod anim;
mod app;
mod archive_thumbs;
mod book;
mod clipboard;
mod cog_remote;
mod copc_remote;
#[cfg(all(unix, not(target_os = "macos")))]
mod desktop_search;
mod develop_export_cli;
mod display_cache;
mod error_text;
mod file_list;
mod hw_report;
mod integration_cli;
mod loader;
mod open_report;
mod quit_signal;
mod remote;
mod sound;
mod stac_remote;
#[cfg(test)]
mod test_host;
mod thumbnail;
mod tiles3d_remote;
mod zarr_remote;
mod zynk;

use std::path::PathBuf;

use specular_core::{CancelToken, HardwareProfile, Priority};

/// The release build targets x86-64-v3 (AVX2, FMA, BMI1/2, LZCNT, MOVBE). On an older
/// processor the first AVX2 instruction would kill the process with SIGILL and no
/// explanation; this checks with CPUID first and explains instead. It runs before any of our
/// code that could use those instructions (the standard library and mimalloc's C code are
/// built for the baseline) and avoids them itself: `is_x86_feature_detected!` can't be used,
/// as it answers "yes" at compile time when the crate is built with the feature enabled.
///
/// The message is English: the translations are not loaded yet, and loading them is the
/// kind of code that could already use AVX2.
#[cfg(target_arch = "x86_64")]
#[inline(never)]
fn require_cpu_features() {
    let missing = missing_cpu_features(|leaf, sub| {
        let r = std::arch::x86_64::__cpuid_count(leaf, sub);
        [r.eax, r.ebx, r.ecx, r.edx]
    });
    if missing.is_empty() {
        return;
    }
    use std::io::Write;
    let _ = writeln!(
        std::io::stderr(),
        "Specular can't run on this processor: it needs the x86-64-v3 instruction set \
         (Intel Haswell from 2013 or later, AMD Zen or later), and this processor lacks {}.",
        missing.join(", ")
    );
    // Started from Explorer there is no terminal for that line: a message box says it (with the
    // text made at compile time, so nothing here converts it).
    specular_platform::error_box_without_terminal(&CPU_BOX_TITLE, &CPU_BOX_TEXT);
    std::process::exit(1);
}

/// The processor check's message box, as NUL-terminated UTF-16 made at compile time.
#[cfg(target_arch = "x86_64")]
static CPU_BOX_TITLE: [u16; 9] = ascii_utf16("Specular");
#[cfg(target_arch = "x86_64")]
static CPU_BOX_TEXT: [u16; CPU_BOX_MESSAGE.len() + 1] = ascii_utf16(CPU_BOX_MESSAGE);
#[cfg(target_arch = "x86_64")]
const CPU_BOX_MESSAGE: &str = "Specular can't run on this processor.\n\nIt needs the x86-64-v3 \
     instruction set (AVX2, FMA and BMI2): Intel Haswell from 2013 or later, AMD Zen or later.";

/// `s` as NUL-terminated UTF-16, at compile time. ASCII only (checked when compiling), so each
/// byte is one code unit; `N` is the length plus the NUL.
#[cfg(target_arch = "x86_64")]
const fn ascii_utf16<const N: usize>(s: &str) -> [u16; N] {
    let b = s.as_bytes();
    assert!(b.len() + 1 == N, "N is the length plus the NUL");
    let mut w = [0u16; N];
    let mut i = 0;
    while i < b.len() {
        assert!(b[i] < 0x80, "ASCII only");
        w[i] = b[i] as u16;
        i += 1;
    }
    w
}

/// The x86-64-v3 features `cpuid(leaf, subleaf) → [eax, ebx, ecx, edx]` reports missing.
#[cfg(target_arch = "x86_64")]
fn missing_cpu_features(cpuid: impl Fn(u32, u32) -> [u32; 4]) -> Vec<&'static str> {
    let bit = |r: u32, b: u32| r >> b & 1 == 1;
    let max = cpuid(0, 0)[0];
    let l1 = cpuid(1, 0);
    let l7 = if max >= 7 { cpuid(7, 0) } else { [0; 4] };
    let e1 = if cpuid(0x8000_0000, 0)[0] >= 0x8000_0001 {
        cpuid(0x8000_0001, 0)
    } else {
        [0; 4]
    };
    let (ecx1, ebx7, ecx_e1) = (l1[2], l7[1], e1[2]);
    [
        // The OS saves the extended (YMM) register state: without it AVX is unusable.
        ("OSXSAVE", bit(ecx1, 27)),
        ("AVX", bit(ecx1, 28)),
        ("AVX2", bit(ebx7, 5)),
        ("FMA", bit(ecx1, 12)),
        ("BMI1", bit(ebx7, 3)),
        ("BMI2", bit(ebx7, 8)),
        ("LZCNT", bit(ecx_e1, 5)),
        ("MOVBE", bit(ecx1, 22)),
    ]
    .into_iter()
    .filter(|c| !c.1)
    .map(|c| c.0)
    .collect()
}

#[cfg(not(target_arch = "x86_64"))]
fn require_cpu_features() {}

/// Command-line options.
#[derive(Default)]
pub struct Args {
    pub path: Option<PathBuf>,
    /// `--compare A B [C D]`: these files compared (the gallery's compare); [`finish_args`]
    /// checks them and makes the first the path.
    pub compare: Vec<PathBuf>,
    /// `--compare-mode side|split|overlay|diff`.
    pub compare_mode: Option<String>,
    compare_flag: bool,
    pub hw_report: bool,
    /// `--no-network`: nothing connects this run (Settings › Privacy can't undo it).
    pub no_network: bool,
    pub bench_open: Option<PathBuf>,
    /// Hidden: with --bench-open, hold → (simulated key repeat) through this many images.
    pub bench_hold: Option<usize>,
    /// Hidden: with --bench-hold, the key repeat as `DELAY_MS,INTERVAL_MS` (default 500,30;
    /// the toolbar's held Previous/Next repeats at 400,80).
    pub bench_repeat: Option<(u64, u64)>,
    /// Hidden: with --bench-open, pan and zoom programmatically for this many seconds once the
    /// image is sharp, and report the frame-time distribution.
    pub bench_pan: Option<f64>,
    /// Hidden: with --bench-open, tap the loupe on once the image is sharp, move a simulated
    /// pointer (jumps, then a sweep for this many seconds) and report frame and tile timings.
    pub bench_loupe: Option<f64>,
    pub sets: Vec<String>,
    pub lang: Option<String>,
    /// Hidden: render one frame to a PNG once the image is sharp, then exit.
    pub screenshot: Option<PathBuf>,
    /// Hidden: overlays to open before the screenshot (info, palette, cheatsheet, about, hud,
    /// scrollbars, settings, toolbar, language; cull, cull-pick and cull-rate-N for cull mode).
    pub show: Vec<String>,
    /// Hidden: palette query to type before the screenshot.
    pub query: Option<String>,
    /// Hidden: window size in logical pixels.
    pub size: Option<(u32, u32)>,
    /// Hidden: zoom (percent) before the screenshot.
    pub zoom: Option<f32>,
    /// `--log SPEC`: component logging, same grammar as `SPECULAR_LOG` (applied after it).
    pub log: Option<String>,
    pub log_components: bool,
    /// Hidden (tests): open these while running, 1.5 s apart, the way a drop or the open
    /// dialog does.
    pub then_open: Vec<PathBuf>,
    /// `--thumbnail-cache DIR`: fill the gallery's thumbnail cache for DIR and exit.
    pub thumbnail_cache: Option<PathBuf>,
    /// `--depth N` with `--thumbnail-cache`: subfolder levels to include.
    pub depth: Option<u32>,
    /// `specular -`, `--files-from FILE`: browse exactly these files (`file_list.rs`).
    pub file_list: Vec<PathBuf>,
    /// `-0`: lists in and out are NUL-separated.
    pub null: bool,
    /// `--print-marked`: write the picked files to standard output on exit.
    pub print_marked: bool,
    /// `specular:` links that are not plain local files, opened (or asked about) once the
    /// window is up.
    pub urls: Vec<String>,
    /// `--library`: start in the Library (the launcher menu's "Library").
    pub library: bool,
    /// `--search QUERY`: start in the Library searching for QUERY (the desktop search's
    /// "more results").
    pub library_search: Option<String>,
    /// `--slideshow [PATH]`: a slideshow of PATH's folder (default: the Pictures folder);
    /// [`finish_args`] moves PATH here from `path`.
    pub slideshow: Option<PathBuf>,
    slideshow_flag: bool,
    /// Not an option: the crash report the last run left (the UI thread panicked), which the
    /// window mentions once. Set by `main`.
    pub last_crash: Option<PathBuf>,
    /// A Zarr store's address given instead of a file (`https://…/x.zarr`, `s3://…`):
    /// opened by the window, which reads it lazily.
    pub remote_zarr: Option<String>,
    /// A COPC or Cloud-Optimized GeoTIFF address given instead of a file: fetched into the
    /// cache by the window (`app/zarr.rs`), so its progress shows in "Downloads".
    pub remote_fetch: Option<String>,
}

fn usage() -> ! {
    println!(
        "Usage: specular [FILE…|FOLDER|specular:URL]\n       \
         specular [-0] [--print-marked] - | --files-from LIST\n       \
         specular --remote COMMAND\n\n\
         Options:\n  \
         --new-window           always open a new window (no single instance)\n  \
         --no-network           nothing connects to a server this run\n  \
         --library              start in the Library\n  \
         --search QUERY         start in the Library, searching for QUERY\n  \
         --slideshow [PATH]     a slideshow of PATH's folder (default: Pictures)\n  \
         --set KEY=VALUE        change a setting for this session\n  \
         --compare A B [C D]    compare 2 to 4 images (4-up beyond two)\n  \
         --compare-mode MODE    with --compare: side, split, overlay or diff\n  \
         --lang ID              user interface language (e.g. de, ar, ja)\n  \
         -                      browse the files listed on standard input, in order\n  \
         --files-from LIST      browse the files listed in LIST (- is standard input):\n                         \
         one path or file:// URL per line\n  \
         -0, --null             lists are NUL-separated (find -print0), in and out\n  \
         --print-marked         on exit, print the picked files (flag P) of the\n                         \
         listing to standard output\n  \
         --thumbnail IN OUT SIZE  write a SIZE px PNG thumbnail of IN to OUT and exit\n  \
         --thumbnail-cache DIR  fill the gallery's thumbnail cache for DIR and exit\n                         \
         (--depth N also includes N levels of subfolders)\n  \
         -h, --help             this help\n\n\
         Diagnostics:\n  \
         --hw-report            print hardware and GPU facts and exit\n  \
         --bench-open FILE      open FILE, print timings as JSON (with the open report:\n                         \
         why it took as long as it did) and exit\n  \
         --log SPEC             debug logging per component, e.g. all, decode,tiles,\n                         \
         jobs:trace or all:info,gpu:off (applied after SPECULAR_LOG)\n  \
         --log-components       list the log components and the level syntax, and exit\n\n\
         {}\n\n\
         {}\n\n\
         {}\n\n\
         Environment:\n  \
         SPECULAR_LOG=SPEC       like --log; SPECULAR_LOG=help lists the components\n  \
         SPECULAR_LOG_FILE=PATH  write the log file to PATH instead of the log folder\n                         \
         (empty: no log file)\n  \
         SPECULAR_OPEN_REPORT=1  print one line per image shown to stderr: where its\n                         \
         open time went\n  \
         SPECULAR_SOCKET=PATH   the socket --remote talks to\n  \
         SPECULAR_NO_NETWORK=1  like --no-network\n  \
         RUST_LOG=FILTER        logging of third-party crates (wgpu, naga, ...)",
        integration_cli::USAGE,
        develop_export_cli::USAGE,
        remote::client::USAGE
    );
    std::process::exit(0)
}

fn parse_args() -> Args {
    let mut a = parse_arg_list(std::env::args_os().skip(1));
    read_file_lists(&mut a);
    finish_args(&mut a);
    if let Err(e) = check_compare(&mut a, |p| p.is_file()) {
        eprintln!("specular: {e}");
        std::process::exit(2);
    }
    // A Zarr store on a server or in a bucket: the window opens it, chunk by chunk.
    if let Some(url) = a
        .path
        .as_ref()
        .and_then(|p| p.to_str())
        .filter(|s| zarr_remote::is_remote_zarr(s))
        .map(str::to_owned)
    {
        a.path = None;
        a.remote_zarr = Some(url);
    }
    // A COPC point cloud on a server: its levels that fit the point budget, by ranges,
    // fetched once the window is there (with its progress in "Downloads").
    if let Some(url) = a
        .path
        .as_ref()
        .and_then(|p| p.to_str())
        .filter(|s| copc_remote::is_remote_copc(s))
        .map(str::to_owned)
    {
        a.path = None;
        a.remote_fetch = Some(url);
    }
    // A Cloud-Optimized GeoTIFF on a server: the finest level that fits, by ranges, also
    // once the window is there.
    if let Some(url) = a
        .path
        .as_ref()
        .and_then(|p| p.to_str())
        .filter(|s| cog_remote::is_remote_cog(s) || stac_remote::is_remote_stac(s))
        .map(str::to_owned)
    {
        a.path = None;
        a.remote_fetch = Some(url);
    }
    a
}

/// `--slideshow`: the path argument (or the Pictures folder) is the show's, not a file to
/// open first.
fn finish_args(a: &mut Args) {
    if a.slideshow_flag {
        let p = a
            .path
            .take()
            .unwrap_or_else(specular_platform::integration::user_pictures_dir);
        a.slideshow = Some(std::path::absolute(&p).unwrap_or(p));
    }
}

/// The most files `--compare` takes: the 4-up view.
const COMPARE_MAX: usize = 4;

/// `--compare`: 2 to 4 existing files (`exists`) and a known `--compare-mode`; the first
/// becomes the path the window opens. A plain message when not.
fn check_compare(a: &mut Args, exists: impl Fn(&std::path::Path) -> bool) -> Result<(), String> {
    if !a.compare_flag {
        return match a.compare_mode {
            Some(_) => Err("--compare-mode goes with --compare".into()),
            None => Ok(()),
        };
    }
    if let Some(m) = &a.compare_mode
        && !remote::protocol::COMPARE_MODES.contains(&m.as_str())
    {
        return Err(format!(
            "--compare-mode is side, split, overlay or diff, not {m:?}"
        ));
    }
    // A path given before `--compare` is compared too.
    if let Some(p) = a.path.take() {
        a.compare.insert(0, p);
    }
    match a.compare.len() {
        0 | 1 => return Err("--compare needs 2 to 4 files".into()),
        n if n > COMPARE_MAX => {
            return Err(format!(
                "--compare takes at most {COMPARE_MAX} files (the 4-up view), not {n}"
            ));
        }
        _ => {}
    }
    if let Some(p) = a.compare.iter().find(|p| !exists(p)) {
        return Err(format!("no such file: {}", p.display()));
    }
    a.compare = a
        .compare
        .iter()
        .map(|p| std::path::absolute(p).unwrap_or_else(|_| p.clone()))
        .collect();
    a.path = a.compare.first().cloned();
    Ok(())
}

/// The list sources (`-`, `--files-from`) are read once all options are known (`-0` may come
/// last); `Args::path` becomes the first listed file.
fn read_file_lists(a: &mut Args) {
    for src in std::mem::take(&mut a.file_list) {
        match file_list::read(&src, a.null) {
            Ok((files, dropped)) => {
                if dropped > 0 {
                    eprintln!("specular: skipped {dropped} listed entries that are not files");
                }
                a.file_list.extend(files);
            }
            Err(e) => {
                eprintln!("specular: {}: {e}", src.display());
                std::process::exit(2);
            }
        }
    }
    a.file_list.truncate(file_list::MAX_ENTRIES);
    if let Some(first) = a.file_list.first() {
        a.path = Some(first.clone());
    }
}

/// Parses the arguments; `Args::file_list` holds the list sources, not yet read.
fn parse_arg_list(args: impl Iterator<Item = std::ffi::OsString>) -> Args {
    let mut a = Args::default();
    let mut it = args;
    let mut only_paths = false;
    let mut stdin_list = false;
    while let Some(arg) = it.next() {
        let s = arg.to_string_lossy().into_owned();
        if !only_paths && s == "-" {
            if !stdin_list {
                stdin_list = true;
                a.file_list.push(PathBuf::from("-"));
            }
            continue;
        }
        if !only_paths
            && let Some(u) = arg.to_str()
            && specular_platform::url_scheme::is_url(u)
        {
            // Plain local files are file arguments; anything else is asked about (or
            // refused) in the window.
            match specular_platform::url_scheme::parse(u) {
                Ok(specular_platform::url_scheme::Target::Local(paths)) => {
                    a.path = paths.into_iter().next();
                }
                _ => a.urls.push(u.to_owned()),
            }
            continue;
        }
        if only_paths || !s.starts_with('-') {
            if a.compare_flag {
                a.compare.push(path_arg(arg));
            } else {
                a.path = Some(path_arg(arg));
            }
            continue;
        }
        let (name, inline) = match s.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_owned(), Some(v.to_owned())),
            _ => (s.clone(), None),
        };
        let value = |it: &mut dyn Iterator<Item = std::ffi::OsString>| -> String {
            inline
                .clone()
                .or_else(|| it.next().map(|v| v.to_string_lossy().into_owned()))
                .unwrap_or_else(|| {
                    eprintln!("specular: {name} needs a value");
                    std::process::exit(2)
                })
        };
        match name.as_str() {
            "--" => only_paths = true,
            "-h" | "--help" => usage(),
            "--hw-report" => a.hw_report = true,
            "--no-network" => a.no_network = true,
            "--bench-open" => a.bench_open = Some(PathBuf::from(value(&mut it))),
            "--set" => a.sets.push(value(&mut it)),
            "--lang" => a.lang = Some(value(&mut it)),
            "--log" => a.log = Some(value(&mut it)),
            "--log-components" => a.log_components = true,
            // Started by the session bus (D-Bus activation); see `integration_cli`.
            "--gapplication-service" => {}
            "--thumbnail-cache" => a.thumbnail_cache = Some(path_arg(value(&mut it).into())),
            "--depth" => a.depth = value(&mut it).parse().ok(),
            // Any option makes a separate instance (see `integration_cli`); nothing else to do.
            "--new-window" => {}
            "--library" => a.library = true,
            "--search" => {
                a.library = true;
                a.library_search = Some(value(&mut it));
            }
            "--slideshow" => a.slideshow_flag = true,
            "--then-open" => a.then_open.push(path_arg(value(&mut it).into())),
            "--screenshot" => a.screenshot = Some(PathBuf::from(value(&mut it))),
            "--show" => a.show.extend(value(&mut it).split(',').map(str::to_owned)),
            "--compare" => a.compare_flag = true,
            "--compare-mode" => a.compare_mode = Some(value(&mut it)),
            "--query" => a.query = Some(value(&mut it)),
            "--zoom" => a.zoom = value(&mut it).trim_end_matches('%').parse().ok(),
            "--bench-hold" => a.bench_hold = value(&mut it).parse().ok(),
            "--bench-repeat" => {
                let v = value(&mut it);
                a.bench_repeat = v
                    .split_once(',')
                    .and_then(|(d, i)| Some((d.parse().ok()?, i.parse().ok()?)));
            }
            "--bench-pan" => a.bench_pan = value(&mut it).parse().ok(),
            "--bench-loupe" => a.bench_loupe = value(&mut it).parse().ok(),
            "--size" => {
                let v = value(&mut it);
                a.size = v
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
            }
            "--files-from" => {
                let v = PathBuf::from(value(&mut it));
                if v != std::path::Path::new("-") || !stdin_list {
                    stdin_list |= v == std::path::Path::new("-");
                    a.file_list.push(v);
                }
            }
            "-0" | "--null" => a.null = true,
            "--print-marked" => a.print_marked = true,
            _ => {
                eprintln!("specular: unknown option {s} (try --help)");
                std::process::exit(2)
            }
        }
    }
    a
}

/// A path argument as file managers and desktop files may pass it: a `file://` URL
/// (percent-encoded) or a path wrapped in quotes by a broken `Exec=` line. Anything else,
/// and anything that does not exist once unwrapped, is taken literally.
fn path_arg(arg: std::ffi::OsString) -> PathBuf {
    let Some(s) = arg.to_str() else {
        return PathBuf::from(arg);
    };
    if let Some(rest) = s.strip_prefix("file://") {
        // file:///path or file://localhost/path.
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        if rest.starts_with('/')
            && let Some(p) = percent_decode(rest)
        {
            return p;
        }
    }
    for q in ['"', '\''] {
        if let Some(inner) = s.strip_prefix(q).and_then(|r| r.strip_suffix(q))
            && !std::path::Path::new(s).exists()
        {
            let p = path_arg(inner.into());
            if p.exists() {
                return p;
            }
        }
    }
    PathBuf::from(arg)
}

fn percent_decode(s: &str) -> Option<PathBuf> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Some(PathBuf::from(std::ffi::OsString::from_vec(out)))
    }
    // file:///C:/dir/x.jpg: the path is UTF-8 and starts after the slash before the drive.
    #[cfg(not(unix))]
    {
        let s = String::from_utf8(out).ok()?;
        let drive = s
            .as_bytes()
            .get(1..3)
            .is_some_and(|d| d[0].is_ascii_alphabetic() && d[1] == b':');
        Some(PathBuf::from(if drive { &s[1..] } else { &s[..] }))
    }
}

/// Builds rayon's global pool (which the decoders' automatic parallelism uses) now, from this
/// unpinned thread, with a panic handler: a panic in a detached `rayon::spawn` would otherwise
/// abort the process (a panic in `join`, `scope` or a parallel iterator comes back to the
/// caller, whose job boundary catches it). On macOS every worker runs at QoS
/// USER_INITIATED, so decode work stays on performance cores whichever thread first touched
/// the pool. Costs one thread spawn per core, which the first parallel decode would pay anyway.
fn rayon_pool() {
    let b = specular_thread::rayon_pool(specular_thread::GLOBAL_POOL)
        .panic_handler(specular_core::isolate::rayon_panic_handler);
    #[cfg(target_os = "macos")]
    let b = b.start_handler(|_| {
        specular_platform::set_thread_class(specular_platform::ThreadClass::UserInitiated)
    });
    let _ = b.build_global();
}

/// Tells the Vulkan loader not to load drivers for hardware this machine lacks, which takes
/// 20-25 ms off GPU start-up, nor Mesa's device-selection layer on a machine with one GPU
/// (2-3 ms; UNSAFE.md). Must run before any other thread exists.
/// The names set here are listed in `SPECULAR_SCRUB_ENV`, so programs Specular starts don't
/// inherit them (`specular_platform::launch::scrub_env`).
#[allow(unsafe_code)]
fn skip_absent_vulkan_drivers() {
    let mut set = Vec::new();
    if let Some(globs) = specular_gfx::gpu::vulkan_drivers_to_skip() {
        // SAFETY: called first thing in `main`, before this process starts any thread, so
        // nothing can read the environment concurrently (the only requirement of `set_var`).
        unsafe { std::env::set_var("VK_LOADER_DRIVERS_DISABLE", globs) };
        set.push("VK_LOADER_DRIVERS_DISABLE");
    }
    if let Some(layers) = specular_gfx::gpu::vulkan_layers_to_skip() {
        // SAFETY: as above: no other thread exists yet.
        unsafe { std::env::set_var("VK_LOADER_LAYERS_DISABLE", layers) };
        set.push("VK_LOADER_LAYERS_DISABLE");
    }
    if !set.is_empty() {
        // SAFETY: as above: no other thread exists yet.
        unsafe { std::env::set_var(specular_platform::launch::SCRUB_ENV, set.join(",")) };
    }
}

fn main() {
    // Windows: print to the terminal Specular was started from (a GUI-subsystem program has no
    // console of its own); before anything, so the processor check's message reaches it too.
    let console = specular_platform::attach_parent_console();
    // First: everything below (and every crate) is compiled for x86-64-v3.
    require_cpu_features();
    // Windows: the taskbar id of the installer's shortcut, before any window or recent file.
    #[cfg(windows)]
    specular_platform::set_app_user_model_id(app::APP_ID);
    // Movies (MP4, MOV, M4V, 3GP) open as images through specular-video, in the viewer, the
    // gallery's thumbnails, the library's index and the desktop thumbnailer alike.
    specular_video::install();
    // The desktop thumbnailer: no window, no GPU, no log file.
    if let Some(code) = thumbnail::from_args() {
        std::process::exit(code);
    }
    // `--remote COMMAND`: control a running viewer through its socket.
    if let Some(code) = remote::client::from_args() {
        std::process::exit(code);
    }
    // `develop export FILE…`: Develop's export without a window.
    if let Some(code) = develop_export_cli::from_args() {
        std::process::exit(code);
    }
    // Desktop integration commands, and handing files to a running viewer (single instance).
    if let Some(code) = integration_cli::from_args() {
        std::process::exit(code);
    }
    let start = specular_core::mark_process_start();
    skip_absent_vulkan_drivers();
    // The UI/render thread is the most urgent one (macOS QoS USER_INTERACTIVE; no-op elsewhere).
    // Threads spawned from here on start in this class: the GPU and font threads want it, the
    // decode workers set their own per job.
    specular_platform::set_thread_class(specular_platform::ThreadClass::Interactive);
    rayon_pool();
    let mut args = parse_args();
    // `--compare` with a viewer running (single instance): compared there.
    if !args.compare.is_empty()
        && integration_cli::single_instance_on()
        && remote::forward_compare(
            &args.compare,
            args.compare_mode.as_deref().unwrap_or("side"),
        )
    {
        return;
    }
    // Before any window or GPU work: listing the components needs neither.
    if args.log_components || specular_log::wants_help(args.log.as_deref()) {
        print!("{}", specular_log::components_help());
        return;
    }
    // The default log file: `specular.log` in the log folder (size-capped, rotated).
    let locations = specular_settings::Locations::detect();
    let log_file = locations.log_dir.join("specular.log");
    specular_log::init_with_file(args.log.as_deref(), Some(&log_file));
    // A panic in a job costs that job (`specular_core::isolate`); one on this (the UI) thread
    // ends the app and leaves a local crash report, which the next start mentions. Nothing is
    // ever sent anywhere.
    let crash_dir = locations.crash_dir();
    args.last_crash = specular_core::isolate::take_pending_report(&crash_dir);
    specular_core::isolate::install(Some(crash_dir));
    app::install_report_hooks();
    // The no-network switch from the flag, SPECULAR_NO_NETWORK and the policy file, before
    // anything could connect (the setting follows once the settings are loaded).
    app::privacy::startup(args.no_network);
    // `.omero` files (images on OMERO servers) open through their server; the tile cache is
    // opened on first use.
    specular_omero::registry::install();
    specular_omero::registry::set_cache_dir(
        locations.cache_dir.join("omero").join("tiles"),
        2 << 30,
    );
    // `.zarrurl` links (Zarr stores on servers) open through their store; chunks are cached
    // on first use.
    zarr_remote::install(locations.cache_dir.clone());
    // STAC documents' linked items and GeoTIFF assets on servers come through the cache.
    stac_remote::install(locations.cache_dir.clone());
    // 3D Tiles on servers: external tilesets, subtrees and tiles come through the cache.
    tiles3d_remote::install(locations.cache_dir.clone());
    // Formats chosen for shared extensions ("Always use this for .pic files"), before the
    // first file opens.
    if args.screenshot.is_none() && args.bench_open.is_none() {
        app::file_kind_startup();
    }
    if args.hw_report {
        hw_report::run();
        return;
    }
    if let Some(dir) = &args.thumbnail_cache {
        let settings = specular_settings::SettingsStore::open_default()
            .map(|s| {
                // `--set gallery.cache_size=…` / `gallery.cache_location=…` apply here too.
                for (arg, e) in s.apply_set_args(&args.sets) {
                    eprintln!("specular: --set {arg}: {e}");
                }
                s.snapshot()
            })
            .unwrap_or_default();
        std::process::exit(app::warm_cache(dir, args.depth.unwrap_or(0), &settings));
    }
    // Benchmarks must not be napped when the window is hidden (macOS App Nap; no-op elsewhere).
    let _no_nap = args
        .bench_open
        .is_some()
        .then(|| specular_platform::begin_activity("Specular benchmark", true));

    // 1. Start decoding before anything else.
    let hw = HardwareProfile::detect();
    specular_core::placement::init(&hw.performance_cpus);
    let (loader, events) = loader::Loader::new(hw.decode_threads());
    let target = args.bench_open.clone().or_else(|| args.path.clone());
    let target = target.map(|p| std::path::absolute(&p).unwrap_or(p));
    let mut first = None;
    if let Some(p) = &target
        && p.is_file()
        && !book::is_archive_path(p)
    {
        let token = CancelToken::new();
        // Its levels may come from the display cache, which the settings (read in `app::run`)
        // turn on.
        if let Some(dir) = app::probable_display_cache_dir(&args) {
            loader.expect_display_cache(dir);
        }
        let h = loader.open(1, p.clone(), Priority::Immediate, None, token);
        first = Some((p.clone(), h, std::time::Instant::now()));
    }
    // 2. GPU device and fonts in parallel with window creation.
    let gpu = specular_gfx::Gpu::spawn();
    let text = specular_thread::builder("font-load")
        .spawn(specular_gfx::Text::new)
        .expect("spawn font thread");
    specular_log::debug!(target: specular_log::STARTUP, "{:.1} ms to spawn the decoder, GPU and font threads", start.elapsed().as_secs_f64() * 1000.0);

    // Windows: the window does not depend on the terminal it was started from (a Ctrl+C typed
    // there later must not close it), unless logging to it was asked for or the picked files
    // are printed to it on exit.
    if console
        && args.log.is_none()
        && std::env::var_os("SPECULAR_LOG").is_none()
        && !args.print_marked
    {
        specular_platform::detach_console();
    }
    if let Err(e) = app::run(args, hw, loader, events, target, first, gpu, text) {
        eprintln!("specular: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Nehalem-class CPU (SSE4.2, no AVX) is told what it lacks; this machine (which runs
    /// the x86-64-v3 test binary) lacks nothing.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn cpu_feature_check() {
        let nehalem = |leaf: u32, _sub: u32| match leaf {
            0 => [0xb, 0, 0, 0],
            // SSE3, SSSE3, SSE4.1/4.2, POPCNT; no FMA, MOVBE, OSXSAVE, AVX.
            1 => [0, 0, 1 | 1 << 9 | 1 << 19 | 1 << 20 | 1 << 23, 0],
            7 => [0; 4],
            0x8000_0000 => [0x8000_0008, 0, 0, 0],
            _ => [0; 4],
        };
        assert_eq!(
            missing_cpu_features(nehalem),
            [
                "OSXSAVE", "AVX", "AVX2", "FMA", "BMI1", "BMI2", "LZCNT", "MOVBE"
            ]
        );
        let real = missing_cpu_features(|l, s| {
            let r = std::arch::x86_64::__cpuid_count(l, s);
            [r.eax, r.ebx, r.ecx, r.edx]
        });
        assert!(real.is_empty(), "{real:?}");
    }

    /// The message box text made at compile time reads back as the message, NUL-terminated.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn cpu_box_strings() {
        let text = |w: &[u16]| {
            assert_eq!(w.last(), Some(&0));
            String::from_utf16(&w[..w.len() - 1]).unwrap()
        };
        assert_eq!(text(&CPU_BOX_TITLE), "Specular");
        assert_eq!(text(&CPU_BOX_TEXT), CPU_BOX_MESSAGE);
    }

    #[test]
    fn path_arguments_from_open_with() {
        let dir = std::path::absolute(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/path-arg"
        ))
        .unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a b%.jpg");
        std::fs::write(&f, b"x").unwrap();
        // A file URL's path is `/`-separated, and on Windows `/C:/…` (file:///C:/…).
        let url_path = {
            let s = dir
                .join("a%20b%25.jpg")
                .display()
                .to_string()
                .replace('\\', "/");
            if s.starts_with('/') {
                s
            } else {
                format!("/{s}")
            }
        };
        let url = format!("file://{url_path}");
        assert_eq!(path_arg(url.into()), f);
        let url = format!("file://localhost{url_path}");
        assert_eq!(path_arg(url.into()), f);
        assert_eq!(path_arg(format!("\"{}\"", f.display()).into()), f);
        assert_eq!(path_arg(format!("'{}'", f.display()).into()), f);
        // Literal names are kept: relative paths, and quotes that do not unwrap to a file.
        assert_eq!(path_arg("x.jpg".into()), PathBuf::from("x.jpg"));
        assert_eq!(
            path_arg("\"missing.jpg\"".into()),
            PathBuf::from("\"missing.jpg\"")
        );
        assert_eq!(
            path_arg("file://host/x.jpg".into()),
            PathBuf::from("file://host/x.jpg")
        );
    }

    fn args(v: &[&str]) -> Args {
        parse_arg_list(v.iter().map(std::ffi::OsString::from))
    }

    #[test]
    fn compare_arguments() {
        let all = |_: &std::path::Path| true;
        let mut a = args(&["--compare", "a.jpg", "b.jpg", "--compare-mode", "split"]);
        assert!(check_compare(&mut a, all).is_ok());
        assert_eq!(a.compare.len(), 2);
        assert!(a.compare.iter().all(|p| p.is_absolute()));
        assert!(a.compare[0].ends_with("a.jpg") && a.compare[1].ends_with("b.jpg"));
        assert_eq!(a.path.as_ref(), a.compare.first());
        assert_eq!(a.compare_mode.as_deref(), Some("split"));
        // A path before the flag counts; four is the 4-up view.
        let mut a = args(&["a.jpg", "--compare", "b.jpg", "c.jpg", "d.jpg"]);
        assert!(check_compare(&mut a, all).is_ok());
        assert_eq!(a.compare.len(), 4);
        assert!(a.compare[0].ends_with("a.jpg"));
        // Refused plainly: too few, too many, an unknown mode, a missing file, a mode alone.
        let err = |v: &[&str], exists: &dyn Fn(&std::path::Path) -> bool| {
            check_compare(&mut args(v), exists).unwrap_err()
        };
        assert_eq!(
            err(&["--compare", "a.jpg"], &all),
            "--compare needs 2 to 4 files"
        );
        let five = ["--compare", "1.jpg", "2.jpg", "3.jpg", "4.jpg", "5.jpg"];
        assert!(err(&five, &all).contains("at most 4 files"));
        assert!(
            err(&["--compare", "a", "b", "--compare-mode", "wipe"], &all).contains("side, split")
        );
        assert_eq!(
            err(&["--compare", "a.jpg", "gone.jpg"], &|p| !p
                .ends_with("gone.jpg")),
            "no such file: gone.jpg"
        );
        assert_eq!(
            err(&["--compare-mode", "diff", "a.jpg"], &all),
            "--compare-mode goes with --compare"
        );
        // Without it nothing changes.
        let mut a = args(&["a.jpg"]);
        assert!(check_compare(&mut a, all).is_ok());
        assert!(a.compare.is_empty() && a.path.is_some());
    }

    #[test]
    fn file_list_and_link_arguments() {
        let a = args(&["-", "-0", "--print-marked"]);
        assert_eq!(a.file_list, [PathBuf::from("-")]);
        assert!(a.null && a.print_marked && a.path.is_none());
        // Standard input is read once, however it is named.
        let a = args(&["--files-from", "-", "-", "--files-from=/l.txt", "--null"]);
        assert_eq!(a.file_list, [PathBuf::from("-"), PathBuf::from("/l.txt")]);
        assert!(a.null);
        // After `--`, `-` is a file name.
        let a = args(&["--", "-"]);
        assert!(a.file_list.is_empty());
        assert_eq!(a.path, Some(PathBuf::from("-")));
        // Links: plain local files become the path; others wait for the window.
        #[cfg(unix)]
        {
            let a = args(&["specular:///home/me/a%20b.jpg"]);
            assert_eq!(a.path, Some(PathBuf::from("/home/me/a b.jpg")));
            assert!(a.urls.is_empty());
        }
        let a = args(&["specular://study?url=https%3A%2F%2Fx"]);
        assert_eq!(a.urls, ["specular://study?url=https%3A%2F%2Fx"]);
        let a = args(&["specular:nonsense"]);
        assert_eq!(a.urls, ["specular:nonsense"]);
    }

    #[test]
    fn launcher_action_arguments() {
        let a = args(&["--library"]);
        assert!(a.library && a.slideshow.is_none());
        let a = args(&["--search", "tag:sea -is:raw"]);
        assert!(a.library);
        assert_eq!(a.library_search.as_deref(), Some("tag:sea -is:raw"));
        // Absolute on this platform (`/photos` has no drive on Windows).
        let trip = if cfg!(windows) {
            r"C:\photos\trip"
        } else {
            "/photos/trip"
        };
        let mut a = args(&["--slideshow", trip]);
        finish_args(&mut a);
        assert_eq!(a.slideshow, Some(PathBuf::from(trip)));
        assert!(
            a.path.is_none(),
            "the folder is the show's, not a file to open"
        );
        // No folder: the Pictures folder (or home), always absolute.
        let mut a = args(&["--slideshow"]);
        finish_args(&mut a);
        assert!(a.slideshow.is_some_and(|p| p.is_absolute()));
        let mut a = args(&["/x.jpg"]);
        finish_args(&mut a);
        assert!(a.slideshow.is_none() && a.path.is_some());
    }
}
