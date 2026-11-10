//! The sequence header (Dirac specification 2.2.3 §10, SMPTE ST 2042-1 §11): parse
//! parameters, the base video format's defaults (Annex C, Tables C.1–C.3) and the source
//! parameters that override them.

use crate::bits::{Reader, intlog2};
use crate::{Chroma, Error, Result, Signal, bad};

/// What a sequence header says, with the base format's defaults filled in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Sequence {
    pub major: u32,
    pub profile: u32,
    pub width: u32,
    pub height: u32,
    pub chroma_index: u32,
    pub interlaced: bool,
    pub top_field_first: bool,
    pub luma_offset: u32,
    pub luma_excursion: u32,
    pub chroma_offset: u32,
    pub chroma_excursion: u32,
    /// Colour primaries, matrix and transfer function indices (Tables 10.7–10.9).
    pub primaries: u32,
    pub matrix: u32,
    pub transfer: u32,
    /// Picture coding mode 1: every picture is a field.
    pub fields: bool,
    /// Frames per second as a ratio (Table 10.3 presets or the stream's own).
    pub frame_rate: (u32, u32),
}

/// Table 10.3: the preset frame rates (index 1–10).
const FRAME_RATES: [(u32, u32); 10] = [
    (24000, 1001),
    (24, 1),
    (25, 1),
    (30000, 1001),
    (30, 1),
    (50, 1),
    (60000, 1001),
    (60, 1),
    (15000, 1001),
    (25, 2),
];

/// Annex C: the frame rate index of each base video format.
const BASE_RATES: [u8; 21] = [
    1, 9, 10, 9, 10, 9, 10, 4, 3, 7, 6, 4, 3, 7, 6, 2, 2, 7, 6, 7, 6,
];

fn preset_rate(i: u32) -> (u32, u32) {
    FRAME_RATES
        .get((i as usize).wrapping_sub(1))
        .copied()
        .unwrap_or((25, 1))
}

/// One row of Annex C: width, height, chroma format index, interlaced, top field first,
/// signal range preset, colour spec preset.
type Format = (u32, u32, u32, bool, bool, u32, u32);

/// Annex C, Tables C.1–C.3 (the parameters a decoder needs; frame rate, pixel aspect ratio and
/// clean area are not kept).
const FORMATS: [Format; 21] = [
    (640, 480, 2, false, false, 1, 0),  // 0 custom
    (176, 120, 2, false, false, 1, 1),  // 1 QSIF525
    (176, 144, 2, false, true, 1, 2),   // 2 QCIF
    (352, 240, 2, false, false, 1, 1),  // 3 SIF525
    (352, 288, 2, false, true, 1, 2),   // 4 CIF
    (704, 480, 2, false, false, 1, 1),  // 5 4SIF525
    (704, 576, 2, false, true, 1, 2),   // 6 4CIF
    (720, 480, 1, true, false, 3, 1),   // 7 SD 480I-60
    (720, 576, 1, true, true, 3, 2),    // 8 SD 576I-50
    (1280, 720, 1, false, true, 3, 3),  // 9 HD 720P-60
    (1280, 720, 1, false, true, 3, 3),  // 10 HD 720P-50
    (1920, 1080, 1, true, true, 3, 3),  // 11 HD 1080I-60
    (1920, 1080, 1, true, true, 3, 3),  // 12 HD 1080I-50
    (1920, 1080, 1, false, true, 3, 3), // 13 HD 1080P-60
    (1920, 1080, 1, false, true, 3, 3), // 14 HD 1080P-50
    (2048, 1080, 0, false, true, 4, 4), // 15 DC 2K-24
    (4096, 2160, 0, false, true, 4, 4), // 16 DC 4K-24
    (3840, 2160, 1, false, true, 3, 3), // 17 UHDTV 4K-60
    (3840, 2160, 1, false, true, 3, 3), // 18 UHDTV 4K-50
    (7680, 4320, 1, false, true, 3, 3), // 19 UHDTV 8K-60
    (7680, 4320, 1, false, true, 3, 3), // 20 UHDTV 8K-50
];

/// Table 10.5: luma offset, luma excursion, chroma offset, chroma excursion.
const SIGNAL_RANGES: [[u32; 4]; 4] = [
    [0, 255, 128, 255],
    [16, 219, 128, 224],
    [64, 876, 512, 896],
    [256, 3504, 2048, 3584],
];

/// Table 10.6: primaries, matrix, transfer function.
const COLOUR_SPECS: [[u32; 3]; 5] = [[0, 0, 0], [1, 1, 0], [2, 1, 0], [0, 0, 0], [0, 0, 3]];

/// The largest picture side accepted.
const MAX_SIDE: u32 = 16_384;

impl Sequence {
    /// Parses a sequence header data unit (after its parse info header).
    pub fn parse(d: &[u8]) -> Result<Sequence> {
        let mut r = Reader::new(d);
        let major = r.uint()?;
        let _minor = r.uint()?;
        let profile = r.uint()?;
        let _level = r.uint()?;
        let base = r.uint()?;
        // Formats beyond Annex C (later revisions add some) start from the custom defaults;
        // their streams normally override every size and range.
        let f = FORMATS.get(base as usize).copied().unwrap_or(FORMATS[0]);
        let mut s = Sequence {
            major,
            profile,
            width: f.0,
            height: f.1,
            chroma_index: f.2,
            interlaced: f.3,
            top_field_first: f.4,
            luma_offset: 0,
            luma_excursion: 0,
            chroma_offset: 0,
            chroma_excursion: 0,
            primaries: 0,
            matrix: 0,
            transfer: 0,
            fields: false,
            frame_rate: preset_rate(u32::from(
                BASE_RATES.get(base as usize).copied().unwrap_or(3),
            )),
        };
        [
            s.luma_offset,
            s.luma_excursion,
            s.chroma_offset,
            s.chroma_excursion,
        ] = SIGNAL_RANGES[f.5 as usize - 1];
        [s.primaries, s.matrix, s.transfer] = COLOUR_SPECS[f.6 as usize];
        if r.bool()? {
            s.width = r.uint()?;
            s.height = r.uint()?;
        }
        if r.bool()? {
            s.chroma_index = r.uint()?;
        }
        if r.bool()? {
            s.interlaced = r.uint()? == 1;
        }
        // Frame rate and pixel aspect ratio: a preset index, or 0 and the ratio.
        for k in 0..2 {
            if r.bool()? {
                let index = r.uint()?;
                if index == 0 {
                    let (n, d) = (r.uint()?, r.uint()?);
                    if k == 0 && n > 0 && d > 0 {
                        s.frame_rate = (n, d);
                    }
                } else if k == 0 {
                    s.frame_rate = preset_rate(index);
                }
            }
        }
        if r.bool()? {
            for _ in 0..4 {
                r.uint()?;
            }
        }
        if r.bool()? {
            let index = r.uint()?;
            if index == 0 {
                s.luma_offset = r.uint()?;
                s.luma_excursion = r.uint()?;
                s.chroma_offset = r.uint()?;
                s.chroma_excursion = r.uint()?;
            } else {
                let p = SIGNAL_RANGES
                    .get(index as usize - 1)
                    .ok_or_else(|| bad("signal range preset"))?;
                [
                    s.luma_offset,
                    s.luma_excursion,
                    s.chroma_offset,
                    s.chroma_excursion,
                ] = *p;
            }
        }
        if r.bool()? {
            let index = r.uint()?;
            let p = COLOUR_SPECS
                .get(index as usize)
                .ok_or_else(|| bad("colour specification preset"))?;
            [s.primaries, s.matrix, s.transfer] = *p;
            if index == 0 {
                if r.bool()? {
                    s.primaries = r.uint()?;
                }
                if r.bool()? {
                    s.matrix = r.uint()?;
                }
                if r.bool()? {
                    s.transfer = r.uint()?;
                }
            }
        }
        let mode = r.uint()?;
        if mode > 1 {
            return Err(Error::Unsupported(format!(
                "VC-2 picture coding mode {mode}"
            )));
        }
        s.fields = mode == 1;
        s.check()?;
        Ok(s)
    }

    fn check(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 || self.width > MAX_SIDE || self.height > MAX_SIDE {
            return Err(bad(format!("picture size {}×{}", self.width, self.height)));
        }
        if self.chroma_index > 2 {
            return Err(bad("chroma format"));
        }
        if self.fields && self.height < 2 {
            return Err(bad("field picture without lines"));
        }
        for d in [self.luma_depth(), self.chroma_depth()] {
            if !(1..=16).contains(&d) {
                return Err(Error::Unsupported(format!("VC-2 video at {d} bits")));
            }
        }
        Ok(())
    }

    pub fn luma_depth(&self) -> u32 {
        intlog2(u64::from(self.luma_excursion) + 1)
    }

    pub fn chroma_depth(&self) -> u32 {
        intlog2(u64::from(self.chroma_excursion) + 1)
    }

    pub fn chroma(&self) -> Chroma {
        match self.chroma_index {
            0 => Chroma::C444,
            1 => Chroma::C422,
            _ => Chroma::C420,
        }
    }

    /// The coded picture's luma size (a field is half the frame's height, §10.5.1).
    pub fn luma_size(&self) -> (usize, usize) {
        let h = if self.fields {
            self.height / 2
        } else {
            self.height
        };
        (self.width as usize, h as usize)
    }

    /// The coded picture's chroma size: luma halved and rounded down.
    pub fn chroma_size(&self) -> (usize, usize) {
        let (w, h) = self.luma_size();
        match self.chroma_index {
            0 => (w, h),
            1 => (w / 2, h),
            _ => (w / 2, h / 2),
        }
    }

    /// H.273 code points of the colour specification; full range when the luma covers every
    /// code at its depth.
    pub fn signal(&self) -> Signal {
        let primaries = match self.primaries {
            0 => Some(1),
            1 => Some(6),
            2 => Some(5),
            3 => Some(10),
            _ => None,
        };
        let matrix = match self.matrix {
            0 => Some(1),
            1 => Some(6),
            2 => Some(8),
            _ => None,
        };
        let transfer = match self.transfer {
            0 => Some(1),
            1 => Some(12),
            2 => Some(8),
            3 => Some(17),
            _ => None,
        };
        Signal {
            full_range: self.luma_offset == 0
                && u64::from(self.luma_excursion) + 1 == 1u64 << self.luma_depth(),
            primaries,
            transfer,
            matrix,
        }
    }
}
