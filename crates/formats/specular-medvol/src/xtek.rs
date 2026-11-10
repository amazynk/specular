//! Nikon Metrology (X-Tek) reconstructions: an `.xtekct` (or `.xtekVolume`) INI header and
//! the volume as raw little-endian samples in a `.vol` file beside it, x fastest, then y,
//! then z.
//!
//! `VoxelsX/Y/Z` give the size and `VoxelSizeX/Y/Z` the voxel size in `Units` (millimetres
//! when none is stated). The sample type is read from the header when it states one
//! (`DataType` / `VolumeDataType` / `BitsPerVoxel`); CT Pro writes float32, and an 8- or
//! 16-bit export is told from the `.vol` file's size otherwise. The data file is the one the
//! header names, else the header's name with `.vol`, else the only `.vol` file of the right
//! size in the folder.

use std::path::{Path, PathBuf};

use crate::ini::{Ini, start};
use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, OrientationSource, Param,
    ParamValue, Result, Skip,
};

/// Whether `data` is an X-Tek header.
pub fn sniff(data: &[u8]) -> bool {
    let s = start(data, 12);
    if s.starts_with("[xtekct]") || s.starts_with("[xtekvolume]") {
        return true;
    }
    // `.xtekVolume` files may open with another section: look for the voxel counts.
    s.starts_with('[') && {
        let head = String::from_utf8_lossy(&data[..data.len().min(16 << 10)]).to_ascii_lowercase();
        head.contains("\nvoxelsx=") && head.contains("\nvoxelsizex=") && head.contains("[xtek")
    }
}

fn unit_mm(u: &str) -> f64 {
    match u.trim().to_ascii_lowercase().as_str() {
        "um" | "µm" | "micron" | "microns" => 1e-3,
        "cm" => 10.0,
        "m" => 1000.0,
        "in" | "inch" | "inches" => 25.4,
        _ => 1.0,
    }
}

/// A sample type named in a header ("float", "unsigned short", "uint16", "16"…).
pub(crate) fn named_type(v: &str) -> Option<DataType> {
    let v = v.trim().to_ascii_lowercase().replace([' ', '_', '-'], "");
    Some(match v.as_str() {
        "float" | "float32" | "single" | "real" | "32bitfloat" | "float32bit" => DataType::F32,
        "double" | "float64" => DataType::F64,
        "uint16" | "unsignedshort" | "ushort" | "16" | "16bit" | "unsigned16bit" | "u16" => {
            DataType::U16
        }
        "int16" | "short" | "signedshort" | "i16" => DataType::I16,
        "uint8" | "unsignedchar" | "uchar" | "byte" | "8" | "8bit" | "u8" => DataType::U8,
        "int8" | "char" | "signedchar" => DataType::I8,
        "uint32" | "unsignedint" => DataType::U32,
        "int32" | "int" => DataType::I32,
        "32" | "32bit" => DataType::F32,
        _ => return None,
    })
}

/// The data file: named in the header, the header's name with `.vol`, or the folder's only
/// `.vol` file of `bytes` per voxel × `voxels` bytes.
fn data_file(ini: &Ini, path: &Path, voxels: u64) -> Result<(PathBuf, u64)> {
    let dir = path.parent().unwrap_or(Path::new(""));
    let size = |p: &Path| std::fs::metadata(p).ok().map(|m| m.len());
    for (_, k, v) in &ini.0 {
        if (k.contains("file") || k.contains("name")) && v.to_ascii_lowercase().ends_with(".vol") {
            let p = crate::beside(path, v);
            if let Some(n) = size(&p) {
                return Ok((p, n));
            }
            // An absolute path from the scanning machine: the same name here.
            let name = v.rsplit(['/', '\\']).next().unwrap_or(v);
            let p = dir.join(name);
            if let Some(n) = size(&p) {
                return Ok((p, n));
            }
        }
    }
    let p = path.with_extension("vol");
    if let Some(n) = size(&p) {
        return Ok((p, n));
    }
    let fits = |n: u64| [1, 2, 4].iter().any(|b| voxels * b == n);
    let dir_ = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let mut found: Vec<(PathBuf, u64)> = std::fs::read_dir(dir_)
        .map_err(|e| Error::Io(e.to_string()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vol")))
        .filter_map(|p| size(&p).filter(|&n| fits(n)).map(|n| (p, n)))
        .collect();
    if found.len() == 1 {
        return Ok(found.remove(0));
    }
    Err(Error::Io(format!(
        "{}: no such file",
        p.file_name().unwrap_or(p.as_os_str()).to_string_lossy()
    )))
}

pub fn parse(data: &[u8], path: &Path) -> Result<Header> {
    let ini = Ini::new(data);
    let dim = |k: &str| {
        ini.num(None, k)
            .filter(|x| *x >= 1.0 && x.fract() == 0.0)
            .map(|x| x as usize)
            .ok_or_else(|| Error::Malformed(format!("X-Tek header: {k}")))
    };
    let size = [dim("voxelsx")?, dim("voxelsy")?, dim("voxelsz")?];
    let voxels = size
        .iter()
        .try_fold(1u64, |a, &n| a.checked_mul(n as u64))
        .ok_or(Error::TooLarge)?;
    let unit = ini.get(None, "units").map_or(1.0, unit_mm);
    let vs = |k: &str| {
        ini.num(None, k)
            .filter(|x| *x > 0.0)
            .map(|x| x.abs() * unit)
    };
    let sx = vs("voxelsizex").unwrap_or(1.0);
    let spacing = [
        sx,
        vs("voxelsizey").unwrap_or(sx),
        vs("voxelsizez").unwrap_or(sx),
    ];
    let (file, len) = data_file(&ini, path, voxels)?;
    let stated = [
        "datatype",
        "volumedatatype",
        "voxeltype",
        "outputdatatype",
        "bitspervoxel",
    ]
    .iter()
    .find_map(|k| ini.get(None, k).and_then(named_type));
    let dtype = match stated {
        Some(t) => t,
        None => match len.checked_div(voxels) {
            Some(1) => DataType::U8,
            Some(2) => DataType::U16,
            _ => DataType::F32,
        },
    };
    // Any bytes before the samples (a header some exports put first).
    let skip = ini
        .num(None, "headersize")
        .filter(|x| *x >= 0.0)
        .map_or(0, |x| x as u64);
    let layout = Layout::dense(size, &[], 1).ok_or(Error::TooLarge)?;
    let off = |k: &str| ini.num(None, k).unwrap_or(0.0) * unit;
    let affine = [
        [spacing[0], 0.0, 0.0, off("offsetx")],
        [0.0, spacing[1], 0.0, off("offsety")],
        [0.0, 0.0, spacing[2], off("offsetz")],
    ];
    let name = file
        .file_name()
        .map(|n| PathBuf::from(n.to_os_string()))
        .unwrap_or(file.clone());
    let rel = if file.parent() == path.parent() {
        name
    } else {
        file
    };
    Ok(Header {
        kind: Kind::Xtek,
        dtype,
        layout,
        affine,
        orientation: OrientationSource::Object,
        dims: size.to_vec(),
        spacing,
        scale: None,
        cal: None,
        description: ini.get(None, "name").map(str::to_owned),
        intent: None,
        anatomical: None,
        params: params(&ini, unit),
        data: DataRef {
            file: DataFile::Detached(vec![rel]),
            skip: Skip::Start {
                lines: 0,
                bytes: skip as usize,
            },
            encoding: Encoding::Raw,
            big_endian: false,
        },
    })
}

fn params(ini: &Ini, unit: f64) -> Vec<Param> {
    let mut out = vec![Param {
        key: "content",
        value: ParamValue::Text("reconstruction".into()),
    }];
    let mut num = |key: &'static str, v: Option<f64>, unit: &'static str| {
        if let Some(x) = v.filter(|x| *x > 0.0) {
            out.push(Param {
                key,
                value: ParamValue::Number(x, unit),
            });
        }
    };
    num("kv", ini.num(None, "xraykv"), "kV");
    num("current", ini.num(None, "xrayua"), "µA");
    num("projections", ini.num(None, "projections"), "");
    num("angle-step", ini.num(None, "angularstep"), "°");
    num("sod", ini.num(None, "srctoobject").map(|x| x * unit), "mm");
    num(
        "sdd",
        ini.num(None, "srctodetector").map(|x| x * unit),
        "mm",
    );
    num("exposure", ini.num(None, "exposure"), "ms");
    let mut text = |key: &'static str, v: Option<&str>| {
        if let Some(v) = v {
            out.push(Param {
                key,
                value: ParamValue::Text(v.to_owned()),
            });
        }
    };
    text(
        "filter",
        ini.get(None, "filter_material").or(ini.get(None, "filter")),
    );
    text(
        "software",
        ini.get(None, "version")
            .or(ini.get(Some("ctpro"), "version")),
    );
    out
}
