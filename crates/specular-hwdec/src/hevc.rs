//! One HEIC coded picture (an `hvcC` record and its length-prefixed NAL units) → HEVC
//! parameter buffers in VA-API's layout (`crate::params`; the D3D12 backend translates
//! them to DXVA's), with the checks that gate what reaches the driver.
//!
//! The parameter sets and slice headers are parsed in safe Rust by specular-heic's HEVC parser
//! (`specular_heic::h265`); the driver gets validated parameter structs and the slice bytes,
//! which the fixed-function engine parses. Only intra (IRAP) pictures of 8- or 10-bit 4:2:0
//! without range / SCC / multilayer extensions qualify; everything else is rejected here and
//! decoded in software.

use crate::params::{self as ffi, IqMatrixHevc, PicParamHevc, PictureHevc, SliceParamHevc};
use specular_heic::h265::hvcc::parse_hvcc;
use specular_heic::h265::nal::strip_emulation_prevention;
use specular_heic::h265::pps::PicParameterSet;
use specular_heic::h265::scaling_list::ScalingListData;
use specular_heic::h265::slice::{SliceSegmentHeader, SliceType};
use specular_heic::h265::sps::SeqParameterSet;
use specular_heic::h265::vui::VideoSignalType;

/// Limits of the decoder we hand tiles to (from the VA config attributes).
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_w: u32,
    pub max_h: u32,
    pub main10: bool,
}

/// One coded picture, ready for vaRenderPicture.
pub struct Picture<'a> {
    pub pic: PicParamHevc,
    pub iq: Option<IqMatrixHevc>,
    pub slices: Vec<(SliceParamHevc, &'a [u8])>,
    /// Coded size (the surface size).
    pub coded_w: u32,
    pub coded_h: u32,
    /// Conformance-window crop in luma samples: left, right, top, bottom.
    pub crop: [u32; 4],
    pub bit_depth: u8,
    /// The SPS VUI `video_signal_type`, when signalled.
    pub signal: Option<VideoSignalType>,
    /// The PPS `uniform_spacing_flag` (DXVA carries it; VA-API takes the tile sizes only).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub uniform_spacing: bool,
}

impl Picture<'_> {
    pub fn out_size(&self) -> (u32, u32) {
        (
            self.coded_w - self.crop[0] - self.crop[1],
            self.coded_h - self.crop[2] - self.crop[3],
        )
    }
}

/// Why a tile can't go to the hardware.
#[derive(Debug)]
pub enum Reject {
    /// Valid HEVC this hardware/profile can't take (4:4:4, 12-bit, too large...).
    Unsupported(String),
    /// Fails our sanity checks: never reaches the driver.
    Malformed(String),
}

impl std::fmt::Display for Reject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reject::Unsupported(s) => write!(f, "unsupported: {s}"),
            Reject::Malformed(s) => write!(f, "malformed: {s}"),
        }
    }
}

fn bad(s: impl Into<String>) -> Reject {
    Reject::Malformed(s.into())
}
fn unsup(s: impl Into<String>) -> Reject {
    Reject::Unsupported(s.into())
}

/// Splits ISO/IEC 14496-15 length-prefixed NAL units.
pub fn split_nals(data: &[u8], length_size: usize) -> Result<Vec<&[u8]>, Reject> {
    if !(1..=4).contains(&length_size) {
        return Err(bad("hvcC lengthSizeMinusOne"));
    }
    let mut v = Vec::new();
    let mut p = 0;
    while p < data.len() {
        if p + length_size > data.len() {
            return Err(bad("truncated NAL length"));
        }
        let n = data[p..p + length_size]
            .iter()
            .fold(0usize, |a, &b| (a << 8) | b as usize);
        p += length_size;
        if n < 2 || p + n > data.len() {
            return Err(bad("NAL length outside the item"));
        }
        v.push(&data[p..p + n]);
        p += n;
    }
    Ok(v)
}

pub fn nal_type(nal: &[u8]) -> u8 {
    (nal[0] >> 1) & 0x3f
}

/// Escaped position (after the 2-byte NAL header) of RBSP byte `rbsp_pos`, and the number of
/// emulation-prevention bytes before it.
pub(crate) fn escaped_offset(payload: &[u8], rbsp_pos: usize) -> (usize, usize) {
    let (mut zeros, mut r, mut emu) = (0u8, 0usize, 0usize);
    for (i, &b) in payload.iter().enumerate() {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            emu += 1;
            continue;
        }
        if r == rbsp_pos {
            return (i, emu);
        }
        r += 1;
        zeros = if b == 0 { zeros.saturating_add(1) } else { 0 };
    }
    (payload.len(), emu)
}

/// The parameter sets of a tile (from its hvcC, possibly overridden in-band).
pub struct ParamSets {
    pub sps: SeqParameterSet,
    pub pps: PicParameterSet,
}

pub fn parse_param_sets<'a>(nals: impl Iterator<Item = &'a [u8]>) -> Result<ParamSets, Reject> {
    let (mut sps, mut pps) = (None, None);
    for nal in nals {
        if nal.len() < 3 {
            continue;
        }
        let rbsp = strip_emulation_prevention(&nal[2..]);
        match nal_type(nal) {
            33 => {
                sps = Some(SeqParameterSet::parse(&rbsp).map_err(|e| bad(format!("SPS: {e:?}")))?)
            }
            34 => {
                let v1 = |id: u8| {
                    sps.as_ref().is_some_and(|s: &SeqParameterSet| {
                        s.sps_id == id && s.ptl.is_version1_profile()
                    })
                };
                pps = Some(
                    PicParameterSet::parse_with(&rbsp, v1)
                        .map_err(|e| bad(format!("PPS: {e:?}")))?,
                )
            }
            _ => {}
        }
    }
    Ok(ParamSets {
        sps: sps.ok_or_else(|| bad("no SPS"))?,
        pps: pps.ok_or_else(|| bad("no PPS"))?,
    })
}

/// The SPS/PPS checks: everything the fixed-function decoder's limits and VA's arrays need.
pub fn check_param_sets(ps: &ParamSets, lim: &Limits) -> Result<(), Reject> {
    let (s, p) = (&ps.sps, &ps.pps);
    if s.chroma_format_idc != 1 {
        // VA-API has HEVC 4:2:2 / 4:4:4 range-extension profiles, but this path decodes into
        // 4:2:0 surfaces only; such streams decode in software.
        return Err(unsup(match s.chroma_format_idc {
            0 => "HEVC monochrome (4:0:0): not decoded in hardware".to_string(),
            2 => "HEVC 4:2:2: not decoded in hardware".to_string(),
            3 => "HEVC 4:4:4: not decoded in hardware".to_string(),
            n => format!("HEVC chroma_format_idc {n}"),
        }));
    }
    if s.separate_colour_plane_flag {
        return Err(unsup("separate colour planes"));
    }
    if s.bit_depth_luma_minus8 != s.bit_depth_chroma_minus8 {
        return Err(unsup("different luma/chroma bit depths"));
    }
    match s.bit_depth_luma_minus8 {
        0 => {}
        2 if lim.main10 => {}
        d => return Err(unsup(format!("{}-bit", d + 8))),
    }
    let prof = s.ptl.general_profile_idc;
    let compat = s.ptl.general_profile_compatibility_flags;
    // Main (1), Main 10 (2), Main Still Picture (3), or RExt (4) when no range-extension tool is
    // on (x265/libheif label 10-bit 4:2:0 stills as RExt); the extension check below catches
    // streams that actually use RExt tools.
    if !matches!(prof, 1..=4) && compat & 0x7800_0000 == 0 {
        return Err(unsup(format!("profile_idc {prof}")));
    }
    if s.sps_range_extension.is_some()
        || s.sps_scc_extension.is_some()
        || s.sps_multilayer_extension.is_some()
        || p.pps_range_extension.is_some()
        || p.pps_scc_extension.is_some()
        || p.pps_multilayer_extension.is_some()
    {
        return Err(unsup("range/SCC/multilayer extensions"));
    }
    if s.nuh_layer_id != 0 {
        return Err(unsup("layered SPS"));
    }
    let (w, h) = (s.pic_width_in_luma_samples, s.pic_height_in_luma_samples);
    let min_cb = 3 + s.log2_min_luma_coding_block_size_minus3 as u32;
    let ctb = min_cb + s.log2_diff_max_min_luma_coding_block_size as u32;
    let min_tb = 2 + s.log2_min_luma_transform_block_size_minus2 as u32;
    let max_tb = min_tb + s.log2_diff_max_min_luma_transform_block_size as u32;
    if !(4..=6).contains(&ctb) || min_cb > ctb || min_tb >= min_cb || max_tb > 5.min(ctb) {
        return Err(bad(format!(
            "block sizes: CTB 2^{ctb}, CB 2^{min_cb}, TB 2^{min_tb}..2^{max_tb}"
        )));
    }
    if w == 0 || h == 0 || w % (1 << min_cb) != 0 || h % (1 << min_cb) != 0 {
        return Err(bad(format!("{w}×{h} is not a multiple of the minimum CB")));
    }
    // VCN's lower limit is 64×64 (radeon_vcn_dec); smaller pictures go to software.
    if w > lim.max_w || h > lim.max_h || w < 64 || h < 64 {
        return Err(unsup(format!(
            "{w}×{h} outside the decoder's 64×64..{}×{}",
            lim.max_w, lim.max_h
        )));
    }
    let cw = &s.conformance_window;
    let (l, r, t, b) = (
        cw.left_offset * 2,
        cw.right_offset * 2,
        cw.top_offset * 2,
        cw.bottom_offset * 2,
    );
    if l + r >= w || t + b >= h {
        return Err(bad("conformance window larger than the picture"));
    }
    if s.pcm_enabled_flag {
        let pcm = s.pcm.as_ref().ok_or_else(|| bad("PCM info missing"))?;
        if pcm.bit_depth_luma_minus1 > s.bit_depth_luma_minus8 + 7
            || pcm.bit_depth_chroma_minus1 > s.bit_depth_chroma_minus8 + 7
        {
            return Err(bad("PCM bit depth above the sample bit depth"));
        }
    }
    if p.sps_id != s.sps_id {
        return Err(bad("PPS refers to another SPS"));
    }
    if p.tiles_enabled_flag
        && (p.tiles.num_tile_columns_minus1 > 19 || p.tiles.num_tile_rows_minus1 > 21)
    {
        return Err(unsup("more HEVC tiles than VA-API carries"));
    }
    if !(-26 - 6 * s.bit_depth_luma_minus8 as i32..=25).contains(&p.init_qp_minus26) {
        return Err(bad("init_qp_minus26 out of range"));
    }
    Ok(())
}

fn bit(v: bool, n: u32) -> u32 {
    (v as u32) << n
}

fn tile_sizes(p: &PicParameterSet, pic_ctbs: u32, n_minus1: u32, explicit: &[u32]) -> Vec<u16> {
    let n = n_minus1 + 1;
    (0..n)
        .map(|i| {
            if p.tiles.uniform_spacing_flag {
                (((i + 1) * pic_ctbs) / n - (i * pic_ctbs) / n - 1) as u16
            } else if i < n_minus1 {
                explicit.get(i as usize).copied().unwrap_or(0) as u16
            } else {
                let used: u32 = explicit.iter().take(n_minus1 as usize).map(|x| x + 1).sum();
                pic_ctbs.saturating_sub(used).saturating_sub(1) as u16
            }
        })
        .collect()
}

/// Raster index of each position of the up-right diagonal scan (H.265 6.5.3).
pub(crate) fn diag_scan(n: usize) -> Vec<usize> {
    let mut v = Vec::with_capacity(n * n);
    let (mut x, mut y) = (0i32, 0i32);
    while v.len() < n * n {
        while y >= 0 {
            if (x as usize) < n && (y as usize) < n {
                v.push(y as usize * n + x as usize);
            }
            y -= 1;
            x += 1;
        }
        y = x;
        x = 0;
    }
    v
}

pub(crate) fn iq_matrix(ps: &ParamSets) -> IqMatrixHevc {
    let def = ScalingListData::all_default();
    let d = ps
        .pps
        .scaling_list_data
        .as_ref()
        .or(ps.sps.scaling_list_data.as_ref())
        .unwrap_or(&def);
    let mut m = IqMatrixHevc {
        scaling_list_4x4: [[0; 16]; 6],
        scaling_list_8x8: [[0; 64]; 6],
        scaling_list_16x16: [[0; 64]; 6],
        scaling_list_32x32: [[0; 64]; 2],
        scaling_list_dc_16x16: [0; 6],
        scaling_list_dc_32x32: [0; 2],
        reserved: [0; 4],
    };
    // The lists arrive in coded (up-right diagonal) order; VA-API wants them in raster order
    // (as FFmpeg's VA-API hwaccel sends them), the DC values apart, and only the two 32x32
    // lists of 4:2:0 (matrixId 0 and 3). Bit-exact against software on the JCT-VC SLIST_A..D
    // streams (tests/hevc_conformance.rs), where the coded order, a transposed scan or DC 16
    // each break most pictures.
    let c = |v: &Vec<u16>, i: usize| v.get(i).copied().unwrap_or(16).min(255) as u8;
    let (d4, d8) = (diag_scan(4), diag_scan(8));
    for mid in 0..6 {
        for (i, &r) in d4.iter().enumerate() {
            m.scaling_list_4x4[mid][r] = c(&d.lists[0][mid].coef, i);
        }
        for (i, &r) in d8.iter().enumerate() {
            m.scaling_list_8x8[mid][r] = c(&d.lists[1][mid].coef, i);
            m.scaling_list_16x16[mid][r] = c(&d.lists[2][mid].coef, i);
        }
        m.scaling_list_dc_16x16[mid] = d.lists[2][mid].dc_coef.min(255) as u8;
    }
    for (k, mid) in [0usize, 3].into_iter().enumerate() {
        for (i, &r) in d8.iter().enumerate() {
            m.scaling_list_32x32[k][r] = c(&d.lists[3][mid].coef, i);
        }
        m.scaling_list_dc_32x32[k] = d.lists[3][mid].dc_coef.min(255) as u8;
    }
    m
}

/// Builds the VA buffers for one picture (`CurrPic` is set when it is submitted).
pub fn build<'a>(ps: &ParamSets, slice_nals: &[&'a [u8]]) -> Result<Picture<'a>, Reject> {
    let (s, p) = (&ps.sps, &ps.pps);
    if slice_nals.is_empty() {
        return Err(bad("no slice NAL"));
    }
    if slice_nals.len() > 200 {
        return Err(unsup("more than 200 slices"));
    }
    let t0 = nal_type(slice_nals[0]);
    if !(16..=21).contains(&t0) || slice_nals.iter().any(|n| nal_type(n) != t0) {
        return Err(unsup(format!("NAL type {t0} (only IRAP pictures)")));
    }
    let idr = t0 == 19 || t0 == 20;
    let min_cb = 3 + s.log2_min_luma_coding_block_size_minus3 as u32;
    let ctb = min_cb + s.log2_diff_max_min_luma_coding_block_size as u32;
    let (w, h) = (s.pic_width_in_luma_samples, s.pic_height_in_luma_samples);
    let (wc, hc) = (w.div_ceil(1 << ctb), h.div_ceil(1 << ctb));

    let mut slices = Vec::with_capacity(slice_nals.len());
    let mut last_indep: Option<SliceSegmentHeader> = None;
    let mut next_addr = 0u32;
    let mut poc = 0i32;
    for (k, nal) in slice_nals.iter().enumerate() {
        if nal.len() < 3 || nal[0] & 0x80 != 0 || (nal[0] & 1) != 0 || nal[1] & 0xf8 != 0 {
            return Err(bad("NAL header (forbidden bit or nuh_layer_id)"));
        }
        let sh = slice_header(nal, t0, s, p)?;
        if (k == 0) != sh.first_slice_segment_in_pic_flag {
            return Err(bad("first_slice_segment_in_pic_flag"));
        }
        if sh.slice_pic_parameter_set_id != p.pps_id {
            return Err(unsup("slices use different PPSs"));
        }
        if k > 0 && sh.slice_segment_address <= next_addr.saturating_sub(1) && k > 0 {
            return Err(bad("slice addresses out of order"));
        }
        if sh.slice_segment_address >= wc * hc {
            return Err(bad("slice address outside the picture"));
        }
        next_addr = sh.slice_segment_address + 1;
        let off = sh
            .byte_offset_to_slice_data
            .ok_or_else(|| bad("slice header not fully parsed"))?;
        let (esc, emu) = escaped_offset(&nal[2..], off);
        if 2 + esc >= nal.len() {
            return Err(bad("no slice data"));
        }
        let hdr: &SliceSegmentHeader = if sh.dependent_slice_segment_flag {
            last_indep
                .as_ref()
                .ok_or_else(|| bad("dependent slice first"))?
        } else {
            &sh
        };
        if hdr.slice_type != Some(SliceType::I) {
            return Err(unsup("non-intra slice"));
        }
        if !idr {
            if hdr.short_term_ref_pic_set_sps_flag == Some(false) {
                // st_rps_bits would be needed (Intel); not worth it for still images.
                return Err(unsup("CRA/BLA with an inline short-term RPS"));
            }
            poc = hdr.slice_pic_order_cnt_lsb.unwrap_or(0) as i32;
        }
        let sp = slice_param(k + 1 == slice_nals.len(), nal, &sh, hdr, p, (esc, emu));
        slices.push((sp, *nal));
        if !sh.dependent_slice_segment_flag {
            last_indep = Some(sh);
        }
    }

    let pic = pic_param(ps, true, idr, true, poc);
    let cw = &s.conformance_window;
    Ok(Picture {
        pic,
        iq: s.scaling_list_enabled_flag.then(|| iq_matrix(ps)),
        slices,
        coded_w: w,
        coded_h: h,
        crop: [
            cw.left_offset * 2,
            cw.right_offset * 2,
            cw.top_offset * 2,
            cw.bottom_offset * 2,
        ],
        bit_depth: 8 + s.bit_depth_luma_minus8,
        signal: s.vui_parameters.as_ref().and_then(|v| v.video_signal_type),
        uniform_spacing: p.tiles_enabled_flag && p.tiles.uniform_spacing_flag,
    })
}

/// The picture parameters common to every picture of a sequence, for the current picture's
/// kind (`rap`: IRAP, `idr`, `intra`: all slices I) and `poc`. `CurrPic`, `ReferenceFrames`
/// and `st_rps_bits` are the caller's.
pub(crate) fn pic_param(
    ps: &ParamSets,
    rap: bool,
    idr: bool,
    intra: bool,
    poc: i32,
) -> PicParamHevc {
    let (s, p) = (&ps.sps, &ps.pps);
    let min_cb = 3 + s.log2_min_luma_coding_block_size_minus3 as u32;
    let ctb = min_cb + s.log2_diff_max_min_luma_coding_block_size as u32;
    let (w, h) = (s.pic_width_in_luma_samples, s.pic_height_in_luma_samples);
    let (wc, hc) = (w.div_ceil(1 << ctb), h.div_ceil(1 << ctb));
    let (cols, rows) = if p.tiles_enabled_flag {
        (
            tile_sizes(
                p,
                wc,
                p.tiles.num_tile_columns_minus1,
                &p.tiles.column_width_minus1,
            ),
            tile_sizes(
                p,
                hc,
                p.tiles.num_tile_rows_minus1,
                &p.tiles.row_height_minus1,
            ),
        )
    } else {
        (vec![], vec![])
    };
    let mut column_width_minus1 = [0u16; 19];
    let mut row_height_minus1 = [0u16; 21];
    for (d, v) in column_width_minus1.iter_mut().zip(&cols) {
        *d = *v;
    }
    for (d, v) in row_height_minus1.iter_mut().zip(&rows) {
        *d = *v;
    }
    let pcm = s.pcm.as_ref();
    let max_sub = s.max_sub_layers_minus1 as usize;
    let sl = s
        .sub_layer_ordering_info
        .get(max_sub)
        .or(s.sub_layer_ordering_info.first());
    let pic_fields = s.chroma_format_idc as u32
        | bit(s.separate_colour_plane_flag, 2)
        | bit(s.pcm_enabled_flag, 3)
        | bit(s.scaling_list_enabled_flag, 4)
        | bit(p.transform_skip_enabled_flag, 5)
        | bit(s.amp_enabled_flag, 6)
        | bit(s.strong_intra_smoothing_enabled_flag, 7)
        | bit(p.sign_data_hiding_enabled_flag, 8)
        | bit(p.constrained_intra_pred_flag, 9)
        | bit(p.cu_qp_delta_enabled_flag, 10)
        | bit(p.weighted_pred_flag, 11)
        | bit(p.weighted_bipred_flag, 12)
        | bit(p.transquant_bypass_enabled_flag, 13)
        | bit(p.tiles_enabled_flag, 14)
        | bit(p.entropy_coding_sync_enabled_flag, 15)
        | bit(p.pps_loop_filter_across_slices_enabled_flag, 16)
        | bit(p.loop_filter_across_tiles_enabled_flag, 17)
        | bit(pcm.is_some_and(|x| x.loop_filter_disabled_flag), 18)
        | bit(sl.is_none_or(|x| x.max_num_reorder_pics == 0), 19);
    let slice_parsing_fields = bit(p.lists_modification_present_flag, 0)
        | bit(s.long_term_ref_pics_present_flag, 1)
        | bit(s.sps_temporal_mvp_enabled_flag, 2)
        | bit(p.cabac_init_present_flag, 3)
        | bit(p.output_flag_present_flag, 4)
        | bit(p.dependent_slice_segments_enabled_flag, 5)
        | bit(p.pps_slice_chroma_qp_offsets_present_flag, 6)
        | bit(s.sample_adaptive_offset_enabled_flag, 7)
        | bit(p.deblocking.override_enabled_flag, 8)
        | bit(p.deblocking.disabled_flag, 9)
        | bit(p.slice_segment_header_extension_present_flag, 10)
        | bit(rap, 11)
        | bit(idr, 12)
        | bit(intra, 13);
    PicParamHevc {
        curr_pic: PictureHevc {
            picture_id: ffi::INVALID_ID,
            pic_order_cnt: poc,
            flags: 0,
            reserved: [0; 4],
        },
        reference_frames: [PictureHevc::INVALID; 15],
        pic_width_in_luma_samples: w as u16,
        pic_height_in_luma_samples: h as u16,
        pic_fields,
        sps_max_dec_pic_buffering_minus1: sl.map_or(0, |x| x.max_dec_pic_buffering_minus1.min(15))
            as u8,
        bit_depth_luma_minus8: s.bit_depth_luma_minus8,
        bit_depth_chroma_minus8: s.bit_depth_chroma_minus8,
        pcm_sample_bit_depth_luma_minus1: pcm.map_or(0, |x| x.bit_depth_luma_minus1),
        pcm_sample_bit_depth_chroma_minus1: pcm.map_or(0, |x| x.bit_depth_chroma_minus1),
        log2_min_luma_coding_block_size_minus3: s.log2_min_luma_coding_block_size_minus3,
        log2_diff_max_min_luma_coding_block_size: s.log2_diff_max_min_luma_coding_block_size,
        log2_min_transform_block_size_minus2: s.log2_min_luma_transform_block_size_minus2,
        log2_diff_max_min_transform_block_size: s.log2_diff_max_min_luma_transform_block_size,
        log2_min_pcm_luma_coding_block_size_minus3: pcm
            .map_or(0, |x| x.log2_min_pcm_luma_coding_block_size_minus3),
        log2_diff_max_min_pcm_luma_coding_block_size: pcm
            .map_or(0, |x| x.log2_diff_max_min_pcm_luma_coding_block_size),
        max_transform_hierarchy_depth_intra: s.max_transform_hierarchy_depth_intra,
        max_transform_hierarchy_depth_inter: s.max_transform_hierarchy_depth_inter,
        init_qp_minus26: p.init_qp_minus26 as i8,
        diff_cu_qp_delta_depth: p.diff_cu_qp_delta_depth.min(255) as u8,
        pps_cb_qp_offset: p.pps_cb_qp_offset,
        pps_cr_qp_offset: p.pps_cr_qp_offset,
        log2_parallel_merge_level_minus2: p.log2_parallel_merge_level_minus2.min(255) as u8,
        num_tile_columns_minus1: if p.tiles_enabled_flag {
            p.tiles.num_tile_columns_minus1 as u8
        } else {
            0
        },
        num_tile_rows_minus1: if p.tiles_enabled_flag {
            p.tiles.num_tile_rows_minus1 as u8
        } else {
            0
        },
        column_width_minus1,
        row_height_minus1,
        slice_parsing_fields,
        log2_max_pic_order_cnt_lsb_minus4: s.log2_max_pic_order_cnt_lsb_minus4,
        num_short_term_ref_pic_sets: s.num_short_term_ref_pic_sets.min(64) as u8,
        num_long_term_ref_pic_sps: s.num_long_term_ref_pics_sps.min(32) as u8,
        num_ref_idx_l0_default_active_minus1: p.num_ref_idx_l0_default_active_minus1,
        num_ref_idx_l1_default_active_minus1: p.num_ref_idx_l1_default_active_minus1,
        pps_beta_offset_div2: p.deblocking.beta_offset_div2,
        pps_tc_offset_div2: p.deblocking.tc_offset_div2,
        num_extra_slice_header_bits: p.num_extra_slice_header_bits,
        st_rps_bits: 0,
        reserved: [0; 8],
    }
}

/// The slice parameters without reference lists or weights: `sh` is the segment's own header,
/// `hdr` the independent segment's (the same one unless `sh` is dependent), `esc` / `emu` where
/// the slice data starts in the NAL payload and how many emulation-prevention bytes precede it.
pub(crate) fn slice_param(
    last: bool,
    nal: &[u8],
    sh: &SliceSegmentHeader,
    hdr: &SliceSegmentHeader,
    p: &PicParameterSet,
    (esc, emu): (usize, usize),
) -> SliceParamHevc {
    let dbk_disabled = hdr
        .deblocking
        .as_ref()
        .map_or(p.deblocking.disabled_flag, |d| d.disabled_flag);
    let (beta, tc) = hdr.deblocking.as_ref().map_or(
        (p.deblocking.beta_offset_div2, p.deblocking.tc_offset_div2),
        |d| (d.beta_offset_div2, d.tc_offset_div2),
    );
    let lf_across = hdr
        .slice_loop_filter_across_slices_enabled_flag
        .unwrap_or(p.pps_loop_filter_across_slices_enabled_flag);
    let flags = bit(last, 0)
        | bit(sh.dependent_slice_segment_flag, 1)
        | (slice_type_bits(hdr.slice_type) << 2)
        | bit(hdr.slice_sao_luma_flag, 6)
        | bit(hdr.slice_sao_chroma_flag, 7)
        | bit(dbk_disabled, 11)
        | bit(lf_across, 13);
    SliceParamHevc {
        slice_data_size: nal.len() as u32,
        slice_data_offset: 0,
        slice_data_flag: ffi::SLICE_DATA_FLAG_ALL,
        slice_data_byte_offset: (2 + esc) as u32,
        slice_segment_address: sh.slice_segment_address,
        ref_pic_list: [[0xff; 15]; 2],
        long_slice_flags: flags,
        collocated_ref_idx: 0xff,
        num_ref_idx_l0_active_minus1: 0,
        num_ref_idx_l1_active_minus1: 0,
        slice_qp_delta: hdr.slice_qp_delta.unwrap_or(0).clamp(-128, 127) as i8,
        slice_cb_qp_offset: hdr.slice_cb_qp_offset,
        slice_cr_qp_offset: hdr.slice_cr_qp_offset,
        slice_beta_offset_div2: beta,
        slice_tc_offset_div2: tc,
        luma_log2_weight_denom: 0,
        delta_chroma_log2_weight_denom: 0,
        delta_luma_weight_l0: [0; 15],
        luma_offset_l0: [0; 15],
        delta_chroma_weight_l0: [[0; 2]; 15],
        chroma_offset_l0: [[0; 2]; 15],
        delta_luma_weight_l1: [0; 15],
        luma_offset_l1: [0; 15],
        delta_chroma_weight_l1: [[0; 2]; 15],
        chroma_offset_l1: [[0; 2]; 15],
        five_minus_max_num_merge_cand: 0,
        num_entry_point_offsets: sh.entry_point_offsets.as_ref().map_or(0, |e| {
            e.entry_point_offset_minus1.len().min(u16::MAX as usize)
        }) as u16,
        entry_offset_to_subset_array: 0,
        slice_data_num_emu_prevn_bytes: emu.min(u16::MAX as usize) as u16,
        reserved: [0; 2],
    }
}

/// VA's `slice_type` field: B 0, P 1, I 2 (the H.265 `slice_type` values).
fn slice_type_bits(t: Option<SliceType>) -> u32 {
    match t {
        Some(SliceType::B) => 0,
        Some(SliceType::P) => 1,
        _ => 2,
    }
}

/// Parses a slice segment header from the first bytes of its NAL unit (the header is a few
/// hundred bytes; unescaping the whole slice would copy it), retrying on the whole NAL unit.
pub(crate) fn slice_header(
    nal: &[u8],
    nal_type: u8,
    s: &SeqParameterSet,
    p: &PicParameterSet,
) -> Result<SliceSegmentHeader, Reject> {
    const PREFIX: usize = 4096;
    let parse =
        |n: &[u8]| SliceSegmentHeader::parse(&strip_emulation_prevention(n), nal_type, s, p);
    if nal.len() > PREFIX + 2
        && let Ok(sh) = parse(&nal[2..PREFIX + 2])
        && sh
            .byte_offset_to_slice_data
            .is_some_and(|o| o + 8 < PREFIX - 16)
    {
        return Ok(sh);
    }
    parse(&nal[2..]).map_err(|e| bad(format!("slice header: {e:?}")))
}

/// Parses one coded picture: the `hvcC` record's parameter sets (or in-band ones), the slice
/// NAL units of `data` (length-prefixed), the checks against `lim`, and the VA buffers.
pub fn parse<'a>(hvcc: &[u8], data: &'a [u8], lim: &Limits) -> Result<Picture<'a>, Reject> {
    let rec = parse_hvcc(hvcc).map_err(|e| bad(format!("hvcC: {e:?}")))?;
    let nals = split_nals(data, rec.length_size)?;
    let mut slices = Vec::with_capacity(nals.len());
    let mut inband = Vec::new();
    for n in &nals {
        match nal_type(n) {
            0..=31 => slices.push(*n),
            32..=34 => inband.push(*n),
            ty @ 41..=47 => return Err(bad(format!("reserved NAL type {ty}"))),
            _ => {}
        }
    }
    let carried: Vec<Vec<u8>> = rec
        .nal_units
        .iter()
        .map(|n| {
            let mut v = Vec::with_capacity(2 + n.escaped.len());
            v.push(n.header.nal_unit_type << 1 | n.header.nuh_layer_id >> 5);
            v.push((n.header.nuh_layer_id & 0x1f) << 3 | (n.header.temporal_id + 1));
            v.extend_from_slice(&n.escaped);
            v
        })
        .collect();
    let ps = parse_param_sets(
        carried
            .iter()
            .map(Vec::as_slice)
            .chain(inband.iter().copied()),
    )?;
    check_param_sets(&ps, lim)?;
    build(&ps, &slices)
}
