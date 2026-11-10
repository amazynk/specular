//! Import from cameras, phones and memory cards.
//!
//! * [`source`]: volumes with a DCIM folder (cards, cameras in mass-storage mode, phones and
//!   cameras over MTP/PTP through gvfs on Linux), and a watcher thread that reports arrivals.
//! * [`scan`]: the media under a source's DCIM folders, grouped into shots (a RAW and its JPEG,
//!   a Live Photo's still and movie) with their sidecars (`.xmp`, `.aae`, `.thm`, `.lrv`),
//!   capture date and camera, and a quick content key per file.
//! * [`ledger`]: what was imported before, keyed by content (not by where it went), so a file
//!   renamed or moved at the destination is still not imported twice.
//! * [`pattern`]: destination folders (`{year}/{year}-{month}-{day}`, `{camera}`, `{event}`)
//!   and optional renames through the batch-convert rename pattern.
//! * [`copy`]: copy to a hidden temporary name, fsync, verify the hash, link under the final
//!   name (never replacing a file), keep the modification time; a partial copy resumes.
//! * [`run`]: the plan (what goes where), the free-space check, and the copy loop with
//!   progress, pause, cancel, delete-after and a report.
//!
//! Nothing here writes to a source except "delete from card after import", which removes a
//! file only after its copy was verified.

pub mod copy;
mod exif;
pub mod ledger;
pub mod pattern;
pub mod run;
pub mod scan;
pub mod source;

pub use copy::Control;
pub use ledger::Ledger;
pub use pattern::{FolderPattern, PatternError, RenamePattern};
pub use run::{Options, Plan, Progress, Report, RunError};
pub use scan::{Media, MediaKind, Shot};
pub use source::Source;

/// A content key: blake3 of a file's first and last 64 KiB, its size and its recorded
/// capture date (see [`scan::quick_key`]).
pub type Key = [u8; 32];

/// A full blake3 hash of a file's bytes.
pub type Hash = [u8; 32];

/// `bytes` as lowercase hex.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// 64 hex digits back to 32 bytes.
pub fn unhex(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        let d = |c: u8| (c as char).to_digit(16);
        *o = (d(b[2 * i])? * 16 + d(b[2 * i + 1])?) as u8;
    }
    Some(out)
}

#[cfg(test)]
pub(crate) mod testutil;
