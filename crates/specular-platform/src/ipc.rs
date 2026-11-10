//! The local IPC transport (DESIGN.md §11, "IPC socket"): a stream socket that only the user
//! running Specular can reach. The protocol spoken over it lives in the app; this module finds
//! the place for the socket, binds it, checks who connects, and reads bounded lines.
//!
//! * **Linux and BSD:** a Unix domain socket in `$XDG_RUNTIME_DIR/specular/` (the folder is
//!   created mode 0700). When `$XDG_RUNTIME_DIR` is missing, not ours or not private, the
//!   caller's fallback folder is used instead, again through a private 0700 subfolder.
//! * **macOS:** the same, with the per-user `$TMPDIR` (`/var/folders/…/T/`) before the fallback.
//! * **Windows:** an `AF_UNIX` socket (Windows 10 1803 and later) in the caller's fallback
//!   folder under `%LOCALAPPDATA%`, which only the user (and SYSTEM and administrators) can
//!   open: the folder's ACL is the authentication, as the 0700 folder is on Unix. This needs no
//!   `unsafe`, unlike a named pipe with its own security descriptor. When the fallback is too
//!   deep for a socket path (a portable copy's own folder), `%LOCALAPPDATA%\Specular\run`.
//!
//! Authentication is by the file system: nobody else can reach a socket inside a folder only
//! its owner may enter. On Unix every accepted connection's peer is also checked (`SO_PEERCRED`
//! / `getpeereid` through the standard library) to be the same user; others are dropped. On
//! Windows, which has no such check without `unsafe`, a [`KeyFile`] next to the socket stands
//! in: a client must send its key before anything else.
//!
//! Nothing here listens on a network port, now or later.
//!
//! The first viewer binds [`SOCKET_NAME`]; when that one is in use by a live viewer, later
//! viewers bind `specular-<pid>.sock` next to it. A socket file left behind by a viewer that
//! crashed (nobody accepts on it) is removed and bound again.

use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
use std::os::windows::net::{UnixListener, UnixStream};

/// The socket of the first (default) viewer, inside [`socket_dir`].
pub const SOCKET_NAME: &str = "specular.sock";

/// Socket paths longer than this do not fit `sockaddr_un` everywhere (104 bytes on macOS and
/// the BSDs, 108 on Linux and Windows, including the terminating NUL).
const MAX_PATH_BYTES: usize = 100;

/// The longest request or response line accepted, in bytes (without the newline).
pub const MAX_LINE: usize = 1 << 20;

/// A connected stream (client or server side).
pub type Stream = UnixStream;

/// The folder the sockets live in: the first usable candidate, made private (0700 on Unix) if
/// Specular created it. `fallback` is used when the platform's runtime folder is missing or
/// unsafe; it is created with its parents.
pub fn socket_dir(fallback: &Path) -> io::Result<PathBuf> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, "no folder for the IPC socket");
    for c in candidates(fallback) {
        // Checked before the folder is made: a folder too deep to use is not created.
        if !fits(&c) {
            last = io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{}: path too long for a socket", c.display()),
            );
            continue;
        }
        match prepare_dir(&c) {
            Ok(()) => return Ok(c),
            Err(e) => {
                specular_log::debug!(target: specular_log::SETTINGS, "ipc: {}: {e}", c.display());
                last = e;
            }
        }
    }
    Err(last)
}

/// Whether a socket in `dir` (the default name or `specular-<pid>.sock`) fits a socket path.
fn fits(dir: &Path) -> bool {
    dir.join(SOCKET_NAME).as_os_str().len() + 12 <= MAX_PATH_BYTES
}

/// A fresh, empty private folder named `<tag>-<pid>` whose sockets fit a socket path, apart
/// from the viewer's own folder (for tests and tools that bind sockets of their own).
///
/// In the session's runtime folder when it is private, else in the temporary folder, else in
/// `/tmp` (a long `$TMPDIR` is common in build sandboxes). Made as [`prepare_dir`] makes it.
// Bases pushed one by one: which there are depends on the platform.
#[cfg_attr(not(unix), allow(clippy::vec_init_then_push))]
pub fn scratch_dir(tag: &str) -> io::Result<PathBuf> {
    let name = format!("{tag}-{}", std::process::id());
    let mut bases = Vec::new();
    #[cfg(unix)]
    if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
        && d.is_absolute()
        && runtime_dir_is_private(&d)
    {
        bases.push(d);
    }
    bases.push(std::env::temp_dir());
    #[cfg(unix)]
    bases.push(PathBuf::from("/tmp"));
    let d = bases
        .into_iter()
        .map(|b| b.join(&name))
        .find(|d| d.is_absolute() && fits(d))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "no folder short enough for a socket",
            )
        })?;
    if dir_is_safe(&d) {
        std::fs::remove_dir_all(&d)?;
    }
    prepare_dir(&d)?;
    Ok(d)
}

/// The default viewer's socket, if one exists in a folder that passes the checks of
/// [`socket_dir`]. Creates and changes nothing (for clients).
pub fn existing_socket(fallback: &Path) -> Option<PathBuf> {
    candidates(fallback)
        .into_iter()
        .filter(|c| dir_is_safe(c))
        .map(|c| c.join(SOCKET_NAME))
        .find(|s| s.exists())
}

#[cfg(unix)]
fn dir_is_safe(dir: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(dir)
        .is_ok_and(|m| m.is_dir() && m.uid() == uid() && m.mode() & 0o077 == 0)
}

#[cfg(windows)]
fn dir_is_safe(dir: &Path) -> bool {
    std::fs::symlink_metadata(dir).is_ok_and(|m| m.is_dir())
}

/// Candidate folders, best first.
// Off Unix the pushes follow the `Vec::new` at once.
#[cfg_attr(not(unix), allow(clippy::vec_init_then_push))]
fn candidates(fallback: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(unix)]
    {
        if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
            && d.is_absolute()
            && runtime_dir_is_private(&d)
        {
            v.push(d.join("specular"));
        }
        #[cfg(target_os = "macos")]
        if let Some(d) = std::env::var_os("TMPDIR").map(PathBuf::from)
            && d.is_absolute()
            && runtime_dir_is_private(&d)
        {
            v.push(d.join(format!("specular-{}", uid())));
        }
    }
    v.push(fallback.to_path_buf());
    // Unix: a short folder for when the runtime folder is missing and the fallback is too deep
    // for a socket path (a deep home folder). `specular-<uid>` in the temporary folder, made
    // 0700 and refused when it is someone else's or a symbolic link (`prepare_dir`).
    // Removed again on exit when empty (`SocketFile`).
    #[cfg(unix)]
    for d in temp_socket_dirs() {
        if !v.contains(&d) {
            v.push(d);
        }
    }
    // Windows: a short folder for when the fallback is too deep for a socket path (a portable
    // copy unpacked deep in Downloads). In the user's profile like the fallback, so as private:
    // not `%TEMP%`, which may be pointed at a shared folder. Removed again on exit when empty
    // (`SocketFile`), so a portable copy leaves nothing there.
    #[cfg(windows)]
    if let Some(d) = short_dir() {
        v.push(d);
    }
    v
}

/// Unix: the short socket folders in the temporary folder and in `/tmp` (see [`candidates`]).
#[cfg(unix)]
fn temp_socket_dirs() -> Vec<PathBuf> {
    [std::env::temp_dir(), PathBuf::from("/tmp")]
        .into_iter()
        .filter(|t| t.is_absolute())
        .map(|t| t.join(format!("specular-{}", uid())))
        .collect()
}

/// Unix: a short folder in the temporary folder goes with its last socket (`remove_dir` only
/// removes empty folders: another viewer's socket keeps it).
#[cfg(unix)]
fn remove_temp_dir_if_empty(socket: &Path) {
    if let Some(dir) = socket.parent()
        && temp_socket_dirs().iter().any(|d| d == dir)
    {
        let _ = std::fs::remove_dir(dir);
    }
}

/// Windows: `%LOCALAPPDATA%\Specular\run`, the short socket folder (see [`candidates`]).
#[cfg(windows)]
fn short_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|d| d.is_absolute())
        .map(|d| d.join("Specular").join("run"))
}

/// Windows: removes the short socket folder once its last socket is gone, and `Specular` above
/// it when that is empty too (an installed copy keeps its caches there, so it stays).
#[cfg(windows)]
fn remove_short_dir_if_empty(socket: &Path) {
    if let Some(short) = short_dir() {
        remove_if_empty(socket, &short);
    }
}

#[cfg(windows)]
fn remove_if_empty(socket: &Path, short: &Path) {
    // `remove_dir` only removes empty folders: another viewer's socket keeps them.
    if socket.parent() == Some(short)
        && std::fs::remove_dir(short).is_ok()
        && let Some(parent) = short.parent()
    {
        let _ = std::fs::remove_dir(parent);
    }
}

/// `$XDG_RUNTIME_DIR` must be a folder owned by us that nobody else may enter (the XDG base
/// directory specification requires mode 0700).
#[cfg(unix)]
fn runtime_dir_is_private(d: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(d)
        .or_else(|_| std::fs::metadata(d))
        .is_ok_and(|m| m.is_dir() && m.uid() == uid() && m.mode() & 0o077 == 0)
}

#[cfg(unix)]
fn uid() -> u32 {
    rustix::process::getuid().as_raw()
}

/// Creates `dir` (0700 on Unix) and checks it is a real folder, owned by us, private.
#[cfg(unix)]
pub fn prepare_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    // Not followed: a symlink planted in our place is refused, not trusted.
    let m = std::fs::symlink_metadata(dir)?;
    if !m.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a folder (or a symbolic link)",
        ));
    }
    if m.uid() != uid() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "owned by another user",
        ));
    }
    if m.mode() & 0o077 != 0 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Windows: the folder inherits the per-user ACL of `%LOCALAPPDATA%`.
#[cfg(windows)]
pub fn prepare_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    if std::fs::symlink_metadata(dir)?.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a symbolic link",
        ));
    }
    Ok(())
}

/// A bound socket accepting connections from the same user only.
pub struct Listener {
    listener: UnixListener,
    path: PathBuf,
}

/// Removes the socket file when the viewer goes away (if it is still ours).
pub struct SocketFile {
    path: PathBuf,
    #[cfg(unix)]
    ident: (u64, u64),
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // Another viewer may have replaced a file it took for stale: leave that one.
            let ours = std::fs::symlink_metadata(&self.path)
                .is_ok_and(|m| (m.dev(), m.ino()) == self.ident);
            if !ours {
                return;
            }
        }
        let _ = std::fs::remove_file(&self.path);
        #[cfg(windows)]
        remove_short_dir_if_empty(&self.path);
        #[cfg(unix)]
        remove_temp_dir_if_empty(&self.path);
    }
}

impl SocketFile {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Binds the default socket in `dir`, or `specular-<pid>.sock` when a live viewer owns the
/// default one. Returns the listener and the guard that removes the file.
pub fn bind(dir: &Path) -> io::Result<(Listener, SocketFile)> {
    match bind_at(&dir.join(SOCKET_NAME)) {
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            bind_at(&dir.join(format!("specular-{}.sock", std::process::id())))
        }
        r => r,
    }
}

/// Binds exactly `path`, replacing a stale socket file nobody accepts on.
pub fn bind_at(path: &Path) -> io::Result<(Listener, SocketFile)> {
    let listener = match UnixListener::bind(path) {
        Ok(l) => l,
        Err(e) if e.kind() == io::ErrorKind::AddrInUse || path.exists() => {
            if UnixStream::connect(path).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "another viewer is listening",
                ));
            }
            if !is_socket(path) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "a file that is not a socket is in the way",
                ));
            }
            specular_log::debug!(target: specular_log::SETTINGS, "ipc: removing the stale socket {}", path.display());
            std::fs::remove_file(path)?;
            UnixListener::bind(path)?
        }
        Err(e) => return Err(e),
    };
    #[cfg(unix)]
    let ident = {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        // The folder already keeps others out; the socket itself is user-only as well.
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        let m = std::fs::symlink_metadata(path)?;
        (m.dev(), m.ino())
    };
    Ok((
        Listener {
            listener,
            path: path.to_path_buf(),
        },
        SocketFile {
            path: path.to_path_buf(),
            #[cfg(unix)]
            ident,
        },
    ))
}

#[cfg(unix)]
fn is_socket(p: &Path) -> bool {
    use std::os::unix::fs::FileTypeExt;
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_socket())
}

/// Windows reports `AF_UNIX` socket files as reparse points; anything we cannot connect to
/// that is not a regular file with content is taken for a stale socket.
#[cfg(windows)]
fn is_socket(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| !m.is_dir() && m.len() == 0)
}

impl Listener {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Waits for the next connection from this user. Connections from anyone else (only
    /// possible for root, or through a misconfigured folder) are closed and skipped.
    pub fn accept(&self) -> io::Result<Stream> {
        loop {
            let (s, _) = self.listener.accept()?;
            match peer_is_us(&s) {
                Ok(true) => return Ok(s),
                Ok(false) => {
                    specular_log::warn!(target: specular_log::SETTINGS, "ipc: refused a connection from another user");
                }
                Err(e) => {
                    specular_log::warn!(target: specular_log::SETTINGS, "ipc: peer check failed ({e}); connection refused");
                }
            }
        }
    }
}

/// Whether the process at the other end runs as the same user.
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn peer_is_us(s: &Stream) -> io::Result<bool> {
    Ok(s.peer_cred()?.uid == uid())
}

/// Elsewhere (Windows) the folder's permissions are the check, with the [`KeyFile`] the
/// server asks clients for.
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
)))]
fn peer_is_us(_s: &Stream) -> io::Result<bool> {
    Ok(true)
}

/// A secret next to a socket (`<socket>.key`) that only the user can read: a client proves
/// it runs as the user by sending it. Windows has no peer check for `AF_UNIX` (it would need
/// `unsafe`), so there the server asks for this key before anything else; the folder's ACL
/// keeps it from other users. Removed when dropped.
pub struct KeyFile {
    path: PathBuf,
    key: String,
}

impl KeyFile {
    /// Writes a fresh random key (256 bits, hex) next to `socket`, replacing an old file.
    pub fn create(socket: &Path) -> io::Result<KeyFile> {
        let mut b = [0u8; 32];
        getrandom::fill(&mut b).map_err(|e| io::Error::other(e.to_string()))?;
        let key: String = b.iter().map(|x| format!("{x:02x}")).collect();
        let mut path = socket.as_os_str().to_owned();
        path.push(".key");
        let path = PathBuf::from(path);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        let mut f = o.open(&path)?;
        f.write_all(key.as_bytes())?;
        f.flush()?;
        Ok(KeyFile { path, key })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether `given` is the key (compared in constant time).
    pub fn matches(&self, given: &str) -> bool {
        let (a, b) = (self.key.as_bytes(), given.trim().as_bytes());
        a.len() == b.len() && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
    }
}

impl Drop for KeyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Connects to the viewer listening at `path`, with `timeout` for every read and write.
pub fn connect(path: &Path, timeout: Duration) -> io::Result<Stream> {
    let s = UnixStream::connect(path)?;
    s.set_read_timeout(Some(timeout))?;
    s.set_write_timeout(Some(timeout))?;
    Ok(s)
}

/// One line read by [`read_line`].
#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    /// The line without its `\n` (and without a `\r` before it).
    Text(Vec<u8>),
    /// The line was longer than allowed; the rest of the stream cannot be trusted.
    TooLong,
    /// The other side closed the connection (a last line without `\n` is still returned).
    End,
}

/// Reads one `\n`-terminated line of at most `max` bytes. Never buffers more than `max + 1`
/// bytes of a line, whatever the peer sends.
pub fn read_line(r: &mut impl BufRead, max: usize) -> io::Result<Line> {
    let mut buf = Vec::new();
    let n = r.take(max as u64 + 1).read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Ok(Line::End);
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
        if buf.last() == Some(&b'\r') {
            buf.pop();
        }
    } else if buf.len() > max {
        return Ok(Line::TooLong);
    }
    Ok(Line::Text(buf))
}

/// Writes `line` and a newline in one call (so concurrent writers never interleave halves).
pub fn write_line(w: &mut impl Write, line: &[u8]) -> io::Result<()> {
    let mut v = Vec::with_capacity(line.len() + 1);
    v.extend_from_slice(line);
    v.push(b'\n');
    w.write_all(&v)?;
    w.flush()
}

/// Drains whatever a misbehaving peer still sends, up to `limit` bytes (so closing does not
/// reset the connection before our error reply is read).
pub fn drain(r: &mut impl Read, limit: u64) {
    let _ = io::copy(&mut r.take(limit), &mut io::sink());
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;

    /// A fallback too deep for a socket path is followed by a short folder in the profile.
    #[cfg(windows)]
    #[test]
    fn a_short_folder_after_a_deep_fallback() {
        let deep = PathBuf::from(r"C:\Users\u\Downloads").join("x".repeat(100));
        assert!(deep.join(SOCKET_NAME).as_os_str().len() > MAX_PATH_BYTES);
        let c = candidates(&deep);
        assert_eq!(c.first(), Some(&deep), "the caller's folder first");
        let short = c.get(1).expect("a second folder");
        assert!(short.ends_with(r"Specular\run"), "{short:?}");
        assert!(short.join(SOCKET_NAME).as_os_str().len() + 12 <= MAX_PATH_BYTES);
    }

    /// The short folder outside a portable copy goes once its last socket is gone, and
    /// `Specular` above it too unless something else (an installed copy's caches) is there.
    #[cfg(windows)]
    #[test]
    fn the_short_folder_goes_when_empty() {
        let root = std::env::temp_dir().join(format!("specular-ipc-short-{}", std::process::id()));
        let app = root.join("Specular");
        let short = app.join("run");
        let socket = short.join(SOCKET_NAME);
        std::fs::create_dir_all(&short).unwrap();
        std::fs::write(short.join("specular-1.sock"), b"").unwrap();
        remove_if_empty(&socket, &short);
        assert!(short.is_dir(), "another viewer's socket keeps it");
        std::fs::remove_file(short.join("specular-1.sock")).unwrap();
        std::fs::create_dir_all(app.join("cache")).unwrap();
        remove_if_empty(&socket, &short);
        assert!(
            !short.exists() && app.is_dir(),
            "an installed copy's caches keep Specular"
        );
        std::fs::create_dir_all(&short).unwrap();
        std::fs::remove_dir(app.join("cache")).unwrap();
        remove_if_empty(&socket, &short);
        assert!(!app.exists(), "a portable copy leaves nothing");
        remove_if_empty(&root.join("elsewhere").join(SOCKET_NAME), &short);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    fn test_dir(name: &str) -> PathBuf {
        scratch_dir(&format!("ipc-{name}")).unwrap()
    }

    /// With no runtime folder and a fallback too deep for a socket path, a short private
    /// folder in the temporary folder (or `/tmp`) still fits.
    #[cfg(unix)]
    #[test]
    fn a_short_folder_after_a_deep_fallback_on_unix() {
        let deep = PathBuf::from("/home/u").join("x".repeat(100));
        assert!(!fits(&deep));
        let c = candidates(&deep);
        let at = c.iter().position(|d| d == &deep).unwrap();
        let short = &c[at + 1..];
        assert!(short.iter().any(|d| fits(d)), "{short:?}");
        let name = format!("specular-{}", uid());
        assert!(short.iter().all(|d| d.ends_with(&name)), "{short:?}");
    }

    /// The short folder in the temporary folder goes once its last socket is gone; any
    /// other folder stays.
    #[cfg(unix)]
    #[test]
    fn the_temp_folder_goes_when_empty() {
        let d = temp_socket_dirs().remove(0);
        let had = d.exists();
        if !had {
            prepare_dir(&d).unwrap();
        }
        let other = d.join("specular-1.sock");
        std::fs::write(&other, b"").unwrap();
        remove_temp_dir_if_empty(&d.join(SOCKET_NAME));
        assert!(d.is_dir(), "another viewer's socket keeps it");
        std::fs::remove_file(&other).unwrap();
        if !had {
            remove_temp_dir_if_empty(&d.join(SOCKET_NAME));
            assert!(!d.exists(), "gone with its last socket");
        }
        let elsewhere = scratch_dir("ipc-keep").unwrap();
        remove_temp_dir_if_empty(&elsewhere.join(SOCKET_NAME));
        assert!(elsewhere.is_dir());
        std::fs::remove_dir(&elsewhere).unwrap();
    }

    /// A scratch folder is private, empty, and short enough for a socket.
    #[cfg(unix)]
    #[test]
    fn scratch_folders_fit_a_socket() {
        let d = scratch_dir("ipc-scratch").unwrap();
        assert!(fits(&d) && dir_is_safe(&d), "{d:?}");
        std::fs::write(d.join("x"), b"").unwrap();
        let again = scratch_dir("ipc-scratch").unwrap();
        assert_eq!(again, d);
        assert!(std::fs::read_dir(&d).unwrap().next().is_none(), "emptied");
        std::fs::remove_dir(&d).unwrap();
    }

    /// The key is fresh each time, private, compared exactly, and gone with its guard.
    #[test]
    fn key_files() {
        let d = scratch_dir("ipc-key").unwrap();
        let sock = d.join(SOCKET_NAME);
        let k = KeyFile::create(&sock).unwrap();
        assert_eq!(k.path(), d.join("specular.sock.key"));
        let text = std::fs::read_to_string(k.path()).unwrap();
        assert_eq!(text.len(), 64);
        assert!(k.matches(&text) && k.matches(&format!("{text}\n")));
        assert!(!k.matches("") && !k.matches(&text[1..]) && !k.matches(&text.to_uppercase()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let m = std::fs::metadata(k.path()).unwrap().permissions().mode();
            assert_eq!(m & 0o777, 0o600);
        }
        let k2 = KeyFile::create(&sock).unwrap();
        assert!(!k2.matches(&text), "a new key each time");
        drop(k);
        drop(k2);
        assert!(!d.join("specular.sock.key").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn lines_are_bounded() {
        let mut r = io::Cursor::new(b"abc\r\ndef\nlonglonglong\nlast".to_vec());
        assert_eq!(read_line(&mut r, 8).unwrap(), Line::Text(b"abc".to_vec()));
        assert_eq!(read_line(&mut r, 8).unwrap(), Line::Text(b"def".to_vec()));
        assert_eq!(read_line(&mut r, 8).unwrap(), Line::TooLong);
        let mut r = io::Cursor::new(b"exactly8\nlast".to_vec());
        assert_eq!(
            read_line(&mut r, 8).unwrap(),
            Line::Text(b"exactly8".to_vec())
        );
        assert_eq!(read_line(&mut r, 8).unwrap(), Line::Text(b"last".to_vec()));
        assert_eq!(read_line(&mut r, 8).unwrap(), Line::End);
        // A huge line without a newline never grows the buffer past the limit.
        let mut r = io::Cursor::new(vec![b'x'; 10 << 20]);
        assert_eq!(read_line(&mut r, 1024).unwrap(), Line::TooLong);
        assert_eq!(r.position(), 1025);
    }

    #[cfg(unix)]
    #[test]
    fn private_folder_and_stale_sockets() {
        use std::os::unix::fs::PermissionsExt;
        let d = test_dir("bind");
        // An existing folder that is too open is tightened.
        std::fs::create_dir_all(&d).unwrap();
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o755)).unwrap();
        // (Not `socket_dir`, which would pick the session's runtime folder first.)
        prepare_dir(&d).unwrap();
        let mode = std::fs::metadata(&d).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        let sub = d.join("s");
        prepare_dir(&sub).unwrap();
        let (l, guard) = bind(&sub).unwrap();
        assert_eq!(l.path(), sub.join(SOCKET_NAME));
        let mode = std::fs::metadata(l.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        // A live listener: the second viewer gets its own socket.
        let (l2, guard2) = bind(&sub).unwrap();
        assert_eq!(
            l2.path(),
            sub.join(format!("specular-{}.sock", std::process::id()))
        );
        // Talk through it: the peer check passes for ourselves. (The second bind's liveness
        // probe is queued first: a connection that closes without a line.)
        let t = std::thread::spawn(move || {
            loop {
                let mut s = l.accept().unwrap();
                let mut r = io::BufReader::new(s.try_clone().unwrap());
                if let Line::Text(t) = read_line(&mut r, 64).unwrap() {
                    write_line(&mut s, &t).unwrap();
                    return l;
                }
            }
        });
        let mut c = connect(&sub.join(SOCKET_NAME), Duration::from_secs(5)).unwrap();
        write_line(&mut c, b"ping").unwrap();
        let mut r = io::BufReader::new(c);
        assert_eq!(read_line(&mut r, 64).unwrap(), Line::Text(b"ping".to_vec()));
        let l = t.join().unwrap();
        // A crashed viewer's socket (nobody listening) is replaced.
        drop(l);
        std::mem::forget(guard); // the file stays, as after a crash
        assert!(sub.join(SOCKET_NAME).exists());
        // Another test in this binary that spawns a process can hold a copy of the closed
        // listener between its fork and exec, so the socket briefly still answers.
        let (_l3, guard3) = (0..200)
            .find_map(|_| {
                bind(&sub).ok().or_else(|| {
                    std::thread::sleep(Duration::from_millis(10));
                    None
                })
            })
            .unwrap();
        assert!(connect(&sub.join(SOCKET_NAME), Duration::from_secs(1)).is_ok());
        drop(guard3);
        assert!(!sub.join(SOCKET_NAME).exists());
        drop(guard2);
        // A regular file in the way is never deleted.
        std::fs::write(sub.join(SOCKET_NAME), b"not a socket").unwrap();
        assert!(bind_at(&sub.join(SOCKET_NAME)).is_err());
        assert!(sub.join(SOCKET_NAME).is_file());
        // A symbolic link in place of the folder is refused.
        let link = d.join("link");
        std::os::unix::fs::symlink(&sub, &link).unwrap();
        assert!(prepare_dir(&link).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
