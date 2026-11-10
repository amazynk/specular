//! Parsing of `seq_parameter_set_rbsp()` (clause 7.3.2.2).

use super::sps::Sps;
use super::{ptl, rps, scaling, vui};
use crate::bits::BitReader;
use crate::error::{Error, Result};

/// Parses `seq_parameter_set_rbsp()`.
pub fn parse(data: &[u8]) -> Result<Sps> {
    let r = &mut BitReader::new(data);
    r.skip(4)?; // sps_video_parameter_set_id
    let max_sub_minus1 = r.u(3)?;
    r.skip(1)?; // sps_temporal_id_nesting_flag
    let ptl = ptl::parse(r, true, max_sub_minus1)?;
    let id = r.ue()?;
    let chroma_format_idc = r.ue()?;
    if chroma_format_idc > 3 {
        return Err(Error::InvalidData("chroma_format_idc > 3"));
    }
    let separate_colour_plane = chroma_format_idc == 3 && r.u1()? != 0;
    let chroma_array_type = if separate_colour_plane {
        0
    } else {
        chroma_format_idc as u8
    };
    let (sub_w, sub_h) = match chroma_array_type {
        1 => (2usize, 2usize),
        2 => (2, 1),
        3 => (1, 1),
        _ => (0, 0),
    };
    let width = r.ue()? as usize;
    let height = r.ue()? as usize;
    if width == 0 || height == 0 || width > 65535 || height > 65535 {
        return Err(Error::InvalidData("implausible picture dimensions"));
    }
    let mut crop = [0usize; 4];
    if r.u1()? != 0 {
        let (cw, ch) = if sub_w == 0 { (1, 1) } else { (sub_w, sub_h) };
        crop[0] = r.ue()? as usize * cw;
        crop[1] = r.ue()? as usize * cw;
        crop[2] = r.ue()? as usize * ch;
        crop[3] = r.ue()? as usize * ch;
        // The window must leave a picture (7.4.3.2.1); one that does not
        // is ignored rather than failing the stream.
        if crop[0] + crop[1] >= width || crop[2] + crop[3] >= height {
            crop = [0; 4];
        }
    }
    let bit_depth_y = r.ue()?.saturating_add(8).min(255) as u8;
    let bit_depth_c = r.ue()?.saturating_add(8).min(255) as u8;
    if !(8..=16).contains(&bit_depth_y) || !(8..=16).contains(&bit_depth_c) {
        return Err(Error::InvalidData("bit depth out of range"));
    }
    let log2_max_poc_lsb = r.ue()?.saturating_add(4);
    if log2_max_poc_lsb > 16 {
        return Err(Error::InvalidData("log2_max_pic_order_cnt_lsb too large"));
    }
    let sub_layer_ordering = r.u1()? != 0;
    let first = if sub_layer_ordering {
        0
    } else {
        max_sub_minus1
    };
    let (mut max_dec, mut max_reorder, mut max_latency) = (1u32, 0u32, None);
    for _ in first..=max_sub_minus1 {
        max_dec = r.ue()?.saturating_add(1);
        max_reorder = r.ue()?;
        let lat = r.ue()?;
        max_latency = (lat != 0).then(|| max_reorder.saturating_add(lat - 1));
    }
    // Only video decoding reads these; a still decodes whatever they say,
    // so out-of-range values are clamped rather than refused.
    let max_dec = max_dec.clamp(1, 16);
    let max_reorder = max_reorder.min(max_dec - 1);
    let log2_min_cb = r.ue()? as usize + 3;
    let log2_ctb = log2_min_cb + r.ue()? as usize;
    let log2_min_tb = r.ue()? as usize + 2;
    let log2_max_tb = log2_min_tb + r.ue()? as usize;
    if log2_ctb > 6 || log2_min_cb < 3 || log2_max_tb > 5 || log2_min_tb >= log2_min_cb {
        return Err(Error::InvalidData("implausible block size configuration"));
    }
    // Both dimensions are multiples of MinCbSizeY (7.4.3.2.1).
    if (width | height) & ((1 << log2_min_cb) - 1) != 0 {
        return Err(Error::InvalidData(
            "picture size not a multiple of the minimum coding block",
        ));
    }
    let max_tr_depth_inter = r.ue()?;
    let max_tr_depth_intra = r.ue()?;
    let scaling_list_enabled = r.u1()? != 0;
    let mut sps_scaling = None;
    if scaling_list_enabled && r.u1()? != 0 {
        let mut d = scaling::ScalingListData::default();
        scaling::parse(r, &mut d)?;
        sps_scaling = Some(d);
    }
    let amp = r.u1()? != 0;
    let sao_enabled = r.u1()? != 0;
    let pcm_enabled = r.u1()? != 0;
    let (mut pbd_y, mut pbd_c, mut l2minp, mut l2maxp, mut pcm_lf) =
        (8u8, 8u8, 3usize, 3usize, false);
    if pcm_enabled {
        pbd_y = r.u(4)? as u8 + 1;
        pbd_c = r.u(4)? as u8 + 1;
        l2minp = r.ue()? as usize + 3;
        l2maxp = l2minp + r.ue()? as usize;
        pcm_lf = r.u1()? != 0;
    }
    let num_st_rps = r.ue()?;
    if num_st_rps > 64 {
        return Err(Error::InvalidData("num_short_term_ref_pic_sets > 64"));
    }
    let st_rps = rps::parse_all(r, num_st_rps)?;
    let long_term_present = r.u1()? != 0;
    let mut num_lt_sps = 0;
    let mut lt_sps = alloc::vec::Vec::new();
    if long_term_present {
        num_lt_sps = r.ue()?;
        if num_lt_sps > 32 {
            return Err(Error::InvalidData("num_long_term_ref_pics_sps > 32"));
        }
        for _ in 0..num_lt_sps {
            let lsb = r.u(log2_max_poc_lsb)?;
            lt_sps.push((lsb, r.u1()? != 0));
        }
    }
    let temporal_mvp = r.u1()? != 0;
    let strong_intra_smoothing = r.u1()? != 0;
    let mut sps = Sps {
        id,
        separate_colour_plane,
        chroma_array_type,
        sub_w,
        sub_h,
        width,
        height,
        crop,
        bit_depth_y,
        bit_depth_c,
        log2_max_poc_lsb,
        log2_min_cb,
        log2_ctb,
        log2_min_tb,
        log2_max_tb,
        max_tr_depth_intra,
        scaling_list_enabled,
        scaling: sps_scaling,
        sao_enabled,
        pcm_enabled,
        pcm_bit_depth_y: pbd_y,
        pcm_bit_depth_c: pbd_c,
        log2_min_pcm_cb: l2minp,
        log2_max_pcm_cb: l2maxp,
        pcm_loop_filter_disabled: pcm_lf,
        num_st_rps,
        st_rps,
        long_term_present,
        num_lt_sps,
        lt_sps,
        amp,
        max_tr_depth_inter,
        max_dec_pic_buffering: max_dec,
        max_num_reorder: max_reorder,
        max_latency,
        explicit_rdpcm: false,
        high_precision_offsets: false,
        temporal_mvp,
        strong_intra_smoothing,
        intra_smoothing_disabled: false,
        persistent_rice: false,
        transform_skip_context: false,
        transform_skip_rotation: false,
        implicit_rdpcm: false,
        extended_precision: false,
        cabac_bypass_alignment: false,
        signal: None,
        field_seq: false,
        chroma_loc: None,
        v1_profile: ptl.is_v1(),
    };
    if r.u1()? != 0 {
        (sps.signal, sps.field_seq, sps.chroma_loc) = vui::parse(r, max_sub_minus1)?;
    }
    // Version 1 streams may set sps_extension_flag with any data after it (7.4.3.2.1).
    if r.more_rbsp_data() && r.u1()? != 0 && !sps.v1_profile {
        parse_extensions(r, &mut sps)?;
    }
    Ok(sps)
}

/// Parses the SPS extension flags and `sps_range_extension()`.
fn parse_extensions(r: &mut BitReader<'_>, sps: &mut Sps) -> Result<()> {
    let range_ext = r.u1()? != 0;
    let multilayer = r.u1()? != 0;
    let three_d = r.u1()? != 0;
    let scc = r.u1()? != 0;
    r.skip(4)?; // sps_extension_4bits
    if range_ext {
        sps.transform_skip_rotation = r.u1()? != 0;
        sps.transform_skip_context = r.u1()? != 0;
        sps.implicit_rdpcm = r.u1()? != 0;
        let explicit_rdpcm = r.u1()? != 0;
        sps.extended_precision = r.u1()? != 0;
        sps.intra_smoothing_disabled = r.u1()? != 0;
        sps.high_precision_offsets = r.u1()? != 0;
        sps.persistent_rice = r.u1()? != 0;
        sps.cabac_bypass_alignment = r.u1()? != 0;
        sps.explicit_rdpcm = explicit_rdpcm;
    }
    // sps_multilayer_extension() holds one inter-view flag, meaningless for
    // the base layer this decoder reconstructs; nothing the decoder reads
    // follows the extensions, so the remaining bits are left unread.
    let _ = multilayer;
    if three_d || scc {
        return Err(Error::Unsupported("SPS 3D or SCC extension"));
    }
    Ok(())
}
