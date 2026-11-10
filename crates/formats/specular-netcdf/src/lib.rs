//! Read-only NetCDF: the classic formats (CDF-1, CDF-2 with 64-bit offsets, CDF-5 with
//! 64-bit data) through an own parser, and NetCDF-4 (an HDF5 file following the NetCDF-4
//! conventions) mapped from `specular-hdf5`: dimension scales become dimensions, the other
//! numeric datasets variables, and the bookkeeping attributes are hidden.
//!
//! Values come out as real numbers with the CF packing attributes applied the way the
//! reference library's Python bindings apply them: values equal to `_FillValue` (or the
//! type's default fill), `missing_value` or outside `valid_min` / `valid_max` /
//! `valid_range` become NaN, the rest `value * scale_factor + add_offset`.

mod classic;

pub use specular_hdf5::{Attr, Num, PlaneSpec, Value};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a NetCDF file")]
    NotNetcdf,
    #[error("malformed NetCDF file: {0}")]
    Malformed(String),
    #[error("unsupported NetCDF feature: {0}")]
    Unsupported(String),
    #[error("NetCDF structure too large")]
    TooLarge,
    #[error("cancelled")]
    Cancelled,
    #[error(transparent)]
    Hdf5(#[from] specular_hdf5::Error),
    /// The memory budget cannot hold the values.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Caps against hostile headers.
pub(crate) mod limits {
    /// Dimensions, attributes per list, variables.
    pub const MAX_ITEMS: u64 = 1 << 16;
    /// Elements of one attribute.
    pub const MAX_ATTR_ELEMS: u64 = 1 << 20;
    /// Characters of a name.
    pub const MAX_NAME: u64 = 1 << 16;
    /// Values of one plane.
    pub const MAX_PLANE: u64 = 1 << 30;
    /// Elements of a whole-variable read.
    pub const MAX_ALL: u64 = 1 << 28;
}

/// Whether the head of a file is a classic NetCDF file.
pub fn sniff_classic(head: &[u8]) -> bool {
    head.len() >= 4 && &head[..3] == b"CDF" && matches!(head[3], 1 | 2 | 5)
}

/// Whether an opened HDF5 file follows the NetCDF-4 conventions.
pub fn is_netcdf4(file: &specular_hdf5::File) -> bool {
    let root = file.get("/");
    root.is_some_and(|r| r.attr("_NCProperties").is_some() || r.attr("_nc3_strict").is_some())
        || file.datasets().any(|(_, o, _)| {
            o.attr("_Netcdf4Dimid").is_some()
                || o.attr("_Netcdf4Coordinates").is_some()
                || o.attr("NAME")
                    .and_then(Value::as_str)
                    .is_some_and(|n| n.starts_with(PURE_DIM))
        })
}

/// What NetCDF-4 writes as the `NAME` of a dimension scale that is not a variable.
const PURE_DIM: &str = "This is a netCDF dimension but not a netCDF variable";

/// Attributes that are NetCDF-4 or HDF5 dimension-scale bookkeeping, not user metadata.
pub fn is_hidden_attr(name: &str) -> bool {
    matches!(
        name,
        "_Netcdf4Dimid"
            | "_Netcdf4Coordinates"
            | "_NCProperties"
            | "_nc3_strict"
            | "_IsNetcdf4"
            | "DIMENSION_LIST"
            | "REFERENCE_LIST"
            | "CLASS"
            | "NAME"
            | "_SuperblockVersion"
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// CDF-1.
    Classic,
    /// CDF-2.
    Offset64,
    /// CDF-5.
    Data64,
    /// HDF5-based.
    Netcdf4,
}

impl Format {
    pub fn describe(self) -> &'static str {
        match self {
            Format::Classic => "NetCDF classic",
            Format::Offset64 => "NetCDF 64-bit offset",
            Format::Data64 => "NetCDF 64-bit data (CDF-5)",
            Format::Netcdf4 => "NetCDF-4",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dim {
    pub name: String,
    pub len: u64,
    pub unlimited: bool,
}

/// How stored values become physical ones (CF conventions).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Packing {
    pub scale: Option<f64>,
    pub offset: Option<f64>,
    /// `_FillValue`, or the type's default fill.
    pub fill: Option<f64>,
    pub missing: Vec<f64>,
    pub valid_min: Option<f64>,
    pub valid_max: Option<f64>,
    /// Integers stored signed that `_Unsigned = "true"` declares unsigned.
    pub unsigned: bool,
}

impl Packing {
    fn from_attrs(attrs: &[Attr], num: Option<Num>) -> Packing {
        let get = |n: &str| attrs.iter().find(|a| a.name == n).map(|a| &a.value);
        let num_attr = |n: &str| {
            get(n)
                .and_then(|v| v.as_f64s())
                .and_then(|v| v.first().copied())
        };
        let unsigned = get("_Unsigned")
            .and_then(Value::as_str)
            .is_some_and(|s| s.eq_ignore_ascii_case("true"))
            && matches!(num, Some(Num::I8 | Num::I16 | Num::I32 | Num::I64));
        let fill = num_attr("_FillValue").or_else(|| match num {
            // The Python bindings do not mask bytes by the default fill.
            Some(Num::I8 | Num::U8) | None => None,
            Some(n) => Some(default_fill(n, unsigned)),
        });
        let range = get("valid_range")
            .and_then(Value::as_f64s)
            .filter(|v| v.len() == 2);
        Packing {
            scale: num_attr("scale_factor"),
            offset: num_attr("add_offset"),
            fill,
            missing: get("missing_value")
                .and_then(Value::as_f64s)
                .unwrap_or_default(),
            valid_min: num_attr("valid_min").or(range.as_ref().map(|r| r[0])),
            valid_max: num_attr("valid_max").or(range.as_ref().map(|r| r[1])),
            unsigned,
        }
    }

    /// Whether anything is to be done.
    pub fn is_identity(&self) -> bool {
        *self == Packing::default()
    }

    /// One stored value to its physical value (NaN when masked).
    #[inline]
    pub fn apply(&self, v: f64) -> f64 {
        if v.is_nan()
            || self.fill == Some(v)
            || self.missing.contains(&v)
            || self.valid_min.is_some_and(|m| v < m)
            || self.valid_max.is_some_and(|m| v > m)
        {
            return f64::NAN;
        }
        match (self.scale, self.offset) {
            (Some(s), Some(o)) => s * v + o,
            (Some(s), None) => s * v,
            (None, Some(o)) => v + o,
            (None, None) => v,
        }
    }

    /// Reinterprets a signed stored value as unsigned when `_Unsigned` says so.
    #[inline]
    fn unsign(&self, v: f64, num: Num) -> f64 {
        if !self.unsigned || v >= 0.0 {
            return v;
        }
        v + match num {
            Num::I8 => 256.0,
            Num::I16 => 65536.0,
            Num::I32 => 4_294_967_296.0,
            _ => 18_446_744_073_709_551_616.0,
        }
    }
}

/// The default fill value of a type (netcdf.h).
pub fn default_fill(num: Num, unsigned: bool) -> f64 {
    match (num, unsigned) {
        (Num::I8, false) => -127.0,
        (Num::U8, _) | (Num::I8, true) => 255.0,
        (Num::I16, false) => -32767.0,
        (Num::U16, _) | (Num::I16, true) => 65535.0,
        (Num::I32, false) => -2_147_483_647.0,
        (Num::U32, _) | (Num::I32, true) => 4_294_967_295.0,
        (Num::I64, false) => -9_223_372_036_854_775_806.0,
        (Num::U64, _) | (Num::I64, true) => 18_446_744_073_709_551_614.0,
        (Num::F16 | Num::F32, _) => f64::from(9.969_21e36_f32),
        (Num::F64, _) => 9.969_209_968_386_869e36,
    }
}

/// A variable.
#[derive(Clone, Debug)]
pub struct Variable {
    /// The name (for NetCDF-4 variables in subgroups, the path without the leading `/`).
    pub name: String,
    pub dims: Vec<String>,
    pub shape: Vec<u64>,
    /// The element type, `None` for characters and strings.
    pub num: Option<Num>,
    /// User attributes (bookkeeping hidden).
    pub attrs: Vec<Attr>,
    pub packing: Packing,
    pub(crate) store: Store,
}

impl Variable {
    pub fn attr(&self, name: &str) -> Option<&Value> {
        self.attrs.iter().find(|a| a.name == name).map(|a| &a.value)
    }

    /// The attribute as text, trimmed, when a non-empty string.
    pub fn text(&self, name: &str) -> Option<&str> {
        self.attr(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// E.g. "16-bit signed, packed".
    pub fn describe(&self) -> String {
        let mut s = self.num.map_or_else(|| "text".to_owned(), Num::describe);
        if self.packing.scale.is_some() || self.packing.offset.is_some() {
            s += ", packed";
        }
        s
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Store {
    Classic(classic::Store),
    /// The object index in the HDF5 file.
    Hdf5(usize),
}

/// An opened NetCDF file.
pub struct Netcdf {
    pub format: Format,
    pub dims: Vec<Dim>,
    /// Global attributes.
    pub attrs: Vec<Attr>,
    pub vars: Vec<Variable>,
    hdf5: Option<specular_hdf5::File>,
}

impl std::fmt::Debug for Netcdf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Netcdf")
            .field("format", &self.format)
            .field("dims", &self.dims)
            .field("vars", &self.vars.len())
            .finish()
    }
}

impl Netcdf {
    /// Opens a classic or a NetCDF-4 file.
    pub fn open(data: &[u8], cancel: &(dyn Fn() -> bool + Sync)) -> Result<Netcdf> {
        if sniff_classic(data) {
            return classic::parse(data);
        }
        if specular_hdf5::find_superblock(data).is_some() {
            return Ok(Netcdf::from_hdf5(specular_hdf5::File::open_with(
                data, cancel,
            )?));
        }
        Err(Error::NotNetcdf)
    }

    /// Maps an opened HDF5 file through the NetCDF-4 conventions.
    pub fn from_hdf5(file: specular_hdf5::File) -> Netcdf {
        let mut dims = Vec::new();
        let mut vars = Vec::new();
        for (i, o, ds) in file.datasets() {
            let scale = o.attr("CLASS").and_then(Value::as_str) == Some("DIMENSION_SCALE");
            let pure = o
                .attr("NAME")
                .and_then(Value::as_str)
                .is_some_and(|n| n.starts_with(PURE_DIM));
            if scale && ds.shape().len() == 1 {
                dims.push(Dim {
                    name: o.name().to_owned(),
                    len: ds.shape()[0],
                    unlimited: ds.space.max.first() == Some(&u64::MAX),
                });
            }
            if pure {
                continue;
            }
            let num = ds.dtype.num().map(|(n, _)| n);
            let dim_names: Vec<String> = match o.attr("DIMENSION_LIST") {
                Some(Value::Refs(r)) => r
                    .iter()
                    .map(|refs| {
                        refs.first()
                            .and_then(|&a| file.at_addr(a))
                            .map_or_else(String::new, |d| d.name().to_owned())
                    })
                    .collect(),
                _ if scale => vec![o.name().to_owned()],
                _ => (0..ds.shape().len()).map(|k| format!("dim{k}")).collect(),
            };
            let attrs: Vec<Attr> = o
                .attrs
                .iter()
                .filter(|a| !is_hidden_attr(&a.name))
                .cloned()
                .collect();
            vars.push(Variable {
                name: o.path.trim_start_matches('/').to_owned(),
                dims: dim_names,
                shape: ds.shape().to_vec(),
                num,
                packing: Packing::from_attrs(&attrs, num),
                attrs,
                store: Store::Hdf5(i),
            });
        }
        let attrs = file
            .get("/")
            .map(|r| {
                r.attrs
                    .iter()
                    .filter(|a| !is_hidden_attr(&a.name))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        Netcdf {
            format: Format::Netcdf4,
            dims,
            attrs,
            vars,
            hdf5: Some(file),
        }
    }

    /// The HDF5 file behind a NetCDF-4 file.
    pub fn hdf5(&self) -> Option<&specular_hdf5::File> {
        self.hdf5.as_ref()
    }

    pub fn var(&self, name: &str) -> Option<&Variable> {
        self.vars.iter().find(|v| v.name == name)
    }

    pub fn attr(&self, name: &str) -> Option<&Value> {
        self.attrs.iter().find(|a| a.name == name).map(|a| &a.value)
    }

    /// Reads one plane of a numeric variable as physical values (see the crate docs).
    pub fn read_plane(
        &self,
        data: &[u8],
        var: usize,
        spec: &PlaneSpec,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<specular_hdf5::Plane<f64>> {
        let v = self
            .vars
            .get(var)
            .ok_or_else(|| Error::Malformed("no such variable".into()))?;
        let num = v
            .num
            .ok_or_else(|| Error::Unsupported("text variable".into()))?;
        let mut p = match &v.store {
            Store::Classic(s) => classic::plane(data, s, num, &v.shape, spec, 1)?,
            Store::Hdf5(i) => self
                .hdf5
                .as_ref()
                .ok_or_else(|| Error::Malformed("no HDF5 file".into()))?
                .read_plane::<f64>(data, *i, spec, cancel)?,
        };
        self.unpack(v, num, &mut p.values);
        Ok(p)
    }

    /// Reads every `step`-th sample of one plane along x and y as physical values, for a
    /// quick look at a big variable (see [`specular_hdf5::File::read_plane_strided`] for
    /// `budget`, which bounds the chunks of a NetCDF-4 variable decompressed).
    pub fn read_plane_strided(
        &self,
        data: &[u8],
        var: usize,
        spec: &PlaneSpec,
        step: u64,
        budget: u64,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<specular_hdf5::Plane<f64>> {
        let v = self
            .vars
            .get(var)
            .ok_or_else(|| Error::Malformed("no such variable".into()))?;
        let num = v
            .num
            .ok_or_else(|| Error::Unsupported("text variable".into()))?;
        let mut p = match &v.store {
            Store::Classic(s) => classic::plane(data, s, num, &v.shape, spec, step)?,
            Store::Hdf5(i) => self
                .hdf5
                .as_ref()
                .ok_or_else(|| Error::Malformed("no HDF5 file".into()))?
                .read_plane_strided::<f64>(data, *i, spec, step, budget, cancel)?,
        };
        self.unpack(v, num, &mut p.values);
        Ok(p)
    }

    /// Reads a whole numeric variable (at most `max` elements) as physical values.
    pub fn read_all(&self, data: &[u8], var: usize, max: u64) -> Result<Vec<f64>> {
        let v = self
            .vars
            .get(var)
            .ok_or_else(|| Error::Malformed("no such variable".into()))?;
        let num = v
            .num
            .ok_or_else(|| Error::Unsupported("text variable".into()))?;
        let count = v.shape.iter().try_fold(1u64, |a, &d| a.checked_mul(d));
        if count.is_none_or(|n| n > max.min(limits::MAX_ALL)) {
            return Err(Error::TooLarge);
        }
        let mut out = match &v.store {
            Store::Classic(s) => classic::all(data, s, num, &v.shape)?,
            Store::Hdf5(i) => self
                .hdf5
                .as_ref()
                .ok_or_else(|| Error::Malformed("no HDF5 file".into()))?
                .read_all::<f64>(data, *i, max)?,
        };
        self.unpack(v, num, &mut out);
        Ok(out)
    }

    /// Reads a box (`start` and `count` per dimension) of a numeric variable as physical
    /// values, row-major over `count`; the flag tells whether some values were missing
    /// from the file (they read as NaN).
    pub fn read_box(
        &self,
        data: &[u8],
        var: usize,
        start: &[u64],
        count: &[u64],
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<(Vec<f64>, bool)> {
        let v = self
            .vars
            .get(var)
            .ok_or_else(|| Error::Malformed("no such variable".into()))?;
        let num = v
            .num
            .ok_or_else(|| Error::Unsupported("text variable".into()))?;
        let n = count.iter().try_fold(1u64, |a, &d| a.checked_mul(d));
        if n.is_none_or(|n| n > limits::MAX_ALL) {
            return Err(Error::TooLarge);
        }
        let (mut out, truncated) = match &v.store {
            Store::Classic(s) => classic::boxed(data, s, num, &v.shape, start, count)?,
            Store::Hdf5(i) => self
                .hdf5
                .as_ref()
                .ok_or_else(|| Error::Malformed("no HDF5 file".into()))?
                .read_box::<f64>(data, *i, start, count, cancel)?,
        };
        self.unpack(v, num, &mut out);
        Ok((out, truncated))
    }

    fn unpack(&self, v: &Variable, num: Num, values: &mut [f64]) {
        let p = &v.packing;
        if p.is_identity() && !p.unsigned {
            return;
        }
        for x in values {
            *x = p.apply(p.unsign(*x, num));
        }
    }

    /// The coordinate variable of a dimension (a 1-D variable of the same name), read whole.
    pub fn coordinates(&self, data: &[u8], dim: &str) -> Option<(usize, Vec<f64>)> {
        let i = self
            .vars
            .iter()
            .position(|v| v.name == dim && v.dims.len() == 1 && v.dims[0] == dim)?;
        let vals = self.read_all(data, i, 1 << 24).ok()?;
        Some((i, vals))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing() {
        let attrs = vec![
            Attr {
                name: "scale_factor".into(),
                value: Value::Float(vec![0.5]),
            },
            Attr {
                name: "add_offset".into(),
                value: Value::Float(vec![10.0]),
            },
            Attr {
                name: "valid_range".into(),
                value: Value::Int(vec![0, 100]),
            },
        ];
        let p = Packing::from_attrs(&attrs, Some(Num::I16));
        assert_eq!(p.apply(4.0), 12.0);
        assert!(p.apply(-32767.0).is_nan());
        assert!(p.apply(101.0).is_nan());
        assert!(Packing::from_attrs(&[], Some(Num::U8)).is_identity());
    }

    #[test]
    fn garbage() {
        assert!(matches!(
            Netcdf::open(b"not a netcdf file", &|| false),
            Err(Error::NotNetcdf)
        ));
        assert!(!sniff_classic(b"CDF\x03"));
        assert!(sniff_classic(b"CDF\x05"));
    }

    fn name(out: &mut Vec<u8>, s: &str) {
        out.extend_from_slice(&(s.len() as u32).to_be_bytes());
        out.extend_from_slice(s.as_bytes());
        out.resize(out.len().next_multiple_of(4), 0);
    }

    /// A CDF-1 file: `fixed` is float [7][11][13]; `rec` (short) and `other` (byte) are
    /// record variables of [5 records][9][10], interleaved record by record.
    fn classic() -> (Vec<u8>, Vec<f32>, Vec<i16>) {
        let fixed: Vec<f32> = (0..7 * 11 * 13).map(|i| (i * 37 % 1001) as f32).collect();
        let rec: Vec<i16> = (0..5 * 9 * 10)
            .map(|i| (i * 13 % 997) as i16 - 400)
            .collect();
        let mut h = b"CDF\x01".to_vec();
        h.extend_from_slice(&5u32.to_be_bytes());
        h.extend_from_slice(&0x0Au32.to_be_bytes());
        h.extend_from_slice(&6u32.to_be_bytes());
        for (n, len) in [
            ("t", 0u32),
            ("z", 7),
            ("y", 11),
            ("x", 13),
            ("v", 9),
            ("u", 10),
        ] {
            name(&mut h, n);
            h.extend_from_slice(&len.to_be_bytes());
        }
        h.extend_from_slice(&[0; 8]);
        h.extend_from_slice(&0x0Bu32.to_be_bytes());
        h.extend_from_slice(&3u32.to_be_bytes());
        // (name, dim ids, type, vsize); begins are patched below.
        let vars: [(&str, &[u32], u32, u32); 3] = [
            ("fixed", &[1, 2, 3], 5, 7 * 11 * 13 * 4),
            ("rec", &[0, 4, 5], 3, 9 * 10 * 2),
            ("other", &[0, 4, 5], 1, 92),
        ];
        let mut begins = Vec::new();
        for (n, dims, ty, vsize) in vars {
            name(&mut h, n);
            h.extend_from_slice(&(dims.len() as u32).to_be_bytes());
            for d in dims {
                h.extend_from_slice(&d.to_be_bytes());
            }
            h.extend_from_slice(&[0; 8]);
            h.extend_from_slice(&ty.to_be_bytes());
            h.extend_from_slice(&vsize.to_be_bytes());
            begins.push(h.len());
            h.extend_from_slice(&0u32.to_be_bytes());
        }
        let fixed_at = h.len();
        for v in &fixed {
            h.extend_from_slice(&v.to_be_bytes());
        }
        let rec_at = h.len();
        for r in 0..5 {
            for v in &rec[r * 90..(r + 1) * 90] {
                h.extend_from_slice(&v.to_be_bytes());
            }
            h.extend_from_slice(&[7; 92]);
        }
        for (at, begin) in begins.iter().zip([fixed_at, rec_at, rec_at + 180]) {
            h[*at..at + 4].copy_from_slice(&(begin as u32).to_be_bytes());
        }
        (h, fixed, rec)
    }

    /// A header-only classic file declaring an 8192×8192 float variable: its 512 MB plane
    /// (and 512 MB whole-variable read) was allocated before any value was read.
    #[test]
    fn header_only_variable_is_refused() {
        fn name(h: &mut Vec<u8>, n: &str) {
            h.extend_from_slice(&(n.len() as u32).to_be_bytes());
            h.extend_from_slice(n.as_bytes());
            h.resize(h.len().next_multiple_of(4), 0);
        }
        let mut h = b"CDF\x01".to_vec();
        h.extend_from_slice(&0u32.to_be_bytes());
        h.extend_from_slice(&[0, 0, 0, 0x0a, 0, 0, 0, 2]);
        for d in ["y", "x"] {
            name(&mut h, d);
            h.extend_from_slice(&8192u32.to_be_bytes());
        }
        h.extend_from_slice(&[0; 8]);
        h.extend_from_slice(&[0, 0, 0, 0x0b, 0, 0, 0, 1]);
        name(&mut h, "v");
        for v in [2u32, 0, 1, 0, 0, 5, 0] {
            h.extend_from_slice(&v.to_be_bytes());
        }
        let begin = h.len() as u32 + 4;
        h.extend_from_slice(&begin.to_be_bytes());
        h.extend_from_slice(&[0; 64]);
        let nc = Netcdf::open(&h, &|| false).unwrap();
        let spec = PlaneSpec {
            x_dim: 1,
            y_dim: 0,
            index: vec![0, 0],
        };
        assert!(matches!(
            nc.read_plane(&h, 0, &spec, &|| false),
            Err(Error::Malformed(_))
        ));
    }

    /// A classic file with a byte variable of 2048 × 4096 (8 Mi values, 64 MiB once read
    /// as f64), of which `have` bytes are in the file.
    fn byte_variable(have: usize) -> Vec<u8> {
        let mut h = b"CDF\x01".to_vec();
        h.extend_from_slice(&0u32.to_be_bytes());
        h.extend_from_slice(&[0, 0, 0, 0x0a, 0, 0, 0, 2]);
        for (d, n) in [("y", 2048u32), ("x", 4096)] {
            name(&mut h, d);
            h.extend_from_slice(&n.to_be_bytes());
        }
        h.extend_from_slice(&[0; 8]);
        h.extend_from_slice(&[0, 0, 0, 0x0b, 0, 0, 0, 1]);
        name(&mut h, "v");
        for v in [2u32, 0, 1, 0, 0, 1, 0] {
            h.extend_from_slice(&v.to_be_bytes());
        }
        let begin = h.len() as u32 + 4;
        h.extend_from_slice(&begin.to_be_bytes());
        h.resize(h.len() + have, 3);
        h
    }

    /// Whole-variable reads and planes come out 8 bytes a value: reserved from the memory
    /// budget before they are allocated, and refused when the file holds under half.
    #[test]
    fn reads_reserved_and_bounded_by_the_file() {
        use specular_core::budget::with_limit;
        let spec = PlaneSpec {
            x_dim: 1,
            y_dim: 0,
            index: vec![0, 0],
        };
        // A sixteenth of the values (512 KiB for 64 MiB): no longer read.
        let h = byte_variable(512 << 10);
        let nc = Netcdf::open(&h, &|| false).unwrap();
        let r = nc.read_all(&h, 0, u64::MAX);
        assert!(matches!(r, Err(Error::Malformed(_))), "{r:?}");
        // Half of them (4 MiB): read, the rest NaN, within the budget only.
        let h = byte_variable(4 << 20);
        let nc = Netcdf::open(&h, &|| false).unwrap();
        let r = with_limit(32 << 20, || nc.read_all(&h, 0, u64::MAX));
        assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
        let r = with_limit(32 << 20, || nc.read_plane(&h, 0, &spec, &|| false));
        assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
        let r = with_limit(32 << 20, || {
            nc.read_box(&h, 0, &[0, 0], &[2048, 4096], &|| false)
        });
        assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
        let v = with_limit(64 << 20, || nc.read_all(&h, 0, u64::MAX)).unwrap();
        assert_eq!(v.len(), 8 << 20);
        assert_eq!(v[0], 3.0);
        assert!(v[(8 << 20) - 1].is_nan());
    }

    #[test]
    fn strided_classic_planes_match_the_full_ones() {
        let (data, _, _) = classic();
        let nc = Netcdf::open(&data, &|| false).unwrap();
        let mut bad = 0;
        let mut checked = 0;
        for var in ["fixed", "rec"] {
            let i = nc.vars.iter().position(|v| v.name == var).unwrap();
            let r = nc.vars[i].shape.len();
            assert_eq!(r, 3);
            for (x_dim, y_dim) in [(2, 1), (1, 2), (2, 0)] {
                for step in [1u64, 2, 3, 4, 7, 20] {
                    let spec = PlaneSpec {
                        x_dim,
                        y_dim,
                        index: vec![2, 3, 0],
                    };
                    let full = nc.read_plane(&data, i, &spec, &|| false).unwrap();
                    let got = nc
                        .read_plane_strided(&data, i, &spec, step, u64::MAX, &|| false)
                        .unwrap();
                    let s = step as usize;
                    assert_eq!(got.width, full.width.div_ceil(s));
                    assert_eq!(got.height, full.height.div_ceil(s));
                    assert!(!got.truncated && !full.truncated);
                    for y in 0..got.height {
                        for x in 0..got.width {
                            let want = full.values[y * s * full.width + x * s];
                            bad += usize::from(got.values[y * got.width + x] != want);
                        }
                    }
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 36);
        assert_eq!(bad, 0, "{bad} mismatches");
    }

    #[test]
    fn strided_classic_values() {
        let (data, fixed, rec) = classic();
        let nc = Netcdf::open(&data, &|| false).unwrap();
        let i = nc.vars.iter().position(|v| v.name == "fixed").unwrap();
        let spec = PlaneSpec {
            x_dim: 2,
            y_dim: 1,
            index: vec![4, 0, 0],
        };
        let p = nc
            .read_plane_strided(&data, i, &spec, 3, u64::MAX, &|| false)
            .unwrap();
        assert_eq!((p.width, p.height), (5, 4));
        assert_eq!(p.values[6], f64::from(fixed[4 * 143 + 3 * 13 + 3]));
        let i = nc.vars.iter().position(|v| v.name == "rec").unwrap();
        let spec = PlaneSpec {
            x_dim: 2,
            y_dim: 0,
            index: vec![0, 8, 0],
        };
        let p = nc
            .read_plane_strided(&data, i, &spec, 2, u64::MAX, &|| false)
            .unwrap();
        assert_eq!((p.width, p.height), (5, 3));
        assert_eq!(p.values[5 + 4], f64::from(rec[2 * 90 + 8 * 10 + 8]));
    }
}
