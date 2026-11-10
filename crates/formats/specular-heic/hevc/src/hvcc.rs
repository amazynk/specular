//! The HEIF framing of HEVC: the `HEVCDecoderConfigurationRecord` (`hvcC`,
//! ISO/IEC 14496-15 §8.3.3) and length-prefixed NAL units.

use crate::error::{Error, Result};
use alloc::vec::Vec;

/// What [`parse`] found in an `hvcC` record.
#[derive(Debug, Clone)]
pub struct Config<'a> {
    /// Bytes in each NAL unit length prefix of the item data (1, 2 or 4).
    pub length_size: usize,
    /// The NAL units of the record's arrays (normally VPS, SPS, PPS and
    /// SEI), in order, without length prefixes.
    pub nal_units: Vec<&'a [u8]>,
}

/// Parses an `hvcC` record body.
///
/// A record cut short inside its arrays keeps the NAL units read so far;
/// whether they suffice is for the decoder to say.
pub fn parse(raw: &[u8]) -> Result<Config<'_>> {
    if raw.len() < 23 {
        return Err(Error::InvalidData("hvcC record shorter than its header"));
    }
    let length_size = usize::from(raw[21] & 3) + 1;
    let arrays = raw[22];
    let mut nal_units = Vec::new();
    let mut p = 23usize;
    'arrays: for _ in 0..arrays {
        let Some(h) = raw.get(p..p + 3) else { break };
        let count = u16::from_be_bytes([h[1], h[2]]);
        p += 3;
        for _ in 0..count {
            let Some(l) = raw.get(p..p + 2) else {
                break 'arrays;
            };
            let len = usize::from(u16::from_be_bytes([l[0], l[1]]));
            p += 2;
            let Some(n) = raw.get(p..p + len) else {
                break 'arrays;
            };
            nal_units.push(n);
            p += len;
        }
    }
    Ok(Config {
        length_size,
        nal_units,
    })
}

/// Splits length-prefixed NAL units.
///
/// A length that runs past the end ends the list: the units before it are
/// returned, and what is missing is the decoder's to conceal. Zero-length
/// units are skipped.
pub fn split(data: &[u8], length_size: usize) -> Result<Vec<&[u8]>> {
    if !(1..=4).contains(&length_size) {
        return Err(Error::InvalidData("hvcC NAL unit length size"));
    }
    let mut out = Vec::new();
    let mut p = 0usize;
    while let Some(pre) = data.get(p..p + length_size) {
        let len = pre.iter().fold(0usize, |a, &b| (a << 8) | usize::from(b));
        p += length_size;
        match data.get(p..p.saturating_add(len)) {
            Some(n) => {
                if !n.is_empty() {
                    out.push(n);
                }
                p += len;
            }
            None => {
                // Keep the truncated tail: its slices decode up to the cut.
                if data.len() - p >= 3 {
                    out.push(&data[p..]);
                }
                break;
            }
        }
    }
    if out.is_empty() {
        return Err(Error::NoSlices);
    }
    Ok(out)
}

/// Decodes one coded picture from an `hvcC` record and its item data.
///
/// Parameter sets carried in the item data are used as well, after those
/// of the record.
pub fn decode(hvcc: &[u8], data: &[u8], options: &crate::Options<'_>) -> Result<crate::Decoded> {
    let cfg = parse(hvcc)?;
    let units = split(data, cfg.length_size)?;
    let is_ps = |n: &&[u8]| {
        n.first()
            .is_some_and(|b| (32..=34).contains(&((b >> 1) & 0x3f)))
    };
    let mut sets = cfg.nal_units;
    sets.extend(units.iter().copied().filter(is_ps));
    let slices: Vec<&[u8]> = units.into_iter().filter(|n| !is_ps(n)).collect();
    crate::decode_still_with(&sets, &slices, options)
}
