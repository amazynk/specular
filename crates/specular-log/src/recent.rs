//! The last lines of the log, kept in memory for crash reports and the diagnostics bundle
//! (both stay on the computer unless the user shares them).

use std::collections::VecDeque;
use std::time::Duration;

use parking_lot::Mutex;

/// Lines kept.
pub const KEEP: usize = 1000;

struct Ring {
    lines: VecDeque<String>,
    /// A line written in pieces, until its newline.
    partial: String,
    keep: usize,
}

impl Ring {
    const fn new(keep: usize) -> Ring {
        Ring {
            lines: VecDeque::new(),
            partial: String::new(),
            keep,
        }
    }

    fn push(&mut self, buf: &[u8]) {
        let text = String::from_utf8_lossy(buf);
        for piece in text.split_inclusive('\n') {
            match piece.strip_suffix('\n') {
                Some(end) => {
                    let mut line = std::mem::take(&mut self.partial);
                    line.push_str(end.trim_end_matches('\r'));
                    if self.lines.len() == self.keep {
                        self.lines.pop_front();
                    }
                    self.lines.push_back(line);
                }
                None => self.partial.push_str(piece),
            }
        }
    }

    fn last(&self, n: usize) -> Vec<String> {
        let skip = self.lines.len().saturating_sub(n);
        self.lines.iter().skip(skip).cloned().collect()
    }
}

static RING: Mutex<Ring> = Mutex::new(Ring::new(KEEP));

/// Adds what the logger wrote (whole lines, or pieces of one).
pub(crate) fn push(buf: &[u8]) {
    RING.lock().push(buf);
}

/// The last `n` lines of the log, oldest first (at most [`KEEP`]). Waits briefly for a line
/// being written; empty if the log stays busy (a crash report must not hang on it).
pub fn recent_lines(n: usize) -> Vec<String> {
    RING.try_lock_for(Duration::from_millis(100))
        .map_or_else(Vec::new, |r| r.last(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_whole_lines_in_order() {
        let mut r = Ring::new(10);
        r.push(b"first line\nsecond ");
        r.push(b"half\r\n");
        assert_eq!(r.last(2), ["first line", "second half"]);
        for i in 0..15 {
            r.push(format!("line {i}\n").as_bytes());
        }
        let all = r.last(usize::MAX);
        assert_eq!(all.len(), 10);
        assert_eq!(all[0], "line 5");
        assert_eq!(all[9], "line 14");
        assert!(r.last(0).is_empty());
    }

    #[test]
    fn the_logger_feeds_it() {
        push(b"recent-test marker\n");
        assert!(recent_lines(KEEP).iter().any(|l| l == "recent-test marker"));
    }
}
