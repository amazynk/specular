//! MINC volumes: MINC2 (`/minc-2.0/` in an HDF5 file) and MINC1 (NetCDF classic), the
//! format of the Montreal Neurological Institute's tools and BrainWeb.
//!
//! Both keep the same variables: `image` (its `dimorder`, slowest axis first, names
//! `xspace`, `yspace`, `zspace`, `time`, `vector_dimension`), one variable per dimension
//! carrying `step`, `start` and `direction_cosines`, and `image-min` / `image-max`, which map
//! the stored `valid_range` of every slice (or of the whole volume) onto real values. World
//! coordinates are RAS; the voxel-to-world mapping is `cosines × (start + index × step)`.
//!
//! Floating-point images are never rescaled. An integer image whose slices share one scale
//! keeps its stored type with that scale; one whose slices differ is read as 32-bit floats,
//! each slice scaled on its own (clipped to the valid range first, as MINC does).

use specular_hdf5 as hdf5;
use specular_netcdf as netcdf;

use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, MedVolume,
    OrientationSource, Result, Skip,
};

const MINC2_ROOT: &str = "/minc-2.0";

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Whether `data` (a file's first bytes, the more the better) is a MINC2 or MINC1 file.
pub fn sniff(data: &[u8]) -> Option<Kind> {
    let head = &data[..data.len().min(1 << 16)];
    if head.starts_with(b"\x89HDF\r\n\x1a\n") && contains(head, b"minc-2.0") {
        return Some(Kind::Minc2);
    }
    // A NetCDF classic header naming an `image` variable and a MINC spatial dimension.
    if (head.starts_with(b"CDF\x01") || head.starts_with(b"CDF\x02"))
        && contains(head, b"\0\0\0\x05image\0\0\0")
        && (contains(head, b"xspace") || contains(head, b"zspace"))
    {
        return Some(Kind::Minc1);
    }
    None
}

/// The variables MINC reads, from either container.
trait Vars {
    /// The `image` variable's dimension names (slowest first), shape and stored type.
    fn image(&self) -> Result<(Vec<String>, Vec<usize>, DataType, bool)>;
    fn num_attr(&self, var: &str, name: &str) -> Option<Vec<f64>>;
    fn text_attr(&self, var: &str, name: &str) -> Option<String>;
    /// A whole (small) variable as numbers, with its rank.
    fn values(&self, var: &str) -> Option<(usize, Vec<f64>)>;
}

struct Minc2<'a> {
    file: hdf5::File,
    data: &'a [u8],
}

impl Minc2<'_> {
    fn path(var: &str) -> String {
        match var {
            "image" | "image-min" | "image-max" => format!("{MINC2_ROOT}/image/0/{var}"),
            d => format!("{MINC2_ROOT}/dimensions/{d}"),
        }
    }
}

fn data_type(num: hdf5::Num) -> Result<DataType> {
    Ok(match num {
        hdf5::Num::U8 => DataType::U8,
        hdf5::Num::I8 => DataType::I8,
        hdf5::Num::U16 => DataType::U16,
        hdf5::Num::I16 => DataType::I16,
        hdf5::Num::U32 => DataType::U32,
        hdf5::Num::I32 => DataType::I32,
        hdf5::Num::U64 => DataType::U64,
        hdf5::Num::I64 => DataType::I64,
        hdf5::Num::F32 => DataType::F32,
        hdf5::Num::F64 => DataType::F64,
        hdf5::Num::F16 => return Err(Error::Unsupported("16-bit float MINC images".into())),
    })
}

impl Vars for Minc2<'_> {
    fn image(&self) -> Result<(Vec<String>, Vec<usize>, DataType, bool)> {
        let o = self
            .file
            .get(&Self::path("image"))
            .ok_or_else(|| Error::Malformed("MINC2 file without an image".into()))?;
        let ds = o
            .dataset()
            .ok_or_else(|| Error::Malformed("MINC2 image".into()))?;
        let hdf5::Dtype::Num { num, be } = ds.dtype else {
            return Err(Error::Unsupported("non-numeric MINC image".into()));
        };
        let shape: Vec<usize> = ds.shape().iter().map(|&d| d as usize).collect();
        let names = names(
            o.attr("dimorder").and_then(hdf5::Value::as_str),
            shape.len(),
        );
        Ok((names, shape, data_type(num)?, be))
    }

    fn num_attr(&self, var: &str, name: &str) -> Option<Vec<f64>> {
        self.file.get(&Self::path(var))?.attr(name)?.as_f64s()
    }

    fn text_attr(&self, var: &str, name: &str) -> Option<String> {
        Some(
            self.file
                .get(&Self::path(var))?
                .attr(name)?
                .as_str()?
                .to_owned(),
        )
    }

    fn values(&self, var: &str) -> Option<(usize, Vec<f64>)> {
        let i = self.file.index_of(&Self::path(var))?;
        let rank = self.file.objects[i].dataset()?.shape().len();
        let v = self.file.read_all::<f64>(self.data, i, 1 << 24).ok()?;
        Some((rank, v))
    }
}

struct Minc1<'a> {
    nc: netcdf::Netcdf,
    data: &'a [u8],
}

impl Vars for Minc1<'_> {
    fn image(&self) -> Result<(Vec<String>, Vec<usize>, DataType, bool)> {
        let v = self
            .nc
            .var("image")
            .ok_or_else(|| Error::Malformed("MINC file without an image".into()))?;
        let unsigned = v
            .text("signtype")
            .is_some_and(|s| s.trim_end_matches(['_', '\0', ' ']) == "unsigned");
        let t = match (v.num, unsigned) {
            (Some(hdf5::Num::I8), true) => DataType::U8,
            (Some(hdf5::Num::I16), true) => DataType::U16,
            (Some(hdf5::Num::I32), true) => DataType::U32,
            (Some(n), _) => data_type(n)?,
            (None, _) => return Err(Error::Unsupported("text MINC image".into())),
        };
        let shape = v.shape.iter().map(|&d| d as usize).collect();
        Ok((v.dims.clone(), shape, t, false))
    }

    fn num_attr(&self, var: &str, name: &str) -> Option<Vec<f64>> {
        self.nc.var(var)?.attr(name)?.as_f64s()
    }

    fn text_attr(&self, var: &str, name: &str) -> Option<String> {
        Some(self.nc.var(var)?.text(name)?.to_owned())
    }

    fn values(&self, var: &str) -> Option<(usize, Vec<f64>)> {
        let i = self.nc.vars.iter().position(|v| v.name == var)?;
        let rank = self.nc.vars[i].shape.len();
        Some((rank, self.nc.read_all(self.data, i, 1 << 24).ok()?))
    }
}

/// `dimorder` split, or MINC's usual order when it is missing.
fn names(dimorder: Option<&str>, rank: usize) -> Vec<String> {
    match dimorder {
        Some(s) => s
            .split(',')
            .map(|n| n.trim().trim_end_matches('\0').to_owned())
            .take(rank)
            .collect(),
        None => ["time", "zspace", "yspace", "xspace"][4 - rank.min(4)..]
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
    }
}

/// How stored values become real ones.
enum Scaling {
    /// None, or one slope and intercept for the whole volume.
    Uniform(Option<(f64, f64)>),
    /// Per block of `block` samples (one or two slowest axes): slope and intercept, with
    /// the valid range to clip to.
    PerBlock {
        block: usize,
        coeffs: Vec<(f64, f64)>,
        valid: (f64, f64),
    },
}

fn type_range(t: DataType) -> (f64, f64) {
    match t {
        DataType::U8 => (0.0, 255.0),
        DataType::I8 => (-128.0, 127.0),
        DataType::U16 => (0.0, 65535.0),
        DataType::I16 => (-32768.0, 32767.0),
        DataType::U32 => (0.0, u32::MAX as f64),
        DataType::I32 => (i32::MIN as f64, i32::MAX as f64),
        DataType::U64 => (0.0, u64::MAX as f64),
        DataType::I64 => (i64::MIN as f64, i64::MAX as f64),
        DataType::F32 | DataType::F64 => (0.0, 1.0),
    }
}

fn scaling(v: &dyn Vars, shape: &[usize], dtype: DataType) -> Scaling {
    if dtype.is_float() {
        return Scaling::Uniform(None);
    }
    let (Some((rmin, imin)), Some((rmax, imax))) = (v.values("image-min"), v.values("image-max"))
    else {
        return Scaling::Uniform(None);
    };
    let valid = v
        .num_attr("image", "valid_range")
        .filter(|r| r.len() == 2 && r[1] > r[0])
        .map(|r| (r[0], r[1]))
        .or_else(|| {
            let lo = v.num_attr("image", "valid_min")?.first().copied()?;
            let hi = v.num_attr("image", "valid_max")?.first().copied()?;
            (hi > lo).then_some((lo, hi))
        })
        .unwrap_or_else(|| type_range(dtype));
    let n = imin.len();
    // The scale variables span the slowest one or two axes (or none: a scalar).
    let ok = rmin == rmax
        && rmin <= 2
        && rmin < shape.len()
        && imax.len() == n
        && shape[..rmin].iter().product::<usize>() == n;
    if !ok || n == 0 {
        return Scaling::Uniform(None);
    }
    let coeffs: Vec<(f64, f64)> = imin
        .iter()
        .zip(&imax)
        .map(|(&lo, &hi)| {
            let slope = (hi - lo) / (valid.1 - valid.0);
            (slope, lo - valid.0 * slope)
        })
        .collect();
    if coeffs.iter().all(|c| *c == coeffs[0]) {
        let c = coeffs[0];
        return Scaling::Uniform((c != (1.0, 0.0)).then_some(c));
    }
    Scaling::PerBlock {
        block: shape[rmin..].iter().product(),
        coeffs,
        valid,
    }
}

/// Everything parsed from the variables, before the voxels.
struct Parsed {
    header: Header,
    stored: DataType,
    big_endian: bool,
    scaling: Scaling,
}

fn parse_vars(v: &dyn Vars, kind: Kind) -> Result<Parsed> {
    let (names, shape, stored, big_endian) = v.image()?;
    if names.len() != shape.len() || shape.is_empty() || shape.contains(&0) {
        return Err(Error::Malformed("MINC image dimensions".into()));
    }
    let rank = shape.len();
    // Row-major strides, in samples.
    let mut stride = vec![1usize; rank];
    for d in (0..rank - 1).rev() {
        stride[d] = stride[d + 1]
            .checked_mul(shape[d + 1])
            .ok_or(Error::TooLarge)?;
    }
    let samples = stride[0].checked_mul(shape[0]).ok_or(Error::TooLarge)?;
    // Spatial axes fastest first: i, j, k.
    let spatial: Vec<usize> = (0..rank)
        .rev()
        .filter(|&d| names[d].ends_with("space"))
        .collect();
    if spatial.len() != 3 {
        return Err(Error::Unsupported(format!(
            "MINC images with {} spatial dimensions",
            spatial.len()
        )));
    }
    let extra: Vec<(usize, usize)> = (0..rank)
        .rev()
        .filter(|d| !spatial.contains(d))
        .map(|d| (shape[d], stride[d]))
        .collect();
    let mut affine = [[0.0; 4]; 3];
    let mut spacing = [1.0; 3];
    let mut origin = [0.0; 3];
    for (a, &d) in spatial.iter().enumerate() {
        let name = names[d].as_str();
        let default: [f64; 3] = match name {
            "xspace" => [1.0, 0.0, 0.0],
            "yspace" => [0.0, 1.0, 0.0],
            _ => [0.0, 0.0, 1.0],
        };
        let cos = v
            .num_attr(name, "direction_cosines")
            .filter(|c| c.len() == 3 && c.iter().all(|x| x.is_finite()))
            .map_or(default, |c| [c[0], c[1], c[2]]);
        let first = |n: &str| v.num_attr(name, n).and_then(|x| x.first().copied());
        let step = first("step")
            .filter(|s| s.is_finite() && *s != 0.0)
            .unwrap_or(1.0);
        let start = first("start").filter(|s| s.is_finite()).unwrap_or(0.0);
        spacing[a] = step.abs();
        for r in 0..3 {
            // RAS to LPS: x and y change sign.
            let s = if r < 2 { -1.0 } else { 1.0 };
            affine[r][a] = s * cos[r] * step;
            origin[r] += s * cos[r] * start;
        }
    }
    for r in 0..3 {
        affine[r][3] = origin[r];
    }
    let scaling = scaling(v, &shape, stored);
    let (dtype, scale) = match &scaling {
        Scaling::Uniform(s) => (stored, *s),
        Scaling::PerBlock { .. } => (DataType::F32, None),
    };
    let size = [shape[spatial[0]], shape[spatial[1]], shape[spatial[2]]];
    let mut dims = size.to_vec();
    dims.extend(extra.iter().map(|e| e.0));
    let description = v
        .text_attr("image", "long_name")
        .filter(|s| !s.trim().is_empty());
    let header = Header {
        kind,
        dtype,
        layout: Layout {
            size,
            stride: [stride[spatial[0]], stride[spatial[1]], stride[spatial[2]]],
            extra,
            colour: None,
            samples,
        },
        affine,
        orientation: OrientationSource::Minc,
        dims,
        spacing,
        scale,
        cal: None,
        description,
        intent: None,
        anatomical: None,
        params: Vec::new(),
        data: DataRef {
            file: DataFile::Attached,
            skip: Skip::Start { lines: 0, bytes: 0 },
            encoding: Encoding::Container,
            big_endian: false,
        },
    };
    Ok(Parsed {
        header,
        stored,
        big_endian,
        scaling,
    })
}

fn open_minc2(data: &[u8]) -> Result<Minc2<'_>> {
    let file = hdf5::File::open(data).map_err(|e| Error::Malformed(format!("MINC2: {e}")))?;
    if file.get(MINC2_ROOT).is_none() {
        return Err(Error::NotRecognised);
    }
    Ok(Minc2 { file, data })
}

fn open_minc1(data: &[u8]) -> Result<Minc1<'_>> {
    let mut nc = netcdf::Netcdf::open(data, &|| false)
        .map_err(|e| Error::Malformed(format!("MINC: {e}")))?;
    // MINC's valid_range is not a mask: read stored values as they are.
    for v in &mut nc.vars {
        v.packing = netcdf::Packing::default();
    }
    Ok(Minc1 { nc, data })
}

fn parse_any(data: &[u8], kind: Kind) -> Result<Parsed> {
    match kind {
        Kind::Minc2 => parse_vars(&open_minc2(data)?, kind),
        _ => parse_vars(&open_minc1(data)?, kind),
    }
}

/// Parses a MINC file's header (all of the file is needed: the container's metadata).
pub fn parse(data: &[u8], kind: Kind) -> Result<Header> {
    Ok(parse_any(data, kind)?.header)
}

/// The stored samples, row-major, as little-endian bytes of `stored`, with their
/// reservation from the memory budget.
fn stored_bytes(
    data: &[u8],
    kind: Kind,
    p: &Parsed,
) -> Result<(Vec<u8>, specular_core::Reservation)> {
    let need = p
        .header
        .layout
        .samples
        .checked_mul(p.stored.size())
        .filter(|&n| n <= 16 << 30)
        .ok_or(Error::TooLarge)?;
    if kind == Kind::Minc2 {
        let m = open_minc2(data)?;
        let i = m
            .file
            .index_of(&Minc2::path("image"))
            .ok_or_else(|| Error::Malformed("MINC2 image".into()))?;
        let shape = m.file.objects[i]
            .dataset()
            .map(|d| d.shape().to_vec())
            .unwrap_or_default();
        let mut b = m
            .file
            .read_box_bytes(data, i, &vec![0; shape.len()], &shape, &|| false)
            .map_err(|e| match e {
                hdf5::Error::NeedsMemory(n) => Error::NeedsMemory(n),
                e => Error::Malformed(format!("MINC2 image: {e}")),
            })?;
        if b.len() != need {
            return Err(Error::Truncated("MINC2 image".into()));
        }
        if p.big_endian {
            for e in b.chunks_exact_mut(p.stored.size()) {
                e.reverse();
            }
        }
        let r = specular_core::budget::reserve(b.capacity() as u64)?;
        return Ok((b, r));
    }
    let m = open_minc1(data)?;
    let i =
        m.nc.vars
            .iter()
            .position(|v| v.name == "image")
            .ok_or_else(|| Error::Malformed("MINC image".into()))?;
    let vals = m.nc.read_all(data, i, u64::MAX).map_err(|e| match e {
        netcdf::Error::NeedsMemory(n) => Error::NeedsMemory(n),
        e => Error::Malformed(format!("MINC image: {e}")),
    })?;
    let (mut b, r) = crate::data::alloc(need)?;
    let (lo, _) = type_range(p.stored);
    for x in vals {
        // Classic NetCDF has signed integers only; MINC's signtype makes them unsigned.
        let x = if lo == 0.0 && x < 0.0 && !p.stored.is_float() {
            x + 2f64.powi(p.stored.bits() as i32)
        } else {
            x
        };
        match p.stored {
            DataType::U8 => b.push(x as u8),
            DataType::I8 => b.push(x as i8 as u8),
            DataType::U16 => b.extend_from_slice(&(x as u16).to_le_bytes()),
            DataType::I16 => b.extend_from_slice(&(x as i16).to_le_bytes()),
            DataType::U32 => b.extend_from_slice(&(x as u32).to_le_bytes()),
            DataType::I32 => b.extend_from_slice(&(x as i32).to_le_bytes()),
            DataType::U64 => b.extend_from_slice(&(x as u64).to_le_bytes()),
            DataType::I64 => b.extend_from_slice(&(x as i64).to_le_bytes()),
            DataType::F32 => b.extend_from_slice(&(x as f32).to_le_bytes()),
            DataType::F64 => b.extend_from_slice(&x.to_le_bytes()),
        }
    }
    if b.len() != need {
        return Err(Error::Truncated("MINC image".into()));
    }
    Ok((b, r))
}

/// Reads the voxels of a MINC file whose header is `header`, from all of its bytes.
pub fn load(header: Header, data: &[u8]) -> Result<MedVolume> {
    let kind = header.kind;
    let p = parse_any(data, kind)?;
    if p.header.layout != header.layout || p.header.dtype != header.dtype {
        return Err(Error::Malformed("MINC file changed while reading".into()));
    }
    let (raw, raw_res) = stored_bytes(data, kind, &p)?;
    let bytes = match &p.scaling {
        Scaling::Uniform(_) => crate::data::held(raw, raw_res),
        Scaling::PerBlock {
            block,
            coeffs,
            valid,
        } => {
            let size = p.stored.size();
            let (mut out, res) = crate::data::alloc(raw.len() / size * 4)?;
            for (s, e) in raw.chunks_exact(size).enumerate() {
                let x = crate::data::sample_f64(e, 0, 0, p.stored, false);
                let (m, b) = coeffs.get(s / block).copied().unwrap_or((1.0, 0.0));
                let y = x.clamp(valid.0, valid.1) * m + b;
                out.extend_from_slice(&(y as f32).to_le_bytes());
            }
            crate::data::held(out, res)
        }
    };
    Ok(MedVolume {
        header,
        bytes,
        offset: 0,
        big_endian: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hdf5::writer::{AttrValue, ROOT, Storage, Ty, Writer};

    const SHAPE: [usize; 3] = [4, 3, 5]; // z, y, x

    fn stored() -> Vec<u16> {
        (0..60).map(|i| (i * 1000) as u16).collect()
    }

    /// A MINC2 file: x reversed (step -2), y and z tilted, per-slice scaling unless
    /// `uniform`.
    fn minc2(uniform: bool) -> Vec<u8> {
        let mut w = Writer::new();
        let dims = w.groups("/minc-2.0/dimensions");
        let img = w.groups("/minc-2.0/image/0");
        let d = |w: &mut Writer, name: &str, step: f64, start: f64, cos: [f64; 3]| {
            let n = w.dataset(dims, name, Ty::I32, &[], vec![0; 4], Storage::Contiguous);
            w.attr(n, "step", AttrValue::F64(vec![step]));
            w.attr(n, "start", AttrValue::F64(vec![start]));
            w.attr(n, "direction_cosines", AttrValue::F64(cos.to_vec()));
        };
        let s = std::f64::consts::FRAC_1_SQRT_2;
        d(&mut w, "xspace", -2.0, 10.0, [1.0, 0.0, 0.0]);
        d(&mut w, "yspace", 1.5, -20.0, [0.0, s, s]);
        d(&mut w, "zspace", 3.0, 5.0, [0.0, -s, s]);
        let raw: Vec<u8> = stored().iter().flat_map(|v| v.to_le_bytes()).collect();
        let shape: Vec<u64> = SHAPE.iter().map(|&n| n as u64).collect();
        let i = w.dataset(img, "image", Ty::U16, &shape, raw, Storage::Contiguous);
        w.str_attr(i, "dimorder", "zspace,yspace,xspace");
        w.attr(i, "valid_range", AttrValue::F64(vec![0.0, 60000.0]));
        let (lo, hi): (Vec<f64>, Vec<f64>) = if uniform {
            (vec![-1.0; 4], vec![2.0; 4])
        } else {
            (
                (0..4).map(|k| -f64::from(k)).collect(),
                (0..4).map(|k| 10.0 * f64::from(k + 1)).collect(),
            )
        };
        let mn = w.f64s(img, "image-min", &[4], &lo);
        let mx = w.f64s(img, "image-max", &[4], &hi);
        w.str_attr(mn, "dimorder", "zspace");
        w.str_attr(mx, "dimorder", "zspace");
        w.finish()
    }

    fn check_geometry(h: &Header) {
        assert_eq!(h.layout.size, [5, 3, 4]);
        assert_eq!(h.layout.stride, [1, 5, 15]);
        assert_eq!(h.spacing, [2.0, 1.5, 3.0]);
        assert_eq!(h.orientation, OrientationSource::Minc);
        // RAS = cos × (start + index × step); LPS negates x and y.
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let ras = |i: f64, j: f64, k: f64| {
            let (x, y, z) = (10.0 - 2.0 * i, -20.0 + 1.5 * j, 5.0 + 3.0 * k);
            [x, s * y - s * z, s * y + s * z]
        };
        for (i, j, k) in [(0.0, 0.0, 0.0), (4.0, 2.0, 3.0), (1.0, 0.0, 2.0)] {
            let r = ras(i, j, k);
            let p = h.to_patient([i, j, k]);
            for (a, b) in [-r[0], -r[1], r[2]].iter().zip(p) {
                assert!((a - b).abs() < 1e-12, "{r:?} {p:?}");
            }
        }
    }

    #[test]
    fn minc2_slice_scaling() {
        let data = minc2(false);
        assert_eq!(crate::sniff(&data), Some(Kind::Minc2));
        let h = parse(&data, Kind::Minc2).unwrap();
        check_geometry(&h);
        assert_eq!(h.dtype, DataType::F32);
        let v = load(h, &data).unwrap();
        for (n, &x) in stored().iter().enumerate() {
            let (i, j, k) = (n % 5, n / 5 % 3, n / 15);
            let (lo, hi) = (-(k as f64), 10.0 * (k + 1) as f64);
            let want = f64::from(x).min(60000.0) / 60000.0 * (hi - lo) + lo;
            let got = v.value(i, j, k, 0, 0);
            assert!(
                (got - want).abs() < 1e-5 * want.abs().max(1.0),
                "{n}: {got} {want}"
            );
        }
    }

    #[test]
    fn minc2_uniform_scale_keeps_the_stored_type() {
        let data = minc2(true);
        let h = parse(&data, Kind::Minc2).unwrap();
        check_geometry(&h);
        assert_eq!(h.dtype, DataType::U16);
        let (m, b) = h.scale.unwrap();
        assert!((m - 3.0 / 60000.0).abs() < 1e-15 && b == -1.0);
        let v = load(h, &data).unwrap();
        assert!((v.value(4, 2, 3, 0, 0) - (59000.0 * m + b)).abs() < 1e-12);
    }

    // A minimal NetCDF classic (CDF-1) writer for MINC1.
    fn name(b: &mut Vec<u8>, s: &str) {
        b.extend_from_slice(&(s.len() as u32).to_be_bytes());
        b.extend_from_slice(s.as_bytes());
        b.resize(b.len().next_multiple_of(4), 0);
    }

    enum A<'a> {
        Text(&'a str),
        F64(&'a [f64]),
    }

    fn attrs(b: &mut Vec<u8>, list: &[(&str, A)]) {
        if list.is_empty() {
            b.extend_from_slice(&[0; 8]);
            return;
        }
        b.extend_from_slice(&0x0Cu32.to_be_bytes());
        b.extend_from_slice(&(list.len() as u32).to_be_bytes());
        for (n, v) in list {
            name(b, n);
            match v {
                A::Text(t) => {
                    b.extend_from_slice(&2u32.to_be_bytes());
                    name(b, t);
                }
                A::F64(x) => {
                    b.extend_from_slice(&6u32.to_be_bytes());
                    b.extend_from_slice(&(x.len() as u32).to_be_bytes());
                    for v in *x {
                        b.extend_from_slice(&v.to_be_bytes());
                    }
                }
            }
        }
    }

    /// Variables: name, dimension ids, attributes, type, size in bytes.
    type Var<'a> = (&'a str, Vec<u32>, Vec<(&'a str, A<'a>)>, u32, u32);

    /// MINC1 with a signed-byte image marked unsigned and whole-volume scaling.
    fn minc1() -> Vec<u8> {
        let dims = [("zspace", 4u32), ("yspace", 3), ("xspace", 5)];
        let axis = |n: &'static str, step: &'static [f64], start: &'static [f64]| -> Var {
            (
                n,
                vec![],
                vec![("step", A::F64(step)), ("start", A::F64(start))],
                4,
                4,
            )
        };
        let vars: Vec<Var> = vec![
            axis("xspace", &[-2.0], &[10.0]),
            axis("yspace", &[1.5], &[-20.0]),
            axis("zspace", &[3.0], &[5.0]),
            (
                "image",
                vec![0, 1, 2],
                vec![
                    ("signtype", A::Text("unsigned")),
                    ("valid_range", A::F64(&[0.0, 255.0])),
                ],
                1,
                60,
            ),
            ("image-min", vec![], vec![], 6, 8),
            ("image-max", vec![], vec![], 6, 8),
        ];
        let mut b = b"CDF\x01\0\0\0\0".to_vec();
        b.extend_from_slice(&0x0Au32.to_be_bytes());
        b.extend_from_slice(&(dims.len() as u32).to_be_bytes());
        for (n, l) in dims {
            name(&mut b, n);
            b.extend_from_slice(&l.to_be_bytes());
        }
        attrs(&mut b, &[]);
        b.extend_from_slice(&0x0Bu32.to_be_bytes());
        b.extend_from_slice(&(vars.len() as u32).to_be_bytes());
        let mut begins = Vec::new();
        for (n, d, a, t, size) in &vars {
            name(&mut b, n);
            b.extend_from_slice(&(d.len() as u32).to_be_bytes());
            for id in d {
                b.extend_from_slice(&id.to_be_bytes());
            }
            attrs(&mut b, a);
            b.extend_from_slice(&t.to_be_bytes());
            b.extend_from_slice(&size.next_multiple_of(4).to_be_bytes());
            begins.push(b.len());
            b.extend_from_slice(&[0; 4]);
        }
        for (k, (n, _, _, _, size)) in vars.iter().enumerate() {
            let at = b.len() as u32;
            b[begins[k]..begins[k] + 4].copy_from_slice(&at.to_be_bytes());
            match *n {
                "image" => b.extend((0..60u32).map(|i| (i * 4) as u8)),
                "image-min" => b.extend_from_slice(&(-5.0f64).to_be_bytes()),
                "image-max" => b.extend_from_slice(&(250.0f64).to_be_bytes()),
                _ => b.extend_from_slice(&0i32.to_be_bytes()),
            }
            b.resize(at as usize + size.next_multiple_of(4) as usize, 0);
        }
        b
    }

    #[test]
    fn minc1_unsigned_bytes() {
        let data = minc1();
        assert_eq!(crate::sniff(&data), Some(Kind::Minc1));
        let h = parse(&data, Kind::Minc1).unwrap();
        assert_eq!(h.dtype, DataType::U8);
        assert_eq!(h.layout.size, [5, 3, 4]);
        assert_eq!(h.to_patient([1.0, 2.0, 3.0]), [-8.0, 17.0, 14.0]);
        let (m, b) = h.scale.unwrap();
        assert_eq!((m, b), (1.0, -5.0));
        let v = load(h, &data).unwrap();
        // 59 × 4 = 236 is stored as a negative signed byte.
        assert_eq!(v.value(4, 2, 3, 0, 0), 231.0);
    }

    #[test]
    fn other_files_are_not_minc() {
        let mut w = Writer::new();
        w.f32s(ROOT, "x", &[2], &[1.0, 2.0]);
        assert_eq!(sniff(&w.finish()), None);
        assert_eq!(sniff(b"CDF\x01\0\0\0\0"), None);
    }
}
