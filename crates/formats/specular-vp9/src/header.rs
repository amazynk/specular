//! The uncompressed header (section 6.2), the compressed header (section 6.3) and superframes
//! (annex B).

use crate::bits::{BitReader, BoolDecoder, check};
use crate::probs::Probs;
use crate::tables::*;
use crate::{ColorSpace, Error, Result, malformed};

pub(crate) const KEY_FRAME: u32 = 0;
pub(crate) const SWITCHABLE: u8 = 4;
pub(crate) const MAX_DIM: u32 = 16384;

/// Splits a container block into its frames (annex B); a block without a valid superframe
/// index is one frame.
pub(crate) fn split_superframe(data: &[u8]) -> Vec<&[u8]> {
    let n = data.len();
    if n == 0 {
        return Vec::new();
    }
    let last = data[n - 1];
    if last & 0xe0 == 0xc0 {
        let sz_bytes = ((last >> 3) & 3) as usize + 1;
        let frames = (last & 7) as usize + 1;
        let index = 2 + frames * sz_bytes;
        if n >= index && data[n - index] == last {
            let mut out = Vec::with_capacity(frames);
            let mut p = n - index + 1;
            let mut off = 0usize;
            for _ in 0..frames {
                let mut sz = 0usize;
                for j in 0..sz_bytes {
                    sz |= (data[p + j] as usize) << (8 * j);
                }
                p += sz_bytes;
                // A frame may not run into the index; a zero size is skipped (libvpx does the
                // same for the padding frames some muxers write).
                let end = off.saturating_add(sz).min(n - index);
                if sz > 0 && off < end {
                    out.push(&data[off..end]);
                }
                off = off.saturating_add(sz);
            }
            return out;
        }
    }
    vec![data]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColorConfig {
    pub bit_depth: u8,
    pub color_space: ColorSpace,
    pub full_range: bool,
    pub ss_x: bool,
    pub ss_y: bool,
}

impl Default for ColorConfig {
    fn default() -> Self {
        ColorConfig {
            bit_depth: 8,
            color_space: ColorSpace::Bt601,
            full_range: false,
            ss_x: true,
            ss_y: true,
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Segmentation {
    pub enabled: bool,
    pub update_map: bool,
    pub temporal_update: bool,
    pub abs_delta: bool,
    pub tree_probs: [u8; 7],
    pub pred_probs: [u8; 3],
    pub feature_enabled: [[bool; 4]; 8],
    pub feature_data: [[i32; 4]; 8],
}

pub(crate) const SEG_LVL_ALT_Q: usize = 0;
pub(crate) const SEG_LVL_ALT_L: usize = 1;
pub(crate) const SEG_LVL_REF_FRAME: usize = 2;
pub(crate) const SEG_LVL_SKIP: usize = 3;

impl Segmentation {
    #[inline]
    pub(crate) fn active(&self, seg: usize, feature: usize) -> bool {
        self.enabled && self.feature_enabled[seg][feature]
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct LoopFilter {
    pub level: u8,
    pub sharpness: u8,
    pub delta_enabled: bool,
    pub ref_deltas: [i32; 4],
    pub mode_deltas: [i32; 2],
}

/// Header values that carry over from frame to frame.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Persist {
    pub color: ColorConfig,
    pub lf: LoopFilter,
    pub seg: Segmentation,
    /// frame_type of the previous decoded frame (LastFrameType), `None` before the first.
    pub last_frame_type: Option<u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct FrameHeader {
    #[allow(dead_code)]
    // Kept with the header for debugging; the colour config carries its effect.
    pub profile: u8,
    pub show_existing: Option<usize>,
    pub key: bool,
    pub show_frame: bool,
    pub error_res: bool,
    pub intra_only: bool,
    /// FrameIsIntra.
    pub intra: bool,
    pub reset_ctx: u8,
    pub color: ColorConfig,
    pub refresh_flags: u8,
    pub width: u32,
    pub height: u32,
    pub render_w: u32,
    pub render_h: u32,
    pub ref_idx: [usize; 3],
    /// ref_frame_sign_bias indexed by reference frame (INTRA, LAST, GOLDEN, ALTREF).
    pub sign_bias: [bool; 4],
    pub allow_hp: bool,
    pub interp_filter: u8,
    pub refresh_ctx: bool,
    pub parallel: bool,
    pub ctx_idx: usize,
    /// setup_past_independence was invoked.
    pub past_independence: bool,
    /// The frame contexts reset to defaults: all four, or the one given.
    pub reset_all: bool,
    pub reset_one: Option<usize>,
    pub lf: LoopFilter,
    pub base_q: i32,
    pub dq_y_dc: i32,
    pub dq_uv_dc: i32,
    pub dq_uv_ac: i32,
    pub lossless: bool,
    pub seg: Segmentation,
    pub tile_cols_log2: u32,
    pub tile_rows_log2: u32,
    pub header_size: usize,
    /// Size of the uncompressed header in bytes (after trailing_bits).
    pub uncomp_size: usize,
    pub last_frame_type: Option<u32>,
    pub mi_cols: usize,
    pub mi_rows: usize,
    pub sb64_cols: usize,
    pub sb64_rows: usize,
}

fn frame_sync(r: &mut BitReader) -> Result<()> {
    let s = r.f(24)?;
    check(s == 0x49_83_42, "bad VP9 frame sync code")
}

fn color_config(r: &mut BitReader, profile: u8) -> Result<ColorConfig> {
    let bit_depth = if profile >= 2 {
        if r.flag()? { 12 } else { 10 }
    } else {
        8
    };
    let cs = r.f(3)?;
    let color_space = ColorSpace::from_code(cs);
    let (full_range, ss_x, ss_y);
    if cs != 7 {
        full_range = r.flag()?;
        if profile == 1 || profile == 3 {
            ss_x = r.flag()?;
            ss_y = r.flag()?;
            check(r.bit()? == 0, "reserved bit set in the colour config")?;
            if ss_x && ss_y {
                return Err(malformed("4:2:0 signalled in profile 1 or 3"));
            }
        } else {
            ss_x = true;
            ss_y = true;
        }
    } else {
        full_range = true;
        if profile == 1 || profile == 3 {
            ss_x = false;
            ss_y = false;
            check(r.bit()? == 0, "reserved bit set in the colour config")?;
        } else {
            return Err(malformed("sRGB colour space in profile 0 or 2"));
        }
    }
    Ok(ColorConfig {
        bit_depth,
        color_space,
        full_range,
        ss_x,
        ss_y,
    })
}

fn frame_size(r: &mut BitReader) -> Result<(u32, u32)> {
    let w = r.f(16)? + 1;
    let h = r.f(16)? + 1;
    Ok((w, h))
}

fn render_size(r: &mut BitReader, w: u32, h: u32) -> Result<(u32, u32)> {
    if r.flag()? {
        Ok((r.f(16)? + 1, r.f(16)? + 1))
    } else {
        Ok((w, h))
    }
}

fn read_delta_q(r: &mut BitReader) -> Result<i32> {
    if r.flag()? { r.s(4) } else { Ok(0) }
}

fn read_prob(r: &mut BitReader) -> Result<u8> {
    if r.flag()? {
        Ok(r.f(8)? as u8)
    } else {
        Ok(255)
    }
}

/// The parts of the uncompressed header up to frame_type, for probing.
pub(crate) struct Start {
    pub profile: u8,
    pub show_existing: Option<usize>,
    pub key: bool,
    pub show_frame: bool,
    pub error_res: bool,
}

fn read_start(r: &mut BitReader) -> Result<Start> {
    check(r.f(2)? == 2, "bad VP9 frame marker")?;
    let lo = r.bit()?;
    let hi = r.bit()?;
    let profile = ((hi << 1) + lo) as u8;
    if profile == 3 {
        check(r.bit()? == 0, "reserved bit set after the profile")?;
    }
    if r.flag()? {
        let idx = r.f(3)? as usize;
        return Ok(Start {
            profile,
            show_existing: Some(idx),
            key: false,
            show_frame: true,
            error_res: false,
        });
    }
    let key = r.f(1)? == KEY_FRAME;
    let show_frame = r.flag()?;
    let error_res = r.flag()?;
    Ok(Start {
        profile,
        show_existing: None,
        key,
        show_frame,
        error_res,
    })
}

/// Header facts of a keyframe or intra-only frame: profile, colour config, frame and render
/// size. `None` for other frames.
pub(crate) fn probe_intra(data: &[u8]) -> Option<(u8, ColorConfig, u32, u32, u32, u32)> {
    let mut r = BitReader::new(data);
    let s = read_start(&mut r).ok()?;
    if s.show_existing.is_some() {
        return None;
    }
    let color;
    if s.key {
        frame_sync(&mut r).ok()?;
        color = color_config(&mut r, s.profile).ok()?;
    } else {
        let intra_only = if s.show_frame { false } else { r.flag().ok()? };
        if !intra_only {
            return None;
        }
        if !s.error_res {
            r.f(2).ok()?;
        }
        frame_sync(&mut r).ok()?;
        color = if s.profile > 0 {
            color_config(&mut r, s.profile).ok()?
        } else {
            ColorConfig::default()
        };
        r.f(8).ok()?;
    }
    let (w, h) = frame_size(&mut r).ok()?;
    let (rw, rh) = render_size(&mut r, w, h).ok()?;
    Some((s.profile, color, w, h, rw, rh))
}

/// Whether the frame is a keyframe.
pub(crate) fn frame_is_key(data: &[u8]) -> bool {
    let mut r = BitReader::new(data);
    matches!(
        read_start(&mut r),
        Ok(Start {
            key: true,
            show_existing: None,
            ..
        })
    )
}

/// Parses the uncompressed header. `persist` holds the values carried over from earlier frames
/// and is updated; `ref_sizes` are the sizes of the eight reference slots.
pub(crate) fn parse_uncompressed(
    data: &[u8],
    persist: &mut Persist,
    ref_sizes: &[Option<(u32, u32)>; 8],
) -> Result<FrameHeader> {
    let mut r = BitReader::new(data);
    let s = read_start(&mut r)?;
    if s.profile > 3 {
        return Err(Error::Unsupported(format!("VP9 profile {}", s.profile)));
    }
    let mut h = FrameHeader {
        profile: s.profile,
        show_existing: s.show_existing,
        key: s.key,
        show_frame: s.show_frame,
        error_res: s.error_res,
        intra_only: false,
        intra: false,
        reset_ctx: 0,
        color: persist.color,
        refresh_flags: 0,
        width: 0,
        height: 0,
        render_w: 0,
        render_h: 0,
        ref_idx: [0; 3],
        sign_bias: [false; 4],
        allow_hp: false,
        interp_filter: 0,
        refresh_ctx: false,
        parallel: true,
        ctx_idx: 0,
        past_independence: false,
        reset_all: false,
        reset_one: None,
        lf: persist.lf,
        base_q: 0,
        dq_y_dc: 0,
        dq_uv_dc: 0,
        dq_uv_ac: 0,
        lossless: false,
        seg: persist.seg,
        tile_cols_log2: 0,
        tile_rows_log2: 0,
        header_size: 0,
        uncomp_size: 0,
        last_frame_type: persist.last_frame_type,
        mi_cols: 0,
        mi_rows: 0,
        sb64_cols: 0,
        sb64_rows: 0,
    };
    if s.show_existing.is_some() {
        h.lf.level = 0;
        return Ok(h);
    }
    let frame_type = if s.key { 0 } else { 1 };
    if s.key {
        frame_sync(&mut r)?;
        h.color = color_config(&mut r, s.profile)?;
        let (w, hh) = frame_size(&mut r)?;
        h.width = w;
        h.height = hh;
        let (rw, rh) = render_size(&mut r, w, hh)?;
        h.render_w = rw;
        h.render_h = rh;
        h.refresh_flags = 0xff;
        h.intra = true;
    } else {
        h.intra_only = if s.show_frame { false } else { r.flag()? };
        h.intra = h.intra_only;
        h.reset_ctx = if s.error_res { 0 } else { r.f(2)? as u8 };
        if h.intra_only {
            frame_sync(&mut r)?;
            h.color = if s.profile > 0 {
                color_config(&mut r, s.profile)?
            } else {
                ColorConfig::default()
            };
            h.refresh_flags = r.f(8)? as u8;
            let (w, hh) = frame_size(&mut r)?;
            h.width = w;
            h.height = hh;
            let (rw, rh) = render_size(&mut r, w, hh)?;
            h.render_w = rw;
            h.render_h = rh;
        } else {
            h.refresh_flags = r.f(8)? as u8;
            for i in 0..3 {
                h.ref_idx[i] = r.f(3)? as usize;
                h.sign_bias[1 + i] = r.flag()?;
            }
            let mut found = None;
            for i in 0..3 {
                if r.flag()? {
                    found = Some(i);
                    break;
                }
            }
            let (w, hh) = match found {
                Some(i) => ref_sizes[h.ref_idx[i]]
                    .ok_or_else(|| malformed("frame size taken from an empty reference slot"))?,
                None => frame_size(&mut r)?,
            };
            h.width = w;
            h.height = hh;
            let (rw, rh) = render_size(&mut r, w, hh)?;
            h.render_w = rw;
            h.render_h = rh;
            h.allow_hp = r.flag()?;
            h.interp_filter = if r.flag()? {
                SWITCHABLE
            } else {
                LITERAL_TO_TYPE[r.f(2)? as usize]
            };
        }
    }
    if h.width > MAX_DIM || h.height > MAX_DIM {
        return Err(Error::Unsupported(format!(
            "VP9 frames of {}x{} (the limit is {MAX_DIM}x{MAX_DIM})",
            h.width, h.height
        )));
    }
    if !h.error_res {
        h.refresh_ctx = r.flag()?;
        h.parallel = r.flag()?;
    } else {
        h.refresh_ctx = false;
        h.parallel = true;
    }
    h.ctx_idx = r.f(2)? as usize;
    if h.intra || h.error_res {
        // setup_past_independence: segmentation features, loop filter deltas and sign biases
        // (the probability reset is done by the decoder).
        h.past_independence = true;
        h.seg.feature_enabled = [[false; 4]; 8];
        h.seg.feature_data = [[0; 4]; 8];
        h.seg.abs_delta = false;
        h.lf.delta_enabled = true;
        h.lf.ref_deltas = [1, 0, -1, -1];
        h.lf.mode_deltas = [0, 0];
        h.sign_bias = [false; 4];
        if h.key || h.error_res || h.reset_ctx == 3 {
            h.reset_all = true;
        } else if h.reset_ctx == 2 {
            h.reset_one = Some(h.ctx_idx);
        }
        h.ctx_idx = 0;
    }
    // loop_filter_params
    h.lf.level = r.f(6)? as u8;
    h.lf.sharpness = r.f(3)? as u8;
    h.lf.delta_enabled = r.flag()?;
    if h.lf.delta_enabled && r.flag()? {
        for i in 0..4 {
            if r.flag()? {
                h.lf.ref_deltas[i] = r.s(6)?;
            }
        }
        for i in 0..2 {
            if r.flag()? {
                h.lf.mode_deltas[i] = r.s(6)?;
            }
        }
    }
    // quantization_params
    h.base_q = r.f(8)? as i32;
    h.dq_y_dc = read_delta_q(&mut r)?;
    h.dq_uv_dc = read_delta_q(&mut r)?;
    h.dq_uv_ac = read_delta_q(&mut r)?;
    h.lossless = h.base_q == 0 && h.dq_y_dc == 0 && h.dq_uv_dc == 0 && h.dq_uv_ac == 0;
    // segmentation_params
    h.seg.update_map = false;
    h.seg.enabled = r.flag()?;
    if h.seg.enabled {
        h.seg.update_map = r.flag()?;
        if h.seg.update_map {
            for i in 0..7 {
                h.seg.tree_probs[i] = read_prob(&mut r)?;
            }
            h.seg.temporal_update = r.flag()?;
            for i in 0..3 {
                h.seg.pred_probs[i] = if h.seg.temporal_update {
                    read_prob(&mut r)?
                } else {
                    255
                };
            }
        }
        if r.flag()? {
            h.seg.abs_delta = r.flag()?;
            for i in 0..8 {
                for j in 0..4 {
                    let mut v = 0i32;
                    let en = r.flag()?;
                    h.seg.feature_enabled[i][j] = en;
                    if en {
                        v = r.f(SEGMENTATION_FEATURE_BITS[j] as u32)? as i32;
                        if SEGMENTATION_FEATURE_SIGNED[j] == 1 && r.flag()? {
                            v = -v;
                        }
                    }
                    h.seg.feature_data[i][j] = v;
                }
            }
        }
    }
    // tile_info
    h.mi_cols = (h.width as usize).div_ceil(8);
    h.mi_rows = (h.height as usize).div_ceil(8);
    h.sb64_cols = h.mi_cols.div_ceil(8);
    h.sb64_rows = h.mi_rows.div_ceil(8);
    let mut min_log2 = 0;
    while (64usize << min_log2) < h.sb64_cols {
        min_log2 += 1;
    }
    let mut max_log2 = 1;
    while (h.sb64_cols >> max_log2) >= 4 {
        max_log2 += 1;
    }
    max_log2 -= 1;
    h.tile_cols_log2 = min_log2;
    while h.tile_cols_log2 < max_log2 {
        if r.flag()? {
            h.tile_cols_log2 += 1;
        } else {
            break;
        }
    }
    h.tile_rows_log2 = r.bit()?;
    if h.tile_rows_log2 == 1 {
        h.tile_rows_log2 += r.bit()?;
    }
    h.header_size = r.f(16)? as usize;
    // trailing_bits
    h.uncomp_size = r.position().div_ceil(8);
    persist.color = h.color;
    persist.lf = h.lf;
    persist.seg = h.seg;
    persist.last_frame_type = Some(frame_type);
    Ok(h)
}

pub(crate) const ONLY_4X4: u8 = 0;
pub(crate) const ALLOW_32X32: u8 = 3;
pub(crate) const TX_MODE_SELECT: u8 = 4;

pub(crate) const SINGLE_REFERENCE: u8 = 0;
pub(crate) const COMPOUND_REFERENCE: u8 = 1;
pub(crate) const REFERENCE_MODE_SELECT: u8 = 2;

/// What the compressed header says besides the probability updates.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Compressed {
    pub tx_mode: u8,
    pub reference_mode: u8,
    pub comp_fixed_ref: i8,
    pub comp_var_ref: [i8; 2],
}

fn inv_recenter_nonneg(v: i32, m: i32) -> i32 {
    if v > 2 * m {
        v
    } else if v & 1 != 0 {
        m - ((v + 1) >> 1)
    } else {
        m + (v >> 1)
    }
}

fn decode_term_subexp(bd: &mut BoolDecoder) -> i32 {
    if bd.literal(1) == 0 {
        return bd.literal(4) as i32;
    }
    if bd.literal(1) == 0 {
        return bd.literal(4) as i32 + 16;
    }
    if bd.literal(1) == 0 {
        return bd.literal(5) as i32 + 32;
    }
    let v = bd.literal(7) as i32;
    if v < 65 {
        return v + 64;
    }
    let bit = bd.literal(1) as i32;
    (v << 1) - 1 + bit
}

fn inv_remap_prob(delta: i32, prob: u8) -> u8 {
    let v = INV_MAP_TABLE[delta.clamp(0, 254) as usize] as i32;
    let m = prob as i32 - 1;
    let r = if (m << 1) <= 255 {
        1 + inv_recenter_nonneg(v, m)
    } else {
        255 - inv_recenter_nonneg(v, 255 - 1 - m)
    };
    r.clamp(1, 255) as u8
}

#[inline]
fn diff_update(bd: &mut BoolDecoder, p: &mut u8) {
    if bd.read(252) {
        let d = decode_term_subexp(bd);
        *p = inv_remap_prob(d, *p);
    }
}

fn update_mv_prob(bd: &mut BoolDecoder, p: &mut u8) {
    if bd.read(252) {
        *p = ((bd.literal(7) << 1) | 1) as u8;
    }
}

/// Parses the compressed header, applying its probability updates to `fc`.
pub(crate) fn parse_compressed(data: &[u8], h: &FrameHeader, fc: &mut Probs) -> Result<Compressed> {
    let mut bd = BoolDecoder::new(data)?;
    let mut c = Compressed::default();
    // read_tx_mode
    if h.lossless {
        c.tx_mode = ONLY_4X4;
    } else {
        c.tx_mode = bd.literal(2) as u8;
        if c.tx_mode == ALLOW_32X32 {
            c.tx_mode += bd.literal(1) as u8;
        }
    }
    if c.tx_mode == TX_MODE_SELECT {
        for i in 0..2 {
            diff_update(&mut bd, &mut fc.tx[1][i][0]);
        }
        for i in 0..2 {
            for j in 0..2 {
                diff_update(&mut bd, &mut fc.tx[2][i][j]);
            }
        }
        for i in 0..2 {
            for j in 0..3 {
                diff_update(&mut bd, &mut fc.tx[3][i][j]);
            }
        }
    }
    // read_coef_probs
    let max_tx = TX_MODE_TO_BIGGEST_TX_SIZE[c.tx_mode as usize] as usize;
    for tx in 0..=max_tx {
        if bd.literal(1) == 1 {
            for i in 0..2 {
                for j in 0..2 {
                    for k in 0..6 {
                        let max_l = if k == 0 { 3 } else { 6 };
                        for l in 0..max_l {
                            for m in 0..3 {
                                diff_update(&mut bd, &mut fc.coef[tx][i][j][k][l][m]);
                            }
                        }
                    }
                }
            }
        }
    }
    for i in 0..3 {
        diff_update(&mut bd, &mut fc.skip[i]);
    }
    if !h.intra {
        for i in 0..7 {
            for j in 0..3 {
                diff_update(&mut bd, &mut fc.inter_mode[i][j]);
            }
        }
        if h.interp_filter == SWITCHABLE {
            for j in 0..4 {
                for i in 0..2 {
                    diff_update(&mut bd, &mut fc.interp_filter[j][i]);
                }
            }
        }
        for i in 0..4 {
            diff_update(&mut bd, &mut fc.is_inter[i]);
        }
        // frame_reference_mode
        let mut compound_allowed = false;
        for i in 1..3 {
            if h.sign_bias[i + 1] != h.sign_bias[1] {
                compound_allowed = true;
            }
        }
        c.reference_mode = SINGLE_REFERENCE;
        if compound_allowed && bd.literal(1) == 1 {
            c.reference_mode = if bd.literal(1) == 0 {
                COMPOUND_REFERENCE
            } else {
                REFERENCE_MODE_SELECT
            };
            // setup_compound_reference_mode
            if h.sign_bias[1] == h.sign_bias[2] {
                c.comp_fixed_ref = 3;
                c.comp_var_ref = [1, 2];
            } else if h.sign_bias[1] == h.sign_bias[3] {
                c.comp_fixed_ref = 2;
                c.comp_var_ref = [1, 3];
            } else {
                c.comp_fixed_ref = 1;
                c.comp_var_ref = [2, 3];
            }
        }
        // frame_reference_mode_probs
        if c.reference_mode == REFERENCE_MODE_SELECT {
            for i in 0..5 {
                diff_update(&mut bd, &mut fc.comp_mode[i]);
            }
        }
        if c.reference_mode != COMPOUND_REFERENCE {
            for i in 0..5 {
                diff_update(&mut bd, &mut fc.single_ref[i][0]);
                diff_update(&mut bd, &mut fc.single_ref[i][1]);
            }
        }
        if c.reference_mode != SINGLE_REFERENCE {
            for i in 0..5 {
                diff_update(&mut bd, &mut fc.comp_ref[i]);
            }
        }
        for i in 0..4 {
            for j in 0..9 {
                diff_update(&mut bd, &mut fc.y_mode[i][j]);
            }
        }
        for i in 0..16 {
            for j in 0..3 {
                diff_update(&mut bd, &mut fc.partition[i][j]);
            }
        }
        // mv_probs
        for j in 0..3 {
            update_mv_prob(&mut bd, &mut fc.mv_joint[j]);
        }
        for i in 0..2 {
            update_mv_prob(&mut bd, &mut fc.mv_sign[i]);
            for j in 0..10 {
                update_mv_prob(&mut bd, &mut fc.mv_class[i][j]);
            }
            update_mv_prob(&mut bd, &mut fc.mv_class0_bit[i]);
            for j in 0..10 {
                update_mv_prob(&mut bd, &mut fc.mv_bits[i][j]);
            }
        }
        for i in 0..2 {
            for j in 0..2 {
                for k in 0..3 {
                    update_mv_prob(&mut bd, &mut fc.mv_class0_fr[i][j][k]);
                }
            }
            for k in 0..3 {
                update_mv_prob(&mut bd, &mut fc.mv_fr[i][k]);
            }
        }
        if h.allow_hp {
            for i in 0..2 {
                update_mv_prob(&mut bd, &mut fc.mv_class0_hp[i]);
                update_mv_prob(&mut bd, &mut fc.mv_hp[i]);
            }
        }
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superframe_index() {
        // Two frames of 3 and 2 bytes, 1-byte sizes: marker 0b110_00_001.
        let m = 0xc1u8;
        let data = [1, 2, 3, 4, 5, m, 3, 2, m];
        let f = split_superframe(&data);
        assert_eq!(f, vec![&[1u8, 2, 3][..], &[4u8, 5][..]]);
        // No index: one frame.
        assert_eq!(split_superframe(&[1, 2, 3]), vec![&[1u8, 2, 3][..]]);
    }

    #[test]
    fn remap() {
        // inv_map_table[0] = 7: an update of +7 around the middle.
        assert_eq!(inv_remap_prob(0, 128), 124);
        assert!((1..=255).contains(&inv_remap_prob(254, 1)));
    }
}
