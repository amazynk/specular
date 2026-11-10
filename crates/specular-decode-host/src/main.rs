//! `specular-decode-host`: runs C/C++ file parsers (libheif, UnRAR, OpenCASCADE, libCZI, and
//! vendor SDKs the user installed: the Blackmagic RAW SDK) in a confined process
//! (DESIGN.md §13). Started by the viewer (`specular-sandbox`), one request at a time over the
//! socket on its stdin (Unix) or the pipes on its stdin and stdout (Windows).
//!
//! Start-up order matters: parent-death signal, resource limits, load libheif (it loads its
//! plugins) and, with `--occt`, OpenCASCADE, with `--czi`, libCZI, with `--braw`, the
//! Blackmagic RAW SDK, then the OS sandbox, then the hello. Nothing is loaded after the
//! sandbox.
//!
//! Options: `--ipc` (required), `--memory-limit=BYTES`, `--occt` (load OpenCASCADE for STEP and
//! IGES), `--czi` (load libCZI for Zeiss CZI files), `--braw=PATH` (the Blackmagic RAW SDK the
//! user installed: its library or folder), `--test-hooks` (enables self-test requests),
//! `--no-sandbox` (with `--test-hooks` only: checks that the probes work at all).

#[cfg(any(unix, windows))]
mod braw;
#[cfg(any(unix, windows))]
mod czi;
#[cfg(any(unix, windows))]
mod heif;
#[cfg(any(unix, windows))]
mod occt;
#[cfg(all(any(unix, windows), feature = "rar"))]
mod rar;
mod sandbox;
#[cfg(any(unix, windows))]
mod serve;
#[cfg(any(unix, windows))]
mod sqlite;
#[cfg(any(unix, windows))]
mod testops;

/// The connection to the viewer.
#[cfg(unix)]
type Conn = specular_sandbox::transport::Conn;
#[cfg(windows)]
type Conn = specular_sandbox::pipe::Conn<std::fs::File, std::fs::File>;

fn usage() -> ! {
    eprintln!(
        "specular-decode-host {}: Specular's sandboxed decoder process; started by the viewer, \
         not by hand.",
        env!("CARGO_PKG_VERSION")
    );
    std::process::exit(2);
}

struct Args {
    memory_limit: u64,
    occt: bool,
    czi: bool,
    test_hooks: bool,
    no_sandbox: bool,
    braw: Option<std::path::PathBuf>,
}

fn args() -> Args {
    let mut a = Args {
        memory_limit: 4 << 30,
        occt: false,
        czi: false,
        test_hooks: false,
        no_sandbox: false,
        braw: None,
    };
    let mut ipc = false;
    for arg in std::env::args_os().skip(1) {
        if arg.as_encoded_bytes().starts_with(b"--braw=") {
            a.braw = Some(braw_path(&arg));
            continue;
        }
        let Some(arg) = arg.to_str().map(str::to_owned) else {
            usage()
        };
        if arg == "--ipc" {
            ipc = true;
        } else if arg == "--occt" {
            a.occt = true;
        } else if arg == "--czi" {
            a.czi = true;
        } else if arg == "--test-hooks" {
            a.test_hooks = true;
        } else if arg == "--no-sandbox" {
            a.no_sandbox = true;
        } else if let Some(v) = arg.strip_prefix("--memory-limit=") {
            match v.parse::<u64>() {
                Ok(n) => a.memory_limit = n.max(64 << 20),
                Err(_) => usage(),
            }
        } else {
            usage();
        }
    }
    if !ipc || (a.no_sandbox && !a.test_hooks) {
        usage();
    }
    a
}

/// The path after `--braw=` in an argument that is not UTF-8.
fn braw_path(arg: &std::ffi::OsStr) -> std::path::PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::OsStr::from_bytes(&arg.as_bytes()["--braw=".len()..]).into()
    }
    #[cfg(not(unix))]
    {
        // The prefix is ASCII, so it is the first characters of the lossy text too.
        arg.to_string_lossy()["--braw=".len()..].to_owned().into()
    }
}

/// Takes over the connection the viewer set up on stdin (and stdout on Windows).
#[cfg(unix)]
fn connect() -> std::io::Result<Conn> {
    use std::os::fd::AsFd;
    let fd = std::io::stdin().as_fd().try_clone_to_owned()?;
    Ok(Conn::new(std::os::unix::net::UnixStream::from(fd)))
}

#[cfg(windows)]
fn connect() -> std::io::Result<Conn> {
    use std::os::windows::io::AsHandle;
    let r = std::io::stdin().as_handle().try_clone_to_owned()?;
    let w = std::io::stdout().as_handle().try_clone_to_owned()?;
    Ok(Conn::new(std::fs::File::from(r), std::fs::File::from(w)))
}

#[cfg(any(unix, windows))]
fn main() {
    use specular_sandbox::protocol::{HostInfo, SandboxLevel, VERSION};

    let a = args();
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        // Die with the viewer.
        let _ =
            rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::KILL));
    }
    let Ok(conn) = connect() else { usage() };
    // UnRAR opens no file by path from here on: only what the viewer passes.
    #[cfg(feature = "rar")]
    rar::install_hook();
    let limit_errors = sandbox::limit_resources(a.memory_limit);
    let heif = heif::Heif::load();
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(2);
    // OpenCASCADE only in hosts started for CAD files (it is large and slow to load).
    let occt = if a.occt {
        occt::Occt::load(threads)
    } else {
        Err("this decode host was not started for CAD files".into())
    };
    // The SDK and its plugins, before the sandbox (nothing is loaded afterwards).
    let braw = a.braw.as_deref().map(|p| {
        let all = std::thread::available_parallelism().map_or(2, |n| n.get());
        braw::Braw::load(p, all)
    });
    // libCZI only in hosts started for CZI files.
    let czi = if a.czi {
        czi::Czi::load(a.memory_limit)
    } else {
        Err("this decode host was not started for Zeiss CZI files".into())
    };
    // SQLite's VFS (it reads only the files the viewer passes) and heap limit.
    let sqlite_error = sqlite::init(a.memory_limit).err();
    let mut status = if a.no_sandbox {
        sandbox::Status {
            level: SandboxLevel::None,
            detail: "disabled (--no-sandbox)".into(),
        }
    } else {
        sandbox::apply()
    };
    if let Some(e) = sqlite_error {
        status.detail = format!("{}; {e}", status.detail);
    }
    if !limit_errors.is_empty() {
        status.detail = format!("{}; limits: {}", status.detail, limit_errors.join(", "));
    }
    let info = HostInfo {
        version: VERSION,
        sandbox: status.level,
        sandbox_detail: status.detail,
        heif: heif.as_ref().ok().map(|h| h.version.clone()),
        heif_error: heif.as_ref().err().cloned().unwrap_or_default(),
        // UnRAR reads the files the viewer passes (descriptors or duplicated handles).
        rar: cfg!(feature = "rar"),
        occt: occt.as_ref().ok().map(|o| o.version.clone()),
        occt_error: occt.as_ref().err().cloned().unwrap_or_default(),
        czi: czi.as_ref().ok().map(|c| c.version.clone()),
        czi_error: czi.as_ref().err().cloned().unwrap_or_default(),
        test_hooks: a.test_hooks,
        braw: match &braw {
            Some(Ok(b)) => Some(b.version.clone()),
            _ => None,
        },
        braw_error: match &braw {
            Some(Err(e)) => e.clone(),
            _ => String::new(),
        },
    };
    let host = serve::Host {
        conn: std::sync::Arc::new(std::sync::Mutex::new(conn)),
        heif,
        occt,
        braw,
        czi,
        sqlite: sqlite::Sqlite::default(),
        threads,
        info,
    };
    std::process::exit(host.run());
}

#[cfg(not(any(unix, windows)))]
fn main() {
    let _ = args();
    let _ = sandbox::limit_resources(0);
    let s = sandbox::apply();
    eprintln!(
        "specular-decode-host: not supported on this platform (sandbox {:?}: {})",
        s.level, s.detail
    );
    std::process::exit(2);
}
