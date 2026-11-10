//! Playback settings: delay clamping, looping, speed and the decode-ahead budget.

use std::time::Duration;

/// How stored frame delays become display times.
///
/// Browsers show GIF frames with a delay of 10 ms or less (0 or 1 centisecond) for 100 ms,
/// because many old GIFs were authored for that behaviour and play far too fast otherwise.
/// The default reproduces it; `clamp_at_or_below: None` shows delays as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DelayPolicy {
    /// Delays at or below this are replaced by `replacement`. `None` disables clamping.
    pub clamp_at_or_below: Option<Duration>,
    pub replacement: Duration,
}

impl DelayPolicy {
    /// Browser behaviour (Chromium, Firefox, WebKit): ≤ 10 ms → 100 ms.
    pub const BROWSER: DelayPolicy = DelayPolicy {
        clamp_at_or_below: Some(Duration::from_millis(10)),
        replacement: Duration::from_millis(100),
    };
    /// Delays exactly as stored. Zero-delay frames are then never shown during playback (their
    /// time slot is empty), which is right for GIFs that build one picture from several layers.
    pub const AS_STORED: DelayPolicy = DelayPolicy {
        clamp_at_or_below: None,
        replacement: Duration::from_millis(100),
    };

    pub fn apply(&self, stored: Duration) -> Duration {
        match self.clamp_at_or_below {
            Some(limit) if stored <= limit => self.replacement,
            _ => stored,
        }
    }
}

impl Default for DelayPolicy {
    fn default() -> Self {
        DelayPolicy::BROWSER
    }
}

/// How many times to play.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LoopMode {
    /// The file's loop count ([`specular_decode::AnimationInfo::loop_count`]; 0 = forever).
    #[default]
    File,
    /// Loop forever whatever the file says (the "loop forever" override).
    Forever,
    /// Play this many times (at least once).
    Times(u32),
}

impl LoopMode {
    /// Number of plays, `None` for forever.
    pub fn plays(self, file_loop_count: u32) -> Option<u64> {
        match self {
            LoopMode::File if file_loop_count == 0 => None,
            LoopMode::File => Some(u64::from(file_loop_count)),
            LoopMode::Forever => None,
            LoopMode::Times(n) => Some(u64::from(n.max(1))),
        }
    }
}

/// Slowest and fastest playback speed.
pub const MIN_SPEED: f64 = 0.25;
pub const MAX_SPEED: f64 = 4.0;

/// Player and decode-ahead settings.
#[derive(Clone, Debug)]
pub struct PlayerConfig {
    pub delay_policy: DelayPolicy,
    pub loop_mode: LoopMode,
    /// Play forwards then backwards. Needs every frame in memory, so it takes effect once the
    /// animation is fully cached (it plays forwards until then, and always when it does not fit
    /// the budget).
    pub ping_pong: bool,
    /// Speed multiplier, clamped to [`MIN_SPEED`]..=[`MAX_SPEED`].
    pub speed: f64,
    /// Start playing as soon as frame 0 is decoded; otherwise show frame 0 paused.
    pub autoplay: bool,
    /// Memory for decoded frames. Animations whose frames all fit are cached completely after
    /// the first loop (the decoder is then dropped); larger ones stream through a window of
    /// frames ahead of the playhead. At least two frames are always allowed.
    pub budget_bytes: usize,
    /// Frames decoded ahead of the playhead while streaming. Ignored when the frame count is
    /// known up front and everything fits the budget (then everything is decoded eagerly).
    pub max_ahead: u32,
    /// When the shown frame is later than this behind its scheduled time because decoding
    /// cannot keep up, the schedule is re-anchored (playback slows to decode speed) instead of
    /// skipping ever further ahead. Short hiccups (late ticks, a slow frame) are absorbed by
    /// skipping frames, keeping the schedule drift-free.
    pub max_lag: Duration,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        PlayerConfig {
            delay_policy: DelayPolicy::BROWSER,
            loop_mode: LoopMode::File,
            ping_pong: false,
            speed: 1.0,
            autoplay: true,
            budget_bytes: 256 << 20,
            max_ahead: 16,
            max_lag: Duration::from_millis(250),
        }
    }
}

/// Speed as a fixed-point factor of 1/1024, so that schedule arithmetic is exact integer math
/// (a deadline converted to an instant and back never lands a nanosecond early).
pub(crate) fn speed_q(speed: f64) -> u64 {
    let s = if speed.is_finite() { speed } else { 1.0 };
    (s.clamp(MIN_SPEED, MAX_SPEED) * 1024.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_clamp() {
        let p = DelayPolicy::BROWSER;
        assert_eq!(p.apply(Duration::ZERO), Duration::from_millis(100));
        assert_eq!(
            p.apply(Duration::from_millis(10)),
            Duration::from_millis(100)
        );
        assert_eq!(
            p.apply(Duration::from_millis(20)),
            Duration::from_millis(20)
        );
        assert_eq!(DelayPolicy::AS_STORED.apply(Duration::ZERO), Duration::ZERO);
    }

    #[test]
    fn plays() {
        assert_eq!(LoopMode::File.plays(0), None);
        assert_eq!(LoopMode::File.plays(3), Some(3));
        assert_eq!(LoopMode::Forever.plays(3), None);
        assert_eq!(LoopMode::Times(0).plays(0), Some(1));
    }

    #[test]
    fn speed_clamped() {
        assert_eq!(speed_q(1.0), 1024);
        assert_eq!(speed_q(100.0), 4096);
        assert_eq!(speed_q(0.0), 256);
        assert_eq!(speed_q(f64::NAN), 1024);
    }
}
