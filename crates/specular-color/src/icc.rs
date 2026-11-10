//! ICC v2/v4 profile parsing: header, matrix/TRC tags, gray TRC and the LUT-based
//! `A2B*` / `B2A*` tags (`mft1`, `mft2`, `mAB `, `mBA `). Every read is bounds checked;
//! malformed data yields an [`Error`], never a panic.

use crate::curve::Curve;
use crate::lut::{Clut, Pipeline, Stage};
use crate::math::{D50, Mat3};

/// Why a profile was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Shorter than a header, or a tag points outside the data.
    Truncated,
    /// Missing `acsp` signature.
    NotIcc,
    /// A tag or field has an impossible value.
    Malformed(&'static str),
    /// Well formed, but not something a viewer can use (device links, named colour…).
    Unsupported(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Truncated => f.write_str("truncated ICC profile"),
            Error::NotIcc => f.write_str("not an ICC profile"),
            Error::Malformed(w) => write!(f, "malformed ICC profile: {w}"),
            Error::Unsupported(w) => write!(f, "unsupported ICC profile: {w}"),
        }
    }
}

impl std::error::Error for Error {}

/// The device (data) colour space of a profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorSpace {
    /// RGB.
    Rgb,
    /// Single-channel gray.
    Gray,
    /// Four-colour print: values are ink amounts (1.0 = 100 % ink).
    Cmyk,
    /// Three-colour print.
    Cmy,
    /// CIE L*a*b*.
    Lab,
    /// CIE XYZ.
    Xyz,
    /// Any other n-channel space (n colour, YCbCr…): parsed but not converted.
    Other(u8),
}

impl ColorSpace {
    fn from_sig(s: &[u8]) -> ColorSpace {
        match s {
            b"RGB " => ColorSpace::Rgb,
            b"GRAY" => ColorSpace::Gray,
            b"CMYK" => ColorSpace::Cmyk,
            b"CMY " => ColorSpace::Cmy,
            b"Lab " => ColorSpace::Lab,
            b"XYZ " => ColorSpace::Xyz,
            b"YCbr" | b"Yxy " | b"Luv " | b"HSV " | b"HLS " => ColorSpace::Other(3),
            [d @ b'2'..=b'9', b'C', b'L', b'R'] => ColorSpace::Other(d - b'0'),
            [d @ b'A'..=b'F', b'C', b'L', b'R'] => ColorSpace::Other(d - b'A' + 10),
            _ => ColorSpace::Other(0),
        }
    }

    /// Channel count.
    pub fn channels(self) -> usize {
        match self {
            ColorSpace::Gray => 1,
            ColorSpace::Cmyk => 4,
            ColorSpace::Rgb | ColorSpace::Cmy | ColorSpace::Lab | ColorSpace::Xyz => 3,
            ColorSpace::Other(n) => n as usize,
        }
    }
}

/// How a LUT pipeline encodes the profile connection space in its normalized values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PcsEncoding {
    /// XYZ, u1Fixed15: 1.0 normalized = 65535 / 32768.
    Xyz,
    /// Legacy 16-bit L*a*b* (`lut16Type`): L 100 = 0xFF00, a/b 0 = 0x8000.
    LabV2,
    /// v4 L*a*b* (`lutAtoBType`, `lut8Type`): L 100 = 1.0, a/b 0 = 128/255.
    LabV4,
}

impl PcsEncoding {
    /// Normalized PCS values → XYZ (D50).
    #[inline]
    pub fn decode(self, v: [f32; 3]) -> [f32; 3] {
        const X: f32 = 65535.0 / 32768.0;
        match self {
            PcsEncoding::Xyz => [v[0] * X, v[1] * X, v[2] * X],
            PcsEncoding::LabV2 => {
                const L: f32 = 100.0 * 65535.0 / 65280.0;
                const A: f32 = 65535.0 / 256.0;
                crate::math::lab_to_xyz([v[0] * L, v[1] * A - 128.0, v[2] * A - 128.0])
            }
            PcsEncoding::LabV4 => {
                crate::math::lab_to_xyz([v[0] * 100.0, v[1] * 255.0 - 128.0, v[2] * 255.0 - 128.0])
            }
        }
    }

    /// XYZ (D50) → normalized PCS values.
    #[inline]
    pub fn encode(self, xyz: [f32; 3]) -> [f32; 3] {
        const X: f32 = 32768.0 / 65535.0;
        match self {
            PcsEncoding::Xyz => [xyz[0] * X, xyz[1] * X, xyz[2] * X],
            PcsEncoding::LabV2 => {
                let l = crate::math::xyz_to_lab(xyz);
                const L: f32 = 65280.0 / 65535.0 / 100.0;
                const A: f32 = 256.0 / 65535.0;
                [l[0] * L, (l[1] + 128.0) * A, (l[2] + 128.0) * A]
            }
            PcsEncoding::LabV4 => {
                let l = crate::math::xyz_to_lab(xyz);
                [l[0] / 100.0, (l[1] + 128.0) / 255.0, (l[2] + 128.0) / 255.0]
            }
        }
    }
}

/// A LUT-based tag: the pipeline plus how its PCS side is encoded.
#[derive(Clone, Debug)]
pub struct Lut {
    /// The processing pipeline (device → PCS for `A2B`, PCS → device for `B2A`).
    pub pipe: Pipeline,
    /// PCS encoding on the PCS side of the pipeline.
    pub pcs: PcsEncoding,
}

/// A parsed profile: the parts a colour transform needs.
#[derive(Clone, Debug)]
pub struct Profile {
    /// Major version (2 or 4).
    pub version: u8,
    /// Profile/device class signature (`mntr`, `scnr`, `prtr`, `spac`…).
    pub class: [u8; 4],
    /// Device colour space.
    pub color_space: ColorSpace,
    /// True when the PCS is L*a*b* (otherwise XYZ).
    pub pcs_lab: bool,
    /// Media white point (XYZ), D50 when absent.
    pub white: [f32; 3],
    /// RGB colorants as XYZ(D50) columns.
    pub matrix: Option<Mat3>,
    /// RGB tone curves (device → linear).
    pub trc: Option<[Curve; 3]>,
    /// Gray tone curve (device → Y).
    pub gray_trc: Option<Curve>,
    /// `A2B0`, `A2B1`, `A2B2` (perceptual, colorimetric, saturation).
    pub a2b: [Option<Lut>; 3],
    /// `B2A0`, `B2A1`, `B2A2`.
    pub b2a: [Option<Lut>; 3],
    /// Built-in analytic device model (the generic CMYK profile), in place of tags.
    pub(crate) model: Option<crate::profiles::Model>,
    /// Content hash (FNV-1a of the bytes, or a fixed id for built-in profiles).
    pub hash: u64,
}

/// Maximum accepted profile size.
const MAX_SIZE: usize = 64 << 20;
/// Maximum CLUT entries (grid points × outputs).
const MAX_CLUT: usize = 1 << 24;

struct Rd<'a>(&'a [u8]);

impl<'a> Rd<'a> {
    fn slice(&self, off: usize, len: usize) -> Result<&'a [u8], Error> {
        self.0
            .get(off..off.checked_add(len).ok_or(Error::Truncated)?)
            .ok_or(Error::Truncated)
    }
    fn u8(&self, off: usize) -> Result<u8, Error> {
        self.0.get(off).copied().ok_or(Error::Truncated)
    }
    fn u16(&self, off: usize) -> Result<u16, Error> {
        let s = self.slice(off, 2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    fn u32(&self, off: usize) -> Result<u32, Error> {
        let s = self.slice(off, 4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn s15(&self, off: usize) -> Result<f32, Error> {
        Ok(self.u32(off)? as i32 as f32 / 65536.0)
    }
    fn sig(&self, off: usize) -> Result<&'a [u8], Error> {
        self.slice(off, 4)
    }
}

/// The profile's description (`desc`: v2 ASCII, v4 `mluc` English record or the first, or
/// `text`), whitespace collapsed; `None` when absent, damaged or empty.
pub fn description(icc: &[u8]) -> Option<String> {
    let rd = Rd(icc);
    let count = (rd.u32(128).ok()? as usize).min(1024);
    let tag = (0..count).find_map(|i| {
        let e = 132 + i * 12;
        if rd.sig(e).ok()? != b"desc" {
            return None;
        }
        let off = rd.u32(e + 4).ok()? as usize;
        let len = rd.u32(e + 8).ok()? as usize;
        rd.slice(off, len).ok()
    })?;
    let t = Rd(tag);
    let text: String = match t.sig(0).ok()? {
        b"desc" => {
            let n = (t.u32(8).ok()? as usize).min(tag.len());
            let b = t.slice(12, n).ok()?;
            b.iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as char)
                .collect()
        }
        b"text" => tag[8.min(tag.len())..]
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect(),
        b"mluc" => {
            let records = (t.u32(8).ok()? as usize).min(64);
            let size = t.u32(12).ok()? as usize;
            let record = |i: usize| {
                let r = 16 + i * size;
                Some((t.slice(r, 2).ok()?, t.u32(r + 4).ok()?, t.u32(r + 8).ok()?))
            };
            let pick = (0..records)
                .find(|&i| record(i).is_some_and(|r| r.0 == b"en"))
                .unwrap_or(0);
            let (_, len, off) = record(pick)?;
            let b = t.slice(off as usize, len as usize).ok()?;
            let units: Vec<u16> = b
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .take_while(|&u| u != 0)
                .collect();
            String::from_utf16_lossy(&units)
        }
        _ => return None,
    };
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

/// FNV-1a over the bytes.
pub fn hash_bytes(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

impl Profile {
    /// Parses an ICC profile.
    pub fn parse(data: &[u8]) -> Result<Profile, Error> {
        if data.len() < 132 {
            return Err(Error::Truncated);
        }
        if data.len() > MAX_SIZE {
            return Err(Error::Unsupported("profile too large"));
        }
        let r = Rd(data);
        if r.sig(36)? != b"acsp" {
            return Err(Error::NotIcc);
        }
        let declared = r.u32(0)? as usize;
        // Trust the shorter of declared and actual length; tags must fit inside.
        let len = if declared >= 132 {
            declared.min(data.len())
        } else {
            data.len()
        };
        let r = Rd(&data[..len]);
        let version = r.u8(8)?;
        let mut class = [0u8; 4];
        class.copy_from_slice(r.sig(12)?);
        let color_space = ColorSpace::from_sig(r.sig(16)?);
        let pcs_lab = match r.sig(20)? {
            b"Lab " => true,
            b"XYZ " => false,
            _ if &class == b"link" => false,
            _ => return Err(Error::Malformed("PCS")),
        };
        match &class {
            b"link" => return Err(Error::Unsupported("device link")),
            b"nmcl" => return Err(Error::Unsupported("named colour")),
            _ => {}
        }
        let count = r.u32(128)? as usize;
        if count > 1024 || 132 + count * 12 > len {
            return Err(Error::Malformed("tag count"));
        }
        let mut tags: Vec<(&[u8], &[u8])> = Vec::with_capacity(count);
        for i in 0..count {
            let e = 132 + i * 12;
            let sig = r.sig(e)?;
            let off = r.u32(e + 4)? as usize;
            let size = r.u32(e + 8)? as usize;
            // Skip tags that point outside the profile instead of rejecting it all.
            if let Ok(body) = r.slice(off, size) {
                tags.push((sig, body));
            }
        }
        let tag = |s: &[u8; 4]| tags.iter().find(|(t, _)| *t == s).map(|(_, b)| *b);

        let mut p = Profile {
            version,
            class,
            color_space,
            pcs_lab,
            white: D50,
            matrix: None,
            trc: None,
            gray_trc: None,
            a2b: [None, None, None],
            b2a: [None, None, None],
            model: None,
            hash: hash_bytes(data),
        };
        if let Some(w) = tag(b"wtpt").and_then(|b| read_xyz(b).ok())
            && w[1] > 0.0
            && w.iter().all(|v| v.is_finite() && *v >= 0.0)
        {
            p.white = w;
        }
        if color_space == ColorSpace::Rgb {
            let cols = [tag(b"rXYZ"), tag(b"gXYZ"), tag(b"bXYZ")];
            let trcs = [tag(b"rTRC"), tag(b"gTRC"), tag(b"bTRC")];
            if let ([Some(r), Some(g), Some(b)], [Some(tr), Some(tg), Some(tb)]) = (cols, trcs) {
                let c = (read_xyz(r), read_xyz(g), read_xyz(b));
                let t = (read_curve(tr), read_curve(tg), read_curve(tb));
                if let ((Ok(r), Ok(g), Ok(b)), (Ok(tr), Ok(tg), Ok(tb))) = (c, t) {
                    let f = |v: [f32; 3]| [v[0] as f64, v[1] as f64, v[2] as f64];
                    let m = Mat3::from_cols(f(r), f(g), f(b));
                    if m.inverse().is_some() {
                        p.matrix = Some(m);
                        p.trc = Some([tr.0, tg.0, tb.0]);
                    }
                }
            }
        }
        if color_space == ColorSpace::Gray {
            p.gray_trc = tag(b"kTRC").and_then(|b| read_curve(b).ok()).map(|c| c.0);
        }
        let n = color_space.channels();
        for (i, (a, b)) in [(b"A2B0", b"B2A0"), (b"A2B1", b"B2A1"), (b"A2B2", b"B2A2")]
            .iter()
            .enumerate()
        {
            if n > 0 {
                p.a2b[i] = tag(a).and_then(|d| read_lut(d, n, 3, pcs_lab, false).ok());
                p.b2a[i] = tag(b).and_then(|d| read_lut(d, 3, n, pcs_lab, true).ok());
            }
        }
        if p.a2b.iter().all(Option::is_none) && p.matrix.is_none() && p.gray_trc.is_none() {
            return Err(Error::Unsupported("no usable device → PCS tags"));
        }
        Ok(p)
    }

    /// True for an RGB profile described by colorants and tone curves.
    pub fn is_matrix_shaper(&self) -> bool {
        self.matrix.is_some() && self.trc.is_some()
    }

    /// Picks the device → PCS LUT for an intent index (0 perceptual, 1 colorimetric,
    /// 2 saturation), falling back to `A2B0`.
    pub fn a2b_for(&self, intent: usize) -> Option<&Lut> {
        self.a2b[intent.min(2)].as_ref().or(self.a2b[0].as_ref())
    }

    /// Picks the PCS → device LUT for an intent index, falling back to `B2A0`.
    pub fn b2a_for(&self, intent: usize) -> Option<&Lut> {
        self.b2a[intent.min(2)].as_ref().or(self.b2a[0].as_ref())
    }
}

fn read_xyz(b: &[u8]) -> Result<[f32; 3], Error> {
    let r = Rd(b);
    if r.sig(0)? != b"XYZ " {
        return Err(Error::Malformed("XYZ tag type"));
    }
    Ok([r.s15(8)?, r.s15(12)?, r.s15(16)?])
}

/// Reads a `curv` or `para` element; returns the curve and its size in bytes (unpadded).
pub(crate) fn read_curve(b: &[u8]) -> Result<(Curve, usize), Error> {
    let r = Rd(b);
    match r.sig(0)? {
        b"curv" => {
            let n = r.u32(8)? as usize;
            if n > 65536 {
                return Err(Error::Malformed("curve length"));
            }
            let c = match n {
                0 => Curve::Identity,
                1 => Curve::Gamma(r.u16(12)? as f32 / 256.0),
                _ => {
                    let s = r.slice(12, n * 2)?;
                    Curve::Table(
                        s.chunks_exact(2)
                            .map(|c| u16::from_be_bytes([c[0], c[1]]) as f32 / 65535.0)
                            .collect(),
                    )
                }
            };
            Ok((c, 12 + n * 2))
        }
        b"para" => {
            let k = r.u16(8)?;
            let np = match k {
                0 => 1,
                1 => 3,
                2 => 4,
                3 => 5,
                4 => 7,
                _ => return Err(Error::Malformed("parametric curve type")),
            };
            let mut p = [0f32; 7];
            for (i, v) in p.iter_mut().take(np).enumerate() {
                *v = r.s15(12 + i * 4)?;
            }
            if p.iter().any(|v| !v.is_finite()) {
                return Err(Error::Malformed("parametric curve"));
            }
            Ok((
                if k == 0 {
                    Curve::Gamma(p[0])
                } else {
                    Curve::Param(k, p)
                },
                12 + np * 4,
            ))
        }
        _ => Err(Error::Malformed("curve type")),
    }
}

fn read_curves(b: &[u8], off: usize, n: usize) -> Result<Vec<Curve>, Error> {
    let mut out = Vec::with_capacity(n);
    let mut o = off;
    for _ in 0..n {
        let (c, len) = read_curve(b.get(o..).ok_or(Error::Truncated)?)?;
        out.push(c);
        o += (len + 3) & !3;
    }
    Ok(out)
}

fn clut_size(grid: &[usize], outputs: usize) -> Result<usize, Error> {
    let mut n = outputs;
    for &g in grid {
        if g < 2 {
            return Err(Error::Malformed("CLUT grid"));
        }
        n = n
            .checked_mul(g)
            .filter(|&n| n <= MAX_CLUT)
            .ok_or(Error::Malformed("CLUT size"))?;
    }
    Ok(n)
}

/// Parses an `mft1`/`mft2`/`mAB `/`mBA ` tag. `pcs_in` says the PCS is on the input side
/// (B2A direction); `pcs_lab` gives the PCS colour space.
fn read_lut(
    b: &[u8],
    inputs: usize,
    outputs: usize,
    pcs_lab: bool,
    pcs_in: bool,
) -> Result<Lut, Error> {
    let r = Rd(b);
    let ty = r.sig(0)?;
    let (ni, no) = (r.u8(8)? as usize, r.u8(9)? as usize);
    if ni != inputs || no != outputs || ni == 0 || no == 0 || ni > 15 || no > 15 {
        return Err(Error::Malformed("LUT channel count"));
    }
    match ty {
        b"mft1" | b"mft2" => {
            let wide = ty == b"mft2";
            let g = r.u8(10)? as usize;
            let mut m = [0f32; 9];
            for (i, v) in m.iter_mut().enumerate() {
                *v = r.s15(12 + i * 4)?;
            }
            let (nin, nout, mut o, width) = if wide {
                (r.u16(48)? as usize, r.u16(50)? as usize, 52, 2)
            } else {
                (256, 256, 48, 1)
            };
            if !(2..=4096).contains(&nin) || !(2..=4096).contains(&nout) {
                return Err(Error::Malformed("LUT table length"));
            }
            let read_vals = |off: usize, count: usize| -> Result<Vec<f32>, Error> {
                let s = r.slice(off, count.checked_mul(width).ok_or(Error::Truncated)?)?;
                Ok(if wide {
                    s.chunks_exact(2)
                        .map(|c| u16::from_be_bytes([c[0], c[1]]) as f32 / 65535.0)
                        .collect()
                } else {
                    s.iter().map(|&v| v as f32 / 255.0).collect()
                })
            };
            let mut stages = Vec::new();
            // The matrix only applies when the input is XYZ.
            let xyz_in = pcs_in && !pcs_lab;
            let ident = m
                .iter()
                .enumerate()
                .all(|(i, v)| (v - if i % 4 == 0 { 1.0 } else { 0.0 }).abs() < 1e-6);
            if xyz_in && ni == 3 && !ident {
                stages.push(Stage::Matrix(m, [0.0; 3]));
            }
            let mut cin = Vec::with_capacity(ni);
            for _ in 0..ni {
                cin.push(Curve::Table(read_vals(o, nin)?));
                o += nin * width;
            }
            stages.push(Stage::Curves(cin));
            let grid = vec![g; ni];
            let size = clut_size(&grid, no)?;
            let data = read_vals(o, size)?;
            o += size * width;
            stages.push(Stage::Clut(
                Clut::new(grid, no, data).ok_or(Error::Malformed("CLUT"))?,
            ));
            let mut cout = Vec::with_capacity(no);
            for _ in 0..no {
                cout.push(Curve::Table(read_vals(o, nout)?));
                o += nout * width;
            }
            stages.push(Stage::Curves(cout));
            let pcs = if !pcs_lab {
                PcsEncoding::Xyz
            } else if wide {
                PcsEncoding::LabV2
            } else {
                PcsEncoding::LabV4
            };
            Ok(Lut {
                pipe: Pipeline {
                    inputs,
                    outputs,
                    stages,
                },
                pcs,
            })
        }
        b"mAB " | b"mBA " => {
            let a2b = ty == b"mAB ";
            let ob = r.u32(12)? as usize;
            let om = r.u32(16)? as usize;
            let ocm = r.u32(20)? as usize;
            let oclut = r.u32(24)? as usize;
            let oa = r.u32(28)? as usize;
            // Channel counts at each element: A on the device side, B on the PCS side.
            let (dev, pcs) = if a2b { (ni, no) } else { (no, ni) };
            let b_curves = if ob != 0 {
                Some(read_curves(b, ob, pcs)?)
            } else {
                None
            };
            let matrix = if om != 0 && pcs == 3 {
                let mut m = [0f32; 9];
                let mut off = [0f32; 3];
                for (i, v) in m.iter_mut().enumerate() {
                    *v = r.s15(om + i * 4)?;
                }
                for (i, v) in off.iter_mut().enumerate() {
                    *v = r.s15(om + 36 + i * 4)?;
                }
                Some(Stage::Matrix(m, off))
            } else {
                None
            };
            let m_curves = if ocm != 0 && pcs == 3 {
                Some(read_curves(b, ocm, pcs)?)
            } else {
                None
            };
            let a_curves = if oa != 0 {
                Some(read_curves(b, oa, dev)?)
            } else {
                None
            };
            let clut = if oclut != 0 {
                let gs = r.slice(oclut, 16)?;
                let grid: Vec<usize> = gs[..ni].iter().map(|&g| g as usize).collect();
                let prec = r.u8(oclut + 16)?;
                let size = clut_size(&grid, no)?;
                let data: Vec<f32> = match prec {
                    1 => r
                        .slice(oclut + 20, size)?
                        .iter()
                        .map(|&v| v as f32 / 255.0)
                        .collect(),
                    2 => r
                        .slice(oclut + 20, size * 2)?
                        .chunks_exact(2)
                        .map(|c| u16::from_be_bytes([c[0], c[1]]) as f32 / 65535.0)
                        .collect(),
                    _ => return Err(Error::Malformed("CLUT precision")),
                };
                Some(Clut::new(grid, no, data).ok_or(Error::Malformed("CLUT"))?)
            } else {
                if ni != no {
                    return Err(Error::Malformed("LUT without CLUT changes channel count"));
                }
                None
            };
            if b_curves.is_none() {
                return Err(Error::Malformed("missing B curves"));
            }
            let mut stages = Vec::new();
            let push_curves = |stages: &mut Vec<Stage>, c: Option<Vec<Curve>>| {
                if let Some(c) = c
                    && !c.iter().all(Curve::is_identity)
                {
                    stages.push(Stage::Curves(c));
                }
            };
            if a2b {
                push_curves(&mut stages, a_curves);
                if let Some(c) = clut {
                    stages.push(Stage::Clut(c));
                }
                push_curves(&mut stages, m_curves);
                if let Some(m) = matrix {
                    stages.push(m);
                }
                push_curves(&mut stages, b_curves);
            } else {
                push_curves(&mut stages, b_curves);
                if let Some(m) = matrix {
                    stages.push(m);
                }
                push_curves(&mut stages, m_curves);
                if let Some(c) = clut {
                    stages.push(Stage::Clut(c));
                }
                push_curves(&mut stages, a_curves);
            }
            let pcs = if pcs_lab {
                PcsEncoding::LabV4
            } else {
                PcsEncoding::Xyz
            };
            Ok(Lut {
                pipe: Pipeline {
                    inputs,
                    outputs,
                    stages,
                },
                pcs,
            })
        }
        _ => Err(Error::Unsupported("LUT tag type")),
    }
}

#[cfg(test)]
mod description_tests {
    use super::description;

    /// A profile header and one `desc` tag holding `body`.
    fn with_desc(body: &[u8]) -> Vec<u8> {
        let mut p = vec![0u8; 128];
        p.extend_from_slice(&1u32.to_be_bytes());
        p.extend_from_slice(b"desc");
        p.extend_from_slice(&144u32.to_be_bytes());
        p.extend_from_slice(&(body.len() as u32).to_be_bytes());
        p.extend_from_slice(body);
        p
    }

    #[test]
    fn v2_v4_and_damaged_descriptions() {
        let mut v2 = b"desc\0\0\0\0".to_vec();
        v2.extend_from_slice(&12u32.to_be_bytes());
        v2.extend_from_slice(b"Glossy  Pap\0");
        assert_eq!(description(&with_desc(&v2)).as_deref(), Some("Glossy Pap"));
        let mut v4 = b"mluc\0\0\0\0".to_vec();
        v4.extend_from_slice(&2u32.to_be_bytes());
        v4.extend_from_slice(&12u32.to_be_bytes());
        for (lang, off) in [(b"de", 40u32), (b"en", 44)] {
            v4.extend_from_slice(lang);
            v4.extend_from_slice(b"DE");
            v4.extend_from_slice(&4u32.to_be_bytes());
            v4.extend_from_slice(&off.to_be_bytes());
        }
        v4.extend_from_slice(&[0, b'X', 0, b'Y', 0, b'O', 0, b'K']);
        assert_eq!(description(&with_desc(&v4)).as_deref(), Some("OK"));
        // Lengths past the end, no tag, junk: none, never a panic.
        assert!(description(&with_desc(&v2[..10])).is_none());
        assert!(description(&[0u8; 100]).is_none());
        assert!(description(&with_desc(b"curv\0\0\0\0")).is_none());
    }
}
