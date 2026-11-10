//! Windows: the process's AppUserModelID, the id the taskbar groups windows, pinned shortcuts
//! and jump lists by. The Start-menu shortcut the installer makes carries the same one.

use specular_log::{SETTINGS, warn};
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::HSTRING;

pub fn set(id: &str) {
    let id = HSTRING::from(id);
    #[allow(unsafe_code)]
    // SAFETY: `id` is a NUL-terminated UTF-16 string that outlives the call; the shell copies
    // it. Called on the main thread before any window exists, as the function requires.
    let r = unsafe { SetCurrentProcessExplicitAppUserModelID(&id) };
    if let Err(e) = r {
        warn!(target: SETTINGS, "taskbar id: {}", e.message());
    }
}
