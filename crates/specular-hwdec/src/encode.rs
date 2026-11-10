//! Hardware H.264 encoding for the slideshow video export (DESIGN.md §12.3): VA-API on Linux,
//! Media Foundation (the GPU driver's encoder MFT) on Windows, nothing on macOS yet. Constant
//! QP, one IDR every `keyint` pictures, P pictures otherwise (no B pictures: decode order is
//! presentation order). The output is AVCC: 4-byte lengths, the SPS and PPS kept out of the
//! samples for the `avcC`.
//!
//! Whether an export uses it is the path selector's choice (`Task::VideoEncode`); the caller
//! keeps our software encoder as the fallback for any error.

use crate::Error;

/// The H.264 profile the hardware encodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeProfile {
    Main,
    High,
}

/// What to encode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodeConfig {
    /// Picture size in samples; both even.
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    /// Quantisation parameter, 10..=51.
    pub qp: u8,
    /// Longest distance between IDR pictures, in frames.
    pub keyint: u32,
}

/// One coded picture: AVCC NAL units (4-byte big-endian lengths).
#[derive(Clone, Debug)]
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub key: bool,
}

/// A hardware H.264 encoder for one stream.
pub struct H264Encoder {
    #[cfg(target_os = "linux")]
    va: crate::vaapi::enc::VaEnc,
    #[cfg(windows)]
    mf: crate::mf_enc::MfEnc,
}

impl H264Encoder {
    /// Opens the device's encoder and learns its SPS and PPS (one warm-up picture, a few ms).
    /// [`Error::Unavailable`] without a device, [`Error::Unsupported`] when it can't take
    /// this stream.
    pub fn new(cfg: &EncodeConfig) -> Result<H264Encoder, Error> {
        if !(10..=51).contains(&cfg.qp) {
            return Err(Error::Unsupported("qp outside 10..=51".into()));
        }
        #[cfg(target_os = "linux")]
        {
            Ok(H264Encoder {
                va: crate::vaapi::enc::VaEnc::open(cfg)?,
            })
        }
        #[cfg(windows)]
        {
            Ok(H264Encoder {
                mf: crate::mf_enc::MfEnc::open(cfg)?,
            })
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            let _ = cfg;
            Err(Error::Unavailable)
        }
    }

    /// The SPS NAL unit (no start code or length).
    pub fn sps(&self) -> &[u8] {
        #[cfg(target_os = "linux")]
        {
            &self.va.sps
        }
        #[cfg(windows)]
        {
            &self.mf.sps
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            &[]
        }
    }

    /// The PPS NAL unit.
    pub fn pps(&self) -> &[u8] {
        #[cfg(target_os = "linux")]
        {
            &self.va.pps
        }
        #[cfg(windows)]
        {
            &self.mf.pps
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            &[]
        }
    }

    pub fn profile(&self) -> EncodeProfile {
        #[cfg(target_os = "linux")]
        {
            self.va.profile
        }
        #[cfg(windows)]
        {
            self.mf.profile
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            EncodeProfile::High
        }
    }

    /// The device, for logs ("/dev/dri/renderD128, Mesa Gallium driver …", "AMDh264Encoder").
    pub fn device(&self) -> &str {
        #[cfg(target_os = "linux")]
        {
            &self.va.device
        }
        #[cfg(windows)]
        {
            &self.mf.device
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            ""
        }
    }

    /// The stream's QP (the PPS's initial QP).
    pub fn qp(&self) -> u8 {
        #[cfg(target_os = "linux")]
        {
            self.va.qp()
        }
        #[cfg(windows)]
        {
            self.mf.qp()
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            26
        }
    }

    /// Encodes one picture from planar 4:2:0 (BT.709 limited range by convention of the
    /// caller; the encoder does not convert) at the stream's QP. The first picture and every
    /// `keyint`-th are IDR, as is any with `force_idr`.
    pub fn encode(
        &mut self,
        y: &[u8],
        u: &[u8],
        v: &[u8],
        y_stride: usize,
        c_stride: usize,
        force_idr: bool,
    ) -> Result<EncodedFrame, Error> {
        let qp = self.qp();
        self.encode_at(y, u, v, y_stride, c_stride, force_idr, qp)
    }

    /// [`H264Encoder::encode`] at quantiser `qp` for this picture (10..=51; signalled as the
    /// slice's QP delta, so the stream's parameter sets stay the same).
    #[allow(clippy::too_many_arguments)]
    pub fn encode_at(
        &mut self,
        y: &[u8],
        u: &[u8],
        v: &[u8],
        y_stride: usize,
        c_stride: usize,
        force_idr: bool,
        qp: u8,
    ) -> Result<EncodedFrame, Error> {
        #[cfg(target_os = "linux")]
        {
            let (data, key) = self.va.encode(y, u, v, y_stride, c_stride, force_idr, qp)?;
            Ok(EncodedFrame { data, key })
        }
        #[cfg(windows)]
        {
            let (data, key) = self.mf.encode(y, u, v, y_stride, c_stride, force_idr, qp)?;
            Ok(EncodedFrame { data, key })
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            let _ = (y, u, v, y_stride, c_stride, force_idr, qp);
            Err(Error::Unavailable)
        }
    }
}

/// The NAL units of an Annex B byte stream (start codes and trailing zero bytes removed).
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
pub(crate) fn annex_b_nals(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut v = Vec::with_capacity(starts.len());
    for (k, &s) in starts.iter().enumerate() {
        let mut e = starts.get(k + 1).map_or(data.len(), |&n| n - 3);
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        if e > s {
            v.push(&data[s..e]);
        }
    }
    v
}

/// A coded picture, as the encoder wrote it (Annex B), as AVCC NAL units: access unit
/// delimiters and filler dropped, the SPS and PPS taken out (the first ones kept in `sps` and
/// `pps` for the `avcC`; later ones must repeat them byte for byte). Also whether the picture
/// is an IDR picture. An error when there is no slice, or IDR and other slices are mixed.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
pub(crate) fn collect_avcc(
    coded: &[u8],
    sps: &mut Vec<u8>,
    pps: &mut Vec<u8>,
) -> Result<(Vec<u8>, bool), Error> {
    let mut out = Vec::with_capacity(coded.len() + 16);
    let (mut idr, mut other) = (0, 0);
    for nal in annex_b_nals(coded) {
        let kind = nal[0] & 0x1f;
        let keep = match kind {
            7 | 8 => {
                let slot = if kind == 7 { &mut *sps } else { &mut *pps };
                if slot.is_empty() {
                    *slot = nal.to_vec();
                } else if slot.as_slice() != nal {
                    return Err(Error::Hardware(
                        "the encoder changed its parameter sets mid-stream".into(),
                    ));
                }
                false
            }
            9 | 12 => false,
            1 => {
                other += 1;
                true
            }
            5 => {
                idr += 1;
                true
            }
            _ => true,
        };
        if keep {
            out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
            out.extend_from_slice(nal);
        }
    }
    if idr + other == 0 {
        let kinds: Vec<u8> = annex_b_nals(coded).iter().map(|n| n[0] & 0x1f).collect();
        return Err(Error::Hardware(format!(
            "the encoder returned no slice ({} bytes, NAL types {kinds:?}, {:02x?})",
            coded.len(),
            &coded[..coded.len().min(48)]
        )));
    }
    if idr != 0 && other != 0 {
        return Err(Error::Hardware(
            "the encoder mixed IDR and other slices in one picture".into(),
        ));
    }
    Ok((out, idr != 0))
}

/// The value of the first `ue(v)` after `skip` bytes of a NAL unit payload (emulation
/// prevention ignored: parameter set ids sit in the first bytes, before any can occur).
fn first_ue(nal: &[u8], skip: usize) -> Option<u32> {
    let bytes = nal.get(skip..)?;
    let mut bits = bytes
        .iter()
        .take(8)
        .flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1));
    let mut zeros = 0;
    while bits.next()? == 0 {
        zeros += 1;
        if zeros > 16 {
            return None;
        }
    }
    let mut v = 0u32;
    for _ in 0..zeros {
        v = v << 1 | u32::from(bits.next()?);
    }
    Some((1 << zeros) - 1 + v)
}

/// `seq_parameter_set_id` of an SPS NAL unit (header byte included).
pub fn sps_id(sps: &[u8]) -> Option<u32> {
    first_ue(sps, 4)
}

/// `pic_parameter_set_id` of a PPS NAL unit (header byte included).
pub fn pps_id(pps: &[u8]) -> Option<u32> {
    first_ue(pps, 1)
}

/// An AVCDecoderConfigurationRecord (ISO/IEC 14496-15, the `avcC` body) holding every given
/// SPS and PPS: the highest profile and level of all, the constraint flags all share.
pub fn avcc(sps: &[&[u8]], pps: &[&[u8]]) -> Vec<u8> {
    let profile = sps
        .iter()
        .map(|s| s.get(1).copied().unwrap_or(0))
        .max()
        .unwrap_or(100);
    let compat = sps
        .iter()
        .fold(0xffu8, |a, s| a & s.get(2).copied().unwrap_or(0));
    let level = sps
        .iter()
        .map(|s| s.get(3).copied().unwrap_or(0))
        .max()
        .unwrap_or(40);
    let mut v = vec![1, profile, compat, level, 0xFF, 0xE0 | sps.len() as u8];
    for s in sps {
        v.extend_from_slice(&(s.len() as u16).to_be_bytes());
        v.extend_from_slice(s);
    }
    v.push(pps.len() as u8);
    for p in pps {
        v.extend_from_slice(&(p.len() as u16).to_be_bytes());
        v.extend_from_slice(p);
    }
    if matches!(profile, 100 | 110 | 122 | 144) {
        // chroma_format 4:2:0, 8-bit, no SPS extensions.
        v.extend_from_slice(&[0xFC | 1, 0xF8, 0xF8, 0]);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_set_ids() {
        // High, constraint 0, level 4.1, then ue(0) = '1' and ue(1) = '010'.
        assert_eq!(sps_id(&[0x67, 100, 0, 41, 0b1000_0000]), Some(0));
        assert_eq!(sps_id(&[0x67, 100, 0, 41, 0b0100_0000]), Some(1));
        assert_eq!(pps_id(&[0x68, 0b0110_0000]), Some(2));
        assert_eq!(pps_id(&[0x68]), None);
    }

    #[test]
    fn splits_annex_b() {
        let s = [
            0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3, 0, 0, 0, 1, 0x65, 4, 0,
        ];
        let n = annex_b_nals(&s);
        assert_eq!(n, vec![&[0x67, 1, 2][..], &[0x68, 3][..], &[0x65, 4][..]]);
        assert!(annex_b_nals(&[1, 2, 3]).is_empty());
    }

    #[test]
    fn collects_avcc_and_parameter_sets() {
        let (mut sps, mut pps) = (Vec::new(), Vec::new());
        // AUD, SPS, PPS, SEI, IDR slice.
        let idr = [
            0, 0, 0, 1, 0x09, 0x10, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3, 0, 0, 1, 0x06, 9, 0, 0,
            0, 1, 0x65, 4, 5,
        ];
        let (out, key) = collect_avcc(&idr, &mut sps, &mut pps).unwrap();
        assert!(key);
        assert_eq!(sps, [0x67, 1, 2]);
        assert_eq!(pps, [0x68, 3]);
        assert_eq!(out, [0, 0, 0, 2, 0x06, 9, 0, 0, 0, 3, 0x65, 4, 5]);
        let p = [0, 0, 0, 1, 0x41, 7, 7];
        let (out, key) = collect_avcc(&p, &mut sps, &mut pps).unwrap();
        assert!(!key);
        assert_eq!(out, [0, 0, 0, 3, 0x41, 7, 7]);
        // A changed PPS, no slice, mixed slices: errors.
        assert!(collect_avcc(&[0, 0, 1, 0x68, 4, 0, 0, 1, 0x65, 1], &mut sps, &mut pps).is_err());
        assert!(collect_avcc(&[0, 0, 1, 0x09, 0x10], &mut sps, &mut pps).is_err());
        assert!(collect_avcc(&[0, 0, 1, 0x65, 1, 0, 0, 1, 0x41, 1], &mut sps, &mut pps).is_err());
    }

    #[test]
    fn avcc_with_two_sets() {
        let a: &[u8] = &[0x67, 100, 0, 40, 0x80];
        let b: &[u8] = &[0x67, 100, 0, 41, 0x40];
        let v = avcc(&[a, b], &[&[0x68, 0xC0], &[0x68, 0x40]]);
        assert_eq!(&v[..6], &[1, 100, 0, 41, 0xFF, 0xE2]);
        assert_eq!(v.len(), 6 + 2 * (2 + 5) + 1 + 2 * (2 + 2) + 4);
    }
}
