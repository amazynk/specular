//! Marker segments: the frame header (SOF), transform table (DTT), quantisation table (DQT),
//! Huffman tables (DHT), comments (COM), restart interval (DRT) and block headers (SOB).
//!
//! Numbers stored as a scale byte and an integer are the integer divided by ten `scale` times.
//! Each division is done in double precision and kept in single precision, step by step, so
//! the values match the reference implementation's bit for bit (the reconstruction is
//! sensitive to the last bit of the filter taps and bin widths).

use crate::huffman::HuffTable;
use crate::{Error, Result};

pub(crate) const SOI: u16 = 0xFFA0;
pub(crate) const EOI: u16 = 0xFFA1;
pub(crate) const SOF: u16 = 0xFFA2;
pub(crate) const SOB: u16 = 0xFFA3;
pub(crate) const DTT: u16 = 0xFFA4;
pub(crate) const DQT: u16 = 0xFFA5;
pub(crate) const DHT: u16 = 0xFFA6;
pub(crate) const DRT: u16 = 0xFFA7;
pub(crate) const COM: u16 = 0xFFA8;

/// Longest filter accepted (the standard's are 9 and 7 taps).
pub(crate) const MAX_TAPS: usize = 32;

/// Bytes with a read position; every read is bounds-checked.
pub(crate) struct Cursor<'a> {
    pub d: &'a [u8],
    pub pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(d: &'a [u8], pos: usize) -> Cursor<'a> {
        Cursor { d, pos }
    }
    fn short(&self, what: &str) -> Error {
        Error::Truncated(format!("the file ends inside the {what}"))
    }
    pub fn u8(&mut self, what: &str) -> Result<u8> {
        let v = *self.d.get(self.pos).ok_or_else(|| self.short(what))?;
        self.pos += 1;
        Ok(v)
    }
    pub fn u16(&mut self, what: &str) -> Result<u16> {
        let b = self
            .d
            .get(self.pos..self.pos + 2)
            .ok_or_else(|| self.short(what))?;
        self.pos += 2;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    pub fn u32(&mut self, what: &str) -> Result<u32> {
        let b = self
            .d
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.short(what))?;
        self.pos += 4;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn bytes(&mut self, n: usize, what: &str) -> Result<&'a [u8]> {
        let b = self
            .d
            .get(self.pos..self.pos.saturating_add(n))
            .ok_or_else(|| self.short(what))?;
        self.pos += n;
        Ok(b)
    }
    /// A marker (0xFF and a code byte).
    pub fn marker(&mut self) -> Result<u16> {
        let m = self.u16("marker")?;
        if m >> 8 != 0xFF {
            return Err(Error::Malformed(format!(
                "expected a marker, found {m:04X} at byte {}",
                self.pos - 2
            )));
        }
        Ok(m)
    }
}

/// `v` divided by ten `scale` times, as the reference does it.
pub(crate) fn scaled(v: f32, scale: u8) -> f32 {
    let mut f = v;
    for _ in 0..scale {
        f = (f64::from(f) / 10.0) as f32;
    }
    f
}

/// The frame header.
#[derive(Clone, Debug)]
pub(crate) struct Frame {
    pub black: u8,
    pub white: u8,
    pub height: u16,
    pub width: u16,
    pub m_shift: f32,
    pub r_scale: f32,
    pub encoder: u8,
    pub software: u16,
}

pub(crate) fn frame(c: &mut Cursor) -> Result<Frame> {
    let _len = c.u16("frame header")?;
    let black = c.u8("frame header")?;
    let white = c.u8("frame header")?;
    let height = c.u16("frame header")?;
    let width = c.u16("frame header")?;
    let s = c.u8("frame header")?;
    let m_shift = scaled(f32::from(c.u16("frame header")?), s);
    let s = c.u8("frame header")?;
    let r_scale = scaled(f32::from(c.u16("frame header")?), s);
    let encoder = c.u8("frame header")?;
    let software = c.u16("frame header")?;
    Ok(Frame {
        black,
        white,
        height,
        width,
        m_shift,
        r_scale,
        encoder,
        software,
    })
}

/// The synthesis filters as the reconstruction uses them: `hi` has as many taps as the first
/// filter in the table, `lo` as the second.
#[derive(Clone, Debug)]
pub(crate) struct Filters {
    pub hi: Vec<f32>,
    pub lo: Vec<f32>,
}

/// One stored coefficient: sign byte, scale byte, 32-bit magnitude.
fn coefficient(c: &mut Cursor) -> Result<f32> {
    let sign = c.u8("transform table")?;
    let scale = c.u8("transform table")?;
    let v = scaled(c.u32("transform table")? as f32, scale);
    Ok(if sign != 0 { -v } else { v })
}

/// Expands half of a symmetric filter (centre tap first) into the whole filter with the
/// alternating signs that turn an analysis filter into the other band's synthesis filter.
/// `odd_shift`: for even lengths, which of the two halves gets the extra sign flip.
fn expand(c: &mut Cursor, size: usize, even_flip_first: bool) -> Result<Vec<f32>> {
    let mut out = vec![0f32; size];
    let half = size.div_ceil(2);
    let a = half - 1;
    for k in 0..half {
        let v = coefficient(c)?;
        let alt = if k % 2 == 0 { v } else { -v };
        if size % 2 == 1 {
            out[a + k] = alt;
            out[a - k] = alt;
        } else if even_flip_first {
            // The first filter (even length): mirrored half negated.
            out[a + k + 1] = alt;
            out[a - k] = -alt;
        } else {
            // The second filter (even length): the sign alternation starts one later.
            out[a + k + 1] = -alt;
            out[a - k] = -alt;
        }
    }
    Ok(out)
}

pub(crate) fn transform_table(c: &mut Cursor) -> Result<Filters> {
    let _len = c.u16("transform table")?;
    let hisz = c.u8("transform table")? as usize;
    let losz = c.u8("transform table")? as usize;
    if hisz == 0 || losz == 0 {
        return Err(Error::Malformed("a wavelet filter has no taps".into()));
    }
    if hisz > MAX_TAPS || losz > MAX_TAPS {
        return Err(Error::Unsupported(format!(
            "WSQ files with wavelet filters longer than {MAX_TAPS} taps are not supported"
        )));
    }
    let hi = expand(c, hisz, true)?;
    let lo = expand(c, losz, false)?;
    Ok(Filters { hi, lo })
}

/// The quantisation table: bin centre and per-subband bin and zero-bin widths.
#[derive(Clone, Debug)]
pub(crate) struct Quant {
    pub bin_center: f32,
    pub q_bin: [f32; 64],
    pub z_bin: [f32; 64],
}

pub(crate) fn quant_table(c: &mut Cursor) -> Result<Quant> {
    let _len = c.u16("quantisation table")?;
    let s = c.u8("quantisation table")?;
    let bin_center = scaled(f32::from(c.u16("quantisation table")?), s);
    let mut q_bin = [0f32; 64];
    let mut z_bin = [0f32; 64];
    for i in 0..64 {
        let s = c.u8("quantisation table")?;
        q_bin[i] = scaled(f32::from(c.u16("quantisation table")?), s);
        let s = c.u8("quantisation table")?;
        z_bin[i] = scaled(f32::from(c.u16("quantisation table")?), s);
    }
    Ok(Quant {
        bin_center,
        q_bin,
        z_bin,
    })
}

/// A DHT segment: one or more tables, stored into `tables` by id.
pub(crate) fn huffman_tables(c: &mut Cursor, tables: &mut [Option<HuffTable>; 8]) -> Result<()> {
    let len = c.u16("Huffman table")? as usize;
    if len < 2 {
        return Err(Error::Malformed("Huffman table segment too short".into()));
    }
    let end = c.pos - 2 + len;
    if end > c.d.len() {
        return Err(Error::Truncated(
            "the file ends inside a Huffman table".into(),
        ));
    }
    loop {
        let id = c.u8("Huffman table")? as usize;
        if id >= 8 {
            return Err(Error::Malformed(format!("Huffman table id {id} (0 to 7)")));
        }
        let counts: [u8; 16] = c
            .bytes(16, "Huffman table")?
            .try_into()
            .map_err(|_| Error::Malformed("Huffman table".into()))?;
        let n: usize = counts.iter().map(|&x| x as usize).sum();
        if n > 256 {
            return Err(Error::Malformed(format!(
                "Huffman table {id} has {n} codes (at most 256)"
            )));
        }
        let values = c.bytes(n, "Huffman table")?;
        tables[id] = Some(HuffTable::new(&counts, values)?);
        if c.pos >= end {
            break;
        }
    }
    c.pos = end;
    Ok(())
}

/// A segment with a length field that is only skipped (or kept: comments).
pub(crate) fn segment<'a>(c: &mut Cursor<'a>, what: &str) -> Result<&'a [u8]> {
    let len = c.u16(what)? as usize;
    if len < 2 {
        return Err(Error::Malformed(format!("{what} segment too short")));
    }
    c.bytes(len - 2, what)
}

/// Everything up to the first block: the tables, the frame header and the comments.
pub(crate) struct Header<'a> {
    pub frame: Frame,
    pub filters: Option<Filters>,
    pub quant: Option<Quant>,
    pub huffman: [Option<HuffTable>; 8],
    pub comments: Vec<&'a [u8]>,
    /// Where the first SOB marker starts (or where the data ends when there is none).
    pub first_block: usize,
}

/// Reads a table or comment segment that may appear between blocks. `marker` was just read.
pub(crate) fn table<'a>(
    c: &mut Cursor<'a>,
    marker: u16,
    filters: &mut Option<Filters>,
    quant: &mut Option<Quant>,
    huffman: &mut [Option<HuffTable>; 8],
    comments: &mut Vec<&'a [u8]>,
) -> Result<()> {
    match marker {
        DTT => *filters = Some(transform_table(c)?),
        DQT => *quant = Some(quant_table(c)?),
        DHT => huffman_tables(c, huffman)?,
        COM => comments.push(segment(c, "comment")?),
        DRT => {
            segment(c, "restart interval")?;
        }
        m if (0xFFA9..=0xFFAF).contains(&m) => {
            // Reserved for future use: skipped by length.
            segment(c, "reserved segment")?;
        }
        m => {
            return Err(Error::Malformed(format!(
                "unexpected marker {m:04X} at byte {}",
                c.pos - 2
            )));
        }
    }
    Ok(())
}

pub(crate) fn header(data: &[u8]) -> Result<Header<'_>> {
    let mut c = Cursor::new(data, 0);
    if c.marker().map_err(|_| Error::NotRecognised)? != SOI {
        return Err(Error::NotRecognised);
    }
    let mut filters = None;
    let mut quant = None;
    let mut huffman: [Option<HuffTable>; 8] = Default::default();
    let mut comments = Vec::new();
    let mut frame_hdr = None;
    loop {
        let at = c.pos;
        let m = match c.marker() {
            Ok(m) => m,
            Err(e) => {
                // Tables and a frame header but no coded data: an empty (grey) image.
                if let (Error::Truncated(_), Some(frame)) = (&e, frame_hdr.take()) {
                    return Ok(Header {
                        frame,
                        filters,
                        quant,
                        huffman,
                        comments,
                        first_block: data.len(),
                    });
                }
                return Err(e);
            }
        };
        match m {
            SOF => {
                if frame_hdr.is_some() {
                    return Err(Error::Malformed("two frame headers".into()));
                }
                frame_hdr = Some(frame(&mut c)?);
            }
            SOB | EOI => {
                let Some(frame) = frame_hdr else {
                    return Err(Error::Malformed("no frame header before the data".into()));
                };
                return Ok(Header {
                    frame,
                    filters,
                    quant,
                    huffman,
                    comments,
                    first_block: at,
                });
            }
            SOI => return Err(Error::Malformed("a second start-of-image marker".into())),
            m => table(
                &mut c,
                m,
                &mut filters,
                &mut quant,
                &mut huffman,
                &mut comments,
            )?,
        }
    }
}
