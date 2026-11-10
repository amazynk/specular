//! FreeSurfer MGH (`.mgh`) and its gzip-compressed form (`.mgz`), from the format's
//! published description.
//!
//! A 284-byte big-endian header: version (1), width, height, depth, frames, type, degrees
//! of freedom, `goodRASflag`, then (meaningful when the flag is set) the voxel sizes, the
//! three direction cosines x, y, z in RAS and the RAS position of the volume's centre. The
//! voxels follow at byte 284, width fastest, then height, depth and frames. After them,
//! optionally: TR, flip angle (radians), TE, TI and field of view as floats, then tagged
//! blocks (command lines, the Talairach transform's file, the phase-encoding direction, the
//! field strength…).

use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, OrientationSource, Param,
    ParamValue, Result, Skip,
};

/// Bytes before the voxels.
pub const HEADER: usize = 284;

/// Bytes of the header that carry fields.
const FIELDS: usize = 90;

const GZIP: [u8; 2] = [0x1f, 0x8b];

/// Largest axis accepted (FreeSurfer volumes are a few hundred voxels; surfaces stored as
/// volumes put up to a few hundred thousand vertices along one axis).
const MAX_AXIS: i32 = 1 << 24;

fn i32_at(b: &[u8], at: usize) -> i32 {
    i32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_bits(i32_at(b, at) as u32)
}

/// The first `n` bytes of the (inflated) file.
fn head(data: &[u8], n: usize) -> Option<std::borrow::Cow<'_, [u8]>> {
    if data.starts_with(&GZIP) {
        specular_png::inflate_gzip_prefix(data, n)
            .ok()
            .map(std::borrow::Cow::Owned)
    } else {
        Some(std::borrow::Cow::Borrowed(data))
    }
}

fn dtype(code: i32) -> Option<DataType> {
    Some(match code {
        0 => DataType::U8,
        1 => DataType::I32,
        3 => DataType::F32,
        4 => DataType::I16,
        _ => return None,
    })
}

/// The type's name in FreeSurfer, for refusals.
fn type_name(code: i32) -> String {
    match code {
        2 => "long".into(),
        5 => "bitmap".into(),
        6 => "tensor".into(),
        c => format!("type {c}"),
    }
}

/// Voxel sizes, direction cosines (columns x, y, z) and centre.
type Geometry = ([f64; 3], [[f64; 3]; 3], [f64; 3]);

/// The header's [`Geometry`], when every value is finite.
fn geometry(b: &[u8]) -> Option<Geometry> {
    let f = |at: usize| f64::from(f32_at(b, at));
    let delta = [f(30), f(34), f(38)];
    let mdc = [0, 1, 2].map(|c| [0, 1, 2].map(|r| f(42 + 12 * c + 4 * r)));
    let c_ras = [f(78), f(82), f(86)];
    let all = delta.iter().chain(mdc.iter().flatten()).chain(c_ras.iter());
    if !all.clone().all(|x| x.is_finite()) {
        return None;
    }
    Some((delta, mdc, c_ras))
}

fn plausible(b: &[u8]) -> bool {
    if b.len() < FIELDS || i32_at(b, 0) != 1 {
        return false;
    }
    let dims = [4, 8, 12, 16].map(|at| i32_at(b, at));
    if !dims.iter().all(|d| (1..=MAX_AXIS).contains(d)) || dtype(i32_at(b, 20)).is_none() {
        return false;
    }
    let good = i16::from_be_bytes([b[28], b[29]]);
    if !(0..=1).contains(&good) {
        return false;
    }
    let Some((delta, mdc, _)) = geometry(b) else {
        return false;
    };
    let unset = delta.iter().chain(mdc.iter().flatten()).all(|&x| x == 0.0);
    if good == 0 && unset {
        return true;
    }
    let norm = |v: [f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    delta.iter().all(|&d| d > 0.0 && d < 1e4) && mdc.iter().all(|&c| (norm(c) - 1.0).abs() < 1e-2)
}

/// Whether `data` (a file's first bytes, or a whole `.mgz`) is an MGH volume: version 1,
/// plausible sizes, a known type and, when set, unit direction cosines.
pub fn sniff(data: &[u8]) -> bool {
    head(data, FIELDS).is_some_and(|b| plausible(&b))
}

/// Parses the header of an MGH file (all of it, or at least the header; a `.mgz` whole).
pub fn parse(data: &[u8]) -> Result<Header> {
    let gz = data.starts_with(&GZIP);
    let b = head(data, FIELDS).ok_or_else(|| Error::Malformed("gzip stream".into()))?;
    if b.len() < FIELDS {
        return Err(Error::Truncated("header".into()));
    }
    if !plausible(&b) {
        let code = i32_at(&b, 20);
        if i32_at(&b, 0) == 1 && dtype(code).is_none() {
            return Err(Error::Unsupported(format!(
                "MGH {} voxels are not supported",
                type_name(code)
            )));
        }
        return Err(Error::NotRecognised);
    }
    let [w, h, d, frames] = [4, 8, 12, 16].map(|at| i32_at(&b, at) as usize);
    let dtype = dtype(i32_at(&b, 20)).ok_or(Error::NotRecognised)?;
    let good = i16::from_be_bytes([b[28], b[29]]) == 1;
    let (delta, mdc, c_ras, orientation) = match geometry(&b) {
        Some((delta, mdc, c)) if good => (delta, mdc, c, OrientationSource::Vox2Ras),
        // FreeSurfer's default: 1 mm coronal slices (x_r = -1, y_s = -1, z_a = 1), centred
        // on the origin.
        _ => (
            [1.0; 3],
            [[-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
            [0.0; 3],
            OrientationSource::MghDefault,
        ),
    };
    let affine = vox2ras_lps([w, h, d], delta, mdc, c_ras);
    let extra: &[usize] = if frames > 1 { &[frames] } else { &[] };
    let layout =
        Layout::dense([w, h, d], extra, 1).ok_or_else(|| Error::Malformed("volume size".into()))?;
    let mut dims = vec![w, h, d];
    if frames > 1 {
        dims.push(frames);
    }
    let mut header = Header {
        kind: Kind::Mgh,
        dtype,
        layout,
        affine,
        orientation,
        dims,
        spacing: delta,
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
                bytes: HEADER,
            },
            encoding: if gz { Encoding::Gzip } else { Encoding::Raw },
            big_endian: true,
        },
    };
    // An uncompressed file's footer is at hand; a `.mgz`'s is read with the voxels.
    if !gz && let Some(n) = header.data_bytes() {
        header.params = footer(data.get(HEADER.saturating_add(n)..).unwrap_or(&[]));
    }
    Ok(header)
}

/// FreeSurfer's vox2ras (`Mdc · diag(delta)`, placed so voxel `dims / 2` is at the centre
/// `c_ras`) in LPS.
pub fn vox2ras_lps(
    dims: [usize; 3],
    delta: [f64; 3],
    mdc: [[f64; 3]; 3],
    c_ras: [f64; 3],
) -> [[f64; 4]; 3] {
    let mut a = [[0.0; 4]; 3];
    for r in 0..3 {
        for c in 0..3 {
            a[r][c] = mdc[c][r] * delta[c];
        }
        a[r][3] = c_ras[r] - (0..3).map(|c| a[r][c] * dims[c] as f64 / 2.0).sum::<f64>();
    }
    // RAS to LPS: x and y change sign.
    for row in &mut a[..2] {
        for v in row.iter_mut() {
            *v = -*v;
        }
    }
    a
}

/// Tags whose length is a 32-bit count (older files); the others use 64 bits.
const TAG_OLD_MGH_XFORM: i32 = 30;
/// Old tags written without a length: nothing after them can be found.
const TAGS_UNSIZED: [i32; 3] = [1, 2, 20];

/// The scan parameters and the tags after the voxels (`rest`: the file from the end of the
/// voxel data). Unknown tags are skipped; anything malformed ends the list.
pub fn footer(rest: &[u8]) -> Vec<Param> {
    let mut out = Vec::new();
    let num = |key, v: f32, unit| Param {
        key,
        value: ParamValue::Number(f64::from(v), unit),
    };
    let params: [(&str, f32, &str); 5] = [
        ("tr", 1.0, "ms"),
        ("flip", 180.0 / std::f32::consts::PI, "°"),
        ("te", 1.0, "ms"),
        ("ti", 1.0, "ms"),
        ("fov", 1.0, "mm"),
    ];
    for (i, (key, scale, unit)) in params.into_iter().enumerate() {
        if rest.len() < 4 * (i + 1) {
            return out;
        }
        let v = f32_at(rest, 4 * i);
        if v.is_finite() && v != 0.0 {
            out.push(num(key, v * scale, unit));
        }
    }
    let mut at = 20;
    while at + 8 <= rest.len() {
        let tag = i32_at(rest, at);
        if TAGS_UNSIZED.contains(&tag) {
            break;
        }
        let (len, body) = if tag == TAG_OLD_MGH_XFORM {
            (i32_at(rest, at + 4) as i64, at + 8)
        } else if at + 12 <= rest.len() {
            let l = i64::from_be_bytes(rest[at + 4..at + 12].try_into().unwrap_or([0; 8]));
            (l, at + 12)
        } else {
            break;
        };
        let Some(data) = usize::try_from(len)
            .ok()
            .and_then(|n| rest.get(body..body.checked_add(n)?))
        else {
            break;
        };
        match tag {
            3 => push_text(&mut out, "cmdline", data),
            30 | 31 => push_text(&mut out, "xform", data),
            41 => push_text(&mut out, "pedir", data),
            43 => {
                if let Some(t) = field_strength(data) {
                    out.push(Param {
                        key: "field",
                        value: ParamValue::Number(t, "T"),
                    });
                }
            }
            _ => {}
        }
        at = body + data.len();
    }
    out
}

fn push_text(out: &mut Vec<Param>, key: &'static str, data: &[u8]) {
    let Some(t) = crate::field_text(data) else {
        return;
    };
    // Several command lines (the volume's history) make one entry, a line each.
    if let Some(Param {
        value: ParamValue::Text(prev),
        ..
    }) = out.iter_mut().find(|p| p.key == key)
    {
        prev.push('\n');
        prev.push_str(&t);
    } else {
        out.push(Param {
            key,
            value: ParamValue::Text(t),
        });
    }
}

/// A field strength in tesla, stored as a float or double in either byte order (FreeSurfer
/// writes the tag's bytes as the machine holds them).
fn field_strength(b: &[u8]) -> Option<f64> {
    let tries: Vec<f64> = match b.len() {
        4 => {
            let a: [u8; 4] = b.try_into().ok()?;
            vec![
                f64::from(f32::from_be_bytes(a)),
                f64::from(f32::from_le_bytes(a)),
            ]
        }
        8 => {
            let a: [u8; 8] = b.try_into().ok()?;
            vec![f64::from_be_bytes(a), f64::from_le_bytes(a)]
        }
        _ => return None,
    };
    tries.into_iter().find(|t| (0.01..=30.0).contains(t))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// An MGH file: `dims` (w, h, d, frames), type code, voxel sizes, cosines (x, y, z),
    /// centre, the voxel bytes, then `tail`.
    pub(crate) fn mgh(
        dims: [i32; 4],
        code: i32,
        good: bool,
        geom: ([f32; 3], [[f32; 3]; 3], [f32; 3]),
        voxels: &[u8],
        tail: &[u8],
    ) -> Vec<u8> {
        let mut b = Vec::with_capacity(HEADER + voxels.len() + tail.len());
        b.extend(1i32.to_be_bytes());
        for d in dims {
            b.extend(d.to_be_bytes());
        }
        b.extend(code.to_be_bytes());
        b.extend(0i32.to_be_bytes());
        b.extend(i16::from(good).to_be_bytes());
        let (delta, mdc, c) = geom;
        for v in delta.iter().chain(mdc.iter().flatten()).chain(c.iter()) {
            b.extend(v.to_be_bytes());
        }
        b.resize(HEADER, 0);
        b.extend_from_slice(voxels);
        b.extend_from_slice(tail);
        b
    }

    pub(crate) const LIA: ([f32; 3], [[f32; 3]; 3], [f32; 3]) = (
        [1.0, 1.5, 2.0],
        [[-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
        [10.0, -20.0, 30.0],
    );

    #[test]
    fn header_and_affine() {
        let vox: Vec<u8> = (0..4 * 3 * 2 * 2).map(|i| i as u8).collect();
        let f = mgh([4, 3, 2, 1], 4, true, LIA, &vox, &[]);
        assert!(sniff(&f));
        assert!(!sniff(&f[..60]));
        let h = parse(&f).unwrap();
        assert_eq!(h.kind, Kind::Mgh);
        assert_eq!(h.dtype, DataType::I16);
        assert_eq!(h.dims, vec![4, 3, 2]);
        assert_eq!(h.spacing, [1.0, 1.5, 2.0]);
        assert_eq!(h.orientation, OrientationSource::Vox2Ras);
        // RAS: i → -x (1 mm), j → -z (1.5 mm), k → +y (2 mm); centre voxel (2, 1.5, 1).
        // c_ras = P0 + M·(2, 1.5, 1): P0 = (10 + 2, -20 - 2, 30 + 2.25) in RAS.
        let ras_p0 = [12.0, -22.0, 32.25];
        assert_eq!(h.affine[0], [1.0, 0.0, 0.0, -ras_p0[0]]);
        assert_eq!(h.affine[1], [0.0, 0.0, -2.0, -ras_p0[1]]);
        assert_eq!(h.affine[2], [0.0, -1.5, 0.0, ras_p0[2]]);
        assert!(h.params.is_empty());
    }

    #[test]
    fn refusals_and_defaults() {
        let f = mgh([2, 2, 2, 1], 6, true, LIA, &[0; 64], &[]);
        assert!(!sniff(&f));
        assert!(matches!(parse(&f), Err(Error::Unsupported(_))));
        let mut bad = mgh([2, 2, 2, 1], 0, true, LIA, &[0; 8], &[]);
        bad[3] = 2;
        assert!(!sniff(&bad));
        let unset = ([0.0; 3], [[0.0; 3]; 3], [0.0; 3]);
        let f = mgh([2, 2, 2, 3], 0, false, unset, &[0; 24], &[]);
        let h = parse(&f).unwrap();
        assert_eq!(h.orientation, OrientationSource::MghDefault);
        assert!(!h.orientation.reliable());
        assert_eq!(h.layout.volumes(), 3);
        assert_eq!(h.dims, vec![2, 2, 2, 3]);
    }

    #[test]
    fn footer_params_and_tags() {
        let mut tail = Vec::new();
        for v in [2300.0f32, 0.15707963, 2.98, 900.0, 256.0] {
            tail.extend(v.to_be_bytes());
        }
        let tag = |t: &mut Vec<u8>, code: i32, body: &[u8]| {
            t.extend(code.to_be_bytes());
            t.extend((body.len() as i64).to_be_bytes());
            t.extend_from_slice(body);
        };
        tag(&mut tail, 3, b"mri_convert a.nii b.mgz\0");
        tag(&mut tail, 99, &[1, 2, 3]);
        tag(&mut tail, 3, b"mri_vol2vol\0");
        tag(&mut tail, 43, &3.0f32.to_le_bytes());
        tag(&mut tail, 41, b"LR\0");
        tail.extend(TAG_OLD_MGH_XFORM.to_be_bytes());
        tail.extend(12i32.to_be_bytes());
        tail.extend_from_slice(b"talairach.x\0");
        let f = mgh([1, 1, 1, 1], 0, true, LIA, &[9], &tail);
        let h = parse(&f).unwrap();
        let get = |k: &str| {
            h.params
                .iter()
                .find(|p| p.key == k)
                .map(|p| p.value.clone())
        };
        assert_eq!(get("tr"), Some(ParamValue::Number(2300.0, "ms")));
        match get("flip") {
            Some(ParamValue::Number(d, "°")) => assert!((d - 9.0).abs() < 1e-4),
            v => panic!("{v:?}"),
        }
        assert_eq!(get("ti"), Some(ParamValue::Number(900.0, "ms")));
        assert_eq!(
            get("cmdline"),
            Some(ParamValue::Text(
                "mri_convert a.nii b.mgz\nmri_vol2vol".into()
            ))
        );
        assert_eq!(get("field"), Some(ParamValue::Number(3.0, "T")));
        assert_eq!(get("pedir"), Some(ParamValue::Text("LR".into())));
        assert_eq!(get("xform"), Some(ParamValue::Text("talairach.x".into())));
        // A truncated tag ends the list without failing.
        let g = mgh([1, 1, 1, 1], 0, true, LIA, &[9], &tail[..30]);
        assert_eq!(parse(&g).unwrap().params.len(), 5);
    }
}
