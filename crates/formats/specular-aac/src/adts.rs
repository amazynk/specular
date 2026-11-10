//! ADTS framing (ISO/IEC 13818-7 §6.2 / 14496-3 §1.A.3.2): a 7- or 9-byte header in front of
//! each frame of one to four raw data blocks.

use crate::config::{Config, Signal, layout, rate_index};
use crate::{Error, Result};

/// A parsed ADTS header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// MPEG-4 audio object type (profile + 1).
    pub object_type: u8,
    /// `sampling_frequency_index`.
    pub rate_index: u8,
    /// `channel_configuration` (0 = a PCE in the first raw data block).
    pub channel_config: u8,
    /// Header length in bytes (7, or 9 with a CRC).
    pub header_len: usize,
    /// Whole frame length in bytes, header included.
    pub frame_len: usize,
    /// Raw data blocks in the frame.
    pub blocks: u8,
}

impl Header {
    /// The payload range of the frame within the bytes it was parsed from.
    pub fn payload(&self) -> std::ops::Range<usize> {
        self.header_len..self.frame_len
    }

    /// The equivalent AudioSpecificConfig, as SBR and PS stay implicit in ADTS.
    pub fn config(&self) -> Result<Config> {
        if !matches!(self.object_type, 1 | 2 | 4) {
            return Err(Error::Unsupported(crate::config::object_type_name(
                self.object_type,
            )));
        }
        let rate = *crate::config::SAMPLE_RATES
            .get(usize::from(self.rate_index))
            .ok_or_else(|| Error::Malformed("reserved ADTS sampling index".into()))?;
        let elements = if self.channel_config == 0 {
            Vec::new()
        } else {
            layout(self.channel_config)?
        };
        let tags = vec![None; elements.len()];
        Ok(Config {
            object_type: self.object_type,
            sample_rate: rate,
            rate_index: rate_index(rate),
            channel_config: self.channel_config,
            elements,
            tags,
            sbr: Signal::Implicit,
            ps: Signal::Implicit,
            sbr_rate: None,
            frame_len: 1024,
        })
    }

    /// A two-byte AudioSpecificConfig for this stream (channel configuration as signalled).
    pub fn asc(&self) -> [u8; 2] {
        let v = (u16::from(self.object_type) << 11)
            | (u16::from(self.rate_index) << 7)
            | (u16::from(self.channel_config) << 3);
        v.to_be_bytes()
    }
}

/// Parses the ADTS header at the start of `data`.
pub fn parse(data: &[u8]) -> Result<Header> {
    if data.len() < 7 {
        return Err(Error::Malformed("ADTS header is truncated".into()));
    }
    if data[0] != 0xff || data[1] & 0xf6 != 0xf0 {
        return Err(Error::Malformed("no ADTS sync word".into()));
    }
    let protection_absent = data[1] & 1 != 0;
    let profile = data[2] >> 6;
    let rate_index = (data[2] >> 2) & 15;
    let channel_config = ((data[2] & 1) << 2) | (data[3] >> 6);
    let frame_len =
        (usize::from(data[3] & 3) << 11) | (usize::from(data[4]) << 3) | usize::from(data[5] >> 5);
    let blocks = (data[6] & 3) + 1;
    let header_len = if protection_absent { 7 } else { 9 };
    if rate_index > 12 {
        return Err(Error::Malformed(format!(
            "reserved ADTS sampling index {rate_index}"
        )));
    }
    if frame_len < header_len {
        return Err(Error::Malformed(
            "ADTS frame shorter than its header".into(),
        ));
    }
    Ok(Header {
        object_type: profile + 1,
        rate_index,
        channel_config,
        header_len,
        frame_len,
        blocks,
    })
}

/// Splits an ADTS stream into frames, resynchronising past garbage. Stops at a truncated frame.
pub fn frames(mut data: &[u8]) -> impl Iterator<Item = (Header, &[u8])> {
    std::iter::from_fn(move || {
        loop {
            if data.len() < 7 {
                return None;
            }
            match parse(data) {
                Ok(h) if h.frame_len <= data.len() => {
                    let frame = &data[..h.frame_len];
                    data = &data[h.frame_len..];
                    return Some((h, frame));
                }
                Ok(_) => return None,
                Err(_) => data = &data[1..],
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header() {
        // LC, 44.1 kHz, stereo, no CRC, 371 bytes, one block
        let h = parse(&[0xff, 0xf1, 0x50, 0x80, 0x2e, 0x7f, 0xfc]).unwrap();
        assert_eq!(h.object_type, 2);
        assert_eq!(h.rate_index, 4);
        assert_eq!(h.channel_config, 2);
        assert_eq!(h.frame_len, 371);
        assert_eq!(h.header_len, 7);
        assert_eq!(h.blocks, 1);
        assert_eq!(h.asc(), [0x12, 0x10]);
        assert_eq!(crate::parse_asc(&h.asc()).unwrap(), h.config().unwrap());
    }

    #[test]
    fn resyncs() {
        let mut s = vec![0u8, 0xff, 0x12];
        let frame = [0xff, 0xf1, 0x50, 0x80, 0x01, 0x3f, 0xfc, 1, 2];
        s.extend_from_slice(&frame);
        s.extend_from_slice(&frame);
        s.extend_from_slice(&frame[..5]);
        let f: Vec<_> = frames(&s).collect();
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].1, &frame);
    }
}
