//! NIfTI-1, NIfTI-2 and Analyze 7.5 headers.
//!
//! One `.nii` file (header, extensions, voxels), the same gzip-compressed, or a `.hdr`
//! header with its voxels in the `.img` beside it. Either byte order, told by `sizeof_hdr`.

use std::path::{Path, PathBuf};

use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, OrientationSource, Result,
    Skip, field_text,
};

const GZIP: [u8; 2] = [0x1f, 0x8b];

/// The header bytes of `data` (inflated when the file is gzip-compressed): at least `n`
/// when the file has them.
fn head(data: &[u8], n: usize) -> Option<std::borrow::Cow<'_, [u8]>> {
    if data.starts_with(&GZIP) {
        specular_png::inflate_gzip_prefix(data, n)
            .ok()
            .map(std::borrow::Cow::Owned)
    } else {
        Some(std::borrow::Cow::Borrowed(data))
    }
}

/// Little-endian `sizeof_hdr` of 348 or 540, either byte order: (NIfTI-2, big endian).
fn size_and_order(b: &[u8]) -> Option<(bool, bool)> {
    let a: [u8; 4] = b.get(..4)?.try_into().ok()?;
    match (i32::from_le_bytes(a), i32::from_be_bytes(a)) {
        (348, _) => Some((false, false)),
        (_, 348) => Some((false, true)),
        (540, _) => Some((true, false)),
        (_, 540) => Some((true, true)),
        _ => None,
    }
}

/// Which of NIfTI-1, NIfTI-2 and Analyze `data` (a file's first bytes, or a whole
/// `.nii.gz`) is.
pub fn sniff(data: &[u8]) -> Option<Kind> {
    let b = head(data, 548)?;
    let (two, big) = size_and_order(&b)?;
    let r = Reader { b: &b, big };
    if two {
        return (b.len() >= 12 && (&b[4..8] == b"n+2\0" || &b[4..8] == b"ni2\0"))
            .then_some(Kind::Nifti2);
    }
    if b.len() < 348 {
        // Only the first bytes: NIfTI-1 needs its magic at 344.
        return None;
    }
    if &b[344..348] == b"n+1\0" || &b[344..348] == b"ni1\0" {
        return Some(Kind::Nifti1);
    }
    // Analyze: no magic; a plausible dim[0] and a known type.
    let nd = r.i16(40);
    let dt = r.i16(70);
    ((1..=7).contains(&nd) && datatype(i32::from(dt)).is_ok()).then_some(Kind::Analyze)
}

struct Reader<'a> {
    b: &'a [u8],
    big: bool,
}

impl Reader<'_> {
    fn bytes<const N: usize>(&self, at: usize) -> [u8; N] {
        let mut a: [u8; N] = self.b[at..at + N].try_into().expect("in header");
        if self.big {
            a.reverse();
        }
        a
    }
    fn i16(&self, at: usize) -> i16 {
        i16::from_le_bytes(self.bytes(at))
    }
    fn i32(&self, at: usize) -> i32 {
        i32::from_le_bytes(self.bytes(at))
    }
    fn i64(&self, at: usize) -> i64 {
        i64::from_le_bytes(self.bytes(at))
    }
    fn f32(&self, at: usize) -> f64 {
        f64::from(f32::from_le_bytes(self.bytes(at)))
    }
    fn f64(&self, at: usize) -> f64 {
        f64::from_le_bytes(self.bytes(at))
    }
}

/// The sample type and colour components of a NIfTI / Analyze datatype code.
fn datatype(code: i32) -> Result<(DataType, usize)> {
    Ok(match code {
        2 => (DataType::U8, 1),
        4 => (DataType::I16, 1),
        8 => (DataType::I32, 1),
        16 => (DataType::F32, 1),
        64 => (DataType::F64, 1),
        128 => (DataType::U8, 3),
        256 => (DataType::I8, 1),
        512 => (DataType::U16, 1),
        768 => (DataType::U32, 1),
        1024 => (DataType::I64, 1),
        1280 => (DataType::U64, 1),
        2304 => (DataType::U8, 4),
        32 | 1792 | 2048 => {
            return Err(Error::Unsupported(
                "complex-valued voxels cannot be shown (no magnitude or phase is chosen)".into(),
            ));
        }
        1536 => return Err(Error::Unsupported("128-bit float voxels".into())),
        1 => return Err(Error::Unsupported("1-bit voxels".into())),
        c => return Err(Error::Malformed(format!("datatype {c}"))),
    })
}

/// Fields read from either header version.
struct Fields {
    dim: [i64; 8],
    pixdim: [f64; 8],
    datatype: i32,
    vox_offset: f64,
    slope: f64,
    inter: f64,
    cal_max: f64,
    cal_min: f64,
    glmax: f64,
    glmin: f64,
    descrip: Option<String>,
    qform_code: i32,
    sform_code: i32,
    quatern: [f64; 3],
    qoffset: [f64; 3],
    srow: [[f64; 4]; 3],
    intent_code: i32,
    intent_name: Option<String>,
    originator: [i16; 3],
}

fn fields1(r: &Reader<'_>) -> Fields {
    Fields {
        dim: std::array::from_fn(|i| i64::from(r.i16(40 + 2 * i))),
        pixdim: std::array::from_fn(|i| r.f32(76 + 4 * i)),
        datatype: i32::from(r.i16(70)),
        vox_offset: r.f32(108),
        slope: r.f32(112),
        inter: r.f32(116),
        cal_max: r.f32(124),
        cal_min: r.f32(128),
        glmax: f64::from(r.i32(140)),
        glmin: f64::from(r.i32(144)),
        descrip: field_text(&r.b[148..228]),
        qform_code: i32::from(r.i16(252)),
        sform_code: i32::from(r.i16(254)),
        quatern: [r.f32(256), r.f32(260), r.f32(264)],
        qoffset: [r.f32(268), r.f32(272), r.f32(276)],
        srow: std::array::from_fn(|row| std::array::from_fn(|c| r.f32(280 + 16 * row + 4 * c))),
        intent_code: i32::from(r.i16(68)),
        intent_name: field_text(&r.b[328..344]),
        originator: [r.i16(253), r.i16(255), r.i16(257)],
    }
}

fn fields2(r: &Reader<'_>) -> Fields {
    Fields {
        dim: std::array::from_fn(|i| r.i64(16 + 8 * i)),
        pixdim: std::array::from_fn(|i| r.f64(104 + 8 * i)),
        datatype: i32::from(r.i16(12)),
        vox_offset: r.i64(168) as f64,
        slope: r.f64(176),
        inter: r.f64(184),
        cal_max: r.f64(192),
        cal_min: r.f64(200),
        glmax: 0.0,
        glmin: 0.0,
        descrip: field_text(&r.b[240..320]),
        qform_code: r.i32(344),
        sform_code: r.i32(348),
        quatern: [r.f64(352), r.f64(360), r.f64(368)],
        qoffset: [r.f64(376), r.f64(384), r.f64(392)],
        srow: std::array::from_fn(|row| std::array::from_fn(|c| r.f64(400 + 32 * row + 8 * c))),
        intent_code: r.i32(504),
        intent_name: field_text(&r.b[508..524]),
        originator: [0; 3],
    }
}

/// Parses the header of the NIfTI or Analyze file at `path` (its bytes, or at least the
/// header's, are `data`).
pub fn parse(data: &[u8], path: &Path) -> Result<Header> {
    let gz = data.starts_with(&GZIP);
    let b = head(data, 548).ok_or_else(|| Error::Malformed("gzip stream".into()))?;
    let (two, big) = size_and_order(&b).ok_or(Error::NotRecognised)?;
    let need = if two { 540 } else { 348 };
    if b.len() < need {
        return Err(Error::Truncated("header".into()));
    }
    let r = Reader { b: &b, big };
    let (kind, single, f) = if two {
        let single = match &b[4..8] {
            b"n+2\0" => true,
            b"ni2\0" => false,
            _ => return Err(Error::NotRecognised),
        };
        (Kind::Nifti2, single, fields2(&r))
    } else {
        match &b[344..348] {
            b"n+1\0" => (Kind::Nifti1, true, fields1(&r)),
            b"ni1\0" => (Kind::Nifti1, false, fields1(&r)),
            _ => (Kind::Analyze, false, fields1(&r)),
        }
    };
    let (dtype, comps) = datatype(f.datatype)?;
    let nd = f.dim[0];
    if !(1..=7).contains(&nd) {
        return Err(Error::Malformed(format!("dim[0] = {nd}")));
    }
    let nd = nd as usize;
    let mut dims = Vec::with_capacity(nd);
    for a in 1..=nd {
        let n = f.dim[a];
        if n < 1 {
            return Err(Error::Malformed(format!("dim[{a}] = {n}")));
        }
        dims.push(usize::try_from(n).map_err(|_| Error::TooLarge)?);
    }
    let size = [0, 1, 2].map(|a| dims.get(a).copied().unwrap_or(1));
    let extra: Vec<usize> = dims.iter().skip(3).copied().collect();
    let mut layout = Layout::dense(size, &extra, comps).ok_or(Error::TooLarge)?;
    if comps > 1 {
        layout.colour = Some((comps, 1));
    }
    let zoom = |a: usize| {
        let z = f.pixdim[a + 1];
        if a < nd && z.is_finite() { z } else { 1.0 }
    };

    // Voxel to RAS, then to LPS.
    let (ras, orientation) = if kind == Kind::Analyze {
        (analyze_affine(&f, size, zoom), OrientationSource::Analyze)
    } else if f.sform_code != 0 {
        (f.srow, OrientationSource::Sform)
    } else if f.qform_code != 0 {
        (qform(&f, zoom), OrientationSource::Qform)
    } else {
        (base_affine(size, zoom), OrientationSource::None)
    };
    let mut affine = ras;
    for x in affine[..2].iter_mut().flatten() {
        *x = -*x;
    }
    if affine.iter().flatten().any(|x| !x.is_finite()) {
        return Err(Error::Malformed("orientation".into()));
    }
    let spacing = [0, 1, 2].map(|c| {
        let n = (affine[0][c].powi(2) + affine[1][c].powi(2) + affine[2][c].powi(2)).sqrt();
        if n > 0.0 { n } else { 1.0 }
    });

    let scale = if comps > 1 {
        None
    } else if kind == Kind::Analyze {
        analyze_scale(&f)
    } else if f.slope != 0.0 && f.slope.is_finite() {
        let inter = if f.inter.is_finite() { f.inter } else { 0.0 };
        Some((f.slope, inter)).filter(|&s| s != (1.0, 0.0))
    } else {
        None
    };
    let cal = (kind != Kind::Analyze
        && comps == 1
        && f.cal_min.is_finite()
        && f.cal_max.is_finite()
        && f.cal_max > f.cal_min)
        .then_some((f.cal_min, f.cal_max));

    let vox = f.vox_offset;
    if !(vox.is_finite() && vox >= 0.0) {
        return Err(Error::Malformed(format!("vox_offset {vox}")));
    }
    let mut vox = vox as usize;
    if single {
        vox = vox.max(if two { 544 } else { 352 });
    }
    let data_ref = if single {
        DataRef {
            file: DataFile::Attached,
            skip: Skip::Start {
                lines: 0,
                bytes: vox,
            },
            encoding: if gz { Encoding::Gzip } else { Encoding::Raw },
            big_endian: big,
        }
    } else {
        let img = image_file(path);
        let gz = img
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("gz"));
        DataRef {
            file: DataFile::Detached(vec![img]),
            skip: Skip::Start {
                lines: 0,
                bytes: vox,
            },
            encoding: if gz { Encoding::Gzip } else { Encoding::Raw },
            big_endian: big,
        }
    };

    Ok(Header {
        kind,
        dtype,
        layout,
        affine,
        orientation,
        dims,
        spacing,
        scale,
        cal,
        description: f.descrip.clone(),
        intent: intent(f.intent_code, f.intent_name.as_deref()),
        anatomical: None,
        params: Vec::new(),
        data: data_ref,
    })
}

/// The `.img` beside a `.hdr` (either case, possibly gzip-compressed); the plain `.img`
/// when none exists.
fn image_file(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    let (stem, gz) = if let Some(s) = lower.strip_suffix(".hdr.gz") {
        (&name[..s.len()], true)
    } else if let Some(s) = lower.strip_suffix(".hdr") {
        (&name[..s.len()], false)
    } else {
        (name.as_str(), false)
    };
    let mut tries: Vec<String> = vec![
        format!("{stem}.img"),
        format!("{stem}.IMG"),
        format!("{stem}.img.gz"),
        format!("{stem}.IMG.gz"),
    ];
    if gz {
        tries.rotate_left(2);
    }
    let dir = path.parent().unwrap_or(Path::new(""));
    tries
        .iter()
        .map(|t| dir.join(t))
        .find(|p| p.is_file())
        .unwrap_or_else(|| dir.join(&tries[0]))
}

/// nibabel's affine for a header with neither form: voxel sizes, x flipped, the volume
/// centred on the origin.
fn base_affine(size: [usize; 3], zoom: impl Fn(usize) -> f64) -> [[f64; 4]; 3] {
    let mut a = [[0.0; 4]; 3];
    for r in 0..3 {
        let z = if r == 0 { -zoom(r) } else { zoom(r) };
        a[r][r] = z;
        a[r][3] = -(size[r] as f64 - 1.0) / 2.0 * z;
    }
    a
}

/// SPM's Analyze affine: voxel sizes with x flipped, the origin at `originator` (1-based)
/// when it lies near the volume, else at its centre.
fn analyze_affine(f: &Fields, size: [usize; 3], zoom: impl Fn(usize) -> f64) -> [[f64; 4]; 3] {
    let o = f.originator.map(f64::from);
    let d = size.map(|n| n as f64);
    let inside = o.iter().any(|&x| x != 0.0) && (0..3).all(|i| o[i] > -d[i] && o[i] < 2.0 * d[i]);
    let origin: [f64; 3] = if inside {
        o.map(|x| x - 1.0)
    } else {
        d.map(|n| (n - 1.0) / 2.0)
    };
    let mut a = [[0.0; 4]; 3];
    for r in 0..3 {
        let z = if r == 0 { -zoom(r) } else { zoom(r) };
        a[r][r] = z;
        a[r][3] = -origin[r] * z;
    }
    a
}

/// SPM2's scaling: the slope in `funused1`, else from the cal and gl ranges.
fn analyze_scale(f: &Fields) -> Option<(f64, f64)> {
    if f.slope != 0.0 && f.slope.is_finite() {
        return Some((f.slope, 0.0)).filter(|&s| s != (1.0, 0.0));
    }
    let unscaled = f.glmax - f.glmin;
    let scaled = f.cal_max - f.cal_min;
    if unscaled != 0.0 && scaled != 0.0 && scaled.is_finite() {
        let slope = scaled / unscaled;
        Some((slope, f.cal_min - slope * f.glmin))
    } else {
        None
    }
}

/// The qform: a rotation from the quaternion (b, c, d), voxel sizes (z times `qfac`), and
/// the offset.
fn qform(f: &Fields, zoom: impl Fn(usize) -> f64) -> [[f64; 4]; 3] {
    let [b, c, d] = f.quatern;
    let a2 = 1.0 - (b * b + c * c + d * d);
    let a = if a2 > 0.0 { a2.sqrt() } else { 0.0 };
    let n = a * a + b * b + c * c + d * d;
    let s = if n > 0.0 { 2.0 / n } else { 0.0 };
    let (x, y, z) = (b * s, c * s, d * s);
    let (wx, wy, wz) = (a * x, a * y, a * z);
    let (xx, xy, xz) = (b * x, b * y, b * z);
    let (yy, yz, zz) = (c * y, c * z, d * z);
    let r = [
        [1.0 - (yy + zz), xy - wz, xz + wy],
        [xy + wz, 1.0 - (xx + zz), yz - wx],
        [xz - wy, yz + wx, 1.0 - (xx + yy)],
    ];
    let qfac = if f.pixdim[0] < 0.0 { -1.0 } else { 1.0 };
    let z = [zoom(0).abs(), zoom(1).abs(), zoom(2).abs() * qfac];
    let mut m = [[0.0; 4]; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = r[i][j] * z[j];
        }
        m[i][3] = f.qoffset[i];
    }
    m
}

/// The intent, for the info panel: its name (the code's) and the file's own label.
fn intent(code: i32, name: Option<&str>) -> Option<String> {
    let known = match code {
        0 => None,
        2 => Some("correlation"),
        3 => Some("t-test"),
        4 => Some("F-test"),
        5 => Some("z-score"),
        6 => Some("chi-squared"),
        7 => Some("beta"),
        8 => Some("binomial"),
        9 => Some("gamma"),
        10 => Some("Poisson"),
        11 => Some("normal"),
        20 => Some("log10 p-value"),
        21 => Some("p-value"),
        22 => Some("ln p-value"),
        1001 => Some("estimate"),
        1002 => Some("label"),
        1003 => Some("NeuroNames label"),
        1004 => Some("matrix"),
        1005 => Some("symmetric matrix"),
        1006 => Some("displacement vector"),
        1007 => Some("vector"),
        1008 => Some("point set"),
        1009 => Some("triangle"),
        1010 => Some("quaternion"),
        1011 => Some("dimensionless"),
        2001 => Some("time series"),
        2002 => Some("node index"),
        2003 => Some("RGB vector"),
        2004 => Some("RGBA vector"),
        2005 => Some("shape"),
        _ => Some("other"),
    };
    let code_text = known.map(|k| {
        if k == "other" {
            format!("intent {code}")
        } else {
            k.to_owned()
        }
    });
    match (code_text, name) {
        (Some(c), Some(n)) => Some(format!("{c} ({n})")),
        (Some(c), None) => Some(c),
        (None, Some(n)) => Some(n.to_owned()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_quaternion_is_diagonal() {
        let mut f = fields1(&Reader {
            b: &[0u8; 348],
            big: false,
        });
        f.qoffset = [1.0, 2.0, 3.0];
        f.pixdim[0] = -1.0;
        let m = qform(&f, |a| [2.0, 3.0, 4.0][a]);
        assert_eq!(m[0], [2.0, 0.0, 0.0, 1.0]);
        assert_eq!(m[1], [0.0, 3.0, 0.0, 2.0]);
        assert_eq!(m[2], [0.0, 0.0, -4.0, 3.0]);
    }

    #[test]
    fn complex_refused() {
        assert!(matches!(datatype(32), Err(Error::Unsupported(_))));
        assert!(matches!(datatype(1792), Err(Error::Unsupported(_))));
    }
}
