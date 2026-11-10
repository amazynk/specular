//! Windows desktop integration. Implemented: recent files ([`recent`], the only entry of
//! [`super::options`] on Windows) and single instance (see below). The rest are design notes.
//!
//! Every option stays per-user (`HKEY_CURRENT_USER`, no administrator rights), records what it
//! changed under `%LOCALAPPDATA%\Specular\integration\` like the Linux records, and can be
//! undone.
//!
//! # To do
//!
//! - [ ] **Default viewer.** Register `HKCU\Software\Classes\Specular.Image` (ProgID with
//!   `shell\open\command` = `"specular.exe" "%1"`, `DefaultIcon`), add it under each extension's
//!   `OpenWithProgids`, and `HKCU\Software\RegisteredApplications` → `Software\Specular\Capabilities`
//!   (`FileAssociations`). Windows 10+ does not let programs set the default themselves (the
//!   `UserChoice` hash): open *Settings → Default apps* on Specular's page
//!   (`ms-settings:defaultapps?registeredAppUser=Specular`) and let the user pick. Then
//!   `SHChangeNotify(SHCNE_ASSOCCHANGED)`. The values are [`file_associations`] (the image
//!   types plus the user's "More file types"); the button opens [`DEFAULT_APPS_URI`].
//! - [ ] **Thumbnails.** An `IThumbnailProvider` COM server (DLL, in-process, registered under
//!   `HKCU\Software\Classes\CLSID` and `ShellEx\{e357fccd-a995-4576-b01f-234630154e96}` per
//!   extension) for the formats Windows cannot thumbnail (JPEG XL, AVIF without the store
//!   extension, raw, EXR, PSD). Needs a separate `cdylib` crate; `unsafe` confined there.
//! - [x] **Single instance** (the `general.single_instance` setting, no integration option): a
//!   named mutex per copy (`crate::instance`) picks the first of several starts; the others
//!   hand their files to it over the remote-control socket and let it come to the front
//!   (`AllowSetForegroundWindow`). In the app: `integration_cli::forward_to_running`.
//! - [ ] **Explorer context menu.** Windows 11's top-level menu needs an `IExplorerCommand`
//!   in a packaged (sparse MSIX) app; until then classic verbs under
//!   `HKCU\Software\Classes\SystemFileAssociations\image\shell\Specular.Open` (and `.Compare`
//!   with `MultiSelectModel=Player`, `.Wallpaper`), which appear under "Show more options".
//! - [ ] **Set as wallpaper.** `IDesktopWallpaper::SetWallpaper` (COM, per monitor), or
//!   `SystemParametersInfoW(SPI_SETDESKWALLPAPER)` as the fallback.
//! - [ ] **Search.** Windows Search indexes files itself; a property handler
//!   (`IPropertyStore`) for formats it cannot read would expose EXIF dates and dimensions.
//!   Low priority.

#[cfg(windows)]
pub mod recent;

/// The state of `id` on Windows.
#[cfg(windows)]
pub(super) fn status(env: &Env, id: super::OptionId) -> super::Status {
    let (state, targets) = match id {
        super::OptionId::RecentFiles => recent::status(env),
        _ => (
            super::State::Unavailable(super::Unavailable::Platform),
            Vec::new(),
        ),
    };
    super::Status { id, state, targets }
}

#[cfg(windows)]
pub(super) fn apply(env: &Env, id: super::OptionId) -> Result<super::Status> {
    match id {
        super::OptionId::RecentFiles => recent::apply(env)?,
        _ => return shell::unsupported(),
    }
    Ok(status(env, id))
}

#[cfg(windows)]
pub(super) fn remove(env: &Env, id: super::OptionId) -> Result<super::Status> {
    match id {
        super::OptionId::RecentFiles => recent::remove(env)?,
        _ => return shell::unsupported(),
    }
    Ok(status(env, id))
}

// ----- Default viewer: what to register --------------------------------------------------
//
// Plain data on every platform, so it is tested on Linux. The registry writes themselves
// (`HKCU\Software\Specular\Capabilities\FileAssociations`, one value per extension) are not
// written yet: UNVERIFIED on Windows.

/// The ProgID every association points at.
pub const PROG_ID: &str = "Specular.Image";

/// Settings → Default apps, on Specular's page: Windows 10+ lets only the user change the
/// default, so the "More file types" choices are registered and then this page is opened.
pub const DEFAULT_APPS_URI: &str = "ms-settings:defaultapps?registeredAppUser=Specular";

/// The `Capabilities\FileAssociations` values (".ext" → [`PROG_ID`]) for the types the
/// default viewer claims in `env` (the raster image types plus the user's extra types).
/// Multi-part extensions (`nii.gz`) are left out: Windows associates by the last one only.
pub fn file_associations(env: &Env) -> Vec<(String, &'static str)> {
    let types = env.default_viewer_types();
    let mut out: Vec<(String, &'static str)> = Vec::new();
    for (mime, exts) in super::mime::IMAGE_TYPES {
        if !types.contains(mime) {
            continue;
        }
        for e in *exts {
            let name = format!(".{e}");
            if !e.contains('.') && !out.iter().any(|(n, _)| *n == name) {
                out.push((name, PROG_ID));
            }
        }
    }
    out
}

// ----- Seams ([`super::shell`]) -----------------------------------------------------------
//
// Plain types on every platform (so they type-check on Linux too); the COM work lives behind
// them once written, under `cfg(windows)` and in its own crate where it needs a DLL.

use std::path::Path;

use super::shell::{self, LauncherMenu, PreviewProvider, PropertyIndexer, SystemCodec};
use super::{Env, Labels, Result};

/// The taskbar jump list.
pub struct JumpList;

impl LauncherMenu for JumpList {
    fn set_actions(&self, _env: &Env, _labels: &Labels) -> Result<()> {
        // TODO(windows): `ICustomDestinationList` (CLSID_DestinationList): BeginList,
        // AddUserTasks with one `IShellLinkW` per `shell::DESKTOP_ACTIONS` entry (path =
        // current exe, arguments = `flag()`, title via `IPropertyStore` PKEY_Title), CommitList.
        // Needs the AppUserModelID `app.specular.Specular` set on the process and the window.
        shell::unsupported()
    }

    fn add_recent(&self, _env: &Env, _file: &Path) -> Result<()> {
        // Recent files are the `RecentFiles` option ([`recent`]), as on Linux.
        shell::unsupported()
    }

    fn clear(&self, _env: &Env) -> Result<()> {
        // TODO(windows): `ICustomDestinationList::DeleteList(app id)` and
        // `IApplicationDestinations::RemoveAllDestinations`.
        shell::unsupported()
    }
}

/// Explorer thumbnails and the preview pane.
pub struct ShellPreview;

impl PreviewProvider for ShellPreview {
    fn register(&self, _env: &Env, _types: &[&str]) -> Result<()> {
        // TODO(windows): a `cdylib` crate with an in-process COM server implementing
        // `IThumbnailProvider` + `IInitializeWithStream` (and `IPreviewHandler` for the
        // preview pane) on the same decode path as `specular --thumbnail`; registered per user
        // under HKCU Software/Classes/CLSID/{…}/InprocServer32 and, per extension of `types`,
        // `.ext/ShellEx/{e357fccd-a995-4576-b01f-234630154e96}` (thumbnail) and
        // `{8895b1c6-b41f-4c1c-a562-0d564250836f}` (preview). Record the previous handler
        // values to restore them; `SHChangeNotify(SHCNE_ASSOCCHANGED)` afterwards.
        shell::unsupported()
    }

    fn unregister(&self, _env: &Env) -> Result<()> {
        shell::unsupported()
    }
}

/// Windows Search and Explorer columns.
pub struct PropertyHandler;

impl PropertyIndexer for PropertyHandler {
    fn register(&self, _env: &Env, _types: &[&str]) -> Result<()> {
        // TODO(windows): `IPropertyStore` + `IInitializeWithStream` in the same DLL as
        // `ShellPreview`, giving System.Image.HorizontalSize/VerticalSize, System.Photo.*
        // (camera, lens, ISO, exposure) and System.Photo.DateTaken from `specular-metadata`;
        // property handlers register machine-wide (HKLM PropertySystem/PropertyHandlers), so
        // this one needs the installer, not a per-user option.
        shell::unsupported()
    }

    fn unregister(&self, _env: &Env) -> Result<()> {
        shell::unsupported()
    }
}

/// A Windows Imaging Component decoder, so Photos, Paint and every WIC client read the
/// formats.
pub struct WicCodec;

impl SystemCodec for WicCodec {
    fn register(&self, _env: &Env, _types: &[&str]) -> Result<()> {
        // TODO(windows): `IWICBitmapDecoder` / `IWICBitmapFrameDecode` (+ the embedded
        // preview through `GetPreview`/`GetThumbnail`) over the registry decoders, in its own
        // DLL; registered in the WIC decoder category {7ED96837-96F0-4812-B211-F13C24117ED3}
        // with the container format GUID, the MIME types and extensions of `types` and the
        // magic-number patterns. WIC enumerates HKCR, so per-user (HKCU Classes) works.
        shell::unsupported()
    }

    fn unregister(&self, _env: &Env) -> Result<()> {
        shell::unsupported()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn associations_follow_the_choice() {
        let env = Env::under(Path::new("/nonexistent"), Path::new("specular.exe"));
        let base = file_associations(&env);
        let has = |v: &[(String, &str)], e: &str| v.iter().any(|(n, _)| n == e);
        assert!(has(&base, ".jpg") && has(&base, ".cr3") && has(&base, ".psd"));
        assert!(!has(&base, ".kra") && !has(&base, ".svg"));
        let env = env.with_extra_default_types("application/x-krita,application/x-nifti");
        let more = file_associations(&env);
        assert!(has(&more, ".kra") && has(&more, ".nii"));
        assert!(
            !has(&more, ".nii.gz"),
            "Windows matches the last extension only"
        );
        // Krita: `.kra` and `.krz`; NIfTI: `.nii`.
        assert_eq!(more.len(), base.len() + 3);
        assert!(more.iter().all(|(_, p)| *p == PROG_ID));
        assert!(DEFAULT_APPS_URI.starts_with("ms-settings:defaultapps?registeredAppUser="));
    }
}
