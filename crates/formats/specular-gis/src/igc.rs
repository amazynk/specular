//! IGC flight logs (FAI gliding and paragliding loggers): the A record names the logger,
//! H records the date, pilot and glider, B records are fixes (UTC time, latitude and
//! longitude in degrees and thousandths of minutes, validity, pressure and GNSS altitude),
//! C records the declared task. Read as two layers: the flight as one line with its
//! summary, and the task's turnpoints.

use crate::crs;
use crate::model::{Dataset, Geometry, Layer, MAX_VERTICES, Pt, Value};

/// One fix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fix {
    /// Seconds since 1970 (UTC), when the file gives the date; else since midnight.
    pub t: f64,
    pub lon: f64,
    pub lat: f64,
    /// `A` (3-D fix) rather than `V` (2-D or none).
    pub valid: bool,
    pub pressure_alt: Option<f64>,
    pub gnss_alt: Option<f64>,
}

/// Whether the bytes start an IGC file: an A record, then H or B records.
pub fn sniff(head: &[u8]) -> bool {
    let h = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    if h.len() < 8 || h[0] != b'A' || !h[1..4].iter().all(u8::is_ascii_alphanumeric) {
        return false;
    }
    let t = String::from_utf8_lossy(&h[..h.len().min(4096)]);
    let mut lines = t.lines().skip(1).filter(|l| !l.is_empty());
    lines
        .next()
        .is_some_and(|l| l.starts_with("HF") || l.starts_with("HO") || l.starts_with("HP"))
}

/// Days since 1970-01-01 of a civil date.
fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn digits(s: &str) -> Option<i64> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

/// `DDMMmmmN` / `DDDMMmmmE` as degrees.
fn coord(s: &str, deg: usize) -> Option<f64> {
    if s.len() != deg + 6 {
        return None;
    }
    let d = digits(&s[..deg])? as f64;
    let m = digits(&s[deg..deg + 5])? as f64 / 1000.0;
    let v = d + m / 60.0;
    match &s[deg + 5..] {
        "N" | "E" => Some(v),
        "S" | "W" => Some(-v),
        _ => None,
    }
}

/// The date of an `HFDTE` record: `HFDTEddmmyy` or `HFDTEDATE:ddmmyy,nn`.
fn date(line: &str) -> Option<i64> {
    let v = line.get(5..)?;
    let v = v.split_once(':').map_or(v, |(_, r)| r).trim();
    let v = v.get(..6)?;
    let (d, m, y) = (digits(&v[..2])?, digits(&v[2..4])?, digits(&v[4..6])?);
    if !(1..=31).contains(&d) || !(1..=12).contains(&m) {
        return None;
    }
    // Two-digit years: the IGC format dates from 1990s.
    let y = if y < 80 { 2000 + y } else { 1900 + y };
    Some(days(y, m, d))
}

/// A B record.
fn fix(line: &str) -> Option<(i64, Fix)> {
    if line.len() < 35 || !line.is_ascii() {
        return None;
    }
    let (h, m, s) = (
        digits(&line[1..3])?,
        digits(&line[3..5])?,
        digits(&line[5..7])?,
    );
    if h > 23 || m > 59 || s > 60 {
        return None;
    }
    let lat = coord(&line[7..15], 2)?;
    let lon = coord(&line[15..24], 3)?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    let alt = |r: std::ops::Range<usize>| {
        let a = line.get(r)?.trim().parse::<f64>().ok()?;
        Some(a)
    };
    Some((
        h * 3600 + m * 60 + s,
        Fix {
            t: 0.0,
            lon,
            lat,
            valid: &line[24..25] == "A",
            pressure_alt: alt(25..30),
            gnss_alt: alt(30..35),
        },
    ))
}

/// A flight log's facts.
#[derive(Clone, Debug, Default)]
pub struct Flight {
    pub fixes: Vec<Fix>,
    /// Whether fix times are on a date (else seconds since midnight).
    pub dated: bool,
    /// H record facts (`pilot`, `glider type`…) in file order.
    pub headers: Vec<(String, String)>,
    pub logger: Option<String>,
    /// Task turnpoints: name and position.
    pub task: Vec<(String, Pt)>,
}

/// The name of an H record's subject (`HFPLTPILOTINCHARGE:` → "pilot").
fn header_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "DTE" => "date",
        "PLT" => "pilot",
        "CM2" => "crew 2",
        "GTY" => "glider type",
        "GID" => "glider id",
        "CID" => "competition id",
        "CCL" => "competition class",
        "SIT" => "site",
        "FTY" => "logger type",
        "RFW" => "firmware",
        "RHW" => "hardware",
        "GPS" => "gps receiver",
        "PRS" => "pressure sensor",
        "DTM" => "datum",
        "ALG" => "gnss altitude",
        "ALP" => "pressure altitude",
        "TZN" => "time zone",
        "FXA" => "fix accuracy",
        _ => return None,
    })
}

/// Reads the log.
pub fn parse(data: &[u8]) -> Result<Flight, String> {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let text = String::from_utf8_lossy(data);
    let mut f = Flight::default();
    let mut day: Option<i64> = None;
    let mut last: Option<i64> = None;
    let mut rollover = 0i64;
    let mut c_lines = 0usize;
    for line in text.lines() {
        let line = line.trim_end();
        match line.as_bytes().first() {
            Some(b'A') if f.logger.is_none() => f.logger = Some(line[1..].trim().to_owned()),
            Some(b'H') if line.len() > 5 => {
                let code = line.get(2..5).unwrap_or("");
                if code == "DTE" && day.is_none() {
                    day = date(line);
                }
                if let Some(name) = header_name(code) {
                    let v = line[5..].split_once(':').map_or(&line[5..], |(_, r)| r);
                    let v = v.trim();
                    if !v.is_empty() && f.headers.len() < 64 {
                        f.headers.push((name.to_owned(), v.to_owned()));
                    }
                }
            }
            Some(b'B') => {
                if f.fixes.len() >= MAX_VERTICES {
                    break;
                }
                if let Some((secs, mut x)) = fix(line) {
                    // Past midnight UTC the clock starts again: a day later.
                    if last.is_some_and(|l| secs + 3600 < l) {
                        rollover += 86_400;
                    }
                    last = Some(secs);
                    x.t = (secs + rollover + day.unwrap_or(0) * 86_400) as f64;
                    f.fixes.push(x);
                }
            }
            Some(b'C') => {
                // The first C record is the declaration's date and time; then turnpoints.
                c_lines += 1;
                if c_lines > 1
                    && line.len() >= 18
                    && line.is_ascii()
                    && let (Some(lat), Some(lon)) = (coord(&line[1..9], 2), coord(&line[9..18], 3))
                    && f.task.len() < 1000
                {
                    f.task.push((line[18..].trim().to_owned(), [lon, lat]));
                }
            }
            _ => {}
        }
    }
    f.dated = day.is_some();
    if f.fixes.is_empty() && f.task.is_empty() {
        return Err("no fixes".into());
    }
    Ok(f)
}

/// An ISO 8601 time of seconds since 1970.
pub fn iso(t: f64) -> String {
    let s = t.floor() as i64;
    let (d, r) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    // Civil date from days (the inverse of `days`).
    let z = d + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        r / 3600,
        r / 60 % 60,
        r % 60
    )
}

pub fn read(data: &[u8], name: &str) -> Result<Dataset, String> {
    let f = parse(data)?;
    let mut ds = Dataset {
        format: "IGC",
        crs: crs::from_epsg(4326),
        ..Dataset::default()
    };
    let mut track = Layer::new("flight");
    track.declared_type = Some("LineString".into());
    if !f.fixes.is_empty() {
        let pts: Vec<Pt> = f.fixes.iter().map(|x| [x.lon, x.lat]).collect();
        let time = |t: f64| {
            if f.dated {
                Value::Date(iso(t))
            } else {
                Value::Text(format!(
                    "{:02}:{:02}:{:02}",
                    (t as i64) / 3600 % 24,
                    (t as i64) / 60 % 60,
                    (t as i64) % 60
                ))
            }
        };
        let mut attrs = vec![("name".to_owned(), Value::Text(name.to_owned()))];
        for (k, v) in &f.headers {
            if k != "date" {
                attrs.push((k.clone(), Value::Text(v.clone())));
            }
        }
        if let Some(l) = &f.logger {
            attrs.push(("logger".into(), Value::Text(l.clone())));
        }
        let t0 = f.fixes[0].t;
        let t1 = f.fixes[f.fixes.len() - 1].t;
        attrs.push(("points".into(), Value::Int(pts.len() as i64)));
        attrs.push(("start".into(), time(t0)));
        attrs.push(("end".into(), time(t1)));
        attrs.push(("duration_s".into(), Value::Float(t1 - t0)));
        let max = |a: &dyn Fn(&Fix) -> Option<f64>| {
            f.fixes
                .iter()
                .filter_map(a)
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
                .map_or(Value::Null, Value::Float)
        };
        attrs.push((
            "max_gnss_alt_m".into(),
            max(&|x| x.gnss_alt.filter(|a| *a != 0.0)),
        ));
        attrs.push((
            "max_pressure_alt_m".into(),
            max(&|x| x.pressure_alt.filter(|a| *a != 0.0)),
        ));
        track.push(Some("0".into()), Some(Geometry::LineString(pts)), attrs);
    }
    let mut task = Layer::new("task");
    task.declared_type = Some("Point".into());
    for (i, (n, p)) in f.task.iter().enumerate() {
        task.push(
            Some(i.to_string()),
            Some(Geometry::Point(*p)),
            vec![
                ("order".into(), Value::Int(i as i64 + 1)),
                ("name".into(), Value::Text(n.clone())),
            ],
        );
    }
    ds.layers.push(track);
    if !task.features.is_empty() {
        ds.layers.push(task);
    }
    Ok(ds)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "AXCSAAA Logger\r\nHFDTE160701\r\nHFPLTPILOTINCHARGE: Jo Bloggs\r\nHFGTYGLIDERTYPE:LS8\r\nC160701090000000000000102\r\nC5111359N00101899WTAKEOFF\r\nC5110179N00102644WTURN 1\r\nB2359595107500N00130106WA0010200110\r\nB0000055107600N00130200WA0010500120\r\nLXXX comment\r\nB0000155107700S00130300EV0011000130\r\nGABCDEF\r\n";

    #[test]
    fn flight() {
        assert!(sniff(LOG.as_bytes()));
        assert!(!sniff(b"ABC\nnot igc"));
        let f = parse(LOG.as_bytes()).unwrap();
        assert!(f.dated);
        assert_eq!(f.fixes.len(), 3);
        let a = f.fixes[0];
        assert!((a.lat - (51.0 + 7.5 / 60.0)).abs() < 1e-12);
        assert!((a.lon + (1.0 + 30.106 / 60.0)).abs() < 1e-12);
        assert_eq!(a.pressure_alt, Some(102.0));
        assert_eq!(a.gnss_alt, Some(110.0));
        // Midnight passed: the next fixes are on the 17th.
        assert_eq!(iso(a.t), "2001-07-16T23:59:59Z");
        assert_eq!(iso(f.fixes[1].t), "2001-07-17T00:00:05Z");
        assert!(f.fixes[2].lat < 0.0 && f.fixes[2].lon > 0.0 && !f.fixes[2].valid);
        assert_eq!(f.task.len(), 2);
        assert_eq!(f.task[1].0, "TURN 1");
        let d = read(LOG.as_bytes(), "x").unwrap();
        let a: std::collections::HashMap<_, _> = d.layers[0].attributes(0).into_iter().collect();
        assert_eq!(a["pilot"], Value::Text("Jo Bloggs".into()));
        assert_eq!(a["glider type"], Value::Text("LS8".into()));
        assert_eq!(a["duration_s"], Value::Float(16.0));
        assert_eq!(a["max_gnss_alt_m"], Value::Float(130.0));
        assert_eq!(d.layers[1].features.len(), 2);
        assert_eq!(days(1970, 1, 1), 0);
        assert_eq!(iso(0.0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(951_782_400.0), "2000-02-29T00:00:00Z");
    }
}
