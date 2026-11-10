//! Times as maps meet them: ISO 8601 stamps of GPS fixes (GPX, TCX), EXIF capture times with
//! or without an offset, and the dates of weather grids. Seconds since 1970-01-01 UTC on the
//! proleptic Gregorian calendar, as `f64` (fractions kept).

/// A parsed stamp: seconds since the epoch, and whether it named its zone (a stamp without
/// one is the camera's local time, read here as if it were UTC).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stamp {
    pub secs: f64,
    pub zoned: bool,
}

/// Days from 1970-01-01 to a civil date (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day count from 1970-01-01.
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
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn num(s: &str) -> Option<u32> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

/// Reads "2024-06-03T10:20:31Z", "2024-06-03T10:20:31.250+02:00", "2024-06-03 10:20",
/// EXIF's "2024:06:03 10:20:31", a bare date, and a weather grid's label with a trailing
/// note in brackets ("2024-01-01 06:00 (+6 h)").
pub fn parse(s: &str) -> Option<Stamp> {
    let s = s.trim();
    let s = s.split(" (").next().unwrap_or(s).trim();
    if s.len() < 10 {
        return None;
    }
    let (date, rest) = s.split_at(10);
    let b = date.as_bytes();
    if !(b[4] == b'-' || b[4] == b':') || b[7] != b[4] {
        return None;
    }
    let y: i64 = i64::from(num(&date[0..4])?);
    let mo = num(&date[5..7])?;
    let d = num(&date[8..10])?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let day = days_from_civil(y, mo, d) as f64 * 86_400.0;
    let rest = rest.trim_start_matches(['T', 't', ' ']);
    if rest.is_empty() {
        return Some(Stamp {
            secs: day,
            zoned: false,
        });
    }
    // The zone: Z, or ±HH[:MM] after the time.
    let (time, zone) = match rest.find(['Z', 'z', '+', '-']) {
        Some(i) => (&rest[..i], Some(&rest[i..])),
        None => (rest, None),
    };
    let mut parts = time.trim().split(':');
    let h = num(parts.next()?)?;
    let mi = num(parts.next()?)?;
    let sec: f64 = match parts.next() {
        Some(p) => {
            let p = p.trim();
            if p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit() || c == b'.') {
                return None;
            }
            p.parse().ok()?
        }
        None => 0.0,
    };
    if h > 24 || mi > 59 || !(0.0..61.0).contains(&sec) {
        return None;
    }
    let mut secs = day + f64::from(h) * 3600.0 + f64::from(mi) * 60.0 + sec;
    let zoned = match zone {
        None => false,
        Some(z) if z.eq_ignore_ascii_case("z") => true,
        Some(z) => {
            secs -= f64::from(offset(z)?);
            true
        }
    };
    Some(Stamp { secs, zoned })
}

/// An offset "+02:00", "-0530" or "+01" in seconds east of UTC.
pub fn offset(z: &str) -> Option<i32> {
    let z = z.trim();
    let sign = match z.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits: String = z[1..].chars().filter(|c| *c != ':').collect();
    let (h, m) = match digits.len() {
        2 => (num(&digits)?, 0),
        4 => (num(&digits[..2])?, num(&digits[2..])?),
        _ => return None,
    };
    if h > 18 || m > 59 {
        return None;
    }
    Some(sign * (h as i32 * 3600 + m as i32 * 60))
}

/// "2024-06-03 10:20:31" for seconds since the epoch (rounded to the second).
pub fn format(secs: f64) -> String {
    let s = secs.round() as i64;
    let (days, rem) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// A signed duration as "+1:02:05", "−0:00:45" (for a camera clock's offset).
pub fn format_offset(secs: f64) -> String {
    let neg = secs < 0.0;
    let s = secs.abs().round() as i64;
    let (h, m, sec) = (s / 3600, s % 3600 / 60, s % 60);
    let sign = if neg { '\u{2212}' } else { '+' };
    // Digits and colons only: nothing to translate, and the run keeps its order in
    // right-to-left text.
    format!("{sign}{h}:{m:02}:{sec:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_round_trip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        for z in [-800_000, -1, 0, 59, 11_016, 19_000, 2_000_000] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
    }

    #[test]
    fn stamps_parse() {
        let a = parse("2024-06-03T10:20:31Z").unwrap();
        assert!(a.zoned);
        assert_eq!(a.secs, 1_717_410_031.0);
        let b = parse("2024-06-03T12:20:31.5+02:00").unwrap();
        assert_eq!(b.secs, 1_717_410_031.5);
        let c = parse("2024:06:03 10:20:31").unwrap();
        assert!(!c.zoned);
        assert_eq!(c.secs, a.secs);
        assert_eq!(parse("2024-06-03").unwrap().secs, 1_717_372_800.0);
        assert_eq!(
            parse("2024-06-03 10:20 (+6 h)").unwrap().secs,
            1_717_410_000.0
        );
        assert_eq!(parse("2024-06-03T05:50:31-0430").unwrap().secs, a.secs);
        assert!(parse("2024-13-03").is_none());
        assert!(parse("yesterday").is_none());
        assert_eq!(format(a.secs), "2024-06-03 10:20:31");
        assert_eq!(format_offset(-3725.0), "\u{2212}1:02:05");
        assert_eq!(format_offset(45.0), "+0:00:45");
    }
}
