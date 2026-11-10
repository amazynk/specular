//! libheif, loaded at run time (LGPL: dynamic linking only, and the library is optional).
//!
//! This is the host's only `unsafe` module: the C ABI is declared by hand (no headers or
//! bindgen at build time), symbols are looked up with `libloading`, and every call is one
//! `unsafe` block. The library is loaded and initialised (it loads its codec plugins then)
//! before the sandbox is applied; decoding happens after.

use specular_sandbox::protocol::{ErrorKind, ImageLimits};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

/// `struct heif_error`, returned by value.
#[repr(C)]
#[derive(Clone, Copy)]
struct HeifError {
    code: c_int,
    subcode: c_int,
    message: *const c_char,
}

/// Opaque libheif types.
#[repr(C)]
struct Context {
    _p: [u8; 0],
}
#[repr(C)]
struct Handle {
    _p: [u8; 0],
}
#[repr(C)]
struct Image {
    _p: [u8; 0],
}

const COLORSPACE_RGB: c_int = 1;
const CHROMA_INTERLEAVED_RGBA: c_int = 11;
const CHANNEL_INTERLEAVED: c_int = 10;

// heif_error_code
const ERR_OK: c_int = 0;
const ERR_INVALID_INPUT: c_int = 2;
const ERR_UNSUPPORTED_FILETYPE: c_int = 3;
const ERR_UNSUPPORTED_FEATURE: c_int = 4;
const ERR_MEMORY: c_int = 6;
const ERR_DECODER_PLUGIN: c_int = 7;

type FnInit = unsafe extern "C" fn(*mut c_void) -> HeifError;
type FnVersion = unsafe extern "C" fn() -> *const c_char;
type FnCtxAlloc = unsafe extern "C" fn() -> *mut Context;
type FnCtxFree = unsafe extern "C" fn(*mut Context);
type FnCtxSetInt = unsafe extern "C" fn(*mut Context, c_int);
type FnCtxRead =
    unsafe extern "C" fn(*mut Context, *const c_void, usize, *const c_void) -> HeifError;
type FnCtxPrimary = unsafe extern "C" fn(*mut Context, *mut *mut Handle) -> HeifError;
type FnHandleInt = unsafe extern "C" fn(*const Handle) -> c_int;
type FnHandleRelease = unsafe extern "C" fn(*const Handle);
type FnProfileSize = unsafe extern "C" fn(*const Handle) -> usize;
type FnProfile = unsafe extern "C" fn(*const Handle, *mut c_void) -> HeifError;
type FnDecode =
    unsafe extern "C" fn(*const Handle, *mut *mut Image, c_int, c_int, *const c_void) -> HeifError;
type FnImageInt = unsafe extern "C" fn(*const Image) -> c_int;
type FnPlane = unsafe extern "C" fn(*const Image, c_int, *mut c_int) -> *const u8;
type FnImageRelease = unsafe extern "C" fn(*const Image);

/// The loaded library. Never unloaded (it lives in a process-wide static).
pub struct Heif {
    _lib: libloading::Library,
    pub version: String,
    ctx_alloc: FnCtxAlloc,
    ctx_free: FnCtxFree,
    ctx_max_threads: FnCtxSetInt,
    ctx_max_size: FnCtxSetInt,
    ctx_read: FnCtxRead,
    ctx_primary: FnCtxPrimary,
    handle_width: FnHandleInt,
    handle_height: FnHandleInt,
    handle_alpha: FnHandleInt,
    handle_luma_bits: FnHandleInt,
    handle_release: FnHandleRelease,
    profile_size: FnProfileSize,
    profile: FnProfile,
    decode: FnDecode,
    image_width: FnImageInt,
    image_height: FnImageInt,
    plane: FnPlane,
    image_release: FnImageRelease,
}

/// Library names tried in order (unless `SPECULAR_LIBHEIF` names one).
const NAMES: &[&str] = if cfg!(target_os = "macos") {
    &[
        "libheif.1.dylib",
        "/opt/homebrew/lib/libheif.1.dylib",
        "/usr/local/lib/libheif.1.dylib",
    ]
} else if cfg!(windows) {
    &["heif.dll", "libheif.dll"]
} else {
    &["libheif.so.1", "libheif.so"]
};

/// One decoded RGBA8 image (rows handed to a sink while copying out of libheif).
pub struct Meta {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub bit_depth: u8,
    pub icc: Vec<u8>,
}

pub type Failure = (ErrorKind, String);

/// Receives a decoded image: its description, then its rows (`width * 4` bytes each, top to
/// bottom).
pub trait Rows {
    fn start(&mut self, meta: &Meta) -> Result<(), Failure>;
    fn row(&mut self, row: &[u8]) -> Result<(), Failure>;
}

impl Heif {
    /// Loads and initialises libheif, or says why not.
    pub fn load() -> Result<Heif, String> {
        // An explicit library is the only one tried.
        let names: Vec<String> = match std::env::var_os("SPECULAR_LIBHEIF") {
            Some(p) => vec![p.to_string_lossy().into_owned()],
            None => NAMES.iter().map(|s| s.to_string()).collect(),
        };
        let mut errors = Vec::new();
        for n in &names {
            #[allow(unsafe_code)]
            // SAFETY: loading libheif runs its (and its dependencies') initialisers, which do
            // not touch our state; this happens once, on the main thread, before any other
            // thread exists.
            let lib = unsafe { libloading::Library::new(n) };
            match lib {
                Ok(lib) => return Self::bind(lib),
                Err(e) => errors.push(e.to_string()),
            }
        }
        Err(errors.last().cloned().unwrap_or_default())
    }

    fn bind(lib: libloading::Library) -> Result<Heif, String> {
        macro_rules! sym {
            ($name:literal, $ty:ty) => {{
                #[allow(unsafe_code)]
                // SAFETY: `$ty` is the C signature of this symbol in libheif ≥ 1.13 (the
                // declarations above follow heif.h); the pointer is copied out and used only
                // while `lib` is alive (both live in the returned `Heif`, which is never
                // dropped before the process exits).
                let s = unsafe { lib.get::<$ty>(concat!($name, "\0").as_bytes()) };
                *s.map_err(|e| format!("libheif lacks {}: {e}", $name))?
            }};
        }
        let init = sym!("heif_init", FnInit);
        let version = sym!("heif_get_version", FnVersion);
        let h = Heif {
            ctx_alloc: sym!("heif_context_alloc", FnCtxAlloc),
            ctx_free: sym!("heif_context_free", FnCtxFree),
            ctx_max_threads: sym!("heif_context_set_max_decoding_threads", FnCtxSetInt),
            ctx_max_size: sym!("heif_context_set_maximum_image_size_limit", FnCtxSetInt),
            ctx_read: sym!("heif_context_read_from_memory_without_copy", FnCtxRead),
            ctx_primary: sym!("heif_context_get_primary_image_handle", FnCtxPrimary),
            handle_width: sym!("heif_image_handle_get_width", FnHandleInt),
            handle_height: sym!("heif_image_handle_get_height", FnHandleInt),
            handle_alpha: sym!("heif_image_handle_has_alpha_channel", FnHandleInt),
            handle_luma_bits: sym!("heif_image_handle_get_luma_bits_per_pixel", FnHandleInt),
            handle_release: sym!("heif_image_handle_release", FnHandleRelease),
            profile_size: sym!(
                "heif_image_handle_get_raw_color_profile_size",
                FnProfileSize
            ),
            profile: sym!("heif_image_handle_get_raw_color_profile", FnProfile),
            decode: sym!("heif_decode_image", FnDecode),
            image_width: sym!("heif_image_get_primary_width", FnImageInt),
            image_height: sym!("heif_image_get_primary_height", FnImageInt),
            plane: sym!("heif_image_get_plane_readonly", FnPlane),
            image_release: sym!("heif_image_release", FnImageRelease),
            version: String::new(),
            _lib: lib,
        };
        #[allow(unsafe_code)]
        // SAFETY: `heif_init(NULL)` uses default parameters (loads the plugins from the default
        // directory or LIBHEIF_PLUGIN_PATH); called once, single-threaded.
        let e = unsafe { init(ptr::null_mut()) };
        if e.code != ERR_OK {
            return Err(format!("heif_init failed: {}", message(e)));
        }
        #[allow(unsafe_code)]
        // SAFETY: returns a pointer to a static NUL-terminated string (or NULL).
        let v = unsafe { version() };
        let version = cstr(v);
        Ok(Heif { version, ..h })
    }

    /// Decodes the primary image to RGBA8 with transformations applied into `out`.
    pub fn decode(
        &self,
        data: &[u8],
        limits: ImageLimits,
        threads: usize,
        out: &mut dyn Rows,
    ) -> Result<Meta, Failure> {
        let ctx = Ctx::new(self, data)?;
        // The limit applies to width and height; libheif also refuses larger tiles/grids.
        let side = c_int::try_from(limits.max_side).unwrap_or(c_int::MAX);
        let threads = c_int::try_from(threads.clamp(1, 64)).unwrap_or(1);
        check(ctx.configure_and_read(side, threads))?;
        let handle = ctx.primary()?;
        let (hw, hh) = handle.size();
        let check_size = |w: c_int, h: c_int| -> Result<(u32, u32), Failure> {
            let (w, h) = (u32::try_from(w).unwrap_or(0), u32::try_from(h).unwrap_or(0));
            if w == 0 || h == 0 {
                return Err((ErrorKind::Malformed, "image has no size".into()));
            }
            if w > limits.max_side
                || h > limits.max_side
                || u64::from(w) * u64::from(h) > limits.max_pixels
            {
                return Err((ErrorKind::TooLarge, format!("image of {w}×{h} too large")));
            }
            Ok((w, h))
        };
        check_size(hw, hh)?;
        let (has_alpha, bits) = handle.alpha_and_bits();
        let icc = handle.icc();
        let img = handle.decode()?;
        let (w, h) = check_size(img.width(), img.height())?;
        let meta = Meta {
            width: w,
            height: h,
            has_alpha,
            bit_depth: u8::try_from(bits.clamp(1, 32)).unwrap_or(8),
            icc,
        };
        out.start(&meta)?;
        let row_len = w as usize * 4;
        let (plane, stride) = img.plane(row_len, h as usize)?;
        for y in 0..h as usize {
            out.row(&plane[y * stride..y * stride + row_len])?;
        }
        Ok(meta)
    }
}

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    #[allow(unsafe_code)]
    // SAFETY: libheif returns NUL-terminated strings (static, or owned by a context that is
    // still alive when this is called).
    let s = unsafe { CStr::from_ptr(p) };
    s.to_string_lossy().into_owned()
}

fn message(e: HeifError) -> String {
    let m = cstr(e.message);
    if m.is_empty() {
        format!("libheif error {}.{}", e.code, e.subcode)
    } else {
        m
    }
}

fn check(e: HeifError) -> Result<(), Failure> {
    let kind = match e.code {
        ERR_OK => return Ok(()),
        ERR_INVALID_INPUT | ERR_DECODER_PLUGIN => ErrorKind::Malformed,
        ERR_UNSUPPORTED_FILETYPE | ERR_UNSUPPORTED_FEATURE => ErrorKind::Unsupported,
        ERR_MEMORY => ErrorKind::TooLarge,
        _ => ErrorKind::Internal,
    };
    Err((kind, format!("libheif: {}", message(e))))
}

/// A `heif_context`, freed on drop. It reads `data` without copying, so it borrows it.
struct Ctx<'a> {
    h: &'a Heif,
    p: *mut Context,
    data: &'a [u8],
}

impl<'a> Ctx<'a> {
    fn new(h: &'a Heif, data: &'a [u8]) -> Result<Ctx<'a>, Failure> {
        #[allow(unsafe_code)]
        // SAFETY: no preconditions.
        let p = unsafe { (h.ctx_alloc)() };
        if p.is_null() {
            return Err((ErrorKind::TooLarge, "heif_context_alloc failed".into()));
        }
        Ok(Ctx { h, p, data })
    }

    /// Sets the limits, then parses the input.
    fn configure_and_read(&self, max_side: c_int, threads: c_int) -> HeifError {
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live context.
        unsafe {
            (self.h.ctx_max_size)(self.p, max_side)
        };
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live context.
        unsafe {
            (self.h.ctx_max_threads)(self.p, threads)
        };
        #[allow(unsafe_code)]
        // SAFETY: `self.data` is valid for its length and outlives the context (it is borrowed
        // for `'a`, the context's own lifetime). The reading options may be NULL.
        unsafe {
            (self.h.ctx_read)(
                self.p,
                self.data.as_ptr().cast(),
                self.data.len(),
                ptr::null(),
            )
        }
    }

    fn primary(&self) -> Result<ImgHandle<'_>, Failure> {
        let mut p = ptr::null_mut();
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live context that has read a file; `p` is a valid out pointer.
        let e = unsafe { (self.h.ctx_primary)(self.p, &mut p) };
        check(e)?;
        if p.is_null() {
            return Err((ErrorKind::Malformed, "no primary image".into()));
        }
        Ok(ImgHandle {
            h: self.h,
            p,
            _ctx: self,
        })
    }
}

impl Drop for Ctx<'_> {
    fn drop(&mut self) {
        #[allow(unsafe_code)]
        // SAFETY: `self.p` came from `heif_context_alloc` and is freed once; handles and
        // images borrowing this context were dropped first (they borrow `self`).
        unsafe {
            (self.h.ctx_free)(self.p)
        };
    }
}

struct ImgHandle<'a> {
    h: &'a Heif,
    p: *mut Handle,
    _ctx: &'a Ctx<'a>,
}

impl ImgHandle<'_> {
    fn size(&self) -> (c_int, c_int) {
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live handle.
        let w = unsafe { (self.h.handle_width)(self.p) };
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live handle.
        let h = unsafe { (self.h.handle_height)(self.p) };
        (w, h)
    }

    fn alpha_and_bits(&self) -> (bool, c_int) {
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live handle.
        let a = unsafe { (self.h.handle_alpha)(self.p) };
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live handle.
        let b = unsafe { (self.h.handle_luma_bits)(self.p) };
        (a != 0, b)
    }

    /// The ICC profile, if the file has one (≤ 16 MiB).
    fn icc(&self) -> Vec<u8> {
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live handle.
        let n = unsafe { (self.h.profile_size)(self.p) };
        if n == 0 || n > 16 << 20 {
            return Vec::new();
        }
        let mut buf = vec![0u8; n];
        #[allow(unsafe_code)]
        // SAFETY: `buf` holds exactly the `n` bytes libheif reported for this profile.
        let e = unsafe { (self.h.profile)(self.p, buf.as_mut_ptr().cast()) };
        if e.code == ERR_OK { buf } else { Vec::new() }
    }

    fn decode(&self) -> Result<Img<'_>, Failure> {
        let mut p = ptr::null_mut();
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live handle, `p` a valid out pointer, NULL options mean the
        // defaults (transformations applied).
        let e = unsafe {
            (self.h.decode)(
                self.p,
                &mut p,
                COLORSPACE_RGB,
                CHROMA_INTERLEAVED_RGBA,
                ptr::null(),
            )
        };
        check(e)?;
        if p.is_null() {
            return Err((ErrorKind::Internal, "libheif returned no image".into()));
        }
        Ok(Img {
            h: self.h,
            p,
            _handle: self,
        })
    }
}

impl Drop for ImgHandle<'_> {
    fn drop(&mut self) {
        #[allow(unsafe_code)]
        // SAFETY: `self.p` came from `heif_context_get_primary_image_handle`, released once.
        unsafe {
            (self.h.handle_release)(self.p)
        };
    }
}

struct Img<'a> {
    h: &'a Heif,
    p: *mut Image,
    _handle: &'a ImgHandle<'a>,
}

impl Img<'_> {
    fn width(&self) -> c_int {
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live image.
        unsafe {
            (self.h.image_width)(self.p)
        }
    }

    fn height(&self) -> c_int {
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live image.
        unsafe {
            (self.h.image_height)(self.p)
        }
    }

    /// The interleaved RGBA plane as a slice covering `rows` rows of at least `row_len` bytes,
    /// and its stride.
    fn plane(&self, row_len: usize, rows: usize) -> Result<(&[u8], usize), Failure> {
        let mut stride: c_int = 0;
        #[allow(unsafe_code)]
        // SAFETY: `self.p` is a live image; `stride` a valid out pointer.
        let data = unsafe { (self.h.plane)(self.p, CHANNEL_INTERLEAVED, &mut stride) };
        let stride = usize::try_from(stride).unwrap_or(0);
        if data.is_null() || stride < row_len || rows == 0 {
            return Err((ErrorKind::Internal, "libheif returned no RGBA plane".into()));
        }
        let len = stride * (rows - 1) + row_len;
        #[allow(unsafe_code)]
        // SAFETY: an interleaved plane of a `width × height` image holds `height` rows of
        // `stride` bytes, the first `width * 4` of each being pixels (`width`/`height` are the
        // image's own, checked by the caller against `row_len`/`rows`). It stays valid and
        // unmodified while `self` (the image) lives, which the returned borrow ensures.
        let s = unsafe { std::slice::from_raw_parts(data, len) };
        Ok((s, stride))
    }
}

impl Drop for Img<'_> {
    fn drop(&mut self) {
        #[allow(unsafe_code)]
        // SAFETY: `self.p` came from `heif_decode_image` and is released once.
        unsafe {
            (self.h.image_release)(self.p)
        };
    }
}
