//! The frame tag, the key frame start code and the frame header at the start of the first
//! partition (RFC 6386 sections 9 and 19.2), with the probabilities and adjustments that
//! persist from frame to frame.

use crate::boolcoder::BoolDecoder;
use crate::tables::*;
use crate::{Error, malformed};

/// Probabilities that persist across frames (reset on key frames).
#[derive(Clone)]
pub(crate) struct Probs {
    /// [plane][band][context][node].
    pub coeff: [[[[u8; 11]; 3]; 8]; 4],
    pub ymode: [u8; 4],
    pub uv_mode: [u8; 3],
    /// Row, then column.
    pub mv: [[u8; 19]; 2],
}

impl Probs {
    pub(crate) fn defaults() -> Probs {
        let mut coeff = [[[[0u8; 11]; 3]; 8]; 4];
        for (i, p) in coeff.iter_mut().flatten().flatten().enumerate() {
            p.copy_from_slice(&DEFAULT_COEFF_PROBS[i * 11..i * 11 + 11]);
        }
        let mut mv = [[0u8; 19]; 2];
        mv[0].copy_from_slice(&DEFAULT_MV_CONTEXT[..19]);
        mv[1].copy_from_slice(&DEFAULT_MV_CONTEXT[19..]);
        Probs {
            coeff,
            ymode: YMODE_PROB,
            uv_mode: UV_MODE_PROB,
            mv,
        }
    }
}

/// Segment-based adjustments (section 9.3); the values persist until changed.
#[derive(Clone, Default)]
pub(crate) struct Segmentation {
    pub enabled: bool,
    pub update_map: bool,
    /// The values replace the frame's (true) or are added to them (false).
    pub absolute: bool,
    pub quant: [i32; 4],
    pub lf: [i32; 4],
    pub tree_probs: [u8; 3],
}

/// Loop filter level deltas by reference frame and mode (section 9.4); persistent.
#[derive(Clone, Default)]
pub(crate) struct LfDeltas {
    pub enabled: bool,
    /// Intra, last, golden, altref.
    pub by_ref: [i32; 4],
    /// B_PRED, ZEROMV, other MV modes (nearest, near, new), SPLITMV.
    pub by_mode: [i32; 4],
}

/// The state the header updates and later frames read.
#[derive(Clone)]
pub(crate) struct Persistent {
    pub probs: Probs,
    pub seg: Segmentation,
    pub lf: LfDeltas,
}

impl Persistent {
    pub(crate) fn new() -> Persistent {
        Persistent {
            probs: Probs::defaults(),
            seg: Segmentation::default(),
            lf: LfDeltas::default(),
        }
    }
}

/// The uncompressed chunk at the start of a frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tag {
    pub key: bool,
    pub version: u8,
    pub show: bool,
    pub first_size: usize,
    /// Key frames: width, height (14 bits each) and their 2-bit scales.
    pub width: u32,
    pub height: u32,
    pub h_scale: u8,
    pub v_scale: u8,
    /// Bytes before the first partition.
    pub header_len: usize,
}

pub(crate) fn parse_tag(data: &[u8]) -> Result<Tag, Error> {
    if data.len() < 3 {
        return Err(malformed("frame shorter than its tag"));
    }
    let t = data[0] as u32 | (data[1] as u32) << 8 | (data[2] as u32) << 16;
    let mut tag = Tag {
        key: t & 1 == 0,
        version: ((t >> 1) & 7) as u8,
        show: (t >> 4) & 1 == 1,
        first_size: (t >> 5) as usize,
        width: 0,
        height: 0,
        h_scale: 0,
        v_scale: 0,
        header_len: 3,
    };
    if tag.key {
        if data.len() < 10 {
            return Err(malformed("key frame shorter than its header"));
        }
        if data[3..6] != [0x9d, 0x01, 0x2a] {
            return Err(malformed("key frame without the start code"));
        }
        let w = u16::from_le_bytes([data[6], data[7]]);
        let h = u16::from_le_bytes([data[8], data[9]]);
        tag.width = (w & 0x3fff) as u32;
        tag.height = (h & 0x3fff) as u32;
        tag.h_scale = (w >> 14) as u8;
        tag.v_scale = (h >> 14) as u8;
        tag.header_len = 10;
    }
    Ok(tag)
}

/// The per-frame header fields.
#[derive(Clone, Debug, Default)]
pub(crate) struct Header {
    pub key: bool,
    pub version: u8,
    pub color_space: bool,
    pub clamping: bool,
    pub simple_filter: bool,
    pub filter_level: i32,
    pub sharpness: i32,
    pub partitions: usize,
    pub q_index: i32,
    pub y1_dc: i32,
    pub y2_dc: i32,
    pub y2_ac: i32,
    pub uv_dc: i32,
    pub uv_ac: i32,
    pub refresh_golden: bool,
    pub refresh_alt: bool,
    /// 0: none, 1: last frame, 2: altref (golden) or golden (altref).
    pub copy_to_golden: u8,
    pub copy_to_alt: u8,
    /// Indexed by reference frame (intra, last, golden, altref).
    pub sign_bias: [bool; 4],
    pub refresh_entropy: bool,
    pub refresh_last: bool,
    /// The per-macroblock skip flag is coded (with `prob_skip`).
    pub skip_coded: bool,
    pub prob_skip: u8,
    pub prob_intra: u8,
    pub prob_last: u8,
    pub prob_golden: u8,
}

/// Reads the frame header from the first partition, updating `st`. When the frame does
/// not keep its probability updates, the probabilities to restore after it are returned.
pub(crate) fn parse_header(
    d: &mut BoolDecoder<'_>,
    tag: &Tag,
    st: &mut Persistent,
) -> Result<(Header, Option<Probs>), Error> {
    let mut h = Header {
        key: tag.key,
        version: tag.version,
        ..Header::default()
    };
    if tag.key {
        h.color_space = d.flag();
        h.clamping = d.flag();
        st.probs = Probs::defaults();
        st.seg = Segmentation::default();
        st.lf = LfDeltas::default();
    }
    // Segmentation.
    st.seg.enabled = d.flag();
    st.seg.update_map = false;
    if st.seg.enabled {
        st.seg.update_map = d.flag();
        let update_data = d.flag();
        if update_data {
            st.seg.absolute = d.flag();
            for q in &mut st.seg.quant {
                *q = d.optional_signed(7);
            }
            for l in &mut st.seg.lf {
                *l = d.optional_signed(6);
            }
        }
        if st.seg.update_map {
            for p in &mut st.seg.tree_probs {
                *p = if d.flag() { d.literal(8) as u8 } else { 255 };
            }
        }
    }
    // Loop filter.
    h.simple_filter = d.flag();
    h.filter_level = d.literal(6) as i32;
    h.sharpness = d.literal(3) as i32;
    st.lf.enabled = d.flag();
    if st.lf.enabled && d.flag() {
        for v in st.lf.by_ref.iter_mut().chain(st.lf.by_mode.iter_mut()) {
            if d.flag() {
                *v = d.signed(6);
            }
        }
    }
    h.partitions = 1 << d.literal(2);
    // Quantisers.
    h.q_index = d.literal(7) as i32;
    h.y1_dc = d.optional_signed(4);
    h.y2_dc = d.optional_signed(4);
    h.y2_ac = d.optional_signed(4);
    h.uv_dc = d.optional_signed(4);
    h.uv_ac = d.optional_signed(4);
    // References.
    if tag.key {
        h.refresh_golden = true;
        h.refresh_alt = true;
        h.refresh_entropy = d.flag();
        h.refresh_last = true;
    } else {
        h.refresh_golden = d.flag();
        h.refresh_alt = d.flag();
        if !h.refresh_golden {
            h.copy_to_golden = d.literal(2) as u8;
        }
        if !h.refresh_alt {
            h.copy_to_alt = d.literal(2) as u8;
        }
        h.sign_bias[2] = d.flag();
        h.sign_bias[3] = d.flag();
        h.refresh_entropy = d.flag();
        h.refresh_last = d.flag();
    }
    let saved = (!h.refresh_entropy).then(|| st.probs.clone());
    // Token probability updates.
    for (i, p) in st.probs.coeff.iter_mut().flatten().flatten().enumerate() {
        for (k, v) in p.iter_mut().enumerate() {
            if d.read(COEFF_UPDATE_PROBS[i * 11 + k]) {
                *v = d.literal(8) as u8;
            }
        }
    }
    h.skip_coded = d.flag();
    if h.skip_coded {
        h.prob_skip = d.literal(8) as u8;
    }
    if !tag.key {
        h.prob_intra = d.literal(8) as u8;
        h.prob_last = d.literal(8) as u8;
        h.prob_golden = d.literal(8) as u8;
        if d.flag() {
            for p in &mut st.probs.ymode {
                *p = d.literal(8) as u8;
            }
        }
        if d.flag() {
            for p in &mut st.probs.uv_mode {
                *p = d.literal(8) as u8;
            }
        }
        for (c, probs) in st.probs.mv.iter_mut().enumerate() {
            for (k, p) in probs.iter_mut().enumerate() {
                if d.read(MV_UPDATE_PROBS[c * 19 + k]) {
                    let x = d.literal(7) as u8;
                    *p = if x == 0 { 1 } else { x << 1 };
                }
            }
        }
    }
    Ok((h, saved))
}

/// The six dequantisation factors of one segment: [Y1, Y2, UV][DC, AC].
pub(crate) type Dequant = [[i32; 2]; 3];

pub(crate) fn dequant_factors(h: &Header, seg: &Segmentation) -> [Dequant; 4] {
    let mut out = [[[0; 2]; 3]; 4];
    for (s, f) in out.iter_mut().enumerate() {
        let mut q = h.q_index;
        if seg.enabled {
            q = if seg.absolute {
                seg.quant[s]
            } else {
                q + seg.quant[s]
            };
        }
        let q = q.clamp(0, 127);
        let dc = |delta: i32| DC_QLOOKUP[(q + delta).clamp(0, 127) as usize] as i32;
        let ac = |delta: i32| AC_QLOOKUP[(q + delta).clamp(0, 127) as usize] as i32;
        f[0] = [dc(h.y1_dc), ac(0)];
        f[1] = [dc(h.y2_dc) * 2, (ac(h.y2_ac) * 155 / 100).max(8)];
        f[2] = [dc(h.uv_dc).min(132), ac(h.uv_ac)];
    }
    out
}
