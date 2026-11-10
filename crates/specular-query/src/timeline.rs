//! The date timeline: photo counts per day made into bars of years, months or days (the
//! finest that fits), and the `date:` term a bar or a run of bars stands for.
//!
//! Dates are what `date:` tests: the capture time, else the modification time, as local
//! seconds. Bars run without gaps from the first dated photo to the last, so empty stretches
//! show as empty bars.

use crate::date::{DAY, civil_from_days, days_from_civil};
use crate::facet::{append, remove_span};
use crate::parse::{self, Field, Kind, Pred};

/// How long one bar is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    Day,
    Month,
    Year,
}

/// One bar: the days `[start, end)` (day numbers) and the photos in them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bar {
    pub start: i64,
    pub end: i64,
    pub count: u64,
    pub step: Step,
}

impl Bar {
    /// The civil date the bar starts on.
    pub fn date(&self) -> (i64, u32, u32) {
        civil_from_days(self.start)
    }

    /// The `date:` value for the bar alone: `2025`, `2025-06` or `2025-06-03`.
    pub fn value(&self) -> String {
        let (y, m, d) = self.date();
        match self.step {
            Step::Year => format!("{y:04}"),
            Step::Month => format!("{y:04}-{m:02}"),
            Step::Day => format!("{y:04}-{m:02}-{d:02}"),
        }
    }

    /// Whether the bar lies within the seconds `[lo, hi)`.
    pub fn within(&self, lo: i64, hi: i64) -> bool {
        self.start * DAY >= lo && self.end * DAY <= hi
    }
}

/// The first day of the period of `step` holding `day`, and of the next one.
fn period_of(day: i64, step: Step) -> (i64, i64) {
    let (y, m, _) = civil_from_days(day);
    match step {
        Step::Day => (day, day + 1),
        Step::Month => {
            let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            (days_from_civil(y, m, 1), days_from_civil(ny, nm, 1))
        }
        Step::Year => (days_from_civil(y, 1, 1), days_from_civil(y + 1, 1, 1)),
    }
}

/// How many bars of `step` cover the days `first..=last`.
fn bars_needed(first: i64, last: i64, step: Step) -> i64 {
    let (ya, ma, _) = civil_from_days(first);
    let (yb, mb, _) = civil_from_days(last);
    match step {
        Step::Day => last - first + 1,
        Step::Month => (yb - ya) * 12 + i64::from(mb) - i64::from(ma) + 1,
        Step::Year => yb - ya + 1,
    }
}

/// The step the bars use for the days `first..=last` with at most `max_bars` bars: days,
/// else months, else years (years always, however many).
pub fn step_for(first: i64, last: i64, max_bars: usize) -> Step {
    let max = max_bars.max(1) as i64;
    [Step::Day, Step::Month]
        .into_iter()
        .find(|s| bars_needed(first, last, *s) <= max)
        .unwrap_or(Step::Year)
}

/// Counts per day (day number, photos; any order, days may repeat) made into bars at the
/// finest step that fits `max_bars`, from the first dated day to the last. Empty without
/// any dated photo.
pub fn bars(days: &[(i64, u64)], max_bars: usize) -> Vec<Bar> {
    let (Some(first), Some(last)) = (
        days.iter().filter(|d| d.1 > 0).map(|d| d.0).min(),
        days.iter().filter(|d| d.1 > 0).map(|d| d.0).max(),
    ) else {
        return Vec::new();
    };
    bars_at(days, first, last, step_for(first, last, max_bars))
}

/// [`bars`] at a given step.
pub fn bars_at(days: &[(i64, u64)], first: i64, last: i64, step: Step) -> Vec<Bar> {
    let mut out = Vec::new();
    let mut d = first;
    while d <= last {
        let (start, end) = period_of(d, step);
        out.push(Bar {
            start,
            end,
            count: 0,
            step,
        });
        d = end;
    }
    for &(day, n) in days {
        if n == 0 || day < first || day > last {
            continue;
        }
        // Bars are in order and contiguous: find by binary search.
        let k = out.partition_point(|b| b.end <= day);
        if let Some(b) = out.get_mut(k) {
            b.count += n;
        }
    }
    out
}

/// The `date:` value for the bars `a` to `b` (either order, same step): one bar's own
/// value, else a range `2025-03..2025-06`.
pub fn range_value(a: &Bar, b: &Bar) -> String {
    let (lo, hi) = if a.start <= b.start { (a, b) } else { (b, a) };
    if lo.start == hi.start {
        lo.value()
    } else {
        format!("{}..{}", lo.value(), hi.value())
    }
}

/// The seconds `[lo, hi)` the query's `date:` terms (not negated) leave; None when it has
/// none.
pub fn selected(text: &str) -> Option<(i64, i64)> {
    let q = parse::parse(text);
    let mut out: Option<(f64, f64)> = None;
    for t in &q.terms {
        if t.negated {
            continue;
        }
        let Kind::Field {
            field: Field::Date,
            pred: Pred::Date(ranges),
            ..
        } = &t.kind
        else {
            continue;
        };
        // A term's ranges are alternatives: their hull.
        let lo = ranges.iter().map(|r| r.lo).fold(f64::INFINITY, f64::min);
        let hi = ranges
            .iter()
            .map(|r| r.hi)
            .fold(f64::NEG_INFINITY, f64::max);
        out = Some(match out {
            Some((a, b)) => (a.max(lo), b.min(hi)),
            None => (lo, hi),
        });
    }
    out.map(|(lo, hi)| (clamp_secs(lo), clamp_secs(hi)))
}

fn clamp_secs(v: f64) -> i64 {
    if v.is_nan() {
        0
    } else {
        v.clamp(i64::MIN as f64, i64::MAX as f64) as i64
    }
}

/// The query with its `date:` terms (not negated) replaced by `date:value`, or removed
/// with None.
pub fn set(text: &str, value: Option<&str>) -> String {
    let q = parse::parse(text);
    let mut out = text.to_owned();
    // From the end, so earlier spans stay valid.
    for t in q.terms.iter().rev() {
        if !t.negated && t.field() == Some(Field::Date) {
            out = remove_span(&out, t.span);
        }
    }
    match value {
        Some(v) => append(&out, &format!("date:{v}")),
        None => out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i64, m: u32, d: u32) -> i64 {
        days_from_civil(y, m, d)
    }

    #[test]
    fn steps_fit_the_room() {
        let (a, b) = (day(2024, 1, 30), day(2024, 2, 2));
        assert_eq!(step_for(a, b, 10), Step::Day);
        assert_eq!(step_for(a, b, 3), Step::Month);
        assert_eq!(step_for(a, b, 1), Step::Year);
        assert_eq!(bars_needed(a, b, Step::Month), 2);
        assert_eq!(
            bars_needed(day(2019, 12, 1), day(2025, 1, 1), Step::Year),
            7
        );
        // Years however many.
        assert_eq!(step_for(day(1900, 1, 1), day(2025, 1, 1), 5), Step::Year);
    }

    #[test]
    fn bars_cover_every_period_with_their_counts() {
        let days = [
            (day(2024, 11, 3), 2),
            (day(2025, 2, 10), 5),
            (day(2024, 11, 30), 1),
            (day(2025, 2, 10), 1),
        ];
        let b = bars(&days, 6);
        assert_eq!(b.len(), 4, "Nov, Dec, Jan, Feb");
        assert!(b.iter().all(|b| b.step == Step::Month));
        assert_eq!(
            b.iter().map(|b| b.count).collect::<Vec<_>>(),
            vec![3, 0, 0, 6]
        );
        assert_eq!(b[0].value(), "2024-11");
        assert_eq!(b[3].value(), "2025-02");
        assert_eq!(range_value(&b[3], &b[0]), "2024-11..2025-02");
        assert_eq!(range_value(&b[1], &b[1]), "2024-12");
        let y = bars(&days, 1);
        assert_eq!(y.len(), 2);
        assert_eq!(y[0].value(), "2024");
        assert_eq!(y[1].count, 6);
        let d = bars(&[(day(2024, 2, 28), 1), (day(2024, 3, 1), 4)], 31);
        assert_eq!(d.len(), 3, "leap day included");
        assert_eq!(d[1].value(), "2024-02-29");
        assert!(bars(&[], 10).is_empty());
        assert!(bars(&[(5, 0)], 10).is_empty());
    }

    #[test]
    fn date_terms_are_set_replaced_and_read() {
        assert_eq!(
            set("rating:>=3", Some("2025-06")),
            "rating:>=3 date:2025-06"
        );
        assert_eq!(
            set("date:2024 camera:x taken:2023", Some("2025")),
            "camera:x date:2025"
        );
        assert_eq!(set("date:2024 -date:2024-05", None), "-date:2024-05");
        assert_eq!(set("", None), "");
        let (lo, hi) = selected("x date:2025-06").unwrap();
        assert_eq!(lo, day(2025, 6, 1) * DAY);
        assert_eq!(hi, day(2025, 7, 1) * DAY);
        let (lo, hi) = selected("date:2025 date:2025-03..2025-04").unwrap();
        assert_eq!((lo, hi), (day(2025, 3, 1) * DAY, day(2025, 5, 1) * DAY));
        assert_eq!(selected("-date:2025 rating:3"), None);
        let b = Bar {
            start: day(2025, 6, 1),
            end: day(2025, 7, 1),
            count: 1,
            step: Step::Month,
        };
        assert!(b.within(lo, hi + 3 * 31 * DAY));
        assert!(!b.within(lo, hi));
    }
}
