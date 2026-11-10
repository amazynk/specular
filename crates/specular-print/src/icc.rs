//! A minimal sRGB ICC profile (version 2, matrix/TRC display class) for the PDF's
//! `ICCBased` colour space and its output intent.
//!
//! Built from the IEC 61966-2-1 definition: the D50-adapted primaries (Bradford, as the ICC
//! specification's sRGB example), a D50 media white point and a 1024-entry tone curve of the
//! sRGB transfer function, shared by the three channels. Version 2 so that every PDF reader
//! (PDF/A-1 included) accepts it.

/// D50, the profile connection space white.
const D50: [f64; 3] = [0.9642, 1.0, 0.8249];

/// The sRGB primaries adapted to D50: columns red, green, blue.
const RXYZ: [f64; 3] = [0.436_074_7, 0.222_504_5, 0.013_932_2];
const GXYZ: [f64; 3] = [0.385_064_9, 0.716_878_6, 0.097_104_5];
const BXYZ: [f64; 3] = [0.143_080_4, 0.060_616_9, 0.714_173_3];

/// Entries of the tone curve.
const CURVE_POINTS: usize = 1024;

/// The profile's description, also used as the output condition in the PDF.
pub const DESCRIPTION: &str = "sRGB IEC61966-2.1";

fn s15f16(v: f64) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

fn xyz_tag(v: [f64; 3]) -> Vec<u8> {
    let mut t = b"XYZ \0\0\0\0".to_vec();
    for c in v {
        t.extend_from_slice(&s15f16(c));
    }
    t
}

fn srgb_decode(v: f64) -> f64 {
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn curve_tag() -> Vec<u8> {
    let mut t = b"curv\0\0\0\0".to_vec();
    t.extend_from_slice(&(CURVE_POINTS as u32).to_be_bytes());
    for i in 0..CURVE_POINTS {
        let y = srgb_decode(i as f64 / (CURVE_POINTS - 1) as f64);
        t.extend_from_slice(&((y * 65535.0).round() as u16).to_be_bytes());
    }
    t
}

/// A version 2 `textDescriptionType`.
fn desc_tag(s: &str) -> Vec<u8> {
    let mut t = b"desc\0\0\0\0".to_vec();
    t.extend_from_slice(&(s.len() as u32 + 1).to_be_bytes());
    t.extend_from_slice(s.as_bytes());
    t.push(0);
    // No Unicode or ScriptCode descriptions: language/count, then code/count and the
    // 67-byte ScriptCode field.
    t.extend_from_slice(&[0; 8]);
    t.extend_from_slice(&[0; 3]);
    t.extend_from_slice(&[0; 67]);
    t
}

fn text_tag(s: &str) -> Vec<u8> {
    let mut t = b"text\0\0\0\0".to_vec();
    t.extend_from_slice(s.as_bytes());
    t.push(0);
    t
}

/// The profile bytes.
pub fn srgb_v2() -> Vec<u8> {
    let trc = curve_tag();
    // (signature, data); the three tone curves share one copy of the data.
    let tags: [(&[u8; 4], Vec<u8>); 6] = [
        (b"desc", desc_tag(DESCRIPTION)),
        (b"cprt", text_tag("No copyright, use freely")),
        (b"wtpt", xyz_tag(D50)),
        (b"rXYZ", xyz_tag(RXYZ)),
        (b"gXYZ", xyz_tag(GXYZ)),
        (b"bXYZ", xyz_tag(BXYZ)),
    ];
    let n_tags = tags.len() + 3;
    let table_end = 128 + 4 + 12 * n_tags;
    let mut data = Vec::new();
    let mut entries = Vec::new();
    let align = |data: &mut Vec<u8>| {
        while !(table_end + data.len()).is_multiple_of(4) {
            data.push(0);
        }
    };
    for (sig, t) in &tags {
        align(&mut data);
        entries.push((**sig, table_end + data.len(), t.len()));
        data.extend_from_slice(t);
    }
    align(&mut data);
    let trc_at = table_end + data.len();
    data.extend_from_slice(&trc);
    for sig in [b"rTRC", b"gTRC", b"bTRC"] {
        entries.push((*sig, trc_at, trc.len()));
    }
    align(&mut data);
    let size = table_end + data.len();

    let mut p = Vec::with_capacity(size);
    p.extend_from_slice(&(size as u32).to_be_bytes());
    p.extend_from_slice(&[0; 4]); // preferred CMM: none
    p.extend_from_slice(&[0x02, 0x10, 0, 0]); // version 2.1
    p.extend_from_slice(b"mntr");
    p.extend_from_slice(b"RGB ");
    p.extend_from_slice(b"XYZ ");
    // Creation date: 2026-01-01 00:00:00 (fixed, so documents are reproducible).
    for v in [2026u16, 1, 1, 0, 0, 0] {
        p.extend_from_slice(&v.to_be_bytes());
    }
    p.extend_from_slice(b"acsp");
    p.extend_from_slice(&[0; 4]); // platform
    p.extend_from_slice(&[0; 4]); // flags
    p.extend_from_slice(&[0; 4]); // manufacturer
    p.extend_from_slice(&[0; 4]); // model
    p.extend_from_slice(&[0; 8]); // attributes
    p.extend_from_slice(&0u32.to_be_bytes()); // perceptual intent
    for c in D50 {
        p.extend_from_slice(&s15f16(c));
    }
    p.extend_from_slice(&[0; 4]); // creator
    p.extend_from_slice(&[0; 16]); // profile ID (none in version 2)
    p.extend_from_slice(&[0; 28]);
    debug_assert_eq!(p.len(), 128);
    p.extend_from_slice(&(n_tags as u32).to_be_bytes());
    for (sig, off, len) in entries {
        p.extend_from_slice(&sig);
        p.extend_from_slice(&(off as u32).to_be_bytes());
        p.extend_from_slice(&(len as u32).to_be_bytes());
    }
    p.extend_from_slice(&data);
    debug_assert_eq!(p.len(), size);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_tags_are_consistent() {
        let p = srgb_v2();
        assert_eq!(
            u32::from_be_bytes(p[0..4].try_into().unwrap()) as usize,
            p.len()
        );
        assert_eq!(&p[36..40], b"acsp");
        assert_eq!(p.len() % 4, 0);
        let n = u32::from_be_bytes(p[128..132].try_into().unwrap()) as usize;
        assert_eq!(n, 9);
        for i in 0..n {
            let e = &p[132 + 12 * i..][..12];
            let off = u32::from_be_bytes(e[4..8].try_into().unwrap()) as usize;
            let len = u32::from_be_bytes(e[8..12].try_into().unwrap()) as usize;
            assert_eq!(off % 4, 0);
            assert!(off + len <= p.len());
        }
    }

    /// Specular's own colour engine reads the profile as plain sRGB: no source transform.
    #[test]
    fn colour_engine_sees_srgb() {
        let p = srgb_v2();
        let prof = specular_color::Profile::parse(&p).expect("parses");
        assert!(prof.is_matrix_shaper());
        assert!(
            specular_color::gpu::source_lut(&p, specular_color::Intent::Perceptual, false)
                .is_none()
        );
    }
}
