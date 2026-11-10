//! Picture layer headers (§7.1.1, §9.1.1).

use crate::bitplane::Bitplane;
use crate::bits::Bits;
use crate::seq::{Profile, Seq};
use crate::small::{BFRACTION_LONG, BFRACTION_SHORT, INVERSE, PQINDEX_TO_PQUANT};
use crate::{Error, malformed, unsupported};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicType {
    I,
    P,
    B,
    Bi,
    Skipped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fcm {
    Progressive,
    FrameInterlace,
    FieldInterlace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MvMode {
    OneMv,
    Mixed,
    /// 1-MV half-pel, bicubic.
    HalfPel,
    /// 1-MV half-pel, bilinear.
    HalfPelBilinear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DqProfile {
    #[default]
    AllEdges,
    DoubleEdge,
    SingleEdge,
    AllMbs,
}

/// VOPDQUANT (Table 24).
#[derive(Clone, Copy, Debug, Default)]
pub struct Dquant {
    pub in_frame: bool,
    pub profile: DqProfile,
    /// The edge (Table 44) or edge pair (Table 45).
    pub edge: u8,
    pub bilevel: bool,
    pub altpquant: u8,
}

#[derive(Clone, Debug)]
pub struct PicHeader {
    pub ptype: PicType,
    pub fcm: Fcm,
    pub rangeredfrm: bool,
    pub pqindex: u8,
    pub pquant: u8,
    pub halfqp: bool,
    pub uniform: bool,
    pub mvrange: usize,
    pub respic: u8,
    /// Coding set index for Cb/Cr (and inter) blocks; for Y blocks of I pictures.
    pub transacfrm: usize,
    pub transacfrm2: usize,
    pub transdctab: bool,
    /// BFRACTION as a scale factor (numerator × 256 / denominator, Figure 70).
    pub scale_factor: i32,
    pub mvmode: MvMode,
    pub intensity: Option<(u8, u8)>,
    pub mvtypemb: Bitplane,
    pub skipmb: Bitplane,
    pub directmb: Bitplane,
    pub acpred: Bitplane,
    pub overflags: Bitplane,
    pub condover: u8,
    pub mvtab: usize,
    pub cbptab: usize,
    pub dq: Dquant,
    pub ttmbf: bool,
    pub ttfrm: u8,
    pub rnd: bool,
    pub tff: bool,
    pub rff: bool,
    pub rptfrm: u8,
    /// Bit position just after the header (macroblock data start).
    pub mb_start: usize,
}

impl PicHeader {
    fn new() -> PicHeader {
        PicHeader {
            ptype: PicType::I,
            fcm: Fcm::Progressive,
            rangeredfrm: false,
            pqindex: 0,
            pquant: 1,
            halfqp: false,
            uniform: true,
            mvrange: 0,
            respic: 0,
            transacfrm: 0,
            transacfrm2: 0,
            transdctab: false,
            scale_factor: 128,
            mvmode: MvMode::OneMv,
            intensity: None,
            mvtypemb: Bitplane::default(),
            skipmb: Bitplane::default(),
            directmb: Bitplane::default(),
            acpred: Bitplane::default(),
            overflags: Bitplane::default(),
            condover: 0,
            mvtab: 0,
            cbptab: 0,
            dq: Dquant::default(),
            ttmbf: true,
            ttfrm: 0,
            rnd: false,
            tff: true,
            rff: false,
            rptfrm: 0,
            mb_start: 0,
        }
    }

    pub fn is_intra(&self) -> bool {
        matches!(self.ptype, PicType::I | PicType::Bi)
    }
}

/// TRANSACFRM: 0b, 10b, 11b.
fn transac(r: &mut Bits) -> usize {
    if r.flag() { 1 + r.bit() as usize } else { 0 }
}

/// BFRACTION (Table 40): the scale factor, or None for the BI code 1111111b.
fn bfraction(r: &mut Bits) -> Result<Option<i32>, Error> {
    let c = r.u(3) as usize;
    let (num, den) = if c < 7 {
        BFRACTION_SHORT[c]
    } else {
        let l = r.u(4) as usize;
        match l {
            0..=13 => BFRACTION_LONG[l],
            14 => return Err(malformed("reserved BFRACTION")),
            _ => return Ok(None),
        }
    };
    Ok(Some(i32::from(num) * INVERSE[den as usize - 1]))
}

fn quantizer(h: &mut PicHeader, r: &mut Bits, seq: &Seq) -> Result<(), Error> {
    h.pqindex = r.u(5) as u8;
    if h.pqindex == 0 {
        return Err(malformed("PQINDEX 0"));
    }
    if h.pqindex <= 8 {
        h.halfqp = r.flag();
    }
    match seq.quantizer {
        0 => {
            h.pquant = PQINDEX_TO_PQUANT[h.pqindex as usize];
            h.uniform = h.pqindex <= 8;
        }
        1 => {
            h.pquant = h.pqindex;
            h.uniform = r.flag();
        }
        2 => {
            h.pquant = h.pqindex;
            h.uniform = false;
        }
        _ => {
            h.pquant = h.pqindex;
            h.uniform = true;
        }
    }
    Ok(())
}

/// MVMODE and, with intensity compensation, MVMODE2, LUMSCALE and LUMSHIFT (P pictures).
fn mvmode_p(h: &mut PicHeader, r: &mut Bits) {
    let low = h.pquant > 12;
    let mut z = 0;
    while z < 3 && !r.flag() {
        z += 1;
    }
    let code = if z == 3 { 3 + r.bit() } else { z };
    // code: 0 = 1b, 1 = 01b, 2 = 001b, 3 = 0000b, 4 = 0001b.
    let modes = if low {
        [
            MvMode::HalfPelBilinear,
            MvMode::OneMv,
            MvMode::HalfPel,
            MvMode::Mixed,
        ]
    } else {
        [
            MvMode::OneMv,
            MvMode::Mixed,
            MvMode::HalfPel,
            MvMode::HalfPelBilinear,
        ]
    };
    if code == 4 {
        let mut z = 0;
        while z < 3 && !r.flag() {
            z += 1;
        }
        h.mvmode = modes[z as usize];
        let scale = r.u(6) as u8;
        let shift = r.u(6) as u8;
        h.intensity = Some((scale, shift));
    } else {
        h.mvmode = modes[code as usize];
    }
}

fn vopdquant(h: &mut PicHeader, r: &mut Bits, seq: &Seq) {
    let mut dq = Dquant::default();
    let alt = |r: &mut Bits, pq: u8| -> u8 {
        let d = r.u(3) as u8;
        if d == 7 { r.u(5) as u8 } else { pq + d + 1 }
    };
    if seq.dquant == 2 {
        dq.in_frame = true;
        dq.profile = DqProfile::AllEdges;
        dq.altpquant = alt(r, h.pquant);
    } else if r.flag() {
        dq.in_frame = true;
        dq.profile = match r.u(2) {
            0 => DqProfile::AllEdges,
            1 => DqProfile::DoubleEdge,
            2 => DqProfile::SingleEdge,
            _ => DqProfile::AllMbs,
        };
        match dq.profile {
            DqProfile::SingleEdge | DqProfile::DoubleEdge => dq.edge = r.u(2) as u8,
            DqProfile::AllMbs => dq.bilevel = r.flag(),
            DqProfile::AllEdges => {}
        }
        if !(dq.profile == DqProfile::AllMbs && !dq.bilevel) {
            dq.altpquant = alt(r, h.pquant);
        }
    }
    h.dq = dq;
}

fn ttmbf(h: &mut PicHeader, r: &mut Bits, seq: &Seq) {
    if seq.vstransform {
        h.ttmbf = r.flag();
        if h.ttmbf {
            h.ttfrm = r.u(2) as u8;
        }
    } else {
        h.ttmbf = true;
        h.ttfrm = 0;
    }
}

/// The Simple/Main profile picture header (Tables 16, 17, 19, 21).
pub fn parse_simple_main(
    r: &mut Bits,
    seq: &Seq,
    mb_w: usize,
    mb_h: usize,
) -> Result<PicHeader, Error> {
    let mut h = PicHeader::new();
    if seq.finterpflag {
        let _ = r.bit();
    }
    let _frmcnt = r.u(2);
    if seq.rangered {
        h.rangeredfrm = r.flag();
    }
    h.ptype = if seq.max_b_frames == 0 {
        if r.flag() { PicType::P } else { PicType::I }
    } else if r.flag() {
        PicType::P
    } else if r.flag() {
        PicType::I
    } else {
        PicType::B
    };
    if h.ptype == PicType::B {
        if seq.profile == Profile::Simple {
            return Err(malformed("a B picture in Simple profile"));
        }
        match bfraction(r)? {
            Some(s) => h.scale_factor = s,
            None => h.ptype = PicType::Bi,
        }
    }
    if h.is_intra() {
        let _bf = r.u(7);
    }
    quantizer(&mut h, r, seq)?;
    if seq.extended_mv {
        h.mvrange = r.unary(3) as usize;
    }
    if seq.multires && matches!(h.ptype, PicType::I | PicType::P) {
        h.respic = r.u(2) as u8;
    }
    match h.ptype {
        PicType::I | PicType::Bi => {
            h.transacfrm = transac(r);
            h.transacfrm2 = transac(r);
            h.transdctab = r.flag();
        }
        PicType::P => {
            mvmode_p(&mut h, r);
            if h.mvmode == MvMode::Mixed {
                h.mvtypemb = Bitplane::read(r, mb_w, mb_h)?;
            }
            h.skipmb = Bitplane::read(r, mb_w, mb_h)?;
            h.mvtab = r.u(2) as usize;
            h.cbptab = r.u(2) as usize;
            if seq.dquant != 0 {
                vopdquant(&mut h, r, seq);
            }
            ttmbf(&mut h, r, seq);
            h.transacfrm = transac(r);
            h.transdctab = r.flag();
        }
        PicType::B => {
            h.mvmode = if r.flag() {
                MvMode::OneMv
            } else {
                MvMode::HalfPelBilinear
            };
            h.directmb = Bitplane::read(r, mb_w, mb_h)?;
            h.skipmb = Bitplane::read(r, mb_w, mb_h)?;
            h.mvtab = r.u(2) as usize;
            h.cbptab = r.u(2) as usize;
            if seq.dquant != 0 {
                vopdquant(&mut h, r, seq);
            }
            ttmbf(&mut h, r, seq);
            h.transacfrm = transac(r);
            h.transdctab = r.flag();
        }
        PicType::Skipped => {}
    }
    if r.overrun() {
        return Err(malformed("the picture header is cut short"));
    }
    h.mb_start = r.position();
    Ok(h)
}

/// The Advanced profile frame header of a progressive or interlaced-frame picture (Tables
/// 18, 20, 22, 23, 82–84). Field pictures are parsed by `field_header`.
pub fn parse_advanced(
    r: &mut Bits,
    seq: &Seq,
    mb_w: usize,
    mb_h: usize,
) -> Result<PicHeader, Error> {
    let mut h = PicHeader::new();
    if seq.interlace {
        h.fcm = if !r.flag() {
            Fcm::Progressive
        } else if !r.flag() {
            Fcm::FrameInterlace
        } else {
            Fcm::FieldInterlace
        };
    }
    if h.fcm == Fcm::FieldInterlace {
        return Err(unsupported("VC-1 interlaced field pictures"));
    }
    h.ptype = match r.unary(4) {
        0 => PicType::P,
        1 => PicType::B,
        2 => PicType::I,
        3 => PicType::Bi,
        _ => PicType::Skipped,
    };
    if h.ptype != PicType::Skipped && seq.tfcntrflag {
        let _ = r.u(8);
    }
    if seq.pulldown {
        if !seq.interlace || seq.psf {
            h.rptfrm = r.u(2) as u8;
        } else {
            h.tff = r.flag();
            h.rff = r.flag();
        }
    }
    if seq.panscan && r.flag() {
        let windows = if seq.interlace && !seq.psf {
            2 + if seq.pulldown { h.rff as u32 } else { 0 }
        } else {
            1 + if seq.pulldown { u32::from(h.rptfrm) } else { 0 }
        };
        for _ in 0..windows {
            let _ = r.u(18);
            let _ = r.u(18);
            let _ = r.u(14);
            let _ = r.u(14);
        }
    }
    if h.ptype == PicType::Skipped {
        h.mb_start = r.position();
        return Ok(h);
    }
    h.rnd = r.flag();
    if seq.interlace {
        let _uvsamp = r.bit();
    }
    if seq.finterpflag {
        let _ = r.bit();
    }
    if h.ptype == PicType::B {
        match bfraction(r)? {
            Some(s) => h.scale_factor = s,
            None => return Err(malformed("BFRACTION 1111111b in Advanced profile")),
        }
    }
    quantizer(&mut h, r, seq)?;
    if seq.postprocflag {
        let _ = r.u(2);
    }
    if h.fcm == Fcm::FrameInterlace {
        return Err(unsupported("VC-1 interlaced frame pictures"));
    }
    match h.ptype {
        PicType::I | PicType::Bi => {
            h.acpred = Bitplane::read(r, mb_w, mb_h)?;
            if seq.overlap && h.pquant <= 8 {
                h.condover = if !r.flag() {
                    0
                } else if !r.flag() {
                    2
                } else {
                    3
                };
                if h.condover == 3 {
                    h.overflags = Bitplane::read(r, mb_w, mb_h)?;
                }
            }
            h.transacfrm = transac(r);
            h.transacfrm2 = transac(r);
            h.transdctab = r.flag();
            if seq.dquant != 0 {
                vopdquant(&mut h, r, seq);
            }
        }
        PicType::P => {
            if seq.extended_mv {
                h.mvrange = r.unary(3) as usize;
            }
            mvmode_p(&mut h, r);
            if h.mvmode == MvMode::Mixed {
                h.mvtypemb = Bitplane::read(r, mb_w, mb_h)?;
            }
            h.skipmb = Bitplane::read(r, mb_w, mb_h)?;
            h.mvtab = r.u(2) as usize;
            h.cbptab = r.u(2) as usize;
            if seq.dquant != 0 {
                vopdquant(&mut h, r, seq);
            }
            ttmbf(&mut h, r, seq);
            h.transacfrm = transac(r);
            h.transdctab = r.flag();
        }
        PicType::B => {
            if seq.extended_mv {
                h.mvrange = r.unary(3) as usize;
            }
            h.mvmode = if r.flag() {
                MvMode::OneMv
            } else {
                MvMode::HalfPelBilinear
            };
            h.directmb = Bitplane::read(r, mb_w, mb_h)?;
            h.skipmb = Bitplane::read(r, mb_w, mb_h)?;
            h.mvtab = r.u(2) as usize;
            h.cbptab = r.u(2) as usize;
            if seq.dquant != 0 {
                vopdquant(&mut h, r, seq);
            }
            ttmbf(&mut h, r, seq);
            h.transacfrm = transac(r);
            h.transdctab = r.flag();
        }
        PicType::Skipped => {}
    }
    if r.overrun() {
        return Err(malformed("the picture header is cut short"));
    }
    h.mb_start = r.position();
    Ok(h)
}
