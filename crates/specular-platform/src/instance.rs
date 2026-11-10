//! One viewer per copy of Specular (Windows; Linux has D-Bus activation instead).
//!
//! Files opened together go to one window: selecting several images in Explorer and pressing
//! Enter starts one process per file, all at once, before any of them listens on its socket. A
//! named mutex per copy (the caller names it after the copy's socket folder, so a portable copy
//! and an installed one stay apart) tells each starting process whether it is the first; the
//! others hand their files to the first over the remote-control socket once it listens.
//! [`allow_foreground`] passes on the right to come to the front, which Windows gives only to
//! the process the user just started.

/// Whether this process is the first of the viewers named `name` (a short name without `\`).
/// The first keeps the name for as long as it runs; a later caller gets false and keeps
/// nothing, so it can ask again (the first may have exited meanwhile). Elsewhere always true.
pub fn claim(name: &str) -> bool {
    #[cfg(windows)]
    return win::claim(name);
    #[cfg(not(windows))]
    {
        let _ = name;
        true
    }
}

/// Lets process `pid` (the running viewer) bring its window to the front. Only takes effect
/// when this process may come to the front itself, as one the user just started may.
pub fn allow_foreground(pid: u32) {
    #[cfg(windows)]
    win::allow_foreground(pid);
    #[cfg(not(windows))]
    let _ = pid;
}

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
    use windows::core::HSTRING;

    pub(super) fn claim(name: &str) -> bool {
        // Local\: this session's namespace (another user's viewer is not ours).
        let name = HSTRING::from(format!("Local\\{name}"));
        #[allow(unsafe_code)]
        // SAFETY: default security, not owned, and a NUL-terminated name that outlives the
        // call; CreateMutexW reads nothing else.
        let handle = match unsafe { CreateMutexW(None, false, &name) } {
            Ok(h) => h,
            // No mutex to decide with: carry on as the first (a window of its own).
            Err(_) => return true,
        };
        #[allow(unsafe_code)]
        // SAFETY: reads this thread's last error, which the CreateMutexW call above set.
        let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        if existed {
            #[allow(unsafe_code)]
            // SAFETY: the handle CreateMutexW returned above, closed once, here.
            let _ = unsafe { CloseHandle(handle) };
            return false;
        }
        // Kept open while this viewer runs: Windows closes it when the process ends, also
        // when it crashes, and the name is free again.
        true
    }

    pub(super) fn allow_foreground(pid: u32) {
        #[allow(unsafe_code)]
        // SAFETY: takes a process id and touches no memory; it fails, harmlessly, when this
        // process may not hand the right on.
        let _ = unsafe { AllowSetForegroundWindow(pid) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn the_first_keeps_the_name() {
        let name = format!("specular-test-instance-{}", std::process::id());
        assert!(claim(&name), "the first");
        assert!(!claim(&name), "a later one");
        assert!(!claim(&name), "still taken: a later one keeps nothing");
        assert!(claim(&format!("{name}-other")), "another copy");
    }

    #[cfg(not(windows))]
    #[test]
    fn elsewhere_every_process_is_first() {
        assert!(claim("x") && claim("x"));
    }
}
