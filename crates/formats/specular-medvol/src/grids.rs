//! Volumes that other readers parse, as [`MedVolume`]s for MPR and the 3-D view: CCP4 / MRC
//! density maps (`MAP ` at byte 208; also gzipped) and Gaussian cube files, both through
//! specular-em's readers.
//!
//! Their axes are the crystal's or the molecule's, not a patient's: the orientation is
//! [`OrientationSource::Object`], and lengths are Ångström where the volume type says mm
//! (MPR's spacing tolerances are in those units, and a map's voxels are about 1 Å).
//! Maps keep their stored axis order through the layout's strides (MAPC, MAPR, MAPS); a
//! cell with angles other than 90° gives a skewed affine. A cube's orbitals (or values per
//! point) are the volume's further volumes.

use specular_em::{cube, mrc};

use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, MedVolume,
    OrientationSource, Result, Skip,
};

/// Largest gzipped map inflated: 16 GiB, as for other voxel data.
const MAX_INFLATED: usize = crate::data::MAX_DATA;

/// Whether `data` (the file start, inflated when gzipped) is a map or a cube this module reads.
pub fn sniff(data: &[u8]) -> Option<Kind> {
    if mrc::sniff(data) {
        Some(Kind::Map)
    } else if cube::sniff(&data[..data.len().min(16 << 10)]) {
        Some(Kind::Cube)
    } else {
        None
    }
}

fn em_err(e: specular_em::Error) -> Error {
    match e {
        specular_em::Error::TooLarge => Error::TooLarge,
        specular_em::Error::Unsupported(s) => Error::Unsupported(s),
        specular_em::Error::Truncated(s) => Error::Truncated(s.into()),
        e => Error::Malformed(e.to_string()),
    }
}

/// The voxel type of a map's mode (MPR takes real values only).
fn dtype(m: &mrc::Mrc) -> Result<DataType> {
    use specular_em::samples::Sample;
    match m.storage {
        mrc::Storage::Plain(Sample::I8) => Ok(DataType::I8),
        mrc::Storage::Plain(Sample::U8) => Ok(DataType::U8),
        mrc::Storage::Plain(Sample::I16) => Ok(DataType::I16),
        mrc::Storage::Plain(Sample::U16) => Ok(DataType::U16),
        mrc::Storage::Plain(Sample::F32) => Ok(DataType::F32),
        _ => Err(Error::Unsupported(format!(
            "MRC mode {} as a volume",
            m.mode
        ))),
    }
}

/// The voxel (i, j, k = x, y, z) to Å matrix of a unit cell with `angles` (α, β, γ in
/// degrees) and voxel sizes `v`, starting at `origin`: a along x, b in the xy plane (the
/// usual orthogonalisation), so a 90° cell is diagonal.
fn cell_affine(v: [f64; 3], angles: [f64; 3], origin: [f64; 3]) -> [[f64; 4]; 3] {
    let [al, be, ga] = angles.map(|a| {
        if a.is_finite() && a > 0.0 && a < 180.0 {
            a.to_radians()
        } else {
            std::f64::consts::FRAC_PI_2
        }
    });
    // Snap 90° (stored as f32) to exact zeros.
    let c = |x: f64| if x.cos().abs() < 1e-6 { 0.0 } else { x.cos() };
    let (ca, cb, cg) = (c(al), c(be), c(ga));
    let sg = ga.sin();
    let cx = cb;
    let cy = (ca - cb * cg) / sg;
    let cz = (1.0 - cx * cx - cy * cy).max(0.0).sqrt();
    let a = [v[0], 0.0, 0.0];
    let b = [v[1] * cg, v[1] * sg, 0.0];
    let cc = [v[2] * cx, v[2] * cy, v[2] * cz];
    let mut m = [[0.0; 4]; 3];
    for r in 0..3 {
        m[r][0] = a[r];
        m[r][1] = b[r];
        m[r][2] = cc[r];
        m[r][3] = origin[r];
    }
    m
}

/// A map's volume header: stored samples at `offset`, x, y, z through the strides of the
/// stored axis order.
pub fn map_header(m: &mrc::Mrc) -> Result<Header> {
    let dtype = dtype(m)?;
    let size = m.size_xyz();
    let stored = [1, m.nx, m.nx * m.ny];
    let mut stride = [0; 3];
    for k in 0..3 {
        stride[m.axes[k]] = stored[k];
    }
    let samples =
        m.nx.checked_mul(m.ny)
            .and_then(|n| n.checked_mul(m.nz))
            .ok_or(Error::TooLarge)?;
    let voxel = m.voxel.unwrap_or([1.0; 3]);
    let affine = cell_affine(voxel, m.angles.map(f64::from), m.origin_xyz());
    Ok(Header {
        kind: Kind::Map,
        dtype,
        layout: Layout {
            size,
            stride,
            extra: Vec::new(),
            colour: None,
            samples,
        },
        affine,
        orientation: OrientationSource::Object,
        dims: size.to_vec(),
        spacing: voxel,
        scale: None,
        cal: None,
        description: m.labels.first().cloned(),
        intent: None,
        anatomical: None,
        params: Vec::new(),
        data: DataRef {
            file: DataFile::Attached,
            skip: Skip::Start {
                lines: 0,
                bytes: m.data_offset(),
            },
            encoding: Encoding::Raw,
            big_endian: m.order == specular_em::samples::Order::Big,
        },
    })
}

/// A cube's volume header (values decoded to f32, x slowest, z fastest, the values of a
/// point together).
pub fn cube_header(c: &cube::Cube) -> Result<Header> {
    let [nx, ny, nz] = c.size;
    let per = c.per_point.max(1);
    let samples = c.values.len();
    let affine = {
        let mut m = [[0.0; 4]; 3];
        for (r, row) in m.iter_mut().enumerate() {
            for (k, axis) in c.axes.iter().enumerate() {
                row[k] = axis[r];
            }
            row[3] = c.origin[r];
        }
        m
    };
    let len = |a: [f64; 3]| (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    let mut dims = vec![nx, ny, nz];
    if per > 1 {
        dims.push(per);
    }
    Ok(Header {
        kind: Kind::Cube,
        dtype: DataType::F32,
        layout: Layout {
            size: [nx, ny, nz],
            stride: [ny * nz * per, nz * per, per],
            extra: if per > 1 { vec![(per, 1)] } else { Vec::new() },
            colour: None,
            samples,
        },
        affine,
        orientation: OrientationSource::Object,
        dims,
        spacing: c.axes.map(len),
        scale: None,
        cal: None,
        description: Some(c.comments[0].clone()).filter(|s| !s.is_empty()),
        intent: None,
        anatomical: None,
        params: Vec::new(),
        data: DataRef {
            file: DataFile::Attached,
            skip: Skip::Start { lines: 0, bytes: 0 },
            encoding: Encoding::Ascii,
            big_endian: false,
        },
    })
}

/// Whether a file starting with `head` is a map (plain or gzipped) or a cube.
pub fn is_grid(head: &[u8]) -> bool {
    if head.starts_with(&[0x1f, 0x8b]) {
        return specular_png::inflate_gzip_prefix(head, mrc::HEADER).is_ok_and(|h| mrc::sniff(&h));
    }
    sniff(head).is_some()
}

/// Reads a map or cube file (all of it); [`is_grid`] said it is one.
pub fn open(data: Vec<u8>) -> Result<MedVolume> {
    let data = if data.starts_with(&[0x1f, 0x8b]) {
        specular_png::inflate_gzip_members(&data, MAX_INFLATED, None).map_err(|e| match e {
            specular_png::InflateError::NeedsMemory(n) => Error::NeedsMemory(n),
            e => Error::Malformed(format!("gzip: {e}")),
        })?
    } else {
        data
    };
    match sniff(&data) {
        Some(Kind::Map) => {
            let m = mrc::parse(&data).map_err(em_err)?;
            let h = map_header(&m)?;
            let big = h.data.big_endian;
            MedVolume::from_bytes(h, crate::data::counted(data)?, m.data_offset(), big)
        }
        Some(_) => {
            let c = cube::parse(&data).map_err(em_err)?;
            drop(data);
            let h = cube_header(&c)?;
            let (mut bytes, res) = crate::data::alloc(c.values.len().saturating_mul(4))?;
            bytes.extend(c.values.iter().flat_map(|v| v.to_le_bytes()));
            MedVolume::from_bytes(h, crate::data::held(bytes, res), 0, false)
        }
        None => Err(Error::NotRecognised),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The em fixtures (checked there against mrcfile and gemmi) read the same as volumes:
    /// voxel (x, y, z) is section z's value at column x, row y.
    #[test]
    fn maps_in_any_axis_order() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../specular-em/tests/data");
        for name in [
            "zxy.map",
            "yzx-be.map",
            "mode0.ccp4",
            "zyx-u16.map",
            "mode2.mrc",
        ] {
            let data = std::fs::read(dir.join(name)).unwrap();
            assert!(is_grid(&data), "{name}");
            let m = mrc::parse(&data).unwrap();
            let v = open(data.clone()).unwrap();
            let [sx, sy, sz] = m.size_xyz();
            assert_eq!(v.header.layout.size, [sx, sy, sz], "{name}");
            let mut plane = vec![0f32; sx * sy];
            for z in 0..sz {
                m.read_xyz(&data, z, &mut plane).unwrap();
                for y in 0..sy {
                    for x in 0..sx {
                        let want = f64::from(plane[y * sx + x]);
                        assert_eq!(v.value(x, y, z, 0, 0), want, "{name} ({x}, {y}, {z})");
                    }
                }
            }
        }
        // A monoclinic cell (β = 100°): z leans along x.
        let data = std::fs::read(dir.join("zyx-u16.map")).unwrap();
        let a = open(data).unwrap().header.affine;
        assert!(a[0][2] < 0.0 && a[1][2] == 0.0);
    }

    #[test]
    fn cube_orbitals_are_volumes() {
        let t = " a\n b\n -1 0 0 0\n 2 1 0 0\n 1 0 1 0\n 3 0 0 1\n 8 8 0 0 0\n 2 5 6\n \
                 1 -1 2 -2 3 -3\n 4 -4 5 -5 6 -6\n";
        assert!(is_grid(t.as_bytes()));
        let v = open(t.as_bytes().to_vec()).unwrap();
        assert_eq!(v.volumes(), 2);
        assert_eq!(v.header.kind, Kind::Cube);
        // x = 1, z = 2: the sixth point.
        assert_eq!(v.value(1, 0, 2, 0, 0), 6.0);
        assert_eq!(v.value(1, 0, 2, 1, 0), -6.0);
        assert!(!is_grid(b"TITLE \"lut\"\nLUT_3D_SIZE 2\n"));
    }

    #[test]
    fn cells() {
        let a = cell_affine([1.0, 2.0, 3.0], [90.0, 90.0, 90.0], [0.5, 0.0, 0.0]);
        assert_eq!(
            a,
            [
                [1.0, 0.0, 0.0, 0.5],
                [0.0, 2.0, 0.0, 0.0],
                [0.0, 0.0, 3.0, 0.0]
            ]
        );
        // Monoclinic β = 120°: c leans back along x.
        let m = cell_affine([1.0, 1.0, 1.0], [90.0, 120.0, 90.0], [0.0; 3]);
        assert!((m[0][2] + 0.5).abs() < 1e-12 && (m[2][2] - 0.75f64.sqrt()).abs() < 1e-12);
    }
}
