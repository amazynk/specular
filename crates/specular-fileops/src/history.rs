//! The undo/redo journal for file operations.
//!
//! Each [`Op`] is a list of [`Step`]s exactly as they happened. Undo walks them backwards,
//! redo forwards; before either touches anything, every step is checked against the disk
//! (the file is still there with the size and modification time the operation left, the old
//! name is still free), so an undo either happens whole or not at all, with the reason.

use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::atomic::{self, Stamp};

/// Where "replace" sends the file it replaces, and where undo gets it back from. The app
/// passes the system trash; tests a folder.
pub trait Trash {
    fn trash(&self, path: &Path) -> io::Result<()>;
    /// Put the most recently trashed file that was at `path` back there.
    fn restore(&self, path: &Path) -> io::Result<()>;
}

/// What kind of operation, for the toast and the menu ("Undo rename").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Rename,
    BatchRename,
    Move,
    Copy,
    Rotate,
}

impl Kind {
    pub fn message_id(self) -> &'static str {
        match self {
            Kind::Rename => "fileop-kind-rename",
            Kind::BatchRename => "fileop-kind-batch-rename",
            Kind::Move => "fileop-kind-move",
            Kind::Copy => "fileop-kind-copy",
            Kind::Rotate => "fileop-kind-rotate",
        }
    }
}

/// One thing that happened to the disk.
#[derive(Clone, Debug)]
pub enum Step {
    /// A file was renamed or moved (across file systems: copied, verified, deleted).
    Moved {
        from: PathBuf,
        to: PathBuf,
        stamp: Stamp,
    },
    /// A copy was made. Undo sends the copy to the trash.
    Copied {
        from: PathBuf,
        to: PathBuf,
        stamp: Stamp,
    },
    /// A file in the way was sent to the trash ("replace"). Undo restores it, after the
    /// file that took its place has been moved away.
    Trashed { path: PathBuf },
    /// A file's bytes were rewritten (a rotation), or created (`before: None`, an XMP sidecar).
    Wrote {
        path: PathBuf,
        before: Option<Arc<[u8]>>,
        before_stamp: Option<Stamp>,
        after: Arc<[u8]>,
        after_stamp: Stamp,
    },
}

impl Step {
    fn bytes(&self) -> usize {
        match self {
            Step::Wrote { before, after, .. } => {
                before.as_ref().map_or(0, |b| b.len()) + after.len()
            }
            _ => 0,
        }
    }
}

/// One user action and everything it did.
#[derive(Clone, Debug)]
pub struct Op {
    pub kind: Kind,
    pub steps: Vec<Step>,
}

impl Op {
    pub fn new(kind: Kind) -> Op {
        Op {
            kind,
            steps: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// The main files the operation left behind (to select or show after it).
    pub fn results(&self) -> Vec<PathBuf> {
        self.steps
            .iter()
            .filter_map(|s| match s {
                Step::Moved { to, .. } | Step::Copied { to, .. } => Some(to.clone()),
                Step::Wrote { path, .. } => Some(path.clone()),
                Step::Trashed { .. } => None,
            })
            .collect()
    }
}

/// Why an undo or redo can't go ahead. Nothing was changed when one of these comes back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndoError {
    /// The file was changed (edited, replaced) since; undoing would lose that.
    Changed(PathBuf),
    /// The file is gone (deleted or moved elsewhere).
    Missing(PathBuf),
    /// The name it would go back to is used by another file now.
    Taken(PathBuf),
    /// The system refused (permissions, disk full, ...); carries its message.
    Io(PathBuf, String),
}

impl UndoError {
    pub fn message_id(&self) -> &'static str {
        match self {
            UndoError::Changed(_) => "undo-problem-changed",
            UndoError::Missing(_) => "undo-problem-missing",
            UndoError::Taken(_) => "undo-problem-taken",
            UndoError::Io(..) => "undo-problem-io",
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            UndoError::Changed(p)
            | UndoError::Missing(p)
            | UndoError::Taken(p)
            | UndoError::Io(p, _) => p,
        }
    }
}

fn io_err(p: &Path, e: io::Error) -> UndoError {
    UndoError::Io(p.to_path_buf(), e.to_string())
}

fn check_present(p: &Path, stamp: &Stamp) -> Result<(), UndoError> {
    if !atomic::taken(p) {
        return Err(UndoError::Missing(p.to_path_buf()));
    }
    if !stamp.matches(p) {
        return Err(UndoError::Changed(p.to_path_buf()));
    }
    Ok(())
}

fn check_free(p: &Path, unless_same_as: Option<&Path>) -> Result<(), UndoError> {
    if atomic::taken(p) && !unless_same_as.is_some_and(|o| atomic::same_file(o, p)) {
        return Err(UndoError::Taken(p.to_path_buf()));
    }
    Ok(())
}

/// Check that `op` can be undone now.
pub fn check_undo(op: &Op) -> Result<(), UndoError> {
    // Names freed by later steps count as free (a rename chain a→b, b→c undoes c→b first).
    // And names filled by earlier undo steps count as present (a staged temporary).
    let mut freed: Vec<&Path> = Vec::new();
    let mut created: Vec<&Path> = Vec::new();
    for s in op.steps.iter().rev() {
        match s {
            Step::Moved { from, to, stamp } => {
                if !created.contains(&to.as_path()) {
                    check_present(to, stamp)?;
                }
                if !freed.contains(&from.as_path()) {
                    check_free(from, Some(to))?;
                }
                freed.push(to);
                created.push(from);
            }
            Step::Copied { to, stamp, .. } => {
                check_present(to, stamp)?;
                freed.push(to);
            }
            Step::Trashed { path } => {
                if !freed.contains(&path.as_path()) {
                    check_free(path, None)?;
                }
            }
            Step::Wrote {
                path, after_stamp, ..
            } => check_present(path, after_stamp)?,
        }
    }
    Ok(())
}

/// Check that `op` (undone) can be redone now.
pub fn check_redo(op: &Op) -> Result<(), UndoError> {
    let mut freed: Vec<&Path> = Vec::new();
    let mut created: Vec<&Path> = Vec::new();
    for s in &op.steps {
        match s {
            Step::Moved { from, to, stamp } => {
                if !created.contains(&from.as_path()) {
                    check_present(from, stamp)?;
                }
                if !freed.contains(&to.as_path()) {
                    check_free(to, Some(from))?;
                }
                freed.push(from);
                created.push(to);
            }
            Step::Copied { from, to, stamp } => {
                // The source may have been edited since; the copy is made of what is there,
                // but a missing source can't be copied.
                let _ = stamp;
                if !atomic::taken(from) {
                    return Err(UndoError::Missing(from.clone()));
                }
                if !freed.contains(&to.as_path()) {
                    check_free(to, None)?;
                }
            }
            Step::Trashed { path } => {
                if !atomic::taken(path) {
                    return Err(UndoError::Missing(path.clone()));
                }
                freed.push(path);
            }
            Step::Wrote {
                path, before_stamp, ..
            } => match before_stamp {
                Some(s) => check_present(path, s)?,
                None => check_free(path, None)?,
            },
        }
    }
    Ok(())
}

/// Undo `op` (after [`check_undo`]). Returns the op to keep for redo: copies made again get
/// new stamps, so the op is updated as it goes.
pub fn undo(op: &Op, trash: &dyn Trash) -> Result<Op, UndoError> {
    check_undo(op)?;
    for s in op.steps.iter().rev() {
        match s {
            Step::Moved { from, to, .. } => {
                crate::transfer::move_file(to, from, false, &mut |_| true)
                    .map_err(|e| io_err(from, e))?;
            }
            Step::Copied { to, .. } => trash.trash(to).map_err(|e| io_err(to, e))?,
            Step::Trashed { path } => trash.restore(path).map_err(|e| io_err(path, e))?,
            Step::Wrote {
                path,
                before,
                before_stamp,
                ..
            } => match before {
                Some(b) => {
                    atomic::replace_file(path, b, false).map_err(|e| io_err(path, e))?;
                    if let Some(t) = before_stamp.and_then(|s| s.modified) {
                        atomic::set_modified(path, t).map_err(|e| io_err(path, e))?;
                    }
                }
                None => std::fs::remove_file(path).map_err(|e| io_err(path, e))?,
            },
        }
    }
    // Stamps of what is back in place (for redo's check).
    let mut back = op.clone();
    for s in &mut back.steps {
        match s {
            Step::Moved { from, stamp, .. } => *stamp = Stamp::of(from).unwrap_or(*stamp),
            Step::Wrote {
                path,
                before_stamp: Some(bs),
                ..
            } => *bs = Stamp::of(path).unwrap_or(*bs),
            _ => {}
        }
    }
    Ok(back)
}

/// Redo `op` (after [`check_redo`]); returns the op as now done, for the next undo.
pub fn redo(op: &Op, trash: &dyn Trash) -> Result<Op, UndoError> {
    check_redo(op)?;
    let mut done = op.clone();
    for s in &mut done.steps {
        match s {
            Step::Moved { from, to, stamp } => {
                crate::transfer::move_file(from, to, false, &mut |_| true)
                    .map_err(|e| io_err(to, e))?;
                *stamp = Stamp::of(to).map_err(|e| io_err(to, e))?;
            }
            Step::Copied { from, to, stamp } => {
                atomic::copy_verified(from, to, &mut |_| true).map_err(|e| io_err(to, e))?;
                *stamp = Stamp::of(to).map_err(|e| io_err(to, e))?;
            }
            Step::Trashed { path } => trash.trash(path).map_err(|e| io_err(path, e))?,
            Step::Wrote {
                path,
                before,
                after,
                after_stamp,
                ..
            } => {
                let keep = after_stamp.modified;
                if before.is_some() {
                    atomic::replace_file(path, after, false).map_err(|e| io_err(path, e))?;
                } else {
                    atomic::write_new(path, after).map_err(|e| io_err(path, e))?;
                }
                if let Some(t) = keep {
                    let _ = atomic::set_modified(path, t);
                }
                *after_stamp = Stamp::of(path).map_err(|e| io_err(path, e))?;
            }
        }
    }
    Ok(done)
}

/// The journal: operations by the app's undo token, newest last, and the redo stack.
/// Rotations keep the file's bytes from before and after; past `byte_cap` the oldest such
/// entries are forgotten (their undo then reports nothing to undo).
#[derive(Debug)]
pub struct History {
    done: VecDeque<(u64, Op)>,
    undone: Vec<(u64, Op)>,
    max_ops: usize,
    byte_cap: usize,
}

impl Default for History {
    fn default() -> History {
        History::new(100, 512 << 20)
    }
}

impl History {
    pub fn new(max_ops: usize, byte_cap: usize) -> History {
        History {
            done: VecDeque::new(),
            undone: Vec::new(),
            max_ops,
            byte_cap,
        }
    }

    /// Record a finished operation under `token`. A new operation clears the redo stack.
    pub fn push(&mut self, token: u64, op: Op) {
        if op.is_empty() {
            return;
        }
        self.undone.clear();
        self.done.push_back((token, op));
        while self.done.len() > self.max_ops {
            self.done.pop_front();
        }
        while self.done.len() > 1 && self.bytes() > self.byte_cap {
            self.done.pop_front();
        }
    }

    fn bytes(&self) -> usize {
        self.done
            .iter()
            .chain(&self.undone)
            .flat_map(|(_, o)| &o.steps)
            .map(Step::bytes)
            .sum()
    }

    pub fn contains(&self, token: u64) -> bool {
        self.done.iter().any(|(t, _)| *t == token)
    }

    pub fn last_token(&self) -> Option<u64> {
        self.done.back().map(|(t, _)| *t)
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    pub fn peek(&self, token: u64) -> Option<&Op> {
        self.done.iter().find(|(t, _)| *t == token).map(|(_, o)| o)
    }

    /// Take operation `token` out for undoing (it goes back with [`History::undone`] or
    /// [`History::restore`] when the undo fails).
    pub fn take(&mut self, token: u64) -> Option<Op> {
        let i = self.done.iter().position(|(t, _)| *t == token)?;
        self.done.remove(i).map(|(_, o)| o)
    }

    /// Put back an operation whose undo failed.
    pub fn restore(&mut self, token: u64, op: Op) {
        self.done.push_back((token, op));
    }

    /// Record a successful undo, for redo.
    pub fn undone(&mut self, token: u64, op: Op) {
        self.undone.push((token, op));
    }

    /// The operation to redo next.
    pub fn take_redo(&mut self) -> Option<(u64, Op)> {
        self.undone.pop()
    }

    /// Put back a redo that failed.
    pub fn restore_redo(&mut self, token: u64, op: Op) {
        self.undone.push((token, op));
    }

    /// Record a successful redo (it can be undone again; the redo stack is kept).
    pub fn redone(&mut self, token: u64, op: Op) {
        self.done.push_back((token, op));
    }

    /// How many operations can be undone.
    pub fn len(&self) -> usize {
        self.done.len()
    }

    pub fn is_empty(&self) -> bool {
        self.done.is_empty()
    }
}

/// The modification time to keep, for callers that build [`Step::Wrote`].
pub fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}
