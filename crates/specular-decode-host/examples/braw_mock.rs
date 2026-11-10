//! A stand-in for the Blackmagic RAW SDK's library, for the host's tests (Linux only): the
//! same exported factory function and the same COM-style interfaces (SDK 6.0 layout, written
//! here a second time from the interface ids and method orders, not shared with the host), so
//! the host's bindings, its file object and its callback are exercised end to end inside the
//! real sandbox. It is never shipped.
//!
//! A "clip" is `MOCKBRAW`, width (u32), height (u32), frame count (u64), frame rate (f32),
//! camera name (u32 length, bytes), little endian. Frame `f` at 1/`s` is `width / s` ×
//! `height / s` RGBA8 with pixel `(x, y)` = `(x·s mod 256, y·s mod 256, f mod 256, 255)`.
//!
//! Like the SDK, jobs run on a worker thread of the library during `FlushJobs`, and the read
//! callback submits the decode job. `OpenClipFromFile` reads the header through the host's
//! file object, asks its file system for a sidecar (there must be none), and tries to open
//! `/etc/hostname` by path: the camera name gets " [opened a file]" if that worked, which a
//! sandboxed host must not allow.
//!
//! `SPECULAR_BRAW_MOCK=old` makes it an SDK older than 6.0 (no 6.0 codec interface).
#![allow(unsafe_code)] // A C ABI test double: every function here is called through raw pointers.
#![allow(clippy::missing_safety_doc)]
// A test double, not shipped: its raw-pointer plumbing is the point, one comment covers it.
#![allow(
    clippy::undocumented_unsafe_blocks,
    clippy::multiple_unsafe_ops_per_block
)]

#[cfg(target_os = "linux")]
mod mock {
    use std::collections::VecDeque;
    use std::ffi::{c_char, c_void};
    use std::ptr;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    type This = *mut c_void;
    type HResult = i32;
    type ULong = std::ffi::c_ulong;
    const S_OK: HResult = 0;
    const E_NOTIMPL: HResult = 0x8000_0001_u32 as i32;
    const E_NOINTERFACE: HResult = 0x8000_0004_u32 as i32;
    const E_FAIL: HResult = 0x8000_0008_u32 as i32;

    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct Iid([u8; 16]);

    /// From the GUID text, in CFUUIDBytes order.
    const fn guid(s: &[u8; 36]) -> Iid {
        const fn hex(c: u8) -> u8 {
            match c {
                b'0'..=b'9' => c - b'0',
                b'A'..=b'F' => c - b'A' + 10,
                _ => c - b'a' + 10,
            }
        }
        let mut out = [0u8; 16];
        let (mut i, mut o) = (0, 0);
        while i < 36 {
            if s[i] == b'-' {
                i += 1;
                continue;
            }
            out[o] = hex(s[i]) << 4 | hex(s[i + 1]);
            o += 1;
            i += 2;
        }
        Iid(out)
    }

    const IID_UNKNOWN: Iid = guid(b"00000000-0000-0000-C000-000000000046");
    const IID_RAW: Iid = guid(b"C25C15E2-D1D0-4743-9EA8-F21F69751AF3");
    const IID_CONFIGURATION: Iid = guid(b"267E9866-FB40-4BFB-8BF8-96EA3F7DA36E");

    const fn cc(s: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*s)
    }

    type Slot = usize;
    type Qi = unsafe extern "C" fn(This, Iid, *mut This) -> HResult;
    type Ref = unsafe extern "C" fn(This) -> ULong;

    #[repr(C)]
    struct Unknown {
        qi: Qi,
        add_ref: Ref,
        release: Ref,
    }

    /// Every mock object starts with its vtable and a count.
    #[repr(C)]
    struct Head<V: 'static> {
        vtbl: &'static V,
        refs: AtomicUsize,
    }

    trait Object {
        fn refs(&self) -> &AtomicUsize;
    }

    unsafe extern "C" fn add_ref<T: Object>(this: This) -> ULong {
        unsafe { &*this.cast::<T>() }
            .refs()
            .fetch_add(1, Ordering::Relaxed) as ULong
            + 1
    }

    unsafe extern "C" fn release<T: Object>(this: This) -> ULong {
        let left = unsafe { &*this.cast::<T>() }
            .refs()
            .fetch_sub(1, Ordering::AcqRel)
            - 1;
        if left == 0 {
            drop(unsafe { Box::from_raw(this.cast::<T>()) });
        }
        left as ULong
    }

    /// Answers only IUnknown (and `own`).
    unsafe fn qi_plain<T: Object>(this: This, iid: Iid, own: Iid, out: *mut This) -> HResult {
        if iid == IID_UNKNOWN || iid == own {
            unsafe { add_ref::<T>(this) };
            unsafe { *out = this };
            S_OK
        } else {
            unsafe { *out = ptr::null_mut() };
            E_NOINTERFACE
        }
    }

    macro_rules! object {
        ($t:ty) => {
            impl Object for $t {
                fn refs(&self) -> &AtomicUsize {
                    &self.head.refs
                }
            }
        };
    }

    fn new<T>(t: T) -> This {
        Box::into_raw(Box::new(t)).cast()
    }

    unsafe fn call_release(p: This) {
        if !p.is_null() {
            let u = unsafe { &**p.cast::<*const Unknown>() };
            unsafe { (u.release)(p) };
        }
    }

    unsafe fn call_add_ref(p: This) {
        let u = unsafe { &**p.cast::<*const Unknown>() };
        unsafe { (u.add_ref)(p) };
    }

    fn c_string(s: &str) -> *const c_char {
        let p = unsafe { libc::malloc(s.len() + 1) }.cast::<u8>();
        unsafe {
            ptr::copy_nonoverlapping(s.as_ptr(), p, s.len());
            *p.add(s.len()) = 0;
        }
        p.cast()
    }

    // ----- Factory

    #[repr(C)]
    struct FactoryVtbl {
        base: Unknown,
        create_codec: unsafe extern "C" fn(This, *mut This) -> HResult,
        _rest: [Slot; 3],
    }
    #[repr(C)]
    struct Factory {
        head: Head<FactoryVtbl>,
    }
    object!(Factory);

    unsafe extern "C" fn factory_qi(this: This, iid: Iid, out: *mut This) -> HResult {
        unsafe { qi_plain::<Factory>(this, iid, IID_UNKNOWN, out) }
    }

    unsafe extern "C" fn create_codec(_this: This, out: *mut This) -> HResult {
        let config = new(Config {
            head: Head {
                vtbl: &CONFIG_VTBL,
                refs: AtomicUsize::new(1),
            },
        });
        let codec = new(Codec {
            head: Head {
                vtbl: &CODEC_VTBL,
                refs: AtomicUsize::new(1),
            },
            config,
            callback: Mutex::new(0),
            queue: Mutex::new(VecDeque::new()),
        });
        CODEC.store(codec as usize, Ordering::Relaxed);
        unsafe { *out = codec };
        S_OK
    }

    static FACTORY_VTBL: FactoryVtbl = FactoryVtbl {
        base: Unknown {
            qi: factory_qi,
            add_ref: add_ref::<Factory>,
            release: release::<Factory>,
        },
        create_codec,
        _rest: [0; 3],
    };

    #[unsafe(no_mangle)]
    pub extern "C" fn CreateBlackmagicRawFactoryInstance() -> This {
        new(Factory {
            head: Head {
                vtbl: &FACTORY_VTBL,
                refs: AtomicUsize::new(1),
            },
        })
    }

    // ----- Codec

    #[repr(C)]
    struct CodecVtbl {
        base: Unknown,
        open_clip: unsafe extern "C" fn(This, *const c_char, *mut This) -> HResult,
        open_clip_with_geometry: Slot,
        open_clip_from_file: unsafe extern "C" fn(This, This, *mut This) -> HResult,
        open_clip_from_file_with_geometry: Slot,
        set_callback: unsafe extern "C" fn(This, This) -> HResult,
        prepare_pipeline: Slot,
        prepare_pipeline_for_device: Slot,
        flush_jobs: unsafe extern "C" fn(This) -> HResult,
    }

    enum Work {
        Read { job: This, clip: This, frame: u64 },
        Decode { job: This, frame: This },
    }
    // SAFETY: the pointers are mock objects with atomic counts, used by one thread at a time.
    unsafe impl Send for Work {}

    #[repr(C)]
    struct Codec {
        head: Head<CodecVtbl>,
        config: This,
        /// The callback, as an address (it is passed to the worker thread).
        callback: Mutex<usize>,
        queue: Mutex<VecDeque<Work>>,
    }
    object!(Codec);

    impl Drop for Codec {
        fn drop(&mut self) {
            unsafe {
                call_release(self.config);
                call_release(*self.callback.lock().unwrap() as This);
            }
        }
    }

    fn old_sdk() -> bool {
        std::env::var("SPECULAR_BRAW_MOCK").is_ok_and(|v| v == "old")
    }

    unsafe extern "C" fn codec_qi(this: This, iid: Iid, out: *mut This) -> HResult {
        let me = unsafe { &*this.cast::<Codec>() };
        if iid == IID_CONFIGURATION {
            unsafe { call_add_ref(me.config) };
            unsafe { *out = me.config };
            return S_OK;
        }
        if iid == IID_RAW && old_sdk() {
            unsafe { *out = ptr::null_mut() };
            return E_NOINTERFACE;
        }
        unsafe { qi_plain::<Codec>(this, iid, IID_RAW, out) }
    }

    unsafe extern "C" fn open_clip(_this: This, _path: *const c_char, _out: *mut This) -> HResult {
        // The host must never open a clip by path.
        std::process::abort()
    }

    #[repr(C)]
    struct IoVec {
        base: *mut c_void,
        len: u64,
    }

    /// The host's IBlackmagicRawFile, as far as used.
    #[repr(C)]
    struct FileVtbl {
        base: Unknown,
        get_filesystem: unsafe extern "C" fn(This, *mut This) -> HResult,
        get_file_name: unsafe extern "C" fn(This, *mut *const c_char) -> HResult,
        set_file_length: unsafe extern "C" fn(This, u64) -> HResult,
        get_file_length: unsafe extern "C" fn(This, *mut u64) -> HResult,
        read_v: unsafe extern "C" fn(This, *mut IoVec, u32, u64, *mut u64) -> HResult,
        write_v: unsafe extern "C" fn(This, *mut IoVec, u32, u64, *mut u64) -> HResult,
    }

    #[repr(C)]
    struct FsVtbl {
        base: Unknown,
        open_companion_file: unsafe extern "C" fn(This, This, *const c_char, *mut This) -> HResult,
    }

    fn read(file: This, at: u64, n: usize) -> Option<Vec<u8>> {
        let vt = unsafe { &**file.cast::<*const FileVtbl>() };
        // Two buffers, to exercise the gather list.
        let half = n / 2;
        let mut a = vec![0u8; half];
        let mut b = vec![0u8; n - half];
        let mut v = [
            IoVec {
                base: a.as_mut_ptr().cast(),
                len: a.len() as u64,
            },
            IoVec {
                base: b.as_mut_ptr().cast(),
                len: b.len() as u64,
            },
        ];
        let mut got = 0u64;
        let h = unsafe { (vt.read_v)(file, v.as_mut_ptr(), 2, at, &mut got) };
        if h != S_OK || got != n as u64 {
            return None;
        }
        a.extend_from_slice(&b);
        Some(a)
    }

    unsafe extern "C" fn open_clip_from_file(_this: This, file: This, out: *mut This) -> HResult {
        unsafe { *out = ptr::null_mut() };
        if file.is_null() {
            return E_FAIL;
        }
        let vt = unsafe { &**file.cast::<*const FileVtbl>() };
        let mut len = 0u64;
        if unsafe { (vt.get_file_length)(file, &mut len) } != S_OK || len < 32 {
            return E_FAIL;
        }
        // The host's file system has no sidecar, and the file is read-only.
        let mut fs: This = ptr::null_mut();
        if unsafe { (vt.get_filesystem)(file, &mut fs) } != S_OK || fs.is_null() {
            return E_FAIL;
        }
        let fvt = unsafe { &**fs.cast::<*const FsVtbl>() };
        let mut side: This = ptr::null_mut();
        let name = c"clip.sidecar";
        let h = unsafe { (fvt.open_companion_file)(fs, file, name.as_ptr(), &mut side) };
        unsafe { call_release(fs) };
        if h >= 0 || !side.is_null() {
            return E_FAIL;
        }
        let mut written = 1u64;
        let mut byte = [0u8; 1];
        let mut v = IoVec {
            base: byte.as_mut_ptr().cast(),
            len: 1,
        };
        if unsafe { (vt.write_v)(file, &mut v, 1, 0, &mut written) } >= 0 || written != 0 {
            return E_FAIL;
        }
        if unsafe { (vt.set_file_length)(file, 0) } >= 0 {
            return E_FAIL;
        }
        let mut fname: *const c_char = ptr::null();
        if unsafe { (vt.get_file_name)(file, &mut fname) } != S_OK || fname.is_null() {
            return E_FAIL;
        }
        unsafe { libc::free(fname.cast_mut().cast()) };
        let Some(h) = read(file, 0, 32) else {
            return E_FAIL;
        };
        if &h[..8] != b"MOCKBRAW" {
            return E_FAIL;
        }
        let u32_at = |i: usize| u32::from_le_bytes([h[i], h[i + 1], h[i + 2], h[i + 3]]);
        let width = u32_at(8);
        let height = u32_at(12);
        let frames = u64::from_le_bytes(h[16..24].try_into().unwrap());
        let rate = f32::from_bits(u32_at(24));
        let n = u32_at(28) as usize;
        let Some(name) = read(file, 32, n.min(1024)) else {
            return E_FAIL;
        };
        let mut camera = String::from_utf8_lossy(&name).into_owned();
        if std::fs::File::open("/etc/hostname").is_ok() {
            camera.push_str(" [opened a file]");
        }
        unsafe { call_add_ref(file) };
        let clip = new(Clip {
            head: Head {
                vtbl: &CLIP_VTBL,
                refs: AtomicUsize::new(1),
            },
            file,
            width,
            height,
            frames,
            rate,
            camera,
        });
        unsafe { *out = clip };
        S_OK
    }

    unsafe extern "C" fn set_callback(this: This, cb: This) -> HResult {
        let me = unsafe { &*this.cast::<Codec>() };
        if !cb.is_null() {
            unsafe { call_add_ref(cb) };
        }
        let old = std::mem::replace(&mut *me.callback.lock().unwrap(), cb as usize);
        unsafe { call_release(old as This) };
        S_OK
    }

    /// The host's IBlackmagicRawCallback, as far as used.
    #[repr(C)]
    struct CallbackVtbl {
        base: Unknown,
        read_complete: unsafe extern "C" fn(This, This, HResult, This),
        read_audio_complete: Slot,
        decode_complete: Slot,
        process_complete: unsafe extern "C" fn(This, This, HResult, This),
    }

    unsafe extern "C" fn flush_jobs(this: This) -> HResult {
        let addr = this as usize;
        // On a worker thread of the library, as the SDK runs its callbacks.
        #[allow(clippy::disallowed_methods)]
        let worker = std::thread::spawn(move || {
            let me = unsafe { &*(addr as This).cast::<Codec>() };
            loop {
                let Some(w) = me.queue.lock().unwrap().pop_front() else {
                    break;
                };
                let cb = *me.callback.lock().unwrap() as This;
                if cb.is_null() {
                    continue;
                }
                let vt = unsafe { &**cb.cast::<*const CallbackVtbl>() };
                match w {
                    Work::Read { job, clip, frame } => {
                        let c = unsafe { &*clip.cast::<Clip>() };
                        if frame >= c.frames {
                            unsafe { (vt.read_complete)(cb, job, E_FAIL, ptr::null_mut()) };
                        } else {
                            let f = new(Frame {
                                head: Head {
                                    vtbl: &FRAME_VTBL,
                                    refs: AtomicUsize::new(1),
                                },
                                codec: addr,
                                index: frame,
                                width: c.width,
                                height: c.height,
                                scale: Mutex::new(cc(b"full")),
                                format: Mutex::new(0),
                            });
                            unsafe { (vt.read_complete)(cb, job, S_OK, f) };
                            unsafe { call_release(f) };
                        }
                        unsafe { call_release(clip) };
                    }
                    Work::Decode { job, frame } => {
                        let f = unsafe { &*frame.cast::<Frame>() };
                        let image = f.process();
                        let (h, img) = match image {
                            Some(i) => (S_OK, new(i)),
                            None => (E_FAIL, ptr::null_mut()),
                        };
                        unsafe { (vt.process_complete)(cb, job, h, img) };
                        unsafe {
                            call_release(img);
                            call_release(frame);
                        }
                    }
                }
            }
        });
        if worker.join().is_ok() { S_OK } else { E_FAIL }
    }

    static CODEC_VTBL: CodecVtbl = CodecVtbl {
        base: Unknown {
            qi: codec_qi,
            add_ref: add_ref::<Codec>,
            release: release::<Codec>,
        },
        open_clip,
        open_clip_with_geometry: 0,
        open_clip_from_file,
        open_clip_from_file_with_geometry: 0,
        set_callback,
        prepare_pipeline: 0,
        prepare_pipeline_for_device: 0,
        flush_jobs,
    };

    // ----- Configuration

    #[repr(C)]
    struct ConfigVtbl {
        base: Unknown,
        set_pipeline: unsafe extern "C" fn(This, u32, This, This) -> HResult,
        get_pipeline: Slot,
        is_pipeline_supported: Slot,
        set_cpu_threads: unsafe extern "C" fn(This, u32) -> HResult,
        get_cpu_threads: Slot,
        get_max_cpu_thread_count: Slot,
        set_write_metadata_per_frame: Slot,
        get_write_metadata_per_frame: Slot,
        set_from_device: Slot,
        get_version: unsafe extern "C" fn(This, *mut *const c_char) -> HResult,
        get_camera_support_version: unsafe extern "C" fn(This, *mut *const c_char) -> HResult,
    }
    #[repr(C)]
    struct Config {
        head: Head<ConfigVtbl>,
    }
    object!(Config);

    unsafe extern "C" fn config_qi(this: This, iid: Iid, out: *mut This) -> HResult {
        unsafe { qi_plain::<Config>(this, iid, IID_CONFIGURATION, out) }
    }
    unsafe extern "C" fn set_pipeline(_this: This, p: u32, ctx: This, q: This) -> HResult {
        if p == cc(b"cpub") && ctx.is_null() && q.is_null() {
            S_OK
        } else {
            E_FAIL
        }
    }
    unsafe extern "C" fn set_cpu_threads(_this: This, n: u32) -> HResult {
        if n > 0 { S_OK } else { E_FAIL }
    }
    unsafe extern "C" fn get_version(_this: This, out: *mut *const c_char) -> HResult {
        unsafe { *out = c_string("0.0") };
        S_OK
    }
    unsafe extern "C" fn get_camera_support_version(
        _this: This,
        out: *mut *const c_char,
    ) -> HResult {
        unsafe { *out = c_string("6.0") };
        S_OK
    }

    static CONFIG_VTBL: ConfigVtbl = ConfigVtbl {
        base: Unknown {
            qi: config_qi,
            add_ref: add_ref::<Config>,
            release: release::<Config>,
        },
        set_pipeline,
        get_pipeline: 0,
        is_pipeline_supported: 0,
        set_cpu_threads,
        get_cpu_threads: 0,
        get_max_cpu_thread_count: 0,
        set_write_metadata_per_frame: 0,
        get_write_metadata_per_frame: 0,
        set_from_device: 0,
        get_version,
        get_camera_support_version,
    };

    // ----- Clip

    #[repr(C)]
    struct ClipVtbl {
        base: Unknown,
        get_width: unsafe extern "C" fn(This, *mut u32) -> HResult,
        get_height: unsafe extern "C" fn(This, *mut u32) -> HResult,
        get_frame_rate: unsafe extern "C" fn(This, *mut f32) -> HResult,
        get_frame_count: unsafe extern "C" fn(This, *mut u64) -> HResult,
        _a: [Slot; 4],
        get_camera_type: unsafe extern "C" fn(This, *mut *const c_char) -> HResult,
        _b: [Slot; 6],
        create_job_read_frame: unsafe extern "C" fn(This, u64, *mut This) -> HResult,
        _c: [Slot; 4],
    }
    #[repr(C)]
    struct Clip {
        head: Head<ClipVtbl>,
        file: This,
        width: u32,
        height: u32,
        frames: u64,
        rate: f32,
        camera: String,
    }
    object!(Clip);

    impl Drop for Clip {
        fn drop(&mut self) {
            unsafe { call_release(self.file) };
        }
    }

    fn clip<'a>(this: This) -> &'a Clip {
        unsafe { &*this.cast::<Clip>() }
    }
    unsafe extern "C" fn clip_qi(this: This, iid: Iid, out: *mut This) -> HResult {
        unsafe { qi_plain::<Clip>(this, iid, IID_UNKNOWN, out) }
    }
    unsafe extern "C" fn clip_width(this: This, out: *mut u32) -> HResult {
        unsafe { *out = clip(this).width };
        S_OK
    }
    unsafe extern "C" fn clip_height(this: This, out: *mut u32) -> HResult {
        unsafe { *out = clip(this).height };
        S_OK
    }
    unsafe extern "C" fn clip_rate(this: This, out: *mut f32) -> HResult {
        unsafe { *out = clip(this).rate };
        S_OK
    }
    unsafe extern "C" fn clip_frames(this: This, out: *mut u64) -> HResult {
        unsafe { *out = clip(this).frames };
        S_OK
    }
    unsafe extern "C" fn clip_camera(this: This, out: *mut *const c_char) -> HResult {
        unsafe { *out = c_string(&clip(this).camera) };
        S_OK
    }
    unsafe extern "C" fn clip_read(this: This, frame: u64, out: *mut This) -> HResult {
        unsafe { call_add_ref(this) };
        let job = new(Job {
            head: Head {
                vtbl: &JOB_VTBL,
                refs: AtomicUsize::new(1),
            },
            work: Mutex::new(Some(WorkSpec::Read { clip: this, frame })),
            codec: 0,
        });
        unsafe { *out = job };
        S_OK
    }

    static CLIP_VTBL: ClipVtbl = ClipVtbl {
        base: Unknown {
            qi: clip_qi,
            add_ref: add_ref::<Clip>,
            release: release::<Clip>,
        },
        get_width: clip_width,
        get_height: clip_height,
        get_frame_rate: clip_rate,
        get_frame_count: clip_frames,
        _a: [0; 4],
        get_camera_type: clip_camera,
        _b: [0; 6],
        create_job_read_frame: clip_read,
        _c: [0; 4],
    };

    // ----- Jobs

    enum WorkSpec {
        Read { clip: This, frame: u64 },
        Decode { frame: This },
    }

    #[repr(C)]
    struct JobVtbl {
        base: Unknown,
        submit: unsafe extern "C" fn(This) -> HResult,
        abort: unsafe extern "C" fn(This) -> HResult,
        _rest: [Slot; 2],
    }
    #[repr(C)]
    struct Job {
        head: Head<JobVtbl>,
        work: Mutex<Option<WorkSpec>>,
        /// The codec whose queue a decode job goes to (read jobs find it via the frame).
        codec: usize,
    }
    object!(Job);

    impl Drop for Job {
        fn drop(&mut self) {
            // Never submitted: what it holds goes with it.
            match self.work.lock().unwrap().take() {
                Some(WorkSpec::Read { clip, .. }) => unsafe { call_release(clip) },
                Some(WorkSpec::Decode { frame }) => unsafe { call_release(frame) },
                None => {}
            }
        }
    }

    /// The codec the clip's jobs run on: the mock keeps one per process (the host makes one).
    static CODEC: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn job_qi(this: This, iid: Iid, out: *mut This) -> HResult {
        unsafe { qi_plain::<Job>(this, iid, IID_UNKNOWN, out) }
    }
    unsafe extern "C" fn submit(this: This) -> HResult {
        let me = unsafe { &*this.cast::<Job>() };
        let Some(spec) = me.work.lock().unwrap().take() else {
            return E_FAIL;
        };
        let codec = if me.codec != 0 {
            me.codec
        } else {
            CODEC.load(Ordering::Relaxed)
        };
        if codec == 0 {
            return E_FAIL;
        }
        let c = unsafe { &*(codec as This).cast::<Codec>() };
        let w = match spec {
            WorkSpec::Read { clip, frame } => Work::Read {
                job: this,
                clip,
                frame,
            },
            WorkSpec::Decode { frame } => Work::Decode { job: this, frame },
        };
        c.queue.lock().unwrap().push_back(w);
        S_OK
    }
    unsafe extern "C" fn abort(_this: This) -> HResult {
        E_NOTIMPL
    }

    static JOB_VTBL: JobVtbl = JobVtbl {
        base: Unknown {
            qi: job_qi,
            add_ref: add_ref::<Job>,
            release: release::<Job>,
        },
        submit,
        abort,
        _rest: [0; 2],
    };

    // ----- Frame and processed image

    #[repr(C)]
    struct FrameVtbl {
        base: Unknown,
        _a: [Slot; 6],
        set_resolution_scale: unsafe extern "C" fn(This, u32) -> HResult,
        _b: Slot,
        set_resource_format: unsafe extern "C" fn(This, u32) -> HResult,
        _c: [Slot; 2],
        create_job_decode_and_process_frame:
            unsafe extern "C" fn(This, This, This, *mut This) -> HResult,
    }
    #[repr(C)]
    struct Frame {
        head: Head<FrameVtbl>,
        codec: usize,
        index: u64,
        width: u32,
        height: u32,
        scale: Mutex<u32>,
        format: Mutex<u32>,
    }
    object!(Frame);

    impl Frame {
        fn process(&self) -> Option<Image> {
            if *self.format.lock().unwrap() != cc(b"rgba") {
                return None;
            }
            let s = match *self.scale.lock().unwrap() {
                x if x == cc(b"half") => 2,
                x if x == cc(b"qrtr") => 4,
                x if x == cc(b"eith") => 8,
                _ => 1,
            };
            let (w, h) = ((self.width / s).max(1), (self.height / s).max(1));
            let mut px = Vec::with_capacity(w as usize * h as usize * 4);
            for y in 0..h {
                for x in 0..w {
                    px.extend_from_slice(&[(x * s) as u8, (y * s) as u8, self.index as u8, 255]);
                }
            }
            Some(Image {
                head: Head {
                    vtbl: &IMAGE_VTBL,
                    refs: AtomicUsize::new(1),
                },
                width: w,
                height: h,
                px,
            })
        }
    }

    unsafe extern "C" fn frame_qi(this: This, iid: Iid, out: *mut This) -> HResult {
        unsafe { qi_plain::<Frame>(this, iid, IID_UNKNOWN, out) }
    }
    unsafe extern "C" fn frame_scale(this: This, s: u32) -> HResult {
        let ok = [cc(b"full"), cc(b"half"), cc(b"qrtr"), cc(b"eith")].contains(&s);
        if !ok {
            return E_FAIL;
        }
        *unsafe { &*this.cast::<Frame>() }.scale.lock().unwrap() = s;
        S_OK
    }
    unsafe extern "C" fn frame_format(this: This, f: u32) -> HResult {
        *unsafe { &*this.cast::<Frame>() }.format.lock().unwrap() = f;
        S_OK
    }
    unsafe extern "C" fn frame_decode(this: This, a: This, b: This, out: *mut This) -> HResult {
        if !a.is_null() || !b.is_null() {
            return E_NOTIMPL;
        }
        let me = unsafe { &*this.cast::<Frame>() };
        unsafe { call_add_ref(this) };
        let job = new(Job {
            head: Head {
                vtbl: &JOB_VTBL,
                refs: AtomicUsize::new(1),
            },
            work: Mutex::new(Some(WorkSpec::Decode { frame: this })),
            codec: me.codec,
        });
        unsafe { *out = job };
        S_OK
    }

    static FRAME_VTBL: FrameVtbl = FrameVtbl {
        base: Unknown {
            qi: frame_qi,
            add_ref: add_ref::<Frame>,
            release: release::<Frame>,
        },
        _a: [0; 6],
        set_resolution_scale: frame_scale,
        _b: 0,
        set_resource_format: frame_format,
        _c: [0; 2],
        create_job_decode_and_process_frame: frame_decode,
    };

    #[repr(C)]
    struct ImageVtbl {
        base: Unknown,
        get_width: unsafe extern "C" fn(This, *mut u32) -> HResult,
        get_height: unsafe extern "C" fn(This, *mut u32) -> HResult,
        get_resource: unsafe extern "C" fn(This, *mut *mut c_void) -> HResult,
        get_resource_type: unsafe extern "C" fn(This, *mut u32) -> HResult,
        get_resource_format: unsafe extern "C" fn(This, *mut u32) -> HResult,
        get_resource_size_bytes: unsafe extern "C" fn(This, *mut u32) -> HResult,
    }
    #[repr(C)]
    struct Image {
        head: Head<ImageVtbl>,
        width: u32,
        height: u32,
        px: Vec<u8>,
    }
    object!(Image);

    fn image<'a>(this: This) -> &'a Image {
        unsafe { &*this.cast::<Image>() }
    }
    unsafe extern "C" fn image_qi(this: This, iid: Iid, out: *mut This) -> HResult {
        unsafe { qi_plain::<Image>(this, iid, IID_UNKNOWN, out) }
    }
    unsafe extern "C" fn image_width(this: This, out: *mut u32) -> HResult {
        unsafe { *out = image(this).width };
        S_OK
    }
    unsafe extern "C" fn image_height(this: This, out: *mut u32) -> HResult {
        unsafe { *out = image(this).height };
        S_OK
    }
    unsafe extern "C" fn image_resource(this: This, out: *mut *mut c_void) -> HResult {
        unsafe { *out = image(this).px.as_ptr().cast_mut().cast() };
        S_OK
    }
    unsafe extern "C" fn image_type(_this: This, out: *mut u32) -> HResult {
        unsafe { *out = cc(b"cpub") };
        S_OK
    }
    unsafe extern "C" fn image_format(_this: This, out: *mut u32) -> HResult {
        unsafe { *out = cc(b"rgba") };
        S_OK
    }
    unsafe extern "C" fn image_size(this: This, out: *mut u32) -> HResult {
        unsafe { *out = image(this).px.len() as u32 };
        S_OK
    }

    static IMAGE_VTBL: ImageVtbl = ImageVtbl {
        base: Unknown {
            qi: image_qi,
            add_ref: add_ref::<Image>,
            release: release::<Image>,
        },
        get_width: image_width,
        get_height: image_height,
        get_resource: image_resource,
        get_resource_type: image_type,
        get_resource_format: image_format,
        get_resource_size_bytes: image_size,
    };
}
