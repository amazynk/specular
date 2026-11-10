//! The main display's refresh rate and EDR headroom (`NSScreen`, main thread only).

use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;

use crate::ScreenInfo;

pub(crate) fn main_screen() -> Option<ScreenInfo> {
    let mtm = MainThreadMarker::new()?;
    let screen = NSScreen::mainScreen(mtm)?;
    let fps = screen.maximumFramesPerSecond();
    let finite = |v: f64| (v.is_finite() && v > 0.0).then_some(v as f32);
    Some(ScreenInfo {
        max_fps: u32::try_from(fps).ok().filter(|&f| f > 0),
        edr_current: finite(screen.maximumExtendedDynamicRangeColorComponentValue()),
        edr_potential: finite(screen.maximumPotentialExtendedDynamicRangeColorComponentValue()),
    })
}
