//! The classic formats: a big-endian header (dimensions, global attributes, variables with
//! their offsets) followed by the fixed-size variables, then the records, each holding one
//! slice of every record variable.

use crate::{
    Attr, Dim, Error, Format, Netcdf, Num, Packing, PlaneSpec, Result, Value, Variable, limits,
};

const TAG_DIMENSION: u32 = 0x0A;
const TAG_VARIABLE: u32 = 0x0B;
const TAG_ATTRIBUTE: u32 = 0x0C;
/// `numrecs` of a file still being written.
const STREAMING: u64 = 0xFFFF_FFFF;

/// Where a variable's values are.
#[derive(Clone, Debug)]
pub(crate) struct Store {
    pub begin: u64,
    /// Bytes per element.
    pub esize: u64,
    /// For a record variable, the bytes from one record to the next.
    pub record: Option<u64>,
    /// File length, for bounds.
    pub file_len: u64,
}

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
    /// CDF-5: counts are 64-bit.
    wide: bool,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: u64) -> Result<&'a [u8]> {
        let n = usize::try_from(n).map_err(|_| Error::TooLarge)?;
        let end = self.pos.checked_add(n).filter(|&e| e <= self.b.len());
        let end = end.ok_or_else(|| Error::Malformed("header past the end of the file".into()))?;
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(
            self.bytes(4)?.try_into().unwrap_or_default(),
        ))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(
            self.bytes(8)?.try_into().unwrap_or_default(),
        ))
    }
    /// A count (`NON_NEG`): 32-bit, or 64-bit in CDF-5.
    fn count(&mut self) -> Result<u64> {
        if self.wide {
            self.u64()
        } else {
            self.u32().map(u64::from)
        }
    }
    fn capped(&mut self, cap: u64) -> Result<u64> {
        let n = self.count()?;
        if n > cap {
            return Err(Error::TooLarge);
        }
        Ok(n)
    }
    /// Skips padding to a multiple of four.
    fn pad(&mut self, n: u64) -> Result<()> {
        self.bytes((4 - n % 4) % 4).map(drop)
    }
    fn name(&mut self) -> Result<String> {
        let n = self.capped(limits::MAX_NAME)?;
        let s = String::from_utf8_lossy(self.bytes(n)?).into_owned();
        self.pad(n)?;
        Ok(s)
    }
    /// A list header: the tag (or ABSENT, a zero tag) and the element count.
    fn list(&mut self, tag: u32) -> Result<u64> {
        let t = self.u32()?;
        let n = self.capped(limits::MAX_ITEMS)?;
        if t == 0 && n == 0 {
            return Ok(0);
        }
        if t != tag {
            return Err(Error::Malformed(format!("list tag {t:#x}")));
        }
        Ok(n)
    }
    fn attrs(&mut self) -> Result<Vec<Attr>> {
        let n = self.list(TAG_ATTRIBUTE)?;
        let mut out = Vec::with_capacity(n.min(256) as usize);
        for _ in 0..n {
            let name = self.name()?;
            let ty = self.u32()?;
            let count = self.capped(limits::MAX_ATTR_ELEMS)?;
            let size = type_size(ty)?;
            let raw = self.bytes(count * size)?;
            self.pad(count * size)?;
            out.push(Attr {
                name,
                value: attr_value(ty, raw),
            });
        }
        Ok(out)
    }
}

/// The element type of an `nc_type`, `None` for characters.
fn num(ty: u32) -> Result<Option<Num>> {
    Ok(Some(match ty {
        1 => Num::I8,
        2 => return Ok(None),
        3 => Num::I16,
        4 => Num::I32,
        5 => Num::F32,
        6 => Num::F64,
        7 => Num::U8,
        8 => Num::U16,
        9 => Num::U32,
        10 => Num::I64,
        11 => Num::U64,
        t => return Err(Error::Malformed(format!("type {t}"))),
    }))
}

fn type_size(ty: u32) -> Result<u64> {
    Ok(num(ty)?.map_or(1, |n| n.size() as u64))
}

fn attr_value(ty: u32, raw: &[u8]) -> Value {
    let Ok(Some(n)) = num(ty) else {
        let s = String::from_utf8_lossy(raw);
        return Value::Str(vec![s.trim_end_matches('\0').to_owned()]);
    };
    let count = raw.len() / n.size();
    let mut f = vec![0f64; count];
    specular_hdf5::convert(raw, n, true, n.size(), &mut f);
    let be = |k: usize, w: usize| -> [u8; 8] {
        let mut a = [0u8; 8];
        a[8 - w..].copy_from_slice(&raw[k * w..k * w + w]);
        a
    };
    match n {
        Num::F32 | Num::F64 | Num::F16 => Value::Float(f),
        // 64-bit integers exactly, not through f64.
        Num::I64 => Value::Int((0..count).map(|k| i64::from_be_bytes(be(k, 8))).collect()),
        Num::U64 => Value::UInt((0..count).map(|k| u64::from_be_bytes(be(k, 8))).collect()),
        Num::U8 | Num::U16 | Num::U32 => Value::UInt(f.iter().map(|&x| x as u64).collect()),
        _ => Value::Int(f.iter().map(|&x| x as i64).collect()),
    }
}

struct RawVar {
    name: String,
    dims: Vec<usize>,
    attrs: Vec<Attr>,
    ty: u32,
    begin: u64,
}

pub(crate) fn parse(data: &[u8]) -> Result<Netcdf> {
    let version = data[3];
    let mut r = Reader {
        b: data,
        pos: 4,
        wide: version == 5,
    };
    let format = match version {
        1 => Format::Classic,
        2 => Format::Offset64,
        5 => Format::Data64,
        _ => return Err(Error::NotNetcdf),
    };
    let mut numrecs = r.count()?;
    let nd = r.list(TAG_DIMENSION)?;
    let mut dims = Vec::with_capacity(nd.min(1024) as usize);
    for _ in 0..nd {
        let name = r.name()?;
        let len = r.count()?;
        dims.push(Dim {
            name,
            len,
            unlimited: len == 0,
        });
    }
    let attrs = r.attrs()?;
    let nv = r.list(TAG_VARIABLE)?;
    let mut raw = Vec::with_capacity(nv.min(1024) as usize);
    for _ in 0..nv {
        let name = r.name()?;
        let n = r.capped(1024)?;
        let mut ids = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let id = r.count()?;
            if id >= dims.len() as u64 {
                return Err(Error::Malformed(format!("{name}: dimension {id}")));
            }
            ids.push(id as usize);
        }
        let vattrs = r.attrs()?;
        let ty = r.u32()?;
        type_size(ty)?;
        let _vsize = r.count()?;
        let begin = if version == 1 {
            u64::from(r.u32()?)
        } else {
            r.u64()?
        };
        raw.push(RawVar {
            name,
            dims: ids,
            attrs: vattrs,
            ty,
            begin,
        });
    }
    let rec: Vec<bool> = raw
        .iter()
        .map(|v| v.dims.first().is_some_and(|&d| dims[d].unlimited))
        .collect();
    // Bytes of one record of each record variable, unpadded.
    let mut slabs = Vec::new();
    for (v, _) in raw.iter().zip(&rec).filter(|(_, r)| **r) {
        let s = v.dims[1..]
            .iter()
            .try_fold(type_size(v.ty)?, |a, &d| a.checked_mul(dims[d].len))
            .ok_or(Error::TooLarge)?;
        slabs.push((s, v.begin));
    }
    // A lone record variable is not padded between records; otherwise each slab is padded
    // to four bytes.
    let recsize = if slabs.len() == 1 {
        slabs[0].0
    } else {
        slabs.iter().try_fold(0u64, |a, (s, _)| {
            s.div_ceil(4)
                .checked_mul(4)
                .and_then(|s| a.checked_add(s))
                .ok_or(Error::TooLarge)
        })?
    };
    if numrecs == STREAMING && version != 5 || numrecs == u64::MAX {
        // Still being written: as many records as the file holds.
        let start = slabs.iter().map(|s| s.1).min().unwrap_or(0);
        numrecs = (data.len() as u64)
            .saturating_sub(start)
            .checked_div(recsize)
            .unwrap_or(0);
    }
    for d in &mut dims {
        if d.unlimited {
            d.len = numrecs;
        }
    }
    let mut vars = Vec::with_capacity(raw.len());
    for (v, is_rec) in raw.into_iter().zip(rec) {
        let n = num(v.ty)?;
        let record = is_rec.then_some(recsize);
        vars.push(Variable {
            dims: v.dims.iter().map(|&d| dims[d].name.clone()).collect(),
            shape: v.dims.iter().map(|&d| dims[d].len).collect(),
            num: n,
            packing: Packing::from_attrs(&v.attrs, n),
            name: v.name,
            attrs: v.attrs,
            store: crate::Store::Classic(Store {
                begin: v.begin,
                esize: type_size(v.ty)?,
                record,
                file_len: data.len() as u64,
            }),
        });
    }
    Ok(Netcdf {
        format,
        dims,
        attrs,
        vars,
        hdf5: None,
    })
}

/// Byte strides of each dimension.
fn strides(s: &Store, shape: &[u64]) -> Result<Vec<u64>> {
    let mut st = vec![0u64; shape.len()];
    let mut acc = s.esize;
    for d in (0..shape.len()).rev() {
        if d == 0
            && let Some(rec) = s.record
        {
            st[0] = rec;
            break;
        }
        st[d] = acc;
        acc = acc.checked_mul(shape[d]).ok_or(Error::TooLarge)?;
    }
    Ok(st)
}

/// Converts `n` elements at `off` with `stride` into `out`; past the end of the file they
/// read as NaN. Returns whether any did.
fn run(data: &[u8], s: &Store, num: Num, off: u64, stride: u64, out: &mut [f64]) -> bool {
    let n = out.len() as u64;
    if n == 0 {
        return false;
    }
    // Elements that lie wholly inside the file.
    let fit = if off.saturating_add(s.esize) > s.file_len.min(data.len() as u64) {
        0
    } else {
        ((data.len() as u64 - off - s.esize) / stride.max(1) + 1).min(n)
    };
    if fit > 0 {
        let start = off as usize;
        let span = ((fit - 1) * stride + s.esize) as usize;
        specular_hdf5::convert(
            &data[start..start + span],
            num,
            true,
            stride as usize,
            &mut out[..fit as usize],
        );
    }
    for o in &mut out[fit as usize..] {
        *o = f64::NAN;
    }
    fit < n
}

/// Reads every `step`-th sample of a plane along x and y (all of them at step 1).
pub(crate) fn plane(
    data: &[u8],
    s: &Store,
    num: Num,
    shape: &[u64],
    spec: &PlaneSpec,
    step: u64,
) -> Result<specular_hdf5::Plane<f64>> {
    let step = step.max(1);
    let r = shape.len();
    let (xd, yd) = (spec.x_dim, spec.y_dim);
    if xd >= r || (r > 1 && (yd >= r || yd == xd)) || spec.index.len() != r {
        return Err(Error::Malformed("plane dimensions".into()));
    }
    let w = shape[xd].div_ceil(step);
    let h = if r > 1 { shape[yd].div_ceil(step) } else { 1 };
    if w.checked_mul(h).is_none_or(|n| n > limits::MAX_PLANE) {
        return Err(Error::TooLarge);
    }
    let st = strides(s, shape)?;
    let mut base = s.begin;
    for d in 0..r {
        if d == xd || (r > 1 && d == yd) {
            continue;
        }
        if spec.index[d] >= shape[d] {
            return Err(Error::Malformed("plane index".into()));
        }
        base = base
            .checked_add(spec.index[d].checked_mul(st[d]).ok_or(Error::TooLarge)?)
            .ok_or(Error::TooLarge)?;
    }
    let (w, h) = (w as usize, h as usize);
    let _held = proven(data, w * h, s)?;
    let mut values = vec![0f64; w * h];
    let mut truncated = false;
    let ys = if r > 1 {
        st[yd].saturating_mul(step)
    } else {
        0
    };
    let xs = st[xd].checked_mul(step).ok_or(Error::TooLarge)?;
    for (y, row) in values.chunks_exact_mut(w.max(1)).enumerate() {
        let off = base.saturating_add((y as u64).saturating_mul(ys));
        truncated |= run(data, s, num, off, xs, row);
    }
    Ok(specular_hdf5::Plane {
        width: w,
        height: h,
        values,
        truncated,
    })
}

pub(crate) fn all(data: &[u8], s: &Store, num: Num, shape: &[u64]) -> Result<Vec<f64>> {
    let r = shape.len();
    if r == 0 {
        let mut v = [0f64];
        run(data, s, num, s.begin, s.esize, &mut v);
        return Ok(v.to_vec());
    }
    let count = shape
        .iter()
        .try_fold(1u64, |a, &c| a.checked_mul(c))
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(Error::TooLarge)?;
    let _held = proven(data, count, s)?;
    let st = strides(s, shape)?;
    let inner = shape[r - 1] as usize;
    let mut out = vec![0f64; count];
    if inner == 0 {
        return Ok(out);
    }
    for (k, row) in out.chunks_exact_mut(inner).enumerate() {
        // Position of this row over the outer dimensions.
        let mut rem = k as u64;
        let mut off = s.begin;
        for d in (0..r - 1).rev() {
            let i = rem % shape[d];
            rem /= shape[d];
            off = off.saturating_add(i.saturating_mul(st[d]));
        }
        run(data, s, num, off, st[r - 1], row);
    }
    Ok(out)
}

/// Refuses `n` values (8 bytes each once read) that the file is too short to store, and
/// reserves them from the memory budget before they are allocated.
///
/// A classic file stores every value (fill values too), so a header-only file must not size
/// a gigabyte buffer. A file cut short still reads, its missing values NaN, as long as it
/// holds at least half of a large read: the values then take at most 16 times the bytes
/// the file has (8 bytes out for each 1-byte value).
fn proven(data: &[u8], n: usize, s: &Store) -> Result<specular_core::Reservation> {
    if n > 1 << 22 && (n as u64).saturating_mul(s.esize) / 2 > data.len() as u64 {
        return Err(Error::Malformed(format!(
            "{n} values in a file of {} bytes",
            data.len()
        )));
    }
    Ok(specular_core::budget::reserve_n(n as u64, 8)?)
}

/// Reads a box (`start` and `count` per dimension), row-major over `count`.
pub(crate) fn boxed(
    data: &[u8],
    s: &Store,
    num: Num,
    shape: &[u64],
    start: &[u64],
    count: &[u64],
) -> Result<(Vec<f64>, bool)> {
    let r = shape.len();
    if start.len() != r || count.len() != r {
        return Err(Error::Malformed("box rank".into()));
    }
    for d in 0..r {
        if start[d].checked_add(count[d]).is_none_or(|e| e > shape[d]) {
            return Err(Error::Malformed("box outside the variable".into()));
        }
    }
    let n = count.iter().try_fold(1u64, |a, &c| a.checked_mul(c));
    if n.is_none_or(|n| n > limits::MAX_ALL) {
        return Err(Error::TooLarge);
    }
    let n = n.unwrap_or(0) as usize;
    if r == 0 {
        let mut v = [0f64];
        let t = run(data, s, num, s.begin, s.esize, &mut v);
        return Ok((v.to_vec(), t));
    }
    let _held = proven(data, n, s)?;
    let st = strides(s, shape)?;
    let inner = count[r - 1] as usize;
    let mut out = vec![0f64; n];
    let mut truncated = false;
    if inner == 0 {
        return Ok((out, false));
    }
    for (k, row) in out.chunks_exact_mut(inner).enumerate() {
        let mut rem = k as u64;
        let mut off = s
            .begin
            .saturating_add(start[r - 1].saturating_mul(st[r - 1]));
        for d in (0..r - 1).rev() {
            let i = rem % count[d].max(1);
            rem /= count[d].max(1);
            off = off.saturating_add((start[d] + i).saturating_mul(st[d]));
        }
        truncated |= run(data, s, num, off, st[r - 1], row);
    }
    Ok((out, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncated_headers_fail_cleanly() {
        let mut h = b"CDF\x01\0\0\0\x03\0\0\0\x0a\0\0\0\x01\0\0\0\x01x\0\0\0\0\0\0\x05".to_vec();
        for n in 4..h.len() {
            let _ = parse(&h[..n]);
        }
        h.extend_from_slice(&[0xff; 64]);
        let _ = parse(&h);
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    #[test]
    fn record_slabs_padding_overflow() {
        // CDF-5: two byte record variables over a fixed dimension of 2^64 - 1, whose
        // record slabs overflow when padded to four bytes.
        let mut h = b"CDF\x05".to_vec();
        let u64s = |h: &mut Vec<u8>, v: &[u64]| {
            for x in v {
                h.extend_from_slice(&x.to_be_bytes());
            }
        };
        u64s(&mut h, &[0]);
        h.extend_from_slice(&TAG_DIMENSION.to_be_bytes());
        u64s(&mut h, &[2, 1]);
        h.extend_from_slice(b"r\0\0\0");
        u64s(&mut h, &[0, 1]);
        h.extend_from_slice(b"x\0\0\0");
        u64s(&mut h, &[u64::MAX]);
        h.extend_from_slice(&[0; 12]);
        h.extend_from_slice(&TAG_VARIABLE.to_be_bytes());
        u64s(&mut h, &[2]);
        for name in [b"a", b"b"] {
            u64s(&mut h, &[1]);
            h.extend_from_slice(name);
            h.extend_from_slice(&[0; 3]);
            u64s(&mut h, &[2, 0, 1]);
            h.extend_from_slice(&[0; 12]);
            h.extend_from_slice(&1u32.to_be_bytes());
            u64s(&mut h, &[0, 0]);
        }
        assert!(matches!(parse(&h), Err(Error::TooLarge)));
    }
}
