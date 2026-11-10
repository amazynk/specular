//! Builds complete synthetic HEVC bitstreams, bit by bit.
//!
//! Nothing external is needed: the parameter sets are written with this
//! module tree's own bit writer and the slice data with its own CABAC
//! encoder. The golden-vector test and the benchmarks both
//! build their input here, so the whole path from NAL parsing through CABAC,
//! the coding tree, intra prediction and the loop filters is exercised without
//! any external fixture.

use crate::bits::BitWriter;
use crate::cabac::ctx::off;
use crate::cabac::encoder::Enc;
use alloc::vec::Vec;

/// Wraps an RBSP in a NAL unit header, inserting emulation prevention bytes.
pub fn to_nal(nal_type: u8, rbsp: &[u8]) -> Vec<u8> {
    let mut out = alloc::vec![nal_type << 1, 1];
    let mut zeros = 0usize;
    for &b in rbsp {
        if zeros >= 2 && b <= 3 {
            out.push(3);
            zeros = 0;
        }
        zeros = if b == 0 { zeros + 1 } else { 0 };
        out.push(b);
    }
    out
}

/// Builds a video parameter set with the mandatory fields only.
pub fn build_vps() -> Vec<u8> {
    let mut w = BitWriter::default();
    w.put(0, 4); // vps_video_parameter_set_id
    w.put(3, 2); // vps_base_layer_internal_flag, vps_base_layer_available_flag
    w.put(0, 6); // vps_max_layers_minus1
    w.put(0, 3); // vps_max_sub_layers_minus1
    w.put(1, 1); // vps_temporal_id_nesting_flag
    w.put(0xffff, 16);
    profile_tier_level(&mut w);
    w.put(1, 1); // vps_sub_layer_ordering_info_present_flag
    w.ue(0); // vps_max_dec_pic_buffering_minus1
    w.ue(0); // vps_max_num_reorder_pics
    w.ue(0); // vps_max_latency_increase_plus1
    w.put(0, 6); // vps_max_layer_id
    w.ue(0); // vps_num_layer_sets_minus1
    w.put(0, 1); // vps_timing_info_present_flag
    w.put(0, 1); // vps_extension_flag
    w.rbsp_trailing();
    w.bytes
}

/// Writes `profile_tier_level(1, 0)` for the Main Still Picture profile.
fn profile_tier_level(w: &mut BitWriter) {
    w.put(0, 2); // general_profile_space
    w.put(0, 1); // general_tier_flag
    w.put(3, 5); // general_profile_idc: Main Still Picture
    for i in 0..32 {
        w.put(u32::from(i == 3), 1);
    }
    w.put(0b1101, 4); // progressive, not interlaced, non-packed, frame only
    w.put(0, 22); // general_reserved_zero_43bits, first half
    w.put(0, 21); // general_reserved_zero_43bits, second half
    w.put(0, 1); // general_inbld_flag
    w.put(30, 8); // general_level_idc
}

/// Builds a sequence parameter set for a 64x64 8-bit 4:2:0 picture.
pub fn build_sps(width: u32, height: u32, chroma_idc: u32, bit_depth: u32) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.put(0, 4); // sps_video_parameter_set_id
    w.put(0, 3); // sps_max_sub_layers_minus1
    w.put(1, 1); // sps_temporal_id_nesting_flag
    profile_tier_level(&mut w);
    w.ue(0); // sps_seq_parameter_set_id
    w.ue(chroma_idc); // chroma_format_idc
    if chroma_idc == 3 {
        w.put(0, 1); // separate_colour_plane_flag
    }
    w.ue(width); // pic_width_in_luma_samples
    w.ue(height); // pic_height_in_luma_samples
    w.put(0, 1); // conformance_window_flag
    w.ue(bit_depth - 8); // bit_depth_luma_minus8
    w.ue(bit_depth - 8); // bit_depth_chroma_minus8
    w.ue(4); // log2_max_pic_order_cnt_lsb_minus4
    w.put(1, 1); // sps_sub_layer_ordering_info_present_flag
    w.ue(0);
    w.ue(0);
    w.ue(0);
    w.ue(0); // log2_min_luma_coding_block_size_minus3: 8x8
    w.ue(3); // log2_diff_max_min_luma_coding_block_size: CTB 64x64
    w.ue(0); // log2_min_luma_transform_block_size_minus2: 4x4
    w.ue(3); // log2_diff_max_min_luma_transform_block_size: 32x32
    w.ue(0); // max_transform_hierarchy_depth_inter
    w.ue(0); // max_transform_hierarchy_depth_intra
    w.put(0, 1); // scaling_list_enabled_flag
    w.put(0, 1); // amp_enabled_flag
    w.put(0, 1); // sample_adaptive_offset_enabled_flag
    w.put(0, 1); // pcm_enabled_flag
    w.ue(0); // num_short_term_ref_pic_sets
    w.put(0, 1); // long_term_ref_pics_present_flag
    w.put(0, 1); // sps_temporal_mvp_enabled_flag
    w.put(0, 1); // strong_intra_smoothing_enabled_flag
    w.put(0, 1); // vui_parameters_present_flag
    w.put(0, 1); // sps_extension_present_flag
    w.rbsp_trailing();
    w.bytes
}

/// Builds a picture parameter set with the loop filters off; `tiles` asks for
/// a two by two uniform tile grid.
pub fn build_pps_opt(tiles: bool) -> Vec<u8> {
    build_pps_grid(tiles.then_some((2, 2)), false, false)
}

/// Builds a picture parameter set with a uniform grid of `tiles` (columns,
/// rows) if given, wavefronts if `wpp`, and deblocking if `deblock`.
pub fn build_pps_grid(tiles: Option<(u32, u32)>, wpp: bool, deblock: bool) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.ue(0); // pps_pic_parameter_set_id
    w.ue(0); // pps_seq_parameter_set_id
    w.put(0, 1); // dependent_slice_segments_enabled_flag
    w.put(0, 1); // output_flag_present_flag
    w.put(0, 3); // num_extra_slice_header_bits
    w.put(0, 1); // sign_data_hiding_enabled_flag
    w.put(0, 1); // cabac_init_present_flag
    w.ue(0);
    w.ue(0);
    w.se(0); // init_qp_minus26
    w.put(0, 1); // constrained_intra_pred_flag
    w.put(0, 1); // transform_skip_enabled_flag
    w.put(0, 1); // cu_qp_delta_enabled_flag
    w.se(0); // pps_cb_qp_offset
    w.se(0); // pps_cr_qp_offset
    w.put(0, 1); // pps_slice_chroma_qp_offsets_present_flag
    w.put(0, 2); // weighted_pred_flag, weighted_bipred_flag
    w.put(0, 1); // transquant_bypass_enabled_flag
    w.put(u32::from(tiles.is_some()), 1); // tiles_enabled_flag
    w.put(u32::from(wpp), 1); // entropy_coding_sync_enabled_flag
    if let Some((cols, rows)) = tiles {
        w.ue(cols - 1); // num_tile_columns_minus1
        w.ue(rows - 1); // num_tile_rows_minus1
        w.put(1, 1); // uniform_spacing_flag
        w.put(1, 1); // loop_filter_across_tiles_enabled_flag
    }
    w.put(0, 1); // pps_loop_filter_across_slices_enabled_flag
    w.put(1, 1); // deblocking_filter_control_present_flag
    w.put(0, 1); // deblocking_filter_override_enabled_flag
    w.put(u32::from(!deblock), 1); // pps_deblocking_filter_disabled_flag
    if deblock {
        w.se(0); // pps_beta_offset_div2
        w.se(0); // pps_tc_offset_div2
    }
    w.put(0, 1); // pps_scaling_list_data_present_flag
    w.put(0, 1); // lists_modification_present_flag
    w.ue(0); // log2_parallel_merge_level_minus2
    w.put(0, 1); // slice_segment_header_extension_present_flag
    w.put(0, 1); // pps_extension_present_flag
    w.rbsp_trailing();
    w.bytes
}

/// Builds the single I slice: one 64x64 coding unit predicted with DC.
pub fn build_slice(ctbs: usize, chroma_idc: u32) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.put(1, 1); // first_slice_segment_in_pic_flag
    w.put(0, 1); // no_output_of_prior_pics_flag
    w.ue(0); // slice_pic_parameter_set_id
    w.ue(2); // slice_type: I
    w.se(0); // slice_qp_delta
    w.put(1, 1); // alignment_bit_equal_to_one
    while w.len_bits() % 8 != 0 {
        w.put(0, 1);
    }
    let mut e = Enc::new(26);
    for k in 0..ctbs {
        encode_ctu(&mut e, chroma_idc);
        e.terminate(u32::from(k + 1 == ctbs)); // end_of_slice_segment_flag
    }
    let mut out = w.bytes;
    out.extend_from_slice(&e.finish());
    out
}

/// Builds a complete DC-predicted still picture of `width` x `height` samples.
///
/// Both dimensions must be multiples of the 64 sample coding tree block size,
/// `chroma_idc` is `chroma_format_idc` (0, 1, 2 or 3) and `bit_depth` is 8 to
/// 14. The returned parameter sets and slice are ready for
/// [`crate::decode_still`]; every sample of the result is `1 << (bit_depth - 1)`.
pub fn picture_fmt(
    width: u32,
    height: u32,
    chroma_idc: u32,
    bit_depth: u32,
) -> (Vec<Vec<u8>>, Vec<u8>) {
    let ctbs = (width as usize / 64) * (height as usize / 64);
    let sets = alloc::vec![
        to_nal(32, &build_vps()),
        to_nal(33, &build_sps(width, height, chroma_idc, bit_depth)),
        to_nal(34, &build_pps()),
    ];
    (sets, to_nal(19, &build_slice(ctbs, chroma_idc)))
}

/// [`picture_fmt`] for the common case of 8-bit 4:2:0.
pub fn picture(width: u32, height: u32) -> (Vec<Vec<u8>>, Vec<u8>) {
    picture_fmt(width, height, 1, 8)
}

/// [`build_pps_opt`] without tiles.
pub fn build_pps() -> Vec<u8> {
    build_pps_opt(false)
}

/// Encodes the coding unit syntax of one all-DC 64x64 coding tree block.
fn encode_ctu(e: &mut Enc, chroma_idc: u32) {
    e.decision(off::SPLIT_CU, 0); // split_cu_flag: keep the 64x64 coding unit
    e.decision(off::PREV_INTRA, 1); // prev_intra_luma_pred_flag
    e.bypass(1); // mpm_idx = 1, selecting INTRA_DC from {planar, DC, vertical}
    e.bypass(0);
    if chroma_idc != 0 {
        e.decision(off::INTRA_CHROMA, 0); // intra_chroma_pred_mode: from luma
        e.decision(off::CBF_CHROMA, 0); // cbf_cb at trafoDepth 0
        e.decision(off::CBF_CHROMA, 0); // cbf_cr at trafoDepth 0
    }
    for _ in 0..4 {
        e.decision(off::CBF_LUMA, 0); // cbf_luma of each forced 32x32 split
    }
}

/// Builds a 256x256 picture cut into a two by two grid of tiles.
///
/// Each tile is its own CABAC substream, so this exercises the tile scan, the
/// context reset at a tile boundary and `end_of_subset_one_bit`. No entry point
/// offsets are signalled, so the decoder has to find each substream itself.
pub fn picture_tiled() -> (Vec<Vec<u8>>, Vec<u8>) {
    let mut w = BitWriter::default();
    w.put(1, 1); // first_slice_segment_in_pic_flag
    w.put(0, 1); // no_output_of_prior_pics_flag
    w.ue(0); // slice_pic_parameter_set_id
    w.ue(2); // slice_type: I
    w.se(0); // slice_qp_delta
    w.ue(0); // num_entry_point_offsets
    w.put(1, 1); // alignment_bit_equal_to_one
    while w.len_bits() % 8 != 0 {
        w.put(0, 1);
    }
    let mut out = w.bytes;
    for tile in 0..4 {
        let mut e = Enc::new(26);
        for ctb in 0..4 {
            encode_ctu(&mut e, 1);
            let last = tile == 3 && ctb == 3;
            e.terminate(u32::from(last)); // end_of_slice_segment_flag
        }
        if tile != 3 {
            e.terminate(1); // end_of_subset_one_bit
        }
        out.extend_from_slice(&e.finish());
    }
    let sets = alloc::vec![
        to_nal(32, &build_vps()),
        to_nal(33, &build_sps(256, 256, 1, 8)),
        to_nal(34, &build_pps_opt(true)),
    ];
    (sets, to_nal(19, &out))
}

/// A small xorshift generator, so the synthetic content is reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self, n: u32) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32 % n
    }
}

/// Encodes a residual block whose only coefficient is a DC of level 1 to 3.
fn encode_dc_residual(e: &mut Enc, rng: &mut Rng, log2_size: usize, chroma: bool) {
    // last_sig_coeff_x_prefix and _y_prefix of 0, the first bin's context.
    let last = if chroma {
        15
    } else {
        3 * (log2_size - 2) + ((log2_size - 1) >> 2)
    };
    e.decision(off::LAST_X + last, 0);
    e.decision(off::LAST_Y + last, 0);
    // The last position needs no coded_sub_block_flag or sig_coeff_flag.
    let set = if chroma { 16 } else { 0 };
    let level = 1 + rng.next(3);
    e.decision(off::GT1 + set + 1, u32::from(level > 1));
    if level > 1 {
        e.decision(off::GT2 + if chroma { 4 } else { 0 }, u32::from(level > 2));
    }
    e.bypass(rng.next(2)); // coeff_sign_flag
    if level > 2 {
        e.bypass(0); // coeff_abs_level_remaining of 0, Rice parameter 0
    }
}

/// Encodes one intra 64x64 coding unit in 4:2:0 with a random most probable
/// mode and random DC residuals in some of its transform blocks.
fn encode_ctu_textured(e: &mut Enc, rng: &mut Rng) {
    e.decision(off::SPLIT_CU, 0);
    e.decision(off::PREV_INTRA, 1);
    match rng.next(3) {
        0 => e.bypass(0),
        m => {
            e.bypass(1);
            e.bypass(u32::from(m == 2));
        }
    }
    e.decision(off::INTRA_CHROMA, 0);
    let (cb, cr) = (rng.next(2), rng.next(2));
    e.decision(off::CBF_CHROMA, cb);
    e.decision(off::CBF_CHROMA, cr);
    for _ in 0..4 {
        let c = [cb != 0 && rng.next(2) != 0, cr != 0 && rng.next(2) != 0];
        if cb != 0 {
            e.decision(off::CBF_CHROMA + 1, u32::from(c[0]));
        }
        if cr != 0 {
            e.decision(off::CBF_CHROMA + 1, u32::from(c[1]));
        }
        let y = rng.next(4) != 0;
        e.decision(off::CBF_LUMA, u32::from(y));
        if y {
            encode_dc_residual(e, rng, 5, false);
        }
        for on in c {
            if on {
                encode_dc_residual(e, rng, 4, true);
            }
        }
    }
}

/// Builds a `width` x `height` intra picture (multiples of 64) in one slice
/// with a uniform grid of `tiles` (columns, rows) and wavefronts, its
/// content textured from `seed`, and an entry point for every substream.
pub fn picture_tiles_wpp(
    width: u32,
    height: u32,
    tiles: (u32, u32),
    deblock: bool,
    seed: u64,
) -> (Vec<Vec<u8>>, Vec<u8>) {
    let (w, h) = (width as usize / 64, height as usize / 64);
    let bounds =
        |n: usize, k: u32| -> Vec<usize> { (0..=k as usize).map(|i| i * n / k as usize).collect() };
    let (cols, rows) = (bounds(w, tiles.0), bounds(h, tiles.1));
    let mut rng = Rng(seed | 1);
    let mut subs: Vec<Vec<u8>> = Vec::new();
    let n_tiles = tiles.0 as usize * tiles.1 as usize;
    for t in 0..n_tiles {
        let (tx, ty) = (t % tiles.0 as usize, t / tiles.0 as usize);
        let (x0, x1, y0, y1) = (cols[tx], cols[tx + 1], rows[ty], rows[ty + 1]);
        let mut saved = None;
        for y in y0..y1 {
            let mut e = Enc::new(26);
            if let Some(ctx) = saved.take().filter(|_| x1 - x0 >= 2) {
                e.set_contexts(ctx);
            }
            for x in x0..x1 {
                encode_ctu_textured(&mut e, &mut rng);
                if x == x0 + 1 {
                    saved = Some(e.contexts());
                }
                let last = t + 1 == n_tiles && y + 1 == y1 && x + 1 == x1;
                e.terminate(u32::from(last)); // end_of_slice_segment_flag
            }
            if !(t + 1 == n_tiles && y + 1 == y1) {
                e.terminate(1); // end_of_subset_one_bit
            }
            subs.push(e.finish());
        }
    }
    // Entry point offsets count emulation prevention bytes; the header ends
    // in a non-zero byte, so the slice data escapes as it would alone.
    let mut sizes = Vec::new();
    let mut zeros = 0usize;
    for s in &subs {
        let mut n = 0u32;
        for &b in s {
            if zeros >= 2 && b <= 3 {
                n += 1;
                zeros = 0;
            }
            zeros = if b == 0 { zeros + 1 } else { 0 };
            n += 1;
        }
        sizes.push(n);
    }
    sizes.pop();
    let len = sizes
        .iter()
        .map(|&s| 32 - (s - 1).leading_zeros())
        .max()
        .unwrap_or(1)
        .max(1);
    let mut wr = BitWriter::default();
    wr.put(1, 1); // first_slice_segment_in_pic_flag
    wr.put(0, 1); // no_output_of_prior_pics_flag
    wr.ue(0); // slice_pic_parameter_set_id
    wr.ue(2); // slice_type: I
    wr.se(0); // slice_qp_delta
    wr.ue(sizes.len() as u32); // num_entry_point_offsets
    if !sizes.is_empty() {
        wr.ue(len - 1); // offset_len_minus1
        for &s in &sizes {
            wr.put(s - 1, len); // entry_point_offset_minus1
        }
    }
    wr.put(1, 1); // alignment_bit_equal_to_one
    while wr.len_bits() % 8 != 0 {
        wr.put(0, 1);
    }
    let mut out = wr.bytes;
    for s in &subs {
        out.extend_from_slice(s);
    }
    let sets = alloc::vec![
        to_nal(32, &build_vps()),
        to_nal(33, &build_sps(width, height, 1, 8)),
        to_nal(34, &build_pps_grid(Some(tiles), true, deblock)),
    ];
    (sets, to_nal(19, &out))
}
