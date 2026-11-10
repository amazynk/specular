//! macOS desktop integration: design notes, not implemented yet ([`super::options`] is empty
//! on macOS and the Integration page is hidden there).
//!
//! On macOS most integration comes from the app bundle's `Info.plist` and is active as soon as
//! the app is in `/Applications` (or `~/Applications`); the options below are the parts a
//! user may want to switch on or off.
//!
//! # To do
//!
//! - [ ] **Default viewer.** `CFBundleDocumentTypes` in `Info.plist` (with `LSHandlerRank`
//!   `Alternate`) makes Specular a candidate; becoming the default is
//!   `NSWorkspace.setDefaultApplication(at:toOpenContentTypes:)` (macOS 12+), which asks the
//!   user. Record the previous handler per UTI (`urlForApplication(toOpen:)`) to restore it.
//!   The types are [`Env::default_viewer_types`](super::Env::default_viewer_types) (the image
//!   types plus the user's "More file types"), each turned into a UTI with
//!   `UTType(mimeType:)`; the ones macOS does not know need `UTImportedTypeDeclarations` in
//!   `Info.plist` (which declares only JPEG and PNG today). Unticking a type restores only
//!   the handlers still pointing at Specular. UNVERIFIED: nothing here is built on macOS yet.
//! - [ ] **Thumbnails and Quick Look.** A Quick Look thumbnail extension and preview
//!   extension (`.appex` in the bundle, `QLThumbnailProvider`) for JPEG XL, raw, EXR, PSD.
//!   Enabled with the bundle; the option toggles it with `pluginkit -e use|ignore -i <id>`.
//! - [ ] **Single instance.** Free on macOS: Launch Services sends `application:openURLs:` to
//!   the running app. Nothing to switch.
//! - [ ] **Finder actions.** A Finder Sync or Action extension ("Open in Specular", "Compare",
//!   "Set Desktop Picture") or Services (`NSServices` in `Info.plist`); toggled with
//!   `pluginkit` like thumbnails.
//! - [ ] **Set as wallpaper.** `NSWorkspace.setDesktopImageURL(_:for:options:)` for every
//!   `NSScreen`.
//! - [ ] **Search.** Spotlight indexes images itself; a Spotlight importer (`.mdimporter`) for
//!   formats it cannot read. Low priority.

// ----- Seams ([`super::shell`]) -----------------------------------------------------------
//
// Plain types on every platform (so they type-check elsewhere too); the AppKit and extension
// work goes behind them under `cfg(target_os = "macos")`.

use std::path::Path;

use super::shell::{self, LauncherMenu, PreviewProvider, PropertyIndexer};
use super::{Env, Labels, Result};

/// The Dock icon's menu.
pub struct DockMenu;

impl LauncherMenu for DockMenu {
    fn set_actions(&self, _env: &Env, _labels: &Labels) -> Result<()> {
        // TODO(macos): answer `applicationDockMenu:` in the app delegate with an `NSMenu` of
        // `shell::DESKTOP_ACTIONS` (labels in the current language), each item doing what its
        // `flag()` does in the running app. Static `UIApplicationShortcutItems` do not exist
        // on macOS; the menu is built at runtime only.
        shell::unsupported()
    }

    fn add_recent(&self, _env: &Env, _file: &Path) -> Result<()> {
        // TODO(macos): `NSDocumentController.sharedDocumentController.noteNewRecentDocumentURL`
        // (the Dock's and the File menu's "Open Recent"); needs `CFBundleDocumentTypes`.
        shell::unsupported()
    }

    fn clear(&self, _env: &Env) -> Result<()> {
        // TODO(macos): `clearRecentDocuments:` (only Specular's own list exists on macOS).
        shell::unsupported()
    }
}

/// Finder thumbnails and Quick Look previews.
pub struct QuickLook;

impl PreviewProvider for QuickLook {
    fn register(&self, _env: &Env, _types: &[&str]) -> Result<()> {
        // TODO(macos): Quick Look thumbnail (`QLThumbnailProvider`) and preview
        // (`QLPreviewingController`) app extensions in the bundle, declaring the UTIs of
        // `types` (exported for the ones macOS does not know: JPEG XL before 14, QOI, raw
        // flavours); turned on and off with `pluginkit -e use|ignore -i <bundle id>`, the
        // previous state recorded so off restores it. Sandboxed: decode only, like
        // `specular --thumbnail`.
        shell::unsupported()
    }

    fn unregister(&self, _env: &Env) -> Result<()> {
        shell::unsupported()
    }
}

/// Spotlight metadata.
pub struct SpotlightImporter;

impl PropertyIndexer for SpotlightImporter {
    fn register(&self, _env: &Env, _types: &[&str]) -> Result<()> {
        // TODO(macos): a `.mdimporter` bundle in Contents/Library/Spotlight giving
        // kMDItemPixelWidth/Height, kMDItemAcquisitionMake/Model, kMDItemFNumber,
        // kMDItemContentCreationDate from `specular-metadata` for the UTIs Spotlight cannot
        // read itself; re-indexed with `mdimport -r`.
        shell::unsupported()
    }

    fn unregister(&self, _env: &Env) -> Result<()> {
        shell::unsupported()
    }
}
