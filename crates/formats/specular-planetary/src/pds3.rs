//! PDS3 products: an ODL label, attached at the start of the data file or detached in a
//! `.lbl` beside it, whose `^` pointers say where each object starts (by record, 1-based, or
//! by byte with `<BYTES>`, in the label's file or a named one). `IMAGE` objects (and
//! `*_IMAGE`: browse images, …) and `QUBE` objects (ISIS2 spectral cubes) become images;
//! an `IMAGE_HEADER` of type VICAR is the embedded VICAR label.

use crate::odl::{self, Block, Value};
use crate::raster::{Interleave, Layout, NoData};
use crate::sample::{Real, Sample};
use crate::{Facts, Result, map_facts, meaningful};

/// Where an object starts: in the label's own file (`None`) or a named one, at a byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pointer {
    pub file: Option<String>,
    pub offset: u64,
}

/// One image of a product.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// The object's name ("IMAGE", "BROWSE_IMAGE", "SPECTRAL_QUBE").
    pub name: String,
    pub pointer: Pointer,
    /// Where the samples are, relative to the start of `pointer.file` (the pointer's offset
    /// is included).
    pub layout: Layout,
    pub unit: Option<String>,
    /// Band wavelengths and their unit, when the label lists one per band.
    pub wavelengths: Vec<f64>,
    pub wavelength_unit: Option<String>,
    /// Band names, when the label lists one per band.
    pub band_names: Vec<String>,
}

/// An object that is an image the reader cannot show, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Refused {
    pub name: String,
    pub reason: String,
}

/// A parsed PDS3 product.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Product {
    pub label: odl::Label,
    pub images: Vec<Image>,
    pub refused: Vec<Refused>,
    /// The embedded VICAR label (`IMAGE_HEADER` with `HEADER_TYPE = VICAR2`).
    pub vicar_header: Option<Pointer>,
    /// Compressed copies of the data (`COMPRESSED_FILE`: file name, encoding).
    pub compressed: Vec<(String, String)>,
    pub facts: Facts,
}

/// Bytes of a file looked at to recognise a label.
pub const SNIFF: usize = 1024;

/// Whether `head` is a PDS3 label (attached or detached): `PDS_VERSION_ID`, `ODL_VERSION_ID`
/// or an SFDU label naming PDS, as the first statement.
pub fn sniff(head: &[u8]) -> bool {
    let h = &head[..head.len().min(SNIFF)];
    let t = h.trim_ascii_start();
    if t.starts_with(b"PDS_VERSION_ID") || t.starts_with(b"ODL_VERSION_ID") {
        return true;
    }
    // "CCSD3ZF0000100000001NJPL3IF0PDS200000001 = SFDU_LABEL", "NJPL1I00PDS1…", alone on
    // its line before PDS_VERSION_ID.
    let line = t.split(|&c| c == b'\n').next().unwrap_or(&[]);
    let has = |n: &[u8]| t.windows(n.len()).any(|w| w == n);
    (t.starts_with(b"CCSD") || t.starts_with(b"NJPL"))
        && line.windows(3).any(|w| w == b"PDS")
        && (line.windows(10).any(|w| w == b"SFDU_LABEL") || has(b"PDS_VERSION_ID"))
}

/// Parses a label (attached or detached) up to its `END`.
pub fn parse(bytes: &[u8]) -> Result<Product> {
    let label = odl::parse(bytes)?;
    let root = &label.root;
    let mut found = Found::default();
    scan(
        root,
        root.f64("RECORD_BYTES").filter(|v| *v > 0.0),
        &mut found,
    );
    let facts = facts(root);
    Ok(Product {
        images: found.images,
        refused: found.refused,
        vicar_header: found.vicar_header,
        compressed: found.compressed,
        facts,
        label,
    })
}

#[derive(Default)]
struct Found {
    images: Vec<Image>,
    refused: Vec<Refused>,
    vicar_header: Option<Pointer>,
    compressed: Vec<(String, String)>,
}

/// The pointers of `block` (the label, or a `FILE` object of a multi-file product) and the
/// objects they describe, then those of its `FILE` objects.
fn scan(block: &Block, record: Option<f64>, found: &mut Found) {
    for e in &block.entries {
        let odl::Entry::Attr(key, value) = e else {
            continue;
        };
        let Some(name) = key.strip_prefix('^') else {
            continue;
        };
        let upper = name.to_ascii_uppercase();
        let image = upper == "IMAGE" || upper.ends_with("_IMAGE");
        let qube = upper == "QUBE" || upper.ends_with("_QUBE");
        let header = upper == "IMAGE_HEADER";
        if !(image || qube || header) {
            continue;
        }
        let Some(obj) = object(block, name) else {
            if image || qube {
                found.refused.push(Refused {
                    name: upper.clone(),
                    reason: format!("the label points to {upper} but does not describe it"),
                });
            }
            continue;
        };
        let pointer = match pointer(value, record) {
            Ok(p) => p,
            Err(why) => {
                found.refused.push(Refused {
                    name: obj.name.clone(),
                    reason: why,
                });
                continue;
            }
        };
        if header {
            let t = obj.str("HEADER_TYPE").unwrap_or("").to_ascii_uppercase();
            if t.starts_with("VICAR") {
                found.vicar_header = Some(pointer);
            }
            continue;
        }
        let r = if qube {
            qube_image(obj, pointer)
        } else {
            plain_image(obj, pointer)
        };
        match r {
            Ok(i) => found.images.push(i),
            Err(reason) => found.refused.push(Refused {
                name: obj.name.clone(),
                reason,
            }),
        }
    }
    for b in block.blocks() {
        let n = b.name.to_ascii_uppercase();
        if n == "COMPRESSED_FILE" {
            if let Some(f) = b.str("FILE_NAME") {
                let enc = b.str("ENCODING_TYPE").unwrap_or("").to_owned();
                found.compressed.push((f.to_owned(), enc));
            }
        } else if n == "FILE" || n.ends_with("_FILE") {
            scan(
                b,
                b.f64("RECORD_BYTES").filter(|v| *v > 0.0).or(record),
                found,
            );
        }
    }
}

/// The object a pointer `^name` describes: the top-level block of that name, else one whose
/// name ends with `_name` (`^QUBE` and `SPECTRAL_QUBE`).
fn object<'a>(root: &'a Block, name: &str) -> Option<&'a Block> {
    let suffix = format!("_{}", name.to_ascii_uppercase());
    root.blocks()
        .find(|b| b.name.eq_ignore_ascii_case(name))
        .or_else(|| {
            root.blocks()
                .find(|b| b.name.to_ascii_uppercase().ends_with(&suffix))
        })
}

/// A pointer's file and byte offset. Records count from 1 and need `RECORD_BYTES`.
pub fn pointer(v: &Value, record: Option<f64>) -> std::result::Result<Pointer, String> {
    let at = |n: &Value| -> std::result::Result<u64, String> {
        let k = n
            .as_f64()
            .filter(|k| *k >= 1.0 && k.fract() == 0.0)
            .ok_or("its pointer is not a record or byte number")?;
        let bytes = n.unit().is_some_and(|u| u.eq_ignore_ascii_case("BYTES"));
        let off = if bytes {
            k - 1.0
        } else {
            (k - 1.0) * record.ok_or("RECORD_BYTES is missing")?
        };
        (off < 2f64.powi(63))
            .then_some(off as u64)
            .ok_or_else(|| "its pointer is past any file".into())
    };
    match v {
        Value::Number { .. } | Value::Based { .. } => Ok(Pointer {
            file: None,
            offset: at(v)?,
        }),
        Value::Text(f) | Value::Symbol(f) => Ok(Pointer {
            file: Some(f.clone()),
            offset: 0,
        }),
        Value::Seq(items) => match items.as_slice() {
            [f] => Ok(Pointer {
                file: f.as_str().map(str::to_owned),
                offset: 0,
            }),
            [f, n, ..] => Ok(Pointer {
                file: f.as_str().map(str::to_owned),
                offset: at(n)?,
            }),
            [] => Err("its pointer is empty".into()),
        },
        Value::Set(_) => Err("its pointer is a set".into()),
    }
}

/// The sample type of a `SAMPLE_TYPE` with `bits` bits.
pub fn sample_type(t: &str, bits: u32) -> std::result::Result<Sample, String> {
    let t = t.trim().to_ascii_uppercase().replace(' ', "_");
    if !matches!(bits, 8 | 16 | 32 | 64 | 128) {
        return Err(format!("{bits}-bit samples are not supported"));
    }
    let bytes = (bits / 8) as u8;
    let int = |signed: bool, big_endian: bool| -> std::result::Result<Sample, String> {
        if bytes > 8 {
            return Err(format!("{bits}-bit integers are not supported"));
        }
        Ok(Sample::Int {
            bytes,
            signed,
            big_endian,
        })
    };
    let real = |real: Real| -> std::result::Result<Sample, String> {
        match bytes {
            4 | 8 => Ok(Sample::Float { bytes, real }),
            _ => Err(format!("{bits}-bit reals are not supported")),
        }
    };
    let complex = |real: Real| -> std::result::Result<Sample, String> {
        match bytes {
            8 | 16 => Ok(Sample::Complex {
                bytes: bytes / 2,
                real,
            }),
            _ => Err(format!("{bits}-bit complex samples are not supported")),
        }
    };
    match t.as_str() {
        "UNSIGNED_INTEGER"
        | "MSB_UNSIGNED_INTEGER"
        | "SUN_UNSIGNED_INTEGER"
        | "MAC_UNSIGNED_INTEGER"
        | "UNSIGNED_BYTE"
        | "N/A"
        | "CHARACTER"
        | "BYTE" => int(false, true),
        "LSB_UNSIGNED_INTEGER" | "PC_UNSIGNED_INTEGER" | "VAX_UNSIGNED_INTEGER" => {
            int(false, false)
        }
        "INTEGER" | "MSB_INTEGER" | "SUN_INTEGER" | "MAC_INTEGER" | "SIGNED_INTEGER" => {
            int(true, true)
        }
        "LSB_INTEGER" | "PC_INTEGER" | "VAX_INTEGER" => int(true, false),
        "IEEE_REAL" | "REAL" | "FLOAT" | "SUN_REAL" | "MAC_REAL" | "MSB_REAL" | "IEEE_FLOAT" => {
            real(Real::IeeeBig)
        }
        "PC_REAL" | "LSB_REAL" | "LSB_IEEE_REAL" => real(Real::IeeeLittle),
        "VAX_REAL" | "VAX_DOUBLE" | "VAXD_REAL" => real(Real::Vax),
        "VAXG_REAL" => real(Real::VaxG),
        "IEEE_COMPLEX" | "COMPLEX" | "SUN_COMPLEX" | "MAC_COMPLEX" | "MSB_COMPLEX" => {
            complex(Real::IeeeBig)
        }
        "PC_COMPLEX" | "LSB_COMPLEX" => complex(Real::IeeeLittle),
        "VAX_COMPLEX" => complex(Real::Vax),
        "VAXG_COMPLEX" => complex(Real::VaxG),
        _ if t.contains("BIT_STRING") => Err("bit-string samples are not supported".into()),
        other => Err(format!("sample type {other} is not supported")),
    }
}

fn int(b: &Block, key: &str) -> Option<u64> {
    b.f64(key)
        .filter(|v| *v >= 0.0 && v.fract() == 0.0 && *v < 2f64.powi(62))
        .map(|v| v as u64)
}

fn dim(v: u64, what: &str) -> std::result::Result<u32, String> {
    if v == 0 {
        return Err(format!("it has no {what}"));
    }
    u32::try_from(v).map_err(|_| format!("too many {what}"))
}

/// No-data constants of an object: missing, invalid and null values (one or one per band);
/// based integers are bit patterns of real samples.
fn nodata(b: &Block, keys: &[&str], sample: Sample) -> NoData {
    let mut n = NoData::default();
    for k in keys {
        if let Some(v) = b.get(k) {
            for i in v.items() {
                let x = match i {
                    Value::Based { bits, .. } => Some(sample.from_bits(*bits)),
                    // Quoted: "16#FF7FFFFB#".
                    Value::Text(t) => match odl::based(t.trim()) {
                        Some((bits, _)) => Some(sample.from_bits(bits)),
                        None => i.as_f64(),
                    },
                    i => i.as_f64(),
                };
                if let Some(x) = x {
                    n.push(x);
                }
            }
        }
    }
    n
}

/// A per-band list of numbers from `key` in `b` or its `BAND_BIN` group.
fn band_list(b: &Block, keys: &[&str], bands: u32) -> (Vec<f64>, Option<String>) {
    let groups = std::iter::once(b).chain(b.find("BAND_BIN"));
    for g in groups {
        for k in keys {
            if let Some(v) = g.get(k) {
                let items = v.items();
                let nums: Vec<f64> = items.iter().filter_map(Value::as_f64).collect();
                if nums.len() == bands as usize && bands > 1 {
                    let unit = items
                        .iter()
                        .find_map(|i| i.unit().map(str::to_owned))
                        .or_else(|| g.str("BAND_BIN_UNIT").map(str::to_owned))
                        .or_else(|| b.str("BAND_BIN_UNIT").map(str::to_owned));
                    return (nums, unit);
                }
            }
        }
    }
    (Vec::new(), None)
}

/// An `IMAGE` object.
fn plain_image(b: &Block, pointer: Pointer) -> std::result::Result<Image, String> {
    if let Some(enc) = b
        .str("ENCODING_TYPE")
        .and_then(meaningful)
        .filter(|e| !e.eq_ignore_ascii_case("NONE"))
    {
        return Err(format!("it is compressed ({enc}), which is not supported"));
    }
    let width = dim(int(b, "LINE_SAMPLES").unwrap_or(0), "samples")?;
    let height = dim(int(b, "LINES").unwrap_or(0), "lines")?;
    let bands = dim(int(b, "BANDS").unwrap_or(1), "bands")?;
    let bits = int(b, "SAMPLE_BITS").unwrap_or(8) as u32;
    let sample = sample_type(b.str("SAMPLE_TYPE").unwrap_or("UNSIGNED_INTEGER"), bits)?;
    let storage = b
        .str("BAND_STORAGE_TYPE")
        .unwrap_or("BAND_SEQUENTIAL")
        .to_ascii_uppercase();
    let interleave = match storage.as_str() {
        "BAND_SEQUENTIAL" | "N/A" => Interleave::Bsq,
        "LINE_INTERLEAVED" | "BAND_INTERLEAVED_BY_LINE" => Interleave::Bil,
        "SAMPLE_INTERLEAVED" | "BAND_INTERLEAVED_BY_PIXEL" | "PIXEL_INTERLEAVED" => Interleave::Bip,
        s => return Err(format!("band storage {s} is not supported")),
    };
    let prefix = int(b, "LINE_PREFIX_BYTES").unwrap_or(0);
    let suffix = int(b, "LINE_SUFFIX_BYTES").unwrap_or(0);
    let mut layout = Layout::new(
        width,
        height,
        bands,
        sample,
        interleave,
        pointer.offset,
        prefix,
        suffix,
    );
    layout.scale = b.f64("SCALING_FACTOR").filter(|v| *v != 0.0).unwrap_or(1.0);
    layout.add = b.f64("OFFSET").unwrap_or(0.0);
    layout.nodata = nodata(
        b,
        &[
            "MISSING_CONSTANT",
            "MISSING",
            "INVALID_CONSTANT",
            "INVALID",
            "NULL",
            "CORE_NULL",
        ],
        sample,
    );
    layout.flip_x = b
        .str("SAMPLE_DISPLAY_DIRECTION")
        .is_some_and(|d| d.eq_ignore_ascii_case("LEFT"));
    layout.flip_y = b
        .str("LINE_DISPLAY_DIRECTION")
        .is_some_and(|d| d.eq_ignore_ascii_case("UP"));
    let (wavelengths, wavelength_unit) = band_list(
        b,
        &["CENTER_FILTER_WAVELENGTH", "BAND_BIN_CENTER", "CENTER"],
        bands,
    );
    let band_names = b
        .get("BAND_NAME")
        .map(|v| {
            v.items()
                .iter()
                .filter_map(|i| i.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .filter(|n| n.len() == bands as usize && bands > 1)
        .unwrap_or_default();
    Ok(Image {
        name: b.name.clone(),
        pointer,
        layout,
        unit: b.str("UNIT").and_then(meaningful),
        wavelengths,
        wavelength_unit,
        band_names,
    })
}

/// A `QUBE` (ISIS2 cube): three axes named SAMPLE, LINE and BAND in storage order (first
/// fastest), core items with suffix (side, bottom and back plane) items after each axis's
/// core, `SUFFIX_BYTES` each.
fn qube_image(b: &Block, pointer: Pointer) -> std::result::Result<Image, String> {
    let axes = int(b, "AXES").unwrap_or(3);
    let names: Vec<String> = b
        .get("AXIS_NAME")
        .map(|v| {
            v.items()
                .iter()
                .filter_map(|i| i.as_str().map(|s| s.to_ascii_uppercase()))
                .collect()
        })
        .unwrap_or_default();
    let core: Vec<u64> = b
        .get("CORE_ITEMS")
        .map(|v| {
            v.items()
                .iter()
                .filter_map(Value::as_f64)
                .map(|x| x as u64)
                .collect()
        })
        .unwrap_or_default();
    if axes != 3 || names.len() != 3 || core.len() != 3 {
        return Err("only three-axis qubes are supported".into());
    }
    let suffix: Vec<u64> = b
        .get("SUFFIX_ITEMS")
        .map(|v| {
            v.items()
                .iter()
                .filter_map(Value::as_f64)
                .map(|x| x as u64)
                .collect()
        })
        .unwrap_or_else(|| vec![0, 0, 0]);
    let suffix = if suffix.len() == 3 {
        suffix
    } else {
        vec![0, 0, 0]
    };
    let item_bytes = int(b, "CORE_ITEM_BYTES");
    let item_type = b.str("CORE_ITEM_TYPE");
    let (Some(bytes), Some(t)) = (item_bytes, item_type) else {
        return Err(
            "its sample type is described in a separate structure file, which is not read".into(),
        );
    };
    let sample = sample_type(t, (bytes * 8) as u32)?;
    let sb = int(b, "SUFFIX_BYTES").unwrap_or(4);
    // Strides: item, then (core + suffix items) of the faster axes.
    let cb = sample.bytes() as u64;
    let s1 = core[0]
        .saturating_mul(cb)
        .saturating_add(suffix[0].saturating_mul(sb));
    let s2 = core[1].saturating_mul(s1).saturating_add(
        suffix[1]
            .saturating_mul(core[0].saturating_add(suffix[0]))
            .saturating_mul(sb),
    );
    let strides = [cb, s1, s2];
    let find = |n: &str| names.iter().position(|a| a == n);
    let (Some(xs), Some(ys), Some(bs)) = (find("SAMPLE"), find("LINE"), find("BAND")) else {
        return Err("its axes are not SAMPLE, LINE and BAND".into());
    };
    let interleave = match (xs, ys, bs) {
        (0, 1, 2) => Interleave::Bsq,
        (0, 2, 1) => Interleave::Bil,
        (1, 2, 0) => Interleave::Bip,
        _ => {
            return Err(format!(
                "axis order ({}) is not supported",
                names.join(", ")
            ));
        }
    };
    let width = dim(core[xs], "samples")?;
    let height = dim(core[ys], "lines")?;
    let bands = dim(core[bs], "bands")?;
    let mut layout = Layout::new(
        width,
        height,
        bands,
        sample,
        interleave,
        pointer.offset,
        0,
        0,
    );
    layout.sample_stride = strides[xs];
    layout.line_stride = strides[ys];
    layout.band_stride = strides[bs];
    layout.scale = b
        .get("CORE_MULTIPLIER")
        .and_then(Value::as_f64)
        .filter(|v| *v != 0.0)
        .unwrap_or(1.0);
    layout.add = b.f64("CORE_BASE").unwrap_or(0.0);
    layout.nodata = nodata(
        b,
        &["CORE_NULL", "MISSING_CONSTANT", "INVALID_CONSTANT"],
        sample,
    );
    if let Some(min) = b.f64("CORE_VALID_MINIMUM") {
        layout.nodata.below = Some(min);
    }
    let (wavelengths, wavelength_unit) = band_list(b, &["BAND_BIN_CENTER", "CENTER"], bands);
    Ok(Image {
        name: b.name.clone(),
        pointer,
        layout,
        unit: b.str("CORE_UNIT").and_then(meaningful),
        wavelengths,
        wavelength_unit,
        band_names: Vec::new(),
    })
}

/// The observation facts of a label.
fn facts(root: &Block) -> Facts {
    let text = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| root.deep(k).and_then(display_text))
    };
    let map = root
        .find("IMAGE_MAP_PROJECTION")
        .and_then(|m| map_facts(|k| m.get(k).cloned()));
    Facts {
        mission: text(&["MISSION_NAME"]),
        spacecraft: text(&[
            "INSTRUMENT_HOST_NAME",
            "SPACECRAFT_NAME",
            "INSTRUMENT_HOST_ID",
        ]),
        instrument: text(&["INSTRUMENT_NAME", "INSTRUMENT_ID"]),
        target: text(&["TARGET_NAME"]),
        start: text(&["START_TIME", "IMAGE_TIME", "IMAGE_MID_TIME"]),
        stop: text(&["STOP_TIME"]),
        product_id: text(&["PRODUCT_ID"]),
        title: text(&["PRODUCT_NAME", "OBSERVATION_NAME"]),
        map,
    }
}

/// A value as label text, `None` when it says nothing; lists are joined.
pub(crate) fn display_text(v: &Value) -> Option<String> {
    match v {
        Value::Seq(items) | Value::Set(items) => {
            let parts: Vec<String> = items.iter().filter_map(display_text).collect();
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        v => meaningful(&v.display()),
    }
}

/// A product file for tests and fixtures: an attached label with one IMAGE of `values`
/// stored as `sample_type` / `bits`, with `extra` statements inside the IMAGE object.
pub fn write_attached(
    (samples, lines, bands): (usize, usize, usize),
    sample_type: &str,
    bits: u32,
    storage: &str,
    extra: &str,
    values: &[f64],
) -> Vec<u8> {
    let s = self::sample_type(sample_type, bits).unwrap_or(Sample::U8);
    let record = 512usize;
    let text = |label_records: usize| {
        format!(
            "PDS_VERSION_ID = PDS3\r\nRECORD_TYPE = FIXED_LENGTH\r\nRECORD_BYTES = {record}\r\n\
             LABEL_RECORDS = {label_records}\r\n^IMAGE = {}\r\nTARGET_NAME = MARS\r\n\
             OBJECT = IMAGE\r\n  LINES = {lines}\r\n  LINE_SAMPLES = {samples}\r\n  BANDS = {bands}\r\n\
             SAMPLE_TYPE = {sample_type}\r\n  SAMPLE_BITS = {bits}\r\n  BAND_STORAGE_TYPE = {storage}\r\n\
             {extra}\r\nEND_OBJECT = IMAGE\r\nEND\r\n",
            label_records + 1
        )
    };
    let n = text(1).len().div_ceil(record) + 1;
    let mut out = text(n).into_bytes();
    out.resize(n * record, b' ');
    let at = |b: usize, y: usize, x: usize| values[(b * lines + y) * samples + x];
    let mut push = |v: f64| out.extend(crate::vicar::encode(s, v));
    match storage {
        "LINE_INTERLEAVED" => {
            for y in 0..lines {
                for b in 0..bands {
                    for x in 0..samples {
                        push(at(b, y, x));
                    }
                }
            }
        }
        "SAMPLE_INTERLEAVED" => {
            for y in 0..lines {
                for x in 0..samples {
                    for b in 0..bands {
                        push(at(b, y, x));
                    }
                }
            }
        }
        _ => {
            for b in 0..bands {
                for y in 0..lines {
                    for x in 0..samples {
                        push(at(b, y, x));
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointers() {
        let rec = Some(100.0);
        let p = |t: &str| {
            let l = odl::parse(format!("A = {t}\nEND\n").as_bytes()).unwrap();
            pointer(l.root.get("A").unwrap(), rec)
        };
        assert_eq!(
            p("3").unwrap(),
            Pointer {
                file: None,
                offset: 200
            }
        );
        assert_eq!(p("3 <BYTES>").unwrap().offset, 2);
        assert_eq!(
            p("(\"X.IMG\", 2)").unwrap(),
            Pointer {
                file: Some("X.IMG".into()),
                offset: 100
            }
        );
        assert_eq!(p("\"X.IMG\"").unwrap().file.as_deref(), Some("X.IMG"));
        assert!(p("0").is_err());
        let l = odl::parse(b"A = 3\nEND\n").unwrap();
        assert!(pointer(l.root.get("A").unwrap(), None).is_err());
    }

    #[test]
    fn sample_types() {
        assert_eq!(
            sample_type("MSB_UNSIGNED_INTEGER", 16).unwrap(),
            Sample::Int {
                bytes: 2,
                signed: false,
                big_endian: true
            }
        );
        assert_eq!(
            sample_type("PC_REAL", 32).unwrap(),
            Sample::Float {
                bytes: 4,
                real: Real::IeeeLittle
            }
        );
        assert_eq!(
            sample_type("VAX_REAL", 64).unwrap(),
            Sample::Float {
                bytes: 8,
                real: Real::Vax
            }
        );
        assert_eq!(sample_type("IEEE_COMPLEX", 64).unwrap().bytes(), 8);
        assert!(
            sample_type("MSB_INTEGER", 12)
                .unwrap_err()
                .contains("12-bit")
        );
        assert!(sample_type("MSB_BIT_STRING", 8).is_err());
    }

    #[test]
    fn attached_products() {
        let values: Vec<f64> = (0..3 * 4 * 2).map(f64::from).collect();
        for storage in ["BAND_SEQUENTIAL", "LINE_INTERLEAVED", "SAMPLE_INTERLEAVED"] {
            for (t, bits) in [
                ("UNSIGNED_INTEGER", 8),
                ("LSB_INTEGER", 16),
                ("MSB_UNSIGNED_INTEGER", 32),
                ("PC_REAL", 32),
                ("VAX_REAL", 32),
                ("IEEE_REAL", 64),
                ("VAX_REAL", 64),
                ("VAXG_REAL", 64),
            ] {
                let f = write_attached(
                    (4, 3, 2),
                    t,
                    bits,
                    storage,
                    "SCALING_FACTOR = 2\r\nOFFSET = 1\r\nMISSING_CONSTANT = 5",
                    &values,
                );
                assert!(sniff(&f));
                let p = parse(&f).unwrap();
                assert_eq!(p.images.len(), 1, "{t} {bits} {storage}");
                let img = &p.images[0];
                let (v, trunc) = img.layout.band(&f, 1);
                assert!(!trunc);
                assert_eq!(v[0], 25.0, "{t} {bits} {storage}");
                assert_eq!(v[11], 47.0);
                let (v, _) = img.layout.band(&f, 0);
                assert!(v[5].is_nan());
                assert_eq!(p.facts.target.as_deref(), Some("MARS"));
            }
        }
    }

    #[test]
    fn qubes_with_suffixes() {
        // SAMPLE, LINE, BAND = 3, 2, 2 with one sample suffix and one band suffix of 4 bytes.
        let label = "PDS_VERSION_ID = PDS3\nRECORD_BYTES = 1\n^QUBE = 1 <BYTES>\n\
            OBJECT = SPECTRAL_QUBE\n AXES = 3\n AXIS_NAME = (SAMPLE, LINE, BAND)\n\
            CORE_ITEMS = (3, 2, 2)\n CORE_ITEM_BYTES = 2\n CORE_ITEM_TYPE = SUN_INTEGER\n\
            SUFFIX_ITEMS = (1, 0, 1)\n SUFFIX_BYTES = 4\n CORE_VALID_MINIMUM = -32752\n\
            BAND_BIN = 1\nEND_OBJECT = SPECTRAL_QUBE\nEND\n";
        let p = parse(label.as_bytes()).unwrap();
        let img = &p.images[0];
        let l = &img.layout;
        assert_eq!((l.sample_stride, l.line_stride, l.band_stride), (2, 10, 20));
        let mut d = vec![0u8; 20 * 2 + 4 * 4 * 2];
        for b in 0..2u64 {
            for y in 0..2u64 {
                for x in 0..3u64 {
                    let v = if (b, y, x) == (1, 1, 1) {
                        -32768i16
                    } else {
                        (100 * b + 10 * y + x) as i16
                    };
                    let o = (b * 20 + y * 10 + x * 2) as usize;
                    d[o..o + 2].copy_from_slice(&v.to_be_bytes());
                }
            }
        }
        let (v, _) = l.band(&d, 1);
        assert_eq!(v[..3], [100.0, 101.0, 102.0]);
        assert!(v[4].is_nan());
        let no_type = label.replace(" CORE_ITEM_TYPE = SUN_INTEGER\n", "");
        let p = parse(no_type.as_bytes()).unwrap();
        assert!(p.images.is_empty() && p.refused[0].reason.contains("structure file"));
    }

    #[test]
    fn sniffs() {
        assert!(sniff(b"\r\nPDS_VERSION_ID = PDS3"));
        assert!(sniff(b"ODL_VERSION_ID = ODL3"));
        assert!(sniff(
            b"CCSD3ZF0000100000001NJPL3IF0PDS200000001 = SFDU_LABEL\r\n"
        ));
        assert!(!sniff(b"LBLSIZE=100"));
        assert!(!sniff(
            b"CCSD3ZF0000100000001NJPL3IF0XXX200000001 = SFDU_LABEL"
        ));
    }
}
