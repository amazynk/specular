//! Wall-clock budgets in tests: a test measures (best of a few runs) and asks
//! [`assert_within`]. On a machine with free cores a time over budget fails the test as it
//! always did. When the machine is busier than it has cores (the one-minute load average
//! above the core count, Linux), a time over budget is not judged: the test says so on
//! stderr and passes, since other processes took the time. A budget is never relaxed.

use std::time::Duration;

/// The one-minute load average and the cores, when the load is above the cores.
pub fn overloaded() -> Option<(f64, usize)> {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let load = load_average()?;
    (load > cores as f64).then_some((load, cores))
}

/// The one-minute load average (Linux `/proc/loadavg`); `None` elsewhere.
pub fn load_average() -> Option<f64> {
    let s = std::fs::read_to_string("/proc/loadavg").ok()?;
    s.split_whitespace().next()?.parse().ok()
}

/// Whether `took` is within `budget`, or is not judged because the machine is overloaded
/// (said on stderr). `what` names the measurement.
pub fn within(what: &str, took: Duration, budget: Duration) -> bool {
    within_at(what, took, budget, overloaded())
}

fn within_at(what: &str, took: Duration, budget: Duration, busy: Option<(f64, usize)>) -> bool {
    if took <= budget {
        return true;
    }
    match busy {
        Some((load, cores)) => {
            eprintln!(
                "{what}: {took:?} over its {budget:?} budget, not judged: load average \
                 {load:.1} on {cores} cores"
            );
            true
        }
        None => false,
    }
}

/// Fails the test when `took` is over `budget` on a machine that is not overloaded.
#[track_caller]
pub fn assert_within(what: &str, took: Duration, budget: Duration) {
    assert!(
        within(what, took, budget),
        "{what} took {took:?} (budget {budget:?})"
    );
}

/// How many budgets a wait may run on an overloaded machine before it counts as a hang.
pub const HANG_FACTOR: u32 = 12;

/// A wall-clock limit on waiting for work in the background (a frame, a reply), checked in
/// the wait's loop. Past the budget the test fails on a machine with free cores; on an
/// overloaded one it keeps waiting (said once on stderr), up to [`HANG_FACTOR`] budgets, after
/// which it fails anyway: that is a hang, not load. The budget is never relaxed.
pub struct Deadline {
    what: &'static str,
    start: std::time::Instant,
    budget: Duration,
    said: std::cell::Cell<bool>,
}

impl Deadline {
    pub fn new(what: &'static str, budget: Duration) -> Deadline {
        Deadline {
            what,
            start: std::time::Instant::now(),
            budget,
            said: std::cell::Cell::new(false),
        }
    }

    /// Fails the test when the wait is over its budget (see [`Deadline`]).
    #[track_caller]
    pub fn check(&self) {
        let took = self.start.elapsed();
        if let Err(e) = self.judge(took, overloaded()) {
            panic!("{e}");
        }
    }

    fn judge(&self, took: Duration, busy: Option<(f64, usize)>) -> Result<(), String> {
        if took <= self.budget {
            return Ok(());
        }
        let what = self.what;
        let budget = self.budget;
        match busy {
            Some(_) if took > budget * HANG_FACTOR => Err(format!(
                "{what}: still waiting after {took:?} ({HANG_FACTOR}× its {budget:?} budget, \
                 overloaded)"
            )),
            Some((load, cores)) => {
                if !self.said.replace(true) {
                    eprintln!(
                        "{what}: over its {budget:?} budget, not judged: load average \
                         {load:.1} on {cores} cores"
                    );
                }
                Ok(())
            }
            None => Err(format!("{what}: took over {budget:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deadline_waits_longer_only_when_overloaded() {
        let ms = Duration::from_millis;
        let d = Deadline::new("x", ms(10));
        assert!(d.judge(ms(5), None).is_ok());
        assert!(d.judge(ms(15), None).is_err());
        assert!(d.judge(ms(15), Some((40.0, 16))).is_ok());
        assert!(d.judge(ms(119), Some((40.0, 16))).is_ok());
        assert!(d.judge(ms(121), Some((40.0, 16))).is_err());
        d.check();
    }

    #[test]
    fn over_budget_fails_unless_overloaded() {
        let ms = Duration::from_millis;
        assert!(within_at("x", ms(5), ms(10), None));
        assert!(!within_at("x", ms(15), ms(10), None));
        assert!(within_at("x", ms(15), ms(10), Some((40.0, 16))));
        assert!(within_at("x", ms(5), ms(10), Some((40.0, 16))));
        if cfg!(target_os = "linux") {
            assert!(load_average().is_some_and(|l| l >= 0.0));
        }
    }
}
