//! Esri File Geodatabase (a `.gdb` directory, ArcGIS 9.2 and later): the system catalog
//! (`a00000001.gdbtable`) names every table; each table is a `.gdbtable` of rows (field
//! descriptions in its header) with a `.gdbtablx` of row offsets, sparse when whole blocks
//! of object ids are missing. `GDB_Items` adds each feature class's spatial reference.
//!
//! Every user table becomes a layer (non-spatial ones without geometry); geometries are
//! Esri shape buffers with varint coordinates scaled by the geometry field's origin and
//! scale. Arcs, Béziers and elliptic arcs are stroked into lines; multipatches become
//! polygons; Z and M values are skipped. Compressed tables (CDF), and tables with 64-bit
//! object ids (format version 4) whose ids have gaps, are refused by name.

use std::path::{Path, PathBuf};

use crate::crs::{self, Crs};
use crate::model::{
    Dataset, Feature, Geometry, Layer, MAX_FEATURES, MAX_VERTICES, Pt, Value, arc_to,
};

/// Rows read per table at most.
const MAX_ROW: usize = 1 << 30;

// ----------------------------------------------------------------------------- reading

struct Cur<'a> {
    b: &'a [u8],
    i: usize,
}

const TRUNC: &str = "a File Geodatabase table is truncated";

impl<'a> Cur<'a> {
    fn new(b: &'a [u8], i: usize) -> Self {
        Cur { b, i }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self
            .b
            .get(self.i..self.i.checked_add(n).ok_or(TRUNC)?)
            .ok_or(TRUNC)?;
        self.i += n;
        Ok(s)
    }
    fn arr<const N: usize>(&mut self) -> Result<[u8; N], String> {
        self.take(N)?.try_into().map_err(|_| TRUNC.to_string())
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.arr()?))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.arr()?))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.arr()?))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.arr()?))
    }
    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(self.arr()?))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.arr()?))
    }
    fn f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_le_bytes(self.arr()?))
    }
    fn varuint(&mut self) -> Result<u64, String> {
        let mut v = 0u64;
        let mut shift = 0u32;
        loop {
            let b = self.u8()?;
            if shift < 64 {
                v |= u64::from(b & 0x7F) << shift;
            }
            if b & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
            if shift > 70 {
                return Err("a File Geodatabase number is malformed".into());
            }
        }
    }
    /// Esri's signed varint: the first byte holds a sign bit (0x40) and 6 value bits.
    fn varint(&mut self) -> Result<i64, String> {
        let b = self.u8()?;
        let neg = b & 0x40 != 0;
        let mut v = u64::from(b & 0x3F);
        let mut more = b & 0x80 != 0;
        let mut shift = 6u32;
        while more {
            let b = self.u8()?;
            if shift < 64 {
                v |= u64::from(b & 0x7F) << shift;
            }
            more = b & 0x80 != 0;
            shift += 7;
            if shift > 70 {
                return Err("a File Geodatabase number is malformed".into());
            }
        }
        let v = v as i64;
        Ok(if neg { v.wrapping_neg() } else { v })
    }
    /// Bytes not read yet.
    fn left(&self) -> usize {
        self.b.len().saturating_sub(self.i)
    }
    fn len(&mut self) -> Result<usize, String> {
        usize::try_from(self.varuint()?).map_err(|_| TRUNC.to_string())
    }
    fn utf16(&mut self, chars: usize) -> Result<String, String> {
        let b = self.take(chars.checked_mul(2).ok_or(TRUNC)?)?;
        let u: Vec<u16> = b
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&u))
    }
}

// ------------------------------------------------------------------------------ fields

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FType {
    I16,
    I32,
    F32,
    F64,
    Str,
    DateTime,
    ObjectId,
    Geometry,
    Binary,
    /// Raster, with how its value is stored (0 path, 1 managed id, 2 inline).
    Raster(u8),
    Guid,
    GlobalId,
    Xml,
    I64,
    Date,
    Time,
    DateTimeOffset,
}

#[derive(Clone, Debug, Default)]
struct GeomDef {
    wkt: String,
    xo: f64,
    yo: f64,
    xys: f64,
}

#[derive(Clone, Debug)]
struct FieldDef {
    name: String,
    ty: FType,
    nullable: bool,
    /// A date-time kept to the millisecond (ArcGIS Pro 3.2 "high precision", from GDB_Items).
    high_precision: bool,
}

/// A table's header and field descriptions.
struct Table {
    data: Vec<u8>,
    tablx: Vec<u8>,
    fields: Vec<FieldDef>,
    geom: Option<GeomDef>,
    geom_type: u8,
    utf8: bool,
    /// Total object ids (deleted included).
    total: u64,
    offset_size: usize,
    blocks: usize,
    /// Which 1024-row blocks are present, when sparse.
    bitmap: Option<Vec<bool>>,
}

fn field_type(t: u8) -> Option<FType> {
    Some(match t {
        0 => FType::I16,
        1 => FType::I32,
        2 => FType::F32,
        3 => FType::F64,
        4 => FType::Str,
        5 => FType::DateTime,
        6 => FType::ObjectId,
        7 => FType::Geometry,
        8 => FType::Binary,
        9 => FType::Raster(0),
        10 => FType::Guid,
        11 => FType::GlobalId,
        12 => FType::Xml,
        13 => FType::I64,
        14 => FType::Date,
        15 => FType::Time,
        16 => FType::DateTimeOffset,
        _ => return None,
    })
}

/// The origin/scale block of a geometry or raster field: returns (x origin, y origin,
/// xy scale) and skips the rest.
fn origins(c: &mut Cur<'_>, flags: u8) -> Result<(f64, f64, f64), String> {
    let (xo, yo, xys) = (c.f64()?, c.f64()?, c.f64()?);
    if flags & 2 != 0 {
        c.take(16)?; // M origin, scale
    }
    if flags & 4 != 0 {
        c.take(16)?; // Z origin, scale
    }
    c.take(8)?; // xy tolerance
    if flags & 2 != 0 {
        c.take(8)?;
    }
    if flags & 4 != 0 {
        c.take(8)?;
    }
    Ok((xo, yo, xys))
}

fn read_fields(c: &mut Cur<'_>) -> Result<(Vec<FieldDef>, Option<GeomDef>, u8, bool), String> {
    let _size = c.u32()?;
    let _version = c.u32()?;
    let flags = c.arr::<4>()?;
    let geom_type = flags[0];
    // Bit 0 of the second byte: strings are UTF-8 (else UTF-16).
    let utf8 = flags[1] & 1 != 0;
    let n = c.u16()?;
    let mut fields = Vec::with_capacity(usize::from(n));
    let mut geom = None;
    for _ in 0..n {
        let nl = usize::from(c.u8()?);
        let name = c.utf16(nl)?;
        let al = usize::from(c.u8()?);
        c.utf16(al)?;
        let tcode = c.u8()?;
        let mut ty = field_type(tcode)
            .ok_or_else(|| format!("unknown File Geodatabase field type {tcode}"))?;
        let nullable;
        match ty {
            FType::ObjectId => {
                c.u8()?;
                nullable = c.u8()? & 1 != 0;
            }
            FType::Geometry => {
                c.u8()?;
                nullable = c.u8()? & 1 != 0;
                let wl = usize::from(c.u16()?);
                let wkt = c.utf16(wl / 2)?;
                let gf = c.u8()?;
                let (xo, yo, xys) = origins(c, gf)?;
                c.take(32)?; // extent
                // Optional Z and M extents, then the spatial index grid sizes.
                let mut found = false;
                for _ in 0..3 {
                    let peek = c.b.get(c.i..c.i + 5).ok_or(TRUNC)?;
                    let count = u32::from_le_bytes([peek[1], peek[2], peek[3], peek[4]]);
                    if peek[0] == 0 && (1..=3).contains(&count) {
                        c.take(5)?;
                        c.take(8 * count as usize)?;
                        found = true;
                        break;
                    }
                    c.take(16)?;
                }
                if !found {
                    return Err("a File Geodatabase geometry field is damaged".into());
                }
                geom = Some(GeomDef {
                    wkt,
                    xo,
                    yo,
                    xys: if xys > 0.0 && xys.is_finite() {
                        xys
                    } else {
                        1.0
                    },
                });
            }
            FType::Str => {
                c.u32()?;
                nullable = c.u8()? & 1 != 0;
                let dl = c.len()?;
                c.take(dl)?;
            }
            FType::Binary | FType::Xml | FType::Guid | FType::GlobalId => {
                c.u8()?;
                nullable = c.u8()? & 1 != 0;
            }
            FType::Raster(_) => {
                c.u8()?;
                nullable = c.u8()? & 1 != 0;
                let cl = usize::from(c.u8()?);
                c.utf16(cl)?;
                let wl = usize::from(c.u16()?);
                c.take(wl)?;
                let magic = c.u8()?;
                if magic > 0 {
                    origins(c, magic)?;
                }
                ty = FType::Raster(c.u8()?);
            }
            _ => {
                c.u8()?;
                nullable = c.u8()? & 1 != 0;
                let dl = usize::from(c.u8()?);
                c.take(dl)?;
            }
        }
        fields.push(FieldDef {
            name,
            ty,
            nullable,
            high_precision: false,
        });
    }
    Ok((fields, geom, geom_type, utf8))
}

fn table_paths(dir: &Path, id: u64) -> (PathBuf, PathBuf) {
    (
        dir.join(format!("a{id:08x}.gdbtable")),
        dir.join(format!("a{id:08x}.gdbtablx")),
    )
}

impl Table {
    fn open(dir: &Path, id: u64, name: &str) -> Result<Table, String> {
        let (tp, xp) = table_paths(dir, id);
        let cdf = dir.join(format!("a{id:08x}.gdbtable.cdf"));
        let data = std::fs::read(&tp).map_err(|e| format!("{name}: {e}"))?;
        if cdf.exists() || data.starts_with(b"<?xml") {
            return Err(format!(
                "{name}: compressed File Geodatabase tables (CDF) are not supported"
            ));
        }
        let mut c = Cur::new(&data, 0);
        let version = c.u32()?;
        if version != 3 && version != 4 {
            return Err(format!(
                "{name}: compressed or unknown File Geodatabase table (version {version}) is not supported"
            ));
        }
        c.i = 32;
        let fo = usize::try_from(c.i64()?).map_err(|_| TRUNC.to_string())?;
        let mut fc = Cur::new(&data, fo);
        let (fields, geom, geom_type, utf8) = read_fields(&mut fc)?;
        let tablx = std::fs::read(&xp).map_err(|e| format!("{name}: no row index: {e}"))?;
        let mut x = Cur::new(&tablx, 0);
        let _magic = x.u32()?;
        // Version 4 (64-bit object ids): a 64-bit block count and no row total.
        let (blocks, total) = if version == 4 {
            let b = usize::try_from(x.i64()?).map_err(|_| TRUNC.to_string())?;
            (b, (b as u64).saturating_mul(1024))
        } else {
            (x.u32()? as usize, u64::from(x.u32()?))
        };
        let offset_size = x.u32()? as usize;
        if !(4..=8).contains(&offset_size) {
            return Err(format!("{name}: the row index is damaged"));
        }
        let end = blocks
            .checked_mul(1024 * offset_size)
            .and_then(|n| n.checked_add(16))
            .ok_or("the row index is damaged")?;
        if end > tablx.len() {
            return Err(format!("{name}: the row index is truncated"));
        }
        // A version 4 index ends with the row total and, when ids have gaps, a block map.
        let mut total = total;
        if version == 4 {
            let tail = &tablx[end..];
            let dense = tail.len() <= 8 || tail[8..].iter().all(|&b| b == 0);
            if !dense {
                return Err(format!(
                    "{name}: File Geodatabase tables with 64-bit object ids and gaps between them are not supported"
                ));
            }
            if let Some(t) = tail.get(..8) {
                let n = u64::from_le_bytes(t.try_into().unwrap_or_default());
                total = total.min(n);
            }
        }
        let mut bitmap = None;
        if version == 3 && blocks > 0 && (blocks as u64) * 1024 < total {
            let mut t = Cur::new(&tablx, end);
            let words = t.u32()? as usize;
            let bits = t.u32()? as usize;
            let _present = t.u32()?;
            let _useful = t.u32()?;
            if words > 0 {
                let w = t.take(words.checked_mul(4).ok_or(TRUNC)?)?;
                bitmap = Some(
                    (0..bits.min(words * 32))
                        .map(|k| {
                            let word = u32::from_le_bytes([
                                w[k / 32 * 4],
                                w[k / 32 * 4 + 1],
                                w[k / 32 * 4 + 2],
                                w[k / 32 * 4 + 3],
                            ]);
                            word >> (k % 32) & 1 != 0
                        })
                        .collect(),
                );
            }
        }
        Ok(Table {
            data,
            tablx,
            fields,
            geom,
            geom_type,
            utf8,
            total,
            offset_size,
            blocks,
            bitmap,
        })
    }

    /// (object id, row offset) of every live row, in object id order.
    fn rows(&self) -> Vec<(u64, usize)> {
        let mut out = Vec::new();
        let total = self.total.min(MAX_ROW as u64);
        // Each 1024-id block of object ids, and the block of offsets holding it.
        let blocks: Vec<(u64, usize)> = match &self.bitmap {
            Some(b) => b
                .iter()
                .enumerate()
                .filter(|(_, p)| **p)
                .enumerate()
                .map(|(slot, (blk, _))| (blk as u64, slot))
                .collect(),
            None => (0..self.blocks).map(|b| (b as u64, b)).collect(),
        };
        for (blk, slot) in blocks {
            if slot >= self.blocks {
                break;
            }
            for j in 0..1024usize {
                let oid = blk * 1024 + j as u64 + 1;
                if oid > total {
                    return out;
                }
                let at = 16 + (slot * 1024 + j) * self.offset_size;
                let Some(o) = self.tablx.get(at..at + self.offset_size) else {
                    return out;
                };
                let off = o.iter().rev().fold(0u64, |a, &x| (a << 8) | u64::from(x));
                if off != 0 {
                    out.push((oid, off as usize));
                }
            }
        }
        out
    }

    fn text(&self, b: &[u8]) -> String {
        if self.utf8 {
            String::from_utf8_lossy(b).into_owned()
        } else {
            let u: Vec<u16> = b
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u)
        }
    }

    /// The row at `off`: attribute values (geometry and object id fields left out) and
    /// the geometry blob.
    fn row(&self, off: usize) -> Result<(Vec<Value>, Option<&[u8]>), String> {
        let mut c = Cur::new(&self.data, off);
        let size = c.i32()?;
        if size < 0 {
            return Err("deleted".into());
        }
        let blob = c.take(size as usize)?;
        let mut c = Cur::new(blob, 0);
        let nullable = self.fields.iter().filter(|f| f.nullable).count();
        let flags = c.take(nullable.div_ceil(8))?;
        let mut nk = 0usize;
        let mut values = Vec::new();
        let mut geom = None;
        for f in &self.fields {
            if f.ty == FType::ObjectId {
                continue;
            }
            let null = if f.nullable {
                let bit = flags[nk / 8] >> (nk % 8) & 1 != 0;
                nk += 1;
                bit
            } else {
                false
            };
            if null {
                if f.ty != FType::Geometry {
                    values.push(Value::Null);
                }
                continue;
            }
            let v = match f.ty {
                FType::I16 => Value::Int(i64::from(c.i16()?)),
                FType::I32 => Value::Int(i64::from(c.i32()?)),
                FType::I64 => Value::Int(c.i64()?),
                FType::F32 => Value::Float(f64::from(c.f32()?)),
                FType::F64 => Value::Float(c.f64()?),
                FType::Str | FType::Xml => {
                    let n = c.len()?;
                    Value::Text(self.text(c.take(n)?))
                }
                FType::DateTime => Value::Date(datetime(c.f64()?, Some(f.high_precision))),
                FType::Date => Value::Date(datetime(c.f64()?, None)),
                FType::Time => Value::Text(time_of_day(c.f64()?)),
                FType::DateTimeOffset => {
                    let d = c.f64()?;
                    let tz = c.i16()?;
                    let mut s = datetime(d, Some(true));
                    let a = tz.unsigned_abs();
                    s.push_str(&format!(
                        "{}{:02}:{:02}",
                        if tz < 0 { '-' } else { '+' },
                        a / 60,
                        a % 60
                    ));
                    Value::Date(s)
                }
                FType::Guid | FType::GlobalId => Value::Text(guid(c.take(16)?)),
                FType::Binary => {
                    let n = c.len()?;
                    c.take(n)?;
                    Value::Blob(n)
                }
                FType::Raster(1) => Value::Int(i64::from(c.i32()?)),
                FType::Raster(0) => {
                    let n = c.len()?;
                    Value::Text(self.text(c.take(n)?))
                }
                FType::Raster(_) => {
                    let n = c.len()?;
                    c.take(n)?;
                    Value::Blob(n)
                }
                FType::Geometry => {
                    let n = c.len()?;
                    geom = Some(c.take(n)?);
                    continue;
                }
                FType::ObjectId => continue,
            };
            values.push(v);
        }
        Ok((values, geom))
    }

    fn attribute_names(&self) -> Vec<String> {
        self.fields
            .iter()
            .filter(|f| !matches!(f.ty, FType::ObjectId | FType::Geometry))
            .map(|f| f.name.clone())
            .collect()
    }
}

fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Days since 1899-12-30 as ISO 8601: the date alone (`None`), else with the time rounded
/// to the second or, with `Some(true)`, to the millisecond (GDAL reads classic date-time
/// fields to the second).
fn datetime(days: f64, time: Option<bool>) -> String {
    if !days.is_finite() || days.abs() > 1e7 {
        return String::new();
    }
    let unit = if time == Some(true) { 1.0 } else { 1000.0 };
    let ms = ((days * 86_400_000.0 / unit).round() * unit) as i64;
    let d = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, dd) = civil(d - 25_569);
    let mut s = format!("{y:04}-{m:02}-{dd:02}");
    if time.is_some() {
        let (h, mi, se, f) = (
            rem / 3_600_000,
            rem / 60_000 % 60,
            rem / 1000 % 60,
            rem % 1000,
        );
        s.push_str(&format!("T{h:02}:{mi:02}:{se:02}"));
        if f != 0 {
            s.push_str(&format!(".{f:03}"));
        }
    }
    s
}

fn time_of_day(frac: f64) -> String {
    if !frac.is_finite() {
        return String::new();
    }
    let ms = (frac.rem_euclid(1.0) * 86_400_000.0).round() as i64;
    let (h, mi, se, f) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    if f != 0 {
        format!("{h:02}:{mi:02}:{se:02}.{f:03}")
    } else {
        format!("{h:02}:{mi:02}:{se:02}")
    }
}

fn guid(b: &[u8]) -> String {
    let o = [3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15];
    let h: Vec<String> = o.iter().map(|&i| format!("{:02X}", b[i])).collect();
    format!(
        "{{{}-{}-{}-{}-{}}}",
        h[..4].concat(),
        h[4..6].concat(),
        h[6..8].concat(),
        h[8..10].concat(),
        h[10..].concat()
    )
}

// ---------------------------------------------------------------------------- geometry

/// Shape type codes with Z and M by the classic numbering.
fn classic_zm(t: u32) -> (bool, bool) {
    let z = matches!(t, 9 | 10 | 11 | 13 | 15 | 18 | 19 | 20 | 31 | 32);
    let m = matches!(t, 11 | 13 | 15 | 18 | 21 | 23 | 25 | 28 | 31);
    (z, m)
}

struct Shape<'a> {
    c: Cur<'a>,
    g: &'a GeomDef,
}

const EXT_Z: u64 = 0x8000_0000;
const EXT_M: u64 = 0x4000_0000;
const EXT_CURVES: u64 = 0x2000_0000;

impl Shape<'_> {
    fn xy_abs(&mut self) -> Result<Option<Pt>, String> {
        let x = self.c.varuint()?;
        let y = self.c.varuint()?;
        if x == 0 {
            return Ok(None); // an empty point
        }
        Ok(Some([
            (x - 1) as f64 / self.g.xys + self.g.xo,
            (y.saturating_sub(1)) as f64 / self.g.xys + self.g.yo,
        ]))
    }

    /// `n` delta-coded points.
    fn points(&mut self, n: usize) -> Result<Vec<Pt>, String> {
        if n > self.c.b.len() {
            return Err("a File Geodatabase geometry is damaged".into());
        }
        let mut out = Vec::with_capacity(n);
        let (mut dx, mut dy) = (0i64, 0i64);
        for _ in 0..n {
            dx = dx.wrapping_add(self.c.varint()?);
            dy = dy.wrapping_add(self.c.varint()?);
            out.push([
                dx as f64 / self.g.xys + self.g.xo,
                dy as f64 / self.g.xys + self.g.yo,
            ]);
        }
        Ok(out)
    }

    fn skip_values(&mut self, n: usize, m: bool) -> Result<(), String> {
        // An M array may be a single 0x42 byte: all M unset.
        if m && self.c.b.get(self.c.i) == Some(&0x42) {
            self.c.i += 1;
            return Ok(());
        }
        for _ in 0..n {
            self.c.varint()?;
        }
        Ok(())
    }

    fn bbox(&mut self) -> Result<(), String> {
        for _ in 0..4 {
            self.c.varuint()?;
        }
        Ok(())
    }
}

/// Straight or curved: the points from `a` to `b` of one curve segment, without `a`.
fn segment(seg: &Seg, a: Pt, b: Pt, out: &mut Vec<Pt>) {
    match *seg {
        Seg::Arc { p, ip, ccw, minor } => {
            if ip {
                arc_to(a, p, b, out);
                return;
            }
            // From the centre: pick the sweep the flags describe.
            let (cx, cy) = (p[0], p[1]);
            let r = ((a[0] - cx).powi(2) + (a[1] - cy).powi(2)).sqrt();
            let a0 = (a[1] - cy).atan2(a[0] - cx);
            let a1 = (b[1] - cy).atan2(b[0] - cx);
            let tau = std::f64::consts::TAU;
            let mut sweep = if ccw {
                (a1 - a0).rem_euclid(tau)
            } else {
                -(a0 - a1).rem_euclid(tau)
            };
            if a == b {
                sweep = if ccw { tau } else { -tau };
            }
            let _ = minor;
            let steps = ((sweep.abs() / 4f64.to_radians()).ceil() as usize).clamp(2, 3600);
            for k in 1..steps {
                let t = a0 + sweep * k as f64 / steps as f64;
                out.push([cx + r * t.cos(), cy + r * t.sin()]);
            }
            out.push(b);
        }
        Seg::Bezier { c1, c2 } => {
            for k in 1..=20 {
                let t = f64::from(k) / 20.0;
                let u = 1.0 - t;
                let f = |i: usize| {
                    u * u * u * a[i]
                        + 3.0 * u * u * t * c1[i]
                        + 3.0 * u * t * t * c2[i]
                        + t * t * t * b[i]
                };
                out.push([f(0), f(1)]);
            }
            if let Some(l) = out.last_mut() {
                *l = b;
            }
        }
        Seg::Ellipse {
            centre,
            rotation,
            major,
            ratio,
            ccw,
        } => {
            // Angles of the end points in the ellipse's own frame.
            let (s, c) = rotation.sin_cos();
            let local = |p: Pt| {
                let (dx, dy) = (p[0] - centre[0], p[1] - centre[1]);
                let (x, y) = (dx * c + dy * s, -dx * s + dy * c);
                (y / (major * ratio)).atan2(x / major)
            };
            let (t0, t1) = (local(a), local(b));
            let tau = std::f64::consts::TAU;
            let mut sweep = if ccw {
                (t1 - t0).rem_euclid(tau)
            } else {
                -(t0 - t1).rem_euclid(tau)
            };
            if a == b {
                sweep = if ccw { tau } else { -tau };
            }
            let steps = ((sweep.abs() / 4f64.to_radians()).ceil() as usize).clamp(2, 3600);
            for k in 1..steps {
                let t = t0 + sweep * k as f64 / steps as f64;
                let (x, y) = (major * t.cos(), major * ratio * t.sin());
                out.push([centre[0] + x * c - y * s, centre[1] + x * s + y * c]);
            }
            out.push(b);
        }
        Seg::Line => out.push(b),
    }
}

#[derive(Clone, Copy, Debug)]
enum Seg {
    Line,
    Arc {
        p: Pt,
        ip: bool,
        ccw: bool,
        minor: bool,
    },
    Bezier {
        c1: Pt,
        c2: Pt,
    },
    Ellipse {
        centre: Pt,
        rotation: f64,
        major: f64,
        ratio: f64,
        ccw: bool,
    },
}

fn read_curves(s: &mut Shape<'_>, n: usize) -> Result<Vec<(usize, Seg)>, String> {
    let mut out = Vec::new();
    for _ in 0..n {
        let start = s.c.len()?;
        let kind = s.c.varuint()?;
        let seg = match kind {
            1 => {
                let p = [s.c.f64()?, s.c.f64()?];
                let bits = s.c.i32()?;
                if bits & (0x1 | 0x20 | 0x40) != 0 {
                    Seg::Line
                } else {
                    Seg::Arc {
                        p,
                        ip: bits & 0x80 != 0,
                        ccw: bits & 0x8 != 0,
                        minor: bits & 0x10 != 0,
                    }
                }
            }
            4 => Seg::Bezier {
                c1: [s.c.f64()?, s.c.f64()?],
                c2: [s.c.f64()?, s.c.f64()?],
            },
            5 => {
                let centre = [s.c.f64()?, s.c.f64()?];
                let rotation = s.c.f64()?;
                let major = s.c.f64()?;
                let ratio = s.c.f64()?;
                let bits = s.c.i32()?;
                if bits & (0x1 | 0x40 | 0x80) != 0 || major <= 0.0 || !major.is_finite() {
                    Seg::Line
                } else {
                    Seg::Ellipse {
                        centre,
                        rotation,
                        major,
                        ratio: if ratio > 0.0 && ratio.is_finite() {
                            ratio
                        } else {
                            1.0
                        },
                        ccw: bits & 0x800 != 0,
                    }
                }
            }
            k => return Err(format!("unknown File Geodatabase curve segment {k}")),
        };
        out.push((start, seg));
    }
    out.sort_by_key(|c| c.0);
    Ok(out)
}

/// Splits points into parts (sizes as stored) and strokes the curves that start in them.
fn parts(pts: &[Pt], sizes: &[usize], curves: &[(usize, Seg)]) -> Vec<Vec<Pt>> {
    let mut out = Vec::with_capacity(sizes.len());
    let mut s = 0usize;
    for &n in sizes {
        let e = (s + n).min(pts.len());
        if s >= e {
            out.push(Vec::new());
            s = e;
            continue;
        }
        let mut line = vec![pts[s]];
        for i in s..e - 1 {
            let seg = curves
                .binary_search_by_key(&i, |c| c.0)
                .ok()
                .map_or(Seg::Line, |k| curves[k].1);
            segment(&seg, pts[i], pts[i + 1], &mut line);
        }
        out.push(line);
        s = e;
    }
    out
}

fn geometry(blob: &[u8], g: &GeomDef) -> Result<Option<Geometry>, String> {
    let mut s = Shape {
        c: Cur::new(blob, 0),
        g,
    };
    let raw = s.c.varuint()?;
    let base = (raw & 0xFF) as u32;
    let (mut z, mut m) = classic_zm(base);
    let mut curves_flag = false;
    if (50..=54).contains(&base) {
        z = raw & EXT_Z != 0;
        m = raw & EXT_M != 0;
        curves_flag = raw & EXT_CURVES != 0;
    }
    match base {
        0 => Ok(None),
        1 | 9 | 11 | 21 | 52 => Ok(s.xy_abs()?.map(Geometry::Point)),
        8 | 18 | 20 | 28 | 53 => {
            let n = s.c.len()?;
            if n == 0 {
                return Ok(None);
            }
            s.bbox()?;
            Ok(Some(Geometry::MultiPoint(s.points(n)?)))
        }
        3 | 10 | 13 | 23 | 50 | 5 | 15 | 19 | 25 | 51 => {
            let n = s.c.len()?;
            if n == 0 {
                return Ok(None);
            }
            let np = s.c.len()?;
            let nc = if curves_flag { s.c.len()? } else { 0 };
            // Each part's size takes at least a byte: checked before the sizes are allocated.
            if np == 0 || np > n || np > s.c.left() {
                return Err("a File Geodatabase geometry is damaged".into());
            }
            s.bbox()?;
            let mut sizes = Vec::with_capacity(np);
            let mut used = 0usize;
            for _ in 0..np - 1 {
                let k = s.c.len()?;
                used = used.saturating_add(k);
                sizes.push(k);
            }
            sizes.push(n.saturating_sub(used));
            let pts = s.points(n)?;
            if z {
                s.skip_values(n, false)?;
            }
            if m {
                s.skip_values(n, true)?;
            }
            let curves = if nc > 0 {
                read_curves(&mut s, nc)?
            } else {
                Vec::new()
            };
            let mut ps = parts(&pts, &sizes, &curves);
            ps.retain(|p| !p.is_empty());
            let polygon = matches!(base, 5 | 15 | 19 | 25 | 51);
            Ok(Some(if polygon {
                crate::shp::polygons(ps)
            } else if ps.len() == 1 {
                Geometry::LineString(ps.pop().unwrap_or_default())
            } else {
                Geometry::MultiLineString(ps)
            }))
        }
        31 | 32 | 54 => {
            let n = s.c.len()?;
            if n == 0 {
                return Ok(None);
            }
            s.c.varuint()?; // a multipatch-only value (not needed to draw)
            let np = s.c.len()?;
            // Each part's size takes at least a byte: checked before the sizes are allocated.
            if np == 0 || np > n || np > s.c.left() {
                return Err("a File Geodatabase geometry is damaged".into());
            }
            s.bbox()?;
            let mut sizes = Vec::with_capacity(np);
            let mut used = 0usize;
            for _ in 0..np - 1 {
                let k = s.c.len()?;
                used = used.saturating_add(k);
                sizes.push(k);
            }
            sizes.push(n.saturating_sub(used));
            let mut kinds = Vec::with_capacity(np);
            for _ in 0..np {
                kinds.push(s.c.varuint()? & 0xF);
            }
            let pts = s.points(n)?;
            Ok(Some(multipatch(&pts, &sizes, &kinds)))
        }
        t => Err(format!("unsupported File Geodatabase shape type {t}")),
    }
}

/// Multipatch parts as polygons: triangle strips, fans and triangles one polygon per
/// triangle; rings grouped into polygons (outer or first ring, then inner rings).
fn multipatch(pts: &[Pt], sizes: &[usize], kinds: &[u64]) -> Geometry {
    let mut polys: Vec<Vec<Vec<Pt>>> = Vec::new();
    let tri = |a: Pt, b: Pt, c: Pt| vec![vec![a, b, c, a]];
    let mut s = 0usize;
    for (k, &n) in sizes.iter().enumerate() {
        let e = (s + n).min(pts.len());
        let p = &pts[s.min(e)..e];
        match kinds.get(k).copied().unwrap_or(5) {
            0 => {
                for w in p.windows(3) {
                    polys.push(tri(w[0], w[1], w[2]));
                }
            }
            1 => {
                for i in 1..p.len().saturating_sub(1) {
                    polys.push(tri(p[0], p[i], p[i + 1]));
                }
            }
            6 => {
                for t in p.chunks_exact(3) {
                    polys.push(tri(t[0], t[1], t[2]));
                }
            }
            3 => match polys.last_mut() {
                Some(last) => last.push(p.to_vec()),
                None => polys.push(vec![p.to_vec()]),
            },
            _ => polys.push(vec![p.to_vec()]),
        }
        s = e;
    }
    Geometry::MultiPolygon(polys)
}

// ----------------------------------------------------------------------------- catalog

/// A geometry field's CRS: its WKT, or the WKID GDB_Items gives when the WKT names no
/// EPSG code (or does not parse).
fn layer_crs(g: &GeomDef, item: Option<&Item>) -> (Option<Crs>, Option<String>) {
    let by_wkt: Option<Crs> = (!g.wkt.trim().is_empty())
        .then(|| crate::wkt::crs_from_wkt(&g.wkt))
        .flatten();
    let by_wkid = item.and_then(|i| i.wkid).and_then(crs::from_epsg);
    let c = match (by_wkt, by_wkid) {
        (Some(w), Some(k)) if w.epsg.is_none() => Some(k),
        (Some(w), _) => Some(w),
        (None, k) => k,
    };
    let note = if c.is_none() {
        item.and_then(|i| i.wkid)
            .map(|k| format!("WKID {k}"))
            .or_else(|| {
                let w = g.wkt.trim();
                (!w.is_empty() && !w.starts_with('{')).then(|| w.chars().take(80).collect())
            })
    } else {
        None
    };
    (c, note)
}

fn same_crs(a: Option<&Crs>, b: Option<&Crs>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => match (a.epsg, b.epsg) {
            (Some(x), Some(y)) => x == y,
            _ => a.name == b.name,
        },
        _ => false,
    }
}

fn map_points(g: &mut Geometry, f: &mut dyn FnMut(Pt) -> Pt) {
    let line = |v: &mut Vec<Pt>, f: &mut dyn FnMut(Pt) -> Pt| v.iter_mut().for_each(|p| *p = f(*p));
    match g {
        Geometry::Point(p) => *p = f(*p),
        Geometry::MultiPoint(v) | Geometry::LineString(v) => line(v, f),
        Geometry::MultiLineString(v) | Geometry::Polygon(v) => {
            v.iter_mut().for_each(|l| line(l, f))
        }
        Geometry::MultiPolygon(v) => v.iter_mut().flatten().for_each(|l| line(l, f)),
        Geometry::Collection(c) => c.iter_mut().for_each(|g| map_points(g, f)),
    }
}

/// The `.gdb` directory a path names or lies in.
fn gdb_dir(path: &Path) -> Option<PathBuf> {
    let is_gdb = |p: &Path| {
        p.is_dir()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("gdb"))
    };
    if is_gdb(path) {
        return Some(path.to_path_buf());
    }
    let parent = path.parent()?;
    is_gdb(parent).then(|| parent.to_path_buf())
}

/// Whether `path` is a File Geodatabase directory (named `*.gdb`, with a system catalog).
pub fn sniff_dir(path: &Path) -> bool {
    path.is_dir()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("gdb"))
        && path.join("a00000001.gdbtable").is_file()
}

/// What GDB_Items says about a table.
#[derive(Default)]
struct Item {
    wkid: Option<u32>,
    /// Date fields stored with millisecond precision.
    high_precision: Vec<String>,
}

fn items(dir: &Path, catalog: &[(u64, String)]) -> std::collections::HashMap<String, Item> {
    let mut out = std::collections::HashMap::new();
    let Some((id, _)) = catalog.iter().find(|(_, n)| n == "GDB_Items") else {
        return out;
    };
    let Ok(t) = Table::open(dir, *id, "GDB_Items") else {
        return out;
    };
    let names = t.attribute_names();
    let col = |n: &str| names.iter().position(|f| f.eq_ignore_ascii_case(n));
    let (Some(pn), Some(def)) = (col("PhysicalName"), col("Definition")) else {
        return out;
    };
    for (_, off) in t.rows() {
        let Ok((v, _)) = t.row(off) else { continue };
        let Some(Value::Text(name)) = v.get(pn) else {
            continue;
        };
        let mut item = Item::default();
        if let Some(Value::Text(xml)) = v.get(def)
            && specular_limits::xml_within_depth(xml.as_bytes(), specular_limits::MAX_XML_DEPTH)
            && let Ok(doc) = roxmltree::Document::parse(xml)
        {
            let find = |tag: &str| {
                doc.descendants()
                    .filter(|n| n.tag_name().name() == tag)
                    .find_map(|n| n.text()?.trim().parse::<u32>().ok())
            };
            item.wkid = find("LatestWKID").or_else(|| find("WKID"));
            item.high_precision = doc
                .descendants()
                .filter(|n| {
                    n.tag_name().name() == "HighPrecision"
                        && n.text().is_some_and(|t| t.trim() == "true")
                })
                .filter_map(|n| {
                    n.parent()?
                        .children()
                        .find(|c| c.tag_name().name() == "Name")?
                        .text()
                        .map(|t| t.trim().to_string())
                })
                .collect();
        }
        out.insert(name.to_ascii_uppercase(), item);
    }
    out
}

/// Reads a File Geodatabase: `path` is the `.gdb` directory or a file inside it.
pub fn read(path: &Path) -> Result<Dataset, String> {
    let dir = gdb_dir(path).ok_or("not a File Geodatabase (a folder named .gdb)")?;
    let cat = Table::open(&dir, 1, "the system catalog")?;
    let names = cat.attribute_names();
    let ni = names
        .iter()
        .position(|n| n.eq_ignore_ascii_case("Name"))
        .ok_or("the system catalog has no table names")?;
    let mut catalog = Vec::new();
    for (oid, off) in cat.rows() {
        if let Ok((v, _)) = cat.row(off)
            && let Some(Value::Text(n)) = v.get(ni)
        {
            catalog.push((oid, n.clone()));
        }
    }
    let items = items(&dir, &catalog);
    let mut ds = Dataset {
        format: "File Geodatabase",
        ..Dataset::default()
    };
    let mut refused = Vec::new();
    let mut first_crs = true;
    let mut vertices = 0usize;
    let mut features = 0usize;
    for (id, name) in &catalog {
        if name.to_ascii_uppercase().starts_with("GDB_") {
            continue;
        }
        if !table_paths(&dir, *id).0.is_file() {
            continue;
        }
        let mut t = match Table::open(&dir, *id, name) {
            Ok(t) => t,
            Err(e) => {
                refused.push(e);
                continue;
            }
        };
        if let Some(item) = items.get(&name.to_ascii_uppercase()) {
            for f in &mut t.fields {
                f.high_precision = item
                    .high_precision
                    .iter()
                    .any(|n| n.eq_ignore_ascii_case(&f.name));
            }
        }
        let mut layer = Layer::new(name.clone());
        let mut other_crs: Option<Option<Crs>> = None;
        for f in t.attribute_names() {
            layer.field(&f);
        }
        let item = items.get(&name.to_ascii_uppercase());
        if let Some(g) = &t.geom {
            layer.declared_type = Some(
                match t.geom_type {
                    1 => "Point",
                    2 => "MultiPoint",
                    3 => "MultiLineString",
                    4 => "MultiPolygon",
                    9 => "MultiPatch",
                    _ => "Geometry",
                }
                .to_string(),
            );
            let (c, note) = layer_crs(g, item);
            if first_crs {
                first_crs = false;
                ds.crs = c;
                ds.crs_note = note;
            } else if !same_crs(c.as_ref(), ds.crs.as_ref()) {
                other_crs = Some(c);
            }
        }
        let mut bad = 0usize;
        for (oid, off) in t.rows() {
            if features >= MAX_FEATURES || vertices > MAX_VERTICES {
                ds.warn("too many features; the rest are not shown");
                break;
            }
            let (values, blob) = match t.row(off) {
                Ok(r) => r,
                Err(e) if e == "deleted" => continue,
                Err(_) => {
                    bad += 1;
                    continue;
                }
            };
            let geometry = match (blob, &t.geom) {
                (Some(b), Some(g)) => match geometry(b, g) {
                    Ok(g) => g,
                    Err(_) => {
                        bad += 1;
                        None
                    }
                },
                _ => None,
            };
            vertices += geometry.as_ref().map_or(0, Geometry::vertex_count);
            features += 1;
            layer.features.push(Feature {
                id: Some(oid.to_string()),
                geometry,
                values,
            });
        }
        if bad > 0 {
            ds.warn(format!("{name}: {bad} damaged rows are not shown"));
        }
        // A layer in another coordinate system is projected into the first one's.
        if let Some(from) = other_crs {
            match (from, ds.crs.as_ref()) {
                (Some(f), Some(to)) if f.convertible() && to.convertible() => {
                    let mut lost = false;
                    for feat in &mut layer.features {
                        if let Some(g) = &mut feat.geometry {
                            map_points(g, &mut |p| match f
                                .to_wgs84(p[0], p[1])
                                .and_then(|(lon, lat)| to.from_wgs84(lon, lat))
                            {
                                Some((x, y)) => [x, y],
                                None => {
                                    lost = true;
                                    [f64::NAN; 2]
                                }
                            });
                        }
                    }
                    ds.warn(format!("{name}: projected from {} to {}", f.name, to.name));
                    if lost {
                        ds.warn(format!("{name}: some points could not be projected"));
                    }
                }
                (f, _) => ds.warn(format!(
                    "{name}: in another coordinate system ({}), drawn as stored",
                    f.map_or_else(|| "unknown".to_string(), |c| c.name)
                )),
            }
        }
        ds.layers.push(layer);
    }
    if ds.layers.is_empty() {
        return Err(refused
            .first()
            .cloned()
            .unwrap_or_else(|| "the File Geodatabase holds no tables".into()));
    }
    for r in refused {
        ds.warn(r);
    }
    Ok(ds)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A polyline claiming 10^12 points in 10^12 parts, in a dozen bytes: refused before
    /// the part sizes are allocated (8 TB).
    #[test]
    fn part_count_bomb() {
        let mut b = Vec::new();
        for v in [3u64, 1 << 40, 1 << 40, 0, 0, 0, 0] {
            vu(v, &mut b);
        }
        let e = geometry(&b, &GeomDef::default()).unwrap_err();
        assert!(e.contains("damaged"), "{e}");
    }

    /// Esri unsigned and signed varints.
    fn vu(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let b = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                return;
            }
            out.push(b | 0x80);
        }
    }
    fn vi(v: i64, out: &mut Vec<u8>) {
        let (neg, mut a) = (v < 0, v.unsigned_abs());
        let mut b = (a & 0x3F) as u8 | if neg { 0x40 } else { 0 };
        a >>= 6;
        if a == 0 {
            out.push(b);
            return;
        }
        b |= 0x80;
        out.push(b);
        vu(a, out);
    }
    fn utf16(s: &str, out: &mut Vec<u8>) {
        out.push(s.encode_utf16().count() as u8);
        for u in s.encode_utf16() {
            out.extend_from_slice(&u.to_le_bytes());
        }
    }

    /// A table file pair: fields (name, type code, nullable) with an optional geometry
    /// field, and rows as raw blobs (None: deleted).
    fn table(
        fields: &[(&str, u8, bool)],
        geom_type: u8,
        rows: &[Option<Vec<u8>>],
    ) -> (Vec<u8>, Vec<u8>) {
        let mut f = Vec::new();
        f.extend_from_slice(&4u32.to_le_bytes());
        f.extend_from_slice(&[geom_type, 1, 0, 0]);
        f.extend_from_slice(&(fields.len() as u16).to_le_bytes());
        for &(name, t, nullable) in fields {
            utf16(name, &mut f);
            f.push(0);
            f.push(t);
            let fl = if nullable { 1 } else { 0 };
            match t {
                6 => f.extend_from_slice(&[4, 2]),
                4 => {
                    f.extend_from_slice(&255u32.to_le_bytes());
                    f.extend_from_slice(&[fl, 0]);
                }
                7 => {
                    f.extend_from_slice(&[0, fl]);
                    let wkt: Vec<u8> = "GEOGCS[\"GCS_WGS_1984\",DATUM[\"D_WGS_1984\",SPHEROID[\"WGS_1984\",6378137.0,298.257223563]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433],AUTHORITY[\"EPSG\",4326]]"
                        .encode_utf16()
                        .flat_map(|u| u.to_le_bytes())
                        .collect();
                    f.extend_from_slice(&(wkt.len() as u16).to_le_bytes());
                    f.extend_from_slice(&wkt);
                    f.push(0); // no Z, no M
                    for v in [-400.0f64, -400.0, 1e9, 1e-9, -180.0, -90.0, 180.0, 90.0] {
                        f.extend_from_slice(&v.to_le_bytes());
                    }
                    f.push(0);
                    f.extend_from_slice(&1u32.to_le_bytes());
                    f.extend_from_slice(&1.0f64.to_le_bytes());
                }
                _ => f.extend_from_slice(&[4, fl, 0]),
            }
        }
        let mut data = vec![0u8; 40];
        data[0..4].copy_from_slice(&3u32.to_le_bytes());
        data[32..40].copy_from_slice(&40u64.to_le_bytes());
        data.extend_from_slice(&(f.len() as u32).to_le_bytes());
        data.extend_from_slice(&f);
        let mut offsets = Vec::new();
        for r in rows {
            match r {
                None => offsets.push(0u64),
                Some(b) => {
                    offsets.push(data.len() as u64);
                    data.extend_from_slice(&(b.len() as i32).to_le_bytes());
                    data.extend_from_slice(b);
                }
            }
        }
        let mut x = Vec::new();
        x.extend_from_slice(&3u32.to_le_bytes());
        x.extend_from_slice(&1u32.to_le_bytes());
        x.extend_from_slice(&(rows.len() as u32).to_le_bytes());
        x.extend_from_slice(&5u32.to_le_bytes());
        for k in 0..1024 {
            let o = offsets.get(k).copied().unwrap_or(0);
            x.extend_from_slice(&o.to_le_bytes()[..5]);
        }
        (data, x)
    }

    fn string(s: &str, out: &mut Vec<u8>) {
        vu(s.len() as u64, out);
        out.extend_from_slice(s.as_bytes());
    }

    /// A geodatabase with one polygon feature class "lakes" (two rows, one deleted, one
    /// with a null name) and a plain table.
    fn fixture(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        let cat_rows: Vec<Option<Vec<u8>>> = [
            "GDB_SystemCatalog",
            "GDB_DBTune",
            "GDB_SpatialRefs",
            "lakes",
        ]
        .iter()
        .map(|n| {
            let mut b = Vec::new();
            string(n, &mut b);
            b.extend_from_slice(&0i32.to_le_bytes());
            Some(b)
        })
        .collect();
        let (d, x) = table(
            &[
                ("ID", 6, false),
                ("Name", 4, false),
                ("FileFormat", 1, false),
            ],
            0,
            &cat_rows,
        );
        std::fs::write(dir.join("a00000001.gdbtable"), d).unwrap();
        std::fs::write(dir.join("a00000001.gdbtablx"), x).unwrap();
        // A polygon: a square from (10, 50) to (11, 51) with xy scale 1e9, origin -400.
        let mut g = Vec::new();
        vu(5, &mut g);
        vu(5, &mut g);
        vu(1, &mut g);
        for _ in 0..4 {
            vu(0, &mut g);
        }
        let s = 1e9f64;
        let pts = [
            [10.0, 50.0],
            [10.0, 51.0],
            [11.0, 51.0],
            [11.0, 50.0],
            [10.0, 50.0],
        ];
        let (mut px, mut py) = (0i64, 0i64);
        for p in pts {
            let (x, y) = (
                ((p[0] + 400.0) * s).round() as i64,
                ((p[1] + 400.0) * s).round() as i64,
            );
            vi(x - px, &mut g);
            vi(y - py, &mut g);
            px = x;
            py = y;
        }
        let row = |name: Option<&str>, depth: f64| {
            let mut b = vec![if name.is_none() { 1u8 } else { 0 }];
            if let Some(n) = name {
                string(n, &mut b);
            }
            vu(g.len() as u64, &mut b);
            b.extend_from_slice(&g);
            b.extend_from_slice(&depth.to_le_bytes());
            b
        };
        let rows = vec![Some(row(Some("Lac"), 12.5)), None, Some(row(None, 3.0))];
        let (d, x) = table(
            &[
                ("OBJECTID", 6, false),
                ("name", 4, true),
                ("SHAPE", 7, true),
                ("depth", 3, false),
            ],
            4,
            &rows,
        );
        std::fs::write(dir.join("a00000004.gdbtable"), d).unwrap();
        std::fs::write(dir.join("a00000004.gdbtablx"), x).unwrap();
    }

    #[test]
    fn reads_a_feature_class() {
        let dir = std::env::temp_dir()
            .join(format!("specular-fgdb-{}", std::process::id()))
            .join("t.gdb");
        fixture(&dir);
        assert!(sniff_dir(&dir));
        let ds = read(&dir.join("a00000004.gdbtable")).unwrap();
        assert_eq!(ds.format, "File Geodatabase");
        assert_eq!(ds.crs.as_ref().and_then(|c| c.epsg), Some(4326));
        let l = &ds.layers[0];
        assert_eq!(l.name, "lakes");
        assert_eq!(l.fields(), vec!["name", "depth"]);
        assert_eq!(l.features.len(), 2);
        assert_eq!(l.features[0].id.as_deref(), Some("1"));
        assert_eq!(l.features[1].id.as_deref(), Some("3"));
        assert_eq!(
            l.features[0].values,
            vec![Value::Text("Lac".into()), Value::Float(12.5)]
        );
        assert_eq!(l.features[1].values, vec![Value::Null, Value::Float(3.0)]);
        let Some(Geometry::Polygon(r)) = &l.features[0].geometry else {
            panic!("{:?}", l.features[0].geometry)
        };
        assert_eq!(r[0].len(), 5);
        assert!((r[0][2][0] - 11.0).abs() < 1e-9 && (r[0][2][1] - 51.0).abs() < 1e-9);

        // Damaged copies never panic.
        let t = std::fs::read(dir.join("a00000004.gdbtable")).unwrap();
        let x = std::fs::read(dir.join("a00000004.gdbtablx")).unwrap();
        for cut in (0..t.len()).step_by(7) {
            std::fs::write(dir.join("a00000004.gdbtable"), &t[..cut]).unwrap();
            let _ = read(&dir);
        }
        for k in 0..t.len() {
            let mut b = t.clone();
            b[k] ^= 0xA5;
            std::fs::write(dir.join("a00000004.gdbtable"), &b).unwrap();
            let _ = read(&dir);
        }
        std::fs::write(dir.join("a00000004.gdbtable"), &t).unwrap();
        for cut in [0, 10, 16, 100] {
            std::fs::write(dir.join("a00000004.gdbtablx"), &x[..cut]).unwrap();
            let _ = read(&dir);
        }
        std::fs::write(dir.join("a00000004.gdbtablx"), &x).unwrap();
        // A compressed table is refused by name.
        std::fs::write(dir.join("a00000004.gdbtable.cdf"), b"x").unwrap();
        let e = read(&dir).unwrap_err();
        assert!(e.contains("lakes") && e.contains("(CDF)"), "{e}");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn values() {
        assert_eq!(datetime(25_569.5, Some(false)), "1970-01-01T12:00:00");
        assert_eq!(
            datetime(25_569.5 + 0.4 / 86_400.0, Some(true)),
            "1970-01-01T12:00:00.400"
        );
        assert_eq!(datetime(45_000.0, None), "2023-03-15");
        assert_eq!(time_of_day(0.75), "18:00:00");
        let mut b = Vec::new();
        vi(-1234567, &mut b);
        vi(63, &mut b);
        vi(-64, &mut b);
        let mut c = Cur::new(&b, 0);
        assert_eq!(c.varint().unwrap(), -1234567);
        assert_eq!(c.varint().unwrap(), 63);
        assert_eq!(c.varint().unwrap(), -64);
        assert_eq!(
            guid(&[
                0x78, 0x56, 0x34, 0x12, 0x34, 0x12, 0x78, 0x56, 1, 2, 3, 4, 5, 6, 7, 8
            ]),
            "{12345678-1234-5678-0102-030405060708}"
        );
    }
}
