//! Sequence and picture parameter sets, the VUI and the `avcC` record.

use crate::bits::{BitWriter, nal};

/// The H.264 profile of the stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Profile {
    /// Constrained Baseline (CAVLC, P pictures only): plays everywhere, largest files.
    Baseline,
    /// Main: CABAC, explicitly weighted prediction (fades) and B pictures (cross-fades).
    Main,
    /// High: Main plus the 8×8 transform. The smallest files; every current decoder.
    #[default]
    High,
}

impl Profile {
    /// (profile_idc, constraint flags byte).
    pub fn idc(self) -> (u8, u8) {
        match self {
            // constraint_set0 and constraint_set1: Constrained Baseline.
            Profile::Baseline => (66, 0b1100_0000),
            // constraint_set1: also conforms to Main.
            Profile::Main => (77, 0b0100_0000),
            Profile::High => (100, 0),
        }
    }

    /// CABAC, weighted prediction and B pictures.
    pub fn advanced(self) -> bool {
        self != Profile::Baseline
    }
}

/// frame_num is coded in this many bits.
pub const LOG2_MAX_FRAME_NUM: u32 = 8;

/// Colour description written into the VUI (and the MP4 `colr` box): H.273 code points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signal {
    pub primaries: u8,
    pub transfer: u8,
    pub matrix: u8,
    pub full_range: bool,
}

impl Signal {
    /// BT.709 primaries, transfer and matrix, limited (video) range.
    pub const BT709: Signal = Signal {
        primaries: 1,
        transfer: 1,
        matrix: 1,
        full_range: false,
    };
}

/// The fixed per-stream parameters.
#[derive(Clone, Debug)]
pub struct Params {
    pub width: u32,
    pub height: u32,
    pub mb_w: u32,
    pub mb_h: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub level_idc: u8,
    pub init_qp: u8,
    pub signal: Signal,
    pub profile: Profile,
    /// The SPS and PPS id.
    pub param_set_id: u8,
}

/// (level_idc, MaxMBPS, MaxFS) of Table A-1.
const LEVELS: [(u8, u64, u64); 16] = [
    (10, 1_485, 99),
    (11, 3_000, 396),
    (12, 6_000, 396),
    (13, 11_880, 396),
    (20, 11_880, 396),
    (21, 19_800, 792),
    (22, 20_250, 1_620),
    (30, 40_500, 1_620),
    (31, 108_000, 3_600),
    (32, 216_000, 5_120),
    (40, 245_760, 8_192),
    (41, 245_760, 8_192),
    (42, 522_240, 8_704),
    (50, 589_824, 22_080),
    (51, 983_040, 36_864),
    (52, 2_073_600, 36_864),
];

/// The lowest level whose macroblock rate, frame size and frame dimensions admit the stream.
/// 1080p and up get at least 4.1 (its 50 Mbit/s ceiling leaves room for constant-QP peaks).
pub fn pick_level(mb_w: u32, mb_h: u32, fps_num: u32, fps_den: u32) -> u8 {
    let fs = u64::from(mb_w) * u64::from(mb_h);
    let mbps = (fs * u64::from(fps_num)).div_ceil(u64::from(fps_den.max(1)));
    for &(idc, max_mbps, max_fs) in &LEVELS {
        let dim = ((max_fs * 8) as f64).sqrt() as u64;
        if mbps <= max_mbps && fs <= max_fs && u64::from(mb_w) <= dim && u64::from(mb_h) <= dim {
            return if fs >= 8_160 && idc < 41 { 41 } else { idc };
        }
    }
    // Beyond 5.2: level 6.2 is the largest Baseline can signal.
    62
}

impl Params {
    pub fn new(
        width: u32,
        height: u32,
        fps_num: u32,
        fps_den: u32,
        init_qp: u8,
        signal: Signal,
        profile: Profile,
    ) -> Params {
        let mb_w = width.div_ceil(16);
        let mb_h = height.div_ceil(16);
        Params {
            width,
            height,
            mb_w,
            mb_h,
            fps_num,
            fps_den,
            level_idc: pick_level(mb_w, mb_h, fps_num, fps_den),
            init_qp,
            signal,
            profile,
            param_set_id: 0,
        }
    }

    /// Reference pictures kept: the previous one, plus the long-term cross-fade anchor in
    /// Main and High.
    fn max_refs(&self) -> u32 {
        if self.profile.advanced() { 2 } else { 1 }
    }

    /// The SPS RBSP.
    pub fn sps_rbsp(&self) -> Vec<u8> {
        let mut w = BitWriter::new();
        let (idc, flags) = self.profile.idc();
        w.put(8, u32::from(idc));
        w.put(8, u32::from(flags));
        w.put(8, u32::from(self.level_idc));
        w.ue(u32::from(self.param_set_id)); // seq_parameter_set_id
        if self.profile == Profile::High {
            w.ue(1); // chroma_format_idc: 4:2:0
            w.ue(0); // bit_depth_luma_minus8
            w.ue(0); // bit_depth_chroma_minus8
            w.flag(false); // qpprime_y_zero_transform_bypass_flag
            w.flag(false); // seq_scaling_matrix_present_flag
        }
        w.ue(LOG2_MAX_FRAME_NUM - 4);
        w.ue(2); // pic_order_cnt_type: output order = decoding order
        w.ue(self.max_refs()); // max_num_ref_frames
        w.flag(false); // gaps_in_frame_num_value_allowed_flag
        w.ue(self.mb_w - 1);
        w.ue(self.mb_h - 1);
        w.flag(true); // frame_mbs_only_flag
        w.flag(true); // direct_8x8_inference_flag
        let crop_r = (self.mb_w * 16 - self.width) / 2;
        let crop_b = (self.mb_h * 16 - self.height) / 2;
        let crop = crop_r > 0 || crop_b > 0;
        w.flag(crop);
        if crop {
            w.ue(0);
            w.ue(crop_r);
            w.ue(0);
            w.ue(crop_b);
        }
        w.flag(true); // vui_parameters_present_flag
        self.vui(&mut w);
        w.trailing();
        w.into_bytes()
    }

    fn vui(&self, w: &mut BitWriter) {
        w.flag(true); // aspect_ratio_info_present_flag
        w.put(8, 1); // 1:1 (square samples)
        w.flag(false); // overscan_info_present_flag
        w.flag(true); // video_signal_type_present_flag
        w.put(3, 5); // video_format: unspecified
        w.flag(self.signal.full_range);
        w.flag(true); // colour_description_present_flag
        w.put(8, u32::from(self.signal.primaries));
        w.put(8, u32::from(self.signal.transfer));
        w.put(8, u32::from(self.signal.matrix));
        w.flag(false); // chroma_loc_info_present_flag (default: left, MPEG-2 style)
        w.flag(true); // timing_info_present_flag
        w.put(32, self.fps_den); // num_units_in_tick
        w.put(32, self.fps_num * 2); // time_scale (two ticks per frame)
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
        w.ue(self.max_refs()); // max_dec_frame_buffering
    }

    /// The PPS RBSP.
    pub fn pps_rbsp(&self) -> Vec<u8> {
        let mut w = BitWriter::new();
        w.ue(u32::from(self.param_set_id)); // pic_parameter_set_id
        w.ue(u32::from(self.param_set_id)); // seq_parameter_set_id
        let adv = self.profile.advanced();
        w.flag(adv); // entropy_coding_mode_flag: CABAC or CAVLC
        w.flag(false); // bottom_field_pic_order_in_frame_present_flag
        w.ue(0); // num_slice_groups_minus1
        w.ue(0); // num_ref_idx_l0_default_active_minus1
        w.ue(0); // num_ref_idx_l1_default_active_minus1
        w.flag(adv); // weighted_pred_flag
        w.put(2, u32::from(adv)); // weighted_bipred_idc: explicit
        w.se(i32::from(self.init_qp) - 26); // pic_init_qp_minus26
        w.se(0); // pic_init_qs_minus26
        w.se(0); // chroma_qp_index_offset
        w.flag(true); // deblocking_filter_control_present_flag
        w.flag(false); // constrained_intra_pred_flag
        w.flag(false); // redundant_pic_cnt_present_flag
        if self.profile == Profile::High {
            w.flag(true); // transform_8x8_mode_flag
            w.flag(false); // pic_scaling_matrix_present_flag
            w.se(0); // second_chroma_qp_index_offset
        }
        w.trailing();
        w.into_bytes()
    }

    /// The SPS NAL unit (header included, no start code or length).
    pub fn sps(&self) -> Vec<u8> {
        nal(3, 7, &self.sps_rbsp())
    }

    pub fn pps(&self) -> Vec<u8> {
        nal(3, 8, &self.pps_rbsp())
    }

    /// The AVCDecoderConfigurationRecord (ISO/IEC 14496-15) — the body of an `avcC` box.
    pub fn avcc(&self) -> Vec<u8> {
        let sps = self.sps();
        let pps = self.pps();
        let (idc, flags) = self.profile.idc();
        let mut v = vec![
            1,
            idc,
            flags,
            self.level_idc,
            0xFF, // lengthSizeMinusOne = 3
            0xE1, // one SPS
        ];
        v.extend_from_slice(&(sps.len() as u16).to_be_bytes());
        v.extend_from_slice(&sps);
        v.push(1);
        v.extend_from_slice(&(pps.len() as u16).to_be_bytes());
        v.extend_from_slice(&pps);
        if idc == 100 {
            // High profile extension: chroma_format 4:2:0, 8-bit, no SPS extensions.
            v.extend_from_slice(&[0xFC | 1, 0xF8, 0xF8, 0]);
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        // 1280×720 = 80×45 MBs
        assert_eq!(pick_level(80, 45, 30, 1), 31);
        assert_eq!(pick_level(80, 45, 60, 1), 32);
        assert_eq!(pick_level(120, 68, 30, 1), 41);
        assert_eq!(pick_level(120, 68, 60, 1), 42);
        assert_eq!(pick_level(240, 135, 30, 1), 51);
        assert_eq!(pick_level(240, 135, 60, 1), 52);
    }
}
