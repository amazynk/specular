//! OpenCASCADE (OCCT), through Specular's bridge library, loaded at run time.
//!
//! OCCT is C++ (LGPL-2.1 with the OCCT exception): it is only ever linked dynamically, and
//! only into this sandboxed process. The bridge (`occt/specular_occt.cpp`, Specular's own code)
//! links OCCT's shared libraries and exports a small C interface; it is built by `build.rs`
//! where OCCT's headers are found and installed next to the host. The host loads it, and
//! with it OCCT, only when started with `--occt`, before the sandbox is applied; reading and
//! tessellating happen after.
//!
//! Like `heif.rs`, the C interface is declared by hand, symbols are looked up with
//! `libloading`, and each call is one `unsafe` block.

use specular_sandbox::protocol::{CadFormat, CadLimits, ErrorKind};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::path::PathBuf;

pub type Failure = (ErrorKind, String);

/// The bridge's interface version this host speaks (`ABI` in the bridge).
const ABI: u32 = 2;

/// `SpecularOcctParams` in the bridge.
#[repr(C)]
struct Params {
    relative_deflection: f64,
    angular_deflection: f64,
    max_triangles: u64,
    max_bytes: u64,
}

type FnAbi = unsafe extern "C" fn() -> u32;
type FnInit = unsafe extern "C" fn(c_int, *mut c_char, usize) -> c_int;
type FnWrite = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> c_int;
type FnRead = unsafe extern "C" fn(
    *const u8,
    usize,
    c_int,
    *const Params,
    FnWrite,
    *mut c_void,
    *mut c_char,
    usize,
) -> c_int;

/// The bridge's file name.
pub const BRIDGE: &str = if cfg!(target_os = "macos") {
    "libspecular-occt.dylib"
} else if cfg!(windows) {
    "specular-occt.dll"
} else {
    "libspecular-occt.so"
};

/// The OCCT release line the bridge was built against ("7.9"), when `build.rs` built one.
pub const BUILT_FOR: Option<&str> = option_env!("SPECULAR_OCCT_BUILT_FOR");

/// The loaded bridge (and OCCT). Never unloaded.
pub struct Occt {
    _lib: libloading::Library,
    pub version: String,
    read: FnRead,
}

/// Where the bridge may be: `SPECULAR_OCCT_BRIDGE` alone when set, else next to the host,
/// in `../lib/specular` (Unix installs), and in a development build where `build.rs` put it.
fn candidates() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("SPECULAR_OCCT_BRIDGE") {
        return vec![PathBuf::from(p)];
    }
    let mut v = Vec::new();
    if let Ok(me) = std::env::current_exe()
        && let Some(dir) = me.parent()
    {
        v.push(dir.join(BRIDGE));
        v.push(dir.join("../lib/specular").join(BRIDGE));
        // `cargo test` binaries run from target/<profile>/deps.
        v.push(dir.join("..").join(BRIDGE));
    }
    if cfg!(debug_assertions)
        && let Some(p) = option_env!("SPECULAR_OCCT_BRIDGE_BUILT")
    {
        v.push(PathBuf::from(p));
    }
    v
}

/// What the user needs, in plain words.
pub fn needed() -> String {
    match BUILT_FOR {
        Some(v) => format!("OpenCASCADE (OCCT) {v}"),
        None => "OpenCASCADE (OCCT)".into(),
    }
}

impl Occt {
    /// Loads the bridge and OCCT and initialises OCCT's readers, or says why not.
    pub fn load(threads: usize) -> Result<Occt, String> {
        let Some(path) = candidates().into_iter().find(|p| p.is_file()) else {
            return Err(format!(
                "{} is needed; this build of Specular has no OpenCASCADE bridge ({BRIDGE})",
                needed()
            ));
        };
        #[allow(unsafe_code)]
        // SAFETY: loading the bridge loads OCCT's libraries and runs their static
        // initialisers, which touch none of our state; this happens once, on the main
        // thread, before any other thread exists.
        let lib = unsafe { libloading::Library::new(&path) };
        let lib = lib.map_err(|e| {
            format!(
                "{} is needed and was not found on this computer ({e})",
                needed()
            )
        })?;
        macro_rules! sym {
            ($name:literal, $ty:ty) => {{
                #[allow(unsafe_code)]
                // SAFETY: `$ty` is the C signature of this symbol in the bridge of ABI
                // version `ABI` (checked before any other call); the pointer is copied out
                // and used only while `lib` lives (both live in the returned `Occt`).
                let s = unsafe { lib.get::<$ty>(concat!($name, "\0").as_bytes()) };
                *s.map_err(|e| format!("the OpenCASCADE bridge lacks {}: {e}", $name))?
            }};
        }
        let abi = sym!("specular_occt_abi", FnAbi);
        #[allow(unsafe_code)]
        // SAFETY: takes nothing, returns a number.
        let found = unsafe { abi() };
        if found != ABI {
            return Err(format!(
                "the OpenCASCADE bridge is version {found}, this host needs {ABI}"
            ));
        }
        let init = sym!("specular_occt_init", FnInit);
        let read = sym!("specular_occt_read", FnRead);
        let mut buf = [0 as c_char; 256];
        let threads = c_int::try_from(threads.clamp(1, 64)).unwrap_or(1);
        #[allow(unsafe_code)]
        // SAFETY: `buf` is writable for its length; the bridge writes at most that many bytes
        // including the terminating NUL. Called once, single-threaded.
        let st = unsafe { init(threads, buf.as_mut_ptr(), buf.len()) };
        let text = c_text(&buf);
        if st != 0 {
            return Err(format!("OpenCASCADE did not start: {text}"));
        }
        Ok(Occt {
            _lib: lib,
            version: text,
            read,
        })
    }

    /// Reads and tessellates a STEP or IGES file; the model's bytes
    /// (`specular_sandbox::cad`).
    pub fn read(
        &self,
        data: &[u8],
        format: CadFormat,
        limits: CadLimits,
    ) -> Result<Vec<u8>, Failure> {
        let params = Params {
            relative_deflection: limits.relative_deflection.clamp(1e-6, 0.5),
            angular_deflection: limits.angular_deflection.clamp(0.05, 1.5),
            max_triangles: limits.max_triangles,
            max_bytes: limits.max_bytes,
        };
        let mut sink = Collect {
            out: Vec::new(),
            max: limits.max_bytes,
            failed: false,
        };
        let mut err = [0 as c_char; 1024];
        #[allow(unsafe_code)]
        // SAFETY: `data` is valid for its length during the call; `params`, `sink` and `err`
        // are live locals the bridge uses only during the call (`err` up to its length);
        // `collect` matches `FnWrite` and gets back the `sink` pointer given here.
        let st = unsafe {
            (self.read)(
                data.as_ptr(),
                data.len(),
                format as c_int,
                &params,
                collect,
                (&raw mut sink).cast(),
                err.as_mut_ptr(),
                err.len(),
            )
        };
        let message = c_text(&err);
        if sink.failed {
            return Err((ErrorKind::TooLarge, "the model is too large to send".into()));
        }
        match st {
            0 => Ok(sink.out),
            1 => Err((ErrorKind::Malformed, message)),
            2 => Err((ErrorKind::Unsupported, message)),
            3 => Err((ErrorKind::TooLarge, message)),
            _ => Err((ErrorKind::Internal, message)),
        }
    }
}

/// Where the bridge's chunks go.
struct Collect {
    out: Vec<u8>,
    max: u64,
    failed: bool,
}

/// The bridge's write callback: appends a chunk, or stops it (non-zero) past the limit or
/// when memory runs out.
extern "C" fn collect(ctx: *mut c_void, data: *const u8, len: usize) -> c_int {
    if ctx.is_null() || (data.is_null() && len > 0) {
        return 1;
    }
    #[allow(unsafe_code)]
    // SAFETY: `ctx` is the `&mut Collect` passed to `specular_occt_read`, alive and not
    // otherwise borrowed while the bridge runs (the caller waits for it).
    let sink = unsafe { &mut *ctx.cast::<Collect>() };
    if len == 0 {
        return 0;
    }
    if sink.out.len() as u64 + len as u64 > sink.max || sink.out.try_reserve(len).is_err() {
        sink.failed = true;
        return 1;
    }
    #[allow(unsafe_code)]
    // SAFETY: the bridge passes a buffer of `len` readable bytes that stays valid and
    // unmodified until this call returns; the slice does not outlive it.
    let chunk = unsafe { std::slice::from_raw_parts(data, len) };
    sink.out.extend_from_slice(chunk);
    0
}

/// A NUL-terminated message in `buf` (or all of it), lossily as UTF-8.
fn c_text(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf.iter().map(|&c| c as u8).collect();
    match CStr::from_bytes_until_nul(&bytes) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => String::from_utf8_lossy(&bytes).into_owned(),
    }
}
