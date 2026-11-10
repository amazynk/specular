//! Syncframe headers: `syncinfo` and `bsi` of AC-3 (A/52 §5.3.1–5.3.2, with the alternate
//! syntax of Annex D) and of E-AC-3 (Annex E §2.2.2).

use std::f32::consts::{FRAC_1_SQRT_2, SQRT_2};

use crate::bits::Bits;

/// Sample rates by `fscod`.
const RATES: [u32; 3] = [48_000, 44_100, 32_000];
/// Nominal bit rates (kbit/s) by `frmsizecod / 2` (Table 5.18).
const KBPS: [u32; 19] = [
    32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 448, 512, 576, 640,
];

/// Full-bandwidth channels by `acmod` (Table 5.8).
pub(crate) const NFCHANS: [usize; 8] = [2, 1, 2, 3, 3, 4, 4, 5];

/// Table 5.9 centre and 5.10 surround mix levels (reserved codes use the middle values).
const CMIXLEV: [f32; 4] = [FRAC_1_SQRT_2, 0.594_603_6, 0.5, 0.594_603_6];
const SURMIXLEV: [f32; 4] = [FRAC_1_SQRT_2, 0.5, 0.0, 0.5];
/// Annex D Tables D2.3–D2.6 (and E-AC-3's mix levels): 3-bit centre and surround levels.
pub(crate) const MIXLEV3: [f32; 8] = [
    SQRT_2,
    1.189_207_1,
    1.0,
    0.840_896_4,
    FRAC_1_SQRT_2,
    0.594_603_6,
    0.5,
    0.0,
];

/// The facts of one syncframe header.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameInfo {
    /// E-AC-3 syntax (`bsid` 11..=16).
    pub eac3: bool,
    pub bsid: u8,
    /// E-AC-3 stream type: 0 independent, 1 dependent, 2 converted from AC-3.
    pub strmtyp: u8,
    pub substreamid: u8,
    /// Bytes of the syncframe.
    pub frame_bytes: usize,
    pub sample_rate: u32,
    /// Audio blocks of 256 samples.
    pub blocks: usize,
    pub acmod: u8,
    pub lfe: bool,
    /// Nominal bit rate of an AC-3 frame (kbit/s); E-AC-3's from its size.
    pub kbps: u32,
    /// A dependent substream's custom channel map (Table E2.5, bit 0 in the top bit).
    pub chanmap: Option<u16>,
}

/// Speaker locations as Table E2.5's bits (bit 0 left … bit 15 LFE), counted with pairs
/// as two.
pub(crate) fn locations(acmod: u8, lfe: bool) -> u16 {
    const L: u16 = 1 << 15;
    const C: u16 = 1 << 14;
    const R: u16 = 1 << 13;
    const LS: u16 = 1 << 12;
    const RS: u16 = 1 << 11;
    const CS: u16 = 1 << 8;
    let m = match acmod {
        0 | 2 => L | R,
        1 => C,
        3 => L | C | R,
        4 => L | R | CS,
        5 => L | C | R | CS,
        6 => L | R | LS | RS,
        _ => L | C | R | LS | RS,
    };
    m | if lfe { 1 } else { 0 }
}

/// The channels of a set of Table E2.5 locations.
pub(crate) fn location_channels(m: u16) -> usize {
    // pairs: bits 5, 6, 9, 10, 11 and 13 (counted from the top)
    const PAIRS: u16 = (1 << 10) | (1 << 9) | (1 << 6) | (1 << 5) | (1 << 4) | (1 << 2);
    (m.count_ones() + (m & PAIRS).count_ones()) as usize
}

impl FrameInfo {
    /// Channels coded in this syncframe, the LFE included.
    pub fn channels(&self) -> usize {
        NFCHANS[usize::from(self.acmod & 7)] + usize::from(self.lfe)
    }

    /// PCM samples per channel.
    pub fn samples(&self) -> usize {
        self.blocks * 256
    }

    /// The channel layout in words ("mono", "stereo", "5.1", "3/1").
    pub fn layout(&self) -> String {
        let base = match self.acmod {
            0 => "dual mono",
            1 => "mono",
            2 => "stereo",
            3 => "3.0",
            4 => "2/1",
            5 => "3/1",
            6 => "2/2",
            _ => "5",
        };
        match (self.acmod, self.lfe) {
            (7, true) => "5.1".into(),
            (7, false) => "5.0".into(),
            (_, true) => format!("{base} + LFE"),
            _ => base.into(),
        }
    }

    /// Parses the header at the start of `b` (`0x0B77` first). Only the fixed fields are read;
    /// the frame need not be complete.
    pub fn parse(b: &[u8]) -> Option<FrameInfo> {
        if b.len() < 8 || b[0] != 0x0B || b[1] != 0x77 {
            return None;
        }
        let bsid = b[5] >> 3;
        match bsid {
            0..=10 => {
                let fscod = b[4] >> 6;
                let frmsizecod = usize::from(b[4] & 0x3F);
                if fscod == 3 || frmsizecod >= 38 {
                    return None;
                }
                let kbps = KBPS[frmsizecod / 2];
                let words = match fscod {
                    0 => kbps as usize * 2,
                    1 => (kbps as usize * 1000 * 1536 / 44_100 / 16) + (frmsizecod & 1),
                    _ => kbps as usize * 3,
                };
                let mut r = Bits::new(&b[5..]);
                r.u(5);
                r.u(3);
                let acmod = r.u(3) as u8;
                if acmod & 1 != 0 && acmod != 1 {
                    r.u(2);
                }
                if acmod & 4 != 0 {
                    r.u(2);
                }
                if acmod == 2 {
                    r.u(2);
                }
                let lfe = r.bit();
                // bsid 9 and 10: half and quarter sample rate in AC-3 syntax
                let shift = u32::from(bsid.saturating_sub(8));
                Some(FrameInfo {
                    eac3: false,
                    bsid,
                    strmtyp: 0,
                    substreamid: 0,
                    frame_bytes: words * 2,
                    sample_rate: RATES[usize::from(fscod)] >> shift,
                    blocks: 6,
                    acmod,
                    lfe,
                    kbps: kbps >> shift,
                    chanmap: None,
                })
            }
            11..=16 => {
                let mut r = Bits::new(&b[2..]);
                let strmtyp = r.u(2) as u8;
                let substreamid = r.u(3) as u8;
                let words = r.u(11) as usize + 1;
                let fscod = r.u(2) as usize;
                let (rate, blocks) = if fscod == 3 {
                    let fscod2 = r.u(2) as usize;
                    if fscod2 == 3 {
                        return None;
                    }
                    (RATES[fscod2] / 2, 6)
                } else {
                    (RATES[fscod], [1, 2, 3, 6][r.u(2) as usize])
                };
                let acmod = r.u(3) as u8;
                let lfe = r.bit();
                if strmtyp == 3 || words < 4 {
                    return None;
                }
                r.u(10); // bsid, dialnorm
                if r.bit() {
                    r.u(8);
                }
                if acmod == 0 {
                    r.u(5);
                    if r.bit() {
                        r.u(8);
                    }
                }
                let chanmap = (strmtyp == 1 && r.bit()).then(|| r.u(16) as u16);
                let kbps =
                    (words as u64 * 16 * u64::from(rate) / (blocks as u64 * 256) / 1000) as u32;
                Some(FrameInfo {
                    eac3: true,
                    bsid,
                    strmtyp,
                    substreamid,
                    frame_bytes: words * 2,
                    sample_rate: rate,
                    blocks,
                    acmod,
                    lfe,
                    kbps,
                    chanmap,
                })
            }
            _ => None,
        }
    }
}

/// Everything of `bsi` the decoder uses.
#[derive(Clone, Debug)]
pub(crate) struct Bsi {
    pub info: FrameInfo,
    /// `fscod` for the bit allocation's hearing threshold (`fscod2` at reduced rates).
    pub fscod: usize,
    /// Dialogue level in dB below full scale (1..=31), per programme (two in dual mono).
    pub dialnorm: [u8; 2],
    /// Lo/Ro centre and surround mix levels.
    pub loro_cmix: f32,
    pub loro_surmix: f32,
    /// LFE level in a downmix, when the stream says (E-AC-3).
    pub lfe_mix: Option<f32>,
    /// Programme scale factors (linear; E-AC-3).
    pub pgmscl: [f32; 2],
    /// E-AC-3 `snroffststr`… are in `audfrm`; this is where `audfrm`/`audblk` start.
    pub bits_used: usize,
    /// E-AC-3 (for tests that turn a frame into a dependent one): where `chanmape` goes,
    /// and the bit ranges `(start, len)` only an independent frame has.
    pub chanmap_at: usize,
    pub indep_only: Vec<(usize, usize)>,
}

fn dialnorm(v: u32) -> u8 {
    // 0 is reserved: read as -31 dB (no change)
    if v == 0 { 31 } else { v as u8 }
}

/// Parses `syncinfo` and `bsi` of a whole syncframe.
pub(crate) fn parse_bsi(frame: &[u8]) -> Option<Bsi> {
    let info = FrameInfo::parse(frame)?;
    if info.eac3 {
        parse_eac3(frame, info)
    } else {
        parse_ac3(frame, info)
    }
}

fn parse_ac3(frame: &[u8], info: FrameInfo) -> Option<Bsi> {
    let mut r = Bits::new(frame);
    r.skip(32); // syncword, crc1
    let fscod = r.u(2) as usize;
    r.u(6);
    r.u(5); // bsid
    r.u(3); // bsmod
    let acmod = r.u(3);
    let mut cmix = CMIXLEV[0];
    let mut surmix = SURMIXLEV[0];
    if acmod & 1 != 0 && acmod != 1 {
        cmix = CMIXLEV[r.u(2) as usize];
    }
    if acmod & 4 != 0 {
        surmix = SURMIXLEV[r.u(2) as usize];
    }
    if acmod == 2 {
        r.u(2); // dsurmod
    }
    r.u(1); // lfeon
    let mut dn = [dialnorm(r.u(5)); 2];
    if r.bit() {
        r.u(8); // compr
    }
    if r.bit() {
        r.u(8); // langcod
    }
    if r.bit() {
        r.u(7); // mixlevel, roomtyp
    }
    if acmod == 0 {
        dn[1] = dialnorm(r.u(5));
        if r.bit() {
            r.u(8);
        }
        if r.bit() {
            r.u(8);
        }
        if r.bit() {
            r.u(7);
        }
    }
    r.u(2); // copyrightb, origbs
    let mut b = Bsi {
        info,
        fscod,
        dialnorm: dn,
        loro_cmix: cmix,
        loro_surmix: surmix,
        lfe_mix: None,
        pgmscl: [1.0; 2],
        bits_used: 0,
        chanmap_at: 0,
        indep_only: Vec::new(),
    };
    if info.bsid == 6 {
        // Annex D: extra bit stream information in place of the time codes
        if r.bit() {
            r.u(8); // dmixmod, Lt/Rt levels
            let loroc = r.u(3) as usize;
            let loros = r.u(3) as usize;
            if acmod & 1 != 0 && acmod != 1 {
                b.loro_cmix = MIXLEV3[loroc];
            }
            if acmod & 4 != 0 {
                b.loro_surmix = MIXLEV3[loros.max(3)];
            }
        }
        if r.bit() {
            r.u(14);
        }
    } else {
        if r.bit() {
            r.u(14);
        }
        if r.bit() {
            r.u(14);
        }
    }
    if r.bit() {
        let len = r.u(6) as usize + 1;
        r.skip(len * 8);
    }
    if r.overrun {
        return None;
    }
    b.bits_used = r.pos();
    Some(b)
}

fn pgmscl(v: u32) -> f32 {
    if v == 0 {
        0.0
    } else {
        10f32.powf((v as f32 - 51.0) / 20.0)
    }
}

fn parse_eac3(frame: &[u8], info: FrameInfo) -> Option<Bsi> {
    let mut r = Bits::new(frame);
    r.skip(16);
    let strmtyp = r.u(2);
    r.u(3);
    r.u(11);
    let mut fscod = r.u(2) as usize;
    let reduced = fscod == 3;
    let numblkscod = if reduced {
        fscod = r.u(2) as usize;
        3
    } else {
        r.u(2)
    };
    let nblocks = [1usize, 2, 3, 6][numblkscod as usize];
    let acmod = r.u(3);
    let lfeon = r.bit();
    r.u(5); // bsid
    let mut dn = [dialnorm(r.u(5)); 2];
    if r.bit() {
        r.u(8);
    }
    if acmod == 0 {
        dn[1] = dialnorm(r.u(5));
        if r.bit() {
            r.u(8);
        }
    }
    let chanmap_at = r.pos();
    if strmtyp == 1 && r.bit() {
        r.u(16); // chanmap
    }
    let mut b = Bsi {
        info,
        fscod,
        dialnorm: dn,
        // without mixing metadata: AC-3's middle levels (−4.5 dB centre, −6 dB surround),
        // as libavcodec does
        loro_cmix: MIXLEV3[5],
        loro_surmix: MIXLEV3[6],
        lfe_mix: None,
        pgmscl: [1.0; 2],
        bits_used: 0,
        chanmap_at,
        indep_only: Vec::new(),
    };
    if r.bit() {
        // mixing metadata
        if acmod > 2 {
            r.u(2); // dmixmod
        }
        if acmod & 1 != 0 && acmod > 2 {
            r.u(3); // Lt/Rt centre
            b.loro_cmix = MIXLEV3[r.u(3) as usize];
        }
        if acmod & 4 != 0 {
            r.u(3); // Lt/Rt surround
            b.loro_surmix = MIXLEV3[(r.u(3) as usize).max(3)];
        }
        if lfeon && r.bit() {
            let cod = r.u(5) as f32;
            // the mix level less 4.5 dB (Annex E §3.9)
            b.lfe_mix = Some(10f32.powf((10.0 - cod - 4.5) / 20.0));
        }
        if strmtyp == 0 {
            let start = r.pos();
            if r.bit() {
                b.pgmscl[0] = pgmscl(r.u(6));
                b.pgmscl[1] = b.pgmscl[0];
            }
            if acmod == 0 && r.bit() {
                b.pgmscl[1] = pgmscl(r.u(6));
            }
            if r.bit() {
                r.u(6); // extpgmscl
            }
            match r.u(2) {
                1 => r.skip(5),
                2 => r.skip(12),
                3 => {
                    let len = r.u(5) as usize;
                    r.skip(8 * (len + 2));
                }
                _ => {}
            }
            if acmod < 2 {
                if r.bit() {
                    r.u(14); // panmean, paninfo
                }
                if acmod == 0 && r.bit() {
                    r.u(14);
                }
            }
            if r.bit() {
                // frame mixing configuration
                if numblkscod == 0 {
                    r.u(5);
                } else {
                    for _ in 0..nblocks {
                        if r.bit() {
                            r.u(5);
                        }
                    }
                }
            }
            b.indep_only.push((start, r.pos() - start));
        }
    }
    if r.bit() {
        // informational metadata
        r.u(5); // bsmod, copyrightb, origbs
        if acmod == 2 {
            r.u(4);
        }
        if acmod >= 6 {
            r.u(2);
        }
        if r.bit() {
            r.u(8);
        }
        if acmod == 0 && r.bit() {
            r.u(8);
        }
        if !reduced {
            r.u(1); // sourcefscod
        }
    }
    if strmtyp == 0 && numblkscod != 3 {
        b.indep_only.push((r.pos(), 1));
        r.u(1); // convsync
    }
    if strmtyp == 2 {
        let blkid = if numblkscod == 3 { true } else { r.bit() };
        if blkid {
            r.u(6);
        }
    }
    if r.bit() {
        let len = r.u(6) as usize + 1;
        r.skip(len * 8);
    }
    if r.overrun {
        return None;
    }
    b.bits_used = r.pos();
    Some(b)
}
