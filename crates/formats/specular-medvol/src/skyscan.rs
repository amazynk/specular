//! Bruker SkyScan reconstructions: a folder of slices written by NRecon (`*_rec00000123.bmp`,
//! `.png` or `.tif`) and the reconstruction's `.log`, opened together as one volume from the
//! log.
//!
//! The log is INI-style (`[System]`, `[Acquisition]`, `[Reconstruction]`). Slices are the
//! files named by `Filename Prefix` (else the log's own name) and a section number,
//! limited to `First Section` … `Last Section`, in number order. `Pixel Size (um)` gives the
//! voxel size (`Section to Section Step` sections apart); 8- and 16-bit slices map back to
//! attenuation through `Minimum` / `Maximum for CS to Image Conversion`. Slices show as
//! NRecon shows them (row 0 at the top), numbered upward.

use std::path::{Path, PathBuf};

use crate::ini::{Ini, start};
use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, MedVolume,
    OrientationSource, Param, ParamValue, Result, Skip,
};

const SLICE_EXT: [&str; 4] = ["bmp", "png", "tif", "tiff"];

/// Whether `data` is a SkyScan reconstruction log (a `[System]` section and a
/// `[Reconstruction]` one).
pub fn sniff(data: &[u8]) -> bool {
    let head = &data[..data.len().min(64 << 10)];
    start(head, 8) == "[system]" && {
        let t = String::from_utf8_lossy(head).to_ascii_lowercase();
        t.contains("\n[reconstruction]") && t.contains("pixel size (um)")
    }
}

/// The slice files a log at `path` describes, in order, and the section numbers.
pub(crate) fn slices(ini: &Ini, path: &Path) -> Result<Vec<(u64, PathBuf)>> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut prefixes = Vec::new();
    if let Some(p) = ini.get(Some("reconstruction"), "filename prefix") {
        prefixes.push(p.to_ascii_lowercase());
    }
    prefixes.push(stem.to_ascii_lowercase());
    let wanted_ext = ini
        .get(Some("reconstruction"), "result file type")
        .map(|t| t.to_ascii_lowercase());
    let first = ini.num(Some("reconstruction"), "first section");
    let last = ini.num(Some("reconstruction"), "last section");
    let entries = std::fs::read_dir(dir).map_err(|e| Error::Io(e.to_string()))?;
    let names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    for prefix in &prefixes {
        let mut found: Vec<(u64, PathBuf)> = names
            .iter()
            .filter_map(|n| {
                let lower = n.to_ascii_lowercase();
                let (base, ext) = lower.rsplit_once('.')?;
                if !SLICE_EXT.contains(&ext) {
                    return None;
                }
                if let Some(w) = &wanted_ext
                    && !(w.starts_with(ext) || ext.starts_with(w.as_str()))
                {
                    return None;
                }
                let digits = base.strip_prefix(prefix.as_str())?;
                if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                let n_: u64 = digits.parse().ok()?;
                Some((n_, dir.join(n)))
            })
            .filter(|(n, _)| {
                first.is_none_or(|f| *n as f64 >= f) && last.is_none_or(|l| *n as f64 <= l)
            })
            .collect();
        if !found.is_empty() {
            found.sort();
            found.dedup_by_key(|x| x.0);
            return Ok(found);
        }
    }
    Err(Error::Malformed(format!(
        "SkyScan: no slices named {}… next to the log",
        prefixes.first().map_or("", |s| s.as_str())
    )))
}

pub fn parse(data: &[u8], path: &Path) -> Result<Header> {
    let ini = Ini::new(data);
    let files = slices(&ini, path)?;
    let paths: Vec<PathBuf> = files.iter().map(|f| f.1.clone()).collect();
    let first = crate::slices::first(&paths)?;
    let (w, h, n) = (first.width, first.height, paths.len());
    let um = ini
        .num(Some("reconstruction"), "pixel size (um)")
        .or_else(|| ini.num(Some("acquisition"), "image pixel size (um)"))
        .filter(|x| *x > 0.0)
        .unwrap_or(1000.0);
    let px = um / 1000.0;
    let step = ini
        .num(Some("reconstruction"), "section to section step")
        .filter(|x| *x >= 1.0)
        .unwrap_or(1.0);
    // Sections between consecutive files, when files are missing from the run.
    let pz = px * step;
    let scale = match first.dtype {
        DataType::U8 | DataType::U16 => {
            let lo = ini.num(Some("reconstruction"), "minimum for cs to image conversion");
            let hi = ini.num(Some("reconstruction"), "maximum for cs to image conversion");
            match (lo, hi) {
                (Some(lo), Some(hi)) if hi > lo => {
                    let top = if first.dtype == DataType::U8 {
                        255.0
                    } else {
                        65535.0
                    };
                    Some(((hi - lo) / top, lo))
                }
                _ => None,
            }
        }
        _ => None,
    };
    let layout = Layout::dense([w, h, n], &[], 1).ok_or(Error::TooLarge)?;
    let affine = [
        [px, 0.0, 0.0, 0.0],
        [0.0, px, 0.0, 0.0],
        [0.0, 0.0, pz, 0.0],
    ];
    Ok(Header {
        kind: Kind::SkyScan,
        dtype: first.dtype,
        layout,
        affine,
        orientation: OrientationSource::Object,
        dims: vec![w, h, n],
        spacing: [px, px, pz],
        scale,
        cal: None,
        description: ini
            .get(Some("reconstruction"), "dataset prefix")
            .map(str::to_owned),
        intent: None,
        anatomical: None,
        params: params(&ini),
        data: DataRef {
            file: DataFile::Detached(paths),
            skip: Skip::Start { lines: 0, bytes: 0 },
            encoding: Encoding::Container,
            big_endian: false,
        },
    })
}

fn params(ini: &Ini) -> Vec<Param> {
    let mut out = vec![Param {
        key: "content",
        value: ParamValue::Text("reconstruction".into()),
    }];
    let a = Some("acquisition");
    let r = Some("reconstruction");
    let mut text = |key: &'static str, v: Option<&str>| {
        if let Some(v) = v {
            out.push(Param {
                key,
                value: ParamValue::Text(v.to_owned()),
            });
        }
    };
    text("instrument", ini.get(Some("system"), "scanner"));
    let program = match (
        ini.get(r, "reconstruction program"),
        ini.get(r, "program version"),
    ) {
        (Some(p), Some(v)) => Some(format!("{p} {}", v.trim_start_matches("Version:").trim())),
        (p, v) => p.or(v).map(str::to_owned),
    };
    text("software", program.as_deref());
    text("filter", ini.get(a, "filter"));
    text(
        "date",
        ini.get(r, "time and date")
            .or(ini.get(a, "study date and time")),
    );
    let mut num = |key: &'static str, v: Option<f64>, unit: &'static str| {
        if let Some(x) = v.filter(|x| *x > 0.0) {
            out.push(Param {
                key,
                value: ParamValue::Number(x, unit),
            });
        }
    };
    num("kv", ini.num(a, "source voltage (kv)"), "kV");
    num("current", ini.num(a, "source current (ua)"), "µA");
    num("exposure", ini.num(a, "exposure (ms)"), "ms");
    num("projections", ini.num(a, "number of files"), "");
    num(
        "angle-step",
        ini.num(a, "rotation step (deg)")
            .or(ini.num(r, "angular step (deg)")),
        "°",
    );
    num("sod", ini.num(a, "object to source (mm)"), "mm");
    num("sdd", ini.num(a, "camera to source (mm)"), "mm");
    num(
        "beam-hardening",
        ini.num(r, "beam hardening correction (%)"),
        "%",
    );
    num("ring", ini.num(r, "ring artifact correction"), "");
    num("smoothing", ini.num(r, "smoothing"), "");
    out
}

pub fn load(header: Header) -> Result<MedVolume> {
    let DataFile::Detached(paths) = &header.data.file else {
        return Err(Error::Malformed("SkyScan: no slices".into()));
    };
    let [w, h, _] = header.layout.size;
    let (bytes, res) = crate::slices::stack(paths, w, h, header.dtype)?;
    Ok(MedVolume {
        header,
        bytes: crate::data::held(bytes, res),
        offset: 0,
        big_endian: false,
    })
}
