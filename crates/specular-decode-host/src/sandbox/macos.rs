//! macOS: Seatbelt, applied by the host itself once libheif is loaded.
//!
//! Why not the App Sandbox: it is configured by entitlements of a separately signed
//! executable, gives every sandboxed program a writable container in the user's Library, and
//! can only be *inherited* from an App-Sandboxed parent; the viewer is not one. What browsers
//! do for their content processes is the same as here — Chromium compiles and applies its
//! own SBPL profiles (`sandbox_compile_string`/`sandbox_apply`), Firefox calls
//! `sandbox_init_with_parameters` — i.e. the Seatbelt SPI that `sandbox_init(3)` fronts. It is
//! marked deprecated in the headers but is what the system's own daemons and those browsers
//! still use, and it is the only way to sandbox a helper of a non-App-Store application.
//!
//! The profile ([`PROFILE`]) denies everything by default: no files at all, no network, no
//! Mach/XPC services, no process creation, no signals except to itself. The socket to the
//! viewer and the passed descriptors are already open, and reading and writing an open
//! descriptor is not checked; UnRAR reads archives through such descriptors (`pread`, via
//! Specular's handle-I/O patch), so it needs no path either.
//!
//! Also here: a watchdog thread that exits when the viewer does (macOS has no parent-death
//! signal; the socket's end-of-file only reaches a host that is waiting for a request).
//! `RLIMIT_AS` is accepted but not enforced by macOS, so the memory limit is not either.

use super::Status;
use specular_sandbox::protocol::SandboxLevel;
use std::ffi::{CStr, CString, c_char, c_int};

/// Seatbelt profile: nothing allowed but computing, memory, threads and the already open
/// descriptors.
pub const PROFILE: &str = r#"(version 1)
(deny default)
(allow process-info-pidinfo (target self))
(allow process-info-setcontrol (target self))
(allow signal (target self))
(allow sysctl-read)
"#;

#[allow(unsafe_code)]
// SAFETY (declarations): signatures from <sandbox.h>; libsystem_sandbox is part of libSystem,
// which every macOS program links.
unsafe extern "C" {
    fn sandbox_init(profile: *const c_char, flags: u64, errorbuf: *mut *mut c_char) -> c_int;
    fn sandbox_free_error(errorbuf: *mut c_char);
}

/// Applies `profile` (SBPL source) to this process, irreversibly.
fn init(profile: &str) -> Result<(), String> {
    let src = CString::new(profile).map_err(|e| e.to_string())?;
    let mut err: *mut c_char = std::ptr::null_mut();
    #[allow(unsafe_code)]
    // SAFETY: `src` is a NUL-terminated string that outlives the call; flags 0 means "profile
    // source text"; `err` is a valid out pointer.
    let r = unsafe { sandbox_init(src.as_ptr(), 0, &raw mut err) };
    if r == 0 {
        return Ok(());
    }
    if err.is_null() {
        return Err(format!("sandbox_init failed ({r})"));
    }
    #[allow(unsafe_code)]
    // SAFETY: on failure `err` points to a NUL-terminated message owned by libsandbox, which
    // stays valid until sandbox_free_error, called once below after copying it.
    let msg = unsafe { CStr::from_ptr(err) }
        .to_string_lossy()
        .into_owned();
    #[allow(unsafe_code)]
    // SAFETY: `err` came from sandbox_init and is freed exactly once.
    unsafe {
        sandbox_free_error(err)
    };
    Err(msg)
}

/// Exits when the viewer (our parent) is gone: we are then re-parented to launchd.
fn watch_parent() -> bool {
    let Some(parent) = rustix::process::getppid() else {
        return false;
    };
    specular_thread::builder("wd-parent")
        .spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if rustix::process::getppid() != Some(parent) {
                    std::process::exit(1);
                }
            }
        })
        .is_ok()
}

pub fn apply() -> Status {
    let watched = watch_parent();
    match init(PROFILE) {
        Ok(()) => Status {
            level: SandboxLevel::Full,
            detail: if watched {
                "seatbelt".into()
            } else {
                "seatbelt; no parent watchdog".into()
            },
        },
        Err(e) => Status {
            level: SandboxLevel::None,
            detail: format!("seatbelt: {e}"),
        },
    }
}
