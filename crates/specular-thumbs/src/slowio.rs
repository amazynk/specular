//! Simulated slow storage for benchmarks: `SPECULAR_SLOW_IO=OPEN_MS,STAT_MS,MB_PER_S` adds
//! OPEN_MS before each file open (source files and cache entries), STAT_MS before each `stat`
//! of a listing, and a transfer time at MB_PER_S for the bytes read. Unset (the default) costs
//! one relaxed load per call.
//!
//! Only the engine's workers and the listing threads call these, never the UI thread: a slow
//! disk must show up as later thumbnails, not as slower frames.

use std::sync::OnceLock;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SlowIo {
    pub open: Duration,
    pub stat: Duration,
    /// Bytes per second (0: unlimited).
    pub bytes_per_s: f64,
}

impl SlowIo {
    /// Parses `OPEN_MS,STAT_MS,MB_PER_S` (missing fields are 0).
    pub fn parse(s: &str) -> Option<SlowIo> {
        let mut it = s.split(',').map(|v| v.trim().parse::<f64>().ok());
        let ms = |v: Option<Option<f64>>| -> Option<Duration> {
            match v {
                None => Some(Duration::ZERO),
                Some(Some(x)) if x >= 0.0 && x.is_finite() => {
                    Some(Duration::from_secs_f64(x / 1e3))
                }
                _ => None,
            }
        };
        let open = ms(it.next())?;
        let stat = ms(it.next())?;
        let bytes_per_s = match it.next() {
            None => 0.0,
            Some(Some(x)) if x >= 0.0 && x.is_finite() => x * 1e6,
            _ => return None,
        };
        Some(SlowIo {
            open,
            stat,
            bytes_per_s,
        })
    }

    fn transfer(&self, bytes: u64) -> Duration {
        if self.bytes_per_s > 0.0 {
            Duration::from_secs_f64(bytes as f64 / self.bytes_per_s)
        } else {
            Duration::ZERO
        }
    }
}

fn config() -> Option<&'static SlowIo> {
    static CFG: OnceLock<Option<SlowIo>> = OnceLock::new();
    CFG.get_or_init(|| {
        let v = std::env::var("SPECULAR_SLOW_IO").ok()?;
        let c = SlowIo::parse(&v);
        if c.is_none() {
            specular_log::warn!(target: specular_log::CACHE, "SPECULAR_SLOW_IO={v}: expected OPEN_MS,STAT_MS,MB_PER_S");
        }
        c.filter(|c| *c != SlowIo::default())
    })
    .as_ref()
}

fn sleep(d: Duration) {
    if !d.is_zero() {
        std::thread::sleep(d);
    }
}

/// Before opening a file and reading `bytes` of it.
pub fn open(bytes: u64) {
    if let Some(c) = config() {
        sleep(c.open + c.transfer(bytes));
    }
}

/// Before a listing's `stat`.
pub fn stat() {
    if let Some(c) = config() {
        sleep(c.stat);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses() {
        let c = SlowIo::parse("8,0.2,40").unwrap();
        assert_eq!(c.open, Duration::from_millis(8));
        assert_eq!(c.stat, Duration::from_micros(200));
        assert_eq!(c.bytes_per_s, 40e6);
        assert_eq!(c.transfer(40_000_000), Duration::from_secs(1));
        assert_eq!(SlowIo::parse("5").unwrap().stat, Duration::ZERO);
        assert!(SlowIo::parse("x").is_none());
    }
}
