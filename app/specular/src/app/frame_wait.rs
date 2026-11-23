//! Frames the window had nothing to draw into.
//!
//! A window that is out of sight (hidden, minimised, on another desktop, wholly covered) gives
//! no frame: `WindowSurface::acquire` answers `Frame::Skip` at once, every time, until the
//! window is seen again. The frame is still owed, but asking for it again straight away is a
//! loop that takes a whole core for as long as the window stays out of sight, and one that
//! never lets the app count as idle. So the owed frame waits. Each retry comes after an
//! eighth of the time waited so far, between [`SHORTEST_RETRY`] and [`LONGEST_RETRY`]: a
//! window that is only just coming on screen (every start: the first frame is asked for a few
//! tens of milliseconds before the system reports the window visible) is tried every few
//! milliseconds, so its first frame is not kept waiting, and one that stays out of sight
//! settles at four tries a second. The window saying it is visible again ends the wait at once.
//!
//! Nothing here stops a frame the system asks for (a `RedrawRequested` of its own): that one
//! is tried whenever it comes.
//!
//! A window that has never shown a frame is a start-up that went wrong as far as the user can
//! tell ("it runs but nothing shows"), so that case is said in the log at the default level,
//! with the reason, as is a surface that fails for any other reason than being out of sight.
//!
//! Benchmarks and screenshots (`App::headless`) are left as they were: they draw on their own
//! clock and their timings must not wait on a retry.

use std::time::{Duration, Instant};

use specular_gfx::Skipped;
use specular_log::{RENDER, debug, warn};

use super::App;

/// The shortest wait between retries (the first ones, while a window comes on screen).
const SHORTEST_RETRY: Duration = Duration::from_millis(1);
/// The longest wait between retries, for the visibility change no event reported.
const LONGEST_RETRY: Duration = Duration::from_millis(250);
/// A retry waits this fraction of the time waited so far: the frame comes at most an eighth
/// later than the window allowed it.
const RETRY_SHARE: u32 = 8;
/// How long without a frame before the log says so.
const SAY_AFTER: Duration = Duration::from_secs(3);

/// A run of frames that could not be drawn, from the first one skipped to the next one
/// presented.
#[derive(Debug)]
pub(super) struct FrameWait {
    since: Instant,
    retry_at: Instant,
    why: Skipped,
    /// The log has the line about this wait.
    said: bool,
}

/// How long to wait for the next try after `waited` without a frame.
fn retry_after(waited: Duration) -> Duration {
    (waited / RETRY_SHARE).clamp(SHORTEST_RETRY, LONGEST_RETRY)
}

/// Whether a wait this long, for this reason, belongs in the log at the default level: always
/// before the first frame, and later only when the window is not merely out of sight.
fn worth_saying(waited: Duration, why: Skipped, frames: u64) -> bool {
    waited >= SAY_AFTER && (frames == 0 || why != Skipped::Occluded)
}

impl App {
    /// `frame` got no frame to draw into: the frame stays owed and is tried again later.
    pub(super) fn frame_skipped(&mut self, why: Skipped) {
        self.redraw = true;
        let now = Instant::now();
        let frames = self.frames;
        let w = self.frame_wait.get_or_insert_with(|| {
            debug!(target: RENDER, "no frame to draw into ({why}); trying again until there is one");
            FrameWait {
                since: now,
                retry_at: now,
                why,
                said: false,
            }
        });
        w.why = why;
        w.retry_at = now + retry_after(now - w.since);
        if !w.said && worth_saying(now - w.since, why, frames) {
            w.said = true;
            if frames == 0 {
                warn!(target: RENDER,
                    "nothing drawn yet, {:.0} s after the window opened: {why}; still trying",
                    (now - w.since).as_secs_f64());
            } else {
                warn!(target: RENDER,
                    "no frame drawn for {:.0} s: {why}; still trying",
                    (now - w.since).as_secs_f64());
            }
        }
    }

    /// A frame was presented: any wait is over.
    pub(super) fn frame_drawn(&mut self) {
        let Some(w) = self.frame_wait.take() else {
            return;
        };
        let waited = w.since.elapsed().as_secs_f64();
        if w.said {
            // At the level of the line it answers, so a log that has one has both.
            warn!(target: RENDER, "drawing again after {waited:.0} s without a frame ({})", w.why);
        } else {
            debug!(target: RENDER, "drawing again after {:.0} ms without a frame ({})", waited * 1e3, w.why);
        }
    }

    /// The window is visible again (unhidden, restored, uncovered): the owed frame is tried
    /// now, not at the next retry.
    pub(super) fn frame_wait_seen(&mut self) {
        if let Some(w) = self.frame_wait.as_mut() {
            w.retry_at = Instant::now();
        }
    }

    /// While an owed frame waits for its retry: when that is. `None` when nothing waits (or
    /// the retry is due), and in benchmarks and screenshots.
    pub(super) fn frame_held_until(&self) -> Option<Instant> {
        if self.headless() {
            return None;
        }
        self.frame_wait
            .as_ref()
            .map(|w| w.retry_at)
            .filter(|&t| Instant::now() < t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Retries start a millisecond apart and settle at four a second: a window coming on
    /// screen is not kept waiting, and one that stays away is never a loop.
    #[test]
    fn retries_back_off() {
        let ms = Duration::from_millis;
        assert_eq!(retry_after(Duration::ZERO), SHORTEST_RETRY);
        assert_eq!(retry_after(ms(40)), ms(5));
        assert_eq!(retry_after(ms(800)), ms(100));
        assert_eq!(retry_after(ms(2000)), LONGEST_RETRY);
        assert_eq!(retry_after(Duration::from_secs(3600)), LONGEST_RETRY);
        // The tries as the app makes them: each one after the wait the last one set.
        let (mut t, mut early, mut first_second, mut tenth_second) = (Duration::ZERO, 0, 0, 0);
        while t < Duration::from_secs(10) {
            let wait = retry_after(t);
            // Never more than an eighth later than the window allowed (or the shortest wait).
            assert!(wait <= (t / RETRY_SHARE).max(SHORTEST_RETRY));
            t += wait;
            early += u32::from(t <= ms(50));
            first_second += u32::from(t <= ms(1000));
            tenth_second += u32::from(t > ms(9000) && t <= ms(10_000));
        }
        // Where a start's first frame falls: tries a few milliseconds apart.
        assert!(early >= 10, "{early} tries in the first 50 ms");
        // Dozens in the first second, not the tens of thousands of a loop.
        assert!(
            (20..=80).contains(&first_second),
            "{first_second} tries in the first second"
        );
        assert_eq!(tenth_second, 4);
    }

    /// A window that never showed a frame is said in the log; one minimised later is not
    /// (that is everyday use); a surface that fails for another reason always is.
    #[test]
    fn what_the_log_says() {
        let long = SAY_AFTER + Duration::from_millis(1);
        let short = SAY_AFTER - Duration::from_millis(1);
        assert!(worth_saying(long, Skipped::Occluded, 0));
        assert!(!worth_saying(short, Skipped::Occluded, 0));
        assert!(!worth_saying(long, Skipped::Occluded, 12));
        assert!(worth_saying(long, Skipped::Timeout, 12));
        assert!(worth_saying(long, Skipped::Unusable, 12));
        assert!(!worth_saying(short, Skipped::Unusable, 12));
    }
}
