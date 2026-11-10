//! H.264 (AVC) video → VA-API buffers, picture by picture: the parameter-set and slice-header
//! parsers (ITU-T H.264 §7.3), picture order count (§8.2.1), reference marking (§8.2.5) and
//! reference picture lists (§8.2.4); the slice data is the engine's.
//!
//! Frames, MBAFF frames and field pictures (PAFF), 8-bit 4:2:0, one slice group; SP/SI slices
//! and frame-number gaps are refused, and so are High 10 and 4:2:2 streams (VA-API has no
//! H.264 4:2:2 profile, and drivers rarely offer High 10). That covers what phone cameras,
//! camcorders and x264 write (Baseline, Main and High, progressive or interlaced). As in the
//! HEVC path, the DPB holds only reference surfaces: a frame is read back as soon as it is
//! decoded (both fields of a field pair go into one surface first) and the container's
//! composition times give the display order. A field without its pair is refused, so the
//! clip plays in software instead.

use crate::hevc::Reject;
use crate::params::{self as ffi, IqMatrixH264, PicParamH264, PictureH264, SliceParamH264};
use specular_heic::h265::bitreader::BitReader;
use specular_heic::h265::nal::strip_emulation_prevention;

type R<T> = Result<T, Reject>;

fn bad(s: impl Into<String>) -> Reject {
    Reject::Malformed(s.into())
}

fn unsup(s: impl Into<String>) -> Reject {
    Reject::Unsupported(s.into())
}

/// The bit reader with errors as [`Reject`]s and range checks on `ue(v)`.
struct Bits<'a>(BitReader<'a>);

impl Bits<'_> {
    fn flag(&mut self) -> R<bool> {
        self.0.u1().map(|b| b != 0).map_err(|_| bad("truncated"))
    }
    fn u(&mut self, n: u8) -> R<u32> {
        if n == 0 {
            return Ok(0);
        }
        self.0.u(n).map_err(|_| bad("truncated"))
    }
    fn ue(&mut self, max: u32, what: &str) -> R<u32> {
        let v = self.0.ue().map_err(|_| bad(format!("{what}: truncated")))?;
        if v > max {
            return Err(bad(format!("{what} {v} out of range")));
        }
        Ok(v)
    }
    fn se(&mut self, min: i32, max: i32, what: &str) -> R<i32> {
        let v = self.0.se().map_err(|_| bad(format!("{what}: truncated")))?;
        if !(min..=max).contains(&v) {
            return Err(bad(format!("{what} {v} out of range")));
        }
        Ok(v)
    }
    fn pos(&self) -> usize {
        self.0.bit_pos()
    }
}

/// `more_rbsp_data()`: is there anything before the RBSP stop bit?
fn more_rbsp_data(rbsp: &[u8], pos: usize) -> bool {
    let Some(last) = rbsp.iter().rposition(|&b| b != 0) else {
        return false;
    };
    let stop = last * 8 + 7 - rbsp[last].trailing_zeros() as usize;
    pos < stop
}

// Table 7-3 and 7-4, in the coded (zig-zag) order; `raster` turns them into the raster order VA
// takes.
const DEFAULT_4X4_INTRA: [u8; 16] = [
    6, 13, 13, 20, 20, 20, 28, 28, 28, 28, 32, 32, 32, 37, 37, 42,
];
const DEFAULT_4X4_INTER: [u8; 16] = [
    10, 14, 14, 20, 20, 20, 24, 24, 24, 24, 27, 27, 27, 30, 30, 34,
];
const DEFAULT_8X8_INTRA: [u8; 64] = [
    6, 10, 10, 13, 11, 13, 16, 16, 16, 16, 18, 18, 18, 18, 18, 23, 23, 23, 23, 23, 23, 25, 25, 25,
    25, 25, 25, 25, 27, 27, 27, 27, 27, 27, 27, 27, 29, 29, 29, 29, 29, 29, 29, 31, 31, 31, 31, 31,
    31, 33, 33, 33, 33, 33, 36, 36, 36, 36, 38, 38, 38, 40, 40, 42,
];
const DEFAULT_8X8_INTER: [u8; 64] = [
    9, 13, 13, 15, 13, 15, 17, 17, 17, 17, 19, 19, 19, 19, 19, 21, 21, 21, 21, 21, 21, 22, 22, 22,
    22, 22, 22, 22, 24, 24, 24, 24, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 27, 27, 27, 27, 27,
    27, 28, 28, 28, 28, 28, 30, 30, 30, 30, 32, 32, 32, 33, 33, 35,
];

/// One coded `scaling_list()` (or its absence).
#[derive(Clone, Debug, PartialEq, Eq)]
enum ListCode {
    Absent,
    UseDefault,
    List(Vec<u8>),
}

/// The 4×4 lists 0–5 and the 8×8 lists 6 (intra Y) and 7 (inter Y) of 4:2:0.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Lists {
    l4: [[u8; 16]; 6],
    l8: [[u8; 64]; 2],
}

const FLAT: Lists = Lists {
    l4: [[16; 16]; 6],
    l8: [[16; 64]; 2],
};

/// The frame zig-zag scans (§8.5.6): raster position of each coded index. Scaling lists use
/// them in field macroblocks too.
pub(crate) const ZIGZAG_4X4: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];
pub(crate) const ZIGZAG_8X8: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// A list in coded order laid out in raster order (`VAIQMatrixBufferH264` takes raster scan
/// order).
fn raster<const N: usize>(coded: &[u8; N], scan: &[usize; N]) -> [u8; N] {
    let mut out = [0; N];
    for (&v, &pos) in coded.iter().zip(scan) {
        out[pos] = v;
    }
    out
}

fn scaling_list(b: &mut Bits<'_>, size: usize) -> R<ListCode> {
    let (mut last, mut next) = (8i32, 8i32);
    let mut list = Vec::with_capacity(size);
    for j in 0..size {
        if next != 0 {
            let delta = b.se(-128, 127, "delta_scale")?;
            next = (last + delta + 256) % 256;
            if j == 0 && next == 0 {
                return Ok(ListCode::UseDefault);
            }
        }
        let v = if next == 0 { last } else { next };
        list.push(v as u8);
        last = v;
    }
    Ok(ListCode::List(list))
}

fn scaling_lists(b: &mut Bits<'_>, n: usize) -> R<Vec<ListCode>> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(if b.flag()? {
            scaling_list(b, if i < 6 { 16 } else { 64 })?
        } else {
            ListCode::Absent
        });
    }
    out.resize(8, ListCode::Absent);
    Ok(out)
}

/// Resolves coded lists with fall-back rule A (`base` = None: the defaults) or B (`base`: the
/// sequence-level lists), Table 7-2.
fn resolve(codes: &[ListCode], base: Option<&Lists>) -> Lists {
    let mut out = FLAT.clone();
    for (i, code) in codes.iter().enumerate().take(6) {
        out.l4[i] = match code {
            ListCode::List(l) => l[..16].try_into().unwrap_or([16; 16]),
            ListCode::UseDefault if i < 3 => DEFAULT_4X4_INTRA,
            ListCode::UseDefault => DEFAULT_4X4_INTER,
            ListCode::Absent => match (i, base) {
                (0, None) => DEFAULT_4X4_INTRA,
                (3, None) => DEFAULT_4X4_INTER,
                (0 | 3, Some(b)) => b.l4[i],
                _ => out.l4[i - 1],
            },
        };
    }
    for i in 0..2 {
        out.l8[i] = match &codes[6 + i] {
            ListCode::List(l) => l[..64].try_into().unwrap_or([16; 64]),
            ListCode::UseDefault | ListCode::Absent
                if base.is_none() || codes[6 + i] == ListCode::UseDefault =>
            {
                if i == 0 {
                    DEFAULT_8X8_INTRA
                } else {
                    DEFAULT_8X8_INTER
                }
            }
            _ => base.map_or([16; 64], |b| b.l8[i]),
        };
    }
    out
}

/// The video signal type from the VUI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VuiSignal {
    pub full_range: bool,
    pub colour: Option<(u8, u8, u8)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sps {
    id: u8,
    profile_idc: u8,
    level_idc: u8,
    bit_depth: u8,
    /// Lists after fall-back rule A; `None`: flat.
    scaling: Option<Lists>,
    log2_max_frame_num: u8,
    poc_type: u8,
    log2_max_poc_lsb: u8,
    delta_pic_order_always_zero: bool,
    offset_for_non_ref_pic: i32,
    offset_for_top_to_bottom_field: i32,
    offsets_for_ref_frame: Vec<i32>,
    max_num_ref_frames: u8,
    gaps_allowed: bool,
    width_mbs: u32,
    /// Frame height in macroblocks (twice the map units of a stream with fields).
    height_mbs: u32,
    frame_mbs_only: bool,
    mbaff: bool,
    direct_8x8_inference: bool,
    /// Frame cropping in luma samples: left, right, top, bottom.
    crop: [u32; 4],
    signal: Option<VuiSignal>,
    max_dec_frame_buffering: Option<u32>,
}

fn hrd(b: &mut Bits<'_>) -> R<()> {
    let n = b.ue(31, "cpb_cnt_minus1")? + 1;
    b.u(8)?;
    for _ in 0..n {
        b.ue(u32::MAX - 1, "bit_rate_value_minus1")?;
        b.ue(u32::MAX - 1, "cpb_size_value_minus1")?;
        b.flag()?;
    }
    b.u(20)?;
    Ok(())
}

impl Sps {
    pub fn parse(rbsp: &[u8]) -> R<Sps> {
        let mut b = Bits(BitReader::new(rbsp));
        let profile_idc = b.u(8)? as u8;
        b.u(8)?; // constraint_set flags, reserved
        let level_idc = b.u(8)? as u8;
        let id = b.ue(31, "seq_parameter_set_id")? as u8;
        let (mut chroma_format_idc, mut bit_depth) = (1, 8);
        let mut scaling = None;
        if matches!(
            profile_idc,
            100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
        ) {
            chroma_format_idc = b.ue(3, "chroma_format_idc")?;
            if chroma_format_idc == 3 {
                b.flag()?; // separate_colour_plane_flag
            }
            bit_depth = 8 + b.ue(6, "bit_depth_luma_minus8")? as u8;
            let chroma_depth = 8 + b.ue(6, "bit_depth_chroma_minus8")? as u8;
            if chroma_depth != bit_depth {
                return Err(unsup("different luma and chroma bit depths"));
            }
            if b.flag()? {
                return Err(unsup("lossless (qpprime_y_zero_transform_bypass)"));
            }
            if b.flag()? {
                let n = if chroma_format_idc == 3 { 12 } else { 8 };
                scaling = Some(resolve(&scaling_lists(&mut b, n)?, None));
            }
        }
        if chroma_format_idc != 1 {
            return Err(unsup(match chroma_format_idc {
                0 => "H.264 monochrome (4:0:0): no VA-API profile".to_string(),
                2 => "H.264 4:2:2 (High 4:2:2): no VA-API profile".to_string(),
                _ => "H.264 4:4:4: no VA-API profile".to_string(),
            }));
        }
        if bit_depth != 8 {
            return Err(unsup(format!(
                "{bit_depth}-bit H.264 (High 10): not decoded in hardware"
            )));
        }
        let log2_max_frame_num = 4 + b.ue(12, "log2_max_frame_num_minus4")? as u8;
        let poc_type = b.ue(2, "pic_order_cnt_type")? as u8;
        let mut log2_max_poc_lsb = 4;
        let (mut always_zero, mut non_ref, mut t2b) = (false, 0, 0);
        let mut offsets = Vec::new();
        match poc_type {
            0 => log2_max_poc_lsb = 4 + b.ue(12, "log2_max_pic_order_cnt_lsb_minus4")? as u8,
            1 => {
                always_zero = b.flag()?;
                non_ref = b.se(i32::MIN + 1, i32::MAX, "offset_for_non_ref_pic")?;
                t2b = b.se(i32::MIN + 1, i32::MAX, "offset_for_top_to_bottom_field")?;
                let n = b.ue(255, "num_ref_frames_in_pic_order_cnt_cycle")?;
                for _ in 0..n {
                    offsets.push(b.se(i32::MIN + 1, i32::MAX, "offset_for_ref_frame")?);
                }
            }
            _ => {}
        }
        let max_num_ref_frames = b.ue(16, "max_num_ref_frames")? as u8;
        let gaps_allowed = b.flag()?;
        let width_mbs = b.ue(1023, "pic_width_in_mbs_minus1")? + 1;
        let height_map_units = b.ue(1023, "pic_height_in_map_units_minus1")? + 1;
        let frame_mbs_only = b.flag()?;
        let mbaff = !frame_mbs_only && b.flag()?;
        let height_mbs = height_map_units * (2 - frame_mbs_only as u32);
        let direct_8x8_inference = b.flag()?;
        let mut crop = [0; 4];
        if b.flag()? {
            // CropUnitX 2, CropUnitY 2 × (2 − frame_mbs_only_flag) in 4:2:0.
            for (k, c) in crop.iter_mut().enumerate() {
                let unit = if k < 2 {
                    2
                } else {
                    2 * (2 - frame_mbs_only as u32)
                };
                *c = unit * b.ue(8192, "frame_crop_offset")?;
            }
        }
        if crop[0] + crop[1] >= width_mbs * 16 || crop[2] + crop[3] >= height_mbs * 16 {
            return Err(bad("frame cropping larger than the picture"));
        }
        let (mut signal, mut max_dec_frame_buffering) = (None, None);
        if b.flag()? {
            // vui_parameters(): only the signal type and the DPB size are kept; a truncated
            // VUI (some encoders) loses them, nothing else.
            let mut vui = || -> R<()> {
                if b.flag()? && b.u(8)? == 255 {
                    b.u(32)?;
                }
                if b.flag()? {
                    b.flag()?;
                }
                if b.flag()? {
                    b.u(3)?;
                    let full_range = b.flag()?;
                    let colour = if b.flag()? {
                        Some((b.u(8)? as u8, b.u(8)? as u8, b.u(8)? as u8))
                    } else {
                        None
                    };
                    signal = Some(VuiSignal { full_range, colour });
                }
                if b.flag()? {
                    b.ue(5, "chroma_sample_loc_type")?;
                    b.ue(5, "chroma_sample_loc_type")?;
                }
                if b.flag()? {
                    b.u(32)?;
                    b.u(32)?;
                    b.flag()?;
                }
                let nal_hrd = b.flag()?;
                if nal_hrd {
                    hrd(&mut b)?;
                }
                let vcl_hrd = b.flag()?;
                if vcl_hrd {
                    hrd(&mut b)?;
                }
                if nal_hrd || vcl_hrd {
                    b.flag()?;
                }
                b.flag()?; // pic_struct_present_flag
                if b.flag()? {
                    b.flag()?;
                    b.ue(16, "max_bytes_per_pic_denom")?;
                    b.ue(16, "max_bits_per_mb_denom")?;
                    b.ue(16, "log2_max_mv_length_horizontal")?;
                    b.ue(16, "log2_max_mv_length_vertical")?;
                    b.ue(16, "max_num_reorder_frames")?;
                    max_dec_frame_buffering = Some(b.ue(16, "max_dec_frame_buffering")?);
                }
                Ok(())
            };
            let _ = vui();
        }
        Ok(Sps {
            id,
            profile_idc,
            level_idc,
            bit_depth,
            scaling,
            log2_max_frame_num,
            poc_type,
            log2_max_poc_lsb,
            delta_pic_order_always_zero: always_zero,
            offset_for_non_ref_pic: non_ref,
            offset_for_top_to_bottom_field: t2b,
            offsets_for_ref_frame: offsets,
            max_num_ref_frames,
            gaps_allowed,
            width_mbs,
            height_mbs,
            frame_mbs_only,
            mbaff,
            direct_8x8_inference,
            crop,
            signal,
            max_dec_frame_buffering,
        })
    }

    pub fn coded_size(&self) -> (u32, u32) {
        (self.width_mbs * 16, self.height_mbs * 16)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pps {
    id: u8,
    sps_id: u8,
    entropy_coding_mode: bool,
    bottom_field_pic_order_in_frame_present: bool,
    num_ref_idx_default: [u32; 2],
    weighted_pred: bool,
    weighted_bipred_idc: u8,
    pic_init_qp_minus26: i32,
    pic_init_qs_minus26: i32,
    chroma_qp_index_offset: i32,
    second_chroma_qp_index_offset: i32,
    deblocking_filter_control_present: bool,
    constrained_intra_pred: bool,
    redundant_pic_cnt_present: bool,
    transform_8x8_mode: bool,
    /// The coded lists (resolved against the SPS with rule B when used).
    scaling: Option<Vec<ListCode>>,
}

impl Pps {
    pub fn parse(rbsp: &[u8]) -> R<Pps> {
        let mut b = Bits(BitReader::new(rbsp));
        let id = b.ue(255, "pic_parameter_set_id")? as u8;
        let sps_id = b.ue(31, "seq_parameter_set_id")? as u8;
        let entropy_coding_mode = b.flag()?;
        let bottom = b.flag()?;
        if b.ue(7, "num_slice_groups_minus1")? != 0 {
            return Err(unsup("slice groups (FMO)"));
        }
        let l0 = b.ue(31, "num_ref_idx_l0_default_active_minus1")? + 1;
        let l1 = b.ue(31, "num_ref_idx_l1_default_active_minus1")? + 1;
        let weighted_pred = b.flag()?;
        let weighted_bipred_idc = b.u(2)? as u8;
        if weighted_bipred_idc == 3 {
            return Err(bad("weighted_bipred_idc 3"));
        }
        let pic_init_qp_minus26 = b.se(-26, 25, "pic_init_qp_minus26")?;
        let pic_init_qs_minus26 = b.se(-26, 25, "pic_init_qs_minus26")?;
        let chroma_qp_index_offset = b.se(-12, 12, "chroma_qp_index_offset")?;
        let deblocking_filter_control_present = b.flag()?;
        let constrained_intra_pred = b.flag()?;
        let redundant_pic_cnt_present = b.flag()?;
        let (mut transform_8x8_mode, mut scaling, mut second) =
            (false, None, chroma_qp_index_offset);
        if more_rbsp_data(rbsp, b.pos()) {
            transform_8x8_mode = b.flag()?;
            if b.flag()? {
                scaling = Some(scaling_lists(&mut b, 6 + 2 * transform_8x8_mode as usize)?);
            }
            second = b.se(-12, 12, "second_chroma_qp_index_offset")?;
        }
        Ok(Pps {
            id,
            sps_id,
            entropy_coding_mode,
            bottom_field_pic_order_in_frame_present: bottom,
            num_ref_idx_default: [l0, l1],
            weighted_pred,
            weighted_bipred_idc,
            pic_init_qp_minus26,
            pic_init_qs_minus26,
            chroma_qp_index_offset,
            second_chroma_qp_index_offset: second,
            deblocking_filter_control_present,
            constrained_intra_pred,
            redundant_pic_cnt_present,
            transform_8x8_mode,
            scaling,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SliceKind {
    P,
    B,
    I,
}

/// One `modification_of_pic_nums_idc` operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListOp {
    Sub(u32),
    Add(u32),
    LongTerm(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mmco {
    UnmarkShort(u32),
    UnmarkLong(u32),
    ShortToLong(u32, u32),
    MaxLongIdx(u32),
    All,
    CurrentToLong(u32),
}

#[derive(Clone, Debug, Default)]
struct Weights {
    luma_denom: u8,
    chroma_denom: u8,
    /// Per list, per reference: (luma (weight, offset) when coded, chroma [(w, o); 2] when coded).
    #[allow(clippy::type_complexity)]
    lists: [Vec<(Option<(i16, i16)>, Option<[(i16, i16); 2]>)>; 2],
}

#[derive(Clone, Debug)]
struct SliceHeader {
    first_mb: u32,
    kind: SliceKind,
    raw_type: u8,
    pps_id: u8,
    frame_num: u32,
    field_pic: bool,
    bottom_field: bool,
    idr: bool,
    idr_pic_id: u32,
    ref_idc: u8,
    poc_lsb: u32,
    delta_poc_bottom: i32,
    delta_poc: [i32; 2],
    redundant_pic_cnt: u32,
    direct_spatial: bool,
    num_ref_idx: [u32; 2],
    mods: [Vec<ListOp>; 2],
    weights: Option<Weights>,
    long_term_reference: bool,
    mmcos: Option<Vec<Mmco>>,
    cabac_init_idc: u8,
    slice_qp_delta: i32,
    disable_deblocking: u8,
    alpha: i32,
    beta: i32,
    /// Bits of the header in the RBSP after the NAL header byte.
    header_bits: usize,
}

/// `slice_header()` up to the slice data. `ref_idc`: `nal_ref_idc`.
fn slice_header(rbsp: &[u8], idr: bool, ref_idc: u8, s: &Sps, p: &Pps) -> R<SliceHeader> {
    let mut b = Bits(BitReader::new(rbsp));
    let first_mb = b.ue(s.width_mbs * s.height_mbs - 1, "first_mb_in_slice")?;
    let raw_type = (b.ue(9, "slice_type")? % 5) as u8;
    let kind = match raw_type {
        0 => SliceKind::P,
        1 => SliceKind::B,
        2 => SliceKind::I,
        _ => return Err(unsup("SP/SI slices")),
    };
    if idr && kind != SliceKind::I {
        return Err(bad("an IDR picture with a predicted slice"));
    }
    let pps_id = b.ue(255, "pic_parameter_set_id")? as u8;
    if pps_id != p.id {
        return Err(bad("slice PPS mismatch"));
    }
    let frame_num = b.u(s.log2_max_frame_num)?;
    let field_pic = !s.frame_mbs_only && b.flag()?;
    let bottom_field = field_pic && b.flag()?;
    let idr_pic_id = if idr { b.ue(65535, "idr_pic_id")? } else { 0 };
    let (mut poc_lsb, mut delta_poc_bottom, mut delta_poc) = (0, 0, [0; 2]);
    let bottom_present = p.bottom_field_pic_order_in_frame_present && !field_pic;
    if s.poc_type == 0 {
        poc_lsb = b.u(s.log2_max_poc_lsb)?;
        if bottom_present {
            delta_poc_bottom = b.se(i32::MIN + 1, i32::MAX, "delta_pic_order_cnt_bottom")?;
        }
    }
    if s.poc_type == 1 && !s.delta_pic_order_always_zero {
        delta_poc[0] = b.se(i32::MIN + 1, i32::MAX, "delta_pic_order_cnt")?;
        if bottom_present {
            delta_poc[1] = b.se(i32::MIN + 1, i32::MAX, "delta_pic_order_cnt")?;
        }
    }
    let redundant_pic_cnt = if p.redundant_pic_cnt_present {
        b.ue(127, "redundant_pic_cnt")?
    } else {
        0
    };
    let direct_spatial = kind == SliceKind::B && b.flag()?;
    let mut num_ref_idx = p.num_ref_idx_default;
    if kind != SliceKind::I && b.flag()? {
        num_ref_idx[0] = b.ue(31, "num_ref_idx_l0_active_minus1")? + 1;
        if kind == SliceKind::B {
            num_ref_idx[1] = b.ue(31, "num_ref_idx_l1_active_minus1")? + 1;
        }
    }
    if kind == SliceKind::I {
        num_ref_idx = [0, 0];
    } else if kind == SliceKind::P {
        num_ref_idx[1] = 0;
    }
    let mut mods: [Vec<ListOp>; 2] = [vec![], vec![]];
    let lists = match kind {
        SliceKind::I => 0,
        SliceKind::P => 1,
        SliceKind::B => 2,
    };
    for m in mods.iter_mut().take(lists) {
        if b.flag()? {
            loop {
                let op = match b.ue(5, "modification_of_pic_nums_idc")? {
                    0 => ListOp::Sub(b.ue(1 << 17, "abs_diff_pic_num_minus1")?),
                    1 => ListOp::Add(b.ue(1 << 17, "abs_diff_pic_num_minus1")?),
                    2 => ListOp::LongTerm(b.ue(1 << 17, "long_term_pic_num")?),
                    3 => break,
                    v => return Err(bad(format!("modification_of_pic_nums_idc {v}"))),
                };
                if m.len() > 32 {
                    return Err(bad("too many list modifications"));
                }
                m.push(op);
            }
        }
    }
    let weights = if (p.weighted_pred && kind == SliceKind::P)
        || (p.weighted_bipred_idc == 1 && kind == SliceKind::B)
    {
        let mut w = Weights {
            luma_denom: b.ue(7, "luma_log2_weight_denom")? as u8,
            chroma_denom: b.ue(7, "chroma_log2_weight_denom")? as u8,
            lists: [vec![], vec![]],
        };
        for (l, &n) in num_ref_idx.iter().enumerate().take(lists) {
            for _ in 0..n {
                let luma = if b.flag()? {
                    Some((
                        b.se(-128, 127, "luma_weight")? as i16,
                        b.se(-128, 127, "luma_offset")? as i16,
                    ))
                } else {
                    None
                };
                let chroma = if b.flag()? {
                    let mut c = [(0, 0); 2];
                    for x in &mut c {
                        *x = (
                            b.se(-128, 127, "chroma_weight")? as i16,
                            b.se(-128, 127, "chroma_offset")? as i16,
                        );
                    }
                    Some(c)
                } else {
                    None
                };
                w.lists[l].push((luma, chroma));
            }
        }
        Some(w)
    } else {
        None
    };
    let (mut long_term_reference, mut mmcos) = (false, None);
    if ref_idc != 0 {
        if idr {
            b.flag()?; // no_output_of_prior_pics_flag
            long_term_reference = b.flag()?;
        } else if b.flag()? {
            let mut v = Vec::new();
            loop {
                let op = match b.ue(6, "memory_management_control_operation")? {
                    0 => break,
                    1 => Mmco::UnmarkShort(b.ue(1 << 17, "difference_of_pic_nums_minus1")?),
                    2 => Mmco::UnmarkLong(b.ue(63, "long_term_pic_num")?),
                    3 => Mmco::ShortToLong(
                        b.ue(1 << 17, "difference_of_pic_nums_minus1")?,
                        b.ue(31, "long_term_frame_idx")?,
                    ),
                    4 => Mmco::MaxLongIdx(b.ue(16, "max_long_term_frame_idx_plus1")?),
                    5 => Mmco::All,
                    _ => Mmco::CurrentToLong(b.ue(31, "long_term_frame_idx")?),
                };
                if v.len() > 66 {
                    return Err(bad("too many memory management operations"));
                }
                v.push(op);
            }
            mmcos = Some(v);
        }
    }
    let cabac_init_idc = if p.entropy_coding_mode && kind != SliceKind::I {
        b.ue(2, "cabac_init_idc")? as u8
    } else {
        0
    };
    let slice_qp_delta = b.se(-87, 77, "slice_qp_delta")?;
    let (mut disable_deblocking, mut alpha, mut beta) = (0, 0, 0);
    if p.deblocking_filter_control_present {
        disable_deblocking = b.ue(2, "disable_deblocking_filter_idc")? as u8;
        if disable_deblocking != 1 {
            alpha = b.se(-6, 6, "slice_alpha_c0_offset_div2")?;
            beta = b.se(-6, 6, "slice_beta_offset_div2")?;
        }
    }
    Ok(SliceHeader {
        first_mb,
        kind,
        raw_type,
        pps_id,
        frame_num,
        field_pic,
        bottom_field,
        idr,
        idr_pic_id,
        ref_idc,
        poc_lsb,
        delta_poc_bottom,
        delta_poc,
        redundant_pic_cnt,
        direct_spatial,
        num_ref_idx,
        mods,
        weights,
        long_term_reference,
        mmcos,
        cabac_init_idc,
        slice_qp_delta,
        disable_deblocking,
        alpha,
        beta,
        header_bits: b.pos(),
    })
}

/// `slice_pic_parameter_set_id` without parsing the rest: `first_mb_in_slice`, `slice_type`,
/// then the id.
fn peek_pps_id(rbsp: &[u8]) -> Option<u8> {
    let mut b = BitReader::new(rbsp);
    b.ue().ok()?;
    b.ue().ok()?;
    b.ue().ok().and_then(|v| u8::try_from(v).ok())
}

/// A reference entry in the DPB, one surface: a frame, a complementary field pair or a lone
/// field. Each field is marked on its own (bit = parity), so field pictures can reference
/// and unmark single fields, while frames only use entries with both fields marked.
#[derive(Clone, Copy, Debug)]
struct Ref {
    slot: usize,
    frame_num: u32,
    /// Field order counts (top, bottom); `i32::MAX` for a field not decoded.
    poc: [i32; 2],
    /// Fields marked "used for short-term reference".
    short: u8,
    /// Fields marked "used for long-term reference".
    long: u8,
    /// LongTermFrameIdx of the long-term fields.
    long_idx: u32,
}

impl Ref {
    fn marked(&self) -> u8 {
        self.short | self.long
    }

    /// PicOrderCnt of the entry: the smaller of its decoded fields' counts.
    fn entry_poc(&self) -> i32 {
        self.poc[0].min(self.poc[1])
    }
}

/// A list entry: a DPB index and the field (`None`: the frame).
type ListRef = (usize, Option<u8>);

/// A slice of a picture: its parsed header and its NAL unit.
type Slice<'a> = (SliceHeader, &'a [u8]);

/// A first field waiting for its second field.
#[derive(Clone, Copy, Debug)]
struct OpenField {
    slot: usize,
    parity: u8,
    frame_num: u32,
    reference: bool,
    idr: bool,
    /// Its field order count as later pictures see it.
    poc: i32,
}

/// The FrameNumOffset and field order counts of a picture, before it is decoded.
struct PocResult {
    frame_num_offset: i32,
    /// TopFieldOrderCnt and BottomFieldOrderCnt (a field has its own count in both).
    top: i32,
    bottom: i32,
    msb: i32,
}

/// One picture ready for the engine.
pub struct H264Job<'a> {
    pub pic: PicParamH264,
    pub iq: IqMatrixH264,
    pub slices: Vec<(SliceParamH264, &'a [u8])>,
    pub slot: usize,
}

/// The pictures of one sample.
pub struct H264Access<'a> {
    pub jobs: Vec<H264Job<'a>>,
    /// The surface of the frame the sample completes (a frame, or a field pair's second
    /// field); `None` when it completes none.
    pub show: Option<usize>,
}

/// The decoding state of one H.264 stream.
pub struct H264Stream {
    length_size: usize,
    sps: Vec<Option<Sps>>,
    pps: Vec<Option<Pps>>,
    max_w: u32,
    max_h: u32,
    dpb: Vec<Ref>,
    max_long_term_idx: Option<u32>,
    first: bool,
    /// Decoding started at a recovery point that is not an IDR picture (an open GOP): its
    /// POC and DPB slot. Its leading pictures (presented before it) refer to pictures never
    /// decoded; they are decoded against the recovery point picture instead, keeping the DPB
    /// as a decode from the start has it, and are not shown.
    leading_below: Option<(i32, usize)>,
    /// A decoded first field waiting for its second field.
    open: Option<OpenField>,
    // §8.2.1 state of the previous (reference) picture.
    prev_msb: i32,
    prev_lsb: i32,
    prev_frame_num: u32,
    prev_frame_num_offset: i32,
    prev_ref_frame_num: u32,
    slots: usize,
    pub coded: (u32, u32),
    pub crop: [u32; 4],
    pub signal: Option<VuiSignal>,
    /// Reference frames the stream declares (`max_num_ref_frames`, or the VUI DPB size).
    pub dpb_size: usize,
}

impl H264Stream {
    /// The stream's `avcC` record: its parameter sets, checked against the decoder's limits.
    pub fn new(avcc: &[u8], max_w: u32, max_h: u32) -> R<H264Stream> {
        if avcc.len() < 7 || avcc[0] != 1 {
            return Err(bad("avcC version"));
        }
        let length_size = (avcc[4] & 3) as usize + 1;
        if length_size == 3 {
            return Err(bad("avcC NAL length size 3"));
        }
        let mut st = H264Stream {
            length_size,
            sps: vec![None; 32],
            pps: vec![None; 256],
            max_w,
            max_h,
            dpb: Vec::new(),
            max_long_term_idx: None,
            first: true,
            leading_below: None,
            open: None,
            prev_msb: 0,
            prev_lsb: 0,
            prev_frame_num: 0,
            prev_frame_num_offset: 0,
            prev_ref_frame_num: 0,
            slots: 0,
            coded: (0, 0),
            crop: [0; 4],
            signal: None,
            dpb_size: 1,
        };
        let mut p = 5;
        let mut sets = Vec::new();
        for kind in 0..2 {
            let n = if kind == 0 {
                *avcc.get(p).ok_or_else(|| bad("avcC"))? as usize & 0x1f
            } else {
                *avcc.get(p).ok_or_else(|| bad("avcC"))? as usize
            };
            p += 1;
            for _ in 0..n {
                let len = avcc
                    .get(p..p + 2)
                    .map(|b| u16::from_be_bytes([b[0], b[1]]) as usize)
                    .ok_or_else(|| bad("avcC parameter set length"))?;
                let nal = avcc
                    .get(p + 2..p + 2 + len)
                    .ok_or_else(|| bad("avcC parameter set"))?;
                sets.push(nal);
                p += 2 + len;
            }
        }
        for nal in sets {
            st.param_set(nal)?;
        }
        let s = st
            .sps
            .iter()
            .flatten()
            .next()
            .ok_or_else(|| bad("avcC without an SPS"))?
            .clone();
        st.check_sps(&s)?;
        st.coded = s.coded_size();
        st.crop = s.crop;
        st.signal = s.signal;
        st.dpb_size = (s.max_num_ref_frames as usize)
            .max(s.max_dec_frame_buffering.unwrap_or(0) as usize)
            .clamp(1, 16);
        Ok(st)
    }

    fn check_sps(&self, s: &Sps) -> R<()> {
        let (w, h) = s.coded_size();
        if w > self.max_w || h > self.max_h || w < 32 || h < 32 {
            return Err(unsup(format!(
                "{w}×{h} outside the decoder's {}×{}",
                self.max_w, self.max_h
            )));
        }
        if !matches!(s.profile_idc, 66 | 77 | 100) {
            return Err(unsup(format!("H.264 profile_idc {}", s.profile_idc)));
        }
        Ok(())
    }

    /// Whether the stream codes fields (PAFF or MBAFF): its parameter sets say so.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn interlaced(&self) -> bool {
        self.sps.iter().flatten().any(|s| !s.frame_mbs_only)
    }

    pub fn surfaces_needed(&self) -> usize {
        (self.dpb_size + 1).min(17)
    }

    pub fn set_slots(&mut self, n: usize) {
        self.slots = n;
    }

    pub fn reset(&mut self) {
        self.dpb.clear();
        self.first = true;
        self.leading_below = None;
        // The picture order count state of a fresh stream: a recovery point (not an IDR
        // picture) derives its POC from it.
        (self.prev_msb, self.prev_lsb) = (0, 0);
        (self.prev_frame_num, self.prev_frame_num_offset) = (0, 0);
        self.prev_ref_frame_num = 0;
        self.open = None;
        self.max_long_term_idx = None;
    }

    pub fn forget_last(&mut self) {
        // The DPB was updated for the failed picture: start again at the next IDR.
        self.reset();
    }

    fn param_set(&mut self, nal: &[u8]) -> R<()> {
        if nal.len() < 2 {
            return Ok(());
        }
        let rbsp = strip_emulation_prevention(&nal[1..]);
        match nal[0] & 0x1f {
            7 => {
                let s = Sps::parse(&rbsp)?;
                let i = s.id as usize;
                self.sps[i] = Some(s);
            }
            8 => {
                let p = Pps::parse(&rbsp)?;
                let i = p.id as usize;
                self.pps[i] = Some(p);
            }
            _ => {}
        }
        Ok(())
    }

    /// Prepares one sample (an access unit, or both fields of a frame) for the engine: its
    /// pictures in decode order, and the surface holding the frame the sample completes
    /// (`None`: nothing to show yet, e.g. a sample with only a first field).
    pub fn prepare<'a>(&mut self, sample: &'a [u8], surfaces: &[ffi::Id]) -> R<H264Access<'a>> {
        let mut vcl: Vec<&'a [u8]> = Vec::new();
        let mut end_of_seq = false;
        let mut recovery = false;
        let mut p = 0;
        while p < sample.len() {
            let n = sample
                .get(p..p + self.length_size)
                .ok_or_else(|| bad("NAL length"))?
                .iter()
                .fold(0usize, |a, &b| a << 8 | b as usize);
            p += self.length_size;
            let nal = sample
                .get(p..p + n)
                .ok_or_else(|| bad("NAL past the sample"))?;
            p += n;
            if nal.is_empty() || nal[0] & 0x80 != 0 {
                return Err(bad("forbidden_zero_bit"));
            }
            match nal[0] & 0x1f {
                1 | 5 => vcl.push(nal),
                7 | 8 => self.param_set(nal)?,
                6 => recovery |= recovery_point(nal),
                2..=4 => return Err(unsup("data partitioning")),
                // End of sequence or stream (after the picture): the next one is an IDR.
                10 | 11 => end_of_seq = true,
                20 | 21 => {} // MVC / 3D extension views: the base view is the picture.
                _ => {}
            }
        }
        let mut out = H264Access {
            jobs: Vec::new(),
            show: None,
        };
        // Slices grouped into pictures (§7.4.1.2.4), each with its parameter sets.
        let mut pics: Vec<(Sps, Pps, Vec<Slice<'a>>)> = Vec::new();
        for nal in vcl {
            let idr = nal[0] & 0x1f == 5;
            let ref_idc = nal[0] >> 5;
            let rbsp = strip_emulation_prevention(&nal[1..nal.len().min(4096)]);
            let pps_id = peek_pps_id(&rbsp).ok_or_else(|| bad("slice PPS id"))?;
            let pps = self
                .pps
                .get(pps_id as usize)
                .and_then(Option::as_ref)
                .ok_or_else(|| bad(format!("PPS {pps_id} not sent")))?;
            let sps = self
                .sps
                .get(pps.sps_id as usize)
                .and_then(Option::as_ref)
                .ok_or_else(|| bad(format!("SPS {} not sent", pps.sps_id)))?;
            let sh = match slice_header(&rbsp, idr, ref_idc, sps, pps) {
                Ok(sh) => sh,
                // The 4 KiB prefix may cut a long header (many weights or MMCOs).
                Err(_) => slice_header(
                    &strip_emulation_prevention(&nal[1..]),
                    idr,
                    ref_idc,
                    sps,
                    pps,
                )?,
            };
            let new = match pics.last() {
                Some((s, _, hs)) => new_picture(&hs[0].0, &sh, s),
                None => true,
            };
            if new {
                pics.push((sps.clone(), pps.clone(), Vec::new()));
            }
            if sh.redundant_pic_cnt == 0 {
                pics.last_mut().expect("a picture").2.push((sh, nal));
            }
        }
        for (sps, pps, headers) in pics {
            if headers.is_empty() {
                continue;
            }
            if out.show.is_some() {
                return Err(unsup("a sample with more than one frame"));
            }
            if let Some((job, done)) = self.picture(&sps, &pps, headers, surfaces, recovery)? {
                if done {
                    out.show = Some(job.slot);
                }
                out.jobs.push(job);
            }
        }
        if end_of_seq {
            self.first = true;
            self.open = None;
        }
        Ok(out)
    }

    /// One picture (a frame or a field): POC, lists, VA parameters, then reference marking.
    /// `None`: skipped (waiting for an IDR picture); the flag: the frame is complete.
    fn picture<'a>(
        &mut self,
        sps: &Sps,
        pps: &Pps,
        headers: Vec<(SliceHeader, &'a [u8])>,
        surfaces: &[ffi::Id],
        recovery: bool,
    ) -> R<Option<(H264Job<'a>, bool)>> {
        let h0 = headers[0].0.clone();
        let (idr, ref_idc) = (h0.idr, h0.ref_idc);
        // Decoding starts at an IDR picture or a recovery point (open GOPs).
        let start = self.first && !idr;
        if start
            && !(recovery
                && !h0.field_pic
                && ref_idc != 0
                && headers.iter().all(|h| h.0.kind == SliceKind::I))
        {
            return Ok(None);
        }
        if idr {
            self.leading_below = None;
        }
        if sps.coded_size() != self.coded {
            return Err(unsup("the picture size changes mid-stream"));
        }
        self.check_sps(sps)?;
        let parity = h0.field_pic.then_some(h0.bottom_field as u8);
        let max_frame_num = 1u32 << sps.log2_max_frame_num;
        let frame_num = h0.frame_num;
        // The second field of the open first field: same frame, opposite parity.
        let open = self.open.take();
        let second = match (open, parity) {
            (Some(o), Some(par)) => {
                par != o.parity
                    && frame_num == o.frame_num
                    && (ref_idc != 0) == o.reference
                    && (!idr || o.idr)
            }
            _ => false,
        };
        if open.is_some() && !second {
            return Err(unsup("an H.264 field without its pair"));
        }
        if idr && frame_num != 0 {
            return Err(bad("IDR frame_num not 0"));
        }
        if second {
            // Same frame_num as the first field: neither a new IDR nor a gap.
        } else if idr {
            self.dpb.clear();
        } else if start {
            // A recovery point: its frame_num follows nothing decoded.
            self.dpb.clear();
        } else if frame_num != self.prev_ref_frame_num
            && frame_num != (self.prev_ref_frame_num + 1) % max_frame_num
        {
            return Err(unsup(if sps.gaps_allowed {
                "frame_num gaps"
            } else {
                "a frame_num gap (lost pictures)"
            }));
        }

        // §8.2.1 POC.
        let pr = self.compute_poc(sps, &h0);
        let cur_poc = pr.top.min(pr.bottom);

        let va_entry = |r: &Ref| PictureH264 {
            picture_id: surfaces[r.slot],
            frame_idx: if r.long != 0 { r.long_idx } else { r.frame_num },
            flags: if r.long != 0 {
                ffi::PICTURE_H264_LONG_TERM_REFERENCE
            } else {
                ffi::PICTURE_H264_SHORT_TERM_REFERENCE
            } | match r.marked() {
                1 => ffi::PICTURE_H264_TOP_FIELD,
                2 => ffi::PICTURE_H264_BOTTOM_FIELD,
                _ => 0,
            },
            top_field_order_cnt: known(r.poc[0]),
            bottom_field_order_cnt: known(r.poc[1]),
            reserved: [0; 4],
        };
        let mut reference_frames = [PictureH264::INVALID; 16];
        for (d, r) in reference_frames.iter_mut().zip(&self.dpb) {
            *d = va_entry(r);
        }

        // A surface for the current picture: the first field's for a second field.
        let slot = match open {
            Some(o) if second => o.slot,
            _ => (0..self.slots.min(surfaces.len()))
                .find(|&k| !self.dpb.iter().any(|r| r.slot == k))
                .ok_or_else(|| bad("the stream holds more references than its DPB size"))?,
        };
        if start {
            self.leading_below = Some((cur_poc, slot));
        }
        // A leading picture of the recovery point decoding started at.
        let leading = self
            .leading_below
            .filter(|&(poc, _)| cur_poc < poc)
            .and_then(|(_, s)| self.dpb.iter().position(|r| r.slot == s));

        let iq = {
            let l = match (&pps.scaling, &sps.scaling) {
                (Some(codes), base) => resolve(codes, Some(base.as_ref().unwrap_or(&FLAT))),
                (None, Some(s)) => s.clone(),
                (None, None) => FLAT,
            };
            IqMatrixH264 {
                scaling_list_4x4: l.l4.map(|c| raster(&c, &ZIGZAG_4X4)),
                scaling_list_8x8: l.l8.map(|c| raster(&c, &ZIGZAG_8X8)),
                reserved: [0; 4],
            }
        };

        // Slices: reference lists (§8.2.4) and weights.
        let va_ref = |(i, par): ListRef| {
            let r = &self.dpb[i];
            let long = match par {
                None => r.long == 3,
                Some(p) => r.long >> p & 1 != 0,
            };
            PictureH264 {
                picture_id: surfaces[r.slot],
                frame_idx: if long { r.long_idx } else { r.frame_num },
                flags: if long {
                    ffi::PICTURE_H264_LONG_TERM_REFERENCE
                } else {
                    ffi::PICTURE_H264_SHORT_TERM_REFERENCE
                } | match par {
                    Some(0) => ffi::PICTURE_H264_TOP_FIELD,
                    Some(_) => ffi::PICTURE_H264_BOTTOM_FIELD,
                    None => 0,
                },
                top_field_order_cnt: known(r.poc[0]),
                bottom_field_order_cnt: known(r.poc[1]),
                reserved: [0; 4],
            }
        };
        let mut slices = Vec::with_capacity(headers.len());
        for (k, (sh, nal)) in headers.iter().enumerate() {
            let mut sp = SliceParamH264 {
                slice_data_size: nal.len() as u32,
                slice_data_offset: 0,
                slice_data_flag: ffi::SLICE_DATA_FLAG_ALL,
                slice_data_bit_offset: u16::try_from(8 + sh.header_bits)
                    .map_err(|_| bad("slice header too long"))?,
                first_mb_in_slice: sh.first_mb as u16,
                slice_type: sh.raw_type,
                direct_spatial_mv_pred_flag: sh.direct_spatial as u8,
                num_ref_idx_l0_active_minus1: sh.num_ref_idx[0].saturating_sub(1) as u8,
                num_ref_idx_l1_active_minus1: sh.num_ref_idx[1].saturating_sub(1) as u8,
                cabac_init_idc: sh.cabac_init_idc,
                slice_qp_delta: sh.slice_qp_delta as i8,
                disable_deblocking_filter_idc: sh.disable_deblocking,
                slice_alpha_c0_offset_div2: sh.alpha as i8,
                slice_beta_offset_div2: sh.beta as i8,
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
            if sh.field_pic != h0.field_pic || sh.bottom_field != h0.bottom_field {
                return Err(bad("slices of one picture disagree"));
            }
            if sh.kind != SliceKind::I {
                if self.dpb.is_empty() {
                    return Err(bad(format!(
                        "slice {k}: a predicted slice without references"
                    )));
                }
                let mut lists = match (self.ref_lists(sh, parity, max_frame_num, cur_poc), leading)
                {
                    (Ok(l), _) => l,
                    (Err(e), None) => return Err(e),
                    (Err(_), Some(_)) => [Vec::new(), Vec::new()],
                };
                if let Some(rp) = leading {
                    // Its references before the recovery point were never decoded.
                    for (l, list) in lists.iter_mut().enumerate() {
                        list.resize(list.len().max(sh.num_ref_idx[l] as usize), None);
                        for e in list.iter_mut() {
                            e.get_or_insert((rp, None));
                        }
                    }
                }
                for (l, list) in lists.iter().enumerate() {
                    let out = if l == 0 {
                        &mut sp.ref_pic_list0
                    } else {
                        &mut sp.ref_pic_list1
                    };
                    for (d, e) in out.iter_mut().zip(list) {
                        if let Some(r) = e {
                            *d = va_ref(*r);
                        }
                    }
                }
                if let Some(w) = &sh.weights {
                    weights(&mut sp, w);
                }
            }
            slices.push((sp, *nal));
        }

        let seq_fields = 1 // chroma_format_idc 4:2:0
            | (sps.gaps_allowed as u32) << 3
            | (sps.frame_mbs_only as u32) << 4
            | (sps.mbaff as u32) << 5
            | (sps.direct_8x8_inference as u32) << 6
            | ((sps.level_idc >= 31) as u32) << 7
            | ((sps.log2_max_frame_num - 4) as u32) << 8
            | (sps.poc_type as u32) << 12
            | ((sps.log2_max_poc_lsb - 4) as u32) << 14
            | (sps.delta_pic_order_always_zero as u32) << 18;
        let pic_fields = pps.entropy_coding_mode as u32
            | (pps.weighted_pred as u32) << 1
            | (pps.weighted_bipred_idc as u32) << 2
            | (pps.transform_8x8_mode as u32) << 4
            | (h0.field_pic as u32) << 5
            | (pps.constrained_intra_pred as u32) << 6
            | (pps.bottom_field_pic_order_in_frame_present as u32) << 7
            | (pps.deblocking_filter_control_present as u32) << 8
            | (pps.redundant_pic_cnt_present as u32) << 9
            | ((ref_idc != 0) as u32) << 10;
        // The current picture's field order counts: a field has its own, plus the first
        // field's when it is the second.
        let mut cur_pocs = [i32::MAX; 2];
        match parity {
            None => cur_pocs = [pr.top, pr.bottom],
            Some(p) => {
                cur_pocs[p as usize] = pr.top;
                if let Some(o) = open.filter(|_| second) {
                    cur_pocs[o.parity as usize] = o.poc;
                }
            }
        }
        let pic = PicParamH264 {
            curr_pic: PictureH264 {
                picture_id: surfaces[slot],
                frame_idx: frame_num,
                flags: if ref_idc == 0 {
                    0
                } else if idr && h0.long_term_reference {
                    ffi::PICTURE_H264_LONG_TERM_REFERENCE
                } else {
                    ffi::PICTURE_H264_SHORT_TERM_REFERENCE
                } | match parity {
                    Some(0) => ffi::PICTURE_H264_TOP_FIELD,
                    Some(_) => ffi::PICTURE_H264_BOTTOM_FIELD,
                    None => 0,
                },
                top_field_order_cnt: known(cur_pocs[0]),
                bottom_field_order_cnt: known(cur_pocs[1]),
                reserved: [0; 4],
            },
            reference_frames,
            picture_width_in_mbs_minus1: (sps.width_mbs - 1) as u16,
            picture_height_in_mbs_minus1: (sps.height_mbs - 1) as u16,
            bit_depth_luma_minus8: 0,
            bit_depth_chroma_minus8: 0,
            num_ref_frames: sps.max_num_ref_frames,
            seq_fields,
            num_slice_groups_minus1: 0,
            slice_group_map_type: 0,
            slice_group_change_rate_minus1: 0,
            pic_init_qp_minus26: pps.pic_init_qp_minus26 as i8,
            pic_init_qs_minus26: pps.pic_init_qs_minus26 as i8,
            chroma_qp_index_offset: pps.chroma_qp_index_offset as i8,
            second_chroma_qp_index_offset: pps.second_chroma_qp_index_offset as i8,
            pic_fields,
            frame_num: frame_num as u16,
            reserved: [0; 8],
        };

        // §8.2.5 marking, after the picture.
        self.first = false;
        let out_poc = self.update_poc(sps, &h0, &pr);
        if ref_idc != 0 {
            self.mark(sps, &h0, slot, parity, second, out_poc)?;
        }
        if let Some(p) = parity.filter(|_| !second) {
            self.open = Some(OpenField {
                slot,
                parity: p,
                frame_num,
                reference: ref_idc != 0,
                idr,
                poc: out_poc[p as usize],
            });
        }
        let job = H264Job {
            pic,
            iq,
            slices,
            slot,
        };
        // A leading picture is decoded for the DPB only.
        Ok(Some((
            job,
            (parity.is_none() || second) && leading.is_none(),
        )))
    }

    /// §8.2.1: the field order counts of a picture from the previous picture's state.
    fn compute_poc(&self, s: &Sps, h: &SliceHeader) -> PocResult {
        let max_frame_num = 1i64 << s.log2_max_frame_num;
        let frame_num_offset = if h.idr {
            0
        } else if self.prev_frame_num > h.frame_num {
            self.prev_frame_num_offset as i64 + max_frame_num
        } else {
            self.prev_frame_num_offset as i64
        };
        let parity = h.field_pic.then_some(h.bottom_field as u8);
        let mut msb = 0;
        let (top, bottom) = match s.poc_type {
            0 => {
                let (prev_msb, prev_lsb) = if h.idr {
                    (0, 0)
                } else {
                    (self.prev_msb as i64, self.prev_lsb as i64)
                };
                let max = 1i64 << s.log2_max_poc_lsb;
                let lsb = h.poc_lsb as i64;
                msb = if lsb < prev_lsb && prev_lsb - lsb >= max / 2 {
                    prev_msb + max
                } else if lsb > prev_lsb && lsb - prev_lsb > max / 2 {
                    prev_msb - max
                } else {
                    prev_msb
                };
                let v = msb + lsb;
                if h.field_pic {
                    (v, v)
                } else {
                    (v, v + h.delta_poc_bottom as i64)
                }
            }
            1 => {
                let n = s.offsets_for_ref_frame.len() as i64;
                let mut abs = if n != 0 {
                    frame_num_offset + h.frame_num as i64
                } else {
                    0
                };
                if h.ref_idc == 0 && abs > 0 {
                    abs -= 1;
                }
                let per_cycle: i64 = s.offsets_for_ref_frame.iter().map(|&x| x as i64).sum();
                let mut expected = if abs > 0 {
                    let (cycle, in_cycle) = ((abs - 1) / n, (abs - 1) % n);
                    cycle * per_cycle
                        + s.offsets_for_ref_frame[..=in_cycle as usize]
                            .iter()
                            .map(|&x| x as i64)
                            .sum::<i64>()
                } else {
                    0
                };
                if h.ref_idc == 0 {
                    expected += s.offset_for_non_ref_pic as i64;
                }
                let t2b = s.offset_for_top_to_bottom_field as i64;
                match parity {
                    None => {
                        let top = expected + h.delta_poc[0] as i64;
                        (top, top + t2b + h.delta_poc[1] as i64)
                    }
                    Some(0) => {
                        let v = expected + h.delta_poc[0] as i64;
                        (v, v)
                    }
                    Some(_) => {
                        let v = expected + t2b + h.delta_poc[0] as i64;
                        (v, v)
                    }
                }
            }
            _ => {
                let t = if h.idr {
                    0
                } else if h.ref_idc == 0 {
                    2 * (frame_num_offset + h.frame_num as i64) - 1
                } else {
                    2 * (frame_num_offset + h.frame_num as i64)
                };
                (t, t)
            }
        };
        let clamp = |v: i64| v.clamp(i32::MIN as i64 + 1, i32::MAX as i64 - 1) as i32;
        PocResult {
            frame_num_offset: clamp(frame_num_offset),
            top: clamp(top),
            bottom: clamp(bottom),
            msb: clamp(msb),
        }
    }

    /// Updates the POC state after a picture; returns its field order counts as later
    /// pictures see them (changed by memory_management_control_operation 5).
    fn update_poc(&mut self, s: &Sps, h: &SliceHeader, r: &PocResult) -> [i32; 2] {
        let mmco5 = has_mmco5(h);
        let (mut top, mut bottom) = (r.top, r.bottom);
        if mmco5 {
            if h.field_pic {
                (top, bottom) = (0, 0);
            } else {
                let t = top.min(bottom);
                top -= t;
                bottom -= t;
            }
        }
        if h.ref_idc != 0 && s.poc_type == 0 {
            if mmco5 {
                self.prev_msb = 0;
                self.prev_lsb = if h.field_pic && h.bottom_field {
                    0
                } else {
                    top
                };
            } else {
                self.prev_msb = r.msb;
                self.prev_lsb = h.poc_lsb as i32;
            }
        }
        self.prev_frame_num_offset = if mmco5 { 0 } else { r.frame_num_offset };
        self.prev_frame_num = if mmco5 { 0 } else { h.frame_num };
        [top, bottom]
    }

    /// §8.2.4.2 initial lists and §8.2.4.3 modifications for a frame (`parity` None) or a
    /// field (`None` entries: "no reference picture"). `cur_poc`: PicOrderCnt(CurrPic).
    fn ref_lists(
        &self,
        h: &SliceHeader,
        parity: Option<u8>,
        max: u32,
        cur_poc: i32,
    ) -> R<[Vec<Option<ListRef>>; 2]> {
        let cur = h.frame_num;
        let dpb = &self.dpb;
        let mut init: [Vec<ListRef>; 2] = [vec![], vec![]];
        match parity {
            None => {
                let mut long: Vec<usize> = (0..dpb.len()).filter(|&i| dpb[i].long == 3).collect();
                long.sort_by_key(|&i| dpb[i].long_idx);
                let short: Vec<usize> = (0..dpb.len()).filter(|&i| dpb[i].short == 3).collect();
                let frame = |i: &usize| (*i, None);
                if h.kind == SliceKind::P {
                    let mut s = short;
                    s.sort_by_key(|&i| std::cmp::Reverse(wrap(&dpb[i], cur, max)));
                    init[0] = s.iter().chain(&long).map(frame).collect();
                } else {
                    let (before, after) = split_by_poc(dpb, short, cur_poc);
                    init[0] = before
                        .iter()
                        .chain(&after)
                        .chain(&long)
                        .map(frame)
                        .collect();
                    init[1] = after
                        .iter()
                        .chain(&before)
                        .chain(&long)
                        .map(frame)
                        .collect();
                }
            }
            Some(par) => {
                let mut long: Vec<usize> = (0..dpb.len()).filter(|&i| dpb[i].long != 0).collect();
                long.sort_by_key(|&i| dpb[i].long_idx);
                let short: Vec<usize> = (0..dpb.len()).filter(|&i| dpb[i].short != 0).collect();
                let long_fields = alternate(dpb, &long, par, |r| r.long);
                if h.kind == SliceKind::P {
                    let mut s = short;
                    s.sort_by_key(|&i| std::cmp::Reverse(wrap(&dpb[i], cur, max)));
                    init[0] = alternate(dpb, &s, par, |r| r.short);
                    init[0].extend(long_fields);
                } else {
                    let (before, after) = split_by_poc(dpb, short, cur_poc);
                    let l0: Vec<usize> = before.iter().chain(&after).copied().collect();
                    let l1: Vec<usize> = after.iter().chain(&before).copied().collect();
                    init[0] = alternate(dpb, &l0, par, |r| r.short);
                    init[0].extend(long_fields.iter().copied());
                    init[1] = alternate(dpb, &l1, par, |r| r.short);
                    init[1].extend(long_fields);
                }
            }
        }
        if h.kind == SliceKind::B && init[1].len() > 1 && init[1] == init[0] {
            init[1].swap(0, 1);
        }
        // PicNum and LongTermPicNum (§8.2.4.1).
        let (cur_pic_num, max_pic_num) = match parity {
            None => (cur as i64, max as i64),
            Some(_) => (2 * cur as i64 + 1, 2 * max as i64),
        };
        let mut out: [Vec<Option<ListRef>>; 2] = [vec![], vec![]];
        for l in 0..2 {
            let n = h.num_ref_idx[l] as usize;
            if n == 0 {
                continue;
            }
            let mut list: Vec<Option<ListRef>> = init[l].iter().map(|&r| Some(r)).collect();
            list.truncate(n);
            list.resize(n, None);
            let mut pred = cur_pic_num;
            for (ref_idx, op) in h.mods[l].iter().enumerate() {
                if ref_idx >= n {
                    return Err(bad("more list modifications than references"));
                }
                let target = match *op {
                    ListOp::Sub(d) | ListOp::Add(d) => {
                        let d = d as i64 + 1;
                        let mut no_wrap = if matches!(op, ListOp::Sub(_)) {
                            pred - d
                        } else {
                            pred + d
                        };
                        if no_wrap < 0 {
                            no_wrap += max_pic_num;
                        } else if no_wrap >= max_pic_num {
                            no_wrap -= max_pic_num;
                        }
                        pred = no_wrap;
                        let num = if no_wrap > cur_pic_num {
                            no_wrap - max_pic_num
                        } else {
                            no_wrap
                        };
                        self.short_by_pic_num(num, parity, cur, max)
                            .map(|(i, bits)| (i, parity.map(|_| bits >> 1)))
                            .ok_or_else(|| {
                                bad(format!("list modification to missing picture {num}"))
                            })?
                    }
                    ListOp::LongTerm(num) => self
                        .long_by_pic_num(num as i64, parity)
                        .map(|(i, bits)| (i, parity.map(|_| bits >> 1)))
                        .ok_or_else(|| bad("list modification to a missing long-term picture"))?,
                };
                list.insert(ref_idx, Some(target));
                let mut k = ref_idx + 1;
                while k < list.len() {
                    if list[k] == Some(target) {
                        list.remove(k);
                    } else {
                        k += 1;
                    }
                }
                list.resize(n, None);
            }
            list.truncate(n);
            if list.first().is_none_or(Option::is_none) {
                return Err(bad("an empty reference list"));
            }
            out[l] = list;
        }
        Ok(out)
    }

    /// The short-term frame (or field, for a field picture of parity `par`) with PicNum `pn`:
    /// its DPB index and the field bits it covers.
    fn short_by_pic_num(
        &self,
        pn: i64,
        par: Option<u8>,
        cur: u32,
        max: u32,
    ) -> Option<(usize, u8)> {
        match par {
            None => self
                .dpb
                .iter()
                .position(|r| r.short == 3 && wrap(r, cur, max) == pn)
                .map(|i| (i, 3)),
            Some(par) => {
                let (p, wr) = field_num(pn, par);
                self.dpb
                    .iter()
                    .position(|r| r.short >> p & 1 != 0 && wrap(r, cur, max) == wr)
                    .map(|i| (i, 1 << p))
            }
        }
    }

    /// The long-term frame or field with LongTermPicNum `n`.
    fn long_by_pic_num(&self, n: i64, par: Option<u8>) -> Option<(usize, u8)> {
        match par {
            None => self
                .dpb
                .iter()
                .position(|r| r.long == 3 && r.long_idx as i64 == n)
                .map(|i| (i, 3)),
            Some(par) => {
                let (p, idx) = field_num(n, par);
                self.dpb
                    .iter()
                    .position(|r| r.long >> p & 1 != 0 && r.long_idx as i64 == idx)
                    .map(|i| (i, 1 << p))
            }
        }
    }

    /// §8.2.5.3, keeping the entry in surface `keep` (the current frame's first field).
    fn sliding_window(&mut self, s: &Sps, cur: u32, keep: Option<usize>) {
        let max_refs = (s.max_num_ref_frames as usize).max(1);
        let max = 1u32 << s.log2_max_frame_num;
        while self.dpb.iter().filter(|r| Some(r.slot) != keep).count() >= max_refs {
            let victim = (0..self.dpb.len())
                .filter(|&i| self.dpb[i].short != 0 && Some(self.dpb[i].slot) != keep)
                .min_by_key(|&i| wrap(&self.dpb[i], cur, max));
            match victim.or_else(|| self.dpb.iter().position(|r| Some(r.slot) != keep)) {
                Some(i) => {
                    self.dpb.remove(i);
                }
                None => break,
            }
        }
    }

    /// §8.2.5: marks the current (reference) picture, field or frame, and applies MMCOs or
    /// the sliding window.
    fn mark(
        &mut self,
        s: &Sps,
        h: &SliceHeader,
        slot: usize,
        parity: Option<u8>,
        mut second: bool,
        poc: [i32; 2],
    ) -> R<()> {
        let max = 1u32 << s.log2_max_frame_num;
        let fnum = h.frame_num;
        let bits = parity.map_or(3u8, |p| 1 << p);
        let mut long_idx = None;
        if h.idr {
            self.dpb.retain(|r| second && r.slot == slot);
            if h.long_term_reference {
                long_idx = Some(0);
                self.max_long_term_idx = Some(0);
            } else {
                self.max_long_term_idx = None;
            }
        } else if let Some(ops) = &h.mmcos {
            let cur_pic_num = match parity {
                None => fnum as i64,
                Some(_) => 2 * fnum as i64 + 1,
            };
            for op in ops {
                match *op {
                    Mmco::UnmarkShort(d) => {
                        let pn = cur_pic_num - (d as i64 + 1);
                        if let Some((i, b)) = self.short_by_pic_num(pn, parity, fnum, max) {
                            self.dpb[i].short &= !b;
                        }
                    }
                    Mmco::UnmarkLong(n) => {
                        if let Some((i, b)) = self.long_by_pic_num(n as i64, parity) {
                            self.dpb[i].long &= !b;
                        }
                    }
                    Mmco::ShortToLong(d, idx) => {
                        let pn = cur_pic_num - (d as i64 + 1);
                        if let Some((i, b)) = self.short_by_pic_num(pn, parity, fnum, max) {
                            let target = self.dpb[i].slot;
                            // A long-term frame or pair, or a field of another frame, holding
                            // the index loses it.
                            for r in &mut self.dpb {
                                if r.long != 0
                                    && r.long_idx == idx
                                    && (r.slot != target || parity.is_none())
                                {
                                    r.long = 0;
                                }
                            }
                            let r = &mut self.dpb[i];
                            if r.long != 0 && r.long_idx != idx {
                                r.long = 0;
                            }
                            r.short &= !b;
                            r.long |= b;
                            r.long_idx = idx;
                        }
                    }
                    Mmco::MaxLongIdx(plus1) => {
                        for r in &mut self.dpb {
                            if r.long != 0 && (plus1 == 0 || r.long_idx >= plus1) {
                                r.long = 0;
                            }
                        }
                        self.max_long_term_idx = plus1.checked_sub(1);
                    }
                    Mmco::All => {
                        self.dpb.clear();
                        self.max_long_term_idx = None;
                    }
                    Mmco::CurrentToLong(idx) => {
                        // Entries holding the index lose it, except the current frame's
                        // first field.
                        for r in &mut self.dpb {
                            if r.long != 0
                                && r.long_idx == idx
                                && (parity.is_none() || r.slot != slot)
                            {
                                r.long = 0;
                            }
                        }
                        long_idx = Some(idx);
                    }
                }
                self.dpb
                    .retain(|r| r.marked() != 0 || (second && r.slot == slot));
            }
            if has_mmco5(h) {
                second = false;
            }
        } else {
            let first_short = second && self.dpb.iter().any(|r| r.slot == slot && r.short != 0);
            if !first_short {
                self.sliding_window(s, fnum, second.then_some(slot));
            }
        }
        let frame_num = if has_mmco5(h) { 0 } else { fnum };
        let i = match self.dpb.iter().position(|r| r.slot == slot) {
            Some(i) if second => i,
            other => {
                if let Some(i) = other {
                    // A lone first field whose surface is taken over (after MMCO 5).
                    self.dpb.remove(i);
                }
                if self.dpb.len() >= (s.max_num_ref_frames as usize).max(1) {
                    self.sliding_window(s, fnum, None);
                }
                if self.dpb.len() >= 16 {
                    return Err(bad("more than 16 reference frames"));
                }
                self.dpb.push(Ref {
                    slot,
                    frame_num,
                    poc: [i32::MAX; 2],
                    short: 0,
                    long: 0,
                    long_idx: 0,
                });
                self.dpb.len() - 1
            }
        };
        let r = &mut self.dpb[i];
        for (p, (dst, src)) in r.poc.iter_mut().zip(poc).enumerate() {
            if bits >> p & 1 != 0 {
                *dst = src;
            }
        }
        match long_idx {
            Some(idx) => {
                if r.long != 0 && r.long_idx != idx {
                    r.long = 0;
                }
                r.long |= bits;
                r.long_idx = idx;
            }
            None => r.short |= bits,
        }
        self.prev_ref_frame_num = frame_num;
        Ok(())
    }
}

/// Whether an SEI NAL unit holds a recovery point with recovery_frame_cnt 0 (an open GOP's
/// I picture, where decoding may start).
fn recovery_point(nal: &[u8]) -> bool {
    let rbsp = strip_emulation_prevention(&nal[1..]);
    let mut p = 0;
    // sei_message()s: payloadType and payloadSize in 0xFF-continued bytes.
    while p + 1 < rbsp.len() {
        let mut read = || {
            let mut v = 0usize;
            loop {
                let b = *rbsp.get(p)?;
                p += 1;
                v += b as usize;
                if b != 0xFF {
                    return Some(v);
                }
            }
        };
        let (Some(ty), Some(size)) = (read(), read()) else {
            return false;
        };
        if ty == 6 {
            // recovery_frame_cnt ue(v) is 0 when its first bit is set.
            return p + size <= rbsp.len() && rbsp.get(p).is_some_and(|b| b & 0x80 != 0);
        }
        p += size;
    }
    false
}

/// Whether the slice header has memory_management_control_operation 5.
fn has_mmco5(h: &SliceHeader) -> bool {
    h.mmcos.as_ref().is_some_and(|v| v.contains(&Mmco::All))
}

/// Whether slice `h` starts a new picture after `prev` (§7.4.1.2.4).
fn new_picture(prev: &SliceHeader, h: &SliceHeader, s: &Sps) -> bool {
    h.first_mb == 0
        || prev.frame_num != h.frame_num
        || prev.pps_id != h.pps_id
        || prev.field_pic != h.field_pic
        || prev.bottom_field != h.bottom_field
        || (prev.ref_idc == 0) != (h.ref_idc == 0)
        || prev.idr != h.idr
        || (h.idr && prev.idr_pic_id != h.idr_pic_id)
        || (s.poc_type == 0
            && (prev.poc_lsb != h.poc_lsb || prev.delta_poc_bottom != h.delta_poc_bottom))
        || (s.poc_type == 1 && prev.delta_poc != h.delta_poc)
}

/// An unknown field order count (a field not decoded) as VA takes it.
fn known(poc: i32) -> i32 {
    if poc == i32::MAX { 0 } else { poc }
}

/// FrameNumWrap (§8.2.4.1), which is also the PicNum of a frame.
fn wrap(r: &Ref, cur: u32, max: u32) -> i64 {
    if r.frame_num > cur {
        r.frame_num as i64 - max as i64
    } else {
        r.frame_num as i64
    }
}

/// A field PicNum or LongTermPicNum seen from a field of parity `par`: the field's parity
/// and its FrameNumWrap or LongTermFrameIdx (§8.2.4.1).
fn field_num(n: i64, par: u8) -> (u8, i64) {
    let same = n & 1 != 0;
    (if same { par } else { par ^ 1 }, n >> 1)
}

/// Short-term entries with POC at most `cur_poc` (descending), and the rest (ascending).
fn split_by_poc(dpb: &[Ref], short: Vec<usize>, cur_poc: i32) -> (Vec<usize>, Vec<usize>) {
    let (mut before, mut after): (Vec<usize>, Vec<usize>) = short
        .into_iter()
        .partition(|&i| dpb[i].entry_poc() <= cur_poc);
    before.sort_by_key(|&i| std::cmp::Reverse(dpb[i].entry_poc()));
    after.sort_by_key(|&i| dpb[i].entry_poc());
    (before, after)
}

/// §8.2.4.2.5: the fields of `entries` marked in `mask`, alternating parity starting with
/// `par`, then the rest of the parity left.
fn alternate(dpb: &[Ref], entries: &[usize], par: u8, mask: impl Fn(&Ref) -> u8) -> Vec<ListRef> {
    let mut out = Vec::new();
    let mut next = [0usize; 2];
    let mut p = par as usize;
    let n = entries.len();
    loop {
        while next[p] < n && mask(&dpb[entries[next[p]]]) >> p & 1 == 0 {
            next[p] += 1;
        }
        if next[p] < n {
            out.push((entries[next[p]], Some(p as u8)));
            next[p] += 1;
            p ^= 1;
            continue;
        }
        let q = p ^ 1;
        while next[q] < n {
            if mask(&dpb[entries[next[q]]]) >> q & 1 != 0 {
                out.push((entries[next[q]], Some(q as u8)));
            }
            next[q] += 1;
        }
        return out;
    }
}

/// The pred-weight table in VA's form: every active entry filled, the inferred defaults
/// (`2^denom`, 0) where the slice codes none.
fn weights(sp: &mut SliceParamH264, w: &Weights) {
    sp.luma_log2_weight_denom = w.luma_denom;
    sp.chroma_log2_weight_denom = w.chroma_denom;
    let (ld, cd) = (1i16 << w.luma_denom, 1i16 << w.chroma_denom);
    for (l, entries) in w.lists.iter().enumerate() {
        let any_luma = entries.iter().any(|e| e.0.is_some()) as u8;
        let any_chroma = entries.iter().any(|e| e.1.is_some()) as u8;
        let (lw, lo, cw, co) = if l == 0 {
            sp.luma_weight_l0_flag = any_luma;
            sp.chroma_weight_l0_flag = any_chroma;
            (
                &mut sp.luma_weight_l0,
                &mut sp.luma_offset_l0,
                &mut sp.chroma_weight_l0,
                &mut sp.chroma_offset_l0,
            )
        } else {
            sp.luma_weight_l1_flag = any_luma;
            sp.chroma_weight_l1_flag = any_chroma;
            (
                &mut sp.luma_weight_l1,
                &mut sp.luma_offset_l1,
                &mut sp.chroma_weight_l1,
                &mut sp.chroma_offset_l1,
            )
        };
        for (i, (luma, chroma)) in entries.iter().enumerate().take(32) {
            (lw[i], lo[i]) = luma.unwrap_or((ld, 0));
            let c = chroma.unwrap_or([(cd, 0); 2]);
            for j in 0..2 {
                (cw[i][j], co[i][j]) = c[j];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_point_sei() {
        // User data (type 5, 3 bytes), then a recovery point with recovery_frame_cnt 0.
        assert!(recovery_point(&[6, 5, 3, 1, 2, 3, 6, 1, 0b1100_0100, 0x80]));
        // recovery_frame_cnt 2.
        assert!(!recovery_point(&[6, 6, 1, 0b0111_0010, 0x80]));
        // Only user data; a truncated message.
        assert!(!recovery_point(&[6, 5, 1, 0, 0x80]));
        assert!(!recovery_point(&[6, 6, 9, 0x80]));
    }

    #[test]
    fn rbsp_stop_bit() {
        // 0b1000_0000: the stop bit at position 0.
        assert!(!more_rbsp_data(&[0x80], 0));
        // 0b0100_0000: one data bit before it.
        assert!(more_rbsp_data(&[0x40], 0));
        assert!(!more_rbsp_data(&[0x40], 1));
        assert!(more_rbsp_data(&[0xff, 0x80, 0x00], 7));
        assert!(!more_rbsp_data(&[0xff, 0x80, 0x00], 8));
    }

    #[test]
    fn scaling_fallbacks() {
        // Nothing coded: rule A gives the defaults, list 1 copies 0, 4 copies 3.
        let absent = vec![ListCode::Absent; 8];
        let a = resolve(&absent, None);
        assert_eq!(a.l4[0], DEFAULT_4X4_INTRA);
        assert_eq!(a.l4[2], DEFAULT_4X4_INTRA);
        assert_eq!(a.l4[5], DEFAULT_4X4_INTER);
        assert_eq!(a.l8, [DEFAULT_8X8_INTRA, DEFAULT_8X8_INTER]);
        // Rule B takes lists 0, 3, 6 and 7 from the sequence.
        let b = resolve(&absent, Some(&FLAT));
        assert_eq!(b, FLAT);
        let mut codes = absent.clone();
        codes[3] = ListCode::List(vec![7; 16]);
        codes[7] = ListCode::UseDefault;
        let c = resolve(&codes, Some(&FLAT));
        assert_eq!(c.l4[0], [16; 16]);
        assert_eq!(c.l4[4], [7; 16]);
        assert_eq!(c.l8[1], DEFAULT_8X8_INTER);
    }

    #[test]
    fn scaling_lists_go_to_va_in_raster_order() {
        // Table 7-3's matrix, row by row.
        assert_eq!(
            raster(&DEFAULT_4X4_INTRA, &ZIGZAG_4X4),
            [
                6, 13, 20, 28, 13, 20, 28, 32, 20, 28, 32, 37, 28, 32, 37, 42
            ]
        );
        let m = raster(&DEFAULT_8X8_INTER, &ZIGZAG_8X8);
        assert_eq!(m[..8], [9, 13, 15, 17, 19, 21, 22, 24]);
        assert_eq!(m[8..16], [13, 13, 17, 19, 21, 22, 24, 25]);
        assert_eq!(m[56..], [24, 25, 27, 28, 30, 32, 33, 35]);
        let mut seen = [false; 64];
        ZIGZAG_8X8.iter().for_each(|&p| seen[p] = true);
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn scaling_list_coding() {
        // sixteen deltas of 0 (se(0) = "1") keep nextScale at 8 → the list repeats 8; a first delta of -8 → default.
        let mut b = Bits(BitReader::new(&[0xff, 0xff]));
        assert_eq!(
            scaling_list(&mut b, 16).unwrap(),
            ListCode::List(vec![8; 16])
        );
        // se(-8) = ue(16) = 0000 1 0001 → 000010001.
        let mut b = Bits(BitReader::new(&[0b0000_1000, 0b1000_0000]));
        assert_eq!(scaling_list(&mut b, 16).unwrap(), ListCode::UseDefault);
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    fn refused(avcc: &str) -> String {
        match H264Stream::new(&hex(avcc), 4096, 4096) {
            Err(Reject::Unsupported(m)) => m,
            Err(Reject::Malformed(m)) => panic!("malformed: {m}"),
            Ok(_) => panic!("accepted"),
        }
    }

    /// avcC records of x264 clips: High 10 and High 4:2:2 are refused with the reason the
    /// hardware report gives; MBAFF and field-coded streams open, with the frame height in
    /// macroblocks (twice the map units) and the crop in field units.
    #[test]
    fn interlaced_and_refused_profiles() {
        let high10 = "016e001effe1001b676e001ea6cd940a02ff970110000003001000000303c0f162d96001000668ebe3cb22c0fdfafa00";
        assert!(refused(high10).contains("High 10"), "{}", refused(high10));
        let high422 = "017a001effe1001a677a001ebcd940a02ff89c0440000003004000000f03c58b658001000668ebe3cb22c0fef8f800";
        assert!(refused(high422).contains("4:2:2"), "{}", refused(high422));

        // 720×360 MBAFF: 45×24 macroblocks, 24 lines cropped at the bottom.
        let mbaff = "0164001effe1001a6764001eacd940b463f3e022000003000200000300783e28532c01000668fba3cb22c0fdf8f800";
        let st = H264Stream::new(&hex(mbaff), 4096, 4096).unwrap();
        let s = st.sps.iter().flatten().next().unwrap();
        assert!(!s.frame_mbs_only && s.mbaff);
        assert_eq!((s.width_mbs, s.height_mbs), (45, 24));
        assert_eq!(s.height_mbs * 16 - s.crop[2] - s.crop[3], 360);

        // A conformance stream of field pictures (PAFF without MBAFF), Main profile.
        let fields = "014d601fffe1000a274d601f8d8d30168f2401000628ce0815c800";
        let st = H264Stream::new(&hex(fields), 4096, 4096).unwrap();
        let s = st.sps.iter().flatten().next().unwrap();
        assert!(!s.frame_mbs_only && !s.mbaff);
        assert_eq!(s.height_mbs % 2, 0);
    }
}
