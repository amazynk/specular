//! QOA, the "Quite OK Audio" format (`.qoa`), from its published specification (MIT).
//!
//! Big-endian. An 8-byte file header: `qoaf` and the samples per channel (0 for a stream of
//! unknown length). Then frames: an 8-byte header (channels, a 24-bit sample rate, the
//! samples per channel in the frame, the frame's bytes), each channel's LMS state (four
//! 16-bit history values, then four weights), and slices of 64 bits, the channels
//! interleaved slice by slice: a 4-bit scale factor and twenty 3-bit residuals.
//!
//! Each sample is predicted from the last four by the LMS weights (`Σ w·h >> 13`), the
//! dequantised residual added and the result clamped to 16 bits; the weights then move by
//! the residual over 16 in the direction of each history value's sign. A track's samples
//! are whole frames, each decoded on its own (its state is in its header).

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::{AudioCodec, AudioTrack, Error, Result};
use crate::sound_file::{MAX_SAMPLES, sample, track};

const FRAME_HEADER: usize = 8;
const SLICE_SAMPLES: usize = 20;

/// round(s^2.75) for s = 1..=16.
const SCALE: [i32; 16] = [
    1, 7, 21, 45, 84, 138, 211, 304, 421, 562, 731, 928, 1157, 1419, 1715, 2048,
];
/// The residual of each 3-bit code, before scaling.
const DEQUANT: [f64; 8] = [0.75, -0.75, 2.5, -2.5, 4.5, -4.5, 7.0, -7.0];

pub(crate) fn is_qoa(c: &AudioCodec) -> bool {
    matches!(c, AudioCodec::Other(t, _) if t == b"qoaf")
}

/// `qoaf` and a first frame header with channels and a sample rate.
pub(crate) fn sniff(d: &[u8]) -> bool {
    d.len() >= 16 && d.starts_with(b"qoaf") && d[8] > 0 && (d[9] | d[10] | d[11]) != 0
}

fn u16be(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

/// The dequantised residual of code `q` at scale factor `sf` (rounded away from zero).
fn dequant(sf: usize, q: usize) -> i32 {
    let v = f64::from(SCALE[sf]) * DEQUANT[q];
    (v.abs() + 0.5).floor().copysign(v) as i32
}

/// A `.qoa` file: one track sample per frame.
pub(crate) fn qoa(d: &[u8]) -> Result<AudioTrack> {
    if !sniff(d) {
        return Err(Error::Malformed("not a QOA file".into()));
    }
    let channels = d[8];
    let rate = u32::from_be_bytes([0, d[9], d[10], d[11]]);
    let mut samples = Vec::new();
    let mut at = 8;
    let mut t = 0u64;
    while at + FRAME_HEADER <= d.len() && samples.len() < MAX_SAMPLES {
        let (Some(n), Some(size)) = (u16be(d, at + 4), u16be(d, at + 6)) else {
            break;
        };
        let size = usize::from(size);
        if d[at] != channels || size <= FRAME_HEADER {
            return Err(Error::Unsupported(
                "QOA frames that change the channels are not supported".into(),
            ));
        }
        let end = (at + size).min(d.len());
        samples.push(sample(at, end - at, t, u64::from(n)));
        t += u64::from(n);
        at += size;
    }
    if samples.is_empty() {
        return Err(Error::Malformed("QOA file without a frame".into()));
    }
    Ok(track(
        AudioCodec::Other(*b"qoaf", 0),
        u16::from(channels),
        rate,
        samples,
    ))
}

/// Decodes one frame into interleaved samples; returns how many per channel.
pub fn decode_frame(f: &[u8], out: &mut Vec<i16>) -> std::result::Result<usize, String> {
    if f.len() < FRAME_HEADER {
        return Err("QOA frame header cut short".into());
    }
    let ch = usize::from(f[0]);
    let n = usize::from(u16::from_be_bytes([f[4], f[5]]));
    if ch == 0 {
        return Err("QOA frame without channels".into());
    }
    let mut hist = vec![[0i32; 4]; ch];
    let mut weights = vec![[0i32; 4]; ch];
    let mut p = FRAME_HEADER;
    for c in 0..ch {
        let lms = f
            .get(p..p + 16)
            .ok_or_else(|| "QOA frame state cut short".to_string())?;
        for i in 0..4 {
            hist[c][i] = i32::from(i16::from_be_bytes([lms[i * 2], lms[i * 2 + 1]]));
            weights[c][i] = i32::from(i16::from_be_bytes([lms[8 + i * 2], lms[9 + i * 2]]));
        }
        p += 16;
    }
    let base = out.len();
    out.resize(base + n * ch, 0);
    let slices = n.div_ceil(SLICE_SAMPLES);
    for s in 0..slices {
        for c in 0..ch {
            let Some(b) = f.get(p..p + 8) else {
                // A frame cut short: what it holds.
                return Ok(n);
            };
            p += 8;
            let slice = u64::from_be_bytes(b.try_into().unwrap_or_default());
            let sf = (slice >> 60) as usize;
            let (h, w) = (&mut hist[c], &mut weights[c]);
            let first = s * SLICE_SAMPLES;
            for j in 0..SLICE_SAMPLES.min(n - first) {
                let q = ((slice >> (57 - 3 * j)) & 7) as usize;
                let pred = (0..4)
                    .map(|i| i64::from(w[i]) * i64::from(h[i]))
                    .sum::<i64>()
                    >> 13;
                let r = dequant(sf, q);
                let v = (pred + i64::from(r)).clamp(-32768, 32767) as i32;
                let delta = r >> 4;
                for i in 0..4 {
                    w[i] += if h[i] < 0 { -delta } else { delta };
                }
                h.copy_within(1..4, 0);
                h[3] = v;
                out[base + (first + j) * ch + c] = v as i16;
            }
        }
    }
    Ok(n)
}

struct Qoa {
    buf: Vec<i16>,
}

impl SoundDecoder for Qoa {
    fn decode(&mut self, s: &[u8], out: &mut Vec<f32>) -> std::result::Result<(), String> {
        self.buf.clear();
        decode_frame(s, &mut self.buf)?;
        out.extend(self.buf.iter().map(|&v| f32::from(v) / 32768.0));
        Ok(())
    }
    fn reset(&mut self) {}
}

pub(crate) fn decoder(
    t: &AudioTrack,
) -> std::result::Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    Ok((
        Box::new(Qoa { buf: Vec::new() }),
        t.channels.max(1),
        t.sample_rate,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dequantisation_table() {
        assert_eq!(
            (0..8).map(|q| dequant(0, q)).collect::<Vec<_>>(),
            [1, -1, 3, -3, 5, -5, 7, -7]
        );
        assert_eq!(
            (0..8).map(|q| dequant(1, q)).collect::<Vec<_>>(),
            [5, -5, 18, -18, 32, -32, 49, -49]
        );
        assert_eq!(dequant(15, 7), -14336);
    }

    #[test]
    fn one_frame() {
        // Mono, 44.1 kHz, 20 samples: zero state, one slice of code 0 at scale 0.
        let mut f = vec![1, 0, 0xAC, 0x44, 0, 20, 0, 32];
        f.extend_from_slice(&[0; 16]);
        f.extend_from_slice(&[0; 8]);
        let mut out = Vec::new();
        assert_eq!(decode_frame(&f, &mut out).unwrap(), 20);
        // With no history the prediction is 0: each sample adds its residual.
        assert!(out.iter().all(|&v| v == 1));
        let mut file = b"qoaf\0\0\0\x14".to_vec();
        file.extend_from_slice(&f);
        let t = qoa(&file).unwrap();
        assert_eq!((t.channels, t.sample_rate, t.samples.len()), (1, 44100, 1));
    }
}
