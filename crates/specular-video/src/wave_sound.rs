//! Telephone and early-PC sound codings, all our own from their public definitions:
//! G.711 µ-law and A-law (ITU-T G.711, 1972), IMA ADPCM in its WAV (Microsoft's 4-bit
//! `WAVE_FORMAT_IMA_ADPCM` blocks) and QuickTime (`ima4` packets) layouts (IMA Digital Audio
//! Focus and Technical Working Groups, recommended practice 1992), and Microsoft ADPCM
//! (`WAVE_FORMAT_ADPCM`, Multimedia Standards Update 1994). None is patented any longer.
//! Also Flash ADPCM (the SWF File Format Specification's IMA variant with 2 to 5-bit codes)
//! and DVD LPCM at 20 and 24 bits (DVD-Video's grouped big-endian samples).
//!
//! Every block starts afresh, so a seek needs no preroll. Output is interleaved `f32`.
#![allow(clippy::needless_range_loop)]

use crate::audio::SoundDecoder;
pub(crate) use crate::mp4::AdpcmKind;
#[cfg(test)]
use crate::mp4::MS_COEFFS;

/// G.711: one byte a sample.
pub(crate) struct G711 {
    pub alaw: bool,
}

/// A µ-law byte as a 14-bit-range sample scaled to 16 bits (G.711 Table 2a).
pub(crate) fn ulaw(b: u8) -> i16 {
    let u = !b;
    let mantissa = i32::from(u & 0x0F);
    let exponent = i32::from((u >> 4) & 7);
    // The segment's base (33 << exponent, less the bias) plus the mantissa's steps.
    let t = ((mantissa << 3) + 0x84) << exponent;
    (if u & 0x80 != 0 { 0x84 - t } else { t - 0x84 }) as i16
}

/// An A-law byte as a 13-bit-range sample scaled to 16 bits (G.711 Table 1a: even bits
/// inverted, the sign bit 1 for positive).
pub(crate) fn alaw(b: u8) -> i16 {
    let a = b ^ 0x55;
    let mantissa = i32::from(a & 0x0F);
    let exponent = i32::from((a >> 4) & 7);
    let t = match exponent {
        0 => (mantissa << 4) + 8,
        e => ((mantissa << 4) + 0x108) << (e - 1),
    };
    (if a & 0x80 != 0 { t } else { -t }) as i16
}

impl SoundDecoder for G711 {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let f = if self.alaw { alaw } else { ulaw };
        out.extend(sample.iter().map(|&b| f32::from(f(b)) / 32768.0));
        Ok(())
    }
    fn reset(&mut self) {}
}

/// The IMA step sizes: 89 steps growing by about 10% from 7 to 32767.
const IMA_STEP: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

/// How the step index moves after each code's magnitude.
const IMA_INDEX: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

/// One channel's IMA state.
#[derive(Clone, Copy, Default)]
struct Ima {
    predictor: i32,
    index: i32,
}

impl Ima {
    /// Microsoft's encoder and decoder compute the difference in one multiply,
    /// (2·|code| + 1)·step / 8, which rounds once instead of per bit as IMA's own
    /// recommendation (and QuickTime) does.
    fn expand_wav(&mut self, code: u8) -> i16 {
        let step = IMA_STEP[self.index as usize];
        let diff = ((2 * i32::from(code & 7) + 1) * step) >> 3;
        self.step(code, diff)
    }

    fn step(&mut self, code: u8, diff: i32) -> i16 {
        self.predictor = if code & 8 != 0 {
            self.predictor - diff
        } else {
            self.predictor + diff
        }
        .clamp(-32768, 32767);
        self.index = (self.index + IMA_INDEX[usize::from(code & 7)]).clamp(0, 88);
        self.predictor as i16
    }

    fn expand(&mut self, code: u8) -> i16 {
        let step = IMA_STEP[self.index as usize];
        let mut diff = step >> 3;
        if code & 4 != 0 {
            diff += step;
        }
        if code & 2 != 0 {
            diff += step >> 1;
        }
        if code & 1 != 0 {
            diff += step >> 2;
        }
        self.step(code, diff)
    }
}

/// How Microsoft ADPCM's step moves after each code (×256).
const MS_ADAPT: [i32; 16] = [
    230, 230, 230, 230, 307, 409, 512, 614, 768, 614, 512, 409, 307, 230, 230, 230,
];

/// IMA or Microsoft ADPCM: the coded samples are cut into whole blocks (`block` bytes;
/// QuickTime packets are 34 bytes a channel), any remainder kept for the next sample.
pub(crate) struct Adpcm {
    pub kind: AdpcmKind,
    pub channels: usize,
    pub block: usize,
    pub pending: Vec<u8>,
    /// QuickTime's running state per channel, kept across packets.
    qt: [Ima; 8],
    /// CD-ROM XA's last two samples per channel, kept across sound groups.
    xa: [[i32; 2]; 2],
}

impl Adpcm {
    pub(crate) fn new(kind: AdpcmKind, channels: u16, block: u16) -> Result<Adpcm, String> {
        let channels = usize::from(channels.max(1));
        let block = match kind {
            AdpcmKind::ImaQt => 34 * channels,
            AdpcmKind::Xa { .. } => XA_GROUP,
            _ => usize::from(block),
        };
        let header = match kind {
            AdpcmKind::ImaWav => 4 * channels,
            AdpcmKind::ImaQt => 0,
            AdpcmKind::Xa { .. } => 16,
            AdpcmKind::Ms(_) => 7 * channels,
        };
        if channels > 8 || block <= header || matches!(kind, AdpcmKind::Xa { .. }) && channels > 2 {
            return Err(format!("{block}-byte ADPCM blocks"));
        }
        if let AdpcmKind::Ms(c) = &kind
            && c.is_empty()
        {
            return Err("Microsoft ADPCM without coefficients".into());
        }
        Ok(Adpcm {
            kind,
            channels,
            block,
            pending: Vec::new(),
            qt: [Ima::default(); 8],
            xa: [[0; 2]; 2],
        })
    }

    /// Samples a channel in one block.
    fn block_samples(&self) -> usize {
        block_frames(&self.kind, self.block, self.channels)
    }

    fn block(&mut self, b: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let ch = self.channels;
        let at = out.len();
        let push = |out: &mut Vec<f32>, v: i16| out.push(f32::from(v) / 32768.0);
        match &self.kind {
            AdpcmKind::ImaWav => {
                let mut st = [Ima::default(); 8];
                for c in 0..ch {
                    let h = &b[4 * c..4 * c + 4];
                    st[c].predictor = i32::from(i16::from_le_bytes([h[0], h[1]]));
                    st[c].index = i32::from(h[2]).min(88);
                }
                for c in 0..ch {
                    push(out, st[c].predictor as i16);
                }
                // Then groups of 4 bytes (8 samples, low nibble first) per channel in turn.
                let data = &b[4 * ch..];
                let groups = data.len() / (4 * ch);
                let n = groups * 8;
                out.resize(at + (1 + n) * ch, 0.0);
                for g in 0..groups {
                    for c in 0..ch {
                        let bytes = &data[(g * ch + c) * 4..(g * ch + c) * 4 + 4];
                        for (k, &byte) in bytes.iter().enumerate() {
                            for (h, code) in [byte & 15, byte >> 4].into_iter().enumerate() {
                                let i = 1 + g * 8 + 2 * k + h;
                                out[at + i * ch + c] = f32::from(st[c].expand_wav(code)) / 32768.0;
                            }
                        }
                    }
                }
            }
            AdpcmKind::ImaQt => {
                out.resize(at + 64 * ch, 0.0);
                for c in 0..ch {
                    let p = &b[34 * c..34 * c + 34];
                    let head = i16::from_be_bytes([p[0], p[1]]);
                    let (predictor, index) = (i32::from(head & !0x7F), i32::from(head & 0x7F));
                    let st = &mut self.qt[c];
                    // The header keeps only the predictor's top 9 bits: the running state
                    // goes on when it agrees with them (Apple's decoder does the same).
                    if st.index != index || (st.predictor - predictor).abs() > 0x7F {
                        *st = Ima {
                            predictor,
                            index: index.min(88),
                        };
                    }
                    for (k, &byte) in p[2..].iter().enumerate() {
                        for (h, code) in [byte & 15, byte >> 4].into_iter().enumerate() {
                            out[at + (2 * k + h) * ch + c] = f32::from(st.expand(code)) / 32768.0;
                        }
                    }
                }
            }
            AdpcmKind::Xa { eight } => {
                // 8 sound units of 28 4-bit samples (4 of 8-bit samples), each with a
                // header byte (shift, filter) at 4 + unit; the samples of unit u are byte
                // 16 + 4 j + u / 2 (a nibble, low first) or 16 + 4 j + u. In stereo even
                // units are left, odd ones right.
                let units = if *eight { 4 } else { 8 };
                let stereo = ch == 2;
                let mut unit = [0i16; 28];
                for u in 0..units {
                    let head = b[4 + u];
                    let range = match head & 15 {
                        r @ 0..=12 => u32::from(r),
                        _ => 9,
                    };
                    let filter = usize::from((head >> 4) & 3);
                    let (f0, f1) = (XA_POS[filter], XA_NEG[filter]);
                    let c = if stereo { u & 1 } else { 0 };
                    let [mut old, mut older] = self.xa[c];
                    for (j, out) in unit.iter_mut().enumerate() {
                        let t = if *eight {
                            i32::from(b[16 + 4 * j + u] as i8) << 8
                        } else {
                            let byte = b[16 + 4 * j + u / 2];
                            let nib = if u & 1 == 0 { byte << 4 } else { byte & 0xF0 };
                            i32::from(nib as i8) << 8
                        };
                        let v = ((t >> range) + ((old * f0 + older * f1 + 32) >> 6))
                            .clamp(-32768, 32767);
                        older = old;
                        old = v;
                        *out = v as i16;
                    }
                    self.xa[c] = [old, older];
                    if stereo {
                        if c == 0 {
                            let at = out.len();
                            out.resize(at + 56, 0.0);
                            for (j, &v) in unit.iter().enumerate() {
                                out[at + 2 * j] = f32::from(v) / 32768.0;
                            }
                        } else {
                            let at = out.len() - 56;
                            for (j, &v) in unit.iter().enumerate() {
                                out[at + 2 * j + 1] = f32::from(v) / 32768.0;
                            }
                        }
                    } else {
                        for &v in &unit {
                            push(out, v);
                        }
                    }
                }
            }
            AdpcmKind::Ms(coeffs) => {
                let mut pred = [0usize; 8];
                let mut delta = [0i32; 8];
                let mut s1 = [0i32; 8];
                let mut s2 = [0i32; 8];
                let r16 = |o: usize| i32::from(i16::from_le_bytes([b[o], b[o + 1]]));
                for c in 0..ch {
                    pred[c] = usize::from(b[c]);
                    if pred[c] >= coeffs.len() {
                        return Err(format!("ADPCM predictor {}", pred[c]));
                    }
                    delta[c] = r16(ch + 2 * c);
                    s1[c] = r16(3 * ch + 2 * c);
                    s2[c] = r16(5 * ch + 2 * c);
                }
                // The older sample is heard first.
                for c in 0..ch {
                    push(out, s2[c] as i16);
                }
                for c in 0..ch {
                    push(out, s1[c] as i16);
                }
                // Then one nibble a sample, high nibble first, channels in turn.
                let mut c = 0;
                for &byte in &b[7 * ch..] {
                    for code in [byte >> 4, byte & 15] {
                        let [c1, c2] = coeffs[pred[c]];
                        let p = (s1[c] * i32::from(c1) + s2[c] * i32::from(c2)) >> 8;
                        let signed = i32::from(code) - if code & 8 != 0 { 16 } else { 0 };
                        let v = (p + signed * delta[c]).clamp(-32768, 32767);
                        s2[c] = s1[c];
                        s1[c] = v;
                        delta[c] = ((MS_ADAPT[usize::from(code)] * delta[c]) >> 8).max(16);
                        push(out, v as i16);
                        c = (c + 1) % ch;
                    }
                }
                // An odd channel count leaves a part frame: drop it.
                let whole = (out.len() - at) / ch * ch;
                out.truncate(at + whole);
            }
        }
        Ok(())
    }
}

/// Samples a channel in one `block`-byte block (`channels` channels).
pub(crate) fn block_frames(kind: &AdpcmKind, block: usize, channels: usize) -> usize {
    let ch = channels.max(1);
    match kind {
        AdpcmKind::ImaWav => 1 + block.saturating_sub(4 * ch) / (4 * ch) * 8,
        AdpcmKind::ImaQt => 64,
        AdpcmKind::Xa { eight: false } => 224 / ch,
        AdpcmKind::Xa { eight: true } => 112 / ch,
        AdpcmKind::Ms(_) => 2 + block.saturating_sub(7 * ch) * 2 / ch,
    }
}

/// Bytes of a CD-ROM XA sound group.
pub(crate) const XA_GROUP: usize = 128;
/// CD-ROM XA's prediction filters (×64; psx-spx "CDROM XA Audio ADPCM Compression").
const XA_POS: [i32; 4] = [0, 60, 115, 98];
const XA_NEG: [i32; 4] = [0, 0, -52, -55];

impl SoundDecoder for Adpcm {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        // Usually a sample is whole blocks; pieces are joined.
        let mut data = sample;
        let joined;
        if !self.pending.is_empty() {
            self.pending.extend_from_slice(sample);
            joined = std::mem::take(&mut self.pending);
            data = &joined;
        }
        let mut chunks = data.chunks_exact(self.block);
        for b in &mut chunks {
            let at = out.len();
            if self.block(b, out).is_err() {
                // A damaged block: silence of its length keeps the timing.
                out.truncate(at);
                out.resize(at + self.block_samples() * self.channels, 0.0);
            }
        }
        let rest = chunks.remainder();
        self.pending.extend_from_slice(rest);
        Ok(())
    }

    fn reset(&mut self) {
        self.pending.clear();
        self.qt = [Ima::default(); 8];
        self.xa = [[0; 2]; 2];
    }
}

/// How Flash ADPCM's step index moves after each magnitude, for 2 to 5-bit codes (SWF File
/// Format Specification, "ADPCM compression").
const SWF_INDEX: [&[i32]; 4] = [
    &[-1, 2],
    &[-1, -1, 2, 4],
    &[-1, -1, -1, -1, 2, 4, 6, 8],
    &[-1, -1, -1, -1, -1, -1, -1, -1, 1, 2, 4, 6, 8, 10, 13, 16],
];

/// Flash ADPCM (FLV sound format 1, the SWF specification's ADPCMSOUNDDATA): each FLV tag
/// holds a 2-bit code size (2 to 5 bits), then packets of 4096 samples a channel, each
/// starting with a 16-bit sample and a 6-bit step index a channel, the codes interleaved by
/// channel. All fields are read most significant bit first; a packet may end early.
pub(crate) struct SwfAdpcm {
    pub(crate) channels: usize,
}

/// Reads bits most significant first.
struct Bits<'a> {
    data: &'a [u8],
    at: usize,
}

impl Bits<'_> {
    fn read(&mut self, n: u32) -> Option<u32> {
        if self.at + n as usize > self.data.len() * 8 {
            return None;
        }
        let mut v = 0u32;
        for _ in 0..n {
            let b = (self.data[self.at / 8] >> (7 - self.at % 8)) & 1;
            v = (v << 1) | u32::from(b);
            self.at += 1;
        }
        Some(v)
    }
}

impl SoundDecoder for SwfAdpcm {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let ch = self.channels.clamp(1, 2);
        let mut b = Bits {
            data: sample,
            at: 0,
        };
        let bits = b.read(2).ok_or("an empty Flash ADPCM packet")? + 2;
        let table = SWF_INDEX[bits as usize - 2];
        let sign = 1u32 << (bits - 1);
        let mut state = [Ima::default(); 2];
        out.reserve(sample.len() * 8 / bits as usize);
        'packets: loop {
            for s in state.iter_mut().take(ch) {
                let (Some(v), Some(i)) = (b.read(16), b.read(6)) else {
                    break 'packets;
                };
                s.predictor = i32::from(v as u16 as i16);
                s.index = i as i32;
            }
            for s in state.iter().take(ch) {
                out.push(s.predictor as f32 / 32768.0);
            }
            for _ in 1..4096 {
                // A frame without all its channels is the packet's end.
                if b.at + (bits as usize) * ch > sample.len() * 8 {
                    break 'packets;
                }
                for s in state.iter_mut().take(ch) {
                    let code = b.read(bits).unwrap_or(0);
                    let mut step = IMA_STEP[s.index.clamp(0, 88) as usize];
                    let mut diff = 0;
                    let mut k = sign >> 1;
                    while k != 0 {
                        if code & k != 0 {
                            diff += step;
                        }
                        step >>= 1;
                        k >>= 1;
                    }
                    diff += step;
                    s.predictor = if code & sign != 0 {
                        s.predictor - diff
                    } else {
                        s.predictor + diff
                    }
                    .clamp(-32768, 32767);
                    s.index = (s.index + table[(code & (sign - 1)) as usize]).clamp(0, 88);
                    out.push(s.predictor as f32 / 32768.0);
                }
            }
        }
        Ok(())
    }

    fn reset(&mut self) {}
}

/// DVD LPCM at 20 or 24 bits (DVD-Video, private stream 1 sub-streams 0xA0‥0xA7): samples
/// run interleaved in units of four. A unit holds the top 16 bits of its four samples
/// (big-endian, in order), then their low bits: a byte each at 24 bits, a nibble each at 20
/// (the first sample in the high nibble). For stereo a unit is two samples a channel; for
/// other layouts units straddle frames, so the decoder works in groups of whole units and
/// whole frames. Packets split groups anywhere, so the rest of one is carried to the next;
/// after a seek the decoder skips to the next group by the byte count of the samples before
/// (`phase`).
pub(crate) struct DvdLpcm {
    bits: u16,
    channels: usize,
    /// Each sample's start, in bytes from the stream's start, modulo the group size.
    phase: Vec<usize>,
    pending: Vec<u8>,
    aligned: bool,
}

impl DvdLpcm {
    pub(crate) fn new(bits: u16, channels: u16, sizes: impl Iterator<Item = u32>) -> DvdLpcm {
        let channels = usize::from(channels.max(1));
        let group = Self::group_size(bits, channels);
        let mut at = 0usize;
        let phase = sizes
            .map(|n| {
                let p = at;
                at = (at + n as usize) % group;
                p
            })
            .collect();
        DvdLpcm {
            bits,
            channels,
            phase,
            pending: Vec::new(),
            aligned: true,
        }
    }

    /// Bytes in a unit of four samples.
    fn unit(bits: u16) -> usize {
        if bits == 20 { 10 } else { 12 }
    }

    /// Bytes in a group: the fewest whole units that are whole frames.
    fn group_size(bits: u16, channels: usize) -> usize {
        let (mut a, mut b) = (4, channels);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        4 * channels / a / 4 * Self::unit(bits)
    }

    fn group(&self, g: &[u8], out: &mut Vec<f32>) {
        for u in g.chunks_exact(Self::unit(self.bits)) {
            let (hi, lo) = u.split_at(8);
            for i in 0..4 {
                let top = i32::from(i16::from_be_bytes([hi[2 * i], hi[2 * i + 1]])) << 8;
                let low = if self.bits == 20 {
                    let b = lo[i / 2];
                    i32::from(if i % 2 == 0 { b >> 4 } else { b & 15 }) << 4
                } else {
                    i32::from(lo[i])
                };
                out.push((top | low) as f32 / 8_388_608.0);
            }
        }
    }
}

impl SoundDecoder for DvdLpcm {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        self.decode_at(usize::MAX, sample, out)
    }

    fn decode_at(&mut self, index: usize, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let size = Self::group_size(self.bits, self.channels);
        let mut data = sample;
        if !self.aligned {
            self.aligned = true;
            let phase = self.phase.get(index).copied().unwrap_or(0);
            data = &data[((size - phase) % size).min(data.len())..];
        }
        self.pending.extend_from_slice(data);
        let joined = std::mem::take(&mut self.pending);
        let mut groups = joined.chunks_exact(size);
        out.reserve(joined.len() / Self::unit(self.bits) * 4);
        for g in &mut groups {
            self.group(g, out);
        }
        self.pending.extend_from_slice(groups.remainder());
        Ok(())
    }

    fn reset(&mut self) {
        self.pending.clear();
        self.aligned = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g711_reference_points() {
        // G.711 Tables 1a/2a: the extreme codes and the ones next to zero.
        assert_eq!(ulaw(0x00), -32124);
        assert_eq!(ulaw(0x80), 32124);
        assert_eq!(ulaw(0xFF), 0);
        assert_eq!(ulaw(0x7F), 0);
        assert_eq!(ulaw(0xFE), 8);
        assert_eq!(ulaw(0x7E), -8);
        assert_eq!(alaw(0xD5), 8);
        assert_eq!(alaw(0x55), -8);
        assert_eq!(alaw(0xAA), 32256);
        assert_eq!(alaw(0x2A), -32256);
        // Both are monotonic over their positive codes.
        let mut last = -1;
        for code in (0x80u8..=0xFF).rev() {
            let v = i32::from(ulaw(code));
            assert!(v > last || v == 0 && last == -1, "{code:#x}");
            last = v;
        }
    }

    /// Samples `0, 1, 2, …` (times 37) as 20 or 24-bit DVD LPCM units.
    fn lpcm_stream(bits: u16, n: usize) -> Vec<u8> {
        let mut s = Vec::new();
        for u in 0..n / 4 {
            let v: Vec<i32> = (0..4).map(|i| ((u * 4 + i) as i32 - 50) * 37).collect();
            for x in &v {
                s.extend_from_slice(&((x >> (bits - 16)) as i16).to_be_bytes());
            }
            if bits == 24 {
                s.extend(v.iter().map(|x| *x as u8));
            } else {
                s.push(((v[0] & 15) << 4 | (v[1] & 15)) as u8);
                s.push(((v[2] & 15) << 4 | (v[3] & 15)) as u8);
            }
        }
        s
    }

    #[test]
    fn dvd_lpcm_units_straddle_packets_and_seeks_realign() {
        for (bits, ch) in [(24u16, 6u16), (20, 6), (24, 2), (20, 1), (24, 3)] {
            let n = 4 * 3 * 60;
            let stream = lpcm_stream(bits, n);
            let pieces: Vec<&[u8]> = stream.chunks(77).collect();
            let mut d = DvdLpcm::new(bits, ch, pieces.iter().map(|p| p.len() as u32));
            let mut all = Vec::new();
            for (k, p) in pieces.iter().enumerate() {
                let at = all.len();
                d.decode_at(k, p, &mut all).unwrap();
                assert_eq!((all.len() - at) % usize::from(ch), 0, "whole frames");
            }
            assert_eq!(all.len(), n, "{bits}-bit {ch} ch");
            let scale = f32::from(1u16 << (24 - bits));
            for (i, v) in all.iter().enumerate() {
                let want = ((i as i32 - 50) * 37) as f32 * scale / 8_388_608.0;
                assert_eq!(*v, want, "{bits}-bit {ch} ch sample {i}");
            }
            // A seek to piece 5: the decode picks up at the next whole group.
            d.reset();
            let mut tail = Vec::new();
            for (k, p) in pieces.iter().enumerate().skip(5) {
                d.decode_at(k, p, &mut tail).unwrap();
            }
            assert_eq!(tail.len() % usize::from(ch), 0);
            assert_eq!(
                tail[..],
                all[all.len() - tail.len()..],
                "{bits}-bit {ch} ch"
            );
            assert!(all.len() - tail.len() <= 5 * 77 * 8 / usize::from(bits) + 4 * 12);
        }
    }

    #[test]
    fn ima_steps_grow_and_index_stays_in_range() {
        assert!(IMA_STEP.windows(2).all(|w| w[0] < w[1]));
        let mut s = Ima::default();
        for _ in 0..200 {
            s.expand(7);
        }
        assert_eq!((s.predictor, s.index), (32767, 88));
        for _ in 0..200 {
            s.expand(8);
        }
        assert_eq!(s.index, 0);
    }

    #[test]
    fn short_blocks_are_refused_and_junk_never_panics() {
        assert!(Adpcm::new(AdpcmKind::ImaWav, 2, 8).is_err());
        assert!(Adpcm::new(AdpcmKind::Ms(Vec::new()), 1, 256).is_err());
        let mut seed = 7u32;
        let junk: Vec<u8> = (0..20_000)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 24) as u8
            })
            .collect();
        for kind in [
            AdpcmKind::ImaWav,
            AdpcmKind::ImaQt,
            AdpcmKind::Ms(MS_COEFFS.to_vec()),
        ] {
            for ch in [1u16, 2, 3] {
                let mut d = Adpcm::new(kind.clone(), ch, 1024).unwrap();
                let mut out = Vec::new();
                for c in junk.chunks(777) {
                    let _ = d.decode(c, &mut out);
                }
                assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
                assert_eq!(out.len() % usize::from(ch), 0);
            }
        }
    }
}
