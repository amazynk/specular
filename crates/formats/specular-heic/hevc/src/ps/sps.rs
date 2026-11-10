//! Sequence parameter set (clause 7.3.2.2).

use super::{rps, scaling};

/// Everything the decoder needs from an SPS.
#[derive(Debug, Clone)]
pub struct Sps {
    /// `sps_seq_parameter_set_id`.
    pub id: u32,
    /// `separate_colour_plane_flag`.
    pub separate_colour_plane: bool,
    /// `ChromaArrayType`.
    pub chroma_array_type: u8,
    /// `SubWidthC`, or 0 for monochrome.
    pub sub_w: usize,
    /// `SubHeightC`, or 0 for monochrome.
    pub sub_h: usize,
    /// `pic_width_in_luma_samples`.
    pub width: usize,
    /// `pic_height_in_luma_samples`.
    pub height: usize,
    /// Conformance window offsets in luma samples: left, right, top, bottom.
    pub crop: [usize; 4],
    /// `BitDepthY`.
    pub bit_depth_y: u8,
    /// `BitDepthC`.
    pub bit_depth_c: u8,
    /// `log2_max_pic_order_cnt_lsb_minus4 + 4`.
    pub log2_max_poc_lsb: u32,
    /// `MinCbLog2SizeY`.
    pub log2_min_cb: usize,
    /// `CtbLog2SizeY`.
    pub log2_ctb: usize,
    /// `MinTbLog2SizeY`.
    pub log2_min_tb: usize,
    /// `MaxTbLog2SizeY`.
    pub log2_max_tb: usize,
    /// `max_transform_hierarchy_depth_intra`.
    pub max_tr_depth_intra: u32,
    /// `scaling_list_enabled_flag`.
    pub scaling_list_enabled: bool,
    /// Scaling lists from the SPS, when `sps_scaling_list_data_present_flag`.
    pub scaling: Option<scaling::ScalingListData>,
    /// `sample_adaptive_offset_enabled_flag`.
    pub sao_enabled: bool,
    /// `pcm_enabled_flag`.
    pub pcm_enabled: bool,
    /// `PcmBitDepthY`.
    pub pcm_bit_depth_y: u8,
    /// `PcmBitDepthC`.
    pub pcm_bit_depth_c: u8,
    /// `Log2MinIpcmCbSizeY`.
    pub log2_min_pcm_cb: usize,
    /// `Log2MaxIpcmCbSizeY`.
    pub log2_max_pcm_cb: usize,
    /// `pcm_loop_filter_disabled_flag`.
    pub pcm_loop_filter_disabled: bool,
    /// `num_short_term_ref_pic_sets`.
    pub num_st_rps: u32,
    /// The SPS short-term reference picture sets.
    pub st_rps: alloc::vec::Vec<rps::StRps>,
    /// `long_term_ref_pics_present_flag`.
    pub long_term_present: bool,
    /// `num_long_term_ref_pics_sps`.
    pub num_lt_sps: u32,
    /// `lt_ref_pic_poc_lsb_sps[]` and `used_by_curr_pic_lt_sps_flag[]`.
    pub lt_sps: alloc::vec::Vec<(u32, bool)>,
    /// `amp_enabled_flag`.
    pub amp: bool,
    /// `max_transform_hierarchy_depth_inter`.
    pub max_tr_depth_inter: u32,
    /// `sps_max_dec_pic_buffering_minus1 + 1` of the highest sub-layer.
    pub max_dec_pic_buffering: u32,
    /// `sps_max_num_reorder_pics` of the highest sub-layer.
    pub max_num_reorder: u32,
    /// `SpsMaxLatencyPictures` of the highest sub-layer, or `None`.
    pub max_latency: Option<u32>,
    /// `explicit_rdpcm_enabled_flag` from the range extension.
    pub explicit_rdpcm: bool,
    /// `high_precision_offsets_enabled_flag` from the range extension.
    pub high_precision_offsets: bool,
    /// `sps_temporal_mvp_enabled_flag`.
    pub temporal_mvp: bool,
    /// `strong_intra_smoothing_enabled_flag`.
    pub strong_intra_smoothing: bool,
    /// `intra_smoothing_disabled_flag` from the range extension.
    pub intra_smoothing_disabled: bool,
    /// `persistent_rice_adaptation_enabled_flag` from the range extension.
    pub persistent_rice: bool,
    /// `transform_skip_context_enabled_flag` from the range extension.
    pub transform_skip_context: bool,
    /// `transform_skip_rotation_enabled_flag` from the range extension.
    pub transform_skip_rotation: bool,
    /// `implicit_rdpcm_enabled_flag` from the range extension.
    pub implicit_rdpcm: bool,
    /// `extended_precision_processing_flag` from the range extension.
    pub extended_precision: bool,
    /// `cabac_bypass_alignment_enabled_flag` from the range extension.
    pub cabac_bypass_alignment: bool,
    /// The VUI video signal type, when signalled.
    pub signal: Option<super::vui::Signal>,
    /// VUI `field_seq_flag`: every picture is one field.
    pub field_seq: bool,
    /// VUI `chroma_sample_loc_type_top_field` (0..=5), when signalled.
    pub chroma_loc: Option<u8>,
    /// A version 1 profile (Main, Main 10, Main Still Picture): the SPS and PPS extension
    /// bits of such streams are `*_extension_data_flag`s, which decoders ignore.
    pub v1_profile: bool,
}

impl Sps {
    /// `PicWidthInCtbsY`.
    pub fn width_in_ctbs(&self) -> usize {
        self.width.div_ceil(1 << self.log2_ctb)
    }

    /// `PicHeightInCtbsY`.
    pub fn height_in_ctbs(&self) -> usize {
        self.height.div_ceil(1 << self.log2_ctb)
    }

    /// `PicSizeInCtbsY`.
    pub fn size_in_ctbs(&self) -> usize {
        self.width_in_ctbs() * self.height_in_ctbs()
    }
}
