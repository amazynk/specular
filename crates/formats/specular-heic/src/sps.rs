//! Container-boundary checks of a coded item's SPS before any decoder
//! core sees it: a small, core-independent reader of the SPS head
//! (H.265 §7.3.2.2.1 up to the coding-block sizes) and the size /
//! limit checks against the item's `ispe`.
//!
//! A damaged or hostile SPS can describe a picture far larger than the
//! tile it claims to be (memory), or a geometry the decoding process
//! cannot tile (a width that is no multiple of MinCbSizeY made the
//! upstream CTU walk spin forever). Both are rejected here, for every
//! core.

use heif::HevcConfig;

/// The SPS fields the boundary checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpsHead {
    /// `chroma_format_idc`.
    pub chroma_format_idc: u32,
    /// `pic_width_in_luma_samples`.
    pub width: u32,
    /// `pic_height_in_luma_samples`.
    pub height: u32,
    /// Luma size after the conformance window.
    pub cropped: (u32, u32),
    /// `BitDepthY`.
    pub bit_depth_luma: u32,
    /// `BitDepthC`.
    pub bit_depth_chroma: u32,
    /// `MinCbLog2SizeY`.
    pub min_cb_log2: u32,
    /// `CtbLog2SizeY`.
    pub ctb_log2: u32,
}

struct Bits<'a> {
    b: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn u(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = *self.b.get(self.pos / 8)?;
            v = (v << 1) | u32::from((byte >> (7 - self.pos % 8)) & 1);
            self.pos += 1;
        }
        Some(v)
    }

    fn skip(&mut self, n: usize) -> Option<()> {
        self.pos += n;
        (self.pos <= self.b.len() * 8).then_some(())
    }

    fn ue(&mut self) -> Option<u32> {
        let mut zeros = 0;
        while self.u(1)? == 0 {
            zeros += 1;
            if zeros > 31 {
                return None;
            }
        }
        let rest = self.u(zeros)?;
        Some(((1u64 << zeros) - 1 + u64::from(rest)).min(u64::from(u32::MAX)) as u32)
    }
}

/// Emulation-prevention bytes removed (`00 00 03` → `00 00`).
fn rbsp(nal: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(nal.len());
    let mut zeros = 0;
    for &b in nal {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            continue;
        }
        zeros = if b == 0 { zeros + 1 } else { 0 };
        out.push(b);
    }
    out
}

fn profile_tier_level(r: &mut Bits<'_>, max_sub_layers_minus1: u32) -> Option<()> {
    // general_profile_space .. general_level_idc: 96 bits.
    r.skip(96)?;
    let mut flags = Vec::with_capacity(max_sub_layers_minus1 as usize);
    for _ in 0..max_sub_layers_minus1 {
        flags.push((r.u(1)?, r.u(1)?));
    }
    if max_sub_layers_minus1 > 0 {
        r.skip(2 * (8 - max_sub_layers_minus1 as usize))?;
    }
    for (profile, level) in flags {
        r.skip(88 * profile as usize + 8 * level as usize)?;
    }
    Some(())
}

/// Parse the head of an SPS NAL unit (two-byte header included).
/// `None` when it is truncated, not a base-layer SPS, or out of the
/// value ranges of §7.4.3.2.1.
pub fn parse_head(nal: &[u8]) -> Option<SpsHead> {
    if nal.len() < 3 || (nal[0] >> 1) & 0x3f != 33 || ((nal[0] & 1) << 5 | nal[1] >> 3) != 0 {
        return None;
    }
    let data = rbsp(&nal[2..]);
    let r = &mut Bits { b: &data, pos: 0 };
    r.u(4)?; // sps_video_parameter_set_id
    let max_sub_layers_minus1 = r.u(3)?;
    if max_sub_layers_minus1 > 6 {
        return None;
    }
    r.u(1)?; // sps_temporal_id_nesting_flag
    profile_tier_level(r, max_sub_layers_minus1)?;
    if r.ue()? > 15 {
        return None; // sps_seq_parameter_set_id
    }
    let chroma_format_idc = r.ue()?;
    if chroma_format_idc > 3 {
        return None;
    }
    let separate_planes = chroma_format_idc == 3 && r.u(1)? == 1;
    let width = r.ue()?;
    let height = r.ue()?;
    let (mut cl, mut cr, mut ct, mut cb) = (0, 0, 0, 0);
    if r.u(1)? == 1 {
        (cl, cr, ct, cb) = (r.ue()?, r.ue()?, r.ue()?, r.ue()?);
    }
    let bit_depth_luma = r.ue()?.checked_add(8)?;
    let bit_depth_chroma = r.ue()?.checked_add(8)?;
    if bit_depth_luma > 16 || bit_depth_chroma > 16 {
        return None;
    }
    if r.ue()? > 12 {
        return None; // log2_max_pic_order_cnt_lsb_minus4
    }
    let all = r.u(1)? == 1;
    for _ in if all { 0 } else { max_sub_layers_minus1 }..=max_sub_layers_minus1 {
        r.ue()?;
        r.ue()?;
        r.ue()?;
    }
    let min_cb_log2 = r.ue()?.checked_add(3)?;
    let ctb_log2 = min_cb_log2.checked_add(r.ue()?)?;
    if ctb_log2 > 6 {
        return None;
    }
    let (sub_w, sub_h) = match (chroma_format_idc, separate_planes) {
        (1, _) => (2, 2),
        (2, _) => (2, 1),
        _ => (1, 1),
    };
    let crop_w = (u64::from(cl) + u64::from(cr)) * sub_w;
    let crop_h = (u64::from(ct) + u64::from(cb)) * sub_h;
    if crop_w >= u64::from(width) || crop_h >= u64::from(height) {
        return None;
    }
    Some(SpsHead {
        chroma_format_idc,
        width,
        height,
        cropped: (width - crop_w as u32, height - crop_h as u32),
        bit_depth_luma,
        bit_depth_chroma,
        min_cb_log2,
        ctb_log2,
    })
}

fn nal_type(n: &[u8]) -> u8 {
    (n[0] >> 1) & 0x3f
}

/// The SPS VUI `chroma_sample_loc_type_top_field` (H.273 chroma sample location, 0..=5) of
/// the item's `hvcC`, when signalled.
pub fn chroma_loc(cfg: &HevcConfig) -> Option<u8> {
    let units: Vec<&[u8]> = cfg
        .arrays
        .iter()
        .flat_map(|a| a.nal_units.iter().map(Vec::as_slice))
        .collect();
    hevc::probe(&units).ok()?.chroma_loc
}

/// Base-layer NAL units of an item: the `hvcC` arrays, then the
/// length-prefixed payload. Framed as leniently as the most lenient core
/// (heic-rs: any length size 1–4, an overrunning last unit kept as the
/// rest of the payload), so that no parameter set a core will use
/// escapes the checks.
fn base_units<'a>(cfg: &'a HevcConfig, data: &'a [u8]) -> Vec<&'a [u8]> {
    let mut out: Vec<&[u8]> = cfg
        .arrays
        .iter()
        .flat_map(|a| a.nal_units.iter().map(Vec::as_slice))
        .collect();
    let ls = cfg.length_size as usize;
    if (1..=4).contains(&ls) {
        let mut pos = 0usize;
        while pos + ls <= data.len() {
            let len = data[pos..pos + ls]
                .iter()
                .fold(0usize, |a, &b| (a << 8) | b as usize);
            pos += ls;
            let Some(nal) = pos.checked_add(len).and_then(|e| data.get(pos..e)) else {
                if data.len() - pos >= 3 {
                    out.push(&data[pos..]);
                }
                break;
            };
            pos += len;
            out.push(nal);
        }
    }
    out.retain(|n| n.len() >= 2 && ((n[0] & 1) << 5 | n[1] >> 3) == 0);
    out
}

/// `(dependent_slice_segments_enabled_flag, num_extra_slice_header_bits)`
/// of the PPS with id `pps_id` (H.265 §7.3.2.3.1 head).
fn pps_head(units: &[&[u8]], pps_id: u32) -> Option<(bool, u32)> {
    units.iter().filter(|n| nal_type(n) == 34).find_map(|n| {
        let data = rbsp(&n[2..]);
        let r = &mut Bits { b: &data, pos: 0 };
        if r.ue()? != pps_id {
            return None;
        }
        r.ue()?; // pps_seq_parameter_set_id
        let dependent = r.u(1)? == 1;
        r.u(1)?; // output_flag_present_flag
        Some((dependent, r.u(3)?))
    })
}

/// `slice_type` of an independent slice segment; `None` for a
/// dependent one or when unreadable (the core reports that).
fn slice_type(nal: &[u8], units: &[&[u8]], sps: &SpsHead) -> Option<u32> {
    let data = rbsp(&nal[2..]);
    let r = &mut Bits { b: &data, pos: 0 };
    let first = r.u(1)? == 1;
    if (16..=23).contains(&nal_type(nal)) {
        r.u(1)?; // no_output_of_prior_pics_flag
    }
    let (dependent_enabled, extra_bits) = pps_head(units, r.ue()?)?;
    if !first {
        if dependent_enabled && r.u(1)? == 1 {
            return None;
        }
        let ctb = 1u64 << sps.ctb_log2;
        let ctbs = u64::from(sps.width).div_ceil(ctb) * u64::from(sps.height).div_ceil(ctb);
        let bits = 64 - (ctbs.max(1) - 1).leading_zeros();
        r.skip(bits as usize)?;
    }
    r.skip(extra_bits as usize)?;
    r.ue()
}

/// Check a coded item's SPS against its `ispe` and the decoder limits
/// before decoding, and that it is intra-only: an item is decoded on its
/// own, without reference pictures, so a P / B slice can never decode
/// correctly (and drives a core through paths no still image needs).
/// `Err` carries the reason. Items without a findable SPS pass (the core
/// reports the missing parameter set).
pub fn check(
    cfg: &HevcConfig,
    data: &[u8],
    ispe: Option<(u32, u32)>,
    max_pixels: u64,
) -> Result<u64, String> {
    let units = base_units(cfg, data);
    // Every SPS, not only the first: a core keeps the last one with a
    // given id, so a later one in the payload would otherwise slip a
    // picture of any size past the limits.
    let mut first = None;
    let mut bytes = 0u64;
    for sps in units.iter().filter(|n| nal_type(n) == 33) {
        let s = parse_head(sps).ok_or("malformed sequence parameter set")?;
        check_sps(&s, ispe, max_pixels)?;
        bytes = bytes.max(picture_bytes(&s));
        first.get_or_insert(s);
    }
    let Some(s) = first else {
        return Ok(0);
    };
    for n in units.iter().filter(|n| nal_type(n) <= 21) {
        if let Some(t) = slice_type(n, &units, &s)
            && t != 2
        {
            return Err(format!(
                "inter-coded slice (slice_type {t}) in an image item without reference pictures"
            ));
        }
    }
    Ok(bytes)
}

/// The bytes of one decoded picture of `s` (its planes at their sample size).
fn picture_bytes(s: &SpsHead) -> u64 {
    let luma = u64::from(s.width) * u64::from(s.height);
    let chroma = match s.chroma_format_idc {
        0 => 0,
        1 => luma / 2,
        2 => luma,
        _ => 2 * luma,
    };
    let sample = |bits: u32| if bits > 8 { 2 } else { 1 };
    luma * sample(s.bit_depth_luma) + chroma * sample(s.bit_depth_chroma)
}

/// The size checks of [`check`] for one SPS.
fn check_sps(s: &SpsHead, ispe: Option<(u32, u32)>, max_pixels: u64) -> Result<(), String> {
    let mask = (1u32 << s.min_cb_log2) - 1;
    if s.width == 0 || s.height == 0 || s.width & mask != 0 || s.height & mask != 0 {
        return Err(format!(
            "SPS picture {}x{} is not a multiple of the {}-sample minimum coding block",
            s.width,
            s.height,
            1 << s.min_cb_log2
        ));
    }
    let pixels = u64::from(s.width) * u64::from(s.height);
    if pixels > max_pixels {
        return Err(format!("SPS picture {}x{} too large", s.width, s.height));
    }
    if let Some((iw, ih)) = ispe {
        let (cw, ch) = s.cropped;
        if cw < iw || ch < ih {
            return Err(format!(
                "ispe {iw}x{ih} exceeds the coded {cw}x{ch} picture"
            ));
        }
        // An encoder pads to whole CTBs (and some to a coarser grid);
        // a coded picture many times the declared item is damage.
        let slack = |i: u32| u64::from(i) * 2 + 256;
        if u64::from(s.width) > slack(iw) || u64::from(s.height) > slack(ih) {
            return Err(format!(
                "coded {}x{} picture far larger than the ispe {iw}x{ih}",
                s.width, s.height
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bit writer for synthetic SPS heads.
    struct W(Vec<bool>);
    impl W {
        fn u(&mut self, n: u32, v: u32) {
            for i in (0..n).rev() {
                self.0.push((v >> i) & 1 == 1);
            }
        }
        fn ue(&mut self, v: u32) {
            let x = v + 1;
            let n = 32 - x.leading_zeros();
            self.u(n - 1, 0);
            self.u(n, x);
        }
        fn bytes(&self) -> Vec<u8> {
            let mut out = vec![0x42, 0x01];
            for c in self.0.chunks(8) {
                let mut b = 0u8;
                for (i, &bit) in c.iter().enumerate() {
                    b |= (bit as u8) << (7 - i);
                }
                out.push(b);
            }
            out.push(0x80);
            out
        }
    }

    fn sps(w: u32, h: u32, min_cb_minus3: u32, window: Option<[u32; 4]>) -> Vec<u8> {
        let mut b = W(Vec::new());
        b.u(4, 0);
        b.u(3, 0);
        b.u(1, 1);
        b.u(32, 0x0160_0000);
        b.u(32, 0x9000_0000);
        b.u(32, 0x0000_005a); // ... general_level_idc 90
        b.ue(0);
        b.ue(1); // 4:2:0
        b.ue(w);
        b.ue(h);
        match window {
            Some(c) => {
                b.u(1, 1);
                c.iter().for_each(|&v| b.ue(v));
            }
            None => b.u(1, 0),
        }
        b.ue(0);
        b.ue(0);
        b.ue(4);
        b.u(1, 1);
        b.ue(0);
        b.ue(0);
        b.ue(0);
        b.ue(min_cb_minus3);
        b.ue(3 - min_cb_minus3);
        b.bytes()
    }

    #[test]
    fn head() {
        let s = parse_head(&sps(1440, 960, 0, Some([0, 4, 0, 2]))).unwrap();
        assert_eq!((s.width, s.height, s.cropped), (1440, 960, (1432, 956)));
        assert_eq!((s.min_cb_log2, s.ctb_log2, s.bit_depth_luma), (3, 6, 8));
    }

    #[test]
    fn rejects() {
        let cfg = |nal: Vec<u8>| {
            let mut c = HevcConfig::parse(&hvcc_with(&nal)).unwrap();
            c.length_size = 4;
            c
        };
        let ok = cfg(sps(512, 512, 0, None));
        assert!(check(&ok, &[], Some((512, 512)), 1 << 30).is_ok());
        assert!(check(&ok, &[], Some((513, 512)), 1 << 30).is_err());
        assert!(check(&ok, &[], Some((64, 64)), 1 << 30).is_err());
        assert!(check(&ok, &[], None, 1 << 10).is_err());
        // The fuzz hang: width not a multiple of MinCbSizeY.
        let odd = cfg(sps(1444, 960, 0, None));
        assert!(
            check(&odd, &[], None, 1 << 30)
                .unwrap_err()
                .contains("minimum coding block")
        );
        assert!(parse_head(&sps(512, 512, 0, Some([300, 0, 0, 0]))).is_none());
    }

    /// A later SPS in the payload replaces the hvcC one in the core, so
    /// it must face the same limits, however the payload is framed.
    #[test]
    fn hostile_payload_sps() {
        let small = sps(512, 512, 0, None);
        let huge = sps(65528, 65528, 0, None);
        let with_ls = |ls: u8| {
            let mut c = HevcConfig::parse(&hvcc_with(&small)).unwrap();
            c.length_size = ls;
            c
        };
        let framed = |ls: usize, nal: &[u8]| {
            let mut d = nal.len().to_be_bytes()[8 - ls..].to_vec();
            d.extend(nal);
            d
        };
        for ls in 2..=4u8 {
            let c = with_ls(ls);
            assert!(check(&c, &framed(ls as usize, &small), Some((512, 512)), 1 << 30).is_ok());
            assert!(check(&c, &framed(ls as usize, &huge), Some((512, 512)), 1 << 30).is_err());
            assert!(check(&c, &framed(ls as usize, &huge), None, 1 << 30).is_err());
        }
        // An overrunning last unit is still a unit to the lenient core.
        let mut cut = vec![0, 0, 0xff, 0xff];
        cut.extend(&huge);
        assert!(check(&with_ls(4), &cut, None, 1 << 30).is_err());
    }

    /// A header-only item may declare a picture of up to MAX_PIXELS: `check` says how many
    /// bytes it decodes to, which the decode reserves from the budget first.
    #[test]
    fn picture_bytes_for_the_budget() {
        let big = sps(16384, 16384, 0, None);
        let c = HevcConfig::parse(&hvcc_with(&big)).unwrap();
        // 4:2:0, 8-bit: 1.5 bytes a pixel.
        assert_eq!(check(&c, &[], None, 1 << 30), Ok(16384 * 16384 * 3 / 2));
        let r = specular_core::budget::with_limit(64 << 20, || {
            specular_core::budget::reserve(2 * 16384 * 16384 * 3 / 2)
        });
        assert!(r.is_err());
    }

    fn hvcc_with(sps: &[u8]) -> Vec<u8> {
        let mut r = vec![
            1, 0x01, 0x60, 0, 0, 0, 0x90, 0, 0, 0, 0, 0, 90, 0xf0, 0, 0xfc, 0xfd, 0xf8, 0xf8, 0, 0,
            0x0f, 1,
        ];
        r.extend([0x80 | 33, 0, 1]);
        r.extend((sps.len() as u16).to_be_bytes());
        r.extend(sps);
        r
    }
}
