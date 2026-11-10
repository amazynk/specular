//! A bounded reader for `.safetensors` weight files (no upstream crate, no mmap).
//!
//! Layout: an 8-byte little-endian header length `n`, `n` bytes of a JSON object mapping
//! tensor names to `{"dtype", "shape", "data_offsets": [begin, end]}` (plus an optional
//! `"__metadata__"` of strings), then the data. Everything is checked before any tensor is
//! read, so a hostile file can only produce an error:
//!
//! - the header is at most [`MAX_HEADER`] bytes and lies inside the file;
//! - JSON nesting is bounded, names are unique, ranks are at most [`MAX_RANK`];
//! - every size and offset is computed with overflow checks; each tensor's byte range is
//!   exactly its element count times its element size;
//! - the ranges tile the data area: sorted, no overlaps, no holes, ending at the file end.
//!
//! Tensors load as f32 (F32, BF16, F64 widened or narrowed to f32) or f16 (F16 kept as
//! bits); other dtypes are recognised (so the file parses) but not loadable.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use half::{bf16, f16};

use crate::Error;
use crate::json::{self, Json};
use crate::tensor::Tensor;

/// Largest header accepted.
pub const MAX_HEADER: u64 = 100 << 20;
/// Highest rank accepted.
pub const MAX_RANK: usize = 8;
/// Most tensors in one file.
pub const MAX_TENSORS: usize = 1 << 20;

/// Element type as stored in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StDType {
    Bool,
    U8,
    I8,
    F8E5M2,
    F8E4M3,
    I16,
    U16,
    F16,
    BF16,
    I32,
    U32,
    F32,
    F64,
    I64,
    U64,
}

impl StDType {
    fn parse(s: &str) -> Option<StDType> {
        Some(match s {
            "BOOL" => StDType::Bool,
            "U8" => StDType::U8,
            "I8" => StDType::I8,
            "F8_E5M2" => StDType::F8E5M2,
            "F8_E4M3" => StDType::F8E4M3,
            "I16" => StDType::I16,
            "U16" => StDType::U16,
            "F16" => StDType::F16,
            "BF16" => StDType::BF16,
            "I32" => StDType::I32,
            "U32" => StDType::U32,
            "F32" => StDType::F32,
            "F64" => StDType::F64,
            "I64" => StDType::I64,
            "U64" => StDType::U64,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            StDType::Bool => "BOOL",
            StDType::U8 => "U8",
            StDType::I8 => "I8",
            StDType::F8E5M2 => "F8_E5M2",
            StDType::F8E4M3 => "F8_E4M3",
            StDType::I16 => "I16",
            StDType::U16 => "U16",
            StDType::F16 => "F16",
            StDType::BF16 => "BF16",
            StDType::I32 => "I32",
            StDType::U32 => "U32",
            StDType::F32 => "F32",
            StDType::F64 => "F64",
            StDType::I64 => "I64",
            StDType::U64 => "U64",
        }
    }

    pub fn size(self) -> usize {
        match self {
            StDType::Bool | StDType::U8 | StDType::I8 | StDType::F8E5M2 | StDType::F8E4M3 => 1,
            StDType::I16 | StDType::U16 | StDType::F16 | StDType::BF16 => 2,
            StDType::I32 | StDType::U32 | StDType::F32 => 4,
            StDType::F64 | StDType::I64 | StDType::U64 => 8,
        }
    }
}

/// One tensor's entry in the header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorInfo {
    pub name: String,
    pub dtype: StDType,
    pub shape: Vec<usize>,
    /// Byte range within the data area.
    pub begin: u64,
    pub end: u64,
}

/// A validated header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// In data order.
    pub tensors: Vec<TensorInfo>,
    /// `__metadata__`, in file order.
    pub metadata: Vec<(String, String)>,
    /// File offset of the data area.
    pub data_start: u64,
}

impl Header {
    pub fn get(&self, name: &str) -> Option<&TensorInfo> {
        self.tensors.iter().find(|t| t.name == name)
    }
}

fn bad<T>(msg: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(msg.into()))
}

/// Reads the header length from the first 8 bytes and checks it against the file length.
pub fn header_len(prefix: &[u8], file_len: u64) -> Result<u64, Error> {
    let Some(b) = prefix.get(..8) else {
        return bad("shorter than a header length");
    };
    let n = u64::from_le_bytes(b.try_into().expect("8 bytes"));
    if n > MAX_HEADER {
        return bad(format!("header of {n} bytes is above {MAX_HEADER}"));
    }
    if n > file_len.saturating_sub(8) {
        return bad("header runs past the end of the file");
    }
    Ok(n)
}

/// Parses and validates the header JSON (`text`, `n` bytes after the length) of a file of
/// `file_len` bytes.
pub fn parse_header(text: &[u8], file_len: u64) -> Result<Header, Error> {
    let data_start = 8 + text.len() as u64;
    if data_start > file_len {
        return bad("header runs past the end of the file");
    }
    let data_len = file_len - data_start;
    if text.first() != Some(&b'{') {
        return bad("header is not a JSON object");
    }
    let root = json::parse(text).map_err(|e| Error::Weights(format!("header JSON: {e}")))?;
    let Some(members) = root.as_object() else {
        return bad("header is not a JSON object");
    };
    if members.len() > MAX_TENSORS + 1 {
        return bad("too many tensors");
    }
    let mut h = Header {
        data_start,
        ..Header::default()
    };
    let mut names: Vec<&str> = Vec::with_capacity(members.len());
    for (name, v) in members {
        names.push(name);
        if name == "__metadata__" {
            let Some(meta) = v.as_object() else {
                return bad("__metadata__ is not an object");
            };
            for (k, mv) in meta {
                let Some(s) = mv.as_str() else {
                    return bad("__metadata__ values must be strings");
                };
                h.metadata.push((k.clone(), s.to_owned()));
            }
            continue;
        }
        h.tensors.push(tensor_info(name, v)?);
    }
    names.sort_unstable();
    if names.windows(2).any(|w| w[0] == w[1]) {
        return bad("a name appears twice");
    }
    // The ranges must tile the data area exactly.
    h.tensors.sort_by_key(|t| (t.begin, t.end));
    let mut at = 0u64;
    for t in &h.tensors {
        if t.begin != at {
            return bad(format!(
                "tensor {:?} starts at {} where {} was expected (overlap or hole)",
                t.name, t.begin, at
            ));
        }
        at = t.end;
    }
    if at != data_len {
        return bad(format!(
            "tensor data ends at {at}, the data area is {data_len} bytes"
        ));
    }
    Ok(h)
}

fn tensor_info(name: &str, v: &Json) -> Result<TensorInfo, Error> {
    let Some(obj) = v.as_object() else {
        return bad(format!("{name:?} is not an object"));
    };
    let (mut dtype, mut shape, mut offsets) = (None, None, None);
    for (k, x) in obj {
        let slot = match k.as_str() {
            "dtype" => &mut dtype,
            "shape" => &mut shape,
            "data_offsets" => &mut offsets,
            _ => return bad(format!("{name:?} has an unknown field {k:?}")),
        };
        if slot.replace(x).is_some() {
            return bad(format!("{name:?} repeats {k:?}"));
        }
    }
    let (Some(dtype), Some(shape), Some(offsets)) = (dtype, shape, offsets) else {
        return bad(format!("{name:?} lacks dtype, shape or data_offsets"));
    };
    let Some(dtype) = dtype.as_str().and_then(StDType::parse) else {
        return bad(format!("{name:?} has an unknown dtype"));
    };
    let Some(dims) = shape.as_array() else {
        return bad(format!("{name:?}: shape is not an array"));
    };
    if dims.len() > MAX_RANK {
        return bad(format!("{name:?}: rank {} above {MAX_RANK}", dims.len()));
    }
    let mut shape = Vec::with_capacity(dims.len());
    let mut count: u64 = 1;
    for d in dims {
        let Some(d) = d.as_u64() else {
            return bad(format!(
                "{name:?}: a dimension is not a non-negative integer"
            ));
        };
        count = count
            .checked_mul(d)
            .ok_or(Error::Weights(format!("{name:?}: size overflows")))?;
        shape.push(usize::try_from(d).map_err(|_| Error::Weights("dimension too large".into()))?);
    }
    let bytes = count
        .checked_mul(dtype.size() as u64)
        .ok_or(Error::Weights(format!("{name:?}: size overflows")))?;
    let off = offsets.as_array().unwrap_or(&[]);
    let (Some(begin), Some(end)) = (
        off.first().and_then(Json::as_u64),
        off.get(1).and_then(Json::as_u64),
    ) else {
        return bad(format!("{name:?}: data_offsets is not [begin, end]"));
    };
    if off.len() != 2 || end < begin || end - begin != bytes {
        return bad(format!(
            "{name:?}: data_offsets [{begin}, {end}] do not hold {bytes} bytes"
        ));
    }
    Ok(TensorInfo {
        name: name.to_owned(),
        dtype,
        shape,
        begin,
        end,
    })
}

/// Converts one tensor's bytes (exactly its range) to a [`Tensor`].
fn decode(info: &TensorInfo, bytes: &[u8]) -> Result<Tensor, Error> {
    debug_assert_eq!(bytes.len() as u64, info.end - info.begin);
    let shape = &info.shape;
    Ok(match info.dtype {
        StDType::F32 => Tensor::f32(
            shape,
            bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().expect("4 bytes")))
                .collect(),
        ),
        StDType::F16 => Tensor::f16_bits(
            shape,
            bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes(c.try_into().expect("2 bytes")))
                .collect(),
        ),
        StDType::BF16 => Tensor::f32(
            shape,
            bytes
                .chunks_exact(2)
                .map(|c| {
                    bf16::from_bits(u16::from_le_bytes(c.try_into().expect("2 bytes"))).to_f32()
                })
                .collect(),
        ),
        StDType::F64 => Tensor::f32(
            shape,
            bytes
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().expect("8 bytes")) as f32)
                .collect(),
        ),
        other => {
            return bad(format!(
                "{:?}: {} tensors are not loadable",
                info.name,
                other.name()
            ));
        }
    })
}

/// A whole file in memory.
#[derive(Clone, Debug)]
pub struct SafeTensors<'a> {
    pub header: Header,
    data: &'a [u8],
}

impl<'a> SafeTensors<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<SafeTensors<'a>, Error> {
        let len = bytes.len() as u64;
        let n = header_len(bytes, len)?;
        let header = parse_header(&bytes[8..8 + n as usize], len)?;
        let data = &bytes[header.data_start as usize..];
        Ok(SafeTensors { header, data })
    }

    /// The raw bytes of one tensor.
    pub fn bytes(&self, name: &str) -> Result<&'a [u8], Error> {
        let Some(t) = self.header.get(name) else {
            return bad(format!("no tensor {name:?}"));
        };
        Ok(&self.data[t.begin as usize..t.end as usize])
    }

    pub fn tensor(&self, name: &str) -> Result<Tensor, Error> {
        let b = self.bytes(name)?;
        decode(self.header.get(name).expect("found above"), b)
    }
}

/// A file read on demand: the header at open, each tensor when asked for.
#[derive(Debug)]
pub struct SafeTensorsFile {
    pub header: Header,
    file: File,
}

impl SafeTensorsFile {
    pub fn open(path: &Path) -> Result<SafeTensorsFile, Error> {
        let io = |e: std::io::Error| Error::Weights(format!("{}: {e}", path.display()));
        let mut file = File::open(path).map_err(io)?;
        let len = file.metadata().map_err(io)?.len();
        let mut prefix = [0u8; 8];
        file.read_exact(&mut prefix).map_err(io)?;
        let n = header_len(&prefix, len)?;
        let mut text = vec![0u8; n as usize];
        file.read_exact(&mut text).map_err(io)?;
        let header = parse_header(&text, len)?;
        Ok(SafeTensorsFile { header, file })
    }

    /// A reader over `file` with a header already checked (offsets from
    /// `header.data_start`): how [`crate::pt::PtFile`] presents a PyTorch archive.
    pub(crate) fn from_parts(header: Header, file: File) -> SafeTensorsFile {
        SafeTensorsFile { header, file }
    }

    pub fn tensor(&mut self, name: &str) -> Result<Tensor, Error> {
        let Some(t) = self.header.get(name) else {
            return bad(format!("no tensor {name:?}"));
        };
        let io = |e: std::io::Error| Error::Weights(format!("reading {name:?}: {e}"));
        let len =
            usize::try_from(t.end - t.begin).map_err(|_| Error::Weights("too large".into()))?;
        let mut buf = vec![0u8; len];
        self.file
            .seek(SeekFrom::Start(self.header.data_start + t.begin))
            .map_err(io)?;
        self.file.read_exact(&mut buf).map_err(io)?;
        decode(t, &buf)
    }
}

/// Writes tensors as a safetensors file (for tests, fuzz seeds and exported test weights).
pub fn serialize(tensors: &[(&str, &Tensor)], metadata: &[(&str, &str)]) -> Vec<u8> {
    let mut header = String::from("{");
    let mut at = 0usize;
    let mut first = true;
    let mut sep = |h: &mut String| {
        if !first {
            h.push(',');
        }
        first = false;
    };
    if !metadata.is_empty() {
        sep(&mut header);
        header.push_str("\"__metadata__\":{");
        for (i, (k, v)) in metadata.iter().enumerate() {
            if i > 0 {
                header.push(',');
            }
            header.push_str(&format!("{}:{}", quote(k), quote(v)));
        }
        header.push('}');
    }
    let mut data = Vec::new();
    for (name, t) in tensors {
        let (dtype, bytes): (&str, Vec<u8>) = match &t.data {
            crate::tensor::Data::F32(v) => {
                ("F32", v.iter().flat_map(|x| x.to_le_bytes()).collect())
            }
            crate::tensor::Data::F16(v) => {
                ("F16", v.iter().flat_map(|x| x.to_le_bytes()).collect())
            }
        };
        sep(&mut header);
        let dims: Vec<String> = t.shape.iter().map(|d| d.to_string()).collect();
        header.push_str(&format!(
            "{}:{{\"dtype\":\"{dtype}\",\"shape\":[{}],\"data_offsets\":[{at},{}]}}",
            quote(name),
            dims.join(","),
            at + bytes.len()
        ));
        at += bytes.len();
        data.extend_from_slice(&bytes);
    }
    header.push('}');
    // Pad the header with spaces to a multiple of 8 (as the reference writer does).
    while header.len() % 8 != 0 {
        header.push(' ');
    }
    let mut out = (header.len() as u64).to_le_bytes().to_vec();
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(&data);
    out
}

/// What [`convert_f16`] stores a tensor as: matrices and tables (rank ≥ 2) of a float type
/// as F16 unless `keep` says otherwise, other float tensors as F32 (biases and norms keep
/// their precision), anything else as it is.
fn converted(t: &TensorInfo, keep: &dyn Fn(&TensorInfo) -> bool) -> StDType {
    match t.dtype {
        StDType::F32 | StDType::BF16 | StDType::F64 | StDType::F16
            if t.shape.len() >= 2 && !keep(t) =>
        {
            StDType::F16
        }
        StDType::F32 | StDType::BF16 | StDType::F64 => StDType::F32,
        other => other,
    }
}

/// Elements converted per read (bounds the memory a conversion needs).
const CONVERT_CHUNK: usize = 1 << 20;

/// Rewrites the safetensors file `src` with its matrices in f16 at `dst` (via `dst.part`,
/// renamed into place when complete): half the size of an f32 checkpoint, read exactly as
/// the runtime would keep the weights anyway. Streams tensor by tensor in bounded chunks;
/// the output depends only on the input (tensor and metadata order kept), so its hash can
/// be pinned. `keep` names matrices to keep in f32 (those the runtime reads at full
/// precision). `cancel` is polled between chunks; a cancelled or failed conversion leaves no
/// `dst`. Returns the bytes written.
pub fn convert_f16(
    src: &Path,
    dst: &Path,
    keep: &dyn Fn(&TensorInfo) -> bool,
    cancel: &dyn Fn() -> bool,
) -> Result<u64, Error> {
    convert_f16_from(SafeTensorsFile::open(src)?, dst, keep, cancel)
}

/// [`convert_f16`] of a file already open: a safetensors file, or a PyTorch checkpoint
/// ([`crate::pt::PtFile`]), whose tensors are written in its pickle order.
pub fn convert_f16_from(
    mut st: SafeTensorsFile,
    dst: &Path,
    keep: &dyn Fn(&TensorInfo) -> bool,
    cancel: &dyn Fn() -> bool,
) -> Result<u64, Error> {
    use std::io::{BufWriter, Write};
    let io = |e: std::io::Error| Error::Weights(format!("{}: {e}", dst.display()));
    // The header, with the new types and offsets.
    let mut header = String::from("{");
    if !st.header.metadata.is_empty() {
        header.push_str("\"__metadata__\":{");
        for (i, (k, v)) in st.header.metadata.iter().enumerate() {
            if i > 0 {
                header.push(',');
            }
            header.push_str(&format!("{}:{}", quote(k), quote(v)));
        }
        header.push('}');
    }
    let mut at = 0u64;
    for t in &st.header.tensors {
        let to = converted(t, keep);
        let n = (t.end - t.begin) / t.dtype.size() as u64;
        let len = n * to.size() as u64;
        if header.len() > 1 {
            header.push(',');
        }
        let dims: Vec<String> = t.shape.iter().map(|d| d.to_string()).collect();
        header.push_str(&format!(
            "{}:{{\"dtype\":\"{}\",\"shape\":[{}],\"data_offsets\":[{at},{}]}}",
            quote(&t.name),
            to.name(),
            dims.join(","),
            at + len
        ));
        at += len;
    }
    header.push('}');
    while header.len() % 8 != 0 {
        header.push(' ');
    }
    let part = dst.with_extension("part");
    let result = (|| -> Result<u64, Error> {
        let mut out = BufWriter::with_capacity(1 << 20, File::create(&part).map_err(io)?);
        out.write_all(&(header.len() as u64).to_le_bytes())
            .map_err(io)?;
        out.write_all(header.as_bytes()).map_err(io)?;
        let file = &mut st.file;
        let mut buf = Vec::new();
        let mut obuf = Vec::new();
        for t in &st.header.tensors {
            let (from, to) = (t.dtype, converted(t, keep));
            let esize = from.size();
            let total = (t.end - t.begin) as usize / esize;
            file.seek(SeekFrom::Start(st.header.data_start + t.begin))
                .map_err(io)?;
            let mut done = 0;
            while done < total {
                if cancel() {
                    return bad("conversion cancelled");
                }
                let n = (total - done).min(CONVERT_CHUNK);
                buf.resize(n * esize, 0);
                file.read_exact(&mut buf).map_err(io)?;
                obuf.clear();
                if from == to {
                    obuf.extend_from_slice(&buf);
                } else {
                    let value = |c: &[u8]| -> f32 {
                        match from {
                            StDType::F32 => f32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                            StDType::F16 => {
                                f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()
                            }
                            StDType::BF16 => {
                                bf16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()
                            }
                            _ => {
                                f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])
                                    as f32
                            }
                        }
                    };
                    for c in buf.chunks_exact(esize) {
                        let x = value(c);
                        if to == StDType::F16 {
                            obuf.extend_from_slice(&f16::from_f32(x).to_bits().to_le_bytes());
                        } else {
                            obuf.extend_from_slice(&x.to_le_bytes());
                        }
                    }
                }
                out.write_all(&obuf).map_err(io)?;
                done += n;
            }
        }
        let f = out.into_inner().map_err(|e| io(e.into_error()))?;
        f.sync_all().map_err(io)?;
        Ok(8 + header.len() as u64 + at)
    })();
    match result {
        Ok(n) => {
            std::fs::rename(&part, dst).map_err(io)?;
            Ok(n)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

fn quote(s: &str) -> String {
    let mut q = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => q.push_str("\\\""),
            '\\' => q.push_str("\\\\"),
            c if (c as u32) < 0x20 => q.push_str(&format!("\\u{:04x}", c as u32)),
            c => q.push(c),
        }
    }
    q.push('"');
    q
}

/// f16 bits of an f32 (round to nearest even), for callers building weights by hand.
pub fn f16_bits(x: f32) -> u16 {
    f16::from_f32(x).to_bits()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(header: &str, data_len: usize) -> Vec<u8> {
        let mut v = (header.len() as u64).to_le_bytes().to_vec();
        v.extend_from_slice(header.as_bytes());
        v.extend(std::iter::repeat_n(0u8, data_len));
        v
    }

    #[test]
    fn round_trip() {
        let a = Tensor::f32(&[2, 3], vec![1.0, -2.0, 3.5, 0.0, 1e-3, 7.0]);
        let b = Tensor::f32(&[3], vec![0.5, 1.5, -2.5]).to_f16();
        let s = Tensor::f32(&[], vec![4.0]);
        let bytes = serialize(
            &[("w.a", &a), ("b \"q\"", &b), ("s", &s)],
            &[("format", "pt")],
        );
        let st = SafeTensors::parse(&bytes).unwrap();
        assert_eq!(st.tensor("w.a").unwrap(), a);
        assert_eq!(st.tensor("b \"q\"").unwrap(), b);
        assert_eq!(st.tensor("s").unwrap(), s);
        assert_eq!(st.header.metadata, vec![("format".into(), "pt".into())]);
        assert!(st.tensor("nope").is_err());
    }

    #[test]
    fn bf16_and_f64_load_as_f32() {
        let hdr = r#"{"a":{"dtype":"BF16","shape":[2],"data_offsets":[0,4]},"b":{"dtype":"F64","shape":[1],"data_offsets":[4,12]}}"#;
        let mut v = file(hdr, 0);
        v.extend_from_slice(&bf16::from_f32(1.5).to_bits().to_le_bytes());
        v.extend_from_slice(&bf16::from_f32(-3.0).to_bits().to_le_bytes());
        v.extend_from_slice(&2.25f64.to_le_bytes());
        let st = SafeTensors::parse(&v).unwrap();
        assert_eq!(st.tensor("a").unwrap().to_f32(), vec![1.5, -3.0]);
        assert_eq!(st.tensor("b").unwrap().to_f32(), vec![2.25]);
    }

    #[test]
    fn rejects_hostile_headers() {
        let cases = [
            // Overlap.
            (
                r#"{"a":{"dtype":"F32","shape":[2],"data_offsets":[0,8]},"b":{"dtype":"F32","shape":[1],"data_offsets":[4,8]}}"#,
                8,
            ),
            // Hole.
            (
                r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[4,8]}}"#,
                8,
            ),
            // Size mismatch.
            (
                r#"{"a":{"dtype":"F32","shape":[3],"data_offsets":[0,8]}}"#,
                8,
            ),
            // Data shorter than claimed.
            (
                r#"{"a":{"dtype":"F32","shape":[4],"data_offsets":[0,16]}}"#,
                8,
            ),
            // Trailing data.
            (
                r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#,
                8,
            ),
            // Overflowing shape.
            (
                r#"{"a":{"dtype":"F32","shape":[4294967296,4294967296],"data_offsets":[0,0]}}"#,
                0,
            ),
            // Duplicate names.
            (
                r#"{"a":{"dtype":"F32","shape":[0],"data_offsets":[0,0]},"a":{"dtype":"F32","shape":[0],"data_offsets":[0,0]}}"#,
                0,
            ),
            // Unknown dtype, negative dim, fractional offset, extra field, deep rank.
            (
                r#"{"a":{"dtype":"F31","shape":[1],"data_offsets":[0,4]}}"#,
                4,
            ),
            (
                r#"{"a":{"dtype":"F32","shape":[-1],"data_offsets":[0,4]}}"#,
                4,
            ),
            (
                r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4.0]}}"#,
                4,
            ),
            (
                r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4],"x":1}}"#,
                4,
            ),
            (
                r#"{"a":{"dtype":"F32","shape":[1,1,1,1,1,1,1,1,1],"data_offsets":[0,4]}}"#,
                4,
            ),
            (
                r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4,8]}}"#,
                4,
            ),
            (r#"[1]"#, 0),
            (r#"{"__metadata__":{"a":1}}"#, 0),
        ];
        for (h, n) in cases {
            assert!(SafeTensors::parse(&file(h, n)).is_err(), "{h}");
        }
        // Header length past the end, and above the cap.
        let mut v = file("{}", 0);
        v[0] = 200;
        assert!(SafeTensors::parse(&v).is_err());
        let mut v = (MAX_HEADER + 1).to_le_bytes().to_vec();
        v.extend_from_slice(b"{}");
        assert!(SafeTensors::parse(&v).is_err());
        assert!(SafeTensors::parse(&[1, 2, 3]).is_err());
        // Integer tensors parse but do not load.
        let st = file(
            r#"{"a":{"dtype":"I64","shape":[1],"data_offsets":[0,8]}}"#,
            8,
        );
        let st = SafeTensors::parse(&st).unwrap();
        assert!(st.tensor("a").is_err());
        assert!(
            SafeTensors::parse(&file("{}", 0))
                .unwrap()
                .header
                .tensors
                .is_empty()
        );
    }

    #[test]
    fn reads_files_on_demand() {
        let dir = std::env::temp_dir();
        let path = dir.join("specular-infer-st-test.safetensors");
        let a = Tensor::f32(&[2, 2], vec![1.0, 2.0, 3.0, 4.0]);
        let b = Tensor::f32(&[1], vec![9.0]).to_f16();
        std::fs::write(&path, serialize(&[("a", &a), ("b", &b)], &[])).unwrap();
        let mut f = SafeTensorsFile::open(&path).unwrap();
        assert_eq!(f.tensor("b").unwrap(), b);
        assert_eq!(f.tensor("a").unwrap(), a);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn converts_matrices_to_f16() {
        let dir = std::env::temp_dir().join(format!("specular-infer-f16-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (src, dst) = (dir.join("a.safetensors"), dir.join("b.safetensors"));
        // Big enough for several chunks.
        let n = CONVERT_CHUNK + 7;
        let m = Tensor::f32(&[n, 1], (0..n).map(|i| i as f32 * 0.001 - 3.0).collect());
        let v = Tensor::f32(&[3], vec![0.1, 1e-7, -2.5]);
        let h = Tensor::f32(&[2, 2], vec![1.0, 2.0, 3.0, 4.0]).to_f16();
        let bytes = serialize(&[("m", &m), ("v", &v), ("h", &h)], &[("format", "pt")]);
        std::fs::write(&src, &bytes).unwrap();
        let written = convert_f16(&src, &dst, &|_| false, &|| false).unwrap();
        assert_eq!(written, std::fs::metadata(&dst).unwrap().len());
        assert!(!dst.with_extension("part").exists());
        let mut f = SafeTensorsFile::open(&dst).unwrap();
        assert_eq!(f.header.metadata, [("format".to_owned(), "pt".to_owned())]);
        assert_eq!(f.tensor("m").unwrap(), m.to_f16());
        assert_eq!(f.tensor("v").unwrap(), v);
        assert_eq!(f.tensor("h").unwrap(), h);
        // The same input gives the same bytes.
        let again = dir.join("c.safetensors");
        convert_f16(&src, &again, &|_| false, &|| false).unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(), std::fs::read(&again).unwrap());
        // Cancelled: nothing left behind.
        let gone = dir.join("d.safetensors");
        assert!(convert_f16(&src, &gone, &|_| false, &|| true).is_err());
        assert!(!gone.exists() && !gone.with_extension("part").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
