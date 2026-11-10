//! Slice headers (7.3.3).

use crate::bits::BitReader;
use crate::params::{Pps, Sps};
use crate::{Result, malformed};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SliceType {
    P,
    B,
    I,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modification {
    /// modification_of_pic_nums_idc 0 / 1 with abs_diff_pic_num_minus1 + 1 (signed).
    ShortTerm(i32),
    LongTerm(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mmco {
    ShortToUnused(u32),
    LongToUnused(u32),
    ShortToLong(u32, u32),
    MaxLongTermIdx(u32),
    ClearAll,
    CurrentToLong(u32),
}

/// Explicit weighted prediction for one reference: luma (w, o), Cb, Cr.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weight {
    pub w: [i32; 3],
    pub o: [i32; 3],
    /// Whether any flag was set (otherwise the default weights apply).
    pub explicit: [bool; 3],
}

#[derive(Clone, Debug)]
pub struct PredWeights {
    pub luma_log2_denom: u32,
    pub chroma_log2_denom: u32,
    pub l: [Vec<Weight>; 2],
}

#[derive(Clone, Debug)]
pub struct SliceHeader {
    pub nal_ref_idc: u8,
    pub first_mb: u32,
    pub slice_type: SliceType,
    pub pps_id: u32,
    /// colour_plane_id (separate colour planes: 0 = Y, 1 = Cb, 2 = Cr).
    pub colour_plane: u8,
    pub frame_num: u32,
    /// A field picture (field_pic_flag), and which field (bottom_field_flag).
    pub field_pic: bool,
    pub bottom_field: bool,
    pub idr: bool,
    pub idr_pic_id: u32,
    pub poc_lsb: u32,
    pub delta_poc_bottom: i32,
    pub delta_poc: [i32; 2],
    pub redundant_pic_cnt: u32,
    pub direct_spatial: bool,
    pub num_ref_idx: [u32; 2],
    pub modifications: [Vec<Modification>; 2],
    pub weights: Option<PredWeights>,
    pub no_output_of_prior_pics: bool,
    pub long_term_reference: bool,
    pub mmcos: Option<Vec<Mmco>>,
    pub cabac_init_idc: u32,
    pub qp: i32,
    pub disable_deblocking: u32,
    pub alpha_offset: i32,
    pub beta_offset: i32,
    /// slice_group_change_cycle (evolving slice groups).
    pub slice_group_change_cycle: u32,
    /// An SP slice (decoded as P with the SP transform path) or an SI slice (decoded as I,
    /// with the SI macroblock type).
    pub switching: Switching,
    /// QS_Y of an SP or SI slice.
    pub qs: i32,
    /// Bit offset of slice_data() in the RBSP.
    pub data_bit: usize,
}

/// Switching slices (8.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Switching {
    #[default]
    None,
    /// An SP slice; `true` for sp_for_switch_flag.
    Sp(bool),
    Si,
}

impl SliceHeader {
    /// The field parity of a field picture (0 top, 1 bottom).
    pub fn parity(&self) -> Option<u8> {
        self.field_pic.then_some(self.bottom_field as u8)
    }

    pub fn has_mmco5(&self) -> bool {
        self.mmcos
            .as_ref()
            .is_some_and(|m| m.contains(&Mmco::ClearAll))
    }
}

/// Reads the first fields up to pic_parameter_set_id.
pub fn peek_pps_id(data: &[u8], len: usize) -> Option<(u32, u32, u32)> {
    let mut r = BitReader::new(data, len);
    let first_mb = r.ue();
    let st = r.ue();
    let pps = r.ue();
    (!r.overrun()).then_some((first_mb, st, pps))
}

fn ref_list_modification(r: &mut BitReader, out: &mut Vec<Modification>) -> Result<()> {
    if !r.flag() {
        return Ok(());
    }
    loop {
        let idc = r.ue();
        match idc {
            0 | 1 => {
                let d = r.ue();
                if d >= 1 << 17 {
                    return Err(malformed("abs_diff_pic_num out of range"));
                }
                let v = d as i32 + 1;
                out.push(Modification::ShortTerm(if idc == 0 { -v } else { v }));
            }
            2 => out.push(Modification::LongTerm(r.ue())),
            3 => return Ok(()),
            _ => return Err(malformed("modification_of_pic_nums_idc out of range")),
        }
        if out.len() > 66 || r.overrun() {
            return Err(malformed("ref_pic_list_modification too long"));
        }
    }
}

fn pred_weight_table(
    r: &mut BitReader,
    lists: usize,
    n: [u32; 2],
    chroma: bool,
) -> Result<PredWeights> {
    let luma_log2_denom = r.ue();
    let chroma_log2_denom = if chroma { r.ue() } else { 0 };
    if luma_log2_denom > 7 || chroma_log2_denom > 7 {
        return Err(malformed("log2_weight_denom out of range"));
    }
    let mut l: [Vec<Weight>; 2] = [Vec::new(), Vec::new()];
    for (list, out) in l.iter_mut().enumerate().take(lists) {
        for _ in 0..n[list] {
            let mut w = Weight {
                w: [
                    1 << luma_log2_denom,
                    1 << chroma_log2_denom,
                    1 << chroma_log2_denom,
                ],
                o: [0; 3],
                explicit: [false; 3],
            };
            if r.flag() {
                w.w[0] = r.se();
                w.o[0] = r.se();
                w.explicit[0] = true;
            }
            if chroma && r.flag() {
                for c in 1..3 {
                    w.w[c] = r.se();
                    w.o[c] = r.se();
                    w.explicit[c] = true;
                }
            }
            if w.w
                .iter()
                .chain(w.o.iter())
                .any(|v| !(-128..=127).contains(v))
            {
                return Err(malformed("prediction weight out of range"));
            }
            out.push(w);
        }
    }
    Ok(PredWeights {
        luma_log2_denom,
        chroma_log2_denom,
        l,
    })
}

/// The slice header up to redundant_pic_cnt: what identifies the picture (7.4.1.2.4), and
/// everything [`crate::fields`] needs.
fn header_start(
    r: &mut BitReader,
    nal_unit_type: u8,
    nal_ref_idc: u8,
    sps: &Sps,
    pps: &Pps,
) -> Result<SliceHeader> {
    let first_mb = r.ue();
    let st = r.ue();
    let pps_id = r.ue();
    let (slice_type, switching) = match st % 5 {
        0 => (SliceType::P, Switching::None),
        1 => (SliceType::B, Switching::None),
        2 => (SliceType::I, Switching::None),
        3 => (SliceType::P, Switching::Sp(false)),
        _ => (SliceType::I, Switching::Si),
    };
    if st > 9 {
        return Err(malformed("slice_type out of range"));
    }
    let idr = nal_unit_type == 5;
    if idr && slice_type != SliceType::I {
        return Err(malformed("IDR picture with a P or B slice"));
    }
    let colour_plane = if sps.separate_colour_plane {
        r.u(2) as u8
    } else {
        0
    };
    if colour_plane > 2 {
        return Err(malformed("colour_plane_id out of range"));
    }
    let frame_num = r.u(sps.log2_max_frame_num);
    let field_pic = !sps.frame_mbs_only && r.flag();
    let bottom_field = field_pic && r.flag();
    // PicSizeInMbs; an MBAFF frame addresses macroblock pairs.
    let size = sps.width_mbs * sps.height_mbs / if field_pic { 2 } else { 1 };
    let mbaff = sps.mbaff && !field_pic;
    if first_mb.saturating_mul(1 + mbaff as u32) >= size {
        return Err(malformed("first_mb_in_slice out of range"));
    }
    let idr_pic_id = if idr { r.ue() } else { 0 };
    let mut h = SliceHeader {
        nal_ref_idc,
        first_mb,
        slice_type,
        pps_id,
        colour_plane,
        frame_num,
        field_pic,
        bottom_field,
        idr,
        idr_pic_id,
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
        qp: 0,
        disable_deblocking: 0,
        alpha_offset: 0,
        beta_offset: 0,
        slice_group_change_cycle: 0,
        switching,
        qs: 0,
        data_bit: 0,
    };
    if sps.poc_type == 0 {
        h.poc_lsb = r.u(sps.log2_max_poc_lsb);
        if pps.bottom_field_pic_order_in_frame_present && !field_pic {
            h.delta_poc_bottom = r.se();
        }
    } else if sps.poc_type == 1 && !sps.delta_pic_order_always_zero {
        h.delta_poc[0] = r.se();
        if pps.bottom_field_pic_order_in_frame_present && !field_pic {
            h.delta_poc[1] = r.se();
        }
    }
    if pps.redundant_pic_cnt_present {
        h.redundant_pic_cnt = r.ue();
    }
    Ok(h)
}

/// The start of a slice header (through redundant_pic_cnt): the fields that tell pictures
/// apart; the reference lists, weights, marking and the rest are left at their defaults.
pub fn peek_slice_header(
    data: &[u8],
    len: usize,
    nal_unit_type: u8,
    nal_ref_idc: u8,
    sps: &Sps,
    pps: &Pps,
) -> Result<SliceHeader> {
    let mut r = BitReader::new(data, len);
    let h = header_start(&mut r, nal_unit_type, nal_ref_idc, sps, pps)?;
    if r.overrun() {
        return Err(malformed("truncated slice header"));
    }
    Ok(h)
}

pub fn parse_slice_header(
    data: &[u8],
    len: usize,
    nal_unit_type: u8,
    nal_ref_idc: u8,
    sps: &Sps,
    pps: &Pps,
) -> Result<SliceHeader> {
    let mut r = BitReader::new(data, len);
    let mut h = header_start(&mut r, nal_unit_type, nal_ref_idc, sps, pps)?;
    let slice_type = h.slice_type;
    let idr = h.idr;
    if slice_type == SliceType::B {
        h.direct_spatial = r.flag();
    }
    if slice_type != SliceType::I {
        h.num_ref_idx = pps.num_ref_idx_default;
        if r.flag() {
            h.num_ref_idx[0] = r.ue().saturating_add(1);
            if slice_type == SliceType::B {
                h.num_ref_idx[1] = r.ue().saturating_add(1);
            }
        }
        if slice_type != SliceType::B {
            h.num_ref_idx[1] = 0;
        }
        if h.num_ref_idx[0] > 32 || h.num_ref_idx[1] > 32 {
            return Err(malformed("num_ref_idx_active out of range"));
        }
        ref_list_modification(&mut r, &mut h.modifications[0])?;
        if slice_type == SliceType::B {
            ref_list_modification(&mut r, &mut h.modifications[1])?;
        }
    }
    if (pps.weighted_pred && slice_type == SliceType::P)
        || (pps.weighted_bipred_idc == 1 && slice_type == SliceType::B)
    {
        let lists = if slice_type == SliceType::B { 2 } else { 1 };
        h.weights = Some(pred_weight_table(
            &mut r,
            lists,
            h.num_ref_idx,
            sps.chroma_array_type() != 0,
        )?);
    }
    if nal_ref_idc != 0 {
        if idr {
            h.no_output_of_prior_pics = r.flag();
            h.long_term_reference = r.flag();
        } else if r.flag() {
            let mut ops = Vec::new();
            loop {
                let op = match r.ue() {
                    0 => break,
                    1 => Mmco::ShortToUnused(r.ue()),
                    2 => Mmco::LongToUnused(r.ue()),
                    3 => {
                        let d = r.ue();
                        Mmco::ShortToLong(d, r.ue())
                    }
                    4 => Mmco::MaxLongTermIdx(r.ue()),
                    5 => Mmco::ClearAll,
                    6 => Mmco::CurrentToLong(r.ue()),
                    _ => {
                        return Err(malformed(
                            "memory_management_control_operation out of range",
                        ));
                    }
                };
                ops.push(op);
                if ops.len() > 100 || r.overrun() {
                    return Err(malformed("dec_ref_pic_marking too long"));
                }
            }
            h.mmcos = Some(ops);
        }
    }
    if pps.cabac && slice_type != SliceType::I {
        h.cabac_init_idc = r.ue();
        if h.cabac_init_idc > 2 {
            return Err(malformed("cabac_init_idc out of range"));
        }
    }
    h.qp = pps.pic_init_qp.saturating_add(r.se());
    if !(-6 * (sps.bit_depth_luma as i32 - 8)..=51).contains(&h.qp) {
        return Err(malformed("slice QP out of range"));
    }
    if h.switching != Switching::None {
        if let Switching::Sp(_) = h.switching {
            h.switching = Switching::Sp(r.flag());
        }
        h.qs = pps.pic_init_qs.saturating_add(r.se());
        if !(0..=51).contains(&h.qs) {
            return Err(malformed("slice QS out of range"));
        }
    }
    if pps.deblocking_filter_control_present {
        h.disable_deblocking = r.ue();
        if h.disable_deblocking > 2 {
            return Err(malformed("disable_deblocking_filter_idc out of range"));
        }
        if h.disable_deblocking != 1 {
            h.alpha_offset = r.se().saturating_mul(2);
            h.beta_offset = r.se().saturating_mul(2);
            if !(-12..=12).contains(&h.alpha_offset) || !(-12..=12).contains(&h.beta_offset) {
                return Err(malformed("deblocking offsets out of range"));
            }
        }
    }
    if let Some(g) = &pps.slice_groups
        && g.cycle_bits > 0
    {
        h.slice_group_change_cycle = r.u(g.cycle_bits);
    }
    if r.overrun() {
        return Err(malformed("truncated slice header"));
    }
    h.data_bit = r.pos();
    Ok(h)
}
