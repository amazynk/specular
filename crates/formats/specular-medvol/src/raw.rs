//! Headerless volumes (`.raw`, `.vol`): samples with nothing saying how they are laid out.
//!
//! The layout comes from the person opening the file ([`remember`], from the viewer's raw
//! volume dialog), offered from [`guesses`] (the size spelled in the file's name, then
//! cubes and square slices that fill the file exactly). A `.raw` / `.vol` file that is the
//! data of a header beside it (`.xtekct`, `.xtekVolume`, `.vgi`, `.mhd`, `.nhdr` of the same
//! name, or naming it) opens through that header instead ([`sibling_header`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, OrientationSource, Result,
    Skip,
};

/// How a headerless file's samples are laid out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawLayout {
    /// Columns, rows, slices (x fastest).
    pub size: [usize; 3],
    pub dtype: DataType,
    pub big_endian: bool,
    /// Bytes before the first sample.
    pub offset: u64,
    /// Millimetres between voxel centres along x, y, z.
    pub spacing: [f64; 3],
}

impl RawLayout {
    /// Bytes the samples take.
    pub fn bytes(&self) -> Option<u64> {
        self.size
            .iter()
            .try_fold(self.dtype.size() as u64, |a, &n| a.checked_mul(n as u64))
    }

    /// Whether the samples fit in a file of `len` bytes.
    pub fn fits(&self, len: u64) -> bool {
        self.size.iter().all(|&n| n > 0)
            && self
                .bytes()
                .and_then(|b| b.checked_add(self.offset))
                .is_some_and(|end| end <= len)
    }
}

fn known() -> &'static Mutex<HashMap<PathBuf, RawLayout>> {
    static K: OnceLock<Mutex<HashMap<PathBuf, RawLayout>>> = OnceLock::new();
    K.get_or_init(Default::default)
}

/// Uses `layout` for the file at `path` from now on (this process).
pub fn remember(path: &Path, layout: RawLayout) {
    if let Ok(mut k) = known().lock() {
        k.insert(path.to_path_buf(), layout);
    }
}

/// The layout given for `path`, if any.
pub fn layout_of(path: &Path) -> Option<RawLayout> {
    known().lock().ok()?.get(path).copied()
}

/// Names headerless volumes go by.
pub fn is_raw_name(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("raw") || e.eq_ignore_ascii_case("vol"))
}

const HEADER_EXT: [&str; 6] = ["xtekct", "xtekvolume", "vgi", "mhd", "nhdr", "hdr"];

/// The header whose data `path` is: one of the same name, or one beside it that names it.
pub fn sibling_header(path: &Path) -> Option<PathBuf> {
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path.file_name()?.to_string_lossy().into_owned();
    let stem = path.file_stem()?.to_string_lossy().to_ascii_lowercase();
    let mut named = None;
    for e in std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()) {
        let p = e.path();
        let Some(ext) = p
            .extension()
            .map(|x| x.to_string_lossy().to_ascii_lowercase())
        else {
            continue;
        };
        if !HEADER_EXT.contains(&ext.as_str()) {
            continue;
        }
        let same = p
            .file_stem()
            .is_some_and(|s| s.to_string_lossy().to_ascii_lowercase() == stem);
        // An Analyze `.hdr` pairs only with `.img`.
        if same && ext != "hdr" {
            return Some(p);
        }
        if named.is_none()
            && ext != "hdr"
            && e.metadata().is_ok_and(|m| m.len() < (256 << 10))
            && std::fs::read(&p).is_ok_and(|b| {
                String::from_utf8_lossy(&b)
                    .lines()
                    .any(|l| l.contains(name.as_str()))
            })
        {
            named = Some(p);
        }
    }
    named
}

/// Whether the file at `path` needs a layout before it can open: a `.raw` / `.vol` with
/// none given, no header beside it, and no format of its own.
pub fn needs_layout(path: &Path) -> bool {
    if !is_raw_name(path) || layout_of(path).is_some() || sibling_header(path).is_some() {
        return false;
    }
    let mut head = vec![0u8; 4096];
    let n = std::fs::File::open(path)
        .and_then(|mut f| std::io::Read::read(&mut f, &mut head))
        .unwrap_or(0);
    crate::sniff(&head[..n]).is_none()
}

/// The header for a headerless file read with `l`.
pub fn header(l: &RawLayout) -> Result<Header> {
    let layout = Layout::dense(l.size, &[], 1).ok_or(Error::TooLarge)?;
    let s = l
        .spacing
        .map(|x| if x.is_finite() && x > 0.0 { x } else { 1.0 });
    Ok(Header {
        kind: Kind::Raw,
        dtype: l.dtype,
        layout,
        affine: [
            [s[0], 0.0, 0.0, 0.0],
            [0.0, s[1], 0.0, 0.0],
            [0.0, 0.0, s[2], 0.0],
        ],
        orientation: OrientationSource::Object,
        dims: l.size.to_vec(),
        spacing: s,
        scale: None,
        cal: None,
        description: None,
        intent: None,
        anatomical: None,
        params: Vec::new(),
        data: DataRef {
            file: DataFile::Attached,
            skip: Skip::Start {
                lines: 0,
                bytes: usize::try_from(l.offset).map_err(|_| Error::TooLarge)?,
            },
            encoding: Encoding::Raw,
            big_endian: l.big_endian,
        },
    })
}

/// A type named in a file name ("uint16", "16bit", "float"…).
fn type_in_name(name: &str) -> Option<DataType> {
    let n = name.to_ascii_lowercase();
    let has = |w: &str| n.contains(w);
    if has("float64") || has("double") || has("f64") {
        Some(DataType::F64)
    } else if has("float") || has("f32") || has("32bit") || has("real") {
        Some(DataType::F32)
    } else if has("uint16") || has("u16") || has("16bit") || has("ushort") {
        Some(DataType::U16)
    } else if has("int16") || has("i16") || has("short") {
        Some(DataType::I16)
    } else if has("uint8") || has("u8") || has("8bit") || has("byte") || has("uchar") {
        Some(DataType::U8)
    } else {
        None
    }
}

/// Sizes spelled in a file name: `512x512x300` (or with `×`), the longest run of at least
/// two numbers.
fn size_in_name(name: &str) -> Option<[usize; 3]> {
    let name = name.replace('×', "x").to_ascii_lowercase();
    let mut best: Option<Vec<usize>> = None;
    let b = name.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() || (i > 0 && b[i - 1].is_ascii_digit()) {
            i += 1;
            continue;
        }
        let mut run = Vec::new();
        let mut j = i;
        loop {
            let s = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            match name[s..j].parse::<usize>() {
                Ok(v) if v > 0 => run.push(v),
                _ => break,
            }
            if j < b.len() && b[j] == b'x' && j + 1 < b.len() && b[j + 1].is_ascii_digit() {
                j += 1;
            } else {
                break;
            }
        }
        if run.len() >= 2 && run.len() <= 3 && best.as_ref().is_none_or(|x| run.len() > x.len()) {
            best = Some(run);
        }
        i = j.max(i + 1);
    }
    let r = best?;
    Some([r[0], r[1], r.get(2).copied().unwrap_or(1)])
}

/// Likely layouts for a headerless file of `len` bytes called `name`, the likeliest first:
/// the size (and type) its name spells, then cubes, then square slices, each of 16-bit,
/// 32-bit float and 8-bit samples, little-endian, filling the file exactly.
pub fn guesses(name: &str, len: u64) -> Vec<RawLayout> {
    let mut out: Vec<RawLayout> = Vec::new();
    let mut push = |l: RawLayout| {
        if l.fits(len) && !out.iter().any(|o| o.size == l.size && o.dtype == l.dtype) {
            out.push(l);
        }
    };
    let make = |size: [usize; 3], dtype: DataType, offset: u64| RawLayout {
        size,
        dtype,
        big_endian: false,
        offset,
        spacing: [1.0; 3],
    };
    let named_type = type_in_name(name);
    let types: Vec<DataType> = named_type
        .into_iter()
        .chain([DataType::U16, DataType::F32, DataType::U8])
        .collect();
    if let Some(size) = size_in_name(name) {
        for &t in &types {
            let voxels: u64 = size.iter().map(|&x| x as u64).product();
            let need = voxels * t.size() as u64;
            if need <= len {
                // Whatever is left over is a header before the samples.
                push(make(size, t, len - need));
                break;
            }
        }
    }
    for &t in &types {
        let s = t.size() as u64;
        if !len.is_multiple_of(s) {
            continue;
        }
        let n = len / s;
        let c = (n as f64).cbrt().round() as u64;
        for c in [c.saturating_sub(1), c, c + 1] {
            if c > 0 && c * c * c == n {
                push(make([c as usize; 3], t, 0));
            }
        }
    }
    // Square slices: the side whose stack depth is nearest the side, and the widest side
    // (a thin stack of large slices).
    for &t in &types {
        let s = t.size() as u64;
        if !len.is_multiple_of(s) {
            continue;
        }
        let n = len / s;
        let mut best: Option<(u64, u64)> = None;
        let mut widest: Option<(u64, u64)> = None;
        let mut w = 16;
        while w * w <= n && w <= 1 << 15 {
            if n.is_multiple_of(w * w) {
                let d = n / (w * w);
                if best.is_none_or(|(bw, bd)| d.abs_diff(w) < bd.abs_diff(bw)) {
                    best = Some((w, d));
                }
                widest = Some((w, d));
            }
            w += 1;
        }
        for (w, d) in best.into_iter().chain(widest) {
            push(make([w as usize, w as usize, d as usize], t, 0));
        }
    }
    out.truncate(6);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_spell_sizes_and_types() {
        assert_eq!(
            size_in_name("part_512x256x100_uint16.raw"),
            Some([512, 256, 100])
        );
        assert_eq!(size_in_name("slice 640×480.raw"), Some([640, 480, 1]));
        assert_eq!(size_in_name("scan2024.raw"), None);
        assert_eq!(type_in_name("a_16bit.raw"), Some(DataType::U16));
        assert_eq!(type_in_name("a_float.vol"), Some(DataType::F32));
    }

    #[test]
    fn guesses_fill_the_file() {
        // 64³ uint16.
        let g = guesses("x.raw", 64 * 64 * 64 * 2);
        assert_eq!(g[0].size, [64, 64, 64]);
        assert_eq!(g[0].dtype, DataType::U16);
        // The name wins, its header skipped.
        let g = guesses("v_100x50x20_8bit.raw", 100 * 50 * 20 + 512);
        assert_eq!(
            (g[0].size, g[0].dtype, g[0].offset),
            ([100, 50, 20], DataType::U8, 512)
        );
        // Square slices: 300 × 300 × 7 bytes.
        let g = guesses("y.vol", 300 * 300 * 7);
        assert!(
            g.iter()
                .any(|l| l.size == [300, 300, 7] && l.dtype == DataType::U8)
        );
        assert!(g.iter().all(|l| l.fits(300 * 300 * 7)));
    }
}
