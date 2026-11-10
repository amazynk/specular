//! LAZ: LAS whose point records are LASzip-compressed, in chunks that decode independently
//! (so in parallel), found through a chunk table after the points.
//!
//! The LASzip VLR (user "laszip encoded", record 22204) lists the items a record is made of.
//! Compressor 2 codes them pointwise ([`v2`]), compressor 3 in layers ([`v3`], formats 6–10).

pub(crate) mod arith;
mod tables;
mod v2;
mod v3;

use crate::las::Rec;
use crate::{Error, Result, malformed};

/// Bytes a decoder may read past its stream's end before the stream counts as truncated.
const OVERRUN_SLACK: usize = 4;

/// One item of a compressed record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Byte(u16),
    Point10,
    Gps11,
    Rgb12,
    Wave13,
    Point14,
    Rgb14,
    RgbNir14,
    Byte14(u16),
    Wave14,
}

impl Item {
    fn size(self) -> usize {
        match self {
            Item::Byte(n) | Item::Byte14(n) => usize::from(n),
            Item::Point10 => 20,
            Item::Gps11 => 8,
            Item::Rgb12 | Item::Rgb14 => 6,
            Item::Wave13 | Item::Wave14 => 29,
            Item::Point14 => 30,
            Item::RgbNir14 => 8,
        }
    }
}

/// What the LASzip VLR says.
#[derive(Clone, Debug)]
pub(crate) struct Laz {
    /// Points per chunk; `u32::MAX` for chunks of varying size (counts in the chunk table).
    pub(crate) chunk_size: u32,
    pub(crate) version: (u8, u8, u16),
    pub(crate) layout: Layout,
}

/// The items of a record with their byte offsets.
#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub(crate) items: Vec<(Item, usize)>,
    pub(crate) record_len: usize,
    layered: bool,
}

impl Laz {
    /// Parses the VLR's payload for records of `record_len` bytes.
    pub(crate) fn parse(data: &[u8], record_len: usize) -> Result<Laz> {
        let u16_at = |o: usize| data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let u32_at = |o: usize| {
            data.get(o..o + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let (Some(compressor), Some(chunk_size), Some(n)) = (u16_at(0), u32_at(12), u16_at(32))
        else {
            return malformed("LASzip VLR truncated");
        };
        let version = (data[4], data[5], u16_at(6).unwrap_or(0));
        let mut items = Vec::with_capacity(usize::from(n));
        let mut off = 0usize;
        for i in 0..usize::from(n) {
            let at = 34 + i * 6;
            let (Some(kind), Some(size), Some(ver)) = (u16_at(at), u16_at(at + 2), u16_at(at + 4))
            else {
                return malformed("LASzip item list truncated");
            };
            let item = match kind {
                0 => Item::Byte(size),
                6 => Item::Point10,
                7 => Item::Gps11,
                8 => Item::Rgb12,
                9 => Item::Wave13,
                10 => Item::Point14,
                11 => Item::Rgb14,
                12 => Item::RgbNir14,
                13 => Item::Wave14,
                14 => Item::Byte14(size),
                k => return Err(Error::Unsupported(format!("LASzip item type {k}"))),
            };
            if item.size() != usize::from(size) {
                return malformed(format!("LASzip item {kind} of {size} bytes"));
            }
            let layered = matches!(
                item,
                Item::Point14 | Item::Rgb14 | Item::RgbNir14 | Item::Byte14(_) | Item::Wave14
            );
            match (layered, ver) {
                (true, 3 | 4) => {}
                (false, 2) => {}
                (_, v) => {
                    return Err(Error::Unsupported(format!(
                        "LASzip item type {kind} version {v}"
                    )));
                }
            }
            if item == Item::Wave13 {
                return Err(Error::Unsupported("LAZ with waveform packets (v2)".into()));
            }
            items.push((item, off));
            off += item.size();
        }
        if off != record_len {
            return malformed(format!(
                "LASzip items make {off}-byte records, the header says {record_len}"
            ));
        }
        let layered = items.iter().any(|(i, _)| *i == Item::Point14);
        let first = items.first().map(|(i, _)| *i);
        match (compressor, layered, first) {
            (2, false, Some(Item::Point10)) | (3, true, Some(Item::Point14)) => {}
            (0 | 1, ..) => {
                return Err(Error::Unsupported(format!(
                    "LASzip compressor {compressor} (not chunked)"
                )));
            }
            _ => return malformed(format!("LASzip compressor {compressor} with these items")),
        }
        Ok(Laz {
            chunk_size,
            version,
            layout: Layout {
                items,
                record_len,
                layered,
            },
        })
    }

    /// "LASzip 3.4 r3, layered".
    pub(crate) fn describe(&self) -> String {
        let (a, b, r) = self.version;
        let kind = if self.layout.layered {
            "layered"
        } else {
            "pointwise"
        };
        format!("LASzip {a}.{b} r{r}, {kind}")
    }
}

/// A chunk: where its bytes are and how many points it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Chunk {
    pub(crate) offset: usize,
    pub(crate) len: usize,
    pub(crate) count: u64,
}

/// Reads the chunk table of a file whose points start at `data_offset` and number `total`.
pub(crate) fn chunks(
    bytes: &[u8],
    data_offset: usize,
    total: u64,
    laz: &Laz,
) -> Result<Vec<Chunk>> {
    let i64_at = |o: usize| {
        bytes
            .get(o..o.checked_add(8)?)
            .map(|b| i64::from_le_bytes(b.try_into().expect("8 bytes")))
    };
    let Some(mut table) = i64_at(data_offset) else {
        return malformed("LAZ point data truncated");
    };
    if table == -1 {
        // Written by a streaming writer: the table's offset is in the file's last 8 bytes.
        table = bytes.len().checked_sub(8).and_then(i64_at).unwrap_or(-1);
    }
    let start = data_offset + 8;
    let table = usize::try_from(table)
        .ok()
        .filter(|&t| t >= start && t + 8 <= bytes.len());
    let Some(table) = table else {
        return Err(Error::Unsupported("LAZ without a chunk table".into()));
    };
    let n = u32::from_le_bytes(bytes[table + 4..table + 8].try_into().expect("4 bytes"));
    // Every chunk takes at least a byte of the table's stream and of the points.
    if u64::from(n) > (bytes.len() as u64) || (n == 0 && total > 0) {
        return malformed(format!("LAZ chunk table with {n} chunks"));
    }
    let variable = laz.chunk_size == u32::MAX;
    if !variable && laz.chunk_size == 0 {
        return malformed("LAZ chunk size 0");
    }
    let mut d = arith::Decoder::new(&bytes[table + 8..]);
    let mut ic = arith::IntCoder::new(32, 2);
    let (mut count, mut size) = (0i32, 0i32);
    let mut out = Vec::with_capacity(n as usize);
    let mut offset = start;
    let mut left = total;
    for _ in 0..n {
        let c = if variable {
            count = ic.decode(&mut d, count, 0);
            u64::from(count as u32)
        } else {
            u64::from(laz.chunk_size).min(left)
        };
        size = ic.decode(&mut d, size, 1);
        let len = size as u32 as usize;
        if offset.checked_add(len).is_none_or(|e| e > table) {
            return malformed("LAZ chunk past the chunk table");
        }
        out.push(Chunk {
            offset,
            len,
            count: c,
        });
        offset += len;
        left = left.saturating_sub(c);
    }
    if d.overrun > OVERRUN_SLACK {
        return malformed("LAZ chunk table truncated");
    }
    Ok(out)
}

/// Decodes a chunk of `count` points into `out`.
pub(crate) fn decode(
    layout: &Layout,
    chunk: &[u8],
    count: usize,
    out: &mut Vec<Rec>,
) -> Result<()> {
    if count == 0 {
        return Ok(());
    }
    if layout.layered {
        return v3::decode(layout, chunk, count, out);
    }
    let len = layout.record_len;
    let Some(raw) = chunk.get(..len) else {
        return malformed("LAZ chunk shorter than its first point");
    };
    enum State {
        P10(Box<v2::Point10>),
        Gps(Box<v2::GpsTime>),
        Rgb(Box<v2::Rgb>),
        Bytes(v2::Bytes),
    }
    let mut first = Rec::default();
    let mut states = Vec::with_capacity(layout.items.len());
    for &(item, off) in &layout.items {
        let b = &raw[off..off + item.size()];
        states.push(match item {
            Item::Point10 => {
                v2::Point10::put_first(b, &mut first);
                State::P10(Box::new(v2::Point10::new(b)))
            }
            Item::Gps11 => State::Gps(Box::new(v2::GpsTime::new(b))),
            Item::Rgb12 => {
                let c = v2::Rgb::new(b);
                first.rgb = c.last;
                State::Rgb(Box::new(c))
            }
            Item::Byte(_) => State::Bytes(v2::Bytes::new(b)),
            _ => return malformed("layered item in a pointwise chunk"),
        });
    }
    out.push(first);
    let mut d = arith::Decoder::new(&chunk[len..]);
    let mut rec = first;
    for _ in 1..count {
        for s in &mut states {
            match s {
                State::P10(p) => {
                    p.read(&mut d);
                    p.put(&mut rec);
                }
                State::Gps(g) => g.read(&mut d),
                State::Rgb(c) => {
                    c.read(&mut d);
                    rec.rgb = c.last;
                }
                State::Bytes(b) => b.read(&mut d),
            }
        }
        out.push(rec);
    }
    if d.overrun > OVERRUN_SLACK {
        return malformed("LAZ chunk ended early");
    }
    Ok(())
}
