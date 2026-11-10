//! Cull mode's items: the files of a folder grouped by stem, so a RAW+JPEG pair (and any
//! other file sharing its stem in the same folder) is culled as one frame. Pure, so it is
//! tested on its own.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Which member of a RAW+JPEG pair is displayed (`cull.show`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Show {
    /// The JPEG (or other rendered file): decodes fastest.
    Jpeg,
    /// The RAW.
    Raw,
}

/// One cull item: the files sharing a folder and stem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Item {
    /// The member on screen (one of `members`).
    pub shown: PathBuf,
    /// Every member, in listing order.
    pub members: Vec<PathBuf>,
}

impl Item {
    /// Whether any member is a RAW.
    pub fn has_raw(&self) -> bool {
        self.members.iter().any(|p| is_raw(p))
    }
}

pub(crate) fn is_raw(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(crate::app::gallery::is_raw_ext)
}

/// The grouping key: folder and lower-case stem.
fn key(p: &Path) -> (Option<PathBuf>, OsString) {
    let stem = p.file_stem().unwrap_or_default();
    let stem = match stem.to_str() {
        Some(s) => OsString::from(s.to_lowercase()),
        None => stem.to_owned(),
    };
    (p.parent().map(Path::to_path_buf), stem)
}

/// Groups `listing` by folder and stem, in the order of each group's first member. The
/// displayed member follows `show` (the first RAW, or the first non-RAW, falling back to the
/// first member), except that a group containing `keep` shows `keep` so whatever is on
/// screen stays there.
pub(crate) fn group(listing: &[PathBuf], show: Show, keep: Option<&Path>) -> Vec<Item> {
    let mut at: HashMap<(Option<PathBuf>, OsString), usize> = HashMap::new();
    let mut groups: Vec<Vec<PathBuf>> = Vec::new();
    for p in listing {
        let i = *at.entry(key(p)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[i].push(p.clone());
    }
    groups
        .into_iter()
        .map(|members| {
            let shown = keep
                .and_then(|k| members.iter().find(|m| m.as_path() == k))
                .or_else(|| match show {
                    Show::Raw => members.iter().find(|m| is_raw(m)),
                    Show::Jpeg => members.iter().find(|m| !is_raw(m)),
                })
                .unwrap_or(&members[0])
                .clone();
            Item { shown, members }
        })
        .collect()
}

/// Whether most of `items` contain a RAW (a RAW shoot, which opens straight into cull mode
/// when `cull.auto_enter` is on). An empty folder is not.
pub(crate) fn mostly_raw(items: &[Item]) -> bool {
    let raw = items.iter().filter(|i| i.has_raw()).count();
    raw * 2 > items.len()
}

/// The index of the item holding `p`.
pub(crate) fn item_of(items: &[Item], p: &Path) -> Option<usize> {
    items.iter().position(|i| i.members.iter().any(|m| m == p))
}

/// A short name for the item: "IMG_0001 · JPG + CR3" (the shown member's extension first),
/// or the file name when it is alone.
pub(crate) fn caption(item: &Item) -> String {
    if item.members.len() < 2 {
        return item
            .shown
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
    }
    let stem = item
        .shown
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut exts: Vec<String> = Vec::new();
    let others = item.members.iter().filter(|m| **m != item.shown);
    for m in std::iter::once(&item.shown).chain(others) {
        let e = m
            .extension()
            .map(|e| e.to_string_lossy().to_uppercase())
            .unwrap_or_default();
        if !exts.contains(&e) {
            exts.push(e);
        }
    }
    specular_i18n::dot_join(&[stem, exts.join(" + ")])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(v: &[&str]) -> Vec<PathBuf> {
        v.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn pairs_group_by_folder_and_stem() {
        let l = paths(&[
            "/a/IMG_1.CR3",
            "/a/IMG_1.JPG",
            "/a/IMG_2.jpg",
            "/a/img_3.nef",
            "/a/IMG_3.JPG",
            "/b/IMG_1.JPG",
        ]);
        let g = group(&l, Show::Jpeg, None);
        assert_eq!(g.len(), 4);
        assert_eq!(g[0].members, paths(&["/a/IMG_1.CR3", "/a/IMG_1.JPG"]));
        assert_eq!(g[0].shown, PathBuf::from("/a/IMG_1.JPG"));
        assert_eq!(g[1].members, paths(&["/a/IMG_2.jpg"]));
        // Stems compare case-insensitively.
        assert_eq!(g[2].members, paths(&["/a/img_3.nef", "/a/IMG_3.JPG"]));
        // Another folder is another frame.
        assert_eq!(g[3].members, paths(&["/b/IMG_1.JPG"]));
    }

    #[test]
    fn shown_member_follows_setting_and_keep() {
        let l = paths(&["/a/x.jpg", "/a/x.arw", "/a/y.arw"]);
        let raw = group(&l, Show::Raw, None);
        assert_eq!(raw[0].shown, PathBuf::from("/a/x.arw"));
        assert_eq!(raw[1].shown, PathBuf::from("/a/y.arw"));
        let jpeg = group(&l, Show::Jpeg, None);
        assert_eq!(jpeg[0].shown, PathBuf::from("/a/x.jpg"));
        // A RAW alone is shown whatever the setting.
        assert_eq!(jpeg[1].shown, PathBuf::from("/a/y.arw"));
        let kept = group(&l, Show::Jpeg, Some(Path::new("/a/x.arw")));
        assert_eq!(kept[0].shown, PathBuf::from("/a/x.arw"));
        assert_eq!(item_of(&kept, Path::new("/a/x.jpg")), Some(0));
        assert_eq!(item_of(&kept, Path::new("/a/y.arw")), Some(1));
        assert_eq!(item_of(&kept, Path::new("/a/z.arw")), None);
    }

    #[test]
    fn mostly_raw_counts_frames_not_files() {
        // Two pairs and one JPEG: 2 of 3 frames have a RAW.
        let l = paths(&["/a/1.cr2", "/a/1.jpg", "/a/2.cr2", "/a/2.jpg", "/a/3.jpg"]);
        assert!(mostly_raw(&group(&l, Show::Jpeg, None)));
        let l = paths(&["/a/1.cr2", "/a/2.jpg", "/a/3.png"]);
        assert!(!mostly_raw(&group(&l, Show::Jpeg, None)));
        assert!(!mostly_raw(&[]));
        // Exactly half is not "mostly".
        let l = paths(&["/a/1.dng", "/a/2.jpg"]);
        assert!(!mostly_raw(&group(&l, Show::Jpeg, None)));
    }

    #[test]
    fn captions() {
        let l = paths(&["/a/IMG_1.CR3", "/a/IMG_1.JPG", "/a/b.png"]);
        let g = group(&l, Show::Jpeg, None);
        assert_eq!(caption(&g[0]), "IMG_1 · JPG + CR3");
        assert_eq!(caption(&g[1]), "b.png");
    }
}
