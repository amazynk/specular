//! Power source, battery and thermal state on Linux and Windows (macOS: `macos/power.rs`).

#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(windows)]
pub(crate) mod windows;
