//! Client side of Specular's sandboxed decode host (`specular-decode-host`, DESIGN.md §13).
//!
//! C/C++ parsers (libheif, UnRAR, libCZI, SQLite) never run in the viewer: they run in a separate, confined
//! helper process. This crate holds the wire protocol (shared with the host), the transports
//! and the viewer's pool of host processes. The same protocol runs on every platform; only
//! the transport and the confinement differ.
//!
//! # Per platform
//!
//! | | Linux | macOS | Windows |
//! |---|---|---|---|
//! | Transport | Unix socket on the host's stdin, `SCM_RIGHTS` ([`transport`]) | same | anonymous pipes on the host's stdin/stdout ([`pipe`]) |
//! | Payloads | sealed `memfd` above 64 KiB, else inline | inline | inline |
//! | Archive input ([`RarSource`]) | the archive file, opened read-only by the viewer, as a descriptor (`SCM_RIGHTS`); copies in a sealed `memfd` | same; copies in an unlinked temp file | the file's handle, duplicated read-only into the host (`DuplicateHandle`); copies in a delete-on-close temp file |
//! | Further RAR volumes | asked for by synthetic name (`NeedVolume`), opened by the viewer next to the first volume, sent like the first | same | same |
//! | Confinement | no_new_privs + Landlock (all rights handled, none granted) + seccomp allowlist without `openat` | Seatbelt `sandbox_init` profile: deny default, no file access at all | AppContainer without capabilities, job object, mitigation policies |
//! | Memory limit | `RLIMIT_AS` | none (macOS ignores `RLIMIT_AS`) | job `ProcessMemoryLimit` |
//! | Dies with the viewer | `PR_SET_PDEATHSIG` + socket EOF | parent watchdog thread + socket EOF | job kill-on-close + pipe EOF |
//! | Timeout | socket `SO_RCVTIMEO`, then kill | same | [`pipe::Watchdog`] terminates the job |
//!
//! # Test plan
//!
//! Automated (`cargo test -p specular-sandbox -p specular-decode-host`): the protocol and both
//! transports run on Linux ([`pipe`] over `std::io::pipe`, so the Windows framing and the
//! watchdog are exercised there too). `specular-decode-host/tests/host.rs` starts real hosts:
//! hello, echo, crash and restart, kill and restart, timeout, memory limit, the sandbox
//! probes (file read/create, directory listing, UDP, TCP, Unix socket, running a program,
//! signalling the viewer; each must be denied under a full sandbox and allowed with
//! `--no-sandbox`), HEIC decoding (when libheif is installed) and RAR: listing and reading
//! from a passed file and from a private copy, multi-volume (new numbering, renamed `.cbr`
//! volumes, more volumes than the host may hold descriptors, solid across volumes; these need
//! the `rar` program and skip without it), a missing volume, truncated archives, the host
//! killed while it waits for a volume, and (`--ignored`) an archive over 4 GiB.
//! `tests/braw.rs` (Linux) starts hosts with `--braw` pointing at a mock of the Blackmagic
//! RAW SDK (`examples/braw_mock.rs`, the SDK 6.0 ABI): loading and the 6.0 check, frames
//! decoded through the host's file object inside the full sandbox (the mock's attempt to open
//! a file by path must fail; a control run without the sandbox succeeds), scales, limits, a
//! changed SDK path restarting the host. The real SDK is not on the build machine. The
//! platform-specific expectations are `#[cfg]`-gated, so the same file is the test on every OS.
//!
//! On a Mac (not reachable from the Linux CI, which only runs `cargo check`/`clippy` for
//! `aarch64-apple-darwin`):
//! 1. `cargo test -p specular-sandbox -p specular-decode-host` with `brew install libheif`:
//!    `hello_reports_capabilities` must report `sandbox: Full` ("seatbelt"), and
//!    `probes_are_denied` every probe denied (the control run with `--no-sandbox` allows
//!    them). `heic::*` and `rar::*` must pass (`brew install rar` for the multi-volume
//!    ones): UnRAR reads only descriptors the viewer passed, the profile allows no path.
//! 2. `sandbox-exec`-free check of the profile: `log stream --predicate 'sender == "Sandbox"'`
//!    while opening a HEIC and a CBR in the viewer shows only the denials the probes caused.
//! 3. Kill the viewer (`kill -9`) during a long decode (`TestOp::Hang`): the host exits
//!    within a second (parent watchdog).
//! 4. Bundle: `packaging/macos/build-dmg.sh` puts the host in `Contents/MacOS`; check that the
//!    signed, notarised bundle still starts it (hardened runtime; the host needs no
//!    entitlements, it is not App-Sandboxed).
//!
//! On Windows (also only `cargo check`/`clippy` from Linux, for `x86_64-pc-windows-msvc`):
//! 1. `cargo test -p specular-sandbox -p specular-decode-host`, from any folder: Windows opens
//!    the host's file as the process that starts it, and a statically linked host loads only
//!    system DLLs, so the folder needs no grant to ALL APPLICATION PACKAGES (checked 2026-10-15
//!    from `target\` on a volume without that grant; untested: a host loading `heif.dll` from
//!    its own folder, which the host itself opens, may need it). With `heif.dll` on `PATH` or
//!    in `SPECULAR_LIBHEIF`: `sandbox: Full` ("appcontainer, job, ..."), probes denied, the
//!    crash test reports `0xc0000409 (fail-fast)`, the memory test is refused by the job.
//! 2. Process Explorer on a running host: integrity "AppContainer", job with "Kill on job
//!    close", active process limit 1, the memory limit; Mitigation Policies column lists
//!    "Signatures restricted (Microsoft only)", "Dynamic code prohibited", "Win32k system
//!    calls disabled" (the latter only if nothing loaded user32 first, which UnRAR does: with
//!    RAR built in it is missing).
//! 3. Close the viewer from Task Manager during a long decode: the host disappears at once
//!    (kill on job close).
//! 4. Install the MSI and open a HEIC: the host starts without any ACL change (the install
//!    folder is readable by ALL APPLICATION PACKAGES).
//! 5. `rar::*` (with WinRAR's `rar.exe` on `PATH` for the multi-volume tests), then a CBR and
//!    a multi-volume RAR from the user's Documents in the viewer: the host reads them only
//!    through duplicated handles (Process Explorer shows them as `File` handles with read
//!    access); the AppContainer still has no file-system access.

#![cfg_attr(windows, feature(windows_process_extensions_raw_attribute))]

pub mod cad;
pub mod client;
pub mod czi;
pub mod pipe;
pub mod protocol;
pub mod sql;
#[cfg(unix)]
pub mod transport;
#[cfg(not(unix))]
#[path = "transport_pipe.rs"]
pub mod transport;
#[cfg(windows)]
pub mod win;

pub use cad::{CadDatumRef, CadMesh, CadModel, CadNode, CadPmi, CadView, PmiKind};
pub use cad::{FORM_VARIANCES, GRADES, TOLERANCE_MODIFIERS, TOLERANCE_TYPES};
pub use client::{
    ClipSource, Config, CziSource, DecodedImage, Error, HOST_EXE, Host, RarSource, Result,
    SharedInput, SqlSource, VendorLibs, VolumeProvider, resolve_volume,
};
pub use czi::CziInfo;
pub use protocol::{
    CadFormat, CadLimits, ClipInfo, CziDisplay, CziPlane, CziPlaneKind, CziRegion, ErrorKind,
    HostInfo, ImageLimits, ListEntry, Listing, SandboxLevel, Secret,
};
pub use sql::{SqlLimits, SqlRows, SqlValue};
