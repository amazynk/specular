//! The few serial-free facts of a Panasonic maker note: the lens name (LensType, 0x0051), the
//! focus mode (0x0007) and image stabilisation (0x001A).
//!
//! The note is "Panasonic\0\0\0", then an IFD whose offsets count from the TIFF header, in the
//! TIFF's byte order (as cameras write it, and as `specular-decode` rebuilds it from an RW2's
//! JpgFromRaw). Serial numbers and the entries naming people or places are never read.

/// What the maker note says; any part may be missing.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct PanasonicNote {
    pub lens: Option<String>,
    /// FocusMode (raw code).
    pub focus_mode: Option<u16>,
    /// ImageStabilization (raw code).
    pub stabilisation: Option<u16>,
}

/// Entries read at most.
const MAX_ENTRIES: usize = 1024;

/// The note at `at` of the TIFF structure `buf`, or None when it is not a Panasonic note.
pub(crate) fn parse(buf: &[u8], le: bool, at: usize) -> Option<PanasonicNote> {
    let ifd = at.checked_add(12)?;
    if buf.get(at..ifd)? != b"Panasonic\0\0\0" {
        return None;
    }
    let u16a = |p: usize| -> Option<u16> {
        let b: [u8; 2] = buf.get(p..p.checked_add(2)?)?.try_into().ok()?;
        Some(if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    };
    let u32a = |p: usize| -> Option<usize> {
        let b: [u8; 4] = buf.get(p..p.checked_add(4)?)?.try_into().ok()?;
        Some(if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        } as usize)
    };
    let n = usize::from(u16a(ifd)?).min(MAX_ENTRIES);
    let mut note = PanasonicNote::default();
    for k in 0..n {
        let e = ifd + 2 + 12 * k;
        let (Some(tag), Some(typ), Some(count)) = (u16a(e), u16a(e + 2), u32a(e + 4)) else {
            break;
        };
        match (tag, typ, count) {
            (0x0007, 3, 1) => note.focus_mode = u16a(e + 8).filter(|&v| focus_mode_known(v)),
            (0x001A, 3, 1) => {
                note.stabilisation = u16a(e + 8).filter(|&v| stabilisation_known(v));
            }
            (0x0051, 2, 1..=256) => {
                let p = if count <= 4 { Some(e + 8) } else { u32a(e + 8) };
                let b = p.and_then(|p| buf.get(p..p.checked_add(count)?));
                if let Some(b) = b {
                    let b = b.split(|&c| c == 0).next().unwrap_or_default();
                    let s = String::from_utf8_lossy(b).trim().to_owned();
                    note.lens = (!s.is_empty()).then_some(s);
                }
            }
            _ => {}
        }
    }
    Some(note)
}

/// FocusMode codes with a name: Auto, Manual, Auto (focus button), Auto (continuous), AF-S,
/// AF-C, AF-F.
pub(crate) fn focus_mode_known(v: u16) -> bool {
    matches!(v, 1 | 2 | 4..=8)
}

/// ImageStabilization codes with a name: optical, off, mode 2, optical panning, body-only
/// (and panning), Dual IS and Dual2 IS (and panning).
pub(crate) fn stabilisation_known(v: u16) -> bool {
    matches!(v, 2..=7 | 9..=12)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A TIFF header, then at 8 a Panasonic note in byte order `le`: SHORT entries `shorts`
    /// and LensType `lens`, its value after the IFD (offsets from the header).
    fn note(le: bool, shorts: &[(u16, u16)], lens: &[u8]) -> Vec<u8> {
        let w16 = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let w32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut b = if le { b"II*\0" } else { b"MM\0*" }.to_vec();
        b.extend(w32(8));
        b.extend(b"Panasonic\0\0\0");
        let n = shorts.len() + 1;
        let value = 8 + 12 + 2 + 12 * n + 4;
        b.extend(w16(n as u16));
        for &(tag, v) in shorts {
            b.extend(w16(tag));
            b.extend(w16(3));
            b.extend(w32(1));
            b.extend(w16(v));
            b.extend([0; 2]);
        }
        b.extend(w16(0x0051));
        b.extend(w16(2));
        b.extend(w32(lens.len() as u32));
        if lens.len() <= 4 {
            let mut v = lens.to_vec();
            v.resize(4, 0);
            b.extend(v);
        } else {
            b.extend(w32(value as u32));
        }
        b.extend([0; 4]);
        assert_eq!(b.len(), value);
        b.extend(lens);
        b
    }

    #[test]
    fn reads_the_note() {
        const LENS: &str = "LEICA DG 12-60/F2.8-4.0";
        for le in [true, false] {
            let b = note(
                le,
                &[(0x0001, 2), (0x0007, 7), (0x001A, 11)],
                b"LEICA DG 12-60/F2.8-4.0\0",
            );
            let n = parse(&b, le, 8).unwrap();
            assert_eq!(n.lens.as_deref(), Some(LENS));
            assert_eq!(n.focus_mode, Some(7));
            assert_eq!(n.stabilisation, Some(11));
            let short = note(le, &[], b"X\0");
            assert_eq!(parse(&short, le, 8).unwrap().lens.as_deref(), Some("X"));
            // Unknown codes and a blank lens are left out.
            let n = parse(
                &note(le, &[(0x0007, 3), (0x001A, 22)], b"  \0\0\0\0"),
                le,
                8,
            );
            assert_eq!(n, Some(PanasonicNote::default()));
            // Not a Panasonic note; cut short.
            assert_eq!(parse(&b, le, 9), None);
            let cut = parse(&b[..b.len() - 4], le, 8).unwrap();
            assert_eq!((cut.lens, cut.focus_mode), (None, Some(7)));
            // Damage: every truncation and byte flip parses or fails without panicking.
            for i in 0..b.len() {
                let _ = parse(&b[..i], le, 8);
                let mut m = b.clone();
                m[i] ^= 0xFF;
                let _ = parse(&m, le, 8);
                let _ = parse(&m, !le, 8);
            }
        }
        assert!(parse(b"Panasonic\0\0\0", true, usize::MAX - 4).is_none());
    }
}
