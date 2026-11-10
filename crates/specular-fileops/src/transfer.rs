//! Rename, copy to a folder and move to a folder, with sidecars and name conflicts.

use std::io;
use std::path::{Path, PathBuf};

use crate::atomic::{self, Stamp};
use crate::history::{Kind, Op, Step, Trash};
use crate::names;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Copy,
    Move,
}

/// What to do when the destination name is taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conflict {
    /// Leave both alone; this file is not copied or moved.
    Skip,
    /// Give the new one the next free name: `photo (2).jpg`.
    KeepBoth,
    /// Send the file in the way to the trash, then put this one there. The app asks first.
    Replace,
}

/// Move `from` to the new name `to` (which must be free): a no-clobber rename within one file
/// system, else (or when `force_copy`, which tests use to stand in for a second disk) copy,
/// flush, compare, and only then delete the original.
pub fn move_file(
    from: &Path,
    to: &Path,
    force_copy: bool,
    step: &mut dyn FnMut(u64) -> bool,
) -> io::Result<()> {
    if !force_copy {
        let dir = to.parent().unwrap_or(Path::new("."));
        if atomic::same_device(from, dir) {
            match atomic::place(from, to) {
                Ok(()) => return Ok(()),
                // EXDEV can still happen (bind mounts, overlay file systems): fall through.
                Err(e) if e.kind() != io::ErrorKind::CrossesDevices => return Err(e),
                Err(_) => {}
            }
        }
    }
    atomic::copy_verified(from, to, step)?;
    if let Err(e) = std::fs::remove_file(from) {
        // Leave things as they were: the original stays, the copy goes.
        let _ = std::fs::remove_file(to);
        return Err(e);
    }
    if let Some(d) = from.parent() {
        atomic::sync_dir(d);
    }
    Ok(())
}

/// Rename `file` to `new_name` in its folder, with its sidecars (and the other half of a
/// RAW+JPEG pair) following. The caller has validated the name ([`names::validate`]); a
/// sidecar whose new name is taken stops the whole rename before anything moves.
pub fn rename(file: &Path, new_name: &str) -> Result<Op, (PathBuf, io::Error)> {
    let dir = file.parent().unwrap_or(Path::new("."));
    let target = dir.join(new_name);
    let mut moves = vec![(file.to_path_buf(), target.clone())];
    for sc in names::sidecars(file, true) {
        if let Some(n) = names::sidecar_new_name(&sc, file, &target) {
            moves.push((sc.clone(), dir.join(n)));
        }
    }
    rename_all(&moves, Kind::Rename)
}

/// Rename several files at once. Checks every target first; names that swap or rotate
/// (a→b, b→a) go through temporary names so no step overwrites another.
pub fn rename_all(moves: &[(PathBuf, PathBuf)], kind: Kind) -> Result<Op, (PathBuf, io::Error)> {
    let sources: Vec<&Path> = moves.iter().map(|(f, _)| f.as_path()).collect();
    for (from, to) in moves {
        if from == to {
            continue;
        }
        let in_set = sources.contains(&to.as_path());
        if atomic::taken(to) && !in_set && !atomic::same_file(from, to) {
            return Err((
                to.clone(),
                io::Error::new(io::ErrorKind::AlreadyExists, "the name is taken"),
            ));
        }
    }
    let mut op = Op::new(kind);
    // Phase 1: anything whose target is another source's current name goes to a temporary.
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    let result = (|| {
        for (from, to) in moves {
            if from == to {
                continue;
            }
            if sources.contains(&to.as_path()) && !atomic::same_file(from, to) {
                let tmp = atomic::temp_beside(to);
                atomic::place(from, &tmp).map_err(|e| (from.clone(), e))?;
                op.steps.push(Step::Moved {
                    from: from.clone(),
                    to: tmp.clone(),
                    stamp: Stamp::of(&tmp).map_err(|e| (tmp.clone(), e))?,
                });
                staged.push((tmp, to.clone()));
            } else {
                atomic::place(from, to).map_err(|e| (from.clone(), e))?;
                op.steps.push(Step::Moved {
                    from: from.clone(),
                    to: to.clone(),
                    stamp: Stamp::of(to).map_err(|e| (to.clone(), e))?,
                });
            }
        }
        for (tmp, to) in &staged {
            atomic::place(tmp, to).map_err(|e| (tmp.clone(), e))?;
            op.steps.push(Step::Moved {
                from: tmp.clone(),
                to: to.clone(),
                stamp: Stamp::of(to).map_err(|e| (to.clone(), e))?,
            });
        }
        Ok(())
    })();
    if let Err(e) = result {
        // Put back what moved, newest first.
        for s in op.steps.iter().rev() {
            if let Step::Moved { from, to, .. } = s {
                let _ = atomic::place(to, from);
            }
        }
        return Err(e);
    }
    Ok(op)
}

/// One file to copy or move, with its sidecars.
#[derive(Clone, Debug)]
pub struct Item {
    pub src: PathBuf,
    pub sidecars: Vec<PathBuf>,
    /// The name it gets in the destination folder.
    pub dst: PathBuf,
    /// Whether that name is taken.
    pub conflict: bool,
}

/// What copying or moving `files` into `dir` would do. Sidecars of each file are found and
/// taken along; a sidecar that is itself in `files` is not taken twice.
pub fn plan(files: &[PathBuf], dir: &Path) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    let mut claimed: Vec<PathBuf> = files.to_vec();
    for f in files {
        let Some(name) = f.file_name() else { continue };
        let dst = dir.join(name);
        let sidecars: Vec<PathBuf> = names::sidecars(f, false)
            .into_iter()
            .filter(|s| !claimed.contains(s))
            .collect();
        claimed.extend(sidecars.iter().cloned());
        items.push(Item {
            conflict: atomic::taken(&dst),
            src: f.clone(),
            sidecars,
            dst,
        });
    }
    items
}

/// How many items have a name conflict (the app asks before "replace" when this is not 0).
pub fn conflicts(items: &[Item]) -> usize {
    items.iter().filter(|i| i.conflict).count()
}

/// Progress through a transfer: bytes so far and in all, and the file at hand.
#[derive(Clone, Debug)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub file: PathBuf,
}

/// How a transfer ended. `op` holds what was done (also when cancelled or partly failed),
/// for undo.
#[derive(Debug)]
pub struct Outcome {
    pub op: Op,
    pub done: usize,
    pub skipped: usize,
    pub failed: Vec<(PathBuf, String)>,
    pub cancelled: bool,
}

/// Run a copy or move. `force_copy` makes a move copy-verify-delete even within one file
/// system (tests). `progress` returns false to cancel; what is done stays done.
pub fn run(
    items: &[Item],
    mode: Mode,
    conflict: Conflict,
    force_copy: bool,
    trash: &dyn Trash,
    progress: &mut dyn FnMut(&Progress) -> bool,
) -> Outcome {
    let total: u64 = items
        .iter()
        .flat_map(|i| std::iter::once(&i.src).chain(&i.sidecars))
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    let mut out = Outcome {
        op: Op::new(match mode {
            Mode::Copy => Kind::Copy,
            Mode::Move => Kind::Move,
        }),
        done: 0,
        skipped: 0,
        failed: Vec::new(),
        cancelled: false,
    };
    let mut base = 0u64;
    for item in items {
        if out.cancelled {
            break;
        }
        let dir = item.dst.parent().unwrap_or(Path::new("."));
        let mut dst = item.dst.clone();
        let mut replace = false;
        if atomic::taken(&dst) {
            if atomic::same_file(&item.src, &dst) {
                // The file is already there (moved or copied into its own folder).
                match mode {
                    Mode::Move => {
                        out.skipped += 1;
                        continue;
                    }
                    Mode::Copy => {
                        if conflict == Conflict::Skip {
                            out.skipped += 1;
                            continue;
                        }
                        let n = dst
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        dst = names::keep_both_name(dir, &n, &|p| atomic::taken(p));
                    }
                }
            } else {
                match conflict {
                    Conflict::Skip => {
                        out.skipped += 1;
                        continue;
                    }
                    Conflict::KeepBoth => {
                        let n = dst
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        dst = names::keep_both_name(dir, &n, &|p| atomic::taken(p));
                    }
                    Conflict::Replace => replace = true,
                }
            }
        }
        // Sidecars take the main file's new stem.
        let mut pairs = vec![(item.src.clone(), dst.clone())];
        for sc in &item.sidecars {
            let Some(n) = names::sidecar_new_name(sc, &item.src, &dst) else {
                continue;
            };
            pairs.push((sc.clone(), dir.join(n)));
        }
        for (k, (from, to)) in pairs.iter().enumerate() {
            let is_main = k == 0;
            if atomic::taken(to) {
                if replace && !atomic::same_file(from, to) {
                    if let Err(e) = trash.trash(to) {
                        out.failed.push((from.clone(), e.to_string()));
                        if is_main {
                            break;
                        }
                        continue;
                    }
                    out.op.steps.push(Step::Trashed { path: to.clone() });
                } else {
                    // A sidecar in the way of a kept-both or skipped name: leave both.
                    if is_main {
                        out.skipped += 1;
                        break;
                    }
                    continue;
                }
            }
            let file = from.clone();
            let mut tick = |n: u64| {
                progress(&Progress {
                    done: base + n,
                    total,
                    file: file.clone(),
                })
            };
            let r = match mode {
                Mode::Copy => atomic::copy_verified(from, to, &mut tick),
                Mode::Move => move_file(from, to, force_copy, &mut tick),
            };
            base += std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
            match r {
                Ok(()) => {
                    let stamp = Stamp::of(to).unwrap_or(Stamp {
                        len: 0,
                        modified: None,
                    });
                    out.op.steps.push(match mode {
                        Mode::Copy => Step::Copied {
                            from: from.clone(),
                            to: to.clone(),
                            stamp,
                        },
                        Mode::Move => Step::Moved {
                            from: from.clone(),
                            to: to.clone(),
                            stamp,
                        },
                    });
                    if is_main {
                        out.done += 1;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                    out.cancelled = true;
                    break;
                }
                Err(e) => {
                    out.failed.push((from.clone(), e.to_string()));
                    if is_main {
                        break;
                    }
                }
            }
        }
        if !out.cancelled
            && !progress(&Progress {
                done: base,
                total,
                file: item.src.clone(),
            })
        {
            out.cancelled = true;
        }
    }
    out
}
