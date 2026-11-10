//! Rebuilding the HEVC video and sequence parameter sets BPG leaves out.
//!
//! A BPG stream carries a compact `hevc_header()` instead of the VPS and SPS. The fields it
//! has go into a standard SPS; the ones it leaves out take the values the specification
//! fixes (section 3.2: ids 0, one sub-layer, `log2_max_pic_order_cnt_lsb` 8, AMP and
//! temporal MVP on, no scaling lists, no short-term RPS sets in the SPS). The picture size
//! is rounded up to the minimum coding block, with no conformance window: the decoder
//! returns the whole coded picture and the caller crops it, as the reference decoder does
//! (a window cannot express an odd crop of subsampled chroma).
//!
//! The SPS declares the Format Range Extensions profile so its `sps_range_extension()`
//! (lossless and 4:4:4 tools) is read.

use crate::Error;

/// The fields of `hevc_header()`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CompactHeader {
    pub log2_min_cb_minus3: u32,
    pub log2_diff_max_min_cb: u32,
    pub log2_min_tb_minus2: u32,
    pub log2_diff_max_min_tb: u32,
    pub max_tr_depth_intra: u32,
    pub sao: bool,
    pub pcm: Option<Pcm>,
    pub strong_intra_smoothing: bool,
    /// `sps_range_extension_flag` and `sps_extension_7bits`, when the extension is present.
    pub extension: Option<(bool, u8)>,
    /// The nine `sps_range_extension()` flags in syntax order.
    pub range_flags: [bool; 9],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Pcm {
    pub bit_depth_luma_minus1: u8,
    pub bit_depth_chroma_minus1: u8,
    pub log2_min_minus3: u32,
    pub log2_diff_max_min: u32,
    pub loop_filter_disabled: bool,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn bit(&mut self) -> Result<bool, Error> {
        let b = self
            .data
            .get(self.pos / 8)
            .ok_or_else(|| Error::Truncated("BPG HEVC header".into()))?;
        let v = b >> (7 - self.pos % 8) & 1 != 0;
        self.pos += 1;
        Ok(v)
    }
    fn bits(&mut self, n: u32) -> Result<u32, Error> {
        let mut v = 0;
        for _ in 0..n {
            v = v << 1 | u32::from(self.bit()?);
        }
        Ok(v)
    }
    fn ue(&mut self) -> Result<u32, Error> {
        let mut zeros = 0;
        while !self.bit()? {
            zeros += 1;
            if zeros > 31 {
                return Err(Error::Malformed("BPG HEVC header".into()));
            }
        }
        let rest = self.bits(zeros)?;
        Ok(((1u64 << zeros) - 1 + u64::from(rest)).min(u64::from(u32::MAX)) as u32)
    }
}

/// Parses `hevc_header()` at `at`; returns it and the offset just past it.
pub(crate) fn parse_compact(data: &[u8], at: usize) -> Result<(CompactHeader, usize), Error> {
    let (len, n) = crate::header::ue7(data, at)?;
    let start = at + n;
    let end = start
        .checked_add(len as usize)
        .filter(|&e| e <= data.len())
        .ok_or_else(|| Error::Truncated("BPG HEVC header".into()))?;
    let r = &mut Reader {
        data: &data[start..end],
        pos: 0,
    };
    let mut h = CompactHeader {
        log2_min_cb_minus3: r.ue()?,
        log2_diff_max_min_cb: r.ue()?,
        log2_min_tb_minus2: r.ue()?,
        log2_diff_max_min_tb: r.ue()?,
        max_tr_depth_intra: r.ue()?,
        sao: r.bit()?,
        ..CompactHeader::default()
    };
    if h.log2_min_cb_minus3 > 3
        || h.log2_diff_max_min_cb > 3
        || h.log2_min_tb_minus2 > 3
        || h.log2_diff_max_min_tb > 3
        || h.max_tr_depth_intra > 4
    {
        return Err(Error::Malformed("BPG HEVC header block sizes".into()));
    }
    if r.bit()? {
        h.pcm = Some(Pcm {
            bit_depth_luma_minus1: r.bits(4)? as u8,
            bit_depth_chroma_minus1: r.bits(4)? as u8,
            log2_min_minus3: r.ue()?.min(8),
            log2_diff_max_min: r.ue()?.min(8),
            loop_filter_disabled: r.bit()?,
        });
    }
    h.strong_intra_smoothing = r.bit()?;
    if r.bit()? {
        let range = r.bit()?;
        let seven = r.bits(7)? as u8;
        h.extension = Some((range, seven));
        if range {
            for f in &mut h.range_flags {
                *f = r.bit()?;
            }
        }
    }
    Ok((h, end))
}

/// An RBSP bit writer.
#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
    bit: u32,
}

impl Writer {
    fn bit(&mut self, b: bool) {
        if self.bit == 0 {
            self.bytes.push(0);
        }
        if b {
            *self.bytes.last_mut().expect("a byte was pushed") |= 0x80 >> self.bit;
        }
        self.bit = (self.bit + 1) % 8;
    }
    fn bits(&mut self, n: u32, v: u32) {
        for i in (0..n).rev() {
            self.bit(v >> i & 1 != 0);
        }
    }
    fn ue(&mut self, v: u32) {
        let x = u64::from(v) + 1;
        let len = 64 - x.leading_zeros();
        for _ in 1..len {
            self.bit(false);
        }
        for i in (0..len).rev() {
            self.bit(x >> i & 1 != 0);
        }
    }
    /// `rbsp_trailing_bits()`, then the NAL unit with its header and emulation prevention.
    fn nal(mut self, nal_unit_type: u8) -> Vec<u8> {
        self.bit(true);
        while self.bit != 0 {
            self.bit(false);
        }
        let mut out = vec![nal_unit_type << 1, 1];
        let mut zeros = 0;
        for &b in &self.bytes {
            if zeros >= 2 && b <= 3 {
                out.push(3);
                zeros = 0;
            }
            out.push(b);
            zeros = if b == 0 { zeros + 1 } else { 0 };
        }
        out
    }
}

/// `profile_tier_level(1, 0)`: Format Range Extensions, level 6.2.
fn profile_tier_level(w: &mut Writer) {
    w.bits(2, 0); // general_profile_space
    w.bit(false); // general_tier_flag
    w.bits(5, 4); // general_profile_idc: format range extensions
    for j in 0..32 {
        w.bit(j == 4);
    }
    w.bit(true); // progressive_source
    w.bit(false); // interlaced_source
    w.bit(false); // non_packed_constraint
    w.bit(true); // frame_only_constraint
    w.bits(32, 0); // the 43 constraint and reserved bits
    w.bits(11, 0);
    w.bit(false); // general_inbld_flag
    w.bits(8, 186); // general_level_idc
}

/// The VPS NAL unit.
pub(crate) fn vps() -> Vec<u8> {
    let mut w = Writer::default();
    w.bits(4, 0); // vps_video_parameter_set_id
    w.bit(true); // vps_base_layer_internal_flag
    w.bit(true); // vps_base_layer_available_flag
    w.bits(6, 0); // vps_max_layers_minus1
    w.bits(3, 0); // vps_max_sub_layers_minus1
    w.bit(true); // vps_temporal_id_nesting_flag
    w.bits(16, 0xffff);
    profile_tier_level(&mut w);
    w.bit(true); // vps_sub_layer_ordering_info_present_flag
    w.ue(1); // vps_max_dec_pic_buffering_minus1
    w.ue(0); // vps_max_num_reorder_pics
    w.ue(0); // vps_max_latency_increase_plus1
    w.bits(6, 0); // vps_max_layer_id
    w.ue(0); // vps_num_layer_sets_minus1
    w.bit(false); // vps_timing_info_present_flag
    w.bit(false); // vps_extension_flag
    w.nal(32)
}

/// The SPS NAL unit for a picture of `width`×`height` (before rounding).
pub(crate) fn sps(
    h: &CompactHeader,
    width: u32,
    height: u32,
    chroma_format_idc: u32,
    bit_depth: u8,
) -> Vec<u8> {
    let cb = 1u32 << (h.log2_min_cb_minus3 + 3);
    let round = |v: u32| v.div_ceil(cb).saturating_mul(cb);
    let mut w = Writer::default();
    w.bits(4, 0); // sps_video_parameter_set_id
    w.bits(3, 0); // sps_max_sub_layers_minus1
    w.bit(true); // sps_temporal_id_nesting_flag
    profile_tier_level(&mut w);
    w.ue(0); // sps_seq_parameter_set_id
    w.ue(chroma_format_idc);
    if chroma_format_idc == 3 {
        w.bit(false); // separate_colour_plane_flag
    }
    w.ue(round(width));
    w.ue(round(height));
    w.bit(false); // conformance_window_flag
    w.ue(u32::from(bit_depth - 8));
    w.ue(u32::from(bit_depth - 8));
    w.ue(4); // log2_max_pic_order_cnt_lsb_minus4
    w.bit(true); // sps_sub_layer_ordering_info_present_flag
    w.ue(1); // sps_max_dec_pic_buffering_minus1: the picture and the one before
    w.ue(0); // sps_max_num_reorder_pics
    w.ue(0); // sps_max_latency_increase_plus1
    w.ue(h.log2_min_cb_minus3);
    w.ue(h.log2_diff_max_min_cb);
    w.ue(h.log2_min_tb_minus2);
    w.ue(h.log2_diff_max_min_tb);
    w.ue(h.max_tr_depth_intra); // max_transform_hierarchy_depth_inter
    w.ue(h.max_tr_depth_intra);
    w.bit(false); // scaling_list_enabled_flag
    w.bit(true); // amp_enabled_flag
    w.bit(h.sao);
    w.bit(h.pcm.is_some());
    if let Some(p) = &h.pcm {
        w.bits(4, u32::from(p.bit_depth_luma_minus1));
        w.bits(4, u32::from(p.bit_depth_chroma_minus1));
        w.ue(p.log2_min_minus3);
        w.ue(p.log2_diff_max_min);
        w.bit(p.loop_filter_disabled);
    }
    w.ue(0); // num_short_term_ref_pic_sets
    w.bit(false); // long_term_ref_pics_present_flag
    w.bit(true); // sps_temporal_mvp_enabled_flag
    w.bit(h.strong_intra_smoothing);
    w.bit(false); // vui_parameters_present_flag
    w.bit(h.extension.is_some());
    if let Some((range, seven)) = h.extension {
        w.bit(range);
        w.bits(7, u32::from(seven));
        if range {
            for &f in &h.range_flags {
                w.bit(f);
            }
        }
    }
    w.nal(33)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_golomb_and_emulation_prevention() {
        let mut w = Writer::default();
        for v in [0, 1, 2, 3, 7, 254] {
            w.ue(v);
        }
        let r = &mut Reader {
            data: &w.bytes,
            pos: 0,
        };
        for v in [0, 1, 2, 3, 7, 254] {
            assert_eq!(r.ue().unwrap(), v);
        }
        let mut w = Writer::default();
        w.bits(24, 0);
        w.bits(8, 1);
        let nal = w.nal(33);
        assert_eq!(nal, [66, 1, 0, 0, 3, 0, 1, 0x80]);
    }

    #[test]
    fn parameter_sets_parse_in_the_hevc_decoder() {
        let h = CompactHeader {
            log2_min_cb_minus3: 0,
            log2_diff_max_min_cb: 1,
            log2_min_tb_minus2: 0,
            log2_diff_max_min_tb: 3,
            max_tr_depth_intra: 3,
            sao: true,
            extension: Some((true, 0)),
            range_flags: [true, true, true, false, false, false, false, false, false],
            ..CompactHeader::default()
        };
        let v = vps();
        let s = sps(&h, 333, 211, 1, 10);
        // A picture without slices: the parameter sets parse, then the slices are missing.
        let e = hevc::decode_still(&[&v, &s], &[]).unwrap_err();
        assert!(matches!(e, hevc::Error::NoSlices), "{e:?}");
    }
}
