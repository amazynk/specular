//! Blackmagic RAW through the Blackmagic RAW SDK the user installed, loaded at run time.
//!
//! The SDK is never bundled, redistributed or downloaded: the viewer finds the copy the user
//! installed (or the user picks it) and names it with `--braw=PATH`. The host loads it, and
//! every decoder plugin beside it, before it confines itself; nothing is loaded afterwards.
//!
//! The SDK's API is COM-style C++ (`BlackmagicRawAPI.h`): interfaces are vtables of
//! `extern "system"` functions behind `IUnknown`. They are declared by hand below, for SDK
//! 6.0, the first whose codec opens a clip from a caller's file object
//! (`IBlackmagicRaw::OpenClipFromFile`): the clip arrives as a descriptor or handle the viewer
//! passed, and the SDK reads it through [`FileObj`], never by path. An older SDK does not
//! answer the 6.0 codec interface and is refused at load.
//!
//! Strings are the SDK's per-platform type: UTF-8 `char*` (Linux), `CFStringRef` (macOS),
//! `BSTR` (Windows). Interface ids are `REFIID`s: 16 bytes by value outside Windows
//! (`CFUUIDBytes`), a pointer to a `GUID` on Windows.
//!
//! Decoding runs on the SDK's CPU pipeline. The host asks for one frame (read job, then a
//! decode-and-process job submitted from the read callback), waits with `FlushJobs`, and
//! copies the processed RGBA8 image out in the process callback, as Blackmagic's samples do.
//!
//! What is not verified against the real SDK (none on the build machine, and downloading it
//! needs registration): the vtable orders and ids follow the 6.0 interface as published by
//! the open-source `braw` bindings and Blackmagic's documentation, checked against a mock
//! library (`examples/braw_mock.rs`) that implements the same ABI. See the crate's tests.

use specular_sandbox::protocol::{ClipInfo, ErrorKind, ImageLimits};
use std::ffi::c_void;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

pub type Failure = (ErrorKind, String);

// ---------------------------------------------------------------------------------------------
// COM basics

type HResult = i32;

/// `ULONG` of the SDK's COM layer: 64-bit on Linux (`LinuxCOM.h` uses `unsigned long`).
#[cfg(all(unix, not(target_vendor = "apple")))]
type ULong = std::ffi::c_ulong;
#[cfg(any(windows, target_vendor = "apple"))]
type ULong = u32;

/// HRESULTs: Windows' own; the SDK's Linux and macOS headers (`LinuxCOM.h`, `CFPlugInCOM.h`)
/// number the failures differently.
#[cfg(windows)]
mod hr {
    pub const E_NOINTERFACE: i32 = 0x8000_4002_u32 as i32;
    pub const E_POINTER: i32 = 0x8000_4003_u32 as i32;
    pub const E_FAIL: i32 = 0x8000_4005_u32 as i32;
    pub const E_ACCESSDENIED: i32 = 0x8007_0005_u32 as i32;
}
#[cfg(not(windows))]
mod hr {
    pub const E_NOINTERFACE: i32 = 0x8000_0004_u32 as i32;
    pub const E_POINTER: i32 = 0x8000_0005_u32 as i32;
    pub const E_FAIL: i32 = 0x8000_0008_u32 as i32;
    pub const E_ACCESSDENIED: i32 = 0x8000_0009_u32 as i32;
}
const S_OK: HResult = 0;

fn ok(h: HResult) -> bool {
    h >= 0
}

/// An interface id in memory as the SDK expects it on this platform.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Iid([u8; 16]);

/// The id `d1-d2-d3-d4` as written in the header.
const fn iid(d1: u32, d2: u16, d3: u16, d4: [u8; 8]) -> Iid {
    // Windows: a GUID struct (little-endian fields). Elsewhere: CFUUIDBytes, the bytes in
    // the order they are written.
    let (a, b, c) = if cfg!(windows) {
        (d1.to_le_bytes(), d2.to_le_bytes(), d3.to_le_bytes())
    } else {
        (d1.to_be_bytes(), d2.to_be_bytes(), d3.to_be_bytes())
    };
    Iid([
        a[0], a[1], a[2], a[3], b[0], b[1], c[0], c[1], d4[0], d4[1], d4[2], d4[3], d4[4], d4[5],
        d4[6], d4[7],
    ])
}

pub const IID_IUNKNOWN: Iid = iid(0, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
/// The 6.0 codec interface.
pub const IID_RAW: Iid = iid(
    0xC25C15E2,
    0xD1D0,
    0x4743,
    [0x9E, 0xA8, 0xF2, 0x1F, 0x69, 0x75, 0x1A, 0xF3],
);
pub const IID_CONFIGURATION: Iid = iid(
    0x267E9866,
    0xFB40,
    0x4BFB,
    [0x8B, 0xF8, 0x96, 0xEA, 0x3F, 0x7D, 0xA3, 0x6E],
);
pub const IID_CALLBACK: Iid = iid(
    0xE9F98FAC,
    0x33DB,
    0x4A65,
    [0xBB, 0x94, 0x8A, 0x82, 0xB0, 0x27, 0xAE, 0xD0],
);
pub const IID_FILESYSTEM: Iid = iid(
    0x48640A3F,
    0xDC26,
    0x4CF9,
    [0xB5, 0x40, 0x1C, 0xD8, 0x08, 0x79, 0x86, 0xDD],
);
pub const IID_FILE: Iid = iid(
    0x9D0B46AB,
    0xC9DC,
    0x4BFB,
    [0x8D, 0xDF, 0x5E, 0x5A, 0x8F, 0x19, 0xBE, 0xFC],
);

/// `REFIID`: by value outside Windows, by pointer on Windows.
#[cfg(not(windows))]
type RefIid = Iid;
#[cfg(windows)]
type RefIid = *const Iid;

#[cfg(not(windows))]
fn refiid(i: &Iid) -> RefIid {
    *i
}
#[cfg(windows)]
fn refiid(i: &Iid) -> RefIid {
    i
}

/// The id a `REFIID` argument names (null on Windows reads as no id).
#[cfg(not(windows))]
fn read_iid(r: RefIid) -> Option<Iid> {
    Some(r)
}
#[cfg(windows)]
fn read_iid(r: RefIid) -> Option<Iid> {
    if r.is_null() {
        return None;
    }
    #[allow(unsafe_code)]
    // SAFETY: a non-null REFIID points to a GUID the caller keeps alive for the call.
    Some(unsafe { *r })
}

/// Four-character codes of the SDK's enums.
const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}
const PIPELINE_CPU: u32 = fourcc(b"cpub");
const FORMAT_RGBA_U8: u32 = fourcc(b"rgba");
const SCALE_FULL: u32 = fourcc(b"full");
const SCALE_HALF: u32 = fourcc(b"half");
const SCALE_QUARTER: u32 = fourcc(b"qrtr");
const SCALE_EIGHTH: u32 = fourcc(b"eith");

type This = *mut c_void;

#[repr(C)]
pub struct UnknownVtbl {
    query_interface: unsafe extern "system" fn(This, RefIid, *mut *mut c_void) -> HResult,
    add_ref: unsafe extern "system" fn(This) -> ULong,
    release: unsafe extern "system" fn(This) -> ULong,
}

// Interfaces this host calls: each vtable as far as the last method used.

#[repr(C)]
struct FactoryVtbl {
    base: UnknownVtbl,
    create_codec: unsafe extern "system" fn(This, *mut This) -> HResult,
}

#[repr(C)]
struct RawVtbl {
    base: UnknownVtbl,
    _open_clip: usize,
    _open_clip_with_geometry: usize,
    open_clip_from_file: unsafe extern "system" fn(This, This, *mut This) -> HResult,
    _open_clip_from_file_with_geometry: usize,
    set_callback: unsafe extern "system" fn(This, This) -> HResult,
    _prepare_pipeline: usize,
    _prepare_pipeline_for_device: usize,
    flush_jobs: unsafe extern "system" fn(This) -> HResult,
}

#[repr(C)]
struct ConfigurationVtbl {
    base: UnknownVtbl,
    set_pipeline: unsafe extern "system" fn(This, u32, *mut c_void, *mut c_void) -> HResult,
    _get_pipeline: usize,
    _is_pipeline_supported: usize,
    set_cpu_threads: unsafe extern "system" fn(This, u32) -> HResult,
    _get_cpu_threads: usize,
    _get_max_cpu_thread_count: usize,
    _set_write_metadata_per_frame: usize,
    _get_write_metadata_per_frame: usize,
    _set_from_device: usize,
    get_version: unsafe extern "system" fn(This, *mut SdkStr) -> HResult,
    get_camera_support_version: unsafe extern "system" fn(This, *mut SdkStr) -> HResult,
}

#[repr(C)]
struct ClipVtbl {
    base: UnknownVtbl,
    get_width: unsafe extern "system" fn(This, *mut u32) -> HResult,
    get_height: unsafe extern "system" fn(This, *mut u32) -> HResult,
    get_frame_rate: unsafe extern "system" fn(This, *mut f32) -> HResult,
    get_frame_count: unsafe extern "system" fn(This, *mut u64) -> HResult,
    _get_timecode_for_frame: usize,
    _get_metadata_iterator: usize,
    _get_metadata: usize,
    _set_metadata: usize,
    get_camera_type: unsafe extern "system" fn(This, *mut SdkStr) -> HResult,
    _clone_clip_processing_attributes: usize,
    _get_multicard_file_count: usize,
    _is_multicard_file_present: usize,
    _get_sidecar_file_attached: usize,
    _save_sidecar_file: usize,
    _reload_sidecar_file: usize,
    create_job_read_frame: unsafe extern "system" fn(This, u64, *mut This) -> HResult,
}

#[repr(C)]
struct FrameVtbl {
    base: UnknownVtbl,
    _get_frame_index: usize,
    _get_timecode: usize,
    _get_metadata_iterator: usize,
    _get_metadata: usize,
    _set_metadata: usize,
    _clone_frame_processing_attributes: usize,
    set_resolution_scale: unsafe extern "system" fn(This, u32) -> HResult,
    _get_resolution_scale: usize,
    set_resource_format: unsafe extern "system" fn(This, u32) -> HResult,
    _get_resource_format: usize,
    _get_sensor_rate: usize,
    create_job_decode_and_process_frame:
        unsafe extern "system" fn(This, This, This, *mut This) -> HResult,
}

#[repr(C)]
struct ProcessedImageVtbl {
    base: UnknownVtbl,
    get_width: unsafe extern "system" fn(This, *mut u32) -> HResult,
    get_height: unsafe extern "system" fn(This, *mut u32) -> HResult,
    get_resource: unsafe extern "system" fn(This, *mut *mut c_void) -> HResult,
    _get_resource_type: usize,
    get_resource_format: unsafe extern "system" fn(This, *mut u32) -> HResult,
    get_resource_size_bytes: unsafe extern "system" fn(This, *mut u32) -> HResult,
}

#[repr(C)]
struct JobVtbl {
    base: UnknownVtbl,
    submit: unsafe extern "system" fn(This) -> HResult,
}

/// An interface pointer this host holds one reference to (released on drop).
struct Com<V> {
    p: NonNull<c_void>,
    _v: std::marker::PhantomData<*const V>,
}

impl<V> Com<V> {
    /// Takes over the reference `p` carries (null: none).
    fn take(p: This) -> Option<Com<V>> {
        NonNull::new(p).map(|p| Com {
            p,
            _v: std::marker::PhantomData,
        })
    }

    fn raw(&self) -> This {
        self.p.as_ptr()
    }

    /// Its vtable: `V` is the layout (or a prefix of it) of the interface this pointer was
    /// obtained as (checked by the SDK's own QueryInterface, or documented as the out type of
    /// the call that produced it).
    fn vt(&self) -> &V {
        vtbl_of(self.raw())
    }

    /// Every interface's vtable starts with IUnknown's three methods.
    fn unknown(&self) -> &UnknownVtbl {
        vtbl_of(self.raw())
    }

    /// The same object as interface `iid` with layout `W`.
    fn query<W>(&self, iid: &Iid) -> Result<Com<W>, HResult> {
        let mut out: This = ptr::null_mut();
        #[allow(unsafe_code)]
        // SAFETY: `self` is a live object; `out` is a valid out pointer.
        let h = unsafe { (self.unknown().query_interface)(self.raw(), refiid(iid), &mut out) };
        match Com::take(out) {
            Some(c) if ok(h) => Ok(c),
            Some(c) => {
                drop(c);
                Err(h)
            }
            None => Err(if ok(h) { hr::E_POINTER } else { h }),
        }
    }
}

/// The vtable of the live COM object `p`, read as layout `V`. The caller names the layout of
/// the interface `p` is (or a prefix of it); the vtable outlives any use here, which is
/// within a call on the object.
fn vtbl_of<'a, V>(p: This) -> &'a V {
    #[allow(unsafe_code)]
    // SAFETY: a COM object starts with a pointer to its vtable.
    let v = unsafe { p.cast::<*const V>().read() };
    #[allow(unsafe_code)]
    // SAFETY: that vtable is static for the object's class; see the function docs for `V`.
    unsafe {
        &*v
    }
}

impl<V> Drop for Com<V> {
    fn drop(&mut self) {
        #[allow(unsafe_code)]
        // SAFETY: this wrapper owns one reference, released once.
        unsafe {
            (self.unknown().release)(self.raw())
        };
    }
}

// SAFETY-relevant: the SDK's objects are free-threaded (its samples use them from its worker
// threads); the host calls them from one thread at a time.
#[allow(unsafe_code)]
// SAFETY: see above; the pointer is only used through the SDK's thread-safe interface.
unsafe impl<V> Send for Com<V> {}
#[allow(unsafe_code)]
// SAFETY: as for `Send`.
unsafe impl<V> Sync for Com<V> {}

// ---------------------------------------------------------------------------------------------
// Strings

#[cfg(all(unix, not(target_vendor = "apple")))]
type SdkStr = *const std::ffi::c_char;
#[cfg(target_vendor = "apple")]
type SdkStr = *const c_void;
#[cfg(windows)]
type SdkStr = *const u16;

#[cfg(all(unix, not(target_vendor = "apple")))]
#[allow(unsafe_code)]
// SAFETY (declarations): the C library's allocator, linked into every Unix program.
unsafe extern "C" {
    fn malloc(n: usize) -> *mut c_void;
    fn free(p: *mut c_void);
}

#[cfg(target_vendor = "apple")]
#[link(name = "CoreFoundation", kind = "framework")]
#[allow(unsafe_code)]
// SAFETY (declarations): signatures from <CoreFoundation/CFString.h>.
unsafe extern "C" {
    fn CFStringCreateWithBytes(
        alloc: *const c_void,
        bytes: *const u8,
        len: isize,
        encoding: u32,
        external: u8,
    ) -> *const c_void;
    fn CFStringGetLength(s: *const c_void) -> isize;
    fn CFStringGetMaximumSizeForEncoding(len: isize, encoding: u32) -> isize;
    fn CFStringGetCString(s: *const c_void, buf: *mut u8, size: isize, encoding: u32) -> u8;
    fn CFRelease(p: *const c_void);
}
#[cfg(target_vendor = "apple")]
const UTF8: u32 = 0x0800_0100;

#[cfg(windows)]
#[link(name = "oleaut32")]
#[allow(unsafe_code)]
// SAFETY (declarations): signatures from <oleauto.h>.
unsafe extern "system" {
    fn SysAllocStringLen(s: *const u16, len: u32) -> *mut u16;
    fn SysStringLen(s: *const u16) -> u32;
    fn SysFreeString(s: *mut u16);
}

/// A new SDK string holding `s`, owned by whoever receives it (the SDK frees what it is
/// handed). Null when allocation fails.
fn new_sdk_str(s: &str) -> SdkStr {
    #[cfg(all(unix, not(target_vendor = "apple")))]
    {
        #[allow(unsafe_code)]
        // SAFETY: malloc of len + 1 bytes; checked for null before writing.
        let p = unsafe { malloc(s.len() + 1) }.cast::<u8>();
        if p.is_null() {
            return ptr::null();
        }
        #[allow(unsafe_code)]
        // SAFETY: `p` holds `s.len() + 1` bytes, freshly allocated (no overlap with `s`).
        let buf = unsafe { std::slice::from_raw_parts_mut(p, s.len() + 1) };
        buf[..s.len()].copy_from_slice(s.as_bytes());
        buf[s.len()] = 0;
        p.cast()
    }
    #[cfg(target_vendor = "apple")]
    {
        #[allow(unsafe_code)]
        // SAFETY: `s` is valid UTF-8 of `s.len()` bytes; default allocator.
        unsafe {
            CFStringCreateWithBytes(ptr::null(), s.as_ptr(), s.len() as isize, UTF8, 0)
        }
    }
    #[cfg(windows)]
    {
        let w: Vec<u16> = s.encode_utf16().collect();
        #[allow(unsafe_code)]
        // SAFETY: `w` holds `w.len()` UTF-16 units.
        unsafe {
            SysAllocStringLen(w.as_ptr(), w.len() as u32)
        }
    }
}

/// Reads and frees a string the SDK returned (its caller owns it).
fn take_sdk_str(p: SdkStr) -> String {
    if p.is_null() {
        return String::new();
    }
    #[cfg(all(unix, not(target_vendor = "apple")))]
    {
        #[allow(unsafe_code)]
        // SAFETY: the SDK returns a NUL-terminated string it allocated for the caller.
        let s = unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned();
        #[allow(unsafe_code)]
        // SAFETY: the caller frees it, once (the SDK allocates plain `char` arrays).
        unsafe {
            free(p.cast_mut().cast())
        };
        s
    }
    #[cfg(target_vendor = "apple")]
    {
        #[allow(unsafe_code)]
        // SAFETY: `p` is a CFStringRef the caller owns.
        let len = unsafe { CFStringGetLength(p) };
        #[allow(unsafe_code)]
        // SAFETY: a pure size computation on a length and an encoding constant.
        let n = unsafe { CFStringGetMaximumSizeForEncoding(len, UTF8) } + 1;
        let mut buf = vec![0u8; n.clamp(1, 1 << 20) as usize];
        #[allow(unsafe_code)]
        // SAFETY: `buf` holds `buf.len()` bytes.
        let got = unsafe { CFStringGetCString(p, buf.as_mut_ptr(), buf.len() as isize, UTF8) };
        #[allow(unsafe_code)]
        // SAFETY: released once, by its owner.
        unsafe {
            CFRelease(p)
        };
        if got == 0 {
            return String::new();
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[..end]).into_owned()
    }
    #[cfg(windows)]
    {
        #[allow(unsafe_code)]
        // SAFETY: `p` is a BSTR: its length precedes it.
        let n = unsafe { SysStringLen(p) } as usize;
        #[allow(unsafe_code)]
        // SAFETY: a BSTR holds `n` UTF-16 units.
        let s = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, n) });
        #[allow(unsafe_code)]
        // SAFETY: freed once, by its owner.
        unsafe {
            SysFreeString(p.cast_mut())
        };
        s
    }
}

// ---------------------------------------------------------------------------------------------
// Objects this host implements: the clip file, its file system (no companion files), the
// callback.

/// One reference-counted object of ours behind a vtable pointer. Freed when the last
/// reference (ours or the SDK's) goes.
#[repr(C)]
struct Obj<T: 'static> {
    vtbl: &'static T,
    refs: AtomicUsize,
    /// The interface ids it answers besides IUnknown.
    iid: Iid,
}

/// IUnknown for an object whose layout starts with `Obj<V>` (`B` is the whole object).
mod unknown {
    use super::*;

    pub extern "system" fn query<B: Header>(
        this: This,
        riid: RefIid,
        out: *mut *mut c_void,
    ) -> HResult {
        if out.is_null() {
            return hr::E_POINTER;
        }
        #[allow(unsafe_code)]
        // SAFETY: `this` is one of our objects of type `B` (only our vtables point here).
        let b = unsafe { &*this.cast::<B>() };
        let yes = read_iid(riid).is_some_and(|i| i == IID_IUNKNOWN || i == b.iid());
        #[allow(unsafe_code)]
        // SAFETY: `out` is a valid out pointer (checked non-null).
        unsafe {
            *out = if yes {
                b.refs().fetch_add(1, Ordering::Relaxed);
                this
            } else {
                ptr::null_mut()
            };
        }
        if yes { S_OK } else { hr::E_NOINTERFACE }
    }

    pub extern "system" fn add_ref<B: Header>(this: This) -> ULong {
        #[allow(unsafe_code)]
        // SAFETY: as in `query`.
        let b = unsafe { &*this.cast::<B>() };
        (b.refs().fetch_add(1, Ordering::Relaxed) + 1) as ULong
    }

    pub extern "system" fn release<B: Header>(this: This) -> ULong {
        #[allow(unsafe_code)]
        // SAFETY: as in `query`.
        let b = unsafe { &*this.cast::<B>() };
        let left = b.refs().fetch_sub(1, Ordering::AcqRel) - 1;
        if left == 0 {
            #[allow(unsafe_code)]
            // SAFETY: the last reference is gone; the object came from `Box::into_raw`.
            drop(unsafe { Box::from_raw(this.cast::<B>()) });
        }
        left as ULong
    }
}

/// What [`unknown`] needs of one of our objects.
trait Header {
    fn refs(&self) -> &AtomicUsize;
    fn iid(&self) -> Iid;
}

macro_rules! header {
    ($t:ty) => {
        impl Header for $t {
            fn refs(&self) -> &AtomicUsize {
                &self.head.refs
            }
            fn iid(&self) -> Iid {
                self.head.iid
            }
        }
    };
}

/// A new object (one reference, the caller's) as a raw interface pointer.
fn into_raw<B>(b: B) -> This {
    Box::into_raw(Box::new(b)).cast()
}

const fn unknown_vtbl<B: Header>() -> UnknownVtbl {
    UnknownVtbl {
        query_interface: unknown::query::<B>,
        add_ref: unknown::add_ref::<B>,
        release: unknown::release::<B>,
    }
}

/// `struct BmdIoVec`.
#[repr(C)]
struct IoVec {
    base: *mut c_void,
    len: u64,
}

#[repr(C)]
struct FileVtbl {
    base: UnknownVtbl,
    get_filesystem: unsafe extern "system" fn(This, *mut This) -> HResult,
    get_file_name: unsafe extern "system" fn(This, *mut SdkStr) -> HResult,
    set_file_length: unsafe extern "system" fn(This, u64) -> HResult,
    get_file_length: unsafe extern "system" fn(This, *mut u64) -> HResult,
    read_v: unsafe extern "system" fn(This, *mut IoVec, u32, u64, *mut u64) -> HResult,
    write_v: unsafe extern "system" fn(This, *mut IoVec, u32, u64, *mut u64) -> HResult,
    get_preferred_io_alignment: unsafe extern "system" fn(This, *mut u32) -> HResult,
    flush_writes: unsafe extern "system" fn(This) -> HResult,
    commit_file: unsafe extern "system" fn(This) -> HResult,
}

/// The clip as an `IBlackmagicRawFile`: positioned reads of the file the viewer passed.
#[repr(C)]
struct FileObj {
    head: Obj<FileVtbl>,
    file: File,
    len: u64,
}
header!(FileObj);

static FILE_VTBL: FileVtbl = FileVtbl {
    base: unknown_vtbl::<FileObj>(),
    get_filesystem: file::get_filesystem,
    get_file_name: file::get_file_name,
    set_file_length: file::read_only_u64,
    get_file_length: file::get_file_length,
    read_v: file::read_v,
    write_v: file::write_v,
    get_preferred_io_alignment: file::alignment,
    flush_writes: file::read_only,
    commit_file: file::read_only,
};

/// The name the SDK knows the clip by (it derives sidecar names from it; there are none).
const CLIP_NAME: &str = "clip.braw";

mod file {
    use super::*;

    fn me<'a>(this: This) -> &'a FileObj {
        #[allow(unsafe_code)]
        // SAFETY: only `FILE_VTBL` points here, and it is only ever installed in a FileObj.
        unsafe {
            &*this.cast::<FileObj>()
        }
    }

    pub extern "system" fn get_filesystem(_this: This, out: *mut This) -> HResult {
        if out.is_null() {
            return hr::E_POINTER;
        }
        let fs = into_raw(FsObj {
            head: Obj {
                vtbl: &FS_VTBL,
                refs: AtomicUsize::new(1),
                iid: IID_FILESYSTEM,
            },
        });
        #[allow(unsafe_code)]
        // SAFETY: `out` is a valid out pointer; the reference goes to the caller.
        unsafe {
            *out = fs
        };
        S_OK
    }

    pub extern "system" fn get_file_name(_this: This, out: *mut SdkStr) -> HResult {
        if out.is_null() {
            return hr::E_POINTER;
        }
        let s = new_sdk_str(CLIP_NAME);
        if s.is_null() {
            return hr::E_FAIL;
        }
        #[allow(unsafe_code)]
        // SAFETY: valid out pointer; the caller owns the new string.
        unsafe {
            *out = s
        };
        S_OK
    }

    pub extern "system" fn get_file_length(this: This, out: *mut u64) -> HResult {
        if out.is_null() {
            return hr::E_POINTER;
        }
        #[allow(unsafe_code)]
        // SAFETY: valid out pointer.
        unsafe {
            *out = me(this).len
        };
        S_OK
    }

    pub extern "system" fn read_v(
        this: This,
        bufs: *mut IoVec,
        count: u32,
        offset: u64,
        read: *mut u64,
    ) -> HResult {
        if bufs.is_null() && count > 0 {
            return hr::E_POINTER;
        }
        let f = me(this);
        let mut at = offset;
        let mut total = 0u64;
        let mut failed = false;
        for i in 0..count as usize {
            #[allow(unsafe_code)]
            // SAFETY: `bufs` holds `count` entries.
            let v = unsafe { bufs.wrapping_add(i).read() };
            let Ok(len) = usize::try_from(v.len) else {
                return hr::E_FAIL;
            };
            if len == 0 {
                continue;
            }
            if v.base.is_null() {
                return hr::E_POINTER;
            }
            #[allow(unsafe_code)]
            // SAFETY: the caller hands `len` writable bytes at `base` for the call.
            let buf = unsafe { std::slice::from_raw_parts_mut(v.base.cast::<u8>(), len) };
            let mut done = 0;
            while done < len {
                match read_at(&f.file, &mut buf[done..], at) {
                    Ok(0) => break,
                    Ok(n) => {
                        done += n;
                        at += n as u64;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
            }
            total += done as u64;
            if done < len {
                break;
            }
        }
        if !read.is_null() {
            #[allow(unsafe_code)]
            // SAFETY: valid out pointer.
            unsafe {
                *read = total
            };
        }
        if failed { hr::E_FAIL } else { S_OK }
    }

    pub extern "system" fn write_v(
        _this: This,
        _bufs: *mut IoVec,
        _count: u32,
        _offset: u64,
        written: *mut u64,
    ) -> HResult {
        if !written.is_null() {
            #[allow(unsafe_code)]
            // SAFETY: valid out pointer.
            unsafe {
                *written = 0
            };
        }
        hr::E_ACCESSDENIED
    }

    pub extern "system" fn alignment(_this: This, out: *mut u32) -> HResult {
        if out.is_null() {
            return hr::E_POINTER;
        }
        #[allow(unsafe_code)]
        // SAFETY: valid out pointer.
        unsafe {
            *out = 1
        };
        S_OK
    }

    pub extern "system" fn read_only(_this: This) -> HResult {
        hr::E_ACCESSDENIED
    }

    pub extern "system" fn read_only_u64(_this: This, _v: u64) -> HResult {
        hr::E_ACCESSDENIED
    }
}

/// Positioned read; the file's shared offset is not used.
fn read_at(f: &File, buf: &mut [u8], at: u64) -> std::io::Result<usize> {
    #[cfg(unix)]
    {
        std::os::unix::fs::FileExt::read_at(f, buf, at)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::FileExt::seek_read(f, buf, at)
    }
}

#[repr(C)]
struct FsVtbl {
    base: UnknownVtbl,
    open_companion_file: unsafe extern "system" fn(This, This, SdkStr, *mut This) -> HResult,
    create_companion_file: unsafe extern "system" fn(This, This, SdkStr, u8, *mut This) -> HResult,
}

/// The clip's file system: there are no companion files (sidecars) to open, and none may be
/// written.
#[repr(C)]
struct FsObj {
    head: Obj<FsVtbl>,
}
header!(FsObj);

static FS_VTBL: FsVtbl = FsVtbl {
    base: unknown_vtbl::<FsObj>(),
    open_companion_file: fs_open,
    create_companion_file: fs_create,
};

extern "system" fn fs_open(_this: This, _parent: This, _name: SdkStr, out: *mut This) -> HResult {
    if !out.is_null() {
        #[allow(unsafe_code)]
        // SAFETY: valid out pointer.
        unsafe {
            *out = ptr::null_mut()
        };
    }
    hr::E_FAIL
}

extern "system" fn fs_create(
    _this: This,
    _parent: This,
    _name: SdkStr,
    _replace: u8,
    out: *mut This,
) -> HResult {
    if !out.is_null() {
        #[allow(unsafe_code)]
        // SAFETY: valid out pointer.
        unsafe {
            *out = ptr::null_mut()
        };
    }
    hr::E_ACCESSDENIED
}

#[repr(C)]
struct CallbackVtbl {
    base: UnknownVtbl,
    read_complete: unsafe extern "system" fn(This, This, HResult, This),
    read_audio_complete: unsafe extern "system" fn(This, This, HResult, This),
    decode_complete: unsafe extern "system" fn(This, This, HResult),
    process_complete: unsafe extern "system" fn(This, This, HResult, This),
    trim_progress: unsafe extern "system" fn(This, This, f32),
    trim_complete: unsafe extern "system" fn(This, This, HResult),
    sidecar_warning: unsafe extern "system" fn(This, This, SdkStr, u32, SdkStr),
    sidecar_error: unsafe extern "system" fn(This, This, SdkStr, u32, SdkStr),
    prepare_pipeline_complete: unsafe extern "system" fn(This, *mut c_void, HResult),
}

/// A decoded frame: width, height, RGBA8 pixels.
type Pixels = (u32, u32, Vec<u8>);

/// What one decode asked for and what came back.
struct Wanted {
    scale: u32,
    limits: ImageLimits,
    result: Option<Result<Pixels, Failure>>,
}

#[repr(C)]
struct CallbackObj {
    head: Obj<CallbackVtbl>,
    state: Mutex<Wanted>,
}
header!(CallbackObj);

static CALLBACK_VTBL: CallbackVtbl = CallbackVtbl {
    base: unknown_vtbl::<CallbackObj>(),
    read_complete: cb::read_complete,
    read_audio_complete: cb::ignore3,
    decode_complete: cb::ignore2,
    process_complete: cb::process_complete,
    trim_progress: cb::ignore_progress,
    trim_complete: cb::ignore2,
    sidecar_warning: cb::ignore_sidecar,
    sidecar_error: cb::ignore_sidecar,
    prepare_pipeline_complete: cb::ignore_prepare,
};

mod cb {
    use super::*;

    fn me<'a>(this: This) -> &'a CallbackObj {
        #[allow(unsafe_code)]
        // SAFETY: only `CALLBACK_VTBL` points here, installed only in a CallbackObj.
        unsafe {
            &*this.cast::<CallbackObj>()
        }
    }

    fn fail(this: This, kind: ErrorKind, why: String) {
        let mut st = me(this)
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if st.result.is_none() {
            st.result = Some(Err((kind, why)));
        }
    }

    /// The SDK hands each submitted job back in its callback; the callback releases it.
    fn release_job(job: This) {
        drop(Com::<JobVtbl>::take(job));
    }

    pub extern "system" fn read_complete(this: This, job: This, result: HResult, frame: This) {
        release_job(job);
        if !ok(result) || frame.is_null() {
            return fail(
                this,
                ErrorKind::Malformed,
                format!("reading the frame failed ({result:#x})"),
            );
        }
        let scale = me(this)
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .scale;
        // `frame` is the IBlackmagicRawFrame of this read, valid during the callback.
        let vt: &FrameVtbl = vtbl_of(frame);
        let mut decode: This = ptr::null_mut();
        #[allow(unsafe_code)]
        // SAFETY: live frame; an enum value of the SDK.
        let mut h = unsafe { (vt.set_resource_format)(frame, FORMAT_RGBA_U8) };
        if ok(h) {
            #[allow(unsafe_code)]
            // SAFETY: as above.
            let r = unsafe { (vt.set_resolution_scale)(frame, scale) };
            h = r;
        }
        if ok(h) {
            #[allow(unsafe_code)]
            // SAFETY: live frame; `decode` a valid out pointer; null processing attributes
            // mean the clip's own.
            let r = unsafe {
                (vt.create_job_decode_and_process_frame)(
                    frame,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut decode,
                )
            };
            h = r;
        }
        let Some(job) = Com::<JobVtbl>::take(decode).filter(|_| ok(h)) else {
            return fail(
                this,
                ErrorKind::Malformed,
                format!("the SDK cannot decode the frame ({h:#x})"),
            );
        };
        #[allow(unsafe_code)]
        // SAFETY: live job.
        let h = unsafe { (job.vt().submit)(job.raw()) };
        if ok(h) {
            // Submitted: the process callback releases it.
            std::mem::forget(job);
        } else {
            fail(
                this,
                ErrorKind::Internal,
                format!("the SDK refused the decode ({h:#x})"),
            );
        }
    }

    pub extern "system" fn process_complete(this: This, job: This, result: HResult, image: This) {
        release_job(job);
        let r = if !ok(result) || image.is_null() {
            Err((
                ErrorKind::Malformed,
                format!("decoding the frame failed ({result:#x})"),
            ))
        } else {
            let limits = me(this)
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .limits;
            copy_image(image, limits)
        };
        let mut st = me(this)
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if st.result.is_none() {
            st.result = Some(r);
        }
    }

    pub extern "system" fn ignore2(_this: This, _job: This, _r: HResult) {}
    pub extern "system" fn ignore3(_this: This, job: This, _r: HResult, _x: This) {
        release_job(job);
    }
    pub extern "system" fn ignore_progress(_this: This, _job: This, _p: f32) {}
    pub extern "system" fn ignore_sidecar(
        _this: This,
        _clip: This,
        _name: SdkStr,
        _line: u32,
        _info: SdkStr,
    ) {
    }
    pub extern "system" fn ignore_prepare(_this: This, _user: *mut c_void, _r: HResult) {}
}

/// Copies an `IBlackmagicRawProcessedImage` (CPU, RGBA8) out, within `limits`.
fn copy_image(image: This, limits: ImageLimits) -> Result<Pixels, Failure> {
    // `image` is the processed image of this job, valid during the callback.
    let vt: &ProcessedImageVtbl = vtbl_of(image);
    let (mut w, mut h, mut fmt, mut size) = (0u32, 0u32, 0u32, 0u32);
    let mut data: *mut c_void = ptr::null_mut();
    let u32_getters = [
        (vt.get_width, &mut w),
        (vt.get_height, &mut h),
        (vt.get_resource_format, &mut fmt),
        (vt.get_resource_size_bytes, &mut size),
    ];
    let mut r = S_OK;
    for (get, out) in u32_getters {
        #[allow(unsafe_code)]
        // SAFETY: live image, valid out pointer.
        let x = unsafe { get(image, out) };
        r = x;
        if !ok(r) {
            break;
        }
    }
    if ok(r) {
        #[allow(unsafe_code)]
        // SAFETY: live image, valid out pointer.
        let x = unsafe { (vt.get_resource)(image, &mut data) };
        r = x;
    }
    if !ok(r) {
        return Err((
            ErrorKind::Internal,
            format!("the SDK's image is unreadable ({r:#x})"),
        ));
    }
    if w == 0 || h == 0 {
        return Err((ErrorKind::Malformed, "the frame has no size".into()));
    }
    if w > limits.max_side || h > limits.max_side || u64::from(w) * u64::from(h) > limits.max_pixels
    {
        return Err((ErrorKind::TooLarge, format!("frame of {w}×{h} too large")));
    }
    let len = w as usize * h as usize * 4;
    if fmt != FORMAT_RGBA_U8 || (size as usize) < len || data.is_null() {
        return Err((
            ErrorKind::Internal,
            "the SDK returned another pixel format".into(),
        ));
    }
    let mut out = Vec::new();
    out.try_reserve_exact(len)
        .map_err(|_| (ErrorKind::TooLarge, format!("cannot allocate {len} bytes")))?;
    #[allow(unsafe_code)]
    // SAFETY: a CPU resource of `size` ≥ `len` bytes at `data`, valid during the callback.
    out.extend_from_slice(unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len) });
    Ok((w, h, out))
}

// ---------------------------------------------------------------------------------------------
// The loaded SDK

/// The library file's name in an SDK folder.
pub const LIB_NAME: &str = if cfg!(windows) {
    "BlackmagicRawAPI.dll"
} else if cfg!(target_vendor = "apple") {
    "BlackmagicRawAPI"
} else {
    "libBlackmagicRawAPI.so"
};

/// The library file for `path`: the file itself, or the SDK library in that folder (a macOS
/// framework, an SDK's `Libraries` folder).
pub fn library_file(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let mut c = vec![path.join(LIB_NAME)];
    if cfg!(target_vendor = "apple") {
        c.push(path.join("BlackmagicRawAPI.framework").join(LIB_NAME));
    }
    c.into_iter().find(|p| p.is_file())
}

/// The decoder plugins beside the library (loaded by bare name later otherwise, which the
/// sandbox no longer allows): every shared library in its folder (and a macOS framework's),
/// except the GPU pipelines, which are never used here.
fn plugins(lib: &Path) -> Vec<PathBuf> {
    let Some(dir) = lib.parent() else {
        return Vec::new();
    };
    let ext = if cfg!(windows) {
        "dll"
    } else if cfg!(target_vendor = "apple") {
        "dylib"
    } else {
        "so"
    };
    let mut dirs = vec![dir.to_path_buf()];
    if cfg!(target_vendor = "apple") {
        dirs.push(dir.join("Libraries"));
        dirs.push(dir.join("Versions/A/Libraries"));
    }
    let mut out = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_ascii_lowercase();
            let shared = p.extension().is_some_and(|x| x.eq_ignore_ascii_case(ext))
                || (ext == "so" && name.contains(".so."));
            let gpu = ["cuda", "opencl", "metal"].iter().any(|g| name.contains(g));
            if shared && !gpu && p != lib && p.is_file() {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

type FnCreateFactory = unsafe extern "system" fn() -> This;

pub struct Braw {
    _plugins: Vec<libloading::Library>,
    _factory: Com<FactoryVtbl>,
    codec: Com<RawVtbl>,
    /// "6.0 (cameras 6.0)", for the Settings page.
    pub version: String,
    /// One decode at a time (the host serves one request at a time anyway).
    busy: Mutex<()>,
    _lib: libloading::Library,
}

impl Braw {
    /// Loads the SDK at `path` (its library, or a folder holding it), creates its codec on
    /// the CPU pipeline with `threads` threads, or says why not.
    pub fn load(path: &Path, threads: usize) -> Result<Braw, String> {
        let lib_path = library_file(path)
            .ok_or_else(|| format!("{} is not the Blackmagic RAW SDK library", path.display()))?;
        let mut loaded = Vec::new();
        for p in plugins(&lib_path) {
            #[allow(unsafe_code)]
            // SAFETY: a library of the SDK the user installed, loaded once on the main thread
            // before any other thread exists; its initialisers do not touch our state.
            if let Ok(l) = unsafe { libloading::Library::new(&p) } {
                loaded.push(l);
            }
        }
        #[allow(unsafe_code)]
        // SAFETY: as above.
        let lib = unsafe { libloading::Library::new(&lib_path) }.map_err(|e| e.to_string())?;
        #[allow(unsafe_code)]
        // SAFETY: `CreateBlackmagicRawFactoryInstance` takes nothing and returns a new
        // IBlackmagicRawFactory (one reference, the caller's) or null; the pointer is used
        // while `lib` lives (both are kept in the returned `Braw`).
        let create = unsafe {
            lib.get::<FnCreateFactory>(b"CreateBlackmagicRawFactoryInstance\0")
                .map(|s| *s)
        }
        .map_err(|e| format!("not the Blackmagic RAW SDK: {e}"))?;
        #[allow(unsafe_code)]
        // SAFETY: see `create`.
        let factory =
            Com::<FactoryVtbl>::take(unsafe { create() }).ok_or("the SDK created no factory")?;
        let mut c: This = ptr::null_mut();
        #[allow(unsafe_code)]
        // SAFETY: live factory, valid out pointer.
        let h = unsafe { (factory.vt().create_codec)(factory.raw(), &mut c) };
        let any = Com::<UnknownVtbl>::take(c)
            .filter(|_| ok(h))
            .ok_or_else(|| format!("the SDK created no codec ({h:#x})"))?;
        // The 6.0 codec interface (OpenClipFromFile): older SDKs do not answer it.
        let codec = any.query::<RawVtbl>(&IID_RAW).map_err(|h| {
            if h == hr::E_NOINTERFACE {
                "this Blackmagic RAW SDK is too old: version 6.0 or later is needed".to_owned()
            } else {
                format!("the SDK's codec is unusable ({h:#x})")
            }
        })?;
        drop(any);
        let config = codec
            .query::<ConfigurationVtbl>(&IID_CONFIGURATION)
            .map_err(|h| format!("the SDK cannot be configured ({h:#x})"))?;
        let threads = u32::try_from(threads.clamp(1, 64)).unwrap_or(1);
        let mut v: SdkStr = ptr::null();
        let mut cams: SdkStr = ptr::null();
        let cv = config.vt();
        #[allow(unsafe_code)]
        // SAFETY: live configuration; an enum value of the SDK; null context and queue are
        // what the CPU pipeline takes.
        let mut h = unsafe {
            (cv.set_pipeline)(config.raw(), PIPELINE_CPU, ptr::null_mut(), ptr::null_mut())
        };
        if ok(h) {
            #[allow(unsafe_code)]
            // SAFETY: live configuration.
            let r = unsafe { (cv.set_cpu_threads)(config.raw(), threads) };
            h = r;
        }
        #[allow(unsafe_code)]
        // SAFETY: live configuration; valid out pointer.
        let hv = unsafe { (cv.get_version)(config.raw(), &mut v) };
        #[allow(unsafe_code)]
        // SAFETY: live configuration; valid out pointer.
        let hc = unsafe { (cv.get_camera_support_version)(config.raw(), &mut cams) };
        if !ok(h) {
            return Err(format!("the SDK's CPU pipeline is unavailable ({h:#x})"));
        }
        let v = if ok(hv) {
            take_sdk_str(v)
        } else {
            String::new()
        };
        let cams = if ok(hc) {
            take_sdk_str(cams)
        } else {
            String::new()
        };
        // 5.x and 6.x answer GetVersion with "0.0"; the codec interface says 6.0 or later.
        let mut version = match v.trim() {
            "" | "0.0" => "6.0 or later".to_owned(),
            s => s.to_owned(),
        };
        if !cams.trim().is_empty() && cams.trim() != "0.0" {
            version = format!("{version} (cameras {})", cams.trim());
        }
        Ok(Braw {
            _plugins: loaded,
            _factory: factory,
            codec,
            version,
            busy: Mutex::new(()),
            _lib: lib,
        })
    }

    /// Decodes frame `frame` of the clip in `file` at 1/`scale`.
    pub fn decode(
        &self,
        file: File,
        frame: u64,
        scale: u8,
        limits: ImageLimits,
    ) -> Result<(ClipInfo, u32, u32, Vec<u8>), Failure> {
        let _one = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        let len = file
            .metadata()
            .map_err(|e| (ErrorKind::Io, e.to_string()))?
            .len();
        let fobj = into_raw(FileObj {
            head: Obj {
                vtbl: &FILE_VTBL,
                refs: AtomicUsize::new(1),
                iid: IID_FILE,
            },
            file,
            len,
        });
        let fobj = Com::<FileVtbl>::take(fobj).ok_or((ErrorKind::Internal, "no file".into()))?;
        let mut c: This = ptr::null_mut();
        #[allow(unsafe_code)]
        // SAFETY: live codec and file object (the SDK takes its own reference if it keeps
        // it); valid out pointer.
        let h =
            unsafe { (self.codec.vt().open_clip_from_file)(self.codec.raw(), fobj.raw(), &mut c) };
        let clip = Com::<ClipVtbl>::take(c).filter(|_| ok(h)).ok_or_else(|| {
            (
                ErrorKind::Malformed,
                format!("the SDK cannot open this clip ({h:#x})"),
            )
        })?;
        let info = clip_info(&clip)?;
        if frame >= info.frame_count {
            return Err((
                ErrorKind::Unsupported,
                format!("the clip has {} frames", info.frame_count),
            ));
        }
        let scale = match scale {
            1 => SCALE_FULL,
            2 => SCALE_HALF,
            4 => SCALE_QUARTER,
            _ => SCALE_EIGHTH,
        };
        let cbo = into_raw(CallbackObj {
            head: Obj {
                vtbl: &CALLBACK_VTBL,
                refs: AtomicUsize::new(1),
                iid: IID_CALLBACK,
            },
            state: Mutex::new(Wanted {
                scale,
                limits,
                result: None,
            }),
        });
        let cbo =
            Com::<CallbackVtbl>::take(cbo).ok_or((ErrorKind::Internal, "no callback".into()))?;
        let mut job: This = ptr::null_mut();
        #[allow(unsafe_code)]
        // SAFETY: live codec and callback (the codec takes its own reference).
        let hs = unsafe { (self.codec.vt().set_callback)(self.codec.raw(), cbo.raw()) };
        if !ok(hs) {
            return Err((
                ErrorKind::Internal,
                format!("the SDK refused the callback ({hs:#x})"),
            ));
        }
        #[allow(unsafe_code)]
        // SAFETY: live clip; valid out pointer.
        let hj = unsafe { (clip.vt().create_job_read_frame)(clip.raw(), frame, &mut job) };
        let job = Com::<JobVtbl>::take(job)
            .filter(|_| ok(hj))
            .ok_or_else(|| {
                (
                    ErrorKind::Malformed,
                    format!("the SDK cannot read frame {frame} ({hj:#x})"),
                )
            })?;
        #[allow(unsafe_code)]
        // SAFETY: live job.
        let h = unsafe { (job.vt().submit)(job.raw()) };
        if ok(h) {
            // Submitted: the read callback releases it.
            std::mem::forget(job);
        } else {
            return Err((
                ErrorKind::Internal,
                format!("the SDK refused the read ({h:#x})"),
            ));
        }
        #[allow(unsafe_code)]
        // SAFETY: live codec; waits for every submitted job and its callbacks.
        let h = unsafe { (self.codec.vt().flush_jobs)(self.codec.raw()) };
        let result = {
            #[allow(unsafe_code)]
            // SAFETY: `cbo` is our own CallbackObj (created above).
            let me = unsafe { &*cbo.raw().cast::<CallbackObj>() };
            me.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .result
                .take()
        };
        drop(clip);
        drop(fobj);
        match result {
            Some(Ok((w, hgt, px))) => Ok((info, w, hgt, px)),
            Some(Err(e)) => Err(e),
            None if !ok(h) => Err((
                ErrorKind::Internal,
                format!("the SDK's jobs failed ({h:#x})"),
            )),
            None => Err((ErrorKind::Internal, "the SDK returned no frame".into())),
        }
    }
}

fn clip_info(clip: &Com<ClipVtbl>) -> Result<ClipInfo, Failure> {
    let (mut w, mut h, mut rate, mut frames) = (0u32, 0u32, 0f32, 0u64);
    let mut cam: SdkStr = ptr::null();
    let vt = clip.vt();
    let c = clip.raw();
    #[allow(unsafe_code)]
    // SAFETY: live clip, valid out pointer (likewise below).
    let mut r = unsafe { (vt.get_width)(c, &mut w) };
    if ok(r) {
        #[allow(unsafe_code)]
        // SAFETY: as above.
        let x = unsafe { (vt.get_height)(c, &mut h) };
        r = x;
    }
    if ok(r) {
        #[allow(unsafe_code)]
        // SAFETY: as above.
        let x = unsafe { (vt.get_frame_count)(c, &mut frames) };
        r = x;
    }
    #[allow(unsafe_code)]
    // SAFETY: as above.
    if ok(r) && !ok(unsafe { (vt.get_frame_rate)(c, &mut rate) }) {
        rate = 0.0;
    }
    #[allow(unsafe_code)]
    // SAFETY: as above.
    let rc = unsafe { (vt.get_camera_type)(c, &mut cam) };
    if !ok(r) {
        return Err((
            ErrorKind::Malformed,
            format!("the SDK cannot read the clip ({r:#x})"),
        ));
    }
    let camera = if ok(rc) {
        take_sdk_str(cam)
    } else {
        String::new()
    };
    Ok(ClipInfo {
        width: w,
        height: h,
        frame_count: frames,
        frame_rate: if rate.is_finite() && rate > 0.0 {
            rate
        } else {
            0.0
        },
        camera,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_laid_out_per_platform() {
        let i = IID_RAW.0;
        if cfg!(windows) {
            assert_eq!(&i[..4], &[0xE2, 0x15, 0x5C, 0xC2]);
        } else {
            assert_eq!(&i[..4], &[0xC2, 0x5C, 0x15, 0xE2]);
        }
        assert_eq!(&i[8..], &[0x9E, 0xA8, 0xF2, 0x1F, 0x69, 0x75, 0x1A, 0xF3]);
        assert_eq!(IID_IUNKNOWN.0[8], 0xC0);
        assert_eq!(fourcc(b"rgba"), 0x7267_6261);
    }

    #[test]
    fn plugins_skip_gpu_pipelines() {
        let dir = std::env::temp_dir().join(format!("braw-plugins-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ext = if cfg!(windows) {
            "dll"
        } else if cfg!(target_vendor = "apple") {
            "dylib"
        } else {
            "so"
        };
        for n in [
            "DecoderCPU",
            "DecoderCUDA",
            "DecoderOpenCL",
            "InstructionSetServicesAVX2",
        ] {
            std::fs::write(dir.join(format!("lib{n}.{ext}")), b"").unwrap();
        }
        let lib = dir.join(LIB_NAME);
        std::fs::write(&lib, b"").unwrap();
        std::fs::write(dir.join("readme.txt"), b"").unwrap();
        let got: Vec<String> = plugins(&lib)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            got,
            [
                format!("libDecoderCPU.{ext}"),
                format!("libInstructionSetServicesAVX2.{ext}")
            ]
        );
        assert_eq!(library_file(&dir), Some(lib.clone()));
        assert_eq!(library_file(&lib), Some(lib));
        assert_eq!(library_file(&dir.join("nope")), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
