//! A bounded reader for PyTorch `.pt` checkpoints (`torch.save` of a state dict) that never
//! runs code from the file.
//!
//! The file is a zip archive (entries stored, not compressed): `<name>/data.pkl`, a pickle
//! of the state dict, and `<name>/data/<key>`, one raw little-endian storage per key. A real
//! unpickler imports and calls whatever the pickle names; this one is a small stack machine
//! over the opcodes `torch.save` writes, and it builds only plain values (strings, numbers,
//! tuples, lists, dicts) and tensor references. Of the callables a pickle may name, it accepts
//! exactly:
//!
//! - `collections.OrderedDict` called with no arguments (an empty dict);
//! - `torch._utils._rebuild_tensor_v2(storage, offset, size, stride, …)` and
//!   `torch._utils._rebuild_parameter(tensor, …)` (a tensor reference);
//! - the storage classes (`torch.FloatStorage`, …) only inside a persistent id
//!   `("storage", class, key, location, numel)`.
//!
//! Anything else, any other opcode, a nesting or memo past its bound, a tensor that is not
//! contiguous or runs past its storage: an error, never an import or a call.
//!
//! [`PtFile`] presents the tensors as a [`SafeTensorsFile`] whose offsets point into the
//! archive, so the CLIP builders and [`convert_f16_from`] read it as they read safetensors.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::Error;
use crate::safetensors::{Header, MAX_RANK, MAX_TENSORS, SafeTensorsFile, StDType, TensorInfo};
use crate::tensor::Tensor;

/// Largest `data.pkl` read.
pub const MAX_PICKLE: u64 = 64 << 20;
/// Most zip entries.
const MAX_ENTRIES: usize = 1 << 20;
/// Deepest value stack and most marks open.
const MAX_STACK: usize = 1 << 16;
/// Most memo slots.
const MAX_MEMO: usize = 1 << 22;
/// Longest string or name.
const MAX_STR: usize = 1 << 20;

fn bad<T>(msg: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(msg.into()))
}

/// One stored zip entry: where its bytes start and how many there are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub offset: u64,
    pub size: u64,
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().expect("4 bytes"))
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().expect("8 bytes"))
}

fn read_at(f: &mut File, at: u64, len: usize) -> Result<Vec<u8>, Error> {
    let io = |e: std::io::Error| Error::Weights(format!("reading the archive: {e}"));
    f.seek(SeekFrom::Start(at)).map_err(io)?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf).map_err(io)?;
    Ok(buf)
}

/// The stored entries of the zip archive `f` (`len` bytes), with their data offsets.
/// Compressed or encrypted entries are refused.
pub fn zip_entries(f: &mut File, len: u64) -> Result<Vec<Entry>, Error> {
    // End of central directory: 22 bytes plus a comment of up to 65,535.
    let tail_len = len.min(22 + 65_535) as usize;
    if tail_len < 22 {
        return bad("not a zip archive (too short)");
    }
    let tail = read_at(f, len - tail_len as u64, tail_len)?;
    let Some(eocd) = (0..=tail_len - 22)
        .rev()
        .find(|&i| u32_at(&tail, i) == 0x0605_4b50)
    else {
        return bad("not a zip archive (no end of central directory)");
    };
    let mut entries = u64::from(u16_at(&tail, eocd + 10));
    let mut cd_size = u64::from(u32_at(&tail, eocd + 12));
    let mut cd_off = u64::from(u32_at(&tail, eocd + 16));
    let eocd_at = len - tail_len as u64 + eocd as u64;
    // Zip64: the locator sits right before the end record.
    if eocd_at >= 20 {
        let loc = read_at(f, eocd_at - 20, 20)?;
        if u32_at(&loc, 0) == 0x0706_4b50 {
            let at = u64_at(&loc, 8);
            if at.checked_add(56).is_none_or(|e| e > eocd_at) {
                return bad("zip64 end record out of range");
            }
            let r = read_at(f, at, 56)?;
            if u32_at(&r, 0) != 0x0606_4b50 {
                return bad("zip64 end record missing");
            }
            entries = u64_at(&r, 32);
            cd_size = u64_at(&r, 40);
            cd_off = u64_at(&r, 48);
        }
    }
    if entries > MAX_ENTRIES as u64 || cd_off.checked_add(cd_size).is_none_or(|e| e > len) {
        return bad("central directory out of range");
    }
    let cd = read_at(f, cd_off, cd_size as usize)?;
    let mut out = Vec::with_capacity(entries as usize);
    let mut at = 0usize;
    for _ in 0..entries {
        if at + 46 > cd.len() || u32_at(&cd, at) != 0x0201_4b50 {
            return bad("bad central directory entry");
        }
        let flags = u16_at(&cd, at + 8);
        let method = u16_at(&cd, at + 10);
        let mut csize = u64::from(u32_at(&cd, at + 20));
        let mut usize_ = u64::from(u32_at(&cd, at + 24));
        let nlen = usize::from(u16_at(&cd, at + 28));
        let xlen = usize::from(u16_at(&cd, at + 30));
        let clen = usize::from(u16_at(&cd, at + 32));
        let mut local = u64::from(u32_at(&cd, at + 42));
        let end = at + 46 + nlen + xlen + clen;
        if end > cd.len() {
            return bad("central directory entry runs past its end");
        }
        let name = String::from_utf8(cd[at + 46..at + 46 + nlen].to_vec())
            .map_err(|_| Error::Weights("a zip entry name is not UTF-8".into()))?;
        // Zip64 extra field: the 32-bit fields that overflowed, in this order.
        let mut x = &cd[at + 46 + nlen..at + 46 + nlen + xlen];
        while x.len() >= 4 {
            let (id, n) = (u16_at(x, 0), usize::from(u16_at(x, 2)));
            let Some(body) = x.get(4..4 + n) else {
                return bad("bad zip extra field");
            };
            if id == 1 {
                let mut p = 0;
                for v in [&mut usize_, &mut csize, &mut local] {
                    if *v == 0xffff_ffff {
                        let Some(b) = body.get(p..p + 8) else {
                            return bad("short zip64 extra field");
                        };
                        *v = u64_at(b, 0);
                        p += 8;
                    }
                }
            }
            x = &x[4 + n..];
        }
        if flags & 1 != 0 {
            return bad(format!("{name}: encrypted"));
        }
        if method != 0 || csize != usize_ {
            return bad(format!("{name}: compressed (method {method})"));
        }
        // The data follows the local header, whose extra field may differ (padding).
        if local.checked_add(30).is_none_or(|e| e > len) {
            return bad(format!("{name}: local header out of range"));
        }
        let lh = read_at(f, local, 30)?;
        if u32_at(&lh, 0) != 0x0403_4b50 {
            return bad(format!("{name}: no local header"));
        }
        let offset = local + 30 + u64::from(u16_at(&lh, 26)) + u64::from(u16_at(&lh, 28));
        if offset.checked_add(csize).is_none_or(|e| e > len) {
            return bad(format!("{name}: data runs past the end of the file"));
        }
        out.push(Entry {
            name,
            offset,
            size: csize,
        });
        at = end;
    }
    Ok(out)
}

/// A value of the restricted pickle machine.
#[derive(Clone, Debug, PartialEq)]
enum Val {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Tuple(Vec<Val>),
    List(Vec<Val>),
    Dict(Vec<(Val, Val)>),
    /// A name the pickle refers to (`module`, `name`): only a few are callable.
    Global(String, String),
    /// A persistent id resolved: (dtype, archive key, elements).
    Storage(StDType, String, u64),
    Tensor(TensorRef),
}

/// A tensor as the pickle describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct TensorRef {
    pub dtype: StDType,
    /// The storage's key (`data/<key>` in the archive).
    pub key: String,
    /// Elements in the storage.
    pub storage_len: u64,
    /// In elements.
    pub offset: u64,
    pub shape: Vec<usize>,
    pub stride: Vec<usize>,
}

fn storage_dtype(module: &str, name: &str) -> Option<StDType> {
    if module != "torch" {
        return None;
    }
    Some(match name {
        "FloatStorage" => StDType::F32,
        "HalfStorage" => StDType::F16,
        "BFloat16Storage" => StDType::BF16,
        "DoubleStorage" => StDType::F64,
        "LongStorage" => StDType::I64,
        "IntStorage" => StDType::I32,
        "ShortStorage" => StDType::I16,
        "CharStorage" => StDType::I8,
        "ByteStorage" => StDType::U8,
        "BoolStorage" => StDType::Bool,
        _ => return None,
    })
}

fn allowed_global(module: &str, name: &str) -> bool {
    matches!(
        (module, name),
        ("collections", "OrderedDict")
            | ("torch._utils", "_rebuild_tensor_v2")
            | ("torch._utils", "_rebuild_parameter")
    ) || storage_dtype(module, name).is_some()
}

fn usize_of(v: &Val, what: &str) -> Result<usize, Error> {
    match v {
        Val::Int(i) if *i >= 0 => usize::try_from(*i).map_err(|_| Error::Weights(what.into())),
        _ => bad(format!("{what} is not a non-negative integer")),
    }
}

fn dims(v: &Val, what: &str) -> Result<Vec<usize>, Error> {
    let Val::Tuple(items) = v else {
        return bad(format!("{what} is not a tuple"));
    };
    if items.len() > MAX_RANK {
        return bad(format!("{what}: rank {} above {MAX_RANK}", items.len()));
    }
    items.iter().map(|x| usize_of(x, what)).collect()
}

/// `_rebuild_tensor_v2(storage, offset, size, stride, requires_grad, hooks[, metadata])`.
fn rebuild_tensor(args: Vec<Val>) -> Result<Val, Error> {
    if !(4..=7).contains(&args.len()) {
        return bad("tensor rebuilt with the wrong number of arguments");
    }
    let Val::Storage(dtype, key, storage_len) = &args[0] else {
        return bad("tensor rebuilt from something not a storage");
    };
    let offset = usize_of(&args[1], "storage offset")? as u64;
    let shape = dims(&args[2], "size")?;
    let stride = dims(&args[3], "stride")?;
    if shape.len() != stride.len() {
        return bad("size and stride differ in rank");
    }
    Ok(Val::Tensor(TensorRef {
        dtype: *dtype,
        key: key.clone(),
        storage_len: *storage_len,
        offset,
        shape,
        stride,
    }))
}

/// Runs the restricted machine over `p` (a whole `data.pkl`) and returns the value it
/// stops with.
fn unpickle(p: &[u8]) -> Result<Val, Error> {
    struct M<'a> {
        p: &'a [u8],
        at: usize,
    }
    impl M<'_> {
        fn take(&mut self, n: usize) -> Result<&[u8], Error> {
            let end = self.at.checked_add(n).filter(|&e| e <= self.p.len());
            let Some(end) = end else {
                return bad("pickle ends early");
            };
            let s = &self.p[self.at..end];
            self.at = end;
            Ok(s)
        }
        fn u8(&mut self) -> Result<u8, Error> {
            Ok(self.take(1)?[0])
        }
        fn u16(&mut self) -> Result<u16, Error> {
            let b = self.take(2)?;
            Ok(u16::from_le_bytes([b[0], b[1]]))
        }
        fn u32(&mut self) -> Result<u32, Error> {
            Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4")))
        }
        fn u64(&mut self) -> Result<u64, Error> {
            Ok(u64::from_le_bytes(self.take(8)?.try_into().expect("8")))
        }
        fn str(&mut self, n: usize) -> Result<String, Error> {
            if n > MAX_STR {
                return bad("pickle string too long");
            }
            String::from_utf8(self.take(n)?.to_vec())
                .map_err(|_| Error::Weights("pickle string is not UTF-8".into()))
        }
        fn line(&mut self) -> Result<String, Error> {
            let rest = &self.p[self.at..];
            let Some(n) = rest.iter().take(MAX_STR).position(|&b| b == b'\n') else {
                return bad("pickle name without an end of line");
            };
            let s = self.str(n)?;
            self.at += 1;
            Ok(s)
        }
    }

    let mut m = M { p, at: 0 };
    let mut stack: Vec<Val> = Vec::new();
    let mut marks: Vec<usize> = Vec::new();
    let mut memo: HashMap<u32, Val> = HashMap::new();
    let pop = |stack: &mut Vec<Val>| {
        stack
            .pop()
            .ok_or(Error::Weights("pickle stack empty".into()))
    };
    let pop_mark = |stack: &mut Vec<Val>, marks: &mut Vec<usize>| -> Result<Vec<Val>, Error> {
        let Some(at) = marks.pop() else {
            return bad("pickle mark missing");
        };
        if at > stack.len() {
            return bad("pickle mark past the stack");
        }
        Ok(stack.split_off(at))
    };
    loop {
        if stack.len() > MAX_STACK || marks.len() > MAX_STACK {
            return bad("pickle nests too deep");
        }
        let op = m.u8()?;
        match op {
            0x80 => {
                // PROTO
                let v = m.u8()?;
                if v > 5 {
                    return bad(format!("pickle protocol {v}"));
                }
            }
            0x95 => {
                // FRAME (a length hint)
                m.u64()?;
            }
            b'.' => break, // STOP
            b'(' => marks.push(stack.len()),
            b'}' => stack.push(Val::Dict(Vec::new())),
            b']' => stack.push(Val::List(Vec::new())),
            b')' => stack.push(Val::Tuple(Vec::new())),
            b'N' => stack.push(Val::None),
            0x88 => stack.push(Val::Bool(true)),
            0x89 => stack.push(Val::Bool(false)),
            b'J' => stack.push(Val::Int(i64::from(m.u32()? as i32))),
            b'K' => stack.push(Val::Int(i64::from(m.u8()?))),
            b'M' => stack.push(Val::Int(i64::from(m.u16()?))),
            0x8a => {
                // LONG1: little-endian two's complement of n ≤ 8 bytes
                let n = usize::from(m.u8()?);
                if n > 8 {
                    return bad("pickle integer too large");
                }
                let b = m.take(n)?;
                let mut x: i64 = 0;
                for (i, &byte) in b.iter().enumerate() {
                    x |= i64::from(byte) << (8 * i);
                }
                if n > 0 && n < 8 && b[n - 1] & 0x80 != 0 {
                    x -= 1i64 << (8 * n);
                }
                stack.push(Val::Int(x));
            }
            b'G' => {
                let b = m.take(8)?;
                stack.push(Val::Float(f64::from_be_bytes(b.try_into().expect("8"))));
            }
            b'X' => {
                let n = m.u32()? as usize;
                stack.push(Val::Str(m.str(n)?));
            }
            0x8c => {
                let n = usize::from(m.u8()?);
                stack.push(Val::Str(m.str(n)?));
            }
            0x8d => {
                let n = usize::try_from(m.u64()?).unwrap_or(usize::MAX);
                stack.push(Val::Str(m.str(n)?));
            }
            b'c' => {
                // GLOBAL "module\nname\n"
                let module = m.line()?;
                let name = m.line()?;
                if !allowed_global(&module, &name) {
                    return bad(format!("the checkpoint refers to {module}.{name}"));
                }
                stack.push(Val::Global(module, name));
            }
            0x93 => {
                // STACK_GLOBAL
                let name = pop(&mut stack)?;
                let module = pop(&mut stack)?;
                let (Val::Str(module), Val::Str(name)) = (module, name) else {
                    return bad("pickle global without names");
                };
                if !allowed_global(&module, &name) {
                    return bad(format!("the checkpoint refers to {module}.{name}"));
                }
                stack.push(Val::Global(module, name));
            }
            b'q' | b'r' | 0x94 => {
                // BINPUT, LONG_BINPUT, MEMOIZE
                let key = match op {
                    b'q' => u32::from(m.u8()?),
                    b'r' => m.u32()?,
                    _ => memo.len() as u32,
                };
                let Some(top) = stack.last() else {
                    return bad("pickle memo of an empty stack");
                };
                if memo.len() >= MAX_MEMO {
                    return bad("pickle memo too large");
                }
                memo.insert(key, top.clone());
            }
            b'h' | b'j' => {
                let key = if op == b'h' {
                    u32::from(m.u8()?)
                } else {
                    m.u32()?
                };
                let Some(v) = memo.get(&key) else {
                    return bad("pickle memo slot unset");
                };
                stack.push(v.clone());
            }
            b't' => {
                let items = pop_mark(&mut stack, &mut marks)?;
                stack.push(Val::Tuple(items));
            }
            0x85..=0x87 => {
                let n = usize::from(op - 0x84);
                if stack.len() < n {
                    return bad("pickle stack too short for a tuple");
                }
                let items = stack.split_off(stack.len() - n);
                stack.push(Val::Tuple(items));
            }
            b'l' => {
                let items = pop_mark(&mut stack, &mut marks)?;
                stack.push(Val::List(items));
            }
            b'a' | b'e' => {
                let items = if op == b'a' {
                    vec![pop(&mut stack)?]
                } else {
                    pop_mark(&mut stack, &mut marks)?
                };
                match stack.last_mut() {
                    Some(Val::List(l)) => l.extend(items),
                    _ => return bad("pickle append to something not a list"),
                }
            }
            b's' | b'u' => {
                let items = if op == b's' {
                    let v = pop(&mut stack)?;
                    let k = pop(&mut stack)?;
                    vec![k, v]
                } else {
                    pop_mark(&mut stack, &mut marks)?
                };
                if items.len() % 2 != 0 {
                    return bad("pickle dict items not in pairs");
                }
                let Some(Val::Dict(d)) = stack.last_mut() else {
                    return bad("pickle item set on something not a dict");
                };
                let mut it = items.into_iter();
                while let (Some(k), Some(v)) = (it.next(), it.next()) {
                    d.push((k, v));
                }
                if d.len() > MAX_TENSORS {
                    return bad("pickle dict too large");
                }
            }
            b'Q' => {
                // BINPERSID: ("storage", class, key, location, numel)
                let pid = pop(&mut stack)?;
                let Val::Tuple(t) = pid else {
                    return bad("persistent id is not a tuple");
                };
                let [
                    Val::Str(kind),
                    Val::Global(module, name),
                    Val::Str(key),
                    _,
                    Val::Int(n),
                ] = t.as_slice()
                else {
                    return bad("persistent id is not a storage");
                };
                let Some(dtype) = storage_dtype(module, name) else {
                    return bad(format!("storage of {module}.{name}"));
                };
                if kind != "storage" || *n < 0 {
                    return bad("persistent id is not a storage");
                }
                stack.push(Val::Storage(dtype, key.clone(), *n as u64));
            }
            b'R' => {
                // REDUCE: only the callables listed in the module documentation.
                let args = pop(&mut stack)?;
                let f = pop(&mut stack)?;
                let Val::Tuple(args) = args else {
                    return bad("call arguments are not a tuple");
                };
                let Val::Global(module, name) = f else {
                    return bad("call of something not a name");
                };
                let v = match (module.as_str(), name.as_str()) {
                    ("collections", "OrderedDict") if args.is_empty() => Val::Dict(Vec::new()),
                    ("torch._utils", "_rebuild_tensor_v2") => rebuild_tensor(args)?,
                    ("torch._utils", "_rebuild_parameter") => match args.into_iter().next() {
                        Some(t @ Val::Tensor(_)) => t,
                        _ => return bad("parameter rebuilt from something not a tensor"),
                    },
                    _ => return bad(format!("the checkpoint calls {module}.{name}")),
                };
                stack.push(v);
            }
            b'b' => {
                // BUILD: state set on a dict (an OrderedDict's `_metadata`): dropped.
                pop(&mut stack)?;
                if !matches!(stack.last(), Some(Val::Dict(_))) {
                    return bad("state set on something not a dict");
                }
            }
            b'0' => {
                pop(&mut stack)?;
            }
            _ => return bad(format!("pickle opcode {op:#04x} is not read")),
        }
    }
    if stack.len() != 1 {
        return bad("pickle leaves more than one value");
    }
    Ok(stack.pop().expect("one value"))
}

/// The tensors of a state dict, in pickle order: names as saved, less a `module.` prefix.
/// A dict under `state_dict`, `weights`, `model` or `params` (BasicSR) is the state dict (as PyTorch's loaders take it);
/// values that are not tensors are skipped.
fn state_dict(v: Val) -> Result<Vec<(String, TensorRef)>, Error> {
    let Val::Dict(mut items) = v else {
        return bad("the checkpoint is not a dict");
    };
    if let Some(i) = items.iter().position(|(k, v)| {
        matches!(k, Val::Str(s) if s == "state_dict" || s == "weights" || s == "model" || s == "params")
            && matches!(v, Val::Dict(_))
    }) {
        let Val::Dict(inner) = items.swap_remove(i).1 else {
            unreachable!("checked above")
        };
        items = inner;
    }
    let mut out = Vec::new();
    for (k, v) in items {
        let (Val::Str(name), Val::Tensor(t)) = (k, v) else {
            continue;
        };
        let name = name.strip_prefix("module.").unwrap_or(&name).to_owned();
        out.push((name, t));
    }
    Ok(out)
}

/// A `.pt` checkpoint opened: its tensors as a [`SafeTensorsFile`] reading the archive.
pub struct PtFile;

impl PtFile {
    /// Opens `path` and checks every tensor (contiguous, inside its storage, a unique name);
    /// nothing is loaded yet.
    pub fn open(path: &Path) -> Result<SafeTensorsFile, Error> {
        let io = |e: std::io::Error| Error::Weights(format!("{}: {e}", path.display()));
        let mut f = File::open(path).map_err(io)?;
        let len = f.metadata().map_err(io)?.len();
        let header = header(&mut f, len)?;
        Ok(SafeTensorsFile::from_parts(header, f))
    }
}

/// The tensors of the archive `f` as a header whose offsets are file offsets
/// (`data_start` 0).
fn header(f: &mut File, len: u64) -> Result<Header, Error> {
    let entries = zip_entries(f, len)?;
    let pkl: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.name.ends_with("data.pkl") && e.name.matches('/').count() <= 1)
        .collect();
    let [pkl] = pkl.as_slice() else {
        return bad("not a PyTorch checkpoint (no single data.pkl)");
    };
    let prefix = &pkl.name[..pkl.name.len() - "data.pkl".len()];
    if let Some(bo) = entries
        .iter()
        .find(|e| e.name == format!("{prefix}byteorder"))
    {
        let b = read_at(f, bo.offset, bo.size.min(16) as usize)?;
        if b != b"little" {
            return bad("a big-endian checkpoint");
        }
    }
    if pkl.size > MAX_PICKLE {
        return bad("data.pkl is too large");
    }
    let p = read_at(f, pkl.offset, pkl.size as usize)?;
    let tensors = state_dict(unpickle(&p)?)?;
    if tensors.len() > MAX_TENSORS {
        return bad("too many tensors");
    }
    let storages: HashMap<&str, &Entry> = entries
        .iter()
        .filter_map(|e| Some((e.name.strip_prefix(prefix)?.strip_prefix("data/")?, e)))
        .collect();
    let mut infos = Vec::with_capacity(tensors.len());
    let mut names = std::collections::HashSet::new();
    for (name, t) in tensors {
        if !names.insert(name.clone()) {
            return bad(format!("{name:?} appears twice"));
        }
        let Some(e) = storages.get(t.key.as_str()) else {
            return bad(format!("{name:?}: storage {:?} missing", t.key));
        };
        let esize = t.dtype.size() as u64;
        if t.storage_len.checked_mul(esize) != Some(e.size) {
            return bad(format!("{name:?}: storage {:?} has the wrong size", t.key));
        }
        // Contiguous, row-major (a dimension of 1 may have any stride).
        let mut want = 1usize;
        for (&d, &s) in t.shape.iter().zip(&t.stride).rev() {
            if d != 1 && s != want {
                return bad(format!("{name:?} is not contiguous"));
            }
            want = want
                .checked_mul(d)
                .ok_or(Error::Weights(format!("{name:?}: size overflows")))?;
        }
        let count = want as u64;
        if t.offset
            .checked_add(count)
            .is_none_or(|e| e > t.storage_len)
        {
            return bad(format!("{name:?} runs past its storage"));
        }
        let begin = e.offset + t.offset * esize;
        infos.push(TensorInfo {
            name,
            dtype: t.dtype,
            shape: t.shape,
            begin,
            end: begin + count * esize,
        });
    }
    Ok(Header {
        tensors: infos,
        metadata: Vec::new(),
        data_start: 0,
    })
}

/// Whether `bytes` start like a zip archive (a `.pt` checkpoint) rather than safetensors.
pub fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

// -- writing (tests and fixtures) ---------------------------------------------------------

/// A stored zip of `files` (name, bytes), as `torch.save` lays it out (data aligned to 64
/// bytes with an extra field).
fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cd = Vec::new();
    for (name, data) in files {
        let local = out.len() as u32;
        let base = out.len() + 30 + name.len() + 4;
        let pad = (64 - base % 64) % 64;
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&((pad + 4) as u16).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&0x4646u16.to_le_bytes());
        out.extend_from_slice(&(pad as u16).to_le_bytes());
        out.extend(std::iter::repeat_n(b'Z', pad));
        out.extend_from_slice(data);
        cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        cd.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
        cd.extend_from_slice(&[0; 12]);
        cd.extend_from_slice(&local.to_le_bytes());
        cd.extend_from_slice(name.as_bytes());
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out
}

fn unicode(p: &mut Vec<u8>, s: &str) {
    p.push(b'X');
    p.extend_from_slice(&(s.len() as u32).to_le_bytes());
    p.extend_from_slice(s.as_bytes());
}

fn int(p: &mut Vec<u8>, x: usize) {
    p.push(b'J');
    p.extend_from_slice(&(x as u32).to_le_bytes());
}

/// A protocol-2 pickle of a state dict, one storage per tensor (name, shape, storage class),
/// as torch writes it (the rebuild function memoised and reused through BINGET).
fn pickle(tensors: &[(&str, &[usize], &str)]) -> Vec<u8> {
    let mut p = vec![0x80, 2, b'}', b'q', 0, b'('];
    for (i, (name, shape, class)) in tensors.iter().enumerate() {
        unicode(&mut p, name);
        if i == 0 {
            p.extend_from_slice(b"ctorch._utils\n_rebuild_tensor_v2\nq\x01");
        } else {
            p.extend_from_slice(b"h\x01");
        }
        p.extend_from_slice(b"((");
        unicode(&mut p, "storage");
        p.extend_from_slice(format!("ctorch\n{class}\n").as_bytes());
        unicode(&mut p, &i.to_string());
        unicode(&mut p, "cpu");
        let n: usize = shape.iter().product();
        int(&mut p, n);
        p.extend_from_slice(b"tQ");
        int(&mut p, 0);
        p.push(b'(');
        for &d in *shape {
            int(&mut p, d);
        }
        p.push(b't');
        p.push(b'(');
        let mut s = vec![0; shape.len()];
        let mut acc = 1;
        for k in (0..shape.len()).rev() {
            s[k] = acc;
            acc *= shape[k];
        }
        for d in s {
            int(&mut p, d);
        }
        p.push(b't');
        p.push(0x89);
        p.extend_from_slice(b"ccollections\nOrderedDict\n)R");
        p.extend_from_slice(b"tR");
    }
    p.extend_from_slice(b"u.");
    p
}

/// Writes `tensors` (f32 or f16) as a PyTorch checkpoint laid out as `torch.save` lays out a
/// state dict: for tests and fixtures, as [`crate::safetensors::serialize`] is.
pub fn serialize(tensors: &[(&str, &Tensor)]) -> Vec<u8> {
    let entries: Vec<(&str, &[usize], &str)> = tensors
        .iter()
        .map(|(n, t)| {
            let class = match t.dtype() {
                crate::DType::F32 => "FloatStorage",
                crate::DType::F16 => "HalfStorage",
            };
            (*n, t.shape.as_slice(), class)
        })
        .collect();
    let pkl = pickle(&entries);
    let data: Vec<Vec<u8>> = tensors.iter().map(|(_, t)| t.to_bytes()).collect();
    let names: Vec<String> = (0..tensors.len()).map(|i| format!("m/data/{i}")).collect();
    let mut files: Vec<(&str, &[u8])> = vec![("m/data.pkl", &pkl), ("m/byteorder", b"little")];
    for (n, d) in names.iter().zip(&data) {
        files.push((n, d));
    }
    files.push(("m/version", b"3\n"));
    zip(&files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("specular-infer-pt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn reads_a_state_dict() {
        let a = Tensor::f32(&[2, 3], vec![1.0, -2.0, 3.5, 0.0, 1e-3, 7.0]);
        let b = Tensor::f32(&[3], vec![0.5, 1.5, -2.5]);
        let s = Tensor::f32(&[], vec![4.0]);
        let path = scratch("state.pt");
        std::fs::write(&path, serialize(&[("module.a", &a), ("b", &b), ("s", &s)])).unwrap();
        let mut f = PtFile::open(&path).unwrap();
        let names: Vec<&str> = f.header.tensors.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "s"]);
        assert_eq!(f.tensor("a").unwrap(), a);
        assert_eq!(f.tensor("b").unwrap(), b);
        assert_eq!(f.tensor("s").unwrap(), s);
    }

    #[test]
    fn refuses_code() {
        // `os.system("x")` through GLOBAL and REDUCE.
        let evil = b"\x80\x02cos\nsystem\nX\x01\x00\x00\x00x\x85R.";
        let e = unpickle(evil).unwrap_err().to_string();
        assert!(e.contains("os.system"), "{e}");
        // The same through STACK_GLOBAL.
        let mut p = vec![0x80, 4];
        unicode(&mut p, "builtins");
        unicode(&mut p, "eval");
        p.extend_from_slice(b"\x93X\x01\x00\x00\x00x\x85R.");
        assert!(unpickle(&p).is_err());
        // An allowed name called with the wrong arguments, an unread opcode (INST), BUILD on
        // a tensor, a storage class called.
        for p in [
            &b"\x80\x02ccollections\nOrderedDict\nK\x01\x85R."[..],
            b"\x80\x02(ios\nsystem\n.",
            b"\x80\x02ctorch\nFloatStorage\n)R.",
            b"\x80\x02K\x01K\x01b.",
        ] {
            assert!(unpickle(p).is_err(), "{p:?}");
        }
    }

    #[test]
    fn refuses_bad_tensors() {
        let a = Tensor::f32(&[2, 2], vec![1.0, 2.0, 3.0, 4.0]);
        let good = serialize(&[("a", &a)]);
        let path = scratch("bad.pt");
        // A transposed view (strides (1, 2)) and an offset past the storage.
        for (from, to) in [
            (
                &b"J\x02\x00\x00\x00J\x01\x00\x00\x00t"[..],
                &b"J\x01\x00\x00\x00J\x02\x00\x00\x00t"[..],
            ),
            (b"tQJ\x00\x00\x00\x00", b"tQJ\x01\x00\x00\x00"),
        ] {
            let at = good
                .windows(from.len())
                .rposition(|w| w == from)
                .expect("pattern");
            let mut v = good.clone();
            v[at..at + from.len()].copy_from_slice(to);
            std::fs::write(&path, &v).unwrap();
            assert!(PtFile::open(&path).is_err());
        }
        // Truncated archives never panic.
        for n in [0, 10, 100, good.len() / 2, good.len() - 1] {
            std::fs::write(&path, &good[..n]).unwrap();
            assert!(PtFile::open(&path).is_err(), "{n}");
        }
        // Every truncation of the pickle errors without panicking.
        let p = pickle(&[("a", &[2, 2], "FloatStorage")]);
        for n in 0..p.len() {
            assert!(unpickle(&p[..n]).is_err());
        }
    }

    #[test]
    fn negative_long1() {
        assert_eq!(unpickle(b"\x80\x02\x8a\x01\xff.").unwrap(), Val::Int(-1));
        assert_eq!(
            unpickle(b"\x80\x02\x8a\x02\x00\x01.").unwrap(),
            Val::Int(256)
        );
    }
}
