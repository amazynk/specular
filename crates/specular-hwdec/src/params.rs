//! The HEVC decode parameters in VA-API's layout (`va_dec_hevc.h`): the one form every hardware
//! backend takes. [`crate::hevc`] fills them from the parsed parameter sets and slice headers;
//! VA-API hands them to the driver as they are, D3D12 translates them to DXVA's structures
//! (`crate::d3d12`). Plain data on every platform, so the parser and its tests build everywhere.
//! Also the planes a decoded picture is read back into, and the copy out of a semi-planar
//! surface both backends share.

/// A VA-API surface id (`VASurfaceID`); D3D12 numbers pool slots the same way.
pub type Id = u32;
pub const INVALID_ID: Id = 0xffff_ffff;
/// `VA_PICTURE_HEVC_INVALID`.
pub const PICTURE_HEVC_INVALID: u32 = 0x1;
/// `VA_SLICE_DATA_FLAG_ALL`.
pub const SLICE_DATA_FLAG_ALL: u32 = 0;

/// `VAPictureHEVC`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PictureHevc {
    pub picture_id: Id,
    pub pic_order_cnt: i32,
    pub flags: u32,
    pub reserved: [u32; 4],
}

impl PictureHevc {
    pub const INVALID: PictureHevc = PictureHevc {
        picture_id: INVALID_ID,
        pic_order_cnt: 0,
        flags: PICTURE_HEVC_INVALID,
        reserved: [0; 4],
    };
}

/// `VAPictureParameterBufferHEVC`; the C bit-field unions are the plain `u32`s `pic_fields`
/// and `slice_parsing_fields` (LSB first, as GCC/Clang/MSVC lay out bit-fields on x86/ARM LE).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PicParamHevc {
    pub curr_pic: PictureHevc,
    pub reference_frames: [PictureHevc; 15],
    pub pic_width_in_luma_samples: u16,
    pub pic_height_in_luma_samples: u16,
    pub pic_fields: u32,
    pub sps_max_dec_pic_buffering_minus1: u8,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub pcm_sample_bit_depth_luma_minus1: u8,
    pub pcm_sample_bit_depth_chroma_minus1: u8,
    pub log2_min_luma_coding_block_size_minus3: u8,
    pub log2_diff_max_min_luma_coding_block_size: u8,
    pub log2_min_transform_block_size_minus2: u8,
    pub log2_diff_max_min_transform_block_size: u8,
    pub log2_min_pcm_luma_coding_block_size_minus3: u8,
    pub log2_diff_max_min_pcm_luma_coding_block_size: u8,
    pub max_transform_hierarchy_depth_intra: u8,
    pub max_transform_hierarchy_depth_inter: u8,
    pub init_qp_minus26: i8,
    pub diff_cu_qp_delta_depth: u8,
    pub pps_cb_qp_offset: i8,
    pub pps_cr_qp_offset: i8,
    pub log2_parallel_merge_level_minus2: u8,
    pub num_tile_columns_minus1: u8,
    pub num_tile_rows_minus1: u8,
    pub column_width_minus1: [u16; 19],
    pub row_height_minus1: [u16; 21],
    pub slice_parsing_fields: u32,
    pub log2_max_pic_order_cnt_lsb_minus4: u8,
    pub num_short_term_ref_pic_sets: u8,
    pub num_long_term_ref_pic_sps: u8,
    pub num_ref_idx_l0_default_active_minus1: u8,
    pub num_ref_idx_l1_default_active_minus1: u8,
    pub pps_beta_offset_div2: i8,
    pub pps_tc_offset_div2: i8,
    pub num_extra_slice_header_bits: u8,
    pub st_rps_bits: u32,
    pub reserved: [u32; 8],
}

/// `VASliceParameterBufferHEVC` (`LongSliceFlags` as a plain `u32`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SliceParamHevc {
    pub slice_data_size: u32,
    pub slice_data_offset: u32,
    pub slice_data_flag: u32,
    pub slice_data_byte_offset: u32,
    pub slice_segment_address: u32,
    pub ref_pic_list: [[u8; 15]; 2],
    pub long_slice_flags: u32,
    pub collocated_ref_idx: u8,
    pub num_ref_idx_l0_active_minus1: u8,
    pub num_ref_idx_l1_active_minus1: u8,
    pub slice_qp_delta: i8,
    pub slice_cb_qp_offset: i8,
    pub slice_cr_qp_offset: i8,
    pub slice_beta_offset_div2: i8,
    pub slice_tc_offset_div2: i8,
    pub luma_log2_weight_denom: u8,
    pub delta_chroma_log2_weight_denom: i8,
    pub delta_luma_weight_l0: [i8; 15],
    pub luma_offset_l0: [i8; 15],
    pub delta_chroma_weight_l0: [[i8; 2]; 15],
    pub chroma_offset_l0: [[i8; 2]; 15],
    pub delta_luma_weight_l1: [i8; 15],
    pub luma_offset_l1: [i8; 15],
    pub delta_chroma_weight_l1: [[i8; 2]; 15],
    pub chroma_offset_l1: [[i8; 2]; 15],
    pub five_minus_max_num_merge_cand: u8,
    pub num_entry_point_offsets: u16,
    pub entry_offset_to_subset_array: u16,
    pub slice_data_num_emu_prevn_bytes: u16,
    pub reserved: [u32; 2],
}

/// `VAIQMatrixBufferHEVC`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IqMatrixHevc {
    pub scaling_list_4x4: [[u8; 16]; 6],
    pub scaling_list_8x8: [[u8; 64]; 6],
    pub scaling_list_16x16: [[u8; 64]; 6],
    pub scaling_list_32x32: [[u8; 64]; 2],
    pub scaling_list_dc_16x16: [u8; 6],
    pub scaling_list_dc_32x32: [u8; 2],
    pub reserved: [u32; 4],
}

// Checked against libva 2.23's va_dec_hevc.h compiled with gcc (sizeof / offsetof).
const _: () = assert!(size_of::<PictureHevc>() == 28);
const _: () = assert!(size_of::<PicParamHevc>() == 604);
const _: () = assert!(std::mem::offset_of!(PicParamHevc, pic_fields) == 452);
const _: () = assert!(std::mem::offset_of!(PicParamHevc, column_width_minus1) == 476);
const _: () = assert!(std::mem::offset_of!(PicParamHevc, slice_parsing_fields) == 556);
const _: () = assert!(std::mem::offset_of!(PicParamHevc, st_rps_bits) == 568);
const _: () = assert!(size_of::<SliceParamHevc>() == 264);
const _: () = assert!(std::mem::offset_of!(SliceParamHevc, long_slice_flags) == 52);
const _: () = assert!(std::mem::offset_of!(SliceParamHevc, five_minus_max_num_merge_cand) == 246);
const _: () = assert!(std::mem::offset_of!(SliceParamHevc, slice_data_num_emu_prevn_bytes) == 252);
const _: () = assert!(size_of::<IqMatrixHevc>() == 1016);

// ------------------------------------------------------------------------- va.h (H.264)

pub const PICTURE_H264_INVALID: u32 = 0x1;
pub const PICTURE_H264_TOP_FIELD: u32 = 0x2;
pub const PICTURE_H264_BOTTOM_FIELD: u32 = 0x4;
pub const PICTURE_H264_SHORT_TERM_REFERENCE: u32 = 0x8;
pub const PICTURE_H264_LONG_TERM_REFERENCE: u32 = 0x10;
pub const PICTURE_HEVC_LONG_TERM_REFERENCE: u32 = 0x8;
pub const PICTURE_HEVC_RPS_ST_CURR_BEFORE: u32 = 0x10;
pub const PICTURE_HEVC_RPS_ST_CURR_AFTER: u32 = 0x20;
pub const PICTURE_HEVC_RPS_LT_CURR: u32 = 0x40;

/// `VAPictureH264`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PictureH264 {
    pub picture_id: Id,
    pub frame_idx: u32,
    pub flags: u32,
    pub top_field_order_cnt: i32,
    pub bottom_field_order_cnt: i32,
    pub reserved: [u32; 4],
}

impl PictureH264 {
    pub const INVALID: PictureH264 = PictureH264 {
        picture_id: INVALID_ID,
        frame_idx: 0,
        flags: PICTURE_H264_INVALID,
        top_field_order_cnt: 0,
        bottom_field_order_cnt: 0,
        reserved: [0; 4],
    };
}

/// `VAPictureParameterBufferH264` (`seq_fields` / `pic_fields` bit-fields as plain `u32`s).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PicParamH264 {
    pub curr_pic: PictureH264,
    pub reference_frames: [PictureH264; 16],
    pub picture_width_in_mbs_minus1: u16,
    pub picture_height_in_mbs_minus1: u16,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub num_ref_frames: u8,
    pub seq_fields: u32,
    pub num_slice_groups_minus1: u8,
    pub slice_group_map_type: u8,
    pub slice_group_change_rate_minus1: u16,
    pub pic_init_qp_minus26: i8,
    pub pic_init_qs_minus26: i8,
    pub chroma_qp_index_offset: i8,
    pub second_chroma_qp_index_offset: i8,
    pub pic_fields: u32,
    pub frame_num: u16,
    pub reserved: [u32; 8],
}

/// `VASliceParameterBufferH264`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SliceParamH264 {
    pub slice_data_size: u32,
    pub slice_data_offset: u32,
    pub slice_data_flag: u32,
    pub slice_data_bit_offset: u16,
    pub first_mb_in_slice: u16,
    pub slice_type: u8,
    pub direct_spatial_mv_pred_flag: u8,
    pub num_ref_idx_l0_active_minus1: u8,
    pub num_ref_idx_l1_active_minus1: u8,
    pub cabac_init_idc: u8,
    pub slice_qp_delta: i8,
    pub disable_deblocking_filter_idc: u8,
    pub slice_alpha_c0_offset_div2: i8,
    pub slice_beta_offset_div2: i8,
    pub ref_pic_list0: [PictureH264; 32],
    pub ref_pic_list1: [PictureH264; 32],
    pub luma_log2_weight_denom: u8,
    pub chroma_log2_weight_denom: u8,
    pub luma_weight_l0_flag: u8,
    pub luma_weight_l0: [i16; 32],
    pub luma_offset_l0: [i16; 32],
    pub chroma_weight_l0_flag: u8,
    pub chroma_weight_l0: [[i16; 2]; 32],
    pub chroma_offset_l0: [[i16; 2]; 32],
    pub luma_weight_l1_flag: u8,
    pub luma_weight_l1: [i16; 32],
    pub luma_offset_l1: [i16; 32],
    pub chroma_weight_l1_flag: u8,
    pub chroma_weight_l1: [[i16; 2]; 32],
    pub chroma_offset_l1: [[i16; 2]; 32],
    pub reserved: [u32; 4],
}

/// `VAIQMatrixBufferH264`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IqMatrixH264 {
    pub scaling_list_4x4: [[u8; 16]; 6],
    pub scaling_list_8x8: [[u8; 64]; 2],
    pub reserved: [u32; 4],
}

// Checked against libva 2.22's va.h compiled with clang (sizeof / offsetof).
const _: () = assert!(size_of::<PictureH264>() == 36);
const _: () = assert!(size_of::<PicParamH264>() == 672);
const _: () = assert!(std::mem::offset_of!(PicParamH264, num_ref_frames) == 618);
const _: () = assert!(std::mem::offset_of!(PicParamH264, seq_fields) == 620);
const _: () = assert!(std::mem::offset_of!(PicParamH264, pic_init_qp_minus26) == 628);
const _: () = assert!(std::mem::offset_of!(PicParamH264, pic_fields) == 632);
const _: () = assert!(std::mem::offset_of!(PicParamH264, frame_num) == 636);
const _: () = assert!(std::mem::offset_of!(PicParamH264, reserved) == 640);
const _: () = assert!(size_of::<SliceParamH264>() == 3128);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, slice_data_bit_offset) == 12);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, slice_type) == 16);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, ref_pic_list0) == 28);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, ref_pic_list1) == 1180);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, luma_log2_weight_denom) == 2332);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, luma_weight_l0) == 2336);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, chroma_weight_l0) == 2466);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, chroma_offset_l0) == 2594);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, luma_weight_l1) == 2724);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, chroma_weight_l1) == 2854);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, chroma_offset_l1) == 2982);
const _: () = assert!(std::mem::offset_of!(SliceParamH264, reserved) == 3112);
const _: () = assert!(size_of::<IqMatrixH264>() == 240);

/// A decoded picture read back: tightly packed Y, Cb, Cr planes (4:2:0, chroma
/// `ceil(w/2)`×`ceil(h/2)`), 8-bit samples as bytes, 10-bit ones as little-endian `u16`.
#[derive(Debug)]
pub struct HevcPlanes {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub planes: [Vec<u8>; 3],
}

/// The `w`×`h` region at (`x`, `y`) of a semi-planar 4:2:0 image in `data` (NV12, or P010
/// with each sample in the top 10 bits of 16) as [`HevcPlanes`]: the luma rows at `y_off`
/// with a pitch of `y_pitch` bytes, the interleaved Cb/Cr rows at `c_off` with `c_pitch`.
/// `x` and `y` are even. Every row read is checked against `data` first.
pub fn planes_from_420(
    data: &[u8],
    (y_off, y_pitch): (usize, usize),
    (c_off, c_pitch): (usize, usize),
    ten_bit: bool,
    (x, y, w, h): (u32, u32, u32, u32),
) -> Result<HevcPlanes, crate::Error> {
    let bad = |s: &str| crate::Error::Hardware(s.into());
    let bps = 1 + ten_bit as usize;
    let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);
    if w == 0 || h == 0 || x % 2 != 0 || y % 2 != 0 {
        return Err(bad("read-back region"));
    }
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let (yp, yo, up, uo) = (y_pitch, y_off, c_pitch, c_off);
    // Bounds of the last row read in each plane.
    let y_end = yo + (y + h - 1) * yp + (x + w) * bps;
    let c_end = uo + (y / 2 + ch - 1) * up + (x / 2 + cw) * 2 * bps;
    if yp < (x + w) * bps || up < (x / 2 + cw) * 2 * bps || y_end > data.len() || c_end > data.len()
    {
        return Err(bad("image plane outside the mapped buffer"));
    }
    let mut luma = Vec::with_capacity(w * h * bps);
    let (mut cb, mut cr) = (
        Vec::with_capacity(cw * ch * bps),
        Vec::with_capacity(cw * ch * bps),
    );
    // The mapping is often uncached (write-combined) driver memory, slow to read a sample at
    // a time: each row is copied out whole first (one memcpy) and split from there.
    let mut tmp = Vec::with_capacity((w * bps).max(cw * 2 * bps));
    fn fetch<'a>(tmp: &'a mut Vec<u8>, data: &[u8], at: usize, n: usize) -> &'a [u8] {
        tmp.clear();
        tmp.extend_from_slice(&data[at..at + n]);
        tmp
    }
    if ten_bit {
        let px = |b: &[u8]| (u16::from_le_bytes([b[0], b[1]]) >> 6).to_le_bytes();
        for r in 0..h {
            let row = fetch(&mut tmp, data, yo + (y + r) * yp + x * 2, w * 2);
            luma.extend(row.chunks_exact(2).flat_map(px));
        }
        for r in 0..ch {
            let row = fetch(&mut tmp, data, uo + (y / 2 + r) * up + x / 2 * 4, cw * 4);
            for q in row.chunks_exact(4) {
                cb.extend(px(&q[..2]));
                cr.extend(px(&q[2..]));
            }
        }
    } else {
        for r in 0..h {
            luma.extend_from_slice(&data[yo + (y + r) * yp + x..][..w]);
        }
        for r in 0..ch {
            let row = fetch(&mut tmp, data, uo + (y / 2 + r) * up + x / 2 * 2, cw * 2);
            for q in row.chunks_exact(2) {
                cb.push(q[0]);
                cr.push(q[1]);
            }
        }
    }
    Ok(HevcPlanes {
        width: w as u32,
        height: h as u32,
        bit_depth: if ten_bit { 10 } else { 8 },
        planes: [luma, cb, cr],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An NV12 image with a pitch wider than its rows, read in part: samples land in the right
    /// planes and the pitch padding is skipped.
    #[test]
    fn reads_a_region_of_nv12() {
        let (pitch, w, h) = (8usize, 6u32, 4u32);
        let mut data = vec![0u8; pitch * 4 + pitch * 2];
        for r in 0..4 {
            for c in 0..6 {
                data[r * pitch + c] = (10 * r + c) as u8;
            }
        }
        let c_off = pitch * 4;
        for r in 0..2 {
            for c in 0..3 {
                data[c_off + r * pitch + 2 * c] = (100 + 10 * r + c) as u8;
                data[c_off + r * pitch + 2 * c + 1] = (200 + 10 * r + c) as u8;
            }
        }
        let p = planes_from_420(&data, (0, pitch), (c_off, pitch), false, (2, 2, 4, 2)).unwrap();
        assert_eq!((p.width, p.height, p.bit_depth), (4, 2, 8));
        assert_eq!(p.planes[0], [22, 23, 24, 25, 32, 33, 34, 35]);
        assert_eq!(p.planes[1], [111, 112]);
        assert_eq!(p.planes[2], [211, 212]);
        assert!(
            planes_from_420(&data, (0, pitch), (c_off, pitch), false, (1, 0, 2, 2)).is_err(),
            "odd x"
        );
        assert!(
            planes_from_420(&data, (0, pitch), (c_off, pitch), false, (0, 0, w, h + 2)).is_err(),
            "past the end"
        );
    }

    /// P010 keeps the top 10 bits of each 16.
    #[test]
    fn reads_p010() {
        let pitch = 8usize;
        let mut data = vec![0u8; pitch * 2 + pitch];
        data[0..2].copy_from_slice(&(1023u16 << 6).to_le_bytes());
        data[2..4].copy_from_slice(&(512u16 << 6).to_le_bytes());
        data[pitch..pitch + 2].copy_from_slice(&(1u16 << 6).to_le_bytes());
        let c_off = pitch * 2;
        data[c_off..c_off + 2].copy_from_slice(&(300u16 << 6).to_le_bytes());
        data[c_off + 2..c_off + 4].copy_from_slice(&(700u16 << 6).to_le_bytes());
        let p = planes_from_420(&data, (0, pitch), (c_off, pitch), true, (0, 0, 2, 2)).unwrap();
        assert_eq!(p.bit_depth, 10);
        assert_eq!(p.planes[0], [255, 3, 0, 2, 1, 0, 0, 0]);
        assert_eq!(p.planes[1], 300u16.to_le_bytes());
        assert_eq!(p.planes[2], 700u16.to_le_bytes());
    }
}
