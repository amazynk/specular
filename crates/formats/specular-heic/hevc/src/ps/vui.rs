//! `vui_parameters` and `hrd_parameters` (Annex E).
//!
//! The decoder itself needs nothing from the VUI, but the SPS range extension
//! flags sit after it, so the syntax has to be walked exactly. The video
//! signal type is kept for the caller's colour management.

use crate::bits::BitReader;
use crate::error::Result;

/// Parses `hrd_parameters(commonInfPresentFlag, maxNumSubLayersMinus1)`.
fn hrd(r: &mut BitReader<'_>, common: bool, max_sub_minus1: u32) -> Result<()> {
    let mut nal_hrd = false;
    let mut vcl_hrd = false;
    let mut sub_pic = false;
    if common {
        nal_hrd = r.u1()? != 0;
        vcl_hrd = r.u1()? != 0;
        if nal_hrd || vcl_hrd {
            sub_pic = r.u1()? != 0;
            if sub_pic {
                r.skip(8 + 5 + 1 + 5)?;
            }
            r.skip(4 + 4)?; // bit_rate_scale, cpb_size_scale
            if sub_pic {
                r.skip(4)?; // cpb_size_du_scale
            }
            r.skip(5 + 5 + 5)?;
        }
    }
    for _ in 0..=max_sub_minus1 {
        let fixed_general = r.u1()? != 0;
        let fixed_within = if fixed_general { true } else { r.u1()? != 0 };
        let mut low_delay = false;
        if fixed_within {
            r.ue()?; // elemental_duration_in_tc_minus1
        } else {
            low_delay = r.u1()? != 0;
        }
        let cpb_cnt = if low_delay { 0 } else { r.ue()? };
        let sets = usize::from(nal_hrd) + usize::from(vcl_hrd);
        for _ in 0..sets {
            for _ in 0..=cpb_cnt {
                r.ue()?; // bit_rate_value_minus1
                r.ue()?; // cpb_size_value_minus1
                if sub_pic {
                    r.ue()?; // cpb_size_du_value_minus1
                    r.ue()?; // bit_rate_du_value_minus1
                }
                r.u1()?; // cbr_flag
            }
        }
    }
    Ok(())
}

/// The video signal type of the VUI (Annex E.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Signal {
    /// `video_full_range_flag`.
    pub full_range: bool,
    /// `colour_primaries`, `transfer_characteristics` and
    /// `matrix_coeffs`, when `colour_description_present_flag` is set.
    pub colour: Option<(u8, u8, u8)>,
}

/// Parses `vui_parameters()`, keeping only the video signal type, when
/// present.
/// Parses `vui_parameters()`: the video signal type, `field_seq_flag` (pictures are
/// fields) and `chroma_sample_loc_type_top_field` (where subsampled chroma sits).
pub fn parse(
    r: &mut BitReader<'_>,
    max_sub_minus1: u32,
) -> Result<(Option<Signal>, bool, Option<u8>)> {
    let mut signal = None;
    if r.u1()? != 0 {
        // aspect_ratio_info_present_flag
        let idc = r.u(8)?;
        if idc == 255 {
            r.skip(32)?;
        }
    }
    if r.u1()? != 0 {
        r.skip(1)?; // overscan_appropriate_flag
    }
    if r.u1()? != 0 {
        // video_signal_type_present_flag
        r.skip(3)?; // video_format
        let full_range = r.u1()? != 0;
        let colour = if r.u1()? != 0 {
            let p = r.u(8)? as u8;
            let t = r.u(8)? as u8;
            let m = r.u(8)? as u8;
            Some((p, t, m))
        } else {
            None
        };
        signal = Some(Signal { full_range, colour });
    }
    let mut chroma_loc = None;
    if r.u1()? != 0 {
        // chroma_loc_info_present_flag: top and bottom field types, 0..=5.
        chroma_loc = Some(r.ue()?.min(5) as u8);
        r.ue()?;
    }
    r.skip(1)?; // neutral_chroma_indication_flag
    let field_seq = r.u1()? != 0;
    r.skip(1)?; // frame_field_info_present_flag
    if r.u1()? != 0 {
        // default_display_window_flag
        for _ in 0..4 {
            r.ue()?;
        }
    }
    if r.u1()? != 0 {
        // vui_timing_info_present_flag
        r.skip(64)?;
        if r.u1()? != 0 {
            r.ue()?; // vui_num_ticks_poc_diff_one_minus1
        }
        if r.u1()? != 0 {
            hrd(r, true, max_sub_minus1)?;
        }
    }
    if r.u1()? != 0 {
        // bitstream_restriction_flag
        r.skip(3)?;
        for _ in 0..5 {
            r.ue()?;
        }
    }
    Ok((signal, field_seq, chroma_loc))
}
