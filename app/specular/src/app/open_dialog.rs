//! The Open dialog's file types: "All supported files" first (selected when it opens), then
//! one entry per kind of file, then "All files". Everything comes from the decoder registry
//! ([`Registry::file_groups`]), so a new format shows up without touching this list.
//!
//! Linux asks the FileChooser portal itself so it can name the selected entry
//! (`current_filter`): KDE's dialog otherwise did not start on the first one. Without a
//! portal, and on Windows and macOS, the `rfd` dialog gets the same list in the same order.

use std::path::{Path, PathBuf};

use specular_decode::Registry;
use specular_decode::mime::FileGroup;
use specular_i18n::t;
use specular_platform::file_chooser::{self, FileFilter};

/// One entry of the list: its name and extensions (no dot, lower case); none for "All files".
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Entry {
    pub name: String,
    pub exts: Vec<&'static str>,
}

/// The dialog's name for a group.
fn group_name(g: FileGroup) -> String {
    match g {
        FileGroup::Images => t!("app-open-group-images"),
        FileGroup::CameraRaw => t!("app-open-group-raw"),
        FileGroup::Video => t!("app-open-group-video"),
        FileGroup::Sound => t!("app-open-group-sound"),
        FileGroup::Documents => t!("app-open-group-documents"),
        FileGroup::Drawings => t!("app-open-group-drawings"),
        FileGroup::Models => t!("app-open-group-models"),
        FileGroup::Maps => t!("app-open-group-maps"),
        FileGroup::Science => t!("app-open-group-science"),
        FileGroup::Fonts => t!("app-open-group-fonts"),
        FileGroup::Other => t!("app-open-group-other"),
    }
}

/// The whole list, in order: all supported, the groups, all files.
pub(super) fn entries(reg: &Registry) -> Vec<Entry> {
    let mut out = vec![Entry {
        name: t!("app-open-dialog-filter"),
        exts: reg.extensions(),
    }];
    out.extend(reg.file_groups().into_iter().map(|(g, exts)| Entry {
        name: group_name(g),
        exts,
    }));
    out.push(Entry {
        name: t!("app-open-dialog-all"),
        exts: Vec::new(),
    });
    out
}

/// The entries as the portal takes them: glob patterns in both cases.
fn portal_filters(list: &[Entry]) -> Vec<FileFilter> {
    list.iter()
        .map(|e| {
            if e.exts.is_empty() {
                FileFilter::any(&e.name)
            } else {
                FileFilter::from_extensions(&e.name, e.exts.iter().copied())
            }
        })
        .collect()
}

/// The entries for `rfd`: extensions without the dot ("*" for all files). Linux dialogs
/// match case-sensitively, so there both cases go in; macOS has no type menu (it allows the
/// union of every list), so "All files" stays out there or it would let every file through.
fn rfd_filters(list: &[Entry]) -> Vec<(String, Vec<String>)> {
    let both_cases = cfg!(all(unix, not(target_os = "macos")));
    list.iter()
        .filter(|e| !(cfg!(target_os = "macos") && e.exts.is_empty()))
        .map(|e| {
            let mut exts = Vec::new();
            if e.exts.is_empty() {
                exts.push("*".to_owned());
            }
            for x in &e.exts {
                exts.push((*x).to_owned());
                let up = x.to_ascii_uppercase();
                if both_cases && up != *x {
                    exts.push(up);
                }
            }
            (e.name.clone(), exts)
        })
        .collect()
}

/// Shows the Open dialog (blocks; call on a background thread) and returns the file picked.
pub(super) fn pick(title: &str, list: &[Entry], dir: Option<&Path>) -> Option<PathBuf> {
    if file_chooser::portal_available() {
        let filters = portal_filters(list);
        let req = file_chooser::OpenFile {
            title,
            filters: &filters,
            current: 0,
            folder: dir,
        };
        match file_chooser::open_file(&req) {
            Ok(p) => return p,
            Err(e) => {
                specular_log::debug!(target: specular_log::UI, "open dialog: portal: {e}");
            }
        }
    }
    let mut d = rfd::FileDialog::new().set_title(title);
    for (name, exts) in rfd_filters(list) {
        d = d.add_filter(name, &exts);
    }
    if let Some(dir) = dir {
        d = d.set_directory(dir);
    }
    d.pick_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "All supported files" comes first and covers every extension a decoder opens, the
    /// groups follow, "All files" ends the list.
    #[test]
    fn first_entry_covers_every_registered_extension() {
        let reg = Registry::new();
        let list = entries(&reg);
        let all = &list[0];
        let mut exts = reg.extensions();
        assert!(exts.len() > 300, "{}", exts.len());
        for e in &exts {
            assert!(all.exts.contains(e), ".{e} missing from {}", all.name);
        }
        assert_eq!(all.exts.len(), exts.len());
        // Every MIME-registered extension too (the desktop's own list).
        for (ext, _) in specular_decode::mime::EXTENSION_MIME {
            assert!(all.exts.contains(ext), ".{ext}");
        }
        // The groups between them hold the same extensions, each once.
        let groups = &list[1..list.len() - 1];
        assert!(groups.len() >= 8);
        let mut grouped: Vec<&str> = groups.iter().flat_map(|g| g.exts.clone()).collect();
        grouped.sort_unstable();
        exts.sort_unstable();
        assert_eq!(grouped, exts);
        let last = list.last().unwrap();
        assert!(last.exts.is_empty());
        // The names are all different and none is a message id left untranslated.
        let mut names: Vec<&str> = list.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names
                .iter()
                .all(|n| !n.is_empty() && !n.starts_with("app-open"))
        );
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), list.len());
    }

    /// The portal gets globs in both cases, the first filter selected; the rfd list keeps
    /// the same order.
    #[test]
    fn portal_and_rfd_lists_keep_the_order() {
        let list = entries(&Registry::new());
        let pf = portal_filters(&list);
        assert_eq!(pf.len(), list.len());
        assert!(pf[0].patterns.contains(&"*.jpg".to_owned()));
        assert!(pf[0].patterns.contains(&"*.JPG".to_owned()));
        assert!(pf[0].patterns.contains(&"*.cr3".to_owned()));
        assert_eq!(pf.last().unwrap().patterns, ["*"]);
        // The portal message stays small next to D-Bus's 128 MiB message limit.
        let bytes: usize = pf
            .iter()
            .flat_map(|f| f.patterns.iter())
            .map(|p| p.len() + 8)
            .sum();
        assert!(bytes < 256 << 10, "{bytes}");
        let rf = rfd_filters(&list);
        assert_eq!(rf[0].0, list[0].name);
        assert!(rf[0].1.iter().any(|e| e == "jpg"));
        for (r, e) in rf.iter().zip(&list) {
            assert_eq!(r.0, e.name);
        }
    }
}
