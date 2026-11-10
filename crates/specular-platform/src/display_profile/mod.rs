//! The colour profile of the display a window is on (DESIGN.md §9).
//!
//! * Linux/BSD: when the Wayland compositor speaks the colour-management protocol
//!   (`wp_color_manager_v1`), it already converts sRGB surfaces to the output's profile, so
//!   [`DisplayProfile::Managed`] tells the caller to keep sRGB output. Otherwise colord
//!   (D-Bus: the device of the RandR output, its default profile), then the X11 `_ICC_PROFILE`
//!   root-window atom (`_ICC_PROFILE_<n>` for the n-th monitor; the ICC Profiles in X spec).
//! * Windows: `GetICMProfileW` on a DC of the monitor's display device.
//! * macOS: `NSScreen.colorSpace.ICCProfileData` (main thread only; elsewhere `Unknown`).
//!
//! Blocking (D-Bus and X round trips, file reads; a few milliseconds): call it off the UI thread
//! except on macOS.

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// Which display to ask about. Both fields come from the windowing toolkit (winit's
/// `MonitorHandle::name` and the monitor's position in `available_monitors`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DisplayQuery {
    /// The monitor's system name: the RandR output (`DP-1`) on X11 and Wayland, the GDI device
    /// (`\\.\DISPLAY1`) on Windows, the localized screen name on macOS.
    pub monitor: Option<String>,
    /// Index of the monitor in the system's list (X11 `_ICC_PROFILE_<n>`).
    pub index: Option<usize>,
}

/// What is known about the display's colour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayProfile {
    /// The system compositor colour-manages sRGB content itself: output plain sRGB.
    Managed,
    /// The display's ICC profile.
    Icc(Vec<u8>),
    /// Nothing found: assume sRGB.
    Unknown,
}

/// Looks up the colour profile of a display. Never fails; problems are logged and give
/// [`DisplayProfile::Unknown`].
pub fn display_profile(query: &DisplayQuery) -> DisplayProfile {
    #[cfg(all(unix, not(target_os = "macos")))]
    return linux::profile(query);
    #[cfg(target_os = "macos")]
    return macos::profile(query);
    #[cfg(windows)]
    return windows::profile(query);
    #[cfg(not(any(unix, windows)))]
    {
        let _ = query;
        DisplayProfile::Unknown
    }
}

/// Reads an ICC file named by the system, with a sanity check of its header.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn read_icc(path: &std::path::Path) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(d) if d.len() >= 132 && &d[36..40] == b"acsp" => Some(d),
        Ok(_) => {
            specular_log::warn!("display profile {}: not an ICC profile", path.display());
            None
        }
        Err(e) => {
            specular_log::warn!("display profile {}: {e}", path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whatever this machine has; must not panic or hang.
    #[test]
    fn lookup_runs() {
        let t0 = std::time::Instant::now();
        let p = display_profile(&DisplayQuery::default());
        let kind = match &p {
            DisplayProfile::Managed => "managed".to_owned(),
            DisplayProfile::Icc(d) => format!("icc {} bytes", d.len()),
            DisplayProfile::Unknown => "unknown".to_owned(),
        };
        eprintln!("display profile: {kind} in {:?}", t0.elapsed());
    }
}
