//! Zeiss CZI through libCZI, by way of Specular's bridge library, loaded at run time.
//!
//! libCZI is C++ (LGPL-3.0): it is only ever linked dynamically, and only into this sandboxed
//! process. The bridge (`czi/specular_czi.cpp`, Specular's own code) links libCZI's shared
//! library and exports a small C interface; it is built by `build.rs` where libCZI's headers
//! are found and installed next to the host. The host loads it, and with it libCZI, only when
//! started with `--czi`, before the sandbox is applied; documents are opened and read after,
//! through the files the viewer passes (libCZI never opens a path).
//!
//! A few documents stay open between requests, by the viewer's key, so a tile does not
//! re-read the file's directory. Like `heif.rs`, the C interface is declared by hand, symbols
//! are looked up with `libloading`, and each call is one `unsafe` block.

use specular_sandbox::protocol::{CziDisplay, CziPlane, CziPlaneKind, CziRegion, ErrorKind};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::fs::File;
use std::path::PathBuf;

pub type Failure = (ErrorKind, String);

/// The bridge's interface version this host speaks (`ABI` in the bridge).
const ABI: u32 = 1;

/// Documents kept open between requests.
const OPEN_DOCS: usize = 4;
/// Decoded sub-blocks libCZI keeps per document between reads.
const CACHE_BYTES: u64 = 128 << 20;

/// `SpecularCziRegion` in the bridge.
#[repr(C)]
struct Region {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    zoom: f32,
    scene: i32,
    coord: [i32; 10],
    coord_set: u32,
    pixel: u32,
    max_w: u32,
    max_h: u32,
    max_bytes: u64,
    max_subblock_bytes: u64,
    cache_bytes: u64,
}

/// `SpecularCziPlane` in the bridge.
#[repr(C)]
#[derive(Default)]
struct Plane {
    width: u32,
    height: u32,
    pixel: u32,
    kind: u32,
    content_type: [u8; 12],
    has_display: u32,
    black: f32,
    white: f32,
    gamma: f32,
}

type FnAbi = unsafe extern "C" fn() -> u32;
type FnInit = unsafe extern "C" fn(*mut c_char, usize) -> c_int;
type FnRead = unsafe extern "C" fn(*mut c_void, u64, *mut u8, u64, *mut u64) -> c_int;
type FnWrite = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> c_int;
type FnOpen =
    unsafe extern "C" fn(FnRead, *mut c_void, u64, *mut *mut c_void, *mut c_char, usize) -> c_int;
type FnClose = unsafe extern "C" fn(*mut c_void);
type FnInfo =
    unsafe extern "C" fn(*mut c_void, u64, FnWrite, *mut c_void, *mut c_char, usize) -> c_int;
type FnReadRegion = unsafe extern "C" fn(
    *mut c_void,
    *const Region,
    *mut Plane,
    FnWrite,
    *mut c_void,
    *mut c_char,
    usize,
) -> c_int;
type FnAttachment = unsafe extern "C" fn(
    *mut c_void,
    i32,
    u32,
    u64,
    *mut Plane,
    FnWrite,
    *mut c_void,
    *mut c_char,
    usize,
) -> c_int;

/// The bridge's file name.
pub const BRIDGE: &str = if cfg!(target_os = "macos") {
    "libspecular-czi.dylib"
} else if cfg!(windows) {
    "specular-czi.dll"
} else {
    "libspecular-czi.so"
};

/// The libCZI release the bridge was built against ("0.70.0"), when `build.rs` built one.
pub const BUILT_FOR: Option<&str> = option_env!("SPECULAR_CZI_BUILT_FOR");

/// Where the bridge may be: `SPECULAR_CZI_BRIDGE` alone when set, else next to the host, in
/// `../lib/specular` (Unix installs), and in a development build where `build.rs` put it.
fn candidates() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("SPECULAR_CZI_BRIDGE") {
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
        && let Some(p) = option_env!("SPECULAR_CZI_BRIDGE_BUILT")
    {
        v.push(PathBuf::from(p));
    }
    v
}

/// What the user needs, in plain words.
pub fn needed() -> String {
    match BUILT_FOR {
        Some(v) => format!("libCZI {v}"),
        None => "libCZI".into(),
    }
}

/// An open document: the bridge's handle and the file it reads through.
struct Doc {
    key: u64,
    handle: *mut c_void,
    /// Boxed: the bridge keeps its address for its reads.
    _file: Box<File>,
}

/// The loaded bridge (and libCZI). Never unloaded.
pub struct Czi {
    _lib: libloading::Library,
    pub version: String,
    open: FnOpen,
    close: FnClose,
    info: FnInfo,
    read: FnReadRegion,
    attachment: FnAttachment,
    /// Most recently used last.
    docs: Vec<Doc>,
    /// Largest single sub-block decoded (a quarter of the host's address space).
    max_subblock: u64,
}

impl Czi {
    /// Loads the bridge and libCZI, or says why not.
    pub fn load(memory_limit: u64) -> Result<Czi, String> {
        let Some(path) = candidates().into_iter().find(|p| p.is_file()) else {
            return Err(format!(
                "{} is needed; this build of Specular has no libCZI bridge ({BRIDGE})",
                needed()
            ));
        };
        #[allow(unsafe_code)]
        // SAFETY: loading the bridge loads libCZI and runs their static initialisers, which
        // touch none of our state; this happens once, on the main thread, before any other
        // thread exists.
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
                // and used only while `lib` lives (both live in the returned `Czi`).
                let s = unsafe { lib.get::<$ty>(concat!($name, "\0").as_bytes()) };
                *s.map_err(|e| format!("the libCZI bridge lacks {}: {e}", $name))?
            }};
        }
        let abi = sym!("specular_czi_abi", FnAbi);
        #[allow(unsafe_code)]
        // SAFETY: takes nothing, returns a number.
        let found = unsafe { abi() };
        if found != ABI {
            return Err(format!(
                "the libCZI bridge is version {found}, this host needs {ABI}"
            ));
        }
        let init = sym!("specular_czi_init", FnInit);
        let mut buf = [0 as c_char; 256];
        #[allow(unsafe_code)]
        // SAFETY: `buf` is writable for its length; the bridge writes at most that many bytes
        // including the terminating NUL. Called once, single-threaded.
        let st = unsafe { init(buf.as_mut_ptr(), buf.len()) };
        let text = c_text(&buf);
        if st != 0 {
            return Err(format!("libCZI did not start: {text}"));
        }
        Ok(Czi {
            open: sym!("specular_czi_open", FnOpen),
            close: sym!("specular_czi_close", FnClose),
            info: sym!("specular_czi_info", FnInfo),
            read: sym!("specular_czi_read", FnReadRegion),
            attachment: sym!("specular_czi_attachment", FnAttachment),
            _lib: lib,
            version: text,
            docs: Vec::new(),
            max_subblock: (memory_limit / 4).max(64 << 20),
        })
    }

    /// The open document `key`, opening `file` for it when it is not open (else `file` is
    /// closed).
    fn doc(&mut self, key: u64, file: File) -> Result<*mut c_void, Failure> {
        if let Some(i) = self.docs.iter().position(|d| d.key == key) {
            let d = self.docs.remove(i);
            let h = d.handle;
            self.docs.push(d);
            return Ok(h);
        }
        if self.docs.len() >= OPEN_DOCS {
            let old = self.docs.remove(0);
            self.close_doc(old);
        }
        let size = file
            .metadata()
            .map_err(|e| (ErrorKind::Io, e.to_string()))?
            .len();
        let mut file = Box::new(file);
        let ctx: *mut File = &mut *file;
        let mut handle = std::ptr::null_mut();
        let mut err = [0 as c_char; 1024];
        #[allow(unsafe_code)]
        // SAFETY: `read_file` matches `FnRead` and is called with `ctx`, a `File` boxed in the
        // returned `Doc`, which outlives the document (`close_doc` closes the document before
        // dropping the box); `handle` and `err` are valid out pointers for the call.
        let st = unsafe {
            (self.open)(
                read_file,
                ctx.cast(),
                size,
                &mut handle,
                err.as_mut_ptr(),
                err.len(),
            )
        };
        if st != 0 || handle.is_null() {
            return Err(failure(st, c_text(&err)));
        }
        self.docs.push(Doc {
            key,
            handle,
            _file: file,
        });
        Ok(handle)
    }

    fn close_doc(&self, d: Doc) {
        #[allow(unsafe_code)]
        // SAFETY: `d.handle` came from `specular_czi_open` and is closed once; its file is
        // dropped after (with `d`).
        unsafe {
            (self.close)(d.handle)
        };
        drop(d);
    }

    /// A document's facts (`specular_sandbox::czi`), at most `max` bytes.
    pub fn info(&mut self, key: u64, file: File, max_xml: u64) -> Result<Vec<u8>, Failure> {
        let doc = self.doc(key, file)?;
        let mut sink = Collect::new(64 << 20);
        let mut err = [0 as c_char; 1024];
        #[allow(unsafe_code)]
        // SAFETY: `doc` is an open document; `collect` matches `FnWrite` and gets back the
        // `sink` pointer given here, alive for the call; `err` is writable for its length.
        let st = unsafe {
            (self.info)(
                doc,
                max_xml,
                collect,
                (&raw mut sink).cast(),
                err.as_mut_ptr(),
                err.len(),
            )
        };
        sink.finish(st, &err)
    }

    /// One channel of a region of a plane: the plane's description and its rows.
    pub fn read(&mut self, key: u64, file: File, r: &CziRegion) -> Result<CziPlane, Failure> {
        let doc = self.doc(key, file)?;
        let (w, h) = (
            i32::try_from(r.w).map_err(|_| bad("region too wide"))?,
            i32::try_from(r.h).map_err(|_| bad("region too tall"))?,
        );
        let out = |n: u32| ((f64::from(n) * f64::from(r.zoom)).ceil() as u32).saturating_add(2);
        let mut q = Region {
            x: r.x,
            y: r.y,
            w,
            h,
            zoom: r.zoom,
            scene: r.scene.map_or(-1, |s| i32::try_from(s).unwrap_or(i32::MAX)),
            coord: [0; 10],
            coord_set: 0,
            pixel: u32::from(r.pixel),
            max_w: out(r.w),
            max_h: out(r.h),
            max_bytes: r.max_bytes,
            max_subblock_bytes: self.max_subblock,
            cache_bytes: CACHE_BYTES,
        };
        for &(letter, v) in r.coords() {
            // libCZI's dimension indices: 1 Z, 2 C, 3 T, 4 R, 5 S, 6 I, 7 H, 8 V, 9 B.
            let i = match letter {
                b'Z' => 1,
                b'C' => 2,
                b'T' => 3,
                b'R' => 4,
                b'I' => 6,
                b'H' => 7,
                b'V' => 8,
                b'B' => 9,
                _ => return Err(bad("bad plane dimension")),
            };
            q.coord[i] = v;
            q.coord_set |= 1 << i;
        }
        let mut plane = Plane::default();
        let mut sink = Collect::new(r.max_bytes);
        let mut err = [0 as c_char; 1024];
        #[allow(unsafe_code)]
        // SAFETY: `doc` is an open document; `q`, `plane`, `sink` and `err` are live locals the
        // bridge uses only during the call; `collect` matches `FnWrite`.
        let st = unsafe {
            (self.read)(
                doc,
                &q,
                &mut plane,
                collect,
                (&raw mut sink).cast(),
                err.as_mut_ptr(),
                err.len(),
            )
        };
        let data = sink.finish(st, &err)?;
        Ok(reply(plane, data))
    }

    /// Attachment `index`: its bytes, or an embedded image's first channel.
    pub fn attachment(
        &mut self,
        key: u64,
        file: File,
        index: u32,
        max_side: u32,
        max_bytes: u64,
    ) -> Result<CziPlane, Failure> {
        let doc = self.doc(key, file)?;
        let index = i32::try_from(index).map_err(|_| bad("no such attachment"))?;
        let mut plane = Plane::default();
        let mut sink = Collect::new(max_bytes);
        let mut err = [0 as c_char; 1024];
        #[allow(unsafe_code)]
        // SAFETY: as in `read`.
        let st = unsafe {
            (self.attachment)(
                doc,
                index,
                max_side.max(1),
                max_bytes,
                &mut plane,
                collect,
                (&raw mut sink).cast(),
                err.as_mut_ptr(),
                err.len(),
            )
        };
        let data = sink.finish(st, &err)?;
        Ok(reply(plane, data))
    }
}

impl Drop for Czi {
    fn drop(&mut self) {
        for d in std::mem::take(&mut self.docs) {
            self.close_doc(d);
        }
    }
}

fn bad(m: &str) -> Failure {
    (ErrorKind::Protocol, m.into())
}

fn failure(st: c_int, message: String) -> Failure {
    let kind = match st {
        1 => ErrorKind::Malformed,
        2 => ErrorKind::Unsupported,
        3 => ErrorKind::TooLarge,
        6 => ErrorKind::Io,
        _ => ErrorKind::Internal,
    };
    (kind, message)
}

/// The plane's description from the bridge, with its bytes (the payload is filled in by the
/// caller: inline here, moved to shared memory by `serve`).
fn reply(p: Plane, data: Vec<u8>) -> CziPlane {
    let ct_len = p
        .content_type
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(8)
        .min(8);
    let content_type: String = p.content_type[..ct_len]
        .iter()
        .filter(|b| b.is_ascii_graphic())
        .map(|&b| char::from(b))
        .collect();
    let finite = |v: f32| v.is_finite().then_some(v);
    let display = (p.has_display != 0)
        .then(|| {
            Some(CziDisplay {
                black: finite(p.black)?,
                white: finite(p.white)?,
                gamma: finite(p.gamma).filter(|g| *g > 0.0),
            })
        })
        .flatten();
    CziPlane {
        kind: if p.kind == 1 {
            CziPlaneKind::Pixels
        } else {
            CziPlaneKind::Encoded
        },
        width: p.width,
        height: p.height,
        pixel: u8::try_from(p.pixel).unwrap_or(255),
        content_type,
        display,
        payload: specular_sandbox::protocol::Payload::Inline(data),
    }
}

/// Where the bridge's chunks go.
struct Collect {
    out: Vec<u8>,
    max: u64,
    failed: bool,
}

impl Collect {
    fn new(max: u64) -> Collect {
        Collect {
            out: Vec::new(),
            max,
            failed: false,
        }
    }

    fn finish(self, st: c_int, err: &[c_char]) -> Result<Vec<u8>, Failure> {
        if self.failed {
            return Err((ErrorKind::TooLarge, "more data than the limit".into()));
        }
        if st != 0 {
            return Err(failure(st, c_text(err)));
        }
        Ok(self.out)
    }
}

/// The bridge's write callback: appends a chunk, or stops it (non-zero) past the limit or
/// when memory runs out.
extern "C" fn collect(ctx: *mut c_void, data: *const u8, len: usize) -> c_int {
    if ctx.is_null() || (data.is_null() && len > 0) {
        return 1;
    }
    #[allow(unsafe_code)]
    // SAFETY: `ctx` is the `&mut Collect` passed with the call that is running, alive and not
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

/// The bridge's read callback: `size` bytes of the document's file at `offset`.
extern "C" fn read_file(
    ctx: *mut c_void,
    offset: u64,
    buf: *mut u8,
    size: u64,
    got: *mut u64,
) -> c_int {
    if ctx.is_null() || got.is_null() || (buf.is_null() && size > 0) {
        return 1;
    }
    let Ok(n) = usize::try_from(size) else {
        return 1;
    };
    #[allow(unsafe_code)]
    // SAFETY: `ctx` is the boxed `File` of an open `Doc` (see `Czi::doc`), only read through
    // here, on the one thread the host serves requests on.
    let file = unsafe { &*ctx.cast::<File>() };
    #[allow(unsafe_code)]
    // SAFETY: libCZI passes a writable buffer of at least `size` bytes, valid for this call.
    let out = unsafe { std::slice::from_raw_parts_mut(buf, n) };
    let mut done = 0usize;
    while done < n {
        match read_at(file, &mut out[done..], offset + done as u64) {
            Ok(0) => break,
            Ok(k) => done += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return 1,
        }
    }
    #[allow(unsafe_code)]
    // SAFETY: `got` is a valid out pointer for this call.
    unsafe {
        *got = done as u64
    };
    0
}

#[cfg(unix)]
fn read_at(f: &File, buf: &mut [u8], at: u64) -> std::io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(f, buf, at)
}

#[cfg(windows)]
fn read_at(f: &File, buf: &mut [u8], at: u64) -> std::io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(f, buf, at)
}

/// A NUL-terminated message in `buf` (or all of it), lossily as UTF-8.
fn c_text(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf.iter().map(|&c| c as u8).collect();
    match CStr::from_bytes_until_nul(&bytes) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => String::from_utf8_lossy(&bytes).into_owned(),
    }
}
