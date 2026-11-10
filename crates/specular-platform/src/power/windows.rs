//! Power source, battery level and battery saver on Windows (`GetSystemPowerStatus`), and EcoQoS
//! for background threads (`SetThreadInformation(ThreadPowerThrottling)`, DESIGN.md §12.2
//! "Hybrid core awareness").
//!
//! Windows has no user-mode thermal-state API (the ACPI thermal zones need WMI and admin rights),
//! so `thermal` stays `Nominal`.

use std::cell::Cell;

use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadInformation, THREAD_POWER_THROTTLING_CURRENT_VERSION,
    THREAD_POWER_THROTTLING_EXECUTION_SPEED, THREAD_POWER_THROTTLING_STATE, ThreadPowerThrottling,
};

use crate::{PowerState, ThreadClass};

pub(crate) fn state() -> PowerState {
    let mut st = SYSTEM_POWER_STATUS::default();
    // SAFETY: `st` is a live, writable SYSTEM_POWER_STATUS; the call only fills it in. About a
    // microsecond (it reads a cached kernel structure), so no caching is needed.
    #[allow(unsafe_code)]
    let ok = unsafe { GetSystemPowerStatus(&mut st) }.is_ok();
    if !ok {
        return PowerState::default();
    }
    from_status(&st)
}

fn from_status(st: &SYSTEM_POWER_STATUS) -> PowerState {
    // BatteryFlag 128: no system battery; 255: unknown.
    let battery = st.BatteryFlag & 128 == 0 && st.BatteryFlag != 255;
    PowerState {
        // ACLineStatus 0: offline, 1: online, 255: unknown.
        on_battery: battery && st.ACLineStatus == 0,
        battery_percent: (battery && st.BatteryLifePercent <= 100).then_some(st.BatteryLifePercent),
        // SystemStatusFlag 1: battery saver is on.
        low_power: st.SystemStatusFlag == 1,
        ..PowerState::default()
    }
}

thread_local! {
    /// Whether this thread was last put on EcoQoS, so per-job calls cost nothing when unchanged.
    static ECO: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Prefetch, thumbnails and idle work run with EcoQoS (efficiency cores, lower clocks);
/// interactive work explicitly opts out, so a pool thread that ran background work before is
/// not left throttled. Windows 10 1709+; older systems reject the call, which is ignored.
pub(crate) fn set_thread_class(class: ThreadClass) {
    let eco = matches!(class, ThreadClass::Utility | ThreadClass::Background);
    if ECO.get() == Some(eco) {
        return;
    }
    if set_eco(eco) {
        ECO.set(Some(eco));
    }
}

/// Puts the calling thread on EcoQoS (`true`) or high QoS (`false`). Returns whether it worked.
pub(crate) fn set_eco(eco: bool) -> bool {
    let state = THREAD_POWER_THROTTLING_STATE {
        Version: THREAD_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: THREAD_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: if eco {
            THREAD_POWER_THROTTLING_EXECUTION_SPEED
        } else {
            0
        },
    };
    // SAFETY: GetCurrentThread takes no arguments and returns the calling thread's
    // pseudo-handle, valid while the thread runs and never closed.
    #[allow(unsafe_code)]
    let me = unsafe { GetCurrentThread() };
    // SAFETY: `me` is the calling thread's pseudo-handle; the pointer and size describe the live
    // `state` above, a THREAD_POWER_THROTTLING_STATE as ThreadPowerThrottling requires; the call
    // only reads it and affects only this thread.
    #[allow(unsafe_code)]
    let r = unsafe {
        SetThreadInformation(
            me,
            ThreadPowerThrottling,
            (&raw const state).cast(),
            size_of::<THREAD_POWER_THROTTLING_STATE>() as u32,
        )
    };
    r.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_flags() {
        let desktop = SYSTEM_POWER_STATUS {
            ACLineStatus: 1,
            BatteryFlag: 128,
            BatteryLifePercent: 255,
            ..Default::default()
        };
        assert_eq!(from_status(&desktop), PowerState::default());
        let unplugged = SYSTEM_POWER_STATUS {
            ACLineStatus: 0,
            BatteryFlag: 0,
            BatteryLifePercent: 42,
            SystemStatusFlag: 1,
            ..Default::default()
        };
        let s = from_status(&unplugged);
        assert!(s.on_battery && s.low_power);
        assert_eq!(s.battery_percent, Some(42));
    }
}
