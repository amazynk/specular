//! Sequence-level parameters: STRUCT_C of Simple and Main profile (Annex J), and the Advanced
//! profile sequence header (§6.1) and entry-point header (§6.2).

use crate::bits::Bits;
use crate::{Error, malformed, unsupported};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Simple,
    Main,
    Advanced,
}

/// The coding parameters in force for a picture: the sequence's, updated by entry points.
#[derive(Clone, Debug)]
pub struct Seq {
    pub profile: Profile,
    pub level: u8,
    /// Coded size in pixels.
    pub width: u32,
    pub height: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub loopfilter: bool,
    pub multires: bool,
    pub fastuvmc: bool,
    pub extended_mv: bool,
    pub dquant: u8,
    pub vstransform: bool,
    pub overlap: bool,
    pub syncmarker: bool,
    /// RES_RTM_FLAG: clear in WMV9 streams from encoders before the release, whose inter
    /// blocks after the first in a macroblock repeat the sub-block pattern of a split
    /// transform.
    pub rtm: bool,
    pub rangered: bool,
    pub max_b_frames: u8,
    pub quantizer: u8,
    pub finterpflag: bool,
    // Advanced only.
    pub postprocflag: bool,
    pub pulldown: bool,
    pub interlace: bool,
    pub tfcntrflag: bool,
    pub psf: bool,
    pub hrd_buckets: u8,
    pub colour: Option<(u8, u8, u8)>,
    pub display: Option<(u32, u32)>,
    pub aspect: Option<(u32, u32)>,
    pub frame_rate: Option<(u32, u32)>,
    // Entry point (Advanced).
    pub entry_seen: bool,
    pub broken_link: bool,
    pub closed_entry: bool,
    pub panscan: bool,
    pub refdist_flag: bool,
    pub extended_dmv: bool,
    pub range_mapy: Option<u8>,
    pub range_mapuv: Option<u8>,
}

impl Seq {
    fn empty(profile: Profile) -> Seq {
        Seq {
            profile,
            level: 0,
            width: 0,
            height: 0,
            max_width: 0,
            max_height: 0,
            loopfilter: false,
            multires: false,
            fastuvmc: false,
            extended_mv: false,
            dquant: 0,
            vstransform: false,
            overlap: false,
            syncmarker: false,
            rtm: true,
            rangered: false,
            max_b_frames: 0,
            quantizer: 0,
            finterpflag: false,
            postprocflag: false,
            pulldown: false,
            interlace: false,
            tfcntrflag: false,
            psf: false,
            hrd_buckets: 0,
            colour: None,
            display: None,
            aspect: None,
            frame_rate: None,
            entry_seen: false,
            broken_link: false,
            closed_entry: true,
            panscan: false,
            refdist_flag: false,
            extended_dmv: false,
            range_mapy: None,
            range_mapuv: None,
        }
    }

    /// Simple and Main profile parameters from STRUCT_C (Table 263) and the container's
    /// frame size.
    pub fn struct_c(c: &[u8], width: u32, height: u32) -> Result<Seq, Error> {
        if c.len() < 4 {
            return Err(malformed("the WMV3 sequence header is cut short"));
        }
        let mut r = Bits::new(c);
        let profile = match r.u(4) {
            0 => Profile::Simple,
            4 => Profile::Main,
            // Not part of SMPTE 421M: the WMV9 encoder's "complex" setting.
            8 => return Err(unsupported("Windows Media Video 9 Complex profile")),
            12 => {
                return Err(malformed(
                    "an Advanced profile STRUCT_C without its headers",
                ));
            }
            _ => return Err(unsupported("VC-1 with a reserved profile")),
        };
        let mut s = Seq::empty(profile);
        let _frmrtq = r.u(3);
        let _bitrtq = r.u(5);
        s.loopfilter = r.flag();
        let res3 = r.bit();
        s.multires = r.flag();
        let res4 = r.bit();
        s.fastuvmc = r.flag();
        s.extended_mv = r.flag();
        s.dquant = r.u(2) as u8;
        s.vstransform = r.flag();
        let res5 = r.bit();
        s.overlap = r.flag();
        s.syncmarker = r.flag();
        s.rangered = r.flag();
        s.max_b_frames = r.u(3) as u8;
        s.quantizer = r.u(2) as u8;
        s.finterpflag = r.flag();
        let res6 = r.bit();
        // Bitstreams from before the standard set these differently (X8 intra pictures, an
        // other transform, pre-release syntax).
        if res3 != 0 {
            return Err(unsupported("WMV3 with X8 intra pictures"));
        }
        s.rtm = res6 == 1;
        if res4 != 1 || res5 != 0 {
            return Err(unsupported("pre-release WMV9 video"));
        }
        if s.dquant > 2 {
            return Err(malformed("reserved DQUANT"));
        }
        if width < 2 || height < 2 || width > 8192 || height > 8192 {
            return Err(unsupported("WMV3 video of this size"));
        }
        s.width = width;
        s.height = height;
        s.max_width = width;
        s.max_height = height;
        s.entry_seen = true;
        Ok(s)
    }

    /// The Advanced profile sequence header (Table 3), from its RBDU (after the suffix byte).
    pub fn advanced(rbdu: &[u8]) -> Result<Seq, Error> {
        let mut r = Bits::new(rbdu);
        if r.u(2) != 3 {
            return Err(unsupported("VC-1 with a reserved profile"));
        }
        let mut s = Seq::empty(Profile::Advanced);
        s.level = r.u(3) as u8;
        if r.u(2) != 1 {
            return Err(unsupported("VC-1 in a colour format other than 4:2:0"));
        }
        let _frmrtq = r.u(3);
        let _bitrtq = r.u(5);
        s.postprocflag = r.flag();
        s.max_width = 2 * r.u(12) + 2;
        s.max_height = 2 * r.u(12) + 2;
        s.width = s.max_width;
        s.height = s.max_height;
        s.pulldown = r.flag();
        s.interlace = r.flag();
        s.tfcntrflag = r.flag();
        s.finterpflag = r.flag();
        let _reserved = r.bit();
        s.psf = r.flag();
        if r.flag() {
            // DISPLAY_EXT
            let dw = r.u(14) + 1;
            let dh = r.u(14) + 1;
            s.display = Some((dw, dh));
            if r.flag() {
                let ar = r.u(4);
                s.aspect = match ar {
                    15 => Some((r.u(8) + 1, r.u(8) + 1)),
                    1..=13 => Some(ASPECT[ar as usize - 1]),
                    _ => None,
                };
            }
            if r.flag() {
                if r.flag() {
                    let exp = r.u(16);
                    s.frame_rate = Some((exp + 1, 32));
                } else {
                    let nr = r.u(8);
                    let dr = r.u(4);
                    let num = match nr {
                        1 => 24_000,
                        2 => 25_000,
                        3 => 30_000,
                        4 => 50_000,
                        5 => 60_000,
                        6 => 48_000,
                        7 => 72_000,
                        _ => 0,
                    };
                    let den = match dr {
                        1 => 1000,
                        2 => 1001,
                        _ => 0,
                    };
                    if num != 0 && den != 0 {
                        s.frame_rate = Some((num, den));
                    }
                }
            }
            if r.flag() {
                let p = r.u(8) as u8;
                let t = r.u(8) as u8;
                let m = r.u(8) as u8;
                s.colour = Some((p, t, m));
            }
        }
        if r.flag() {
            s.hrd_buckets = r.u(5) as u8;
            let _ = r.u(4);
            let _ = r.u(4);
            for _ in 0..s.hrd_buckets {
                let _ = r.u(16);
                let _ = r.u(16);
            }
        }
        if r.overrun() {
            return Err(malformed("the sequence header is cut short"));
        }
        if s.level > 4 {
            return Err(unsupported("VC-1 with a reserved level"));
        }
        Ok(s)
    }

    /// The entry-point header (Table 14).
    pub fn entry_point(&mut self, rbdu: &[u8]) -> Result<(), Error> {
        let mut r = Bits::new(rbdu);
        self.broken_link = r.flag();
        self.closed_entry = r.flag();
        self.panscan = r.flag();
        self.refdist_flag = r.flag();
        self.loopfilter = r.flag();
        self.fastuvmc = r.flag();
        self.extended_mv = r.flag();
        self.dquant = r.u(2) as u8;
        self.vstransform = r.flag();
        self.overlap = r.flag();
        self.quantizer = r.u(2) as u8;
        for _ in 0..self.hrd_buckets {
            let _ = r.u(8);
        }
        if r.flag() {
            self.width = 2 * r.u(12) + 2;
            self.height = 2 * r.u(12) + 2;
        } else {
            self.width = self.max_width;
            self.height = self.max_height;
        }
        self.extended_dmv = self.extended_mv && r.flag();
        self.range_mapy = if r.flag() { Some(r.u(3) as u8) } else { None };
        self.range_mapuv = if r.flag() { Some(r.u(3) as u8) } else { None };
        if r.overrun() {
            return Err(malformed("the entry-point header is cut short"));
        }
        if self.dquant > 2 {
            return Err(malformed("reserved DQUANT"));
        }
        self.entry_seen = true;
        Ok(())
    }

    pub fn mb_width(&self) -> usize {
        self.width.div_ceil(16) as usize
    }

    pub fn mb_height(&self) -> usize {
        self.height.div_ceil(16) as usize
    }
}

/// Table 7: sample aspect ratios 1..13.
const ASPECT: [(u32, u32); 13] = [
    (1, 1),
    (12, 11),
    (10, 11),
    (16, 11),
    (40, 33),
    (24, 11),
    (20, 11),
    (32, 11),
    (80, 33),
    (18, 11),
    (15, 11),
    (64, 33),
    (160, 99),
];
