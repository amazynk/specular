//! The live tap: the samples being heard, kept for the sound page's live displays.
//!
//! The device callback ([`crate::audio`]) writes each buffer it hands out into a ring here,
//! with the media time of its first frame; the UI reads the frames heard at a media time
//! (the playback clock's). One writer, any number of readers, no locks: the samples are
//! atomics, and a reader checks after copying that the writer did not come round over what
//! it copied (a torn read is retried, then given up).
//!
//! - The ring holds [`CAPACITY`] samples (about 1.3 s of 48 kHz stereo; less with more
//!   channels or a higher rate): enough for the spectrum's window and the RMS span.
//! - Paused, stopped or seeking, the device gets silence and the tap goes idle: readers get
//!   nothing until sound is heard again, and the displays fall back to the whole-file
//!   analysis.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering, fence};

/// Samples held (a power of two).
pub const CAPACITY: usize = 1 << 17;
const MASK: u64 = CAPACITY as u64 - 1;

/// How far past the last written frame a read may ask (seconds): the clock runs on between
/// device buffers.
const AHEAD_S: f64 = 0.1;

/// The samples being heard.
pub struct Tap {
    samples: Box<[AtomicU32]>,
    /// Samples written so far (all time).
    written: AtomicU64,
    /// Samples the writer is writing up to: claimed before, published after.
    claim: AtomicU64,
    /// The first sample of the current run (nothing before it belongs to what plays now).
    from: AtomicU64,
    channels: AtomicU32,
    rate: AtomicU32,
    /// The last buffer's first sample and its media time and speed, under a sequence
    /// number (odd while it changes).
    seq: AtomicU64,
    stamp_at: AtomicU64,
    stamp_ns: AtomicU64,
    stamp_speed: AtomicU64,
    live: AtomicBool,
}

impl Default for Tap {
    fn default() -> Self {
        Tap::new()
    }
}

/// What a read gave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heard {
    pub channels: usize,
    pub rate: u32,
    /// Frames copied (interleaved, oldest first).
    pub frames: usize,
}

impl Tap {
    pub fn new() -> Tap {
        Tap {
            samples: (0..CAPACITY).map(|_| AtomicU32::new(0)).collect(),
            written: AtomicU64::new(0),
            claim: AtomicU64::new(0),
            from: AtomicU64::new(0),
            channels: AtomicU32::new(0),
            rate: AtomicU32::new(0),
            seq: AtomicU64::new(0),
            stamp_at: AtomicU64::new(0),
            stamp_ns: AtomicU64::new(0),
            stamp_speed: AtomicU64::new(1f64.to_bits()),
            live: AtomicBool::new(false),
        }
    }

    /// The writer (one thread): `buf` (interleaved, `ch` channels at `rate`) is being heard,
    /// its first frame at media time `media_ns`, playing at `speed`.
    pub fn write(&self, buf: &[f32], ch: usize, rate: u32, media_ns: i64, speed: f64) {
        let ch = ch.max(1);
        // A buffer longer than half the ring: its end.
        let n = (buf.len() / ch * ch).min(CAPACITY / 2 / ch * ch);
        let skipped = (buf.len() / ch * ch - n) / ch;
        let buf = &buf[buf.len() / ch * ch - n..buf.len() / ch * ch];
        let media_ns = media_ns + (skipped as f64 * speed * 1e9 / f64::from(rate.max(1))) as i64;
        let mut pos = self.written.load(Ordering::Relaxed);
        let fmt_changed = self.channels.load(Ordering::Relaxed) != ch as u32
            || self.rate.load(Ordering::Relaxed) != rate;
        if fmt_changed || !self.live.load(Ordering::Relaxed) {
            // A new run starts on a frame boundary of its own channels.
            pos = pos.div_ceil(ch as u64) * ch as u64;
            self.written.store(pos, Ordering::Relaxed);
            self.claim.store(pos, Ordering::Relaxed);
            self.from.store(pos, Ordering::Relaxed);
            self.channels.store(ch as u32, Ordering::Relaxed);
            self.rate.store(rate, Ordering::Relaxed);
        }
        let end = pos + n as u64;
        self.claim.store(end, Ordering::Relaxed);
        fence(Ordering::Release);
        for (i, &v) in buf.iter().enumerate() {
            self.samples[((pos + i as u64) & MASK) as usize].store(v.to_bits(), Ordering::Relaxed);
        }
        // The stamp: the first frame of this buffer.
        let s = self.seq.load(Ordering::Relaxed);
        self.seq.store(s + 1, Ordering::Relaxed);
        fence(Ordering::Release);
        self.stamp_at.store(pos, Ordering::Relaxed);
        self.stamp_ns
            .store(media_ns.max(0) as u64, Ordering::Relaxed);
        self.stamp_speed.store(speed.to_bits(), Ordering::Relaxed);
        self.seq.store(s + 2, Ordering::Release);
        self.written.store(end, Ordering::Release);
        self.live.store(true, Ordering::Release);
    }

    /// Nothing is heard (paused, stopped, seeking, starved): readers get nothing until the
    /// next [`Tap::write`], which starts a new run.
    pub fn idle(&self) {
        self.live.store(false, Ordering::Release);
    }

    /// Whether sound is being heard.
    pub fn is_live(&self) -> bool {
        self.live.load(Ordering::Acquire)
    }

    /// The channels and rate of what is heard, while it is.
    pub fn format(&self) -> Option<(usize, u32)> {
        if !self.is_live() {
            return None;
        }
        let ch = self.channels.load(Ordering::Relaxed) as usize;
        let rate = self.rate.load(Ordering::Relaxed);
        (ch > 0 && rate > 0).then_some((ch, rate))
    }

    /// The stamp, read whole.
    fn stamp(&self) -> Option<(u64, i64, f64)> {
        for _ in 0..8 {
            let s0 = self.seq.load(Ordering::Acquire);
            if s0 & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let at = self.stamp_at.load(Ordering::Relaxed);
            let ns = self.stamp_ns.load(Ordering::Relaxed) as i64;
            let speed = f64::from_bits(self.stamp_speed.load(Ordering::Relaxed));
            fence(Ordering::Acquire);
            if self.seq.load(Ordering::Relaxed) == s0 {
                return Some((at, ns, speed));
            }
        }
        None
    }

    /// Copies into `out` the `frames` frames heard up to media time `t` (seconds), oldest
    /// first. `None` when nothing is heard, when `t` is not in what the ring holds (a seek
    /// not heard yet, or too long ago), or when the writer kept overwriting it.
    pub fn read_at(&self, t: f64, frames: usize, out: &mut Vec<f32>) -> Option<Heard> {
        for _ in 0..4 {
            match self.try_read(t, frames, out) {
                Read::Done(h) => return Some(h),
                Read::None => return None,
                Read::Torn => {}
            }
        }
        None
    }

    fn try_read(&self, t: f64, frames: usize, out: &mut Vec<f32>) -> Read {
        out.clear();
        if !self.is_live() || frames == 0 {
            return Read::None;
        }
        let written = self.written.load(Ordering::Acquire);
        let ch = u64::from(self.channels.load(Ordering::Relaxed).max(1));
        let rate = self.rate.load(Ordering::Relaxed);
        let from = self.from.load(Ordering::Relaxed);
        let Some((at, ns, speed)) = self.stamp() else {
            return Read::Torn;
        };
        if rate == 0 || at < from || at > written {
            return Read::Torn;
        }
        // The frame heard at `t`, from the stamp.
        let dt = (t - ns as f64 / 1e9) / speed.max(1e-6);
        let end_f = at as f64 / ch as f64 + dt * f64::from(rate);
        let last = (written / ch) as f64;
        if end_f > last + AHEAD_S * f64::from(rate) {
            return Read::None;
        }
        // (A hair over: a time on a frame boundary reads up to that frame.)
        let end = ((end_f + 1e-4).min(last).floor().max(0.0) as u64) * ch;
        let Some(start) = end.checked_sub(frames as u64 * ch) else {
            return Read::None;
        };
        // Not yet heard enough of this run, or gone round.
        if start < from || start + (CAPACITY as u64) < written {
            return Read::None;
        }
        out.reserve((end - start) as usize);
        for i in start..end {
            out.push(f32::from_bits(
                self.samples[(i & MASK) as usize].load(Ordering::Relaxed),
            ));
        }
        fence(Ordering::Acquire);
        // The writer may have come round over the start while copying.
        let claim = self.claim.load(Ordering::Relaxed);
        if start + (CAPACITY as u64) < claim {
            return Read::Torn;
        }
        if !self.is_live() || self.from.load(Ordering::Relaxed) != from {
            return Read::None;
        }
        Read::Done(Heard {
            channels: ch as usize,
            rate,
            frames: ((end - start) / ch) as usize,
        })
    }
}

enum Read {
    Done(Heard),
    None,
    Torn,
}

#[cfg(test)]
#[path = "live_tap_tests.rs"]
mod tests;
