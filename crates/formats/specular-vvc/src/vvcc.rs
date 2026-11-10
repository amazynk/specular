//! The `vvcC` configuration record (VvcDecoderConfigurationRecord, ISO/IEC 14496-15 § 11.2)
//! of HEIF VVC image items (ISO/IEC 23008-12 Annex L).

use crate::error::{Error, Result};

/// A parsed `vvcC` record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VvcConfig {
    /// Bytes of each NAL unit length prefix in the item data (1, 2 or 4).
    pub length_size: usize,
    /// chroma_format_idc, bit depth and general_profile_idc when the record carries its
    /// profile, tier and level.
    pub chroma_format_idc: Option<u8>,
    pub bit_depth: Option<u8>,
    pub profile_idc: Option<u8>,
    /// The parameter set (and SEI) NAL units of the record, without length prefixes.
    pub nal_units: Vec<Vec<u8>>,
}

/// MSB-first reader over the record.
struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn u(&mut self, n: usize) -> Result<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let b = *self.d.get(self.pos >> 3).ok_or(Error::Truncated)?;
            v = (v << 1) | u32::from((b >> (7 - (self.pos & 7))) & 1);
            self.pos += 1;
        }
        Ok(v)
    }

    fn skip(&mut self, n: usize) -> Result<()> {
        if self.pos + n > self.d.len() * 8 {
            return Err(Error::Truncated);
        }
        self.pos += n;
        Ok(())
    }

    fn bytes(&mut self, n: usize) -> Result<&[u8]> {
        debug_assert!(self.pos.is_multiple_of(8));
        let s = self.pos / 8;
        let b = self.d.get(s..s + n).ok_or(Error::Truncated)?;
        self.pos += 8 * n;
        Ok(b)
    }
}

/// Parses the payload of a `vvcC` box (after the box header). The FullBox version and flags
/// that HEIF puts before the record are skipped when present: a record starts with five
/// reserved one bits, a version-0 FullBox with a zero byte.
pub fn parse(record: &[u8]) -> Result<VvcConfig> {
    let body = match record.first() {
        Some(b) if b & 0xf8 == 0xf8 => record,
        Some(0) if record.len() > 4 => &record[4..],
        Some(_) => return Err(Error::InvalidData("vvcC record of an unknown version")),
        None => return Err(Error::Truncated),
    };
    if body.first().is_none_or(|b| b & 0xf8 != 0xf8) {
        return Err(Error::InvalidData("vvcC record without its reserved bits"));
    }
    let mut r = Bits { d: body, pos: 0 };
    r.skip(5)?;
    let length_size = r.u(2)? as usize + 1;
    if length_size == 3 {
        return Err(Error::InvalidData("vvcC NAL length size of 3 bytes"));
    }
    let mut cfg = VvcConfig {
        length_size,
        ..VvcConfig::default()
    };
    if r.u(1)? == 1 {
        r.skip(9)?; // ols_idx
        let num_sublayers = r.u(3)? as usize;
        r.skip(2)?; // constant_frame_rate
        cfg.chroma_format_idc = Some(r.u(2)? as u8);
        cfg.bit_depth = Some(r.u(3)? as u8 + 8);
        r.skip(5)?;
        // VvcPTLRecord(num_sublayers).
        r.skip(2)?;
        let constraint_bytes = r.u(6)? as usize;
        cfg.profile_idc = Some(r.u(7)? as u8);
        r.skip(1 + 8)?; // tier, level
        r.skip(2)?; // frame_only, multilayer
        r.skip((8 * constraint_bytes).saturating_sub(2))?;
        let mut present = [false; 8];
        for i in (0..num_sublayers.saturating_sub(1)).rev() {
            present[i] = r.u(1)? == 1;
        }
        if num_sublayers > 1 {
            r.skip(9 - num_sublayers)?;
        }
        for i in (0..num_sublayers.saturating_sub(1)).rev() {
            if present[i] {
                r.skip(8)?;
            }
        }
        let sub_profiles = r.u(8)? as usize;
        r.skip(32 * sub_profiles)?;
        r.skip(16 * 3)?; // max_picture_width, max_picture_height, avg_frame_rate
    }
    if !r.pos.is_multiple_of(8) {
        return Err(Error::InvalidData("vvcC profile record not byte aligned"));
    }
    let arrays = r.u(8)?;
    for _ in 0..arrays {
        let nal_type = r.u(8)? & 0x1f;
        let n = if nal_type == 12 || nal_type == 13 {
            1
        } else {
            r.u(16)?
        };
        for _ in 0..n {
            let len = r.u(16)? as usize;
            let nal = r.bytes(len)?;
            if !nal.is_empty() {
                cfg.nal_units.push(nal.to_vec());
            }
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPS: &[u8] = &[0x00, 0x79, 0x00, 0xad];
    const PPS: &[u8] = &[0x00, 0x81, 0x00];

    fn arrays() -> Vec<u8> {
        let mut v = vec![2u8];
        v.extend([0x80 | 15, 0, 1, 0, SPS.len() as u8]);
        v.extend(SPS);
        v.extend([0x80 | 16, 0, 1, 0, PPS.len() as u8]);
        v.extend(PPS);
        v
    }

    #[test]
    fn bare_record_without_ptl() {
        let mut r = vec![0xf8 | (1 << 1)]; // length size 2, no PTL
        r.extend(arrays());
        let c = parse(&r).unwrap();
        assert_eq!(c.length_size, 2);
        assert_eq!(c.chroma_format_idc, None);
        assert_eq!(c.nal_units, vec![SPS.to_vec(), PPS.to_vec()]);
    }

    #[test]
    fn full_box_record_with_ptl() {
        // libheif's layout: FullBox header, length size 4, PTL with 6 sublayers.
        let mut r = vec![0, 0, 0, 0, 0xff, 0x00, 0x65, 0x1f];
        r.extend([0x01, 0x02, 0x10, 0x80, 0x00, 0x00]); // PTL, 1 constraint byte
        r.extend([0x00, 0x20, 0x00, 0x20, 0x63, 0x17]);
        r.extend(arrays());
        let c = parse(&r).unwrap();
        assert_eq!(c.length_size, 4);
        assert_eq!(c.chroma_format_idc, Some(1));
        assert_eq!(c.bit_depth, Some(8));
        assert_eq!(c.profile_idc, Some(1));
        assert_eq!(c.nal_units.len(), 2);
        // One sublayer, 10 bits, 4:4:4, a DCI array (no count).
        let mut r = vec![0xfb, 0x00, 0x1f, 0x5f];
        r.extend([0x01, 0x42, 0x10, 0x80, 0x00]);
        r.extend([0x00, 0x20, 0x00, 0x20, 0x00, 0x00]);
        r.extend([1, 0x80 | 13, 0, 2, 0x00, 0x69]);
        let c = parse(&r).unwrap();
        assert_eq!((c.chroma_format_idc, c.bit_depth), (Some(3), Some(10)));
        assert_eq!(c.profile_idc, Some(33));
        assert_eq!(c.nal_units, vec![vec![0x00, 0x69]]);
    }

    #[test]
    fn truncated_and_bad_records() {
        assert!(parse(&[]).is_err());
        assert!(parse(&[0x12, 0, 0]).is_err());
        let mut r = vec![0xf8 | 3 << 1];
        r.extend(&arrays()[..6]);
        assert_eq!(parse(&r), Err(Error::Truncated));
    }
}
