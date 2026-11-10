//! File I/O advice through `fcntl`.

use std::fs::File;
use std::os::fd::AsRawFd;
use std::sync::OnceLock;

/// `SPECULAR_RDADVISE=0` turns [`rdadvise`] off, for A/B timing on a real Mac.
fn rdadvise_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SPECULAR_RDADVISE").is_none_or(|v| v != "0"))
}

/// `F_RDADVISE`: asynchronous read-ahead of a byte range into the unified buffer cache, which
/// backs both `read` and memory maps on macOS. One call covers at most 2 GiB (`ra_count` is an
/// `int`); larger files get several.
pub(crate) fn rdadvise(file: &File, offset: u64, len: u64) {
    if !rdadvise_enabled() {
        return;
    }
    let (mut off, end) = (offset, offset.saturating_add(len));
    while off < end {
        let count = (end - off).min(i32::MAX as u64 & !0xFFFF);
        let ra = libc::radvisory {
            ra_offset: off as libc::off_t,
            ra_count: count as libc::c_int,
        };
        if fcntl_rdadvise(file, &ra) != 0 {
            return;
        }
        off += count;
    }
}

#[allow(unsafe_code)]
fn fcntl_rdadvise(file: &File, ra: &libc::radvisory) -> libc::c_int {
    // SAFETY: `file` keeps the descriptor open for the call; F_RDADVISE reads one
    // `struct radvisory` through the pointer, which points to a live, initialised value. The
    // call only queues read-ahead and reports failure as -1.
    unsafe {
        libc::fcntl(
            file.as_raw_fd(),
            libc::F_RDADVISE,
            ra as *const libc::radvisory,
        )
    }
}

/// Sets an on/off `fcntl` flag (`F_NOCACHE`, `F_RDAHEAD`); whether it took effect.
#[allow(unsafe_code)]
pub(crate) fn fcntl_flag(file: &File, cmd: libc::c_int, on: bool) -> bool {
    debug_assert!(cmd == libc::F_NOCACHE || cmd == libc::F_RDAHEAD);
    // SAFETY: `file` keeps the descriptor open for the call; F_NOCACHE and F_RDAHEAD take one
    // `int` argument and change only per-descriptor caching behaviour (no memory is touched).
    unsafe { libc::fcntl(file.as_raw_fd(), cmd, libc::c_int::from(on)) != -1 }
}
