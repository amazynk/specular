//! Rename, copy, move, batch rename, undo and redo against real folders (under the build's
//! temporary directory), with a folder standing in for the system trash.

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

use specular_fileops::batch;
use specular_fileops::history::{self, Kind};
use specular_fileops::transfer::{self, Conflict, Mode};
use specular_fileops::{History, Platform, Trash, UndoError};

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("fileops-ops")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn put(p: &Path, s: &str) {
    std::fs::write(p, s).unwrap();
}

fn text(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap()
}

/// A trash folder: each trashed file gets a numbered name; restore takes the newest that came
/// from that path.
struct FolderTrash {
    dir: PathBuf,
    items: RefCell<Vec<(PathBuf, PathBuf)>>,
}

impl FolderTrash {
    fn new(dir: PathBuf) -> FolderTrash {
        std::fs::create_dir_all(&dir).unwrap();
        FolderTrash {
            dir,
            items: RefCell::new(Vec::new()),
        }
    }
}

impl Trash for FolderTrash {
    fn trash(&self, path: &Path) -> io::Result<()> {
        let mut items = self.items.borrow_mut();
        let to = self.dir.join(format!("{}", items.len()));
        std::fs::rename(path, &to)?;
        items.push((path.to_path_buf(), to));
        Ok(())
    }

    fn restore(&self, path: &Path) -> io::Result<()> {
        let mut items = self.items.borrow_mut();
        let i = items
            .iter()
            .rposition(|(o, _)| o == path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not in the trash"))?;
        let (orig, at) = items.remove(i);
        std::fs::rename(at, orig)
    }
}

#[test]
fn rename_takes_sidecars_and_pair_and_undoes() {
    let d = scratch("rename");
    for n in [
        "IMG_1.CR3",
        "IMG_1.JPG",
        "IMG_1.xmp",
        "IMG_1.JPG.xmp",
        "IMG_1.AAE",
    ] {
        put(&d.join(n), n);
    }
    let file = d.join("IMG_1.JPG");
    let target = specular_fileops::names::validate(&file, "Beach.JPG", Platform::Unix).unwrap();
    assert_eq!(target, d.join("Beach.JPG"));
    let op = transfer::rename(&file, "Beach.JPG").unwrap();
    for n in [
        "Beach.CR3",
        "Beach.JPG",
        "Beach.xmp",
        "Beach.JPG.xmp",
        "Beach.AAE",
    ] {
        assert!(d.join(n).exists(), "{n} after rename");
    }
    assert_eq!(text(&d.join("Beach.xmp")), "IMG_1.xmp");
    let trash = FolderTrash::new(d.join(".trash"));
    let back = history::undo(&op, &trash).unwrap();
    for n in [
        "IMG_1.CR3",
        "IMG_1.JPG",
        "IMG_1.xmp",
        "IMG_1.JPG.xmp",
        "IMG_1.AAE",
    ] {
        assert_eq!(text(&d.join(n)), n, "{n} after undo");
    }
    let again = history::redo(&back, &trash).unwrap();
    assert!(d.join("Beach.JPG").exists());
    history::undo(&again, &trash).unwrap();
    assert!(d.join("IMG_1.JPG").exists());
}

#[test]
fn rename_validation_reasons() {
    let d = scratch("validate");
    put(&d.join("a.jpg"), "a");
    put(&d.join("b.jpg"), "b");
    let a = d.join("a.jpg");
    use specular_fileops::NameProblem as P;
    let v = |n: &str, p| specular_fileops::names::validate(&a, n, p);
    assert_eq!(v("b.jpg", Platform::Unix), Err(P::Exists));
    assert_eq!(v("x/y.jpg", Platform::Unix), Err(P::InvalidChar('/')));
    assert_eq!(v("x:y.jpg", Platform::Windows), Err(P::InvalidChar(':')));
    assert_eq!(
        v("CON.jpg", Platform::Windows),
        Err(P::Reserved("CON".into()))
    );
    assert_eq!(v("", Platform::Unix), Err(P::Empty));
    assert_eq!(v("..", Platform::Unix), Err(P::DotName));
    assert!(v("c.jpg", Platform::Unix).is_ok());
    // Only the case changes: allowed (the same file).
    assert!(v("A.jpg", Platform::Unix).is_ok());
}

#[test]
fn undo_refuses_when_changed() {
    let d = scratch("changed");
    put(&d.join("a.jpg"), "a");
    let op = transfer::rename(&d.join("a.jpg"), "b.jpg").unwrap();
    // Edited elsewhere after the rename: a different size.
    put(&d.join("b.jpg"), "edited since");
    let trash = FolderTrash::new(d.join(".trash"));
    let e = history::undo(&op, &trash).unwrap_err();
    assert_eq!(e, UndoError::Changed(d.join("b.jpg")));
    assert_eq!(e.message_id(), "undo-problem-changed");
    // Nothing moved.
    assert!(d.join("b.jpg").exists() && !d.join("a.jpg").exists());
    // Gone entirely.
    std::fs::remove_file(d.join("b.jpg")).unwrap();
    assert_eq!(
        history::undo(&op, &trash).unwrap_err(),
        UndoError::Missing(d.join("b.jpg"))
    );
}

#[test]
fn undo_refuses_when_old_name_taken() {
    let d = scratch("taken");
    put(&d.join("a.jpg"), "a");
    let op = transfer::rename(&d.join("a.jpg"), "b.jpg").unwrap();
    put(&d.join("a.jpg"), "a new file");
    let trash = FolderTrash::new(d.join(".trash"));
    assert_eq!(
        history::undo(&op, &trash).unwrap_err(),
        UndoError::Taken(d.join("a.jpg"))
    );
}

fn copy_move(force_copy: bool, mode: Mode) {
    let name = format!("cm-{force_copy}-{mode:?}");
    let d = scratch(&name);
    let src = d.join("src");
    let dst = d.join("dst");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dst).unwrap();
    put(&src.join("a.jpg"), "aaaa");
    put(&src.join("a.xmp"), "<xmp a>");
    put(&src.join("b.jpg"), "bbbb");
    put(&dst.join("b.jpg"), "old b");
    let files = vec![src.join("a.jpg"), src.join("b.jpg")];
    let items = transfer::plan(&files, &dst);
    assert_eq!(transfer::conflicts(&items), 1);
    let trash = FolderTrash::new(d.join(".trash"));
    let mut ticks = 0;
    let out = transfer::run(
        &items,
        mode,
        Conflict::KeepBoth,
        force_copy,
        &trash,
        &mut |_| {
            ticks += 1;
            true
        },
    );
    assert_eq!((out.done, out.skipped, out.failed.len()), (2, 0, 0));
    assert!(ticks > 0);
    assert_eq!(text(&dst.join("a.jpg")), "aaaa");
    assert_eq!(text(&dst.join("a.xmp")), "<xmp a>");
    assert_eq!(text(&dst.join("b (2).jpg")), "bbbb");
    assert_eq!(text(&dst.join("b.jpg")), "old b");
    let moved = mode == Mode::Move;
    assert_eq!(src.join("a.jpg").exists(), !moved);
    assert_eq!(src.join("a.xmp").exists(), !moved);
    // Undo: a copy goes to the trash, a move comes back.
    let back = history::undo(&out.op, &trash).unwrap();
    assert!(!dst.join("a.jpg").exists() && !dst.join("b (2).jpg").exists());
    assert_eq!(text(&src.join("a.jpg")), "aaaa");
    assert_eq!(text(&src.join("a.xmp")), "<xmp a>");
    assert_eq!(text(&dst.join("b.jpg")), "old b");
    // Redo puts it all back again.
    let again = history::redo(&back, &trash).unwrap();
    assert_eq!(text(&dst.join("b (2).jpg")), "bbbb");
    assert_eq!(src.join("b.jpg").exists(), !moved);
    history::undo(&again, &trash).unwrap();
}

#[test]
fn copy_same_disk() {
    copy_move(false, Mode::Copy);
}

#[test]
fn move_same_disk() {
    copy_move(false, Mode::Move);
}

#[test]
fn move_across_disks() {
    // Copy, verify, delete, as between two file systems.
    copy_move(true, Mode::Move);
}

#[test]
fn conflicts_skip_and_replace() {
    let d = scratch("conflicts");
    let src = d.join("src");
    let dst = d.join("dst");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dst).unwrap();
    put(&src.join("a.jpg"), "new a");
    put(&dst.join("a.jpg"), "old a");
    let items = transfer::plan(&[src.join("a.jpg")], &dst);
    let trash = FolderTrash::new(d.join(".trash"));
    let out = transfer::run(
        &items,
        Mode::Copy,
        Conflict::Skip,
        false,
        &trash,
        &mut |_| true,
    );
    assert_eq!((out.done, out.skipped), (0, 1));
    assert_eq!(text(&dst.join("a.jpg")), "old a");
    let out = transfer::run(
        &items,
        Mode::Move,
        Conflict::Replace,
        true,
        &trash,
        &mut |_| true,
    );
    assert_eq!((out.done, out.skipped), (1, 0));
    assert_eq!(text(&dst.join("a.jpg")), "new a");
    assert!(!src.join("a.jpg").exists());
    // Undo moves it back and restores the replaced file from the trash.
    history::undo(&out.op, &trash).unwrap();
    assert_eq!(text(&dst.join("a.jpg")), "old a");
    assert_eq!(text(&src.join("a.jpg")), "new a");
}

#[test]
fn move_into_own_folder_is_skipped_and_copy_keeps_both() {
    let d = scratch("own");
    put(&d.join("a.jpg"), "a");
    let items = transfer::plan(&[d.join("a.jpg")], &d);
    let trash = FolderTrash::new(d.join(".trash"));
    let out = transfer::run(
        &items,
        Mode::Move,
        Conflict::Replace,
        false,
        &trash,
        &mut |_| true,
    );
    assert_eq!((out.done, out.skipped), (0, 1));
    assert_eq!(text(&d.join("a.jpg")), "a");
    let out = transfer::run(
        &items,
        Mode::Copy,
        Conflict::Replace,
        false,
        &trash,
        &mut |_| true,
    );
    assert_eq!(out.done, 1);
    assert_eq!(text(&d.join("a (2).jpg")), "a");
}

#[test]
fn cancel_keeps_what_is_done() {
    let d = scratch("cancel");
    let dst = d.join("dst");
    std::fs::create_dir_all(&dst).unwrap();
    put(&d.join("a.jpg"), "a");
    put(&d.join("b.jpg"), "b");
    let items = transfer::plan(&[d.join("a.jpg"), d.join("b.jpg")], &dst);
    let trash = FolderTrash::new(d.join(".trash"));
    let mut calls = 0;
    let out = transfer::run(
        &items,
        Mode::Copy,
        Conflict::Skip,
        false,
        &trash,
        &mut |p| {
            calls += 1;
            !p.file.ends_with("b.jpg")
        },
    );
    assert!(out.cancelled);
    assert_eq!(out.done, 1);
    assert!(dst.join("a.jpg").exists());
    assert!(!dst.join("b.jpg").exists());
    // No temporary left behind.
    let left: Vec<_> = std::fs::read_dir(&dst).unwrap().flatten().collect();
    assert_eq!(left.len(), 1, "{left:?}");
    assert!(calls > 0);
}

#[test]
fn batch_preview_and_apply_with_swap() {
    let d = scratch("batch");
    for n in ["a.jpg", "b.jpg", "c.jpg"] {
        put(&d.join(n), n);
    }
    put(&d.join("a.xmp"), "a sidecar");
    put(&d.join("taken.jpg"), "someone else");
    let rows = batch::preview(
        &[
            (d.join("a.jpg"), "b.jpg".into()),
            (d.join("b.jpg"), "a.jpg".into()),
            (d.join("c.jpg"), "taken.jpg".into()),
        ],
        Platform::Unix,
    );
    assert!(rows[0].problem.is_none() && rows[1].problem.is_none());
    assert!(rows[2].problem.is_some());
    let op = batch::apply(&rows).unwrap();
    assert_eq!(op.kind, Kind::BatchRename);
    assert_eq!(text(&d.join("a.jpg")), "b.jpg");
    assert_eq!(text(&d.join("b.jpg")), "a.jpg");
    assert_eq!(text(&d.join("b.xmp")), "a sidecar");
    assert_eq!(text(&d.join("c.jpg")), "c.jpg");
    let trash = FolderTrash::new(d.join(".trash"));
    history::undo(&op, &trash).unwrap();
    assert_eq!(text(&d.join("a.jpg")), "a.jpg");
    assert_eq!(text(&d.join("b.jpg")), "b.jpg");
    assert_eq!(text(&d.join("a.xmp")), "a sidecar");
    // Only the starting files (plus the trash folder) remain: no temporaries.
    let n = std::fs::read_dir(&d).unwrap().count();
    assert_eq!(n, 6);
}

#[test]
fn batch_duplicates_are_flagged() {
    let d = scratch("dups");
    put(&d.join("a.jpg"), "a");
    put(&d.join("b.jpg"), "b");
    let rows = batch::preview(
        &[
            (d.join("a.jpg"), "x.jpg".into()),
            (d.join("b.jpg"), "X.jpg".into()),
        ],
        Platform::Windows,
    );
    assert!(rows.iter().all(|r| r.problem.is_some()));
    let rows = batch::preview(
        &[
            (d.join("a.jpg"), "x.jpg".into()),
            (d.join("b.jpg"), "X.jpg".into()),
        ],
        Platform::Unix,
    );
    assert!(rows.iter().all(|r| r.problem.is_none()));
}

#[test]
fn history_redo_cleared_by_new_op() {
    let d = scratch("history");
    put(&d.join("a.jpg"), "a");
    let trash = FolderTrash::new(d.join(".trash"));
    let mut h = History::default();
    let op = transfer::rename(&d.join("a.jpg"), "b.jpg").unwrap();
    h.push(1, op);
    let op = h.take(1).unwrap();
    let back = history::undo(&op, &trash).unwrap();
    h.undone(1, back);
    assert!(h.can_redo());
    let (t, op) = h.take_redo().unwrap();
    let done = history::redo(&op, &trash).unwrap();
    h.redone(t, done);
    assert!(d.join("b.jpg").exists());
    assert!(!h.can_redo());
    h.push(2, transfer::rename(&d.join("b.jpg"), "c.jpg").unwrap());
    assert_eq!(h.len(), 2);
    assert!(!h.can_redo());
}
