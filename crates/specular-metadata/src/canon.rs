//! The few serial-free facts of a Canon maker note: the lens name (0x0095) and, from
//! CameraSettings (0x0001), the focus mode, image stabilisation, the lens's focal range and
//! its widest aperture at the focal length used.
//!
//! The note is an IFD whose offsets count from the TIFF header, in the TIFF's byte order (as
//! Canon writes it in JPEGs, and as `specular-decode` rebuilds it from a CR3's CMT3). Serial
//! numbers (0x000C, 0x0096) are never read.

/// What the maker note says; any part may be missing.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct CanonNote {
    pub lens: Option<String>,
    /// CameraSettings FocusMode (raw code).
    pub focus_mode: Option<u16>,
    /// CameraSettings ImageStabilization (raw code).
    pub stabilisation: Option<u16>,
    /// The lens's shortest and longest focal length, in mm.
    pub focal: Option<(f64, f64)>,
    /// The widest aperture as an f-number, for a prime only: on a zoom the camera records the
    /// one at the current focal length, not the lens's.
    pub prime_aperture: Option<f64>,
    /// CameraSettings MaxAperture: the widest aperture at the focal length used, as an
    /// f-number (MinAperture, index 27, is the narrowest one, of no use for the lens's spec).
    pub aperture_now: Option<f64>,
}

/// Entries read at most (Canon notes have a few dozen).
const MAX_ENTRIES: usize = 512;

/// The note at `at` of the TIFF structure `buf`.
pub(crate) fn parse(buf: &[u8], le: bool, at: usize) -> Option<CanonNote> {
    let u16a = |p: usize| -> Option<u16> {
        let b: [u8; 2] = buf.get(p..p.checked_add(2)?)?.try_into().ok()?;
        Some(if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    };
    let u32a = |p: usize| -> Option<u32> {
        let b: [u8; 4] = buf.get(p..p.checked_add(4)?)?.try_into().ok()?;
        Some(if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    };
    let n = usize::from(u16a(at)?).min(MAX_ENTRIES);
    let mut note = CanonNote::default();
    let mut settings: Vec<u16> = Vec::new();
    for i in 0..n {
        let e = at + 2 + 12 * i;
        let (Some(tag), Some(typ), Some(count)) = (u16a(e), u16a(e + 2), u32a(e + 4)) else {
            break;
        };
        let count = count as usize;
        // The value: inline when it fits in 4 bytes, else at the offset.
        let place = |size: usize| -> Option<usize> {
            let len = size.checked_mul(count)?;
            if len <= 4 {
                Some(e + 8)
            } else {
                Some(u32a(e + 8)? as usize)
                    .filter(|&p| p.checked_add(len).is_some_and(|end| end <= buf.len()))
            }
        };
        match (tag, typ) {
            // CameraSettings: SHORTs, element 0 the length in bytes.
            (0x0001, 3) if count <= 256 => {
                if let Some(p) = place(2) {
                    settings = (0..count).map_while(|k| u16a(p + 2 * k)).collect();
                }
            }
            // LensModel: ASCII, junk after the NUL.
            (0x0095, 2) if count <= 256 => {
                if let Some(p) = place(1)
                    && let Some(b) = buf.get(p..p + count)
                {
                    let b = b.split(|&c| c == 0).next().unwrap_or_default();
                    let s = String::from_utf8_lossy(b).trim().to_owned();
                    if s.chars().any(char::is_alphanumeric) {
                        note.lens = Some(s);
                    }
                }
            }
            _ => {}
        }
    }
    let cs = |k: usize| settings.get(k).copied();
    note.focus_mode = cs(7).filter(|&v| focus_mode_known(v));
    note.stabilisation = cs(34).filter(|&v| stabilisation_known(v));
    note.aperture_now = cs(26).and_then(canon_ev_f_number);
    let units = f64::from(cs(25).filter(|&u| u > 0).unwrap_or(1));
    if let (Some(long), Some(short)) = (cs(23), cs(24))
        && short > 0
        && short <= long
    {
        let (short, long) = (f64::from(short) / units, f64::from(long) / units);
        // A real lens: compacts' sensors from a few mm, telephotos to a few metres; anything
        // else (0xFFFF, a misread note's 0–0.1 mm) is not one. Compacts count in µm (units
        // 1000), so the raw values pass 0x7FFF.
        if !(1.0..=5000.0).contains(&short) || long > 5000.0 {
            return Some(note);
        }
        note.focal = Some((short, long));
        if short == long {
            note.prime_aperture = cs(26).and_then(canon_ev_f_number);
        }
    }
    Some(note)
}

/// FocusMode codes with a name (live-view variants add 256).
pub(crate) fn focus_mode_known(v: u16) -> bool {
    matches!(v, 0..=6 | 16 | 256..=258 | 512 | 519)
}

/// ImageStabilization codes with a name (the second set adds 256).
pub(crate) fn stabilisation_known(v: u16) -> bool {
    matches!(v, 0..=4 | 256..=260)
}

/// An f-number from a Canon EV aperture code (thirds marked by fractions 0x0C and 0x14).
fn canon_ev_f_number(v: u16) -> Option<f64> {
    let v = i32::from(v as i16);
    if v <= 0 {
        return None;
    }
    let frac = v & 0x1F;
    let frac_ev = match frac {
        0x0C => 32.0 / 3.0,
        0x14 => 64.0 / 3.0,
        f => f64::from(f),
    };
    let ev = (f64::from(v - frac) + frac_ev) / 32.0;
    let f = 2f64.powf(ev / 2.0);
    // f/0.7 .. f/64, to the hundredth.
    (0.7..=64.0)
        .contains(&f)
        .then(|| (f * 100.0).round() / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(entries: &[(u16, u16, u32, Vec<u8>)]) -> Vec<u8> {
        let mut t = b"II*\0\x08\0\0\0".to_vec();
        t.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        let mut data = 8 + 2 + 12 * entries.len() as u32 + 4;
        let mut tail = Vec::new();
        for (tag, typ, count, v) in entries {
            t.extend_from_slice(&tag.to_le_bytes());
            t.extend_from_slice(&typ.to_le_bytes());
            t.extend_from_slice(&count.to_le_bytes());
            if v.len() <= 4 {
                let mut b = [0; 4];
                b[..v.len()].copy_from_slice(v);
                t.extend_from_slice(&b);
            } else {
                t.extend_from_slice(&data.to_le_bytes());
                tail.extend_from_slice(v);
                data += v.len() as u32;
            }
        }
        t.extend_from_slice(&[0; 4]);
        t.extend(tail);
        t
    }

    fn settings(set: &[(usize, u16)]) -> Vec<u8> {
        let mut cs = [0u16; 40];
        cs[0] = 80;
        for &(k, v) in set {
            cs[k] = v;
        }
        cs.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn reads_the_note() {
        let t = note(&[
            (
                0x0001,
                3,
                40,
                settings(&[(7, 1), (23, 35), (24, 35), (25, 1), (26, 64), (34, 257)]),
            ),
            (0x0095, 2, 20, b"EF35mm f/2 IS\0\x01junk!".to_vec()),
        ]);
        let n = parse(&t, true, 8).unwrap();
        assert_eq!(n.lens.as_deref(), Some("EF35mm f/2 IS"));
        assert_eq!(n.focus_mode, Some(1));
        assert_eq!(n.stabilisation, Some(257));
        assert_eq!(n.focal, Some((35.0, 35.0)));
        assert_eq!(n.prime_aperture, Some(2.0));
        // A zoom: the range, no aperture; unknown codes dropped; a blank lens name ignored.
        let t = note(&[
            (
                0x0001,
                3,
                40,
                settings(&[(7, 99), (23, 1050), (24, 240), (25, 10), (26, 128), (34, 7)]),
            ),
            (0x0095, 2, 8, b"  \0\0\0\0\0\0".to_vec()),
        ]);
        let n = parse(&t, true, 8).unwrap();
        assert_eq!(n.lens, None);
        assert_eq!(n.focus_mode, None);
        assert_eq!(n.stabilisation, None);
        assert_eq!(n.focal, Some((24.0, 105.0)));
        assert_eq!(n.prime_aperture, None);
        assert_eq!(n.aperture_now, Some(4.0));
        // Implausible ranges (a note read from the wrong place) are dropped.
        for (long, short, units) in [
            (1, 0, 10),
            (1, 1, 10),
            (40000, 30000, 1),
            (85, 17, 100),
            (0xFFFF, 0xFFFF, 1),
        ] {
            let cs = settings(&[(23, long), (24, short), (25, units)]);
            let n = parse(&note(&[(0x0001, 3, 40, cs)]), true, 8).unwrap();
            assert_eq!(n.focal, None, "{short}–{long} / {units}");
        }
        // A compact's lens in µm: 8.8–36.8 mm.
        let cs = settings(&[(23, 36800), (24, 8800), (25, 1000)]);
        let n = parse(&note(&[(0x0001, 3, 40, cs)]), true, 8).unwrap();
        assert_eq!(n.focal, Some((8.8, 36.8)));
        // Damage: every truncation and byte flip parses or fails without panicking.
        for i in 0..t.len() {
            let _ = parse(&t[..i], true, 8);
            let mut m = t.clone();
            m[i] ^= 0xFF;
            let _ = parse(&m, true, 8);
            let _ = parse(&m, false, 8);
        }
        assert!(parse(&t, true, usize::MAX - 1).is_none());
    }

    #[test]
    fn canon_ev() {
        assert_eq!(canon_ev_f_number(64), Some(2.0));
        assert_eq!(canon_ev_f_number(32), Some(1.41));
        assert_eq!(canon_ev_f_number(0x2C), Some(1.59)); // 1 + 1/3 EV
        assert_eq!(canon_ev_f_number(0x34), Some(1.78)); // 1 + 2/3 EV
        assert_eq!(canon_ev_f_number(96), Some(2.83));
        assert_eq!(canon_ev_f_number(0), None);
        assert_eq!(canon_ev_f_number(0xFFFF), None);
        assert_eq!(canon_ev_f_number(0x7FFF), None);
    }
}
