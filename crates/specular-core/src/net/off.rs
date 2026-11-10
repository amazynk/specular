//! The no-network switch: when on, nothing in the process opens a connection.
//!
//! It is one process-wide flag, set from up to four places (any one turns it on):
//!
//! - [`By::Setting`]: Settings › Privacy › "No network".
//! - [`By::Flag`]: `--no-network` on the command line (this run only).
//! - [`By::Env`]: `SPECULAR_NO_NETWORK=1` (this run only).
//! - [`By::Policy`]: `no_network = true` in the system policy file (managed installs).
//!
//! The only HTTP client (`specular_remote::Client`) checks [`is_off`] before it resolves a
//! name or opens a socket, and refuses with its `NetworkOff` error; the requests it refuses
//! are never reported to the download registry, so the "Downloads" indicator stays hidden.
//! Features check [`is_off`] first and show "Network is off" instead of trying.

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

/// What turned the switch on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum By {
    Setting,
    Flag,
    Env,
    Policy,
}

impl By {
    /// Strongest first: the user cannot undo these from Settings.
    const ALL: [By; 4] = [By::Policy, By::Env, By::Flag, By::Setting];

    fn bit(self) -> u8 {
        match self {
            By::Setting => 1,
            By::Flag => 2,
            By::Env => 4,
            By::Policy => 8,
        }
    }
}

/// The environment variable that turns the switch on for one run.
pub const ENV: &str = "SPECULAR_NO_NETWORK";

/// The command-line flag that turns the switch on for one run.
pub const FLAG: &str = "--no-network";

static OFF: AtomicU8 = AtomicU8::new(0);
static REFUSED: AtomicU64 = AtomicU64::new(0);

/// Whether the network is off (nothing may connect).
pub fn is_off() -> bool {
    OFF.load(Ordering::Acquire) != 0
}

/// Turns the switch on or off for one source; the network stays off while any source has it
/// on. Returns whether [`is_off`] changed.
pub fn set(by: By, off: bool) -> bool {
    let was = if off {
        OFF.fetch_or(by.bit(), Ordering::AcqRel)
    } else {
        OFF.fetch_and(!by.bit(), Ordering::AcqRel)
    };
    let now = if off { was | by.bit() } else { was & !by.bit() };
    (was != 0) != (now != 0)
}

/// What holds the switch on, strongest first (policy, environment, flag, then the setting);
/// `None` while the network is on.
pub fn held_by() -> Option<By> {
    let v = OFF.load(Ordering::Acquire);
    By::ALL.into_iter().find(|b| v & b.bit() != 0)
}

/// Whether something other than the setting holds the switch on (the setting cannot turn
/// the network back on then).
pub fn locked() -> bool {
    held_by().is_some_and(|b| b != By::Setting)
}

/// Whether `value` (of [`ENV`]) turns the switch on: anything but empty, `0`, `false`, `no`
/// or `off`.
pub fn env_value_on(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no" | "off"
    )
}

/// Applies [`ENV`] from the environment (start-up).
pub fn apply_env() {
    let on = std::env::var(ENV).is_ok_and(|v| env_value_on(&v));
    set(By::Env, on);
}

/// Counts a connection refused because the network is off (the client calls this).
pub fn note_refused() {
    REFUSED.fetch_add(1, Ordering::Relaxed);
}

/// Connections refused so far (tests, diagnostics).
pub fn refused() -> u64 {
    REFUSED.load(Ordering::Relaxed)
}

/// The refusal in English (logs; the app words it with `network-off`).
pub const MESSAGE: &str = "the network is off in Settings";

/// The refusal as an `io::Error` (for code that returns those).
pub fn io_error() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, MESSAGE)
}

/// Whether `e` is [`io_error`]'s refusal.
pub fn is_refusal(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::PermissionDenied && e.to_string() == MESSAGE
}

/// `Err` while the network is off (and counts the refusal).
pub fn check() -> std::io::Result<()> {
    if is_off() {
        note_refused();
        Err(io_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test touches the process-wide switch (tests run in parallel).
    #[test]
    fn sources_combine_and_the_lock_follows_the_strongest() {
        assert!(!is_off());
        assert_eq!(held_by(), None);
        assert!(set(By::Setting, true));
        assert!(is_off() && !locked());
        assert_eq!(held_by(), Some(By::Setting));
        assert!(!set(By::Policy, true));
        assert_eq!(held_by(), Some(By::Policy));
        assert!(locked());
        assert!(!set(By::Setting, false));
        assert!(is_off(), "the policy still holds it");
        let before = refused();
        let e = check().unwrap_err();
        assert!(is_refusal(&e));
        assert_eq!(refused(), before + 1);
        assert!(set(By::Policy, false));
        assert!(!is_off() && check().is_ok());
        for v in ["1", "true", "yes", "on", " ON "] {
            assert!(env_value_on(v), "{v}");
        }
        for v in ["", "0", "false", "No", "off"] {
            assert!(!env_value_on(v), "{v}");
        }
    }
}
