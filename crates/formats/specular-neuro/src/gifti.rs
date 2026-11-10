//! GIFTI (`.gii`): an XML document of data arrays, from the GIFTI 1.0 specification.
//!
//! A surface (`.surf.gii`) is a `NIFTI_INTENT_POINTSET` array of vertices (n × 3 float32,
//! with the transforms from its data space) and a `NIFTI_INTENT_TRIANGLE` array of faces
//! (m × 3 int32). Functional, shape and label files (`.func.gii`, `.shape.gii`,
//! `.label.gii`) hold one or more arrays of n per-vertex values for a surface stored
//! elsewhere; label files carry a `LabelTable` of keys, names and colours.
//!
//! Arrays are coded as ASCII, base64 (`Base64Binary`), zlib then base64
//! (`GZipBase64Binary`) or as a range of an external file (`ExternalFileBinary`), in either
//! byte order and either index order (`ColumnMajorOrder` arrays are transposed here, so
//! every array reads row-major).

use std::path::Path;

use crate::xml::{self, Element};
use crate::{Error, Result};

/// Most values accepted in one array (512 MB of float32).
pub const MAX_VALUES: usize = 1 << 27;

/// How an array's values are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Ascii,
    Base64,
    GzipBase64,
    External,
}

impl Encoding {
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Ascii => "ASCII",
            Encoding::Base64 => "Base64Binary",
            Encoding::GzipBase64 => "GZipBase64Binary",
            Encoding::External => "ExternalFileBinary",
        }
    }
}

/// The values of one array.
#[derive(Clone, Debug, PartialEq)]
pub enum Values {
    F32(Vec<f32>),
    I32(Vec<i32>),
    U8(Vec<u8>),
}

impl Values {
    pub fn len(&self) -> usize {
        match self {
            Values::F32(v) => v.len(),
            Values::I32(v) => v.len(),
            Values::U8(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Value `i` as a double.
    pub fn get(&self, i: usize) -> f64 {
        match self {
            Values::F32(v) => f64::from(v[i]),
            Values::I32(v) => f64::from(v[i]),
            Values::U8(v) => f64::from(v[i]),
        }
    }

    /// The NIfTI type name ("NIFTI_TYPE_FLOAT32"…), as the file states it.
    pub fn type_name(&self) -> &'static str {
        match self {
            Values::F32(_) => "NIFTI_TYPE_FLOAT32",
            Values::I32(_) => "NIFTI_TYPE_INT32",
            Values::U8(_) => "NIFTI_TYPE_UINT8",
        }
    }

    /// Every value as float32 (keys and indices exactly up to 2²⁴).
    pub fn to_f32(&self) -> Vec<f32> {
        match self {
            Values::F32(v) => v.clone(),
            Values::I32(v) => v.iter().map(|&x| x as f32).collect(),
            Values::U8(v) => v.iter().map(|&x| f32::from(x)).collect(),
        }
    }

    /// Every value as int32 (floats rounded).
    pub fn to_i32(&self) -> Vec<i32> {
        match self {
            Values::F32(v) => v.iter().map(|&x| x.round() as i32).collect(),
            Values::I32(v) => v.clone(),
            Values::U8(v) => v.iter().map(|&x| i32::from(x)).collect(),
        }
    }
}

/// A transform from the array's data space to another space.
#[derive(Clone, Debug, PartialEq)]
pub struct CoordSystem {
    /// "NIFTI_XFORM_TALAIRACH", "NIFTI_XFORM_SCANNER_ANAT", …
    pub data_space: String,
    pub transformed_space: String,
    /// Row-major 4 × 4.
    pub matrix: [[f64; 4]; 4],
}

/// One `<Label>` of a label table.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub key: i32,
    pub name: String,
    /// Red, green, blue and alpha in 0..=1.
    pub rgba: [f32; 4],
}

/// One data array.
#[derive(Clone, Debug, PartialEq)]
pub struct DataArray {
    /// The intent as written ("NIFTI_INTENT_POINTSET"…).
    pub intent: String,
    pub dims: Vec<usize>,
    pub encoding: Encoding,
    pub big_endian: bool,
    pub column_major: bool,
    pub meta: Vec<(String, String)>,
    pub coord: Vec<CoordSystem>,
    /// Row-major values.
    pub values: Values,
}

impl DataArray {
    pub fn is(&self, intent: &str) -> bool {
        self.intent.eq_ignore_ascii_case(intent)
    }

    /// The array's `Name` metadata, when it has one.
    pub fn name(&self) -> Option<&str> {
        meta(&self.meta, "Name")
    }

    /// Values per row (the second dimension; 1 for a plain vector).
    pub fn columns(&self) -> usize {
        self.dims.get(1).copied().unwrap_or(1).max(1)
    }
}

/// The value of metadata entry `name`, when it is set.
pub fn meta<'a>(m: &'a [(String, String)], name: &str) -> Option<&'a str> {
    m.iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
        .filter(|v| !v.is_empty())
}

/// What a GIFTI file is, for choosing how to show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Vertices and faces.
    Surface,
    /// Per-vertex label keys with a label table.
    Labels,
    /// Per-vertex values (functional, shape, time series).
    Data,
}

/// A GIFTI document.
#[derive(Clone, Debug, PartialEq)]
pub struct Gifti {
    pub version: String,
    pub meta: Vec<(String, String)>,
    pub labels: Vec<Label>,
    pub arrays: Vec<DataArray>,
}

pub const POINTSET: &str = "NIFTI_INTENT_POINTSET";
pub const TRIANGLE: &str = "NIFTI_INTENT_TRIANGLE";
pub const LABEL: &str = "NIFTI_INTENT_LABEL";

impl Gifti {
    /// The vertex array, n × 3.
    pub fn pointset(&self) -> Option<&DataArray> {
        self.arrays
            .iter()
            .find(|a| a.is(POINTSET) && a.columns() == 3 && a.values.len() % 3 == 0)
    }

    /// The face array, m × 3.
    pub fn triangles(&self) -> Option<&DataArray> {
        self.arrays
            .iter()
            .find(|a| a.is(TRIANGLE) && a.columns() == 3 && a.values.len() % 3 == 0)
    }

    /// Vertices of the surface, or values per array of a data file.
    pub fn vertex_count(&self) -> Option<usize> {
        if let Some(p) = self.pointset() {
            return Some(p.values.len() / 3);
        }
        self.data_arrays().next().map(|a| a.dims[0])
    }

    /// The arrays that hold per-vertex values (all but vertices, faces and node indices).
    pub fn data_arrays(&self) -> impl Iterator<Item = &DataArray> {
        self.arrays
            .iter()
            .filter(|a| !a.is(POINTSET) && !a.is(TRIANGLE) && !a.is("NIFTI_INTENT_NODE_INDEX"))
    }

    pub fn role(&self) -> Role {
        if self.pointset().is_some() && self.triangles().is_some() {
            Role::Surface
        } else if self.data_arrays().any(|a| a.is(LABEL))
            || (!self.labels.is_empty()
                && self
                    .data_arrays()
                    .all(|a| !matches!(a.values, Values::F32(_))))
        {
            Role::Labels
        } else {
            Role::Data
        }
    }

    /// The file's `AnatomicalStructurePrimary` (e.g. "CortexLeft"), from the file or its
    /// first array.
    pub fn structure(&self) -> Option<&str> {
        meta(&self.meta, "AnatomicalStructurePrimary").or_else(|| {
            self.arrays
                .iter()
                .find_map(|a| meta(&a.meta, "AnatomicalStructurePrimary"))
        })
    }

    /// The label with key `key`.
    pub fn label(&self, key: i32) -> Option<&Label> {
        self.labels.iter().find(|l| l.key == key)
    }
}

/// Whether `data` (a file's first bytes) looks like GIFTI: XML whose root is `<GIFTI`.
pub fn sniff(data: &[u8]) -> bool {
    let head = &data[..data.len().min(4096)];
    let s = String::from_utf8_lossy(head);
    let mut rest: &str = &s;
    // Skip the declaration, comments and the DOCTYPE to the first element.
    loop {
        rest = rest.trim_start_matches('\u{feff}').trim_start();
        if let Some(r) = rest.strip_prefix("<?") {
            match r.find("?>") {
                Some(e) => rest = &r[e + 2..],
                None => return false,
            }
        } else if let Some(r) = rest.strip_prefix("<!") {
            match r.find('>') {
                Some(e) => rest = &r[e + 1..],
                None => return false,
            }
        } else {
            break;
        }
    }
    rest.starts_with("<GIFTI") && rest[6..].starts_with(|c: char| c.is_whitespace() || c == '>')
}

fn md(el: Option<&Element>, src: &[u8]) -> Vec<(String, String)> {
    let Some(el) = el else {
        return Vec::new();
    };
    el.all("MD")
        .filter_map(|m| {
            Some((
                m.child("Name")?.text(src),
                m.child("Value").map(|v| v.text(src)).unwrap_or_default(),
            ))
        })
        .collect()
}

fn numbers(text: &[u8]) -> impl Iterator<Item = &[u8]> {
    text.split(|c| c.is_ascii_whitespace() || *c == b',')
        .filter(|t| !t.is_empty())
}

fn parse_f64(t: &[u8]) -> Option<f64> {
    std::str::from_utf8(t).ok()?.parse().ok()
}

/// A label table: `<Label Key= Red= Green= Blue= Alpha=>name</Label>` (GIFTI 1.0's
/// `Index` for older files).
pub fn label_table(el: &Element, src: &[u8]) -> Vec<Label> {
    el.all("Label")
        .filter_map(|l| {
            let key = l
                .attr("Key")
                .or_else(|| l.attr("Index"))?
                .trim()
                .parse()
                .ok()?;
            let c = |n: &str, d: f32| {
                l.attr(n)
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .map_or(d, |v| v.clamp(0.0, 1.0))
            };
            Some(Label {
                key,
                name: l.text(src),
                rgba: [
                    c("Red", 0.0),
                    c("Green", 0.0),
                    c("Blue", 0.0),
                    c("Alpha", 1.0),
                ],
            })
        })
        .collect()
}

fn transform(el: &Element, src: &[u8]) -> Option<CoordSystem> {
    let text = el.child("MatrixData")?.bytes(src);
    let v: Vec<f64> = numbers(&text).filter_map(parse_f64).collect();
    if v.len() != 16 {
        return None;
    }
    let mut matrix = [[0.0; 4]; 4];
    for (r, row) in matrix.iter_mut().enumerate() {
        row.copy_from_slice(&v[r * 4..r * 4 + 4]);
    }
    let data_space = el.child_text("DataSpace", src).unwrap_or_default();
    let transformed_space = el.child_text("TransformedSpace", src).unwrap_or_default();
    // A transform between unnamed spaces says nothing (Connectome Workbench writes an
    // empty identity before the real one; nibabel keeps the named one too).
    if data_space.trim().is_empty() && transformed_space.trim().is_empty() {
        return None;
    }
    Some(CoordSystem {
        data_space,
        transformed_space,
        matrix,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ty {
    F32,
    I32,
    U8,
}

impl Ty {
    fn size(self) -> usize {
        match self {
            Ty::U8 => 1,
            _ => 4,
        }
    }
}

fn values_from_bytes(b: &[u8], ty: Ty, big: bool) -> Values {
    let w4 = |c: &[u8]| {
        let a = [c[0], c[1], c[2], c[3]];
        if big {
            u32::from_be_bytes(a)
        } else {
            u32::from_le_bytes(a)
        }
    };
    match ty {
        Ty::U8 => Values::U8(b.to_vec()),
        Ty::I32 => Values::I32(b.chunks_exact(4).map(|c| w4(c) as i32).collect()),
        Ty::F32 => Values::F32(b.chunks_exact(4).map(|c| f32::from_bits(w4(c))).collect()),
    }
}

fn values_from_text(text: &[u8], ty: Ty, n: usize) -> Result<Values> {
    let bad = || Error::Malformed("GIFTI ASCII data".into());
    let it = numbers(text);
    let v = match ty {
        Ty::F32 => Values::F32(
            it.take(n + 1)
                .map(|t| parse_f64(t).map(|x| x as f32).ok_or_else(bad))
                .collect::<Result<_>>()?,
        ),
        Ty::I32 => Values::I32(
            it.take(n + 1)
                .map(|t| {
                    let x = parse_f64(t).ok_or_else(bad)?;
                    (x.fract() == 0.0 && x >= f64::from(i32::MIN) && x <= f64::from(i32::MAX))
                        .then_some(x as i32)
                        .ok_or_else(bad)
                })
                .collect::<Result<_>>()?,
        ),
        Ty::U8 => Values::U8(
            it.take(n + 1)
                .map(|t| {
                    std::str::from_utf8(t)
                        .ok()
                        .and_then(|s| s.parse::<u8>().ok())
                        .ok_or_else(bad)
                })
                .collect::<Result<_>>()?,
        ),
    };
    Ok(v)
}

/// Transposes an `n × m` column-major array to row-major.
fn to_row_major<T: Copy>(v: &[T], n: usize, m: usize) -> Vec<T> {
    let mut out = Vec::with_capacity(v.len());
    for i in 0..n {
        for j in 0..m {
            out.push(v[j * n + i]);
        }
    }
    out
}

fn data_array(el: &Element, src: &[u8], path: Option<&Path>, idx: usize) -> Result<DataArray> {
    let what = |m: &str| Error::Malformed(format!("GIFTI data array {idx}: {m}"));
    let ty = match el.attr("DataType").map(str::trim) {
        Some("NIFTI_TYPE_FLOAT32") => Ty::F32,
        Some("NIFTI_TYPE_INT32") => Ty::I32,
        Some("NIFTI_TYPE_UINT8") => Ty::U8,
        Some(t) => {
            return Err(Error::Unsupported(format!(
                "GIFTI arrays of type {t} are not supported"
            )));
        }
        None => return Err(what("no DataType")),
    };
    let nd: usize = el
        .attr("Dimensionality")
        .and_then(|d| d.trim().parse().ok())
        .filter(|d| (1..=6).contains(d))
        .ok_or_else(|| what("bad Dimensionality"))?;
    let dims: Vec<usize> = (0..nd)
        .map(|i| {
            el.attr(&format!("Dim{i}"))
                .and_then(|d| d.trim().parse().ok())
                .ok_or_else(|| what("bad dimension"))
        })
        .collect::<Result<_>>()?;
    let n = dims
        .iter()
        .try_fold(1usize, |a, &d| a.checked_mul(d))
        .filter(|&n| n <= MAX_VALUES)
        .ok_or(Error::TooLarge)?;
    let encoding = match el.attr("Encoding").map(str::trim) {
        // The GIFTI_ENCODING_* spellings are older (nibabel reads them too).
        Some("ASCII" | "GIFTI_ENCODING_ASCII") => Encoding::Ascii,
        Some("Base64Binary" | "GIFTI_ENCODING_B64BIN") => Encoding::Base64,
        Some("GZipBase64Binary" | "GIFTI_ENCODING_B64GZ") => Encoding::GzipBase64,
        Some("ExternalFileBinary" | "GIFTI_ENCODING_EXTBIN") => Encoding::External,
        Some(e) => return Err(what(&format!("unknown Encoding {e}"))),
        None => return Err(what("no Encoding")),
    };
    let big_endian = el.attr("Endian").map(str::trim) == Some("BigEndian");
    let column_major = el.attr("ArrayIndexingOrder").map(str::trim) == Some("ColumnMajorOrder");
    let bytes_needed = n * ty.size();
    let data = el.child("Data");
    let text = data.map(|d| d.bytes(src)).unwrap_or_default();
    let mut values = match encoding {
        Encoding::Ascii => {
            // Each value needs a digit and a separator: a bound before reading.
            if n > text.len() / 2 + 1 {
                return Err(Error::Truncated(format!("GIFTI data array {idx}")));
            }
            values_from_text(&text, ty, n)?
        }
        Encoding::Base64 | Encoding::GzipBase64 => {
            let raw = crate::base64(&text).ok_or_else(|| what("bad base64"))?;
            let raw = if encoding == Encoding::GzipBase64 {
                let mut out = specular_png::inflate_zlib(&raw, bytes_needed)
                    .or_else(|_| specular_png::inflate_gzip(&raw, bytes_needed))
                    .map_err(|e| what(&format!("compressed data: {e}")))?;
                out.truncate(bytes_needed);
                out
            } else {
                raw
            };
            if raw.len() < bytes_needed {
                return Err(Error::Truncated(format!("GIFTI data array {idx}")));
            }
            values_from_bytes(&raw[..bytes_needed], ty, big_endian)
        }
        Encoding::External => {
            let name = el
                .attr("ExternalFileName")
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| what("no ExternalFileName"))?;
            let offset: u64 = el
                .attr("ExternalFileOffset")
                .and_then(|o| o.trim().parse().ok())
                .unwrap_or(0);
            let base = path.and_then(Path::parent).ok_or_else(|| {
                Error::Unsupported("the GIFTI data is in another file, which was not given".into())
            })?;
            let raw = read_range(&base.join(name.trim()), offset, bytes_needed)?;
            values_from_bytes(&raw, ty, big_endian)
        }
    };
    if values.len() < n {
        return Err(Error::Truncated(format!("GIFTI data array {idx}")));
    }
    if values.len() > n {
        return Err(what("more values than its dimensions"));
    }
    if column_major && dims.len() == 2 && dims[1] > 1 {
        let (r, c) = (dims[0], dims[1]);
        values = match &values {
            Values::F32(v) => Values::F32(to_row_major(v, r, c)),
            Values::I32(v) => Values::I32(to_row_major(v, r, c)),
            Values::U8(v) => Values::U8(to_row_major(v, r, c)),
        };
    }
    Ok(DataArray {
        intent: el
            .attr("Intent")
            .unwrap_or("NIFTI_INTENT_NONE")
            .trim()
            .to_owned(),
        dims,
        encoding,
        big_endian,
        column_major,
        meta: md(el.child("MetaData"), src),
        coord: el
            .all("CoordinateSystemTransformMatrix")
            .filter_map(|c| transform(c, src))
            .collect(),
        values,
    })
}

fn read_range(path: &Path, offset: u64, n: usize) -> Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let io = |e: std::io::Error| Error::Io(format!("{}: {e}", path.display()));
    let mut f = std::fs::File::open(path).map_err(io)?;
    let len = f.metadata().map_err(io)?.len();
    if offset.saturating_add(n as u64) > len {
        return Err(Error::Truncated("GIFTI external data".into()));
    }
    f.seek(SeekFrom::Start(offset)).map_err(io)?;
    let mut out = vec![0; n];
    f.read_exact(&mut out).map_err(io)?;
    Ok(out)
}

/// Reads a GIFTI document (`path`, when known, finds external data files).
pub fn parse(data: &[u8], path: Option<&Path>) -> Result<Gifti> {
    if !sniff(data) {
        return Err(Error::NotRecognised);
    }
    let root = xml::parse(data)?;
    if root.name != "GIFTI" {
        return Err(Error::NotRecognised);
    }
    let arrays = root
        .all("DataArray")
        .enumerate()
        .map(|(i, a)| data_array(a, data, path, i))
        .collect::<Result<Vec<_>>>()?;
    Ok(Gifti {
        version: root.attr("Version").unwrap_or("").trim().to_owned(),
        meta: md(root.child("MetaData"), data),
        labels: root
            .child("LabelTable")
            .map(|t| label_table(t, data))
            .unwrap_or_default(),
        arrays,
    })
}

/// Reads a GIFTI file.
pub fn open(path: &Path) -> Result<Gifti> {
    let data = std::fs::read(path).map_err(|e| Error::Io(e.to_string()))?;
    parse(&data, Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE GIFTI SYSTEM "http://www.nitrc.org/frs/download.php/115/gifti.dtd">
<GIFTI Version="1.0" NumberOfDataArrays="2">
<MetaData><MD><Name><![CDATA[AnatomicalStructurePrimary]]></Name><Value>CortexLeft</Value></MD></MetaData>
<DataArray Intent="NIFTI_INTENT_POINTSET" DataType="NIFTI_TYPE_FLOAT32" ArrayIndexingOrder="RowMajorOrder" Dimensionality="2" Dim0="3" Dim1="3" Encoding="ASCII" Endian="LittleEndian" ExternalFileName="" ExternalFileOffset="">
<CoordinateSystemTransformMatrix><DataSpace>NIFTI_XFORM_TALAIRACH</DataSpace><TransformedSpace>NIFTI_XFORM_TALAIRACH</TransformedSpace>
<MatrixData>1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1</MatrixData></CoordinateSystemTransformMatrix>
<Data>0 0 0
1 0 0
0 1 0</Data></DataArray>
<DataArray Intent="NIFTI_INTENT_TRIANGLE" DataType="NIFTI_TYPE_INT32" ArrayIndexingOrder="ColumnMajorOrder" Dimensionality="2" Dim0="1" Dim1="3" Encoding="Base64Binary" Endian="BigEndian"><Data>AAAAAAAAAAEAAAAC</Data></DataArray>
</GIFTI>"#;

    #[test]
    fn reads_a_small_surface() {
        let g = parse(SURF.as_bytes(), None).unwrap();
        assert_eq!(g.role(), Role::Surface);
        assert_eq!(g.vertex_count(), Some(3));
        assert_eq!(g.structure(), Some("CortexLeft"));
        assert_eq!(g.triangles().unwrap().values, Values::I32(vec![0, 1, 2]));
        assert_eq!(
            g.pointset().unwrap().coord[0].data_space,
            "NIFTI_XFORM_TALAIRACH"
        );
    }

    #[test]
    fn refuses_short_or_bad_arrays() {
        let short = SURF.replace("Dim0=\"3\" Dim1=\"3\"", "Dim0=\"4\" Dim1=\"3\"");
        assert!(matches!(
            parse(short.as_bytes(), None),
            Err(Error::Truncated(_))
        ));
        let huge = SURF.replace("Dim0=\"3\" Dim1=\"3\"", "Dim0=\"999999999\" Dim1=\"3\"");
        assert!(parse(huge.as_bytes(), None).is_err());
        let bad = SURF.replace("AAAAAAAAAAEAAAAC", "A*AA");
        assert!(parse(bad.as_bytes(), None).is_err());
        let ty = SURF.replace("NIFTI_TYPE_INT32", "NIFTI_TYPE_FLOAT64");
        assert!(matches!(
            parse(ty.as_bytes(), None),
            Err(Error::Unsupported(_))
        ));
        assert!(!sniff(b"<?xml version=\"1.0\"?><svg>"));
    }
}
