//! macOS: `NSSharingServicePicker` under the window's view, the Mail compose service
//! (`NSSharingServiceNameComposeEmail`), and `NSWorkspace.setDesktopImageURL` for every
//! screen, and another application's Service (`NSPerformService`) with the files on a pasteboard.
//! AppKit wants the main thread: the picker is shown from the UI thread, the others hop there
//! with `dispatch2::run_on_main`.

use std::io;
use std::path::{Path, PathBuf};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardWriting, NSPerformService, NSRunningApplication, NSScreen,
    NSSharingService, NSSharingServiceNameComposeEmail, NSSharingServicePicker, NSView,
    NSWorkspace,
};
use objc2_foundation::{NSArray, NSDictionary, NSRectEdge, NSString, NSURL};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

fn urls(files: &[PathBuf]) -> Retained<NSArray<AnyObject>> {
    let v: Vec<Retained<AnyObject>> = files
        .iter()
        .map(|f| {
            let u = NSURL::fileURLWithPath(&NSString::from_str(&f.to_string_lossy()));
            Retained::into_super(Retained::into_super(u))
        })
        .collect();
    NSArray::from_retained_slice(&v)
}

/// The share picker for `files`, under the window's content view.
#[allow(unsafe_code)]
pub(crate) fn share(files: &[PathBuf], window: &dyn HasWindowHandle) -> io::Result<()> {
    let mtm = MainThreadMarker::new().ok_or(io::ErrorKind::Unsupported)?;
    let RawWindowHandle::AppKit(h) = window
        .window_handle()
        .map_err(|e| io::Error::other(format!("{e:?}")))?
        .as_raw()
    else {
        return Err(io::ErrorKind::Unsupported.into());
    };
    // SAFETY: winit's AppKit handle points at the window's live NSView, used on the main
    // thread (checked above) while the window exists (the caller holds it).
    let view: &NSView = unsafe { h.ns_view.cast::<NSView>().as_ref() };
    let items = urls(files);
    // SAFETY: the items are NSURLs, which the picker accepts (its array is untyped).
    let picker = unsafe { NSSharingServicePicker::initWithItems(mtm.alloc(), &items) };
    let rect = view.bounds();
    picker.showRelativeToRect_ofView_preferredEdge(rect, view, NSRectEdge::MinY);
    Ok(())
}

/// A new Mail message with `files` attached.
#[allow(unsafe_code)]
pub(crate) fn email(files: &[PathBuf]) -> io::Result<()> {
    let files = files.to_vec();
    dispatch2::run_on_main(move |_mtm| {
        // SAFETY: reading an AppKit constant string.
        let name = unsafe { NSSharingServiceNameComposeEmail };
        let service = NSSharingService::sharingServiceNamed(name)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let items = urls(&files);
        // SAFETY: the items are file NSURLs, which the compose service takes as attachments
        // (its array is untyped).
        if !unsafe { service.canPerformWithItems(Some(&items)) } {
            return Err(io::Error::other("Mail cannot attach these files"));
        }
        // SAFETY: as above.
        unsafe { service.performWithItems(&items) };
        Ok(())
    })
}

/// Sets `image` as the desktop picture of every screen.
#[allow(unsafe_code)]
pub(crate) fn set_wallpaper(image: &Path) -> io::Result<&'static str> {
    let path = std::path::absolute(image)?;
    dispatch2::run_on_main(move |mtm| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let ws = NSWorkspace::sharedWorkspace();
        let options = NSDictionary::new();
        for screen in NSScreen::screens(mtm).iter() {
            // SAFETY: an empty options dictionary (its value types are what AppKit expects).
            unsafe { ws.setDesktopImageURL_forScreen_options_error(&url, &screen, &options) }
                .map_err(|e| io::Error::other(e.localizedDescription().to_string()))?;
        }
        Ok("NSWorkspace")
    })
}

/// The application Launch Services has for `bundle_id`: an install, or a build that has been
/// run once.
pub(crate) fn app_for_bundle_id(bundle_id: &str) -> Option<PathBuf> {
    NSWorkspace::sharedWorkspace()
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))
        .and_then(|u| u.path())
        .map(|p| PathBuf::from(p.to_string()))
}

/// None when no application with `bundle_id` runs; otherwise whether one has finished
/// launching.
pub(crate) fn app_running(bundle_id: &str) -> Option<bool> {
    let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
        bundle_id,
    ));
    if apps.is_empty() {
        return None;
    }
    Some(apps.iter().any(|a| a.isFinishedLaunching()))
}

/// The pasteboard files go to a Service on.
const SERVICE_PASTEBOARD: &str = "app.specular.Specular.service";

/// Runs the Service whose menu item is `name` with `files` on its pasteboard, as Finder's
/// Services menu does. The system lets a sandboxed provider read the files.
pub(crate) fn perform_service(name: &str, files: &[PathBuf]) -> io::Result<()> {
    let name = name.to_owned();
    let files = files.to_vec();
    dispatch2::run_on_main(move |_mtm| {
        let items: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = files
            .iter()
            .map(|f| {
                ProtocolObject::from_retained(NSURL::fileURLWithPath(&NSString::from_str(
                    &f.to_string_lossy(),
                )))
            })
            .collect();
        // One pasteboard of its own, reused: each call runs on the main thread, after the last
        // (a unique one per call would stay in the pasteboard server until logout).
        let pboard = NSPasteboard::pasteboardWithName(&NSString::from_str(SERVICE_PASTEBOARD));
        pboard.clearContents();
        if !pboard.writeObjects(&NSArray::from_retained_slice(&items)) {
            return Err(io::Error::other("Could not put the files on a pasteboard"));
        }
        if NSPerformService(&NSString::from_str(&name), Some(&pboard)) {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "The \u{201c}{name}\u{201d} service did not run"
            )))
        }
    })
}
