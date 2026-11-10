//! Parsing EXIF/XMP date strings and formatting them in plain language.

use std::time::SystemTime;

use chrono::{
    DateTime, Datelike, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone,
    Timelike, Utc,
};

use crate::i18n::Localizer;
use crate::strings;

/// A date-time as recorded in the file: local wall-clock time plus the UTC
/// offset when the file states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub local: NaiveDateTime,
    pub offset_minutes: Option<i32>,
}

impl Stamp {
    /// ISO 8601: "2024-05-14T18:32:10+01:00", or without offset when unknown.
    pub fn iso(&self) -> String {
        let base = self.local.format("%Y-%m-%dT%H:%M:%S").to_string();
        match self.offset_minutes {
            Some(off) => {
                let sign = if off < 0 { '-' } else { '+' };
                format!("{base}{sign}{:02}:{:02}", off.abs() / 60, off.abs() % 60)
            }
            None => base,
        }
    }

    /// "Tuesday 14 May 2024, 18:32 (UTC+1)" in en-GB; ISO 8601 if ICU cannot format the date.
    pub fn display(&self, l: &Localizer) -> String {
        let d = self.local;
        strings::date_time(
            l,
            d.year(),
            d.month(),
            d.day(),
            d.hour(),
            d.minute(),
            self.offset_minutes,
        )
        .unwrap_or_else(|| self.iso())
    }

    /// True when this moment is later than `now` (with slack for unknown zones and clock skew).
    pub fn is_future(&self, now: SystemTime) -> bool {
        let now: DateTime<Utc> = now.into();
        match self.offset_minutes {
            Some(off) => {
                let utc = self.local - chrono::Duration::minutes(off as i64);
                utc > now.naive_utc() + chrono::Duration::minutes(10)
            }
            // Wall-clock time in an unknown zone: allow for the furthest-ahead zone.
            None => {
                self.local
                    > now.naive_utc() + chrono::Duration::hours(14) + chrono::Duration::minutes(10)
            }
        }
    }
}

/// Parses EXIF "YYYY:MM:DD HH:MM:SS" (also tolerates '-' separators, 'T', and a missing seconds field).
/// Blank or zeroed values ("    :  :     ", "0000:00:00 00:00:00") return `None`.
pub fn parse_exif_datetime(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim().trim_end_matches('\0');
    if s.len() < 16 {
        return None;
    }
    let b = s.as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<u32> { s.get(r)?.trim().parse().ok() };
    let year = num(0..4)? as i32;
    let month = num(5..7)?;
    let day = num(8..10)?;
    if !matches!(b[10], b' ' | b'T') {
        return None;
    }
    let hour = num(11..13)?;
    let minute = num(14..16)?;
    let second = if s.len() >= 19 {
        num(17..19).unwrap_or(0)
    } else {
        0
    };
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    // Some cameras write 24:00:00.
    let (hour, add_day) = if hour == 24 { (0, true) } else { (hour, false) };
    let time = NaiveTime::from_hms_opt(hour, minute, second.min(59))?;
    let dt = NaiveDateTime::new(date, time);
    Some(if add_day {
        dt + chrono::Duration::days(1)
    } else {
        dt
    })
}

/// Parses an EXIF offset "+01:00" / "-03:30" into minutes.
pub fn parse_offset(s: &str) -> Option<i32> {
    let s = s.trim().trim_end_matches('\0');
    let (sign, rest) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    let (h, m) = rest.split_once(':').unwrap_or((rest, "0"));
    let h: i32 = h.trim().parse().ok()?;
    let m: i32 = m.trim().parse().ok()?;
    if h > 14 || m >= 60 {
        return None;
    }
    Some(sign * (h * 60 + m))
}

/// Parses an XMP (ISO 8601) date: "2024-05-14T18:32:10+01:00", "2024-05-14T18:32:10.25Z",
/// "2024-05-14T18:32", "2024-05-14" (midnight, no offset).
pub fn parse_xmp_datetime(s: &str) -> Option<Stamp> {
    let s = s.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(Stamp {
            local: dt.naive_local(),
            offset_minutes: Some(dt.offset().local_minus_utc() / 60),
        });
    }
    // Split off a trailing zone designator.
    let (body, offset) = if let Some(b) = s.strip_suffix('Z') {
        (b, Some(0))
    } else if s.len() > 10
        && let Some(pos) = s[10..].rfind(['+', '-']).map(|p| p + 10)
    {
        (&s[..pos], parse_offset(&s[pos..]))
    } else {
        (s, None)
    };
    let body = body.split('.').next().unwrap_or(body);
    for fmt in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
        if let Ok(local) = NaiveDateTime::parse_from_str(body, fmt) {
            return Some(Stamp {
                local,
                offset_minutes: offset,
            });
        }
    }
    NaiveDate::parse_from_str(body, "%Y-%m-%d")
        .ok()
        .map(|d| Stamp {
            local: d.and_hms_opt(0, 0, 0).unwrap_or_default(),
            offset_minutes: None,
        })
}

/// Infers the UTC offset from a local camera time and the GPS (UTC) time, when the two
/// agree to within a few minutes of a 15-minute zone boundary.
pub fn offset_from_gps(local: NaiveDateTime, gps_utc: NaiveDateTime) -> Option<i32> {
    let diff = (local - gps_utc).num_seconds() as f64 / 60.0;
    let rounded = (diff / 15.0).round() * 15.0;
    if rounded.abs() > 14.0 * 60.0 || (diff - rounded).abs() > 3.0 {
        return None;
    }
    Some(rounded as i32)
}

/// Formats a file-system time in the viewer's local time zone.
pub fn format_system_time(t: SystemTime, l: &Localizer) -> String {
    let local: DateTime<Local> = t.into();
    let fixed: DateTime<FixedOffset> = local.fixed_offset();
    Stamp {
        local: fixed.naive_local(),
        offset_minutes: Some(fixed.offset().local_minus_utc() / 60),
    }
    .display(l)
}

/// "14 May 2024, 17:32:10 UTC".
pub fn format_utc(dt: NaiveDateTime, l: &Localizer) -> String {
    strings::utc_date_time(
        l,
        dt.year(),
        dt.month(),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
    )
    .unwrap_or_else(|| utc_iso(dt))
}

/// GPS date stamp "YYYY:MM:DD" + hours/minutes/seconds.
pub fn gps_datetime(date: &str, h: f64, m: f64, s: f64) -> Option<NaiveDateTime> {
    let date = date.trim().trim_end_matches('\0').replace('-', ":");
    let mut it = date.split(':');
    let y: i32 = it.next()?.parse().ok()?;
    let mo: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    let secs = (h * 3600.0 + m * 60.0 + s).floor();
    if !(0.0..86_400.0).contains(&secs) {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(y, mo, d)?;
    Some(date.and_hms_opt(0, 0, 0)? + chrono::Duration::seconds(secs as i64))
}

/// "2024-05-14T17:32:10Z".
pub fn utc_iso(dt: NaiveDateTime) -> String {
    Utc.from_utc_datetime(&dt)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{langid, strip_isolates};

    #[test]
    fn exif_dates() {
        let l = Localizer::english();
        let d = parse_exif_datetime("2024:05:14 18:32:10").unwrap();
        let s = Stamp {
            local: d,
            offset_minutes: Some(60),
        };
        assert_eq!(
            strip_isolates(&s.display(&l)),
            "Tuesday 14 May 2024, 18:32 (UTC+1)"
        );
        let de = Localizer::new(langid!("de"));
        assert_eq!(
            strip_isolates(&s.display(&de)),
            "Dienstag, 14. Mai 2024, 18:32 (UTC+1)"
        );
        assert_eq!(s.iso(), "2024-05-14T18:32:10+01:00");
        assert!(parse_exif_datetime("    :  :     :  :  ").is_none());
        assert!(parse_exif_datetime("0000:00:00 00:00:00").is_none());
        let s = Stamp {
            local: d,
            offset_minutes: Some(-210),
        };
        assert_eq!(
            strip_isolates(&s.display(&l)),
            "Tuesday 14 May 2024, 18:32 (UTC\u{2212}3:30)"
        );
        assert_eq!(s.iso(), "2024-05-14T18:32:10-03:30");
    }

    #[test]
    fn xmp_dates() {
        let s = parse_xmp_datetime("2024-05-14T18:32:10.25+01:00").unwrap();
        assert_eq!(s.offset_minutes, Some(60));
        let s = parse_xmp_datetime("2024-05-14T18:32").unwrap();
        assert_eq!(s.offset_minutes, None);
        assert_eq!(s.iso(), "2024-05-14T18:32:00");
        let s = parse_xmp_datetime("2024-05-14T18:32-03:00").unwrap();
        assert_eq!(s.offset_minutes, Some(-180));
    }

    #[test]
    fn gps_offset() {
        let local = parse_exif_datetime("2024:05:14 18:32:10").unwrap();
        let utc = gps_datetime("2024:05:14", 17.0, 31.0, 58.0).unwrap();
        assert_eq!(offset_from_gps(local, utc), Some(60));
        let utc = gps_datetime("2024:05:14", 17.0, 10.0, 0.0).unwrap();
        assert_eq!(offset_from_gps(local, utc), None);
    }
}
