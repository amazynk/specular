//! Reading the voxel data (attached or detached, raw, compressed or text) and gathering
//! slices out of it in any axis order.

use std::path::Path;
use std::sync::Arc;

use crate::{
    Canonical, DataFile, DataType, Encoding, Error, Header, MedVolume, Plane, Result, Skip,
};

/// A file's bytes, shared (a mapped source file, or a decoded buffer).
pub type Bytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

/// Largest decoded voxel data accepted: 16 GiB.
pub(crate) const MAX_DATA: usize = 16 << 30;

/// Decoded bytes with the memory budget's reservation for them, given back when the last
/// user of the volume drops them.
struct Held {
    bytes: Vec<u8>,
    _held: specular_core::Reservation,
}

impl AsRef<[u8]> for Held {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

/// `bytes` as a volume's buffer, holding `held` for as long as it lives.
pub(crate) fn held(bytes: Vec<u8>, held: specular_core::Reservation) -> Bytes {
    Arc::new(Held { bytes, _held: held })
}

/// A buffer the decoder made, reserved from the memory budget for as long as it lives (it
/// is in memory already: this counts it, it cannot refuse it beforehand).
pub(crate) fn counted(bytes: Vec<u8>) -> Result<Bytes> {
    let r = specular_core::budget::reserve(bytes.capacity() as u64)?;
    Ok(held(bytes, r))
}

/// An empty buffer with room for `n` bytes, reserved from the memory budget before it is
/// allocated.
pub(crate) fn alloc(n: usize) -> Result<(Vec<u8>, specular_core::Reservation)> {
    let r = specular_core::budget::reserve(n as u64)?;
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| Error::TooLarge)?;
    Ok((v, r))
}

/// Maps an inflate error, keeping a memory refusal one.
pub(crate) fn inflate_err(e: specular_png::InflateError) -> Error {
    match e {
        specular_png::InflateError::NeedsMemory(n) => Error::NeedsMemory(n),
        e => Error::Malformed(format!("compressed voxel data: {e}")),
    }
}

/// The voxel bytes `h` describes: the buffer, where the samples start in it, and their byte
/// order.
pub(crate) fn read(h: &Header, file: Bytes, path: &Path) -> Result<(Bytes, usize, bool)> {
    let need = h
        .data_bytes()
        .filter(|&n| n <= MAX_DATA)
        .ok_or(Error::TooLarge)?;
    let d = &h.data;
    // The (still encoded) bytes holding the data, and where the data starts in them.
    let (raw, start): (Bytes, usize) = match &d.file {
        DataFile::Attached => {
            let at = header_end(h, (*file).as_ref())?;
            (file, at)
        }
        DataFile::Detached(files) if files.len() == 1 => {
            let v = crate::read_named(&crate::beside(path, &files[0].to_string_lossy()))?;
            (Arc::new(v), 0)
        }
        DataFile::Detached(files) => {
            // Grows with the parts read (the files bound it), reserved before each.
            let mut res = specular_core::Reservation::none();
            let mut all = Vec::new();
            for p in files {
                if d.encoding != Encoding::Raw {
                    return Err(Error::Unsupported(
                        "compressed data split over several files".into(),
                    ));
                }
                // Enough read: a list naming one large file a million times stops here.
                if all.len() >= need {
                    break;
                }
                let v = crate::read_named(&crate::beside(path, &p.to_string_lossy()))?;
                let part = skip_into(&v, d.skip, need / files.len())?;
                let part = &part[..part.len().min(need - all.len())];
                res.grow_to((all.len() + part.len()) as u64)?;
                all.try_reserve_exact(part.len())
                    .map_err(|_| Error::TooLarge)?;
                all.extend_from_slice(part);
            }
            let n = all.len();
            if n < need {
                return Err(Error::Truncated("voxel data".into()));
            }
            return Ok((held(all, res), 0, d.big_endian));
        }
    };
    let src = &(*raw).as_ref()[start..];
    match d.encoding {
        Encoding::Raw => {
            let at = start + skip_offset(src, d.skip, need)?;
            if (*raw).as_ref().len() < at + need {
                return Err(Error::Truncated("voxel data".into()));
            }
            Ok((raw, at, d.big_endian))
        }
        Encoding::Gzip | Encoding::Zlib | Encoding::Bzip2 => {
            // Line skips count text lines before the compressed stream; byte skips count
            // decoded bytes (from the end of them for -1), as teem reads them.
            let (lines, bytes) = match d.skip {
                Skip::Start { lines, bytes } => (lines, bytes),
                Skip::FromEnd => (0, 0),
            };
            let src = &src[skip_offset(src, Skip::Start { lines, bytes: 0 }, 0)?..];
            let gz = src.starts_with(&[0x1f, 0x8b]);
            let want = need.checked_add(bytes).ok_or(Error::TooLarge)?;
            let max = match d.skip {
                Skip::FromEnd => MAX_DATA,
                Skip::Start { .. } => want.saturating_add(want / 4 + (1 << 20)).min(MAX_DATA),
            };
            let out = if d.encoding == Encoding::Bzip2 {
                // Stops once `max` bytes are out (cut there), like the inflaters.
                specular_xcf::bunzip2(src, max as u64)
                    .map_err(|e| Error::Malformed(format!("bzip2 voxel data: {e}")))?
            } else if gz {
                specular_png::inflate_gzip_members(src, max, Some(want)).map_err(inflate_err)?
            } else {
                specular_png::inflate_zlib(src, max).map_err(inflate_err)?
            };
            let skip = match d.skip {
                Skip::FromEnd => Skip::FromEnd,
                Skip::Start { .. } => Skip::Start { lines: 0, bytes },
            };
            let at = skip_offset(&out, skip, need)?;
            if out.len() < at + need {
                return Err(Error::Truncated("voxel data".into()));
            }
            Ok((counted(out)?, at, d.big_endian))
        }
        Encoding::Ascii => {
            let at = skip_offset(src, d.skip, 0)?;
            let (v, r) = ascii(&src[at..], h)?;
            Ok((held(v, r), 0, false))
        }
        Encoding::Hex => {
            let at = skip_offset(src, d.skip, 0)?;
            let (v, r) = hex(&src[at..], need)?;
            Ok((held(v, r), 0, d.big_endian))
        }
        Encoding::ByteRle => {
            let at = skip_offset(src, d.skip, 0)?;
            let (v, r) = crate::amira::byte_rle(&src[at..], need)?;
            Ok((held(v, r), 0, d.big_endian))
        }
        Encoding::Container => Err(Error::Unsupported("voxels inside a container".into())),
    }
}

/// Where an attached header ends.
fn header_end(h: &Header, file: &[u8]) -> Result<usize> {
    match h.kind {
        crate::Kind::Nrrd => crate::nrrd::data_start(file),
        crate::Kind::MetaImage => crate::meta::data_start(file),
        crate::Kind::Amira => crate::amira::data_start(file),
        // NIfTI: `vox_offset` is the skip.
        _ => Ok(0),
    }
}

/// The offset in `src` past `skip`, for `need` bytes of data.
fn skip_offset(src: &[u8], skip: Skip, need: usize) -> Result<usize> {
    match skip {
        Skip::FromEnd => src
            .len()
            .checked_sub(need)
            .ok_or_else(|| Error::Truncated("voxel data".into())),
        Skip::Start { lines, bytes } => {
            let mut at = 0;
            for _ in 0..lines {
                let nl = src[at..]
                    .iter()
                    .position(|&c| c == b'\n')
                    .ok_or_else(|| Error::Truncated("line skip".into()))?;
                at += nl + 1;
            }
            at.checked_add(bytes)
                .filter(|&a| a <= src.len())
                .ok_or_else(|| Error::Truncated("byte skip".into()))
        }
    }
}

fn skip_into(src: &[u8], skip: Skip, need: usize) -> Result<&[u8]> {
    let at = skip_offset(src, skip, need)?;
    Ok(&src[at..])
}

/// ASCII samples (whitespace or commas between them) as little-endian bytes of `h`'s type,
/// with their reservation.
fn ascii(text: &[u8], h: &Header) -> Result<(Vec<u8>, specular_core::Reservation)> {
    let n = h.layout.samples;
    let t = h.dtype;
    // Each value takes a character and a separator (the last needs none): more values
    // than that are not in the text, and nothing is reserved for them.
    let room = text.len() / 2 + 1;
    if n > room {
        return Err(Error::Truncated(format!(
            "ASCII voxel data (room for {room} of {n} values)"
        )));
    }
    let (mut out, res) = alloc(n * t.size())?;
    let s = std::str::from_utf8(text).map_err(|_| Error::Malformed("ASCII voxel data".into()))?;
    let mut count = 0;
    for tok in s
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
    {
        if count == n {
            break;
        }
        let bad = || Error::Malformed(format!("ASCII voxel value {tok:?}"));
        let int = || -> Result<i128> {
            tok.parse::<i128>()
                .or_else(|_| {
                    tok.parse::<f64>()
                        .ok()
                        .filter(|f| f.fract() == 0.0)
                        .map(|f| f as i128)
                        .ok_or(())
                })
                .map_err(|_| bad())
        };
        match t {
            DataType::U8 => out.push(int()? as u8),
            DataType::I8 => out.push(int()? as i8 as u8),
            DataType::U16 => out.extend((int()? as u16).to_le_bytes()),
            DataType::I16 => out.extend((int()? as i16).to_le_bytes()),
            DataType::U32 => out.extend((int()? as u32).to_le_bytes()),
            DataType::I32 => out.extend((int()? as i32).to_le_bytes()),
            DataType::U64 => out.extend((int()? as u64).to_le_bytes()),
            DataType::I64 => out.extend((int()? as i64).to_le_bytes()),
            DataType::F32 => out.extend(tok.parse::<f32>().map_err(|_| bad())?.to_le_bytes()),
            DataType::F64 => out.extend(tok.parse::<f64>().map_err(|_| bad())?.to_le_bytes()),
        }
        count += 1;
    }
    if count < n {
        return Err(Error::Truncated(format!(
            "ASCII voxel data ({count} of {n} values)"
        )));
    }
    Ok((out, res))
}

/// Hex-coded bytes (two digits each; whitespace ignored), with their reservation.
fn hex(text: &[u8], need: usize) -> Result<(Vec<u8>, specular_core::Reservation)> {
    // Two digits a byte: more bytes than that are not in the text.
    if need > text.len() / 2 {
        return Err(Error::Truncated("hex voxel data".into()));
    }
    let (mut out, res) = alloc(need)?;
    let mut hi: Option<u8> = None;
    for &c in text {
        if out.len() == need {
            break;
        }
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            c if c.is_ascii_whitespace() => continue,
            _ => return Err(Error::Malformed("hex voxel data".into())),
        };
        match hi.take() {
            Some(h) => out.push(h << 4 | d),
            None => hi = Some(d),
        }
    }
    if out.len() < need {
        return Err(Error::Truncated("hex voxel data".into()));
    }
    Ok((out, res))
}

/// A sample type read from bytes in either order.
trait Sample: Copy {
    const N: usize;
    fn read(b: &[u8], big: bool) -> Self;
    fn f64(self) -> f64;
}

macro_rules! sample {
    ($($t:ty),*) => {$(
        impl Sample for $t {
            const N: usize = size_of::<$t>();
            #[inline(always)]
            fn read(b: &[u8], big: bool) -> Self {
                let a: [u8; size_of::<$t>()] = b[..Self::N].try_into().expect("sample bytes");
                if big { <$t>::from_be_bytes(a) } else { <$t>::from_le_bytes(a) }
            }
            #[inline(always)]
            fn f64(self) -> f64 {
                self as f64
            }
        }
    )*};
}
sample!(u8, i8, u16, i16, u32, i32, u64, i64, f32, f64);

#[inline]
pub(crate) fn sample_f64(b: &[u8], offset: usize, s: usize, t: DataType, big: bool) -> f64 {
    let at = offset + s * t.size();
    let b = &b[at..];
    match t {
        DataType::U8 => f64::from(b[0]),
        DataType::I8 => f64::from(b[0] as i8),
        DataType::U16 => u16::read(b, big).f64(),
        DataType::I16 => i16::read(b, big).f64(),
        DataType::U32 => u32::read(b, big).f64(),
        DataType::I32 => i32::read(b, big).f64(),
        DataType::U64 => u64::read(b, big).f64(),
        DataType::I64 => i64::read(b, big).f64(),
        DataType::F32 => f32::read(b, big).f64(),
        DataType::F64 => f64::read(b, big),
    }
}

/// The sample index of canonical (0, 0, k) in volume `v` and the sample steps along the
/// canonical columns and rows.
fn walk(m: &MedVolume, c: &Canonical, k: usize, v: usize) -> (isize, isize, isize) {
    let l = &m.header.layout;
    let first = c.stored([0, 0, k]);
    let base = (first[0] * l.stride[0] + first[1] * l.stride[1] + first[2] * l.stride[2]) as isize
        + l.volume_base(v) as isize;
    let step = |t: usize| {
        let s = l.stride[c.perm[t]] as isize;
        if c.flip[t] { -s } else { s }
    };
    (base, step(0), step(1))
}

fn gather<T: Sample>(m: &MedVolume, c: &Canonical, k: usize, v: usize, comp: usize) -> Vec<T> {
    let [w, h, _] = c.dims;
    let (base, si, sj) = walk(m, c, k, v);
    let cs = m.header.layout.colour.map_or(0, |x| x.1) as isize;
    let base = base + comp as isize * cs;
    let b = &(*m.bytes).as_ref()[m.offset..];
    let big = m.big_endian;
    let mut out = Vec::with_capacity(w * h);
    for j in 0..h as isize {
        let row = base + j * sj;
        if si == 1 && !big {
            // Contiguous little-endian rows (the common case): read straight through.
            let a = row as usize * T::N;
            out.extend(
                b[a..a + w * T::N]
                    .chunks_exact(T::N)
                    .map(|x| T::read(x, false)),
            );
            continue;
        }
        for i in 0..w as isize {
            let s = (row + i * si) as usize;
            out.push(T::read(&b[s * T::N..], big));
        }
    }
    out
}

pub(crate) fn plane(m: &MedVolume, c: &Canonical, k: usize, v: usize) -> Plane {
    if let Some((n, _)) = m.header.layout.colour {
        let rgb: Vec<Vec<u8>> = (0..3.min(n)).map(|x| gather::<u8>(m, c, k, v, x)).collect();
        let mut out = Vec::with_capacity(rgb[0].len() * 3);
        for ((&r, &g), &b) in rgb[0].iter().zip(&rgb[1]).zip(&rgb[2]) {
            out.extend([r, g, b]);
        }
        return Plane::Rgb8(out);
    }
    match m.header.dtype {
        DataType::U8 => Plane::U8(gather(m, c, k, v, 0)),
        DataType::I8 => Plane::I8(gather(m, c, k, v, 0)),
        DataType::U16 => Plane::U16(gather(m, c, k, v, 0)),
        DataType::I16 => Plane::I16(gather(m, c, k, v, 0)),
        DataType::U32 => Plane::U32(gather(m, c, k, v, 0)),
        DataType::I32 => Plane::I32(gather(m, c, k, v, 0)),
        DataType::U64 => Plane::U64(gather(m, c, k, v, 0)),
        DataType::I64 => Plane::I64(gather(m, c, k, v, 0)),
        DataType::F32 => Plane::F32(gather(m, c, k, v, 0)),
        DataType::F64 => Plane::F64(gather(m, c, k, v, 0)),
    }
}

pub(crate) fn plane_f32(m: &MedVolume, c: &Canonical, k: usize, v: usize) -> Vec<f32> {
    fn scaled<T: Sample>(v: Vec<T>, s: Option<(f64, f64)>) -> Vec<f32> {
        match s {
            Some((a, b)) => v.into_iter().map(|x| (x.f64() * a + b) as f32).collect(),
            None => v.into_iter().map(|x| x.f64() as f32).collect(),
        }
    }
    let s = m.header.scale;
    if m.header.layout.colour.is_some() {
        // Colour as its luminance-free mean, for volumes rendered in grey.
        let p = (0..3)
            .map(|x| gather::<u8>(m, c, k, v, x))
            .collect::<Vec<_>>();
        return (0..p[0].len())
            .map(|i| (f32::from(p[0][i]) + f32::from(p[1][i]) + f32::from(p[2][i])) / 3.0)
            .collect();
    }
    match m.header.dtype {
        DataType::U8 => scaled(gather::<u8>(m, c, k, v, 0), s),
        DataType::I8 => scaled(gather::<i8>(m, c, k, v, 0), s),
        DataType::U16 => scaled(gather::<u16>(m, c, k, v, 0), s),
        DataType::I16 => scaled(gather::<i16>(m, c, k, v, 0), s),
        DataType::U32 => scaled(gather::<u32>(m, c, k, v, 0), s),
        DataType::I32 => scaled(gather::<i32>(m, c, k, v, 0), s),
        DataType::U64 => scaled(gather::<u64>(m, c, k, v, 0), s),
        DataType::I64 => scaled(gather::<i64>(m, c, k, v, 0), s),
        DataType::F32 => match s {
            None => gather::<f32>(m, c, k, v, 0),
            s => scaled(gather::<f32>(m, c, k, v, 0), s),
        },
        DataType::F64 => scaled(gather::<f64>(m, c, k, v, 0), s),
    }
}
