//! Windows: the viewer starts the host already confined (`specular-sandbox`'s `win` module): an
//! AppContainer without capabilities, inside a job object (kill on close, one process, memory
//! limit), with creation-time mitigations (no dynamic code, no child processes, …).
//!
//! Here, once libheif is loaded, the host adds the mitigations that would have stopped
//! libheif from loading — only Microsoft-signed images from now on, no Win32k system calls
//! (best effort: impossible once anything loaded user32) — and then checks what it really
//! got: the token, the job and its limits, the policies. The hello reports that, and the
//! viewer refuses a host that is not confined at all.

use super::Status;
use specular_sandbox::protocol::SandboxLevel;
use std::ffi::c_void;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Security::{GetTokenInformation, TokenIsAppContainer};
use windows::Win32::System::JobObjects::{
    IsProcessInJob, JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, QueryInformationJobObject,
};
use windows::Win32::System::Threading::{
    GetProcessMitigationPolicy, PROCESS_MITIGATION_POLICY, ProcessChildProcessPolicy,
    ProcessDynamicCodePolicy, ProcessSignaturePolicy, ProcessSystemCallDisablePolicy,
    SetProcessMitigationPolicy,
};

/// `GetCurrentProcess()`: a pseudo handle, always valid, never closed.
const CURRENT_PROCESS: HANDLE = HANDLE(-1isize as *mut c_void);
/// `GetCurrentProcessToken()`: a pseudo handle (Windows 8 and later).
const CURRENT_TOKEN: HANDLE = HANDLE(-4isize as *mut c_void);

/// Every `PROCESS_MITIGATION_*_POLICY` used here is one DWORD of flags; bit 0 is the one set:
/// `MicrosoftSignedOnly`, `ProhibitDynamicCode`, `DisallowWin32kSystemCalls`,
/// `NoChildProcessCreation`.
const FLAG: u32 = 1;

fn set_policy(policy: PROCESS_MITIGATION_POLICY) -> Result<(), String> {
    let v: u32 = FLAG;
    #[allow(unsafe_code)]
    // SAFETY: each policy used here takes a 4-byte structure; `v` is that structure, read
    // during the call only.
    unsafe { SetProcessMitigationPolicy(policy, (&raw const v).cast(), size_of::<u32>()) }
        .map_err(|e| e.message())
}

fn policy_on(policy: PROCESS_MITIGATION_POLICY) -> bool {
    let mut v: u32 = 0;
    #[allow(unsafe_code)]
    // SAFETY: as in `set_policy`; `v` is writable for its 4 bytes during the call.
    let r = unsafe {
        GetProcessMitigationPolicy(
            CURRENT_PROCESS,
            policy,
            (&raw mut v).cast(),
            size_of::<u32>(),
        )
    };
    r.is_ok() && v & FLAG != 0
}

fn is_app_container() -> bool {
    let mut v: u32 = 0;
    let mut len = 0u32;
    #[allow(unsafe_code)]
    // SAFETY: TokenIsAppContainer returns a DWORD; `v` has room for it and `len` receives the
    // written size. The pseudo token handle needs no closing.
    let r = unsafe {
        GetTokenInformation(
            CURRENT_TOKEN,
            TokenIsAppContainer,
            Some((&raw mut v).cast()),
            size_of::<u32>() as u32,
            &raw mut len,
        )
    };
    r.is_ok() && v != 0
}

/// The limits of the job holding this process, if any.
fn job_limits() -> Option<JOBOBJECT_EXTENDED_LIMIT_INFORMATION> {
    let mut in_job = windows::core::BOOL(0);
    #[allow(unsafe_code)]
    // SAFETY: pseudo process handle; `in_job` is a writable BOOL.
    let r = unsafe { IsProcessInJob(CURRENT_PROCESS, None, &raw mut in_job) };
    if r.is_err() || !in_job.as_bool() {
        return None;
    }
    let mut lim = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    #[allow(unsafe_code)]
    // SAFETY: `None` queries the caller's job; `lim` is the structure the class names, with
    // its exact size, written during the call only.
    let r = unsafe {
        QueryInformationJobObject(
            None,
            JobObjectExtendedLimitInformation,
            (&raw mut lim).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            None,
        )
    };
    r.ok().map(|()| lim)
}

/// The memory limit is the job's (set by the viewer); this only checks that it is there.
pub fn limit_resources(_memory: u64) -> Vec<String> {
    match job_limits() {
        Some(l)
            if l.BasicLimitInformation.LimitFlags.0 & JOB_OBJECT_LIMIT_PROCESS_MEMORY.0 != 0 =>
        {
            Vec::new()
        }
        Some(_) => vec!["the job has no memory limit".into()],
        None => vec!["not in a job object (no memory limit)".into()],
    }
}

pub fn apply() -> Status {
    let mut have = Vec::new();
    let mut missing = Vec::new();

    let container = is_app_container();
    if container {
        have.push("appcontainer".to_string());
    } else {
        missing.push("appcontainer".to_string());
    }

    let job = match job_limits() {
        Some(l) => {
            let f = l.BasicLimitInformation.LimitFlags.0;
            let want = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.0
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS.0
                | JOB_OBJECT_LIMIT_PROCESS_MEMORY.0;
            if f & want == want && l.BasicLimitInformation.ActiveProcessLimit <= 1 {
                have.push(format!("job ({} MiB)", l.ProcessMemoryLimit >> 20));
                true
            } else {
                missing.push(format!("job limits (flags {f:#x})"));
                false
            }
        }
        None => {
            missing.push("job".into());
            false
        }
    };

    // Set now, after libheif and its dependencies are loaded.
    let mut policies = true;
    for (name, policy, required) in [
        ("signed images only", ProcessSignaturePolicy, true),
        ("no dynamic code", ProcessDynamicCodePolicy, true),
        ("no child processes", ProcessChildProcessPolicy, true),
        ("no win32k", ProcessSystemCallDisablePolicy, false),
    ] {
        // Already on from the creation attributes, or set here.
        let r = if policy_on(policy) {
            Ok(())
        } else {
            set_policy(policy)
        };
        match r {
            Ok(()) => have.push(name.to_string()),
            Err(e) => {
                missing.push(format!("{name}: {}", e.trim()));
                policies &= !required;
            }
        }
    }

    let level = if container && job && policies {
        SandboxLevel::Full
    } else if container || job || policies {
        SandboxLevel::Partial
    } else {
        SandboxLevel::None
    };
    let mut detail = have.join(", ");
    if !missing.is_empty() {
        detail.push_str(&format!("; missing: {}", missing.join(", ")));
    }
    Status { level, detail }
}
