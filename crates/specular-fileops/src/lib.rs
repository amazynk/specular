//! File operations the user asks for explicitly: rename, copy and move to a folder, undo and
//! redo of those, and permanent rotation.
//!
//! The rules, shared by every operation (DESIGN.md §1, file handling):
//! - Nothing is overwritten without asking. A name that is taken is reported first
//!   ([`transfer::conflicts`], [`names::validate`]); "replace" sends the old file to the trash
//!   so it can come back.
//! - New names are created without clobbering (hard link then unlink, `O_EXCL`), so a race or a
//!   planted symbolic link never loses a file.
//! - Across file systems a move is copy, flush, compare, then delete; the original stays until
//!   the copy has been read back equal.
//! - Every change is journalled ([`history`]) with the size and modification time it left
//!   behind; undo refuses, with the reason, when the file has changed since.
//! - Sidecars travel with their image: `IMG_1.xmp`, `IMG_1.CR3.xmp`, `IMG_1.AAE`, and for a
//!   rename the other half of a RAW+JPEG pair.
//!
//! The functions block; the app runs them on its background workers.

pub mod atomic;
pub mod batch;
pub mod history;
pub mod names;
pub mod orient;
pub mod transfer;

pub use atomic::Stamp;
pub use history::{History, Op, Step, Trash, UndoError};
pub use names::{NameProblem, Platform};
pub use orient::{Method, RotateError, RotateOptions, Rotated, Transform, sidecar_orientation};
pub use transfer::{Conflict, Mode};
