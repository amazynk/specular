//! Sequence, GOP and picture headers and their extensions (§6.2.2, §6.2.3, MPEG-1 §2.4.2).

use crate::bits::Bits;
use crate::tables::{DEFAULT_INTRA, ZIGZAG};
use crate::{Chroma, Error, malformed};

/// Start codes.
pub(crate) const PICTURE: u8 = 0x00;
pub(crate) const SLICE_LAST: u8 = 0xAF;
pub(crate) const SEQUENCE: u8 = 0xB3;
pub(crate) const EXTENSION: u8 = 0xB5;
pub(crate) const SEQUENCE_END: u8 = 0xB7;
pub(crate) const GOP: u8 = 0xB8;

/// What the sequence header and its extensions say.
#[derive(Clone, Debug, PartialEq)]
pub struct Sequence {
    pub width: u32,
    pub height: u32,
    /// MPEG-2 (a sequence extension follows the header); else MPEG-1.
    pub mpeg2: bool,
    /// MPEG-2 `profile_and_level_indication`.
    pub profile_level: u8,
    pub progressive: bool,
    pub chroma: Chroma,
    /// aspect_ratio_information (MPEG-2: display aspect; MPEG-1: pixel aspect code).
    pub aspect_code: u8,
    pub frame_rate_code: u8,
    pub frame_rate_ext: (u8, u8),
    pub low_delay: bool,
    /// Sequence display extension: colour description (primaries, transfer, matrix).
    pub colour: Option<(u8, u8, u8)>,
    /// Sequence display extension: the intended display size.
    pub display: Option<(u32, u32)>,
    pub bit_rate: u32,
    /// Quantiser matrices in raster order: intra, non-intra, chroma intra, chroma non-intra.
    pub(crate) quant: [[u8; 64]; 4],
    /// A sequence scalable extension was present (refused).
    pub(crate) scalable: bool,
}

impl Sequence {
    /// Macroblocks across and down (frame macroblock rows).
    pub fn mb_size(&self) -> (u32, u32) {
        let w = self.width.div_ceil(16);
        let h = if self.mpeg2 && !self.progressive {
            2 * self.height.div_ceil(32)
        } else {
            self.height.div_ceil(16)
        };
        (w, h)
    }

    /// Frames per second (Table 6-4 with the extension's factor).
    pub fn frame_rate(&self) -> Option<f64> {
        let base = match self.frame_rate_code {
            1 => 24000.0 / 1001.0,
            2 => 24.0,
            3 => 25.0,
            4 => 30000.0 / 1001.0,
            5 => 30.0,
            6 => 50.0,
            7 => 60000.0 / 1001.0,
            8 => 60.0,
            _ => return None,
        };
        let (n, d) = self.frame_rate_ext;
        Some(base * f64::from(n + 1) / f64::from(d + 1))
    }

    /// The pixel aspect ratio (horizontal : vertical spacing), when not square.
    pub fn pixel_aspect(&self) -> Option<(u32, u32)> {
        if !self.mpeg2 {
            // MPEG-1 Table 2-D.2.2: height/width of a pixel ×10000.
            let h_over_w: u32 = match self.aspect_code {
                2 => 6735,
                3 => 7031,
                4 => 7615,
                5 => 8055,
                6 => 8437,
                7 => 8935,
                8 => 9157,
                9 => 9815,
                10 => 10255,
                11 => 10695,
                12 => 10950,
                13 => 11575,
                14 => 12015,
                _ => return None,
            };
            return Some((10000, h_over_w));
        }
        let (dw, dh) = self.display.unwrap_or((self.width, self.height));
        let (num, den): (u64, u64) = match self.aspect_code {
            2 => (4, 3),
            3 => (16, 9),
            4 => (221, 100),
            _ => return None,
        };
        // DAR = (display_w * par_h_spacing) / (display_h * par_v_spacing)...: the pixel's
        // width over height is DAR × display_h / display_w.
        let h = num * u64::from(dh.max(1));
        let v = den * u64::from(dw.max(1));
        let g = gcd(h, v);
        let (h, v) = (h / g, v / g);
        (h != v).then_some((
            h.min(u64::from(u32::MAX)) as u32,
            v.min(u64::from(u32::MAX)) as u32,
        ))
    }

    /// Why this decoder won't decode the sequence, in words (`None`: it will).
    pub fn refusal(&self) -> Option<String> {
        if self.scalable {
            return Some("scalable MPEG-2 video".into());
        }
        if self.height > 2800 || self.width > 8192 {
            return Some(format!("MPEG-2 video of {}×{}", self.width, self.height));
        }
        None
    }

    /// "Main Profile @ High Level", "4:2:2 Profile @ Main Level", or for MPEG-1 `None`.
    pub fn profile_name(&self) -> Option<String> {
        if !self.mpeg2 {
            return None;
        }
        let pl = self.profile_level;
        if pl & 0x80 != 0 {
            return Some(
                match pl {
                    0x82 => "4:2:2 Profile @ High Level",
                    0x85 => "4:2:2 Profile @ Main Level",
                    0x8A..=0x8E => "Multi-view Profile",
                    _ => return None,
                }
                .into(),
            );
        }
        let profile = match (pl >> 4) & 7 {
            1 => "High Profile",
            2 => "Spatially Scalable Profile",
            3 => "SNR Scalable Profile",
            4 => "Main Profile",
            5 => "Simple Profile",
            _ => return None,
        };
        let level = match pl & 15 {
            4 => "High Level",
            6 => "High 1440 Level",
            8 => "Main Level",
            10 => "Low Level",
            _ => return Some(profile.into()),
        };
        Some(format!("{profile} @ {level}"))
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a.max(1) } else { gcd(b, a % b) }
}

fn read_matrix(r: &mut Bits, m: &mut [u8; 64]) -> Result<(), Error> {
    for &z in &ZIGZAG {
        m[usize::from(z)] = r.u(8) as u8;
    }
    if m[0] == 0 {
        return Err(malformed("quantiser matrix with a zero entry"));
    }
    if m.contains(&0) {
        return Err(malformed("quantiser matrix with a zero entry"));
    }
    Ok(())
}

/// The sequence header (after its start code). Matrices not loaded take their defaults.
pub(crate) fn sequence_header(data: &[u8]) -> Result<Sequence, Error> {
    let mut r = Bits::new(data);
    let width = r.u(12);
    let height = r.u(12);
    let aspect_code = r.u(4) as u8;
    let frame_rate_code = r.u(4) as u8;
    let bit_rate = r.u(18);
    r.skip(1);
    let _vbv = r.u(10);
    let _constrained = r.flag();
    let mut quant = [DEFAULT_INTRA, [16; 64], DEFAULT_INTRA, [16; 64]];
    if r.flag() {
        read_matrix(&mut r, &mut quant[0])?;
        quant[2] = quant[0];
    }
    if r.flag() {
        read_matrix(&mut r, &mut quant[1])?;
        quant[3] = quant[1];
    }
    if r.overrun() {
        return Err(malformed("truncated sequence header"));
    }
    if width == 0 || height == 0 {
        return Err(malformed("zero picture size"));
    }
    Ok(Sequence {
        width,
        height,
        mpeg2: false,
        profile_level: 0,
        progressive: true,
        chroma: Chroma::Yuv420,
        aspect_code,
        frame_rate_code,
        frame_rate_ext: (0, 0),
        low_delay: false,
        colour: None,
        display: None,
        bit_rate,
        quant,
        scalable: false,
    })
}

/// Applies a sequence-level extension (sequence, display, quant matrix, scalable).
pub(crate) fn sequence_extension(seq: &mut Sequence, data: &[u8]) -> Result<(), Error> {
    let mut r = Bits::new(data);
    match r.u(4) {
        1 => {
            seq.mpeg2 = true;
            seq.profile_level = r.u(8) as u8;
            seq.progressive = r.flag();
            seq.chroma = match r.u(2) {
                1 => Chroma::Yuv420,
                2 => Chroma::Yuv422,
                3 => Chroma::Yuv444,
                _ => return Err(malformed("reserved chroma_format")),
            };
            seq.width |= r.u(2) << 12;
            seq.height |= r.u(2) << 12;
            seq.bit_rate |= r.u(12) << 18;
            r.skip(1);
            let _vbv_ext = r.u(8);
            seq.low_delay = r.flag();
            seq.frame_rate_ext = (r.u(2) as u8, r.u(5) as u8);
        }
        2 => {
            let _video_format = r.u(3);
            if r.flag() {
                seq.colour = Some((r.u(8) as u8, r.u(8) as u8, r.u(8) as u8));
            }
            let w = r.u(14);
            r.skip(1);
            let h = r.u(14);
            if w > 0 && h > 0 {
                seq.display = Some((w, h));
            }
        }
        3 => quant_matrix_extension(&mut seq.quant, &mut r)?,
        5 => seq.scalable = true,
        _ => {}
    }
    Ok(())
}

fn quant_matrix_extension(q: &mut [[u8; 64]; 4], r: &mut Bits) -> Result<(), Error> {
    if r.flag() {
        read_matrix(r, &mut q[0])?;
        q[2] = q[0];
    }
    if r.flag() {
        read_matrix(r, &mut q[1])?;
        q[3] = q[1];
    }
    if r.flag() {
        read_matrix(r, &mut q[2])?;
    }
    if r.flag() {
        read_matrix(r, &mut q[3])?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureType {
    I,
    P,
    B,
    /// MPEG-1 DC-only pictures (refused).
    D,
}

/// picture_structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Structure {
    Top,
    Bottom,
    Frame,
}

/// The picture header with its coding extension (MPEG-1 values when there is none).
#[derive(Clone, Debug)]
pub(crate) struct PictureHeader {
    pub temporal_reference: u16,
    pub kind: PictureType,
    /// f_code[s][t]: forward/backward, horizontal/vertical.
    pub f_code: [[u8; 2]; 2],
    /// MPEG-1 full-pel vectors (forward, backward).
    pub full_pel: [bool; 2],
    pub intra_dc_precision: u8,
    pub structure: Structure,
    pub top_field_first: bool,
    pub frame_pred_frame_dct: bool,
    pub concealment: bool,
    pub q_scale_type: bool,
    pub intra_vlc: bool,
    pub alternate_scan: bool,
    pub repeat_first_field: bool,
    pub progressive_frame: bool,
    /// A picture scalable extension was present (refused).
    pub scalable: bool,
}

pub(crate) fn picture_header(data: &[u8]) -> Result<PictureHeader, Error> {
    let mut r = Bits::new(data);
    let temporal_reference = r.u(10) as u16;
    let kind = match r.u(3) {
        1 => PictureType::I,
        2 => PictureType::P,
        3 => PictureType::B,
        4 => PictureType::D,
        _ => return Err(malformed("reserved picture_coding_type")),
    };
    let _vbv_delay = r.u(16);
    let mut f_code = [[15u8; 2]; 2];
    let mut full_pel = [false; 2];
    if matches!(kind, PictureType::P | PictureType::B) {
        full_pel[0] = r.flag();
        let f = r.u(3) as u8;
        f_code[0] = [f, f];
    }
    if kind == PictureType::B {
        full_pel[1] = r.flag();
        let f = r.u(3) as u8;
        f_code[1] = [f, f];
    }
    if r.overrun() {
        return Err(malformed("truncated picture header"));
    }
    Ok(PictureHeader {
        temporal_reference,
        kind,
        f_code,
        full_pel,
        intra_dc_precision: 0,
        structure: Structure::Frame,
        top_field_first: false,
        frame_pred_frame_dct: true,
        concealment: false,
        q_scale_type: false,
        intra_vlc: false,
        alternate_scan: false,
        repeat_first_field: false,
        progressive_frame: true,
        scalable: false,
    })
}

/// Applies a picture-level extension: coding, quant matrix (to `quant`, the matrices in
/// force: they stay until the next sequence header), scalable ones.
pub(crate) fn picture_extension(
    p: &mut PictureHeader,
    quant: &mut [[u8; 64]; 4],
    data: &[u8],
) -> Result<(), Error> {
    let mut r = Bits::new(data);
    match r.u(4) {
        8 => {
            for s in 0..2 {
                for t in 0..2 {
                    p.f_code[s][t] = r.u(4) as u8;
                }
            }
            p.full_pel = [false; 2];
            p.intra_dc_precision = r.u(2) as u8;
            p.structure = match r.u(2) {
                1 => Structure::Top,
                2 => Structure::Bottom,
                3 => Structure::Frame,
                _ => return Err(malformed("reserved picture_structure")),
            };
            p.top_field_first = r.flag();
            p.frame_pred_frame_dct = r.flag();
            p.concealment = r.flag();
            p.q_scale_type = r.flag();
            p.intra_vlc = r.flag();
            p.alternate_scan = r.flag();
            p.repeat_first_field = r.flag();
            let _chroma_420_type = r.flag();
            p.progressive_frame = r.flag();
            if r.overrun() {
                return Err(malformed("truncated picture coding extension"));
            }
        }
        3 => quant_matrix_extension(quant, &mut r)?,
        9 | 10 => p.scalable = true,
        _ => {}
    }
    Ok(())
}

/// A GOP header: closed_gop, broken_link.
pub(crate) fn gop_header(data: &[u8]) -> (bool, bool) {
    let mut r = Bits::new(data);
    r.skip(25);
    (r.flag(), r.flag())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_aspect_becomes_a_pixel_aspect() {
        let mut s = sequence_header(&[0x2D, 0x01, 0xE0, 0x24, 0xFF, 0xFF, 0xE0, 0x00]).unwrap();
        assert_eq!((s.width, s.height), (720, 480));
        s.mpeg2 = true;
        s.aspect_code = 2;
        // 4:3 at 720×480: 8:9 pixels.
        assert_eq!(s.pixel_aspect(), Some((8, 9)));
        s.aspect_code = 3;
        assert_eq!(s.pixel_aspect(), Some((32, 27)));
        s.aspect_code = 1;
        assert_eq!(s.pixel_aspect(), None);
        s.profile_level = 0x48;
        assert_eq!(
            s.profile_name().as_deref(),
            Some("Main Profile @ Main Level")
        );
        s.profile_level = 0x85;
        assert_eq!(
            s.profile_name().as_deref(),
            Some("4:2:2 Profile @ Main Level")
        );
    }
}
