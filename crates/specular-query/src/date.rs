//! Civil dates as day numbers (days since 1970-01-01, proleptic Gregorian; Hinnant's
//! algorithms). Times in queries are "local seconds as stored": capture times carry no zone,
//! so a day is simply 86 400 of them.

pub const DAY: i64 = 86_400;

/// Day number of a civil date.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil date of a day number.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (y + i64::from(m <= 2), m, d)
}

/// Year of a time in seconds.
pub fn year_of(secs: i64) -> i64 {
    civil_from_days(secs.div_euclid(DAY)).0
}

pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A calendar period typed in a query: a year, a month or a day, as the half-open range of
/// seconds `[start, end)` it covers.
pub fn period(text: &str) -> Option<(i64, i64)> {
    let parts: Vec<&str> = text.split(['-', '/', '.']).collect();
    let num = |s: &str, digits: std::ops::RangeInclusive<usize>| -> Option<i64> {
        (digits.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    let y = num(parts.first()?, 4..=4)?;
    let (start, end) = match parts.len() {
        1 => (days_from_civil(y, 1, 1), days_from_civil(y + 1, 1, 1)),
        2 | 3 => {
            let m = u32::try_from(num(parts[1], 1..=2)?).ok()?;
            if !(1..=12).contains(&m) {
                return None;
            }
            if parts.len() == 2 {
                let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                (days_from_civil(y, m, 1), days_from_civil(ny, nm, 1))
            } else {
                let d = u32::try_from(num(parts[2], 1..=2)?).ok()?;
                if d == 0 || d > days_in_month(y, m) {
                    return None;
                }
                let s = days_from_civil(y, m, d);
                (s, s + 1)
            }
        }
        _ => return None,
    };
    Some((start * DAY, end * DAY))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trip() {
        for (y, m, d) in [
            (1970, 1, 1),
            (2000, 2, 29),
            (2024, 12, 31),
            (1969, 12, 31),
            (1600, 3, 1),
        ] {
            assert_eq!(civil_from_days(days_from_civil(y, m, d)), (y, m, d));
        }
        assert_eq!(days_from_civil(1970, 1, 2), 1);
    }

    #[test]
    fn periods() {
        let jan = days_from_civil(2024, 1, 1) * DAY;
        assert_eq!(
            period("2024"),
            Some((jan, days_from_civil(2025, 1, 1) * DAY))
        );
        assert_eq!(
            period("2024-12"),
            Some((
                days_from_civil(2024, 12, 1) * DAY,
                days_from_civil(2025, 1, 1) * DAY
            ))
        );
        let d = days_from_civil(2024, 2, 29) * DAY;
        assert_eq!(period("2024-02-29"), Some((d, d + DAY)));
        assert_eq!(period("2023-02-29"), None);
        assert_eq!(period("2024-13"), None);
        assert_eq!(period("24"), None);
        assert_eq!(period("2024-1-5"), period("2024-01-05"));
        assert_eq!(period(""), None);
        assert_eq!(period("２０２４"), None);
    }
}
