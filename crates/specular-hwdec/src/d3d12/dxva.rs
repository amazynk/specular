//! The DXVA structures D3D12 Video Decode takes (`dxva.h`, byte-packed), filled from the
//! VA-API-layout parameters [`crate::hevc`], [`crate::hevc_video`] and [`crate::h264`] build:
//! the two carry the same syntax elements, so the parsers, their checks and their tests are
//! shared, and only the bit positions differ. The VA-API positions are those of
//! `va_dec_hevc.h` / `va.h` (see `crate::hevc::pic_param` and `crate::h264`); the DXVA ones are
//! read off `dxva.h` (Windows SDK 10.0.26100). DXVA's "short" slice format is used: the driver
//! parses the slice headers itself, so the slice parameters only locate the NAL units.

use std::ffi::c_void;

use crate::params::{
    INVALID_ID, IqMatrixH264, IqMatrixHevc, PICTURE_H264_BOTTOM_FIELD, PICTURE_H264_INVALID,
    PICTURE_H264_LONG_TERM_REFERENCE, PICTURE_H264_TOP_FIELD, PICTURE_HEVC_INVALID,
    PICTURE_HEVC_LONG_TERM_REFERENCE, PICTURE_HEVC_RPS_LT_CURR, PICTURE_HEVC_RPS_ST_CURR_AFTER,
    PICTURE_HEVC_RPS_ST_CURR_BEFORE, PicParamH264, PicParamHevc, SliceParamH264,
};

/// A parameter struct as D3D12 takes it: its address and size.
pub trait Blob {
    fn blob(&mut self) -> (*mut c_void, u32);
}

macro_rules! blob {
    ($t:ty) => {
        impl Blob for $t {
            fn blob(&mut self) -> (*mut c_void, u32) {
                ((self as *mut $t).cast(), size_of::<$t>() as u32)
            }
        }
    };
}

// ------------------------------------------------------------------------------------- HEVC

/// `DXVA_PicParams_HEVC`. The bit-field unions are the plain integers `format_and_sequence`,
/// `coding_tools` and `picture_properties` (LSB first, as MSVC lays out bit-fields).
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PicParams {
    pub pic_width_in_min_cbs_y: u16,
    pub pic_height_in_min_cbs_y: u16,
    pub format_and_sequence: u16,
    /// `DXVA_PicEntry_HEVC`: index (7 bits) and the "associated" flag (bit 7).
    pub curr_pic: u8,
    pub sps_max_dec_pic_buffering_minus1: u8,
    pub log2_min_luma_coding_block_size_minus3: u8,
    pub log2_diff_max_min_luma_coding_block_size: u8,
    pub log2_min_transform_block_size_minus2: u8,
    pub log2_diff_max_min_transform_block_size: u8,
    pub max_transform_hierarchy_depth_inter: u8,
    pub max_transform_hierarchy_depth_intra: u8,
    pub num_short_term_ref_pic_sets: u8,
    pub num_long_term_ref_pics_sps: u8,
    pub num_ref_idx_l0_default_active_minus1: u8,
    pub num_ref_idx_l1_default_active_minus1: u8,
    pub init_qp_minus26: i8,
    pub num_delta_pocs_of_ref_rps_idx: u8,
    pub num_bits_for_short_term_rps_in_slice: u16,
    pub reserved2: u16,
    pub coding_tools: u32,
    pub picture_properties: u32,
    pub pps_cb_qp_offset: i8,
    pub pps_cr_qp_offset: i8,
    pub num_tile_columns_minus1: u8,
    pub num_tile_rows_minus1: u8,
    pub column_width_minus1: [u16; 19],
    pub row_height_minus1: [u16; 21],
    pub diff_cu_qp_delta_depth: u8,
    pub pps_beta_offset_div2: i8,
    pub pps_tc_offset_div2: i8,
    pub log2_parallel_merge_level_minus2: u8,
    pub curr_pic_order_cnt_val: i32,
    /// `DXVA_PicEntry_HEVC` each: index, long-term flag in bit 7; 0xff empty.
    pub ref_pic_list: [u8; 15],
    pub reserved5: u8,
    pub pic_order_cnt_val_list: [i32; 15],
    /// Indices into `ref_pic_list` (0xff empty).
    pub ref_pic_set_st_curr_before: [u8; 8],
    pub ref_pic_set_st_curr_after: [u8; 8],
    pub ref_pic_set_lt_curr: [u8; 8],
    pub reserved6: u16,
    pub reserved7: u16,
    pub status_report_feedback_number: u32,
}
blob!(PicParams);

/// `DXVA_Qmatrix_HEVC`: the same lists as VA-API's `VAIQMatrixBufferHEVC`, in the coded
/// (up-right diagonal) order of the HEVC syntax where VA-API (Mesa) takes raster order: with
/// raster order the AMD driver dequantised every 8×8 and larger block wrongly (luma off by up
/// to 34 on a 24 MP iPhone grid; the 4×4 lists are flat, so most chroma stayed right).
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct Qmatrix {
    pub lists_4x4: [[u8; 16]; 6],
    pub lists_8x8: [[u8; 64]; 6],
    pub lists_16x16: [[u8; 64]; 6],
    pub lists_32x32: [[u8; 64]; 2],
    pub dc_16x16: [u8; 6],
    pub dc_32x32: [u8; 2],
}
blob!(Qmatrix);

/// `DXVA_Slice_HEVC_Short` and `DXVA_Slice_H264_Short` (the same layout): where a slice's NAL
/// unit (with its start code) lies in the bitstream buffer.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SliceShort {
    pub nal_unit_data_location: u32,
    pub slice_bytes_in_buffer: u32,
    pub bad_slice_chopping: u16,
}

// dxva.h: `#pragma pack(push, BeforeDXVApacking, 1)`.
const _: () = assert!(size_of::<PicParams>() == 232);
const _: () = assert!(std::mem::offset_of!(PicParams, coding_tools) == 24);
const _: () = assert!(std::mem::offset_of!(PicParams, column_width_minus1) == 36);
const _: () = assert!(std::mem::offset_of!(PicParams, curr_pic_order_cnt_val) == 120);
const _: () = assert!(std::mem::offset_of!(PicParams, status_report_feedback_number) == 228);
const _: () = assert!(size_of::<Qmatrix>() == 1000);
const _: () = assert!(size_of::<SliceShort>() == 10);

/// DXVA pads every bitstream buffer to a multiple of this.
pub const BITSTREAM_ALIGN: usize = 128;

fn bit(v: u32, n: u32) -> u32 {
    (v >> n) & 1
}

/// A VA picture id (a pool slot here; [`INVALID_ID`] for none) as a DXVA picture entry.
fn entry(id: u32) -> u8 {
    if id == INVALID_ID {
        0xff
    } else {
        (id & 0x7f) as u8
    }
}

/// The picture parameters of `p` (VA-API layout) for DXVA. `uniform_spacing` is the PPS
/// `uniform_spacing_flag`, which VA-API does not carry; `num_delta_pocs_of_ref_rps_idx` the
/// `NumDeltaPocs` of the SPS set a slice RPS is predicted from; `status` the status report
/// number (nonzero, one per submission). Picture ids are pool slots: the current picture's
/// slot indexes the reference-frame list handed to D3D12 (0 for a still picture, which is
/// that list's one entry), and each reference's slot its entry.
pub fn pic_params(
    p: &PicParamHevc,
    uniform_spacing: bool,
    num_delta_pocs_of_ref_rps_idx: u8,
    status: u32,
) -> PicParams {
    let (pf, sp) = (p.pic_fields, p.slice_parsing_fields);
    let min_cb = 3 + u32::from(p.log2_min_luma_coding_block_size_minus3);
    let format_and_sequence = (pf & 3)
        | bit(pf, 2) << 2
        | (u32::from(p.bit_depth_luma_minus8) & 7) << 3
        | (u32::from(p.bit_depth_chroma_minus8) & 7) << 6
        | (u32::from(p.log2_max_pic_order_cnt_lsb_minus4) & 15) << 9
        | bit(pf, 19) << 13;
    let coding_tools = bit(pf, 4)
        | bit(pf, 6) << 1
        | bit(sp, 7) << 2
        | bit(pf, 3) << 3
        | (u32::from(p.pcm_sample_bit_depth_luma_minus1) & 15) << 4
        | (u32::from(p.pcm_sample_bit_depth_chroma_minus1) & 15) << 8
        | (u32::from(p.log2_min_pcm_luma_coding_block_size_minus3) & 3) << 12
        | (u32::from(p.log2_diff_max_min_pcm_luma_coding_block_size) & 3) << 14
        | bit(pf, 18) << 16
        | bit(sp, 1) << 17
        | bit(sp, 2) << 18
        | bit(pf, 7) << 19
        | bit(sp, 5) << 20
        | bit(sp, 4) << 21
        | (u32::from(p.num_extra_slice_header_bits) & 7) << 22
        | bit(pf, 8) << 25
        | bit(sp, 3) << 26;
    let picture_properties = bit(pf, 9)
        | bit(pf, 5) << 1
        | bit(pf, 10) << 2
        | bit(sp, 6) << 3
        | bit(pf, 11) << 4
        | bit(pf, 12) << 5
        | bit(pf, 13) << 6
        | bit(pf, 14) << 7
        | bit(pf, 15) << 8
        | u32::from(uniform_spacing) << 9
        | bit(pf, 17) << 10
        | bit(pf, 16) << 11
        | bit(sp, 8) << 12
        | bit(sp, 9) << 13
        | bit(sp, 0) << 14
        | bit(sp, 10) << 15
        | bit(sp, 11) << 16
        | bit(sp, 12) << 17
        | bit(sp, 13) << 18;
    let mut ref_pic_list = [0xffu8; 15];
    let mut pic_order_cnt_val_list = [0i32; 15];
    let mut sets = [[0xffu8; 8]; 3];
    let mut counts = [0usize; 3];
    for (i, r) in p.reference_frames.iter().enumerate() {
        if r.picture_id == INVALID_ID || r.flags & PICTURE_HEVC_INVALID != 0 {
            continue;
        }
        let long_term = r.flags & PICTURE_HEVC_LONG_TERM_REFERENCE != 0;
        ref_pic_list[i] = entry(r.picture_id) | if long_term { 0x80 } else { 0 };
        pic_order_cnt_val_list[i] = r.pic_order_cnt;
        let set = if r.flags & PICTURE_HEVC_RPS_ST_CURR_BEFORE != 0 {
            0
        } else if r.flags & PICTURE_HEVC_RPS_ST_CURR_AFTER != 0 {
            1
        } else if r.flags & PICTURE_HEVC_RPS_LT_CURR != 0 {
            2
        } else {
            continue;
        };
        if counts[set] < 8 {
            sets[set][counts[set]] = i as u8;
            counts[set] += 1;
        }
    }
    PicParams {
        pic_width_in_min_cbs_y: (u32::from(p.pic_width_in_luma_samples) >> min_cb) as u16,
        pic_height_in_min_cbs_y: (u32::from(p.pic_height_in_luma_samples) >> min_cb) as u16,
        format_and_sequence: format_and_sequence as u16,
        curr_pic: if p.curr_pic.picture_id == INVALID_ID {
            0
        } else {
            entry(p.curr_pic.picture_id)
        },
        sps_max_dec_pic_buffering_minus1: p.sps_max_dec_pic_buffering_minus1,
        log2_min_luma_coding_block_size_minus3: p.log2_min_luma_coding_block_size_minus3,
        log2_diff_max_min_luma_coding_block_size: p.log2_diff_max_min_luma_coding_block_size,
        log2_min_transform_block_size_minus2: p.log2_min_transform_block_size_minus2,
        log2_diff_max_min_transform_block_size: p.log2_diff_max_min_transform_block_size,
        max_transform_hierarchy_depth_inter: p.max_transform_hierarchy_depth_inter,
        max_transform_hierarchy_depth_intra: p.max_transform_hierarchy_depth_intra,
        num_short_term_ref_pic_sets: p.num_short_term_ref_pic_sets,
        num_long_term_ref_pics_sps: p.num_long_term_ref_pic_sps,
        num_ref_idx_l0_default_active_minus1: p.num_ref_idx_l0_default_active_minus1,
        num_ref_idx_l1_default_active_minus1: p.num_ref_idx_l1_default_active_minus1,
        init_qp_minus26: p.init_qp_minus26,
        num_delta_pocs_of_ref_rps_idx,
        num_bits_for_short_term_rps_in_slice: p.st_rps_bits.min(u32::from(u16::MAX)) as u16,
        reserved2: 0,
        coding_tools,
        picture_properties,
        pps_cb_qp_offset: p.pps_cb_qp_offset,
        pps_cr_qp_offset: p.pps_cr_qp_offset,
        num_tile_columns_minus1: p.num_tile_columns_minus1,
        num_tile_rows_minus1: p.num_tile_rows_minus1,
        column_width_minus1: p.column_width_minus1,
        row_height_minus1: p.row_height_minus1,
        diff_cu_qp_delta_depth: p.diff_cu_qp_delta_depth,
        pps_beta_offset_div2: p.pps_beta_offset_div2,
        pps_tc_offset_div2: p.pps_tc_offset_div2,
        log2_parallel_merge_level_minus2: p.log2_parallel_merge_level_minus2,
        curr_pic_order_cnt_val: p.curr_pic.pic_order_cnt,
        ref_pic_list,
        reserved5: 0,
        pic_order_cnt_val_list,
        ref_pic_set_st_curr_before: sets[0],
        ref_pic_set_st_curr_after: sets[1],
        ref_pic_set_lt_curr: sets[2],
        reserved6: 0,
        reserved7: 0,
        status_report_feedback_number: status,
    }
}

/// The VA-API matrix (raster order) in DXVA's coded order.
pub fn qmatrix(m: &IqMatrixHevc) -> Qmatrix {
    let (d4, d8) = (crate::hevc::diag_scan(4), crate::hevc::diag_scan(8));
    let coded =
        |raster: &[u8], scan: &[usize]| -> Vec<u8> { scan.iter().map(|&r| raster[r]).collect() };
    let mut q = Qmatrix {
        lists_4x4: [[0; 16]; 6],
        lists_8x8: [[0; 64]; 6],
        lists_16x16: [[0; 64]; 6],
        lists_32x32: [[0; 64]; 2],
        dc_16x16: m.scaling_list_dc_16x16,
        dc_32x32: m.scaling_list_dc_32x32,
    };
    for i in 0..6 {
        q.lists_4x4[i].copy_from_slice(&coded(&m.scaling_list_4x4[i], &d4));
        q.lists_8x8[i].copy_from_slice(&coded(&m.scaling_list_8x8[i], &d8));
        q.lists_16x16[i].copy_from_slice(&coded(&m.scaling_list_16x16[i], &d8));
    }
    for i in 0..2 {
        q.lists_32x32[i].copy_from_slice(&coded(&m.scaling_list_32x32[i], &d8));
    }
    q
}

/// The bitstream buffer and slice controls: every slice NAL unit behind a 3-byte start code,
/// in order, zero-padded to a multiple of [`BITSTREAM_ALIGN`] bytes (counted into the last
/// slice, as DXVA's own decoders expect).
pub fn bitstream<S: Copy>(slices: &[(S, &[u8])]) -> (Vec<u8>, Vec<SliceShort>) {
    let total: usize = slices.iter().map(|(_, nal)| 3 + nal.len()).sum();
    let mut buf = Vec::with_capacity(total.next_multiple_of(BITSTREAM_ALIGN));
    let mut ctl = Vec::with_capacity(slices.len());
    for (_, nal) in slices {
        let at = buf.len();
        buf.extend_from_slice(&[0, 0, 1]);
        buf.extend_from_slice(nal);
        ctl.push(SliceShort {
            nal_unit_data_location: at as u32,
            slice_bytes_in_buffer: (buf.len() - at) as u32,
            bad_slice_chopping: 0,
        });
    }
    let padded = buf.len().next_multiple_of(BITSTREAM_ALIGN);
    if let Some(last) = ctl.last_mut() {
        last.slice_bytes_in_buffer += (padded - buf.len()) as u32;
    }
    buf.resize(padded, 0);
    (buf, ctl)
}

// ------------------------------------------------------------------------------------ H.264

/// `DXVA_PicParams_H264`; `bit_fields` is the `wBitFields` union (LSB first).
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct PicParamsH264 {
    pub frame_width_in_mbs_minus1: u16,
    pub frame_height_in_mbs_minus1: u16,
    /// `DXVA_PicEntry_H264`: index, bottom-field flag in bit 7.
    pub curr_pic: u8,
    pub num_ref_frames: u8,
    pub bit_fields: u16,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub reserved16: u16,
    pub status_report_feedback_number: u32,
    /// `DXVA_PicEntry_H264` each: index, long-term flag in bit 7; 0xff empty.
    pub ref_frame_list: [u8; 16],
    pub curr_field_order_cnt: [i32; 2],
    pub field_order_cnt_list: [[i32; 2]; 16],
    pub pic_init_qs_minus26: i8,
    pub chroma_qp_index_offset: i8,
    pub second_chroma_qp_index_offset: i8,
    pub continuation_flag: u8,
    pub pic_init_qp_minus26: i8,
    pub num_ref_idx_l0_active_minus1: u8,
    pub num_ref_idx_l1_active_minus1: u8,
    pub reserved8a: u8,
    /// `frame_num` of each reference, or `LongTermFrameIdx` of a long-term one.
    pub frame_num_list: [u16; 16],
    /// Bits 2i (top) and 2i+1 (bottom): the fields of entry i used for reference.
    pub used_for_reference_flags: u32,
    pub non_existing_frame_flags: u16,
    pub frame_num: u16,
    pub log2_max_frame_num_minus4: u8,
    pub pic_order_cnt_type: u8,
    pub log2_max_pic_order_cnt_lsb_minus4: u8,
    pub delta_pic_order_always_zero_flag: u8,
    pub direct_8x8_inference_flag: u8,
    pub entropy_coding_mode_flag: u8,
    pub pic_order_present_flag: u8,
    pub num_slice_groups_minus1: u8,
    pub slice_group_map_type: u8,
    pub deblocking_filter_control_present_flag: u8,
    pub redundant_pic_cnt_present_flag: u8,
    pub reserved8b: u8,
    pub slice_group_change_rate_minus1: u16,
    pub slice_group_map: [u8; 810],
}
blob!(PicParamsH264);

/// `DXVA_Qmatrix_H264`, in zig-zag (coded) order like the HEVC one.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct QmatrixH264 {
    pub lists_4x4: [[u8; 16]; 6],
    pub lists_8x8: [[u8; 64]; 2],
}
blob!(QmatrixH264);

const _: () = assert!(size_of::<PicParamsH264>() == 1040);
const _: () = assert!(std::mem::offset_of!(PicParamsH264, ref_frame_list) == 16);
const _: () = assert!(std::mem::offset_of!(PicParamsH264, field_order_cnt_list) == 40);
const _: () = assert!(std::mem::offset_of!(PicParamsH264, pic_init_qp_minus26) == 172);
const _: () = assert!(std::mem::offset_of!(PicParamsH264, frame_num_list) == 176);
const _: () = assert!(std::mem::offset_of!(PicParamsH264, frame_num) == 214);
const _: () = assert!(std::mem::offset_of!(PicParamsH264, slice_group_map) == 230);
const _: () = assert!(size_of::<QmatrixH264>() == 224);

/// The H.264 picture parameters of `p` (VA-API layout, `va.h`'s `seq_fields` and `pic_fields`
/// bit positions) for DXVA. `first` is the picture's first slice (the active reference
/// counts), `intra` whether every slice is an I slice. Picture ids are pool slots, as for HEVC.
pub fn pic_params_h264(
    p: &PicParamH264,
    first: &SliceParamH264,
    intra: bool,
    status: u32,
) -> PicParamsH264 {
    let (sf, pf) = (p.seq_fields, p.pic_fields);
    let field_pic = bit(pf, 5);
    let bit_fields = field_pic
        | (bit(sf, 5) & (1 - field_pic)) << 1
        | bit(sf, 2) << 2
        | (sf & 3) << 4
        | bit(pf, 10) << 6
        | bit(pf, 6) << 7
        | bit(pf, 1) << 8
        | ((pf >> 2) & 3) << 9
        | 1 << 11
        | bit(sf, 4) << 12
        | bit(pf, 4) << 13
        | bit(sf, 7) << 14
        | u32::from(intra) << 15;
    let mut ref_frame_list = [0xffu8; 16];
    let mut field_order_cnt_list = [[0i32; 2]; 16];
    let mut frame_num_list = [0u16; 16];
    let mut used = 0u32;
    for (i, r) in p.reference_frames.iter().enumerate() {
        if r.picture_id == INVALID_ID || r.flags & PICTURE_H264_INVALID != 0 {
            continue;
        }
        let long_term = r.flags & PICTURE_H264_LONG_TERM_REFERENCE != 0;
        ref_frame_list[i] = entry(r.picture_id) | if long_term { 0x80 } else { 0 };
        field_order_cnt_list[i] = [r.top_field_order_cnt, r.bottom_field_order_cnt];
        frame_num_list[i] = r.frame_idx.min(u32::from(u16::MAX)) as u16;
        let fields = r.flags & (PICTURE_H264_TOP_FIELD | PICTURE_H264_BOTTOM_FIELD);
        if fields == 0 || fields & PICTURE_H264_TOP_FIELD != 0 {
            used |= 1 << (2 * i);
        }
        if fields == 0 || fields & PICTURE_H264_BOTTOM_FIELD != 0 {
            used |= 1 << (2 * i + 1);
        }
    }
    let bottom = p.curr_pic.flags & PICTURE_H264_BOTTOM_FIELD != 0;
    PicParamsH264 {
        frame_width_in_mbs_minus1: p.picture_width_in_mbs_minus1,
        frame_height_in_mbs_minus1: p.picture_height_in_mbs_minus1,
        curr_pic: entry(p.curr_pic.picture_id) & 0x7f | if bottom { 0x80 } else { 0 },
        num_ref_frames: p.num_ref_frames,
        bit_fields: bit_fields as u16,
        bit_depth_luma_minus8: p.bit_depth_luma_minus8,
        bit_depth_chroma_minus8: p.bit_depth_chroma_minus8,
        reserved16: 0,
        status_report_feedback_number: status,
        ref_frame_list,
        curr_field_order_cnt: [
            p.curr_pic.top_field_order_cnt,
            p.curr_pic.bottom_field_order_cnt,
        ],
        field_order_cnt_list,
        pic_init_qs_minus26: p.pic_init_qs_minus26,
        chroma_qp_index_offset: p.chroma_qp_index_offset,
        second_chroma_qp_index_offset: p.second_chroma_qp_index_offset,
        continuation_flag: 1,
        pic_init_qp_minus26: p.pic_init_qp_minus26,
        num_ref_idx_l0_active_minus1: first.num_ref_idx_l0_active_minus1,
        num_ref_idx_l1_active_minus1: first.num_ref_idx_l1_active_minus1,
        reserved8a: 0,
        frame_num_list,
        used_for_reference_flags: used,
        non_existing_frame_flags: 0,
        frame_num: p.frame_num,
        log2_max_frame_num_minus4: ((sf >> 8) & 15) as u8,
        pic_order_cnt_type: ((sf >> 12) & 3) as u8,
        log2_max_pic_order_cnt_lsb_minus4: ((sf >> 14) & 15) as u8,
        delta_pic_order_always_zero_flag: bit(sf, 18) as u8,
        direct_8x8_inference_flag: bit(sf, 6) as u8,
        entropy_coding_mode_flag: bit(pf, 0) as u8,
        pic_order_present_flag: bit(pf, 7) as u8,
        num_slice_groups_minus1: p.num_slice_groups_minus1,
        slice_group_map_type: p.slice_group_map_type,
        deblocking_filter_control_present_flag: bit(pf, 8) as u8,
        redundant_pic_cnt_present_flag: bit(pf, 9) as u8,
        reserved8b: 0,
        slice_group_change_rate_minus1: p.slice_group_change_rate_minus1,
        slice_group_map: [0; 810],
    }
}

/// The VA-API H.264 matrices (raster order) in DXVA's zig-zag order.
pub fn qmatrix_h264(m: &IqMatrixH264) -> QmatrixH264 {
    use crate::h264::{ZIGZAG_4X4, ZIGZAG_8X8};
    let mut q = QmatrixH264 {
        lists_4x4: [[0; 16]; 6],
        lists_8x8: [[0; 64]; 2],
    };
    for (out, raster) in q.lists_4x4.iter_mut().zip(&m.scaling_list_4x4) {
        for (i, &r) in ZIGZAG_4X4.iter().enumerate() {
            out[i] = raster[r];
        }
    }
    for (out, raster) in q.lists_8x8.iter_mut().zip(&m.scaling_list_8x8) {
        for (i, &r) in ZIGZAG_8X8.iter().enumerate() {
            out[i] = raster[r];
        }
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{
        PICTURE_H264_SHORT_TERM_REFERENCE, PictureH264, PictureHevc, SLICE_DATA_FLAG_ALL,
        SliceParamHevc,
    };

    fn va() -> PicParamHevc {
        PicParamHevc {
            curr_pic: PictureHevc {
                pic_order_cnt: 7,
                ..PictureHevc::INVALID
            },
            reference_frames: [PictureHevc::INVALID; 15],
            pic_width_in_luma_samples: 4032,
            pic_height_in_luma_samples: 3024,
            // 4:2:0, scaling lists (4), amp (6), strong intra smoothing (7), tiles (14),
            // loop filter across tiles (17), no reordering (19).
            pic_fields: 1 | 1 << 4 | 1 << 6 | 1 << 7 | 1 << 14 | 1 << 17 | 1 << 19,
            sps_max_dec_pic_buffering_minus1: 0,
            bit_depth_luma_minus8: 2,
            bit_depth_chroma_minus8: 2,
            pcm_sample_bit_depth_luma_minus1: 0,
            pcm_sample_bit_depth_chroma_minus1: 0,
            log2_min_luma_coding_block_size_minus3: 0,
            log2_diff_max_min_luma_coding_block_size: 3,
            log2_min_transform_block_size_minus2: 0,
            log2_diff_max_min_transform_block_size: 3,
            log2_min_pcm_luma_coding_block_size_minus3: 0,
            log2_diff_max_min_pcm_luma_coding_block_size: 0,
            max_transform_hierarchy_depth_intra: 1,
            max_transform_hierarchy_depth_inter: 1,
            init_qp_minus26: -4,
            diff_cu_qp_delta_depth: 0,
            pps_cb_qp_offset: 0,
            pps_cr_qp_offset: 0,
            log2_parallel_merge_level_minus2: 0,
            num_tile_columns_minus1: 1,
            num_tile_rows_minus1: 0,
            column_width_minus1: {
                let mut c = [0; 19];
                c[0] = 30;
                c[1] = 32;
                c
            },
            row_height_minus1: [0; 21],
            // sao (7), rap (11), idr (12), intra (13).
            slice_parsing_fields: 1 << 7 | 1 << 11 | 1 << 12 | 1 << 13,
            log2_max_pic_order_cnt_lsb_minus4: 4,
            num_short_term_ref_pic_sets: 0,
            num_long_term_ref_pic_sps: 0,
            num_ref_idx_l0_default_active_minus1: 0,
            num_ref_idx_l1_default_active_minus1: 0,
            pps_beta_offset_div2: 0,
            pps_tc_offset_div2: 0,
            num_extra_slice_header_bits: 0,
            st_rps_bits: 0,
            reserved: [0; 8],
        }
    }

    #[test]
    fn translates_the_bit_fields() {
        let d = pic_params(&va(), true, 0, 3);
        let (w, h, f) = (
            d.pic_width_in_min_cbs_y,
            d.pic_height_in_min_cbs_y,
            d.format_and_sequence,
        );
        assert_eq!((w, h), (504, 378), "4032×3024 in 8×8 blocks");
        // chroma 4:2:0 (1), 10-bit luma and chroma (2 << 3, 2 << 6), POC lsb bits (4 << 9),
        // no reordering (1 << 13).
        assert_eq!(f, 1 | 2 << 3 | 2 << 6 | 4 << 9 | 1 << 13);
        let tools = d.coding_tools;
        assert_eq!(
            tools,
            1 | 1 << 1 | 1 << 2 | 1 << 19,
            "scaling lists, amp, sao, strong intra"
        );
        let props = d.picture_properties;
        assert_eq!(
            props,
            1 << 7 | 1 << 9 | 1 << 10 | 1 << 16 | 1 << 17 | 1 << 18,
            "tiles, uniform spacing, loop filter across tiles, irap, idr, intra"
        );
        let (poc, status, init_qp) = (
            d.curr_pic_order_cnt_val,
            d.status_report_feedback_number,
            d.init_qp_minus26,
        );
        assert_eq!((poc, status, init_qp), (7, 3, -4));
        let (cur, list) = (d.curr_pic, d.ref_pic_list);
        assert_eq!(
            (cur, list),
            (0, [0xff; 15]),
            "a still: entry 0, no references"
        );
        let cols = d.column_width_minus1;
        assert_eq!(&cols[..2], &[30, 32]);
    }

    /// References become entries by slot, with the RPS subsets as indices into them.
    #[test]
    fn translates_hevc_references() {
        let mut p = va();
        p.curr_pic.picture_id = 3;
        let r = |id, poc, flags| PictureHevc {
            picture_id: id,
            pic_order_cnt: poc,
            flags,
            reserved: [0; 4],
        };
        p.reference_frames[0] = r(1, 4, PICTURE_HEVC_RPS_ST_CURR_BEFORE);
        p.reference_frames[1] = r(5, 9, PICTURE_HEVC_RPS_ST_CURR_AFTER);
        p.reference_frames[2] = r(
            2,
            0,
            PICTURE_HEVC_LONG_TERM_REFERENCE | PICTURE_HEVC_RPS_LT_CURR,
        );
        let d = pic_params(&p, false, 2, 9);
        assert_eq!(d.curr_pic, 3);
        let list = d.ref_pic_list;
        assert_eq!(&list[..4], &[1, 5, 2 | 0x80, 0xff]);
        let pocs = d.pic_order_cnt_val_list;
        assert_eq!(&pocs[..3], &[4, 9, 0]);
        let (b, a, l) = (
            d.ref_pic_set_st_curr_before,
            d.ref_pic_set_st_curr_after,
            d.ref_pic_set_lt_curr,
        );
        assert_eq!((b[0], b[1]), (0, 0xff));
        assert_eq!((a[0], a[1]), (1, 0xff));
        assert_eq!((l[0], l[1]), (2, 0xff));
        assert_eq!(d.num_delta_pocs_of_ref_rps_idx, 2);
    }

    /// Raster in, coded (up-right diagonal) order out: the first coded positions of a raster
    /// 4×4 list are (0,0), (0,1), (1,0), (0,2)...
    #[test]
    fn qmatrix_is_in_coded_order() {
        let mut m = IqMatrixHevc {
            scaling_list_4x4: [[0; 16]; 6],
            scaling_list_8x8: [[0; 64]; 6],
            scaling_list_16x16: [[0; 64]; 6],
            scaling_list_32x32: [[0; 64]; 2],
            scaling_list_dc_16x16: [16; 6],
            scaling_list_dc_32x32: [16; 2],
            reserved: [0; 4],
        };
        for (i, v) in m.scaling_list_4x4[1].iter_mut().enumerate() {
            *v = i as u8;
        }
        for (i, v) in m.scaling_list_8x8[0].iter_mut().enumerate() {
            *v = i as u8;
        }
        let q = qmatrix(&m);
        let l4 = q.lists_4x4;
        assert_eq!(&l4[1][..6], &[0, 4, 1, 8, 5, 2]);
        let l8 = q.lists_8x8;
        assert_eq!(&l8[0][..6], &[0, 8, 1, 16, 9, 2]);
        let dc = q.dc_16x16;
        assert_eq!(dc, [16; 6]);
    }

    fn slice() -> SliceParamHevc {
        SliceParamHevc {
            slice_data_size: 0,
            slice_data_offset: 0,
            slice_data_flag: SLICE_DATA_FLAG_ALL,
            slice_data_byte_offset: 0,
            slice_segment_address: 0,
            ref_pic_list: [[0xff; 15]; 2],
            long_slice_flags: 0,
            collocated_ref_idx: 0xff,
            num_ref_idx_l0_active_minus1: 0,
            num_ref_idx_l1_active_minus1: 0,
            slice_qp_delta: 0,
            slice_cb_qp_offset: 0,
            slice_cr_qp_offset: 0,
            slice_beta_offset_div2: 0,
            slice_tc_offset_div2: 0,
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
            num_entry_point_offsets: 0,
            entry_offset_to_subset_array: 0,
            slice_data_num_emu_prevn_bytes: 0,
            reserved: [0; 2],
        }
    }

    #[test]
    fn bitstream_has_start_codes_and_padding() {
        let sp = slice();
        let a = [0x26u8, 1, 0xaf, 9, 9];
        let b = [0x26u8, 1, 0x30, 1, 2, 3];
        let (buf, ctl) = bitstream(&[(sp, &a[..]), (sp, &b[..])]);
        assert_eq!(buf.len(), BITSTREAM_ALIGN);
        assert_eq!(&buf[..8], &[0, 0, 1, 0x26, 1, 0xaf, 9, 9]);
        assert_eq!(&buf[8..17], &[0, 0, 1, 0x26, 1, 0x30, 1, 2, 3]);
        assert!(buf[17..].iter().all(|&x| x == 0));
        let (l0, n0) = (ctl[0].nal_unit_data_location, ctl[0].slice_bytes_in_buffer);
        let (l1, n1) = (ctl[1].nal_unit_data_location, ctl[1].slice_bytes_in_buffer);
        assert_eq!((l0, n0), (0, 8));
        assert_eq!(
            (l1, n1),
            (8, 9 + (128 - 17) as u32),
            "the padding counts into the last slice"
        );
    }

    /// A P frame referencing a frame and a top field: entries, counts and the reference bits.
    #[test]
    fn translates_h264() {
        let va_ref = |id, frame_idx, flags, top, bottom| PictureH264 {
            picture_id: id,
            frame_idx,
            flags,
            top_field_order_cnt: top,
            bottom_field_order_cnt: bottom,
            reserved: [0; 4],
        };
        let mut reference_frames = [PictureH264::INVALID; 16];
        reference_frames[0] = va_ref(2, 11, PICTURE_H264_SHORT_TERM_REFERENCE, 20, 21);
        reference_frames[1] = va_ref(
            4,
            3,
            PICTURE_H264_LONG_TERM_REFERENCE | PICTURE_H264_TOP_FIELD,
            6,
            0,
        );
        let p = PicParamH264 {
            curr_pic: va_ref(1, 12, PICTURE_H264_SHORT_TERM_REFERENCE, 24, 25),
            reference_frames,
            picture_width_in_mbs_minus1: 119,
            picture_height_in_mbs_minus1: 67,
            bit_depth_luma_minus8: 0,
            bit_depth_chroma_minus8: 0,
            num_ref_frames: 4,
            // 4:2:0, frame_mbs_only (4), direct_8x8 (6), MinLumaBiPred (7), log2_max_frame_num
            // 8 (4 << 8), poc type 0, log2_max_poc_lsb 8 (4 << 14).
            seq_fields: 1 | 1 << 4 | 1 << 6 | 1 << 7 | 4 << 8 | 4 << 14,
            num_slice_groups_minus1: 0,
            slice_group_map_type: 0,
            slice_group_change_rate_minus1: 0,
            pic_init_qp_minus26: -3,
            pic_init_qs_minus26: 0,
            chroma_qp_index_offset: 2,
            second_chroma_qp_index_offset: 2,
            // cabac (0), weighted_bipred_idc 2 (2 << 2), transform 8x8 (4), deblocking control
            // (8), reference (10).
            pic_fields: 1 | 2 << 2 | 1 << 4 | 1 << 8 | 1 << 10,
            frame_num: 12,
            reserved: [0; 8],
        };
        let first = SliceParamH264 {
            slice_data_size: 0,
            slice_data_offset: 0,
            slice_data_flag: 0,
            slice_data_bit_offset: 0,
            first_mb_in_slice: 0,
            slice_type: 0,
            direct_spatial_mv_pred_flag: 0,
            num_ref_idx_l0_active_minus1: 1,
            num_ref_idx_l1_active_minus1: 0,
            cabac_init_idc: 0,
            slice_qp_delta: 0,
            disable_deblocking_filter_idc: 0,
            slice_alpha_c0_offset_div2: 0,
            slice_beta_offset_div2: 0,
            ref_pic_list0: [PictureH264::INVALID; 32],
            ref_pic_list1: [PictureH264::INVALID; 32],
            luma_log2_weight_denom: 0,
            chroma_log2_weight_denom: 0,
            luma_weight_l0_flag: 0,
            luma_weight_l0: [0; 32],
            luma_offset_l0: [0; 32],
            chroma_weight_l0_flag: 0,
            chroma_weight_l0: [[0; 2]; 32],
            chroma_offset_l0: [[0; 2]; 32],
            luma_weight_l1_flag: 0,
            luma_weight_l1: [0; 32],
            luma_offset_l1: [0; 32],
            chroma_weight_l1_flag: 0,
            chroma_weight_l1: [[0; 2]; 32],
            chroma_offset_l1: [[0; 2]; 32],
            reserved: [0; 4],
        };
        let d = pic_params_h264(&p, &first, false, 5);
        let (w, h, cur, n) = (
            d.frame_width_in_mbs_minus1,
            d.frame_height_in_mbs_minus1,
            d.curr_pic,
            d.num_ref_frames,
        );
        assert_eq!((w, h, cur, n), (119, 67, 1, 4));
        let bits = d.bit_fields;
        // 4:2:0 (1 << 4), reference (1 << 6), weighted_bipred_idc 2 (2 << 9), consecutive
        // (1 << 11), frame_mbs_only (1 << 12), transform 8x8 (1 << 13), MinLumaBiPred (1 << 14).
        assert_eq!(
            bits,
            1 << 4 | 1 << 6 | 2 << 9 | 1 << 11 | 1 << 12 | 1 << 13 | 1 << 14
        );
        let list = d.ref_frame_list;
        assert_eq!(&list[..3], &[2, 4 | 0x80, 0xff]);
        let fn_list = d.frame_num_list;
        assert_eq!(&fn_list[..2], &[11, 3]);
        let pocs = d.field_order_cnt_list;
        assert_eq!((pocs[0], pocs[1]), ([20, 21], [6, 0]));
        let used = d.used_for_reference_flags;
        assert_eq!(
            used,
            0b11 | 0b01 << 2,
            "a frame: both fields; a top field: the top"
        );
        let cur_poc = d.curr_field_order_cnt;
        assert_eq!(cur_poc, [24, 25]);
        let (l0, qp, cabac, dbk, fnum, cont) = (
            d.num_ref_idx_l0_active_minus1,
            d.pic_init_qp_minus26,
            d.entropy_coding_mode_flag,
            d.deblocking_filter_control_present_flag,
            d.frame_num,
            d.continuation_flag,
        );
        assert_eq!((l0, qp, cabac, dbk, fnum, cont), (1, -3, 1, 1, 12, 1));
        let (lmfn, poct, lmpoc) = (
            d.log2_max_frame_num_minus4,
            d.pic_order_cnt_type,
            d.log2_max_pic_order_cnt_lsb_minus4,
        );
        assert_eq!((lmfn, poct, lmpoc), (4, 0, 4));
    }

    #[test]
    fn h264_qmatrix_is_zigzag() {
        let mut m = IqMatrixH264 {
            scaling_list_4x4: [[0; 16]; 6],
            scaling_list_8x8: [[0; 64]; 2],
            reserved: [0; 4],
        };
        for (i, v) in m.scaling_list_4x4[0].iter_mut().enumerate() {
            *v = i as u8;
        }
        let q = qmatrix_h264(&m);
        let l = q.lists_4x4;
        assert_eq!(&l[0][..6], &[0, 1, 4, 8, 5, 2]);
    }
}
