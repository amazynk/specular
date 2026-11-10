//! Windows: how the viewer starts a confined host.
//!
//! The host is created directly inside its confinement, by attributes on `CreateProcess`
//! (std's `spawn_with_attributes`), so it never runs unconfined:
//!
//! - **AppContainer** `Specular.DecodeHost` with no capabilities: no network, no named objects
//!   or files outside what "ALL APPLICATION PACKAGES" may read (system DLLs, `Program Files`),
//!   nothing writable, a low-box token (lower than low integrity). This is what Chromium's
//!   and Edge's "lowbox" sandboxed utility processes use.
//! - **Job object**: killed when the viewer's handle closes (the viewer exiting or crashing),
//!   at most one active process (no children), a per-process commit limit (the memory
//!   limit), no error dialogs on a crash, and every UI restriction.
//! - **Creation mitigations**: no dynamic code, no extension points (AppInit DLLs, IMEs, …),
//!   no remote or low-label images, System32 preferred for DLL search, mandatory bottom-up
//!   and high-entropy ASLR, heap corruption terminates, no non-system fonts; and the child
//!   process policy "restricted" (no `CreateProcess` at all).
//!
//! The host adds what it can only set after loading libheif (Microsoft-signed images only,
//! Win32k disabled) and reports the result in its hello; see the host's `sandbox/windows.rs`.
//!
//! IPC is the anonymous stdin/stdout pipes std creates for the child: inherited handles need no
//! name and no ACL, and nothing else can open them.

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::os::windows::process::{CommandExt, ProcThreadAttributeList};
use std::process::{Child, Command};
use std::sync::{Arc, OnceLock};
use windows::Win32::Foundation::{DUPLICATE_HANDLE_OPTIONS, DuplicateHandle, HANDLE};
use windows::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName,
};
use windows::Win32::Security::{FreeSid, PSID, SECURITY_CAPABILITIES};
use windows::Win32::System::JobObjects::{
    CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOB_OBJECT_UILIMIT,
    JOBOBJECT_BASIC_UI_RESTRICTIONS, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOBOBJECTINFOCLASS,
    JobObjectBasicUIRestrictions, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject,
};
use windows::Win32::System::Threading::{
    DETACHED_PROCESS, PROC_THREAD_ATTRIBUTE_CHILD_PROCESS_POLICY, PROC_THREAD_ATTRIBUTE_JOB_LIST,
    PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
};
use windows::core::{HRESULT, PCWSTR, w};

/// Name of the AppContainer profile (per user; created on first use).
pub const CONTAINER: &str = "Specular.DecodeHost";

/// Exit code of a host terminated by the viewer's watchdog.
pub const EXIT_TIMEOUT: u32 = 0x5350_0001;

// PROCESS_CREATION_MITIGATION_POLICY_* (winnt.h); the `windows` crate does not carry them.
const HEAP_TERMINATE: u64 = 1 << 12;
const BOTTOM_UP_ASLR: u64 = 1 << 16;
const HIGH_ENTROPY_ASLR: u64 = 1 << 20;
const EXTENSION_POINT_DISABLE: u64 = 1 << 32;
const PROHIBIT_DYNAMIC_CODE: u64 = 1 << 36;
const FONT_DISABLE: u64 = 1 << 48;
const IMAGE_LOAD_NO_REMOTE: u64 = 1 << 52;
const IMAGE_LOAD_NO_LOW_LABEL: u64 = 1 << 56;
const IMAGE_LOAD_PREFER_SYSTEM32: u64 = 1 << 60;

/// Mitigations set at creation. Not here: "Microsoft-signed images only" (libheif is not) and
/// "Win32k disabled" (a DLL importing user32 would fail to load); the host sets both at run
/// time once libheif is loaded. Strict handle checks are left out because a host started
/// without a console writes to an invalid stderr handle, which they turn into a crash.
pub const MITIGATIONS: u64 = HEAP_TERMINATE
    | BOTTOM_UP_ASLR
    | HIGH_ENTROPY_ASLR
    | EXTENSION_POINT_DISABLE
    | PROHIBIT_DYNAMIC_CODE
    | FONT_DISABLE
    | IMAGE_LOAD_NO_REMOTE
    | IMAGE_LOAD_NO_LOW_LABEL
    | IMAGE_LOAD_PREFER_SYSTEM32;

/// PROCESS_CREATION_CHILD_PROCESS_RESTRICTED.
const CHILD_PROCESS_RESTRICTED: u32 = 1;

/// HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS).
const ALREADY_EXISTS: HRESULT = HRESULT(0x8007_00B7_u32 as i32);

fn os_error(what: &str, e: &windows::core::Error) -> io::Error {
    let hr = e.code().0 as u32;
    // A Win32 error wrapped in an HRESULT keeps its ErrorKind (PermissionDenied, …).
    let kind = if hr & 0xFFFF_0000 == 0x8007_0000 {
        io::Error::from_raw_os_error((hr & 0xFFFF) as i32).kind()
    } else {
        io::ErrorKind::Other
    };
    io::Error::new(kind, format!("{what}: {e}"))
}

/// How to start the host.
#[derive(Debug, Clone)]
pub struct Options {
    /// Commit limit of the host process, bytes.
    pub memory_limit: u64,
    /// AppContainer and mitigations (false only for tests that check the probes work at all;
    /// the job object is always used).
    pub confine: bool,
}

/// A job object owned by the viewer. Dropping the last handle kills the host.
#[derive(Debug)]
pub struct Job(OwnedHandle);

impl Job {
    pub fn new(memory_limit: u64) -> io::Result<Job> {
        #[allow(unsafe_code)]
        // SAFETY: default security attributes, no name; the call has no other inputs.
        let h = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(|e| os_error("CreateJobObject", &e))?;
        #[allow(unsafe_code)]
        // SAFETY: `h` is a valid handle just returned to us and owned by nothing else; the
        // OwnedHandle closes it exactly once.
        let job = Job(unsafe { OwnedHandle::from_raw_handle(h.0) });

        let mut lim = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        lim.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        lim.BasicLimitInformation.ActiveProcessLimit = 1;
        lim.ProcessMemoryLimit = usize::try_from(memory_limit).unwrap_or(usize::MAX);
        job.set(JobObjectExtendedLimitInformation, &lim)?;
        // Every JOB_OBJECT_UILIMIT_* flag: no clipboard, desktop, global atoms, USER handles
        // of other processes, display or system settings, ExitWindows.
        let ui = JOBOBJECT_BASIC_UI_RESTRICTIONS {
            UIRestrictionsClass: JOB_OBJECT_UILIMIT(0xFF),
        };
        job.set(JobObjectBasicUIRestrictions, &ui)?;
        Ok(job)
    }

    fn handle(&self) -> HANDLE {
        HANDLE(self.0.as_raw_handle())
    }

    /// Sets one information class. `T` must be the structure `class` names.
    fn set<T>(&self, class: JOBOBJECTINFOCLASS, value: &T) -> io::Result<()> {
        let len = u32::try_from(size_of::<T>()).map_err(|_| io::Error::other("job info size"))?;
        #[allow(unsafe_code)]
        // SAFETY: the job handle is valid for `self`'s lifetime; `value` points to `len`
        // readable bytes holding the structure `class` expects (both callers pass matching
        // pairs) and is only read during the call.
        unsafe { SetInformationJobObject(self.handle(), class, (value as *const T).cast(), len) }
            .map_err(|e| os_error("SetInformationJobObject", &e))
    }

    /// Kills every process in the job (the watchdog's deadline action).
    pub fn terminate(&self) {
        #[allow(unsafe_code)]
        // SAFETY: the job handle is valid for `self`'s lifetime; terminating an empty or
        // already terminated job is harmless.
        let _ = unsafe { TerminateJobObject(self.handle(), EXIT_TIMEOUT) };
    }
}

/// An AppContainer SID, freed on drop.
struct Sid(PSID);

impl Drop for Sid {
    fn drop(&mut self) {
        #[allow(unsafe_code)]
        // SAFETY: the SID was allocated by CreateAppContainerProfile or
        // DeriveAppContainerSidFromAppContainerName, whose documentation says to free it with
        // FreeSid; it is freed once, here.
        unsafe {
            FreeSid(self.0);
        }
    }
}

/// The container's SID, creating its profile on first use (once per process; an existing
/// profile is fine).
fn container_sid() -> io::Result<Sid> {
    static PROFILE: OnceLock<Result<(), String>> = OnceLock::new();
    let created = PROFILE.get_or_init(|| {
        #[allow(unsafe_code)]
        // SAFETY: constant NUL-terminated wide strings; no capabilities (None).
        let r = unsafe {
            CreateAppContainerProfile(
                w!("Specular.DecodeHost"),
                w!("Specular decode host"),
                w!("Confined process that decodes HEIF and RAR files for Specular"),
                None,
            )
        };
        match r {
            Ok(sid) => {
                drop(Sid(sid));
                Ok(())
            }
            Err(e) if e.code() == ALREADY_EXISTS => Ok(()),
            Err(e) => Err(format!("CreateAppContainerProfile: {e}")),
        }
    });
    if let Err(e) = created {
        return Err(io::Error::other(e.clone()));
    }
    #[allow(unsafe_code)]
    // SAFETY: constant NUL-terminated wide string; the returned SID is owned by the guard.
    let sid = unsafe { DeriveAppContainerSidFromAppContainerName(w!("Specular.DecodeHost")) }
        .map_err(|e| os_error("DeriveAppContainerSidFromAppContainerName", &e))?;
    Ok(Sid(sid))
}

/// Starts `cmd` confined (see the module docs) inside a new job. The returned job must live as
/// long as the host should.
pub fn spawn(cmd: &mut Command, opts: &Options) -> io::Result<(Child, Arc<Job>)> {
    let job = Arc::new(Job::new(opts.memory_limit)?);
    // PROC_THREAD_ATTRIBUTE_JOB_LIST takes an array of handles: one.
    let jobs: [RawHandle; 1] = [job.0.as_raw_handle()];
    // No console at all. CREATE_NO_WINDOW would still give the host a hidden one, whose
    // conhost.exe is a child process: the child-process policy blocks it and the host dies in
    // DLL initialisation (STATUS_DLL_INIT_FAILED). Its I/O is the pipes, not a console.
    cmd.creation_flags(DETACHED_PROCESS.0);
    let child = if opts.confine {
        let sid = container_sid()?;
        let caps = SECURITY_CAPABILITIES {
            AppContainerSid: sid.0,
            Capabilities: std::ptr::null_mut(),
            CapabilityCount: 0,
            Reserved: 0,
        };
        let mitigations: u64 = MITIGATIONS;
        let child_policy: u32 = CHILD_PROCESS_RESTRICTED;
        let attrs = ProcThreadAttributeList::build()
            .attribute(PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize, &caps)
            .attribute(PROC_THREAD_ATTRIBUTE_JOB_LIST as usize, &jobs)
            .attribute(
                PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY as usize,
                &mitigations,
            )
            .attribute(
                PROC_THREAD_ATTRIBUTE_CHILD_PROCESS_POLICY as usize,
                &child_policy,
            )
            .finish()?;
        // `sid` (which `caps` points into) outlives the call.
        let child = cmd.spawn_with_attributes(&attrs);
        drop(attrs);
        drop(sid);
        child?
    } else {
        let attrs = ProcThreadAttributeList::build()
            .attribute(PROC_THREAD_ATTRIBUTE_JOB_LIST as usize, &jobs)
            .finish()?;
        cmd.spawn_with_attributes(&attrs)?
    };
    Ok((child, job))
}

/// Duplicates `file` into `process` with read access only (`FILE_READ_DATA`,
/// `FILE_READ_ATTRIBUTES`, `SYNCHRONIZE`) and returns the new handle's value in that process.
/// The AppContainer host cannot open files itself; a duplicated handle carries the access that
/// was checked when the viewer opened the file. The host owns (and closes) the copy.
pub fn duplicate_for_reading(process: &impl AsRawHandle, file: &std::fs::File) -> io::Result<u64> {
    const FILE_READ_DATA: u32 = 0x0001;
    const FILE_READ_ATTRIBUTES: u32 = 0x0080;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    // GetCurrentProcess(): the constant pseudo handle -1.
    let current = HANDLE(-1isize as *mut core::ffi::c_void);
    let mut out = HANDLE::default();
    // SAFETY: `file` and `process` are live objects whose handles stay open for the call (they
    // are borrowed), `current` is the documented pseudo handle, and `out` is a valid place for
    // the result. Without DUPLICATE_CLOSE_SOURCE nothing of ours is closed.
    #[allow(unsafe_code)]
    let r = unsafe {
        DuplicateHandle(
            current,
            HANDLE(file.as_raw_handle()),
            HANDLE(process.as_raw_handle()),
            &mut out,
            FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            false,
            DUPLICATE_HANDLE_OPTIONS(0),
        )
    };
    r.map_err(io::Error::from)?;
    Ok(out.0 as usize as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mitigation_bits() {
        // Values from winnt.h.
        assert_eq!(PROHIBIT_DYNAMIC_CODE, 0x0000_0010_0000_0000);
        assert_eq!(IMAGE_LOAD_PREFER_SYSTEM32, 0x1000_0000_0000_0000);
        assert_eq!(MITIGATIONS & (1 << 44), 0, "signed-only is set at run time");
        assert_eq!(MITIGATIONS & (1 << 28), 0, "win32k is disabled at run time");
    }

    #[test]
    fn job_kills_on_terminate() {
        let job = Arc::new(Job::new(256 << 20).unwrap());
        // Not through cmd.exe: the job allows one process.
        let mut cmd = Command::new("ping.exe");
        cmd.args(["-n", "30", "127.0.0.1"])
            .stdout(std::process::Stdio::null());
        let (mut child, job2) = {
            // Unconfined: the test binary's folder may not be readable by the container.
            let opts = Options {
                memory_limit: 256 << 20,
                confine: false,
            };
            spawn(&mut cmd, &opts).unwrap()
        };
        drop(job);
        job2.terminate();
        let st = child.wait().unwrap();
        assert_eq!(st.code(), Some(EXIT_TIMEOUT as i32));
    }
}
