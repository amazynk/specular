//! The key recorder of Settings → Keys (DESIGN §11): press the keys to bind. A sequence ends
//! after [`KeySeq::MAX`] keys or when no key follows within the timeout. Esc held cancels, so
//! that a short Esc press can itself be recorded.

use crate::{KeyCombo, KeySeq};

/// What a recorder input did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordEvent {
    /// Nothing changed.
    None,
    /// A key was added, or Esc is being held: show the keys so far.
    Updated,
    /// The keys to bind.
    Done(KeySeq),
    /// Esc was held: stop without binding.
    Cancelled,
}

/// Records a key, chord or sequence. Times are in seconds on any steady clock.
#[derive(Clone, Debug, PartialEq)]
pub struct Recorder {
    keys: Vec<KeyCombo>,
    /// When the last key was added.
    last: f64,
    /// When a plain Esc went down, while it is held.
    esc_down: Option<f64>,
    timeout: f64,
}

impl Recorder {
    /// How long Esc must be held to cancel.
    pub const ESC_HOLD: f64 = 0.8;

    /// Start recording; `timeout` is how long to wait for the next key of a sequence.
    pub fn new(timeout: f64, now: f64) -> Recorder {
        Recorder {
            keys: Vec::new(),
            last: now,
            esc_down: None,
            timeout: timeout.max(0.1),
        }
    }

    /// The keys pressed so far.
    pub fn keys(&self) -> &[KeyCombo] {
        &self.keys
    }

    /// Whether Esc is down (released soon, it is recorded; held, it cancels).
    pub fn holding_escape(&self) -> bool {
        self.esc_down.is_some()
    }

    fn add(&mut self, c: KeyCombo, now: f64) -> RecordEvent {
        self.keys.push(c);
        self.last = now;
        if self.keys.len() >= KeySeq::MAX {
            self.finish()
        } else {
            RecordEvent::Updated
        }
    }

    fn finish(&mut self) -> RecordEvent {
        match KeySeq::from_keys(&self.keys) {
            Some(s) => RecordEvent::Done(s),
            None => RecordEvent::None,
        }
    }

    /// A key went down (`repeat` for auto-repeat while held).
    pub fn press(&mut self, c: KeyCombo, repeat: bool, now: f64) -> RecordEvent {
        if c.is_escape() {
            return match self.esc_down {
                Some(t) if now - t >= Self::ESC_HOLD => RecordEvent::Cancelled,
                Some(_) => RecordEvent::None,
                None if repeat => RecordEvent::None,
                None => {
                    self.esc_down = Some(now);
                    RecordEvent::Updated
                }
            };
        }
        if repeat {
            return RecordEvent::None;
        }
        // Another key while Esc is still down: Esc was meant as a key.
        if let Some(_t) = self.esc_down.take() {
            let esc = KeyCombo::new(
                crate::Key::Named(crate::NamedKey::Escape),
                crate::Mods::NONE,
            );
            if let RecordEvent::Done(s) = self.add(esc, now) {
                return RecordEvent::Done(s);
            }
        }
        self.add(c, now)
    }

    /// A key came up.
    pub fn release(&mut self, c: KeyCombo, now: f64) -> RecordEvent {
        if !c.is_escape() {
            return RecordEvent::None;
        }
        match self.esc_down.take() {
            Some(t) if now - t >= Self::ESC_HOLD => RecordEvent::Cancelled,
            Some(_) => self.add(c, now),
            None => RecordEvent::None,
        }
    }

    /// Time passed: Esc held long enough cancels, a sequence with no new key ends.
    pub fn tick(&mut self, now: f64) -> RecordEvent {
        if let Some(t) = self.esc_down {
            return if now - t >= Self::ESC_HOLD - 1e-9 {
                RecordEvent::Cancelled
            } else {
                RecordEvent::None
            };
        }
        // A hair of slack so that a tick scheduled at `deadline()` always ends it.
        if !self.keys.is_empty() && now - self.last >= self.timeout - 1e-9 {
            return self.finish();
        }
        RecordEvent::None
    }

    /// When [`Recorder::tick`] next has something to do.
    pub fn deadline(&self) -> Option<f64> {
        match self.esc_down {
            Some(t) => Some(t + Self::ESC_HOLD),
            None if !self.keys.is_empty() => Some(self.last + self.timeout),
            None => None,
        }
    }

    /// End now with the keys so far (the Done button).
    pub fn done(&mut self) -> RecordEvent {
        self.esc_down = None;
        self.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(s: &str) -> KeyCombo {
        KeyCombo::parse(s).unwrap()
    }
    fn seq(s: &str) -> KeySeq {
        KeySeq::parse(s).unwrap()
    }

    #[test]
    fn single_key_ends_after_the_timeout() {
        let mut r = Recorder::new(1.0, 0.0);
        assert_eq!(r.tick(5.0), RecordEvent::None, "nothing pressed yet");
        assert_eq!(r.deadline(), None);
        assert_eq!(r.press(k("Ctrl+k"), false, 1.0), RecordEvent::Updated);
        assert_eq!(r.press(k("Ctrl+k"), true, 1.2), RecordEvent::None);
        assert_eq!(r.release(k("Ctrl+k"), 1.3), RecordEvent::None);
        assert_eq!(r.deadline(), Some(2.0));
        assert_eq!(r.tick(1.9), RecordEvent::None);
        assert_eq!(r.tick(2.0), RecordEvent::Done(seq("Ctrl+k")));
    }

    #[test]
    fn sequences() {
        let mut r = Recorder::new(1.0, 0.0);
        r.press(k("g"), false, 0.1);
        assert_eq!(r.tick(0.9), RecordEvent::None);
        r.press(k("g"), false, 0.9);
        assert_eq!(r.keys(), &[k("g"), k("g")]);
        assert_eq!(r.tick(1.89), RecordEvent::None);
        assert_eq!(r.tick(1.9), RecordEvent::Done(seq("g g")));
        // Three keys end at once.
        let mut r = Recorder::new(1.0, 0.0);
        r.press(k("a"), false, 0.0);
        r.press(k("b"), false, 0.1);
        assert_eq!(r.press(k("c"), false, 0.2), RecordEvent::Done(seq("a b c")));
        // The Done button.
        let mut r = Recorder::new(1.0, 0.0);
        assert_eq!(r.done(), RecordEvent::None);
        r.press(k("r"), false, 0.0);
        assert_eq!(r.done(), RecordEvent::Done(seq("r")));
    }

    #[test]
    fn escape_tapped_is_a_key_and_held_cancels() {
        // Tapped: recorded.
        let mut r = Recorder::new(1.0, 0.0);
        assert_eq!(r.press(k("Escape"), false, 0.0), RecordEvent::Updated);
        assert!(r.holding_escape());
        assert_eq!(r.deadline(), Some(Recorder::ESC_HOLD));
        assert_eq!(r.release(k("Escape"), 0.2), RecordEvent::Updated);
        assert_eq!(r.tick(1.2), RecordEvent::Done(seq("Escape")));
        // Held: auto-repeat past the hold time cancels.
        let mut r = Recorder::new(1.0, 0.0);
        r.press(k("g"), false, 0.0);
        r.press(k("Escape"), false, 0.1);
        assert_eq!(r.press(k("Escape"), true, 0.5), RecordEvent::None);
        assert_eq!(r.press(k("Escape"), true, 0.95), RecordEvent::Cancelled);
        // Held with no auto-repeat: the tick cancels, and so does a late release.
        let mut r = Recorder::new(1.0, 0.0);
        r.press(k("Escape"), false, 0.0);
        assert_eq!(r.tick(0.5), RecordEvent::None);
        assert_eq!(r.tick(0.8), RecordEvent::Cancelled);
        let mut r = Recorder::new(1.0, 0.0);
        r.press(k("Escape"), false, 0.0);
        assert_eq!(r.release(k("Escape"), 1.0), RecordEvent::Cancelled);
        // Esc with a modifier is an ordinary key.
        let mut r = Recorder::new(1.0, 0.0);
        assert_eq!(r.press(k("Shift+Escape"), false, 0.0), RecordEvent::Updated);
        assert!(!r.holding_escape());
        // Esc still down when the next key comes: both are recorded.
        let mut r = Recorder::new(1.0, 0.0);
        r.press(k("Escape"), false, 0.0);
        r.press(k("x"), false, 0.1);
        assert_eq!(r.keys(), &[k("Escape"), k("x")]);
        assert!(!r.holding_escape());
    }
}
