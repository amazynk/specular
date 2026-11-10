//! NMEA 0183 GPS logs: `$..RMC` (time, date, position, speed, course), `$..GGA` (time,
//! position, fix quality, satellites, altitude), `$..GLL` (position, time) and `$..WPL`
//! waypoints, from any talker (GP, GN, GL, GA…). Sentences whose checksum is wrong are
//! skipped. Read as a track (one line, with its summary) and the waypoints.

use crate::crs;
use crate::igc::iso;
use crate::model::{Dataset, Geometry, Layer, MAX_VERTICES, Pt, Value};

/// One fix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fix {
    /// Seconds since 1970 (UTC) once a date is known (RMC); else since midnight.
    pub t: Option<f64>,
    pub lon: f64,
    pub lat: f64,
    pub alt: Option<f64>,
    /// Knots.
    pub speed: Option<f64>,
}

/// The sentence a line holds (between `$` or `!` and `*`) when its checksum is right or
/// absent.
fn sentence(line: &str) -> Option<&str> {
    let start = line.find(['$', '!'])?;
    let body = &line[start + 1..];
    match body.split_once('*') {
        Some((s, sum)) => {
            let want = u8::from_str_radix(sum.get(..2)?, 16).ok()?;
            let got = s.bytes().fold(0u8, |a, b| a ^ b);
            (want == got).then_some(s)
        }
        None => Some(body.trim_end()),
    }
}

/// Whether the bytes look like an NMEA log: two or more sentences with right checksums in
/// the first lines.
pub fn sniff(head: &[u8]) -> bool {
    let t = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    let (mut good, mut bad) = (0, 0);
    for l in t.lines().take(20) {
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        if !l.starts_with('$') || !l.contains('*') {
            return false;
        }
        match sentence(l) {
            Some(s) if s.len() > 5 && s.as_bytes()[..5].iter().all(u8::is_ascii_uppercase) => {
                good += 1
            }
            _ => bad += 1,
        }
    }
    good >= 2 && good > bad
}

/// `ddmm.mmmm` with its hemisphere as degrees.
fn coord(v: &str, hemi: &str, deg: usize) -> Option<f64> {
    if v.len() < deg + 2 || !v.is_ascii() {
        return None;
    }
    let d: f64 = v[..deg].parse().ok()?;
    let m: f64 = v[deg..].parse().ok()?;
    if !(0.0..60.0).contains(&m) {
        return None;
    }
    let x = d + m / 60.0;
    match hemi {
        "N" | "E" => Some(x),
        "S" | "W" => Some(-x),
        _ => None,
    }
}

/// `hhmmss.ss` as seconds since midnight.
fn hms(v: &str) -> Option<f64> {
    if v.len() < 6 || !v.is_ascii() {
        return None;
    }
    let h: f64 = v[..2].parse().ok()?;
    let m: f64 = v[2..4].parse().ok()?;
    let s: f64 = v[4..].parse().ok()?;
    (h < 24.0 && m < 60.0 && s < 61.0).then_some(h * 3600.0 + m * 60.0 + s)
}

/// `ddmmyy` as days since 1970.
fn dmy(v: &str) -> Option<i64> {
    if v.len() != 6 || !v.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = |r: std::ops::Range<usize>| v[r].parse::<i64>().ok();
    let (d, m, y) = (n(0..2)?, n(2..4)?, n(4..6)?);
    if !(1..=31).contains(&d) || !(1..=12).contains(&m) {
        return None;
    }
    let y = if y < 80 { 2000 + y } else { 1900 + y };
    let yy = if m <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    Some(era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468)
}

/// What a log holds.
#[derive(Clone, Debug, Default)]
pub struct Log {
    pub fixes: Vec<Fix>,
    pub waypoints: Vec<(String, Pt)>,
    /// Sentences read, and those skipped for a wrong checksum.
    pub sentences: usize,
    pub bad: usize,
    /// Talker and sentence kinds seen with their counts (`GPRMC`), in first-seen order.
    pub kinds: Vec<(String, usize)>,
}

/// Reads the log.
pub fn parse(data: &[u8]) -> Log {
    let text = String::from_utf8_lossy(data);
    let mut log = Log::default();
    let mut day: Option<i64> = None;
    let mut last_secs: Option<f64> = None;
    for line in text.lines() {
        let l = line.trim();
        if !l.starts_with(['$', '!']) {
            continue;
        }
        let Some(s) = sentence(l) else {
            log.bad += 1;
            continue;
        };
        log.sentences += 1;
        let f: Vec<&str> = s.split(',').collect();
        let kind = f[0];
        match log.kinds.iter().position(|(k, _)| k == kind) {
            Some(i) => log.kinds[i].1 += 1,
            None if log.kinds.len() < 64 => log.kinds.push((kind.to_owned(), 1)),
            None => {}
        }
        let tail = kind.get(kind.len().saturating_sub(3)..).unwrap_or("");
        let get = |i: usize| f.get(i).copied().unwrap_or("");
        let mut dated_here = false;
        let fix = match tail {
            "RMC" if get(2) == "A" => {
                if let Some(d) = dmy(get(9)) {
                    day = Some(d);
                    dated_here = true;
                }
                coord(get(3), get(4), 2)
                    .zip(coord(get(5), get(6), 3))
                    .map(|(lat, lon)| (hms(get(1)), lat, lon, None, get(7).parse().ok()))
            }
            "GGA" if get(6).parse::<u32>().is_ok_and(|q| q > 0) => coord(get(2), get(3), 2)
                .zip(coord(get(4), get(5), 3))
                .map(|(lat, lon)| (hms(get(1)), lat, lon, get(9).parse().ok(), None)),
            "GLL" if get(6) != "V" => coord(get(1), get(2), 2)
                .zip(coord(get(3), get(4), 3))
                .map(|(lat, lon)| (hms(get(5)), lat, lon, None, None)),
            "WPL" => {
                if let (Some(lat), Some(lon)) = (coord(get(1), get(2), 2), coord(get(3), get(4), 3))
                    && log.waypoints.len() < 100_000
                {
                    log.waypoints.push((get(5).to_owned(), [lon, lat]));
                }
                None
            }
            _ => None,
        };
        let Some((secs, lat, lon, alt, speed)) = fix else {
            continue;
        };
        // A day passes when the clock goes back past midnight.
        if let (Some(s), Some(l), Some(d)) = (secs, last_secs, day.as_mut())
            && s + 3600.0 < l
            && !dated_here
        {
            *d += 1;
        }
        if secs.is_some() {
            last_secs = secs;
        }
        let t = secs.map(|s| s + day.map_or(0.0, |d| d as f64 * 86_400.0));
        // Sentences of one fix (RMC and GGA at the same second) make one point.
        if let Some(prev) = log.fixes.last_mut()
            && prev.t.is_some()
            && secs.is_some()
            && prev.t.map(|p| p.rem_euclid(86_400.0)) == secs
        {
            prev.alt = prev.alt.or(alt);
            prev.speed = prev.speed.or(speed);
            if prev.t.is_some_and(|p| p < 86_400.0) && day.is_some() {
                prev.t = t;
            }
            continue;
        }
        if log.fixes.len() >= MAX_VERTICES {
            break;
        }
        log.fixes.push(Fix {
            t,
            lon,
            lat,
            alt,
            speed,
        });
    }
    log
}

pub fn read(data: &[u8], name: &str) -> Result<Dataset, String> {
    let log = parse(data);
    if log.fixes.is_empty() && log.waypoints.is_empty() {
        return Err(if log.sentences == 0 && log.bad > 0 {
            "no NMEA sentence has a right checksum".into()
        } else {
            "no GPS positions".into()
        });
    }
    let mut ds = Dataset {
        format: "NMEA",
        crs: crs::from_epsg(4326),
        ..Dataset::default()
    };
    if log.bad > 0 {
        ds.warn(format!(
            "{} sentences with a wrong checksum were left out",
            log.bad
        ));
    }
    let mut track = Layer::new("track");
    track.declared_type = Some("LineString".into());
    if !log.fixes.is_empty() {
        let pts: Vec<Pt> = log.fixes.iter().map(|f| [f.lon, f.lat]).collect();
        let dated = log.fixes.iter().any(|f| f.t.is_some_and(|t| t >= 86_400.0));
        let time = |t: Option<f64>| match t {
            Some(t) if dated => Value::Date(iso(t)),
            Some(t) => Value::Text(format!(
                "{:02}:{:02}:{:02}",
                (t as i64) / 3600 % 24,
                (t as i64) / 60 % 60,
                (t as i64) % 60
            )),
            None => Value::Null,
        };
        let times: Vec<f64> = log.fixes.iter().filter_map(|f| f.t).collect();
        let (t0, t1) = (times.first().copied(), times.last().copied());
        let alts: Vec<f64> = log.fixes.iter().filter_map(|f| f.alt).collect();
        let speeds: Vec<f64> = log.fixes.iter().filter_map(|f| f.speed).collect();
        let fmax = |v: &[f64]| {
            v.iter()
                .copied()
                .reduce(f64::max)
                .map_or(Value::Null, Value::Float)
        };
        let fmin = |v: &[f64]| {
            v.iter()
                .copied()
                .reduce(f64::min)
                .map_or(Value::Null, Value::Float)
        };
        let kinds = log
            .kinds
            .iter()
            .map(|(k, n)| format!("{k} {n}"))
            .collect::<Vec<_>>()
            .join(", ");
        track.push(
            Some("0".into()),
            Some(Geometry::LineString(pts)),
            vec![
                ("name".into(), Value::Text(name.to_owned())),
                ("points".into(), Value::Int(log.fixes.len() as i64)),
                ("start".into(), time(t0)),
                ("end".into(), time(t1)),
                (
                    "duration_s".into(),
                    t0.zip(t1).map_or(Value::Null, |(a, b)| Value::Float(b - a)),
                ),
                ("min_alt_m".into(), fmin(&alts)),
                ("max_alt_m".into(), fmax(&alts)),
                ("max_speed_kn".into(), fmax(&speeds)),
                ("sentences".into(), Value::Text(kinds)),
            ],
        );
        ds.layers.push(track);
    }
    if !log.waypoints.is_empty() {
        let mut w = Layer::new("waypoints");
        w.declared_type = Some("Point".into());
        for (i, (n, p)) in log.waypoints.iter().enumerate() {
            w.push(
                Some(i.to_string()),
                Some(Geometry::Point(*p)),
                vec![("name".into(), Value::Text(n.clone()))],
            );
        }
        ds.layers.push(w);
    }
    Ok(ds)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sentence with its checksum.
    fn s(body: &str) -> String {
        let sum = body.bytes().fold(0u8, |a, b| a ^ b);
        format!("${body}*{sum:02X}\r\n")
    }

    #[test]
    fn log() {
        let mut t = String::new();
        t += &s("GPRMC,235959.00,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W");
        t += &s("GPGGA,235959.00,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,");
        t += &s("GNRMC,000001.00,A,4807.100,N,01131.100,E,010.0,084.4,240394,,");
        t += "$GPGGA,000002.00,4807.2,N,01131.2,E,1,08,0.9,545.4,M,46.9,M,,*00\r\n";
        t += &s("GPGLL,4807.300,S,01131.300,W,000003.00,A");
        t += &s("GPWPL,4807.038,N,01131.000,E,WPTNME");
        assert!(sniff(t.as_bytes()));
        assert!(!sniff(b"$GPRMC,1*00\nhello\n"));
        let l = parse(t.as_bytes());
        assert_eq!(l.bad, 1);
        assert_eq!(l.fixes.len(), 3);
        let a = l.fixes[0];
        assert!((a.lat - (48.0 + 7.038 / 60.0)).abs() < 1e-12);
        assert!((a.lon - (11.0 + 31.0 / 60.0)).abs() < 1e-12);
        assert_eq!(a.alt, Some(545.4));
        assert_eq!(a.speed, Some(22.4));
        assert_eq!(iso(a.t.unwrap()), "1994-03-23T23:59:59Z");
        assert_eq!(iso(l.fixes[1].t.unwrap()), "1994-03-24T00:00:01Z");
        assert!(l.fixes[2].lat < 0.0 && l.fixes[2].lon < 0.0);
        assert_eq!(iso(l.fixes[2].t.unwrap()), "1994-03-24T00:00:03Z");
        assert_eq!(l.waypoints[0].0, "WPTNME");
        let d = read(t.as_bytes(), "x").unwrap();
        assert_eq!(d.layers.len(), 2);
        let a: std::collections::HashMap<_, _> = d.layers[0].attributes(0).into_iter().collect();
        assert_eq!(a["duration_s"], Value::Float(4.0));
        assert_eq!(a["max_speed_kn"], Value::Float(22.4));
        assert_eq!(d.warnings.len(), 1);
    }
}
