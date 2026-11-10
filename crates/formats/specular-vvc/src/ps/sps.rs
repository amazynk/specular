//! Sequence parameter set (7.3.2.4), with the profile, DPB, HRD and VUI structures it carries.

use crate::bits::{BitReader, ceil_log2};
use crate::error::{Error, Result};

/// One `ref_pic_list_struct()` (7.3.10).
#[derive(Clone, Debug, Default)]
pub struct RplStruct {
    pub ltrp_in_header: bool,
    /// Per entry: short-term delta POC (signed), long-term POC LSB, or an inter-layer index.
    pub entries: Vec<RplEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RplEntry {
    /// DeltaPocValSt.
    Short(i32),
    /// rpls_poc_lsb_lt, or None when signalled in the header.
    Long(Option<u32>),
    InterLayer(u32),
}

impl RplStruct {
    pub fn num_ltrp(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, RplEntry::Long(_)))
            .count()
    }
}

/// Parses `ref_pic_list_struct( listIdx, rplsIdx )`.
pub fn parse_rpl_struct(
    r: &mut BitReader<'_>,
    sps: &Sps,
    list: usize,
    idx: usize,
) -> Result<RplStruct> {
    let n = r.ue_max(64, "num_ref_entries out of range")? as usize;
    let mut out = RplStruct::default();
    if sps.long_term_ref_pics && idx < sps.num_ref_pic_lists[list] && n > 0 {
        out.ltrp_in_header = r.flag()?;
    }
    let lsb_bits = sps.log2_max_poc_lsb;
    for _ in 0..n {
        let ilrp = if sps.inter_layer_prediction {
            r.flag()?
        } else {
            false
        };
        if ilrp {
            out.entries.push(RplEntry::InterLayer(r.ue()?));
            continue;
        }
        let st = if sps.long_term_ref_pics {
            r.flag()?
        } else {
            true
        };
        if st {
            let abs = r.ue_max(1 << 15, "abs_delta_poc_st out of range")? as i32;
            // AbsDeltaPocSt: abs + 1 unless the entry is a repeat allowed by weighted prediction
            // (7.4.11): abs_delta_poc_st + 1 when !(wp && i > 0), else abs_delta_poc_st.
            let first_or_no_wp =
                !(sps.weighted_pred || sps.weighted_bipred) || out.entries.is_empty();
            let abs_val = if first_or_no_wp { abs + 1 } else { abs };
            let neg = if abs_val > 0 { r.flag()? } else { false };
            out.entries
                .push(RplEntry::Short(if neg { -abs_val } else { abs_val }));
        } else if !out.ltrp_in_header {
            out.entries.push(RplEntry::Long(Some(r.u(lsb_bits)?)));
        } else {
            out.entries.push(RplEntry::Long(None));
        }
    }
    Ok(out)
}

/// Video signal type from the VUI (Rec. ITU-T H.274).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signal {
    pub colour_primaries: u8,
    pub transfer_characteristics: u8,
    pub matrix_coeffs: u8,
    pub full_range: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Subpic {
    /// Top-left CTU and size, in CTUs.
    pub ctu_x: u32,
    pub ctu_y: u32,
    pub w: u32,
    pub h: u32,
    pub treated_as_pic: bool,
    pub loop_filter_across: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PartitionConstraints {
    pub log2_diff_min_qt_min_cb: [u32; 3],
    pub max_mtt_depth: [u32; 3],
    pub log2_diff_max_bt_min_qt: [u32; 3],
    pub log2_diff_max_tt_min_qt: [u32; 3],
}

/// The SPS, with the derived variables the decoder uses.
#[derive(Clone, Debug, Default)]
pub struct Sps {
    pub id: u8,
    pub vps_id: u8,
    pub max_sublayers_minus1: u8,
    pub chroma_format_idc: u8,
    pub log2_ctu_size: u32,
    pub general_profile_idc: u8,
    pub gdr_enabled: bool,
    pub ref_pic_resampling: bool,
    pub res_change_in_clvs: bool,
    pub pic_width_max: u32,
    pub pic_height_max: u32,
    pub conf_win: [u32; 4],
    pub subpic_info_present: bool,
    pub independent_subpics: bool,
    pub subpics: Vec<Subpic>,
    pub subpic_id_len: u32,
    pub subpic_id_mapping_explicit: bool,
    pub subpic_id_mapping_present: bool,
    pub subpic_ids: Vec<u32>,
    pub bit_depth: u32,
    pub entropy_coding_sync: bool,
    pub entry_point_offsets_present: bool,
    pub log2_max_poc_lsb: u32,
    pub poc_msb_cycle: bool,
    pub poc_msb_cycle_len: u32,
    pub num_extra_ph_bits: u32,
    pub num_extra_sh_bits: u32,
    pub max_dec_pic_buffering: u32,
    pub max_num_reorder: u32,
    pub log2_min_cb_size: u32,
    pub partition_constraints_override: bool,
    /// Index 0: intra luma, 1: intra chroma, 2: inter.
    pub pc: PartitionConstraints,
    pub qtbtt_dual_tree_intra: bool,
    pub max_luma_tb_64: bool,
    pub transform_skip: bool,
    pub log2_ts_max_size: u32,
    pub bdpcm: bool,
    pub mts: bool,
    pub explicit_mts_intra: bool,
    pub explicit_mts_inter: bool,
    pub lfnst: bool,
    pub joint_cbcr: bool,
    /// ChromaQpTable[i][qp + QpBdOffset] for qp in -QpBdOffset..=63.
    pub chroma_qp_table: [Vec<i8>; 3],
    pub sao: bool,
    pub alf: bool,
    pub ccalf: bool,
    pub lmcs: bool,
    pub weighted_pred: bool,
    pub weighted_bipred: bool,
    pub long_term_ref_pics: bool,
    pub inter_layer_prediction: bool,
    pub idr_rpl_present: bool,
    pub rpl1_same_as_rpl0: bool,
    pub num_ref_pic_lists: [usize; 2],
    pub rpl: [Vec<RplStruct>; 2],
    pub ref_wraparound: bool,
    pub temporal_mvp: bool,
    pub sbtmvp: bool,
    pub amvr: bool,
    pub bdof: bool,
    pub bdof_control_in_ph: bool,
    pub smvd: bool,
    pub dmvr: bool,
    pub dmvr_control_in_ph: bool,
    pub mmvd: bool,
    pub mmvd_fullpel_only: bool,
    pub max_num_merge_cand: u32,
    pub sbt: bool,
    pub affine: bool,
    pub max_num_subblock_merge_cand: u32,
    pub six_param_affine: bool,
    pub affine_amvr: bool,
    pub affine_prof: bool,
    pub prof_control_in_ph: bool,
    pub bcw: bool,
    pub ciip: bool,
    pub gpm: bool,
    pub max_num_gpm_merge_cand: u32,
    pub log2_parallel_merge_level: u32,
    pub isp: bool,
    pub mrl: bool,
    pub mip: bool,
    pub cclm: bool,
    pub chroma_horizontal_collocated: bool,
    pub chroma_vertical_collocated: bool,
    pub palette: bool,
    pub act: bool,
    pub min_qp_prime_ts: i32,
    pub ibc: bool,
    pub max_num_ibc_merge_cand: u32,
    pub ladf: bool,
    pub num_ladf_intervals: usize,
    pub ladf_lowest_interval_qp_offset: i32,
    pub ladf_qp_offset: [i32; 5],
    /// SpsLadfIntervalLowerBound[ i ].
    pub ladf_lower_bound: [i32; 5],
    pub explicit_scaling_list: bool,
    pub scaling_matrix_for_lfnst_disabled: bool,
    pub scaling_matrix_for_alt_colour_space_disabled: bool,
    pub scaling_matrix_designated_colour_space: bool,
    pub dep_quant: bool,
    pub sign_data_hiding: bool,
    pub virtual_boundaries_enabled: bool,
    pub virtual_boundaries_present: bool,
    pub vb_x: Vec<u32>,
    pub vb_y: Vec<u32>,
    pub field_seq: bool,
    pub signal: Option<Signal>,
    // Range extension (refused when used).
    pub extended_precision: bool,
    pub ts_residual_coding_rice_present_in_sh: bool,
    pub rrc_rice_extension: bool,
    pub persistent_rice_adaptation: bool,
    pub reverse_last_sig_coeff: bool,
}

impl Sps {
    pub fn ctb_size(&self) -> u32 {
        1 << self.log2_ctu_size
    }
    pub fn sub_width_c(&self) -> u32 {
        if matches!(self.chroma_format_idc, 1 | 2) {
            2
        } else {
            1
        }
    }
    pub fn sub_height_c(&self) -> u32 {
        if self.chroma_format_idc == 1 { 2 } else { 1 }
    }
    pub fn qp_bd_offset(&self) -> i32 {
        6 * (self.bit_depth as i32 - 8)
    }
    /// ChromaQpTable[ i ][ qp ] for qp in -QpBdOffset..=63.
    pub fn chroma_qp(&self, table: usize, qp: i32) -> i32 {
        let off = self.qp_bd_offset();
        let q = qp.clamp(-off, 63);
        i32::from(self.chroma_qp_table[table][(q + off) as usize])
    }
}

/// Parses `profile_tier_level( profileTierPresentFlag, MaxNumSubLayersMinus1 )`; returns
/// general_profile_idc (0 when not present).
pub fn parse_ptl(r: &mut BitReader<'_>, profile_present: bool, max_sub: u32) -> Result<u8> {
    let mut profile = 0;
    if profile_present {
        profile = r.u(7)? as u8;
        r.u(1)?; // tier
    }
    r.u(8)?; // level
    r.u(2)?; // frame_only, multilayer
    if profile_present {
        // general_constraints_info()
        if r.flag()? {
            r.skip(71)?;
            let extra = r.u(8)? as usize;
            r.skip(extra)?;
        }
        r.align_zero()?;
    }
    let mut present = [false; 8];
    for i in (0..max_sub as usize).rev() {
        present[i] = r.flag()?;
    }
    r.align_zero()?;
    for i in (0..max_sub as usize).rev() {
        if present[i] {
            r.u(8)?;
        }
    }
    if profile_present {
        let n = r.u(8)?;
        for _ in 0..n {
            r.u(32)?;
        }
    }
    Ok(profile)
}

struct Hrd {
    nal: bool,
    vcl: bool,
    du: bool,
    cpb_cnt: u32,
}

fn parse_general_hrd(r: &mut BitReader<'_>) -> Result<Hrd> {
    r.u(32)?;
    r.u(32)?;
    let nal = r.flag()?;
    let vcl = r.flag()?;
    let mut h = Hrd {
        nal,
        vcl,
        du: false,
        cpb_cnt: 0,
    };
    if nal || vcl {
        r.u(1)?;
        h.du = r.flag()?;
        if h.du {
            r.u(8)?;
        }
        r.u(8)?;
        if h.du {
            r.u(4)?;
        }
        h.cpb_cnt = r.ue_max(31, "hrd_cpb_cnt_minus1 out of range")?;
    }
    Ok(h)
}

fn parse_ols_hrd(r: &mut BitReader<'_>, h: &Hrd, first: u32, max: u32) -> Result<()> {
    for _ in first..=max {
        let fixed_general = r.flag()?;
        let fixed_cvs = if !fixed_general { r.flag()? } else { true };
        if fixed_cvs {
            r.ue()?;
        } else if (h.nal || h.vcl) && h.cpb_cnt == 0 {
            r.u(1)?;
        }
        for present in [h.nal, h.vcl] {
            if present {
                for _ in 0..=h.cpb_cnt {
                    r.ue()?;
                    r.ue()?;
                    if h.du {
                        r.ue()?;
                        r.ue()?;
                    }
                    r.u(1)?;
                }
            }
        }
    }
    Ok(())
}

fn parse_vui(r: &mut BitReader<'_>, size: usize) -> Result<Option<Signal>> {
    // vui_parameters() of Rec. ITU-T H.274; only the colour description is kept.
    let start = r.pos();
    let mut sub = r.clone();
    let res = (|| -> Result<Option<Signal>> {
        sub.u(4)?; // progressive, interlaced, non_packed, non_projected
        if sub.flag()? {
            sub.u(1)?;
            if sub.u(8)? == 255 {
                sub.u(32)?;
            }
        }
        if sub.flag()? {
            sub.u(1)?;
        }
        if sub.flag()? {
            let cp = sub.u(8)? as u8;
            let tc = sub.u(8)? as u8;
            let mc = sub.u(8)? as u8;
            let full = sub.flag()?;
            return Ok(Some(Signal {
                colour_primaries: cp,
                transfer_characteristics: tc,
                matrix_coeffs: mc,
                full_range: full,
            }));
        }
        Ok(None)
    })();
    r.skip(start + size * 8 - r.pos())?;
    Ok(res.unwrap_or(None))
}

fn parse_partition(r: &mut BitReader<'_>, pc: &mut PartitionConstraints, i: usize) -> Result<()> {
    pc.log2_diff_min_qt_min_cb[i] = r.ue_max(6, "log2_diff_min_qt_min_cb out of range")?;
    pc.max_mtt_depth[i] = r.ue_max(10, "max_mtt_hierarchy_depth out of range")?;
    if pc.max_mtt_depth[i] != 0 {
        pc.log2_diff_max_bt_min_qt[i] = r.ue_max(6, "log2_diff_max_bt_min_qt out of range")?;
        pc.log2_diff_max_tt_min_qt[i] = r.ue_max(6, "log2_diff_max_tt_min_qt out of range")?;
    }
    Ok(())
}

pub fn parse_sps(rbsp: &[u8]) -> Result<Sps> {
    let mut r = BitReader::new(rbsp);
    let r = &mut r;
    let mut s = Sps {
        id: r.u(4)? as u8,
        vps_id: r.u(4)? as u8,
        max_sublayers_minus1: r.u(3)? as u8,
        chroma_format_idc: r.u(2)? as u8,
        log2_ctu_size: r.u(2)? + 5,
        ..Sps::default()
    };
    if s.max_sublayers_minus1 > 6 {
        return Err(Error::InvalidData("sps_max_sublayers_minus1 above 6"));
    }
    if s.log2_ctu_size > 7 {
        return Err(Error::InvalidData("CTU size above 128"));
    }
    let ptl_dpb_hrd = r.flag()?;
    if ptl_dpb_hrd {
        s.general_profile_idc = parse_ptl(r, true, u32::from(s.max_sublayers_minus1))?;
    }
    s.gdr_enabled = r.flag()?;
    s.ref_pic_resampling = r.flag()?;
    if s.ref_pic_resampling {
        s.res_change_in_clvs = r.flag()?;
    }
    s.pic_width_max = r.ue_max(1 << 16, "picture width out of range")?;
    s.pic_height_max = r.ue_max(1 << 16, "picture height out of range")?;
    if s.pic_width_max == 0 || s.pic_height_max == 0 {
        return Err(Error::InvalidData("zero picture size"));
    }
    if r.flag()? {
        for v in s.conf_win.iter_mut() {
            *v = r.ue_max(1 << 16, "conformance window out of range")?;
        }
    }
    let ctb = s.ctb_size();
    let tmp_w = s.pic_width_max.div_ceil(ctb);
    let tmp_h = s.pic_height_max.div_ceil(ctb);
    s.subpic_info_present = r.flag()?;
    s.independent_subpics = true;
    if s.subpic_info_present {
        let n = r.ue_max(599, "sps_num_subpics_minus1 out of range")? as usize + 1;
        let mut same = false;
        if n > 1 {
            s.independent_subpics = r.flag()?;
            same = r.flag()?;
        }
        let wb = ceil_log2(tmp_w);
        let hb = ceil_log2(tmp_h);
        let mut subs: Vec<Subpic> = Vec::with_capacity(n);
        for i in 0..n {
            let mut sp = Subpic::default();
            if n > 1 {
                if !same || i == 0 {
                    if i > 0 && s.pic_width_max > ctb {
                        sp.ctu_x = r.u(wb)?;
                    }
                    if i > 0 && s.pic_height_max > ctb {
                        sp.ctu_y = r.u(hb)?;
                    }
                    sp.w = if i < n - 1 && s.pic_width_max > ctb {
                        r.u(wb)? + 1
                    } else {
                        tmp_w.saturating_sub(sp.ctu_x)
                    };
                    sp.h = if i < n - 1 && s.pic_height_max > ctb {
                        r.u(hb)? + 1
                    } else {
                        tmp_h.saturating_sub(sp.ctu_y)
                    };
                } else {
                    let w0 = subs[0].w.max(1);
                    let h0 = subs[0].h.max(1);
                    let cols = (tmp_w / w0).max(1);
                    sp.ctu_x = (i as u32 % cols) * w0;
                    sp.ctu_y = (i as u32 / cols) * h0;
                    sp.w = w0;
                    sp.h = h0;
                }
                if !s.independent_subpics {
                    sp.treated_as_pic = r.flag()?;
                    sp.loop_filter_across = r.flag()?;
                } else {
                    sp.treated_as_pic = true;
                }
            } else {
                sp.w = tmp_w;
                sp.h = tmp_h;
                sp.treated_as_pic = true;
            }
            if sp.w == 0 || sp.h == 0 || sp.ctu_x + sp.w > tmp_w || sp.ctu_y + sp.h > tmp_h {
                return Err(Error::InvalidData("subpicture outside the picture"));
            }
            subs.push(sp);
        }
        s.subpics = subs;
        s.subpic_id_len = r.ue_max(15, "sps_subpic_id_len_minus1 out of range")? + 1;
        s.subpic_id_mapping_explicit = r.flag()?;
        if s.subpic_id_mapping_explicit {
            s.subpic_id_mapping_present = r.flag()?;
            if s.subpic_id_mapping_present {
                for _ in 0..n {
                    s.subpic_ids.push(r.u(s.subpic_id_len)?);
                }
            }
        }
    } else {
        s.subpics.push(Subpic {
            ctu_x: 0,
            ctu_y: 0,
            w: tmp_w,
            h: tmp_h,
            treated_as_pic: true,
            loop_filter_across: false,
        });
    }
    s.bit_depth = r.ue_max(8, "sps_bitdepth_minus8 out of range")? + 8;
    s.entropy_coding_sync = r.flag()?;
    s.entry_point_offsets_present = r.flag()?;
    s.log2_max_poc_lsb = r.u(4)? + 4;
    if s.log2_max_poc_lsb > 16 {
        return Err(Error::InvalidData(
            "sps_log2_max_pic_order_cnt_lsb_minus4 above 12",
        ));
    }
    s.poc_msb_cycle = r.flag()?;
    if s.poc_msb_cycle {
        s.poc_msb_cycle_len = r.ue_max(31, "poc_msb_cycle_len out of range")? + 1;
    }
    let extra_ph = r.u(2)? * 8;
    for _ in 0..extra_ph {
        if r.flag()? {
            s.num_extra_ph_bits += 1;
        }
    }
    let extra_sh = r.u(2)? * 8;
    for _ in 0..extra_sh {
        if r.flag()? {
            s.num_extra_sh_bits += 1;
        }
    }
    if ptl_dpb_hrd {
        let sub_info = if s.max_sublayers_minus1 > 0 {
            r.flag()?
        } else {
            false
        };
        let first = if sub_info { 0 } else { s.max_sublayers_minus1 };
        for _ in first..=s.max_sublayers_minus1 {
            s.max_dec_pic_buffering = r.ue_max(64, "dpb size out of range")? + 1;
            s.max_num_reorder = r.ue_max(64, "dpb reorder out of range")?;
            r.ue()?;
        }
    }
    s.log2_min_cb_size = r.ue_max(5, "min coding block size out of range")? + 2;
    if s.log2_min_cb_size > s.log2_ctu_size {
        return Err(Error::InvalidData(
            "minimum coding block larger than the CTU",
        ));
    }
    s.partition_constraints_override = r.flag()?;
    parse_partition(r, &mut s.pc, 0)?;
    if s.chroma_format_idc != 0 {
        s.qtbtt_dual_tree_intra = r.flag()?;
    }
    if s.qtbtt_dual_tree_intra {
        parse_partition(r, &mut s.pc, 1)?;
    }
    parse_partition(r, &mut s.pc, 2)?;
    if ctb > 32 {
        s.max_luma_tb_64 = r.flag()?;
    }
    s.transform_skip = r.flag()?;
    if s.transform_skip {
        s.log2_ts_max_size = r.ue_max(3, "transform skip size out of range")? + 2;
        s.bdpcm = r.flag()?;
    }
    s.mts = r.flag()?;
    if s.mts {
        s.explicit_mts_intra = r.flag()?;
        s.explicit_mts_inter = r.flag()?;
    }
    s.lfnst = r.flag()?;
    let qp_bd = s.qp_bd_offset();
    if s.chroma_format_idc != 0 {
        s.joint_cbcr = r.flag()?;
        let same = r.flag()?;
        let n = if same {
            1
        } else if s.joint_cbcr {
            3
        } else {
            2
        };
        for i in 0..n {
            let start =
                r.se_range(-26 - qp_bd, 36, "sps_qp_table_start_minus26 out of range")? + 26;
            let points = r.ue_max(63, "too many chroma QP table points")? as usize + 1;
            let mut qin = vec![start];
            let mut qout = vec![start];
            let mut din = Vec::with_capacity(points);
            for j in 0..points {
                let d_in = r.ue_max(127, "chroma QP table step out of range")? as i32;
                let d_diff = r.ue_max(127, "chroma QP table step out of range")? as i32;
                qin.push(qin[j] + d_in + 1);
                qout.push(qout[j] + (d_in ^ d_diff));
                din.push(d_in);
            }
            s.chroma_qp_table[i] = chroma_qp_table(&qin, &qout, &din, qp_bd)?;
        }
        for i in n..3 {
            s.chroma_qp_table[i] = s.chroma_qp_table[0].clone();
        }
    } else {
        // Unused; keep the identity so lookups stay defined.
        let id: Vec<i8> = (-qp_bd..=63).map(|q| q as i8).collect();
        s.chroma_qp_table = [id.clone(), id.clone(), id];
    }
    s.sao = r.flag()?;
    s.alf = r.flag()?;
    if s.alf && s.chroma_format_idc != 0 {
        s.ccalf = r.flag()?;
    }
    s.lmcs = r.flag()?;
    s.weighted_pred = r.flag()?;
    s.weighted_bipred = r.flag()?;
    s.long_term_ref_pics = r.flag()?;
    if s.vps_id > 0 {
        s.inter_layer_prediction = r.flag()?;
    }
    s.idr_rpl_present = r.flag()?;
    s.rpl1_same_as_rpl0 = r.flag()?;
    let lists = if s.rpl1_same_as_rpl0 { 1 } else { 2 };
    for i in 0..lists {
        s.num_ref_pic_lists[i] = r.ue_max(64, "sps_num_ref_pic_lists out of range")? as usize;
        for j in 0..s.num_ref_pic_lists[i] {
            let st = parse_rpl_struct(r, &s, i, j)?;
            s.rpl[i].push(st);
        }
    }
    if s.rpl1_same_as_rpl0 {
        s.num_ref_pic_lists[1] = s.num_ref_pic_lists[0];
        s.rpl[1] = s.rpl[0].clone();
    }
    s.ref_wraparound = r.flag()?;
    s.temporal_mvp = r.flag()?;
    if s.temporal_mvp {
        s.sbtmvp = r.flag()?;
    }
    s.amvr = r.flag()?;
    s.bdof = r.flag()?;
    if s.bdof {
        s.bdof_control_in_ph = r.flag()?;
    }
    s.smvd = r.flag()?;
    s.dmvr = r.flag()?;
    if s.dmvr {
        s.dmvr_control_in_ph = r.flag()?;
    }
    s.mmvd = r.flag()?;
    if s.mmvd {
        s.mmvd_fullpel_only = r.flag()?;
    }
    s.max_num_merge_cand = 6 - r.ue_max(5, "sps_six_minus_max_num_merge_cand out of range")?;
    s.sbt = r.flag()?;
    s.affine = r.flag()?;
    if s.affine {
        s.max_num_subblock_merge_cand =
            5 - r.ue_max(5, "sps_five_minus_max_num_subblock_merge_cand out of range")?;
        s.six_param_affine = r.flag()?;
        if s.amvr {
            s.affine_amvr = r.flag()?;
        }
        s.affine_prof = r.flag()?;
        if s.affine_prof {
            s.prof_control_in_ph = r.flag()?;
        }
    }
    s.bcw = r.flag()?;
    s.ciip = r.flag()?;
    if s.max_num_merge_cand >= 2 {
        s.gpm = r.flag()?;
        if s.gpm {
            s.max_num_gpm_merge_cand = if s.max_num_merge_cand >= 3 {
                s.max_num_merge_cand
                    .checked_sub(r.ue()?)
                    .ok_or(Error::InvalidData("GPM merge candidates out of range"))?
            } else {
                2
            };
        }
    }
    s.log2_parallel_merge_level = r.ue_max(5, "parallel merge level out of range")? + 2;
    s.isp = r.flag()?;
    s.mrl = r.flag()?;
    s.mip = r.flag()?;
    if s.chroma_format_idc != 0 {
        s.cclm = r.flag()?;
    }
    s.chroma_horizontal_collocated = true;
    s.chroma_vertical_collocated = true;
    if s.chroma_format_idc == 1 {
        s.chroma_horizontal_collocated = r.flag()?;
        s.chroma_vertical_collocated = r.flag()?;
    }
    s.palette = r.flag()?;
    if s.chroma_format_idc == 3 && !s.max_luma_tb_64 {
        s.act = r.flag()?;
    }
    if s.transform_skip || s.palette {
        s.min_qp_prime_ts = 4 + 6 * r.ue_max(8, "sps_min_qp_prime_ts out of range")? as i32;
    } else {
        s.min_qp_prime_ts = 4;
    }
    s.ibc = r.flag()?;
    if s.ibc {
        s.max_num_ibc_merge_cand = 6 - r.ue_max(5, "IBC merge candidates out of range")?;
    }
    s.ladf = r.flag()?;
    if s.ladf {
        s.num_ladf_intervals = r.u(2)? as usize + 2;
        s.ladf_lowest_interval_qp_offset = r.se_range(-63, 63, "LADF offset out of range")?;
        s.ladf_lower_bound[0] = 0;
        for i in 0..s.num_ladf_intervals - 1 {
            s.ladf_qp_offset[i] = r.se_range(-63, 63, "LADF offset out of range")?;
            let d = r.ue_max(1 << 16, "LADF threshold out of range")? as i32 + 1;
            s.ladf_lower_bound[i + 1] = s.ladf_lower_bound[i] + d;
        }
    }
    s.explicit_scaling_list = r.flag()?;
    if s.lfnst && s.explicit_scaling_list {
        s.scaling_matrix_for_lfnst_disabled = r.flag()?;
    }
    if s.act && s.explicit_scaling_list {
        s.scaling_matrix_for_alt_colour_space_disabled = r.flag()?;
    }
    if s.scaling_matrix_for_alt_colour_space_disabled {
        s.scaling_matrix_designated_colour_space = r.flag()?;
    }
    s.dep_quant = r.flag()?;
    s.sign_data_hiding = r.flag()?;
    s.virtual_boundaries_enabled = r.flag()?;
    if s.virtual_boundaries_enabled {
        s.virtual_boundaries_present = r.flag()?;
        if s.virtual_boundaries_present {
            let nv = r.ue_max(3, "too many virtual boundaries")?;
            for _ in 0..nv {
                s.vb_x
                    .push((r.ue_max(1 << 13, "virtual boundary out of range")? + 1) * 8);
            }
            let nh = r.ue_max(3, "too many virtual boundaries")?;
            for _ in 0..nh {
                s.vb_y
                    .push((r.ue_max(1 << 13, "virtual boundary out of range")? + 1) * 8);
            }
        }
    }
    if ptl_dpb_hrd && r.flag()? {
        let h = parse_general_hrd(r)?;
        let sub = if s.max_sublayers_minus1 > 0 {
            r.flag()?
        } else {
            false
        };
        let first = if sub {
            0
        } else {
            u32::from(s.max_sublayers_minus1)
        };
        parse_ols_hrd(r, &h, first, u32::from(s.max_sublayers_minus1))?;
    }
    s.field_seq = r.flag()?;
    if r.flag()? {
        let size = r.ue_max(1023, "VUI too large")? as usize + 1;
        r.align_zero()?;
        s.signal = parse_vui(r, size)?;
    }
    if r.flag()? {
        let range = r.flag()?;
        r.u(7)?;
        if range {
            s.extended_precision = r.flag()?;
            if s.transform_skip {
                s.ts_residual_coding_rice_present_in_sh = r.flag()?;
            }
            s.rrc_rice_extension = r.flag()?;
            s.persistent_rice_adaptation = r.flag()?;
            s.reverse_last_sig_coeff = r.flag()?;
        }
    }
    Ok(s)
}

/// ChromaQpTable derivation (7.4.3.4, equation 57), stored from -QpBdOffset to 63.
fn chroma_qp_table(qin: &[i32], qout: &[i32], din: &[i32], qp_bd: i32) -> Result<Vec<i8>> {
    let n = (qp_bd + 64) as usize;
    let mut t = vec![0i32; n];
    let idx = |k: i32| (k + qp_bd) as usize;
    let ok = |k: i32| (-qp_bd..=63).contains(&k);
    if qin.iter().chain(qout).any(|&v| !ok(v)) {
        return Err(Error::InvalidData(
            "chroma QP table outside -QpBdOffset..63",
        ));
    }
    t[idx(qin[0])] = qout[0];
    let mut k = qin[0] - 1;
    while k >= -qp_bd {
        t[idx(k)] = (t[idx(k + 1)] - 1).clamp(-qp_bd, 63);
        k -= 1;
    }
    for j in 0..din.len() {
        let sh = (din[j] + 1) >> 1;
        for (m, k) in (qin[j] + 1..=qin[j + 1]).enumerate() {
            let m = m as i32 + 1;
            t[idx(k)] = t[idx(qin[j])] + ((qout[j + 1] - qout[j]) * m + sh) / (din[j] + 1);
        }
    }
    for k in qin[din.len()] + 1..=63 {
        t[idx(k)] = (t[idx(k - 1)] + 1).clamp(-qp_bd, 63);
    }
    Ok(t.into_iter().map(|v| v.clamp(-128, 127) as i8).collect())
}
