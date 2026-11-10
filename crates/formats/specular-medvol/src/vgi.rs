//! Volume Graphics `.vgi` headers (VGSTUDIO's raw export): `{volume1}` with
//! `[representation]` and `[file1]` (`Size`, `Name`, `Datatype`, `BitsPerElement`,
//! `SkipHeader`, `Endian`), and the voxel size in a `{volumeprimitive…}`'s `[geometry]`
//! (`resolution`, `unit`). The samples are in the raw file `Name` beside the header, x
//! fastest.

use std::path::{Path, PathBuf};

use crate::ini::{Ini, start};
use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, OrientationSource, Result,
    Skip,
};

/// Whether `data` is a VGI header.
pub fn sniff(data: &[u8]) -> bool {
    start(data, 7) == "{volume"
}

fn unit_mm(u: &str) -> f64 {
    match u.trim().to_ascii_lowercase().as_str() {
        "um" | "µm" | "micron" | "microns" | "micrometer" => 1e-3,
        "nm" => 1e-6,
        "cm" => 10.0,
        "m" => 1000.0,
        "in" | "inch" => 25.4,
        _ => 1.0,
    }
}

fn dtype(kind: &str, bits: Option<f64>) -> Result<DataType> {
    let k = kind.to_ascii_lowercase();
    let bits = bits.unwrap_or(0.0) as u32;
    let float = k.contains("float") || k.contains("real");
    let signed = !float && (k.contains("signed") && !k.contains("unsigned"));
    Ok(match (float, signed, bits) {
        (true, _, 64) => DataType::F64,
        (true, _, _) => DataType::F32,
        (false, false, 8) => DataType::U8,
        (false, true, 8) => DataType::I8,
        (false, false, 16) => DataType::U16,
        (false, true, 16) => DataType::I16,
        (false, false, 32) => DataType::U32,
        (false, true, 32) => DataType::I32,
        (false, false, 64) => DataType::U64,
        (false, true, 64) => DataType::I64,
        _ => {
            return Err(Error::Unsupported(format!(
                "VGI data type {kind:?}, {bits} bits"
            )));
        }
    })
}

pub fn parse(data: &[u8], path: &Path) -> Result<Header> {
    let ini = Ini::new(data);
    let f = Some("file1");
    let r = Some("representation");
    let size = ini
        .nums(f, "size")
        .or_else(|| ini.nums(r, "size"))
        .filter(|v| (1..=3).contains(&v.len()) && v.iter().all(|x| *x >= 1.0 && x.fract() == 0.0))
        .ok_or_else(|| Error::Malformed("VGI: no size".into()))?;
    let mut s = [1usize; 3];
    for (a, x) in size.iter().enumerate() {
        s[a] = *x as usize;
    }
    let kind = ini
        .get(f, "datatype")
        .or(ini.get(r, "datatype"))
        .unwrap_or("unsigned integer");
    let bits = ini
        .num(f, "bitsperelement")
        .or(ini.num(r, "bitsperelement"));
    let dtype = dtype(kind, bits)?;
    let name = ini
        .get(f, "name")
        .ok_or_else(|| Error::Malformed("VGI: no data file name".into()))?;
    let format = ini
        .get(f, "fileformat")
        .unwrap_or("raw")
        .to_ascii_lowercase();
    if format != "raw" {
        return Err(Error::Unsupported(format!("VGI file format {format}")));
    }
    let skip = ini
        .num(f, "skipheader")
        .filter(|x| *x >= 0.0)
        .unwrap_or(0.0) as usize;
    let endian = ini
        .get(f, "endian")
        .or(ini.get(f, "byteorder"))
        .unwrap_or("")
        .to_ascii_lowercase();
    let big_endian = endian.contains("big") || endian.contains("motorola");
    let unit = ini.get(Some("geometry"), "unit").map_or(1.0, unit_mm);
    let res = ini
        .nums(Some("geometry"), "resolution")
        .filter(|v| v.len() == 3 && v.iter().all(|x| *x > 0.0))
        .map_or([1.0; 3], |v| [v[0] * unit, v[1] * unit, v[2] * unit]);
    let layout = Layout::dense(s, &[], 1).ok_or(Error::TooLarge)?;
    let affine = [
        [res[0], 0.0, 0.0, 0.0],
        [0.0, res[1], 0.0, 0.0],
        [0.0, 0.0, res[2], 0.0],
    ];
    // A path from the exporting machine: the same name beside the header.
    let name = name.trim_matches('"');
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let file = if crate::beside(path, name).exists() || base == name {
        PathBuf::from(name)
    } else {
        PathBuf::from(base)
    };
    Ok(Header {
        kind: Kind::Vgi,
        dtype,
        layout,
        affine,
        orientation: OrientationSource::Object,
        dims: s.to_vec(),
        spacing: res,
        scale: None,
        cal: None,
        description: ini.get(Some("description"), "text").map(str::to_owned),
        intent: None,
        anatomical: None,
        params: Vec::new(),
        data: DataRef {
            file: DataFile::Detached(vec![file]),
            skip: Skip::Start {
                lines: 0,
                bytes: skip,
            },
            encoding: Encoding::Raw,
            big_endian,
        },
    })
}
