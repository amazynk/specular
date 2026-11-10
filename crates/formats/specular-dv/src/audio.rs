//! DV sound (SMPTE 314M 4.6, IEC 61834): 16-bit linear PCM at 48, 44.1 or 32 kHz, and
//! 12-bit nonlinear at 32 kHz (four-channel recordings; the first pair is returned), two
//! channels de-shuffled from the audio DIF blocks of the first half of a frame.
//!
//! 12-bit samples are stored in pairs, three bytes per pair (the upper eight bits of each,
//! then their low nibbles), shuffled like 16-bit words. Their expansion to 16 bits was
//! measured against FFmpeg's decode (2,762 distinct codes of two camcorder files, none
//! inconsistent): linear below 512, then segments of 256 codes whose step doubles (2 to 64),
//! mirrored for negative values as the one's complement. Codes 0x700–0x7FF (above 16384)
//! were not seen and follow the same rule.

/// One frame's sound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Audio {
    pub rate: u32,
    pub channels: u16,
    /// Interleaved samples (`channels` per frame).
    pub samples: Vec<i16>,
}

const SEQ: usize = 150 * 80;

/// The audio DIF block `a` (0..9) of a sequence.
fn audio_block(seq: &[u8], a: usize) -> &[u8] {
    let at = (6 + a * 16) * 80;
    &seq[at..at + 80]
}

/// 12-bit nonlinear code to a 16-bit sample.
fn expand12(c: u16) -> i16 {
    let pos = |c: u16| -> i32 {
        let seg = c >> 8;
        if seg < 2 {
            i32::from(c)
        } else {
            (1 << (seg + 7)) + i32::from(c & 0xFF) * (1 << (seg - 1))
        }
    };
    if c < 0x800 {
        pos(c) as i16
    } else {
        (-pos(!c & 0xFFF) - 1) as i16
    }
}

/// The frame's sound: CH1 and CH2. `None` when there is no valid AAUX source pack or the
/// frame is cut short.
pub fn audio(frame: &[u8]) -> Option<Audio> {
    if frame.len() < SEQ {
        return None;
    }
    let pal = crate::header(frame)?[3] & 0x80 != 0;
    let seqs = if pal { 12 } else { 10 };
    if frame.len() < seqs * SEQ {
        return None;
    }
    // The AAUX source pack (0x50) of the first sequences; the second half's describes CH2.
    let source = |s: usize| {
        let seq = &frame[s * SEQ..(s + 1) * SEQ];
        (0..9)
            .map(|a| &audio_block(seq, a)[3..8])
            .find(|p| p[0] == 0x50)
    };
    let half = seqs / 2;
    let as1 = source(0).or_else(|| source(1))?;
    let smp = (as1[4] >> 3) & 7;
    let qu = as1[4] & 7;
    if qu > 1 || qu == 1 && smp != 2 {
        return None;
    }
    let (rate, min) = match (smp, pal) {
        (0, false) => (48_000, 1580),
        (1, false) => (44_100, 1452),
        (2, false) => (32_000, 1053),
        (0, true) => (48_000, 1896),
        (1, true) => (44_100, 1742),
        (2, true) => (32_000, 1264),
        _ => return None,
    };
    let n = min + usize::from(as1[1] & 0x3F);
    let cap = 9 * half * if qu == 1 { 24 } else { 36 };
    if n > cap {
        return None;
    }
    let mut samples = vec![0i16; n * 2];
    if qu == 1 {
        // Pairs of CH1 and CH2 in the first half of the sequences.
        for s in 0..n {
            let dseq = (s / 3 + 2 * (s % 3)) % half;
            let blk = 3 * (s % 3) + (s % (9 * half)) / (3 * half);
            let byte = 8 + 3 * (s / (9 * half));
            let seq = &frame[dseq * SEQ..(dseq + 1) * SEQ];
            let b = &audio_block(seq, blk)[byte..byte + 3];
            let l = u16::from(b[0]) << 4 | u16::from(b[2] >> 4);
            let r = u16::from(b[1]) << 4 | u16::from(b[2] & 15);
            samples[s * 2] = expand12(l);
            samples[s * 2 + 1] = expand12(r);
        }
        return Some(Audio {
            rate,
            channels: 2,
            samples,
        });
    }
    // CH2 is invalid (audio mode 1111) in mono recordings: CH1 is then given twice.
    let ch2_valid = source(half).is_none_or(|p| p[2] & 0x0F != 0x0F);
    for ch in 0..2 {
        let src_ch = if ch == 1 && !ch2_valid { 0 } else { ch };
        for s in 0..n {
            let dseq = (s / 3 + 2 * (s % 3)) % half + src_ch * half;
            let blk = 3 * (s % 3) + (s % (9 * half)) / (3 * half);
            let byte = 8 + 2 * (s / (9 * half));
            let seq = &frame[dseq * SEQ..(dseq + 1) * SEQ];
            let b = audio_block(seq, blk);
            let v = i16::from_be_bytes([b[byte], b[byte + 1]]);
            // 8000h marks an invalid sample.
            samples[s * 2 + ch] = if v == i16::MIN { 0 } else { v };
        }
    }
    Some(Audio {
        rate,
        channels: 2,
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::expand12;

    #[test]
    fn twelve_bit_segments() {
        assert_eq!(expand12(0), 0);
        assert_eq!(expand12(0x1FF), 511);
        assert_eq!(expand12(0x200), 512);
        assert_eq!(expand12(0x614), 8832);
        assert_eq!(expand12(0x7FF), 32704);
        assert_eq!(expand12(0xFFF), -1);
        assert_eq!(expand12(0xDFF), -513);
        assert_eq!(expand12(0xA68), -6513);
    }
}
