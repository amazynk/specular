//! The last check before a hostile bitstream reaches the GPU driver.
//!
//! Hardware decoders stay in the viewer process (DESIGN.md §12.3; why: the module docs of
//! [`crate`], "Where it runs"). What makes that acceptable is that the driver never parses the
//! bitstream's headers: our parsers ([`crate::hevc`], [`crate::hevc_video`], [`crate::h264`])
//! turn the parameter sets and slice headers into VA-API parameter structs, and the driver gets
//! those structs plus the entropy-coded slice bytes, which the fixed-function engine parses.
//!
//! The parsers check what they read, but they are long and the structs are filled in many
//! places. This gate checks every struct field a driver indexes or sizes with, independently
//! of how the parser got there, right at the hand-off (VA-API and D3D12 alike): picture and
//! block sizes against the surface, surface ids against the pool, reference indices against
//! the reference list, slice offsets against the slice bytes, quantiser, deblocking, weight
//! and tile values against their ITU-T ranges, scaling lists without zeros. A picture that
//! fails is never submitted: the decode falls back to software, which parses it with its own
//! checks.

use crate::params::{
    INVALID_ID, Id, IqMatrixH264, IqMatrixHevc, PicParamH264, PicParamHevc, PictureH264,
    SliceParamH264, SliceParamHevc,
};

/// Why a picture was stopped at the gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "refused before the hardware decoder: {}", self.0)
    }
}

impl From<Refused> for crate::Error {
    fn from(r: Refused) -> crate::Error {
        crate::Error::Malformed(r.to_string())
    }
}

type R = Result<(), Refused>;

fn check(ok: bool, what: impl FnOnce() -> String) -> R {
    if ok { Ok(()) } else { Err(Refused(what())) }
}

fn in_range<T: PartialOrd + Copy + std::fmt::Display>(v: T, lo: T, hi: T, what: &str) -> R {
    check(v >= lo && v <= hi, || {
        format!("{what} {v} outside {lo}..={hi}")
    })
}

/// A surface id the pool owns, or none.
fn surface_or_none(id: Id, surfaces: &[Id], what: &str) -> R {
    check(id == INVALID_ID || surfaces.contains(&id), || {
        format!("{what}: surface {id:#x} is not in the pool")
    })
}

/// Bytes of one slice as the driver will read them.
fn slice_bytes(size: u32, offset: u32, flag: u32, data: &[u8], header: u32) -> R {
    check(size as usize == data.len(), || {
        format!("slice size {size} for {} bytes", data.len())
    })?;
    check(offset == 0 && flag == 0, || {
        "slice data offset or flag".to_string()
    })?;
    check(header <= size, || {
        format!("slice data offset {header} past the slice ({size} bytes)")
    })
}

// ------------------------------------------------------------------------------------- HEVC

/// One HEVC picture: its parameters, scaling lists and slices, for a surface of `size` from
/// `surfaces`.
pub fn hevc(
    pic: &PicParamHevc,
    iq: Option<&IqMatrixHevc>,
    slices: &[(SliceParamHevc, &[u8])],
    surfaces: &[Id],
    size: (u32, u32),
) -> R {
    check(surfaces.contains(&pic.curr_pic.picture_id), || {
        "current picture is not a pool surface".into()
    })?;
    for (i, r) in pic.reference_frames.iter().enumerate() {
        surface_or_none(r.picture_id, surfaces, &format!("reference {i}"))?;
        check(r.picture_id != pic.curr_pic.picture_id, || {
            format!("reference {i} is the current picture")
        })?;
    }
    let (w, h) = (
        u32::from(pic.pic_width_in_luma_samples),
        u32::from(pic.pic_height_in_luma_samples),
    );
    check(w > 0 && h > 0 && w <= size.0 && h <= size.1, || {
        format!("{w}×{h} picture for a {}×{} surface", size.0, size.1)
    })?;
    in_range(pic.bit_depth_luma_minus8, 0, 2, "luma bit depth - 8")?;
    in_range(pic.bit_depth_chroma_minus8, 0, 2, "chroma bit depth - 8")?;
    in_range(
        pic.log2_min_luma_coding_block_size_minus3,
        0,
        3,
        "log2 min CB - 3",
    )?;
    let min_cb = 3 + u32::from(pic.log2_min_luma_coding_block_size_minus3);
    let ctb = min_cb + u32::from(pic.log2_diff_max_min_luma_coding_block_size);
    in_range(ctb, 4, 6, "log2 CTB size")?;
    check(w % (1 << min_cb) == 0 && h % (1 << min_cb) == 0, || {
        format!("{w}×{h} is not a multiple of the minimum CB")
    })?;
    let min_tb = 2 + u32::from(pic.log2_min_transform_block_size_minus2);
    let max_tb = min_tb + u32::from(pic.log2_diff_max_min_transform_block_size);
    check(min_tb < min_cb && max_tb <= ctb.min(5), || {
        format!("transform blocks 2^{min_tb}..2^{max_tb} for CTB 2^{ctb}")
    })?;
    in_range(
        u32::from(pic.max_transform_hierarchy_depth_intra),
        0,
        ctb - min_tb,
        "intra transform depth",
    )?;
    in_range(
        u32::from(pic.max_transform_hierarchy_depth_inter),
        0,
        ctb - min_tb,
        "inter transform depth",
    )?;
    in_range(
        u32::from(pic.diff_cu_qp_delta_depth),
        0,
        ctb - min_cb,
        "diff_cu_qp_delta_depth",
    )?;
    in_range(
        u32::from(pic.log2_parallel_merge_level_minus2),
        0,
        ctb - 2,
        "log2 parallel merge level - 2",
    )?;
    in_range(pic.sps_max_dec_pic_buffering_minus1, 0, 15, "DPB size - 1")?;
    in_range(
        pic.pcm_sample_bit_depth_luma_minus1,
        0,
        pic.bit_depth_luma_minus8 + 7,
        "PCM luma bit depth - 1",
    )?;
    in_range(
        pic.pcm_sample_bit_depth_chroma_minus1,
        0,
        pic.bit_depth_chroma_minus8 + 7,
        "PCM chroma bit depth - 1",
    )?;
    let qp_bd = 6 * i32::from(pic.bit_depth_luma_minus8);
    in_range(
        i32::from(pic.init_qp_minus26),
        -26 - qp_bd,
        25,
        "init_qp - 26",
    )?;
    in_range(pic.pps_cb_qp_offset, -12, 12, "Cb QP offset")?;
    in_range(pic.pps_cr_qp_offset, -12, 12, "Cr QP offset")?;
    in_range(pic.pps_beta_offset_div2, -6, 6, "beta offset / 2")?;
    in_range(pic.pps_tc_offset_div2, -6, 6, "tC offset / 2")?;
    in_range(
        pic.log2_max_pic_order_cnt_lsb_minus4,
        0,
        12,
        "log2 max POC LSB - 4",
    )?;
    in_range(
        pic.num_short_term_ref_pic_sets,
        0,
        64,
        "short-term RPS count",
    )?;
    in_range(
        pic.num_long_term_ref_pic_sps,
        0,
        32,
        "long-term reference count",
    )?;
    in_range(
        pic.num_ref_idx_l0_default_active_minus1,
        0,
        14,
        "L0 default size - 1",
    )?;
    in_range(
        pic.num_ref_idx_l1_default_active_minus1,
        0,
        14,
        "L1 default size - 1",
    )?;
    in_range(
        pic.num_extra_slice_header_bits,
        0,
        7,
        "extra slice header bits",
    )?;
    // Tiles: the explicit sizes fit the picture's CTB columns and rows.
    let (wc, hc) = (w.div_ceil(1 << ctb), h.div_ceil(1 << ctb));
    in_range(pic.num_tile_columns_minus1, 0, 19, "tile columns - 1")?;
    in_range(pic.num_tile_rows_minus1, 0, 21, "tile rows - 1")?;
    let cols = usize::from(pic.num_tile_columns_minus1).min(19);
    let rows = usize::from(pic.num_tile_rows_minus1).min(21);
    let sum = |v: &[u16]| v.iter().map(|&x| u32::from(x) + 1).sum::<u32>();
    if cols > 0 {
        check(sum(&pic.column_width_minus1[..cols]) <= wc, || {
            "tile columns wider than the picture".into()
        })?;
    }
    if rows > 0 {
        check(sum(&pic.row_height_minus1[..rows]) <= hc, || {
            "tile rows taller than the picture".into()
        })?;
    }
    if let Some(iq) = iq {
        let lists = iq
            .scaling_list_4x4
            .iter()
            .flatten()
            .chain(iq.scaling_list_8x8.iter().flatten())
            .chain(iq.scaling_list_16x16.iter().flatten())
            .chain(iq.scaling_list_32x32.iter().flatten())
            .chain(&iq.scaling_list_dc_16x16)
            .chain(&iq.scaling_list_dc_32x32);
        check(lists.into_iter().all(|&v| v > 0), || {
            "a zero scaling factor".into()
        })?;
    }
    check(!slices.is_empty(), || "no slices".into())?;
    let ctbs = wc * hc;
    for (sp, data) in slices {
        hevc_slice(pic, sp, data, ctbs)?;
    }
    Ok(())
}

fn hevc_slice(pic: &PicParamHevc, sp: &SliceParamHevc, data: &[u8], ctbs: u32) -> R {
    slice_bytes(
        sp.slice_data_size,
        sp.slice_data_offset,
        sp.slice_data_flag,
        data,
        sp.slice_data_byte_offset,
    )?;
    check(sp.slice_segment_address < ctbs, || {
        format!(
            "slice address {} past the picture's {ctbs} CTBs",
            sp.slice_segment_address
        )
    })?;
    check(
        u32::from(sp.slice_data_num_emu_prevn_bytes) <= sp.slice_data_size,
        || "emulation prevention bytes".into(),
    )?;
    in_range(sp.num_ref_idx_l0_active_minus1, 0, 14, "L0 size - 1")?;
    in_range(sp.num_ref_idx_l1_active_minus1, 0, 14, "L1 size - 1")?;
    // Reference indices point at a reference the picture parameters list, or at none.
    for (l, list) in sp.ref_pic_list.iter().enumerate() {
        for &i in list {
            if i == 0xff {
                continue;
            }
            check(
                usize::from(i) < pic.reference_frames.len()
                    && pic.reference_frames[usize::from(i)].picture_id != INVALID_ID,
                || format!("list {l} names reference {i}, which is not there"),
            )?;
        }
    }
    check(
        sp.collocated_ref_idx == 0xff || sp.collocated_ref_idx <= 14,
        || format!("collocated_ref_idx {}", sp.collocated_ref_idx),
    )?;
    let qp_bd = 6 * i32::from(pic.bit_depth_luma_minus8);
    let qp = 26 + i32::from(pic.init_qp_minus26) + i32::from(sp.slice_qp_delta);
    in_range(qp, -qp_bd, 51, "slice QP")?;
    in_range(sp.slice_cb_qp_offset, -12, 12, "slice Cb QP offset")?;
    in_range(sp.slice_cr_qp_offset, -12, 12, "slice Cr QP offset")?;
    in_range(sp.slice_beta_offset_div2, -6, 6, "slice beta offset / 2")?;
    in_range(sp.slice_tc_offset_div2, -6, 6, "slice tC offset / 2")?;
    in_range(sp.luma_log2_weight_denom, 0, 7, "luma weight denominator")?;
    in_range(
        i32::from(sp.luma_log2_weight_denom) + i32::from(sp.delta_chroma_log2_weight_denom),
        0,
        7,
        "chroma weight denominator",
    )?;
    in_range(
        sp.five_minus_max_num_merge_cand,
        0,
        4,
        "5 - merge candidates",
    )?;
    check(u32::from(sp.num_entry_point_offsets) < ctbs.max(1), || {
        "more entry points than CTBs".into()
    })
}

// ------------------------------------------------------------------------------------ H.264

fn h264_picture(p: &PictureH264, surfaces: &[Id], what: &str) -> R {
    surface_or_none(p.picture_id, surfaces, what)
}

/// One H.264 picture (a frame or a field): its parameters, scaling lists and slices, for a
/// surface of `size` from `surfaces`.
pub fn h264(
    pic: &PicParamH264,
    iq: Option<&IqMatrixH264>,
    slices: &[(SliceParamH264, &[u8])],
    surfaces: &[Id],
    size: (u32, u32),
) -> R {
    check(surfaces.contains(&pic.curr_pic.picture_id), || {
        "current picture is not a pool surface".into()
    })?;
    for (i, r) in pic.reference_frames.iter().enumerate() {
        h264_picture(r, surfaces, &format!("reference {i}"))?;
    }
    let (wm, hm) = (
        u32::from(pic.picture_width_in_mbs_minus1) + 1,
        u32::from(pic.picture_height_in_mbs_minus1) + 1,
    );
    check(wm * 16 <= size.0 && hm * 16 <= size.1, || {
        format!(
            "{}×{} macroblocks for a {}×{} surface",
            wm, hm, size.0, size.1
        )
    })?;
    check(
        pic.bit_depth_luma_minus8 == 0 && pic.bit_depth_chroma_minus8 == 0,
        || "not 8-bit".into(),
    )?;
    in_range(pic.num_ref_frames, 0, 16, "reference frames")?;
    check(pic.num_slice_groups_minus1 == 0, || "slice groups".into())?;
    in_range(pic.pic_init_qp_minus26, -26, 25, "pic_init_qp - 26")?;
    in_range(pic.pic_init_qs_minus26, -26, 25, "pic_init_qs - 26")?;
    in_range(pic.chroma_qp_index_offset, -12, 12, "chroma QP offset")?;
    in_range(
        pic.second_chroma_qp_index_offset,
        -12,
        12,
        "second chroma QP offset",
    )?;
    // log2_max_frame_num - 4 and log2_max_pic_order_cnt_lsb - 4 (seq_fields bits 8..12, 14..18).
    in_range((pic.seq_fields >> 8) & 0xf, 0, 12, "log2 max frame_num - 4")?;
    in_range((pic.seq_fields >> 12) & 0x3, 0, 2, "POC type")?;
    in_range((pic.seq_fields >> 14) & 0xf, 0, 12, "log2 max POC LSB - 4")?;
    in_range((pic.pic_fields >> 2) & 0x3, 0, 2, "weighted_bipred_idc")?;
    if let Some(iq) = iq {
        let all = iq
            .scaling_list_4x4
            .iter()
            .flatten()
            .chain(iq.scaling_list_8x8.iter().flatten());
        check(all.into_iter().all(|&v| v > 0), || {
            "a zero scaling factor".into()
        })?;
    }
    check(!slices.is_empty(), || "no slices".into())?;
    let mbs = wm * hm;
    for (sp, data) in slices {
        h264_slice(pic, sp, data, surfaces, mbs)?;
    }
    Ok(())
}

fn h264_slice(
    pic: &PicParamH264,
    sp: &SliceParamH264,
    data: &[u8],
    surfaces: &[Id],
    mbs: u32,
) -> R {
    slice_bytes(
        sp.slice_data_size,
        sp.slice_data_offset,
        sp.slice_data_flag,
        data,
        u32::from(sp.slice_data_bit_offset).div_ceil(8),
    )?;
    check(u32::from(sp.first_mb_in_slice) < mbs, || {
        format!(
            "first macroblock {} past the picture's {mbs}",
            sp.first_mb_in_slice
        )
    })?;
    in_range(sp.slice_type, 0, 4, "slice type")?;
    in_range(sp.num_ref_idx_l0_active_minus1, 0, 31, "L0 size - 1")?;
    in_range(sp.num_ref_idx_l1_active_minus1, 0, 31, "L1 size - 1")?;
    for (l, list) in [&sp.ref_pic_list0, &sp.ref_pic_list1]
        .into_iter()
        .enumerate()
    {
        for (i, r) in list.iter().enumerate() {
            h264_picture(r, surfaces, &format!("list {l} entry {i}"))?;
        }
    }
    in_range(sp.cabac_init_idc, 0, 2, "cabac_init_idc")?;
    in_range(
        sp.disable_deblocking_filter_idc,
        0,
        2,
        "deblocking filter idc",
    )?;
    in_range(sp.slice_alpha_c0_offset_div2, -6, 6, "alpha offset / 2")?;
    in_range(sp.slice_beta_offset_div2, -6, 6, "beta offset / 2")?;
    let qp = 26 + i32::from(pic.pic_init_qp_minus26) + i32::from(sp.slice_qp_delta);
    in_range(qp, 0, 51, "slice QP")?;
    in_range(sp.luma_log2_weight_denom, 0, 7, "luma weight denominator")?;
    in_range(
        sp.chroma_log2_weight_denom,
        0,
        7,
        "chroma weight denominator",
    )?;
    let weights = sp
        .luma_weight_l0
        .iter()
        .chain(&sp.luma_offset_l0)
        .chain(sp.chroma_weight_l0.iter().flatten())
        .chain(sp.chroma_offset_l0.iter().flatten())
        .chain(&sp.luma_weight_l1)
        .chain(&sp.luma_offset_l1)
        .chain(sp.chroma_weight_l1.iter().flatten())
        .chain(sp.chroma_offset_l1.iter().flatten());
    for &v in weights {
        // Weights are -128..=127; a default weight is 2^denominator, at most 128.
        in_range(v, -128, 128, "weight or offset")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{PICTURE_HEVC_INVALID, PictureHevc};

    const POOL: [Id; 3] = [10, 11, 12];

    /// Named changes to a struct, each made alone.
    type Cases<T> = Vec<(&'static str, Box<dyn Fn(&mut T)>)>;

    fn hevc_pic() -> PicParamHevc {
        PicParamHevc {
            curr_pic: PictureHevc {
                picture_id: 10,
                ..PictureHevc::INVALID
            },
            reference_frames: [PictureHevc::INVALID; 15],
            pic_width_in_luma_samples: 128,
            pic_height_in_luma_samples: 64,
            pic_fields: 1,
            sps_max_dec_pic_buffering_minus1: 2,
            bit_depth_luma_minus8: 0,
            bit_depth_chroma_minus8: 0,
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
            init_qp_minus26: 0,
            diff_cu_qp_delta_depth: 0,
            pps_cb_qp_offset: 0,
            pps_cr_qp_offset: 0,
            log2_parallel_merge_level_minus2: 0,
            num_tile_columns_minus1: 0,
            num_tile_rows_minus1: 0,
            column_width_minus1: [0; 19],
            row_height_minus1: [0; 21],
            slice_parsing_fields: 0,
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

    fn hevc_slice_param(len: usize) -> SliceParamHevc {
        SliceParamHevc {
            slice_data_size: len as u32,
            slice_data_offset: 0,
            slice_data_flag: 0,
            slice_data_byte_offset: 4,
            slice_segment_address: 0,
            ref_pic_list: [[0xff; 15]; 2],
            long_slice_flags: 1 | 2 << 2,
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
    fn hevc_good_picture_passes() {
        let data = [0u8; 32];
        let s = [(hevc_slice_param(32), &data[..])];
        hevc(&hevc_pic(), None, &s, &POOL, (128, 64)).unwrap();
    }

    /// Hostile values the parsers should never let through, one at a time: each is stopped.
    #[test]
    fn hevc_hostile_values_are_stopped() {
        let data = [0u8; 32];
        let good = hevc_slice_param(32);
        let pic_cases: Cases<PicParamHevc> = vec![
            ("foreign surface", Box::new(|p| p.curr_pic.picture_id = 99)),
            (
                "reference not in pool",
                Box::new(|p| p.reference_frames[3].picture_id = 0x4000),
            ),
            (
                "reference is current",
                Box::new(|p| p.reference_frames[0].picture_id = 10),
            ),
            (
                "wider than surface",
                Box::new(|p| p.pic_width_in_luma_samples = 4096),
            ),
            ("12-bit", Box::new(|p| p.bit_depth_luma_minus8 = 4)),
            (
                "CTB 128",
                Box::new(|p| p.log2_diff_max_min_luma_coding_block_size = 4),
            ),
            (
                "TB above CTB",
                Box::new(|p| p.log2_diff_max_min_transform_block_size = 6),
            ),
            ("init_qp", Box::new(|p| p.init_qp_minus26 = 40)),
            ("cb offset", Box::new(|p| p.pps_cb_qp_offset = -100)),
            (
                "tile columns",
                Box::new(|p| p.num_tile_columns_minus1 = 200),
            ),
            (
                "tile widths",
                Box::new(|p| {
                    p.num_tile_columns_minus1 = 2;
                    p.column_width_minus1[0] = 500;
                }),
            ),
            (
                "POC bits",
                Box::new(|p| p.log2_max_pic_order_cnt_lsb_minus4 = 200),
            ),
            (
                "RPS count",
                Box::new(|p| p.num_short_term_ref_pic_sets = 255),
            ),
        ];
        for (what, f) in &pic_cases {
            let mut p = hevc_pic();
            f(&mut p);
            let s = [(good, &data[..])];
            assert!(hevc(&p, None, &s, &POOL, (128, 64)).is_err(), "{what}");
        }
        let slice_cases: Cases<SliceParamHevc> = vec![
            ("size", Box::new(|s| s.slice_data_size = 1 << 30)),
            ("offset", Box::new(|s| s.slice_data_offset = 8)),
            (
                "header past data",
                Box::new(|s| s.slice_data_byte_offset = 33),
            ),
            ("address", Box::new(|s| s.slice_segment_address = 1_000_000)),
            ("ref index", Box::new(|s| s.ref_pic_list[0][0] = 40)),
            ("missing ref", Box::new(|s| s.ref_pic_list[1][2] = 3)),
            ("L0 size", Box::new(|s| s.num_ref_idx_l0_active_minus1 = 15)),
            ("collocated", Box::new(|s| s.collocated_ref_idx = 20)),
            ("qp", Box::new(|s| s.slice_qp_delta = 100)),
            ("weight denom", Box::new(|s| s.luma_log2_weight_denom = 9)),
            ("merge", Box::new(|s| s.five_minus_max_num_merge_cand = 7)),
            (
                "entry points",
                Box::new(|s| s.num_entry_point_offsets = 9000),
            ),
            (
                "emulation bytes",
                Box::new(|s| s.slice_data_num_emu_prevn_bytes = 100),
            ),
        ];
        for (what, f) in &slice_cases {
            let mut sp = good;
            f(&mut sp);
            let s = [(sp, &data[..])];
            assert!(
                hevc(&hevc_pic(), None, &s, &POOL, (128, 64)).is_err(),
                "{what}"
            );
        }
        // A valid reference passes; a zero scaling factor does not.
        let mut p = hevc_pic();
        p.reference_frames[0] = PictureHevc {
            picture_id: 11,
            flags: 0,
            ..PictureHevc::INVALID
        };
        let mut sp = good;
        sp.ref_pic_list[0][0] = 0;
        hevc(&p, None, &[(sp, &data[..])], &POOL, (128, 64)).unwrap();
        let iq = IqMatrixHevc {
            scaling_list_4x4: [[16; 16]; 6],
            scaling_list_8x8: [[16; 64]; 6],
            scaling_list_16x16: [[16; 64]; 6],
            scaling_list_32x32: [[16; 64]; 2],
            scaling_list_dc_16x16: [16; 6],
            scaling_list_dc_32x32: [0, 16],
            reserved: [0; 4],
        };
        assert!(hevc(&p, Some(&iq), &[(sp, &data[..])], &POOL, (128, 64)).is_err());
        assert!(hevc(&p, None, &[], &POOL, (128, 64)).is_err(), "no slices");
        let _ = PICTURE_HEVC_INVALID;
    }

    fn h264_pic() -> PicParamH264 {
        PicParamH264 {
            curr_pic: PictureH264 {
                picture_id: 10,
                ..PictureH264::INVALID
            },
            reference_frames: [PictureH264::INVALID; 16],
            picture_width_in_mbs_minus1: 7,
            picture_height_in_mbs_minus1: 3,
            bit_depth_luma_minus8: 0,
            bit_depth_chroma_minus8: 0,
            num_ref_frames: 2,
            seq_fields: 1 | 1 << 4,
            num_slice_groups_minus1: 0,
            slice_group_map_type: 0,
            slice_group_change_rate_minus1: 0,
            pic_init_qp_minus26: 0,
            pic_init_qs_minus26: 0,
            chroma_qp_index_offset: 0,
            second_chroma_qp_index_offset: 0,
            pic_fields: 0,
            frame_num: 0,
            reserved: [0; 8],
        }
    }

    fn h264_slice_param(len: usize) -> SliceParamH264 {
        SliceParamH264 {
            slice_data_size: len as u32,
            slice_data_offset: 0,
            slice_data_flag: 0,
            slice_data_bit_offset: 20,
            first_mb_in_slice: 0,
            slice_type: 2,
            direct_spatial_mv_pred_flag: 0,
            num_ref_idx_l0_active_minus1: 0,
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
        }
    }

    #[test]
    fn h264_hostile_values_are_stopped() {
        let data = [0u8; 16];
        let good = h264_slice_param(16);
        h264(&h264_pic(), None, &[(good, &data[..])], &POOL, (128, 64)).unwrap();
        let pic_cases: Cases<PicParamH264> = vec![
            ("foreign surface", Box::new(|p| p.curr_pic.picture_id = 1)),
            (
                "reference not in pool",
                Box::new(|p| p.reference_frames[15].picture_id = 77),
            ),
            (
                "too wide",
                Box::new(|p| p.picture_width_in_mbs_minus1 = 500),
            ),
            ("10-bit", Box::new(|p| p.bit_depth_luma_minus8 = 2)),
            ("refs", Box::new(|p| p.num_ref_frames = 40)),
            ("FMO", Box::new(|p| p.num_slice_groups_minus1 = 3)),
            ("qp", Box::new(|p| p.pic_init_qp_minus26 = 60)),
            ("frame_num bits", Box::new(|p| p.seq_fields |= 0xf << 8)),
        ];
        for (what, f) in &pic_cases {
            let mut p = h264_pic();
            f(&mut p);
            assert!(
                h264(&p, None, &[(good, &data[..])], &POOL, (128, 64)).is_err(),
                "{what}"
            );
        }
        let slice_cases: Cases<SliceParamH264> = vec![
            ("size", Box::new(|s| s.slice_data_size = 17)),
            (
                "header past data",
                Box::new(|s| s.slice_data_bit_offset = 200),
            ),
            ("first MB", Box::new(|s| s.first_mb_in_slice = 32)),
            ("slice type", Box::new(|s| s.slice_type = 9)),
            ("L1 size", Box::new(|s| s.num_ref_idx_l1_active_minus1 = 32)),
            (
                "list surface",
                Box::new(|s| s.ref_pic_list1[31].picture_id = 5),
            ),
            ("cabac", Box::new(|s| s.cabac_init_idc = 3)),
            (
                "deblocking",
                Box::new(|s| s.disable_deblocking_filter_idc = 7),
            ),
            ("qp", Box::new(|s| s.slice_qp_delta = -60)),
            ("weight", Box::new(|s| s.luma_weight_l0[3] = 1000)),
            ("denominator", Box::new(|s| s.chroma_log2_weight_denom = 8)),
        ];
        for (what, f) in &slice_cases {
            let mut sp = good;
            f(&mut sp);
            assert!(
                h264(&h264_pic(), None, &[(sp, &data[..])], &POOL, (128, 64)).is_err(),
                "{what}"
            );
        }
        let iq = IqMatrixH264 {
            scaling_list_4x4: [[16; 16]; 6],
            scaling_list_8x8: [[0; 64]; 2],
            reserved: [0; 4],
        };
        assert!(
            h264(
                &h264_pic(),
                Some(&iq),
                &[(good, &data[..])],
                &POOL,
                (128, 64)
            )
            .is_err()
        );
    }
}
