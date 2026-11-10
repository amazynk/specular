//! Garmin FIT activity files: the 12- or 14-byte header (".FIT"), definition and data
//! messages with local message types, compressed-timestamp record headers, developer
//! fields (stepped over), both byte orders and the base types' invalid values. Several
//! FIT files back to back (chained) are read in turn. The file CRC is checked; a mismatch
//! is a warning, not a refusal.
//!
//! Read: file_id (file type, manufacturer), session, lap and record messages. Each
//! session becomes one track feature with a summary (sport, times, distance, heart rate,
//! cadence, temperature, ascent, calories, device); laps become a second layer. Records
//! without a position are counted, not drawn. A file without any position (settings,
//! monitoring) is refused.
//!
//! Also the summary helpers TCX and GPX share: [`Summary`], [`iso_utc`], [`parse_iso`].

use crate::crs;
use crate::model::{Dataset, Geometry, Layer, MAX_FEATURES, Pt, Value};

/// Seconds from the Unix epoch to the FIT epoch (1989-12-31T00:00:00Z).
const FIT_EPOCH: i64 = 631_065_600;
/// Records read at most.
const MAX_RECORDS: usize = 50_000_000;

pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 12 && matches!(head[0], 12 | 14) && &head[8..12] == b".FIT"
}

// ------------------------------------------------------------------- shared summaries

/// Days since 1970-01-01 to a proleptic Gregorian (year, month, day).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from((m + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Unix seconds as ISO 8601 UTC ("2024-05-01T10:00:00Z").
pub fn iso_utc(unix: i64) -> String {
    let (days, sod) = (unix.div_euclid(86_400), unix.rem_euclid(86_400));
    let (y, m, d) = civil(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        sod / 3600,
        sod / 60 % 60,
        sod % 60
    )
}

/// An ISO 8601 date-time ("2024-05-01T10:00:00.5Z", "+02:00" offsets, no zone = UTC) as
/// Unix seconds.
pub fn parse_iso(s: &str) -> Option<f64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't' | b' ') {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 24 || mi > 59 || sec > 60 {
        return None;
    }
    let mut i = 19;
    let mut frac = 0.0;
    if b.get(i) == Some(&b'.') {
        let st = i + 1;
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac = format!("0.{}", &s[st..i]).parse().unwrap_or(0.0);
    }
    let mut off = 0i64;
    match b.get(i) {
        Some(b'+' | b'-') => {
            let sign = if b[i] == b'-' { -1 } else { 1 };
            let oh = num(i + 1..i + 3)?;
            let om = match b.get(i + 3) {
                Some(b':') => num(i + 4..i + 6).unwrap_or(0),
                _ => num(i + 3..i + 5).unwrap_or(0),
            };
            off = sign * (oh * 3600 + om * 60);
        }
        Some(b'Z' | b'z') | None => {}
        _ => return None,
    }
    let days = days_from_civil(y, mo as u32, d as u32);
    Some((days * 86_400 + h * 3600 + mi * 60 + sec - off) as f64 + frac)
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Stat {
    n: usize,
    sum: f64,
    min: f64,
    max: f64,
}

impl Stat {
    pub(crate) fn add(&mut self, v: f64) {
        if !v.is_finite() {
            return;
        }
        if self.n == 0 {
            self.min = v;
            self.max = v;
        } else {
            self.min = self.min.min(v);
            self.max = self.max.max(v);
        }
        self.n += 1;
        self.sum += v;
    }
    fn avg(&self) -> Option<f64> {
        (self.n > 0).then(|| self.sum / self.n as f64)
    }
    fn min(&self) -> Option<f64> {
        (self.n > 0).then_some(self.min)
    }
    fn max(&self) -> Option<f64> {
        (self.n > 0).then_some(self.max)
    }
}

/// What a track says about itself: what the file states, else what its points give.
#[derive(Clone, Debug, Default)]
pub(crate) struct Summary {
    pub sport: Option<String>,
    /// Unix seconds.
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub duration: Option<f64>,
    pub distance: Option<f64>,
    pub points: usize,
    pub hr: Stat,
    pub cad: Stat,
    pub temp: Stat,
    pub avg_hr: Option<f64>,
    pub max_hr: Option<f64>,
    pub avg_cad: Option<f64>,
    pub ascent: Option<f64>,
    pub calories: Option<f64>,
    pub device: Option<String>,
    /// Time of each point, to find start and end when the file does not say.
    pub first_time: Option<f64>,
    pub last_time: Option<f64>,
}

/// The summary fields, in order (shared by FIT, TCX and GPX tracks).
pub(crate) const SUMMARY_FIELDS: &[&str] = &[
    "sport",
    "start",
    "end",
    "duration_s",
    "distance_m",
    "points",
    "avg_hr",
    "max_hr",
    "avg_cadence",
    "min_temp",
    "avg_temp",
    "max_temp",
    "ascent_m",
    "calories",
    "device",
];

fn round(v: f64, k: f64) -> f64 {
    (v * k).round() / k
}

impl Summary {
    pub(crate) fn time(&mut self, t: f64) {
        if !t.is_finite() {
            return;
        }
        self.first_time = Some(self.first_time.map_or(t, |f| f.min(t)));
        self.last_time = Some(self.last_time.map_or(t, |f| f.max(t)));
    }

    /// The fields named in `fields` (a subset of [`SUMMARY_FIELDS`]).
    pub(crate) fn attrs(&self, fields: &[&str]) -> Vec<(String, Value)> {
        let f = |v: Option<f64>| {
            v.filter(|v| v.is_finite())
                .map_or(Value::Null, Value::Float)
        };
        let start = self.start.or(self.first_time);
        let end = self
            .end
            .or_else(|| Some(start? + self.duration?))
            .or(self.last_time);
        let duration = self.duration.or_else(|| Some(end? - start?));
        fields
            .iter()
            .map(|&name| {
                let v = match name {
                    "sport" => self.sport.clone().map_or(Value::Null, Value::Text),
                    "start" => {
                        start.map_or(Value::Null, |t| Value::Date(iso_utc(t.floor() as i64)))
                    }
                    "end" => end.map_or(Value::Null, |t| Value::Date(iso_utc(t.floor() as i64))),
                    "duration_s" => f(duration.map(|d| round(d, 1000.0))),
                    "distance_m" => f(self.distance.map(|d| round(d, 100.0))),
                    "points" => Value::Int(self.points as i64),
                    "avg_hr" => f(self.avg_hr.or(self.hr.avg()).map(|v| round(v, 10.0))),
                    "max_hr" => f(self.max_hr.or(self.hr.max())),
                    "avg_cadence" => f(self.avg_cad.or(self.cad.avg()).map(|v| round(v, 10.0))),
                    "min_temp" => f(self.temp.min()),
                    "avg_temp" => f(self.temp.avg().map(|v| round(v, 10.0))),
                    "max_temp" => f(self.temp.max()),
                    "ascent_m" => f(self.ascent),
                    "calories" => f(self.calories),
                    "device" => self.device.clone().map_or(Value::Null, Value::Text),
                    _ => Value::Null,
                };
                (name.to_string(), v)
            })
            .collect()
    }
}

/// A layer with the summary fields declared.
pub(crate) fn summary_layer(name: &str, fields: &[&str], ty: &str) -> Layer {
    let mut l = Layer::new(name);
    for f in fields {
        l.field(f);
    }
    l.declared_type = Some(ty.to_string());
    l
}

// ---------------------------------------------------------------------------- FIT

/// The FIT CRC-16 (reflected, polynomial 0xA001).
fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &b in data {
        crc ^= u16::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

#[derive(Clone, Debug)]
struct FieldDef {
    num: u8,
    size: usize,
    base: u8,
}

#[derive(Clone, Debug)]
struct Def {
    global: u16,
    big: bool,
    fields: Vec<FieldDef>,
    /// Total bytes of the developer fields.
    dev_size: usize,
}

/// A field's first value, `None` when it holds the base type's invalid value.
fn value(b: &[u8], base: u8, big: bool) -> Option<f64> {
    macro_rules! rd {
        ($t:ty, $n:expr) => {{
            let a: [u8; $n] = b.get(..$n)?.try_into().ok()?;
            if big {
                <$t>::from_be_bytes(a)
            } else {
                <$t>::from_le_bytes(a)
            }
        }};
    }
    Some(match base & 0x1F {
        0 | 2 | 13 => {
            let v = *b.first()?;
            (v != 0xFF).then_some(f64::from(v))?
        }
        10 => {
            let v = *b.first()?;
            (v != 0).then_some(f64::from(v))?
        }
        1 => {
            let v = *b.first()? as i8;
            (v != 0x7F).then_some(f64::from(v))?
        }
        3 => {
            let v = rd!(i16, 2);
            (v != 0x7FFF).then_some(f64::from(v))?
        }
        4 => {
            let v = rd!(u16, 2);
            (v != 0xFFFF).then_some(f64::from(v))?
        }
        11 => {
            let v = rd!(u16, 2);
            (v != 0).then_some(f64::from(v))?
        }
        5 => {
            let v = rd!(i32, 4);
            (v != 0x7FFF_FFFF).then_some(f64::from(v))?
        }
        6 => {
            let v = rd!(u32, 4);
            (v != 0xFFFF_FFFF).then_some(f64::from(v))?
        }
        12 => {
            let v = rd!(u32, 4);
            (v != 0).then_some(f64::from(v))?
        }
        8 => {
            let v = rd!(u32, 4);
            (v != 0xFFFF_FFFF).then_some(f64::from(f32::from_bits(v)))?
        }
        9 => {
            let v = rd!(u64, 8);
            (v != u64::MAX).then_some(f64::from_bits(v))?
        }
        14 => {
            let v = rd!(i64, 8);
            (v != i64::MAX).then_some(v as f64)?
        }
        15 => {
            let v = rd!(u64, 8);
            (v != u64::MAX).then_some(v as f64)?
        }
        16 => {
            let v = rd!(u64, 8);
            (v != 0).then_some(v as f64)?
        }
        _ => return None,
    })
}

/// One decoded data message: (field number, value) pairs and its timestamp.
struct Msg {
    global: u16,
    fields: Vec<(u8, f64)>,
}

impl Msg {
    fn get(&self, n: u8) -> Option<f64> {
        self.fields.iter().find(|(k, _)| *k == n).map(|(_, v)| *v)
    }
}

#[derive(Clone, Copy, Debug)]
struct Rec {
    /// Unix seconds.
    t: Option<f64>,
    pos: Option<Pt>,
    hr: Option<f64>,
    cad: Option<f64>,
    temp: Option<f64>,
}

#[derive(Default)]
struct Parsed {
    file_type: Option<u8>,
    manufacturer: Option<u16>,
    sessions: Vec<Msg>,
    laps: Vec<Msg>,
    records: Vec<Rec>,
    warnings: Vec<String>,
}

impl Parsed {
    fn warn(&mut self, w: impl Into<String>) {
        let w = w.into();
        if !self.warnings.contains(&w) && self.warnings.len() < 16 {
            self.warnings.push(w);
        }
    }
}

/// Reads one FIT file at `at`; returns where the next one would start.
fn parse_one(data: &[u8], at: usize, out: &mut Parsed) -> Result<usize, String> {
    let head = data.get(at..).ok_or("truncated")?;
    if !sniff(head) {
        return Err("not a FIT file".into());
    }
    let hsize = usize::from(head[0]);
    if head.len() < hsize {
        return Err("the FIT header is truncated".into());
    }
    let size = u32::from_le_bytes(head[4..8].try_into().unwrap_or_default()) as usize;
    let avail = head.len().saturating_sub(hsize);
    let body_len = if size == 0 || size > avail {
        if size > avail {
            out.warn("the FIT file is truncated; the rest is not shown");
        }
        avail.saturating_sub(if size == 0 { 2 } else { 0 })
    } else {
        size
    };
    let body = &head[hsize..hsize + body_len];
    if size > 0 && size + 2 <= avail {
        let stored = u16::from_le_bytes([head[hsize + size], head[hsize + size + 1]]);
        if stored != 0 && crc16(&head[..hsize + size]) != stored {
            out.warn("the FIT file's checksum does not match; it may be damaged");
        }
    }
    let mut defs: [Option<Def>; 16] = Default::default();
    let mut last_ts: Option<u32> = None;
    let mut i = 0usize;
    while i < body.len() {
        let h = body[i];
        i += 1;
        let (local, compressed_ts) = if h & 0x80 != 0 {
            (usize::from((h >> 5) & 3), Some(u32::from(h & 0x1F)))
        } else {
            (usize::from(h & 0x0F), None)
        };
        if compressed_ts.is_none() && h & 0x40 != 0 {
            // Definition message.
            let Some(d) = body.get(i..i + 5) else {
                out.warn("the FIT file is truncated; the rest is not shown");
                break;
            };
            let big = d[1] == 1;
            let global = if big {
                u16::from_be_bytes([d[2], d[3]])
            } else {
                u16::from_le_bytes([d[2], d[3]])
            };
            let n = usize::from(d[4]);
            i += 5;
            let Some(fb) = body.get(i..i + 3 * n) else {
                out.warn("the FIT file is truncated; the rest is not shown");
                break;
            };
            let fields = fb
                .chunks_exact(3)
                .map(|c| FieldDef {
                    num: c[0],
                    size: usize::from(c[1]),
                    base: c[2],
                })
                .collect();
            i += 3 * n;
            let mut dev_size = 0;
            if h & 0x20 != 0 {
                let Some(&nd) = body.get(i) else {
                    out.warn("the FIT file is truncated; the rest is not shown");
                    break;
                };
                i += 1;
                let nd = usize::from(nd);
                let Some(db) = body.get(i..i + 3 * nd) else {
                    out.warn("the FIT file is truncated; the rest is not shown");
                    break;
                };
                dev_size = db.chunks_exact(3).map(|c| usize::from(c[1])).sum();
                i += 3 * nd;
            }
            defs[local] = Some(Def {
                global,
                big,
                fields,
                dev_size,
            });
            continue;
        }
        let Some(def) = defs[local].as_ref() else {
            out.warn("a FIT data message has no definition; the rest is not shown");
            break;
        };
        let total: usize = def.fields.iter().map(|f| f.size).sum::<usize>() + def.dev_size;
        let Some(m) = body.get(i..i + total) else {
            out.warn("the FIT file is truncated; the rest is not shown");
            break;
        };
        i += total;
        let mut fields = Vec::with_capacity(def.fields.len());
        let mut o = 0usize;
        for f in &def.fields {
            if let Some(v) = value(&m[o..o + f.size], f.base, def.big) {
                fields.push((f.num, v));
            }
            o += f.size;
        }
        let mut msg = Msg {
            global: def.global,
            fields,
        };
        // Timestamps: field 253, or a compressed 5-bit offset from the last one.
        if let Some(t) = msg.get(253) {
            last_ts = Some(t as u32);
        } else if let (Some(off), Some(last)) = (compressed_ts, last_ts) {
            let t = if off >= last & 0x1F {
                (last & !0x1F) + off
            } else {
                (last & !0x1F) + off + 0x20
            };
            last_ts = Some(t);
            msg.fields.push((253, f64::from(t)));
        }
        let unix = |v: f64| v + FIT_EPOCH as f64;
        match msg.global {
            0 => {
                if out.file_type.is_none() {
                    out.file_type = msg.get(0).map(|v| v as u8);
                    out.manufacturer = msg.get(1).map(|v| v as u16);
                }
            }
            18 => out.sessions.push(msg),
            19 => out.laps.push(msg),
            20 => {
                if out.records.len() >= MAX_RECORDS {
                    continue;
                }
                let semi = 180.0 / 2f64.powi(31);
                let pos = match (msg.get(1), msg.get(0)) {
                    (Some(lon), Some(lat)) => Some([lon * semi, lat * semi]),
                    _ => None,
                };
                out.records.push(Rec {
                    t: msg.get(253).map(unix),
                    pos,
                    hr: msg.get(3),
                    cad: msg.get(4),
                    temp: msg.get(13),
                });
            }
            _ => {}
        }
    }
    Ok(at + hsize + body_len + 2)
}

fn sport_name(v: u8) -> String {
    const S: &[&str] = &[
        "generic",
        "running",
        "cycling",
        "transition",
        "fitness equipment",
        "swimming",
        "basketball",
        "soccer",
        "tennis",
        "american football",
        "training",
        "walking",
        "cross-country skiing",
        "alpine skiing",
        "snowboarding",
        "rowing",
        "mountaineering",
        "hiking",
        "multisport",
        "paddling",
        "flying",
        "e-biking",
        "motorcycling",
        "boating",
        "driving",
        "golf",
        "hang gliding",
        "horseback riding",
        "hunting",
        "fishing",
        "inline skating",
        "rock climbing",
        "sailing",
        "ice skating",
        "sky diving",
        "snowshoeing",
        "snowmobiling",
        "stand-up paddleboarding",
        "surfing",
        "wakeboarding",
        "water skiing",
        "kayaking",
        "rafting",
        "windsurfing",
        "kitesurfing",
    ];
    S.get(usize::from(v))
        .map_or_else(|| format!("sport {v}"), |s| (*s).to_string())
}

fn manufacturer_name(v: u16) -> String {
    match v {
        1 => "Garmin".into(),
        15 => "Dynastream".into(),
        23 => "Suunto".into(),
        32 => "Wahoo".into(),
        41 => "Shimano".into(),
        69 => "Stages".into(),
        89 => "Tacx".into(),
        123 => "Polar".into(),
        255 => "development".into(),
        260 => "Zwift".into(),
        265 => "Strava".into(),
        267 => "Bryton".into(),
        294 => "Coros".into(),
        v => format!("manufacturer {v}"),
    }
}

fn file_type_name(v: u8) -> String {
    match v {
        1 => "device".into(),
        2 => "settings".into(),
        3 => "sport settings".into(),
        4 => "activity".into(),
        5 => "workout".into(),
        6 => "course".into(),
        7 => "schedules".into(),
        9 => "weight".into(),
        10 => "totals".into(),
        11 => "goals".into(),
        14 => "blood pressure".into(),
        15 | 28 | 32 => "monitoring".into(),
        20 => "activity summary".into(),
        34 => "segment".into(),
        35 => "segment list".into(),
        v => format!("type {v}"),
    }
}

/// The records of a time span (all records when the span is unknown or the only one).
fn span(recs: &[Rec], start: Option<f64>, end: Option<f64>) -> Vec<Rec> {
    match (start, end) {
        (Some(s), Some(e)) => recs
            .iter()
            .filter(|r| r.t.is_some_and(|t| t >= s && t <= e))
            .copied()
            .collect(),
        _ => recs.to_vec(),
    }
}

fn track(recs: &[Rec], s: &mut Summary) -> Option<Geometry> {
    let mut pts: Vec<Pt> = Vec::new();
    for r in recs {
        if let Some(t) = r.t {
            s.time(t);
        }
        if let Some(v) = r.hr {
            s.hr.add(v);
        }
        if let Some(v) = r.cad {
            s.cad.add(v);
        }
        if let Some(v) = r.temp {
            s.temp.add(v);
        }
        if let Some(p) = r.pos
            && p[0].is_finite()
            && p[1].is_finite()
        {
            pts.push(p);
        }
    }
    s.points = pts.len();
    match pts.len() {
        0 => None,
        1 => Some(Geometry::Point(pts[0])),
        _ => Some(Geometry::LineString(pts)),
    }
}

/// Session or lap times: start (2), end (253 or start + elapsed 7), unix seconds.
fn times(m: &Msg) -> (Option<f64>, Option<f64>, Option<f64>) {
    let start = m.get(2).map(|v| v + FIT_EPOCH as f64);
    let dur = m.get(7).map(|v| v / 1000.0);
    let end = m
        .get(253)
        .map(|v| v + FIT_EPOCH as f64)
        .or_else(|| Some(start? + dur?));
    (start, end, dur)
}

/// Every record message of a FIT file (or several chained), in order: how many there are
/// and the positions (longitude, latitude in degrees) of those that have one.
pub fn positions(data: &[u8]) -> Result<(usize, Vec<Pt>), String> {
    let mut p = Parsed::default();
    let mut at = 0usize;
    let mut files = 0;
    while at + 12 <= data.len() && sniff(&data[at..]) && files < 1024 {
        match parse_one(data, at, &mut p) {
            Ok(next) => at = next,
            Err(e) if files == 0 => return Err(e),
            Err(_) => break,
        }
        files += 1;
    }
    if files == 0 {
        return Err("not a FIT file".into());
    }
    Ok((
        p.records.len(),
        p.records.iter().filter_map(|r| r.pos).collect(),
    ))
}

/// Reads a FIT file (or several chained).
pub fn read(data: &[u8], name: &str) -> Result<Dataset, String> {
    let _ = name;
    let mut p = Parsed::default();
    let mut at = 0usize;
    let mut files = 0;
    while at + 12 <= data.len() && sniff(&data[at..]) && files < 1024 {
        match parse_one(data, at, &mut p) {
            Ok(next) => at = next,
            Err(e) => {
                if files == 0 {
                    return Err(e);
                }
                break;
            }
        }
        files += 1;
    }
    if files == 0 {
        return Err("not a FIT file".into());
    }
    if !p.records.iter().any(|r| r.pos.is_some()) {
        return Err(match p.file_type {
            Some(t) => {
                let n = file_type_name(t);
                let a = if n.starts_with(['a', 'e', 'i', 'o', 'u']) {
                    "an"
                } else {
                    "a"
                };
                format!("this FIT file has no GPS positions ({a} {n} file)")
            }
            None => "this FIT file has no GPS positions".into(),
        });
    }
    let device = p.manufacturer.map(manufacturer_name);
    let mut ds = Dataset {
        format: "FIT",
        crs: crs::from_epsg(4326),
        ..Dataset::default()
    };
    let mut tracks = summary_layer("tracks", SUMMARY_FIELDS, "LineString");
    let single = p.sessions.len() <= 1;
    let sessions: Vec<Option<&Msg>> = if p.sessions.is_empty() {
        vec![None]
    } else {
        p.sessions.iter().map(Some).collect()
    };
    for (k, m) in sessions.into_iter().enumerate() {
        if k >= MAX_FEATURES {
            break;
        }
        let mut s = Summary {
            device: device.clone(),
            ..Summary::default()
        };
        let recs = match m {
            Some(m) => {
                let (start, end, dur) = times(m);
                s.start = start;
                s.end = end;
                s.duration = dur;
                s.sport = m.get(5).map(|v| sport_name(v as u8));
                s.distance = m.get(9).map(|v| v / 100.0);
                s.calories = m.get(11);
                s.avg_hr = m.get(16);
                s.max_hr = m.get(17);
                s.avg_cad = m.get(18);
                s.ascent = m.get(22);
                if single {
                    p.records.clone()
                } else {
                    span(&p.records, start, end)
                }
            }
            None => p.records.clone(),
        };
        let g = track(&recs, &mut s);
        tracks.push(Some(k.to_string()), g, s.attrs(SUMMARY_FIELDS));
    }
    ds.layers.push(tracks);
    if !p.laps.is_empty() {
        const LAP_FIELDS: &[&str] = &["start", "end", "duration_s", "distance_m", "points"];
        let mut laps = summary_layer("laps", LAP_FIELDS, "LineString");
        for (k, m) in p.laps.iter().enumerate().take(MAX_FEATURES) {
            let (start, end, dur) = times(m);
            let mut s = Summary {
                start,
                end,
                duration: dur,
                distance: m.get(9).map(|v| v / 100.0),
                ..Summary::default()
            };
            let recs = if start.is_some() && end.is_some() {
                span(&p.records, start, end)
            } else {
                Vec::new()
            };
            let g = track(&recs, &mut s);
            laps.push(Some(k.to_string()), g, s.attrs(LAP_FIELDS));
        }
        ds.layers.push(laps);
    }
    let unplaced = p.records.iter().filter(|r| r.pos.is_none()).count();
    if unplaced > 0 {
        ds.warn(format!(
            "{unplaced} records have no position and are not drawn"
        ));
    }
    for w in std::mem::take(&mut p.warnings) {
        ds.warn(w);
    }
    Ok(ds)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds FIT files for tests.
    pub struct Writer {
        pub body: Vec<u8>,
    }

    impl Writer {
        pub fn new() -> Writer {
            Writer { body: Vec::new() }
        }
        /// A definition: local type, global number, fields (num, size, base), developer
        /// field sizes, byte order.
        pub fn def(
            &mut self,
            local: u8,
            global: u16,
            fields: &[(u8, u8, u8)],
            dev: &[u8],
            big: bool,
        ) {
            let dev_flag = if dev.is_empty() { 0 } else { 0x20 };
            self.body.push(0x40 | dev_flag | local);
            self.body.push(0);
            self.body.push(u8::from(big));
            if big {
                self.body.extend_from_slice(&global.to_be_bytes());
            } else {
                self.body.extend_from_slice(&global.to_le_bytes());
            }
            self.body.push(fields.len() as u8);
            for f in fields {
                self.body.extend_from_slice(&[f.0, f.1, f.2]);
            }
            if !dev.is_empty() {
                self.body.push(dev.len() as u8);
                for (k, s) in dev.iter().enumerate() {
                    self.body.extend_from_slice(&[k as u8, *s, 0]);
                }
            }
        }
        pub fn data(&mut self, header: u8, bytes: &[u8]) {
            self.body.push(header);
            self.body.extend_from_slice(bytes);
        }
        pub fn finish(&self) -> Vec<u8> {
            let mut out = vec![14u8, 0x20];
            out.extend_from_slice(&2100u16.to_le_bytes());
            out.extend_from_slice(&(self.body.len() as u32).to_le_bytes());
            out.extend_from_slice(b".FIT");
            let hc = crc16(&out);
            out.extend_from_slice(&hc.to_le_bytes());
            out.extend_from_slice(&self.body);
            let c = crc16(&out);
            out.extend_from_slice(&c.to_le_bytes());
            out
        }
    }

    fn semi(deg: f64) -> i32 {
        (deg / (180.0 / 2f64.powi(31))).round() as i32
    }

    /// An activity: file_id, three records (one by compressed timestamp, one without a
    /// position, a developer field on each), a session.
    pub fn activity() -> Vec<u8> {
        let mut w = Writer::new();
        w.def(0, 0, &[(0, 1, 0), (1, 2, 0x84)], &[], false);
        let mut b = vec![4u8];
        b.extend_from_slice(&1u16.to_le_bytes());
        w.data(0, &b);
        // record: timestamp, lat, long, heart rate, temperature; one 2-byte dev field.
        w.def(
            1,
            20,
            &[
                (253, 4, 0x86),
                (0, 4, 0x85),
                (1, 4, 0x85),
                (3, 1, 2),
                (13, 1, 1),
            ],
            &[2],
            false,
        );
        let rec = |t: Option<u32>, lat: f64, lon: f64, hr: u8, temp: i8| {
            let mut b = Vec::new();
            b.extend_from_slice(&t.unwrap_or(u32::MAX).to_le_bytes());
            b.extend_from_slice(&semi(lat).to_le_bytes());
            b.extend_from_slice(&semi(lon).to_le_bytes());
            b.push(hr);
            b.push(temp as u8);
            b.extend_from_slice(&[0xAB, 0xCD]);
            b
        };
        let t0 = 1_000_000_000u32;
        w.data(1, &rec(Some(t0), 47.0, 8.0, 100, 20));
        // Compressed timestamp header for local type 1: offset (t0 + 5) & 31.
        let off = ((t0 + 5) & 0x1F) as u8;
        let mut r = rec(None, 47.001, 8.002, 120, 22);
        r[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        w.data(0x80 | (1 << 5) | off, &r);
        // A record without a position, big-endian definition.
        w.def(2, 20, &[(253, 4, 0x86), (0, 4, 0x85), (3, 1, 2)], &[], true);
        let mut b = Vec::new();
        b.extend_from_slice(&(t0 + 9).to_be_bytes());
        b.extend_from_slice(&0x7FFF_FFFFi32.to_be_bytes());
        b.push(140);
        w.data(2, &b);
        // session: timestamp, start_time, sport, total_elapsed_time, total_distance.
        w.def(
            3,
            18,
            &[
                (253, 4, 0x86),
                (2, 4, 0x86),
                (5, 1, 0),
                (7, 4, 0x86),
                (9, 4, 0x86),
            ],
            &[],
            false,
        );
        let mut b = Vec::new();
        b.extend_from_slice(&(t0 + 9).to_le_bytes());
        b.extend_from_slice(&t0.to_le_bytes());
        b.push(1);
        b.extend_from_slice(&9000u32.to_le_bytes());
        b.extend_from_slice(&25_000u32.to_le_bytes());
        w.data(3, &b);
        w.finish()
    }

    #[test]
    fn reads_an_activity() {
        let data = activity();
        assert!(sniff(&data));
        let ds = read(&data, "x").unwrap();
        assert_eq!(ds.format, "FIT");
        assert_eq!(ds.crs.as_ref().and_then(|c| c.epsg), Some(4326));
        assert!(
            ds.warnings
                .iter()
                .any(|w| w.contains("1 records have no position")),
            "{:?}",
            ds.warnings
        );
        assert!(
            !ds.warnings.iter().any(|w| w.contains("checksum")),
            "{:?}",
            ds.warnings
        );
        let l = &ds.layers[0];
        assert_eq!(l.features.len(), 1);
        let Some(Geometry::LineString(pts)) = &l.features[0].geometry else {
            panic!("{:?}", l.features[0].geometry)
        };
        assert_eq!(pts.len(), 2);
        assert!((pts[0][0] - 8.0).abs() < 1e-7 && (pts[0][1] - 47.0).abs() < 1e-7);
        assert!((pts[1][1] - 47.001).abs() < 1e-7);
        let a: std::collections::HashMap<_, _> = l.attributes(0).into_iter().collect();
        assert_eq!(a["sport"], Value::Text("running".into()));
        assert_eq!(a["start"], Value::Date(iso_utc(1_000_000_000 + FIT_EPOCH)));
        assert_eq!(a["duration_s"], Value::Float(9.0));
        assert_eq!(a["distance_m"], Value::Float(250.0));
        assert_eq!(a["points"], Value::Int(2));
        assert_eq!(a["avg_hr"], Value::Float(120.0));
        assert_eq!(a["max_hr"], Value::Float(140.0));
        assert_eq!(a["min_temp"], Value::Float(20.0));
        assert_eq!(a["max_temp"], Value::Float(22.0));
        assert_eq!(a["device"], Value::Text("Garmin".into()));
    }

    #[test]
    fn chained_files_and_damage() {
        let one = activity();
        let mut two = one.clone();
        two.extend_from_slice(&one);
        let ds = read(&two, "x").unwrap();
        assert_eq!(ds.layers[0].features.len(), 2);
        for cut in [0, 5, 13, 20, 40, one.len() - 1] {
            let _ = read(&one[..cut], "x");
        }
        let mut bad = one.clone();
        bad[30] ^= 0xFF;
        let _ = read(&bad, "x");
        for k in 14..one.len() {
            let mut b = one.clone();
            b[k] = b[k].wrapping_add(0x55);
            let _ = read(&b, "x");
        }
    }

    #[test]
    fn no_positions_is_refused() {
        let mut w = Writer::new();
        w.def(0, 0, &[(0, 1, 0)], &[], false);
        w.data(0, &[2]);
        assert_eq!(
            read(&w.finish(), "x").unwrap_err(),
            "this FIT file has no GPS positions (a settings file)"
        );
    }

    #[test]
    fn times() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(FIT_EPOCH), "1989-12-31T00:00:00Z");
        assert_eq!(parse_iso("1989-12-31T00:00:00Z"), Some(FIT_EPOCH as f64));
        assert_eq!(
            parse_iso("2024-05-01T12:00:00+02:00"),
            parse_iso("2024-05-01T10:00:00Z")
        );
        assert_eq!(
            parse_iso("2024-05-01T10:00:00.5Z").map(|t| t.fract()),
            Some(0.5)
        );
        assert_eq!(parse_iso("nonsense"), None);
    }
}
