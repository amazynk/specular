//! Picture header (7.3.2.8), slice header (7.3.7), reference picture lists (7.3.9) and
//! weighted prediction tables (7.3.8).

use crate::bits::{BitReader, ceil_log2};
use crate::error::{Error, Result};
use crate::nal;
use crate::ps::pps::{DbfOffsets, Layout, Pps};
use crate::ps::sps::{PartitionConstraints, RplEntry, RplStruct, Sps, parse_rpl_struct};

/// One reference picture list as signalled: its structure and the long-term POC fields.
#[derive(Clone, Debug, Default)]
pub struct RefPicList {
    pub rpl: RplStruct,
    /// Per long-term entry: poc_lsb_lt and delta_poc_msb_cycle_lt (when present).
    pub lt: Vec<(u32, Option<u32>)>,
}

impl RefPicList {
    pub fn num_entries(&self) -> usize {
        self.rpl.entries.len()
    }
}

fn parse_ref_pic_lists(r: &mut BitReader<'_>, sps: &Sps, pps: &Pps) -> Result<[RefPicList; 2]> {
    let mut out: [RefPicList; 2] = Default::default();
    let mut sps_flag = [false; 2];
    let mut idx = [0usize; 2];
    for i in 0..2 {
        let nl = sps.num_ref_pic_lists[i];
        if nl > 0 && (i == 0 || pps.rpl1_idx_present) {
            sps_flag[i] = r.flag()?;
        } else if nl > 0 && i == 1 {
            sps_flag[1] = sps_flag[0];
        }
        let st = if sps_flag[i] {
            if nl > 1 && (i == 0 || pps.rpl1_idx_present) {
                idx[i] = r.u(ceil_log2(nl as u32))? as usize;
            } else if i == 1 && nl > 1 {
                idx[1] = idx[0];
            }
            sps.rpl[i]
                .get(idx[i])
                .cloned()
                .ok_or(Error::InvalidData("rpl_idx out of range"))?
        } else {
            parse_rpl_struct(r, sps, i, nl)?
        };
        let mut lt = Vec::new();
        let nlt = st.num_ltrp();
        let lt_from_struct: Vec<Option<u32>> = st
            .entries
            .iter()
            .filter_map(|e| match e {
                RplEntry::Long(v) => Some(*v),
                _ => None,
            })
            .collect();
        for j in 0..nlt {
            let lsb = if st.ltrp_in_header {
                r.u(sps.log2_max_poc_lsb)?
            } else {
                lt_from_struct[j].unwrap_or(0)
            };
            let msb = if r.flag()? { Some(r.ue()?) } else { None };
            lt.push((lsb, msb));
        }
        out[i] = RefPicList { rpl: st, lt };
    }
    Ok(out)
}

/// Explicit weighted prediction parameters (7.3.8), as signalled.
#[derive(Clone, Debug, Default)]
pub struct PredWeights {
    pub luma_log2_denom: u32,
    pub chroma_log2_denom: i32,
    /// Per list, per entry: luma (weight delta, offset) and chroma [(weight delta, offset); 2].
    pub luma: [Vec<Option<(i32, i32)>>; 2],
    pub chroma: [Vec<Option<ChromaWeights>>; 2],
}

/// (weight delta, offset) for Cb and Cr.
pub type ChromaWeights = [(i32, i32); 2];

fn parse_pred_weights(
    r: &mut BitReader<'_>,
    sps: &Sps,
    n: [usize; 2],
    in_ph: bool,
    pps: &Pps,
    rpl1_entries: usize,
) -> Result<PredWeights> {
    let mut w = PredWeights {
        luma_log2_denom: r.ue_max(7, "luma_log2_weight_denom out of range")?,
        ..PredWeights::default()
    };
    if sps.chroma_format_idc != 0 {
        w.chroma_log2_denom = r.se()?;
    }
    let mut counts = n;
    for l in 0..2 {
        if in_ph {
            if l == 0 {
                counts[0] = r.ue_max(15, "num_l0_weights out of range")? as usize;
            } else if pps.weighted_bipred && rpl1_entries > 0 {
                counts[1] = r.ue_max(15, "num_l1_weights out of range")? as usize;
            } else {
                counts[1] = 0;
            }
        }
        let c = counts[l];
        let mut lf = vec![false; c];
        let mut cf = vec![false; c];
        for f in lf.iter_mut() {
            *f = r.flag()?;
        }
        if sps.chroma_format_idc != 0 {
            for f in cf.iter_mut() {
                *f = r.flag()?;
            }
        }
        for i in 0..c {
            w.luma[l].push(if lf[i] {
                Some((r.se()?, r.se()?))
            } else {
                None
            });
            w.chroma[l].push(if cf[i] {
                Some([(r.se()?, r.se()?), (r.se()?, r.se()?)])
            } else {
                None
            });
        }
    }
    Ok(w)
}

/// The picture header, with values the slice headers inherit.
#[derive(Clone, Debug, Default)]
pub struct PictureHeader {
    pub gdr_or_irap: bool,
    pub non_ref_pic: bool,
    pub gdr: bool,
    pub inter_slice_allowed: bool,
    pub intra_slice_allowed: bool,
    pub pps_id: u8,
    pub poc_lsb: u32,
    pub recovery_poc_cnt: u32,
    pub poc_msb_cycle: Option<u32>,
    pub alf: AlfSlice,
    pub lmcs_enabled: bool,
    pub lmcs_aps_id: u8,
    pub chroma_residual_scale: bool,
    pub explicit_scaling_list: bool,
    pub scaling_list_aps_id: u8,
    pub vb_x: Vec<u32>,
    pub vb_y: Vec<u32>,
    pub pic_output: bool,
    pub rpl: Option<[RefPicList; 2]>,
    /// Partition constraints in force: intra luma, intra chroma, inter.
    pub pc: PartitionConstraints,
    pub cu_qp_delta_subdiv: [u32; 2],
    pub cu_chroma_qp_offset_subdiv: [u32; 2],
    pub temporal_mvp: bool,
    pub collocated_from_l0: bool,
    pub collocated_ref_idx: u32,
    pub mmvd_fullpel_only: bool,
    pub mvd_l1_zero: bool,
    pub bdof_disabled: bool,
    pub dmvr_disabled: bool,
    pub prof_disabled: bool,
    pub weights: Option<PredWeights>,
    pub qp_delta: i32,
    pub joint_cbcr_sign: bool,
    pub sao_luma: bool,
    pub sao_chroma: bool,
    pub dbf: DbfOffsets,
}

/// ALF enables and APS ids of a picture or slice.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AlfSlice {
    pub enabled: bool,
    pub luma_aps: Vec<u8>,
    pub cb: bool,
    pub cr: bool,
    pub chroma_aps: u8,
    pub cc_cb: bool,
    pub cc_cb_aps: u8,
    pub cc_cr: bool,
    pub cc_cr_aps: u8,
}

fn parse_alf(r: &mut BitReader<'_>, sps: &Sps) -> Result<AlfSlice> {
    let mut a = AlfSlice {
        enabled: r.flag()?,
        ..AlfSlice::default()
    };
    if a.enabled {
        let n = r.u(3)?;
        for _ in 0..n {
            a.luma_aps.push(r.u(3)? as u8);
        }
        if sps.chroma_format_idc != 0 {
            a.cb = r.flag()?;
            a.cr = r.flag()?;
        }
        if a.cb || a.cr {
            a.chroma_aps = r.u(3)? as u8;
        }
        if sps.ccalf {
            a.cc_cb = r.flag()?;
            if a.cc_cb {
                a.cc_cb_aps = r.u(3)? as u8;
            }
            a.cc_cr = r.flag()?;
            if a.cc_cr {
                a.cc_cr_aps = r.u(3)? as u8;
            }
        }
    }
    Ok(a)
}

fn parse_dbf(r: &mut BitReader<'_>, pps: &Pps, inherit: &DbfOffsets) -> Result<DbfOffsets> {
    let mut d = DbfOffsets {
        disabled: if !pps.dbf.disabled { r.flag()? } else { false },
        ..*inherit
    };
    if !d.disabled {
        let b = r.se_range(-12, 12, "deblocking offset out of range")?;
        let t = r.se_range(-12, 12, "deblocking offset out of range")?;
        d.beta_div2 = [b; 3];
        d.tc_div2 = [t; 3];
        if pps.chroma_tool_offsets_present {
            for c in 1..3 {
                d.beta_div2[c] = r.se_range(-12, 12, "deblocking offset out of range")?;
                d.tc_div2[c] = r.se_range(-12, 12, "deblocking offset out of range")?;
            }
        }
    }
    Ok(d)
}

fn parse_pc(r: &mut BitReader<'_>, pc: &mut PartitionConstraints, i: usize) -> Result<()> {
    pc.log2_diff_min_qt_min_cb[i] = r.ue_max(6, "log2_diff_min_qt_min_cb out of range")?;
    pc.max_mtt_depth[i] = r.ue_max(10, "max_mtt_hierarchy_depth out of range")?;
    pc.log2_diff_max_bt_min_qt[i] = 0;
    pc.log2_diff_max_tt_min_qt[i] = 0;
    if pc.max_mtt_depth[i] != 0 {
        pc.log2_diff_max_bt_min_qt[i] = r.ue_max(6, "log2_diff_max_bt_min_qt out of range")?;
        pc.log2_diff_max_tt_min_qt[i] = r.ue_max(6, "log2_diff_max_tt_min_qt out of range")?;
    }
    Ok(())
}

/// Parses `picture_header_structure()`. `ps` finds the PPS (and its SPS) by id.
pub fn parse_picture_header<'p>(
    r: &mut BitReader<'_>,
    ps: impl Fn(u8) -> Option<(&'p Sps, &'p Pps)>,
) -> Result<PictureHeader> {
    let mut h = PictureHeader {
        gdr_or_irap: r.flag()?,
        non_ref_pic: r.flag()?,
        ..PictureHeader::default()
    };
    if h.gdr_or_irap {
        h.gdr = r.flag()?;
    }
    h.inter_slice_allowed = r.flag()?;
    h.intra_slice_allowed = if h.inter_slice_allowed {
        r.flag()?
    } else {
        true
    };
    h.pps_id = r.ue_max(63, "ph_pic_parameter_set_id out of range")? as u8;
    let (sps, pps) = ps(h.pps_id).ok_or(Error::MissingParameterSet("PPS of a picture"))?;
    h.poc_lsb = r.u(sps.log2_max_poc_lsb)?;
    if h.gdr {
        h.recovery_poc_cnt = r.ue()?;
    }
    r.skip(sps.num_extra_ph_bits as usize)?;
    if sps.poc_msb_cycle && r.flag()? {
        h.poc_msb_cycle = Some(r.u(sps.poc_msb_cycle_len)?);
    }
    if sps.alf && pps.alf_info_in_ph {
        h.alf = parse_alf(r, sps)?;
    }
    if sps.lmcs {
        h.lmcs_enabled = r.flag()?;
        if h.lmcs_enabled {
            h.lmcs_aps_id = r.u(2)? as u8;
            if sps.chroma_format_idc != 0 {
                h.chroma_residual_scale = r.flag()?;
            }
        }
    }
    if sps.explicit_scaling_list {
        h.explicit_scaling_list = r.flag()?;
        if h.explicit_scaling_list {
            h.scaling_list_aps_id = r.u(3)? as u8;
        }
    }
    if sps.virtual_boundaries_enabled && !sps.virtual_boundaries_present {
        if r.flag()? {
            let nv = r.ue_max(3, "too many virtual boundaries")?;
            for _ in 0..nv {
                h.vb_x
                    .push((r.ue_max(1 << 13, "virtual boundary out of range")? + 1) * 8);
            }
            let nh = r.ue_max(3, "too many virtual boundaries")?;
            for _ in 0..nh {
                h.vb_y
                    .push((r.ue_max(1 << 13, "virtual boundary out of range")? + 1) * 8);
            }
        }
    } else if sps.virtual_boundaries_present {
        h.vb_x = sps.vb_x.clone();
        h.vb_y = sps.vb_y.clone();
    }
    h.pic_output = true;
    if pps.output_flag_present && !h.non_ref_pic {
        h.pic_output = r.flag()?;
    }
    if pps.rpl_info_in_ph {
        h.rpl = Some(parse_ref_pic_lists(r, sps, pps)?);
    }
    let override_pc = if sps.partition_constraints_override {
        r.flag()?
    } else {
        false
    };
    h.pc = sps.pc.clone();
    if h.intra_slice_allowed {
        if override_pc {
            parse_pc(r, &mut h.pc, 0)?;
            if sps.qtbtt_dual_tree_intra {
                parse_pc(r, &mut h.pc, 1)?;
            }
        }
        if pps.cu_qp_delta_enabled {
            h.cu_qp_delta_subdiv[0] = r.ue_max(2 * 7, "cu_qp_delta_subdiv out of range")?;
        }
        if pps.cu_chroma_qp_offset_list_enabled {
            h.cu_chroma_qp_offset_subdiv[0] =
                r.ue_max(2 * 7, "chroma QP offset subdiv out of range")?;
        }
    }
    if h.inter_slice_allowed {
        if override_pc {
            parse_pc(r, &mut h.pc, 2)?;
        }
        if pps.cu_qp_delta_enabled {
            h.cu_qp_delta_subdiv[1] = r.ue_max(2 * 7, "cu_qp_delta_subdiv out of range")?;
        }
        if pps.cu_chroma_qp_offset_list_enabled {
            h.cu_chroma_qp_offset_subdiv[1] =
                r.ue_max(2 * 7, "chroma QP offset subdiv out of range")?;
        }
        if sps.temporal_mvp {
            h.temporal_mvp = r.flag()?;
            if h.temporal_mvp && pps.rpl_info_in_ph {
                let rpl = h
                    .rpl
                    .as_ref()
                    .ok_or(Error::InvalidData("no reference lists"))?;
                h.collocated_from_l0 = true;
                if rpl[1].num_entries() > 0 {
                    h.collocated_from_l0 = r.flag()?;
                }
                let n = if h.collocated_from_l0 {
                    rpl[0].num_entries()
                } else {
                    rpl[1].num_entries()
                };
                if n > 1 {
                    h.collocated_ref_idx = r.ue()?;
                }
            }
        }
        if sps.mmvd_fullpel_only {
            h.mmvd_fullpel_only = r.flag()?;
        }
        let presence = match &h.rpl {
            None => true,
            Some(l) => l[1].num_entries() > 0,
        };
        if presence {
            h.mvd_l1_zero = r.flag()?;
            if sps.bdof_control_in_ph {
                h.bdof_disabled = r.flag()?;
            }
            if sps.dmvr_control_in_ph {
                h.dmvr_disabled = r.flag()?;
            }
        }
        if sps.prof_control_in_ph {
            h.prof_disabled = r.flag()?;
        }
        if (pps.weighted_pred || pps.weighted_bipred) && pps.wp_info_in_ph {
            let n1 = h.rpl.as_ref().map(|l| l[1].num_entries()).unwrap_or(0);
            h.weights = Some(parse_pred_weights(r, sps, [0, 0], true, pps, n1)?);
        }
    }
    if !sps.bdof_control_in_ph {
        h.bdof_disabled = !sps.bdof;
    }
    if !sps.dmvr_control_in_ph {
        h.dmvr_disabled = !sps.dmvr;
    }
    if !sps.prof_control_in_ph {
        h.prof_disabled = !sps.affine_prof;
    }
    if pps.qp_delta_info_in_ph {
        h.qp_delta = r.se()?;
    }
    if sps.joint_cbcr {
        h.joint_cbcr_sign = r.flag()?;
    }
    if sps.sao && pps.sao_info_in_ph {
        h.sao_luma = r.flag()?;
        if sps.chroma_format_idc != 0 {
            h.sao_chroma = r.flag()?;
        }
    }
    h.dbf = pps.dbf;
    if pps.dbf_info_in_ph && r.flag()? {
        h.dbf = parse_dbf(r, pps, &pps.dbf)?;
    }
    if pps.picture_header_extension_present {
        let n = r.ue_max(256, "picture header extension too long")?;
        r.skip(8 * n as usize)?;
    }
    Ok(h)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliceType {
    B,
    P,
    I,
}

/// A slice header with every inherited value resolved.
#[derive(Clone, Debug)]
pub struct SliceHeader {
    pub ph_in_sh: bool,
    pub subpic_idx: usize,
    pub slice_address: u32,
    pub num_tiles_in_slice: u32,
    pub slice_type: SliceType,
    pub no_output_of_prior_pics: bool,
    pub alf: AlfSlice,
    pub lmcs_used: bool,
    pub explicit_scaling_list_used: bool,
    pub rpl: Option<[RefPicList; 2]>,
    pub num_ref_idx_active: [u32; 2],
    pub cabac_init: bool,
    pub collocated_from_l0: bool,
    pub collocated_ref_idx: u32,
    pub weights: Option<PredWeights>,
    pub qp_y: i32,
    pub cb_qp_offset: i32,
    pub cr_qp_offset: i32,
    pub joint_cbcr_qp_offset: i32,
    pub cu_chroma_qp_offset_enabled: bool,
    pub sao_luma: bool,
    pub sao_chroma: bool,
    pub dbf: DbfOffsets,
    pub dep_quant: bool,
    pub sign_data_hiding: bool,
    pub ts_residual_coding_disabled: bool,
    pub ts_rice_idx: u32,
    pub reverse_last_sig_coeff: bool,
    /// The CTBs of the slice, in decoding order (picture raster addresses).
    pub ctbs: Vec<u32>,
    /// Byte offsets of the entry points, relative to the first byte of the slice data.
    pub entry_points: Vec<usize>,
    /// Byte offset of the slice data in the RBSP.
    pub data_offset: usize,
}

impl SliceHeader {
    pub fn init_type(&self) -> usize {
        match self.slice_type {
            SliceType::I => 0,
            SliceType::P => {
                if self.cabac_init {
                    2
                } else {
                    1
                }
            }
            SliceType::B => {
                if self.cabac_init {
                    1
                } else {
                    2
                }
            }
        }
    }
}

/// Parses a slice header. `ph` is the picture header in force (from a PH NAL unit), or None
/// when it must be in the slice header; the parsed picture header is returned in that case.
#[allow(clippy::too_many_arguments)]
pub fn parse_slice_header<'p>(
    r: &mut BitReader<'_>,
    nal_type: u8,
    ph_in: Option<&PictureHeader>,
    ps: impl Fn(u8) -> Option<(&'p Sps, &'p Pps)> + Copy,
    layout_of: impl Fn(u8) -> Option<&'p Layout>,
) -> Result<(SliceHeader, Option<PictureHeader>)> {
    let ph_in_sh = r.flag()?;
    let own_ph;
    let ph: &PictureHeader = if ph_in_sh {
        own_ph = Some(parse_picture_header(r, ps)?);
        own_ph
            .as_ref()
            .ok_or(Error::InvalidData("no picture header"))?
    } else {
        own_ph = None;
        ph_in.ok_or(Error::InvalidData("slice without a picture header"))?
    };
    let (sps, pps) = ps(ph.pps_id).ok_or(Error::MissingParameterSet("PPS of a slice"))?;
    let layout = layout_of(ph.pps_id).ok_or(Error::MissingParameterSet("PPS of a slice"))?;
    let mut subpic_idx = 0usize;
    if sps.subpic_info_present {
        let id = r.u(sps.subpic_id_len)?;
        // SubpicIdVal: PPS mapping, then SPS mapping, else the index itself.
        let ids: Vec<u32> = if pps.subpic_id_mapping_present {
            pps.subpic_ids.clone()
        } else if sps.subpic_id_mapping_explicit {
            sps.subpic_ids.clone()
        } else {
            (0..sps.subpics.len() as u32).collect()
        };
        subpic_idx = ids
            .iter()
            .position(|&v| v == id)
            .ok_or(Error::InvalidData("unknown sh_subpic_id"))?;
    }
    let num_tiles = layout.num_tiles();
    let mut slice_address = 0;
    if pps.rect_slice {
        let n = *layout.subpic_slice_count.get(subpic_idx).unwrap_or(&1);
        if n > 1 {
            slice_address = r.u(ceil_log2(n as u32))?;
        }
    } else if num_tiles > 1 {
        slice_address = r.u(ceil_log2(num_tiles))?;
    }
    r.skip(sps.num_extra_sh_bits as usize)?;
    let mut num_tiles_in_slice = 1;
    if !pps.rect_slice && num_tiles.saturating_sub(slice_address) > 1 {
        num_tiles_in_slice = r.ue_max(num_tiles - 1, "too many tiles in a slice")? + 1;
    }
    let slice_type = if ph.inter_slice_allowed {
        match r.ue_max(2, "sh_slice_type out of range")? {
            0 => SliceType::B,
            1 => SliceType::P,
            _ => SliceType::I,
        }
    } else {
        SliceType::I
    };
    let mut no_output_of_prior_pics = false;
    if matches!(
        nal_type,
        nal::IDR_W_RADL | nal::IDR_N_LP | nal::CRA | nal::GDR
    ) {
        no_output_of_prior_pics = r.flag()?;
    }
    let alf = if sps.alf && !pps.alf_info_in_ph {
        parse_alf(r, sps)?
    } else {
        ph.alf.clone()
    };
    let lmcs_used = if ph.lmcs_enabled && !ph_in_sh {
        r.flag()?
    } else {
        ph_in_sh && ph.lmcs_enabled
    };
    let explicit_scaling_list_used = if ph.explicit_scaling_list && !ph_in_sh {
        r.flag()?
    } else {
        ph_in_sh && ph.explicit_scaling_list
    };
    let idr = nal_type == nal::IDR_W_RADL || nal_type == nal::IDR_N_LP;
    let mut rpl = ph.rpl.clone();
    if !pps.rpl_info_in_ph && (!idr || sps.idr_rpl_present) {
        rpl = Some(parse_ref_pic_lists(r, sps, pps)?);
    }
    let entries = |i: usize| rpl.as_ref().map(|l| l[i].num_entries()).unwrap_or(0);
    let mut num_ref_idx_active = [0u32; 2];
    let mut override_minus1: [Option<u32>; 2] = [None, None];
    if ((slice_type != SliceType::I && entries(0) > 1)
        || (slice_type == SliceType::B && entries(1) > 1))
        && r.flag()?
    {
        {
            let n = if slice_type == SliceType::B { 2 } else { 1 };
            for (i, o) in override_minus1.iter_mut().enumerate().take(n) {
                if entries(i) > 1 {
                    *o = Some(r.ue_max(14, "num_ref_idx_active out of range")?);
                } else {
                    *o = Some(0);
                }
            }
        }
    }
    for (i, a) in num_ref_idx_active.iter_mut().enumerate() {
        let used = slice_type == SliceType::B || (slice_type == SliceType::P && i == 0);
        if used {
            *a = match override_minus1[i] {
                Some(v) => v + 1,
                None => {
                    let e = entries(i) as u32;
                    if e >= pps.num_ref_idx_default_active[i] {
                        pps.num_ref_idx_default_active[i]
                    } else {
                        e
                    }
                }
            };
        }
    }
    let mut cabac_init = false;
    let mut collocated_from_l0 = ph.collocated_from_l0;
    let mut collocated_ref_idx = ph.collocated_ref_idx;
    let mut weights = ph.weights.clone();
    if slice_type != SliceType::I {
        if pps.cabac_init_present {
            cabac_init = r.flag()?;
        }
        if ph.temporal_mvp && !pps.rpl_info_in_ph {
            collocated_from_l0 = true;
            if slice_type == SliceType::B {
                collocated_from_l0 = r.flag()?;
            }
            collocated_ref_idx = 0;
            if (collocated_from_l0 && num_ref_idx_active[0] > 1)
                || (!collocated_from_l0 && num_ref_idx_active[1] > 1)
            {
                collocated_ref_idx = r.ue()?;
            }
        }
        if !pps.wp_info_in_ph
            && ((pps.weighted_pred && slice_type == SliceType::P)
                || (pps.weighted_bipred && slice_type == SliceType::B))
        {
            let n1 = if slice_type == SliceType::B && pps.weighted_bipred {
                num_ref_idx_active[1] as usize
            } else {
                0
            };
            weights = Some(parse_pred_weights(
                r,
                sps,
                [num_ref_idx_active[0] as usize, n1],
                false,
                pps,
                entries(1),
            )?);
        }
    }
    let qp_delta = if !pps.qp_delta_info_in_ph {
        r.se()?
    } else {
        ph.qp_delta
    };
    let qp_y = pps.init_qp + qp_delta;
    if qp_y < -sps.qp_bd_offset() || qp_y > 63 {
        return Err(Error::InvalidData("SliceQpY out of range"));
    }
    let (mut cb, mut cr, mut jc) = (0, 0, 0);
    if pps.slice_chroma_qp_offsets_present {
        cb = r.se_range(-12, 12, "sh_cb_qp_offset out of range")?;
        cr = r.se_range(-12, 12, "sh_cr_qp_offset out of range")?;
        if sps.joint_cbcr {
            jc = r.se_range(-12, 12, "sh_joint_cbcr_qp_offset out of range")?;
        }
    }
    let mut cu_chroma_qp_offset_enabled = false;
    if pps.cu_chroma_qp_offset_list_enabled {
        cu_chroma_qp_offset_enabled = r.flag()?;
    }
    let (mut sao_luma, mut sao_chroma) = (ph.sao_luma, ph.sao_chroma);
    if sps.sao && !pps.sao_info_in_ph {
        sao_luma = r.flag()?;
        sao_chroma = false;
        if sps.chroma_format_idc != 0 {
            sao_chroma = r.flag()?;
        }
    }
    let mut dbf = ph.dbf;
    if pps.deblocking_override_enabled && !pps.dbf_info_in_ph && r.flag()? {
        dbf = parse_dbf(r, pps, &ph.dbf)?;
    }
    let mut dep_quant = false;
    if sps.dep_quant {
        dep_quant = r.flag()?;
    }
    let mut sign_data_hiding = false;
    if sps.sign_data_hiding && !dep_quant {
        sign_data_hiding = r.flag()?;
    }
    let mut ts_residual_coding_disabled = false;
    if sps.transform_skip && !dep_quant && !sign_data_hiding {
        ts_residual_coding_disabled = r.flag()?;
    }
    let mut ts_rice_idx = 1;
    if !ts_residual_coding_disabled && sps.ts_residual_coding_rice_present_in_sh {
        ts_rice_idx = r.u(3)? + 1;
    }
    let mut reverse_last_sig_coeff = false;
    if sps.reverse_last_sig_coeff {
        reverse_last_sig_coeff = r.flag()?;
    }
    if pps.slice_header_extension_present {
        let n = r.ue_max(256, "slice header extension too long")?;
        r.skip(8 * n as usize)?;
    }
    // CtbAddrInCurrSlice (equation 113).
    let ctbs: Vec<u32> = if pps.rect_slice {
        let mut idx = slice_address as usize;
        for j in 0..subpic_idx {
            idx += layout.subpic_slice_count.get(j).copied().unwrap_or(0);
        }
        layout
            .slice_ctbs
            .get(idx)
            .cloned()
            .ok_or(Error::InvalidData("sh_slice_address out of range"))?
    } else {
        if slice_address + num_tiles_in_slice > num_tiles {
            return Err(Error::InvalidData("slice runs past the last tile"));
        }
        let mut v = Vec::new();
        for t in slice_address..slice_address + num_tiles_in_slice {
            layout.tile_ctbs(t, &mut v);
        }
        v
    };
    // NumEntryPoints (equation 141).
    let mut num_entry_points = 0;
    if sps.entry_point_offsets_present {
        for i in 1..ctbs.len() {
            let (x, y) = (ctbs[i] % layout.w_ctbs, ctbs[i] / layout.w_ctbs);
            let (px, py) = (ctbs[i - 1] % layout.w_ctbs, ctbs[i - 1] / layout.w_ctbs);
            if layout.ctb_to_row[y as usize] != layout.ctb_to_row[py as usize]
                || layout.ctb_to_col[x as usize] != layout.ctb_to_col[px as usize]
                || (y != py && sps.entropy_coding_sync)
            {
                num_entry_points += 1;
            }
        }
    }
    let mut entry_points = Vec::with_capacity(num_entry_points);
    if num_entry_points > 0 {
        let len = r.ue_max(31, "sh_entry_offset_len_minus1 out of range")? + 1;
        let mut acc = 0usize;
        for _ in 0..num_entry_points {
            acc += r.u(len)? as usize + 1;
            entry_points.push(acc);
        }
    }
    r.byte_alignment()?;
    let sh = SliceHeader {
        ph_in_sh,
        subpic_idx,
        slice_address,
        num_tiles_in_slice,
        slice_type,
        no_output_of_prior_pics,
        alf,
        lmcs_used,
        explicit_scaling_list_used,
        rpl,
        num_ref_idx_active,
        cabac_init,
        collocated_from_l0,
        collocated_ref_idx,
        weights,
        qp_y,
        cb_qp_offset: cb,
        cr_qp_offset: cr,
        joint_cbcr_qp_offset: jc,
        cu_chroma_qp_offset_enabled,
        sao_luma,
        sao_chroma,
        dbf,
        dep_quant,
        sign_data_hiding,
        ts_residual_coding_disabled,
        ts_rice_idx,
        reverse_last_sig_coeff,
        ctbs,
        entry_points,
        data_offset: r.byte_pos(),
    };
    Ok((sh, own_ph))
}
