//! Random access points: IDR pictures and recovery point SEI messages (D.1.8, D.2.8).

use crate::bits::{BitReader, unescape};

/// recovery_frame_cnt of the recovery point SEI message in an SEI RBSP, if it has one.
fn recovery_frame_cnt(rbsp: &[u8], len: usize) -> Option<u32> {
    let mut p = 0;
    // sei_message() until rbsp_trailing_bits (a lone 0x80).
    while p < len && !(rbsp[p] == 0x80 && p + 1 == len) {
        let mut read = || {
            let mut v = 0usize;
            loop {
                let b = *rbsp.get(p)?;
                p += 1;
                v += b as usize;
                if b != 0xFF {
                    return Some(v);
                }
            }
        };
        let ty = read()?;
        let size = read()?;
        let body = rbsp.get(p..p + size)?;
        if ty == 6 {
            let mut b = body.to_vec();
            b.resize(size + crate::bits::PAD, 0);
            let mut r = BitReader::new(&b, size);
            let cnt = r.ue();
            return (!r.overrun()).then_some(cnt);
        }
        p += size;
    }
    None
}

/// recovery_frame_cnt of the recovery point SEI message in an SEI NAL unit (with its header
/// byte), if it has one.
pub fn sei_recovery_frame_cnt(nal: &[u8]) -> Option<u32> {
    let mut buf = Vec::new();
    let len = unescape(nal.get(1..)?, &mut buf);
    recovery_frame_cnt(&buf, len)
}

/// Whether an access unit of NAL units with `length_size`-byte lengths (an MP4 or Matroska
/// sample) is a random access point: decoding can start there and every picture presented
/// from it on is exact. That is an IDR picture, or an I picture with a recovery point SEI
/// message with recovery_frame_cnt 0 (how x264 and others mark the I pictures of open GOPs).
pub fn random_access(sample: &[u8], length_size: usize) -> bool {
    let mut p = 0;
    let mut recovery = false;
    let mut buf = Vec::new();
    while p + length_size <= sample.len() && length_size > 0 {
        let n = sample[p..p + length_size]
            .iter()
            .fold(0usize, |a, &b| (a << 8) | b as usize);
        p += length_size;
        let Some(nal) = sample.get(p..p + n).filter(|n| !n.is_empty()) else {
            return false;
        };
        p += n;
        match nal[0] & 31 {
            5 => return true,
            6 => recovery |= sei_recovery_frame_cnt(nal) == Some(0),
            1 => {
                if !recovery {
                    return false;
                }
                // slice_type 2 or 7: I.
                let len = unescape(&nal[1..nal.len().min(16)], &mut buf);
                let mut r = BitReader::new(&buf, len);
                r.ue();
                return matches!(r.ue(), 2 | 7) && !r.overrun();
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(nals: &[&[u8]]) -> Vec<u8> {
        let mut v = Vec::new();
        for n in nals {
            v.extend_from_slice(&(n.len() as u32).to_be_bytes());
            v.extend_from_slice(n);
        }
        v
    }

    #[test]
    fn recovery_points() {
        // SEI: user data (type 5, 3 bytes), then recovery point (type 6): cnt 0, exact,
        // not broken, idc 0 → bits 1 1 0 00 → 0b1100_0000 + stop bit.
        let sei: &[u8] = &[6, 5, 3, 1, 2, 3, 6, 1, 0b1100_0100, 0x80];
        // Non-IDR slice: first_mb 0 (1), slice_type 7 (0001000).
        let i_slice: &[u8] = &[0x21, 0b1000_1000, 0x80];
        // slice_type 5 (P): 00110.
        let p_slice: &[u8] = &[0x21, 0b1001_1000, 0x80];
        assert!(random_access(&sample(&[sei, i_slice]), 4));
        assert!(!random_access(&sample(&[sei, p_slice]), 4));
        assert!(!random_access(&sample(&[i_slice]), 4));
        assert!(random_access(&sample(&[&[0x65, 0x88, 0x80]]), 4));
        // recovery_frame_cnt 2 (011): not a point to start an exact decode at.
        let later: &[u8] = &[6, 6, 1, 0b0111_0010, 0x80];
        assert!(!random_access(&sample(&[later, i_slice]), 4));
        assert!(!random_access(&sample(&[&[6, 6, 9]]), 4));
        assert_eq!(sei_recovery_frame_cnt(later), Some(2));
        assert_eq!(sei_recovery_frame_cnt(&[6, 5, 1, 0, 0x80]), None);
    }
}
