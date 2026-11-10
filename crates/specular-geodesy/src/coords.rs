//! Coordinates as text: four display formats and one parser that reads them all (and the
//! usual ways people type positions).
//!
//! Text is locale-neutral: a full stop for decimals, ASCII digits, latitude first.

use crate::utm::{Utm, band, band_south, from_utm, to_utm};

/// How a position is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CoordFormat {
    /// Signed decimal degrees, latitude first: "47.376887, 8.541694".
    Decimal,
    /// Degrees, minutes and seconds: "47°22′36.79″N 8°32′30.10″E".
    Dms,
    /// UTM zone and band, easting, northing in metres: "32T 465402 5247149" (UPS: "Z …").
    Utm,
    /// MGRS/USNG to the metre: "32TMT 65402 47149".
    Mgrs,
}

impl CoordFormat {
    pub const ALL: [CoordFormat; 4] = [
        CoordFormat::Decimal,
        CoordFormat::Dms,
        CoordFormat::Utm,
        CoordFormat::Mgrs,
    ];

    /// The name in settings files.
    pub fn id(self) -> &'static str {
        match self {
            CoordFormat::Decimal => "dd",
            CoordFormat::Dms => "dms",
            CoordFormat::Utm => "utm",
            CoordFormat::Mgrs => "mgrs",
        }
    }

    pub fn from_id(s: &str) -> Option<CoordFormat> {
        CoordFormat::ALL.into_iter().find(|f| f.id() == s.trim())
    }

    /// The next format, round.
    pub fn next(self) -> CoordFormat {
        let i = CoordFormat::ALL
            .iter()
            .position(|f| *f == self)
            .unwrap_or(0);
        CoordFormat::ALL[(i + 1) % CoordFormat::ALL.len()]
    }
}

/// Why text is not a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    Unrecognised,
    /// A latitude past ±90° or a longitude past ±180°.
    OutOfRange,
}

/// `v` degrees as degrees, minutes and seconds with two decimals, carried so that seconds
/// never read 60.
fn dms(v: f64, pos: char, neg: char) -> String {
    let hemi = if v < 0.0 { neg } else { pos };
    let hundredths = (v.abs() * 360_000.0).round() as u64;
    let d = hundredths / 360_000;
    let m = (hundredths / 6000) % 60;
    let s = hundredths % 6000;
    format!("{d}°{m:02}′{:02}.{:02}″{hemi}", s / 100, s % 100)
}

/// A position as text in format `f`; `None` where the format has no reference (never for
/// valid input).
pub fn format(lat: f64, lon: f64, f: CoordFormat) -> Option<String> {
    if !(lat.is_finite() && lon.is_finite()) || lat.abs() > 90.0 {
        return None;
    }
    let lon = crate::angle::normalize(lon);
    match f {
        CoordFormat::Decimal => Some(format!("{:.6}, {:.6}", lat + 0.0, lon + 0.0)),
        CoordFormat::Dms => Some(format!("{} {}", dms(lat, 'N', 'S'), dms(lon, 'E', 'W'))),
        CoordFormat::Utm => {
            let u = to_utm(lat, lon)?;
            let (e, n) = (u.easting.round() as i64, u.northing.round() as i64);
            if u.zone == 0 {
                let letter = match (u.north, u.easting < 2_000_000.0) {
                    (true, true) => 'Y',
                    (true, false) => 'Z',
                    (false, true) => 'A',
                    (false, false) => 'B',
                };
                Some(format!("{letter} {e} {n}"))
            } else {
                Some(format!("{}{} {e} {n}", u.zone, band(lat)?))
            }
        }
        CoordFormat::Mgrs => crate::mgrs::to_mgrs(lat, lon, 5),
    }
}

/// A position from text: (latitude, longitude). Reads decimal degrees (signed, or with
/// N/S/E/W before or after), degrees and minutes, degrees minutes seconds (with °′″, '"
/// or spaces), UTM/UPS ("32T 465402 5247149", "32N …" for a hemisphere) and MGRS/USNG.
pub fn parse(s: &str) -> Result<(f64, f64), ParseError> {
    let t = s.trim();
    if t.is_empty() {
        return Err(ParseError::Empty);
    }
    if let Some(p) = crate::mgrs::from_mgrs(t) {
        return Ok(p);
    }
    if let Some(r) = parse_utm(t) {
        return r;
    }
    parse_degrees(t)
}

fn check(lat: f64, lon: f64) -> Result<(f64, f64), ParseError> {
    if !(lat.is_finite() && lon.is_finite()) {
        return Err(ParseError::Unrecognised);
    }
    if lat.abs() > 90.0 || lon.abs() > 180.0 {
        return Err(ParseError::OutOfRange);
    }
    Ok((lat, lon))
}

/// A number, allowing a trailing unit suffix (`m`, `E`, `N`, `mE`, `mN`).
fn utm_number(s: &str) -> Option<f64> {
    let t = s.trim_end_matches(['m', 'M', 'e', 'E', 'n', 'N']);
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    t.parse().ok()
}

fn parse_utm(t: &str) -> Option<Result<(f64, f64), ParseError>> {
    let toks: Vec<&str> = t
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|s| !s.is_empty())
        .collect();
    let (zone_band, rest): (String, &[&str]) = match toks.as_slice() {
        [zb, _, _] => ((*zb).to_owned(), &toks[1..]),
        [z, b, _, _] if b.len() == 1 => (format!("{z}{b}"), &toks[2..]),
        _ => return None,
    };
    let zb = zone_band.to_ascii_uppercase();
    let nd = zb.chars().take_while(char::is_ascii_digit).count();
    let letter = zb[nd..].chars().next()?;
    if zb.len() != nd + 1 || !letter.is_ascii_alphabetic() {
        return None;
    }
    let e = utm_number(rest[0])?;
    let n = utm_number(rest[1])?;
    if nd == 0 {
        if !matches!(letter, 'A' | 'B' | 'Y' | 'Z') {
            return None;
        }
        let u = Utm {
            zone: 0,
            north: matches!(letter, 'Y' | 'Z'),
            easting: e,
            northing: n,
        };
        return Some(match from_utm(&u) {
            Some((la, lo)) if (la > 0.0) == u.north => check(la, lo),
            _ => Err(ParseError::OutOfRange),
        });
    }
    let zone: u8 = zb[..nd].parse().ok()?;
    if !(1..=60).contains(&zone) {
        return Some(Err(ParseError::OutOfRange));
    }
    let south = band_south(letter);
    let try_hemi = |north: bool| {
        from_utm(&Utm {
            zone,
            north,
            easting: e,
            northing: n,
        })
    };
    if let Some(s0) = south {
        // A latitude band: the position must lie in it (half a degree of slack).
        let top = if letter == 'X' { s0 + 12.0 } else { s0 + 8.0 };
        if let Some((la, lo)) = try_hemi(letter >= 'N')
            && la >= s0 - 0.5
            && la <= top + 0.5
        {
            return Some(check(la, lo));
        }
    }
    // N or S for the hemisphere.
    match letter {
        'N' | 'S' => Some(match try_hemi(letter == 'N') {
            Some((la, lo)) => check(la, lo),
            None => Err(ParseError::OutOfRange),
        }),
        _ if south.is_some() => Some(Err(ParseError::OutOfRange)),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tok {
    Num(f64, bool),
    Hemi(char),
    Deg,
    Min,
    Sec,
    Sep,
}

fn tokenize(t: &str) -> Option<Vec<Tok>> {
    let mut out = Vec::new();
    let cs: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        match c {
            c if c.is_whitespace() => i += 1,
            ',' | ';' | '/' => {
                out.push(Tok::Sep);
                i += 1;
            }
            '°' | 'º' | '˚' => {
                out.push(Tok::Deg);
                i += 1;
            }
            '\'' | '′' | '’' => {
                // Two single quotes are seconds.
                if cs.get(i + 1).is_some_and(|n| matches!(n, '\'' | '′' | '’')) {
                    out.push(Tok::Sec);
                    i += 2;
                } else {
                    out.push(Tok::Min);
                    i += 1;
                }
            }
            '"' | '″' | '”' => {
                out.push(Tok::Sec);
                i += 1;
            }
            'N' | 'S' | 'E' | 'W' | 'n' | 's' | 'e' | 'w' => {
                out.push(Tok::Hemi(c.to_ascii_uppercase()));
                i += 1;
            }
            'd' | 'D' if !out.is_empty() => {
                out.push(Tok::Deg);
                i += 1;
            }
            '-' | '+' | '−' | '.' | '0'..='9' => {
                let neg = matches!(c, '-' | '−');
                let mut j = if matches!(c, '-' | '+' | '−') {
                    i + 1
                } else {
                    i
                };
                let start = j;
                while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == '.') {
                    j += 1;
                }
                if j == start {
                    return None;
                }
                let v: f64 = cs[start..j].iter().collect::<String>().parse().ok()?;
                out.push(Tok::Num(if neg { -v } else { v }, neg));
                i = j;
            }
            _ => return None,
        }
    }
    Some(out)
}

/// One coordinate: optional hemisphere before or after, then 1–3 numbers (degrees, minutes,
/// seconds, by their symbols or their order). Returns the value and the hemisphere letter.
fn coordinate(toks: &[Tok]) -> Option<(f64, Option<char>)> {
    let mut hemi = None;
    let mut nums: Vec<(f64, bool, Option<Tok>)> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match toks[i] {
            Tok::Hemi(h) => {
                if hemi.is_some() {
                    return None;
                }
                hemi = Some(h);
            }
            Tok::Num(v, neg) => {
                let sym = toks
                    .get(i + 1)
                    .copied()
                    .filter(|t| matches!(t, Tok::Deg | Tok::Min | Tok::Sec));
                if sym.is_some() {
                    i += 1;
                }
                nums.push((v, neg, sym));
            }
            _ => return None,
        }
        i += 1;
    }
    if nums.is_empty() || nums.len() > 3 {
        return None;
    }
    // Only the first number may carry a sign, and not together with a hemisphere.
    if nums.iter().skip(1).any(|n| n.1) || (nums[0].1 && hemi.is_some()) {
        return None;
    }
    let mut parts = [None::<f64>; 3];
    for (k, (v, _, sym)) in nums.iter().enumerate() {
        let slot = match sym {
            Some(Tok::Deg) => 0,
            Some(Tok::Min) => 1,
            Some(Tok::Sec) => 2,
            _ => k,
        };
        if parts[slot].is_some() {
            return None;
        }
        parts[slot] = Some(v.abs());
    }
    let d = parts[0]?;
    let m = parts[1].unwrap_or(0.0);
    let s = parts[2].unwrap_or(0.0);
    if m >= 60.0 || s >= 60.0 || (parts[2].is_some() && parts[1].is_none()) {
        return None;
    }
    // Fractions only on the last part given.
    let last = parts.iter().rposition(Option::is_some)?;
    if parts[..last].iter().flatten().any(|v| v.fract() != 0.0) {
        return None;
    }
    let v = d + m / 60.0 + s / 3600.0;
    let neg = nums[0].1 || matches!(hemi, Some('S' | 'W'));
    Some((if neg { -v } else { v }, hemi))
}

fn parse_degrees(t: &str) -> Result<(f64, f64), ParseError> {
    let toks = tokenize(t).ok_or(ParseError::Unrecognised)?;
    let seps: Vec<usize> = toks
        .iter()
        .enumerate()
        .filter(|(_, t)| **t == Tok::Sep)
        .map(|(i, _)| i)
        .collect();
    let hemis: Vec<usize> = toks
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t, Tok::Hemi(_)))
        .map(|(i, _)| i)
        .collect();
    let split = if seps.len() == 1 {
        Some((seps[0], seps[0] + 1))
    } else if !seps.is_empty() {
        None
    } else if hemis.len() == 2 {
        if hemis[0] == 0 {
            // Letters first: the second group starts at the second letter.
            Some((hemis[1], hemis[1]))
        } else {
            // Letters after: the first group ends at the first letter.
            Some((hemis[0] + 1, hemis[0] + 1))
        }
    } else if hemis.is_empty() {
        let degs: Vec<usize> = toks
            .iter()
            .enumerate()
            .filter(|(_, t)| **t == Tok::Deg)
            .map(|(i, _)| i)
            .collect();
        if degs.len() == 2 {
            // The second group starts at the number before the second degree sign.
            Some((degs[1] - 1, degs[1] - 1))
        } else {
            let nums = toks.iter().filter(|t| matches!(t, Tok::Num(..))).count();
            let all_nums = toks.iter().all(|t| matches!(t, Tok::Num(..)));
            if all_nums && matches!(nums, 2 | 4 | 6) {
                Some((nums / 2, nums / 2))
            } else {
                None
            }
        }
    } else {
        None
    };
    let (end_a, start_b) = split.ok_or(ParseError::Unrecognised)?;
    if end_a == 0 || start_b >= toks.len() {
        return Err(ParseError::Unrecognised);
    }
    let a = coordinate(&toks[..end_a]).ok_or(ParseError::Unrecognised)?;
    let b = coordinate(&toks[start_b..]).ok_or(ParseError::Unrecognised)?;
    let is_lon = |h: Option<char>| matches!(h, Some('E' | 'W'));
    let is_lat = |h: Option<char>| matches!(h, Some('N' | 'S'));
    let (lat, lon) = match (a.1, b.1) {
        (None, None) => (a.0, b.0),
        (ha, hb) if is_lat(ha) && !is_lat(hb) && (hb.is_none() || is_lon(hb)) => (a.0, b.0),
        (ha, hb) if is_lon(ha) && !is_lon(hb) && (hb.is_none() || is_lat(hb)) => (b.0, a.0),
        (None, hb) if is_lon(hb) => (a.0, b.0),
        (None, hb) if is_lat(hb) => (b.0, a.0),
        _ => return Err(ParseError::Unrecognised),
    };
    check(lat, lon)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(p: Result<(f64, f64), ParseError>, lat: f64, lon: f64, tol: f64) {
        let (la, lo) = p.unwrap_or_else(|e| panic!("{e:?}"));
        assert!(
            (la - lat).abs() < tol && (lo - lon).abs() < tol,
            "{la} {lo} vs {lat} {lon}"
        );
    }

    #[test]
    fn formats() {
        let (lat, lon) = (47.376887, 8.541694);
        assert_eq!(
            format(lat, lon, CoordFormat::Decimal).unwrap(),
            "47.376887, 8.541694"
        );
        assert_eq!(
            format(lat, lon, CoordFormat::Dms).unwrap(),
            "47°22′36.79″N 8°32′30.10″E"
        );
        assert_eq!(
            format(-0.000001, -0.0, CoordFormat::Dms).unwrap(),
            "0°00′00.00″S 0°00′00.00″E"
        );
        // 59.999″ carries into the minute.
        assert_eq!(
            format(10.0 + 59.999 / 3600.0, 0.0, CoordFormat::Dms).unwrap(),
            "10°01′00.00″N 0°00′00.00″E"
        );
        assert_eq!(
            format(lat, lon, CoordFormat::Mgrs).unwrap(),
            "32TMT 65402 47149"
        );
        let utm = format(lat, lon, CoordFormat::Utm).unwrap();
        assert!(
            utm.starts_with("32T 46540") && utm.contains(" 524714"),
            "{utm}"
        );
        assert!(
            format(89.0, 0.0, CoordFormat::Utm)
                .unwrap()
                .starts_with("Z ")
        );
        assert_eq!(CoordFormat::Mgrs.next(), CoordFormat::Decimal);
        assert_eq!(CoordFormat::from_id("utm"), Some(CoordFormat::Utm));
    }

    #[test]
    fn parses_every_spelling() {
        let (lat, lon) = (47.376887, 8.541694);
        for s in [
            "47.376887, 8.541694",
            "47.376887 8.541694",
            "47.376887;8.541694",
            "47.376887N 8.541694E",
            "N47.376887 E8.541694",
            "8.541694E, 47.376887N",
            "47.376887 n, 8.541694 e",
            "47°22′36.79″N 8°32′30.10″E",
            "47°22'36.79\"N 8°32'30.10\"E",
            "47 22 36.79 N 8 32 30.10 E",
            "47 22 36.79, 8 32 30.10",
            "47°22′36.79″ 8°32′30.10″",
            "47°22.61322′N 8°32.50164′E",
            "32T 465402 5247149",
            "32 T 465402mE 5247149mN",
            "32N 465402 5247149",
            "32TMT6540247149",
            "32T MT 65402 47149",
        ] {
            near(parse(s), lat, lon, 2e-5);
        }
        near(parse("-33.86, 151.21"), -33.86, 151.21, 1e-12);
        near(parse("33.86S 151.21E"), -33.86, 151.21, 1e-12);
        near(parse("W 77.0365 N 38.8977"), 38.8977, -77.0365, 1e-12);
        // "S" not fitting band S: the southern hemisphere.
        near(parse("56S 334369 6250948"), -33.86, 151.21, 2e-2);
        near(parse("Z 2000000 2000000"), 90.0, 0.0, 1e-9);
        assert_eq!(parse("  "), Err(ParseError::Empty));
        assert_eq!(parse("hello"), Err(ParseError::Unrecognised));
        assert_eq!(parse("91, 10"), Err(ParseError::OutOfRange));
        assert_eq!(parse("10, 181"), Err(ParseError::OutOfRange));
        assert_eq!(parse("47 61 0 N 8 0 0 E"), Err(ParseError::Unrecognised));
        assert_eq!(parse("47N 8N"), Err(ParseError::Unrecognised));
    }
}
