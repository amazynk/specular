//! National Instruments TDMS files (`.tdms`, TDMS 2.0: versions 4712 and 4713), own reader.
//!
//! A file is a chain of segments, each a lead-in (`TDSm`, a table-of-contents mask, the
//! version, the offsets of the next segment and of the raw data), optional metadata (objects
//! by path with a raw data index and properties) and optional raw data, which repeats in
//! chunks. Objects not listed again keep the previous segment's layout unless the segment
//! starts a new object list.
//!
//! * [`parse`] reads only the metadata: groups, channels, properties, and per channel a list
//!   of runs (where each stretch of its samples lies in the file), so reading any range of
//!   a channel is a binary search and a strided copy ([`Channel::read`]).
//! * Raw data: integers, floats, booleans, timestamps, complex numbers and strings,
//!   interleaved or not, little- or big-endian; DAQmx raw data (format-changing and digital
//!   line scalers, several raw buffers).
//! * Scaling from `NI_Scale[n]_*` properties (the channel's, else its group's, else the
//!   file's, unless `NI_Scaling_Status` is `scaled`): linear, polynomial, table, add,
//!   subtract, no-op and DAQmx scaler inputs. RTD, strain, thermistor and thermocouple scales
//!   are refused by name.
//! * A final segment cut short (next-segment offset all ones, or past the end) is read as
//!   far as it goes.
//!
//! Every count is checked against the bytes present before anything is allocated, and no
//! input panics.

mod read;
mod scale;

use std::collections::HashMap;

const TOC_META: u32 = 1 << 1;
const TOC_NEW_LIST: u32 = 1 << 2;
const TOC_RAW: u32 = 1 << 3;
const TOC_INTERLEAVED: u32 = 1 << 5;
const TOC_BIG_ENDIAN: u32 = 1 << 6;
const TOC_DAQMX: u32 = 1 << 7;
const LEAD_IN: usize = 28;
/// Raw data index words.
const NO_DATA: u32 = 0xFFFF_FFFF;
const SAME_AS_BEFORE: u32 = 0;
/// `69 12 00 00` and `69 13 00 00` as stored (little-endian words).
const DAQMX_FORMAT: u32 = 0x0000_1269;
const DAQMX_DIGITAL: u32 = 0x0000_126A;
/// Runs (stretches of a channel's samples) kept for the whole file at most.
const MAX_RUNS: usize = 1 << 24;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a TDMS file")]
    NotTdms,
    #[error("truncated TDMS file: {0}")]
    Truncated(String),
    #[error("malformed TDMS file: {0}")]
    Malformed(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("TDMS file too large")]
    TooLarge,
}

pub type Result<T> = std::result::Result<T, Error>;

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Malformed(msg.into()))
}

/// A channel's (or property's) data type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Extended,
    String,
    Bool,
    Timestamp,
    ComplexF32,
    ComplexF64,
    DaqmxRaw,
    Other(u32),
}

impl DataType {
    fn from_code(c: u32) -> DataType {
        match c {
            1 => DataType::I8,
            2 => DataType::I16,
            3 => DataType::I32,
            4 => DataType::I64,
            5 => DataType::U8,
            6 => DataType::U16,
            7 => DataType::U32,
            8 => DataType::U64,
            9 | 0x19 => DataType::F32,
            0x0A | 0x1A => DataType::F64,
            0x0B | 0x1B => DataType::Extended,
            0x20 => DataType::String,
            0x21 => DataType::Bool,
            0x44 => DataType::Timestamp,
            0x0008_000C => DataType::ComplexF32,
            0x0010_000D => DataType::ComplexF64,
            0xFFFF_FFFF => DataType::DaqmxRaw,
            c => DataType::Other(c),
        }
    }

    /// E.g. "16-bit signed", "32-bit float", "DAQmx raw".
    pub fn describe(&self) -> &'static str {
        match self {
            DataType::I8 => "8-bit signed",
            DataType::I16 => "16-bit signed",
            DataType::I32 => "32-bit signed",
            DataType::I64 => "64-bit signed",
            DataType::U8 => "8-bit unsigned",
            DataType::U16 => "16-bit unsigned",
            DataType::U32 => "32-bit unsigned",
            DataType::U64 => "64-bit unsigned",
            DataType::F32 => "32-bit float",
            DataType::F64 => "64-bit float",
            DataType::Extended => "extended float",
            DataType::String => "text",
            DataType::Bool => "boolean",
            DataType::Timestamp => "timestamp",
            DataType::ComplexF32 => "complex 32-bit float",
            DataType::ComplexF64 => "complex 64-bit float",
            DataType::DaqmxRaw => "DAQmx raw",
            DataType::Other(_) => "unknown",
        }
    }

    /// The stored element, for fixed-size types.
    fn elem(self) -> Option<Elem> {
        Some(match self {
            DataType::I8 => Elem::I8,
            DataType::I16 => Elem::I16,
            DataType::I32 => Elem::I32,
            DataType::I64 => Elem::I64,
            DataType::U8 => Elem::U8,
            DataType::U16 => Elem::U16,
            DataType::U32 => Elem::U32,
            DataType::U64 => Elem::U64,
            DataType::F32 => Elem::F32,
            DataType::F64 => Elem::F64,
            DataType::Extended => Elem::Ext,
            DataType::Bool => Elem::Bool,
            DataType::Timestamp => Elem::Time,
            DataType::ComplexF32 => Elem::C32,
            DataType::ComplexF64 => Elem::C64,
            DataType::String => Elem::Str,
            DataType::DaqmxRaw | DataType::Other(_) => return None,
        })
    }
}

/// How one sample is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Elem {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Ext,
    Bool,
    Time,
    C32,
    C64,
    /// Strings: a table of end offsets, then the text.
    Str,
    /// One bit (of the byte at the sample's offset): a DAQmx digital line.
    Bit(u8),
}

impl Elem {
    /// Bytes per sample (0 for strings).
    pub(crate) fn size(self) -> u64 {
        match self {
            Elem::I8 | Elem::U8 | Elem::Bool | Elem::Bit(_) => 1,
            Elem::I16 | Elem::U16 => 2,
            Elem::I32 | Elem::U32 | Elem::F32 => 4,
            Elem::I64 | Elem::U64 | Elem::F64 | Elem::C32 => 8,
            Elem::Ext | Elem::Time | Elem::C64 => 16,
            Elem::Str => 0,
        }
    }

    fn daqmx(code: u32) -> Result<Elem> {
        Ok(match code {
            0 => Elem::U8,
            1 => Elem::I8,
            2 => Elem::U16,
            3 => Elem::I16,
            4 => Elem::U32,
            5 => Elem::I32,
            6 => Elem::U64,
            7 => Elem::I64,
            8 => Elem::F32,
            9 => Elem::F64,
            0xFFFF_FFFF => Elem::Time,
            c => {
                return Err(Error::Unsupported(format!(
                    "DAQmx raw data of type {c} is not supported"
                )));
            }
        })
    }
}

/// A property value.
#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    I64(i64),
    U64(u64),
    F64(f64),
    Str(String),
    Bool(bool),
    /// Seconds since 1904-01-01 00:00 UTC and fractions of a second in units of 2⁻⁶⁴.
    Time {
        seconds: i64,
        fraction: u64,
    },
    Complex(f64, f64),
}

impl Prop {
    /// The value as text; timestamps in ISO 8601 UTC ("2024-05-01T12:00:00.25Z").
    pub fn text(&self) -> String {
        match self {
            Prop::I64(v) => v.to_string(),
            Prop::U64(v) => v.to_string(),
            Prop::F64(v) => v.to_string(),
            Prop::Str(s) => s.clone(),
            Prop::Bool(b) => b.to_string(),
            Prop::Time { seconds, fraction } => iso_time(*seconds, *fraction),
            Prop::Complex(re, im) => {
                if *im < 0.0 {
                    format!("{re} − {}i", -im)
                } else {
                    format!("{re} + {im}i")
                }
            }
        }
    }

    /// Numbers and booleans as f64; timestamps as seconds since 1904.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Prop::I64(v) => Some(*v as f64),
            Prop::U64(v) => Some(*v as f64),
            Prop::F64(v) => Some(*v),
            Prop::Bool(b) => Some(f64::from(u8::from(*b))),
            Prop::Time { seconds, fraction } => Some(time_f64(*seconds, *fraction)),
            Prop::Str(_) | Prop::Complex(..) => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Prop::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Seconds since 1904 as f64.
pub(crate) fn time_f64(seconds: i64, fraction: u64) -> f64 {
    seconds as f64 + fraction as f64 * (-64f64).exp2()
}

/// Seconds between 1904-01-01 and 1970-01-01.
const EPOCH_1904: i64 = 2_082_844_800;

/// A LabVIEW timestamp as ISO 8601 UTC, with up to six fractional digits.
pub fn iso_time(seconds: i64, fraction: u64) -> String {
    let unix = seconds.saturating_sub(EPOCH_1904);
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let micros = ((u128::from(fraction) * 1_000_000 + (1u128 << 63)) >> 64) as u64;
    let (secs, micros) = if micros >= 1_000_000 {
        (secs + 1, 0)
    } else {
        (secs, micros)
    };
    let (days_carry, secs) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = if days_carry > 0 {
        civil_from_days(days + 1)
    } else {
        (y, m, d)
    };
    let mut s = format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    );
    if micros > 0 {
        let f = format!("{micros:06}");
        s.push('.');
        s.push_str(f.trim_end_matches('0'));
    }
    s.push('Z');
    s
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A stretch of a channel's samples: `repeats` chunks of `count` samples, the `k`-th
/// starting at byte `offset + k × step`, samples `stride` bytes apart. For strings, `offset`
/// is the chunk's table of end offsets and the text follows it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Run {
    pub first: u64,
    pub count: u64,
    pub offset: u64,
    pub stride: u64,
    pub repeats: u64,
    pub step: u64,
    pub elem: Elem,
    pub be: bool,
}

impl Run {
    pub(crate) fn end(&self) -> u64 {
        self.first + self.count * self.repeats
    }
}

/// One TDMS group.
#[derive(Clone, Debug, Default)]
pub struct Group {
    pub name: String,
    pub properties: Vec<(String, Prop)>,
    pub channels: Vec<Channel>,
}

/// One channel: its type, length and where its samples are.
#[derive(Clone, Debug)]
pub struct Channel {
    pub name: String,
    pub properties: Vec<(String, Prop)>,
    /// The type of the first segment that holds data ([`DataType::Other`]`(0)` when none
    /// does).
    pub dtype: DataType,
    /// Samples in the file.
    pub len: u64,
    /// Runs of the raw data (for DAQmx data, the first scaler's).
    pub(crate) runs: Vec<Run>,
    /// DAQmx data: the runs of each scaler, by scale id.
    pub(crate) scalers: Vec<(u32, Vec<Run>)>,
    /// The scaling that applies, or why it cannot.
    pub(crate) scaling: std::result::Result<Option<scale::Scaling>, String>,
}

impl Channel {
    fn new(name: String) -> Channel {
        Channel {
            name,
            properties: Vec::new(),
            dtype: DataType::Other(0),
            len: 0,
            runs: Vec::new(),
            scalers: Vec::new(),
            scaling: Ok(None),
        }
    }

    pub fn property(&self, name: &str) -> Option<&Prop> {
        find(&self.properties, name)
    }

    /// Whether [`Channel::read`] gives numbers: integers, floats, booleans, timestamps,
    /// complex numbers (magnitude) and DAQmx data.
    pub fn is_numeric(&self) -> bool {
        !matches!(
            self.dtype,
            DataType::String | DataType::Extended | DataType::Other(_)
        )
    }

    /// The values have a scaling applied ([`Channel::read`] differs from
    /// [`Channel::read_raw`]).
    pub fn is_scaled(&self) -> bool {
        matches!(self.scaling, Ok(Some(_)))
    }

    /// Why the channel's scaling cannot be applied, if it cannot.
    pub fn scaling_problem(&self) -> Option<&str> {
        self.scaling.as_ref().err().map(String::as_str)
    }

    /// The `unit_string` property.
    pub fn unit(&self) -> Option<&str> {
        self.property("unit_string")
            .and_then(Prop::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// A waveform's x axis: `wf_start_offset`, `wf_increment` and `wf_xunit_string`, when
    /// the channel has an increment.
    pub fn waveform(&self) -> Option<(f64, f64, Option<String>)> {
        let inc = self.property("wf_increment")?.as_f64()?;
        let start = self
            .property("wf_start_offset")
            .and_then(Prop::as_f64)
            .unwrap_or(0.0);
        let unit = self
            .property("wf_xunit_string")
            .and_then(Prop::as_str)
            .map(str::to_owned);
        Some((start, inc, unit))
    }
}

/// An opened file's metadata.
#[derive(Clone, Debug, Default)]
pub struct Tdms {
    /// The first segment's version (4712 or 4713).
    pub version: u32,
    pub segments: usize,
    /// The file object's properties.
    pub properties: Vec<(String, Prop)>,
    pub groups: Vec<Group>,
    /// Some segment's data is interleaved.
    pub interleaved: bool,
    /// Some segment is big-endian.
    pub big_endian: bool,
    /// Some segment holds DAQmx raw data.
    pub daqmx: bool,
    /// The last segment was cut short.
    pub truncated: bool,
}

impl Tdms {
    pub fn property(&self, name: &str) -> Option<&Prop> {
        find(&self.properties, name)
    }
}

pub(crate) fn find<'a>(props: &'a [(String, Prop)], name: &str) -> Option<&'a Prop> {
    props.iter().find(|(k, _)| k == name).map(|(_, v)| v)
}

/// Whether `head` starts like a TDMS file (or its `.tdms_index`): `TDSm` / `TDSh`, then a
/// known version.
pub fn sniff(head: &[u8]) -> bool {
    if head.len() < 12 || !(head.starts_with(b"TDSm") || head.starts_with(b"TDSh")) {
        return false;
    }
    let toc = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
    let v = [head[8], head[9], head[10], head[11]];
    let version = if toc & TOC_BIG_ENDIAN != 0 {
        u32::from_be_bytes(v)
    } else {
        u32::from_le_bytes(v)
    };
    matches!(version, 4712 | 4713)
}

/// A DAQmx scaler: where its values sit in a raw buffer.
#[derive(Clone, Debug)]
struct Scaler {
    elem: Elem,
    buffer: u32,
    byte: u32,
    scale_id: u32,
}

/// A raw data index.
#[derive(Clone, Debug)]
enum Index {
    Plain {
        dtype: DataType,
        count: u64,
        /// Bytes per chunk.
        bytes: u64,
    },
    Daqmx {
        count: u64,
        scalers: Vec<Scaler>,
        widths: Vec<u32>,
    },
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    File,
    Group(usize),
    Channel(usize, usize),
    /// A path deeper than a channel, or one that does not parse.
    Ignored,
}

struct Obj {
    kind: Kind,
    /// The last raw data index given for it.
    last: Option<Index>,
}

struct Cur<'a> {
    d: &'a [u8],
    p: usize,
    end: usize,
    be: bool,
}

impl<'a> Cur<'a> {
    fn left(&self) -> usize {
        self.end - self.p
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.left() < n {
            return Err(Error::Truncated("metadata".into()));
        }
        let s = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn arr<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.take(N)?);
        if self.be {
            a.reverse();
        }
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        self.arr::<4>().map(u32::from_le_bytes)
    }
    fn u64(&mut self) -> Result<u64> {
        self.arr::<8>().map(u64::from_le_bytes)
    }
    fn string(&mut self) -> Result<String> {
        let n = self.u32()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
}

/// Splits an object path ("/", "/'group'", "/'group'/'channel'", quotes doubled inside).
fn parse_path(p: &str) -> Option<Vec<String>> {
    let b = p.as_bytes();
    if b == b"/" {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'/' || b.get(i + 1) != Some(&b'\'') {
            return None;
        }
        i += 2;
        let mut s = Vec::new();
        loop {
            match b.get(i) {
                None => return None,
                Some(b'\'') if b.get(i + 1) == Some(&b'\'') => {
                    s.push(b'\'');
                    i += 2;
                }
                Some(b'\'') => {
                    i += 1;
                    break;
                }
                Some(&c) => {
                    s.push(c);
                    i += 1;
                }
            }
        }
        out.push(String::from_utf8(s).ok()?);
    }
    (!out.is_empty()).then_some(out)
}

fn set_prop(props: &mut Vec<(String, Prop)>, name: String, v: Prop) {
    match props.iter_mut().find(|(k, _)| *k == name) {
        Some(e) => e.1 = v,
        None => props.push((name, v)),
    }
}

/// An IEEE binary128 value (TDMS extended float) as f64.
fn f128_to_f64(a: [u8; 16]) -> f64 {
    let v = u128::from_le_bytes(a);
    let sign = if v >> 127 != 0 { -1.0 } else { 1.0 };
    let exp = ((v >> 112) & 0x7FFF) as i32;
    let frac = v & ((1u128 << 112) - 1);
    let m = (frac >> 60) as f64 * (-52f64).exp2();
    sign * match exp {
        0 if frac == 0 => 0.0,
        0 => m * f64::from(2.0f32).powi(-16382),
        0x7FFF if frac == 0 => f64::INFINITY,
        0x7FFF => f64::NAN,
        e => (1.0 + m) * 2f64.powi(e - 16383),
    }
}

fn read_prop(c: &mut Cur) -> Result<Prop> {
    let t = c.u32()?;
    Ok(match t {
        1 => Prop::I64(i64::from(c.u8()? as i8)),
        2 => Prop::I64(i64::from(i16::from_le_bytes(c.arr()?))),
        3 => Prop::I64(i64::from(i32::from_le_bytes(c.arr()?))),
        4 => Prop::I64(i64::from_le_bytes(c.arr()?)),
        5 => Prop::U64(u64::from(c.u8()?)),
        6 => Prop::U64(u64::from(u16::from_le_bytes(c.arr()?))),
        7 => Prop::U64(u64::from(c.u32()?)),
        8 => Prop::U64(c.u64()?),
        9 | 0x19 => Prop::F64(f64::from(f32::from_le_bytes(c.arr()?))),
        0x0A | 0x1A => Prop::F64(f64::from_le_bytes(c.arr()?)),
        0x0B | 0x1B => Prop::F64(f128_to_f64(c.arr()?)),
        0x20 => Prop::Str(c.string()?),
        0x21 => Prop::Bool(c.u8()? != 0),
        0x44 => {
            let (seconds, fraction) = if c.be {
                (c.u64()? as i64, c.u64()?)
            } else {
                let f = c.u64()?;
                (c.u64()? as i64, f)
            };
            Prop::Time { seconds, fraction }
        }
        0x0008_000C => Prop::Complex(
            f64::from(f32::from_le_bytes(c.arr()?)),
            f64::from(f32::from_le_bytes(c.arr()?)),
        ),
        0x0010_000D => Prop::Complex(f64::from_le_bytes(c.arr()?), f64::from_le_bytes(c.arr()?)),
        0x4F => {
            return Err(Error::Unsupported(
                "TDMS fixed-point properties are not supported".into(),
            ));
        }
        t => return bad(format!("property of unknown type {t:#x}")),
    })
}

fn read_index(c: &mut Cur, word: u32) -> Result<Index> {
    if word == DAQMX_FORMAT || word == DAQMX_DIGITAL {
        let _dtype = c.u32()?;
        let dim = c.u32()?;
        if dim != 1 {
            return Err(Error::Unsupported(
                "TDMS arrays of more than one dimension are not supported".into(),
            ));
        }
        let count = c.u64()?;
        let n = c.u32()? as usize;
        if n > c.left() / 17 {
            return bad("DAQmx scaler count");
        }
        let mut scalers = Vec::with_capacity(n);
        for _ in 0..n {
            let code = c.u32()?;
            let buffer = c.u32()?;
            let offset = c.u32()?;
            let (elem, byte) = if word == DAQMX_DIGITAL {
                let _bitmap = c.u8()?;
                (Elem::Bit((offset % 8) as u8), offset / 8)
            } else {
                let _bitmap = c.u32()?;
                (Elem::daqmx(code)?, offset)
            };
            let scale_id = c.u32()?;
            scalers.push(Scaler {
                elem,
                buffer,
                byte,
                scale_id,
            });
        }
        let n = c.u32()? as usize;
        if n > c.left() / 4 {
            return bad("DAQmx raw data width count");
        }
        let widths = (0..n).map(|_| c.u32()).collect::<Result<Vec<_>>>()?;
        return Ok(Index::Daqmx {
            count,
            scalers,
            widths,
        });
    }
    let code = c.u32()?;
    let dim = c.u32()?;
    let count = c.u64()?;
    let dtype = DataType::from_code(code);
    if dim != 1 {
        return Err(Error::Unsupported(
            "TDMS arrays of more than one dimension are not supported".into(),
        ));
    }
    let bytes = match dtype.elem() {
        Some(Elem::Str) => {
            let total = c.u64()?;
            if total < count.saturating_mul(4) {
                return bad("string data smaller than its offset table");
            }
            total
        }
        Some(e) => count
            .checked_mul(e.size())
            .ok_or_else(|| Error::Malformed("raw data size".into()))?,
        None => {
            return Err(Error::Unsupported(match code {
                0x4F => "TDMS fixed-point channels are not supported".into(),
                0 => "TDMS channels of type void are not supported".into(),
                c => format!("TDMS channels of data type {c:#x} are not supported"),
            }));
        }
    };
    Ok(Index::Plain {
        dtype,
        count,
        bytes,
    })
}

struct State {
    t: Tdms,
    objs: Vec<Obj>,
    by_path: HashMap<String, usize>,
    /// The current segment's objects in order and their index for it (`None`: no data in
    /// this segment).
    list: Vec<(usize, Option<Index>)>,
    runs: usize,
}

impl State {
    fn group(&mut self, name: &str) -> usize {
        match self.t.groups.iter().position(|g| g.name == name) {
            Some(i) => i,
            None => {
                self.t.groups.push(Group {
                    name: name.to_owned(),
                    ..Default::default()
                });
                self.t.groups.len() - 1
            }
        }
    }

    fn object(&mut self, path: String) -> usize {
        if let Some(&i) = self.by_path.get(&path) {
            return i;
        }
        let kind = match parse_path(&path).as_deref() {
            Some([]) => Kind::File,
            Some([g]) => Kind::Group(self.group(g)),
            Some([g, c]) => {
                let gi = self.group(g);
                let chans = &mut self.t.groups[gi].channels;
                chans.push(Channel::new(c.clone()));
                Kind::Channel(gi, chans.len() - 1)
            }
            _ => Kind::Ignored,
        };
        self.objs.push(Obj { kind, last: None });
        self.by_path.insert(path, self.objs.len() - 1);
        self.objs.len() - 1
    }

    fn props(&mut self, id: usize) -> Option<&mut Vec<(String, Prop)>> {
        match self.objs[id].kind {
            Kind::File => Some(&mut self.t.properties),
            Kind::Group(g) => Some(&mut self.t.groups[g].properties),
            Kind::Channel(g, c) => Some(&mut self.t.groups[g].channels[c].properties),
            Kind::Ignored => None,
        }
    }

    fn metadata(&mut self, c: &mut Cur, new_list: bool) -> Result<()> {
        let n = c.u32()? as usize;
        if n > c.left() / 12 {
            return bad("object count");
        }
        if new_list {
            self.list.clear();
        }
        for _ in 0..n {
            let path = c.string()?;
            let id = self.object(path);
            let word = c.u32()?;
            let index = match word {
                NO_DATA => None,
                SAME_AS_BEFORE => Some(self.objs[id].last.clone().ok_or_else(|| {
                    Error::Malformed("an object reuses a raw data index it never had".into())
                })?),
                w => Some(read_index(c, w)?),
            };
            if let Some(ix) = &index {
                self.objs[id].last = Some(ix.clone());
                if let Kind::Channel(g, ci) = self.objs[id].kind {
                    let ch = &mut self.t.groups[g].channels[ci];
                    if ch.dtype == DataType::Other(0) {
                        ch.dtype = match ix {
                            Index::Plain { dtype, .. } => *dtype,
                            Index::Daqmx { .. } => DataType::DaqmxRaw,
                        };
                    }
                }
            }
            match self.list.iter_mut().find(|e| e.0 == id) {
                Some(e) => e.1 = index,
                None => self.list.push((id, index)),
            }
            let m = c.u32()? as usize;
            if m > c.left() / 8 {
                return bad("property count");
            }
            for _ in 0..m {
                let name = c.string()?;
                let v = read_prop(c)?;
                if let Some(p) = self.props(id) {
                    set_prop(p, name, v);
                }
            }
        }
        Ok(())
    }

    fn push_run(&mut self, id: usize, scale_id: Option<u32>, mut r: Run) -> Result<()> {
        let Kind::Channel(g, c) = self.objs[id].kind else {
            return Ok(());
        };
        if r.count == 0 || r.repeats == 0 {
            return Ok(());
        }
        if r.repeats > 1 && r.elem != Elem::Str && r.step == r.count * r.stride {
            r.count *= r.repeats;
            r.repeats = 1;
        }
        self.runs += 1;
        if self.runs > MAX_RUNS {
            return Err(Error::TooLarge);
        }
        let ch = &mut self.t.groups[g].channels[c];
        r.first = ch.len;
        match scale_id {
            None => ch.runs.push(r),
            Some(sid) => {
                if ch.runs.is_empty() || ch.scalers.first().is_some_and(|s| s.0 == sid) {
                    ch.runs.push(r);
                }
                match ch.scalers.iter_mut().find(|s| s.0 == sid) {
                    Some(s) => s.1.push(r),
                    None => ch.scalers.push((sid, vec![r])),
                }
            }
        }
        Ok(())
    }

    fn finish_len(&mut self, id: usize, n: u64) {
        if let Kind::Channel(g, c) = self.objs[id].kind {
            self.t.groups[g].channels[c].len += n;
        }
    }

    /// Lays out the raw data of a segment: `raw` bytes at `start`.
    fn layout(
        &mut self,
        start: u64,
        raw: u64,
        interleaved: bool,
        be: bool,
        incomplete: bool,
    ) -> Result<()> {
        let items: Vec<(usize, Index)> = self
            .list
            .iter()
            .filter_map(|(id, ix)| ix.clone().map(|ix| (*id, ix)))
            .collect();
        if items.is_empty() || raw == 0 {
            return Ok(());
        }
        let ovf = || Error::Malformed("raw data size".into());
        if items
            .iter()
            .any(|(_, ix)| matches!(ix, Index::Daqmx { .. }))
        {
            return self.layout_daqmx(start, raw, &items, be);
        }
        let plain = |ix: &Index| match ix {
            Index::Plain {
                dtype,
                count,
                bytes,
            } => (*dtype, *count, *bytes),
            Index::Daqmx { .. } => (DataType::Other(0), 0, 0),
        };
        if interleaved {
            let mut stride = 0u64;
            let first = plain(&items[0].1).1;
            for (_, ix) in &items {
                let (dt, count, _) = plain(ix);
                let e = dt.elem().unwrap_or(Elem::Str);
                if e == Elem::Str {
                    return bad("strings cannot be interleaved");
                }
                if count != first {
                    return bad("interleaved channels of different lengths");
                }
                stride += e.size();
            }
            // Interleaved chunks follow each other without gaps: one run per channel.
            let take = if first == 0 { 0 } else { raw / stride };
            let mut off = 0;
            for (id, ix) in &items {
                let (dt, _, _) = plain(ix);
                let e = dt.elem().unwrap_or(Elem::U8);
                self.push_run(
                    *id,
                    None,
                    Run {
                        first: 0,
                        count: take,
                        offset: start + off,
                        stride,
                        repeats: 1,
                        step: 0,
                        elem: e,
                        be,
                    },
                )?;
                self.finish_len(*id, take);
                off += e.size();
            }
            return Ok(());
        }
        let mut chunk = 0u64;
        for (_, ix) in &items {
            chunk = chunk.checked_add(plain(ix).2).ok_or_else(ovf)?;
        }
        if chunk == 0 {
            return Ok(());
        }
        let full = raw / chunk;
        let mut rem = raw % chunk;
        let mut off = 0u64;
        for (id, ix) in &items {
            let (dt, count, bytes) = plain(ix);
            let e = dt.elem().unwrap_or(Elem::U8);
            let stride = e.size();
            self.push_run(
                *id,
                None,
                Run {
                    first: 0,
                    count,
                    offset: start + off,
                    stride,
                    repeats: full,
                    step: chunk,
                    elem: e,
                    be,
                },
            )?;
            self.finish_len(*id, count * full);
            off += bytes;
        }
        // A final chunk cut short (as NI's readers take it): nothing when some object holds
        // strings; in a segment cut short, each object in turn gets what fits; otherwise each
        // gets its share of the remainder. The objects' data follow each other.
        if rem == 0 || items.iter().any(|(_, ix)| plain(ix).0 == DataType::String) {
            return Ok(());
        }
        let base = start + full * chunk;
        let mut off = 0u64;
        for (id, ix) in &items {
            let (dt, count, bytes) = plain(ix);
            let e = dt.elem().unwrap_or(Elem::U8);
            let n = if !incomplete {
                ((u128::from(count) * u128::from(rem)) / u128::from(chunk)) as u64
            } else if rem > bytes {
                rem -= bytes;
                count
            } else {
                let n = rem / e.size();
                rem = 0;
                n
            };
            self.push_run(
                *id,
                None,
                Run {
                    first: 0,
                    count: n,
                    offset: base + off,
                    stride: e.size(),
                    repeats: 1,
                    step: 0,
                    elem: e,
                    be,
                },
            )?;
            self.finish_len(*id, n);
            off += n * e.size();
            if incomplete && rem == 0 {
                break;
            }
        }
        Ok(())
    }

    fn layout_daqmx(
        &mut self,
        start: u64,
        raw: u64,
        items: &[(usize, Index)],
        be: bool,
    ) -> Result<()> {
        let ovf = || Error::Malformed("DAQmx raw data size".into());
        // Buffers: (values, width), the values the most any scaler in it has.
        let mut dims: Option<Vec<(u64, u64)>> = None;
        for (_, ix) in items {
            let Index::Daqmx {
                count,
                scalers,
                widths,
            } = ix
            else {
                return Err(Error::Unsupported(
                    "TDMS segments mixing DAQmx and other raw data are not supported".into(),
                ));
            };
            let d = dims.get_or_insert_with(|| widths.iter().map(|&w| (0, u64::from(w))).collect());
            if d.len() != widths.len() || d.iter().zip(widths).any(|(a, &w)| a.1 != u64::from(w)) {
                return bad("DAQmx raw data widths differ between channels");
            }
            if scalers.is_empty() {
                return bad("a DAQmx channel without scalers");
            }
            for s in scalers {
                let b = d
                    .get_mut(s.buffer as usize)
                    .ok_or_else(|| Error::Malformed("DAQmx raw buffer index".into()))?;
                if u64::from(s.byte) + s.elem.size() > b.1 {
                    return bad("DAQmx scaler outside its raw buffer");
                }
                b.0 = b.0.max(*count);
            }
        }
        let dims = dims.unwrap_or_default();
        let mut chunk = 0u64;
        let mut starts = Vec::with_capacity(dims.len());
        for &(n, w) in &dims {
            starts.push(chunk);
            chunk = chunk
                .checked_add(n.checked_mul(w).ok_or_else(ovf)?)
                .ok_or_else(ovf)?;
        }
        if chunk == 0 {
            return Ok(());
        }
        let full = raw / chunk;
        let rem = raw % chunk;
        // Buffer lengths of the final chunk cut short.
        let mut last = vec![0u64; dims.len()];
        let mut left = rem;
        for (i, &(n, w)) in dims.iter().enumerate() {
            if left > n * w {
                last[i] = n;
                left -= n * w;
            } else {
                last[i] = left / w.max(1);
                break;
            }
        }
        for (id, ix) in items {
            let Index::Daqmx { count, scalers, .. } = ix else {
                continue;
            };
            let mut tail = None;
            for s in scalers {
                let b = s.buffer as usize;
                let w = dims[b].1;
                let r = Run {
                    first: 0,
                    count: *count,
                    offset: start + starts[b] + u64::from(s.byte),
                    stride: w,
                    repeats: full,
                    step: chunk,
                    elem: s.elem,
                    be,
                };
                self.push_run(*id, Some(s.scale_id), r)?;
                let n = if rem > 0 { last[b].min(*count) } else { 0 };
                tail = Some(tail.map_or(n, |t: u64| t.min(n)));
                if n > 0 {
                    let mut r = r;
                    r.count = n;
                    r.repeats = 1;
                    r.offset = start + full * chunk + starts[b] + u64::from(s.byte);
                    // Placed after the full chunks: `first` is set from the length below.
                    self.pending_tail(*id, s.scale_id, r, *count * full);
                }
            }
            self.finish_len(*id, *count * full + tail.unwrap_or(0));
        }
        Ok(())
    }

    /// A DAQmx scaler's run in a final chunk cut short, which starts `after` samples past
    /// the channel's current length.
    fn pending_tail(&mut self, id: usize, sid: u32, mut r: Run, after: u64) {
        let Kind::Channel(g, c) = self.objs[id].kind else {
            return;
        };
        let ch = &mut self.t.groups[g].channels[c];
        r.first = ch.len + after;
        if ch.scalers.first().is_some_and(|s| s.0 == sid) {
            ch.runs.push(r);
        }
        if let Some(s) = ch.scalers.iter_mut().find(|s| s.0 == sid) {
            s.1.push(r);
        }
    }
}

/// Reads the metadata of a TDMS file: groups, channels, properties and where every
/// channel's samples are.
pub fn parse(data: &[u8]) -> Result<Tdms> {
    if !sniff(data) || !data.starts_with(b"TDSm") {
        return Err(Error::NotTdms);
    }
    let mut st = State {
        t: Tdms::default(),
        objs: Vec::new(),
        by_path: HashMap::new(),
        list: Vec::new(),
        runs: 0,
    };
    let len = data.len();
    let mut pos = 0usize;
    while len - pos >= LEAD_IN {
        if &data[pos..pos + 4] != b"TDSm" {
            if pos == 0 {
                return Err(Error::NotTdms);
            }
            st.t.truncated = true;
            break;
        }
        let toc = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
        let be = toc & TOC_BIG_ENDIAN != 0;
        let mut c = Cur {
            d: data,
            p: pos + 8,
            end: pos + LEAD_IN,
            be,
        };
        let version = c.u32()?;
        let next_off = c.u64()?;
        let raw_off = c.u64()?;
        if !matches!(version, 4712 | 4713) {
            return Err(Error::Unsupported(format!(
                "TDMS version {version} is not supported"
            )));
        }
        let seg = pos + LEAD_IN;
        let (next, cut) = match usize::try_from(next_off)
            .ok()
            .and_then(|o| seg.checked_add(o))
        {
            Some(n) if next_off != u64::MAX && n <= len => (n, false),
            _ => (len, true),
        };
        let raw_start = usize::try_from(raw_off)
            .ok()
            .and_then(|o| seg.checked_add(o))
            .map_or(next, |r| r.min(next));
        if st.t.segments == 0 {
            st.t.version = version;
        }
        st.t.segments += 1;
        st.t.interleaved |= toc & TOC_INTERLEAVED != 0 && toc & TOC_RAW != 0;
        st.t.big_endian |= be;
        st.t.daqmx |= toc & TOC_DAQMX != 0;
        if toc & TOC_META != 0 {
            let mut c = Cur {
                d: data,
                p: seg,
                end: raw_start,
                be,
            };
            match st.metadata(&mut c, toc & TOC_NEW_LIST != 0 || pos == 0) {
                Ok(()) => {}
                Err(Error::Truncated(_)) if pos > 0 => {
                    st.t.truncated = true;
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        if toc & TOC_RAW != 0 {
            st.layout(
                raw_start as u64,
                (next - raw_start) as u64,
                toc & TOC_INTERLEAVED != 0,
                be,
                cut,
            )?;
        }
        if cut {
            st.t.truncated = true;
            break;
        }
        pos = next;
    }
    let file_props = st.t.properties.clone();
    for g in &mut st.t.groups {
        for ch in &mut g.channels {
            ch.scaling = scale::find_scaling([&ch.properties, &g.properties, &file_props]);
        }
    }
    Ok(st.t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        assert_eq!(parse_path("/"), Some(vec![]));
        assert_eq!(parse_path("/'a'"), Some(vec!["a".to_owned()]));
        assert_eq!(
            parse_path("/'it''s'/'b/c'"),
            Some(vec!["it's".to_owned(), "b/c".to_owned()])
        );
        assert_eq!(parse_path("/'a"), None);
        assert_eq!(parse_path("a"), None);
    }

    #[test]
    fn times() {
        assert_eq!(iso_time(EPOCH_1904, 0), "1970-01-01T00:00:00Z");
        assert_eq!(
            iso_time(EPOCH_1904 + 86_399, 1 << 63),
            "1970-01-01T23:59:59.5Z"
        );
        assert_eq!(iso_time(0, 0), "1904-01-01T00:00:00Z");
        assert_eq!(iso_time(EPOCH_1904 + 59, u64::MAX), "1970-01-01T00:01:00Z");
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(parse(b"TDSm"), Err(Error::NotTdms)));
        assert!(matches!(parse(&[0u8; 64]), Err(Error::NotTdms)));
        let mut d = b"TDSm\x0e\0\0\0\x69\x12\0\0".to_vec();
        d.extend_from_slice(&[0xff; 16]);
        d.extend_from_slice(&[0xff; 8]);
        // A huge object count in a short metadata block.
        assert!(parse(&d).is_err());
        for n in 0..d.len() {
            let _ = parse(&d[..n]);
        }
    }
}
