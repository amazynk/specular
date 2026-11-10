//! NAL unit headers (7.3.1.2) and Annex B byte stream splitting.

use crate::error::{Error, Result};

pub const TRAIL: u8 = 0;
pub const STSA: u8 = 1;
pub const RADL: u8 = 2;
pub const RASL: u8 = 3;
pub const IDR_W_RADL: u8 = 7;
pub const IDR_N_LP: u8 = 8;
pub const CRA: u8 = 9;
pub const GDR: u8 = 10;
pub const OPI: u8 = 12;
pub const DCI: u8 = 13;
pub const VPS: u8 = 14;
pub const SPS: u8 = 15;
pub const PPS: u8 = 16;
pub const PREFIX_APS: u8 = 17;
pub const SUFFIX_APS: u8 = 18;
pub const PH: u8 = 19;
pub const AUD: u8 = 20;
pub const EOS: u8 = 21;
pub const EOB: u8 = 22;

/// A NAL unit header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub layer_id: u8,
    pub nal_type: u8,
    pub temporal_id: u8,
}

impl Header {
    pub fn parse(nal: &[u8]) -> Result<Header> {
        if nal.len() < 2 {
            return Err(Error::InvalidData("NAL unit shorter than its header"));
        }
        if nal[0] & 0x80 != 0 {
            return Err(Error::InvalidData("forbidden_zero_bit set"));
        }
        let tid1 = nal[1] & 7;
        if tid1 == 0 {
            return Err(Error::InvalidData("nuh_temporal_id_plus1 is 0"));
        }
        Ok(Header {
            layer_id: nal[0] & 0x3f,
            nal_type: nal[1] >> 3,
            temporal_id: tid1 - 1,
        })
    }

    /// A coded slice (VCL NAL unit types 0 to 11).
    pub fn is_vcl(self) -> bool {
        self.nal_type <= 11
    }

    pub fn is_irap(self) -> bool {
        (IDR_W_RADL..=11).contains(&self.nal_type)
    }

    pub fn is_idr(self) -> bool {
        self.nal_type == IDR_W_RADL || self.nal_type == IDR_N_LP
    }
}

/// Splits an Annex B byte stream into NAL units (without start codes or trailing zero bytes).
pub fn split_annexb(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let n = data.len();
    let mut i = 0;
    let mut start: Option<usize> = None;
    while i + 2 < n {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            if let Some(s) = start {
                out.push(trim_zeros(&data[s..i]));
            }
            i += 3;
            start = Some(i);
        } else {
            i += 1;
        }
    }
    if let Some(s) = start {
        out.push(trim_zeros(&data[s..]));
    }
    out.retain(|u| !u.is_empty());
    out
}

fn trim_zeros(mut s: &[u8]) -> &[u8] {
    while let [rest @ .., 0] = s {
        s = rest;
    }
    s
}

/// Splits length-prefixed NAL units (`length_size` bytes of big-endian length each).
pub fn split_length_prefixed(data: &[u8], length_size: usize) -> Result<Vec<&[u8]>> {
    if !(1..=4).contains(&length_size) {
        return Err(Error::InvalidData("NAL length size is not 1 to 4 bytes"));
    }
    let mut out = Vec::new();
    let mut p = 0;
    while p < data.len() {
        if p + length_size > data.len() {
            return Err(Error::Truncated);
        }
        let mut len = 0usize;
        for &b in &data[p..p + length_size] {
            len = (len << 8) | usize::from(b);
        }
        p += length_size;
        if len > data.len() - p {
            return Err(Error::Truncated);
        }
        if len > 0 {
            out.push(&data[p..p + len]);
        }
        p += len;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annexb_and_headers() {
        let s = [0, 0, 0, 1, 0x00, 0x79, 0xaa, 0, 0, 1, 0x00, 0x41, 0xbb, 0];
        let u = split_annexb(&s);
        assert_eq!(u.len(), 2);
        assert_eq!(Header::parse(u[0]).unwrap().nal_type, SPS);
        let h = Header::parse(u[1]).unwrap();
        assert_eq!(h.nal_type, IDR_N_LP);
        assert!(h.is_vcl() && h.is_irap() && h.is_idr());
        let lp = [0, 2, 9, 9, 0, 1, 7];
        assert_eq!(
            split_length_prefixed(&lp, 2).unwrap(),
            vec![&[9u8, 9][..], &[7][..]]
        );
    }
}
