//! DNG camera profiles as files (`.dcp`): the DNG colour tags of a profile in a TIFF-like
//! file whose magic is `IIRC` (DNG 1.7.1, "Camera Profiles"). Read, written and applied to
//! a sensor's colour in place of the file's own.
//!
//! - **Read**: the header, IFD 0, then the same tag reader as DNG files (`dng_color`), so a
//!   profile reads the same from a `.dcp` as from a DNG.
//! - **Write**: little-endian, one IFD, tags in order. Matrices as SRATIONAL over 10⁶ (DNG
//!   readers accept any denominator; 10⁴ would round them visibly). Tables as FLOAT. No
//!   tone curve: Specular's view transform owns tone.
//! - **Apply** ([`SensorMeta::apply_dcp`]): the profile's matrices with the photo's own
//!   neutral (DNG `AsShotNeutral`, else 1 / the as-shot multipliers), through the DNG colour
//!   model, replace the sensor's matrix and profile.

use std::sync::Arc;

use crate::container::DngColor;
use crate::profile::{HsvTable, HsvTableData};
use crate::sensor::SensorMeta;
use crate::tiff::{Endian, read_ifd, tag};

/// `II` + `RC` (0x4352): a little-endian camera profile.
const MAGIC_LE: &[u8; 4] = b"IIRC";
/// `MM` + `CR`: big-endian.
const MAGIC_BE: &[u8; 4] = b"MMCR";
/// Largest profile file read (Adobe's largest are about 2 MB).
pub const MAX_BYTES: usize = 32 << 20;

/// Why a profile file was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DcpError {
    /// Not a camera profile (no `IIRC` header).
    NotProfile,
    /// Larger than [`MAX_BYTES`].
    TooLarge,
    /// The header or IFD is cut short or points outside the file.
    Truncated,
    /// No ColorMatrix1: a DNG reader cannot use the profile.
    NoColorMatrix,
    /// A table tag is present but its dimensions or values are unusable.
    BadTable,
    /// The photo's white balance gives no camera neutral to apply the profile with.
    NoNeutral,
    /// The matrices cannot be inverted (all-zero, or degenerate).
    Singular,
}

impl std::fmt::Display for DcpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DcpError::NotProfile => "not a DNG camera profile",
            DcpError::TooLarge => "the profile file is too large",
            DcpError::Truncated => "the profile file is cut short",
            DcpError::NoColorMatrix => "the profile has no colour matrix",
            DcpError::BadTable => "the profile's colour table is damaged",
            DcpError::NoNeutral => "the photo has no white balance to apply the profile with",
            DcpError::Singular => "the profile's matrices are degenerate",
        })
    }
}

impl std::error::Error for DcpError {}

/// One camera profile: what a `.dcp` holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dcp {
    /// `UniqueCameraModel`: the camera the profile is for.
    pub camera_model: String,
    pub name: String,
    pub copyright: String,
    /// `ProfileEmbedPolicy`: 0 allow copying, 1 embed if used, 2 never embed, 3 no
    /// restrictions.
    pub embed_policy: u32,
    /// EXIF light source codes (17 A, 20 D55, 21 D65, 23 D50, ...); 0 for none.
    pub illuminants: [u32; 2],
    /// XYZ → reference camera, per illuminant (row-major).
    pub color_matrix: [Option<[f64; 9]>; 2],
    /// White-balanced camera → XYZ D50, per illuminant.
    pub forward_matrix: [Option<[f64; 9]>; 2],
    /// `ProfileHueSatMapData1/2` (encoding 0).
    pub hue_sat_map: [Option<HsvTableData>; 2],
    /// `ProfileLookTableData` (other makers' profiles; Specular writes none).
    pub look_table: Option<HsvTableData>,
}

impl Dcp {
    /// Reads a profile file.
    pub fn read(d: &[u8]) -> Result<Dcp, DcpError> {
        if d.len() > MAX_BYTES {
            return Err(DcpError::TooLarge);
        }
        let head = d.get(..8).ok_or(DcpError::NotProfile)?;
        let le = match &head[..4] {
            m if m == MAGIC_LE => true,
            m if m == MAGIC_BE => false,
            _ => return Err(DcpError::NotProfile),
        };
        let off = Endian { le }.u32(d, 4).ok_or(DcpError::Truncated)? as usize;
        let ifd = read_ifd(d, off, 0, le).ok_or(DcpError::Truncated)?;
        let c = crate::container::dng_color(d, &ifd, &ifd);
        if c.tables_malformed {
            return Err(DcpError::BadTable);
        }
        if c.cm[0].is_none() {
            return Err(DcpError::NoColorMatrix);
        }
        let text = |t: u16| ifd.string(d, t).unwrap_or_default();
        Ok(Dcp {
            camera_model: text(tag::UNIQUE_CAMERA_MODEL),
            name: text(tag::PROFILE_NAME),
            copyright: text(tag::PROFILE_COPYRIGHT),
            embed_policy: ifd.uint(d, tag::PROFILE_EMBED_POLICY).unwrap_or(0),
            illuminants: c.illuminant,
            color_matrix: c.cm,
            forward_matrix: c.fm,
            hue_sat_map: c.hue_sat_map.map(|t| t.as_ref().map(HsvTable::plain)),
            look_table: c.look_table.as_ref().map(HsvTable::plain),
        })
    }

    /// The profile as a `.dcp` file (little-endian).
    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::default();
        let ascii = |s: &str| {
            let mut b = s.as_bytes().to_vec();
            b.push(0);
            b
        };
        w.add(tag::UNIQUE_CAMERA_MODEL, ASCII, ascii(&self.camera_model));
        for (i, t) in [tag::COLOR_MATRIX1, tag::COLOR_MATRIX2]
            .into_iter()
            .enumerate()
        {
            if let Some(m) = &self.color_matrix[i] {
                w.add(t, SRATIONAL, srationals(m));
            }
        }
        let two = self.color_matrix[1].is_some();
        w.add(
            tag::CALIBRATION_ILLUMINANT1,
            SHORT,
            (self.illuminants[0] as u16).to_le_bytes().to_vec(),
        );
        if two {
            w.add(
                tag::CALIBRATION_ILLUMINANT2,
                SHORT,
                (self.illuminants[1] as u16).to_le_bytes().to_vec(),
            );
        }
        w.add(tag::PROFILE_NAME, ASCII, ascii(&self.name));
        let hsm = self.hue_sat_map[0].as_ref();
        if let Some(t) = hsm {
            w.add(
                tag::PROFILE_HUE_SAT_MAP_DIMS,
                LONG,
                longs(&[t.hue, t.sat, t.val]),
            );
            w.add(tag::PROFILE_HUE_SAT_MAP_DATA1, FLOAT, floats(t));
            if let Some(t2) = self.hue_sat_map[1].as_ref().filter(|_| two) {
                w.add(tag::PROFILE_HUE_SAT_MAP_DATA2, FLOAT, floats(t2));
            }
        }
        w.add(tag::PROFILE_EMBED_POLICY, LONG, longs(&[self.embed_policy]));
        w.add(tag::PROFILE_COPYRIGHT, ASCII, ascii(&self.copyright));
        for (i, t) in [tag::FORWARD_MATRIX1, tag::FORWARD_MATRIX2]
            .into_iter()
            .enumerate()
        {
            if let Some(m) = self.forward_matrix[i].as_ref().filter(|_| i == 0 || two) {
                w.add(t, SRATIONAL, srationals(m));
            }
        }
        if let Some(t) = &self.look_table {
            w.add(
                tag::PROFILE_LOOK_TABLE_DIMS,
                LONG,
                longs(&[t.hue, t.sat, t.val]),
            );
            w.add(tag::PROFILE_LOOK_TABLE_DATA, FLOAT, floats(t));
        }
        if hsm.is_some() {
            w.add(tag::PROFILE_HUE_SAT_MAP_ENCODING, LONG, longs(&[0]));
        }
        w.finish()
    }

    /// The DNG colour tags of this profile with the camera neutral `neutral` (G = 1).
    fn color(&self, neutral: [f64; 3], analog: Option<[f64; 3]>) -> Result<DngColor, DcpError> {
        let table = |t: &Option<HsvTableData>| -> Result<Option<HsvTable>, DcpError> {
            t.as_ref()
                .map(|t| {
                    let data: Vec<f64> = t.data.iter().flatten().map(|&v| f64::from(v)).collect();
                    HsvTable::new(&[t.hue, t.sat, t.val], &data, u32::from(t.srgb))
                        .ok_or(DcpError::BadTable)
                })
                .transpose()
        };
        let adobe = |s: &str| s.to_ascii_lowercase().contains("adobe");
        Ok(DngColor {
            cm: self.color_matrix,
            fm: self.forward_matrix,
            ab: analog,
            illuminant: self.illuminants,
            neutral: Some(neutral),
            hue_sat_map: [table(&self.hue_sat_map[0])?, table(&self.hue_sat_map[1])?],
            look_table: table(&self.look_table)?,
            needs_tables: adobe(&self.name) || adobe(&self.copyright),
            ..Default::default()
        })
    }
}

impl SensorMeta {
    /// Uses camera profile `p` for this sensor's colour instead of its own: the matrix
    /// ([`SensorMeta::rgb_cam`]), its source and the profile with its tables. The white
    /// balance stays the file's.
    pub fn apply_dcp(&mut self, p: &Dcp) -> Result<(), DcpError> {
        let dng = self.dng.as_ref();
        let neutral = match dng.and_then(|d| d.as_shot_neutral) {
            Some(n) => n,
            None => {
                let w = self.wb;
                if !w.iter().all(|v| v.is_finite() && *v > 0.0) {
                    return Err(DcpError::NoNeutral);
                }
                [f64::from(w[1] / w[0]), 1.0, f64::from(w[1] / w[2])]
            }
        };
        let c = p.color(neutral, dng.and_then(|d| d.analog_balance))?;
        let (rgb_cam, source, _wb, profile) = crate::color::dng(&c).ok_or(DcpError::Singular)?;
        self.rgb_cam = rgb_cam;
        self.matrix_source = source;
        self.profile = profile.map(Arc::new);
        Ok(())
    }
}

/// Linear ProPhoto RGB (D50) → XYZ D50, as the DNG colour model here uses it.
pub fn prophoto_to_xyz_d50() -> [[f64; 3]; 3] {
    crate::color::PROPHOTO_TO_XYZD50
}

/// XYZ D50 → linear sRGB / Rec.709 (D65), Bradford, as the DNG colour model here uses it.
pub fn xyz_d50_to_rec709() -> [[f64; 3]; 3] {
    crate::color::XYZD50_TO_SRGB
}

const ASCII: u16 = 2;
const SHORT: u16 = 3;
const LONG: u16 = 4;
const SRATIONAL: u16 = 10;
const FLOAT: u16 = 11;

fn srationals(m: &[f64; 9]) -> Vec<u8> {
    const DEN: i32 = 1_000_000;
    m.iter()
        .flat_map(|&v| {
            let n = (v * f64::from(DEN))
                .round()
                .clamp(f64::from(i32::MIN), f64::from(i32::MAX));
            let mut b = (n as i32).to_le_bytes().to_vec();
            b.extend(DEN.to_le_bytes());
            b
        })
        .collect()
}

fn longs(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn floats(t: &HsvTableData) -> Vec<u8> {
    t.data
        .iter()
        .flatten()
        .flat_map(|x| x.to_le_bytes())
        .collect()
}

/// A one-IFD little-endian TIFF-like writer.
#[derive(Default)]
struct Writer {
    /// (tag, type, value bytes).
    entries: Vec<(u16, u16, Vec<u8>)>,
}

impl Writer {
    fn add(&mut self, tag: u16, typ: u16, value: Vec<u8>) {
        self.entries.push((tag, typ, value));
    }

    fn finish(mut self) -> Vec<u8> {
        self.entries.sort_by_key(|e| e.0);
        let size = |t: u16| match t {
            SHORT => 2,
            LONG | FLOAT => 4,
            SRATIONAL => 8,
            _ => 1,
        };
        let n = self.entries.len();
        let ifd_len = 2 + 12 * n + 4;
        let mut out = Vec::new();
        out.extend(MAGIC_LE);
        out.extend(8u32.to_le_bytes());
        let mut data_at = 8 + ifd_len;
        let mut data = Vec::new();
        out.extend((n as u16).to_le_bytes());
        for (tag, typ, v) in &self.entries {
            out.extend(tag.to_le_bytes());
            out.extend(typ.to_le_bytes());
            out.extend(((v.len() / size(*typ)) as u32).to_le_bytes());
            if v.len() <= 4 {
                let mut b = v.clone();
                b.resize(4, 0);
                out.extend(b);
            } else {
                out.extend((data_at as u32).to_le_bytes());
                data.extend(v);
                // Word-aligned values.
                if data.len() % 2 == 1 {
                    data.push(0);
                }
                data_at = 8 + ifd_len + data.len();
            }
        }
        out.extend(0u32.to_le_bytes());
        out.extend(data);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> Dcp {
        let table = HsvTableData {
            hue: 6,
            sat: 3,
            val: 1,
            srgb: false,
            data: (0..18)
                .map(|i| [(i % 5) as f32 - 2.0, 1.0 + 0.01 * i as f32, 0.98])
                .collect(),
        };
        Dcp {
            camera_model: "Test Camera".into(),
            name: "Chart".into(),
            copyright: "Made with Specular".into(),
            embed_policy: 3,
            illuminants: [21, 0],
            color_matrix: [
                Some([0.9, -0.3, -0.1, -0.4, 1.2, 0.2, -0.05, 0.15, 0.6]),
                None,
            ],
            forward_matrix: [
                Some([0.7, 0.15, 0.1142, 0.28, 0.75, -0.03, 0.02, -0.12, 0.9249]),
                None,
            ],
            hue_sat_map: [Some(table), None],
            look_table: None,
        }
    }

    #[test]
    fn write_then_read_gives_the_same_profile() {
        let p = demo();
        let bytes = p.write();
        assert_eq!(&bytes[..4], b"IIRC");
        let q = Dcp::read(&bytes).unwrap();
        assert_eq!(q.camera_model, p.camera_model);
        assert_eq!(q.name, p.name);
        assert_eq!(q.copyright, p.copyright);
        assert_eq!(q.embed_policy, 3);
        assert_eq!(q.illuminants, [21, 0]);
        assert_eq!(q.hue_sat_map, p.hue_sat_map);
        for (a, b) in q.color_matrix[0]
            .unwrap()
            .iter()
            .zip(&p.color_matrix[0].unwrap())
        {
            assert!((a - b).abs() < 1e-6);
        }
        for (a, b) in q.forward_matrix[0]
            .unwrap()
            .iter()
            .zip(&p.forward_matrix[0].unwrap())
        {
            assert!((a - b).abs() < 1e-6);
        }
        assert_eq!(q.forward_matrix[1], None);
    }

    #[test]
    fn bad_files_are_refused_with_a_reason() {
        assert_eq!(Dcp::read(b"II*\0\x08\0\0\0"), Err(DcpError::NotProfile));
        assert_eq!(Dcp::read(b"IIRC\xff\0\0\0"), Err(DcpError::Truncated));
        let mut p = demo();
        p.color_matrix = [None, None];
        assert_eq!(Dcp::read(&p.write()), Err(DcpError::NoColorMatrix));
        let mut p = demo();
        if let Some(t) = &mut p.hue_sat_map[0] {
            t.data.pop();
        }
        assert_eq!(Dcp::read(&p.write()), Err(DcpError::BadTable));
    }
}
