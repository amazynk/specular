//! Sequence and picture parameter sets (7.3.2.1, 7.3.2.2), VUI (E.1.1) and scaling lists.

use crate::bits::{BitReader, rbsp_end};
use crate::spec_tables::{DEFAULT_4X4, DEFAULT_8X8};
use crate::tables::{ZIGZAG4, ZIGZAG8, norm4, norm8};
use crate::{Error, Result, malformed};

/// Weight scale matrices in raster order: six 4x4 lists (Intra Y/Cb/Cr, Inter Y/Cb/Cr) and
/// two 8x8 lists (Intra Y, Inter Y).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScalingLists {
    pub l4: [[u8; 16]; 6],
    /// 8x8 lists: Intra Y, Inter Y, then Cb and Cr (4:4:4) in the same order.
    pub l8: [[u8; 64]; 6],
}

impl ScalingLists {
    pub fn flat() -> ScalingLists {
        ScalingLists {
            l4: [[16; 16]; 6],
            l8: [[16; 64]; 6],
        }
    }
}

/// One scaling_list(): returns the list in raster order, or `None` when
/// useDefaultScalingMatrixFlag is set.
fn scaling_list<const N: usize>(r: &mut BitReader) -> Result<Option<[u8; N]>> {
    let scan: &[u8] = if N == 16 { &ZIGZAG4 } else { &ZIGZAG8 };
    let mut out = [0u8; N];
    let mut last = 8i32;
    let mut next = 8i32;
    for j in 0..N {
        if next != 0 {
            let delta = r.se();
            if !(-128..=127).contains(&delta) {
                return Err(malformed("scaling list delta out of range"));
            }
            next = (last + delta + 256) % 256;
            if j == 0 && next == 0 {
                return Ok(None);
            }
        }
        let v = if next == 0 { last } else { next };
        out[scan[j] as usize] = v as u8;
        last = v;
    }
    Ok(Some(out))
}

/// Parses the scaling_list_present_flag loop (SPS: fall-back rule A with `fallback` None,
/// PPS: rule B with the sequence-level lists).
fn parse_scaling_lists(
    r: &mut BitReader,
    n8: usize,
    fallback: Option<&ScalingLists>,
) -> Result<ScalingLists> {
    let mut s = ScalingLists::flat();
    for i in 0..6 {
        let present = r.flag();
        let list = if present {
            scaling_list::<16>(r)?
        } else {
            None
        };
        s.l4[i] = match (present, list) {
            (true, Some(l)) => l,
            (true, None) => DEFAULT_4X4[i / 3],
            (false, _) => {
                if i == 0 || i == 3 {
                    match fallback {
                        Some(f) => f.l4[i],
                        None => DEFAULT_4X4[i / 3],
                    }
                } else {
                    s.l4[i - 1]
                }
            }
        };
    }
    for i in 0..n8 {
        let present = r.flag();
        let list = if present {
            scaling_list::<64>(r)?
        } else {
            None
        };
        s.l8[i] = match (present, list) {
            (true, Some(l)) => l,
            (true, None) => DEFAULT_8X8[i % 2],
            (false, _) => {
                if i < 2 {
                    match fallback {
                        Some(f) => f.l8[i],
                        None => DEFAULT_8X8[i],
                    }
                } else {
                    // Table 7-2: a 4:4:4 Cb or Cr list falls back to the list two before.
                    s.l8[i - 2]
                }
            }
        };
    }
    // Without 4:4:4 lists (n8 = 2), the Cb and Cr lists are the luma ones (only 4:4:4 reads
    // them).
    for i in n8.max(2)..6 {
        s.l8[i] = s.l8[i - 2];
    }
    Ok(s)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vui {
    /// video_signal_type_present_flag.
    pub signal: bool,
    pub full_range: bool,
    pub colour: Option<(u8, u8, u8)>,
    /// chroma_sample_loc_type_top_field (0..=5), when signalled.
    pub chroma_loc: Option<u8>,
    pub max_num_reorder_frames: Option<u32>,
    pub max_dec_frame_buffering: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sps {
    pub id: u32,
    pub profile_idc: u8,
    pub constraint_flags: u8,
    pub level_idc: u8,
    pub chroma_format_idc: u32,
    pub separate_colour_plane: bool,
    pub bit_depth_luma: u32,
    pub bit_depth_chroma: u32,
    pub transform_bypass: bool,
    pub scaling: Option<ScalingLists>,
    pub log2_max_frame_num: u32,
    pub poc_type: u32,
    pub log2_max_poc_lsb: u32,
    pub delta_pic_order_always_zero: bool,
    pub offset_for_non_ref_pic: i32,
    pub offset_for_top_to_bottom_field: i32,
    pub offset_for_ref_frame: Vec<i32>,
    pub max_num_ref_frames: u32,
    pub gaps_allowed: bool,
    pub width_mbs: u32,
    pub height_mbs: u32,
    pub frame_mbs_only: bool,
    pub mbaff: bool,
    pub direct_8x8_inference: bool,
    /// Crop in luma samples: left, right, top, bottom.
    pub crop: [u32; 4],
    pub vui: Vui,
}

impl Sps {
    /// Why this stream can't be decoded here, if it can't.
    pub fn unsupported(&self) -> Option<String> {
        None
    }

    /// ChromaArrayType: 0 for monochrome and for separate colour planes.
    pub fn chroma_array_type(&self) -> u32 {
        if self.separate_colour_plane {
            0
        } else {
            self.chroma_format_idc
        }
    }

    /// Separate colour planes: the stream holds three monochrome pictures per frame, told
    /// apart by colour_plane_id (7.4.2.1.1).
    pub fn split_planes(&self) -> bool {
        self.separate_colour_plane && self.chroma_format_idc == 3
    }

    /// The sequence one colour plane of a [`Sps::split_planes`] stream is decoded with: a
    /// monochrome one at the luma bit depth (ChromaArrayType 0). The slice headers still
    /// carry colour_plane_id.
    pub fn plane_sps(&self) -> Sps {
        Sps {
            chroma_format_idc: 0,
            bit_depth_chroma: self.bit_depth_luma,
            ..self.clone()
        }
    }

    /// DPB size in frames (A.3.1, Table A-1), or max_dec_frame_buffering when signalled.
    pub fn dpb_frames(&self) -> u32 {
        let max_dpb_mbs: u32 = match self.level_idc {
            9 | 10 => 396,
            11 if self.constraint_flags & 0x10 != 0 && self.profile_idc != 100 => 396,
            11 => 900,
            12 | 13 | 20 => 2376,
            21 => 4752,
            22 | 30 => 8100,
            31 => 18000,
            32 => 20480,
            40 | 41 => 32768,
            42 => 34816,
            50 => 110400,
            51 | 52 => 184320,
            _ => 696320,
        };
        let frames = (max_dpb_mbs / (self.width_mbs * self.height_mbs).max(1)).clamp(1, 16);
        let frames = frames.max(self.max_num_ref_frames);
        match self.vui.max_dec_frame_buffering {
            Some(n) => n.clamp(self.max_num_ref_frames.max(1), 16),
            None => frames,
        }
    }

    /// Output reorder depth: pictures that may precede another in decoding order yet
    /// follow it in output order.
    pub fn reorder_depth(&self) -> u32 {
        if let Some(n) = self.vui.max_num_reorder_frames {
            return n.min(16);
        }
        if self.poc_type == 2 {
            return 0;
        }
        // Profiles without B slices output in decoding order unless POC says otherwise;
        // without the VUI hint the DPB size bounds the reordering.
        self.dpb_frames()
    }
}

fn hrd(r: &mut BitReader) {
    let cnt = r.ue().min(31) + 1;
    r.u(8);
    for _ in 0..cnt {
        r.ue();
        r.ue();
        r.u(1);
    }
    r.u(20);
}

fn vui(r: &mut BitReader, stop: usize) -> Vui {
    let mut v = Vui::default();
    if r.flag() {
        let idc = r.u(8);
        if idc == 255 {
            r.u(32);
        }
    }
    if r.flag() {
        r.u(1);
    }
    if r.flag() {
        r.u(3);
        v.signal = true;
        v.full_range = r.flag();
        if r.flag() {
            let p = r.u(8) as u8;
            let t = r.u(8) as u8;
            let m = r.u(8) as u8;
            v.colour = Some((p, t, m));
        }
    }
    if r.flag() {
        // chroma_loc_info_present_flag: top and bottom field types, 0..=5.
        v.chroma_loc = Some(r.ue().min(5) as u8);
        r.ue();
    }
    if r.flag() {
        r.u(32);
        r.u(32);
        r.u(1);
    }
    let nal = r.flag();
    if nal {
        hrd(r);
    }
    let vcl = r.flag();
    if vcl {
        hrd(r);
    }
    if nal || vcl {
        r.u(1);
    }
    r.u(1);
    if r.overrun() || r.pos() > stop {
        return v;
    }
    if r.flag() {
        r.u(1);
        r.ue();
        r.ue();
        r.ue();
        r.ue();
        let reorder = r.ue();
        let dec = r.ue();
        // Truncated VUIs (some encoders cut the trailing fields) are ignored.
        if !r.overrun() && r.pos() <= stop + 1 && reorder <= 16 && dec <= 16 {
            v.max_num_reorder_frames = Some(reorder);
            v.max_dec_frame_buffering = Some(dec.max(reorder));
        }
    }
    v
}

pub fn parse_sps(data: &[u8], len: usize) -> Result<Sps> {
    let stop = rbsp_end(data, len);
    let mut r = BitReader::new(data, len);
    let profile_idc = r.u(8) as u8;
    let constraint_flags = r.u(8) as u8;
    let level_idc = r.u(8) as u8;
    let id = r.ue();
    if id > 31 {
        return Err(malformed("seq_parameter_set_id out of range"));
    }
    let mut sps = Sps {
        id,
        profile_idc,
        constraint_flags,
        level_idc,
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
        max_num_ref_frames: 0,
        gaps_allowed: false,
        width_mbs: 0,
        height_mbs: 0,
        frame_mbs_only: true,
        mbaff: false,
        direct_8x8_inference: false,
        crop: [0; 4],
        vui: Vui::default(),
    };
    if matches!(
        profile_idc,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        sps.chroma_format_idc = r.ue();
        if sps.chroma_format_idc > 3 {
            return Err(malformed("chroma_format_idc out of range"));
        }
        if sps.chroma_format_idc == 3 {
            sps.separate_colour_plane = r.flag();
        }
        sps.bit_depth_luma = r.ue().saturating_add(8);
        sps.bit_depth_chroma = r.ue().saturating_add(8);
        if sps.bit_depth_luma > 14 || sps.bit_depth_chroma > 14 {
            return Err(malformed("bit depth out of range"));
        }
        sps.transform_bypass = r.flag();
        if r.flag() {
            let n8 = if sps.chroma_format_idc == 3 { 6 } else { 2 };
            sps.scaling = Some(parse_scaling_lists(&mut r, n8, None)?);
        }
    }
    sps.log2_max_frame_num = r.ue().saturating_add(4);
    if sps.log2_max_frame_num > 16 {
        return Err(malformed("log2_max_frame_num out of range"));
    }
    sps.poc_type = r.ue();
    match sps.poc_type {
        0 => {
            sps.log2_max_poc_lsb = r.ue().saturating_add(4);
            if sps.log2_max_poc_lsb > 16 {
                return Err(malformed("log2_max_pic_order_cnt_lsb out of range"));
            }
        }
        1 => {
            sps.delta_pic_order_always_zero = r.flag();
            sps.offset_for_non_ref_pic = r.se();
            sps.offset_for_top_to_bottom_field = r.se();
            let n = r.ue();
            if n > 255 {
                return Err(malformed(
                    "num_ref_frames_in_pic_order_cnt_cycle out of range",
                ));
            }
            for _ in 0..n {
                sps.offset_for_ref_frame.push(r.se());
            }
        }
        2 => {}
        _ => return Err(malformed("pic_order_cnt_type out of range")),
    }
    sps.max_num_ref_frames = r.ue();
    if sps.max_num_ref_frames > 16 {
        return Err(malformed("max_num_ref_frames out of range"));
    }
    sps.gaps_allowed = r.flag();
    let w = r.ue();
    let h = r.ue();
    sps.frame_mbs_only = r.flag();
    if w >= 1024 || h >= 1024 {
        return Err(malformed("picture size out of range"));
    }
    sps.width_mbs = w + 1;
    sps.height_mbs = (h + 1) * if sps.frame_mbs_only { 1 } else { 2 };
    if sps.width_mbs * sps.height_mbs > 139264 {
        return Err(Error::Unsupported(format!(
            "H.264 picture of {}x{} macroblocks",
            sps.width_mbs, sps.height_mbs
        )));
    }
    if !sps.frame_mbs_only {
        sps.mbaff = r.flag();
    }
    sps.direct_8x8_inference = r.flag();
    if r.flag() {
        // CropUnitX and CropUnitY (7-19 to 7-22).
        let (sub_w, sub_h) = match sps.chroma_format_idc {
            0 => (1, 1),
            1 => (2, 2),
            2 => (2, 1),
            _ => (1, 1),
        };
        let unit_y = sub_h * if sps.frame_mbs_only { 1 } else { 2 };
        let c = [r.ue(), r.ue(), r.ue(), r.ue()];
        let crop = [
            c[0].saturating_mul(sub_w),
            c[1].saturating_mul(sub_w),
            c[2].saturating_mul(unit_y),
            c[3].saturating_mul(unit_y),
        ];
        let fits = |a: u32, b: u32, size: u32| a.checked_add(b).is_some_and(|s| s < size);
        if fits(crop[0], crop[1], sps.width_mbs * 16) && fits(crop[2], crop[3], sps.height_mbs * 16)
        {
            sps.crop = crop;
        }
    }
    if r.overrun() {
        return Err(malformed("truncated sequence parameter set"));
    }
    if r.flag() {
        sps.vui = vui(&mut r, stop);
    }
    Ok(sps)
}

#[derive(Clone, Debug)]
pub struct Pps {
    pub id: u32,
    pub sps_id: u32,
    pub cabac: bool,
    pub bottom_field_pic_order_in_frame_present: bool,
    pub num_ref_idx_default: [u32; 2],
    pub weighted_pred: bool,
    pub weighted_bipred_idc: u32,
    pub pic_init_qp: i32,
    pub chroma_qp_offset: [i32; 2],
    pub deblocking_filter_control_present: bool,
    pub constrained_intra_pred: bool,
    pub redundant_pic_cnt_present: bool,
    pub transform_8x8_mode: bool,
    /// Slice groups (FMO), when there is more than one.
    pub slice_groups: Option<std::sync::Arc<SliceGroups>>,
    /// pic_init_qs (SP and SI slices).
    pub pic_init_qs: i32,
    /// LevelScale4x4 per list (0..6) and qP % 6, raster order.
    pub level4: Box<[[[i32; 16]; 6]; 6]>,
    /// LevelScale8x8 per list (Intra Y, Inter Y) and qP % 6, raster order.
    pub level8: Box<[[[i32; 64]; 6]; 6]>,
}

impl Pps {
    /// This PPS for colour plane `plane` of a separate-colour-plane stream: the plane's
    /// scaling lists take the luma lists' places (iYCbCr = colour_plane_id, 8.5.9).
    pub fn plane_pps(&self, plane: usize) -> Pps {
        let mut p = self.clone();
        p.level4[0] = self.level4[plane];
        p.level4[3] = self.level4[3 + plane];
        p.level8[0] = self.level8[2 * plane];
        p.level8[1] = self.level8[2 * plane + 1];
        p
    }
}

pub fn parse_pps(data: &[u8], len: usize, spss: &[Option<std::sync::Arc<Sps>>]) -> Result<Pps> {
    let stop = rbsp_end(data, len);
    let mut r = BitReader::new(data, len);
    let id = r.ue();
    if id > 255 {
        return Err(malformed("pic_parameter_set_id out of range"));
    }
    let sps_id = r.ue();
    let sps = spss
        .get(sps_id as usize)
        .and_then(|s| s.as_ref())
        .ok_or_else(|| malformed("picture parameter set refers to a missing SPS"))?;
    let cabac = r.flag();
    let bottom_field_pic_order_in_frame_present = r.flag();
    let num_slice_groups = r.ue().saturating_add(1);
    let slice_groups = if num_slice_groups > 1 {
        Some(std::sync::Arc::new(parse_slice_groups(
            &mut r,
            num_slice_groups,
            sps,
        )?))
    } else {
        None
    };
    let l0 = r.ue().saturating_add(1);
    let l1 = r.ue().saturating_add(1);
    if l0 > 32 || l1 > 32 {
        return Err(malformed("num_ref_idx_default_active out of range"));
    }
    let weighted_pred = r.flag();
    let weighted_bipred_idc = r.u(2);
    if weighted_bipred_idc > 2 {
        return Err(malformed("weighted_bipred_idc out of range"));
    }
    let pic_init_qp = r.se().saturating_add(26);
    let pic_init_qs = r.se().saturating_add(26);
    let cqp = r.se();
    let qp_min = -6 * (sps.bit_depth_luma as i32 - 8);
    if !(qp_min..=51).contains(&pic_init_qp)
        || !(-12..=12).contains(&cqp)
        || !(0..=51).contains(&pic_init_qs)
    {
        return Err(malformed("PPS QP out of range"));
    }
    let deblocking_filter_control_present = r.flag();
    let constrained_intra_pred = r.flag();
    let redundant_pic_cnt_present = r.flag();
    let mut transform_8x8_mode = false;
    let mut second = cqp;
    let seq = sps.scaling.clone().unwrap_or_else(ScalingLists::flat);
    let mut scaling = seq.clone();
    if r.more_rbsp_data(stop) {
        transform_8x8_mode = r.flag();
        if r.flag() {
            let n8 = if transform_8x8_mode {
                if sps.chroma_format_idc == 3 { 6 } else { 2 }
            } else {
                0
            };
            scaling = parse_scaling_lists(&mut r, n8, Some(&seq))?;
            if n8 == 0 {
                scaling.l8 = seq.l8;
            }
        }
        second = r.se();
        if !(-12..=12).contains(&second) {
            return Err(malformed("second_chroma_qp_index_offset out of range"));
        }
    }
    if r.overrun() {
        return Err(malformed("truncated picture parameter set"));
    }
    let mut level4 = Box::new([[[0i32; 16]; 6]; 6]);
    let mut level8 = Box::new([[[0i32; 64]; 6]; 6]);
    for (l, list) in scaling.l4.iter().enumerate() {
        for m in 0..6 {
            for p in 0..16 {
                level4[l][m][p] = list[p] as i32 * norm4(m, p);
            }
        }
    }
    for (l, list) in scaling.l8.iter().enumerate() {
        for m in 0..6 {
            for p in 0..64 {
                level8[l][m][p] = list[p] as i32 * norm8(m, p);
            }
        }
    }
    Ok(Pps {
        id,
        sps_id,
        cabac,
        bottom_field_pic_order_in_frame_present,
        num_ref_idx_default: [l0, l1],
        weighted_pred,
        weighted_bipred_idc,
        pic_init_qp,
        chroma_qp_offset: [cqp, second],
        deblocking_filter_control_present,
        constrained_intra_pred,
        redundant_pic_cnt_present,
        transform_8x8_mode,
        slice_groups,
        pic_init_qs,
        level4,
        level8,
    })
}

/// How macroblocks are allotted to slice groups (7.4.2.2): slice_group_map_type and its
/// parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SliceGroupMap {
    /// 0: interleaved runs (run_length_minus1 + 1 per group).
    Interleaved(Vec<u32>),
    /// 1: dispersed.
    Dispersed,
    /// 2: foreground rectangles (top_left, bottom_right per group but the last).
    Foreground(Vec<(u32, u32)>),
    /// 3: box-out, 4: raster scan, 5: wipe; with slice_group_change_direction_flag.
    Evolving { kind: u8, reverse: bool },
    /// 6: explicit slice_group_id per map unit.
    Explicit(Vec<u8>),
}

/// The slice groups of a picture parameter set (FMO).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SliceGroups {
    pub count: u32,
    pub map: SliceGroupMap,
    /// SliceGroupChangeRate (types 3 to 5).
    pub change_rate: u32,
    /// Bits of slice_group_change_cycle in the slice header (types 3 to 5).
    pub cycle_bits: u32,
}

fn ceil_log2(v: u32) -> u32 {
    if v <= 1 {
        0
    } else {
        32 - (v - 1).leading_zeros()
    }
}

/// PicSizeInMapUnits of `sps`.
pub fn map_units(sps: &Sps) -> u32 {
    sps.width_mbs * (sps.height_mbs / if sps.frame_mbs_only { 1 } else { 2 })
}

fn parse_slice_groups(r: &mut BitReader, count: u32, sps: &Sps) -> Result<SliceGroups> {
    if count > 8 {
        return Err(malformed("num_slice_groups out of range"));
    }
    let units = map_units(sps);
    let kind = r.ue();
    let mut change_rate = 1;
    let map = match kind {
        0 => {
            let mut runs = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let run = r.ue();
                if run >= units {
                    return Err(malformed("run_length out of range"));
                }
                runs.push(run + 1);
            }
            SliceGroupMap::Interleaved(runs)
        }
        1 => SliceGroupMap::Dispersed,
        2 => {
            let mut rects = Vec::with_capacity(count as usize);
            for _ in 0..count - 1 {
                let tl = r.ue();
                let br = r.ue();
                if tl > br || br >= units || tl % sps.width_mbs > br % sps.width_mbs {
                    return Err(malformed("slice group rectangle out of range"));
                }
                rects.push((tl, br));
            }
            SliceGroupMap::Foreground(rects)
        }
        3..=5 => {
            if count != 2 {
                return Err(malformed("evolving slice groups need two groups"));
            }
            let reverse = r.flag();
            change_rate = r.ue().saturating_add(1);
            if change_rate > units {
                return Err(malformed("slice_group_change_rate out of range"));
            }
            SliceGroupMap::Evolving {
                kind: kind as u8,
                reverse,
            }
        }
        6 => {
            let n = r.ue().saturating_add(1);
            if n != units {
                return Err(malformed("pic_size_in_map_units does not match the SPS"));
            }
            let bits = ceil_log2(count);
            let mut ids = Vec::with_capacity(n as usize);
            for _ in 0..n {
                let id = r.u(bits);
                if id >= count {
                    return Err(malformed("slice_group_id out of range"));
                }
                ids.push(id as u8);
                if r.overrun() {
                    return Err(malformed("truncated slice group map"));
                }
            }
            SliceGroupMap::Explicit(ids)
        }
        _ => return Err(malformed("slice_group_map_type out of range")),
    };
    Ok(SliceGroups {
        count,
        map,
        change_rate,
        // Ceil(Log2(PicSizeInMapUnits ÷ SliceGroupChangeRate + 1)), exact division; only
        // evolving groups carry slice_group_change_cycle.
        cycle_bits: if (3..=5).contains(&kind) {
            ceil_log2(units.div_ceil(change_rate) + 1)
        } else {
            0
        },
    })
}

impl SliceGroups {
    /// mapUnitToSliceGroupMap (8.2.2.1 to 8.2.2.7) for a picture `w` map units wide and
    /// `h` high, with slice_group_change_cycle `cycle`.
    pub fn unit_map(&self, w: u32, h: u32, cycle: u32) -> Vec<u8> {
        let n = (w * h) as usize;
        let groups = self.count;
        let mut m = vec![0u8; n];
        let in_group0 = (cycle.saturating_mul(self.change_rate) as usize).min(n);
        match &self.map {
            SliceGroupMap::Interleaved(runs) => {
                let mut i = 0usize;
                while i < n {
                    for (g, &run) in runs.iter().enumerate() {
                        if i >= n {
                            break;
                        }
                        let end = (i + run as usize).min(n);
                        m[i..end].fill(g as u8);
                        i += run as usize;
                    }
                }
            }
            SliceGroupMap::Dispersed => {
                for (i, v) in m.iter_mut().enumerate() {
                    let i = i as u32;
                    *v = ((i % w + ((i / w) * groups) / 2) % groups) as u8;
                }
            }
            SliceGroupMap::Foreground(rects) => {
                m.fill((groups - 1) as u8);
                for (g, &(tl, br)) in rects.iter().enumerate().rev() {
                    let (y0, x0, y1, x1) = (tl / w, tl % w, br / w, br % w);
                    for y in y0..=y1.min(h.saturating_sub(1)) {
                        for x in x0..=x1.min(w - 1) {
                            m[(y * w + x) as usize] = g as u8;
                        }
                    }
                }
            }
            SliceGroupMap::Evolving { kind, reverse } => {
                let dir = *reverse as i32;
                let upper_left = if *reverse { n - in_group0 } else { in_group0 };
                let (g0, g1) = (dir as u8, 1 - dir as u8);
                match kind {
                    3 => box_out(&mut m, w as i32, h as i32, dir, in_group0),
                    4 => {
                        for (i, v) in m.iter_mut().enumerate() {
                            *v = if i < upper_left { g0 } else { g1 };
                        }
                    }
                    _ => {
                        let mut k = 0;
                        for j in 0..w {
                            for i in 0..h {
                                m[(i * w + j) as usize] = if k < upper_left { g0 } else { g1 };
                                k += 1;
                            }
                        }
                    }
                }
            }
            SliceGroupMap::Explicit(ids) => {
                let k = ids.len().min(n);
                m[..k].copy_from_slice(&ids[..k]);
            }
        }
        m
    }
}

/// The box-out map (8.2.2.4): `in_group0` map units spiralling out from the centre.
fn box_out(m: &mut [u8], w: i32, h: i32, dir: i32, in_group0: usize) {
    m.fill(1);
    let mut x = (w - dir) / 2;
    let mut y = (h - dir) / 2;
    let (mut left, mut top, mut right, mut bottom) = (x, y, x, y);
    let (mut xd, mut yd) = (dir - 1, dir);
    let mut k = 0usize;
    // The spiral covers the picture well within this many steps.
    let mut steps = 4 * m.len() + 16;
    while k < in_group0 && steps > 0 {
        steps -= 1;
        let at = (y * w + x) as usize;
        let vacant = m[at] == 1;
        if vacant {
            m[at] = 0;
        }
        if xd == -1 && x == left {
            left = (left - 1).max(0);
            x = left;
            (xd, yd) = (0, 2 * dir - 1);
        } else if xd == 1 && x == right {
            right = (right + 1).min(w - 1);
            x = right;
            (xd, yd) = (0, 1 - 2 * dir);
        } else if yd == -1 && y == top {
            top = (top - 1).max(0);
            y = top;
            (xd, yd) = (1 - 2 * dir, 0);
        } else if yd == 1 && y == bottom {
            bottom = (bottom + 1).min(h - 1);
            y = bottom;
            (xd, yd) = (2 * dir - 1, 0);
        } else {
            x += xd;
            y += yd;
        }
        k += vacant as usize;
    }
}
