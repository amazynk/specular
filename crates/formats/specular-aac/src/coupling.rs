//! Coupling channel elements (§4.6.8.3): a channel whose spectrum, or whose output after
//! the filterbank, is added with per-band or per-channel gains to channels of other elements
//! (dependently switched coupling before or after TNS, independently switched coupling after
//! the filterbank).

use crate::Result;
use crate::bits::Bits;
use crate::ics::{Channel, Core, Lcg, ZERO_HCB};
use crate::tables::Books;

/// Where a coupling channel joins its targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Point {
    BeforeTns,
    AfterTns,
    AfterFilterbank,
}

/// One coupled element: a CPE or SCE by its instance tag, and for a CPE which channels
/// (`cc_l`, `cc_r` as bits 1 and 0; 0 means both with one gain list). An SCE has 2.
#[derive(Debug, Clone, Copy)]
struct Target {
    cpe: bool,
    tag: u8,
    select: u8,
}

/// Gain steps by `gain_element_scale`: 2^(1/8), 2^(1/4), 2^(1/2), 2.
const SCALE: [f32; 4] = [1.090_507_7, 1.189_207_1, std::f32::consts::SQRT_2, 2.0];

pub(crate) struct Cce {
    pub tag: u8,
    pub ch: Channel,
    pub point: Point,
    targets: Vec<Target>,
    /// Each gain list: one gain per (group, band) at `g·64 + sfb`, or a single gain after
    /// the filterbank.
    gains: Vec<Vec<f32>>,
    /// The channel's own output, for coupling after the filterbank.
    pub time: Vec<f32>,
}

impl Cce {
    pub fn read(b: &mut Bits, books: &Books, core: &Core, noise: &mut Lcg) -> Result<Cce> {
        let tag = b.read(4) as u8;
        let ind_sw = b.bit();
        let n = b.read(3) + 1;
        let mut lists = 0;
        let mut targets = Vec::new();
        for _ in 0..n {
            lists += 1;
            let cpe = b.bit();
            let t = b.read(4) as u8;
            let select = if cpe { b.read(2) as u8 } else { 2 };
            if select == 3 {
                lists += 1;
            }
            targets.push(Target {
                cpe,
                tag: t,
                select,
            });
        }
        let after_tns = b.bit();
        let point = if ind_sw {
            Point::AfterFilterbank
        } else if after_tns {
            Point::AfterTns
        } else {
            Point::BeforeTns
        };
        let sign = b.bit();
        let scale = SCALE[b.read(2) as usize];
        let mut ch = Channel::default();
        ch.read(b, books, core, None, noise)?;
        let bands = 8 * 64;
        let delta = |b: &mut Bits| -> Result<i32> {
            books
                .scalefactor
                .read(b)
                .map(|v| v as i32 - 60)
                .ok_or_else(|| crate::Error::Malformed("invalid coupling gain code".into()))
        };
        let mut gains = Vec::with_capacity(lists);
        for c in 0..lists {
            let len = if point == Point::AfterFilterbank {
                1
            } else {
                bands
            };
            if c == 0 {
                // the first list is not sent: 0 dB
                gains.push(vec![1.0; len]);
                continue;
            }
            let common = ind_sw || b.bit();
            if common {
                let g = delta(b)?;
                gains.push(vec![scale.powi(-g); len]);
                continue;
            }
            let mut list = vec![0.0; bands];
            let (mut gain, mut cache) = (0i32, 1.0f32);
            for g in 0..ch.info.group_len.len() {
                for s in 0..ch.info.max_sfb {
                    let i = g * 64 + s;
                    if ch.cb[i] == ZERO_HCB {
                        continue;
                    }
                    let t = delta(b)?;
                    if t != 0 {
                        gain += t;
                        let (sgn, e) = if sign {
                            (if gain & 1 != 0 { -1.0 } else { 1.0 }, gain >> 1)
                        } else {
                            (1.0, gain)
                        };
                        cache = sgn * scale.powi(-e);
                    }
                    list[i] = cache;
                }
            }
            gains.push(list);
        }
        Ok(Cce {
            tag,
            ch,
            point,
            targets,
            gains,
            time: Vec::new(),
        })
    }

    /// The gain lists for the channels (0 left or mono, 1 right) of the element `cpe`/`tag`.
    pub fn lists_for(&self, cpe: bool, tag: u8) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut idx = 0;
        for t in &self.targets {
            if t.cpe == cpe && t.tag == tag {
                if t.select != 1 {
                    out.push((0, idx));
                    if t.select != 0 {
                        idx += 1;
                    }
                }
                if t.select != 2 {
                    out.push((1, idx));
                    idx += 1;
                }
            } else {
                idx += 1 + usize::from(t.select == 3);
            }
        }
        out
    }

    /// Adds this channel's spectrum to `dest` with gain list `list`.
    pub fn add_spectrum(&self, dest: &mut Channel, list: usize) {
        let Some(gains) = self.gains.get(list) else {
            return;
        };
        let info = &self.ch.info;
        let win_len = info.window_len();
        let mut win = 0;
        for (g, &glen) in info.group_len.iter().enumerate() {
            for s in 0..info.max_sfb {
                let i = g * 64 + s;
                if self.ch.cb[i] == ZERO_HCB {
                    continue;
                }
                let gain = gains[i];
                let (lo, hi) = (
                    usize::from(info.offsets[s]),
                    usize::from(info.offsets[s + 1]),
                );
                for w in win..win + glen {
                    let base = w * win_len;
                    for k in base + lo..base + hi {
                        dest.spec[k] += gain * self.ch.spec[k];
                    }
                }
            }
            win += glen;
        }
    }

    /// Adds this channel's filterbank output to `dest` with gain list `list`.
    pub fn add_time(&self, dest: &mut [f32], list: usize) {
        let g = self
            .gains
            .get(list)
            .and_then(|l| l.first())
            .copied()
            .unwrap_or(0.0);
        for (d, s) in dest.iter_mut().zip(&self.time) {
            *d += g * s;
        }
    }
}
