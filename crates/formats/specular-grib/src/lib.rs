//! GRIB, the WMO's gridded binary format for weather and climate fields, editions 1 and 2.
//!
//! A file is a run of messages, each starting `GRIB` and ending `7777`; a GRIB2 message may
//! repeat its later sections to carry several fields. [`scan`] lists every field with its
//! parameter, level, times and grid from the section headers alone; [`Field::decode`]
//! unpacks one field's values in the message's own point order and [`Field::plane`]
//! lays them out north up and west to east.
//!
//! Packing: simple (GRIB1 and template 5.0), complex and complex with spatial differencing
//! (5.2, 5.3), JPEG 2000 (5.40, through `specular-jp2`), PNG (5.41), CCSDS/AEC (5.42, through
//! `specular-aec`) and IEEE floats (5.4), with bitmaps of missing points. Grids: regular and
//! rotated latitude/longitude, regular, reduced and rotated Gaussian, polar stereographic and
//! Lambert conformal ([`grid`]). Parameter, level and time names come from a compact table
//! of the common WMO and ECMWF fields ([`tables`]).
//!
//! Written from the WMO Manual on Codes (FM 92 GRIB edition 2, FM 92-XI edition 1).

use std::ops::Range;
use std::sync::Arc;

use thiserror::Error;

mod bits;
mod grib1;
mod grib2;
pub mod grid;
pub mod tables;
mod unpack;

pub use grid::{Earth, Grid, GridKind, Plane};

#[derive(Debug, Error)]
pub enum Error {
    #[error("GRIB message ends early")]
    Truncated,
    #[error("malformed GRIB: {0}")]
    Malformed(String),
    #[error("unsupported GRIB: {0}")]
    Unsupported(String),
    /// The memory budget cannot hold the field.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Values or grid points past which a field is refused. A constant field (no packed bits)
/// needs no data, so a few bytes can declare any count, and every value is an `f64` (then
/// an `f32` in the plane): 2^27 points is 1 GiB, past the finest global grids (a 0.025°
/// grid has 10^8 points).
pub const MAX_POINTS: usize = 1 << 27;

fn bad<T>(what: impl Into<String>) -> Result<T> {
    Err(Error::Malformed(what.into()))
}

fn unsupported<T>(what: impl Into<String>) -> Result<T> {
    Err(Error::Unsupported(what.into()))
}

/// Whether `head` starts a GRIB file (edition 1 or 2).
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 8 && &head[..4] == b"GRIB" && matches!(head[7], 1 | 2)
}

/// A UTC date and time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl DateTime {
    /// Seconds since 1970-01-01 00:00 UTC.
    pub fn unix(&self) -> i64 {
        days_from_civil(
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        ) * 86400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
    }

    pub fn from_unix(t: i64) -> DateTime {
        let days = t.div_euclid(86400);
        let rest = t.rem_euclid(86400);
        let (y, m, d) = civil_from_days(days);
        DateTime {
            year: y as i32,
            month: m as u8,
            day: d as u8,
            hour: (rest / 3600) as u8,
            minute: (rest / 60 % 60) as u8,
            second: (rest % 60) as u8,
        }
    }
}

impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute
        )?;
        if self.second != 0 {
            write!(f, ":{:02}", self.second)?;
        }
        Ok(())
    }
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// What a field measures.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Param {
    /// GRIB2: discipline (table 0.0). GRIB1: 255.
    pub discipline: u8,
    /// GRIB2: parameter category. GRIB1: the parameter table version.
    pub category: u8,
    pub number: u8,
    /// GRIB1: the originating centre (its local tables), else 0.
    pub centre: u16,
    /// Short name ("t", "2t", "msl"), or a code like "0.0.0" when not in the table.
    pub short: String,
    /// English name, empty when not in the table.
    pub name: String,
    /// Units of the values, empty when not in the table.
    pub units: String,
}

impl Param {
    /// "t (Temperature)", or just the short name when the table doesn't know it.
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            self.short.clone()
        } else {
            format!("{} ({})", self.short, self.name)
        }
    }
}

/// The vertical level or layer of a field.
#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    /// The edition's level type code (GRIB2 table 4.5, GRIB1 table 3).
    pub kind: u8,
    /// The type's name in a table ("isobaricInhPa", "surface").
    pub name: &'static str,
    /// The level value in `unit` (hPa for pressure levels), when the type has one.
    pub value: Option<f64>,
    /// The bottom of a layer.
    pub value2: Option<f64>,
    pub unit: &'static str,
}

impl Level {
    /// "850 hPa", "2 m above ground", "surface", "0–7 cm below ground".
    pub fn label(&self) -> String {
        let num = |v: f64| {
            if v.fract() == 0.0 && v.abs() < 1e15 {
                format!("{}", v as i64)
            } else {
                format!("{v}")
            }
        };
        let (suffix, human) = tables::level_words(self.name);
        match (self.value, self.value2) {
            (Some(a), Some(b)) if a != b && !self.unit.is_empty() => {
                format!("{}–{} {}{suffix}", num(a), num(b), self.unit)
            }
            (Some(a), _) if !self.unit.is_empty() => format!("{} {}{suffix}", num(a), self.unit),
            (Some(a), _) => format!("{human} {}", num(a)),
            _ => human.to_owned(),
        }
    }

    /// For ordering levels of one type: the value (pressure descending reads top-down).
    pub fn sort_key(&self) -> (u8, f64) {
        (self.kind, self.value.unwrap_or(0.0))
    }
}

/// Units of a forecast time (GRIB2 table 4.4; GRIB1 table 4 is mapped onto it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimeUnit {
    Second,
    Minute,
    Hour,
    Day,
    Month,
    Year,
}

impl TimeUnit {
    /// Approximate seconds (30-day months, 365-day years).
    pub fn seconds(self) -> i64 {
        match self {
            TimeUnit::Second => 1,
            TimeUnit::Minute => 60,
            TimeUnit::Hour => 3600,
            TimeUnit::Day => 86400,
            TimeUnit::Month => 30 * 86400,
            TimeUnit::Year => 365 * 86400,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            TimeUnit::Second => "s",
            TimeUnit::Minute => "min",
            TimeUnit::Hour => "h",
            TimeUnit::Day => "d",
            TimeUnit::Month => "mo",
            TimeUnit::Year => "y",
        }
    }
}

/// When a field is valid, relative to the reference time.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Forecast {
    pub unit: TimeUnit,
    /// The forecast time (the start of a statistical range), in `unit`.
    pub start: i64,
    /// The end of a statistical range (accumulation, average, extreme), in `unit`.
    pub end: Option<i64>,
    /// The statistical process over the range ("accumulation", "average", ...).
    pub statistic: Option<&'static str>,
}

impl Forecast {
    /// The offset of the validity time (the end of a range) in seconds.
    pub fn valid_seconds(&self) -> i64 {
        // Saturating: a 4-octet count of centuries overflows.
        self.end
            .unwrap_or(self.start)
            .saturating_mul(self.unit.seconds())
    }

    /// "+6 h", "0–12 h accumulation".
    pub fn label(&self) -> String {
        let u = self.unit.symbol();
        let mut s = match self.end {
            Some(e) if e != self.start => format!("{}–{e} {u}", self.start),
            _ => format!("+{} {u}", self.start),
        };
        if let Some(st) = self.statistic {
            s.push(' ');
            s.push_str(st);
        }
        s
    }
}

/// How the field's values are packed.
#[derive(Clone, Debug, PartialEq)]
pub enum Packing {
    /// Simple packing (GRIB1, template 5.0) and the same value scaling for the others.
    Simple(Scale),
    /// Complex packing (5.2) or with spatial differencing (5.3, `order` 1 or 2).
    Complex(Scale, Complex),
    /// JPEG 2000 codestream (5.40).
    Jpeg2000(Scale),
    /// PNG (5.41).
    Png(Scale),
    /// CCSDS/AEC (5.42).
    Ccsds(Scale, Ccsds),
    /// IEEE floats (5.4): 4 or 8 bytes.
    Ieee(u8),
}

impl Packing {
    pub fn name(&self) -> &'static str {
        match self {
            Packing::Simple(_) => "simple",
            Packing::Complex(_, c) if c.order > 0 => "complex, spatial differencing",
            Packing::Complex(..) => "complex",
            Packing::Jpeg2000(_) => "JPEG 2000",
            Packing::Png(_) => "PNG",
            Packing::Ccsds(..) => "CCSDS",
            Packing::Ieee(_) => "IEEE float",
        }
    }

    /// Bits per packed value (0 for a constant field).
    pub fn bits(&self) -> u32 {
        match self {
            Packing::Simple(s)
            | Packing::Complex(s, _)
            | Packing::Jpeg2000(s)
            | Packing::Png(s)
            | Packing::Ccsds(s, _) => u32::from(s.bits),
            Packing::Ieee(b) => u32::from(*b) * 8,
        }
    }
}

/// Y = (R + X · 2^E) · 10^−D.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    pub reference: f64,
    pub binary: i32,
    pub decimal: i32,
    pub bits: u8,
}

impl Scale {
    #[inline]
    fn value(&self, x: f64) -> f64 {
        (self.reference + x * self.bscale()) * self.dscale()
    }

    fn bscale(&self) -> f64 {
        2f64.powi(self.binary)
    }

    fn dscale(&self) -> f64 {
        10f64.powi(-self.decimal)
    }
}

/// Template 5.2/5.3 parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Complex {
    /// 0 none, 1 primary, 2 primary and secondary missing values.
    pub missing: u8,
    pub groups: u32,
    pub width_ref: u8,
    pub width_bits: u8,
    pub length_ref: u32,
    pub length_inc: u8,
    pub last_length: u32,
    pub length_bits: u8,
    /// Spatial differencing order (0 for template 5.2).
    pub order: u8,
    /// Octets of each extra descriptor (5.3).
    pub octets: u8,
}

/// Template 5.42 parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ccsds {
    pub flags: u8,
    pub block: u8,
    pub rsi: u16,
}

/// Ensemble facts (GRIB2 templates 4.1, 4.11; ECMWF GRIB1 local definitions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ensemble {
    pub member: u32,
    pub size: Option<u32>,
}

/// One field: the headers of a GRIB1 message or of one repetition in a GRIB2 message.
#[derive(Clone, Debug)]
pub struct Field {
    pub edition: u8,
    /// The message's bytes in the file.
    pub message: Range<usize>,
    pub centre: u16,
    pub sub_centre: u16,
    pub reference: DateTime,
    pub param: Param,
    pub level: Level,
    pub forecast: Forecast,
    pub ensemble: Option<Ensemble>,
    /// Shared by the fields of a message (and by equal grids across messages, see
    /// [`scan`]).
    pub grid: Arc<Grid>,
    pub packing: Packing,
    /// Packed values (points less those the bitmap marks missing).
    pub values: usize,
    /// The packed data in the file.
    pub data: Range<usize>,
    /// The bitmap in the file (one bit per grid point, 1 = present).
    pub bitmap: Option<Range<usize>>,
    /// The product (GRIB2 template 4.x, GRIB1 time range indicator), for the info panel.
    pub product: u16,
}

impl Field {
    /// The validity time.
    pub fn valid(&self) -> DateTime {
        DateTime::from_unix(
            self.reference
                .unix()
                .saturating_add(self.forecast.valid_seconds()),
        )
    }

    /// The values in the message's point order, NaN where missing.
    pub fn decode(&self, file: &[u8]) -> Result<Vec<f64>> {
        unpack::field(self, file)
    }

    /// The values as a north-up, west-to-east plane (reduced grids interpolated to their
    /// longest row), NaN where missing.
    pub fn plane(&self, file: &[u8]) -> Result<Plane> {
        let (w, h) = self.grid.plane_size();
        if w as usize * h as usize > MAX_POINTS {
            return unsupported(format!("{w} × {h} plane"));
        }
        let v = self.decode(file)?;
        Ok(self.grid.plane(&v))
    }
}

/// Lists every field of a GRIB file. Bytes between messages are skipped; a damaged message
/// ends the scan (the fields before it are kept) unless nothing was found.
pub fn scan(file: &[u8]) -> Result<Vec<Field>> {
    let mut out: Vec<Field> = Vec::new();
    let mut grids: Vec<Arc<Grid>> = Vec::new();
    let mut pos = 0;
    let mut first_err = None;
    while let Some(at) = find_grib(file, pos) {
        let r = match file.get(at + 7) {
            Some(1) => grib1::message(file, at),
            Some(2) => grib2::message(file, at),
            _ => {
                pos = at + 4;
                continue;
            }
        };
        match r {
            Ok((fields, end)) => {
                for mut f in fields {
                    // Equal grids share one Arc, so callers can group by pointer.
                    match grids.iter().find(|g| ***g == *f.grid) {
                        Some(g) => f.grid = g.clone(),
                        None => grids.push(f.grid.clone()),
                    }
                    out.push(f);
                }
                pos = end.max(at + 4);
            }
            Err(e) => {
                if out.is_empty() && first_err.is_none() {
                    first_err = Some(e);
                }
                // Resynchronise past this signature.
                pos = at + 4;
            }
        }
    }
    match first_err {
        Some(e) if out.is_empty() => Err(e),
        _ if out.is_empty() => bad("no GRIB messages"),
        _ => Ok(out),
    }
}

fn find_grib(file: &[u8], from: usize) -> Option<usize> {
    file.get(from..)?
        .windows(4)
        .position(|w| w == b"GRIB")
        .map(|p| p + from)
}

#[cfg(test)]
mod tests;
