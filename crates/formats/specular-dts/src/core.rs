//! One core frame (ETSI TS 102 114 clause 5): the bit stream header (5.4.2), the primary
//! audio coding header (5.4.3), then per subframe the side information (5.5.1), the LFE
//! samples and the subband samples (5.6), joint intensity and sum/difference decoding
//! (C.3.4, C.3.5), the 32-band synthesis filter bank (C.3.6) and the LFE interpolation
//! (C.3.7).

use std::sync::OnceLock;

use crate::Error;
use crate::bits::Bits;
use crate::tables as t;

fn bad(s: &str) -> Error {
    Error::Malformed(s.into())
}

/// Sampling rates by SFREQ (Table 5-5); 0 is invalid.
const RATES: [u32; 16] = [
    0, 8000, 16000, 32000, 0, 0, 11025, 22050, 44100, 0, 0, 12000, 24000, 48000, 0, 0,
];

/// Where each primary channel of an AMODE (Table 5-4, bitstream order) goes in the output
/// (WAVE order: L R C LFE then the surrounds), and the output's channel count without LFE.
fn layout(amode: u8) -> Option<(&'static [usize], usize)> {
    Some(match amode {
        0 => (&[0], 1),
        1..=4 => (&[0, 1], 2),
        5 => (&[2, 0, 1], 3),
        6 => (&[0, 1, 2], 3),
        7 => (&[2, 0, 1, 3], 4),
        8 => (&[0, 1, 2, 3], 4),
        9 => (&[2, 0, 1, 3, 4], 5),
        _ => return None,
    })
}

/// The bit stream header (Table 5-1) of a core frame, in 16-bit big-endian form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    /// Bytes of the frame (FSIZE + 1), core and any extensions inside it.
    pub size: usize,
    /// PCM samples a channel (32 × (NBLKS + 1)).
    pub samples: usize,
    /// AMODE (Table 5-4).
    pub amode: u8,
    pub sample_rate: u32,
    /// LFE present, with its interpolation (1: 128×, 2: 64×).
    pub lfe: u8,
    /// Output channels: the primary ones and LFE.
    pub channels: usize,
    /// An extension (XCh, X96, XXCh) follows the core audio in the frame.
    pub ext_audio: bool,
    /// The RATE code: 29 is open, 31 lossless.
    pub rate_code: u8,
    pub(crate) crc: bool,
    pub(crate) aspf: bool,
    pub(crate) dynf: bool,
    pub(crate) filts: bool,
    pub(crate) sumf: bool,
    pub(crate) sums: bool,
}

impl FrameInfo {
    /// Parses the header at the start of `b` (a sync word first).
    pub fn parse(b: &[u8]) -> Option<FrameInfo> {
        let mut r = Bits::new(b.get(..16)?);
        if r.u(32) != crate::SYNC_CORE {
            return None;
        }
        let _ftype = r.u(1);
        let _short = r.u(5);
        let crc = r.u(1) != 0;
        let nblks = r.u(7) as usize;
        let fsize = r.u(14) as usize;
        let amode = r.u(6) as u8;
        let sfreq = r.u(4) as usize;
        let rate_code = r.u(5) as u8;
        let _fixed = r.u(1);
        let dynf = r.u(1) != 0;
        let _timef = r.u(1);
        let _auxf = r.u(1);
        let _hdcd = r.u(1);
        let _ext_id = r.u(3);
        let ext_audio = r.u(1) != 0;
        let aspf = r.u(1) != 0;
        let lfe = r.u(2) as u8;
        let _hflag = r.u(1);
        if crc {
            r.u(16);
        }
        let filts = r.u(1) != 0;
        let _vernum = r.u(4);
        let _chist = r.u(2);
        let _pcmr = r.u(3);
        let sumf = r.u(1) != 0;
        let sums = r.u(1) != 0;
        let rate = RATES[sfreq];
        if nblks < 5 || fsize < 95 || rate == 0 || lfe == 3 {
            return None;
        }
        let primary = layout(amode).map_or(0, |l| l.1);
        Some(FrameInfo {
            size: fsize + 1,
            samples: 32 * (nblks + 1),
            amode,
            sample_rate: rate,
            lfe,
            channels: primary + usize::from(lfe > 0),
            ext_audio,
            rate_code,
            crc,
            aspf,
            dynf,
            filts,
            sumf,
            sums,
        })
    }

    /// Bits of the header (with HCRC and the dialogue normalisation field).
    fn bits(&self) -> usize {
        // 32 sync + 6 + 1 + 7 + 14 + 6 + 4 + 5 + 10 flags + 2 + 1, then 1 + 4 + 2 + 3 + 1 + 1
        // + 4 (DIALNORM or unspecified).
        let base = 32 + 6 + 1 + 7 + 14 + 6 + 4 + 5 + 10 + 2 + 1 + 1 + 4 + 2 + 3 + 1 + 1 + 4;
        base + if self.crc { 16 } else { 0 }
    }
}

/// A Huffman code book as a binary tree: `>= 0` next node, `< 0` leaf `-(index + 1)`.
struct Vlc {
    nodes: Vec<[i32; 2]>,
    levels: Vec<i32>,
}

impl Vlc {
    fn new(words: &[(i8, u8, u16)]) -> Vlc {
        let mut nodes = vec![[0i32; 2]];
        let mut levels = Vec::new();
        for &(level, len, code) in words {
            levels.push(i32::from(level));
            let leaf = -(levels.len() as i32);
            let mut at = 0usize;
            for i in (0..len).rev() {
                let bit = usize::from((code >> i) & 1);
                if i == 0 {
                    nodes[at][bit] = leaf;
                } else {
                    if nodes[at][bit] == 0 {
                        nodes.push([0, 0]);
                        nodes[at][bit] = (nodes.len() - 1) as i32;
                    }
                    at = nodes[at][bit] as usize;
                }
            }
        }
        Vlc { nodes, levels }
    }

    fn decode(&self, r: &mut Bits) -> Result<i32, Error> {
        let mut at = 0usize;
        for _ in 0..24 {
            let n = self.nodes[at][r.u(1) as usize];
            if n < 0 {
                return Ok(self.levels[(-n - 1) as usize]);
            }
            if n == 0 || r.overrun() {
                break;
            }
            at = n as usize;
        }
        Err(bad("bad Huffman code"))
    }
}

struct Books {
    tmode: [Vlc; 4],
    scale: [Vlc; 5],
    bitalloc: [Vlc; 5],
    /// By ABITS − 1 (1..=10), the Huffman books in SEL order.
    audio: Vec<Vec<Vlc>>,
}

fn books() -> &'static Books {
    static B: OnceLock<Books> = OnceLock::new();
    B.get_or_init(|| {
        let v = |w: &[(i8, u8, u16)]| Vlc::new(w);
        Books {
            tmode: [v(&t::H_A4), v(&t::H_B4), v(&t::H_C4), v(&t::H_D4)],
            scale: [
                v(&t::H_SA129),
                v(&t::H_SB129),
                v(&t::H_SC129),
                v(&t::H_SD129),
                v(&t::H_SE129),
            ],
            bitalloc: [
                v(&t::H_A12),
                v(&t::H_B12),
                v(&t::H_C12),
                v(&t::H_D12),
                v(&t::H_E12),
            ],
            audio: vec![
                vec![v(&t::H_A3)],
                vec![v(&t::H_A5), v(&t::H_B5), v(&t::H_C5)],
                vec![v(&t::H_A7), v(&t::H_B7), v(&t::H_C7)],
                vec![v(&t::H_A9), v(&t::H_B9), v(&t::H_C9)],
                vec![v(&t::H_A13), v(&t::H_B13), v(&t::H_C13)],
                vec![
                    v(&t::H_A17),
                    v(&t::H_B17),
                    v(&t::H_C17),
                    v(&t::H_D17),
                    v(&t::H_E17),
                    v(&t::H_F17),
                    v(&t::H_G17),
                ],
                vec![
                    v(&t::H_A25),
                    v(&t::H_B25),
                    v(&t::H_C25),
                    v(&t::H_D25),
                    v(&t::H_E25),
                    v(&t::H_F25),
                    v(&t::H_G25),
                ],
                vec![
                    v(&t::H_A33),
                    v(&t::H_B33),
                    v(&t::H_C33),
                    v(&t::H_D33),
                    v(&t::H_E33),
                    v(&t::H_F33),
                    v(&t::H_G33),
                ],
                vec![
                    v(&t::H_A65),
                    v(&t::H_B65),
                    v(&t::H_C65),
                    v(&t::H_D65),
                    v(&t::H_E65),
                    v(&t::H_F65),
                    v(&t::H_G65),
                ],
                vec![
                    v(&t::H_A129),
                    v(&t::H_B129),
                    v(&t::H_C129),
                    v(&t::H_D129),
                    v(&t::H_E129),
                    v(&t::H_F129),
                    v(&t::H_G129),
                ],
            ],
        }
    })
}

/// Quantization levels of the block codes by ABITS − 1 (Table 5-26) and their code bits
/// (clause D.6).
const BLOCK_LEVELS: [i32; 7] = [3, 5, 7, 9, 13, 17, 25];
const BLOCK_BITS: [u32; 7] = [7, 10, 12, 13, 15, 17, 19];

/// Scale factor adjustment (Table 5-27).
const ADJ: [f32; 4] = [1.0, 1.125, 1.25, 1.4375];

/// The 32-band synthesis of one channel (C.3.6) and its history.
#[derive(Clone)]
pub(crate) struct Qmf {
    x: [f32; 512],
    z: [f32; 64],
}

impl Default for Qmf {
    fn default() -> Qmf {
        Qmf {
            x: [0.0; 512],
            z: [0.0; 64],
        }
    }
}

/// The cosine modulation coefficients of C.3.6.
struct CosMod {
    a: [[f32; 16]; 16],
    b: [[f32; 16]; 16],
    sum: [f32; 16],
    diff: [f32; 16],
}

fn cos_mod() -> &'static CosMod {
    static C: OnceLock<CosMod> = OnceLock::new();
    C.get_or_init(|| {
        let pi = std::f64::consts::PI;
        let mut c = CosMod {
            a: [[0.0; 16]; 16],
            b: [[0.0; 16]; 16],
            sum: [0.0; 16],
            diff: [0.0; 16],
        };
        for k in 0..16 {
            for i in 0..16 {
                c.a[k][i] = (((2 * i + 1) * (2 * k + 1)) as f64 * pi / 64.0).cos() as f32;
                c.b[k][i] = ((i * (2 * k + 1)) as f64 * pi / 32.0).cos() as f32;
            }
            let w = (2 * k + 1) as f64 * pi / 128.0;
            c.sum[k] = (0.25 / (2.0 * w.cos())) as f32;
            c.diff[k] = (-0.25 / (2.0 * w.sin())) as f32;
        }
        c
    })
}

impl Qmf {
    /// 32 PCM samples from one sample of each of the 32 subbands.
    fn run(&mut self, xin: &[f32; 32], coef: &[f32; 512], out: &mut [f32]) {
        let c = cos_mod();
        let mut a = [0f32; 16];
        let mut b = [0f32; 16];
        for k in 0..16 {
            let mut s = 0.0;
            for i in 0..16 {
                s += (xin[2 * i] + xin[2 * i + 1]) * c.a[k][i];
            }
            a[k] = s;
            let mut s = xin[0] * c.b[k][0];
            for i in 1..16 {
                s += (xin[2 * i] + xin[2 * i - 1]) * c.b[k][i];
            }
            b[k] = s;
        }
        for k in 0..16 {
            self.x[k] = c.sum[k] * (a[k] + b[k]);
            self.x[31 - k] = c.diff[k] * (a[k] - b[k]);
        }
        for i in 0..32 {
            let k = 31 - i;
            let (mut lo, mut hi) = (0f32, 0f32);
            for j in (0..512).step_by(64) {
                lo += coef[i + j] * (self.x[i + j] - self.x[j + k]);
                hi += coef[32 + i + j] * (-self.x[i + j] - self.x[j + k]);
            }
            self.z[i] += lo;
            self.z[32 + i] += hi;
        }
        out[..32].copy_from_slice(&self.z[..32]);
        self.x.copy_within(0..480, 32);
        self.z.copy_within(32..64, 0);
        self.z[32..].fill(0.0);
    }
}

/// What a decoder keeps between frames.
#[derive(Clone, Default)]
pub(crate) struct State {
    qmf: Vec<Qmf>,
    /// The last decimated LFE samples (newest last).
    lfe_hist: [f32; 8],
}

/// Decodes the core of one frame (`b`: the frame in 16-bit big-endian form) into `pcm`:
/// `info.samples` samples a channel, interleaved in [`FrameInfo::channels`] order, as
/// floats at full scale 1.0. With `partial`, frames using the tools whose code books are
/// not published decode without them (for checking the rest against an oracle).
pub(crate) fn decode(
    b: &[u8],
    info: &FrameInfo,
    st: &mut State,
    pcm: &mut Vec<f32>,
    partial: bool,
) -> Result<(), Error> {
    let Some((map, nprim)) = layout(info.amode) else {
        return Err(Error::Unsupported(format!(
            "DTS channel arrangement {} (beyond 5.1)",
            info.amode
        )));
    };
    let bk = books();
    let mut r = Bits::new(b);
    r.skip(info.bits());
    // Primary audio coding header (Table 5-21).
    let nsubfs = r.u(4) as usize + 1;
    let npchs = r.u(3) as usize + 1;
    if npchs > 5 {
        return Err(bad("more than five primary channels"));
    }
    if npchs > map.len() {
        return Err(bad("more primary channels than the arrangement has"));
    }
    let mut subs = [0usize; 5];
    let mut vqsub = [0usize; 5];
    let mut joinx = [0usize; 5];
    let mut thuff = [0usize; 5];
    let mut shuff = [0usize; 5];
    let mut bhuff = [0usize; 5];
    for v in subs.iter_mut().take(npchs) {
        *v = r.u(5) as usize + 2;
        if *v > 32 {
            return Err(bad("more than 32 subbands"));
        }
    }
    for ch in 0..npchs {
        vqsub[ch] = (r.u(5) as usize + 1).min(subs[ch]);
    }
    for ch in 0..npchs {
        joinx[ch] = r.u(3) as usize;
        if joinx[ch] > ch {
            // The source channel comes first (Table 5-22).
            return Err(bad("joint intensity from a later channel"));
        }
    }
    for v in thuff.iter_mut().take(npchs) {
        *v = r.u(2) as usize;
    }
    for v in shuff.iter_mut().take(npchs) {
        *v = r.u(3) as usize;
        if *v == 7 {
            return Err(bad("invalid scale factor code book"));
        }
    }
    for v in bhuff.iter_mut().take(npchs) {
        *v = r.u(3) as usize;
        if *v == 7 {
            return Err(bad("invalid bit allocation code book"));
        }
    }
    let mut sel = [[0usize; 26]; 5];
    for n in 0..10 {
        let w = match n {
            0 => 1,
            1..=4 => 2,
            _ => 3,
        };
        for row in sel.iter_mut().take(npchs) {
            row[n] = r.u(w) as usize;
        }
    }
    let mut adj = [[1f32; 26]; 5];
    for n in 0..10 {
        let huff = match n {
            0 => 1,
            1..=4 => 3,
            _ => 7,
        };
        for ch in 0..npchs {
            if sel[ch][n] < huff {
                adj[ch][n] = ADJ[r.u(2) as usize];
            }
        }
    }
    if info.crc {
        r.skip(16);
    }
    let step_table: &[u32; 32] = if info.rate_code == 31 {
        &t::STEP_LOSSLESS
    } else {
        &t::STEP_LOSSY
    };
    let qmf_coef: &[f32; 512] = if info.filts {
        &t::QMF_PERFECT
    } else {
        &t::QMF_LOSSY
    };
    let nout = nprim + usize::from(info.lfe > 0);
    if st.qmf.len() != npchs {
        st.qmf = vec![Qmf::default(); npchs];
    }
    let base = pcm.len();
    pcm.resize(base + info.samples * nout, 0.0);
    let lfe_out = if info.lfe > 0 {
        // after the fronts (L R C), before the surrounds
        Some(if nprim >= 3 && matches!(info.amode, 5 | 7 | 9) {
            3
        } else {
            2.min(nprim)
        })
    } else {
        None
    };
    // Output slot of each primary channel with LFE inserted.
    let slot = |ch: usize| {
        let o = map[ch];
        match lfe_out {
            Some(l) if o >= l => o + 1,
            _ => o,
        }
    };
    let mut done = 0usize; // samples a channel written so far
    let mut sub = vec![[[0f32; 32]; 32]; npchs]; // [ch][band][sample]
    for _ in 0..nsubfs {
        // Side information (Table 5-28).
        let nssc = r.u(2) as usize + 1;
        let _psc = r.u(3);
        let mut pmode = [[false; 32]; 5];
        for ch in 0..npchs {
            for n in 0..subs[ch] {
                pmode[ch][n] = r.u(1) != 0;
            }
        }
        for ch in 0..npchs {
            for n in 0..subs[ch] {
                if pmode[ch][n] {
                    if !partial {
                        return Err(Error::Unsupported(
                            "DTS ADPCM prediction (its code book is not in the published \
                             specification)"
                                .into(),
                        ));
                    }
                    // The prediction coefficients' address: left out (no prediction).
                    r.skip(12);
                }
            }
        }
        let mut abits = [[0usize; 32]; 5];
        for ch in 0..npchs {
            for n in 0..vqsub[ch] {
                abits[ch][n] = match bhuff[ch] {
                    5 => r.u(4) as usize,
                    6 => r.u(5) as usize,
                    k => bk.bitalloc[k].decode(&mut r)? as usize,
                };
                if abits[ch][n] > 26 {
                    return Err(bad("bit allocation index past 26"));
                }
            }
        }
        let mut tmode = [[0usize; 32]; 5];
        if nssc > 1 {
            for ch in 0..npchs {
                for n in 0..vqsub[ch] {
                    if abits[ch][n] > 0 {
                        tmode[ch][n] = bk.tmode[thuff[ch]].decode(&mut r)? as usize;
                    }
                }
            }
        }
        let mut scales = [[[0f32; 2]; 32]; 5];
        for ch in 0..npchs {
            let table: &[u32] = if shuff[ch] == 6 {
                &t::SCALE_7BIT
            } else {
                &t::SCALE_6BIT
            };
            let mut sum = 0i32;
            let mut next = |r: &mut Bits| -> Result<f32, Error> {
                let v = match shuff[ch] {
                    5 => r.u(6) as i32,
                    6 => r.u(7) as i32,
                    k => sum + bk.scale[k].decode(r)?,
                };
                sum = v;
                let i = usize::try_from(v).map_err(|_| bad("negative scale factor index"))?;
                table
                    .get(i)
                    .map(|&s| s as f32)
                    .ok_or_else(|| bad("scale factor index past the table"))
            };
            for n in 0..vqsub[ch] {
                if abits[ch][n] > 0 {
                    scales[ch][n][0] = next(&mut r)?;
                    if tmode[ch][n] > 0 {
                        scales[ch][n][1] = next(&mut r)?;
                    }
                }
            }
            for n in vqsub[ch]..subs[ch] {
                scales[ch][n][0] = next(&mut r)?;
            }
        }
        let mut join_shuff = [0usize; 5];
        for ch in 0..npchs {
            if joinx[ch] > 0 {
                join_shuff[ch] = r.u(3) as usize;
                if join_shuff[ch] == 7 {
                    return Err(bad("invalid joint scale code book"));
                }
            }
        }
        let mut join_scale = [[0f32; 32]; 5];
        for ch in 0..npchs {
            if joinx[ch] > 0 {
                let src = joinx[ch] - 1;
                for n in subs[ch]..subs[src] {
                    let v = match join_shuff[ch] {
                        5 => r.u(6) as i32,
                        6 => r.u(7) as i32,
                        k => bk.scale[k].decode(&mut r)?,
                    } + 64;
                    join_scale[ch][n] = *usize::try_from(v)
                        .ok()
                        .and_then(|i| t::JOINT_SCALE.get(i))
                        .ok_or_else(|| bad("joint scale index past the table"))?;
                }
            }
        }
        if info.dynf {
            // The dynamic range coefficient: not applied.
            r.skip(8);
        }
        if info.crc {
            r.skip(16);
        }
        // High-frequency VQ (Table 5-29).
        for ch in 0..npchs {
            if vqsub[ch] < subs[ch] {
                if !partial {
                    return Err(Error::Unsupported(
                        "DTS high-frequency VQ (its code book is not in the published \
                         specification)"
                            .into(),
                    ));
                }
                // The vector addresses: left out (silent subbands).
                r.skip(10 * (subs[ch] - vqsub[ch]));
            }
        }
        // LFE.
        let mut lfe = [0f32; 16];
        let nlfe = 2 * usize::from(info.lfe) * nssc;
        if info.lfe > 0 {
            for v in lfe.iter_mut().take(nlfe) {
                *v = f32::from(r.u(8) as u8 as i8);
            }
            let idx = r.u(8) as usize;
            let s = *t::SCALE_7BIT
                .get(idx)
                .ok_or_else(|| bad("LFE scale index past the table"))? as f32
                * 0.035;
            for v in lfe.iter_mut().take(nlfe) {
                *v *= s;
            }
        }
        // Subband samples, by subsubframe.
        for band in sub.iter_mut() {
            for s in band.iter_mut() {
                s.fill(0.0);
            }
        }
        for k in 0..nssc {
            for ch in 0..npchs {
                for n in 0..vqsub[ch] {
                    let a = abits[ch][n];
                    let mut q = [0i32; 8];
                    if a > 0 {
                        let s = sel[ch][a - 1];
                        let nbooks = if a <= 10 { bk.audio[a - 1].len() } else { 0 };
                        if s < nbooks {
                            for v in q.iter_mut() {
                                *v = bk.audio[a - 1][s].decode(&mut r)?;
                            }
                        } else if a <= 7 {
                            // Block code: four samples, base-L digits, the first lowest.
                            let levels = BLOCK_LEVELS[a - 1];
                            for half in 0..2 {
                                let mut code = r.u(BLOCK_BITS[a - 1]) as i32;
                                for v in q[half * 4..half * 4 + 4].iter_mut() {
                                    *v = code % levels - (levels - 1) / 2;
                                    code /= levels;
                                }
                                if code != 0 {
                                    return Err(bad("bad block code"));
                                }
                            }
                        } else {
                            // No further encoding: ABITS − 3 bits, two's complement.
                            let w = a as u32 - 3;
                            for v in q.iter_mut() {
                                let x = r.u(w) as i32;
                                *v = (x << (32 - w)) >> (32 - w);
                            }
                        }
                    }
                    let step = step_table[a] as f32 / (1u32 << 22) as f32;
                    let tm = if tmode[ch][n] == 0 {
                        nssc
                    } else {
                        tmode[ch][n]
                    };
                    let scale = scales[ch][n][usize::from(k >= tm)];
                    let f = step * scale * if a > 0 { adj[ch][sel[ch][a - 1]] } else { 1.0 };
                    for (m, &v) in q.iter().enumerate() {
                        sub[ch][n][8 * k + m] = f * v as f32;
                    }
                }
            }
            if (k == nssc - 1 || info.aspf) && r.u(16) != 0xFFFF {
                return Err(bad("DSYNC missing"));
            }
            if r.overrun() {
                return Err(bad("truncated frame"));
            }
        }
        // Joint intensity (C.3.4): the copied subbands count as active.
        let mut active = subs;
        for ch in 0..npchs {
            if joinx[ch] > 0 {
                let src = joinx[ch] - 1;
                for n in subs[ch]..subs[src] {
                    for m in 0..8 * nssc {
                        sub[ch][n][m] = join_scale[ch][n] * sub[src][n][m];
                    }
                }
                active[ch] = active[ch].max(subs[src]);
            }
        }
        // Sum/difference (C.3.5).
        let pair = |l: usize, r: usize, sub: &mut [[[f32; 32]; 32]]| {
            for n in 0..32 {
                for m in 0..32 {
                    let (a, b) = (sub[l][n][m], sub[r][n][m]);
                    sub[l][n][m] = a + b;
                    sub[r][n][m] = a - b;
                }
            }
        };
        let front = if matches!(info.amode, 5 | 7 | 9) {
            1
        } else {
            0
        };
        if ((info.sumf && info.amode > 0) || info.amode == 3) && front + 1 < npchs {
            pair(front, front + 1, &mut sub);
        }
        let surround = match info.amode {
            8 => Some(2),
            9 => Some(3),
            _ => None,
        };
        if let Some(s) = surround.filter(|&s| info.sums && s + 1 < npchs) {
            pair(s, s + 1, &mut sub);
        }
        // Synthesis.
        let n = 8 * nssc;
        if done + 32 * n > info.samples {
            return Err(bad("more subband samples than the header says"));
        }
        let mut block = [0f32; 32];
        for ch in 0..npchs {
            let o = slot(ch);
            for m in 0..n {
                let mut xin = [0f32; 32];
                for (i, x) in xin.iter_mut().enumerate().take(active[ch]) {
                    *x = sub[ch][i][m];
                }
                st.qmf[ch].run(&xin, qmf_coef, &mut block);
                for (i, &v) in block.iter().enumerate() {
                    pcm[base + (done + 32 * m + i) * nout + o] = v * QMF_SCALE;
                }
            }
        }
        if let Some(lo) = lfe_out {
            let (factor, coef): (usize, &[f32; 512]) = if info.lfe == 1 {
                (128, &t::LFE_128X)
            } else {
                (64, &t::LFE_64X)
            };
            let taps = 512 / factor;
            // The history then this subframe's decimated samples.
            let mut seq = [0f32; 24];
            seq[..8].copy_from_slice(&st.lfe_hist);
            seq[8..8 + nlfe].copy_from_slice(&lfe[..nlfe]);
            st.lfe_hist.copy_from_slice(&seq[nlfe..nlfe + 8]);
            for d in 0..nlfe {
                let i = 8 + d;
                for kk in 0..factor {
                    let mut acc = 0f32;
                    for j in 0..taps {
                        acc += seq[i - j] * coef[kk + j * factor];
                    }
                    let at = done + d * factor + kk;
                    if at < info.samples {
                        pcm[base + at * nout + lo] = acc * OUT_SCALE;
                    }
                }
            }
        }
        done += 32 * n;
    }
    if r.overrun() {
        return Err(bad("truncated frame"));
    }
    Ok(())
}

/// PCM full scale: the LFE interpolation's output is in 24-bit sample units.
const OUT_SCALE: f32 = 1.0 / 8_388_608.0;
/// And the synthesis filter bank's, with its 2^-7.5 gain (the scale C.3.6 leaves as
/// `rScale`), measured against FFmpeg's decode.
const QMF_SCALE: f32 = OUT_SCALE * 128.0 * std::f32::consts::SQRT_2;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every code word of a sample of the code books decodes to its level.
    #[test]
    fn code_books_decode_every_word() {
        type Word = (i8, u8, u16);
        let b = books();
        let all: Vec<(&Vlc, &[Word])> = vec![
            (&b.tmode[0], &t::H_A4),
            (&b.tmode[3], &t::H_D4),
            (&b.scale[0], &t::H_SA129),
            (&b.scale[4], &t::H_SE129),
            (&b.bitalloc[0], &t::H_A12),
            (&b.audio[0][0], &t::H_A3),
            (&b.audio[8][3], &t::H_D65),
            (&b.audio[9][3], &t::H_D129),
            (&b.audio[9][6], &t::H_G129),
        ];
        for (vlc, words) in all {
            for &(level, len, code) in words {
                let bytes = (u32::from(code) << (32 - u32::from(len))).to_be_bytes();
                let mut r = Bits::new(&bytes);
                assert_eq!(vlc.decode(&mut r).unwrap(), i32::from(level));
            }
        }
    }

    /// The filter bank passes a constant in the lowest subband as a constant (after its
    /// delay), at 2^-7.5 of it (the gain the output scale undoes).
    #[test]
    fn synthesis_dc_gain() {
        let mut q = Qmf::default();
        let mut x = [0f32; 32];
        x[0] = 1.0;
        let mut out = [0f32; 32];
        for _ in 0..40 {
            q.run(&x, &t::QMF_PERFECT, &mut out);
        }
        let want = 1.0 / (128.0 * std::f32::consts::SQRT_2);
        for v in out {
            assert!((v - want).abs() < want * 0.05, "{v} vs {want}");
        }
    }
}
