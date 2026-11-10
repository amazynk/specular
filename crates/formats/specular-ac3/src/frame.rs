//! One syncframe to PCM: E-AC-3's `audfrm` (Annex E §2.2.3), then each audio block (A/52
//! §5.3.3, Annex E §2.2.4): exponents, bit allocation, mantissas, coupling, rematrixing,
//! spectral extension, the adaptive hybrid transform, dynamic range control and the IMDCT.

use crate::Error;
use crate::alloc::{self, Alloc, BaParams, Dba, REUSE};
use crate::bits::Bits;
use crate::header::{Bsi, NFCHANS};
use crate::mdct::Imdct;
use crate::tables::{GAQ_REMAP, SPXATTENTAB, VQ1, VQ2, VQ3, VQ4, VQ5, VQ6, VQ7};

/// Slots: full-bandwidth channels 0..5, the LFE at 5, the coupling channel at 6.
pub(crate) const LFE: usize = 5;
const CPL: usize = 6;

/// Table E2.10: block exponent strategies of the 32 frame strategies (0 reuse, 1 D15, 2 D25,
/// 3 D45).
const FRM_EXPSTR: [[u8; 6]; 32] = [
    [1, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 3],
    [1, 0, 0, 0, 2, 0],
    [1, 0, 0, 0, 3, 3],
    [2, 0, 0, 2, 0, 0],
    [2, 0, 0, 2, 0, 3],
    [2, 0, 0, 3, 2, 0],
    [2, 0, 0, 3, 3, 3],
    [2, 0, 1, 0, 0, 0],
    [2, 0, 2, 0, 0, 3],
    [2, 0, 2, 0, 2, 0],
    [2, 0, 2, 0, 3, 3],
    [2, 0, 3, 2, 0, 0],
    [2, 0, 3, 2, 0, 3],
    [2, 0, 3, 3, 2, 0],
    [2, 0, 3, 3, 3, 3],
    [3, 1, 0, 0, 0, 0],
    [3, 1, 0, 0, 0, 3],
    [3, 2, 0, 0, 2, 0],
    [3, 2, 0, 0, 3, 3],
    [3, 2, 0, 2, 0, 0],
    [3, 2, 0, 2, 0, 3],
    [3, 2, 0, 3, 2, 0],
    [3, 2, 0, 3, 3, 3],
    [3, 3, 1, 0, 0, 0],
    [3, 3, 2, 0, 0, 3],
    [3, 3, 2, 0, 2, 0],
    [3, 3, 2, 0, 3, 3],
    [3, 3, 3, 2, 0, 0],
    [3, 3, 3, 2, 0, 3],
    [3, 3, 3, 3, 2, 0],
    [3, 3, 3, 3, 3, 3],
];
/// Table E2.11 and E2.12: default spectral extension and coupling band structures, by
/// sub-band number.
const DEF_SPXBNDSTRC: [bool; 17] = [
    false, false, false, false, false, false, false, false, true, false, true, false, true, false,
    true, false, true,
];
const DEF_CPLBNDSTRC: [bool; 18] = [
    false, false, false, false, false, false, false, false, true, false, true, true, false, true,
    true, true, true, true,
];
/// Mantissa bits of the asymmetric quantizers, bap 6..=15 (Table 7.18).
const ASYM_BITS: [u32; 10] = [5, 6, 7, 8, 9, 10, 11, 12, 14, 16];
/// Mantissa bits of the AHT scalar quantizers, hebap 8..=19 (Table E3.2).
const HE_BITS: [u32; 12] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 16];
/// Index bits of the AHT vector quantizers, hebap 1..=7.
const VQ_BITS: [u32; 7] = [2, 3, 4, 5, 7, 8, 9];

/// A uniform pseudo-random source for dither and spectral extension noise.
pub(crate) struct Rng(u32);

impl Rng {
    pub(crate) fn new() -> Rng {
        Rng(0x2545_F491)
    }
    /// Uniform in [-1, 1).
    #[inline]
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as i32) as f32 / 2_147_483_648.0
    }
}

/// Decoder options that change the PCM (see [`crate::Options`]).
#[derive(Clone, Copy)]
pub(crate) struct Process {
    pub dither: bool,
    pub drc: f32,
}

/// Unpacks grouped mantissas (bap 1, 2 and 4) across a block's exponent sets.
#[derive(Default)]
struct Groups {
    g1: [f32; 3],
    n1: usize,
    g2: [f32; 3],
    n2: usize,
    g4: [f32; 2],
    n4: usize,
}

const Q3: [f32; 3] = [-2.0 / 3.0, 0.0, 2.0 / 3.0];
const Q5: [f32; 5] = [-0.8, -0.4, 0.0, 0.4, 0.8];
const Q11: [f32; 11] = [
    -10.0 / 11.0,
    -8.0 / 11.0,
    -6.0 / 11.0,
    -4.0 / 11.0,
    -2.0 / 11.0,
    0.0,
    2.0 / 11.0,
    4.0 / 11.0,
    6.0 / 11.0,
    8.0 / 11.0,
    10.0 / 11.0,
];

impl Groups {
    /// The mantissa (−1..1) of a bin with pointer `bap` > 0.
    #[inline]
    fn mantissa(&mut self, r: &mut Bits, bap: u8) -> f32 {
        match bap {
            1 => {
                if self.n1 == 0 {
                    let g = (r.u(5) as usize).min(26);
                    self.g1 = [Q3[g / 9], Q3[(g % 9) / 3], Q3[g % 3]];
                    self.n1 = 3;
                }
                self.n1 -= 1;
                self.g1[2 - self.n1]
            }
            2 => {
                if self.n2 == 0 {
                    let g = (r.u(7) as usize).min(124);
                    self.g2 = [Q5[g / 25], Q5[(g % 25) / 5], Q5[g % 5]];
                    self.n2 = 3;
                }
                self.n2 -= 1;
                self.g2[2 - self.n2]
            }
            3 => (2.0 * r.u(3) as f32 - 6.0) / 7.0,
            4 => {
                if self.n4 == 0 {
                    let g = (r.u(7) as usize).min(120);
                    self.g4 = [Q11[g / 11], Q11[g % 11]];
                    self.n4 = 2;
                }
                self.n4 -= 1;
                self.g4[1 - self.n4]
            }
            5 => (2.0 * r.u(4) as f32 - 14.0) / 15.0,
            6..=15 => {
                let bits = ASYM_BITS[usize::from(bap - 6)];
                r.s(bits) as f32 / (1u32 << (bits - 1)) as f32
            }
            _ => 0.0,
        }
    }
}

#[inline]
fn pow2(e: u8) -> f32 {
    f32::from_bits((127 - u32::from(e.min(24))) << 23)
}

/// State kept from block to block (and, for the overlap, frame to frame).
pub(crate) struct State {
    imdct: Imdct,
    rng: Rng,
    delay: [[f32; 256]; 6],
    exps: [[u8; 256]; 7],
    bap: [[u8; 256]; 7],
    /// Coefficients of the block being decoded (fbw and LFE).
    coef: [[f32; 256]; 6],
    /// The coupling channel's mantissas scaled by its exponents, and where its bap is 0.
    cplcoef: [f32; 256],
    endmant: [usize; 5],
    blksw: [bool; 5],
    dithflag: [bool; 5],
    dynrng: [f32; 2],
    // coupling
    cplinu: bool,
    chincpl: [bool; 5],
    phsflginu: bool,
    cplbegf: usize,
    /// One past the last coupling sub-band (3 + cplendf).
    cplend_sub: usize,
    cplbndstrc: [bool; 18],
    /// Coupling coordinates per channel and sub-band.
    cplco: [[f32; 18]; 5],
    phsflg: [bool; 18],
    rematflg: [bool; 4],
    // bit allocation
    ba: BaParams,
    csnroffst: i32,
    fsnroffst: [i32; 7],
    fgaincod: [u8; 7],
    cplleak: (u8, u8),
    dba: [Dba; 7],
    // E-AC-3 spectral extension
    spxinu: bool,
    chinspx: [bool; 5],
    spxstrtf: usize,
    spxbegf: usize,
    spx_begin: usize,
    spx_end: usize,
    spxbndstrc: [bool; 17],
    spxco: [[f32; 17]; 5],
    spx_nblend: [[f32; 17]; 5],
    spx_sblend: [[f32; 17]; 5],
    firstspxcos: [bool; 5],
    firstcplcos: [bool; 5],
    firstcplleak: bool,
    // E-AC-3 adaptive hybrid transform: per slot, 6 blocks of coefficients
    aht: Vec<[[f32; 256]; 6]>,
    ahtinu: [bool; 7],
    /// PCM of the frame: per slot, up to 6 × 256.
    pub(crate) pcm: [[f32; 1536]; 6],
    /// Where in the frame (bits) a dither flag was set ([`crate::without_dither`]).
    pub(crate) dith_at: Vec<usize>,
    /// Bit ranges `(start, len)` of `audfrm` and `audblk` only an independent frame has.
    pub(crate) indep_at: Vec<(usize, usize)>,
}

impl State {
    pub(crate) fn new() -> State {
        State {
            imdct: Imdct::new(),
            rng: Rng::new(),
            delay: [[0.0; 256]; 6],
            exps: [[0; 256]; 7],
            bap: [[0; 256]; 7],
            coef: [[0.0; 256]; 6],
            cplcoef: [0.0; 256],
            endmant: [0; 5],
            blksw: [false; 5],
            dithflag: [false; 5],
            dynrng: [1.0; 2],
            cplinu: false,
            chincpl: [false; 5],
            phsflginu: false,
            cplbegf: 0,
            cplend_sub: 0,
            cplbndstrc: [false; 18],
            cplco: [[0.0; 18]; 5],
            phsflg: [false; 18],
            rematflg: [false; 4],
            ba: BaParams::default(),
            csnroffst: 0,
            fsnroffst: [0; 7],
            fgaincod: [0; 7],
            cplleak: (0, 0),
            dba: [Dba::default(); 7],
            spxinu: false,
            chinspx: [false; 5],
            spxstrtf: 0,
            spxbegf: 0,
            spx_begin: 0,
            spx_end: 0,
            spxbndstrc: [false; 17],
            spxco: [[0.0; 17]; 5],
            spx_nblend: [[0.0; 17]; 5],
            spx_sblend: [[0.0; 17]; 5],
            firstspxcos: [true; 5],
            firstcplcos: [true; 5],
            firstcplleak: true,
            aht: vec![[[0.0; 256]; 6]; 7],
            ahtinu: [false; 7],
            pcm: [[0.0; 1536]; 6],
            dith_at: Vec::new(),
            indep_at: Vec::new(),
        }
    }

    /// Forgets the overlap (after a seek or a lost frame) and starts the dither again, so
    /// what plays after a seek does not depend on how much was decoded before it.
    pub(crate) fn reset(&mut self) {
        self.delay = [[0.0; 256]; 6];
        self.rng = Rng::new();
    }
}

/// E-AC-3 frame-level fields of `audfrm`.
struct Audfrm {
    expstr: [[u8; 7]; 6],
    cplstre: [bool; 6],
    cplinu: [bool; 6],
    snroffststr: u8,
    blkswe: bool,
    dithflage: bool,
    bamode: bool,
    frmfgaincode: bool,
    dbaflde: bool,
    skipflde: bool,
    frmcsnroffst: i32,
    frmfsnroffst: i32,
    chinspxatten: [Option<usize>; 5],
    ahte: bool,
    ahtinu: [bool; 7],
    /// `convexpstre`… (independent frames only), as a bit range.
    conv: Option<(usize, usize)>,
}

/// The AC-3 frame's implied `audfrm`: everything per block.
fn ac3_audfrm() -> Audfrm {
    Audfrm {
        expstr: [[0; 7]; 6],
        cplstre: [false; 6],
        cplinu: [false; 6],
        snroffststr: 0,
        blkswe: true,
        dithflage: true,
        bamode: true,
        frmfgaincode: true,
        dbaflde: true,
        skipflde: true,
        frmcsnroffst: 0,
        frmfsnroffst: 0,
        chinspxatten: [None; 5],
        ahte: false,
        ahtinu: [false; 7],
        conv: None,
    }
}

fn read_audfrm(r: &mut Bits, b: &Bsi, nfch: usize) -> Result<Audfrm, Error> {
    let info = &b.info;
    let nblk = info.blocks;
    let acmod = info.acmod;
    let lfe = info.lfe;
    let mut f = ac3_audfrm();
    let (expstre, ahte) = if nblk == 6 {
        (r.bit(), r.bit())
    } else {
        (true, false)
    };
    f.ahte = ahte;
    f.snroffststr = r.u(2) as u8;
    let transproce = r.bit();
    f.blkswe = r.bit();
    f.dithflage = r.bit();
    f.bamode = r.bit();
    f.frmfgaincode = r.bit();
    f.dbaflde = r.bit();
    f.skipflde = r.bit();
    let spxattene = r.bit();
    if acmod > 1 {
        f.cplstre[0] = true;
        f.cplinu[0] = r.bit();
        for blk in 1..nblk {
            f.cplstre[blk] = r.bit();
            f.cplinu[blk] = if f.cplstre[blk] {
                r.bit()
            } else {
                f.cplinu[blk - 1]
            };
        }
    }
    let ncplblks = f.cplinu[..nblk].iter().filter(|&&c| c).count();
    if expstre {
        for blk in 0..nblk {
            if f.cplinu[blk] {
                f.expstr[blk][CPL] = r.u(2) as u8;
            }
            for ch in 0..nfch {
                f.expstr[blk][ch] = r.u(2) as u8;
            }
        }
    } else {
        if acmod > 1 && ncplblks > 0 {
            let s = r.u(5) as usize;
            for blk in 0..nblk {
                f.expstr[blk][CPL] = FRM_EXPSTR[s][blk];
            }
        }
        for ch in 0..nfch {
            let s = r.u(5) as usize;
            for blk in 0..nblk {
                f.expstr[blk][ch] = FRM_EXPSTR[s][blk];
            }
        }
    }
    if lfe {
        for blk in 0..nblk {
            f.expstr[blk][LFE] = r.u(1) as u8;
        }
    }
    if info.strmtyp == 0 {
        let start = r.pos();
        let convexpstre = if nblk != 6 { r.bit() } else { true };
        if convexpstre {
            r.skip(5 * nfch);
        }
        f.conv = Some((start, r.pos() - start));
    }
    if ahte {
        let regs = |slot: usize, f: &Audfrm| {
            (0..6)
                .filter(|&blk| {
                    if slot == CPL {
                        f.cplstre[blk] || f.expstr[blk][CPL] != REUSE
                    } else {
                        f.expstr[blk][slot] != REUSE
                    }
                })
                .count()
        };
        if ncplblks == 6 && regs(CPL, &f) == 1 {
            f.ahtinu[CPL] = r.bit();
        }
        for ch in 0..nfch {
            if regs(ch, &f) == 1 {
                f.ahtinu[ch] = r.bit();
            }
        }
        if lfe && regs(LFE, &f) == 1 {
            f.ahtinu[LFE] = r.bit();
        }
    }
    if f.snroffststr == 0 {
        f.frmcsnroffst = r.u(6) as i32;
        f.frmfsnroffst = r.u(4) as i32;
    }
    if transproce {
        // transient pre-noise processing is optional for decoders: read past it
        for _ in 0..nfch {
            if r.bit() {
                r.u(18);
            }
        }
    }
    if spxattene {
        for ch in 0..nfch {
            if r.bit() {
                f.chinspxatten[ch] = Some(r.u(5) as usize);
            }
        }
    }
    let blkstrtinfoe = if nblk > 1 { r.bit() } else { false };
    if blkstrtinfoe {
        let words = info.frame_bytes / 2;
        let log2 = usize::BITS - (words.max(1) - 1).leading_zeros();
        r.skip((nblk - 1) * (4 + log2 as usize));
    }
    if r.overrun {
        return Err(Error::Malformed("short E-AC-3 frame header".into()));
    }
    Ok(f)
}

/// The number of groups of a full-bandwidth channel's exponents (§7.1.3).
fn nchgrps(end: usize, strategy: u8) -> usize {
    match strategy {
        1 => (end.saturating_sub(1)) / 3,
        2 => (end.saturating_sub(1) + 3) / 6,
        _ => (end.saturating_sub(1) + 9) / 12,
    }
}

/// Decodes one syncframe's blocks into `st.pcm` (slots 0..nfchans and the LFE).
pub(crate) fn decode(frame: &[u8], b: &Bsi, st: &mut State, p: Process) -> Result<(), Error> {
    let info = &b.info;
    let eac3 = info.eac3;
    let acmod = usize::from(info.acmod);
    let nfch = NFCHANS[acmod];
    let lfe = info.lfe;
    let nblk = info.blocks;
    let mut r = Bits::new(frame);
    r.skip(b.bits_used);
    let f = if eac3 {
        read_audfrm(&mut r, b, nfch)?
    } else {
        ac3_audfrm()
    };
    // frame initialisation
    st.cplbndstrc = DEF_CPLBNDSTRC;
    st.spxbndstrc = DEF_SPXBNDSTRC;
    st.firstspxcos = [true; 5];
    st.firstcplcos = [true; 5];
    st.firstcplleak = true;
    st.ahtinu = f.ahtinu;
    for d in st.dba.iter_mut() {
        d.mode = 2;
        d.nseg = 0;
    }
    if !eac3 {
        st.spxinu = false;
        st.chinspx = [false; 5];
    }
    st.dith_at.clear();
    st.indep_at.clear();
    st.indep_at.extend(f.conv);
    for blk in 0..nblk {
        block(&mut r, b, &f, blk, st, p)?;
        if r.overrun {
            return Err(Error::Malformed("block runs past the frame".into()));
        }
        // inverse transforms
        for ch in 0..nfch {
            let (coef, delay) = (&st.coef[ch], &mut st.delay[ch]);
            st.imdct.run(
                coef,
                st.blksw[ch],
                delay,
                &mut st.pcm[ch][blk * 256..blk * 256 + 256],
            );
        }
        if lfe {
            st.imdct.run(
                &st.coef[LFE],
                false,
                &mut st.delay[LFE],
                &mut st.pcm[LFE][blk * 256..blk * 256 + 256],
            );
        }
    }
    Ok(())
}

fn dynrng(v: u32, scale: f32) -> f32 {
    let x = ((v as i32) << 24 >> 29) + 1; // signed 3-bit integer + 1
    let y = (v & 0x1F) as f32;
    let g = 2f32.powi(x) * (32.0 + y) / 64.0;
    g.powf(scale)
}

#[allow(clippy::too_many_lines)]
fn block(
    r: &mut Bits,
    b: &Bsi,
    f: &Audfrm,
    blk: usize,
    st: &mut State,
    p: Process,
) -> Result<(), Error> {
    let info = &b.info;
    let eac3 = info.eac3;
    let acmod = usize::from(info.acmod);
    let nfch = NFCHANS[acmod];
    let lfe = info.lfe;
    // block switch and dither flags
    for ch in 0..nfch {
        st.blksw[ch] = if f.blkswe { r.bit() } else { false };
    }
    for ch in 0..nfch {
        st.dithflag[ch] = if f.dithflage {
            let at = r.pos();
            let on = r.bit();
            if on {
                st.dith_at.push(at);
            }
            on
        } else {
            true
        };
    }
    // dynamic range control
    for prog in 0..(if acmod == 0 { 2 } else { 1 }) {
        if r.bit() {
            st.dynrng[prog] = dynrng(r.u(8), p.drc);
        } else if blk == 0 {
            st.dynrng[prog] = 1.0;
        }
    }
    // spectral extension strategy and coordinates
    if eac3 {
        let spxstre = blk == 0 || r.bit();
        if spxstre {
            st.spxinu = r.bit();
            if st.spxinu {
                if acmod == 1 {
                    st.chinspx[0] = true;
                } else {
                    for ch in 0..nfch {
                        st.chinspx[ch] = r.bit();
                    }
                }
                st.spxstrtf = r.u(2) as usize;
                let begf = r.u(3) as usize;
                let endf = r.u(3) as usize;
                st.spxbegf = begf;
                st.spx_begin = if begf < 6 { begf + 2 } else { begf * 2 - 3 };
                st.spx_end = if endf < 3 { endf + 5 } else { endf * 2 + 3 };
                if st.spx_begin >= st.spx_end || st.spxstrtf >= st.spx_begin {
                    return Err(Error::Malformed("spectral extension range".into()));
                }
                if r.bit() {
                    for bnd in st.spx_begin + 1..st.spx_end {
                        st.spxbndstrc[bnd] = r.bit();
                    }
                }
            } else {
                st.chinspx = [false; 5];
                st.firstspxcos = [true; 5];
            }
        }
        if st.spxinu {
            let sizes = spx_bands(st);
            for ch in 0..nfch {
                if !st.chinspx[ch] {
                    st.firstspxcos[ch] = true;
                    continue;
                }
                let coe = if st.firstspxcos[ch] {
                    st.firstspxcos[ch] = false;
                    true
                } else {
                    r.bit()
                };
                if coe {
                    let spxblnd = r.u(5) as f32;
                    let mstr = r.u(2) as i32;
                    let noffset = spxblnd / 32.0;
                    let mut spxmant = spx_bin(st.spx_begin);
                    let endbin = spx_bin(st.spx_end) as f32;
                    for (bnd, &size) in sizes.iter().enumerate() {
                        let e = r.u(4) as i32;
                        let m = r.u(2) as f32;
                        let t = if e == 15 { m / 4.0 } else { (m + 4.0) / 8.0 };
                        st.spxco[ch][bnd] = t * 2f32.powi(-(e + 3 * mstr));
                        let nratio = ((spxmant as f32 + 0.5 * size as f32) / endbin - noffset)
                            .clamp(0.0, 1.0);
                        st.spx_nblend[ch][bnd] = nratio.sqrt();
                        st.spx_sblend[ch][bnd] = (1.0 - nratio).sqrt();
                        spxmant += size;
                    }
                }
            }
        }
    }
    // coupling strategy
    let cplstre = if eac3 { f.cplstre[blk] } else { r.bit() };
    if cplstre {
        st.cplinu = if eac3 { f.cplinu[blk] } else { r.bit() };
        if st.cplinu {
            if eac3 && r.bit() {
                return Err(Error::Unsupported("E-AC-3 with enhanced coupling".into()));
            }
            if eac3 && acmod == 2 {
                st.chincpl[0] = true;
                st.chincpl[1] = true;
            } else {
                for ch in 0..nfch {
                    st.chincpl[ch] = r.bit();
                }
            }
            st.phsflginu = acmod == 2 && r.bit();
            st.cplbegf = r.u(4) as usize;
            let cplendf: i32 = if eac3 && st.spxinu {
                let begf = st.spxbegf;
                if begf < 6 {
                    begf as i32 - 2
                } else {
                    begf as i32 * 2 - 7
                }
            } else {
                r.u(4) as i32
            };
            let end = 3 + cplendf;
            if end <= st.cplbegf as i32 || end > 18 {
                return Err(Error::Malformed("coupling range".into()));
            }
            st.cplend_sub = end as usize;
            if !eac3 || r.bit() {
                for bnd in st.cplbegf + 1..st.cplend_sub {
                    st.cplbndstrc[bnd] = r.bit();
                }
            }
        } else {
            st.chincpl = [false; 5];
            st.firstcplcos = [true; 5];
            st.firstcplleak = true;
            st.phsflginu = false;
        }
    }
    // coupling coordinates
    if st.cplinu {
        let mut any_coe = false;
        for ch in 0..nfch {
            if !st.chincpl[ch] {
                st.firstcplcos[ch] = true;
                continue;
            }
            let coe = if eac3 && st.firstcplcos[ch] {
                st.firstcplcos[ch] = false;
                true
            } else {
                r.bit()
            };
            if coe {
                any_coe |= ch < 2;
                let mstr = 3 * r.u(2) as i32;
                let mut sub = st.cplbegf;
                while sub < st.cplend_sub {
                    let e = r.u(4) as i32;
                    let m = r.u(4) as f32;
                    let t = if e == 15 { m / 16.0 } else { (m + 16.0) / 32.0 };
                    let co = t * 2f32.powi(-(e + mstr));
                    st.cplco[ch][sub] = co;
                    sub += 1;
                    while sub < st.cplend_sub && st.cplbndstrc[sub] {
                        st.cplco[ch][sub] = co;
                        sub += 1;
                    }
                }
            }
        }
        if acmod == 2 && st.phsflginu && any_coe {
            let mut sub = st.cplbegf;
            while sub < st.cplend_sub {
                let fl = r.bit();
                st.phsflg[sub] = fl;
                sub += 1;
                while sub < st.cplend_sub && st.cplbndstrc[sub] {
                    st.phsflg[sub] = fl;
                    sub += 1;
                }
            }
        } else if acmod == 2 && !st.phsflginu {
            st.phsflg = [false; 18];
        }
    }
    // rematrixing
    let mut nrematbd = 0;
    if acmod == 2 {
        nrematbd = if st.cplinu {
            if st.cplbegf == 0 {
                2
            } else if st.cplbegf < 3 {
                3
            } else {
                4
            }
        } else if st.spxinu {
            if st.spxbegf < 2 { 3 } else { 4 }
        } else {
            4
        };
        let rematstr = (eac3 && blk == 0) || r.bit();
        if rematstr {
            for bnd in 0..nrematbd {
                st.rematflg[bnd] = r.bit();
            }
        }
    }
    // exponent strategies
    let mut expstr = [0u8; 7];
    if eac3 {
        expstr = f.expstr[blk];
    } else {
        if st.cplinu {
            expstr[CPL] = r.u(2) as u8;
        }
        for ch in 0..nfch {
            expstr[ch] = r.u(2) as u8;
        }
        if lfe {
            expstr[LFE] = r.u(1) as u8;
        }
    }
    // channel bandwidths
    let cplstrt = 37 + 12 * st.cplbegf;
    let cplendmant = 37 + 12 * st.cplend_sub;
    for ch in 0..nfch {
        if expstr[ch] != REUSE {
            if st.chincpl[ch] && st.cplinu {
                st.endmant[ch] = cplstrt;
            } else if st.chinspx[ch] && st.spxinu {
                st.endmant[ch] = spx_bin(st.spx_begin);
            } else {
                let bw = r.u(6) as usize;
                if bw > 60 {
                    return Err(Error::Malformed("channel bandwidth".into()));
                }
                st.endmant[ch] = 37 + 3 * (bw + 12);
            }
        }
    }
    // exponents
    if st.cplinu && expstr[CPL] != REUSE {
        let abs = (r.u(4) as i32) << 1;
        let s = expstr[CPL];
        let n = (cplendmant - cplstrt) / (3 << (s - 1));
        if !alloc::exponents(r, s, abs, n, cplstrt, cplendmant, &mut st.exps[CPL]) {
            return Err(Error::Malformed("coupling exponents".into()));
        }
    }
    for ch in 0..nfch {
        if expstr[ch] != REUSE {
            let abs = r.u(4) as i32;
            st.exps[ch][0] = abs as u8;
            let end = st.endmant[ch];
            let n = nchgrps(end, expstr[ch]);
            if !alloc::exponents(r, expstr[ch], abs, n, 1, end, &mut st.exps[ch]) {
                return Err(Error::Malformed("exponents".into()));
            }
            r.u(2); // gainrng
        }
    }
    if lfe && expstr[LFE] != REUSE {
        let abs = r.u(4) as i32;
        st.exps[LFE][0] = abs as u8;
        if !alloc::exponents(r, 1, abs, 2, 1, 7, &mut st.exps[LFE]) {
            return Err(Error::Malformed("LFE exponents".into()));
        }
    }
    // bit allocation parameters
    if !eac3 || f.bamode {
        if r.bit() {
            st.ba = BaParams {
                sdcycod: r.u(2) as u8,
                fdcycod: r.u(2) as u8,
                sgaincod: r.u(2) as u8,
                dbpbcod: r.u(2) as u8,
                floorcod: r.u(3) as u8,
            };
        }
    } else {
        st.ba = BaParams {
            sdcycod: 2,
            fdcycod: 1,
            sgaincod: 1,
            dbpbcod: 2,
            floorcod: 7,
        };
    }
    let slots: Vec<usize> = (0..nfch)
        .chain(lfe.then_some(LFE))
        .chain(st.cplinu.then_some(CPL))
        .collect();
    if !eac3 {
        if r.bit() {
            st.csnroffst = r.u(6) as i32;
            if st.cplinu {
                st.fsnroffst[CPL] = r.u(4) as i32;
                st.fgaincod[CPL] = r.u(3) as u8;
            }
            for ch in 0..nfch {
                st.fsnroffst[ch] = r.u(4) as i32;
                st.fgaincod[ch] = r.u(3) as u8;
            }
            if lfe {
                st.fsnroffst[LFE] = r.u(4) as i32;
                st.fgaincod[LFE] = r.u(3) as u8;
            }
        }
    } else {
        if f.snroffststr == 0 {
            st.csnroffst = f.frmcsnroffst;
            for &s in &slots {
                st.fsnroffst[s] = f.frmfsnroffst;
            }
        } else if blk == 0 || r.bit() {
            st.csnroffst = r.u(6) as i32;
            if f.snroffststr == 1 {
                let v = r.u(4) as i32;
                for &s in &slots {
                    st.fsnroffst[s] = v;
                }
            } else {
                if st.cplinu {
                    st.fsnroffst[CPL] = r.u(4) as i32;
                }
                for ch in 0..nfch {
                    st.fsnroffst[ch] = r.u(4) as i32;
                }
                if lfe {
                    st.fsnroffst[LFE] = r.u(4) as i32;
                }
            }
        }
        let fgaincode = f.frmfgaincode && r.bit();
        if fgaincode {
            if st.cplinu {
                st.fgaincod[CPL] = r.u(3) as u8;
            }
            for ch in 0..nfch {
                st.fgaincod[ch] = r.u(3) as u8;
            }
            if lfe {
                st.fgaincod[LFE] = r.u(3) as u8;
            }
        } else {
            for &s in &slots {
                st.fgaincod[s] = 4;
            }
        }
        if info.strmtyp == 0 {
            let start = r.pos();
            if r.bit() {
                r.u(10); // convsnroffst
            }
            st.indep_at.push((start, r.pos() - start));
        }
    }
    if st.cplinu {
        let leake = if eac3 && st.firstcplleak {
            st.firstcplleak = false;
            true
        } else {
            r.bit()
        };
        if leake {
            st.cplleak = (r.u(3) as u8, r.u(3) as u8);
        }
    }
    // delta bit allocation
    if (!eac3 || f.dbaflde) && r.bit() {
        let mut modes = [0u8; 7];
        if st.cplinu {
            modes[CPL] = r.u(2) as u8;
        }
        for ch in 0..nfch {
            modes[ch] = r.u(2) as u8;
        }
        let order: Vec<usize> = st
            .cplinu
            .then_some(CPL)
            .into_iter()
            .chain(0..nfch)
            .collect();
        for s in order {
            match modes[s] {
                0 => {}
                1 => {
                    st.dba[s].mode = 1;
                    st.dba[s].read(r);
                }
                m => {
                    st.dba[s].mode = m;
                    st.dba[s].nseg = 0;
                }
            }
        }
    }
    // skipped data
    if (!eac3 || f.skipflde) && r.bit() {
        let n = r.u(9) as usize;
        r.skip(n * 8);
    }
    // bit allocation
    let all_zero = st.csnroffst == 0 && slots.iter().all(|&s| st.fsnroffst[s] == 0);
    for &s in &slots {
        let (start, end) = match s {
            CPL => (cplstrt, cplendmant),
            LFE => (0, 7),
            ch => (0, st.endmant[ch]),
        };
        if all_zero {
            st.bap[s][start..end].fill(0);
            continue;
        }
        // AHT channels are allocated once, in the block their exponents come
        if st.ahtinu[s] && expstr[s] == REUSE && blk > 0 {
            continue;
        }
        let a = Alloc {
            exps: &st.exps[s],
            start,
            end,
            params: st.ba,
            fscod: b.fscod,
            fgaincod: st.fgaincod[s],
            snroffset: (((st.csnroffst - 15) << 4) + st.fsnroffst[s]) << 2,
            cpl_leak: (s == CPL).then_some(st.cplleak),
            lfe: s == LFE,
            dba: (s != LFE).then_some(&st.dba[s]),
            hebap: st.ahtinu[s],
        };
        let mut bap = st.bap[s];
        alloc::allocate(&a, &mut bap);
        st.bap[s] = bap;
    }
    // mantissas
    let mut g = Groups::default();
    let mut got_cpl = false;
    for ch in 0..nfch {
        let end = st.endmant[ch];
        if st.ahtinu[ch] {
            if blk == 0 {
                let (bap, exps) = (st.bap[ch], st.exps[ch]);
                aht_read(r, &bap, &exps, 0, end, &mut st.aht[ch]);
            }
            st.coef[ch][..end].copy_from_slice(&st.aht[ch][blk][..end]);
            dither_zero_bins(st, ch, 0, end, p.dither);
        } else {
            read_channel(r, &mut g, st, ch, 0, end, p.dither);
        }
        st.coef[ch][end..].fill(0.0);
        if st.cplinu && st.chincpl[ch] && !got_cpl {
            got_cpl = true;
            if st.ahtinu[CPL] {
                if blk == 0 {
                    let (bap, exps) = (st.bap[CPL], st.exps[CPL]);
                    aht_read(r, &bap, &exps, cplstrt, cplendmant, &mut st.aht[CPL]);
                }
                st.cplcoef[cplstrt..cplendmant]
                    .copy_from_slice(&st.aht[CPL][blk][cplstrt..cplendmant]);
            } else {
                for bin in cplstrt..cplendmant {
                    let bp = st.bap[CPL][bin];
                    st.cplcoef[bin] = if bp == 0 {
                        0.0
                    } else {
                        g.mantissa(r, bp) * pow2(st.exps[CPL][bin])
                    };
                }
            }
        }
    }
    if lfe {
        if st.ahtinu[LFE] {
            if blk == 0 {
                let (bap, exps) = (st.bap[LFE], st.exps[LFE]);
                aht_read(r, &bap, &exps, 0, 7, &mut st.aht[LFE]);
            }
            st.coef[LFE][..7].copy_from_slice(&st.aht[LFE][blk][..7]);
        } else {
            for bin in 0..7 {
                let bp = st.bap[LFE][bin];
                st.coef[LFE][bin] = if bp == 0 {
                    0.0
                } else {
                    g.mantissa(r, bp) * pow2(st.exps[LFE][bin])
                };
            }
        }
        st.coef[LFE][7..].fill(0.0);
    }
    // decoupling
    if st.cplinu {
        for ch in 0..nfch {
            if !st.chincpl[ch] {
                continue;
            }
            for sub in st.cplbegf..st.cplend_sub {
                let mut co = st.cplco[ch][sub] * 8.0;
                if acmod == 2 && ch == 1 && st.phsflg[sub] {
                    co = -co;
                }
                for bin in 37 + 12 * sub..49 + 12 * sub {
                    let v = if st.bap[CPL][bin] == 0 {
                        if st.dithflag[ch] && p.dither {
                            st.rng.next() * 0.707 * pow2(st.exps[CPL][bin])
                        } else {
                            0.0
                        }
                    } else {
                        st.cplcoef[bin]
                    };
                    st.coef[ch][bin] = v * co;
                }
            }
            st.coef[ch][cplendmant..].fill(0.0);
        }
    }
    // rematrixing
    if acmod == 2 && nrematbd > 0 {
        const EDGES: [usize; 5] = [13, 25, 37, 61, 253];
        let mut top = st.endmant[0].min(st.endmant[1]);
        if st.cplinu {
            top = top.min(cplstrt);
        }
        for bnd in 0..nrematbd {
            if !st.rematflg[bnd] {
                continue;
            }
            let lo = EDGES[bnd];
            let hi = if bnd + 1 == nrematbd {
                top
            } else {
                EDGES[bnd + 1].min(top)
            };
            for bin in lo..hi {
                let (l, rr) = (st.coef[0][bin], st.coef[1][bin]);
                st.coef[0][bin] = l + rr;
                st.coef[1][bin] = l - rr;
            }
        }
    }
    // spectral extension
    if st.spxinu {
        for ch in 0..nfch {
            if st.chinspx[ch] {
                spx_synthesize(st, ch, f.chinspxatten[ch]);
            }
        }
    }
    // dynamic range
    for ch in 0..nfch {
        let g = st.dynrng[if acmod == 0 && ch == 1 { 1 } else { 0 }];
        if g != 1.0 {
            for v in st.coef[ch].iter_mut() {
                *v *= g;
            }
        }
    }
    if lfe && st.dynrng[0] != 1.0 {
        let g = st.dynrng[0];
        for v in st.coef[LFE][..7].iter_mut() {
            *v *= g;
        }
    }
    Ok(())
}

/// Reads one channel's mantissas for bins `start..end` (bap from the allocation).
fn read_channel(
    r: &mut Bits,
    g: &mut Groups,
    st: &mut State,
    ch: usize,
    start: usize,
    end: usize,
    dither: bool,
) {
    let dith = st.dithflag[ch] && dither;
    for bin in start..end {
        let bp = st.bap[ch][bin];
        let m = if bp == 0 {
            if dith { st.rng.next() * 0.707 } else { 0.0 }
        } else {
            g.mantissa(r, bp)
        };
        st.coef[ch][bin] = m * pow2(st.exps[ch][bin]);
    }
}

/// Dither for the zero-pointer bins of an AHT channel.
fn dither_zero_bins(st: &mut State, ch: usize, start: usize, end: usize, dither: bool) {
    if !(st.dithflag[ch] && dither) {
        return;
    }
    for bin in start..end {
        if st.bap[ch][bin] == 0 {
            st.coef[ch][bin] = st.rng.next() * 0.707 * pow2(st.exps[ch][bin]);
        }
    }
}

/// Reads an AHT channel's mantissas for all six blocks (Annex E §3.4): vector quantized or
/// gain-adaptive quantized, then the inverse DCT across blocks, scaled by the exponents.
fn aht_read(
    r: &mut Bits,
    hebap: &[u8; 256],
    exps: &[u8; 256],
    start: usize,
    end: usize,
    out: &mut [[f32; 256]; 6],
) {
    let gaqmod = r.u(2);
    let endbap = if gaqmod < 2 { 12 } else { 17 };
    let active = (start..end)
        .filter(|&b| hebap[b] > 7 && hebap[b] < endbap)
        .count();
    let mut gains: Vec<u8> = Vec::new();
    if gaqmod != 0 {
        let sections = match gaqmod {
            1 | 2 => active,
            _ => active.div_ceil(3),
        };
        for _ in 0..sections {
            if gaqmod == 3 {
                let gg = (r.u(5) as usize).min(26);
                gains.extend([gg / 9, (gg % 9) / 3, gg % 3].map(|m| m as u8));
            } else {
                gains.push(if r.bit() { gaqmod as u8 } else { 0 });
            }
        }
    }
    let mut gi = 0;
    for bin in start..end {
        let hb = hebap[bin];
        let mut pre = [0f32; 6];
        match hb {
            0 => {}
            1..=7 => {
                let idx = r.u(VQ_BITS[usize::from(hb - 1)]) as usize;
                let v: &[i16; 6] = match hb {
                    1 => &VQ1[idx],
                    2 => &VQ2[idx],
                    3 => &VQ3[idx],
                    4 => &VQ4[idx],
                    5 => &VQ5[idx],
                    6 => &VQ6[idx],
                    _ => &VQ7[idx],
                };
                for n in 0..6 {
                    pre[n] = f32::from(v[n]) / 32768.0;
                }
            }
            _ => {
                let h = usize::from(hb.min(19) - 8);
                let m = HE_BITS[h];
                // gain code: 0 → 1, 1 → 2, 2 → 4 (mapped values of Table E3.4)
                let gcode = if gaqmod != 0 && hb < endbap {
                    let c = gains.get(gi).copied().unwrap_or(0);
                    gi += 1;
                    usize::from(c)
                } else {
                    0
                };
                for v in pre.iter_mut() {
                    *v = gaq_mantissa(r, h, m, gcode);
                }
            }
        }
        // inverse DCT-II across the six blocks
        let e = pow2(exps[bin]);
        for (mb, o) in out.iter_mut().enumerate() {
            // C(k, m) = √2 Σ R_j X(k, j) cos(j (2m + 1) π / 12), R_0 = 1/√2, R_j = 1 (the
            // printed equation lost its radicals; this is the scale that matches real streams)
            let mut s = std::f32::consts::FRAC_1_SQRT_2 * pre[0];
            for (j, &x) in pre.iter().enumerate().skip(1) {
                s += x * idct_cos(j, mb);
            }
            o[bin] = std::f32::consts::SQRT_2 * s * e;
        }
    }
}

#[inline]
fn idct_cos(j: usize, m: usize) -> f32 {
    static T: std::sync::OnceLock<[[f32; 6]; 6]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = [[0f32; 6]; 6];
        for (j, row) in t.iter_mut().enumerate() {
            for (m, v) in row.iter_mut().enumerate() {
                *v = (j as f64 * (2 * m + 1) as f64 * std::f64::consts::PI / 12.0).cos() as f32;
            }
        }
        t
    })[j][m]
}

/// One GAQ mantissa (Tables E3.5 and E3.6). `h` is hebap − 8, `m` its bits, `gcode` the gain
/// (0: 1, 1: 2, 2: 4).
fn gaq_mantissa(r: &mut Bits, h: usize, m: u32, gcode: usize) -> f32 {
    let remap = |x: f32, g: usize| {
        let (a, b) = GAQ_REMAP[h][usize::from(x < 0.0)][g];
        x + f32::from(a) / 32768.0 * x + f32::from(b) / 32768.0
    };
    if gcode == 0 {
        let x = r.s(m) as f32 / (1u32 << (m - 1)) as f32;
        return remap(x, 0);
    }
    let small = m - gcode as u32;
    let code = r.s(small);
    if small >= 1 && code == -(1 << (small - 1)) {
        let large = if gcode == 1 { m - 1 } else { m };
        let x = r.s(large) as f32 / (1u32 << (large - 1)) as f32;
        remap(x, gcode)
    } else {
        let x = code as f32 / (1u32 << (small.max(1) - 1)) as f32;
        x / (1 << gcode) as f32
    }
}

/// The first bin of spectral extension sub-band `s` (Table E3.13).
fn spx_bin(s: usize) -> usize {
    25 + 12 * s
}

/// The sizes of the spectral extension bands.
fn spx_bands(st: &State) -> Vec<usize> {
    let mut sizes = vec![12];
    for bnd in st.spx_begin + 1..st.spx_end {
        if st.spxbndstrc[bnd] {
            if let Some(l) = sizes.last_mut() {
                *l += 12;
            }
        } else {
            sizes.push(12);
        }
    }
    sizes
}

/// High frequency synthesis of one channel (Annex E §3.6.4).
fn spx_synthesize(st: &mut State, ch: usize, atten: Option<usize>) {
    let sizes = spx_bands(st);
    let copystart = spx_bin(st.spxstrtf);
    let copyend = spx_bin(st.spx_begin);
    let tc = &mut st.coef[ch];
    let mut wrap = vec![false; sizes.len()];
    // translation
    let mut copy = copystart;
    let mut insert = copyend;
    for (bnd, &size) in sizes.iter().enumerate() {
        if copy + size > copyend {
            copy = copystart;
            wrap[bnd] = true;
        }
        for _ in 0..size {
            if copy == copyend {
                copy = copystart;
            }
            if insert < 256 {
                tc[insert] = tc[copy];
            }
            insert += 1;
            copy += 1;
        }
    }
    // banded RMS energy
    let mut rms = vec![0f32; sizes.len()];
    let mut at = copyend;
    for (bnd, &size) in sizes.iter().enumerate() {
        let mut acc = 0f32;
        for bin in at..(at + size).min(256) {
            acc += tc[bin] * tc[bin];
        }
        rms[bnd] = (acc / size as f32).sqrt();
        at += size;
    }
    // border notch filtering
    if let Some(code) = atten {
        let t = SPXATTENTAB[code.min(31)];
        let notch = |tc: &mut [f32; 256], at: usize| {
            for (k, &a) in [t[0], t[1], t[2], t[1], t[0]].iter().enumerate() {
                if let Some(v) = (at + k).checked_sub(2).and_then(|i| tc.get_mut(i)) {
                    *v *= a;
                }
            }
        };
        let mut border = copyend;
        notch(tc, border);
        border += sizes[0];
        for (bnd, &size) in sizes.iter().enumerate().skip(1) {
            if wrap[bnd] {
                notch(tc, border);
            }
            border += size;
        }
    }
    // noise blending and scaling
    let mut at = copyend;
    for (bnd, &size) in sizes.iter().enumerate() {
        let nscale = rms[bnd] * st.spx_nblend[ch][bnd] * 1.732_050_8;
        let sscale = st.spx_sblend[ch][bnd];
        let co = st.spxco[ch][bnd] * 32.0;
        for bin in at..(at + size).min(256) {
            let noise = st.rng.next();
            tc[bin] = (tc[bin] * sscale + noise * nscale) * co;
        }
        at += size;
    }
    let end = at.min(256);
    tc[end..].fill(0.0);
}
