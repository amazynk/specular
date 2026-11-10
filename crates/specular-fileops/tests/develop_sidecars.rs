//! Develop sidecars (`IMG.ext.specular.xmp`, `IMG.ext.vN.specular.xmp`) follow their photo
//! through rename, batch rename, copy and move, and undo brings them back. Real
//! folders under the build's temporary directory; a folder stands in for the trash.

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

use specular_fileops::names;
use specular_fileops::transfer::{self, Conflict, Mode};
use specular_fileops::{Platform, Trash, batch, history};

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("fileops-develop")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn put(p: &Path, s: &str) {
    std::fs::write(p, s).unwrap();
}

fn text(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

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

/// A RAW with its JPEG, each with a Develop edit, the RAW with two more versions; a
/// lookalike that belongs to neither.
const SET: [&str; 8] = [
    "IMG_7.CR3",
    "IMG_7.CR3.specular.xmp",
    "IMG_7.CR3.v2.specular.xmp",
    "IMG_7.CR3.v10.specular.xmp",
    "IMG_7.JPG",
    "IMG_7.JPG.specular.xmp",
    "IMG_7.CR3.vX.specular.xmp",
    "IMG_70.CR3.specular.xmp",
];

fn fill(d: &Path) {
    for n in SET {
        put(&d.join(n), n);
    }
}

#[test]
fn develop_sidecars_are_recognised() {
    assert!(names::is_develop_sidecar_of(
        "IMG_7.CR3.specular.xmp",
        "IMG_7.CR3"
    ));
    assert!(names::is_develop_sidecar_of(
        "IMG_7.CR3.v12.specular.xmp",
        "IMG_7.CR3"
    ));
    for no in [
        "IMG_7.CR3.vX.specular.xmp",
        "IMG_7.CR3.v.specular.xmp",
        "IMG_70.CR3.specular.xmp",
        "IMG_7.CR3.xmp",
        "IMG_7.specular.xmp",
    ] {
        assert!(!names::is_develop_sidecar_of(no, "IMG_7.CR3"), "{no}");
    }
    let d = scratch("listed");
    fill(&d);
    let got: Vec<String> = names::sidecars(&d.join("IMG_7.CR3"), false)
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        got,
        [
            "IMG_7.CR3.specular.xmp",
            "IMG_7.CR3.v10.specular.xmp",
            "IMG_7.CR3.v2.specular.xmp"
        ]
    );
    let all = names::sidecars(&d.join("IMG_7.CR3"), true);
    assert_eq!(
        names::develop_sidecars(&all, &d.join("IMG_7.JPG")),
        [d.join("IMG_7.JPG.specular.xmp")]
    );
}

#[test]
fn rename_carries_every_version_and_undo_brings_them_back() {
    let d = scratch("rename");
    fill(&d);
    let op = transfer::rename(&d.join("IMG_7.CR3"), "Harbour.CR3").unwrap();
    for n in [
        "Harbour.CR3",
        "Harbour.CR3.specular.xmp",
        "Harbour.CR3.v2.specular.xmp",
        "Harbour.CR3.v10.specular.xmp",
        "Harbour.JPG",
        "Harbour.JPG.specular.xmp",
    ] {
        assert!(d.join(n).exists(), "{n} after rename");
    }
    assert_eq!(
        text(&d.join("Harbour.CR3.v2.specular.xmp")),
        "IMG_7.CR3.v2.specular.xmp"
    );
    // Not theirs: left where they were.
    assert!(d.join("IMG_7.CR3.vX.specular.xmp").exists());
    assert!(d.join("IMG_70.CR3.specular.xmp").exists());
    let trash = FolderTrash::new(d.join(".trash"));
    let back = history::undo(&op, &trash).unwrap();
    for n in SET {
        assert_eq!(text(&d.join(n)), n, "{n} after undo");
    }
    let again = history::redo(&back, &trash).unwrap();
    assert!(d.join("Harbour.CR3.v10.specular.xmp").exists());
    history::undo(&again, &trash).unwrap();
    assert!(d.join("IMG_7.CR3.v10.specular.xmp").exists());
}

#[test]
fn rename_stops_when_a_version_name_is_taken() {
    let d = scratch("taken");
    fill(&d);
    put(&d.join("New.CR3.v2.specular.xmp"), "someone else's");
    assert!(transfer::rename(&d.join("IMG_7.CR3"), "New.CR3").is_err());
    for n in SET {
        assert_eq!(text(&d.join(n)), n, "{n} untouched");
    }
}

#[test]
fn batch_rename_carries_them() {
    let d = scratch("batch");
    fill(&d);
    let rows = batch::preview(
        &[(d.join("IMG_7.CR3"), "Day1_001.CR3".into())],
        Platform::Unix,
    );
    let op = batch::apply(&rows).unwrap();
    assert!(d.join("Day1_001.CR3.v10.specular.xmp").exists());
    assert!(d.join("Day1_001.JPG.specular.xmp").exists());
    let trash = FolderTrash::new(d.join(".trash"));
    history::undo(&op, &trash).unwrap();
    for n in SET {
        assert_eq!(text(&d.join(n)), n, "{n} after undo");
    }
}

fn transfer_case(mode: Mode, keep_both: bool) {
    let d = scratch(&format!("{mode:?}-{keep_both}"));
    let (src, dst) = (d.join("src"), d.join("dst"));
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dst).unwrap();
    fill(&src);
    if keep_both {
        put(&dst.join("IMG_7.CR3"), "another photo");
    }
    let items = transfer::plan(&[src.join("IMG_7.CR3")], &dst);
    assert_eq!(items[0].sidecars.len(), 3);
    let trash = FolderTrash::new(d.join(".trash"));
    let out = transfer::run(&items, mode, Conflict::KeepBoth, false, &trash, &mut |_| {
        true
    });
    assert_eq!((out.done, out.failed.len()), (1, 0));
    let stem = if keep_both {
        "IMG_7 (2).CR3"
    } else {
        "IMG_7.CR3"
    };
    for v in ["", ".v2", ".v10"] {
        let n = format!("{stem}{v}.specular.xmp");
        assert_eq!(
            text(&dst.join(&n)),
            format!("IMG_7.CR3{v}.specular.xmp"),
            "{n}"
        );
        let left = src.join(format!("IMG_7.CR3{v}.specular.xmp")).exists();
        assert_eq!(left, mode == Mode::Copy, "source {v} after {mode:?}");
    }
    history::undo(&out.op, &trash).unwrap();
    for n in SET {
        assert_eq!(text(&src.join(n)), n, "{n} after undo");
    }
    let left: Vec<_> = std::fs::read_dir(&dst)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let expect: Vec<String> = if keep_both {
        vec!["IMG_7.CR3".into()]
    } else {
        Vec::new()
    };
    assert_eq!(left, expect);
}

#[test]
fn move_carries_them_and_undo_brings_them_back() {
    transfer_case(Mode::Move, false);
}

#[test]
fn copy_carries_them_and_undo_removes_the_copies() {
    transfer_case(Mode::Copy, false);
}

#[test]
fn keep_both_renames_them_with_the_photo() {
    transfer_case(Mode::Copy, true);
    transfer_case(Mode::Move, true);
}
