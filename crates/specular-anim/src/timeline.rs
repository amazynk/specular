//! The playback schedule: which frame is due at which play time.
//!
//! Play positions are *sequence numbers* `s`: the s-th frame shown since playback started, over
//! all loops. A cycle is one loop (forwards, or forwards and back for ping-pong); `s` maps to
//! loop `s / cycle_len` and cycle position `s % cycle_len`. Frame `s` is due from play time
//! `T(s)` = loops × loop duration + the delays before it, so deadlines are absolute and never
//! drift however late individual ticks are.
//!
//! While the frame count is unknown (GIF's first pass) the timeline is *open*: it covers the
//! frames decoded so far, `s` is the frame index, and a time past the last known delay maps to
//! the not-yet-decoded frame `len`.

use crate::policy::DelayPolicy;
use std::time::Duration;

/// Used when every delay of a closed timeline is zero, so that time still advances.
const ZERO_LOOP_FALLBACK: Duration = Duration::from_millis(10);

pub(crate) struct Timeline {
    /// Stored delay of each frame index decoded so far.
    raw: Vec<Duration>,
    /// Frame count, when known (stated by the container or found at the end of the first pass).
    count: Option<u32>,
    policy: DelayPolicy,
    ping_pong: bool,
    /// Frame index at each cycle position.
    cycle: Vec<u32>,
    /// Start time (ns) of each cycle position; one more entry than `cycle` (the cycle length).
    start: Vec<u64>,
    closed: bool,
}

fn nanos(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

impl Timeline {
    pub fn new(count: Option<u32>, policy: DelayPolicy) -> Timeline {
        Timeline {
            raw: Vec::new(),
            count: count.filter(|&n| n > 0),
            policy,
            ping_pong: false,
            cycle: Vec::new(),
            start: vec![0],
            closed: false,
        }
    }

    /// Frames whose delay is known.
    pub fn known(&self) -> usize {
        self.raw.len()
    }

    pub fn count(&self) -> Option<u32> {
        self.count
    }

    /// Whether the whole cycle is known.
    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn ping_pong(&self) -> bool {
        self.ping_pong
    }

    /// Positions in one cycle (the number of known frames while open).
    pub fn cycle_len(&self) -> u64 {
        self.cycle.len() as u64
    }

    /// Duration of one cycle in ns (closed timelines only).
    pub fn cycle_ns(&self) -> u64 {
        *self.start.last().unwrap_or(&0)
    }

    /// Take the delays of frames `self.known()..` from `all` (the stored delays of every frame
    /// decoded so far, indexed by frame).
    pub fn sync(&mut self, all: &[Duration], count: Option<u32>) {
        let count = count.filter(|&n| n > 0);
        if count != self.count {
            self.count = count;
            if let Some(n) = count {
                self.raw.truncate(n as usize);
            }
            self.rebuild();
        }
        if all.len() > self.raw.len() {
            let limit = self.count.map_or(usize::MAX, |n| n as usize);
            let new = &all[self.raw.len()..all.len().min(limit)];
            self.raw.extend_from_slice(new);
            if self.closed || self.count.is_some_and(|n| self.raw.len() >= n as usize) {
                self.rebuild();
            } else {
                for i in self.cycle.len()..self.raw.len() {
                    let end = self.start[i] + nanos(self.policy.apply(self.raw[i]));
                    self.cycle.push(i as u32);
                    self.start.push(end);
                }
            }
        }
    }

    /// Change ping-pong or the delay policy. Positions keep their meaning only through
    /// [`Timeline::remap`].
    pub fn configure(&mut self, ping_pong: bool, policy: DelayPolicy) {
        if ping_pong != self.ping_pong || policy != self.policy {
            self.ping_pong = ping_pong;
            self.policy = policy;
            self.rebuild();
        }
    }

    fn rebuild(&mut self) {
        let n = self.raw.len();
        self.closed = self.count.is_some_and(|c| n >= c as usize) && n > 0;
        self.cycle.clear();
        self.cycle.extend(0..n as u32);
        if self.closed && self.ping_pong && n >= 3 {
            self.cycle.extend((1..n as u32 - 1).rev());
        }
        let mut eff: Vec<u64> = self
            .raw
            .iter()
            .map(|&d| nanos(self.policy.apply(d)))
            .collect();
        if self.closed && eff.iter().all(|&d| d == 0) {
            eff.fill(nanos(ZERO_LOOP_FALLBACK));
        }
        self.start.clear();
        self.start.push(0);
        let mut t = 0u64;
        for &i in &self.cycle {
            t = t.saturating_add(eff[i as usize]);
            self.start.push(t);
        }
    }

    /// Frame index shown at sequence position `s`.
    pub fn idx(&self, s: u64) -> u32 {
        if self.closed {
            self.cycle[(s % self.cycle_len()) as usize]
        } else {
            s.min(u64::from(u32::MAX)) as u32
        }
    }

    /// Play time (ns) at which sequence position `s` becomes due, when known.
    pub fn start_of(&self, s: u64) -> Option<u64> {
        if self.closed {
            let len = self.cycle_len();
            let lap = (s / len).checked_mul(self.cycle_ns())?;
            lap.checked_add(self.start[(s % len) as usize])
        } else {
            self.start.get(usize::try_from(s).ok()?).copied()
        }
    }

    /// Sequence position due at play time `t` (ns). On an open timeline a time after every
    /// known frame gives `known()`, the next frame to be decoded.
    pub fn seq_at(&self, t: u64) -> u64 {
        let total = self.cycle_ns();
        if self.closed {
            let lap = t / total;
            let rem = t % total;
            lap * self.cycle_len() + (self.start.partition_point(|&x| x <= rem) - 1) as u64
        } else if t >= total {
            self.cycle_len()
        } else {
            (self.start.partition_point(|&x| x <= t) - 1) as u64
        }
    }

    /// Cycle position of the first occurrence of frame `idx`.
    pub fn pos_of(&self, idx: u32) -> u64 {
        if self.closed {
            u64::from(idx.min(self.cycle_len() as u32 - 1))
        } else {
            u64::from(idx)
        }
    }

    /// Map a position of an earlier version of this timeline (same frames, different cycle) onto this timeline,
    /// keeping the loop number and the frame.
    pub fn remap(&self, old_idx: u32, old_lap: u64) -> u64 {
        if self.closed {
            old_lap * self.cycle_len() + self.pos_of(old_idx)
        } else {
            u64::from(old_idx)
        }
    }

    /// Loop number of `s`.
    pub fn lap(&self, s: u64) -> u64 {
        if self.closed { s / self.cycle_len() } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: u64) -> Duration {
        Duration::from_millis(v)
    }
    const MS: u64 = 1_000_000;

    #[test]
    fn open_then_closed() {
        let mut tl = Timeline::new(None, DelayPolicy::AS_STORED);
        tl.sync(&[ms(10), ms(20)], None);
        assert!(!tl.closed());
        assert_eq!(tl.seq_at(0), 0);
        assert_eq!(tl.seq_at(10 * MS), 1);
        assert_eq!(tl.seq_at(30 * MS), 2); // beyond what is known
        tl.sync(&[ms(10), ms(20), ms(30)], None);
        assert_eq!(tl.seq_at(30 * MS), 2);
        tl.sync(&[ms(10), ms(20), ms(30)], Some(3));
        assert!(tl.closed());
        assert_eq!(tl.cycle_ns(), 60 * MS);
        assert_eq!(tl.seq_at(60 * MS), 3);
        assert_eq!(tl.idx(3), 0);
        assert_eq!(tl.start_of(4), Some(70 * MS));
        assert_eq!(tl.seq_at(125 * MS), 6);
    }

    #[test]
    fn zero_delays_skipped() {
        let mut tl = Timeline::new(Some(3), DelayPolicy::AS_STORED);
        tl.sync(&[ms(0), ms(0), ms(50)], Some(3));
        // Frames 0 and 1 have empty slots; frame 2 is what shows.
        assert_eq!(tl.seq_at(0), 2);
        let mut all_zero = Timeline::new(Some(2), DelayPolicy::AS_STORED);
        all_zero.sync(&[ms(0), ms(0)], Some(2));
        assert_eq!(all_zero.cycle_ns(), 20 * MS);
    }

    #[test]
    fn ping_pong_cycle() {
        let mut tl = Timeline::new(Some(4), DelayPolicy::AS_STORED);
        tl.sync(&[ms(10); 4], Some(4));
        tl.configure(true, DelayPolicy::AS_STORED);
        assert_eq!(tl.cycle_len(), 6);
        let order: Vec<u32> = (0..8).map(|s| tl.idx(s)).collect();
        assert_eq!(order, [0, 1, 2, 3, 2, 1, 0, 1]);
        assert_eq!(tl.remap(3, 2), 2 * 6 + 3);
    }

    #[test]
    fn browser_policy_applies() {
        let mut tl = Timeline::new(None, DelayPolicy::BROWSER);
        tl.sync(&[ms(0), ms(40)], None);
        assert_eq!(tl.start_of(1), Some(100 * MS));
        assert_eq!(tl.start_of(2), Some(140 * MS));
        assert_eq!(tl.start_of(3), None);
    }
}
