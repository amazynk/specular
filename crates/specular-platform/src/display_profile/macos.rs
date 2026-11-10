//! Display profiles on macOS: the ICC data of the screen's `NSColorSpace` (main thread only).

use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;

use super::{DisplayProfile, DisplayQuery};

pub(super) fn profile(q: &DisplayQuery) -> DisplayProfile {
    let Some(mtm) = MainThreadMarker::new() else {
        return DisplayProfile::Unknown;
    };
    let screens = NSScreen::screens(mtm);
    let by_name = q.monitor.as_deref().and_then(|name| {
        screens
            .iter()
            .find(|s| s.localizedName().to_string() == name)
    });
    let screen = by_name
        .or_else(|| {
            q.index
                .and_then(|i| (i < screens.count()).then(|| screens.objectAtIndex(i)))
        })
        .or_else(|| NSScreen::mainScreen(mtm));
    let icc = screen
        .and_then(|s| s.colorSpace())
        .and_then(|cs| cs.ICCProfileData())
        .map(|d| d.to_vec());
    match icc {
        Some(v) if v.len() >= 132 => DisplayProfile::Icc(v),
        _ => DisplayProfile::Unknown,
    }
}
