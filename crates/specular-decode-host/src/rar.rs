//! RAR through UnRAR (C++, via the `unrar` crate). Extraction only: the UnRAR licence allows
//! using its code to read RAR archives, never to build a RAR-compatible archiver.
//!
//! The host cannot open files. Specular's patch to UnRAR
//! (`third_party/unrar_sys/patches/specular-handle-io.patch`) sends every file UnRAR opens
//! through [`open_hook`], which hands out the files the viewer passed: the archive's first
//! volume with the request, further volumes on demand ([`Volumes::ask`], which asks the
//! viewer). UnRAR reads them with positioned reads and never closes them; they are dropped
//! when the request is done. Names are the synthetic names of the protocol, never paths.
//!
//! A password reaches UnRAR the same way (`third_party/unrar_sys/patches/specular-password.patch`):
//! before opening the archive, UnRAR asks the hook for the reserved names [`PASSWORD_NAME`]`0`,
//! `1`, ..., one byte of the password's UTF-8 each, so it covers encrypted headers (`-hp`) as
//! well as encrypted files, and never travels as a C string.
//!
//! UnRAR source code may be used in any software to handle
//! RAR archives without limitations free of charge, but cannot be
//! used to develop RAR (WinRAR) compatible archiver and to
//! re-create RAR compression algorithm, which is proprietary.
//! Distribution of modified UnRAR source code in separate form
//! or as a part of other software is permitted, provided that
//! full text of this paragraph, starting from "UnRAR source code"
//! words, is included in license, or in documentation if license
//! is not available, and in source code comments of resulting package.

use specular_sandbox::protocol::{ErrorKind, ListEntry, Listing, Secret, valid_volume_name};
use std::ffi::c_char;
use std::fs::File;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};
use unrar::error::{Code, UnrarError};

type OpenHook = extern "C" fn(name: *const c_char, len: usize) -> isize;

#[allow(unsafe_code)]
// SAFETY (declaration): defined by the Specular patch in file.cpp as
// `extern "C" void SpecularUnrarSetOpenHook(SpecularOpenHook)`, with
// `typedef intptr_t (*SpecularOpenHook)(const char *, size_t)`; a null hook is allowed. It only
// stores the pointer, so calling it is safe.
unsafe extern "C" {
    safe fn SpecularUnrarSetOpenHook(hook: Option<OpenHook>);
}

/// Most volumes one request may open (each costs a round trip to the viewer).
const MAX_VOLUMES: usize = 4096;

/// Volumes kept open at once besides the first (the host may hold only 64 descriptors).
const OPEN_VOLUMES: usize = 8;

/// The prefix of the names under which the patched UnRAR asks the hook for the password, a
/// byte at a time (`"\x01password/0"`, ...); see `specular-password.patch`.
const PASSWORD_NAME: &str = "\x01password/";

/// Asks the viewer for another volume by name.
pub type AskVolume = dyn FnMut(&str) -> Option<File> + Send;

/// The files of one request.
pub struct Volumes {
    /// The synthetic name of the first volume and the file the request carried.
    pub first: (String, File),
    /// Asks the viewer for another volume by name.
    pub ask: Box<AskVolume>,
    /// The archive's password (encrypted entries, or encrypted headers).
    pub password: Option<Secret>,
}

struct Broker {
    volumes: Volumes,
    /// Volumes asked for and still open (at most [`OPEN_VOLUMES`], oldest first), and names
    /// the viewer had no file for (`None`).
    more: Vec<(String, Option<File>)>,
    /// Questions asked so far (at most [`MAX_VOLUMES`]).
    asked: usize,
}

static BROKER: Mutex<Option<Broker>> = Mutex::new(None);

fn broker() -> MutexGuard<'static, Option<Broker>> {
    BROKER.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Routes UnRAR's file opens through [`open_hook`]. Called once at start; from then on UnRAR
/// opens nothing by path (and creates nothing).
pub fn install_hook() {
    SpecularUnrarSetOpenHook(Some(open_hook));
}

#[cfg(unix)]
fn raw(f: &File) -> isize {
    use std::os::fd::AsRawFd;
    f.as_raw_fd() as isize
}

#[cfg(windows)]
fn raw(f: &File) -> isize {
    use std::os::windows::io::AsRawHandle;
    f.as_raw_handle() as isize
}

/// UnRAR's open hook: the descriptor or handle for volume `name` (UTF-8, `len` bytes), or -1.
/// It stays owned by the broker; UnRAR does not close it.
extern "C" fn open_hook(name: *const c_char, len: usize) -> isize {
    if name.is_null() || len > 4096 {
        return -1;
    }
    #[allow(unsafe_code)]
    // SAFETY: UnRAR passes `NameU.c_str()` and `NameU.size()` of a `std::string` that lives
    // until the hook returns: `len` initialised bytes at `name`, not written meanwhile.
    let bytes = unsafe { std::slice::from_raw_parts(name.cast::<u8>(), len) };
    let Ok(name) = std::str::from_utf8(bytes) else {
        return -1;
    };
    open(name)
}

fn open(name: &str) -> isize {
    let mut g = broker();
    let Some(b) = g.as_mut() else { return -1 };
    if let Some(i) = name.strip_prefix(PASSWORD_NAME) {
        let byte = i.parse::<usize>().ok().and_then(|i| {
            let p = b.volumes.password.as_ref()?;
            p.expose().as_bytes().get(i).copied()
        });
        return byte.map_or(-1, isize::from);
    }
    if name == b.volumes.first.0 {
        return raw(&b.volumes.first.1);
    }
    if let Some((_, f)) = b.more.iter().find(|(n, _)| n == name) {
        return f.as_ref().map_or(-1, raw);
    }
    if !valid_volume_name(name) || b.asked >= MAX_VOLUMES {
        return -1;
    }
    b.asked += 1;
    let f = (b.volumes.ask)(name);
    let r = f.as_ref().map_or(-1, raw);
    if f.is_some() && b.more.iter().filter(|v| v.1.is_some()).count() >= OPEN_VOLUMES {
        // UnRAR reads one volume at a time (it closes the previous one before opening the
        // next, and goes back only to the one it just had), so the oldest open volume can go;
        // asked for again, the viewer opens it again.
        if let Some(i) = b.more.iter().position(|v| v.1.is_some()) {
            b.more.remove(i);
        }
    }
    b.more.push((name.to_owned(), f));
    r
}

/// Runs `f` on the archive known as the first volume's name, with `volumes` behind the hook;
/// `f` learns whether a password was given.
fn with_volumes<T>(
    volumes: Volumes,
    f: impl FnOnce(&Path, bool) -> Result<T, Failure>,
) -> Result<T, Failure> {
    let name = volumes.first.0.clone();
    let password = volumes.password.is_some();
    // UnRAR could not be told about a NUL (it ends its strings), and no RAR tool makes one.
    if volumes
        .password
        .as_ref()
        .is_some_and(|p| p.expose().contains('\0'))
    {
        return Err((ErrorKind::BadPassword, "wrong password".into()));
    }
    *broker() = Some(Broker {
        volumes,
        more: Vec::new(),
        asked: 0,
    });
    let r = f(Path::new(&name), password);
    // UnRAR has closed (forgotten) every handle by now: the archive was dropped inside `f`.
    *broker() = None;
    r
}

pub type Failure = (ErrorKind, String);

/// What went wrong, knowing whether a `password` was given and whether the failure came from
/// `encrypted` data. RAR5 checks passwords; RAR4 does not, so a wrong one shows up as damage
/// in the encrypted data and is reported as a wrong password.
fn fail(e: UnrarError, password: bool, encrypted: bool) -> Failure {
    let kind = match e.code {
        Code::MissingPassword if !password => {
            return (
                ErrorKind::Encrypted,
                "encrypted, and no password given".into(),
            );
        }
        Code::MissingPassword | Code::BadPassword => {
            return (ErrorKind::BadPassword, "wrong password".into());
        }
        Code::BadData | Code::BadArchive if password && encrypted => {
            return (ErrorKind::BadPassword, "wrong password".into());
        }
        Code::NoMemory | Code::SmallBuf => ErrorKind::TooLarge,
        Code::BadData
        | Code::BadArchive
        | Code::UnknownFormat
        | Code::EndArchive
        | Code::EReference => ErrorKind::Malformed,
        Code::EOpen | Code::ERead | Code::EClose => ErrorKind::Io,
        _ => ErrorKind::Internal,
    };
    (kind, format!("unrar: {e}"))
}

/// [`fail`] for opening the archive or reading its headers. RAR 4 cannot tell a wrong
/// password for encrypted headers from damage, so with a password, headers that do not read
/// are a wrong password exactly when, without one, UnRAR asks for a password.
fn header_fail(e: UnrarError, password: bool, path: &Path) -> Failure {
    if password && matches!(e.code, Code::BadData | Code::BadArchive) {
        let saved = broker().as_mut().and_then(|b| b.volumes.password.take());
        let probe = unrar::Archive::new(path)
            .open_for_listing()
            .and_then(|mut a| a.next().transpose().map(|_| ()));
        if let Some(b) = broker().as_mut() {
            b.volumes.password = saved;
        }
        if probe.is_err_and(|e| e.code == Code::MissingPassword) {
            return (ErrorKind::BadPassword, "wrong password".into());
        }
    }
    fail(e, password, false)
}

fn entry(h: &unrar::FileHeader) -> ListEntry {
    let name = h.filename.to_string_lossy().into_owned();
    // UnRAR turns the archive's `/` into the native separator; entry names are `/`-separated
    // on every platform (as for ZIP and 7z), and a Windows name cannot contain `\` itself.
    #[cfg(windows)]
    let name = name.replace('\\', "/");
    ListEntry {
        name,
        size: h.unpacked_size,
        encrypted: h.is_encrypted(),
        directory: h.is_directory(),
    }
}

/// Lists the archive (at most `max_entries` entries).
pub fn list(volumes: Volumes, max_entries: u32) -> Result<Listing, Failure> {
    with_volumes(volumes, |p, pw| list_at(p, max_entries, pw))
}

/// Extracts entry `index` (counting every header, as [`list`] does), which must be called
/// `name`. Entries before it are skipped; in a solid archive UnRAR decompresses them on the
/// way (it has to).
pub fn read(volumes: Volumes, index: u32, name: &str, max_size: u64) -> Result<Vec<u8>, Failure> {
    with_volumes(volumes, |p, pw| read_at(p, index, name, max_size, pw))
}

fn list_at(path: &Path, max_entries: u32, pw: bool) -> Result<Listing, Failure> {
    let archive = unrar::Archive::new(path)
        .open_for_listing()
        .map_err(|e| header_fail(e, pw, path))?;
    let solid = archive.is_solid();
    let mut entries = Vec::new();
    for h in archive {
        let h = h.map_err(|e| header_fail(e, pw, path))?;
        if entries.len() >= max_entries as usize {
            return Err((
                ErrorKind::TooLarge,
                format!("archive has more than {max_entries} entries"),
            ));
        }
        entries.push(entry(&h));
    }
    Ok(Listing { solid, entries })
}

fn read_at(
    path: &Path,
    index: u32,
    name: &str,
    max_size: u64,
    pw: bool,
) -> Result<Vec<u8>, Failure> {
    let fail = |e| header_fail(e, pw, path);
    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(fail)?;
    let mut i = 0u32;
    loop {
        let Some(at) = archive.read_header().map_err(fail)? else {
            return Err((ErrorKind::Malformed, format!("entry {index} not found")));
        };
        // The continuation of a file split across volumes (listings leave these out; in
        // extraction mode UnRAR reports one header per volume piece).
        if at.entry().is_split_before() {
            archive = at.skip().map_err(fail)?;
            continue;
        }
        if i < index {
            archive = at.skip().map_err(fail)?;
            i += 1;
            continue;
        }
        let e = entry(at.entry());
        if e.name != name {
            return Err((
                ErrorKind::Malformed,
                format!("entry {index} is `{}`, not `{name}`", e.name),
            ));
        }
        if e.directory {
            return Err((ErrorKind::Unsupported, format!("`{name}` is a directory")));
        }
        if e.size > max_size {
            return Err((
                ErrorKind::TooLarge,
                format!("`{name}` has {} bytes, more than {max_size}", e.size),
            ));
        }
        let (data, _) = at.read().map_err(|x| self::fail(x, pw, e.encrypted))?;
        if data.len() as u64 > max_size {
            return Err((
                ErrorKind::TooLarge,
                format!("`{name}` exceeds {max_size} bytes"),
            ));
        }
        return Ok(data);
    }
}

/// Extracts the entries `items` (index and name as in [`read`], ascending) in one pass over
/// the archive: a solid archive is decompressed once for all of them, not once per entry.
/// Each is refused past `max_size` bytes, all of them together past `max_total`.
pub fn read_many(
    volumes: Volumes,
    items: &[(u32, String)],
    max_size: u64,
    max_total: u64,
) -> Result<Vec<Vec<u8>>, Failure> {
    if !items.windows(2).all(|w| w[0].0 < w[1].0) {
        return Err((ErrorKind::Protocol, "entries not in ascending order".into()));
    }
    with_volumes(volumes, |p, pw| {
        read_many_at(p, items, max_size, max_total, pw)
    })
}

fn read_many_at(
    path: &Path,
    items: &[(u32, String)],
    max_size: u64,
    max_total: u64,
    pw: bool,
) -> Result<Vec<Vec<u8>>, Failure> {
    let fail = |e| header_fail(e, pw, path);
    let mut out = Vec::with_capacity(items.len());
    if items.is_empty() {
        return Ok(out);
    }
    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(fail)?;
    let mut i = 0u32;
    let mut total = 0u64;
    while let Some((index, name)) = items.get(out.len()) {
        let Some(at) = archive.read_header().map_err(fail)? else {
            return Err((ErrorKind::Malformed, format!("entry {index} not found")));
        };
        if at.entry().is_split_before() {
            archive = at.skip().map_err(fail)?;
            continue;
        }
        if i < *index {
            archive = at.skip().map_err(fail)?;
            i += 1;
            continue;
        }
        let e = entry(at.entry());
        if e.name != *name {
            return Err((
                ErrorKind::Malformed,
                format!("entry {index} is `{}`, not `{name}`", e.name),
            ));
        }
        if e.directory {
            return Err((ErrorKind::Unsupported, format!("`{name}` is a directory")));
        }
        let allowed = max_size.min(max_total.saturating_sub(total));
        if e.size > allowed {
            return Err((
                ErrorKind::TooLarge,
                format!("`{name}` has {} bytes, more than {allowed}", e.size),
            ));
        }
        let (data, next) = at.read().map_err(|x| self::fail(x, pw, e.encrypted))?;
        if data.len() as u64 > allowed {
            return Err((
                ErrorKind::TooLarge,
                format!("`{name}` exceeds {allowed} bytes"),
            ));
        }
        total += data.len() as u64;
        out.push(data);
        archive = next;
        i += 1;
    }
    Ok(out)
}
