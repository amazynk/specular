//! The binary side of the protocol, on the connection's thread: blobs (byte buffers a client
//! fills or reads in chunks, reserved from the memory budget) and arrays (`array-show`),
//! gathered into C order and written as a NumPy file the viewer then opens like any other.
//!
//! Folders, both inside the private socket folder (0700, checked as it is):
//! * `shared/`: where a client may put an array's bytes for `array-show` with `"file"`
//!   (named by a bare file name; the viewer takes the file away once read). Nothing else is
//!   ever read from there, and a symbolic link or anything but a regular file is refused.
//! * `arrays/`: the NumPy files shown, one subfolder per array (`<pid>-<n>`); a viewer keeps
//!   its last [`KEEP`] and removes its own when it quits.

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};

use specular_core::budget::{self, Reservation};

use super::protocol::{Code, Fail, MAX_BLOB};

/// Blobs one connection may hold at once.
pub const MAX_BLOBS: usize = 16;
/// Arrays a viewer keeps on disk (older ones go).
const KEEP: usize = 16;

fn no_memory(e: budget::NeedsMemory) -> Fail {
    Fail::new(Code::NoMemory, format!("the viewer {e}"))
}

struct Blob {
    data: Vec<u8>,
    _reserved: Reservation,
}

/// A connection's blobs.
#[derive(Default)]
pub struct Blobs {
    map: HashMap<u64, Blob>,
    next: u64,
}

impl Blobs {
    /// A zero-filled blob of `bytes`, reserved from the budget first.
    pub fn create(&mut self, bytes: u64) -> Result<u64, Fail> {
        self.insert_with(bytes, |n| vec![0; n])
    }

    /// Holds `data` as a new blob (a screenshot), reserving its size.
    pub fn insert(&mut self, data: Vec<u8>) -> Result<u64, Fail> {
        let mut data = Some(data);
        self.insert_with(data.as_ref().map_or(0, |d| d.len() as u64), |_| {
            data.take().unwrap_or_default()
        })
    }

    fn insert_with(
        &mut self,
        bytes: u64,
        make: impl FnOnce(usize) -> Vec<u8>,
    ) -> Result<u64, Fail> {
        if self.map.len() >= MAX_BLOBS {
            return Err(Fail::new(
                Code::Busy,
                format!("at most {MAX_BLOBS} blobs per connection: free some first"),
            ));
        }
        if bytes > MAX_BLOB {
            return Err(Fail::new(
                Code::BadArguments,
                format!("a blob holds at most {MAX_BLOB} bytes"),
            ));
        }
        let reserved = budget::reserve(bytes).map_err(no_memory)?;
        let n = usize::try_from(bytes).map_err(|_| Fail::new(Code::NoMemory, "too large"))?;
        self.next += 1;
        self.map.insert(
            self.next,
            Blob {
                data: make(n),
                _reserved: reserved,
            },
        );
        Ok(self.next)
    }

    fn get(&self, id: u64) -> Result<&Blob, Fail> {
        self.map
            .get(&id)
            .ok_or_else(|| Fail::new(Code::NotFound, format!("no blob {id}")))
    }

    pub fn len(&self, id: u64) -> Result<u64, Fail> {
        Ok(self.get(id)?.data.len() as u64)
    }

    pub fn write(&mut self, id: u64, offset: u64, data: &[u8]) -> Result<(), Fail> {
        let b = self
            .map
            .get_mut(&id)
            .ok_or_else(|| Fail::new(Code::NotFound, format!("no blob {id}")))?;
        let end = offset.checked_add(data.len() as u64);
        match end {
            Some(e) if e <= b.data.len() as u64 => {
                b.data[offset as usize..e as usize].copy_from_slice(data);
                Ok(())
            }
            _ => Err(Fail::new(
                Code::BadArguments,
                format!("blob {id} holds {} bytes", b.data.len()),
            )),
        }
    }

    pub fn read(&self, id: u64, offset: u64, length: u64) -> Result<&[u8], Fail> {
        let b = self.get(id)?;
        let len = b.data.len() as u64;
        if offset > len {
            return Err(Fail::new(
                Code::BadArguments,
                format!("blob {id} holds {len} bytes"),
            ));
        }
        let end = offset.saturating_add(length).min(len);
        Ok(&b.data[offset as usize..end as usize])
    }

    /// Takes a blob out (it is no longer the connection's).
    pub fn take(&mut self, id: u64) -> Result<Vec<u8>, Fail> {
        self.map
            .remove(&id)
            .map(|b| b.data)
            .ok_or_else(|| Fail::new(Code::NotFound, format!("no blob {id}")))
    }

    pub fn free(&mut self, id: u64) -> Result<(), Fail> {
        self.take(id).map(drop)
    }
}

/// An array to show, checked.
pub struct Array<'a> {
    pub shape: &'a [u64],
    pub dtype: &'a str,
    pub strides: Option<&'a [i64]>,
}

impl Array<'_> {
    fn itemsize(&self) -> u64 {
        let body = self.dtype.trim_start_matches(['<', '>', '|', '=']);
        body[1..].parse().unwrap_or(1)
    }

    fn count(&self) -> Option<u64> {
        self.shape.iter().try_fold(1u64, |a, &d| a.checked_mul(d))
    }

    /// The bytes the array spans in its buffer, and the offset of its first element: with
    /// negative strides the first element is not at the buffer's start.
    fn span(&self) -> Result<(u64, u64, Vec<i64>), Fail> {
        let item = self.itemsize();
        let count = self
            .count()
            .filter(|c| c.saturating_mul(item) <= MAX_BLOB)
            .ok_or_else(|| Fail::new(Code::BadArguments, "the array is too large"))?;
        let strides: Vec<i64> = match self.strides {
            Some(s) => s.to_vec(),
            None => {
                let mut s = vec![0i64; self.shape.len()];
                let mut step = item as i64;
                for (k, &d) in self.shape.iter().enumerate().rev() {
                    s[k] = step;
                    step = step.saturating_mul(d as i64);
                }
                s
            }
        };
        let (mut lo, mut hi) = (0i128, 0i128);
        for (&d, &s) in self.shape.iter().zip(&strides) {
            let reach = i128::from(s) * i128::from(d - 1);
            if reach < 0 {
                lo += reach;
            } else {
                hi += reach;
            }
        }
        let span = hi - lo + i128::from(item);
        let span = u64::try_from(span)
            .ok()
            .filter(|&s| s <= MAX_BLOB)
            .ok_or_else(|| Fail::new(Code::BadArguments, "the strides reach too far"))?;
        let _ = count;
        Ok((span, (-lo) as u64, strides))
    }

    /// The array's elements in C order, read from `data` (laid out by the strides).
    pub fn gather(&self, data: &[u8]) -> Result<Vec<u8>, Fail> {
        let (span, base, strides) = self.span()?;
        if (data.len() as u64) < span {
            return Err(Fail::new(
                Code::BadArguments,
                format!("the array needs {span} bytes, the data has {}", data.len()),
            ));
        }
        let item = self.itemsize() as usize;
        let count = self.count().unwrap_or(0) as usize;
        let c_order = {
            let mut step = item as i64;
            let mut ok = true;
            for (k, &d) in self.shape.iter().enumerate().rev() {
                ok &= d == 1 || strides[k] == step;
                step *= d as i64;
            }
            ok
        };
        if c_order {
            return Ok(data[..count * item].to_vec());
        }
        let _reserved = budget::reserve((count * item) as u64).map_err(no_memory)?;
        let mut out = Vec::with_capacity(count * item);
        let n = self.shape.len();
        let mut idx = vec![0u64; n];
        let mut off = base as i64;
        for _ in 0..count {
            let o = off as usize;
            out.extend_from_slice(&data[o..o + item]);
            // Next index, last dimension fastest.
            for k in (0..n).rev() {
                idx[k] += 1;
                off += strides[k];
                if idx[k] < self.shape[k] {
                    break;
                }
                off -= strides[k] * self.shape[k] as i64;
                idx[k] = 0;
            }
        }
        Ok(out)
    }
}

/// A NumPy `.npy` file (format 1.0, or 2.0 for a long header) of C-order `data`.
pub fn npy(dtype: &str, shape: &[u64], data: &[u8]) -> Vec<u8> {
    let descr = match dtype.chars().next() {
        Some('<' | '>' | '|') => dtype.to_owned(),
        Some('=') => format!("<{}", &dtype[1..]),
        _ if dtype.ends_with('1') => format!("|{dtype}"),
        _ => format!("<{dtype}"),
    };
    let dims: Vec<String> = shape.iter().map(u64::to_string).collect();
    let shape = if dims.len() == 1 {
        format!("({},)", dims[0])
    } else {
        format!("({})", dims.join(", "))
    };
    let mut header = format!("{{'descr': '{descr}', 'fortran_order': False, 'shape': {shape}, }}");
    let v2 = header.len() + 11 > 65_535;
    let pre = if v2 { 12 } else { 10 };
    let total = (pre + header.len() + 1).div_ceil(64) * 64;
    while pre + header.len() + 1 < total {
        header.push(' ');
    }
    header.push('\n');
    let mut out = Vec::with_capacity(total + data.len());
    out.extend_from_slice(b"\x93NUMPY");
    if v2 {
        out.extend_from_slice(&[2, 0]);
        out.extend_from_slice(&(header.len() as u32).to_le_bytes());
    } else {
        out.extend_from_slice(&[1, 0]);
        out.extend_from_slice(&(header.len() as u16).to_le_bytes());
    }
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(data);
    out
}

/// The folders under the socket folder, and this viewer's arrays on disk.
pub struct Folders {
    pub shared: PathBuf,
    arrays: PathBuf,
    shown: VecDeque<PathBuf>,
    next: u64,
}

impl Folders {
    /// Makes (or checks) `shared/` and `arrays/` in the private socket folder `dir`.
    pub fn new(dir: &Path) -> std::io::Result<Folders> {
        let shared = dir.join("shared");
        let arrays = dir.join("arrays");
        specular_platform::ipc::prepare_dir(&shared)?;
        specular_platform::ipc::prepare_dir(&arrays)?;
        Ok(Folders {
            shared,
            arrays,
            shown: VecDeque::new(),
            next: 0,
        })
    }

    /// Reads and removes the file `name` (a bare name, checked by the parser) from the
    /// shared folder: a regular file, not a link, at most [`MAX_BLOB`] bytes.
    pub fn take_shared(&self, name: &str) -> Result<(Vec<u8>, Reservation), Fail> {
        if name.contains(['/', '\\']) || name.starts_with('.') {
            return Err(Fail::new(Code::Refused, "only bare file names"));
        }
        let p = self.shared.join(name);
        let m = std::fs::symlink_metadata(&p).map_err(|_| {
            Fail::new(
                Code::NotFound,
                format!("no file {name:?} in the shared folder"),
            )
        })?;
        if !m.file_type().is_file() {
            return Err(Fail::new(Code::Refused, "not a regular file"));
        }
        if m.len() > MAX_BLOB {
            return Err(Fail::new(Code::BadArguments, "the file is too large"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if m.nlink() > 1 {
                return Err(Fail::new(
                    Code::Refused,
                    "a file with other names (hard links)",
                ));
            }
        }
        let reserved = budget::reserve(m.len()).map_err(no_memory)?;
        let data = std::fs::read(&p).map_err(|e| Fail::new(Code::Internal, e.to_string()))?;
        let _ = std::fs::remove_file(&p);
        Ok((data, reserved))
    }

    /// Writes an array as `<name>.npy` in a folder of its own; returns its path.
    pub fn write(&mut self, name: Option<&str>, npy: &[u8]) -> Result<PathBuf, Fail> {
        let io = |e: std::io::Error| Fail::new(Code::Internal, e.to_string());
        self.next += 1;
        let folder = self
            .arrays
            .join(format!("{}-{}", std::process::id(), self.next));
        std::fs::create_dir(&folder).map_err(io)?;
        let stem = name
            .map(|n| n.strip_suffix(".npy").unwrap_or(n))
            .filter(|n| !n.is_empty())
            .unwrap_or("array");
        let path = folder.join(format!("{stem}.npy"));
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(io)?;
        f.write_all(npy).map_err(io)?;
        self.shown.push_back(folder);
        while self.shown.len() > KEEP {
            if let Some(old) = self.shown.pop_front() {
                let _ = std::fs::remove_dir_all(old);
            }
        }
        Ok(path)
    }
}

impl Drop for Folders {
    fn drop(&mut self) {
        for f in self.shown.drain(..) {
            let _ = std::fs::remove_dir_all(f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npy_headers_parse() {
        for (dtype, shape) in [
            ("<f4", vec![2u64, 3]),
            ("u1", vec![5]),
            ("|b1", vec![1, 1, 1]),
        ] {
            let n: u64 = shape.iter().product();
            let a = Array {
                shape: &shape,
                dtype,
                strides: None,
            };
            let data = vec![1u8; (n * a.itemsize()) as usize];
            let f = npy(dtype, &shape, &data);
            assert_eq!((f.len() - data.len()) % 64, 0);
            let h = specular_arrays::npy::parse(&f).unwrap();
            assert_eq!(h.shape, shape);
        }
    }

    #[test]
    fn gathers_strided_arrays() {
        // A 2×3 u2 array stored in Fortran order: columns first.
        let c: Vec<u16> = vec![1, 2, 3, 4, 5, 6];
        let f: Vec<u16> = vec![1, 4, 2, 5, 3, 6];
        let bytes = |v: &[u16]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
        let a = Array {
            shape: &[2, 3],
            dtype: "<u2",
            strides: Some(&[2, 4]),
        };
        assert_eq!(a.gather(&bytes(&f)).unwrap(), bytes(&c));
        // Reversed rows (negative stride): the first element is the second row's.
        let a = Array {
            shape: &[2, 3],
            dtype: "<u2",
            strides: Some(&[-6, 2]),
        };
        assert_eq!(a.gather(&bytes(&c)).unwrap(), bytes(&[4, 5, 6, 1, 2, 3]));
        // A broadcast (stride 0) and C order pass.
        let a = Array {
            shape: &[2, 3],
            dtype: "<u2",
            strides: Some(&[0, 2]),
        };
        assert_eq!(
            a.gather(&bytes(&[7, 8, 9])).unwrap(),
            bytes(&[7, 8, 9, 7, 8, 9])
        );
        let a = Array {
            shape: &[2, 3],
            dtype: "<u2",
            strides: None,
        };
        assert_eq!(a.gather(&bytes(&c)).unwrap(), bytes(&c));
        // Hostile: strides past the data, too short a buffer, absurd sizes.
        let a = Array {
            shape: &[2, 3],
            dtype: "<u2",
            strides: Some(&[1 << 20, 2]),
        };
        assert!(a.gather(&bytes(&c)).is_err());
        let a = Array {
            shape: &[4, 3],
            dtype: "<u2",
            strides: None,
        };
        assert!(a.gather(&bytes(&c)).is_err());
        let a = Array {
            shape: &[1 << 31, 1 << 31],
            dtype: "<f8",
            strides: None,
        };
        assert_eq!(a.gather(&[]).unwrap_err().code, Code::BadArguments);
    }

    #[test]
    fn blobs_are_bounded() {
        let mut b = Blobs::default();
        let id = b.create(10).unwrap();
        b.write(id, 8, &[1, 2]).unwrap();
        assert!(b.write(id, 9, &[1, 2]).is_err());
        assert!(b.write(id, u64::MAX, &[1]).is_err());
        assert_eq!(b.read(id, 8, 100).unwrap(), &[1, 2]);
        assert!(b.read(id, 11, 1).is_err());
        assert_eq!(b.read(99, 0, 1).unwrap_err().code, Code::NotFound);
        for _ in 1..MAX_BLOBS {
            b.create(1).unwrap();
        }
        assert_eq!(b.create(1).unwrap_err().code, Code::Busy);
        b.free(id).unwrap();
        assert_eq!(b.free(id).unwrap_err().code, Code::NotFound);
        // More than the memory budget allows: refused before anything is allocated.
        let small = std::sync::Arc::new(budget::MemoryBudget::fixed(1 << 20));
        budget::with_budget(small, || {
            let mut b = Blobs::default();
            assert_eq!(b.create(64 << 20).unwrap_err().code, Code::NoMemory);
            assert!(b.create(1 << 10).is_ok());
        });
    }
}
