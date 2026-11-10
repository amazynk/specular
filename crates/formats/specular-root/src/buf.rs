//! A big-endian cursor over one object's bytes and ROOT's streaming conventions: versions
//! with byte counts, TStrings, TObject and TNamed headers, and `ReadObjectAny` class tags.

use std::collections::HashMap;

use crate::{Error, Result};

/// A byte count is flagged by this bit in the leading word.
const BYTE_COUNT: u32 = 0x4000_0000;
const NEW_CLASS: u32 = 0xFFFF_FFFF;
const CLASS_MASK: u32 = 0x8000_0000;
/// ROOT numbers class references from the start of the key plus this.
const MAP_OFFSET: usize = 2;
/// TObject::fBits: a referenced object carries a process id.
const IS_REFERENCED: u32 = 1 << 4;

pub(crate) struct Cur<'a> {
    pub(crate) b: &'a [u8],
    pub(crate) p: usize,
    /// Where the buffer sits after its key header (fKeylen), so class references, which
    /// count from the start of the key, resolve.
    pub(crate) origin: usize,
    /// Class names by reference tag.
    pub(crate) classes: HashMap<u32, String>,
}

/// One `ReadObjectAny` slot.
pub(crate) enum Any {
    Null,
    /// A reference to an object read earlier.
    Ref,
    /// A new object of this class; its bytes start at the cursor and end at `end` when the
    /// writer recorded a byte count.
    Obj {
        class: String,
        end: Option<usize>,
    },
}

macro_rules! be {
    ($name:ident, $t:ty) => {
        pub(crate) fn $name(&mut self) -> Result<$t> {
            const N: usize = std::mem::size_of::<$t>();
            let s = self.take(N)?;
            let mut a = [0u8; N];
            a.copy_from_slice(s);
            Ok(<$t>::from_be_bytes(a))
        }
    };
}

impl<'a> Cur<'a> {
    pub(crate) fn new(b: &'a [u8], origin: usize) -> Cur<'a> {
        Cur {
            b,
            p: 0,
            origin,
            classes: HashMap::new(),
        }
    }

    pub(crate) fn at(b: &'a [u8], p: usize) -> Cur<'a> {
        Cur {
            b,
            p,
            origin: 0,
            classes: HashMap::new(),
        }
    }

    pub(crate) fn left(&self) -> usize {
        self.b.len().saturating_sub(self.p)
    }

    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.p.checked_add(n).filter(|&e| e <= self.b.len());
        let Some(end) = end else {
            return Err(Error::Truncated(format!(
                "{n} bytes wanted at {} of {}",
                self.p,
                self.b.len()
            )));
        };
        let s = &self.b[self.p..end];
        self.p = end;
        Ok(s)
    }

    pub(crate) fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }

    /// Moves to `end` (from a byte count), which must lie inside the buffer and not behind
    /// what was already read.
    pub(crate) fn seek_end(&mut self, end: usize, what: &str) -> Result<()> {
        if end > self.b.len() {
            return Err(Error::Truncated(format!("{what} ends past its buffer")));
        }
        if end < self.p {
            return Err(Error::Corrupt(format!(
                "{what} is longer than its byte count"
            )));
        }
        self.p = end;
        Ok(())
    }

    be!(u8, u8);
    be!(u16, u16);
    be!(i16, i16);
    be!(u32, u32);
    be!(i32, i32);
    be!(i64, i64);
    be!(f64, f64);

    /// A seek or count stored as 32 or 64 bits.
    pub(crate) fn seek(&mut self, wide: bool) -> Result<u64> {
        if wide {
            let v = self.i64()?;
            u64::try_from(v).map_err(|_| Error::Corrupt(format!("negative seek {v}")))
        } else {
            let v = self.i32()?;
            u64::try_from(v).map_err(|_| Error::Corrupt(format!("negative seek {v}")))
        }
    }

    /// A TString: a length byte (255: a 32-bit length follows), then the bytes.
    pub(crate) fn tstring(&mut self) -> Result<String> {
        let n = match self.u8()? {
            255 => {
                let n = self.i32()?;
                usize::try_from(n).map_err(|_| Error::Corrupt(format!("string length {n}")))?
            }
            n => n as usize,
        };
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }

    /// A NUL-terminated string (class names after a new-class tag).
    pub(crate) fn cstring(&mut self) -> Result<String> {
        let rest = &self.b[self.p.min(self.b.len())..];
        let Some(n) = rest.iter().take(1024).position(|&c| c == 0) else {
            return Err(Error::Corrupt("class name without an end".into()));
        };
        let s = String::from_utf8_lossy(&rest[..n]).into_owned();
        self.p += n + 1;
        Ok(s)
    }

    /// A class version, with the end of the object when a byte count precedes it.
    pub(crate) fn version(&mut self) -> Result<(i16, Option<usize>)> {
        let start = self.p;
        let w = self.u32()?;
        if w & BYTE_COUNT != 0 {
            let n = (w & !BYTE_COUNT) as usize;
            let v = self.i16()?;
            let end = start + 4 + n;
            if end > self.b.len() {
                return Err(Error::Truncated(format!(
                    "an object of {n} bytes at {start} of {}",
                    self.b.len()
                )));
            }
            Ok((v, Some(end)))
        } else {
            self.p = start;
            Ok((self.i16()?, None))
        }
    }

    /// A class version that must lie in `known` (inclusive), with the object's end.
    pub(crate) fn version_of(
        &mut self,
        class: &str,
        known: (i16, i16),
    ) -> Result<(i16, Option<usize>)> {
        let (v, end) = self.version()?;
        if v < known.0 || v > known.1 {
            return Err(Error::Version {
                class: class.to_owned(),
                version: v,
                known: if known.0 == known.1 {
                    format!("{}", known.0)
                } else {
                    format!("{} to {}", known.0, known.1)
                },
            });
        }
        Ok((v, end))
    }

    /// Skips a versioned part (a base class whose content is not needed) by its byte count,
    /// or, for the attribute classes written without one (Geant4's g4tools), by the size of
    /// the version's members.
    pub(crate) fn skip_versioned(&mut self, what: &str) -> Result<()> {
        match self.version()? {
            (_, Some(end)) => self.seek_end(end, what),
            (v, None) => {
                let n = match (what, v) {
                    // fLineColor, fLineStyle, fLineWidth.
                    ("TAttLine", 1 | 2) => 6,
                    // fFillColor, fFillStyle.
                    ("TAttFill", 1 | 2) => 4,
                    // fMarkerColor, fMarkerStyle, fMarkerSize.
                    ("TAttMarker", 2) => 8,
                    // fNdivisions, three colours and a font, five floats, a colour and a font.
                    ("TAttAxis", 4) => 34,
                    ("TAtt3D", 1) => 0,
                    _ => {
                        return Err(Error::Unsupported(format!(
                            "{what} version {v} without a byte count"
                        )));
                    }
                };
                self.skip(n)
            }
        }
    }

    /// TObject: version, fUniqueID, fBits (and the process id of a referenced object).
    /// Gives fUniqueID.
    pub(crate) fn tobject(&mut self) -> Result<u32> {
        let (_, end) = self.version()?;
        let id = self.u32()?;
        let bits = self.u32()?;
        if bits & IS_REFERENCED != 0 {
            self.skip(2)?;
        }
        if let Some(end) = end {
            self.seek_end(end, "TObject")?;
        }
        Ok(id)
    }

    /// TNamed: TObject, fName and fTitle.
    pub(crate) fn tnamed(&mut self) -> Result<(String, String)> {
        let (_, end) = self.version()?;
        self.tobject()?;
        let name = self.tstring()?;
        let title = self.tstring()?;
        if let Some(end) = end {
            self.seek_end(end, "TNamed")?;
        }
        Ok((name, title))
    }

    /// One `ReadObjectAny` header: a null pointer, a reference to an earlier object, or a
    /// new object with its class (new, or a reference to a class named earlier).
    pub(crate) fn any(&mut self) -> Result<Any> {
        let beg = self.p;
        let first = self.u32()?;
        let (tag, start, end) = if first & BYTE_COUNT == 0 || first == NEW_CLASS {
            (first, None, None)
        } else {
            let start = self.p;
            let end = beg + 4 + (first & !BYTE_COUNT) as usize;
            if end > self.b.len() {
                return Err(Error::Truncated("an object pointer past its buffer".into()));
            }
            (self.u32()?, Some(start), Some(end))
        };
        if tag & CLASS_MASK == 0 {
            if let Some(end) = end {
                self.seek_end(end, "object reference")?;
            }
            return Ok(if tag == 0 { Any::Null } else { Any::Ref });
        }
        let class = if tag == NEW_CLASS {
            let name = self.cstring()?;
            let key = match start {
                Some(s) => (s + self.origin + MAP_OFFSET) as u32,
                None => self.classes.len() as u32 + 1,
            };
            self.classes.insert(key, name.clone());
            name
        } else {
            let r = tag & !CLASS_MASK;
            match self.classes.get(&r) {
                Some(n) => n.clone(),
                None => {
                    return Err(Error::Corrupt(format!(
                        "a reference to an unknown class ({r})"
                    )));
                }
            }
        };
        Ok(Any::Obj { class, end })
    }

    /// A TArray: a 32-bit count, then the values, as f64.
    pub(crate) fn tarray(&mut self, kind: Storage) -> Result<Vec<f64>> {
        let n = self.i32()?;
        let n = usize::try_from(n).map_err(|_| Error::Corrupt(format!("array length {n}")))?;
        self.values(kind, n)
    }

    /// `n` values of one storage type, as f64.
    pub(crate) fn values(&mut self, kind: Storage, n: usize) -> Result<Vec<f64>> {
        let size = kind.size();
        let bytes = n
            .checked_mul(size)
            .filter(|&b| b <= self.left())
            .ok_or_else(|| Error::Truncated(format!("an array of {n} values")))?;
        let s = self.take(bytes)?;
        let out = match kind {
            Storage::C => s.iter().map(|&b| f64::from(b as i8)).collect(),
            Storage::S => s
                .chunks_exact(2)
                .map(|c| f64::from(i16::from_be_bytes([c[0], c[1]])))
                .collect(),
            Storage::I => s
                .chunks_exact(4)
                .map(|c| f64::from(i32::from_be_bytes([c[0], c[1], c[2], c[3]])))
                .collect(),
            Storage::L => s
                .chunks_exact(8)
                .map(|c| {
                    let mut a = [0u8; 8];
                    a.copy_from_slice(c);
                    i64::from_be_bytes(a) as f64
                })
                .collect(),
            Storage::F => s
                .chunks_exact(4)
                .map(|c| f64::from(f32::from_be_bytes([c[0], c[1], c[2], c[3]])))
                .collect(),
            Storage::D => s
                .chunks_exact(8)
                .map(|c| {
                    let mut a = [0u8; 8];
                    a.copy_from_slice(c);
                    f64::from_be_bytes(a)
                })
                .collect(),
        };
        Ok(out)
    }

    /// A `Double_t*` array member with its size elsewhere: a speed-bump byte, then values.
    pub(crate) fn fast_array(&mut self, n: usize) -> Result<Vec<f64>> {
        if self.u8()? == 0 {
            return Ok(Vec::new());
        }
        self.values(Storage::D, n)
    }
}

/// How a histogram stores its cells (the letter of TH1F, TH2D, ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// 8-bit integers (TH1C).
    C,
    /// 16-bit integers (TH1S).
    S,
    /// 32-bit integers (TH1I).
    I,
    /// 64-bit integers (TH1L, ROOT 6.32 on).
    L,
    /// 32-bit floats (TH1F).
    F,
    /// 64-bit floats (TH1D, profiles).
    D,
}

impl Storage {
    pub fn size(self) -> usize {
        match self {
            Storage::C => 1,
            Storage::S => 2,
            Storage::I | Storage::F => 4,
            Storage::L | Storage::D => 8,
        }
    }

    pub fn from_letter(c: char) -> Option<Storage> {
        Some(match c {
            'C' => Storage::C,
            'S' => Storage::S,
            'I' => Storage::I,
            'L' => Storage::L,
            'F' => Storage::F,
            'D' => Storage::D,
            _ => return None,
        })
    }

    /// The storage as a data type name (`float32`), for the info panel.
    pub fn describe(self) -> &'static str {
        match self {
            Storage::C => "int8",
            Storage::S => "int16",
            Storage::I => "int32",
            Storage::L => "int64",
            Storage::F => "float32",
            Storage::D => "float64",
        }
    }
}
