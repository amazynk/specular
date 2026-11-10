//! OS confinement of the host process, applied after the libraries are loaded and before the
//! first request is read.

use specular_sandbox::protocol::SandboxLevel;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// What [`apply`] achieved.
#[derive(Debug, Clone)]
pub struct Status {
    pub level: SandboxLevel,
    pub detail: String,
}

/// Process limits: address space (bounds every allocation, C++ included; not enforced by
/// macOS), no core dumps, few descriptors.
#[cfg(unix)]
pub fn limit_resources(memory: u64) -> Vec<String> {
    use rustix::process::{Resource, Rlimit, setrlimit};
    let mut failed = Vec::new();
    let mut set = |name: &str, r: Resource, v: u64| {
        let lim = Rlimit {
            current: Some(v),
            maximum: Some(v),
        };
        if let Err(e) = setrlimit(r, lim) {
            failed.push(format!("{name}: {e}"));
        }
    };
    set("RLIMIT_AS", Resource::As, memory);
    set("RLIMIT_CORE", Resource::Core, 0);
    set("RLIMIT_NOFILE", Resource::Nofile, 64);
    failed
}

/// The job object the viewer put the host in holds the limits; this checks them.
#[cfg(windows)]
pub fn limit_resources(memory: u64) -> Vec<String> {
    windows::limit_resources(memory)
}

#[cfg(not(any(unix, windows)))]
pub fn limit_resources(_memory: u64) -> Vec<String> {
    vec!["resource limits not implemented on this platform".into()]
}

/// Confines the process as far as this platform allows.
pub fn apply() -> Status {
    #[cfg(target_os = "linux")]
    {
        linux::apply()
    }
    #[cfg(target_os = "macos")]
    {
        macos::apply()
    }
    #[cfg(windows)]
    {
        windows::apply()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        Status {
            level: SandboxLevel::None,
            detail: "no sandbox on this platform".into(),
        }
    }
}
