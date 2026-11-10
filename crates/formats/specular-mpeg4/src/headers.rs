//! Visual object sequence, visual object, video object layer and VOP headers (ISO/IEC
//! 14496-2 §6.2.1–§6.2.5), and the H.263 picture header of the short header mode (§6.2.5.2,
//! ITU-T H.263 §5.1).

use specular_mpeg2::util::Bits;

trait CloneSkip {
    fn clone_skip(&self, n: u32) -> Self;
}

impl CloneSkip for Bits<'_> {
    fn clone_skip(&self, n: u32) -> Self {
        let mut b = self.clone();
        b.skip(n);
        b
    }
}

use crate::tables::{DEFAULT_INTER, DEFAULT_INTRA, ZIGZAG};
use crate::{Error, malformed, unsupported};

pub(crate) const VOS: u8 = 0xB0;
pub(crate) const USER_DATA: u8 = 0xB2;
pub(crate) const GOV: u8 = 0xB3;
pub(crate) const VISUAL_OBJECT: u8 = 0xB5;
pub(crate) const VOP: u8 = 0xB6;

/// The video object layer: what every VOP of the stream shares.
#[derive(Clone, Debug, PartialEq)]
pub struct Vol {
    pub width: u32,
    pub height: u32,
    /// H.263 baseline pictures (short header).
    pub short_header: bool,
    /// Sorenson Spark (FLV1): the version of its picture header (0: H.263 escapes, 1: the
    /// 7/11-bit escape levels); H.263 short header pictures without GOBs.
    pub sorenson: Option<u8>,
    /// profile_and_level_indication of the visual object sequence (0 when absent).
    pub profile_level: u8,
    pub object_type: u8,
    pub verid: u8,
    /// Pixel aspect (horizontal : vertical spacing), when not square.
    pub pixel_aspect: Option<(u32, u32)>,
    pub low_delay: bool,
    pub time_resolution: u32,
    pub(crate) time_bits: u32,
    pub fixed_increment: Option<u32>,
    pub interlaced: bool,
    /// sprite_enable: 0 none, 1 static, 2 GMC.
    pub sprite: u8,
    pub warping_points: u8,
    pub(crate) warping_accuracy: u8,
    pub(crate) brightness_change: bool,
    /// MPEG quantisation (with matrices); else H.263.
    pub mpeg_quant: bool,
    pub(crate) intra_matrix: [u8; 64],
    pub(crate) inter_matrix: [u8; 64],
    pub quarter_sample: bool,
    pub(crate) resync_disable: bool,
    pub data_partitioned: bool,
    pub(crate) reversible_vlc: bool,
    pub(crate) reduced_resolution: bool,
    /// Colour description of the visual object: primaries, transfer, matrix.
    pub colour: Option<(u8, u8, u8)>,
    pub full_range: bool,
    /// Why this decoder won't decode the layer (shapes, scalability, more bits...).
    pub(crate) refusal: Option<String>,
}

impl Vol {
    /// Macroblocks across and down.
    pub fn mb_size(&self) -> (usize, usize) {
        (
            self.width.div_ceil(16) as usize,
            self.height.div_ceil(16) as usize,
        )
    }

    /// "Advanced Simple Profile @ Level 5", "Simple Profile @ Level 3", from the visual
    /// object sequence; `None` when the stream doesn't say.
    pub fn profile_name(&self) -> Option<String> {
        if let Some(v) = self.sorenson {
            return Some(format!("Sorenson Spark, version {v}"));
        }
        if self.short_header {
            return Some("H.263 baseline".into());
        }
        let pl = self.profile_level;
        let (profile, level) = match pl {
            0x01..=0x03 => ("Simple Profile", format!("Level {pl}")),
            0x08 => ("Simple Profile", "Level 0".into()),
            0x09 => ("Simple Profile", "Level 0b".into()),
            0x04 => ("Simple Profile", "Level 4a".into()),
            0x05 => ("Simple Profile", "Level 5".into()),
            0x06 => ("Simple Profile", "Level 6".into()),
            0xF0..=0xF5 => ("Advanced Simple Profile", format!("Level {}", pl - 0xF0)),
            0xF7 => ("Advanced Simple Profile", "Level 3b".into()),
            0x11..=0x12 => ("Simple Scalable Profile", format!("Level {}", pl - 0x10)),
            0x21..=0x22 => ("Core Profile", format!("Level {}", pl - 0x20)),
            0x32..=0x34 => ("Main Profile", format!("Level {}", pl - 0x30)),
            0x91..=0x94 => (
                "Advanced Real Time Simple Profile",
                format!("Level {}", pl - 0x90),
            ),
            0xB1..=0xB4 => (
                "Advanced Coding Efficiency Profile",
                format!("Level {}", pl - 0xB0),
            ),
            _ => {
                return match self.object_type {
                    1 => Some("Simple Profile".into()),
                    17 => Some("Advanced Simple Profile".into()),
                    _ => None,
                };
            }
        };
        Some(format!("{profile} @ {level}"))
    }

    /// Why the stream can't be decoded, in words.
    pub fn refusal(&self) -> Option<&str> {
        self.refusal.as_deref()
    }

    fn base() -> Vol {
        Vol {
            width: 0,
            height: 0,
            short_header: false,
            sorenson: None,
            profile_level: 0,
            object_type: 0,
            verid: 1,
            pixel_aspect: None,
            low_delay: false,
            time_resolution: 30,
            time_bits: 5,
            fixed_increment: None,
            interlaced: false,
            sprite: 0,
            warping_points: 0,
            warping_accuracy: 0,
            brightness_change: false,
            mpeg_quant: false,
            intra_matrix: DEFAULT_INTRA,
            inter_matrix: DEFAULT_INTER,
            quarter_sample: false,
            resync_disable: true,
            data_partitioned: false,
            reversible_vlc: false,
            reduced_resolution: false,
            colour: None,
            full_range: false,
            refusal: None,
        }
    }
}

/// What the visual object header said (applies to the layer that follows).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct VisualObject {
    pub colour: Option<(u8, u8, u8)>,
    pub full_range: bool,
}

pub(crate) fn visual_object(data: &[u8]) -> VisualObject {
    let mut r = Bits::new(data);
    let mut out = VisualObject::default();
    if r.flag() {
        r.skip(7);
    }
    if r.u(4) != 1 {
        return out;
    }
    if r.flag() {
        let _format = r.u(3);
        out.full_range = r.flag();
        if r.flag() {
            out.colour = Some((r.u(8) as u8, r.u(8) as u8, r.u(8) as u8));
        }
    }
    out
}

fn marker(r: &mut Bits) -> Result<(), Error> {
    if r.flag() {
        Ok(())
    } else {
        Err(malformed("missing marker bit"))
    }
}

/// A quantiser matrix: up to 64 values in zigzag order, a zero ending it early (the last value
/// repeats).
fn read_matrix(r: &mut Bits, m: &mut [u8; 64]) -> Result<(), Error> {
    let mut last = 0u8;
    for (i, &z) in ZIGZAG.iter().enumerate() {
        let v = r.u(8) as u8;
        if v == 0 {
            if i == 0 {
                return Err(malformed("empty quantiser matrix"));
            }
            for &z in &ZIGZAG[i..] {
                m[usize::from(z)] = last;
            }
            return Ok(());
        }
        m[usize::from(z)] = v;
        last = v;
    }
    Ok(())
}

/// The video object layer header (after its start code).
/// The largest picture read (4096 × 4096).
const MAX_PIXELS: u64 = 4096 * 4096;

pub(crate) fn vol(data: &[u8], vo: VisualObject, profile_level: u8) -> Result<Vol, Error> {
    let mut r = Bits::new(data);
    let mut v = Vol::base();
    v.profile_level = profile_level;
    v.colour = vo.colour;
    v.full_range = vo.full_range;
    let refuse = |v: &mut Vol, why: &str| {
        v.refusal.get_or_insert_with(|| why.to_owned());
    };
    // The Studio Profiles have a layer syntax of their own.
    if (0xE1..=0xE8).contains(&profile_level) {
        refuse(&mut v, "MPEG-4 Studio Profile");
        return Ok(v);
    }
    let _random_accessible = r.flag();
    v.object_type = r.u(8) as u8;
    if matches!(v.object_type, 14 | 15) {
        refuse(&mut v, "MPEG-4 Studio Profile");
        return Ok(v);
    }
    if r.flag() {
        v.verid = r.u(4) as u8;
        r.skip(3);
    }
    let aspect = r.u(4);
    v.pixel_aspect = match aspect {
        2 => Some((12, 11)),
        3 => Some((10, 11)),
        4 => Some((16, 11)),
        5 => Some((40, 33)),
        15 => {
            let (w, h) = (r.u(8), r.u(8));
            (w > 0 && h > 0 && w != h).then_some((w, h))
        }
        _ => None,
    };
    if r.flag() {
        let chroma = r.u(2);
        if chroma != 1 {
            refuse(&mut v, "MPEG-4 video in other than 4:2:0");
        }
        v.low_delay = r.flag();
        if r.flag() {
            // VBV parameters: 15+15+15+3+11+15 bits with markers.
            r.skip(15);
            r.skip(1);
            r.skip(15);
            r.skip(1);
            r.skip(15);
            r.skip(1);
            r.skip(3);
            r.skip(11);
            r.skip(1);
            r.skip(15);
            r.skip(1);
        }
    }
    let shape = r.u(2);
    if shape != 0 {
        refuse(&mut v, "MPEG-4 video objects with shapes");
        return Ok(v);
    }
    marker(&mut r)?;
    v.time_resolution = r.u(16).max(1);
    marker(&mut r)?;
    v.time_bits = (32 - (v.time_resolution - 1).leading_zeros()).max(1);
    if r.flag() {
        v.fixed_increment = Some(r.u(v.time_bits));
    }
    marker(&mut r)?;
    v.width = r.u(13);
    marker(&mut r)?;
    v.height = r.u(13);
    marker(&mut r)?;
    if v.width == 0 || v.height == 0 {
        return Err(malformed("zero picture size"));
    }
    // MPEG-4 Part 2 video stops at 4K; the 13-bit fields allow 8191², which costs about
    // 100 MB a frame (and several are live) from a header alone.
    if u64::from(v.width) * u64::from(v.height) > MAX_PIXELS {
        return Err(Error::Unsupported(format!(
            "{}×{} pictures",
            v.width, v.height
        )));
    }
    v.interlaced = r.flag();
    let _obmc_disable = r.flag();
    v.sprite = if v.verid == 1 {
        r.u(1) as u8
    } else {
        r.u(2) as u8
    };
    if v.sprite == 1 || v.sprite == 2 {
        if v.sprite != 2 {
            r.skip(13);
            marker(&mut r)?;
            r.skip(13);
            marker(&mut r)?;
            r.skip(13);
            marker(&mut r)?;
            r.skip(13);
            marker(&mut r)?;
        }
        v.warping_points = r.u(6) as u8;
        v.warping_accuracy = r.u(2) as u8;
        v.brightness_change = r.flag();
        if v.sprite != 2 {
            let _low_latency = r.flag();
        }
        if v.sprite == 1 {
            refuse(&mut v, "MPEG-4 static sprites");
        }
        if v.warping_points > 3 {
            refuse(
                &mut v,
                "MPEG-4 global motion with more than three warping points",
            );
        }
        if v.brightness_change {
            refuse(&mut v, "MPEG-4 global motion with brightness change");
        }
    } else if v.sprite == 3 {
        return Err(malformed("reserved sprite_enable"));
    }
    if r.flag() {
        let _precision = r.u(4);
        let bits = r.u(4);
        if bits != 8 {
            refuse(&mut v, "MPEG-4 video deeper than 8 bits");
        }
    }
    v.mpeg_quant = r.flag();
    if v.mpeg_quant {
        if r.flag() {
            read_matrix(&mut r, &mut v.intra_matrix)?;
        }
        if r.flag() {
            read_matrix(&mut r, &mut v.inter_matrix)?;
        }
    }
    if v.verid != 1 {
        v.quarter_sample = r.flag();
    }
    if !r.flag() {
        refuse(&mut v, "MPEG-4 complexity estimation headers");
        return Ok(v);
    }
    v.resync_disable = r.flag();
    v.data_partitioned = r.flag();
    if v.data_partitioned {
        v.reversible_vlc = r.flag();
        if v.reversible_vlc {
            refuse(&mut v, "MPEG-4 reversible VLC (RVLC)");
        } else if v.interlaced {
            refuse(&mut v, "interlaced MPEG-4 with data partitioning");
        }
    }
    if v.verid != 1 {
        if r.flag() {
            refuse(&mut v, "MPEG-4 NEWPRED");
            return Ok(v);
        }
        v.reduced_resolution = r.flag();
    }
    if r.flag() {
        refuse(&mut v, "scalable MPEG-4 video");
    }
    if r.overrun() {
        return Err(malformed("truncated video object layer header"));
    }
    Ok(v)
}

/// VOP coding types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VopType {
    I,
    P,
    B,
    /// Sprite (GMC) VOP: a P-VOP whose macroblocks may follow the global motion.
    S,
}

/// A VOP header.
#[derive(Clone, Debug)]
pub(crate) struct Vop {
    pub kind: VopType,
    pub coded: bool,
    /// Whole seconds (modulo_time_base) and the time increment.
    pub seconds: u32,
    pub increment: u32,
    /// A Sorenson disposable inter frame: never a reference.
    pub disposable: bool,
    pub rounding: bool,
    pub intra_dc_thr: u8,
    pub top_field_first: bool,
    pub alternate_scan: bool,
    pub quant: i32,
    pub fcode_f: u8,
    pub fcode_b: u8,
    /// Sprite trajectory: (du, dv) per warping point.
    pub warp: [[i32; 2]; 4],
    /// Bits of the header (the macroblock data starts here).
    pub header_bits: usize,
    /// Short header: GOB data instead of video packets.
    pub short_header: bool,
    /// The time increment's size as found (it may correct the layer's).
    pub time_bits: u32,
}

fn warping_mv(r: &mut Bits, dmv: &specular_mpeg2::util::Vlc) -> Result<i32, Error> {
    let len = u32::from(dmv.read(r).ok_or_else(|| malformed("sprite trajectory"))?);
    if len == 0 {
        return Ok(0);
    }
    let v = r.u(len) as i32;
    // The top bit set: positive; else v - 2^len + 1.
    Ok(if v >> (len - 1) & 1 == 1 {
        v
    } else {
        v - (1 << len) + 1
    })
}

/// The VOP header (after its start code).
pub(crate) fn vop(data: &[u8], vol: &Vol, dmv: &specular_mpeg2::util::Vlc) -> Result<Vop, Error> {
    let mut r = Bits::new(data);
    let kind = match r.u(2) {
        0 => VopType::I,
        1 => VopType::P,
        2 => VopType::B,
        _ => VopType::S,
    };
    let mut seconds = 0;
    while r.flag() {
        seconds += 1;
        if seconds > 60 || r.overrun() {
            return Err(malformed("modulo_time_base"));
        }
    }
    marker(&mut r)?;
    // A layer header whose time increment size doesn't fit the stream (old encoders): the
    // size whose following bits look like a VOP header's (marker, vop_coded, ...).
    let mut time_bits = vol.time_bits;
    if (r.peek32() >> (31 - time_bits)) & 1 == 0 {
        let p = u64::from(r.peek32()) << 32 | u64::from(r.clone_skip(32).peek32());
        let show = |n: u32| (p >> (64 - n)) as u32;
        if let Some(b) = (1..16).find(|&b| {
            if kind == VopType::P || kind == VopType::S && vol.sprite == 2 {
                show(b + 6) & 0x37 == 0x30
            } else {
                show(b + 5) & 0x1F == 0x18
            }
        }) {
            time_bits = b;
        }
    }
    let increment = r.u(time_bits);
    marker(&mut r)?;
    let coded = r.flag();
    let mut v = Vop {
        kind,
        coded,
        seconds,
        increment,
        disposable: false,
        rounding: false,
        intra_dc_thr: 0,
        top_field_first: false,
        alternate_scan: false,
        quant: 1,
        fcode_f: 1,
        fcode_b: 1,
        warp: [[0; 2]; 4],
        header_bits: 0,
        short_header: false,
        time_bits,
    };
    if !coded {
        return Ok(v);
    }
    if kind == VopType::S && vol.sprite != 2 {
        return Err(unsupported("MPEG-4 sprite VOPs"));
    }
    if matches!(kind, VopType::P) || kind == VopType::S && vol.sprite == 2 {
        v.rounding = r.flag();
    }
    if vol.reduced_resolution && matches!(kind, VopType::I | VopType::P) && r.flag() {
        return Err(unsupported("MPEG-4 reduced-resolution VOPs"));
    }
    v.intra_dc_thr = r.u(3) as u8;
    if vol.interlaced {
        v.top_field_first = r.flag();
        v.alternate_scan = r.flag();
    }
    if kind == VopType::S {
        for i in 0..usize::from(vol.warping_points) {
            v.warp[i][0] = warping_mv(&mut r, dmv)?;
            marker(&mut r)?;
            v.warp[i][1] = warping_mv(&mut r, dmv)?;
            marker(&mut r)?;
        }
    }
    v.quant = r.u(5) as i32;
    if v.quant == 0 {
        return Err(malformed("zero vop_quant"));
    }
    if kind != VopType::I {
        v.fcode_f = r.u(3) as u8;
        if v.fcode_f == 0 {
            return Err(malformed("zero vop_fcode_forward"));
        }
    }
    if kind == VopType::B {
        v.fcode_b = r.u(3) as u8;
        if v.fcode_b == 0 {
            return Err(malformed("zero vop_fcode_backward"));
        }
    }
    if r.overrun() {
        return Err(malformed("truncated VOP header"));
    }
    v.header_bits = r.position();
    Ok(v)
}

/// The H.263 picture header of a short header stream (`data` from the picture start code's
/// first byte): the picture's size and a VOP header for it.
pub(crate) fn short_header(data: &[u8]) -> Result<(Vol, Vop), Error> {
    let mut r = Bits::new(data);
    if r.u(22) != 0x20 {
        return Err(malformed("no H.263 picture start code"));
    }
    let _temporal_reference = r.u(8);
    if r.u(2) != 2 {
        return Err(malformed("H.263 PTYPE marker bits"));
    }
    let _split = r.flag();
    let _camera = r.flag();
    let _freeze = r.flag();
    let (w, h) = match r.u(3) {
        1 => (128, 96),
        2 => (176, 144),
        3 => (352, 288),
        4 => (704, 576),
        5 => (1408, 1152),
        7 => return Err(unsupported("H.263+ (PLUSPTYPE)")),
        _ => return Err(malformed("H.263 source format")),
    };
    let kind = if r.flag() { VopType::P } else { VopType::I };
    if r.flag() || r.flag() || r.flag() || r.flag() {
        return Err(unsupported(
            "H.263 options beyond baseline (unrestricted vectors, arithmetic coding, advanced prediction, PB frames)",
        ));
    }
    let quant = r.u(5) as i32;
    if r.flag() {
        // CPM: the sub-bitstream indicator.
        r.skip(2);
    }
    while r.flag() {
        r.skip(8);
    }
    if quant == 0 || r.overrun() {
        return Err(malformed("H.263 picture header"));
    }
    let mut vol = Vol::base();
    vol.short_header = true;
    vol.width = w;
    vol.height = h;
    vol.time_resolution = 30000;
    vol.time_bits = 15;
    vol.fixed_increment = Some(1001);
    vol.pixel_aspect = Some((12, 11));
    vol.low_delay = true;
    let vop = Vop {
        kind,
        coded: true,
        seconds: 0,
        increment: 0,
        disposable: false,
        rounding: false,
        intra_dc_thr: 7,
        top_field_first: false,
        alternate_scan: false,
        quant,
        fcode_f: 1,
        fcode_b: 1,
        warp: [[0; 2]; 4],
        header_bits: r.position(),
        short_header: true,
        time_bits: 15,
    };
    Ok((vol, vop))
}

/// The picture header of a Sorenson Spark frame (FLV codec 2; the SWF File Format
/// Specification, "Sorenson H.263 Bitstream Format"): a 17-bit start code, the version,
/// the size (a code or 8/16 bits each way), the picture type (intra, inter, disposable inter),
/// the advisory deblocking flag and the quantiser; then H.263 macroblocks without GOBs.
pub(crate) fn sorenson_header(data: &[u8]) -> Result<(Vol, Vop), Error> {
    let mut r = Bits::new(data);
    if r.u(17) != 1 {
        return Err(malformed("no Sorenson Spark picture start code"));
    }
    let version = r.u(5);
    if version > 1 {
        return Err(Error::Unsupported(format!(
            "Sorenson Spark version {version}"
        )));
    }
    let _temporal_reference = r.u(8);
    let (w, h) = match r.u(3) {
        0 => (r.u(8), r.u(8)),
        1 => (r.u(16), r.u(16)),
        2 => (352, 288),
        3 => (176, 144),
        4 => (128, 96),
        5 => (320, 240),
        6 => (160, 120),
        _ => return Err(malformed("Sorenson Spark picture size")),
    };
    if w == 0 || h == 0 {
        return Err(malformed("Sorenson Spark picture size"));
    }
    if w > 4096 || h > 4096 {
        return Err(Error::Unsupported(format!("{w}×{h} Sorenson Spark video")));
    }
    let (kind, disposable) = match r.u(2) {
        0 => (VopType::I, false),
        1 => (VopType::P, false),
        2 => (VopType::P, true),
        _ => return Err(malformed("Sorenson Spark picture type")),
    };
    let _deblocking = r.flag();
    let quant = r.u(5) as i32;
    while r.flag() {
        r.skip(8);
    }
    if quant == 0 || r.overrun() {
        return Err(malformed("Sorenson Spark picture header"));
    }
    let mut vol = Vol::base();
    vol.short_header = true;
    vol.sorenson = Some(version as u8);
    vol.width = w;
    vol.height = h;
    vol.time_resolution = 1000;
    vol.time_bits = 10;
    vol.low_delay = true;
    let vop = Vop {
        kind,
        coded: true,
        seconds: 0,
        increment: 0,
        disposable,
        rounding: false,
        intra_dc_thr: 7,
        top_field_first: false,
        alternate_scan: false,
        quant,
        fcode_f: 1,
        fcode_b: 1,
        warp: [[0; 2]; 4],
        header_bits: r.position(),
        short_header: true,
        time_bits: 10,
    };
    Ok((vol, vop))
}

/// DivX and Xvid user data: the DivX version and whether B-VOPs are packed into the chunk
/// of the P-VOP before them (DivX's "p" flag).
pub(crate) fn divx_packed(data: &[u8]) -> Option<bool> {
    let s = std::str::from_utf8(data.get(..data.len().min(32))?).ok()?;
    let rest = s.strip_prefix("DivX")?;
    let digits = rest.find(|c: char| !c.is_ascii_digit())?;
    let tail = rest[digits..].trim_end_matches('\0');
    Some(tail.ends_with('p'))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A VOL header of a few bytes declaring 8191 × 8191 (the 13-bit maximum): refused
    /// before any frame is allocated; 1920 × 1080 is read.
    #[test]
    fn huge_vol_refused() {
        let vol_of = |w: u32, h: u32| {
            let mut bits = String::new();
            let mut put = |v: u32, n: u32| {
                for i in (0..n).rev() {
                    bits.push(if v >> i & 1 == 1 { '1' } else { '0' });
                }
            };
            put(0, 1); // random_accessible_vol
            put(1, 8); // video_object_type_indication
            put(0, 1); // is_object_layer_identifier
            put(1, 4); // aspect_ratio_info
            put(0, 1); // vol_control_parameters
            put(0, 2); // rectangular shape
            put(1, 1);
            put(30, 16); // vop_time_increment_resolution
            put(1, 1);
            put(0, 1); // fixed_vop_rate
            put(1, 1);
            put(w, 13);
            put(1, 1);
            put(h, 13);
            put(1, 1);
            bits.push_str(&"0".repeat(64));
            let bytes: Vec<u8> = bits
                .as_bytes()
                .chunks(8)
                .map(|c| c.iter().fold(0u8, |a, &b| a << 1 | (b - b'0')) << (8 - c.len()))
                .collect();
            vol(&bytes, VisualObject::default(), 0)
        };
        assert!(matches!(vol_of(8191, 8191), Err(Error::Unsupported(_))));
        let v = vol_of(1920, 1080).unwrap();
        assert_eq!((v.width, v.height), (1920, 1080));
    }

    #[test]
    fn divx_user_data() {
        assert_eq!(divx_packed(b"DivX503b1393p"), Some(true));
        assert_eq!(divx_packed(b"DivX503b1393"), Some(false));
        assert_eq!(divx_packed(b"XviD0050"), None);
    }
}
