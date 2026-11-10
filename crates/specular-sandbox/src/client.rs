//! The viewer side: a small pool of host processes, spawned lazily, restarted after a crash,
//! killed on timeout. A failed host call is an ordinary decode error for the caller.

use crate::protocol::{
    CadFormat, CadLimits, ClipInfo, CziPlane, CziPlaneKind, CziRegion, ErrorKind, HostInfo,
    ImageLimits, ImageReply, Input, ListEntry, Listing, Payload, Request, Response, Secret, TestOp,
};
use crate::sql::{SqlLimits, SqlRows, SqlValue};
use parking_lot::{Mutex, MutexGuard};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// File name of the host binary (next to the viewer).
pub const HOST_EXE: &str = "specular-decode-host";

/// Why a host call failed. Every variant is a decode error for the viewer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The host binary (or a library it needs) is not installed.
    #[error("{0}")]
    NotInstalled(String),
    /// The host cannot run here: unsupported platform, spawn failure, no sandbox.
    #[error("decode host unavailable: {0}")]
    Unavailable(String),
    /// The host died while handling the request (it restarts on the next call).
    #[error("decode host crashed: {0}")]
    Crashed(String),
    #[error("decode host timed out after {0:?}")]
    Timeout(Duration),
    #[error("decode host protocol error: {0}")]
    Protocol(String),
    /// The host handled the request and reported an error (malformed file, …).
    #[error("{message}")]
    Remote { kind: ErrorKind, message: String },
    #[error("i/o: {0}")]
    Io(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Host process settings.
#[derive(Debug, Clone)]
pub struct Config {
    /// Host binary; `None` searches `SPECULAR_DECODE_HOST`, then next to the running executable.
    pub exe: Option<PathBuf>,
    /// Longest a request may take before the host is killed.
    pub timeout: Duration,
    /// Longest a start (spawn, library loading, sandbox, hello) may take.
    pub start_timeout: Duration,
    /// Address-space limit of each host process, bytes.
    pub memory_limit: u64,
    /// Most host processes running at once.
    pub pool_size: usize,
    /// Refuse a host that could not confine itself at all.
    pub require_sandbox: bool,
    /// Start hosts with `--test-hooks` (enables [`TestOp`]).
    pub test_hooks: bool,
    /// Start hosts with `--no-sandbox` (needs `test_hooks`; for tests that check the probes).
    pub disable_sandbox: bool,
    /// Extra environment for the host (e.g. `SPECULAR_LIBHEIF`), on top of the passed-through
    /// variables.
    pub env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    /// Start hosts with `--occt`: they load OpenCASCADE for [`Host::cad_read`] (slower to
    /// start, so CAD files get a pool of their own).
    pub occt: bool,
    /// Start hosts with `--czi`: they load libCZI for the CZI requests (Zeiss microscopy
    /// files get a pool of their own).
    pub czi: bool,
    /// Vendor libraries the user installed, loaded by the host at start (changed later with
    /// [`Host::set_vendor_libs`]).
    pub vendor: VendorLibs,
}

/// Vendor decoder libraries a host loads at start, before it confines itself. They are the
/// user's own installs, found on this computer or picked by hand: never bundled or downloaded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VendorLibs {
    /// The Blackmagic RAW SDK: its `BlackmagicRawAPI` library (file) or the folder (or macOS
    /// framework) holding it.
    pub braw: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            exe: None,
            timeout: Duration::from_secs(30),
            start_timeout: Duration::from_secs(10),
            memory_limit: 4 << 30,
            pool_size: 2,
            require_sandbox: true,
            test_hooks: false,
            disable_sandbox: false,
            env: Vec::new(),
            occt: false,
            vendor: VendorLibs::default(),
            czi: false,
        }
    }
}

impl Config {
    /// Defaults with environment overrides: `SPECULAR_DECODE_HOST` (binary path),
    /// `SPECULAR_HOST_TIMEOUT_MS`, `SPECULAR_HOST_MEMORY_MB`, `SPECULAR_HOST_POOL`,
    /// `SPECULAR_HOST_ALLOW_UNSANDBOXED=1` (accept a host without OS confinement, e.g. on
    /// platforms whose sandbox is not implemented yet).
    pub fn from_env() -> Config {
        let num = |k: &str| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
        };
        let mut c = Config::default();
        if let Some(ms) = num("SPECULAR_HOST_TIMEOUT_MS") {
            c.timeout = Duration::from_millis(ms.max(1));
        }
        if let Some(mb) = num("SPECULAR_HOST_MEMORY_MB") {
            c.memory_limit = mb.max(64) << 20;
        }
        if let Some(n) = num("SPECULAR_HOST_POOL") {
            c.pool_size = n.clamp(1, 16) as usize;
        }
        if std::env::var_os("SPECULAR_HOST_ALLOW_UNSANDBOXED").is_some_and(|v| v == "1") {
            c.require_sandbox = false;
        }
        c
    }
}

/// A decoded image: `width × height` RGBA8, tightly packed, straight alpha, transformations
/// (rotation, mirroring, crop) already applied.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    /// Bit depth of the coded image.
    pub bit_depth: u8,
    pub icc: Vec<u8>,
    pub rgba: Vec<u8>,
}

/// Bytes for one host call (a HEIF file): a sealed `memfd` on Linux, inline elsewhere.
pub struct SharedInput {
    #[cfg(unix)]
    file: Option<std::fs::File>,
    inline: Option<Vec<u8>>,
    len: u64,
}

impl std::fmt::Debug for SharedInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedInput")
            .field("len", &self.len)
            .finish()
    }
}

impl SharedInput {
    /// Shared memory where the platform has it, else inline.
    pub fn new(bytes: &[u8]) -> Result<SharedInput> {
        #[cfg(unix)]
        if crate::transport::shm::AVAILABLE {
            return Ok(SharedInput {
                file: Some(crate::transport::shm::from_bytes("specular-input", bytes)?),
                inline: None,
                len: bytes.len() as u64,
            });
        }
        Ok(Self::inline(bytes.to_vec()))
    }

    fn inline(bytes: Vec<u8>) -> SharedInput {
        SharedInput {
            #[cfg(unix)]
            file: None,
            len: bytes.len() as u64,
            inline: Some(bytes),
        }
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn input(&self) -> Input {
        match &self.inline {
            Some(b) => Input::Inline(b.clone()),
            None => Input::Fd,
        }
    }
}

/// Finds another volume of a multi-volume archive by its synthetic name.
pub type VolumeProvider = dyn Fn(&str) -> Option<std::fs::File> + Send + Sync;

/// A RAR archive for the host: an open, read-only file the viewer passes to the host (the host
/// cannot open files), and how to find the archive's other volumes.
pub struct RarSource {
    file: std::fs::File,
    /// The synthetic name UnRAR knows the first volume by ([`crate::protocol::volume_name`]).
    name: String,
    len: u64,
    volumes: Option<Box<VolumeProvider>>,
}

impl std::fmt::Debug for RarSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RarSource")
            .field("name", &self.name)
            .field("len", &self.len)
            .field("volumes", &self.volumes.is_some())
            .finish()
    }
}

impl RarSource {
    /// The archive at `path`, opened read-only; further volumes are looked up next to it
    /// ([`resolve_volume`]).
    pub fn open(path: &Path) -> Result<RarSource> {
        let file = open_regular(path)?;
        let real = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = crate::protocol::volume_name(&real);
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let synthetic = name.clone();
        let provider = move |want: &str| {
            let p = resolve_volume(&dir, &real, &synthetic, want)?;
            open_regular(&p)
                .inspect_err(|e| specular_log::debug!(target: specular_log::DECODE, "volume {}: {e}", p.display()))
                .ok()
        };
        Self::with_volumes(file, &name, Some(Box::new(provider)))
    }

    /// A private copy of `bytes` (an archive inside another archive, or not on disk): a sealed
    /// `memfd` (Linux), an unlinked temporary file (other Unixes) or a delete-on-close
    /// temporary file (Windows). It has no other volumes.
    pub fn from_bytes(bytes: &[u8]) -> Result<RarSource> {
        let file = private_copy(bytes)?;
        Self::with_volumes(file, "archive.rar", None)
    }

    /// `file` (the first volume, readable), known to UnRAR as `name`; `volumes` finds the
    /// others by the names UnRAR asks for.
    pub fn with_volumes(
        file: std::fs::File,
        name: &str,
        volumes: Option<Box<VolumeProvider>>,
    ) -> Result<RarSource> {
        if !crate::protocol::valid_volume_name(name) {
            return Err(Error::Io(format!("bad volume name {name:?}")));
        }
        let len = file.metadata()?.len();
        Ok(RarSource {
            file,
            name: name.to_owned(),
            len,
            volumes,
        })
    }

    /// Size of the first volume.
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the first volume holds exactly `bytes` (the file did not change since the
    /// caller read it). Compares the length and up to 64 KiB at each end.
    pub fn same_content(&self, bytes: &[u8]) -> bool {
        const EDGE: usize = 64 << 10;
        if self.len != bytes.len() as u64 {
            return false;
        }
        let head = bytes.len().min(EDGE);
        let tail = bytes.len() - bytes.len().min(EDGE);
        read_at(&self.file, 0, head).is_some_and(|b| b == bytes[..head])
            && read_at(&self.file, tail as u64, bytes.len() - tail)
                .is_some_and(|b| b == bytes[tail..])
    }
}

/// A video clip for a vendor SDK in the host: an open, read-only file the viewer passes (the
/// host cannot open files; the SDK reads through the host's file object).
#[derive(Debug)]
pub struct ClipSource {
    file: std::fs::File,
    len: u64,
}

impl ClipSource {
    /// The clip at `path`, opened read-only.
    pub fn open(path: &Path) -> Result<ClipSource> {
        let file = open_regular(path)?;
        let len = file.metadata()?.len();
        Ok(ClipSource { file, len })
    }

    /// A private copy of `bytes` (a clip that is not on disk), as for [`RarSource::from_bytes`].
    pub fn from_bytes(bytes: &[u8]) -> Result<ClipSource> {
        let file = private_copy(bytes)?;
        Ok(ClipSource {
            file,
            len: bytes.len() as u64,
        })
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A Zeiss CZI file for the host: opened read-only by the viewer and passed with every
/// request (the host cannot open files). `key` names the document to the host, which keeps a
/// few open between requests.
pub struct CziSource {
    file: std::fs::File,
    len: u64,
    key: u64,
}

impl std::fmt::Debug for CziSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CziSource")
            .field("len", &self.len)
            .field("key", &self.key)
            .finish()
    }
}

impl CziSource {
    /// The file at `path`, opened read-only.
    pub fn open(path: &Path) -> Result<CziSource> {
        Self::new(open_regular(path)?)
    }

    /// A private copy of `bytes` (a file inside an archive, or not on disk).
    pub fn from_bytes(bytes: &[u8]) -> Result<CziSource> {
        Self::new(private_copy(bytes)?)
    }

    fn new(file: std::fs::File) -> Result<CziSource> {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        // Unique within this viewer: the process id in the high bits, a counter below.
        let key = (u64::from(std::process::id()) << 32) | SEQ.fetch_add(1, Ordering::Relaxed);
        let len = file.metadata()?.len();
        Ok(CziSource { file, len, key })
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The bytes at `at` (up to `n`, fewer at the end of the file).
    pub fn read_at(&self, at: u64, n: usize) -> Option<Vec<u8>> {
        let n = n.min(usize::try_from(self.len.saturating_sub(at)).unwrap_or(usize::MAX));
        read_at(&self.file, at, n)
    }
}

/// An SQLite database (GeoPackage, MBTiles) for the host: opened read-only by the viewer and
/// passed with every query (the host cannot open files). `key` names the database to the host,
/// which keeps a few connections open between queries.
pub struct SqlSource {
    file: std::fs::File,
    len: u64,
    key: u64,
    /// The database's `-wal` file beside it, when there is one (the host asks for it).
    wal: Option<std::path::PathBuf>,
}

impl std::fmt::Debug for SqlSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlSource")
            .field("len", &self.len)
            .field("key", &self.key)
            .finish()
    }
}

impl SqlSource {
    /// The database at `path`, opened read-only. Its `-wal` file, if any, is handed to the
    /// host too (read-only), which reads the transactions committed there.
    pub fn open(path: &Path) -> Result<SqlSource> {
        let mut s = Self::new(open_regular(path)?)?;
        let mut wal = path.as_os_str().to_owned();
        wal.push("-wal");
        let wal = std::path::PathBuf::from(wal);
        s.wal = wal.is_file().then_some(wal);
        Ok(s)
    }

    /// A private copy of `bytes` (a database inside an archive, or not on disk).
    pub fn from_bytes(bytes: &[u8]) -> Result<SqlSource> {
        Self::new(private_copy(bytes)?)
    }

    fn new(file: std::fs::File) -> Result<SqlSource> {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        // As for CZI: the process id in the high bits, a counter below.
        let key = (u64::from(std::process::id()) << 32) | SEQ.fetch_add(1, Ordering::Relaxed);
        let len = file.metadata()?.len();
        Ok(SqlSource {
            file,
            len,
            key,
            wal: None,
        })
    }

    /// Whether the database has a `-wal` file beside it.
    pub fn has_wal(&self) -> bool {
        self.wal.is_some()
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Positioned read of exactly `n` bytes (the file's shared offset is not touched).
fn read_at(f: &std::fs::File, at: u64, n: usize) -> Option<Vec<u8>> {
    let mut b = vec![0u8; n];
    let mut done = 0;
    while done < n {
        #[cfg(unix)]
        let r = std::os::unix::fs::FileExt::read_at(f, &mut b[done..], at + done as u64);
        #[cfg(windows)]
        let r = std::os::windows::fs::FileExt::seek_read(f, &mut b[done..], at + done as u64);
        #[cfg(not(any(unix, windows)))]
        let r: std::io::Result<usize> = {
            let _ = (f, at);
            Err(std::io::ErrorKind::Unsupported.into())
        };
        match r {
            Ok(0) | Err(_) => return None,
            Ok(k) => done += k,
        }
    }
    Some(b)
}

/// Opens `p` read-only and checks that it is a regular file.
fn open_regular(p: &Path) -> Result<std::fs::File> {
    let f = std::fs::File::open(p)?;
    if !f.metadata()?.is_file() {
        return Err(Error::Io(format!("{} is not a regular file", p.display())));
    }
    Ok(f)
}

/// Maps a volume name UnRAR asked for (`want`, derived from the synthetic name `synthetic` of the
/// real file `real` in `dir`) back to a real path, or refuses it.
///
/// Only names of RAR volumes are honoured: a valid synthetic name other than the first volume's,
/// ending in `.rar`/`.cbr` or a volume-number extension (`.r00`, `.s01`, `.001`). The synthetic
/// name maps character for character to the real one, so the real name is the real file's
/// characters up to where the names first differ, then the requested tail (plain ASCII). The
/// result is always a file in `dir`: no separators can appear.
pub fn resolve_volume(dir: &Path, real: &str, synthetic: &str, want: &str) -> Option<PathBuf> {
    if !crate::protocol::valid_volume_name(want) || want == synthetic {
        return None;
    }
    // Only another volume of the same set: same name apart from the volume part.
    if volume_stem(want)? != volume_stem(synthetic)? {
        return None;
    }
    let real: Vec<char> = real.chars().collect();
    let syn: Vec<char> = synthetic.chars().collect();
    // `synthetic` may be a cut-down tail of `real` (very long names): align the ends.
    let skip = real.len().checked_sub(syn.len())?;
    let want_c: Vec<char> = want.chars().collect();
    let p = syn.iter().zip(&want_c).take_while(|(a, b)| a == b).count();
    let name: String = real[..skip + p].iter().chain(&want_c[p..]).collect();
    if name.contains(['/', '\\']) || name == "." || name == ".." {
        return None;
    }
    Some(dir.join(name))
}

/// A volume name without its volume part: `x.part2.rar`, `x.rar`, `x.cbr`, `x.r00`, `x.s01` and
/// `x.001` all give `x`. `None` for names that are no RAR volume.
fn volume_stem(name: &str) -> Option<&str> {
    let (base, ext) = name.rsplit_once('.')?;
    let ext = ext.to_ascii_lowercase();
    if ext == "rar" || ext == "cbr" {
        // An optional `.partN` before the extension.
        if let Some((stem, part)) = base.rsplit_once('.')
            && part.len() > 4
            && part[..4].eq_ignore_ascii_case("part")
            && part[4..].bytes().all(|b| b.is_ascii_digit())
        {
            return Some(stem);
        }
        return Some(base);
    }
    let b = ext.as_bytes();
    let numbered = b.len() >= 2
        && (b[0].is_ascii_alphabetic() || b[0].is_ascii_digit())
        && b[1..].iter().all(u8::is_ascii_digit);
    numbered.then_some(base)
}

/// A private, read-only-to-the-host copy of `bytes`.
fn private_copy(bytes: &[u8]) -> Result<std::fs::File> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        Ok(crate::transport::shm::from_bytes(
            "specular-archive",
            bytes,
        )?)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        Ok(temp_copy(bytes)?)
    }
}

/// A private copy of `bytes` in a temporary file nobody else can open: unlinked at once (Unix)
/// or deleted when the last handle closes and shared with nobody but readers of this handle
/// (Windows).
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn temp_copy(bytes: &[u8]) -> std::io::Result<std::fs::File> {
    use std::io::Write;
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    for _ in 0..16 {
        let p = dir.join(format!(
            "specular-input-{}-{}-{nanos:08x}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let mut o = std::fs::OpenOptions::new();
        o.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            const GENERIC_READ: u32 = 0x8000_0000;
            const GENERIC_WRITE: u32 = 0x4000_0000;
            const DELETE: u32 = 0x0001_0000;
            const FILE_SHARE_READ: u32 = 1;
            const FILE_SHARE_DELETE: u32 = 4;
            const FILE_ATTRIBUTE_TEMPORARY: u32 = 0x100;
            const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
            o.access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
                .attributes(FILE_ATTRIBUTE_TEMPORARY)
                .custom_flags(FILE_FLAG_DELETE_ON_CLOSE);
        }
        match o.open(&p) {
            Ok(mut f) => {
                #[cfg(unix)]
                let _ = std::fs::remove_file(&p);
                f.write_all(bytes)?;
                return Ok(f);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::ErrorKind::AlreadyExists.into())
}

/// The descriptor type that travels with frames: none on Windows.
type Fd = crate::transport::Fd;

#[cfg(unix)]
type Conn = crate::transport::Conn;
#[cfg(windows)]
type Conn = crate::pipe::Conn<std::process::ChildStdout, std::process::ChildStdin>;

struct Proc {
    #[cfg(any(unix, windows))]
    conn: Conn,
    child: std::process::Child,
    /// The job object holding the host (closing the last handle kills it).
    #[cfg(windows)]
    _job: std::sync::Arc<crate::win::Job>,
    info: HostInfo,
}

impl Proc {
    /// Waits briefly for the process to exit by itself (it closed the connection), then kills
    /// it. Returns how it ended.
    fn reap(mut self, grace: Duration) -> String {
        let t0 = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(st)) => return describe_exit(st),
                Ok(None) if t0.elapsed() < grace => std::thread::sleep(Duration::from_millis(2)),
                _ => break,
            }
        }
        let _ = self.child.kill();
        match self.child.wait() {
            Ok(st) => describe_exit(st),
            Err(e) => e.to_string(),
        }
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn describe_exit(st: std::process::ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = st.signal() {
            let name = match sig {
                4 => " (SIGILL)",
                6 => " (SIGABRT)",
                7 => " (SIGBUS)",
                9 => " (SIGKILL)",
                11 => " (SIGSEGV)",
                31 => " (SIGSYS)",
                _ => "",
            };
            return format!("killed by signal {sig}{name}");
        }
    }
    #[cfg(windows)]
    if let Some(c) = st.code() {
        // NTSTATUS of a crash (the exit code of a process killed by an exception).
        let name = match c as u32 {
            0xC000_0005 => Some("access violation"),
            0xC000_001D => Some("illegal instruction"),
            0xC000_0017 | 0xC000_012D => Some("out of memory"),
            0xC000_00FD => Some("stack overflow"),
            0xC000_0409 => Some("fail-fast (abort)"),
            0xC000_0374 => Some("heap corruption"),
            0xC000_0135 | 0xC000_0139 | 0xC000_007B => Some("could not load a DLL"),
            0xC000_0022 => Some("access denied"),
            _ => None,
        };
        if let Some(n) = name {
            return format!("crashed with status {:#010x} ({n})", c as u32);
        }
    }
    match st.code() {
        Some(c) => format!("exited with status {c}"),
        None => "exited".into(),
    }
}

/// A pool of decode host processes.
pub struct Host {
    cfg: Config,
    slots: Box<[Mutex<Option<Proc>>]>,
    next: AtomicUsize,
    /// The last start failure, so a missing host is not respawned for every file.
    start_failure: Mutex<Option<(Instant, Error)>>,
    info: Mutex<Option<HostInfo>>,
    /// What the next host started loads ([`Config::vendor`], then [`Host::set_vendor_libs`]).
    vendor: Mutex<VendorLibs>,
}

impl Host {
    /// How long a failed start is remembered.
    const RETRY_AFTER: Duration = Duration::from_secs(20);

    pub fn new(cfg: Config) -> Host {
        let n = cfg.pool_size.max(1);
        let vendor = cfg.vendor.clone();
        Host {
            cfg,
            slots: (0..n).map(|_| Mutex::new(None)).collect(),
            next: AtomicUsize::new(0),
            start_failure: Mutex::new(None),
            info: Mutex::new(None),
            vendor: Mutex::new(vendor),
        }
    }

    /// The process-wide pool, configured from the environment ([`Config::from_env`]).
    pub fn global() -> &'static Host {
        static HOST: OnceLock<Host> = OnceLock::new();
        HOST.get_or_init(|| Host::new(Config::from_env()))
    }

    /// The process-wide pool for vendor SDKs the user installed ([`VendorLibs`]): one host,
    /// apart from the libheif/UnRAR ones, so a vendor library never shares their process, and
    /// a longer timeout (a camera RAW frame of a large sensor takes seconds on the CPU).
    pub fn vendor() -> &'static Host {
        static HOST: OnceLock<Host> = OnceLock::new();
        HOST.get_or_init(|| {
            let mut cfg = Config::from_env();
            cfg.pool_size = 1;
            cfg.timeout = cfg.timeout.max(Duration::from_secs(120));
            cfg.start_timeout = cfg.start_timeout.max(Duration::from_secs(30));
            Host::new(cfg)
        })
    }

    /// The vendor libraries the next host loads.
    pub fn vendor_libs(&self) -> VendorLibs {
        self.vendor.lock().clone()
    }

    /// Changes the vendor libraries hosts load. Running hosts stop when it changes (they
    /// loaded the old set before confining themselves); the next call starts a new one.
    pub fn set_vendor_libs(&self, libs: VendorLibs) {
        {
            let mut v = self.vendor.lock();
            if *v == libs {
                return;
            }
            *v = libs;
        }
        self.shutdown();
        *self.info.lock() = None;
        *self.start_failure.lock() = None;
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// The host's capabilities, starting one if none ran yet.
    pub fn info(&self) -> Result<HostInfo> {
        if let Some(i) = self.info.lock().clone() {
            return Ok(i);
        }
        let g = self.acquire();
        self.ensure(g).map(|(_, i)| i)
    }

    /// Process ids of the running hosts.
    pub fn pids(&self) -> Vec<u32> {
        self.slots
            .iter()
            .filter_map(|s| s.try_lock().and_then(|g| g.as_ref().map(|p| p.child.id())))
            .collect()
    }

    /// Stops every host (they restart on the next call).
    pub fn shutdown(&self) {
        for s in self.slots.iter() {
            if let Some(p) = s.lock().take() {
                p.reap(Duration::ZERO);
            }
        }
    }

    /// Decodes the primary image of a HEIF/HEIC file.
    pub fn heif_decode(&self, data: &[u8], limits: ImageLimits) -> Result<DecodedImage> {
        let info = self.info()?;
        if info.heif.is_none() {
            return Err(Error::Remote {
                kind: ErrorKind::NotInstalled,
                message: heif_missing(&info.heif_error),
            });
        }
        let input = SharedInput::new(data)?;
        let (resp, fds) = self.call(
            &|input| Request::HeifDecode { input, limits },
            Attach::Shared(&input),
            None,
        )?;
        match resp {
            Response::Image(img) => image_of(img, fds, limits),
            Response::Error { kind, message } => Err(remote(kind, message)),
            other => Err(unexpected(&other)),
        }
    }

    /// Decodes frame `frame` of a Blackmagic RAW clip at 1/`scale` (1, 2, 4 or 8) with the
    /// SDK the user installed ([`VendorLibs::braw`]).
    pub fn braw_decode(
        &self,
        clip: &ClipSource,
        frame: u64,
        scale: u8,
        limits: ImageLimits,
    ) -> Result<(ClipInfo, DecodedImage)> {
        let info = self.info()?;
        if info.braw.is_none() {
            let why = if info.braw_error.is_empty() {
                "the Blackmagic RAW SDK is not installed".to_owned()
            } else {
                format!(
                    "the Blackmagic RAW SDK could not be used: {}",
                    info.braw_error
                )
            };
            return Err(Error::NotInstalled(why));
        }
        let scale = match scale {
            0..=1 => 1,
            2..=3 => 2,
            4..=7 => 4,
            _ => 8,
        };
        let (resp, fds) = self.call(
            &|input| Request::BrawDecode {
                input,
                frame,
                scale,
                limits,
            },
            Attach::File(&clip.file),
            None,
        )?;
        match resp {
            Response::Clip { info, image } => {
                let img = image_of(image, fds, limits)?;
                Ok((info, img))
            }
            Response::Error { kind, message } => Err(remote(kind, message)),
            other => Err(unexpected(&other)),
        }
    }

    /// Lists a RAR archive; `password` opens one whose file names are encrypted.
    pub fn rar_list(
        &self,
        archive: &RarSource,
        max_entries: u32,
        password: Option<&Secret>,
    ) -> Result<Listing> {
        let (resp, _) = self.call(
            &|input| Request::RarList {
                input,
                volume: archive.name.clone(),
                max_entries,
                password: password.cloned(),
            },
            Attach::File(&archive.file),
            archive.volumes.as_deref(),
        )?;
        match resp {
            Response::Listing(l) if l.entries.len() <= max_entries as usize => Ok(l),
            Response::Listing(_) => Err(Error::Protocol("listing exceeds the entry limit".into())),
            Response::Error { kind, message } => Err(remote(kind, message)),
            other => Err(unexpected(&other)),
        }
    }

    /// Reads entry `index` of a [`Listing`] (`entry` is that listing's entry).
    pub fn rar_read(
        &self,
        archive: &RarSource,
        index: u32,
        entry: &ListEntry,
        max_size: u64,
        password: Option<&Secret>,
    ) -> Result<Vec<u8>> {
        let (resp, fds) = self.call(
            &|input| Request::RarRead {
                input,
                volume: archive.name.clone(),
                index,
                name: entry.name.clone(),
                max_size,
                password: password.cloned(),
            },
            Attach::File(&archive.file),
            archive.volumes.as_deref(),
        )?;
        match resp {
            Response::Data(p) => take_payload(p, fds, max_size),
            Response::Error { kind, message } => Err(remote(kind, message)),
            other => Err(unexpected(&other)),
        }
    }

    /// Reads several entries of a [`Listing`] in one pass (`items`: index and that listing's
    /// entry, in ascending index order), so a solid archive is decompressed once. Each entry
    /// is refused past `max_size` bytes, all of them together past `max_total`.
    pub fn rar_read_many(
        &self,
        archive: &RarSource,
        items: &[(u32, &ListEntry)],
        max_size: u64,
        max_total: u64,
        password: Option<&Secret>,
    ) -> Result<Vec<Vec<u8>>> {
        let wire: Vec<(u32, String)> = items.iter().map(|(i, e)| (*i, e.name.clone())).collect();
        let (resp, fds) = self.call(
            &|input| Request::RarReadMany {
                input,
                volume: archive.name.clone(),
                items: wire.clone(),
                max_size,
                max_total,
                password: password.cloned(),
            },
            Attach::File(&archive.file),
            archive.volumes.as_deref(),
        )?;
        match resp {
            Response::DataMany { sizes, payload } => {
                let total = sizes
                    .iter()
                    .try_fold(0u64, |a, &s| a.checked_add(s).filter(|_| s <= max_size))
                    .filter(|&t| t <= max_total);
                let Some(total) = total else {
                    return Err(Error::Protocol("batch exceeds the limits".into()));
                };
                if sizes.len() != items.len() {
                    return Err(Error::Protocol("batch answers another request".into()));
                }
                let all = take_payload(payload, fds, total)?;
                if all.len() as u64 != total {
                    return Err(Error::Protocol("batch size mismatch".into()));
                }
                let mut out = Vec::with_capacity(sizes.len());
                let mut at = 0;
                for s in sizes {
                    let s = s as usize;
                    out.push(all[at..at + s].to_vec());
                    at += s;
                }
                Ok(out)
            }
            Response::Error { kind, message } => Err(remote(kind, message)),
            other => Err(unexpected(&other)),
        }
    }

    /// Reads a STEP or IGES file with OpenCASCADE (a pool started with [`Config::occt`]) and
    /// tessellates it; the model's bytes ([`crate::cad::CadModel::decode`] reads them).
    pub fn cad_read(&self, data: &[u8], format: CadFormat, limits: CadLimits) -> Result<Vec<u8>> {
        let info = self.info()?;
        if info.occt.is_none() {
            return Err(Error::Remote {
                kind: ErrorKind::NotInstalled,
                message: info.occt_error,
            });
        }
        let input = SharedInput::new(data)?;
        let (resp, fds) = self.call(
            &|input| Request::CadRead {
                input,
                format,
                limits,
            },
            Attach::Shared(&input),
            None,
        )?;
        match resp {
            Response::Model(p) => take_payload(p, fds, limits.max_bytes),
            Response::Error { kind, message } => Err(remote(kind, message)),
            other => Err(unexpected(&other)),
        }
    }

    /// The facts of a CZI document (a pool started with [`Config::czi`]): the bytes
    /// [`crate::czi::CziInfo::decode`] reads, at most `max_bytes`.
    pub fn czi_info(&self, src: &CziSource, max_xml: u64, max_bytes: u64) -> Result<Vec<u8>> {
        self.czi_ready()?;
        let (resp, fds) = self.call(
            &|input| Request::CziInfo {
                input,
                key: src.key,
                max_xml,
            },
            Attach::File(&src.file),
            None,
        )?;
        match resp {
            Response::CziInfo(p) => take_payload(p, fds, max_bytes),
            Response::Error { kind, message } => Err(remote(kind, message)),
            other => Err(unexpected(&other)),
        }
    }

    /// One channel of a region of a CZI plane, `want` (width, height) pixels: its width,
    /// height and rows (checked against the request: [`crate::czi::check_plane`]).
    pub fn czi_read(
        &self,
        src: &CziSource,
        region: CziRegion,
        want: (u32, u32),
    ) -> Result<(u32, u32, Vec<u8>)> {
        self.czi_ready()?;
        let (resp, fds) = self.call(
            &|input| Request::CziRead {
                input,
                key: src.key,
                region,
            },
            Attach::File(&src.file),
            None,
        )?;
        match resp {
            Response::CziPlane(p) => {
                let len = crate::czi::check_plane(&p, region.pixel, want, region.max_bytes)
                    .map_err(Error::Protocol)?;
                let data = take_payload(p.payload, fds, len)?;
                if data.len() as u64 != len {
                    return Err(Error::Protocol("CZI plane size mismatch".into()));
                }
                Ok((p.width, p.height, data))
            }
            Response::Error { kind, message } => Err(remote(kind, message)),
            other => Err(unexpected(&other)),
        }
    }

    /// Attachment `index` of a CZI document: its encoded bytes, or an embedded CZI image's
    /// first channel fitted into `max_side` (sizes checked; the payload at most `max_bytes`).
    pub fn czi_attachment(
        &self,
        src: &CziSource,
        index: u32,
        max_side: u32,
        max_bytes: u64,
    ) -> Result<(CziPlane, Vec<u8>)> {
        self.czi_ready()?;
        let (resp, fds) = self.call(
            &|input| Request::CziAttachment {
                input,
                key: src.key,
                index,
                max_side,
                max_bytes,
            },
            Attach::File(&src.file),
            None,
        )?;
        match resp {
            Response::CziPlane(p) => {
                let want = match p.kind {
                    CziPlaneKind::Encoded => max_bytes,
                    CziPlaneKind::Pixels => {
                        let bpp = crate::protocol::czi_pixel_bytes(p.pixel)
                            .ok_or_else(|| Error::Protocol("unknown pixel type".into()))?;
                        if p.width == 0
                            || p.height == 0
                            || p.width.max(p.height) > max_side.saturating_add(2)
                        {
                            return Err(Error::Protocol("attachment image of a bad size".into()));
                        }
                        u64::from(p.width) * u64::from(p.height) * bpp as u64
                    }
                };
                let data = take_payload(p.payload.clone(), fds, want.min(max_bytes))?;
                if p.kind == CziPlaneKind::Pixels && data.len() as u64 != want {
                    return Err(Error::Protocol("attachment size mismatch".into()));
                }
                Ok((p, data))
            }
            Response::Error { kind, message } => Err(remote(kind, message)),
            other => Err(unexpected(&other)),
        }
    }

    /// Runs `sql` (the viewer's own statement, never text from the file) with `params` on the
    /// database `src`, read-only, inside `limits` ([`crate::sql`]). The rows are checked
    /// against the limits.
    pub fn sql_query(
        &self,
        src: &SqlSource,
        sql: &str,
        params: &[SqlValue],
        limits: SqlLimits,
    ) -> Result<SqlRows> {
        // The host asks for the WAL (volume "wal") when it opens the database.
        let path = src.wal.clone();
        let wal = move |name: &str| {
            let p = path.as_deref().filter(|_| name == "wal")?;
            open_regular(p).ok()
        };
        let (resp, fds) = self.call(
            &|input| Request::SqlQuery {
                input,
                key: src.key,
                sql: sql.to_owned(),
                params: params.to_vec(),
                limits,
            },
            Attach::File(&src.file),
            Some(&wal),
        )?;
        match resp {
            Response::Rows {
                columns,
                rows,
                truncated,
                payload,
            } => {
                if rows > limits.max_rows {
                    return Err(Error::Protocol("more rows than allowed".into()));
                }
                let data = take_payload(payload, fds, limits.max_bytes)?;
                SqlRows::decode(columns as usize, rows as usize, truncated, &data)
                    .map_err(Error::Protocol)
            }
            Response::Error { kind, message } => Err(remote(kind, message)),
            other => Err(unexpected(&other)),
        }
    }

    fn czi_ready(&self) -> Result<()> {
        let info = self.info()?;
        if info.czi.is_none() {
            return Err(Error::Remote {
                kind: ErrorKind::NotInstalled,
                message: info.czi_error,
            });
        }
        Ok(())
    }

    /// Runs a self-test operation (host started with `test_hooks`).
    pub fn test(&self, op: TestOp) -> Result<Response> {
        let (resp, _) = self.call(&|_| Request::Test(op.clone()), Attach::Nothing, None)?;
        match resp {
            Response::Error { kind, message } => Err(remote(kind, message)),
            r => Ok(r),
        }
    }

    fn acquire(&self) -> MutexGuard<'_, Option<Proc>> {
        let n = self.slots.len();
        let start = self.next.fetch_add(1, Ordering::Relaxed) % n;
        // A free slot that already runs a host, then any free slot, then wait.
        for want_running in [true, false] {
            for i in 0..n {
                if let Some(g) = self.slots[(start + i) % n].try_lock()
                    && g.is_some() == want_running
                {
                    return g;
                }
            }
        }
        self.slots[start].lock()
    }

    fn ensure<'a>(
        &self,
        mut g: MutexGuard<'a, Option<Proc>>,
    ) -> Result<(MutexGuard<'a, Option<Proc>>, HostInfo)> {
        if let Some(p) = g.as_ref() {
            let i = p.info.clone();
            return Ok((g, i));
        }
        if let Some((t, e)) = self.start_failure.lock().as_ref()
            && t.elapsed() < Self::RETRY_AFTER
        {
            return Err(e.clone());
        }
        match self.spawn() {
            Ok(p) => {
                *self.start_failure.lock() = None;
                let i = p.info.clone();
                *self.info.lock() = Some(i.clone());
                *g = Some(p);
                Ok((g, i))
            }
            Err(e) => {
                specular_log::warn!(target: specular_log::DECODE, "decode host: {e}");
                *self.start_failure.lock() = Some((Instant::now(), e.clone()));
                Err(e)
            }
        }
    }

    /// Finds the host binary.
    pub fn exe(&self) -> Result<PathBuf> {
        let name = format!("{HOST_EXE}{}", std::env::consts::EXE_SUFFIX);
        let explicit = self
            .cfg
            .exe
            .clone()
            .or_else(|| std::env::var_os("SPECULAR_DECODE_HOST").map(PathBuf::from));
        if let Some(p) = explicit {
            return if p.is_file() {
                Ok(p)
            } else {
                Err(Error::NotInstalled(format!(
                    "decode host {} not found",
                    p.display()
                )))
            };
        }
        let me = std::env::current_exe()?;
        let dir = me.parent().unwrap_or(Path::new("."));
        // Next to the viewer (installed, `cargo run`), one level up (`cargo test` binaries
        // live in target/<profile>/deps), the macOS bundle's Helpers, a Unix libexec.
        let candidates = [
            dir.join(&name),
            dir.join("..").join(&name),
            dir.join("../Helpers").join(&name),
            dir.join("../libexec/specular").join(&name),
        ];
        candidates.into_iter().find(|p| p.is_file()).ok_or_else(|| {
            Error::NotInstalled(format!(
                "decode host `{name}` not found next to {}",
                me.display()
            ))
        })
    }

    #[cfg(not(any(unix, windows)))]
    fn spawn(&self) -> Result<Proc> {
        Err(Error::Unavailable(
            "the sandboxed decode host is not implemented on this platform".into(),
        ))
    }

    #[cfg(not(any(unix, windows)))]
    fn call(
        &self,
        _make: &dyn Fn(Input) -> Request,
        _attach: Attach<'_>,
        _volumes: Option<&VolumeProvider>,
    ) -> Result<(Response, Vec<Fd>)> {
        // `spawn` always fails here; this returns (and caches) that error.
        let (g, _) = self.ensure(self.acquire())?;
        drop(g);
        Err(Error::Unavailable("no decode host on this platform".into()))
    }
}

/// What travels with a request.
#[derive(Clone, Copy)]
enum Attach<'a> {
    Nothing,
    /// Shared memory or inline bytes.
    Shared(&'a SharedInput),
    /// An open file: a descriptor (Unix) or a handle duplicated into the host (Windows).
    File(&'a std::fs::File),
}

/// Most [`Response::NeedVolume`] questions one request may ask (UnRAR asks at most a few per
/// volume; this bounds a confused or hostile host).
const MAX_VOLUME_REQUESTS: usize = 4096;

/// The pixels of an image reply, checked against the request's limits.
fn image_of(img: ImageReply, fds: Vec<Fd>, limits: ImageLimits) -> Result<DecodedImage> {
    let px = u64::from(img.width) * u64::from(img.height);
    if px == 0 || px > limits.max_pixels || img.width.max(img.height) > limits.max_side {
        return Err(Error::Protocol(format!(
            "image of {}×{} outside the limits",
            img.width, img.height
        )));
    }
    let rgba = take_payload(img.pixels, fds, px * 4)?;
    if rgba.len() as u64 != px * 4 {
        return Err(Error::Protocol("pixel buffer size mismatch".into()));
    }
    Ok(DecodedImage {
        width: img.width,
        height: img.height,
        has_alpha: img.has_alpha,
        bit_depth: img.bit_depth,
        icc: img.icc,
        rgba,
    })
}

fn heif_missing(detail: &str) -> String {
    if detail.is_empty() {
        "HEIC support not installed: libheif was not found".into()
    } else {
        format!("HEIC support not installed: libheif was not found ({detail})")
    }
}

fn remote(kind: ErrorKind, message: String) -> Error {
    Error::Remote { kind, message }
}

/// A reply that does not answer the request, named by its kind only: its contents (Debug
/// text, possibly long and full of bytes) never reach the error the user may see.
fn unexpected(r: &Response) -> Error {
    let debug = format!("{r:?}");
    Error::Protocol(format!("unexpected {} response", response_name(&debug)))
}

/// The variant name at the start of a response's Debug text ("Decoded { … }" → "Decoded").
fn response_name(debug: &str) -> &str {
    let end = debug
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(debug.len());
    if end == 0 { "unknown" } else { &debug[..end] }
}

fn take_payload(p: Payload, mut fds: Vec<Fd>, max: u64) -> Result<Vec<u8>> {
    match p {
        Payload::Inline(b) if b.len() as u64 <= max => Ok(b),
        Payload::Inline(_) => Err(Error::Protocol("payload exceeds the limit".into())),
        Payload::Fd { len } => {
            let fd = fds
                .pop()
                .ok_or_else(|| Error::Protocol("payload descriptor missing".into()))?;
            crate::transport::shm::read(fd, len, max).map_err(|e| Error::Protocol(e.to_string()))
        }
    }
}

/// Starts `cmd` from one long-lived thread. The host asks for a parent-death signal so it never
/// outlives the viewer, and on Linux that signal fires when the *thread* that forked it exits:
/// spawned from a short-lived worker, the host would be killed together with that worker.
#[cfg(unix)]
fn spawn_from_stable_thread(cmd: std::process::Command) -> std::io::Result<std::process::Child> {
    use std::sync::mpsc;
    type Reply = mpsc::SyncSender<std::io::Result<std::process::Child>>;
    static SPAWNER: OnceLock<Option<mpsc::Sender<(std::process::Command, Reply)>>> =
        OnceLock::new();
    let spawner = SPAWNER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(std::process::Command, Reply)>();
        specular_thread::builder("host-spawn")
            .spawn(move || {
                // A panic costs the start in hand (its caller hears "could not start"); the
                // thread itself goes on, so hosts it started keep their parent-death signal.
                specular_log::service::supervise(
                    "host-spawn",
                    specular_log::service::Policy::Restart(specular_log::service::Backoff {
                        first: std::time::Duration::from_millis(100),
                        max: std::time::Duration::from_secs(5),
                    }),
                    || false,
                    || {
                        for (mut cmd, reply) in rx.iter() {
                            spawn_trip(&cmd);
                            let _ = reply.send(cmd.spawn());
                        }
                    },
                    |_| {},
                );
            })
            .ok()
            .map(|_| tx)
    });
    let Some(tx) = spawner else {
        let mut cmd = cmd;
        return cmd.spawn();
    };
    let (reply, rx) = mpsc::sync_channel(1);
    tx.send((cmd, reply))
        .map_err(|_| std::io::Error::other("spawner thread is gone"))?;
    rx.recv()
        .map_err(|_| std::io::Error::other("internal error while starting the host"))?
}

/// Test hook: in debug builds, starting a program whose file name contains
/// `specular-test-panic` panics on the spawner thread.
#[cfg(unix)]
fn spawn_trip(cmd: &std::process::Command) {
    #[cfg(debug_assertions)]
    if Path::new(cmd.get_program())
        .file_name()
        .is_some_and(|n| n.to_string_lossy().contains("specular-test-panic"))
    {
        panic!(
            "test panic starting {}",
            cmd.get_program().to_string_lossy()
        );
    }
    #[cfg(not(debug_assertions))]
    let _ = cmd;
}

/// The host command line and environment shared by every platform.
#[cfg(any(unix, windows))]
fn command(cfg: &Config, exe: &Path, vendor: &VendorLibs) -> std::process::Command {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--ipc")
        .arg(format!("--memory-limit={}", cfg.memory_limit));
    if cfg.occt {
        cmd.arg("--occt");
    }
    if let Some(p) = &vendor.braw {
        let mut a = std::ffi::OsString::from("--braw=");
        a.push(p);
        cmd.arg(a);
    }
    if cfg.czi {
        cmd.arg("--czi");
    }
    if cfg.test_hooks {
        cmd.arg("--test-hooks");
        if cfg.disable_sandbox {
            cmd.arg("--no-sandbox");
        }
    }
    cmd.env_clear();
    let pass: &[&str] = if cfg!(windows) {
        // The system needs these to load DLLs; PATH finds libheif's dependencies in
        // development setups (installed builds keep them next to the host). LOCALAPPDATA is
        // where Windows puts the AppContainer's own folder (Packages\<name>\AC): without it
        // CreateProcess fails with ERROR_ENVVAR_NOT_FOUND.
        &[
            "SystemRoot",
            "windir",
            "SystemDrive",
            "PATH",
            "LOCALAPPDATA",
            "LIBHEIF_PLUGIN_PATH",
            "SPECULAR_LIBHEIF",
            "SPECULAR_OCCT_BRIDGE",
            "SPECULAR_CZI_BRIDGE",
            "SPECULAR_LOG",
        ]
    } else {
        &[
            "LD_LIBRARY_PATH",
            "DYLD_LIBRARY_PATH",
            "DYLD_FALLBACK_LIBRARY_PATH",
            "LIBHEIF_PLUGIN_PATH",
            "SPECULAR_LIBHEIF",
            "SPECULAR_OCCT_BRIDGE",
            "SPECULAR_CZI_BRIDGE",
            "SPECULAR_LOG",
        ]
    };
    for k in pass {
        if let Some(v) = std::env::var_os(k) {
            cmd.env(k, v);
        }
    }
    cmd.envs(cfg.env.iter().map(|(k, v)| (k, v)));
    cmd
}

#[cfg(any(unix, windows))]
impl Host {
    /// Starts a host process and connects to it (no hello yet).
    #[cfg(unix)]
    fn launch(&self, exe: &Path) -> Result<Proc> {
        use std::os::fd::OwnedFd;
        use std::os::unix::net::UnixStream;
        use std::process::Stdio;

        let (ours, theirs) = UnixStream::pair()?;
        let mut cmd = command(&self.cfg, exe, &self.vendor_libs());
        // The host's end of the socket becomes its stdin; nothing else is inherited (std
        // opens every descriptor close-on-exec).
        cmd.stdin(Stdio::from(OwnedFd::from(theirs)))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .current_dir("/");
        // The command (and with it our copy of the host's end of the socket, so the host's
        // death reads as end-of-file) is dropped once spawned.
        let child = spawn_from_stable_thread(cmd)
            .map_err(|e| Error::Unavailable(format!("cannot start {}: {e}", exe.display())))?;
        Ok(Proc {
            conn: Conn::new(ours),
            child,
            info: placeholder_info(),
        })
    }

    /// Starts a host process in its AppContainer and job object, its stdin and stdout piped
    /// to us.
    #[cfg(windows)]
    fn launch(&self, exe: &Path) -> Result<Proc> {
        use std::process::Stdio;

        let mut cmd = command(&self.cfg, exe, &self.vendor_libs());
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(root) = std::env::var_os("SystemRoot") {
            // A directory the AppContainer may read (the viewer's may not be).
            cmd.current_dir(PathBuf::from(root).join("System32"));
        }
        let opts = crate::win::Options {
            memory_limit: self.cfg.memory_limit,
            confine: !self.cfg.disable_sandbox,
        };
        let (mut child, job) = crate::win::spawn(&mut cmd, &opts).map_err(|e| {
            let hint = if e.kind() == std::io::ErrorKind::PermissionDenied {
                format!(
                    " (the AppContainer cannot read the program's folder; install Specular, or \
                     run `icacls \"{}\" /grant *S-1-15-2-1:(OI)(CI)RX`)",
                    exe.parent().unwrap_or(Path::new(".")).display()
                )
            } else {
                String::new()
            };
            Error::Unavailable(format!("cannot start {}: {e}{hint}", exe.display()))
        })?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            return Err(Error::Unavailable("host pipes missing".into()));
        };
        let killer = job.clone();
        let conn = crate::pipe::Conn::with_watchdog(stdout, stdin, move || killer.terminate())?;
        Ok(Proc {
            conn,
            child,
            _job: job,
            info: placeholder_info(),
        })
    }

    fn spawn(&self) -> Result<Proc> {
        use crate::protocol::{SandboxLevel, VERSION};

        let exe = self.exe()?;
        let t0 = Instant::now();
        let mut proc_ = self.launch(&exe)?;
        proc_.conn.set_timeout(Some(self.cfg.start_timeout))?;
        let hello = Request::Hello { version: VERSION }.encode();
        let reply = proc_
            .conn
            .send(&hello, &[])
            .and_then(|()| proc_.conn.recv());
        let (body, _) = match reply {
            Ok(r) => r,
            Err(e) if is_timeout(&e) => {
                proc_.reap(Duration::ZERO);
                return Err(Error::Timeout(self.cfg.start_timeout));
            }
            Err(e) => {
                let how = proc_.reap(Duration::from_millis(500));
                return Err(Error::Crashed(format!("at start ({e}): {how}")));
            }
        };
        let info = match Response::decode(&body) {
            Ok(Response::Hello(i)) if i.version == VERSION => *i,
            Ok(Response::Hello(i)) => {
                return Err(Error::Unavailable(format!(
                    "decode host speaks protocol {}, the viewer {VERSION}: reinstall Specular",
                    i.version
                )));
            }
            Ok(Response::Error { message, .. }) => return Err(Error::Unavailable(message)),
            Ok(other) => return Err(unexpected(&other)),
            Err(e) => return Err(Error::Protocol(e.to_string())),
        };
        if self.cfg.require_sandbox && info.sandbox == SandboxLevel::None {
            return Err(Error::Unavailable(format!(
                "decode host could not sandbox itself ({}); set SPECULAR_HOST_ALLOW_UNSANDBOXED=1 to accept that",
                info.sandbox_detail
            )));
        }
        specular_log::info!(
            target: specular_log::DECODE,
            "decode host pid {} started in {:.1} ms: sandbox {:?} ({}), libheif {}, rar {}, braw {}",
            proc_.child.id(),
            t0.elapsed().as_secs_f64() * 1e3,
            info.sandbox,
            info.sandbox_detail,
            info.heif.as_deref().unwrap_or("missing"),
            info.rar,
            info.braw.as_deref().unwrap_or(if info.braw_error.is_empty() {
                "not asked"
            } else {
                &info.braw_error
            })
        );
        proc_.info = info;
        Ok(proc_)
    }

    /// Sends `make(input)` to a host (restarting a dead one once), answers its volume questions
    /// from `volumes`, and returns its answer. `make` is called once per attempt: on Windows the
    /// input names a handle duplicated into that particular process.
    fn call(
        &self,
        make: &dyn Fn(Input) -> Request,
        attach: Attach<'_>,
        volumes: Option<&VolumeProvider>,
    ) -> Result<(Response, Vec<Fd>)> {
        let mut g = self.acquire();
        let mut retried = false;
        loop {
            let (g2, _) = self.ensure(g)?;
            g = g2;
            let Some(p) = g.as_mut() else {
                return Err(Error::Unavailable("no host".into()));
            };
            // A host that died while idle fails here or at the send: macOS refuses to set a
            // timeout on a socket whose other end is gone (EINVAL).
            let sent = p
                .conn
                .set_timeout(Some(self.cfg.timeout))
                .and_then(|()| p.send(make, attach));
            if let Err(e) = sent {
                // The host died while idle (or right now). Start a fresh one once.
                let how = g.take().map(|p| p.reap(Duration::from_millis(100)));
                if !retried && !is_timeout(&e) {
                    retried = true;
                    specular_log::debug!(target: specular_log::DECODE, "decode host gone before the request ({e}; {how:?}); restarting");
                    continue;
                }
                return Err(Error::Crashed(how.unwrap_or_else(|| e.to_string())));
            }
            return self.answer(&mut g, volumes);
        }
    }

    /// Receives the answer to a sent request, serving volume questions on the way. Any failure
    /// leaves the slot empty (the host is killed).
    fn answer(
        &self,
        g: &mut MutexGuard<'_, Option<Proc>>,
        volumes: Option<&VolumeProvider>,
    ) -> Result<(Response, Vec<Fd>)> {
        let mut asked = 0usize;
        loop {
            let Some(p) = g.as_mut() else {
                return Err(Error::Unavailable("no host".into()));
            };
            let pid = p.child.id();
            let io = match p.conn.recv() {
                Ok((body, fds)) => match Response::decode(&body) {
                    Ok(Response::NeedVolume { name }) if fds.is_empty() => {
                        asked += 1;
                        if asked > MAX_VOLUME_REQUESTS {
                            g.take().map(|p| p.reap(Duration::ZERO));
                            return Err(Error::Protocol("too many volume requests".into()));
                        }
                        let file = volumes.and_then(|v| v(&name));
                        specular_log::debug!(target: specular_log::DECODE, "decode host pid {pid} asks for volume {name}: {}", if file.is_some() { "found" } else { "none" });
                        let sent = match &file {
                            Some(f) => p.send(
                                &|input| Request::Volume { input: Some(input) },
                                Attach::File(f),
                            ),
                            None => p.send(&|_| Request::Volume { input: None }, Attach::Nothing),
                        };
                        match sent {
                            Ok(()) => continue,
                            Err(e) => e,
                        }
                    }
                    Ok(r)
                        if r.fd_count() == fds.len()
                            && !matches!(r, Response::NeedVolume { .. }) =>
                    {
                        return Ok((r, fds));
                    }
                    Ok(_) => {
                        g.take().map(|p| p.reap(Duration::ZERO));
                        return Err(Error::Protocol("descriptor count mismatch".into()));
                    }
                    Err(e) => {
                        g.take().map(|p| p.reap(Duration::ZERO));
                        return Err(Error::Protocol(e.to_string()));
                    }
                },
                Err(e) => e,
            };
            if is_timeout(&io) {
                g.take().map(|p| p.reap(Duration::ZERO));
                specular_log::warn!(target: specular_log::DECODE, "decode host pid {pid} timed out after {:?}; killed", self.cfg.timeout);
                return Err(Error::Timeout(self.cfg.timeout));
            }
            let how = g
                .take()
                .map(|p| p.reap(Duration::from_millis(500)))
                .unwrap_or_default();
            specular_log::warn!(target: specular_log::DECODE, "decode host pid {pid} crashed ({io}): {how}");
            return Err(Error::Crashed(how));
        }
    }
}

#[cfg(any(unix, windows))]
impl Proc {
    /// Sends `make(input)` with `attach`.
    fn send(&mut self, make: &dyn Fn(Input) -> Request, attach: Attach<'_>) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::fd::AsFd;
            let (input, file) = match attach {
                Attach::Nothing => (Input::Inline(Vec::new()), None),
                Attach::Shared(s) => (s.input(), s.file.as_ref()),
                Attach::File(f) => (Input::Fd, Some(f)),
            };
            let req = make(input);
            let fds: Vec<_> = file.map(|f| f.as_fd()).into_iter().collect();
            if req.fd_count() != fds.len() {
                return Err(std::io::Error::other("request and descriptors disagree"));
            }
            // Wiped once sent when it carries a password.
            let body = zeroize::Zeroizing::new(req.encode());
            self.conn.send(&body, &fds)
        }
        #[cfg(windows)]
        {
            let input = match attach {
                Attach::Nothing => Input::Inline(Vec::new()),
                Attach::Shared(s) => s.input(),
                Attach::File(f) => {
                    Input::Handle(crate::win::duplicate_for_reading(&self.child, f)?)
                }
            };
            let req = make(input);
            if req.fd_count() != 0 {
                return Err(std::io::Error::other("request needs a descriptor"));
            }
            let body = zeroize::Zeroizing::new(req.encode());
            self.conn.send(&body, &[])
        }
    }
}

#[cfg(any(unix, windows))]
fn placeholder_info() -> HostInfo {
    HostInfo {
        version: 0,
        sandbox: crate::protocol::SandboxLevel::None,
        sandbox_detail: String::new(),
        heif: None,
        heif_error: String::new(),
        rar: false,
        occt: None,
        occt_error: String::new(),
        czi: None,
        czi_error: String::new(),
        test_hooks: false,
        braw: None,
        braw_error: String::new(),
    }
}

#[cfg(any(unix, windows))]
fn is_timeout(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

#[cfg(test)]
mod tests {
    /// A panic while starting a host fails that start only; the spawner thread goes on.
    #[cfg(unix)]
    #[test]
    fn spawner_survives_a_panicking_start() {
        let bad = std::process::Command::new("/nonexistent/specular-test-panic-host");
        let e = super::spawn_from_stable_thread(bad).unwrap_err();
        assert!(e.to_string().contains("internal error"), "{e}");
        let mut ok = super::spawn_from_stable_thread(std::process::Command::new("true"))
            .expect("the spawner still starts programs");
        assert!(ok.wait().unwrap().success());
    }
    use super::*;

    #[test]
    fn unexpected_names_the_kind_only() {
        let e = unexpected(&Response::Echo(vec![0xAB; 4096]));
        let s = e.to_string();
        assert!(s.contains("unexpected Echo response"), "{s}");
        assert!(
            !s.contains("171") && !s.contains('[') && s.len() < 80,
            "{s}"
        );
        let e = unexpected(&Response::NeedVolume {
            name: "secret.part2.rar".into(),
        });
        assert!(!e.to_string().contains("secret"), "{e}");
        assert_eq!(response_name("Ok"), "Ok");
        assert_eq!(response_name(""), "unknown");
    }

    #[test]
    fn shared_input_kinds() {
        let s = SharedInput::new(b"image bytes").unwrap();
        assert_eq!(s.len(), 11);
        assert_eq!(
            matches!(s.input(), Input::Fd),
            cfg!(any(target_os = "linux", target_os = "android"))
        );
        assert!(SharedInput::inline(Vec::new()).is_empty());
    }

    #[test]
    fn rar_source_from_bytes() {
        let bytes: Vec<u8> = (0..200_000u32).map(|i| (i * 7) as u8).collect();
        let r = RarSource::from_bytes(&bytes).unwrap();
        assert_eq!(r.len(), bytes.len() as u64);
        assert!(r.same_content(&bytes));
        let mut other = bytes.clone();
        other[199_999] ^= 1;
        assert!(!r.same_content(&other));
        assert!(!r.same_content(&bytes[1..]));
        assert!(r.volumes.is_none());
        assert!(RarSource::from_bytes(&[]).unwrap().same_content(&[]));
    }

    #[test]
    fn bad_volume_names_refused() {
        let f = private_copy(b"x").unwrap();
        assert!(RarSource::with_volumes(f, "../x.rar", None).is_err());
    }

    #[test]
    fn volume_resolution() {
        let d = Path::new("/d");
        let r = |real: &str, want: &str| {
            let syn = crate::protocol::volume_name(real);
            resolve_volume(d, real, &syn, want).map(|p| p.to_string_lossy().into_owned())
        };
        // Expected paths joined the same way (`\` on Windows).
        let j = |name: &str| Some(d.join(name).to_string_lossy().into_owned());
        // New numbering, with characters the synthetic name replaced.
        assert_eq!(
            r("Mein Comic Ü.part1.rar", "Mein_Comic__.part2.rar"),
            j("Mein Comic Ü.part2.rar")
        );
        assert_eq!(r("a.part01.rar", "a.part02.rar"), j("a.part02.rar"));
        assert_eq!(r("a.part9.rar", "a.part10.rar"), j("a.part10.rar"));
        // Old numbering, CBR, numbered extensions.
        assert_eq!(r("a b.rar", "a_b.r00"), j("a b.r00"));
        assert_eq!(r("a.r99", "a.s00"), j("a.s00"));
        assert_eq!(r("a.cbr", "a.r00"), j("a.r00"));
        assert_eq!(r("a.001", "a.002"), j("a.002"));
        // Refused: itself, other sets, other kinds of files, paths, bad names.
        assert_eq!(r("a.rar", "a.rar"), None);
        assert_eq!(r("a.rar", "b.r00"), None);
        assert_eq!(r("a.part1.rar", "b.part2.rar"), None);
        assert_eq!(r("a.rar", "a.txt"), None);
        assert_eq!(r("a.rar", "a.jpg"), None);
        assert_eq!(r("a.rar", "a"), None);
        assert_eq!(r("a.rar", "../a.r00"), None);
        assert_eq!(r("a.rar", "a.r00/x"), None);
        assert_eq!(r("a.rar", ""), None);
        assert_eq!(r("a.rar", ".."), None);
        assert_eq!(r("notes.txt", "notes.r00"), None);
        // A very long name: the synthetic name is its tail.
        let long = format!("{}.part1.rar", "x".repeat(300));
        let syn = crate::protocol::volume_name(&long);
        let want = syn.replace(".part1.", ".part2.");
        assert_eq!(
            resolve_volume(d, &long, &syn, &want),
            Some(d.join(format!("{}.part2.rar", "x".repeat(300))))
        );
    }

    #[test]
    fn stems() {
        assert_eq!(volume_stem("x.part2.rar"), Some("x"));
        assert_eq!(volume_stem("x.PART2.RAR"), Some("x"));
        assert_eq!(volume_stem("x.part.rar"), Some("x.part"));
        assert_eq!(volume_stem("x.y.cbr"), Some("x.y"));
        assert_eq!(volume_stem("x.r00"), Some("x"));
        assert_eq!(volume_stem("x.001"), Some("x"));
        assert_eq!(volume_stem("x.r"), None);
        assert_eq!(volume_stem("x.zip"), None);
        assert_eq!(volume_stem("rar"), None);
    }

    #[cfg(windows)]
    #[test]
    fn crash_codes_are_named() {
        use std::os::windows::process::ExitStatusExt;
        let st = std::process::ExitStatus::from_raw(0xC000_0409);
        assert!(describe_exit(st).contains("abort"), "{}", describe_exit(st));
        assert_eq!(
            describe_exit(std::process::ExitStatus::from_raw(3)),
            "exited with status 3"
        );
    }
}
