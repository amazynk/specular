//! Seismic traces: SEG-Y (rev 0 of 1975, rev 1 of 2002, rev 2.0 of 2017 and 2.1 of 2023)
//! and Seismic Unix `.su` files.
//!
//! A SEG-Y file is a 3200-byte textual header (40 card images, EBCDIC or ASCII), a
//! 400-byte binary header, optional 3200-byte extended textual header stanzas, then traces:
//! each a 240-byte trace header (rev 2: plus a fixed number of additional 240-byte
//! headers) and its samples. Everything is big-endian unless a rev 2 file says otherwise
//! with its byte-order word (binary header bytes 3297–3300), or the binary header only
//! makes sense read little-endian (files written by PCs that ignored the standard).
//!
//! A Seismic Unix file is the traces alone, IEEE floats in the byte order of the machine
//! that wrote it; the trace header's sample count says how long the traces are, and the
//! byte order is the one whose count tiles the file.
//!
//! Every trace must have the same number of samples: files with variable trace length
//! (allowed by rev 0 and 1 when the fixed-length flag is clear) are refused, by name.
//!
//! Header byte positions are 1-based as in the standard throughout (`189` is the inline
//! number of rev 1 and later).

mod geometry;
mod samples;
mod text;

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

pub use geometry::{Geometry, KEY_CANDIDATES, Keys, Volume};
pub use samples::{SampleFormat, ibm_to_f32};
pub use text::{ebcdic, is_ebcdic};

/// Size of the textual header and of each extended textual header stanza.
pub const TEXT_BYTES: usize = 3200;
/// Size of the binary header.
pub const BINARY_BYTES: usize = 400;
/// Size of a trace header.
pub const TRACE_HEADER: usize = 240;

#[derive(Debug, Error)]
pub enum Error {
    #[error("not a SEG-Y file")]
    NotSegy,
    #[error("not a Seismic Unix file")]
    NotSu,
    #[error("malformed SEG-Y: {0}")]
    Malformed(String),
    #[error("unsupported SEG-Y: {0}")]
    Unsupported(String),
    #[error(
        "SEG-Y with variable trace length is not supported (trace {trace} has {samples} samples, the first {first})"
    )]
    VariableLength {
        trace: usize,
        samples: usize,
        first: usize,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The memory budget cannot hold the values.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, Error>;

/// SEG-Y with file headers, or bare Seismic Unix traces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    SegY,
    Su,
}

/// Byte order of the binary header, trace headers and samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endian {
    Big,
    Little,
}

/// What the vertical axis measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    /// Two-way time; the sample interval is in microseconds.
    Time,
    /// Depth; the sample interval is in the file's measurement unit (metres or feet), in
    /// thousandths as most depth writers store it.
    Depth,
}

/// The fields of the 400-byte binary header that matter (rev 2 numbering).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Binary {
    pub job: i32,
    pub line: i32,
    pub reel: i32,
    pub traces_per_ensemble: i32,
    pub aux_per_ensemble: i32,
    /// Sample interval (µs for time data); the extended rev 2 interval when this is 0.
    pub interval: f64,
    pub original_interval: i32,
    /// Samples per trace; the extended rev 2 count when the 16-bit field is 0.
    pub samples: i64,
    /// The rev 2 32-bit sample count (bytes 3269-3272) as stored.
    pub extended_samples: i32,
    pub original_samples: i32,
    pub format_code: i16,
    pub fold: i32,
    /// Trace sorting code (4 = horizontally stacked, 2 = CDP ensemble, ...).
    pub sorting: i16,
    /// 1 metres, 2 feet.
    pub measurement: i16,
    /// The revision as stored (bytes 3501, 3502): (1, 0) for rev 1.0, (0, 0) for rev 0.
    pub revision: (u8, u8),
    pub fixed_length: bool,
    /// Extended textual header stanzas; -1 for "until the end stanza".
    pub extended_headers: i32,
    /// Additional 240-byte headers after each trace header (rev 2).
    pub extra_headers: i32,
    /// Traces in the file (rev 2 bytes 3513–3520), 0 when not stated.
    pub traces_in_file: u64,
    /// Byte offset of the first trace (rev 2 bytes 3521–3528), 0 when not stated.
    pub first_trace: u64,
    /// The rev 2 byte-order word was present.
    pub byte_order_word: bool,
}

/// Bytes shared with the caller (a memory map, a buffer).
pub type Shared = Arc<dyn AsRef<[u8]> + Send + Sync>;

/// Where the bytes are: a file read by position, or memory.
enum Store {
    File(File),
    Mem(Shared),
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Store::File(file) => f.debug_tuple("File").field(file).finish(),
            Store::Mem(m) => write!(f, "Mem({} bytes)", (**m).as_ref().len()),
        }
    }
}

/// Positioned read that fills `buf`. On Windows `seek_read` also moves the file's offset, which
/// nothing here relies on: every read of a `Store::File` is positioned.
fn read_exact_at(f: &File, buf: &mut [u8], off: u64) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::FileExt::read_exact_at(f, buf, off)
    }
    #[cfg(windows)]
    {
        let mut done = 0;
        while done < buf.len() {
            match std::os::windows::fs::FileExt::seek_read(f, &mut buf[done..], off + done as u64) {
                Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => done += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (f, buf, off);
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

impl Store {
    fn read_at(&self, off: u64, buf: &mut [u8]) -> Result<()> {
        match self {
            Store::File(f) => read_exact_at(f, buf, off)?,
            Store::Mem(m) => {
                let s = usize::try_from(off).unwrap_or(usize::MAX);
                let src = (**m)
                    .as_ref()
                    .get(s..s.saturating_add(buf.len()))
                    .ok_or_else(|| Error::Malformed("read past the end".into()))?;
                buf.copy_from_slice(src);
            }
        }
        Ok(())
    }
}

/// An opened SEG-Y or SU file: its headers, and access to traces by number.
#[derive(Debug)]
pub struct Segy {
    store: Store,
    /// File length in bytes.
    pub len: u64,
    pub kind: Kind,
    pub endian: Endian,
    /// The textual header as card images, and whether it was EBCDIC (SEG-Y only).
    pub text: Vec<String>,
    pub ebcdic: bool,
    /// Extended textual header stanzas, each as card images.
    pub extended: Vec<Vec<String>>,
    pub binary: Option<Binary>,
    pub format: SampleFormat,
    /// Samples per trace.
    pub samples: usize,
    /// Sample interval as stored (µs for time data; for depth data in thousandths of the
    /// unit), from the binary header or else the first trace header.
    pub interval: f64,
    /// Delay recording time of the first trace (bytes 109–110), ms (or depth units).
    pub delay: f64,
    pub domain: Domain,
    /// Byte offset of the first trace.
    pub data_offset: u64,
    /// Additional 240-byte headers per trace (rev 2).
    pub extra_headers: usize,
    /// Bytes per trace, headers included.
    pub trace_bytes: u64,
    pub traces: usize,
    /// Bytes after the last whole trace (a cut file, or rev 2 trailer stanzas).
    pub trailing: u64,
}

/// A trace header, with readers by 1-based byte position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceHeader {
    pub raw: [u8; TRACE_HEADER],
    pub big: bool,
}

impl TraceHeader {
    /// The 4-byte integer at 1-based byte `pos` (0 when out of range).
    pub fn i32(&self, pos: usize) -> i32 {
        let a = self
            .raw
            .get(pos.wrapping_sub(1)..pos + 3)
            .and_then(|s| <[u8; 4]>::try_from(s).ok())
            .unwrap_or([0; 4]);
        if self.big {
            i32::from_be_bytes(a)
        } else {
            i32::from_le_bytes(a)
        }
    }

    /// The 2-byte integer at 1-based byte `pos`.
    pub fn i16(&self, pos: usize) -> i16 {
        let a = self
            .raw
            .get(pos.wrapping_sub(1)..pos + 1)
            .and_then(|s| <[u8; 2]>::try_from(s).ok())
            .unwrap_or([0; 2]);
        if self.big {
            i16::from_be_bytes(a)
        } else {
            i16::from_le_bytes(a)
        }
    }

    /// The field at `pos` of `width` bytes (2 or 4), as an integer.
    pub fn get(&self, pos: usize, width: usize) -> i32 {
        if width == 2 && pos == 115 {
            // The sample count is unsigned (rev 2).
            i32::from(self.i16(pos) as u16)
        } else if width == 2 {
            i32::from(self.i16(pos))
        } else {
            self.i32(pos)
        }
    }

    /// A coordinate field scaled by the coordinate scalar (bytes 71–72: positive
    /// multiplies, negative divides, 0 is 1).
    pub fn coordinate(&self, pos: usize) -> f64 {
        apply_scalar(f64::from(self.i32(pos)), self.i16(71))
    }

    /// CDP X and Y (bytes 181, 185), scaled.
    pub fn cdp_xy(&self) -> (f64, f64) {
        (self.coordinate(181), self.coordinate(185))
    }

    /// Source X and Y (bytes 73, 77), scaled.
    pub fn source_xy(&self) -> (f64, f64) {
        (self.coordinate(73), self.coordinate(77))
    }
}

/// Applies a SEG-Y scalar: positive multiplies, negative divides, zero is one.
pub fn apply_scalar(v: f64, scalar: i16) -> f64 {
    match scalar {
        0 => v,
        s if s > 0 => v * f64::from(s),
        s => v / -f64::from(s),
    }
}

/// The trace header fields the standard defines, as (byte, width, name), for the info
/// panel and the oracle.
pub const TRACE_FIELDS: &[(usize, usize, &str)] = &[
    (1, 4, "Trace sequence number in line"),
    (5, 4, "Trace sequence number in file"),
    (9, 4, "Field record number"),
    (13, 4, "Trace number in field record"),
    (17, 4, "Energy source point number"),
    (21, 4, "CDP ensemble number"),
    (25, 4, "Trace number in ensemble"),
    (29, 2, "Trace identification code"),
    (37, 4, "Offset"),
    (41, 4, "Receiver group elevation"),
    (45, 4, "Surface elevation at source"),
    (69, 2, "Elevation scalar"),
    (71, 2, "Coordinate scalar"),
    (73, 4, "Source X"),
    (77, 4, "Source Y"),
    (81, 4, "Group X"),
    (85, 4, "Group Y"),
    (89, 2, "Coordinate units"),
    (109, 2, "Delay recording time"),
    (115, 2, "Samples in this trace"),
    (117, 2, "Sample interval"),
    (157, 2, "Year"),
    (159, 2, "Day of year"),
    (161, 2, "Hour"),
    (163, 2, "Minute"),
    (165, 2, "Second"),
    (181, 4, "CDP X"),
    (185, 4, "CDP Y"),
    (189, 4, "Inline number"),
    (193, 4, "Crossline number"),
    (197, 4, "Shotpoint number"),
    (201, 2, "Shotpoint scalar"),
];

/// Whether `head` (at least the first 3600 bytes) looks like SEG-Y: a binary header with a
/// known sample format, a sample count and a sample interval in one byte order, behind a
/// textual header that reads as text.
pub fn sniff(head: &[u8]) -> bool {
    if head.len() < TEXT_BYTES + BINARY_BYTES {
        return false;
    }
    if !plausible_binary(&head[TEXT_BYTES..TEXT_BYTES + BINARY_BYTES]) {
        return false;
    }
    // Card images: the first column of most cards is 'C' in EBCDIC (0xC3) or ASCII.
    let cards = head[..TEXT_BYTES]
        .chunks(80)
        .filter(|c| c[0] == 0xC3 || c[0] == b'C')
        .count();
    let texty = head[..TEXT_BYTES]
        .iter()
        .filter(|&&b| b == 0x40 || b == b' ' || b.is_ascii_alphanumeric() || b >= 0x81)
        .count();
    cards >= 10 || texty > TEXT_BYTES * 3 / 4
}

/// A binary header with a known sample format (or the obsolete 4) and a sample count, in
/// either byte order.
fn plausible_binary(bin: &[u8]) -> bool {
    let plausible = |big: bool| {
        let fmt = rd16(bin, 25, big);
        let ns_ok = rd16(bin, 21, big) != 0 || rd32(bin, 69, big) > 0;
        (SampleFormat::from_code(i64::from(fmt)).is_some() || fmt == 4) && ns_ok
    };
    plausible(true) || plausible(false)
}

/// Reads a 2-byte integer at 1-based `pos` within a binary header slice (positions
/// relative to byte 3201, so `pos` 25 is file byte 3225).
fn rd16(b: &[u8], pos: usize, big: bool) -> i16 {
    let a = [b[pos - 1], b[pos]];
    if big {
        i16::from_be_bytes(a)
    } else {
        i16::from_le_bytes(a)
    }
}

fn rd32(b: &[u8], pos: usize, big: bool) -> i32 {
    let a = [b[pos - 1], b[pos], b[pos + 1], b[pos + 2]];
    if big {
        i32::from_be_bytes(a)
    } else {
        i32::from_le_bytes(a)
    }
}

fn rd64(b: &[u8], pos: usize, big: bool) -> u64 {
    let a: [u8; 8] = b[pos - 1..pos + 7].try_into().unwrap_or([0; 8]);
    if big {
        u64::from_be_bytes(a)
    } else {
        u64::from_le_bytes(a)
    }
}

fn parse_binary(b: &[u8], big: bool) -> Binary {
    let ext_ns = rd32(b, 69, big);
    let ns16 = rd16(b, 21, big) as u16;
    let samples = if ns16 == 0 && ext_ns > 0 {
        i64::from(ext_ns)
    } else {
        i64::from(ns16)
    };
    let dt16 = rd16(b, 17, big) as u16;
    let ext_dt = f64::from_bits(rd64(b, 73, big));
    let interval = if dt16 == 0 && ext_dt.is_finite() && ext_dt > 0.0 {
        ext_dt
    } else {
        f64::from(dt16)
    };
    let wide = |narrow: i16, wide: i32| if narrow == 0 { wide } else { i32::from(narrow) };
    Binary {
        job: rd32(b, 1, big),
        line: rd32(b, 5, big),
        reel: rd32(b, 9, big),
        traces_per_ensemble: wide(rd16(b, 13, big), rd32(b, 61, big)),
        aux_per_ensemble: wide(rd16(b, 15, big), rd32(b, 65, big)),
        interval,
        original_interval: i32::from(rd16(b, 19, big) as u16),
        samples,
        extended_samples: ext_ns,
        original_samples: wide(rd16(b, 23, big), rd32(b, 89, big)),
        format_code: rd16(b, 25, big),
        fold: wide(rd16(b, 27, big), rd32(b, 93, big)),
        sorting: rd16(b, 29, big),
        measurement: rd16(b, 55, big),
        revision: (b[300], b[301]),
        fixed_length: rd16(b, 303, big) == 1,
        extended_headers: i32::from(rd16(b, 305, big)),
        extra_headers: rd32(b, 307, big),
        traces_in_file: rd64(b, 313, big),
        first_trace: rd64(b, 321, big),
        byte_order_word: rd32(b, 97, big) == 0x0102_0304,
    }
}

impl Segy {
    /// Opens a SEG-Y file (`su` false) or a Seismic Unix file.
    pub fn open(path: &Path, su: bool) -> Result<Segy> {
        let f = File::open(path)?;
        let len = f.metadata()?.len();
        let store = Store::File(f);
        if su {
            Segy::parse_su(store, len)
        } else {
            Segy::parse_segy(store, len)
        }
    }

    /// Opens a file by content: SEG-Y when it sniffs as one, else Seismic Unix.
    pub fn open_any(path: &Path) -> Result<Segy> {
        let f = File::open(path)?;
        let len = f.metadata()?.len();
        let mut head = vec![0; (TEXT_BYTES + BINARY_BYTES).min(len as usize)];
        read_exact_at(&f, &mut head, 0)?;
        let store = Store::File(f);
        if sniff(&head) {
            Segy::parse_segy(store, len)
        } else {
            Segy::parse_su(store, len)
        }
    }

    /// Parses SEG-Y from memory.
    pub fn from_bytes(bytes: impl Into<Arc<[u8]>>) -> Result<Segy> {
        let m: Arc<[u8]> = bytes.into();
        Segy::from_shared(Arc::new(m), false)
    }

    /// Parses Seismic Unix traces from memory.
    pub fn su_from_bytes(bytes: impl Into<Arc<[u8]>>) -> Result<Segy> {
        let m: Arc<[u8]> = bytes.into();
        Segy::from_shared(Arc::new(m), true)
    }

    /// Parses SEG-Y (`su` false) or Seismic Unix traces from shared bytes (a memory map),
    /// which are read in place.
    pub fn from_shared(bytes: Shared, su: bool) -> Result<Segy> {
        let len = (*bytes).as_ref().len() as u64;
        if su {
            Segy::parse_su(Store::Mem(bytes), len)
        } else {
            Segy::parse_segy(Store::Mem(bytes), len)
        }
    }

    fn parse_segy(store: Store, len: u64) -> Result<Segy> {
        let head_len = TEXT_BYTES + BINARY_BYTES;
        if len < head_len as u64 {
            return Err(Error::NotSegy);
        }
        let mut head = vec![0; head_len];
        store.read_at(0, &mut head)?;
        if !plausible_binary(&head[TEXT_BYTES..]) {
            return Err(Error::NotSegy);
        }
        let bin = &head[TEXT_BYTES..];
        // Byte order: the rev 2 word says; else the reading with a known format code and a
        // sane sample count (big-endian first, as the standard asks).
        let big = if rd32(bin, 97, true) == 0x0102_0304 {
            true
        } else if rd32(bin, 97, false) == 0x0102_0304 {
            false
        } else {
            let ok = |big: bool| {
                SampleFormat::from_code(i64::from(rd16(bin, 25, big))).is_some()
                    && (rd16(bin, 21, big) != 0 || rd32(bin, 69, big) > 0)
            };
            ok(true) || !ok(false)
        };
        let binary = parse_binary(bin, big);
        let format = SampleFormat::from_code(i64::from(binary.format_code)).ok_or_else(|| {
            if binary.format_code == 4 {
                Error::Unsupported("fixed-point samples with gain (format 4)".into())
            } else {
                Error::Unsupported(format!("sample format {}", binary.format_code))
            }
        })?;
        let ebcdic = text::is_ebcdic(&head[..TEXT_BYTES]);
        let text = text::decode(&head[..TEXT_BYTES], ebcdic);

        // Only revisions 1 and 2 have the rev 2 fields; anything else in bytes 3501/3502
        // (junk, or a field-limit test) reads as rev 0.
        let rev_major = match binary.revision.0 {
            m @ (1 | 2) => m,
            _ => 0,
        };
        // Extended textual headers: honoured whatever the revision says (rev 0 writers set
        // the count too), when the first stanza reads as text; -1 means "until EndText".
        let mut offset = head_len as u64;
        let mut extended = Vec::new();
        let stanza_texty = |off: u64| -> Result<bool> {
            if off + TEXT_BYTES as u64 > len {
                return Ok(false);
            }
            let mut st = vec![0u8; TEXT_BYTES];
            store.read_at(off, &mut st)?;
            // A rev 2 stanza header "((" in ASCII or EBCDIC (its data may be binary), or
            // text with no control codes to speak of; a trace header (what a stale count
            // would point at) is mostly zero bytes.
            let stanza = st.starts_with(b"((") || st.starts_with(&[0x4d, 0x4d]);
            let binary = st.iter().filter(|&&b| b < 0x20).count();
            Ok(stanza || binary < TEXT_BYTES / 20)
        };
        let count = binary.extended_headers;
        if (count > 0 || count == -1) && (rev_major >= 1 && count > 0 || stanza_texty(offset)?) {
            let mut stanza = vec![0u8; TEXT_BYTES];
            let mut n = 0i32;
            loop {
                if binary.extended_headers > 0 && n >= binary.extended_headers {
                    break;
                }
                if n >= 10_000 || offset + TEXT_BYTES as u64 > len {
                    if binary.extended_headers > 0 {
                        return Err(Error::Malformed(
                            "extended textual headers past the end".into(),
                        ));
                    }
                    break;
                }
                store.read_at(offset, &mut stanza)?;
                offset += TEXT_BYTES as u64;
                n += 1;
                let lines = text::decode(&stanza, ebcdic && text::is_ebcdic(&stanza));
                let end = lines
                    .first()
                    .is_some_and(|l| l.replace(' ', "").eq_ignore_ascii_case("((SEG:EndText))"));
                if end {
                    break;
                }
                extended.push(lines);
            }
        }
        if rev_major >= 2 && binary.first_trace > 0 && binary.first_trace >= head_len as u64 {
            // Clamped to the file so later `offset + ...` sums cannot overflow; one past
            // the end leaves no traces.
            offset = binary.first_trace.min(len);
        }
        let mut first = TraceHeader {
            raw: [0; TRACE_HEADER],
            big,
        };
        if offset + TRACE_HEADER as u64 <= len {
            store.read_at(offset, &mut first.raw)?;
        }
        let trace_ns = usize::from(first.i16(115) as u16);
        let bin_ns = usize::try_from(binary.samples.max(0)).unwrap_or(0);
        let ext_ns = usize::try_from(binary.extended_samples.max(0)).unwrap_or(0);
        let data = len.saturating_sub(offset);
        // Additional trace headers (rev 2): the 32-bit count, or a 16-bit one at 3507 as
        // some writers store it (65536 for 1 read as 32 bits), whichever tiles the data.
        let extras: Vec<usize> = if rev_major >= 2 {
            let wide = binary.extra_headers;
            let narrow = i32::from(rd16(bin, 307, big));
            [wide, narrow]
                .into_iter()
                .filter(|n| (1..=1000).contains(n))
                .map(|n| n as usize)
                .collect()
        } else {
            Vec::new()
        };
        let size_with =
            |extra: usize, ns: usize| ((1 + extra) * TRACE_HEADER + ns * format.size()) as u64;
        // The binary header's count when it tiles the data; else the first trace's (as
        // segyio does); else the rev 2 32-bit count; else whichever is set, and a
        // variable-length check.
        let tiles = |extra: usize, ns: usize| ns > 0 && data.is_multiple_of(size_with(extra, ns));
        let pick = |extra: usize| {
            [bin_ns, trace_ns, ext_ns]
                .into_iter()
                .find(|&ns| tiles(extra, ns))
        };
        let (extra, samples) = extras
            .iter()
            .chain(&[0])
            .find_map(|&e| pick(e).map(|ns| (e, ns)))
            .unwrap_or_else(|| {
                let e = extras.first().copied().unwrap_or(0);
                (e, if bin_ns > 0 { bin_ns } else { trace_ns })
            });
        let size = |ns: usize| size_with(extra, ns);
        if samples == 0 {
            return Err(Error::Malformed("no samples per trace".into()));
        }
        let mut segy = Segy {
            store,
            len,
            kind: Kind::SegY,
            endian: if big { Endian::Big } else { Endian::Little },
            text,
            ebcdic,
            extended,
            binary: None,
            format,
            samples,
            interval: 0.0,
            delay: scaled_delay(&first),
            domain: Domain::Time,
            data_offset: offset,
            extra_headers: extra,
            trace_bytes: size(samples),
            traces: (data / size(samples)) as usize,
            trailing: data % size(samples),
        };
        if segy.trailing != 0 && !(rev_major >= 2 && binary.traces_in_file > 0) {
            segy.check_fixed_length(samples)?;
        }
        if rev_major >= 2 && binary.traces_in_file > 0 {
            let stated = usize::try_from(binary.traces_in_file).unwrap_or(usize::MAX);
            segy.traces = segy.traces.min(stated);
        }
        let trace_dt = f64::from(first.i16(117) as u16);
        segy.interval = if binary.interval > 0.0 {
            binary.interval
        } else {
            trace_dt
        };
        segy.domain = domain_of(&segy.text, &segy.extended);
        segy.binary = Some(binary);
        Ok(segy)
    }

    /// Walks the trace headers when the traces do not tile the file: a trace whose own
    /// sample count differs and whose traces then tile is variable length.
    fn check_fixed_length(&self, samples: usize) -> Result<()> {
        let mut off = self.data_offset;
        let mut h = TraceHeader {
            raw: [0; TRACE_HEADER],
            big: self.endian == Endian::Big,
        };
        let hdrs = ((1 + self.extra_headers) * TRACE_HEADER) as u64;
        let mut i = 0;
        while off + hdrs <= self.len && i < 100_000 {
            self.store.read_at(off, &mut h.raw)?;
            let ns = usize::from(h.i16(115) as u16);
            if ns != samples && ns > 0 {
                // Confirm: the stated lengths walk exactly to the end.
                let mut o = off;
                let mut ok = false;
                while o + hdrs <= self.len {
                    self.store.read_at(o, &mut h.raw)?;
                    let n = u64::from(h.i16(115) as u16);
                    o += hdrs + n * self.format.size() as u64;
                    if o == self.len {
                        ok = true;
                        break;
                    }
                }
                if ok {
                    return Err(Error::VariableLength {
                        trace: i,
                        samples: ns,
                        first: samples,
                    });
                }
                return Ok(());
            }
            off += self.trace_bytes;
            i += 1;
        }
        Ok(())
    }

    fn parse_su(store: Store, len: u64) -> Result<Segy> {
        let mut first = [0u8; TRACE_HEADER];
        if len < TRACE_HEADER as u64 {
            return Err(Error::NotSu);
        }
        store.read_at(0, &mut first)?;
        // The byte order whose sample count tiles the file; the machine's own first.
        let native_big = cfg!(target_endian = "big");
        let mut pick = None;
        for big in [native_big, !native_big] {
            let h = TraceHeader { raw: first, big };
            let ns = usize::from(h.i16(115) as u16);
            let size = (TRACE_HEADER + ns * 4) as u64;
            if ns > 0 && len.is_multiple_of(size) {
                pick = Some((big, ns, h));
                break;
            }
        }
        let (big, ns, h) = pick.ok_or(Error::NotSu)?;
        let size = (TRACE_HEADER + ns * 4) as u64;
        Ok(Segy {
            store,
            len,
            kind: Kind::Su,
            endian: if big { Endian::Big } else { Endian::Little },
            text: Vec::new(),
            ebcdic: false,
            extended: Vec::new(),
            binary: None,
            format: SampleFormat::F32,
            samples: ns,
            interval: f64::from(h.i16(117) as u16),
            delay: scaled_delay(&h),
            domain: Domain::Time,
            data_offset: 0,
            extra_headers: 0,
            trace_bytes: size,
            traces: (len / size) as usize,
            trailing: 0,
        })
    }

    pub fn big(&self) -> bool {
        self.endian == Endian::Big
    }

    fn trace_offset(&self, i: usize) -> u64 {
        self.data_offset + i as u64 * self.trace_bytes
    }

    /// The header of trace `i`.
    pub fn header(&self, i: usize) -> Result<TraceHeader> {
        let mut h = TraceHeader {
            raw: [0; TRACE_HEADER],
            big: self.big(),
        };
        if i >= self.traces {
            return Err(Error::Malformed(format!("no trace {i}")));
        }
        self.store.read_at(self.trace_offset(i), &mut h.raw)?;
        Ok(h)
    }

    /// The samples of trace `i` into `out` (at least [`Segy::samples`] long).
    pub fn trace_into(&self, i: usize, out: &mut [f32], scratch: &mut Vec<u8>) -> Result<()> {
        if i >= self.traces {
            return Err(Error::Malformed(format!("no trace {i}")));
        }
        let n = self.samples * self.format.size();
        scratch.resize(n, 0);
        let off = self.trace_offset(i) + ((1 + self.extra_headers) * TRACE_HEADER) as u64;
        self.store.read_at(off, scratch)?;
        samples::decode(self.format, self.big(), scratch, &mut out[..self.samples]);
        Ok(())
    }

    /// The samples of trace `i`.
    pub fn trace(&self, i: usize) -> Result<Vec<f32>> {
        if i >= self.traces {
            return Err(Error::Malformed(format!("no trace {i}")));
        }
        let mut out = vec![0.0; self.samples];
        self.trace_into(i, &mut out, &mut Vec::new())?;
        Ok(out)
    }

    /// Reads the headers of traces `range` in one read (or a few), calling `f` for each.
    pub fn headers(
        &self,
        range: std::ops::Range<usize>,
        mut f: impl FnMut(usize, &TraceHeader),
    ) -> Result<()> {
        let range = range.start..range.end.min(self.traces);
        let tb = self.trace_bytes as usize;
        // Batches of about 4 MB.
        let per = (4 << 20) / tb.max(1) + 1;
        let mut buf = Vec::new();
        let mut h = TraceHeader {
            raw: [0; TRACE_HEADER],
            big: self.big(),
        };
        let mut i = range.start;
        while i < range.end {
            let n = per.min(range.end - i);
            if tb > 16 * TRACE_HEADER {
                // Long traces: read the headers alone.
                for k in i..i + n {
                    self.store.read_at(self.trace_offset(k), &mut h.raw)?;
                    f(k, &h);
                }
            } else {
                buf.resize((n - 1) * tb + TRACE_HEADER, 0);
                self.store.read_at(self.trace_offset(i), &mut buf)?;
                for k in 0..n {
                    h.raw.copy_from_slice(&buf[k * tb..k * tb + TRACE_HEADER]);
                    f(i + k, &h);
                }
            }
            i += n;
        }
        Ok(())
    }

    /// Reads traces `range` into `out` (`samples` values per trace, in order).
    pub fn traces_into(&self, range: std::ops::Range<usize>, out: &mut [f32]) -> Result<()> {
        let range = range.start..range.end.min(self.traces);
        let ns = self.samples;
        let tb = self.trace_bytes as usize;
        let skip = (1 + self.extra_headers) * TRACE_HEADER;
        let per = ((8 << 20) / tb.max(1)).max(1);
        let mut buf = Vec::new();
        let mut i = range.start;
        while i < range.end {
            let n = per.min(range.end - i);
            buf.resize(n * tb, 0);
            self.store.read_at(self.trace_offset(i), &mut buf)?;
            for k in 0..n {
                let o = (i - range.start + k) * ns;
                samples::decode(
                    self.format,
                    self.big(),
                    &buf[k * tb + skip..(k + 1) * tb],
                    &mut out[o..o + ns],
                );
            }
            i += n;
        }
        Ok(())
    }

    /// The time (or depth) of sample `k`: delay + k × interval, in ms (or units).
    pub fn sample_position(&self, k: f64) -> f64 {
        self.delay + k * self.interval / 1000.0
    }

    /// The revision as "0", "1.0", "2.1"; `None` for SU.
    pub fn revision(&self) -> Option<String> {
        let b = self.binary.as_ref()?;
        Some(match b.revision {
            (0, _) => "0".to_owned(),
            (m, n) => format!("{m}.{n}"),
        })
    }

    /// The length unit of coordinates and depths ("m", "ft"), when stated.
    pub fn length_unit(&self) -> Option<&'static str> {
        match self.binary.as_ref()?.measurement {
            1 => Some("m"),
            2 => Some("ft"),
            _ => None,
        }
    }

    /// Finds the geometry: a 3-D post-stack volume from inline / crossline header bytes,
    /// else a 2-D line.
    pub fn geometry(&self) -> Result<Geometry> {
        geometry::detect(self)
    }
}

/// Depth data, by what the textual header says: a card naming a depth domain or depth
/// samples, and none naming a time one.
/// The delay recording time (bytes 109-110, ms) times the rev 2 scalar at 215 (a positive
/// scalar multiplies, a negative one divides; 0 means 1).
fn scaled_delay(h: &TraceHeader) -> f64 {
    let delay = f64::from(h.i16(109));
    match h.i16(215) {
        0 => delay,
        s if s > 0 => delay * f64::from(s),
        s => delay / -f64::from(s),
    }
}

fn domain_of(text: &[String], extended: &[Vec<String>]) -> Domain {
    let mut depth = false;
    for l in text.iter().chain(extended.iter().flatten()) {
        let u = l.to_ascii_uppercase();
        let words = |w: &[&str]| w.iter().any(|w| u.contains(w));
        if words(&[
            "DEPTH DOMAIN",
            "DEPTH MIGRAT",
            "DEPTH CONVERTED",
            "DOMAIN: DEPTH",
            "DOMAIN DEPTH",
            "Z DOMAIN: DEPTH",
            "PSDM",
        ]) {
            depth = true;
        }
        if words(&["TIME DOMAIN", "DOMAIN: TIME", "DOMAIN TIME", "PSTM"]) {
            return Domain::Time;
        }
    }
    if depth { Domain::Depth } else { Domain::Time }
}

#[cfg(test)]
mod tests;
