//! Present-time pacing of an animation: when each frame actually went to the compositor
//! (`queue.present`) against when the schedule wanted it.
//!
//! The player's own lateness (`Pacing`) is measured at the tick; this follows the frame on to
//! the present call, which is as close to the screen as wgpu lets us see. Three numbers matter:
//!
//! - **latency**: present − due, per frame;
//! - **jitter**: the error of each present-to-present interval against the scheduled interval
//!   (what the eye sees as uneven motion);
//! - **drift**: elapsed present time against the file's nominal time (the sum of the shown
//!   frames' delays after the delay policy and speed), measured from the first frame's due
//!   time over drop-free stretches. Without drift it stays at the present latency; a schedule
//!   that slips (re-anchoring on late ticks, rounding) makes it grow.
//!
//! `render:debug` logs a summary every [`REPORT_EVERY`] and when the player stops;
//! `render:trace` logs every frame (for offline analysis).

use std::time::{Duration, Instant};

use specular_log::{RENDER, debug, trace};

/// How often a summary is logged while playing.
pub const REPORT_EVERY: Duration = Duration::from_secs(10);
/// Samples kept for percentiles (one minute at 100 fps, ample for a summary window).
const MAX_SAMPLES: usize = 6000;

/// A frame handed to the GPU and not presented yet.
#[derive(Clone, Copy, Debug)]
struct Pending {
    index: u32,
    due: Instant,
    /// Nominal play time of this frame (ns since the first frame), after delay policy and speed.
    nominal_ns: u64,
    /// Frames skipped right before this one.
    dropped: u64,
}

/// Statistics over one window (and over the whole run, for the final report).
#[derive(Clone, Debug, Default)]
pub struct Window {
    pub presents: u64,
    pub dropped: u64,
    /// Present minus due, ms.
    pub latency: Vec<f32>,
    /// |Δpresent − Δdue| of consecutive frames without a drop between them, ms.
    pub jitter: Vec<f32>,
}

impl Window {
    fn push(v: &mut Vec<f32>, x: f32) {
        if v.len() < MAX_SAMPLES {
            v.push(x);
        }
    }

    /// The `p`-th percentile (0–100) of `v`.
    pub fn pct(v: &[f32], p: f32) -> f32 {
        if v.is_empty() {
            return 0.0;
        }
        let mut s = v.to_vec();
        s.sort_by(f32::total_cmp);
        let i = ((p / 100.0) * (s.len() - 1) as f32).round() as usize;
        s[i.min(s.len() - 1)]
    }

    fn mean(v: &[f32]) -> f32 {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f32>() / v.len() as f32
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "{} presents, {} dropped; latency mean {:.2} p50 {:.2} p95 {:.2} max {:.2} ms; \
             interval error mean {:.2} p95 {:.2} max {:.2} ms",
            self.presents,
            self.dropped,
            Self::mean(&self.latency),
            Self::pct(&self.latency, 50.0),
            Self::pct(&self.latency, 95.0),
            Self::pct(&self.latency, 100.0),
            Self::mean(&self.jitter),
            Self::pct(&self.jitter, 95.0),
            Self::pct(&self.jitter, 100.0),
        )
    }
}

/// Follows shown frames to their present.
#[derive(Debug)]
pub struct PresentTrace {
    pending: Option<Pending>,
    /// First presented frame of a drop-free run: (due, nominal ns), the origin of drift. The
    /// due time rather than the present, so a late first frame (startup) is not counted as
    /// drift of every later one.
    origin: Option<(Instant, u64)>,
    /// Last presented frame: (present, due).
    last: Option<(Instant, Instant)>,
    /// Nominal time of the next frame to show (ns), and the effective delay of the one shown.
    nominal_ns: u64,
    /// Drift (present elapsed − nominal elapsed) at the latest present, ms; reset by drops.
    pub drift_ms: f64,
    /// Largest |drift| seen.
    pub drift_max_ms: f64,
    window: Window,
    pub total: Window,
    window_start: Option<Instant>,
}

impl Default for PresentTrace {
    fn default() -> Self {
        PresentTrace {
            pending: None,
            origin: None,
            last: None,
            nominal_ns: 0,
            drift_ms: 0.0,
            drift_max_ms: 0.0,
            window: Window::default(),
            total: Window::default(),
            window_start: None,
        }
    }
}

impl PresentTrace {
    /// A frame was shown (uploaded) while playing. `due`: when the schedule wanted it;
    /// `effective`: its display time (delay policy and speed applied); `dropped`: frames
    /// skipped to reach it.
    pub fn shown(&mut self, index: u32, due: Instant, effective: Duration, dropped: u64) {
        // A frame that never reached a present (superseded in the same turn) counts as dropped.
        let superseded = u64::from(self.pending.is_some());
        let nominal_ns = self.nominal_ns;
        self.nominal_ns = self
            .nominal_ns
            .saturating_add(u64::try_from(effective.as_nanos()).unwrap_or(u64::MAX));
        self.pending = Some(Pending {
            index,
            due,
            nominal_ns,
            dropped: dropped + superseded,
        });
    }

    /// Playback was interrupted (pause, step, seek, speed change): the next frame starts a
    /// new run.
    pub fn reset_run(&mut self) {
        self.pending = None;
        self.origin = None;
        self.last = None;
        self.nominal_ns = 0;
    }

    /// The frame went to the compositor at `now`.
    pub fn presented(&mut self, now: Instant) {
        let Some(p) = self.pending.take() else { return };
        let start = *self.window_start.get_or_insert(now);
        let late = now.saturating_duration_since(p.due).as_secs_f32() * 1e3;
        for w in [&mut self.window, &mut self.total] {
            w.presents += 1;
            w.dropped += p.dropped;
            Window::push(&mut w.latency, late);
        }
        if p.dropped > 0 {
            // Drift restarts after a drop: the skipped frames' delays are not known here.
            self.origin = None;
        }
        if let Some((lp, ld)) = self.last
            && p.dropped == 0
        {
            let dp = now.saturating_duration_since(lp).as_secs_f64();
            let dd = p.due.saturating_duration_since(ld).as_secs_f64();
            let err = ((dp - dd).abs() * 1e3) as f32;
            Window::push(&mut self.window.jitter, err);
            Window::push(&mut self.total.jitter, err);
        }
        let (o_at, o_nom) = *self.origin.get_or_insert((p.due, p.nominal_ns));
        let real = now.saturating_duration_since(o_at).as_secs_f64() * 1e3;
        let nominal = p.nominal_ns.saturating_sub(o_nom) as f64 / 1e6;
        self.drift_ms = real - nominal;
        self.drift_max_ms = self.drift_max_ms.max(self.drift_ms.abs());
        self.last = Some((now, p.due));
        trace!(target: RENDER, "anim present idx={} late={late:.3}ms drift={:.3}ms dropped={}", p.index, self.drift_ms, p.dropped);
        if now.saturating_duration_since(start) >= REPORT_EVERY {
            debug!(target: RENDER, "anim pacing (last {:.0} s): {}; drift {:+.2} ms (max |{:.2}|)",
                now.saturating_duration_since(start).as_secs_f64(), self.window.summary(), self.drift_ms, self.drift_max_ms);
            self.window = Window::default();
            self.window_start = Some(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: u64) -> Duration {
        Duration::from_millis(v)
    }

    #[test]
    fn steady_presents_have_no_jitter_or_drift() {
        let mut t = PresentTrace::default();
        let t0 = Instant::now();
        for i in 0..50u32 {
            let due = t0 + ms(40) * i;
            t.shown(i, due, ms(40), 0);
            t.presented(due + ms(3));
        }
        assert_eq!(t.total.presents, 50);
        assert!(Window::pct(&t.total.jitter, 100.0) < 0.01);
        // Drift stays at the constant latency.
        assert!((t.drift_max_ms - 3.0).abs() < 0.01, "{}", t.drift_max_ms);
        assert!((Window::pct(&t.total.latency, 50.0) - 3.0).abs() < 0.01);
    }

    #[test]
    fn slow_presents_show_up_as_drift() {
        let mut t = PresentTrace::default();
        let t0 = Instant::now();
        for i in 0..10u32 {
            // Each frame shown 1 ms later than the one before relative to the schedule.
            t.shown(i, t0 + ms(40) * i, ms(40), 0);
            t.presented(t0 + ms(41) * i);
        }
        assert!((t.drift_ms - 9.0).abs() < 0.01, "{}", t.drift_ms);
        assert!((Window::pct(&t.total.jitter, 50.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn superseded_frames_count_as_dropped() {
        let mut t = PresentTrace::default();
        let t0 = Instant::now();
        t.shown(0, t0, ms(10), 0);
        t.shown(1, t0 + ms(10), ms(10), 2);
        t.presented(t0 + ms(12));
        assert_eq!((t.total.presents, t.total.dropped), (1, 3));
    }
}
