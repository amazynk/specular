//! GUIDs (random UUIDs, lower case as BCF 3.0 wants) and xs:dateTime stamps.

use std::time::{SystemTime, UNIX_EPOCH};

/// A random (version 4) UUID, lower case.
pub fn new_guid() -> String {
    let mut b = [0u8; 16];
    if getrandom::fill(&mut b).is_err() {
        // No entropy source: the clock and a counter (unique within the run).
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        b[..8].copy_from_slice(&t.to_le_bytes());
        b[8..].copy_from_slice(&N.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// Is `s` a GUID as the schemas spell it (8-4-4-4-12 hex digits)?
pub fn is_guid(s: &str, lower_only: bool) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, n)| p.len() == n && p.bytes().all(|c| c.is_ascii_hexdigit()))
        && (!lower_only || !s.bytes().any(|c| c.is_ascii_uppercase()))
}

/// Now, as xs:dateTime in UTC (`2026-10-25T09:30:00Z`).
pub fn now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    date_time(secs)
}

/// Seconds since 1970 as xs:dateTime in UTC.
pub fn date_time(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, m, d) = civil(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 → (year, month, day), proleptic Gregorian.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Is `s` an xs:dateTime (`YYYY-MM-DDThh:mm:ss`, optional fraction, optional zone)?
pub fn is_date_time(s: &str) -> bool {
    let b = s.as_bytes();
    let digits =
        |r: std::ops::Range<usize>| r.clone().all(|i| b.get(i).is_some_and(u8::is_ascii_digit));
    if b.len() < 19
        || !digits(0..4)
        || b[4] != b'-'
        || !digits(5..7)
        || b[7] != b'-'
        || !digits(8..10)
        || b[10] != b'T'
        || !digits(11..13)
        || b[13] != b':'
        || !digits(14..16)
        || b[16] != b':'
        || !digits(17..19)
    {
        return false;
    }
    let mut i = 19;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let s0 = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == s0 {
            return false;
        }
    }
    match &b[i..] {
        [] | [b'Z'] => true,
        [s, h1, h2, b':', m1, m2] => {
            (*s == b'+' || *s == b'-') && [h1, h2, m1, m2].iter().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guids_and_dates() {
        let g = new_guid();
        assert!(is_guid(&g, true), "{g}");
        assert_ne!(g, new_guid());
        assert_eq!(&g[14..15], "4");
        assert!(!is_guid("0123", false));
        assert!(!is_guid("ABCDEF01-2345-6789-ABCD-EF0123456789", true));
        assert!(is_guid("ABCDEF01-2345-6789-ABCD-EF0123456789", false));
        assert_eq!(date_time(0), "1970-01-01T00:00:00Z");
        assert_eq!(date_time(1_792_920_600), "2026-10-25T09:30:00Z");
        assert!(is_date_time(&now()));
        assert!(is_date_time("2014-10-16T13:10:56.000+02:00"));
        assert!(is_date_time("2015-06-09T14:50:28"));
        assert!(!is_date_time("2015-06-09"));
        assert!(!is_date_time("2015-06-09T14:50:28.+02:00"));
    }
}
