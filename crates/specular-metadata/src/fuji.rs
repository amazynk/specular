//! The serial-free facts of a Fujifilm maker note: the film simulation and tone settings, the
//! white balance, focus and drive modes, the shutter type and stabilisation.
//!
//! The note is "FUJIFILM", a little-endian offset to its IFD, then the IFD: always
//! little-endian, its value offsets counted from the start of the note (so it reads the same
//! from a JPEG, the JPEG preview of a RAF, or a rebuilt TIFF structure). Written from the
//! entries' public descriptions; the serial number (0x0010) and the face entries are never
//! read.

/// What the maker note says; any part may be missing. Codes are kept raw and named by the
/// `fuji-*` messages.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct FujiNote {
    /// 0x1001 Sharpness.
    pub sharpness: Option<u16>,
    /// 0x1002 WhiteBalance.
    pub white_balance: Option<u16>,
    /// 0x1003 Saturation ("Color"; also the monochrome film simulations).
    pub saturation: Option<u16>,
    /// 0x1005 ColorTemperature (kelvin), for the Kelvin white balance.
    pub colour_temperature: Option<u16>,
    /// 0x100E NoiseReduction (high-ISO noise reduction).
    pub noise_reduction: Option<u16>,
    /// 0x100F Clarity (×1000).
    pub clarity: Option<i32>,
    /// 0x1021 FocusMode (0 auto, 1 manual).
    pub focus_mode: Option<u16>,
    /// FocusSettings (0x102D) bits 0–3: 0 manual, 1 single (AF-S), 2 continuous (AF-C).
    pub focus_mode2: Option<u8>,
    /// 0x1031 PictureMode.
    pub picture_mode: Option<u16>,
    /// 0x1040 ShadowTone and 0x1041 HighlightTone (−16 per step up).
    pub shadow_tone: Option<i32>,
    pub highlight_tone: Option<i32>,
    /// 0x1047 GrainEffectRoughness (0 off, 32 weak, 64 strong).
    pub grain: Option<i32>,
    /// 0x104C GrainEffectSize (16 small, 32 large).
    pub grain_size: Option<u16>,
    /// 0x1048 ColorChromeEffect and 0x104E ColorChromeFXBlue (0 off, 32 weak, 64 strong).
    pub colour_chrome: Option<i32>,
    pub colour_chrome_blue: Option<i32>,
    /// 0x1050 ShutterType.
    pub shutter_type: Option<u16>,
    /// DriveSettings (0x1103) bits 0–7: 0 single, 1 continuous low, 2 continuous high.
    pub drive_mode: Option<u8>,
    /// 0x1401 FilmMode (colour film simulations; absent for monochrome).
    pub film_mode: Option<u16>,
    /// 0x1402 DynamicRangeSetting (0 auto, 1 manual, …).
    pub dynamic_range_setting: Option<u16>,
    /// 0x1403 DevelopmentDynamicRange (100, 200, 400 %).
    pub development_dynamic_range: Option<u16>,
    /// 0x1422 ImageStabilization: type, mode, and a third value.
    pub stabilisation: Option<[u16; 3]>,
    /// 0x1431 Rating (0–5).
    pub rating: Option<u32>,
}

/// Entries read at most.
const MAX_ENTRIES: usize = 1024;

/// Bytes per value of each TIFF type.
fn type_size(t: u16) -> Option<usize> {
    Some(match t {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 => 4,
        5 | 10 | 12 => 8,
        _ => return None,
    })
}

/// The maker note `note` (the MakerNote value's bytes), or None when it is not a Fujifilm
/// note.
pub(crate) fn parse(note: &[u8]) -> Option<FujiNote> {
    if !note.starts_with(b"FUJIFILM") {
        return None;
    }
    let u16a = |p: usize| -> Option<u16> {
        Some(u16::from_le_bytes(
            note.get(p..p.checked_add(2)?)?.try_into().ok()?,
        ))
    };
    let u32a = |p: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            note.get(p..p.checked_add(4)?)?.try_into().ok()?,
        ))
    };
    let ifd = u32a(8)? as usize;
    let n = usize::from(u16a(ifd)?).min(MAX_ENTRIES);
    let mut f = FujiNote::default();
    for k in 0..n {
        let e = ifd + 2 + 12 * k;
        let (Some(tag), Some(typ), Some(count)) = (u16a(e), u16a(e + 2), u32a(e + 4)) else {
            break;
        };
        let Some(size) = type_size(typ).and_then(|s| s.checked_mul(count as usize)) else {
            continue;
        };
        let at = if size <= 4 {
            e + 8
        } else {
            match u32a(e + 8) {
                Some(o) => o as usize,
                None => continue,
            }
        };
        let Some(v) = note.get(at..at.saturating_add(size)) else {
            continue;
        };
        // The first value, as the type stores it.
        let short = || (typ == 3 && v.len() >= 2).then(|| u16::from_le_bytes([v[0], v[1]]));
        let long = || {
            (matches!(typ, 4 | 9) && v.len() >= 4)
                .then(|| i32::from_le_bytes([v[0], v[1], v[2], v[3]]))
        };
        // A bit field stored as a LONG, or as UNDEFINED bytes (older firmware).
        let bits = || (v.len() >= 4).then(|| u32::from_le_bytes([v[0], v[1], v[2], v[3]]));
        match tag {
            0x1001 => f.sharpness = short(),
            0x1002 => f.white_balance = short(),
            0x1003 => f.saturation = short(),
            0x1005 => f.colour_temperature = short().filter(|&k| k > 0),
            0x100E => f.noise_reduction = short(),
            0x100F => f.clarity = long(),
            0x1021 => f.focus_mode = short(),
            0x102D => f.focus_mode2 = bits().map(|b| (b & 0x0F) as u8),
            0x1031 => f.picture_mode = short(),
            0x1040 => f.shadow_tone = long(),
            0x1041 => f.highlight_tone = long(),
            0x1047 => f.grain = long(),
            0x1048 => f.colour_chrome = long(),
            0x104C => f.grain_size = short(),
            0x104E => f.colour_chrome_blue = long(),
            0x1050 => f.shutter_type = short(),
            0x1103 => f.drive_mode = bits().map(|b| (b & 0xFF) as u8),
            0x1401 => f.film_mode = short(),
            0x1402 => f.dynamic_range_setting = short(),
            0x1403 => f.development_dynamic_range = short().filter(|&p| p > 0),
            0x1422 if typ == 3 && count >= 3 => {
                let s = |i: usize| u16::from_le_bytes([v[2 * i], v[2 * i + 1]]);
                f.stabilisation = Some([s(0), s(1), s(2)]);
            }
            0x1431 => f.rating = long().and_then(|r| u32::try_from(r).ok()),
            _ => {}
        }
    }
    Some(f)
}

/// WhiteBalance codes with a name.
pub(crate) fn white_balance_known(v: u16) -> bool {
    matches!(
        v,
        0..=2 | 256 | 512 | 768..=772 | 1024 | 1280 | 1536 | 3840..=3844 | 4080
    )
}

/// The monochrome film simulations, from Saturation.
pub(crate) fn monochrome_known(v: u16) -> bool {
    matches!(v, 768..=771 | 784 | 1280..=1283)
}

/// FilmMode codes with a name.
pub(crate) fn film_mode_known(v: u16) -> bool {
    matches!(
        v,
        0 | 256
            | 272
            | 288
            | 304
            | 512
            | 768
            | 1024
            | 1280
            | 1281
            | 1536
            | 1792
            | 2048
            | 2304
            | 2560
            | 2816
    )
}

/// ShutterType codes with a name.
pub(crate) fn shutter_type_known(v: u16) -> bool {
    v <= 3
}

/// ImageStabilization type codes with a name (the first value).
pub(crate) fn stabilisation_known(v: u16) -> bool {
    matches!(v, 0..=3 | 258 | 512)
}

/// The effect strengths (grain, colour chrome): 0 off, 32 weak, 64 strong.
pub(crate) fn effect_known(v: i32) -> bool {
    matches!(v, 0 | 32 | 64)
}

/// A tone step from a ShadowTone / HighlightTone value (−16 per step, so −32 is +2), when
/// it is a whole or half step.
pub(crate) fn tone_steps(v: i32) -> Option<f64> {
    (v % 8 == 0 && (-64..=64).contains(&v)).then(|| -f64::from(v) / 16.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Fujifilm note with `entries` (tag, type, count, value bytes); values over 4 bytes
    /// follow the IFD.
    fn note(entries: &[(u16, u16, u32, Vec<u8>)]) -> Vec<u8> {
        let mut b = b"FUJIFILM".to_vec();
        b.extend(12u32.to_le_bytes());
        b.extend((entries.len() as u16).to_le_bytes());
        let mut data_at = 12 + 2 + 12 * entries.len() + 4;
        let mut data: Vec<u8> = Vec::new();
        for (tag, typ, count, v) in entries {
            b.extend(tag.to_le_bytes());
            b.extend(typ.to_le_bytes());
            b.extend(count.to_le_bytes());
            if v.len() <= 4 {
                let mut x = v.clone();
                x.resize(4, 0);
                b.extend(x);
            } else {
                b.extend((data_at as u32).to_le_bytes());
                data.extend(v);
                data_at += v.len();
            }
        }
        b.extend([0; 4]);
        b.extend(data);
        b
    }

    #[test]
    fn reads_the_settings() {
        let s = |v: u16| v.to_le_bytes().to_vec();
        let l = |v: i32| v.to_le_bytes().to_vec();
        let n = note(&[
            (0x0010, 2, 8, b"SERIAL1\0".to_vec()),
            (0x1002, 3, 1, s(4080)),
            (0x1005, 3, 1, s(5600)),
            (0x1040, 9, 1, l(-32)),
            (0x1041, 9, 1, l(16)),
            (0x102D, 7, 4, vec![0x02, 0x40, 0, 0]),
            (0x1103, 4, 1, l(2)),
            (0x1401, 3, 1, s(1536)),
            (0x1422, 3, 3, [s(2), s(1), s(0)].concat()),
        ]);
        let f = parse(&n).unwrap();
        assert_eq!(f.white_balance, Some(4080));
        assert_eq!(f.colour_temperature, Some(5600));
        assert_eq!(f.shadow_tone.and_then(tone_steps), Some(2.0));
        assert_eq!(f.highlight_tone.and_then(tone_steps), Some(-1.0));
        assert_eq!(f.focus_mode2, Some(2));
        assert_eq!(f.drive_mode, Some(2));
        assert_eq!(f.film_mode, Some(1536));
        assert_eq!(f.stabilisation, Some([2, 1, 0]));
        assert!(parse(b"Nikon\0\x02\x10\0\0").is_none());
        // Cut short: what fits is read, nothing panics.
        for k in 0..n.len() {
            let _ = parse(&n[..k]);
        }
    }
}
