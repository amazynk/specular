//! The lens of a Samsung maker note: its LensType code (0xA003) named, for SRWs and Samsung
//! JPEGs whose Exif has no LensModel (the NX cameras before the NX1 and NX500).
//!
//! The note is the IFD-style "Type 2" note of the NX cameras: an IFD straight away, its first
//! entry MakerNoteVersion (UNDEFINED[4] "0100"), as `specular-decode` rebuilds it. The serial
//! numbers (0xA002, 0xA005) are never read.

/// What the maker note says.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct SamsungNote {
    pub lens: Option<String>,
}

/// Entries read at most.
const MAX_ENTRIES: usize = 1024;

/// The note at `at` of the TIFF structure `buf`, or None when it is not a Type 2 note.
pub(crate) fn parse(buf: &[u8], le: bool, at: usize) -> Option<SamsungNote> {
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
    let first = at.checked_add(2)?;
    let version = (u16a(first)?, u16a(first + 2)?, u32a(first + 4)?);
    if version != (0x0001, 7, 4) || buf.get(first + 8..first + 12)? != b"0100" {
        return None;
    }
    let n = usize::from(u16a(at)?).min(MAX_ENTRIES);
    let mut note = SamsungNote::default();
    for k in 0..n {
        let e = first + 12 * k;
        let (Some(tag), Some(typ), Some(count)) = (u16a(e), u16a(e + 2), u32a(e + 4)) else {
            break;
        };
        if (tag, typ, count) == (0xA003, 3, 1) {
            note.lens = u16a(e + 8).and_then(lens_name).map(str::to_owned);
        }
    }
    Some(note)
}

/// The name of Samsung LensType `code`; None for 0 (a built-in or manual lens) and unknown
/// codes.
pub(crate) fn lens_name(code: u16) -> Option<&'static str> {
    Some(match code {
        1 => "Samsung NX 30mm F2 Pancake",
        2 => "Samsung NX 18-55mm F3.5-5.6 OIS",
        3 => "Samsung NX 50-200mm F4-5.6 ED OIS",
        4 => "Samsung NX 20-50mm F3.5-5.6 ED",
        5 => "Samsung NX 20mm F2.8 Pancake",
        6 => "Samsung NX 18-200mm F3.5-6.3 ED OIS",
        7 => "Samsung NX 60mm F2.8 Macro ED OIS SSA",
        8 => "Samsung NX 16mm F2.4 Pancake",
        9 => "Samsung NX 85mm F1.4 ED SSA",
        10 => "Samsung NX 45mm F1.8",
        11 => "Samsung NX 45mm F1.8 2D/3D",
        12 => "Samsung NX 12-24mm F4-5.6 ED",
        13 => "Samsung NX 16-50mm F2-2.8 S ED OIS",
        14 => "Samsung NX 10mm F3.5 Fisheye",
        15 => "Samsung NX 16-50mm F3.5-5.6 Power Zoom ED OIS",
        20 => "Samsung NX 50-150mm F2.8 S ED OIS",
        21 => "Samsung NX 300mm F2.8 ED OIS",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A TIFF header, then at 8 a Samsung note in byte order `le`: MakerNoteVersion, a serial
    /// number (ASCII, after the IFD) and LensType `lens`.
    fn note(le: bool, lens: u16) -> Vec<u8> {
        let w16 = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let w32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut b = if le { b"II*\0" } else { b"MM\0*" }.to_vec();
        b.extend(w32(8));
        b.extend(w16(3));
        b.extend(w16(0x0001));
        b.extend(w16(7));
        b.extend(w32(4));
        b.extend(b"0100");
        b.extend(w16(0xA002));
        b.extend(w16(2));
        b.extend(w32(8));
        b.extend(w32(8 + 2 + 36 + 4));
        b.extend(w16(0xA003));
        b.extend(w16(3));
        b.extend(w32(1));
        b.extend(w16(lens));
        b.extend([0; 2]);
        b.extend([0; 4]);
        b.extend(b"SN-0042\0");
        b
    }

    #[test]
    fn reads_the_lens() {
        for le in [true, false] {
            let b = note(le, 13);
            let n = parse(&b, le, 8).unwrap();
            assert_eq!(
                n.lens.as_deref(),
                Some("Samsung NX 16-50mm F2-2.8 S ED OIS")
            );
            // A built-in or manual lens, an unknown code.
            assert_eq!(parse(&note(le, 0), le, 8), Some(SamsungNote::default()));
            assert_eq!(parse(&note(le, 99), le, 8), Some(SamsungNote::default()));
            // Not a Type 2 note: another first entry, another place.
            let mut other = b.clone();
            other[if le { 10 } else { 11 }] = 2;
            assert_eq!(parse(&other, le, 8), None);
            assert_eq!(parse(&b, le, 9), None);
            // Damage: every truncation and byte flip parses or fails without panicking.
            for i in 0..b.len() {
                let _ = parse(&b[..i], le, 8);
                let mut m = b.clone();
                m[i] ^= 0xFF;
                let _ = parse(&m, le, 8);
                let _ = parse(&m, !le, 8);
            }
        }
        assert!(parse(&[0; 4], true, usize::MAX - 1).is_none());
    }
}
