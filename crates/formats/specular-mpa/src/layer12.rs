//! Layers I and II: bit allocation, scale factors and requantisation (ISO/IEC 11172-3
//! §2.4.1.5–§2.4.3.3, Tables 3-B.2a–d and 3-B.4; ISO/IEC 13818-3 Table B.1 for the lower
//! sampling rates).

use crate::synth::Synth;
use crate::{Error, Header, Version};

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn u(&mut self, n: u32) -> Result<u32, Error> {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = *self
                .d
                .get(self.pos >> 3)
                .ok_or_else(|| Error::Malformed("frame too short".into()))?;
            v = (v << 1) | u32::from((byte >> (7 - (self.pos & 7))) & 1);
            self.pos += 1;
        }
        Ok(v)
    }
}

/// The quantisation steps an allocation index selects, per row of the allocation tables.
const ROW_A: &[u16] = &[
    0, 3, 7, 15, 31, 63, 127, 255, 511, 1023, 2047, 4095, 8191, 16383, 32767, 65535,
];
const ROW_B: &[u16] = &[
    0, 3, 5, 7, 9, 15, 31, 63, 127, 255, 511, 1023, 2047, 4095, 8191, 65535,
];
const ROW_C: &[u16] = &[0, 3, 5, 7, 9, 15, 31, 65535];
const ROW_D: &[u16] = &[0, 3, 5, 65535];
const ROW_E: &[u16] = &[
    0, 3, 5, 9, 15, 31, 63, 127, 255, 511, 1023, 2047, 4095, 8191, 16383, 32767,
];
const ROW_F: &[u16] = &[0, 3, 5, 9, 15, 31, 63, 127];
const ROW_H: &[u16] = &[
    0, 3, 5, 7, 9, 15, 31, 63, 127, 255, 511, 1023, 2047, 4095, 8191, 16383,
];
const ROW_G: &[u16] = &[0, 3, 5, 9];

/// Table 3-B.2a–d and the lower-rate table: the row used by each subband (the table's length
/// is its sblimit).
fn table(h: &Header) -> Vec<&'static [u16]> {
    let rows = |spec: &[(usize, &'static [u16])]| -> Vec<&'static [u16]> {
        spec.iter()
            .flat_map(|&(n, r)| std::iter::repeat_n(r, n))
            .collect()
    };
    if h.version != Version::Mpeg1 {
        return rows(&[(4, ROW_H), (7, ROW_F), (19, ROW_G)]);
    }
    let per_channel = h.kbps() / h.channels() as u32;
    let fs = h.sample_rate;
    let low = per_channel <= 48;
    match (fs, low) {
        (48000, false) => rows(&[(3, ROW_A), (8, ROW_B), (12, ROW_C), (4, ROW_D)]),
        (_, false) if per_channel <= 80 => rows(&[(3, ROW_A), (8, ROW_B), (12, ROW_C), (4, ROW_D)]),
        (_, false) => rows(&[(3, ROW_A), (8, ROW_B), (12, ROW_C), (7, ROW_D)]),
        (32000, true) => rows(&[(2, ROW_E), (10, ROW_F)]),
        (_, true) => rows(&[(2, ROW_E), (6, ROW_F)]),
    }
}

/// A quantisation class: bits per codeword, and whether three samples share one codeword.
fn class(steps: u16) -> (u32, bool) {
    match steps {
        3 => (5, true),
        5 => (7, true),
        9 => (10, true),
        s => (16 - s.leading_zeros(), false),
    }
}

/// Scale factor `i` (Table 3-B.1): 2^(1 - i/3).
fn scalefactor(i: u32) -> f32 {
    if i >= 63 {
        return 0.0;
    }
    2f32.powf(1.0 - i as f32 / 3.0)
}

/// A code of a quantiser with `steps` levels as a fraction in (-1, 1).
#[inline]
fn requantise(code: u32, steps: u32) -> f32 {
    (2.0 * code as f32 - (steps - 1) as f32) / steps as f32
}

/// Decodes one Layer I or II frame and appends its PCM (interleaved) to `out`.
pub(crate) fn decode_frame(
    h: &Header,
    frame: &[u8],
    synth: &mut [Synth; 2],
    out: &mut Vec<f32>,
) -> Result<(), Error> {
    let mut r = Bits {
        d: frame,
        pos: 32 + if h.crc { 16 } else { 0 },
    };
    let nch = h.channels();
    if h.layer == 1 {
        layer1(h, &mut r, nch, synth, out)
    } else {
        layer2(h, &mut r, nch, synth, out)
    }
}

/// The joint stereo bound: subbands from here on carry one set of samples.
fn bound(h: &Header, sblimit: usize) -> usize {
    if h.mode == 1 {
        (4 * (usize::from(h.mode_extension) + 1)).min(sblimit)
    } else {
        sblimit
    }
}

fn emit(nch: usize, sb: &[[f32; 32]; 2], synth: &mut [Synth; 2], out: &mut Vec<f32>) {
    let mut pcm = [[0f32; 32]; 2];
    for ch in 0..nch {
        synth[ch].run(&sb[ch], &mut pcm[ch]);
    }
    for j in 0..32 {
        for p in pcm.iter().take(nch) {
            out.push(p[j].clamp(-1.0, 1.0));
        }
    }
}

fn layer1(
    h: &Header,
    r: &mut Bits,
    nch: usize,
    synth: &mut [Synth; 2],
    out: &mut Vec<f32>,
) -> Result<(), Error> {
    let bound = bound(h, 32);
    let mut alloc = [[0u32; 32]; 2];
    for sb in 0..32 {
        if sb < bound {
            for ch in 0..nch {
                alloc[ch][sb] = r.u(4)?;
            }
        } else {
            let a = r.u(4)?;
            alloc[0][sb] = a;
            alloc[1][sb] = a;
        }
    }
    if alloc.iter().flatten().any(|&a| a == 15) {
        return Err(Error::Malformed("forbidden allocation".into()));
    }
    let mut scf = [[0f32; 32]; 2];
    for sb in 0..32 {
        for ch in 0..nch {
            if alloc[ch][sb] != 0 {
                scf[ch][sb] = scalefactor(r.u(6)?);
            }
        }
    }
    for _ in 0..12 {
        let mut s = [[0f32; 32]; 2];
        for sb in 0..32 {
            let chans = if sb < bound { nch } else { 1 };
            for ch in 0..chans {
                let a = alloc[ch][sb];
                if a == 0 {
                    continue;
                }
                let nb = a + 1;
                let v = requantise(r.u(nb)?, (1 << nb) - 1);
                if sb < bound {
                    s[ch][sb] = v * scf[ch][sb];
                } else {
                    for c in 0..nch {
                        s[c][sb] = v * scf[c][sb];
                    }
                }
            }
        }
        emit(nch, &s, synth, out);
    }
    Ok(())
}

fn layer2(
    h: &Header,
    r: &mut Bits,
    nch: usize,
    synth: &mut [Synth; 2],
    out: &mut Vec<f32>,
) -> Result<(), Error> {
    let rows = table(h);
    let sblimit = rows.len();
    let bound = bound(h, sblimit);
    // Steps per channel and subband (0: not coded).
    let mut steps = [[0u16; 32]; 2];
    for sb in 0..sblimit {
        let row = rows[sb];
        let nbal = row.len().trailing_zeros();
        if sb < bound {
            for ch in 0..nch {
                steps[ch][sb] = row[r.u(nbal)? as usize];
            }
        } else {
            let s = row[r.u(nbal)? as usize];
            steps[0][sb] = s;
            steps[1][sb] = s;
        }
    }
    let mut scfsi = [[0u32; 32]; 2];
    for sb in 0..sblimit {
        for ch in 0..nch {
            if steps[ch][sb] != 0 {
                scfsi[ch][sb] = r.u(2)?;
            }
        }
    }
    let mut scf = [[[0f32; 3]; 32]; 2];
    for sb in 0..sblimit {
        for ch in 0..nch {
            if steps[ch][sb] == 0 {
                continue;
            }
            let s = &mut scf[ch][sb];
            match scfsi[ch][sb] {
                0 => {
                    for v in s.iter_mut() {
                        *v = scalefactor(r.u(6)?);
                    }
                }
                1 => {
                    let a = scalefactor(r.u(6)?);
                    let b = scalefactor(r.u(6)?);
                    *s = [a, a, b];
                }
                2 => {
                    let a = scalefactor(r.u(6)?);
                    *s = [a, a, a];
                }
                _ => {
                    let a = scalefactor(r.u(6)?);
                    let b = scalefactor(r.u(6)?);
                    *s = [a, b, b];
                }
            }
        }
    }
    for gr in 0..12 {
        let part = gr / 4;
        let mut s = [[[0f32; 32]; 3]; 2];
        for sb in 0..sblimit {
            let chans = if sb < bound { nch } else { 1 };
            for ch in 0..chans {
                let n = steps[ch][sb];
                if n == 0 {
                    continue;
                }
                let n32 = u32::from(n);
                let (bits, grouped) = class(n);
                let mut v = [0f32; 3];
                if grouped {
                    let mut c = r.u(bits)?;
                    for x in &mut v {
                        *x = requantise(c % n32, n32);
                        c /= n32;
                    }
                } else {
                    for x in &mut v {
                        *x = requantise(r.u(bits)?, n32);
                    }
                }
                for (t, &x) in v.iter().enumerate() {
                    if sb < bound {
                        s[ch][t][sb] = x * scf[ch][sb][part];
                    } else {
                        for c in 0..nch {
                            s[c][t][sb] = x * scf[c][sb][part];
                        }
                    }
                }
            }
        }
        for t in 0..3 {
            emit(nch, &[s[0][t], s[1][t]], synth, out);
        }
    }
    Ok(())
}
