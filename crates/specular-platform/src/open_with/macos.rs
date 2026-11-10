//! "Open with…" on macOS: Launch Services through `NSWorkspace` (the modern replacement for
//! `LSCopyApplicationURLsForURL`). Safe bindings only; launching goes through `open -a`.

use std::path::{Path, PathBuf};

use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSString, NSURL};

use super::{Handler, How};

pub(super) fn handlers_for(path: &Path) -> Vec<Handler> {
    let Some(p) = path.to_str() else {
        return Vec::new();
    };
    let url = NSURL::fileURLWithPath(&NSString::from_str(p));
    let ws = NSWorkspace::sharedWorkspace();
    let default = ws
        .URLForApplicationToOpenURL(&url)
        .and_then(|u| u.path())
        .map(|p| p.to_string());
    let mut out = Vec::new();
    for app in ws.URLsForApplicationsToOpenURL(&url).iter() {
        let Some(app) = app.path().map(|p| p.to_string()) else {
            continue;
        };
        let bundle = PathBuf::from(&app);
        let name = bundle
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| app.clone());
        if name.eq_ignore_ascii_case("specular") || out.iter().any(|h: &Handler| h.id == app) {
            continue;
        }
        let is_default = default.as_deref() == Some(app.as_str());
        let h = Handler {
            name,
            id: app,
            is_default,
            how: How::Bundle(bundle),
        };
        if is_default {
            out.insert(0, h);
        } else {
            out.push(h);
        }
    }
    out
}
