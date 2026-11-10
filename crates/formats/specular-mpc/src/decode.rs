//! One Musepack frame: 36 samples in each of 32 subbands per channel, read from SV7's or
//! SV8's bit stream, scaled back and filtered into 1152 PCM samples.
//!
//! Each subband has a resolution (`res`): 0 is silence, -1 noise, 1 and 2 code three or two
//! samples at a time, the higher ones one sample each (up to 2^16 steps). Three scale
//! factors per subband cover 12 samples each; mid/side bands are summed back into left and
//! right.

use std::sync::OnceLock;

use crate::Error;
use crate::bits::{Bits, Code};
use crate::synth::Synth;
use crate::tables as t;

/// PCM samples per channel in a frame.
pub const FRAME: usize = 1152;

struct Codes {
    hdr: Code,
    scfi7: Code,
    dscf7: Code,
    q7: Vec<[Code; 2]>,
    scfi8: [Code; 2],
    dscf8: [Code; 2],
    bands: Code,
    res8: [Code; 2],
    q8_1: Code,
    q8_2: [Code; 2],
    q8_34: [Code; 2],
    q8_58: Vec<[Code; 2]>,
    q8_9up: Code,
}

fn codes() -> &'static Codes {
    static C: OnceLock<Codes> = OnceLock::new();
    C.get_or_init(|| Codes {
        hdr: Code::new(t::HDR),
        scfi7: Code::new(t::SCFI7),
        dscf7: Code::new(t::DSCF7),
        q7: vec![
            [Code::new(t::Q7_1_0), Code::new(t::Q7_1_1)],
            [Code::new(t::Q7_2_0), Code::new(t::Q7_2_1)],
            [Code::new(t::Q7_3_0), Code::new(t::Q7_3_1)],
            [Code::new(t::Q7_4_0), Code::new(t::Q7_4_1)],
            [Code::new(t::Q7_5_0), Code::new(t::Q7_5_1)],
            [Code::new(t::Q7_6_0), Code::new(t::Q7_6_1)],
            [Code::new(t::Q7_7_0), Code::new(t::Q7_7_1)],
        ],
        scfi8: [Code::new(t::SCFI8_1), Code::new(t::SCFI8_2)],
        dscf8: [Code::new(t::DSCF8_1), Code::new(t::DSCF8_2)],
        bands: Code::new(t::BANDS),
        res8: [Code::new(t::RES_1), Code::new(t::RES_2)],
        q8_1: Code::new(t::Q8_1),
        q8_2: [Code::new(t::Q8_2_1), Code::new(t::Q8_2_2)],
        q8_34: [Code::new(t::Q8_3), Code::new(t::Q8_4)],
        q8_58: vec![
            [Code::new(t::Q8_5_1), Code::new(t::Q8_5_2)],
            [Code::new(t::Q8_6_1), Code::new(t::Q8_6_2)],
            [Code::new(t::Q8_7_1), Code::new(t::Q8_7_2)],
            [Code::new(t::Q8_8_1), Code::new(t::Q8_8_2)],
        ],
        q8_9up: Code::new(t::Q8_9UP),
    })
}

/// Half the number of steps of resolution `res` (1..=17), the offset of its codes.
fn half_steps(res: i32) -> i32 {
    match res {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 7,
        r => (1 << (r - 2)) - 1,
    }
}

struct Scales {
    /// The step of each resolution, -1 (noise) at index 0.
    step: [f32; 19],
    /// Scale factor `i & 0xff`: 1/32768 at index 1, each next one 1.5 dB lower.
    scf: [f32; 256],
}

fn scales() -> &'static Scales {
    static S: OnceLock<Scales> = OnceLock::new();
    S.get_or_init(|| {
        let mut step = [0f32; 19];
        // noise: four bytes summed spread ±510 with σ ≈ 147; 32768/2/255·√3 brings it level
        step[0] = (32768.0 / 2.0 / 255.0 * 3f64.sqrt()) as f32;
        for r in 1..=17 {
            step[r as usize + 1] = (65536.0 / f64::from(2 * half_steps(r) + 1)) as f32;
        }
        // 1.5 dB apart; built by repeated products as the reference decoder does, so the
        // rounding of each entry is the same.
        let ratio = 0.832_980_664_765_826_7_f64;
        let inv = 1.0 / ratio;
        let base = 1.0 / 32768.0;
        let mut scf = [0f32; 256];
        scf[1] = base as f32;
        let (mut down, mut up) = (base * ratio, base * inv);
        for n in 1..=128usize {
            scf[(1 + n) & 255] = down as f32;
            scf[(1usize.wrapping_sub(n)) & 255] = up as f32;
            down *= ratio;
            up *= inv;
        }
        Scales { step, scf }
    })
}

/// Pairs and triples of SV7's and SV8's grouped codes, -2..=2 or -1..=1 each.
fn digit(x: i32, base: i32, place: u32) -> i16 {
    ((x / base.pow(place)) % base - base / 2) as i16
}

#[derive(Clone)]
struct Channel {
    res: [i32; 33],
    scfi: [u32; 33],
    scf: [[i32; 3]; 33],
    /// SV8: the next scale factor of the band is coded whole (after a key frame).
    whole: [bool; 33],
    q: [[i16; 36]; 33],
    synth: Synth,
}

impl Channel {
    fn new() -> Channel {
        Channel {
            res: [0; 33],
            scfi: [0; 33],
            scf: [[0; 3]; 33],
            whole: [true; 33],
            q: [[0; 36]; 33],
            synth: Synth::new(),
        }
    }
}

/// The decoder of one stream: its channels' state from frame to frame.
#[derive(Clone)]
pub struct FrameDecoder {
    version: u8,
    channels: usize,
    max_band: usize,
    ms: bool,
    ch: [Channel; 2],
    ms_flag: [bool; 33],
    last_bands: usize,
    noise: (u32, u32),
}

fn bad(m: &str) -> Error {
    Error::Malformed(m.into())
}

impl FrameDecoder {
    pub(crate) fn new(version: u8, channels: usize, max_band: usize, ms: bool) -> FrameDecoder {
        FrameDecoder {
            version,
            channels: channels.clamp(1, 2),
            max_band: max_band.min(32),
            ms,
            ch: [Channel::new(), Channel::new()],
            ms_flag: [false; 33],
            last_bands: 0,
            noise: (1, 1),
        }
    }

    /// Forgets everything, as at the start of the stream; `mid` sets SV7's scale factors as
    /// the reference decoder does after a seek into the middle.
    pub fn reset(&mut self, mid: bool) {
        let mut fresh = FrameDecoder::new(self.version, self.channels, self.max_band, self.ms);
        if mid {
            for c in &mut fresh.ch {
                for s in &mut c.scf {
                    *s = [0x0101_0101; 3];
                }
            }
        }
        *self = fresh;
    }

    /// The next pseudo-random number of the noise subbands (two LFSRs of coprime periods).
    fn random(&mut self) -> u32 {
        let (r1, r2) = self.noise;
        let p1 = (r1 & 0xF5).count_ones() & 1;
        let p2 = ((r2 >> 25) & 0x63).count_ones() & 1;
        let n1 = (r1 >> 1) | (p1 << 31);
        let n2 = r2.wrapping_add(r2) | p2;
        self.noise = (n1, n2);
        n1 ^ n2
    }

    fn noise_band(&mut self, c: usize, n: usize) {
        for k in 0..36 {
            let x = self.random();
            let s = (x >> 24) + ((x >> 16) & 0xFF) + ((x >> 8) & 0xFF) + (x & 0xFF);
            self.ch[c].q[n][k] = s as i16 - 510;
        }
    }

    /// Reads one SV7 frame's subband samples.
    pub(crate) fn read_sv7(&mut self, r: &mut Bits) -> Result<(), Error> {
        let cs = codes();
        let mut used = 0;
        for n in 0..=self.max_band {
            for c in 0..2 {
                self.ch[c].res[n] = if n == 0 {
                    r.read(4) as i32
                } else {
                    match r.huff(&cs.hdr) {
                        4 => r.read(4) as i32,
                        d => self.ch[c].res[n - 1] + d,
                    }
                };
            }
            if self.ch[0].res[n] != 0 || self.ch[1].res[n] != 0 {
                if self.ms {
                    self.ms_flag[n] = r.bit();
                }
                used = n + 1;
            }
        }
        for n in used..=self.max_band {
            self.ms_flag[n] = false;
        }
        for n in 0..used {
            for c in 0..2 {
                if self.ch[c].res[n] != 0 {
                    self.ch[c].scfi[n] = r.huff(&cs.scfi7) as u32;
                }
            }
        }
        for n in 0..used {
            for c in 0..2 {
                if self.ch[c].res[n] == 0 {
                    continue;
                }
                let ch = &mut self.ch[c];
                let next = |r: &mut Bits, prev: i32| match r.huff(&cs.dscf7) {
                    8 => r.read(6) as i32,
                    d => prev + d,
                };
                let s = &mut ch.scf[n];
                match ch.scfi[n] {
                    0 => {
                        s[0] = next(r, s[2]);
                        s[1] = next(r, s[0]);
                        s[2] = next(r, s[1]);
                    }
                    1 => {
                        s[0] = next(r, s[2]);
                        s[1] = next(r, s[0]);
                        s[2] = s[1];
                    }
                    2 => {
                        s[0] = next(r, s[2]);
                        s[1] = s[0];
                        s[2] = next(r, s[1]);
                    }
                    _ => {
                        s[0] = next(r, s[2]);
                        s[1] = s[0];
                        s[2] = s[0];
                    }
                }
                for v in s.iter_mut() {
                    if *v > 1024 {
                        *v = 0x8080;
                    }
                }
            }
        }
        for n in 0..used {
            for c in 0..2 {
                let res = self.ch[c].res[n];
                match res {
                    0 => {}
                    -1 => self.noise_band(c, n),
                    1 => {
                        let code = &cs.q7[0][r.read(1) as usize];
                        for k in (0..36).step_by(3) {
                            let i = r.huff(code);
                            let q = &mut self.ch[c].q[n];
                            q[k] = digit(i, 3, 0);
                            q[k + 1] = digit(i, 3, 1);
                            q[k + 2] = digit(i, 3, 2);
                        }
                    }
                    2 => {
                        let code = &cs.q7[1][r.read(1) as usize];
                        for k in (0..36).step_by(2) {
                            let i = r.huff(code);
                            let q = &mut self.ch[c].q[n];
                            q[k] = digit(i, 5, 0);
                            q[k + 1] = digit(i, 5, 1);
                        }
                    }
                    3..=7 => {
                        let code = &cs.q7[res as usize - 1][r.read(1) as usize];
                        for k in 0..36 {
                            self.ch[c].q[n][k] = r.huff(code) as i16;
                        }
                    }
                    8..=17 => {
                        let off = half_steps(res);
                        for k in 0..36 {
                            self.ch[c].q[n][k] = (r.read(res as u32 - 1) as i32 - off) as i16;
                        }
                    }
                    _ => return Err(bad("a Musepack frame with an impossible resolution")),
                }
            }
        }
        for n in used..=self.max_band {
            self.ch[0].res[n] = 0;
            self.ch[1].res[n] = 0;
        }
        Ok(())
    }

    /// Reads one SV8 frame's subband samples; `key` for the first frame of a packet.
    pub(crate) fn read_sv8(&mut self, r: &mut Bits, key: bool) -> Result<(), Error> {
        let cs = codes();
        let used = if key {
            r.bounded(self.max_band as u32 + 1) as usize
        } else {
            let mut u = self.last_bands + r.huff(&cs.bands) as u8 as usize;
            if u > 32 {
                u -= 33;
            }
            u
        };
        if used > self.max_band + 1 || used > 32 {
            return Err(bad("a Musepack frame with more subbands than the stream"));
        }
        self.last_bands = used;
        let wrap = |x: i32| if x > 15 { x - 17 } else { x };
        if used > 0 {
            for c in 0..2 {
                let top = wrap(r.huff(&cs.res8[0]) as u8 as i32);
                self.ch[c].res[used - 1] = top;
            }
            for n in (0..used - 1).rev() {
                for c in 0..2 {
                    let above = self.ch[c].res[n + 1];
                    let d = r.huff(&cs.res8[usize::from(above > 2)]) as u8 as i32;
                    self.ch[c].res[n] = wrap(d + above);
                }
            }
            if self.ms {
                let coded: Vec<usize> = (0..used)
                    .filter(|&n| self.ch[0].res[n] != 0 || self.ch[1].res[n] != 0)
                    .collect();
                let tot = coded.len() as u32;
                let cnt = r.bounded(tot);
                let mut bits = 0u32;
                if cnt != 0 && cnt != tot {
                    bits = r.combination(cnt.min(tot - cnt), tot);
                }
                if cnt * 2 > tot {
                    bits = !bits;
                }
                for &n in coded.iter().rev() {
                    self.ms_flag[n] = bits & 1 != 0;
                    bits >>= 1;
                }
            }
        }
        for n in used..33 {
            self.ch[0].res[n] = 0;
            self.ch[1].res[n] = 0;
            self.ms_flag[n] = false;
        }
        if key {
            for c in &mut self.ch {
                c.whole = [true; 33];
            }
        }
        for n in 0..used {
            let (l, rr) = (self.ch[0].res[n] != 0, self.ch[1].res[n] != 0);
            let both = l && rr;
            if l || rr {
                let v = r.huff(&cs.scfi8[usize::from(both)]) as u32;
                if l {
                    self.ch[0].scfi[n] = if both { v >> 2 } else { v };
                }
                if rr {
                    self.ch[1].scfi[n] = v & 3;
                }
            }
        }
        for n in 0..used {
            for c in 0..2 {
                let ch = &mut self.ch[c];
                if ch.res[n] == 0 {
                    continue;
                }
                let s = &mut ch.scf[n];
                if ch.whole[n] {
                    s[0] = r.read(7) as i32 - 6;
                    ch.whole[n] = false;
                } else {
                    let mut d = r.huff(&cs.dscf8[1]) as u8 as i32;
                    if d == 64 {
                        d += r.read(6) as i32;
                    }
                    s[0] = ((s[2] - 25 + d) & 127) - 6;
                }
                for m in 0..2 {
                    if (ch.scfi[n] << m) & 2 == 0 {
                        let mut d = r.huff(&cs.dscf8[0]) as u8 as i32;
                        if d == 31 {
                            d = 64 + r.read(6) as i32;
                        }
                        s[m + 1] = ((s[m] - 25 + d) & 127) - 6;
                    } else {
                        s[m + 1] = s[m];
                    }
                }
            }
        }
        const THRES: [u32; 9] = [0, 0, 3, 0, 0, 1, 3, 4, 8];
        for n in 0..used {
            for c in 0..2 {
                let res = self.ch[c].res[n];
                match res {
                    0 => {}
                    -1 => self.noise_band(c, n),
                    1 => {
                        for half in 0..2 {
                            let cnt = r.huff(&cs.q8_1) as u8 as u32;
                            let mut set = 0u32;
                            if cnt > 0 && cnt < 18 {
                                set = r.combination(if cnt <= 9 { cnt } else { 18 - cnt }, 18);
                            }
                            if cnt > 9 {
                                set = !set;
                            }
                            for k in 0..18 {
                                let q = if set & (1 << 17) != 0 {
                                    if r.bit() { 1 } else { -1 }
                                } else {
                                    0
                                };
                                self.ch[c].q[n][half * 18 + k] = q;
                                set <<= 1;
                            }
                        }
                    }
                    2 => {
                        let th = THRES[2];
                        let mut ctx = 2 * th;
                        for k in (0..36).step_by(3) {
                            let i = r.huff(&cs.q8_2[usize::from(ctx > th)]) as u8 as i32;
                            let q = &mut self.ch[c].q[n];
                            q[k] = digit(i, 5, 0);
                            q[k + 1] = digit(i, 5, 1);
                            q[k + 2] = digit(i, 5, 2);
                            let mag = q[k].unsigned_abs()
                                + q[k + 1].unsigned_abs()
                                + q[k + 2].unsigned_abs();
                            ctx = (ctx >> 1) + u32::from(mag);
                        }
                    }
                    3 | 4 => {
                        let code = &cs.q8_34[res as usize - 3];
                        for k in (0..36).step_by(2) {
                            let s = r.huff(code) as i8;
                            self.ch[c].q[n][k] = i16::from((s << 4) >> 4);
                            self.ch[c].q[n][k + 1] = i16::from(s >> 4);
                        }
                    }
                    5..=8 => {
                        let th = THRES[res as usize];
                        let pair = &cs.q8_58[res as usize - 5];
                        let mut ctx = 2 * th;
                        for k in 0..36 {
                            let q = r.huff(&pair[usize::from(ctx > th)]) as i16;
                            self.ch[c].q[n][k] = q;
                            ctx = (ctx >> 1) + u32::from(q.unsigned_abs());
                        }
                    }
                    9..=17 => {
                        let extra = res as u32 - 9;
                        let off = half_steps(res);
                        for k in 0..36 {
                            let mut q = r.huff(&cs.q8_9up) as u8 as i32;
                            if extra > 0 {
                                q = (q << extra) | r.read(extra) as i32;
                            }
                            self.ch[c].q[n][k] = (q - off) as i16;
                        }
                    }
                    _ => return Err(bad("a Musepack frame with an impossible resolution")),
                }
            }
        }
        Ok(())
    }

    /// Scales the subband samples back and filters them into `FRAME` samples per channel,
    /// interleaved, appended to `out`.
    pub(crate) fn synthesise(&mut self, out: &mut Vec<f32>) {
        let sc = scales();
        let nch = self.channels;
        // y[c][slot][band]
        let mut y = vec![[[0f32; 32]; 36]; 2];
        for n in 0..=self.max_band.min(31) {
            let mut vals = [[0f32; 36]; 2];
            for c in 0..2 {
                let ch = &self.ch[c];
                let res = ch.res[n];
                if res == 0 {
                    continue;
                }
                let step = sc.step[(res + 1) as usize];
                for part in 0..3 {
                    let fac = step * sc.scf[(ch.scf[n][part] & 0xFF) as usize];
                    for k in part * 12..part * 12 + 12 {
                        vals[c][k] = fac * f32::from(ch.q[n][k]);
                    }
                }
            }
            for k in 0..36 {
                let (l, r) = (vals[0][k], vals[1][k]);
                let (l, r) = if self.ms_flag[n] {
                    (l + r, l - r)
                } else {
                    (l, r)
                };
                y[0][k][n] = l;
                y[1][k][n] = r;
            }
        }
        let start = out.len();
        out.resize(start + FRAME * nch, 0.0);
        let pcm = &mut out[start..];
        for c in 0..nch {
            for (slot, ys) in y[c].iter().enumerate() {
                let at = slot * 32 * nch + c;
                self.ch[c].synth.run(ys, &mut pcm[at..], nch);
            }
        }
    }

    /// One frame of the filter's tail with nothing coded (the stream's last samples).
    pub(crate) fn drain(&mut self, out: &mut Vec<f32>) {
        let nch = self.channels;
        let start = out.len();
        out.resize(start + FRAME * nch, 0.0);
        let pcm = &mut out[start..];
        for c in 0..nch {
            for slot in 0..36 {
                self.ch[c]
                    .synth
                    .run(&[0.0; 32], &mut pcm[slot * 32 * nch + c..], nch);
            }
        }
    }

    pub(crate) fn reset_filters(&mut self) {
        for c in &mut self.ch {
            c.synth.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_match_the_reference_offsets() {
        let want = [
            1, 2, 3, 4, 7, 15, 31, 63, 127, 255, 511, 1023, 2047, 4095, 8191, 16383, 32767,
        ];
        for (r, w) in (1..=17).zip(want) {
            assert_eq!(half_steps(r), w);
        }
        let s = scales();
        assert_eq!(s.step[2], (65536.0f64 / 3.0) as f32);
        assert!((s.step[0] - 111.285_96).abs() < 1e-3);
        assert_eq!(s.scf[1], 1.0 / 32768.0);
        assert!(s.scf[2] < s.scf[1] && s.scf[0] > s.scf[1]);
    }

    #[test]
    fn digits_spread_grouped_codes() {
        assert_eq!(
            (digit(0, 3, 0), digit(0, 3, 1), digit(0, 3, 2)),
            (-1, -1, -1)
        );
        assert_eq!(
            (digit(26, 3, 0), digit(26, 3, 1), digit(26, 3, 2)),
            (1, 1, 1)
        );
        assert_eq!((digit(7, 5, 0), digit(7, 5, 1)), (0, -1));
        assert_eq!(digit(124, 5, 2), 2);
    }

    #[test]
    fn noise_is_reproducible() {
        let mut a = FrameDecoder::new(8, 2, 31, true);
        let mut b = a.clone();
        let x: Vec<u32> = (0..8).map(|_| a.random()).collect();
        let y: Vec<u32> = (0..8).map(|_| b.random()).collect();
        assert_eq!(x, y);
        assert!(x.windows(2).all(|w| w[0] != w[1]));
    }
}
