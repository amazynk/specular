//! Display profiles on Windows: `GetICMProfileW` on a DC of the monitor's display device.

use std::path::PathBuf;

use windows::Win32::Graphics::Gdi::{CreateDCW, DeleteDC};
use windows::Win32::UI::ColorSystem::GetICMProfileW;
use windows::core::{HSTRING, PCWSTR, PWSTR};

use super::{DisplayProfile, DisplayQuery, read_icc};

pub(super) fn profile(q: &DisplayQuery) -> DisplayProfile {
    match icm_path(q.monitor.as_deref()).and_then(|p| read_icc(&p)) {
        Some(icc) => DisplayProfile::Icc(icc),
        None => DisplayProfile::Unknown,
    }
}

/// The ICM profile path of display device `device` (`\\.\DISPLAY1`), or of the primary display.
#[allow(unsafe_code)]
fn icm_path(device: Option<&str>) -> Option<PathBuf> {
    let driver = HSTRING::from("DISPLAY");
    let device = device.map(HSTRING::from);
    // SAFETY: both strings are NUL-terminated wide strings that outlive the call; no DEVMODE.
    // The returned DC is checked and released below.
    let dc = unsafe {
        CreateDCW(
            &driver,
            device
                .as_ref()
                .map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
            PCWSTR::null(),
            None,
        )
    };
    if dc.is_invalid() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `dc` is a valid display DC; `buf` is writable for `len` UTF-16 units, the size
    // GetICMProfileW is told. It writes a NUL-terminated path of at most `len` units.
    let ok = unsafe { GetICMProfileW(dc, &mut len, Some(PWSTR(buf.as_mut_ptr()))) }.as_bool();
    // SAFETY: `dc` came from CreateDCW above and is not used after this.
    let _ = unsafe { DeleteDC(dc) };
    if !ok {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..end])))
}
