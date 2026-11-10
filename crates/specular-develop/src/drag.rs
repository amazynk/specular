//! Adaptive preview while dragging: picks how large a slider's drafts are from how long the
//! last ones took, so drags stay smooth on slow machines without making fast ones softer.
//!
//! - **Levels.** The view's size ([`Level::Full`]), half of it ([`Level::Half`], the
//!   half-scale draft path of [`crate::preview`]), a quarter ([`Level::Quarter`]), and,
//!   as a last resort, a quarter-size picture only when the slider pauses or is let go
//!   ([`Level::Pause`]).
//! - **Down** when the average of the last [`AVERAGE`] frames is over the frame budget
//!   (one display refresh, [`Pace::set_refresh`]; [`BUDGET_MS`] when unknown).
//! - **Up** only after [`UP_AFTER`] frames in a row under [`UP_BELOW`] of the budget, and
//!   twice as many after each step up that had to come back down in the same drag
//!   (hysteresis: no flicker between two levels).
//! - **Memory.** The level a drag ended at is kept per photo and edit shape ([`Key`]) and
//!   the next drag with the same key starts there.
//! - **Setting.** [`Mode::Sharp`] always drafts at the view's size, [`Mode::Fast`] always
//!   at half; only [`Mode::Automatic`] adapts.
//!
//! The release render, exports, thumbnails and the refined picture never go through this:
//! it only sizes drafts of a module in focus.

/// The frame budget when the display's refresh is unknown (about 60 Hz).
pub const BUDGET_MS: f64 = 16.0;
/// Bounds of the budget taken from the display's refresh: a 240 Hz display does not ask
/// for 4 ms frames, a 24 Hz one does not allow 42 ms.
pub const BUDGET_RANGE: (f64, f64) = (8.0, 33.0);
/// Frames averaged before stepping down (one slow frame among them is not enough).
pub const AVERAGE: usize = 4;
/// Fast frames in a row before stepping up.
pub const UP_AFTER: u32 = 8;
/// A frame is fast below this fraction of the budget: the next level up draws about four
/// times the pixels, so it has to fit about four of these.
pub const UP_BELOW: f64 = 0.35;
/// Frames after a step up within which a step down counts as a failed step up.
pub const PROBE: u32 = 6;
/// Photo-and-shape keys remembered.
pub const MEMORY: usize = 64;
/// How long the slider has to rest before [`Level::Pause`] draws a picture (ms).
pub const PAUSE_MS: u64 = 150;

/// What "Preview while dragging" is set to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Adapt to how long frames take.
    #[default]
    Automatic,
    /// Always the view's size.
    Sharp,
    /// Always half the view's size.
    Fast,
}

/// How a drag's drafts are drawn, sharpest first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Level {
    /// The view's size.
    #[default]
    Full,
    /// Half the view's size, scaled up.
    Half,
    /// A quarter of the view's size, scaled up.
    Quarter,
    /// A quarter-size picture, only when the slider rests ([`PAUSE_MS`]) or is let go.
    Pause,
}

impl Level {
    /// The divisor of the view's sides the draft is drawn at.
    pub fn divisor(self) -> u32 {
        match self {
            Level::Full => 1,
            Level::Half => 2,
            Level::Quarter | Level::Pause => 4,
        }
    }

    /// The level whose drafts are drawn at `divisor` (rounded to the nearest level).
    pub fn of_divisor(divisor: u32) -> Level {
        match divisor {
            0 | 1 => Level::Full,
            2 | 3 => Level::Half,
            _ => Level::Quarter,
        }
    }

    fn down(self) -> Level {
        match self {
            Level::Full => Level::Half,
            Level::Half => Level::Quarter,
            Level::Quarter | Level::Pause => Level::Pause,
        }
    }

    fn up(self) -> Level {
        match self {
            Level::Full | Level::Half => Level::Full,
            Level::Quarter => Level::Half,
            Level::Pause => Level::Quarter,
        }
    }
}

/// A photo and an edit shape (the caller's hash: the photo, the modules in use, the module
/// dragged and the view's size).
pub type Key = u64;

/// The controller: one per preview.
#[derive(Clone, Debug)]
pub struct Pace {
    pub mode: Mode,
    budget_ms: f64,
    level: Level,
    /// The last frames' times at `level` (ms), oldest first.
    recent: Vec<f64>,
    /// Fast frames in a row at `level`.
    fast: u32,
    /// The next frame starts a level (its source may be made first): not counted.
    warm: bool,
    /// Step ups that came back down in this drag.
    failed: u32,
    /// Frames since the last step up (`None`: none in this drag).
    since_up: Option<u32>,
    /// The drag's key while one goes on.
    key: Option<Key>,
    /// Remembered levels, the most recent last.
    memory: Vec<(Key, Level)>,
}

impl Default for Pace {
    fn default() -> Self {
        Pace::new(Mode::Automatic)
    }
}

impl Pace {
    pub fn new(mode: Mode) -> Pace {
        Pace {
            mode,
            budget_ms: BUDGET_MS,
            level: Level::Full,
            recent: Vec::with_capacity(AVERAGE),
            fast: 0,
            warm: true,
            failed: 0,
            since_up: None,
            key: None,
            memory: Vec::new(),
        }
    }

    /// The frame budget from the display's refresh rate (Hz), when known.
    pub fn set_refresh(&mut self, hz: Option<f64>) {
        self.budget_ms = match hz.filter(|h| h.is_finite() && *h > 0.0) {
            Some(h) => (1000.0 / h).clamp(BUDGET_RANGE.0, BUDGET_RANGE.1),
            None => BUDGET_MS,
        };
    }

    /// The frame budget (ms).
    pub fn budget_ms(&self) -> f64 {
        self.budget_ms
    }

    /// A drag starts: at the level remembered for `key`, else at `start` (what the preview
    /// would draw without this controller).
    pub fn begin(&mut self, key: Key, start: Level) {
        if self.key == Some(key) {
            return;
        }
        self.end();
        self.level = self
            .memory
            .iter()
            .rev()
            .find(|(k, _)| *k == key)
            .map_or(start, |(_, l)| *l);
        self.key = Some(key);
        self.restart();
        self.failed = 0;
        self.since_up = None;
    }

    /// The drag ended: its level is remembered for its key.
    pub fn end(&mut self) {
        let Some(key) = self.key.take() else { return };
        if self.mode != Mode::Automatic {
            return;
        }
        self.memory.retain(|(k, _)| *k != key);
        self.memory.push((key, self.level));
        if self.memory.len() > MEMORY {
            self.memory.remove(0);
        }
    }

    /// Whether a drag goes on.
    pub fn dragging(&self) -> bool {
        self.key.is_some()
    }

    /// The level to draw the next draft at.
    pub fn level(&self) -> Level {
        match self.mode {
            Mode::Automatic => self.level,
            Mode::Sharp => Level::Full,
            Mode::Fast => Level::Half,
        }
    }

    /// The level remembered for `key`, if any.
    pub fn remembered(&self, key: Key) -> Option<Level> {
        self.memory
            .iter()
            .rev()
            .find(|(k, _)| *k == key)
            .map(|(_, l)| *l)
    }

    /// A draft of the drag drawn at `divisor` took `ms` (from the render call to the GPU
    /// being done). Frames at another level than the current one (a module with its own
    /// drafts, a shown mask, a picture too small to reduce) are not counted.
    pub fn record(&mut self, divisor: u32, ms: f64) {
        if self.mode != Mode::Automatic || self.key.is_none() || !ms.is_finite() {
            return;
        }
        if divisor != self.level.divisor() {
            return;
        }
        if std::mem::take(&mut self.warm) {
            return;
        }
        if let Some(n) = self.since_up.as_mut() {
            *n += 1;
        }
        if self.recent.len() == AVERAGE {
            self.recent.remove(0);
        }
        self.recent.push(ms);
        self.fast = if ms < self.budget_ms * UP_BELOW {
            self.fast + 1
        } else {
            0
        };
        let avg = self.recent.iter().sum::<f64>() / self.recent.len() as f64;
        if self.recent.len() == AVERAGE && avg > self.budget_ms && self.level != Level::Pause {
            if self.since_up.is_some_and(|n| n <= PROBE) {
                self.failed += 1;
            }
            self.since_up = None;
            self.level = self.level.down();
            self.restart();
        } else if self.level != Level::Full && self.fast >= UP_AFTER << self.failed.min(4) {
            self.level = self.level.up();
            self.since_up = Some(0);
            self.restart();
        }
    }

    fn restart(&mut self) {
        self.recent.clear();
        self.fast = 0;
        self.warm = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drag(p: &mut Pace, ms: f64, n: usize) {
        for _ in 0..n {
            let d = p.level().divisor();
            p.record(d, ms);
        }
    }

    #[test]
    fn slow_frames_step_down_one_level_at_a_time() {
        let mut p = Pace::default();
        p.begin(1, Level::Full);
        // The first frame warms up; three slow ones step down.
        drag(&mut p, 30.0, 1 + AVERAGE - 1);
        assert_eq!(p.level(), Level::Full);
        drag(&mut p, 30.0, 1);
        assert_eq!(p.level(), Level::Half);
        drag(&mut p, 20.0, 1 + AVERAGE);
        assert_eq!(p.level(), Level::Quarter);
        drag(&mut p, 20.0, 1 + AVERAGE);
        assert_eq!(p.level(), Level::Pause);
        // Nothing below a pause.
        drag(&mut p, 50.0, 10);
        assert_eq!(p.level(), Level::Pause);
    }

    #[test]
    fn one_slow_frame_does_not_step_down() {
        let mut p = Pace::default();
        p.begin(1, Level::Full);
        for i in 0..40 {
            p.record(1, if i % 5 == 0 { 30.0 } else { 10.0 });
        }
        assert_eq!(p.level(), Level::Full);
    }

    #[test]
    fn fast_frames_step_up_after_a_run() {
        let mut p = Pace::default();
        p.begin(1, Level::Quarter);
        drag(&mut p, 2.0, UP_AFTER as usize);
        assert_eq!(
            p.level(),
            Level::Quarter,
            "the warm-up frame is not counted"
        );
        drag(&mut p, 2.0, 1);
        assert_eq!(p.level(), Level::Half);
        // Frames under budget but not fast keep the level.
        drag(&mut p, 10.0, 50);
        assert_eq!(p.level(), Level::Half);
        drag(&mut p, 3.0, 1 + UP_AFTER as usize);
        assert_eq!(p.level(), Level::Full);
    }

    #[test]
    fn a_failed_step_up_waits_longer_before_the_next() {
        let mut p = Pace::default();
        p.begin(1, Level::Half);
        // Half is fast, full is slow: up, then back down.
        drag(&mut p, 4.0, 1 + UP_AFTER as usize);
        assert_eq!(p.level(), Level::Full);
        drag(&mut p, 24.0, 1 + AVERAGE);
        assert_eq!(p.level(), Level::Half);
        // UP_AFTER fast frames are no longer enough.
        drag(&mut p, 4.0, 1 + UP_AFTER as usize);
        assert_eq!(p.level(), Level::Half);
        drag(&mut p, 4.0, UP_AFTER as usize);
        assert_eq!(p.level(), Level::Full);
        // Fewer flips than frames: over 200 frames the level changes rarely.
        let mut flips = 0;
        let mut last = p.level();
        for _ in 0..200 {
            let ms = if p.level() == Level::Full { 24.0 } else { 4.0 };
            p.record(p.level().divisor(), ms);
            if p.level() != last {
                flips += 1;
                last = p.level();
            }
        }
        assert!(flips <= 6, "{flips} flips");
    }

    #[test]
    fn the_level_is_remembered_per_key() {
        let mut p = Pace::default();
        p.begin(7, Level::Full);
        drag(&mut p, 40.0, 1 + AVERAGE);
        assert_eq!(p.level(), Level::Half);
        p.end();
        assert!(!p.dragging());
        assert_eq!(p.remembered(7), Some(Level::Half));
        // Another photo or shape starts where the preview would.
        p.begin(8, Level::Full);
        assert_eq!(p.level(), Level::Full);
        p.end();
        // The same one starts where the last drag ended.
        p.begin(7, Level::Full);
        assert_eq!(p.level(), Level::Half);
        p.end();
        // Bounded.
        for k in 100..100 + MEMORY as u64 {
            p.begin(k, Level::Full);
            p.end();
        }
        assert_eq!(p.remembered(7), None);
    }

    #[test]
    fn frames_at_another_level_are_not_counted() {
        let mut p = Pace::default();
        p.begin(1, Level::Full);
        // A module with its own drafts drew at full size while the level is half, etc.
        drag(&mut p, 1.0, 1);
        for _ in 0..20 {
            p.record(2, 100.0);
        }
        assert_eq!(p.level(), Level::Full);
        // Nothing counts between drags.
        p.end();
        p.record(1, 100.0);
        assert_eq!(p.remembered(1), Some(Level::Full));
    }

    #[test]
    fn the_setting_overrides_the_controller() {
        let mut p = Pace::new(Mode::Sharp);
        p.begin(1, Level::Half);
        assert_eq!(p.level(), Level::Full);
        drag(&mut p, 100.0, 20);
        assert_eq!(p.level(), Level::Full);
        p.end();
        assert_eq!(p.remembered(1), None, "fixed modes remember nothing");
        p.mode = Mode::Fast;
        p.begin(2, Level::Full);
        assert_eq!(p.level(), Level::Half);
        drag(&mut p, 1.0, 50);
        assert_eq!(p.level(), Level::Half);
        p.end();
        // Back to automatic: the controller's own level again.
        p.mode = Mode::Automatic;
        p.begin(3, Level::Full);
        assert_eq!(p.level(), Level::Full);
    }

    #[test]
    fn the_budget_follows_the_refresh_rate_within_bounds() {
        let mut p = Pace::default();
        assert_eq!(p.budget_ms(), BUDGET_MS);
        p.set_refresh(Some(60.0));
        assert!((p.budget_ms() - 16.667).abs() < 0.01);
        p.set_refresh(Some(240.0));
        assert_eq!(p.budget_ms(), BUDGET_RANGE.0);
        p.set_refresh(Some(10.0));
        assert_eq!(p.budget_ms(), BUDGET_RANGE.1);
        p.set_refresh(Some(0.0));
        assert_eq!(p.budget_ms(), BUDGET_MS);
        // At 120 Hz (8.3 ms) a 12 ms frame is slow.
        p.set_refresh(Some(120.0));
        p.begin(1, Level::Full);
        drag(&mut p, 12.0, 1 + AVERAGE);
        assert_eq!(p.level(), Level::Half);
    }

    #[test]
    fn divisors_and_levels() {
        for l in [Level::Full, Level::Half, Level::Quarter] {
            assert_eq!(Level::of_divisor(l.divisor()), l);
        }
        assert_eq!(Level::Pause.divisor(), 4);
    }
}
