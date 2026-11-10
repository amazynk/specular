//! The shell features every desktop has in some form, as one interface per feature with an
//! implementation per platform. Linux has them all ([`super::linux`]); Windows
//! ([`super::windows`]) and macOS ([`super::macos`]) have the seams only, each returning
//! [`Unavailable::Platform`] with what is still to do written next to it.
//!
//! | Feature | Linux | Windows | macOS |
//! |---|---|---|---|
//! | [`LauncherMenu`]: actions and recent files on the app's icon | `.desktop` actions, `recently-used.xbel` | jump list (`ICustomDestinationList`) | Dock menu (`applicationDockMenu:`), `NSDocumentController` recents |
//! | [`PreviewProvider`]: thumbnails and previews in the file manager | `.thumbnailer` (`specular --thumbnail`) | `IThumbnailProvider` / `IPreviewHandler` | Quick Look thumbnail and preview extensions |
//! | [`PropertyIndexer`]: dimensions, camera and date for desktop search | (the search provider instead) | property handler (`IPropertyStore`) | Spotlight importer (`.mdimporter`) |
//! | [`SystemCodec`]: other programs open the formats through the OS | — | WIC codec (`IWICBitmapDecoder`) | — (Image I/O has no plug-ins) |
//!
//! None of them is an [`super::OptionId`] yet on Windows and macOS: [`super::options`] stays
//! empty there until a seam is filled in.

use std::path::Path;

use super::{Env, Error, Labels, Result, Unavailable};

/// An entry of the launcher menu (the `.desktop` file's actions, the Windows jump list's
/// tasks, the macOS Dock menu).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopAction {
    /// A new, separate window.
    NewWindow,
    /// The Library (the catalogue of watched locations).
    Library,
    /// A slideshow of the Pictures folder.
    Slideshow,
}

/// The launcher-menu entries, in menu order.
pub const DESKTOP_ACTIONS: [DesktopAction; 3] = [
    DesktopAction::NewWindow,
    DesktopAction::Library,
    DesktopAction::Slideshow,
];

impl DesktopAction {
    /// The action id (`[Desktop Action <id>]`, the name `ActivateAction` receives, the jump
    /// list task's id).
    pub fn id(self) -> &'static str {
        match self {
            DesktopAction::NewWindow => "new-window",
            DesktopAction::Library => "library",
            DesktopAction::Slideshow => "slideshow",
        }
    }

    /// The command-line flag that does the same.
    pub fn flag(self) -> &'static str {
        match self {
            DesktopAction::NewWindow => "--new-window",
            DesktopAction::Library => "--library",
            DesktopAction::Slideshow => "--slideshow",
        }
    }

    pub fn from_id(id: &str) -> Option<DesktopAction> {
        DESKTOP_ACTIONS.into_iter().find(|a| a.id() == id)
    }

    /// The menu text.
    pub fn label(self, labels: &Labels) -> &str {
        match self {
            DesktopAction::NewWindow => &labels.new_window,
            DesktopAction::Library => &labels.library,
            DesktopAction::Slideshow => &labels.slideshow,
        }
    }

    /// Used where labels are read back (Linux desktop entries).
    #[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
    pub(crate) fn set_label(self, labels: &mut Labels, v: &str) {
        let slot = match self {
            DesktopAction::NewWindow => &mut labels.new_window,
            DesktopAction::Library => &mut labels.library,
            DesktopAction::Slideshow => &mut labels.slideshow,
        };
        *slot = v.to_owned();
    }
}

/// The app icon's menu: launcher actions and recently viewed files.
pub trait LauncherMenu {
    /// Puts [`DESKTOP_ACTIONS`] in the menu, in the current language.
    fn set_actions(&self, env: &Env, labels: &Labels) -> Result<()>;
    /// Adds a viewed file to the menu's recent list.
    fn add_recent(&self, env: &Env, file: &Path) -> Result<()>;
    /// Takes the actions and Specular's recent entries out again.
    fn clear(&self, env: &Env) -> Result<()>;
}

/// File-manager thumbnails and previews of the formats the system cannot read.
pub trait PreviewProvider {
    /// Registers the thumbnail (and, where there is one, preview) handler for `types`
    /// (MIME types, [`super::mime::IMAGE_TYPES`]).
    fn register(&self, env: &Env, types: &[&str]) -> Result<()>;
    fn unregister(&self, env: &Env) -> Result<()>;
}

/// Image facts (dimensions, camera, date taken) handed to the desktop search's indexer.
pub trait PropertyIndexer {
    fn register(&self, env: &Env, types: &[&str]) -> Result<()>;
    fn unregister(&self, env: &Env) -> Result<()>;
}

/// A system-wide decoder, so other programs open the formats through the OS image API.
pub trait SystemCodec {
    fn register(&self, env: &Env, types: &[&str]) -> Result<()>;
    fn unregister(&self, env: &Env) -> Result<()>;
}

/// The error of a seam that is not filled in yet.
pub(crate) fn unsupported<T>() -> Result<T> {
    Err(Error::Unavailable(Unavailable::Platform))
}

/// This platform's launcher menu, `None` where it is not done yet (Linux keeps its actions in
/// the `.desktop` file; Linux and Windows keep their recent files under the `RecentFiles`
/// option instead).
pub fn launcher_menu() -> Option<&'static dyn LauncherMenu> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_round_trip() {
        let mut l = Labels::default();
        for a in DESKTOP_ACTIONS {
            assert_eq!(DesktopAction::from_id(a.id()), Some(a));
            assert!(a.flag().starts_with("--"));
            a.set_label(&mut l, a.id());
            assert_eq!(a.label(&l), a.id());
        }
        assert_eq!(DesktopAction::from_id("nope"), None);
    }

    /// The Windows and macOS seams type-check on every platform and refuse cleanly.
    #[test]
    fn other_platforms_are_unsupported() {
        let env = Env::under(
            Path::new("/nonexistent"),
            Path::new("/nonexistent/specular"),
        );
        let labels = Labels::default();
        let unsupported = |r: Result<()>| {
            assert!(matches!(r, Err(Error::Unavailable(Unavailable::Platform))));
        };
        let menus: [&dyn LauncherMenu; 2] = [
            &super::super::windows::JumpList,
            &super::super::macos::DockMenu,
        ];
        for m in menus {
            unsupported(m.set_actions(&env, &labels));
            unsupported(m.add_recent(&env, Path::new("/a.jpg")));
            unsupported(m.clear(&env));
        }
        let previews: [&dyn PreviewProvider; 2] = [
            &super::super::windows::ShellPreview,
            &super::super::macos::QuickLook,
        ];
        let indexers: [&dyn PropertyIndexer; 2] = [
            &super::super::windows::PropertyHandler,
            &super::super::macos::SpotlightImporter,
        ];
        for p in previews {
            unsupported(p.register(&env, &["image/jxl"]));
            unsupported(p.unregister(&env));
        }
        for p in indexers {
            unsupported(p.register(&env, &["image/jxl"]));
            unsupported(p.unregister(&env));
        }
        let wic: &dyn SystemCodec = &super::super::windows::WicCodec;
        unsupported(wic.register(&env, &["image/jxl"]));
        unsupported(wic.unregister(&env));
        assert!(launcher_menu().is_none());
    }
}
