//! The SPS, PPS and slice headers handed to the VA-API encoder as packed headers (Mesa writes
//! the NAL unit headers and parameter sets only from these). They must agree with the
//! parameter buffers: the driver takes the picture-level values from the buffers and the
//! header bits from here.

use crate::encode::EncodeProfile;

/// An MSB-first bit writer.
#[derive(Default)]
pub struct Bits {
    bytes: Vec<u8>,
    cur: u64,
    n: u32,
}

impl Bits {
    pub fn put(&mut self, bits: u32, v: u32) {
        debug_assert!(bits <= 32);
        if bits == 0 {
            return;
        }
        let v = u64::from(v) & ((1u64 << bits) - 1);
        self.cur = self.cur << bits | v;
        self.n += bits;
        while self.n >= 8 {
            self.n -= 8;
            self.bytes.push((self.cur >> self.n) as u8);
        }
        self.cur &= (1u64 << self.n) - 1;
    }

    pub fn flag(&mut self, b: bool) {
        self.put(1, u32::from(b));
    }

    pub fn ue(&mut self, v: u32) {
        let x = u64::from(v) + 1;
        let len = 64 - x.leading_zeros();
        self.put(len - 1, 0);
        self.put(len, x as u32);
    }

    pub fn se(&mut self, v: i32) {
        self.ue(if v > 0 {
            (2 * v - 1) as u32
        } else {
            (-2 * v) as u32
        });
    }

    /// rbsp_trailing_bits().
    pub fn trailing(&mut self) {
        self.put(1, 1);
        if self.n > 0 {
            self.put(8 - self.n, 0);
        }
    }

    /// The whole bytes, the pending bits left-aligned in a last byte, and the bit count.
    pub fn finish(mut self) -> (Vec<u8>, u32) {
        let bits = self.bytes.len() as u32 * 8 + self.n;
        if self.n > 0 {
            self.bytes.push((self.cur << (8 - self.n)) as u8);
        }
        (self.bytes, bits)
    }
}

/// A start code, the NAL header byte and `payload` with emulation prevention; when the
/// payload ends mid-byte (`bits` not a multiple of 8) its last byte is copied as is. Returns
/// the bytes and their length in bits.
pub fn nal(header: u8, payload: &[u8], bits: u32) -> (Vec<u8>, u32) {
    let mut v = vec![0, 0, 0, 1, header];
    let whole = (bits / 8) as usize;
    let mut zeros = 0;
    for &b in &payload[..whole] {
        if zeros >= 2 && b <= 3 {
            v.push(3);
            zeros = 0;
        }
        v.push(b);
        zeros = if b == 0 { zeros + 1 } else { 0 };
    }
    let mut len = v.len() as u32 * 8;
    if !bits.is_multiple_of(8) {
        v.push(payload[whole]);
        len += bits % 8;
    }
    (v, len)
}

/// What the headers describe.
pub struct Stream {
    pub profile: EncodeProfile,
    pub level_idc: u8,
    pub mb_w: u32,
    pub mb_h: u32,
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub log2_frame_num: u32,
    pub log2_poc: u32,
    pub init_qp: u8,
}

impl Stream {
    pub fn sps_rbsp(&self) -> Vec<u8> {
        let mut w = Bits::default();
        let high = self.profile == EncodeProfile::High;
        w.put(8, if high { 100 } else { 77 });
        // constraint_set1_flag for Main (it is also a valid constrained High stream subset).
        w.put(8, if high { 0 } else { 0x40 });
        w.put(8, u32::from(self.level_idc));
        w.ue(0); // seq_parameter_set_id
        if high {
            w.ue(1); // chroma_format_idc 4:2:0
            w.ue(0); // bit_depth_luma_minus8
            w.ue(0); // bit_depth_chroma_minus8
            w.flag(false); // qpprime_y_zero_transform_bypass_flag
            w.flag(false); // seq_scaling_matrix_present_flag
        }
        w.ue(self.log2_frame_num - 4);
        w.ue(0); // pic_order_cnt_type
        w.ue(self.log2_poc - 4);
        w.ue(1); // max_num_ref_frames
        w.flag(false); // gaps_in_frame_num_value_allowed_flag
        w.ue(self.mb_w - 1);
        w.ue(self.mb_h - 1);
        w.flag(true); // frame_mbs_only_flag
        w.flag(true); // direct_8x8_inference_flag
        let (cr, cb) = (
            (self.mb_w * 16 - self.width) / 2,
            (self.mb_h * 16 - self.height) / 2,
        );
        w.flag(cr > 0 || cb > 0);
        if cr > 0 || cb > 0 {
            w.ue(0);
            w.ue(cr);
            w.ue(0);
            w.ue(cb);
        }
        w.flag(true); // vui_parameters_present_flag
        w.flag(false); // aspect_ratio_info_present_flag
        w.flag(false); // overscan_info_present_flag
        w.flag(true); // video_signal_type_present_flag
        w.put(3, 5); // video_format: unspecified
        w.flag(false); // video_full_range_flag: limited
        w.flag(true); // colour_description_present_flag
        w.put(8, 1); // BT.709 primaries
        w.put(8, 1); // BT.709 transfer
        w.put(8, 1); // BT.709 matrix
        w.flag(false); // chroma_loc_info_present_flag
        w.flag(true); // timing_info_present_flag
        w.put(32, self.fps_den);
        w.put(32, self.fps_num * 2);
        w.flag(true); // fixed_frame_rate_flag
        w.flag(false); // nal_hrd_parameters_present_flag
        w.flag(false); // vcl_hrd_parameters_present_flag
        w.flag(false); // pic_struct_present_flag
        w.flag(true); // bitstream_restriction_flag
        w.flag(true); // motion_vectors_over_pic_boundaries_flag
        w.ue(0); // max_bytes_per_pic_denom
        w.ue(0); // max_bits_per_mb_denom
        w.ue(15); // log2_max_mv_length_horizontal
        w.ue(15); // log2_max_mv_length_vertical
        w.ue(0); // max_num_reorder_frames
        w.ue(1); // max_dec_frame_buffering
        w.trailing();
        w.finish().0
    }

    pub fn pps_rbsp(&self) -> Vec<u8> {
        let mut w = Bits::default();
        w.ue(0); // pic_parameter_set_id
        w.ue(0); // seq_parameter_set_id
        w.flag(true); // entropy_coding_mode_flag: CABAC
        w.flag(false); // bottom_field_pic_order_in_frame_present_flag
        w.ue(0); // num_slice_groups_minus1
        w.ue(0); // num_ref_idx_l0_default_active_minus1
        w.ue(0); // num_ref_idx_l1_default_active_minus1
        w.flag(false); // weighted_pred_flag
        w.put(2, 0); // weighted_bipred_idc
        w.se(i32::from(self.init_qp) - 26);
        w.se(0); // pic_init_qs_minus26
        w.se(0); // chroma_qp_index_offset
        w.flag(true); // deblocking_filter_control_present_flag
        w.flag(false); // constrained_intra_pred_flag
        w.flag(false); // redundant_pic_cnt_present_flag
        if self.profile == EncodeProfile::High {
            w.flag(true); // transform_8x8_mode_flag
            w.flag(false); // pic_scaling_matrix_present_flag
            w.se(0); // second_chroma_qp_index_offset
        }
        w.trailing();
        w.finish().0
    }

    /// The SPS and PPS NAL units in Annex B, and their length in bits.
    pub fn parameter_sets(&self) -> (Vec<u8>, u32) {
        let sps = self.sps_rbsp();
        let pps = self.pps_rbsp();
        let (mut v, _) = nal(0x67, &sps, sps.len() as u32 * 8);
        let (p, _) = nal(0x68, &pps, pps.len() as u32 * 8);
        v.extend_from_slice(&p);
        let bits = v.len() as u32 * 8;
        (v, bits)
    }

    /// A slice header (start code and NAL header included; the driver appends the slice
    /// data), and its length in bits.
    pub fn slice_header(
        &self,
        idr: bool,
        frame_num: u32,
        idr_pic_id: u32,
        poc_lsb: u32,
        qp_delta: i32,
    ) -> (Vec<u8>, u32) {
        let mut w = Bits::default();
        w.ue(0); // first_mb_in_slice
        w.ue(if idr { 7 } else { 5 }); // slice_type: I or P, every slice of the picture
        w.ue(0); // pic_parameter_set_id
        w.put(self.log2_frame_num, frame_num);
        if idr {
            w.ue(idr_pic_id);
        }
        w.put(self.log2_poc, poc_lsb);
        if !idr {
            w.flag(false); // num_ref_idx_active_override_flag
            w.flag(false); // ref_pic_list_modification_flag_l0
        }
        // dec_ref_pic_marking() (nal_ref_idc is 3).
        if idr {
            w.flag(false); // no_output_of_prior_pics_flag
            w.flag(false); // long_term_reference_flag
        } else {
            w.flag(false); // adaptive_ref_pic_marking_mode_flag
            w.ue(0); // cabac_init_idc
        }
        w.se(qp_delta);
        w.ue(0); // disable_deblocking_filter_idc
        w.se(0); // slice_alpha_c0_offset_div2
        w.se(0); // slice_beta_offset_div2
        let (bytes, bits) = w.finish();
        nal(if idr { 0x65 } else { 0x61 }, &bytes, bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_golomb() {
        let mut w = Bits::default();
        w.ue(0);
        w.ue(1);
        w.ue(2);
        w.se(-1);
        w.se(1);
        let (b, n) = w.finish();
        // 1 010 011 011 010
        assert_eq!(n, 13);
        assert_eq!(b, vec![0b1010_0110, 0b1101_0000]);
    }

    #[test]
    fn emulation_prevention() {
        let (v, bits) = nal(0x67, &[0, 0, 1, 0, 0, 0, 5], 56);
        assert_eq!(v, vec![0, 0, 0, 1, 0x67, 0, 0, 3, 1, 0, 0, 3, 0, 5]);
        assert_eq!(bits, v.len() as u32 * 8);
        let (v, bits) = nal(0x65, &[0x88, 0x80], 9);
        assert_eq!(v, vec![0, 0, 0, 1, 0x65, 0x88, 0x80]);
        assert_eq!(bits, 6 * 8 + 1);
    }
}
