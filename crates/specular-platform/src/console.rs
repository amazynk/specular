//! Windows: the viewer is a GUI-subsystem program, so starting it from Explorer opens no console
//! window; its command-line modes (`--help`, `--hw-report`, `--remote`, errors) still print to
//! the terminal they were started from by attaching to that terminal's console.

use windows::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, FreeConsole, GetStdHandle, STD_ERROR_HANDLE, STD_HANDLE,
    STD_OUTPUT_HANDLE,
};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW};
use windows::core::PCWSTR;

/// Whether the standard handle `which` leads somewhere (a console, file or pipe).
fn has_handle(which: STD_HANDLE) -> bool {
    #[allow(unsafe_code)]
    // SAFETY: GetStdHandle only reads this process's standard handle table.
    let h = unsafe { GetStdHandle(which) };
    h.is_ok_and(|h| !h.is_invalid() && !h.0.is_null())
}

/// Attaches to the console of the terminal Specular was started from, when its output is not
/// redirected (a file or pipe is kept as it is). Returns whether it attached; false when started
/// from Explorer or another program without a console.
pub fn attach_parent_console() -> bool {
    if has_handle(STD_OUTPUT_HANDLE) {
        return false;
    }
    #[allow(unsafe_code)]
    // SAFETY: AttachConsole takes a process id (here the documented "parent process" value) and
    // touches no memory of ours; it fails when the parent has no console. The standard handles
    // it sets are read by std on every write, so later output goes to that console.
    let attached = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
    attached.is_ok()
}

/// Detaches from the console [`attach_parent_console`] attached to: the window then no longer
/// depends on the terminal (a Ctrl+C typed there later does not end the viewer), and further
/// output to it is dropped.
pub fn detach_console() {
    #[allow(unsafe_code)]
    // SAFETY: FreeConsole takes no arguments and only detaches this process from its console;
    // the standard handles left behind are invalid, which std's writes treat as a sink.
    let _ = unsafe { FreeConsole() };
}

/// Shows `text` in an error message box titled `title` when there is no terminal (nor file or
/// pipe) for error output: started from Explorer, a message printed to it would be lost. Both
/// are NUL-terminated UTF-16, prepared at compile time by the caller: the processor check calls
/// this before code built for x86-64-v3 may run, so nothing here converts or formats.
pub fn error_box_without_terminal(title: &[u16], text: &[u16]) {
    if has_handle(STD_ERROR_HANDLE) || title.last() != Some(&0) || text.last() != Some(&0) {
        return;
    }
    #[allow(unsafe_code)]
    // SAFETY: both pointers come from slices that end in NUL (checked above) and outlive the
    // call; MessageBoxW only reads them. No owner window: the box stands alone.
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        )
    };
}
