//! TrackVis `.trk` streamlines (versions 1 and 2), from TrackVis's published format.
//!
//! A 1000-byte header (either byte order, told by `hdr_size`): volume dimensions, voxel
//! size, up to ten per-point scalar and ten per-streamline property names, the voxel-to-RAS
//! matrix (version 2), the voxel order ("LPS", "RAS"…) and the streamline count. Each
//! streamline is a point count, the points (x, y, z and the scalars, float32) and the
//! properties.
//!
//! Points are stored in "voxmm" space: voxel coordinates times the voxel size, with (0, 0, 0)
//! at a voxel's corner. They are brought to RAS millimetres here the way nibabel does it:
//! divided by the voxel size, shifted half a voxel, flipped for any axis whose stored voxel
//! order disagrees with the matrix, then through the voxel-to-RAS matrix (the identity when
//! a version 1 file has none).

use crate::{Error, Result, Tractogram};

pub const HEADER: usize = 1000;
/// Most points accepted (each needs 12 bytes in the file anyway).
pub const MAX_POINTS: usize = 1 << 28;

/// The header facts.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub version: i32,
    pub big_endian: bool,
    pub dim: [i16; 3],
    pub voxel_size: [f32; 3],
    pub origin: [f32; 3],
    /// Scalar names, with the count each stands for (a name ending `\0<n>` stands for n).
    pub scalars: Vec<String>,
    pub properties: Vec<String>,
    /// Row-major 4 × 4 (identity when not recorded).
    pub vox_to_ras: [[f64; 4]; 4],
    /// Whether the file recorded `vox_to_ras`.
    pub has_vox_to_ras: bool,
    pub voxel_order: String,
    /// Streamlines the header states (0: not stated).
    pub count: usize,
    /// voxmm to RAS millimetres.
    pub to_ras: [[f64; 4]; 4],
}

/// Whether `data` (at least the first 6 bytes) is a TrackVis file.
pub fn sniff(data: &[u8]) -> bool {
    data.starts_with(b"TRACK\0") || (data.starts_with(b"TRACK") && data.len() < 6)
}

fn rd<const N: usize>(b: &[u8], at: usize, big: bool) -> [u8; N] {
    let mut a = [0u8; N];
    a.copy_from_slice(&b[at..at + N]);
    if big {
        a.reverse();
    }
    a
}

fn i16_at(b: &[u8], at: usize, big: bool) -> i16 {
    i16::from_le_bytes(rd(b, at, big))
}
fn i32_at(b: &[u8], at: usize, big: bool) -> i32 {
    i32::from_le_bytes(rd(b, at, big))
}
fn f32_at(b: &[u8], at: usize, big: bool) -> f32 {
    f32::from_le_bytes(rd(b, at, big))
}

/// Names from a 10 × 20-byte field; a name's trailing `\0<n>` repeats it n times (nibabel's
/// convention for multi-valued entries).
fn names(b: &[u8], n: usize) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for i in 0..10 {
        let f = &b[i * 20..i * 20 + 20];
        let end = f.iter().position(|&c| c == 0).unwrap_or(20);
        let name = String::from_utf8_lossy(&f[..end]).trim().to_owned();
        let rep = if end < 20 {
            let rest = &f[end + 1..];
            let e = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
            std::str::from_utf8(&rest[..e])
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1)
                .max(1)
        } else {
            1
        };
        if out.len() >= n {
            break;
        }
        if name.is_empty() && rep == 1 && out.len() < n {
            out.push(format!("{}", out.len() + 1));
            continue;
        }
        for k in 0..rep.min(n) {
            if out.len() >= n {
                break;
            }
            out.push(if rep > 1 {
                format!("{name}{k}")
            } else {
                name.clone()
            });
        }
    }
    while out.len() < n {
        out.push(format!("{}", out.len() + 1));
    }
    Ok(out)
}

/// Each voxel axis: the RAS axis it follows (0, 1, 2) and its sign.
type Ornt = [(usize, f64); 3];

/// nibabel's `io_orientation`: per input axis, the nearest output axis, strongest first.
fn io_orientation(m: &[[f64; 4]; 4]) -> Option<Ornt> {
    let mut r = [[0.0; 3]; 3];
    for c in 0..3 {
        let n = (0..3).map(|i| m[i][c] * m[i][c]).sum::<f64>().sqrt();
        let n = if n == 0.0 { 1.0 } else { n };
        for i in 0..3 {
            r[i][c] = m[i][c] / n;
        }
    }
    let mut order = [0usize, 1, 2];
    let strength =
        |c: usize, r: &[[f64; 3]; 3]| (0..3).map(|i| r[i][c] * r[i][c]).fold(0.0, f64::max);
    order.sort_by(|&a, &b| strength(b, &r).total_cmp(&strength(a, &r)));
    let mut out = [(usize::MAX, 1.0); 3];
    for &c in &order {
        let col = [r[0][c], r[1][c], r[2][c]];
        if col.iter().all(|v| v.abs() < 1e-8) {
            continue;
        }
        let (o, v) = (0..3)
            .map(|i| (i, col[i]))
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))?;
        if v == 0.0 {
            return None;
        }
        out[c] = (o, if v < 0.0 { -1.0 } else { 1.0 });
        r[o] = [0.0; 3];
    }
    out.iter().all(|o| o.0 < 3).then_some(out)
}

/// Axis codes ("LPS") to an orientation.
fn axcodes(code: &str) -> Option<Ornt> {
    let mut out = [(0, 1.0); 3];
    let mut seen = [false; 3];
    let cs: Vec<char> = code.chars().collect();
    if cs.len() < 3 {
        return None;
    }
    for (i, c) in cs[..3].iter().enumerate() {
        let (a, s) = match c.to_ascii_uppercase() {
            'R' => (0, 1.0),
            'L' => (0, -1.0),
            'A' => (1, 1.0),
            'P' => (1, -1.0),
            'S' => (2, 1.0),
            'I' => (2, -1.0),
            _ => return None,
        };
        if seen[a] {
            return None;
        }
        seen[a] = true;
        out[i] = (a, s);
    }
    Some(out)
}

fn mul(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut o = [[0.0; 4]; 4];
    for (r, row) in o.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    o
}

const EYE: [[f64; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// voxmm to RAS millimetres (nibabel's `get_affine_trackvis_to_rasmm`).
fn to_ras(h: &Header) -> Result<[[f64; 4]; 4]> {
    let mut scale = EYE;
    for (a, row) in scale.iter_mut().enumerate().take(3) {
        row[a] = 1.0 / f64::from(h.voxel_size[a]);
    }
    let mut offset = EYE;
    for row in offset.iter_mut().take(3) {
        row[3] = -0.5;
    }
    let mut aff = mul(&offset, &scale);
    let start = axcodes(&h.voxel_order)
        .ok_or_else(|| Error::Malformed(format!("TrackVis voxel order {:?}", h.voxel_order)))?;
    let end = io_orientation(&h.vox_to_ras)
        .ok_or_else(|| Error::Malformed("TrackVis vox_to_ras".into()))?;
    // ornt_transform(start, end), then inv_ornt_aff with the volume's dimensions.
    let mut ornt = [(0usize, 1.0f64); 3];
    for (ei, &(eo, ef)) in end.iter().enumerate() {
        let si = start
            .iter()
            .position(|s| s.0 == eo)
            .ok_or_else(|| Error::Malformed("TrackVis voxel order".into()))?;
        ornt[si] = (ei, if start[si].1 == ef { 1.0 } else { -1.0 });
    }
    let mut reorder = [[0.0; 4]; 4];
    for (i, o) in ornt.iter().enumerate() {
        reorder[i] = EYE[o.0];
    }
    reorder[3] = EYE[3];
    let mut flip = EYE;
    for i in 0..3 {
        flip[i][i] = ornt[i].1;
        let centre = -(f64::from(h.dim[i]) - 1.0) / 2.0;
        flip[i][3] = ornt[i].1 * centre - centre;
    }
    aff = mul(&mul(&flip, &reorder), &aff);
    Ok(mul(&h.vox_to_ras, &aff))
}

/// Reads the 1000-byte header.
pub fn header(data: &[u8]) -> Result<Header> {
    if !sniff(data) {
        return Err(Error::NotRecognised);
    }
    if data.len() < HEADER {
        return Err(Error::Truncated("TrackVis header".into()));
    }
    let big = match i32_at(data, 996, false) {
        1000 => false,
        _ if i32_at(data, 996, true) == 1000 => true,
        _ => return Err(Error::Malformed("TrackVis header size".into())),
    };
    let version = i32_at(data, 992, big);
    if !(1..=3).contains(&version) {
        return Err(Error::Unsupported(format!(
            "TrackVis version {version} is not supported"
        )));
    }
    let n_s = i16_at(data, 36, big);
    let n_p = i16_at(data, 238, big);
    if !(0..=10 * 20).contains(&n_s) || !(0..=10 * 20).contains(&n_p) {
        return Err(Error::Malformed("TrackVis scalar or property count".into()));
    }
    let mut vox_to_ras = [[0.0; 4]; 4];
    for (r, row) in vox_to_ras.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            *v = f64::from(f32_at(data, 440 + 4 * (r * 4 + c), big));
        }
    }
    let has = version >= 2 && vox_to_ras[3][3] != 0.0;
    if !has {
        vox_to_ras = EYE;
    }
    let voxel_size = [12, 16, 20].map(|at| f32_at(data, at, big));
    if voxel_size.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return Err(Error::Malformed("TrackVis voxel size".into()));
    }
    let mut voxel_order = crate::cstr(&data[948..952]).to_ascii_uppercase();
    if voxel_order.is_empty() {
        voxel_order = "LPS".into();
    }
    let count = i32_at(data, 988, big).max(0) as usize;
    let mut h = Header {
        version,
        big_endian: big,
        dim: [6, 8, 10].map(|at| i16_at(data, at, big)),
        voxel_size,
        origin: [24, 28, 32].map(|at| f32_at(data, at, big)),
        scalars: names(&data[38..238], n_s as usize)?,
        properties: names(&data[240..440], n_p as usize)?,
        vox_to_ras,
        has_vox_to_ras: has,
        voxel_order,
        count,
        to_ras: EYE,
    };
    if vox_to_ras.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error::Malformed("TrackVis vox_to_ras".into()));
    }
    h.to_ras = to_ras(&h)?;
    Ok(h)
}

/// Reads a whole `.trk` file (inflated if it was `.trk.gz`).
pub fn parse(data: &[u8]) -> Result<(Header, Tractogram)> {
    let h = header(data)?;
    let big = h.big_endian;
    let ns = h.scalars.len();
    let np = h.properties.len();
    let body = &data[HEADER..];
    let mut t = Tractogram {
        offsets: vec![0],
        scalars: h.scalars.iter().map(|n| (n.clone(), Vec::new())).collect(),
        properties: h
            .properties
            .iter()
            .map(|n| (n.clone(), Vec::new()))
            .collect(),
        ..Tractogram::default()
    };
    // Bounded by the bytes: at least 12 bytes a point.
    t.points
        .reserve((body.len() / (12 + 4 * ns)).min(MAX_POINTS));
    let m = &h.to_ras;
    let mut at = 0usize;
    while at < body.len() {
        if h.count > 0 && t.streamlines() == h.count {
            break;
        }
        if at + 4 > body.len() {
            return Err(Error::Truncated("TrackVis streamline".into()));
        }
        let n = i32_at(body, at, big);
        at += 4;
        let n = usize::try_from(n).map_err(|_| Error::Malformed("TrackVis point count".into()))?;
        let stride = 4 * (3 + ns);
        let need = n
            .checked_mul(stride)
            .and_then(|x| x.checked_add(4 * np))
            .ok_or(Error::TooLarge)?;
        if at + need > body.len() {
            return Err(Error::Truncated("TrackVis streamline".into()));
        }
        if t.points.len() + n > MAX_POINTS {
            return Err(Error::TooLarge);
        }
        for p in 0..n {
            let b = at + p * stride;
            let v = [0, 4, 8].map(|o| f64::from(f32_at(body, b + o, big)));
            t.points.push(
                [0, 1, 2]
                    .map(|r| (m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2] + m[r][3]) as f32),
            );
            for (s, sc) in t.scalars.iter_mut().enumerate() {
                sc.1.push(f32_at(body, b + 12 + 4 * s, big));
            }
        }
        at += n * stride;
        for (k, pr) in t.properties.iter_mut().enumerate() {
            pr.1.push(f32_at(body, at + 4 * k, big));
        }
        at += 4 * np;
        t.offsets.push(t.points.len() as u32);
    }
    if h.count > 0 && t.streamlines() < h.count {
        return Err(Error::Truncated(format!(
            "TrackVis: {} of {} streamlines",
            t.streamlines(),
            h.count
        )));
    }
    Ok((h, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(order: &[u8; 3], points: &[[f32; 3]]) -> Vec<u8> {
        let mut b = vec![0u8; HEADER];
        b[..6].copy_from_slice(b"TRACK\0");
        for (i, d) in [10i16, 20, 30].iter().enumerate() {
            b[6 + 2 * i..8 + 2 * i].copy_from_slice(&d.to_le_bytes());
        }
        for i in 0..3 {
            b[12 + 4 * i..16 + 4 * i].copy_from_slice(&2f32.to_le_bytes());
        }
        for i in 0..4 {
            b[440 + 4 * (i * 4 + i)..444 + 4 * (i * 4 + i)].copy_from_slice(&1f32.to_le_bytes());
        }
        b[948..951].copy_from_slice(order);
        b[988..992].copy_from_slice(&1i32.to_le_bytes());
        b[992..996].copy_from_slice(&2i32.to_le_bytes());
        b[996..1000].copy_from_slice(&1000i32.to_le_bytes());
        b.extend_from_slice(&(points.len() as i32).to_le_bytes());
        for p in points {
            for v in p {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        b
    }

    #[test]
    fn voxmm_to_ras() {
        let (_, t) = parse(&file(b"RAS", &[[1.0, 1.0, 1.0], [3.0, 5.0, 7.0]])).unwrap();
        assert_eq!(t.points, vec![[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]]);
        // LAS against an RAS matrix: x flips about the volume's centre (10 voxels).
        let (_, t) = parse(&file(b"LAS", &[[1.0, 1.0, 1.0]])).unwrap();
        assert_eq!(t.points, vec![[9.0, 0.0, 0.0]]);
    }

    #[test]
    fn refuses_damage() {
        let mut f = file(b"RAS", &[[1.0, 1.0, 1.0]]);
        f.truncate(f.len() - 2);
        assert!(matches!(parse(&f), Err(Error::Truncated(_))));
        let mut f = file(b"RAS", &[[1.0, 1.0, 1.0]]);
        f[1000..1004].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(parse(&f).is_err());
        let mut f = file(b"RAS", &[]);
        f[996] = 7;
        assert!(parse(&f).is_err());
        assert!(parse(b"TRACK\0").is_err());
    }
}
