//! Damaged-stream regressions: header values that overflowed arithmetic in debug builds.

use std::sync::Arc;

use crate::dpb::Dpb;
use crate::params::{Sps, Vui};
use crate::picture::PicShared;
use crate::pixel::Format;
use crate::slice::{Mmco, SliceHeader, SliceType};
use crate::slicedec::{RefPic, implicit_weights};

fn sps() -> Sps {
    Sps {
        id: 0,
        profile_idc: 77,
        constraint_flags: 0,
        level_idc: 30,
        chroma_format_idc: 1,
        separate_colour_plane: false,
        bit_depth_luma: 8,
        bit_depth_chroma: 8,
        transform_bypass: false,
        scaling: None,
        log2_max_frame_num: 4,
        poc_type: 0,
        log2_max_poc_lsb: 4,
        delta_pic_order_always_zero: false,
        offset_for_non_ref_pic: 0,
        offset_for_top_to_bottom_field: 0,
        offset_for_ref_frame: Vec::new(),
        max_num_ref_frames: 1,
        gaps_allowed: false,
        width_mbs: 1,
        height_mbs: 1,
        frame_mbs_only: true,
        mbaff: false,
        direct_8x8_inference: true,
        crop: [0; 4],
        vui: Vui::default(),
    }
}

fn header() -> SliceHeader {
    SliceHeader {
        colour_plane: 0,
        nal_ref_idc: 1,
        first_mb: 0,
        slice_type: SliceType::I,
        pps_id: 0,
        frame_num: 1,
        field_pic: false,
        bottom_field: false,
        idr: false,
        idr_pic_id: 0,
        poc_lsb: 0,
        delta_poc_bottom: 0,
        delta_poc: [0; 2],
        redundant_pic_cnt: 0,
        direct_spatial: false,
        num_ref_idx: [0; 2],
        modifications: [Vec::new(), Vec::new()],
        weights: None,
        no_output_of_prior_pics: false,
        long_term_reference: false,
        mmcos: None,
        cabac_init_idc: 0,
        qp: 26,
        disable_deblocking: 0,
        alpha_offset: 0,
        beta_offset: 0,
        slice_group_change_cycle: 0,
        switching: crate::slice::Switching::None,
        qs: 0,
        data_bit: 0,
    }
}

/// delta_pic_order_cnt_bottom of -2^31 with memory_management_control_operation 5: the
/// counts were rebased by `top - min(top, bottom)`, which overflowed i32.
#[test]
fn mmco5_with_far_apart_field_counts() {
    let sps = sps();
    let mut h = header();
    h.poc_lsb = 7;
    h.delta_poc_bottom = i32::MIN;
    h.mmcos = Some(vec![Mmco::ClearAll]);
    let mut dpb = Dpb::default();
    let r = dpb.compute_poc(&sps, &h);
    let [top, bottom] = dpb.update_poc(&sps, &h, &r);
    assert!(top >= 0 && bottom >= 0);
    // The next picture's counts derive from the saturated state without overflowing.
    let mut next = header();
    next.frame_num = 2;
    let r = dpb.compute_poc(&sps, &next);
    dpb.update_poc(&sps, &next, &r);
}

/// Implicit bi-prediction weights between references at opposite ends of the i32 range:
/// DiffPicOrderCnt overflowed before it was clipped.
#[test]
fn implicit_weights_extreme_counts() {
    let pic = Arc::new(PicShared::grey(
        1,
        1,
        Format {
            cfi: 1,
            depth: [8, 8],
        },
    ));
    let r = |poc: i32| RefPic {
        pic: pic.clone(),
        id: 1,
        poc,
        pocs: [poc; 2],
        long: false,
        parity: None,
    };
    let (w0, w1) = implicit_weights(i32::MAX, &r(i32::MIN), &r(i32::MAX));
    assert_eq!(w0 + w1, 64);
    let (w0, w1) = implicit_weights(i32::MIN, &r(i32::MAX), &r(i32::MIN));
    assert_eq!(w0 + w1, 64);
}

/// Bits, MSB first, with Exp-Golomb codes; `nal` ends the RBSP with its stop bit.
#[derive(Default)]
struct Bits(Vec<bool>);

impl Bits {
    fn u(&mut self, n: u32, v: u32) -> &mut Self {
        for i in (0..n).rev() {
            self.0.push(v >> i & 1 != 0);
        }
        self
    }
    fn ue(&mut self, v: u32) -> &mut Self {
        let x = v + 1;
        let len = 32 - x.leading_zeros();
        self.u(len - 1, 0).u(len, x)
    }
    fn nal(&mut self, header: u8) -> Vec<u8> {
        self.0.push(true);
        while !self.0.len().is_multiple_of(8) {
            self.0.push(false);
        }
        let mut out = vec![header];
        out.extend(
            self.0
                .chunks(8)
                .map(|c| c.iter().fold(0u8, |a, &b| a << 1 | u8::from(b))),
        );
        out
    }
}

/// A Baseline 8192 × 4352 stream (the largest picture accepted) with 16 reference frames:
/// its parameter sets and the first slice header, some 30 bytes for ~900 MB of DPB.
fn huge_stream() -> [Vec<u8>; 3] {
    let sps = Bits::default()
        .u(8, 66) // profile_idc: Baseline
        .u(8, 0) // constraint flags
        .u(8, 51) // level_idc
        .ue(0) // seq_parameter_set_id
        .ue(0) // log2_max_frame_num − 4
        .ue(2) // pic_order_cnt_type
        .ue(16) // max_num_ref_frames
        .u(1, 0) // gaps_in_frame_num_value_allowed_flag
        .ue(511) // pic_width_in_mbs − 1
        .ue(271) // pic_height_in_map_units − 1
        .u(1, 1) // frame_mbs_only_flag
        .u(1, 1) // direct_8x8_inference_flag
        .u(1, 0) // frame_cropping_flag
        .u(1, 0) // vui_parameters_present_flag
        .nal(0x67);
    let pps = Bits::default()
        .ue(0) // pic_parameter_set_id
        .ue(0) // seq_parameter_set_id
        .u(1, 0) // entropy_coding_mode_flag
        .u(1, 0) // bottom_field_pic_order_in_frame_present_flag
        .ue(0) // num_slice_groups − 1
        .ue(0) // num_ref_idx_l0_default_active − 1
        .ue(0) // num_ref_idx_l1_default_active − 1
        .u(1, 0) // weighted_pred_flag
        .u(2, 0) // weighted_bipred_idc
        .ue(0) // pic_init_qp − 26 (se 0)
        .ue(0) // pic_init_qs − 26 (se 0)
        .ue(0) // chroma_qp_index_offset (se 0)
        .u(1, 1) // deblocking_filter_control_present_flag
        .u(1, 0) // constrained_intra_pred_flag
        .u(1, 0) // redundant_pic_cnt_present_flag
        .nal(0x68);
    let idr = Bits::default()
        .ue(0) // first_mb_in_slice
        .ue(7) // slice_type: I
        .ue(0) // pic_parameter_set_id
        .u(4, 0) // frame_num
        .ue(0) // idr_pic_id
        .u(1, 0) // no_output_of_prior_pics_flag
        .u(1, 0) // long_term_reference_flag
        .ue(0) // slice_qp_delta (se 0)
        .ue(1) // disable_deblocking_filter_idc
        .u(16, 0xffff) // (macroblock data)
        .nal(0x65);
    [sps, pps, idr]
}

/// The DPB of a ~30-byte stream is refused under a 64 MiB memory budget before any picture
/// buffer is made, with what it needs and what is free.
#[test]
fn dpb_within_memory_budget() {
    let [sps, pps, idr] = huge_stream();
    let mut d = crate::Decoder::new(crate::Options {
        threads: 1,
        output: crate::OutputOrder::Display,
    });
    d.decode_nal(&sps, 0).unwrap();
    d.decode_nal(&pps, 0).unwrap();
    let e = specular_core::budget::with_limit(64 << 20, || d.decode_nal(&idr, 0)).unwrap_err();
    let crate::Error::NeedsMemory(n) = e else {
        panic!("{e:?}")
    };
    // 16 reference frames and the current one of 139264 macroblocks of 384 bytes.
    assert_eq!(n.needed, (17 * 139_264 * 384u64).div_ceil(4) * 5);
    assert!(n.needed > n.free);
}

/// An Extended-profile 32 × 32 stream: an IDR picture of four I_PCM macroblocks, then a P
/// picture sent as data partitions: partition A codes one P_L0_16x16 macroblock (zero
/// motion, chroma coefficients in partition C) and skips the rest; `part_c` is the
/// partition C sent, if any.
fn partitioned_stream(part_c: Option<&[u8]>) -> Vec<Vec<u8>> {
    let sps = Bits::default()
        .u(8, 88) // profile_idc: Extended
        .u(8, 0) // constraint flags
        .u(8, 30) // level_idc
        .ue(0) // seq_parameter_set_id
        .ue(0) // log2_max_frame_num − 4
        .ue(2) // pic_order_cnt_type
        .ue(1) // max_num_ref_frames
        .u(1, 0) // gaps_in_frame_num_value_allowed_flag
        .ue(1) // pic_width_in_mbs − 1
        .ue(1) // pic_height_in_map_units − 1
        .u(1, 1) // frame_mbs_only_flag
        .u(1, 1) // direct_8x8_inference_flag
        .u(1, 0) // frame_cropping_flag
        .u(1, 0) // vui_parameters_present_flag
        .nal(0x67);
    let pps = Bits::default()
        .ue(0) // pic_parameter_set_id
        .ue(0) // seq_parameter_set_id
        .u(1, 0) // entropy_coding_mode_flag: CAVLC
        .u(1, 0) // bottom_field_pic_order_in_frame_present_flag
        .ue(0) // num_slice_groups − 1
        .ue(0) // num_ref_idx_l0_default_active − 1
        .ue(0) // num_ref_idx_l1_default_active − 1
        .u(1, 0) // weighted_pred_flag
        .u(2, 0) // weighted_bipred_idc
        .ue(0) // pic_init_qp − 26 (se 0)
        .ue(0) // pic_init_qs − 26 (se 0)
        .ue(0) // chroma_qp_index_offset (se 0)
        .u(1, 1) // deblocking_filter_control_present_flag
        .u(1, 0) // constrained_intra_pred_flag
        .u(1, 0) // redundant_pic_cnt_present_flag
        .nal(0x68);
    let mut b = Bits::default();
    b.ue(0) // first_mb_in_slice
        .ue(7) // slice_type: I
        .ue(0) // pic_parameter_set_id
        .u(4, 0) // frame_num
        .ue(0) // idr_pic_id
        .u(1, 0) // no_output_of_prior_pics_flag
        .u(1, 0) // long_term_reference_flag
        .ue(0) // slice_qp_delta (se 0)
        .ue(1); // disable_deblocking_filter_idc
    for mb in 0..4u32 {
        b.ue(25); // mb_type: I_PCM
        while !b.0.len().is_multiple_of(8) {
            b.0.push(false);
        }
        for i in 0..384u32 {
            b.u(8, 0x40 + mb * 0x20 + (i * 7) % 29);
        }
    }
    let idr = b.nal(0x65);
    let part_a = Bits::default()
        .ue(0) // first_mb_in_slice
        .ue(5) // slice_type: P
        .ue(0) // pic_parameter_set_id
        .u(4, 1) // frame_num
        .u(1, 0) // num_ref_idx_active_override_flag
        .u(1, 0) // ref_pic_list_modification_flag_l0
        .u(1, 0) // adaptive_ref_pic_marking_mode_flag
        .ue(0) // slice_qp_delta (se 0)
        .ue(1) // disable_deblocking_filter_idc
        .ue(0) // slice_id
        .ue(0) // mb_skip_run
        .ue(0) // mb_type: P_L0_16x16
        .ue(0) // mvd_l0 x (se 0)
        .ue(0) // mvd_l0 y (se 0)
        .ue(1) // coded_block_pattern: chroma DC only (inter codeNum 1)
        .ue(0) // mb_qp_delta (se 0)
        .ue(3) // mb_skip_run: the other three
        .nal(0x42);
    let mut out = vec![sps, pps, idr, part_a];
    if let Some(c) = part_c {
        let mut p = vec![0x44];
        p.extend_from_slice(c);
        out.push(p);
    }
    out
}

fn decode_all(nals: &[Vec<u8>]) -> Vec<crate::Picture> {
    let mut d = crate::Decoder::new(crate::Options {
        threads: 1,
        output: crate::OutputOrder::Display,
    });
    for n in nals {
        let _ = d.decode_nal(n, 0);
    }
    d.flush();
    std::iter::from_fn(|| d.next_picture(true)).collect()
}

/// Data partitioning: a P picture whose partition C was lost decodes from partition A alone
/// (no coefficients), and a damaged partition C neither panics nor stops the picture.
#[test]
fn data_partitions_lost_or_damaged() {
    let pics = decode_all(&partitioned_stream(None));
    assert_eq!(pics.len(), 2);
    assert_eq!(pics[0].planes, pics[1].planes, "zero motion, no residual");
    for c in [
        &[0x80u8][..],
        &[0x00, 0x00, 0x01],
        &[0x8f, 0xff, 0xff, 0xff, 0x00],
    ] {
        let pics = decode_all(&partitioned_stream(Some(c)));
        assert_eq!(pics.len(), 2);
        assert_eq!(
            pics[1].planes[0], pics[0].planes[0],
            "luma is not in partition C"
        );
    }
}
