//! Picture parameter set (clause 7.3.2.3).

use super::scaling;
use crate::bits::BitReader;
use crate::error::{Error, Result};
use alloc::vec::Vec;

/// Everything the decoder needs from a PPS.
#[derive(Debug, Clone, Default)]
pub struct Pps {
    /// `pps_pic_parameter_set_id`.
    pub id: u32,
    /// `pps_seq_parameter_set_id`.
    pub sps_id: u32,
    /// `dependent_slice_segments_enabled_flag`.
    pub dependent_slices: bool,
    /// `output_flag_present_flag`.
    pub output_flag_present: bool,
    /// `num_extra_slice_header_bits`.
    pub extra_slice_header_bits: u32,
    /// `sign_data_hiding_enabled_flag`.
    pub sign_data_hiding: bool,
    /// `cabac_init_present_flag`.
    pub cabac_init_present: bool,
    /// `num_ref_idx_l0_default_active_minus1 + 1` and the same for list 1.
    pub num_ref_idx_default: [u32; 2],
    /// `weighted_pred_flag`.
    pub weighted_pred: bool,
    /// `weighted_bipred_flag`.
    pub weighted_bipred: bool,
    /// `lists_modification_present_flag`.
    pub lists_modification_present: bool,
    /// `Log2ParMrgLevel`.
    pub log2_par_mrg_level: u32,
    /// `init_qp_minus26 + 26`.
    pub init_qp: i32,
    /// `constrained_intra_pred_flag`.
    pub constrained_intra_pred: bool,
    /// `transform_skip_enabled_flag`.
    pub transform_skip: bool,
    /// `cu_qp_delta_enabled_flag`.
    pub cu_qp_delta_enabled: bool,
    /// `diff_cu_qp_delta_depth`.
    pub diff_cu_qp_delta_depth: u32,
    /// `pps_cb_qp_offset`.
    pub cb_qp_offset: i32,
    /// `pps_cr_qp_offset`.
    pub cr_qp_offset: i32,
    /// `pps_slice_chroma_qp_offsets_present_flag`.
    pub slice_chroma_qp_offsets_present: bool,
    /// `transquant_bypass_enabled_flag`.
    pub transquant_bypass: bool,
    /// `tiles_enabled_flag`.
    pub tiles_enabled: bool,
    /// `entropy_coding_sync_enabled_flag`.
    pub wpp_enabled: bool,
    /// `num_tile_columns_minus1 + 1`.
    pub num_tile_cols: usize,
    /// `num_tile_rows_minus1 + 1`.
    pub num_tile_rows: usize,
    /// `uniform_spacing_flag`.
    pub uniform_spacing: bool,
    /// `column_width_minus1[i] + 1`, empty when spacing is uniform.
    pub column_widths: Vec<usize>,
    /// `row_height_minus1[i] + 1`, empty when spacing is uniform.
    pub row_heights: Vec<usize>,
    /// `loop_filter_across_tiles_enabled_flag`.
    pub across_tiles: bool,
    /// `pps_loop_filter_across_slices_enabled_flag`.
    pub across_slices: bool,
    /// `deblocking_filter_override_enabled_flag`.
    pub deblock_override_enabled: bool,
    /// `pps_deblocking_filter_disabled_flag`.
    pub deblock_disabled: bool,
    /// `pps_beta_offset_div2`.
    pub beta_offset_div2: i32,
    /// `pps_tc_offset_div2`.
    pub tc_offset_div2: i32,
    /// Scaling lists from the PPS, when `pps_scaling_list_data_present_flag`.
    pub scaling: Option<scaling::ScalingListData>,
    /// `slice_segment_header_extension_present_flag`.
    pub slice_header_extension: bool,
    /// `Log2MaxTransformSkipBlockSize`.
    pub log2_max_transform_skip: usize,
    /// `cross_component_prediction_enabled_flag` from the range extension.
    pub cross_component_prediction: bool,
    /// `chroma_qp_offset_list_enabled_flag`.
    pub chroma_qp_offset_list_enabled: bool,
    /// `diff_cu_chroma_qp_offset_depth`.
    pub diff_cu_chroma_qp_offset_depth: u32,
    /// `cb_qp_offset_list[]`.
    pub cb_qp_offset_list: Vec<i32>,
    /// `cr_qp_offset_list[]`.
    pub cr_qp_offset_list: Vec<i32>,
    /// `log2_sao_offset_scale_luma`.
    pub sao_offset_scale_luma: u32,
    /// `log2_sao_offset_scale_chroma`.
    pub sao_offset_scale_chroma: u32,
}

/// Parses `pic_parameter_set_rbsp()`. `v1_sps` tells whether the SPS with a given id is
/// known to declare a version 1 profile, whose PPS extension bits are ignored.
pub fn parse(data: &[u8], v1_sps: impl Fn(u32) -> bool) -> Result<Pps> {
    let r = &mut BitReader::new(data);
    let mut p = Pps {
        num_tile_cols: 1,
        num_tile_rows: 1,
        uniform_spacing: true,
        across_tiles: true,
        log2_max_transform_skip: 2,
        cross_component_prediction: false,
        ..Default::default()
    };
    p.id = r.ue()?;
    p.sps_id = r.ue()?;
    p.dependent_slices = r.u1()? != 0;
    p.output_flag_present = r.u1()? != 0;
    p.extra_slice_header_bits = r.u(3)?;
    p.sign_data_hiding = r.u1()? != 0;
    p.cabac_init_present = r.u1()? != 0;
    // Range-checked when a slice uses them, so that a still is not refused
    // over a field only inter slices read.
    p.num_ref_idx_default = [r.ue()?.saturating_add(1), r.ue()?.saturating_add(1)];
    // init_qp_minus26 lies in -(26 + QpBdOffsetY)..=25 (7.4.3.3.1); the
    // widest QpBdOffsetY (16-bit) bounds it before the SPS is known.
    p.init_qp = se_in(r, -(26 + 48), 25, "init_qp_minus26")? + 26;
    p.constrained_intra_pred = r.u1()? != 0;
    p.transform_skip = r.u1()? != 0;
    p.cu_qp_delta_enabled = r.u1()? != 0;
    if p.cu_qp_delta_enabled {
        p.diff_cu_qp_delta_depth = r.ue()?;
    }
    p.cb_qp_offset = se_in(r, -12, 12, "pps_cb_qp_offset")?;
    p.cr_qp_offset = se_in(r, -12, 12, "pps_cr_qp_offset")?;
    p.slice_chroma_qp_offsets_present = r.u1()? != 0;
    p.weighted_pred = r.u1()? != 0;
    p.weighted_bipred = r.u1()? != 0;
    p.transquant_bypass = r.u1()? != 0;
    p.tiles_enabled = r.u1()? != 0;
    p.wpp_enabled = r.u1()? != 0;
    if p.tiles_enabled {
        p.num_tile_cols = r.ue()? as usize + 1;
        p.num_tile_rows = r.ue()? as usize + 1;
        if p.num_tile_cols > 1024 || p.num_tile_rows > 1024 {
            return Err(Error::InvalidData("implausible tile count"));
        }
        p.uniform_spacing = r.u1()? != 0;
        if !p.uniform_spacing {
            for _ in 0..p.num_tile_cols - 1 {
                p.column_widths.push(r.ue()? as usize + 1);
            }
            for _ in 0..p.num_tile_rows - 1 {
                p.row_heights.push(r.ue()? as usize + 1);
            }
        }
        p.across_tiles = r.u1()? != 0;
    }
    p.across_slices = r.u1()? != 0;
    if r.u1()? != 0 {
        // deblocking_filter_control_present_flag
        p.deblock_override_enabled = r.u1()? != 0;
        p.deblock_disabled = r.u1()? != 0;
        if !p.deblock_disabled {
            p.beta_offset_div2 = r.se()?;
            p.tc_offset_div2 = r.se()?;
        }
    }
    if r.u1()? != 0 {
        let mut d = scaling::ScalingListData::default();
        scaling::parse(r, &mut d)?;
        p.scaling = Some(d);
    }
    p.lists_modification_present = r.u1()? != 0;
    p.log2_par_mrg_level = r.ue()?.saturating_add(2);
    p.slice_header_extension = r.u1()? != 0;
    // Version 1 streams may set pps_extension_flag with any data after it (7.4.3.3.1).
    if r.more_rbsp_data() && r.u1()? != 0 && !v1_sps(p.sps_id) {
        parse_extensions(r, &mut p)?;
    }
    Ok(p)
}

/// `se(v)` checked against its range `lo..=hi`: the QP arithmetic of the
/// decoding process may then add these without overflow.
pub(crate) fn se_in(r: &mut BitReader<'_>, lo: i32, hi: i32, what: &'static str) -> Result<i32> {
    let v = r.se()?;
    if !(lo..=hi).contains(&v) {
        return Err(Error::InvalidData(what));
    }
    Ok(v)
}

/// Parses the PPS extension flags and `pps_range_extension()`.
fn parse_extensions(r: &mut BitReader<'_>, p: &mut Pps) -> Result<()> {
    let range_ext = r.u1()? != 0;
    let _multilayer = r.u1()? != 0;
    let other = r.u(2)? != 0; // pps_3d_extension_flag, pps_scc_extension_flag
    r.skip(4)?; // pps_extension_4bits
    if range_ext {
        if p.transform_skip {
            p.log2_max_transform_skip = r.ue()? as usize + 2;
        }
        p.cross_component_prediction = r.u1()? != 0;
        p.chroma_qp_offset_list_enabled = r.u1()? != 0;
        if p.chroma_qp_offset_list_enabled {
            p.diff_cu_chroma_qp_offset_depth = r.ue()?;
            let len = r.ue()? as usize + 1;
            if len > 6 {
                return Err(Error::InvalidData("chroma_qp_offset_list_len_minus1 > 5"));
            }
            for _ in 0..len {
                p.cb_qp_offset_list
                    .push(se_in(r, -12, 12, "cb_qp_offset_list")?);
                p.cr_qp_offset_list
                    .push(se_in(r, -12, 12, "cr_qp_offset_list")?);
            }
        }
        p.sao_offset_scale_luma = r.ue()?;
        p.sao_offset_scale_chroma = r.ue()?;
    }
    // pps_multilayer_extension() only concerns inter-layer prediction of
    // enhancement layers; the base layer decodes without it, and nothing the
    // decoder reads follows it, so it is left unread.
    if other {
        return Err(Error::Unsupported("PPS 3D or SCC extension"));
    }
    Ok(())
}
