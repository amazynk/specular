//! The value under the cursor, for screen readers: the readout the app already shows (pixel
//! value, HU, temperature, physical units, map or sky position) is read out through the
//! chrome's polite status node ([`crate::Chrome::announce_status`]).
//!
//! A moving cursor would flood the screen reader, so [`CursorAnnouncer`] waits until the
//! readout has rested for [`CURSOR_REST_S`] and never repeats what it last read. The announce
//! key reads the readout at once, rested or not.

/// How long the readout must stay the same before it is read out (seconds).
pub const CURSOR_REST_S: f64 = 0.6;

/// Debounces the readout under the cursor.
#[derive(Clone, Debug, Default)]
pub struct CursorAnnouncer {
    /// The readout waiting to rest, and when it last changed.
    pending: Option<(String, f64)>,
    /// What was read out last (not read again until it changes).
    last: Option<String>,
}

impl CursorAnnouncer {
    /// The readout at `now` (`None`: nothing under the cursor). A change restarts the wait.
    pub fn update(&mut self, text: Option<String>, now: f64) {
        let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
            // Off the image: nothing to read, and coming back reads the readout again.
            self.pending = None;
            self.last = None;
            return;
        };
        match &self.pending {
            Some((p, _)) if *p == text => {}
            _ if self.pending.is_none() && self.last.as_deref() == Some(text.as_str()) => {}
            _ => self.pending = Some((text, now)),
        }
    }

    /// The readout to read out now, once it has rested; `None` while it moves or when it
    /// was read already.
    pub fn due(&mut self, now: f64) -> Option<String> {
        let (_, since) = self.pending.as_ref()?;
        // (A microsecond's slack for clocks summed in steps.)
        if now - since + 1e-6 < CURSOR_REST_S {
            return None;
        }
        let (text, _) = self.pending.take()?;
        if self.last.as_deref() == Some(text.as_str()) {
            return None;
        }
        self.last = Some(text.clone());
        Some(text)
    }

    /// When [`CursorAnnouncer::due`] next has something (seconds, same clock as `now`).
    pub fn wake(&self) -> Option<f64> {
        self.pending.as_ref().map(|(_, t)| t + CURSOR_REST_S)
    }

    /// The announce key read `text` out: it counts as read, and what was waiting is dropped.
    pub fn read_now(&mut self, text: &str) {
        self.pending = None;
        self.last = Some(text.to_owned());
    }

    /// Forgets everything (the option was turned off, or the image changed).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The readout lines as one announcement (the lines in the order shown).
pub fn spoken(lines: &[String]) -> Option<String> {
    let parts: Vec<&str> = lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// The pixel inspector's readout as one announcement: position and sRGB value, then the
/// source value when it is known (the rows the panel shows).
pub fn pixel_readout(p: &crate::PixelInfo) -> String {
    let mut s = crate::inspect::pixel_summary(p);
    if let Some((label, value)) = crate::inspect::source_row(&p.source)
        && value != "…"
    {
        s = format!("{s}; {label} {value}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t: &str) -> Option<String> {
        Some(t.to_owned())
    }

    #[test]
    fn waits_for_the_readout_to_rest() {
        let mut a = CursorAnnouncer::default();
        a.update(s("12, 4: 200"), 0.0);
        assert_eq!(a.due(0.1), None);
        assert_eq!(a.wake(), Some(CURSOR_REST_S));
        // Each move restarts the wait: a moving cursor says nothing.
        for i in 1..20 {
            let t = i as f64 * 0.1;
            a.update(Some(format!("{i}, 4: 200")), t);
            assert_eq!(a.due(t), None, "spoke while moving at {t}");
        }
        let rest = 1.9 + CURSOR_REST_S;
        assert_eq!(a.due(rest - 0.01), None);
        assert_eq!(a.due(rest).as_deref(), Some("19, 4: 200"));
        // Read once.
        assert_eq!(a.due(rest + 5.0), None);
        assert_eq!(a.wake(), None);
    }

    #[test]
    fn the_same_readout_is_not_read_twice() {
        let mut a = CursorAnnouncer::default();
        a.update(s("A"), 0.0);
        assert_eq!(a.due(1.0).as_deref(), Some("A"));
        // Frames keep reporting the same readout: silence.
        for i in 0..10 {
            a.update(s("A"), 1.0 + f64::from(i));
            assert_eq!(a.due(1.5 + f64::from(i)), None);
        }
        // A brief move away and back to the same value before resting: still nothing new.
        a.update(s("B"), 20.0);
        a.update(s("A"), 20.1);
        assert_eq!(a.due(21.0), None);
        // Leaving the image and coming back reads it again.
        a.update(None, 22.0);
        a.update(s("A"), 23.0);
        assert_eq!(a.due(24.0).as_deref(), Some("A"));
    }

    #[test]
    fn the_key_reads_at_once_and_counts_as_read() {
        let mut a = CursorAnnouncer::default();
        a.update(s("A"), 0.0);
        a.read_now("A");
        assert_eq!(a.due(5.0), None);
        a.update(s("A"), 6.0);
        assert_eq!(a.due(7.0), None);
        a.update(s("B"), 8.0);
        assert_eq!(a.due(9.0).as_deref(), Some("B"));
        a.reset();
        a.update(s("B"), 10.0);
        assert_eq!(a.due(11.0).as_deref(), Some("B"));
    }

    #[test]
    fn blank_readouts_say_nothing() {
        let mut a = CursorAnnouncer::default();
        a.update(s("  "), 0.0);
        assert_eq!(a.due(5.0), None);
        assert_eq!(spoken(&[]), None);
        assert_eq!(spoken(&[" ".into()]), None);
        assert_eq!(
            spoken(&["-120 HU".into(), String::new(), "W 400 L 40".into()]).as_deref(),
            Some("-120 HU; W 400 L 40")
        );
    }
}
