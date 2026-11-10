//! Slice segment header (clause 7.3.6.1).

use crate::bits::BitReader;
use crate::error::{Error, Result};
use crate::nal::{NalHeader, unit_type};
use crate::ps::{Pps, Sps, rps};
use alloc::boxed::Box;
use alloc::vec::Vec;

/// `slice_type` values.
pub mod slice_type {
    /// Bi-predictive slice.
    pub const B: u8 = 0;
    /// Predictive slice.
    pub const P: u8 = 1;
    /// Intra slice.
    pub const I: u8 = 2;
}

/// One long-term reference picture named by a slice header.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LtEntry {
    /// `PocLsbLt[i]`.
    pub poc_lsb: u32,
    /// `UsedByCurrPicLt[i]`.
    pub used: bool,
    /// `delta_poc_msb_present_flag[i]`.
    pub msb_present: bool,
    /// `DeltaPocMsbCycleLt[i]`.
    pub msb_cycle: u32,
}

/// Explicit weighted-prediction factors of one reference, per component.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Weight {
    /// `LumaWeightLX`, `ChromaWeightLX[0]`, `ChromaWeightLX[1]`.
    pub w: [i32; 3],
    /// The matching offsets, already shifted by `WpOffsetBdShift`.
    pub o: [i32; 3],
}

/// `pred_weight_table()` in derived form.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PredWeights {
    /// `luma_log2_weight_denom` and `ChromaLog2WeightDenom`.
    pub log2_denom: [u32; 2],
    /// Factors per list and reference index.
    pub w: [[Weight; 16]; 2],
}

/// The parsed slice segment header.
#[derive(Debug, Clone, Default)]
pub struct SliceHeader {
    /// `first_slice_segment_in_pic_flag`.
    pub first_in_pic: bool,
    /// `no_output_of_prior_pics_flag`.
    pub no_output_of_prior_pics: bool,
    /// `slice_pic_parameter_set_id`.
    pub pps_id: u32,
    /// `dependent_slice_segment_flag`.
    pub dependent: bool,
    /// `colour_plane_id` (separate colour planes: 0 = Y, 1 = Cb, 2 = Cr).
    pub colour_plane: u8,
    /// `slice_segment_address`, a CTB raster address.
    pub segment_address: u32,
    /// Address of the first CTB of the containing slice (not segment).
    pub slice_address: u32,
    /// `slice_type` (see [`slice_type`]).
    pub slice_type: u8,
    /// `pic_output_flag` (1 when absent).
    pub pic_output: bool,
    /// `slice_pic_order_cnt_lsb`.
    pub poc_lsb: u32,
    /// The short-term reference picture set in use.
    pub st_rps: rps::StRps,
    /// Long-term reference pictures.
    pub lt: Vec<LtEntry>,
    /// `slice_temporal_mvp_enabled_flag`.
    pub temporal_mvp: bool,
    /// `slice_sao_luma_flag`.
    pub sao_luma: bool,
    /// `slice_sao_chroma_flag`.
    pub sao_chroma: bool,
    /// `num_ref_idx_lX_active_minus1 + 1` (0 for lists the slice lacks).
    pub num_ref_idx: [u32; 2],
    /// `ref_pic_list_modification_flag_lX`.
    pub list_modified: [bool; 2],
    /// `list_entry_lX[]`.
    pub list_entry: [[u8; 16]; 2],
    /// `mvd_l1_zero_flag`.
    pub mvd_l1_zero: bool,
    /// `cabac_init_flag`.
    pub cabac_init: bool,
    /// `collocated_from_l0_flag` (1 when absent).
    pub collocated_from_l0: bool,
    /// `collocated_ref_idx`.
    pub collocated_ref_idx: u32,
    /// Explicit weights, when the PPS enables them for this slice type.
    pub weights: Option<Box<PredWeights>>,
    /// `MaxNumMergeCand`.
    pub max_num_merge_cand: u32,
    /// `NumPicTotalCurr`.
    pub num_pic_total_curr: u32,
    /// `SliceQpY`.
    pub qp: i32,
    /// `slice_cb_qp_offset`.
    pub cb_qp_offset: i32,
    /// `slice_cr_qp_offset`.
    pub cr_qp_offset: i32,
    /// `cu_chroma_qp_offset_enabled_flag`.
    pub cu_chroma_qp_offset_enabled: bool,
    /// `slice_deblocking_filter_disabled_flag`.
    pub deblock_disabled: bool,
    /// `slice_beta_offset_div2`.
    pub beta_offset_div2: i32,
    /// `slice_tc_offset_div2`.
    pub tc_offset_div2: i32,
    /// `slice_loop_filter_across_slices_enabled_flag`.
    pub across_slices: bool,
    /// `entry_point_offset_minus1[i] + 1`, in NAL unit bytes.
    pub entry_points: Vec<u32>,
    /// Byte offset of the slice segment data within the RBSP.
    pub data_offset: usize,
}

impl SliceHeader {
    /// True for a P or B slice.
    #[inline]
    pub fn is_inter(&self) -> bool {
        self.slice_type != slice_type::I
    }

    /// True for a B slice.
    #[inline]
    pub fn is_b(&self) -> bool {
        self.slice_type == slice_type::B
    }

    /// CABAC `initType` (9.3.2.2).
    pub fn init_type(&self) -> usize {
        match self.slice_type {
            slice_type::I => 0,
            slice_type::P => {
                if self.cabac_init {
                    2
                } else {
                    1
                }
            }
            _ => {
                if self.cabac_init {
                    1
                } else {
                    2
                }
            }
        }
    }
}

/// Parses `slice_segment_header()` from `data`, which is an SPS-scoped RBSP.
///
/// `prev` supplies the inherited fields of a dependent slice segment.
pub fn parse(
    nh: &NalHeader,
    data: &[u8],
    sps: &Sps,
    pps: &Pps,
    prev: Option<&SliceHeader>,
) -> Result<SliceHeader> {
    let r = &mut BitReader::new(data);
    let mut h = SliceHeader {
        across_slices: pps.across_slices,
        slice_type: slice_type::I,
        pic_output: true,
        collocated_from_l0: true,
        ..Default::default()
    };
    h.first_in_pic = r.u1()? != 0;
    if nh.is_irap() {
        h.no_output_of_prior_pics = r.u1()? != 0;
    }
    h.pps_id = r.ue()?;
    if !h.first_in_pic {
        if pps.dependent_slices {
            h.dependent = r.u1()? != 0;
        }
        let bits = ceil_log2(sps.size_in_ctbs());
        h.segment_address = r.u(bits)?;
        if h.segment_address as usize >= sps.size_in_ctbs() {
            return Err(Error::InvalidData("slice_segment_address out of range"));
        }
    }
    if h.dependent {
        // A dependent slice segment inherits every slice header field from
        // the independent segment that precedes it (7.4.7.1).
        let p = prev.ok_or(Error::InvalidData(
            "dependent slice segment without a preceding slice segment",
        ))?;
        if p.pps_id != h.pps_id {
            return Err(Error::InvalidData(
                "dependent slice segment changes the PPS",
            ));
        }
        let (pps_id, address) = (h.pps_id, h.segment_address);
        h = SliceHeader {
            first_in_pic: false,
            pps_id,
            dependent: true,
            segment_address: address,
            entry_points: Vec::new(),
            data_offset: 0,
            ..p.clone()
        };
    } else {
        h.slice_address = h.segment_address;
        parse_independent(r, nh, sps, pps, &mut h)?;
    }
    if pps.tiles_enabled || pps.wpp_enabled {
        let n = r.ue()? as usize;
        if n > sps.size_in_ctbs() + 1 {
            return Err(Error::InvalidData("num_entry_point_offsets too large"));
        }
        if n > 0 {
            let len = r.ue()? + 1;
            if len > 32 {
                return Err(Error::InvalidData("offset_len_minus1 > 31"));
            }
            for _ in 0..n {
                h.entry_points.push(r.u(len)?.wrapping_add(1));
            }
        }
    }
    if pps.slice_header_extension {
        let n = r.ue()? as usize;
        r.skip(n * 8)?;
    }
    // byte_alignment(): alignment_bit_equal_to_one then zero bits.
    r.u1()?;
    r.byte_align();
    h.data_offset = r.byte_pos();
    Ok(h)
}

/// Parses the fields present only in an independent slice segment header.
fn parse_independent(
    r: &mut BitReader<'_>,
    nh: &NalHeader,
    sps: &Sps,
    pps: &Pps,
    h: &mut SliceHeader,
) -> Result<()> {
    r.skip(pps.extra_slice_header_bits as usize)?;
    let st = r.ue()?;
    if st > 2 {
        return Err(Error::InvalidData("slice_type out of range"));
    }
    h.slice_type = st as u8;
    if nh.is_irap() && h.slice_type != slice_type::I {
        return Err(Error::InvalidData("inter slice in an IRAP picture"));
    }
    if pps.output_flag_present {
        h.pic_output = r.u1()? != 0;
    }
    if sps.separate_colour_plane {
        h.colour_plane = r.u(2)? as u8;
        if h.colour_plane > 2 {
            return Err(Error::InvalidData("colour_plane_id out of range"));
        }
    }
    if nh.nal_unit_type != unit_type::IDR_W_RADL && nh.nal_unit_type != unit_type::IDR_N_LP {
        h.poc_lsb = r.u(sps.log2_max_poc_lsb)?;
        parse_rps(r, sps, h)?;
        if sps.temporal_mvp {
            h.temporal_mvp = r.u1()? != 0;
        }
    }
    if sps.sao_enabled {
        h.sao_luma = r.u1()? != 0;
        if sps.chroma_array_type != 0 {
            h.sao_chroma = r.u1()? != 0;
        }
    }
    h.num_pic_total_curr = (h.st_rps.num_used() + h.lt.iter().filter(|e| e.used).count()) as u32;
    if h.is_inter() {
        parse_inter(r, sps, pps, h)?;
    }
    // SliceQpY lies in -QpBdOffsetY..=51 (7.4.7.1); both terms are range
    // checked, so the sum and the QP arithmetic downstream cannot overflow.
    let qp_bd = 6 * (i32::from(sps.bit_depth_y) - 8);
    h.qp = pps.init_qp + crate::ps::pps::se_in(r, -(26 + 48) - 51, 51 + 26 + 48, "slice_qp_delta")?;
    if !(-qp_bd..=51).contains(&h.qp) {
        return Err(Error::InvalidData("SliceQpY out of range"));
    }
    if pps.slice_chroma_qp_offsets_present {
        h.cb_qp_offset = crate::ps::pps::se_in(r, -12, 12, "slice_cb_qp_offset")?;
        h.cr_qp_offset = crate::ps::pps::se_in(r, -12, 12, "slice_cr_qp_offset")?;
    }
    if pps.chroma_qp_offset_list_enabled {
        h.cu_chroma_qp_offset_enabled = r.u1()? != 0;
    }
    h.deblock_disabled = pps.deblock_disabled;
    h.beta_offset_div2 = pps.beta_offset_div2;
    h.tc_offset_div2 = pps.tc_offset_div2;
    let mut overridden = false;
    if pps.deblock_override_enabled {
        overridden = r.u1()? != 0;
    }
    if overridden {
        h.deblock_disabled = r.u1()? != 0;
        if !h.deblock_disabled {
            h.beta_offset_div2 = r.se()?;
            h.tc_offset_div2 = r.se()?;
        }
    }
    if pps.across_slices && (h.sao_luma || h.sao_chroma || !h.deblock_disabled) {
        h.across_slices = r.u1()? != 0;
    }
    Ok(())
}

/// Parses the short-term set choice and the long-term pictures.
fn parse_rps(r: &mut BitReader<'_>, sps: &Sps, h: &mut SliceHeader) -> Result<()> {
    if r.u1()? == 0 {
        h.st_rps = rps::parse(r, sps.num_st_rps, sps.num_st_rps, &sps.st_rps)?;
    } else {
        let mut idx = 0;
        if sps.num_st_rps > 1 {
            idx = r.u(ceil_log2(sps.num_st_rps as usize))?;
        }
        h.st_rps = *sps.st_rps.get(idx as usize).ok_or(Error::InvalidData(
            "short_term_ref_pic_set_idx out of range",
        ))?;
    }
    if !sps.long_term_present {
        return Ok(());
    }
    let n_sps = if sps.num_lt_sps > 0 { r.ue()? } else { 0 };
    let n_pics = r.ue()?;
    if n_sps > sps.num_lt_sps || n_pics > 32 || n_sps + n_pics > 32 {
        return Err(Error::InvalidData("too many long-term reference pictures"));
    }
    let mut prev_cycle = 0u32;
    for i in 0..n_sps + n_pics {
        let mut e = LtEntry::default();
        if i < n_sps {
            let mut idx = 0;
            if sps.num_lt_sps > 1 {
                idx = r.u(ceil_log2(sps.num_lt_sps as usize))?;
            }
            let &(lsb, used) = sps
                .lt_sps
                .get(idx as usize)
                .ok_or(Error::InvalidData("lt_idx_sps out of range"))?;
            e.poc_lsb = lsb;
            e.used = used;
        } else {
            e.poc_lsb = r.u(sps.log2_max_poc_lsb)?;
            e.used = r.u1()? != 0;
        }
        e.msb_present = r.u1()? != 0;
        let mut cycle = 0;
        if e.msb_present {
            cycle = r.ue()?;
        }
        // Equation 7-52: the cycles accumulate within each of the two groups.
        if i != 0 && i != n_sps {
            cycle = cycle.saturating_add(prev_cycle);
        }
        e.msb_cycle = cycle;
        prev_cycle = cycle;
        h.lt.push(e);
    }
    Ok(())
}

/// Parses the P/B-only part of the header.
fn parse_inter(r: &mut BitReader<'_>, sps: &Sps, pps: &Pps, h: &mut SliceHeader) -> Result<()> {
    let b = h.is_b();
    h.num_ref_idx = pps.num_ref_idx_default;
    if !b {
        h.num_ref_idx[1] = 0;
    }
    if r.u1()? != 0 {
        h.num_ref_idx[0] = r.ue()?.saturating_add(1);
        if b {
            h.num_ref_idx[1] = r.ue()?.saturating_add(1);
        }
    }
    if h.num_ref_idx[0] > 15 || h.num_ref_idx[1] > 15 {
        return Err(Error::InvalidData("num_ref_idx_active_minus1 > 14"));
    }
    if h.num_pic_total_curr == 0 {
        return Err(Error::InvalidData("inter slice without reference pictures"));
    }
    if pps.lists_modification_present && h.num_pic_total_curr > 1 {
        let bits = ceil_log2(h.num_pic_total_curr as usize);
        for l in 0..if b { 2 } else { 1 } {
            h.list_modified[l] = r.u1()? != 0;
            if h.list_modified[l] {
                for i in 0..h.num_ref_idx[l] as usize {
                    let e = r.u(bits)?;
                    if e >= h.num_pic_total_curr {
                        return Err(Error::InvalidData("list_entry out of range"));
                    }
                    h.list_entry[l][i] = e as u8;
                }
            }
        }
    }
    if b {
        h.mvd_l1_zero = r.u1()? != 0;
    }
    if pps.cabac_init_present {
        h.cabac_init = r.u1()? != 0;
    }
    if h.temporal_mvp {
        if b {
            h.collocated_from_l0 = r.u1()? != 0;
        }
        let l = if h.collocated_from_l0 { 0 } else { 1 };
        if h.num_ref_idx[l] > 1 {
            h.collocated_ref_idx = r.ue()?;
            if h.collocated_ref_idx >= h.num_ref_idx[l] {
                return Err(Error::InvalidData("collocated_ref_idx out of range"));
            }
        }
    }
    if (pps.weighted_pred && !b) || (pps.weighted_bipred && b) {
        h.weights = Some(Box::new(pred_weight_table(r, sps, h)?));
    }
    let five_minus = r.ue()?;
    if five_minus > 4 {
        return Err(Error::InvalidData("five_minus_max_num_merge_cand > 4"));
    }
    h.max_num_merge_cand = 5 - five_minus;
    Ok(())
}

/// Parses `pred_weight_table()` (7.3.6.3) and derives the factors (7.4.7.3).
fn pred_weight_table(r: &mut BitReader<'_>, sps: &Sps, h: &SliceHeader) -> Result<PredWeights> {
    let mut p = PredWeights::default();
    let luma_denom = r.ue()?;
    if luma_denom > 7 {
        return Err(Error::InvalidData("luma_log2_weight_denom > 7"));
    }
    let chroma = sps.chroma_array_type != 0;
    let mut chroma_denom = luma_denom;
    if chroma {
        let d = luma_denom as i32 + r.se()?;
        if !(0..=7).contains(&d) {
            return Err(Error::InvalidData("ChromaLog2WeightDenom out of range"));
        }
        chroma_denom = d as u32;
    }
    p.log2_denom = [luma_denom, chroma_denom];
    let hp = sps.high_precision_offsets;
    let bd_y = u32::from(sps.bit_depth_y);
    let bd_c = u32::from(sps.bit_depth_c);
    let shift_y = if hp { 0 } else { bd_y - 8 };
    let shift_c = if hp { 0 } else { bd_c - 8 };
    let half_y = 1i32 << if hp { bd_y - 1 } else { 7 };
    let half_c = 1i32 << if hp { bd_c - 1 } else { 7 };
    let lists = if h.is_b() { 2 } else { 1 };
    for l in 0..lists {
        let n = h.num_ref_idx[l] as usize;
        let mut luma_flag = [false; 16];
        let mut chroma_flag = [false; 16];
        for f in luma_flag.iter_mut().take(n) {
            *f = r.u1()? != 0;
        }
        if chroma {
            for f in chroma_flag.iter_mut().take(n) {
                *f = r.u1()? != 0;
            }
        }
        for i in 0..n {
            let wt = &mut p.w[l][i];
            wt.w = [1 << luma_denom, 1 << chroma_denom, 1 << chroma_denom];
            if luma_flag[i] {
                let dw = r.se()?;
                if !(-128..=127).contains(&dw) {
                    return Err(Error::InvalidData("delta_luma_weight out of range"));
                }
                wt.w[0] += dw;
                let o = r.se()?;
                if !(-half_y..half_y).contains(&o) {
                    return Err(Error::InvalidData("luma_offset out of range"));
                }
                wt.o[0] = o << shift_y;
            }
            if chroma_flag[i] {
                for j in 1..3 {
                    let dw = r.se()?;
                    if !(-128..=127).contains(&dw) {
                        return Err(Error::InvalidData("delta_chroma_weight out of range"));
                    }
                    wt.w[j] += dw;
                    let dof = r.se()?;
                    if !(-4 * half_c..4 * half_c).contains(&dof) {
                        return Err(Error::InvalidData("delta_chroma_offset out of range"));
                    }
                    // Equation 7-56.
                    let o = (half_c - ((half_c * wt.w[j]) >> chroma_denom) + dof)
                        .clamp(-half_c, half_c - 1);
                    wt.o[j] = o << shift_c;
                }
            }
        }
    }
    Ok(p)
}

/// `Ceil(Log2(n))`.
pub fn ceil_log2(n: usize) -> u32 {
    if n <= 1 {
        0
    } else {
        usize::BITS - (n - 1).leading_zeros()
    }
}
