//! Thermal state, Low Power Mode and App Nap (`NSProcessInfo`).

use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSProcessInfoThermalState, NSString};

use crate::{PowerState, Thermal};

pub(crate) fn state() -> PowerState {
    let info = NSProcessInfo::processInfo();
    let thermal = match info.thermalState() {
        NSProcessInfoThermalState::Fair => Thermal::Fair,
        NSProcessInfoThermalState::Serious => Thermal::Serious,
        NSProcessInfoThermalState::Critical => Thermal::Critical,
        _ => Thermal::Nominal,
    };
    // macOS 12+, the minimum Specular supports (DESIGN.md §2).
    PowerState {
        thermal,
        low_power: info.isLowPowerModeEnabled(),
        ..PowerState::default()
    }
}

/// The token `beginActivityWithOptions:reason:` returns; ends the activity when dropped.
pub(crate) struct ActivityToken(Retained<ProtocolObject<dyn NSObjectProtocol>>);

pub(crate) fn begin(reason: &str, latency_critical: bool) -> ActivityToken {
    let mut options = NSActivityOptions::UserInitiatedAllowingIdleSystemSleep;
    if latency_critical {
        options |= NSActivityOptions::LatencyCritical;
    }
    let reason = NSString::from_str(reason);
    let token = NSProcessInfo::processInfo().beginActivityWithOptions_reason(options, &reason);
    specular_log::debug!(target: specular_log::STARTUP, "App Nap held off: {reason}");
    ActivityToken(token)
}

/// An activity that also keeps the display from sleeping (a movie plays).
pub(crate) fn begin_awake(reason: &str) -> ActivityToken {
    let options = NSActivityOptions::UserInitiated | NSActivityOptions::IdleDisplaySleepDisabled;
    let reason = NSString::from_str(reason);
    let token = NSProcessInfo::processInfo().beginActivityWithOptions_reason(options, &reason);
    ActivityToken(token)
}

impl Drop for ActivityToken {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let info = NSProcessInfo::processInfo();
        // SAFETY: `self.0` is the token `beginActivityWithOptions:reason:` returned, and it is
        // ended exactly once (here, on drop), which is all endActivity requires.
        unsafe { info.endActivity(&self.0) };
    }
}
